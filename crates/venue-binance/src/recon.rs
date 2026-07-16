//! Recon REST snapshot fetcher для Binance spot (OPS-I-1 + OPS-I-9 на стороне venue).
//!
//! **Задача.** Периодически (cadence через `ReconBudget::max_per_min`) дёргать
//! независимый REST-снапшот стакана (`GET /api/v3/depth`) и отдавать результат
//! оркестратору (отдельная задача engine-dev, task 3 M-09), который вызовет
//! `ops::recon::reconcile` против локальной книги. Это ЕДИНСТВЕННАЯ проверка
//! правильности данных (эвикция C1 стирала best bid при зелёном healthcheck —
//! `docs/fa/ops.md` §4).
//!
//! **MD-only**: read-only REST, без подписи, без order-egress. **Эмиттер, не
//! владелец**: не вызывает `reconcile` (принадлежит оркестратору), не владеет
//! риск/позициями, не пишет в журнал (JR-I-1 — это recorder'а задача), не держит
//! `journal`-handle. Граница venue: отдать `book::OrderBook` через канал +
//! держать rate-budget.
//!
//! **TD-013 anti-hot-loop STRUCTURAL.** Все задержки `tokio::time::sleep` берутся
//! ИЗ `ops::budget::ReconBudget::next_delay(outcome)` — RED-тестированной функции
//! (`crates/ops/tests/red_ops_budget.rs`, sacred) с инвариантами:
//!
//! - никогда не 0 после Error/RateLimited;
//! - honor `Retry-After` header (≥ cooldown по коду);
//! - exp backoff ×2, cap `RECON_MAX_DELAY` (300с);
//! - reset на `Ok`.
//!
//! Это СТРУКТУРНАЯ гарантия (а не комментарий): единственный sleep-источник после
//! fetch = `next_delay`. Тест-канарейка в `crates/recorder/tests/red_recon_loop.rs`
//! (sacred, architect-owned) проверит: инъекция 418/429 → spacing ≥ budget-задержки,
//! `max_per_min` не превышен.
//!
//! **Cadence.** Управляется `ReconBudget::max_per_min` (window 60с в `may_request`):
//! `max_per_min=1` → не чаще 1/мин; `max_per_min=N` → не чаще N/мин. Это всё, что
//! нужно для rate-budget безопасности; «опрос раз в 5 мин» из `docs/fa/ops.md` §4
//! выражается через комбинацию `max_per_min` + возможный внешний scheduler.

use std::sync::Arc;
use std::time::{Duration, Instant};

use book::OrderBook;
use ops::budget::{ReconBudget, RestOutcome, RECON_BASE_DELAY};
use ops::metrics::Metrics;
use tokio::sync::mpsc;

/// Имя venue для label-метрик (`venue_http_status_total{venue="binance",...}`).
pub const VENUE_LABEL: &str = "binance";
/// Дефолтный HTTP-таймаут одного запроса.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Дефолтный `max_per_min` — 1 запрос в минуту = spacing ≥ 60с (window-логика
/// `ReconBudget::may_request`). См. `docs/fa/ops.md` §4 «раз в 5 мин, per symbol» —
/// реализуется через комбинацию `max_per_min` + внешний scheduler (оркестратор).
pub const DEFAULT_MAX_PER_MIN: u32 = 1;

/// `GET /api/v3/depth?symbol=...&limit=5000` — независимый эндпоинт для recon.
/// Источник истины для сравнения с локально реконструированной книгой.
const REST_DEPTH_BASE: &str = "https://api.binance.com/api/v3/depth?symbol=";
const REST_DEPTH_LIMIT: &str = "5000";

/// Ошибка recon-fetch'а. Семантика согласована с `ops::budget::RestOutcome` через
/// [`ReconError::to_outcome`]: `RateLimited → RateLimited { retry_after }`, прочее → `Error`.
#[derive(Debug)]
pub enum ReconError {
    /// 418 (IP-ban) / 429 (rate-limit). `retry_after` — из `Retry-After` header
    /// (RFC 7231 §7.1.3, delta-seconds), если биржа прислала; иначе `None` —
    /// бюджет применит дефолтный cooldown по статусу.
    RateLimited {
        status: u16,
        retry_after: Option<Duration>,
    },
    /// 2xx с мусором / неполным JSON. НЕ fabrication (VN-I-7).
    Malformed(String),
    /// Сеть / таймаут / 5xx / не-418/429 HTTP-ошибки. exp backoff (OPS-I-9).
    Other(anyhow::Error),
}

impl std::fmt::Display for ReconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RateLimited {
                status,
                retry_after,
            } => match retry_after {
                Some(ra) => write!(f, "rate-limited (status {status}), Retry-After {ra:?}"),
                None => write!(f, "rate-limited (status {status}), Retry-After absent"),
            },
            Self::Malformed(msg) => write!(f, "malformed snapshot response: {msg}"),
            Self::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ReconError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl ReconError {
    /// Транслировать ошибку в `RestOutcome` для `ReconBudget::next_delay`.
    /// `RateLimited` честно пробрасывает `retry_after` (honor биржи); прочее → `Error`
    /// (exp backoff).
    pub fn to_outcome(&self) -> RestOutcome {
        match self {
            Self::RateLimited { retry_after, .. } => RestOutcome::RateLimited {
                retry_after: *retry_after,
            },
            Self::Malformed(_) | Self::Other(_) => RestOutcome::Error,
        }
    }
}

/// Сопоставить результат с `RestOutcome` для budget-обновления.
pub fn classify_outcome(result: &Result<OrderBook, ReconError>) -> RestOutcome {
    match result {
        Ok(_) => RestOutcome::Ok,
        Err(e) => e.to_outcome(),
    }
}

/// Метка `code=` для `venue_http_status_total{venue, code}` (cardinality control:
/// 4 distinct значения).
fn code_label(result: &Result<OrderBook, ReconError>) -> &'static str {
    match result {
        Ok(_) => "200",
        Err(ReconError::RateLimited { status, .. }) => match *status {
            418 => "418",
            429 => "429",
            _ => "rate_limited",
        },
        Err(ReconError::Malformed(_)) => "parse_err",
        Err(ReconError::Other(_)) => "error",
    }
}

/// Распарсить JSON-ответ `/api/v3/depth` в канонический `book::OrderBook`.
/// Чистая функция (без I/O) — тестируется без сети.
///
/// Формат: `{"lastUpdateId":u64,"bids":[["p","q"],...],"asks":[[...]]}`. Цены/размеры —
/// строки (Binance FIX), парсятся в f64 → `to_fixed` (×1e8). size==0 уровни пропускаются.
pub fn parse_recon_snapshot(json: &str) -> Result<OrderBook, ReconError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| ReconError::Malformed(format!("JSON parse: {e}")))?;
    let obj = value
        .as_object()
        .ok_or_else(|| ReconError::Malformed("response is not a JSON object".into()))?;
    let bids_raw = obj
        .get("bids")
        .ok_or_else(|| ReconError::Malformed("missing 'bids'".into()))?
        .as_array()
        .ok_or_else(|| ReconError::Malformed("'bids' is not an array".into()))?;
    let asks_raw = obj
        .get("asks")
        .ok_or_else(|| ReconError::Malformed("missing 'asks'".into()))?
        .as_array()
        .ok_or_else(|| ReconError::Malformed("'asks' is not an array".into()))?;

    let bids = parse_levels(bids_raw, "bid")?;
    let asks = parse_levels(asks_raw, "ask")?;

    let mut book = OrderBook::new();
    book.apply_snapshot(&bids, &asks);
    Ok(book)
}

fn parse_levels(
    arr: &[serde_json::Value],
    side: &str,
) -> Result<Vec<contracts::Level>, ReconError> {
    let mut out = Vec::with_capacity(arr.len());
    for entry in arr {
        let pair = entry.as_array().ok_or_else(|| {
            ReconError::Malformed(format!("{side} entry not an array [price, qty]"))
        })?;
        let price = pair
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| ReconError::Malformed(format!("{side} price not a string")))?;
        let qty = pair
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ReconError::Malformed(format!("{side} qty not a string")))?;
        let p: f64 = price
            .parse()
            .map_err(|e| ReconError::Malformed(format!("{side} price parse: {e}")))?;
        let q: f64 = qty
            .parse()
            .map_err(|e| ReconError::Malformed(format!("{side} qty parse: {e}")))?;
        let size = contracts::to_fixed(q);
        if size > 0 {
            out.push(contracts::Level {
                price: contracts::to_fixed(p),
                size,
            });
        }
    }
    Ok(out)
}

/// `Retry-After` header (RFC 7231 §7.1.3) → `Duration`. Binance использует формат
/// delta-seconds (целое число секунд); HTTP-date игнорируем (Binance не применяет).
pub fn parse_retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let v = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    v.parse::<u64>().ok().map(Duration::from_secs)
}

/// Сырой JSON-fetch одного snapshot с timeout-ом. Публичный — может переиспользоваться
/// оркестратором для offline-тестов на записанных снимках (replay-проверка, audit).
pub async fn fetch_snapshot_json(
    client: &reqwest::Client,
    symbol: &str,
    timeout: Duration,
) -> Result<String, ReconError> {
    let url = format!("{REST_DEPTH_BASE}{symbol}&limit={REST_DEPTH_LIMIT}");
    let response = client
        .get(&url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| ReconError::Other(e.into()))?;
    let status = response.status();
    if status.as_u16() == 418 || status.as_u16() == 429 {
        let retry_after = parse_retry_after_header(response.headers());
        return Err(ReconError::RateLimited {
            status: status.as_u16(),
            retry_after,
        });
    }
    let text = response
        .error_for_status()
        .map_err(|e| ReconError::Other(e.into()))?
        .text()
        .await
        .map_err(|e| ReconError::Other(e.into()))?;
    Ok(text)
}

/// Один fetch (HTTP + parse) → `OrderBook` или `ReconError`. Публичный для unit/integration
/// тестов и для оркестратора, если он предпочитает делать цикл сам.
pub async fn fetch_recon_snapshot(
    client: &reqwest::Client,
    symbol: &str,
    timeout: Duration,
) -> Result<OrderBook, ReconError> {
    let json = fetch_snapshot_json(client, symbol, timeout).await?;
    parse_recon_snapshot(&json)
}

/// Конфиг recon-фетчера. Cadence выражается через `max_per_min` (window 60с в
/// `ReconBudget::may_request`): `max_per_min=1` → не чаще 1/мин.
#[derive(Debug, Clone)]
pub struct ReconConfig {
    pub symbol: String,
    pub request_timeout: Duration,
    pub max_per_min: u32,
}

impl ReconConfig {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            max_per_min: DEFAULT_MAX_PER_MIN,
        }
    }
}

/// Драйвер recon-цикла. Владеет `ReconBudget` (per venue). Принимает `Arc<Metrics>`
/// из deploy/ для инкремента `venue_http_status_total{venue="binance",code=...}`.
///
/// **Эмиттер, не владелец (VN-I-*, JR-I-1):**
///
/// - НЕ вызывает `ops::recon::reconcile` — это делает оркестратор (engine-dev, task 3);
/// - НЕ пишет в журнал — это recorder (JR-I-1);
/// - НЕ держит `journal`-handle.
///
/// Только отдаёт сырой `book::OrderBook` через канал + держит rate-budget.
pub struct ReconFetcher {
    client: reqwest::Client,
    cfg: ReconConfig,
    budget: ReconBudget,
    metrics: Arc<Metrics>,
    start: Instant,
}

impl ReconFetcher {
    pub fn new(client: reqwest::Client, cfg: ReconConfig, metrics: Arc<Metrics>) -> Self {
        let budget = ReconBudget::new(cfg.max_per_min);
        Self {
            client,
            cfg,
            budget,
            metrics,
            start: Instant::now(),
        }
    }

    pub fn config(&self) -> &ReconConfig {
        &self.cfg
    }

    /// Один fetch (HTTP + parse → `OrderBook` или `ReconError`). Публичный для
    /// unit/integration тестов и для оркестратора.
    pub async fn fetch_once(&self) -> Result<OrderBook, ReconError> {
        fetch_recon_snapshot(&self.client, &self.cfg.symbol, self.cfg.request_timeout).await
    }

    /// Запустить цикл до закрытия `tx` (получатель ушёл → graceful exit).
    ///
    /// **TD-013 STRUCTURAL anti-hot-loop.** Каждый `tokio::time::sleep` берёт длительность
    /// ИЗ `ops::budget::ReconBudget` (RED-тестирован):
    ///
    /// - `continue`-ветка (budget не разрешает) → `RECON_BASE_DELAY` (константа из
    ///   `ops::budget`, согласована с OPS-I-9 «не 0 после ошибки»);
    /// - пост-fetch → `budget.next_delay(outcome)` — единственный источник.
    ///
    /// Это структурная гарантия: никаких hardcoded «времённых констант», удаление которых
    /// может вернуть hot-loop. Удалить `sleep(delay)` = сломать компиляцию (delay
    /// не используется). Удалить `RECON_BASE_DELAY` из continue = использовать не-changed-
    /// by-budget magic number, что легко ловится code-review.
    pub async fn run(&mut self, tx: mpsc::Sender<OrderBook>) {
        loop {
            let now = self.start.elapsed();

            // Budget gate: cooldown / window full → анти-hot-spin (RECON_BASE_DELAY ≥ BASE).
            if !self.budget.may_request(now) {
                tokio::time::sleep(RECON_BASE_DELAY).await;
                continue;
            }
            self.budget.on_request(now);

            // HTTP + parse.
            let result = self.fetch_once().await;

            // Метрика на КАЖДЫЙ ответ (200/418/429/error — все считаются, §3 ops.md).
            self.metrics.inc_counter(
                "venue_http_status_total",
                &[("venue", VENUE_LABEL), ("code", code_label(&result))],
                1,
            );

            // Классификация → budget.next_delay (honor Retry-After, exp backoff, reset на Ok).
            let outcome = classify_outcome(&result);
            let delay = self.budget.next_delay(outcome);

            // Успех → оркестратору. Если он ушёл — graceful exit (orchestrator shutdown).
            if let Ok(book) = result {
                if tx.send(book).await.is_err() {
                    return;
                }
            }

            // TD-013 STRUCTURAL: единственный sleep-источник после fetch = budget.
            tokio::time::sleep(delay).await;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit-тесты публичного API крейта (parse + classify + Retry-Header + конфиг).
// NOT SACRED — публичный API крейта, не architectural-инвариант. Liveness-RED
// (TD-013 anti-hot-loop проверка) пишет architect в `crates/recorder/tests/
// red_recon_loop.rs` (sacred) — он инжектит 418/429-поток через wiremock-сервер и
// проверяет spacing ≥ budget-задержки и max_per_min не превышен. Наши тесты —
// pure-функции, не I/O.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::Side;

    fn mid_book_json(mid: f64, depth: usize) -> String {
        let tick = 0.01_f64;
        let bids: Vec<String> = (1..=depth)
            .map(|k| format!("[\"{:.8}\", \"5.0\"]", mid - (k as f64) * tick))
            .collect();
        let asks: Vec<String> = (1..=depth)
            .map(|k| format!("[\"{:.8}\", \"5.0\"]", mid + (k as f64) * tick))
            .collect();
        format!(
            r#"{{"lastUpdateId":12345,"bids":[{}],"asks":[{}]}}"#,
            bids.join(","),
            asks.join(",")
        )
    }

    /// Валидный JSON → канонический `OrderBook` с правильными best_bid/best_ask.
    #[test]
    fn parse_well_formed_yields_canonical_book() {
        let json = mid_book_json(65000.00, 10);
        let book = parse_recon_snapshot(&json).expect("ok");
        assert_eq!(book.best_bid(), Some(contracts::to_fixed(65000.00 - 0.01)));
        assert_eq!(book.best_ask(), Some(contracts::to_fixed(65000.00 + 0.01)));
        assert_eq!(book.mid(), Some(contracts::to_fixed(65000.00)));
        assert_eq!(book.n_levels(Side::Buy), 10);
        assert_eq!(book.n_levels(Side::Sell), 10);
    }

    /// size==0 уровни пропускаются (drop, не fabricate, VN-I-7).
    #[test]
    fn parse_skips_zero_size_levels() {
        let json = r#"{"lastUpdateId":1,"bids":[["100.00","1.0"],["99.00","0"]],"asks":[["101.00","0"],["102.00","2.0"]]}"#;
        let book = parse_recon_snapshot(json).expect("ok");
        assert_eq!(book.best_bid(), Some(contracts::to_fixed(100.00)));
        assert_eq!(book.best_ask(), Some(contracts::to_fixed(102.00)));
        assert_eq!(book.n_levels(Side::Buy), 1);
        assert_eq!(book.n_levels(Side::Sell), 1);
    }

    /// Пустой bids/asks → пустая книга (валидный ответ «нет ликвидности»).
    #[test]
    fn parse_empty_book_is_valid() {
        let json = r#"{"lastUpdateId":1,"bids":[],"asks":[]}"#;
        let book = parse_recon_snapshot(json).expect("ok");
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
    }

    /// Malformed: not-JSON → `ReconError::Malformed`, не паника (VN-I-7).
    #[test]
    fn parse_garbage_returns_malformed() {
        let r = parse_recon_snapshot("not json");
        assert!(matches!(r, Err(ReconError::Malformed(_))));
    }

    /// Malformed: отсутствует `bids` → `Malformed` (НЕ best_bid=None — это бы скрыло проблему).
    #[test]
    fn parse_missing_bids_returns_malformed() {
        let json = r#"{"lastUpdateId":1,"asks":[]}"#;
        let r = parse_recon_snapshot(json);
        assert!(matches!(r, Err(ReconError::Malformed(_))));
    }

    /// Malformed: bids — не массив → `Malformed`.
    #[test]
    fn parse_bids_not_array_returns_malformed() {
        let json = r#"{"lastUpdateId":1,"bids":"oops","asks":[]}"#;
        let r = parse_recon_snapshot(json);
        assert!(matches!(r, Err(ReconError::Malformed(_))));
    }

    /// Malformed: bid price не строка (Binance FIX — строки; число = неправильный формат).
    #[test]
    fn parse_bid_price_not_string_returns_malformed() {
        let json = r#"{"lastUpdateId":1,"bids":[[123,"1.0"]],"asks":[]}"#;
        let r = parse_recon_snapshot(json);
        assert!(matches!(r, Err(ReconError::Malformed(_))));
    }

    /// `to_outcome`: RateLimited → RestOutcome::RateLimited с retry_after.
    #[test]
    fn rate_limited_error_to_outcome() {
        let e = ReconError::RateLimited {
            status: 429,
            retry_after: Some(Duration::from_secs(60)),
        };
        match e.to_outcome() {
            RestOutcome::RateLimited {
                retry_after: Some(ra),
            } => {
                assert_eq!(ra, Duration::from_secs(60));
            }
            _ => panic!("expected RateLimited with retry_after"),
        }
    }

    /// `to_outcome`: Malformed / Other → RestOutcome::Error (exp backoff).
    #[test]
    fn non_rate_limited_to_outcome_is_error() {
        let m = ReconError::Malformed("test".into());
        assert!(matches!(m.to_outcome(), RestOutcome::Error));
        let o = ReconError::Other(anyhow::anyhow!("net"));
        assert!(matches!(o.to_outcome(), RestOutcome::Error));
    }

    /// `classify_outcome`: Ok → RestOutcome::Ok; Err → Err.to_outcome().
    #[test]
    fn classify_outcome_maps_correctly() {
        let ok: Result<OrderBook, ReconError> = Ok(OrderBook::new());
        assert!(matches!(classify_outcome(&ok), RestOutcome::Ok));
        let err: Result<OrderBook, ReconError> = Err(ReconError::Malformed("x".into()));
        assert!(matches!(classify_outcome(&err), RestOutcome::Error));
    }

    /// `code_label`: 4 канонических значения для cardinality control.
    #[test]
    fn code_label_canonical_values() {
        let ok: Result<OrderBook, ReconError> = Ok(OrderBook::new());
        assert_eq!(code_label(&ok), "200");
        let rl_418 = Err(ReconError::RateLimited {
            status: 418,
            retry_after: None,
        });
        assert_eq!(code_label(&rl_418), "418");
        let rl_429 = Err(ReconError::RateLimited {
            status: 429,
            retry_after: None,
        });
        assert_eq!(code_label(&rl_429), "429");
        let parse_err: Result<OrderBook, ReconError> = Err(ReconError::Malformed("x".into()));
        assert_eq!(code_label(&parse_err), "parse_err");
        let net_err: Result<OrderBook, ReconError> = Err(ReconError::Other(anyhow::anyhow!("net")));
        assert_eq!(code_label(&net_err), "error");
    }

    /// `parse_retry_after_header`: delta-seconds → Duration; missing/garbage → None.
    #[test]
    fn retry_after_header_parsing() {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(reqwest::header::RETRY_AFTER, "120".parse().unwrap());
        assert_eq!(parse_retry_after_header(&h), Some(Duration::from_secs(120)));
        let empty = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after_header(&empty), None);
        let mut bad = reqwest::header::HeaderMap::new();
        bad.insert(reqwest::header::RETRY_AFTER, "garbage".parse().unwrap());
        assert_eq!(parse_retry_after_header(&bad), None);
    }

    /// Smoke: выход `parse_recon_snapshot` имеет правильный fixed-point scale (×1e8).
    #[test]
    fn parse_levels_apply_to_book_with_correct_scale() {
        let json = r#"{"lastUpdateId":1,"bids":[["100.50","2.5"]],"asks":[["101.00","3.0"]]}"#;
        let book = parse_recon_snapshot(json).expect("ok");
        assert_eq!(book.best_bid(), Some(10_050_000_000));
        assert_eq!(book.best_ask(), Some(10_100_000_000));
        assert_eq!(book.size_at(Side::Buy, 10_050_000_000), 250_000_000);
    }

    /// `ReconConfig::new` — дефолты соответствуют §4 ops.md (max_per_min=1).
    #[test]
    fn recon_config_defaults() {
        let cfg = ReconConfig::new("BTCUSDT");
        assert_eq!(cfg.symbol, "BTCUSDT");
        assert_eq!(cfg.request_timeout, DEFAULT_REQUEST_TIMEOUT);
        assert_eq!(cfg.max_per_min, DEFAULT_MAX_PER_MIN);
    }

    /// `ReconFetcher::new` + `config()` roundtrip.
    #[test]
    fn recon_fetcher_config_roundtrip() {
        let metrics = Arc::new(Metrics::new());
        let client = reqwest::Client::new();
        let cfg = ReconConfig {
            symbol: "BTCUSDT".into(),
            request_timeout: Duration::from_secs(5),
            max_per_min: 5,
        };
        let fetcher = ReconFetcher::new(client, cfg.clone(), metrics);
        assert_eq!(fetcher.config().symbol, "BTCUSDT");
        assert_eq!(fetcher.config().request_timeout, Duration::from_secs(5));
        assert_eq!(fetcher.config().max_per_min, 5);
    }
}
