//! RED M-87 (sacred, architect-only) — ОРАКУЛ ТОЧКИ ВХОДА: предохранитель стоит на
//! РЕАЛЬНОМ публичном пути, а не рядом с ним.
//!
//! ## Почему этот файл существует (`C-234` R2)
//!
//! Первая редакция набора звала `gateway::LiveReducer::resume` НАПРЯМУЮ и называла это
//! «живым путём». Настоящий публичный путь другой: `handle_v1_message` после
//! `session::validate_selector` (`crates/gateway-serve/src/lib.rs:810-814`) НЕМЕДЛЕННО уходит
//! в `spawn_blocking` с `LiveReducer::resume` — на ветке SWITCH (`:842-849`) и на ветке ADD
//! (`:938-944`). Оракул, обходящий эту точку, не доказывает, что защита к ней подключена:
//! dev мог бы не подключить допуск вовсе — и набор остался бы зелёным.
//!
//! Следствие второе, названное гейтом и принятое: GREEN можно получить и НЕВЕРНО — гвардом
//! внутри общего `gateway::validate_selector`/`LiveReducer`. Это задело бы offline-путь,
//! чекпоинтер и реплей, то есть воспроизвело конструкцию `M-84`, признанную негодной
//! арбитражем `A-033` (209 красных из 329, поломка прод-формы argv). Поэтому запрет на такую
//! правку — в запретном списке спеки §10, а здесь судится ИМЕННО транспорт.
//!
//! ## Чем доказывается «журнал не читался» на точке входа
//!
//! Счётчиком ПРОЧИТАННЫХ БАЙТ, наблюдаемым СНАРУЖИ (`ServingCounters::journal_payload_bytes_read`).
//! Счётчик декодированных событий недостаточен по плану §15.1: сегмент можно открыть,
//! прочитать и распаковать, не вызвав декодер. Счётчик снимается ДО и ПОСЛЕ запроса —
//! то есть судится ПРОДЮСЕР на реальном пути, как требует `OPS-I-10`
//! (`docs/fa/ops.md:478`: «объявлена ⟹ эмитится»).
//!
//! COMPILE-RED: модулей `gateway_serve::admission` / `gateway_serve::metrics` не существует.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::auth::Claims;
use gateway_serve::metrics::{freshness, serving_counters};
use gateway_serve::server::{bind_with_policy, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m87-entrypoint-secret";
const BASE_MS: i64 = 1_784_116_800_000;
const BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
/// Канонические семь полос (`П-029`).
const CANONICAL: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

fn sign() -> String {
    encode(
        &Header::default(),
        &Claims {
            sub: "m87".to_string(),
            exp: 9_999_999_999,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("encode")
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 64 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m87".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// Журнал прод-подобной длины: пересчёт по нему ДОЛЖЕН быть заметно дороже отказа.
fn journal_busy(events: i64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![lvl(65_000.0, 2.0)],
                asks: vec![lvl(65_010.0, 1.5)],
                ts_exch_ms: BASE_MS,
            },
        ))
        .expect("snap");
        for i in 0..events {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + (i % 20) as f64),
                    size: to_fixed(0.5),
                    side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    ts_exch_ms: BASE_MS + i * 100,
                },
            ))
            .expect("trade");
        }
        j.flush().expect("flush");
    }
    dir
}

fn canonical_sel() -> Selector {
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
        // Круг `C-245`/`C-246`: шестипольная форма §4.1. Пороги ЗАВЕДОМО ЩЕДРЫЕ —
        // предмет этого файла не свежесть слепка, и строгий порог определял бы его
        // исход посторонней величиной (`testing.md`, целостность гейта п.2).
        // ПОРОГ ОБЩИЙ НА ФАЙЛ, И ЕГО ВЫБОР ОБЯЗАН БЫТЬ ДОКАЗАН ПО ВСЕМ ПОТРЕБИТЕЛЯМ
        // (`C-248` R4-2). Прошлая редакция утверждала «прочие сценарии снимают слепок в
        // конце, отставание ≈0» — утверждение ЛОЖНО: `u1_entry_stale_checkpoint_beyond_budget`
        // дописывает 4 000 событий ПОСЛЕ слепка и ждёт отказа ИМЕННО по свежести.
        //
        // Действительная таблица отставаний (снята чтением фикстур, а не памятью):
        //   c9_four_positions…              +200   ⇒ обязан ОБСЛУЖИТЬ (отставание — норма)
        //   u1_served_request_with_tail     +50    ⇒ обязан ОБСЛУЖИТЬ
        //   u1_entry_stale_checkpoint…      +4 000 ⇒ обязан ОТКАЗАТЬ по свежести
        //   прочие (c1/c4/c5/c6/u1_warm)    0      ⇒ порогом не задеты
        // Отсюда допустимый порог: 200 ≤ max_tail_events < 4 000. Выбрано 1 000 — середина
        // коридора, не край: значение на краю ломалось бы от любой правки фикстуры.
        max_tail_events: 1_000,
        // ОПОРА СНИЖЕНА С 500 ДО 100 РАДИ ДОКАЗУЕМОСТИ ГРАНИЦЫ (`C-248` R4-1).
        // Граница `C9` доказывается мутацией порога НИЖЕ отставания 200. При опоре 500
        // такая мутация делает политику НЕВАЛИДНОЙ (`max_tail_events < expected_warmup_events`),
        // и сценарий упал бы на отказе СТАРТА, а не на свежести — то есть доказывал бы не то.
        // При опоре 100 мутация `1_000 → 150` оставляет политику валидной (150 ≥ 100) и
        // пересекает отставание 200: `C9` обязан покраснеть ИМЕННО по свежести.
        expected_warmup_events: 100,
    }
}

fn config(dir: &std::path::Path, ckpt: Option<std::path::PathBuf>) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: canonical_sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: ckpt,
    }
}

/// **ЛОВУШКА (`A-037` §3.4, план §15.1).** Журнал, у которого полезная нагрузка сегмента
/// испорчена при ЦЕЛОМ заголовке: прод-путь чтения строг к CRC (`journal::stream`), поэтому
/// любое чтение содержимого даёт `Err`, а опись каталога и заголовки читаются.
///
/// Это и есть «подставной читатель, немедленно проваливающий проверку при попытке читать
/// полезную нагрузку». Счётчик декодированных событий недостаточен (план §15.1): сегмент
/// можно открыть и распаковать, не вызвав декодер. Ловушка ловит именно ЧТЕНИЕ.
fn poison_segment(dir: &std::path::Path, index: usize) {
    let mut segs: Vec<_> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jrnl"))
        .collect();
    segs.sort();
    let target = segs.get(index).unwrap_or_else(|| {
        panic!(
            "setup-страж: сегмента {index} нет, сегментов {}",
            segs.len()
        )
    });
    let mut bytes = std::fs::read(target).expect("read segment");
    assert!(
        bytes.len() > 512,
        "setup-страж: сегмент короче 512 Б — порча попала бы в заголовок, и ловушка ловила бы \
         не то"
    );
    let mid = bytes.len() / 2;
    for b in bytes.iter_mut().skip(mid).take(64) {
        *b = !*b;
    }
    std::fs::write(target, &bytes).expect("write poisoned segment");
}

async fn serve(dir: &std::path::Path, ckpt: Option<std::path::PathBuf>) -> String {
    serve_with(dir, ckpt, policy()).await
}

/// Политика передаётся серверу ЯВНО (`A-037` У-3). Билдер выбран вместо поля `ServeConfig`
/// намеренно: он АДДИТИВЕН — одиннадцать существующих sacred-файлов с литералом
/// `ServeConfig { … }` не ломаются, и правило `A-037` D-1 («библиотека и корпус меняются
/// только аддитивно») соблюдается и здесь.
async fn serve_with(
    dir: &std::path::Path,
    ckpt: Option<std::path::PathBuf>,
    pol: AdmissionPolicy,
) -> String {
    let server = bind_with_policy(config(dir, ckpt), pol)
        .await
        .expect("bind_with_policy");
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    addr
}

/// Поднять сервер и вернуть ВМЕСТЕ с адресом ручку слотов ЭТОГО экземпляра.
///
/// Зачем отдельный хелпер: `serve_with` отдаёт сервер в `tokio::spawn` и владение теряется,
/// а наблюдать занятость слота нужно ИЗВНЕ. Процессный счётчик `serving_counters()` для
/// этого не годится — он общий на весь тест-бинарь, и под параллельным прогоном оракул мерил
/// бы соседей (ровно то противоречие «C4 требует --test-threads=1 ↔ CI гоняет параллельно»,
/// которое ревьюер вынес founder'у; оно снимается конструкцией, а не решением).
///
/// `slots_handle()` — аддитивный доступ, задача dev'а №2 круга `R-196`.
#[cfg(feature = "testing")]
async fn serve_with_slots(
    dir: &std::path::Path,
    ckpt: Option<std::path::PathBuf>,
) -> (
    String,
    std::sync::Arc<gateway_serve::admission::ServingSlots>,
) {
    let server = bind_with_policy(config(dir, ckpt), policy())
        .await
        .expect("bind_with_policy");
    let addr = server.local_addr().to_string();
    let slots = server
        .slots_handle()
        .expect("bind_with_policy обязан дать ручку слотов");
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    (addr, slots)
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(addr: &str) -> Ws {
    let url = format!("ws://{addr}/?token={}", sign());
    let (ws, _) = tokio::time::timeout(BUDGET, tokio_tungstenite::connect_async(url))
        .await
        .expect("connect timeout")
        .expect("connect");
    ws
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string())).await.expect("send");
}

async fn recv(ws: &mut Ws) -> Option<Value> {
    match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref()).ok(),
        _ => None,
    }
}

fn subscribe(id: &str, sel: Value) -> Value {
    json!({"op":"subscribe","v":1,"id":id,"selector":sel})
}

fn canonical_selector_json() -> Value {
    json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
           "bands":CANONICAL.to_vec(),"window_ms":60000})
}

// ═════════════ C1/C2 — точка входа отвечает ИСХОДОМ, не пересчётом ═════════════

/// Холодный публичный запрос: слепка нет. Сервер обязан ответить НАЗВАННЫМ исходом,
/// и журнал при этом читаться НЕ ДОЛЖЕН — проверяется счётчиком, снятым до и после.
#[tokio::test]
async fn c1_entry_cold_request_named_outcome_without_reading_journal() {
    let dir = journal_busy(3_000);
    let addr = serve(dir.path(), None).await;

    let before = serving_counters().journal_payload_bytes_read;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал на подписку");

    assert_eq!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("error"),
        "холодный запрос обслужен вместо отказа: {msg}"
    );
    let code = msg.get("code").and_then(|c| c.as_str()).unwrap_or("");
    assert!(
        matches!(code, "not_ready" | "warming"),
        "исход обязан быть НАЗВАН ('not_ready'/'warming'), получено '{code}'"
    );

    let after = serving_counters().journal_payload_bytes_read;
    assert_eq!(
        after,
        before,
        "публичный путь прочитал {} байт журнала при неготовом состоянии — предохранитель \
         не стоит между валидацией селектора и spawn_blocking",
        after - before
    );
}

/// АНТИ-ПЛАЦЕБО: реализация «всегда отказывать» проходит тест выше. Здесь она обязана упасть.
#[tokio::test]
async fn c1_entry_ready_state_is_actually_served() {
    let dir = journal_busy(500);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");

    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");

    assert_eq!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "готовое состояние не обслужено: {msg}. 'Отказывать всегда' решением не является"
    );
}

// ═════════════ C5 — политика допуска на РЕАЛЬНОМ входе ═════════════

/// **Дыра, найденная гейтом (`C-234` R1).** `window_ms: null` — это unbounded offline-свёртка
/// (`crates/gateway/src/lib.rs:287-305`), и общий валидатор её ПРОПУСКАЕТ. Политика,
/// проверяющая только символ и полосы, такой запрос отклонить не способна.
#[tokio::test]
async fn c5_entry_unbounded_profile_is_refused() {
    let dir = journal_busy(3_000);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;

    let before = serving_counters().journal_payload_bytes_read;
    let mut ws = connect(&addr).await;
    // Символ и полосы КАНОНИЧЕСКИЕ, профиль — неограниченный.
    send(
        &mut ws,
        subscribe(
            "s1",
            json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1,
                   "bands":CANONICAL.to_vec(),"window_ms":null}),
        ),
    )
    .await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");

    assert_eq!(
        msg.get("code").and_then(|c| c.as_str()),
        Some("unsupported"),
        "неограниченный профиль принят: {msg}. Символ и полосы разрешены, но окно не \
         ограничено — это публичный путь к полной свёртке истории"
    );
    assert_eq!(
        serving_counters().journal_payload_bytes_read,
        before,
        "отказ наступил ПОСЛЕ чтения журнала — работа уже оплачена"
    );
}

/// Параметры НЕ подменяются молча на поддержанные (`CT-RFC-09` §2.7).
#[tokio::test]
async fn c5_entry_refusal_does_not_substitute_parameters() {
    let dir = journal_busy(200);
    let addr = serve(dir.path(), None).await;
    let mut ws = connect(&addr).await;
    send(
        &mut ws,
        subscribe(
            "s1",
            json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
                   "bands":[0.0123,0.4567],"window_ms":60000}),
        ),
    )
    .await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    assert_eq!(
        msg.get("code").and_then(|c| c.as_str()),
        Some("unsupported"),
        "неканонический набор полос принят: {msg}"
    );
    assert_ne!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "вместо отказа пришёл снимок — параметры подменены молча"
    );
}

/// Соседняя подписка и соединение остаются живыми (`CT-RFC-09` §2.7).
#[tokio::test]
async fn c5_entry_refusal_keeps_connection_and_neighbours_alive() {
    let dir = journal_busy(500);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;

    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("good", canonical_selector_json())).await;
    let first = recv(&mut ws).await.expect("нет ответа на годную подписку");
    assert_eq!(first.get("type").and_then(|t| t.as_str()), Some("snapshot"));

    send(
        &mut ws,
        subscribe(
            "bad",
            json!({"venue":"Binance","symbol":"DOGEUSDT","timeframe_ms":1000,
                                "bands":CANONICAL.to_vec(),"window_ms":60000}),
        ),
    )
    .await;
    let refused = recv(&mut ws).await.expect("соединение закрыто отказом");
    assert_eq!(
        refused.get("code").and_then(|c| c.as_str()),
        Some("unsupported"),
        "неразрешённый инструмент принят: {refused}"
    );
    assert_eq!(
        refused.get("sub").and_then(|x| x.as_str()),
        Some("bad"),
        "отказ обязан быть адресован СВОЕЙ подписке, а не соединению"
    );
}

// ═════════════ C4 — слот НЕ освобождается по таймауту ОЖИДАНИЯ ═════════════

/// **УДЕРЖАНИЕ РАБОТЫ — ТОЧКА РАНДЕВУ, А НЕ ПОДМЕНА ФАЙЛА ЖУРНАЛА (`R-196` B8).**
///
/// Прежняя редакция ставила вместо сегмента журнала именованный канал: чтение сегмента
/// упиралось в него и стояло, пока тест не отпустит. Конструкция была неверна В КОРНЕ, и
/// вердикт `R-196` назвал цену точно.
///
/// **Почему неверна.** Смысл всего милестоуна — чтобы живой путь журнал НЕ ЧИТАЛ: он
/// обязан подниматься из прогретого слепка. Значит защёлка, сделанная из сегмента журнала,
/// на исправной реализации не удерживает НИЧЕГО. Разработчик, которому оракул надо было
/// сделать зелёным, поступил единственно возможным способом: заставил `readiness` обходить
/// все сегменты журнала на КАЖДОМ запросе — `let _ = journal_list_segments_count(dir)`,
/// результат отброшен, вызов существует ради побочного эффекта в тесте. Мой оракул
/// продавил в продукт ровно тот расход, против которого продукт затевается. Это худший
/// вид дефекта оракула: он не пропускает дефект, он ЕГО СОЗДАЁТ.
///
/// **Правильный механизм в проекте уже есть и обкатан** — `crate::test_sync::rendezvous`
/// (M-65, `R-086` §10.3). Работа в `spawn_blocking` сигналит «вошла» и ждёт разрешения
/// теста; тест ждёт сигнала С ПРЕДЕЛОМ (`test_wait_for_pump` возвращает `bool`, а не висит).
/// Свойства, которых у канала не было:
/// · **нулевая цена для прода** — вызов под `#[cfg(any(test, feature = "testing"))]`, в
///   боевой сборке блок пуст;
/// · **детерминизм** — удержание управляется тестом, а не длительностью и не файловой
///   системой;
/// · **ограниченность по построению** — сторож от зависаний (`LatchWatchdog`) больше не
///   нужен: ожидание теста имеет предел, а работа ждёт Condvar внутри блокирующего пула,
///   не на потоке рантайма.
///
/// **Что удалено вместе с каналом:** `latch_segment`, `release_latch`, `LatchGuard`,
/// `LatchWatchdog` и две пробы сторожа. Они решали задачи, порождённые неверной
/// конструкцией, и ни одна из них не переживает её замену. Урок сохранён в спеке
/// (§14.1bis), а не в мёртвом коде.
///
/// **Счётчик слотов читается ПО ЭКЗЕМПЛЯРУ, а не процессный.** Это снимает требование
/// `--test-threads=1`, которое ревьюер вынес founder'у как противоречие с параллельным
/// прогоном CI (`gates.md` §3, паритет). Противоречия больше нет: оракул не трогает
/// процессное состояние и не мешает соседям по бинарю.
///
/// **ТРЕБУЕТ ОТ DEV (задачи §13, добавлены кругом `R-196`):**
/// 1. вызов `test_sync::rendezvous::pump_signal_and_wait(&id)` ПЕРВОЙ строкой замыкания
///    `spawn_blocking` на ADD-пути (`LiveReducer::resume` + `snapshot_checked`), под
///    `#[cfg(any(test, feature = "testing"))]`, где `id` — идентификатор подписки;
/// 2. аддитивный доступ `Server::slots_handle(&self) -> Option<Arc<ServingSlots>>`;
/// 3. удаление `journal_list_segments_count` из `readiness` — он существовал только ради
///    прежней защёлки.
/// До этого оракул НЕ СОБИРАЕТСЯ под `--features testing` — это и есть требуемое
/// RED-состояние; без флага он не компилируется вовсе, поэтому `cargo test --all` и CI
/// остаются зелёными (тот же порядок, что у `O-12` в M-65).

/// **ИМЯ КАНАЛА РАЗЛИЧАЕТ ЗАМЫКАНИЯ, И ЭТО НЕ УКРАШЕНИЕ (`C-245` R2).**
///
/// В дереве УЖЕ ЕСТЬ два вызова `pump_signal_and_wait` с ГОЛЫМ `id` подписки — это
/// периодический v1-pump из M-65 (`lib.rs:1495`, `:2038`), механизм совсем другой задачи.
/// Первая редакция этого оракула ждала ровно такой же голый `id` — и потому НЕ МОГЛА
/// отличить, какое замыкание подало сигнал. Дев, добавив один `slots_handle()`, получил бы
/// зелёный `C4` БЕЗ требуемой точки на ADD-пути: сигнал пришёл бы от старого pump'а.
/// Оракул проверял бы наличие КАКОГО-ТО рандеву вместо инварианта задачи 13.
///
/// Префикс делает подмену невозможной СТРУКТУРНО: канал `m87-add:<sub>` не может быть
/// разбужен pump'ом, который пишет в канал `<sub>`. Плюс сценарий `c4b` ниже наблюдает
/// разницу поведением, а не именем.
#[cfg(feature = "testing")]
fn add_channel(sub: &str) -> String {
    format!("m87-add:{sub}")
}

/// Снять канал рандеву на любом выходе, включая панику: оставленный канал достался бы
/// следующему тесту в том же бинаре уже «взведённым».
#[cfg(feature = "testing")]
struct RendezvousGuard(String);

#[cfg(feature = "testing")]
impl Drop for RendezvousGuard {
    fn drop(&mut self) {
        gateway_serve::test_sync::rendezvous::test_release(&self.0);
        gateway_serve::test_sync::rendezvous::test_remove(&self.0);
    }
}

/// ПОЛНЫЙ жизненный цикл слота: удержание → отказ второму → наблюдаемое освобождение.
/// Каждый переход управляется ТЕСТОМ, а не временем.
#[cfg(feature = "testing")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn c4_slot_lifecycle_hold_refuse_release() {
    use gateway_serve::test_sync::rendezvous;

    let dir = journal_busy(300);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");

    // Канал взводится ДО подъёма сервера: работа первой подписки не должна проскочить
    // точку рандеву раньше, чем тест будет готов её ждать. Имя — КАНАЛА ADD-ПУТИ, а не
    // голый идентификатор подписки: голый занят периодическим pump'ом M-65 (`C-245` R2).
    let ch = add_channel("a");
    rendezvous::arm(&ch);
    let _rv = RendezvousGuard(ch.clone());

    let (addr, slots) = serve_with_slots(dir.path(), Some(ckpt.path().to_path_buf())).await;

    // (1) УДЕРЖАНИЕ. Первый клиент входит; его работа встаёт на рандеву.
    let mut a = connect(&addr).await;
    send(&mut a, subscribe("a", canonical_selector_json())).await;
    assert!(
        rendezvous::test_wait_for_pump(&ch, std::time::Duration::from_secs(10)),
        "setup-страж: работа НЕ ДОШЛА до точки рандеву «{ch}» за 10 с. Либо вызов \
         `pump_signal_and_wait(&format!(\"m87-add:{{id}}\"))` не стоит ПЕРВОЙ строкой \
         замыкания ADD-пути (задача 13), либо запрос не дошёл до работы вовсе. Сценарий \
         НЕСОСТОЯВШИЙСЯ — всё ниже проверяло бы не то"
    );
    // РАЗЛИЧИТЕЛЬ ЗАМЫКАНИЙ (`C-245` R2), и он важнее имени канала: пока работа стоит на
    // рандеву, клиент не получил НИЧЕГО. Если бы сигнал подало замыкание периодического
    // pump'а, ADD-работа к этому моменту была бы ЗАВЕРШЕНА и снимок уже ушёл бы клиенту.
    // Имя канала закрывает подмену структурно, эта проверка — наблюдением.
    let early = tokio::time::timeout(std::time::Duration::from_millis(700), a.next()).await;
    assert!(
        early.is_err(),
        "клиент получил ответ, пока работа якобы удерживается на рандеву: {early:?}. \
         Значит удерживается НЕ ADD-работа, а что-то после неё — например периодический \
         pump M-65. Слот при этом мог быть занят по другому пути, и `in_flight()==1` ниже \
         ничего бы не доказывал"
    );
    assert_eq!(
        slots.in_flight(),
        1,
        "слот обязан быть занят, пока работа стоит на рандеву"
    );

    // (2) ОТКАЗ ВТОРОМУ — пока первая работа ДЕЙСТВИТЕЛЬНО в полёте.
    let mut b = connect(&addr).await;
    send(&mut b, subscribe("b", canonical_selector_json())).await;
    let mb = recv(&mut b).await.expect("второй клиент не получил ответа");
    let code = mb.get("code").and_then(|c| c.as_str()).unwrap_or("");
    assert_eq!(
        code, "overloaded",
        "при занятом единственном слоте второй запрос обязан получить РОВНО 'overloaded': {mb}"
    );
    assert_eq!(
        slots.in_flight(),
        1,
        "слот освобождён, пока работа ещё стоит на рандеву — это и есть запрещённое \
         освобождение по таймауту ОЖИДАНИЯ"
    );

    // (3) ОСВОБОЖДЕНИЕ управляется ТЕСТОМ.
    rendezvous::test_release(&ch);
    let first = recv(&mut a).await.expect("первый клиент не получил ответа");
    assert_eq!(
        first.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "отпущенная работа обязана завершиться выдачей, а не отказом: {first}"
    );

    let mut freed = false;
    for _ in 0..40 {
        if slots.in_flight() == 0 {
            freed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        freed,
        "после завершения работы слот НЕ освобождён (in_flight={})",
        slots.in_flight()
    );

    // (4) Освобождённый слот ФАКТИЧЕСКИ ПОЛУЧЕН следующим запросом (`C-242` R5-2).
    //
    // «Слот не занят» и «слот получен» — разные утверждения, и доказывать надо второе:
    // отказ по ЛЮБОЙ другой причине (`not_ready`, `unsupported`, ошибка селектора) тоже
    // даёт «не overloaded». Поэтому проверяется ОБСЛУЖИВАНИЕ, а не отсутствие отказа.
    // Процессный счётчик успехов для этого больше НЕ используется: он общий на бинарь и
    // под параллельным прогоном мерил бы соседей, а не предмет.
    let mut c = connect(&addr).await;
    send(&mut c, subscribe("c", canonical_selector_json())).await;
    let mc = recv(&mut c).await.expect("третий клиент не получил ответа");
    assert_eq!(
        mc.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "третий запрос ОБСЛУЖЕН не был: {mc}. Освобождение слота доказывается тем, что \
         следующий запрос его ПОЛУЧИЛ и был обслужен, а не тем, что ему не отказали \
         именно по перегрузке"
    );
}

// ═════════════ C6 — счётчики ЭМИТЯТСЯ продюсером на реальном пути ═════════════

/// `OPS-I-10` (`docs/fa/ops.md:478`): объявления мало — метрика обязана РЕАЛЬНО
/// инкрементироваться продюсером. Первая редакция набора конструировала `ServingCounters`
/// литералом и судила чистую функцию правила тревоги; гейт это отклонил (`C-234` R3).
#[tokio::test]
async fn c6_counters_are_emitted_by_the_real_serving_path() {
    let dir = journal_busy(300);
    let addr = serve(dir.path(), None).await;

    let before = serving_counters();
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let _ = recv(&mut ws).await;
    send(
        &mut ws,
        subscribe(
            "s2",
            json!({"venue":"Binance","symbol":"DOGEUSDT","timeframe_ms":1000,
                               "bands":CANONICAL.to_vec(),"window_ms":60000}),
        ),
    )
    .await;
    let _ = recv(&mut ws).await;
    let after = serving_counters();

    assert!(
        after.attempts > before.attempts,
        "счётчик попыток не вырос после ДВУХ реальных запросов — продюсера нет, метрика мертва"
    );
    assert!(
        after.refusals_supported > before.refusals_supported,
        "отказ по неготовности не посчитан как отказ ПОДДЕРЖАННОГО запроса"
    );
    assert!(
        after.refusals_unsupported > before.refusals_unsupported,
        "отказ неразрешённому инструменту не посчитан ОТДЕЛЬНО — поток мусора будет \
         держать тревогу включённой"
    );
}

// ═════════════ C9 — четыре позиции свежести ═════════════

/// Позиции обязаны быть РАЗЛИЧИМЫ **конструкцией фикстуры** (`A-037` У-7): слепок строится,
/// ЗАТЕМ журнал дописывается, ЗАТЕМ поднимается сервер. Обязана выполняться СТРОГАЯ
/// `snapshot_ms < source_ms` — одна общая константа на четыре поля этот тест не проходит.
///
/// И отставание СЛЕПКА не имеет права останавливать исправную живую выдачу: слепок — кэш,
/// а не источник истины.
#[tokio::test]
async fn c9_four_positions_differ_and_stale_snapshot_does_not_stop_serving() {
    let dir = journal_busy(300);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");

    // Журнал уезжает ПОСЛЕ слепка — источник заведомо свежее слепка.
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..200i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_020.0),
                    size: to_fixed(0.2),
                    side: Side::Buy,
                    ts_exch_ms: BASE_MS + 500_000 + i * 100,
                },
            ))
            .expect("trade");
        }
        j.flush().expect("flush");
    }

    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    assert_eq!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "исправная выдача остановлена при отставшем СЛЕПКЕ: {msg}. Слепок — кэш, а не \
         источник истины"
    );

    let f = freshness();
    assert!(
        f.source_ms > 0 && f.projection_ms > 0 && f.published_ms > 0 && f.snapshot_ms > 0,
        "четыре позиции свежести обязаны быть заполнены: {:?}",
        (f.source_ms, f.projection_ms, f.published_ms, f.snapshot_ms)
    );
    assert!(
        f.snapshot_ms < f.source_ms,
        "позиции НЕ различимы: слепок ({}) не отстаёт от источника ({}) на фикстуре, где \
         журнал дописан ПОСЛЕ слепка. Одна общая константа на четыре поля этот тест не \
         проходит — и не должна",
        f.snapshot_ms,
        f.source_ms
    );
    assert!(
        f.published_ms >= f.snapshot_ms,
        "опубликованные данные не могут отставать от слепка: слепок пишется ИЗ них"
    );
}

// ════════ У-1 (`A-037`) — ЧЕТЫРЕ состояния слепка НА ТОЧКЕ ВХОДА, через ЛОВУШКУ ════════
//
// ПРИМЕЧАНИЕ О ПОТЕРЕ И ВОССТАНОВЛЕНИИ. Этот блок был написан, а затем УНИЧТОЖЕН правкой
// соседнего теста: скрипт обрезал файл по индексу строки и снёс всё, что шло после. Автор
// отчитался о нём как о сделанном, не проверив грепом. Поймал гейт (`C-238` R3-1), а не
// автор. След оставлен намеренно: правило «утверждение о состоянии — только с командой в
// том же сообщении» нарушено ровно там, где казалось, что проверять нечего.
//
// План §15.1 требует подставного читателя и прогона на четырёх состояниях. Ловушка строится
// ФИКСТУРОЙ (`A-037` §3.4): порча полезной нагрузки при целом заголовке, прод-путь чтения
// строг к CRC. Три обязательных спутника — ниже, без них ловушка есть плацебо.

/// СТРАЖ ЛОВУШКИ: она обязана предъявить себя. Если чтение испорченного журнала НЕ падает,
/// все оракулы ниже проверяют не тот сценарий, и набор объявляет setup несостоявшимся.
#[test]
fn u1_guard_trap_actually_traps() {
    let dir = journal_busy(2_000);
    poison_segment(dir.path(), 0);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let res = gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    );
    assert!(
        res.is_err(),
        "setup-страж: чтение журнала-ловушки НЕ упало — ловушка не ловит, и проверки \
         'журнал не читался' ниже бессмысленны"
    );
}

/// Состояние 1 — слепка НЕТ, журнал под ловушкой. Вход обязан ответить НАЗВАННЫМ исходом
/// готовности, а не транспортной ошибкой: `invalid_selector`/`resume failed` означает, что
/// журнал уже читали, то есть допуска на пути нет.
#[tokio::test]
async fn u1_entry_missing_checkpoint_over_trap_gives_named_outcome() {
    let dir = journal_busy(2_000);
    poison_segment(dir.path(), 0);
    let addr = serve(dir.path(), None).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    let code = msg.get("code").and_then(|c| c.as_str()).unwrap_or("");
    assert!(
        matches!(code, "not_ready" | "warming"),
        "ожидался НАЗВАННЫЙ исход готовности, получено '{code}': {msg}"
    );
}

/// Состояние 2 — слепок ПОВРЕЖДЁН, журнал под ловушкой.
#[tokio::test]
async fn u1_entry_corrupt_checkpoint_over_trap_gives_named_outcome() {
    let dir = journal_busy(2_000);
    let ckpt = tempfile::tempdir().expect("ckpt");
    std::fs::write(
        ckpt.path().join(format!(
            "ckpt-{:016x}.bin",
            gateway::checkpoint::selector_fingerprint(&canonical_sel())
        )),
        vec![0xABu8; 4096],
    )
    .expect("write corrupt ckpt");
    poison_segment(dir.path(), 0);

    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    let code = msg.get("code").and_then(|c| c.as_str()).unwrap_or("");
    assert!(
        matches!(code, "not_ready" | "warming"),
        "повреждённый слепок дал '{code}' вместо названного исхода: {msg}"
    );
}

/// Состояние 3 — слепок НЕСОВМЕСТИМ по версии провода, журнал под ловушкой.
#[tokio::test]
async fn u1_entry_incompatible_checkpoint_over_trap_gives_named_outcome() {
    let dir = journal_busy(600);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let path = ckpt.path().join(format!(
        "ckpt-{:016x}.bin",
        gateway::checkpoint::selector_fingerprint(&canonical_sel())
    ));
    let mut bytes = std::fs::read(&path).expect("read ckpt");
    let declared = u32::from_le_bytes(bytes[12..16].try_into().expect("4 байта"));
    assert_eq!(
        declared,
        gateway::GATEWAY_SCHEMA_VERSION,
        "setup-страж: байты 12..16 не несут версию провода"
    );
    bytes[12..16].copy_from_slice(&(declared + 1).to_le_bytes());
    std::fs::write(&path, &bytes).expect("write incompatible");
    poison_segment(dir.path(), 0);

    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    let code = msg.get("code").and_then(|c| c.as_str()).unwrap_or("");
    assert!(
        matches!(code, "not_ready" | "warming"),
        "несовместимый слепок дал '{code}' вместо названного исхода: {msg}"
    );
}

/// Состояние 4 — слепок ВАЛИДЕН, но ОТСТАЛ сверх бюджета. Ловушка ставится в первый
/// сегмент ПОСЛЕ курсора слепка (`A-037` У-1): голова цела, хвост читать нельзя.
#[tokio::test]
async fn u1_entry_stale_checkpoint_beyond_budget_gives_named_outcome() {
    let dir = journal_busy(300);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let segs_before = std::fs::read_dir(dir.path())
        .expect("read_dir")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "jrnl"))
        .count();
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..4_000i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + (i % 20) as f64),
                    size: to_fixed(0.5),
                    side: Side::Buy,
                    ts_exch_ms: BASE_MS + 100_000 + i * 100,
                },
            ))
            .expect("trade");
        }
        j.flush().expect("flush");
    }
    poison_segment(dir.path(), segs_before);

    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    let code = msg.get("code").and_then(|c| c.as_str()).unwrap_or("");
    assert!(
        matches!(code, "not_ready" | "warming" | "overloaded"),
        "отставший сверх бюджета слепок дал '{code}': {msg}"
    );
}

/// СПУТНИК 2: тёплый путь над журналом С ЛОВУШКОЙ В ГОЛОВЕ обслуживается — слепок построен
/// ДО порчи, и `resume` со слепком голову не читает. Пин «слепок спасает от чтения головы».
#[tokio::test]
async fn u1_warm_path_is_served_over_trapped_head() {
    let dir = journal_busy(600);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    poison_segment(dir.path(), 0);

    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    assert_eq!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "тёплый путь не обслужен при ловушке в ГОЛОВЕ: {msg}"
    );
}

/// СПУТНИК 3: ПОЗИТИВНЫЙ КОНТРОЛЬ СЧЁТЧИКА. Без него реализация «никогда не инкрементировать
/// `journal_payload_bytes_read`» проходит все проверки «журнал не читался».
#[tokio::test]
async fn u1_served_request_with_tail_increments_payload_counter() {
    let dir = journal_busy(300);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..50i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_010.0),
                    size: to_fixed(0.1),
                    side: Side::Buy,
                    ts_exch_ms: BASE_MS + 200_000 + i * 10,
                },
            ))
            .expect("trade");
        }
        j.flush().expect("flush");
    }

    let before = serving_counters().journal_payload_bytes_read;
    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    assert_eq!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "тёплый запрос с хвостом обязан обслуживаться: {msg}"
    );
    assert!(
        serving_counters().journal_payload_bytes_read > before,
        "счётчик прочитанных байт НЕ вырос на законной докормке хвоста — реализация \
         'никогда не инкрементировать' удовлетворила бы все проверки отсутствия чтения"
    );
}
