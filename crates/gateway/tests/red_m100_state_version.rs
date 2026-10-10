//! RED `M-100` (sacred, architect-only) — **версия ПРОВОДА больше не обесценивает
//! ВЫЧИСЛИТЕЛЬНОЕ состояние** (план SCALE §1 п.3, §15.6 строка 1: «изменился только формат
//! выдачи ⇒ совместимое вычислительное состояние НЕ потеряно»).
//!
//! # Дефект
//!
//! Слепок несёт в заголовке (байты `12..16`) `GATEWAY_SCHEMA_VERSION`, и чтение слепка отвергает
//! его при несовпадении с версией провода (`read_and_validate`; допуск `admission::readiness`
//! сверяет те же байты). Значит любая правка ФОРМЫ кадра — даже не трогающая редьюсер —
//! инвалидирует все слепки и стоит ≈23 мин холодной пересборки (замер `M-70` п.4). `M-100` сам
//! меняет провод (11 → 12) — и без этой развязки заплатил бы ровно эту цену.
//!
//! # Объявленная форма (спека §4.4)
//!
//! ```ignore
//! // crates/gateway/src/lib.rs, модуль checkpoint
//! pub const CALC_STATE_VERSION: u32 = 11;   // версия ФОРМЫ состояния редьюсера в слепке
//! // GATEWAY_SCHEMA_VERSION = 12            // версия ПРОВОДА
//! // заголовок слепка, байты 12..16, пишет и сверяет CALC_STATE_VERSION
//! ```
//! `11` выбрано намеренно: слепки, записанные до `M-100` (байты `12..16` = 11), остаются годными.
//!
//! COMPILE-RED до появления `gateway::checkpoint::CALC_STATE_VERSION` (engine-dev, задача 4).
use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{LiveReducer, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1000,
        bands: vec![0.015],
        window_ms: Some(60_000),
        depth_cadence_ms: Some(1000),
    }
}

fn journal() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tmp");
    let mut j = Journal::open_with(
        dir.path(),
        WriterConfig {
            max_segment_bytes: 1 << 20,
            min_free_bytes: 0,
            source: DataSource::OwnCapture,
            provenance: "m100".to_string(),
            epoch_id: "own-test".to_string(),
        },
    )
    .expect("open");
    for i in 0..50i64 {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(65_000.0),
                size: to_fixed(0.1),
                side: Side::Buy,
                ts_exch_ms: 1_752_000_000_000 + i * 100,
            },
        ))
        .expect("append");
    }
    j.flush().expect("flush");
    dir
}

/// Слепок, записанный прод-путём (`checkpoint::advance`), с подменённой версией в байтах 12..16.
fn ckpt_with_state_version(v: Option<u32>) -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = journal();
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    let p = gateway::checkpoint::ckpt_path_for_pub(ckpt.path(), &sel());
    let mut b = std::fs::read(&p).expect("read ckpt");
    assert!(b.len() > 16, "SETUP НЕ СОСТОЯЛСЯ: слепок короче заголовка");
    if let Some(v) = v {
        b[12..16].copy_from_slice(&v.to_le_bytes());
        std::fs::write(&p, &b).expect("write ckpt");
    }
    (dir, ckpt)
}

/// `resume` берёт ТЁПЛЫЙ путь (слепок принят) ⟺ событий журнала декодировано меньше, чем в
/// журнале (хвост за слепком пуст). Холодный путь читает все 50.
fn warm(dir: &std::path::Path, ckpt: &std::path::Path) -> bool {
    let (_r, stats) =
        LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &sel(), ckpt).expect("resume");
    stats.events_decoded < 50
}

/// **`sv1` — константы разведены:** провод 12, состояние 11.
#[test]
fn sv1_wire_and_state_versions_are_distinct_constants() {
    assert_eq!(
        gateway::GATEWAY_SCHEMA_VERSION,
        12,
        "M-100 меняет провод: 11 → 12"
    );
    assert_eq!(
        gateway::checkpoint::CALC_STATE_VERSION,
        11,
        "M-100 НЕ меняет форму состояния редьюсера — слепки до M-100 обязаны остаться годными"
    );
}

/// **`sv2` — слепок с версией СОСТОЯНИЯ принят, хотя версия ПРОВОДА другая.**
#[test]
fn sv2_checkpoint_with_state_version_is_accepted_under_new_wire_version() {
    let (dir, ckpt) = ckpt_with_state_version(Some(11));
    assert!(
        warm(dir.path(), ckpt.path()),
        "M-100 / план §15.6: слепок с CALC_STATE_VERSION = 11 отвергнут — версия провода (12) \
         продолжает обесценивать вычислительное состояние"
    );
}

/// **`sv3` — слепок с ЧУЖОЙ версией состояния отвергается** (в т.ч. равной версии провода) —
/// развязка не превращается в «принимать всё».
#[test]
fn sv3_foreign_state_version_is_rejected() {
    for v in [10u32, 12] {
        let (dir, ckpt) = ckpt_with_state_version(Some(v));
        assert!(
            !warm(dir.path(), ckpt.path()),
            "M-100: слепок с версией состояния {v} ≠ CALC_STATE_VERSION принят тёплым путём"
        );
    }
}
