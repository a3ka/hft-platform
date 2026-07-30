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

use contracts::{Event, EventKind, SegmentHeader};

pub mod segments;

pub use segments::{
    compact_closed_segments, compact_segment, CompactionReport, COMPACTED_SUFFIX,
    DEFAULT_COMPACT_LEVEL,
};
pub use segments::{
    declare_legacy, fingerprint, free_bytes, is_foreign_segment, is_storage_guard, prune_segment,
    segments as list_segments, storage_status, stream, stream_from, verify_cold_copy,
    ColdCopyProof, EpochFilter, EventStream, SegmentInfo, StorageStatus, WriterConfig,
    LEGACY_MANIFEST,
};
pub use segments::{
    retention_execute, retention_plan, RetentionMode, RetentionPlan, RetentionPolicy,
    RetentionReport,
};

// Доступно из `crate` для engine-dev call-sites внутри lib.rs.
pub(crate) use segments::{
    decide_open_segment, open_seg_for_write, resolve_next_seq_or_declared, segment_path,
    serialize_event_frame,
};
pub use segments::{FORCE_NEXT_SEQ_DECL, FORCE_NEXT_SEQ_DECL_APPLIED};

const META: &str = "journal.meta";
const SEGMENT: &str = "segment-00000000.jrnl";

/// Размер хвостового чанка для `Journal::open()` — O(1) памяти на прод-масштабе
/// (2.65 GiB сегмент не должен грузиться целиком). 4 MiB ≪ 8 MiB-бюджет TD-011.
pub(crate) const TAIL_SCAN_CHUNK: usize = 4 * 1024 * 1024;

pub struct Journal {
    /// Каталог журнала (нужен для `open_with` и ротации).
    dir: PathBuf,
    /// Файл активного сегмента (`segment-NNNNNNNN.jrnl` под `dir`). Для legacy-path —
    /// всегда `segment-00000000.jrnl`.
    seg_path: PathBuf,
    /// Индекс активного сегмента (для legacy-path = 0).
    seg_index: u32,
    /// Текущий размер сегмента (на диске), включая magic+header если применимо.
    seg_size: u64,
    seg: BufWriter<File>,
    meta_path: PathBuf,
    next_seq: u64,
    since_flush: u32,
    epoch: SystemTime,
    /// `None` — legacy-path (`Journal::open`); `Some` — новый путь с ротацией и disk-guard.
    cfg: Option<segments::WriterConfig>,
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
        let seg_size = seg_file.metadata()?.len();
        let seg_path = dir.join(SEGMENT);
        Ok(Self {
            dir: dir.to_path_buf(),
            seg_path,
            seg_index: 0,
            seg_size,
            seg: BufWriter::new(seg_file),
            meta_path,
            next_seq,
            since_flush: 0,
            epoch: SystemTime::now(),
            cfg: None,
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
    pub fn open_with(dir: impl AsRef<Path>, cfg: WriterConfig) -> io::Result<Self> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        let meta_path = dir.join(META);
        let dir_buf = dir.to_path_buf();

        // next_seq — источник истины сегмент, а не (возможно отставшая) мета (TD-011).
        //
        // M-49 (JR-I-8): если хвост нечитаем — `open_with` обязан отказать, а не молча
        // стартовать с `meta_seq` (при restore из холодного хранилища мета отсутствует ⇒ 0 ⇒
        // seq-reuse поверх истории). Операторский выход — файловая декларация
        // `journal.force-next-seq.json` (`resolve_next_seq_or_declared` её учитывает; при
        // читаемом хвосте декларация даже не читается — инертна).
        let next_seq = resolve_next_seq_or_declared(&dir_buf, &meta_path)?;

        // Решаем, какой сегмент открывать (reuse или новый). `next_seq` передаётся явно —
        // после операторского override он ОБЯЗАН совпасть с `SegmentHeader.first_seq` нового
        // сегмента, а не быть заново (заниженно) пересчитан.
        let decision = decide_open_segment(&dir_buf, &cfg, next_seq)?;

        // Строим заголовок для НЕ-reuse сегмента.
        let created_wall_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let header = SegmentHeader {
            schema_version: contracts::SCHEMA_VERSION,
            source: cfg.source,
            provenance: cfg.provenance.clone(),
            epoch_id: cfg.epoch_id.clone(),
            created_wall_ms,
            first_seq: decision.first_seq,
        };

        let opened = open_seg_for_write(&decision.seg_path, decision.reuse, &header)?;
        Ok(Self {
            dir: dir_buf,
            seg_path: decision.seg_path,
            seg_index: decision.seg_index,
            seg_size: opened.seg_size_after_header,
            seg: opened.writer,
            meta_path,
            next_seq,
            since_flush: 0,
            epoch: SystemTime::now(),
            cfg: Some(cfg),
        })
    }

    /// Индекс активного сегмента (`segment-NNNNNNNN.jrnl`).
    pub fn active_segment_index(&self) -> u32 {
        self.seg_index
    }

    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Состояние хранилища для heartbeat (E4/TD-019): свободное место + порог disk-guard.
    ///
    /// Для legacy-`open()` (без `cfg`) порог берётся 0 (никакого fail-closed не настроено);
    /// для `open_with()` — `cfg.min_free_bytes`. `writable` = `free_bytes >= min_free_bytes`.
    /// Ошибка чтения места возвращается как есть — heartbeat-потребитель сам решит, что
    /// с ней делать (recorder, например, логирует warn и пишет `null`-поля).
    pub fn storage_status(&self) -> io::Result<StorageStatus> {
        let min_free = self.cfg.as_ref().map(|c| c.min_free_bytes).unwrap_or(0);
        let free = segments::free_bytes_at(&self.dir)?;
        Ok(StorageStatus {
            free_bytes: free,
            min_free_bytes: min_free,
            writable: free >= min_free,
        })
    }

    /// Присвоить seq/метки, записать фрейм. Возвращает записанное событие.
    /// Единственный путь записи в журнал (JR-I-1).
    ///
    /// **M-08 (E2/E4):** при `cfg != None`:
    /// - свободное место < `cfg.min_free_bytes` → `Err(StorageGuard)`, **seq/байты НЕ сдвинуты**;
    /// - `cfg.max_segment_bytes` превышен → сегмент ротируется (новый `segment-{N+1}.jrnl`
    ///   с magic+header, `seq` продолжается сквозь границу).
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
        let frame = serialize_event_frame(&ev)?;
        let frame_len = frame.len() as u64;

        // === E4: disk-guard (fail-closed). Проверяем ДО записи. ===
        if let Some(cfg) = self.cfg.as_ref() {
            let free = segments::free_bytes_at(&self.dir)?;
            if free < cfg.min_free_bytes {
                return Err(segments::storage_guard_err());
            }
        }

        // === E2: ротация, если превышен порог размера (ДО записи, чтобы seq не сдвигался). ===
        if let Some(cfg) = self.cfg.as_ref() {
            if self.seg_size + frame_len > cfg.max_segment_bytes && self.seg_size > 0 {
                self.rotate()?;
            }
        }

        self.seg.write_all(&frame)?;
        self.seg_size += frame_len;

        self.next_seq += 1;
        self.since_flush += 1;
        // Батч-flush (MD высокочастотный; JR-I fsync-политика по классам — уточним позже).
        if self.since_flush >= 64 {
            self.flush()?;
        }
        Ok(ev)
    }

    /// Закрыть текущий сегмент, открыть `segment-{N+1}.jrnl` с новым magic+header.
    /// `seq` не меняется — сшивка честная (JR-I-1).
    fn rotate(&mut self) -> io::Result<()> {
        let cfg = match self.cfg.as_ref() {
            Some(c) => c.clone(),
            None => return Ok(()), // legacy-path не ротирует.
        };
        // Закрываем текущий сегмент (буфер сбрасывается и данные уходят в stable storage).
        self.seg.flush()?;
        // sync_data уже выполнен при предыдущем flush()/append-baseline; на ротации
        // дополнительный sync здесь обычно не нужен, но для безопасности — data-
        // durability важнее лишнего миллисекундного sys-вызова.
        self.seg.get_ref().sync_data()?;
        // Перемещаем writer'а во временный Option, дроп — без sentinel-файла.
        let old = std::mem::replace(&mut self.seg, dummy_writer());
        drop(old);

        // Следующий сегмент.
        let new_index = self.seg_index + 1;
        let new_path = segment_path(&self.dir, new_index);

        let created_wall_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let header = SegmentHeader {
            schema_version: contracts::SCHEMA_VERSION,
            source: cfg.source,
            provenance: cfg.provenance.clone(),
            epoch_id: cfg.epoch_id.clone(),
            created_wall_ms,
            first_seq: self.next_seq,
        };

        let opened = open_seg_for_write(&new_path, false, &header)?;

        // Перемещаем writer'а и состояние.
        self.seg_path = new_path;
        self.seg_index = new_index;
        self.seg_size = opened.seg_size_after_header;
        self.seg = opened.writer;
        Ok(())
    }

    /// Сбросить буфер на диск + обновить мету (next_seq переживёт рестарт).
    ///
    /// M-08 (TD-016 / JR-I): данные отправляются в stable storage (`sync_data`) —
    /// НЕ только в page-cache. RED-оракул `red_stream_bounded` ловит intermittent
    /// «torn frame при чтении» (page-cache ещё не сбросился, читаем то, что
    /// page-daemon успел). Batch-flush синхронизирует на каждые 64 события;
    /// HFT-допустимый loss-window ~64 events × snapshot ≈ 1-2 секунды MD-частоты.
    pub fn flush(&mut self) -> io::Result<()> {
        self.seg.flush()?;
        self.seg.get_ref().sync_data()?;
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

/// Временный BufWriter<File> для безопасного `mem::replace` без sentinel-файла.
/// Открывает `/dev/null` (Unix) или `NUL` (Windows); используется только на короткий
/// миг между `flush()` и `drop()` старого сегмента и открытием нового — в него
/// не пишут, не закрывают через Drop.
#[cfg(unix)]
fn dummy_writer() -> BufWriter<File> {
    use std::fs::OpenOptions;
    let f = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .expect("/dev/null");
    BufWriter::new(f)
}

#[cfg(not(unix))]
fn dummy_writer() -> BufWriter<File> {
    use std::fs::OpenOptions;
    let f = OpenOptions::new().write(true).open("NUL").expect("NUL");
    BufWriter::new(f)
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

/// Прочитать все события журнала по порядку (для replay/малых фикстур). M-08 task 10.
///
/// Обходит ВСЕ `segment-NNNNNNNN.jrnl` каталога по возрастанию индекса, сшивая их
/// в один `Vec<Event>` по порядку `seq`. v2-сегменты: пропускается magic + header.
/// legacy (без магии): читается как сырые фреймы с начала файла — БЕЗ требования
/// декларации в манифесте (`stream` требует, `read_all` — нет: это ОФЛАЙН-диагностика,
/// не прод-чтение). Если вендор подсунет мусор под нашим именем, он прочитается
/// как мусор; прод-путь `stream` его отвергнет.
///
/// **DET-I-1 strict**: первая CRC-ошибка / torn-фрейм → `Err` (ровно на одном сегменте,
/// без «silent drop»). Используется `dump.rs`/`bands.rs`/`obi_probe.rs` (диагностика),
/// НЕ прод-путь чтения (для прод — `stream`, O(1) памяти на сегмент).
pub fn read_all(dir: impl AsRef<Path>) -> io::Result<Vec<Event>> {
    let dir = dir.as_ref();
    let segs = segments::iter_segments_sorted(dir)?;
    let mut out = Vec::new();
    for seg in segs {
        out.extend(segments::read_segment_events(&seg, true)?);
    }
    Ok(out)
}

/// **M-05 task 4 / J3 + M-08 task 10:** resync-толерантное чтение всего журнала.
///
/// Полный проход по каталогу. На CRC-ошибке / torn / десериализации ВНУТРИ сегмента —
/// байт-ресинк вперёд до следующего валидного фрейма. Чужой/незадекларированный
/// безголовый сегмент — `Err` (через `list_segments`), как и в `stream()`.
///
/// Отдельная функция от `read_all` (strict), НЕ в горячем `open()` — для CLI-инструмента
/// восстановления прод-журнала. **Не** bounded-memory: читает ВЕСЬ сегмент в RAM.
/// Для прод 2.65 GiB — ОК только как offline-инструмент, не как часть `open()`.
pub fn recover(dir: impl AsRef<Path>) -> io::Result<Vec<Event>> {
    let dir = dir.as_ref();
    let segs = segments::iter_segments_sorted(dir)?;
    let mut out = Vec::new();
    for seg in segs {
        out.extend(segments::read_segment_events(&seg, false)?);
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
        // M-08 task 10: legacy-path сегмент (без магии) читается read_all ТОЛЬКО
        // через явную декларацию в journal.legacy.json (fail-closed CT-RFC-02 rev 2).
        declare_legacy(
            dir.path(),
            contracts::LegacySegmentDecl {
                file_name: "segment-00000000.jrnl".to_string(),
                fingerprint_sha256: String::new(),
                size_bytes_at_decl: 0,
                source: contracts::DataSource::OwnCapture,
                provenance: "lib.rs unit test".to_string(),
                epoch_id: contracts::LEGACY_EPOCH_ID.to_string(),
            },
        )
        .expect("declare_legacy");
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
