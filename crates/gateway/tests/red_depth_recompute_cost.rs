//! RED `MD-I-8` `d6` (sacred, architect-only) — ЦЕНА ПЕРЕСЧЁТА ПОЛОС НЕ РАСТЁТ С ГЛУБИНОЙ
//! КНИГИ (`C-094` B5).
//!
//! Вынесен в отдельный файл НАМЕРЕННО, и причина методическая: это COMPILE-RED — он требует
//! поля `ReadStats::depth_levels_visited`, которого ещё нет. Оставленный в общем наборе, он
//! ронял бы КОМПИЛЯЦИЮ всего бинаря, и `d1`…`d5` нельзя было бы ПРЕДЪЯВИТЬ красными: «не
//! собралось» и «упало на ассерте» — разные вещи, а RED-first требует второго. Ресурсные
//! оракулы проекта и так живут отдельными файлами (`red_checkpoint_resource_bound.rs`,
//! `red_segment_meta_bound.rs`).
//!
//! Счётчик отдаёт САМ API — тот же приём, которым введены `events_scanned` (M-57) и
//! `segment_meta_ops` (M-62). Глобальных счётчиков в оракуле нет: процессный атомик делает
//! замер зависимым от соседних тестов, и `scripts/check_resource_oracles.sh` его запрещает.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;
const FAR_BAND: f64 = 0.60;
const NARROW_OFFSET: f64 = 0.0008;
const NARROW_SIZE: f64 = 300.0;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "MD-I-8 d6 depth-recompute-cost fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

fn sel(bands: Vec<f64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
    }
}

/// **D6 (`C-094` B5) — цена пересчёта полос не растёт с ГЛУБИНОЙ КНИГИ.**
///
/// `red_gateway_bounded` и `red_snapshot_noclone` держат окно ПАМЯТИ; стоимость НОВОЙ
/// проводки они не меряют. Наивная реализация «на каждой дельте пройти всю книгу × 7 полос»
/// корректна и при этом даёт цену, растущую с глубиной книги, — а книга у нас ±60 % от mid.
///
/// # Что здесь меряется и почему это РЕСУРС, а не прокси
///
/// Меряется `ReadStats::depth_levels_visited` — число уровней книги, посещённых при
/// пересчёте полос за проход. Это тот же приём, которым уже введены `events_scanned`
/// (M-57) и `segment_meta_ops` (M-62): счётчик отдаёт САМ API, глобальных счётчиков в
/// оракуле нет (их запрещает `scripts/check_resource_oracles.sh` — процессный атомик делает
/// замер зависимым от соседних тестов).
///
/// # Конфаундинг held constant
///
/// Число дельт и число полос ОДИНАКОВЫ в обоих прогонах; варьируется ТОЛЬКО глубина книги
/// (10 уровней против 400). Наблюдаемая величина обязана НЕ масштабироваться с ней.
///
/// COMPILE-RED: поле `ReadStats::depth_levels_visited` вводится реализацией (задача 5).
#[test]
fn md_i8_d6_depth_recompute_cost_does_not_scale_with_book_depth() {
    fn journal_with_book_depth(levels: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        let bids: Vec<Level> = (0..levels)
            .map(|k| lvl(MID * (1.0 - 0.0005 - 0.001 * k as f64), 1.0))
            .collect();
        let asks: Vec<Level> = (0..levels)
            .map(|k| lvl(MID * (1.0 + 0.0005 + 0.001 * k as f64), 1.0))
            .collect();
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids,
                asks,
                ts_exch_ms: T0,
            },
        ))
        .expect("append snapshot");
        // РОВНО столько же дельт в обоих прогонах — конфаундинг держится константным.
        for i in 0..24i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::L2Delta {
                    bids: vec![lvl(MID * (1.0 - NARROW_OFFSET), NARROW_SIZE)],
                    asks: vec![lvl(MID * (1.0 + NARROW_OFFSET), NARROW_SIZE)],
                    first_update_id: (i as u64) * 2 + 1,
                    final_update_id: (i as u64) * 2 + 2,
                    prev_final_update_id: if i == 0 { None } else { Some((i as u64) * 2) },
                    ts_exch_ms: T0 + i * 100 + 10,
                },
            ))
            .expect("append delta");
        }
        j.flush().expect("flush");
        dir
    }

    /// Возвращает ПАРУ (посещённые уровни, лучшие цены bid/ask) — вторая половина нужна
    /// setup-guard'у неподвижности mid (спека §5.5): бюджет объявлен для установившегося
    /// режима, и сдвиг mid обязан быть исключён ЗАМЕРОМ, а не устройством фикстуры «на глаз».
    fn visited(dir: &std::path::Path) -> (u64, (i64, i64)) {
        let ckpt = tempfile::tempdir().expect("ckpt tempdir");
        let (snap, stats) = gateway::snapshot_from_checkpoint(
            dir,
            EpochFilter::OwnCaptureOnly,
            &sel(vec![0.015, 0.03, 0.05, 0.08, 0.15, 0.30, FAR_BAND]),
            ckpt.path(),
            Cursor::LATEST,
        )
        .expect("snapshot_from_checkpoint обязан строиться");
        let best_bid = snap
            .series
            .cob
            .iter()
            .filter(|l| l.side == "bid")
            .map(|l| l.price_e8)
            .max()
            .unwrap_or(0);
        let best_ask = snap
            .series
            .cob
            .iter()
            .filter(|l| l.side == "ask")
            .map(|l| l.price_e8)
            .min()
            .unwrap_or(0);
        (stats.depth_levels_visited, (best_bid, best_ask))
    }

    let shallow = journal_with_book_depth(10);
    let deep = journal_with_book_depth(400);

    let (a, mid_shallow) = visited(shallow.path());
    let (b, mid_deep) = visited(deep.path());

    // SETUP-GUARD (спека §5.5) — КОНФАУНДИНГ `mid` ДЕРЖИТСЯ КОНСТАНТНЫМ, и это предъявлено
    // замером. Дельты фикстуры кладутся СТРОГО ВНУТРЬ книги (0.08 % от mid при лучшем уровне
    // на 0.05 %), поэтому лучшие цены не двигаются ни одной дельтой и совпадают между двумя
    // прогонами. Без этого ассерта варьировалась бы не только измеряемая величина: сдвиг mid
    // смещает пороги ВСЕХ полос и делает полный пересчёт законным — тогда красное `b > a*4`
    // означало бы не наивную реализацию, а честный пересчёт по смещённым порогам
    // (`testing.md` §«Целостность гейта» свойство 2: конфаундинг держать КОНСТАНТНЫМ).
    assert_eq!(
        mid_shallow, mid_deep,
        "MD-I-8 d6 SETUP НЕ СОСТОЯЛСЯ: лучшие цены разошлись между прогонами \
         (мелкая книга {mid_shallow:?}, глубокая {mid_deep:?}). Пороги полос считаются от mid, \
         значит варьируется не только глубина книги, и сравнение ниже сравнивает не то."
    );
    assert!(
        mid_shallow.0 > 0 && mid_shallow.1 > 0,
        "MD-I-8 d6 SETUP НЕ СОСТОЯЛСЯ: COB пуст ({mid_shallow:?}) — неподвижность mid \
         не предъявлена ничем, ассерт выше вакуумен"
    );

    // SETUP-GUARD: если счётчик молчит в ОБОИХ прогонах, оракул проверяет не то
    // (`testing.md`: проба обязана падать и при несостоявшемся setup'е).
    assert!(
        a > 0,
        "MD-I-8 d6 SETUP НЕ СОСТОЯЛСЯ: depth_levels_visited = 0 на мелкой книге — счётчик \
         ничего не наблюдает, и сравнение ниже было бы вакуумным"
    );

    // Книга глубже в 40 раз. Допуск ×4 — щедрый: он пропускает любую реализацию, чья цена
    // растёт с ЧИСЛОМ ПОЛОС или логарифмом книги, и ловит ровно линейный обход по глубине.
    assert!(
        b <= a * 4,
        "MD-I-8 d6: книга глубже в 40 раз дала {b} посещённых уровней против {a} на мелкой \
         (допуск ×4). Цена пересчёта полос масштабируется с ГЛУБИНОЙ КНИГИ — это наивный \
         полный обход на каждой дельте. Число дельт и число полос в обоих прогонах одинаковы, \
         значит различие вносит только глубина."
    );
}
