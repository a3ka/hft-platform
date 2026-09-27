//! RED `M-89` (sacred, architect-only) — **СТРУКТУРНАЯ граница работы одной выдачи на
//! РЕАЛЬНОМ WS-пути** (`I-3`, `TD-219` по форме, план §4 п. 2 / `PL-I-4`).
//!
//! # Что заменяется и почему
//!
//! `M-87` объявил бюджет вызова (`CallBudget`/`BudgetStop`/`Cancel`/`feed_tail_within`) и
//! так и не подключил его к пути выдачи: `grep -c 'feed_tail_within(' src/lib.rs → 0`
//! (`TD-219`). Причина не в забывчивости: механизм был ПЛАЦЕБО по построению —
//! `pump_one` читал ВСЕ сегменты каталога с начала на КАЖДОЙ порции, не зная ни курсора,
//! ни селектора (`R-201` Б-1: 88 МБ холостого чтения за итерацию), а `BudgetStop`
//! отбрасывался. Подключить его значило бы подключить дефект.
//!
//! `M-89` меняет постановку: предохранитель по объёму — не счётчик поверх чтения, а
//! ГРАНИЦА САМОГО ЧТЕНИЯ. Допущенный запрос (`readiness = Ready` ⇔ хвост ≤
//! `max_tail_events`) обязан стоить `f(max_tail_events)`, а не `f(длина активного
//! сегмента)`. Тогда бюджет не нужен как отдельный механизм: `max_tail_events` — И ЕСТЬ
//! бюджет, и он уже стоит на точке входа (`admit → try_acquire → readiness`, §4.0bis `M-87`).
//!
//! # Мера
//!
//! `rchar` из `/proc/self/io` — байты через `read()` ВСЕГО процесса (сервер поднят в этом
//! же процессе через `bind_with_policy` + `tokio::spawn`, как в `red_m87_read_volume_truth.rs`).
//! Один сценарий на бинарь: величина процессная.
//!
//! # Прод-форма
//!
//! ОДИН сырой активный сегмент (1 ГиБ конфиг), префикс 200 тыс. событий перед курсором
//! слепка (замер 2026-09-27: 304 883 на проде), хвост 500 ≤ `max_tail_events`. Слепок
//! снимается ДО дописки хвоста. Запрос идёт через настоящий `subscribe` v1, ждём
//! `snapshot` И первый `frame` — первый `pump` живёт в периодическом push-цикле
//! (`PUSH_INTERVAL_MS = 250`), и именно он сегодня читает префикс.
//!
//! # Два режима входа — оба судятся (`C-260` N3)
//!
//! `s1` — v1 (`subscribe` с `v:1`): первый `pump` живёт в push-цикле `run_v1_session_loop`.
//! `s2` — legacy (клиент молчит grace-окно, env-селектор, OLD wire `{"Snapshot":…}`): тёплый
//! `resume` + drain-цикл `pump(LEGACY_DRAIN_BATCH)` ДО снимка (`run_authorized_session`).
//! Оба зовут общий `LiveReducer::pump`, но точки входа разные, и «второй режим унаследует
//! границу» — довод, а не замер; замер дешевле довода: та же фикстура, второй сервер.
//! `rchar` процессный ⇒ оба сценария в ОДНОМ теле, последовательно.
//!
//! # Чего оракул НЕ ловит
//!
//! · время (мерит хост); · правдивость данных (другой класс); · параллелизм (`C4` `M-87`);
//! · счётчик выдачи (`red_m89_read_volume_truth.rs`).
//!
//! RUNTIME-RED на ревизии набора: `s1`/`s2` — прочитано ≈ размер файла (10 МБ) при пороге
//! `ckpt + tail·4 + 2 МиБ`.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::server::{bind_with_policy, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m89-structural-bound";
const FUTURE: usize = 4_000_000_000;
const BASE_MS: i64 = 1_784_116_800_000;
const BUDGET: std::time::Duration = std::time::Duration::from_secs(60);
const CANONICAL: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

/// Прод-форма: один сырой сегмент на 1 ГиБ.
const SEG_BYTES: u64 = 1 << 30;
/// Префикс перед курсором слепка — прод-масштаб (мандат: ≥ 200 тыс.).
const PREFIX: u64 = 200_000;
/// Хвост после слепка — строго ниже порога свежести, запрос ДОПУЩЕН.
const TAIL: u64 = 500;
const MAX_TAIL_EVENTS: u64 = 1_000;
const EXPECTED_WARMUP_EVENTS: u64 = 100;
/// Допуск на поиск позиции, опись каталога, буфер заголовка и служебное чтение процесса
/// (JWT, /proc): 2 МиБ. Дефектный путь читает весь файл — ≈ 10 МБ при 200 тыс. × ~51 Б.
const ALLOWANCE: u64 = 2 * 1024 * 1024;

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m89".to_string(),
            exp: FUTURE,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt")
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m89".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(65_000.0 + (i % 997) as f64 * 0.01),
            size: to_fixed(0.001 + (i % 101) as f64 * 0.0001),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: BASE_MS + i as i64 * 7,
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: CANONICAL.to_vec(),
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        allowed_symbols: vec!["BTCUSDT".to_string()],
        canonical_bands: CANONICAL.to_vec(),
        allowed_profiles: vec![LiveProfile {
            timeframe_ms: 1_000,
            window_ms: 60_000,
            depth_cadence_ms: None,
        }],
        max_concurrent_serves: 1,
        max_tail_events: MAX_TAIL_EVENTS,
        expected_warmup_events: EXPECTED_WARMUP_EVENTS,
    }
}

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о границе работы: фикстура не воспроизвела \
         сценарий, ради которого оракул написан."
    )
}

fn rchar() -> u64 {
    let s = std::fs::read_to_string("/proc/self/io")
        .unwrap_or_else(|e| setup_failed(&format!("/proc/self/io недоступен ({e})")));
    for line in s.lines() {
        if let Some(v) = line.strip_prefix("rchar:") {
            return v.trim().parse().expect("rchar — число");
        }
    }
    setup_failed("в /proc/self/io нет строки rchar")
}

fn dir_bytes(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

fn segments(dir: &std::path::Path) -> usize {
    journal::list_segments(dir).expect("segments").len()
}

struct Fixture {
    dir: tempfile::TempDir,
    ckpt: tempfile::TempDir,
    ckpt_bytes: u64,
    tail_bytes: u64,
    file_bytes: u64,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..PREFIX {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance (тёплый слепок)");
    let ckpt_bytes = std::fs::metadata(gateway::checkpoint::ckpt_path_for_pub(ckpt.path(), &sel()))
        .map(|m| m.len())
        .unwrap_or_else(|e| setup_failed(&format!("файл слепка: {e}")));
    let before = dir_bytes(dir.path());
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("reopen");
        for i in PREFIX..PREFIX + TAIL {
            j.append(trade(i)).expect("append tail");
        }
        j.flush().expect("flush tail");
    }
    let file_bytes = dir_bytes(dir.path());
    Fixture {
        ckpt_bytes,
        tail_bytes: file_bytes.saturating_sub(before),
        file_bytes,
        dir,
        ckpt,
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn recv(ws: &mut Ws) -> Option<Value> {
    match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref()).ok(),
        _ => None,
    }
}

/// Один допущенный запрос по прод-форме: `subscribe` v1 → `snapshot` → первый `frame`.
/// Возвращает прирост `rchar` вокруг ВСЕГО обслуживания (включая первый pump push-цикла).
async fn serve_admitted_request(f: &Fixture) -> u64 {
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: f.dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(f.ckpt.path().to_path_buf()),
    };
    let server = bind_with_policy(cfg, policy())
        .await
        .expect("bind_with_policy");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let before = rchar();
    let token = sign();
    let (mut ws, _) = tokio::time::timeout(
        BUDGET,
        tokio_tungstenite::connect_async(format!("ws://{addr}/?token={token}")),
    )
    .await
    .expect("подключение не состоялось")
    .expect("connect");
    let sub = json!({"op":"subscribe","v":1,"id":"s1","selector":{
        "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
        "bands":CANONICAL.to_vec(),"window_ms":60000}});
    ws.send(Message::Text(sub.to_string())).await.expect("send");
    let first = recv(&mut ws)
        .await
        .unwrap_or_else(|| setup_failed("сервер промолчал"));
    if first.get("type").and_then(Value::as_str) != Some("snapshot") {
        setup_failed(&format!(
            "допущенный запрос не обслужен снимком: {first} — оракул судит СТОИМОСТЬ обслуженного \
             запроса, а не отказ"
        ));
    }
    // Первый `pump` — в push-цикле; ждём первый кадр с хвостом.
    let mut got_frame = false;
    for _ in 0..8 {
        match recv(&mut ws).await {
            Some(m) if m.get("type").and_then(Value::as_str) == Some("frame") => {
                got_frame = true;
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    if !got_frame {
        setup_failed("первый кадр хвоста не пришёл — первый pump не наблюдался, мерилась пустота");
    }
    rchar().saturating_sub(before)
}

/// Legacy-режим: клиент подключается и МОЛЧИТ grace-окно ⇒ env-селектор, OLD wire.
/// В legacy весь хвост ДРЕНИРУЕТСЯ ДО снимка (`run_authorized_session`: resume + цикл
/// `pump(LEGACY_DRAIN_BATCH)` до пустого кадра), поэтому дорогая работа заканчивается на
/// `{"Snapshot":…}`; кадров после него без новых событий не будет. Setup-страж: курсор
/// снимка == последнее событие журнала (дренаж состоялся). Возвращает прирост `rchar`.
async fn serve_legacy_request(f: &Fixture) -> u64 {
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: f.dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(f.ckpt.path().to_path_buf()),
    };
    let server = bind_with_policy(cfg, policy())
        .await
        .expect("bind_with_policy (legacy)");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let before = rchar();
    let token = sign();
    let (mut ws, _) = tokio::time::timeout(
        BUDGET,
        tokio_tungstenite::connect_async(format!("ws://{addr}/?token={token}")),
    )
    .await
    .expect("подключение (legacy) не состоялось")
    .expect("connect");
    // Ничего не шлём: по истечении grace сервер уходит в legacy-режим.
    let first = recv(&mut ws)
        .await
        .unwrap_or_else(|| setup_failed("legacy: сервер промолчал"));
    let Some(snap) = first.get("Snapshot") else {
        setup_failed(&format!(
            "legacy: первое сообщение — не OLD-wire Snapshot: {first} — режим не legacy, \
             оракул судит не тот вход"
        ));
    };
    let read = rchar().saturating_sub(before);
    let upto = snap
        .get("cursor")
        .and_then(|c| c.get("upto_seq"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| setup_failed(&format!("legacy: в снимке нет cursor.upto_seq: {snap}")));
    if upto != PREFIX + TAIL - 1 {
        setup_failed(&format!(
            "legacy: курсор снимка {upto} ≠ последнему событию {} — хвост не дренирован до снимка, \
             дорогая работа не наблюдалась",
            PREFIX + TAIL - 1
        ));
    }
    drop(ws);
    read
}

/// **s0 + s1 + s2 — один сценарий (rchar процессный).**
#[tokio::test]
async fn s_admitted_request_costs_tail_not_active_segment() {
    let f = fixture();

    // ── s0: SETUP-СТРАЖИ прод-формы ─────────────────────────────────────────────
    let segs = segments(f.dir.path());
    if segs != 1 {
        setup_failed(&format!(
            "сегментов {segs}, а прод-форма — ОДИН активный сырой сегмент с префиксом и хвостом"
        ));
    }
    if f.tail_bytes == 0 || f.file_bytes < f.tail_bytes * 100 {
        setup_failed(&format!(
            "файл {} Б при хвосте {} Б — префикс меньше стократного, «∝ хвосту» и «∝ файлу» \
             неразличимы",
            f.file_bytes, f.tail_bytes
        ));
    }
    if TAIL + 1 > MAX_TAIL_EVENTS {
        setup_failed("хвост выше порога свежести — запрос не был бы допущен");
    }

    let read = serve_admitted_request(&f).await;
    let limit = f.ckpt_bytes + f.tail_bytes * 4 + ALLOWANCE;

    // ── s1: СТРУКТУРНАЯ граница, v1-вход ─────────────────────────────────────────
    assert!(
        read <= limit,
        "I-3 / TD-219 / PL-I-4: допущенный запрос (хвост {TAIL} ≤ max_tail_events \
         {MAX_TAIL_EVENTS}) стоил {read} Б чтения при пороге {limit} Б \
         (слепок {} + хвост {}·4 + допуск {ALLOWANCE}); файл активного сегмента — {} Б. \
         Работа одной выдачи ∝ ДЛИНЕ АКТИВНОГО СЕГМЕНТА, а не допущенному хвосту: первый pump \
         после тёплого resume читает сегмент с header_end (`tail_hint = None`). На проде — \
         ≈412 МБ на подписку сразу после прогрева, до 1 ГБ к ротации (замер 2026-09-27). \
         Бюджет как счётчик поверх этого чтения проблему не решает — граница обязана быть \
         у самого чтения.",
        f.ckpt_bytes,
        f.tail_bytes,
        f.file_bytes
    );

    // ── s2: та же граница на LEGACY-входе (C-260 N3) ─────────────────────────────
    let read_legacy = serve_legacy_request(&f).await;
    assert!(
        read_legacy <= limit,
        "I-3 / C-260 N3: legacy-сессия (env-селектор, OLD wire) стоила {read_legacy} Б при пороге \
         {limit} Б; файл активного сегмента — {} Б. Второй режим входа делает тёплый resume + \
         drain через `run_authorized_session`, и граница обязана держаться и там.",
        f.file_bytes
    );
}
