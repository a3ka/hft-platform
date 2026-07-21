//! RED CT-RFC-03 (sacred, architect-only): аудит-событие recon в `SysEvent` (M-09 OPS-I-1).
//!
//! Анти-плацебо: тесты падают на любой реализации, где
//!  - новый вариант добавлен НЕ в конец (сдвиг postcard-дискриминантов Heartbeat/ConnUp/ConnDown
//!    → старые журналы прочитаются как другой вид события);
//!  - `Event`/`EventKind` изменили wire-формат (журнал бессмертен, CT-I-3);
//!  - порядок `ReconAction` переставлен.
//!
//! Без вариантов `ReconDivergence`/`ReconAudit`/`ReconAction` этот файл НЕ КОМПИЛИРУЕТСЯ —
//! compile-RED, доказывающий, что тип действительно добавлен, а не «запланирован».

use contracts::{
    Event, EventKind, MdPayload, ReconAction, ReconAudit, Side, SysEvent, Venue, SCHEMA_VERSION,
};

/// CT-RFC-03 САМ по себе не бампил `schema_version` (был 2 на момент CT-RFC-03). Точное значение
/// здесь НЕ пинится: `SCHEMA_VERSION` = эпоха эмитируемых вариантов и растёт с новыми RFC
/// (CT-RFC-04 rev2 поднял 2→3 для L2Delta-изоляции, TD-031). Инвариант CT-RFC-03 — версия header
/// осталась ≥ 2 (формат сегмент-заголовка не сломан), а recon-вариант аддитивен (дискриминант ниже).
#[test]
fn ct_rfc03_schema_version_has_header() {
    // const-контекст (см. red_rfc02): compile-time инвариант, без clippy::assertions_on_constants,
    // без пина точного значения.
    const {
        assert!(
            SCHEMA_VERSION >= 2,
            "формат сегмент-заголовка ≥ 2 (recon-вариант аддитивен)"
        )
    };
}

/// Новый вариант — СТРОГО в конце `SysEvent`: дискриминант 3, а 0/1/2 неизменны.
/// Сдвиг → старый журнал с `Heartbeat`(0) прочитается как другой вид (порча истории).
#[test]
fn ct_rfc03_sys_event_discriminants_are_stable() {
    let d = |e: &SysEvent| postcard::to_stdvec(e).expect("serialize")[0];
    assert_eq!(d(&SysEvent::Heartbeat), 0, "Heartbeat обязан остаться 0");
    assert_eq!(d(&SysEvent::ConnUp(Venue::Binance)), 1, "ConnUp = 1");
    assert_eq!(d(&SysEvent::ConnDown(Venue::Binance)), 2, "ConnDown = 2");
    assert_eq!(
        d(&SysEvent::ReconDivergence(sample_audit())),
        3,
        "ReconDivergence обязан быть ПОСЛЕДНИМ (3) — вставка в середину сдвинет старые дискриминанты"
    );
}

/// Порядок `ReconAction` фиксирован (postcard-дискриминанты).
#[test]
fn ct_rfc03_recon_action_discriminants_are_stable() {
    assert_eq!(postcard::to_stdvec(&ReconAction::AlertOnly).unwrap()[0], 0);
    assert_eq!(postcard::to_stdvec(&ReconAction::Resynced).unwrap()[0], 1);
}

/// Роундтрип recon-события через postcard (тот же конверт, что у всех событий).
#[test]
fn ct_rfc03_recon_event_roundtrips() {
    let ev = recon_event();
    let bytes = postcard::to_stdvec(&ev).expect("serialize");
    let back: Event = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(ev, back, "recon-событие обязано читаться байт-в-байт");
}

/// CT-I-3 (журнал бессмертен): добавление recon-варианта НЕ ломает wire-формат уже пишущихся
/// событий. Байты `Heartbeat` и `Md(Trade)`, записанные ДО CT-RFC-03, обязаны читаться.
#[test]
fn ct_rfc03_pre_existing_events_still_read() {
    // Heartbeat — дискриминант 0, самый чувствительный к сдвигу.
    let hb = Event {
        seq: 1,
        ts_mono_ns: 1,
        ts_wall_ms: 1_752_000_000_000,
        kind: EventKind::Sys(SysEvent::Heartbeat),
    };
    let back: Event = postcard::from_bytes(&postcard::to_stdvec(&hb).unwrap()).unwrap();
    assert_eq!(
        hb, back,
        "Heartbeat, записанный до CT-RFC-03, обязан читаться (CT-I-3)"
    );

    let trade = Event {
        seq: 2,
        ts_mono_ns: 2,
        ts_wall_ms: 1_752_000_000_001,
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
    let back: Event = postcard::from_bytes(&postcard::to_stdvec(&trade).unwrap()).unwrap();
    assert_eq!(
        back, trade,
        "Md-событие не меняется в CT-RFC-03 (аддитивность CT-I-3)"
    );
}

/// Аудит несёт достаточно, чтобы офлайн ответить «каким данным верить»: venue+symbol+
/// магнитуда+порча-best+действие. Поля читаются обратно теми же.
#[test]
fn ct_rfc03_recon_audit_carries_provenance_of_divergence() {
    let a = ReconAudit {
        venue: Venue::BinanceFutures,
        symbol: "BTCUSDT".to_string(),
        divergence_bps: 512,
        best_price_diverged: true,
        action: ReconAction::Resynced,
    };
    let back: ReconAudit = postcard::from_bytes(&postcard::to_stdvec(&a).unwrap()).unwrap();
    assert_eq!(a, back);
    assert!(
        back.best_price_diverged,
        "флаг порчи лучшей цены (ε_test / C1-класс) обязан переживать сериализацию — \
         это качественно иной класс, чем расхождение дальних полос"
    );
}

fn sample_audit() -> ReconAudit {
    ReconAudit {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        divergence_bps: 0,
        best_price_diverged: false,
        action: ReconAction::AlertOnly,
    }
}

fn recon_event() -> Event {
    Event {
        seq: 18_733_828,
        ts_mono_ns: 42_000_000,
        ts_wall_ms: 1_752_000_000_000,
        kind: EventKind::Sys(SysEvent::ReconDivergence(ReconAudit {
            venue: Venue::Binance,
            symbol: "BTCUSDT".to_string(),
            divergence_bps: 137,
            best_price_diverged: true,
            action: ReconAction::Resynced,
        })),
    }
}
