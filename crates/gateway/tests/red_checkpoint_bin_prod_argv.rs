//! RED M-38b rev4 (sacred, architect-only) — **B1/B2 (reviewer PR-гейт): прод-бинарь обязан
//! стартовать РОВНО теми аргументами, что стоят в `docker-compose.yml`, и публиковать
//! ФАКТИЧЕСКИ достигнутый курсор.**
//!
//! ## Почему этого оракула не было и почему это мой промах
//!
//! `verify_M-38b.sh` проверял бинарь канарейками `test -f` + `grep 'covered_through_seq'` —
//! то есть «файл существует» и «строка встречается». Оба зелёные при бинаре, который НЕ
//! ЗАПУСКАЕТСЯ. Reviewer поймал это исполнением, и был прав: 32/32 PASS при полностью
//! неработоспособном прод-сервисе. Это ровно класс «код на main ≠ функция в проде»
//! (TD-019/TD-020), от которого я предостерегаю в milestone'ах — и сам же его допустил
//! в собственном гейте.
//!
//! **Замер (architect, до фикса):**
//! ```text
//! $ gateway-checkpoint --dir=/tmp/x --ckpt-dir=... --cursor=LATEST      # форма compose
//! gateway-checkpoint: неизвестный флаг `--dir=/tmp/x` (попробуй --help)      exit=1
//! $ gateway-checkpoint --dir /tmp/x ... --cursor LATEST                 # форма «через пробел»
//! gateway-checkpoint: --cursor: invalid digit found in string                exit=1
//! ```
//! Падают ОБЕ формы: `--flag=value` не разбирается вовсе, а `--cursor LATEST` не парсится
//! (`parse::<u64>()`), хотя `LATEST` — прод-дефолт в compose. Соседний `journal-retention`
//! форму `--flag=value` принимает — то есть два ops-бинаря одного проекта имеют РАЗНЫЙ
//! контракт argv.
//!
//! ## Парный vantage (требование reviewer'а RN-25)
//!
//! Аргументы **читаются из `docker-compose.yml`**, а не дублируются здесь константами.
//! Тест, повторяющий argv в своём теле, доказывает лишь самосогласованность и снова
//! пропустит расхождение «compose ↔ парсер» — ровно то, что произошло.
//!
//! ## B2 — семантика `covered_through_seq`
//!
//! Публикуется CLI-аргумент (`args.cursor.upto_seq.unwrap_or(u64::MAX)`), а НЕ курсор
//! реально записанного чекпоинта. При прод-дефолте `--cursor=LATEST` в артефакт уходит
//! `u64::MAX` ⇒ гейт `last_seq(seg) <= covered` пропускает ВСЁ ⇒ строгая связка C-030 R1
//! становится no-op, причём МОЛЧА: `pruned_without_checkpoint_coverage` заполняется только
//! по ветке override, поэтому непокрытый prune не попадает и в аудит. Это инверсия всех трёх
//! условий, на которых C-031 принял escape-hatch.
//!
//! **Требование:** `advance`/`advance_to` возвращают ДОСТИГНУТЫЙ `Cursor`, и публикуется
//! именно он. `u64::MAX` не является допустимым значением артефакта НИКОГДА — «до конца»
//! выражается конкретным seq последнего свёрнутого события.
//!
//! testing.md: п.4 границы (пустой журнал, `--cursor=<seq>` в середине), п.6 композиция
//! (бинарь → артефакт → гейт retention), п.7 парный vantage (argv из compose; и форма
//! `--flag value` тоже обязана работать — сужать контракт не требуется).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{LiveReducer, Selector};
use journal::{EpochFilter, Journal, WriterConfig};
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_gateway-checkpoint");
const N: u64 = 300;

fn compose_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/gateway
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docker-compose.yml")
}

/// Извлечь `command:`-аргументы сервиса из docker-compose.yml и подставить дефолты
/// `${VAR:-default}`. Намеренно примитивный разбор: блок в compose — плоский список строк
/// `- --flag=value`, и зависимость от YAML-парсера здесь была бы дороже пользы.
fn compose_command_args(service: &str) -> Vec<String> {
    let text = std::fs::read_to_string(compose_path()).expect("docker-compose.yml читается");
    let mut out = Vec::new();
    let mut in_service = false;
    let mut in_command = false;
    for line in text.lines() {
        if line.starts_with("  ") && line.trim_end().ends_with(':') && !line.starts_with("    ") {
            let name = line.trim().trim_end_matches(':');
            in_service = name == service;
            in_command = false;
            continue;
        }
        if !in_service {
            continue;
        }
        let t = line.trim();
        if t == "command:" {
            in_command = true;
            continue;
        }
        if in_command {
            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim().trim_matches('"').to_string();
                out.push(subst_env_default(&item));
            } else if !t.starts_with('#') && !t.is_empty() {
                in_command = false; // вышли из списка command
            }
        }
    }
    assert!(
        !out.is_empty(),
        "в docker-compose.yml не найден command-блок сервиса `{service}` — оракул обязан \
         читать прод-аргументы, а не догадываться о них"
    );
    out
}

/// `${VAR:-default}` → `default`; `${VAR}` → пусто (в M-38b таких нет).
fn subst_env_default(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find('}').expect("незакрытая ${...}") + start;
        let inner = &rest[start + 2..end];
        out.push_str(inner.split_once(":-").map(|(_, d)| d).unwrap_or(""));
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Подменить в прод-аргументах ТОЛЬКО пути (на временные), сохранив ФОРМУ записи
/// (`--flag=value` остаётся `--flag=value`). Именно форма и проверяется.
fn retarget_paths(args: &[String], journal: &Path, ckpt: &Path, cov: &Path) -> Vec<String> {
    args.iter()
        .map(|a| {
            for (flag, val) in [
                ("--dir", journal),
                ("--ckpt-dir", ckpt),
                ("--coverage-out", cov),
            ] {
                if let Some(rest) = a.strip_prefix(flag) {
                    if rest.starts_with('=') {
                        return format!("{flag}={}", val.display());
                    }
                    if rest.is_empty() {
                        return a.clone(); // форма «через пробел» — значение отдельным элементом
                    }
                }
            }
            a.clone()
        })
        .collect()
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 8 * 1024,
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
            price: to_fixed(100.0 + (i % 5) as f64),
            size: to_fixed(1.0),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_752_000_000_000 + i as i64 * 100,
        },
    )
}

fn journal_of(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..n {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    dir
}

struct Run {
    code: Option<i32>,
    stderr: String,
}

fn run(args: &[String]) -> Run {
    let out = Command::new(BIN)
        .args(args)
        .output()
        .expect("запуск бинаря");
    Run {
        code: out.status.code(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B1 — прод-argv из docker-compose.yml
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn starts_with_exact_compose_argv() {
    let jdir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = tempfile::tempdir().expect("cov");
    let cov_file = cov.path().join("covered_through_seq");

    let prod = compose_command_args("gateway-checkpoint");
    let args = retarget_paths(&prod, jdir.path(), ckpt.path(), &cov_file);

    let r = run(&args);
    assert_eq!(
        r.code,
        Some(0),
        "B1 НАРУШЕН: прод-бинарь НЕ стартует с аргументами из docker-compose.yml.\n\
         argv (пути подменены на временные, ФОРМА прод-овская): {args:?}\n\
         stderr: {}\n\
         Это не косметика: ops-сервис не поднимется никогда, а канарейки `test -f` + grep \
         этого не видят (класс TD-019/TD-020 «механизм есть, никто не зовёт»).",
        r.stderr.trim()
    );
    assert!(
        cov_file.exists(),
        "успешный прогон обязан опубликовать артефакт покрытия {}",
        cov_file.display()
    );
}

/// Парный vantage: сужать контракт argv нельзя — форма «через пробел» тоже обязана работать
/// (её использует ручной запуск оператора и cron-обёртка). Заглушка, понимающая ТОЛЬКО
/// `--flag=value`, падает здесь.
#[test]
fn starts_with_space_separated_argv_too() {
    let jdir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = tempfile::tempdir().expect("cov");
    let cov_file = cov.path().join("cov");

    let args: Vec<String> = vec![
        "--dir".into(),
        jdir.path().display().to_string(),
        "--ckpt-dir".into(),
        ckpt.path().display().to_string(),
        "--coverage-out".into(),
        cov_file.display().to_string(),
        "--venue".into(),
        "Binance".into(),
        "--symbol".into(),
        "BTCUSDT".into(),
        "--timeframe-ms".into(),
        "1000".into(),
        "--bands".into(),
        "0.001".into(),
        "--window-ms".into(),
        "60000".into(),
        "--cursor".into(),
        "LATEST".into(),
    ];
    let r = run(&args);
    assert_eq!(
        r.code,
        Some(0),
        "форма `--flag value` обязана приниматься наравне с `--flag=value`. stderr: {}",
        r.stderr.trim()
    );
}

/// `--cursor=LATEST` — прод-дефолт в compose. Парсер обязан понимать его как «до конца»,
/// а не пытаться `parse::<u64>()`.
#[test]
fn cursor_latest_is_accepted() {
    let jdir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = tempfile::tempdir().expect("cov");
    let cov_file = cov.path().join("cov");
    let args: Vec<String> = vec![
        format!("--dir={}", jdir.path().display()),
        format!("--ckpt-dir={}", ckpt.path().display()),
        format!("--coverage-out={}", cov_file.display()),
        "--cursor=LATEST".into(),
    ];
    let r = run(&args);
    assert_eq!(
        r.code,
        Some(0),
        "`--cursor=LATEST` — прод-дефолт docker-compose.yml, он обязан парситься. stderr: {}",
        r.stderr.trim()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// B2 — публикуется ФАКТИЧЕСКИ достигнутый курсор, и это НИКОГДА не u64::MAX
// ─────────────────────────────────────────────────────────────────────────────

fn read_coverage(p: &Path) -> u64 {
    std::fs::read_to_string(p)
        .expect("артефакт покрытия читается")
        .trim()
        .parse::<u64>()
        .expect("артефакт покрытия — число")
}

#[test]
fn published_coverage_is_real_cursor_not_max() {
    let jdir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = tempfile::tempdir().expect("cov");
    let cov_file = cov.path().join("cov");

    let args: Vec<String> = vec![
        format!("--dir={}", jdir.path().display()),
        format!("--ckpt-dir={}", ckpt.path().display()),
        format!("--coverage-out={}", cov_file.display()),
        "--cursor=LATEST".into(),
    ];
    assert_eq!(run(&args).code, Some(0), "прогон обязан быть успешным");

    let covered = read_coverage(&cov_file);
    assert_ne!(
        covered,
        u64::MAX,
        "B2 НАРУШЕН: опубликовано u64::MAX. Гейт retention `last_seq(seg) <= covered` \
         пропустит ВСЁ ⇒ строгая связка C-030 R1 превращается в no-op, причём МОЛЧА \
         (`pruned_without_checkpoint_coverage` заполняется только по ветке override). \
         «До конца» обязано выражаться конкретным seq последнего свёрнутого события."
    );
    assert_eq!(
        covered,
        N - 1,
        "опубликованное покрытие обязано равняться курсору РЕАЛЬНО записанного чекпоинта \
         (последний seq журнала = {}), а не CLI-аргументу",
        N - 1
    );
}

/// Граница (п.4): усечённый прогон `--cursor=<seq>` публикует ровно достигнутый seq.
#[test]
fn published_coverage_matches_explicit_cursor() {
    let jdir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = tempfile::tempdir().expect("cov");
    let cov_file = cov.path().join("cov");
    let k = N / 2;

    let args: Vec<String> = vec![
        format!("--dir={}", jdir.path().display()),
        format!("--ckpt-dir={}", ckpt.path().display()),
        format!("--coverage-out={}", cov_file.display()),
        format!("--cursor={k}"),
    ];
    assert_eq!(run(&args).code, Some(0), "прогон обязан быть успешным");
    assert_eq!(
        read_coverage(&cov_file),
        k,
        "при `--cursor={k}` покрытие обязано быть ровно {k}"
    );
}

/// Граница (п.4): на ПУСТОМ журнале свёрнуто ничего — артефакт не должен утверждать покрытие.
/// Молчаливая публикация «покрыто до MAX» на пустом журнале разрешила бы prune всего.
#[test]
fn empty_journal_publishes_no_coverage_claim() {
    let jdir = journal_of(0);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = tempfile::tempdir().expect("cov");
    let cov_file = cov.path().join("cov");

    let args: Vec<String> = vec![
        format!("--dir={}", jdir.path().display()),
        format!("--ckpt-dir={}", ckpt.path().display()),
        format!("--coverage-out={}", cov_file.display()),
        "--cursor=LATEST".into(),
    ];
    let r = run(&args);
    assert_eq!(
        r.code,
        Some(0),
        "пустой журнал — не ошибка. stderr: {}",
        r.stderr.trim()
    );
    if cov_file.exists() {
        let covered = read_coverage(&cov_file);
        assert_ne!(
            covered,
            u64::MAX,
            "на пустом журнале опубликовано покрытие u64::MAX — retention получил бы право \
             удалить всё, ничего не свернув"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// C3ter — КОМПОЗИЦИЯ ПИСАТЕЛЬ ↔ ЧИТАТЕЛЬ, предъявленная ИСПОЛНЕНИЕМ (R-145 Б-2)
// ─────────────────────────────────────────────────────────────────────────────

/// Селектор читателя ровно тот, что compose даёт службе `gateway-serve`.
fn reader_selector(cadence: Option<i64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
        depth_cadence_ms: cadence,
    }
}

/// **`C3ter` — ПИСАТЕЛЬ И ЧИТАТЕЛЬ ЧЕКПОИНТА НАХОДЯТ ОДИН СЛЕПОК.**
///
/// Исполнение условия 1 `R-141`: «композиция обязана быть предъявлена ОРАКУЛОМ ТОЧКИ ВХОДА,
/// не грепом». Круг 4 заменил его лексическим инвентарём, обосновав это утверждением, что
/// из интеграционного теста Rust композиция недостижима, потому что `selector_fingerprint`
/// и `ckpt_path_for` — `pub(super)`.
///
/// **Утверждение было ЛОЖНЫМ, и это моя ошибка, а не спорная оценка.** Отпечаток вычислять
/// НЕ НУЖНО: композиция наблюдаема ПОВЕДЕНЧЕСКИ. Ревьюер предъявил рабочую пробу за один
/// заход (`R-145` Б-2, ~110 строк, 3.1 с). Здесь она приведена к форме набора и поставлена
/// на место инвентаря. Прецедент, который этим закрывается, важнее самого оракула: требование
/// гейта было снято обоснованием, которое опровергается прогоном за десять минут — так можно
/// закрыть любое неудобное требование, и механизм долговых карточек обесценивается.
///
/// # Что именно судится
///
/// Прод-писатель — НАСТОЯЩИЙ бинарь `gateway-checkpoint`, запущенный с argv из
/// `docker-compose.yml` (не смоделированный вызов библиотеки). Прод-читатель — публичный
/// `LiveReducer::resume`. Свидетель — `ReadStats::events_decoded`: слепок НАЙДЕН ⇒ читать
/// журнал не нужно ⇒ `0`. Слепок НЕ найден ⇒ полный реплей ⇒ `N`.
///
/// Имя файла чекпоинта есть `selector_fingerprint`, и `depth_cadence_ms` в него ВХОДИТ —
/// поэтому расхождение каденции между службами делает слепок ненаходимым, и `gateway-serve`
/// перечитывает журнал при КАЖДОМ подключении (`TD-044`: сотни секунд, против которых
/// строились `M-38b`/`M-48`/`M-54`).
///
/// # Парный vantage — обязателен
///
/// Без контроля «каденция РАСХОДИТСЯ ⇒ слепок НЕ найден» оракул был бы зелен и у реализации,
/// которая игнорирует каденцию в отпечатке вовсе.
#[test]
fn c3ter_writer_and_reader_agree_on_checkpoint() {
    let journal = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    let cov = ckpt.path().join("covered_through_seq");

    // ПИСАТЕЛЬ: прод-бинарь, argv из compose, пути перенацелены на фикстуру.
    let args = retarget_paths(
        &compose_command_args("gateway-checkpoint"),
        journal.path(),
        ckpt.path(),
        &cov,
    );
    // SETUP-GUARD: argv обязан НЕСТИ каденцию — иначе тест судит не композицию, а её отсутствие.
    if !args.iter().any(|a| a.starts_with("--depth-cadence-ms")) {
        panic!(
            "SETUP НЕ СОСТОЯЛСЯ: в argv службы gateway-checkpoint нет --depth-cadence-ms. \
             Композиция не может быть предъявлена: ручка не доходит до писателя, и это \
             ОТДЕЛЬНЫЙ дефект (задача 23), а не зелёный этого оракула. argv: {args:?}"
        );
    }
    let w = run(&args);
    if w.code != Some(0) {
        panic!(
            "SETUP НЕ СОСТОЯЛСЯ: прод-писатель вышел с {:?}: {}",
            w.code, w.stderr
        );
    }

    // ЧИТАТЕЛЬ: публичный прод-путь, каденция ТА ЖЕ, что compose даёт обеим службам.
    let (_r, stats) = LiveReducer::resume(
        journal.path(),
        EpochFilter::OwnCaptureOnly,
        &reader_selector(Some(1_000)),
        ckpt.path(),
    )
    .expect("resume читателя");

    assert_eq!(
        stats.events_decoded, 0,
        "КОМПОЗИЦИЯ РАЗОРВАНА: читатель при каденции 1000 мс декодировал {} событий вместо 0 — \
         слепок, только что записанный ПРОД-БИНАРЁМ с argv из docker-compose.yml, не найден. \
         Имя файла чекпоинта есть selector_fingerprint, и depth_cadence_ms в него входит: \
         значит писатель и читатель разошлись отпечатком, и gateway-serve будет реплеить \
         журнал ЦЕЛИКОМ при каждом подключении (TD-044). Это условие 1 R-141, и предъявляется \
         оно ИСПОЛНЕНИЕМ, а не грепом по строке в исходнике",
        stats.events_decoded
    );

    // ПАРНЫЙ VANTAGE: расхождение каденции обязано слепок ПОТЕРЯТЬ.
    let (_r2, control) = LiveReducer::resume(
        journal.path(),
        EpochFilter::OwnCaptureOnly,
        &reader_selector(None),
        ckpt.path(),
    )
    .expect("resume контроля");
    assert!(
        control.events_decoded > 0,
        "КОНТРОЛЬ НЕ СОСТОЯЛСЯ: читатель с ДРУГОЙ каденцией (None против 1000) тоже нашёл \
         слепок — значит depth_cadence_ms в отпечаток НЕ входит, и основной ассерт выше \
         зелен по неверной причине. Оракул, у которого положительный и отрицательный случаи \
         неразличимы, не пиннит ничего"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// d18g — СИММЕТРИЯ ОТКАЗА: писатель и читатель судят одну переменную одинаково
// ─────────────────────────────────────────────────────────────────────────────

const CADENCE_VAR: &str = "GATEWAY_DEPTH_CADENCE_MS";

/// Прогон прод-бинаря с ЗАДАННЫМ значением переменной каденции. `None` — переменная снята.
fn run_with_cadence(args: &[String], value: Option<&str>) -> Run {
    let mut cmd = Command::new(BIN);
    cmd.args(args);
    match value {
        Some(v) => cmd.env(CADENCE_VAR, v),
        None => cmd.env_remove(CADENCE_VAR),
    };
    let out = cmd.output().expect("запуск бинаря");
    Run {
        code: out.status.code(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

/// **`d18g` — НЕВАЛИДНОЕ ЗНАЧЕНИЕ ОТВЕРГАЕТСЯ ОБОИМИ ПРОЦЕССАМИ, И ОТКАЗ НАЗЫВАЕТ ПЕРЕМЕННУЮ.**
///
/// Исполнение `R-145` `N-1` / `R-147` `N-1`. Оракул написан по формулировке `SCOPE VIOLATION
/// REQUEST` engine-dev'а: он остановился верно — правка без RED-оракула нарушила бы
/// `gates.md` §2, а тесты sacred и писать их на собственную реализацию dev не вправе.
///
/// # Предмет — СИММЕТРИЯ, а не текст сообщения
///
/// `gateway-checkpoint` (писатель чекпоинта) и `gateway-serve` (читатель) делят ОДНУ
/// переменную, и её значение входит в `selector_fingerprint` — имя файла слепка. До фикса
/// писатель глотал мусор через `parse().ok()` и молча брал дефолт 1000, а
/// `serve_config_from_env` тот же мусор ОТВЕРГАЛ. Оператор с опечаткой в `docker-compose.yml`
/// получал писателя на 1000 и читателя, не поднявшегося вовсе: расхождение ПОВЕДЕНИЯ на одном
/// входе хуже обоих чистых исходов. Замер до фикса (Done Block engine-dev'а):
/// `abc`, `1000.0`, `1_000` → `exit=0` и молчаливый дефолт.
///
/// # Парный vantage ОБЯЗАТЕЛЕН и защищает подписанное решение
///
/// Отсутствие, пустая строка и строка из пробелов обязаны остаться `exit=0` с дефолтом
/// 1000 — это `A-015` §3 п.1, отдельное решение founder'а. Без этой половины прошла бы
/// переширокая реализация «отвергать всё, что не разбирается в число», которая сломала бы
/// подписанную политику отсутствия. Оракул, красный против правильного кода, хуже
/// отсутствующего.
///
/// # Что оракул НЕ пиннит
///
/// Текст сообщения дословно не сверяется — только присутствие ИМЕНИ переменной. Дежурный по
/// §8 eyes-on ищет ручку, а не формулировку; требовать побайтного совпадения двух процессов
/// значило бы пиннить конструкцию вместо свойства.
#[test]
fn d18g_garbage_cadence_is_rejected_naming_the_variable() {
    let journal = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    let cov = ckpt.path().join("covered_through_seq");
    let args = retarget_paths(
        &compose_command_args("gateway-checkpoint"),
        journal.path(),
        ckpt.path(),
        &cov,
    );
    // Прод-argv несёт `--depth-cadence-ms`, и ФЛАГ имеет приоритет над env. Чтобы судить
    // именно env-путь, флаг из argv убирается — иначе тест мерил бы не то, что обещает.
    let args: Vec<String> = args
        .into_iter()
        .filter(|a| !a.starts_with("--depth-cadence-ms"))
        .collect();
    if args.iter().any(|a| a.starts_with("--depth-cadence-ms")) {
        panic!("SETUP НЕ СОСТОЯЛСЯ: флаг каденции не убран из argv — env-путь не проверяется");
    }

    // ── ОТВЕРГАЕТСЯ: невалидное ЗНАЧЕНИЕ ────────────────────────────────────────────
    for bad in ["abc", "-1", "0", "1000.0", "1_000", "999"] {
        let r = run_with_cadence(&args, Some(bad));
        assert_eq!(
            r.code,
            Some(2),
            "{CADENCE_VAR}={bad:?} принято писателем (exit={:?}), тогда как соседний \
             serve_config_from_env его ОТВЕРГАЕТ. Значение входит в selector_fingerprint: \
             оператор с опечаткой получит писателя на молчаливом дефолте 1000 и читателя, \
             не поднявшегося вовсе — слепок не найдётся, журнал будет перечитываться целиком \
             при каждом подключении (TD-044). stderr: {}",
            r.code,
            r.stderr.trim()
        );
        assert!(
            r.stderr.contains(CADENCE_VAR),
            "{CADENCE_VAR}={bad:?} отвергнут, но сообщение НЕ НАЗЫВАЕТ переменную: {:?}. \
             Дежурный по §8 eyes-on видит упавший контейнер и обязан узнать ручку из текста, \
             а не искать её по исходникам (класс R-143 B-3: ложный диагноз дороже отказа)",
            r.stderr.trim()
        );
    }

    // ── ПРИНИМАЕТСЯ: отсутствие ≡ пустое ≡ пробельное ≡ подписанный дефолт (A-015 §3 п.1) ──
    for ok in [None, Some(""), Some("   "), Some("1000")] {
        let r = run_with_cadence(&args, ok);
        assert_eq!(
            r.code,
            Some(0),
            "{CADENCE_VAR}={ok:?} отвергнуто стартом (exit={:?}). A-015 §3 п.1 — подписанное \
             решение founder'а: отсутствие, пустая строка и пробельная эквивалентны и дают \
             дефолт 1000. Гвард стал ШИРЕ собственного контракта и теперь красен против \
             ПРАВИЛЬНОЙ конфигурации. stderr: {}",
            r.code,
            r.stderr.trim()
        );
    }
}
