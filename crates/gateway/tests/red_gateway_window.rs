//! RED M-37 (sacred, architect-only) — split-retention корректность + windowed live==replay.
//!
//! Путь А (TD-039): snapshot держит только окно `[at−W, at]` бакет-оконного состояния
//! (heatmap/ohlcv/depth/bubbles), но СЕССИОННО-СКАЛЯРНОЕ (CVD running-база, VP полная текущая
//! сессия, VWAP all-time) переживает эвикцию. Эти оракулы пинуют ИМЕННО тонкие места, которые
//! критик отметил как ломкие: наивная bucket-эвикция теряет базу CVD и режет POC текущей сессии.
//!
//! Приём: сравниваем ОКНОВОЙ snapshot с UNBOUNDED (window=None) на ОДНОМ журнале. Windowing обязан
//! менять ТОЛЬКО охват серий (меньше бакетов), но НЕ значения на удержанных бакетах и НЕ агрегаты
//! сессии. Анти-плацебо: наивная эвикция → значения расходятся.
//!
//! COMPILE-RED: `Selector.window_ms` ещё нет (task #1). GREEN после эвикции+база (task #2-4).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const WINDOW_MS: i64 = 60_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(price: f64, size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn sel(window_ms: Option<i64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms,
    }
}

fn snap(dir: &std::path::Path, window_ms: Option<i64>, at: Cursor) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(window_ms), at).expect("snapshot")
}

#[test]
fn cvd_base_survives_window_eviction() {
    // Одна UTC-сессия. Сделка A рано (вне окна), B в окне. CVD = running-сумма сессии.
    let t0 = 20_278 * DAY_MS; // начало UTC-дня
    let a = t0 + 10_000;
    let b = t0 + 130_000; // +130s → окно 60s эвиктит бакет A
    let dir = journal_of(vec![
        trade(100.0, 10.0, Side::Buy, a),
        trade(100.0, 5.0, Side::Buy, b),
    ]);
    let full = snap(dir.path(), None, Cursor::LATEST);
    let win = snap(dir.path(), Some(WINDOW_MS), Cursor::LATEST);

    let full_last = full.series.cumulative_delta.last().expect("full cvd").1;
    let win_last = win.series.cumulative_delta.last().expect("win cvd").1;

    // База сохранена: CVD в последнем бакете идентичен полному (окно не обнуляет running-сумму).
    assert_eq!(
        win_last, full_last,
        "CVD-база потеряна при эвикции: окно={win_last} != полный={full_last}; \
         наивная bucket-эвикция обнулила бы running (была бы дельта только бакета B)"
    );
    // Окно действительно обрезало ранние бакеты (иначе тест не про эвикцию).
    assert!(
        win.series.cumulative_delta.len() < full.series.cumulative_delta.len(),
        "окно обязано отбросить ранний бакет A из CVD-серии (окно={}, полный={})",
        win.series.cumulative_delta.len(),
        full.series.cumulative_delta.len()
    );
}

#[test]
fn vp_current_session_whole_not_bucket_windowed() {
    // ≥2 UTC-дня. POC текущей сессии S2 определяется РАННИМ (вне окна) тяжёлым бакетом.
    let s1 = 20_278 * DAY_MS + 10_000; // прошлая сессия — должна эвиктнуться целиком
    let d2 = 20_279 * DAY_MS; // текущая сессия S2
    let s2_early = d2 + 10_000; // вне окна: тяжёлый объём на цене 200 → POC(S2)
    let s2_late = d2 + 200_000; // в окне: лёгкий объём на 210
    let dir = journal_of(vec![
        trade(150.0, 3.0, Side::Buy, s1),
        trade(200.0, 100.0, Side::Buy, s2_early),
        trade(210.0, 1.0, Side::Buy, s2_late),
    ]);
    let full = snap(dir.path(), None, Cursor::LATEST);
    let win = snap(dir.path(), Some(WINDOW_MS), Cursor::LATEST);

    assert!(
        full.series.volume_profile.len() >= 2,
        "предусловие: полный VP содержит ≥2 сессии (S1,S2)"
    );
    let sid1 = full.series.volume_profile.first().expect("S1").session_id;
    let sid2 = full.series.volume_profile.last().expect("S2").session_id;

    let full_s2 = full
        .series
        .volume_profile
        .iter()
        .find(|r| r.session_id == sid2)
        .expect("full S2");
    let win_s2 = win
        .series
        .volume_profile
        .iter()
        .find(|r| r.session_id == sid2)
        .expect("win S2 обязан присутствовать (текущая сессия)");

    // POC текущей сессии считается по ПОЛНОЙ сессии (200), не по окну (210).
    assert_eq!(
        win_s2.poc_e8, full_s2.poc_e8,
        "POC текущей сессии обрезан окном: окно={} != полный={}",
        win_s2.poc_e8, full_s2.poc_e8
    );
    assert_eq!(
        win_s2.poc_e8,
        to_fixed(200.0),
        "POC(S2) обязан быть 200 (тяжёлый ранний бакет), а не 210 (лёгкий в окне)"
    );
    // Прошлая сессия S1 эвиктнута целиком.
    assert!(
        win.series
            .volume_profile
            .iter()
            .all(|r| r.session_id != sid1),
        "прошлая сессия S1 обязана быть эвиктнута из окнового VP"
    );
}

#[test]
fn windowed_live_eq_replay() {
    // VB-I-2 под окном: full(LATEST) ≡ snapshot(C) + frames_since(C..), окно одинаково у обоих.
    let t0 = 20_278 * DAY_MS;
    let mut events = Vec::new();
    // 180 бакетов (180с) > окно 60с → окно РЕАЛЬНО обрезает историю (иначе тест не про окно).
    for i in 0..180i64 {
        events.push(trade(100.0 + i as f64, 1.0, Side::Buy, t0 + i * 1_000));
    }
    let dir = journal_of(events);
    let w = Some(WINDOW_MS);

    let seqs: Vec<u64> = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("ev").seq)
        .collect();
    let c = Cursor::at(seqs[seqs.len() / 2]); // курсор в середине истории

    let full = snap(dir.path(), w, Cursor::LATEST);
    let mut merged = snap(dir.path(), w, c);
    let (frames, _next) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(w),
        c,
        usize::MAX,
    )
    .expect("frames_since");
    for f in &frames {
        merged.apply(f);
    }

    // Окновая серия обязана совпасть побайтно (иначе эвикция/merge несогласованы).
    assert_eq!(
        merged.series, full.series,
        "windowed live != replay: snapshot(C)+frames != full под окном (эвикция/merge/база рассинхронены)"
    );
    // И это действительно окно (обрезано из 180 бакетов истории до ~окна).
    assert!(
        full.series.ohlcv.len() < 180,
        "окно обязано обрезать историю (ohlcv={} из 180 бакетов)",
        full.series.ohlcv.len()
    );
}

#[test]
fn windowed_live_eq_replay_overlap_multistep() {
    // TD-042 (пропуск предыдущего оракула: C был в СЕРЕДИНЕ → окна не пересекались → сдвиг шёл по
    // ПУСТОМУ списку). Здесь C около КОНЦА → удержанное окно snapshot(C) ПЕРЕСЕКАЕТСЯ с финальным
    // [LATEST−W, LATEST]. cumulative_delta АБСОЛЮТЕН → эвикция префикса НЕ должна менять удержанные
    // значения. Плюс multi-step fold (кадры малыми батчами, как штатный live push-loop) → сдвиг
    // КОПИТСЯ на каждом apply. Штатный live даёт пересечение окон ВСЕГДА — это норма, не край.
    let t0 = 20_278 * DAY_MS;
    let mut events = Vec::new();
    for i in 0..180i64 {
        events.push(trade(100.0 + i as f64, 1.0, Side::Buy, t0 + i * 1_000));
    }
    let dir = journal_of(events);
    let w = Some(WINDOW_MS);

    let seqs: Vec<u64> = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("ev").seq)
        .collect();
    // C близко к концу → existing-окно [C−W, C] ПЕРЕСЕКАЕТСЯ с [LATEST−W, LATEST].
    let c = Cursor::at(seqs[seqs.len() - 6]);

    let full = snap(dir.path(), w, Cursor::LATEST);
    let mut merged = snap(dir.path(), w, c);

    // Multi-step: тянем кадры батчами по 2 до сходимости курсора (fold — накопление ошибки).
    let mut cur = c;
    loop {
        let (batch, next) =
            gateway::frames_since(dir.path(), EpochFilter::OwnCaptureOnly, &sel(w), cur, 2)
                .expect("frames_since");
        if batch.is_empty() {
            break;
        }
        for f in &batch {
            merged.apply(f);
        }
        if next == cur {
            break;
        }
        cur = next;
    }

    // Предусловие: окна реально пересеклись (тест про overlap, не про evict-all).
    assert!(
        full.series.cumulative_delta.len() > 5,
        "финальное окно должно удержать пересечение (>5 бакетов)"
    );
    assert_eq!(
        merged.series.cumulative_delta, full.series.cumulative_delta,
        "TD-042: cumulative_delta абсолютен — эвикция префикса под ПЕРЕСЕКАЮЩИМСЯ окном НЕ должна \
         сдвигать удержанные значения (баг: merged сдвинут на сумму эвиктнутых delta, копится по apply)"
    );
    assert_eq!(
        merged.series.cvd_session_base, full.series.cvd_session_base,
        "cvd_session_base расходится merged vs full"
    );
    assert_eq!(
        merged.series, full.series,
        "windowed live != replay под пересекающимся окном + multi-step (TD-042)"
    );
}
