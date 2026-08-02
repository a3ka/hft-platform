//! RED M-51 — **DET-I-1 на ПРОД-ФОРМЕ** (sacred, architect-only). Гонять `--release`.
//!
//! ## Форма прода — ЗАМЕРЕНА, не вспомнена (2026-08-01, ssh на VPS)
//!
//! ```text
//! $ ls -la /var/lib/docker/volumes/hft-platform_journal-data/_data
//!   segment-00000144.jrnl.zst   128 030 767
//!   segment-00000145.jrnl     1 073 739 765     <- сырые упираются в кап ~1 GiB
//!   ...
//!   segment-00000153.jrnl       109 291 629     <- активный
//! $ ls $D | grep -c '\.zst$'   ->  144      сжатых
//! $ du -sh $D                  ->  27G
//! $ cat $D/recorder.heartbeat
//!   {"events":4414898,"next_seq":145992262,"segment_index":153,...}
//! ```
//! Существенные черты, которые обязана воспроизвести фикстура:
//!  1. **МНОГО сегментов** (154), а не один;
//!  2. **СМЕШАННЫЙ формат в ОДНОМ каталоге** — 144 `.zst` + 10 сырых (не «сначала все
//!     сырые, потом все сжатые»: компакция догоняет хвост, обе формы сосуществуют всегда);
//!  3. **общий объём много больше окна хвостового скана** (4 MiB) и любого разумного буфера.
//!
//! ## Что здесь ЧЕСТНО не воспроизводится и почему это не дыра
//!
//! `next_seq` прода — 145 992 262 события / 27 GB. Оракул гоняет **подвыборку** (тысячи
//! событий, десятки MiB): полный прогон 27 GB в CI невозможен по времени и по диску. Мост от
//! подвыборки к проду — не «надеемся, что масштабируется», а `det_13`: **независимость пика
//! памяти от размера журнала**. Именно так уже закрыт TD-011
//! (`crates/journal/tests/red_open_bounded.rs`: «двух-размерная независимость — достаточный
//! перманентный guard»). Реализация, чей пик не растёт между 4 MiB и 16 MiB, — потоковая; на
//! 27 GB она отработает тем же кодом. Реализация через `read_all` провалит `det_13` здесь и
//! была бы неработоспособна на проде (класс TD-011: recorder уже переставал писать).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

mod common;

use common::{cfg_with, snap, TAIL_SCAN_CHUNK};

use journal::{EpochFilter, Journal};

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

/// Сегмент 1 MiB — тот же ПОРЯДОК числа сегментов, что на проде (десятки-сотни), при
/// вменяемом объёме фикстуры. Кап прода (1 GiB) воспроизводить незачем: проверяемое
/// свойство — сшивка МНОГИХ сегментов и двух форматов, а не конкретное число байт.
const SEG_BYTES: u64 = 1024 * 1024;

/// Построить журнал прод-ФОРМЫ: `target_bytes` данных, много сегментов, смешанный
/// raw/`.zst`. Возвращает (каталог, число событий).
fn build_prod_form(target_bytes: u64, keep_raw: u32) -> (tempfile::TempDir, u64) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut n = 0u64;
    {
        let mut j =
            Journal::open_with(dir.path(), cfg_with(SEG_BYTES, "det-prodscale")).expect("open");
        // `snap` ~2.4 KiB — прод-подобное крупное событие (L2Snapshot).
        while dir_bytes(dir.path()) < target_bytes {
            for i in 0..64u64 {
                j.append(snap(n + i)).expect("append");
            }
            n += 64;
            j.flush().expect("flush");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(dir.path(), keep_raw, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact");
    (dir, n)
}

fn dir_bytes(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .expect("read_dir")
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Setup-guard: фикстура ДЕЙСТВИТЕЛЬНО имеет прод-форму. Без него оракул мог бы «пройти» на
/// одном сыром сегменте и не проверить ничего (класс дефекта «идеальная фикстура»).
fn assert_prod_form(dir: &std::path::Path, min_segments: usize, min_bytes: u64) {
    let names = common::ls(dir);
    let segs: Vec<&String> = names
        .iter()
        .filter(|n| common::is_segment_name(n))
        .collect();
    let n_zst = segs.iter().filter(|n| n.ends_with(".jrnl.zst")).count();
    let n_raw = segs.len() - n_zst;
    assert!(
        segs.len() >= min_segments,
        "форма прода: ожидалось >= {min_segments} сегментов, а получилось {} — фикстура не \
         воспроизводит многосегментность (прод: 154)",
        segs.len()
    );
    assert!(
        n_zst > 0 && n_raw > 0,
        "форма прода: ожидалась СМЕШАННАЯ форма (сжатые + сырые в одном каталоге), а получено \
         zst={n_zst} raw={n_raw} — на проде замерено 144 .zst + 10 сырых, обе формы \
         сосуществуют всегда"
    );
    let bytes = dir_bytes(dir);
    assert!(
        bytes > min_bytes,
        "форма прода: объём {bytes} B не превысил требуемый порог {min_bytes} B — фикстура \
         слишком мала, чтобы отличить потоковый реплей от чтения целиком"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_12 — DET-I-1 держится на прод-форме (много сегментов, raw+.zst вперемешку).
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_12_replay_is_bit_identical_on_prod_form() {
    let (dir, n) = build_prod_form(16 * 1024 * 1024, 3);
    assert_prod_form(dir.path(), 10, TAIL_SCAN_CHUNK);

    let a = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None).expect("a");
    let b = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None).expect("b");
    assert_eq!(
        a.state_hash, b.state_hash,
        "DET-I-1: на прод-форме (много сегментов, смешанный raw/.zst) два реплея разошлись"
    );
    assert_eq!(
        a.events, n,
        "реплей обязан пройти ВСЕ события ({n}), а прошёл {}",
        a.events
    );

    // Окно, гарантированно пересекающее и границы сегментов, и границу форматов.
    let (from, to) = (
        a.first_seq.expect("first") + 100,
        a.last_seq.expect("last") - 100,
    );
    let w1 = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(from),
        Some(to),
    )
    .expect("w1");
    let w2 = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(from),
        Some(to),
    )
    .expect("w2");
    assert_eq!(
        w1.state_hash, w2.state_hash,
        "DET-I-1: окно на прод-форме недетерминировано"
    );
    assert_ne!(
        w1.state_hash, a.state_hash,
        "фикстура/реализация: окно обязано отличаться от полного журнала"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_13 — ГРАНИЦА РЕСУРСА: пик памяти реплея НЕ растёт с размером журнала.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_13_replay_memory_is_independent_of_journal_size() {
    // Мост «подвыборка → прод»: 27 GB / 146 млн событий в CI не прогнать. Проверяем СВОЙСТВО,
    // которое генерализуется на любой размер (тот же приём, что red_open_bounded для TD-011).
    let (small, _) = build_prod_form(4 * 1024 * 1024, 2);
    let (big, _) = build_prod_form(16 * 1024 * 1024, 3);
    // «Малый» журнал СПЕЦИАЛЬНО меньше окна хвостового скана — на контрасте с «большим»
    // (> окна) и строится вся проверка независимости от размера. Порог объёма к нему
    // неприменим, форма (много сегментов + оба формата) — обязательна.
    assert_prod_form(small.path(), 3, 0);
    assert_prod_form(big.path(), 10, TAIL_SCAN_CHUNK);

    let bytes_small = dir_bytes(small.path());
    let bytes_big = dir_bytes(big.path());
    assert!(
        bytes_big > bytes_small * 2,
        "фикстура: журналы обязаны РАЗЛИЧАТЬСЯ по объёму в разы ({bytes_big} против \
         {bytes_small}), иначе независимость от размера не проверяется"
    );

    let (ds, peak_small) = peak_delta(|| {
        journal::replay_digest(small.path(), EpochFilter::OwnCaptureOnly, None, None)
            .expect("small")
    });
    let (db, peak_big) = peak_delta(|| {
        journal::replay_digest(big.path(), EpochFilter::OwnCaptureOnly, None, None).expect("big")
    });
    assert_ne!(
        ds.state_hash, db.state_hash,
        "фикстура: журналы обязаны различаться"
    );

    // (1) Независимость от размера — ловит и полное чтение (растёт линейно), и «читать ДОЛЮ»
    //     (растёт медленнее, но растёт: 1/10 от 27 GB — это 2.7 GB на проде).
    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < 1024 * 1024,
        "DET-I-1/TD-011: пик аллокаций реплея вырос на {growth} B при росте журнала с \
         {bytes_small} B до {bytes_big} B — память НЕ O(1). На проде (27 GB, 146 млн событий) \
         такая реализация неработоспособна: ровно так recorder уже переставал писать (TD-011). \
         Реплей обязан быть ПОТОКОВЫМ (`stream`), а не `read_all`"
    );

    // (2) Абсолютный бюджет — вторая, независимая линия: ловит реализацию, которая одинаково
    //     много ест на обоих размерах (например, буфер «на весь сегмент» в 1 MiB × N).
    assert!(
        peak_big < 8 * 1024 * 1024,
        "DET-I-1/TD-011: пик аллокаций реплея {peak_big} B на журнале {bytes_big} B превысил \
         бюджет 8 MiB — реплей буферизует данные вместо потоковой свёртки"
    );
}
