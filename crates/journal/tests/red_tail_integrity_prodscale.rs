//! SACRED (architect-only) — M-49 rev4 / **JR-I-8 на ПРОД-МАСШТАБЕ сегмента**.
//!
//! ## Дефект (найден reviewer'ом на PR-гейте rev3, `research/reviews/R-001-M-49.md`)
//!
//! Хвостовой скан читает последние `TAIL_SCAN_CHUNK = 4 MiB` файла. Если сырой сегмент
//! БОЛЬШЕ этого окна, буфер начинается не с нуля, магии сегмента в нём нет ⇒ флаг
//! `had_header` остаётся `false` ⇒ страж JR-I-8 НЕ СРАБАТЫВАЕТ ⇒ `tail_last_seq_of`
//! возвращает `Ok(None)` ⇒ `resolve_next_seq_with` берёт `meta_seq` (при restore меты нет
//! ⇒ 0) ⇒ запись идёт в уже занятый диапазон `seq`.
//!
//! То есть ровно тот дефект, ради которого заведён M-49, остался жив **для файлов
//! прод-размера**.
//!
//! ## Почему это прод, а не экзотика
//!
//! `DEFAULT_MAX_SEGMENT_BYTES = 1 GiB` (`segments.rs:41`), recorder берёт дефолт — активный
//! сырой сегмент на проде **на три порядка больше окна скана**. При этом ВСЕ фикстуры
//! набора M-49 (`red_tail_integrity*.rs`) используют `max_segment_bytes: 8 * 1024`, то есть
//! проверяют исключительно ветку `had_header == true`.
//!
//! **Это прямое нарушение `.claude/rules/testing.md` §«Прод-масштаб для sacred I/O-путей»
//! (урок TD-011)** — правила, которое спека M-49 сама цитирует. Тот же класс, что TD-011:
//! `Journal::open` делал `read_to_end` на 2.65 GiB прод-сегменте, юнит-фикстуры в десятки
//! байт этого не видели, CI был зелёный, поймал только eyes-on на VPS.
//!
//! ## Почему файл отдельный
//!
//! Фикстура пишет >4 MiB (сотни тысяч событий) — это десятки секунд. Держать её в основном
//! наборе значит замедлить каждый прогон; отдельный тест-бинарь запускается прицельно и
//! входит в `verify_M-49.sh` отдельной строкой.

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

const T0: i64 = 1_752_000_000_000;
/// Окно хвостового скана (`journal::TAIL_SCAN_CHUNK`, приватная константа) — 4 MiB.
/// Дублируется здесь СОЗНАТЕЛЬНО: оракул обязан пережить её изменение и всё равно
/// проверять «файл больше окна» (см. setup-guard ниже).
const TAIL_SCAN_CHUNK: u64 = 4 * 1024 * 1024;

fn cfg() -> WriterConfig {
    WriterConfig {
        // Прод-режим: сегмент НЕ режется на мелкие куски (на проде дефолт 1 GiB).
        max_segment_bytes: 512 * 1024 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "prodscale tail fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
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

fn ls(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

// ═════════════════════════════════════════════════════════════════════════════════════
// TI-7 — JR-I-8 держится на сегменте БОЛЬШЕ окна хвостового скана
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn ti_7_jr_i_8_holds_for_segment_larger_than_tail_scan_window() {
    let dir = tempfile::tempdir().expect("dir");

    // Пишем ОДИН сегмент заведомо больше окна скана.
    let mut n: u64 = 0;
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        loop {
            j.append(trade(n)).expect("append");
            n += 1;
            if n.is_multiple_of(20_000) {
                j.flush().expect("flush");
                let sz: u64 = ls(dir.path())
                    .iter()
                    .filter(|f| f.ends_with(".jrnl"))
                    .map(|f| {
                        std::fs::metadata(dir.path().join(f))
                            .map(|m| m.len())
                            .unwrap_or(0)
                    })
                    .sum();
                if sz > TAIL_SCAN_CHUNK + 1024 * 1024 {
                    break;
                }
            }
            assert!(
                n < 5_000_000,
                "фикстура не набрала объём — проверь размер события"
            );
        }
        j.flush().expect("flush");
    }

    let seg = ls(dir.path())
        .into_iter()
        .rfind(|f| f.ends_with(".jrnl"))
        .expect("сырой сегмент создан");
    let path = dir.path().join(&seg);
    let size = std::fs::metadata(&path).expect("meta").len();

    // Setup-guard: без этого условия оракул проверяет НЕ ТУ ветку (и молча зеленеет).
    assert!(
        size > TAIL_SCAN_CHUNK,
        "фикстура не состоялась: сегмент {seg} = {size} B, окно скана {TAIL_SCAN_CHUNK} B. \
         Оракул обязан проверять случай «файл БОЛЬШЕ окна», где буфер не содержит магии."
    );

    let history_max = journal::stream(dir.path(), EpochFilter::All)
        .expect("stream")
        .filter_map(|e| e.ok())
        .map(|e| e.seq)
        .max()
        .expect("история непуста");

    // Прод-режим restore: меты нет (ретеншен выгружает сегменты, не мету).
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    // Порча: последние 5 MiB забиваем мусором. Заголовок и почти вся история ЦЕЛЫ —
    // повреждён именно хвост, то есть окно скана содержит только мусор.
    let mut bytes = std::fs::read(&path).expect("read");
    let from = bytes.len().saturating_sub(5 * 1024 * 1024);
    for b in bytes[from..].iter_mut() {
        *b = 0x5A;
    }
    std::fs::write(&path, &bytes).expect("write");

    match Journal::open_with(dir.path(), cfg()) {
        Ok(mut j) => {
            j.append(trade(9_999_999)).expect("append");
            j.flush().expect("flush");
            drop(j);
            panic!(
                "JR-I-8 НЕ ДЕРЖИТСЯ НА ПРОД-МАСШТАБЕ: сегмент {seg} ({size} B > окна \
                 {TAIL_SCAN_CHUNK} B) с полностью повреждённым хвостом дал СТАРТ.\n\
                 ДОЛЖНО БЫТЬ: open_with = Err (последний seq не установлен ⇒ стартовать нельзя)\n\
                 ПОЛУЧЕНО: Ok — запись пошла с meta_seq (меты нет ⇒ 0) поверх истории \
                 0..{history_max}.\n\
                 Причина: буфер хвостового скана не достаёт до начала файла, магии в нём нет, \
                 флаг had_header остаётся false и страж JR-I-8 не срабатывает. На проде \
                 сегменты до 1 GiB — это НОРМА, а не край."
            );
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(&seg) || msg.to_lowercase().contains("tail"),
                "отказ обязан называть файл и причину: «{msg}»"
            );
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// TI-8 — ПАРНЫЙ vantage: большой ЗДОРОВЫЙ сегмент стартует штатно
// ═════════════════════════════════════════════════════════════════════════════════════

/// Ужесточение обязано быть точным: recorder перезапускается на большом сегменте каждый
/// деплой. Если страж начнёт отказывать на здоровом файле больше окна скана — это
/// остановка сбора данных на проде.
#[test]
fn ti_8_large_healthy_segment_starts_and_continues_seq() {
    let dir = tempfile::tempdir().expect("dir");
    let mut n: u64 = 0;
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        loop {
            j.append(trade(n)).expect("append");
            n += 1;
            if n.is_multiple_of(20_000) {
                j.flush().expect("flush");
                let sz: u64 = ls(dir.path())
                    .iter()
                    .filter(|f| f.ends_with(".jrnl"))
                    .map(|f| {
                        std::fs::metadata(dir.path().join(f))
                            .map(|m| m.len())
                            .unwrap_or(0)
                    })
                    .sum();
                if sz > TAIL_SCAN_CHUNK + 1024 * 1024 {
                    break;
                }
            }
        }
        j.flush().expect("flush");
    }
    let before = journal::stream(dir.path(), EpochFilter::All)
        .expect("stream")
        .filter_map(|e| e.ok())
        .map(|e| e.seq)
        .max()
        .expect("история непуста");
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let mut j = Journal::open_with(dir.path(), cfg())
        .expect("большой ЗДОРОВЫЙ сегмент обязан стартовать — иначе это остановка сбора на проде");
    j.append(trade(9_999_999)).expect("append");
    j.flush().expect("flush");
    drop(j);

    let after = journal::stream(dir.path(), EpochFilter::All)
        .expect("stream")
        .filter_map(|e| e.ok())
        .map(|e| e.seq)
        .max()
        .expect("читается");
    assert!(
        after > before,
        "seq обязан продолжать историю без меты: было {before}, стало {after}"
    );
}
