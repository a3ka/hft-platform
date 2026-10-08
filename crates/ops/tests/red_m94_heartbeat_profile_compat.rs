//! Сторож `M-94` (sacred, architect-only) — **поле `calc_profile` в сердцебиении выдачи не
//! ломает читателя `ops-watchdog`** (`П-032` п.4 (а); спека `milestones/M-94-calc-profile.md`
//! §3.6, оракул `o1`).
//!
//! Сердцебиение `gateway-serve` получает аддитивный объект `"calc_profile": {"version",
//! "sha256"}` при `schema = 1`. Читатель — `read_serving_heartbeat` бинаря `ops-watchdog`
//! (`serde_json::from_str::<ServingHeartbeatSample>`). Если на тип однажды повесят
//! `deny_unknown_fields`, сторож тишины выдачи увидит `MISSING` на каждом такте — тревога о том,
//! чего нет, при живой выдаче. Зелёный ДО и ПОСЛЕ поставки; краснеет только на поломке читателя.
//!
//! Предел: судится тип, а не функция бинаря (она приватна в `src/bin/ops-watchdog.rs`); форма
//! разбора — та же строка `serde_json::from_str::<ServingHeartbeatSample>`.

use ops::watchdog::ServingHeartbeatSample;

#[test]
fn o1_heartbeat_with_calc_profile_parses_for_the_watchdog() {
    let body = r#"{"schema":1,"ts_wall_ms":1784116800000,"attempts":3,"successes":2,
        "refusals_supported":1,"refusals_unsupported":0,"journal_payload_bytes_read":42,
        "slots_in_flight":0,
        "freshness":{"source_ms":1,"projection_ms":2,"published_ms":3,"snapshot_ms":4},
        "calc_profile":{"version":1,"sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}"#;
    let s: ServingHeartbeatSample = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!(
            "I-4 (а): читатель ops-watchdog не разбирает сердцебиение с полем calc_profile ({e}) — \
             сторож тишины выдачи увидит MISSING при живой выдаче"
        )
    });
    // Анти-плацебо: разбор не «пустой» — значения дошли.
    assert_eq!(
        (s.attempts, s.successes, s.journal_payload_bytes_read),
        (3, 2, 42)
    );
}
