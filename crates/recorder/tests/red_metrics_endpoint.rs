//! RED M-09 task 4A — `/metrics` HTTP-СЕРВЕР, socket-путь (sacred, architect-only). `ops.md` §3.
//!
//! `ops::server::http_response` — ЧИСТАЯ трансформация (юнит-RED `crates/ops/tests/red_ops_server.rs`).
//! ЗДЕСЬ пиннится socket-обвязка рекордера: `recorder::metrics_server::serve` реально биндит
//! `TcpListener`, принимает соединение, отдаёт `prometheus_text()` и закрывает — на РЕАЛЬНОМ TCP.
//! loopback (`127.0.0.1`) = «без внешнего доступа» (§3). Это класс «GREEN чистая логика, но wiring не
//! сервит» (тот же урок, что recon-wiring кормил пустую книгу) — юнит http_response его не ловит.
//!
//! Анти-плацебо: против отсутствующего/no-op `serve` соединение не даст 200 с телом → тест падает.
//! Engine-dev: recorder Cargo.toml — добавить tokio features `net` + `io-util` (в [dependencies] и/или
//! [dev-dependencies]) для bind/accept/read/write; это shared-access правило scope-guard (свои deps).

use std::sync::Arc;
use std::time::Duration;

use ops::metrics::{MetricKind, Metrics, METRICS};
use recorder::metrics_server::serve;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn recorder_serves_metrics_over_loopback_tcp() {
    // Известное значение метрики → тело scrape обязано его нести (семантика, не заглушка).
    let metrics = Arc::new(Metrics::new());
    let name = METRICS
        .iter()
        .find(|s| s.kind == MetricKind::Counter && s.labels.is_empty())
        .map(|s| s.name)
        .expect("counter без labels");
    metrics.inc_counter(name, &[], 3);

    // Эфемерный loopback-порт (127.0.0.1:0 → ядро выдаёт свободный) — без внешнего доступа (§3).
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("local_addr");

    // Сервер-таск (accept-loop живёт вечно; в тесте дропается по завершении).
    tokio::spawn(serve(listener, Arc::clone(&metrics)));

    // Клиент: реальный TCP GET /metrics.
    let body = tokio::time::timeout(Duration::from_secs(5), async move {
        let mut stream = TcpStream::connect(addr).await.expect("connect");
        stream
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .expect("write GET");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.expect("read response");
        String::from_utf8_lossy(&buf).into_owned()
    })
    .await
    .expect("сервер /metrics не ответил за 5с — serve не биндит/не отвечает (no-op wiring)");

    assert!(
        body.starts_with("HTTP/1.1 200"),
        "recorder /metrics не вернул 200 по TCP (`{}`) — socket-обвязка не сервит",
        body.lines().next().unwrap_or("")
    );
    assert!(
        body.contains(&format!("{name} 3")),
        "тело /metrics по TCP не несёт `{name} 3` — serve отдаёт не `prometheus_text()` (заглушка)"
    );
    assert!(
        body.contains("book_divergence_bps"),
        "тело /metrics не содержит `book_divergence_bps` — §8-наблюдаемости recon (ради которой task 4) \
         в scrape нет"
    );
}
