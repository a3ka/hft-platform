//! RED M-88 (sacred, architect-only) — **список наблюдённых колонок обязан быть ОКОННЫМ.**
//!
//! Предмет поставки M-88 (снятая ликвидность исчезает и у клиента) решён и принят
//! (`R-195`, блок «Что ПРИНЯТО без замечаний»). Этот набор — не про него, а про ЦЕНУ,
//! которой он оплачен: новое состояние редьюсера `heatmap_buckets_observed` росло с
//! ИСТОРИЕЙ журнала, а не с окном, и на прод-глубине физически ломало выдачу.
//!
//! **Находка `R-195` Б-1, воспроизведённая ревьюером ИСПОЛНЕНИЕМ:**
//! окно 60 с, история 180 с → в ответе объявлено 180 наблюдённых колонок; на журнале
//! в 220 000 секундных бакетов (≈2.5 суток) `snapshot` возвращает `Err` по пределу
//! `PL-I-5` (2 423 152 Б при лимите 2 000 000 Б) — причём все содержательные счётчики
//! в сообщении нулевые: весь объём ответа занимает сам список. Прод несёт два месяца
//! истории ⇒ после выкладки кокпит не получил бы снимок ВООБЩЕ. Это хуже чинимого
//! дефекта: призрак хотя бы виден, молчание — нет.
//!
//! **`VB-I-10`** (`docs/fa/viz-backend.md:208`) сформулирован ровно против этого: память
//! `snapshot`/`frames_since` ограничена ОКНОМ `[at−W, at]`, **не числом time-бакетов
//! истории**. Находка — прямой его регресс.
//!
//! **Почему это не поймал ни один существующий оракул — замер `R-195`, а не догадка.**
//! `red_gateway_bounded.rs` гоняет 20 000 бакетов при порогах 8 МиБ/2 МиБ: рост в 220 КБ
//! на порядок ниже обоих, сигнала нет; его пер-серийные ассерты перечисляют поля ПОИМЁННО,
//! и нового поля в перечне нет. Оракулы самого M-88 слепы по построению:
//! `red_gateway_live_eq_replay::sel()` несёт `window_ms: None`, а
//! `red_gateway_window::windowed_live_eq_replay` окно имеет, но его фикстура состоит ИЗ
//! ОДНИХ СДЕЛОК — ни одного L2-события, поэтому список наблюдений пуст в ОБЕИХ
//! сравниваемых сторонах. Пересечение {окно} × {L2 на входе} × {ассерт по наблюдениям}
//! было ПУСТЫМ. Этот файл — ровно это пересечение.
//!
//! **Анти-плацебо.** Все сценарии ниже КРАСНЫ против кода на `e14d299` и зелены только
//! после того, как список подчинён окну ОДИНАКОВО на обоих путях. Сценарий `w2` —
//! сторож соседнего инварианта (`testing.md`, второй вопрос мутационного контроля):
//! он краснеет, если сужение сделано ТОЛЬКО на одном из путей, то есть если починка
//! ресурса куплена ценой `VB-I-2`.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{ApplyOutcome, Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const BASE_TS: i64 = 1_752_000_000_000;
const WINDOW_MS: i64 = 60_000;
/// История ВТРОЕ длиннее окна: окно обязано реально резать, иначе сценарий вырожден.
const HISTORY_BUCKETS: i64 = 180;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

/// L2-снимок книги: две стороны по четыре уровня. Уровни МЕНЯЮТСЯ от бакета к бакету
/// (`jitter`), иначе редьюсер мог бы схлопнуть одинаковые наблюдения и сценарий судил бы
/// не то.
fn book(jitter: i64, ts: i64) -> EventKind {
    let mk = |base: i64| -> Vec<Level> {
        (0..4)
            .map(|k| Level {
                price: base + k * 100,
                size: 1_000 + k + jitter,
            })
            .collect()
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: mk(6_400_000_000_000),
            asks: mk(6_400_100_000_000),
            ts_exch_ms: ts,
        },
    )
}

fn trade(ts: i64, i: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(64_000.0),
            size: to_fixed(1.0),
            side: [Side::Buy, Side::Sell][(i % 2) as usize],
            ts_exch_ms: ts,
        },
    )
}

/// Журнал: на КАЖДЫЙ секундный бакет — L2-событие (даёт наблюдение карты) и сделка
/// (даёт `ohlcv`, по которому сценарий вычисляет фактическую границу окна).
fn journal_with_l2_per_bucket(buckets: i64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for b in 0..buckets {
            let ts = BASE_TS + b * 1_000;
            j.append(book(b, ts)).expect("append book");
            j.append(trade(ts, b)).expect("append trade");
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

/// Фактическая нижняя граница окна, снятая С ДАННЫХ (последний бакет `ohlcv` минус окно),
/// а не вычисленная повторением формулы реализации: оракул, повторяющий формулу
/// проверяемого кода, проверяет сам себя.
fn window_bounds(s: &gateway::SeriesBundle) -> (i64, i64) {
    let hi = s
        .ohlcv
        .last()
        .expect("ohlcv непуст — фикстура даёт сделку в каждом бакете")
        .time_s;
    (hi - WINDOW_MS / 1_000, hi)
}

// ═══════════════ W1 — путь редьюсера: список следует за ОКНОМ, не за историей ═══════════════

#[test]
fn w1_observed_list_is_window_bounded_not_history() {
    let dir = journal_with_l2_per_bucket(HISTORY_BUCKETS);
    let s = snap(dir.path(), Some(WINDOW_MS), Cursor::LATEST).series;

    // SETUP-СТРАЖ: окно обязано РЕАЛЬНО резать историю. Без этой проверки сценарий,
    // выродившийся в «история короче окна», был бы зелёным, ничего не проверяя.
    assert!(
        (s.ohlcv.len() as i64) < HISTORY_BUCKETS,
        "setup-страж: окно не обрезало историю (ohlcv={} из {HISTORY_BUCKETS}) — сценарий \
         вырожден, и всё ниже проверяет не то",
        s.ohlcv.len()
    );
    assert!(
        !s.heatmap_observed_time_s.is_empty(),
        "setup-страж: список наблюдений ПУСТ — фикстура не подала ни одного L2-события на \
         путь карты, и сценарий снова судил бы пустоту (именно так его пропустили оракулы \
         M-88: их фикстуры состояли из одних сделок)"
    );

    let (lo, hi) = window_bounds(&s);
    let outside: Vec<i64> = s
        .heatmap_observed_time_s
        .iter()
        .copied()
        .filter(|&t| t < lo || t > hi)
        .collect();
    assert!(
        outside.is_empty(),
        "VB-I-10 (R-195 Б-1): объявлено {} наблюдённых колонок ВНЕ окна [{lo}, {hi}] \
         (всего в списке {}, первые вне окна: {:?}). Величина следует за ДЛИНОЙ ИСТОРИИ — \
         ровно то, что VB-I-10 запрещает дословно: память ограничена окном, НЕ числом \
         time-бакетов истории",
        outside.len(),
        s.heatmap_observed_time_s.len(),
        &outside[..outside.len().min(5)]
    );

    // Колонка, ОБЪЯВЛЕННАЯ наблюдённой, обязана быть колонкой, которую клиент способен
    // заместить: то есть лежать в том же окне, что и сама карта. Иначе объявление
    // бессмысленно по существу, а не только дорого по объёму.
    assert!(
        s.heatmap_observed_time_s.len() <= (WINDOW_MS / 1_000) as usize + 2,
        "список наблюдений ({}) шире окна ({} бакетов + допуск на граничный)",
        s.heatmap_observed_time_s.len(),
        WINDOW_MS / 1_000
    );
}

// ═══ W2 — СТОРОЖ СОСЕДНЕГО ИНВАРИАНТА: сужение обязано быть ОДИНАКОВЫМ на обоих путях ═══

#[test]
fn w2_fold_equals_replay_on_observed_list() {
    // `R-195`, «Граница роли»: обе стороны сегодня СОГЛАСОВАНЫ (склейка=180, пересчёт=180),
    // поэтому `VB-I-2` цел. Любое сужение списка обязано быть сделано ОДИНАКОВО на живом
    // пути и на пути пересчёта — иначе починка ресурса сломает предмет поставки, и
    // снятая ликвидность вернётся на экран. Этот сценарий краснеет ровно на такую починку.
    let dir = journal_with_l2_per_bucket(HISTORY_BUCKETS);
    let w = Some(WINDOW_MS);

    let seqs: Vec<u64> = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("ev").seq)
        .collect();
    let c = Cursor::at(seqs[seqs.len() / 2]);

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
    assert!(
        !frames.is_empty(),
        "setup-страж: кадров нет — фолду нечего применять"
    );
    for f in &frames {
        assert_eq!(
            merged.apply(f),
            ApplyOutcome::Applied,
            "кадр обязан быть принят: он продолжает курсор потребителя"
        );
    }

    assert_eq!(
        merged.series.heatmap_observed_time_s, full.series.heatmap_observed_time_s,
        "VB-I-2: список наблюдений у КЛИЕНТА (снимок+кадры) разошёлся с ПЕРЕСЧЁТОМ. \
         Сужение сделано только на одном из путей — значит клиент либо получит колонку, \
         которую не объявили наблюдённой (снятая ликвидность останется на экране — тот \
         самый дефект, ради которого затеян M-88), либо наоборот сотрёт живые данные"
    );
    // И весь бандл целиком: список — не единственное, что merge мог расстроить.
    assert_eq!(
        merged.series, full.series,
        "VB-I-2: окновая серия склейки != пересчёту"
    );
}

// ═══════════ W3 — накопление у клиента: список не растёт с числом применённых кадров ═══════════

#[test]
fn w3_client_accumulation_stays_window_bounded() {
    // Клиент применяет кадры ПОДРЯД, малыми батчами, как штатный live-push. Накопленный
    // список наблюдений обязан оставаться оконным: `Snapshot::apply` только ДОБАВЛЯЕТ в
    // него (`:2982-2987`), и без эвикции долгоживущая сессия кокпита копит список вечно —
    // тот же дефект, что у редьюсера, только на другой стороне провода.
    let dir = journal_with_l2_per_bucket(HISTORY_BUCKETS);
    let w = Some(WINDOW_MS);

    let seqs: Vec<u64> = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("ev").seq)
        .collect();
    let c = Cursor::at(seqs[10]); // курсор В НАЧАЛЕ истории → кадров много, окна расходятся
    let mut client = snap(dir.path(), w, c);

    let mut cursor = c;
    let mut steps = 0usize;
    loop {
        let (frames, next) = gateway::frames_since(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &sel(w),
            cursor,
            8, // малыми батчами — накопление копится на каждом apply
        )
        .expect("frames_since");
        if frames.is_empty() {
            break;
        }
        for f in &frames {
            assert_eq!(
                merged_apply(&mut client, f),
                ApplyOutcome::Applied,
                "кадр продолжает курсор"
            );
        }
        cursor = next;
        steps += 1;
        assert!(steps < 1_000, "setup-страж: фолд не сходится");
    }
    assert!(
        steps > 3,
        "setup-страж: батчей вышло {steps} — накоплению негде копиться"
    );

    let (lo, hi) = window_bounds(&client.series);
    let outside: Vec<i64> = client
        .series
        .heatmap_observed_time_s
        .iter()
        .copied()
        .filter(|&t| t < lo || t > hi)
        .collect();
    assert!(
        outside.is_empty(),
        "накопленный у клиента список наблюдений держит {} колонок вне окна [{lo}, {hi}] \
         (всего {}). Долгоживущая сессия кокпита копила бы его неограниченно",
        outside.len(),
        client.series.heatmap_observed_time_s.len()
    );
}

/// Обёртка ради явного наблюдения исхода (`§6` спеки запрещает `let _ = apply(..)`).
fn merged_apply(s: &mut gateway::Snapshot, f: &gateway::Frame) -> ApplyOutcome {
    s.apply(f)
}

// ═══ W4 — КАНАРЕЙКА КЛАССА: новое поле бандла нельзя завести, не решив вопрос об окне ═══

/// **Это не про `heatmap_observed_time_s`, а про КЛАСС, которым он проехал.**
///
/// Находка `R-195` Б-1 стала возможна потому, что оба сторожа окна перечисляют поля
/// ПОИМЁННО: новое поле в перечне не появляется само, и ни один гейт не спрашивает автора,
/// оконное оно или нет. Порог по объёму этот пробел не закрывает — рост в 220 КБ лежит на
/// порядок ниже любого разумного порога и всё равно ломает выдачу на проде.
///
/// Канарейка разбирает `SeriesBundle` ИСЧЕРПЫВАЮЩЕ — без `..`. Новое поле ⇒ **ошибка
/// компиляции здесь**, и автор обязан отнести его к одному из двух списков ниже. Это тот
/// же приём, которым `CT-I-*` держит `EventKind`: заставить решение быть ПРИНЯТЫМ, а не
/// забытым. Канарейка не проверяет истинность отнесения — это названный предел; она
/// проверяет, что вопрос ЗАДАН.
#[test]
fn w4_every_bundle_field_is_classified_as_windowed_or_not() {
    let dir = journal_with_l2_per_bucket(HISTORY_BUCKETS);
    let bundle = snap(dir.path(), Some(WINDOW_MS), Cursor::LATEST).series;
    let (lo, hi) = window_bounds(&bundle);

    let gateway::SeriesBundle {
        // ── БАКЕТ-КЛЮЧЕВЫЕ: обязаны лежать в окне [lo, hi] ──
        ohlcv,
        cumulative_delta,
        depth_series,
        vwap,
        heatmap,
        volume_bubbles,
        heatmap_observed_time_s,
        // ── НЕ БАКЕТ-КЛЮЧЕВЫЕ: окну не подчиняются, и почему ──
        // `cvd_session_base`/`vp_session_max_time_s` — per-СЕССИЯ (эвикция whole-session,
        // M-38a); `volume_profile` — бины цены текущей сессии, не бакеты времени;
        // `cob` — срез книги на `at`, одна точка; `cadence_ms` — метаданные селектора;
        // `cob_observed` — скаляр; прочее — скаляры/провенанс.
        cvd_session_base: _,
        vp_session_max_time_s: _,
        volume_profile: _,
        cob: _,
        cadence_ms: _,
        cob_observed: _,
    } = &bundle;

    let out_of_window = |name: &str, ts: Vec<i64>| {
        let bad: Vec<i64> = ts.into_iter().filter(|&t| t < lo || t > hi).collect();
        assert!(
            bad.is_empty(),
            "{name}: {} точек вне окна [{lo}, {hi}] (первые: {:?}) — VB-I-10",
            bad.len(),
            &bad[..bad.len().min(5)]
        );
    };

    out_of_window("ohlcv", ohlcv.iter().map(|r| r.time_s).collect());
    out_of_window(
        "cumulative_delta",
        cumulative_delta.iter().map(|&(t, _)| t).collect(),
    );
    out_of_window("vwap", vwap.iter().map(|&(t, _)| t).collect());
    out_of_window("heatmap", heatmap.iter().map(|c| c.time_s).collect());
    out_of_window(
        "volume_bubbles",
        volume_bubbles.iter().map(|c| c.time_s).collect(),
    );
    out_of_window("heatmap_observed_time_s", heatmap_observed_time_s.clone());
    for row in depth_series {
        out_of_window(
            "depth_series[].series",
            row.series.iter().map(|&(t, _)| t).collect(),
        );
    }
}
