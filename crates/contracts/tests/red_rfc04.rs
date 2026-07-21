//! RED CT-RFC-04 (sacred, architect-only): `MdPayload::L2Delta` — сырой `@depth` diff в журнал.
//!
//! Анти-плацебо (обе стороны):
//! 1. **Аддитивность СТРОГО в конец (CT-I-3).** Postcard кодирует дискриминант enum как varint
//!    индекса варианта. L2Delta обязан быть индексом 6 (после MarginRate=5); дискриминанты 0..5
//!    НЕ сдвинуты. Гвоздь — ИСТОРИЧЕСКИЙ байт-блоб события, закодированного ДО существования
//!    L2Delta: он обязан раздекодиться байт-в-байт. Вставка L2Delta не в конец сдвинула бы
//!    дискриминанты → блоб раздекодился бы иначе → тест ПАДАЕТ.
//! 2. **Capture-форма ПОЛНАЯ и БЕЗ ПОТЕРЬ.** Дельта несёт U/u/pu + асимметрию сторон + `size==0`
//!    remove. Если реализация роняет поле (или путает spot/futures continuity) — реконструкция
//!    стакана (absorption/DOM) невозможна, и тест это ловит.
//!
//! `.claude/rules/testing.md` чек-лист: асимметрия (одна сторона пустая), множественность (2+
//! уровня), отсутствие (неупомянутый уровень ≠ удаление), границы (`size==0` remove, пустая
//! дельта, spot `None` vs futures `Some`).

use std::path::Path;

use contracts::{Event, EventKind, Level, MdEvent, MdPayload, Venue};

fn spot_delta() -> MdPayload {
    MdPayload::L2Delta {
        // АСИММЕТРИЯ + МНОЖЕСТВЕННОСТЬ + size==0 remove: bids = [upsert, remove], asks = [].
        bids: vec![
            Level {
                price: 6_500_050_000_000,
                size: 30_000_000,
            },
            Level {
                price: 6_500_040_000_000,
                size: 0, // явный remove от биржи
            },
        ],
        asks: vec![], // пустая сторона = «не менялось», НЕ «очистить»
        first_update_id: 101,
        final_update_id: 103,
        prev_final_update_id: None, // СПОТ — непрерывность по U == prev.u + 1
        ts_exch_ms: 1_752_000_000_499,
    }
}

fn event(payload: MdPayload, venue: Venue) -> Event {
    Event {
        seq: 7,
        ts_mono_ns: 100,
        ts_wall_ms: 1_752_000_000_500,
        kind: EventKind::md(venue, "BTCUSDT", payload),
    }
}

/// Роундтрип L2Delta-события через postcard (тот же конверт, что у всех событий журнала).
#[test]
fn ct_rfc04_l2delta_event_roundtrips_postcard() {
    let ev = event(spot_delta(), Venue::Binance);
    let bytes = postcard::to_stdvec(&ev).expect("serialize");
    let back: Event = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(ev, back, "L2Delta-событие не пережило postcard-роундтрип");
}

/// CT-I §6 / CT-I-3: дискриминанты MdPayload зафиксированы; L2Delta — ПОСЛЕДНИЙ (индекс 6).
/// Первый байт postcard payload'а = varint индекса варианта (для 0..127 — один байт == индекс).
#[test]
fn ct_rfc04_discriminants_frozen_l2delta_is_index_six() {
    let disc = |p: &MdPayload| postcard::to_stdvec(p).expect("ser")[0];

    assert_eq!(
        disc(&MdPayload::Trade {
            price: 1,
            size: 1,
            side: contracts::Side::Buy,
            ts_exch_ms: 1
        }),
        0
    );
    assert_eq!(
        disc(&MdPayload::L2Snapshot {
            bids: vec![],
            asks: vec![],
            ts_exch_ms: 1
        }),
        1
    );
    assert_eq!(
        disc(&MdPayload::Funding {
            rate_e8: 1,
            ts_exch_ms: 1
        }),
        2
    );
    assert_eq!(
        disc(&MdPayload::OpenInterest {
            oi_e8: 1,
            ts_exch_ms: 1
        }),
        3
    );
    assert_eq!(
        disc(&MdPayload::Liquidation {
            price: 1,
            size: 1,
            side: contracts::Side::Buy,
            ts_exch_ms: 1
        }),
        4
    );
    assert_eq!(
        disc(&MdPayload::MarginRate {
            rate_e8: 1,
            ts_exch_ms: 1
        }),
        5
    );
    assert_eq!(
        disc(&spot_delta()),
        6,
        "L2Delta ОБЯЗАН быть индексом 6 (аддитивно в конец); иначе старые журналы misdecode"
    );
}

/// ГВОЗДЬ CT-I-3: событие `L2Snapshot`, закодированное ДО существования L2Delta (фиксированный
/// байт-блоб, вычислен на дереве без L2Delta), обязано раздекодиться НОВЫМ кодом байт-в-байт.
/// Если кто-то сдвинет дискриминанты (вставит L2Delta не в конец) — блоб раздекодится в другой
/// вариант / с ошибкой → FAIL. Журнал бессмертен: старая запись читается навсегда.
#[test]
fn ct_rfc04_historical_l2snapshot_bytes_decode_identically() {
    // Вычислено: cargo run (см. RFC §8) на MdPayload без L2Delta. НЕ перегенерировать —
    // это исторический артефакт, весь смысл в его неизменности.
    const HISTORICAL: &[u8] = &[
        3, 42, 128, 192, 179, 181, 253, 101, 1, 0, 7, 66, 84, 67, 85, 83, 68, 84, 1, 1, 128, 226,
        222, 146, 173, 250, 2, 128, 218, 196, 9, 1, 128, 188, 163, 156, 173, 250, 2, 128, 180, 137,
        19, 246, 193, 179, 181, 253, 101,
    ];
    let decoded: Event = postcard::from_bytes(HISTORICAL).expect("исторический блоб не декодится");
    let expected = Event {
        seq: 3,
        ts_mono_ns: 42,
        ts_wall_ms: 1_752_000_000_000,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![Level {
                    price: 6_500_050_000_000,
                    size: 10_000_000,
                }],
                asks: vec![Level {
                    price: 6_500_060_000_000,
                    size: 20_000_000,
                }],
                ts_exch_ms: 1_752_000_000_123,
            },
        ),
    };
    assert_eq!(
        decoded, expected,
        "исторический L2Snapshot раздекодился иначе → дискриминанты сдвинуты (CT-I-3 нарушен)"
    );
}

/// Capture БЕЗ ПОТЕРЬ: каждое поле дельты переживает роундтрип, включая `size==0` remove,
/// пустую сторону (отсутствие ≠ удаление) и обе границы update-id. Реконструкция стакана
/// невозможна, если хоть одно поле теряется.
#[test]
fn ct_rfc04_capture_is_lossless() {
    let ev = event(spot_delta(), Venue::Binance);
    let back: Event = postcard::from_bytes(&postcard::to_stdvec(&ev).unwrap()).unwrap();
    let EventKind::Md(MdEvent {
        payload:
            MdPayload::L2Delta {
                bids,
                asks,
                first_update_id,
                final_update_id,
                prev_final_update_id,
                ts_exch_ms,
            },
        ..
    }) = back.kind
    else {
        panic!("ожидался L2Delta");
    };
    assert_eq!(bids.len(), 2, "оба бид-уровня сохранены (множественность)");
    assert_eq!(bids[1].size, 0, "size==0 remove сохранён как явный маркер");
    assert!(
        asks.is_empty(),
        "пустая сторона осталась пустой (отсутствие)"
    );
    assert_eq!((first_update_id, final_update_id), (101, 103));
    assert_eq!(prev_final_update_id, None, "spot: prev_final == None");
    assert_eq!(ts_exch_ms, 1_752_000_000_499);
}

/// Futures `pu` (`prev_final_update_id = Some`) переживает роундтрип и ОТЛИЧИМ от spot (`None`).
/// Путаница spot/futures continuity ломает gap-детекцию перп-книги (урок TD-014).
#[test]
fn ct_rfc04_futures_prev_final_is_carried_and_distinct() {
    let fut = MdPayload::L2Delta {
        bids: vec![Level {
            price: 6_500_050_000_000,
            size: 45_000_000,
        }],
        asks: vec![],
        first_update_id: 501,
        final_update_id: 510,
        prev_final_update_id: Some(500), // futures pu чейнится на предыдущий final
        ts_exch_ms: 1_752_000_000_599,
    };
    let ev = event(fut, Venue::BinanceFutures);
    let back: Event = postcard::from_bytes(&postcard::to_stdvec(&ev).unwrap()).unwrap();
    let EventKind::Md(MdEvent {
        payload: MdPayload::L2Delta {
            prev_final_update_id,
            ..
        },
        ..
    }) = back.kind
    else {
        panic!("ожидался L2Delta");
    };
    assert_eq!(
        prev_final_update_id,
        Some(500),
        "futures pu обязан сохраниться (≠ spot None)"
    );
}

/// Фикстуры valid/ десериализуются (CT-I-5 — общий контракт с research-тулингом).
#[test]
fn ct_rfc04_valid_fixtures_deserialize() {
    for name in ["event-l2delta-spot.json", "event-l2delta-futures.json"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid")
            .join(name);
        let json = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {name}"));
        let ev: Event = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            matches!(
                ev.kind,
                EventKind::Md(MdEvent {
                    payload: MdPayload::L2Delta { .. },
                    ..
                })
            ),
            "{name} должен быть L2Delta"
        );
    }
}

/// Фикстура invalid/ (нет `final_update_id`) ОТВЕРГАЕТСЯ — форма total, неполная дельта ≠ валид.
#[test]
fn ct_rfc04_invalid_fixture_is_rejected() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/invalid/event-l2delta-missing-final-id.json");
    let json = std::fs::read_to_string(&path).expect("read invalid fixture");
    let parsed: Result<Event, _> = serde_json::from_str(&json);
    assert!(
        parsed.is_err(),
        "L2Delta без final_update_id обязан быть отвергнут (fail-closed)"
    );
}
