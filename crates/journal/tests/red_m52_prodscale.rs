//! SACRED (architect-only) — M-52 на **ПРОД-ФОРМЕ**. Гонять `--release`.
//!
//! ## Форма прода — ЗАМЕРЕНА, не вспомнена (ssh на VPS, read-only, 2026-08-02)
//!
//! ```text
//! $ ls -la /var/lib/docker/volumes/hft-platform_journal-data/_data | tail -5
//!   segment-00000155.jrnl   1 073 731 461
//!   segment-00000156.jrnl   1 073 713 123
//!   segment-00000157.jrnl   1 073 739 468
//!   segment-00000158.jrnl      66 764 331     <- активный
//! $ ls $D | wc -l               -> 161   (158 сегментов + journal.meta + heartbeat + legacy)
//! $ ls $D | grep -c '\.zst$'    -> 152   сжатых
//! $ ls $D | grep -c '\.jrnl$'   ->   6   сырых
//! $ du -sh $D                   -> 26G   (сжатых; сырой объём ≈ 140 GiB)
//! ```
//!
//! Черты, которые обязана воспроизвести фикстура:
//!  1. **МНОГО сегментов** (158), а не один — обход каталога есть отдельное измерение
//!     стоимости (TD-052) и отдельная поверхность сшивки (TD-030);
//!  2. **СМЕШАННЫЙ формат в ОДНОМ каталоге** (152 `.zst` + 6 сырых) — компакция догоняет
//!     хвост, обе формы сосуществуют ВСЕГДА;
//!  3. **сегменты много больше окна хвостового скана** (4 MiB).
//!
//! ## Что здесь честно НЕ воспроизводится
//!
//! 26 GB / 148 млн событий в CI не прогнать. Мост от подвыборки к проду — тот же, что в
//! M-51 (`det_13`) и TD-011 (`red_open_bounded`): **независимость ресурса от размера**.
//! Здесь ресурсом является РАБОТА: реализация, чья работа ограничена бюджетом на 12
//! сегментах, ограничена им и на 158.

mod common;

use std::process::Command;

use common::{
    append_bytes, cfg_with, first_seqs, is_segment_name, lcg_garbage, ls, max_seq_in,
    open_with_deadline, swap_segment_files, write_decl, TAIL_SCAN_CHUNK,
};
use journal::{EpochFilter, Journal, WriterConfig};

const BIN: &str = env!("CARGO_BIN_EXE_journal-retention");

/// Размер, на котором цена скана ЗАМЕРЕНА reviewer'ом (16 MiB → 384.94 s после M-50).
const LCG_TAIL: usize = 16 * 1024 * 1024;
const CHEAP_TAIL: usize = 4 * 1024 * 1024 + 512 * 1024;
const CEILING_SECS: u64 = 60;
/// Потолок операторского прогона дайджеста на подвыборке — с запасом на медленный CI.
const DIGEST_CEILING_SECS: u64 = 120;

const _: () = assert!(CHEAP_TAIL as u64 > TAIL_SCAN_CHUNK, "фикстура: см. red_floor_work_budget");

fn cfg() -> WriterConfig {
    cfg_with(256 * 1024, "M-52 prodscale fixture")
}

fn seg_names(dir: &std::path::Path) -> Vec<String> {
    ls(dir).into_iter().filter(|n| is_segment_name(n)).collect()
}

/// Каталог прод-ФОРМЫ: ≥12 сегментов, СМЕШАННЫЙ формат (`.zst` + сырые в хвосте).
fn prodlike_dir(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..n {
            j.append(common::snap(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    // keep_raw=3 — как на проде: свежий хвост сырой, глубина сжата.
    journal::compact_closed_segments(dir.path(), 3, 3).expect("compact");
    let names = seg_names(dir.path());
    assert!(
        names.len() >= 12,
        "setup-guard: прод-форме нужно ≥12 сегментов, получено {}",
        names.len()
    );
    assert!(
        names.iter().any(|s| s.ends_with(".zst")) && names.iter().any(|s| s.ends_with(".jrnl")),
        "setup-guard: каталог обязан быть СМЕШАННЫМ (152 .zst + 6 сырых на проде): {names:?}"
    );
    dir
}

// ═════════════════════════════════════════════════════════════════════════════════════
// PS-1 (TD-052) — обход КАТАЛОГА ограничен, а не только один сегмент
// ═════════════════════════════════════════════════════════════════════════════════════

/// Хвостовой сегмент — сплошной равномерный мусор (валидных фреймов нет), поэтому пол
/// вынужден идти ГЛУБЖЕ по каталогу: измерение «стоимость обхода каталога», которого нет
/// в однофайловых оракулах. На проде таких сегментов 158.
#[test]
fn ps_1_floor_scan_over_a_prod_shaped_catalogue_is_bounded() {
    let dir = prodlike_dir(3_000);
    let names = seg_names(dir.path());
    let last = names.last().expect("хвост").clone();

    // Читаемый максимум каталога БЕЗ хвостового сегмента — эталон (его и обязан найти пол,
    // если бюджета хватит; если не хватит — обязан ответить Unknown, но не занизить).
    let prev_raw = names
        .iter()
        .rev()
        .skip(1)
        .find(|n| n.ends_with(".jrnl"))
        .expect("в хвосте есть сырой сегмент");
    let prev_max = max_seq_in(&std::fs::read(dir.path().join(prev_raw)).expect("read"))
        .expect("setup-guard: предыдущий сегмент читается");

    // Хвостовой сегмент: только заголовок + равномерный мусор ⇒ валидных фреймов нет.
    let last_path = dir.path().join(&last);
    let head = {
        let bytes = std::fs::read(&last_path).expect("read");
        bytes[..common::header_end(&bytes)].to_vec()
    };
    assert!(!head.is_empty(), "setup-guard: у хвостового сегмента обязан быть заголовок");
    std::fs::write(&last_path, &head).expect("truncate to header");
    append_bytes(&last_path, &lcg_garbage(LCG_TAIL, 0x5EED));
    append_bytes(&last_path, &vec![0x5A_u8; CHEAP_TAIL]);
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    write_decl(dir.path(), prev_max + 1, "хвостовой сегмент невосстановим");

    let outcome = open_with_deadline(dir.path(), cfg(), CEILING_SECS).unwrap_or_else(|| {
        panic!(
            "JR-I-10 НАРУШЕН на ПРОД-ФОРМЕ: обход каталога ({} сегментов, смешанный формат) \
             не уложился в {CEILING_SECS} s. На проде сегментов 158 и объём 26 GB сжатых \
             (≈140 GiB сырых) — то есть операторский выход M-49, ради которого тот шёл шесть \
             кругов, на боевом каталоге практически недоступен.",
            names.len()
        )
    });
    if let Ok(next) = outcome {
        assert!(
            next > prev_max,
            "JR-I-10 НАРУШЕН (fail-open на прод-форме): старт с next_seq={next} при читаемом \
             максимуме {prev_max} — частично просмотренный каталог дал ЗАНИЖЕННЫЙ пол"
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// PS-2 (TD-030) — guard монотонности на СМЕШАННОМ каталоге (включая `.zst`)
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn ps_2_monotonic_guard_holds_on_mixed_format_catalogue() {
    // (1) ПАРНЫЙ VANTAGE: здоровый смешанный каталог прод-формы читается целиком.
    let dir = prodlike_dir(3_000);
    let fs = first_seqs(dir.path());
    assert!(
        fs.windows(2).all(|w| w[0] < w[1]),
        "setup-guard: здоровая прод-форма обязана быть монотонной: {fs:?}"
    );
    let n = journal::stream(dir.path(), EpochFilter::All)
        .unwrap_or_else(|e| panic!("здоровая прод-форма обязана читаться: {e}"))
        .filter_map(|e| e.ok())
        .count();
    assert_eq!(n, 3_000, "здоровая прод-форма обязана отдать все события");

    // (2) Тот же каталог с переставленными СЖАТЫМИ сегментами: заголовок берётся из
    // zstd-потока, и guard обязан работать там ровно так же, как на сыром.
    let dir2 = prodlike_dir(3_000);
    let zst: Vec<String> = seg_names(dir2.path())
        .into_iter()
        .filter(|s| s.ends_with(".zst"))
        .collect();
    assert!(zst.len() >= 2, "setup-guard: нужно ≥2 сжатых сегмента");
    swap_segment_files(dir2.path(), &zst[0], &zst[zst.len() - 1]);
    let fs2 = first_seqs(dir2.path());
    assert!(
        fs2.windows(2).any(|w| w[0] >= w[1]),
        "setup-guard: перестановка обязана сломать монотонность: {fs2:?}"
    );
    match journal::stream(dir2.path(), EpochFilter::All) {
        Err(e) => assert!(
            e.to_string().contains("first_seq"),
            "диагностика обязана назвать нарушенное свойство: «{e}»"
        ),
        Ok(s) => {
            let seqs: Vec<u64> = s.filter_map(|e| e.ok()).map(|e| e.seq).take(8).collect();
            panic!(
                "JR-I-11 НАРУШЕН на прод-форме: немонотонный СМЕШАННЫЙ каталог сшит молча, \
                 первые seq {seqs:?}"
            );
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// PS-3 (TD-067) — дайджест из ДОСТАВЛЯЕМОГО бинаря на прод-форме
// ═════════════════════════════════════════════════════════════════════════════════════

/// Сшивка 12+ сегментов СМЕШАННОГО формата — ровно то, что дайджест обязан свернуть
/// одинаково на проде. Эталон — библиотечный `replay_digest` (M-51, `det_12`/`det_13`).
#[test]
fn ps_3_delivered_binary_digest_matches_library_on_prod_shape() {
    let dir = prodlike_dir(3_000);
    let lib = journal::replay_digest(dir.path(), EpochFilter::All, None, None).expect("digest");
    let hash: String = lib.state_hash.iter().map(|b| format!("{b:02x}")).collect();

    let d = dir.path().to_str().unwrap().to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = Command::new(BIN)
            .args([format!("--dir={d}"), "--mode=replay-digest".to_string()])
            .output()
            .expect("запуск journal-retention");
        let mut s = String::from_utf8_lossy(&out.stderr).into_owned();
        s.push_str(&String::from_utf8_lossy(&out.stdout));
        let _ = tx.send((out.status.code().unwrap_or(-1), s));
    });
    let (code, out) = rx
        .recv_timeout(std::time::Duration::from_secs(DIGEST_CEILING_SECS))
        .unwrap_or_else(|_| {
            panic!(
                "JR-I-12 НАРУШЕН: прогон дайджеста на прод-форме (12+ сегментов, смешанный \
                 формат) не уложился в {DIGEST_CEILING_SECS} s"
            )
        });

    assert_eq!(
        code, 0,
        "JR-I-12 НАРУШЕН: доставляемый бинарь не считает дайджест на прод-форме (exit={code}). \
         Именно этот прогон — обещанный TD-067 первый прод-замер: два запуска подряд с \
         предъявлением совпавшего state_hash на 26 GB / 148 млн событий.\n{out}"
    );
    assert!(
        out.contains(&hash) && out.contains(&format!("events={}", lib.events)),
        "дайджест из бинаря обязан совпасть с библиотечным (events={}, {hash}): «{out}»",
        lib.events
    );
}
