//! CT-I-4 / CT-I-5 (sacred, architect-only) — JSON Schema СГЕНЕРИРОВАНА из Rust-типов и
//! согласована с фикстурами. Требование `docs/05-contract-layer.md` §4/§6: схема — часть
//! contract-RFC, не «допишем потом» (находка critic C-005 C1).
//!
//! Анти-плацебо: тест падает, если кто-то правит Rust-тип и НЕ перегенерирует схему
//! (`cargo run -p contracts --example gen_schema`) — то есть если канон и схема разошлись.
//! Питоновский research-тулинг валидирует чтения против ЭТОЙ схемы (CT-I-5), поэтому
//! рассинхрон = молча неверная валидация на стороне деска.

use std::path::{Path, PathBuf};

use contracts::{Event, LegacyManifest, SegmentHeader};

fn schema_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schema")
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn committed(name: &str) -> String {
    std::fs::read_to_string(schema_dir().join(name))
        .unwrap_or_else(|e| panic!("схема {name} обязана быть в репозитории (CT-I-4): {e}"))
}

fn generated(schema: schemars::schema::RootSchema) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(&schema).expect("serialize")
    )
}

/// CT-I-4: закоммиченная схема == сгенерированная из типов.
#[test]
fn ct_i_4_committed_schema_matches_rust_types() {
    for (name, gen) in [
        ("event.schema.json", generated(schemars::schema_for!(Event))),
        (
            "segment-header.schema.json",
            generated(schemars::schema_for!(SegmentHeader)),
        ),
        (
            "legacy-manifest.schema.json",
            generated(schemars::schema_for!(LegacyManifest)),
        ),
    ] {
        assert_eq!(
            committed(name),
            gen,
            "схема {name} разошлась с Rust-типами — перегенерируй: \
             `cargo run -p contracts --example gen_schema` (CT-I-4: канон — типы, схема \
             деривируется; расхождение = деск валидирует данные против неверной схемы)"
        );
    }
}

/// Фикстуры valid/* обязаны разбираться, invalid/* — НЕТ (05 §4: фикстуры в том же RFC).
#[test]
fn ct_rfc02_fixtures_valid_parse_invalid_reject() {
    let read = |sub: &str| -> Vec<(PathBuf, String)> {
        let dir = fixtures_dir().join(sub);
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("фикстуры {sub}: {e}")) {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "json") {
                let body = std::fs::read_to_string(&p).expect("read fixture");
                out.push((p, body));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        assert!(!out.is_empty(), "каталог фикстур {sub} пуст");
        out
    };

    for (path, body) in read("valid") {
        let name = path.file_name().and_then(|s| s.to_str()).expect("name");
        let ok = if name.starts_with("segment-header") {
            serde_json::from_str::<SegmentHeader>(&body).is_ok()
        } else if name.starts_with("legacy-manifest") {
            serde_json::from_str::<LegacyManifest>(&body).is_ok()
        } else {
            serde_json::from_str::<Event>(&body).is_ok()
        };
        assert!(ok, "valid-фикстура {name} обязана разбираться");
    }

    for (path, body) in read("invalid") {
        let name = path.file_name().and_then(|s| s.to_str()).expect("name");
        let rejected = if name.starts_with("segment-header") {
            serde_json::from_str::<SegmentHeader>(&body).is_err()
        } else if name.starts_with("legacy-manifest") {
            serde_json::from_str::<LegacyManifest>(&body).is_err()
        } else {
            serde_json::from_str::<Event>(&body).is_err()
        };
        assert!(
            rejected,
            "invalid-фикстура {name} обязана быть ОТВЕРГНУТА (иначе мусор попадёт в T1)"
        );
    }
}
