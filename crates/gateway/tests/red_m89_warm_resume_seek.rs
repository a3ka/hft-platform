//! RED `M-89` (sacred, architect-only) — **ПЕРВЫЙ `pump` после тёплого `resume` стоит ХВОСТ,
//! а не ПРЕФИКС активного сегмента.** Инварианты `I-1` (работа), `I-2` (корректность на
//! вырожденных входах), `I-4` (счётчик прочитанных байт — от читателя, не от описи).
//!
//! Милестоун `docs/archive/M-89-s0-volume-guard-observability.md` §4–§6. Замер, из которого
//! оракул вырос, — `docs/plans/m89-warm-resume-measure-2026-09-27.md` (архитектор-клон,
//! прод 2026-09-27 12:11Z): после `resume` из прогретого слепка `tail_hint = None`, и первый
//! `pump` открывает активный сегмент с `header_end` — `events_scanned = N + хвост` РОВНО
//! (2 500 / 200 500 / 400 500 при хвосте 500), `rchar ≈ размер файла` (20 620 325 Б при
//! 20 554 613 Б). На проде это 305 тыс. событий / ≈412 МБ перед курсором сразу после
//! прогрева и до 1 ГБ к ротации — на КАЖДУЮ новую подписку.
//!
//! # Почему существующие оракулы этого не ловят (названо, чтобы не искать там)
//!
//! `red_tick_read_cost.rs` (`F-036`) делает `resume` БЕЗ слепка, догоняет хвост
//! несколькими `pump` — первый из них и есть дорогой — и меряет ТОЛЬКО следующий тик.
//! `red_tail_cursor_prod_form.rs` / `red_hint_pos_guard.rs` судят hint, который на
//! первом тике ещё пуст. То есть корпус пиннит установившийся тик и слеп к первому.
//!
//! # Мера
//!
//! Две меры, обе на границе потребителя (`docs/workflow/oracle-blindness-class-2026-08-28.md`
//! §5, `testing.md` §«Оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ» п. 2):
//! · `rchar` из `/proc/self/io` — байты, прошедшие через `read()` ПРОЦЕССА; ядро ведёт счёт,
//!   подделать его нельзя; замер ВСЕГДА единственный в процессе (`serial()`);
//! · `ReadStats::events_scanned` — честный счётчик парсера (M-57), аддитивно.
//!
//! Абсолютные пороги названы с запасом и отделены от границы (`testing.md` п. 4 чек-листа):
//! поиск позиции по `seq` внутри сегмента стоит `O(log)` проб по ≤ 64 КиБ каждая, значит на
//! 20 МиБ ≈ 10 проб ≈ 640 КиБ; порог — 2 МиБ. Дефектная реализация читает 20 МиБ.
//!
//! # Эталон — НЕЗАВИСИМЫЙ путь
//!
//! `gateway::snapshot(dir, filter, sel, Cursor::at(last))` — полная свёртка через
//! `journal::stream` без слепка и без hint. `testing.md` §«Мутационный контроль»: эталон не
//! делит с предметом ни код позиционирования, ни слепок.
//!
//! # Чего оракул НЕ ловит — названо
//!
//! · Он не судит транспорт: точка входа WS — `crates/gateway-serve/tests/red_m89_*`.
//! · Он не меряет время: время мерит хост (`testing.md`, св. 2 целостности гейта).
//! · Случай `.zst` (`d4`) — НАЗВАННЫЙ ПРЕДЕЛ: zstd-декодер не `Seek`, сдвиг без распаковки
//!   невозможен; оракул требует лишь корректности и «не хуже одного сегмента».
//! · Размер события в фикстуре ≈ 51 Б против ≈ 1 351 Б на проде (замер §1): число проб
//!   растёт как `log2(размер)`, то есть на проде их на ~5 больше — порог это покрывает.
//!
//! RUNTIME-RED на ревизии набора: `w1`, `d1`, `d2`, `d3`, `d6`, `d8`, `d9`, `p1` красны
//! по СТОИМОСТИ (`events_scanned = префикс + хвост`), `p1` — ещё и по счётчику байт
//! (`payload_bytes_read = 0` у `pump`). `d4`, `d7`, `d7b`, `d7c` — стражи против будущей
//! реализации, зелены сегодня и названы стражами. ТОЧНЫЙ учёт байт `payload_bytes_read`
//! (равенство, а не интервал) — `crates/journal/tests/red_m89_bytes_accounting.rs`; `p1`
//! здесь держит только согласованность с ядром на прод-пути `pump`.

use std::io::Write;

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, LiveReducer, ReadStats, Selector, Snapshot};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Прод-форма: ОДИН сырой активный сегмент на 1 ГиБ (`journal.meta`/`ls` прода 2026-09-27:
/// `max_segment_bytes ≈ 1 073 741 xxx`, активный 585 МБ). Префикс лежит в ТОМ ЖЕ файле,
/// что и хвост, — именно это и делает первый `pump` дорогим.
const SEG_BYTES_PROD: u64 = 1 << 30;
/// Прод-масштаб префикса: замер 2026-09-27 — 304 883 события перед курсором сразу после
/// прогрева; мандат требует ≥ 200 тыс.
const PREFIX_PROD: u64 = 200_000;
/// Малая точка для отношения (та же, что в замере: 2 000).
const PREFIX_SMALL: u64 = 2_000;
/// Средняя точка для вырожденных входов: префикс на порядок больше хвоста, но дешёвый.
const PREFIX_MID: u64 = 5_000;
/// Хвост после слепка (замер: 500).
const TAIL: u64 = 500;
/// Прод-партия `PUSH_MAX_EVENTS` (`crates/gateway-serve/src/lib.rs`).
const PUMP_BATCH: usize = 256;
/// Допуск на поиск позиции: `≤ 2·⌈log2(file/64 КиБ)⌉ + 4` декодированных кадров — при
/// файле до 2^30 · 64 КиБ это ≤ 64. Дефектная реализация даёт `events_scanned = N + хвост`.
const SEEK_SCAN_ALLOWANCE: u64 = 64;
/// Допуск на байты поиска позиции + буфер заголовка + опись каталога: 2 МиБ. Дефектная
/// реализация читает файл целиком (10 МиБ при 200 тыс. событий по ~51 Б).
const SEEK_READ_ALLOWANCE: u64 = 2 * 1024 * 1024;
/// Предел РАЗНИЦЫ `rchar` первого pump'а «большой префикс − малый префикс» (`TD-250`).
///
/// Прежде здесь стояло ОТНОШЕНИЕ с допуском 2.0×, и оно флаковало: CI на одном дереве дал
/// `success` и `2.20×` (`R-248`, run 37857079885). Замер 2026-10-09, 30 + 15 + 15 прогонов: у
/// честной реализации `rchar` первого pump'а ДВУХМОДАЛЕН — ≈164 КБ или ≈349 КБ (лишние ≈185 КБ)
/// примерно в трети прогонов, на МАЛОМ и на БОЛЬШОМ префиксе одинаково. Лишнее читает тот же
/// поток (`/proc/thread-self/io` = `/proc/self/io` + 238 Б на чтение самих счётчиков), пауза
/// 1.1 с перед pump'ом частоту не меняет (5/15 против 6/15) — это не гонка времени изменения
/// файла и не чужой поток; мода закреплена за ФИКСТУРОЙ (в одном прогоне малая — 349 КБ,
/// большая — 164 КБ), то есть за раскладкой байт, в которую писатель журнала кладёт настенное
/// время. Добавка ПОСТОЯННА — с длиной префикса не растёт, инвариант `I-1` ею не нарушен; но
/// отношение двух чисел порядка 160 КБ она двигает до 2.13×, а знаменатель и числитель
/// попадают в разные моды независимо.
///
/// Мерится то, что обещано: работа первого тика НЕ ЗАВИСИТ от длины префикса. Разница при
/// ×100 длине (2 тыс. → 200 тыс. событий, ~10 МиБ) у честной реализации — `0 ± 185 КБ`;
/// дефект «читать префикс» даёт ≈10 МиБ (замер 2026-09-27: ×109). Предел 512 КиБ ловит любую
/// реализацию, читающую больше 5 % префикса, и не краснеет на постоянной добавке.
/// **Предел, названный честно:** частичный дефект, читающий меньше 512 КиБ префикса (< 5 %),
/// этим миром не ловится; абсолютный предел `SEEK_READ_ALLOWANCE` — тоже.
const MAX_PREFIX_DEPENDENT_BYTES: u64 = 512 * 1024;

fn cfg(seg_bytes: u64) -> WriterConfig {
    WriterConfig {
        max_segment_bytes: seg_bytes,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-89 warm-resume seek fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        // Прод-окно `GATEWAY_WINDOW_MS=60000` (`docker-compose.yml`).
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

/// Замер `rchar` обязан быть единственным в процессе в свой момент.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о цене первого pump'а: фикстура не \
         воспроизвела сценарий, ради которого оракул написан."
    )
}

/// Байты, реально прочитанные процессом вызовами `read` (Linux `/proc/self/io`).
fn rchar() -> u64 {
    let text = std::fs::read_to_string("/proc/self/io").unwrap_or_else(|e| {
        setup_failed(&format!(
            "/proc/self/io недоступен ({e}) — меры границы процесса нет, и подменять её \
             счётчиком участника запрещено"
        ))
    });
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("rchar:") {
            return v
                .trim()
                .parse::<u64>()
                .unwrap_or_else(|e| setup_failed(&format!("rchar не разобран из «{line}»: {e}")));
        }
    }
    setup_failed("в /proc/self/io нет строки rchar")
}

/// Сделка с уникальным размером: любой пропуск/дубль события меняет `Σ size` и VWAP, то есть
/// виден в эталонном сравнении, а не тонет в одинаковых значениях.
fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(MID + (i % 997) as f64 * 0.01),
            size: to_fixed(0.001 + (i % 101) as f64 * 0.0001),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: T0 + i as i64 * 7,
        },
    )
}

fn append_trades(j: &mut Journal, from: u64, count: u64) {
    for i in from..from + count {
        j.append(trade(i))
            .unwrap_or_else(|e| setup_failed(&format!("append #{i}: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
}

/// Байты всех файлов каталога журнала — для точного размера хвоста (разность до/после дописки).
fn dir_bytes(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| setup_failed(&format!("read_dir: {e}")))
        .filter_map(Result::ok)
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

fn segment_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| setup_failed(&format!("read_dir: {e}")))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            n.ends_with(".jrnl") || n.ends_with(".jrnl.zst")
        })
        .collect();
    v.sort();
    v
}

fn last_seq(dir: &std::path::Path) -> u64 {
    // `journal.meta` — u64 LE `next_seq`; после `flush` он актуален (тем же способом читает
    // `readiness`, `crates/gateway-serve/src/admission.rs`).
    let bytes = std::fs::read(dir.join("journal.meta"))
        .unwrap_or_else(|e| setup_failed(&format!("journal.meta: {e}")));
    u64::from_le_bytes(bytes[..8].try_into().expect("8 байт")) - 1
}

fn canon(s: &Snapshot) -> Vec<u8> {
    serde_json::to_vec(s).expect("snapshot → json")
}

/// Независимый эталон: полная свёртка `[START .. at]` через `journal::stream`, без слепка,
/// без hint, без `LiveReducer`.
fn reference(dir: &std::path::Path, at: u64) -> Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::at(at))
        .unwrap_or_else(|e| setup_failed(&format!("эталонная свёртка: {e}")))
}

struct Fixture {
    dir: tempfile::TempDir,
    ckpt: tempfile::TempDir,
    /// Байты, дописанные ПОСЛЕ слепка (точный размер хвоста на диске).
    tail_bytes: u64,
    last_seq: u64,
    tail_events: u64,
}

/// Тёплый слепок снимается на префиксе, хвост дописывается ПОСЛЕ, в ТОТ ЖЕ активный сегмент
/// (`open_with` с тем же `WriterConfig` переиспользует активный сегмент — `decide_open_segment`).
fn fixture(prefix: u64, tail: u64, seg_bytes: u64) -> Fixture {
    let dir = tempfile::tempdir().unwrap_or_else(|e| setup_failed(&format!("tempdir: {e}")));
    {
        let mut j = Journal::open_with(dir.path(), cfg(seg_bytes))
            .unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
        append_trades(&mut j, 0, prefix);
    }
    let ckpt = tempfile::tempdir().unwrap_or_else(|e| setup_failed(&format!("ckpt: {e}")));
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .unwrap_or_else(|e| setup_failed(&format!("advance (тёплый слепок): {e}")));
    let before = dir_bytes(dir.path());
    {
        let mut j = Journal::open_with(dir.path(), cfg(seg_bytes))
            .unwrap_or_else(|e| setup_failed(&format!("reopen: {e}")));
        append_trades(&mut j, prefix, tail);
    }
    let tail_bytes = dir_bytes(dir.path()).saturating_sub(before);
    let f = Fixture {
        last_seq: last_seq(dir.path()),
        tail_events: tail,
        dir,
        ckpt,
        tail_bytes,
    };
    if tail > 0 && f.tail_bytes == 0 {
        setup_failed("хвост дописан, а каталог не вырос — мера хвоста пуста");
    }
    f
}

struct FirstPump {
    live: LiveReducer,
    stats: ReadStats,
    rchar_delta: u64,
    frames: usize,
}

/// Тёплый `resume` + РОВНО ОДИН `pump` (он дренирует весь доступный backlog партиями по
/// `PUMP_BATCH`, как прод). `rchar` снимается вокруг `pump`, а не вокруг `resume`: `resume`
/// журнал не читает (замер: `events_scanned=0`, `rchar = размер слепка`).
fn warm_first_pump(f: &Fixture) -> FirstPump {
    let (mut live, resume_stats) = LiveReducer::resume(
        f.dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        f.ckpt.path(),
    )
    .unwrap_or_else(|e| setup_failed(&format!("resume: {e}")));
    if resume_stats.events_scanned != 0 {
        setup_failed(&format!(
            "resume прочитал {} событий — слепок не сработал, и первый pump мерил бы холодный \
             путь, а не тёплый",
            resume_stats.events_scanned
        ));
    }
    let before = rchar();
    let (frames, _cursor, stats) = live
        .pump(f.dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH)
        .unwrap_or_else(|e| setup_failed(&format!("первый pump: {e}")));
    let after = rchar();
    FirstPump {
        live,
        stats,
        rchar_delta: after.saturating_sub(before),
        frames: frames.len(),
    }
}

/// Общие утверждения `I-1` для одного сценария: работа первого pump'а ∝ хвосту.
fn assert_first_pump_bounded(tag: &str, f: &Fixture, p: &FirstPump) {
    assert_eq!(
        p.stats.events_decoded, f.tail_events,
        "{tag}: первый pump отдал редьюсеру {} событий при хвосте {} — хвост либо потерян, \
         либо задвоен",
        p.stats.events_decoded, f.tail_events
    );
    assert!(
        p.stats.events_scanned <= f.tail_events + SEEK_SCAN_ALLOWANCE,
        "{tag} / I-1 (TD-219 по существу): первый pump после тёплого resume ПРОСКАНИРОВАЛ {} \
         событий при хвосте {} (допуск на поиск позиции {SEEK_SCAN_ALLOWANCE}). Парсер прочёл \
         префикс активного сегмента с `header_end`: `tail_hint = None` в обеих ветках \
         `LiveReducer::resume` (`crates/gateway/src/lib.rs`), и `resolve_active_start_offset` \
         без hint возвращает начало (`crates/journal/src/segments.rs`). На проде это 305 тыс. \
         событий / ≈412 МБ на КАЖДУЮ подписку сразу после прогрева и до 1 ГБ к ротации \
         (замер 2026-09-27 12:11Z).",
        p.stats.events_scanned,
        f.tail_events
    );
    assert!(
        p.rchar_delta <= f.tail_bytes + SEEK_READ_ALLOWANCE,
        "{tag} / I-1: ядро видело {} Б чтения за первый pump при хвосте {} Б (допуск на поиск \
         позиции {SEEK_READ_ALLOWANCE} Б). Работа пропорциональна ПРЕФИКСУ, а не хвосту: \
         замер 2026-09-27 — rchar 20 620 325 Б при файле 20 554 613 Б и хвосте ≈25 КБ (×800).",
        p.rchar_delta,
        f.tail_bytes
    );
}

/// `I-2`: состояние после сдвига бит-идентично независимой свёртке того же окна.
fn assert_identical_to_reference(tag: &str, f: &Fixture, live: &LiveReducer) {
    assert_eq!(
        live.cursor(),
        Cursor::at(f.last_seq),
        "{tag}: курсор после первого pump'а не на последнем событии журнала"
    );
    let got = canon(&live.snapshot());
    let want = canon(&reference(f.dir.path(), f.last_seq));
    assert!(
        got == want,
        "{tag} / I-2 (VB-I-2/VB-I-11): снимок после тёплого resume + сдвиг ≠ независимая \
         свёртка `gateway::snapshot(START..{})`. Позиционирование по seq сдвинуло чтение мимо \
         части хвоста или задвоило её. Длины: got={} want={}",
        f.last_seq,
        got.len(),
        want.len()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// w1 — ПРОД-МАСШТАБ: 200 тыс. событий в одном сырьевом сегменте, хвост 500
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **w1 — главный оракул `I-1`.** Один сценарий держит и абсолютные границы, и отношение
/// «большой / малый префикс»: `rchar` процессный, второй замер в том же процессе обязан идти
/// последовательно, поэтому оба живут в одном теле под `serial()`.
#[test]
fn w1_first_pump_after_warm_resume_costs_tail_not_prefix() {
    let _g = serial();

    let small = fixture(PREFIX_SMALL, TAIL, SEG_BYTES_PROD);
    let big = fixture(PREFIX_PROD, TAIL, SEG_BYTES_PROD);

    // SETUP-СТРАЖИ прод-формы: ровно ОДИН сегмент (префикс и хвост в одном файле) и
    // префикс на порядки больше хвоста — иначе «∝ хвосту» и «∝ префиксу» неразличимы.
    for (tag, f) in [("small", &small), ("big", &big)] {
        let segs = segment_files(f.dir.path());
        if segs.len() != 1 {
            setup_failed(&format!(
                "{tag}: сегментов {}, а прод-форма — ОДИН активный сырой сегмент",
                segs.len()
            ));
        }
    }
    let big_file = dir_bytes(big.dir.path());
    if big_file < big.tail_bytes * 100 {
        setup_failed(&format!(
            "big: файл {} Б при хвосте {} Б — префикс меньше стократного, границы не различают \
             предметы",
            big_file, big.tail_bytes
        ));
    }

    let ps = warm_first_pump(&small);
    let pb = warm_first_pump(&big);
    if ps.frames == 0 || pb.frames == 0 {
        setup_failed("первый pump не отдал ни одного кадра — хвост не прочитан, мерилась пустота");
    }

    assert_first_pump_bounded("w1/big", &big, &pb);
    let extra = pb.rchar_delta.saturating_sub(ps.rchar_delta);
    assert!(
        extra <= MAX_PREFIX_DEPENDENT_BYTES,
        "w1 / I-1: первый pump прочитал {} Б при префиксе {PREFIX_PROD} против {} Б при \
         {PREFIX_SMALL} — на {extra} Б больше при {}-кратной разнице длины (допуск \
         {MAX_PREFIX_DEPENDENT_BYTES} Б). Работа первого тика растёт с длиной активного \
         сегмента — замер 2026-09-27: ×109 при ×200.",
        pb.rchar_delta,
        ps.rchar_delta,
        PREFIX_PROD / PREFIX_SMALL
    );
    assert_identical_to_reference("w1/big", &big, &pb.live);

    // Установившийся тик ПОСЛЕ сдвига остаётся дешёвым (страж: сдвиг не сломал hint).
    {
        let mut j = Journal::open_with(big.dir.path(), cfg(SEG_BYTES_PROD))
            .unwrap_or_else(|e| setup_failed(&format!("reopen: {e}")));
        append_trades(&mut j, big.last_seq + 1, 3);
    }
    let mut live = pb.live;
    let (frames, _c, st) = live
        .pump(big.dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH)
        .expect("второй pump");
    assert!(!frames.is_empty(), "3 новых события обязаны дать кадр");
    assert!(
        st.events_scanned <= 3 + SEEK_SCAN_ALLOWANCE,
        "w1: второй тик просканировал {} при приращении 3 — hint после сдвига негоден",
        st.events_scanned
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// p1 — I-4: `payload_bytes_read` у pump'а — от ЧИТАТЕЛЯ, включая активный сегмент
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **p1 — счётчик прочитанных байт `pump`'а согласован с ядром.** Сегодня `pump` кладёт
/// `payload_bytes_read: 0` (`read_stats_from_stream`, «filled by call sites»), а warm-ветка
/// `resume` считает `payload_bytes_after_cursor`, которая активный сегмент НЕ учитывает
/// (замер §3 отчёта: `counter 11 157 < rchar 57 711` в `R-202`). Нижняя граница — байты хвоста
/// на диске (кадры `[len][payload][crc]`, ровно то, что дописано после слепка), верхняя —
/// `rchar` ядра. Литерал, ноль и «размер описи» нарушают одну из двух.
#[test]
fn p1_pump_payload_bytes_read_is_measured_by_the_reader() {
    let _g = serial();
    let f = fixture(PREFIX_MID, TAIL, SEG_BYTES_PROD);
    let p = warm_first_pump(&f);
    if p.frames == 0 {
        setup_failed("первый pump не отдал кадров — мерилась пустота");
    }
    assert!(
        p.stats.payload_bytes_read >= f.tail_bytes,
        "p1 / I-4: `ReadStats.payload_bytes_read` первого pump'а = {} Б, а один только хвост \
         весит {} Б на диске. Счётчик не считает, а подставляет: `pump` кладёт 0, warm-`resume` \
         — опись без активного сегмента (`payload_bytes_after_cursor`). Величина, названная \
         «прочитанные байты», обязана включать прочитанный хвост (задача 15 `M-87`, `R-202`).",
        p.stats.payload_bytes_read,
        f.tail_bytes
    );
    assert!(
        p.stats.payload_bytes_read <= p.rchar_delta,
        "p1 / I-4: счётчик объявил {} Б, а ядро видело {} Б — учесть больше прочитанного \
         невозможно, величина сочиняется",
        p.stats.payload_bytes_read,
        p.rchar_delta
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d1..d9 — I-2: вырожденные входы (testing.md, чек-лист) — каждый со сдвигом и с эталоном
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **d1 — курсор слепка = ПЕРВОЕ событие сегмента.** Граница снизу: позиция «сразу после
/// первого кадра» обязана находиться так же, как любая другая, и не срываться в
/// `header_end` по особому случаю.
#[test]
fn d1_cursor_at_first_event_of_segment() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg(SEG_BYTES_PROD)).expect("open_with");
        append_trades(&mut j, 0, 1);
    }
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance на первом событии");
    let before = dir_bytes(dir.path());
    {
        let mut j = Journal::open_with(dir.path(), cfg(SEG_BYTES_PROD)).expect("reopen");
        append_trades(&mut j, 1, TAIL);
    }
    let f = Fixture {
        tail_bytes: dir_bytes(dir.path()).saturating_sub(before),
        last_seq: last_seq(dir.path()),
        tail_events: TAIL,
        dir,
        ckpt,
    };
    let p = warm_first_pump(&f);
    assert_first_pump_bounded("d1", &f, &p);
    assert_identical_to_reference("d1", &f, &p.live);
}

/// **d2 — хвост ПУСТ: курсор слепка = последнее событие.** Легитимный пустой тик: позиция
/// `pos == len` (`resolve_active_start_offset`, условие 6) — ни одного кадра не читается, а
/// не «нет хвоста ⇒ прочитать всё, чтобы убедиться».
#[test]
fn d2_empty_tail_cursor_at_last_event() {
    let _g = serial();
    let f = fixture(5_000, 0, SEG_BYTES_PROD);
    let p = warm_first_pump(&f);
    assert_eq!(p.frames, 0, "d2: пустой хвост дал кадры");
    assert!(
        p.stats.events_scanned <= SEEK_SCAN_ALLOWANCE,
        "d2 / I-1: пустой хвост, а просканировано {} событий — сегмент перечитан ради того, \
         чтобы убедиться в пустоте",
        p.stats.events_scanned
    );
    assert!(
        p.rchar_delta <= SEEK_READ_ALLOWANCE,
        "d2 / I-1: пустой хвост, а ядро видело {} Б чтения",
        p.rchar_delta
    );
    assert_identical_to_reference("d2", &f, &p.live);
}

/// **d3 — курсор в ЗАКРЫТОМ предыдущем сыром сегменте; хвост пересекает границу сегментов.**
/// Прод-окно: ротация случилась ПОСЛЕ последнего прогрева (≤ 15 мин из ≈ 2.3 ч жизни
/// сегмента, ≈ 11 % времени). Сегмент, содержащий курсор, сегодня читается `Passive`
/// с начала (`open_next_segment`, ветка «закрытый raw») — до 1 ГиБ на проде. Сдвиг обязан
/// применяться к СЕГМЕНТУ, СОДЕРЖАЩЕМУ КУРСОР, а не только к активному.
///
/// Ротация в тесте — уменьшенным `max_segment_bytes`: ротация на 1 ГиБ в тесте неисполнима,
/// а предмет здесь — ГРАНИЦА, не размер.
#[test]
fn d3_cursor_in_closed_raw_segment_tail_crosses_rotation() {
    let _g = serial();
    const SEG: u64 = 512 * 1024;
    let dir = tempfile::tempdir().expect("tempdir");
    // Префикс 6 000 (~300 КиБ) — сегмент 0 ещё активен.
    {
        let mut j = Journal::open_with(dir.path(), cfg(SEG)).expect("open_with");
        append_trades(&mut j, 0, 6_000);
    }
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance (курсор внутри сегмента 0)");
    let before = dir_bytes(dir.path());
    // Хвост 8 000 (~400 КиБ) переполняет сегмент 0 и ротирует в сегмент 1.
    {
        let mut j = Journal::open_with(dir.path(), cfg(SEG)).expect("reopen");
        append_trades(&mut j, 6_000, 8_000);
    }
    let segs = segment_files(dir.path());
    if segs.len() != 2 {
        setup_failed(&format!(
            "d3: сегментов {} — ротация не состоялась, курсор не в закрытом сегменте",
            segs.len()
        ));
    }
    let f = Fixture {
        tail_bytes: dir_bytes(dir.path()).saturating_sub(before),
        last_seq: last_seq(dir.path()),
        tail_events: 8_000,
        dir,
        ckpt,
    };
    let p = warm_first_pump(&f);
    assert_first_pump_bounded("d3", &f, &p);
    assert_identical_to_reference("d3", &f, &p.live);
}

/// **d4 — курсор в `.zst` (компактированном) сегменте — НАЗВАННЫЙ ПРЕДЕЛ.** zstd-декодер не
/// `Seek`; сдвиг без распаковки невозможен. Требуется: корректность (эталон) и «не хуже
/// одного сегмента» — `events_scanned ≤ события этого сегмента + хвост + допуск`. Оракул
/// ЗЕЛЕН сегодня и остаётся стражем: он краснеет, если реализация сдвига на `.zst` начнёт
/// пропускать события или читать больше одного сегмента.
#[test]
fn d4_cursor_in_compacted_segment_is_correct_within_one_segment_limit() {
    let _g = serial();
    const SEG: u64 = 256 * 1024;
    const N0: u64 = 4_000;
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg(SEG)).expect("open_with");
        append_trades(&mut j, 0, N0);
    }
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    let before = dir_bytes(dir.path());
    {
        let mut j = Journal::open_with(dir.path(), cfg(SEG)).expect("reopen");
        append_trades(&mut j, N0, 3_000);
    }
    let tail_bytes = dir_bytes(dir.path()).saturating_sub(before);
    let segs_info = journal::list_segments(dir.path()).expect("segments");
    if segs_info.len() != 2 {
        setup_failed(&format!(
            "d4: сегментов {} — ротация не состоялась",
            segs_info.len()
        ));
    }
    journal::compact_segment(&segs_info[0], journal::DEFAULT_COMPACT_LEVEL)
        .unwrap_or_else(|e| setup_failed(&format!("компакция сегмента 0: {e}")));
    let names: Vec<String> = segment_files(dir.path())
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    if !names.iter().any(|n| n.ends_with(".jrnl.zst")) {
        setup_failed(&format!("d4: после компакции нет .zst: {names:?}"));
    }
    let f = Fixture {
        tail_bytes,
        last_seq: last_seq(dir.path()),
        tail_events: 3_000,
        dir,
        ckpt,
    };
    let p = warm_first_pump(&f);
    assert_eq!(
        p.stats.events_decoded, f.tail_events,
        "d4: хвост потерян или задвоен"
    );
    assert!(
        p.stats.events_scanned <= N0 + f.tail_events + SEEK_SCAN_ALLOWANCE,
        "d4: просканировано {} — больше одного компактированного сегмента + хвост",
        p.stats.events_scanned
    );
    assert_identical_to_reference("d4", &f, &p.live);
}

/// **d6 — «рваный» кадр в конце активного сегмента (писатель дописывает).** На диске после
/// хвоста лежит НЕПОЛНЫЙ кадр (первые байты валидного кадра). Требования: (а) первый pump
/// отдаёт ровно хвост и НЕ срывается в полный перескан (сегодня `probe_frame_boundary` на
/// рваном кадре возвращает `false`, и `resolve_active_start_offset` откатывается в
/// `header_end`); (б) когда кадр дописан — он доставляется РОВНО ОДИН РАЗ; (в) итог равен
/// независимой свёртке.
#[test]
fn d6_torn_frame_at_segment_end_is_deferred_not_rescanned() {
    let _g = serial();
    let f = fixture(PREFIX_MID, TAIL, SEG_BYTES_PROD);
    // Построить валидный кадр следующего события и дописать ТОЛЬКО его первую половину.
    let next = f.last_seq + 1;
    let seg = segment_files(f.dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let full_len_before = std::fs::metadata(&seg).expect("meta").len();
    // Кадр: [u32 len][payload][u32 crc32] — форма `crates/journal/src/segments.rs`.
    let ev = contracts::Event {
        seq: next,
        ts_mono_ns: 0,
        ts_wall_ms: T0,
        kind: trade(next),
    };
    let payload = postcard::to_allocvec(&ev).expect("postcard");
    let mut frame = Vec::with_capacity(payload.len() + 8);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    let cut = frame.len() / 2;
    {
        let mut fh = std::fs::OpenOptions::new()
            .append(true)
            .open(&seg)
            .expect("open append");
        fh.write_all(&frame[..cut]).expect("рваный кадр");
        fh.flush().expect("flush");
    }
    if std::fs::metadata(&seg).expect("meta").len() != full_len_before + cut as u64 {
        setup_failed("рваный кадр не дописан");
    }

    let p = warm_first_pump(&f);
    assert_first_pump_bounded("d6", &f, &p);
    assert_identical_to_reference("d6", &f, &p.live);

    // Писатель дописал остаток кадра.
    {
        let mut fh = std::fs::OpenOptions::new()
            .append(true)
            .open(&seg)
            .expect("open append");
        fh.write_all(&frame[cut..]).expect("остаток кадра");
        fh.flush().expect("flush");
    }
    let mut live = p.live;
    let (frames, _c, st) = live
        .pump(f.dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH)
        .expect("pump после дописки");
    assert_eq!(
        st.events_decoded, 1,
        "d6: дописанный кадр доставлен {} раз(а), а обязан ровно один",
        st.events_decoded
    );
    assert!(!frames.is_empty(), "d6: дописанный кадр не дал кадра");
    assert!(
        st.events_scanned <= 1 + SEEK_SCAN_ALLOWANCE,
        "d6: тик после дописки просканировал {} — рваный кадр сорвал hint в полный перескан",
        st.events_scanned
    );
    let got = canon(&live.snapshot());
    let want = canon(&reference(f.dir.path(), next));
    assert!(got == want, "d6: итог после дописки ≠ независимая свёртка");
}

/// **d7 — испорченный кадр СРАЗУ ЗА курсором ⇒ fail-closed, не тихий пропуск.** Независимый
/// путь (`journal::stream`, строгий CRC) даёт `Err`; сдвинутое чтение обязано дать `Err` ТАК
/// ЖЕ, а не приземлиться за порчей на следующем валидном кадре и обслужить «почти всё».
/// Страж: зелен сегодня (полный перескан упирается в порчу), краснеет против ресинка,
/// перепрыгивающего кадры.
#[test]
fn d7_corrupt_frame_right_after_cursor_fails_closed() {
    let _g = serial();
    let f = fixture(PREFIX_MID, TAIL, SEG_BYTES_PROD);
    corrupt_first_tail_frame(&f, 0);
    // Независимый путь — Err.
    let independent = gateway::snapshot(
        f.dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::at(f.last_seq),
    );
    if independent.is_ok() {
        setup_failed("порча не ловится независимым путём — фикстура испортила не кадр");
    }
    let (mut live, _) = LiveReducer::resume(
        f.dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        f.ckpt.path(),
    )
    .expect("resume");
    let r = live.pump(f.dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH);
    assert!(
        r.is_err(),
        "d7 / I-2: испорченный кадр за курсором ПРОГЛОЧЕН — pump вернул Ok с {} кадрами. \
         Ресинк перепрыгнул порчу и, возможно, валидные события рядом с ней; независимый путь \
         на том же журнале отказывает",
        r.map(|(fr, _, _)| fr.len()).unwrap_or(0)
    );
}

/// **d7b — порча через несколько кадров ПОСЛЕ точки сдвига.** Точка сдвига валидна, порча
/// впереди: оба пути обязаны отказать; курсор `LiveReducer` не сдвигается за порчу.
#[test]
fn d7b_corrupt_frame_a_few_frames_after_seek_point_fails_closed() {
    let _g = serial();
    let f = fixture(PREFIX_MID, TAIL, SEG_BYTES_PROD);
    corrupt_first_tail_frame(&f, 5);
    let (mut live, _) = LiveReducer::resume(
        f.dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        f.ckpt.path(),
    )
    .expect("resume");
    let cursor_before = live.cursor();
    let r = live.pump(f.dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH);
    assert!(r.is_err(), "d7b: порча в хвосте проглочена");
    assert_eq!(
        live.cursor(),
        cursor_before,
        "d7b: курсор доставки сдвинут при отказавшем pump'е"
    );
}

/// **d7c — порча в СЕРЕДИНЕ хвоста (`TAIL/2`, `C-260` N1).** `d7`/`d7b` портят кадры `k=0`
/// и `k=5`; мутант, отдающий EOF после ошибки кадра, ловился бы ими только пока порча лежит
/// в первых 16 кадрах доставки. Здесь порча существенно дальше: оба пути обязаны отказать
/// (`Err`, не усечённый `Ok`), курсор доставки не двигается.
#[test]
fn d7c_corrupt_frame_mid_tail_fails_closed_and_matches_independent_path() {
    let _g = serial();
    let f = fixture(PREFIX_MID, TAIL, SEG_BYTES_PROD);
    corrupt_first_tail_frame(&f, (TAIL / 2) as usize);
    let independent = gateway::snapshot(
        f.dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::at(f.last_seq),
    );
    if independent.is_ok() {
        setup_failed(
            "порча в середине хвоста не ловится независимым путём — фикстура испортила не кадр",
        );
    }
    let (mut live, _) = LiveReducer::resume(
        f.dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        f.ckpt.path(),
    )
    .expect("resume");
    let cursor_before = live.cursor();
    let r = live.pump(f.dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH);
    assert!(
        r.is_err(),
        "d7c / I-2 (JR-I-2): порча на {}-м кадре хвоста ПРОГЛОЧЕНА — pump вернул Ok с {} кадрами; \
         независимый путь `gateway::snapshot` на том же журнале отказывает. Усечение хвоста до \
         порчи = «пропустить», а не abort",
        TAIL / 2,
        r.as_ref().map(|(fr, _, _)| fr.len()).unwrap_or(0)
    );
    assert_eq!(
        live.cursor(),
        cursor_before,
        "d7c: курсор доставки сдвинут при отказавшем pump'е"
    );
}

/// Инвертировать 8 байт полезной нагрузки `k`-го кадра ПОСЛЕ курсора слепка (заголовок и
/// префикс целы). Позиция кадра находится независимым проходом по `[len][payload][crc]`.
fn corrupt_first_tail_frame(f: &Fixture, k: usize) {
    let seg = segment_files(f.dir.path())
        .pop()
        .unwrap_or_else(|| setup_failed("нет сегмента"));
    let mut bytes = std::fs::read(&seg).expect("read segment");
    // Найти начало кадров: первый кадр лежит сразу за заголовком; заголовок ищем как
    // позицию, с которой цепочка [len][payload][crc] с валидными CRC доходит до EOF.
    let mut start = None;
    'outer: for s in 0..bytes.len().min(4096) {
        let mut i = s;
        let mut n = 0usize;
        while i + 8 <= bytes.len() {
            let len = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
            if i + 8 + len > bytes.len() {
                break;
            }
            let crc = u32::from_le_bytes(bytes[i + 4 + len..i + 8 + len].try_into().unwrap());
            if crc32fast::hash(&bytes[i + 4..i + 4 + len]) != crc {
                break;
            }
            n += 1;
            i += 8 + len;
        }
        if i == bytes.len() && n > 0 {
            start = Some(s);
            break 'outer;
        }
    }
    let start = start.unwrap_or_else(|| setup_failed("не найдена цепочка кадров сегмента"));
    // Дойти до кадра с seq == cursor + 1 + k.
    let cursor = f.last_seq - f.tail_events; // = seq последнего события префикса
    let target_seq = cursor + 1 + k as u64;
    let mut i = start;
    let mut hit = None;
    while i + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        // Первый кадр цепочки — заголовок сегмента (тоже `[len][payload][crc]`), он не
        // `Event` и пропускается; всё остальное обязано декодироваться.
        if let Ok(ev) = postcard::from_bytes::<contracts::Event>(&bytes[i + 4..i + 4 + len]) {
            if ev.seq == target_seq {
                hit = Some((i + 4, len));
                break;
            }
        }
        i += 8 + len;
    }
    let (p, len) = hit.unwrap_or_else(|| setup_failed(&format!("кадр seq={target_seq} не найден")));
    if len < 8 {
        setup_failed("кадр короче 8 байт");
    }
    for b in bytes.iter_mut().skip(p).take(8) {
        *b = !*b;
    }
    std::fs::write(&seg, &bytes).expect("write corrupted");
}

/// **d8 — ДВЕ сессии из одного слепка (testing.md п. 7).** Позиция — состояние СЕССИИ, не
/// каталога: обе делают ограниченную работу и приходят к одному состоянию.
#[test]
fn d8_two_sessions_from_same_checkpoint_are_bounded_and_identical() {
    let _g = serial();
    let f = fixture(PREFIX_MID, TAIL, SEG_BYTES_PROD);
    let a = warm_first_pump(&f);
    let b = warm_first_pump(&f);
    assert_first_pump_bounded("d8/a", &f, &a);
    assert_first_pump_bounded("d8/b", &f, &b);
    assert!(
        canon(&a.live.snapshot()) == canon(&b.live.snapshot()),
        "d8: две сессии из одного слепка дали разные снимки"
    );
    assert_identical_to_reference("d8/a", &f, &a.live);
}

/// **d9 — журнал смонтирован ТОЛЬКО ДЛЯ ЧТЕНИЯ (testing.md п. 6).** Прод: `journal-data:/journal:ro`
/// у `gateway-serve` (`docker-compose.yml`). Поиск позиции не имеет права ничего писать —
/// ни sidecar, ни индекс (M-57 круг 2: sidecar `journal.tail-offset` уже провалил три круга
/// на этом). Права выставляются на каталоге и файлах; после работы опись каталога та же.
#[test]
fn d9_read_only_journal_dir_seek_writes_nothing() {
    let _g = serial();
    if unsafe_is_root() {
        setup_failed(
            "тест идёт под root — права носителя не применяются, сценарий не воспроизводим",
        );
    }
    let f = fixture(PREFIX_MID, TAIL, SEG_BYTES_PROD);
    let listing_before = listing(f.dir.path());
    set_readonly(f.dir.path());
    let p = warm_first_pump(&f);
    let listing_after = listing(f.dir.path());
    // Вернуть права ДО утверждений — иначе `TempDir::drop` не уберёт каталог.
    set_writable(f.dir.path());
    assert_first_pump_bounded("d9", &f, &p);
    assert_eq!(
        listing_before, listing_after,
        "d9: опись каталога журнала изменилась — сдвиг что-то записал (или пытался и оставил след)"
    );
    assert_identical_to_reference("d9", &f, &p.live);
}

fn unsafe_is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .map(|s| {
            s.lines()
                .any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0"))
        })
        .unwrap_or(false)
}

fn listing(dir: &std::path::Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                e.metadata().map(|m| m.len()).unwrap_or(0),
            )
        })
        .collect();
    v.sort();
    v
}

fn set_readonly(dir: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    for e in std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
    {
        std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o444)).expect("chmod");
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).expect("chmod dir");
}

fn set_writable(dir: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).expect("chmod dir");
    for e in std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
    {
        std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o644)).expect("chmod");
    }
}
