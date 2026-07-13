//! grid — оркестрация перебора (FA §5): для КАЖДОЙ ячейки — инстанс сигнала →
//! настоящий strategy-пайплайн → sim fills/PnL → ЗАПИСЬ В LEDGER независимо от
//! исхода и попадания в top-K (RC-I-9/§5).
//!
//! M-07 D7/D8: один и тот же `DirectionalStrategy` исполняется в backtest и будущем
//! live; returns берутся из mark-to-market equity `StrategyBacktest`, а хэш ячейки
//! покрывает signal+strategy+costs. Стресс-режимы остаются отдельными sim-прогонами
//! со scaled-таблицами (RC-I-10). Отказ ledger-записи abort'ит весь grid (FA §3).

use alpha::{Instrument, LinearAlpha, SignalWeight, EDGE_SCALE};
use book::Books;
use contracts::{Event, EventKind};
use portfolio::RiskBudget;
use sha2::{Digest, Sha256};
use signals::obi::{Obi, ObiParams};
use signals::{RegistryStatus, SignalId};
use sim::{FeeSchedule, LatencyTable, StrategyBacktest};
use strategy::{DirectionalStrategy, StrategyConfig};

use crate::ledger::Ledger;
use crate::metrics;
use crate::split::ValGateToken;
use crate::strategy_cell::{
    capital_ref_e8, cell_params_hash, returns_from_equity, strategy_cell_config,
};
use crate::types::{
    CellResult, CostsMode, GridSpec, RcError, SplitKind, TrialRecord, TRIALS_LEDGER_SCHEMA_VERSION,
};

/// Аннуализация Sharpe внутри грида (сравнение ячеек между собой; не путать с
/// итоговым отчётным Sharpe research/reports — там периодичность явная).
const ANNUALIZATION_PERIODS_PER_YEAR: f64 = 252.0;

/// Окружение прогона: ledger (единственная точка записи) + честные таблицы sim.
pub struct GridRunEnv<'a> {
    pub ledger: &'a mut Ledger,
    pub latency: &'a LatencyTable,
    pub fees: &'a FeeSchedule,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// sha256 исходника `crates/signals/src/obi.rs` (D3). Путь строится через
/// `CARGO_MANIFEST_DIR` (компилируемая константа этого крейта) — детерминирован,
/// не зависит от текущего рабочего каталога процесса. "unknown" при отсутствии файла
/// (не должно случаться в дереве репозитория; не паникуем).
fn signal_code_hash() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../signals/src/obi.rs");
    match std::fs::read(&path) {
        Ok(bytes) => sha256_hex(&bytes),
        Err(_) => "unknown".to_string(),
    }
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
    let code_hash = signal_code_hash();
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
