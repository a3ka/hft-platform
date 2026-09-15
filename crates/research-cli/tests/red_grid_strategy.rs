//! RED-оракулы задачи 6 M-07 (GR-I-1..7): грид ОБЯЗАН гонять настоящий strategy-пайплайн
//! через `sim::StrategyBacktest` и считать returns по D7, а ledger — хэшировать блок
//! `strategy` (D8). SACRED — architect-only. Заведено по вердикту critic C-004 (C2).
//!
//! Почему грепов недостаточно (C2): research-dev может удалить имена `OpenPosition`/`Action`
//! и упомянуть `StrategyBacktest` в комментарии — T9-грепы позеленеют, а грид продолжит
//! мерить ad-hoc-логику, которой не будет в live. Ниже — ПОВЕДЕНЧЕСКИЕ оракулы:
//! GR-I-6/7 падают на любой реализации, которая игнорирует блок `strategy` ячейки
//! (старый harness с фиксированным qty=1.0 и taker-выходом по horizon — именно такая).

use book::OrderBook;
use contracts::{Event, EventKind, Level, MdPayload, Venue};
use research_cli::strategy_cell::{
    capital_ref_e8, cell_params_hash, returns_from_equity, strategy_cell_config,
    DEFAULT_INTENT_TTL_MS, DEFAULT_MARKETABLE_MARGIN_BP, DEFAULT_MAX_POSITION_E8,
    DEFAULT_MIN_ORDER_E8,
};
use research_cli::types::{CostsMode, GridSpec, SplitKind};
use research_cli::{grid, Ledger};
use sim::{FeeRates, FeeSchedule, LatencyTable};

const E8: i64 = 100_000_000;

/// Ячейка: OBI top_n + опциональный блок strategy.
fn cell(strategy_block: Option<serde_json::Value>) -> serde_json::Value {
    let mut c = serde_json::json!({
        "mode": "top_n",
        "n_levels": 3,
        "theta_e8": 10_000_000,
        "horizon_ms": 2_000,
        "venue": "Binance",
        "symbol": "BTCUSDT"
    });
    if let Some(s) = strategy_block {
        c.as_object_mut()
            .expect("object")
            .insert("strategy".to_string(), s);
    }
    c
}

/// Сильно перекошенная книга (bids ≫ asks) → OBI даёт устойчивый BUY-score.
fn snapshot(seq: u64) -> Event {
    Event {
        seq,
        ts_mono_ns: seq * 200_000_000, // 200ms шаг
        ts_wall_ms: 1_752_000_000_000 + (seq as i64) * 200,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![
                    Level {
                        price: contracts::to_fixed(100.0),
                        size: contracts::to_fixed(50.0),
                    },
                    Level {
                        price: contracts::to_fixed(99.0),
                        size: contracts::to_fixed(50.0),
                    },
                    Level {
                        price: contracts::to_fixed(98.0),
                        size: contracts::to_fixed(50.0),
                    },
                ],
                asks: vec![
                    Level {
                        price: contracts::to_fixed(101.0),
                        size: contracts::to_fixed(5.0),
                    },
                    Level {
                        price: contracts::to_fixed(102.0),
                        size: contracts::to_fixed(5.0),
                    },
                    Level {
                        price: contracts::to_fixed(103.0),
                        size: contracts::to_fixed(5.0),
                    },
                ],
                ts_exch_ms: 1_752_000_000_000,
            },
        ),
    }
}

fn events() -> Vec<Event> {
    (1..=60u64).map(snapshot).collect()
}

fn latency() -> LatencyTable {
    let mut t = LatencyTable::new();
    t.insert_samples(
        Venue::Binance,
        "BTCUSDT",
        vec![1_000_000],
        vec![1_000_000],
        vec![500_000],
        "synthetic-test-fixture",
    );
    t
}

fn fees() -> FeeSchedule {
    let mut f = FeeSchedule::new();
    f.insert_rates(
        Venue::Binance,
        FeeRates {
            maker_rate_e8: 10_000,
            taker_rate_e8: 45_000,
        },
    );
    f
}

fn spec(cells: Vec<serde_json::Value>) -> GridSpec {
    GridSpec {
        signal_family: "obi".to_string(),
        signal_id_prefix: "S-001".to_string(),
        cells,
        costs_mode: CostsMode::Baseline,
        seed: 42,
    }
}

/// Прогнать грид, вернуть (результаты ячеек, записи ledger).
fn run(
    cells: Vec<serde_json::Value>,
) -> (
    Vec<research_cli::CellResult>,
    Vec<research_cli::TrialRecord>,
) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ledger = Ledger::open(dir.path().join("trials.jsonl")).expect("ledger");
    let lat = latency();
    let fee = fees();
    let mut env = grid::GridRunEnv {
        ledger: &mut ledger,
        latency: &lat,
        fees: &fee,
    };
    let evs = events();
    let s = spec(cells);
    let results = grid::run_grid(
        &evs,
        &s,
        SplitKind::Train,
        (1_752_000_000_000, 1_752_000_099_999),
        &mut env,
        None,
    )
    .expect("grid run");
    let records = ledger.read_all().expect("ledger read");
    (results, records)
}

/// GR-I-1 (D8): блока `strategy` нет → ДОКУМЕНТИРОВАННЫЕ дефолты (не нули, не «что-нибудь»).
#[test]
fn gr_i_1_absent_strategy_block_uses_documented_defaults() {
    let c = strategy_cell_config(&cell(None)).expect("дефолты валидны");
    assert_eq!(c.max_position_e8, DEFAULT_MAX_POSITION_E8);
    assert_eq!(c.min_order_e8, DEFAULT_MIN_ORDER_E8);
    assert_eq!(c.intent_ttl_ms, DEFAULT_INTENT_TTL_MS);
    assert_eq!(c.marketable_margin_bp, DEFAULT_MARKETABLE_MARGIN_BP);
    assert_eq!(c.kind, "taker", "v1 directional — taker");
}

/// GR-I-2 (D8): блок присутствует → он и применяется; кривой блок → Err (fail-closed,
/// а не «молча дефолт» — иначе отчёт описывает не ту стратегию, что бежала).
#[test]
fn gr_i_2_present_strategy_block_is_applied_and_validated() {
    let c = strategy_cell_config(&cell(Some(serde_json::json!({
        "max_position_e8": 250_000_000i64,
        "min_order_e8": 5_000_000i64,
        "intent_ttl_ms": 3_000i64,
        "marketable_margin_bp": 25i64,
        "kind": "taker"
    }))))
    .expect("валидный блок");
    assert_eq!(c.max_position_e8, 250_000_000);
    assert_eq!(c.min_order_e8, 5_000_000);
    assert_eq!(c.intent_ttl_ms, 3_000);
    assert_eq!(c.marketable_margin_bp, 25);

    assert!(
        strategy_cell_config(&cell(Some(serde_json::json!({ "max_position_e8": 0i64 })))).is_err(),
        "неположительный max_position → Err, не дефолт"
    );
    assert!(
        strategy_cell_config(&cell(Some(serde_json::json!({ "kind": "quantum" })))).is_err(),
        "неизвестный kind → Err (fail-closed)"
    );
}

/// GR-I-3 (D8): `params_hash` ОБЯЗАН покрывать блок `strategy` и `costs_mode`.
/// Иначе два прогона РАЗНЫХ стратегий пишутся в ledger одним хэшем → счётчик проб
/// (deflated Sharpe, анти-оверфит) фальсифицирован.
#[test]
fn gr_i_3_params_hash_covers_strategy_block_and_costs() {
    let base = cell(None);
    let with_strategy = cell(Some(
        serde_json::json!({ "max_position_e8": 250_000_000i64 }),
    ));
    let other_strategy = cell(Some(
        serde_json::json!({ "max_position_e8": 300_000_000i64 }),
    ));

    let h_base = cell_params_hash(&base, CostsMode::Baseline);
    assert_eq!(
        h_base,
        cell_params_hash(&base, CostsMode::Baseline),
        "хэш детерминирован"
    );
    assert_ne!(
        h_base,
        cell_params_hash(&with_strategy, CostsMode::Baseline),
        "наличие блока strategy обязано менять хэш"
    );
    assert_ne!(
        cell_params_hash(&with_strategy, CostsMode::Baseline),
        cell_params_hash(&other_strategy, CostsMode::Baseline),
        "разные strategy-параметры → разные хэши"
    );
    assert_ne!(
        h_base,
        cell_params_hash(&base, CostsMode::CostX15),
        "стресс-режим → другой хэш (RC-I-10)"
    );
}

/// GR-I-4 (D7): returns считаются из mark-to-market equity, нормированной на capital_ref.
/// Старая формула (entry/exit-нотионалы ad-hoc harness'а) даёт другие числа.
#[test]
fn gr_i_4_returns_are_equity_deltas_over_capital_ref() {
    // equity: 0 → +50 → +30 (×1e8); capital_ref = 100 (×1e8)
    let equity = vec![0, 50 * E8, 30 * E8];
    let r = returns_from_equity(&equity, 100 * E8);
    assert_eq!(r.len(), 2, "N точек equity → N−1 доходностей");
    assert!(
        (r[0] - 0.5).abs() < 1e-12,
        "Δ+50 / 100 = +0.5, получено {}",
        r[0]
    );
    assert!(
        (r[1] + 0.2).abs() < 1e-12,
        "Δ−20 / 100 = −0.2, получено {}",
        r[1]
    );

    assert!(
        returns_from_equity(&equity, 0).is_empty(),
        "capital_ref = 0 → пусто (никаких NaN/inf в Sharpe)"
    );
    assert!(
        returns_from_equity(&[42], 100 * E8).is_empty(),
        "< 2 точек equity → пусто"
    );
}

/// GR-I-5 (D7): опорный капитал = нотионал максимальной позиции по первому mid.
#[test]
fn gr_i_5_capital_ref_is_max_position_notional() {
    // max_position = 2.0, mid = 100.0 → capital_ref = 200.0
    assert_eq!(capital_ref_e8(2 * E8, 100 * E8), 200 * E8);
    assert_eq!(capital_ref_e8(E8, 0), 0, "нет книги → 0, не «1»");

    // Sanity фикстуры: mid книги теста = 100.5
    let mut b = OrderBook::new();
    let EventKind::Md(md) = &snapshot(1).kind else {
        unreachable!()
    };
    let MdPayload::L2Snapshot { bids, asks, .. } = &md.payload else {
        unreachable!()
    };
    b.apply_snapshot(bids, asks);
    assert_eq!(b.mid(), Some(contracts::to_fixed(100.5)));
}

/// GR-I-6 (ПОВЕДЕНЧЕСКИЙ, убивает плацебо): ячейки, различающиеся ТОЛЬКО блоком
/// `strategy.max_position_e8`, ОБЯЗАНЫ дать разные результаты. Старый ad-hoc harness
/// торгует фиксированным qty=1.0 и блок `strategy` не читает → результаты совпадут
/// → тест FAIL. Упоминание `StrategyBacktest` в комментарии этот тест не пройдёт.
#[test]
fn gr_i_6_strategy_block_changes_grid_behaviour() {
    let small = cell(Some(
        serde_json::json!({ "max_position_e8": 10_000_000i64 }),
    )); // 0.1
    let large = cell(Some(
        serde_json::json!({ "max_position_e8": 300_000_000i64 }),
    )); // 3.0

    let (results, _) = run(vec![small, large]);
    assert_eq!(results.len(), 2);
    assert!(
        results[0].intents > 0 && results[1].intents > 0,
        "сигнал перекошенной книги обязан породить интенты в обеих ячейках"
    );
    assert_ne!(
        results[0].turnover_e8, results[1].turnover_e8,
        "max_position ×30 обязан менять оборот — грид игнорирует блок strategy \
         (значит, гоняет НЕ strategy-пайплайн)"
    );
    assert!(
        results[1].turnover_e8 > results[0].turnover_e8,
        "больший лимит позиции → больший оборот"
    );
}

/// GR-I-7 (ПОВЕДЕНЧЕСКИЙ): деадбенд `min_order_e8` больше лимита позиции → торговли НЕТ.
/// Ad-hoc harness всё равно бы торговал (он не знает про min_order) → FAIL.
/// Плюс: params_hash в ledger обязан совпадать с каноническим cell_params_hash (D8) —
/// иначе грид пишет в ledger хэш, не покрывающий strategy-параметры.
#[test]
fn gr_i_7_deadband_blocks_trading_and_ledger_uses_canonical_hash() {
    let muted = cell(Some(serde_json::json!({
        "max_position_e8": 100_000_000i64,
        "min_order_e8": 500_000_000i64 // 5.0 > max_position → дельта никогда не дотянет
    })));

    let (results, records) = run(vec![muted.clone()]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].intents, 0,
        "деадбенд шире лимита → интентов быть не должно (harness, игнорирующий \
         min_order_e8, всё равно наторгует)"
    );
    assert_eq!(results[0].fills, 0);

    assert_eq!(records.len(), 1, "ячейка обязана попасть в ledger (RC-I-9)");
    assert_eq!(
        records[0].params_hash,
        cell_params_hash(&muted, CostsMode::Baseline),
        "ledger обязан нести КАНОНИЧЕСКИЙ хэш ячейки (покрывающий блок strategy)"
    );
    assert_eq!(
        results[0].params_hash, records[0].params_hash,
        "хэш результата и хэш ledger-записи — один и тот же"
    );
}
