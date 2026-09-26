//! RED M-87 круг `R-201` (sacred, architect-only) — **сколько журнала РЕАЛЬНО прочитано:
//! замер ядром, а не нашим счётчиком и не текстом исходника.**
//!
//! ## Почему этот файл заменяет два моих плацебо
//!
//! Вердикт `R-201` предъявил мутациями, что `c2` и `c3` из `red_m87_r196_conditions.rs`
//! проверяли ТЕКСТ, а не работу:
//!
//! ```text
//! мутация 1: payload_bytes_read: 1          (величина заменена литералом) → c2 ЗЕЛЁН
//! мутация 2: if true { None } else { … }    (вызов не исполняется)       → c3 ЗЕЛЁН
//! ```
//!
//! Признать надо прямо: коммит, который их вводил, назывался «проверяются ПО РАБОТЕ, не по
//! имени» — и сам этому заголовку не соответствовал. `c2` искал подстроку `snap_text.len()`,
//! то есть ловил ОДНУ конкретную неверную величину и пропускал любую другую; `c3` считал
//! строки с текстом `feed_tail_within(`, то есть присутствие имени. **Это третий рецидив
//! одного класса в моём исполнении за милестоун**, и лечится он не более точным грепом.
//!
//! ## Развязка: мерить ЯДРОМ, а не собой
//!
//! Проверять счётчик прочитанных байт нашим же счётчиком нельзя — он и есть предмет
//! подозрения (`R-201` Б-2: он кормился размером КАТАЛОГА). Нужен НЕЗАВИСИМЫЙ путь, и он
//! есть: `/proc/self/io` `rchar` — байты, прошедшие через `read()`, счёт ведёт ядро.
//! Повлиять на эту величину из нашего кода невозможно, подделать её нельзя, а `testing.md`
//! требует эталон именно из независимого пути: «сломай X так, чтобы Y сломался ТЕМ ЖЕ
//! образом, — покраснеет ли оракул?». Для `rchar` ответ: не покраснеет, потому что он от
//! нашей реализации не зависит вовсе.
//!
//! ## Что пиннится
//!
//! | # | свойство | что ловит |
//! |---|---|---|
//! | `q0` | SETUP-СТРАЖ: каталог ВЕЛИК, хвост МАЛ, отношение известно | без него границы не различают предметы и всё зеленеет на вырожденной фикстуре |
//! | `q1` | фактическое чтение с диска ОГРАНИЧЕНО и не равно «весь каталог» | холостое чтение на пути инициализации сессии (`R-201` Б-1: ≈88 МБ на итерацию при 11 сегментах по 1 ГБ) |
//! | `q2` | счётчик не может «учесть» БОЛЬШЕ, чем ядро реально прочитало, и не меньше хвоста | и литерал `1`, и «размер каталога» — обе мутации `R-201` |
//!
//! ## Почему ОТДЕЛЬНЫЙ бинарь
//!
//! И `rchar`, и `JOURNAL_BYTES_GLOBAL` — ПРОЦЕССНЫЕ величины. В общем бинаре соседние тесты
//! читают файлы и двигают оба счётчика, и замер стал бы флаком (`testing.md` §«Целостность
//! гейта», свойство 2: конфаундинг-величину держать КОНСТАНТНОЙ). Здесь один сценарий на
//! процесс.
//!
//! RUNTIME-RED по построению: на ревизии вердикта `q1` и `q2` красны.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::StreamExt;
use gateway::Selector;
use gateway_serve::server::{bind, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};

const SECRET: &[u8] = b"m87-read-volume-truth";
const FUTURE: usize = 4_000_000_000;
/// Журнал заведомо МНОГОСЕГМЕНТНЫЙ: предмет замера — отношение «хвост / весь каталог»,
/// и на одном сегменте оно неразличимо.
const BODY: u64 = 4_000;
const TAIL: u64 = 10;
const SEG_BYTES: u64 = 8 * 1024;

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m87-vol".to_string(),
            exp: FUTURE,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt")
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(100.0 + (i % 7) as f64),
            size: to_fixed(1.0 + (i % 3) as f64),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_752_000_000_000 + i as i64 * 100,
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

/// Байты, прошедшие через `read()` В ЭТОМ ПРОЦЕССЕ. Счёт ведёт ЯДРО — величина независима
/// от нашей реализации, и это единственная причина, по которой она годна как эталон.
fn rchar() -> u64 {
    let s = std::fs::read_to_string("/proc/self/io").expect("/proc/self/io недоступен");
    for line in s.lines() {
        if let Some(v) = line.strip_prefix("rchar:") {
            return v.trim().parse().expect("rchar — число");
        }
    }
    panic!("в /proc/self/io нет строки rchar");
}

fn dir_bytes(dir: &std::path::Path) -> u64 {
    let mut total = 0;
    for e in std::fs::read_dir(dir).expect("read_dir") {
        let p = e.expect("entry").path();
        if p.is_file() {
            total += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

struct Fixture {
    dir: tempfile::TempDir,
    ckpt: tempfile::TempDir,
    dir_bytes_before_tail: u64,
    tail_bytes: u64,
    segments: usize,
}

/// Тёплый слепок снимается на ПОЛНОМ теле журнала, хвост дописывается ПОСЛЕ — иначе
/// «прочитанное при обслуживании» не отличалось бы от «прочитанного при построении слепка»,
/// и замер судил бы не тот механизм.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..BODY {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance (warm checkpoint)");

    let before = dir_bytes(dir.path());
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("reopen");
        for i in BODY..BODY + TAIL {
            j.append(trade(i)).expect("append tail");
        }
        j.flush().expect("flush tail");
    }
    let tail_bytes = dir_bytes(dir.path()).saturating_sub(before);
    let segments = journal::list_segments(dir.path()).expect("segments").len();
    Fixture {
        dir,
        ckpt,
        dir_bytes_before_tail: before,
        tail_bytes,
        segments,
    }
}

/// Один запрос по прод-форме: подключение + первое сообщение. Возвращает
/// (прирост `rchar`, прирост счётчика выдачи).
async fn serve_once(f: &Fixture) -> (u64, u64) {
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: f.dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(f.ckpt.path().to_path_buf()),
    };
    let server = bind(cfg).await.expect("bind");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let rchar_before = rchar();
    let counter_before = gateway_serve::metrics::serving_counters().journal_payload_bytes_read;

    let token = sign();
    let (mut ws, _) = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio_tungstenite::connect_async(format!("ws://{addr}/?token={token}")),
    )
    .await
    .expect("подключение не состоялось за 30 с")
    .expect("connect");
    let _msg = tokio::time::timeout(std::time::Duration::from_secs(30), ws.next())
        .await
        .expect("СЕРВЕР МОЛЧИТ: первое сообщение не пришло за 30 с")
        .expect("поток закрыт")
        .expect("ws error");

    let rchar_after = rchar();
    let counter_after = gateway_serve::metrics::serving_counters().journal_payload_bytes_read;
    (
        rchar_after.saturating_sub(rchar_before),
        counter_after.saturating_sub(counter_before),
    )
}

/// **q0+q1+q2 — один сценарий, и разделять его нельзя.**
///
/// `rchar` и счётчик выдачи — ПРОЦЕССНЫЕ: второй тест в этом же бинаре наблюдал бы
/// последствия первого. Поэтому страж, замер объёма и замер счётчика — три части одного тела.
#[tokio::test]
async fn q_read_volume_and_counter_tell_the_truth() {
    let f = fixture();

    // ── q0: SETUP-СТРАЖ ─────────────────────────────────────────────────────────────
    // Без него фикстура из одного сегмента с огромным хвостом дала бы «прочитано ≈ каталог»
    // на ИСПРАВНОЙ реализации, и оракул краснел бы по неверной причине.
    assert!(
        f.segments >= 8,
        "SETUP-СТРАЖ: сегментов {} — фикстура односегментная, отношение «хвост / каталог» \
         неразличимо, и замер судил бы не тот предмет",
        f.segments
    );
    assert!(
        f.tail_bytes > 0,
        "SETUP-СТРАЖ: хвост пуст ({} Б) — нечего читать, и нижняя граница счётчика \
         выполнялась бы тривиально",
        f.tail_bytes
    );
    assert!(
        f.dir_bytes_before_tail > f.tail_bytes * 20,
        "SETUP-СТРАЖ: каталог {} Б при хвосте {} Б — разница меньше двадцатикратной, \
         границы q1/q2 не различают «хвост» от «весь каталог»",
        f.dir_bytes_before_tail,
        f.tail_bytes
    );

    let (read_delta, counter_delta) = serve_once(&f).await;
    let dir_total = f.dir_bytes_before_tail + f.tail_bytes;

    // ── q1: ФАКТИЧЕСКОЕ чтение ограничено ───────────────────────────────────────────
    // Замер ведёт ЯДРО. Тёплый слепок покрывает тело журнала; обслуживание обязано прочесть
    // слепок и ХВОСТ, а не весь каталог. Порог — половина каталога: он не придирчив (слепок
    // сам весит немало), но холостой проход по всем сегментам через него не проходит.
    let limit = dir_total / 2;
    assert!(
        read_delta < limit,
        "ФАКТИЧЕСКИ прочитано {read_delta} Б при каталоге {dir_total} Б (порог {limit}). \
         Тёплый слепок покрывает тело журнала — обслуживание обязано читать слепок и ХВОСТ \
         ({} Б), а не обходить все {} сегментов. Замер независим от наших счётчиков: это \
         `rchar` из /proc/self/io, его ведёт ядро.\n\
         Класс: холостое чтение на пути инициализации КАЖДОЙ сессии (`R-201` Б-1), на проде — \
         ≈88 МБ за итерацию при 11 сегментах по 1 ГБ.",
        f.tail_bytes,
        f.segments
    );

    // ── q2: счётчик согласован с ФАКТОМ ─────────────────────────────────────────────
    // Верхняя граница физическая: учесть больше, чем ядро прочитало, невозможно. Нижняя —
    // содержательная: хвост прочитан, значит он учтён. Литерал `1` нарушает нижнюю,
    // «размер каталога» — верхнюю (когда каталог целиком не читался).
    assert!(
        counter_delta <= read_delta,
        "счётчик выдачи объявил {counter_delta} Б прочитанными, а ядро видело только \
         {read_delta} Б. Учесть больше, чем реально прочитано, невозможно — значит величина \
         не измеряется, а СОЧИНЯЕТСЯ (`R-201` Б-2: кормилась размером КАТАЛОГА)"
    );
    assert!(
        counter_delta >= f.tail_bytes,
        "счётчик выдачи объявил {counter_delta} Б, а один только хвост весит {} Б. \
         Величина, названная «прочитанные байты журнала», обязана включать прочитанный хвост; \
         меньшее значение означает, что она не считает, а подставляет (мутация `R-201`: \
         литерал `1` проходил прежнюю форму оракула)",
        f.tail_bytes
    );
}
