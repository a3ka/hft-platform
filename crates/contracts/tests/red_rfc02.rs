//! RED CT-RFC-02 (sacred, architect-only): SegmentHeader / DataSource / schema_version 2.
//!
//! Анти-плацебо: тесты падают на любой реализации, где эпоху можно «не заметить» —
//! legacy-заголовок обязан быть ВМЕНЁН явно (а не оставлен пустым), а `Event` обязан
//! пережить bump схемы байт-в-байт (журнал бессмертен, CT-I-3).

use contracts::{
    DataSource, Event, EventKind, MdPayload, SegmentHeader, Side, Venue, LEGACY_EPOCH_ID,
    SCHEMA_VERSION, SCHEMA_VERSION_PRE_HEADER,
};

/// CT-I-6: версия схемы 2 — сегменты несут заголовок.
#[test]
fn ct_rfc02_schema_version_is_two() {
    assert_eq!(SCHEMA_VERSION, 2, "CT-RFC-02: bump 1 → 2");
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

/// CT-RFC02-1: вменённый заголовок legacy-сегмента — источник НАЗВАН, а не пуст.
/// Наивная реализация (`Default` / пустая строка / `Vendor`) здесь падает.
#[test]
fn ct_rfc02_legacy_implied_header_names_the_epoch() {
    let h = SegmentHeader::legacy_implied(1_752_000_000_000, 0);
    assert_eq!(h.schema_version, SCHEMA_VERSION_PRE_HEADER);
    assert_eq!(
        h.source,
        DataSource::OwnCapture,
        "боевой сегмент (пишется с 2026-07-10) — НАШ захват; это факт истории репозитория, \
         и он обязан быть зафиксирован явно, а не молчанием"
    );
    assert_eq!(h.epoch_id, LEGACY_EPOCH_ID);
    assert!(
        !h.provenance.is_empty(),
        "provenance пустым быть не может — иначе эпоха неотличима от вендорской"
    );
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
