//! `M-89` (sacred, architect-only) — **СТОРОЖ сериализации `red_m87_entrypoint.rs`**
//! (`TD-225`): пока в предмете существует `SERIAL`, КАЖДЫЙ асинхронный тест обязан брать его
//! ПЕРВЫМ исполняемым действием тела.
//!
//! # Почему отдельный бинарь, а не тест «в том же файле»
//!
//! `red_m87_registry.rs::r1` держит БИЕКЦИЮ реестра с перечнем сценариев
//! `red_m87_entrypoint.rs` (`A-040`); любой новый `#[test]` в предмете — строка реестра.
//! Сторож формы предмета реестру не принадлежит, и корпус уже решил этот случай той же
//! формой: `red_m87_registry.rs` судит предмет через `include_str!` из соседнего бинаря.
//!
//! # Что проверяется — ПО-ФУНКЦИОННО, а не счётом (`C-260` N4)
//!
//! Первая редакция сравнивала ЧИСЛО атрибутов `#[tokio::test` с ЧИСЛОМ строк
//! `SERIAL.lock().await`: тест, читающий процессный счётчик ДО блокировки, при второй
//! блокировке в соседнем тесте давал равенство — и класс `TD-224` возвращался молча. Здесь
//! для КАЖДОЙ функции под `#[tokio::test…]` находится тело и требуется, чтобы ПЕРВАЯ
//! исполняемая строка (пустые и `//`-комментарии пропускаются) лексически равнялась
//! `let _serial = SERIAL.lock().await;`. Нарушители называются по имени. Счёт остаётся
//! вторым, более слабым стражем (лишняя блокировка вне тестов — тоже сигнал).
//!
//! Шаблоны собираются конкатенацией, чтобы литералы не встречались в этом файле целиком
//! (сторож читает предмет `include_str!`, не себя, — но привычка корпуса сохранена).
//!
//! # Условие снятия (спека `M-89` §Tasks, задача 7)
//!
//! Когда `SERIAL` исчезает из предмета (счётчики переведены на экземплярные, `I-5`) — есть
//! объявление ⇒ требуется блокировка первой строкой в каждом тесте; нет объявления ⇒
//! требуется, чтобы и вызовов не осталось.
//!
//! # Проба-мутанты — в этом же файле
//!
//! `m1`…`m6`: сама проверка прогоняется над инлайн-источниками — переставленная строка,
//! блокировка отсутствует в одном тесте при лишней в другом (счёт сходится — по-функционная
//! проверка обязана поймать), комментарий перед блокировкой (допустим), многострочная
//! сигнатура, отсутствие объявления. Сторож, не различающий эти случаи, — плацебо.
//!
//! Зелен на ревизии набора (14 тестов, у каждого блокировка первой строкой — замер этим же
//! сторожем); мутация предмета «переставить строку» — красен.

const SUBJECT: &str = include_str!("red_m87_entrypoint.rs");

fn attr_marker() -> String {
    ["#[tokio", "::test"].concat()
}
fn lock_line() -> String {
    ["let _serial = ", "SERIAL", ".lock()", ".await;"].concat()
}
fn decl_marker() -> String {
    ["static ", "SERIAL", ":"].concat()
}

fn is_comment_or_blank(l: &str) -> bool {
    let t = l.trim_start();
    t.is_empty() || t.starts_with("//")
}

/// Разбор предмета: для каждого `#[tokio::test…]` — имя функции и ПЕРВАЯ исполняемая строка
/// тела (после строки с открывающей `{` сигнатуры). Возвращает `(имя, первая_строка)`;
/// незавершённая сигнатура/тело ⇒ имя с пометкой «тело не найдено».
fn async_tests(src: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = src.lines().collect();
    let attr = attr_marker();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].trim_start().starts_with(&attr) {
            i += 1;
            continue;
        }
        // Следующая строка `async fn NAME(`.
        let mut j = i + 1;
        while j < lines.len() && is_comment_or_blank(lines[j]) {
            j += 1;
        }
        let Some(rest) = lines
            .get(j)
            .and_then(|l| l.trim_start().strip_prefix("async fn "))
        else {
            out.push((
                format!("<строка {}>", i + 1),
                "<async fn не найден>".to_string(),
            ));
            i += 1;
            continue;
        };
        let name = rest.split('(').next().unwrap_or("").trim().to_string();
        // Строка, где открывается тело: первая строка от сигнатуры, оканчивающаяся на `{`.
        let mut k = j;
        while k < lines.len() && !lines[k].trim_end().ends_with('{') {
            k += 1;
        }
        // Первая исполняемая строка тела.
        let mut b = k + 1;
        while b < lines.len() && is_comment_or_blank(lines[b]) {
            b += 1;
        }
        let first = lines
            .get(b)
            .map(|l| l.trim().to_string())
            .unwrap_or_else(|| "<тело не найдено>".to_string());
        out.push((name, first));
        i = b.max(i + 1);
    }
    out
}

/// Вердикт сторожа над источником: `Ok(число тестов)` либо `Err(нарушители)`.
fn check(src: &str) -> Result<usize, Vec<String>> {
    let lock = lock_line();
    let declared = src
        .lines()
        .filter(|l| !is_comment_or_blank(l))
        .any(|l| l.contains(&decl_marker()));
    let lock_lines = src
        .lines()
        .filter(|l| !is_comment_or_blank(l))
        .filter(|l| l.trim() == lock)
        .count();
    let tests = async_tests(src);
    if !declared {
        return if lock_lines == 0 {
            Ok(tests.len())
        } else {
            Err(vec![format!(
                "SERIAL не объявлен, а блокировок {lock_lines}"
            )])
        };
    }
    let mut bad: Vec<String> = tests
        .iter()
        .filter(|(_, first)| first != &lock)
        .map(|(name, first)| format!("{name}: первая строка тела — `{first}`"))
        .collect();
    if lock_lines != tests.len() {
        bad.push(format!(
            "блокировок {lock_lines} при {} async-тестах — лишняя/недостающая вне тел тестов",
            tests.len()
        ));
    }
    if bad.is_empty() {
        Ok(tests.len())
    } else {
        Err(bad)
    }
}

#[test]
fn every_async_test_of_the_subject_takes_serial_first_while_serial_exists() {
    let tests = async_tests(SUBJECT);
    assert!(
        !tests.is_empty(),
        "SETUP-СТРАЖ: в предмете нет ни одного `#[tokio::test]` — читается не тот файл"
    );
    match check(SUBJECT) {
        Ok(n) => assert!(
            n >= 14,
            "SETUP-СТРАЖ: найдено {n} async-тестов, ожидалось ≥ 14"
        ),
        Err(bad) => panic!(
            "TD-225: в `red_m87_entrypoint.rs` есть async-тест, не берущий SERIAL ПЕРВОЙ строкой \
             тела — класс TD-224 вернётся МОЛЧА (ложный зелёный у `c6`/`u1` на заимствованном \
             приросте не виден ни одному прогону). Нарушители:\n  {}",
            bad.join("\n  ")
        ),
    }
}

// ─────────────────────────── проба-мутанты сторожа ───────────────────────────

fn src(decl: bool, bodies: &[&str]) -> String {
    let mut s = String::new();
    if decl {
        s.push_str(&decl_marker());
        s.push_str(" tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());\n");
    }
    for (i, body) in bodies.iter().enumerate() {
        s.push_str(&attr_marker());
        s.push_str("]\nasync fn t");
        s.push_str(&i.to_string());
        s.push_str("() {\n");
        s.push_str(body);
        s.push_str("\n}\n");
    }
    s
}

#[test]
fn m1_positive_control_lock_first_in_every_test_passes() {
    let l = lock_line();
    let s = src(
        true,
        &[&format!("    {l}\n    let x = 1;"), &format!("    {l}")],
    );
    assert_eq!(check(&s), Ok(2));
}

#[test]
fn m2_lock_not_first_is_named() {
    let l = lock_line();
    let s = src(
        true,
        &[
            &format!("    let before = counter();\n    {l}"),
            &format!("    {l}"),
        ],
    );
    let bad = check(&s).expect_err("переставленная строка обязана быть поймана");
    assert!(
        bad.iter().any(|b| b.starts_with("t0:")),
        "нарушитель не назван: {bad:?}"
    );
}

#[test]
fn m3_missing_in_one_and_doubled_in_another_is_caught_despite_equal_counts() {
    let l = lock_line();
    let s = src(
        true,
        &[&format!("    {l}\n    {l}"), "    let before = counter();"],
    );
    let bad = check(&s).expect_err("счёт сходится (2 == 2), а тест t1 без блокировки");
    assert!(
        bad.iter().any(|b| b.starts_with("t1:")),
        "t1 не назван: {bad:?}"
    );
}

#[test]
fn m4_comment_before_lock_is_allowed() {
    let l = lock_line();
    let s = src(true, &[&format!("    // почему сериализуемся\n\n    {l}")]);
    assert_eq!(check(&s), Ok(1));
}

#[test]
fn m5_no_declaration_requires_no_locks() {
    let l = lock_line();
    assert_eq!(check(&src(false, &["    let x = 1;"])), Ok(1));
    assert!(check(&src(false, &[&format!("    {l}")])).is_err());
}

#[test]
fn m6_multiline_signature_is_parsed() {
    let l = lock_line();
    let mut s = String::new();
    s.push_str(&decl_marker());
    s.push_str(" tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());\n");
    s.push_str(&attr_marker());
    s.push_str("(flavor = \"multi_thread\", worker_threads = 4)]\nasync fn long(\n) {\n    ");
    s.push_str(&l);
    s.push_str("\n}\n");
    s.push_str(&attr_marker());
    s.push_str("]\nasync fn short() {\n    let y = 2;\n    ");
    s.push_str(&l);
    s.push_str("\n}\n");
    let bad = check(&s).expect_err("short берёт блокировку второй строкой");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].starts_with("short:"), "{bad:?}");
}
