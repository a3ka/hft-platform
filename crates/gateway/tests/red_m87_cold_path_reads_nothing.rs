//! RED M-87 (sacred, architect-only) — ЖИВОЙ путь не запускает холодный пересчёт.
//!
//! Спека — `milestones/M-87-serving-circuit-breaker.md`. Предмет: один публичный запрос не
//! имеет права положить сервис. Сегодня имеет: при промахе слепка живой путь уходит в полный
//! реплей журнала (на проде — 74-75 ГБ), и отключение клиента работу не прекращает
//! (наблюдалось 2026-09-20).
//!
//! ЧТО ЭТОТ ФАЙЛ ДОКАЗЫВАЕТ И ЧТО НЕТ — граница названа, чтобы на него не опирались лишнего.
//! Он судит ОДИН из двух обязательных признаков: «ноль ДЕКОДИРОВАННЫХ событий». Этого
//! НЕДОСТАТОЧНО (план `docs/plans/scale-program-2026-09-21.md` §15.1): процесс может открыть
//! сегмент, прочитать и распаковать его, не вызвав декодер, — счётчик нулевой, дорогая работа
//! произошла. Второй признак — «ноль обращений к читателю полезной нагрузки», меряемый
//! ПРОЧИТАННЫМИ БАЙТАМИ, — живёт в `crates/gateway-serve/tests/red_m87_admission.rs`, потому
//! что требует поля, которого в `ReadStats` сегодня нет вовсе (замер: `events_decoded`,
//! `segments_opened`, `events_scanned`, `segment_meta_ops`, `depth_levels_visited` — байт
//! среди них НЕТ).
//!
//! ЧЕТЫРЕ СОСТОЯНИЯ СЛЕПКА (план §15.1) проверяются по отдельности, а не только «отсутствует»:
//! отсутствует · повреждён · несовместим по версии · слишком отстал. Каждое обязано дать
//! НАЗВАННЫЙ исход и НЕ запустить полный пересчёт на живом пути.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const T0: i64 = 1_752_000_000_000;
/// Событий в фикстуре. Достаточно, чтобы «полный реплей» отличался от «хвоста» на ПОРЯДОК,
/// и мало, чтобы тест был быстрым.
const N: u64 = 60;
/// Бюджет докормки хвоста для ОТСТАВШЕГО слепка. Величина — предмет спеки §5; здесь важна
/// не она сама, а то, что предел СУЩЕСТВУЕТ: сегодня докормка неограниченна.
const TAIL_BUDGET: u64 = 16;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 16,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

fn l2(ts: i64, bid: f64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: vec![lvl(bid, 3.0)],
            asks: vec![lvl(bid + 20.0, 4.0)],
            ts_exch_ms: ts,
        },
    )
}

fn trade(ts: i64, price: f64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(0.5),
            side: Side::Buy,
            ts_exch_ms: ts,
        },
    )
}

/// Журнал из `n` событий, чередуя книгу и сделки.
fn journal_of(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..n {
            let ts = T0 + (i as i64) * 100;
            let e = if i % 2 == 0 {
                l2(ts, 65_000.0 + i as f64)
            } else {
                trade(ts, 65_000.0 + i as f64)
            };
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn append_more(dir: &std::path::Path, from: u64, n: u64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in from..(from + n) {
        let ts = T0 + (i as i64) * 100;
        let e = if i % 2 == 0 {
            l2(ts, 65_000.0 + i as f64)
        } else {
            trade(ts, 65_000.0 + i as f64)
        };
        j.append(e).expect("append");
    }
    j.flush().expect("flush");
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
        depth_cadence_ms: None,
    }
}

/// Путь файла слепка для селектора: `ckpt-<fp_hex16>.bin`
/// (`crates/gateway/src/lib.rs:3613-3628`). Собирается здесь, потому что `ckpt_path_for`
/// — `pub(super)`; `selector_fingerprint` публичен.
fn ckpt_file(ckpt_dir: &std::path::Path, s: &Selector) -> std::path::PathBuf {
    let fp = gateway::checkpoint::selector_fingerprint(s);
    ckpt_dir.join(format!("ckpt-{fp:016x}.bin"))
}

fn live_resume(
    dir: &std::path::Path,
    ckpt: &std::path::Path,
    s: &Selector,
) -> (gateway::LiveReducer, gateway::ReadStats) {
    gateway::LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, s, ckpt).expect("resume")
}

// ─────────────── SETUP-СТРАЖИ: фикстура обязана РАЗЛИЧАТЬ тёплое и холодное ───────────────

/// Без этого стража весь файл — плацебо: если счётчик не растёт ни при каком входе,
/// «ноль декодированных» выполняется тривиально и ничего не доказывает.
/// (`testing.md` §«Целостность гейта», свойство 3: падать против несостоявшегося setup'а.)
#[test]
fn guard_counter_grows_on_this_fixture() {
    let dir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let (_live, stats) = live_resume(dir.path(), ckpt.path(), &sel());
    assert!(
        stats.events_decoded >= N,
        "setup-страж: на фикстуре из {N} событий холодный resume обязан декодировать ≥{N}, \
         а декодировал {}. Счётчик не растёт ⇒ остальные проверки этого файла бессмысленны",
        stats.events_decoded
    );
}

/// Тёплый путь ДОЛЖЕН быть дешевле холодного — иначе «слепок» ничего не экономит и
/// предохранитель защищает пустоту.
#[test]
fn guard_warm_resume_is_cheaper_than_cold() {
    let dir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    let (_live, warm) = live_resume(dir.path(), ckpt.path(), &sel());

    let cold_dir = journal_of(N);
    let cold_ckpt = tempfile::tempdir().expect("ckpt2");
    let (_l2, cold) = live_resume(cold_dir.path(), cold_ckpt.path(), &sel());

    assert!(
        warm.events_decoded < cold.events_decoded,
        "setup-страж: тёплый resume ({}) обязан декодировать МЕНЬШЕ холодного ({})",
        warm.events_decoded,
        cold.events_decoded
    );
}

// ─────────────── C2: ЧЕТЫРЕ состояния слепка, каждое по отдельности ───────────────

/// Состояние 1 — слепок ОТСУТСТВУЕТ.
#[test]
fn c2_missing_checkpoint_live_path_decodes_nothing() {
    let dir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let (_live, stats) = live_resume(dir.path(), ckpt.path(), &sel());
    assert_eq!(
        stats.events_decoded, 0,
        "живой путь без слепка декодировал {} событий — это полный пересчёт по запросу \
         пользователя (PL-I-4). Пересчёт обязан жить ТОЛЬКО в worker-пути",
        stats.events_decoded
    );
}

/// Состояние 2 — слепок ПОВРЕЖДЁН (файл на месте, содержимое мусор).
#[test]
fn c2_corrupt_checkpoint_live_path_decodes_nothing() {
    let dir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let path = ckpt_file(ckpt.path(), &sel());
    std::fs::write(&path, vec![0xABu8; 4096]).expect("write corrupt");
    assert!(
        path.exists(),
        "setup-страж: файл повреждённого слепка не создан"
    );

    let (_live, stats) = live_resume(dir.path(), ckpt.path(), &sel());
    assert_eq!(
        stats.events_decoded, 0,
        "повреждённый слепок увёл живой путь в пересчёт ({} событий). Порча слепка — \
         сигнал worker-пути, а не команда пересчитать при клиенте",
        stats.events_decoded
    );
}

/// Состояние 3 — слепок НЕСОВМЕСТИМ по версии провода. Файл валиден во всём, кроме
/// объявленной версии: `read_and_validate` (`crates/gateway/src/lib.rs:4251-4252`)
/// отвергает при `gw_v != GATEWAY_SCHEMA_VERSION`. Версия лежит в байтах `12..16`.
#[test]
fn c2_incompatible_version_live_path_decodes_nothing() {
    let dir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");

    let path = ckpt_file(ckpt.path(), &sel());
    let mut bytes = std::fs::read(&path).expect("read ckpt");
    assert!(
        bytes.len() > 16,
        "setup-страж: файл слепка короче заголовка ({} Б)",
        bytes.len()
    );
    let declared = u32::from_le_bytes(bytes[12..16].try_into().expect("4 байта"));
    assert_eq!(
        declared,
        gateway::GATEWAY_SCHEMA_VERSION,
        "setup-страж: байты 12..16 не несут версию провода — подмена испортила бы не то поле"
    );
    bytes[12..16].copy_from_slice(&(declared + 1).to_le_bytes());
    std::fs::write(&path, &bytes).expect("write incompatible");

    let (_live, stats) = live_resume(dir.path(), ckpt.path(), &sel());
    assert_eq!(
        stats.events_decoded, 0,
        "несовместимый по версии слепок увёл живой путь в пересчёт ({} событий). Версия \
         ПРОВОДА не должна обесценивать ВЫЧИСЛИТЕЛЬНОЕ состояние ценой аварии выдачи",
        stats.events_decoded
    );
}

/// Состояние 4 — слепок валиден, но СЛИШКОМ ОТСТАЛ: журнал ушёл далеко вперёд.
/// Докормка хвоста законна, но обязана быть ОГРАНИЧЕННОЙ: иначе «валидный слепок» —
/// обходной путь к тому же неограниченному пересчёту.
#[test]
fn c2_stale_checkpoint_tail_feed_is_budgeted() {
    let dir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");

    // Журнал уезжает далеко вперёд относительно слепка.
    append_more(dir.path(), N, N * 4);

    // ВАЖНО, и это поправка к первой редакции теста: докормка живёт в `pump`, а не в
    // `resume`. Первая редакция мерила ТОЛЬКО `resume`, проходила ЗЕЛЁНОЙ и не доказывала
    // ничего — ровно класс «оракул зелен по неверной причине» (`testing.md`: проба, молча
    // тестирующая не тот сценарий, есть плацебо самой себя). Ошибка поймана прогоном, а не
    // рассуждением, и оставлена видимой.
    let (mut live, resume_stats) = live_resume(dir.path(), ckpt.path(), &sel());
    let (_frames, _cursor, pump_stats) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 64)
        .expect("pump");
    let total = resume_stats.events_decoded + pump_stats.events_decoded;

    assert!(
        total <= TAIL_BUDGET,
        "отставший слепок дал докормку в {total} событий при бюджете {TAIL_BUDGET} \
         (resume={}, pump={}). Живой путь обязан ответить названным исходом, а не \
         догонять журнал произвольной длины",
        resume_stats.events_decoded,
        pump_stats.events_decoded
    );
}

// ─────────────── C1: и наоборот — ГОТОВОЕ состояние обслуживается ───────────────

/// Анти-плацебо в обратную сторону: реализация «всегда отказывать» тривиально проходит
/// все проверки выше. Здесь она обязана упасть.
#[test]
fn c1_ready_state_is_actually_served() {
    let dir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");

    let (live, _stats) = live_resume(dir.path(), ckpt.path(), &sel());
    let snap = live.snapshot();
    assert_ne!(
        snap.cursor,
        Cursor::START,
        "готовое состояние обязано обслуживаться: курсор остался в начале — реализация \
         'всегда отказывать' не является решением"
    );
    assert!(
        !snap.series.ohlcv.is_empty(),
        "готовое состояние обязано нести серии, а не пустой ответ"
    );
}
