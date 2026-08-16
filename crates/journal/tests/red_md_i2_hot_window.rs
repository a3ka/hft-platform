//! RED M-67 `MD-I-2` (sacred, architect-only) — ГОРЯЧЕЕ ОКНО ретеншена меряется в ДАННЫХ,
//! а не в файлах.
//!
//! # Что этот оракул пиннит и почему он существует
//!
//! `M-67` (rev1) обещал слой L2: сырые дельты всего рынка в скользящем окне 48 часов, при
//! постоянном хранении L1. Критик `C-091` F-2 заявил, что совместимой конструкции хранения
//! нет. Замер (architect, 2026-08-16) показал, что критик прав, и назвал ПРИЧИНУ точнее:
//!
//! 1. **ротация — ТОЛЬКО по размеру**: `DEFAULT_MAX_SEGMENT_BYTES = 1 GiB`
//!    (`segments.rs:41`), решение о ротации принимается по `seg_size + frame_len >
//!    max_segment_bytes` (`lib.rs:232-237`). Ротации по ВРЕМЕНИ не существует;
//! 2. **ретеншен удаляет ЦЕЛЫЙ сегмент по возрасту его ПЕРВОГО события**:
//!    `age = now − segment_decision_ts(s)`, где `segment_decision_ts` = ts первого кадра
//!    (`segments.rs:3512-3530`, `:3793-3798`).
//!
//! Следствие, из-за которого окно не реализуемо в принципе, а не «не описано»: сегмент
//! покрывает тем БОЛЬШИЙ интервал времени, чем реже пишет его источник. Замер провода
//! 2026-08-16 (`docs/plans/M-67-capacity-2026-08-16.md`): медианный хвостовой спот-символ —
//! 51 B/s, в журнале ×2.447 ⇒ 125 B/s ⇒ один сегмент 1 GiB набирается **~99 суток**.
//! Тогда единственное решение, доступное ретеншену, — «удалить эти 99 суток целиком» либо
//! «не удалять ничего». Окна в 48 часов среди этих двух вариантов нет.
//!
//! Оба отказа при этом происходят на ОДНОМ шарде: он держит данные 99 суток (слишком долго)
//! и, дождавшись ротации, удаляет разом всё — включая события, которым секунды.
//!
//! # Контракт, который обязана предоставить реализация
//!
//! Периметр удаления обязан определяться ДАННЫМИ, а не файлом:
//! **ни одно событие моложе окна не удаляется, и ни одно событие старше окна не остаётся**
//! (второе — с точностью до активного сегмента, который трогать нельзя).
//! Как это достигается — время-ориентированная ротация, посегментный дозапрос по времени,
//! перепаковка или пошардный класс политики — зона реализации; оракул фиксирует СВОЙСТВО.
//!
//! # Анти-плацебо (обе стороны, `testing.md` «мутационный контроль — вопросов ДВА»)
//!
//! * `b2` роняет реализацию «ничего не удалять»: она тривиально проходит `b1`;
//! * `b1` роняет сегодняшнюю реализацию и любую, режущую по файлам: она проходит `b2`.
//!
//! Ни один из двух ассертов не достаточен сам по себе — именно поэтому они в одном файле.

use contracts::{to_fixed, DataSource, Event, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, RetentionPolicy, WriterConfig};

const HOUR_MS: i64 = 3_600_000;
const T0: i64 = 1_752_000_000_000;
/// Горячее окно M-67 §4.2. Выражается политикой точно: `retain_days = 2` ⇒ `cutoff = 48 ч`.
const WINDOW_MS: i64 = 48 * HOUR_MS;
/// Полный охват фикстуры по времени данных.
const SPAN_H: i64 = 120;
/// «Сейчас» для детерминированного плана (часы снаружи — `DESIGN.md` §1).
const NOW: i64 = T0 + SPAN_H * HOUR_MS;
const N_EVENTS: u64 = 6_000;

fn trade_at(ts_exch_ms: i64, i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "TAILUSDT",
        MdPayload::Trade {
            price: to_fixed(1.0) + i as i64,
            size: to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms,
        },
    )
}

fn cfg() -> WriterConfig {
    WriterConfig {
        // Мал НАМЕРЕННО: даёт десятки сегментов на фикстуре, каждый покрывает несколько
        // часов данных. Это МОДЕЛЬ прод-ситуации «редкий источник, крупный сегмент»
        // (99 суток на 1 GiB) в масштабе, который тест может построить за миллисекунды.
        max_segment_bytes: 16 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-67 MD-I-2 hot-window fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn policy(cold: &std::path::Path) -> RetentionPolicy {
    RetentionPolicy {
        retain_days: 2, // ровно 48 ч — окно M-67 §4.2 выражается политикой ТОЧНО
        keep_min_segments: 0,
        cold_root: cold.to_path_buf(),
        min_free_bytes: 0,
        // Покрытие «всё» — оракул НЕ про чекпоинт-гейт M-38b; иначе «не удалено из-за
        // отсутствия покрытия» было бы неотличимо от «периметр режет по файлу».
        checkpoint_covered_through_seq: Some(u64::MAX),
        allow_prune_without_checkpoint: false,
    }
}

fn data_ts(ev: &Event) -> i64 {
    match &ev.kind {
        EventKind::Md(md) => match &md.payload {
            MdPayload::Trade { ts_exch_ms, .. } => *ts_exch_ms,
            _ => unreachable!("фикстура пишет только Trade"),
        },
        EventKind::Sys(_) => ev.ts_wall_ms,
    }
}

/// Время данных течёт РАВНОМЕРНО по всей фикстуре: событие `i` несёт
/// `ts = T0 + i * SPAN / N`. Поэтому каждый сегмент покрывает непрерывный интервал времени,
/// а граница окна (`NOW − 48 ч` = `T0 + 72 ч`) заведомо попадает ВНУТРЬ одного из них.
fn build(dir: &std::path::Path) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in 0..N_EVENTS {
        let ts = T0 + (i as i64 * SPAN_H * HOUR_MS) / N_EVENTS as i64;
        j.append(trade_at(ts, i)).expect("append");
    }
    j.flush().expect("flush");
}

/// `(first_seq, last_data_ts)` по каждому сегменту: события относятся к сегменту по
/// диапазону `seq` из `list_segments`, ts берётся из ДАННЫХ.
fn segment_last_data_ts(dir: &std::path::Path) -> Vec<(u64, i64)> {
    let mut segs = journal::list_segments(dir).expect("list_segments");
    segs.sort_by_key(|s| s.index);
    let bounds: Vec<u64> = segs.iter().map(|s| s.header.first_seq).collect();

    let mut last: Vec<i64> = vec![i64::MIN; segs.len()];
    let stream = journal::stream(dir, EpochFilter::OwnCaptureOnly).expect("stream");
    for ev in stream {
        let ev = ev.expect("event");
        // индекс сегмента = последний, чей first_seq <= ev.seq
        let idx = match bounds.binary_search(&ev.seq) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };
        last[idx] = last[idx].max(data_ts(&ev));
    }
    bounds.into_iter().zip(last).collect()
}

/// SETUP-GUARD (`testing.md` «целостность гейта» §3): проба обязана падать и тогда, когда
/// сценарий НЕ СОСТОЯЛСЯ. Без этого тест мог бы молча проверять журнал из одного сегмента.
fn setup_guard(dir: &std::path::Path) -> Vec<(u64, i64)> {
    let per_seg = segment_last_data_ts(dir);
    assert!(
        per_seg.len() >= 5,
        "SETUP не состоялся: сегментов {} (<5) — фикстура не моделирует «сегмент шире окна»",
        per_seg.len()
    );
    let cutoff = NOW - WINDOW_MS;
    let segs = journal::list_segments(dir).expect("list_segments");
    let spanning = segs
        .iter()
        .filter(|s| journal::segment_decision_ts(s) < cutoff)
        .filter_map(|s| {
            per_seg
                .iter()
                .find(|(fs, _)| *fs == s.header.first_seq)
                .map(|(_, lt)| *lt)
        })
        .any(|last_ts| last_ts > cutoff);
    assert!(
        spanning,
        "SETUP не состоялся: ни один сегмент не пересекает границу окна {cutoff} — \
         оракул проверял бы не тот сценарий"
    );
    per_seg
}

/// **B1 — ГЛАВНОЕ.** Ни один сегмент, попавший в план удаления, не содержит события МОЛОЖЕ
/// окна. Сегодня падает: периметр режется по файлу, а файл шире окна.
#[test]
fn md_i2_b1_no_event_younger_than_window_is_deleted() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path());
    let per_seg = setup_guard(dir.path());

    let plan = journal::retention_plan(dir.path(), &policy(cold.path()), NOW).expect("plan");
    let cutoff = NOW - WINDOW_MS;

    let mut violations: Vec<String> = Vec::new();
    for s in plan
        .offload_and_prune
        .iter()
        .chain(plan.offload_only.iter())
    {
        let last_ts = per_seg
            .iter()
            .find(|(fs, _)| *fs == s.header.first_seq)
            .map(|(_, lt)| *lt)
            .expect("сегмент плана обязан быть в перечне");
        if last_ts > cutoff {
            violations.push(format!(
                "{}: последнее событие ts={} моложе границы окна {} на {} мин",
                s.path.display(),
                last_ts,
                cutoff,
                (last_ts - cutoff) / 60_000
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "MD-I-2 нарушен: план удаляет ЦЕЛЫЕ сегменты, внутри которых есть данные моложе \
         окна 48 ч. Горячее окно обязано резаться по ДАННЫМ, а не по файлам.\n{}",
        violations.join("\n")
    );
}

/// **B2 — обратная сторона.** Данные СТАРШЕ окна обязаны реально удаляться. Реализация
/// «ничего не удалять» проходит B1 тривиально и обязана падать здесь.
#[test]
fn md_i2_b2_stale_data_is_actually_removed() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path());
    let per_seg = setup_guard(dir.path());
    let cutoff = NOW - WINDOW_MS;

    let stale_exists = per_seg.iter().any(|(_, lt)| *lt <= cutoff);
    assert!(
        stale_exists,
        "SETUP не состоялся: в фикстуре нет ни одного сегмента целиком старше окна"
    );

    let plan = journal::retention_plan(dir.path(), &policy(cold.path()), NOW).expect("plan");
    let planned: usize = plan.offload_and_prune.len();
    assert!(
        planned > 0,
        "MD-I-2 нарушен с другой стороны: данные старше 48 ч существуют, но план пуст — \
         горячее окно не удерживается, диск растёт неограниченно"
    );
}
