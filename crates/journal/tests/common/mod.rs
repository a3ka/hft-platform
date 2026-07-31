//! SACRED (architect-only) — общие фикстурные помощники оракулов M-49 rev5.
//!
//! ## Зачем отдельный модуль
//!
//! Дефект оракула, вскрытый `research/reviews/R-002-M-49-rev4.md`: эталон «максимального
//! читаемого seq» вычислялся АРИФМЕТИЧЕСКИ (`first_seq(жертвы) − 1`) и потому **по
//! построению исключал читаемый префикс жертвы** — то есть кодировал проверяемый дефект в
//! ожидание. Оракул не мог поймать регресс, который обязан был ловить.
//!
//! Здесь эталон **ИЗМЕРЯЕТСЯ**, а не выводится: `tolerant_readable_max` терпимо проходит
//! каждый файл каталога (сырой — как есть; `.zst` — потоковой распаковкой до первой ошибки)
//! и берёт максимальный `seq` РЕАЛЬНО распознанного фрейма. Это тот самый пол, который
//! обязана знать реализация (`Known` в контракте rev5), вычисленный независимо от неё.
//!
//! Формат фрейма дублируется здесь СОЗНАТЕЛЬНО (`[u32 LE len][payload][u32 LE crc32]`,
//! перед событиями — `SEGMENT_MAGIC` + header-фрейм): оракул не имеет права опираться на
//! ту самую функцию крейта, корректность которой он проверяет.

#![allow(dead_code)] // каждый тест-бинарь использует свой поднабор помощников

use std::io::Read;
use std::path::Path;

use contracts::{DataSource, Event, EventKind, Level, MdPayload, Side, Venue, SEGMENT_MAGIC};
use journal::WriterConfig;

pub const T0: i64 = 1_752_000_000_000;
pub const DECL: &str = "journal.force-next-seq.json";
pub const DECL_APPLIED: &str = "journal.force-next-seq.applied.json";

/// Окно хвостового скана (`journal::TAIL_SCAN_CHUNK`, приватная константа крейта).
/// Дублируется сознательно — оракулы обязаны пережить её изменение и всё равно проверять
/// случай «файл БОЛЬШЕ окна» (см. setup-guard'ы в оракулах прод-масштаба).
pub const TAIL_SCAN_CHUNK: u64 = 4 * 1024 * 1024;

pub fn cfg_with(max_segment_bytes: u64, provenance: &str) -> WriterConfig {
    WriterConfig {
        max_segment_bytes,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: provenance.to_string(),
        epoch_id: "own-test".to_string(),
    }
}

/// Мелкое прод-подобное событие (~48 B в сегменте).
pub fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + i as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: T0 + i as i64,
        },
    )
}

/// Крупное событие (~2.4 KiB): нужно там, где фикстуре требуется МНОГО БАЙТ при небольшом
/// числе append'ов — в частности, чтобы сжатый сегмент состоял из НЕСКОЛЬКИХ zstd-блоков
/// (иначе усечение убивает поток целиком и «читаемого префикса» не существует в принципе —
/// измерено architect'ом: одноблочный .zst после усечения даёт 0 B).
pub fn snap(i: u64) -> EventKind {
    let mk = |base: i64| -> Vec<Level> {
        (0..100)
            .map(|k| Level {
                price: base + k as i64 * 100 + i as i64,
                size: 1_000 + k as i64 + i as i64,
            })
            .collect()
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: mk(6_400_000_000_000),
            asks: mk(6_400_100_000_000),
            ts_exch_ms: T0 + i as i64,
        },
    )
}

pub fn ls(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

pub fn is_segment_name(name: &str) -> bool {
    name.starts_with("segment-") && (name.ends_with(".jrnl") || name.ends_with(".jrnl.zst"))
}

pub fn write_decl(dir: &Path, next_seq: u64, reason: &str) {
    let json = format!(
        r#"{{"next_seq": {next_seq}, "reason": "{reason}", "declared_at_ms": 1785362203969}}"#
    );
    std::fs::write(dir.join(DECL), json).expect("write decl");
}

/// Смещение конца header-фрейма (`magic(8) + len(4) + payload + crc(4)`), либо 0 для
/// legacy-сегмента без магии. Вычисляется ИЗ САМОГО ФАЙЛА — фиксированная константа уже
/// однажды сделала оракул слепым (дефект первой редакции `ti_3`, rev2).
pub fn header_end(data: &[u8]) -> usize {
    if !data.starts_with(&SEGMENT_MAGIC) {
        return 0;
    }
    let m = SEGMENT_MAGIC.len();
    if data.len() < m + 4 {
        return 0;
    }
    let h = u32::from_le_bytes(data[m..m + 4].try_into().unwrap()) as usize;
    match m
        .checked_add(4)
        .and_then(|x| x.checked_add(h))
        .and_then(|x| x.checked_add(4))
    {
        Some(end) if end <= data.len() => end,
        _ => 0,
    }
}

/// Максимальный `seq` среди РЕАЛЬНО распознанных фреймов буфера (байт-ресинк вперёд на
/// любой ошибке — как терпимый путь крейта). `None` — валидных фреймов нет вообще.
pub fn max_seq_in(data: &[u8]) -> Option<u64> {
    let mut i = header_end(data);
    let mut max: Option<u64> = None;
    while i + 8 <= data.len() {
        let len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        let end = match i
            .checked_add(4)
            .and_then(|x| x.checked_add(len))
            .and_then(|x| x.checked_add(4))
        {
            Some(e) if e <= data.len() => e,
            _ => {
                i += 1;
                continue;
            }
        };
        let payload = &data[i + 4..i + 4 + len];
        let crc = u32::from_le_bytes(data[i + 4 + len..end].try_into().unwrap());
        if crc32fast::hash(payload) != crc {
            i += 1;
            continue;
        }
        match postcard::from_bytes::<Event>(payload) {
            Ok(ev) => {
                max = Some(max.map_or(ev.seq, |m: u64| m.max(ev.seq)));
                i = end;
            }
            Err(_) => i += 1,
        }
    }
    max
}

/// Байты сегмента, прочитанные ТЕРПИМО: сырой — как есть; `.zst` — потоковой распаковкой
/// до первой ошибки (усечённый поток отдаёт свой читаемый ПРЕФИКС, а не ничего).
pub fn tolerant_bytes(path: &Path) -> Vec<u8> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if !name.ends_with(".zst") {
        return std::fs::read(path).unwrap_or_default();
    }
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut dec = match zstd::Decoder::new(f) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        match dec.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(_) => break, // обрыв потока: то, что уже распаковано, ЧИТАЕМО
        }
    }
    out
}

/// ЭТАЛОН оракулов rev5: фактически читаемый максимум `seq` каталога — с учётом читаемых
/// ПРЕФИКСОВ повреждённых сегментов. `None` ⇔ во всём каталоге не распознан ни один фрейм.
pub fn tolerant_readable_max(dir: &Path) -> Option<u64> {
    let mut max: Option<u64> = None;
    for name in ls(dir) {
        if !is_segment_name(&name) {
            continue;
        }
        if let Some(m) = max_seq_in(&tolerant_bytes(&dir.join(&name))) {
            max = Some(max.map_or(m, |c: u64| c.max(m)));
        }
    }
    max
}

/// Забить последние `n` байт файла мусором (порча ХВОСТА: заголовок и префикс целы).
pub fn corrupt_tail(path: &Path, n: usize) -> u64 {
    let mut bytes = std::fs::read(path).expect("read");
    let from = bytes.len().saturating_sub(n);
    for b in bytes[from..].iter_mut() {
        *b = 0x5A;
    }
    std::fs::write(path, &bytes).expect("write");
    (bytes.len() - from) as u64
}

/// Забить мусором ВСЁ ТЕЛО СОБЫТИЙ (после header-фрейма): сегмент остаётся непустым и
/// узнаваемым, но ни одного валидного фрейма в нём нет.
pub fn corrupt_body_after_header(path: &Path) {
    let mut bytes = std::fs::read(path).expect("read");
    let h = header_end(&bytes);
    for b in bytes[h..].iter_mut() {
        *b = 0x5A;
    }
    std::fs::write(path, &bytes).expect("write");
}
