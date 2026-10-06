//! RED `M-94` (`R-245` B-1; sacred, architect-only) — **у каждой величины профиля ОДНО правило
//! разбора: значение, отвергнутое без профиля, отвергается и в профиле, и наоборот; принятое —
//! даёт ту же конфигурацию выдачи** (спека §3.2 «Одна грамматика на величину», `A-049` Р-6, §5).
//!
//! # Почему
//!
//! `R-245` B-1 нашёл второй экземпляр правил: загрузчик профиля проверял полосы, таймфрейм,
//! каденцию и окно СВОИМИ условиями, а выдача без профиля — своими. Правила уже разошлись
//! (полосы `-0.1` без профиля приняты, в профиле — отказ). Два правила одной величины — это два
//! определения расчёта, против чего заведён `П-032`.
//!
//! # Как судится
//!
//! Прод-функция `gateway_serve::serve_config_from_env` (её зовёт `main` прод-бинаря) дважды на
//! каждое значение корпуса: (L) без профиля, ключ в окружении; (P) с профилем, тот же ключ — в
//! файле профиля, остальное — `config/calc-profile/active.env`. `GATEWAY_ALLOWED_PROFILES` в (P)
//! — тройка самого файла, чтобы правило ПРОФИЛЯ «тройка ∈ ALLOWED» не подменяло спор о грамматике.
//! Сравнивается: принято/отвергнуто; при приёме — селектор и применённые окно heatmap и шаг VP.
//!
//! Единственное названное отличие режимов — политика профиля, а не грамматика (спека §3.2):
//! профиль не допускает окна «без границы» (`GATEWAY_WINDOW_MS=0` вне профиля = offline, M-37 /
//! `C-099` B-2). Это судится отдельным миром `g2`, а не исключается молча.
//!
//! Отсутствие ключа и пустое значение вне профиля (дефолт/offline) — не значение и в корпус не
//! входят: в профиле все ключи обязательны (`p5`).
//!
//! # Сегодня (`2209a05`) — красен по предмету
//!
//! `g1`: полосы `-0.1`, `0`, `NaN`, `inf` без профиля приняты, в профиле — отказ (и т.п.).
//! RUNTIME-RED: только существующие публичные символы (урок `A-033`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("корень репозитория")
        .to_path_buf()
}

/// Профиль разбирается здесь НЕЗАВИСИМО от загрузчика dev'а.
fn active() -> Vec<(String, String)> {
    let p = repo_root().join("config/calc-profile/active.env");
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("SETUP НЕ СОСТОЯЛСЯ: нет {} ({e})", p.display()))
        .lines()
        .map(str::trim)
        .filter(|t| !t.is_empty() && !t.starts_with('#'))
        .filter_map(|t| t.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn base_env() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("GATEWAY_JWT_SECRET".into(), "ab".repeat(32));
    m.insert("GATEWAY_VENUE".into(), "Binance".into());
    m.insert("GATEWAY_SYMBOL".into(), "BTCUSDT".into());
    m
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Accepted {
        selector: String,
        heatmap: u64,
        vp: i64,
    },
    Refused(String),
}

fn run(env: &BTreeMap<String, String>) -> Outcome {
    match gateway_serve::serve_config_from_env(|k| env.get(k).cloned()) {
        Ok(cfg) => Outcome::Accepted {
            selector: format!("{:?}", cfg.selector),
            heatmap: gateway::effective_heatmap_window_frac().to_bits(),
            vp: gateway::effective_vp_bin_width_e8(),
        },
        Err(e) => Outcome::Refused(e),
    }
}

/// (L): ключ `k` = `v` в окружении, остальные величины профиля — из `active.env`, без профиля.
fn legacy(k: &str, v: &str) -> Outcome {
    let mut env = base_env();
    for (key, val) in active() {
        if key == "CALC_PROFILE_VERSION" || key == "GATEWAY_ALLOWED_PROFILES" {
            continue; // не величины `serve_config_from_env` вне профиля
        }
        env.insert(key, val);
    }
    env.insert(k.into(), v.into());
    run(&env)
}

/// (P): тот же ключ в файле профиля; ALLOWED — тройка самого файла (текстом).
fn profiled(k: &str, v: &str, dir: &Path) -> Outcome {
    let mut prof: BTreeMap<String, String> = active().into_iter().collect();
    prof.insert(k.into(), v.into());
    let triple = format!(
        "{}/{}/{}",
        prof["GATEWAY_TIMEFRAME_MS"], prof["GATEWAY_WINDOW_MS"], prof["GATEWAY_DEPTH_CADENCE_MS"]
    );
    prof.insert("GATEWAY_ALLOWED_PROFILES".into(), triple);
    let text: String = prof.iter().map(|(a, b)| format!("{a}={b}\n")).collect();
    let path = dir.join("profile.env");
    std::fs::write(&path, text).expect("запись профиля");
    let mut env = base_env();
    env.insert("GATEWAY_CALC_PROFILE".into(), path.display().to_string());
    run(&env)
}

fn agree(l: &Outcome, p: &Outcome) -> bool {
    match (l, p) {
        (Outcome::Refused(_), Outcome::Refused(_)) => true,
        (a @ Outcome::Accepted { .. }, b @ Outcome::Accepted { .. }) => a == b,
        _ => false,
    }
}

const CORPUS: &[(&str, &[&str])] = &[
    (
        "GATEWAY_BANDS",
        &[
            "0.015,0.03",
            "0.6",
            "-0.1",
            "0",
            "abc",
            "0.01,,0.02",
            "NaN",
            "inf",
            "0.01,-0.02",
        ],
    ),
    (
        "GATEWAY_TIMEFRAME_MS",
        &["1000", "500", "999", "0", "-1000", "abc", "604800000"],
    ),
    (
        "GATEWAY_WINDOW_MS",
        &["60000", "120000", "-1", "abc", "1e3"],
    ),
    (
        "GATEWAY_DEPTH_CADENCE_MS",
        &["1000", "2000", "999", "1500", "0", "-1000", "abc"],
    ),
    (
        "GATEWAY_HEATMAP_WINDOW",
        &["0.001", "0.5", "0", "1", "1.5", "-0.1", "abc", "NaN"],
    ),
    (
        "GATEWAY_VP_BIN_WIDTH_E8",
        &["25000000", "1", "0", "-1", "abc", "1.5", "1e8"],
    ),
];

/// Всё в ОДНОЙ функции: окно heatmap и шаг VP — глобальные значения процесса, параллельные тесты
/// перетирали бы их между вызовом и чтением.
#[test]
fn g1_one_grammar_per_value_g2_profile_policy_bounded_window() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Setup-страж: неизменённый активный профиль принят в ОБОИХ режимах и даёт одно и то же —
    // иначе сравнение ниже меряет окружение фикстуры, а не грамматику.
    let (k0, v0) = ("GATEWAY_TIMEFRAME_MS", "1000");
    let (l0, p0) = (legacy(k0, v0), profiled(k0, v0, dir.path()));
    assert!(
        matches!(l0, Outcome::Accepted { .. }) && l0 == p0,
        "SETUP НЕ СОСТОЯЛСЯ: активный профиль не принят одинаково в обоих режимах:\n  L={l0:?}\n  P={p0:?}"
    );

    let mut diverged = Vec::new();
    let mut accepted = 0usize;
    let mut refused = 0usize;
    for (k, values) in CORPUS {
        for v in *values {
            let l = legacy(k, v);
            let p = profiled(k, v, dir.path());
            match &l {
                Outcome::Accepted { .. } => accepted += 1,
                Outcome::Refused(_) => refused += 1,
            }
            if !agree(&l, &p) {
                diverged.push(format!(
                    "  {k}={v:?}\n    без профиля: {l:?}\n    в профиле:   {p:?}"
                ));
            }
        }
    }
    // Корпус обязан давить на обе стороны: иначе «всё отвергнуто» в обоих режимах — тавтология.
    assert!(
        accepted >= 6 && refused >= 15,
        "SETUP НЕ СОСТОЯЛСЯ: корпус вырожден (принято {accepted}, отвергнуто {refused})"
    );
    assert!(
        diverged.is_empty(),
        "M-94 / R-245 B-1 / спека §3.2: у величины ДВА правила разбора — значение судится по-разному \
         с профилем и без него ({} расхождений):\n{}",
        diverged.len(),
        diverged.join("\n")
    );

    // g2: политика профиля, названная явно, — окно без границы в профиле запрещено.
    let l = legacy("GATEWAY_WINDOW_MS", "0");
    match &l {
        Outcome::Accepted { selector, .. } => assert!(
            selector.contains("window_ms: None"),
            "SETUP НЕ СОСТОЯЛСЯ: вне профиля `GATEWAY_WINDOW_MS=0` не канонизирован в offline (M-37, \
             C-099 B-2): {selector}"
        ),
        Outcome::Refused(e) => panic!(
            "SETUP НЕ СОСТОЯЛСЯ: вне профиля `GATEWAY_WINDOW_MS=0` отвергнут ({e}) — сегодняшняя \
             семантика offline сломана, мир g2 ничего не различает"
        ),
    }
    match profiled("GATEWAY_WINDOW_MS", "0", dir.path()) {
        Outcome::Refused(e) => assert!(
            e.contains("GATEWAY_WINDOW_MS"),
            "M-94 / спека §3.2: отказ профиля с окном 0 обязан назвать ключ: {e}"
        ),
        a => panic!(
            "M-94 / спека §3.2 (политика профиля): профиль с `GATEWAY_WINDOW_MS=0` принят как offline \
             — определение расчёта прода обязано быть с границей окна (TD-020): {a:?}"
        ),
    }
}
