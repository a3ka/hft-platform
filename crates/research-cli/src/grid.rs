//! grid — оркестрация перебора (FA §5): для КАЖДОЙ ячейки — инстанс сигнала →
//! реплей событий диапазона через harness (сигнал → интенты → sim fills → PnL) →
//! ЗАПИСЬ В LEDGER (независимо от исхода и попадания в топ-K — RC-I-9/§5).
//! Стресс-режимы — отдельные прогоны с scaled() таблицами (RC-I-10).
//!
//! Harness v1 (M-04): направленный вход по SignalOut (taker по умолчанию; maker —
//! параметр ячейки), выход taker через horizon_ms. Отказ ledger-записи → abort
//! ВСЕГО прогона (FA §3).
//!
//! Реализация — research-dev (M-04 task 4).

use book::Books;
use contracts::{Event, EventKind, Side};
use sha2::{Digest, Sha256};
use signals::obi::{Obi, ObiParams};
use signals::{RegistryStatus, Signal, SignalId};
use sim::{BacktestExchange, FeeSchedule, LatencyTable, OrderIntent, OrderKind};

use crate::ledger::Ledger;
use crate::metrics;
use crate::split::ValGateToken;
use crate::types::{
    CellResult, CostsMode, GridSpec, RcError, SplitKind, TrialRecord, TRIALS_LEDGER_SCHEMA_VERSION,
};

/// Аннуализация Sharpe внутри harness v1 (грид-сравнение ячеек между собой; не
/// путать с итоговым отчётным Sharpe research/reports — там периодичность явная).
const ANNUALIZATION_PERIODS_PER_YEAR: f64 = 252.0;
/// Запас маркетабельности лимит-цены taker-входа/выхода (1%, per milestone).
const MARKETABLE_MARGIN_PCT: f64 = 0.01;

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

fn costs_mode_str(mode: CostsMode) -> &'static str {
    match mode {
        CostsMode::Baseline => "baseline",
        CostsMode::CostX15 => "cost_x15",
        CostsMode::LatencyX2 => "latency_x2",
    }
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

fn notional_e8(price: i64, qty: i64) -> i64 {
    ((price as i128 * qty as i128) / contracts::PRICE_SCALE as i128) as i64
}

/// Открытая позиция harness'а (taker-вход по SignalOut, taker-выход через horizon_ms).
struct OpenPosition {
    entry_side: Side,
    exit_due_mono_ns: u64,
    entry_order_id: u64,
    exit_order_id: Option<u64>,
    entry_notional_e8: i64,
    entry_fee_e8: i64,
    entry_qty_e8: i64,
    exit_notional_e8: i64,
    exit_fee_e8: i64,
    exit_qty_e8: i64,
}

enum Action {
    None,
    RequestExit {
        side: Side,
    },
    RequestEntry {
        side: Side,
        horizon_ms: i64,
        ts_event_mono_ns: u64,
    },
}

/// Marketable-limit цена (1% запас) для тейкера на стороне `side` по текущей книге.
fn marketable_price(book: &book::OrderBook, side: Side) -> Option<i64> {
    match side {
        Side::Buy => book
            .best_ask()
            .map(|p| ((p as f64) * (1.0 + MARKETABLE_MARGIN_PCT)).round() as i64),
        Side::Sell => book
            .best_bid()
            .map(|p| ((p as f64) * (1.0 - MARKETABLE_MARGIN_PCT)).round() as i64),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_one_cell(
    events_in_range: &[&Event],
    obi_params: &ObiParams,
    mut signal: Obi,
    latency: LatencyTable,
    fees: FeeSchedule,
    seed: u64,
    params: serde_json::Value,
    params_hash: String,
) -> CellResult {
    let mut exchange = BacktestExchange::new(latency, fees, seed);
    let mut quote_books = Books::new();
    let mut position: Option<OpenPosition> = None;

    let mut intents = 0usize;
    let mut fills_count = 0usize;
    let mut net_pnl_e8 = 0i64;
    let mut turnover_e8 = 0i64;
    let mut equity_running_e8 = 0i64;
    let mut equity_series: Vec<i64> = vec![0];
    let mut returns: Vec<f64> = Vec::new();

    for ev in events_in_range.iter().copied() {
        let fills = exchange.on_event(ev);
        if let EventKind::Md(md) = &ev.kind {
            quote_books.apply(md);
        }
        fills_count += fills.len();

        for fill in &fills {
            if let Some(pos) = position.as_mut() {
                if fill.order_id == pos.entry_order_id {
                    pos.entry_notional_e8 += notional_e8(fill.price, fill.qty);
                    pos.entry_fee_e8 += fill.fee_e8;
                    pos.entry_qty_e8 += fill.qty;
                } else if Some(fill.order_id) == pos.exit_order_id {
                    pos.exit_notional_e8 += notional_e8(fill.price, fill.qty);
                    pos.exit_fee_e8 += fill.fee_e8;
                    pos.exit_qty_e8 += fill.qty;
                }
            }
        }

        // Закрытие позиции при первом exit-филле (harness v1: qty фикс. 1.0 — на
        // синтетических/реальных данных с достаточной глубиной исполняется одним тейком).
        let ready_to_close = position
            .as_ref()
            .is_some_and(|p| p.exit_order_id.is_some() && p.exit_qty_e8 > 0);
        if ready_to_close {
            let pos = position.take().expect("checked Some above");
            let sign: i64 = match pos.entry_side {
                Side::Buy => 1,
                Side::Sell => -1,
            };
            let pnl_e8 = sign * (pos.exit_notional_e8 - pos.entry_notional_e8)
                - pos.entry_fee_e8
                - pos.exit_fee_e8;
            let ret = pnl_e8 as f64 / (pos.entry_notional_e8.max(1)) as f64;
            returns.push(ret);
            net_pnl_e8 += pnl_e8;
            equity_running_e8 += pnl_e8;
            equity_series.push(equity_running_e8);
            turnover_e8 += pos.entry_notional_e8.abs() + pos.exit_notional_e8.abs();
        }

        let sig_out = signal.on_event(ev);

        // Определяем ДЕЙСТВИЕ до мутации `position` (иначе конфликт заимствований
        // между immutable-чтением состояния и mutable-записью order_id/новой позиции).
        let action = if let Some(pos) = &position {
            if pos.exit_order_id.is_none() && ev.ts_mono_ns >= pos.exit_due_mono_ns {
                let side = match pos.entry_side {
                    Side::Buy => Side::Sell,
                    Side::Sell => Side::Buy,
                };
                Action::RequestExit { side }
            } else {
                Action::None
            }
        } else if let Some(out) = &sig_out {
            let side = if out.value > 0 { Side::Buy } else { Side::Sell };
            Action::RequestEntry {
                side,
                horizon_ms: out.meta.horizon_ms,
                ts_event_mono_ns: out.ts_event_mono_ns,
            }
        } else {
            Action::None
        };

        match action {
            Action::None => {}
            Action::RequestExit { side } => {
                if let Some(book) = quote_books.get(obi_params.venue, &obi_params.symbol) {
                    if let Some(price) = marketable_price(book, side) {
                        let intent = OrderIntent {
                            venue: obi_params.venue,
                            symbol: obi_params.symbol.clone(),
                            side,
                            price,
                            qty: contracts::to_fixed(1.0),
                            kind: OrderKind::Taker,
                        };
                        if let Ok(order_id) = exchange.submit(intent) {
                            intents += 1;
                            if let Some(pos) = position.as_mut() {
                                pos.exit_order_id = Some(order_id);
                            }
                        }
                    }
                }
            }
            Action::RequestEntry {
                side,
                horizon_ms,
                ts_event_mono_ns,
            } => {
                if let Some(book) = quote_books.get(obi_params.venue, &obi_params.symbol) {
                    if let Some(price) = marketable_price(book, side) {
                        let intent = OrderIntent {
                            venue: obi_params.venue,
                            symbol: obi_params.symbol.clone(),
                            side,
                            price,
                            qty: contracts::to_fixed(1.0),
                            kind: OrderKind::Taker,
                        };
                        if let Ok(order_id) = exchange.submit(intent) {
                            intents += 1;
                            position = Some(OpenPosition {
                                entry_side: side,
                                exit_due_mono_ns: ts_event_mono_ns
                                    + (horizon_ms.max(0) as u64) * 1_000_000,
                                entry_order_id: order_id,
                                exit_order_id: None,
                                entry_notional_e8: 0,
                                entry_fee_e8: 0,
                                entry_qty_e8: 0,
                                exit_notional_e8: 0,
                                exit_fee_e8: 0,
                                exit_qty_e8: 0,
                            });
                        }
                    }
                }
            }
        }
    }

    let sharpe = metrics::sharpe(&returns, ANNUALIZATION_PERIODS_PER_YEAR);
    let max_dd = metrics::max_drawdown_e8(&equity_series);

    CellResult {
        params,
        params_hash,
        net_pnl_e8,
        sharpe,
        max_drawdown_e8: max_dd,
        intents,
        fills: fills_count,
        turnover_e8,
        returns,
    }
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

    let events_in_range: Vec<&Event> = events
        .iter()
        .filter(|e| e.ts_wall_ms >= range_ms.0 && e.ts_wall_ms < range_ms.1)
        .collect();
    let ts_wall_ms = events_in_range
        .last()
        .map(|e| e.ts_wall_ms)
        .unwrap_or(range_ms.0);

    // D11: грид инстанцирует OBI напрямую (единственный сигнал M-04).
    let signal_id_str = format!("{}-obi-asym", spec.signal_id_prefix);
    let code_hash = signal_code_hash();
    let costs_str = costs_mode_str(spec.costs_mode);

    let mut results = Vec::with_capacity(spec.cells.len());

    for cell in &spec.cells {
        let canonical = serde_json::to_string(cell).map_err(|e| RcError::Parse(e.to_string()))?;
        let params_hash = sha256_hex(format!("{canonical}:{costs_str}").as_bytes());

        let id = SignalId::parse(&signal_id_str).map_err(|e| RcError::Signal(format!("{e:?}")))?;
        let signal = Obi::from_json_params(id, 1, RegistryStatus::Candidate, cell)
            .map_err(|e| RcError::Signal(format!("{e:?}")))?;
        let obi_params: ObiParams =
            serde_json::from_value(cell.clone()).map_err(|e| RcError::Parse(e.to_string()))?;

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
            signal,
            latency,
            fees,
            spec.seed,
            cell.clone(),
            params_hash.clone(),
        );

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
            .map_err(|e| RcError::LedgerWrite(e.to_string()))?;

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
