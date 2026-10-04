//! Генератор JSON Schema из Rust-типов (CT-I-4: схема ДЕРИВИРУЕТСЯ, не пишется руками).
//!
//! Запуск: `cargo run -p contracts --example gen_schema`
//! Пишет `crates/contracts/schema/*.schema.json`. Гейт `tests/red_schema.rs` падает, если
//! закоммиченная схема разошлась с типами (05 §4: схема — часть contract-RFC).

use std::path::Path;

use contracts::{Event, LegacyManifest, SegmentHeader};

fn write(path: &Path, schema: schemars::schema::RootSchema) {
    let json = serde_json::to_string_pretty(&schema).expect("serialize schema");
    std::fs::write(path, format!("{json}\n")).expect("write schema");
    println!("wrote {}", path.display());
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema");
    std::fs::create_dir_all(&dir).expect("mkdir schema");
    write(&dir.join("event.schema.json"), schemars::schema_for!(Event));
    write(
        &dir.join("segment-header.schema.json"),
        schemars::schema_for!(SegmentHeader),
    );
    write(
        &dir.join("legacy-manifest.schema.json"),
        schemars::schema_for!(LegacyManifest),
    );
}
