//! RED M-38b rev2 (sacred, architect-only) — **C-030 R3/R1: скрытый полный реплей ФИЗИЧЕСКИ
//! невозможен, а lineage переживает удаление ПОКРЫТОГО префикса.**
//!
//! ## Зачем этот файл появился (critic C-030, REJECT rev1)
//!
//! В rev1 форсингов было два: подменный чекпоинт (`foreign_checkpoint_changes_output`) и
//! счётчик `ReadStats` (`red_checkpoint_resource_bound`). Критик показал реализацию, которая
//! проходит ОБА и при этом НЕ решает TD-044:
//!
//! > загрузить чекпоинт ровно настолько, чтобы возмутить выход для foreign-теста; для обычной
//! > корректности сделать ПОЛНЫЙ реплей от START; вернуть маленький `ReadStats`, собранный
//! > отдельным вызовом `stream_from` по хвосту.
//!
//! Дыра в том, что оба прежних форсинга наблюдают то, что реализация САМА о себе сообщает.
//! Здесь наблюдается физика: **покрытых чекпоинтом сегментов на диске больше нет**. Реализация,
//! втайне реплеящая историю, не может вернуть правильные байты — истории не существует.
//! Ни wall-clock, ни аллокатора: только удалённые файлы и байтовое сравнение.
//!
//! ## Второй инвариант того же теста — lineage под pruning (C-030 N2/R1)
//!
//! `journal_lineage` считается по заголовкам сегментов. Удаление покрытого префикса МЕНЯЕТ
//! множество видимых заголовков ⇒ наивная реализация («sha по всем текущим заголовкам»)
//! объявит валидный чекпоинт чужим и уйдёт в rebuild — а реплеить уже нечего, и кокпит молча
//! получит УСЕЧЁННУЮ историю (all-time VWAP поедет). Поэтому правило обязано быть
//! **суффикс-совместимым**: чекпоинт хранит манифест заголовков, которые он свернул; при
//! валидации отсутствие покрытых префиксных сегментов — ЗАКОННО, а любое расхождение в
//! ОСТАВШИХСЯ заголовках (или подмена/переупорядочивание) — нет. Спека: milestone §Инвалидация.
//!
//! Прецедент реалистичен: журнал уже штатно живёт без нижнего сегмента (M-36/TD-038 purge,
//! `crates/journal/tests/red_seg0_removed.rs`).
//!
//! COMPILE-RED: `gateway::checkpoint::advance_to`, `gateway::snapshot_from_checkpoint` ещё нет.
//!
//! testing.md: п.4 границы (prune ровно по границе сегмента), п.6 композиция стадий
//! (чекпоинт → prune → чтение — цепочка cron'а, а не одиночная стадия), п.7 парный vantage
//! (покрытый префикс удалять МОЖНО и результат обязан совпасть; непокрытый — зона retention-гварда,
//! `crates/journal/tests/red_retention_checkpoint_coverage.rs`).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;
const N: u64 = 2_000;
const SEG_BYTES: u64 = 24 * 1024;

fn cfg() -> WriterConfig {
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
            side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
            ts_exch_ms: D2_MS - (N as i64 * 100) + (i as i64 * 100),
        },
    )
}

/// Окно НЕ задано намеренно: unbounded-режим держит всю историю в состоянии, поэтому
/// «потерять префикс» максимально заметно (VWAP all-time, VP по всем сессиям).
fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
    }
}

fn big_journal() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    dir
}

fn canon(s: &gateway::Snapshot) -> Vec<u8> {
    serde_json::to_vec(s).expect("сериализация")
}

/// Удалить сегменты с индексом < `upto_index` — ровно то, что делает retention-prune после
/// доказанной холодной копии (`ColdCopyProof`). Возвращает число удалённых файлов.
fn prune_prefix(dir: &std::path::Path, upto_index: u32) -> usize {
    let segs = journal::list_segments(dir).expect("segments");
    let mut removed = 0;
    for s in segs.iter().filter(|s| s.index < upto_index) {
        std::fs::remove_file(&s.path).expect("remove segment");
        removed += 1;
    }
    removed
}

// ─────────────────────────────────────────────────────────────────────────────
// C-030 R3 — скрытый полный реплей невозможен: истории на диске НЕТ
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn covered_prefix_pruned_output_still_byte_identical() {
    let dir = big_journal();
    let segs = journal::list_segments(dir.path()).expect("segments");
    assert!(
        segs.len() >= 4,
        "нужен многосегментный журнал, есть {}",
        segs.len()
    );

    // Эталон считаем ДО удаления — потом его будет не из чего пересчитать.
    let want = canon(
        &gateway::snapshot(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &sel(),
            Cursor::LATEST,
        )
        .expect("snapshot(START) до prune"),
    );

    // Чекпоинт ровно на границе сегмента: покрывает сегменты 0..cut-1 ЦЕЛИКОМ.
    // last_seq(cut-1) = first_seq(cut) - 1.
    let cut = segs.len() as u32 - 2; // оставляем минимум два хвостовых сегмента
    let cut_first_seq = segs
        .iter()
        .find(|s| s.index == cut)
        .expect("сегмент cut")
        .header
        .first_seq;
    let k = Cursor {
        upto_seq: Some(cut_first_seq - 1),
    };

    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        k,
    )
    .expect("advance_to на границе сегмента");

    // PRUNE: покрытый префикс физически удаляется (retention после ColdCopyProof).
    let removed = prune_prefix(dir.path(), cut);
    assert!(
        removed >= 2,
        "должны были удалиться ≥2 сегмента, удалено {removed}"
    );

    let (got, stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint после prune покрытого префикса");

    assert_eq!(
        canon(&got),
        want,
        "C-030 R3 НАРУШЕН: после удаления ПОКРЫТОГО чекпоинтом префикса результат разошёлся \
         с эталоном, снятым до prune. Два возможных корня, оба блокирующие: (а) реализация \
         втайне реплеит историю от START (её больше нет — значит выход усечён); (б) \
         journal_lineage посчитан по ТЕКУЩИМ заголовкам, удаление префикса объявило валидный \
         чекпоинт чужим → тихий rebuild по остаткам → кокпит молча получил усечённую историю \
         (all-time VWAP поехал). Требуется суффикс-совместимая валидация lineage."
    );

    // Ресурс: читать можно только хвост — старых сегментов уже нет физически, но реализация
    // не должна и пытаться (иначе после prune посыплются ошибки открытия).
    assert!(
        stats.events_decoded < N,
        "декодировано {} из {N} — реализация всё ещё пытается читать историю",
        stats.events_decoded
    );
    assert!(
        (stats.segments_opened as usize) <= 3,
        "открыто {} сегментов — после prune должен читаться только хвост",
        stats.segments_opened
    );
}

/// Композиция стадий (п.6): cron гоняет `advance` МНОГОКРАТНО, а retention — между прогонами.
/// Проверяем цепочку «advance → prune → advance → prune → чтение», а не одиночную стадию:
/// именно в композиции ломались TD-042/TD-045.
#[test]
fn repeated_advance_and_prune_cycles_stay_identical() {
    let dir = big_journal();
    let want = canon(
        &gateway::snapshot(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &sel(),
            Cursor::LATEST,
        )
        .expect("snapshot(START) до всех prune"),
    );

    let ckpt = tempfile::tempdir().expect("ckpt");
    let segs = journal::list_segments(dir.path()).expect("segments");
    let total = segs.len() as u32;
    assert!(total >= 5, "нужно ≥5 сегментов, есть {total}");

    // Два цикла: покрыть префикс чекпоинтом → удалить его → повторить глубже.
    for cut in [total - 3, total - 2] {
        let cur = journal::list_segments(dir.path()).expect("segments");
        let Some(seg) = cur.iter().find(|s| s.index == cut) else {
            continue;
        };
        let k = Cursor {
            upto_seq: Some(seg.header.first_seq - 1),
        };
        gateway::checkpoint::advance_to(
            dir.path(),
            ckpt.path(),
            &sel(),
            EpochFilter::OwnCaptureOnly,
            k,
        )
        .expect("advance_to в цикле");
        prune_prefix(dir.path(), cut);
    }

    let (got, _stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint после двух циклов");

    assert_eq!(
        canon(&got),
        want,
        "КОМПОЗИЦИЯ НАРУШЕНА: после двух циклов «advance → prune» результат разошёлся с \
         эталоном. Инкрементальный чекпоинт обязан переживать повторное усечение префикса — \
         это штатный режим ops-cron, а не экзотика."
    );
}

/// Парный vantage к суффикс-совместимости (п.7 + п.3 «отсутствие»): удаление ПОКРЫТОГО
/// префикса законно (тесты выше) — но исчезновение НЕПОКРЫТОГО сегмента не даёт права
/// досочинить его из чекпоинта. Иначе «суффикс-совместимость» выродится в «lineage не
/// проверяем», и пропажа/подмена ещё-не-свёрнутых данных пройдёт молча.
#[test]
fn missing_uncovered_segment_is_not_invented_from_checkpoint() {
    let dir = big_journal();
    let segs = journal::list_segments(dir.path()).expect("segments");
    let cut = segs.len() as u32 - 2;
    let cut_first_seq = segs
        .iter()
        .find(|s| s.index == cut)
        .expect("сегмент cut")
        .header
        .first_seq;

    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        Cursor {
            upto_seq: Some(cut_first_seq - 1),
        },
    )
    .expect("advance_to");

    // Исчезает сегмент, который чекпоинт ЕЩЁ НЕ свернул (индекс `cut` — начало хвоста).
    // Это НЕ prune покрытого префикса: свернуть его в состояние было невозможно.
    let tail = journal::list_segments(dir.path())
        .expect("segments")
        .into_iter()
        .find(|s| s.index == cut)
        .expect("хвостовой сегмент");
    std::fs::remove_file(&tail.path).expect("remove tail segment");

    let (got, _stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("не ошибка: чекпоинт — кэш (GW-I-9б)");

    let honest = canon(
        &gateway::snapshot(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &sel(),
            Cursor::LATEST,
        )
        .expect("snapshot(START) на усечённом журнале"),
    );
    assert_eq!(
        canon(&got),
        honest,
        "исчезновение НЕПОКРЫТОГО (хвостового) сегмента — не то же самое, что prune покрытого \
         префикса: пересчёт обязан идти по фактически доступным данным, без досочинения \
         пропавшего хвоста из чекпоинта"
    );
}
