//! SACRED (architect-only) — `M-75` `H-6`: **смена СЕРВЕРНОЙ настройки меняет охват карты.**
//!
//! Закрывает вторую половину `C-194` `B-2` и блокер `C-196` `B-3`. Требование вердикта
//! дословно: оракул обязан наблюдать, «that changing the declared effective server setting,
//! rather than `Selector.bands`, changes their window».
//!
//! # СПОРА НЕТ — ЕСТЬ ИСПОЛНЕНИЕ, и это сказано прямо
//!
//! `C-196` направил предмет арбитру как методологический спор («допустим ли dispatch task 2
//! при сознательно отложенном `H-6`»). Спор снят не доводом, а работой: требование
//! принято целиком, и оракул написан ДО dispatch — то есть основание для арбитража
//! исчезло, а не было обойдено. `gates.md` §0 созывает арбитра, когда стороны не понимают
//! друг друга; здесь architect с критиком согласен.
//!
//! # ПОЧЕМУ ЭТО ОКАЗАЛОСЬ ПИСУЕМО СЕГОДНЯ, хотя круг 1 объявил обратное
//!
//! Круг 1 рассуждал так: наблюдать смену серверной настройки нельзя, пока нет
//! `gateway::set_effective_heatmap_window_frac`, а COMPILE-RED против несуществующей
//! сигнатуры роняет сборку всего workspace (`M-67` §10). Первая половина посылки НЕВЕРНА, и
//! ошибка была моя: я искал СЕТТЕР, тогда как прод меняет настройку не сеттером, а
//! ОКРУЖЕНИЕМ — через `serve_config_from_env`, который существует и публичен.
//!
//! Домашний образец лежал в этом же каталоге и прошёл гейты дважды:
//! `red_egress_cap_governed.rs:44-52` — «Оракул судит ЦЕПЬ ЦЕЛИКОМ:
//! `env → serve_config_from_env → эффективное значение → отказ`», и там же названа причина
//! выбора: «путь тот же, каким его дёргает прод, а не сконструированный тестом
//! (`testing.md`, „Целостность гейта" свойство 1)». Здесь та же цепь, только последнее
//! звено — не отказ, а ОХВАТ ВЫДАЧИ.
//!
//! Это правило предшественника (`reading-map` §2), не применённое вовремя: конструкция
//! существовала, я её не искал и объявил задачу невозможной. Наблюдение через прод-путь
//! вдобавок СИЛЬНЕЕ наблюдения через сеттер — оно судит и доставку значения (env → конфиг),
//! а не только его применение.
//!
//! # ПРИЗНАК И ЕГО РАЗЛИЧАЮЩАЯ СИЛА (`Р-4`, `main` `af29452`)
//!
//! Признак: **охват карты РАСТЁТ, когда растёт серверная настройка, при НЕИЗМЕННОМ
//! клиентском селекторе.** Мир, где событие не произошло (настройка не влияет на выдачу),
//! этого признака не несёт по построению — там охват не меняется ВООБЩЕ:
//!
//! | реализация | окно при `0.001` | окно при `0.01` | признак «охват вырос» |
//! |---|---|---|---|
//! | сегодняшняя связка `max(bands)` | `bands` | `bands` | ЛОЖЕН ⇒ RED |
//! | зажатая `min(max(bands), 0.001)` | ≤ `0.001` | ≤ `0.001` | ЛОЖЕН ⇒ RED |
//! | **жёсткая константа `w = 0.001`** — мир, прошедший все пять оракулов круга 2 (`C-196` B-3) | `0.001` | `0.001` | **ЛОЖЕН ⇒ RED** |
//! | честная (окно из эффективной настройки) | `0.001` | `0.01` | ИСТИНЕН ⇒ GREEN |
//!
//! Третья строка — ровно тот живой остаточный мир, который `C-196` предъявил мутацией и
//! который я в круге 2 признал непокрытым. Здесь он закрыт.
//!
//! # СТРАЖ ВЫПОЛНИМОСТИ (`Р-4а`)
//!
//! Признак «охват вырос» достижим, только если между двумя настройками лежат уровни книги.
//! Страж считает их ПО КОНСТАНТАМ ФИКСТУРЫ, а не по выдаче gateway: спрашивать у
//! проверяемого, состоятельна ли проверка, — та же ошибка, что признак без различающей силы.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use gateway_serve::serve_config_from_env;
use journal::{EpochFilter, Journal, WriterConfig};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;
const VAR: &str = "GATEWAY_HEATMAP_WINDOW";

/// Шаг цены фикстуры — доля от mid (тот же, что в соседних оракулах `M-75`).
const STEP_FRAC: f64 = 0.0002;
/// Узкая серверная настройка — прод-дефолт `M-75` §5.
const WINDOW_NARROW: &str = "0.001";
const WINDOW_NARROW_F: f64 = 0.001;
/// Широкая серверная настройка. Внутри `(0, 1)`, заведомо законна для fail-closed разбора.
const WINDOW_WIDE: &str = "0.01";
const WINDOW_WIDE_F: f64 = 0.01;

const LEVELS_TO_60PCT: usize = 3_000;
const TICKS: usize = 10;

/// Эффективное окно — ПРОЦЕССНОЕ состояние. Тесты, трогающие его, обязаны идти
/// последовательно, иначе они меряют друг друга. Приём и причина — `red_egress_cap_governed.rs:66`.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

const BASE: &[(&str, &str)] = &[
    ("GATEWAY_JWT_SECRET", "test-secret"),
    ("GATEWAY_TIMEFRAME_MS", "1000"),
];

fn base_plus(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = BASE.to_vec();
    v.extend_from_slice(extra);
    v
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-75 effective server setting fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvls(v: &[(f64, f64)]) -> Vec<Level> {
    v.iter()
        .map(|(p, s)| Level {
            price: to_fixed(*p),
            size: to_fixed(*s),
        })
        .collect()
}

fn journal_deep_book(ticks: usize, levels_per_side: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for t in 0..ticks as i64 {
        let step = MID * STEP_FRAC;
        let bids: Vec<(f64, f64)> = (1..=levels_per_side)
            .map(|k| (MID - k as f64 * step, 1.0 + (k % 17) as f64))
            .collect();
        let asks: Vec<(f64, f64)> = (1..=levels_per_side)
            .map(|k| (MID + k as f64 * step, 1.0 + (k % 17) as f64))
            .collect();
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: lvls(&bids),
                asks: lvls(&asks),
                ts_exch_ms: T0 + t * 1_000,
            },
        ))
        .expect("append L2Snapshot");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID + t as f64 * 0.01),
                size: to_fixed(1.0),
                side: if t % 2 == 0 { Side::Buy } else { Side::Sell },
                ts_exch_ms: T0 + t * 1_000,
            },
        ))
        .expect("append Trade");
    }
    j.flush().expect("flush");
    dir
}

/// Прогон ПРОД-ФОРМЫ: окружение → `serve_config_from_env` → снимок ЕГО селектором.
///
/// Селектор берётся из `ServeConfig`, а не собирается тестом: иначе оракул судил бы путь,
/// которым прод не ходит (`testing.md` «Целостность гейта» св. 1). Между двумя прогонами
/// селектор ОДИНАКОВ — меняется ровно серверная настройка, и только она.
fn heatmap_extent_for(dir: &std::path::Path, window: &'static str) -> (usize, usize, Selector) {
    let env = base_plus(&[(VAR, window)]);
    let cfg = serve_config_from_env(getter(&env)).unwrap_or_else(|e| {
        panic!(
            "SETUP НЕ СОСТОЯЛСЯ: {VAR}={window:?} — законное значение внутри (0,1), старт \
             обязан состояться. Отказ: {e}"
        )
    });
    let snap = gateway::snapshot(
        dir,
        EpochFilter::OwnCaptureOnly,
        &cfg.selector,
        Cursor::LATEST,
    )
    .unwrap_or_else(|e| panic!("SETUP НЕ СОСТОЯЛСЯ: снимок при {VAR}={window:?} не построен: {e}"));
    (
        snap.series.heatmap.len(),
        snap.series.cob.len(),
        cfg.selector,
    )
}

/// СТРАЖ ВЫПОЛНИМОСТИ (`Р-4а`): между узкой и широкой настройкой обязаны лежать уровни
/// книги, иначе признак «охват вырос» недостижим и при ЧЕСТНОЙ реализации — то есть оракул
/// был бы невыполним, а не строг. Считается по константам фикстуры, независимо от gateway.
fn assert_setting_gap_is_observable() {
    let k_narrow = (WINDOW_NARROW_F / STEP_FRAC).floor() as usize;
    let k_wide = (WINDOW_WIDE_F / STEP_FRAC).floor() as usize;
    let in_gap = k_wide.saturating_sub(k_narrow);
    assert!(
        in_gap >= 10,
        "SETUP НЕ СОСТОЯЛСЯ: между настройками {WINDOW_NARROW} и {WINDOW_WIDE} лежит всего \
         {in_gap} уровней на сторону (шаг {STEP_FRAC}). Признак «охват вырос» был бы \
         недостижим и для честной реализации — оракул невыполним, а не строг. Чинить \
         фикстуру: правило Р-4а"
    );
    assert!(
        k_wide <= LEVELS_TO_60PCT,
        "SETUP НЕ СОСТОЯЛСЯ: широкая настройка {WINDOW_WIDE} выходит за хвост фикстуры \
         ({LEVELS_TO_60PCT} уровней на сторону) — рост охвата упёрся бы в край книги, а не в \
         настройку"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// H-6 — ПРЕДМЕТ: серверная настройка УПРАВЛЯЕТ охватом; клиентский селектор неизменен
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Инвариант `HW-I-6`. Единственный оракул набора, красный против мутанта «жёсткая
/// константа» (`C-196` B-3) — того мира, который прошёл все пять предыдущих.
#[test]
fn hw_i_6_effective_server_setting_controls_map_extent() {
    let _g = serial();
    assert_setting_gap_is_observable();

    let dir = journal_deep_book(TICKS, LEVELS_TO_60PCT);

    let (narrow_cells, narrow_cob, sel_narrow) = heatmap_extent_for(dir.path(), WINDOW_NARROW);
    let (wide_cells, wide_cob, sel_wide) = heatmap_extent_for(dir.path(), WINDOW_WIDE);

    // СТРАЖ ЧИСТОТЫ ОПЫТА: варьируется РОВНО ОДНА величина. Если селекторы разошлись,
    // рост охвата объясняется клиентским входом, и вывод о серверной настройке был бы
    // подменой причины (`testing.md`: конфаундинг держать КОНСТАНТНЫМ).
    assert_eq!(
        sel_narrow.bands, sel_wide.bands,
        "SETUP НЕ СОСТОЯЛСЯ: клиентские полосы различаются между прогонами ({:?} против \
         {:?}) — оракул мерил бы влияние СЕЛЕКТОРА, а не серверной настройки",
        sel_narrow.bands, sel_wide.bands
    );

    assert!(
        narrow_cells > 0 && narrow_cob > 0,
        "SETUP НЕ СОСТОЯЛСЯ: узкая настройка даёт пустую карту ({narrow_cells} ячеек, \
         {narrow_cob} уровней COB) — сравнение выродилось бы в «непусто против нуля»"
    );

    assert!(
        wide_cells > narrow_cells,
        "HW-I-6 НАРУШЕН: смена {VAR} с {WINDOW_NARROW} на {WINDOW_WIDE} НЕ расширила карту \
         — {wide_cells} ячеек против {narrow_cells} при одном и том же клиентском селекторе \
         (bands={:?}). Значит окно берётся откуда угодно, только не из серверной настройки: \
         из `max(selector.bands)`, из зажатой связки либо из жёсткой константы в теле. \
         Последний мир проходит H-1/H-3/H-4/H-5/H-5b (замер C-196 B-3) и ловится ТОЛЬКО \
         здесь. Ручка, не влияющая на выдачу, есть built-not-wired: оператор её выставил, \
         поведение не изменилось, все прочие гейты зелены",
        sel_narrow.bands
    );
    assert!(
        wide_cob > narrow_cob,
        "HW-I-6 НАРУШЕН: COB не расширился при смене {VAR} — {wide_cob} против {narrow_cob}. \
         COB строится тем же окном, что и heatmap, и обязан следовать за настройкой вместе с \
         ним; расхождение означало бы, что расцеплена только половина выдачи"
    );
}

/// ПАРНЫЙ VANTAGE — настройка управляет охватом, но НЕ отменяет его.
///
/// Без него `H-6` удовлетворяется реализацией «узкая настройка даёт пустую карту, широкая —
/// непустую»: рост формально есть, продукт разрушен. Тот же класс, что `H-4`/`H-5b`, но на
/// СВОЕЙ оси — оси серверной настройки (`testing.md` §«что пришлось ослабить рядом»).
#[test]
fn hw_i_6b_both_settings_produce_a_nonempty_map() {
    let _g = serial();
    let dir = journal_deep_book(TICKS, LEVELS_TO_60PCT);

    for w in [WINDOW_NARROW, WINDOW_WIDE] {
        let (cells, cob, _) = heatmap_extent_for(dir.path(), w);
        assert!(
            cells > 0 && cob > 0,
            "HW-I-6b НАРУШЕН: при {VAR}={w} карта пуста ({cells} ячеек, {cob} уровней COB). \
             Реализация, где узкое окно обнуляет выдачу, удовлетворила бы H-6 ростом от нуля \
             и уничтожила бы продукт на прод-дефолте"
        );
    }
}
