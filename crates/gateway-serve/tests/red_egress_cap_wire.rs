//! RED `PL-I-5` УРОВЕНЬ 2 (sacred, architect-only) — **ПРЕДЕЛ СУДИТ БАЙТЫ, ПРИШЕДШИЕ В СОКЕТ.**
//!
//! Милестоун `milestones/M-71-egress-cap.md` rev5. Исполнение второго арбитража по предмету —
//! `research/arbitration/A-022-m71-oracle-judges-its-own-construction.md`.
//!
//! # Почему файл переписан ЦЕЛИКОМ, а не дополнен
//!
//! Четыре круга гейта нашли ОДИН корень в трёх из них (`A-022` Вопрос 1): **величина в
//! ассерте ВЫЧИСЛЯЛАСЬ самим тестом, а не СНИМАЛАСЬ с исполнения прод-границы.**
//!
//! * `C-157` R1 — судил `heatmap.len()`: тест сам выбрал, что считать ресурсом, и выбрал
//!   подмножество; провод такой величины не знает;
//! * `C-158` R1 — судил `serde_json::to_vec(&Snapshot)`: тест сам сериализовал, вместо
//!   байтов, которые кладёт на провод сервер;
//! * `C-161` — строил текст ошибки локальным `describe()`, дублируя трансформацию
//!   обработчика. Честная реализация, выбравшая разрешённое лечение «не echo'ить venue»,
//!   оставила бы оракул красным. Мутация обработчика показание не сдвигала НИКОГДА.
//!
//! Правило границы `A-020` запрещает пятый экземпляр покрытия. Смена конструкции:
//! **судимая величина — длина `Message`, ПОЛУЧЕННОГО реальным клиентом через реальный
//! сокет.** Объемлющего множества ниже сокета на стороне сервера нет; строить объект самому
//! здесь больше нечего, и класс закрыт ПО ПОСТРОЕНИЮ, а не ещё одной проверкой.
//!
//! # Почему сокет достижим (и почему приватная функция — нет)
//!
//! `handle_v1_message` (`lib.rs:700`) и `send_v1_error` (`lib.rs:1020`) — приватные `async
//! fn`. Публичная обёртка `parse_and_dispatch_v1_message` (`:320`) требует `&mut
//! SessionInner`, а его поля приватны: конструирование только внутри крейта. Открывать
//! конструктор ради теста значило бы судить полусобранную сессию — полушаг того же класса,
//! и арбитр это отверг.
//!
//! Зато шесть наборов крейта уже поднимают НАСТОЯЩИЙ сервер (`bind("127.0.0.1:0")` →
//! `local_addr()` → `spawn(serve())` → `connect_async`): `smoke_ws`, `red_ws_protocol`,
//! `red_ws_session`, `red_ws_series_vs_replay`, `red_ws_honesty_sessions`,
//! `red_ws_liveness_under_load`. Прод-граница достижима тем способом, каким её дёргает
//! клиент, — это лучше вызова приватной функции, а не хуже.
//!
//! # Цена размена названа (`A-022` Вопрос 2в)
//!
//! Сокетные тесты медленнее и уязвимее к таймингам. Но: судимая величина — ДЛИНА
//! полученного сообщения, от хоста не зависит; тайминговых ассертов здесь нет вовсе;
//! срабатывание таймаута оформляется как **НЕСОСТОЯВШИЙСЯ SETUP** с отдельной диагностикой,
//! а не как вердикт о пределе. Риск флака спекулятивен, цена реконструкции — ИЗМЕРЕНА
//! четырьмя кругами гейта.
//!
//! # ТАБЛИЦА «ДВЕРЬ → СЦЕНАРИЙ» (сверяется пробой `scripts/tests/red_egress_doors.sh`)
//!
//! | дверь исходящего текста | сценарий здесь |
//! |---|---|
//! | `wire_v1::snapshot_msg` | `W1` — v1-снапшот после `subscribe` |
//! | `wire_v1::frame_msg`    | `W2` — v1-кадр после дописи в журнал |
//! | `ServeMsg` (legacy)     | `W3` — снапшот по истечении grace-окна без `subscribe` |
//! | `wire_v1::error_msg`    | `W4` — текст ошибки на гигантском `venue` · `W-C3` — честная ошибка |
//! | `serve::snapshot_msg`   | `W5` — прокидывание отказа уровня 1 (объект: вердикт обёртки) |
//! | `serve::frames_msgs`    | `W5` — то же |
//!
//! **Предел таблицы назван честно:** присутствие имени в ней ≠ истинность сопоставления.
//! Зубы этому даёт МУТАЦИЯ НАБЛЮДАЕМОСТИ (`A-022` Вопрос 3), которую architect обязан
//! прогнать ДО запроса критика и предъявить таблицей чувствительности в Done Block.
//!
//! # Что здесь НЕ судится
//!
//! Форма лечения не выбирается: усечь эхо, не echo'ить, ограничить поле при разборе —
//! решает реализация. Требование одно: **наружу не уходит сообщение сверх предела**.

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

/// Предложенная величина предела (спека §5.1). Она **founder-owned**; оракулы судят
/// ПОВЕДЕНИЕ (сообщение сверх предела наружу не уходит), а число здесь — рабочая отсечка,
/// разведённая с фикстурами на порядки, чтобы набор не зависел от её точного значения.
const PROPOSED_CAP: usize = 2_000_000;

/// Плотный НЕ-heatmap ресурс: 25 000 сделок, ни одного L2-события. Именно он предъявлен
/// критиком как 2 804 666 Б на формах, которых прежние оракулы не покрывали.
const DENSE_TRADES: usize = 25_000;

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

/// Подписка, ГАРАНТИРОВАННО попавшая в grace-окно, и первый v1-снапшот по ней.
///
/// Повтор обязателен: `subscribe`, опоздавший в grace-окно, сервер игнорирует и уходит в
/// env-режим (`CT-RFC-09` §2.8). Без повтора оракул молча не получал бы v1-сообщений и был
/// бы ЗЕЛЁН ПО ОТСУТСТВИЮ ПРЕДМЕТА — ровно тот класс, который закрывает `A-022`. Тот же
/// приём применён в `red_ws_session.rs:383-405`.
async fn subscribed_snapshot(addr: &str, venue: &str) -> (Ws, usize, Value) {
    let mut last: Option<Value> = None;
    for attempt in 1..=8u64 {
        let mut ws = connect(addr).await;
        send(&mut ws, subscribe(SHORT_SUB, venue)).await;
        match recv(&mut ws).await {
            Some((n, v)) if type_of(&v) == Some("snapshot") => return (ws, n, v),
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

fn type_of(v: &Value) -> Option<&str> {
    v.get("type").and_then(|t| t.as_str())
}

/// Диагностика несостоявшегося setup'а — отдельно от вердикта о пределе
/// (`testing.md`, «Целостность гейта» свойство 3).
fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о пределе: сервер не прислал ожидаемого \
         сообщения в пределах бюджета {BUDGET:?}. Оракул судит длину ПОЛУЧЕННОГО сообщения; \
         если получать нечего — судить нечего, и молчать об этом нельзя."
    )
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// КОНТРОЛИ — первыми: страж, ломающий честную работу, выключат, и защиты не станет
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **W-C1 — честная нагрузка проходит через сокет.**
#[tokio::test]
async fn pl_i_5_w_c1_prod_default_is_served_over_the_socket() {
    let dir = journal_of_trades(200);
    let addr = serve_on(dir.path()).await;
    let (_ws, n, _v) = subscribed_snapshot(&addr, "Binance").await;
    assert!(
        n < 200_000,
        "PL-I-5 W-C1: обычный ответ весит {n} Б — фикстура не разводит честный и плотный \
         случаи на порядки, и оракулы ниже начинают зависеть от точной величины предела"
    );
}

/// **W-C3 — честная ошибка ДОСТАВЛЯЕТСЯ, с кодом и причиной** (парный vantage к `W4`).
///
/// Лечение `W4` не смеет выродиться в «ошибок не отдаём»: клиент обязан узнать, что чинить.
/// Прецедент требования — `GW-I-14` («отказ обязан НАЗЫВАТЬ переменную»).
#[tokio::test]
async fn pl_i_5_w_c3_honest_error_is_delivered_over_the_socket() {
    let dir = journal_of_trades(50);
    let addr = serve_on(dir.path()).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe(SHORT_SUB, "NoSuchVenue")).await;

    let mut err = None;
    for _ in 0..4 {
        match recv(&mut ws).await {
            Some((n, v)) if type_of(&v) == Some("error") => {
                err = Some((n, v));
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    let Some((n, v)) = err else {
        setup_failed("сообщение об ошибке на неизвестную площадку не пришло")
    };
    assert_eq!(
        v.get("code").and_then(|c| c.as_str()),
        Some("unknown_venue"),
        "PL-I-5 W-C3: ошибка обязана нести машиночитаемый код (`CT-RFC-09` §2.7); получено: {v}"
    );
    assert!(
        n > 20 && n < 4_096,
        "PL-I-5 W-C3: честная ошибка весит {n} Б — обязана быть и непустой, и небольшой"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// ПРЕДМЕТ — судятся БАЙТЫ, ПРИШЕДШИЕ КЛИЕНТУ
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **W1 — v1-СНАПШОТ: КОНТРОЛЬ, и почему именно контроль.**
///
/// # Замер, из-за которого оракул объявлен контролем, а не предметом
///
/// Прогон на плотной фикстуре (25 000 сделок): **v1-снапшот с сокета = 425 Б**. Плотный
/// ресурс на этот путь НЕ ПОПАДАЕТ: подписка v1 начинает от текущей позиции, а не от начала
/// журнала. Требовать здесь красноты значило бы завести ЛОЖНУЮ ТРЕВОГУ — тот самый класс,
/// которым набор уже болел дважды (`C-159` R2, `C-161`).
///
/// Оракул остаётся, и не для симметрии: он держит границу СЕГОДНЯШНЕГО поведения. Реализация
/// предела, которая заодно изменит стартовую позицию v1-подписки, обрушит сюда весь журнал —
/// и покраснеет здесь. Отсутствие снапшота — тоже отказ (`subscribed_snapshot` объявит
/// несостоявшийся setup), поэтому зелёным по молчанию он быть не может.
#[tokio::test]
async fn pl_i_5_w1_v1_snapshot_over_cap_is_not_delivered() {
    let dir = journal_of_trades(DENSE_TRADES);
    let addr = serve_on(dir.path()).await;
    // Отсутствие v1-снапшота — НЕ зелёный исход: `subscribed_snapshot` объявит несостоявшийся
    // setup. Зелёным этот оракул станет только когда снапшот ПРИДЁТ и уложится в отсечку,
    // либо когда сервер откажет ЯВНО (ветка ниже недостижима — helper паникует раньше).
    let (_ws, n, _v) = subscribed_snapshot(&addr, "Binance").await;
    assert!(
        n <= PROPOSED_CAP,
        "PL-I-5 W1 НАРУШЕН: клиенту ПРИШЁЛ v1-снапшот {n} Б при отсечке {PROPOSED_CAP}. Это \
         байты, снятые С СОКЕТА, а не собранные тестом. Селектор прод-дефолтный, heatmap пуст \
         — объём принесли сделки; злоупотребления шириной полосы не требуется."
    );
}

/// **W2 — v1-КАДР: КОНТРОЛЬ по тому же основанию.**
///
/// Замер: после дописи 25 000 сделок **крупнейший v1-кадр с сокета = 29 454 Б**. Размер
/// кадра ограничен ПАКЕТИРОВАНИЕМ push-цикла, а не пределом: ресурс приходит размазанным по
/// многим кадрам. Значит одиночный кадр сверх предела сегодня не воспроизводится, и красный
/// ассерт здесь был бы ложной тревогой.
///
/// Контроль сторожит именно это свойство: реализация, увеличившая пакет или собравшая кадр
/// целиком, покраснеет здесь. Отсутствие кадра — несостоявшийся setup, не зелёный исход.
#[tokio::test]
async fn pl_i_5_w2_v1_frame_over_cap_is_not_delivered() {
    let dir = journal_of_trades(200);
    let addr = serve_on(dir.path()).await;
    let (mut ws, _n, _v) = subscribed_snapshot(&addr, "Binance").await;

    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 1_000..(1_000 + DENSE_TRADES as i64) {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    while let Some((n, v)) = recv(&mut ws).await {
        if type_of(&v) == Some("frame") {
            assert!(
                n <= PROPOSED_CAP,
                "PL-I-5 W2 НАРУШЕН: клиенту ПРИШЁЛ v1-кадр {n} Б при отсечке {PROPOSED_CAP}. \
                 Предел, поставленный только на снапшот, оставляет открытым путь основного \
                 трафика."
            );
            return;
        }
    }
    // Кадра не было вовсе — предмет не воспроизведён, и молчать об этом нельзя.
    setup_failed("v1-кадр после дописи в журнал не пришёл")
}

/// **W3 — LEGACY-форма: та же отсечка.** Две половины в ОДНОМ тесте, и это конструкция,
/// а не удобство.
///
/// Клиент, не приславший `subscribe`, по истечении grace-окна получает снапшот по
/// env-селектору в legacy-конверте (`CT-RFC-09` §2.8). Другая форма — тот же предел.
///
/// # ПЕРЕПИСАН 2026-08-27 — прежняя редакция была НЕВЫПОЛНИМА под действующей спекой
///
/// Она ждала, что сверхлимитный legacy-снапшот ПРИДЁТ, и валила тест, если его размер
/// превышал отсечку. Это форма «усечь и отдать», которую §4bis.1 СНЯЛА (`R-133` B-2/B-3):
/// сегодня `snapshot_checked` возвращает `Err`, сессия не отправляет ничего, `recv` даёт
/// `None`, цикл выходит — и `setup_failed` срабатывал на КОРРЕКТНОЙ реализации. Поймано
/// `SCOPE VIOLATION REQUEST` от engine-dev; тест sacred, правит architect.
///
/// # Почему НЕ «принять молчание как валидный исход» — развязка, которую пришлось отклонить
///
/// Это первое, что просится, и это плацебо. `recv` возвращает `None` И на close, И на
/// таймаут (`:206-217`, ветка `_ => None`) — «корректно отказал» и «завис» неразличимы.
/// Оракул, зелёный от молчания, зелен и против сервера, который не шлёт НИЧЕГО, включая
/// сломанный. `testing.md` §«Целостность гейта» св. 3: проба, молча тестирующая не тот
/// сценарий, есть плацебо самой себя.
///
/// Отдельно замерено: парного vantage на legacy-пути в наборе НЕТ. `W-C1` идёт `v1`-путём
/// через `subscribed_snapshot`, то есть legacy-конверт не пиннит ни один зелёный оракул.
/// Ослабить W3 до «молчание — ок» значило бы оставить эту форму без покрытия вовсе.
///
/// # Конструкция: vantage СНАЧАЛА, предмет ПОТОМ
///
/// 1. **Vantage (setup, различающий).** Тот же legacy-путь на МАЛОМ журнале обязан отдать
///    снапшот. Половина отвечает на вопрос «а доходит ли вообще до этой формы» и краснеет
///    против мёртвого/зависшего сервера, против сломанного grace-окна и против фикстуры,
///    не попадающей в legacy-режим.
/// 2. **Предмет.** Тот же путь на ПЛОТНОМ журнале не смеет отдать снапшот сверх отсечки.
///    Молчание здесь — легитимный fail-closed, и оно больше не вакуумно: половина 1 уже
///    доказала, что путь живой.
///
/// Форму отказа оракул НЕ выбирает (тот же принцип, что в шапке файла): молча закрыть,
/// прислать ошибку — решает реализация. Требование одно: **наружу не уходит legacy-снапшот
/// сверх предела, и при этом форма не мертва.**
#[tokio::test]
async fn pl_i_5_w3_legacy_snapshot_over_cap_is_not_delivered() {
    // ── Половина 1: VANTAGE. Legacy-путь на малом журнале ОБЯЗАН отдать снапшот. ──────
    let small = journal_of_trades(200);
    let small_addr = serve_on(small.path()).await;
    let mut small_ws = connect(&small_addr).await;
    // намеренно НЕ подписываемся — ждём env-снапшот по истечении grace-окна
    let mut vantage: Option<usize> = None;
    while let Some((n, v)) = recv(&mut small_ws).await {
        if v.get("Snapshot").is_some() || type_of(&v) == Some("snapshot") {
            vantage = Some(n);
            break;
        }
    }
    let vantage = vantage.unwrap_or_else(|| {
        setup_failed(
            "VANTAGE: legacy-снапшот НЕ пришёл даже на малом журнале. Судить отсечку не по \
             чему: молчание на плотном журнале было бы неотличимо от мёртвого legacy-пути",
        )
    });
    assert!(
        vantage <= PROPOSED_CAP,
        "PL-I-5 W3 VANTAGE: честный legacy-снапшот весит {vantage} Б при отсечке \
         {PROPOSED_CAP} — фикстура не разводит честный и плотный случаи, и предмет ниже \
         судил бы не то"
    );

    // ── Половина 2: ПРЕДМЕТ. Тот же путь на плотном журнале — сверх отсечки не отдаёт. ──
    let dense = journal_of_trades(DENSE_TRADES);
    let dense_addr = serve_on(dense.path()).await;
    let mut dense_ws = connect(&dense_addr).await;
    while let Some((n, v)) = recv(&mut dense_ws).await {
        if v.get("Snapshot").is_some() || type_of(&v) == Some("snapshot") {
            assert!(
                n <= PROPOSED_CAP,
                "PL-I-5 W3 НАРУШЕН: клиенту ПРИШЁЛ legacy-снапшот {n} Б при отсечке \
                 {PROPOSED_CAP}. Форм две, предел обязан быть один. (Vantage той же формы \
                 на малом журнале дал {vantage} Б — путь заведомо живой, и это не setup.)"
            );
            return;
        }
    }
    // Молчание ЗДЕСЬ — легитимный fail-closed (§4bis.1: «при отказе клиент знает, что не
    // получил ничего»), и оно не вакуумно: половина 1 предъявила работающий legacy-путь.
}

/// **W4 — ТЕКСТ ОШИБКИ, УПРАВЛЯЕМЫЙ КЛИЕНТОМ** (`C-161` F-161-1, `A-022`).
///
/// Прежняя редакция строила этот текст САМА, дублируя трансформацию обработчика, — и потому
/// осталась бы красной против честного лечения. Теперь судятся байты, ПРИШЕДШИЕ клиенту:
/// мутация обработчика показание сдвигает, реконструкции в тесте нет.
///
/// `sub` — два байта намеренно: это НЕ именованный остаток «длина `sub`-id не ограничена»
/// (`A-021`), а отдельный класс — ошибка echo'ит поле ЗАПРОСА.
#[tokio::test]
async fn pl_i_5_w4_client_controlled_error_text_is_capped() {
    let dir = journal_of_trades(50);
    let addr = serve_on(dir.path()).await;
    let mut ws = connect(&addr).await;
    let huge_venue = "V".repeat(2_100_000);
    send(&mut ws, subscribe(SHORT_SUB, &huge_venue)).await;

    while let Some((n, v)) = recv(&mut ws).await {
        if type_of(&v) == Some("error") {
            assert!(
                n <= PROPOSED_CAP,
                "PL-I-5 W4 НАРУШЕН: клиенту ПРИШЁЛ текст ошибки {n} Б при `sub` из {} символов. \
                 Почти весь объём принесён полем ЗАПРОСА, которое сообщение echo'ит целиком. \
                 Форму лечения оракул не выбирает — усечь эхо, не echo'ить, ограничить поле при \
                 разборе; требование одно: наружу не уходит сообщение сверх предела.",
                SHORT_SUB.len()
            );
            return;
        }
    }
    setup_failed("сообщение об ошибке на гигантскую площадку не пришло")
}

/// **W5 — serve-обёртки ПРОКИДЫВАЮТ отказ уровня 1.**
///
/// Объект этого оракула назван честно и он ДРУГОЙ: не «исходящий текст», а ВЕРДИКТ обёртки.
/// Прод-сервер эти функции не зовёт (их зовут тесты), поэтому судить их байтами против
/// предела было бы той же реконструкцией. Судится одно: `Err` уровня 1 не теряется.
#[tokio::test]
async fn pl_i_5_w5_serve_wrappers_propagate_level1_refusal() {
    let dir = journal_of_trades(DENSE_TRADES);
    let s = sel();

    let snap = gateway_serve::serve::snapshot_msg(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        gateway::Cursor::LATEST,
        None,
    );
    assert!(
        snap.is_err(),
        "PL-I-5 W5: `serve::snapshot_msg` вернул Ok на ресурсе, который уровень 1 обязан \
         отвергнуть. Обёртка объявлена ТОНКИМ passthrough (`GS-I-5`) — глотать отказ она не \
         вправе."
    );

    let frames = gateway_serve::serve::frames_msgs(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        gateway::Cursor::START,
        usize::MAX,
    );
    assert!(
        frames.is_err(),
        "PL-I-5 W5: `serve::frames_msgs` вернул Ok на том же ресурсе — отказ уровня 1 потерян"
    );
}
