//! RED `PL-I-5` УРОВЕНЬ 2 (sacred, architect-only) — **ДЕГЕНЕРИРОВАННЫЙ ВХОД: КЛИЕНТ НЕ
//! РОНЯЕТ ОБРАБОТЧИК ОДНОЙ СТРОКОЙ.**
//!
//! Милестоун `milestones/M-71-egress-cap.md`. Исполнение вердикта
//! `research/reviews/R-133-M-71-egress-cap-impl.md`, блокер **B-4**.
//!
//! # Предмет
//!
//! Код, добавленный РОВНО ради защиты от перегрузки (`&name[..MAX_VENUE_ECHO]`,
//! `crates/gateway-serve/src/lib.rs:772` и `:786`), режет `String` по БАЙТУ. В Rust срез
//! `&str` по индексу, не попавшему на границу символа, — паника. Замер `R-133` через
//! настоящий сокет: `venue = "日"×100` (300 Б) ⇒ `panic: byte index 256 is not a char
//! boundary`, клиенту не приходит НИЧЕГО.
//!
//! Комментарий рядом с кодом говорит «256 символов» — код считает БАЙТЫ. Это не опечатка в
//! комментарии: расхождение и есть дефект, потому что для ASCII они совпадают, а для всего
//! остального нет.
//!
//! # Почему отдельным файлом от `red_egress_cap_wire.rs`
//!
//! Тот файл судит ДЛИНУ сообщения. Этот судит ФАКТ ДОСТАВКИ на вырожденном входе — другая
//! величина и другой класс (`testing.md` §«Дегенерированный вход обязателен», п.4 «границы»).
//! Смешение дало бы файл, у которого два предмета и ни одного названного.
//!
//! # Анти-плацебо в ОБЕ стороны
//!
//! `U-C1` (контроль) — ASCII-строка ТОЙ ЖЕ длины обязана давать честную ошибку. Без него
//! оракул был бы зелен против реализации, которая просто перестала отвечать на длинный
//! `venue` вообще. `W-C3` соседнего файла этого не покрывает: он гоняет КОРОТКУЮ ASCII-строку
//! и к границе среза не подходит.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;
const SECRET: &[u8] = b"m71-egress-secret";
const PROD_BAND: f64 = 0.001;
const SHORT_SUB: &str = "s1";

/// Щедрый бюджет ожидания. Истечение = НЕСОСТОЯВШИЙСЯ SETUP, не вердикт о пределе.
const BUDGET: Duration = Duration::from_secs(20);

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn sign() -> String {
    let claims = Claims {
        sub: "m71".to_string(),
        exp: 9_999_999_999,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET),
    )
    .expect("encode")
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "PL-I-5 wire-level fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![PROD_BAND],
        window_ms: None,
    }
}

fn trade(i: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(MID + i as f64 * 0.01),
            size: to_fixed(1.0),
            side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
            ts_exch_ms: T0 + i,
        },
    )
}

fn journal_of_trades(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..n as i64 {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn config(dir: &Path) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: None,
    }
}

async fn serve_on(dir: &Path) -> String {
    let server = bind(config(dir)).await.expect("bind");
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    addr
}

async fn connect(addr: &str) -> Ws {
    let url = format!("ws://{addr}/?token={}", sign());
    let (ws, _r) = tokio::time::timeout(BUDGET, tokio_tungstenite::connect_async(url))
        .await
        .expect("SETUP: бюджет коннекта истёк")
        .expect("SETUP: connect");
    ws
}

fn subscribe(sub: &str, venue: &str) -> Value {
    json!({
        "op": "subscribe",
        "v": 1,
        "id": sub,
        "selector": {
            "venue": venue,
            "symbol": "BTCUSDT",
            "timeframe_ms": 1000,
            "bands": [PROD_BAND],
        }
    })
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string()))
        .await
        .expect("SETUP: send");
}

/// ОДНО сообщение с сокета: его РАЗМЕР В БАЙТАХ и разобранное тело.
/// `None` — сервер промолчал в пределах бюджета (несостоявшийся setup, не вердикт).
async fn recv(ws: &mut Ws) -> Option<(usize, Value)> {
    match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => {
            let data = m.into_data();
            let n = data.len();
            serde_json::from_slice::<Value>(data.as_ref())
                .ok()
                .map(|v| (n, v))
        }
        _ => None,
    }
}

/// Диагностика несостоявшегося setup'а — отдельно от вердикта (`testing.md`, «Целостность
/// гейта» свойство 3).
fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт: сервер не поднялся либо не ответил в \
         пределах бюджета {BUDGET:?}."
    )
}

/// Длина `venue`, заведомо превышающая `MAX_VENUE_ECHO = 256` (`lib.rs:766`).
/// ASCII-вариант и многобайтовый обязаны вести себя ОДИНАКОВО — в этом весь предмет.
const LONG_ASCII: usize = 300;

/// `"日"` — три байта. Сто повторов = 300 Б; байт 256 приходится на СЕРЕДИНУ символа
/// (256 не делится на 3), то есть ровно на ту границу, которую срез по байту разрывает.
fn multibyte_venue() -> String {
    "日".repeat(100)
}

/// Ждём сообщение типа `error` (или любое) — и РАЗЛИЧАЕМ «пришло» от «не пришло».
async fn first_message(addr: &str, venue: &str) -> Option<(usize, Value)> {
    let mut ws = connect(addr).await;
    send(&mut ws, subscribe(SHORT_SUB, venue)).await;
    for _ in 0..4 {
        match recv(&mut ws).await {
            Some((n, v)) => return Some((n, v)),
            None => continue,
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// КОНТРОЛЬ — первым
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **U-C1 — ДЛИННАЯ ASCII-площадка: обработчик отвечает честной ошибкой.**
///
/// Доказывает, что путь эхо-усечения ДОСТИЖИМ этой фикстурой. Без него красное `U1` было бы
/// неотличимо от «сервер вообще не отвечает на длинный `venue`».
#[tokio::test]
async fn pl_i_5_u_c1_long_ascii_venue_gets_an_honest_error() {
    let dir = journal_of_trades(50);
    let addr = serve_on(dir.path()).await;
    let venue = "A".repeat(LONG_ASCII);

    let Some((n, v)) = first_message(&addr, &venue).await else {
        setup_failed(
            "на ДЛИННУЮ ASCII-площадку сервер не ответил ничем — путь эхо-усечения этой \
             фикстурой не достигается, и судить U1 не на чем",
        )
    };
    assert_eq!(
        v.get("code").and_then(|c| c.as_str()),
        Some("unknown_venue"),
        "PL-I-5 U-C1: ожидался машиночитаемый код ошибки (`CT-RFC-09` §2.7); получено {n} Б: {v}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// ПРЕДМЕТ
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **U1 (`R-133` B-4) — МНОГОБАЙТОВАЯ площадка НЕ роняет обработчик.**
///
/// Краснеет против сегодняшней реализации: срез по байту на границе символа паникует, задача
/// обработчика умирает, клиент не получает НИЧЕГО. Отсутствие ответа здесь — это ВЕРДИКТ, а не
/// несостоявшийся setup: контроль `U-C1` уже доказал, что на строке той же длины сервер
/// отвечает.
#[tokio::test]
async fn pl_i_5_u1_multibyte_venue_does_not_kill_the_handler() {
    let dir = journal_of_trades(50);
    let addr = serve_on(dir.path()).await;
    let venue = multibyte_venue();
    assert!(
        venue.len() > 256 && !venue.is_char_boundary(256),
        "PL-I-5 U1 SETUP: фикстура обязана попадать НЕ на границу символа; len={} boundary={}",
        venue.len(),
        venue.is_char_boundary(256)
    );

    let got = first_message(&addr, &venue).await;
    let Some((n, v)) = got else {
        panic!(
            "PL-I-5 B-4: на многобайтовую площадку ({} Б, граница символа на 256 разорвана) \
             клиент не получил НИЧЕГО. Контроль U-C1 на ASCII той же длины ответ получает, \
             значит дело не в фикстуре: обработчик умер. Замер R-133: panic «byte index 256 \
             is not a char boundary», lib.rs:772. Код, добавленный ради защиты от перегрузки, \
             сам стал способом уронить соединение одной строкой",
            venue.len()
        )
    };
    assert_eq!(
        v.get("code").and_then(|c| c.as_str()),
        Some("unknown_venue"),
        "PL-I-5 U1: многобайтовая площадка обязана давать ТУ ЖЕ честную ошибку, что ASCII; \
         получено {n} Б: {v}"
    );
}
