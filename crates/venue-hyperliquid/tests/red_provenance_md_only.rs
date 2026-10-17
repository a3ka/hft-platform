//! RED M-41 task #1 (sacred, architect-only): провенанс времени + детерминизм +
//! MD-only-канарейка (carve-out `.claude/rules/gates.md` §5).
//!
//! 1) `ts_exch_ms` — время БИРЖИ из сообщения, не локальные часы (иначе ломается DET-I-1
//!    при реплее и возрастной фильтр ретеншена).
//! 2) Дубликаты/сообщения «из прошлого» адаптер НЕ фильтрует и НЕ переупорядочивает:
//!    журнал — сырая правда; порядок/gap-детекция — забота книги/реплея выше
//!    (характеризация осознанного дизайна, помечено явно).
//! 3) MD-only: в src нет order-egress (submit/cancel/подпись) — условие carve-out'а,
//!    по которому M-41 идёт без risk-critic. Канарейка ЛОМАЕТСЯ, как только в крейте
//!    появится торговый путь → сигнал reviewer'у, что carve-out больше не применим.

use contracts::{EventKind, MdPayload};
use venue_hyperliquid::parse_message;

const MSG: &str = r#"{"channel":"trades","data":[{"coin":"BTC","side":"A","px":"118250.0","sz":"0.001","time":1753000000100}]}"#;

/// Детерминизм + провенанс: два вызова в разные моменты wall-clock дают БИТ-идентичные
/// события, ts равен литералу из сообщения. Если бы парсер подмешивал SystemTime — оба
/// ассерта сломались бы.
#[test]
fn ts_from_message_not_local_clock_deterministic() {
    let first = parse_message(MSG);
    let second = parse_message(MSG);
    assert_eq!(
        first, second,
        "одинаковый вход → бит-идентичный выход (DET-I-1 предпосылка)"
    );
    let EventKind::Md(md) = &first[0] else {
        panic!("ожидался Md")
    };
    let MdPayload::Trade { ts_exch_ms, .. } = &md.payload else {
        panic!("ожидался Trade")
    };
    assert_eq!(
        *ts_exch_ms, 1_753_000_000_100,
        "ts_exch_ms == time из сообщения биржи, точно"
    );
}

/// Характеризация: дубликат и «сообщение из прошлого» проходят как есть (по одному событию
/// на вызов) — адаптер stateless, дедуп/ordering не его зона (см. заголовок файла).
#[test]
fn duplicates_and_stale_messages_pass_through() {
    let newer = r#"{"channel":"trades","data":[{"coin":"BTC","side":"A","px":"118251.0","sz":"0.001","time":1753000000200}]}"#;
    let older = r#"{"channel":"trades","data":[{"coin":"BTC","side":"A","px":"118250.0","sz":"0.001","time":1753000000100}]}"#;
    assert_eq!(parse_message(newer).len(), 1);
    assert_eq!(
        parse_message(older).len(),
        1,
        "прошлое-по-ts сообщение НЕ дропается адаптером (сырая правда в журнал)"
    );
    assert_eq!(
        parse_message(older),
        parse_message(older),
        "дубликат даёт идентичное событие (пассивная передача, не дедуп)"
    );
}

/// parse_message эмитит ТОЛЬКО Md-события (Sys(ConnUp/ConnDown) — зона run/супервизора,
/// не парсера): парсер не притворяется источником системных фактов.
#[test]
fn parser_emits_only_md_events() {
    let l2 = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[[{"px":"118250.0","sz":"1.5","n":1}],[{"px":"118250.5","sz":"0.5","n":1}]]}}"#;
    for msg in [MSG, l2] {
        for e in parse_message(msg) {
            assert!(
                matches!(e, EventKind::Md(_)),
                "парсер эмитит только Md, получено: {e:?}"
            );
        }
    }
}

/// MD-only канарейка (углубление carve-out'а gates.md §5): src не содержит order-egress.
/// Проверяем сырой текст src — появление торговых токенов означает, что M-41-паритет
/// «без risk-critic» БОЛЬШЕ НЕ ДЕЙСТВУЕТ (RISK-BLOCK применяется полностью).
#[test]
fn md_only_no_order_egress_canary() {
    let src = include_str!("../src/lib.rs").to_lowercase();
    for token in [
        "order",
        "cancel",
        "signature",
        "sign(",
        "wallet",
        "private_key",
        "privatekey",
        "/exchange", // HL REST /exchange — торговый эндпоинт (в отличие от read-only /info)
    ] {
        assert!(
            !src.contains(token),
            "src/lib.rs содержит '{token}' — похоже на order-egress: MD-only carve-out \
             недействителен, требуется risk-critic (gates.md §5)"
        );
    }
    assert!(
        src.contains("wss://api.hyperliquid.xyz/ws"),
        "точка наблюдения — прод-URL HL WS; смена эндпоинта = осознанное решение architect'а"
    );
}
