//! depth_lifetime — прод-анализатор M-58 (life-level метрика жизненного цикла уровня,
//! TD-103) на BTCUSDT L2Delta.
//!
//! До M-58 единица учёта была ЦЕНА (`size==0` для цены = single lifetime cancel/freeze).
//! С M-58 единица учёта — ЖИЗНЬ: одна цена может родиться/умереть/родиться снова, и
//! каждое такое рождение теперь считается отдельно. Поля BandReport переименованы:
//! `born`→`lives_born`, `cancelled`→`lives_cancelled`, `frozen`→`lives_frozen`,
//! `censored`→`lives_censored`; `cancel_fraction()` сохранил семантику (M-32 §Инварианты).
//!
//! Прогоняет `analyze` (DV-I-1..14, staleness/lifetime per-life) и `consistency`
//! (DV-I-6, order-flow faithfulness) через `journal::stream` (EpochFilter НАЗВАН,
//! `OwnCaptureOnly` per CT-RFC02-2). Фильтрует BTCUSDT (venue=Binance + symbol="BTCUSDT")
//! — спот, не фьючерс.
//!
//! Вывод: per-band lives_born/lives_cancelled/lives_frozen/lives_censored + cancel_fraction
//! near vs far; доля consistent/inconsistent trades.
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
    eprintln!("[M-58] reading journal dir={dir} epochs={epoch_ids:?}");

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
                eprintln!("[M-58] stream error: {err}");
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
        "[M-58] window: first_ts_ms={first_ts_ms} last_ts_ms={last_ts_ms} span_ms={} \
         l2delta={l2delta_count} trades={trade_count}",
        last_ts_ms - first_ts_ms,
    );

    // ── M-58 (бывш. M-32 Q2а): analyze() — staleness/lifetime per-life (DV-I-1..14) ──
    let report = analyze(&delta_ticks);
    println!("\n=== M-58 — depth_lifetime::analyze (DV-I-1..14, lives_* per band) ===");
    println!("gaps: {} (sequence-разрывы continuity)", report.gaps);
    println!(
        "{:<8} {:<10} {:<10} {:<14} {:<10} {:<10} {:<10} {:<10}",
        "side",
        "band_bps",
        "lives_born",
        "lives_cancelled",
        "lives_frozen",
        "lives_censored",
        "cancel_frac",
        "near_vs_far",
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
            band.lives_born,
            band.lives_cancelled,
            band.lives_frozen,
            band.lives_censored,
            frac_str,
            near_far,
        );
    }

    // ── Сводка near vs far (ключевой вывод M-58) ──
    let mut near_lives_born = 0u64;
    let mut near_lives_cancelled = 0u64;
    let mut near_lives_frozen = 0u64;
    let mut near_lives_censored = 0u64;
    let mut far_lives_born = 0u64;
    let mut far_lives_cancelled = 0u64;
    let mut far_lives_frozen = 0u64;
    let mut far_lives_censored = 0u64;
    for band in &report.bands {
        if band.lo_bps < 150 {
            near_lives_born += band.lives_born;
            near_lives_cancelled += band.lives_cancelled;
            near_lives_frozen += band.lives_frozen;
            near_lives_censored += band.lives_censored;
        } else if band.lo_bps >= 500 {
            far_lives_born += band.lives_born;
            far_lives_cancelled += band.lives_cancelled;
            far_lives_frozen += band.lives_frozen;
            far_lives_censored += band.lives_censored;
        }
    }
    let near_frac = if near_lives_cancelled + near_lives_frozen > 0 {
        near_lives_cancelled as f64 / (near_lives_cancelled + near_lives_frozen) as f64
    } else {
        f64::NAN
    };
    let far_frac = if far_lives_cancelled + far_lives_frozen > 0 {
        far_lives_cancelled as f64 / (far_lives_cancelled + far_lives_frozen) as f64
    } else {
        f64::NAN
    };
    println!("\n=== Сводка NEAR (≤150bps) vs FAR (≥500bps) — lives_* (M-58) ===");
    println!(
        "NEAR: lives_born={near_lives_born} lives_cancelled={near_lives_cancelled} \
         lives_frozen={near_lives_frozen} lives_censored={near_lives_censored} \
         cancel_fraction={near_frac:.3}"
    );
    println!(
        "FAR : lives_born={far_lives_born} lives_cancelled={far_lives_cancelled} \
         lives_frozen={far_lives_frozen} lives_censored={far_lives_censored} \
         cancel_fraction={far_frac:.3}"
    );
    if far_lives_cancelled + far_lives_frozen == 0 {
        println!("(FAR cancel_fraction=N/A — все уровни либо censored, либо окно пустое)");
    }

    // ── Q2б (DV-I-6): consistency() — order-flow faithfulness ──
    eprintln!(
        "[M-58] running consistency over {} events...",
        faith_events.len()
    );
    let faith = consistency(&faith_events, 1_000);
    println!("\n=== orderflow::consistency (DV-I-6) ===");
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
            "{{\"side\":\"{side}\",\"lo_bps\":{},\"hi_bps\":{},\"lives_born\":{},\"lives_cancelled\":{},\
             \"lives_frozen\":{},\"lives_censored\":{},\"cancel_fraction\":{frac}}}",
            b.lo_bps, b.hi_bps, b.lives_born, b.lives_cancelled, b.lives_frozen, b.lives_censored,
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
