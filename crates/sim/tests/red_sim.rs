//! RED-suite sim (sacred, architect-only; docs/fa/sim.md §T).
//! SM-I-1,2,4,5,6,7,8,10 — обязаны падать на todo!-заглушках до реализации.

use contracts::{to_fixed, Event, EventKind, Level, MdPayload, Side, Venue};
use sim::{
    fill_model, p4_gate, BacktestExchange, DivergenceMetric, DivergenceTolerance, FeeRates,
    FeeSchedule, FillDecision, GateBlocked, LatencyTable, OrderIntent, OrderKind, QueueState,
    SimError, SimFill, SimOrder, SplitMix64, TradedTick,
};

// ── фикстуры ────────────────────────────────────────────────────────────────

fn mk_order(price: f64, qty: f64, ahead: f64, submitted_seq: u64) -> SimOrder {
    SimOrder {
        id: 1,
        intent: OrderIntent {
            venue: Venue::Binance,
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            price: to_fixed(price),
            qty: to_fixed(qty),
            kind: OrderKind::Maker,
        },
        submitted_seq,
        effective_ts_mono_ns: 0,
        queue: QueueState {
            ahead: to_fixed(ahead),
            cum_traded: 0,
            filled: 0,
        },
    }
}

fn tick(price: f64, qty: f64, seq: u64) -> TradedTick {
    TradedTick {
        price: to_fixed(price),
        qty: to_fixed(qty),
        side: Side::Sell,
        seq,
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

fn snap(seq: u64, ts_ms: u64, bids: &[(f64, f64)], asks: &[(f64, f64)]) -> Event {
    Event {
        seq,
        ts_mono_ns: ts_ms * 1_000_000,
        ts_wall_ms: ts_ms as i64,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: bids.iter().map(|&(p, s)| lvl(p, s)).collect(),
                asks: asks.iter().map(|&(p, s)| lvl(p, s)).collect(),
                ts_exch_ms: ts_ms as i64,
            },
        ),
    }
}

fn trade(seq: u64, ts_ms: u64, price: f64, qty: f64) -> Event {
    Event {
        seq,
        ts_mono_ns: ts_ms * 1_000_000,
        ts_wall_ms: ts_ms as i64,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(price),
                size: to_fixed(qty),
                side: Side::Sell,
                ts_exch_ms: ts_ms as i64,
            },
        ),
    }
}

fn table() -> LatencyTable {
    let mut t = LatencyTable::new();
    t.insert_samples(
        Venue::Binance,
        "BTCUSDT",
        vec![1_000_000], // δ_submit = 1мс
        vec![1_000_000],
        vec![500_000],
        "synthetic-test-fixture",
    );
    t
}

fn fee_sched() -> FeeSchedule {
    let mut f = FeeSchedule::new();
    f.insert_rates(
        Venue::Binance,
        FeeRates {
            maker_rate_e8: 10_000, // 0.0001
            taker_rate_e8: 45_000, // 0.00045
        },
    );
    f
}

// ── SM-I-6 / SM-I-1: пессимистичная очередь ────────────────────────────────

#[test]
fn test_maker_fill_requires_traded_volume_exceeds_depth_ahead() {
    // SM-I-6: fill ТОЛЬКО когда cum_traded ПРЕВЫСИЛ ahead; равенства недостаточно.
    let o = mk_order(100.0, 5.0, 10.0, 0);
    let (q1, d1) = fill_model::on_traded_tick(&o, &tick(100.0, 10.0, 1));
    assert_eq!(d1, FillDecision::NoFill, "traded == ahead → NoFill");
    assert_eq!(q1.cum_traded, to_fixed(10.0));

    let mut o2 = o.clone();
    o2.queue = q1;
    let (q2, d2) = fill_model::on_traded_tick(&o2, &tick(100.0, 0.5, 2));
    assert_eq!(
        d2,
        FillDecision::Partial { qty: to_fixed(0.5) },
        "излишек над ahead достаётся нам, но НЕ больше излишка"
    );
    assert_eq!(q2.filled, to_fixed(0.5));
}

#[test]
fn test_fill_never_exceeds_pessimistic_bound() {
    // SM-I-1: суммарный fill ≤ min(наш qty, max(0, cum_traded − ahead)) на каждом шаге.
    let mut o = mk_order(100.0, 5.0, 10.0, 0);
    let ticks = [3.0, 4.0, 4.0, 2.0, 8.0]; // cum: 3,7,11,13,21
    let mut cum = 0i64;
    let mut filled_total = 0i64;
    for (i, t) in ticks.iter().enumerate() {
        let tk = tick(100.0, *t, (i + 1) as u64);
        cum += to_fixed(*t);
        let (q, d) = fill_model::on_traded_tick(&o, &tk);
        match d {
            FillDecision::Partial { qty } | FillDecision::Full { qty } => filled_total += qty,
            FillDecision::NoFill => {}
        }
        o.queue = q;
        let bound = (cum - to_fixed(10.0)).max(0).min(to_fixed(5.0));
        assert!(
            filled_total <= bound,
            "fill {filled_total} превысил пессимистичную границу {bound} на тике {i}"
        );
        assert_eq!(
            q.filled, filled_total,
            "QueueState.filled расходится с фактом"
        );
    }
    assert_eq!(
        filled_total,
        to_fixed(5.0),
        "к концу должны быть исполнены целиком (анти-плацебо)"
    );
}

#[test]
fn test_tick_at_other_price_does_not_advance_queue() {
    let o = mk_order(100.0, 5.0, 1.0, 0);
    let (q, d) = fill_model::on_traded_tick(&o, &tick(101.0, 50.0, 1));
    assert_eq!(d, FillDecision::NoFill);
    assert_eq!(q, o.queue, "чужая цена не двигает нашу очередь");
}

#[test]
fn test_zero_traded_volume_no_fill() {
    // FA §3: traded-объём = 0 → NoFill безусловно (даже при ahead=0).
    let o = mk_order(100.0, 5.0, 0.0, 0);
    let (_, d) = fill_model::on_traded_tick(&o, &tick(100.0, 0.0, 1));
    assert_eq!(d, FillDecision::NoFill);
}

#[test]
fn test_tick_before_or_at_submission_ignored() {
    // SM-I-4: события с seq ≤ submitted_seq не участвуют в решении.
    let o = mk_order(100.0, 5.0, 0.0, 10);
    let (q, d) = fill_model::on_traded_tick(&o, &tick(100.0, 50.0, 10));
    assert_eq!(d, FillDecision::NoFill);
    assert_eq!(q, o.queue);
}

// ── SM-I-5: отмены впереди не освобождают место ─────────────────────────────

#[test]
fn test_cancel_ahead_does_not_improve_fill_probability() {
    let q = QueueState {
        ahead: to_fixed(10.0),
        cum_traded: to_fixed(3.0),
        filled: 0,
    };
    let q2 = fill_model::on_cancel_ahead(q, to_fixed(9.0));
    assert_eq!(q2, q, "SM-I-5: cancel-ahead обязан быть тождеством");
}

// ── taker: только видимая книга ─────────────────────────────────────────────

#[test]
fn test_taker_eats_visible_book_only() {
    let mut b = book::OrderBook::new();
    b.apply_snapshot(
        &[lvl(99.0, 1.0)],
        &[lvl(101.0, 1.0), lvl(102.0, 2.0), lvl(105.0, 50.0)],
    );
    // buy 5.0 с лимитом 102 → 1@101 + 2@102; остаток 2.0 НЕ исполняется (105 за лимитом)
    let fills = fill_model::taker_fills(&b, Side::Buy, to_fixed(5.0), to_fixed(102.0));
    let total: i64 = fills.iter().map(|&(_, q)| q).sum();
    assert_eq!(total, to_fixed(3.0));
    assert_eq!(fills[0].0, to_fixed(101.0), "начинаем с top-of-book");
    assert!(
        fills.windows(2).all(|w| w[0].0 <= w[1].0),
        "цены монотонно ухудшаются"
    );
}

// ── SM-I-2: детерминизм при (journal, seed) ────────────────────────────────

fn scenario_events() -> Vec<Event> {
    vec![
        snap(1, 1_000, &[(100.0, 2.0), (99.0, 5.0)], &[(101.0, 2.0)]),
        snap(2, 1_010, &[(100.0, 2.0), (99.0, 5.0)], &[(101.0, 2.0)]),
        trade(3, 1_050, 100.0, 2.0),
        trade(4, 1_060, 100.0, 1.5), // cum 3.5 > ahead 2.0 → наш fill 1.0
        snap(5, 1_070, &[(100.0, 1.0)], &[(101.0, 2.0)]),
    ]
}

fn run_scenario(seed: u64) -> Vec<SimFill> {
    let mut ex = BacktestExchange::new(table(), fee_sched(), seed);
    let mut fills = Vec::new();
    for ev in scenario_events() {
        fills.extend(ex.on_event(&ev));
        if ev.seq == 2 {
            // maker buy 1.0 @ 100 — встаём в хвост уровня (ahead = 2.0)
            ex.submit(OrderIntent {
                venue: Venue::Binance,
                symbol: "BTCUSDT".into(),
                side: Side::Buy,
                price: to_fixed(100.0),
                qty: to_fixed(1.0),
                kind: OrderKind::Maker,
            })
            .expect("submit после первого события обязан пройти");
        }
    }
    fills
}

#[test]
fn test_replay_deterministic_given_seed() {
    let a = run_scenario(42);
    let b = run_scenario(42);
    assert_eq!(
        a, b,
        "SM-I-2: одинаковый (поток, seed) → бит-идентичные fills"
    );
    assert!(
        !a.is_empty(),
        "анти-плацебо: сценарий обязан производить fills (cum 3.5 > ahead 2.0)"
    );
    let filled: i64 = a.iter().map(|f| f.qty).sum();
    assert_eq!(filled, to_fixed(1.0), "исполнен ровно наш 1.0");
    assert!(a.iter().all(|f| f.maker), "в сценарии только maker-fill");
    assert!(
        a.iter().all(|f| f.fee_e8 != 0),
        "комиссия обязана считаться (не нулевая)"
    );
}

// ── SM-I-4: no lookahead ────────────────────────────────────────────────────

#[test]
fn test_fill_model_no_lookahead() {
    // Два потока с общим префиксом (seq ≤ 4) и разными хвостами:
    // fills с seq ≤ 4 обязаны совпасть — будущее не влияет на прошлое.
    let prefix = scenario_events(); // 5 событий, последний seq=5
    let mut stream_a = prefix.clone();
    stream_a.push(trade(6, 2_000, 100.0, 50.0));
    let mut stream_b = prefix;
    stream_b.push(snap(6, 2_000, &[(90.0, 1.0)], &[(120.0, 1.0)]));

    let run = |evs: &[Event]| -> Vec<SimFill> {
        let mut ex = BacktestExchange::new(table(), fee_sched(), 7);
        let mut fills = Vec::new();
        for ev in evs {
            fills.extend(ex.on_event(ev));
            if ev.seq == 2 {
                ex.submit(OrderIntent {
                    venue: Venue::Binance,
                    symbol: "BTCUSDT".into(),
                    side: Side::Buy,
                    price: to_fixed(100.0),
                    qty: to_fixed(1.0),
                    kind: OrderKind::Maker,
                })
                .unwrap();
            }
        }
        fills
    };
    let cut = |v: Vec<SimFill>| -> Vec<SimFill> { v.into_iter().filter(|f| f.seq <= 4).collect() };
    let a = cut(run(&stream_a));
    let b = cut(run(&stream_b));
    assert_eq!(
        a, b,
        "SM-I-4: решения на префиксе зависят только от префикса"
    );
    assert!(
        !a.is_empty(),
        "анти-плацебо: fill обязан случиться внутри префикса"
    );
}

// ── SM-I-7 / SM-I-8: латентность только из таблицы; отсутствие = Halt ──────

#[test]
fn test_latency_draw_deterministic_and_from_samples() {
    let mut t = LatencyTable::new();
    let samples = vec![1_000_000u64, 2_000_000, 3_000_000];
    t.insert_samples(
        Venue::Binance,
        "BTCUSDT",
        samples.clone(),
        samples.clone(),
        samples.clone(),
        "synthetic",
    );
    let mut r1 = SplitMix64::new(9);
    let mut r2 = SplitMix64::new(9);
    let d1 = t.draw(Venue::Binance, "BTCUSDT", &mut r1).unwrap();
    let d2 = t.draw(Venue::Binance, "BTCUSDT", &mut r2).unwrap();
    assert_eq!(d1, d2, "тот же seed → тот же draw (SM-I-2)");
    assert!(
        samples.contains(&d1.delta_submit_ns),
        "SM-I-7: значение — из ИЗМЕРЕННЫХ сэмплов, не выдуманное"
    );
}

#[test]
fn test_missing_latency_distribution_halts_startup() {
    // SM-I-8: незнакомый инструмент → Err, никакого default.
    let t = table();
    let mut rng = SplitMix64::new(1);
    assert!(
        matches!(
            t.draw(Venue::Hyperliquid, "ETH", &mut rng),
            Err(SimError::MissingLatency { .. })
        ),
        "draw по инструменту без распределения обязан падать закрыто"
    );

    // и submit в BacktestExchange на неизвестный инструмент → fail-closed
    let mut ex = BacktestExchange::new(table(), fee_sched(), 5);
    ex.on_event(&snap(1, 1_000, &[(100.0, 1.0)], &[(101.0, 1.0)]));
    let res = ex.submit(OrderIntent {
        venue: Venue::Hyperliquid,
        symbol: "ETH".into(),
        side: Side::Buy,
        price: to_fixed(100.0),
        qty: to_fixed(1.0),
        kind: OrderKind::Maker,
    });
    assert!(
        matches!(
            res,
            Err(SimError::MissingLatency { .. }) | Err(SimError::MissingFees { .. })
        ),
        "submit без измеренной латентности/тарифа обязан вернуть Err"
    );
}

#[test]
fn test_missing_fee_schedule_halts() {
    // FA §7: нет тарифа → Err (не «нулевая комиссия»).
    let f = fee_sched();
    assert!(matches!(
        f.fee_e8(Venue::Hyperliquid, "BTC", true, to_fixed(1000.0)),
        Err(SimError::MissingFees { .. })
    ));
}

// ── SM-I-10: divergence-отчёт — обязательные ворота P4 ─────────────────────

#[test]
fn test_divergence_report_required_before_p4_gate_passes() {
    let tol = DivergenceTolerance {
        max_fill_rate_delta: 0.10,
        max_pnl_delta_e8: to_fixed(10.0),
    };
    assert_eq!(
        p4_gate(None, &tol),
        Err(GateBlocked::MissingReport),
        "SM-I-10: без отчёта ворота P4 закрыты БЕЗУСЛОВНО"
    );
    let ok = DivergenceMetric {
        window_ms: 60_000,
        fill_rate_delta: 0.05,
        pnl_delta_e8: to_fixed(1.0),
    };
    assert_eq!(p4_gate(Some(&ok), &tol), Ok(()));
    let bad = DivergenceMetric {
        window_ms: 60_000,
        fill_rate_delta: 0.50,
        pnl_delta_e8: 0,
    };
    assert!(matches!(
        p4_gate(Some(&bad), &tol),
        Err(GateBlocked::OutOfTolerance { .. })
    ));
}

// ── FA §5: queue ahead = объём на НАШЕМ ценовом уровне (SVR-резолюция) ──────

#[test]
fn test_maker_ahead_uses_our_price_level_not_top() {
    // Ордер стоит НЕ на топе: bid 100.0(2.0) — топ, наш maker buy 1.0 @ 99.0,
    // где видимый объём 5.0. Если бы ahead ошибочно брался с ЛУЧШЕГО уровня (2.0),
    // трейд qty 5.0 по 99.0 дал бы fill уже на seq=3. Правильно (ahead=5.0 на нашей
    // цене): cum 5.0 == ahead → NoFill; fill начинается только после ПРЕВЫШЕНИЯ.
    let evs = vec![
        snap(1, 1_000, &[(100.0, 2.0), (99.0, 5.0)], &[(101.0, 2.0)]),
        snap(2, 1_010, &[(100.0, 2.0), (99.0, 5.0)], &[(101.0, 2.0)]),
        trade(3, 1_050, 99.0, 5.0),  // cum 5.0 == ahead 5.0 → NoFill
        trade(4, 1_060, 99.0, 0.5),  // cum 5.5 → fill 0.5
        trade(5, 1_070, 99.0, 10.0), // остаток 0.5
    ];
    let mut ex = BacktestExchange::new(table(), fee_sched(), 3);
    let mut fills = Vec::new();
    for ev in &evs {
        fills.extend(ex.on_event(ev));
        if ev.seq == 2 {
            ex.submit(OrderIntent {
                venue: Venue::Binance,
                symbol: "BTCUSDT".into(),
                side: Side::Buy,
                price: to_fixed(99.0),
                qty: to_fixed(1.0),
                kind: OrderKind::Maker,
            })
            .unwrap();
        }
    }
    assert!(
        fills.iter().all(|f| f.seq != 3),
        "fill на seq=3 означает, что ahead взят с ЛУЧШЕГО уровня, а не с нашего (FA §5)"
    );
    let first = fills.first().expect("fill обязан произойти на seq=4");
    assert_eq!(first.seq, 4);
    assert_eq!(first.qty, to_fixed(0.5));
    let total: i64 = fills.iter().map(|f| f.qty).sum();
    assert_eq!(total, to_fixed(1.0));
}
