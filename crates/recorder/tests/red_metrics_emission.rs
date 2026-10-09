//! RED OPS-I-10 ЖИВАЯ ЭМИССИЯ МЕТРИК (sacred, architect-only) — урок TD-027. `ops.md` §3/§6.
//!
//! TD-027 (§8, reviewer): task 4 дал реестр + /metrics + правила + паритет OPS-I-5 (зелёный), но
//! 13/15 метрик НЕ подключены к продюсерам — инкрементировались только `book_divergence_bps` +
//! `venue_http_status_total`. Правила P0/P1 формирующих инцидентов (TD-011/014/016/OPS-GAP)
//! ссылались на МЁРТВЫЕ метрики. Тот же класс, что recon-wiring кормил пустую книгу: паритет
//! проверял ИМЕНА реестра, а не РАНТАЙМ-ЭМИССИЮ.
//!
//! Контракт OPS-I-10: ОБЪЯВЛЕНА ⟹ ЭМИТИТСЯ. Оракулы прогоняют РЕАЛЬНЫЕ продюсер-сеймы (§3
//! продюсер-карта) с общим `Arc<Metrics>` и ассертят SAMPLE-серию с ВЕРНОЙ размерностью И значением.
//!
//! СТРОГОСТЬ (C-014 + re-audit #1/#2/#3): недостаточно «серия присутствует». Оракул обязан ловить:
//!  - registry-only (только HELP/TYPE) — `has_sample`;
//!  - схлопывание размерности (безлейбловый `md_events_total 30`) — `has_labeled_sample`;
//!  - КОЛЛАПС ЗНАЧЕНИЙ LABEL (все venue→"binance", side→"bid", неверный kind) — МУЛЬТИ-вендор/символ/
//!    kind/side фикстура + `labeled_sample_value` с ТОЧНЫМИ per-серия числами;
//!  - DEAD-ZERO для steady-величин (`journal_seq_current`/`journal_disk_free_bytes`/`md_event_age_ms`
//!    всегда 0) — value-ассерты `> 0` / точное `now-last`.
//!
//! Анти-плацебо доказан architect'ом прототипом в обе стороны (см. Handoff). Против wired impl — GREEN.

use std::sync::Arc;

use contracts::{EventKind, Level, MdEvent, MdPayload, Side, SysEvent, Venue};
use journal::Journal;
use ops::metrics::Metrics;
use recorder::metric_emit::{parse_rss_anon, sample_md_age, sample_rss};
use recorder::recon_loop::ReconBooks;
use recorder::{run_books_feeder, run_writer};

const UNIT: i64 = 100_000_000;

/// Канонический venue-label (контракт, согласован с `ops::sink`/venue-адаптерами).
fn vlabel(v: Venue) -> &'static str {
    match v {
        Venue::Binance => "binance",
        Venue::BinanceFutures => "binance_futures",
        Venue::Hyperliquid => "hyperliquid",
    }
}

/// SAMPLE-строка `name` (серия `name{...} v` или `name v`), НЕ `# HELP`/`# TYPE`.
fn has_sample(text: &str, name: &str) -> bool {
    text.lines().any(|l| is_series_line(l, name))
}

fn sample_value(text: &str, name: &str) -> Option<i64> {
    text.lines()
        .find(|l| is_series_line(l, name))
        .and_then(parse_last)
}

fn is_series_line(l: &str, name: &str) -> bool {
    !l.starts_with('#')
        && l.starts_with(name)
        && l[name.len()..]
            .chars()
            .next()
            .map(|c| c == ' ' || c == '{')
            .unwrap_or(false)
}

fn parse_last(l: &str) -> Option<i64> {
    l.split_whitespace().last().and_then(|v| v.parse().ok())
}

/// Требует ЛЕЙБЛОВАННУЮ серию `name{...}` с КАЖДЫМ ключом из `keys` (`key=`). Ловит схлопывание
/// размерности (безлейбловый sample; урок C-009 M2).
fn has_labeled_sample(text: &str, name: &str, keys: &[&str]) -> bool {
    text.lines().any(|l| {
        !l.starts_with('#')
            && l.starts_with(name)
            && l[name.len()..].starts_with('{')
            && keys.iter().all(|k| l.contains(&format!("{k}=")))
    })
}

/// Значение КОНКРЕТНОЙ серии `name{... k="v" ...}` (первая с ВСЕМИ парами). Label-aware: различает
/// venue/symbol/kind/side и ловит labeled-но-нулевой sample.
fn labeled_sample_value(text: &str, name: &str, pairs: &[(&str, &str)]) -> Option<i64> {
    text.lines()
        .find(|l| {
            !l.starts_with('#')
                && l.starts_with(name)
                && l[name.len()..].starts_with('{')
                && pairs
                    .iter()
                    .all(|(k, v)| l.contains(&format!("{k}=\"{v}\"")))
        })
        .and_then(parse_last)
}

/// АСИММЕТРИЧНЫЙ снапшот произвольной площадки: `n_bid` бидов, `n_ask` асков.
fn l2_event(venue: Venue, sym: &str, n_bid: usize, n_ask: usize) -> EventKind {
    let bids: Vec<Level> = (0..n_bid)
        .map(|i| Level {
            price: 65_000 * UNIT - (i as i64 + 1) * UNIT,
            size: UNIT,
        })
        .collect();
    let asks: Vec<Level> = (0..n_ask)
        .map(|i| Level {
            price: 65_000 * UNIT + (i as i64 + 1) * UNIT,
            size: UNIT,
        })
        .collect();
    EventKind::Md(MdEvent {
        venue,
        symbol: sym.to_string(),
        payload: MdPayload::L2Snapshot {
            bids,
            asks,
            ts_exch_ms: 1,
        },
    })
}

/// КАНОНИЧЕСКИЕ `kind`-label (контракт engine-dev — ОДИН на вариант `MdPayload`): `Trade`→`trade`,
/// `L2Snapshot`→`l2snapshot`, `Funding`→`funding`, `OpenInterest`→`open_interest`,
/// `Liquidation`→`liquidation`, `MarginRate`→`margin_rate`. kind ОБЯЗАН отражать РЕАЛЬНЫЙ тип payload.
fn trade_event(venue: Venue, sym: &str) -> EventKind {
    EventKind::Md(MdEvent {
        venue,
        symbol: sym.to_string(),
        payload: MdPayload::Trade {
            price: 65_000 * UNIT,
            size: UNIT,
            side: Side::Buy,
            ts_exch_ms: 1,
        },
    })
}

/// (1) РЕАЛЬНЫЙ writer → journal-* (значение, не только присутствие) + md_events_total с РАЗЛИЧИМЫМИ
/// venue/symbol/kind (мульти-вендор/символ/kind ловит коллапс любой размерности; C-014 re-audit #3).
#[tokio::test]
async fn writer_emits_journal_and_md_metrics() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::open(dir.path()).unwrap();
    let metrics = Arc::new(Metrics::new());

    let (tx, rx) = tokio::sync::mpsc::channel::<EventKind>(500);
    // Матрица, где КАЖДАЯ размерность различима уникальным счётчиком (коллапс любой → неверное число):
    for _ in 0..30 {
        tx.send(l2_event(Venue::Binance, "BTCUSDT", 5, 5))
            .await
            .unwrap(); // baseline
    }
    for _ in 0..5 {
        tx.send(l2_event(Venue::BinanceFutures, "BTCUSDT", 5, 5))
            .await
            .unwrap(); // venue различает
    }
    for _ in 0..7 {
        tx.send(l2_event(Venue::Binance, "ETHUSDT", 5, 5))
            .await
            .unwrap(); // symbol различает
    }
    for _ in 0..10 {
        tx.send(trade_event(Venue::Binance, "BTCUSDT"))
            .await
            .unwrap(); // kind различает
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
    // journal_segment_index может быть 0 легитимно (первый сегмент) → только присутствие серии.
    assert!(
        has_sample(&text, "journal_segment_index"),
        "journal_segment_index НЕ несёт SAMPLE — продюсер не подключён (TD-027)"
    );
    // DEAD-ZERO ловим (C-014 re-audit #3): steady-величины обязаны быть > 0.
    assert!(
        sample_value(&text, "journal_frames_written_total").unwrap_or(0) > 0,
        "journal_frames_written_total == 0 после 72 append'ов — счётчик кадров мёртв (TD-011; NOTE-1: кадры, не байты)"
    );
    assert!(
        sample_value(&text, "journal_seq_current").unwrap_or(0) > 0,
        "journal_seq_current == 0 после 72 append'ов — seq-гейдж мёртв/константа (dead-zero, C-014 re-audit #3)"
    );
    assert!(
        sample_value(&text, "journal_disk_free_bytes").unwrap_or(0) > 0,
        "journal_disk_free_bytes == 0 — свободное место на диске не измеряется (dead-zero; TD-006 P0-метрика мертва)"
    );
    // md_events_total: КАЖДАЯ размерность различима (коллапс venue/symbol/kind → неверное число).
    let b = vlabel(Venue::Binance);
    let bf = vlabel(Venue::BinanceFutures);
    assert_eq!(
        labeled_sample_value(
            &text,
            "md_events_total",
            &[("venue", b), ("symbol", "BTCUSDT"), ("kind", "l2snapshot")]
        ),
        Some(30),
        "md_events_total{{binance,BTCUSDT,l2snapshot}} != 30 — baseline не эмитится/схлопнут"
    );
    assert_eq!(
        labeled_sample_value(&text, "md_events_total", &[("venue", bf), ("symbol", "BTCUSDT"), ("kind", "l2snapshot")]),
        Some(5),
        "md_events_total{{binance_futures,...}} != 5 — VENUE-размерность схлопнута (все venue=binance, C-014 re-audit #3)"
    );
    assert_eq!(
        labeled_sample_value(
            &text,
            "md_events_total",
            &[("venue", b), ("symbol", "ETHUSDT"), ("kind", "l2snapshot")]
        ),
        Some(7),
        "md_events_total{{...,ETHUSDT,...}} != 7 — SYMBOL-размерность схлопнута"
    );
    assert_eq!(
        labeled_sample_value(
            &text,
            "md_events_total",
            &[("venue", b), ("symbol", "BTCUSDT"), ("kind", "trade")]
        ),
        Some(10),
        "md_events_total{{...,kind=trade}} != 10 — KIND не отражает тип payload (TD-014 незаметен)"
    );
}

/// (2) md_event_age_ms через ДЕТЕРМИНИРОВАННЫЙ sampler-сейм `sample_md_age(metrics, venue, now, last)`
/// → gauge = `now - last` (возраст с последнего приёма; OPS-I-8 silence растёт при тишине). Ловит
/// dead-zero (константа 0) И venue-коллапс: per-venue РАЗНЫЕ значения.
#[tokio::test]
async fn md_age_sampler_emits_real_age_per_venue() {
    let metrics = Metrics::new();
    // now=5000; binance приняли на 4800 (age 200), binance_futures на 4000 (age 1000).
    sample_md_age(&metrics, vlabel(Venue::Binance), 5_000, 4_800);
    sample_md_age(&metrics, vlabel(Venue::BinanceFutures), 5_000, 4_000);

    let text = metrics.prometheus_text();
    assert!(
        has_labeled_sample(&text, "md_event_age_ms", &["venue"]),
        "md_event_age_ms НЕ несёт labeled SAMPLE `{{venue}}` — silence-метрика (OPS-I-8) мертва"
    );
    assert_eq!(
        labeled_sample_value(&text, "md_event_age_ms", &[("venue", vlabel(Venue::Binance))]),
        Some(200),
        "md_event_age_ms{{binance}} != 200 (now-last=5000-4800) — dead-zero/константа или не считает возраст"
    );
    assert_eq!(
        labeled_sample_value(&text, "md_event_age_ms", &[("venue", vlabel(Venue::BinanceFutures))]),
        Some(1000),
        "md_event_age_ms{{binance_futures}} != 1000 — VENUE-коллапс (перезаписан binance) или dead-zero"
    );
}

/// (3) ЖИВОЙ feeder-loop `run_books_feeder` (тот же, что спавнит main — НЕ leaf; C-014 gap-2). ТРИ
/// книги `(venue,symbol)` с УНИКАЛЬНОЙ асимметричной глубиной — так что venue, symbol И side различимы
/// НЕЗАВИСИМО (C-014 re-audit #4: symbol-collapse `symbol="BTCUSDT"` всегда — тоже false-GREEN):
///  - (Binance, BTCUSDT) bid=5 ask=3;
///  - (Binance, ETHUSDT) bid=4 ask=2  — ТА ЖЕ площадка, ДРУГОЙ символ → symbol обязан различать;
///  - (BinanceFutures, BTCUSDT) bid=6 ask=1 — ДРУГАЯ площадка, ТОТ ЖЕ символ → venue обязан различать.
#[tokio::test]
async fn live_feeder_loop_emits_book_levels() {
    let books: ReconBooks = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let metrics = Arc::new(Metrics::new());

    let (tx, rx) = tokio::sync::mpsc::channel::<EventKind>(16);
    tx.send(l2_event(Venue::Binance, "BTCUSDT", 5, 3))
        .await
        .unwrap();
    tx.send(l2_event(Venue::Binance, "ETHUSDT", 4, 2))
        .await
        .unwrap();
    tx.send(l2_event(Venue::BinanceFutures, "BTCUSDT", 6, 1))
        .await
        .unwrap();
    drop(tx);

    run_books_feeder(rx, Arc::clone(&books), Arc::clone(&metrics)).await;

    let text = metrics.prometheus_text();
    let b = vlabel(Venue::Binance);
    let bf = vlabel(Venue::BinanceFutures);
    // (venue, symbol, side, want) — уникальные значения: коллапс venue|symbol|side → неверное число.
    for (venue, symbol, side, want) in [
        (b, "BTCUSDT", "bid", 5),
        (b, "BTCUSDT", "ask", 3),
        (b, "ETHUSDT", "bid", 4),
        (b, "ETHUSDT", "ask", 2),
        (bf, "BTCUSDT", "bid", 6),
        (bf, "BTCUSDT", "ask", 1),
    ] {
        assert_eq!(
            labeled_sample_value(
                &text,
                "book_levels",
                &[("venue", venue), ("symbol", symbol), ("side", side)]
            ),
            Some(want),
            "book_levels{{venue={venue},symbol={symbol},side={side}}} != {want} — коллапс venue|symbol|\
             side (глубина не различима по площадке/символу/стороне — TD-016-метрика бесполезна)"
        );
    }
}

/// Фикстура `/proc/self/status`, где VmRSS ≠ RssAnon (VmRSS включает page cache файла журнала —
/// ложный «лик», TD-021). Парсер ОБЯЗАН взять RssAnon, НЕ VmRSS.
const PROC_STATUS_FIXTURE: &str = "Name:\trecorder\n\
VmPeak:\t 2100000 kB\n\
VmRSS:\t   999999 kB\n\
RssAnon:\t   12345 kB\n\
RssFile:\t   50000 kB\n\
RssShmem:\t      0 kB\n";

/// (4a, ДЕТЕРМИНИРОВАННЫЙ parser-оракул, C-014 re-audit #5) `parse_rss_anon` берёт ИМЕННО `RssAnon`
/// (не `VmRSS`/`RssFile`) и переводит kB→байты (×1024). TD-021: VmRSS/cgroup включают page cache →
/// ложный лик. Отсутствие `RssAnon` → `None` (НЕ fallback на VmRSS/0).
#[test]
fn rss_parser_reads_rss_anon_not_vmrss() {
    let bytes = parse_rss_anon(PROC_STATUS_FIXTURE).expect("RssAnon обязан распарситься");
    assert_eq!(
        bytes,
        12_345 * 1024,
        "parse_rss_anon вернул {bytes}, а не RssAnon(12345 kB)×1024 — читает VmRSS/RssFile или не ×1024 \
         (TD-021: page cache завышает VmRSS → ложный лик; метрика обязана мерить АНОНИМНУЮ кучу)"
    );
    assert_ne!(
        bytes,
        999_999 * 1024,
        "parse_rss_anon вернул VmRSS вместо RssAnon (TD-021 регресс)"
    );
    assert!(
        parse_rss_anon("Name:\tx\nVmRSS:\t 100 kB\n").is_none(),
        "без строки RssAnon парсер обязан вернуть None, а не fallback на VmRSS/0 (иначе тихо мерит не то)"
    );
}

/// (4b, ЖИВОЙ sampler >0) `sample_rss` на РЕАЛЬНОМ `/proc/self/status` эмитит `recorder_rss_anon_bytes`
/// SAMPLE с НЕНУЛЕВЫМ значением (живой процесс на Linux всегда имеет RssAnon>0). Ловит sample-only-0
/// (C-014 re-audit #5) И registry-only (TD-027).
#[test]
fn rss_sampler_emits_positive_anon_bytes() {
    let metrics = Metrics::new();
    sample_rss(&metrics);
    let text = metrics.prometheus_text();
    assert!(
        has_sample(&text, "recorder_rss_anon_bytes"),
        "recorder_rss_anon_bytes НЕ несёт SAMPLE после sample_rss — TD-016-метрика мертва (sampler не подключён)"
    );
    assert!(
        sample_value(&text, "recorder_rss_anon_bytes").unwrap_or(0) > 0,
        "recorder_rss_anon_bytes == 0 после sample_rss — sampler эмитит 0/константу вместо реального \
         RssAnon (живой процесс на Linux имеет RssAnon>0; dead-zero, C-014 re-audit #5)"
    );
}
