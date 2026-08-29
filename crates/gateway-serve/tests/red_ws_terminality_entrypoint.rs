//! RED `TD-178` / `GW-I-14` / `VB-I-2` (sacred, architect-only) — **ОРАКУЛ ТОЧКИ ВХОДА
//! PUSH-ПУТИ: терминальный отказ ЗАВЕРШАЕТ подписку И ИЗВЕЩАЕТ КЛИЕНТА.**
//!
//! Милестоун `milestones/M-72-subscription-terminality.md`, задача 1. Источник —
//! `TECH-DEBT.md` `TD-178` (заведено reviewer'ом по `R-146` `N-2`, названо также tester'ом).
//!
//! # Что именно не было покрыто
//!
//! `gates.md` §4, DoD «Механизм на пути»: milestone, вводящий механизм несущего пути,
//! мержится ТОЛЬКО с подключением, ДОКАЗАННЫМ ОРАКУЛОМ ТОЧКИ ВХОДА. Терминальность
//! подписки (`M-71`) — такой механизм. Замер `TD-178`:
//!
//! | оракул | что исполняет |
//! |---|---|
//! | `P6` (`crates/gateway/tests/red_egress_cap_paths.rs`) | БИБЛИОТЕКУ `LiveReducer` |
//! | `W5` (`crates/gateway-serve/tests/red_egress_cap_wire.rs`) | ОБЁРТКИ `serve::snapshot_msg`/`frames_msgs` |
//! | — | **push-цикл `v1` с отказывающим `pump` не исполняет НИ ОДИН** |
//!
//! То есть механизм подключён, а подключение не доказано: дефект `TD-177` живёт ровно в
//! этом неохваченном шве и найден ЧТЕНИЕМ, а не гейтом.
//!
//! **Недостижимым это не является, и объявлять его таковым запрещено.** Харнесс живой
//! WS-сессии в репозитории есть (`red_ws_session.rs`, `red_ws_protocol.rs`,
//! `red_ws_series_vs_replay.rs`, `smoke_ws.rs`), плотная фикстура предела —
//! в `red_egress_cap_wire.rs`. Цена ошибки «объявить недостижимым» уже оплачена: на `M-68`
//! автор снял требование гейта таким утверждением, и ревьюер написал рабочую пробу за один
//! заход (`R-145` `Б-2`).
//!
//! # Мера — на границе ПОТРЕБИТЕЛЯ (Р-1 разбора класса)
//!
//! Судится ТО, ЧТО ВИДИТ КЛИЕНТ В СОКЕТЕ: пришло ли извещение и приходят ли после него
//! кадры по той же подписке. Не `subs.len()`, не `is_cap_terminal()`, не внутренние карты —
//! они менялись бы вместе с реализацией, а провод не меняется. `docs/workflow/
//! oracle-blindness-class-2026-08-28.md`, правило Р-1.
//!
//! # Vantage СНАЧАЛА, предмет ПОТОМ (конструкция `W3`)
//!
//! `E-1` доказывает, что push-путь ЖИВОЙ и кадры по нему доходят. Без него `E-2` был бы
//! зелен и против мёртвого сервера, и против сломанного grace-окна: «кадров после ошибки
//! нет» верно и тогда, когда их не было никогда. Молчание как исход допустимо ТОЛЬКО после
//! того, как соседний оракул предъявил, что путь не мёртв.

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
const SECRET: &[u8] = b"m72-terminality-secret";
const PROD_BAND: f64 = 0.001;
const SHORT_SUB: &str = "s1";

/// Плотная нагрузка, гарантированно выводящая delta-кадр за подписанный предел
/// (`П-020`, 2 000 000 Б). Та же величина и по той же причине, что в `red_egress_cap_wire.rs`:
/// фикстура разведена с отсечкой на порядок, чтобы набор не зависел от точного числа.
const DENSE_TRADES: usize = 25_000;

/// Щедрый бюджет ожидания. Истечение = НЕСОСТОЯВШИЙСЯ SETUP, не вердикт о терминальности.
const BUDGET: Duration = Duration::from_secs(20);

/// **ПРЕДЕЛ ДЛЯ E-2 — ЗАМЕР, А НЕ ВЫДУМКА.** Прогон фикстуры дал: снапшот при подписке
/// **425 Б**, delta-кадр push-цикла **~29 КБ**, предел по умолчанию — 2 000 000 Б. То есть
/// на дефолтном пределе предмет НЕДОСТИЖИМ: 25 000 сделок уходят к клиенту десятками мелких
/// кадров и отказа не вызывают никогда. Первая редакция этого файла ровно так и провалилась —
/// `E-2` сообщал «клиент не извещён», хотя истинная причина была «предел не сработал», то
/// есть оракул выдал бы ЛОЖНУЮ находку. Значение 10 000 Б разведено с обеими замеренными
/// величинами: снапшот проходит, кадр отвергается.
const TEST_CAP: usize = 10_000;

/// Предел — ПРОЦЕССНАЯ переменная (`gateway::set_effective_max_response_bytes`), и оба теста
/// этого файла его ТРОГАЮТ разными значениями. Значит они обязаны идти последовательно —
/// иначе меряют друг друга. Тот же приём и та же причина, что `serial()` в
/// `crates/gateway/tests/red_egress_cap_paths.rs:127`; здесь мьютекс асинхронный, потому что
/// удержание `std`-мьютекса через `.await` — самостоятельный дефект (`clippy::await_holding_lock`).
/// **ВОЗВРАТ ПРЕДЕЛА — ЧЕРЕЗ `Drop`, А НЕ ПОСЛЕДНЕЙ СТРОКОЙ ТЕСТА.** Первая редакция ставила
/// восстановление дефолта в конец `E-2`: упавший ассерт до него не доезжает, и следующий тест
/// этого бинаря унаследовал бы чужое значение процессного предела. Сегодня безвредно (тестов
/// два, и `E-1` ставит предел сам), но третий добавленный поймал бы флак от глобального
/// состояния — ровно тот класс, который набор `red_egress_cap_paths.rs` уже ловил замером
/// («параллельный прогон роняет ЕЩЁ и `P1`»). Страж восстанавливает дефолт при ЛЮБОМ выходе,
/// включая панику ассерта.
struct CapGuard;

impl Drop for CapGuard {
    fn drop(&mut self) {
        gateway::set_effective_max_response_bytes(gateway::DEFAULT_MAX_RESPONSE_BYTES);
    }
}

async fn serial() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

/// Сколько сообщений готовы прочитать, разыскивая предмет. Ограничение обязательно: без него
/// оракул при сломанной терминальности висел бы до таймаута CI вместо того, чтобы назвать
/// дефект.
const MAX_DRAIN: usize = 64;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn sign() -> String {
    let claims = Claims {
        sub: "m72".to_string(),
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
        provenance: "TD-178 entrypoint fixture".to_string(),
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

fn append_dense(dir: &Path) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in 1_000..(1_000 + DENSE_TRADES as i64) {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
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

fn subscribe(sub: &str) -> Value {
    json!({
        "op": "subscribe",
        "v": 1,
        "id": sub,
        "selector": {
            "venue": "Binance",
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

async fn recv(ws: &mut Ws) -> Option<(usize, Value)> {
    match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => {
            let d = m.into_data();
            let n = d.len();
            serde_json::from_slice::<Value>(d.as_ref())
                .ok()
                .map(|v| (n, v))
        }
        _ => None,
    }
}

fn type_of(v: &Value) -> Option<&str> {
    v.get("type").and_then(|t| t.as_str())
}

fn sub_of(v: &Value) -> Option<&str> {
    v.get("sub").and_then(|t| t.as_str())
}

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о терминальности: сервер не довёл \
         сценарий до точки, в которой предмет наблюдаем. Молчать об этом нельзя — оракул, \
         зелёный от несостоявшегося setup'а, есть плацебо самого себя."
    )
}

/// Подписка, ГАРАНТИРОВАННО попавшая в grace-окно, и первый v1-снапшот по ней. Повтор
/// обязателен: `subscribe`, опоздавший в grace-окно, сервер игнорирует и уходит в env-режим
/// (`CT-RFC-09` §2.8) — без повтора оракул не получал бы v1-сообщений и был бы ЗЕЛЁН ПО
/// ОТСУТСТВИЮ ПРЕДМЕТА (`A-022`). Тот же приём — `red_egress_cap_wire.rs:225`.
async fn subscribed(addr: &str) -> Ws {
    let mut last: Option<Value> = None;
    for attempt in 1..=8u64 {
        let mut ws = connect(addr).await;
        send(&mut ws, subscribe(SHORT_SUB)).await;
        match recv(&mut ws).await {
            Some((n, v)) if type_of(&v) == Some("snapshot") => {
                let _ = n;
                return ws;
            }
            Some((_, v)) => {
                last = Some(v);
                let _ = ws.close(None).await;
            }
            None => {
                let _ = ws.close(None).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(10 * attempt)).await;
    }
    setup_failed(&format!(
        "v1-снапшот не получен за 8 попыток попасть в grace-окно; последнее сообщение: {last:?}"
    ))
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// E-1 — VANTAGE, первым: push-путь ЖИВОЙ, кадры по подписке ДОХОДЯТ ДО СОКЕТА
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Без этой половины `E-2` зелен и против мёртвого сервера: «кадров после ошибки нет» верно
/// и тогда, когда кадров не было никогда. Она же краснеет против сломанного grace-окна и
/// против фикстуры, не попадающей в v1-режим.
#[tokio::test]
async fn td_178_e1_push_path_delivers_frames_to_the_socket() {
    let _g = serial().await;
    let _cap = CapGuard;
    gateway::set_effective_max_response_bytes(usize::MAX);
    let dir = journal_of_trades(200);
    let addr = serve_on(dir.path()).await;
    let mut ws = subscribed(&addr).await;

    // Небольшая дописка — заведомо ПОД пределом: предмет здесь «кадр доходит», а не «предел».
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 500..560 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    for _ in 0..MAX_DRAIN {
        match recv(&mut ws).await {
            Some((n, v)) if type_of(&v) == Some("frame") && sub_of(&v) == Some(SHORT_SUB) => {
                let _ = n;
                return;
            }
            Some(_) => continue,
            None => break,
        }
    }
    setup_failed(
        "по живой v1-подписке не пришло ни одного кадра под пределом — push-путь не доказан \
         живым, и вердикт E-2 о его терминальности был бы вакуумным",
    )
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// E-2 — ПРЕДМЕТ: терминальный отказ ИЗВЕЩАЕТ клиента и ЗАВЕРШАЕТ подписку
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **Инвариант, судимый на проводе.** Когда `pump` отвергнут пределом ОБЪЁМА:
///
/// 1. клиенту приходит сообщение об ошибке, НАЗЫВАЮЩЕЕ его подписку — молчание запрещено
///    (`PL-I-7`: деградация никогда не выдаётся за норму; `CT-RFC-09`: всякий отказ выражен
///    машиночитаемым `code` и никогда не выглядит как молчание);
/// 2. после извещения кадры по ЭТОЙ подписке больше не приходят — подписка ЗАВЕРШЕНА, и
///    клиент обязан пересобраться снапшотом (`DESIGN` §16, `M-71` §4bis.5: повторная попытка
///    того же кадра ничего не лечит, он не станет меньше, пока предел стоит — это livelock).
///
/// **Форму извещения оракул НЕ ВЫБИРАЕТ** (тот же принцип, что в шапке `red_egress_cap_wire.rs`):
/// какой именно `code` и текст — решение задачи 5 спеки. Требование одно: сообщение об ошибке
/// пришло, назвало подписку, и после него по ней тихо.
#[tokio::test]
async fn td_178_e2_cap_terminal_refusal_notifies_client_and_ends_subscription() {
    let _g = serial().await;
    let _cap = CapGuard;
    // Снапшот (425 Б) проходит, delta-кадр (~29 КБ) — нет. Предел ставится ДО `serve_on`,
    // чтобы сессия читала уже сниженное значение с первого тика.
    gateway::set_effective_max_response_bytes(TEST_CAP);
    let dir = journal_of_trades(200);
    let addr = serve_on(dir.path()).await;
    let mut ws = subscribed(&addr).await;

    append_dense(dir.path());

    // ── (1) извещение обязано прийти ────────────────────────────────────────────────
    let mut notified: Option<Value> = None;
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..MAX_DRAIN {
        match recv(&mut ws).await {
            Some((n, v)) => {
                seen.push(format!("{:?}/{:?}/{n}Б", type_of(&v), sub_of(&v)));
                if type_of(&v) == Some("error") {
                    notified = Some(v);
                    break;
                }
            }
            None => break,
        }
    }
    let err = notified.unwrap_or_else(|| {
        panic!(
            "TD-178 E-2 (1) НАРУШЕН: delta-кадр вышел за подписанный предел, а клиент НЕ \
             ИЗВЕЩЁН — за {MAX_DRAIN} сообщений ни одного `error`. Виденное: {seen:?}. \
             Терминальность подписки существует в библиотеке и НЕ подключена к точке входа: \
             клиент остаётся на молчащем канале, не зная, что серия оборвалась."
        )
    });
    assert_eq!(
        sub_of(&err),
        Some(SHORT_SUB),
        "TD-178 E-2 (1): извещение пришло, но НЕ НАЗВАЛО подписку (sub={:?}, тело={err}). \
         Клиент с несколькими подписками не может понять, какая из них завершена, и \
         пересоберёт не ту — либо не пересоберёт вовсе",
        sub_of(&err)
    );

    // ── (2) после извещения по ЭТОЙ подписке тихо ───────────────────────────────────
    // Молчание здесь НЕ вакуумно: `E-1` уже предъявил, что путь живой и кадры по нему ходят.
    for _ in 0..MAX_DRAIN {
        match recv(&mut ws).await {
            Some((_, v)) if type_of(&v) == Some("frame") && sub_of(&v) == Some(SHORT_SUB) => {
                panic!(
                    "TD-178 E-2 (2) НАРУШЕН: после извещения о терминальном отказе по \
                     подписке «{SHORT_SUB}» ПРИШЁЛ кадр {v}. Подписка не завершена — значит \
                     сессия продолжает качать тот же сверхлимитный кадр, и это livelock, \
                     который M-71 §4bis.5 закрывал (R-143 B-2: восемь тиков, кадров ноль)"
                )
            }
            Some(_) => continue,
            None => break,
        }
    }
}
