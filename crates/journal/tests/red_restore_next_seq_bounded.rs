//! SACRED (architect-only) — M-40 / R2b, **прод-масштаб**: определение `next_seq` поверх
//! ВОССТАНОВЛЕННОЙ СЖАТОЙ истории обязано быть потоковым (bounded memory).
//!
//! ## Зачем отдельный оракул
//!
//! `red_restore_from_cold.rs` требует КОРРЕКТНОСТИ: после restore писатель продолжает `seq`
//! с конца сжатой истории. Самая простая реализация этого требования — распаковать последний
//! `.zst` целиком в память и взять последний `seq`. На боевом сегменте (1 GiB несжатого,
//! ~110 MB сжатого) это ровно инцидент **TD-011**: `Journal::open` делал `read_to_end` всего
//! сегмента, recorder переставал писать, юнит-тесты на фикстурах в десятки байт были зелёными,
//! CI зелёный, «Deploy success» зелёный — поймал только eyes-on на VPS.
//!
//! Путь `open()` — единственный, который исполняется при КАЖДОМ старте recorder'а, в том
//! числе на VPS без `mem_limit` (R10 в `docs/08`: OOM одного контейнера уводит весь хост).
//! Поэтому граница ресурса пиннится тестом, а не code-review.
//!
//! ## Почему файл содержит РОВНО ОДИН тест
//!
//! Счётчик аллокаций глобален для процесса, а `cargo` гонит тесты одного бинаря ПАРАЛЛЕЛЬНО
//! (урок TD-040: замер одного теста ловил аллокации другого → dev PASS / CI FAIL). Отдельный
//! тест-бинарь = отдельный процесс: замер видит только свои аллокации, без `Mutex`-сериализации.
//!
//! ## Оракул мерит ТО, ЧТО ОБЕЩАЕТ (TD-021)
//!
//! Меряется пик аллокаций во время `Journal::open_with` — то есть память ОТКРЫТИЯ, а не размер
//! какого-либо результата: `open_with` ничего не материализует по контракту. Одновременно
//! проверяется КОРРЕКТНОСТЬ (`next_seq` продолжает историю) — иначе реализация «ничего не
//! читаем, укладываемся в 0 байт» прошла бы бюджет, потеряв данные.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{DataSource, EventKind, Level, MdPayload, Venue};
use journal::{EpochFilter, Journal, WriterConfig, DEFAULT_COMPACT_LEVEL};

static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), SeqCst) + l.size();
            PEAK.fetch_max(c, SeqCst);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        CUR.fetch_sub(l.size(), SeqCst);
    }
}
#[global_allocator]
static GA: Counting = Counting;

fn peak_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = CUR.load(SeqCst);
    PEAK.store(base, SeqCst);
    let r = f();
    (r, PEAK.load(SeqCst).saturating_sub(base))
}

/// Крупное событие (~2 KB) — чтобы сегменты набирали прод-подобный объём за разумное время.
fn snapshot(i: u64) -> EventKind {
    let lvl = |k: i64| Level {
        price: 6_400_000_000_000 + k * 100 + i as i64,
        size: 1_000 + k,
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: (0..60).map(lvl).collect(),
            asks: (0..60).map(lvl).collect(),
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 32 * 1024 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "restore bounded fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

const N: u64 = 60_000;
/// Бюджет памяти открытия. Ориентир — `red_compaction.rs::c5` (тот же класс: потоковое
/// чтение сжатого сегмента). Наивная реализация распаковывает сегмент (32 MiB) целиком.
const BUDGET_BYTES: usize = 16 * 1024 * 1024;

#[test]
fn rs_5_next_seq_over_restored_compacted_history_is_bounded_memory() {
    // (1) Прод-масштабная история, полностью сжатая, без journal.meta — каталог из холодного.
    let src = tempfile::tempdir().expect("src");
    {
        let mut j = Journal::open_with(src.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(snapshot(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(src.path(), 0, DEFAULT_COMPACT_LEVEL).expect("compact");

    let restored = tempfile::tempdir().expect("restored");
    let mut names: Vec<String> = std::fs::read_dir(src.path())
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    let mut compacted_bytes = 0u64;
    for name in &names {
        if name.ends_with(".zst") {
            let dst = restored.path().join(name);
            std::fs::copy(src.path().join(name), &dst).expect("copy");
            compacted_bytes += std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        }
    }

    // Setup-guard: фикстура обязана доказать прод-масштаб, иначе бюджет проходит по пустоте.
    let raw_left = std::fs::read_dir(restored.path())
        .expect("read_dir")
        .filter(|e| {
            e.as_ref()
                .map(|e| e.file_name().to_string_lossy().ends_with(".jrnl"))
                .unwrap_or(false)
        })
        .count();
    assert!(
        compacted_bytes > 4 * 1024 * 1024 && raw_left == 0,
        "фикстура не состоялась: восстановлено {compacted_bytes} B сжатых сегментов, сырых {raw_left} \
         (нужны десятки MiB несжатого объёма и НИ ОДНОГО сырого — иначе бюджет памяти ничего не значит)"
    );
    let history: Vec<u64> = journal::stream(restored.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event").seq)
        .collect();
    let max_history = *history.iter().max().expect("непустая история");
    // Активный сегмент остаётся сырым и в холодное хранилище не уезжает — восстанавливается
    // ВСЯ сжатая часть, кроме хвоста. Требуем подавляющее большинство событий.
    assert!(
        history.len() as u64 > N * 4 / 5,
        "фикстура: восстановлено {} событий из {N} — слишком мало для прод-масштабного замера",
        history.len()
    );
    let restored_n = history.len();

    // (2) Замеряем ИМЕННО открытие: recorder стартует поверх восстановленного каталога.
    let (mut j, peak) =
        peak_delta(|| Journal::open_with(restored.path(), cfg()).expect("open_with"));

    // (2а) КОРРЕКТНОСТЬ — иначе «ничего не прочитали» уложилось бы в любой бюджет.
    j.append(snapshot(999_999)).expect("append");
    j.flush().expect("flush");
    drop(j);
    let after: Vec<u64> = journal::stream(restored.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event").seq)
        .collect();
    assert_eq!(
        after.len(),
        restored_n + 1,
        "старт поверх восстановленной сжатой истории потерял события.\n\
         ДОЛЖНО БЫТЬ: {} событий (восстановлено {restored_n} + дописано 1)\nПОЛУЧЕНО: {}",
        restored_n + 1,
        after.len()
    );
    let new_seq = *after.iter().max().expect("непусто");
    assert!(
        new_seq > max_history,
        "next_seq не продолжил сжатую историю.\nДОЛЖНО БЫТЬ: новый seq > {max_history}\n\
         ПОЛУЧЕНО: {new_seq}"
    );

    // (2б) ГРАНИЦА РЕСУРСА.
    assert!(
        peak < BUDGET_BYTES,
        "открытие журнала поверх сжатой истории выделило {peak} B (бюджет {BUDGET_BYTES} B).\n\
         Сжатая история восстановлена целиком в память вместо потокового чтения хвоста. На \
         боевом сегменте (1 GiB несжатого) это OOM при КАЖДОМ старте recorder'а — класс TD-011, \
         который уже один раз останавливал сбор данных на проде."
    );
}
