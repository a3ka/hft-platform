//! DV-I-1..5 — L2Delta lifetime/staleness анализатор (M-32 Q2а).
//!
//! ЦЕЛЬ: доказать/опровергнуть достоверность дальних полос 3-30% БЕЗ эталона глубже 1.3%
//! (Q1 подтвердил: эталон недостижим ни у биржи, ни у вендоров). Прямой измеритель — СЫРОЙ
//! `MdPayload::L2Delta` (BTCUSDT, CT-RFC-04): `size==0` = явный remove от биржи,
//! sequencing (`U/u/pu`) = gap-детекция. Вопрос: дальний уровень получает `size=0`
//! при жизни (ЖИВОЙ) или замерзает до конца окна (ФАНТОМ)?
//!
//! Ключ (то, что shell-notional depth_probe НЕ мог): de-конфаунд resync'а. Уровень,
//! исчезнувший ЧЕРЕЗ sequence-GAP, — CENSORED (fate неизвестен), НЕ отмена и НЕ заморозка.
//!
//! Контракт (sacred в `tests/red_depth_lifetime.rs`):
//!   `analyze(ticks: &[DeltaTick]) -> LifetimeReport`
//!   - чистый редьюсер; трекает mid (running best bid/ask из дельт — raw apply, БЕЗ stale-FSM,
//!     чтобы mid не терялся после gap); атрибуция уровня к полосе — по дистанции от mid ПРИ РОЖДЕНИИ;
//!   - per-price жизненный цикл: born → (explicit size=0 в contiguous окне = cancelled) |
//!     (жив на конце окна, ни разу size=0 = frozen) | (исчез через seq-gap = censored);
//!   - gap-правило (то же, что `book::OrderBook::apply_l2delta`): спот `U==prev.u+1`;
//!     фьючерс `pu==prev.u`;
//!   - вывод детерминирован (BTreeMap-порядок; `bands` отсортированы по (side, lo_bps)).
//!
//! Детерминизм: BTreeMap для стабильной итерации; чистый редьюсер (без wall-clock/rand/I/O).
//! Граница A.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use contracts::{Level, Side};

/// Полосы (bps от mid), симметрично для bid и ask. Совпадают с константами в RED-тестах.
///
/// Семантика интервалов: `[lo, hi)` — левая граница включительно, правая исключительно.
/// Уровень с `|distance_bps| >= 6000` кладётся в последнюю полосу `[3000, 6000)` как
/// `>=3000` (защита от выбросов за структурным потолком reach — `MAX_REL_DIST=±60%`,
/// см. `book::OrderBook::apply_l2delta`).
pub const BANDS_BPS: &[(i64, i64)] = &[
    (0, 150),
    (150, 300),
    (300, 500),
    (500, 800),
    (800, 1500),
    (1500, 3000),
    (3000, 6000),
];

/// Чистый вход анализатора: один L2Delta-тик (минимально достаточная проекция
/// `MdPayload::L2Delta` без `venue`/`symbol` — анализатор инструмент-агностичен).
///
/// `PartialEq` нужен для фикстур в RED-тестах (`mk()`-замыкания). `Eq` НЕ derive'им:
/// `Level` содержит `f64`-нет поля, но транзитивно через `Vec<Level>` собирается —
// контрактный `Level: PartialEq, !Eq` нам не позволяет; в данном случае `Level: PartialEq`
// достаточно для тестовых фикстур.
#[derive(Debug, Clone, PartialEq)]
pub struct DeltaTick {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub prev_final_update_id: Option<u64>,
    /// Присутствует для трейс-совместимости с `MdPayload::L2Delta` (не используется
    /// самим анализатором — все решения принимаются по ordering + sequencing).
    pub ts_exch_ms: i64,
}

/// Per-полоса агрегат по одной стороне.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandReport {
    pub side: Side,
    pub lo_bps: i64,
    pub hi_bps: i64,
    /// Число уровней, рождённых в полосе за окно (size>0 хотя бы раз в contiguous-окне).
    pub born: u64,
    /// Из них: явно отменены биржей (size=0) до конца contiguous-окна.
    pub cancelled: u64,
    /// Из них: дожили до конца contiguous-окна без явного size=0 (= фантом-кандидат).
    pub frozen: u64,
    /// Из них: исчезли через sequence-GAP (= fate неизвестен; не отмена, не заморозка).
    pub censored: u64,
}

impl BandReport {
    /// `cancel_fraction = cancelled / (cancelled + frozen)` — censored ИСКЛЮЧЕНЫ из знаменателя
    /// (M-32 §Инварианты). Если знаменатель 0 — возвращаем `None` (вся полоса — censored
    /// или пустая; долю считать бессмысленно).
    pub fn cancel_fraction(&self) -> Option<f64> {
        let denom = self.cancelled.saturating_add(self.frozen);
        if denom == 0 {
            None
        } else {
            Some(self.cancelled as f64 / denom as f64)
        }
    }
}

/// Итоговый отчёт. `bands` отсортированы по `(side, lo_bps)` — Buy перед Sell
/// (порядок вариантов enum), `lo_bps` по возрастанию; детерминированный порядок.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifetimeReport {
    pub bands: Vec<BandReport>,
    /// Число обнаруженных sequence-разрывов (spot `U != prev.u + 1` / futures `pu != prev.u`).
    pub gaps: u64,
}

impl LifetimeReport {
    /// Найти полосу `(side, lo_bps)`; возвращает `None`, если такой полосы нет в отчёте.
    pub fn band(&self, side: Side, lo_bps: i64) -> Option<&BandReport> {
        self.bands
            .iter()
            .find(|b| b.side == side && b.lo_bps == lo_bps)
    }
}

// ── Состояние редьюсера ──────────────────────────────────────────────────────────────────────────

/// Финальная судьба уровня.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// Уровень действующий в текущем contiguous-окне.
    Alive,
    /// Явный size=0 от биржи в contiguous-окне.
    Cancelled,
    /// Окно кончилось, size=0 не пришёл (= фантом-кандидат, заморозка).
    Frozen,
    /// Исчез через sequence-GAP (fate неизвестен).
    Censored,
}

/// Per-уровень запись: в какой полосе родился + текущий fate. ПЕРСИСТИТСЯ после size=0,
/// чтобы зафиксировать Cancelled-fate — в отличие от running-книги, где уровень удаляется.
#[derive(Debug, Clone, Copy)]
struct LevelState {
    /// Полоса рождения. `None` означает: уровень родился до появления двустороннего
    /// mid и ожидает атрибуции в очереди `unattributed`.
    born_band_lo_bps: Option<i64>,
    fate: Fate,
}

/// Per-side редьюсер. Два независимых отображения:
///   - `book`: running price→size (нужен для mid; удаляется при size=0);
///   - `states`: price→LevelState (НЕ удаляется при size=0 — для фиксации fate).
#[derive(Debug, Default)]
struct SideBook {
    book: BTreeMap<i64, i64>,
    states: BTreeMap<i64, LevelState>,
    /// Только ещё не атрибутированные новорождённые цены. В отличие от `states`, эта
    /// очередь не сканируется целиком каждый тик и обычно опустошается на том же тике.
    unattributed: VecDeque<i64>,
    /// Только живые уровни: gap/finalization не сканируют ever-growing `states`.
    alive: BTreeSet<i64>,
}

impl SideBook {
    /// Применить дельту и поставить новые рождения в O(1)-очередь атрибуции.
    /// Атрибуция выполняется после применения ОБЕИХ сторон тика, когда известен mid
    /// именно этого тика, а не предыдущего.
    fn apply_delta(&mut self, side_levels: &[Level]) {
        for l in side_levels {
            if l.size == 0 {
                self.book.remove(&l.price);
                let was_alive = self.states.get_mut(&l.price).is_some_and(|state| {
                    if state.fate == Fate::Alive {
                        state.fate = Fate::Cancelled;
                        true
                    } else {
                        false
                    }
                });
                if was_alive {
                    self.alive.remove(&l.price);
                }
            } else if l.size > 0 {
                let new_birth = !self.states.contains_key(&l.price);
                self.book.insert(l.price, l.size);
                if new_birth {
                    self.states.insert(
                        l.price,
                        LevelState {
                            born_band_lo_bps: None,
                            fate: Fate::Alive,
                        },
                    );
                    self.unattributed.push_back(l.price);
                    self.alive.insert(l.price);
                }
            }
            // size < 0 невозможен по контракту Level.
        }
    }

    /// Атрибутировать только очередь новорождённых, не весь `states` (DV-I-7).
    fn attribute_newborns(&mut self, mid: i64) {
        while let Some(price) = self.unattributed.pop_front() {
            if let Some(state) = self.states.get_mut(&price) {
                state.born_band_lo_bps = Some(band_for_bps(signed_bps(price, mid).abs()));
            }
        }
    }

    /// При gap: только текущие alive-уровни → Censored. Повторный gap после
    /// fail-closed разрыва — O(1), поскольку `alive` уже пуст.
    fn apply_gap_censor(&mut self) {
        for price in std::mem::take(&mut self.alive) {
            if let Some(state) = self.states.get_mut(&price) {
                state.fate = Fate::Censored;
            }
        }
    }

    /// Конец окна: только всё ещё живые уровни → Frozen.
    fn freeze_remaining(&mut self) {
        for price in std::mem::take(&mut self.alive) {
            if let Some(state) = self.states.get_mut(&price) {
                state.fate = Fate::Frozen;
            }
        }
    }
}

/// Вычислить mid из текущих сторон книги (running). None если хотя бы одна сторона пуста.
fn compute_mid(bids: &SideBook, asks: &SideBook) -> Option<i64> {
    let best_bid = bids.book.keys().next_back().copied();
    let best_ask = asks.book.keys().next().copied();
    match (best_bid, best_ask) {
        (Some(b), Some(a)) => Some((b + a) / 2),
        _ => None,
    }
}

/// Расстояние от mid до цены в базисных пунктах: `bps = (price - mid) * 10_000 / mid`.
/// Знак сохраняется: bid (price<mid) → отрицательное; ask (price>mid) → положительное.
/// Возвращает `0` если `mid <= 0` (защита от деления на ноль; на реальных ценах BTC не
/// достигается).
#[inline]
fn signed_bps(price: i64, mid: i64) -> i64 {
    if mid <= 0 {
        return 0;
    }
    let diff = price - mid;
    diff.saturating_mul(10_000).checked_div(mid).unwrap_or(0)
}

/// Определить полосу (lo_bps) по `|signed_bps|`. Полосы — `[lo, hi)`.
/// `|bps| >= 6000` (за пределами схемы / за потолком reach) → последняя полоса
/// `[3000, 6000)` как `>=3000` (защита от выбросов книжной реализации).
fn band_for_bps(abs_bps: i64) -> i64 {
    for &(lo, hi) in BANDS_BPS {
        if abs_bps >= lo && abs_bps < hi {
            return lo;
        }
    }
    BANDS_BPS.last().map(|&(lo, _)| lo).unwrap_or(1500)
}

/// Gap-детекция. Возвращает `true`, если continuity нарушена.
///
/// Правило (как `book::OrderBook::apply_l2delta`):
///   - bootstrap (`prev_final == None`): пропускаем (первая дельта — принимаем как есть);
///   - спот (`prev_final_update_id == None` ⇒ спот-поток): `first_update_id == prev.u + 1`;
///   - фьючерс (`prev_final_update_id == Some(pu)`): `pu == prev.u`.
fn is_gap(prev: Option<u64>, t: &DeltaTick) -> bool {
    let prev_u = match prev {
        None => return false, // bootstrap
        Some(p) => p,
    };
    match t.prev_final_update_id {
        None => t.first_update_id != prev_u.saturating_add(1),
        Some(pu) => pu != prev_u,
    }
}

/// Сортировка по `(side, lo_bps)`: Buy (вариант 0) перед Sell (вариант 1), внутри стороны —
/// `lo_bps` по возрастанию. Используется для детерминированного порядка `bands`.
/// Match вместо `as u8 as i64` — без неявных кастов, явно.
fn side_rank(side: Side) -> i64 {
    match side {
        Side::Buy => 0,
        Side::Sell => 1,
    }
}

/// Главный анализатор. Чистый редьюсер; один и тот же `ticks` → идентичный `LifetimeReport`.
///
/// Сложность: O(N log S + S), где N = число входных уровней, S = число distinct-цен.
/// Каждый уровень попадает в `unattributed` ровно один раз; ever-growing `states`
/// сканируется только при финальной агрегации, но никогда per-tick (DV-I-7).
pub fn analyze(ticks: &[DeltaTick]) -> LifetimeReport {
    let mut bids = SideBook::default();
    let mut asks = SideBook::default();
    let mut prev_final_update_id: Option<u64> = None;
    let mut gaps: u64 = 0;

    for t in ticks {
        let gap = is_gap(prev_final_update_id, t);
        if gap {
            gaps += 1;
            // Цензурируем все alive-уровни обеих сторон (fate неизвестен после разрыва
            // continuity). Дельту НЕ применяем (fail-closed).
            bids.apply_gap_censor();
            asks.apply_gap_censor();
            // prev_final_update_id НЕ обновляется при gap (fail-closed: stale-книга
            // отвергает всё до ресинка — тот же принцип, что в book::OrderBook).
        } else {
            // Сначала применяем обе стороны. Затем mid отражает ИМЕННО этот тик, и
            // атрибутируется только очередь новорождённых (обычно несколько цен).
            bids.apply_delta(&t.bids);
            asks.apply_delta(&t.asks);
            if let Some(mid) = compute_mid(&bids, &asks) {
                bids.attribute_newborns(mid);
                asks.attribute_newborns(mid);
            }
            prev_final_update_id = Some(t.final_update_id);
        }
    }

    // Конец окна: всё, что ещё Alive → Frozen.
    bids.freeze_remaining();
    asks.freeze_remaining();

    // Агрегация per-band: born/cancelled/frozen/censored.
    // Ключ — (side_rank, lo_bps): `Side` не реализует `Ord`, поэтому используем `i64` ранг.
    let mut aggregates: BTreeMap<(i64, i64), BandAggregate> = BTreeMap::new();
    for (lo, _hi) in BANDS_BPS {
        for side in [Side::Buy, Side::Sell] {
            aggregates.insert((side_rank(side), *lo), (side, 0, 0, 0, 0));
        }
    }

    for state in bids.states.values() {
        // Вырожденный односторонний поток может так и не получить mid; такие рождения
        // детерминированно попадают в первую полосу.
        let lo = state.born_band_lo_bps.unwrap_or(BANDS_BPS[0].0);
        let entry = aggregates
            .get_mut(&(side_rank(Side::Buy), lo))
            .expect("полоса pre-init");
        bump_fate_with_side(entry, Side::Buy, state.fate);
    }
    for state in asks.states.values() {
        let lo = state.born_band_lo_bps.unwrap_or(BANDS_BPS[0].0);
        let entry = aggregates
            .get_mut(&(side_rank(Side::Sell), lo))
            .expect("полоса pre-init");
        bump_fate_with_side(entry, Side::Sell, state.fate);
    }

    // Строим bands в детерминированном порядке (side_rank, lo_bps).
    let mut bands: Vec<BandReport> = aggregates
        .into_iter()
        .map(|(_key, (side, born, cancelled, frozen, censored))| {
            let lo = _key.1;
            let hi = BANDS_BPS
                .iter()
                .find(|(l, _)| *l == lo)
                .map(|(_, h)| *h)
                .unwrap_or(6000);
            BandReport {
                side,
                lo_bps: lo,
                hi_bps: hi,
                born,
                cancelled,
                frozen,
                censored,
            }
        })
        .collect();
    bands.sort_by_key(|b| (side_rank(b.side), b.lo_bps));
    LifetimeReport { bands, gaps }
}

/// Per-band агрегат: `(side, born, cancelled, frozen, censored)`.
type BandAggregate = (Side, u64, u64, u64, u64);

/// Инкрементить per-band агрегат по fate уровня (`side` остаётся неизменным — это метка
/// полосы, не уровня).
fn bump_fate_with_side(slot: &mut BandAggregate, side: Side, fate: Fate) {
    let (_, born, cancelled, frozen, censored) = *slot;
    *slot = (
        side,
        born + 1,
        cancelled + u64::from(fate == Fate::Cancelled),
        frozen + u64::from(fate == Fate::Frozen),
        censored + u64::from(fate == Fate::Censored),
    );
}
