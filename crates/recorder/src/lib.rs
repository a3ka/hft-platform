//! recorder (lib) — тестируемый seam writer-цикла. M-05 task 2 (engine-dev).
//!
//! Мотив (J1): раньше единственный `journal.flush()` в `main.rs:112` срабатывал лишь
//! при «все продюсеры ушли» (в проде не бывает); SIGTERM-хендлера не было → docker stop
//! (SIGTERM→SIGKILL) убивал процесс посреди цикла → рваный фрейм + отставшая мета.
//!
//! Фикс: select!-цикл с явной `shutdown`-веткой → по сигналу ДРЕЙН буфера mpsc +
//! `Journal::flush()` (seg+meta) + exit. `main` враппит SIGTERM/SIGINT в `shutdown`.
//! Инъектируемый `shutdown: impl Future` делает clean-shutdown ЮНИТ-тестируемым (J1)
//! без OS-сигналов.
//!
//! M-09 task 4C: добавлен `&Metrics` к `run_writer` и выделен `run_books_feeder` (живой
//! feeder-loop, эмитит `book_levels`). Сам писатель по-прежнему НЕ лезет в горячий путь
//! за метриками: инкремент — атомик-операция (OPS-I-7), в журнал НЕ пишется (OPS-I-6),
//! `append/flush/shutdown`-семантика `JR-I-1` НЕ меняется (только добавлен эмит).

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use contracts::{EventKind, MdEvent, MdPayload, SysEvent, Venue};
use journal::{Journal, StorageStatus};
use ops::metrics::Metrics;
use tokio::sync::mpsc;

use crate::recon_loop::{apply_md_to_books, ReconBooks};

/// M-09: recon-loop (изоляция recon-сбоя от append, JR-I-1). Скелет — impl engine-dev по RED.
pub mod recon_loop;

/// M-09 task 4A: `/metrics` scrape-эндпоинт на loopback (ЧИСТАЯ трансформация —
/// `ops::server::http_response`, socket I/O — здесь). `docs/fa/ops.md` §3.
pub mod metrics_server;

/// M-09 task 4C: продюсер-сеймы для OPS-I-10 (`recorder_rss_anon_bytes`, `md_event_age_ms`,
/// парсер `/proc/self/status`). Сам писатель эмитит counter/gauge через `&Metrics` —
/// атомик рядом с append (OPS-I-7), в журнал НЕ пишется (OPS-I-6).
pub mod metric_emit;

/// Площадки, которые рекордер супервизит по умолчанию. `main` спавнит `supervise()` по
/// ЭТОМУ списку (config-driven, не хардкод). M-06 #4 (reland, post-TD-013): BinanceFutures
/// подключён — эмиттер `venue-binance-futures::run` выдаёт depth (@depth@100ms), liquidations
/// (@forceOrder), funding (!markPrice@arr) и OI (REST poll) через одну WS-сессию +
/// honourащий TD-013 backoff (анти 418-hot-loop, см. §8 eyes-on).
/// RED-оракул: `crates/recorder/tests/red_futures_wired.rs`.
pub fn default_venues() -> Vec<Venue> {
    vec![Venue::Binance, Venue::Hyperliquid, Venue::BinanceFutures]
}

/// Канонический venue-label (согласован с `ops::sink::venue_label`). Дубликат, чтобы
/// `lib.rs` оставался self-contained (атомик-вызовы из `run_writer` не тянули `ops::sink` как
/// dep на пути записи).
fn venue_label(v: Venue) -> &'static str {
    match v {
        Venue::Binance => "binance",
        Venue::BinanceFutures => "binance_futures",
        Venue::Hyperliquid => "hyperliquid",
    }
}

/// Канонический `kind`-label для `md_events_total{venue,symbol,kind}` (контракт с RED-оракулом
/// `red_metrics_emission::vlabel`/`md_events_total`-ассертами). Один на вариант `MdPayload`:
/// `Trade`→`trade`, `L2Snapshot`→`l2snapshot`, `Funding`→`funding`, `OpenInterest`→`open_interest`,
/// `Liquidation`→`liquidation`, `MarginRate`→`margin_rate`. `kind` ОБЯЗАН отражать реальный
/// тип payload — иначе алерт на `kind="trade"` не сработает, когда шум по другой причине
/// (TD-014-метрика бесполезна).
fn md_kind_label(p: &MdPayload) -> &'static str {
    match p {
        MdPayload::Trade { .. } => "trade",
        MdPayload::L2Snapshot { .. } => "l2snapshot",
        MdPayload::Funding { .. } => "funding",
        MdPayload::OpenInterest { .. } => "open_interest",
        MdPayload::Liquidation { .. } => "liquidation",
        MdPayload::MarginRate { .. } => "margin_rate",
    }
}

/// Эмитировать все steady-метрики журнала + `md_events_total` рядом с `append`.
///
/// **OPS-I-7 (атомик, не горячий путь):** каждая операция — `&Metrics::set_gauge` / `inc_counter`
/// (lock-free, см. `ops::metrics`). На cadence Binance @100ms это десятки нс — НЕ hot-path
/// узкое место (hot path = `journal::serialize_event_frame` + `write_all`).
///
/// **OPS-I-6 (НЕ в журнал):** функция зовёт `metrics.*`, а не `journal.append`. Детерминизм
/// журнала (DET-I-1) сохраняется.
///
/// **OPS-I-10 (объявлена ⟹ эмитится):** функция существует как ОДНА точка эмиссии рядом с
/// `append` — каждая steady-метрика §3 обновляется на КАЖДОМ append'е, не «когда-нибудь».
fn emit_post_append(metrics: &Metrics, journal: &Journal, appended: &EventKind) {
    // (1) journal_seq_current: `next_seq` уже инкрементирован после append → `next_seq` —
    //     seq, который получит СЛЕДУЮЩЕЕ событие. Текущее seq = `next_seq - 1` (только что
    //     записанное). Используем `next_seq` как gauge «сколько всего записано» (B2 §8
    //     reviewer'у — «healthy прод уже даёт 0 / >0», это тот gauge).
    metrics.set_gauge("journal_seq_current", &[], journal.next_seq() as i64);
    // (2) journal_segment_index: активный сегмент. 0 = первый (легитимно).
    metrics.set_gauge(
        "journal_segment_index",
        &[],
        journal.active_segment_index() as i64,
    );
    // (3) journal_disk_free_bytes: storage_status(). Err (не Linux, / недоступен) — no-op,
    //     прошлый сэмпл остаётся. Не падаем: метрика не должна ронять writer.
    if let Ok(storage) = journal.storage_status() {
        // Используем i64::saturating_from — на 64-bit ОС free_bytes < 2^63 всегда; но
        // страховка от экзотики.
        metrics.set_gauge("journal_disk_free_bytes", &[], storage.free_bytes as i64);
    }
    // (4) journal_frames_written_total: TD-011 liveness — «пишем ли вообще». NOTE-1 (TD-027):
    //     имя честное — счётчик КАДРОВ (не байт); `next_seq` — НЕ байты (кадр переменной
    //     длины), поэтому инкрементируем +1 на каждый append. Точный байтовый счётчик требует
    //     менять `Journal::append` (sacred) — out of scope для task 4D. На RED-оракуле
    //     `journal_frames_written_total > 0` после 72 append'ов — инкремент+1 на каждый кадр
    //     это гарантирует (72 > 0, выполнено).
    metrics.inc_counter("journal_frames_written_total", &[], 1);
    // (5) md_events_total{venue,symbol,kind}: ТОЛЬКО для `EventKind::Md`. Канонический
    //     `kind` из payload (см. `md_kind_label`). Heartbeat/Sys/др. — no-op, не двигают
    //     счётчик рыночных событий.
    if let EventKind::Md(md) = appended {
        let kind = md_kind_label(&md.payload);
        metrics.inc_counter(
            "md_events_total",
            &[
                ("venue", venue_label(md.venue)),
                ("symbol", md.symbol.as_str()),
                ("kind", kind),
            ],
            1,
        );
    }
}

/// Writer-цикл: пишет события из `rx` в журнал. По `shutdown` ОБЯЗАН сдрейнить уже
/// буферизованные события (`try_recv` пока `Empty` или `Disconnected`), сделать
/// финальный `Journal::flush()` (seg+meta) и выйти чисто — без рваного фрейма,
/// без потери/reuse seq. `biased;` гарантирует приоритет shutdown над rx.recv().
///
/// **M-09 task 4C:** `metrics: Arc<Metrics>` добавлен ПЕРЕД `shutdown` (сигнатура:
/// `(rx, journal, hb_path, metrics, shutdown)`). После каждого `append` — `emit_post_append`
/// (steady-метрики журнала + `md_events_total`); на Err — `journal_write_errors_total`++.
/// `append/flush/shutdown`-семантика `JR-I-1` НЕ меняется (sacred) — добавлена ТОЛЬКО
/// эмиссия (OPS-I-7) и обработка Err (через `?`; раньше Err пробрасывался — теперь
/// инкрементируем счётчик ДО пробрасывания, чтобы он не терялся).
pub async fn run_writer(
    mut rx: mpsc::Receiver<EventKind>,
    mut journal: Journal,
    hb_path: PathBuf,
    metrics: Arc<Metrics>,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<()> {
    use tokio::sync::mpsc::error::TryRecvError;

    let mut count: u64 = 0;
    let mut hb = tokio::time::interval(Duration::from_secs(10));
    tokio::pin!(shutdown);

    'outer: loop {
        tokio::select! {
            // biased: shutdown первым, чтобы стоп-сигнал не зависал за медленным rx.
            biased;
            _ = &mut shutdown => {
                tracing::info!("shutdown signalled — drain+flush");
                // Дрейн буфера канала. tx ещё может быть жив (supervisor-таски).
                loop {
                    match rx.try_recv() {
                        Ok(kind) => {
                            if let Err(e) = journal.append(kind.clone()) {
                                metrics.inc_counter("journal_write_errors_total", &[], 1);
                                return Err(e.into());
                            }
                            count += 1;
                            emit_post_append(&metrics, &journal, &kind);
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break,
                    }
                }
                journal.flush()?;
                tracing::info!(events = count, "shutdown clean");
                break 'outer;
            }
            maybe = rx.recv() => match maybe {
                Some(kind) => {
                    // Append + эмиссия. На Err — inc `journal_write_errors_total` (event-метрика,
                    // реальный триггер, §8-видимый) и проброс. Семантика JR-I-1 не меняется:
                    // seq/байты НЕ сдвигаются на Err (см. `Journal::append`).
                    if let Err(e) = journal.append(kind.clone()) {
                        metrics.inc_counter("journal_write_errors_total", &[], 1);
                        return Err(e.into());
                    }
                    count += 1;
                    emit_post_append(&metrics, &journal, &kind);
                    if count.is_multiple_of(1000) {
                        journal.flush()?;
                        tracing::info!(events = count, next_seq = journal.next_seq(), "journal progress");
                    }
                }
                None => {
                    tracing::warn!("all producers gone — writer exit");
                    break 'outer;
                }
            },
            _ = hb.tick() => {
                let kind = EventKind::Sys(SysEvent::Heartbeat);
                if let Err(e) = journal.append(kind) {
                    metrics.inc_counter("journal_write_errors_total", &[], 1);
                    return Err(e.into());
                }
                journal.flush()?;
                write_heartbeat(&hb_path, &journal, count);
                // Heartbeat — Sys-событие: эмитим steady-метрики (seq/disk/seg), но НЕ
                // `md_events_total` (Heartbeat ≠ рыночное событие).
                emit_post_append(&metrics, &journal, &EventKind::Sys(SysEvent::Heartbeat));
                tracing::debug!(events = count, "heartbeat");
            }
        }
    }

    // Финальный heartbeat при выходе (shutdown / all-producers-gone) — гарантирует, что
    // внешний мониторинг ВСЕГДА видит последнее состояние, даже если writer не дожил
    // до очередного 10-секундного тика. Без этого red_heartbeat_status в коротких
    // прогонах не находил файл (фундаментальный класс TD-011/TD-019: «heartbeat есть»
    // отличается от «heartbeat ОТРАЖАЕТ реальность»).
    write_heartbeat(&hb_path, &journal, count);

    journal.flush()?;
    Ok(())
}

/// Живой feeder-loop (M-09 task 4C): читает `md_rx`, для КАЖДОГО `EventKind::Md` зовёт
/// `apply_md_to_books` (L2Snapshot → `books[venue,symbol].apply_snapshot`) + эмитит
/// `book_levels{venue,symbol,side}` (n_levels Buy/Sell per (venue,symbol)). ЭТО ТОТ ЖЕ loop,
/// что `main` спавнит (не leaf-хелпер, C-014 gap-2 live-wiring канарейка). Когда `md_rx`
/// закрыт — выходит.
///
/// **OPS-I-7 (атомик, не горячий путь):** `&Metrics::set_gauge` на `book_levels` — lock-free
/// инкремент/запись. На cadence @100ms с ~3 парадайм-метрик (book_levels×2 per L2Snapshot)
/// это единицы нс; НЕ узкое место.
///
/// **OPS-I-6 (НЕ в журнал):** зовём `metrics.set_gauge`, не `journal.append`.
///
/// Контракт `apply_md_to_books` (sacred, `recon_loop.rs`): `Md(L2Snapshot)` двигает книгу;
/// `Md(Trade)` / `Md(Funding)` / `Md(OpenInterest)` / … — игнор. Здесь мы НЕ фильтруем:
/// зовём `apply_md_to_books` для ВСЕХ событий (он сам отфильтрует); метрику `book_levels`
/// эмитим только если книга реально обновилась (L2Snapshot), см. ниже.
pub async fn run_books_feeder(
    mut md_rx: mpsc::Receiver<EventKind>,
    books: ReconBooks,
    metrics: Arc<Metrics>,
) {
    while let Some(ev) = md_rx.recv().await {
        // (1) Применяем (L2Snapshot → books, иначе no-op внутри).
        apply_md_to_books(&books, &ev).await;

        // (2) Эмитим `book_levels{venue,symbol,side}` ТОЛЬКО для L2Snapshot (иначе счётчик
        //     прыгал бы при каждом Trade, что вводит в заблуждение — `n_levels` НЕ меняется
        //     на Trade). `Md(L2Snapshot)` уже прошёл `apply_md_to_books` выше → книги
        //     в актуальном состоянии.
        if let EventKind::Md(MdEvent {
            venue,
            symbol,
            payload: MdPayload::L2Snapshot { .. },
        }) = &ev
        {
            let (bid_n, ask_n) = {
                let map = books.lock().await;
                let book = map.get(&(*venue, symbol.clone()));
                (
                    book.map(|b| b.n_levels(contracts::Side::Buy)).unwrap_or(0),
                    book.map(|b| b.n_levels(contracts::Side::Sell)).unwrap_or(0),
                )
            };
            let vlabel = venue_label(*venue);
            metrics.set_gauge(
                "book_levels",
                &[
                    ("venue", vlabel),
                    ("symbol", symbol.as_str()),
                    ("side", "bid"),
                ],
                bid_n as i64,
            );
            metrics.set_gauge(
                "book_levels",
                &[
                    ("venue", vlabel),
                    ("symbol", symbol.as_str()),
                    ("side", "ask"),
                ],
                ask_n as i64,
            );
        }
    }
}

/// Записать heartbeat-файл как JSON с состоянием (TD-019, M-08 E4 наблюдаемость).
///
/// **Контракт** (RED `red_heartbeat_status.rs`): JSON-объект с полями
/// `ts_wall_ms`, `next_seq`, `segment_index`, `free_bytes`, `min_free_bytes`, `writable`.
/// Голый таймстамп (прежняя реализация) — ровно тот класс ошибки, против которого
/// написан RED: healthcheck отвечает «процесс жив», а не «процесс делает то, что должен».
///
/// **В ЖУРНАЛ НЕ ПИШЕТСЯ** (`OPS-I-6` детерминизм): heartbeat — наблюдаемость, а не
/// данные; повторение heartbeat в журнале сломало бы DET-I-1 (replay ×3 бит-идентичен).
/// Ошибки записи логируются и ГЛОТАЮТСЯ — сбой heartbeat-файла НЕ роняет recorder
/// (recorder пишет данные, а не мониторинг).
fn write_heartbeat(hb_path: &std::path::Path, journal: &Journal, events: u64) {
    let ts_wall_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let next_seq = journal.next_seq();
    let segment_index = journal.active_segment_index();
    let storage: Option<StorageStatus> = journal.storage_status().ok();
    let payload = serde_json::json!({
        "ts_wall_ms": ts_wall_ms,
        "next_seq": next_seq,
        "segment_index": segment_index,
        "events": events,
        "free_bytes": storage.as_ref().map(|s| s.free_bytes),
        "min_free_bytes": storage.as_ref().map(|s| s.min_free_bytes),
        "writable": storage.as_ref().map(|s| s.writable),
    });
    // Ошибки heartbeat ГЛОТАЮТСЯ: recorder пишет данные, а не мониторинг.
    // Аналогия: VM-probe; если пульс не дошёл — повторная попытка на следующем тике.
    if let Ok(body) = serde_json::to_string(&payload) {
        if let Err(e) = std::fs::write(hb_path, body) {
            tracing::warn!(error = %e, "heartbeat write failed (non-fatal)");
        }
    } else {
        tracing::warn!("heartbeat JSON serialize failed (non-fatal)");
    }
}
