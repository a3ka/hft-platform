//! Структурные канарейки signals (sacred): SG-I-3, SG-I-4, SG-I-5, SG-I-10.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn workspace_root() -> PathBuf {
    crate_root().join("../..").canonicalize().unwrap()
}

fn src_files() -> Vec<PathBuf> {
    fs::read_dir(crate_root().join("src"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect()
}

#[test]
fn test_no_io_surface() {
    // SG-I-3: никакого I/O в hot-path. Исключение: registry.rs — boot-time I/O
    // санкционирован FA §3 (outer-слой, загрузка реестра РОВНО ОДИН РАЗ на старте).
    for f in src_files() {
        if f.file_name().is_some_and(|n| n == "registry.rs") {
            continue;
        }
        let content = fs::read_to_string(&f).unwrap();
        for pat in ["std::fs", "std::net", "reqwest", "tokio::net", "File::"] {
            assert!(
                !content.contains(pat),
                "SG-I-3: {} содержит `{pat}` — I/O в hot-path запрещён",
                f.display()
            );
        }
    }
}

#[test]
fn test_no_wallclock_surface() {
    // SG-I-4: время только из Event; часы запрещены ВЕЗДЕ в src (включая registry).
    for f in src_files() {
        let content = fs::read_to_string(&f).unwrap();
        for pat in ["Instant::now", "SystemTime::now", "Utc::now", "Local::now"] {
            assert!(
                !content.contains(pat),
                "SG-I-4: {} содержит `{pat}`",
                f.display()
            );
        }
    }
}

#[test]
fn test_dependency_direction() {
    // SG-I-5: только вниз (contracts, book); oms/risk/venues/strategy/portfolio — запрещены.
    let toml = fs::read_to_string(crate_root().join("Cargo.toml")).unwrap();
    for forbidden in [
        "oms",
        "risk",
        "venue-",
        "strategy",
        "portfolio",
        "alpha",
        "sim",
    ] {
        let dep_line = format!("\n{forbidden}");
        assert!(
            !toml.contains(&dep_line),
            "SG-I-5: crates/signals зависит от `{forbidden}` — направление вверх запрещено"
        );
    }
}

#[test]
fn test_new_signal_requires_spec_and_tests() {
    // SG-I-10: каждый сигнальный модуль src/<name>.rs (содержит `impl Signal for`)
    // обязан иметь tests/test_<name>_determinism.rs + research/specs/S-NNN-<name>*.md.
    let specs_dir = workspace_root().join("research/specs");
    for f in src_files() {
        let content = fs::read_to_string(&f).unwrap();
        let name = f.file_stem().unwrap().to_string_lossy().to_string();
        if name == "lib" || name == "bank" || name == "registry" {
            continue;
        }
        if !content.contains("impl Signal for") {
            continue;
        }
        let det_test = crate_root().join(format!("tests/test_{name}_determinism.rs"));
        assert!(
            det_test.exists(),
            "SG-I-10: сигнал `{name}` без детерминизм-теста {}",
            det_test.display()
        );
        let has_spec = fs::read_dir(&specs_dir)
            .map(|rd| {
                rd.flatten().any(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.starts_with("S-") && n.contains(&name) && n.ends_with(".md")
                })
            })
            .unwrap_or(false);
        assert!(
            has_spec,
            "SG-I-10: сигнал `{name}` без SignalSpec-карточки в research/specs/"
        );
    }
}
