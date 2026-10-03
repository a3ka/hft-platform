//! RED OPS-I-4 `/metrics` HTTP-СЕРВЕР — ЧИСТЫЙ слой (sacred, architect-only). `ops.md` §3.
//!
//! Гэп task 4A: `Metrics::prometheus_text()` есть, но НИКТО не сервит его по HTTP — §8 recon
//! пришлось мерить bounded-декодером журнала (наблюдаемости в рантайме не было). Task 4A даёт
//! HTTP-эндпоинт `/metrics` (Prometheus text, отдельный loopback-порт, без внешнего доступа).
//!
//! Разделение (ops остаётся БЕЗ tokio — только `contracts`+`book`): ЧИСТАЯ трансформация
//! `request-line → raw HTTP-ответ` живёт в `ops::server::http_response` (детерминированная, юнит-RED
//! здесь); socket accept-loop — в `recorder::metrics_server` (integration-RED
//! `crates/recorder/tests/red_metrics_endpoint.rs`, реальный TCP на loopback).
//!
//! Контракт `ops::server::http_response(request_line: &str, metrics: &Metrics) -> String`:
//!  - `GET /metrics HTTP/1.1` → `HTTP/1.1 200`, `Content-Type: text/plain`, тело = `prometheus_text()`;
//!  - не-`/metrics` путь → `HTTP/1.1 404`;
//!  - не-`GET` метод → `HTTP/1.1 405`.
//!
//! Анти-плацебо: стаб `"HTTP/1.1 200\r\n\r\n"` (пустое тело) валит `get_metrics_*` (нет §3-метрик и
//! set-значения); «200 на любой путь» валит `non_metrics_path_is_404`; «всегда 200» валит
//! `non_get_method_is_405`. Против `todo!()` — все падают.

use ops::metrics::{MetricKind, Metrics, METRICS};
use ops::server::http_response;

/// Первая строка запроса как её прочитает recorder из сокета (метод, цель, версия).
fn req(line: &str) -> String {
    format!("{line} HTTP/1.1")
}

/// GET /metrics → 200 + Content-Type + тело несёт РЕАЛЬНОЕ значение (не заглушка).
#[test]
fn get_metrics_returns_200_with_live_body() {
    let m = Metrics::new();
    // Установить известное значение счётчика без labels → тело обязано его нести (семантика).
    let name = METRICS
        .iter()
        .find(|s| s.kind == MetricKind::Counter && s.labels.is_empty())
        .map(|s| s.name)
        .expect("нужен counter без labels для семантической проверки");
    m.inc_counter(name, &[], 7);

    let resp = http_response(&req("GET /metrics"), &m);
    assert!(
        resp.starts_with("HTTP/1.1 200"),
        "GET /metrics не вернул 200 (`{}`) — scrape-эндпоинт не отвечает",
        resp.lines().next().unwrap_or("")
    );
    assert!(
        resp.to_ascii_lowercase()
            .contains("content-type: text/plain"),
        "ответ /metrics без `Content-Type: text/plain` — Prometheus не распарсит scrape"
    );
    assert!(
        resp.contains(&format!("{name} 7")),
        "тело /metrics не несёт установленное значение `{name} 7` — сервер отдаёт заглушку/пустоту, \
         а не `prometheus_text()` (значение метрики не видно в scrape)"
    );
}

/// OPS-I-4 над HTTP: КАЖДАЯ метрика §3 присутствует в теле /metrics (grep-канарейка по scrape).
#[test]
fn every_section3_metric_is_served() {
    let m = Metrics::new();
    let resp = http_response(&req("GET /metrics"), &m);
    for spec in METRICS {
        assert!(
            resp.contains(spec.name),
            "метрика `{}` из §3 НЕ в теле /metrics — подсистема невидима для мониторинга через scrape \
             (OPS-I-4 над HTTP)",
            spec.name
        );
    }
}

/// Не-`/metrics` путь → 404 (сервер не отдаёт метрики по произвольному пути; анти-плацебо против
/// «200 на любой путь»).
#[test]
fn non_metrics_path_is_404() {
    let m = Metrics::new();
    for path in ["GET /", "GET /healthz", "GET /metrics/../secret"] {
        let resp = http_response(&req(path), &m);
        assert!(
            resp.starts_with("HTTP/1.1 404"),
            "путь `{path}` не дал 404 (`{}`) — сервер отдаёт метрики/200 по чужому пути (нет маршрутизации)",
            resp.lines().next().unwrap_or("")
        );
    }
}

/// Не-`GET` метод на /metrics → 405 (scrape — только чтение; анти-плацебо против «всегда 200»).
#[test]
fn non_get_method_is_405() {
    let m = Metrics::new();
    for line in ["POST /metrics", "DELETE /metrics"] {
        let resp = http_response(&req(line), &m);
        assert!(
            resp.starts_with("HTTP/1.1 405"),
            "метод в `{line}` не дал 405 (`{}`) — сервер обслужил не-GET на scrape-эндпоинте",
            resp.lines().next().unwrap_or("")
        );
    }
}

/// Детерминизм: одинаковый запрос + одинаковое состояние метрик → идентичный ответ (чистая функция,
/// без wall-clock/rand). Scrape воспроизводим.
#[test]
fn http_response_is_deterministic() {
    let m = Metrics::new();
    m.set_gauge("md_event_age_ms", &[("venue", "binance")], 42);
    let a = http_response(&req("GET /metrics"), &m);
    let b = http_response(&req("GET /metrics"), &m);
    assert_eq!(
        a, b,
        "два одинаковых scrape дали разный ответ — http_response недетерминирована"
    );
}
