//! RED `M-100` (sacred, architect-only) — **три контракта выдачи на ПРОВОДЕ** (S1a-K; `CT-RFC-09`
//! §2.10; спека `milestones/M-100-three-serving-contracts.md`).
//!
//! | контракт | что пиннится здесь |
//! |---|---|
//! | что сервер СЧИТАЕТ | ключ расчёта резолвит СЕРВЕР: полосы — из профиля, клиент их не задаёт; `bands` ≠ профилю — `unsupported` (а не `not_ready`); `window_ms`/`depth_cadence_ms` вне `allowed_profiles` — `unsupported` (`TD-242`) |
//! | что пользователь СМОТРИТ | `view.depth_rows` выбирает строки глубины из ГОТОВОГО расчёта — в снимке и в кадрах; неверный выбор — `invalid_view` |
//! | как это ДОСТАВЛЯЕТСЯ | `GATEWAY_SCHEMA_VERSION = 12`; снимок несёт `calc_key_id`, `calc_profile`, `heatmap_window_e8`; конверт снимка повторяет принятый `view` |
//!
//! Путь — прод-форма: `bind_with_policy` → `serve` → WS с JWT → `subscribe` v1 → снимок/кадр по
//! проводу. Слепок пишет прод-путь `checkpoint::advance`, выдача читает его тёплым путём.
//!
//! COMPILE-RED нет: файл собирается на сегодняшнем коде и КРАСЕН по поведению (снимок без новых
//! полей, `bands` обязателен, выбора отображения нет).
use std::time::Duration;

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::server::{bind_with_policy, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m100-three-contracts";
const SEVEN: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6];
const SEVEN_E8: [i64; 7] = [
    1_500_000, 3_000_000, 5_000_000, 8_000_000, 15_000_000, 30_000_000, 60_000_000,
];
const T0: i64 = 1_752_000_000_000;
const MID: f64 = 65_000.0;
const BUDGET: Duration = Duration::from_secs(20);
/// Независимо посчитанный `sha256` канонической формы прод-ключа (`red_m100_calc_key_id` `k1`).
const PROD_KEY_ID: &str = "201e0f954c40013f39fdfc8c7db729c19fe026c85a44d79d665a1aca7653c081";
const PROFILE_SHA: &str = "abababababababababababababababababababababababababababababababab";

static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn resolved() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1000,
        bands: SEVEN.to_vec(),
        window_ms: Some(60_000),
        depth_cadence_ms: Some(1000),
    }
}

fn lv(v: Vec<(f64, f64)>) -> Vec<Level> {
    v.into_iter()
        .map(|(p, s)| Level {
            price: to_fixed(p),
            size: to_fixed(s),
        })
        .collect()
}

fn append_book(dir: &std::path::Path, from_s: i64, n_s: i64) {
    let mut j = Journal::open_with(
        dir,
        WriterConfig {
            max_segment_bytes: 1 << 22,
            min_free_bytes: 0,
            source: DataSource::OwnCapture,
            provenance: "m100".to_string(),
            epoch_id: "own-test".to_string(),
        },
    )
    .expect("open_with");
    let step = MID * 0.005;
    for t in from_s..from_s + n_s {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: lv((1..=119).map(|k| (MID - k as f64 * step, 1.0)).collect()),
                asks: lv((1..=119).map(|k| (MID + k as f64 * step, 1.0)).collect()),
                ts_exch_ms: T0 + t * 1_000,
            },
        ))
        .expect("append");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID),
                size: to_fixed(1.0),
                side: Side::Buy,
                ts_exch_ms: T0 + t * 1_000,
            },
        ))
        .expect("append");
    }
    j.flush().expect("flush");
}

fn fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("journal");
    append_book(dir.path(), 0, 5);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &resolved(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    (dir, ckpt)
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        allowed_symbols: vec!["BTCUSDT".to_string()],
        canonical_bands: SEVEN.to_vec(),
        allowed_profiles: vec![LiveProfile {
            timeframe_ms: 1000,
            window_ms: 60_000,
            depth_cadence_ms: Some(1000),
        }],
        max_concurrent_serves: 4,
        max_tail_events: 1_000_000,
        expected_warmup_events: 1,
    }
}

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn start(dir: &std::path::Path, ckpt: &std::path::Path) -> Ws {
    gateway_serve::server::set_effective_grace_ms(5_000);
    gateway::set_effective_heatmap_window_frac(0.001);
    gateway::calc_profile::set_effective_calc_profile(1, PROFILE_SHA.to_string());
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: resolved(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(ckpt.to_path_buf()),
    };
    let server = bind_with_policy(cfg, policy())
        .await
        .expect("bind_with_policy");
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    let token = jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m100".to_string(),
            exp: 4_000_000_000,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt");
    tokio::time::timeout(
        BUDGET,
        tokio_tungstenite::connect_async(format!("ws://{addr}/?token={token}")),
    )
    .await
    .expect("connect timeout")
    .expect("connect")
    .0
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string())).await.expect("send");
}

fn sel_json(bands: Option<Vec<f64>>, window_ms: i64) -> Value {
    let mut s = json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
                       "window_ms":window_ms,"depth_cadence_ms":1000});
    if let Some(b) = bands {
        s["bands"] = json!(b);
    }
    s
}

/// Следующее сообщение подписки `id` с типом `want` (`snapshot`|`frame`|`error`); прочие типы
/// той же подписки — провал (например, снимок там, где ожидалась ошибка).
async fn next_of(ws: &mut Ws, id: &str, want: &str) -> Value {
    let deadline = tokio::time::Instant::now() + BUDGET;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let m = match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(m))) => m,
            other => panic!("за {BUDGET:?} не пришло «{want}» для «{id}»: {other:?}"),
        };
        let Ok(v) = serde_json::from_slice::<Value>(m.into_data().as_ref()) else {
            continue;
        };
        if v.get("sub").and_then(Value::as_str) != Some(id) {
            continue;
        }
        let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
        if ty == want {
            return v;
        }
        if want == "frame" && ty == "frame" {
            return v;
        }
        if ty == "frame" && want != "frame" {
            continue;
        }
        panic!("для «{id}» ожидалось «{want}», пришло: {v}");
    }
}

fn rows(data: &Value) -> Vec<(i64, String)> {
    data["series"]["depth_series"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| {
                    (
                        r["band_pct_e8"].as_i64().unwrap_or(-1),
                        r["side"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn all_rows() -> Vec<(i64, String)> {
    let mut v = Vec::new();
    for b in SEVEN_E8 {
        for s in ["bid", "ask"] {
            v.push((b, s.to_string()));
        }
    }
    v.sort();
    v
}

/// **`w1` — клиент НЕ задаёт полосы, выбирает ОДНУ строку к отображению.** Сервер резолвит ключ
/// расчёта из профиля; снимок несёт новые поля провода и ровно выбранную строку.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w1_server_resolves_calc_key_and_view_selects_rows() {
    let _s = SERIAL.lock().await;
    let (dir, ckpt) = fixture();
    let mut ws = start(dir.path(), ckpt.path()).await;
    let view = json!({"depth_rows":[{"band_pct_e8":15_000_000,"side":"bid"}]});
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w1","selector":sel_json(None, 60_000),"view":view}),
    )
    .await;
    let m = next_of(&mut ws, "w1", "snapshot").await;
    let d = &m["data"];
    assert_eq!(
        d["schema_version"],
        json!(12),
        "M-100: провод обязан быть v12: {m}"
    );
    assert_eq!(
        d["selector"]["bands"],
        json!(SEVEN.to_vec()),
        "M-100 / П-029: полосы ключа расчёта резолвит СЕРВЕР из профиля"
    );
    assert_eq!(
        d["calc_key_id"],
        json!(PROD_KEY_ID),
        "M-100: calc_key_id снимка: {m}"
    );
    assert_eq!(
        d["calc_profile"],
        json!({"version":1,"sha256":PROFILE_SHA}),
        "M-100 / П-032 п.4(в): применённая версия профиля обязана быть в снимке"
    );
    assert_eq!(
        d["heatmap_window_e8"],
        json!(100_000),
        "M-100 / TD-197: окно карты (0.001 ×1e8) обязано быть объявлено в снимке"
    );
    assert_eq!(
        m["view"], view,
        "M-100: конверт снимка обязан повторять принятый выбор"
    );
    assert_eq!(
        rows(d),
        vec![(15_000_000, "bid".to_string())],
        "M-100 / П-029: снимок обязан нести РОВНО выбранную строку глубины"
    );

    // Кадры — тот же выбор: дописать журнал и дождаться кадра подписки.
    append_book(dir.path(), 5, 3);
    let f = next_of(&mut ws, "w1", "frame").await;
    let fr = &f["data"]["delta"]["depth_series"];
    if let Some(a) = fr.as_array() {
        for r in a {
            assert_eq!(
                (r["band_pct_e8"].as_i64(), r["side"].as_str()),
                (Some(15_000_000), Some("bid")),
                "M-100: кадр несёт невыбранную строку глубины: {f}"
            );
        }
    }
}

/// **`w2` — без выбора — все 14 строк** (обратная совместимость показа).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w2_absent_view_shows_all_rows() {
    let _s = SERIAL.lock().await;
    let (dir, ckpt) = fixture();
    let mut ws = start(dir.path(), ckpt.path()).await;
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w2","selector":sel_json(None, 60_000)}),
    )
    .await;
    let m = next_of(&mut ws, "w2", "snapshot").await;
    let mut got = rows(&m["data"]);
    got.sort();
    assert_eq!(
        got,
        all_rows(),
        "M-100: без выбора отображения — все строки расчёта"
    );
}

/// **`w3` — клиентские полосы ≠ профилю (подмножество) — НАЗВАННЫЙ отказ `unsupported`**, а не
/// `not_ready` (`П-030`: отказ назван, параметры не подменяются). Сообщение указывает на `view`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w3_client_band_subset_is_unsupported_and_points_to_view() {
    let _s = SERIAL.lock().await;
    let (dir, ckpt) = fixture();
    let mut ws = start(dir.path(), ckpt.path()).await;
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w3","selector":sel_json(Some(vec![0.15]), 60_000)}),
    )
    .await;
    let e = next_of(&mut ws, "w3", "error").await;
    assert_eq!(e["code"], json!("unsupported"), "M-100: {e}");
    assert!(
        e["message"].as_str().unwrap_or("").contains("view"),
        "M-100: отказ по полосам обязан указывать, как выбрать отображение (`view`): {e}"
    );
}

/// **`w4` — полосы, РАВНЫЕ профилю, принимаются** (переходная совместимость клиентов v1).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w4_client_bands_equal_to_profile_are_accepted() {
    let _s = SERIAL.lock().await;
    let (dir, ckpt) = fixture();
    let mut ws = start(dir.path(), ckpt.path()).await;
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w4","selector":sel_json(Some(SEVEN.to_vec()), 60_000)}),
    )
    .await;
    let m = next_of(&mut ws, "w4", "snapshot").await;
    assert_eq!(m["data"]["calc_key_id"], json!(PROD_KEY_ID));
}

/// **`w5` — `window_ms` вне `allowed_profiles` — `unsupported`** (`TD-242`: сегодня клиентская
/// тройка после допуска не сверяется, и клиент получает `not_ready` по чужому отпечатку).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w5_window_outside_allowed_profiles_is_unsupported() {
    let _s = SERIAL.lock().await;
    let (dir, ckpt) = fixture();
    let mut ws = start(dir.path(), ckpt.path()).await;
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w5","selector":sel_json(None, 30_000)}),
    )
    .await;
    let e = next_of(&mut ws, "w5", "error").await;
    assert_eq!(e["code"], json!("unsupported"), "M-100 / TD-242: {e}");
}

/// **`w6` — неверный выбор отображения — `invalid_view`**, соединение живо.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w6_invalid_view_is_named_error() {
    let _s = SERIAL.lock().await;
    let (dir, ckpt) = fixture();
    let mut ws = start(dir.path(), ckpt.path()).await;
    for (id, view) in [
        (
            "w6a",
            json!({"depth_rows":[{"band_pct_e8":20_000_000,"side":"bid"}]}),
        ),
        ("w6b", json!({"depth_rows":[]})),
    ] {
        send(
            &mut ws,
            json!({"op":"subscribe","v":1,"id":id,"selector":sel_json(None, 60_000),"view":view}),
        )
        .await;
        let e = next_of(&mut ws, id, "error").await;
        assert_eq!(e["code"], json!("invalid_view"), "M-100: {e}");
    }
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w6c","selector":sel_json(None, 60_000)}),
    )
    .await;
    let _ = next_of(&mut ws, "w6c", "snapshot").await;
}

/// **`w7` — смена выбора = переподписка с тем же `id`:** новый снимок несёт новый выбор, расчёт
/// тот же (`calc_key_id` не меняется).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w7_resubscribe_changes_view_not_calc() {
    let _s = SERIAL.lock().await;
    let (dir, ckpt) = fixture();
    let mut ws = start(dir.path(), ckpt.path()).await;
    let v1 = json!({"depth_rows":[{"band_pct_e8":15_000_000,"side":"bid"}]});
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w7","selector":sel_json(None, 60_000),"view":v1}),
    )
    .await;
    let a = next_of(&mut ws, "w7", "snapshot").await;
    let v2 = json!({"depth_rows":[{"band_pct_e8":60_000_000,"side":"ask"}]});
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w7","selector":sel_json(None, 60_000),"view":v2}),
    )
    .await;
    let b = next_of(&mut ws, "w7", "snapshot").await;
    assert_eq!(rows(&b["data"]), vec![(60_000_000, "ask".to_string())]);
    assert_eq!(b["view"], v2);
    assert_eq!(a["data"]["calc_key_id"], b["data"]["calc_key_id"]);
}

/// Слепок прод-пути, у которого байты `12..16` заголовка (версия, которую сверяют и
/// `read_and_validate`, и `admission::readiness`) ЯВНО переписаны на `v` — независимо от того,
/// что пишет сегодняшний писатель (`C-297` M-100 R2).
fn fixture_with_header_version(v: u32) -> (tempfile::TempDir, tempfile::TempDir) {
    let (dir, ckpt) = fixture();
    let p = gateway::checkpoint::ckpt_path_for_pub(ckpt.path(), &resolved());
    let mut b = std::fs::read(&p).expect("read ckpt");
    assert!(b.len() > 16, "SETUP НЕ СОСТОЯЛСЯ: слепок короче заголовка");
    b[12..16].copy_from_slice(&v.to_le_bytes());
    std::fs::write(&p, &b).expect("write ckpt");
    (dir, ckpt)
}

/// **`w8` — слепок, записанный ДО `M-100` (версия состояния 11), принимается ДОПУСКОМ при проводе
/// 12:** подписка получает снимок v12, а не `not_ready`. Это прод-случай первого деплоя `M-100`:
/// прод-слепок `ckpt-8f69809dd707e8c9.bin` несёт 11. Мутация ТОЛЬКО `admission::readiness` обратно
/// на `GATEWAY_SCHEMA_VERSION` делает этот сценарий красным.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w8_pre_m100_checkpoint_state_11_admitted_under_wire_12() {
    let _s = SERIAL.lock().await;
    let (dir, ckpt) = fixture_with_header_version(11);
    let mut ws = start(dir.path(), ckpt.path()).await;
    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"w8","selector":sel_json(Some(SEVEN.to_vec()), 60_000)}),
    )
    .await;
    let m = next_of(&mut ws, "w8", "snapshot").await;
    assert_eq!(
        m["data"]["schema_version"],
        json!(12),
        "M-100 / план §15.6: слепок с версией состояния 11 обязан дать снимок провода v12: {m}"
    );
}

/// **`w9` — слепок с ЧУЖОЙ версией состояния (10, и 12 = версия провода) допуск НЕ принимает:**
/// названный `not_ready`, а не тихий холодный пересчёт и не принятие (развязка ≠ «принимать всё»).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn w9_foreign_state_version_is_not_ready() {
    let _s = SERIAL.lock().await;
    for v in [10u32, 12] {
        let (dir, ckpt) = fixture_with_header_version(v);
        let mut ws = start(dir.path(), ckpt.path()).await;
        let id = format!("w9-{v}");
        send(
            &mut ws,
            json!({"op":"subscribe","v":1,"id":id,"selector":sel_json(Some(SEVEN.to_vec()), 60_000)}),
        )
        .await;
        let e = next_of(&mut ws, &id, "error").await;
        assert_eq!(
            e["code"],
            json!("not_ready"),
            "M-100: слепок с версией состояния {v} ≠ CALC_STATE_VERSION допущен: {e}"
        );
    }
}
