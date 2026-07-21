//! report — детерминированная генерация ValidationReport/metrics.json (FA §7).
//! RC-I-5: те же входы → байт-идентичный metrics.json. НИКАКИХ wall-clock/HashMap-
//! итераций в сериализуемом составе. Нарратив R-NNN.md — шаблонная генерация из
//! чисел (без LLM) — интерпретация остаётся критику/человеку.
//!
//! Пре-регистрация (FA §8.1): финальная валидация ОТКАЗЫВАЕТСЯ работать без карточки
//! research/hypotheses/H-*.md с заполненным разделом «критерии фальсификации».
//!
//! Реализация — research-dev (M-04 task 4).

use std::fs;
use std::path::Path;

use contracts::Event;
use sha2::{Digest, Sha256};
use sim::{FeeSchedule, LatencyTable};

use crate::data_quality;
use crate::grid::{self, GridRunEnv, JournalSource};
use crate::ledger::Ledger;
use crate::metrics;
pub use crate::types::Verdict;
use crate::types::{
    CellResult, CostsMode, GridSpec, RcError, StressResult, ValidationReport, REPORT_SCHEMA_VERSION,
};

/// Kill-screen inputs and classification API are deliberately kept in this module's
/// public surface: callers must not infer a promotion verdict from prose or from
/// an ad-hoc backtest harness.
#[derive(Debug, Clone, PartialEq)]
pub struct KillScreenInputs {
    pub sharpe: f64,
    pub se_sharpe: f64,
    pub data_span_days: f64,
    pub oos_sharpe: f64,
    pub deflated_sharpe: f64,
    pub walkforward_min_sharpe: f64,
    pub half_life_ms: i64,
    pub horizon_ms: i64,
    pub worst_stress_net_pnl_e8: i64,
}

/// Provenance that must accompany an R-001 verdict. It is separate from
/// `KillScreenInputs` because E8 and TD-015 are produced by I/O/report layers,
/// not by the pure numerical classifier.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportHonesty {
    pub data_span_days: f64,
    pub se_sharpe: f64,
    pub gap_ref: String,
    pub ledger_cutoff: String,
}

/// Классифицировать R-001 по ПРЕ-РЕГ критериям.
///
/// Порядок является частью контракта KS-I-4/KS-I-1: любой сработавший критерий
/// фальсификации убивает гипотезу, даже если точечная оценка имеет узкий CI.
/// Только после проверки всех Kill-критериев разрешается рассматривать нижнюю
/// границу Sharpe над баром.
pub fn classify_verdict(inputs: &KillScreenInputs, bar: f64) -> Verdict {
    if inputs.oos_sharpe <= 0.5 {
        return Verdict::Kill(format!(
            "oos_sharpe={:.6} <= 0.5 (пре-рег критерий Net Sharpe)",
            inputs.oos_sharpe
        ));
    }
    if inputs.deflated_sharpe <= 0.0 {
        return Verdict::Kill(format!(
            "deflated_sharpe={:.6} <= 0 (trials-ledger correction)",
            inputs.deflated_sharpe
        ));
    }
    if inputs.walkforward_min_sharpe < 0.0 {
        return Verdict::Kill(format!(
            "walkforward_min_sharpe={:.6} < 0 (режимная нестабильность)",
            inputs.walkforward_min_sharpe
        ));
    }
    if inputs.half_life_ms < inputs.horizon_ms {
        return Verdict::Kill(format!(
            "half_life_ms={} < horizon_ms={} (decay быстрее удержания)",
            inputs.half_life_ms, inputs.horizon_ms
        ));
    }
    if inputs.worst_stress_net_pnl_e8 < 0 {
        return Verdict::Kill(format!(
            "worst_stress_net_pnl_e8={} < 0 (отрицательный stress PnL)",
            inputs.worst_stress_net_pnl_e8
        ));
    }

    if inputs.sharpe - 2.0 * inputs.se_sharpe > bar {
        Verdict::Pass
    } else {
        Verdict::Inconclusive(format!(
            "нижняя 95%-граница Sharpe={:.6} <= bar={:.6}; данных недостаточно для Pass",
            inputs.sharpe - 2.0 * inputs.se_sharpe,
            bar
        ))
    }
}

/// Проверить обязательные provenance-поля R-001 до записи артефакта.
pub fn validate_report_honesty(report: &ReportHonesty) -> Result<(), String> {
    if report.gap_ref.trim().is_empty() {
        return Err("gap_ref пуст: отчёт не ссылается на E8 gap-артефакт".into());
    }
    let ledger_cutoff = report.ledger_cutoff.trim();
    if ledger_cutoff.is_empty() {
        return Err("ledger_cutoff пуст: эпоха trials-ledger не названа".into());
    }
    if ledger_cutoff == "f7f4761" {
        return Err(
            "ledger_cutoff=f7f4761: пре-M-07 эпоха TD-015 несопоставима с текущей логикой".into(),
        );
    }
    if !report.data_span_days.is_finite() || report.data_span_days <= 0.0 {
        return Err(format!(
            "data_span_days должен быть конечным и > 0, получен {}",
            report.data_span_days
        ));
    }
    if !report.se_sharpe.is_finite() || report.se_sharpe < 0.0 {
        return Err(format!(
            "se_sharpe должен быть конечным и >= 0, получен {}",
            report.se_sharpe
        ));
    }
    Ok(())
}

/// Параметры первого сквозного R-001 запуска. Входной поток уже прочитан
/// read-only journal-хэндлом; `run_r001` не имеет writer-а журнала.
pub struct R001RunConfig<'a> {
    pub events: &'a [Event],
    pub source: &'a JournalSource,
    pub ledger: &'a mut Ledger,
    pub latency: &'a LatencyTable,
    pub fees: &'a FeeSchedule,
    pub journal_sha256: String,
    pub report_dir: &'a Path,
    pub gap_dir: &'a Path,
    pub seed: u64,
}

const R001_SIGNAL_FAMILY: &str = "obi";
const R001_SIGNAL_ID_PREFIX: &str = "S-001";
const R001_HORIZONS_MS: [i64; 4] = [500, 1_000, 2_000, 5_000];
const R001_N_LEVELS: [u64; 4] = [1, 5, 10, 20];
const R001_THETAS_E8: [i64; 4] = [10_000_000, 20_000_000, 30_000_000, 40_000_000];
const R001_TOP_K: usize = 5;
const R001_BAR: f64 = 0.5;
const PERIODS_PER_YEAR: f64 = 252.0;
const MS_PER_DAY: f64 = 86_400_000.0;

fn r001_spec(cells: Vec<serde_json::Value>, costs_mode: CostsMode, seed: u64) -> GridSpec {
    GridSpec {
        signal_family: R001_SIGNAL_FAMILY.to_string(),
        signal_id_prefix: R001_SIGNAL_ID_PREFIX.to_string(),
        cells,
        costs_mode,
        seed,
    }
}

fn r001_cells() -> Vec<serde_json::Value> {
    let mut cells =
        Vec::with_capacity(R001_N_LEVELS.len() * R001_THETAS_E8.len() * R001_HORIZONS_MS.len());
    for n_levels in R001_N_LEVELS {
        for theta_e8 in R001_THETAS_E8 {
            for horizon_ms in R001_HORIZONS_MS {
                cells.push(serde_json::json!({
                    "mode": "top_n",
                    "n_levels": n_levels,
                    "theta_e8": theta_e8,
                    "horizon_ms": horizon_ms,
                    "venue": "Binance",
                    "symbol": "BTCUSDT"
                }));
            }
        }
    }
    cells
}

type TimeRange = (i64, i64);
type SplitRanges = (TimeRange, TimeRange);

fn split_range(events: &[Event]) -> Result<SplitRanges, RcError> {
    let first = events
        .iter()
        .map(|event| event.ts_wall_ms)
        .min()
        .ok_or_else(|| RcError::CorruptInput("R-001: журнал пуст".into()))?;
    let last = events
        .iter()
        .map(|event| event.ts_wall_ms)
        .max()
        .ok_or_else(|| RcError::CorruptInput("R-001: журнал пуст".into()))?;
    if last <= first {
        return Err(RcError::CorruptInput(
            "R-001: окно журнала имеет нулевую длину".into(),
        ));
    }
    // 2/3 train, 1/3 OOS. Полуинтервалы сохраняют последнюю точку в OOS.
    let span = (last as i128) - (first as i128);
    let train_end = (first as i128 + span * 2 / 3).clamp(i64::MIN as i128, i64::MAX as i128) as i64;
    Ok(((first, train_end), (train_end, last.saturating_add(1))))
}

fn grid_env<'a>(
    ledger: &'a mut Ledger,
    latency: &'a LatencyTable,
    fees: &'a FeeSchedule,
) -> GridRunEnv<'a> {
    GridRunEnv {
        ledger,
        latency,
        fees,
    }
}

fn variance(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64
}

/// Conservative standard error for an annualized Sharpe estimate.
///
/// At short horizons an empty/one-point equity series does not mean certainty:
/// the effective observation count is clamped to one, yielding a wide finite SE
/// and therefore `Inconclusive`, not a false `Pass`.
fn sharpe_se(returns: &[f64], sharpe: f64) -> f64 {
    let observations = returns.len().max(1) as f64;
    (PERIODS_PER_YEAR / observations * (1.0 + 0.5 * sharpe * sharpe)).sqrt()
}

fn min_stress_pnl(stress: &[StressResult]) -> i64 {
    stress
        .iter()
        .map(|result| result.net_pnl_e8)
        .min()
        .unwrap_or(0)
}

fn half_life_from_decay(decay: &[(i64, f64)], baseline: f64) -> i64 {
    if decay.is_empty() {
        return 0;
    }
    if baseline > 0.0 {
        if let Some((horizon, _)) = decay.iter().find(|(_, value)| *value <= baseline * 0.5) {
            return *horizon;
        }
    }
    decay.last().map(|(horizon, _)| *horizon).unwrap_or(0)
}

fn selected_result(results: &[CellResult]) -> Result<CellResult, RcError> {
    results
        .first()
        .cloned()
        .ok_or_else(|| RcError::CorruptInput("R-001: OOS grid не вернул ячеек".into()))
}

/// Выполнить R-001 Track A и записать оба детерминированных артефакта.
///
/// Путь исполнения намеренно состоит только из существующих компонентов:
/// `grid::run_grid` → `top_k` → `StrategyBacktest` внутри grid →
/// `run_walkforward` → отдельные cost/latency grids. Никаких ad-hoc сигналов,
/// ручной правки чисел или post-hoc пересчёта stress PnL.
pub fn run_r001(config: R001RunConfig<'_>) -> Result<ValidationReport, RcError> {
    let (train_range, oos_range) = split_range(config.events)?;
    let gap_report = data_quality::gaps(config.source, data_quality::DEFAULT_GAP_THRESHOLD_MS)?;
    let epoch = gap_report
        .epoch_ids
        .first()
        .cloned()
        .ok_or_else(|| RcError::CorruptInput("R-001: gap-артефакт не назвал эпоху".into()))?;
    data_quality::write_gap_artifact(&gap_report, config.gap_dir)?;
    let gap_ref = format!("research/data-quality/gaps-{epoch}.json");

    let cells = r001_cells();
    let baseline_spec = r001_spec(cells, CostsMode::Baseline, config.seed);
    let train_results = {
        let mut env = grid_env(config.ledger, config.latency, config.fees);
        grid::run_grid(
            config.events,
            &baseline_spec,
            crate::types::SplitKind::Train,
            train_range,
            &mut env,
            None,
        )?
    };
    let top_results = grid::top_k(&train_results, R001_TOP_K);
    if top_results.is_empty() {
        return Err(RcError::CorruptInput("R-001: train grid пуст".into()));
    }
    let top_cells: Vec<serde_json::Value> = top_results
        .iter()
        .map(|result| result.params.clone())
        .collect();

    let oos_spec = r001_spec(top_cells.clone(), CostsMode::Baseline, config.seed);
    let oos_results = {
        let mut env = grid_env(config.ledger, config.latency, config.fees);
        grid::run_grid(
            config.events,
            &oos_spec,
            crate::types::SplitKind::Val,
            oos_range,
            &mut env,
            None,
        )?
    };
    let best_oos = selected_result(&grid::top_k(&oos_results, 1))?;

    // Stress-режимы — два независимых grid-прогона, поэтому каждая их ячейка
    // получает собственную params_hash и ledger-запись.
    let mut stress = Vec::with_capacity(2);
    for costs_mode in [CostsMode::CostX15, CostsMode::LatencyX2] {
        let stress_spec = r001_spec(vec![best_oos.params.clone()], costs_mode, config.seed);
        let results = {
            let mut env = grid_env(config.ledger, config.latency, config.fees);
            grid::run_grid(
                config.events,
                &stress_spec,
                crate::types::SplitKind::Val,
                oos_range,
                &mut env,
                None,
            )?
        };
        let result = selected_result(&grid::top_k(&results, 1))?;
        stress.push(StressResult {
            mode: costs_mode,
            sharpe: result.sharpe,
            net_pnl_e8: result.net_pnl_e8,
        });
    }

    let walkforward = crate::walkforward::run_walkforward(
        config.events,
        &oos_spec,
        &crate::types::WalkForwardWindow {
            train_window_ms: 4 * 60 * 60 * 1_000,
            test_window_ms: 60 * 60 * 1_000,
            step_ms: 60 * 60 * 1_000,
        },
        config.ledger,
        config.latency,
        config.fees,
    )?;
    let walkforward_sharpes: Vec<f64> =
        walkforward.iter().map(|window| window.oos_sharpe).collect();
    let walkforward_min = walkforward_sharpes
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0);

    // Decay — те же top-N/theta, но каждый горизонт снова проходит через grid.
    // Это сохраняет честное исполнение и не подменяет decay формулой после PnL.
    let best_mode = best_oos
        .params
        .get("mode")
        .cloned()
        .unwrap_or_else(|| serde_json::json!("top_n"));
    let best_n = best_oos
        .params
        .get("n_levels")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(1));
    let best_theta = best_oos
        .params
        .get("theta_e8")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(10_000_000));
    let best_venue = best_oos
        .params
        .get("venue")
        .cloned()
        .unwrap_or_else(|| serde_json::json!("Binance"));
    let best_symbol = best_oos
        .params
        .get("symbol")
        .cloned()
        .unwrap_or_else(|| serde_json::json!("BTCUSDT"));
    let mut decay = Vec::with_capacity(R001_HORIZONS_MS.len());
    for horizon_ms in R001_HORIZONS_MS {
        let cell = serde_json::json!({
            "mode": best_mode.clone(),
            "n_levels": best_n.clone(),
            "theta_e8": best_theta.clone(),
            "horizon_ms": horizon_ms,
            "venue": best_venue.clone(),
            "symbol": best_symbol.clone()
        });
        let decay_spec = r001_spec(vec![cell], CostsMode::Baseline, config.seed);
        let results = {
            let mut env = grid_env(config.ledger, config.latency, config.fees);
            grid::run_grid(
                config.events,
                &decay_spec,
                crate::types::SplitKind::Val,
                oos_range,
                &mut env,
                None,
            )?
        };
        let result = selected_result(&results)?;
        decay.push((horizon_ms, result.sharpe));
    }

    let code_hash = grid::research_code_hash();
    let trials = config
        .ledger
        .trial_count_for_code_hash(R001_SIGNAL_FAMILY, &code_hash)
        .map_err(RcError::Io)?;
    let family_sharpes = config
        .ledger
        .family_sharpes_for_code_hash(R001_SIGNAL_FAMILY, &code_hash)
        .map_err(RcError::Io)?;
    let deflated_sharpe = metrics::deflated_sharpe(
        best_oos.sharpe,
        best_oos.returns.len(),
        0.0,
        3.0,
        &trials,
        variance(&family_sharpes),
    );
    let span_days = (oos_range.1.saturating_sub(oos_range.0) as f64) / MS_PER_DAY;
    let se_sharpe = sharpe_se(&best_oos.returns, best_oos.sharpe);
    let honesty = ReportHonesty {
        data_span_days: span_days,
        se_sharpe,
        gap_ref: gap_ref.clone(),
        ledger_cutoff: grid::LEDGER_EPOCH_CUTOFF.to_string(),
    };
    validate_report_honesty(&honesty).map_err(RcError::GateDenied)?;

    let inputs = KillScreenInputs {
        sharpe: best_oos.sharpe,
        se_sharpe,
        data_span_days: span_days,
        oos_sharpe: best_oos.sharpe,
        deflated_sharpe,
        walkforward_min_sharpe: walkforward_min,
        half_life_ms: half_life_from_decay(&decay, best_oos.sharpe),
        horizon_ms: best_oos
            .params
            .get("horizon_ms")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(R001_HORIZONS_MS[0]),
        worst_stress_net_pnl_e8: min_stress_pnl(&stress),
    };
    let verdict = classify_verdict(&inputs, R001_BAR);

    let report = ValidationReport {
        report_schema_version: REPORT_SCHEMA_VERSION,
        hypothesis: "H-20260710-obi-asym".into(),
        signal_id: "S-001-obi-asym".into(),
        params: best_oos.params.clone(),
        journal_sha256: config.journal_sha256,
        code_hash,
        ledger_n: trials.n(),
        net_pnl_e8: best_oos.net_pnl_e8,
        sharpe: best_oos.sharpe,
        deflated_sharpe,
        max_drawdown_e8: best_oos.max_drawdown_e8,
        fill_rate: metrics::fill_rate(best_oos.fills, best_oos.intents),
        turnover_e8: best_oos.turnover_e8,
        capacity_notional_e8: {
            let mut turnover = [best_oos.turnover_e8.abs()];
            metrics::capacity_v1_e8(&mut turnover, 0.05)
        },
        capacity_method: "v1-participation".into(),
        decay,
        stress,
        walkforward_sharpes,
        data_span_days: honesty.data_span_days,
        se_sharpe: honesty.se_sharpe,
        verdict,
        gap_ref: honesty.gap_ref,
        ledger_cutoff: honesty.ledger_cutoff,
    };

    fs::create_dir_all(config.report_dir).map_err(RcError::Io)?;
    let json_path = config.report_dir.join("R-001-obi-trackA.json");
    let md_path = config.report_dir.join("R-001-obi-trackA.md");
    write_metrics_json(&report, &json_path)?;
    write_narrative_md(&report, &md_path)?;
    Ok(report)
}

/// формат имени файла; research-cli читает журнал read-only, без writer-хэндла, RC-I-7).
const JOURNAL_SEGMENT_FILE: &str = "segment-00000000.jrnl";

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// sha256 файла сегмента журнала (вход воспроизводимости отчёта).
pub fn journal_sha256(journal_dir: &Path) -> Result<String, RcError> {
    let path = journal_dir.join(JOURNAL_SEGMENT_FILE);
    let bytes = fs::read(&path).map_err(RcError::Io)?;
    Ok(sha256_hex(&bytes))
}

/// Проверить пре-регистрацию: карточка существует и содержит непустой раздел
/// критериев фальсификации (грепается заголовок «критерии фальсификации»).
pub fn require_preregistration(hypothesis_card: &Path) -> Result<(), RcError> {
    let content = fs::read_to_string(hypothesis_card).map_err(|e| {
        RcError::PreRegistrationMissing(format!(
            "{}: карточка не найдена ({e})",
            hypothesis_card.display()
        ))
    })?;
    let lower = content.to_lowercase();
    const MARKER: &str = "критерии фальсификации";
    let idx = lower.find(MARKER).ok_or_else(|| {
        RcError::PreRegistrationMissing(format!(
            "{}: раздел «критерии фальсификации» не найден",
            hypothesis_card.display()
        ))
    })?;

    // Всё после заголовка (пропускаем остаток строки заголовка), до следующего "## "
    // заголовка или конца файла — там ищем хотя бы один непустой пункт (буллет).
    let after = &content[idx + MARKER.len()..];
    let has_bullet = after
        .lines()
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with("## "))
        .any(|l| {
            let t = l.trim();
            !t.is_empty()
                && (t.starts_with('-')
                    || t.starts_with('*')
                    || t.starts_with(|c: char| c.is_ascii_digit()))
        });
    if !has_bullet {
        return Err(RcError::PreRegistrationMissing(format!(
            "{}: раздел критериев фальсификации пуст",
            hypothesis_card.display()
        )));
    }
    Ok(())
}

/// Записать metrics.json детерминированно (serde_json по фиксированному порядку
/// полей структуры; повторный вызов с тем же отчётом → байт-идентичный файл).
pub fn write_metrics_json(report: &ValidationReport, path: &Path) -> Result<(), RcError> {
    validate_report_honesty(&ReportHonesty {
        data_span_days: report.data_span_days,
        se_sharpe: report.se_sharpe,
        gap_ref: report.gap_ref.clone(),
        ledger_cutoff: report.ledger_cutoff.clone(),
    })
    .map_err(RcError::GateDenied)?;
    let mut json =
        serde_json::to_string_pretty(report).map_err(|e| RcError::Parse(e.to_string()))?;
    json.push('\n');
    fs::write(path, json).map_err(RcError::Io)
}

/// Шаблонный нарратив R-NNN.md из чисел отчёта (детерминированный, без LLM).
pub fn write_narrative_md(report: &ValidationReport, path: &Path) -> Result<(), RcError> {
    let mut s = String::new();
    s.push_str(&format!(
        "# {} — {}\n\n",
        report.hypothesis, report.signal_id
    ));
    s.push_str(&format!(
        "- report_schema_version: {}\n",
        report.report_schema_version
    ));
    s.push_str(&format!("- journal_sha256: {}\n", report.journal_sha256));
    s.push_str(&format!("- code_hash: {}\n", report.code_hash));
    s.push_str(&format!(
        "- ledger_n (счётчик семейства): {}\n",
        report.ledger_n
    ));
    s.push_str(&format!("- net_pnl_e8: {}\n", report.net_pnl_e8));
    s.push_str(&format!("- sharpe: {:.6}\n", report.sharpe));
    s.push_str(&format!("- se_sharpe: {:.6}\n", report.se_sharpe));
    s.push_str(&format!("- data_span_days: {:.6}\n", report.data_span_days));
    s.push_str(&format!("- verdict: {:?}\n", report.verdict));
    s.push_str(&format!("- gap_ref: {}\n", report.gap_ref));
    s.push_str(&format!("- ledger_cutoff: {}\n", report.ledger_cutoff));
    s.push_str(&format!(
        "- deflated_sharpe: {:.6}\n",
        report.deflated_sharpe
    ));
    s.push_str(&format!("- max_drawdown_e8: {}\n", report.max_drawdown_e8));
    s.push_str(&format!("- fill_rate: {:.6}\n", report.fill_rate));
    s.push_str(&format!("- turnover_e8: {}\n", report.turnover_e8));
    s.push_str(&format!(
        "- capacity_notional_e8: {} ({})\n",
        report.capacity_notional_e8, report.capacity_method
    ));

    s.push_str("\n## Decay (horizon_ms, sharpe)\n");
    for (h, sh) in &report.decay {
        s.push_str(&format!("- {h}ms: {sh:.6}\n"));
    }

    s.push_str("\n## Stress\n");
    for st in &report.stress {
        s.push_str(&format!(
            "- {:?}: sharpe={:.6} net_pnl_e8={}\n",
            st.mode, st.sharpe, st.net_pnl_e8
        ));
    }

    s.push_str("\n## Walk-forward Sharpes\n");
    for sh in &report.walkforward_sharpes {
        s.push_str(&format!("- {sh:.6}\n"));
    }

    fs::write(path, s).map_err(RcError::Io)
}
