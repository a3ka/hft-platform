//! RED-спека транспорта (задача §4: два транспорта за трейтом; "TelegramTransport без
//! токена не паникует"). Не бьём реальную сеть в тестах: `TelegramTransport` тестируется
//! через `with_credentials`, который не читает env процесса (детерминизм — тест не должен
//! зависеть от того, стоит ли токен на машине, где гоняются тесты).

use ops::transport::{StdoutTransport, TelegramTransport, Transport};

#[test]
fn stdout_transport_never_errors() {
    let t = StdoutTransport;
    assert!(t.send("[CRITICAL] WD-HB-STALE — test message").is_ok());
}

#[test]
fn stdout_transport_handles_multiline_message() {
    let t = StdoutTransport;
    assert!(t.send("line1\nline2\nline3").is_ok());
}

#[test]
fn telegram_transport_without_credentials_is_not_configured() {
    let t = TelegramTransport::with_credentials(None);
    assert!(!t.is_configured());
}

#[test]
fn telegram_transport_without_credentials_does_not_panic_and_returns_ok() {
    // Анти-плацебо: заглушка "always Err" ловится этим тестом (требуем именно Ok, не просто
    // "не паникует"); заглушка "always Ok без no-op-логики" тоже проходит здесь, но
    // `telegram_transport_with_credentials_is_configured` ниже отличает "сконфигурирован"
    // от "нет" — вместе пара покрывает обе стороны контракта.
    let t = TelegramTransport::with_credentials(None);
    let result = t.send("[CRITICAL] WD-HB-STALE — test message, no token available");
    assert!(result.is_ok(), "no-op transport must not error: {result:?}");
}

#[test]
fn telegram_transport_with_credentials_is_configured() {
    let t = TelegramTransport::with_credentials(Some((
        "fake-token".to_string(),
        "fake-chat-id".to_string(),
    )));
    assert!(t.is_configured());
}

#[test]
fn telegram_transport_from_env_without_vars_set_does_not_panic() {
    // `from_env()` в CI-окружении без TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID — тот же no-op путь.
    // Гарантированно не читаем чужие переменные окружения других тестов процесса (только эти
    // две), поэтому тест безопасен для параллельного запуска.
    if std::env::var("TELEGRAM_BOT_TOKEN").is_ok() || std::env::var("TELEGRAM_CHAT_ID").is_ok() {
        // Токен реально стоит в окружении раннера — не наш случай тестировать здесь молча.
        return;
    }
    let t = TelegramTransport::from_env();
    assert!(!t.is_configured());
    assert!(t.send("smoke").is_ok());
}
