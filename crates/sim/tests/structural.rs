//! Структурные канарейки sim (sacred): SM-I-3, SM-I-7 (греп-половина), SM-I-9.
//! Проходят с первого дня — их работа ловить БУДУЩУЮ деградацию (класс канареек
//! per FA §T; поведенческие RED-оракулы живут в red_sim.rs).

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                rs_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
}

#[test]
fn test_no_sim_only_branch_in_strategy_stack() {
    // SM-I-3: ни один крейт workspace не содержит cfg(sim)-веток бизнес-логики.
    let crates = workspace_root().join("crates");
    let mut files = Vec::new();
    for e in fs::read_dir(&crates).unwrap().flatten() {
        let src = e.path().join("src");
        rs_files(&src, &mut files);
    }
    assert!(!files.is_empty());
    for f in files {
        let content = fs::read_to_string(&f).unwrap();
        assert!(
            !content.contains("cfg(sim)") && !content.contains("feature = \"sim\""),
            "SM-I-3: {} содержит sim-специфичную ветку — один код для всех 4 режимов",
            f.display()
        );
    }
}

#[test]
fn test_sim_has_no_live_gateway_dependency() {
    // SM-I-9 (структурная половина): sim не зависит от venue-*/сетевых крейтов —
    // paper-fill физически не может уйти на биржу через этот крейт.
    let toml = fs::read_to_string(workspace_root().join("crates/sim/Cargo.toml")).unwrap();
    for forbidden in [
        "venue-binance",
        "venue-hyperliquid",
        "tokio-tungstenite",
        "reqwest",
        "hyper",
    ] {
        assert!(
            !toml.contains(forbidden),
            "SM-I-9: crates/sim/Cargo.toml содержит запрещённую зависимость `{forbidden}`"
        );
    }
}

#[test]
fn test_no_hardcoded_default_latency() {
    // SM-I-7 (греп-половина): в src нет пути с нулевой/захардкоженной задержкой.
    let mut files = Vec::new();
    rs_files(&workspace_root().join("crates/sim/src"), &mut files);
    assert!(!files.is_empty());
    for f in files {
        let content = fs::read_to_string(&f).unwrap();
        for pat in [
            "delta_submit_ns: 0",
            "DEFAULT_LATENCY",
            "default_latency",
            "unwrap_or(LatencyDraw",
        ] {
            assert!(
                !content.contains(pat),
                "SM-I-7: {} содержит `{pat}` — латентность только из измеренной таблицы",
                f.display()
            );
        }
    }
}

#[test]
fn test_no_rand_crate() {
    // D10: детерминизм — свой SplitMix64; rand-крейт меняет алгоритмы между версиями.
    let toml = fs::read_to_string(workspace_root().join("crates/sim/Cargo.toml")).unwrap();
    assert!(!toml.contains("\nrand"), "D10: rand запрещён в sim");
}
