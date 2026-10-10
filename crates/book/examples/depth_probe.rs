//! depth_probe — ФАНТОМ-детектор дальних полос TPP (закрывает вопрос, оставленный bands/obi_probe).
//!
//! bands/obi_probe измерили ДОСЯГАЕМОСТЬ книги (до ~50-59% от mid), но НЕ КАЧЕСТВО дальних полос.
//! reach ≠ реальная ликвидность. TD-016 (OPEN): «мёртвые уровни… попадают в полосы OBI 6-60% →
//! ФАНТОМНАЯ ликвидность, которой на бирже нет». Этот пробник отвечает: полосы 3-30% РЕАЛЬНЫ или фантом?
//!
//! Сигнатура TD-016 (механика лика): цена уходит → уровни, из-под которых она ушла, size=0 больше НЕ
//! получают и живут вечно ⇒ notional дальней полосы растёт МОНОТОННО и почти НЕ проседает. Реальная
//! ликвидность флуктуирует (есть просадки — ордера снимают/исполняют). Поэтому per (venue,symbol,side,shell)
//! меряем: frac_up (доля шагов роста), max_rel_drawdown (макс. просадка от бегущего пика). Фантом ⇒
//! frac_up→1.0 И drawdown→0. Плюс p10/p50/p90 max_reach_pct и полосы 0.5%/1% (SVR research-dev'а).
//!
//! ОФЛАЙН-диагностика (read_all, мягкая классификация — как bands/obi_probe; T11e carve-out). Журнал/
//! recorder не трогает. Запуск: cargo run --release -p book --example depth_probe -- <journal-dir>

use std::collections::HashMap;

use book::Books;
use contracts::{EventKind, MdPayload, Side, Venue};

/// Полосы (доля от mid), включая 0.5% и 1% — SVR research-dev'а.
const BANDS: [f64; 8] = [0.005, 0.01, 0.015, 0.03, 0.05, 0.08, 0.15, 0.30];
/// Оболочки [inner, outer) для фантом-теста: именно ДАЛЬНИЕ полосы, где живёт TD-016.
const SHELLS: [(f64, f64); 4] = [(0.03, 0.05), (0.05, 0.08), (0.08, 0.15), (0.15, 0.30)];

#[derive(Default)]
struct Series {
    n: u64,
    reach_bid: Vec<f64>,
    reach_ask: Vec<f64>,
    shell_bid: [Vec<f64>; 4],
    shell_ask: [Vec<f64>; 4],
    last_n_bid: u64,
    last_n_ask: u64,
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./journal-data".into());
    let evs = journal::read_all(&dir).expect("read journal");

    let mut books = Books::new();
    let mut ser: HashMap<(Venue, String), Series> = HashMap::new();

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
        let s = ser.entry((m.venue, m.symbol.clone())).or_default();
        s.n += 1;
        s.reach_bid
            .push(100.0 * ob.max_reach_pct(Side::Buy).unwrap_or(0.0));
        s.reach_ask
            .push(100.0 * ob.max_reach_pct(Side::Sell).unwrap_or(0.0));
        for (i, (inner, outer)) in SHELLS.iter().enumerate() {
            // Нотионал оболочки = notional(outer) − notional(inner) (в USD, через from_fixed·from_fixed уже в $).
            let sb = shell_usd(ob, Side::Buy, *inner, *outer);
            let sa = shell_usd(ob, Side::Sell, *inner, *outer);
            s.shell_bid[i].push(sb);
            s.shell_ask[i].push(sa);
        }
        s.last_n_bid = ob.n_levels(Side::Buy) as u64;
        s.last_n_ask = ob.n_levels(Side::Sell) as u64;
    }

    let mut keys: Vec<_> = ser.keys().cloned().collect();
    keys.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));

    println!(
        "# depth_probe — фантом-детектор дальних полос (TD-016). snapshots × {}",
        keys.len()
    );
    for k in &keys {
        let s = &ser[k];
        if s.n < 2 {
            continue;
        }
        println!(
            "\n=== {:?}/{} (снапшотов: {}, уровней bid={}/ask={}) ===",
            k.0, k.1, s.n, s.last_n_bid, s.last_n_ask
        );
        // Досягаемость — распределение, а не только среднее.
        println!(
            "  max_reach% BID  p10/p50/p90 = {:.2}/{:.2}/{:.2}   ASK = {:.2}/{:.2}/{:.2}",
            pct(&s.reach_bid, 0.10),
            pct(&s.reach_bid, 0.50),
            pct(&s.reach_bid, 0.90),
            pct(&s.reach_ask, 0.10),
            pct(&s.reach_ask, 0.50),
            pct(&s.reach_ask, 0.90),
        );
        println!(
            "  {:>10}  {:>8}  {:>10}  {:>10}  {:>8}   вердикт",
            "shell", "side", "start$M", "end$M", "up%/dd%"
        );
        for (i, (inner, outer)) in SHELLS.iter().enumerate() {
            for (side, name, v) in [
                (Side::Buy, "BID", &s.shell_bid[i]),
                (Side::Sell, "ASK", &s.shell_ask[i]),
            ] {
                let _ = side;
                let (start, end, frac_up, max_dd) = signature(v);
                // Фантом (TD-016): растёт монотонно (up>0.9) И почти не проседает (dd<0.1) И вырос.
                let phantom = frac_up > 0.90 && max_dd < 0.10 && end > start * 1.3;
                println!(
                    "  {:>3.0}-{:>3.0}%  {:>8}  {:>10.2}  {:>10.2}  {:>3.0}/{:>3.0}   {}",
                    inner * 100.0,
                    outer * 100.0,
                    name,
                    start / 1e6,
                    end / 1e6,
                    frac_up * 100.0,
                    max_dd * 100.0,
                    if phantom {
                        "⚠ ФАНТОМ-СИГНАТУРА (TD-016)"
                    } else {
                        "флуктуирует (похоже на реальную)"
                    }
                );
            }
        }
        // Полосы last-snapshot (включая 0.5% и 1% — SVR): notional $M per side.
        if let Some(ob) = books.get(k.0, &k.1) {
            print!("  полосы$M BID:");
            for b in BANDS {
                print!(
                    " {:.1}%={:.1}",
                    b * 100.0,
                    ob.notional_within(Side::Buy, b) / 1e6
                );
            }
            println!();
            print!("  полосы$M ASK:");
            for b in BANDS {
                print!(
                    " {:.1}%={:.1}",
                    b * 100.0,
                    ob.notional_within(Side::Sell, b) / 1e6
                );
            }
            println!();
        }
    }
    println!(
        "\n# Чтение: ФАНТОМ-СИГНАТУРА = shell-notional растёт монотонно (up%→100) при просадке≈0 (dd%→0)\n\
         #          ⇒ дальняя полоса состоит из мёртвых уровней (TD-016), TPP-сумма НЕДОСТОВЕРНА.\n\
         #          «флуктуирует» = есть просадки ⇒ ликвидность живая (снимают/исполняют) ⇒ полоса реальна.\n\
         # ОГОВОРКА: это НЕ recon с биржей (её глубже 1.3% нет). Это НЕОБХОДИМОЕ, не достаточное условие:\n\
         #          фантом-сигнатура доказывает загрязнение; её отсутствие НЕ доказывает 100% чистоту."
    );
}

/// Нотионал оболочки [inner, outer) в USD.
fn shell_usd(ob: &book::OrderBook, side: Side, inner: f64, outer: f64) -> f64 {
    ob.notional_within(side, outer) - ob.notional_within(side, inner)
}

/// (start, end, frac_up, max_rel_drawdown) временного ряда.
fn signature(v: &[f64]) -> (f64, f64, f64, f64) {
    if v.len() < 2 {
        let x = v.first().copied().unwrap_or(0.0);
        return (x, x, 0.0, 0.0);
    }
    let start = v[0];
    let end = *v.last().unwrap();
    let mut up = 0u64;
    let mut steps = 0u64;
    let mut peak = v[0];
    let mut max_dd = 0.0f64;
    for w in v.windows(2) {
        steps += 1;
        if w[1] > w[0] {
            up += 1;
        }
        if w[1] > peak {
            peak = w[1];
        }
        if peak > 0.0 {
            let dd = (peak - w[1]) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }
    let frac_up = if steps > 0 {
        up as f64 / steps as f64
    } else {
        0.0
    };
    (start, end, frac_up, max_dd)
}

/// Перцентиль p∈[0,1] по копии-сортировке.
fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((s.len() as f64 - 1.0) * p).round() as usize;
    s[idx]
}
