//! journal — append-only журнал (docs/fa/journal.md). MVP.
//!
//! Инварианты (частично; полный набор JR-I-* пофазно):
//! - JR-I-1 единственный писатель: `Journal` не Clone/Sync-shared; writer один.
//! - seq тотальный порядок, монотонный, персистится (переживает рестарт recorder'а).
//! - JR-I-7 деньги в payload — fixed-point i64 (гарантируется типами `contracts`).
//!
//! Формат фрейма: [u32 LE len][postcard(Event) len байт][u32 LE crc32(payload)].
//! Сегмент: `segment-00000000.jrnl` (ротация — позже). Мета: `journal.meta` (next_seq u64 LE).

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use contracts::{Event, EventKind};

const META: &str = "journal.meta";
const SEGMENT: &str = "segment-00000000.jrnl";

pub struct Journal {
    seg: BufWriter<File>,
    meta_path: PathBuf,
    next_seq: u64,
    since_flush: u32,
    epoch: SystemTime,
}

impl Journal {
    /// Открыть/создать журнал в каталоге `dir`.
    ///
    /// `next_seq` АВТОРИТЕТНО выводится из скана последнего валидного фрейма сегмента
    /// (JR-I «seq монотонный, переживает рестарт» + DET-I-1). Мета (`journal.meta`)
    /// используется лишь как нижняя граница-подсказка: после unclean-рестарта (SIGKILL
    /// посреди батча) мета ОТСТАЁТ от сегмента, и доверие ей вызвало бы reuse seq.
    pub fn open(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        let meta_path = dir.join(META);
        let meta_next = read_meta(&meta_path)?;
        // Скан сегмента — источник истины: next_seq = last_valid_seq + 1.
        let seg_next = scan_next_seq(&dir.join(SEGMENT))?;
        let next_seq = meta_next.max(seg_next);
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

/// Прочитать все события сегмента по порядку (для replay/тестов). MVP.
/// STRICT: любой CRC-mismatch → Err (инвариант DET-I-1 exact-replay). Для
/// resync-толерантного чтения повреждённого журнала — `recover`.
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

/// Просканировать сегмент и вывести next_seq = (последний валидный seq)+1.
///
/// Толерантно к хвостовому обрыву фрейма (unclean-стоп посреди flush) и к CRC-mismatch
/// внутри сегмента: продолжает скан, отслеживая максимальный валидный seq. Это источник
/// истины для `next_seq` при `open()` — мета отстаёт и ей доверять нельзя (reuse seq).
fn scan_next_seq(seg_path: &Path) -> io::Result<u64> {
    let data = match File::open(seg_path) {
        Ok(mut f) => {
            let mut d = Vec::new();
            f.read_to_end(&mut d)?;
            d
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut next = 0u64;
    let mut i = 0usize;
    while i + 4 <= data.len() {
        let len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        if len == 0 || i + 4 + len + 4 > data.len() {
            i += 1; // рваный/хвостовой фрейм — ресинк по байту
            continue;
        }
        let payload = &data[i + 4..i + 4 + len];
        let crc = u32::from_le_bytes(data[i + 4 + len..i + 4 + len + 4].try_into().unwrap());
        if crc32fast::hash(payload) != crc {
            i += 1;
            continue;
        }
        match postcard::from_bytes::<Event>(payload) {
            Ok(ev) => {
                next = next.max(ev.seq + 1);
                i += 4 + len + 4;
            }
            Err(_) => i += 1,
        }
    }
    Ok(next)
}

/// Resync-толерантное чтение: возвращает ВСЕ валидные события сегмента, ресинхронизируясь
/// через рваные фреймы (unclean SIGKILL посреди BufWriter-flush оставляет торн-фрейм В
/// СЕРЕДИНЕ). В отличие от strict `read_all` (падает на первом CRC-mismatch ради DET-I-1),
/// `recover` при повреждении сдвигается по одному байту, пока не найдёт следующий валидный
/// фрейм (честный ресинк — без rand-детекта). Для восстановления накопленных прод-данных.
pub fn recover(dir: impl AsRef<Path>) -> io::Result<Vec<Event>> {
    let path = dir.as_ref().join(SEGMENT);
    let data = match File::open(&path) {
        Ok(mut f) => {
            let mut d = Vec::new();
            f.read_to_end(&mut d)?;
            d
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 4 <= data.len() {
        let len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        if len == 0 || i + 4 + len + 4 > data.len() {
            i += 1; // рваный/хвостовой фрейм — ресинк по байту
            continue;
        }
        let payload = &data[i + 4..i + 4 + len];
        let crc = u32::from_le_bytes(data[i + 4 + len..i + 4 + len + 4].try_into().unwrap());
        if crc32fast::hash(payload) != crc {
            i += 1; // CRC-mismatch (торн-фрейм) — ресинк
            continue;
        }
        match postcard::from_bytes::<Event>(payload) {
            Ok(ev) => {
                out.push(ev);
                i += 4 + len + 4;
            }
            Err(_) => i += 1, // payload не декодируется — ложное совпадение len/crc, ресинк
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
