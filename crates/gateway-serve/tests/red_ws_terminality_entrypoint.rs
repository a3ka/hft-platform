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

/// `code` конверта ошибки (`CT-RFC-09` §2.3/§2.10).
fn code_of(v: &Value) -> Option<&str> {
    v.get("code").and_then(|c| c.as_str())
}

/// `reason` конверта ошибки — обязателен при `code == "subscription_terminated"` (§2.10).
fn reason_of(v: &Value) -> Option<&str> {
    v.get("reason").and_then(|c| c.as_str())
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

    // ── (1b) КОД И ПРИЧИНА, а не только «ошибка» (`C-190` B-3) ──────────────────────
    // Прежняя редакция утверждала лишь `type == "error"` и `sub`, и потому НЕ МОГЛА решить
    // форму извещения: под неё подходил и `invalid_selector`, которым §2.7 отвечает на
    // НЕВЕРНЫЙ СЕЛЕКТОР. Клиент, получив «неверный селектор» на селектор, который сервер
    // сам только что принял снапшотом, обязан заключить, что ошибся он — это ложь о причине
    // и класс `TD-138`. Форма решена критиком и записана в `CT-RFC-09` §2.10.
    assert_eq!(
        code_of(&err),
        Some("subscription_terminated"),
        "TD-178 E-2 (1b) НАРУШЕН: извещение о ТЕРМИНАЛЬНОМ отказе несёт код {:?}, а обязано \
         нести родовой `subscription_terminated` (CT-RFC-09 §2.10). Тело: {err}. \
         `invalid_selector` здесь — ложь о причине: селектор верен, сервер принял его \
         снапшотом, а завершил подписку по СВОЕЙ причине",
        code_of(&err)
    );
    assert_eq!(
        reason_of(&err),
        Some("response_limit_exceeded"),
        "TD-178 E-2 (1b) НАРУШЕН: причина завершения {:?}, а обязана быть \
         `response_limit_exceeded` (CT-RFC-09 §2.10). Тело: {err}. Без машиночитаемой \
         причины клиент не отличит «сузь селектор» от «переподпишись как есть» и вынужден \
         разбирать текст — прямой запрет O-6",
        reason_of(&err)
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

// ═══════════════════════════════════════════════════════════════════════════════════════════
// TD-177 — ОТСТАВШИЙ PUMP НЕ СМЕЕТ УБИТЬ ПОДПИСКУ, СОЗДАННУЮ ПОЗЖЕ
//
// Задача 2 милестоуна. Оракул ИСПОЛНИМ и гоняется под `--features testing`.
//
// ═══ ЧТО ИСПРАВЛЕНО ПО `C-190` B-1 — И ПОЧЕМУ ЭТО БЫЛО ХУЖЕ ОБЫЧНОГО ПРОМАХА ═══
//
// Прежняя редакция ссылалась на `gateway_serve::test_seam` — шов на пять функций, который я
// объявил в спеке как «подлежащий внесению задачей 3», и держала себя в COMPILE-RED против
// него. Шов был ВЫДУМАН: детерминированная точка встречи УЖЕ СУЩЕСТВУЕТ в
// `crates/gateway-serve/src/test_sync.rs` (модуль `rendezvous`, `arm` / `pump_signal_and_wait`
// / `test_wait_for_pump` / `test_release` / `test_remove`), ПОДКЛЮЧЕНА к обоим прод-путям
// pump'а (`lib.rs:1228` для v1 и `:1693` для legacy) и уже используется оракулом `O-12`
// (`red_ws_session.rs:1325`).
//
// Обоснованием выдумки служил ЗАМЕР — и в этом суть ошибки, а не в невнимательности:
//   в спеке:   grep -c 'cfg(feature = "testing")' … → 0   «фича объявлена, но не используется»
//   на деле:   grep -c 'cfg(any(test, feature'      … → 4   ← настоящий вид ограждения
// Узкий шаблон дал ноль, ноль был предъявлен как доказательство отсутствия, и ВЕРНОЕ
// утверждение предыдущей редакции («крейт его уже использует») было заменено на ложное
// коммитом, озаглавленным «снято ложное утверждение о коде». То есть измерение послужило
// установке неправды. Класс — `reading-map` §2 «ПРАВИЛО ПРЕДШЕСТВЕННИКА»: искать РЕШЁННОЕ
// прежде, чем проектировать; здесь корпус проверялся памятью, а код — не тем грепом.
//
// ═══ ЧТО ИСПРАВЛЕНО ПО `C-190` B-2 ═══
//
// Прежняя редакция дописывала 20 событий ETH ДО переключения и НИ ОДНОГО после, а затем
// ждала кадр по новой подписке. Кадр рождается РОСТОМ ЖУРНАЛА ПОСЛЕ подписки; снапшот
// доказательством будущей прокачки не является. Значит корректная реализация могла
// не отдать ничего, и оракул объявил бы живую подписку мёртвой — ложное красное, которое
// потом «чинят» правкой реализации. Теперь свежая дописка идёт ПОСЛЕ switch+release, и
// кадр опознаётся ПО ЦЕНЕ.
//
// ПОЧЕМУ ПО ЦЕНЕ, А НЕ ПО СИМВОЛУ. `Frame` селектора не несёт (тот же факт стоил `O-12`
// вакуумного ассерта: он искал имя символа в теле кадра, вектор был пуст ВСЕГДА, и ассерт
// был истинен при любой реализации). Инструменты кормятся ценовыми диапазонами, разнесёнными
// на порядок — это делает признак устойчивым к округлению бакетов.
//
// ПОЧЕМУ ЗА `#[cfg(feature = "testing")]`. `test_sync` компилируется только при
// `cfg(any(test, feature = "testing"))`. Интеграционные тесты — отдельные крейты, линкующиеся
// с библиотекой без `cfg(test)`, поэтому без фичи модуля не существует. Условие висит на
// ФУНКЦИИ, а не на файле, чтобы `E-1`/`E-2` продолжали работать в обычном `cargo test --all`.
//
// ЧТО ПИННИТ. Сегодня `pump`, вернувшийся с ТЕРМИНАЛЬНЫМ отказом, сносит `subs`/`gens`
// БЕЗ сверки поколения (`crates/gateway-serve/src/lib.rs:1401-1406` и `:1806-1808`), тогда
// как СОХРАНЕНИЕ подписки поколение сверяет (`:1391`, `:1796`). Асимметрия и есть `TD-177`:
// пока старый `pump` в полёте, клиент успевает переподписать тот же id на другой селектор,
// `gens[id]` растёт, — и возврат отставшего `pump` убивает ЧУЖУЮ, живую подписку.
//
// ПОЧЕМУ БЕЗ ШВА ЭТОТ ОРАКУЛ БЫЛ БЫ ФЛАКОВЫМ. Окно между захватом `gen_at_pump` и возвратом
// `pump` — планировщик, а не наблюдаемое состояние. Оракул, ловящий его `sleep`'ом, красен
// через раз, и его выключат как шум (`testing.md`, целостность гейта, свойство 2).
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Цена BTC-серии. Разнесена с ETH на порядок: `Frame` не несёт селектора, и различить
/// инструменты можно ТОЛЬКО по значению.
#[cfg(feature = "testing")]
const BTC_PRICE: f64 = 65_000.0;
/// Цена ETH-серии — та же роль, другой порядок.
#[cfg(feature = "testing")]
const ETH_PRICE: f64 = 3_000.0;

/// Подписка на ДРУГОЙ символ: ответ по нему заведомо мал и предела не касается. Это условие
/// достижимости (б)-половины: если бы новая подписка тоже упиралась в предел, «она жива»
/// было бы неотличимо от «она мертва» — оба случая дали бы молчание.
#[cfg(feature = "testing")]
fn subscribe_sym(sub: &str, symbol: &str) -> Value {
    json!({
        "op": "subscribe",
        "v": 1,
        "id": sub,
        "selector": {
            "venue": "Binance",
            "symbol": symbol,
            "timeframe_ms": 1000,
            "bands": [PROD_BAND],
        }
    })
}

/// События по символу с ЯВНО ЗАДАННОЙ базовой ценой и явным началом шкалы времени.
/// Обе величины — параметры, потому что от них зависит опознание кадра: по цене различаются
/// инструменты, по времени — «до переключения» и «после».
#[cfg(feature = "testing")]
fn append_priced(dir: &Path, symbol: &str, n: usize, from_ms: i64, base_price: f64) {
    let mut j = Journal::open_with(dir, cfg()).expect("SETUP: open_with (ценовая дописка)");
    for i in 0..n as i64 {
        j.append(EventKind::md(
            Venue::Binance,
            symbol,
            MdPayload::Trade {
                price: to_fixed(base_price + i as f64 * 0.01),
                size: to_fixed(1.0),
                side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                ts_exch_ms: from_ms + i,
            },
        ))
        .expect("SETUP: append (ценовая дописка)");
    }
    j.flush().expect("SETUP: flush (ценовая дописка)");
}

/// Максимальная цена в кадре. `None` — кадр без ценовых серий (не годится для опознания).
/// Форма взята из `O-12` (`red_ws_session.rs:298`): и снапшот, и дельта несут `ohlcv`.
#[cfg(feature = "testing")]
fn max_price_in(v: &Value) -> Option<i64> {
    let d = v.get("data")?;
    let mut best: Option<i64> = None;
    for row in d
        .pointer("/delta/ohlcv")
        .or_else(|| d.pointer("/series/ohlcv"))?
        .as_array()?
    {
        if let Some(h) = row.get("high").and_then(|x| x.as_i64()) {
            best = Some(best.map_or(h, |b: i64| b.max(h)));
        }
    }
    best
}

#[cfg(feature = "testing")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn td177_stale_pump_does_not_kill_new_sub() {
    use gateway_serve::test_sync::rendezvous;

    let _g = serial().await;
    let _cap = CapGuard;

    // Предел ставится ДО старта сессии: отказ обязан случиться на ПЕРВОМ же тике по BTC.
    gateway::set_effective_max_response_bytes(TEST_CAP);
    let dir = journal_of_trades(200);
    append_priced(dir.path(), "ETHUSDT", 20, T0 + 1_000_000, ETH_PRICE);

    // Точка встречи ключуется ID ПОДПИСКИ (`lib.rs:1214` — `id_for_pump = id.clone()`), и id
    // при переподписке ТОТ ЖЕ. Это не помеха, а свойство: `test_release` не сбрасывает
    // `release`, поэтому удержан будет только ПЕРВЫЙ pump, а pump новой подписки пройдёт
    // насквозь. Вооружаем ДО первой подписки — иначе pump пролетит окно, и тест снова гонка.
    rendezvous::arm(SHORT_SUB);

    let addr = serve_on(dir.path()).await;
    let mut ws = subscribed(&addr).await;

    // Плотная дописка по BTC — следующий `pump` этой подписки уйдёт за предел.
    append_dense(dir.path());

    // ── ОКНО ОТКРЫТО: pump вошёл в точку встречи, `gen_at_pump` захвачен, возврат удержан ──
    if !rendezvous::test_wait_for_pump(SHORT_SUB, BUDGET) {
        rendezvous::test_release(SHORT_SUB);
        rendezvous::test_remove(SHORT_SUB);
        setup_failed(
            "pump не вошёл в точку встречи за отведённый бюджет — окно гонки не открыто, \
             и всё, что ниже, судило бы установившийся режим вместо предмета",
        );
    }

    // ── ПОКА ОКНО ОТКРЫТО: переподписываем ТОТ ЖЕ id на другой селектор (SWITCH) ────────
    send(&mut ws, subscribe_sym(SHORT_SUB, "ETHUSDT")).await;
    let mut switched = false;
    for _ in 0..MAX_DRAIN {
        match recv(&mut ws).await {
            Some((_, v)) if type_of(&v) == Some("snapshot") && sub_of(&v) == Some(SHORT_SUB) => {
                switched = true;
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    if !switched {
        rendezvous::test_release(SHORT_SUB);
        rendezvous::test_remove(SHORT_SUB);
        setup_failed(
            "новая подписка на ETHUSDT не подтверждена снапшотом, пока старый `pump` удержан — \
             SWITCH не состоялся, и «новая подписка жива» ниже было бы утверждением ни о чём",
        );
    }

    // ── ОКНО ЗАКРЫВАЕТСЯ: отставший `pump` возвращается с ТЕРМИНАЛЬНЫМ отказом ─────────
    rendezvous::test_release(SHORT_SUB);

    // ── СВЕЖЕЕ СОБЫТИЕ ПОСЛЕ ПЕРЕКЛЮЧЕНИЯ (`C-190` B-2) ────────────────────────────────
    // Кадр рождается ростом журнала ПОСЛЕ подписки. Без этой дописки «кадров нет» означало бы
    // «журнал не рос», а не «подписка мертва», и оракул объявлял бы мёртвой живую подписку.
    append_priced(dir.path(), "ETHUSDT", 8, T0 + 2_000_000, ETH_PRICE);

    // ── (а) ошибка по этому id клиенту НЕ уходит ────────────────────────────────────────
    // Отказ относится к СНЯТОМУ селектору, которого клиент уже не ждёт. Извещение о нём
    // заставит клиента пересобрать подписку, которая жива и здорова.
    // ── (б) новая подписка ЖИВА — и кадр опознаётся ПО ЦЕНЕ, а не по факту прихода ─────
    let mut fresh_eth_frame = false;
    let mut priced_seen = 0usize;
    let mut stale_btc_frame = false;
    for _ in 0..MAX_DRAIN {
        match recv(&mut ws).await {
            Some((_, v)) if type_of(&v) == Some("error") && sub_of(&v) == Some(SHORT_SUB) => {
                rendezvous::test_remove(SHORT_SUB);
                panic!(
                    "TD-177 (а) НАРУШЕН: отставший `pump` СНЯТОГО селектора отказал по пределу, \
                     и ошибка ушла клиенту по подписке «{SHORT_SUB}», которая с тех пор \
                     переподписана на другой селектор и в предел укладывается. Тело: {v}. \
                     Снятие подписки не сверяет поколение (lib.rs:1401-1406 и :1806-1808), \
                     тогда как СОХРАНЕНИЕ сверяет (:1391, :1796) — это и есть асимметрия TD-177"
                )
            }
            Some((_, v)) if type_of(&v) == Some("frame") && sub_of(&v) == Some(SHORT_SUB) => {
                match max_price_in(&v) {
                    Some(p) if p >= to_fixed(BTC_PRICE) => stale_btc_frame = true,
                    Some(_) => {
                        priced_seen += 1;
                        fresh_eth_frame = true;
                        break;
                    }
                    None => continue,
                }
            }
            Some(_) => continue,
            None => break,
        }
    }
    rendezvous::test_remove(SHORT_SUB);

    // SETUP-GUARD различения (`testing.md`, целостность гейта, свойство 3): признак построен
    // на ЦЕНЕ, значит сценарий обязан убедиться, что ценовые серии до клиента вообще доехали.
    // Кадр по СТАРОМУ инструменту после переключения — сам по себе дефект, и он не считается
    // доказательством живости новой подписки.
    assert!(
        !stale_btc_frame,
        "TD-177 НАРУШЕН иначе: после переключения на ETHUSDT клиенту пришёл кадр с ценой \
         BTC-диапазона (≥ {BTC_PRICE}). Отставший `pump` СНЯТОГО селектора не только выжил, \
         но и доставил свою серию под чужим id — клиент получит данные инструмента, на \
         который не подписан"
    );
    assert!(
        fresh_eth_frame && priced_seen > 0,
        "TD-177 (б) НАРУШЕН: ошибка клиенту не пришла, но и ни одного кадра ETH-диапазона по \
         «{SHORT_SUB}» нет за {MAX_DRAIN} сообщений — при том, что свежие события ETH дописаны \
         ПОСЛЕ переключения и релиза. Отставший `pump` снёс `subs`/`gens` без сверки поколения: \
         новая подписка удалена вместе со старой, и клиент остался на молчащем канале, ничего \
         об этом не зная. Это TD-177 в его втором проявлении: не ложное извещение, а тихая \
         потеря живой подписки"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// E-3 — ОТКАЗ ЧТЕНИЯ ПОСРЕДИ ДОГОНА ЗАВЕРШАЕТ ПОДПИСКУ С ОТДЕЛЬНОЙ ПРИЧИНОЙ
//
// Задача 5 милестоуна, оракул на ПРОВОДЕ. Внесён по `C-190` B-3: форму извещения спека
// делегировала кругу критика, критик её выбрал (`CT-RFC-09` §2.10), но ни один закоммиченный
// оракул её не наблюдал. Оракул `TD-179` зовёт `LiveReducer` НАПРЯМУЮ и ошибку на проводе
// увидеть не может по построению; `E-2` до этого круга судил лишь `type == "error"`.
//
// ЧЕМ ЭТОТ СЦЕНАРИЙ ОТЛИЧАЕТСЯ ОТ `E-2`, И ПОЧЕМУ ОБА НУЖНЫ. `E-2` — отказ ПО ПРЕДЕЛУ:
// он предсказуем, повторяем и лечится сужением селектора. Здесь — отказ ЧТЕНИЯ ЖУРНАЛА:
// селектор ни при чём, сужать нечего, клиенту следует переподписаться как есть. Один код
// на оба случая заставил бы клиента лечить не то; ровно поэтому §2.10 требует `reason`.
//
// КАК ИНЖЕКТИРУЕТСЯ ОТКАЗ. Один перевёрнутый байт в теле кадра ломает CRC ИМЕННО ЭТОГО
// кадра: предшествующие декодируются честно, и отказ приходит из СЕРЕДИНЫ прохода. Приём
// взят у `red_pump_midstream_failure.rs:205-210` вместе с его оплаченным уроком: порча
// ЗАГОЛОВКА следующего сегмента давала ЗЕЛЁНЫЙ ВАКУУМ — стрим падал на перечислении
// сегментов, до первого события, и тест судил не тот сценарий.
//
// ПОРЧА СТАВИТСЯ ПОСЛЕ ПОДПИСКИ. Сессия при `subscribe` уже прошла журнал, чтобы собрать
// снапшот; порча, внесённая раньше, сорвала бы САМ снапшот — другой сценарий, и подписки
// в нём не возникает вовсе.
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Сегменты каталога по возрастанию имени.
fn segments_of(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .expect("SETUP: read_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let s = p.to_string_lossy().to_string();
            s.ends_with(".jrnl") || s.ends_with(".jrnl.zst")
        })
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn e3_non_cap_midstream_failure_terminates_with_pump_failed_reason() {
    let _g = serial().await;
    let _cap = CapGuard;

    // Предел СНЯТ намеренно: предмет — отказ, НЕ связанный с пределом. Если бы предел стоял,
    // причина оказалась бы неотличима от `E-2`, и оракул судил бы чужой сценарий.
    gateway::set_effective_max_response_bytes(usize::MAX);

    let dir = journal_of_trades(200);
    let addr = serve_on(dir.path()).await;
    let mut ws = subscribed(&addr).await;

    // Рост журнала — иначе следующему догону нечего читать и отказ недостижим.
    append_dense(dir.path());

    // ── ИНЪЕКЦИЯ: ломаем CRC кадра в теле ПОСЛЕДНЕГО сегмента ──────────────────────────
    let segs = segments_of(dir.path());
    let victim = match segs.last() {
        Some(p) => p.clone(),
        None => setup_failed("фикстура не дала ни одного сегмента — портить нечего"),
    };
    let mut bytes = std::fs::read(&victim)
        .unwrap_or_else(|e| setup_failed(&format!("сегмент-жертва не прочитан: {e}")));
    if bytes.len() < 64 {
        setup_failed(&format!(
            "сегмент-жертва {} Б — слишком мал, порча неотличима от пустоты",
            bytes.len()
        ));
    }
    let at = bytes.len() * 3 / 5;
    bytes[at] ^= 0xFF;
    std::fs::write(&victim, &bytes)
        .unwrap_or_else(|e| setup_failed(&format!("сегмент не испорчен: {e}")));

    // ── SETUP-GUARD НЕЗАВИСИМЫМ ПУТЁМ ─────────────────────────────────────────────────
    // Эталон берётся НЕ из проверяемого пути (`testing.md`: зависимый эталон мутация ловит
    // плохо). Своим проходом убеждаемся, что журнал ДЕЙСТВИТЕЛЬНО отказывает и отказывает
    // ПОСЛЕ выдачи событий: отказ на первом же событии означал бы «журнал не читается
    // вовсе», а это не отказ ПОСРЕДИ догона.
    let mut yielded = 0usize;
    let mut failed = false;
    match journal::stream(dir.path(), EpochFilter::OwnCaptureOnly) {
        Ok(st) => {
            for ev in st {
                match ev {
                    Ok(_) => yielded += 1,
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
        }
        Err(e) => setup_failed(&format!("независимый проход не открылся вовсе: {e}")),
    }
    if !failed || yielded == 0 {
        setup_failed(&format!(
            "инъекция не дала отказа ПОСРЕДИ прохода (отдано {yielded} событий, отказ={failed}). \
             Без этого сценарий вырождается: либо журнал цел и отказывать нечему, либо он не \
             читается с первого события — а предмет здесь третий"
        ));
    }

    // ── ПРЕДМЕТ: клиент извещён ПРАВДИВЫМ кодом и ОТДЕЛЬНОЙ причиной ──────────────────
    let mut notified: Option<Value> = None;
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..MAX_DRAIN {
        match recv(&mut ws).await {
            Some((n, v)) => {
                seen.push(format!("{:?}/{:?}/{n}Б", type_of(&v), sub_of(&v)));
                if type_of(&v) == Some("error") && sub_of(&v) == Some(SHORT_SUB) {
                    notified = Some(v);
                    break;
                }
            }
            None => break,
        }
    }
    let err = notified.unwrap_or_else(|| {
        panic!(
            "E-3 (1) НАРУШЕН: чтение журнала отказало посреди догона, а клиент НЕ ИЗВЕЩЁН — \
             за {MAX_DRAIN} сообщений ни одного `error` по «{SHORT_SUB}». Виденное: {seen:?}. \
             Клиент остаётся на молчащем канале и считает, что рынок замер, тогда как \
             оборвалась ЕГО серия (PL-I-7: деградация не выдаётся за норму)"
        )
    });
    assert_eq!(
        code_of(&err),
        Some("subscription_terminated"),
        "E-3 (2) НАРУШЕН: код {:?} вместо родового `subscription_terminated` \
         (CT-RFC-09 §2.10). Тело: {err}. `invalid_selector` здесь — прямая ложь: селектор \
         верен, сервер принял его снапшотом, а сломался журнал",
        code_of(&err)
    );
    assert_eq!(
        reason_of(&err),
        Some("pump_failed"),
        "E-3 (3) НАРУШЕН: причина {:?} вместо `pump_failed` (CT-RFC-09 §2.10). Тело: {err}. \
         Причина обязана ОТЛИЧАТЬСЯ от `response_limit_exceeded`: там клиент сужает \
         селектор, здесь сужать нечего — лечится переподпиской как есть. Один код на оба \
         случая заставит клиента лечить не то",
        reason_of(&err)
    );

    // ── (4) после извещения по ЭТОЙ подписке НИ ОДНОГО кадра ─────────────────────────
    // Молчание не вакуумно: `E-1` уже предъявил, что путь живой и кадры по нему ходят.
    for _ in 0..MAX_DRAIN {
        match recv(&mut ws).await {
            Some((_, v)) if type_of(&v) == Some("frame") && sub_of(&v) == Some(SHORT_SUB) => {
                panic!(
                    "E-3 (4) НАРУШЕН: после извещения о завершении подписки «{SHORT_SUB}» \
                     ПРИШЁЛ кадр {v}. Подписка не снята — значит сессия продолжает биться в \
                     тот же нечитаемый участок журнала, и извещение было ложным"
                )
            }
            Some(_) => continue,
            None => break,
        }
    }
}
