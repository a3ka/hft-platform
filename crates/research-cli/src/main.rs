//! CLI: research-cli grid|validate|report (FA §3 outer). std::env::args, без clap.
//! Тонкая обвязка над `research_cli` библиотекой; сам CLI не покрыт RED-suite'ом
//! (sacred-тесты бьют по библиотечным функциям напрямую) — важна работоспособность
//! и компилируемость (milestone M-04 task 4).
//!
//! M-08 E5/E6 (задача 5): CLI-`grid` ходит в журнал через `journal::stream` +
//! `EpochFilter::OwnCaptureOnly`. На боевых 8.3 GB старый путь с материализацией
//! в `Vec<Event>` OOM-нул бы машину (класс TD-011).

use std::path::{Path, PathBuf};

use journal::EpochFilter;
use research_cli::grid::{run_grid_streamed, GridRunEnv, JournalSource};
use research_cli::ledger::Ledger;
use research_cli::report::{
    journal_sha256, require_preregistration, write_metrics_json, write_narrative_md,
};
use research_cli::types::{GridSpec, SplitKind, ValidationReport};
use sim::{FeeSchedule, LatencyTable};

/// Найти значение флага `--name value` в списке аргументов.
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn cmd_grid(args: &[String]) -> Result<(), String> {
    let journal_dir = flag(args, "--journal").ok_or("--journal <dir> обязателен")?;
    let spec_path = flag(args, "--spec").ok_or("--spec <json-file> обязателен")?;
    let ledger_path = flag(args, "--ledger").ok_or("--ledger <path> обязателен")?;
    let range = flag(args, "--range").ok_or("--range <from,to> обязателен")?;
    let (from_s, to_s) = range
        .split_once(',')
        .ok_or("--range должен быть вида from,to")?;
    let from: i64 = from_s
        .trim()
        .parse()
        .map_err(|e| format!("--range from: {e}"))?;
    let to: i64 = to_s
        .trim()
        .parse()
        .map_err(|e| format!("--range to: {e}"))?;

    let split = match flag(args, "--split").as_deref().unwrap_or("train") {
        "train" => SplitKind::Train,
        "val" => SplitKind::Val,
        "test" => {
            return Err(
                "split=test требует ValGateToken (RC-I-8) — недоступно из bare CLI; \
                 используйте программный API (split::SplitState) для финальной валидации"
                    .into(),
            )
        }
        other => return Err(format!("неизвестный --split {other}")),
    };

    let spec_raw = std::fs::read_to_string(&spec_path).map_err(|e| e.to_string())?;
    let spec: GridSpec = serde_json::from_str(&spec_raw).map_err(|e| e.to_string())?;

    // M-08 E5/E6: прод-путь чтения — `journal::stream` + ЯВНО названный EpochFilter.
    // CLI по умолчанию использует `OwnCaptureOnly`: vendor/синтетика в обучение по
    // умолчанию НЕ попадают (CT-RFC02-3/4). Осознанное смешение эпох — через
    // программный API с `EpochFilter::Explicit(...)`.
    let source = JournalSource {
        dir: PathBuf::from(&journal_dir),
        filter: EpochFilter::OwnCaptureOnly,
    };

    let mut latency = LatencyTable::new();
    if let Some(p) = flag(args, "--latency") {
        latency
            .load_artifact(Path::new(&p))
            .map_err(|e| format!("{e:?}"))?;
    }
    let mut fees = FeeSchedule::new();
    if let Some(p) = flag(args, "--fees") {
        fees.load_artifact(Path::new(&p))
            .map_err(|e| format!("{e:?}"))?;
    }

    let mut ledger = Ledger::open(&ledger_path).map_err(|e| e.to_string())?;
    let mut env = GridRunEnv {
        ledger: &mut ledger,
        latency: &latency,
        fees: &fees,
    };

    let results = run_grid_streamed(&source, &spec, split, (from, to), &mut env, None)
        .map_err(|e| format!("{e:?}"))?;

    println!(
        "grid: {} ячеек прогнано; ledger={}",
        results.len(),
        ledger.path().display()
    );
    for r in &results {
        println!(
            "  hash={} sharpe={:.4} intents={} fills={} net_pnl_e8={}",
            r.params_hash, r.sharpe, r.intents, r.fills, r.net_pnl_e8
        );
    }
    Ok(())
}

fn cmd_validate(args: &[String]) -> Result<(), String> {
    let hyp = flag(args, "--hypothesis").ok_or("--hypothesis <md> обязателен")?;
    require_preregistration(Path::new(&hyp)).map_err(|e| format!("{e:?}"))?;
    println!("validate: пре-регистрация OK для {hyp}");
    Ok(())
}

fn cmd_report(args: &[String]) -> Result<(), String> {
    let journal_dir = flag(args, "--journal").ok_or("--journal <dir> обязателен")?;
    let out = flag(args, "--out").ok_or("--out <prefix> обязателен")?;
    let sha = journal_sha256(Path::new(&journal_dir)).map_err(|e| format!("{e:?}"))?;
    println!("report: journal_sha256={sha}");

    if let Some(report_json_path) = flag(args, "--report-json") {
        let raw = std::fs::read_to_string(&report_json_path).map_err(|e| e.to_string())?;
        let report: ValidationReport = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let out_path = PathBuf::from(&out);
        write_metrics_json(&report, &out_path.with_extension("json"))
            .map_err(|e| format!("{e:?}"))?;
        write_narrative_md(&report, &out_path.with_extension("md"))
            .map_err(|e| format!("{e:?}"))?;
        println!("report: записаны {out}.json / {out}.md");
    } else {
        println!(
            "report: передайте --report-json <ValidationReport JSON> для записи metrics.json/R-NNN.md"
        );
    }
    Ok(())
}

fn cmd_export(args: &[String]) -> Result<(), String> {
    use std::path::PathBuf;

    use journal::EpochFilter;
    use research_cli::export_io::{export_to_dir, ExportConfig};

    let journal_dir = flag(args, "--journal").ok_or("--journal <dir> обязателен")?;
    let out = flag(args, "--out").ok_or("--out <dir> обязателен")?;
    let timeframe_ms: i64 = flag(args, "--timeframe-ms")
        .map(|s| s.parse::<i64>().map_err(|e| format!("--timeframe-ms: {e}")))
        .unwrap_or(Ok(1_000))?;
    // Полосы depth — повторяемый флаг --band <float>; дефолт 0.001/0.003/0.005.
    let mut depth_bands_pct: Vec<f64> = Vec::new();
    let mut i = 0;
    while let Some(pos) = args.iter().skip(i).position(|a| a == "--band") {
        let abs = i + pos;
        let val = args
            .get(abs + 1)
            .ok_or_else(|| "--band <float> требует значение".to_string())?
            .parse::<f64>()
            .map_err(|e| format!("--band: {e}"))?;
        depth_bands_pct.push(val);
        i = abs + 2;
    }
    if depth_bands_pct.is_empty() {
        depth_bands_pct = vec![0.001, 0.003, 0.005];
    }
    // Фильтр эпох (CT-RFC02-2). По умолчанию — OwnCaptureOnly.
    let filter = match flag(args, "--filter").as_deref() {
        Some("own_capture_only") | None => EpochFilter::OwnCaptureOnly,
        Some(other) => return Err(format!("неизвестный --filter {other}")),
    };

    let cfg = ExportConfig {
        timeframe_ms,
        depth_bands_pct,
    };
    let source = research_cli::grid::JournalSource {
        dir: PathBuf::from(&journal_dir),
        filter,
    };
    let written =
        export_to_dir(&source, &cfg, std::path::Path::new(&out)).map_err(|e| format!("{e}"))?;
    println!("export: записано {} артефактов в {out}", written.len());
    for path in &written {
        println!("  {}", path.display());
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rest: Vec<String> = args.iter().skip(2).cloned().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("grid") => cmd_grid(&rest),
        Some("validate") => cmd_validate(&rest),
        Some("report") => cmd_report(&rest),
        Some("export") => cmd_export(&rest),
        _ => {
            eprintln!("usage: research-cli <grid|validate|report|export> ...");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
