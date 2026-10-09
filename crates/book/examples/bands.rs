//! bands — сверка с платформенным BID/ASK индикатором. По последнему полному снапшоту
//! книги каждого символа печатает кумулятивный нотионал BID/ASK (USD) на полосах
//! 1.5/3/5/8/15/30/60% + DIFF(3B-8A). cargo run --example bands -p book -- <journal-dir>

use std::collections::HashMap;

use book::Books;
use contracts::{EventKind, MdPayload, Side, Venue};

const BANDS: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./journal-data".into());
    let evs = journal::read_all(&dir).expect("read journal");

    // Держим последнюю книгу каждого символа (проигрывая все снапшоты).
    let mut books = Books::new();
    let mut seen: HashMap<(Venue, String), u64> = HashMap::new();
    for e in &evs {
        if let EventKind::Md(m) = &e.kind {
            if matches!(m.payload, MdPayload::L2Snapshot { .. }) {
                books.apply(m);
                *seen.entry((m.venue, m.symbol.clone())).or_default() += 1;
            }
        }
    }

    let mut keys: Vec<_> = seen.keys().cloned().collect();
    keys.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    for k in keys {
        let Some(ob) = books.get(k.0, &k.1) else {
            continue;
        };
        let (Some(bb), Some(ba)) = (ob.best_bid(), ob.best_ask()) else {
            continue;
        };
        println!(
            "\n=== {:?}/{} (снапшотов: {}, mid={:.2}, уровней bid={}/ask={}, глубина bid={:.2}%/ask={:.2}%) ===",
            k.0, k.1, seen[&k],
            contracts::from_fixed(ob.mid().unwrap()),
            ob.n_levels(Side::Buy), ob.n_levels(Side::Sell),
            100.0 * ob.max_reach_pct(Side::Buy).unwrap_or(0.0),
            100.0 * ob.max_reach_pct(Side::Sell).unwrap_or(0.0),
        );
        let _ = (bb, ba);
        println!(
            "{:>6}  {:>16}  {:>16}  {:>16}",
            "полоса", "BID $", "ASK $", "DIFF(B-A) $"
        );
        for pct in BANDS {
            let bid = ob.notional_within(Side::Buy, pct);
            let ask = ob.notional_within(Side::Sell, pct);
            println!(
                "{:>5.1}%  {:>16}  {:>16}  {:>16}",
                pct * 100.0,
                fmt_usd(bid),
                fmt_usd(ask),
                fmt_usd(bid - ask)
            );
        }
        // Твой оригинальный сигнал: DIFF(BID3 − ASK8).
        let diff_3b_8a = ob.notional_within(Side::Buy, 0.03) - ob.notional_within(Side::Sell, 0.08);
        println!("DIFF 3B-8A (твой сигнал) = {}", fmt_usd(diff_3b_8a));
    }
}

fn fmt_usd(x: f64) -> String {
    let m = x / 1e6;
    format!("{m:.3} M")
}
