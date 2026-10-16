//! Структурные канарейки research-cli (sacred): RC-I-1, RC-I-6, RC-I-7, RC-I-11.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
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
fn test_no_llm_dependency() {
    // RC-I-1/11: ноль LLM-клиентов и сетевых зависимостей — чистый compute.
    // комментарии не считаются: канарейка ловит ЗАВИСИМОСТИ, не слова в доках
    let toml = fs::read_to_string(crate_root().join("Cargo.toml"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "openai",
        "anthropic",
        "llm",
        "reqwest",
        "hyper",
        "tokio-tungstenite",
        "ureq",
        "curl",
    ] {
        assert!(
            !toml.to_lowercase().contains(forbidden),
            "RC-I-1: Cargo.toml содержит `{forbidden}`"
        );
    }
    for f in src_files() {
        let content = fs::read_to_string(&f).unwrap().to_lowercase();
        for forbidden in ["openai", "anthropic", "api_key", "std::net"] {
            assert!(
                !content.contains(forbidden),
                "RC-I-1/11: {} содержит `{forbidden}`",
                f.display()
            );
        }
    }
}

#[test]
fn test_no_write_to_signals_registry() {
    // RC-I-6: research-cli вообще не касается signals.json (грид инстанцирует
    // напрямую по SignalSpec — D11); граница B пишется снаружи через подпись.
    for f in src_files() {
        let content = fs::read_to_string(&f).unwrap();
        assert!(
            !content.contains("signals.json"),
            "RC-I-6: {} упоминает signals.json — реестр вне зоны research-cli",
            f.display()
        );
    }
}

#[test]
fn test_no_journal_write_path() {
    // RC-I-7: журнал read-only — писатель (journal::Journal) не появляется в src.
    for f in src_files() {
        let content = fs::read_to_string(&f).unwrap();
        for forbidden in ["Journal::open", "journal::Journal", ".append(EventKind"] {
            assert!(
                !content.contains(forbidden),
                "RC-I-7: {} содержит `{forbidden}` — read-only журнал",
                f.display()
            );
        }
    }
}
