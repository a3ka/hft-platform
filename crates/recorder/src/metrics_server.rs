//! M-09 task 4A — `/metrics` scrape-эндпоинт на loopback. `ops.md` §3.
//!
//! Socket-обвязка рекордера: ЧИСТАЯ трансформация `request-line → response` живёт в
//! `ops::server::http_response` (юнит-RED `crates/ops/tests/red_ops_server.rs`), здесь
//! только I/O — bind+accept+read+write. Loopback (`127.0.0.1`) по умолчанию —
//! «без внешнего доступа» (§3, без публикации наружу).
//!
//! Разделение слоёв (architect, critic N1):
//!  - `ops` остаётся БЕЗ `tokio` (только `contracts`+`book`) — трансформация детерминирована,
//!    юнит-тестируется без scheduler'а;
//!  - `recorder` владеет `Arc<Metrics>` (M-09 P2.5) и socket-loop'ом — `Arc`-клон на соединение,
//!    scrape читает атомики по запросу (`OPS-I-7`: экспорт не в горячем пути).
//!
//! **Shutdown:** `serve` принимает `listener` (владение) и крутит accept-loop вечно. Уход
//! рекордера — дроп `JoinHandle` или shutdown runtime'а → tokio отменяет future. Per-connection
//! таски получают свой клон `Arc<Metrics>`; cancel-safety — read-line-then-write, оба
//! `tokio::io` операции cancel-safe на `TcpStream` (после read-line мы не зависим от future'а
//! до завершения write).
//!
//! **Безопасность loopback (§3):** `bind` берётся ИЗ ВНЕ, из `main.rs` (`METRICS_BIND_ADDR`,
//! дефолт `127.0.0.1:9100`); тест в `red_metrics_endpoint.rs` биндит `127.0.0.1:0` —
//! ядро выдаёт эфемерный loopback-порт. Никакого `0.0.0.0` здесь нет (избегаем случайного
//! выставления в локалку/интернет).
//!
//! **Журнал/order не трогаем** (architect task-2 carve-out-extended для 4A): этот модуль
//! ТОЛЬКО читает `Arc<Metrics>` по запросу и пишет в сокет; ни `journal::append`, ни
//! `EventKind::Md/Ord/...`, ни `book::OrderBook::apply` здесь не вызываются.

use std::sync::Arc;

use ops::metrics::Metrics;
use ops::server::http_response;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Сколько байт первой строки запроса прочитать максимум. HTTP/1.1 request-line в норме
/// < 8 KiB (`GET /very/long/path?query HTTP/1.1`), плюс запас на будущие query-string.
/// Если клиент пришлёт длиннее — обрезаем, отдаём 414 (или 400) — мы НЕ парсим URL,
/// только первую строку.
const REQUEST_LINE_MAX: usize = 8192;

/// Accept-loop `/metrics`-эндпоинта. Принимает владение `listener` (уже забинденным на
/// loopback) и крутится, пока future не отменят (drop JoinHandle или shutdown runtime'а).
///
/// Каждое соединение обрабатывается в ОТДЕЛЬНОМ спавн-таске: `accept` не блокируется на
/// медленном scrape-клиенте (TCP-прокси, метрики-сборщик с backoff'ом). Per-connection таск
/// получает свой клон `Arc<Metrics>` — `OPS-I-7`: scrape-вызов НЕ блокирует инкремент
/// счётчиков (lock-free на `&self`).
pub async fn serve(listener: TcpListener, metrics: Arc<Metrics>) {
    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                let metrics = Arc::clone(&metrics);
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(stream, metrics).await {
                        tracing::debug!(error = %e, "metrics conn ended with error");
                    }
                });
            }
            Err(e) => {
                // accept-сбой (на практике — listener закрыт или ОС-уровень): логируем и
                // пробуем снова. Если listener закрыт (shutdown) — accept даст
                // `OtherIoError`; НЕ паникуем, recorder пишет данные, а не мониторинг.
                tracing::warn!(error = %e, "metrics accept failed — retry");
                // Краткая пауза, чтобы не закрутить accept-spin на persistent EBADF.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

/// Обработать одно TCP-соединение: прочитать request-line, отдать response, закрыть.
///
/// Протокол минимальный: read до `\n` (первая строка HTTP), построить `http_response`,
/// записать, дроп. Prometheus scrape сам закрывает соединение после чтения тела
/// (HTTP/1.0 short-lived + `Connection: close` в ответе). Любая ошибка I/O — return,
/// вызывающий логирует на debug-уровне (production scrape-ошибка — не алерт).
async fn handle_conn(mut stream: TcpStream, metrics: Arc<Metrics>) -> std::io::Result<()> {
    // (1) Прочитать первую строку запроса (read до '\n', с лимитом REQUEST_LINE_MAX).
    let mut buf = Vec::with_capacity(256);
    let mut tmp = [0u8; 512];
    loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            // EOF до '\n' — клиент закрыл, не дождавшись. Ничего не отдаём (как и обещали:
            // мы — сервер, не инициатор).
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > REQUEST_LINE_MAX {
            // Request-line слишком длинный — отдаём 414 и закрываем (мы не парсер URL,
            // у нас нет компромисса «ещё чуть-чуть потерпеть»).
            stream
                .write_all(
                    b"HTTP/1.1 414 URI Too Long\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await?;
            return Ok(());
        }
        if buf.contains(&b'\n') {
            break;
        }
    }

    // (2) Извлечь request-line до '\n' (отрезать '\r', если есть — HTTP стандарт).
    let line_end = buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len());
    let mut line_bytes = buf[..line_end].to_vec();
    if line_bytes.last() == Some(&b'\r') {
        line_bytes.pop();
    }
    let request_line = match std::str::from_utf8(&line_bytes) {
        Ok(s) => s,
        Err(_) => {
            // request-line не UTF-8 — отдаём 400 и закрываем. Не пытаемся парсить мусор.
            stream
                .write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await?;
            return Ok(());
        }
    };

    // (3) Построить ответ через ЧИСТУЮ трансформацию (юнит-RED в ops::server::http_response).
    //    Здесь — только I/O; никакой логики маршрутизации.
    let response = http_response(request_line, &metrics);

    // (4) Записать и закрыть (Prometheus scrape прочитает до EOF; `Connection: close` мы
    //     уже указали в ответе).
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Smoke-тесты handle_conn (юнит-RED в `tests/red_metrics_endpoint.rs` ловит
    //! end-to-end socket loop; здесь — узел-инварианты, чтобы diff I/O-обвязки падал
    //! явно, а не «неожиданно» через интеграционный RED).

    use super::*;

    #[test]
    fn request_line_max_is_sane() {
        // 8 KiB — намного больше типичной request-line (~50–100 байт), но защищает от
        // медленного loris-стиля чтения без `\n` вечно. Clippy `assertions_on_constants`
        // здесь не триггерит (значение не compile-time const — оно в `const`, но мы
        // используем runtime-bound; clippy пропускает если хотя бы один аргумент — non-const).
        let max = REQUEST_LINE_MAX;
        assert!(max >= 1024, "REQUEST_LINE_MAX {max} < 1024 — слишком тесно");
        assert!(
            max <= 65536,
            "REQUEST_LINE_MAX {max} > 65536 — слишком щедро (loris)"
        );
    }
}
