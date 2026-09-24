//! RED M-87 круг `R-200` (sacred, architect-only) — **честность истории обязана держаться на
//! ПУТИ ПОДПИСКИ, а не только на legacy-пути.**
//!
//! ## Находка ревьюера, и это МОЙ дефект — причём ровно того класса, который милестоун ловит
//!
//! Задача 20 вынесла обработку сбоя в `history_provenance_for_serve` и потребовала, чтобы
//! транспорт звал ТОЛЬКО её. Я объявил в `§14.1decies`, что «две проверки вместе закрывают и
//! поведение, и подключение»: оракул судит функцию, канарейка приёмки — подключение. **Обе
//! оказались негодны для ВТОРОГО места вызова**, и ревьюер показал это мутацией:
//!
//! · вызов оставлен, а запись признака в снимок снята — клиенту снова уходит ложь о полноте
//!   истории; **не покраснело НИЧЕГО**: ни оракул функции, ни канарейка, ни тесты пакета, ни
//!   `clippy`;
//! · канарейка считала УПОМИНАНИЯ ИМЕНИ в файле, а одно из трёх — импорт (`:2396`), поэтому
//!   порог «не меньше двух вызовов» выполнялся при ОДНОМ живом вызове.
//!
//! Это тот самый класс, который я весь милестоун предъявлял другим и записал в мандаты:
//! **проверка обязана идти по ВЫЗОВУ, а не по присутствию имени.** Своё же требование я
//! нарушил в той самой канарейке, которая должна была его исполнять.
//!
//! ## Почему одного оракула было недостаточно — два разных пути к клиенту
//!
//! Перезапись провенанса стоит в ДВУХ местах `crates/gateway-serve/src/lib.rs`, и они
//! принадлежат РАЗНЫМ путям:
//!
//! | место | функция | чем покрыт ДО этого оракула |
//! |---|---|---|
//! | `:1949` | `run_authorized_session` — env-селектор, снимок при подключении | `red_ws_honesty_sessions::o6_pruned_journal_is_honestly_marked` |
//! | `:1268` | `handle_v1_message` — **путь ADD/SWITCH v1-протокола** | **НИЧЕМ** |
//!
//! `o6` берёт ПЕРВОЕ сообщение соединения, то есть env-снимок. Подписка `subscribe` идёт
//! другим кодом, и её снимок никто не судил. Между тем именно подписка — живой путь продукта:
//! клиент выбирает инструмент ею, а не переменной окружения сервера.
//!
//! **Класс дефекта назван в `testing.md` прямо** («каждая публичная функция — с тестом»,
//! §дегенерированный вход п.2 «множественность»): два места одного контракта — это два
//! места, а не одно, и покрытие одного не переносится на другое автоматически.
//!
//! ## Что здесь проверяется
//!
//! Снимок, пришедший НА ПОДПИСКУ, несёт честный провенанс истории на журнале с удалённым
//! префиксом. Фикстура прод-формы: слепок снят ДО удаления (иначе `checkpoint::advance`
//! законно откажет), префикс удалён физически — как это делает `retention-prune`.
//!
//! Парный vantage (`p2`) обязателен: на ЦЕЛОМ журнале тот же путь НЕ объявляет историю
//! усечённой. Без него реализация «всегда `truncated = true`» прошла бы `p1`.
//!
//! COMPILE/RUNTIME-RED по построению: на ревизии находки `p1` КРАСЕН — снимок ADD-пути несёт
//! замороженное `history_truncated = false`, снятое из слепка ДО удаления префикса.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::server::{bind, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m87-add-path-secret";
const FUTURE: usize = 4_000_000_000;
const N: u64 = 2_000;
const SEG_BYTES: u64 = 8 * 1024;

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m87-add".to_string(),
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
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(100.0 + (i % 7) as f64),
            size: to_fixed(1.0 + (i % 3) as f64),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_752_000_000_000 + i as i64 * 100,
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

fn config(dir: &std::path::Path, ckpt: &std::path::Path) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(ckpt.to_path_buf()),
    }
}

fn journal_with_events() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    dir
}

/// Слепок снимается ДО удаления префикса — иначе `checkpoint::advance` законно откажет
/// по правилу «fail-loud на усечённом префиксе без чекпоинта».
fn warm_checkpoint(dir: &std::path::Path) -> tempfile::TempDir {
    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    gateway::checkpoint::advance(dir, ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance (warm checkpoint)");
    ckpt
}

/// Прод-форма: нижние сегменты удаляются ФИЗИЧЕСКИ, как `retention-prune`.
fn prune_prefix(dir: &std::path::Path) -> u64 {
    for s in journal::list_segments(dir)
        .expect("segments")
        .iter()
        .filter(|s| s.index < 3)
    {
        std::fs::remove_file(&s.path).expect("remove segment");
    }
    journal::stream(dir, EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .next()
        .expect("хотя бы одно событие осталось")
        .expect("event")
        .seq
}

/// Снимок, пришедший НА ПОДПИСКУ (`op:subscribe` → ADD-путь `handle_v1_message`), а не при
/// подключении. Первое сообщение соединения — env-снимок legacy-пути — пропускается явно:
/// его судит `o6`, и спутать их нельзя, иначе оракул проверит не тот код.
async fn snapshot_of_subscribe(dir: &std::path::Path, ckpt: &std::path::Path) -> Value {
    let server = bind(config(dir, ckpt)).await.expect("bind");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    let token = sign();
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/?token={token}"))
        .await
        .expect("connect");

    const SUB_ID: &str = "add-path-probe";
    ws.send(Message::Text(
        json!({"op":"subscribe","v":1,"id":SUB_ID,"selector":{
            "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
            "bands":[0.001],"window_ms":60000}})
        .to_string(),
    ))
    .await
    .expect("send subscribe");

    // Читаем до снимка ИМЕННО нашей подписки. Предел итераций — чтобы оракул не висел:
    // молчащий тест хуже ложно-зелёного, он не отдаёт управление.
    for _ in 0..40 {
        let m = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next())
            .await
            .expect("таймаут ожидания снимка подписки")
            .expect("поток закрыт")
            .expect("ws error");
        let Ok(v) = serde_json::from_slice::<Value>(m.into_data().as_ref()) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) == Some("snapshot")
            && v.get("sub").and_then(Value::as_str) == Some(SUB_ID)
        {
            return v;
        }
    }
    panic!("снимок подписки «{SUB_ID}» не пришёл за 40 сообщений — оракул судил бы не тот путь");
}

fn truncated_of(msg: &Value) -> bool {
    msg.get("data")
        .and_then(|d| d.get("history_truncated"))
        .and_then(Value::as_bool)
        .expect("снимок обязан нести history_truncated (VB-I-11)")
}

fn start_seq_of(msg: &Value) -> u64 {
    msg.get("data")
        .and_then(|d| d.get("history_start_seq"))
        .and_then(Value::as_u64)
        .expect("снимок обязан нести history_start_seq (VB-I-11)")
}

/// **p1 — ГЛАВНЫЙ: снимок ПОДПИСКИ честен об усечённой истории.**
///
/// Слепок снят до удаления префикса и помнит «история полная». Префикс удалён. Снимок,
/// выданный на `subscribe`, обязан объявить усечение — иначе клиент считает `all-time`
/// полным, а реплей его не воспроизведёт (`VB-I-11`).
#[tokio::test]
async fn p1_subscribe_snapshot_declares_pruned_history() {
    let dir = journal_with_events();
    let ckpt = warm_checkpoint(dir.path());
    let earliest = prune_prefix(dir.path());
    assert!(
        earliest > 0,
        "SETUP-СТРАЖ: префикс не удалён, earliest={earliest} — оракул судил бы не тот сценарий"
    );

    let msg = snapshot_of_subscribe(dir.path(), ckpt.path()).await;
    assert!(
        truncated_of(&msg),
        "снимок ПОДПИСКИ объявил историю полной на журнале с удалённым префиксом. \
         Путь ADD/SWITCH (`handle_v1_message`) — живой путь продукта: клиент выбирает \
         инструмент подпиской, а не переменной окружения. Покрытие legacy-пути (`o6`) на \
         него НЕ переносится (`R-200`)"
    );
    assert_eq!(
        start_seq_of(&msg),
        earliest,
        "начало истории обязано быть ВЫЧИСЛЕННЫМ против текущего журнала, а не замороженным \
         из слепка"
    );
}

/// **p2 — ПАРНЫЙ VANTAGE: на целом журнале тот же путь не объявляет усечения.**
///
/// Без него реализация «всегда `truncated = true`» прошла бы `p1` и обесценила обе проверки.
#[tokio::test]
async fn p2_subscribe_snapshot_on_intact_journal_is_not_truncated() {
    let dir = journal_with_events();
    let ckpt = warm_checkpoint(dir.path());

    let msg = snapshot_of_subscribe(dir.path(), ckpt.path()).await;
    assert!(
        !truncated_of(&msg),
        "целый журнал объявлен усечённым — «всегда truncated» честностью не является"
    );
    assert_eq!(start_seq_of(&msg), 0, "на целом журнале начало истории — 0");
}
