//! RED-suite research-cli (sacred; docs/fa/research-cli.md §T):
//! RC-I-2,3,4,5,8,9,10 + метрики + грид-дисциплина. Падают на todo!-заглушках.

use std::fs;
use std::path::{Path, PathBuf};

use contracts::{to_fixed, Event, EventKind, Level, MdPayload, Side, Venue};
use research_cli::grid::{run_grid, GridRunEnv};
use research_cli::ledger::Ledger;
use research_cli::metrics;
use research_cli::report::{require_preregistration, write_metrics_json};
use research_cli::split::SplitState;
use research_cli::types::{
    CostsMode, GridSpec, RcError, SplitKind, StressResult, TimeSplit, TrialRecord,
    ValidationReport, Verdict, REPORT_SCHEMA_VERSION, TRIALS_LEDGER_SCHEMA_VERSION,
};
use sim::{FeeRates, FeeSchedule, LatencyTable};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

// ── фикстуры ────────────────────────────────────────────────────────────────

fn rec(family: &str, split: SplitKind, sharpe: Option<f64>, result_ref: &str) -> TrialRecord {
    TrialRecord {
        schema_version: TRIALS_LEDGER_SCHEMA_VERSION,
        signal_family: family.into(),
        signal_id: "S-001-obi-asym".into(),
        params_hash: "abc123".into(),
        split,
        costs_mode: CostsMode::Baseline,
        ts_wall_ms: 1_700_000_000_000,
        code_hash: "deadbeef".into(),
        result_ref: result_ref.into(),
        sharpe,
        prev_sha256: String::new(), // проставляет append
    }
}

fn split_0_to_3() -> TimeSplit {
    TimeSplit {
        train_ms: (0, 1_000),
        val_ms: (1_000, 2_000),
        test_ms: (2_000, 3_000),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// Синтетический поток: перекошенные снапшоты + трейды (сигналы + материал для fills).
fn synthetic_events() -> Vec<Event> {
    let mut evs = Vec::new();
    let mut seq = 0u64;
    for i in 0..60u64 {
        let ts_ms = (i * 200) as i64; // 0..12s
        seq += 1;
        evs.push(Event {
            seq,
            ts_mono_ns: (ts_ms as u64) * 1_000_000,
            ts_wall_ms: ts_ms,
            kind: EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::L2Snapshot {
                    bids: vec![lvl(100.0, 50.0), lvl(99.9, 40.0)],
                    asks: vec![lvl(100.1, 5.0), lvl(100.2, 5.0)],
                    ts_exch_ms: ts_ms,
                },
            ),
        });
        seq += 1;
        evs.push(Event {
            seq,
            ts_mono_ns: (ts_ms as u64) * 1_000_000 + 50_000_000,
            ts_wall_ms: ts_ms + 50,
            kind: EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(100.1),
                    size: to_fixed(2.0),
                    side: Side::Buy,
                    ts_exch_ms: ts_ms + 50,
                },
            ),
        });
    }
    evs
}

fn latency() -> LatencyTable {
    let mut t = LatencyTable::new();
    t.insert_samples(
        Venue::Binance,
        "BTCUSDT",
        vec![1_000_000],
        vec![1_000_000],
        vec![500_000],
        "synthetic-test-fixture",
    );
    t
}

fn fee_sched() -> FeeSchedule {
    let mut f = FeeSchedule::new();
    f.insert_rates(
        Venue::Binance,
        FeeRates {
            maker_rate_e8: 10_000,
            taker_rate_e8: 45_000,
        },
    );
    f
}

fn obi_cell() -> serde_json::Value {
    serde_json::json!({
        "mode": "top_n", "n_levels": 5, "theta_e8": 20_000_000,
        "horizon_ms": 1_000, "venue": "Binance", "symbol": "BTCUSDT"
    })
}

fn grid_spec(mode: CostsMode) -> GridSpec {
    GridSpec {
        signal_family: "obi".into(),
        signal_id_prefix: "S-001".into(),
        cells: vec![obi_cell()],
        costs_mode: mode,
        seed: 42,
    }
}

// ── RC-I-2 / D8 / RC-I-9: trials-ledger append-only + hash-chain ───────────

#[test]
fn test_ledger_append_only() {
    // Осознанное ограничение оракула (critic C-001 m2): тест проверяет append-only
    // ЭФФЕКТ (префикс-сохранность байт), не механизм O_APPEND как таковой.
    // Механизм — требование FA §6 к реализации (research-dev: OpenOptions::append);
    // rewrite-реализация, проходящая этот тест, будет поймана на ревью diff'а.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trials-ledger.jsonl");
    let mut l = Ledger::open(&path).unwrap();
    l.append(rec("obi", SplitKind::Train, Some(1.0), "r1"))
        .unwrap();
    let bytes1 = fs::read(&path).unwrap();
    l.append(rec("obi", SplitKind::Train, Some(0.5), "r2"))
        .unwrap();
    let bytes2 = fs::read(&path).unwrap();
    assert!(
        bytes2.starts_with(&bytes1),
        "RC-I-2: старое содержимое обязано остаться байт-в-байт префиксом"
    );
    assert_eq!(l.read_all().unwrap().len(), 2);

    // переоткрытие не переписывает
    let mut l2 = Ledger::open(&path).unwrap();
    l2.append(rec("obi", SplitKind::Val, Some(0.2), "r3"))
        .unwrap();
    let bytes3 = fs::read(&path).unwrap();
    assert!(bytes3.starts_with(&bytes2));
    assert_eq!(l2.read_all().unwrap().len(), 3);
}

#[test]
fn test_ledger_hash_chain_detects_tampering() {
    // D8: ручное редактирование файла в обход инструмента обнаруживается.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trials-ledger.jsonl");
    let mut l = Ledger::open(&path).unwrap();
    for i in 0..3 {
        l.append(rec(
            "obi",
            SplitKind::Train,
            Some(i as f64),
            &format!("r{i}"),
        ))
        .unwrap();
    }
    assert!(l.verify_chain().unwrap(), "нетронутая цепочка валидна");

    let mut bytes = fs::read(&path).unwrap();
    let mid = bytes.len() / 2;
    // портим цифру в середине файла (не таргетируем перевод строки)
    let pos = (mid..bytes.len())
        .find(|&i| bytes[i].is_ascii_digit())
        .unwrap();
    bytes[pos] = if bytes[pos] == b'0' { b'1' } else { b'0' };
    fs::write(&path, &bytes).unwrap();
    let l3 = Ledger::open(&path).unwrap();
    assert!(
        !l3.verify_chain().unwrap_or(false),
        "подмена байта обязана ломать hash-chain"
    );
}

#[test]
fn test_kill_results_not_deleted() {
    // RC-I-9: отрицательные результаты (KILL) не исчезают после новых записей.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trials-ledger.jsonl");
    let mut l = Ledger::open(&path).unwrap();
    l.append(rec("obi", SplitKind::Val, Some(-2.0), "KILL"))
        .unwrap();
    for i in 0..5 {
        l.append(rec(
            "obi",
            SplitKind::Train,
            Some(i as f64),
            &format!("r{i}"),
        ))
        .unwrap();
    }
    let all = l.read_all().unwrap();
    assert!(
        all.iter().any(|r| r.result_ref == "KILL"),
        "RC-I-9: KILL-запись обязана пережить все последующие append'ы"
    );
}

// ── RC-I-3: deflated Sharpe только от глобального ledger ───────────────────

#[test]
fn test_deflated_sharpe_reads_global_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trials-ledger.jsonl");
    let mut l = Ledger::open(&path).unwrap();
    l.append(rec("obi", SplitKind::Train, Some(0.8), "r0"))
        .unwrap();
    let n1 = l.trial_count("obi").unwrap();
    assert_eq!(n1.n(), 1);

    for i in 0..49 {
        l.append(rec(
            "obi",
            SplitKind::Train,
            Some(0.1 * (i % 10) as f64),
            &format!("r{i}"),
        ))
        .unwrap();
    }
    let n50 = l.trial_count("obi").unwrap();
    assert_eq!(n50.n(), 50);
    // чужое семейство не учитывается
    l.append(rec("meanrev", SplitKind::Train, Some(1.0), "x"))
        .unwrap();
    assert_eq!(l.trial_count("obi").unwrap().n(), 50);

    // D4: больше попыток → жёстче deflation (монотонность).
    // SVR-резолюция 2026-07-10 (architect): исходные параметры (sr=1.2, T=500)
    // насыщали Φ до ровно 1.0 в f64 для ОБОИХ N — монотонность была математически
    // невыполнима (research-dev honest-STOP, сверено с точным erf). Параметры
    // переведены в ненасыщающую зону: sr=0.3, T=60 → z ≈ 2.3 (N=1) vs ≈ −2.9 (N=50).
    let dsr_few = metrics::deflated_sharpe(0.3, 60, 0.0, 3.0, &n1, 0.09);
    let dsr_many = metrics::deflated_sharpe(0.3, 60, 0.0, 3.0, &n50, 0.09);
    assert!((0.0..=1.0).contains(&dsr_few));
    assert!((0.0..=1.0).contains(&dsr_many));
    assert!(
        dsr_few < 1.0,
        "параметры теста обязаны оставаться вне зоны насыщения Φ: dsr_few={dsr_few}"
    );
    assert!(
        dsr_many < dsr_few,
        "50 попыток обязаны дефлировать сильнее одной: {dsr_many} !< {dsr_few}"
    );
}

// ── RC-I-4 / RC-I-8: test-сегмент за val-гейтом, касание однократно ─────────

#[test]
fn test_test_segment_touch_once() {
    let mut st = SplitState::new("H-test", split_0_to_3());
    let tok = st.pass_val_gate(1.0, 0.5).expect("val-гейт пройден");
    assert!(st.val_gate_passed);
    let range = st.touch_test(&tok, None).unwrap();
    assert_eq!(range, (2_000, 3_000));
    assert!(st.test_touched);
    assert!(
        st.touch_test(&tok, None).is_err(),
        "RC-I-4: второе касание без override отклоняется"
    );
    st.touch_test(&tok, Some("повторная валидация после фикса бага харнесса"))
        .expect("override с обоснованием разрешён");
    assert!(
        st.touch_log.iter().any(|l| l.contains("фикса бага")),
        "обоснование обязано попасть в аудит-лог"
    );
}

#[test]
fn test_val_gate_fail_gives_no_token() {
    // RC-I-8 (рантайм-половина): провал критериев на val → токена нет.
    // Компиляционная половина: ValGateToken без публичного конструктора,
    // touch_test/run_grid(Test) требуют &ValGateToken.
    let mut st = SplitState::new("H-test", split_0_to_3());
    assert!(st.pass_val_gate(0.1, 0.5).is_err());
    assert!(!st.val_gate_passed);
}

#[test]
fn test_test_split_without_token_denied_in_grid() {
    // RC-I-8 на API грида: Test-прогон без токена → GateDenied.
    let dir = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(dir.path().join("ledger.jsonl")).unwrap();
    let (lat, fees) = (latency(), fee_sched());
    let mut env = GridRunEnv {
        ledger: &mut ledger,
        latency: &lat,
        fees: &fees,
    };
    let res = run_grid(
        &synthetic_events(),
        &grid_spec(CostsMode::Baseline),
        SplitKind::Test,
        (0, 100_000),
        &mut env,
        None,
    );
    assert!(matches!(res, Err(RcError::GateDenied(_))));
}

// ── FA §5: каждая ячейка → запись в ledger; RC-I-10 стресс — отдельные записи ──

#[test]
fn test_grid_ledgers_every_cell() {
    let dir = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(dir.path().join("ledger.jsonl")).unwrap();
    let mut spec = grid_spec(CostsMode::Baseline);
    let mut cell2 = obi_cell();
    cell2["theta_e8"] = serde_json::json!(40_000_000);
    spec.cells.push(cell2);

    let (lat, fees) = (latency(), fee_sched());
    let mut env = GridRunEnv {
        ledger: &mut ledger,
        latency: &lat,
        fees: &fees,
    };
    let results = run_grid(
        &synthetic_events(),
        &spec,
        SplitKind::Train,
        (0, 100_000),
        &mut env,
        None,
    )
    .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(
        ledger.read_all().unwrap().len(),
        2,
        "FA §5: КАЖДАЯ ячейка — запись, независимо от топ-K"
    );
    assert!(
        results.iter().any(|r| r.intents > 0),
        "анти-плацебо: перекошенный поток обязан порождать интенты"
    );
    assert!(
        results.iter().any(|r| r.fills > 0),
        "анти-плацебо: trades в потоке обязаны дать fills"
    );
}

#[test]
fn test_stress_variants_own_ledger_entries() {
    // RC-I-10: стресс — ОТДЕЛЬНЫЙ прогон с собственным params_hash, не пост-обработка.
    let dir = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(dir.path().join("ledger.jsonl")).unwrap();
    let evs = synthetic_events();
    let (lat, fees) = (latency(), fee_sched());
    let mut env = GridRunEnv {
        ledger: &mut ledger,
        latency: &lat,
        fees: &fees,
    };
    run_grid(
        &evs,
        &grid_spec(CostsMode::Baseline),
        SplitKind::Train,
        (0, 100_000),
        &mut env,
        None,
    )
    .unwrap();
    run_grid(
        &evs,
        &grid_spec(CostsMode::CostX15),
        SplitKind::Train,
        (0, 100_000),
        &mut env,
        None,
    )
    .unwrap();
    let all = ledger.read_all().unwrap();
    assert_eq!(all.len(), 2);
    assert_ne!(
        all[0].params_hash, all[1].params_hash,
        "RC-I-10: одинаковая ячейка при другом costs_mode обязана иметь ДРУГОЙ params_hash"
    );
    assert_eq!(all[1].costs_mode, CostsMode::CostX15);
}

// ── RC-I-5: детерминизм отчёта ──────────────────────────────────────────────

fn fixed_report() -> ValidationReport {
    ValidationReport {
        report_schema_version: REPORT_SCHEMA_VERSION,
        hypothesis: "H-20260710-obi-asym".into(),
        signal_id: "S-001-obi-asym".into(),
        params: obi_cell(),
        journal_sha256: "aa".repeat(32),
        code_hash: "bb".repeat(32),
        ledger_n: 7,
        net_pnl_e8: to_fixed(12.5),
        sharpe: 1.1,
        deflated_sharpe: 0.62,
        max_drawdown_e8: to_fixed(3.0),
        fill_rate: 0.4,
        turnover_e8: to_fixed(1000.0),
        capacity_notional_e8: to_fixed(500.0),
        capacity_method: "v1-participation".into(),
        decay: vec![(500, 1.3), (1_000, 1.1), (2_000, 0.6), (5_000, 0.1)],
        stress: vec![StressResult {
            mode: CostsMode::CostX15,
            sharpe: 0.7,
            net_pnl_e8: to_fixed(6.0),
        }],
        walkforward_sharpes: vec![0.9, 1.2, 0.8],
        // C-019 rev2: обязательные поля честности kill-screen (M-10). Значения фикстуры —
        // детерминизм RC-I-5, не классификация; вердикт Inconclusive (не Pass — не подразумеваем
        // промоушен), эпоха ≥5141fd9, gap_ref задан.
        data_span_days: 120.0,
        se_sharpe: 0.3,
        verdict: Verdict::Inconclusive("fixture: RC-I-5 determinism".into()),
        gap_ref: "research/data-quality/gaps-own-2026-07.json".into(),
        ledger_cutoff: "5141fd9".into(),
    }
}

#[test]
fn test_report_generation_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("m1.json");
    let p2 = dir.path().join("m2.json");
    let r = fixed_report();
    write_metrics_json(&r, &p1).unwrap();
    write_metrics_json(&r, &p2).unwrap();
    let b1 = fs::read(&p1).unwrap();
    let b2 = fs::read(&p2).unwrap();
    assert_eq!(
        b1, b2,
        "RC-I-5: тот же отчёт → байт-идентичный metrics.json"
    );
    assert!(!b1.is_empty());
    // и повторная запись в тот же путь стабильна
    write_metrics_json(&r, &p1).unwrap();
    assert_eq!(fs::read(&p1).unwrap(), b2);
}

// ── FA §8.1: пре-регистрация обязательна ────────────────────────────────────

#[test]
fn test_preregistration_required() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        require_preregistration(&dir.path().join("нет-такой-карточки.md")).is_err(),
        "без карточки финальная валидация не запускается"
    );
    let empty = dir.path().join("H-empty.md");
    fs::write(&empty, "# H-empty\nидея без критериев\n").unwrap();
    assert!(
        require_preregistration(&empty).is_err(),
        "карточка без раздела критериев фальсификации отклоняется"
    );
    let real = workspace_root().join("research/hypotheses/H-20260710-obi-asym.md");
    require_preregistration(&real).expect("настоящая пре-регистрированная карточка проходит");
}

// ── метрики: базовая корректность ───────────────────────────────────────────

#[test]
fn test_metrics_basics() {
    let up: Vec<f64> = (0..100).map(|_| 0.01).collect();
    assert!(
        metrics::sharpe(&up, 252.0) > 0.0,
        "монотонный рост → Sharpe > 0"
    );

    let eq: Vec<i64> = vec![
        0,
        to_fixed(10.0),
        to_fixed(4.0),
        to_fixed(8.0),
        to_fixed(2.0),
    ];
    assert_eq!(
        metrics::max_drawdown_e8(&eq),
        to_fixed(8.0),
        "пик 10 → дно 2 → maxDD 8"
    );

    assert!((metrics::fill_rate(2, 8) - 0.25).abs() < 1e-12);

    let mut vols = vec![to_fixed(100.0), to_fixed(300.0), to_fixed(200.0)];
    let cap = metrics::capacity_v1_e8(&mut vols, 0.05);
    assert_eq!(
        cap,
        (to_fixed(200.0) as f64 * 0.05) as i64,
        "D5: 5% от медианы"
    );
}
