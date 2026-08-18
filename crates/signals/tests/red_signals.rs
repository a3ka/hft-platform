//! RED-suite signals (sacred; docs/fa/signals.md §T): SignalId, SignalBank (SG-I-9),
//! registry-загрузчик (SG-I-6/7/8/11). Обязаны падать на todo!-заглушках.

use std::fs;
use std::path::{Path, PathBuf};

use contracts::{to_fixed, Event, EventKind, Level, MdPayload, Venue};
use signals::bank::SignalBank;
use signals::registry::{self, RegistryEntry};
use signals::{
    RegistryStatus, Signal, SignalError, SignalId, SignalMeta, SignalOut, SignalSpecRef,
};

fn signals_src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn snap(seq: u64, ts_ms: u64) -> Event {
    Event {
        seq,
        ts_mono_ns: ts_ms * 1_000_000,
        ts_wall_ms: ts_ms as i64,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![Level {
                    price: to_fixed(100.0),
                    size: to_fixed(50.0),
                }],
                asks: vec![Level {
                    price: to_fixed(100.1),
                    size: to_fixed(1.0),
                }],
                ts_exch_ms: ts_ms as i64,
            },
        ),
    }
}

// ── SignalId ────────────────────────────────────────────────────────────────

#[test]
fn test_signal_id_format() {
    assert!(SignalId::parse("S-001-obi-asym").is_ok());
    assert!(SignalId::parse("S-042-mean-rev-2").is_ok());
    for bad in [
        "",
        "obi",
        "S-1-obi",
        "S-001",
        "s-001-obi",
        "S-001-ОБИ",
        "S-001-obi asym",
    ] {
        assert!(
            SignalId::parse(bad).is_err(),
            "`{bad}` обязан быть отвергнут (опечатка ≠ новый сигнал)"
        );
    }
    assert_eq!(
        SignalId::parse("S-001-obi-asym").unwrap().as_str(),
        "S-001-obi-asym"
    );
}

// ── SG-I-9: изоляция паники одного сигнала ─────────────────────────────────

struct Panicky {
    id: SignalId,
}
impl Signal for Panicky {
    fn on_event(&mut self, _ev: &Event) -> Option<SignalOut> {
        panic!("сигнал-саботажник (тестовый)");
    }
    fn spec(&self) -> SignalSpecRef {
        SignalSpecRef {
            id: self.id.clone(),
            version: 1,
        }
    }
}

struct Steady {
    id: SignalId,
    emitted: u64,
}
impl Signal for Steady {
    fn on_event(&mut self, ev: &Event) -> Option<SignalOut> {
        self.emitted += 1;
        Some(SignalOut {
            signal_id: self.id.clone(),
            ts_event_mono_ns: ev.ts_mono_ns,
            value: to_fixed(0.5),
            status: RegistryStatus::Candidate,
            meta: SignalMeta { horizon_ms: 1000 },
        })
    }
    fn spec(&self) -> SignalSpecRef {
        SignalSpecRef {
            id: self.id.clone(),
            version: 1,
        }
    }
}

#[test]
fn test_signal_panic_isolated() {
    // SG-I-9: паника ОДНОГО сигнала изолируется; здоровый сигнал и последующие
    // события продолжают обрабатываться; выдуманных SignalOut нет.
    let mut bank = SignalBank::new();
    bank.register(Box::new(Panicky {
        id: SignalId::parse("S-998-panicky").unwrap(),
    }));
    bank.register(Box::new(Steady {
        id: SignalId::parse("S-999-steady").unwrap(),
        emitted: 0,
    }));
    for i in 0..3u64 {
        let outs = bank.on_event(&snap(i + 1, 1_000 + i * 100));
        assert_eq!(outs.len(), 1, "тик {i}: только здоровый сигнал эмитит");
        assert_eq!(outs[0].signal_id.as_str(), "S-999-steady");
    }
}

// ── registry: SG-I-6/7/8/11 ────────────────────────────────────────────────

fn write_registry(dir: &Path, entries: &str) -> PathBuf {
    let p = dir.join("signals.json");
    fs::write(&p, entries).unwrap();
    p
}

fn valid_obi_params() -> String {
    r#"{"mode":"top_n","n_levels":5,"theta_e8":20000000,"horizon_ms":1000,"venue":"Binance","symbol":"BTCUSDT"}"#
        .to_string()
}

fn entry_json(code_hash: &str, status: &str, params: &str) -> String {
    format!(
        r#"[{{"signal_id":"S-001-obi-asym","version":1,"module":"obi","code_hash":"{code_hash}","status":"{status}","params":{params},"ensemble_weight":0.0}}]"#
    )
}

#[test]
fn test_registry_code_hash_mismatch_rejects_boot() {
    // SG-I-6: mismatch → Reject boot, не тихий skip.
    let dir = tempfile::tempdir().unwrap();
    let reg = write_registry(
        dir.path(),
        &entry_json("deadbeef", "candidate", &valid_obi_params()),
    );
    let res = registry::load_registry(&reg, &signals_src_root());
    assert!(
        matches!(res, Err(SignalError::CodeHashMismatch { .. })),
        "боевой отказ загрузки при неверном code_hash"
    );
}

#[test]
fn test_registry_loads_valid_and_skips_retired() {
    // SG-I-7: retired не инстанцируется; валидная запись — инстанцируется.
    let real_hash = registry::module_code_hash(&signals_src_root(), "obi").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let two = format!(
        r#"[{{"signal_id":"S-001-obi-asym","version":1,"module":"obi","code_hash":"{h}","status":"candidate","params":{p},"ensemble_weight":0.0}},
            {{"signal_id":"S-002-obi-old","version":1,"module":"obi","code_hash":"{h}","status":"retired","params":{p},"ensemble_weight":0.0}}]"#,
        h = real_hash,
        p = valid_obi_params()
    );
    let reg = write_registry(dir.path(), &two);
    let loaded = registry::load_registry(&reg, &signals_src_root()).unwrap();
    assert_eq!(loaded.len(), 1, "retired пропущен, candidate загружен");
    assert_eq!(loaded[0].id, "S-001-obi-asym");
    assert_eq!(loaded[0].status, RegistryStatus::Candidate);
}

#[test]
fn test_invalid_params_rejects_boot() {
    // SG-I-8: params-мусор → Reject (fail-closed на конфигурацию).
    let real_hash = registry::module_code_hash(&signals_src_root(), "obi").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let reg = write_registry(
        dir.path(),
        &entry_json(&real_hash, "candidate", r#"{"garbage": true}"#),
    );
    assert!(matches!(
        registry::load_registry(&reg, &signals_src_root()),
        Err(SignalError::InvalidParams(_))
    ));
}

#[test]
fn test_signal_id_self_consistency() {
    // SG-I-11: инстанцированный сигнал обязан отвечать spec().id == entry.signal_id.
    let real_hash = registry::module_code_hash(&signals_src_root(), "obi").unwrap();
    let json = entry_json(&real_hash, "candidate", &valid_obi_params());
    let entry: RegistryEntry =
        serde_json::from_str(json.trim_start_matches('[').trim_end_matches(']')).unwrap();
    let sig = registry::instantiate(&entry).unwrap();
    assert_eq!(sig.spec().id.as_str(), "S-001-obi-asym");
}
