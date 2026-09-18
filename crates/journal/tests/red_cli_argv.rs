//! SACRED (architect-only) — TD-024: CLI `journal-retention` обязан принимать equals-форму.
//!
//! Дефект (reviewer, §8 M-08 rev10): ручной arg-парсер бинаря сравнивает аргумент ЦЕЛИКОМ
//! (`match arg { "--dir" => next() }`) и берёт значение СЛЕДУЮЩИМ элементом argv, т.е. понимает
//! ТОЛЬКО раздельную форму `--dir X`. А `docker-compose.yml` держит `command:` в equals-форме
//! (`--dir=/journal`, `--mode=compact`), и её же печатает `--help`, и её же естественно набирает
//! человек. Следствие: `docker compose run --rm journal-compaction` падает «неизвестный флаг»
//! (exit 1) ⇒ операторский путь компакции/ретеншена через compose НЕ РАБОТАЕТ; работает только
//! cron-скрипт (полный раздельный argv). Хуже: append-команда из README
//! (`docker compose run --rm journal-retention --mode apply`) ЗАМЕНЯЕТ весь command-блок → теряет
//! `--dir=/journal` → бинарь берёт DEFAULT_DIR `./journal-data` вместо боевого `/journal`.
//!
//! Это ЧЕТВЁРТЫЙ виток одного и того же argv-дефекта в M-08 (D5 grep → D5 argv стаб → D5a → TD-024).
//! Правильный фикс — не заставлять всех под одну форму, а научить парсер понимать ОБЕ
//! (`split_once('=')`): тогда compose, README, `--help` и cron согласованы, и `--help` перестаёт
//! врать про контракт.
//!
//! Оракул гоняет НАСТОЯЩИЙ бинарь (`CARGO_BIN_EXE_journal-retention`) — грепом по исходнику этот
//! класс не ловится (урок всего M-08: «функция существует в проде» доказывается ЗАПУСКОМ).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_journal-retention");

fn run(args: &[String]) -> (i32, String) {
    let out = Command::new(BIN)
        .args(args)
        .output()
        .expect("запуск journal-retention");
    let code = out.status.code().unwrap_or(-1);
    let mut s = String::from_utf8_lossy(&out.stderr).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    (code, s)
}

/// TD-024: equals-форма (`--flag=value`) — ровно та, что в `docker-compose.yml command:` и в
/// `--help`. Парсер обязан её принимать (dry-run на пустом каталоге отрабатывает, exit 0).
#[test]
fn td024_cli_accepts_equals_form() {
    let tmp = tempfile::tempdir().expect("dir");
    let d = tmp.path().to_str().unwrap();
    let (code, out) = run(&[
        format!("--dir={d}"),
        "--mode=dry-run".to_string(),
        "--min-free-gb=0".to_string(),
    ]);
    assert_eq!(
        code, 0,
        "equals-форма отвергнута (exit={code}): docker-compose `command:` использует ровно её \
         (`--dir=/journal`, `--mode=compact`) ⇒ `docker compose run journal-compaction` падает \
         «неизвестный флаг», операторский путь через compose не работает. Вывод:\n{out}"
    );
}

/// Тот же контракт для compact-режима equals-формой (сервис `journal-compaction`).
#[test]
fn td024_cli_accepts_equals_form_compact_mode() {
    let tmp = tempfile::tempdir().expect("dir");
    let d = tmp.path().to_str().unwrap();
    let (code, out) = run(&[
        format!("--dir={d}"),
        "--keep-raw=2".to_string(),
        "--mode=compact".to_string(),
    ]);
    assert_eq!(
        code, 0,
        "compact equals-форма отвергнута (exit={code}) — это argv сервиса journal-compaction. \
         Вывод:\n{out}"
    );
}

/// Раздельная форма (как в cron-скрипте) обязана ПРОДОЛЖАТЬ работать — регрессии нет.
#[test]
fn td024_cli_still_accepts_separate_form() {
    let tmp = tempfile::tempdir().expect("dir");
    let d = tmp.path().to_str().unwrap();
    let (code, out) = run(&[
        "--dir".to_string(),
        d.to_string(),
        "--mode".to_string(),
        "dry-run".to_string(),
        "--min-free-gb".to_string(),
        "0".to_string(),
    ]);
    assert_eq!(
        code, 0,
        "раздельная форма сломалась (exit={code}) — cron-путь перестал работать. Вывод:\n{out}"
    );
}
