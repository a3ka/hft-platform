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
