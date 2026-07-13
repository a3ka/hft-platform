//! RED-оракулы strategy (ST-I-1..5, docs/fa/strategy-brain.md §7). SACRED — architect-only.
//!
//! Анти-плацебо:
//! - ST-I-2 бьёт по «эмитим интент на каждом сигнале» (наивный код шлёт ордер, даже когда
//!   позиция уже равна цели);
//! - ST-I-3 бьёт по «нет учёта ордера в полёте» (наивный diff шлёт второй ордер на том же
//!   таргете, пока филл не пришёл → двойная позиция в live);
//! - ST-I-5 (no-lookahead) падает на любой реализации, подглядывающей вперёд по потоку.
//!
//! Маркетабельная цена (контракт v1, целочисленно, i128):
//!   BUY  price = best_ask · (10_000 + margin_bp) / 10_000
//!   SELL price = best_bid · (10_000 − margin_bp) / 10_000
//! Нет книги / нет лучшей котировки → интент НЕ эмитится (не «по любой цене»).

use std::collections::BTreeMap;

use alpha::{Instrument, LinearAlpha, SignalWeight, EDGE_SCALE};
use contracts::{Event, EventKind, Level, MdPayload, Side, Venue};
use portfolio::RiskBudget;
use signals::{RegistryStatus, Signal, SignalId, SignalMeta, SignalOut, SignalSpecRef};
use strategy::{DirectionalStrategy, FillReport, OrderIntent, OrderKind, Strategy, StrategyConfig};

const MS: u64 = 1_000_000;
const SIG: &str = "S-001-obi-asym";
const HORIZON_MS: i64 = 10_000;

fn btc() -> Instrument {
    Instrument::new(Venue::Binance, "BTCUSDT")
}

/// Сигнал-скрипт: на событии с seq=k отдаёт заданное значение (или ничего).
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
            meta: SignalMeta {
                horizon_ms: HORIZON_MS,
            },
        })
    }
    fn spec(&self) -> SignalSpecRef {
        SignalSpecRef {
            id: self.id.clone(),
            version: 1,
        }
    }
}

fn book_event(seq: u64, ts_mono_ns: u64) -> Event {
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

fn cfg() -> StrategyConfig {
    StrategyConfig {
        min_order_e8: 1_000_000, // 0.01
        intent_ttl_ms: 1_000,
        marketable_margin_bp: 100, // 1%
        kind: OrderKind::Taker,
    }
}

/// max_position = 1.0 → target = edge (в единицах ×1e8).
fn strat(script: BTreeMap<u64, i64>) -> DirectionalStrategy {
    let sig = ScriptedSignal {
        id: SignalId::parse(SIG).expect("valid"),
        script,
    };
    let alpha = LinearAlpha::new(vec![SignalWeight {
        signal_id: SignalId::parse(SIG).expect("valid"),
        instrument: btc(),
        weight_e8: EDGE_SCALE,
    }])
    .expect("weights valid");
    let budget = RiskBudget::new(vec![(btc(), EDGE_SCALE)]).expect("limit valid");
    DirectionalStrategy::new(vec![Box::new(sig)], Box::new(alpha), budget, cfg())
        .expect("config valid")
}

fn fill(side: Side, qty_e8: i64, ts_mono_ns: u64) -> FillReport {
    FillReport {
        instrument: btc(),
        side,
        price_e8: contracts::to_fixed(101.0),
        qty_e8,
        fee_e8: 0,
        ts_mono_ns,
    }
}

/// ST-I-1: diff current→target. 0 → +1.0 даёт РОВНО один BUY на 1.0 по маркетабельной цене;
/// после исполнения +1.0 → target 0 даёт SELL на 1.0.
#[test]
fn st_i_1_diff_emits_intent_to_reach_target() {
    let mut s = strat(BTreeMap::from([(1, EDGE_SCALE), (2, 0)]));

    let intents = s.on_event(&book_event(1, 1_000 * MS));
    assert_eq!(intents.len(), 1, "0 → +1.0 = ровно один интент");
    assert_eq!(
        intents[0],
        OrderIntent {
            venue: Venue::Binance,
            symbol: "BTCUSDT".to_string(),
            side: Side::Buy,
            price: 10_201_000_000, // 101.0 × 1.01
            qty: EDGE_SCALE,       // 1.0
            kind: OrderKind::Taker,
        }
    );

    // Исполнение входа: позиция = +1.0 (двигать позицию может ТОЛЬКО филл).
    s.on_fill(&fill(Side::Buy, EDGE_SCALE, 1_100 * MS));
    assert_eq!(s.position_e8(&btc()), EDGE_SCALE);

    // Сигнал сказал «ноль» → target 0 → выход РОВНО набранным объёмом.
    let intents = s.on_event(&book_event(2, 2_000 * MS));
    assert_eq!(intents.len(), 1, "+1.0 → 0 = один интент выхода");
    assert_eq!(intents[0].side, Side::Sell);
    assert_eq!(intents[0].qty, EDGE_SCALE);
    assert_eq!(intents[0].price, 9_900_000_000, "100.0 × 0.99");
}

/// ST-I-2: target == current → интентов НЕТ. Также: дельта меньше min_order_e8 → нет интента
/// (деадбенд против дребезга на шуме edge).
#[test]
fn st_i_2_no_intent_when_target_equals_current() {
    let mut s = strat(BTreeMap::from([
        (1, EDGE_SCALE),
        (2, EDGE_SCALE),
        (3, EDGE_SCALE - 500_000), // отклонение 0.005 < min_order 0.01
    ]));

    let first = s.on_event(&book_event(1, 1_000 * MS));
    assert_eq!(first.len(), 1);
    s.on_fill(&fill(Side::Buy, EDGE_SCALE, 1_100 * MS));

    let same = s.on_event(&book_event(2, 2_000 * MS));
    assert!(
        same.is_empty(),
        "цель достигнута → ордеров быть не должно (наивный код шлёт ещё один)"
    );

    let noise = s.on_event(&book_event(3, 3_000 * MS));
    assert!(
        noise.is_empty(),
        "дельта < min_order_e8 → деадбенд, интента нет"
    );
}

/// ST-I-3: ордер в полёте не дублируется; после intent_ttl_ms без филла — переотправка.
/// Наивный diff (без in_flight) шлёт второй ордер на seq=2 → двойная позиция в live.
#[test]
fn st_i_3_in_flight_is_not_duplicated_and_expires_by_event_time() {
    let mut s = strat(BTreeMap::from([
        (1, EDGE_SCALE),
        (2, EDGE_SCALE),
        (3, EDGE_SCALE),
    ]));

    let first = s.on_event(&book_event(1, 1_000 * MS));
    assert_eq!(first.len(), 1, "первый интент ушёл");

    // Филла ещё нет; 500ms спустя (< ttl=1000ms) — повторного интента быть НЕ должно.
    let dup = s.on_event(&book_event(2, 1_500 * MS));
    assert!(
        dup.is_empty(),
        "ордер в полёте: дубля быть не должно (иначе двойная позиция)"
    );

    // Прошло > ttl по EVENT-TIME, филла так и нет → интент считается умершим, переотправка.
    let retry = s.on_event(&book_event(3, 2_600 * MS));
    assert_eq!(retry.len(), 1, "после ttl без филла — переотправка");
    assert_eq!(retry[0].side, Side::Buy);
    assert_eq!(retry[0].qty, EDGE_SCALE);
}

/// ST-I-4: детерминизм — два независимых прогона одного потока → идентичные интенты.
#[test]
fn st_i_4_replay_is_deterministic() {
    let script: BTreeMap<u64, i64> = (1..=40u64)
        .filter(|i| i % 4 == 0)
        .map(|i| (i, ((i as i64 * 13) % 200 - 100) * 1_000_000))
        .collect();

    let run = || -> Vec<OrderIntent> {
        let mut s = strat(script.clone());
        let mut all = Vec::new();
        for i in 1..=40u64 {
            all.extend(s.on_event(&book_event(i, i * 200 * MS)));
        }
        all
    };

    let a = run();
    let b = run();
    assert!(!a.is_empty(), "прогон обязан произвести интенты");
    assert_eq!(a, b, "DET: два прогона одного потока обязаны совпасть");
}

/// ST-I-5 (PREFIX-STABILITY / REPLAY-DETERMINISM — честная формулировка, C-004 M1):
/// интенты, произведённые на ПРЕФИКСЕ потока, обязаны быть в точности префиксом интентов
/// полного потока. Это НЕ доказательство future-blindness: `on_event(&Event)` физически не
/// получает будущего, так что подглядывать здесь нечем — тест закрывает скрытое состояние,
/// зависящее от длины прогона (буферы, «прогрев», батчинг).
/// НАСТОЯЩИЙ no-lookahead оракул живёт там, где есть поверхность подглядывания — у харнесса,
/// получающего ВЕСЬ срез событий: `crates/sim/tests/red_strategy_backtest.rs` ST-I-8f
/// (мутация будущего не меняет прошлое).
#[test]
fn st_i_5_prefix_stability_and_replay_determinism() {
    let script: BTreeMap<u64, i64> = (1..=30u64)
        .filter(|i| i % 3 == 0)
        .map(|i| (i, ((i as i64 * 17) % 200 - 100) * 1_000_000))
        .collect();

    let run = |upto: u64| -> Vec<OrderIntent> {
        let mut s = strat(script.clone());
        let mut all = Vec::new();
        for i in 1..=upto {
            all.extend(s.on_event(&book_event(i, i * 200 * MS)));
        }
        all
    };

    let full = run(30);
    let prefix = run(12);
    assert!(!prefix.is_empty(), "префикс обязан произвести интенты");
    assert_eq!(
        prefix[..],
        full[..prefix.len()],
        "решения на префиксе обязаны совпадать с решениями полного потока — \
         иначе стратегия видит будущее"
    );
}

/// Конфиг стратегии fail-closed: невалидные значения → Err, не «разумные дефолты».
#[test]
fn st_config_validation_is_fail_closed() {
    let mk = |c: StrategyConfig| {
        let sig = ScriptedSignal {
            id: SignalId::parse(SIG).expect("valid"),
            script: BTreeMap::new(),
        };
        let alpha = LinearAlpha::new(vec![SignalWeight {
            signal_id: SignalId::parse(SIG).expect("valid"),
            instrument: btc(),
            weight_e8: EDGE_SCALE,
        }])
        .expect("valid");
        let budget = RiskBudget::new(vec![(btc(), EDGE_SCALE)]).expect("valid");
        DirectionalStrategy::new(vec![Box::new(sig)], Box::new(alpha), budget, c)
    };

    assert!(
        mk(StrategyConfig {
            min_order_e8: 0,
            ..cfg()
        })
        .is_err(),
        "min_order_e8 = 0 → Err (иначе дребезг нулевыми ордерами)"
    );
    assert!(
        mk(StrategyConfig {
            intent_ttl_ms: 0,
            ..cfg()
        })
        .is_err(),
        "intent_ttl_ms = 0 → Err"
    );
    assert!(
        mk(StrategyConfig {
            marketable_margin_bp: -1,
            ..cfg()
        })
        .is_err(),
        "отрицательный запас маркетабельности → Err"
    );
}
