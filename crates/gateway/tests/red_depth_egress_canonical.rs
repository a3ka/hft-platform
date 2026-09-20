//! SACRED (architect-only) — `M-70` `DB-I-0`: **объём канонического набора полос СНЯТ
//! ЗАМЕРОМ в байтах сериализованного ответа, а не выведен арифметикой по числу ячеек.**
//!
//! # Зачем оракул, если «и так понятно»
//!
//! Два наших собственных документа называли для одной и той же величины РАЗНЫЕ числа:
//! `M-70` §0.3 — «канонический набор ≈ 22 МБ на клиента», `M-71` §2.2 — «45 261 937 Б
//! (43.2 МБ)». Оба выше подписанного предела `П-020` (2 000 000 Б), поэтому РЕШЕНИЕ от
//! расхождения не менялось — но одно из чисел неверно, и выбирать состав полос по неверному
//! нельзя. Обе величины получены АРИФМЕТИКОЙ по числу ячеек; `testing.md` §«Оракул обязан
//! мерить ТО, ЧТО ОБЕЩАЕТ» п.2 требует мерить РЕСУРС, а прокси — не ресурс. Здесь меряются
//! байты `serde_json::to_vec(&Snapshot)` — ровно то, чем платит сеть.
//!
//! # Что изменилось после `M-75` — и почему замер имеет смысл только теперь
//!
//! До расцепления ширина карты выводилась из `max(Selector.bands)`, поэтому канонический
//! набор раздувал heatmap и выносил ответ за предел (замер `M-75`: `observed=7 882 335`
//! при `limit=2 000 000` на прод-форменной фикстуре). После расцепления (`main` `571403d`)
//! окно карты — серверная настройка, и полосы отвечают ТОЛЬКО за депт-серию. Значит вопрос
//! «22 или 43 МБ» относился к миру, которого больше нет; величина, на которой стоит решение
//! `П-014` п.4, — это объём ответа СЕГОДНЯ, и он снимается здесь.
//!
//! # Что именно утверждают сценарии
//!
//! `db_i_0_canonical_set_fits_under_signed_cap` — канонический набор из семи полос
//! (`depth-verdict.md:15`, решение founder'а: 1.5/3/5/8/15/30/60 %) даёт ответ ПОД
//! подписанным пределом. Это и есть предусловие задачи 7: включать состав, который не
//! помещается, значит вводить в прод отказ выдачи.
//!
//! `db_i_0b_growth_is_the_depth_series_not_the_map` — прирост байт между узким и каноническим
//! селектором принадлежит ДЕПТ-СЕРИИ, а карта не растёт вовсе. Это анти-плацебо к первому:
//! «влезло» может быть истинным и в мире, где полосы вообще ни на что не влияют (например
//! депт-серия молча пуста). Сценарий требует, чтобы полосы работали — и чтобы работали
//! ровно там, где им положено после `M-75`.
//!
//! # Различающая сила признака (`Р-4`)
//!
//! Признак первого сценария — «байт меньше предела». Мир, где событие не произошло
//! (связка окна с полосами жива), этого признака не несёт: там канонический набор либо
//! выносит ответ за предел, либо получает отказ `enforce_response_limit` — и оба исхода
//! роняют сценарий. Признак второго — «число ячеек карты одинаково при РАЗНЫХ полосах»;
//! он сопровождается положительным контролем (депт-серия обязана вырасти), иначе
//! «одинаково» было бы истинно и при двух неразличимых селекторах.
//!
//! # Состояние
//!
//! Оракул ЗЕЛЁН на `main` после `M-75` — и это правильное состояние для `DB-I-0`: задача 0
//! есть ЗАМЕР, а не изменение поведения. Красным он станет, если расцепление откатят или
//! если состав полос перестанет доезжать до депт-серии; ровно эти два регресса он и стоит
//! сторожить до задачи 7 и после неё.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector, DEFAULT_MAX_RESPONSE_BYTES};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Прод-дефолт (`docker-compose.yml`): узкая полоса ±0.1 %.
const BANDS_NARROW: &[f64] = &[0.001];

/// Канонический набор — РЕШЕНИЕ FOUNDER'А, а не оценка автора теста:
/// `research/data-quality/depth-verdict.md:15` — «полный TPP-набор 1.5/3/5/8/15/30/60»;
/// то же семь значений в `docs/fa/viz-backend.md:42` и в шаге `task #7` гейта `M-70`.
const BANDS_CANONICAL: &[f64] = &[0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-70 DB-I-0 egress fixture".to_string(),
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

/// Форма книги ПЕРЕИСПОЛЬЗОВАНА, а не изобретена заново: та же конструкция, что у оракула
/// `M-75` (`red_heatmap_window_decoupled.rs`), и по той же причине — она снята с прод-замера
/// (`M-71` §2.2: heatmap 359 880 ячеек при 5 998 уровнях книги). Шаг 0.02 % от mid делает
/// книгу ГУСТОЙ у touch, `LEVELS_TO_60PCT` тянет хвост до ±60 %, то есть до самой дальней
/// канонической полосы. Равномерная раскладка была бы дефектом: шаг вышел бы грубее узкой
/// полосы, и узкий селектор дал бы ПУСТУЮ депт-серию — сравнение выродилось бы в «что-то
/// против нуля», истинное при любой реализации.
fn journal_deep_book(ticks: usize, levels_per_side: usize) -> tempfile::TempDir {
    const STEP_FRAC: f64 = 0.0002;
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

/// `0.60 / 0.0002 = 3000` — уровней на сторону, чтобы книга доставала до самой дальней
/// канонической полосы. Связь «шаг × уровни = охват» названа константой намеренно.
const LEVELS_TO_60PCT: usize = 3_000;
/// Тактов — 60, а не 10: прод-окно `GATEWAY_WINDOW_MS=60000` при `timeframe_ms=1000` даёт
/// РОВНО 60 бакетов, и объём ответа линеен по их числу. Фикстура на 10 тактов была бы
/// прод-ФОРМЫ, но не прод-РАЗМЕРА, и абсолютное число байт пришлось бы экстраполировать —
/// то есть вернуться к арифметике, ради ухода от которой задача 0 и существует.
const TICKS: usize = 60;

/// Снимок по селектору. `Err` — отдельный исход: предел `M-71` отверг ответ ДО клиента.
fn snap(dir: &std::path::Path, bands: &[f64]) -> std::io::Result<gateway::Snapshot> {
    gateway::snapshot(
        dir,
        EpochFilter::OwnCaptureOnly,
        &sel_with(bands),
        Cursor::LATEST,
    )
}

/// РЕСУРС, а не прокси: байты того самого JSON, который уходит на провод
/// (`gateway-serve` сериализует `Snapshot` целиком — `VB-I-6`).
fn bytes_of(s: &gateway::Snapshot) -> usize {
    serde_json::to_vec(s).expect("сериализация Snapshot").len()
}

/// SETUP-СТРАЖ: фикстура обязана давать НЕПУСТУЮ депт-серию на ОБОИХ селекторах. Иначе
/// «влезло под предел» истинно вырожденно — влезает пустота, а не выдача.
fn assert_series_are_real(narrow: &gateway::Snapshot, wide: &gateway::Snapshot) {
    let n_points: usize = narrow
        .series
        .depth_series
        .iter()
        .map(|r| r.series.len())
        .sum();
    let w_points: usize = wide
        .series
        .depth_series
        .iter()
        .map(|r| r.series.len())
        .sum();
    assert!(
        n_points > 0 && w_points > 0,
        "SETUP НЕ СОСТОЯЛСЯ: депт-серия пуста (узкий {n_points} точек, канонический \
         {w_points}). Замер объёма на пустой выдаче ничего не говорит о проде; чинить \
         фикстуру, а не вывод"
    );
}

/// `DB-I-0` — канонический набор помещается под подписанный предел. Замер в БАЙТАХ.
#[test]
fn db_i_0_canonical_set_fits_under_signed_cap() {
    let dir = journal_deep_book(TICKS, LEVELS_TO_60PCT);

    let narrow = snap(dir.path(), BANDS_NARROW).expect("узкий селектор обязан отдать снимок");
    let wide = match snap(dir.path(), BANDS_CANONICAL) {
        Ok(s) => s,
        Err(e) => panic!(
            "канонический набор ОТВЕРГНУТ пределом: {e}. Это мир, где окно карты снова \
             выводится из полос (регресс `M-75`): после расцепления полосы отвечают только \
             за депт-серию, и ответ обязан помещаться"
        ),
    };
    assert_series_are_real(&narrow, &wide);

    let n_bytes = bytes_of(&narrow);
    let w_bytes = bytes_of(&wide);

    assert!(
        w_bytes < DEFAULT_MAX_RESPONSE_BYTES,
        "DB-I-0 НАРУШЕН: канонический набор даёт {w_bytes} Б при подписанном пределе {} Б \
         (узкий — {n_bytes} Б). Включать состав, который не помещается, значит вводить в прод \
         отказ выдачи вместо данных (`П-020`, `PL-I-5`)",
        DEFAULT_MAX_RESPONSE_BYTES
    );

    // Замер печатается: число из этого прогона и есть ответ задачи 0, и оно обязано быть
    // видно в выводе, а не только в утверждении.
    println!(
        "DB-I-0 ЗАМЕР: narrow={n_bytes} Б · canonical={w_bytes} Б · предел={} Б · \
         запас ×{:.1}",
        DEFAULT_MAX_RESPONSE_BYTES,
        DEFAULT_MAX_RESPONSE_BYTES as f64 / w_bytes as f64
    );
}

/// `DB-I-0b` — прирост принадлежит ДЕПТ-СЕРИИ, карта не растёт. Анти-плацебо к первому
/// сценарию и одновременно сторож регресса `M-75` изнутри `M-70`.
#[test]
fn db_i_0b_growth_is_the_depth_series_not_the_map() {
    let dir = journal_deep_book(TICKS, LEVELS_TO_60PCT);

    let narrow = snap(dir.path(), BANDS_NARROW).expect("узкий селектор");
    let wide = snap(dir.path(), BANDS_CANONICAL).expect("канонический селектор");
    assert_series_are_real(&narrow, &wide);

    // ПОЛОЖИТЕЛЬНЫЙ КОНТРОЛЬ: полосы обязаны работать там, где им положено.
    let n_rows = narrow.series.depth_series.len();
    let w_rows = wide.series.depth_series.len();
    assert_eq!(
        (n_rows, w_rows),
        (BANDS_NARROW.len() * 2, BANDS_CANONICAL.len() * 2),
        "SETUP НЕ СОСТОЯЛСЯ: строк депт-серии {n_rows}/{w_rows}, ожидалось \
         {}/{} (полоса × сторона). Если полосы не доезжают до серии, вывод «прирост \
         принадлежит серии» проверять не на чем",
        BANDS_NARROW.len() * 2,
        BANDS_CANONICAL.len() * 2
    );

    // ГЛАВНОЕ: карта от полос не зависит — это свойство внесено `M-75` и здесь сторожится
    // изнутри соседнего предмета, потому что именно `M-70` меняет состав полос на проде.
    assert_eq!(
        wide.series.heatmap.len(),
        narrow.series.heatmap.len(),
        "РЕГРЕСС `M-75`: карта выросла с {} до {} ячеек при смене ТОЛЬКО полос. Окно карты \
         обязано приходить из серверной настройки (`effective_heatmap_window_frac`), а не из \
         `max(Selector.bands)`",
        narrow.series.heatmap.len(),
        wide.series.heatmap.len()
    );

    let delta = bytes_of(&wide).saturating_sub(bytes_of(&narrow));
    assert!(
        delta > 0,
        "прирост нулевой: канонический набор не добавил в ответ ничего, хотя строк серии \
         стало {w_rows} против {n_rows}. Либо серия не сериализуется, либо селектор не \
         доехал — оба случая делают замер задачи 0 бессмысленным"
    );
    println!("DB-I-0b ЗАМЕР: прирост от полос = {delta} Б, карта не изменилась");
}
