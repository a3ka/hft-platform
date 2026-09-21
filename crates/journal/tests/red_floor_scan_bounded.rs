//! SACRED (architect-only) — M-50 / **граница ПАМЯТИ верификации крупного фрейма**.
//!
//! ## Почему этот оракул существует
//!
//! Наивный «фикс» TD-053 — поднять `READABLE_SCAN_MAX_CARRY` до санити-капа ридера
//! (64 MiB) — чинит видимость крупных событий ценой НЕОГРАНИЧЕННОЙ памяти на операторском
//! пути: мусорная правдоподобная длина заставила бы carry копить десятки MiB, а буферизация
//! валидного крупного фрейма растёт с размером СОБЫТИЯ. Это класс TD-011 и прямой размен
//! «терпимость против памяти», запрещённый контрактом rev5 (op_8) и M-50 (JR-I-9).
//!
//! Контракт: верификация крупного кандидата обязана быть ПОТОКОВОЙ — CRC считается
//! инкрементально по чанкам, `seq` извлекается из ограниченного префикса payload
//! (первое поле `Event` — ведущий varint ≤ 10 B). Тело фрейма НЕ буферизуется целиком.
//!
//! ## Метод замера
//!
//! Счётчик аллокаций через global allocator (образец `red_open_bounded.rs` /
//! `red_tail_integrity_bounded.rs`, TD-011/TD-040: в бинаре РОВНО ОДИН #[test], замер
//! single-threaded по построению). Проверяются:
//!  (1) ЧЕСТНОСТЬ: декларация РОВНО на seq крупного события отвергнута (само крупное
//!      событие — 16 MiB `L2Delta`, архитектурно неограниченный вариант M-18);
//!  (2) абсолютный бюджет: пик на пути валидации < 8 MiB при событии 16 MiB — ловит
//!      буферизацию тела фрейма;
//!  (3) независимость от размера СОБЫТИЯ (пик(16 MiB) − пик(4 MiB) < 2 MiB) — ловит
//!      «буферизуем долю фрейма».
//! Анти-плацебо: `fs::read` большого сегмента обязан превысить бюджет — иначе замер слеп.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

mod common;

use common::{
    append_bytes, cfg_with, frame_of, l2delta_event_of_approx, ls, snap, tolerant_readable_max,
    write_decl, DECL_APPLIED, FLOOR_SCAN_CARRY_CAP, TAIL_SCAN_CHUNK,
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
    cfg_with(256 * 1024 * 1024, "bounded large-frame fixture")
}

const SEG: &str = "segment-00000000.jrnl";
/// Мусорный хвост строго больше окна хвостового скана — путь декларации обязан входиться.
const GARBAGE: usize = 4 * 1024 * 1024 + 512 * 1024;
/// Размеры КРУПНОГО валидного события (фрейм), между которыми меряется рост памяти.
const SMALL_EV: usize = 4 * 1024 * 1024;
const BIG_EV: usize = 16 * 1024 * 1024;
/// Бюджет пика на пути валидации декларации. Обязан быть МЕНЬШЕ BIG_EV (иначе замер не
/// ловит буферизацию тела) и БОЛЬШЕ окна хвостового скана (4 MiB — легитимный буфер
/// прод-пути, в замер попадает неизбежно).
const THRESHOLD: usize = 8 * 1024 * 1024;
const INDEP_DELTA: usize = 2 * 1024 * 1024;

const _: () = assert!(GARBAGE as u64 > TAIL_SCAN_CHUNK);
const _: () = assert!(
    THRESHOLD < BIG_EV,
    "бюджет обязан ловить буферизацию тела фрейма"
);
const _: () = assert!(
    THRESHOLD as u64 > TAIL_SCAN_CHUNK,
    "бюджет не может быть меньше легитимного окна хвостового скана"
);

/// Каталог: мелкий префикс (journal) + ОДНО крупное валидное событие (~ev_frame B, ручной
/// фрейм) + мусорный хвост; меты нет. Возвращает (каталог, seq крупного события).
fn fixture(ev_frame: usize) -> (tempfile::TempDir, u64) {
    let dir = tempfile::tempdir().expect("dir");
    let prefix: u64 = 256;
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..prefix {
            j.append(snap(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let seg = dir.path().join(SEG);
    let fb = frame_of(&l2delta_event_of_approx(prefix, ev_frame));
    assert!(
        fb.len() > FLOOR_SCAN_CARRY_CAP,
        "setup-guard: событие обязано быть крупнее капа carry, иначе фикстура не давит"
    );
    assert!(
        fb.len().abs_diff(ev_frame) < ev_frame / 20,
        "setup-guard: фрейм {} B слишком далёк от цели {ev_frame} B",
        fb.len()
    );
    append_bytes(&seg, &fb);
    append_bytes(&seg, &vec![0x5A_u8; GARBAGE]);
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let measured = tolerant_readable_max(dir.path()).expect("префикс обязан читаться");
    assert_eq!(
        measured, prefix,
        "setup-guard: эталон (без капа) обязан видеть крупное событие"
    );
    (dir, prefix)
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-8 — пол видит крупное событие И цена этого знания ограничена по памяти
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_8_large_frame_floor_is_honest_and_memory_bounded() {
    let (small, small_seq) = fixture(SMALL_EV);
    let (big, big_seq) = fixture(BIG_EV);

    // Анти-плацебо: полное чтение большого сегмента обязано превысить бюджет.
    let (_, peak_full) = peak_delta(|| std::fs::read(big.path().join(SEG)).expect("read"));
    assert!(
        peak_full > THRESHOLD,
        "контроль: полное чтение сегмента ({peak_full} B) обязано превышать бюджет \
         {THRESHOLD} B — иначе замер ничего не доказывает"
    );

    // ── (1) ЧЕСТНОСТЬ: декларация РОВНО на seq крупного события — внутри диапазона ──
    write_decl(
        big.path(),
        big_seq,
        "ошибка оператора: seq крупного события занят",
    );
    let (res_big, peak_big) = peak_delta(|| Journal::open_with(big.path(), cfg()));
    write_decl(
        small.path(),
        small_seq,
        "ошибка оператора: seq крупного события занят",
    );
    let (res_small, peak_small) = peak_delta(|| Journal::open_with(small.path(), cfg()));

    for (res, seq, ev, label) in [
        (res_big, big_seq, BIG_EV, "BIG"),
        (res_small, small_seq, SMALL_EV, "SMALL"),
    ] {
        let err = res.err().unwrap_or_else(|| {
            panic!(
                "{label}: JR-I-9 НАРУШЕН — декларация next_seq={seq} ПРИНЯТА, хотя seq \
                 занят валидным событием {ev} B (последним читаемым). Скан пола молча \
                 трактует крупный фрейм как порчу (TD-053) → seq-reuse.\n\
                 ДОЛЖНО БЫТЬ: Err — крупный кандидат верифицируется потоково (CRC + \
                 seq-префикс) и участвует в поле."
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

    // ── (2) БЮДЖЕТ: тело крупного фрейма НЕ буферизуется ────────────────────────────
    assert!(
        peak_big < THRESHOLD,
        "цена знания о крупном событии — {peak_big} B (> бюджета {THRESHOLD} B) при \
         событии {BIG_EV} B: верификация БУФЕРИЗУЕТ тело фрейма вместо потокового CRC.\n\
         Это возврат класса TD-011 на операторском пути и запрещённый размен \
         «терпимость против памяти» (rev5/op_8, M-50/JR-I-9)."
    );

    // ── (3) НЕЗАВИСИМОСТЬ ОТ РАЗМЕРА СОБЫТИЯ ────────────────────────────────────────
    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < INDEP_DELTA,
        "память верификации РАСТЁТ с размером события ({SMALL_EV} B → {BIG_EV} B: \
         +{growth} B, пики {peak_small} → {peak_big}) — значит буферизуется доля тела \
         фрейма. `L2Delta` архитектурно не ограничен: доля от неограниченного — \
         неограниченна."
    );
}
