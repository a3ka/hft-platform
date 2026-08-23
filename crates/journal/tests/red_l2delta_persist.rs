//! RED M-18 / CT-RFC-04 (sacred, architect-only): sacred live-path (recorder → journal)
//! персистит `MdPayload::L2Delta` БЕЗ ПОТЕРЬ через реальный write→read_all (postcard + crc32).
//!
//! Журнал generic по `EventKind`, но L2Delta — новый вариант И самый объёмный MD-поток
//! (сырые diff'ы), поэтому его прохождение через боевой путь записи фиксируется явно:
//! recorder пишет ровно то, что эмитят venue-адаптеры; DET-I-1 exact-replay обязан вернуть
//! идентичное событие. Анти-плацебо: read_all STRICT — искажение байта/CRC → Err (тут была бы
//! паника на unwrap), схлопывание пустой стороны/потеря pu → assert_eq падает.

use contracts::{DataSource, Event, EventKind, Level, MdEvent, MdPayload, Venue};
use journal::{Journal, WriterConfig};

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test recorder v0 (git:deadbeef)".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn l2delta(prev_final: Option<u64>) -> EventKind {
    EventKind::md(
        if prev_final.is_some() {
            Venue::BinanceFutures
        } else {
            Venue::Binance
        },
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: vec![
                Level {
                    price: 6_500_050_000_000,
                    size: 30_000_000,
                },
                Level {
                    price: 6_500_040_000_000,
                    size: 0, // remove
                },
            ],
            asks: vec![], // асимметрия — пустая сторона обязана дожить
            first_update_id: 101,
            final_update_id: 103,
            prev_final_update_id: prev_final,
            ts_exch_ms: 1_752_000_000_499,
        },
    )
}

#[test]
fn l2delta_survives_journal_write_read_exact() {
    let dir = tempfile::tempdir().expect("tempdir");
    let written = [l2delta(None), l2delta(Some(500))];
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for k in &written {
            j.append(k.clone()).expect("append");
        }
        j.flush().expect("flush");
    }

    let events: Vec<Event> = journal::read_all(dir.path()).expect("read_all strict");
    assert_eq!(events.len(), 2, "оба L2Delta-события прочитаны");

    // Первое: spot (prev_final None); второе: futures (Some(500)) — оба точны.
    for (ev, expected_kind) in events.iter().zip(written.iter()) {
        assert_eq!(&ev.kind, expected_kind, "L2Delta исказился на sacred-пути");
    }

    // Явно распакуем, чтобы assert бил по КАЖДОМУ полю (не только PartialEq целиком).
    let EventKind::Md(MdEvent {
        payload:
            MdPayload::L2Delta {
                asks,
                bids,
                prev_final_update_id,
                ..
            },
        ..
    }) = &events[0].kind
    else {
        panic!("ожидался L2Delta");
    };
    assert!(asks.is_empty(), "пустая сторона дожила (не схлопнута)");
    assert_eq!(bids[1].size, 0, "size==0 remove дожил");
    assert_eq!(*prev_final_update_id, None, "spot prev_final None");
}
