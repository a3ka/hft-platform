//! grid — оркестрация перебора (FA §5): для КАЖДОЙ ячейки — инстанс сигнала →
//! настоящий strategy-пайплайн → sim fills/PnL → ЗАПИСЬ В LEDGER независимо от
//! исхода и попадания в top-K (RC-I-9/§5).
//!
//! M-07 D7/D8: один и тот же `DirectionalStrategy` исполняется в backtest и будущем
//! live; returns берутся из mark-to-market equity `StrategyBacktest`, а хэш ячейки
//! покрывает signal+strategy+costs. Стресс-режимы остаются отдельными sim-прогонами
//! со scaled-таблицами (RC-I-10). Отказ ledger-записи abort'ит весь grid (FA §3).
//!
//! M-08 E5/E6 (задача 5): прод-путь — `run_grid_streamed` через `journal::stream` +
//! `EpochFilter`. Память O(1) по размеру журнала: журнал на 8.3 GB больше НЕ
//! материализуется в `Vec<Event>` (класс TD-011, этажом выше). `run_grid` остаётся
//! для сравнительных/малых фикстур (red_research и т.п.).

use std::collections::{BTreeMap, BTreeSet};

use alpha::{Instrument, LinearAlpha, SignalWeight, EDGE_SCALE};
use book::Books;
use contracts::{Event, EventKind, Side};
use portfolio::RiskBudget;
use sha2::{Digest, Sha256};
use signals::obi::{Obi, ObiParams};
use signals::{RegistryStatus, SignalId};
use sim::{BacktestExchange, FeeSchedule, LatencyTable, SimFill, StrategyBacktest};
use strategy::{DirectionalStrategy, FillReport, Strategy, StrategyConfig};

use crate::ledger::Ledger;
use crate::metrics;
use crate::split::ValGateToken;
use crate::strategy_cell::{
    capital_ref_e8, cell_params_hash, returns_from_equity, strategy_cell_config, StrategyCellConfig,
};
use crate::types::{
    CellResult, CostsMode, GridSpec, RcError, SplitKind, TrialRecord, TRIALS_LEDGER_SCHEMA_VERSION,
};

/// Эпоха семантики strategy-grid. Она является частью хэша TrialRecord и отделяет
/// post-M-07 equity-кривую от четырёх исторических записей старого harness'а
/// (TD-015). Это не хэш файла сигналов: сам сигнал неизменен, изменилась семантика
/// исполнения через DirectionalStrategy/StrategyBacktest.
pub const LEDGER_EPOCH_CUTOFF: &str = "5141fd9";

/// Аннуализация Sharpe внутри грида (сравнение ячеек между собой; не путать с
/// итоговым отчётным Sharpe research/reports — там периодичность явная).
const ANNUALIZATION_PERIODS_PER_YEAR: f64 = 252.0;

/// Окружение прогона: ledger (единственная точка записи) + честные таблицы sim.
pub struct GridRunEnv<'a> {
    pub ledger: &'a mut Ledger,
    pub latency: &'a LatencyTable,
    pub fees: &'a FeeSchedule,
}

/// sha256 исходника сигнала плюс маркер семантической эпохи (D3/TD-015).
///
/// Исторические pre-M-07 trial records несут голый sha256 `obi.rs` (его префикс
/// `f7f4761`). После миграции на настоящий strategy-пайплайн такой хэш был бы
/// неразличим с legacy-записями, поэтому в post-M-07 хэш включает фиксированный
/// epoch marker. Идентичные исходы в одинаковой эпохе дают идентичный hash.
pub fn research_code_hash() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../signals/src/obi.rs");
    let bytes = std::fs::read(&path).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(b"research-cli-strategy-grid:");
    hasher.update(LEDGER_EPOCH_CUTOFF.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Первый наблюдённый mid именно инструмента ячейки. Реконструкция идёт только вперёд
/// по переданному сегменту; отсутствие двусторонней книги даёт 0 (D7 fail-closed).
fn first_mid_e8(events: &[Event], instrument: &Instrument) -> i64 {
    let mut books = Books::new();
    for event in events {
        if let EventKind::Md(md) = &event.kind {
            books.apply(md);
            if md.venue == instrument.venue && md.symbol == instrument.symbol {
                if let Some(mid) = books
                    .get(instrument.venue, &instrument.symbol)
                    .and_then(|book| book.mid())
                {
                    return mid;
                }
            }
        }
    }
    0
}

#[allow(clippy::too_many_arguments)]
fn run_one_cell(
    events_in_range: &[Event],
    obi_params: &ObiParams,
    signal_id: SignalId,
    signal: Obi,
    latency: LatencyTable,
    fees: FeeSchedule,
    seed: u64,
    params: serde_json::Value,
    params_hash: String,
) -> Result<CellResult, RcError> {
    let cell_config = strategy_cell_config(&params)?;
    let instrument = Instrument::new(obi_params.venue, obi_params.symbol.clone());
    let initial_mid_e8 = first_mid_e8(events_in_range, &instrument);

    // Один OBI — вырожденный случай того же LinearAlpha, что и будущий ансамбль:
    // отдельного «односигнального» решения в research harness больше нет.
    let alpha = LinearAlpha::new(vec![SignalWeight {
        signal_id,
        instrument: instrument.clone(),
        weight_e8: EDGE_SCALE,
    }])
    .map_err(|error| RcError::Signal(format!("alpha config: {error:?}")))?;
    let budget = RiskBudget::new(vec![(instrument, cell_config.max_position_e8)])
        .map_err(|error| RcError::Signal(format!("portfolio config: {error:?}")))?;
    let strategy_config = StrategyConfig {
        min_order_e8: cell_config.min_order_e8,
        intent_ttl_ms: cell_config.intent_ttl_ms,
        marketable_margin_bp: cell_config.marketable_margin_bp,
        kind: cell_config.order_kind()?,
    };
    let mut strategy = DirectionalStrategy::new(
        vec![Box::new(signal)],
        Box::new(alpha),
        budget,
        strategy_config,
    )
    .map_err(|error| RcError::Signal(format!("strategy config: {error:?}")))?;

    let mut backtest = StrategyBacktest::new(latency, fees, seed);
    let report = backtest.run(events_in_range, &mut strategy);

    let capital_ref = capital_ref_e8(cell_config.max_position_e8, initial_mid_e8);
    let returns = returns_from_equity(&report.equity_curve_e8, capital_ref);
    let sharpe = metrics::sharpe(&returns, ANNUALIZATION_PERIODS_PER_YEAR);
    let max_drawdown_e8 = metrics::max_drawdown_e8(&report.equity_curve_e8);
    // Денежный PnL открытой directional-позиции — последняя mark-to-market equity,
    // а не cash (cash сам по себе включает стоимость незакрытого инвентаря).
    let net_pnl_e8 = report.equity_curve_e8.last().copied().unwrap_or(0);

    Ok(CellResult {
        params,
        params_hash,
        net_pnl_e8,
        sharpe,
        max_drawdown_e8,
        intents: report.intents,
        fills: report.fills.len(),
        turnover_e8: report.turnover_e8,
        returns,
    })
}

/// Прогнать грид над событиями диапазона range_ms (полуинтервал [from, to) по ts_wall_ms).
/// Для SplitKind::Test ОБЯЗАТЕЛЕН &ValGateToken (RC-I-8): Test без токена →
/// Err::GateDenied; для Train/Val — test_proof = None.
///
/// Для КАЖДОЙ ячейки — обязательная запись в trials-ledger (FA §5/RC-I-9), НЕЗАВИСИМО
/// от исхода; отказ записи → abort ВСЕГО прогона (Err::LedgerWrite, FA §3).
pub fn run_grid(
    events: &[Event],
    spec: &GridSpec,
    split: SplitKind,
    range_ms: (i64, i64),
    env: &mut GridRunEnv<'_>,
    test_proof: Option<&ValGateToken>,
) -> Result<Vec<CellResult>, RcError> {
    if split == SplitKind::Test && test_proof.is_none() {
        return Err(RcError::GateDenied(
            "SplitKind::Test требует &ValGateToken (RC-I-8) — val-гейт не пройден".into(),
        ));
    }

    let events_in_range: Vec<Event> = events
        .iter()
        .filter(|event| event.ts_wall_ms >= range_ms.0 && event.ts_wall_ms < range_ms.1)
        .cloned()
        .collect();
    let ts_wall_ms = events_in_range
        .last()
        .map(|event| event.ts_wall_ms)
        .unwrap_or(range_ms.0);

    // D11: грид инстанцирует OBI напрямую (единственный сигнал M-04/M-07).
    let signal_id_str = format!("{}-obi-asym", spec.signal_id_prefix);
    let code_hash = research_code_hash();
    let mut results = Vec::with_capacity(spec.cells.len());

    for cell in &spec.cells {
        // Fail-closed до запуска sim: кривой strategy-блок не заменяется дефолтом.
        strategy_cell_config(cell)?;
        let params_hash = cell_params_hash(cell, spec.costs_mode);
        let signal_id = SignalId::parse(&signal_id_str)
            .map_err(|error| RcError::Signal(format!("{error:?}")))?;
        let signal = Obi::from_json_params(signal_id.clone(), 1, RegistryStatus::Candidate, cell)
            .map_err(|error| RcError::Signal(format!("{error:?}")))?;
        let obi_params: ObiParams = serde_json::from_value(cell.clone())
            .map_err(|error| RcError::Parse(error.to_string()))?;

        // Стресс-режимы — ОТДЕЛЬНЫЕ прогоны через ту же модель sim, не пост-обработка
        // готовых чисел (RC-I-10): scaled() строит собственную честную таблицу.
        let latency = match spec.costs_mode {
            CostsMode::LatencyX2 => env.latency.scaled(2.0),
            _ => env.latency.scaled(1.0),
        };
        let fees = match spec.costs_mode {
            CostsMode::CostX15 => env.fees.scaled(1.5),
            _ => env.fees.scaled(1.0),
        };

        let cell_result = run_one_cell(
            &events_in_range,
            &obi_params,
            signal_id,
            signal,
            latency,
            fees,
            spec.seed,
            cell.clone(),
            params_hash.clone(),
        )?;

        env.ledger
            .append(TrialRecord {
                schema_version: TRIALS_LEDGER_SCHEMA_VERSION,
                signal_family: spec.signal_family.clone(),
                signal_id: signal_id_str.clone(),
                params_hash,
                split,
                costs_mode: spec.costs_mode,
                ts_wall_ms,
                code_hash: code_hash.clone(),
                result_ref: format!("cell-{}", results.len()),
                sharpe: Some(cell_result.sharpe),
                prev_sha256: String::new(),
            })
            .map_err(|error| RcError::LedgerWrite(error.to_string()))?;

        results.push(cell_result);
    }

    Ok(results)
}

/// Источник событий для грида на ПРОД-МАСШТАБЕ (M-08 E5/E6, carve-out A3).
///
/// Каталог журнала + ЯВНО названная эпоха. Грид открывает свежий bounded-memory стрим
/// на КАЖДУЮ ячейку — журнал (8.3 GB и растёт) в память не помещается и помещаться не должен.
pub struct JournalSource {
    pub dir: std::path::PathBuf,
    /// Эпоху нельзя не назвать (CT-RFC02-2): вендорские данные не подмешиваются молча.
    pub filter: journal::EpochFilter,
}

/// Прогон одной ячейки через стрим (E5) — держит state per-cell без `&[Event]`.
///
/// `sim::StrategyBacktest::run(&[Event], ...)` — единственный публичный путь прогона,
/// но он материализует срез. Для стрима мы реплицируем ровно тело `run` event-by-event
/// в `feed()`. Семантика гарантированно идентична in-memory пути — оракул
/// `tests/red_stream_grid.rs::streamed_grid_equals_in_memory_grid`.
///
/// Порядок в `feed` — STRICT (ST-I-5/SM-I-4):
/// 1. `exchange.on_event` (биржа видит первой; модель не видит будущего);
/// 2. `strategy.on_fill` по каждому филлу — ВСЕ ДО `books.apply` (D7/ST-I-8g:
///    equity-марк отражает позицию, в которую УЖЕ включены все филлы этого события);
/// 3. `books.apply` (mid для MTM);
/// 4. equity-точка РОВНО ОДНА если `had_new_fill` (без фантомных точек);
/// 5. `strategy.on_event` — ТОЛЬКО ПОСЛЕ equity-точки (никакого будущего);
/// 6. submit интентов → `order_meta.insert`.
///
/// Дублирование тела `StrategyBacktest::run` — сознательная плата за streaming без
/// модификации `sim`. Любое изменение семантики обязано применяться и здесь.
struct CellRunner {
    cell_config: StrategyCellConfig,
    instrument: Instrument,
    strategy: DirectionalStrategy,
    exchange: BacktestExchange,
    /// Копия реконструкции книги для MTM (mid в equity-точке). Mirror exchange.books
    /// по snapshot'ам; для MTM стратегии — `book::Books::apply` (только L2Snapshot).
    books: Books,
    order_meta: BTreeMap<u64, (Instrument, Side)>,
    instruments_seen: BTreeSet<Instrument>,
    intents_count: usize,
    fills_out: Vec<SimFill>,
    cash_e8: i128,
    turnover_e8: i128,
    equity_curve_e8: Vec<i64>,
    /// D7: первый mid по инструменту (для `capital_ref_e8`).
    first_mid_e8: i64,
    params: serde_json::Value,
    params_hash: String,
}

impl CellRunner {
    #[allow(clippy::too_many_arguments)]
    fn new(
        cell_params: serde_json::Value,
        params_hash: String,
        obi_params: &ObiParams,
        signal_id: SignalId,
        signal: Obi,
        latency: LatencyTable,
        fees: FeeSchedule,
        seed: u64,
    ) -> Result<Self, RcError> {
        let cell_config = strategy_cell_config(&cell_params)?;
        let instrument = Instrument::new(obi_params.venue, obi_params.symbol.clone());

        let alpha = LinearAlpha::new(vec![SignalWeight {
            signal_id,
            instrument: instrument.clone(),
            weight_e8: EDGE_SCALE,
        }])
        .map_err(|error| RcError::Signal(format!("alpha config: {error:?}")))?;
        let budget = RiskBudget::new(vec![(instrument.clone(), cell_config.max_position_e8)])
            .map_err(|error| RcError::Signal(format!("portfolio config: {error:?}")))?;
        let strategy_config = StrategyConfig {
            min_order_e8: cell_config.min_order_e8,
            intent_ttl_ms: cell_config.intent_ttl_ms,
            marketable_margin_bp: cell_config.marketable_margin_bp,
            kind: cell_config.order_kind()?,
        };
        let strategy = DirectionalStrategy::new(
            vec![Box::new(signal)],
            Box::new(alpha),
            budget,
            strategy_config,
        )
        .map_err(|error| RcError::Signal(format!("strategy config: {error:?}")))?;

        Ok(Self {
            cell_config,
            instrument,
            strategy,
            exchange: BacktestExchange::new(latency, fees, seed),
            books: Books::new(),
            order_meta: BTreeMap::new(),
            instruments_seen: BTreeSet::new(),
            intents_count: 0,
            fills_out: Vec::new(),
            cash_e8: 0,
            turnover_e8: 0,
            equity_curve_e8: Vec::new(),
            first_mid_e8: 0,
            params: cell_params,
            params_hash,
        })
    }

    /// Подать ОДНО событие в ячейку. Семантика — IDENTICAL `StrategyBacktest::run`
    /// event-by-event (см. комментарий к `CellRunner`).
    ///
    /// `range_ms` — полуинтервал [from, to) по `ts_wall_ms`; события вне его
    /// пропускаются (как фильтр в in-memory `run_grid`).
    fn feed(&mut self, event: &Event, range_ms: (i64, i64)) {
        if event.ts_wall_ms < range_ms.0 || event.ts_wall_ms >= range_ms.1 {
            return;
        }

        // ── 1. Биржа видит событие первой (SM-I-4). ──────────────────────────────────
        let new_fills = self.exchange.on_event(event);
        let had_new_fill_this_event = !new_fills.is_empty();

        // ── 2. Доложить филлы стратегии (мост SimFill → FillReport, M-07 D2). ─────────
        let scale = contracts::PRICE_SCALE as i128;
        for sim_fill in &new_fills {
            let (instrument, side) = match self.order_meta.get(&sim_fill.order_id) {
                Some(meta) => meta.clone(),
                None => continue, // не наш ордер (теоретически не должно случиться, защита)
            };
            self.instruments_seen.insert(instrument.clone());

            let notional_e128 = (sim_fill.price as i128) * (sim_fill.qty as i128) / scale;
            let cash_delta = if matches!(side, Side::Buy) {
                -(notional_e128 + sim_fill.fee_e8 as i128)
            } else {
                notional_e128 - sim_fill.fee_e8 as i128
            };
            self.cash_e8 += cash_delta;
            self.turnover_e8 += notional_e128.abs();

            let report = FillReport {
                instrument: instrument.clone(),
                side,
                price_e8: sim_fill.price,
                qty_e8: sim_fill.qty,
                fee_e8: sim_fill.fee_e8,
                ts_mono_ns: sim_fill.ts_mono_ns,
            };
            self.strategy.on_fill(&report);
        }
        self.fills_out.extend(new_fills);

        // ── 2.5 Применить Md-событие к локальной реконструкции книги (mid для MTM). ─
        if let EventKind::Md(md) = &event.kind {
            self.books.apply(md);
            // D7: first_mid_e8 — только первый раз, когда books для ИНСТРУМЕНТА ячейки
            // имеет mid (L2Snapshot для нашего (venue, symbol)). Зеркалит in-memory
            // `first_mid_e8(events_in_range, &instrument)` — оба видят ОДНО И ТО ЖЕ
            // первое появление книги, потому что оба применяют Md в том же порядке.
            if self.first_mid_e8 == 0
                && md.venue == self.instrument.venue
                && md.symbol == self.instrument.symbol
            {
                if let Some(mid) = self
                    .books
                    .get(self.instrument.venue, &self.instrument.symbol)
                    .and_then(|book| book.mid())
                {
                    self.first_mid_e8 = mid;
                }
            }
        }

        // ── 2.7 Equity-точка: РОВНО ОДНА на событие, где биржа дала ≥1 филл (D7/ST-I-8g).
        if had_new_fill_this_event {
            let mut eq: i128 = self.cash_e8;
            for inst in &self.instruments_seen {
                let pos = self.strategy.position_e8(inst) as i128;
                let mid_price = match self
                    .books
                    .get(inst.venue, &inst.symbol)
                    .and_then(|b| b.mid())
                {
                    Some(p) => p as i128,
                    None => continue,
                };
                eq += pos * mid_price / scale;
            }
            let clamped = eq.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
            self.equity_curve_e8.push(clamped);
        }

        // ── 3. Стратегия видит событие (только ev.seq, без будущего). ────────────────
        let intents = self.strategy.on_event(event);

        // ── 4. Подать интенты на биржу; запомнить order_meta. ────────────────────────
        for intent in &intents {
            self.intents_count += 1;
            let inst = Instrument::new(intent.venue, intent.symbol.clone());
            self.instruments_seen.insert(inst.clone());
            if let Ok(order_id) = self.exchange.submit(intent.clone()) {
                self.order_meta.insert(order_id, (inst, intent.side));
            }
        }
    }

    fn finalize(self) -> Result<CellResult, RcError> {
        let capital_ref = capital_ref_e8(self.cell_config.max_position_e8, self.first_mid_e8);
        let returns = returns_from_equity(&self.equity_curve_e8, capital_ref);
        let sharpe = metrics::sharpe(&returns, ANNUALIZATION_PERIODS_PER_YEAR);
        let max_drawdown_e8 = metrics::max_drawdown_e8(&self.equity_curve_e8);
        let net_pnl_e8 = self.equity_curve_e8.last().copied().unwrap_or(0);

        Ok(CellResult {
            params: self.params,
            params_hash: self.params_hash,
            net_pnl_e8,
            sharpe,
            max_drawdown_e8,
            intents: self.intents_count,
            fills: self.fills_out.len(),
            turnover_e8: self.turnover_e8.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            returns,
        })
    }
}

/// Стрим-версия `run_grid` (M-08 задача 5, E5/E6).
///
/// Семантика ячеек/ledger/стресс-режимов — ТА ЖЕ, что у `run_grid` (оракул
/// эквивалентности в `tests/red_stream_grid.rs`). Отличается тем, что события НЕ
/// материализуются в `Vec<Event>`: стрим открывается ОДИН раз, и каждое событие
/// подаётся во ВСЕ `CellRunner`'ы по очереди — O(1) памяти по размеру журнала
/// (8.3 GB на проде, +2.8 GB/сут, не влезает в RAM уже сегодня — класс TD-011).
///
/// `EpochFilter` обязан быть НАЗВАН (`JournalSource.filter` — не `Option`), иначе
/// невозможно собрать `JournalSource`: вендорские данные не подмешиваются молча
/// (CT-RFC02-2/3/4).
pub fn run_grid_streamed(
    source: &JournalSource,
    spec: &GridSpec,
    split: SplitKind,
    range_ms: (i64, i64),
    env: &mut GridRunEnv<'_>,
    test_proof: Option<&ValGateToken>,
) -> Result<Vec<CellResult>, RcError> {
    if split == SplitKind::Test && test_proof.is_none() {
        return Err(RcError::GateDenied(
            "SplitKind::Test требует &ValGateToken (RC-I-8) — val-гейт не пройден".into(),
        ));
    }

    let signal_id_str = format!("{}-obi-asym", spec.signal_id_prefix);
    let code_hash = research_code_hash();
    // Конструируем runners ДО открытия стрима — fail-closed на кривом конфиге ячейки.
    // Runners держат state per-cell: BacktestExchange через события + трекинг отчёта.
    let mut runners: Vec<CellRunner> = Vec::with_capacity(spec.cells.len());
    for cell in &spec.cells {
        strategy_cell_config(cell)?;
        let params_hash = cell_params_hash(cell, spec.costs_mode);
        let signal_id = SignalId::parse(&signal_id_str)
            .map_err(|error| RcError::Signal(format!("{error:?}")))?;
        let signal = Obi::from_json_params(signal_id.clone(), 1, RegistryStatus::Candidate, cell)
            .map_err(|error| RcError::Signal(format!("{error:?}")))?;
        let obi_params: ObiParams = serde_json::from_value(cell.clone())
            .map_err(|error| RcError::Parse(error.to_string()))?;

        // Стресс-режимы — ОТДЕЛЬНЫЕ прогоны (RC-I-10).
        let latency = match spec.costs_mode {
            CostsMode::LatencyX2 => env.latency.scaled(2.0),
            _ => env.latency.scaled(1.0),
        };
        let fees = match spec.costs_mode {
            CostsMode::CostX15 => env.fees.scaled(1.5),
            _ => env.fees.scaled(1.0),
        };

        runners.push(CellRunner::new(
            cell.clone(),
            params_hash,
            &obi_params,
            signal_id,
            signal,
            latency,
            fees,
            spec.seed,
        )?);
    }

    // ОДИН стрим на грид (не на ячейку) — каждое событие идёт во все runners.
    let stream = journal::stream(&source.dir, source.filter.clone()).map_err(RcError::Io)?;

    let mut ts_wall_ms = range_ms.0;
    for result in stream {
        let event = result.map_err(RcError::Io)?;
        ts_wall_ms = event.ts_wall_ms;
        for runner in &mut runners {
            runner.feed(&event, range_ms);
        }
    }

    // Финализация: runners → CellResult + запись в ledger (RC-I-9: каждая ячейка
    // пишется независимо от исхода; отказ записи → abort Err::LedgerWrite, FA §3).
    let mut results: Vec<CellResult> = Vec::with_capacity(runners.len());
    for (index, runner) in runners.into_iter().enumerate() {
        let cell_result = runner.finalize()?;
        let params_hash = cell_result.params_hash.clone();

        env.ledger
            .append(TrialRecord {
                schema_version: TRIALS_LEDGER_SCHEMA_VERSION,
                signal_family: spec.signal_family.clone(),
                signal_id: signal_id_str.clone(),
                params_hash,
                split,
                costs_mode: spec.costs_mode,
                ts_wall_ms,
                code_hash: code_hash.clone(),
                result_ref: format!("cell-{index}"),
                sharpe: Some(cell_result.sharpe),
                prev_sha256: String::new(),
            })
            .map_err(|error| RcError::LedgerWrite(error.to_string()))?;

        results.push(cell_result);
    }

    Ok(results)
}

/// Механическая сортировка топ-K по предварительной метрике (Sharpe без deflation —
/// deflation только на финальной валидации, FA §5). НЕ трогает val/test.
pub fn top_k(results: &[CellResult], k: usize) -> Vec<CellResult> {
    let mut sorted: Vec<CellResult> = results.to_vec();
    sorted.sort_by(|a, b| {
        b.sharpe
            .partial_cmp(&a.sharpe)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.truncate(k);
    sorted
}
