//! OBI RED-тесты (sacred; docs/fa/signals.md §T): SG-I-1, SG-I-2 + сигнал-специфика.
//! Имя файла — контракт SG-I-10: src/obi.rs ⇒ tests/test_obi_determinism.rs.

use contracts::{to_fixed, Event, EventKind, Level, MdPayload, Venue};
use signals::obi::{Obi, ObiMode, ObiParams};
use signals::{RegistryStatus, Signal, SignalId, SIGNAL_VALUE_SCALE};

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

fn obi(mode: ObiMode, theta: f64, status: RegistryStatus) -> Obi {
    Obi::new(
        SignalId::parse("S-001-obi-asym").expect("валидный id"),
        1,
        status,
        ObiParams {
            mode,
            theta_e8: to_fixed(theta),
            horizon_ms: 1_000,
            venue: Venue::Binance,
            symbol: "BTCUSDT".into(),
        },
    )
}

/// Тяжёлый BID-перекос: top-5 bid-глубина ≫ ask.
fn heavy_bid_events() -> Vec<Event> {
    (0..10u64)
        .map(|i| {
            snap(
                i + 1,
                1_000 + i * 100,
                &[(100.0, 50.0), (99.9, 40.0), (99.8, 30.0)],
                &[(100.1, 1.0), (100.2, 1.0)],
            )
        })
        .collect()
}

fn balanced_events() -> Vec<Event> {
    (0..10u64)
        .map(|i| {
            snap(
                i + 1,
                1_000 + i * 100,
                &[(100.0, 10.0), (99.9, 10.0)],
                &[(100.1, 10.0), (100.2, 10.0)],
            )
        })
        .collect()
}

#[test]
fn test_obi_determinism() {
    // SG-I-2: одинаковая последовательность Event → бит-идентичные SignalOut.
    let evs = heavy_bid_events();
    let run = |mut s: Obi| -> Vec<_> { evs.iter().map(|e| s.on_event(e)).collect() };
    let a = run(obi(
        ObiMode::TopN { n_levels: 5 },
        0.2,
        RegistryStatus::Candidate,
    ));
    let b = run(obi(
        ObiMode::TopN { n_levels: 5 },
        0.2,
        RegistryStatus::Candidate,
    ));
    assert_eq!(a, b);
    assert!(
        a.iter().any(Option::is_some),
        "анти-плацебо: перекошенный стакан обязан эмитить сигнал при θ=0.2"
    );
}

#[test]
fn test_obi_no_signal_below_theta() {
    // FA §7: ниже порога — None («нет мнения»), не «мнение=0».
    let mut s = obi(
        ObiMode::TopN { n_levels: 5 },
        0.5,
        RegistryStatus::Candidate,
    );
    for ev in balanced_events() {
        assert_eq!(
            s.on_event(&ev),
            None,
            "сбалансированная книга при θ=0.5 → None"
        );
    }
}

#[test]
fn test_obi_direction_range_and_status_tag() {
    // D1: value — направленный score ∈ [−1e8, +1e8]; BID-перекос → value > 0.
    // SG-I-7 (половина тега): status пробрасывается честно.
    let mut s = obi(ObiMode::TopN { n_levels: 5 }, 0.2, RegistryStatus::Paper);
    let outs: Vec<_> = heavy_bid_events()
        .iter()
        .filter_map(|e| s.on_event(e))
        .collect();
    assert!(!outs.is_empty());
    for o in &outs {
        assert!(
            o.value > 0,
            "BID-перекос → BUY (value>0), получили {}",
            o.value
        );
        assert!(o.value.abs() <= SIGNAL_VALUE_SCALE, "D1: |value| ≤ 1e8");
        assert_eq!(
            o.status,
            RegistryStatus::Paper,
            "SG-I-7: тег статуса честный"
        );
        assert_eq!(o.signal_id.as_str(), "S-001-obi-asym");
        assert_eq!(
            o.meta.horizon_ms, 1_000,
            "D2: horizon — метаданные для downstream"
        );
    }
    // ts берётся из события, не из часов (SG-I-4 поведенчески)
    let mut s2 = obi(ObiMode::TopN { n_levels: 5 }, 0.2, RegistryStatus::Paper);
    let ev = &heavy_bid_events()[0];
    let out = s2.on_event(ev).expect("первый же снапшот перекошен");
    assert_eq!(out.ts_event_mono_ns, ev.ts_mono_ns);
}

#[test]
fn test_obi_bands_mode_uses_price_bands() {
    // Трек B: полосы d_bid=3% / d_ask=8% от mid. Bid-глубина сосредоточена в 3%-полосе;
    // ask-глубина в основном ЗА 8%-полосой → полосная асимметрия → BUY.
    let evs: Vec<Event> = (0..5u64)
        .map(|i| {
            snap(
                i + 1,
                1_000 + i * 100,
                &[(100.0, 10.0), (98.0, 30.0)], // всё в пределах 3% от mid≈100
                &[(100.2, 1.0), (115.0, 200.0)], // 115 — за 8%-полосой, не считается
            )
        })
        .collect();
    let mut s = obi(
        ObiMode::Bands {
            d_bid_pct: 0.03,
            d_ask_pct: 0.08,
        },
        0.2,
        RegistryStatus::Candidate,
    );
    let outs: Vec<_> = evs.iter().filter_map(|e| s.on_event(e)).collect();
    assert!(!outs.is_empty(), "полосная асимметрия обязана дать сигнал");
    assert!(outs.iter().all(|o| o.value > 0), "bid-полоса тяжелее → BUY");
}

#[test]
fn test_obi_no_lookahead() {
    // SG-I-1: outs на префиксе не зависят от хвоста потока.
    let full = heavy_bid_events();
    let prefix = &full[..5];
    let run = |evs: &[Event]| -> Vec<_> {
        let mut s = obi(
            ObiMode::TopN { n_levels: 5 },
            0.2,
            RegistryStatus::Candidate,
        );
        evs.iter().map(|e| s.on_event(e)).collect()
    };
    let on_full = run(&full);
    let on_prefix = run(prefix);
    assert_eq!(
        &on_full[..5],
        &on_prefix[..],
        "SG-I-1: префикс самодостаточен"
    );
}
