//! `DV-I-15` — граница ПАМЯТИ per-life анализатора (M-59, долг `TD-107`).
//!
//! ЗАЧЕМ. `SideBook.finished: Vec<(Option<i64>, Fate)>` растёт на КАЖДУЮ завершённую жизнь и
//! никогда не сбрасывается: 650 372 записи за 3.4 часа ОДНОГО символа (`R-033` F-5). Сегодня
//! это офлайн-инструмент на машине разработчика, поэтому долг MINOR по последствиям — но по
//! КЛАССУ это `TD-011`: расход, линейный по объёму входа, обнаруживается не тогда, когда
//! появляется, а когда вход вырастает (несколько символов, многосуточное окно).
//!
//! ПОЧЕМУ СУЩЕСТВУЮЩИЕ ОРАКУЛЫ ЭТОГО НЕ ЛОВЯТ — и почему нужен отдельный тест, а не ассерт
//! в соседнем. `DV-I-7` меряет ВРЕМЯ (бюджет 15 с), а его фикстура рождает уровни, которые
//! никогда не отменяются ⇒ `finished` в ней ПУСТ. То есть дефект не наблюдаем не потому, что
//! оракул слаб, а потому, что он меряет другую величину на входе, где явления нет. Ровно
//! класс «ресурс меряется прокси» (`TD-011`/`TD-021`, `testing.md`).
//!
//! РОВНО ОДИН ТЕСТ В ФАЙЛЕ. Счётчик аллокаций глобален для процесса, а `cargo` гонит тесты
//! одного бинаря параллельными потоками: второй тест в этом же файле сделал бы замер
//! недействительным без `Mutex`. Отдельный тест-бинарь даёт изоляцию замера по построению
//! (урок `TD-040`; образец — `crates/journal/tests/red_open_bounded.rs`).
//!
//! ЧТО ЭТОТ ОРАКУЛ ПАДАЕТ ПРОТИВ (анти-плацебо, обязано предъявляться прогоном):
//!   - против ТЕКУЩЕЙ реализации: `finished` растёт вчетверо вместе с числом жизней ⇒ пик
//!     памяти растёт пропорционально ⇒ ассерт (2) КРАСНЫЙ **до** фикса;
//!   - против реализации, которая «уложилась в бюджет, перестав считать»: ассерт (1) сверяет
//!     `lives_born`/`lives_cancelled` с числами фикстуры, поэтому потеря данных не проходит.
//!
//! Обе стороны нужны: без (1) бюджет проходит пустой редьюсер, без (2) — накопитель.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{Level, Side};
use research_cli::depth_lifetime::{analyze, DeltaTick};

static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), SeqCst) + l.size();
            PEAK.fetch_max(c, SeqCst);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        CUR.fetch_sub(l.size(), SeqCst);
    }
}
#[global_allocator]
static GA: Counting = Counting;

/// Пиковая аллокация (дельта над базой) во время `f`.
fn peak_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = CUR.load(SeqCst);
    PEAK.store(base, SeqCst);
    let r = f();
    (r, PEAK.load(SeqCst).saturating_sub(base))
}

const UNIT: i64 = 100_000_000;
const MID: i64 = 64_000 * UNIT;
const FAR_PCT: f64 = 0.06;
const FAR_LO_BPS: i64 = 500;

/// Число циклов «рождение → отмена» в малой конфигурации. Большая — ×4.
const N: u64 = 20_000;

fn lvl_at(mid_ref: i64, pct: f64, side: Side, size_units: i64) -> Level {
    let price = match side {
        Side::Buy => mid_ref as f64 * (1.0 - pct),
        Side::Sell => mid_ref as f64 * (1.0 + pct),
    };
    Level {
        price: price as i64,
        size: size_units * UNIT,
    }
}

fn lvl(pct: f64, side: Side, size_units: i64) -> Level {
    lvl_at(MID, pct, side, size_units)
}

fn tick(u: u64, bids: Vec<Level>, asks: Vec<Level>) -> DeltaTick {
    DeltaTick {
        bids,
        asks,
        first_update_id: u,
        final_update_id: u,
        prev_final_update_id: None,
        ts_exch_ms: 1_700_000_000_000,
    }
}

/// Окно из `cycles` ЗАВЕРШЁННЫХ жизней НА ОДНОЙ И ТОЙ ЖЕ цене плюс одна незавершённая.
///
/// Ключевое свойство фикстуры: множество РАЗЛИЧНЫХ цен не зависит от `cycles` (якоря лучших
/// цен + одна дальняя). Значит `book`/`states`/`alive` — величины, растущие по ЦЕНАМ, — в
/// обеих конфигурациях одинаковы, и единственное, что меняется, — число ЖИЗНЕЙ. Без этого
/// рост `finished` спутался бы с ростом `states`, и оракул мерил бы не то, что обещает
/// (`testing.md`, «целостность гейта», свойство 2: конфаундер держится КОНСТАНТНЫМ).
fn cycles_window(cycles: u64) -> Vec<DeltaTick> {
    let (sb, sa) = (
        vec![lvl_at(MID, 0.001, Side::Buy, 10)],
        vec![lvl_at(MID, 0.001, Side::Sell, 10)],
    );
    let mut ticks = Vec::with_capacity(2 * cycles as usize + 3);
    ticks.push(tick(1, sb, sa));
    let mut u = 2u64;
    for _ in 0..cycles {
        ticks.push(tick(u, vec![lvl(FAR_PCT, Side::Buy, 5)], vec![]));
        u += 1;
        ticks.push(tick(u, vec![lvl(FAR_PCT, Side::Buy, 0)], vec![]));
        u += 1;
    }
    // последняя жизнь рождается и остаётся живой — чтобы окно не вырождалось в «всё закрыто»
    ticks.push(tick(u, vec![lvl(FAR_PCT, Side::Buy, 9)], vec![]));
    ticks
}

/// Сколько жизней ЗАВЕРШАЕТСЯ в окне (тик с `size = 0` на дальней цене).
/// Именно на завершённые жизни растёт `finished`, поэтому страж считает их, а не тики.
fn completed_lives(ticks: &[DeltaTick]) -> usize {
    ticks
        .iter()
        .filter(|t| t.bids.iter().any(|l| l.size == 0))
        .count()
}

/// Сколько РАЗЛИЧНЫХ цен встречается в окне (страж конфаундера).
fn distinct_prices(ticks: &[DeltaTick]) -> usize {
    let mut s = BTreeSet::new();
    for t in ticks {
        for l in t.bids.iter().chain(t.asks.iter()) {
            s.insert(l.price);
        }
    }
    s.len()
}

#[test]
fn dv_i_15_lifetime_memory_bounded() {
    // Фикстуры строятся ВНЕ измеряемой области намеренно. Само окно растёт линейно по
    // `cycles`; если бы оно аллоцировалось внутри `peak_delta`, его собственный рост попал
    // бы в замер, и оракул остался бы красным ДАЖЕ ПОСЛЕ фикса — то есть мерил бы фикстуру,
    // а не предмет.
    let ticks_n = cycles_window(N);
    let ticks_4n = cycles_window(4 * N);

    // ── СТРАЖ ФИКСТУРЫ: сценарий действительно тот, что заявлен ──────────────────────────
    // Проба, молча измеряющая не тот сценарий, — плацебо самой себя (`testing.md`).
    assert_eq!(
        distinct_prices(&ticks_n),
        distinct_prices(&ticks_4n),
        "конфаундер не константен: число РАЗЛИЧНЫХ цен различается между конфигурациями, \
         значит рост памяти нельзя приписать числу ЖИЗНЕЙ"
    );
    // Меряется ИМЕННО та величина, о которой утверждение: `finished` растёт на каждую
    // ЗАВЕРШЁННУЮ жизнь. Первая редакция этого стража сверяла производную (длину окна минус
    // магическая константа 3) и разошлась на 3 — страж поймал арифметику автора раньше, чем
    // она стала бы «зелёным» результатом.
    assert_eq!(
        completed_lives(&ticks_4n),
        4 * completed_lives(&ticks_n),
        "число ЗАВЕРШЁННЫХ жизней не выросло вчетверо — фикстура не соответствует замыслу"
    );

    let (rep_n, peak_n) = peak_delta(|| analyze(&ticks_n));
    let (rep_4n, peak_4n) = peak_delta(|| analyze(&ticks_4n));

    let bn = rep_n
        .band(Side::Buy, FAR_LO_BPS)
        .expect("полоса [500,800) bid обязана быть в отчёте (N)");
    let b4 = rep_4n
        .band(Side::Buy, FAR_LO_BPS)
        .expect("полоса [500,800) bid обязана быть в отчёте (4N)");

    eprintln!(
        "DV-I-15: N={N} жизней born={} cancelled={} peak={} B | 4N born={} cancelled={} peak={} B \
         | рост памяти ×{:.2} (бюджет <1.50)",
        bn.lives_born,
        bn.lives_cancelled,
        peak_n,
        b4.lives_born,
        b4.lives_cancelled,
        peak_4n,
        peak_4n as f64 / peak_n.max(1) as f64
    );

    // ── (1) КОРРЕКТНОСТЬ ────────────────────────────────────────────────────────────────
    // Без этого блока бюджет памяти проходит реализация, которая «ничего не накапливает»,
    // потому что перестала считать. Экономия памяти ценой потери данных — не фикс.
    assert_eq!(
        (bn.lives_born, bn.lives_cancelled),
        (N + 1, N),
        "N-конфигурация: ожидались {} рождений и {N} отмен",
        N + 1
    );
    assert_eq!(
        (b4.lives_born, b4.lives_cancelled),
        (4 * N + 1, 4 * N),
        "4N-конфигурация: ожидались {} рождений и {} отмен",
        4 * N + 1,
        4 * N
    );
    assert_eq!(
        b4.lives_born,
        b4.lives_cancelled + b4.lives_frozen + b4.lives_censored,
        "баланс жизней нарушен: born != cancelled + frozen + censored"
    );

    // ── (2) ПАМЯТЬ ──────────────────────────────────────────────────────────────────────
    // Сравнительное свойство, а не абсолютный порог: абсолютный зависел бы от ширины
    // фикстуры и от аллокатора, а отношение при КОНСТАНТНОМ числе цен зависит ровно от
    // того, накапливаются ли завершённые жизни (урок M-56 O-1).
    assert!(
        peak_4n * 2 < peak_n * 3,
        "DV-I-15: жизней вчетверо больше при ТОМ ЖЕ числе различных цен, а пик памяти вырос \
         с {peak_n} B до {peak_4n} B (×{:.2} при бюджете <1.50). Значит завершённые жизни \
         НАКАПЛИВАЮТСЯ, а не сворачиваются в агрегаты: расход линеен по объёму входа \
         (TD-107). На 650 тыс. жизней одного символа это десятки МБ, на нескольких символах \
         за сутки — гигабайты.",
        peak_4n as f64 / peak_n.max(1) as f64
    );
}
