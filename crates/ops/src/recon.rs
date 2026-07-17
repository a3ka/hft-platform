//! OPS-I-1 — сверка локальной книги с независимым REST-снапшотом биржи (`ops.md` §4).
//!
//! Recon — ЕДИНСТВЕННАЯ проверка ПРАВИЛЬНОСТИ данных (эвикция C1 стирала best bid при зелёном
//! healthcheck). Компаратор чист: `reconcile(local, reference)` → расхождение по best bid/ask и
//! суммам полос. Значимое расхождение → `SysEvent::ReconDivergence` в журнал (CT-RFC-03) + алерт.
//!
//! Порог — ТРИ числа (`ops.md` §4): `ε_test` (гейт, фикс, не калибруется), `ε_prod` (рабочий,
//! калибруется), `ε_max` (fail-closed потолок; `ε_prod ≤ ε_max`).

use book::OrderBook;
use contracts::{ReconAction, ReconAudit, Side, Venue};

/// ε_test (гейт RED, НЕ калибруется): ЛЮБОЕ расхождение best bid/ask ИЛИ ≥ этого по суммам полос.
pub const EPS_TEST_BPS: i64 = 50;
/// ε_max (fail-closed потолок): `ε_prod` не может быть задан выше — «откалибровали до ∞» запрещено.
pub const EPS_MAX_BPS: i64 = 50;
/// ε_prod по умолчанию (bps по суммам полос; калибруется по первым суткам).
pub const EPS_PROD_DEFAULT_BPS: i64 = 5;

/// Допуск на sub-bp timing-skew лучшей цены между WS-книгой и REST-снапшотом (bps, `ops.md`
/// §4.2a). Подпороговый best-shift — норма, сверх — десинк/протухшая книга (ε_test).
/// `5 bps` — между sub-bp skew (0.5 bp — норма) и реальным десинком (10 bp — десинк); пинит
/// верхний край теста `best_price_timing_skew_is_tolerated` (0.5 bp = skew) и нижний край
/// теста `real_desync_best_moved_ten_bps_still_diverges` (10 bp = десинк).
pub const BEST_SKEW_BPS: i64 = 5;

/// Ценовые полосы recon (доли mid), `ops.md` §4 п.2. Near-book redesign (founder ★ 2026-07-17):
/// REST-снапшот Binance достаёт лишь ~1.1–1.7% от mid (измерено architect'ом), поэтому полосы
/// ОБЯЗАНЫ быть ≤0.8%, а полоса, которую reference НЕ достаёт (`reference.max_reach_pct(side) <
/// band`), ПРОПУСКАЕТСЯ — невалидируемое ≠ расхождение (асимметрия глубины, §8-провал).
pub const RECON_BANDS: [f64; 3] = [0.001, 0.003, 0.005];

/// Длина окна персистентности `K` (число recon-циклов), `ops.md` §4.3. Второй §8-провал
/// (2026-07-17): near-book ОБЪЁМНЫЕ суммы полос churn'ят между local (WS-книга, момент T1) и
/// reference (async REST, момент T2) — architect измерил живой Binance: знак per-cycle GULЯЕТ
/// (mean-reverting), магнитуда до сотен-тысяч bps. Per-cycle порог по объёму ПРИНЦИПИАЛЬНО
/// нежизнеспособен (любой ловящий порог флудит на churn). Дискриминатор churn↔порча — НЕ магнитуда,
/// а ПЕРСИСТЕНТНОСТЬ: churn mean→0 за окно, порча (C1-стрип / TD-016 near-touch фантом) держит знак.
/// `K` калибруется по живому churn-остатку (`[verify-at-impl]` + §8): длиннее окно → сильнее гасится
/// churn. Дефолт 12 циклов (при cadence 5 мин — 1 час). **Best-price остаётся PER-CYCLE** (immediate,
/// §4.2a) — окно только для ОБЪЁМА near-touch.
pub const RECON_WINDOW: usize = 12;

/// Рабочий порог recon. Конструктор fail-closed: `prod_bps ≤ EPS_MAX_BPS`.
#[derive(Debug, Clone, Copy)]
pub struct ReconThresholds {
    prod_bps: i64,
}

impl ReconThresholds {
    /// `Err`, если `prod_bps > EPS_MAX_BPS` (fail-closed: нельзя откалибровать порог до бесконечности).
    pub fn new(prod_bps: i64) -> Result<Self, String> {
        if prod_bps > EPS_MAX_BPS {
            return Err(format!(
                "ε_prod={prod_bps} > ε_max={EPS_MAX_BPS}: fail-closed потолок превышен \
                 (нельзя откалибровать порог до бесконечности, OPS-I-1)"
            ));
        }
        Ok(Self { prod_bps })
    }
    pub fn prod_bps(&self) -> i64 {
        self.prod_bps
    }
}

/// Результат сверки локальной книги с REST-снапшотом.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconOutcome {
    /// Максимальное расхождение сумм полос, bps (магнитуда, ≥ 0).
    pub divergence_bps: i64,
    /// Расхождение по ЛУЧШЕЙ цене (bid или ask) — порча (C1-класс), а не шум дальних полос.
    pub best_price_diverged: bool,
}

impl ReconOutcome {
    /// Превышает `ε_test`: best-price разошлась ИЛИ `divergence_bps ≥ EPS_TEST_BPS`.
    ///
    /// **⚠ ОБЪЁМНАЯ ветка (`divergence_bps ≥ EPS_TEST_BPS`) СНЯТА С ЭМИССИИ (`ops.md` §4.3, второй
    /// §8-провал 2026-07-17).** Per-cycle сумма полос churn'ит на здоровом рынке (timing-skew local
    /// vs async REST) → per-cycle порог по объёму флудит. Решение об эмиссии для ОБЪЁМА принимает
    /// оконный `ReconDetector` (персистентность знака). `divergence_bps` остаётся PER-CYCLE ГЕЙДЖЕМ
    /// для метрики `book_divergence_bps` (§3). Best-price (immediate) — по-прежнему через эту ветку.
    ///
    /// Per-cycle ε_test = BEST-ONLY: best-price расхождение (десинк/C1-стрип best'а) алертит немедленно.
    /// ОБЪЁМ near-touch ушёл в оконный `ReconDetector` — здесь его нет (иначе §8-флуд на churn).
    pub fn exceeds_test(&self) -> bool {
        self.best_price_diverged
    }
    /// Per-cycle ε_prod = BEST-ONLY (объём — в `ReconDetector`). `thr` сохранён для совместимости
    /// сигнатуры и как носитель калибруемого ε_prod, который теперь применяет окно, а не per-cycle.
    pub fn exceeds_prod(&self, _thr: &ReconThresholds) -> bool {
        self.best_price_diverged
    }
    /// Аудит-событие для журнала (CT-RFC-03). `divergence_bps`/`best_price_diverged` — из self.
    pub fn to_audit(&self, venue: Venue, symbol: &str, action: ReconAction) -> ReconAudit {
        ReconAudit {
            venue,
            symbol: symbol.to_string(),
            divergence_bps: self.divergence_bps,
            best_price_diverged: self.best_price_diverged,
            action,
        }
    }
}

/// Сверить локальную книгу с REST-референсом: best bid/ask + суммы полос `RECON_BANDS`.
/// Чистая функция (детерминизм). `reference` — книга, собранная из REST-снапшота (venue-dev).
///
/// **Near-book semantics (founder ★ 2026-07-17, `docs/fa/ops.md` §4.2):**
///
/// - **(a) best-price divergence:** С ТОЛЕРАНТНОСТЬЮ `BEST_SKEW_BPS`. Sub-bp timing-skew между
///   WS-книгой и REST-моментом — НОРМА (best_price_diverged = false). Свысшего сдвига — десинк
///   (best_price_diverged = true, ловит C1 — эвикция стирала именно best bid).
///   Односторонняя книга (local имеет best, reference пуст — REST halt/сбой, или наоборот) —
///   тоже `best_price_diverged = true`: невалидируемость при живой другой стороне ПОДНИМАЕТСЯ,
///   а не глушится (`empty_reference_is_not_silently_ok`).
/// - **(b) полосы:** МЕЛКИЕ (`RECON_BANDS = [0.1%, 0.3%, 0.5%]` в пределах гарантированного
///   REST-reach ~1.1%). Полоса, которую reference НЕ достаёт (`reference.max_reach_pct(side) <
///   band`) ПРОПУСКАЕТСЯ (невалидируемо ≠ расхождение). Это и есть фикс §8-провала — local≫ref
///   по построению на глубоких полосах → структурный флуд.
///
/// `divergence_bps`: максимум по СРАВНИВАЕМЫМ (within reference-reach) полосам относительного
/// расхождения сумм объёмов, в basis points (1 bp = 0.01%, ×10000). `denom = max(|sum_ref|, 1)`
/// — нулевой референс для полосы пропускается (см. выше), а невалидируемое ≠ расхождение.
///
/// **Bounded-time:** O(|RECON_BANDS| × |side| × depth) = O(n) по сумме уровней обеих сторон.
/// Для 5000-уровневой книги Binance — десятки тысяч операций за один reconcile; на медленном CPU
/// десятки мкс. Никаких вложенных сканов по одним и тем же уровням.
pub fn reconcile(local: &OrderBook, reference: &OrderBook) -> ReconOutcome {
    let best_price_diverged = best_side_diverged(local.best_bid(), reference.best_bid())
        || best_side_diverged(local.best_ask(), reference.best_ask());

    let mut max_bps: i64 = 0;
    for &band in &RECON_BANDS {
        for side in [Side::Buy, Side::Sell] {
            // Глубина reference для этой стороны. None/0 → reference не достаёт ничего → пропуск.
            let ref_reach = reference.max_reach_pct(side).unwrap_or(0.0);
            if ref_reach < band {
                continue;
            }
            let l = local.depth_within(side, band);
            let r = reference.depth_within(side, band);
            let denom = if r.abs() < 1 { 1 } else { r.abs() };
            let diff_bps = ((l - r).abs() * 10_000) / denom;
            if diff_bps > max_bps {
                max_bps = diff_bps;
            }
        }
    }

    ReconOutcome {
        divergence_bps: max_bps,
        best_price_diverged,
    }
}

/// Сравнить одну сторону best-price С ТОЛЕРАНТНОСТЬЮ `BEST_SKEW_BPS`. Четыре случая:
/// - **оба `None`** → расхождения нет (невалидируемое согласованное);
/// - **ровно один `None`** → расхождение (невалидируемость при живой другой стороне
///   ПОДНИМАЕТСЯ — `empty_reference_is_not_silently_ok`);
/// - **оба `Some`** → `|diff| / max(local, 1) × 10_000` в bps; сверх `BEST_SKEW_BPS` — расхождение
///   (sub-bp skew — норма, реальный десинк — порча).
fn best_side_diverged(l: Option<i64>, r: Option<i64>) -> bool {
    match (l, r) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(li), Some(ri)) => {
            let diff = (li as i128 - ri as i128).unsigned_abs();
            // bps = diff / max(local, 1) × 10_000 — нормируем к локальной цене.
            let norm = (li as u128).max(1);
            let bps = diff.saturating_mul(10_000) / norm;
            bps > BEST_SKEW_BPS as u128
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════════════
// ОКОННЫЙ ДЕТЕКТОР ПЕРСИСТЕНТНОСТИ (`ops.md` §4.3, второй §8-провал 2026-07-17).
//
// Recon становится STATEFUL по ОБЪЁМУ: per (venue, symbol) держим окно `RECON_WINDOW` циклов на
// каждую (полосу, сторону). Дискриминатор churn↔порча — ЗНАКОВОЕ СРЕДНЕЕ:
//   • churn (timing-skew): знак per-cycle гуляет → mean→0 за окно → ТИШИНА;
//   • порча (C1-стрип / TD-016 near-touch фантом): local ОДНОСТОРОННЕ ниже/выше reference каждый
//     цикл → mean держит знак и магнитуду → АЛЕРТ.
// Магнитуда per-cycle у churn и порчи МОЖЕТ БЫТЬ ОДИНАКОВОЙ — различает ТОЛЬКО персистентность.
// Best-price — по-прежнему PER-CYCLE (immediate, `reconcile` best-ветка): C1 стирал именно best bid.
// Детерминизм эмиссий: окно — рантайм-состояние (как `ReconBudget`), НЕ журналируется (JR-I-1/OPS-I-6);
// одинаковая последовательность наблюдений → одинаковая последовательность вердиктов (нет wall-clock/rand).
// ═════════════════════════════════════════════════════════════════════════════════════════════

// ВНУТРЕННЕЕ ПРЕДСТАВЛЕНИЕ ОКНА (кольцевой буфер знаковых наблюдений per (полоса,сторона), знаковое
// среднее, skip-за-reach) — ЗОНА engine-dev (impl-детали, а НЕ контракт). Прототип корректного impl,
// доказавший достижимость (все recon-оракулы + оконные GREEN, анти-плацебо в обе стороны), реверчен
// architect'ом: `git show` этого коммита в истории/`C-0NN` verdict несёт эталон. Контракт, который
// engine-dev обязан удовлетворить, — RED-оракулы `crates/ops/tests/red_recon_window.rs` (sacred).

/// Вердикт одного recon-цикла оконного детектора.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconVerdict {
    /// Эмитировать `SysEvent::ReconDivergence`: best-price разошлась (immediate) ИЛИ хотя бы одно
    /// заполненное окно (полоса,сторона) держит `|signed_mean| ≥` порога (персистентная порча объёма).
    pub alert: bool,
    /// Best-price расхождение этого цикла (immediate, §4.2a). Пишется в аудит как есть.
    pub best_price_diverged: bool,
    /// Магнитуда для аудита: `|signed_mean|` худшего сработавшего окна (объём) ИЛИ per-cycle гейдж,
    /// если сработала только best-ветка. Кладётся в `ReconAudit.divergence_bps`.
    pub window_divergence_bps: i64,
    /// Per-cycle гейдж (`reconcile().divergence_bps`) для метрики `book_divergence_bps` (§3).
    pub gauge_divergence_bps: i64,
}

/// Оконный детектор персистентности per (venue, symbol). Держит окно на каждую (полосу × сторону).
/// Живёт в оркестраторе рядом с `ReconBudget` (рантайм-состояние). `thr` — калибруемый ε_prod (окно),
/// `EPS_TEST_BPS` — фиксированный гейт (не калибруется): персистентная порча с `|mean| ≥ ε_test` алертит
/// независимо от ε_prod.
///
/// СКЕЛЕТ (architect: сигнатура + контракт). Внутреннее поле окна — impl-деталь engine-dev; здесь
/// хранится только калибруемый `thr` (ε_prod окна). `EPS_TEST_BPS` — фиксированный гейт (не
/// калибруется): персистентная порча с `|mean| ≥ ε_test` алертит независимо от ε_prod (fail-closed).
#[derive(Debug, Clone)]
pub struct ReconDetector {
    /// Калибруемый ε_prod окна (`≤ EPS_MAX_BPS`). engine-dev добавляет внутреннее окно персистентности
    /// per (полоса,сторона) рядом (impl-деталь; форма — за engine-dev, ops.md §4.3).
    thr: ReconThresholds,
}

impl ReconDetector {
    pub fn new(thr: ReconThresholds) -> Self {
        Self { thr }
    }

    /// Скормить один recon-цикл: best (per-cycle, immediate) + знаковый ОБЪЁМ каждой (полосы,стороны)
    /// в окно; алерт, если best разошёлся ИЛИ ЗАПОЛНЕННОЕ окно держит `|signed_mean|` над порогом
    /// (`ε_test` гейт ИЛИ `ε_prod`). Детерминирована (окно — чистое рантайм-состояние, без wall-clock/rand).
    ///
    /// Контракт (RED `red_recon_window.rs`, sacred): churn (знак per-cycle гуляет) → mean→0 → ТИШИНА
    /// даже при той же per-cycle магнитуде, что у порчи; персистентный дефицит/профицит (C1-стрип /
    /// TD-016 near-touch фантом) → АЛЕРТ; skip полос за пределами `reference.max_reach_pct(side)`.
    pub fn observe(&mut self, local: &OrderBook, reference: &OrderBook) -> ReconVerdict {
        // engine-dev: реализовать оконный детектор персистентности (ops.md §4.3). Best-ветка —
        // per-cycle через `reconcile(local, reference)`; объём — знаковое среднее окна per (полоса,сторона),
        // порог `EPS_TEST_BPS.min(self.thr.prod_bps())`. См. reverted-прототип в истории коммита.
        let _ = (&self.thr, local, reference);
        todo!("engine-dev: оконный детектор персистентности объёма — контракт в red_recon_window.rs (ops.md §4.3)")
    }

    /// Аудит-событие (CT-RFC-03) из последнего вердикта. `divergence_bps` = оконная/гейдж-магнитуда,
    /// `best_price_diverged` — как в вердикте. Форма T1 не меняется (windowed-алерт = best=false +
    /// divergence_bps=|mean|).
    pub fn verdict_to_audit(
        v: &ReconVerdict,
        venue: Venue,
        symbol: &str,
        action: ReconAction,
    ) -> ReconAudit {
        ReconAudit {
            venue,
            symbol: symbol.to_string(),
            divergence_bps: v.window_divergence_bps,
            best_price_diverged: v.best_price_diverged,
            action,
        }
    }
}
