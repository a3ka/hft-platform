//! RED OPS-I-10 ЖИВАЯ ЭМИССИЯ МЕТРИК (sacred, architect-only) — урок TD-027. `ops.md` §3/§6.
//!
//! TD-027 (§8, reviewer): task 4 дал реестр + /metrics + правила + паритет OPS-I-5 (зелёный), но
//! 13/15 метрик НЕ подключены к продюсерам — инкрементировались только `book_divergence_bps` +
//! `venue_http_status_total`. Правила P0/P1 формирующих инцидентов (TD-011/014/016/OPS-GAP)
//! ссылались на МЁРТВЫЕ метрики. Тот же класс, что recon-wiring кормил пустую книгу: паритет
//! проверял ИМЕНА реестра, а не РАНТАЙМ-ЭМИССИЮ.
//!
//! Контракт OPS-I-10: ОБЪЯВЛЕНА ⟹ ЭМИТИТСЯ. Эти оракулы прогоняют РЕАЛЬНЫЕ продюсер-сеймы
//! (§3 продюсер-карта) с общим `Arc<Metrics>` и ассертят SAMPLE-серию (`name{labels} value`), а НЕ
//! только `# HELP`/`# TYPE`. `has_sample` отличает эмиссию от реестра — анти-TD-027 в лоб.
//!
//! Анти-плацебо: registry-only impl (продюсеры не трогают metrics) несёт лишь HELP/TYPE → has_sample
//! == false → все падают. Против `todo!()`-сеймов — все падают. Против wired impl — GREEN.
//!
//! Сеймы (engine-dev, carve-out task-4C): `run_writer(...,&Metrics,...)` эмитит journal_* + md_*;
//! `metric_emit::emit_book_levels` — book_levels; `metric_emit::sample_rss` — recorder_rss_anon_bytes.

use std::sync::Arc;

use contracts::{EventKind, Level, MdEvent, MdPayload, SysEvent, Venue};
use journal::Journal;
use ops::metrics::Metrics;
use recorder::metric_emit::{emit_book_levels, sample_rss};
use recorder::recon_loop::{apply_md_to_books, ReconBooks};
use recorder::run_writer;

const UNIT: i64 = 100_000_000;

/// SAMPLE-строка метрики `name`: НЕ комментарий (`#`) и начинается с `name` + `{`/` ` (серия
/// `name{labels} v` или `name v`). Отличает РАНТАЙМ-эмиссию от реестровых `# HELP`/`# TYPE` строк.
fn has_sample(text: &str, name: &str) -> bool {
    text.lines().any(|l| {
        !l.starts_with('#')
            && l.starts_with(name)
            && l[name.len()..]
                .chars()
                .next()
                .map(|c| c == ' ' || c == '{')
                .unwrap_or(false)
    })
}

/// Значение последнего поля SAMPLE-строки метрики `name` (первая совпавшая серия).
fn sample_value(text: &str, name: &str) -> Option<i64> {
    text.lines()
        .find(|l| {
            !l.starts_with('#')
                && l.starts_with(name)
                && l[name.len()..]
                    .chars()
                    .next()
                    .map(|c| c == ' ' || c == '{')
                    .unwrap_or(false)
        })
        .and_then(|l| l.split_whitespace().last())
        .and_then(|v| v.parse::<i64>().ok())
}

fn l2_event(sym: &str, n: usize) -> EventKind {
    let bids: Vec<Level> = (0..n)
        .map(|i| Level {
            price: 65_000 * UNIT - (i as i64 + 1) * UNIT,
            size: UNIT,
        })
        .collect();
    let asks: Vec<Level> = (0..n)
        .map(|i| Level {
            price: 65_000 * UNIT + (i as i64 + 1) * UNIT,
            size: UNIT,
        })
        .collect();
    EventKind::Md(MdEvent {
        venue: Venue::Binance,
        symbol: sym.to_string(),
        payload: MdPayload::L2Snapshot {
            bids,
            asks,
            ts_exch_ms: 1,
        },
    })
}

/// (1, ПАДАЕТ против registry-only) Прогон РЕАЛЬНОГО writer'а на Md+Sys событиях → journal-* и md-*
/// метрики несут SAMPLE (не только HELP/TYPE). Ровно TD-027: writer append'ит, но метрики были мертвы.
#[tokio::test]
async fn writer_emits_journal_and_md_metrics() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::open(dir.path()).unwrap();
    let metrics = Arc::new(Metrics::new());

    let (tx, rx) = tokio::sync::mpsc::channel::<EventKind>(500);
    // 30 L2Snapshot (Md) + 20 Heartbeat (Sys) — writer append'ит все, метрики обязаны ожить.
    for _ in 0..30 {
        tx.send(l2_event("BTCUSDT", 5)).await.unwrap();
    }
    for _ in 0..20 {
        tx.send(EventKind::Sys(SysEvent::Heartbeat)).await.unwrap();
    }
    let (sd_tx, sd_rx) = tokio::sync::oneshot::channel::<()>();
    sd_tx.send(()).unwrap();

    run_writer(
        rx,
        journal,
        dir.path().join("recorder.heartbeat"),
        Arc::clone(&metrics),
        async move {
            let _ = sd_rx.await;
        },
    )
    .await
    .unwrap();

    let text = metrics.prometheus_text();
    for m in [
        "journal_bytes_written_total",
        "journal_seq_current",
        "journal_segment_index",
        "journal_disk_free_bytes",
        "md_events_total",
        "md_event_age_ms",
    ] {
        assert!(
            has_sample(&text, m),
            "после прогона writer'а метрика `{m}` НЕ несёт SAMPLE-серию (только # HELP/# TYPE) — \
             продюсер не подключён (TD-027: объявлена, но мертва). OPS-I-10 нарушен"
        );
    }
    assert!(
        sample_value(&text, "journal_bytes_written_total").unwrap_or(0) > 0,
        "journal_bytes_written_total == 0 после 50 append'ов — writer пишет, а счётчик байт мёртв \
         (ровно TD-011-метрика «жив, но не пишет» неработоспособна)"
    );
    assert!(
        sample_value(&text, "md_events_total").unwrap_or(0) >= 1,
        "md_events_total не инкрементирован на 30 Md-событиях (TD-014-метрика «класс событий пропал» мертва)"
    );
}

/// (2, ПАДАЕТ против registry-only) books-feeder-эмиссия: после apply_snapshot + emit_book_levels
/// метрика `book_levels{...,side}` несёт SAMPLE с реальной глубиной.
#[tokio::test]
async fn feeder_emits_book_levels() {
    let books: ReconBooks = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let metrics = Metrics::new();

    apply_md_to_books(&books, &l2_event("BTCUSDT", 5)).await;
    emit_book_levels(&books, &metrics).await;

    let text = metrics.prometheus_text();
    assert!(
        has_sample(&text, "book_levels"),
        "book_levels НЕ несёт SAMPLE после наполнения книги — TD-016-метрика (рост уровней) мертва \
         (объявлена, но продюсер не подключён)"
    );
    assert_eq!(
        sample_value(&text, "book_levels"),
        Some(5),
        "book_levels != 5 после снапшота на 5 уровней/сторону — глубина не измеряется корректно"
    );
}

/// (3, ПАДАЕТ против registry-only) RSS-sampler: sample_rss эмитит `recorder_rss_anon_bytes` SAMPLE
/// (TD-016 лик памяти). Значение из `/proc/self/status` RssAnon (Linux CI) — присутствие серии
/// критично (анти-TD-027); точное значение проверяет §8 на проде (non-zero тренд).
#[tokio::test]
async fn rss_sampler_emits_anon_bytes() {
    let metrics = Metrics::new();
    sample_rss(&metrics);
    let text = metrics.prometheus_text();
    assert!(
        has_sample(&text, "recorder_rss_anon_bytes"),
        "recorder_rss_anon_bytes НЕ несёт SAMPLE после sample_rss — TD-016-метрика (лик памяти) мертва; \
         sampler не подключён к /proc/self/status RssAnon"
    );
}
