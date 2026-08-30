//! CT-RFC-05 (sacred, architect-only) — `MdPayload::MarginInventory` аддитивно (дискриминант 7).
//! `docs/05-contract-layer.md` §4/§6, `05` CT-I-3 (старые сегменты читаются байт-в-байт).
//!
//! MI-I-1: (a) новый вариант postcard+serde_json roundtrip; (b) pre-RFC05 байты старого варианта
//! декодятся бит-в-бит (доказ. аддитивности — вставка в СЕРЕДИНУ сдвинула бы дискриминанты 0..6);
//! (c) `SCHEMA_VERSION == 4`.
//! Анти-плацебо: если `MarginInventory` вставлен НЕ в конец enum'а → `FUNDING_PRECHANGE` (дискр.2)
//! декодится неверно → (b) FAIL.

use contracts::{Event, EventKind, MdPayload, Venue, SCHEMA_VERSION};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Pre-change postcard-байты Funding (дискриминант 2), сгенерированы ДО любых добавлений
/// (переиспользованы из `ct_rfc01.rs` — proven pre-change). Обязаны декодиться под schema-4.
const FUNDING_PRECHANGE: &str = "032c84a0abfef96201010342544302f2c0019aa5abfef962";

// ── MI-I-1a: MarginInventory roundtrip (postcard + serde_json бит-идентичны) ─────────────────────
#[test]
fn mi_i_1_margin_inventory_roundtrip() {
    let e = Event {
        seq: 8,
        ts_mono_ns: 100,
        ts_wall_ms: 1_784_991_235_000,
        kind: EventKind::md(
            Venue::BinanceFutures,
            "USDT",
            MdPayload::MarginInventory {
                available_e8: 1_993_259_228_568_050, // 19_932_592.2856805 ×1e8
                ts_exch_ms: 1_784_991_235_000,
            },
        ),
    };
    let pc: Event = postcard::from_bytes(&postcard::to_stdvec(&e).unwrap()).unwrap();
    assert_eq!(e, pc, "postcard roundtrip MarginInventory");
    let js: Event = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
    assert_eq!(e, js, "serde_json roundtrip MarginInventory");
}

// ── MI-I-1b: старый вариант (pre-RFC05) декодится бит-в-бит — аддитивность CT-I-3 ────────────────
#[test]
fn mi_i_1_pre_rfc05_funding_still_decodes() {
    let funding: Event = postcard::from_bytes(&unhex(FUNDING_PRECHANGE))
        .expect("pre-RFC05 Funding обязан декодиться под schema-4 (аддитивность)");
    // Дискриминант Funding (2) НЕ сдвинут добавлением MarginInventory (7) → те же поля.
    match funding.kind {
        EventKind::Md(md) => match md.payload {
            MdPayload::Funding {
                rate_e8,
                ts_exch_ms,
            } => {
                assert_eq!(rate_e8, 12345, "rate не съехал (дискриминанты не сдвинуты)");
                assert_eq!(ts_exch_ms, 1_700_000_000_333);
            }
            other => panic!(
                "ожидался Funding, получен {other:?} — дискриминант сдвинут (вставка не в конец)"
            ),
        },
        other => panic!("ожидался Md, получен {other:?}"),
    }
    assert_eq!(funding.seq, 3);
}

// ── MI-I-1c: SCHEMA_VERSION поднят до 4 (новая эпоха, изоляция как L2Delta/TD-031) ───────────────
#[test]
fn mi_i_1_schema_version_is_4() {
    assert_eq!(SCHEMA_VERSION, 4, "CT-RFC-05 ⇒ SCHEMA_VERSION == 4");
}
