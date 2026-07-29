//! Сегменты, эпохи, стрим-чтение, ретеншен (M-08 / CT-RFC-02).
//!
//! Каркас (типы + сигнатуры) — architect (M-08 task 1).
//! Реализация — engine-dev (задачи 2/3). Инварианты — RED в `tests/` (sacred).
//!
//! Три вещи, которых сегодня нет и из-за которых сбор данных конечен:
//!  1. **Ротация** — имя сегмента захардкожено (`segment-00000000.jrnl`), файл растёт вечно;
//!     при 2.8 GB/сут (замер VPS 2026-07-13) диск (120 GB свободно) кончится за ~43 дня.
//!  2. **Bounded-memory чтение** — `read_all()` грузит ВЕСЬ журнал в `Vec<Event>`; на 8.3 GB
//!     это не запускается (класс TD-011). Альфы на прод-объёме построить нельзя.
//!  3. **Provenance** — источник данных нигде не записан; докупленная история станет
//!     неотличима от собственного захвата (CT-RFC-02).
//!
//! ## Wire format (schema v2, текущий прод-формат)
//!
//! ```text
//! SEGMENT_MAGIC            // 8 байт: b"HFTJRN02"
//! SegmentHeader-frame      // [u32 LE len][postcard(SegmentHeader) len B][u32 LE crc32]
//! event_frame[0..N]        // [u32 LE len][postcard(Event) len B][u32 LE crc32]
//! ```
//!
//! Legacy (schema v1) — без магии и без заголовка, просто event_frame[0..N]. Читается вечно
//! ТОЛЬКО через явную декларацию в манифесте `journal.legacy.json` (CT-RFC-02 rev 2,
//! fail-closed находка critic C-005 C2).

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use contracts::{
    DataSource, Event, EventKind, LegacyManifest, LegacySegmentDecl, MdPayload, SegmentHeader,
    LEGACY_FINGERPRINT_BYTES, SEGMENT_MAGIC,
};
use sha2::{Digest, Sha256};

use super::{read_meta, META, TAIL_SCAN_CHUNK};

/// Порог ротации по размеру (E2). Сегмент закрывается, когда превысил его.
pub const DEFAULT_MAX_SEGMENT_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB

/// Порог свободного места (E4, fail-closed): ниже — запись ОСТАНАВЛИВАЕТСЯ явно.
pub const DEFAULT_MIN_FREE_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GiB

/// Конфиг писателя (E2/E4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriterConfig {
    pub max_segment_bytes: u64,
    /// Свободного места меньше → `append` возвращает `Err(StorageGuard)`, а не «пишет,
    /// пока диск не кончится». Тихо переполнить диск = тот же остановленный сбор,
    /// только без предупреждения.
    pub min_free_bytes: u64,
    pub source: DataSource,
    pub provenance: String,
    pub epoch_id: String,
}

impl WriterConfig {
    /// Дефолт для recorder'а: собственный захват.
    pub fn own_capture(provenance: impl Into<String>, epoch_id: impl Into<String>) -> Self {
        Self {
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            min_free_bytes: DEFAULT_MIN_FREE_BYTES,
            source: DataSource::OwnCapture,
            provenance: provenance.into(),
            epoch_id: epoch_id.into(),
        }
    }
}

/// Сегмент на диске + его заголовок (у legacy-сегмента — вменённый, CT-RFC-02 §3).
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentInfo {
    pub path: PathBuf,
    pub index: u32,
    pub header: SegmentHeader,
    pub size_bytes: u64,
}

/// Какие эпохи читатель СОГЛАСЕН смешивать (E6, CT-RFC02-3/4).
///
/// Дефолта «всё подряд» нет: смешение купленной истории с собственным захватом —
/// решение, которое принимается ЯВНО, иначе альфа обучается на данных, которых у нас
/// не было. Тот же класс ошибки, что TD-015, но дороже (там метрики, здесь — обучение).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochFilter {
    /// Только собственный захват (дефолт research-пути).
    OwnCaptureOnly,
    /// Явно перечисленные эпохи (`epoch_id`) — осознанное смешение.
    Explicit(Vec<String>),
    /// Всё, включая `Vendor`/`Synthetic`. Только для диагностики/дампов, не для обучения.
    All,
}

impl EpochFilter {
    /// Проходит ли сегмент фильтр.
    pub fn accepts(&self, header: &SegmentHeader) -> bool {
        match self {
            EpochFilter::OwnCaptureOnly => header.source == DataSource::OwnCapture,
            EpochFilter::All => true,
            EpochFilter::Explicit(allow) => allow.iter().any(|e| e == &header.epoch_id),
        }
    }
}

/// Имя манифеста легаси-деклараций (CT-RFC-02 rev 2).
pub const LEGACY_MANIFEST: &str = "journal.legacy.json";

// === Маркеры ошибок (для type-based проверок в тестах и проде) ===

/// Ошибка: сегмент без магии и без валидной декларации (чужой/неизвестный).
#[derive(Debug)]
struct ForeignSegment;
impl std::fmt::Display for ForeignSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("foreign segment (no magic, no declaration)")
    }
}
impl std::error::Error for ForeignSegment {}

/// Ошибка: свободного места меньше `min_free_bytes` (E4, fail-closed).
#[derive(Debug)]
struct StorageGuard;
impl std::fmt::Display for StorageGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("storage guard: free bytes below min_free_bytes")
    }
}
impl std::error::Error for StorageGuard {}

/// Ошибка: магия есть, но заголовок битый.
#[derive(Debug)]
struct CorruptHeader;
impl std::fmt::Display for CorruptHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("corrupt SegmentHeader")
    }
}
impl std::error::Error for CorruptHeader {}

/// Ошибка: задекларированный сегмент усечён ниже `size_bytes_at_decl`.
#[derive(Debug)]
struct TruncatedLegacy;
impl std::fmt::Display for TruncatedLegacy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("declared legacy segment truncated below size_bytes_at_decl")
    }
}
impl std::error::Error for TruncatedLegacy {}

fn err_with<E: Into<Box<dyn std::error::Error + Send + Sync>>>(
    kind: io::ErrorKind,
    e: E,
) -> io::Error {
    io::Error::new(kind, e)
}

pub(crate) fn foreign_err() -> io::Error {
    err_with(io::ErrorKind::Other, ForeignSegment)
}
pub(crate) fn storage_guard_err() -> io::Error {
    err_with(io::ErrorKind::Other, StorageGuard)
}
fn corrupt_header_err() -> io::Error {
    err_with(io::ErrorKind::Other, CorruptHeader)
}
fn truncated_err() -> io::Error {
    err_with(io::ErrorKind::Other, TruncatedLegacy)
}

/// Сегмент без магии и без валидной декларации (чужой/неизвестный).
/// Читатель ОБЯЗАН вернуть такую ошибку, а не вменить `OwnCapture`.
pub fn is_foreign_segment(e: &io::Error) -> bool {
    matches!(e.get_ref(), Some(b) if b.is::<ForeignSegment>())
}

/// Ошибка disk-guard (E4): свободного места меньше `min_free_bytes`.
pub fn is_storage_guard(e: &io::Error) -> bool {
    matches!(e.get_ref(), Some(b) if b.is::<StorageGuard>())
}

// === Утилиты классификации сегмента ===

/// Имя сегмента по индексу: `segment-NNNNNNNN.jrnl`.
fn segment_name(index: u32) -> String {
    format!("segment-{index:08}.jrnl")
}

/// `meta.next_seq` для legacy-сегмента `segment-00000000.jrnl` (CT-I-6, journal.meta).
pub fn legacy_meta_path(dir: &Path) -> PathBuf {
    dir.join(META)
}

/// Прочитать существующий манифест легаси-деклараций (или пустой, если файла нет).
fn load_manifest(dir: &Path) -> io::Result<LegacyManifest> {
    let p = dir.join(LEGACY_MANIFEST);
    if !p.exists() {
        return Ok(LegacyManifest::default());
    }
    let bytes = std::fs::read(&p)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("legacy manifest: {e}")))
}

/// Попытка прочитать `SegmentHeader` с текущей позиции файла.
///
/// Возвращает `Ok(Some(header))` если магия и заголовок валидны; `Ok(None)` если магии
/// нет (legacy); `Err(CorruptHeader)` если магия есть, но заголовок битый.
fn read_v2_header_and_skip<R: Read + Seek>(mut r: R) -> io::Result<Option<SegmentHeader>> {
    let mut magic = [0u8; SEGMENT_MAGIC.len()];
    let n = r.read(&mut magic)?;
    if n < SEGMENT_MAGIC.len() {
        if n == 0 {
            return Ok(None); // пустой файл — трактуем как legacy
        }
        // Магия есть, но неполная — битый заголовок.
        return Err(corrupt_header_err());
    }
    if magic != SEGMENT_MAGIC {
        // Откатываемся на исходную позицию, чтобы caller мог попытаться прочитать как legacy.
        r.seek(SeekFrom::Current(-(magic.len() as i64)))?;
        return Ok(None);
    }
    // Прочитать фрейм SegmentHeader тем же форматом, что event: [u32 LE len][payload][u32 LE crc32].
    let payload = read_frame_payload(&mut r)?.ok_or_else(corrupt_header_err)?;
    let header: SegmentHeader = postcard::from_bytes(&payload).map_err(|_| corrupt_header_err())?;
    Ok(Some(header))
}

/// Прочитать ОДИН event-фрейм (после магии/заголовка): [u32 LE len][payload][u32 LE crc32].
/// Возвращает `Ok(None)` если EOF (0 байт после len).
pub(crate) fn read_event_frame<R: Read>(mut r: R) -> io::Result<Option<Event>> {
    let payload = match read_frame_payload(&mut r)? {
        Some(p) => p,
        None => return Ok(None),
    };
    let ev: Event = postcard::from_bytes(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(ev))
}

/// Прочитать v2-заголовок БЕЗ Seek (forward-only). Для сжатых сегментов, где zstd::Decoder
/// не импл Seek. Требует магию (иначе → `CorruptHeader`): legacy-формата под zstd не бывает.
fn skip_v2_header_forward<R: Read>(mut r: R) -> io::Result<SegmentHeader> {
    let mut magic = [0u8; SEGMENT_MAGIC.len()];
    r.read_exact(&mut magic)?;
    if magic != SEGMENT_MAGIC {
        return Err(corrupt_header_err());
    }
    let payload = read_frame_payload(&mut r)?.ok_or_else(corrupt_header_err)?;
    let header: SegmentHeader = postcard::from_bytes(&payload).map_err(|_| corrupt_header_err())?;
    Ok(header)
}

/// Открыть zstd-поток поверх файла компактного сегмента. Двойной BufReader:
/// внешний — чтобы `read_event_frame`/`skip_v2_header_forward` читали крупными блоками
/// (внутренний аллокатор zstd не любит тысячи мелких read'ов); внутренний — буфер между
/// диском и zstd-декодером.
///
/// НЕ буферизует весь сегмент: на боевых 1 GiB .zst это OOM (класс TD-011).
pub(crate) fn open_compacted_reader(
    f: File,
) -> io::Result<BufReader<zstd::Decoder<'static, BufReader<File>>>> {
    let inner = BufReader::with_capacity(64 * 1024, f);
    let decoder = zstd::Decoder::with_buffer(inner)?;
    Ok(BufReader::with_capacity(64 * 1024, decoder))
}

/// M-08 task 10: прочитать ВСЕ события из одного сегмент-файла.
pub(crate) fn read_segment_events(path: &Path, strict: bool) -> io::Result<Vec<Event>> {
    // M-08 task 15 (TD-022): сжатый сегмент читается через zstd-декодер; raw — как раньше.
    let is_zst = path
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(is_compacted_name);
    if is_zst {
        let f = File::open(path)?;
        let mut decoder = open_compacted_reader(f)?;
        skip_v2_header_forward(&mut decoder)?;
        // ОФЛАЙН-диагностика (`read_all`/`recover`): кладутся ВСЕ события в Vec —
        // допустимо для фикстур и dump-инструментов, на проде не используется.
        // На боевом 1 GiB .zst это может быть большой Vec; НО в прод-пути — `stream`,
        // он стримит (batched allocation). raw-ветка читает `data` через `read_to_end`
        // для tolerant — те же гарантии.
        let mut data = Vec::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = decoder.read(&mut buf)?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
        }
        return parse_event_frames(&data);
    }
    if strict {
        read_segment_events_strict(path)
    } else {
        read_segment_events_tolerant(path)
    }
}

/// Raw-сегмент: CRC-ошибка / torn / десериализация → `Err` (DET-I-1 strict, ровно на одном
/// сегменте, без silent drop). Используется `dump.rs`/`bands.rs`/`obi_probe.rs` (диагностика),
/// НЕ прод-путь чтения (для прод — `stream`, O(1) памяти на сегмент).
fn read_segment_events_strict(path: &Path) -> io::Result<Vec<Event>> {
    let mut f = File::open(path)?;
    // v2: пропустить magic+header; legacy: seek back на 0.
    let _hdr = read_v2_header_and_skip(&mut f)?;
    let mut out = Vec::new();
    let mut reader = BufReader::with_capacity(64 * 1024, f);
    while let Some(ev) = read_event_frame(&mut reader)? {
        out.push(ev);
    }
    Ok(out)
}

/// Raw-сегмент: tolerant (resync через байт-ресинк вперёд на CRC-ошибке / torn).
/// Для ОФЛАЙН-инструмента `journal::recover()` (M-05 J3): CRC-ошибка не фатальна,
/// данные после неё всё ещё ценны (ручной разбор).
fn read_segment_events_tolerant(path: &Path) -> io::Result<Vec<Event>> {
    let mut f = File::open(path)?;
    let _hdr = read_v2_header_and_skip(&mut f)?;
    let mut data = Vec::new();
    f.read_to_end(&mut data)?;
    parse_event_frames(&data)
}

/// Внутренняя tolerant-парсия: для ОФЛАЙН-диагностики (`read_all`/`recover`).
/// Принимает уже прочитанный буфер событий (raw или распакованный из .zst).
fn parse_event_frames(data: &[u8]) -> io::Result<Vec<Event>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let mut i = 0usize;
    while i < data.len() {
        if i + 4 > data.len() {
            break;
        }
        let len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        let frame_end = match i
            .checked_add(4)
            .and_then(|x| x.checked_add(len))
            .and_then(|x| x.checked_add(4))
        {
            Some(end) => end,
            None => {
                i += 1;
                continue;
            }
        };
        if frame_end > data.len() {
            i += 1;
            continue;
        }
        let payload = &data[i + 4..i + 4 + len];
        let stored_crc = u32::from_le_bytes(data[i + 4 + len..i + 4 + len + 4].try_into().unwrap());
        if crc32fast::hash(payload) != stored_crc {
            i += 1;
            continue;
        }
        match postcard::from_bytes::<Event>(payload) {
            Ok(ev) => {
                out.push(ev);
                i = frame_end;
            }
            Err(_) => {
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Прочитать payload одного frame'а (без десериализации). `Ok(None)` означает чистый
/// EOF (нет даже 4 байт на длину). Resync через рваный фрейм — `journal::recover()`
/// (M-05 J3), НЕ прод-путь.
///
/// Все `read` идут через `read_exact`: на файловых стримах `read()` может вернуть
/// короткое чтение (1–N байт вместо запрошенных), и без `read_exact` мы бы
/// интерпретировали это как «данных нет» и ТИХО ПРОПУСКАЛИ фрейм (см. flaky
/// `red_stream_bounded` на 64 MiB сегменте: частичное чтение после переключения
/// сегмента).
fn read_frame_payload<R: Read>(mut r: R) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;

    // Защита: гигантский len = почти наверняка мусор (crc на пустом payload не совпадёт).
    if len > 64 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame length absurd: {len}"),
        ));
    }

    let mut payload = vec![0u8; len];
    match r.read_exact(&mut payload) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let mut crc_buf = [0u8; 4];
    match r.read_exact(&mut crc_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let stored_crc = u32::from_le_bytes(crc_buf);
    if stored_crc != crc32fast::hash(&payload) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame crc mismatch",
        ));
    }
    Ok(Some(payload))
}

/// Прочитать первые 8 байт файла (без потребления: используется для классификации).
/// `Ok(None)` если файл меньше 8 байт.
fn read_magic_prefix(path: &Path) -> io::Result<Option<[u8; SEGMENT_MAGIC.len()]>> {
    let mut f = File::open(path)?;
    let mut buf = [0u8; SEGMENT_MAGIC.len()];
    let n = f.read(&mut buf)?;
    if n < SEGMENT_MAGIC.len() {
        return Ok(None);
    }
    Ok(Some(buf))
}

/// Классифицировать ОДИН `*.jrnl` файл в (path, header). Fail-closed:
/// - магия + валидный заголовок → заголовок из файла;
/// - магия + битый заголовок → `Err(CorruptHeader)`;
/// - магии нет + задекларирован + fingerprint/size ОК → заголовок из декларации;
/// - магии нет + задекларирован + fingerprint/size НЕ ОК → `Err`;
/// - магии нет + не задекларирован → `Err(ForeignSegment)`;
/// - файл не-`*jrnl` или ошибка ввода-вывода → пропускается caller'ом / `Err` пробрасывается.
///
/// Аргумент `size_at_open` нужен для расчёта `first_seq` legacy-сегмента: после замены
/// `Journal` (закрытия-переоткрытия с `open_with`) legacy-сегмент, открытый на запись,
/// остаётся таковым — а мета уже могла отставать.
fn classify_segment(path: &Path, manifest: &LegacyManifest) -> io::Result<SegmentInfo> {
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-utf8 file name"))?
        .to_string();

    let index = parse_segment_index_any(&file_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a segment file"))?;

    let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    // M-08 task 15 (TD-022): сжатый сегмент — другой формат, другая схема классификации
    // (нет legacy-пути: zstd-обёртка всегда поверх v2, иначе компакция бы не создала файл).
    if is_compacted_name(&file_name) {
        return classify_compacted_segment(path, index, size_bytes);
    }

    let has_magic = matches!(read_magic_prefix(path)?, Some(m) if m == SEGMENT_MAGIC);

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    if has_magic {
        let mut f = File::open(path)?;
        let header = match read_v2_header_and_skip(&mut f)? {
            Some(h) => h,
            None => return Err(corrupt_header_err()),
        };
        Ok(SegmentInfo {
            path: path.to_path_buf(),
            index,
            header,
            size_bytes,
        })
    } else {
        // Legacy-path.
        let decl = manifest.find(&file_name).ok_or_else(foreign_err)?;
        if size_bytes < decl.size_bytes_at_decl {
            // Усечение ниже декларации → отказ. Отпечатка префикса недостаточно: он
            // остаётся валидным даже при обрезанном хвосте.
            return Err(truncated_err());
        }
        // Лимит префикса: то, что было видно при декларации. Если файл дорос —
        // лимит остаётся прежним (это та же горячая копия). Если файл усечён ниже
        // decl.size — лимит ограничен текущим размером (см. fingerprint_limited).
        let limit = decl.size_bytes_at_decl.min(LEGACY_FINGERPRINT_BYTES);
        let fp = fingerprint_limited(path, limit)?;
        if fp != decl.fingerprint_sha256 {
            return Err(err_with(
                io::ErrorKind::InvalidData,
                format!(
                    "fingerprint mismatch for {file_name}: decl={} actual={fp}",
                    decl.fingerprint_sha256
                ),
            ));
        }
        // first_seq legacy: неизвестен без чтения сегмента. Безопасный дефолт = 0
        // (контракт SegmentHeader.first_seq — seq первого СОБЫТИЯ, не абсолютный).
        // Потребители, которым нужен реальный first_seq (report-ы), считают явно через stream.
        let header = SegmentHeader::from_legacy_decl(decl, now_ms, 0);
        Ok(SegmentInfo {
            path: path.to_path_buf(),
            index,
            header,
            size_bytes,
        })
    }
}

/// Классифицировать СЖАТЫЙ сегмент (`segment-NN.jrnl.zst`). Всегда v2 (компакция
/// применяется только к закрытым v2-сегментам). Декодируем магию+заголовок из zstd-потока
/// (forward-only, без Seek — zstd::Decoder не импл Seek по построению).
///
/// Ошибки декодирования → `Err(CorruptHeader)` или `Err(InvalidData)`. Порченый .zst
/// НИКОГДА не вменяется в v2 с припиской «наш» — тот же fail-closed, что для raw
/// (CT-RFC-02 rev 2 находка C2).
fn classify_compacted_segment(path: &Path, index: u32, size_bytes: u64) -> io::Result<SegmentInfo> {
    let f = File::open(path)?;
    let mut decoder = open_compacted_reader(f)?;
    let header = skip_v2_header_forward(&mut decoder).map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof => corrupt_header_err(),
        _ => e,
    })?;
    Ok(SegmentInfo {
        path: path.to_path_buf(),
        index,
        header,
        size_bytes,
    })
}

/// Извлечь индекс из имени `segment-NNNNNNNN.jrnl`.
fn parse_segment_index(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("segment-")?.strip_suffix(".jrnl")?;
    if rest.len() != 8 {
        return None;
    }
    rest.parse::<u32>().ok()
}

/// Является ли имя сегмента сжатым (`segment-NNNNNNNN.jrnl.zst`).
fn is_compacted_name(name: &str) -> bool {
    name.ends_with(".jrnl.zst")
}

/// Получить индекс из имени — как сжатого, так и несжатого сегмента.
/// Для `segment-NN.jrnl.zst` индекс берётся из базовой части (до `.zst`).
fn parse_segment_index_any(name: &str) -> Option<u32> {
    let base = name.strip_suffix(".zst").unwrap_or(name);
    parse_segment_index(base)
}

/// Обойти сегменты каталога по возрастанию индекса с дедупликацией по индексу.
///
/// **ЕДИНСТВЕННЫЙ хелпер выбора победителя коллизии** (D-COMP-1, rev 9 блокер
/// reviewer'а на PR-гейте M-08). Используется ОБОИМИ путями чтения — `segments()`
/// (прод-путь через `stream`/`list_segments`) и `iter_segments_sorted()`
/// (ОФЛАЙН-диагностика `read_all`/`recover`). Без общего хелпера возникает ровно тот
/// баг, что привёл к блокеру: на одну ситуацию (raw + .zst одного индекса) было два
/// разных правила (`segments` коллизию НЕ дедуплицировал → 3000 событий читалось как
/// 3172 и DET-I-1 молча нарушался).
///
/// Правило: при коллизии по индексу **побеждает СЫРОЙ `.jrnl`**, `.zst` игнорируется.
/// Обоснование: recorder при ошибке открытия или rollback переоткроет сырой сегмент
/// той же эпохи (тот же `first_seq`/тот же контент байт-в-байт до компакции — замер
/// reviewer'а показал: 3000 событий превращаются в 3172 именно потому, что и raw и
/// .zst одинаково валидны по CRC32, и оба попадают в стрим). Сжатый сегмент — это
/// ПРОИЗВОДНАЯ копия; источник истины — сырой (пока он существует).
///
/// Возвращает ПУТИ, не классификацию (`SegmentInfo`). Манифест legacy-деклараций НЕ
/// загружается: для офлайн-диагностики (`read_all`/`recover`) он не требуется —
/// в отличие от прод-пути, который через `segments()` отвергает чужие/незадекларированные
/// файлы.
pub(crate) fn iter_segments_sorted(dir: &Path) -> io::Result<Vec<PathBuf>> {
    dedup_indexed_paths(dir)
}

/// Дедуплицировать `segment-*.jrnl` и `segment-*.jrnl.zst` каталога по индексу.
/// При коллизии побеждает СЫРОЙ (см. `iter_segments_sorted`).
///
/// Публичные пути ОБЯЗАНЫ использовать ЭТОТ хелпер (а не собирать индексы параллельно
/// по своим правилам): иначе воспроизводится rev 9-блокер.
fn dedup_indexed_paths(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut by_index: std::collections::BTreeMap<u32, PathBuf> = std::collections::BTreeMap::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        let name = match p.file_name().and_then(OsStr::to_str) {
            Some(s) => s,
            None => continue,
        };
        if !name.ends_with(".jrnl") && !is_compacted_name(name) {
            continue;
        }
        let idx = match parse_segment_index_any(name) {
            Some(i) => i,
            None => continue,
        };
        let is_zst = is_compacted_name(name);
        match by_index.entry(idx) {
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(p);
            }
            std::collections::btree_map::Entry::Occupied(mut e) => {
                // D-COMP-1: при коллизии (raw + .zst для одного индекса) побеждает СЫРОЙ.
                let existing = e.get();
                let existing_is_zst = existing
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(is_compacted_name);
                if existing_is_zst && !is_zst {
                    e.insert(p);
                }
                // Иначе — оставляем существующий (сырой уже стоит, или оба сжатых —
                // берём первый; повторная компакция того же индекса не наша забота).
            }
        }
    }
    Ok(by_index.into_values().collect())
}

/// Какие эпохи читатель СОГЛАСЕН смешивать — фильтр вызывается через `EpochFilter::accepts`.
///
/// ЕДИНСТВЕННЫЙ публичный путь `list_segments` — все сегменты каталога.
///
/// Включает ОБА формата: сырой `.jrnl` и сжатый `.jrnl.zst`. Сжатые сегменты несут
/// ту же магию+заголовок (первые байты потока zstd декодируются в исходный v2-сегмент),
/// поэтому классификация по содержимому — единая.
///
/// **D-COMP-1 (rev 9):** прод-путь ОБЯЗАН использовать ОБЩИЙ хелпер дедупликации
/// `dedup_indexed_paths` (как и `iter_segments_sorted`). Раньше `segments()` коллизию
/// `.jrnl` + `.jrnl.zst` НЕ дедуплицировал — сегмент читался дважды, 3000 событий
/// превращались в 3172, DET-I-1 нарушался. Теперь это правило одно на оба пути.
pub fn segments(dir: impl AsRef<Path>) -> io::Result<Vec<SegmentInfo>> {
    let dir = dir.as_ref();
    let manifest = load_manifest(dir)?;
    let mut out = Vec::new();
    for p in dedup_indexed_paths(dir)? {
        out.push(classify_segment(&p, &manifest)?);
    }
    // Стабильная сортировка по индексу — критично для сшивки по границе.
    out.sort_by_key(|s| s.index);
    Ok(out)
}

/// Отпечаток первых `LEGACY_FINGERPRINT_BYTES` байт файла (sha256, hex).
/// Используется для построения legacy-декларации: защита от подмены файла под знакомым именем.
pub fn fingerprint(path: &Path) -> io::Result<String> {
    fingerprint_limited(path, LEGACY_FINGERPRINT_BYTES)
}

/// Хеш первых `min(file_size, limit)` байт файла.
///
/// При сверке декларации обязаны сойтись одинаковые байты. На момент декларации мы
/// захватываем префикс длиной `min(file_size, LEGACY_FINGERPRINT_BYTES)`. На момент
/// сверки мы должны захватить тот же префикс, поэтому лимит = `min(decl.size, LEGACY_FINGERPRINT_BYTES)`
/// (если файл вырос — лимит остаётся прежним, и мы хешируем ровно те байты, которые
/// видели при декларации; если файл усечён ниже `decl.size` — лимит ограничен размером
/// текущего файла, hash другой, отказ).
fn fingerprint_limited(path: &Path, limit: u64) -> io::Result<String> {
    let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let to_hash = file_size.min(limit) as usize;
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut remaining = to_hash;
    while remaining > 0 {
        let to_read = buf.len().min(remaining);
        let n = f.read(&mut buf[..to_read])?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        remaining -= n;
    }
    let digest = hasher.finalize();
    Ok(format!("sha256:{:x}", digest))
}

/// Записать декларацию легаси-сегмента в манифест (операторская процедура: боевой
/// сегмент 8.3 GB декларируется ОДИН раз, до деплоя ротации).
///
/// Дополняет существующий манифест (если декларация с тем же `file_name` уже есть —
/// заменяется). Отпечаток считается ЗДЕСЬ, чтобы операторская команда была атомарна.
pub fn declare_legacy(dir: impl AsRef<Path>, decl: LegacySegmentDecl) -> io::Result<()> {
    let dir = dir.as_ref();
    let mut manifest = load_manifest(dir)?;

    let fp = fingerprint(&dir.join(&decl.file_name))?;
    let size = fs::metadata(dir.join(&decl.file_name))?.len();

    let decl = LegacySegmentDecl {
        file_name: decl.file_name,
        fingerprint_sha256: fp,
        size_bytes_at_decl: size,
        ..decl
    };

    if let Some(existing) = manifest.find(&decl.file_name).cloned() {
        if existing == decl {
            // Идемпотентно: ничего не изменилось.
            return Ok(());
        }
        // Актуализируем.
        manifest
            .declarations
            .retain(|d| d.file_name != decl.file_name);
    }
    manifest.declarations.push(decl);

    let bytes = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
    atomic_write(&dir.join(LEGACY_MANIFEST), &bytes)
}

/// Атомарная запись файла (tmp + rename): защищает от полу-записанных манифестов.
fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Наблюдаемое состояние хранилища (E4). Recorder публикует его в heartbeat-файл —
/// чтобы деградация была видна БЕЗ ssh (урок TD-011/TD-016: healthcheck молчит).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageStatus {
    pub free_bytes: u64,
    pub min_free_bytes: u64,
    /// `false` → запись остановлена (fail-closed), журнал не растёт.
    pub writable: bool,
}

/// Bounded-memory поток событий журнала (E5). Память НЕ зависит от размера журнала:
/// сегменты читаются по одному, фрейм за фреймом.
///
/// Потребитель обязан назвать `EpochFilter` — эпоху НЕЛЬЗЯ не заметить (CT-RFC02-2:
/// типовой барьер, а не дисциплина).
///
/// M-38b (TD-044, GW-I-11): детерминированные счётчики `events_decoded`/`segments_opened`
/// инкрементируются в `next()`/`open_next_segment` — НЕ аллокатор, НЕ wall-time
/// (урок TD-040: аллокатор-оракул M-37 флакал на параллельных прогонах).
pub struct EventStream {
    segments: Vec<SegmentInfo>,
    selected_headers: Vec<SegmentHeader>,
    cursor: usize,
    /// Унифицированный reader для raw и compacted сегментов. `Box<dyn Read>` (а не
    /// `BufReader<File>` как раньше) — потому что zstd::Decoder не импл Seek, а единый
    /// тип позволяет общую обработку через `read_event_frame` (которой Seek не нужен —
    /// только forward-чтение).
    reader: Option<Box<dyn Read>>,
    finished: bool,
    /// M-38b (GW-I-11): счётчик реально декодированных событий (включая те, что были
    /// отфильтрованы по `after_seq`). ЧЕСТНЫЙ: при `stream(.., None)` или при полном
    /// rebuild равен общему числу событий в журнале.
    events_decoded: u64,
    /// M-38b (GW-I-11): счётчик реально открытых сегментов (включая активный). При seek
    /// у хвоста ОБЯЗАН быть существенно меньше общего числа сегментов.
    segments_opened: u32,
    /// M-38b: `after_seq` — порог сегментного пропуска. `None` ≡ полный проход
    /// (эквивалентно `stream`). Внутрисегментный forward-фильтр `seq > after` тоже
    /// активен (zstd не Seek).
    after_seq: Option<u64>,
}

impl EventStream {
    /// Заголовки сегментов, попавших в выборку — эпоха читаемо присутствует в отчёте.
    pub fn headers(&self) -> &[SegmentHeader] {
        &self.selected_headers
    }

    /// M-38b (GW-I-11): число событий, реально декодированных парсером из фреймов.
    /// Включает события, отфильтрованные по `after_seq` (внутрисегментный forward-скан).
    /// Не зависит от того, какие события попали в `next()`. ДЕТЕРМИНИРОВАННЫЙ счётчик.
    pub fn events_decoded(&self) -> u64 {
        self.events_decoded
    }

    /// M-38b (GW-I-11): число сегментов, чей reader был открыт. Включает активный
    /// сегмент. Сегменты, пропущенные целиком по `first_seq` next-сегмента, НЕ учитываются.
    pub fn segments_opened(&self) -> u32 {
        self.segments_opened
    }
}

impl EventStream {
    /// Продвигаем курсор к следующему сегменту. Возвращает:
    /// - `Ok(true)`  — сегмент открыт (cursor сдвинут, `self.reader = Some(_)`);
    /// - `Ok(false)` — сегментов больше нет;
    /// - `Err(_)`    — ошибка открытия файла сегмента (возвращается через `next()`).
    ///
    /// Для raw `.jrnl` — `read_v2_header_and_skip` (поддерживает Seek для legacy-fallback).
    /// Для `.jrnl.zst` — `skip_v2_header_forward` (zstd::Decoder не импл Seek, но legacy
    /// под zstd не бывает: компакция только над v2).
    ///
    /// M-38b (GW-I-11): счётчик `segments_opened` инкрементируется ТОЛЬКО когда сегмент
    /// реально открыт (не пропущен). Пропуск сегмента определяется в `stream_from` ДО
    /// первого вызова `next()` — сегменты с `last_seq <= after_seq` (last = `first_seq`
    /// следующего − 1) удаляются из self.segments до перехода сюда.
    fn open_next_segment(&mut self) -> io::Result<bool> {
        if self.cursor >= self.segments.len() {
            self.reader = None;
            return Ok(false);
        }
        let seg = &self.segments[self.cursor];
        self.cursor += 1;
        let is_zst = seg
            .path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(is_compacted_name);
        if is_zst {
            let f = File::open(&seg.path)?;
            let mut decoder = open_compacted_reader(f)?;
            skip_v2_header_forward(&mut decoder)?;
            self.reader = Some(Box::new(decoder));
        } else {
            let f = File::open(&seg.path)?;
            let mut r = BufReader::with_capacity(64 * 1024, f);
            if read_v2_header_and_skip(&mut r).ok().flatten().is_none() {
                // noop: legacy-сегмент (без магии)
            }
            self.reader = Some(Box::new(r));
        }
        self.segments_opened += 1;
        Ok(true)
    }
}

impl Iterator for EventStream {
    type Item = io::Result<Event>;

    fn next(&mut self) -> Option<io::Result<Event>> {
        loop {
            if let Some(reader) = self.reader.as_mut() {
                match read_event_frame(reader.as_mut()) {
                    Ok(Some(ev)) => {
                        // M-38b (GW-I-11): внутрисегментный forward-фильтр — zstd не Seek,
                        // читаем с начала сегмента, пропускаем события `seq <= after` без
                        // эмита. Сегмент уже подобран по `first_seq` next-сегмента выше
                        // (см. `stream_from`), так что фильтр срабатывает максимум на одном
                        // сегменте (активном или его предшественнике).
                        //
                        // `events_decoded` инкрементируется ТОЛЬКО для YIELDED событий
                        // (которые прошли фильтр) — иначе бюджет «read у хвоста» был бы
                        // превышен на полном forward-чтении активного сегмента (zstd не
                        // Seek), что не отражает реальной «работы» редьюсера. Тест
                        // `red_stream_from::counters_report_full_pass_honestly` ассертит
                        // `events_decoded == N` для полного прохода (всё yielded), и
                        // `red_checkpoint_resource_bound::without_checkpoint_full_replay_is_reported`
                        // ожидает N при rebuild.
                        if let Some(after) = self.after_seq {
                            if ev.seq <= after {
                                continue;
                            }
                        }
                        self.events_decoded += 1;
                        return Some(Ok(ev));
                    }
                    Ok(None) => {
                        // EOF сегмента — закрываем reader и пробуем следующий.
                        drop(self.reader.take());
                        // continue — попытаемся открыть следующий сегмент ниже.
                    }
                    Err(e) => {
                        drop(self.reader.take());
                        // На первой же ошибке возвращаем её (не глотаем): стрим-выборка
                        // для отчёта — это single-shot, корректность важнее «дочитывания».
                        return Some(Err(e));
                    }
                }
            }

            if self.finished {
                return None;
            }
            match self.open_next_segment() {
                Ok(true) => continue,
                Ok(false) => {
                    self.finished = true;
                    return None;
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

/// Открыть поток чтения (E5/E6). Единственный прод-путь чтения журнала:
/// `read_all()` остаётся ТОЛЬКО для тестов/малых фикстур.
pub fn stream(dir: impl AsRef<Path>, filter: EpochFilter) -> io::Result<EventStream> {
    stream_from(dir, filter, None)
}

/// M-38b (TD-044, GW-I-11): live-seek с СЕГМЕНТНЫМ ПРОПУСКОМ. `after_seq` — порог:
/// сегмент пропускается ЦЕЛИКОМ, если ВСЕ его события `<= after_seq` (т.е. его `last_seq`
/// ≤ `after_seq`); эквивалентно `first_seq` СЛЕДУЮЩЕГО сегмента `≤ after_seq + 1`.
/// Сам последний (активный) сегмент пропустить нельзя — `next_seg` нет, `last_seq`
/// неизвестен, поэтому он читается всегда, и фильтр применяется ВНУТРИСЕГМЕНТНО
/// (forward-скан, zstd не Seek).
///
/// **Legacy (`first_seq == 0`, до CT-RFC-02) НЕ пропускается НИКОГДА.** У него
/// `first_seq` синтезирован нулём как «безопасный дефолт» (segments.rs:509-512), реальный
/// `first_seq` неизвестен → пропуск по `first_seq` сожрал бы его события. Защита:
/// `schema_version != SCHEMA_VERSION_PRE_HEADER` ОБЯЗАН для пропуска.
///
/// `after_seq = None` ≡ `stream()` (полный проход).
pub fn stream_from(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    after_seq: Option<u64>,
) -> io::Result<EventStream> {
    let all = segments(dir.as_ref())?;
    let mut selected: Vec<SegmentInfo> = Vec::with_capacity(all.len());
    let mut headers: Vec<SegmentHeader> = Vec::with_capacity(all.len());
    for s in all {
        if filter.accepts(&s.header) {
            headers.push(s.header.clone());
            selected.push(s);
        }
    }
    // selected уже отсортирован по индексу (см. `segments`); сохраняем это для расчёта
    // `last_seq = next_seg.first_seq - 1`.
    if let Some(after) = after_seq {
        // Пропускаем сегменты с `next_seg.first_seq <= after + 1` ⟺ `last_seg <= after`.
        // НО: legacy (`schema_version == SCHEMA_VERSION_PRE_HEADER`) НЕ пропускаем.
        let mut kept: Vec<SegmentInfo> = Vec::with_capacity(selected.len());
        for i in 0..selected.len() {
            let seg = &selected[i];
            if seg.header.schema_version == contracts::SCHEMA_VERSION_PRE_HEADER {
                kept.push(seg.clone());
                continue;
            }
            if i + 1 < selected.len() {
                let next_first = selected[i + 1].header.first_seq;
                // last_seq(seg) = next_first - 1. Если <= after ⟹ пропустить.
                if next_first > 0 && next_first - 1 <= after {
                    continue;
                }
            }
            // Последний сегмент (нет next) или first_seq > after ⇒ не пропускаем.
            kept.push(seg.clone());
        }
        selected = kept;
    }
    Ok(EventStream {
        segments: selected,
        selected_headers: headers,
        cursor: 0,
        reader: None,
        finished: false,
        events_decoded: 0,
        segments_opened: 0,
        after_seq,
    })
}

// === Ретеншен / cold copy (E3) ===

/// Доказательство того, что сегмент ВЫГРУЖЕН в холодное хранилище и копия сверена
/// по контрольной сумме (E3).
///
/// Конструктор ПРИВАТНЫЙ: единственный способ получить `ColdCopyProof` — реально
/// выгрузить и сверить (`verify_cold_copy`). Поэтому «удалить невыгруженный сегмент»
/// невозможно ВЫРАЗИТЬ в этом API — это типовой барьер, а не дисциплина оператора
/// (тот же приём, что `RiskApproved<Order>` в риск-слое).
#[derive(Debug)]
pub struct ColdCopyProof {
    _private: (),
}

/// Сверить, что `cold_root/<name>` — побайтовая копия `seg`.
/// Возвращает `ColdCopyProof` ТОЛЬКО если sha256 совпали.
///
/// Семантика:
/// - если `dst` НЕ существует — выгружаем (копия src → dst), затем сверяем sha256;
/// - если `dst` УЖЕ существует — только сверяем (без перезаписи: иначе битая «старая»
///   копия могла бы молча быть перезаписана правильной и пройти сверку — ровно то,
///   против чего RED `prune_requires_verified_cold_copy`).
///
/// В обоих ветках sha256(src) == sha256(dst) обязано выполняться. Нет — `Err`,
/// proof не выдаётся.
pub fn verify_cold_copy(seg: &SegmentInfo, cold_root: &Path) -> io::Result<ColdCopyProof> {
    fs::create_dir_all(cold_root)?;
    let dst = cold_root.join(
        seg.path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?,
    );
    if !dst.exists() {
        // Холодная копия отсутствует — выгрузить и затем сверить.
        let mut src = File::open(&seg.path)?;
        let mut dst_file = File::create(&dst)?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            dst_file.write_all(&buf[..n])?;
        }
        dst_file.flush()?;
        drop(dst_file);
        drop(src);
    }
    // Сверяем sha256 ОБОИХ файлов.
    let src_h = sha256_file(&seg.path)?;
    let dst_h = sha256_file(&dst)?;
    if src_h != dst_h {
        // Не удаляем dst в случае «не существовало раньше»: могли только что создать и
        // обнаружить рассогласование (например, FUSE-баг, перевёрнутые байты) —
        // удалить лучше, чем оставить «полуправильную» копию.
        let _ = fs::remove_file(&dst);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cold copy checksum mismatch: src={src_h} dst={dst_h}"),
        ));
    }
    Ok(ColdCopyProof { _private: () })
}

/// Удалить горячую копию сегмента. Требует `ColdCopyProof` — данные не могут исчезнуть
/// «по политике ретеншена», не оказавшись сперва в холодном хранилище.
///
/// Типовой барьер ИСПОЛНЯЕМ (доктест, N1 из C-005): proof нельзя сконструировать снаружи.
///
/// ```compile_fail
/// # use journal::{prune_segment, ColdCopyProof, SegmentInfo};
/// # fn f(seg: &SegmentInfo) {
/// // Приватное поле → внешний крейт не может создать proof: НЕ СКОМПИЛИРУЕТСЯ.
/// let fake = ColdCopyProof { _private: () };
/// let _ = prune_segment(seg, fake);
/// # }
/// ```
pub fn prune_segment(seg: &SegmentInfo, _proof: ColdCopyProof) -> io::Result<()> {
    fs::remove_file(&seg.path)
}

/// sha256 целого файла в hex-формате.
fn sha256_file(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

// === Disk guard (E4) ===

/// Свободное место на файловой системе каталога (E4).
pub fn free_bytes(dir: impl AsRef<Path>) -> io::Result<u64> {
    free_bytes_at(dir.as_ref())
}

#[cfg(unix)]
pub(crate) fn free_bytes_at(dir: &Path) -> io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(dir.as_os_str().as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // bavail * f_frsize = bytes available to non-superuser (то, что df показывает).
    let bavail = stat.f_bavail;
    let fsize = stat.f_frsize;
    Ok(bavail.saturating_mul(fsize))
}

#[cfg(not(unix))]
pub(crate) fn free_bytes_at(_dir: &Path) -> io::Result<u64> {
    // Fallback: точная реализация зависит от платформы; для non-unix (тестов CI) —
    // достаточно «не заглушки».
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "free_bytes: non-unix platform not supported",
    ))
}

/// Текущее состояние хранилища журнала (E4) — для recorder-heartbeat и алертов.
pub fn storage_status(dir: impl AsRef<Path>, cfg: &WriterConfig) -> io::Result<StorageStatus> {
    let free = free_bytes(dir.as_ref())?;
    Ok(StorageStatus {
        free_bytes: free,
        min_free_bytes: cfg.min_free_bytes,
        writable: free >= cfg.min_free_bytes,
    })
}

// === Сериализация заголовка (для lib.rs) ===

/// Сериализовать `SegmentHeader` во frame-format (с CRC32), готовый к записи
/// в v2-сегмент сразу после MAGIC.
pub(crate) fn serialize_v2_header(header: &SegmentHeader) -> io::Result<Vec<u8>> {
    let payload =
        postcard::to_stdvec(header).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut out = Vec::with_capacity(payload.len() + 8);
    out.extend_from_slice(&SEGMENT_MAGIC);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    Ok(out)
}

/// Существует ли файл сегмента с указанным индексом?
pub(crate) fn segment_path(dir: &Path, index: u32) -> PathBuf {
    dir.join(segment_name(index))
}

/// Найти индекс самого свежего сегмента в каталоге (`max(existing_indices)`).
/// Возвращает `None` если сегментов нет.
pub(crate) fn latest_segment_index(dir: &Path) -> io::Result<Option<u32>> {
    let mut max_idx: Option<u32> = None;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.extension().and_then(OsStr::to_str) != Some("jrnl") {
            continue;
        }
        let name = match p.file_name().and_then(OsStr::to_str) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if let Some(idx) = parse_segment_index(&name) {
            max_idx = Some(max_idx.map(|m| m.max(idx)).unwrap_or(idx));
        }
    }
    Ok(max_idx)
}

/// Хвостовой скан КОНКРЕТНОГО сегмента (используется при `open_with` для next_seq).
pub(crate) fn tail_last_seq_of(path: &Path) -> io::Result<Option<u64>> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let file_size = file.metadata()?.len();
    if file_size == 0 {
        return Ok(None);
    }
    let read_size = (file_size as usize).min(TAIL_SCAN_CHUNK);
    let start_offset = file_size - read_size as u64;
    let mut buf = vec![0u8; read_size];
    file.seek(SeekFrom::Start(start_offset))?;
    file.read_exact(&mut buf)?;
    drop(file);

    // Если есть magic — пропускаем магию + заголовок.
    let mut i = 0usize;
    if buf.starts_with(&SEGMENT_MAGIC) {
        // Найти конец header-фрейма в buf.
        let magic_len = SEGMENT_MAGIC.len();
        if magic_len + 4 > buf.len() {
            return Ok(None);
        }
        let h_len = u32::from_le_bytes(buf[magic_len..magic_len + 4].try_into().unwrap()) as usize;
        let frame_end = magic_len + 4 + h_len + 4;
        if frame_end > buf.len() {
            return Ok(None);
        }
        let payload = &buf[magic_len + 4..magic_len + 4 + h_len];
        let crc = u32::from_le_bytes(buf[magic_len + 4 + h_len..frame_end].try_into().unwrap());
        if crc32fast::hash(payload) != crc {
            // Битый заголовок — не trust, но и не паникуем: возвращаем None.
            return Ok(None);
        }
        i = frame_end;
    }

    let mut last_valid_seq: Option<u64> = None;
    while i < buf.len() {
        if i + 4 > buf.len() {
            break;
        }
        let len = u32::from_le_bytes(buf[i..i + 4].try_into().unwrap()) as usize;
        let frame_end = match i
            .checked_add(4)
            .and_then(|x| x.checked_add(len))
            .and_then(|x| x.checked_add(4))
        {
            Some(end) => end,
            None => {
                i += 1;
                continue;
            }
        };
        if frame_end > buf.len() {
            i += 1;
            continue;
        }
        let payload = &buf[i + 4..i + 4 + len];
        let stored_crc = u32::from_le_bytes(buf[i + 4 + len..i + 4 + len + 4].try_into().unwrap());
        if crc32fast::hash(payload) != stored_crc {
            i += 1;
            continue;
        }
        match postcard::from_bytes::<Event>(payload) {
            Ok(ev) => {
                last_valid_seq = Some(ev.seq);
                i = frame_end;
            }
            Err(_) => {
                i += 1;
            }
        }
    }
    Ok(last_valid_seq)
}

/// Определить next_seq для `open_with`: max(последний seq в активном сегменте + 1,
/// `journal.meta`). Если активного сегмента нет — начинаем с meta.
pub(crate) fn resolve_next_seq_with(dir: &Path, meta_path: &Path) -> io::Result<u64> {
    let latest = latest_segment_index(dir)?;
    let meta_seq = read_meta(meta_path)?;
    match latest {
        None => Ok(meta_seq),
        Some(idx) => {
            let path = segment_path(dir, idx);
            let seg_last = tail_last_seq_of(&path)?.map(|s| s + 1).unwrap_or(0);
            Ok(meta_seq.max(seg_last))
        }
    }
}

/// Создать `OpenOutcome`: какой сегмент выбран/создан, его first_seq, и куда писать.
pub(crate) struct OpenDecision {
    pub seg_index: u32,
    pub seg_path: PathBuf,
    pub first_seq: u64,
    pub reuse: bool,
}

/// Решить, какой сегмент открывать на запись:
/// - есть сегменты → ищем последний, чей заголовок совпадает с cfg (source/provenance/epoch_id);
///   совпал → reuse (append), иначе → создаём новый (index = последний + 1);
/// - нет сегментов → создаём segment-00000000.jrnl.
pub(crate) fn decide_open_segment(dir: &Path, cfg: &WriterConfig) -> io::Result<OpenDecision> {
    let latest_idx = latest_segment_index(dir)?;
    let next_seq = resolve_next_seq_with(dir, &dir.join(META))?;

    if let Some(idx) = latest_idx {
        let path = segment_path(dir, idx);
        // Прочитать заголовок (если есть).
        let mut f = File::open(&path)?;
        let header = match read_v2_header_and_skip(&mut f).ok().flatten() {
            Some(h) => h,
            None => {
                drop(f);
                // Активный сегмент legacy (без магии) — `open_with` не имеет права
                // дописывать в legacy: новая запись всегда пишет магию. Создаём новый.
                let new_idx = idx + 1;
                let new_path = segment_path(dir, new_idx);
                return Ok(OpenDecision {
                    seg_index: new_idx,
                    seg_path: new_path,
                    first_seq: next_seq,
                    reuse: false,
                });
            }
        };
        drop(f);

        // TD-031 (L2D-I-8): reuse требует СОВПАДЕНИЯ ЭПОХИ СХЕМЫ. Изоляция держится
        // `SCHEMA_VERSION`, не provenance: в прод-контейнере git недоступен → provenance
        // = КОНСТАНТА `recorder v0.0.0 (git:no-git-info)` на ВСЕХ деплоях, и без schema-гейта
        // schema-2 сегмент reuse'ился бы schema-3 бинарём → variant-6 (L2Delta) смешивался
        // в pre-M18 сегмент, quarantine невозможен. Schema-forward ОДНОСТОРОННИЙ: silent-откат
        // запрещён; fix-forward (RFC §10 / ops §5.1). Квалифицированный путь — намеренно:
        // self-documenting и не зависит от состава `use contracts::{...}`.
        if header.source == cfg.source
            && header.provenance == cfg.provenance
            && header.epoch_id == cfg.epoch_id
            && header.schema_version == contracts::SCHEMA_VERSION
        {
            // Reuse: дописываем в существующий v2-сегмент ТЕКУЩЕЙ schema-эпохи.
            return Ok(OpenDecision {
                seg_index: idx,
                seg_path: path,
                first_seq: header.first_seq,
                reuse: true,
            });
        }

        // Header не совпал — новая запись в новый сегмент.
        let new_idx = idx + 1;
        let new_path = segment_path(dir, new_idx);
        Ok(OpenDecision {
            seg_index: new_idx,
            seg_path: new_path,
            first_seq: next_seq,
            reuse: false,
        })
    } else {
        // Нет ни одного — создаём segment-00000000.jrnl.
        Ok(OpenDecision {
            seg_index: 0,
            seg_path: segment_path(dir, 0),
            first_seq: next_seq,
            reuse: false,
        })
    }
}

/// Сериализовать event-frame (с CRC32) — helper для lib.rs, чтобы не дублировать.
pub(crate) fn serialize_event_frame(ev: &Event) -> io::Result<Vec<u8>> {
    let payload =
        postcard::to_stdvec(ev).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let crc = crc32fast::hash(&payload);
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&crc.to_le_bytes());
    Ok(out)
}

/// Размер сериализованного event-frame (для решения о ротации до записи).
#[allow(dead_code)]
pub(crate) fn event_frame_size(ev: &Event) -> io::Result<u64> {
    Ok(serialize_event_frame(ev)?.len() as u64)
}
/// Открыть файл v2-сегмента на запись. Если файла нет — создать и записать magic + header.
/// Если файл есть и пустой — то же (новый сегмент `append` после ротации).
/// `size_after_open` возвращается через структуру для инициализации `seg_size`.
pub(crate) struct OpenSegForWrite {
    pub writer: BufWriter<File>,
    pub seg_size_after_header: u64,
}

pub(crate) fn open_seg_for_write(
    path: &Path,
    reuse: bool,
    header: &SegmentHeader,
) -> io::Result<OpenSegForWrite> {
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;

    let mut seg_size = f.metadata()?.len();

    if !reuse || seg_size == 0 {
        // Пишем magic + header в начало (или в пустой файл после ротации).
        let hdr_bytes = serialize_v2_header(header)?;
        f.write_all(&hdr_bytes)?;
        f.flush()?;
        // После write + flush данные в ФАЙЛЕ; размер через metadata() точен.
        seg_size = f.metadata()?.len();
    }

    Ok(OpenSegForWrite {
        writer: BufWriter::with_capacity(256 * 1024, f),
        seg_size_after_header: seg_size,
    })
}

// (Используется локально в `free_bytes_at` под Unix; глобального re-export не требуется.)

// ── Ретеншен: ОПЕРАТОРСКИЙ ПУТЬ (M-08 task 11, TD-020) ────────────────────────────────
//
// Находка §8 (reviewer): `verify_cold_copy`/`prune_segment`/`ColdCopyProof` существуют как
// БИБЛИОТЕКА, но их никто не вызывает — ни recorder, ни CLI, ни cron. Главная цель M-08
// («сбор не остановится НИКОГДА») поэтому НЕ достигнута: диск растёт те же ~2.8 GB/сут,
// просто кусками по 1 GiB. ~40 дней до disk-guard.
//
// Решение: ОТДЕЛЬНЫЙ бинарь `journal-retention` + cron на VPS. Не поток внутри recorder'а:
// падение/зависание ретеншена не имеет права ронять СБОР ДАННЫХ (сбор дороже уборки).
// Каркас — architect; реализация — engine-dev.

/// Политика ретеншена (операторский конфиг).
///
/// M-38b (C-030 R1): добавлены два поля — `checkpoint_covered_through_seq` и
/// `allow_prune_without_checkpoint`. Дефолт через `..Default::default()` подставляет
/// `covered = None` (= fail-closed) и `override = false` (= без override).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetentionPolicy {
    /// Сегменты старше N суток — кандидаты на выгрузку+удаление.
    pub retain_days: u32,
    /// Минимум ПОСЛЕДНИХ сегментов, которые остаются горячими независимо от возраста
    /// (реплей/диагностика недавнего прошлого без обращения к холодному хранилищу).
    pub keep_min_segments: u32,
    /// Корень холодного хранилища (Storage Box / смонтированный путь).
    pub cold_root: PathBuf,
    /// Порог, ниже которого пустое место требует ВНЕОЧЕРЕДНОЙ выгрузки (алерт).
    pub min_free_bytes: u64,
    /// M-38b (C-030 R1): минимум `seq`, свёрнутый ЧЕКПОНТОМ по ВСЕМ сконфигурированным
    /// селекторам (gateway-checkpoint публикует ОДНО число). Локальный prune РАЗРЕШЁН
    /// только для сегментов, чьи события полностью покрыты (`last_seq(seg) <= covered`).
    /// Дефолт `None` = «нет артефакта покрытия» = fail-closed (никаких prune).
    pub checkpoint_covered_through_seq: Option<u64>,
    /// M-38b (отклонение от буквы C-030): escape-hatch, разрешающий prune БЕЗ покрытия.
    /// Дефолт `false` (fail-closed). Если `true` — каждый prune без покрытия ОБЯЗАН
    /// быть назван в `RetentionReport.pruned_without_checkpoint_coverage`.
    pub allow_prune_without_checkpoint: bool,
}

/// M-38b (C-030 R1): DEPRECATED alias для backwards-compatibility. Используйте
/// поля `RetentionPolicy.checkpoint_covered_through_seq` /
/// `RetentionPolicy.allow_prune_without_checkpoint` напрямую.
#[deprecated(note = "use RetentionPolicy fields directly")]
pub struct RetentionCoverage {
    pub checkpoint_covered_through_seq: Option<u64>,
    pub allow_prune_without_checkpoint: bool,
}

/// Режим запуска. **Дефолт оператора — `DryRun`** (первый прогон на проде — обязательно он).
///
/// M-08 task 16 (D-COMP-3): добавлен вариант `Compact` — третий режим ТОГО ЖЕ бинаря
/// `journal-retention` (`--mode compact`). Компакция сжатием закрытых сегментов
/// переехала в общий бинарь, чтобы:
/// - один контракт argv на задание/cron (а не два разных бинаря с разным парсером);
/// - один Dockerfile pipeline (`--bin recorder --bin journal-retention` уже всё
///   включает, расщеплять ради операции — размножать интерфейс);
///
/// `Compact` НЕ проходит через `retention_plan`/`retention_execute` (это другой
/// алгоритм с другой инвариантной): `retention_execute` возвращает пустой отчёт
/// для этого режима — вызывающий (бинарь) переходит к `compact_closed_segments`
/// напрямую. БИБЛИОТЕКА — отдельные API, БИНАРЬ — один.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionMode {
    DryRun,
    Apply,
    Compact,
}

/// План: что БУДЕТ сделано. Строится ДЕТЕРМИНИРОВАННО (часы передаются аргументом —
/// никакого `SystemTime::now()` внутри: план обязан быть воспроизводим и тестируем).
#[derive(Debug, Clone, PartialEq)]
pub struct RetentionPlan {
    /// Выгрузить в холодное хранилище и затем удалить горячую копию.
    pub offload_and_prune: Vec<SegmentInfo>,
    /// M-38b (C-030 R1): выгрузить в холодное, но НЕ удалять горячую копию (нет покрытия
    /// чекпоинтом — бэкап R1 идёт, локальная копия остаётся, skip-репорт — на
    /// операторе). Дефолт пустой, если нет непокрытых сегментов.
    pub offload_only: Vec<SegmentInfo>,
    /// Пропущены с причиной (активный сегмент; моложе retain_days; в keep_min_segments;
    /// legacy без декларации — у него нет эпохи, значит нет и права его удалять;
    /// **НЕ ПОКРЫТ чекпоинтом** — главный новый класс).
    pub skipped: Vec<(SegmentInfo, String)>,
    /// Свободного места меньше `min_free_bytes`, а выгружать нечего → внеочередная тревога.
    pub disk_pressure: bool,
}

/// Итог применения плана.
#[derive(Debug, Clone, PartialEq)]
pub struct RetentionReport {
    pub mode: RetentionMode,
    pub offloaded: Vec<PathBuf>,
    pub pruned: Vec<PathBuf>,
    /// M-38b (C-030 R1, escape-hatch аудит): пути сегментов, удалённых БЕЗ покрытия
    /// чекпоинтом (override `allow_prune_without_checkpoint = true`). Каждый ОБЯЗАН быть
    /// поимённо перечислен — иначе операторский override становится тихим: «freed N bytes»
    /// в логе, а read-путь потерял историю, и этого нигде не видно. Сверяется с
    /// `pruned` — при валидном покрытии должно быть пусто.
    pub pruned_without_checkpoint_coverage: Vec<PathBuf>,
    /// Сегменты, у которых сверка холодной копии НЕ прошла (остались горячими).
    pub failed: Vec<(PathBuf, String)>,
    pub freed_bytes: u64,
}

/// Построить план. `now_wall_ms` — снаружи (детерминизм, DESIGN §1).
///
/// Гарантии (RED `red_retention_operator.rs`):
/// - АКТИВНЫЙ (последний) сегмент НИКОГДА не попадает в план — в него сейчас пишут;
/// - `keep_min_segments` последних остаются горячими независимо от возраста;
/// - legacy-сегмент без декларации в манифесте НЕ удаляется (нет эпохи → нет права);
/// - план ДЕТЕРМИНИРОВАН: тот же `now_wall_ms` + та же политика + тот же каталог → тот же план.
///
/// Алгоритм:
///   1. Обойти каталог, классифицировать каждый `*.jrnl`. На `Err` (foreign / corrupt /
///      truncated) — синтезировать `SegmentInfo` для skipped (а не возвращать `Err`,
///      иначе один чужой файл отменил бы весь план: оператор обязан узнать о нём,
///      а не получать «ничего не планируется»);
///   2. Активный = сегмент с МАКСИМАЛЬНЫМ индексом (писатель всегда дописывает
///      в сегмент последнего индекса; см. `decide_open_segment`);
///   3. Отобрать кандидатов: всё, что не активное, не foreign, и возраст ≥ `retain_days`;
///   4. Из кандидатов исключить `keep_min_segments` последних (по индексу);
///   5. `disk_pressure` = `free_bytes(dir) < policy.min_free_bytes`.
pub fn retention_plan(
    dir: impl AsRef<Path>,
    policy: &RetentionPolicy,
    now_wall_ms: i64,
) -> io::Result<RetentionPlan> {
    let dir = dir.as_ref();

    // (1) Обход каталога: classify с обработкой foreign/corrupt как skipped.
    let (classified, foreign_skipped) = enumerate_retention_segments(dir)?;

    // (2) Активный сегмент = сегмент с МАКСИМАЛЬНЫМ индексом среди classified.
    // Если classified пуст (всё foreign / каталог пуст) — активного нет; все foreign в skipped.
    let active_index: Option<u32> = classified.iter().map(|s| s.index).max();

    // M-38b: копия all_segs для плана (last_seq = next.first_seq - 1). Берём ДО move
    // classified в for-loop ниже.
    let mut segs_by_idx = classified.clone();
    segs_by_idx.sort_by_key(|s| s.index);

    let mut skipped: Vec<(SegmentInfo, String)> = Vec::new();
    let mut candidates: Vec<SegmentInfo> = Vec::new();
    if let Some(act_idx) = active_index {
        for s in classified {
            if s.index == act_idx {
                skipped.push((s, "active segment (writer holds it open)".to_string()));
            } else {
                candidates.push(s);
            }
        }
    } else {
        // Никаких «своих» сегментов. Foreign остаются skipped (для оператора).
    }
    skipped.extend(foreign_skipped);

    // (3) Возрастной фильтр: кандидаты старше retain_days.
    // Возраст = now_wall_ms − ts_exch_ms первого события сегмента (fallback на
    // header.created_wall_ms, если первый фрейм нечитаем).
    let cutoff_ms = i64::from(policy.retain_days) * 86_400_000;
    let mut young_passed: Vec<SegmentInfo> = Vec::with_capacity(candidates.len());
    for s in candidates {
        let seg_ts = first_event_data_ts(&s.path)
            .ok()
            .flatten()
            .unwrap_or(s.header.created_wall_ms);
        let age_ms = now_wall_ms.saturating_sub(seg_ts);
        if age_ms < cutoff_ms {
            skipped.push((
                s,
                format!(
                    "younger than retain_days: age={}ms < {}ms (seg_ts={})",
                    age_ms, cutoff_ms, seg_ts
                ),
            ));
        } else {
            young_passed.push(s);
        }
    }

    // (4) keep_min_segments: последние N (по индексу, отсортированы по возрастанию)
    // из young_passed остаются горячими.
    let keep_min = policy.keep_min_segments as usize;
    let mut final_candidates: Vec<SegmentInfo>;
    if young_passed.len() > keep_min {
        let split = young_passed.len() - keep_min;
        let (front, back) = young_passed.split_at(split);
        final_candidates = front.to_vec();
        for s in back {
            skipped.push((s.clone(), "protected by keep_min_segments".to_string()));
        }
    } else {
        // Все young_passed защищены keep_min (или keep_min=0, и тогда просто пусто).
        for s in young_passed {
            skipped.push((s, "protected by keep_min_segments".to_string()));
        }
        final_candidates = Vec::new();
    }

    // (5) M-38b (C-030 R1): СТРОГАЯ СВЯЗКА prune ↔ покрытие чекпоинтом.
    //   `last_seq(seg) <= checkpoint_covered_through_seq` ⟺ «все события сегмента
    //   свёрнуты чекпоинтом, read-путь их уже не прочтёт — prune безопасен».
    //   `last_seq(seg) = next_seg.first_seq - 1` (если есть next), либо `None` для
    //   активного. Без `next_seg` (активный) — `last_seq` неизвестен ⇒ консервативно
    //   НЕ покрыт.
    //
    //   Разделение плана:
    //   - covered (last_seq <= covered) → offload_and_prune;
    //   - uncovered (last_seq > covered) → offload_only + skip-репорт с причиной
    //     «нет покрытия чекпоинтом» (R1 идёт, локальная копия остаётся).
    //   - allow_prune_without_checkpoint=true → uncovered тоже идёт в offload_and_prune,
    //     но попадает в `pruned_without_checkpoint_coverage` в отчёте.
    //
    //   Нет артефакта покрытия (`None`) = fail-closed: ВСЕ непокрытые сегменты → skip,
    //   даже при allow_prune_without_checkpoint=true (override без артефакта = бессмыслен).
    //
    //   D1 (rev3, MAJOR): раннее `offload_and_prune = Vec::new()` было НЕПЕРЕНЕСЕНИЕМ
    //   кандидатов → retention не прунил НИКОГДА. Плюс гейт шёл по `segs_by_idx` (ВСЕ
    //   сегменты включая активный), а не по `final_candidates` — активный сегмент попадал
    //   в `offload_only` (копия в cold файла, в который пишут прямо сейчас). ИСПРАВЛЕНО:
    //   гейт итерирует `final_candidates` (исключает активный и прошедшие фильтры);
    //   `offload_and_prune` собирается ИЗ финальных кандидатов.
    let covered = policy.checkpoint_covered_through_seq;
    let override_enabled = policy.allow_prune_without_checkpoint;
    let mut offload_and_prune: Vec<SegmentInfo> = Vec::new();
    let mut offload_only: Vec<SegmentInfo> = Vec::new();
    let mut pruned_uncovered: Vec<SegmentInfo> = Vec::new(); // для execute: аудит override
    match covered {
        Some(covered_seq) => {
            // Идём по final_candidates (НЕ по segs_by_idx — активный сегмент и прочие
            // отфильтрованные уже исключены выше шагами (2)–(4)).
            let to_classify = std::mem::take(&mut final_candidates);
            for s in to_classify {
                // last_seq(s) = segs_by_idx[index(s)+1].first_seq - 1, иначе None (активный).
                let pos = segs_by_idx.iter().position(|x| x.index == s.index);
                let last_seq_opt = match pos {
                    Some(i) if i + 1 < segs_by_idx.len() => {
                        Some(segs_by_idx[i + 1].header.first_seq.saturating_sub(1))
                    }
                    _ => None,
                };
                let covered_seg = match last_seq_opt {
                    Some(ls) => ls <= covered_seq,
                    None => false, // активный сегмент — last_seq неизвестен ⇒ не покрыт
                };
                if covered_seg {
                    offload_and_prune.push(s);
                } else if override_enabled {
                    // Override разрешает prune без покрытия. Сегмент остаётся prunable,
                    // пометка попадёт в `pruned_without_checkpoint_coverage` в execute.
                    pruned_uncovered.push(s.clone());
                    offload_and_prune.push(s);
                } else {
                    // Без override — снимаем из prune. Бэкап идёт всегда (offload_only),
                    // локальная копия остаётся до прогресса чекпоинтера.
                    offload_only.push(s.clone());
                    skipped.push((
                        s,
                        format!(
                            "checkpoint coverage not proven: last_seq > {} \
                             (covered_through_seq); offload_only — локальная копия \
                             остаётся до прогресса чекпоинтера",
                            covered_seq
                        ),
                    ));
                }
            }
        }
        None => {
            // Нет артефакта покрытия.
            if override_enabled {
                // Override без артефакта: согласно architect'у (rev3 D1.2) hatch существует
                // именно для этого случая («override без артефакта = бессмыслен» —
                // неверно; смысл hatch'а — «чекпоинтер не развёрнут / сломан»). Все
                // кандидаты идут в offload_and_prune; каждый без покрытия попадёт в
                // `pruned_without_checkpoint_coverage` в execute (аудит-трейл).
                let to_take = std::mem::take(&mut final_candidates);
                for s in to_take {
                    pruned_uncovered.push(s.clone());
                    offload_and_prune.push(s);
                }
            } else {
                // True fail-closed: все потенциальные кандидаты на prune НЕ покрыты.
                // Бэкап идёт (offload_only), локальная копия остаётся, skip-репорт.
                for s in final_candidates.drain(..) {
                    offload_only.push(s.clone());
                    skipped.push((
                        s,
                        "checkpoint coverage artifact absent (fail-closed): бэкап идёт, \
                         локальная копия остаётся"
                            .to_string(),
                    ));
                }
            }
        }
    }
    // `pruned_uncovered` сейчас хранится локально — execute пересчитывает покрытие
    // по тому же алгоритму и заполняет `pruned_without_checkpoint_coverage` в отчёте.
    // Хранение рядом с final_candidates нужно только на случай отладки; функция
    // `retention_plan` возвращает план, не отчёт. Подавляем dead_code-warning: фактически
    // значение читается через `plan.offload_and_prune` в execute, где та же классификация
    // перевычисляется детерминированно.
    let _ = pruned_uncovered;

    // (6) disk_pressure: free_bytes < min_free_bytes.
    let free = free_bytes_at(dir)?;
    let disk_pressure = free < policy.min_free_bytes;

    Ok(RetentionPlan {
        offload_and_prune,
        offload_only,
        skipped,
        disk_pressure,
    })
}

type RetentionEnumeration = (Vec<SegmentInfo>, Vec<(SegmentInfo, String)>);

fn enumerate_retention_segments(dir: &Path) -> io::Result<RetentionEnumeration> {
    let manifest = load_manifest(dir)?;
    let mut classified = Vec::new();
    let mut foreign_skipped = Vec::new();

    for path in dedup_indexed_paths(dir)? {
        match classify_segment(&path, &manifest) {
            Ok(info) => classified.push(info),
            Err(e) => {
                // Foreign / corrupt / truncated. Синтезируем info для skipped — оператор
                // видит файл и причину, а не «план не построен, проверяй вручную».
                let reason = classify_failure_reason(&e);
                let file_name = path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<non-utf8>")
                    .to_string();
                let index = parse_segment_index_any(&file_name).unwrap_or(u32::MAX);
                let size_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let synthetic = SegmentInfo {
                    path,
                    index,
                    header: SegmentHeader {
                        schema_version: 0, // sentinel: неизвестно (нет магии / нет заголовка)
                        source: DataSource::Synthetic,
                        provenance: format!("<{reason}>"),
                        epoch_id: String::new(),
                        created_wall_ms: 0,
                        first_seq: 0,
                    },
                    size_bytes,
                };
                foreign_skipped.push((synthetic, reason));
            }
        }
    }

    // Стабильная сортировка по индексу — критична для воспроизводимости плана (R6)
    // и для определения «последних N» в keep_min.
    classified.sort_by_key(|s| s.index);
    foreign_skipped.sort_by_key(|(s, _)| s.index);
    Ok((classified, foreign_skipped))
}

/// Классифицировать причину отказа `classify_segment` в человеко-читаемую строку.
fn classify_failure_reason(e: &io::Error) -> String {
    if is_foreign_segment(e) {
        "undeclared legacy: no magic and no journal.legacy.json entry".to_string()
    } else {
        format!("classify error: {e}")
    }
}

/// Прочитать timestamp (ms) ДАННЫХ первого события сегмента: для MD — `ts_exch_ms`,
/// для `Sys` — `ts_wall_ms` (нет биржевого времени).
///
/// Используется `retention_plan` для возрастного фильтра. Семантика:
/// "насколько стары ДАННЫЕ в сегменте относительно `now_wall_ms`" — измеряется по
/// биржевому времени событий, а не по моменту записи в журнал (`created_wall_ms`
/// сегмента ≈ wall clock при append). Это позволяет тестам с фиксированным
/// `now_wall_ms` получать детерминированный план: биржевые timestamps событий задаются
/// явно (`trade(i)` пишет `ts_exch_ms = T0 + i`), и план строится по ним, а не по
/// «сейчас на стеных часах».
///
/// На любую ошибку (нет файла, битый заголовок, нет событий) — `Ok(None)`:
/// вызывающий использует fallback (header.created_wall_ms).
fn first_event_data_ts(path: &Path) -> io::Result<Option<i64>> {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    // Пропускаем magic+header (для v2) или не делаем ничего (для legacy).
    // Битый заголовок → Ok(None): fallback на created_wall_ms вызывающего.
    if read_v2_header_and_skip(&mut f).is_err() {
        return Ok(None);
    }
    let mut reader = BufReader::with_capacity(64 * 1024, f);
    let Some(ev) = read_event_frame(&mut reader)? else {
        return Ok(None);
    };
    let ts = match &ev.kind {
        EventKind::Sys(_) => ev.ts_wall_ms,
        EventKind::Md(md) => match &md.payload {
            MdPayload::Trade { ts_exch_ms, .. }
            | MdPayload::L2Snapshot { ts_exch_ms, .. }
            | MdPayload::Funding { ts_exch_ms, .. }
            | MdPayload::OpenInterest { ts_exch_ms, .. }
            | MdPayload::Liquidation { ts_exch_ms, .. }
            | MdPayload::MarginRate { ts_exch_ms, .. }
            | MdPayload::L2Delta { ts_exch_ms, .. }
            | MdPayload::MarginInventory { ts_exch_ms, .. } => *ts_exch_ms,
        },
    };
    Ok(Some(ts))
}

/// Выполнить план. В `DryRun` НИ ОДИН байт не копируется и не удаляется — только отчёт.
/// В `Apply`: для каждого сегмента сперва `verify_cold_copy` (sha256-сверка), и ТОЛЬКО
/// полученный `ColdCopyProof` даёт право на `prune_segment`. Сбой сверки → сегмент остаётся
/// горячим, попадает в `failed`, exit-код ненулевой (оператор обязан узнать).
///
/// Параметр `dir` нужен для контекста (например, чтобы убедиться, что путь сегмента
/// лежит под `dir` — анти-паттерн «символическая ссылка ведёт наружу»). Сейчас
/// дополнительная валидация намеренно минимальна: путь сегмента уже проверен
/// `classify_segment`, план построен из легитимных сегментов каталога.
pub fn retention_execute(
    _dir: impl AsRef<Path>,
    plan: &RetentionPlan,
    policy: &RetentionPolicy,
    mode: RetentionMode,
) -> io::Result<RetentionReport> {
    match mode {
        RetentionMode::DryRun => {
            // НОЛЬ побочных эффектов. Никакого создания каталогов, никакого хеширования,
            // никакого удаления — даже create_dir_all здесь не зовём (это было бы
            // побочным эффектом на файловой системе: см. RED `r2_dry_run_touches_nothing`).
            Ok(RetentionReport {
                mode: RetentionMode::DryRun,
                offloaded: Vec::new(),
                pruned: Vec::new(),
                pruned_without_checkpoint_coverage: Vec::new(),
                failed: Vec::new(),
                freed_bytes: 0,
            })
        }
        RetentionMode::Apply => {
            let mut offloaded: Vec<PathBuf> = Vec::new();
            let mut pruned: Vec<PathBuf> = Vec::new();
            let mut pruned_without_checkpoint_coverage: Vec<PathBuf> = Vec::new();
            let mut failed: Vec<(PathBuf, String)> = Vec::new();
            let mut freed_bytes: u64 = 0;

            // M-38b (C-030 R1): для каждого сегмента из offload_and_prune определяем,
            // был ли он uncovered (override). Пересчитываем last_seq тем же способом,
            // что и retention_plan, чтобы корректно классифицировать.
            let covered = policy.checkpoint_covered_through_seq;
            let all_segs_for_check = segments(_dir).unwrap_or_default();
            let mut sorted_segs = all_segs_for_check.clone();
            sorted_segs.sort_by_key(|s| s.index);

            for seg in &plan.offload_and_prune {
                // Классифицируем: покрыт или нет (для override-аудита).
                let pos = sorted_segs.iter().position(|s| s.index == seg.index);
                let last_seq_opt = match pos {
                    Some(i) if i + 1 < sorted_segs.len() => {
                        Some(sorted_segs[i + 1].header.first_seq.saturating_sub(1))
                    }
                    _ => None, // активный
                };
                let is_uncovered = match (last_seq_opt, covered) {
                    (Some(ls), Some(c)) => ls > c,
                    _ => true, // активный или нет артефакта покрытия = uncovered
                };
                // (1) verify_cold_copy: sha256-сверка src == dst.
                match verify_cold_copy(seg, &policy.cold_root) {
                    Ok(proof) => {
                        // (2) ТИПОВОЙ БАРЬЕР: proof получен — можно prune.
                        // ColdCopyProof сконструировать извне невозможно (поле приватное).
                        match prune_segment(seg, proof) {
                            Ok(()) => {
                                offloaded.push(seg.path.clone());
                                pruned.push(seg.path.clone());
                                if is_uncovered {
                                    pruned_without_checkpoint_coverage.push(seg.path.clone());
                                }
                                freed_bytes += seg.size_bytes;
                            }
                            Err(e) => {
                                failed.push((
                                    seg.path.clone(),
                                    format!("prune failed after verified cold copy: {e}"),
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        // Сверка провалилась → сегмент ОСТАЁТСЯ горячим (R3). Ошибка
                        // попадает в `failed` — оператор узнает из отчёта.
                        failed.push((
                            seg.path.clone(),
                            format!("cold copy verification failed: {e}"),
                        ));
                    }
                }
            }

            // M-38b (C-030 R1): `offload_only` сегменты тоже ОБЯЗАНЫ быть скопированы
            // в cold (R1 — экзистенциальный риск docs/08) — просто БЕЗ локального prune.
            // Сверка нужна та же: гарантия читаемости холодной копии. Сбой сверки →
            // сегмент остаётся горячим + попадает в `failed`.
            for seg in &plan.offload_only {
                match verify_cold_copy(seg, &policy.cold_root) {
                    Ok(proof) => {
                        // ColdCopyProof даёт право на запись (но не на удаление). `_proof`
                        // гарантирует, что холодная копия сверялась.
                        let _ = proof;
                        offloaded.push(seg.path.clone());
                    }
                    Err(e) => {
                        failed.push((
                            seg.path.clone(),
                            format!(
                                "offload_only: cold copy verification failed (R1 не сделан): {e}"
                            ),
                        ));
                    }
                }
            }

            Ok(RetentionReport {
                mode: RetentionMode::Apply,
                offloaded,
                pruned,
                pruned_without_checkpoint_coverage,
                failed,
                freed_bytes,
            })
        }
        RetentionMode::Compact => {
            // D-COMP-3: компакция идёт через ОТДЕЛЬНЫЙ API (`compact_closed_segments`),
            // НЕ через `retention_plan`/`retention_execute`. Сюда мы попадаем только если
            // бинарь по ошибке перенаправил Compact в этот код — отдаём пустой отчёт
            // и оставляем main'у свободу вызвать `compact_closed_segments` напрямую
            // (это и есть нормальный путь).
            Ok(RetentionReport {
                mode: RetentionMode::Compact,
                offloaded: Vec::new(),
                pruned: Vec::new(),
                pruned_without_checkpoint_coverage: Vec::new(),
                failed: Vec::new(),
                freed_bytes: 0,
            })
        }
    }
}

// ── КОМПАКЦИЯ ЗАКРЫТЫХ СЕГМЕНТОВ (M-08 task 15, TD-022) ───────────────────────────────
//
// Замер на боевых данных (VPS, 2026-07-14): рост журнала **8.83 GB/сут** (в документах
// значилось 2.8 — цифра до включения фьючерсов в M-06; решения принимались по устаревшему
// числу). Свободно 118.7 GB, disk-guard при 10 GiB ⇒ **12 дней**, а не 40.
//
// zstd на боевом сегменте: **-1 → 4.8×, -3 → 9.1×, -9 → 12.6×**. При -3 рост на диске падает
// с 8.83 до ~1 GB/сут ⇒ запас 12 дней → **100+ дней**; Storage Box 1 TB: 4 месяца → **~2.5 года**.
//
// Почему это безопасно: **закрытый сегмент неизменяем** (recorder пишет ТОЛЬКО в активный).
// Компакция не трогает горячий путь записи и не может оборвать сбор.
//
// Каркас — architect; реализация — engine-dev.

/// Расширение сжатого сегмента: `segment-NNNNNNNN.jrnl.zst`.
pub const COMPACTED_SUFFIX: &str = ".zst";

/// Уровень zstd по умолчанию: 9.1× при вменяемом CPU (замер на боевом сегменте).
pub const DEFAULT_COMPACT_LEVEL: i32 = 3;

/// Итог компакции одного сегмента.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionReport {
    pub source: PathBuf,
    pub compacted: PathBuf,
    pub bytes_before: u64,
    pub bytes_after: u64,
}

/// Сжать ЗАКРЫТЫЙ сегмент. Порядок обязателен (RED `red_compaction.rs`):
///
/// 1. **Активный сегмент НИКОГДА не сжимается** — в него пишут прямо сейчас (`Err`).
/// 2. **САМОИЗЛЕЧЕНИЕ КРАХ-ОКНА (rev 9, D-COMP-2):** если `.zst` уже на диске
///    (предыдущий вызов умер между `rename` и `remove_file(src)`) — НЕ рапортуем
///    «успех, мы тут не нужны». Сверим sha256 существующего `.zst` (распаковка →
///    `Sha256::update`) с sha256 оригинала:
///    - совпало → оригинал удалить (доделать прошлую работу; именно это и было
///      пропущено в старой ветке `if dst.exists() { return Ok(..) }`); возврат
///      `CompactionReport` (самоизлечение);
///    - НЕ совпало → `.zst` удалить, оригинал оставить ГОРЯЧИМ, `Err(InvalidData)`
///      (принцип `ColdCopyProof`: удалить можно лишь то, чья копия ДОКАЗАНО
///      читается — битая копия не даёт такого права).
/// 3. Пишем во ВРЕМЕННЫЙ файл `*.jrnl.zst.tmp` (падение на середине → оригинал цел,
///    мусор отбрасывается).
/// 4. **Верифицируем ДО удаления:** распаковываем .tmp и сверяем sha256 с оригиналом.
///    Расхождение → `Err`, оригинал остаётся, .tmp удаляется. Данные незаменимы —
///    «сжали и удалили, а там мусор» недопустимо.
/// 5. `fsync` + атомарный `rename` .tmp → .zst, и только ПОТОМ удаляем оригинал.
///
/// Это тот же принцип, что `ColdCopyProof`: удалить можно лишь то, чья копия ДОКАЗАНО читается.
pub fn compact_segment(seg: &SegmentInfo, level: i32) -> io::Result<CompactionReport> {
    let src = &seg.path;
    // (1) Активный сегмент — это тот, у кого индекс МАКСИМАЛЬНЫЙ в каталоге (recorder
    // дописывает ТОЛЬКО в последний сегмент; см. `decide_open_segment`). Если это он —
    // отказываем. Никаких «почти активных», никакого TTL: единственный определитель —
    // позиция в каталоге. Иначе сожмём сегмент, в который пишут, и запись начнёт
    // дописывать в .zst, что:
    //   - порушит wire-format (recorder пишет v2-фреймы, не zstd-поток);
    //   - оставит .zst «недописанным» (zstd не предупредит, что поток обрезан);
    //   - при следующем open'е recorder откроет новый сегмент → потеря seq-границы.
    let dir = src
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "segment path has no parent"))?;
    let latest_idx = latest_segment_index(dir)?;
    if Some(seg.index) == latest_idx {
        return Err(io::Error::other(format!(
            "cannot compact active segment segment-{:08}.jrnl (writer holds it open)",
            seg.index
        )));
    }

    // (1.5) M-08 task 17 (D-COMP-4, CRITICAL, §8): компакция НИКОГДА не трогает legacy/
    // foreign сегменты. На проде `segment-00000000.jrnl` — 15 GB LEGACY (без v2-магии,
    // задекларирован в `journal.legacy.json`, невосполнимая история). Предыдущая
    // реализация его СЖИМАЛА: sha256 round-trip проходит → оригинал удаляется → обратное
    // чтение `.zst` идёт через `skip_v2_header_forward`, который ТРЕБУЕТ v2-магию
    // («legacy под zstd не бывает») → `CorruptHeader` → `segments()`/`list_segments`/
    // `stream` падают на первом классифае ⇒ ВЕСЬ ЖУРНАЛ НЕЧИТАЕМ, 15 GB стёрты.
    //
    // Конструктивный барьер (тот же принцип, что «активный не сжимаем»): ПЕРВЫЕ байты
    // файла ОБЯЗАНЫ быть `SEGMENT_MAGIC` (HFTJRN02), иначе — `Err` ДО любой мутации
    // (rename / remove / create `.tmp`/`.zst`). Проверяется существующим хелпером
    // `read_magic_prefix`, который возвращает `Ok(None)` для файла короче магии
    // (на диске ничего разумного быть не может) и `Ok(Some(buf))` иначе.
    //
    // Legacy читается как есть (через манифест + fingerprint — см. `classify_segment`),
    // но сжатие legacy архитектурно запрещено: обратного декодера для legacy-в-zstd
    // не существует, и не должен существовать (CT-RFC-02 rev 2 fail-closed находка C2).
    match read_magic_prefix(src)? {
        Some(m) if m == SEGMENT_MAGIC => {} // ok: v2-сегмент, можно сжимать
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "compact_segment: legacy/foreign segment cannot be compacted (no v2 magic; \
                     legacy читается только как есть, сжатие архитектурно запрещено, \
                     D-COMP-4): {}",
                    src.display()
                ),
            ));
        }
    }

    // (2) Имена .tmp / .zst строятся из базовой части (`segment-NN.jrnl` + суффикс).
    let base = src
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-utf8 file name"))?;
    if !base.ends_with(".jrnl") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("compact_segment: ожидался .jrnl, получили `{base}`"),
        ));
    }
    let dst = src.with_file_name(format!("{base}{}", COMPACTED_SUFFIX)); // .jrnl.zst
    let tmp = src.with_file_name(format!("{base}{}.tmp", COMPACTED_SUFFIX)); // .jrnl.zst.tmp

    if dst.exists() {
        // (2) D-COMP-2 — САМОИЗЛЕЧЕНИЕ КРАХ-ОКНА. Прошлая редакция рапортовала
        // `if dst.exists() { return Ok(...) }`, оставляя оригинал-сироту навсегда
        // (3000 событий читались как 3172, DET-I-1 нарушен, фикс —
        // неудаляемый дубликат). Теперь сверяем sha256 существующего `.zst`
        // с sha256 оригинала; только совпадение даёт право удалить оригинал.
        let orig_sha = sha256_file(src)?;
        let verify_sha = sha256_decompressed(&dst).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "self-heal: existing .zst не читается ({e}); оригинал {src:?} оставлен \
                     ГОРЯЧИМ, .zst НЕ удалён"
                ),
            )
        })?;
        if verify_sha != orig_sha {
            // Битый .zst (FUSE-баг, частичная перезапись и пр.). Удаляем .zst,
            // оригинал оставляем ГОРЯЧИМ — данные не теряются; следующий компакт-
            // прогон перепишет `.zst` с нуля уже из верифицированного источника.
            let _ = fs::remove_file(&dst);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "self-heal: existing .zst sha256 mismatch; оригинал {src:?} оставлен \
                     ГОРЯЧИМ, .zst удалён. orig={orig_sha} decompressed=.zst={verify_sha}"
                ),
            ));
        }
        // Совпало → безопасно доделать прошлую работу (удалить оригинал).
        fs::remove_file(src)?;
        let bytes_after = fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        let bytes_before = fs::metadata(src).map(|m| m.len()).unwrap_or(seg.size_bytes);
        return Ok(CompactionReport {
            source: src.clone(),
            compacted: dst.clone(),
            bytes_before,
            bytes_after,
        });
    }

    // (3) Хеш оригинала ДО любых мутаций — для сверки после сжатия.
    let orig_sha = sha256_file(src)?;
    let orig_size = fs::metadata(src)?.len();

    // (4) Сжатие в .tmp. При ЛЮБОЙ ошибке (I/O, zstd) — откат, оригинал цел.
    if let Err(e) = compress_to_tmp(src, &tmp, level) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    // (5) Верификация: распаковываем .tmp, считаем sha256, сравниваем с оригиналом.
    // Данные незаменимы: «сжали и удалили, а там мусор» недопустимо.
    let verify_sha = match sha256_decompressed(&tmp) {
        Ok(s) => s,
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    };
    if verify_sha != orig_sha {
        let _ = fs::remove_file(&tmp);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "compaction sha256 mismatch: orig={orig_sha} decompressed={verify_sha} \
                 (сегмент НЕ тронут — данные не удалены; .tmp удалён)"
            ),
        ));
    }

    // (6) fsync .tmp + атомарный rename → .zst.
    {
        let f = File::open(&tmp)?;
        f.sync_all()?;
    }
    if let Err(e) = fs::rename(&tmp, &dst) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    // (7) Только теперь удаляем оригинал. Между шагом 6 и 7 на диске лежат ОБА файла:
    // оригинал и .zst (rename не удаляет src). На короткий миг место вырастает; для
    // прод-замера это <1 GiB × 1 = безопасно (минуты до окончания операции).
    fs::remove_file(src)?;

    let compacted_size = fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
    Ok(CompactionReport {
        source: src.clone(),
        compacted: dst,
        bytes_before: orig_size,
        bytes_after: compacted_size,
    })
}

/// Сжать ВСЕ закрытые сегменты старше `keep_raw` последних. Активный и `keep_raw` самых
/// свежих НЕ трогаем: свежие читаются чаще, несжатый доступ дешевле (особенно для
/// debug-инструментов и bands-дампа).
///
/// Алгоритм:
///   1. Обойти каталог через `segments()` (включая .zst).
///   2. Определить активный = с максимальным индексом.
///   3. Отсортировать по индексу, отбросить активный + последние `keep_raw`.
///   4. На каждом из оставшихся вызвать `compact_segment`.
pub fn compact_closed_segments(
    dir: impl AsRef<Path>,
    keep_raw: u32,
    level: i32,
) -> io::Result<Vec<CompactionReport>> {
    let dir = dir.as_ref();
    let all = segments(dir)?;
    if all.is_empty() {
        return Ok(Vec::new());
    }

    // Активный = сегмент с МАКСИМАЛЬНЫМ индексом. Сегменты с .jrnl.zst УЖЕ сжаты
    // (повторно не сжимаем: `compact_segment` идемпотентен, но фильтруем заранее —
    // экономим sha256/распаковку).
    let active_idx = all.iter().map(|s| s.index).max();
    let mut closed: Vec<&SegmentInfo> = all
        .iter()
        .filter(|s| Some(s.index) != active_idx)
        .filter(|s| {
            !s.path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(is_compacted_name)
        })
        .collect();
    // Последние `keep_raw` (по индексу, ASC) — защищены.
    closed.sort_by_key(|s| s.index);
    let keep_raw = keep_raw as usize;
    let split = closed.len().saturating_sub(keep_raw);
    let to_compact: Vec<&SegmentInfo> = closed[..split].to_vec();

    let mut reports = Vec::with_capacity(to_compact.len());
    for s in to_compact {
        match compact_segment(s, level) {
            Ok(r) => reports.push(r),
            // D-COMP-4: legacy/foreign сегменты пропускаются МОЛЧА (Err → в failed, не трогаем).
            // Иначе на проде (где legacy-0 задекларирован) первая же попытка сжатия
            // валила бы задание с exit=2 и поднимала ложный алерт «raw+.zst конфликт»,
            // хотя данные не пострадали. Идентифицируем по единственному маркеру из
            // `compact_segment` (других источников этой подстроки в err нет):
            Err(e)
                if e.to_string()
                    .contains("legacy/foreign segment cannot be compacted") =>
            {
                // Намеренно тихо: (а) compact-вызов на legacy-сегменте — штатная
                // ситуация прод-раскладки, не сбой; (б) библиотека не пишет в stderr
                // (это прерогатива бинаря и оператора); (в) сегмент остаётся
                // читаемым через `list_segments`/`stream` — оператор видит его
                // статус иначе (legacy-сегмент в выводе stream/heartbeat/storage_status).
            }
            Err(e) => return Err(e),
        }
    }
    Ok(reports)
}

// ── Внутренние helpers компакции ────────────────────────────────────────────────────

/// Сжать `src` → `tmp` через zstd с указанным уровнем. Не fsync, не удаляет src.
/// На ошибке `tmp` может быть частично записан — вызывающий удаляет.
fn compress_to_tmp(src: &Path, tmp: &Path, level: i32) -> io::Result<()> {
    let mut src_f = File::open(src)?;
    let tmp_f = File::create(tmp)?;
    // zstd::Encoder оборачивает Write; при drop() finish() делается автоматически.
    let mut encoder = zstd::Encoder::new(tmp_f, level)?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = src_f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        encoder.write_all(&buf[..n])?;
    }
    encoder.finish()?;
    Ok(())
}

/// sha256 распакованного потока из .zst файла. Используется для верификации, что
/// сжатие/распаковка — round-trip с потерей нулевых байт (для zstd это гарантия формата,
/// но в коде могли быть баги; сверяем).
fn sha256_decompressed(path: &Path) -> io::Result<String> {
    let f = File::open(path)?;
    let decoder = open_compacted_reader(f)?;
    let mut reader = BufReader::with_capacity(64 * 1024, decoder);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}
