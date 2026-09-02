//! RED `PL-I-4`/`PL-I-5` (sacred, architect-only) — **ПРЕДЕЛ ОБЪЁМА ОТВЕТА, fail-closed.**
//!
//! Милестоун `milestones/M-71-egress-cap.md`. Это ПЕРВЫЕ оракулы двух инвариантов, объявленных
//! в `docs/DESIGN.md` §22 и до сих пор числящихся там как «будущие RED-оракулы, PENDING»:
//!
//! * **`PL-I-5`** — «Тарифные лимиты enforce'ятся сервером; **отсутствие/невалидность лимита =
//!   отказ, не unbounded** (урок R7)»;
//! * **`PL-I-4`** — «Пользовательский запрос не читает event-store напрямую — только через
//!   проекции/gateway **с cap и квотами** (N клиентов ≠ N сканов журнала)».
//!
//! # Дефект работает СЕГОДНЯ — это не подготовка к будущему
//!
//! Селектор приходит ОТ КЛИЕНТА в теле подписки (`gateway-serve/src/wire_v1.rs:5`,
//! `parse_selector` `:120`). Единственная проверка полосы — `0 < b < 1`
//! (`gateway-serve/src/session.rs:79-82`); `gateway::validate_selector` о `bands` не знает
//! вовсе (`crates/gateway/src/lib.rs:1878-1917`). Окно heatmap при этом равно
//! `max(selector.bands)` (`:1192`). Ограничения на РАЗМЕР ответа в проекте нет ни одного:
//! `grep -rniE 'max_frame|max_message|max_size|frame_size|max_send' crates/gateway-serve/src`
//! даёт ноль совпадений. Существующий `DEFAULT_MAX_SUBSCRIPTIONS = 16` ограничивает ЧИСЛО
//! подписок, а не размер каждой, — то есть умножает проблему.
//!
//! Замер architect'а на геометрии прод-книги (бакеты 0.02 %, охват ±60 %, 60 временных
//! бакетов): `bands=[0.001]` → 62 688 Б; `bands=[0.99]` → **45 242 536 Б (43.2 МБ)**.
//! Усиление **×722** одним сообщением подписки, без смены прод-конфига.
//!
//! # Ресурс — ПОЛНЫЙ СЕРИАЛИЗОВАННЫЙ ОТВЕТ, и это правка по `C-157` R1
//!
//! Первая редакция судила `series.heatmap.len()`. Критик предъявил обход ИСПОЛНЕНИЕМ, и я
//! воспроизвёл его сам: **25 000 сделок и НИ ОДНОГО L2-события** дают `heatmap = 0`,
//! `cob = 0` — и при этом `volume_profile[0].bins = 25 000`, `volume_bubbles = 25 000`,
//! ответ **2 804 765 Б (2.67 МБ)**. Предел на ячейки heatmap такой ответ не видит вовсе.
//!
//! Поэтому ресурс здесь — **байты сериализованного ответа**, а не одна его часть и не ширина
//! полосы. Это не произвольный выбор единицы: `gateway-serve` кладёт на провод именно
//! `serde_json::to_vec(&Snapshot)` целиком (`docs/fa/viz-backend.md` §5, `VB-I-6`), то есть
//! байты и есть та величина, которой платит сеть.
//!
//! Ширина полосы — ПРОКСИ вдвойне: она не видит ни плотности книги, ни сделок вовсе
//! (`testing.md`: «оракул границы ресурса меряет ресурс, а не прокси»). Реализация вправе
//! ограничивать что угодно внутри — оракулы судят БАЙТЫ.
//!
//! # Числа предела в оракулах НЕТ, и это намеренно
//!
//! Величина предела — продуктовое решение того же класса, что `DEFAULT_MAX_SUBSCRIPTIONS = 16`
//! («подписанная норма»), и назначается founder'ом (спека §5.1). Фикстуры разведены на три
//! порядка (сотня ячеек против десятков тысяч), поэтому набор судит ПОВЕДЕНИЕ при любом
//! разумном пределе и не протухнет от смены числа. Побочная выгода: оракулы не COMPILE-RED —
//! они не ссылаются на ещё не существующую константу, и потому предъявимы КРАСНЫМИ, а не
//! «не собралось» (урок `M-68` rev2, `C-138` п.3).

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Шаг бакета venue (`BUCKET_WIDTH`) — 0.02 % от mid.
const STEP: f64 = 0.0002;
/// Охват эмиссии venue (`MAX_REL_DIST`) — ±60 %.
const REACH: f64 = 0.60;
/// Временных бакетов в фикстуре. Прод-окно кокпита — десятки; десяти достаточно, чтобы
/// «широкий» случай ушёл на три порядка от «узкого», и тест остаётся быстрым.
const N_BUCKETS: i64 = 10;

/// Прод-дефолт `GATEWAY_BANDS` (`docker-compose.yml:134,203`) — честная рабочая нагрузка.
const PROD_BAND: f64 = 0.001;
/// Полоса, которую сегодня ПРИНИМАЕТ проверка `0 < b < 1` и которая даёт книгу целиком.
const ABUSIVE_BAND: f64 = 0.99;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 24,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "PL-I-5 egress-cap fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// **Гигиена процессно-глобального окна (`C-198` B-5).** Эффективное окно heatmap/COB —
/// состояние ПРОЦЕССА, а тесты этого файла намеренно используют РАЗНЫЕ охваты: одни строят
/// заведомо огромный ответ, другой (`pl_i_5_e`) проверяет, что прод-дефолтный селектор
/// обслуживается. При параллельном исполнении широкий сосед перезаписывал окно под ногами у
/// узкого, и `pl_i_5_e` падал, НЕ БУДУЧИ сломанным.
///
/// Замер, доказавший, что это гонка, а не дефект: `cargo test --test red_egress_cap
/// -- --test-threads=1` → `9 passed; 0 failed`, тогда как параллельный прогон давал три
/// падения. Приём и причина — те же, что в `red_egress_cap_governed.rs:66`.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn sel(bands: Vec<f64>) -> Selector {
    // M-75 (`C-198` B-5): окно heatmap/COB БОЛЬШЕ НЕ выводится из `Selector.bands` — оно
    // серверное. Оракулы этого файла строились, когда окно было `max(bands)`, и их предмет
    // (глубина карты/COB, объём ответа) от охвата ЗАВИСИТ. Приём восстановления предмета:
    // ставим СЕРВЕРНОЕ окно равным тому, что прежде давал селектор, — смысл каждого теста
    // сохраняется дословно, меняется лишь ИСТОЧНИК величины.
    //
    // Настройка процессно-глобальна ⇒ тесты, зависящие от охвата, идут под `serial()`.
    let w = bands.iter().copied().fold(0.0_f64, f64::max);
    if w > 0.0 {
        gateway::set_effective_heatmap_window_frac(w);
    }
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
        // M-68 задача 22: поле добавлено в Selector; `None` = пер-событийно,
        // то есть НЕЙТРАЛ — прежняя семантика этого теста сохранена бит-в-бит.
        depth_cadence_ms: None,
    }
}

/// Книга ПРОД-ФОРМЫ: уровни через `STEP` до `REACH` по обеим сторонам. Форма снята с кода
/// venue, а не придумана: `bucket_levels` + `MAX_REL_DIST = 0.60`
/// (`crates/venue-binance/src/lib.rs:33,398-399`).
///
/// **Смещение на полшага — не косметика** (`testing.md` §«Дегенерированный вход» п.4:
/// «фикстура не должна стоять РОВНО на границе диапазона: округление уводит её в соседний,
/// и тест падает по неверной причине»). Без него уровень `k = 5` ложится ТОЧНО на край
/// полосы `0.001`, порог bid'а (`mid*(1−b)`) и ask'а (`mid*(1+b)`) округляются в разные
/// стороны, и счёт получается асимметричным: замер до правки давал 90 ячеек вместо 100 —
/// 4 уровня на одной стороне против 5 на другой. Полшага уводят все уровни с границы, и
/// эталон оракула `B` становится однозначным.
fn journal_prod_shape(levels_per_side: usize, buckets: i64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    let bids: Vec<Level> = (1..=levels_per_side)
        .map(|k| lvl(MID * (1.0 - STEP * (k as f64 - 0.5)), 1.0))
        .collect();
    let asks: Vec<Level> = (1..=levels_per_side)
        .map(|k| lvl(MID * (1.0 + STEP * (k as f64 - 0.5)), 1.0))
        .collect();
    for b in 0..buckets {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: bids.clone(),
                asks: asks.clone(),
                ts_exch_ms: T0 + b * 1_000,
            },
        ))
        .expect("append");
    }
    j.flush().expect("flush");
    dir
}

/// Глубина книги, используемая оракулом полноты: столько же, сколько у `deep_book`.
fn per_side_levels() -> usize {
    (REACH / STEP) as usize
}

/// Та же прод-форма, но с ОДНОЙ сделкой в каждом временном бакете. Нужна оракулу `B`:
/// полнота принятого ответа проверяется по ДВУМ разным частям (heatmap и OHLCV), а
/// `journal_prod_shape` сделок не эмитит вовсе, и OHLCV в ней пуст по построению (`C-159` R2).
fn journal_prod_shape_with_trades(levels_per_side: usize, buckets: i64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    let bids: Vec<Level> = (1..=levels_per_side)
        .map(|k| lvl(MID * (1.0 - STEP * (k as f64 - 0.5)), 1.0))
        .collect();
    let asks: Vec<Level> = (1..=levels_per_side)
        .map(|k| lvl(MID * (1.0 + STEP * (k as f64 - 0.5)), 1.0))
        .collect();
    for b in 0..buckets {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: bids.clone(),
                asks: asks.clone(),
                ts_exch_ms: T0 + b * 1_000,
            },
        ))
        .expect("append snapshot");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID),
                size: to_fixed(1.0),
                side: Side::Buy,
                ts_exch_ms: T0 + b * 1_000 + 500,
            },
        ))
        .expect("append trade");
    }
    j.flush().expect("flush");
    dir
}

fn deep_book() -> tempfile::TempDir {
    journal_prod_shape((REACH / STEP) as usize, N_BUCKETS)
}

/// ПЛОТНЫЙ НЕ-heatmap ресурс: 25 000 сделок с РАЗНЫМИ ценами и НИ ОДНОГО L2-события.
/// `heatmap = 0`, `cob = 0`, но `vp_bins = 25 000`, `bubbles = 25 000`, ответ ≈ 2.8 МБ.
/// Ровно тот ресурс, который `C-158` R1 предъявил на непокрытых формах как 2 804 666 Б.
fn dense_trades() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..25_000i64 {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID + i as f64 * 0.01),
                size: to_fixed(1.0),
                side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                ts_exch_ms: T0 + i,
            },
        ))
        .expect("append trade");
    }
    j.flush().expect("flush");
    dir
}

/// Величина ресурса — ровно то, что уходит на провод (`serde_json::to_vec` в `gateway-serve`).
fn response_bytes(s: &gateway::Snapshot) -> usize {
    serde_json::to_vec(s)
        .expect("Snapshot сериализуем — иначе он не мог бы уйти клиенту")
        .len()
}

/// Сумма ВСЕХ выдаваемых сущностей — для сообщений об ошибке: она называет, ЧЕМ именно
/// раздут ответ, а байты одни этого не показывают.
fn entity_counts(s: &gateway::Snapshot) -> String {
    let vp_bins: usize = s.series.volume_profile.iter().map(|r| r.bins.len()).sum();
    format!(
        "heatmap={} cob={} vp_bins={} bubbles={} ohlcv={} depth_rows={}",
        s.series.heatmap.len(),
        s.series.cob.len(),
        vp_bins,
        s.series.volume_bubbles.len(),
        s.series.ohlcv.len(),
        s.series.depth_series.len()
    )
}

fn snapshot(dir: &std::path::Path, bands: Vec<f64>) -> std::io::Result<gateway::Snapshot> {
    gateway::snapshot(
        dir,
        EpochFilter::OwnCaptureOnly,
        &sel(bands),
        Cursor::LATEST,
    )
}

/// Все целые ≥ 1000, встречающиеся в тексте, — для оракула F.
fn big_numbers(msg: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in msg.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else {
            if let Ok(v) = cur.parse::<u64>() {
                if v >= 1_000 {
                    out.push(v);
                }
            }
            cur.clear();
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// E — АНТИ-ЛОЖНОЕ-КРАСНОЕ. Идёт первым намеренно: страж, мешающий работать, выключат первым
// же «он мешает», и тогда предмет остальных оракулов исчезнет вместе с ним.
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **E — честная работа не ломается: прод-дефолт обслуживается.**
///
/// `GATEWAY_BANDS=0.001` — то, что крутится на проде прямо сейчас. Оракул валит переширокую
/// заглушку «всегда `Err`», и он же — половина парного vantage: A валит «всегда `Ok`».
/// Без E набор был бы зелен против реализации, отвергающей вообще всё.
#[test]
fn pl_i_5_e_prod_default_selector_is_served() {
    let _g = serial(); // C-198 B-5: окно heatmap процессно-глобально
    let dir = deep_book();
    let got = snapshot(dir.path(), vec![PROD_BAND]);
    let s = got.expect(
        "PL-I-5 E: прод-дефолт GATEWAY_BANDS=0.001 обязан обслуживаться. Предел ставится \
         против ЗЛОУПОТРЕБЛЕНИЯ, а не против работы; отказ здесь означает, что страж мешает \
         честной нагрузке — а такого стража выключат первым же «он мешает», и вместе с ним \
         исчезнет вся защита",
    );
    assert!(
        !s.series.heatmap.is_empty(),
        "PL-I-5 E SETUP НЕ СОСТОЯЛСЯ: ответ пуст — фикстура не построила предмет, и «принят» \
         неотличимо от «пуст»"
    );
    // Узкий ответ обязан быть на порядки дешевле широкого — это и делает набор независимым от
    // ЧИСЛА предела (его назначает founder, спека §5.1).
    let b = response_bytes(&s);
    assert!(
        b < 100_000,
        "PL-I-5 E SETUP НЕ СОСТОЯЛСЯ: узкий ответ весит {b} Б — фикстура не разводит узкий и \
         широкий случаи на порядки, и оракулы ниже начинают зависеть от точной величины \
         предела. Состав: {}",
        entity_counts(&s)
    );
}

/// **E-2 — вырожденный вход не отвергается.** Пустая и односторонняя книга — легитимные
/// состояния (старт процесса, ресинк, односторонний рынок). Реализация, отвергающая их
/// «на всякий случай», ломает работу там, где ресурса не тратится вовсе.
#[test]
fn pl_i_5_e2_degenerate_books_are_served() {
    let _g = serial(); // C-198 B-5: окно heatmap процессно-глобально
    let empty = journal_prod_shape(0, N_BUCKETS);
    snapshot(empty.path(), vec![ABUSIVE_BAND])
        .expect("PL-I-5 E-2: пустая книга не тратит ресурса — отвергать её нечем и не за что");

    // Односторонняя книга: только bid'ы.
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![lvl(MID * (1.0 - STEP), 1.0)],
                asks: vec![],
                ts_exch_ms: T0,
            },
        ))
        .expect("append");
        j.flush().expect("flush");
    }
    snapshot(dir.path(), vec![PROD_BAND])
        .expect("PL-I-5 E-2: односторонняя книга — штатное состояние, не повод для отказа");
}

/// Верхняя граница ОБЪЯВЛЕННОГО рабочего диапазона по числу полос (`A-021`, Вопрос 3).
/// `CT-RFC-09` §2.7 максимума не задаёт; это НЕ новый предел протокола, а диапазон, который
/// набор ДОКАЗЫВАЕТ. Восьмиполосный случай `C-158` входит сюда частным случаем.
const N_MAX_BANDS: usize = 12;

/// `n` узких полос, кратно растущих от `base`. Все внутри `base * 2^(n-1)`, то есть окно
/// выдачи задаёт последняя — самая широкая.
fn narrow_bands(n: usize, base: f64) -> Vec<f64> {
    (0..n).map(|k| base * (1u64 << k) as f64).collect()
}

/// **E-3 — СЕМЕЙСТВО честных многополосных запросов, а не один экземпляр** (`A-021` Предл. 2).
///
/// # Почему семейство, а не ещё одна фикстура
///
/// Два круга подряд контроль честной нагрузки был ФИКСИРОВАННЫМ экземпляром, и каждый раз
/// прокси на единицу шире его переживал: `C-157` — контроль на ОДНУ полосу, прокси
/// «reject при >1» зелен; `C-158` — контроль на СЕМЬ, прокси `bands.len() <= 7` зелен, а
/// валидный восьмиполосный запрос (152 588 Б, 13× запаса) отвергнут. Правило границы `A-020`
/// запрещает девятую полосу как ответ: меняется КОНСТРУКЦИЯ.
///
/// # Форма, принятая арбитром — и его же поправка ко мне
///
/// Я утверждал, что парная проверка убивает «любой прокси независимо от порога». **Это
/// неверно, и арбитр это назвал:** пара с кардинальностями (n, n+1) убивает прокси только с
/// порогом < n+1; прокси с порогом выше любой конечной фикстуры не фальсифицируем конечным
/// набором. Принятая форма — семейство по ОБЪЯВЛЕННОМУ диапазону `1..=N_MAX_BANDS`, а остаток
/// назван честно: **прокси с порогом выше `N_MAX_BANDS` этот набор не ловит —
/// `COGNITIVE-ONLY`.** Диапазон объявлен спекой; расширять его — правка спеки, не теста.
#[test]
fn pl_i_5_e3_family_of_honest_multi_band_requests_is_served() {
    let _g = serial(); // C-198 B-5: окно heatmap процессно-глобально
    let dir = journal_prod_shape(400, 4);
    let base = 0.0001_f64;

    let widest = *narrow_bands(N_MAX_BANDS, base).last().expect("непусто");
    let single = snapshot(dir.path(), vec![widest])
        .expect("PL-I-5 E-3 SETUP: одна полоса максимальной ширины обязана обслуживаться");
    let single_b = response_bytes(&single);

    for n in 1..=N_MAX_BANDS {
        let bands = narrow_bands(n, base);
        let got = snapshot(dir.path(), bands.clone()).unwrap_or_else(|e| {
            panic!(
                "PL-I-5 E-3: запрос из {n} валидных полос ОТВЕРГНУТ ({e}). Полосы отсортированы, \
                 без дублей, все в (0,1) — `CT-RFC-09` §2.7 их принимает, и окно выдачи задаёт \
                 самая широкая из них ({widest}), то есть ресурс тот же, что у одной полосы. \
                 Отвергать положено по ВЕЛИЧИНЕ ОТВЕТА, а не по ЧИСЛУ полос: прокси на \
                 `bands.len()` — ровно дефект, который C-158 предъявил мутацией."
            )
        });
        let b = response_bytes(&got);
        assert!(
            b < 4 * single_b.max(1),
            "PL-I-5 E-3: {n} узких полос дали {b} Б против {single_b} Б у одной полосы той же \
             максимальной ширины. Разница обязана быть в разы, а не в порядки — цена берётся \
             за ОБЪЁМ. Состав: {}",
            entity_counts(&got)
        );
    }
}

/// **E-4 — вторая ось семейства: РАВНЫЕ байты, ПРОТИВОПОЛОЖНЫЙ состав** (`A-021` Предл. 2 п.2).
///
/// Два запроса примерно равного веса, но собранные из разных частей ответа: один
/// heatmap-тяжёлый (книга, без сделок), другой trades-тяжёлый (сделки, пустой heatmap).
/// Вердикт обязан быть ОДИНАКОВЫМ. Прокси, ограничивающий одну часть ответа, разводит их — и
/// краснеет здесь. Это анти-ложно-КРАСНАЯ пара к `A-2`: та требует ОТВЕРГАТЬ плотные сделки,
/// эта — НЕ отвергать их, пока они дёшевы.
#[test]
fn pl_i_5_e4_equal_bytes_opposite_composition_get_the_same_verdict() {
    // C-198 B-5: окно heatmap процессно-глобально
    let _g = serial();
    // heatmap-тяжёлый: книга, ноль сделок
    let hm = journal_prod_shape(300, 4);
    // trades-тяжёлый: сделки, ноль L2-событий
    let tr = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(tr.path(), cfg()).expect("open_with");
        for i in 0..3_000i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(MID + i as f64 * 0.01),
                    size: to_fixed(1.0),
                    side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    ts_exch_ms: T0 + i,
                },
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }

    let a = snapshot(hm.path(), vec![ABUSIVE_BAND]);
    let b = snapshot(tr.path(), vec![PROD_BAND]);

    let (ba, bb) = (
        a.as_ref().map(response_bytes).unwrap_or(0),
        b.as_ref().map(response_bytes).unwrap_or(0),
    );
    // SETUP: пара имеет смысл, только если веса СОПОСТАВИМЫ. Иначе разные вердикты
    // объясняются разным объёмом, а не разным составом.
    if a.is_ok() && b.is_ok() {
        assert!(
            ba.max(bb) < 8 * ba.min(bb).max(1),
            "PL-I-5 E-4 SETUP НЕ СОСТОЯЛСЯ: веса пары разошлись ({ba} Б против {bb} Б) — \
             сравнивать вердикты нельзя, различие объяснимо объёмом"
        );
    }
    assert_eq!(
        a.is_ok(),
        b.is_ok(),
        "PL-I-5 E-4: ответы сопоставимого веса ({ba} Б heatmap-тяжёлый против {bb} Б \
         trades-тяжёлый) получили РАЗНЫЕ вердикты. Значит предел судит не ВЕЛИЧИНУ, а одну \
         часть ответа — тот самый прокси, из-за которого набор дважды пропускал обход \
         (C-157 R1, C-158 R1)."
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// A — ПРЕДЕЛ ДЕЙСТВУЕТ, и меряется он РЕСУРСОМ
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **A — запрос, раздувающий ответ, ОТВЕРГАЕТСЯ, а не обслуживается.**
///
/// `bands=[0.99]` проходит сегодняшнюю проверку `0 < b < 1` и даёт книгу целиком: замер
/// architect'а — 43.2 МБ на один ответ, ×722 к прод-дефолту. Клиенту для этого достаточно
/// одного сообщения подписки.
#[test]
fn pl_i_5_a_oversized_response_is_refused_not_served() {
    let _g = serial(); // C-198 B-5: окно heatmap процессно-глобально
    let dir = deep_book();
    let narrow = snapshot(dir.path(), vec![PROD_BAND]).expect("узкий обязан обслуживаться");

    match snapshot(dir.path(), vec![ABUSIVE_BAND]) {
        Err(_) => {}
        Ok(s) => panic!(
            "PL-I-5 НАРУШЕН: bands=[{ABUSIVE_BAND}] обслужен — {} ячеек heatmap против {} у \
             прод-дефолта (×{:.0}). Селектор приходит ОТ КЛИЕНТА (wire_v1.rs:5), проверяется \
             только `0 < b < 1` (session.rs:79-82), предела на РАЗМЕР ответа нет ни одного. \
             DESIGN §22 PL-I-5: «отсутствие лимита = отказ, не unbounded». DESIGN §16: прод \
             деградирует уже при 1-2 зрителях, а подписок на соединение разрешено 16.",
            s.series.heatmap.len(),
            narrow.series.heatmap.len(),
            s.series.heatmap.len() as f64 / narrow.series.heatmap.len().max(1) as f64
        ),
    }
}

/// **A-2 — ответ, раздутый НЕ heatmap'ом, тоже отвергается** (`C-157` R1).
///
/// Центральная находка круга 1, и она моя ошибка: первая редакция судила `heatmap.len()`, а
/// `SeriesBundle` несёт на провод ещё `volume_profile[].bins`, `volume_bubbles`, `ohlcv`,
/// `cob`, `depth_series`, CVD и VWAP. Замер (мой, воспроизводит предъявленный критиком):
/// **25 000 сделок и НИ ОДНОГО L2-события** ⇒ `heatmap = 0`, `cob = 0`, но
/// `vp_bins = 25 000`, `bubbles = 25 000`, ответ **2.67 МБ**.
///
/// Предел на одну часть ответа — не предел. Этот оракул краснеет против ЛЮБОЙ реализации,
/// ограничившей heatmap и забывшей остальное.
#[test]
fn pl_i_5_a2_dense_non_heatmap_response_is_refused() {
    let _g = serial(); // C-198 B-5: окно heatmap процессно-глобально
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        // РАЗНЫЕ цены: каждая заводит свой bin профиля объёма и свой bubble. L2-событий нет
        // вовсе, поэтому heatmap и COB останутся пустыми — предмет оракула именно в этом.
        for i in 0..25_000i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(MID + i as f64 * 0.01),
                    size: to_fixed(1.0),
                    side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    ts_exch_ms: T0 + i,
                },
            ))
            .expect("append trade");
        }
        j.flush().expect("flush");
    }

    match snapshot(dir.path(), vec![PROD_BAND]) {
        Err(_) => {}
        Ok(s) => panic!(
            "PL-I-5 A-2 НАРУШЕН: ответ {} Б обслужен при ПУСТОМ heatmap. Состав: {}. \
             Предел, считающий только ячейки heatmap, этот ответ не видит вовсе — а на провод \
             он уходит целиком (`serde_json::to_vec(&Snapshot)`). Ограничивать положено ПОЛНЫЙ \
             ответ, а не одну его часть; селектор здесь прод-дефолтный, то есть путь достижим \
             без всякого злоупотребления шириной полосы.",
            response_bytes(&s),
            entity_counts(&s)
        ),
    }
}

/// **B — отказ ЯВНЫЙ; принятый ответ ПОЛОН.**
///
/// Соблазнительный «фикс» — усечь ответ до предела и отдать. Он зелен во всех liveness-
/// проверках (`healthy`, heartbeat, рост журнала) и врёт клиенту молча: `PL-I-7` —
/// «деградация никогда не выдаётся за норму». Оракул закрывает обе стороны: превышение даёт
/// `Err`, а НЕ урезанный `Ok`; принятый запрос отдаёт ровно столько, сколько есть в книге.
#[test]
fn pl_i_5_b_no_silent_truncation_on_either_side() {
    let _g = serial(); // C-198 B-5: окно heatmap процессно-глобально
    let dir = deep_book();

    // (1) превышение обязано быть ОШИБКОЙ, а не урезанным успехом
    if let Ok(s) = snapshot(dir.path(), vec![ABUSIVE_BAND]) {
        panic!(
            "PL-I-5 B: превышение вернуло Ok на {} Б ({}). Если это усечение — оно МОЛЧАЛИВОЕ: \
             клиент получил неполную книгу под видом полной (PL-I-7). Отказ обязан быть явным; \
             усечение, если оно вообще допускается, обязано быть ПОМЕЧЕНО в ответе.",
            response_bytes(&s),
            entity_counts(&s)
        );
    }

    // (2) принятый запрос НЕ урезан: число ячеек в точности равно геометрии фикстуры.
    // Уровней внутри ±0.1 % при шаге 0.02 % со смещением на полшага: (k−0.5)*STEP <= PROD_BAND
    // ⇒ k = 1..=5, обе стороны ⇒ 10 на бакет; бакетов N_BUCKETS. Эталон вычислен из ГЕОМЕТРИИ
    // ФИКСТУРЫ, а не из той же функции, что строит ответ (`testing.md`, «зависимый эталон»).
    let per_side = (PROD_BAND / STEP + 0.5).floor() as usize;
    let expected = per_side * 2 * N_BUCKETS as usize;
    // `C-159` R2 — МОЁ ложное КРАСНОЕ, найденное критиком. Прежняя редакция требовала здесь
    // `ohlcv.len() == N_BUCKETS` от фикстуры `journal_prod_shape`, которая эмитит ТОЛЬКО
    // `L2Snapshot` и ни одной сделки: OHLCV в ней ноль по построению. Сегодня ассерт не
    // исполняется — тест падает раньше, на части (1). Но как только предел появится, часть (1)
    // пройдёт, и набор станет НЕВОЗМОЖНО сделать зелёным честной реализацией: она упёрлась бы
    // в неверный ОЖИДАЕМЫЙ ФАКТ, а не в поведение. Ровно тот класс, против которого написаны
    // `E`/`E-2`/`E-3`/`E-4`, — и я внёс его сам.
    //
    // Лечение — фикстура, в которой полнота ЕСТЬ что проверять по двум разным частям ответа:
    // те же L2-снимки ПЛЮС по одной сделке на бакет. Проверять полноту по одной части значило
    // бы повторить исходный дефект `C-157` R1 в миниатюре.
    let mixed = journal_prod_shape_with_trades(per_side_levels(), N_BUCKETS);
    let s = snapshot(mixed.path(), vec![PROD_BAND]).expect("узкий обязан обслуживаться");
    assert_eq!(
        s.series.ohlcv.len(),
        N_BUCKETS as usize,
        "PL-I-5 B: OHLCV урезан — {} баров при {N_BUCKETS} бакетах фикстуры, в каждом по сделке. \
         Ответ, прошедший предел, обязан быть ПОЛНЫМ во всех своих частях",
        s.series.ohlcv.len()
    );
    assert_eq!(
        s.series.heatmap.len(),
        expected,
        "PL-I-5 B: принятый запрос отдал {} ячеек при {expected} по геометрии фикстуры \
         ({per_side} уровней на сторону × 2 × {N_BUCKETS} бакетов). Ответ, который прошёл \
         предел, обязан быть ПОЛНЫМ — иначе предел молча режет честную нагрузку.",
        s.series.heatmap.len()
    );
}

/// **C (`PL-I-4`) — БАЙПАСА НЕТ: предел действует на КАЖДОМ строителе ответа.**
///
/// # Что было неверно в первой редакции (`C-157` R2)
///
/// Оракул звал только `frames_since` и `snapshot_from_checkpoint`. Критик предъявил
/// исполнением, что **живой WS-путь** — `LiveReducer::resume → pump → snapshot` — принимает
/// `bands=[0.99]` и строит больше 50 000 ячеек. Это не теоретическая дыра: именно этим путём
/// `gateway-serve` обслуживает `subscribe` и все последующие кадры, то есть ПРОД-путь
/// оставался открытым, а набор — зелёным.
///
/// # Перечень закрыт ГРЕПОМ, а не памятью
///
/// ```text
/// $ grep -nE '^pub fn (snapshot|frames_since|frames_since_with_stats|replay)\(' src/lib.rs
/// $ grep -nE '^    pub fn (resume|pump|snapshot)\(' src/lib.rs      # LiveReducer
/// ```
///
/// Шесть публичных строителей, принимающих `Selector` прямо или через `LiveReducer`. Оракул
/// бьёт по каждому. Появился седьмой — он обязан появиться и здесь; иначе перечень «по
/// построению» покрывает лишь то, о чём вспомнил автор.
///
/// # Почему предел живёт в `gateway`, а не в транспорте
///
/// `Selector` собирают напрямую чекпоинтер (M-38b), shared-tailer (M-39), `research-cli` и
/// replay. Гвард, посаженный только в `gateway-serve`, оставил бы им открытую дверь — тот же
/// довод, которым `GW-I-14` посажен в `gateway::validate_selector`
/// (`crates/gateway/src/lib.rs:1893-1905`), а не в `serve_config_from_env`.
#[test]
fn pl_i_4_c_limit_has_no_bypass_across_entry_points() {
    // C-198 B-5: окно heatmap процессно-глобально
    let _g = serial();
    // ДВА РАЗНЫХ РЕСУРСА через КАЖДУЮ дверь (`A-021`, обязательный набор п.1). Первая
    // редакция гоняла только широкую книгу, и реализация, отвергающая её и пропускающая
    // плотные сделки, оставалась зелёной — `C-158` R1 предъявил это исполнением.
    let wide = deep_book();
    let dense = dense_trades();
    for (name, dir, bands) in [
        ("широкая книга", wide.path(), vec![ABUSIVE_BAND]),
        ("плотные сделки", dense.path(), vec![PROD_BAND]),
    ] {
        let s = sel(bands);

        // (1) snapshot
        if let Ok(v) = gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST) {
            panic!(
                "PL-I-4 C [{name}]: `snapshot` обслужил {} Б ({})",
                response_bytes(&v),
                entity_counts(&v)
            );
        }

        // (2) frames_since — судятся БАЙТЫ КАДРА, а не факт возврата: кадр уходит на провод
        // целиком, и его величина есть тот же ресурс (`C-158` R1: 2 804 666 Б).
        if let Ok((frames, _)) = gateway::frames_since(
            dir,
            EpochFilter::OwnCaptureOnly,
            &s,
            Cursor::START,
            usize::MAX,
        ) {
            let worst = frames
                .iter()
                .map(|f| serde_json::to_vec(f).expect("Frame сериализуем").len())
                .max()
                .unwrap_or(0);
            panic!(
                "PL-I-4 C [{name}]: `frames_since` отдал {} кадров, крупнейший — {worst} Б. \
                 Push-путь — тот, которым идёт основной трафик после первого снапшота.",
                frames.len()
            );
        }

        // (3) frames_since_with_stats — отдельная дверь той же формы
        assert!(
            gateway::frames_since_with_stats(
                dir,
                EpochFilter::OwnCaptureOnly,
                &s,
                Cursor::START,
                usize::MAX,
            )
            .is_err(),
            "PL-I-4 C [{name}]: `frames_since_with_stats` обслужил раздувающий запрос"
        );

        // (4) replay
        assert!(
            gateway::replay(
                dir,
                EpochFilter::OwnCaptureOnly,
                &s,
                Cursor::START,
                Cursor::LATEST,
            )
            .is_err(),
            "PL-I-4 C [{name}]: `replay` обслужил раздувающий запрос"
        );

        // (5) warm-путь: чекпоинт снимается по расписанию — обычный прод-путь
        let ckpt = tempfile::tempdir().expect("ckpt tempdir");
        assert!(
            gateway::snapshot_from_checkpoint(
                dir,
                EpochFilter::OwnCaptureOnly,
                &s,
                ckpt.path(),
                Cursor::LATEST,
            )
            .is_err(),
            "PL-I-4 C [{name}]: `snapshot_from_checkpoint` обслужил раздувающий запрос"
        );

        // (6) ЖИВОЙ WS-ПУТЬ — им `gateway-serve` отвечает на `subscribe`. Отказ вправе
        // случиться на `resume` ЛИБО на `pump`; недопустимо пройти цепочку целиком.
        let ckpt_live = tempfile::tempdir().expect("ckpt tempdir");
        if let Ok((mut lr, _)) =
            gateway::LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &s, ckpt_live.path())
        {
            if let Ok((frames, _, _)) = lr.pump(dir, EpochFilter::OwnCaptureOnly, usize::MAX) {
                let worst = frames
                    .iter()
                    .map(|f| serde_json::to_vec(f).expect("Frame сериализуем").len())
                    .max()
                    .unwrap_or(0);
                let snap = lr.snapshot();
                panic!(
                    "PL-I-4 C [{name}] НАРУШЕН на ЖИВОМ WS-ПУТИ: resume → pump → snapshot прошёл \
                     целиком; крупнейший кадр {worst} Б, снапшот {} Б ({}). Это ПРОД-путь \
                     подписки; реализация, закрывшая библиотечные вызовы и оставившая его \
                     открытым, зеленит остальной набор и не чинит ничего.",
                    response_bytes(&snap),
                    entity_counts(&snap)
                );
            }
        }
    }
}

/// **F — отказ НАЗЫВАЕТ предел и полученную величину.**
///
/// Немой `Err` оставляет клиента без единственного, что ему нужно: что просить вместо этого.
/// Прецедент требования — `GW-I-14`: «отказ обязан НАЗЫВАТЬ переменную, оператор должен понять,
/// что чинить, без чтения исходников» (`red_window_guard_startup.rs`).
///
/// Проверка не привязана к ЧИСЛУ предела (его назначает founder, спека §5.1): требуется, чтобы
/// в сообщении стояли ДВЕ различные величины ≥ 1000 — предел и наблюдённое. Это ловит и немое
/// «too large», и сообщение, называющее только одну из двух величин.
#[test]
fn pl_i_5_f_refusal_names_the_limit_and_the_observed_value() {
    let _g = serial(); // C-198 B-5: окно heatmap процессно-глобально
    let dir = deep_book();
    let err = match snapshot(dir.path(), vec![ABUSIVE_BAND]) {
        Err(e) => e,
        Ok(_) => panic!(
            "PL-I-5 F SETUP НЕ СОСТОЯЛСЯ: отказа не произошло вовсе — судить текст нечего \
             (предмет закрывает оракул A)"
        ),
    };
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::InvalidInput,
        "PL-I-5 F: отказ по пределу — это невалидный ВХОД клиента, а не сбой ввода-вывода; \
         вызывающий различает их по `kind()`. Получено: {:?}",
        err.kind()
    );
    let msg = err.to_string();
    let nums = big_numbers(&msg);
    assert!(
        nums.len() >= 2,
        "PL-I-5 F: сообщение об отказе несёт {} величин(ы) ≥ 1000, нужно минимум две — ПРЕДЕЛ и \
         НАБЛЮДЁННОЕ. Без обеих клиент не знает, насколько сузить запрос, и подбирает вслепую. \
         Получено: {msg:?}",
        nums.len()
    );
}
