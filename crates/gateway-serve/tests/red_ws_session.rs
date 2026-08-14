//! `O-1..O-10` — подписка есть параметр СЕССИИ, а не конфигурация процесса (M-65, `CT-RFC-09`).
//!
//! ИНВАРИАНТ ОТ РЕЗУЛЬТАТА (спека §4.1): что клиент получает в сокет, определяется ЕГО ТЕКУЩИМ
//! множеством подписок и ничем иным — ни конфигурацией процесса, ни подписками соседа по тому
//! же соединению, ни подписками другого соединения. Множество ИЗМЕНЧИВО: `subscribe` его
//! пополняет, `unsubscribe` сокращает, и выдача следует за ним в ОБЕ стороны, включая
//! освобождение места под лимитом. Всякий отказ выражен машиночитаемым `code`, оставляет
//! соединение и соседние подписки живыми и никогда не выглядит как молчание.
//!
//! ВОСЕМЬ ОСЕЙ (спека §4.2): 1 источник селектора · 2 число подписок · 3 валидность сообщения ·
//! 4 момент сообщения · 5 судьба соседей при отказе · 6 носитель отказа · 7 граница соединения ·
//! 8 жизненный цикл подписки. Оси 7 и 8 добавлены по `C-077` — обе следуют из грамматики самого
//! инварианта, и без них набор слеп к cross-talk и к no-op-`unsubscribe`.
//!
//! ПОЧЕМУ НАБОР ГОВОРИТ С СЕРВЕРОМ ПО ПРОВОДУ, А НЕ ЧЕРЕЗ ТИПЫ. Предмет `CT-RFC-09` — контракт
//! с ВНЕШНИМ миром: против него пишет фронт, который живёт в другом репозитории и наших типов
//! не видит никогда. Оракул, сверяющий внутренние структуры, доказал бы согласие кода с самим
//! собой. Поэтому сообщения отправляются как JSON-текст, а ответы разбираются как
//! `serde_json::Value`: проверяется ровно то, что увидит клиент. Побочная выгода — набор
//! КОМПИЛИРУЕТСЯ сегодня и краснеет ассертами, а не отсутствием типов: красный, называющий
//! дефект, полезнее красного «не собралось».
//!
//! СОСТОЯНИЕ НАБОРА: RED. Сервер сегодня читает клиентские сообщения и ВЫБРАСЫВАЕТ их
//! (`crates/gateway-serve/src/lib.rs:507-510`, комментарий «MVP: replay-контролы НЕ
//! реализованы»), отдаёт старую форму без конверта `{type,v,sub,data}` и не знает ни кодов
//! ошибок, ни лимита, ни grace-окна. Красное здесь — спецификация, написанная раньше кода
//! (`gates.md` §2), а не поломка.
//!
//! ЧТО ЗЕЛЕНО СЕГОДНЯ И ОБЯЗАНО ОСТАТЬСЯ: `O-5` — legacy-клиент, не приславший ничего, получает
//! серию по env-селектору. Это подписанное решение founder'а (переходный режим §2.5 живёт до
//! первого релиза фронта), и анти-плацебо в обе стороны требует, чтобы набор его НЕ ломал.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::{Cursor, Frame, Selector, Snapshot};
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m65-secret";
const FUTURE: usize = 9_999_999_999;
const BASE_MS: i64 = 1_700_000_000_000;
/// Бюджет ожидания одного сообщения. Заведомо больше `PUSH_INTERVAL_MS = 250`
/// (`gateway-serve/src/lib.rs:445`), чтобы медленная машина не давала флак.
const BUDGET: Duration = Duration::from_secs(10);
/// Значение подписано founder'ом 2026-08-11 (`CT-RFC-09` §6, спека §1.1).
const MAX_SUBS: usize = 16;
/// `initial_subscribe_grace_ms`, дефолт по `CT-RFC-09` §2.8.
const GRACE_MS: u64 = 250;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

// ─── МАНИФЕСТ: сценарий · ось · значение · вид (V нарушение / L легитимный) ──────────────
// Значения совпадают ПОСИМВОЛЬНО с атомами таблицы §4.2 спеки; `o0` сверяет состав в ОБЕ
// стороны. Одна фикстура вправе нести несколько claim'ов — каждый отдельной строкой.
const MANIFEST: &[(&str, u8, &str, char)] = &[
    ("o1", 1, "выдача старого селектора после смены", 'V'),
    ("o1", 4, "кадр в полёте приходит после смены", 'V'),
    ("o2", 2, "вторая подписка смешивается с первой", 'V'),
    ("o2", 2, "две независимые подписки", 'L'),
    ("o3", 3, "неизвестная версия v молча игнорируется", 'V'),
    ("o3", 3, "неизвестная операция молча игнорируется", 'V'),
    ("o4", 2, "превышение лимита проходит", 'V'),
    ("o4", 2, "подписка ровно на лимите", 'L'),
    ("o5", 1, "клиент без subscribe получает env-селектор", 'L'),
    ("o5", 4, "молчание в окне даёт legacy", 'L'),
    ("o6", 6, "отказ выражен прозой без code", 'V'),
    ("o6", 6, "отказ несёт машиночитаемый code", 'L'),
    ("o7", 3, "невалидный селектор рвёт соединение", 'V'),
    ("o7", 5, "отказ одной подписки убивает соединение", 'V'),
    ("o7", 5, "отказ одной подписки глушит соседнюю", 'V'),
    ("o7", 5, "соседняя подписка продолжает поток", 'L'),
    (
        "o7",
        3,
        "валидный селектор отсутствующего в журнале инструмента даёт пустой snapshot",
        'L',
    ),
    ("o8", 1, "env побеждает клиентский subscribe", 'V'),
    (
        "o8",
        4,
        "subscribe после grace-окна не меняет инструмент",
        'V',
    ),
    ("o8", 4, "subscribe внутри grace-окна", 'L'),
    (
        "o9",
        7,
        "подписка другого соединения меняет выдачу текущего",
        'V',
    ),
    (
        "o9",
        7,
        "одинаковый sub id в двух соединениях делит состояние",
        'V',
    ),
    (
        "o9",
        7,
        "два соединения с одинаковым sub id и разными селекторами дают независимые потоки",
        'L',
    ),
    ("o10", 8, "unsubscribe не прекращает поток", 'V'),
    (
        "o10",
        8,
        "unsubscribe не освобождает место под лимитом",
        'V',
    ),
    ("o10", 8, "unsubscribe неизвестного id молчит", 'V'),
    (
        "o10",
        8,
        "unsubscribe одной подписки не трогает соседнюю",
        'L',
    ),
    (
        "o10",
        8,
        "место под лимитом освобождено и переиспользуемо",
        'L',
    ),
    (
        "o2",
        2,
        "кадры идут только одной подписке из нескольких",
        'V',
    ),
    ("o10", 8, "снятый id не подписывается повторно", 'V'),
    ("o11", 9, "кадр синтезирован сервером, а не журналом", 'V'),
    ("o11", 9, "кадр не отличим клиентом от настоящего", 'V'),
    ("o11", 9, "кадр отражает события журнала", 'L'),
    ("o11", 9, "молчание при отсутствии событий", 'L'),
];

fn claims(id: &str, axis: u8, value: &str) {
    assert!(
        MANIFEST
            .iter()
            .any(|(i, a, v, _)| *i == id && *a == axis && *v == value),
        "МАНИФЕСТ НЕ СОДЕРЖИТ заявленного покрытия: {id} / ось {axis} / «{value}». \
         Оракул, покрывающий значение вне манифеста, делает перечень осей ложью."
    );
}

// ─── фикстуры ───────────────────────────────────────────────────────────────────────────
fn lvl(px: f64, qty: f64) -> Level {
    Level {
        price: to_fixed(px),
        size: to_fixed(qty),
    }
}
fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 64 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m65".to_string(),
        epoch_id: "own-test".to_string(),
    }
}
fn sel_of(symbol: &str) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: symbol.to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
    }
}
fn config(dir: &Path, selector: Selector) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector,
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: None,
    }
}
fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m65".to_string(),
            exp: FUTURE,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt")
}

/// Журнал с ДВУМЯ инструментами — иначе «смена инструмента» и «изоляция» непроверяемы:
/// подписка на другой символ обязана давать ДРУГИЕ данные, а не те же самые.
fn seed(dir: &Path) {
    let mut j = Journal::open_with(dir, writer_cfg()).expect("open_with");
    for (sym, px) in [("BTCUSDT", 65_000.0), ("ETHUSDT", 3_000.0)] {
        j.append(EventKind::md(
            Venue::Binance,
            sym,
            MdPayload::L2Snapshot {
                bids: vec![lvl(px, 2.0)],
                asks: vec![lvl(px + 10.0, 1.5)],
                ts_exch_ms: BASE_MS,
            },
        ))
        .expect("append snapshot");
        for k in 0..8i64 {
            j.append(EventKind::md(
                Venue::Binance,
                sym,
                MdPayload::Trade {
                    price: to_fixed(px + k as f64),
                    size: to_fixed(0.1),
                    side: Side::Buy,
                    ts_exch_ms: BASE_MS + k,
                },
            ))
            .expect("append trade");
        }
    }
    j.flush().expect("flush");
}

/// Дозапись событий в УЖЕ ОТКРЫТЫЙ журнал — то, чего в наборе не было и из-за чего он был
/// неудовлетворим (`R-057` Б-3, разбор независимого Fable). `seed()` пишет всё ДО старта
/// сервера, после подписки журнал не рос НИКОГДА, и в окнах, где оракул требовал сообщение,
/// законным поведением было МОЛЧАНИЕ. Реализация, шлющая кадры только при наличии событий,
/// пройти набор не могла ПО ПОСТРОЕНИЮ — и изобрела синтетический кадр вне `CT-RFC-09` §2.3.
/// Оракул, вынуждающий подделать контракт, есть дефект ОРАКУЛА, а не реализации.
fn append_more(dir: &Path, symbol: &str, n: i64, from_ms: i64) {
    let mut j = Journal::open_with(dir, writer_cfg()).expect("reopen journal");
    for i in 0..n {
        j.append(EventKind::md(
            Venue::Binance,
            symbol,
            MdPayload::Trade {
                price: to_fixed(100.0 + i as f64),
                size: to_fixed(0.5),
                side: Side::Sell,
                ts_exch_ms: from_ms + i,
            },
        ))
        .expect("append more");
    }
    j.flush().expect("flush more");
}

/// ЭТАЛОН §4.6 — НЕЗАВИСИМЫЙ путь: `gateway::snapshot` строит состояние с нуля, не касаясь
/// push-конвейера. До этой правки эталона в наборе не было ВООБЩЕ (`grep gateway::` давал
/// единственную строку `use gateway::Selector`), поэтому ни один ассерт не проверял
/// СОДЕРЖИМОЕ — только присутствие идентификаторов и типов, и пустой кадр с
/// `SeriesBundle::default()` удовлетворял весь набор. §4.3 усл. 4 требовал эталон, а
/// механизма не было: норма без механизма — это ноль, а не «частично сделано».
///
/// КУРСОР — `Cursor::LATEST`, а НЕ `Cursor { upto_seq: None }` (`R-057` заявка dev'а, принята
/// architect'ом 13.08). `None` — это `Cursor::START`, «ничего не свёрнуто»: его `includes()`
/// возвращает `false` для ЛЮБОГО `seq` (`crates/gateway/src/lib.rs:166`), поэтому эталон
/// отдавал ПУСТУЮ серию. Дефект хуже упавшего теста: сравнение шло не с независимым
/// источником, а ни с чем, и `assert_eq!(факт, эталон)` был бы зелёным на пустом факте.
/// Эталон обязан быть не только независимым, но и НЕПУСТЫМ — иначе он не эталон.
fn reference_bars(dir: &Path, symbol: &str) -> usize {
    let snap = gateway::snapshot(
        dir,
        EpochFilter::OwnCaptureOnly,
        &sel_of(symbol),
        Cursor::LATEST,
    )
    .expect("эталон: gateway::snapshot");
    let n = snap.series.ohlcv.len();
    assert!(
        n > 0,
        "ЭТАЛОН ПУСТ для «{symbol}»: сравнивать не с чем, и любой ассерт против него зелен по \
         построению. Так набор уже был сломан однажды курсором `START` вместо `LATEST`."
    );
    n
}

async fn serve(dir: &Path, env_symbol: &str) -> String {
    let server = bind(config(dir, sel_of(env_symbol))).await.expect("bind");
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
        .expect("бюджет коннекта")
        .expect("connect");
    ws
}
async fn connect_subscribed(
    addr: &str,
    sub: &str,
    symbol: &str,
    selector: &Selector,
) -> (Ws, Snapshot) {
    let mut last_seen: Option<Value> = None;
    for attempt in 1..=8 {
        let mut ws = connect(addr).await;
        send(&mut ws, subscribe(sub, symbol)).await;
        match recv(&mut ws).await {
            Some(v) if type_of(&v) == Some("snapshot") && sub_of(&v) == Some(sub) => {
                let snap = wire_snapshot(&v, sub, selector);
                return (ws, snap);
            }
            Some(v) => {
                last_seen = Some(v);
                let _ = ws.close(None).await;
            }
            None => {
                let _ = ws.close(None).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(10 * attempt)).await;
    }
    panic!(
        "v1 subscribe не попал в grace-window за 8 попыток; последний ответ: {:?}",
        last_seen
    );
}
async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string())).await.expect("send");
}
/// Одно сообщение в пределах бюджета. `None` = сервер промолчал.
async fn recv(ws: &mut Ws) -> Option<Value> {
    match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref()).ok(),
        _ => None,
    }
}
/// Собрать всё, что сервер пришлёт за `ms`, — форма для «не должно прийти НИЧЕГО такого».
async fn drain(ws: &mut Ws, ms: u64) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    let mut out = Vec::new();
    while tokio::time::Instant::now() < deadline {
        let left = deadline - tokio::time::Instant::now();
        match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(m))) => {
                if let Ok(v) = serde_json::from_slice::<Value>(m.into_data().as_ref()) {
                    out.push(v);
                }
            }
            _ => break,
        }
    }
    out
}
fn subscribe(id: &str, symbol: &str) -> Value {
    json!({"op":"subscribe","v":1,"id":id,"selector":{
        "venue":"Binance","symbol":symbol,"timeframe_ms":1000,
        "bands":[0.001],"window_ms":60000}})
}
fn unsubscribe(id: &str) -> Value {
    json!({"op":"unsubscribe","v":1,"id":id})
}
/// Есть ли в сообщении ХОТЬ ЧТО-ТО, кроме пустых серий. Синтетический кадр отличается от
/// настоящего ровно этим: у него все массивы пусты (`SeriesBundle::default()`), и до оси 9
/// такой кадр удовлетворял ВЕСЬ набор — ассерты проверяли присутствие идентификаторов, а не
/// содержимое. Проверка идёт по ПРОВОДНОЙ форме: клиент видит именно её.
fn has_content(v: &Value) -> bool {
    fn walk(v: &Value) -> bool {
        match v {
            Value::Array(a) => !a.is_empty() && a.iter().any(|x| !x.is_null()),
            Value::Object(m) => m.values().any(walk),
            _ => false,
        }
    }
    v.get("data").map(walk).unwrap_or(false)
}

/// `sub` конверта (`CT-RFC-09` §2.3). Отсутствие поля — уже нарушение формы.
fn sub_of(v: &Value) -> Option<&str> {
    v.get("sub").and_then(|s| s.as_str())
}
fn type_of(v: &Value) -> Option<&str> {
    v.get("type").and_then(|s| s.as_str())
}
fn wire_snapshot(v: &Value, sub: &str, client_selector: &Selector) -> Snapshot {
    assert_eq!(
        type_of(v),
        Some("snapshot"),
        "ожидался snapshot для подписки «{sub}», пришло: {v}"
    );
    assert_eq!(
        sub_of(v),
        Some(sub),
        "snapshot пришёл не той подписке: ожидали «{sub}», пришло: {v}"
    );
    let data = v
        .get("data")
        .cloned()
        .unwrap_or_else(|| panic!("snapshot без data: {v}"));
    let mut snap: Snapshot = serde_json::from_value(data)
        .unwrap_or_else(|e| panic!("snapshot data не разбирается как gateway::Snapshot: {e}; {v}"));
    assert_eq!(
        &snap.selector, client_selector,
        "snapshot data.selector обязан совпадать с selector'ом, который клиент послал в \
         subscribe. Иначе фронт получает правильное содержимое под чужой подписью инструмента."
    );
    // Якорь Р-Б — selector, который КЛИЕНТ послал в subscribe, а не Snapshot.selector:
    // поле в снапшоте проставляет тот же сервер, чьё смешение подписок мы проверяем.
    snap.selector = client_selector.clone();
    snap
}
fn wire_frames(msgs: &[Value], sub: &str) -> Vec<Frame> {
    msgs.iter()
        .filter(|v| type_of(v) == Some("frame") && sub_of(v) == Some(sub))
        .map(|v| {
            let data = v
                .get("data")
                .cloned()
                .unwrap_or_else(|| panic!("frame без data для подписки «{sub}»: {v}"));
            serde_json::from_value(data).unwrap_or_else(|e| {
                panic!("frame data не разбирается как gateway::Frame: {e}; {v}")
            })
        })
        .collect()
}
fn cursor_at_to(frame: &Frame) -> Cursor {
    let seq = frame
        .to
        .upto_seq
        .expect("frame.to обязан быть Cursor::at(seq), а не START/LATEST");
    Cursor::at(seq)
}
fn apply_frames(mut acc: Snapshot, frames: &[Frame], label: &str) -> Snapshot {
    assert!(
        !frames.is_empty(),
        "{label}: SETUP НЕ СОСТОЯЛСЯ — кадров нет; оракул сходимости зеленел бы вакуумно"
    );
    for frame in frames {
        assert_eq!(
            frame.from, acc.cursor,
            "{label}: кадр не продолжает текущий курсор снапшота. from={:?}, acc.cursor={:?}, to={:?}",
            frame.from, acc.cursor, frame.to
        );
        acc.apply(frame);
    }
    acc
}
fn reference_at(dir: &Path, selector: &Selector, cursor: Cursor) -> Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, selector, cursor)
        .expect("независимый эталон gateway::snapshot")
}

// ─── O-0: манифест ⇄ таблица осей §4.2 спеки, в ОБЕ стороны ─────────────────────────────
#[test]
fn o0_manifest_covers_every_axis_in_both_directions() {
    let spec = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../milestones/M-65-ws-session.md"),
    )
    .expect("спека M-65 читается — состав осей сверять не с чем без неё");

    // Атомы таблицы §4.2: колонка 2 — нарушения (V), колонка 3 — легитимные (L).
    let mut in_42 = false;
    let mut from_spec: Vec<(u8, String, char)> = Vec::new();
    for line in spec.lines() {
        if line.starts_with("### 4.2") {
            in_42 = true;
            continue;
        }
        if in_42 && line.starts_with("### ") {
            break;
        }
        if !in_42 || !line.starts_with("| **") {
            continue;
        }
        let cols: Vec<&str> = line.split('|').collect();
        if cols.len() < 4 {
            continue;
        }
        let axis: u8 = cols[1]
            .trim()
            .trim_start_matches('*')
            .split('.')
            .next()
            .and_then(|d| d.trim().parse().ok())
            .expect("номер оси");
        for (col, kind) in [(cols[2], 'V'), (cols[3], 'L')] {
            for atom in col.split('`').skip(1).step_by(2) {
                if !atom.trim().is_empty() {
                    from_spec.push((axis, atom.to_string(), kind));
                }
            }
        }
    }
    assert!(
        !from_spec.is_empty(),
        "разбор §4.2 дал ПУСТО — парсер сломан либо раздел переименован; сверять не с чем"
    );

    let missing: Vec<String> = from_spec
        .iter()
        .filter(|(a, v, k)| {
            !MANIFEST
                .iter()
                .any(|(_, ma, mv, mk)| ma == a && mv == v && mk == k)
        })
        .map(|(a, v, k)| format!("ось {a} [{k}] «{v}»"))
        .collect();
    let extra: Vec<String> = MANIFEST
        .iter()
        .filter(|(_, a, v, k)| {
            !from_spec
                .iter()
                .any(|(sa, sv, sk)| sa == a && sv == v && sk == k)
        })
        .map(|(i, a, v, k)| format!("{i}: ось {a} [{k}] «{v}»"))
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "СОСТАВ РАЗОШЁЛСЯ.\n  объявлено в §4.2, НЕ покрыто набором: {missing:#?}\n  \
         покрыто набором, НЕ объявлено в §4.2: {extra:#?}\n\
         Сверка двусторонняя намеренно: односторонняя пропускает и забытое значение, и \
         самодеятельное покрытие вне перечня осей."
    );

    // §4.3 усл. 2: у КАЖДОЙ оси есть легитимный сценарий — иначе набор проходит «запретить всё».
    for axis in 1..=8u8 {
        assert!(
            MANIFEST.iter().any(|(_, a, _, k)| *a == axis && *k == 'L'),
            "§4.3 усл. 2: у оси {axis} нет НИ ОДНОГО легитимного сценария"
        );
    }
}

// ─── O-1: смена инструмента (§2.4) ──────────────────────────────────────────────────────
#[tokio::test]
async fn o1_subscribe_switches_instrument_and_old_frames_stop() {
    claims("o1", 1, "выдача старого селектора после смены");
    claims("o1", 4, "кадр в полёте приходит после смены");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let mut ws = connect(&addr).await;

    send(&mut ws, subscribe("w1", "BTCUSDT")).await;
    let first = recv(&mut ws).await.expect("snapshot первой подписки");
    assert_eq!(
        type_of(&first),
        Some("snapshot"),
        "первым обязан прийти snapshot: {first}"
    );
    let _settle_before_switch = drain(&mut ws, 2 * GRACE_MS + 600).await;

    // Смена селектора тем же `id` (§2.2: «смена параметров = subscribe с ТЕМ ЖЕ id»).
    send(&mut ws, subscribe("w1", "ETHUSDT")).await;
    let after = recv(&mut ws).await.expect("snapshot после смены");
    assert_eq!(
        type_of(&after),
        Some("snapshot"),
        "смена селектора обязана начинаться с НОВОГО snapshot (§2.4), пришло: {after}"
    );
    let sel_eth = sel_of("ETHUSDT");
    let snap_eth = wire_snapshot(&after, "w1", &sel_eth);

    // Ни одного кадра прежнего селектора после нового снапшота. Кадр «в полёте» — это
    // деградированный вход оси 4: он уже был отправлен push-циклом в момент смены.
    append_more(dir.path(), "BTCUSDT", 4, BASE_MS + 6_000);
    append_more(dir.path(), "ETHUSDT", 4, BASE_MS + 6_000);
    let tail = drain(&mut ws, 3 * GRACE_MS + 1_200).await;
    let stale: Vec<&Value> = tail
        .iter()
        .filter(|v| {
            let s = serde_json::to_string(v).unwrap_or_default();
            s.contains("BTCUSDT")
        })
        .collect();
    assert!(
        stale.is_empty(),
        "O-1: после смены селектора пришло {} сообщений прежнего инструмента. §2.4 запрещает \
         это ровно потому, что клиент уже перерисовал виджет: чужие кадры после смены \
         неотличимы для него от новых. Примеры: {:?}",
        stale.len(),
        stale.iter().take(2).collect::<Vec<_>>()
    );

    let frames = wire_frames(&tail, "w1");
    let acc = apply_frames(snap_eth, &frames, "O-1 switch");
    let ref_eth = reference_at(
        dir.path(),
        &sel_eth,
        cursor_at_to(frames.last().expect("O-1 frames not empty")),
    );
    assert_eq!(
        acc, ref_eth,
        "O-1: после switch кадры подписки `w1` не продолжают ETH snapshot до независимого \
         эталона. Оракул обязан ловить не только буквальный BTCUSDT в проводной форме, но и \
         старый reducer, который продолжает присылать содержимое прежнего инструмента под тем \
         же id."
    );
}

// ─── O-2: мультиплекс ───────────────────────────────────────────────────────────────────
#[tokio::test]
async fn o2_multiplex_subscriptions_are_independent() {
    claims("o2", 2, "вторая подписка смешивается с первой");
    claims("o2", 2, "две независимые подписки");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let mut ws = connect(&addr).await;

    send(&mut ws, subscribe("a", "BTCUSDT")).await;
    send(&mut ws, subscribe("b", "ETHUSDT")).await;

    claims("o2", 2, "кадры идут только одной подписке из нескольких");
    let _ = drain(&mut ws, 1_000).await; // снапшоты обеих подписок

    // ПОТОКИ, а не присутствие идентификаторов. До этой правки O-2 считал, что `a` и `b`
    // встретились среди сообщений, — но снапшот приходит ОБЕИМ подпискам мгновенно, ещё до
    // вопроса о кадрах, поэтому реализация, пампящая на тик РОВНО ОДНУ подписку (`R-057` Б-1),
    // проходила оракул. Событие пишется ОБОИМ инструментам, и кадры с содержимым обязаны
    // прийти ОБЕИМ подпискам.
    append_more(dir.path(), "BTCUSDT", 6, BASE_MS + 5_000);
    append_more(dir.path(), "ETHUSDT", 6, BASE_MS + 5_000);
    let live = drain(&mut ws, 4 * GRACE_MS + 2_000).await;
    for id in ["a", "b"] {
        assert!(
            live.iter()
                .any(|v| sub_of(v) == Some(id) && type_of(v) == Some("frame") && has_content(v)),
            "O-2: подписка «{id}» не получила НИ ОДНОГО кадра с содержимым, хотя в её инструмент \
             дописаны события. Мультиплекс — это независимые ПОТОКИ, а не два идентификатора в \
             логе: реализация, отдающая на тик одну подписку, оставляет остальные жить одним \
             снапшотом, и виджет пользователя замирает навсегда. Пришло: {:?}",
            live.iter().filter_map(sub_of).collect::<Vec<_>>()
        );
    }

    // ВТОРОЕ ОКНО — про ДЛИТЕЛЬНОСТЬ, а не про факт: поток обязан идти СНОВА, а не однажды.
    // Инвариант отличается от проверенного выше («каждая подписка получила кадр»), поэтому
    // блок сохранён, но чинится источник (`R-057` заявка dev'а, принята architect'ом 13.08).
    //
    // ЧЕМ ОН БЫЛ СЛОМАН И ЧЕГО ЭТО СТОИЛО. Событий в это окно никто не дописывал, поэтому оно
    // было пусто ПО ПОСТРОЕНИЮ, и требование «кадры обязаны идти» не оставляло реализации
    // никакого выхода, кроме как ВЫДУМАТЬ источник. Она его и выдумала — синтетический
    // heartbeat вне `CT-RFC-09` §2.3, за который получила блокер `Б-3`. То есть дефект СПЕКИ
    // был предъявлен как дефект реализации. Оракул, требующий невозможного, не строг — он
    // порождает обход и наказывает за него.
    append_more(dir.path(), "BTCUSDT", 4, BASE_MS + 11_000);
    append_more(dir.path(), "ETHUSDT", 4, BASE_MS + 11_000);
    let msgs = drain(&mut ws, 4 * GRACE_MS + 2_000).await;
    assert!(
        !msgs.is_empty(),
        "O-2 SETUP НЕ СОСТОЯЛСЯ: во втором окне не пришло НИ ОДНОГО сообщения, хотя события \
         дописаны обоим инструментам. Проверка «каждое сообщение несёт `sub`» ниже на пустом \
         списке зелена ВАКУУМНО — молчащий оракул хуже отсутствующего."
    );
    let subs: Vec<&str> = msgs.iter().filter_map(sub_of).collect();
    for id in ["a", "b"] {
        assert!(
            msgs.iter()
                .any(|v| sub_of(v) == Some(id) && type_of(v) == Some("frame") && has_content(v)),
            "O-2: во ВТОРОМ окне подписка «{id}» не получила кадра с содержимым. Первое окно \
             доказывает, что поток НАЧАЛСЯ; это — что он ПРОДОЛЖАЕТСЯ. Реализация, отдающая \
             каждой подписке по одному кадру и замолкающая, проходит первую проверку и \
             оставляет виджет замершим. Пришло: {subs:?}"
        );
    }
    for m in &msgs {
        let s = sub_of(m).unwrap_or("<нет поля sub>");
        assert!(
            s == "a" || s == "b",
            "O-2: сообщение без корректного `sub` или с чужим: {s}. Каждое серверное сообщение \
             обязано нести идентификатор подписки (§2.3) — без него фронт не сопоставит ответ \
             с виджетом, и мультиплекс вырождается в кашу: {m}"
        );
    }
}

// ─── O-3: неизвестная версия и неизвестная операция ─────────────────────────────────────
#[tokio::test]
async fn o3_unknown_version_and_unknown_op_are_errors() {
    claims("o3", 3, "неизвестная версия v молча игнорируется");
    claims("o3", 3, "неизвестная операция молча игнорируется");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;

    for (name, msg) in [
        (
            "v:0",
            json!({"op":"subscribe","v":0,"id":"x","selector":{"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,"bands":[0.001],"window_ms":60000}}),
        ),
        (
            "v:999",
            json!({"op":"subscribe","v":999,"id":"x","selector":{"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,"bands":[0.001],"window_ms":60000}}),
        ),
        (
            "без поля v",
            json!({"op":"subscribe","id":"x","selector":{"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,"bands":[0.001],"window_ms":60000}}),
        ),
        ("неизвестная op", json!({"op":"resubscribe","v":1,"id":"x"})),
    ] {
        let mut ws = connect(&addr).await;
        send(&mut ws, msg).await;
        let got = drain(&mut ws, 1_500).await;
        let has_error = got.iter().any(|v| type_of(v) == Some("error"));
        assert!(
            has_error,
            "O-3 [{name}]: сервер не ответил `error`. Молчание — ХУДШИЙ из возможных исходов: \
             клиент считает, что подписался, и ждёт данные, которых не будет (`CT-RFC-09` §2.2). \
             Пришло: {got:?}"
        );
    }
}

// ─── O-4: лимит подписок ────────────────────────────────────────────────────────────────
#[tokio::test]
async fn o4_subscription_cap_is_fail_closed() {
    claims("o4", 2, "превышение лимита проходит");
    claims("o4", 2, "подписка ровно на лимите");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let mut ws = connect(&addr).await;

    // РОВНО на лимите — законно (легитимное значение оси 2).
    for k in 0..MAX_SUBS {
        send(&mut ws, subscribe(&format!("s{k}"), "BTCUSDT")).await;
    }
    let at_cap = drain(&mut ws, 2_000).await;
    let refused_at_cap: Vec<&Value> = at_cap
        .iter()
        .filter(|v| type_of(v) == Some("error"))
        .collect();
    assert!(
        refused_at_cap.is_empty(),
        "O-4: {MAX_SUBS} подписок — это РОВНО лимит, отказывать нельзя. Отказы: {refused_at_cap:?}"
    );

    // Превышение — `error` с кодом, НЕ разрыв и НЕ молчание (§2.6).
    send(&mut ws, subscribe("over", "BTCUSDT")).await;
    let over = drain(&mut ws, 2_000).await;
    let err = over.iter().find(|v| type_of(v) == Some("error"));
    let err = err.unwrap_or_else(|| {
        panic!(
            "O-4: подписка №{} прошла молча — лимит не проверяется. Отсутствие предела при цели \
             10 000 соединений означает, что один клиент способен занять узел целиком \
             (`gates.md`: «parse-error → unbounded — запрещено»). Пришло: {over:?}",
            MAX_SUBS + 1
        )
    });
    assert!(
        err.get("code").and_then(|c| c.as_str()).is_some(),
        "O-4: отказ пришёл без машиночитаемого `code`: {err}"
    );
    // Соединение обязано пережить отказ (ось 5).
    send(&mut ws, subscribe("s0", "ETHUSDT")).await;
    let alive = drain(&mut ws, 2_000).await;
    assert!(
        !alive.is_empty(),
        "O-4: после отказа по лимиту соединение перестало отвечать — fail-closed не значит \
         «рвать всё» (§2.6: НЕ разрыв соединения)"
    );
}

// ─── O-5: переходный режим ──────────────────────────────────────────────────────────────
#[tokio::test]
async fn o5_legacy_client_without_subscribe_is_still_served() {
    claims("o5", 1, "клиент без subscribe получает env-селектор");
    claims("o5", 4, "молчание в окне даёт legacy");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let mut ws = connect(&addr).await;

    // Клиент молчит — ровно то, что делает сегодняшний `wsprobe` и смоук-тесты.
    let got = drain(&mut ws, GRACE_MS + 2_000).await;
    assert!(
        !got.is_empty(),
        "O-5: legacy-клиент, не приславший ничего, не получил НИЧЕГО. Переходный режим §2.5 — \
         подписанное решение founder'а (живёт до первого релиза фронта), и на нём стоят wsprobe \
         и все прод-замеры. Анти-плацебо в обе стороны: набор не вправе ломать законный случай."
    );
}

// ─── O-6: носитель отказа ───────────────────────────────────────────────────────────────
#[tokio::test]
async fn o6_errors_carry_machine_readable_code() {
    claims("o6", 6, "отказ выражен прозой без code");
    claims("o6", 6, "отказ несёт машиночитаемый code");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let mut ws = connect(&addr).await;

    send(
        &mut ws,
        json!({"op":"subscribe","v":1,"id":"bad","selector":{
            "venue":"НетТакойБиржи","symbol":"BTCUSDT","timeframe_ms":1000,
            "bands":[0.001],"window_ms":60000}}),
    )
    .await;
    let got = drain(&mut ws, 2_000).await;
    let err = got
        .iter()
        .find(|v| type_of(v) == Some("error"))
        .unwrap_or_else(|| panic!("O-6: на неизвестный venue не пришло `error`: {got:?}"));
    let code = err.get("code").and_then(|c| c.as_str());
    assert_eq!(
        code,
        Some("unknown_venue"),
        "O-6: отказ обязан нести код из таксономии §2.7, а не свободный текст. Клиент различает \
         отказы БЕЗ разбора прозы: сообщение на другом языке или переформулированное ломает \
         разбор по тексту молча, и это тот же класс, что «молчание» — отказ, невидимый машине. \
         Пришло: {err}"
    );
}

// ─── O-7: валидация селектора, соседи и пустой снапшот ──────────────────────────────────
#[tokio::test]
async fn o7_selector_validation_keeps_connection_and_neighbours_alive() {
    claims("o7", 3, "невалидный селектор рвёт соединение");
    claims("o7", 5, "отказ одной подписки убивает соединение");
    claims("o7", 5, "отказ одной подписки глушит соседнюю");
    claims("o7", 5, "соседняя подписка продолжает поток");
    claims(
        "o7",
        3,
        "валидный селектор отсутствующего в журнале инструмента даёт пустой snapshot",
    );

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let mut ws = connect(&addr).await;

    // Живая соседняя подписка — она обязана пережить отказ соседа.
    send(&mut ws, subscribe("good", "BTCUSDT")).await;
    let _ = drain(&mut ws, 1_000).await;

    for (name, bad) in [
        (
            "пустой symbol",
            json!({"venue":"Binance","symbol":"","timeframe_ms":1000,"bands":[0.001],"window_ms":60000}),
        ),
        (
            "timeframe_ms <= 0",
            json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":0,"bands":[0.001],"window_ms":60000}),
        ),
        (
            "bands вне (0,1)",
            json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,"bands":[1.5],"window_ms":60000}),
        ),
        (
            "bands не отсортированы",
            json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,"bands":[0.01,0.001],"window_ms":60000}),
        ),
        (
            "bands с дублями",
            json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,"bands":[0.001,0.001],"window_ms":60000}),
        ),
    ] {
        send(
            &mut ws,
            json!({"op":"subscribe","v":1,"id":"bad","selector":bad}),
        )
        .await;
        let got = drain(&mut ws, 1_500).await;
        let err = got.iter().find(|v| type_of(v) == Some("error"));
        assert!(
            err.is_some(),
            "O-7 [{name}]: невалидный селектор обязан давать `error` с кодом (§2.7), а не \
             молчание и не подставленный дефолт: молча подставленный дефолт даёт клиенту \
             данные, которых он не просил. Пришло: {got:?}"
        );
        assert_eq!(
            err.and_then(|e| e.get("code")).and_then(|c| c.as_str()),
            Some("invalid_selector"),
            "O-7 [{name}]: код отказа обязан быть `invalid_selector` (§2.7)"
        );
    }

    // События ПОСЛЕ подписки: без них «соседняя подписка продолжает поток» непроверяемо —
    // молчание было бы законным, и оракул вынуждал бы синтезировать кадр (Б-3).
    append_more(dir.path(), "BTCUSDT", 6, BASE_MS + 1_000);
    // Соседняя подписка жива после серии отказов.
    let neighbour = drain(&mut ws, 2_000).await;
    assert!(
        neighbour.iter().any(|v| sub_of(v) == Some("good")),
        "O-7: после отказов соседняя подписка «good» замолчала. Разрыв или глушение соседей \
         означает, что один опечатавшийся виджет гасит весь экран пользователя (§2.7)."
    );

    // Валидный, но отсутствующий в журнале инструмент — ПУСТОЙ snapshot, а НЕ ошибка.
    send(&mut ws, subscribe("absent", "SOLUSDT")).await;
    let absent = drain(&mut ws, 2_000).await;
    let for_absent: Vec<&Value> = absent
        .iter()
        .filter(|v| sub_of(v) == Some("absent"))
        .collect();
    assert!(
        for_absent.iter().any(|v| type_of(v) == Some("snapshot")),
        "O-7: валидный селектор инструмента, которого нет в журнале, обязан дать ПУСТОЙ snapshot \
         и живую подписку — recorder может начать писать этот символ позже, и клиент обязан \
         увидеть это без переподключения. Различение сознательное: «мы тебя не поняли» (ошибка) \
         против «пока нечего показать» (пустой снапшот). Пришло: {for_absent:?}"
    );
    assert!(
        !for_absent.iter().any(|v| type_of(v) == Some("error")),
        "O-7: отсутствующий в журнале инструмент — НЕ ошибка (§2.7, последняя строка таблицы)"
    );
}

// ─── O-8: grace-окно и порядок сообщений при подключении (§2.8) ─────────────────────────
#[tokio::test]
async fn o8_grace_window_decides_v1_or_legacy() {
    claims("o8", 1, "env побеждает клиентский subscribe");
    claims("o8", 4, "subscribe после grace-окна не меняет инструмент");
    claims("o8", 4, "subscribe внутри grace-окна");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    // env-селектор — BTCUSDT; клиент просит ETHUSDT. Если env победит, это видно по данным.
    let addr = serve(dir.path(), "BTCUSDT").await;

    // (а) `subscribe` ВНУТРИ окна: env НЕ применяется, первым уходит снапшот запрошенного.
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("w", "ETHUSDT")).await;
    let first = recv(&mut ws).await.expect("первое сообщение");
    assert_eq!(
        sub_of(&first),
        Some("w"),
        "O-8(а): первым сообщением пришло НЕ то, что запросил клиент. §2.8 требует, чтобы сервер \
         молчал до конца grace-окна ({GRACE_MS} мс) или до первого клиентского сообщения — что \
         раньше. Иначе клиент получает «чужой» первый снапшот незаметно для себя: {first}"
    );
    let body = serde_json::to_string(&first).unwrap_or_default();
    assert!(
        !body.contains("BTCUSDT"),
        "O-8(а): в ответе на подписку ETHUSDT присутствует env-инструмент BTCUSDT — \
         конфигурация процесса победила клиентскую подписку, что и есть предмет M-65: {first}"
    );

    // (б) `subscribe` ПОСЛЕ окна — обычная смена инструмента (§2.4), а не игнор.
    let mut ws2 = connect(&addr).await;
    tokio::time::sleep(Duration::from_millis(GRACE_MS * 4)).await;
    let _legacy = drain(&mut ws2, 500).await;
    send(&mut ws2, subscribe("late", "ETHUSDT")).await;
    let after = drain(&mut ws2, 2_500).await;
    assert!(
        after.iter().any(|v| sub_of(v) == Some("late")),
        "O-8(б): `subscribe` после grace-окна не дал ничего по подписке «late». Он обязан \
         обрабатываться как обычная смена инструмента (§2.8, третий пункт), иначе клиент, \
         подключившийся раньше, чем решил, что показывать, остаётся заперт в env-селекторе \
         навсегда. Пришло: {after:?}"
    );
}

// ─── O-9: изоляция ДВУХ соединений (ось 7, `C-077` B-1) ─────────────────────────────────
#[tokio::test]
async fn o9_connections_are_isolated() {
    claims(
        "o9",
        7,
        "подписка другого соединения меняет выдачу текущего",
    );
    claims(
        "o9",
        7,
        "одинаковый sub id в двух соединениях делит состояние",
    );
    claims(
        "o9",
        7,
        "два соединения с одинаковым sub id и разными селекторами дают независимые потоки",
    );

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let sel_a = sel_of("BTCUSDT");
    let sel_b = sel_of("ETHUSDT");

    // ОДИНАКОВЫЙ `sub id` в двух соединениях — это РАЗНЫЕ подписки: `id` назначает клиент
    // (§2.2), и пространство идентификаторов у каждого соединения СВОЁ.
    let (mut a, snap_a) = connect_subscribed(&addr, "w", "BTCUSDT", &sel_a).await;
    let mut a_msgs = drain(&mut a, 1_500).await;
    let (mut b, snap_b) = connect_subscribed(&addr, "w", "ETHUSDT", &sel_b).await;
    assert_eq!(
        snap_b.cursor, snap_a.cursor,
        "O-9 SETUP: A и B должны стартовать с одного курсора до live-дозаписи; иначе негатив \
         может разойтись курсором, а не содержимым"
    );
    let mut b_msgs = drain(&mut b, 1_500).await;

    // Kill-фикстура растит ОБА инструмента. При дозаписи только A мутант crosstalk умирает
    // молчанием чужого селектора; это падение по неверной причине, а не проверка Р-Б.
    append_more(dir.path(), "BTCUSDT", 6, BASE_MS + 2_000);
    append_more(dir.path(), "ETHUSDT", 6, BASE_MS + 2_000);
    a_msgs.extend(drain(&mut a, 2_500).await);
    b_msgs.extend(drain(&mut b, 2_500).await);
    let a_frames = wire_frames(&a_msgs, "w");
    let b_frames = wire_frames(&b_msgs, "w");

    let acc_a = apply_frames(snap_a.clone(), &a_frames, "O-9 positive/A");
    let ref_a = reference_at(
        dir.path(),
        &sel_a,
        cursor_at_to(a_frames.last().expect("A frames not empty")),
    );
    assert_eq!(
        acc_a,
        ref_a,
        "O-9: кадры соединения A НЕ продолжают снапшот selector'а, который клиент A послал \
         в subscribe. Это Р-Б: snapshot(A) ⊕ frames(A) обязан бит-в-бит сходиться с \
         независимым gateway::snapshot(dir, filter, selector_A, Cursor::at(to)). Пришло: {:?}",
        a_msgs.iter().take(3).collect::<Vec<_>>()
    );
    let acc_b = apply_frames(snap_b, &b_frames, "O-9 positive/B");
    let ref_b = reference_at(
        dir.path(),
        &sel_b,
        cursor_at_to(b_frames.last().expect("B frames not empty")),
    );
    assert_eq!(
        acc_b, ref_b,
        "O-9: кадры соединения B НЕ продолжают снапшот selector'а, который клиент B послал \
         в subscribe. Изоляция соединений обязана доказываться с ОБЕИХ сторон, а не только \
         как «B не равно эталону A»."
    );

    // Негатив на ТОЙ ЖЕ фикстуре: если A получит кадры B, курсорная цепочка выглядит
    // законной, но содержимое обязано разойтись с эталоном selector'а A.
    assert_eq!(
        b_frames.first().map(|f| f.from),
        Some(snap_a.cursor),
        "O-9 SETUP: чужие кадры должны продолжать курсор A; иначе негатив ловит не Р-Б, а \
         разрыв цепочки курсоров"
    );
    let alien = apply_frames(snap_a, &b_frames, "O-9 negative/B-as-A");
    let ref_alien = reference_at(
        dir.path(),
        &sel_a,
        cursor_at_to(b_frames.last().expect("B frames not empty")),
    );
    assert_eq!(
        alien.cursor, ref_alien.cursor,
        "O-9 SETUP: негатив обязан расходиться содержимым при одинаковом курсоре"
    );
    assert_ne!(
        alien.series, ref_alien.series,
        "O-9 NEGATIVE: чужие кадры B, применённые к снапшоту A, сошлись с эталоном A. \
         Оракул снова слеп к cross-talk по содержимому."
    );
}

// ─── O-10: жизненный цикл подписки (ось 8, `C-077` B-2) ─────────────────────────────────
#[tokio::test]
async fn o10_unsubscribe_stops_sub_and_frees_capacity() {
    claims("o10", 8, "unsubscribe не прекращает поток");
    claims("o10", 8, "unsubscribe не освобождает место под лимитом");
    claims("o10", 8, "unsubscribe неизвестного id молчит");
    claims("o10", 8, "unsubscribe одной подписки не трогает соседнюю");
    claims("o10", 8, "место под лимитом освобождено и переиспользуемо");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let mut ws = connect(&addr).await;

    send(&mut ws, subscribe("gone", "BTCUSDT")).await;
    send(&mut ws, subscribe("stay", "ETHUSDT")).await;
    let _ = drain(&mut ws, 1_500).await;

    append_more(dir.path(), "BTCUSDT", 6, BASE_MS + 3_000);
    append_more(dir.path(), "ETHUSDT", 6, BASE_MS + 3_000);
    send(&mut ws, unsubscribe("gone")).await;
    let tail = drain(&mut ws, 2_500).await;
    assert!(
        !tail.iter().any(|v| sub_of(v) == Some("gone")),
        "O-10: после `unsubscribe(gone)` поток по этой подписке продолжился. Реализация, \
         разбирающая `unsubscribe` и игнорирующая его, проходит любой набор, знающий только \
         `subscribe`, — и нарушает инвариант §4.1: множество подписок клиента больше не \
         содержит этот id, а выдача содержит. Пришло: {tail:?}"
    );
    assert!(
        tail.iter().any(|v| sub_of(v) == Some("stay")),
        "O-10: `unsubscribe` одной подписки погасил СОСЕДНЮЮ — снятие одного виджета не вправе \
         гасить остальной экран (ось 5/8)"
    );

    // Повторный/неизвестный `unsubscribe` — `error` с кодом (решение спеки §4.2): «успех» на
    // снятии того, чего нет, сделал бы клиентскую бухгалтерию подписок недоказуемой.
    //
    // МЕСТО ПРОВЕРКИ ИСПРАВЛЕНО (`R-057` заявка dev'а, принята architect'ом 13.08). Она стояла
    // В КОНЦЕ теста — ПОСЛЕ переподписки тем же id. Но там «gone» уже ЖИВАЯ подписка, и её
    // снятие обязано пройти УСПЕШНО: оракул требовал двух взаимоисключающих вещей разом
    // («переподписка работает полноценно» И «её снятие даёт ошибку») и был неудовлетворим ни
    // одной корректной реализацией. Противоречие было не в требованиях — оба нужны, — а в
    // порядке: «повторное снятие» проверяется ТАМ, ГДЕ подписка действительно снята.
    send(&mut ws, unsubscribe("gone")).await;
    let repeat = drain(&mut ws, 1_500).await;
    assert!(
        repeat.iter().any(|v| type_of(v) == Some("error")),
        "O-10: повторный `unsubscribe` уже снятой подписки прошёл молча. Решение спеки §4.2 — \
         `error` с кодом: клиент обязан отличать «снял» от «не было», иначе его учёт подписок \
         недоказуем. Пришло: {repeat:?}"
    );

    // Место под лимитом обязано освободиться и быть ПЕРЕИСПОЛЬЗУЕМЫМ: без этого проверки
    // «поток прекратился» достаточно для мутанта `capleak`, а лимит 16 деградирует до
    // «16 подписок за всё время жизни соединения».
    for k in 0..(MAX_SUBS - 2) {
        send(&mut ws, subscribe(&format!("f{k}"), "BTCUSDT")).await;
    }
    let filled = drain(&mut ws, 2_500).await;
    assert!(
        !filled.iter().any(|v| type_of(v) == Some("error")),
        "O-10: заполнение до лимита после `unsubscribe` дало отказ — место снятой подписки НЕ \
         освобождено. Отказы: {:?}",
        filled
            .iter()
            .filter(|v| type_of(v) == Some("error"))
            .collect::<Vec<_>>()
    );

    // Снятый id обязан ПОДПИСЫВАТЬСЯ ПОВТОРНО. Без этого O-10 проверял освобождение места
    // ЧУЖИМИ идентификаторами и пропускал «вечное надгробие» (`R-057` Б-2): помеченный id
    // глушит любую будущую подписку с тем же именем, а клиент назначает id сам (§2.2) и
    // законно переиспользует их при перерисовке виджета.
    claims("o10", 8, "снятый id не подписывается повторно");
    send(&mut ws, subscribe("gone", "BTCUSDT")).await;
    let _ = drain(&mut ws, 1_000).await;
    append_more(dir.path(), "BTCUSDT", 6, BASE_MS + 20_000);
    let revived = drain(&mut ws, 4 * GRACE_MS + 2_000).await;
    assert!(
        revived
            .iter()
            .any(|v| sub_of(v) == Some("gone") && type_of(v) == Some("frame") && has_content(v)),
        "O-10: подписка с ранее снятым id «gone» не получила кадров с содержимым — id отравлен \
         до конца соединения. Клиент назначает идентификаторы сам и переиспользует их; \
         одноразовый id означает, что перерисовка виджета молча перестаёт работать. Пришло: {:?}",
        revived.iter().filter_map(sub_of).collect::<Vec<_>>()
    );
}

// ─── O-11: происхождение содержимого кадра (ось 9, `R-057` Б-3) ─────────────────────────
#[tokio::test]
async fn o11_frames_come_from_journal_not_synthesis() {
    claims("o11", 9, "кадр синтезирован сервером, а не журналом");
    claims("o11", 9, "кадр не отличим клиентом от настоящего");
    claims("o11", 9, "кадр отражает события журнала");
    claims("o11", 9, "молчание при отсутствии событий");

    let dir = tempfile::tempdir().expect("tempdir");
    seed(dir.path());
    let addr = serve(dir.path(), "BTCUSDT").await;
    let mut ws = connect(&addr).await;

    send(&mut ws, subscribe("w", "BTCUSDT")).await;
    let _snap = recv(&mut ws).await.expect("snapshot подписки");
    let _ = drain(&mut ws, 1_000).await; // добираем засеянный бэклог

    // (1) МОЛЧАНИЕ — законное поведение: журнал не растёт, отдавать нечего.
    let quiet = drain(&mut ws, 3 * GRACE_MS + 1_200).await;
    let fabricated: Vec<&Value> = quiet
        .iter()
        .filter(|v| type_of(v) == Some("frame") && !has_content(v))
        .collect();
    assert!(
        fabricated.is_empty(),
        "O-11: сервер прислал {} кадр(ов) с ПУСТЫМ содержимым, хотя в журнале не произошло \
         ничего. Кадр, синтезированный сервером, клиент не отличит от настоящего: у него нет \
         ни одного признака, и он перерисует виджет по пустоте. Цена не только семантическая \
         — 311 байт × 4/с × 10 000 соединений ≈ 100 Мбит/с чистой пустоты, нижняя граница \
         всего бюджета egress по DESIGN §16 на РЕАЛЬНЫЕ данные со сжатием. Примеры: {:?}",
        fabricated.len(),
        fabricated.iter().take(2).collect::<Vec<_>>()
    );

    // (2) Появились события — обязан прийти кадр, и его содержимое обязано их отражать.
    let before = reference_bars(dir.path(), "BTCUSDT");
    append_more(dir.path(), "BTCUSDT", 8, BASE_MS + 10_000);
    let after = reference_bars(dir.path(), "BTCUSDT");
    assert!(
        after > before,
        "SETUP НЕ СОСТОЯЛСЯ: независимый эталон не вырос ({before} → {after}) — дозапись не \
         дошла до журнала, и проверять происхождение кадра не на чем"
    );

    let live = drain(&mut ws, 4 * GRACE_MS + 1_500).await;
    let real: Vec<&Value> = live
        .iter()
        .filter(|v| type_of(v) == Some("frame") && sub_of(v) == Some("w") && has_content(v))
        .collect();
    assert!(
        !real.is_empty(),
        "O-11: журнал вырос ({before} → {after} баров по НЕЗАВИСИМОМУ эталону \
         gateway::snapshot), а подписка не получила ни одного кадра с содержимым. Пришло: {:?}",
        live.iter().take(3).collect::<Vec<_>>()
    );
}
