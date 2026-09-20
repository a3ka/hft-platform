//! `M-85` — `VB-I-2` на пути `frames_since`: кадр обязан строиться от КНИГИ, а не от пустой.
//!
//! # Предмет — половина `TD-199`, которую `M-77` НЕ закрывал, и это объявлено им самим
//!
//! `M-77` закрыл `TD-199` для пути `pump`: кадр берётся срезом ЖИВОГО редьюсера
//! (`Reducer::book_series_in`), у которого книга есть. Вызовы `book_series_in` — ровно два, и
//! оба внутри `pump`; шапка `Reducer::set_capture_book_observations` объявляет границу прямо:
//! «Включается ТОЛЬКО в `LiveReducer::resume` … Свежие batch'и в `pump` и Reducer'ы в
//! snapshot/checkpoint/replay НЕ включают».
//!
//! `frames_since` идёт ДРУГИМ путём и остаётся сломанным:
//!
//! ```text
//! frames_since → journal::stream(dir, filter)              ← ПОЛНОЕ чтение С ГОЛОВЫ (:2974)
//!              → reduce_event_stream(…)                    ← общая часть обоих API (:2226)
//!              → let mut reducer = Reducer::new(selector)  ← КНИГА ПУСТА (:2233)
//!                события `seq <= after` → seed_vwap(…)     ← книгу НЕ трогают (:2253-2255)
//! ```
//!
//! Затравочные события в потоке ЕСТЬ и до редьюсера ДОХОДЯТ — но `seed_vwap` книгу не трогает,
//! а депт-серия считается ИЗ КНИГИ. На дельта-хвосте без якорного снимка в окне книга пуста ⇒
//! точки ряда либо не рождаются, либо рождаются с другим содержимым; полный реплей книгу имеет
//! ⇒ пути расходятся, и `VB-I-2` нарушен. Seek-вариант `frames_since_with_stats` — ОТДЕЛЬНЫЙ
//! API вне предмета (`M-85` §2bis.2).
//!
//! # Почему это ДЕФЕКТ, а не «путь никому не нужен»
//!
//! Прод им действительно не пользуется: у обёртки `serve::frames_msgs` НОЛЬ вызовов, push идёт
//! через `LiveReducer::pump` с `M-53`/`TD-083`. Но:
//!
//! 1. `frames_since` ЭКСПОРТИРОВАН и служит ЭТАЛОНОМ сверки в 17 тестовых файлах. Кривой
//!    эталон портит каждый оракул, который на него обопрётся, — `testing.md`
//!    §«Зависимый эталон мутация ловит плохо».
//! 2. `VB-I-2` сформулирован НЕ про конкретный путь: два способа собрать одно состояние обязаны
//!    сойтись. Сегодня не сходятся.
//! 3. `gates.md` §8 — красные тесты в `main` не живут, и это блокирует `M-70`.
//!
//! # ПОЧЕМУ НУЖЕН НОВЫЙ ОРАКУЛ, а не «тот, что уже красен» (`A-028` §1: предшественник ищется)
//!
//! `gw_i_4_holds_when_the_tail_frame_is_delta_only` (`red_depth_provenance_by_reach.rs`) красен
//! на ветке `docs/M-70-rev2` и ЗЕЛЁН в `main` — замер 2026-09-16 на `95d2422`:
//! `cargo test -p gateway --test red_depth_provenance_by_reach` → `9 passed; 0 failed`.
//! Причина слепоты названа в `R-171` Б-2: он сравнивает ОДНУ метку СТРОКИ, а расходится НАБОР
//! ТОЧЕК. Различающим он становится только вместе с формой `M-70` (метка на точку).
//!
//! Строить `M-85` на оракуле, который краснеет лишь в присутствии другого невлитого предмета, —
//! значит поставить фикс в зависимость от `M-70`, тогда как зависимость обязана идти ОБРАТНО.
//! Поэтому мера снимается с того, что расходится САМО: с РЯДА ТОЧЕК `(time_s, depth_e8)`.
//!
//! # Мера — РЯД ТОЧЕК, и это выбрано замером, а не вкусом
//!
//! `R-172` Н-1 предъявил сырое расхождение: реплей — 4 точки, клиент — 3. Метки при этом
//! совпадали поэлементно на том, что дожило. Значит различитель — ключи и значения ряда, а не
//! подпись достоверности; оракул на подписи был бы зелёным против работающего дефекта.
//!
//! # Анти-плацебо — в ОБЕ стороны
//!
//! · `m85_1` падает против СЕГОДНЯШНЕГО кода (кадр без книги);
//! · `m85_2` — СТОРОЖ: на журнале, где окно кадра СОДЕРЖИТ якорный снимок, пути обязаны
//!   сходиться УЖЕ СЕЙЧАС. Он зелен в `main` и обязан остаться зелёным после фикса — иначе
//!   «фикс» сломал бы то, что работало (ложное КРАСНОЕ на честной работе опаснее пропуска);
//! · `m85_3` — SETUP-СТРАЖ: предъявляет, что хвостовой кадр действительно НЕ несёт якоря.
//!   Без него `m85_1` мог бы молча тестировать не тот сценарий (`testing.md`, целостность
//!   гейта, свойство 3).
//!
//! # Что этот набор НЕ проверяет — названо, а не умолчано
//!
//! Он не судит ЦЕНУ развязки. Бутстрап книги до `after` может стоить `O(журнал)` на вызов —
//! ровно та цена, от которой `M-53`/`TD-083` увели прод. Оракул границы ресурса — отдельная
//! задача спеки (`M-85` §3 задача 3), и он обязан мерить РЕСУРС (посещённые события), а не
//! время: время зависит от хоста и дало бы флак.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, DepthRow, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_752_000_010_000;
const MID: f64 = 65_000.0;

/// Полоса, на которой судится предмет: ШИРЕ узкого ресинк-снимка и УЖЕ живой книги.
const BAND: f64 = 0.02;

/// Окно heatmap процессно-глобально (`set_effective_heatmap_window_frac`) ⇒ тесты,
/// зависящие от охвата, идут под одним замком. `C-201` B-6.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
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

fn lvls(v: &[(f64, f64)]) -> Vec<Level> {
    v.iter()
        .map(|&(p, s)| Level {
            price: to_fixed(p),
            size: to_fixed(s),
        })
        .collect()
}

/// Якорный снимок с охватом `reach_pct` по обеим сторонам.
fn book_at(reach_pct: f64, ts: i64) -> EventKind {
    let mut bids = vec![(MID - 1.0, 5.0)];
    let mut asks = vec![(MID + 1.0, 5.0)];
    for k in [0.5_f64, 1.0_f64] {
        bids.push((MID * (1.0 - reach_pct * k), 1.0));
        asks.push((MID * (1.0 + reach_pct * k), 1.0));
    }
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lvls(&bids),
            asks: lvls(&asks),
            ts_exch_ms: ts,
        },
    )
}

fn trade_at(price: f64, size: f64, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side: Side::Buy,
            ts_exch_ms: ts,
        },
    )
}

fn delta_at(bids: &[(f64, f64)], asks: &[(f64, f64)], ts: i64, uid: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: lvls(bids),
            asks: lvls(asks),
            first_update_id: uid,
            final_update_id: uid,
            prev_final_update_id: None,
            ts_exch_ms: ts,
        },
    )
}

fn sel() -> Selector {
    // Серверное окно карты — как у `M-75`: величина берётся не из полос, а из настройки.
    gateway::set_effective_heatmap_window_frac(0.10);
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001, BAND, 0.10],
        window_ms: None,
        depth_cadence_ms: None,
    }
}

/// Ряд точек полосы как ПАРЫ `(time_s, depth_e8)` — предмет сверки.
fn series_of(rows: &[DepthRow], side: &str) -> Vec<(i64, i64)> {
    let want = (BAND * 1e8).round() as i64;
    rows.iter()
        .find(|r| r.side == side && r.band_pct_e8 == want)
        .map(|r| r.series.clone())
        .unwrap_or_default()
}

/// Журнал, чей ХВОСТ — дельта: якорь есть в начале, в последнем окне кадра его НЕТ.
///
/// Это прод-форма (`DESIGN` §17: дельты 100 мс против якоря 1 Гц, а после `M-45` якорь станет
/// в 10–30 раз реже), и именно она порождает «кадр без книги».
fn journal_with_delta_tail() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    j.append(book_at(0.05, T)).expect("a1"); // якорь: книга широкая
    j.append(delta_at(
        &[(MID * 0.95, 0.0)],
        &[(MID * 1.05, 0.0)],
        T + 1_000,
        1,
    ))
    .expect("a2"); // дельта срезает дальние уровни
    j.append(book_at(0.005, T + 2_000)).expect("a3"); // ресинк: книга узкая (окно REST)
    j.append(delta_at(
        &[(MID * 0.95, 2.0)],
        &[(MID * 1.05, 2.0)],
        T + 3_000,
        2,
    ))
    .expect("a4"); // ХВОСТ — дельта, вернувшая дальние уровни
    j.flush().expect("flush");
    dir
}

/// Собрать состояние КАК КЛИЕНТ: `snapshot(START)` + дренаж `frames_since` по одному событию.
fn assemble_via_frames(dir: &std::path::Path, s: &Selector) -> (gateway::Snapshot, usize) {
    let mut merged = gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, s, Cursor::START)
        .expect("snapshot(START)");
    let mut cur = Cursor::START;
    let mut n = 0_usize;
    loop {
        let (batch, next) = gateway::frames_since(dir, EpochFilter::OwnCaptureOnly, s, cur, 1)
            .expect("frames_since");
        if batch.is_empty() {
            break;
        }
        for f in &batch {
            merged.apply(f);
            n += 1;
        }
        assert!(
            next > cur,
            "GW-I-8: курсор не монотонен ({next:?} <= {cur:?})"
        );
        cur = next;
    }
    (merged, n)
}

/// SETUP-СТРАЖ. Предъявляет, что предмет ВОСПРОИЗВЕДЁН: кадров несколько, и ПОСЛЕДНИЙ из них
/// построен на дельте, то есть якоря в своём окне не имеет.
///
/// Без этого стража `m85_1` мог бы оказаться зелёным по причине «журнал свернулся в один кадр»
/// и молча тестировать не тот сценарий — плацебо самого себя.
#[test]
fn m85_3_setup_guard_tail_frame_is_delta_only() {
    let _g = serial();
    let dir = journal_with_delta_tail();
    let s = sel();
    let (_merged, n_frames) = assemble_via_frames(dir.path(), &s);
    assert!(
        n_frames >= 4,
        "SETUP НЕ СОСТОЯЛСЯ: кадров {n_frames} при четырёх событиях — дельта-хвост не пришёл \
         ОТДЕЛЬНЫМ кадром, и предмет (кадр без якоря в своём окне) не воспроизведён"
    );
    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot(LATEST)");
    assert!(
        !series_of(&full.series.depth_series, "bid").is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: полный реплей не дал НИ ОДНОЙ точки полосы {BAND} — сверять нечего, \
         и `m85_1` сравнивал бы пустоту с пустотой"
    );
}

/// ЯДРО. `VB-I-2` на пути `frames_since`: клиент, собравший состояние из `snapshot(C) + frames`,
/// обязан получить ТОТ ЖЕ ряд точек, что полный реплей того же окна.
///
/// КРАСЕН в `main` (`95d2422`) по построению: кадр строится свежим `Reducer::new` с пустой
/// книгой, и точка, рождающаяся из книги, в него не попадает.
#[test]
fn m85_1_client_assembled_series_equals_full_replay_on_delta_tail() {
    let _g = serial();
    let dir = journal_with_delta_tail();
    let s = sel();

    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot(LATEST)");
    let (merged, _n) = assemble_via_frames(dir.path(), &s);

    for side in ["bid", "ask"] {
        let want = series_of(&full.series.depth_series, side);
        let got = series_of(&merged.series.depth_series, side);
        assert_eq!(
            got, want,
            "VB-I-2 НАРУШЕН на пути `frames_since`: ряд точек депт-серии, собранный клиентом \
             (`snapshot(C) + frames`), не равен полному реплею того же окна. side={side}, \
             полоса={BAND}.\n  реплей: {want:?}\n  клиент: {got:?}\n\
             Причина структурная: `frames_since` отдаёт окно СВЕЖЕМУ `Reducer::new` \
             (`reduce_event_stream`), а затравочные события `seq <= after` идут только в \
             `seed_vwap`, который книгу не трогает — книга пуста, депт-серия считается ИЗ книги. \
             `M-77` закрыл это ТОЛЬКО для `pump` (`book_series_in`, два вызова, оба внутри)."
        );
    }
}

/// СТОРОЖ ОБРАТНОЙ СТОРОНЫ (анти-плацебо). Там, где окно кадра СОДЕРЖИТ якорный снимок, пути
/// обязаны сходиться УЖЕ СЕЙЧАС.
///
/// Зелен в `main` и обязан остаться зелёным после фикса. Его назначение — поймать развязку,
/// которая «чинит» дельта-хвост ценой поломки нормального случая: ложное КРАСНОЕ на честной
/// работе опаснее пропуска, потому что его чинят ослаблением оракула.
#[test]
fn m85_2_anchored_window_already_agrees_and_must_keep_agreeing() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        j.append(book_at(0.05, T)).expect("b1");
        j.append(book_at(0.05, T + 1_000)).expect("b2");
        j.append(book_at(0.03, T + 2_000)).expect("b3");
        j.flush().expect("flush");
    }
    let s = sel();

    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot(LATEST)");
    let (merged, n) = assemble_via_frames(dir.path(), &s);
    assert!(
        n >= 2,
        "SETUP НЕ СОСТОЯЛСЯ: кадров {n} — склейка не задействована, сторож ничего не сторожит"
    );

    for side in ["bid", "ask"] {
        let want = series_of(&full.series.depth_series, side);
        let got = series_of(&merged.series.depth_series, side);
        assert_eq!(
            got, want,
            "РЕГРЕСС: на журнале ИЗ ОДНИХ ЯКОРЕЙ пути разошлись. Этот случай работал до `M-85`, \
             значит развязка сломала норму, а не починила дефект. side={side}\n  реплей: \
             {want:?}\n  клиент: {got:?}"
        );
    }
}

/// СТОРОЖ ЗАПРЕТНОГО СПИСКА §2.1 — НЕ-книжные серии не смеют задвоиться.
///
/// `C-223` N-1: мутант «полный bootstrap» (`reducer.apply(&event)` вместо `seed_vwap` на
/// затравочных событиях) зеленил ВЕСЬ набор `m85_*`, хотя §2.1 запрещает его прямо. Ловил его
/// только соседний независимый оракул — то есть запрет жил прозой, а не сторожем.
///
/// Мера снята с серий, которые от книги НЕ зависят и потому обязаны сходиться УЖЕ СЕГОДНЯ:
/// `vwap`, `cumulative_delta`, `volume_profile`. Полный bootstrap сворачивает затравочные
/// сделки в `frame.delta`, тогда как `snapshot(C)` их уже содержит; `Snapshot::apply` мёржит
/// `vwap` ЗАМЕНОЙ по ключу (`BTreeMap::extend`), а VP — инкрементом бинов, поэтому задвоение
/// видно на обоих.
///
/// ЗЕЛЁН СЕГОДНЯ и обязан остаться зелёным: его предмет — не прогресс, а граница.
#[test]
fn m85_6_non_book_series_do_not_double_count_under_bootstrap() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        j.append(book_at(0.05, T)).expect("c1");
        // Сделки ДО хвоста — они и задваиваются при полном bootstrap'е.
        for i in 0..6 {
            j.append(trade_at(MID + (i as f64), 1.0 + i as f64, T + 100 * i))
                .expect("trade");
        }
        j.append(delta_at(
            &[(MID * 0.95, 3.0)],
            &[(MID * 1.05, 3.0)],
            T + 2_000,
            9,
        ))
        .expect("c2");
        j.flush().expect("flush");
    }
    let s = sel();

    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot(LATEST)");
    let (merged, n) = assemble_via_frames(dir.path(), &s);
    assert!(
        n >= 2,
        "SETUP НЕ СОСТОЯЛСЯ: кадров {n} — затравка и хвост не разделены, задваивать нечего"
    );
    assert!(
        !full.series.vwap.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: vwap пуст — сделок в фикстуре нет, сторож судил бы пустоту"
    );

    assert_eq!(
        merged.series.vwap, full.series.vwap,
        "§2.1 НАРУШЕН: `vwap` у клиента разошёлся с полным реплеем. Это подпись ЗАТРАВКИ, \
         сворачивающей не-книжные события в кадр: `snapshot(C)` их уже содержит, и `apply` \
         мёржит ряд ЗАМЕНОЙ по ключу — значение кадра обязано быть АБСОЛЮТНЫМ, не локальным.\n  \
         реплей: {:?}\n  клиент: {:?}",
        full.series.vwap, merged.series.vwap
    );
    assert_eq!(
        merged.series.cumulative_delta, full.series.cumulative_delta,
        "§2.1 НАРУШЕН: `cumulative_delta` задвоился — затравка свернула сделки, уже учтённые в \
         `snapshot(C)`"
    );
    assert_eq!(
        merged.series.volume_profile.len(),
        full.series.volume_profile.len(),
        "§2.1 НАРУШЕН: число бинов `volume_profile` разошлось — VP мёржится ИНКРЕМЕНТОМ, \
         поэтому затравочные сделки, свёрнутые в кадр, добавляются поверх уже учтённых \
         (`apply_vp`, `crates/gateway/src/lib.rs:1099-1103` называет этот класс прямо)"
    );
}
