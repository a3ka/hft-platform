//! ST-I-8 — интеграционный RED: тот же `dyn Strategy` гоняется через `sim::BacktestExchange`.
//! SACRED — architect-only. docs/fa/strategy-brain.md §7, M-07 D3.
//!
//! Это оракул равенства DESIGN §1 №2 (`backtest == paper == live`): бэктест ОБЯЗАН исполнять
//! настоящий код решений, а не ad-hoc harness. Анти-плацебо: тест падает, если `run()`
//! (а) не доносит интенты до биржи (fills пусты), (б) не докладывает филлы стратегии
//! (позиция стратегии разъедется с нетто филлов), (в) недетерминирован при том же seed.

use std::collections::BTreeMap;

use alpha::{Instrument, LinearAlpha, SignalWeight, EDGE_SCALE};
use contracts::{Event, EventKind, Level, MdPayload, Venue};
use portfolio::RiskBudget;
use signals::{RegistryStatus, Signal, SignalId, SignalMeta, SignalOut, SignalSpecRef};
use sim::{BacktestReport, FeeRates, FeeSchedule, LatencyTable, StrategyBacktest};
use strategy::{DirectionalStrategy, OrderKind, Strategy, StrategyConfig};

const MS: u64 = 1_000_000;
const SIG: &str = "S-001-obi-asym";

fn btc() -> Instrument {
    Instrument::new(Venue::Binance, "BTCUSDT")
}

struct ScriptedSignal {
    id: SignalId,
    script: BTreeMap<u64, i64>,
}

impl Signal for ScriptedSignal {
    fn on_event(&mut self, ev: &Event) -> Option<SignalOut> {
        self.script.get(&ev.seq).map(|v| SignalOut {
            signal_id: self.id.clone(),
            ts_event_mono_ns: ev.ts_mono_ns,
            value: *v,
            status: RegistryStatus::Candidate,
            meta: SignalMeta { horizon_ms: 5_000 },
        })
    }
    fn spec(&self) -> SignalSpecRef {
        SignalSpecRef {
            id: self.id.clone(),
            version: 1,
        }
    }
}

fn snapshot(seq: u64, ts_mono_ns: u64) -> Event {
    Event {
        seq,
        ts_mono_ns,
        ts_wall_ms: 1_752_000_000_000 + seq as i64,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![Level {
                    price: contracts::to_fixed(100.0),
                    size: contracts::to_fixed(10.0),
                }],
                asks: vec![Level {
                    price: contracts::to_fixed(101.0),
                    size: contracts::to_fixed(10.0),
                }],
                ts_exch_ms: 1_752_000_000_000,
            },
        ),
    }
}

fn table() -> LatencyTable {
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

/// Поток: снапшоты каждые 500ms; сигнал зовёт в лонг (seq 2), затем в ноль (seq 8).
fn events() -> Vec<Event> {
    (1..=12u64).map(|i| snapshot(i, i * 500 * MS)).collect()
}

fn script() -> BTreeMap<u64, i64> {
    BTreeMap::from([(2, EDGE_SCALE), (8, 0)])
}

fn strat() -> DirectionalStrategy {
    let sig = ScriptedSignal {
        id: SignalId::parse(SIG).expect("valid"),
        script: script(),
    };
    let alpha = LinearAlpha::new(vec![SignalWeight {
        signal_id: SignalId::parse(SIG).expect("valid"),
        instrument: btc(),
        weight_e8: EDGE_SCALE,
    }])
    .expect("valid");
    let budget = RiskBudget::new(vec![(btc(), EDGE_SCALE)]).expect("valid");
    DirectionalStrategy::new(
        vec![Box::new(sig)],
        Box::new(alpha),
        budget,
        StrategyConfig {
            min_order_e8: 1_000_000,
            intent_ttl_ms: 1_000,
            marketable_margin_bp: 100,
            kind: OrderKind::Taker,
        },
    )
    .expect("valid")
}

fn run(seed: u64) -> (BacktestReport, i64) {
    let mut bt = StrategyBacktest::new(table(), fees(), seed);
    let mut s = strat();
    let report = bt.run(&events(), &mut s);
    let pos = s.position_e8(&btc());
    (report, pos)
}

/// ST-I-8a: интенты стратегии РЕАЛЬНО доходят до биржи и исполняются (taker по видимой
/// книге). Пустой отчёт = харнесс не подключён (плацебо).
#[test]
fn st_i_8a_strategy_intents_reach_the_exchange() {
    let (report, _) = run(42);
    assert!(
        report.intents >= 2,
        "ожидаем вход (seq 2) и выход (seq 8), получено интентов: {}",
        report.intents
    );
    assert!(
        !report.fills.is_empty(),
        "интенты обязаны исполняться против видимой книги (fills пусты → мост мёртв)"
    );
    assert!(report.turnover_e8 > 0, "исполнение обязано давать оборот");
    assert!(
        report.fills.iter().all(|f| !f.maker),
        "v1 стратегия — taker"
    );
}

/// ST-I-8b: позиция, которую ведёт СТРАТЕГИЯ, равна нетто исполнений биржи. Расхождение =
/// филлы не докладываются стратегии (фантомная позиция; прямой предок recon-mismatch RK-I-8).
#[test]
fn st_i_8b_strategy_position_equals_net_of_fills() {
    let (report, strat_pos) = run(42);
    assert!(
        report.fills.iter().map(|f| f.qty).sum::<i64>() > 0,
        "хоть что-то обязано исполниться"
    );

    let reported = report
        .positions
        .get(&btc())
        .copied()
        .expect("инструмент обязан быть в отчёте");
    assert_eq!(
        strat_pos, reported,
        "позиция стратегии обязана совпадать с нетто-позицией отчёта \
         (иначе стратегия торгует по фантомной позиции)"
    );

    // Вход +1.0 и выход −1.0 → нетто ≈ 0 (в пределах того, что дала видимая книга).
    assert_eq!(
        strat_pos, 0,
        "после сигнала «в ноль» позиция обязана закрыться"
    );
}

/// ST-I-8c (DET): один и тот же поток + seed → бит-идентичный отчёт (DESIGN §1).
#[test]
fn st_i_8c_backtest_is_deterministic_given_seed() {
    let (a, pa) = run(7);
    let (b, pb) = run(7);
    assert_eq!(a, b, "DET: один seed + один поток → идентичный отчёт");
    assert_eq!(pa, pb);
}

/// ST-I-8d (NO-LOOKAHEAD, зеркало SM-I-4): стратегия не может действовать на событии,
/// которого биржа ещё не видела — интенты первого события не могут исполниться ДО него.
/// Прогон на префиксе даёт префикс филлов полного прогона.
#[test]
fn st_i_8d_prefix_run_is_prefix_of_full_run() {
    let full = {
        let mut bt = StrategyBacktest::new(table(), fees(), 7);
        let mut s = strat();
        bt.run(&events(), &mut s)
    };
    let prefix = {
        let mut bt = StrategyBacktest::new(table(), fees(), 7);
        let mut s = strat();
        bt.run(&events()[..6], &mut s)
    };

    assert!(!prefix.fills.is_empty(), "префикс обязан дать филлы");
    assert_eq!(
        prefix.fills[..],
        full.fills[..prefix.fills.len()],
        "филлы префикса обязаны совпадать с началом полного прогона — иначе где-то \
         используется информация из будущего"
    );
    assert!(
        prefix.intents <= full.intents,
        "префикс не может произвести БОЛЬШЕ решений, чем полный поток"
    );
}
