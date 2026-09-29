//! RED `M-89` (sacred, architect-only) — **`EventStream::payload_bytes_read()` — ТОЧНЫЙ учёт
//! байт, прочитанных стримом** (`I-4`, `C-260` R4): кадры `[len][payload][crc]` + заголовок
//! сегмента (магия + header-кадр) + пробы границ, инкремент В МЕСТЕ ЧТЕНИЯ, а не в
//! вызывателе и не из числа событий.
//!
//! # Проходящий мутант, который это закрывает (`C-260` R4)
//!
//! `p1` (`crates/gateway`) и `v` (`crates/gateway-serve`) держали счётчик лишь в интервале
//! `[хвост, rchar]`; величина `events_scanned × 64` в него попадала. Здесь ожидание считается
//! НЕЗАВИСИМЫМ проходом по байтам файла (свой разбор `[len][payload][crc]`, свой счётчик
//! прочитанного у zstd-декодера), а кадры в фикстуре РАЗНОГО размера (сделки ≈ 50 Б и
//! L2-снимки ≈ 2.4 КиБ вперемешку) — синтетика «N × константа» не совпадёт ни с одним
//! равенством ниже.
//!
//! # Форма (спека `M-89` §4.1, задана ДОСЛОВНО)
//!
//! ```ignore
//! impl EventStream { pub fn payload_bytes_read(&self) -> u64; }
//! ```
//!
//! # Где допуск, и почему он именно такой (`testing.md`: допуск только названный)
//!
//! · полный проход (`b1`, `b2`, `b3`, `b4`) — РАВЕНСТВО, допуска нет: всё, что лежит в файле,
//!   прочитано ровно один раз;
//! · валидный hint (`b5`) — `[хвост, хвост + 2·заголовок + кадр пробы]`: заголовок читается
//!   `resolve_active_start_offset` и повторно `PositionedBufReader::open`, проба —
//!   `probe_frame_boundary` читает ОДИН кадр по `pos`;
//! · откат по невалидному hint'у (`b6`) — `[файл, файл + 2·заголовок + 64 КиБ]`: проба с
//!   неверной позиции читает не более одного чанка;
//! · сдвиг по seq (`b7`, задача 1) — `[хвост, хвост + заголовок + ПРОБЫ(файл) · 64 КиБ]`,
//!   `ПРОБЫ = 2·⌈log2(файл/64 КиБ)⌉ + 4` (спека §5.2 п. 1).
//!
//! COMPILE-RED на ревизии набора: метода `payload_bytes_read` у `EventStream` нет.

mod common;

use std::io::Read;
use std::path::Path;

use common::{cfg_with, header_end, snap, trade};
use contracts::Event;
use journal::{EpochFilter, Journal, TailHint};

const BIG_SEG: u64 = 1 << 30;
const CHUNK: u64 = 64 * 1024;

fn setup_failed(what: &str) -> ! {
    panic!("SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт об учёте байт.")
}

/// Сделки и L2-снимки вперемешку: кадры ≈ 50 Б и ≈ 2.4 КиБ.
fn write_mixed(dir: &Path, from: u64, n: u64, seg: u64) {
    let mut j = Journal::open_with(dir, cfg_with(seg, "M-89 bytes"))
        .unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
    for i in from..from + n {
        let ev = if i % 7 == 3 { snap(i) } else { trade(i) };
        j.append(ev)
            .unwrap_or_else(|e| setup_failed(&format!("append #{i}: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
}

fn raw_segments(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".jrnl"))
        .collect();
    v.sort();
    v
}

/// Независимый разбор сегмента: (байты заголовка, [(seq, начало кадра, размер кадра)]).
/// Setup-страж: цепочка кадров обязана дойти РОВНО до EOF — иначе фикстура не та.
fn walk(seg: &Path) -> (u64, Vec<(u64, u64, u64)>) {
    let data = std::fs::read(seg).expect("read segment");
    let h = header_end(&data);
    if h == 0 {
        setup_failed("сегмент без заголовка v2");
    }
    let mut frames = Vec::new();
    let mut i = h;
    while i + 8 <= data.len() {
        let len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        let ev: Event = postcard::from_bytes(&data[i + 4..i + 4 + len])
            .unwrap_or_else(|e| setup_failed(&format!("кадр @{i} не декодируется: {e}")));
        frames.push((ev.seq, i as u64, (8 + len) as u64));
        i += 8 + len;
    }
    if i != data.len() {
        setup_failed("цепочка кадров не дошла до EOF — независимый разбор не согласован с файлом");
    }
    (h as u64, frames)
}

fn file_len(p: &Path) -> u64 {
    std::fs::metadata(p).expect("meta").len()
}

fn drain_count(mut s: journal::EventStream) -> (usize, u64) {
    let mut n = 0usize;
    for item in s.by_ref() {
        item.unwrap_or_else(|e| setup_failed(&format!("ошибка стрима: {e}")));
        n += 1;
    }
    (n, s.payload_bytes_read())
}

fn mixed_sizes_guard(frames: &[(u64, u64, u64)]) {
    let min = frames.iter().map(|f| f.2).min().unwrap_or(0);
    let max = frames.iter().map(|f| f.2).max().unwrap_or(0);
    if max < min * 10 {
        setup_failed(&format!(
            "кадры не разного размера (min {min}, max {max}) — синтетика «N × константа» не отличима"
        ));
    }
}

/// **b1 — полный проход, один сырой сегмент: РАВЕНСТВО с размером файла и с суммой кадров.**
#[test]
fn b1_full_pass_over_mixed_frames_counts_exactly_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_mixed(dir.path(), 0, 2_000, BIG_SEG);
    let seg = raw_segments(dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let (header, frames) = walk(&seg);
    mixed_sizes_guard(&frames);
    let expected = header + frames.iter().map(|f| f.2).sum::<u64>();
    assert_eq!(expected, file_len(&seg), "setup-страж: разбор ≠ файл");
    let s = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream");
    let (n, bytes) = drain_count(s);
    assert_eq!(n, 2_000);
    assert_eq!(
        bytes, expected,
        "b1 / I-4: payload_bytes_read = {bytes}, а файл (магия + header-кадр + Σ[len][payload][crc]) = \
         {expected}. Счётчик ведётся не в месте чтения: {n} событий × 64 = {}",
        n * 64
    );
}

/// **b2 — старый API `stream_from(Some(after))` читает активный сегмент С НАЧАЛА** (семантика
/// §10 не меняется): считается ПРОЧИТАННОЕ, а не выданное — файл целиком.
#[test]
fn b2_old_api_counts_what_was_read_not_what_was_yielded() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_mixed(dir.path(), 0, 2_000, BIG_SEG);
    let seg = raw_segments(dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let (_, frames) = walk(&seg);
    let after = 1_499u64;
    let tail: u64 = frames.iter().filter(|f| f.0 > after).map(|f| f.2).sum();
    let s = journal::stream_from(dir.path(), EpochFilter::OwnCaptureOnly, Some(after))
        .expect("stream_from");
    let (n, bytes) = drain_count(s);
    assert_eq!(n, 500);
    assert_eq!(
        bytes,
        file_len(&seg),
        "b2 / I-4: stream_from(after) прошёл файл целиком (forward-scan, §10), а счётчик = {bytes} \
         ≠ {}; байты только выданных кадров = {tail} — считать надо у читателя, не у фильтра",
        file_len(&seg)
    );
}

/// **b3 — два сырых сегмента (закрытый + активный), полный проход: Σ размеров.**
#[test]
fn b3_two_raw_segments_full_pass_counts_both_files() {
    // 64 КиБ, не 256: 400 смешанных событий весят ≈154 КиБ (57 снимков по ≈2.4 КиБ +
    // 343 сделки по ≈50 Б) — при 256 КиБ ротации не было, и сценарий падал СОБСТВЕННЫМ
    // setup-стражем, ничего не проверив (найдено прогоном engine-dev на реализации).
    const SEG: u64 = 64 * 1024;
    let dir = tempfile::tempdir().expect("tempdir");
    write_mixed(dir.path(), 0, 400, SEG);
    write_mixed(dir.path(), 400, 400, SEG);
    let segs = raw_segments(dir.path());
    if segs.len() < 2 {
        setup_failed("ротация не состоялась");
    }
    let expected: u64 = segs.iter().map(|p| file_len(p)).sum();
    let s = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream");
    let (n, bytes) = drain_count(s);
    assert_eq!(n, 800);
    assert_eq!(
        bytes, expected,
        "b3 / I-4: Σ файлов {expected}, счётчик {bytes}"
    );
}

struct Counting<R> {
    inner: R,
    n: std::rc::Rc<std::cell::Cell<u64>>,
}
impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let k = self.inner.read(buf)?;
        self.n.set(self.n.get() + k as u64);
        Ok(k)
    }
}

/// **b4 — `.zst`: считаются байты, ПОТРЕБЛЁННЫЕ ИЗ ФАЙЛА (сжатые), а не распакованные.**
/// Ожидание — независимый счётчик под тем же декодером (`zstd::Decoder::with_buffer` над
/// `BufReader` 64 КиБ — форма `open_compacted_reader`).
#[test]
fn b4_compacted_segment_counts_compressed_bytes_consumed_from_file() {
    // 64 КиБ, не 256: 400 смешанных событий весят ≈154 КиБ (57 снимков по ≈2.4 КиБ +
    // 343 сделки по ≈50 Б) — при 256 КиБ ротации не было, и сценарий падал СОБСТВЕННЫМ
    // setup-стражем, ничего не проверив (найдено прогоном engine-dev на реализации).
    const SEG: u64 = 64 * 1024;
    let dir = tempfile::tempdir().expect("tempdir");
    write_mixed(dir.path(), 0, 400, SEG);
    write_mixed(dir.path(), 400, 200, SEG);
    let segs = journal::list_segments(dir.path()).expect("segments");
    if segs.len() < 2 {
        setup_failed("ротация не состоялась");
    }
    journal::compact_segment(&segs[0], journal::DEFAULT_COMPACT_LEVEL)
        .unwrap_or_else(|e| setup_failed(&format!("компакция: {e}")));
    let segs = journal::list_segments(dir.path()).expect("segments");
    let zst = segs
        .iter()
        .find(|s| s.path.to_string_lossy().ends_with(".jrnl.zst"))
        .unwrap_or_else(|| setup_failed("нет .zst"));
    let raw_rest: u64 = segs
        .iter()
        .filter(|s| !s.path.to_string_lossy().ends_with(".jrnl.zst"))
        .map(|s| file_len(&s.path))
        .sum();
    let n = std::rc::Rc::new(std::cell::Cell::new(0u64));
    {
        let f = std::fs::File::open(&zst.path).expect("open zst");
        let inner = std::io::BufReader::with_capacity(
            64 * 1024,
            Counting {
                inner: f,
                n: n.clone(),
            },
        );
        let mut dec = zstd::Decoder::with_buffer(inner).expect("decoder");
        let mut sink = Vec::new();
        dec.read_to_end(&mut sink).expect("decode");
        if sink.len() as u64 <= n.get() {
            setup_failed("распакованное не больше сжатого — фикстура не сжимается");
        }
    }
    let expected = n.get() + raw_rest;
    let s = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream");
    let (cnt, bytes) = drain_count(s);
    assert_eq!(cnt, 600);
    assert_eq!(
        bytes, expected,
        "b4 / I-4: .zst потребил из файла {} Б + сырой {raw_rest} Б = {expected}, счётчик {bytes} — \
         для сжатого сегмента считаются байты С ДИСКА (то, что видит rchar), не распакованные",
        n.get()
    );
}

fn boundary_of(frames: &[(u64, u64, u64)], seq: u64) -> (u64, u64) {
    frames
        .iter()
        .find(|f| f.0 == seq)
        .map(|f| (f.1, f.2))
        .unwrap_or_else(|| setup_failed(&format!("кадр seq={seq} не найден")))
}

/// **b5 — валидный hint (путь M-57): хвост + заголовок·2 + кадр пробы, не больше.**
#[test]
fn b5_valid_hint_counts_tail_plus_header_and_probe_within_named_bounds() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_mixed(dir.path(), 0, 3_000, BIG_SEG);
    let seg = raw_segments(dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let (header, frames) = walk(&seg);
    let after = 2_499u64;
    let (pos, probe) = boundary_of(&frames, after + 1);
    let tail: u64 = frames.iter().filter(|f| f.0 > after).map(|f| f.2).sum();
    let seg_idx = journal::list_segments(dir.path()).expect("segments")[0].index;
    let hint = TailHint {
        seg_idx,
        last_seq: after,
        pos,
    };
    let s = journal::stream_from_at(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(after),
        Some(hint),
    )
    .expect("stream_from_at");
    let (n, bytes) = drain_count(s);
    assert_eq!(n, 500);
    let upper = tail + 2 * header + probe;
    assert!(
        (tail..=upper).contains(&bytes),
        "b5 / I-4: валидный hint — счётчик {bytes} вне [{tail}, {upper}] (хвост {tail}, заголовок \
         {header}·2, проба {probe})"
    );
}

/// **b6 — невалидный hint (позиция внутри кадра) ⇒ откат ⇒ файл целиком (+ заголовок·2 + чанк пробы).**
#[test]
fn b6_invalid_hint_fallback_counts_the_whole_file_plus_probe_chunk() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_mixed(dir.path(), 0, 3_000, BIG_SEG);
    let seg = raw_segments(dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let (header, frames) = walk(&seg);
    let after = 2_499u64;
    let (pos, _) = boundary_of(&frames, after + 1);
    let seg_idx = journal::list_segments(dir.path()).expect("segments")[0].index;
    let bad = TailHint {
        seg_idx,
        last_seq: after,
        pos: pos + 1,
    };
    let s = journal::stream_from_at(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(after),
        Some(bad),
    )
    .expect("stream_from_at");
    let (n, bytes) = drain_count(s);
    assert_eq!(n, 500);
    let file = file_len(&seg);
    let upper = file + 2 * header + CHUNK;
    assert!(
        (file..=upper).contains(&bytes),
        "b6 / I-4: откат по невалидному hint — счётчик {bytes} вне [{file}, {upper}]"
    );
}

/// **b7 — сдвиг по seq (`hint = None`, задача 1): хвост + заголовок + пробы, не больше.**
/// Это `I-1` в байтах на уровне библиотеки; сегодня равнялся бы файлу целиком.
#[test]
fn b7_seek_by_seq_costs_tail_plus_bounded_probes() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_mixed(dir.path(), 0, 20_000, BIG_SEG);
    let seg = raw_segments(dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let (header, frames) = walk(&seg);
    let after = 19_499u64;
    let tail: u64 = frames.iter().filter(|f| f.0 > after).map(|f| f.2).sum();
    let file = file_len(&seg);
    if file < tail * 20 {
        setup_failed("файл меньше 20 хвостов — «∝ хвосту» и «∝ файлу» неразличимы");
    }
    let probes = 2 * (file.div_ceil(CHUNK).max(1) as f64).log2().ceil() as u64 + 4;
    let s = journal::stream_from_at(dir.path(), EpochFilter::OwnCaptureOnly, Some(after), None)
        .expect("stream_from_at");
    let (n, bytes) = drain_count(s);
    assert_eq!(n, 500);
    let upper = tail + header + probes * CHUNK;
    assert!(
        (tail..=upper).contains(&bytes),
        "b7 / I-1+I-4: сдвиг по seq — счётчик {bytes} вне [{tail}, {upper}] (хвост {tail}, заголовок \
         {header}, ≤ {probes} проб по {CHUNK} Б; файл {file})"
    );
}
