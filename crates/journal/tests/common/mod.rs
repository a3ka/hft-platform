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
use journal::{Journal, WriterConfig};

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

// ═════════════════════════════════════════════════════════════════════════════════════
// M-50 (TD-053) — помощники оракулов «крупное событие и скан пола»
// ═════════════════════════════════════════════════════════════════════════════════════

/// Кап переноса незавершённого фрейма в скане пола (`journal::READABLE_SCAN_MAX_CARRY`,
/// приватная константа крейта). Дублируется СОЗНАТЕЛЬНО (как `TAIL_SCAN_CHUNK`): оракулы
/// границы обязаны пережить её изменение и падать, если граница видимости сдвинулась
/// молча. Валидный фрейм в 65 536 B (кап включительно) обязан быть видим ВСЕГДА; фрейм
/// в кап+1 — предмет TD-053.
pub const FLOOR_SCAN_CARRY_CAP: usize = 64 * 1024;

/// Санити-кап длины фрейма штатного ридера (`read_frame_payload`, 64 MiB). Дублируется
/// сознательно: выше этого предела валидных фреймов не существует ДЛЯ ВСЕГО крейта.
pub const FRAME_LEN_SANITY_CAP: usize = 64 * 1024 * 1024;

/// Максимальный фрейм `L2Snapshot` по архитектурному потолку bucket-cap venue-binance
/// (3000 уровней/сторона): 66 032 B = 100.8% от 64 KiB. Форма прода, СНЯТА ЗАМЕРОМ
/// (`research/measurements/td-053-event-size.md` §2.3/§synthetic), не выдумана.
pub const PROD_L2SNAPSHOT_MAX_FRAME: usize = 66_032;

/// Константный `ts_mono_ns` для ручных фреймов (величина класса прод-значений).
pub const TS_MONO: u64 = 1 << 60;

/// i64-значение, чей postcard-varint (zigzag) занимает РОВНО `l` байт (1..=10).
/// Нужен для побайтовой подгонки размера события в `event_of_frame_size`.
pub fn val_of_varint_len(l: u32) -> i64 {
    assert!((1..=10).contains(&l), "varint i64: 1..=10 байт");
    if l == 1 {
        1
    } else {
        1i64 << (7 * (l as i64) - 8)
    }
}

/// Собрать ВАЛИДНОЕ событие (`L2Snapshot`, реальные типы контракта) с фреймом
/// (`4B len + payload + 4B crc`) РОВНО `target_frame` байт. Грубая подгонка — числом
/// уровней (14 B/уровень), тонкая — varint-классом значений последнего уровня.
/// Setup-guard: если точный размер недостижим (форма postcard изменилась) — паника,
/// а не молчаливо «примерно тот» размер (оракул границы обязан давить на границу).
pub fn event_of_frame_size(seq: u64, target_frame: usize) -> Event {
    let payload_target = target_frame
        .checked_sub(8)
        .expect("setup-guard: фрейм не меньше 8 байт");
    let base: i64 = val_of_varint_len(7); // 7-байтовый varint — класс реальных цен ×1e8
    let build = |n: usize, c1: u32, c2: u32| -> Event {
        let mut bids: Vec<Level> = vec![
            Level {
                price: base,
                size: base,
            };
            n
        ];
        if let Some(l) = bids.last_mut() {
            l.price = val_of_varint_len(c1);
            l.size = val_of_varint_len(c2);
        }
        Event {
            seq,
            ts_mono_ns: TS_MONO,
            ts_wall_ms: T0,
            kind: EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::L2Snapshot {
                    bids,
                    asks: Vec::new(),
                    ts_exch_ms: T0,
                },
            ),
        }
    };
    let probe = postcard::to_stdvec(&build(16, 7, 7)).expect("ser").len();
    let est = 16 + payload_target.saturating_sub(probe) / 14;
    for n in est.saturating_sub(6)..=est + 6 {
        if n == 0 {
            continue;
        }
        for c1 in 1..=10u32 {
            for c2 in 1..=10u32 {
                let ev = build(n, c1, c2);
                if postcard::to_stdvec(&ev).expect("ser").len() == payload_target {
                    return ev;
                }
            }
        }
    }
    panic!(
        "setup-guard: не удалось собрать событие с фреймом РОВНО {target_frame} B — \
         форма postcard изменилась, подгонку в event_of_frame_size нужно перекалибровать"
    );
}

/// ВАЛИДНЫЙ `L2Delta` (архитектурно неограниченный вариант, M-18) с фреймом ~`approx_frame`
/// байт. Точность не нужна — используется bounded-оракулом границы ПАМЯТИ.
pub fn l2delta_event_of_approx(seq: u64, approx_frame: usize) -> Event {
    let base: i64 = val_of_varint_len(7);
    let n = approx_frame / 14;
    Event {
        seq,
        ts_mono_ns: TS_MONO,
        ts_wall_ms: T0,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Delta {
                bids: vec![
                    Level {
                        price: base,
                        size: base,
                    };
                    n
                ],
                asks: Vec::new(),
                first_update_id: 1,
                final_update_id: 2,
                prev_final_update_id: None,
                ts_exch_ms: T0,
            },
        ),
    }
}

/// Сериализовать событие в on-disk фрейм `[u32 LE len][payload][u32 LE crc32]` — тот же
/// формат, что дублирует весь модуль (см. док-коммент вверху: оракул не имеет права
/// опираться на функцию крейта, которую проверяет).
pub fn frame_of(ev: &Event) -> Vec<u8> {
    let p = postcard::to_stdvec(ev).expect("ser");
    let mut out = Vec::with_capacity(p.len() + 8);
    out.extend_from_slice(&(p.len() as u32).to_le_bytes());
    out.extend_from_slice(&p);
    out.extend_from_slice(&crc32fast::hash(&p).to_le_bytes());
    out
}

/// Дописать сырые байты В КОНЕЦ файла (ручной валидный фрейм либо мусорный хвост).
pub fn append_bytes(path: &Path, bytes: &[u8]) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open append");
    f.write_all(bytes).expect("append bytes");
}

/// Общий assert оракулов M-50: декларация `next_seq` ВНУТРИ занятого диапазона обязана
/// быть ОТВЕРГНУТА. Контракт JR-I-9 допускает ДВЕ формы отказа — «строго больше
/// читаемого максимума» (`Known`) ИЛИ «пол непроверяем» (`Unknown`) — но не приём.
/// После проверки декларация убирается (housekeeping для следующей фазы фикстуры).
pub fn assert_decl_rejected(dir: &Path, cfg: WriterConfig, next_seq: u64, ctx: &str) {
    write_decl(
        dir,
        next_seq,
        "ошибка оператора: seq внутри занятого диапазона",
    );
    match Journal::open_with(dir, cfg) {
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            assert!(
                msg.contains("seq"),
                "{ctx}: отказ обязан объяснить причину (ожидается упоминание seq): «{e}»"
            );
            assert!(
                !ls(dir).iter().any(|n| n == DECL_APPLIED),
                "{ctx}: отвергнутая декларация не должна помечаться применённой"
            );
            let _ = std::fs::remove_file(dir.join(DECL));
        }
        Ok(_) => panic!(
            "JR-I-9 НАРУШЕН ({ctx}): декларация next_seq={next_seq} ВНУТРИ занятого \
             диапазона ПРИНЯТА → запись пойдёт поверх существующих seq (seq-reuse, \
             необратимая порча append-only журнала).\n\
             Скан пола обязан ВИДЕТЬ валидный крупный фрейм (CRC верифицируем потоково, \
             seq — ведущий varint payload) либо отказать как Unknown — но НЕ молча \
             трактовать размер как порчу (TD-053: кап carry 64 KiB против санити-капа \
             ридера 64 MiB; архитектурный потолок L2Snapshot уже 66 032 B = 100.8% капа)."
        ),
    }
}
