//! RED OPS-I-1 ОКОННАЯ ПЕРСИСТЕНТНОСТЬ (sacred, architect-only) — near-book ОБЪЁМ churn'ит, порча
//! персистентна. Второй §8-провал (2026-07-17): reviewer прогнал live-recon (2×210с живой Binance,
//! оба feeder'а) — depth-asymmetry (виток 1) и best-price ложняки закрыты, но recon ВСЁ РАВНО флудил
//! `ReconDivergence` ~1/цикл/символ (`band_divergence` 16–853 bps > ε_test=50) на ЗДОРОВОМ рынке.
//!
//! КОРЕНЬ (architect измерил, `api.binance.com/api/v3/depth?limit=5000`, BTC/ETH 2026-07-17):
//! два async REST-снапшота ~2.5с врозь расходятся по near-touch суммам полос на сотни bps, и ЗНАК
//! per-cycle ГУЛЯЕТ (BTC BID 0.3% `+--`, ASK 0.3% `+-+`, ETH ASK 0.1% `-0+`; BTC BID 0.1% дал ДАЖЕ
//! 3 подряд одного знака `---`). Это чистый timing-skew биржи (local — WS-книга момента T1,
//! reference — async REST момента T2), НЕ ошибка local и НЕ TD-016 фантом. Per-cycle порог по
//! ОБЪЁМУ ПРИНЦИПИАЛЬНО нежизнеспособен: любой порог тишины прячет порчу, любой ловящий порчу флудит.
//!
//! ДИЗАЙН (`ops.md` §4.3): дискриминатор churn↔порча — НЕ магнитуда, а ПЕРСИСТЕНТНОСТЬ ЗНАКА. Recon
//! становится STATEFUL (`ReconDetector`): окно `RECON_WINDOW` циклов на (полосу,сторону); знаковое
//! среднее. churn (знак гуляет) → mean→0 → ТИШИНА; порча (C1-стрип / TD-016 near-touch фантом держат
//! знак) → |mean| над порогом → АЛЕРТ. Best-price — ПО-ПРЕЖНЕМУ per-cycle (immediate).
//!
//! Гейт-целостность (`.claude/rules/testing.md`, 4 свойства + «RED двух источников → live-режим»):
//!  • ПРОД-ФОРМА: последовательность циклов (не один такт), два РАЗНЫХ момента (timing-skew объёма);
//!  • СВОЙ инвариант: знаковое среднее окна, не per-cycle магнитуда;
//!  • анти-плацебо В ОБЕ стороны — churn (та же per-cycle магнитуда, что у порчи) обязан молчать
//!    (валит per-cycle-magnitude impl И «K подряд одного знака» impl); порча обязана алертить
//!    (валит always-silent impl);
//!  • ОТСУТСТВИЕ: churn с 3-подряд-одного-знака (реальный замер) не смеет тихо провоцировать алерт.

use book::OrderBook;
use contracts::{Level, Side};
use ops::recon::{ReconDetector, ReconThresholds, EPS_MAX_BPS, EPS_PROD_DEFAULT_BPS, RECON_WINDOW};

const MID: i64 = 65_000_000_000_000; // $65k ×1e8
const UNIT: i64 = 100_000_000; // 1.0 объёма ×1e8
const BASE: f64 = 100.0; // базовый объём уровня (units) — масштабируется для дефицита/профицита

/// Уровни на 0.05..0.55% от mid → reach≈0.55%, покрывает полосы recon 0.1/0.3/0.5%.
const PCTS: [f64; 6] = [0.0005, 0.0015, 0.0025, 0.0035, 0.0045, 0.0055];

/// Книга, где объём КАЖДОГО уровня стороны масштабирован: bid×`bid_scale`, ask×`ask_scale`.
/// ЦЕНЫ уровней НЕ меняются (best-price идентичен reference → best_price_diverged=false, изолируем
/// ОБЪЁМ). `scale=1.0` — reference-эталон; `1.15` — профицит +1500 bps; `0.85` — дефицит −1500 bps.
fn scaled_book(bid_scale: f64, ask_scale: f64) -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = PCTS
        .iter()
        .map(|&p| Level {
            price: (MID as f64 * (1.0 - p)) as i64,
            size: (BASE * bid_scale).round() as i64 * UNIT,
        })
        .collect();
    let asks: Vec<Level> = PCTS
        .iter()
        .map(|&p| Level {
            price: (MID as f64 * (1.0 + p)) as i64,
            size: (BASE * ask_scale).round() as i64 * UNIT,
        })
        .collect();
    b.apply_snapshot(&bids, &asks);
    b
}

fn reference() -> OrderBook {
    scaled_book(1.0, 1.0)
}

fn detector() -> ReconDetector {
    ReconDetector::new(ReconThresholds::new(EPS_PROD_DEFAULT_BPS).expect("thr"))
}

/// Прогнать последовательность `locals` против фиксированного reference; вернуть, эмитил ли алерт
/// ХОТЬ ОДИН цикл, и best-diverged ли где-либо (страховка изоляции объёма).
fn run_sequence(det: &mut ReconDetector, locals: &[OrderBook]) -> (bool, bool) {
    let reference = reference();
    let mut any_alert = false;
    let mut any_best = false;
    for local in locals {
        let v = det.observe(local, &reference);
        any_alert |= v.alert;
        any_best |= v.best_price_diverged;
    }
    (any_alert, any_best)
}

// ─────────────────────────────────────────────────────────────────────────────
// (1) CHURN → ТИШИНА. Знак per-cycle гуляет (чередование) → mean→0. Та же per-cycle МАГНИТУДА
//     (±1500 bps), что у порчи ниже — различает ТОЛЬКО персистентность. Валит per-cycle-порог impl.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn volume_timing_skew_does_not_alert() {
    let mut det = detector();
    // BID-объём чередуется 1.15/0.85 (профицит/дефицит) — чистый timing-skew, знак гуляет.
    // ASK держит эталон (изолируем один канал churn). Best-цена не двигается (масштаб — по объёму).
    let locals: Vec<OrderBook> = (0..RECON_WINDOW)
        .map(|i| {
            let bid_scale = if i % 2 == 0 { 1.15 } else { 0.85 };
            scaled_book(bid_scale, 1.0)
        })
        .collect();
    let (any_alert, any_best) = run_sequence(&mut det, &locals);
    assert!(
        !any_best,
        "фикстура сломана: best-цена разошлась — тест должен изолировать ОБЪЁМ (best_scale не трогали)"
    );
    assert!(
        !any_alert,
        "near-touch объём churn'ил (знак чередуется, mean→0), а recon поднял ReconDivergence — это \
         ровно §8-флуд на здоровом рынке, который поймал reviewer (band_divergence 16–853 bps на \
         timing-skew). Per-cycle порог по объёму нежизнеспособен — окно обязано усреднить churn в 0"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (2) CHURN С 3-ПОДРЯД-ОДНОГО-ЗНАКА → ТИШИНА (анти-плацебо против «K подряд → алерт»). Реальный
//     замер BTC BID 0.1% дал `---` (3 подряд «−»); окно всё равно сбалансировано → mean→0.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[allow(clippy::assertions_on_constants, clippy::manual_is_multiple_of)]
fn churn_with_same_sign_run_stays_silent() {
    // Фикстура-предусловие (документирует форму; guard на случай смены RECON_WINDOW на нечётное).
    assert!(
        RECON_WINDOW >= 6 && RECON_WINDOW % 2 == 0,
        "фикстура рассчитана на чётное окно ≥6 (баланс +/− с 3-раном); RECON_WINDOW={RECON_WINDOW}"
    );
    let mut det = detector();
    // Сбалансированная последовательность знаков с блоком 3-подряд «+» и 3-подряд «−» в начале.
    // Ровно RECON_WINDOW/2 профицитов и RECON_WINDOW/2 дефицитов → знаковое среднее = 0.
    let half = RECON_WINDOW / 2;
    let mut plus_left = half;
    let mut minus_left = half;
    let mut scales: Vec<f64> = Vec::with_capacity(RECON_WINDOW);
    // первые 3 «+», затем 3 «−» (реальный churn-ран), остаток — чередование до баланса.
    for i in 0..RECON_WINDOW {
        let want_plus = if i < 3 {
            true
        } else if i < 6 {
            false
        } else {
            i % 2 == 0
        };
        let plus = if want_plus && plus_left > 0 {
            plus_left -= 1;
            true
        } else if minus_left > 0 {
            minus_left -= 1;
            false
        } else {
            plus_left -= 1;
            true
        };
        scales.push(if plus { 1.15 } else { 0.85 });
    }
    let locals: Vec<OrderBook> = scales.iter().map(|&s| scaled_book(s, 1.0)).collect();
    let (any_alert, _) = run_sequence(&mut det, &locals);
    assert!(
        !any_alert,
        "churn с 3-подряд-одного-знака (реальный замер BTC BID 0.1% `---`) поднял алерт — детектор \
         среагировал на КОРОТКИЙ ран знака, а не на устойчивый сдвиг окна (наивный «K подряд → алерт» \
         вместо знакового среднего). Окно сбалансировано (mean=0) — обязана быть тишина"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (3) ПЕРСИСТЕНТНЫЙ ДЕФИЦИТ (C1-класс) → АЛЕРТ. local ОДНОСТОРОННЕ ниже reference каждый цикл.
//     Та же per-cycle магнитуда, что у churn (−1500 bps), но знак ДЕРЖИТСЯ → mean=−1500 → алерт.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn persistent_volume_deficit_alerts() {
    let mut det = detector();
    // local держит bid-объём на 15% НИЖЕ reference КАЖДЫЙ цикл (near-touch ликвидность «испарилась» —
    // класс C1: наша книга систематически недосчитывает near-book объём).
    let locals: Vec<OrderBook> = (0..RECON_WINDOW).map(|_| scaled_book(0.85, 1.0)).collect();
    let (any_alert, any_best) = run_sequence(&mut det, &locals);
    assert!(
        !any_best,
        "фикстура: best не должна расходиться (изолируем персистентный ОБЪЁМНЫЙ дефицит)"
    );
    assert!(
        any_alert,
        "local держал −15% near-book объёма ВСЕ {RECON_WINDOW} циклов (персистентный дефицит, C1-класс), \
         а recon смолчал — знаковое среднее окна = −1500 bps обязано пробить порог. Детектор не \
         отличает персистентную порчу от churn (та же магнитуда, но знак держится)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (4) ПЕРСИСТЕНТНЫЙ ПРОФИЦИТ (TD-016 near-touch фантом) → АЛЕРТ. local систематически ВЫШЕ reference
//     (уровень, из-под которого цена ушла, не обнулён → фантомный объём держится).
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn persistent_volume_surplus_alerts() {
    let mut det = detector();
    let locals: Vec<OrderBook> = (0..RECON_WINDOW).map(|_| scaled_book(1.15, 1.0)).collect();
    let (any_alert, _) = run_sequence(&mut det, &locals);
    assert!(
        any_alert,
        "local держал +15% фантомного near-book объёма ВСЕ {RECON_WINDOW} циклов (TD-016 near-touch \
         фантом — систематический local>ref), а recon смолчал — персистентный профицит обязан алертить"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (5) КОНКРЕТНАЯ ФОРМА C1: within-reach уровень (НЕ best) УДАЛЁН персистентно → дефицит полос
//     0.3%/0.5% держится → алерт. best (0.05%) цел → best_price_diverged=false (изоляция окна).
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn near_book_eviction_persists_then_alerts() {
    let mut det = detector();
    let reference = reference();
    // local: near-book БЕЗ уровня 0.25% (эвикция C1 стёрла within-reach уровень; best 0.05% цел).
    let evicted: Vec<f64> = PCTS.iter().copied().filter(|&p| p != 0.0025).collect();
    let mut any_alert = false;
    let mut any_best = false;
    for _ in 0..RECON_WINDOW {
        let mut local = OrderBook::new();
        let bids: Vec<Level> = evicted
            .iter()
            .map(|&p| Level {
                price: (MID as f64 * (1.0 - p)) as i64,
                size: BASE as i64 * UNIT,
            })
            .collect();
        let asks: Vec<Level> = PCTS
            .iter()
            .map(|&p| Level {
                price: (MID as f64 * (1.0 + p)) as i64,
                size: BASE as i64 * UNIT,
            })
            .collect();
        local.apply_snapshot(&bids, &asks);
        let v = det.observe(&local, &reference);
        any_alert |= v.alert;
        any_best |= v.best_price_diverged;
    }
    assert!(
        !any_best,
        "фикстура: best bid (0.05%) цел — эвикция стёрла within-reach уровень 0.25%, НЕ best (изоляция \
         оконного объёмного пути от best-пути)"
    );
    assert!(
        any_alert,
        "within-reach уровень 0.25% удалён ПЕРСИСТЕНТНО (C1-эвикция), а recon смолчал — устойчивый \
         дефицит полос 0.3%/0.5% обязан пробить окно"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (6) ε_test (оконный) НЕ КАЛИБРУЕТСЯ: персистентная порча с |mean| ≥ ε_test алертит даже при
//     ε_prod = ε_max (самый мягкий допустимый рабочий порог). fail-closed.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn windowed_eps_test_not_calibratable() {
    // ε_prod задан на ПОТОЛКЕ (ε_max) — максимально «оглушённый» допустимый детектор.
    let lax = ReconThresholds::new(EPS_MAX_BPS).expect("ε_prod == ε_max допустим");
    let mut det = ReconDetector::new(lax);
    let locals: Vec<OrderBook> = (0..RECON_WINDOW).map(|_| scaled_book(0.85, 1.0)).collect();
    let (any_alert, _) = run_sequence(&mut det, &locals);
    assert!(
        any_alert,
        "персистентная порча (−1500 bps окно) НЕ пробила детектор при ε_prod=ε_max — ε_test обязан \
         быть фиксированным гейтом (нельзя откалибровать окно до бесконечности, fail-closed)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (7) ДЕТЕРМИНИЗМ ЭМИССИЙ: одинаковая последовательность наблюдений → одинаковая последовательность
//     вердиктов (окно — рантайм-состояние, но чистое: нет wall-clock/rand). DET-принцип для recon.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn detector_is_deterministic_across_replay() {
    let reference = reference();
    // Смешанная последовательность: churn-хвост + персистентный дефицит (чтобы были и тишина, и алерт).
    let scales: Vec<f64> = (0..RECON_WINDOW * 2)
        .map(|i| {
            if i < RECON_WINDOW {
                if i % 2 == 0 {
                    1.15
                } else {
                    0.85
                }
            } else {
                0.85 // персистентный дефицит во второй половине
            }
        })
        .collect();
    let locals: Vec<OrderBook> = scales.iter().map(|&s| scaled_book(s, 1.0)).collect();

    let verdicts = |()| -> Vec<bool> {
        let mut det = detector();
        locals
            .iter()
            .map(|l| det.observe(l, &reference).alert)
            .collect()
    };
    let run1 = verdicts(());
    let run2 = verdicts(());
    assert_eq!(
        run1, run2,
        "два прогона одной последовательности дали РАЗНЫЕ эмиссии — детектор недетерминирован \
         (окно обязано быть чистым рантайм-состоянием, без wall-clock/rand; DET-принцип recon)"
    );
    assert!(
        run1.iter().any(|&a| a),
        "фикстура-сетап не состоялся: персистентный хвост обязан был поднять хотя бы один алерт \
         (иначе детерминизм проверяется вхолостую на вечной тишине)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (8) DEPTH-SKIP В ОКОННОМ ПУТИ (C-011 Concern-1, Mutation B reviewer'а). Пиннит фикс ПЕРВОГО
//     §8-флуда (асимметрия глубины §4.2): полоса, которую reference НЕ достаёт, ПРОПУСКАЕТСЯ в
//     observe(). Прочие оракулы сюиты используют reference, достающий ВСЕ полосы (0.55%), поэтому
//     skip-`continue` там НИКОГДА не срабатывает — depth-skip был не запиннен НИ ОДНИМ RED-оракулом
//     (Mutation B: удаление skip → 21/21 всё равно GREEN). Здесь reference ТОНКИЙ (reach ~0.15%),
//     local ПЕРСИСТЕНТНО держит односторонний объём на НЕДОСТИЖИМЫХ полосах 0.3%/0.5% каждый цикл.
// ─────────────────────────────────────────────────────────────────────────────

/// Reference достаёт только 0.05%/0.15% (reach≈0.15%) — полосы 0.3%/0.5% ВНЕ его reach.
fn truncated_reference() -> OrderBook {
    let mut b = OrderBook::new();
    let shallow = &PCTS[0..2]; // 0.05%, 0.15% — как REST, обрезанный на тонком/волатильном рынке
    let bids: Vec<Level> = shallow
        .iter()
        .map(|&p| Level {
            price: (MID as f64 * (1.0 - p)) as i64,
            size: BASE as i64 * UNIT,
        })
        .collect();
    let asks: Vec<Level> = shallow
        .iter()
        .map(|&p| Level {
            price: (MID as f64 * (1.0 + p)) as i64,
            size: BASE as i64 * UNIT,
        })
        .collect();
    b.apply_snapshot(&bids, &asks);
    b
}

/// (8, GREEN со skip; ПАДАЕТ без skip = Mutation B) reference тонкий (≤0.15%), local — полная книга
/// (0.55%), near-book 0.1% ИДЕНТИЧЕН, а на НЕДОСТИЖИМЫХ полосах 0.3%/0.5% local персистентно держит
/// односторонний объём ВСЕ RECON_WINDOW циклов. Детектор ОБЯЗАН молчать: невалидируемая полоса
/// (reference.max_reach_pct(side) < band) ПРОПУСКАЕТСЯ, невалидируемое ≠ расхождение (§4.2).
/// Анти-плацебо: убери skip в observe() → local(300)≫reference(200) на полосе 0.3% каждый цикл →
/// окно держит персистентный +знак → ЛОЖНЫЙ алерт → тест ПАДАЕТ (это §8-флуд #1, вернувшийся на
/// тонком рынке при зелёных гейтах — ровно то, что поймал reviewer Mutation B).
#[test]
fn unreachable_band_is_skipped_not_flooded() {
    let mut det = detector();
    let reference = truncated_reference();
    // local — ПОЛНАЯ книга (reach 0.55%): near-book 0.05%/0.15% совпадает с reference (size BASE),
    // а глубже (0.25%..0.55%) несёт объём, которого reference не видит. НЕ меняется по циклам
    // (персистентно) — если бы это считалось расхождением, окно держало бы знак и флудило.
    let local = scaled_book(1.0, 1.0);

    // Страховка сетапа (testing.md: гейт обязан краснеть и при несостоявшемся setup): reference
    // ДЕЙСТВИТЕЛЬНО не достаёт 0.3%/0.5%, а local — достаёт (иначе тест проверяет skip вхолостую).
    for side in [Side::Buy, Side::Sell] {
        let rr = reference.max_reach_pct(side).expect("reference не пуст");
        assert!(
            rr < 0.003,
            "фикстура-сетап: reference обязан НЕ достигать полосу 0.3% (reach={rr}) — иначе skip не тестируется"
        );
        let lr = local.max_reach_pct(side).expect("local не пуст");
        assert!(
            lr >= 0.005,
            "фикстура-сетап: local обязан достигать полосу 0.5% (reach={lr}) — иначе нечему флудить без skip"
        );
    }

    let (any_alert, any_best) = run_sequence_ref(&mut det, &local, &reference);
    assert!(
        !any_best,
        "фикстура: best (0.05%) идентичен → best_price_diverged=false. Тест изолирует depth-skip \
         объёмного пути от best-пути"
    );
    assert!(
        !any_alert,
        "local держал объём на НЕДОСТИЖИМЫХ reference'ом полосах 0.3%/0.5% ПЕРСИСТЕНТНО, а детектор \
         поднял алерт — depth-skip в observe() снят/сломан. Полоса за reference.max_reach_pct обязана \
         ПРОПУСКАТЬСЯ (§4.2), иначе §8-флуд #1 (асимметрия глубины) вернётся на тонком/волатильном \
         рынке при зелёных гейтах (Mutation B reviewer'а, C-011 Concern-1)"
    );
}

/// Как `run_sequence`, но с ЯВНЫМ reference (тот же local каждый цикл — персистентно).
fn run_sequence_ref(
    det: &mut ReconDetector,
    local: &OrderBook,
    reference: &OrderBook,
) -> (bool, bool) {
    let mut any_alert = false;
    let mut any_best = false;
    for _ in 0..RECON_WINDOW {
        let v = det.observe(local, reference);
        any_alert |= v.alert;
        any_best |= v.best_price_diverged;
    }
    (any_alert, any_best)
}
