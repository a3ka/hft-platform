//! OPS-I-1 — сверка локальной книги с независимым REST-снапшотом биржи (`docs/fa/ops.md` §4).
//!
//! Recon — ЕДИНСТВЕННАЯ проверка ПРАВИЛЬНОСТИ данных (эвикция C1 стирала best bid при зелёном
//! healthcheck). Компаратор чист: `reconcile(local, reference)` → расхождение по best bid/ask и
//! суммам полос. Значимое расхождение → `SysEvent::ReconDivergence` в журнал (CT-RFC-03) + алерт.
//!
//! **B2 (founder ★ 2026-07-18, `docs/fa/ops.md` §4.3.2):** РАНТАЙМ-recon = **best-price per-cycle +
//! seed-gate**. Объёмная near-touch сверка СНЯТА С РАНТАЙМА (три §8-провала подряд показали
//! систематический WS(T1)-vs-REST(T2) объёмный bias; near-touch объём REST-неверифицируем) →
//! офлайн-трек research-dev. Рантайм-alert ⟺ `best_price_diverged`. `book_divergence_bps` (per-cycle
//! гейдж) живёт как НАБЛЮДАТЕЛЬНЫЙ сигнал для офлайн-трека; эмиссии `Sys` по объёму нет.
//!
//! Порог — ТРИ числа (`ops.md` §4): `ε_test` (гейт, фикс, не калибруется), `ε_prod` (рабочий,
//! калибруется), `ε_max` (fail-closed потолок; `ε_prod ≤ ε_max`).

use book::OrderBook;
use contracts::{ReconAction, ReconAudit, Side, Venue};

/// ε_test (гейт RED, НЕ калибруется): ЛЮБОЕ расхождение best bid/ask. Под B2 — только best-ветка:
/// объёмная сверка снята с рантайма (REST-неверифицируема, §4.3.2).
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

/// Длина окна персистентности `K` (число recon-циклов), `ops.md` §4.3 — **ВЕТИГИАЛЬНАЯ под B2.**
/// Исторически использовалась оконным `ReconDetector` для дискриминации churn↔порча по объёму
/// (персистентность знакового среднего). Под B2 (`§4.3.2`, founder ★ 2026-07-18) объёмная
/// сверка снята С РАНТАЙМА — рантайм-alert ⟺ `best_price_diverged`, объёмного окна в коде нет.
/// Константа оставлена публичной для совместимости с SACRED-тестами `red_recon_runtime.rs`/
/// `red_recon_sink.rs` (используют её как размер последовательности в фикстурах: «2×`RECON_WINDOW`
/// циклов персистентного сдвига»), но НЕ читается реализацией (`Window`/`windows`/push-цикл
/// удалены, см. §B2-cleanup).
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

/// Результат сверки локальной книги с REST-снапшотом (per-cycle, чистая функция).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconOutcome {
    /// Per-cycle гейдж: максимальное расхождение сумм полос в пределах reference-reach, bps
    /// (магнитуда, ≥ 0). Под B2 — **наблюдательный сигнал** (метрика `book_divergence_bps`),
    /// НЕ источник рантайм-эмиссии.
    pub divergence_bps: i64,
    /// Расхождение по ЛУЧШЕЙ цене (bid или ask) — единственный рантайм-триггер под B2
    /// (`§4.3.2`): десинк/C1-стрип best'а ловится немедленно.
    pub best_price_diverged: bool,
}

impl ReconOutcome {
    /// Превышает `ε_test` ⟺ **best-price** расхождение. Под B2 (`§4.3.2`) `divergence_bps` —
    /// per-cycle ГЕЙДЖ наблюдаемости, НЕ источник эмиссии (объёмная сверка снята с рантайма;
    /// REST-неверифицируема, систематический WS-vs-REST bias).
    pub fn exceeds_test(&self) -> bool {
        self.best_price_diverged
    }
    /// Превышает `ε_prod` ⟺ **best-price** расхождение (аналогично `exceeds_test` — под B2 объёмной
    /// ветки в рантайме нет, `thr` сохранён в сигнатуре для совместимости API).
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
/// Под B2 — per-cycle ГЕЙДЖ для метрики `book_divergence_bps` (наблюдаемость), НЕ триггер эмиссии.
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
// STATEFUL-ДЕТЕКТОР (`docs/fa/ops.md` §4.3 + §4.3.1 + §4.3.2 B2):
//
// Под B2 (founder ★ 2026-07-18) рантайм-recon = **best-price per-cycle + seed-gate**. Объёмного
// окна в коде НЕТ: `Window`/`windows`/push-цикл/`window_divergence_bps`/`any_full`/`max_abs_mean`
// УДАЛЕНЫ как мёртвая машинерия (дискриминатор churn↔порча по персистентности знака принципиально
// не сходится на живом рынке — систематический WS(T1)-vs-REST(T2) bias, три §8-провала подряд).
//
// Остаются ДВА инварианта:
//   1. **Рантайм-alert ⟺ `best_price_diverged`** — десинк/C1-стрип best'а ловится immediate.
//   2. **Seed-gate (§4.3.1)** — до первой НЕПУСТОЙ local детектор молчит («моя книга ещё не
//      пришла» ≠ «биржа испортилась»); после первой непустой — `seeded=true` навсегда. Пост-seed
//      пустая local — РЕАЛЬНАЯ потеря/порча → best-путь эмитит немедленно (gate НЕ over-suppress'ит).
//
// Детерминизм: `seeded` — чистое рантайм-состояние по последовательности наблюдений (нет
// wall-clock/rand); одинаковая последовательность наблюдений → одинаковая последовательность
// вердиктов (`runtime_detector_is_deterministic_across_replay`).
// ═════════════════════════════════════════════════════════════════════════════════════════════

/// Вердикт одного recon-цикла. Под B2 (`§4.3.2`) — компактная best-only форма:
/// рантайм-эмиссия определяется `alert` (⟺ `best_price_diverged`); `divergence_bps` — per-cycle
/// гейдж для аудита (`verdict_to_audit` кладёт его в `ReconAudit.divergence_bps`) и метрики
/// `book_divergence_bps` (наблюдаемость, обновляется КАЖДЫЙ цикл вне зависимости от `alert`).
/// T1 (`ReconAudit`) форма НЕ меняется → CT-RFC НЕ нужен.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconVerdict {
    /// Эмитировать `SysEvent::ReconDivergence` ⟺ `best_price_diverged`. Под B2 — единственный
    /// рантайм-триггер; объёмная сверка снята (`§4.3.2`).
    pub alert: bool,
    /// Best-price расхождение этого цикла (immediate, `§4.2a`). Пишется в аудит как есть.
    pub best_price_diverged: bool,
    /// Per-cycle гейдж (`reconcile().divergence_bps`): для `ReconAudit.divergence_bps` (аудит
    /// на best-эмиссии, офлайн-агрегация) и метрики `book_divergence_bps` (наблюдаемость, §3).
    /// На тишине кладётся в гейдж (0/норма), в аудит НЕ идёт (sink не зовёт `verdict_to_audit`).
    pub divergence_bps: i64,
}

/// Детектор per (venue, symbol) — **SEED-GATE-ONLY** под B2. Живёт в оркестраторе рядом с
/// `ReconBudget` (рантайм-состояние, передаётся `&mut` в `sink::handle_recon_snapshot` —
/// сигнатура СОХРАНЕНА; carve-out recorder НЕ расширяется). `thr` — калибруемый ε_prod,
/// сохранён в конструкторе для совместимости API (sacred-RED ссылается на
/// `ReconDetector::new(ReconThresholds::new(...))`); под B2 в `observe()` не применяется.
#[derive(Debug, Clone)]
pub struct ReconDetector {
    /// Калибруемый ε_prod (`≤ EPS_MAX_BPS`). Под B2 — ВЕТИГИАЛЬНЫЙ (рантайм-alert ⟺ best,
    /// объёмной ветки в рантайме нет; `thr` сохранён в конструкторе для совместимости API с
    /// `crates/recorder/src/main.rs` и SACRED-RED `red_recon_runtime.rs`/`red_recon_sink.rs` —
    /// оба зовут `ReconDetector::new(ReconThresholds::new(...))`; сигнатура конструктора и
    /// recorder-carve-out не растут). `verdict.divergence_bps` берётся из
    /// `reconcile().divergence_bps` (per-cycle гейдж), без участия `thr`.
    #[allow(dead_code)] // B2: vestigial — see doc-comment on struct.
    thr: ReconThresholds,
    /// Seed-gate (`docs/fa/ops.md` §4.3.1, третий §8-провал — дефект A, 2026-07-18).
    /// `false` до первой НЕПУСТОЙ local (`best_bid` ИЛИ `best_ask` есть); после первой непустой —
    /// `true` навсегда. До seed `observe` возвращает no-alert и НЕ зовёт `reconcile`
    /// (нет состояния для кормления, но и не тратим цикл на бессмысленный reconcile): «моя книга
    /// ещё не пришла» (orchestrator взял пустую `OrderBook::new()` ДО первого `L2Snapshot`
    /// feeder'а) — это НЕ «биржа испортилась». Пост-seed пустая local — РЕАЛЬНАЯ потеря/порча
    /// → best-путь эмитит немедленно (gate НЕ глушит пост-seed порчу).
    seeded: bool,
}

impl ReconDetector {
    pub fn new(thr: ReconThresholds) -> Self {
        Self { thr, seeded: false }
    }

    /// Скормить один recon-цикл: best-price (per-cycle, immediate) + seed-gate.
    ///
    /// **Seed-gate (первым делом, ДО `reconcile`):** пока `!self.seeded` и `local` пуста
    /// (`best_bid().is_none() && best_ask().is_none()`) — вернуть no-alert, `reconcile` НЕ звать
    /// (нет состояния для кормления). На первой непустой local — выставить `self.seeded = true`
    /// и продолжить обычный путь. Пост-seed пустая local — РЕАЛЬНАЯ потеря/порча, идёт через
    /// обычный best-путь (gate НЕ over-suppress'ит; тест `runtime_post_seed_empty_local_still_emits`).
    ///
    /// **Решение об эмиссии под B2:** `alert = per_cycle.best_price_diverged`. Никаких окон,
    /// никаких объёмных порогов, никакой «магнитуды» — только REST-верифицируемая величина
    /// (`docs/fa/ops.md` §4.3.2). Персистентный объёмный сдвиг (даже ≫ ε_max), within-reach
    /// эвикция НЕ-best уровня, churn — всё МОЛЧИТ в рантайме (объём → офлайн-трек research-dev).
    ///
    /// Детерминирована: `seeded` — чистое рантайм-состояние по последовательности наблюдений,
    /// без wall-clock/rand; одинаковая последовательность наблюдений → одинаковая последовательность
    /// вердиктов.
    ///
    /// **Контракт (RED `crates/ops/tests/red_recon_runtime.rs`, sacred, B2):**
    /// - рантайм эмитит ⟺ `best_price_diverged` (post-seed пустая local / пропал best / десинк
    ///   >`BEST_SKEW_BPS`);
    /// - персистентный объёмный сдвиг (≫ ε_max), within-reach НЕ-best эвикция, churn —
    ///   ТИШИНА (объёмная сверка снята из рантайма, REST-неверифицируема);
    /// - pre-seed пустая local МОЛЧИТ И НЕ КОРМИТ состояние (`pre_seed_empty_does_not_poison_state`).
    pub fn observe(&mut self, local: &OrderBook, reference: &OrderBook) -> ReconVerdict {
        // (0) Seed-gate: до первой НЕПУСТОЙ local детектор молчит и НЕ зовёт reconcile.
        //     «Моя книга ещё не пришла» (orchestrator взял `OrderBook::new()` ДО первого
        //     `L2Snapshot` books-feeder'а) ≠ «биржа испортилась». Тест
        //     `pre_seed_empty_does_not_poison_state` (critic C-012, 9c) пиннит «не кормит
        //     состояние»: под B2 состояние = `seeded`, и pre-seed пустышки НЕ должны были
        //     отравить его (путь через `reconcile()` бы сравнил пустую local с полным reference
        //     и положил `best_price_diverged=true` в `divergence_bps` аудита — но мы их не зовём).
        if !self.seeded {
            let local_empty = local.best_bid().is_none() && local.best_ask().is_none();
            if local_empty {
                return ReconVerdict {
                    alert: false,
                    best_price_diverged: false,
                    divergence_bps: 0,
                };
            }
            // Первая непустая local: фиксируем seed и идём обычным путём (reconcile).
            self.seeded = true;
        }

        // (1) Per-cycle `reconcile` (best-price — immediate; `divergence_bps` — гейдж наблюдаемости).
        let per_cycle = reconcile(local, reference);

        // (2) Под B2 — алерт ⟺ best-price расхождение. Оконная/объёмная ветки СНЯТЫ.
        ReconVerdict {
            alert: per_cycle.best_price_diverged,
            best_price_diverged: per_cycle.best_price_diverged,
            divergence_bps: per_cycle.divergence_bps,
        }
    }

    /// Аудит-событие (CT-RFC-03) из вердикта. Под B2: `divergence_bps` = per-cycle гейдж
    /// (`reconcile().divergence_bps`), `best_price_diverged` = как в вердикте. T1 форма НЕ
    /// меняется (`best=true` + `divergence_bps=per-cycle`; объёмной ветки в рантайме больше нет).
    pub fn verdict_to_audit(
        v: &ReconVerdict,
        venue: Venue,
        symbol: &str,
        action: ReconAction,
    ) -> ReconAudit {
        ReconAudit {
            venue,
            symbol: symbol.to_string(),
            divergence_bps: v.divergence_bps,
            best_price_diverged: v.best_price_diverged,
            action,
        }
    }
}
