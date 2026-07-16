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

/// Ценовые полосы recon (доли mid), `ops.md` §4 п.2. Суммы объёма в каждой сверяются local vs ref.
pub const RECON_BANDS: [f64; 3] = [0.015, 0.03, 0.08];

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
    /// Всегда алерт (с первой минуты, `ε_test` не калибруется).
    pub fn exceeds_test(&self) -> bool {
        self.best_price_diverged || self.divergence_bps >= EPS_TEST_BPS
    }
    /// Превышает рабочий `ε_prod` (алерт в проде после калибровки).
    pub fn exceeds_prod(&self, thr: &ReconThresholds) -> bool {
        self.best_price_diverged || self.divergence_bps >= thr.prod_bps
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
/// **Bounded-time:** O(|RECON_BANDS| × |side| × depth) = O(n) по сумме уровней обеих сторон.
/// Для 5000-уровневой книги Binance — десятки тысяч операций за один reconcile; на медленном CPU
/// десятки мкс (замер в §6.1 FA). Никаких вложенных сканов по одним и тем же уровням.
///
/// `best_price_diverged`: лучшая цена ЛЮБОЙ стороны отличается. Это ПОРЧА (C1 — эвикция стирала
/// именно best bid), не шум дальних полос; алертится ВСЕГДА (ε_test), независимо от ε_prod.
///
/// `divergence_bps`: максимум по `RECON_BANDS` относительного расхождения сумм объёмов в полосе,
/// в basis points (1 bp = 0.01%, ×10000). `denom = max(|sum_ref|, 1)` — нулевой референс даёт
/// `|sum_local|/1`, а не деление на 0 (REF-снапшот с одной битой стороной не должен падать).
pub fn reconcile(local: &OrderBook, reference: &OrderBook) -> ReconOutcome {
    let best_price_diverged =
        local.best_bid() != reference.best_bid() || local.best_ask() != reference.best_ask();

    let mut max_bps: i64 = 0;
    for &band in &RECON_BANDS {
        for side in [Side::Buy, Side::Sell] {
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
