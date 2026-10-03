//! RED M-87 круг `R-200` (sacred, architect-only) — **прод-бинарь выдачи обязан
//! ПОДНИМАТЬСЯ на прод-окружении, а не только компилироваться.**
//!
//! ## Находка, ради которой оракул и потребовался
//!
//! Ревьюер написал в `R-200` §B6: задача 14 «предъявлена только компиляцией — прод-бинарь
//! выдачи не запускает ни один тест». Написание этого оракула немедленно вскрыло, ЧТО
//! именно пряталось за отсутствием прогона:
//!
//! ```text
//! $ env -i … <всё окружение сервиса gateway-serve из docker-compose.yml> ./gateway-serve
//! gateway-serve: policy error: GATEWAY_ALLOWED_SYMBOLS must be set
//! EXIT=2
//! ```
//!
//! `admission_policy_from_env` требует ПЯТЬ переменных как ОБЯЗАТЕЛЬНЫЕ
//! (`GATEWAY_ALLOWED_SYMBOLS`, `GATEWAY_ALLOWED_PROFILES`, `GATEWAY_MAX_CONCURRENT_SERVES`,
//! `GATEWAY_MAX_TAIL_EVENTS`, `GATEWAY_EXPECTED_WARMUP_EVENTS`), и **ни одной из них нет в
//! `docker-compose.yml`**. То есть после выкатки милестоуна сервис выдачи не поднялся бы
//! ВООБЩЕ, а `restart: unless-stopped` крутил бы его по кругу.
//!
//! Это тот самый класс, ради которого `testing.md` требует оракул точки входа: `test -f
//! binary` и `grep name docker-compose.yml` бывают зелёными, когда бинарь не стартует ни
//! при какой форме argv. Здесь он был зелёным при бинаре, который гарантированно падает.
//!
//! ## Почему проверка идёт ЧЕРЕЗ COMPOSE, а не через константы в теле теста
//!
//! Тест, повторяющий окружение своими константами, доказывает лишь самосогласованность и
//! пропустит ровно то расхождение, которое произошло: «прод задаёт одно, парсер требует
//! другое». Поэтому окружение ЧИТАЕТСЯ из `docker-compose.yml` — прецедент и образец:
//! `crates/gateway/tests/red_checkpoint_bin_prod_argv.rs` (M-38b, найдено тем же способом
//! и тем же ревьюером).
//!
//! ## Что здесь пиннится
//!
//! · `e1` — на окружении ИЗ COMPOSE бинарь поднимается (не выходит с ошибкой политики);
//! · `e2` — парный vantage: несогласованная политика валит СТАРТ с названием ОБОИХ чисел,
//!   то есть fail-closed разбор работает и молчаливого дефолта нет;
//! · `e3` — setup-страж: перечень обязательных переменных снят С КОДА, а не переписан
//!   руками, и compose обязан покрывать его ЦЕЛИКОМ. Именно этот страж краснеет сегодня.
//!
//! COMPILE-зелёный, RUNTIME-КРАСНЫЙ по построению: `e1`/`e3` падают, пока compose не несёт
//! переменных политики. Чинит это engine-dev (зона `docker-compose.yml`), не architect.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` = crates/gateway-serve
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("корень репозитория")
        .to_path_buf()
}

fn compose_text() -> String {
    let p = repo_root().join("docker-compose.yml");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("чтение {}: {e}", p.display()))
}

/// Блок `environment:` сервиса `gateway-serve` из `docker-compose.yml`, разобранный в пары.
/// Подстановка `${VAR:-default}` берёт ДЕФОЛТ — так ведёт себя compose, когда переменной
/// нет в host `.env`, и это и есть прод-состояние по умолчанию.
fn compose_env() -> BTreeMap<String, String> {
    let text = compose_text();
    let mut out = BTreeMap::new();
    let mut in_service = false;
    let mut in_env = false;
    for line in text.lines() {
        if line.starts_with("  gateway-serve:") {
            in_service = true;
            continue;
        }
        if in_service && line.starts_with("  ") && !line.starts_with("    ") {
            break; // следующий сервис
        }
        if !in_service {
            continue;
        }
        if line.trim_start().starts_with("environment:") {
            in_env = true;
            continue;
        }
        if in_env && line.starts_with("    ") && !line.starts_with("      ") {
            in_env = false;
        }
        if !in_env {
            continue;
        }
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some((k, v)) = t.split_once(':') else {
            continue;
        };
        let k = k.trim();
        if !k.starts_with("GATEWAY_") {
            continue;
        }
        out.insert(k.to_string(), subst_default(v.trim()));
    }
    assert!(
        !out.is_empty(),
        "SETUP-СТРАЖ: в docker-compose.yml не найден блок environment сервиса gateway-serve — \
         разбор смотрит не туда, и все сценарии зеленели бы на пустоте"
    );
    out
}

/// `${VAR:-default}` → `default`; `${VAR:?msg}` → подставляется проба (на проде это
/// обязательная переменная оператора, её отсутствие — отдельный контур).
fn subst_default(raw: &str) -> String {
    let s = raw.trim().trim_matches('"');
    if let Some(inner) = s.strip_prefix("${").and_then(|x| x.strip_suffix('}')) {
        if let Some((_, d)) = inner.split_once(":-") {
            return d.to_string();
        }
        if inner.contains(":?") {
            return "probe-value".to_string();
        }
    }
    s.to_string()
}

/// Обязательные переменные политики — снимаются С КОДА (`admission_policy_from_env`), а не
/// переписываются здесь руками: перечень, продублированный в тесте, разойдётся с кодом ровно
/// так же, как compose разошёлся с парсером.
fn required_policy_vars() -> Vec<String> {
    let src = std::fs::read_to_string(repo_root().join("crates/gateway-serve/src/lib.rs"))
        .expect("чтение lib.rs");
    let mut out = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        // форма отказа: `.ok_or_else(|| "GATEWAY_X must be set".to_string())?`
        let Some(i) = t.find("\"GATEWAY_") else {
            continue;
        };
        if !t.contains("must be set") {
            continue;
        }
        let rest = &t[i + 1..];
        if let Some(j) = rest.find(' ') {
            out.push(rest[..j].to_string());
        }
    }
    out.sort();
    out.dedup();
    assert!(
        !out.is_empty(),
        "SETUP-СТРАЖ: в коде не найдено ни одной обязательной переменной политики — \
         разбор формы отказа устарел, и e3 зеленел бы на пустоте"
    );
    out
}

fn bin_path() -> PathBuf {
    // Тест-бинарь лежит в target/<profile>/deps/…; сам бинарь — двумя уровнями выше.
    let exe = std::env::current_exe().expect("current_exe");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("target/<profile>");
    dir.join("gateway-serve")
}

struct Run {
    code: Option<i32>,
    out: String,
}

fn run_with(env: &BTreeMap<String, String>, journal: &Path, ckpt: &Path) -> Run {
    let bin = bin_path();
    assert!(
        bin.exists(),
        "SETUP-СТРАЖ: прод-бинарь не собран ({}). Оракул точки входа обязан ИСПОЛНЯТЬ \
         границу процесса, а не проверять наличие файла",
        bin.display()
    );
    let mut cmd = Command::new(&bin);
    cmd.env_clear().env("PATH", "/usr/bin:/bin");
    for (k, v) in env {
        // Пути тома подменяются на временные: /journal и /ckpt существуют только в контейнере.
        let v = match k.as_str() {
            "GATEWAY_JOURNAL_DIR" => journal.display().to_string(),
            "GATEWAY_CHECKPOINT_DIR" => ckpt.display().to_string(),
            "GATEWAY_ADDR" => "127.0.0.1:0".to_string(),
            _ => v.clone(),
        };
        cmd.env(k, v);
    }
    // Бинарь — ДОЛГОЖИВУЩИЙ сервер: успех виден как «не завершился за отведённое время».
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("запуск прод-бинаря");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        if let Some(st) = child.try_wait().expect("try_wait") {
            let o = child.wait_with_output().expect("output");
            return Run {
                code: st.code(),
                out: format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                ),
            };
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let o = child.wait_with_output().expect("output");
            return Run {
                code: None, // жив — то есть поднялся
                out: format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                ),
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// **e3 — SETUP-СТРАЖ И ГЛАВНАЯ НАХОДКА: compose покрывает ВСЕ обязательные переменные.**
///
/// Перечень снят с кода, а не переписан. Сегодня он краснеет: пять переменных политики
/// обязательны в коде и отсутствуют в прод-описании.
#[test]
fn e3_compose_declares_every_required_policy_var() {
    let env = compose_env();
    let required = required_policy_vars();
    let missing: Vec<&String> = required.iter().filter(|k| !env.contains_key(*k)).collect();
    assert!(
        missing.is_empty(),
        "docker-compose.yml НЕ объявляет обязательные переменные политики: {missing:?}.\n\
         `admission_policy_from_env` отвергает старт при отсутствии любой из них, поэтому \
         после выкатки сервис выдачи не поднимется ВООБЩЕ, а `restart: unless-stopped` будет \
         крутить его по кругу. Перечень снят С КОДА ({} переменных), а не переписан в тесте.",
        required.len()
    );
}

/// **e1 — на окружении ИЗ COMPOSE прод-бинарь ПОДНИМАЕТСЯ.**
///
/// Успех наблюдается как «процесс жив через отведённое время», а не как код ответа: сервер
/// долгоживущий, и завершение — это и есть провал.
#[test]
fn e1_prod_binary_starts_on_compose_environment() {
    let env = compose_env();
    let journal = tempfile::tempdir().expect("journal tempdir");
    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    let r = run_with(&env, journal.path(), ckpt.path());
    assert!(
        r.code.is_none(),
        "прод-бинарь ВЫШЕЛ с кодом {:?} на окружении из docker-compose.yml — \
         значит на проде он не поднимется. Вывод:\n{}",
        r.code,
        r.out
    );
}

/// **e2 — ПАРНЫЙ VANTAGE: несогласованная политика валит СТАРТ и называет ОБА числа.**
///
/// Без него `e1` был бы удовлетворён реализацией, которая принимает ЛЮБУЮ политику, в том
/// числе делающую исправную выдачу невозможной. Проверяется прод-путь целиком: env → разбор
/// → `validate()` → отказ процесса.
#[test]
fn e2_inconsistent_policy_refuses_to_start_naming_both_numbers() {
    let mut env = compose_env();
    for (k, v) in required_policy_vars().iter().zip(std::iter::repeat("")) {
        let _ = v;
        // Заполняем обязательные переменные заведомо валидными значениями…
        let val = match k.as_str() {
            "GATEWAY_ALLOWED_SYMBOLS" => "BTCUSDT",
            "GATEWAY_ALLOWED_PROFILES" => "1000/60000/none",
            "GATEWAY_MAX_CONCURRENT_SERVES" => "4",
            "GATEWAY_MAX_TAIL_EVENTS" => "100",
            "GATEWAY_EXPECTED_WARMUP_EVENTS" => "500", // …кроме ПАРЫ: порог НИЖЕ прогрева
            _ => "1",
        };
        env.insert(k.clone(), val.to_string());
    }
    let journal = tempfile::tempdir().expect("journal tempdir");
    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    let r = run_with(&env, journal.path(), ckpt.path());
    assert!(
        r.code.is_some(),
        "порог свежести НИЖЕ объёма цикла прогрева делает исправную выдачу невозможной, \
         а процесс всё равно поднялся. Отказ СТАРТА — требование §4.1"
    );
    assert!(
        r.out.contains("100") && r.out.contains("500"),
        "отказ обязан назвать ОБА числа (порог 100 и прогрев 500), иначе оператор не узнает, \
         что именно чинить. Вывод:\n{}",
        r.out
    );
}
