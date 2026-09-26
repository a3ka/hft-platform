//! Структурные оракулы мозга стратегии (ST-I-6, ST-I-7). SACRED — architect-only.
//!
//! Это тесты об ОТСУТСТВИИ путей, а не о наличии проверок (паттерн INTG-I-*): «нельзя»
//! должно быть фактом Cargo-графа и исходников, а не дисциплиной автора.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Все .rs файлы в crates/<crate>/src.
fn src_files(krate: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, acc: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).expect("read_dir") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                walk(&p, acc);
            } else if p.extension().is_some_and(|x| x == "rs") {
                acc.push(p);
            }
        }
    }
    let mut acc = Vec::new();
    walk(&root().join("crates").join(krate).join("src"), &mut acc);
    acc
}

const BRAIN: [&str; 3] = ["alpha", "portfolio", "strategy"];

/// ST-I-6a: мозг стратегии структурно не знает про исполнение и I/O.
/// `strategy` не смеет зависеть от `sim` — иначе live-runner тащил бы симулятор,
/// и «backtest == live» превратилось бы в «live == backtest-обёртка».
#[test]
fn st_i_6a_brain_has_no_forbidden_dependencies() {
    let forbidden = [
        "sim",
        "venue-binance",
        "venue-binance-futures",
        "venue-hyperliquid",
        "journal",
        "recorder",
        "risk",
        "killswitch",
        "oms",
        "tokio",
        "reqwest",
        "rand",
        "fastrand",
        "chrono",
        "research-cli",
    ];
    for krate in BRAIN {
        let manifest = read(&format!("crates/{krate}/Cargo.toml"));
        for dep in forbidden {
            let needle = format!("\n{dep} =");
            assert!(
                !manifest.contains(&needle),
                "{krate}/Cargo.toml зависит от `{dep}` — мозг стратегии обязан быть \
                 чистым редьюсером (docs/fa/strategy-brain.md §2)"
            );
        }
    }
}

/// ST-I-6b: никакого недетерминизма в редьюсерах — ни часов, ни рандома, ни файлов/сети.
#[test]
fn st_i_6b_brain_has_no_nondeterminism() {
    let forbidden = [
        "SystemTime",
        "Instant::now",
        "std::time",
        "rand::",
        "thread_rng",
        "std::fs",
        "std::net",
        "std::env",
    ];
    for krate in BRAIN {
        for f in src_files(krate) {
            let src = std::fs::read_to_string(&f).expect("read src");
            for needle in forbidden {
                assert!(
                    !src.contains(needle),
                    "{}: найден недетерминизм/IO `{needle}` — DESIGN §1 (журнал-принцип)",
                    f.display()
                );
            }
        }
    }
}

/// ST-I-6c: никакой итерации по HashMap/HashSet в редьюсерах (порядок обхода не определён →
/// replay перестаёт быть бит-идентичным). Только BTreeMap/BTreeSet/Vec.
/// Проверяем ИСПОЛЬЗОВАНИЕ (`HashMap<`, `collections::HashMap`), а не упоминание в комментарии.
#[test]
fn st_i_6c_brain_uses_ordered_collections_only() {
    for krate in BRAIN {
        for f in src_files(krate) {
            let src = std::fs::read_to_string(&f).expect("read src");
            for needle in [
                "HashMap<",
                "HashSet<",
                "collections::HashMap",
                "collections::HashSet",
            ] {
                assert!(
                    !src.contains(needle),
                    "{}: `{needle}` в редьюсере — порядок обхода недетерминирован \
                     (используй BTreeMap/BTreeSet)",
                    f.display()
                );
            }
        }
    }
}

/// ST-I-7 (канарейка, зеркало CT-I-1): `OrderIntent` определён РОВНО в одном крейте —
/// `strategy`. Два определения одной формы = болезнь hft-core-rs (три wire-формата).
#[test]
fn st_i_7_order_intent_defined_exactly_once() {
    let mut defs: Vec<String> = Vec::new();
    let crates_dir = root().join("crates");
    for e in std::fs::read_dir(&crates_dir).expect("read crates") {
        let krate = e.expect("entry").path();
        let name = krate
            .file_name()
            .and_then(|s| s.to_str())
            .expect("crate name")
            .to_string();
        if !krate.join("src").is_dir() {
            continue;
        }
        for f in src_files(&name) {
            let src = std::fs::read_to_string(&f).expect("read src");
            if src.contains("pub struct OrderIntent") {
                defs.push(f.display().to_string());
            }
        }
    }
    assert_eq!(
        defs.len(),
        1,
        "`pub struct OrderIntent` обязан быть определён ровно один раз, найдено: {defs:?}"
    );
    assert!(
        defs[0].contains("crates/strategy/"),
        "владелец формы OrderIntent — `strategy` (Слой 4, продюсер), найдено: {}",
        defs[0]
    );
}
