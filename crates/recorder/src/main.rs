//! recorder — вход даталеера. venue-адаптеры (Binance + Hyperliquid + BinanceFutures) →
//! mpsc-канал (EventKind) → журнал (ЕДИНСТВЕННЫЙ писатель, seq тотальный порядок).
//! docs/fa/{venues,journal}.md.
//!
//! Площадки: спавн итерацией по `recorder::default_venues()` (config-driven, не хардкод).
//! Конфиг через env: JOURNAL_DIR, BINANCE_SYMBOLS / HL_COINS / BINANCE_FUTURES_SYMBOLS
//! (csv, defaults BTCUSDT,ETHUSDT / BTC,ETH / BTCUSDT,ETHUSDT). Reconnect + TD-013 backoff —
//! внутри venue::run; здесь supervisor + spawn-цикл.
//!
//! M-05 (engine-dev): SIGTERM/SIGINT → `shutdown` future → `run_writer` дренит mpsc +
//! flush перед exit (J1 — clean-shutdown).
//! M-06 #4 (reland, post-TD-013): подключён Venue::BinanceFutures — funding-breadth C5 вход.
//! MD-only → risk-critic НЕ нужен (gates.md §5 N4).

use std::future::Future;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use book::OrderBook;
use contracts::{EventKind, SysEvent, Venue};
use journal::{Journal, WriterConfig};
use ops::metrics::Metrics;
use ops::recon::{ReconDetector, ReconThresholds};
use ops::sink::handle_recon_snapshot;
use recorder::recon_loop::spawn_recon_isolated;
use tokio::sync::{mpsc, Mutex};

fn env_csv(key: &str, default: &[&str]) -> Vec<String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.split(',').map(|s| s.trim().to_string()).collect(),
        _ => default.iter().map(|s| s.to_string()).collect(),
    }
}

/// Supervisor: гоняет venue::run в цикле с exp-backoff (fail-closed к «нет данных», не паника
/// процесса). ConnDown фиксируется в журнале через канал (единый путь к писателю). ConnUp
/// эмитит сам venue::run при успешном коннекте.
///
/// **M-09 task 4C:** на каждой итерации (сбой или exit) — `venue_ws_reconnects_total{venue}`++
/// (event-метрика на реальный триггер, §8-видимый). OPS-I-7: lock-free атомик-инкремент.
async fn supervise<F, Fut>(
    name: &'static str,
    venue: Venue,
    tx: mpsc::Sender<EventKind>,
    metrics: Arc<Metrics>,
    run: F,
) where
    F: Fn(mpsc::Sender<EventKind>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let venue_lbl = match venue {
        Venue::Binance => "binance",
        Venue::BinanceFutures => "binance_futures",
        Venue::Hyperliquid => "hyperliquid",
    };
    let mut backoff = 1u64;
    loop {
        tracing::info!(venue = name, "venue connect");
        match run(tx.clone()).await {
            Ok(()) => tracing::warn!(venue = name, "venue run exited — reconnect"),
            Err(e) => tracing::error!(venue = name, error = %e, "venue run error — reconnect"),
        }
        // reconnect-event: counter++ рядом с триггером (OPS-I-7, lock-free).
        metrics.inc_counter("venue_ws_reconnects_total", &[("venue", venue_lbl)], 1);
        let _ = tx.send(EventKind::Sys(SysEvent::ConnDown(venue))).await;
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

/// Future, который резолвится по первому из SIGTERM / SIGINT (Unix) или Ctrl-C (fallback).
/// M-05 task 2 (engine-dev): даёт writer'у шанс сдрейнить буфер + flush до выхода.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "SIGTERM handler install failed — falling back to ctrl_c");
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "SIGINT handler install failed — falling back to ctrl_c");
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = sigterm.recv() => tracing::info!("SIGTERM received — initiating clean shutdown"),
            _ = sigint.recv()  => tracing::info!("SIGINT received — initiating clean shutdown"),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Ctrl-C received — initiating clean shutdown");
    }
}

/// Per-symbol recon-fetcher, запущенный через `spawn_recon_isolated` (JR-I-1, 24/7).
/// `ReconFetcher::run` сам владеет `ReconBudget` и rate-limit'ит REST-вызовы; recon-сбой
/// (rate-limit, parse, сеть) изолируется от writer-потока самим tokio-рантаймом. Канал
/// `tx_book` буферизован на `RECON_BOOK_BUFFER` снапшотов (дроп при переполнении — stale
/// бесполезны, журнал не замусоривается).
const RECON_BOOK_BUFFER: usize = 4;

/// Live-книга по (venue, symbol). Заполняется BOOKS-FEEDER'ом (`apply_md_to_books`) из потока
/// `MdEvent::L2Snapshot` (см. books-feeder-таск в `main` ниже); до первого снимка — пустая,
/// orchestrator использует дефолт `OrderBook::new()` (контракт `red_recon_wiring`).
type BooksLive = Arc<Mutex<std::collections::HashMap<(Venue, String), OrderBook>>>;

/// Запустить recon-fetcher (изолированно) и orchestrator для ОДНОГО (venue, symbol).
/// `events_tx` — ЕДИНСТВЕННЫЙ путь в журнал (JR-I-1, тот же канал, что у venue-supervisor'ов).
/// Orchestrator читает `tx_book` → сравнивает с live-книгой → при divergence шлёт
/// `EventKind::Sys(ReconDivergence)` в `events_tx` + обновляет метрики (через sink).
fn spawn_recon_wiring(
    venue: Venue,
    symbol: String,
    metrics: Arc<Metrics>,
    events_tx: mpsc::Sender<EventKind>,
    books: BooksLive,
) {
    // 1. Канал снапшотов: fetcher (REST) → orchestrator.
    let (tx_book, mut rx_book) = mpsc::channel::<OrderBook>(RECON_BOOK_BUFFER);

    // 2. ReconFetcher изолирован (spawn_recon_isolated) — паника НЕ роняет writer.
    //    Сейчас fetcher'ы есть ТОЛЬКО на Binance (spot+futures); HL без recon-модуля.
    //    Для HL task 2 venue-dev отдельным заходом (его зона).
    let symbol_for_fetcher = symbol.clone();
    let metrics_for_fetcher = Arc::clone(&metrics);
    spawn_recon_isolated(move || async move {
        match venue {
            Venue::Binance => {
                let cfg = venue_binance::recon::ReconConfig::new(symbol_for_fetcher.clone());
                let mut fetcher = venue_binance::recon::ReconFetcher::new(
                    reqwest::Client::new(),
                    cfg,
                    Arc::clone(&metrics_for_fetcher),
                );
                fetcher.run(tx_book).await;
            }
            Venue::BinanceFutures => {
                let cfg =
                    venue_binance_futures::recon::ReconConfig::new(symbol_for_fetcher.clone());
                let mut fetcher = venue_binance_futures::recon::ReconFetcher::new(
                    reqwest::Client::new(),
                    cfg,
                    Arc::clone(&metrics_for_fetcher),
                );
                fetcher.run(tx_book).await;
            }
            Venue::Hyperliquid => {
                // HL recon — вне scope M-09 task 2. Не спавним ничего, чтобы не врать
                // test-оракулу.
                tracing::warn!(venue = "hyperliquid", "recon wiring not yet implemented");
            }
        }
    });

    // 3. Orchestrator: читает tx_book, сравнивает с live, шлёт divergence-события.
    //    Сам по себе НЕ долгий: просыпается только когда пришёл снапшот. Backoff в
    //    отсутствие снапшотов не нужен — fetcher уже rate-limit'ит по `ReconBudget`.
    //
    //    M-09 task 2: оконный детектор персистентности (`ops.md` §4.3) — STATEFUL, поэтому
    //    `ReconDetector::new(thr)` поднимается ДО `while let Some(reference)` и передаётся
    //    `&mut detector` в каждый вызов `handle_recon_snapshot`. Детектор живёт всё время
    //    жизни orchestrator-таска (per (venue,symbol) рядом с ReconBudget).
    let metrics_o = Arc::clone(&metrics);
    let events_o = events_tx.clone();
    let symbol_o = symbol.clone();
    spawn_recon_isolated(move || async move {
        let thresholds = ReconThresholds::new(ops::recon::EPS_PROD_DEFAULT_BPS)
            .expect("EPS_PROD_DEFAULT_BPS is a valid prod threshold (≤ EPS_MAX_BPS, fail-closed)");
        let mut detector = ReconDetector::new(thresholds);
        while let Some(reference) = rx_book.recv().await {
            // local: из live-книги (ЗАГЛУШКА для §8 end-to-end). Пока books пустые,
            // divergence считается относительно пустой книги → `exceeds_test() == true`
            // для ЛЮБОГО непустого reference. Это ВРЕМЕННОЕ поведение, не для прода
            // (в проде нужен books-feeder из MdEvent::L2Snapshot; см. TODO в `BooksLive`).
            let local = {
                let map = books.lock().await;
                map.get(&(venue, symbol_o.clone()))
                    .cloned()
                    .unwrap_or_else(OrderBook::new)
            };
            handle_recon_snapshot(
                &mut detector,
                &local,
                &reference,
                venue,
                &symbol_o,
                &metrics_o,
                |ev| {
                    // try_send: если writer-канал полон (back-pressure), drop события,
                    // чтобы orchestrator НЕ блокировался. ReconDivergence — наблюдаемость,
                    // её потеря НЕ роняет сбор; ops::sink + metrics уже зафиксировали факт.
                    let _ = events_o.try_send(ev);
                },
            );
        }
    });
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let dir =
        PathBuf::from(std::env::var("JOURNAL_DIR").unwrap_or_else(|_| "./journal-data".into()));

    tracing::info!(
        journal_dir = %dir.display(),
        schema_version = contracts::SCHEMA_VERSION,
        venues = ?recorder::default_venues(),
        "recorder start"
    );

    let (tx, rx) = mpsc::channel::<EventKind>(50_000);

    // M-09: рекордер владеет Arc<Metrics> (process-global observability); раздаёт `&Metrics`
    // продюсерам (venue-fetcher'ам + recon-циклу). Recon пишет `book_divergence_bps` и
    // `book_resync_total` через `ops::sink`; venue-fetcher'ы уже пишут `venue_http_status_total`.
    let metrics = Arc::new(Metrics::new());

    // M-09 task 4A: `/metrics` scrape-эндпоинт на loopback (`ops.md §3 — без внешнего
    // доступа`). Bind-addr из env `METRICS_BIND_ADDR`, дефолт `127.0.0.1:9101` — loopback-only
    // (НЕ `0.0.0.0`; случайная публикация наружу здесь невозможна). 9101 — чтобы не
    // конфликтовать с типичным node_exporter 9100. Бинд-сбой (порт занят, EBADF) → WARN
    // + продолжение БЕЗ scrape-эндпоинта (recorder пишет данные, а не мониторинг; запись в
    // журнал НЕ должна падать из-за bind-сбоя на metrics). Journal-write/order-путь
    // НЕ затронуты (serve живёт в отдельном таске).
    spawn_metrics_server(Arc::clone(&metrics)).await;

    // M-09 task 4C: last_receipt per venue (для `md_event_age_ms`). Обновляется в fanout при
    // каждом `EventKind::Md`; читается sampler-таском (1 Гц). `Mutex` — короткий (insert на
    // каждое MD-событие), `HashMap` — по 3 ключам (venue). Не в журнал (OPS-I-6): это
    // runtime-состояние, не доменный факт.
    let last_receipt: Arc<Mutex<std::collections::HashMap<Venue, i64>>> =
        Arc::new(Mutex::new(std::collections::HashMap::new()));

    // M-09 task 4C: SAMPLER-ТАСК (1 Гц) — эмитит `recorder_rss_anon_bytes` (RssAnon из
    // /proc/self/status) + `md_event_age_ms{venue}` (now − last_receipt[venue]). Не в горячем
    // пути: /proc/self/status — раз в секунду, Mutex<HashMap> — раз в секунду. Journal-write
    // путь НЕ затронут (только метрики, OPS-I-6). Сам `last_receipt` строится в fanout-тасках
    // (см. ниже); sampler только ЧИТАЕТ.
    {
        let metrics_sampler = Arc::clone(&metrics);
        let last_receipt_sampler = Arc::clone(&last_receipt);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            // `MissedTickBehavior::Skip` — если tick пропущен, НЕ догоняем пачкой (CDS: на старте
            // под нагрузкой возможен drift, но важнее не плодить лавину lock-ов).
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                // (a) RssAnon — через `metric_emit::sample_rss` (lock-free атомик-эмиссия).
                recorder::metric_emit::sample_rss(&metrics_sampler);
                // (b) per-venue md_event_age_ms.
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                // Снимок last_receipt (lock короткий, без await внутри map).
                let snapshot: Vec<(Venue, i64)> = {
                    let map = last_receipt_sampler.lock().await;
                    map.iter().map(|(v, t)| (*v, *t)).collect()
                };
                for (venue, last_ms) in snapshot {
                    let lbl = match venue {
                        Venue::Binance => "binance",
                        Venue::BinanceFutures => "binance_futures",
                        Venue::Hyperliquid => "hyperliquid",
                    };
                    recorder::metric_emit::sample_md_age(&metrics_sampler, lbl, now_ms, last_ms);
                }
            }
        });
    }

    // M-09: live-книга для recon. Заполняется BOOKS-FEEDER'ом из `MdEvent::L2Snapshot`-потока
    // через fanout-tap ниже (см. books-feeder-таск). До первого L2Snapshot — пустая;
    // orchestrator использует дефолт `OrderBook::new()` и в §8 это ВРЕМЕННО даёт ложный флуд
    // (книга появится сразу после первого снапшота в feeder-таске).
    let books: BooksLive = Arc::new(Mutex::new(std::collections::HashMap::new()));

    // MD-events TAP для books-feeder. Отдельный канал (не writer-канал) — федер-таск не конкурирует
    // с writer'ом за lock recv()'а (mpsc::Receiver не share'ится). Логика: каждый venue-emitter
    // шлёт СВОИ события и в writer (через fanout ниже), и в `md_tx` (через тот же fanout, best-effort);
    // books-feeder-таск читает `md_rx` и зовёт `apply_md_to_books(&books, &ev)` для каждого Md.
    // Только `EventKind::Md(MdPayload::L2Snapshot)` реально двигает книгу; Trade/Funding/OI/… ignored
    // (контракт `red_recon_wiring::feeder_ignores_non_l2_events`).
    let (md_tx, md_rx) = mpsc::channel::<EventKind>(50_000);

    // spawn one supervisor per venue from `default_venues()` — config-driven, не 3 хардкод-блока.
    // M-06 #4 (reland, post-TD-013): добавлен `Venue::BinanceFutures` (fstream @depth@100ms +
    // @forceOrder + !markPrice@arr + REST OI poll). Аргументы площадок: `BINANCE_SYMBOLS` /
    // `HL_COINS` / `BINANCE_FUTURES_SYMBOLS`.
    //
    // Type-erasure: три `::run`-функции имеют РАЗНЫЕ concrete-типы возвращаемых futures →
    // общая сигнатура `Fn(Sender) -> Fut` в `supervise()` вместить нельзя. Решение —
    // `Box<dyn Fn(Sender) -> Pin<Box<dyn Future + Send>>>` per call (один dyn-индирект на
    // создание future; внутри спарн-цикла это дешевле, чем статически дублировать N^2 supervisor'ов).
    type VenueRunFut =
        std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>;
    type VenueRunFn = Box<dyn Fn(mpsc::Sender<EventKind>) -> VenueRunFut + Send + Sync>;

    // M-09 task 4C: BOOKS-FEEDER (живой loop, эмитит `book_levels`). ВЫНЕСЕН в `recorder::run_books_feeder`
    // (тот же, что `main` спавнит — не leaf, C-014 gap-2 live-wiring канарейка). Books +
    // метрики + Rx-канал — всё что нужно; last_receipt НЕ здесь (см. fanout ниже).
    let books_for_feeder: BooksLive = Arc::clone(&books);
    let metrics_for_feeder = Arc::clone(&metrics);
    tokio::spawn(async move {
        recorder::run_books_feeder(md_rx, books_for_feeder, metrics_for_feeder).await;
    });

    for venue in recorder::default_venues() {
        let tx_v = tx.clone();
        let md_tx_v = md_tx.clone();
        let metrics_v = Arc::clone(&metrics);
        // M-09: per-venue FANOUT. Venue шлёт `EventKind` в `fanout_tx`; fanout-таск форвардит
        // (a) в writer (`tx_v`) с backpressure (`send().await` — путь записи НЕ теряет события);
        // (b) в books-feeder (`md_tx_v`) best-effort (`try_send` — при переполнении дроп, книги
        //     наблюдательны и не критичны; следующий L2Snapshot восполнит). Клон `EventKind`
        //     неизбежен (mpsc::Sender — exclusive owner); для L2Snapshot это клон `Vec<Level>`,
        //     на cadence 100ms с ~100 уровнями — десятки мкс, не hot-path проблема. journal-write
        //     путь и order-путь НЕ затронуты (writer-канал сохраняет свою backpressure-семантику).
        //
        // M-09 task 4C: fanout ТАКЖЕ обновляет `last_receipt[venue]` для каждого `EventKind::Md`
        // (для `md_event_age_ms` sampler-таска). Mutex короткий (insert), не в горячем пути.
        let last_receipt_v = Arc::clone(&last_receipt);
        let (fanout_tx, mut fanout_rx) = mpsc::channel::<EventKind>(50_000);
        tokio::spawn(async move {
            while let Some(ev) = fanout_rx.recv().await {
                // (a) writer: блокирующий send → сохраняем обратное давление прежним (как если бы
                //     venue слал прямо в `tx_v` без fanout). На saturated writer fanout
                //     буферизует до 50_000 событий, далее venue блокируется — аналогично прежнему.
                if tx_v.send(ev.clone()).await.is_err() {
                    // writer закрыт — прекращаем fanout (recon-таски тоже встанут).
                    break;
                }
                // (b) books-feeder: best-effort. Дроп при переполнении — следующий L2Snapshot
                //     восполнит stale-состояние; recon пропустит ОДИН цикл сравнения, не катастрофа.
                let _ = md_tx_v.try_send(ev.clone());
                // (c) M-09 task 4C: last_receipt для sampler-таска `md_event_age_ms`.
                //     wall_ms берём тут (lock-окно — один insert; не держим lock через await).
                if let EventKind::Md(md) = &ev {
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    let mut map = last_receipt_v.lock().await;
                    map.insert(md.venue, now_ms);
                }
            }
        });
        let (name, run_fn): (&'static str, VenueRunFn) = match venue {
            Venue::Binance => {
                let syms = env_csv("BINANCE_SYMBOLS", &["BTCUSDT", "ETHUSDT"]);
                (
                    "binance",
                    Box::new(move |t| Box::pin(venue_binance::run(t, syms.clone()))),
                )
            }
            Venue::Hyperliquid => {
                let coins = env_csv("HL_COINS", &["BTC", "ETH"]);
                (
                    "hyperliquid",
                    Box::new(move |t| Box::pin(venue_hyperliquid::run(t, coins.clone()))),
                )
            }
            Venue::BinanceFutures => {
                let syms = env_csv("BINANCE_FUTURES_SYMBOLS", &["BTCUSDT", "ETHUSDT"]);
                (
                    "binance_futures",
                    Box::new(move |t| Box::pin(venue_binance_futures::run(t, syms.clone()))),
                )
            }
        };
        tokio::spawn(async move {
            supervise(name, venue, fanout_tx, metrics_v, run_fn).await;
        });

        // M-09 task 2: per-venue recon wiring (fetcher + orchestrator). Изолированно через
        // `spawn_recon_isolated` (JR-I-1, 24/7 — паника recon не роняет writer).
        // Binance + BinanceFutures имеют recon-модули; HL — отдельная задача venue-dev.
        let recon_symbols: &[&str] = match venue {
            Venue::Binance => &["BTCUSDT", "ETHUSDT"],
            Venue::BinanceFutures => &["BTCUSDT", "ETHUSDT"],
            Venue::Hyperliquid => &[],
        };
        for sym in recon_symbols {
            spawn_recon_wiring(
                venue,
                (*sym).to_string(),
                Arc::clone(&metrics),
                tx.clone(),
                Arc::clone(&books),
            );
        }
    }
    drop(tx); // writer завершится, только если все продюсеры уйдут (в норме не уходят).

    // === M-08 task 4: писать заголовок сегмента (CT-RFC-02, E2/E4) ===
    //
    // Каждый НОВЫЙ сегмент открывается заголовком SegmentHeader с provenance =
    // версия recorder'а + git sha + epoch_id. Ротация — внутри `Journal::append()`
    // (порог из `WriterConfig::max_segment_bytes`), disk-guard — там же.
    // Миграция с `Journal::open()` на `open_with()` ОДНА запись = ОДИН коммит; на этой
    // неделе прод-сегмент (8.3 GB, без магии) будет прочитан legacy-путём через
    // явную декларацию `journal.legacy.json` (CT-RFC-02 rev 2, fail-closed).
    let cfg = build_writer_config();
    tracing::info!(
        max_segment_bytes = cfg.max_segment_bytes,
        min_free_bytes = cfg.min_free_bytes,
        provenance = %cfg.provenance,
        epoch_id = %cfg.epoch_id,
        "writer config",
    );
    let journal = Journal::open_with(&dir, cfg)?;
    let hb_path = dir.join("recorder.heartbeat");

    recorder::run_writer(
        rx,
        journal,
        hb_path,
        Arc::clone(&metrics),
        shutdown_signal(),
    )
    .await?;
    Ok(())
}

/// Собрать `WriterConfig` для recorder'а: собственный захват, порог 1 GiB/сегмент,
/// 10 GiB disk-guard, provenance = версия recorder'а + короткий git SHA (если доступен).
fn build_writer_config() -> WriterConfig {
    let version = env!("CARGO_PKG_VERSION");
    let git_sha = git_short_sha().unwrap_or_else(|| "no-git-info".to_string());
    let provenance = format!("recorder v{version} (git:{git_sha})");
    // epoch_id — стабильный ключ эпохи. По умолчанию: "own-<UTC-YYYY-MM>". Оператор
    // может переопределить через env (прод-развёртывание с известным epoch_id).
    let epoch_id = std::env::var("EPOCH_ID").unwrap_or_else(|_| default_epoch_id_now());
    WriterConfig::own_capture(provenance, epoch_id)
}

/// Короткий git SHA текущего HEAD (если рабочая копия — git-репо). Иначе `None`.
/// `git rev-parse --short HEAD` вызывается синхронно на старте recorder'а (раз в процесс).
fn git_short_sha() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `own-<UTC-YYYY-MM>` из текущего времени (дефолт для дев-сборки без явного `EPOCH_ID`).
fn default_epoch_id_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs_per_day = 86_400u64;
    // Unix epoch → 1970-01-01. Грубый расчёт year-month (UTC, без leap-секунд) — достаточно
    // для группировки эпох по месяцу; точный разбор chrono — за рамками M-08.
    let days = now / secs_per_day;
    let (y, m) = days_to_ym(days);
    format!("own-{y:04}-{m:02}")
}

/// Дни → (year, month_utc) без chrono. Алгоритм: 400-летний Gregorian cycle.
fn days_to_ym(days_since_1970: u64) -> (i32, u32) {
    let mut z = days_since_1970 as i64;
    let mut year = 1970i32;
    loop {
        let leap = is_leap(year);
        let year_days = if leap { 366 } else { 365 };
        if z < year_days {
            break;
        }
        z -= year_days;
        year += 1;
    }
    let leap = is_leap(year);
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for &dm in &months {
        if z < dm {
            break;
        }
        z -= dm;
        month += 1;
    }
    (year, month)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Заспавнить `/metrics` scrape-сервер на loopback. M-09 task 4A.
///
/// Bind-addr берётся из env `METRICS_BIND_ADDR` (дефолт `127.0.0.1:9101`) — явно
/// loopback-only, без внешнего доступа (`ops.md §3`). Бинд-сбой (порт занят, EBADF)
/// логируется и таск НЕ спавнится — recorder продолжает писать данные в журнал, как
/// если бы scrape-эндпоинт отсутствовал (а он отсутствует — graceful degradation, не
/// паника процесса).
async fn spawn_metrics_server(metrics: Arc<Metrics>) {
    // Дефолт — loopback `127.0.0.1:9101`. Если env содержит `0.0.0.0:...` явно — выполнится
    // (операторский override), но по умолчанию внешний bind невозможен. Prometheus
    // стандартно слушает на loopback + reverse-proxy; для HFT — bind-loopback + ssh-tunnel
    // к Prometheus — самый безопасный путь.
    let bind_addr = std::env::var("METRICS_BIND_ADDR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1:9101".to_string());

    match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => {
            tracing::info!(bind_addr = %bind_addr, "metrics-server bound (/metrics scrape)");
            tokio::spawn(recorder::metrics_server::serve(listener, metrics));
        }
        Err(e) => {
            tracing::warn!(
                bind_addr = %bind_addr,
                error = %e,
                "metrics-server bind failed — /metrics НЕ доступен; recorder продолжает без него"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_to_ym_epoch() {
        assert_eq!(days_to_ym(0), (1970, 1));
    }

    #[test]
    fn days_to_ym_one_year_later() {
        // 1971-01-01 = day 365
        assert_eq!(days_to_ym(365), (1971, 1));
    }

    #[test]
    fn days_to_ym_leap_year() {
        // 2000-02-29 = day 11015 (29 Feb 2000)
        // 1970-01-01 ... 2000-02-29 = 30 лет + 60 дней в феврале 2000
        // Approximate day: 30*365 + 7 (leap) = 10957 дней до 2000-01-01, + 31 (янв) + 28 (фев до 29) = 11016
        // Точное: 2000-02-29 = day 11016 в предположении что 1970-01-01 это день 0.
        let (y, m) = days_to_ym(11016);
        assert_eq!(y, 2000);
        assert_eq!(m, 2);
    }
}
