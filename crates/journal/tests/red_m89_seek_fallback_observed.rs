//! RED `M-89` (sacred, architect-only) — **ОТКАТ поиска позиции НАБЛЮДАЕМ** (`§5.2` п. 7):
//! `EventStream::seek_fallbacks()` считает случаи «кандидат найден, гард точности не прошёл,
//! чтение откатилось к `header_end`». Без него «сдвиг не сработал и всё перечиталось» видно
//! только в `rchar`, а `ReadStats::seek_fallbacks` (`crates/gateway/src/lib.rs`) кормить нечем.
//!
//! # Форма (спека `M-89` §4.1, задана ДОСЛОВНО)
//!
//! ```ignore
//! impl EventStream {
//!     /// Число откатов поиска позиции к `header_end` за проход (0 — сдвиг не понадобился
//!     /// или сработал; `.zst` — не откат, а названный предел: поиск не предпринимается).
//!     pub fn seek_fallbacks(&self) -> u64;
//! }
//! ```
//!
//! # Что здесь пиннится
//!
//! · `f1` — кандидат с НЕВЕРНЫМ seq (дыра в нумерации: кадр `after+1` отсутствует, следующий
//!   несёт `after+2`) ⇒ гард `§5.2` п. 3 не проходит ⇒ `seek_fallbacks == 1`, и откат
//!   НАСТОЯЩИЙ (`events_scanned ≥ префикс`), а выдача равна независимому фильтру;
//! · `f2` — курсор за пределами журнала ⇒ откат, пусто;
//! · `f3`/`f4` — парный vantage: активный и закрытый сырой сегмент — `0` откатов и
//!   стоимость ∝ хвосту (счётчик-«крикун», считающий каждый проход откатом, краснеет);
//! · `f5` — `.zst`: поиск не предпринимается, откатом не считается (`0`).
//!
//! COMPILE-RED на ревизии набора: метода `seek_fallbacks` не существует.

mod common;

use std::path::Path;

use common::{append_bytes, cfg_with, frame_of, trade};
use contracts::Event;
use journal::{EpochFilter, Journal};

const SEEK_SCAN_ALLOWANCE: u64 = 64;
const BIG_SEG: u64 = 1 << 30;

fn setup_failed(what: &str) -> ! {
    panic!("SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о наблюдаемости отката.")
}

fn write(dir: &Path, from: u64, n: u64, seg: u64) {
    let mut j = Journal::open_with(dir, cfg_with(seg, "M-89 fallback"))
        .unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
    for i in from..from + n {
        j.append(trade(i))
            .unwrap_or_else(|e| setup_failed(&format!("append #{i}: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
}

fn active_segment(dir: &Path) -> std::path::PathBuf {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".jrnl"))
        .collect();
    v.sort();
    v.pop()
        .unwrap_or_else(|| setup_failed("нет сырого сегмента"))
}

fn last_seq(dir: &Path) -> u64 {
    let bytes = std::fs::read(dir.join("journal.meta")).expect("journal.meta");
    u64::from_le_bytes(bytes[..8].try_into().expect("8 байт")) - 1
}

/// (события, класс ошибки, events_scanned, seek_fallbacks)
fn run(dir: &Path, after: u64) -> (Vec<u64>, Option<std::io::ErrorKind>, u64, u64) {
    let mut s = journal::stream_from_at(dir, EpochFilter::OwnCaptureOnly, Some(after), None)
        .unwrap_or_else(|e| setup_failed(&format!("stream_from_at: {e}")));
    let mut out = Vec::new();
    let mut err = None;
    for item in s.by_ref() {
        match item {
            Ok(ev) => out.push(ev.seq),
            Err(e) => {
                err = Some(e.kind());
                break;
            }
        }
    }
    let scanned = s.events_scanned();
    let fallbacks = s.seek_fallbacks();
    (out, err, scanned, fallbacks)
}

fn independent(dir: &Path, after: u64) -> (Vec<u64>, Option<std::io::ErrorKind>) {
    let s = journal::stream(dir, EpochFilter::OwnCaptureOnly).expect("stream");
    let mut out = Vec::new();
    let mut err = None;
    for item in s {
        match item {
            Ok(ev) if ev.seq > after => out.push(ev.seq),
            Ok(_) => {}
            Err(e) => {
                err = Some(e.kind());
                break;
            }
        }
    }
    (out, err)
}

/// **f1 — кандидат с неверным seq.** Дыра в нумерации построена ВРУЧНУЮ (кадры seq
/// `PREFIX+1..` дописаны байтами, `PREFIX` пропущен): единственный способ получить кадр,
/// который бисекция найдёт, а гард `seq == after+1` отвергнет, не портя CRC.
#[test]
fn f1_wrong_seq_candidate_falls_back_observably_and_yields_independent_result() {
    const PREFIX: u64 = 3_000;
    const TAIL: u64 = 200;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, PREFIX, BIG_SEG);
    let seg = active_segment(dir.path());
    for seq in PREFIX + 1..PREFIX + 1 + TAIL {
        let ev = Event {
            seq,
            ts_mono_ns: 0,
            ts_wall_ms: common::T0,
            kind: trade(seq),
        };
        append_bytes(&seg, &frame_of(&ev));
    }
    let after = PREFIX - 1;
    let (want, want_err) = independent(dir.path(), after);
    if want.first() != Some(&(PREFIX + 1)) && want_err.is_none() {
        setup_failed("дыра в нумерации не построена: независимый путь не видит PREFIX+1 первым");
    }
    let (got, err, scanned, fallbacks) = run(dir.path(), after);
    assert_eq!(
        fallbacks, 1,
        "f1 / §5.2 п. 7: кадр after+1 отсутствует (следующий несёт after+2) — гард точности обязан \
         был отвергнуть кандидата и ОТКАТ обязан быть виден: seek_fallbacks = {fallbacks}"
    );
    assert!(
        scanned >= PREFIX,
        "f1: seek_fallbacks == 1, а events_scanned = {scanned} < префикс {PREFIX} — «откат» \
         засчитан без чтения с header_end: счётчик врёт о работе"
    );
    assert_eq!(
        (got, err),
        (want, want_err),
        "f1 / N2: выдача после отката ≠ независимому фильтру"
    );
}

/// **f2 — курсор за журналом** (восстановление из старой копии): кадра `after+1` нет вовсе.
#[test]
fn f2_cursor_beyond_journal_is_a_fallback() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, 3_000, BIG_SEG);
    let after = last_seq(dir.path()) + 10;
    let (got, err, _, fallbacks) = run(dir.path(), after);
    assert!(
        err.is_none() && got.is_empty(),
        "f2: за журналом выдано {} / {err:?}",
        got.len()
    );
    assert_eq!(
        fallbacks, 1,
        "f2: курсор за журналом — откат не наблюдаем (seek_fallbacks = {fallbacks})"
    );
}

/// **f3 — парный vantage, активный сегмент:** сдвиг сработал ⇒ 0 откатов И стоимость ∝ хвосту.
#[test]
fn f3_active_segment_seek_succeeds_with_zero_fallbacks() {
    const PREFIX: u64 = 10_000;
    const TAIL: u64 = 400;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, PREFIX + TAIL, BIG_SEG);
    let after = PREFIX - 1;
    let (got, err, scanned, fallbacks) = run(dir.path(), after);
    assert!(err.is_none());
    assert_eq!(got.len() as u64, TAIL);
    assert_eq!(
        fallbacks, 0,
        "f3: сдвиг на чистом активном сегменте засчитан откатом ({fallbacks})"
    );
    assert!(
        scanned <= TAIL + SEEK_SCAN_ALLOWANCE,
        "f3 / I-1: просканировано {scanned} при хвосте {TAIL}"
    );
}

/// **f4 — парный vantage, закрытый сырой сегмент.**
#[test]
fn f4_closed_segment_seek_succeeds_with_zero_fallbacks() {
    const SEG: u64 = 512 * 1024;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, 6_000, SEG);
    write(dir.path(), 6_000, 8_000, SEG);
    if journal::list_segments(dir.path()).expect("segments").len() != 2 {
        setup_failed("ротация не состоялась");
    }
    let (got, err, scanned, fallbacks) = run(dir.path(), 5_999);
    assert!(err.is_none());
    assert_eq!(got.len(), 8_000);
    assert_eq!(
        fallbacks, 0,
        "f4: сдвиг в закрытом сегменте засчитан откатом ({fallbacks})"
    );
    assert!(
        scanned <= 8_000 + SEEK_SCAN_ALLOWANCE,
        "f4 / I-1: просканировано {scanned}"
    );
}

/// **f5 — `.zst`: названный предел, не откат.** Поиск не предпринимается; корректность —
/// `red_m89_seek_contract::j8`.
#[test]
fn f5_compacted_segment_is_a_named_limit_not_a_fallback() {
    const SEG: u64 = 256 * 1024;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, 4_000, SEG);
    write(dir.path(), 4_000, 3_000, SEG);
    let segs = journal::list_segments(dir.path()).expect("segments");
    if segs.len() != 2 {
        setup_failed("ротация не состоялась");
    }
    journal::compact_segment(&segs[0], journal::DEFAULT_COMPACT_LEVEL)
        .unwrap_or_else(|e| setup_failed(&format!("компакция: {e}")));
    let (got, err, _, fallbacks) = run(dir.path(), 3_999);
    assert!(err.is_none());
    assert_eq!(got.len(), 3_000);
    assert_eq!(
        fallbacks, 0,
        "f5: чтение .zst целиком — предел, а не откат ({fallbacks})"
    );
}
