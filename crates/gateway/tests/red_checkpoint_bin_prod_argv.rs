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
use journal::{Journal, WriterConfig};
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
