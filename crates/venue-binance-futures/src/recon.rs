//! Recon REST snapshot fetcher для Binance USDT-M perp (OPS-I-1 + OPS-I-9 на стороне venue).
//!
//! Симметрично `venue_binance::recon` (Binance spot). Endpoint другой:
//! `/fapi/v1/depth` (futures) vs `/api/v3/depth` (spot); `limit=1000` vs `5000`;
//! парсер fapi требует поле `T` (transact-time). Переиспользуем существующий
//! [`super::parse_depth_snapshot`] (pub).
//!
//! **TD-013 anti-hot-loop STRUCTURAL** — все sleep'и берутся ИЗ
//! `ops::budget::ReconBudget::next_delay(outcome)` (RED-тестированной). См.
//! подробнее в `venue_binance::recon` — общая структура loop'а и инварианты.
//!
//! **HL вне M-09 (TD-005, ≤20 уровней)** — этот креёдт покрывает оба Binance-рынка
//! (spot + USDT-M perp), оба с осмысленной глубиной для recon-компаратора.

use std::sync::Arc;
use std::time::{Duration, Instant};

use book::OrderBook;
use contracts::MdPayload;
use ops::budget::{ReconBudget, RestOutcome, RECON_BASE_DELAY};
use ops::metrics::Metrics;
use tokio::sync::mpsc;

use super::parse_depth_snapshot;

/// Имя venue для label-метрик.
pub const VENUE_LABEL: &str = "binance_futures";
/// Дефолтный HTTP-таймаут одного запроса.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Дефолтный `max_per_min` — 1 запрос в минуту (window-логика `ReconBudget`).
pub const DEFAULT_MAX_PER_MIN: u32 = 1;

/// `GET /fapi/v1/depth?symbol=...&limit=1000` — независимый эндпоинт для recon.
const REST_DEPTH_BASE: &str = "https://fapi.binance.com/fapi/v1/depth?symbol=";
const REST_DEPTH_LIMIT: &str = "1000";

/// Ошибка recon-fetch'а. Семантика согласована с `ops::budget::RestOutcome`.
#[derive(Debug)]
pub enum ReconError {
    /// 418 (IP-ban) / 429 (rate-limit). `retry_after` — из `Retry-After` header.
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
    pub fn to_outcome(&self) -> RestOutcome {
        match self {
            Self::RateLimited { retry_after, .. } => RestOutcome::RateLimited {
                retry_after: *retry_after,
            },
            Self::Malformed(_) | Self::Other(_) => RestOutcome::Error,
        }
    }
}

pub fn classify_outcome(result: &Result<OrderBook, ReconError>) -> RestOutcome {
    match result {
        Ok(_) => RestOutcome::Ok,
        Err(e) => e.to_outcome(),
    }
}

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

/// Распарсить JSON-ответ `/fapi/v1/depth` в канонический `book::OrderBook`.
/// Чистая функция (без I/O). Переиспользует [`super::parse_depth_snapshot`]
/// (pub) — единая точка парсинга fapi-формата.
pub fn parse_recon_snapshot(symbol: &str, json: &str) -> Result<OrderBook, ReconError> {
    let md = parse_depth_snapshot(symbol, json)
        .ok_or_else(|| ReconError::Malformed("fapi depth snapshot parse failed".into()))?;
    let MdPayload::L2Snapshot { bids, asks, .. } = md.payload else {
        return Err(ReconError::Malformed("not a L2 snapshot".into()));
    };
    let mut book = OrderBook::new();
    book.apply_snapshot(&bids, &asks);
    Ok(book)
}

/// `Retry-After` header (RFC 7231 §7.1.3) → `Duration`.
pub fn parse_retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let v = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    v.parse::<u64>().ok().map(Duration::from_secs)
}

/// Сырой JSON-fetch одного snapshot с timeout-ом.
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

/// Один fetch (HTTP + parse) → `OrderBook` или `ReconError`. Публичный.
pub async fn fetch_recon_snapshot(
    client: &reqwest::Client,
    symbol: &str,
    timeout: Duration,
) -> Result<OrderBook, ReconError> {
    let json = fetch_snapshot_json(client, symbol, timeout).await?;
    parse_recon_snapshot(symbol, &json)
}

/// Конфиг recon-фетчера. Cadence — `max_per_min` (window 60с).
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

/// Драйвер recon-цикла. См. `venue_binance::recon::ReconFetcher` — структура loop'а
/// идентична: budget-gate → fetch → classify → next_delay → sleep(delay) →
/// emit (Ok). TD-013 STRUCTURAL anti-hot-loop — все sleep'и из budget.
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

    pub async fn fetch_once(&self) -> Result<OrderBook, ReconError> {
        fetch_recon_snapshot(&self.client, &self.cfg.symbol, self.cfg.request_timeout).await
    }

    /// TD-013 STRUCTURAL anti-hot-loop — все sleep'и из `ops::budget::ReconBudget`
    /// (RED-тестированной). См. подробнее в `venue_binance::recon::ReconFetcher::run`.
    pub async fn run(&mut self, tx: mpsc::Sender<OrderBook>) {
        loop {
            let now = self.start.elapsed();

            if !self.budget.may_request(now) {
                tokio::time::sleep(RECON_BASE_DELAY).await;
                continue;
            }
            self.budget.on_request(now);

            let result = self.fetch_once().await;
            self.metrics.inc_counter(
                "venue_http_status_total",
                &[("venue", VENUE_LABEL), ("code", code_label(&result))],
                1,
            );
            let outcome = classify_outcome(&result);
            let delay = self.budget.next_delay(outcome);

            if let Ok(book) = result {
                if tx.send(book).await.is_err() {
                    return;
                }
            }

            tokio::time::sleep(delay).await;
        }
    }
}

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
            r#"{{"lastUpdateId":12345,"T":1700000000000,"bids":[{}],"asks":[{}]}}"#,
            bids.join(","),
            asks.join(",")
        )
    }

    /// Валидный JSON (fapi требует `T`) → канонический `OrderBook`.
    #[test]
    fn parse_well_formed_yields_canonical_book() {
        let json = mid_book_json(65000.00, 10);
        let book = parse_recon_snapshot("BTCUSDT", &json).expect("ok");
        assert_eq!(book.best_bid(), Some(contracts::to_fixed(65000.00 - 0.01)));
        assert_eq!(book.best_ask(), Some(contracts::to_fixed(65000.00 + 0.01)));
        assert_eq!(book.mid(), Some(contracts::to_fixed(65000.00)));
        assert_eq!(book.n_levels(Side::Buy), 10);
        assert_eq!(book.n_levels(Side::Sell), 10);
    }

    /// size==0 уровни пропускаются (drop, не fabricate).
    #[test]
    fn parse_skips_zero_size_levels() {
        let json = r#"{"lastUpdateId":1,"T":1,"bids":[["100.00","1.0"],["99.00","0"]],"asks":[["101.00","0"],["102.00","2.0"]]}"#;
        let book = parse_recon_snapshot("BTCUSDT", json).expect("ok");
        assert_eq!(book.best_bid(), Some(contracts::to_fixed(100.00)));
        assert_eq!(book.best_ask(), Some(contracts::to_fixed(102.00)));
        assert_eq!(book.n_levels(Side::Buy), 1);
        assert_eq!(book.n_levels(Side::Sell), 1);
    }

    /// Пустой bids/asks → пустая книга.
    #[test]
    fn parse_empty_book_is_valid() {
        let json = r#"{"lastUpdateId":1,"T":1,"bids":[],"asks":[]}"#;
        let book = parse_recon_snapshot("BTCUSDT", json).expect("ok");
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
    }

    /// Malformed: not-JSON → `ReconError::Malformed`, не паника.
    #[test]
    fn parse_garbage_returns_malformed() {
        let r = parse_recon_snapshot("BTCUSDT", "not json");
        assert!(matches!(r, Err(ReconError::Malformed(_))));
    }

    /// Malformed: отсутствует `T` (обязательное поле fapi) → `Malformed`.
    #[test]
    fn parse_missing_t_returns_malformed() {
        let json = r#"{"lastUpdateId":1,"bids":[],"asks":[]}"#;
        let r = parse_recon_snapshot("BTCUSDT", json);
        assert!(matches!(r, Err(ReconError::Malformed(_))));
    }

    /// Malformed: отсутствует `bids` → `Malformed`.
    #[test]
    fn parse_missing_bids_returns_malformed() {
        let json = r#"{"lastUpdateId":1,"T":1,"asks":[]}"#;
        let r = parse_recon_snapshot("BTCUSDT", json);
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

    /// `to_outcome`: прочие → Error.
    #[test]
    fn non_rate_limited_to_outcome_is_error() {
        let m = ReconError::Malformed("x".into());
        assert!(matches!(m.to_outcome(), RestOutcome::Error));
        let o = ReconError::Other(anyhow::anyhow!("net"));
        assert!(matches!(o.to_outcome(), RestOutcome::Error));
    }

    /// `classify_outcome` round-trip.
    #[test]
    fn classify_outcome_maps_correctly() {
        let ok: Result<OrderBook, ReconError> = Ok(OrderBook::new());
        assert!(matches!(classify_outcome(&ok), RestOutcome::Ok));
        let err: Result<OrderBook, ReconError> = Err(ReconError::Malformed("x".into()));
        assert!(matches!(classify_outcome(&err), RestOutcome::Error));
    }

    /// `code_label`: 4 значения.
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

    /// `parse_retry_after_header` round-trip.
    #[test]
    fn retry_after_header_parsing() {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(reqwest::header::RETRY_AFTER, "60".parse().unwrap());
        assert_eq!(parse_retry_after_header(&h), Some(Duration::from_secs(60)));
        let empty = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after_header(&empty), None);
    }

    /// `ReconConfig::new` дефолты.
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
