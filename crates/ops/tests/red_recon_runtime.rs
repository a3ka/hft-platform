//! RED OPS-I-1 РАНТАЙМ-КОНТРАКТ ПОД B2 (sacred, architect-only) — рантайм-recon = **best-price
//! per-cycle + seed-gate**; near-touch ОБЪЁМ в рантайме МОЛЧИТ (REST-неверифицируем).
//!
//! B2 ПРИНЯТ founder ★ 2026-07-18 (`docs/fa/ops.md` §4.3.2). Три §8-провала подряд по одному классу
//! (near-touch объём): depth-asymmetry (§4.2) → per-cycle volume → windowed volume (§4.3). Корень
//! глубже калибровки: усреднение окна гасит ДИСПЕРСИЮ (zero-mean churn), но НЕ СИСТЕМАТИЧЕСКИЙ сдвиг.
//! §8 re-run reviewer'а (merge `e9fc258`) намерил 12× `best_diverged=false div_bps 103..747` на
//! ЗДОРОВОМ рынке, **в т.ч. на нетронутом инъекцией `BinanceFutures`** → систематический
//! WS(T1)-vs-REST(T2) объёмный bias, часть значений ≫ ε_max=50 (порогом fail-closed непобедимы).
//!
//! РЕШЕНИЕ B2: рантайм-эмиссию объёмной сверки УБРАТЬ. Рантайм-alert ⟺ `best_price_diverged`
//! (best-price REST-верифицируем: §8 healthy 0 эмиссий, injection 6× best=true). Объёмная сверка →
//! ОФЛАЙН-трек (research-dev) над записанной книгой, необязательный follow-up. Полная книга/объёмы
//! пишутся в журнал БЕЗ изменений.
//!
//! ЧТО ПИННИТ ЭТОТ ФАЙЛ (рантайм-контракт B2):
//!  • seed-gate (§4.3.1) — 9a/9b/9c: до первого своего непустого снапшота recon молчит и НЕ кормит
//!    состояние; пост-seed пустая local = РЕАЛЬНАЯ порча (best-путь) → эмит;
//!  • **B2-ядро** — персистентный объёмный сдвиг (тот самый, что флудил прод, ≫ ε_max) в рантайме
//!    МОЛЧИТ; within-reach эвикция НЕ-best уровня в рантайме МОЛЧИТ (объём → офлайн);
//!  • best-путь — по-прежнему эмитит (пост-seed пустая local; см. также `red_recon_sink`/`red_ops_recon`
//!    для best-десинка и `red_recon_live` для skew-толерантности/depth-skip гейджа);
//!  • детерминизм рантайм-вердиктов (seed — чистое рантайм-состояние, без wall-clock/rand).
//!
//! АНТИ-ПЛАЦЕБО В ОБЕ СТОРОНЫ (`.claude/rules/testing.md`):
//!  • B2-silent оракулы ПАДАЮТ против window-active impl (текущий прод-код эмитит на персистентном
//!    объёме — это §8-флуд B, который B2 удаляет) → доказывают РЕАЛЬНОЕ изменение поведения;
//!  • best-emit оракулы (9b + red_recon_sink/red_ops_recon) ПАДАЮТ против always-silent impl →
//!    запрещают «заглушить всё». Вместе набор допускает ровно best-only+seed-gate.
//!
//! testing.md чек-лист против РЕАЛЬНОГО (не идеального) входа: асимметрия — односторонний bid-дефицит
//! и односторонняя best-порча; множественность — все полосы расходятся в одном такте; отсутствие —
//! удалённый within-reach уровень НЕ рантайм-алерт (объём→офлайн); границы — пустая local до/после
//! seed, пустой reference (red_recon_live); прод-масштаб — сдвиг ≫ ε_max (моделирует прод 103..747).

use book::OrderBook;
use contracts::Level;
use ops::recon::{ReconDetector, ReconThresholds, EPS_PROD_DEFAULT_BPS, RECON_WINDOW};

const MID: i64 = 65_000_000_000_000; // $65k ×1e8
const UNIT: i64 = 100_000_000; // 1.0 объёма ×1e8
const BASE: f64 = 100.0; // базовый объём уровня (units)

/// Уровни на 0.05..0.55% от mid → reach≈0.55%, покрывает полосы recon 0.1/0.3/0.5%.
const PCTS: [f64; 6] = [0.0005, 0.0015, 0.0025, 0.0035, 0.0045, 0.0055];

/// Книга, где объём КАЖДОГО уровня стороны масштабирован: bid×`bid_scale`, ask×`ask_scale`.
/// ЦЕНЫ уровней НЕ меняются (best-price идентичен reference → best_price_diverged=false, изолируем
/// ОБЪЁМ). `scale=1.0` — reference-эталон; `1.15` — профицит +1500 bps; `0.85` — дефицит −1500 bps
/// (≫ ε_max=50 — моделирует прод-класс входа: часть значений порогом непобедима).
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

// ═════════════════════════════════════════════════════════════════════════════════════════════
// B2-ЯДРО: ОБЪЁМ В РАНТАЙМЕ МОЛЧИТ (near-touch объём REST-неверифицируем — §4.3.2).
// Эти оракулы — ИНВЕРСИЯ снятых оконных оракулов (persistent_volume_deficit/surplus/eviction
// ТРЕБОВАЛИ алерт; под B2 тот же вход ТРЕБУЕТ ТИШИНУ). Анти-плацебо: падают против window-active
// impl (прод-код эмитит на персистентном объёме — §8-флуд B).
// ═════════════════════════════════════════════════════════════════════════════════════════════

/// (B2-1, ПАДАЕТ против window-active impl) ПЕРСИСТЕНТНЫЙ ОБЪЁМНЫЙ ДЕФИЦИТ (−1500 bps ≫ ε_max) держится
/// 2×`RECON_WINDOW` циклов — ровно прод-класс, флудивший §8 (best=false, систематический сдвиг). Под B2
/// рантайм МОЛЧИТ: объёмная эмиссия снята, near-touch объём REST-неверифицируем → офлайн-трек. best-цена
/// цела → изоляция объёмного пути от best.
#[test]
fn runtime_persistent_volume_deficit_is_silent() {
    let mut det = detector();
    // local держит bid-объём на 15% НИЖЕ reference КАЖДЫЙ цикл (тот же вход, что в снятом оракуле
    // persistent_volume_deficit_alerts — теперь ТРЕБУЕТ ТИШИНУ). 2×окна: далеко за наполнением.
    let locals: Vec<OrderBook> = (0..RECON_WINDOW * 2)
        .map(|_| scaled_book(0.85, 1.0))
        .collect();
    let (any_alert, any_best) = run_sequence(&mut det, &locals);
    assert!(
        !any_best,
        "фикстура сломана: best-цена разошлась — тест изолирует ОБЪЁМ (цены не трогаются)"
    );
    assert!(
        !any_alert,
        "рантайм эмитил на ПЕРСИСТЕНТНОМ объёмном дефиците (−1500 bps ≫ ε_max, 2×{RECON_WINDOW} циклов) — \
         это РОВНО §8-флуд B, который B2 удаляет. Под B2 объёмная сверка снята из рантайма (REST-\
         неверифицируема, систематический WS-vs-REST bias) → офлайн-трек. Рантайм-alert обязан быть \
         ⟺ best_price_diverged; объёмный оконный путь снят из решения об эмиссии (window-active impl \
         не удалён)"
    );
}

/// (B2-2, ПАДАЕТ против window-active impl) ПЕРСИСТЕНТНЫЙ ОБЪЁМНЫЙ ПРОФИЦИТ (TD-016 near-touch фантом,
/// +1500 bps) — под B2 тоже ТИШИНА в рантайме (фантом near-touch/дальних полос → офлайн, §4.2/§4.3.2).
#[test]
fn runtime_persistent_volume_surplus_is_silent() {
    let mut det = detector();
    let locals: Vec<OrderBook> = (0..RECON_WINDOW * 2)
        .map(|_| scaled_book(1.15, 1.0))
        .collect();
    let (any_alert, any_best) = run_sequence(&mut det, &locals);
    assert!(!any_best, "фикстура: best цела (изоляция объёма)");
    assert!(
        !any_alert,
        "рантайм эмитил на персистентном объёмном ПРОФИЦИТЕ (+1500 bps, TD-016 near-touch фантом) — под \
         B2 объёмная сверка снята из рантайма; фантом обнаруживается офлайн над записанной книгой, не \
         рантайм-эмиссией"
    );
}

/// (B2-3, ОТСУТСТВИЕ + ПАДАЕТ против window-active impl) within-reach уровень (НЕ best, 0.25%) удалён
/// ПЕРСИСТЕНТНО — конкретная форма C1, НЕ двигающая best. Под B2 рантайм МОЛЧИТ (объёмное проявление C1 →
/// офлайн; в рантайме C1 ловится ТОЛЬКО через best bid). Снятый оракул near_book_eviction_persists_then_alerts
/// требовал алерт — под B2 инвертирован в тишину. best (0.05%) цел → best_price_diverged=false.
#[test]
fn runtime_nonbest_eviction_is_silent() {
    let mut det = detector();
    let reference = reference();
    // local: near-book БЕЗ уровня 0.25% (эвикция C1 стёрла within-reach НЕ-best уровень; best 0.05% цел).
    let evicted: Vec<f64> = PCTS.iter().copied().filter(|&p| p != 0.0025).collect();
    let mut any_alert = false;
    let mut any_best = false;
    for _ in 0..RECON_WINDOW * 2 {
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
        "фикстура: best bid (0.05%) цел — эвикция стёрла within-reach уровень 0.25%, НЕ best"
    );
    assert!(
        !any_alert,
        "within-reach НЕ-best уровень 0.25% удалён персистентно (объёмное проявление C1), а рантайм \
         эмитил — под B2 объёмная сверка снята из рантайма (REST-неверифицируема). C1, двигающий best, \
         ловится best-путём; объёмный — офлайн-треком над записанной книгой"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════════════
// BEST-ПУТЬ ОСТАЁТСЯ (запрет «заглушить всё»): пост-seed пустая local — РЕАЛЬНАЯ порча → эмит.
// (Также: best-десинк — red_recon_sink::best_desync_emits_immediately, red_ops_recon; skew — red_recon_live.)
// ═════════════════════════════════════════════════════════════════════════════════════════════

/// (BEST, GREEN — анти-плацебо к B2-silent: НЕ «заглушить всё») ПОСЛЕ seed внезапно пустая local — это
/// РЕАЛЬНАЯ потеря/порча книги (best исчез) → ОБЯЗАН эмитить. Валит always-silent impl. Прямой guard
/// против «B2 = молчать всегда» на РАНТАЙМ-пути (best).
#[test]
fn runtime_post_seed_empty_local_still_emits() {
    let mut det = detector();
    let reference = reference();
    let seed = scaled_book(1.0, 1.0);
    let v0 = det.observe(&seed, &reference);
    assert!(
        !v0.alert,
        "seed на идентичных книгах не алертит (иначе фикстура невалидна)"
    );
    let empty = OrderBook::new();
    let v1 = det.observe(&empty, &reference);
    assert!(
        v1.alert && v1.best_price_diverged,
        "пост-seed пустая local (потеря живой книги — РЕАЛЬНАЯ порча) не эмитировала — B2 НЕ смеет глушить \
         best-путь; рантайм молчит ТОЛЬКО по объёму, best-расхождение эмитит всегда"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (9) SEED-GATE (третий §8-провал, дефект A — §8-подтверждён РАБОЧИМ, merge e9fc258). Ортогонален B2,
//     остаётся sacred. Прод-факт (reviewer, b1adec0): recon эмитил 4 стартовых `best_diverged=true
//     div_bps=10000 Resynced` на КАЖДЫЙ рестарт — сравнение REST-reference с ПУСТОЙ local (fetcher
//     тянет REST ДО первого L2Snapshot feeder'а). Дизайн — SELF-SEEDING `ReconDetector` (`seeded: bool`,
//     true на первой НЕПУСТОЙ local; до seed — no-alert И не кормит состояние).
// ─────────────────────────────────────────────────────────────────────────────

/// (9a, ПАДАЛ против pre-seed impl) Пока local ПУСТА (не seeded), recon НЕ эмитит НИЧЕГО даже против
/// полного REST-reference. Self-seeding подавляет стартовый флуд `best_diverged=true div=10000`.
#[test]
fn empty_local_before_first_seed_does_not_emit() {
    let mut det = detector();
    let reference = reference();
    let empty = OrderBook::new();
    let mut any_alert = false;
    for _ in 0..RECON_WINDOW {
        let v = det.observe(&empty, &reference);
        any_alert |= v.alert;
    }
    assert!(
        !any_alert,
        "recon эмитил на ПУСТОЙ (не seeded) local — стартовый флуд `best_diverged=true div=10000` \
         (дефект A): fetcher тянет REST ДО первого L2Snapshot. Пустая СВОЯ книга ≠ порча биржи — \
         детектор обязан self-seed'иться на первой НЕПУСТОЙ local и молчать до seed"
    );
}

/// (9b, GREEN — анти-плацебо к seed-gate: не over-suppress) ПОСЛЕ первого снапшота (seeded) внезапно
/// пустая local — РЕАЛЬНАЯ потеря/порча → эмит (best-путь). Валит «всегда молчать на пустой local».
#[test]
fn empty_local_after_seed_is_corruption_and_emits() {
    let mut det = detector();
    let reference = reference();
    let seed = scaled_book(1.0, 1.0);
    let v0 = det.observe(&seed, &reference);
    assert!(
        !v0.alert,
        "seed-цикл на идентичных книгах не алертит (иначе фикстура невалидна)"
    );
    let empty = OrderBook::new();
    let v1 = det.observe(&empty, &reference);
    assert!(
        v1.alert && v1.best_price_diverged,
        "пост-seed пустая local (потеря живой книги — РЕАЛЬНАЯ порча, не старт) не поднялась: seed-gate \
         заглушил настоящее расхождение (over-suppress). Гейт молчит ТОЛЬКО до первого seed"
    );
}

/// (9c, ПАДАЛ против current+poison impl — находка critic C-012) Seed-gate = ДВА обязательства: до seed
/// observe (1) НЕ алертит И (2) НЕ КОРМИТ состояние. Плохой impl может подавить стартовый алерт, но
/// протолкнуть pre-seed наблюдения (пустая local vs полный REST → большой односторонний сигнал) в
/// состояние — тогда после seed на здоровом рынке всплывает ложный алерт. Фикстура: `RECON_WINDOW`
/// циклов pre-seed пустой local, затем `RECON_WINDOW` циклов ЗДОРОВЫХ ИДЕНТИЧНЫХ книг → тишина всю
/// последовательность у корректного self-seeding (не кормит состояние до seed). Под B2 объёмного окна
/// нет, но контракт «не кормить состояние до seed» + «здоровое после seed молчит» сохранён и остаётся
/// guard'ом seed-gate'а под любым impl (best-путь тоже обязан молчать на здоровых идентичных книгах).
#[test]
fn pre_seed_empty_does_not_poison_state() {
    let mut det = detector();
    let reference = reference();
    let empty = OrderBook::new();
    let healthy = scaled_book(1.0, 1.0); // == reference → здоровый рынок

    let mut any_alert = false;
    for _ in 0..RECON_WINDOW {
        any_alert |= det.observe(&empty, &reference).alert;
    }
    for _ in 0..RECON_WINDOW {
        any_alert |= det.observe(&healthy, &reference).alert;
    }
    assert!(
        !any_alert,
        "pre-seed пустая local ОТРАВИЛА состояние: после seed на ЗДОРОВЫХ идентичных книгах детектор \
         алертит — pre-seed пустышки попали в состояние. observe ДО seed обязан НЕ КОРМИТЬ состояние, а \
         не только молчать (critic C-012: 9a пиннил no-alert, но не no-feed)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (ДЕТЕРМИНИЗМ) одинаковая последовательность наблюдений → одинаковая последовательность вердиктов
//     (seed — рантайм-состояние, но чистое: нет wall-clock/rand). DET-принцип для рантайм-recon.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn runtime_detector_is_deterministic_across_replay() {
    let reference = reference();
    // Смешанная последовательность рантайм-пути: пустой pre-seed → seed здоровым → пост-seed пустой
    // (порча, best-эмит) → здоровый. Проверяем и тишину, и алерт на РАНТАЙМ-пути (best + seed-gate).
    let verdicts = |()| -> Vec<bool> {
        let mut det = detector();
        let seq: Vec<OrderBook> = vec![
            OrderBook::new(),      // pre-seed пусто → no-alert (seed-gate)
            scaled_book(1.0, 1.0), // seed здоровым → no-alert
            OrderBook::new(),      // пост-seed пусто → best-эмит (порча)
            scaled_book(1.0, 1.0), // здоровый → no-alert
        ];
        seq.iter()
            .map(|l| det.observe(l, &reference).alert)
            .collect()
    };
    let run1 = verdicts(());
    let run2 = verdicts(());
    assert_eq!(
        run1, run2,
        "два прогона одной последовательности дали РАЗНЫЕ эмиссии — детектор недетерминирован (seed — \
         чистое рантайм-состояние, без wall-clock/rand; DET-принцип рантайм-recon)"
    );
    assert!(
        run1.iter().any(|&a| a),
        "фикстура-сетап не состоялся: пост-seed пустая local обязана была поднять best-алерт (иначе \
         детерминизм проверяется вхолостую на вечной тишине)"
    );
}
