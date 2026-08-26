//! RED `MD-I-8` `d12`/`d13` (sacred, architect-only) — **КАДЕНЦИЯ ЗАДАЁТСЯ НА СЕРИЮ И
//! ОБЪЯВЛЯЕТСЯ ПОТРЕБИТЕЛЮ.**
//!
//! Милестоун `milestones/M-68-depth-from-book.md` rev5, задачи 15 и 16. Исполнение решения
//! founder'а 2026-08-26 (§0sexies): «показатели на разных глубинах — не чаще секунды, для
//! исторического анализа достаточно раз в минуту; но для аналога букмап нужен хитмап,
//! обновляющийся с каждым тиком».
//!
//! # COMPILE-RED, отдельным файлом — НАМЕРЕННО
//!
//! Оба оракула ссылаются на форму, которой ещё нет: `Selector::depth_cadence_ms` и
//! `SeriesBundle::cadence_ms`. Оставленные в общем наборе, они ронят КОМПИЛЯЦИЮ соседей, и
//! `d1..d10` нельзя было бы предъявить красными: «не собралось» и «упало на ассерте» — разные
//! вещи, а RED-first требует второго. Тот же приём и по той же причине, что в
//! `red_egress_cap_boundary.rs` и `red_depth_recompute_cost.rs`.
//!
//! # Почему каденция — поле `Selector`, а не глобальная настройка процесса
//!
//! 1. Частота серии есть свойство ЗАПРОШЕННОГО ВИДА, а не процесса: два клиента вправе
//!    смотреть один инструмент с разной зернистостью (`DESIGN` §16: разрешение — тарифная
//!    ручка, «Free получает 1-секундные кадры, не 100 мс»).
//! 2. Процессный глобал невидим системе типов — ровно так `M-71` `B-1` и прожил мимо
//!    зелёного гейта (`R-133`: ручка разобрана, читателей ноль).
//! 3. Каденция ВХОДИТ в `selector_fingerprint`. Это не подгонка кэша под совместимость
//!    (спека §3.1 её запрещает), а обратное: смена смысла редьюсера ОБЯЗАНА
//!    инвалидировать чекпоинт ЯВНО.
//!
//! # Почему `None` означает «на каждом событии», а не число
//!
//! Магическое `0` пришлось бы всем потребителям читать как «делить нельзя, это особый
//! случай». `Option` называет особый случай типом, а не соглашением.
//!
//! # Ведётся ВРЕМЕНЕМ СОБЫТИЯ, не часами — и это судится
//!
//! `VB-I-2` («живое == перепроигранное») рушится немедленно, если каденция зависит от
//! wall-clock: одно и то же окно журнала при повторном проигрывании дало бы другие точки.
//! `d12` сравнивает ДВА независимых прогона по одному журналу — при wall-clock-каденции они
//! разойдутся.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;
const NEAR_BAND: f64 = 0.001;

/// Событий в журнале: по одному L2-снимку каждые 100 мс на протяжении 10 секунд.
/// Тик heatmap — каждое событие; тик депт-серии — её собственный интервал.
const EVENTS: i64 = 100;
const EVENT_STEP_MS: i64 = 100;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "MD-I-8 cadence fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// **COMPILE-RED:** поля `depth_cadence_ms` в `Selector` ещё нет — в этом предмет `d12`.
fn sel(depth_cadence_ms: Option<i64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![NEAR_BAND],
        window_ms: None,
        depth_cadence_ms,
    }
}

fn journal() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("SETUP: tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("SETUP: open_with");
    for i in 0..EVENTS {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![lvl(MID - 1.0 - i as f64 * 0.01, 5.0)],
                asks: vec![lvl(MID + 1.0 + i as f64 * 0.01, 5.0)],
                ts_exch_ms: T0 + i * EVENT_STEP_MS,
            },
        ))
        .expect("SETUP: append");
    }
    j.flush().expect("SETUP: flush");
    dir
}

fn snap(dir: &std::path::Path, cadence: Option<i64>) -> gateway::Snapshot {
    gateway::snapshot(
        dir,
        EpochFilter::OwnCaptureOnly,
        &sel(cadence),
        Cursor::LATEST,
    )
    .expect("SETUP: snapshot обязан строиться")
}

fn depth_points(s: &gateway::Snapshot) -> usize {
    s.series.depth_series.iter().map(|r| r.series.len()).sum()
}

/// **`d12` (задача 15) — каденция депт-серии СВОЯ, ведётся временем события, настраивается.**
///
/// Три утверждения в одном оракуле намеренно: они об одном свойстве и порознь не значат
/// ничего. «Настраивается» без «влияет» — ручка `M-71` `B-1`; «влияет» без «временем
/// события» — сломанный `VB-I-2`.
#[test]
fn md_i8_d12_depth_cadence_is_per_series_and_event_time_driven() {
    let dir = journal();

    let fast = depth_points(&snap(dir.path(), Some(EVENT_STEP_MS)));
    let slow = depth_points(&snap(dir.path(), Some(1_000)));
    let slower = depth_points(&snap(dir.path(), Some(10_000)));

    assert!(
        fast > slow && slow > slower,
        "MD-I-8 d12: каденция не УПРАВЛЯЕТ числом точек депт-серии. Точек при интервалах \
         100/1000/10000 мс: {fast}/{slow}/{slower}. Ожидалось строгое убывание: реже тик — \
         меньше точек. Настройка, которая не меняет поведения, есть ручка без механизма \
         (класс R-133 B-1)"
    );
    assert!(
        slower >= 1,
        "MD-I-8 d12: при интервале 10 с на окне 10 с не осталось НИ ОДНОЙ точки — каденция \
         не прореживает, а гасит серию. Инструмент анализа обязан оставаться инструментом"
    );

    // Детерминизм: ДВА независимых прогона по одному журналу обязаны совпасть побайтно по
    // составу. При wall-clock-каденции они разойдутся — `VB-I-2` рушится немедленно.
    let a = snap(dir.path(), Some(1_000));
    let b = snap(dir.path(), Some(1_000));
    assert_eq!(
        a.series.depth_series, b.series.depth_series,
        "MD-I-8 d12 (VB-I-2): два прогона по ОДНОМУ журналу с одной каденцией дали разные \
         депт-серии. Значит тик ведётся часами, а не временем события, и живое перестало \
         быть равно перепроигранному"
    );
}

/// **`d13` (задача 16) — выдача НАЗЫВАЕТ каденцию каждой серии.**
///
/// `П-014` п.2 дословно: «выдача обязана ЭТО НАЗЫВАТЬ, а не умалчивать». В августе метка
/// прикрывала ДЕФЕКТ и была временной; после решения founder'а о разной каденции она
/// описывает НАМЕРЕННОЕ свойство и постоянна: две серии в одном кадре имеют разную частоту,
/// и потребитель обязан их различать, не догадываясь.
///
/// **COMPILE-RED:** поля `cadence_ms` в `SeriesBundle` ещё нет.
#[test]
fn md_i8_d13_output_declares_cadence_of_each_series() {
    let dir = journal();
    let s = snap(dir.path(), Some(1_000));

    let declared = &s.series.cadence_ms;
    let depth = declared
        .iter()
        .find(|(name, _)| name == "depth_series")
        .unwrap_or_else(|| {
            panic!(
                "MD-I-8 d13 (П-014 п.2): выдача не называет каденцию депт-серии. Объявлено: \
                 {declared:?}. Потребитель, сравнивающий полосу с ячейкой heatmap в одном \
                 кадре, обязан знать, что их частоты РАЗНЫЕ по решению продукта"
            )
        });
    assert_eq!(
        depth.1,
        Some(1_000),
        "MD-I-8 d13: объявленная каденция депт-серии {:?} не совпадает с запрошенной 1000 мс. \
         Метка, расходящаяся с поведением, хуже отсутствия метки: она врёт уверенно",
        depth.1
    );

    let heatmap = declared
        .iter()
        .find(|(name, _)| name == "heatmap")
        .unwrap_or_else(|| {
            panic!("MD-I-8 d13: выдача не называет каденцию heatmap. Объявлено: {declared:?}")
        });
    assert_eq!(
        heatmap.1, None,
        "MD-I-8 d13: heatmap обязан объявляться как пер-событийный (None), а не числом. \
         Магическое 0 пришлось бы каждому потребителю читать как особый случай; Option \
         называет особый случай типом, а не соглашением. Объявлено: {:?}",
        heatmap.1
    );
}
