//! SACRED (architect-only) — `M-75` `H-5`: **окно карты ПРИНАДЛЕЖИТ СЕРВЕРУ, а не сужается
//! клиентской полосой снизу.**
//!
//! Закрывает `C-194` `B-2`. Предмет находки дословно: реализация, берущая окно как
//! **зажатое** `min(max(selector.bands), 0.001)`, проходит `H-1`, `H-3`, `H-4` И структурную
//! канарейку гейта — «both present selectors produce the same nonempty 0.001 map», тело
//! функции не содержит `selector.bands». Но связка ЖИВА: селектор НИЖЕ конфига по-прежнему
//! управляет шириной карты.
//!
//! # ПОЧЕМУ H-1 ЭТОГО НЕ ЛОВИТ — и почему это ровно правило `Р-4`
//!
//! `H-1` судит пару `[0.001]` против `[0.015…0.60]`: обе полосы ВЫШЕ или РАВНЫ конфигу, и
//! зажатие `min(·, 0.001)` делает их окна одинаковыми. Признак «ячеек столько же» истинен и в
//! честном мире, и в зажатом — то есть **производится обеими конструкциями**. Это третий
//! экземпляр класса, разобранного `A-029`: признак назначен, различающая сила не проверена
//! против того, что признак обязан отличать.
//!
//! `Р-4` требует признака, НЕДОСТУПНОГО миру, где событие не произошло. Здесь он строится
//! так: взять полосу **СТРОГО НИЖЕ** конфига. Зажатие снизу не работает — `min(0.0004, 0.001)`
//! = `0.0004`, окно сужается, карта редеет. Честная реализация окна не сужает ни при какой
//! клиентской полосе, потому что клиентская полоса в окно не входит вовсе.
//!
//! | реализация | `bands=[0.0004]` | `bands=[0.001]` | признак «карты равны» |
//! |---|---|---|---|
//! | сегодняшняя связка `max(bands)` | окно 0.0004 | окно 0.001 | ЛОЖЕН ⇒ RED |
//! | зажатая `min(max(bands), 0.001)` | окно 0.0004 | окно 0.001 | ЛОЖЕН ⇒ RED |
//! | честная (окно из конфига) | окно конфига | окно конфига | ИСТИНЕН ⇒ GREEN |
//!
//! # ЧЕГО ЭТОТ ОРАКУЛ НЕ ЛОВИТ — названо, а не умолчано
//!
//! Реализацию «окно = жёсткая константа `0.001` в теле, конфиг игнорируется» он пропускает:
//! она даёт равные карты при любых полосах. Отличить её можно только СМЕНОЙ серверной
//! настройки, а `gateway::set_effective_heatmap_window_frac` ещё не существует — писать
//! COMPILE-RED против неё запрещено прецедентом `M-67` §10 (`red_grace_from_env.rs:12-15`:
//! «это не RED, а поломка сборки», роняющая весь `cargo test --all`, включая этот файл).
//!
//! Остаток закрыт ДВУМЯ вещами, обе объявлены в `M-75` §8, а не подразумеваются:
//!   1. канарейка гейта на МЕСТЕ ВЫЗОВА (`C-194`: «pin the call-site/property that supplies
//!      that effective value, not only the callee body») — структурная, и её предел тот же,
//!      что у всякой структурной проверки (`M-45` §D-1: обходится сдвигом на уровень выше);
//!      поэтому она не выдаётся за доказательство;
//!   2. задача `5b` — оракул смены серверной настройки, пишется architect'ом СРАЗУ после
//!      появления сигнатуры (задача 2) и ДО close-out. Тот же порядок, что
//!      `red_grace_from_env.rs:19` объявляет для своего поля.
//!
//! # СОСТОЯНИЕ: КРАСНЫЙ ПО ПОСТРОЕНИЮ
//!
//! Сегодня окно есть `max(selector.bands)` (`crates/gateway/src/lib.rs:1557`), поэтому полоса
//! ниже конфига сужает карту, и `hw_i_5_below_config_band_cannot_shrink_the_map` падает.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Шаг цены фикстуры — доля от mid. Тот же, что в `red_heatmap_window_decoupled.rs`: снят с
/// прод-замера (`M-71` §2.2 — 359 880 ячеек при 5 998 уровнях), густо у touch.
const STEP_FRAC: f64 = 0.0002;

/// Серверный дефолт окна (`M-75` §5, равен сегодняшнему эффективному `GATEWAY_BANDS=0.001`).
const CONFIG_WINDOW: f64 = 0.001;
/// Полоса СТРОГО НИЖЕ конфига — ядро различающей силы. Кратна шагу: `0.0004 / 0.0002 = 2`
/// уровня на сторону, то есть карта непуста, но заметно у́же конфигурной.
const BAND_BELOW_CONFIG: f64 = 0.0004;

/// Уровней на сторону: хвост до ±60 % при шаге 0.02 % (`0.60 / 0.0002`).
const LEVELS_TO_60PCT: usize = 3_000;
const TICKS: usize = 10;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-75 server-owned window fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel_with(bands: &[f64]) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: bands.to_vec(),
        window_ms: None,
        depth_cadence_ms: None,
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

fn snap(dir: &std::path::Path, bands: &[f64]) -> std::io::Result<gateway::Snapshot> {
    gateway::snapshot(
        dir,
        EpochFilter::OwnCaptureOnly,
        &sel_with(bands),
        Cursor::LATEST,
    )
}

/// **СТРАЖ РАЗЛИЧАЮЩЕЙ СИЛЫ (`Р-4а`), ИСПОЛНЯЕМЫЙ И НЕЗАВИСИМЫЙ ОТ РЕАЛИЗАЦИИ.**
///
/// Признак этого оракула — «карты равны». Если бы в кольце между двумя полосами не лежало ни
/// одного уровня книги, признак был бы истинен И ПРИ ЖИВОЙ СВЯЗКЕ — оракул зеленел бы ни о
/// чём, ровно как `H-1` зеленел против зажатой связки.
///
/// Страж считает уровни ПО САМОЙ ФИКСТУРЕ (шаг и число уровней — константы этого файла), а не
/// по выдаче gateway: иначе он спрашивал бы у проверяемого, состоятельна ли проверка.
fn assert_discriminating_power_of_the_fixture() {
    // Уровень k лежит на расстоянии k*STEP_FRAC от mid. В кольцо (BAND_BELOW_CONFIG,
    // CONFIG_WINDOW] попадают k, для которых BAND_BELOW_CONFIG < k*STEP_FRAC <= CONFIG_WINDOW.
    let k_lo = (BAND_BELOW_CONFIG / STEP_FRAC).floor() as usize; // последний k внутри узкой
    let k_hi = (CONFIG_WINDOW / STEP_FRAC).floor() as usize; // последний k внутри конфигурной
    let in_ring = k_hi.saturating_sub(k_lo);
    assert!(
        in_ring >= 3,
        "SETUP НЕ СОСТОЯЛСЯ: между полосой {BAND_BELOW_CONFIG} и конфигом {CONFIG_WINDOW} лежит \
         всего {in_ring} уровней на сторону (шаг фикстуры {STEP_FRAC}). Признак «карты равны» \
         был бы истинен и при ЖИВОЙ связке — оракул зеленел бы ни о чём. Чинить фикстуру \
         (шаг/полосы), а не реализацию: правило Р-4а, A-029"
    );
    assert!(
        k_lo >= 1,
        "SETUP НЕ СОСТОЯЛСЯ: узкая полоса {BAND_BELOW_CONFIG} у́же ПЕРВОГО уровня фикстуры \
         (шаг {STEP_FRAC}) — её карта пуста при любой реализации, и сравнение вырождается в \
         «непусто против нуля». Это дефект фикстуры, уже пойманный однажды на H-1"
    );
}

/// Третье условие различающей силы вынесено в КОМПАЙЛ-ТАЙМ намеренно: оно зависит только от
/// констант этого файла, и правка, ставящая полосу выше конфига, обязана не собраться, а не
/// упасть в прогоне. Рантайм-`assert!` здесь был бы вдобавок отвергнут clippy
/// (`assertions_on_constants`) — тот случай, когда лишний диагностический шум указывает на
/// верную конструкцию.
const _: () = assert!(
    BAND_BELOW_CONFIG < CONFIG_WINDOW,
    "SETUP: полоса обязана быть СТРОГО НИЖЕ конфига — иначе зажатие сверху её накрывает и \
     мутант C-194 остаётся неотличим"
);

// ═══════════════════════════════════════════════════════════════════════════════════════════
// H-5 — ПРЕДМЕТ: полоса НИЖЕ конфига не вправе сузить карту
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Инвариант `HW-I-5`. Падает и против сегодняшней связки `max(bands)`, и против зажатой
/// `min(max(bands), 0.001)` — того мутанта, который прошёл весь набор круга 1 (`C-194` B-2).
#[test]
fn hw_i_5_below_config_band_cannot_shrink_the_map() {
    assert_discriminating_power_of_the_fixture();

    let dir = journal_deep_book(TICKS, LEVELS_TO_60PCT);

    let below = snap(dir.path(), &[BAND_BELOW_CONFIG]).unwrap_or_else(|e| {
        panic!("SETUP НЕ СОСТОЯЛСЯ: узкий селектор обязан обслуживаться сегодня: {e}")
    });
    let at_config = snap(dir.path(), &[CONFIG_WINDOW]).unwrap_or_else(|e| {
        panic!("SETUP НЕ СОСТОЯЛСЯ: конфигурная полоса обязана обслуживаться: {e}")
    });

    // Обе карты непусты — иначе сравнение вырождается (тот же дефект, что чинился на H-1).
    assert!(
        !below.series.heatmap.is_empty() && !at_config.series.heatmap.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: одна из карт пуста ({} и {} ячеек) — шаг фикстуры разъехался с \
         полосами, сравнение ничего не судит",
        below.series.heatmap.len(),
        at_config.series.heatmap.len()
    );

    assert_eq!(
        below.series.heatmap.len(),
        at_config.series.heatmap.len(),
        "HW-I-5 НАРУШЕН: полоса {BAND_BELOW_CONFIG}, лежащая НИЖЕ серверного окна \
         {CONFIG_WINDOW}, сузила карту — {} ячеек против {}. Значит окно по-прежнему \
         выводится из клиентского селектора: либо напрямую (`max(bands)`, lib.rs:1557), либо \
         зажатием сверху (`min(max(bands), {CONFIG_WINDOW})`) — второе проходит H-1/H-3/H-4 и \
         структурную канарейку, и ровно поэтому существует этот оракул (C-194 B-2). Ширина \
         карты обязана быть СЕРВЕРНОЙ настройкой, не клиентским входом (PL-I-5)",
        below.series.heatmap.len(),
        at_config.series.heatmap.len()
    );
    assert_eq!(
        below.series.cob.len(),
        at_config.series.cob.len(),
        "HW-I-5 НАРУШЕН: COB сузился той же полосой — {} против {}. COB строится тем же окном, \
         что и heatmap, и наследует ту же связку",
        below.series.cob.len(),
        at_config.series.cob.len()
    );
}

/// ПАРНЫЙ VANTAGE — расцепление не куплено обнулением: карта под серверным окном обязана
/// остаться непустой и на полосе НИЖЕ конфига.
///
/// Без него `H-5` удовлетворяется реализацией «окно всегда 0» (равные пустые карты) —
/// `testing.md` §«что пришлось ослабить рядом». `H-4` сторожит тот же класс на своей паре
/// полос; здесь сторож нужен на СВОЕЙ, иначе пара «ниже конфига» им не покрыта.
#[test]
fn hw_i_5b_server_window_still_produces_a_map_for_a_below_config_band() {
    let dir = journal_deep_book(TICKS, LEVELS_TO_60PCT);
    let below = snap(dir.path(), &[BAND_BELOW_CONFIG])
        .unwrap_or_else(|e| panic!("SETUP НЕ СОСТОЯЛСЯ: {e}"));

    assert!(
        !below.series.heatmap.is_empty(),
        "HW-I-5b НАРУШЕН: карта ПУСТА при полосе {BAND_BELOW_CONFIG} на плотной книге. \
         Реализация, обнулившая окно, удовлетворяет H-5 (обе карты пусты и потому равны) и \
         уничтожает продукт"
    );
    assert!(
        !below.series.cob.is_empty(),
        "HW-I-5b НАРУШЕН: COB пуст при полосе {BAND_BELOW_CONFIG} — то же самое"
    );
}
