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
use recorder::metric_emit::sample_rss;
use recorder::recon_loop::ReconBooks;
use recorder::{run_books_feeder, run_writer};

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

/// Как `has_sample`, но требует ЛЕЙБЛОВАННУЮ серию (`name{...}`) с КАЖДЫМ ключом из `keys`
/// (`key="..."`). Ловит СХЛОПЫВАНИЕ РАЗМЕРНОСТИ (C-014 gap-1 / урок C-009 M2): безлейбловый
/// `md_events_total 30` вместо `md_events_total{venue,symbol,kind}` — venue/symbol/kind не различить,
/// метрика бесполезна (TD-014 «класс событий пропал» нельзя локализовать по venue/kind). `has_sample`
/// такое пропускал (` ` после имени) → false-GREEN; здесь размерность обязана присутствовать.
fn has_labeled_sample(text: &str, name: &str, keys: &[&str]) -> bool {
    text.lines().any(|l| {
        !l.starts_with('#')
            && l.starts_with(name)
            && l[name.len()..].starts_with('{')
            && keys.iter().all(|k| l.contains(&format!("{k}=")))
    })
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
    // Безлейбловые journal-метрики: SAMPLE-серия обязана присутствовать.
    for m in [
        "journal_bytes_written_total",
        "journal_seq_current",
        "journal_segment_index",
        "journal_disk_free_bytes",
    ] {
        assert!(
            has_sample(&text, m),
            "после прогона writer'а метрика `{m}` НЕ несёт SAMPLE-серию (только # HELP/# TYPE) — \
             продюсер не подключён (TD-027: объявлена, но мертва). OPS-I-10 нарушен"
        );
    }
    // Labeled md-метрики: размерность ОБЯЗАНА присутствовать (C-014 gap-1) — иначе схлопнутый
    // `md_events_total 30` прошёл бы, а venue/symbol/kind не различить (TD-014 не локализуем).
    assert!(
        has_labeled_sample(&text, "md_events_total", &["venue", "symbol", "kind"]),
        "md_events_total НЕ несёт labeled SAMPLE `{{venue,symbol,kind}}` после 30 Md-событий — либо \
         не эмитится (TD-027), либо размерность схлопнута (C-009 M2): класс событий не локализовать"
    );
    assert!(
        has_labeled_sample(&text, "md_event_age_ms", &["venue"]),
        "md_event_age_ms НЕ несёт labeled SAMPLE `{{venue}}` — тишина потока (OPS-I-8) не различима по venue"
    );
    assert!(
        sample_value(&text, "journal_bytes_written_total").unwrap_or(0) > 0,
        "journal_bytes_written_total == 0 после 50 append'ов — writer пишет, а счётчик байт мёртв \
         (ровно TD-011-метрика «жив, но не пишет» неработоспособна)"
    );
}

/// (2, ПАДАЕТ против registry-only И против helper-only-non-live) Гоняем ЖИВОЙ feeder-loop
/// `run_books_feeder` — ТОТ ЖЕ, что спавнит `main` (C-014 gap-2: leaf-хелпер, который тест зовёт
/// напрямую, а main НЕ вызывает, — это рекурсия TD-027). Loop читает Md из канала, применяет к книге
/// и эмитит `book_levels{venue,symbol,side}`. Тест шлёт L2Snapshot, закрывает канал → loop
/// обрабатывает и выходит → ассерт labeled SAMPLE с реальной глубиной.
#[tokio::test]
async fn live_feeder_loop_emits_book_levels() {
    let books: ReconBooks = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let metrics = Arc::new(Metrics::new());

    let (tx, rx) = tokio::sync::mpsc::channel::<EventKind>(16);
    tx.send(l2_event("BTCUSDT", 5)).await.unwrap();
    drop(tx); // закрываем канал → run_books_feeder дообработает и выйдет

    // Гоняем САМ live-loop (не leaf emit-хелпер): его же спавнит main (verify live-wiring канарейка).
    run_books_feeder(rx, Arc::clone(&books), Arc::clone(&metrics)).await;

    let text = metrics.prometheus_text();
    assert!(
        has_labeled_sample(&text, "book_levels", &["venue", "symbol", "side"]),
        "book_levels НЕ несёт labeled SAMPLE `{{venue,symbol,side}}` после прогона live-feeder — либо \
         feeder-loop не эмитит (TD-027), либо размерность схлопнута (C-009 M2). TD-016-метрика мертва"
    );
    assert_eq!(
        sample_value(&text, "book_levels"),
        Some(5),
        "book_levels != 5 после снапшота на 5 уровней/сторону — глубина измеряется неверно"
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
