//! RED (sacred, architect-only) — **bump `GATEWAY_SCHEMA_VERSION` является ГЕЙТОМ.**
//!
//! **Файл намеренно версионно-АГНОСТИЧЕН по имени (M-48, C-032 R1).** Раньше он назывался
//! `red_gateway_schema_v7.rs` и пиннил 7. M-48 поднимает версию до 8 — и оракул, прибитый к
//! прошлой версии, превратился в блокер: engine-dev не мог провести bump, не тронув sacred-тест.
//! Это МОЙ процессный промах, второй раз подряд того же класса (reviewer предупреждал на M-38b:
//! «смена публичной сигнатуры без адаптации call-site'ов в том же RED-коммите вынуждает dev'а
//! править sacred-тесты»). Правило: при смене контракта architect обновляет ВСЕ свои оракулы и
//! verify-скрипты в ТОМ ЖЕ коммите, а имя файла не привязывается к номеру версии.
//!
//! C-028 K1: `red_gateway_export_v2` проверяет только `snap.schema_version == GATEWAY_SCHEMA_VERSION`
//! (тавтология — зелёная при ЛЮБОМ значении константы). Она НЕ доказывает, что M-38a поднял версию
//! до нормы своего милестоуна. engine-dev мог бы реализовать поведение и оставить публичную
//! схему на прежней версии — named-гейт задачи остался бы зелёным. Здесь версия ПРИБИТА к
//! ДЕЙСТВУЮЩЕЙ НОРМЕ явно, в трёх местах:
//!   (1) сама константа `GATEWAY_SCHEMA_VERSION == EXPECTED_SCHEMA_VERSION`;
//!   (2) `Snapshot.schema_version` (то, что видит консюмер envelope через snapshot);
//!   (3) `Frame.schema_version` из `frames_since` (live-push путь).
//!
//! **ДЕЙСТВУЮЩЕЕ значение объявлено в `EXPECTED_SCHEMA_VERSION` и больше нигде не
//! утверждается в настоящем времени.** Проза этого файла говорит о версиях только в двух
//! законных формах: (1) исполняемое сравнение — три ассерта ниже и шаг гейта, где расхождение
//! КРИЧИТ падением и молча протухнуть не может; (2) событийно-заякоренная история —
//! «M-38a: 6→7», «M-48: 7→8», «M-68: 8→9», транскрипты прошлых падений. История истинна
//! навсегда и дубликатом нормы не является (`reading-map.md` §3: снятые редакции в этом
//! корпусе сохраняются НАМЕРЕННО).
//!
//! **Тест на протухание, по которому это различается** (`A-023` §1.3): может ли предложение
//! стать ложным от будущего bump'а БЕЗ правки самого предложения и МОЛЧА? «Прибит к 7» —
//! может, и запрещено. «M-48: 7→8» — не может, и остаётся.
//!
//! Прежняя редакция шапки заявляла, что значение нормы в прозе файла отсутствует вовсе, — и
//! это опровергалось грепом по ЭТОМУ ЖЕ файлу в момент коммита (`A-023` §1.1 п.3). Дефект
//! был не в исторических числах, а в самоописании, обещавшем больше, чем показывает команда.
//! Формулировки той редакции здесь намеренно НЕ воспроизводятся даже как цитата: закрывающие
//! команды `A-023` §4.3 ищут их буквально, и цитата ломала бы проверку, ради которой они
//! написаны.
//!
//! Анти-плацебо: RUNTIME-RED против текущего кода — все три assert'а падают по ЗНАЧЕНИЮ.
//! GREEN только после bump'а. Форма v1-аддитивности и провенанс остаются в
//! `red_gateway_export_v2` как отдельная регрессия (они НЕ доказывают non-additive bump).
//!
//! ---
//!
//! ## M-68 (2026-08-25): 8 → 9, и это ТРЕТЬЕ срабатывание класса, о котором предупреждает шапка
//!
//! Правило записано выше, в этом самом файле: «при смене контракта architect обновляет ВСЕ свои
//! оракулы и verify-скрипты в ТОМ ЖЕ коммите». M-68 rev3 завёл задачу 9 (bump 8→9 — смена
//! СЕМАНТИКИ депт-серии, `П-014` п.3), и я этот файл не тронул. Честная реализация задачи 9
//! неизбежно роняла `cargo test --all` на трёх ассертах ниже (`left: 9, right: 8`), а `cargo
//! test --all` — первый шаг `scripts/verify_M-68.sh`. То есть acceptance и задача 9 были
//! несовместимы уже на committed-форме: dev получил бы набор, который нельзя сделать зелёным.
//!
//! Нашёл круг 4 критика (`research/critiques/C-160-M-68-round4.md` F1). Шапка предупреждала о
//! ВТОРОМ разе; это ТРЕТИЙ, и корень тот же, что у «правила предшественника»
//! (`reading-map.md` §2): ответ лежал в корпусе готовым, я его не открыл. Оставляю след, а не
//! молчаливую правку константы — иначе четвёртый раз придёт тем же путём.
//!
//! **Что здесь по-прежнему НЕ проверяется** (граница предмета, не пробел): что версия
//! поднята ПО ПРАВИЛЬНОЙ причине. Оракул пиннит ЧИСЛО в трёх публичных путях; обоснование
//! bump'а — предмет спеки милестоуна и вердикта критика, механизма у него нет.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector, GATEWAY_SCHEMA_VERSION};
use journal::{EpochFilter, Journal, WriterConfig};

/// Текущая версия контракта провода. M-38a: 6→7 (session-reset CVD). M-48: 7→8 —
/// `history_start_seq` + `history_truncated` (VB-I-11). **M-68: 8→9** — смена СЕМАНТИКИ
/// депт-серии: была «глубина на момент последнего снимка», стала «глубина на момент
/// последнего события». Форма `DepthRow` не меняется, меняется СМЫСЛ чисел — тот же класс,
/// что M-36 (VWAP 5→6). Bump здесь ЕДИНСТВЕННЫЙ рычаг, отвергающий чекпоинт со старым
/// смыслом (`read_and_validate` шаг 3, `crates/gateway/src/lib.rs:2901-2904`).
const EXPECTED_SCHEMA_VERSION: u32 = 9;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(price: f64, size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
    }
}

/// (1) Сама константа обязана быть ровно `EXPECTED_SCHEMA_VERSION`.
#[test]
fn schema_version_constant_matches_expected() {
    assert_eq!(
        GATEWAY_SCHEMA_VERSION, EXPECTED_SCHEMA_VERSION,
        "GATEWAY_SCHEMA_VERSION обязан быть {EXPECTED_SCHEMA_VERSION} (M-68: смена СЕМАНТИКИ \
         депт-серии — была «глубина на момент последнего снимка», стала «на момент последнего \
         события»; `П-014` п.3, прецедент VB-I-6. Полная история значений — чейнджлог \
         константы). Текущее {GATEWAY_SCHEMA_VERSION} → bump не сделан"
    );
}

/// (2) Snapshot несёт `EXPECTED_SCHEMA_VERSION` (то, что уходит консюмеру в envelope через snapshot).
#[test]
fn snapshot_carries_expected_schema_version() {
    let t0 = 20_278_i64 * 86_400_000;
    let dir = journal_of(vec![
        trade(100.0, 3.0, Side::Buy, t0 + 1_000),
        trade(100.0, 2.0, Side::Sell, t0 + 2_000),
    ]);
    let snap = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("snapshot");
    assert_eq!(
        snap.schema_version, EXPECTED_SCHEMA_VERSION,
        "Snapshot.schema_version обязан быть {EXPECTED_SCHEMA_VERSION}, а не {}",
        snap.schema_version
    );
}

/// (3) Frame из `frames_since` несёт `EXPECTED_SCHEMA_VERSION` (live-push путь). frames обязаны
/// быть НЕПУСТЫ, иначе `all(==…)` вырождается в vacuous-true (анти-плацебо: проверяем, что
/// кадры реально есть).
#[test]
fn frame_carries_expected_schema_version() {
    let t0 = 20_278_i64 * 86_400_000;
    let dir = journal_of(vec![
        trade(100.0, 3.0, Side::Buy, t0 + 1_000),
        trade(100.0, 2.0, Side::Sell, t0 + 2_000),
        trade(100.0, 1.0, Side::Buy, t0 + 3_000),
    ]);
    let (frames, _next) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::START,
        usize::MAX,
    )
    .expect("frames_since");
    assert!(
        !frames.is_empty(),
        "предусловие: frames_since(START..) обязан вернуть ≥1 кадр (иначе all(==норме) vacuous)"
    );
    assert!(
        frames
            .iter()
            .all(|f| f.schema_version == EXPECTED_SCHEMA_VERSION),
        "каждый Frame.schema_version обязан быть {EXPECTED_SCHEMA_VERSION}; получено: {:?}",
        frames.iter().map(|f| f.schema_version).collect::<Vec<_>>()
    );
}
