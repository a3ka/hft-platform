//! depth_lifetime — прод-анализатор M-32 Q2а/Q2б на BTCUSDT L2Delta.
//!
//! Прогоняет `analyze` (DV-I-1..5, staleness/lifetime) и `consistency` (DV-I-6, order-flow
//! faithfulness) через `journal::stream` (EpochFilter НАЗВАН, `OwnCaptureOnly` per
//! CT-RFC02-2). Фильтрует BTCUSDT (venue=Binance + symbol="BTCUSDT") — спот, не фьючерс.
//!
//! Вывод: per-band born/cancelled/frozen/censored + cancel_fraction near vs far; доля
//! consistent/inconsistent trades.
//!
//! Запуск:
//!   cargo run --release -p research-cli --example depth_lifetime -- /var/lib/docker/volumes/hft-platform_journal-data/_data/

use contracts::{EventKind, MdPayload, Side};
use journal::EpochFilter;
use research_cli::depth_lifetime::{analyze, DeltaTick};
use research_cli::orderflow::{consistency, FaithEvent};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./journal-data".into());

    let stream = journal::stream(&dir, EpochFilter::OwnCaptureOnly)
        .expect("open journal::stream with EpochFilter::OwnCaptureOnly");
    let epoch_ids: Vec<String> = stream
        .headers()
        .iter()
        .map(|h| h.epoch_id.clone())
        .collect();
    eprintln!("[M-32] reading journal dir={dir} epochs={epoch_ids:?}");

    let mut delta_ticks: Vec<DeltaTick> = Vec::new();
    let mut faith_events: Vec<FaithEvent> = Vec::new();
    let mut l2delta_count: u64 = 0;
    let mut trade_count: u64 = 0;
    let mut first_ts_ms: i64 = 0;
    let mut last_ts_ms: i64 = 0;

    for result in stream {
        let event = match result {
            Ok(e) => e,
            Err(err) => {
                eprintln!("[M-32] stream error: {err}");
                continue;
            }
        };
        let EventKind::Md(md) = &event.kind else {
            continue;
        };
        // Только Binance Spot BTCUSDT — это основной эталон в Q1 (rest cap 1.3%, но diff
        // покрывает все полосы).
        if !matches!(md.venue, contracts::Venue::Binance) || md.symbol != "BTCUSDT" {
            continue;
        }
        match &md.payload {
            MdPayload::L2Delta {
                bids,
                asks,
                first_update_id,
                final_update_id,
                prev_final_update_id,
                ts_exch_ms,
            } => {
                l2delta_count += 1;
                delta_ticks.push(DeltaTick {
                    bids: bids.clone(),
                    asks: asks.clone(),
                    first_update_id: *first_update_id,
                    final_update_id: *final_update_id,
                    prev_final_update_id: *prev_final_update_id,
                    ts_exch_ms: *ts_exch_ms,
                });
                last_ts_ms = event.ts_wall_ms;
                if first_ts_ms == 0 {
                    first_ts_ms = event.ts_wall_ms;
                }
                // FaithEvent stream — Delta-ы для `consistency`.
                faith_events.push(FaithEvent::Delta {
                    ts_ms: event.ts_wall_ms,
                    bids: bids.clone(),
                    asks: asks.clone(),
                });
            }
            MdPayload::Trade {
                price,
                size,
                side,
                ts_exch_ms: _,
            } => {
                trade_count += 1;
                faith_events.push(FaithEvent::Trade {
                    ts_ms: event.ts_wall_ms,
                    price: *price,
                    side: *side,
                    size: *size,
                });
                last_ts_ms = event.ts_wall_ms;
                if first_ts_ms == 0 {
                    first_ts_ms = event.ts_wall_ms;
                }
            }
            _ => {}
        }
    }

    eprintln!(
        "[M-32] window: first_ts_ms={first_ts_ms} last_ts_ms={last_ts_ms} span_ms={} \
         l2delta={l2delta_count} trades={trade_count}",
        last_ts_ms - first_ts_ms,
    );

    // ── Q2а: analyze() — staleness/lifetime per band ──
    let report = analyze(&delta_ticks);
    println!("\n=== M-32 Q2а — depth_lifetime::analyze (DV-I-1..5) ===");
    println!("gaps: {} (sequence-разрывы continuity)", report.gaps);
    println!(
        "{:<8} {:<10} {:<8} {:<10} {:<10} {:<10} {:<10} {:<10}",
        "side", "band_bps", "born", "cancelled", "frozen", "censored", "cancel_frac", "near_vs_far"
    );
    for band in &report.bands {
        let side_str = match band.side {
            Side::Buy => "bid",
            Side::Sell => "ask",
        };
        let frac_str = match band.cancel_fraction() {
            Some(f) => format!("{:.3}", f),
            None => "n/a".to_string(),
        };
        let near_far = if band.lo_bps < 150 {
            "NEAR".to_string()
        } else if band.lo_bps >= 500 {
            "FAR".to_string()
        } else {
            "MID".to_string()
        };
        println!(
            "{:<8} {:<10} {:<8} {:<10} {:<10} {:<10} {:<10} {:<10}",
            side_str,
            format!("[{},{})", band.lo_bps, band.hi_bps),
            band.born,
            band.cancelled,
            band.frozen,
            band.censored,
            frac_str,
            near_far,
        );
    }

    // ── Сводка near vs far (ключевой вывод M-32) ──
    let mut near_born = 0u64;
    let mut near_cancelled = 0u64;
    let mut near_frozen = 0u64;
    let mut near_censored = 0u64;
    let mut far_born = 0u64;
    let mut far_cancelled = 0u64;
    let mut far_frozen = 0u64;
    let mut far_censored = 0u64;
    for band in &report.bands {
        if band.lo_bps < 150 {
            near_born += band.born;
            near_cancelled += band.cancelled;
            near_frozen += band.frozen;
            near_censored += band.censored;
        } else if band.lo_bps >= 500 {
            far_born += band.born;
            far_cancelled += band.cancelled;
            far_frozen += band.frozen;
            far_censored += band.censored;
        }
    }
    let near_frac = if near_cancelled + near_frozen > 0 {
        near_cancelled as f64 / (near_cancelled + near_frozen) as f64
    } else {
        f64::NAN
    };
    let far_frac = if far_cancelled + far_frozen > 0 {
        far_cancelled as f64 / (far_cancelled + far_frozen) as f64
    } else {
        f64::NAN
    };
    println!("\n=== Сводка NEAR (≤150bps) vs FAR (≥500bps) ===");
    println!(
        "NEAR: born={near_born} cancelled={near_cancelled} frozen={near_frozen} censored={near_censored} \
         cancel_fraction={near_frac:.3}"
    );
    println!(
        "FAR : born={far_born} cancelled={far_cancelled} frozen={far_frozen} censored={far_censored} \
         cancel_fraction={far_frac:.3}"
    );
    if far_cancelled + far_frozen == 0 {
        println!("(FAR cancel_fraction=N/A — все уровни либо censored, либо окно пустое)");
    }

    // ── Q2б: consistency() — order-flow faithfulness ──
    eprintln!(
        "[M-32] running consistency over {} events...",
        faith_events.len()
    );
    let faith = consistency(&faith_events, 1_000);
    println!("\n=== M-32 Q2б — orderflow::consistency (DV-I-6) ===");
    println!(
        "checked={} consistent={} inconsistent={} \
         consistency_rate={:.3}",
        faith.checked,
        faith.consistent,
        faith.inconsistent,
        if faith.checked > 0 {
            faith.consistent as f64 / faith.checked as f64
        } else {
            f64::NAN
        }
    );

    // Сохранить результаты в JSON для дальнейшей документации
    println!("\n=== Сводный JSON ===");
    print_summary_json(
        &report,
        &faith,
        &epoch_ids,
        first_ts_ms,
        last_ts_ms,
        l2delta_count,
        trade_count,
    );
}

fn print_summary_json(
    report: &research_cli::depth_lifetime::LifetimeReport,
    faith: &research_cli::orderflow::FaithReport,
    epoch_ids: &[String],
    first_ts_ms: i64,
    last_ts_ms: i64,
    l2delta_count: u64,
    trade_count: u64,
) {
    // Минимальный ручной JSON — без зависимостей (мы не хотим тащить serde_json в example
    // для архитектурной чистоты). Детерминирован: фиксированный порядок полей.
    let mut bands_out = String::new();
    for (i, b) in report.bands.iter().enumerate() {
        let side = match b.side {
            Side::Buy => "buy",
            Side::Sell => "sell",
        };
        let frac = b
            .cancel_fraction()
            .map(|f| format!("{f:.6}"))
            .unwrap_or_else(|| "null".to_string());
        if i > 0 {
            bands_out.push(',');
        }
        bands_out.push_str(&format!(
            "{{\"side\":\"{side}\",\"lo_bps\":{},\"hi_bps\":{},\"born\":{},\"cancelled\":{},\
             \"frozen\":{},\"censored\":{},\"cancel_fraction\":{frac}}}",
            b.lo_bps, b.hi_bps, b.born, b.cancelled, b.frozen, b.censored,
        ));
    }
    let epoch_json = format!(
        "[{}]",
        epoch_ids
            .iter()
            .map(|e| format!("\"{e}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    println!(
        "{{\"epoch_ids\":{epoch_json},\"first_ts_ms\":{first_ts_ms},\"last_ts_ms\":{last_ts_ms},\
         \"l2delta_count\":{l2delta_count},\"trade_count\":{trade_count},\
         \"gaps\":{},\"bands\":[{bands_out}],\
         \"faith\":{{\"checked\":{},\"consistent\":{},\"inconsistent\":{}}}}}",
        report.gaps, faith.checked, faith.consistent, faith.inconsistent,
    );
}
