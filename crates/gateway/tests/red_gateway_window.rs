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
const DAY_S: i64 = 86_400;
const WINDOW_MS: i64 = 60_000;

/// M-38a: UTC-сессия из бакет-`time_s` (зеркалит `gateway::utc_session_id` в секундах).
fn session_of(time_s: i64) -> i64 {
    time_s.div_euclid(DAY_S)
}

/// M-38a: per-session base из формы v7 `cvd_session_base: Vec<(session_id, base)>`.
/// Сессия без base-записи трактуется как base=0.
fn session_base(bases: &[(i64, i64)], sid: i64) -> i64 {
    bases
        .iter()
        .find(|(s, _)| *s == sid)
        .map(|(_, b)| *b)
        .unwrap_or(0)
}

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
        depth_cadence_ms: None,
    }
}

fn snap(dir: &std::path::Path, window_ms: Option<i64>, at: Cursor) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(window_ms), at).expect("snapshot")
}

#[test]
fn cvd_base_survives_window_eviction() {
    // Одна UTC-сессия. Сделка A рано (вне окна), B в окне. CVD = running-сумма сессии.
    // M-38a (форма v7): base эвиктнутого бакета A хранится в per-session ledger
    // `cvd_session_base: Vec<(session_id, base)>` ЭТОЙ сессии (не в скаляре).
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
    // Форма v7 (compile-RED против скаляра M-37): base эвиктнутого бакета A (+10) осело в
    // per-session ledger ЭТОЙ сессии; base = сумма эвиктнутых внутрисессионных дельт.
    let s = session_of(a / 1000);
    assert_eq!(
        session_base(&win.series.cvd_session_base, s),
        to_fixed(10.0),
        "base текущей сессии обязан нести дельту эвиктнутого бакета A (+10)"
    );
    // full (без окна, ничего не эвиктнуто) → base сессии = 0.
    assert_eq!(
        session_base(&full.series.cvd_session_base, s),
        0,
        "без окна эвикции нет → per-session base = 0"
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
fn cvd_two_sessions_live_across_midnight_window() {
    // M-38a: окно ЧЕРЕЗ 00:00 UTC → 2 session-ledger элемента живы ОДНОВРЕМЕННО (хвост S1 +
    // голова S2). Каждая сессия — свой running (reset на границе, зеркально VP). Окно режет
    // ТОЛЬКО охват (ранний бакет S1), НЕ session-local значения.
    let d1 = 20_278 * DAY_MS;
    let d2 = 20_279 * DAY_MS; // 00:00 UTC следующего дня
    let w = Some(120_000_i64); // окно 120s
    let dir = journal_of(vec![
        trade(100.0, 8.0, Side::Buy, d1 + 10_000), // S1 ранний (вне окна → эвикт → base S1)
        trade(100.0, 4.0, Side::Buy, d2 - 30_000), // S1 хвост 23:59:30 (в окне [at-120s, at])
        trade(100.0, 3.0, Side::Sell, d2 + 30_000), // S2 голова 00:00:30 (в окне, = at)
    ]);
    let full = snap(dir.path(), None, Cursor::LATEST);
    let win = snap(dir.path(), w, Cursor::LATEST);

    let s1 = session_of(d1 / 1000);
    let s2 = session_of(d2 / 1000);

    // Обе сессии живы в окне (2 ledger-элемента).
    assert!(
        win.series
            .cumulative_delta
            .iter()
            .any(|(t, _)| session_of(*t) == s1),
        "S1-хвост обязан быть жив в окне (23:59:30 внутри [at-120s, at])"
    );
    assert!(
        win.series
            .cumulative_delta
            .iter()
            .any(|(t, _)| session_of(*t) == s2),
        "S2-голова обязана быть жива в окне"
    );

    // S2 running session-local = -3 (НЕ несёт S1 +8/+4). full и win дают одно session-local.
    let win_s2 = win
        .series
        .cumulative_delta
        .iter()
        .rev()
        .find(|(t, _)| session_of(*t) == s2)
        .expect("S2 в окне")
        .1;
    let full_s2 = full
        .series
        .cumulative_delta
        .iter()
        .rev()
        .find(|(t, _)| session_of(*t) == s2)
        .expect("S2 в full")
        .1;
    assert_eq!(
        win_s2,
        -to_fixed(3.0),
        "S2 running session-local = -3 (reset на 00:00 UTC)"
    );
    assert_eq!(
        win_s2, full_s2,
        "session-local S2 не зависит от окна (окно меняет охват, не значение)"
    );

    // Форма v7: per-session base. Ранний бакет S1 (+8) эвиктнут → осел в base ЭТОЙ сессии S1.
    // S2 эвикции не было → base S2 = 0.
    assert_eq!(
        session_base(&win.series.cvd_session_base, s1),
        to_fixed(8.0),
        "base S1 = сумма эвиктнутых ранних бакетов S1 (+8)"
    );
    assert_eq!(
        session_base(&win.series.cvd_session_base, s2),
        0,
        "base S2 = 0 (голова сессии, эвикции внутри S2 не было)"
    );
    // Хвост S1 (23:59:30, в окне) удержан: его running session-local = base(S1)+8+4 = 8+4=... нет,
    // running S1 = base(эвиктнут +8) применён к удержанному бакету (+4) → 8+4 = 12 (непрерывность
    // сессии под окном, зеркально M-37 cvd_base_survives).
    let win_s1 = win
        .series
        .cumulative_delta
        .iter()
        .rev()
        .find(|(t, _)| session_of(*t) == s1)
        .expect("S1-хвост в окне")
        .1;
    assert_eq!(
        win_s1,
        to_fixed(12.0),
        "S1-хвост running = base(+8) + удержанный бакет(+4) = +12 (непрерывность S1 под окном)"
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
    //
    // M-38a: поток ПЕРЕСЕКАЕТ 00:00 UTC (180 бакетов вокруг границы: 90 в S1, 90 в S2) → merge
    // per-session с reset. Overlap-fold обязан удержать session-local значения S2 (не накопленные
    // через границу) БАЙТ-идентично между live и replay.
    //
    // C-028 K2: КУРСОР У ГРАНИЦЫ (не в глубине S2). snapshot(C) существующее окно содержит ХВОСТ S1
    // + ГОЛОВУ S2; финальное окно уходит целиком в S2 → S1 должна whole-dropped'иться НА ПУТИ MERGE
    // (existing пересекает финальное окно — testing.md §vantage). Явные pre/post-asserts вокруг fold'а
    // ловят реализацию, которая роняет прошлую сессию в `Reducer::finish`, но НЕ в bundle-merge.
    let d2 = 20_279 * DAY_MS;
    let t0 = d2 - 90_000; // старт за 90s до 00:00 UTC → i=0..89 в S1, i=90..179 в S2
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
    // C-028 K2: курсор У ГРАНИЦЫ (не в глубине S2). Время C = d2+35s → окно snapshot(C) =
    // [d2−25s, d2+35s] содержит ХВОСТ S1 (23:59:35..59) + ГОЛОВУ S2 (00:00:00..35). Финальное окно
    // [d2+29s, d2+89s] целиком в S2 → S1 обязана быть whole-dropped на пути merge (существующее
    // состояние ПЕРЕСЕКАЕТ финальное окно — testing.md §vantage). Это давит ИМЕННО на
    // bundle-merge/`evict_series_bundle_under_window`: реализация может корректно ронять прошлую
    // сессию в `Reducer::finish` для snapshot(LATEST), но оставить хвост S1 в existing при apply.
    let c_idx = ((d2 + 35_000 - t0) / 1_000) as usize; // = 125 (событие на времени d2+35s)
    let c = Cursor::at(seqs[c_idx]);

    let full = snap(dir.path(), w, Cursor::LATEST);
    let mut merged = snap(dir.path(), w, c);

    // Session-reset на full: финальное окно целиком в S2; последний бакет = 90-й бакет S2 → +90
    // (session-local, reset на 00:00 UTC). Текущий single-running дал бы +180 (S1+S2 сквозняком).
    let s1 = session_of(t0 / 1000);
    let s2 = session_of((t0 + 179_000) / 1000);
    assert_eq!(
        full.series.cumulative_delta.last().expect("cvd непуст").1,
        to_fixed(90.0),
        "S2 session-local: последний бакет = +90 (reset на 00:00 UTC), НЕ +180 через границу"
    );
    // Прошлая сессия S1 dropped целиком (не осела в base) — зеркально whole-session VP эвикции.
    assert_eq!(
        session_base(&full.series.cvd_session_base, s1),
        0,
        "прошлая сессия S1 dropped целиком (не в base)"
    );
    // base S2 = сумма эвиктнутых ранних S2-бакетов (i=90..118, +0..+28s → 29 бакетов по +1 = +29).
    assert_eq!(
        session_base(&full.series.cvd_session_base, s2),
        to_fixed(29.0),
        "base S2 = 29 эвиктнутых ранних бакетов текущей сессии"
    );

    // PRE-fold vantage (C-028 K2): existing-состояние merged (= snapshot(C)) обязано СОДЕРЖАТЬ S1
    // (хвост в окне [d2−25s, d2+35s]) — иначе whole-drop нечему тестировать. full же уже БЕЗ S1.
    assert!(
        merged
            .series
            .cumulative_delta
            .iter()
            .any(|(t, _)| session_of(*t) == s1),
        "pre-fold: snapshot(C) у границы обязан нести хвост S1 в cumulative_delta (иначе overlap \
         с финальным окном не пересекает прошлую сессию → whole-drop не под тестом)"
    );
    assert!(
        !full
            .series
            .cumulative_delta
            .iter()
            .any(|(t, _)| session_of(*t) == s1),
        "pre-fold: full(LATEST) обязан быть БЕЗ строк S1 (финальное окно целиком в S2)"
    );
    assert_eq!(
        session_base(&full.series.cvd_session_base, s1),
        0,
        "pre-fold: base S1 в full = 0 (сессия whole-dropped, не в base)"
    );

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

    // POST-fold whole-drop (C-028 K2): S1, живая в existing до fold'а, обязана исчезнуть ЦЕЛИКОМ —
    // ни строки в cumulative_delta, ни базы в ledger. Наивный bundle-merge, роняющий только префикс
    // ТЕКУЩЕЙ сессии (а не whole прошлой), оставил бы хвост S1 → это assert поймает.
    assert!(
        !merged
            .series
            .cumulative_delta
            .iter()
            .any(|(t, _)| session_of(*t) == s1),
        "post-fold: S1 обязана быть whole-dropped из merged (bundle-merge не уронил прошлую сессию \
         из existing под финальным окном)"
    );
    assert_eq!(
        session_base(&merged.series.cvd_session_base, s1),
        0,
        "post-fold: base S1 в merged = 0 (whole-drop, не осела в per-session ledger)"
    );

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
        "windowed live != replay под пересекающимся окном (у границы 00:00 UTC) + multi-step (TD-042/K2)"
    );
}

#[test]
fn windowed_live_eq_replay_past_session_survives_overlap() {
    // TD-045 — ПАРНЫЙ vantage к whole-drop (overlap_multistep): fold ОСТАНАВЛИВАЕТСЯ, пока финальное
    // окно ЕЩЁ ПЕРЕСЕКАЕТ прошлую сессию S1 → S1 обязана УЦЕЛЕТЬ в merged (== full). Односторонний
    // оракул K2 пинует только «S1 dropped» (финальное окно целиком в S2); эта дыра дала регрессию:
    // merge дропает VP-сессию по `session_id < utc_session_id(at)`, а не по оконному критерию редьюсера
    // (`session_max_time_s[sid] < lo`) → роняет S1 сразу после 00:00, хотя окно её ещё держит.
    // Анти-плацебо: падает на текущем merge (merged.vp теряет S1), GREEN только когда merge применяет
    // ИДЕНТИЧНЫЙ оконный критерий (per-session max_time_s в bundle).
    let d2 = 20_279 * DAY_MS;
    let t0 = d2 - 90_000; // i=0..89 в S1, i=90..179 в S2
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
    // C у границы (d2+35s); finalize at d2+45s → финальное окно [d2−15s, d2+45s] ЕЩЁ пересекает S1.
    let c_idx = ((d2 + 35_000 - t0) / 1_000) as usize; // 125
    let at_idx = ((d2 + 45_000 - t0) / 1_000) as usize; // 135
    let c = Cursor::at(seqs[c_idx]);
    let at_final = Cursor::at(seqs[at_idx]);

    let full = snap(dir.path(), w, at_final);
    let mut merged = snap(dir.path(), w, c);
    // fold РОВНО (at_idx − c_idx) событий (126..=135) → merged.at = d2+45s, то же окно, что у full.
    let (frames, _next) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(w),
        c,
        at_idx - c_idx,
    )
    .expect("frames_since");
    for f in &frames {
        merged.apply(f);
    }

    let s1 = session_of(t0 / 1000);
    let s2 = session_of((t0 + 179_000) / 1000);

    // Предусловие: финальное окно ПЕРЕСЕКАЕТ S1 → full.vp содержит ОБЕ сессии (иначе тест не про survive).
    let full_vp: Vec<i64> = full
        .series
        .volume_profile
        .iter()
        .map(|r| r.session_id)
        .collect();
    assert!(
        full_vp.contains(&s1) && full_vp.contains(&s2),
        "предусловие: финальное окно [d2−15s,d2+45s] пересекает S1 → full.vp = обе сессии (={full_vp:?})"
    );

    // TD-045: S1 обязана УЦЕЛЕТЬ в merged, пока окно её пересекает.
    let merged_vp: Vec<i64> = merged
        .series
        .volume_profile
        .iter()
        .map(|r| r.session_id)
        .collect();
    assert!(
        merged_vp.contains(&s1),
        "TD-045: S1 обязана уцелеть в merged, пока финальное окно её пересекает (merge уронил её по \
         session_id<utc_session_id(at), а не по оконному критерию) — merged.vp={merged_vp:?}"
    );

    // Байт-идентичность live==replay (VB-I-2): VP полностью совпадает.
    assert_eq!(
        merged.series.volume_profile, full.series.volume_profile,
        "TD-045: VP merged != full под пересекающимся окном — whole-session drop на merge не совпадает \
         с оконным критерием редьюсера"
    );
    // И вся свёртка байт-идентична (CVD-часть уже принята; тут пинуем именно VP-регрессию в составе).
    assert_eq!(
        merged.series, full.series,
        "TD-045: windowed live != replay при пересечении окном прошлой сессии (VP whole-drop merge)"
    );
}
