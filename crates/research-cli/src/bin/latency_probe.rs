//! latency_probe — генератор артефактов честности D7 (M-04 task 5).
//!
//! Три источника delta-сэмплов (формат `crates/sim/src/latency.rs`):
//! - `delta_md_ns` — ЭМПИРИКА из журнала: max(0, ts_wall_ms − ts_exch_ms) на каждое
//!   Md-событие пары (venue, symbol). Кламп отрицательных обязателен — клоки VPS и
//!   биржи НЕ синхронизированы (NTP-скью), отрицательный сырец не «нулевая
//!   латентность», а артефакт скью; методика зафиксирована в provenance.
//! - `delta_submit_ns` / `delta_cancel_ns` — ПРОКСИ до P1-замеров реального
//!   order-path: HTTPS RTT VPS→api.<биржа> ×2 (пессимизм ×2, D7). Сырые
//!   RTT-замеры (curl -w time_total, 5 сэмплов, 2026-07-10) захардкожены ниже как
//!   ДАННЫЕ измерения (не default-задержка sim — SM-I-7 не затрагивается: sim
//!   по-прежнему грузит только артефакт).
//!
//! После записи каждый файл round-trip'ится через sim::LatencyTable::load_artifact
//! (+ fees-артефакты через sim::FeeSchedule::load_artifact) — печатает OK/ошибку.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use contracts::{Event, EventKind, MdPayload, Venue};
use journal::EpochFilter;
use sim::{FeeSchedule, LatencyTable};

/// Максимум delta_md-сэмплов на инструмент (равномерная подвыборка сверх этого).
const MAX_MD_SAMPLES: usize = 5000;

/// RTT-замеры с VPS (Hetzner cpx32, 2026-07-10, curl -w time_total, секунды).
/// Собраны architect'ом; time_connect ~0.007-0.009s на обоих хостах.
const BINANCE_RTT_TOTAL_S: [f64; 5] = [0.262252, 0.258150, 0.257400, 0.258631, 0.258807];
const HYPERLIQUID_RTT_TOTAL_S: [f64; 5] = [0.256179, 0.258654, 0.257939, 0.475819, 0.256529];

/// Пессимизм ×2 к измеренному RTT (D7).
const RTT_PESSIMISM_FACTOR: f64 = 2.0;

fn venue_str(v: Venue) -> &'static str {
    match v {
        Venue::Binance => "Binance",
        Venue::Hyperliquid => "Hyperliquid",
        Venue::BinanceFutures => "BinanceFutures",
    }
}

/// RTT-сэмплы × пессимизм → наносекунды, отсортированные по возрастанию.
fn rtt_to_ns_sorted(rtt_s: &[f64]) -> Vec<u64> {
    let mut ns: Vec<u64> = rtt_s
        .iter()
        .map(|s| (s * RTT_PESSIMISM_FACTOR * 1e9).round() as u64)
        .collect();
    ns.sort_unstable();
    ns
}

/// Равномерная подвыборка до MAX_MD_SAMPLES (детерминированная: индексы i·len/N).
fn subsample_uniform(samples: Vec<u64>) -> Vec<u64> {
    if samples.len() <= MAX_MD_SAMPLES {
        return samples;
    }
    let len = samples.len();
    (0..MAX_MD_SAMPLES)
        .map(|i| samples[i * len / MAX_MD_SAMPLES])
        .collect()
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let journal_dir = flag(&args, "--journal")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("tmp/journal-vps"));
    let out_dir = flag(&args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("research/latency"));
    let fees_dir = flag(&args, "--fees")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("research/fees"));

    // M-08 E5/E6: прод-путь чтения — `journal::stream` + ЯВНО названный EpochFilter.
    // На боевых 8.3 GB материализация событий в RAM OOM-нула бы машину (класс TD-011).
    // Здесь latency-пробе нужны ТОЛЬКО `OwnCapture` — vendor/синтетика не участвуют
    // в методике (CT-RFC02-3/4).
    let mut stream = match journal::stream(&journal_dir, EpochFilter::OwnCaptureOnly) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "error: открытие стрима журнала {}: {e}",
                journal_dir.display()
            );
            std::process::exit(1);
        }
    };

    // Два прохода не нужны: в первом же проходе считаем total и собираем сэмплы.
    // Стрим одноразовый — rewind'а нет, поэтому накапливаем счётчик локально.
    let mut total_events: u64 = 0;
    let mut md_samples: BTreeMap<(String, String), Vec<u64>> = BTreeMap::new();
    let mut missing_ts: BTreeMap<(String, String, &'static str), usize> = BTreeMap::new();
    let mut first_error: Option<String> = None;
    loop {
        let ev: Event = match stream.next() {
            Some(Ok(ev)) => ev,
            Some(Err(e)) => {
                if first_error.is_none() {
                    first_error = Some(format!("{e}"));
                }
                break;
            }
            None => break,
        };
        total_events += 1;
        let EventKind::Md(md) = &ev.kind else {
            continue;
        };
        let (ts_exch_ms, payload_kind) = match &md.payload {
            MdPayload::Trade { ts_exch_ms, .. } => (*ts_exch_ms, "Trade"),
            MdPayload::L2Snapshot { ts_exch_ms, .. } => (*ts_exch_ms, "L2Snapshot"),
            MdPayload::Funding { ts_exch_ms, .. } => (*ts_exch_ms, "Funding"),
            // CT-RFC-01/CT-RFC-04/CT-RFC-05: новые md-варианты не участвуют в md-latency пробе.
            // MarginInventory (CT-RFC-05, дискриминант 7) — СЫРОЙ пул доступного маржинального
            // обеспечения по активу, НЕ latency-релевантно (как L2Delta/OpenInterest/MarginRate):
            // latency = delta_wall_minus_exch по TIMESTAMP-у события, а inventory-events приходят
            // отдельным signed read-only poll'ом, семантически другая категория. Сэмплировать в
            // md-latency — смешение осей (capacity-poll vs market-data-stream).
            MdPayload::OpenInterest { .. }
            | MdPayload::Liquidation { .. }
            | MdPayload::MarginRate { .. }
            | MdPayload::L2Delta { .. }
            | MdPayload::MarginInventory { .. } => continue,
        };
        let key = (venue_str(md.venue).to_string(), md.symbol.clone());
        if ts_exch_ms <= 0 {
            *missing_ts.entry((key.0, key.1, payload_kind)).or_default() += 1;
            continue;
        }
        // Кламп отрицательных: клоки VPS/биржи не синхронны (NTP-скью) —
        // отрицательный сырец не латентность, а скью; см. provenance.
        let delta_ns = (ev.ts_wall_ms - ts_exch_ms).max(0) as u64 * 1_000_000;
        md_samples.entry(key).or_default().push(delta_ns);
    }

    if let Some(err) = first_error {
        eprintln!(
            "error: чтение стрима {} прервано после {total_events} событий: {err}",
            journal_dir.display()
        );
        std::process::exit(1);
    }
    println!(
        "journal: {} — {total_events} событий",
        journal_dir.display()
    );

    for ((venue, symbol, kind), n) in &missing_ts {
        println!("skipped (ts_exch_ms<=0, не замер): {venue} {symbol} {kind} — {n} событий");
    }

    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("error: создание {}: {e}", out_dir.display());
        std::process::exit(1);
    }

    let mut written: Vec<PathBuf> = Vec::new();
    let mut failures = 0usize;

    for ((venue, symbol), samples) in md_samples {
        let raw_n = samples.len();
        let skipped_n: usize = missing_ts
            .iter()
            .filter(|((v, s, _), _)| *v == venue && *s == symbol)
            .map(|(_, n)| n)
            .sum();
        let mut delta_md_ns = subsample_uniform(samples);
        delta_md_ns.sort_unstable();

        let rtt = match venue.as_str() {
            "Binance" => &BINANCE_RTT_TOTAL_S,
            _ => &HYPERLIQUID_RTT_TOTAL_S,
        };
        let submit_ns = rtt_to_ns_sorted(rtt);
        let cancel_ns = submit_ns.clone();

        let provenance = format!(
            "delta_md: эмпирика ts_wall_ms−ts_exch_ms по журналу VPS 2026-07-10 \
             ({total_events} событий; {raw_n} Md-сэмплов инструмента, равномерная \
             подвыборка до {}), кламп отрицательных (клоки не синхронизированы, \
             NTP-скью включён в оценку); {skipped_n} событий с ts_exch_ms<=0 \
             (биржевая метка отсутствует — не замер) исключены; \
             delta_submit/cancel: HTTPS RTT VPS(Hetzner hel1)→api.{} ×2 \
             (5 сэмплов curl 2026-07-10) — ПРОКСИ до P1-замеров реального \
             order-path, консервативно завышено",
            delta_md_ns.len(),
            venue.to_lowercase(),
        );

        let artifact = serde_json::json!({
            "schema_version": 1,
            "venue": venue,
            "symbol": symbol,
            "provenance": provenance,
            "delta_submit_ns": submit_ns,
            "delta_cancel_ns": cancel_ns,
            "delta_md_ns": delta_md_ns,
        });

        let file = out_dir.join(format!("{}-{}.json", venue.to_lowercase(), symbol));
        let mut body = match serde_json::to_string_pretty(&artifact) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: сериализация {}: {e}", file.display());
                std::process::exit(1);
            }
        };
        body.push('\n');
        if let Err(e) = std::fs::write(&file, body) {
            eprintln!("error: запись {}: {e}", file.display());
            std::process::exit(1);
        }
        println!(
            "written: {} (md-сэмплов: {raw_n} → {})",
            file.display(),
            artifact["delta_md_ns"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0)
        );
        written.push(file);
    }

    // Round-trip: каждый latency-артефакт обязан грузиться sim'ом (SM-I-8 формат).
    for file in &written {
        let mut table = LatencyTable::new();
        match table.load_artifact(file) {
            Ok(()) => println!("round-trip latency OK: {}", file.display()),
            Err(e) => {
                println!("round-trip latency FAIL: {}: {e:?}", file.display());
                failures += 1;
            }
        }
    }

    // Round-trip fees-артефактов (D7 тарифы).
    for name in ["binance.json", "hyperliquid.json"] {
        let file = fees_dir.join(name);
        let mut fees = FeeSchedule::new();
        match fees.load_artifact(&file) {
            Ok(()) => println!("round-trip fees OK: {}", file.display()),
            Err(e) => {
                println!("round-trip fees FAIL: {}: {e:?}", file.display());
                failures += 1;
            }
        }
    }

    if failures > 0 {
        eprintln!("latency_probe: {failures} round-trip провалов");
        std::process::exit(1);
    }
    println!("latency_probe: все round-trip проверки OK");
}
