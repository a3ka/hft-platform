//! Дампер журнала для проверки содержимого: cargo run --example dump -p journal -- <dir>
fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./journal-data".into());
    let evs = journal::read_all(&dir).expect("read journal");
    println!("TOTAL events: {}", evs.len());
    // Разбивка по площадке × типу.
    let mut bin_t = 0;
    let mut bin_l2 = 0;
    let mut hl_t = 0;
    let mut hl_l2 = 0;
    let mut sys = 0;
    for e in &evs {
        match &e.kind {
            contracts::EventKind::Md(m) => {
                let is_hl = matches!(m.venue, contracts::Venue::Hyperliquid);
                match m.payload {
                    contracts::MdPayload::Trade { .. } => {
                        if is_hl {
                            hl_t += 1
                        } else {
                            bin_t += 1
                        }
                    }
                    contracts::MdPayload::L2Snapshot { .. } => {
                        if is_hl {
                            hl_l2 += 1
                        } else {
                            bin_l2 += 1
                        }
                    }
                    contracts::MdPayload::Funding { .. } => {}
                    // CT-RFC-01/CT-RFC-04/CT-RFC-05: новые md-варианты в диагностике-дампе не считаются.
                    contracts::MdPayload::OpenInterest { .. }
                    | contracts::MdPayload::Liquidation { .. }
                    | contracts::MdPayload::MarginRate { .. }
                    | contracts::MdPayload::L2Delta { .. }
                    | contracts::MdPayload::MarginInventory { .. } => {}
                }
            }
            contracts::EventKind::Sys(_) => sys += 1,
        }
    }
    println!(
        "Binance: Trade={bin_t} L2={bin_l2} | Hyperliquid: Trade={hl_t} L2={hl_l2} | Sys={sys}"
    );
    println!("--- last 6 events (real content) ---");
    for e in evs.iter().rev().take(6) {
        match &e.kind {
            contracts::EventKind::Md(m) => match &m.payload {
                contracts::MdPayload::Trade {
                    price, size, side, ..
                } => println!(
                    "seq{} {:?} {} TRADE {:?} px={:.2} sz={:.6}",
                    e.seq,
                    m.venue,
                    m.symbol,
                    side,
                    contracts::from_fixed(*price),
                    contracts::from_fixed(*size)
                ),
                contracts::MdPayload::L2Snapshot { bids, asks, .. } => println!(
                    "seq{} {:?} {} L2 bid={:.2} ask={:.2} ({}x{} levels)",
                    e.seq,
                    m.venue,
                    m.symbol,
                    bids.first()
                        .map(|l| contracts::from_fixed(l.price))
                        .unwrap_or(0.0),
                    asks.first()
                        .map(|l| contracts::from_fixed(l.price))
                        .unwrap_or(0.0),
                    bids.len(),
                    asks.len()
                ),
                _ => {}
            },
            contracts::EventKind::Sys(s) => println!("seq{} SYS {:?}", e.seq, s),
        }
    }
}
