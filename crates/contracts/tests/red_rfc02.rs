//! RED CT-RFC-02 (sacred, architect-only): SegmentHeader / DataSource / schema_version 2.
//!
//! Анти-плацебо: тесты падают на любой реализации, где эпоху можно «не заметить» —
//! legacy-заголовок обязан быть ВМЕНЁН явно (а не оставлен пустым), а `Event` обязан
//! пережить bump схемы байт-в-байт (журнал бессмертен, CT-I-3).

use contracts::{
    DataSource, Event, EventKind, LegacyManifest, LegacySegmentDecl, MdPayload, SegmentHeader,
    Side, Venue, LEGACY_EPOCH_ID, SCHEMA_VERSION, SCHEMA_VERSION_PRE_HEADER,
};

/// CT-I-6: schema ≥ 2 — сегменты несут заголовок. Точное значение НЕ пинится здесь (оно
/// растёт с каждым новым эмитируемым вариантом — CT-RFC-04 rev2 поднял 2→3, TD-031); инвариант
/// CT-RFC-02 — «версия header-формата ≥ 2», а legacy (без заголовка) = 1.
#[test]
fn ct_rfc02_schema_version_has_header() {
    assert!(
        SCHEMA_VERSION >= 2,
        "CT-RFC-02: сегменты с заголовком имеют schema ≥ 2 (получено {SCHEMA_VERSION})"
    );
    assert_eq!(SCHEMA_VERSION_PRE_HEADER, 1, "legacy-сегменты = schema 1");
}

/// Роундтрип заголовка через postcard (тот же конверт, что у событий).
#[test]
fn ct_rfc02_segment_header_roundtrips() {
    let h = SegmentHeader {
        schema_version: SCHEMA_VERSION,
        source: DataSource::Vendor,
        provenance: "tardis.dev binance-spot L2 2024-01..2024-06, лицензия X, выгрузка 2026-07-20"
            .to_string(),
        epoch_id: "tardis-binance-spot-2024".to_string(),
        created_wall_ms: 1_752_000_000_000,
        first_seq: 42,
    };
    let bytes = postcard::to_stdvec(&h).expect("serialize");
    let back: SegmentHeader = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(h, back);
}

/// CT-RFC02-1 (rev 2, после C-005 C2): заголовок legacy-сегмента строится ИЗ ЯВНОЙ
/// ДЕКЛАРАЦИИ манифеста, а НЕ вменяется по правилу «не разобрался → значит наш».
/// Происхождение берётся из того, что оператор записал, — включая случай, когда
/// задекларирован ВЕНДОРСКИЙ безголовый сегмент (он обязан остаться вендорским).
#[test]
fn ct_rfc02_legacy_header_comes_from_explicit_declaration() {
    let own = LegacySegmentDecl {
        file_name: "segment-00000000.jrnl".to_string(),
        fingerprint_sha256: "ab".repeat(32),
        size_bytes_at_decl: 8_877_245_289,
        source: DataSource::OwnCapture,
        provenance: "pre-RFC02 recorder capture, VPS, 2026-07-10..".to_string(),
        epoch_id: LEGACY_EPOCH_ID.to_string(),
    };
    let h = SegmentHeader::from_legacy_decl(&own, 1_752_000_000_000, 0);
    assert_eq!(h.schema_version, SCHEMA_VERSION_PRE_HEADER);
    assert_eq!(h.source, DataSource::OwnCapture);
    assert_eq!(h.epoch_id, LEGACY_EPOCH_ID);
    assert!(!h.provenance.is_empty());

    // Ключевое: декларация — источник правды. Безголовый ВЕНДОРСКИЙ дамп не превращается
    // в наш захват (прежнее fail-open правило делало ровно это).
    let vendor = LegacySegmentDecl {
        file_name: "vendor-dump.jrnl".to_string(),
        source: DataSource::Vendor,
        epoch_id: "tardis-2024".to_string(),
        ..own.clone()
    };
    let hv = SegmentHeader::from_legacy_decl(&vendor, 1_752_000_000_000, 0);
    assert_eq!(
        hv.source,
        DataSource::Vendor,
        "задекларированный вендорский сегмент обязан остаться Vendor — молчаливая приписка \
         OwnCapture = обучение альфы на чужой реальности"
    );
    assert_eq!(hv.epoch_id, "tardis-2024");
}

/// Манифест ищет декларацию по имени файла; незадекларированный файл → None
/// (потребитель обязан на этом упасть, а не «вменить своё» — CT-RFC02-1 rev 2).
#[test]
fn ct_rfc02_manifest_lookup_is_explicit() {
    let m = LegacyManifest {
        declarations: vec![LegacySegmentDecl {
            file_name: "segment-00000000.jrnl".to_string(),
            fingerprint_sha256: "cd".repeat(32),
            size_bytes_at_decl: 10,
            source: DataSource::OwnCapture,
            provenance: "x".to_string(),
            epoch_id: LEGACY_EPOCH_ID.to_string(),
        }],
    };
    assert!(m.find("segment-00000000.jrnl").is_some());
    assert!(
        m.find("segment-99999999.jrnl").is_none(),
        "незадекларированный сегмент НЕ имеет происхождения — читатель обязан вернуть Err"
    );
}

/// Магия сегмента — стабильная константа: по ней читатель отличает «новый формат» от
/// «безголового старого», а не гадает по успеху парсинга.
#[test]
fn ct_rfc02_segment_magic_is_stable() {
    assert_eq!(&contracts::SEGMENT_MAGIC, b"HFTJRN02");
    assert_eq!(contracts::LEGACY_FINGERPRINT_BYTES, 1024 * 1024);
}

/// CT-I-3 (журнал бессмертен): bump схемы НЕ ломает wire-формат `Event`.
/// Байты, записанные ДО RFC-02, обязаны читаться новым кодом.
#[test]
fn ct_rfc02_event_wire_format_unchanged() {
    let ev = Event {
        seq: 7,
        ts_mono_ns: 123,
        ts_wall_ms: 1_752_000_000_000,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: contracts::to_fixed(65_000.5),
                size: contracts::to_fixed(0.1),
                side: Side::Buy,
                ts_exch_ms: 1_752_000_000_123,
            },
        ),
    };
    // Байты, зафиксированные ДО CT-RFC-02 (schema 1) — контрольная запись.
    let pre_rfc02_bytes: Vec<u8> = postcard::to_stdvec(&ev).expect("serialize");
    let back: Event = postcard::from_bytes(&pre_rfc02_bytes).expect("старый Event обязан читаться");
    assert_eq!(ev, back, "Event НЕ меняется в RFC-02 (аддитивность CT-I-3)");
}

/// Порядок вариантов `DataSource` фиксирован (postcard-дискриминанты): расширение —
/// строго в конец, иначе старые сегменты прочитаются как ЧУЖОЙ источник.
#[test]
fn ct_rfc02_data_source_discriminants_are_stable() {
    for (src, expect) in [
        (DataSource::OwnCapture, 0u8),
        (DataSource::Vendor, 1),
        (DataSource::Synthetic, 2),
    ] {
        let b = postcard::to_stdvec(&src).expect("serialize");
        assert_eq!(
            b[0], expect,
            "дискриминант {src:?} обязан остаться {expect} — сдвиг превратит наш захват \
             в вендорские данные при чтении старых сегментов"
        );
    }
}
