//! RED `M-89` (sacred, architect-only) — **ПУБЛИЧНЫЙ ДОГОВОР сдвига к позиции курсора**:
//! `journal::stream_from_at(dir, filter, Some(after), hint = None)` (и делегат
//! `stream_from_at_with_catalog`) отдаёт РОВНО события `seq > after`, начиная с `after + 1`,
//! и стоит `O(хвост) + O(log)` проб, а не `O(префикс сегмента, содержащего курсор)`.
//!
//! # Почему договор — на публичной точке, а не на примитиве (`C-260` R2)
//!
//! Первая редакция набора объявляла `pub fn locate_after_seq` и проверяла его ГРЕПОМ
//! сигнатуры. Критик предъявил проходящий мутант: публичная функция-заглушка `Ok(None)` +
//! приватный поиск в `LiveReducer` — все RED зелены, публичное API мертво. Решение круга 2:
//! примитив поиска — `pub(crate)`, имя не контрактно; ПУБЛИЧНЫЙ договор — поведение
//! `stream_from_at`/`stream_from_at_with_catalog` при `hint = None, after_seq = Some(..)`.
//! Оракул зовёт ИМЕННО эту точку — обойти её реализация не может, потому что она и есть
//! прод-путь (`LiveReducer::pump` → `stream_from_at_with_catalog`).
//!
//! # Инварианты
//!
//! · `I-1` — `events_scanned ≤ хвост + SEEK_SCAN_ALLOWANCE` (`j1`, `j2`, `j3`, `j6`, `j7`);
//! · `I-2` / `JR-I-2` — первое выданное событие несёт РОВНО `after + 1`; порча ⇒ `Err` до
//!   выдачи чего-либо за ней, как у независимого `stream()` (`j5`, `j5b`); рваный кадр —
//!   не порча, а «писатель дописывает» (`j6`); ничего не пишется (`j7`);
//! · `N2` — эквивалентность без потерь: `stream_from_at(dir, Some(a), None)` ≡ независимый
//!   фильтр `stream(dir)` по `seq > a` на ВСЕХ вырожденных входах (`j8`); `after = None` ≡
//!   полный проход;
//! · вторая валидация hint'а (`resolve_active_start_offset`, условия 1–6) сохранена (`j9`).
//!
//! # Эталон — НЕЗАВИСИМЫЙ путь
//!
//! `journal::stream(dir, filter)` — полный проход без `after_seq`, без hint, без каталога;
//! фильтр `seq > after` применяется В ТЕСТЕ. Он не делит с предметом ни выбор сегмента,
//! ни позиционирование (`testing.md` §«Зависимый эталон мутация ловит плохо»).
//!
//! # Чего оракул НЕ ловит — названо
//!
//! · Наблюдаемость ОТКАТА (`seek_fallbacks`) — `red_m89_seek_fallback_observed.rs`
//!   (COMPILE-RED: счётчика ещё нет);
//! · байты — `red_m89_bytes_accounting.rs`; транспорт — `crates/gateway-serve/tests/red_m89_*`;
//! · `.zst` — названный предел: только корректность (`j8`), стоимость не судится.
//!
//! RUNTIME-RED на ревизии набора: `j1`, `j2`, `j3`, `j6`, `j7` красны по СТОИМОСТИ
//! (`events_scanned = префикс + хвост`). `j4`, `j5`, `j5b`, `j8`, `j9` — зелены сегодня и
//! названы стражами (fail-closed, эквивалентность, валидация hint'а).

mod common;

use std::io;
use std::path::Path;

use common::{append_bytes, cfg_with, frame_of, header_end, trade};
use contracts::{Event, SEGMENT_MAGIC};
use journal::{EpochFilter, Journal, TailHint, WriterConfig};

/// Допуск на поиск позиции: `≤ 2·⌈log2(файл / 64 КиБ)⌉ + 4` декодированных кадров
/// (спека §5.2 п. 1) — при файле до 2^30 · 64 КиБ это ≤ 64. Дефектная реализация даёт
/// `events_scanned = N + хвост`.
const SEEK_SCAN_ALLOWANCE: u64 = 64;
const BIG_SEG: u64 = 1 << 30;

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о сдвиге: фикстура не воспроизвела \
         сценарий, ради которого оракул написан."
    )
}

fn cfg(seg: u64) -> WriterConfig {
    cfg_with(seg, "M-89 seek contract")
}

fn write(dir: &Path, from: u64, n: u64, seg: u64) {
    let mut j = Journal::open_with(dir, cfg(seg))
        .unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
    for i in from..from + n {
        j.append(trade(i))
            .unwrap_or_else(|e| setup_failed(&format!("append #{i}: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
}

fn segment_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            n.ends_with(".jrnl") || n.ends_with(".jrnl.zst")
        })
        .collect();
    v.sort();
    v
}

fn last_seq(dir: &Path) -> u64 {
    let bytes = std::fs::read(dir.join("journal.meta")).expect("journal.meta");
    u64::from_le_bytes(bytes[..8].try_into().expect("8 байт")) - 1
}

/// Прогнать стрим до конца ИЛИ до первой ошибки: (события, ошибка, events_scanned).
fn drain(mut s: journal::EventStream) -> (Vec<Event>, Option<io::Error>, u64) {
    let mut out = Vec::new();
    let mut err = None;
    for item in s.by_ref() {
        match item {
            Ok(ev) => out.push(ev),
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    let scanned = s.events_scanned();
    (out, err, scanned)
}

/// ПРЕДМЕТ: публичная точка при `hint = None`.
fn subject(dir: &Path, after: Option<u64>) -> (Vec<Event>, Option<io::Error>, u64) {
    let s = journal::stream_from_at(dir, EpochFilter::OwnCaptureOnly, after, None)
        .unwrap_or_else(|e| setup_failed(&format!("stream_from_at: {e}")));
    drain(s)
}

/// ЭТАЛОН: полный проход + фильтр в тесте.
fn independent(dir: &Path, after: Option<u64>) -> (Vec<Event>, Option<io::Error>) {
    let s = journal::stream(dir, EpochFilter::OwnCaptureOnly)
        .unwrap_or_else(|e| setup_failed(&format!("stream: {e}")));
    let (all, err, _) = drain(s);
    let kept = match after {
        Some(a) => all.into_iter().filter(|e| e.seq > a).collect(),
        None => all,
    };
    (kept, err)
}

fn seqs(v: &[Event]) -> Vec<u64> {
    v.iter().map(|e| e.seq).collect()
}

fn assert_exact_tail(tag: &str, got: &[Event], after: u64, tail: u64) {
    assert_eq!(
        got.first().map(|e| e.seq),
        if tail > 0 { Some(after + 1) } else { None },
        "{tag} / I-2 (JR-I-2): первое выданное событие ≠ after+1 = {}",
        after + 1
    );
    assert_eq!(
        got.len() as u64,
        tail,
        "{tag}: выдано {} при хвосте {tail}",
        got.len()
    );
    for (k, ev) in got.iter().enumerate() {
        assert_eq!(
            ev.seq,
            after + 1 + k as u64,
            "{tag}: seq не подряд на позиции {k}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j1 — сырой АКТИВНЫЙ сегмент: точный after+1, стоимость ∝ хвосту
// ═══════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn j1_active_raw_segment_yields_exactly_after_plus_one_at_tail_cost() {
    const PREFIX: u64 = 20_000;
    const TAIL: u64 = 500;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, PREFIX + TAIL, BIG_SEG);
    let segs = segment_files(dir.path());
    if segs.len() != 1 {
        setup_failed(&format!(
            "сегментов {}, прод-форма — один активный",
            segs.len()
        ));
    }
    let after = PREFIX - 1;
    let (got, err, scanned) = subject(dir.path(), Some(after));
    assert!(err.is_none(), "j1: ошибка на чистом журнале: {err:?}");
    assert_exact_tail("j1", &got, after, TAIL);
    assert!(
        scanned <= TAIL + SEEK_SCAN_ALLOWANCE,
        "j1 / I-1: stream_from_at(after={after}, hint=None) ПРОСКАНИРОВАЛ {scanned} событий при \
         хвосте {TAIL} (допуск {SEEK_SCAN_ALLOWANCE}). Активный сегмент читается с header_end: \
         `resolve_active_start_offset` без hint возвращает начало (`crates/journal/src/segments.rs`). \
         На проде это 305 тыс. событий / ≈412 МБ на КАЖДУЮ новую подписку (замер 2026-09-27)."
    );
    let (want, _) = independent(dir.path(), Some(after));
    assert_eq!(
        seqs(&got),
        seqs(&want),
        "j1 / N2: расхождение с независимым фильтром"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j2 — курсор в ЗАКРЫТОМ сыром сегменте, хвост пересекает ротацию
// ═══════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn j2_closed_raw_segment_seeks_too_not_only_active() {
    const SEG: u64 = 512 * 1024;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, 6_000, SEG); // ~300 КиБ — сегмент 0 ещё активен
    write(dir.path(), 6_000, 8_000, SEG); // переполняет сегмент 0 ⇒ ротация
    let segs = journal::list_segments(dir.path()).expect("segments");
    if segs.len() != 2 {
        setup_failed(&format!("сегментов {} — ротация не состоялась", segs.len()));
    }
    let after = 5_999u64;
    if !(segs[0].header.first_seq <= after && after < segs[1].header.first_seq) {
        setup_failed("курсор не внутри ЗАКРЫТОГО сегмента 0");
    }
    let (got, err, scanned) = subject(dir.path(), Some(after));
    assert!(err.is_none(), "j2: {err:?}");
    assert_exact_tail("j2", &got, after, 8_000);
    assert!(
        scanned <= 8_000 + SEEK_SCAN_ALLOWANCE,
        "j2 / I-1 (§5.2 п. 5): курсор в закрытом сыром сегменте — просканировано {scanned} при \
         хвосте 8 000. Закрытый raw читается `Passive` с начала (`open_next_segment`); сдвиг \
         обязан применяться к СЕГМЕНТУ, СОДЕРЖАЩЕМУ КУРСОР, а не только к активному."
    );
    let (want, _) = independent(dir.path(), Some(after));
    assert_eq!(seqs(&got), seqs(&want), "j2 / N2");
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j3 — пустой хвост на EOF: ни одного кадра, ни одного лишнего скана
// ═══════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn j3_empty_tail_at_eof_reads_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, 5_000, BIG_SEG);
    let after = last_seq(dir.path());
    let (got, err, scanned) = subject(dir.path(), Some(after));
    assert!(err.is_none(), "j3: {err:?}");
    assert!(got.is_empty(), "j3: пустой хвост дал {} событий", got.len());
    assert!(
        scanned <= SEEK_SCAN_ALLOWANCE,
        "j3 / I-1 (§5.2 п. 3, «хвост пуст»): просканировано {scanned} — сегмент перечитан, чтобы \
         убедиться в пустоте; легитимная позиция `pos == len` (условие 6 \
         `resolve_active_start_offset`) не найдена"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j4 — курсор ЗА пределами журнала (восстановление из старой копии): пусто, без ошибки
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Страж (зелен сегодня): кандидат с неверным seq (кадра `after+1` не существует) не смеет
/// ничего выдать. Наблюдаемость отката — `red_m89_seek_fallback_observed.rs::f2`.
#[test]
fn j4_cursor_beyond_journal_yields_nothing_without_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, 3_000, BIG_SEG);
    let after = last_seq(dir.path()) + 10;
    let (got, err, _) = subject(dir.path(), Some(after));
    assert!(err.is_none(), "j4: {err:?}");
    assert!(
        got.is_empty(),
        "j4: курсор за журналом, а выдано {} событий",
        got.len()
    );
    let (want, _) = independent(dir.path(), Some(after));
    assert!(want.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j5 / j5b — порча: fail-closed РОВНО как независимый путь
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Инвертировать 8 байт payload'а кадра с `seq == target` (независимый проход по кадрам).
fn corrupt_frame_of_seq(seg: &Path, target: u64) {
    let mut bytes = std::fs::read(seg).expect("read segment");
    let mut i = header_end(&bytes);
    if i == 0 {
        setup_failed("сегмент без заголовка v2");
    }
    let mut hit = None;
    while i + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        if let Ok(ev) = postcard::from_bytes::<Event>(&bytes[i + 4..i + 4 + len]) {
            if ev.seq == target {
                hit = Some(i + 4);
                break;
            }
        }
        i += 8 + len;
    }
    let p = hit.unwrap_or_else(|| setup_failed(&format!("кадр seq={target} не найден")));
    for b in bytes.iter_mut().skip(p).take(8) {
        *b = !*b;
    }
    std::fs::write(seg, &bytes).expect("write corrupted");
}

fn corrupted_fixture(k: u64) -> (tempfile::TempDir, u64) {
    const PREFIX: u64 = 5_000;
    const TAIL: u64 = 500;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, PREFIX + TAIL, BIG_SEG);
    let after = PREFIX - 1;
    let seg = segment_files(dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    corrupt_frame_of_seq(&seg, after + 1 + k);
    let (_, ind_err) = independent(dir.path(), Some(after));
    if ind_err.is_none() {
        setup_failed("порча не ловится независимым путём — фикстура испортила не кадр");
    }
    (dir, after)
}

/// **j5 — испорчен кадр `after+1`.** Гард точности не проходит ⇒ откат ⇒ чтение с
/// `header_end` упирается в порчу ⇒ `Err`, и НИ ОДНОГО события за курсором до неё.
#[test]
fn j5_corrupt_frame_at_after_plus_one_fails_closed_like_independent_path() {
    let (dir, after) = corrupted_fixture(0);
    let (got, err, _) = subject(dir.path(), Some(after));
    assert!(
        err.is_some(),
        "j5 / I-2 (JR-I-2): испорченный кадр after+1 ПРОГЛОЧЕН — выдано {} событий без ошибки; \
         ресинк перепрыгнул порчу, независимый путь на том же журнале отказывает",
        got.len()
    );
    assert!(
        got.is_empty(),
        "j5: до ошибки выдано {} событий за курсором",
        got.len()
    );
}

/// **j5b — порча в СЕРЕДИНЕ хвоста (`TAIL/2`, `C-260` N1).** Выдаётся ровно валидный
/// префикс хвоста, затем `Err`; те же события и та же ошибка, что у независимого пути.
#[test]
fn j5b_corrupt_frame_mid_tail_yields_prefix_then_err_like_independent_path() {
    let (dir, after) = corrupted_fixture(250);
    let (got, err, _) = subject(dir.path(), Some(after));
    let (want, want_err) = independent(dir.path(), Some(after));
    assert!(
        err.is_some(),
        "j5b / I-2: порча в середине хвоста проглочена ({} событий)",
        got.len()
    );
    assert_eq!(
        seqs(&got),
        seqs(&want),
        "j5b: префикс до порчи ≠ независимый (got {} / want {})",
        got.len(),
        want.len()
    );
    assert_eq!(
        got.len(),
        250,
        "j5b: до порчи обязано выдаться ровно 250 событий"
    );
    assert_eq!(
        err.map(|e| e.kind()),
        want_err.map(|e| e.kind()),
        "j5b: класс ошибки ≠ независимому пути"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j6 — рваный целевой кадр: не порча, а «писатель дописывает»
// ═══════════════════════════════════════════════════════════════════════════════════════════

fn torn_frame_for(seq: u64) -> (Vec<u8>, usize) {
    let ev = Event {
        seq,
        ts_mono_ns: 0,
        ts_wall_ms: common::T0,
        kind: trade(seq),
    };
    let frame = frame_of(&ev);
    let cut = frame.len() / 2;
    (frame, cut)
}

#[test]
fn j6_torn_target_frame_is_deferred_not_error_and_not_rescanned() {
    const PREFIX: u64 = 5_000;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, PREFIX, BIG_SEG);
    let seg = segment_files(dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let (frame, cut) = torn_frame_for(PREFIX);
    let before = std::fs::metadata(&seg).expect("meta").len();
    append_bytes(&seg, &frame[..cut]);
    if std::fs::metadata(&seg).expect("meta").len() != before + cut as u64 {
        setup_failed("рваный кадр не дописан");
    }

    // (а) after = последнее целое событие: целевой кадр рваный ⇒ пусто, БЕЗ ошибки, БЕЗ перескана.
    let after = PREFIX - 1;
    let (got, err, scanned) = subject(dir.path(), Some(after));
    assert!(
        err.is_none(),
        "j6 / §5.4: рваный кадр принят за порчу — {err:?}"
    );
    assert!(
        got.is_empty(),
        "j6: из рваного кадра выдано {} событий",
        got.len()
    );
    assert!(
        scanned <= SEEK_SCAN_ALLOWANCE,
        "j6 / §5.4: рваный целевой кадр сорвал чтение в полный перескан — просканировано {scanned} \
         (сегодня `probe_frame_boundary` на неполном кадре даёт false ⇒ откат в header_end)"
    );

    // (б) after = за три события до конца: три целых кадра выданы, рваный отложен.
    let after3 = PREFIX - 4;
    let (got3, err3, scanned3) = subject(dir.path(), Some(after3));
    assert!(err3.is_none(), "j6б: {err3:?}");
    assert_exact_tail("j6б", &got3, after3, 3);
    assert!(
        scanned3 <= 3 + SEEK_SCAN_ALLOWANCE,
        "j6б: просканировано {scanned3} при хвосте 3"
    );

    // (в) писатель дописал остаток — кадр читается РОВНО один раз, результат = независимому.
    append_bytes(&seg, &frame[cut..]);
    let (got_after, err_after, _) = subject(dir.path(), Some(after));
    assert!(err_after.is_none(), "j6в: {err_after:?}");
    assert_exact_tail("j6в", &got_after, after, 1);
    let (want, _) = independent(dir.path(), Some(after));
    assert_eq!(seqs(&got_after), seqs(&want), "j6в / N2");
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j7 — каталог ТОЛЬКО ДЛЯ ЧТЕНИЯ (прод: `journal-data:/journal:ro`)
// ═══════════════════════════════════════════════════════════════════════════════════════════

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .map(|s| {
            s.lines()
                .any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0"))
        })
        .unwrap_or(false)
}

fn listing(dir: &Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                e.metadata().map(|m| m.len()).unwrap_or(0),
            )
        })
        .collect();
    v.sort();
    v
}

fn set_mode(dir: &Path, dir_mode: u32, file_mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if dir_mode > 0o555 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(dir_mode))
            .expect("chmod dir");
    }
    for e in std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
    {
        std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(file_mode))
            .expect("chmod");
    }
    if dir_mode <= 0o555 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(dir_mode))
            .expect("chmod dir");
    }
}

#[test]
fn j7_read_only_catalog_seeks_and_writes_nothing() {
    if is_root() {
        setup_failed(
            "тест идёт под root — права носителя не применяются, сценарий не воспроизводим",
        );
    }
    const PREFIX: u64 = 5_000;
    const TAIL: u64 = 500;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, PREFIX + TAIL, BIG_SEG);
    let before = listing(dir.path());
    set_mode(dir.path(), 0o555, 0o444);
    let after = PREFIX - 1;
    let res = std::panic::catch_unwind(|| subject(dir.path(), Some(after)));
    let after_listing = listing(dir.path());
    set_mode(dir.path(), 0o755, 0o644); // вернуть права ДО утверждений — иначе TempDir не уберётся
    let (got, err, scanned) = res.unwrap_or_else(|_| panic!("j7: сдвиг на :ro-каталоге паникует"));
    assert!(
        err.is_none(),
        "j7 / §5.2 п. 6: на :ro-каталоге сдвиг вернул ошибку — {err:?}"
    );
    assert_exact_tail("j7", &got, after, TAIL);
    assert!(
        scanned <= TAIL + SEEK_SCAN_ALLOWANCE,
        "j7 / I-1: на :ro-каталоге просканировано {scanned} при хвосте {TAIL}"
    );
    assert_eq!(
        before, after_listing,
        "j7: опись каталога изменилась — сдвиг что-то записал (sidecar/индекс)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j8 — N2: эквивалентность без потерь на ВСЕХ вырожденных входах
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Страж (зелен сегодня, краснеет против любого сдвига, теряющего/дублирующего события):
/// для каждой фикстуры и каждого `after` предмет ≡ независимый фильтр; `None` ≡ `stream()`.
#[test]
fn j8_stream_from_at_equals_independent_filter_on_all_degenerate_inputs() {
    let mut fixtures: Vec<(&str, tempfile::TempDir)> = Vec::new();

    let active = tempfile::tempdir().expect("tempdir");
    write(active.path(), 0, 3_300, BIG_SEG);
    fixtures.push(("active", active));

    let closed = tempfile::tempdir().expect("tempdir");
    write(closed.path(), 0, 6_000, 512 * 1024);
    write(closed.path(), 6_000, 8_000, 512 * 1024);
    if journal::list_segments(closed.path())
        .expect("segments")
        .len()
        != 2
    {
        setup_failed("closed: ротация не состоялась");
    }
    fixtures.push(("closed", closed));

    let zst = tempfile::tempdir().expect("tempdir");
    write(zst.path(), 0, 4_000, 256 * 1024);
    write(zst.path(), 4_000, 3_000, 256 * 1024);
    let segs = journal::list_segments(zst.path()).expect("segments");
    if segs.len() != 2 {
        setup_failed("zst: ротация не состоялась");
    }
    journal::compact_segment(&segs[0], journal::DEFAULT_COMPACT_LEVEL)
        .unwrap_or_else(|e| setup_failed(&format!("компакция: {e}")));
    if !segment_files(zst.path())
        .iter()
        .any(|p| p.to_string_lossy().ends_with(".jrnl.zst"))
    {
        setup_failed("zst: после компакции нет .zst");
    }
    fixtures.push(("zst", zst));

    let torn = tempfile::tempdir().expect("tempdir");
    write(torn.path(), 0, 2_000, BIG_SEG);
    let seg = segment_files(torn.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let (frame, cut) = torn_frame_for(2_000);
    append_bytes(&seg, &frame[..cut]);
    fixtures.push(("torn", torn));

    for (tag, dir) in &fixtures {
        let last = last_seq(dir.path());
        let firsts: Vec<u64> = journal::list_segments(dir.path())
            .expect("segments")
            .iter()
            .map(|s| s.header.first_seq)
            .collect();
        let mut afters = vec![0, 1, last / 2, last - 1, last, last + 5];
        for f in firsts {
            afters.extend([f.saturating_sub(1), f, f + 1]);
        }
        afters.sort_unstable();
        afters.dedup();
        for a in afters {
            let (got, err, _) = subject(dir.path(), Some(a));
            let (want, want_err) = independent(dir.path(), Some(a));
            assert!(
                err.is_none() && want_err.is_none(),
                "{tag}/after={a}: ошибка на чистом журнале"
            );
            assert_eq!(
                seqs(&got),
                seqs(&want),
                "{tag} / N2: stream_from_at(after={a}) ≠ stream() ∩ seq>{a} (got {} / want {})",
                got.len(),
                want.len()
            );
            assert_eq!(
                got, want,
                "{tag}/after={a}: события равны по seq, но не по содержимому"
            );
        }
        let (full, e1, _) = subject(dir.path(), None);
        let (want_full, e2) = independent(dir.path(), None);
        assert!(e1.is_none() && e2.is_none());
        assert_eq!(
            full, want_full,
            "{tag} / §10: after=None обязан оставаться полным проходом"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// j9 — вторая валидация hint'а сохранена
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Страж: hint с позицией СРЕДИ кадра (не на границе) и с `last_seq == after` — условие 6
/// `resolve_active_start_offset` обязано отвергнуть его (или сдвиг по seq — найти верную
/// позицию); итог в любом случае равен независимому фильтру. Реализация задачи 1 не вправе
/// снять эту валидацию, заменив её собственным поиском.
#[test]
fn j9_hint_validation_is_preserved_bad_pos_never_skips_events() {
    const PREFIX: u64 = 3_000;
    const TAIL: u64 = 300;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), 0, PREFIX + TAIL, BIG_SEG);
    let seg = segment_files(dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let bytes = std::fs::read(&seg).expect("read");
    if !bytes.starts_with(&SEGMENT_MAGIC) {
        setup_failed("сегмент без магии v2");
    }
    // Независимо найти границу кадра `after+1` и сдвинуть её на 1 байт внутрь кадра.
    let after = PREFIX - 1;
    let mut i = header_end(&bytes);
    let mut boundary = None;
    while i + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        if let Ok(ev) = postcard::from_bytes::<Event>(&bytes[i + 4..i + 4 + len]) {
            if ev.seq == after + 1 {
                boundary = Some(i as u64);
                break;
            }
        }
        i += 8 + len;
    }
    let boundary = boundary.unwrap_or_else(|| setup_failed("граница кадра after+1 не найдена"));
    let seg_idx = journal::list_segments(dir.path()).expect("segments")[0].index;
    let bad = TailHint {
        seg_idx,
        last_seq: after,
        pos: boundary + 1,
    };
    let s = journal::stream_from_at(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(after),
        Some(bad),
    )
    .unwrap_or_else(|e| setup_failed(&format!("stream_from_at: {e}")));
    let (got, err, _) = drain(s);
    assert!(
        err.is_none(),
        "j9: невалидный hint дал ошибку вместо отката — {err:?}"
    );
    assert_exact_tail("j9", &got, after, TAIL);
    let (want, _) = independent(dir.path(), Some(after));
    assert_eq!(
        seqs(&got),
        seqs(&want),
        "j9: невалидный hint пропустил/задвоил события"
    );

    // Парный vantage: ВАЛИДНЫЙ hint на границе — тот же результат (позитивный контроль).
    let good = TailHint {
        seg_idx,
        last_seq: after,
        pos: boundary,
    };
    let s = journal::stream_from_at(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(after),
        Some(good),
    )
    .unwrap_or_else(|e| setup_failed(&format!("stream_from_at: {e}")));
    let (got_good, err_good, scanned_good) = drain(s);
    assert!(err_good.is_none());
    assert_exact_tail("j9/good", &got_good, after, TAIL);
    assert!(
        scanned_good <= TAIL + SEEK_SCAN_ALLOWANCE,
        "j9/good: валидный hint не сработал ({scanned_good})"
    );
}
