//! SACRED (architect-only) — `M-70` `DB-I-7`: **канонический состав полос ДОЕЗЖАЕТ от
//! конфига до КАДРА НА ПРОВОДЕ**, а не остаётся строкой в `docker-compose.yml`.
//!
//! Заявление проверяется на ДВУХ уровнях — парсер и прод-точка входа; почему одного мало,
//! сказано ниже (`C-208` B-3).
//!
//! # Против какого мира стоит оракул
//!
//! Задача 7 меняет `GATEWAY_BANDS` на канонический набор (`П-014` п.4). Мир, которого надо
//! бояться, — «включили в конфиге, а выдача прежняя»: запись в compose есть, юниты зелены,
//! а сервис читает не то, парсит не так или молча подставляет дефолт. Это класс
//! **built-not-wired** (`gates.md` §4 «Механизм на пути»), и он у нас уже случался: `TD-155`
//! — барьер построен, джоб не подключён; `M-53` — `LiveReducer` написан, на путь не выведен.
//!
//! Проверка COMPOSE-ЗАПИСИ живёт в шаге гейта (`task #7`) и смотрит на файл. Здесь
//! проверяется ВТОРАЯ половина, которой файл не покрывает: значение из окружения обязано
//! стать `Selector.bands` того процесса, который строит ответ.
//!
//! # Различающая сила (`Р-4`)
//!
//! Признак — «`bands` конфига РАВНЫ каноническому набору». Мир, где переменная не читается
//! вовсе, этого признака не несёт: там `bands` останутся прод-дефолтом `[0.001]`, потому что
//! `serve_config_from_env` подставляет его при отсутствии ключа (`lib.rs:2082`). Мир, где
//! парсинг ломается на разделителе, тоже не несёт — состав выйдет иным по длине или
//! значениям. Признак недоступен обоим ¬P-мирам по построению, а не по совпадению.
//!
//! Отдельный сценарий держит границу с другой стороны: ОТСУТСТВИЕ переменной обязано давать
//! прод-дефолт, а не отказ. Без него требование удовлетворяется реализацией «всегда Err» —
//! ценой неработающего сервиса (тот же парный vantage, что в `red_heatmap_window_env.rs`).
//!
//! # ДВА УРОВНЯ, и первый сам по себе НЕ доказывает доставку (`C-208` B-3)
//!
//! Круг 2 предъявил: сценарии, зовущие только `serve_config_from_env`, покрывают ПАРСЕР, а
//! не путь выдачи. Реализация, где парсер верен, а исполняемый файл после разбора теряет или
//! подменяет селектор, проходила бы их все — то есть ровно тот built-not-wired мир, который
//! оракул заявлял закрытым. Находка верна, и заявление сужено не словами, а работой:
//!
//! - **уровень парсера** (`db_i_7`, `db_i_7b`, `db_i_7c`): окружение → `ServeConfig.selector`.
//!   Дёшев, ловит опечатку в ключе, поломку разделителя, молчаливый дефолт;
//! - **уровень ДОСТАВКИ** (`db_i_7d`): прод-точка входа `bind` → `serve` → WS-подключение →
//!   первый `Snapshot` НА ПРОВОДЕ. Это граница процесса тем вызовом, каким её дёргает прод
//!   (`testing.md` §«Механизм несущего пути обязан иметь оракул точки входа»), и никакой
//!   парсер её не подменяет.
//!
//! # Состояние
//!
//! Все четыре сценария ЗЕЛЕНЫ уже сегодня: механизм доставки состава существует, задача 7
//! лишь меняет ЗНАЧЕНИЕ в конфиге. Это СТОРОЖ, а не предмет — он краснеет ровно тогда, когда
//! задачу 7 сделают «в конфиге, но не на пути». Зелёное состояние названо прямо, чтобы
//! следующий круг не искал в нём RED, которого не предполагалось (`gates.md` §2: шаг,
//! зелёный раньше своей задачи, — дефект; шаг-сторож — не тот случай, и разница объявлена).

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;

/// Канонический набор — РЕШЕНИЕ FOUNDER'А (`research/data-quality/depth-verdict.md:15`:
/// «полный TPP-набор 1.5/3/5/8/15/30/60»); то же в `docs/fa/viz-backend.md:42` и в шаге
/// `task #7` гейта `M-70`. Значения дублируются здесь НАМЕРЕННО, с другой стороны границы.
const BANDS_CANONICAL: &[f64] = &[0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

/// Прод-дефолт `serve_config_from_env` при отсутствии переменной (`lib.rs:2010`).
const BANDS_DEFAULT: &[f64] = &[0.001];

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

const BASE: &[(&str, &str)] = &[
    ("GATEWAY_JWT_SECRET", "test-secret"),
    ("GATEWAY_TIMEFRAME_MS", "1000"),
    ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
];

fn base_plus(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = BASE.to_vec();
    v.extend_from_slice(extra);
    v
}

/// Канонический набор в том виде, в каком его несёт `docker-compose.yml`.
const CANONICAL_ENV: &str = "0.015,0.03,0.05,0.08,0.15,0.3,0.6";

/// `DB-I-7` ЯДРО — состав из окружения становится составом СЕЛЕКТОРА, по которому строится
/// ответ. Сравнение поэлементное: длина совпала, но значения разъехались — это тоже провал.
#[test]
fn db_i_7_canonical_bands_from_env_reach_the_selector() {
    let cfg = serve_config_from_env(getter(&base_plus(&[("GATEWAY_BANDS", CANONICAL_ENV)])))
        .expect("канонический состав обязан разбираться: он и есть предмет `П-014` п.4");

    assert_eq!(
        cfg.selector.bands.len(),
        BANDS_CANONICAL.len(),
        "DB-I-7 НАРУШЕН: из {CANONICAL_ENV:?} получилось {} полос вместо {}. Состав, не \
         доехавший до селектора, означает «включили в конфиге, а выдача прежняя» — класс \
         built-not-wired (`gates.md` §4)",
        cfg.selector.bands.len(),
        BANDS_CANONICAL.len()
    );

    for (got, want) in cfg.selector.bands.iter().zip(BANDS_CANONICAL) {
        assert!(
            (got - want).abs() < 1e-12,
            "DB-I-7 НАРУШЕН: полоса {got} вместо {want}. Длина совпала, значения разъехались — \
             это тихая подмена состава, подписанного founder'ом; клиент получит глубину не по \
             тем уровням, о которых принято решение. Полный разбор: {:?}",
            cfg.selector.bands
        );
    }
}

/// ПАРНЫЙ VANTAGE — отсутствие переменной даёт ПРОД-ДЕФОЛТ, а не отказ.
///
/// Без этого сценария требование выше удовлетворяется реализацией «всегда `Err`» — ценой
/// неработающего сервиса. Он же пиннит, что смена состава — операторский шаг: сегодня прод
/// живёт на `[0.001]`, и это законное состояние до исполнения задачи 7.
#[test]
fn db_i_7b_absent_bands_fall_back_to_prod_default_not_to_refusal() {
    let cfg = serve_config_from_env(getter(&base_plus(&[])))
        .expect("отсутствие GATEWAY_BANDS обязано быть законным — прод живёт так сегодня");

    assert_eq!(
        cfg.selector.bands, BANDS_DEFAULT,
        "отсутствие переменной обязано давать прод-дефолт {BANDS_DEFAULT:?}, а не {:?}. \
         Молча подставленный ИНОЙ состав дал бы клиенту данные, которых он не просил — тот же \
         класс, что `PL-I-5` закрывает для пределов",
        cfg.selector.bands
    );
}

/// АНТИ-ПЛАЦЕБО СЕТАПА — оракул отличал бы канонический состав от дефолта, даже если бы
/// разбор был сломан «в пользу» теста.
///
/// Сценарий существует потому, что первые два по отдельности слепы к вырожденному миру, где
/// `bands` вообще игнорируются и всегда равны какой-то константе: тогда либо ядро, либо
/// vantage упал бы — но проверить, что они судят РАЗНЫЕ исходы, надо явно.
#[test]
fn db_i_7c_canonical_and_default_are_actually_distinguishable() {
    let canon = serve_config_from_env(getter(&base_plus(&[("GATEWAY_BANDS", CANONICAL_ENV)])))
        .expect("канонический состав")
        .selector
        .bands;
    let dflt = serve_config_from_env(getter(&base_plus(&[])))
        .expect("дефолт")
        .selector
        .bands;

    assert_ne!(
        canon, dflt,
        "SETUP НЕ СОСТОЯЛСЯ: канонический состав и прод-дефолт дали ОДИН результат {canon:?}. \
         Значит `GATEWAY_BANDS` не влияет ни на что, и оба сценария выше зелены ни о чём"
    );
}

// ─────────────────────────── УРОВЕНЬ ДОСТАВКИ ───────────────────────────

use futures_util::StreamExt;
use gateway_serve::server::bind;
use gateway_serve::wire::ServeMsg;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};

const SECRET: &[u8] = b"m70-db-i-7-delivery";
const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

#[derive(serde::Serialize, serde::Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign(secret: &[u8]) -> String {
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("время")
        .as_secs() as usize
        + 3_600;
    encode(
        &Header::default(),
        &Claims {
            sub: "db-i-7".to_string(),
            exp,
        },
        &EncodingKey::from_secret(secret),
    )
    .expect("подпись JWT")
}

/// Книга до ±60 % — чтобы КАЖДАЯ каноническая полоса имела уровни и строка серии была не
/// пустой: «доехало» на пустой выдаче ничего не доказывает.
fn journal_deep_book() -> tempfile::TempDir {
    use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
    const STEP_FRAC: f64 = 0.002;
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(
        dir.path(),
        WriterConfig {
            max_segment_bytes: 1 << 26,
            min_free_bytes: 0,
            source: DataSource::OwnCapture,
            provenance: "M-70 DB-I-7 delivery fixture".to_string(),
            epoch_id: "own-test".to_string(),
        },
    )
    .expect("open_with");
    let step = MID * STEP_FRAC;
    let n = (0.60 / STEP_FRAC) as usize;
    let lv = |v: Vec<(f64, f64)>| -> Vec<Level> {
        v.into_iter()
            .map(|(p, s)| Level {
                price: to_fixed(p),
                size: to_fixed(s),
            })
            .collect()
    };
    for t in 0..3i64 {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: lv((1..=n).map(|k| (MID - k as f64 * step, 1.0)).collect()),
                asks: lv((1..=n).map(|k| (MID + k as f64 * step, 1.0)).collect()),
                ts_exch_ms: T0 + t * 1_000,
            },
        ))
        .expect("append");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID),
                size: to_fixed(1.0),
                side: Side::Buy,
                ts_exch_ms: T0 + t * 1_000,
            },
        ))
        .expect("append");
    }
    j.flush().expect("flush");
    dir
}

/// `DB-I-7d` — УРОВЕНЬ ДОСТАВКИ: состав из окружения доезжает до КАДРА НА ПРОВОДЕ.
///
/// Закрывает `C-208` B-3. Путь исполняется тот же, каким его дёргает прод:
/// `serve_config_from_env` → `bind` → `serve` → WS-подключение с валидным JWT → первый
/// `Snapshot`. Между конфигом и кадром нет ни одной подмены, которую сценарий бы не заметил:
/// он смотрит на СЕЛЕКТОР ВНУТРИ ОТВЕТА и на число строк депт-серии, а не на `ServeConfig`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn db_i_7d_canonical_bands_reach_the_frame_on_the_wire() {
    let dir = journal_deep_book();
    let cfg = serve_config_from_env(getter(&base_plus(&[
        ("GATEWAY_BANDS", CANONICAL_ENV),
        ("GATEWAY_SYMBOL", "BTCUSDT"),
    ])))
    .expect("конфиг с каноническим составом");

    // Прод-точка входа. Журнал и ключ подменяются на тестовые — это ЕДИНСТВЕННОЕ отличие от
    // прода, и оно названо: селектор, ради которого сценарий существует, приходит из env.
    let server = bind(gateway_serve::server::ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: cfg.selector.clone(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: None,
    })
    .await
    .expect("bind прод-точки входа");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let url = format!("ws://{addr}/?token={}", sign(SECRET));
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url)
        .await
        .expect("WS-подключение валидным JWT");
    let msg = ws
        .next()
        .await
        .expect("сервер обязан прислать первое сообщение")
        .expect("сообщение читается");
    let parsed: ServeMsg =
        serde_json::from_slice(msg.into_data().as_ref()).expect("разбор ServeMsg");
    let snap = match parsed {
        ServeMsg::Snapshot(s) => s,
        other => panic!("первым сообщением обязан быть Snapshot, получено: {other:?}"),
    };

    assert_eq!(
        snap.selector.bands.len(),
        BANDS_CANONICAL.len(),
        "DB-I-7d НАРУШЕН: кадр НА ПРОВОДЕ несёт {} полос вместо {}. Парсер мог отработать \
         верно, а исполняемый путь — потерять или подменить селектор после разбора: ровно \
         built-not-wired, который парсерные сценарии не видят (`C-208` B-3)",
        snap.selector.bands.len(),
        BANDS_CANONICAL.len()
    );
    assert_eq!(
        snap.series.depth_series.len(),
        BANDS_CANONICAL.len() * 2,
        "DB-I-7d НАРУШЕН: в доставленном кадре {} строк депт-серии, ожидалось {} \
         (полоса × сторона). Селектор доехал, но выдача по нему не построена — «включили в \
         конфиге, а выдача прежняя» в чистом виде",
        snap.series.depth_series.len(),
        BANDS_CANONICAL.len() * 2
    );
    assert!(
        snap.series
            .depth_series
            .iter()
            .all(|r| !r.series.is_empty()),
        "DB-I-7d НАРУШЕН: часть строк депт-серии ПУСТА. «Доехало» на пустой выдаче ничего не \
         доказывает: книга фикстуры тянется до ±60 %, каждая каноническая полоса обязана \
         иметь уровни"
    );
}
