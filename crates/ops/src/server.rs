//! OPS-I-4 — `/metrics` HTTP-сервер, ЧИСТЫЙ слой (sacred contract layer). `ops.md` §3.
//!
//! Разделение слоёв (critic N1, C-007 разделитель):
//!  - ЧИСТАЯ трансформация `request-line → raw HTTP-ответ` — ЗДЕСЬ (детерминированная,
//!    без `tokio`, без `std::net`, без wall-clock; юнит-RED `crates/ops/tests/red_ops_server.rs`).
//!  - socket accept-loop и bind — в `recorder::metrics_server` (`crates/recorder/src/metrics_server.rs`,
//!    integration-RED `crates/recorder/tests/red_metrics_endpoint.rs`, реальный TCP на loopback).
//!
//! Мотив (ops остаётся лёгким — `contracts`+`book`): tokio/http-стек в `crates/ops` создал бы
//! скрытые зависимости от runtime/scheduler и угрозу циклической зависимости
//! (recorder уже владеет `Arc<Metrics>`). Чистая функция = детерминированный scrape, юнит-тест
//! ловит логику без сетевых эффектов, recorder-RED ловит wiring. То же разделение, что у
//! `journal::stream` (чистый декодер) vs recorder-цикл (writer с tokio).
//!
//! Контракт (`http_response(request_line, &Metrics) -> String`):
//!  - `GET /metrics HTTP/1.1` → `HTTP/1.1 200 OK`, `Content-Type: text/plain; version=0.0.4`,
//!    `Content-Length: N`, тело = `metrics.prometheus_text()`;
//!  - путь ≠ `/metrics` → `HTTP/1.1 404 Not Found`, пустое тело;
//!  - метод ≠ `GET` → `HTTP/1.1 405 Method Not Allowed`, `Allow: GET`, пустое тело.
//!
//! Формат ответа — минимальный HTTP/1.1 (нет `Transfer-Encoding`, нет keep-alive header'ов);
//! Prometheus scrape это парсит без претензий (он сам читает до EOF на close-delimited).
//! `Content-Length` — для совместимости с HTTP-клиентами, которые до close не дожидаются.

use crate::metrics::Metrics;

/// MIME Prometheus text exposition format v0.0.4 (канон scrape-эндпоинта).
const CONTENT_TYPE: &str = "Content-Type: text/plain; version=0.0.4\r\n";

/// Построить сырой HTTP/1.1-ответ для scrape-эндпоинта `GET /metrics`.
///
/// Чистая функция (`request_line` + состояние `Metrics` → строка-байты): детерминирована
/// (`http_response_is_deterministic`), без `wall-clock`/rand/IO (anti-placebo + replay-safe).
///
/// `request_line` — ПЕРВАЯ строка HTTP-запроса, как её прочитал recorder из сокета
/// (`GET /metrics HTTP/1.1`, `POST /metrics HTTP/1.1`, `GET /healthz HTTP/1.1`, …). Метод
/// чувствителен к регистру (`GET`/`get` — разное); путь сравнивается байт-в-байт.
pub fn http_response(request_line: &str, metrics: &Metrics) -> String {
    let (method, path) = parse_request_line(request_line);

    if method != "GET" {
        return method_not_allowed();
    }
    if path != "/metrics" {
        return not_found();
    }

    let body = metrics.prometheus_text();
    let len = body.len();
    format!(
        "HTTP/1.1 200 OK\r\n{CONTENT_TYPE}Content-Length: {len}\r\nConnection: close\r\n\r\n{body}"
    )
}

/// Разобрать первую строку запроса на `(method, path)`. Версия HTTP не используется
/// (минимальный HTTP/1.1-ответ совместим и с HTTP/1.0-клиентами).
fn parse_request_line(line: &str) -> (&str, &str) {
    // Метод — первое слово до пробела; путь — между первым пробелом и следующим.
    let mut parts = line.split_ascii_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    (method, path)
}

fn not_found() -> String {
    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
}

fn method_not_allowed() -> String {
    "HTTP/1.1 405 Method Not Allowed\r\nAllow: GET\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        .to_string()
}

#[cfg(test)]
mod tests {
    //! Smoke-проверки парсера (юнит-RED покрывает публичный контракт в
    //! `crates/ops/tests/red_ops_server.rs`; здесь — узел-инварианты, чтобы диф парсера
    //! падал явно, а не «неожиданно» через scrape-RED).
    use super::*;

    #[test]
    fn parse_request_line_basic() {
        assert_eq!(
            parse_request_line("GET /metrics HTTP/1.1"),
            ("GET", "/metrics")
        );
    }

    #[test]
    fn parse_request_line_lowercase_method_is_distinct() {
        // Регистр ЗНАЧИМ: HTTP-методы case-sensitive (RFC 7230 §3.1.1); lowercase → не-GET → 405.
        assert_eq!(
            parse_request_line("get /metrics HTTP/1.1"),
            ("get", "/metrics")
        );
    }

    #[test]
    fn parse_request_line_empty() {
        assert_eq!(parse_request_line(""), ("", ""));
    }

    #[test]
    fn method_not_allowed_carries_allow_header() {
        let resp = method_not_allowed();
        assert!(resp.starts_with("HTTP/1.1 405"));
        assert!(
            resp.contains("Allow: GET"),
            "405 без Allow: GET — клиент не узнает, что разрешено"
        );
    }

    #[test]
    fn not_found_has_zero_body() {
        let resp = not_found();
        assert!(resp.starts_with("HTTP/1.1 404"));
        assert!(resp.contains("Content-Length: 0"));
    }
}
