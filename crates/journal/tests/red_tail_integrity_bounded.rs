//! SACRED (architect-only) — M-49 rev5 / **задача 5b: «терпимо И ограниченно» неразменны**.
//!
//! ## Почему этот оракул существует
//!
//! Контракт rev5 требует от вычисления пола (`Known`) ДВУХ свойств ОДНОВРЕМЕННО:
//! - **терпимо** — читаемый ПРЕФИКС повреждённого сегмента участвует в поле (иначе блокер
//!   R-002: пол падает до нуля и декларация становится каналом seq-reuse);
//! - **ограниченно** — без `Vec<Event>` на весь сегмент и без полной распаковки `.zst`
//!   (иначе возвращается NOTE 2 вердикта R-001, класс TD-011: 1 GiB в RAM ровно в момент,
//!   когда оператор разбирает инцидент).
//!
//! Наивный фикс блокера — «вернуть `read_segment_events(p, false)`» — чинит первое свойство
//! ценой второго, и по отдельности каждый оракул это пропустит. Поэтому оба утверждения
//! живут в ОДНОМ тесте и не подлежат размену: сначала проверяется, что пол честный
//! (декларация внутри занятого диапазона ОТВЕРГНУТА), затем — что цена этого знания
//! ограничена и НЕ РАСТЁТ с размером сегмента.
//!
//! ## Метод замера
//!
//! Счётчик аллокаций через global allocator — тот же образец, что `red_open_bounded.rs`
//! (перманентный guard TD-011). Проверяются ДВА свойства памяти, а не одно:
//!  (2) абсолютный бюджет на большом сегменте — ловит полное чтение;
//!  (3) НЕЗАВИСИМОСТЬ ОТ РАЗМЕРА (пик(BIG) − пик(SMALL)) — ловит и «читать долю файла»,
//!      что абсолютный бюджет на одном размере пропустил бы (6 MiB на 64 MiB выглядят
//!      прилично, но те же 10% от 1 GiB — это 100 MiB на проде).
//! Анти-плацебо: контрольное `fs::read` большого сегмента обязано превысить бюджет —
//! иначе замер слеп.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

mod common;

use common::{
    cfg_with, corrupt_tail, ls, snap, tolerant_readable_max, write_decl, DECL_APPLIED,
    TAIL_SCAN_CHUNK,
};
use journal::{Journal, WriterConfig};

static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), SeqCst) + l.size();
            PEAK.fetch_max(c, SeqCst);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        CUR.fetch_sub(l.size(), SeqCst);
    }
}
#[global_allocator]
static GA: Counting = Counting;

/// Пиковая аллокация (дельта) во время `f`.
fn peak_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = CUR.load(SeqCst);
    PEAK.store(base, SeqCst);
    let r = f();
    (r, PEAK.load(SeqCst).saturating_sub(base))
}

fn cfg() -> WriterConfig {
    cfg_with(256 * 1024 * 1024, "bounded floor fixture")
}

const SEG: &str = "segment-00000000.jrnl";
/// Порча строго больше окна хвостового скана — путь декларации обязан быть достижим.
const CORRUPT_BYTES: usize = 4 * 1024 * 1024 + 1024 * 1024;
const SMALL: u64 = 8 * 1024 * 1024;
const BIG: u64 = 32 * 1024 * 1024;
const THRESHOLD: usize = 16 * 1024 * 1024; // абсолютный бюджет на пути валидации декларации
const INDEP_DELTA: usize = 2 * 1024 * 1024; // допустимый рост памяти между размерами

/// Один СЫРОЙ сегмент ≥ `target` с испорченным хвостом, без меты, с декларацией внутри
/// уже занятого диапазона `seq`. Возвращает `(каталог, читаемый максимум)`.
fn corrupt_prodscale_dir(target: u64) -> (tempfile::TempDir, u64) {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        let mut n: u64 = 0;
        loop {
            j.append(snap(n)).expect("append");
            n += 1;
            if n.is_multiple_of(512) {
                j.flush().expect("flush");
                let sz = std::fs::metadata(dir.path().join(SEG)).expect("meta").len();
                if sz > target {
                    break;
                }
            }
            assert!(n < 5_000_000, "фикстура не набрала объём");
        }
        j.flush().expect("flush");
    }
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let size = std::fs::metadata(dir.path().join(SEG)).expect("meta").len();
    assert!(
        size > TAIL_SCAN_CHUNK,
        "фикстура: сегмент {size} B обязан быть больше окна скана {TAIL_SCAN_CHUNK} B"
    );
    corrupt_tail(&dir.path().join(SEG), CORRUPT_BYTES);

    let readable_max = tolerant_readable_max(dir.path()).unwrap_or_else(|| {
        panic!("фикстура: у повреждённого сегмента обязан остаться ЧИТАЕМЫЙ ПРЕФИКС")
    });
    assert!(
        readable_max > 1,
        "фикстура: читаемый префикс обязан нести события (max seq={readable_max}), иначе \
         декларация next_seq=1 законна и оракул проверяет не тот контракт"
    );

    // Оператор объявляет позицию ЗАВЕДОМО внутри занятого диапазона.
    write_decl(dir.path(), 1, "ошибка оператора: seq внутри истории");
    (dir, readable_max)
}

// ═════════════════════════════════════════════════════════════════════════════════════
// OP-8 — пол ЧЕСТЕН и стоит ОГРАНИЧЕННОЙ памяти (свойства неразменны)
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn op_8_readable_floor_is_tolerant_and_memory_bounded() {
    let (small, small_max) = corrupt_prodscale_dir(SMALL);
    let (big, big_max) = corrupt_prodscale_dir(BIG);
    assert!(
        big_max > small_max,
        "предусловие: большой сегмент несёт больше событий ({small_max} < {big_max})"
    );

    // Анти-плацебо: полное чтение большого сегмента обязано превысить бюджет — иначе замер
    // слеп и «уложился в бюджет» ничего не значит.
    let (_, peak_full) = peak_delta(|| std::fs::read(big.path().join(SEG)).expect("read"));
    assert!(
        peak_full > THRESHOLD,
        "контроль: полное чтение сегмента ({peak_full} B) обязано превышать бюджет \
         {THRESHOLD} B — иначе замер ничего не доказывает"
    );

    let (res_big, peak_big) = peak_delta(|| Journal::open_with(big.path(), cfg()));
    let (res_small, peak_small) = peak_delta(|| Journal::open_with(small.path(), cfg()));

    // ── (1) ТЕРПИМОСТЬ: пол честный, декларация внутри занятого диапазона отвергнута ──
    for (res, max, label) in [(res_big, big_max, "BIG"), (res_small, small_max, "SMALL")] {
        let err = res.err().unwrap_or_else(|| {
            panic!(
                "{label}: декларация next_seq=1 при ЧИТАЕМОМ максимуме {max} ПРИНЯТА — \
                 escape-hatch стал каналом seq-reuse.\n\
                 ДОЛЖНО БЫТЬ: Err. Пол обязан учитывать читаемый ПРЕФИКС повреждённого \
                 сегмента (`Known`), а не схлопываться в 0 из-за того, что хвостовой скан \
                 вернул Err (R-002, Находка 1)."
            )
        });
        assert!(
            err.to_string().to_lowercase().contains("seq"),
            "{label}: отказ обязан объяснить причину: «{err}»"
        );
    }
    assert!(
        !ls(big.path()).iter().any(|n| n == DECL_APPLIED),
        "отвергнутая декларация не должна помечаться применённой"
    );

    // ── (2) ОГРАНИЧЕННОСТЬ: абсолютный бюджет — ловит полное чтение сегмента ─────────
    assert!(
        peak_big < THRESHOLD,
        "цена честного пола — {peak_big} B (> бюджета {THRESHOLD} B) на сегменте {BIG} B: \
         пол вычисляется ЗАГРУЗКОЙ сегмента в память (`read_segment_events`/`read_to_end`/\
         полная распаковка .zst).\n\
         Это класс TD-011: на проде сегмент 1 GiB, и это происходит РОВНО в момент разбора \
         инцидента, когда оператор поднимает recorder. Терпимость обязана быть куплена \
         потоковым сканом при постоянной памяти, а не размером RAM."
    );

    // ── (3) НЕЗАВИСИМОСТЬ ОТ РАЗМЕРА: ловит «читать долю файла» ─────────────────────
    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < INDEP_DELTA,
        "память вычисления пола РАСТЁТ с размером сегмента ({SMALL} B → {BIG} B: \
         +{growth} B, пики {peak_small} → {peak_big}) — значит она НЕ O(1).\n\
         Доля файла тоже не годится: 10% от 64 MiB выглядят прилично, но те же 10% от \
         прод-сегмента 1 GiB — это 100 MiB в момент инцидента."
    );
}
