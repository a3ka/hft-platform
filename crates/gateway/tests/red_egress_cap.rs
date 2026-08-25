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
//! # Почему предел меряется РЕСУРСОМ, а не шириной полосы
//!
//! Ширина — ПРОКСИ. При уплотнении книги тот же `0.013` даст другой объём, и оракул на ширине
//! пропустит превышение (`testing.md`: «оракул границы ресурса меряет ресурс, а не прокси»).
//! Поэтому оракулы ниже судят ВЕЛИЧИНУ ПОСТРОЕННОГО ОТВЕТА.
//!
//! # Числа предела в оракулах НЕТ, и это намеренно
//!
//! Величина предела — продуктовое решение того же класса, что `DEFAULT_MAX_SUBSCRIPTIONS = 16`
//! («подписанная норма»), и назначается founder'ом (спека §5.1). Фикстуры разведены на три
//! порядка (сотня ячеек против десятков тысяч), поэтому набор судит ПОВЕДЕНИЕ при любом
//! разумном пределе и не протухнет от смены числа. Побочная выгода: оракулы не COMPILE-RED —
//! они не ссылаются на ещё не существующую константу, и потому предъявимы КРАСНЫМИ, а не
//! «не собралось» (урок `M-68` rev2, `C-138` п.3).

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
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

fn sel(bands: Vec<f64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
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

fn deep_book() -> tempfile::TempDir {
    journal_prod_shape((REACH / STEP) as usize, N_BUCKETS)
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
    // Узкая полоса на порядки дешевле широкой — это и делает набор независимым от ЧИСЛА предела.
    assert!(
        s.series.heatmap.len() < 1_000,
        "PL-I-5 E SETUP НЕ СОСТОЯЛСЯ: узкий ответ дал {} ячеек — фикстура не разводит узкий и \
         широкий случаи на порядки, и оракулы ниже начинают зависеть от точной величины предела",
        s.series.heatmap.len()
    );
}

/// **E-2 — вырожденный вход не отвергается.** Пустая и односторонняя книга — легитимные
/// состояния (старт процесса, ресинк, односторонний рынок). Реализация, отвергающая их
/// «на всякий случай», ломает работу там, где ресурса не тратится вовсе.
#[test]
fn pl_i_5_e2_degenerate_books_are_served() {
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

/// **B — отказ ЯВНЫЙ; принятый ответ ПОЛОН.**
///
/// Соблазнительный «фикс» — усечь ответ до предела и отдать. Он зелен во всех liveness-
/// проверках (`healthy`, heartbeat, рост журнала) и врёт клиенту молча: `PL-I-7` —
/// «деградация никогда не выдаётся за норму». Оракул закрывает обе стороны: превышение даёт
/// `Err`, а НЕ урезанный `Ok`; принятый запрос отдаёт ровно столько, сколько есть в книге.
#[test]
fn pl_i_5_b_no_silent_truncation_on_either_side() {
    let dir = deep_book();

    // (1) превышение обязано быть ОШИБКОЙ, а не урезанным успехом
    if let Ok(s) = snapshot(dir.path(), vec![ABUSIVE_BAND]) {
        panic!(
            "PL-I-5 B: превышение вернуло Ok с {} ячейками. Если это усечение — оно МОЛЧАЛИВОЕ: \
             клиент получил неполную книгу под видом полной (PL-I-7). Отказ обязан быть явным; \
             усечение, если оно вообще допускается, обязано быть ПОМЕЧЕНО в ответе.",
            s.series.heatmap.len()
        );
    }

    // (2) принятый запрос НЕ урезан: число ячеек в точности равно геометрии фикстуры.
    // Уровней внутри ±0.1 % при шаге 0.02 % со смещением на полшага: (k−0.5)*STEP <= PROD_BAND
    // ⇒ k = 1..=5, обе стороны ⇒ 10 на бакет; бакетов N_BUCKETS. Эталон вычислен из ГЕОМЕТРИИ
    // ФИКСТУРЫ, а не из той же функции, что строит ответ (`testing.md`, «зависимый эталон»).
    let per_side = (PROD_BAND / STEP + 0.5).floor() as usize;
    let expected = per_side * 2 * N_BUCKETS as usize;
    let s = snapshot(dir.path(), vec![PROD_BAND]).expect("узкий обязан обслуживаться");
    assert_eq!(
        s.series.heatmap.len(),
        expected,
        "PL-I-5 B: принятый запрос отдал {} ячеек при {expected} по геометрии фикстуры \
         ({per_side} уровней на сторону × 2 × {N_BUCKETS} бакетов). Ответ, который прошёл \
         предел, обязан быть ПОЛНЫМ — иначе предел молча режет честную нагрузку.",
        s.series.heatmap.len()
    );
}

/// **C (`PL-I-4`) — БАЙПАСА НЕТ: предел живёт в `gateway`, а не только в транспорте.**
///
/// `Selector` собирают напрямую четыре потребителя мимо `gateway-serve`: чекпоинтер (M-38b),
/// shared-tailer (M-39), `research-cli`, replay. Гвард, посаженный только в транспорт,
/// оставил бы им открытую дверь — ровно тот довод, которым `GW-I-14` посажен в
/// `gateway::validate_selector` (`crates/gateway/src/lib.rs:1893-1905`), а не в
/// `serve_config_from_env`.
///
/// Оракул бьёт по ДРУГИМ точкам входа, чем `A`: реализация, добавившая предел только в
/// `snapshot`, красна здесь.
#[test]
fn pl_i_4_c_limit_has_no_bypass_across_entry_points() {
    let dir = deep_book();
    let s = sel(vec![ABUSIVE_BAND]);

    let frames = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        usize::MAX,
    );
    assert!(
        frames.is_err(),
        "PL-I-4 C: `frames_since` обслужил раздувающий селектор. Предел, поставленный только \
         в `snapshot`, оставляет открытым push-путь — а именно им живёт WS-клиент после \
         первого снапшота."
    );

    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    let warm = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        ckpt.path(),
        Cursor::LATEST,
    );
    assert!(
        warm.is_err(),
        "PL-I-4 C: `snapshot_from_checkpoint` обслужил раздувающий селектор. Warm-путь — \
         обычный прод-путь (чекпоинт снимается по расписанию), и предел обязан действовать \
         на нём тождественно."
    );
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
