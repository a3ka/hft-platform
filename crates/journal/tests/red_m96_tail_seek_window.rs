//! RED `M-96` (`TD-250`, `C-290` Б-2; sacred, architect-only) — **быстрый путь сдвига к курсору
//! (`seek_back_from_tail`) находит `after + 1` при ошибке оценки позиции В ОБЕ СТОРОНЫ, а не только
//! когда оценка не «перелетела» цель**.
//!
//! # Почему
//!
//! Тёплое возобновление подписки (`LiveReducer::resume` → `pump` → `stream_from_at_with_catalog`,
//! `hint = None`) ищет позицию `after + 1` в АКТИВНОМ сыром сегменте оценкой «от конца»:
//! `approx = file_len − (last_seq − after) × floor((file_len − header_end) / N)`, и читает окно
//! 8 КиБ ТОЛЬКО ВПЕРЁД от `approx` (`crates/journal/src/segments.rs` `seek_back_from_tail`).
//! Описание функции обещает «8 КиБ ВОКРУГ оценки», реализация читает вперёд. Если кадры хвоста в
//! среднем длиннее среднего по файлу хотя бы на долю байта, оценка ПЕРЕЛЕТАЕТ цель, первый кадр
//! окна несёт `seq > after + 1` ⇒ `Ok(None)` ⇒ откат на бисекцию (`locate_after_seq`, пробы по
//! 64 КиБ, ∝ log2 файла). Замер `C-290`: +185 КБ на 120 КБ файле, +594 КБ на 10 МиБ; на проде при
//! сегменте 1 ГиБ ≈ 1 МиБ лишнего чтения на подписку. Какая мода выпадет, решает длина varint
//! `ts_mono_ns` — то есть скорость записи на хосте: отсюда флак `red_m89_warm_resume_seek` `w1`
//! (`TD-250`, `R-248`).
//!
//! # Как судится — детерминированно, без зависимости от хоста
//!
//! Перекос длины кадров задаётся СОДЕРЖИМЫМ, а не временем: символ события хвоста длиннее (или
//! короче) символа префикса на `SKEW` байт. Каждый лишний символ — ровно один байт кадра (строка
//! postcard: varint-длина + байты), поэтому ошибка оценки ≈ `TAIL × SKEW` байт и её ЗНАК известен
//! заранее; шум varint времени (±2 Б/кадр) меньше перекоса и действует в ту же сторону для
//! «длинного хвоста» (хвост пишется позже ⇒ его `ts_mono_ns` не короче).
//!
//! Мера — `rchar` ПОТОКА (`/proc/thread-self/io`): байты, прочитанные `read()` этим потоком.
//! Стрим журнала читает в вызывающем потоке; поток-счётчик, а не процессный, — чтобы чужие потоки
//! теста не попадали в меру (`testing.md`, целостность гейта, свойство 2).
//!
//! Граница стоимости быстрого пути: 64 КиБ (хвостовой скан `last_seq`) + окно поиска (≤ 32 КиБ) +
//! байты самого хвоста + заголовок и служебное. Бисекция на файле `PREFIX` ≈ 5 МиБ — не меньше
//! 6 проб × 64 КиБ = 384 КиБ сверх: границы различают предметы с запасом в обе стороны.
//!
//! # Миры
//!
//! * `t1` — хвост ДЛИННЕЕ (+`SKEW` Б/кадр): оценка перелетает цель на ≈ `TAIL × SKEW` байт.
//!   Сегодня — откат на бисекцию ⇒ RED по стоимости.
//! * `t2` — хвост КОРОЧЕ (−`SKEW` Б/кадр): оценка не долетает; страж второй стороны (сегодня
//!   зелен — окно вперёд накрывает недолёт; исправление не смеет его сломать).
//! * `t3` — равномерные кадры: страж базового случая.
//! * `t4` — перелёт БОЛЬШЕ окна поиска (`TAIL_FAR × SKEW` ≫ 32 КиБ): быстрый путь обязан честно
//!   отказать, и бисекция обязана найти `after + 1` — корректность отката не куплена ценой поиска
//!   (страж анти-плацебо «окно = весь файл»: стоимость здесь ограничена бисекцией, не файлом).
//! * во всех мирах — эквивалентность: выдано РОВНО `after+1 ..= last`, без потерь и дублей, и
//!   `seek_fallbacks == 0` (откат к `header_end` = порча, его здесь нет).
//!
//! RUNTIME-RED: только существующие публичные символы (`stream_from_at`, `EventStream`).

mod common;

use std::io;
use std::path::Path;

use common::cfg_with;
use contracts::{Event, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

/// Префикс — ≈5 МиБ сырого сегмента (≈ 50 Б/кадр).
const PREFIX: u64 = 100_000;
/// Хвост, который читает первый `pump` после тёплого `resume` (прод-форма `R-211`: сотни событий).
const TAIL: u64 = 500;
/// Перекос длины кадра хвоста относительно префикса, байт. Ошибка оценки ≈ `TAIL × SKEW` = 3 000 Б:
/// больше любого округления `floor(avg)` (≤ 1 Б/кадр ⇒ ≤ 500 Б) и меньше окна в обе стороны.
const SKEW: usize = 6;
/// Хвост мира `t4`: перелёт ≈ `TAIL_FAR × SKEW` = 120 000 Б ≫ окна поиска.
const TAIL_FAR: u64 = 20_000;
const BIG_SEG: u64 = 1 << 30;
/// Хвост миров `t5`/`t6`: ошибка оценки ≈ 5…7 КиБ — между 4 КиБ и 8 КиБ (`C-291` B-1). Окно
/// уже 8 КиБ в нужную сторону её не накрывает, окно ≥ 8 КиБ — накрывает.
const TAIL_MID: u64 = 1_000;
/// Перекос `t5` (перелёт) и `t6` (недолёт), байт/кадр. Шум varint времени (хвост пишется позже ⇒
/// его кадры на 0…2 Б длиннее) сдвигает ошибку на +0…2 000 Б: для `t5` вверх (5 000 → ≤7 000),
/// для `t6` вниз (7 000 → ≥5 000). Фактическая ошибка сверяется setup-стражем `assert_error_band`.
const SKEW_OVER: usize = 5;
const SKEW_UNDER: usize = 7;
/// Полоса ошибки оценки для `t5`/`t6`: строго между 4 КиБ и 8 КиБ с запасом 768 Б.
const ERR_BAND: (i64, i64) = (4 * 1024 + 768, 8 * 1024 - 768);

/// Граница стоимости БЫСТРОГО пути сверх байт хвоста: 172 КиБ = 176 128 Б (`C-291` B-1).
///
/// Замер 2026-10-09 (`t1`, `t2`, `t3`; образцы откачены): служебное чтение честного пути —
/// хвостовой скан `last_seq` 64 КиБ, окно, заголовок, `journal.meta`, буферы стрима — постоянно:
/// окно 32 КиБ с началом за 16 КиБ до оценки — 164 237…164 732 Б; минимально допустимое окно
/// (8 КиБ назад + 8 КиБ вперёд) — 147 841…148 354 Б; окно 64 КиБ (32 КиБ назад) — 188 737 Б.
/// Предел ловит окно 64 КиБ (на 12 609 Б выше) и пропускает окно 32 КиБ (на 11 396 Б ниже). Чтение
/// ВПЕРЁД за конец файла стоит 0 байт, поэтому мера стоимости ограничивает в первую очередь часть
/// окна ПОЗАДИ оценки — она и есть цена. Откат на бисекцию на файле ≈5 МиБ — ≈ 676 КБ служебного
/// (замер на `ad20a7e`: 701 463 Б при хвосте 25 000 Б).
const FAST_PATH_OVERHEAD: u64 = 172 * 1024;
/// Граница стоимости ОТКАТА на бисекцию сверх байт хвоста (`t4`): ⌈log2(5 МиБ / 64 КиБ)⌉ + 4 проб
/// по 64 КиБ ≈ 11 × 64 КиБ, с запасом — 1 МиБ. Чтение файла целиком — ≈ 5 МиБ.
const BISECT_OVERHEAD: u64 = 1024 * 1024;

const BASE_SYMBOL: &str = "BTCUSDT";

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о сдвиге: фикстура не воспроизвела \
         сценарий, ради которого оракул написан."
    )
}

fn cfg() -> WriterConfig {
    cfg_with(BIG_SEG, "M-96 tail seek window")
}

fn ev(i: u64, symbol: &str) -> EventKind {
    EventKind::md(
        Venue::Binance,
        symbol,
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + (i % 1000) as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: 1_784_116_800_000 + i as i64,
        },
    )
}

/// Журнал: `prefix` событий с символом `BASE_SYMBOL`, затем `tail` событий с символом `tail_sym`.
/// Возвращает (каталог, `after` = последний `seq` префикса, байты хвоста в файле).
fn fixture(prefix: u64, tail: u64, tail_sym: &str) -> (tempfile::TempDir, u64, u64) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg())
        .unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
    for i in 0..prefix {
        j.append(ev(i, BASE_SYMBOL))
            .unwrap_or_else(|e| setup_failed(&format!("append #{i}: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
    let seg = only_segment(dir.path());
    let before_tail = std::fs::metadata(&seg).expect("meta").len();
    for i in prefix..prefix + tail {
        j.append(ev(i, tail_sym))
            .unwrap_or_else(|e| setup_failed(&format!("append #{i}: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
    drop(j);
    let file_len = std::fs::metadata(&seg).expect("meta").len();
    (dir, prefix - 1, file_len - before_tail)
}

fn only_segment(dir: &Path) -> std::path::PathBuf {
    let v: Vec<_> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jrnl"))
        .collect();
    if v.len() != 1 {
        setup_failed(&format!(
            "сырых сегментов {} — прод-форма тёплого resume: ОДИН активный сырой сегмент",
            v.len()
        ));
    }
    v[0].clone()
}

/// Байты, прочитанные `read()` ЭТИМ потоком.
fn rchar_thread() -> u64 {
    let t = std::fs::read_to_string("/proc/thread-self/io").unwrap_or_else(|e| {
        setup_failed(&format!(
            "/proc/thread-self/io недоступен ({e}) — меры границы нет"
        ))
    });
    t.lines()
        .find_map(|l| l.strip_prefix("rchar:"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| setup_failed("в /proc/thread-self/io нет rchar"))
}

struct Read {
    events: Vec<Event>,
    err: Option<io::Error>,
    rchar: u64,
    seek_fallbacks: u64,
}

/// ПРЕДМЕТ: публичная точка тёплого сдвига (`hint = None`, `after = Some`), стрим до конца.
fn subject(dir: &Path, after: u64) -> Read {
    let before = rchar_thread();
    let mut s = journal::stream_from_at(dir, EpochFilter::OwnCaptureOnly, Some(after), None)
        .unwrap_or_else(|e| setup_failed(&format!("stream_from_at: {e}")));
    let mut events = Vec::new();
    let mut err = None;
    for item in s.by_ref() {
        match item {
            Ok(e) => events.push(e),
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    let rchar = rchar_thread().saturating_sub(before);
    Read {
        events,
        err,
        rchar,
        seek_fallbacks: s.seek_fallbacks(),
    }
}

fn assert_exact_tail(tag: &str, r: &Read, after: u64, tail: u64) {
    if let Some(e) = &r.err {
        panic!("{tag}: стрим вернул ошибку на целом журнале: {e}");
    }
    let got: Vec<u64> = r.events.iter().map(|e| e.seq).collect();
    let want: Vec<u64> = (after + 1..=after + tail).collect();
    assert!(
        got == want,
        "{tag} / JR-I-2: выдано {} событий [{:?}…{:?}], ожидалось РОВНО {}..={} — потеря или дубль",
        got.len(),
        got.first(),
        got.last(),
        after + 1,
        after + tail
    );
    assert_eq!(
        r.seek_fallbacks, 0,
        "{tag}: откат к header_end (порча) на целом журнале — поиск позиции сломан"
    );
}

/// Ошибка оценки позиции, которую увидит реализация: `+` — оценка ПЕРЕЛЕТАЕТ цель (лежит дальше
/// от начала, чем `after + 1`), `−` — не долетает. Арифметика — ТОЧНО та же, что в
/// `seek_back_from_tail` (`crates/journal/src/segments.rs`, `C-294` B-1): `header_end` из заголовка
/// сегмента, `avg = floor((file_len − header_end) / (last − first + 1))`, `approx_offset =
/// min(events_back × avg, file_len − header_end)`, `approx_pos = max(file_len − approx_offset,
/// header_end)`. Цель — начало кадра `after + 1` = `file_len − байты хвоста` (хвост дописан
/// последним, `fixture`).
fn estimate_error(dir: &Path, tail_bytes: u64, tail: u64) -> i64 {
    let seg = only_segment(dir);
    let data = std::fs::read(&seg).expect("read segment");
    let file_len = data.len() as u64;
    let he = common::header_end(&data) as u64;
    if he == 0 {
        setup_failed("сегмент без заголовка v2 — арифметика оценки не воспроизводима");
    }
    let total_events = PREFIX + tail; // first = 0, last = PREFIX + tail − 1
    let avg = (file_len - he) / total_events;
    let approx_offset = (tail * avg).min(file_len - he);
    let approx_pos = (file_len - approx_offset).max(he);
    let target = file_len - tail_bytes;
    approx_pos as i64 - target as i64
}

/// Setup-страж мира `t5`/`t6`: модуль ошибки — в `ERR_BAND`, знак — заданный.
fn assert_error_band(tag: &str, err: i64, overshoot: bool) {
    let ok_sign = if overshoot { err > 0 } else { err < 0 };
    let m = err.abs();
    if !(ok_sign && m >= ERR_BAND.0 && m <= ERR_BAND.1) {
        setup_failed(&format!(
            "{tag}: ошибка оценки {err} Б вне полосы ±[{}, {}] (знак: {}) — мир не различает окно              < 8 КиБ и ≥ 8 КиБ",
            ERR_BAND.0,
            ERR_BAND.1,
            if overshoot { "перелёт" } else { "недолёт" }
        ));
    }
}

fn tail_symbol(delta: isize) -> String {
    let n = (BASE_SYMBOL.len() as isize + delta) as usize;
    let s: String = "BTCUSDTXYZWQRSTUV".chars().take(n).collect();
    if s.len() != n {
        setup_failed("символ перекоса не собран");
    }
    s
}

/// Setup-страж перекоса: средняя длина кадра хвоста отличается от префикса на `SKEW` в нужную
/// сторону — иначе мир меряет не тот сценарий.
fn assert_skew(
    tag: &str,
    tail_bytes: u64,
    tail: u64,
    prefix_dir: &Path,
    want_longer: Option<bool>,
) {
    let seg = only_segment(prefix_dir);
    let file_len = std::fs::metadata(&seg).expect("meta").len();
    let prefix_bytes = file_len - tail_bytes;
    let prefix_avg = prefix_bytes as f64 / PREFIX as f64;
    let tail_avg = tail_bytes as f64 / tail as f64;
    let d = tail_avg - prefix_avg;
    let ok = match want_longer {
        Some(true) => d >= SKEW as f64 - 2.5,
        Some(false) => d <= -(SKEW as f64) + 2.5,
        None => d.abs() <= 2.5,
    };
    if !ok {
        setup_failed(&format!(
            "{tag}: средний кадр хвоста {tail_avg:.2} Б против префикса {prefix_avg:.2} Б (Δ {d:.2}) — \
             перекос не тот, что задан"
        ));
    }
}

/// `t1` — хвост длиннее: оценка «от конца» ПЕРЕЛЕТАЕТ цель на ≈ `TAIL × SKEW` байт.
#[test]
fn t1_tail_frames_longer_overshoot_is_found_by_fast_path() {
    let (dir, after, tail_bytes) = fixture(PREFIX, TAIL, &tail_symbol(SKEW as isize));
    assert_skew("t1", tail_bytes, TAIL, dir.path(), Some(true));
    let r = subject(dir.path(), after);
    assert_exact_tail("t1", &r, after, TAIL);
    assert!(
        r.rchar <= tail_bytes + FAST_PATH_OVERHEAD,
        "M-96 / TD-250 / C-290 Б-2: хвост длиннее среднего на {SKEW} Б/кадр (оценка перелетает цель \
         на ≈{} Б), и первый сдвиг прочитал {} Б при хвосте {tail_bytes} Б — сверх быстрого пути \
         ({FAST_PATH_OVERHEAD} Б). Окно поиска только ВПЕРЁД от оценки: перелёт уходит в бисекцию. \
         Обязан: окно в обе стороны от оценки",
        TAIL as usize * SKEW,
        r.rchar
    );
}

/// `t2` — хвост короче: оценка НЕ ДОЛЕТАЕТ (страж: исправление не ломает вторую сторону).
#[test]
fn t2_tail_frames_shorter_undershoot_is_found_by_fast_path() {
    let (dir, after, tail_bytes) = fixture(PREFIX, TAIL, &tail_symbol(-(SKEW as isize)));
    assert_skew("t2", tail_bytes, TAIL, dir.path(), Some(false));
    let r = subject(dir.path(), after);
    assert_exact_tail("t2", &r, after, TAIL);
    assert!(
        r.rchar <= tail_bytes + FAST_PATH_OVERHEAD,
        "M-96: хвост короче среднего на {SKEW} Б/кадр — первый сдвиг прочитал {} Б при хвосте \
         {tail_bytes} Б, сверх быстрого пути ({FAST_PATH_OVERHEAD} Б)",
        r.rchar
    );
}

/// `t3` — равномерные кадры: базовый случай.
#[test]
fn t3_uniform_frames_fast_path() {
    let (dir, after, tail_bytes) = fixture(PREFIX, TAIL, BASE_SYMBOL);
    assert_skew("t3", tail_bytes, TAIL, dir.path(), None);
    let r = subject(dir.path(), after);
    assert_exact_tail("t3", &r, after, TAIL);
    assert!(
        r.rchar <= tail_bytes + FAST_PATH_OVERHEAD,
        "M-96: равномерные кадры — первый сдвиг прочитал {} Б при хвосте {tail_bytes} Б, сверх \
         быстрого пути ({FAST_PATH_OVERHEAD} Б)",
        r.rchar
    );
}

/// `t4` — перелёт больше любого разумного окна: быстрый путь честно отказывает, бисекция находит
/// `after + 1`; стоимость — бисекции, не файла (страж: «окно = весь файл» не исправление).
#[test]
fn t4_overshoot_beyond_window_falls_back_to_bisection_not_full_scan() {
    let (dir, after, tail_bytes) = fixture(PREFIX, TAIL_FAR, &tail_symbol(SKEW as isize));
    assert_skew("t4", tail_bytes, TAIL_FAR, dir.path(), Some(true));
    let r = subject(dir.path(), after);
    assert_exact_tail("t4", &r, after, TAIL_FAR);
    assert!(
        r.rchar <= tail_bytes + BISECT_OVERHEAD,
        "M-96: перелёт ≈{} Б — первый сдвиг прочитал {} Б при хвосте {tail_bytes} Б, сверх отката \
         на бисекцию ({BISECT_OVERHEAD} Б): поиск читает префикс, а не пробует log-число окон",
        TAIL_FAR as usize * SKEW,
        r.rchar
    );
}

/// `t5` — перелёт ≈ 5…7 КиБ (`C-291` B-1): окно, начатое меньше чем за 8 КиБ ДО оценки, цель не
/// накрывает ⇒ бисекция ⇒ RED по стоимости. Требование §3: не меньше 8 КиБ назад.
#[test]
fn t5_overshoot_between_4_and_8_kib_needs_8_kib_back() {
    let (dir, after, tail_bytes) = fixture(PREFIX, TAIL_MID, &tail_symbol(SKEW_OVER as isize));
    let err = estimate_error(dir.path(), tail_bytes, TAIL_MID);
    assert_error_band("t5", err, true);
    let r = subject(dir.path(), after);
    assert_exact_tail("t5", &r, after, TAIL_MID);
    assert!(
        r.rchar <= tail_bytes + FAST_PATH_OVERHEAD,
        "M-96 / C-291 B-1: оценка перелетает цель на {err} Б (между 4 и 8 КиБ) — первый сдвиг \
         прочитал {} Б при хвосте {tail_bytes} Б, сверх быстрого пути ({FAST_PATH_OVERHEAD} Б). \
         Окно назад от оценки уже 8 КиБ (§3)",
        r.rchar
    );
}

/// `t6` — недолёт ≈ 5…7 КиБ: окно, кончающееся меньше чем через 8 КиБ ПОСЛЕ оценки, цель не
/// накрывает. Требование §3: не меньше 8 КиБ вперёд.
#[test]
fn t6_undershoot_between_4_and_8_kib_needs_8_kib_forward() {
    let (dir, after, tail_bytes) = fixture(PREFIX, TAIL_MID, &tail_symbol(-(SKEW_UNDER as isize)));
    let err = estimate_error(dir.path(), tail_bytes, TAIL_MID);
    assert_error_band("t6", err, false);
    let r = subject(dir.path(), after);
    assert_exact_tail("t6", &r, after, TAIL_MID);
    assert!(
        r.rchar <= tail_bytes + FAST_PATH_OVERHEAD,
        "M-96 / C-291 B-1: оценка не долетает до цели на {} Б (между 4 и 8 КиБ) — первый сдвиг \
         прочитал {} Б при хвосте {tail_bytes} Б, сверх быстрого пути ({FAST_PATH_OVERHEAD} Б). \
         Окно вперёд от оценки уже 8 КиБ (§3)",
        -err,
        r.rchar
    );
}
