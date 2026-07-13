//! journal — append-only журнал (docs/fa/journal.md). MVP.
//!
//! Инварианты (частично; полный набор JR-I-* пофазно):
//! - JR-I-1 единственный писатель: `Journal` не Clone/Sync-shared; writer один.
//! - seq тотальный порядок, монотонный, персистится (переживает рестарт recorder'а).
//! - JR-I-7 деньги в payload — fixed-point i64 (гарантируется типами `contracts`).
//!
//! Формат фрейма: [u32 LE len][postcard(Event) len байт][u32 LE crc32(payload)].
//! Сегмент: `segment-00000000.jrnl` (ротация — позже). Мета: `journal.meta` (next_seq u64 LE).
//!
//! M-05 (TD-011): `open()` выводит `next_seq` через ХВОСТОВОЙ скан сегмента O(1) памяти
//! (`scan_tail_for_last_seq`) — НЕ `read_to_end` всего файла. `recover()` — отдельная
//! функция для полного resync-чтения (полный проход, не в горячем пути).

use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use contracts::{Event, EventKind};

pub mod segments;

pub use segments::{
    declare_legacy, fingerprint, free_bytes, is_foreign_segment, is_storage_guard, prune_segment,
    segments as list_segments, storage_status, stream, verify_cold_copy, ColdCopyProof,
    EpochFilter, EventStream, SegmentInfo, StorageStatus, WriterConfig, LEGACY_MANIFEST,
};

const META: &str = "journal.meta";
const SEGMENT: &str = "segment-00000000.jrnl";

/// Размер хвостового чанка для `Journal::open()` — O(1) памяти на прод-масштабе
/// (2.65 GiB сегмент не должен грузиться целиком). 4 MiB ≪ 8 MiB-бюджет TD-011.
const TAIL_SCAN_CHUNK: usize = 4 * 1024 * 1024;

pub struct Journal {
    seg: BufWriter<File>,
    meta_path: PathBuf,
    next_seq: u64,
    since_flush: u32,
    epoch: SystemTime,
}

impl Journal {
    /// Открыть/создать журнал в каталоге `dir`. `next_seq` =
    /// max(мета, последний валидный фрейм сегмента + 1).
    ///
    /// **M-05 / TD-011:** хвостовой скан сегмента — bounded memory, не `read_to_end`.
    /// Мета может отставать (SIGKILL посреди батча); источник истины — сегмент.
    pub fn open(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        let meta_path = dir.join(META);
        let meta_seq = read_meta(&meta_path)?;

        // TD-011: bounded-memory tail scan, vec освобождается до открытия seg на запись.
        let seg_seq_plus_one = {
            let seg_path = dir.join(SEGMENT);
            scan_tail_for_last_seq(&seg_path)?.map(|s| s + 1)
        };
        let next_seq = match seg_seq_plus_one {
            Some(s) => max(meta_seq, s),
            None => meta_seq,
        };

        let seg_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(SEGMENT))?;
        Ok(Self {
            seg: BufWriter::new(seg_file),
            meta_path,
            next_seq,
            since_flush: 0,
            epoch: SystemTime::now(),
        })
    }

    /// Открыть журнал с provenance-конфигом (M-08 E2/E4, CT-RFC-02).
    ///
    /// Отличия от `open()` (legacy-путь, остаётся для совместимости):
    /// - каждый НОВЫЙ сегмент открывается заголовком `SegmentHeader` (CT-I-6);
    /// - при превышении `max_segment_bytes` сегмент РОТИРУЕТСЯ (`seq` продолжается сквозь
    ///   границу — тотальный порядок один на журнал, JR-I-1);
    /// - при свободном месте < `min_free_bytes` запись останавливается ЯВНО (fail-closed:
    ///   `append` → `Err`), а не «пишет, пока диск не кончится».
    pub fn open_with(_dir: impl AsRef<Path>, _cfg: WriterConfig) -> io::Result<Self> {
        todo!("M-08 task 2 (engine-dev): заголовок сегмента + ротация + disk-guard")
    }

    /// Индекс активного сегмента (`segment-NNNNNNNN.jrnl`).
    pub fn active_segment_index(&self) -> u32 {
        todo!("M-08 task 2 (engine-dev)")
    }

    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Присвоить seq/метки, записать фрейм. Возвращает записанное событие.
    /// Единственный путь записи в журнал (JR-I-1).
    pub fn append(&mut self, kind: EventKind) -> io::Result<Event> {
        let now = SystemTime::now();
        let ts_wall_ms = now
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let ts_mono_ns = now
            .duration_since(self.epoch)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let ev = Event {
            seq: self.next_seq,
            ts_mono_ns,
            ts_wall_ms,
            kind,
        };
        let payload =
            postcard::to_stdvec(&ev).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let crc = crc32fast::hash(&payload);
        self.seg.write_all(&(payload.len() as u32).to_le_bytes())?;
        self.seg.write_all(&payload)?;
        self.seg.write_all(&crc.to_le_bytes())?;

        self.next_seq += 1;
        self.since_flush += 1;
        // Батч-flush (MD высокочастотный; JR-I fsync-политика по классам — уточним позже).
        if self.since_flush >= 64 {
            self.flush()?;
        }
        Ok(ev)
    }

    /// Сбросить буфер на диск + обновить мету (next_seq переживёт рестарт).
    pub fn flush(&mut self) -> io::Result<()> {
        self.seg.flush()?;
        write_meta(&self.meta_path, self.next_seq)?;
        self.since_flush = 0;
        Ok(())
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

fn read_meta(path: &Path) -> io::Result<u64> {
    match File::open(path) {
        Ok(mut f) => {
            let mut buf = [0u8; 8];
            f.read_exact(&mut buf)?;
            Ok(u64::from_le_bytes(buf))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}

fn write_meta(path: &Path, next_seq: u64) -> io::Result<()> {
    let tmp = path.with_extension("meta.tmp");
    std::fs::write(&tmp, next_seq.to_le_bytes())?;
    std::fs::rename(&tmp, path)?; // атомарная замена меты
    Ok(())
}

/// Хвостовой скан сегмента: последние ≤ `TAIL_SCAN_CHUNK` байт, вперёд с байт-ресинком.
/// Возвращает seq последнего валидного фрейма, либо `None` (если в хвосте мусор).
///
/// O(1) памяти (chunk фиксирован), O(chunk_size) времени в худшем случае (полный
/// байт-скан если хвост начинается с мусора). Для прод-сегмента 2.65 GiB с
/// валидными фреймами — мгновенно: находим первый валидный len в первых КБ.
fn scan_tail_for_last_seq(seg_path: &Path) -> io::Result<Option<u64>> {
    let mut file = match File::open(seg_path) {
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
    drop(file); // закрываем fd до парсинга — минимизируем peak alloc.

    let mut last_valid_seq: Option<u64> = None;
    let mut i = 0usize;

    while i < buf.len() {
        if i + 4 > buf.len() {
            break; // не хватает байт на len-поле
        }
        let len = u32::from_le_bytes(buf[i..i + 4].try_into().unwrap()) as usize;

        // Защита от переполнения и torn: иначе это мусор — байт-ресинк.
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
            i += 1; // CRC fail — байт-ресинк
            continue;
        }

        match postcard::from_bytes::<Event>(payload) {
            Ok(ev) => {
                last_valid_seq = Some(ev.seq);
                i = frame_end;
            }
            Err(_) => {
                i += 1; // deserialize fail — байт-ресинк
            }
        }
    }

    Ok(last_valid_seq)
}

/// Прочитать все события сегмента по порядку (для replay/тестов). MVP.
/// **DET-I-1 strict**: первая CRC-ошибка → `Err`. Для resync используй `recover()`.
pub fn read_all(dir: impl AsRef<Path>) -> io::Result<Vec<Event>> {
    let path = dir.as_ref().join(SEGMENT);
    let mut data = Vec::new();
    match File::open(&path) {
        Ok(mut f) => {
            f.read_to_end(&mut data)?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 4 <= data.len() {
        let len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        if i + len + 4 > data.len() {
            break; // хвостовой обрыв фрейма — игнор (JR-I-2: полный фрейм или ничего)
        }
        let payload = &data[i..i + len];
        let crc = u32::from_le_bytes(data[i + len..i + len + 4].try_into().unwrap());
        if crc32fast::hash(payload) != crc {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "crc mismatch"));
        }
        let ev: Event = postcard::from_bytes(payload)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        out.push(ev);
        i += len + 4;
    }
    Ok(out)
}

/// **M-05 task 4 / J3:** resync-толерантное чтение через рваные фреймы.
///
/// Полный проход по сегменту. На CRC-ошибке / torn / десериализации — байт-ресинк
/// вперёд до следующего валидного фрейма. Возвращает ВСЕ валидные события в порядке
/// seq. Отдельная функция от `read_all` (DET-I-1 strict), полный проход (НЕ в горячем
/// `open()`) — для CLI-инструмента восстановления прод-журнала.
///
/// Принимает `dir` (каталог журнала) — чтобы соответствовать API `read_all`.
/// **Не** bounded-memory: читает ВЕСЬ сегмент в RAM. Для прод 2.65 GiB — ОК
/// только как offline-инструмент, не как часть `open()`.
pub fn recover(dir: impl AsRef<Path>) -> io::Result<Vec<Event>> {
    let path = dir.as_ref().join(SEGMENT);
    let mut data = Vec::new();
    match File::open(&path) {
        Ok(mut f) => {
            f.read_to_end(&mut data)?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
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
                i += 1; // переполнение → мусор → байт-ресинк
                continue;
            }
        };
        if frame_end > data.len() {
            i += 1; // torn или мусорный len → байт-ресинк
            continue;
        }

        let payload = &data[i + 4..i + 4 + len];
        let stored_crc = u32::from_le_bytes(data[i + 4 + len..i + 4 + len + 4].try_into().unwrap());
        if crc32fast::hash(payload) != stored_crc {
            i += 1; // CRC fail → байт-ресинк
            continue;
        }

        match postcard::from_bytes::<Event>(payload) {
            Ok(ev) => {
                out.push(ev);
                i = frame_end;
            }
            Err(_) => {
                i += 1; // deserialize fail → байт-ресинк
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{EventKind, SysEvent};

    #[test]
    fn append_assigns_monotonic_seq_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut j = Journal::open(dir.path()).unwrap();
            for _ in 0..5 {
                j.append(EventKind::Sys(SysEvent::Heartbeat)).unwrap();
            }
            j.flush().unwrap();
        }
        let evs = read_all(dir.path()).unwrap();
        assert_eq!(evs.len(), 5);
        for (i, e) in evs.iter().enumerate() {
            assert_eq!(e.seq, i as u64);
        }
    }

    #[test]
    fn seq_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut j = Journal::open(dir.path()).unwrap();
            j.append(EventKind::Sys(SysEvent::Heartbeat)).unwrap();
            j.flush().unwrap();
        }
        let j2 = Journal::open(dir.path()).unwrap();
        assert_eq!(j2.next_seq(), 1); // продолжает, не сбрасывается в 0
    }
}
