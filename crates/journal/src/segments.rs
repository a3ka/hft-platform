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
    DataSource, Event, LegacyManifest, LegacySegmentDecl, SegmentHeader, LEGACY_FINGERPRINT_BYTES,
    SEGMENT_MAGIC,
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

/// M-08 task 10: прочитать ВСЕ события из одного сегмент-файла.
pub(crate) fn read_segment_events(path: &Path, strict: bool) -> io::Result<Vec<Event>> {
    if strict {
        read_segment_events_strict(path)
    } else {
        read_segment_events_tolerant(path)
    }
}

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

fn read_segment_events_tolerant(path: &Path) -> io::Result<Vec<Event>> {
    let mut f = File::open(path)?;
    let _hdr = read_v2_header_and_skip(&mut f)?;
    let mut data = Vec::new();
    f.read_to_end(&mut data)?;
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

    let index = parse_segment_index(&file_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a segment file"))?;

    let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
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

/// Извлечь индекс из имени `segment-NNNNNNNN.jrnl`.
fn parse_segment_index(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("segment-")?.strip_suffix(".jrnl")?;
    if rest.len() != 8 {
        return None;
    }
    rest.parse::<u32>().ok()
}

/// Обойти `segment-NNNNNNNN.jrnl` каталога по возрастанию индекса — БЕЗ классификации
/// по манифесту. Используется в `read_all`/`recover` (ОФЛАЙН-диагностика, не требует
/// декларации — в отличие от прод-пути `stream`, который через `list_segments`
/// отвергает чужие/незадекларированные файлы).
pub(crate) fn iter_segments_sorted(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out: Vec<(u32, PathBuf)> = Vec::new();
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
            out.push((idx, p));
        }
    }
    out.sort_by_key(|(idx, _)| *idx);
    Ok(out.into_iter().map(|(_, p)| p).collect())
}

/// Какие эпохи читатель СОГЛАСЕН смешивать — фильтр вызывается через `EpochFilter::accepts`.
///
/// ЕДИНСТВЕННЫЙ публичный путь `list_segments` — все сегменты каталога.
pub fn segments(dir: impl AsRef<Path>) -> io::Result<Vec<SegmentInfo>> {
    let dir = dir.as_ref();
    let manifest = load_manifest(dir)?;
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.extension().and_then(OsStr::to_str) != Some("jrnl") {
            continue;
        }
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
pub struct EventStream {
    segments: Vec<SegmentInfo>,
    selected_headers: Vec<SegmentHeader>,
    cursor: usize,
    reader: Option<BufReader<File>>,
    finished: bool,
}

impl EventStream {
    /// Заголовки сегментов, попавших в выборку — эпоха читаемо присутствует в отчёте.
    pub fn headers(&self) -> &[SegmentHeader] {
        &self.selected_headers
    }
}

impl EventStream {
    /// Продвигаем курсор к следующему сегменту. Возвращает:
    /// - `Ok(true)`  — сегмент открыт (cursor сдвинут, `self.reader = Some(_)`);
    /// - `Ok(false)` — сегментов больше нет;
    /// - `Err(_)`    — ошибка открытия файла сегмента (возвращается через `next()`).
    fn open_next_segment(&mut self) -> io::Result<bool> {
        if self.cursor >= self.segments.len() {
            self.reader = None;
            return Ok(false);
        }
        let seg = &self.segments[self.cursor];
        self.cursor += 1;
        let f = File::open(&seg.path)?;
        let mut r = BufReader::with_capacity(64 * 1024, f);
        // v2: пропустить magic+header. legacy: перемотки не было.
        if read_v2_header_and_skip(&mut r).ok().flatten().is_none() {
            // noop
        }
        self.reader = Some(r);
        Ok(true)
    }
}

impl Iterator for EventStream {
    type Item = io::Result<Event>;

    fn next(&mut self) -> Option<io::Result<Event>> {
        loop {
            if let Some(reader) = self.reader.as_mut() {
                match read_event_frame(reader) {
                    Ok(Some(ev)) => return Some(Ok(ev)),
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
    let all = segments(dir.as_ref())?;
    let mut selected = Vec::with_capacity(all.len());
    let mut headers = Vec::with_capacity(all.len());
    for s in all {
        if filter.accepts(&s.header) {
            headers.push(s.header.clone());
            selected.push(s);
        }
    }
    Ok(EventStream {
        segments: selected,
        selected_headers: headers,
        cursor: 0,
        reader: None,
        finished: false,
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

        if header.source == cfg.source
            && header.provenance == cfg.provenance
            && header.epoch_id == cfg.epoch_id
        {
            // Reuse: дописываем в существующий v2-сегмент.
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
