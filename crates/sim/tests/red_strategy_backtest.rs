//! ST-I-8 — интеграционный RED: тот же `dyn Strategy` гоняется через `sim::BacktestExchange`.
//! SACRED — architect-only. docs/fa/strategy-brain.md §7, M-07 D3.
//!
//! Это оракул равенства DESIGN §1 №2 (`backtest == paper == live`): бэктест ОБЯЗАН исполнять
//! настоящий код решений, а не ad-hoc harness. Анти-плацебо: тест падает, если `run()`
//! (а) не доносит интенты до биржи (fills пусты), (б) не докладывает филлы стратегии
//! (позиция стратегии разъедется с нетто филлов), (в) недетерминирован при том же seed.

use std::collections::BTreeMap;

use alpha::{Instrument, LinearAlpha, SignalWeight, EDGE_SCALE};
use contracts::{Event, EventKind, Level, MdPayload, Side, Venue};
use portfolio::RiskBudget;
use signals::{RegistryStatus, Signal, SignalId, SignalMeta, SignalOut, SignalSpecRef};
use sim::{BacktestReport, FeeRates, FeeSchedule, LatencyTable, StrategyBacktest};
use strategy::{DirectionalStrategy, FillReport, OrderIntent, OrderKind, Strategy, StrategyConfig};

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

// ── ST-I-8e (C-004 C1): независимый оракул ДОСТАВКИ филлов стратегии ──────────────────
//
// Прежний ST-I-8b сравнивал позицию стратегии с `report.positions` — обе могли быть нулём
// у реализации, которая НИКОГДА не зовёт `strategy.on_fill(...)`. Спай ниже фиксирует
// КАЖДЫЙ вызов `on_fill` и его содержимое, поэтому падает, если `run()`:
//   (а) не докладывает филлы стратегии вовсе;
//   (б) неверно подписывает FillReport (сторона/инструмент/цена/размер не из SimFill);
//   (в) выдумывает филлы, которых не было на бирже.
// Спай НЕ зависит от DirectionalStrategy — он проверяет ТОЛЬКО мост StrategyBacktest.

/// Стратегия-шпион: шлёт заранее заданные интенты и записывает всё, что ей доложили.
struct SpyStrategy {
    /// seq события → интент, который надо отдать бирже.
    script: BTreeMap<u64, OrderIntent>,
    /// Полный журнал вызовов `on_fill` (порядок сохраняется).
    seen: Vec<FillReport>,
}

impl SpyStrategy {
    /// Нетто-позиция, посчитанная ИЗ ДОЛОЖЕННЫХ филлов (buy +, sell −).
    fn signed_net_e8(&self) -> i64 {
        self.seen
            .iter()
            .map(|f| match f.side {
                Side::Buy => f.qty_e8,
                Side::Sell => -f.qty_e8,
            })
            .sum()
    }
}

impl Strategy for SpyStrategy {
    fn on_event(&mut self, ev: &Event) -> Vec<OrderIntent> {
        self.script.get(&ev.seq).cloned().into_iter().collect()
    }
    fn on_fill(&mut self, fill: &FillReport) {
        self.seen.push(fill.clone());
    }
    fn position_e8(&self, _instrument: &Instrument) -> i64 {
        self.signed_net_e8()
    }
}

fn spy() -> SpyStrategy {
    let buy = OrderIntent {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        side: Side::Buy,
        price: 10_201_000_000, // 101.0 × 1.01 — маркетабельно
        qty: EDGE_SCALE,       // 1.0
        kind: OrderKind::Taker,
    };
    let sell = OrderIntent {
        side: Side::Sell,
        price: 9_900_000_000, // 100.0 × 0.99
        ..buy.clone()
    };
    SpyStrategy {
        script: BTreeMap::from([(2, buy), (8, sell)]),
        seen: Vec::new(),
    }
}

/// ST-I-8e: `StrategyBacktest::run` ОБЯЗАН доложить стратегии КАЖДЫЙ филл биржи, корректно
/// подписав его (instrument/side/price/qty/fee/ts из соответствующего `SimFill` и интента).
#[test]
fn st_i_8e_run_reports_every_fill_back_to_strategy() {
    let mut bt = StrategyBacktest::new(table(), fees(), 42);
    let mut s = spy();
    let report = bt.run(&events(), &mut s);

    // (а) доставка вообще произошла
    assert!(
        !report.fills.is_empty(),
        "фикстура обязана дать филлы (иначе оракул бессмысленен)"
    );
    assert_eq!(
        s.seen.len(),
        report.fills.len(),
        "стратегии обязан быть доложен КАЖДЫЙ филл биржи: доложено {}, исполнено {} \
         (пропуск on_fill → стратегия торгует по фантомной позиции)",
        s.seen.len(),
        report.fills.len()
    );

    // (б) подпись FillReport соответствует SimFill (цена/размер/комиссия/время) —
    // выдуманные или перепутанные филлы не пройдут.
    for (got, sim_fill) in s.seen.iter().zip(report.fills.iter()) {
        assert_eq!(got.instrument, btc(), "инструмент филла");
        assert_eq!(got.price_e8, sim_fill.price, "цена филла из SimFill");
        assert_eq!(got.qty_e8, sim_fill.qty, "размер филла из SimFill");
        assert_eq!(got.fee_e8, sim_fill.fee_e8, "комиссия филла из SimFill");
        assert_eq!(
            got.ts_mono_ns, sim_fill.ts_mono_ns,
            "время филла из SimFill"
        );
        assert!(got.qty_e8 > 0, "размер филла — положительная величина");
    }

    // (в) СТОРОНА восстановлена из интента (в SimFill стороны нет — её обязан подставить
    // мост order_meta). Реализация, подписывающая всё как Buy, здесь падает.
    let buys: i64 = s
        .seen
        .iter()
        .filter(|f| f.side == Side::Buy)
        .map(|f| f.qty_e8)
        .sum();
    let sells: i64 = s
        .seen
        .iter()
        .filter(|f| f.side == Side::Sell)
        .map(|f| f.qty_e8)
        .sum();
    assert!(buys > 0, "BUY-интент (seq 2) обязан дать BUY-филлы");
    assert!(
        sells > 0,
        "SELL-интент (seq 8) обязан дать SELL-филлы — сторона берётся из интента, \
         не из SimFill (там её нет)"
    );

    // Нетто из ДОЛОЖЕННЫХ филлов == нетто-позиция отчёта: два независимых источника истины.
    let reported = report.positions.get(&btc()).copied().unwrap_or(0);
    assert_eq!(
        s.signed_net_e8(),
        reported,
        "позиция отчёта обязана быть выводима из доложенных стратегии филлов"
    );
}

/// ST-I-8f (NO-LOOKAHEAD, замена переоценённого ST-I-5, C-004 M1): `run()` получает ВЕСЬ
/// срез событий — это и есть настоящая поверхность подглядывания. Мутируем ТОЛЬКО будущее
/// (события после seq=6): решения и исполнения в прошлом обязаны остаться бит-в-бит теми же.
/// Реализация, заглядывающая вперёд по срезу, здесь падает; префикс-стабильность (ST-I-5)
/// такое не ловит.
#[test]
fn st_i_8f_mutating_the_future_cannot_change_the_past() {
    let baseline = events();

    // Тот же поток, но у событий seq > 6 книга РАДИКАЛЬНО другая (цены ×2).
    let mutated: Vec<Event> = baseline
        .iter()
        .map(|e| {
            if e.seq <= 6 {
                return e.clone();
            }
            let EventKind::Md(md) = &e.kind else {
                return e.clone();
            };
            let MdPayload::L2Snapshot { ts_exch_ms, .. } = &md.payload else {
                return e.clone();
            };
            Event {
                kind: EventKind::md(
                    Venue::Binance,
                    "BTCUSDT",
                    MdPayload::L2Snapshot {
                        bids: vec![Level {
                            price: contracts::to_fixed(200.0),
                            size: contracts::to_fixed(10.0),
                        }],
                        asks: vec![Level {
                            price: contracts::to_fixed(202.0),
                            size: contracts::to_fixed(10.0),
                        }],
                        ts_exch_ms: *ts_exch_ms,
                    },
                ),
                ..e.clone()
            }
        })
        .collect();

    let run_stream = |evs: &[Event]| -> Vec<sim::SimFill> {
        let mut bt = StrategyBacktest::new(table(), fees(), 7);
        let mut s = strat();
        bt.run(evs, &mut s)
            .fills
            .into_iter()
            .filter(|f| f.seq <= 6) // только «прошлое»
            .collect()
    };

    let past_baseline = run_stream(&baseline);
    let past_mutated = run_stream(&mutated);

    assert!(
        !past_baseline.is_empty(),
        "в прошлом обязаны быть исполнения (иначе оракул пуст)"
    );
    assert_eq!(
        past_baseline, past_mutated,
        "изменение БУДУЩЕГО изменило исполнения в ПРОШЛОМ — значит, где-то читается \
         срез событий вперёд (нарушение SM-I-4 / no-lookahead)"
    );
}
