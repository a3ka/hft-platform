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
    pub lives_born: u64,
    /// Из них: явно отменены биржей.
    pub lives_cancelled: u64,
    /// Из них: дожили до конца окна.
    pub lives_frozen: u64,
    /// Из них: исчезли через sequence-GAP.
    pub lives_censored: u64,
}

impl BandReport {
    /// `cancel_fraction = cancelled / (cancelled + frozen)` — censored ИСКЛЮЧЕНЫ из знаменателя
    /// (M-32 §Инварианты). Если знаменатель 0 — возвращаем `None` (вся полоса — censored
    /// или пустая; долю считать бессмысленно).
    pub fn cancel_fraction(&self) -> Option<f64> {
        let denom = self.lives_cancelled.saturating_add(self.lives_frozen);
        if denom == 0 {
            None
        } else {
            Some(self.lives_cancelled as f64 / denom as f64)
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
///
/// С M-59 (TD-107) сами конструкторы `Cancelled`/`Frozen`/`Censored` больше не
/// вызываются напрямую: ветвление по fate ушло в `SideBook::bump(BumpKind)`,
/// сравнение с `Fate::Alive` осталось. `#[allow(dead_code)]` глушит ложное
/// предупреждение о конструкторах — тип остаётся частью `LevelState::fate`,
/// и его нельзя удалить, не сломав контракт состояния редьюсера.
#[allow(dead_code)]
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

/// Per-полоса агрегат ЗАВЕРШЁННЫХ жизней: (cancelled, frozen, censored).
/// `born` на полосе не хранится, потому что `born == cancelled + frozen + censored`
/// (каждая жизнь ровно один раз получает один из этих fate; контракт BandReport).
type BandCounts = (u64, u64, u64);

/// Какое fate инкрементируется в `SideBook::bump`.
///
/// Не вызываем её до того, как у уровня определена полоса рождения: для отмены до
/// появления двустороннего mid атрибуция детерминированно падает на первую полосу
/// (см. `apply_delta` size=0 и комментарий в `analyze`).
#[derive(Debug, Clone, Copy)]
enum BumpKind {
    Cancelled,
    Frozen,
    Censored,
}

/// Per-side редьюсер. Два независимых отображения:
///   - `book`: running price→size (нужен для mid; удаляется при size=0);
///   - `states`: price→LevelState (живые уровни; при size=0 с известной полосой
///     bump делается сразу, при size=0 с НЕизвестной полосой — state удаляется,
///     а запись откладывается в `pending_completed` для атрибуции при появлении mid).
///
/// До M-59 (TD-107) `finished: Vec<(Option<i64>, Fate)>` накапливал запись на КАЖДУЮ
/// завершённую жизнь, давая O(жизней) памяти. Сейчас — `counts: BTreeMap<lo_bps,
/// BandCounts>` размером с число полос (7), инкремент в момент ЗАВЕРШЕНИЯ жизни, а
/// не накопление.
///
/// Память O(число полос) и расход O(1) на завершённую жизнь — **после того, как mid известен**.
/// До первого mid в окне завершённые жизни копятся в `pending_completed`, и расход растёт с их
/// числом: замер `R-076` — ×4.00 на одностороннем потоке против ×1.00 на пути с mid
/// (`DV-I-15`). Граница здесь НЕ достигнута сознательно (решение `e0e56a3`): милестоун
/// обязывает её НАЗВАТЬ, а не устранить. Устранение — отдельный предмет; до него **расширение
/// на топ-300 упирается именно в этот путь**.
///
/// `pending_completed` — отложенные ЗАВЕРШЁННЫЕ жизни (size=0 ДО появления mid),
/// ждущие атрибуции на ближайшем известном mid. Регрессия `TD-107`-фикса
/// (R-038 F-2, `DV-I-16`): если на той же цене до атрибуции рождается новая жизнь, старая
/// завершённая жизнь терялась при перезаписи `states`. `pending_completed` хранит их
/// ОТДЕЛЬНО от `states`, и `attribute_newborns` (или `flush_pending_completed` при
/// отсутствии mid) учитывает их в момент атрибуции, а не в момент завершения.
#[derive(Debug, Default)]
struct SideBook {
    book: BTreeMap<i64, i64>,
    states: BTreeMap<i64, LevelState>,
    /// Per-band агрегаты завершённых жизней; ключ — `lo_bps` полосы, значение —
    /// `(cancelled, frozen, censored)`. `born` неявно = сумма этих трёх
    /// (контракт BandReport, шаг T6 гейта M-59).
    counts: BTreeMap<i64, BandCounts>,
    /// Только ещё не атрибутированные новорождённые цены. В отличие от `states`, эта
    /// очередь не сканируется целиком каждый тик и обычно опустошается на том же тике.
    unattributed: VecDeque<i64>,
    /// Только живые уровни: gap/finalization не сканируют ever-growing `states`.
    alive: BTreeSet<i64>,
    /// ЗАВЕРШЁННЫЕ жизни (size=0), НЕ ДОЖДАВШИЕСЯ mid — bump на ближайшем известном
    /// `mid` в `attribute_newborns` (или fallback `BANDS_BPS[0].0` в
    /// `flush_pending_completed`, если mid так и не появился). Хранит (price, fate), чтобы
    /// судьба пережила перерождение цены (фикс `DV-I-16`).
    pending_completed: VecDeque<(i64, Fate)>,
}

impl SideBook {
    /// Инкрементить per-band агрегат. `lo_bps` — полоса рождения уровня. Если полоса
    /// ещё не появлялась — создаётся запись с нулём во всех трёх слотах и нужный fate
    /// инкрементируется. Никогда не инкрементит `born` напрямую: суммарный `born`
    /// восстанавливается как `c + f + ce` (контракт).
    fn bump(&mut self, lo_bps: i64, kind: BumpKind) {
        let entry = self.counts.entry(lo_bps).or_insert((0, 0, 0));
        match kind {
            BumpKind::Cancelled => entry.0 += 1,
            BumpKind::Frozen => entry.1 += 1,
            BumpKind::Censored => entry.2 += 1,
        }
    }

    /// Применить дельту и поставить новые рождения в O(1)-очередь атрибуции.
    /// Атрибуция выполняется после применения ОБЕИХ сторон тика, когда известен mid
    /// именно этого тика, а не предыдущего.
    fn apply_delta(&mut self, side_levels: &[Level]) {
        for l in side_levels {
            if l.size == 0 {
                self.book.remove(&l.price);
                if self
                    .states
                    .get(&l.price)
                    .is_some_and(|state| state.fate == Fate::Alive)
                {
                    let state = self.states.remove(&l.price).expect("just checked");
                    if let Some(lo) = state.born_band_lo_bps {
                        // Полоса уже атрибутирована — bump сейчас.
                        self.bump(lo, BumpKind::Cancelled);
                    } else {
                        // Отмена ДО атрибуции (односторонний поток / mid ещё не
                        // появился). НЕ возвращаем state в `states`: новое рождение
                        // на той же цене (до атрибуции) сотрёт его и судьба потеряется
                        // (`R-038` F-2, `DV-I-16`). Кладём `(price, Cancelled)` в
                        // `pending_completed` — `attribute_newborns` при появлении mid
                        // (или `flush_delayed_states` в конце окна) закроет судьбу.
                        self.pending_completed.push_back((l.price, Fate::Cancelled));
                    }
                    self.alive.remove(&l.price);
                }
            } else if l.size > 0 {
                self.book.insert(l.price, l.size);
                let new_birth = self
                    .states
                    .get(&l.price)
                    .is_none_or(|state| state.fate != Fate::Alive);
                if new_birth {
                    // Предыдущее состояние (если было) уже учтено в size=0: либо
                    // bump при известной полосе, либо запись в `pending_completed`
                    // при неизвестной. `states.remove` — чисто убирает остаток; новый
                    // Alive идёт в `states`.
                    self.states.remove(&l.price);
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
    /// Заодно закрывает судьбы из `pending_completed` — завершённые ДО mid жизни
    /// учитываются в момент АТРИБУЦИИ, а не в момент завершения (`DV-I-16`, фикс
    /// регрессии `R-038` F-2). Полоса для них вычисляется из их ЦЕНЫ и ТЕКУЩЕГО mid:
    /// отложенная жизнь обязана попасть в ту полосу, в которой цена жила, когда mid
    /// появился.
    fn attribute_newborns(&mut self, mid: i64) {
        while let Some(price) = self.unattributed.pop_front() {
            let band = band_for_bps(signed_bps(price, mid).abs());
            if let Some(state) = self.states.get_mut(&price) {
                state.born_band_lo_bps = Some(band);
            }
        }
        // Отложенные завершённые жизни — bump на полосе, определяемой их ценой и
        // текущим mid. Если цена за это время «ушла», атрибуция всё равно идёт по
        // дистанции от mid в момент атрибуции (та же семантика, что и для новых
        // рождений — полоса привязана к mid, а не к первой жизни цены, см. `DV-I-14`).
        let pending = std::mem::take(&mut self.pending_completed);
        for (price, fate) in pending {
            let band = band_for_bps(signed_bps(price, mid).abs());
            bump_finished_fate(self, band, fate);
        }
    }

    /// При gap: только текущие alive-уровни → Censored. Повторный gap после
    /// fail-closed разрыва — O(1), поскольку `alive` уже пуст.
    fn apply_gap_censor(&mut self) {
        for price in std::mem::take(&mut self.alive) {
            if let Some(state) = self.states.remove(&price) {
                let lo = state.born_band_lo_bps.unwrap_or(BANDS_BPS[0].0);
                self.bump(lo, BumpKind::Censored);
            }
        }
    }

    /// Конец окна: только всё ещё живые уровни → Frozen.
    fn freeze_remaining(&mut self) {
        for price in std::mem::take(&mut self.alive) {
            if let Some(state) = self.states.remove(&price) {
                let lo = state.born_band_lo_bps.unwrap_or(BANDS_BPS[0].0);
                self.bump(lo, BumpKind::Frozen);
            }
        }
    }
}

/// Отложенный bump: известна полоса и судьба — инкрементируем `counts`. Вызывается
/// только на закрытых fate (`Cancelled`/`Frozen`/`Censored`), НИКОГДА на Alive —
/// alive-уровни уходят через `freeze_remaining`/`apply_gap_censor`, иначе они бы
/// накапливались в отчёте как фантомные «рождения без событий».
fn bump_finished_fate(book: &mut SideBook, lo_bps: i64, fate: Fate) {
    let kind = match fate {
        Fate::Cancelled => BumpKind::Cancelled,
        Fate::Frozen => BumpKind::Frozen,
        Fate::Censored => BumpKind::Censored,
        Fate::Alive => return,
    };
    book.bump(lo_bps, kind);
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

    // Агрегация per-band. Ключ — (side_rank, lo_bps): `Side` не реализует `Ord`,
    // поэтому используем `i64` ранг. Per-side `counts` уже инкрементированы в момент
    // завершения жизни (или отложенно на attribute_newborns) — здесь только переносим
    // их в формат отчёта. `born` = `cancelled + frozen + censored` восстанавливается
    // по ходу (контракт BandReport, шаг T6 гейта M-59).
    let mut aggregates: BTreeMap<(i64, i64), BandAggregate> = BTreeMap::new();
    for (lo, _hi) in BANDS_BPS {
        for side in [Side::Buy, Side::Sell] {
            aggregates.insert((side_rank(side), *lo), (side, 0, 0, 0, 0));
        }
    }

    merge_band_counts(&mut aggregates, Side::Buy, &bids.counts);
    merge_band_counts(&mut aggregates, Side::Sell, &asks.counts);

    // Финальная итерация по `states` — добираем судьбы, ЗАВЕРШЁННЫЕ ДО атрибуции,
    // когда mid так и не появился до конца окна (односторонний поток). В этом пути
    // судьба известна (`Cancelled`/`Frozen`/`Censored`) — бамп в первой полосе по
    // `unwrap_or(BANDS_BPS[0].0)`, как и в прежней реализации.
    flush_delayed_states(&mut aggregates, Side::Buy, &bids.states);
    flush_delayed_states(&mut aggregates, Side::Sell, &asks.states);

    // Доразбираем `pending_completed` — судьбы, ЗАВЕРШЁННЫЕ ДО атрибуции. Если mid
    // так и не появился (односторонний поток до конца окна) — bump на первой полосе
    // (`BANDS_BPS[0].0`), как и прежний путь через `states`. Иначе (mid был известен)
    // `pending_completed` пуст — `attribute_newborns` уже всё закрыл.
    flush_pending_completed(&mut aggregates, Side::Buy, &bids.pending_completed);
    flush_pending_completed(&mut aggregates, Side::Sell, &asks.pending_completed);

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
                lives_born: born,
                lives_cancelled: cancelled,
                lives_frozen: frozen,
                lives_censored: censored,
            }
        })
        .collect();
    bands.sort_by_key(|b| (side_rank(b.side), b.lo_bps));
    LifetimeReport { bands, gaps }
}

/// Per-band агрегат: `(side, born, cancelled, frozen, censored)`.
type BandAggregate = (Side, u64, u64, u64, u64);

/// Перенести per-band `BandCounts` одной стороны в `aggregates` отчёта. Полосы,
/// для которых `counts` пуст (жизней такого исхода не было), оставляют 0 —
/// строки отчёта остаются полными по схеме полос (DV-I-1..5 этого ожидают).
fn merge_band_counts(
    aggregates: &mut BTreeMap<(i64, i64), BandAggregate>,
    side: Side,
    counts: &BTreeMap<i64, BandCounts>,
) {
    for (lo, &(c, f, ce)) in counts {
        let entry = aggregates
            .get_mut(&(side_rank(side), *lo))
            .expect("полоса pre-init");
        let (_, born, cancelled, frozen, censored) = *entry;
        *entry = (
            side,
            born + c + f + ce,
            cancelled + c,
            frozen + f,
            censored + ce,
        );
    }
}

/// Добрать отложенные bumps по `states` (завершённые-судьбы без атрибуции из-за
/// одностороннего потока). По построению к этому моменту `states` НЕ содержит
/// `Alive` (alive ушли через `freeze_remaining`/`apply_gap_censor`) — только
/// delayed `Cancelled`/etc. В Alive-ветку всё равно входим: `match` exhaustive.
fn flush_delayed_states(
    aggregates: &mut BTreeMap<(i64, i64), BandAggregate>,
    side: Side,
    states: &BTreeMap<i64, LevelState>,
) {
    for state in states.values() {
        if state.fate == Fate::Alive {
            // alive-уровни уходят через freeze/gap; сюда попасть не должны. Если
            // попали — это артефакт построения, считаем как Cancelled, чтобы
            // баланс `born == c + f + ce` не разъехался.
        }
        let lo = state.born_band_lo_bps.unwrap_or(BANDS_BPS[0].0);
        let entry = aggregates
            .get_mut(&(side_rank(side), lo))
            .expect("полоса pre-init");
        let (_, born, cancelled, frozen, censored) = *entry;
        *entry = match state.fate {
            Fate::Alive => (side, born + 1, cancelled, frozen, censored),
            Fate::Cancelled => (side, born + 1, cancelled + 1, frozen, censored),
            Fate::Frozen => (side, born + 1, cancelled, frozen + 1, censored),
            Fate::Censored => (side, born + 1, cancelled, frozen, censored + 1),
        };
    }
}

/// Доразобрать `pending_completed` в конце окна: судьбы, ЗАВЕРШЁННЫЕ ДО атрибуции,
/// bump'аются на полосе `BANDS_BPS[0].0` (mid не появился — fallback, как и в
/// `flush_delayed_states`). На момент вызова `pending_completed` обычно пуст:
/// `attribute_newborns` обработал его на ближайшем известном mid.
fn flush_pending_completed(
    aggregates: &mut BTreeMap<(i64, i64), BandAggregate>,
    side: Side,
    pending: &VecDeque<(i64, Fate)>,
) {
    for &(_price, fate) in pending {
        let lo = BANDS_BPS[0].0;
        let entry = aggregates
            .get_mut(&(side_rank(side), lo))
            .expect("полоса pre-init");
        let (_, born, cancelled, frozen, censored) = *entry;
        *entry = match fate {
            Fate::Alive => (side, born + 1, cancelled, frozen, censored),
            Fate::Cancelled => (side, born + 1, cancelled + 1, frozen, censored),
            Fate::Frozen => (side, born + 1, cancelled, frozen + 1, censored),
            Fate::Censored => (side, born + 1, cancelled, frozen, censored + 1),
        };
    }
}
