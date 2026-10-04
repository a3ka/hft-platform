//! RED-спека дедупликации (задача §2: "повторное срабатывание того же условия не шлёт
//! второй раз в пределах окна") + персистентности состояния между запусками cron'а.
//!
//! Анти-плацебо: заглушка "always suppress" ловится тестом `first_fire_is_not_suppressed`;
//! заглушка "always fire" (без окна) ловится `repeat_within_window_is_suppressed`.

use ops::state::WatchdogState;

#[test]
fn first_fire_is_not_suppressed() {
    let mut state = WatchdogState::default();
    assert!(state.should_fire("WD-HB-STALE", 1_000, 30 * 60_000));
}

#[test]
fn repeat_within_window_is_suppressed() {
    let mut state = WatchdogState::default();
    let window = 30 * 60_000;
    assert!(state.should_fire("WD-HB-STALE", 1_000, window));
    // 5 минут спустя, всё ещё внутри 30-минутного окна.
    assert!(!state.should_fire("WD-HB-STALE", 1_000 + 5 * 60_000, window));
}

#[test]
fn repeat_after_window_expires_fires_again() {
    let mut state = WatchdogState::default();
    let window = 30 * 60_000;
    assert!(state.should_fire("WD-HB-STALE", 0, window));
    assert!(state.should_fire("WD-HB-STALE", window + 1, window));
}

#[test]
fn different_keys_do_not_suppress_each_other() {
    // Разные контейнеры/маркеры — независимые дедуп-серии (несут разный target, задача §2
    // "того же условия", не "любого").
    let mut state = WatchdogState::default();
    let window = 30 * 60_000;
    assert!(state.should_fire("WD-CONTAINER-UNHEALTHY:hft-recorder", 0, window));
    assert!(state.should_fire("WD-CONTAINER-UNHEALTHY:hft-gateway-serve", 0, window));
}

#[test]
fn clear_lifts_suppression_immediately() {
    // Условие вернулось к норме, потом сорвалось снова — не должно быть подавлено остатком
    // старого окна.
    let mut state = WatchdogState::default();
    let window = 30 * 60_000;
    assert!(state.should_fire("WD-DISK-LOW", 0, window));
    state.clear("WD-DISK-LOW");
    assert!(state.should_fire("WD-DISK-LOW", 60_000, window));
}

#[test]
fn roundtrip_preserves_dedup_map_and_prev_samples() {
    use ops::watchdog::HeartbeatSample;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("watchdog.state.json");

    let mut state = WatchdogState::default();
    state.should_fire("WD-HB-STALE", 1_000, 30 * 60_000);
    state.prev_check_ms = Some(1_000);
    state.prev_heartbeat = Some(HeartbeatSample {
        ts_wall_ms: 1_000,
        next_seq: 42,
        segment_index: 1,
        events: 42,
        free_bytes: Some(1_000_000),
        min_free_bytes: Some(100),
        writable: Some(true),
    });
    state.save(&path).unwrap();

    let loaded = WatchdogState::load_or_default(&path);
    assert_eq!(loaded.prev_check_ms, Some(1_000));
    assert_eq!(loaded.prev_heartbeat.unwrap().next_seq, 42);
    // Загруженное состояние всё ещё подавляет тот же ключ в пределах окна.
    let mut loaded = loaded;
    assert!(!loaded.should_fire("WD-HB-STALE", 1_100, 30 * 60_000));
}
