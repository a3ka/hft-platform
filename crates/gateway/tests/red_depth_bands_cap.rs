//! SACRED (architect-only) — `M-70` `DB-I-3`: **число клиентских полос ограничено, и отказ
//! наступает ДО построения ответа, а не после.**
//!
//! # Дыра, замеренная а не предположенная
//!
//! После `M-75` ширина карты перестала быть клиентским входом, но ЧИСЛО полос им осталось:
//! `gateway::validate_selector` о `bands` не знает ни одной строкой, а транспорт
//! (`gateway-serve::session::validate_selector`) проверяет только диапазон `(0,1)`,
//! сортировку и дубли — **но не количество**. Замер architect'а 2026-09-03 на прод-форменной
//! фикстуре (60 бакетов, книга ±60 %):
//!
//! ```text
//! N полос ·  строк · байт ответа · доля предела · время построения
//!       1 ·      2 ·     80 436 ·   4.0 %       · 0.25 с
//!       7 ·     14 ·    101 077 ·   5.1 %       · 0.27 с   ← канонический набор `П-014`
//!      64 ·    128 ·    297 015 ·  14.9 %       · 0.45 с
//!     256 ·    512 ·    957 223 ·  47.9 %       · 1.09 с
//!    1024 ·   2048 · ОТКАЗ `PL-I-5` после построения 3 577 553 Б · 3.80 с
//!    4096 ·   8192 · ОТКАЗ `PL-I-5` после построения 14 077 293 Б · 18.13 с
//! ```
//!
//! Две нижние строки и есть предмет: предел `M-71` СРАБАТЫВАЕТ — но лишь после того, как
//! сервер собрал 14 МБ и потратил 18 секунд. Клиент получает отказ, сервер платит работой.
//! Это `PL-I-5` дословно: «усиление ресурса, управляемое клиентом». При 10 000 сессий один
//! такой запрос на сессию превращает отказ в отказ обслуживания.
//!
//! # Почему признак «вернулся `Err`» НЕГОДЕН, и как оракул это обходит (`Р-4`)
//!
//! Мир, где дефект НЕ исправлен, ТОЖЕ отдаёт `Err` на 1024 полосах — его отдаёт предел
//! `M-71` после построения. Признак «`snapshot` вернул ошибку» доступен обоим мирам, и
//! оракул на нём был бы зелен ни о чём — ровно класс `Р-4` (`oracle-blindness-class` §5).
//!
//! Различающий признак выбран СТРУКТУРНЫЙ, а не временной (время зависит от хоста и дало бы
//! флак — `testing.md` §«гейт меряет свой инвариант, а не окружение»):
//! **`gateway::validate_selector` — чистая функция, она НИЧЕГО не строит.** Если предел живёт
//! в ней, вызов с 1024 полосами вернёт `Err` без единого прочитанного события; предел `M-71`
//! этого сделать не может ПО ПОСТРОЕНИЮ — ему нужен собранный ответ. Мир ¬P признака не
//! несёт структурно, а не по совпадению.
//!
//! Второй сценарий пиннит, что предел ДОСТИЖИМ с пути `snapshot`: текст отказа обязан
//! называть предел ПОЛОС, а не предел ОТВЕТА. Так отличается «отвергли на входе» от
//! «построили и отвергли».
//!
//! # Почему гвард обязан жить в `gateway`, а не в транспорте
//!
//! `Selector` собирают напрямую `research-cli`, чекпоинтер (`M-38b`) и replay — гвард только
//! в `gateway-serve` оставил бы байпас-поверхность. Тот же довод уже посадил в
//! `gateway::validate_selector` проверки `GW-I-10`, `GW-I-14` и каденции `MD-I-8 d14`
//! (`crates/gateway/src/lib.rs`), и спека `M-70` §2.1 запрещает вводить предел только в
//! транспорт отдельной строкой.
//!
//! # Анти-плацебо: чего оракул НЕ примет
//!
//! Реализация «отвергать всё подряд» удовлетворила бы первый сценарий и убила бы продукт:
//! канонический набор `П-014` — семь полос, и он ОБЯЗАН проходить. Это третий сценарий, и он
//! же связывает предел с подписанным составом: предел ниже семи означал бы, что подпись
//! founder'а неисполнима.
//!
//! # Состояние: КРАСНЫЙ ПО ПОСТРОЕНИЮ
//!
//! `MAX_BANDS` в `crates/gateway/src/lib.rs` не существует; `validate_selector` о `bands`
//! молчит. Первый и второй сценарии падают, третий зелен (сегодня проходит вообще всё) —
//! он сторож, а не предмет.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Канонический набор — РЕШЕНИЕ FOUNDER'А (`research/data-quality/depth-verdict.md:15`:
/// «полный TPP-набор 1.5/3/5/8/15/30/60»); то же в `docs/fa/viz-backend.md:42`.
/// Он ОБЯЗАН проходить: предел, отвергающий подписанный состав, делает подпись неисполнимой.
const BANDS_CANONICAL: &[f64] = &[0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

/// **ПРЕДЕЛ, ОБЪЯВЛЕННЫЙ СПЕКОЙ `M-70` §2bis.3** — решение architect'а по делегированию
/// founder'а 2026-09-03. Снизу зажат подписью (`П-014` — семь полос), сверху — долей
/// подписанного предела ответа: 32 полосы ≈ 10 %, 256 ≈ 48 %, а цель `DESIGN` §16 — десять
/// тысяч одновременных подписчиков, и бюджет кадра общий.
/// Число живёт в ОДНОМ месте — в реализации
/// (`gateway::MAX_BANDS`), — а здесь дублируется НАМЕРЕННО и с другой стороны границы:
/// оракул обязан краснеть, если реализация тихо разойдётся со спекой. Тот же приём, что у
/// `EXPECTED_SCHEMA_VERSION` в `red_gateway_schema_version.rs`.
const EXPECTED_MAX_BANDS: usize = 32;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-70 DB-I-3 bands-cap fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel_with(bands: Vec<f64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
        depth_cadence_ms: None,
    }
}

/// `n` РАЗЛИЧНЫХ возрастающих полос внутри `(0, 1)` — ровно та форма, которую сегодня
/// пропускает транспорт: диапазон соблюдён, сортировка соблюдена, дублей нет. Отвергать её
/// нечему, кроме предела на КОЛИЧЕСТВО.
fn many_bands(n: usize) -> Vec<f64> {
    (1..=n)
        .map(|k| k as f64 * (0.6 / (n as f64 + 1.0)))
        .collect()
}

fn lvls(v: &[(f64, f64)]) -> Vec<Level> {
    v.iter()
        .map(|(p, s)| Level {
            price: to_fixed(*p),
            size: to_fixed(*s),
        })
        .collect()
}

/// Книга прод-формы — та же конструкция, что в `red_depth_egress_canonical.rs` и у оракула
/// `M-75`: густо у touch (шаг 0.02 % от mid), хвост до ±60 %.
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

/// `DB-I-3` ЯДРО — отказ наступает В ЧИСТОЙ ФУНКЦИИ, то есть ДО всякого построения.
///
/// Именно этот сценарий отличает исправленный мир от сегодняшнего: предел `M-71` тоже
/// отвергает 1024 полосы, но только собрав 3.5 МБ. `validate_selector` ничего не строит —
/// пройти его ответом нельзя.
#[test]
fn db_i_3_selector_with_too_many_bands_is_rejected_before_any_work() {
    let sel = sel_with(many_bands(EXPECTED_MAX_BANDS + 1));

    let verdict = gateway::validate_selector(&sel);

    assert!(
        verdict.is_err(),
        "DB-I-3 НАРУШЕН: селектор с {} полосами принят чистой проверкой. Значит отказ (если \
         он вообще будет) придёт ПОСЛЕ построения ответа — предел `M-71` срабатывает на \
         собранном кадре. Замер: 1024 полосы = 3 577 553 Б и 3.8 с работы сервера ради \
         отказа; 4096 = 14 077 293 Б и 18.1 с. Это `PL-I-5`: усиление ресурса, управляемое \
         клиентом",
        EXPECTED_MAX_BANDS + 1
    );

    let msg = verdict.unwrap_err().to_string();
    assert!(
        msg.contains("bands") || msg.contains("полос"),
        "отказ обязан НАЗЫВАТЬ предмет: оператор читает его в логе упавшего запроса и должен \
         понять, что превышено число полос, а не гадать. Текст: {msg}"
    );
}

/// `DB-I-3b` — предел ДОСТИЖИМ с прод-пути и отличается от предела ОТВЕТА.
///
/// Пиннит, что `snapshot` действительно зовёт `validate_selector` ДО сборки: текст отказа
/// обязан говорить о полосах, а не `response exceeds limit`. Второе означало бы, что мы
/// снова построили ответ и лишь потом его отвергли.
#[test]
fn db_i_3b_snapshot_path_rejects_by_band_cap_not_by_response_size() {
    let dir = journal_deep_book(60, 3_000);
    let sel = sel_with(many_bands(EXPECTED_MAX_BANDS + 1));

    let err = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel,
        Cursor::LATEST,
    )
    .expect_err(
        "снимок с превышенным числом полос обязан быть отвергнут — иначе клиент управляет \
             объёмом работы сервера",
    )
    .to_string();

    assert!(
        !err.contains("response exceeds limit"),
        "отказ пришёл от предела ОТВЕТА (`M-71`), а не от предела ПОЛОС: сервер собрал кадр и \
         только потом его отверг. Работа уже потрачена — ровно то, что `DB-I-3` обязан снять. \
         Текст: {err}"
    );
    assert!(
        err.contains("bands") || err.contains("полос"),
        "отказ с пути `snapshot` обязан называть предел полос. Текст: {err}"
    );
}

/// АНТИ-ПЛАЦЕБО — подписанный состав ОБЯЗАН проходить.
///
/// Реализация «отвергать всё» удовлетворила бы оба сценария выше и уничтожила бы продукт:
/// `П-014` п.4 подписал СЕМЬ полос. Предел ниже семи делает подпись founder'а неисполнимой,
/// и это дефект предела, а не подписи.
#[test]
fn db_i_3c_signed_canonical_set_is_accepted() {
    assert!(
        BANDS_CANONICAL.len() <= EXPECTED_MAX_BANDS,
        "СПЕКА ПРОТИВОРЕЧИТ ПОДПИСИ: предел {EXPECTED_MAX_BANDS} ниже канонического состава \
         из {} полос (`П-014` п.4). Чинить предел, а не подпись",
        BANDS_CANONICAL.len()
    );

    gateway::validate_selector(&sel_with(BANDS_CANONICAL.to_vec())).expect(
        "канонический набор `П-014` ОБЯЗАН проходить проверку: предел, отвергающий \
         подписанный состав, делает подпись неисполнимой",
    );

    let dir = journal_deep_book(60, 3_000);
    let snap = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel_with(BANDS_CANONICAL.to_vec()),
        Cursor::LATEST,
    )
    .expect("канонический набор обязан отдавать снимок, а не отказ");

    assert_eq!(
        snap.series.depth_series.len(),
        BANDS_CANONICAL.len() * 2,
        "SETUP НЕ СОСТОЯЛСЯ: строк депт-серии {}, ожидалось {} (полоса × сторона). Если \
         полосы не доезжают до серии, «канонический набор принят» ничего не доказывает",
        snap.series.depth_series.len(),
        BANDS_CANONICAL.len() * 2
    );
}

/// ГРАНИЦА проверяется С ОБЕИХ СТОРОН (`testing.md` §«Дегенерированный вход» п.4): ровно
/// предел — законен, предел плюс один — отказ. Фикстура не стоит НА границе двусмысленно.
#[test]
fn db_i_3d_boundary_is_inclusive_and_exact() {
    gateway::validate_selector(&sel_with(many_bands(EXPECTED_MAX_BANDS))).expect(
        "РОВНО предел обязан приниматься: «не более N» значит, что N законно. Отказ здесь — \
         смещение границы на единицу, классическая ошибка предела",
    );
    assert!(
        gateway::validate_selector(&sel_with(many_bands(EXPECTED_MAX_BANDS + 1))).is_err(),
        "предел плюс один обязан отвергаться — иначе предела нет"
    );
}
