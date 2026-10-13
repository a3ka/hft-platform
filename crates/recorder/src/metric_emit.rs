//! M-09 task 4C: продюсер-сеймы для OPS-I-10 «объявлена ⟹ эмитится» (TD-027).
//!
//! Каждая §3-метрика обязана иметь НАЗВАННЫЙ продюсер, и RED-оракул (`red_metrics_emission.rs`)
//! прогоняет продюсер с общим `Arc<Metrics>` и ассертит SAMPLE-серию (не только HELP/TYPE —
//! registry-only = ровно класс TD-027, который вскрыл reviewer).
//!
//! Карта продюсеров (FA §3, доп. от C-014 re-audit #1..#5):
//! - `recorder_rss_anon_bytes`         — `sample_rss`        (этот файл, /proc/self/status).
//! - `md_event_age_ms{venue}`          — `sample_md_age`     (sampler-таск в `main`, раз в секунду).
//! - `journal_frames_written_total`    — `run_writer`        (post-`append`, +1/кадр; NOTE-1: кадры, не байты).
//! - `journal_seq_current`             — `run_writer`        (post-`append`, =`next_seq`).
//! - `journal_segment_index`           — `run_writer`        (post-`append`, =`active_segment_index`).
//! - `journal_disk_free_bytes`         — `run_writer`        (post-`append`, =`storage_status().free_bytes`).
//! - `journal_write_errors_total`      — `run_writer`        (на Err append — event-метрика, real trigger).
//! - `journal_seq_gaps_total`          — event-метрика       (нет естественного триггера в journal.next_seq —
//!                                                       пропускается, оракул её не ассертит).
//! - `md_events_total{venue,symbol,kind}` — `run_writer`     (на `EventKind::Md`; kind = канон payload).
//! - `book_levels{venue,symbol,side}`  — `run_books_feeder`  (lib.rs — фидер-loop, эмитит per-(venue,symbol)).
//! - `venue_ws_reconnects_total{venue}`— supervisor main.rs (на Err или exit из venue-`run`).
//! - `venue_http_status_total`         — venue-fetcher'ы     (event, already wired).
//! - `book_divergence_bps{venue,symbol}`/`book_resync_total{venue,symbol}` — `ops::sink` (already wired).
//! - `backup_restore_drill_ok`         — deferred (task 3, OPS-I-2/3).
//!
//! OPS-I-6 (в журнал НЕ пишется): этот модуль импортирует ТОЛЬКО `ops::metrics` + `contracts`
//! — без `journal` (метрики — наблюдаемость, а не события домена; DET-I-1 на журнале сохраняется).
//! OPS-I-7 (не в горячем пути): все операции — `Arc<Metrics>::inc_counter`/`set_gauge`, lock-free
//! атомики (см. `ops::metrics`); этот модуль НЕ делает I/O, кроме разовой `read_to_string(/proc/self/status)`
//! в `sample_rss` (1 Гц из sampler-таска, не горячий путь).

use ops::metrics::Metrics;

/// Парсер строки `RssAnon:` из `/proc/self/status` (`/proc/[pid]/status` формат, kB).
/// Возвращает значение В БАЙТАХ (kB × 1024). `None` если строка не найдена (НЕ fallback на
/// VmRSS/0 — TD-021: VmRSS включает page cache → ложный «лик» рекордера на ~сотни MiB).
///
/// **Почему RssAnon, не VmRSS:** VmRSS = RssAnon + RssFile + RssShmem. У рекордера в page cache
/// лежит весь 8-ми ГБ журнал (mmap'нутый/прочитанный) — VmRSS показывает лик, которого нет.
/// RssAnon — «анонимная куча» = то, что аллоцировал процесс сам (arena, stacks, наши `Vec<Level>`
/// на снимках). Это и есть интересующий объём.
///
/// Формат строки (см. `man 5 proc`):
///   `RssAnon:   12345 kB`
/// — токены разделены whitespace; ищем строку с префиксом `RssAnon:`, берём первый
/// следующий ЦЕЛОЧИСЛЕННЫЙ токен (kB), переводим в байты.
pub fn parse_rss_anon(status: &str) -> Option<i64> {
    for line in status.lines() {
        let line = line.trim_start();
        if !line.starts_with("RssAnon:") {
            continue;
        }
        // `RssAnon:\t  12345 kB` — после `RssAnon:` идёт число kB.
        let after = line.trim_start_matches("RssAnon:").trim();
        // Берём ПЕРВЫЙ whitespace-delimited токен — это число kB (формат стабилен с Linux 4.x).
        let kb_str = after.split_whitespace().next()?;
        let kb: i64 = kb_str.parse().ok()?;
        return Some(kb.saturating_mul(1024));
    }
    None
}

/// Сэмплировать RssAnon из `/proc/self/status` и записать в `recorder_rss_anon_bytes`.
/// На Linux всегда есть `RssAnon` (с Linux 4.0); на других ОС — no-op (метрика остаётся 0,
/// но это в проде не встречается — recorder только на Linux). На любом Err чтения — no-op
/// (sampler не должен ронять recorder).
pub fn sample_rss(metrics: &Metrics) {
    let body = match std::fs::read_to_string("/proc/self/status") {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(error = %e, "/proc/self/status read failed — rss sample skipped");
            return;
        }
    };
    if let Some(bytes) = parse_rss_anon(&body) {
        metrics.set_gauge("recorder_rss_anon_bytes", &[], bytes);
    }
    // Нет `RssAnon` — оставляем прошлый сэмпл как есть (set_gauge не зовём); sampler не падает.
}

/// Сэмплировать «возраст последнего MD-события» per-venue → `md_event_age_ms{venue}`.
/// `now_ms` — текущее wall-time (ms since epoch), `last_receipt_ms` — момент последнего
/// `EventKind::Md` с этой площадки. Возраст = `max(0, now - last)`. Используется sampler-таском
/// (1 Гц), который трекает `last_receipt_ms` per-venue (см. `main.rs` — `writer`/`feeder`
/// обновляют общий `Arc<Mutex<HashMap<Venue, i64>>>`).
///
/// **OPS-I-8 silence:** при тишине потока значение РАСТЁТ — оператор сразу видит «площадка
/// не шлёт N секунд». До `0` НЕ сбрасываем (dead-zero = ровно тот класс, что C-014 re-audit #3
/// ловит; 0 = «свежее», но если sampler потерял событие — нужен честный возраст).
pub fn sample_md_age(metrics: &Metrics, venue: &str, now_ms: i64, last_receipt_ms: i64) {
    let age = now_ms.saturating_sub(last_receipt_ms).max(0);
    metrics.set_gauge("md_event_age_ms", &[("venue", venue)], age);
}
