//! OPS-I-1 — сверка локальной книги с независимым REST-снапшотом биржи (`ops.md` §4).
//!
//! Recon — ЕДИНСТВЕННАЯ проверка ПРАВИЛЬНОСТИ данных (эвикция C1 стирала best bid при зелёном
//! healthcheck). Компаратор чист: `reconcile(local, reference)` → расхождение по best bid/ask и
//! суммам полос. Значимое расхождение → `SysEvent::ReconDivergence` в журнал (CT-RFC-03) + алерт.
//!
//! Порог — ТРИ числа (`ops.md` §4): `ε_test` (гейт, фикс, не калибруется), `ε_prod` (рабочий,
//! калибруется), `ε_max` (fail-closed потолок; `ε_prod ≤ ε_max`).

use book::OrderBook;
use contracts::{ReconAction, ReconAudit, Venue};

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
    pub fn new(_prod_bps: i64) -> Result<Self, String> {
        todo!("OPS-I-1: prod_bps ≤ EPS_MAX_BPS иначе Err (fail-closed потолок)")
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
        todo!("OPS-I-1: best_price_diverged || divergence_bps >= EPS_TEST_BPS")
    }
    /// Превышает рабочий `ε_prod` (алерт в проде после калибровки).
    pub fn exceeds_prod(&self, _thr: &ReconThresholds) -> bool {
        todo!("OPS-I-1: best_price_diverged || divergence_bps >= thr.prod_bps")
    }
    /// Аудит-событие для журнала (CT-RFC-03). `divergence_bps`/`best_price_diverged` — из self.
    pub fn to_audit(&self, _venue: Venue, _symbol: &str, _action: ReconAction) -> ReconAudit {
        todo!("OPS-I-1: собрать ReconAudit из outcome (venue/symbol/bps/best/action)")
    }
}

/// Сверить локальную книгу с REST-референсом: best bid/ask + суммы полос `RECON_BANDS`.
/// Чистая функция (детерминизм). `reference` — книга, собранная из REST-снапшота (venue-dev).
pub fn reconcile(_local: &OrderBook, _reference: &OrderBook) -> ReconOutcome {
    todo!(
        "OPS-I-1: best_price_diverged = (best_bid|best_ask отличаются); \
         divergence_bps = max по RECON_BANDS от |sum_local - sum_ref| / sum_ref в bps"
    )
}
