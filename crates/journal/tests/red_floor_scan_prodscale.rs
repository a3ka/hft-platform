//! SACRED (architect-only) — M-50 / **прод-масштаб: крупное событие ЗА префиксом больше
//! окна хвостового скана**.
//!
//! Прод-форма (замер R-002 + td-053): активный сегмент СЫРОЙ, сотни MB..1 GiB при окне
//! хвостового скана 4 MiB; крупное событие (архитектурный потолок `L2Snapshot` 66 032 B)
//! может оказаться ПОСЛЕДНИМ читаемым перед порчей. Скан пола обязан дойти до него через
//! префикс прод-масштаба и включить его seq в пол — при этом декларационный путь входится
//! именно потому, что окно хвостового скана не достаёт до валидных фреймов.
//!
//! Требование `.claude/rules/testing.md` §«Прод-масштаб для sacred I/O-путей»: фикстуры
//! в десятки KiB проверяют не ту ветку. Файл отдельный: пишет >5 MiB, гоняется --release
//! из `verify_M-50.sh` (образец op_5/ti_7).

mod common;

use common::{
    append_bytes, assert_decl_rejected, cfg_with, event_of_frame_size, frame_of, ls,
    tolerant_readable_max, trade, write_decl, DECL_APPLIED, PROD_L2SNAPSHOT_MAX_FRAME,
    TAIL_SCAN_CHUNK,
};
use journal::{Journal, WriterConfig};

const SEG: &str = "segment-00000000.jrnl";
const MAX_SEGMENT_BYTES: u64 = 64 * 1024 * 1024;
/// Сам ЧИТАЕМЫЙ ПРЕФИКС (до крупного события) — больше окна хвостового скана.
const PREFIX_TARGET: u64 = TAIL_SCAN_CHUNK + 3 * 1024 * 1024 / 2;
/// Мусорный хвост строго больше окна — окно обязано содержать ТОЛЬКО мусор.
const GARBAGE: usize = 4 * 1024 * 1024 + 512 * 1024;

const _: () = assert!(
    PREFIX_TARGET < MAX_SEGMENT_BYTES,
    "префикс не должен вызвать ротацию"
);
const _: () = assert!(
    PREFIX_TARGET > TAIL_SCAN_CHUNK,
    "префикс обязан превышать окно скана"
);
const _: () = assert!(
    GARBAGE as u64 > TAIL_SCAN_CHUNK,
    "мусор обязан перекрыть окно скана"
);

fn cfg() -> WriterConfig {
    cfg_with(MAX_SEGMENT_BYTES, "prodscale floor-scan fixture")
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-9 — пол видит крупное событие за прод-масштабным префиксом; честная декларация
// работает (парный vantage внутри того же теста, образец op_5)
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_9_floor_sees_large_event_behind_prodscale_prefix() {
    let dir = tempfile::tempdir().expect("dir");
    let mut n: u64 = 0;
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        loop {
            j.append(trade(n)).expect("append");
            n += 1;
            if n.is_multiple_of(20_000) {
                j.flush().expect("flush");
                let sz = std::fs::metadata(dir.path().join(SEG)).expect("meta").len();
                if sz > PREFIX_TARGET {
                    break;
                }
            }
            assert!(n < 20_000_000, "фикстура не набрала объём");
        }
        j.flush().expect("flush");
    }
    let seg = dir.path().join(SEG);
    let prefix_size = std::fs::metadata(&seg).expect("meta").len();
    assert!(
        prefix_size > TAIL_SCAN_CHUNK,
        "setup-guard: читаемый префикс {prefix_size} B обязан быть больше окна скана \
         {TAIL_SCAN_CHUNK} B — иначе проверяется не та ветка (урок rev4/ti_7)"
    );

    // Крупное событие — ПОСЛЕДНЕЕ читаемое; за ним порча до конца файла.
    let large_seq = n;
    append_bytes(
        &seg,
        &frame_of(&event_of_frame_size(large_seq, PROD_L2SNAPSHOT_MAX_FRAME)),
    );
    append_bytes(&seg, &vec![0x5A_u8; GARBAGE]);
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let measured = tolerant_readable_max(dir.path()).expect("префикс обязан читаться");
    assert_eq!(
        measured, large_seq,
        "setup-guard: эталон (без капа) обязан видеть крупное событие за префиксом"
    );

    // ── (1) декларация на занятый seq крупного события — отказ ──────────────────────
    assert_decl_rejected(
        dir.path(),
        cfg(),
        large_seq,
        "прод-масштаб: крупное событие за префиксом больше окна скана",
    );

    // ── (2) ПАРНЫЙ vantage: честная декларация обязана разблокировать старт ─────────
    let declared = large_seq + 1;
    write_decl(
        dir.path(),
        declared,
        "хвост невосстановим: холодной копии нет",
    );
    let mut j = Journal::open_with(dir.path(), cfg()).unwrap_or_else(|e| {
        panic!(
            "честная декларация next_seq={declared} (строго больше читаемого максимума \
             {measured}) обязана РАЗБЛОКИРОВАТЬ старт, получено Err: {e}"
        )
    });
    assert_eq!(
        j.next_seq(),
        declared,
        "запись обязана идти РОВНО с объявленной позиции"
    );
    j.append(trade(7_777_777)).expect("append");
    j.flush().expect("flush");
    drop(j);
    assert!(
        ls(dir.path()).iter().any(|n| n == DECL_APPLIED),
        "применённая декларация обязана быть помечена (одноразовость + аудит)"
    );
}
