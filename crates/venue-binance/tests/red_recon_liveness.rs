//! RED liveness OPS-I-9 (sacred, architect-only) — recon-фетчер НЕ hot-loop'ит на 418/429.
//!
//! Прямой урок TD-013: hot-loop REST-ресинка = 133×418 за 25с → IP-бан Binance. `ops::budget`
//! (OPS-I-9) тестит ПОЛИТИКУ `next_delay`, но не то, что `ReconFetcher::run` реально её `.await`ит
//! (ровно риск M-06: RED тестил Backoff, reviewer вручную проверял I/O-await). «STRUCTURAL
//! anti-hot-loop» в комментарии — НЕ гарантия; этот тест ПРОГОНЯЕТ цикл против живого 418-мока.
//!
//! Метод — count-based (детерминированно, НЕ wall-clock margin как TD-023): корректный фетчер при
//! 418 берёт `next_delay` = cooldown (десятки-сотни секунд) → за короткое окно делает ~1 запрос;
//! hot-loop (удалён `sleep(delay)` / обойдён budget) делает СОТНИ. Разница на 2 порядка — не флак.
//!
//! Анти-плацебо: убери `sleep(delay)` из `ReconFetcher::run` → число запросов взрывается → падёт.
//! Шов `ReconConfig::with_base_url` (venue-dev) — RED требует его: без него мок не подставить.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ops::metrics::Metrics;
use tokio::sync::mpsc;
use venue_binance::recon::{ReconConfig, ReconFetcher};

/// Локальный мок: отвечает 418 на любой запрос, считает подключения. Живёт до `stop`.
fn spawn_mock_418() -> (String, Arc<AtomicUsize>, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("addr");
    listener.set_nonblocking(true).expect("nonblocking");
    let count = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let (c, s) = (Arc::clone(&count), Arc::clone(&stop));
    std::thread::spawn(move || {
        while !s.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut sock, _)) => {
                    c.fetch_add(1, Ordering::SeqCst);
                    let mut buf = [0u8; 1024];
                    let _ = sock.read(&mut buf);
                    // 418 с Retry-After — фетчер обязан honor'ить cooldown, не долбить.
                    let resp = "HTTP/1.1 418 I'm a teapot\r\nRetry-After: 120\r\n\
                                Content-Length: 0\r\nConnection: close\r\n\r\n";
                    let _ = sock.write_all(resp.as_bytes());
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(_) => break,
            }
        }
    });
    (format!("http://{addr}"), count, stop)
}

/// OPS-I-9 liveness: поток 418 НЕ вводит фетчер в hot-loop — за окно ~1 запрос, не сотни.
#[tokio::test]
async fn ops_i_9_recon_fetcher_does_not_hot_loop_on_418() {
    let (base_url, count, stop) = spawn_mock_418();

    let metrics = Arc::new(Metrics::new());
    // Шов `with_base_url` (venue-dev) — направляет фетчер на мок вместо api.binance.com.
    let cfg = ReconConfig::new("BTCUSDT").with_base_url(base_url);
    let client = reqwest::Client::new();
    let mut fetcher = ReconFetcher::new(client, cfg, metrics);

    let (tx, _rx) = mpsc::channel(16);
    let handle = tokio::spawn(async move { fetcher.run(tx).await });

    // Короткое окно: корректный фетчер после первого 418 уходит на cooldown (Retry-After 120с).
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.abort();
    stop.store(true, Ordering::SeqCst);

    let n = count.load(Ordering::SeqCst);
    assert!(
        n <= 3,
        "ReconFetcher сделал {n} запросов за 500мс против 418-мока (Retry-After 120с) — это hot-loop \
         (TD-013: 133×418/25с = IP-бан). Корректный фетчер обязан honor'ить cooldown budget'а и \
         сделать ~1 запрос. Проверь, что `sleep(budget.next_delay(...))` реально в цикле run()."
    );
    assert!(
        n >= 1,
        "фетчер не сделал НИ одного запроса — мок/шов не сработал, тест бессмыслен"
    );
}
