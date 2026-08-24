//! obi_probe — прогнать записанный журнал, восстановить стаканы, измерить глубину данных
//! (сколько уровней, как далеко от mid достают в %), применимость полос 3%/8% (если крайний
//! уровень ближе 3% — полосы захватывают весь стакан → асимметрия не выражается), и
//! распределение OBI-метрики bid_depth(3%) / (bid_depth(3%) + ask_depth(8%)).
//! Запуск: cargo run --example obi_probe -p book -- <journal-dir>

use std::collections::HashMap;

use book::Books;
use contracts::{EventKind, MdPayload, Side, Venue};

#[derive(Default)]
struct Stat {
    n: u64,
    sum_lvl_bid: u64,
    sum_lvl_ask: u64,
    sum_reach_bid: f64,
    sum_reach_ask: f64,
    obi: Vec<f64>,
    band3_captures_all: u64, // depth(3%) == total bid depth
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./journal-data".into());
    let evs = journal::read_all(&dir).expect("read journal");
    let mut books = Books::new();
    let mut stats: HashMap<(Venue, String), Stat> = HashMap::new();

    for e in &evs {
        let EventKind::Md(m) = &e.kind else { continue };
        books.apply(m);
        if !matches!(m.payload, MdPayload::L2Snapshot { .. }) {
            continue;
        }
        let Some(ob) = books.get(m.venue, &m.symbol) else {
            continue;
        };
        if ob.mid().is_none() {
            continue;
        }
        let s = stats.entry((m.venue, m.symbol.clone())).or_default();
        s.n += 1;
        s.sum_lvl_bid += ob.n_levels(Side::Buy) as u64;
        s.sum_lvl_ask += ob.n_levels(Side::Sell) as u64;
        let rb = ob.max_reach_pct(Side::Buy).unwrap_or(0.0);
        let ra = ob.max_reach_pct(Side::Sell).unwrap_or(0.0);
        s.sum_reach_bid += rb;
        s.sum_reach_ask += ra;
        let bid3 = ob.depth_within(Side::Buy, 0.03);
        let ask8 = ob.depth_within(Side::Sell, 0.08);
        let bid_total: i64 = ob.depth_within(Side::Buy, 10.0); // 1000% — весь стакан
        if bid3 == bid_total {
            s.band3_captures_all += 1;
        }
        let denom = (bid3 + ask8) as f64;
        if denom > 0.0 {
            s.obi.push(bid3 as f64 / denom);
        }
    }

    println!(
        "=== OBI feasibility probe (снапшотов всего в журнале: {}) ===",
        evs.len()
    );
    println!(
        "{:<22} {:>7} {:>10} {:>10} {:>12} {:>12} {:>16} {:>10}",
        "venue/symbol",
        "snaps",
        "avg_lvl_b",
        "avg_lvl_a",
        "avg_reach_b%",
        "avg_reach_a%",
        "band3=всё_стак%",
        "OBI_mean"
    );
    let mut keys: Vec<_> = stats.keys().cloned().collect();
    keys.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    for k in keys {
        let s = &stats[&k];
        if s.n == 0 {
            continue;
        }
        let obi_mean = if s.obi.is_empty() {
            0.0
        } else {
            s.obi.iter().sum::<f64>() / s.obi.len() as f64
        };
        println!(
            "{:<22} {:>7} {:>10.1} {:>10.1} {:>12.4} {:>12.4} {:>15.0}% {:>10.3}",
            format!("{:?}/{}", k.0, k.1),
            s.n,
            s.sum_lvl_bid as f64 / s.n as f64,
            s.sum_lvl_ask as f64 / s.n as f64,
            100.0 * s.sum_reach_bid / s.n as f64,
            100.0 * s.sum_reach_ask / s.n as f64,
            100.0 * s.band3_captures_all as f64 / s.n as f64,
            obi_mean
        );
    }
    println!("\nЧитать так: если avg_reach_b% << 3 И band3=всё_стак% ~100 → полоса 3% захватывает");
    println!("весь доступный стакан, т.е. асимметрия 3%/8% на этих данных НЕ выражается —");
    println!("нужна бОльшая глубина (полный стакан) прежде чем OBI-3%/8% станет вычислимой.");
}
