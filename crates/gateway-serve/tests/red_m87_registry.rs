//! M-87 — **ДОКАЗАТЕЛЬСТВО ПОЛНОТЫ реестра, предъявляемое ПРОГОНОМ** (`A-039` §Q2).
//!
//! Три круга гейта подряд поймали утверждения о наборе, не предъявленные набором. Этот
//! бинарь существует затем, чтобы такого утверждения больше не было: всё, что раньше
//! говорилось прозой («все потребители перечислены», «числа согласованы», «валидная
//! мутация существует»), здесь ПЕЧАТАЕТСЯ прогоном.
//!
//! **Почему ОТДЕЛЬНЫЙ бинарь, а не тест внутри файла предмета.** `red_m87_entrypoint.rs`
//! COMPILE-RED до задач 12-13 — он опирается на форму политики, которой ещё нет. Реестр-
//! тест внутри него не исполнился бы до реализации, и доказательство полноты снова стало
//! бы ОБЕЩАНИЕМ «после задачи 12» — ровно тем классом, на котором милестоун потерял три
//! круга. Этот бинарь НЕ импортирует `gateway_serve::admission`, собирается и зеленеет НА
//! ТЕКУЩЕЙ ветке. Доказательство, которое нельзя прогнать сегодня, этот класс не закрывает.

mod m87_registry;

use m87_registry::{Request, EXPECTED_WARMUP_EVENTS, MAX_TAIL_EVENTS, REGISTRY};

/// Имена тестовых функций файла предмета, вынутые из ЕГО ИСХОДНИКА.
///
/// `include_str!` с ЛИТЕРАЛОМ имени соседа, а не `file!()`: путь из `file!()` относителен
/// корню workspace, а `include_str!` — каталогу этого файла, и такой вызов не собрался бы
/// или собрал бы не то. Прецедент — `crates/venue-hyperliquid/tests/red_provenance_md_only.rs`.
const SUBJECT_SRC: &str = include_str!("red_m87_entrypoint.rs");

/// Исход классификации атрибута. Исходов ДВА: распознан или отказ — третьего нет.
///
/// `A-040` §Q3(б): прежний `SuspiciousTest` был ЧЁРНЫМ списком («похоже на тест, но не
/// знаю такого»), а чёрный список не сходится ни сегодня, ни завтра — круг 6 добавил
/// `cfg_attr`, круг 7 нашёл `quickcheck`, и следующая форма нашлась бы так же. Список
/// ИНВЕРТИРОВАН: разрешено ровно то, что стоит в файле сегодня, всё прочее — FAIL.
#[derive(PartialEq, Eq, Debug)]
enum Attr {
    /// Распознанная форма объявления теста.
    Test,
    /// Разрешённый НЕ-тестовый атрибут (сегодня это только `cfg`).
    AllowedNonTest,
    /// Не атрибут вовсе.
    NotAttr,
    /// Голова вне белого списка — ОТКАЗ с именем строки, а не догадка.
    Refused,
}

/// Белый список ГОЛОВ атрибутов. Ровно те три, что есть в предмете на этой ревизии
/// (замер `A-040`: 5 `cfg`, 1 `test`, 14 `tokio::test`). Расширение — единственная
/// разрешённая будущая правка, и только вместе с пробой.
const ALLOWED_ATTR_HEADS: &[&str] = &["test", "tokio::test", "cfg"];

fn classify_attr(line: &str) -> Attr {
    let t = line.trim_start();
    let Some(inner) = t.strip_prefix("#[") else {
        return Attr::NotAttr;
    };
    let head: String = inner
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
        .collect();
    match head.as_str() {
        "test" | "tokio::test" => Attr::Test,
        "cfg" => Attr::AllowedNonTest,
        _ => Attr::Refused,
    }
}

/// Строки предмета, нарушающие СТИЛЕВОЙ КОНТРАКТ, внутри которого построчная выемка вообще
/// имеет смысл. Вне контракта страж ОТКАЗЫВАЕТ, а не догадывается (`A-040` §4 п. 1).
fn contract_violations(src: &str) -> Vec<String> {
    let mut bad = Vec::new();
    for (n, line) in src.lines().enumerate() {
        let t = line.trim_start();
        let no = n + 1;
        if classify_attr(line) == Attr::Refused {
            bad.push(format!("{no}: голова атрибута вне белого списка — {t}"));
        }
        // Атрибут и объявление на ОДНОЙ строке: построчная выемка их не связывает.
        if t.starts_with("#[") && (t.contains("] fn ") || t.contains("] async fn ")) {
            bad.push(format!("{no}: атрибут и fn на одной строке — {t}"));
        }
        // Макрос, порождающий тесты: то, чего текст не видит, текст ЗАПРЕЩАЕТ.
        if t.starts_with("macro_rules!") {
            bad.push(format!(
                "{no}: macro_rules! в предмете — порождённые тесты невидимы выемке"
            ));
        }
        // Блочный комментарий: `#[test]` внутри него — ложная находка, которую белый
        // список не различит.
        if t.contains("/*") {
            bad.push(format!(
                "{no}: блочный комментарий — выемка не различает код и комментарий"
            ));
        }
    }
    bad
}

/// Вынуть имена функций, идущих за распознанным тестовым атрибутом.
fn scenarios_in(src: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut armed = false;
    for line in src.lines() {
        match classify_attr(line) {
            Attr::Test => {
                armed = true;
                continue;
            }
            // Разрешённый не-тестовый атрибут между тестовым и сигнатурой (`#[cfg(...)]`).
            Attr::AllowedNonTest => continue,
            // Отказ разбирается стражем контракта; здесь такую строку просто не читаем.
            Attr::Refused => continue,
            Attr::NotAttr => {}
        }
        if !armed {
            continue;
        }
        let t = line.trim_start();
        if let Some(rest) = t
            .strip_prefix("async fn ")
            .or_else(|| t.strip_prefix("fn "))
        {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                names.push(name);
            }
            armed = false;
        }
    }
    names
}

fn scenarios_in_subject() -> Vec<String> {
    scenarios_in(SUBJECT_SRC)
}

/// СТРАЖ СТИЛЕВОГО КОНТРАКТА (`A-040` §Q3 п. 2, §4 п. 1).
///
/// Три захода в один механизм (круг 6 — `cfg_attr`, круг 7 — `quickcheck` и вложенное
/// отрицание) показали не отдельные промахи, а неверный уровень: чёрный список форм не
/// сходится. Список инвертирован — разрешено ровно то, что в файле есть, остальное ОТКАЗ.
///
/// Страж проверяет не полноту (её истина — перечень харнесса, шаг `A-040` в приёмке), а
/// то, что файл остаётся в форме, где построчная выемка вообще что-то значит.
#[test]
fn r0b_subject_stays_inside_the_parsing_contract() {
    let bad = contract_violations(SUBJECT_SRC);
    assert!(
        bad.is_empty(),
        "предмет вышел за стилевой контракт разбора:\n  {}\nВне контракта построчная \
         выемка не значит ничего, и её зелёный цвет — видимость. Расширять белый список \
         можно ТОЛЬКО вместе с пробой (A-040 §4 п. 5)",
        bad.join("\n  ")
    );

    // Каждый распознанный тестовый атрибут обязан быть ПОТРЕБЛЁН объявлением `fn` — это и
    // есть setup-страж «найдено ровно столько, сколько атрибутов» (`A-039` §Q2 п. 3),
    // которого в биекции не было: она сверяла реестр с найденным, а не найденное с числом
    // атрибутов.
    let attrs = SUBJECT_SRC
        .lines()
        .filter(|l| classify_attr(l) == Attr::Test)
        .count();
    let found = scenarios_in_subject();
    assert!(
        attrs > 0,
        "setup-страж: тестовых атрибутов не найдено вовсе"
    );
    assert_eq!(
        found.len(),
        attrs,
        "тестовых атрибутов {attrs}, а имён вынуто {} — часть атрибутов не потреблена \
         объявлением, и выемка считает не всё множество. Найдено: {found:?}",
        found.len()
    );

    // АНТИ-ПЛАЦЕБО: страж обязан отвергать ровно те формы, на которых ломались круги 6 и 7.
    for form in [
        "#[cfg_attr(test, tokio::test)]\nasync fn sneaky() {}\n",
        "#[quickcheck]\nfn prop_something(x: u8) -> bool { true }\n",
        "#[rstest]\nfn parametrised() {}\n",
    ] {
        assert!(
            !contract_violations(form).is_empty(),
            "страж ПРОПУСТИЛ форму вне белого списка: {form:?} — чёрный список вернулся"
        );
    }
    // И НЕ отвергать законное: ложное срабатывание — такая же находка.
    assert!(
        contract_violations("#[cfg(feature = \"testing\")]\n#[tokio::test]\nasync fn ok() {}\n")
            .is_empty(),
        "страж отверг законную форму — он краснел бы на исправном файле"
    );
}

/// БИЕКЦИЯ: каждый сценарий файла назван в реестре и каждая строка реестра существует.
///
/// Обе стороны обязательны: без первой новый сценарий проезжает мимо доказательства (так
/// прошли три строки в круге 5), без второй переименование оставляет мёртвую строку,
/// которая молча «покрывает» несуществующее.
#[test]
fn r1_registry_is_bijective_with_subject_file() {
    let found = scenarios_in_subject();
    let registered: Vec<&str> = REGISTRY.iter().map(|(n, _)| *n).collect();

    let missing: Vec<&String> = found
        .iter()
        .filter(|n| !registered.contains(&n.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "сценарии файла НЕ НАЗВАНЫ в реестре: {missing:?}. Пока сценарий не в реестре, его \
         отставание и стадия остановки — утверждение автора, а не факт; именно так три \
         строки семейства «слепка нет / повреждён / чужая версия» прошли мимо двух кругов"
    );

    let dead: Vec<&&str> = registered
        .iter()
        .filter(|n| !found.iter().any(|f| f == *n))
        .collect();
    assert!(
        dead.is_empty(),
        "строки реестра не соответствуют ни одному сценарию файла: {dead:?}. Переименование \
         обязано ломать этот тест ГРОМКО — молчаливое расхождение и было болезнью"
    );

    assert_eq!(
        registered.len(),
        found.len(),
        "число строк реестра ({}) не равно числу сценариев ({}) — дубль в реестре",
        registered.len(),
        found.len()
    );
}

/// АРИФМЕТИКА КОРИДОРА: порог согласован со ВСЕМИ обслуживаемыми и отвергаемыми запросами.
#[test]
fn r2_threshold_is_consistent_with_every_registered_request() {
    let mut served_tails: Vec<u64> = Vec::new();
    let mut refused_tails: Vec<u64> = Vec::new();

    for (name, requests) in REGISTRY {
        for r in *requests {
            if let Request::Freshness { tail, served } = r {
                if *served {
                    assert!(
                        *tail < MAX_TAIL_EVENTS,
                        "«{name}» обслуживается при отставании {tail}, но порог \
                         {MAX_TAIL_EVENTS} его НЕ ПЕРЕЖИВАЕТ: верная реализация ответит \
                         отказом, и сценарий не сможет позеленеть никогда"
                    );
                    served_tails.push(*tail);
                } else {
                    assert!(
                        *tail > MAX_TAIL_EVENTS,
                        "«{name}» ждёт отказа ПО СВЕЖЕСТИ при отставании {tail}, но порог \
                         {MAX_TAIL_EVENTS} его переживает: верная реализация обслужит запрос, \
                         и сценарий не сможет позеленеть никогда"
                    );
                    refused_tails.push(*tail);
                }
            }
        }
    }

    // Коридор обязан быть ограничен С ОБЕИХ сторон СОДЕРЖАТЕЛЬНО: иначе порог «доказан»
    // пустотой — при отсутствии отвергаемых строк годится любой большой, при отсутствии
    // ненулевых обслуживаемых любой маленький.
    assert!(
        !refused_tails.is_empty(),
        "в реестре нет ни одного запроса, ОТВЕРГАЕМОГО по свежести — порог не ограничен \
         сверху ничем, и «щедрое» значение прошло бы проверку (класс C-247 R3-1)"
    );
    assert!(
        served_tails.iter().any(|t| *t > 0),
        "в реестре нет ни одного ОБСЛУЖИВАЕМОГО запроса с ненулевым отставанием — \
         утверждение «отставание в пределах порога есть норма» проверять нечем (тавтология)"
    );
}

/// МУТАЦИОННОЕ ОКНО: валидная мутация порога ниже наибольшего обслуживаемого отставания
/// ОБЯЗАНА существовать — иначе мутация делает политику невалидной, и граница проверялась
/// бы отказом СТАРТА вместо свежести (`C-248` R4-1). Число мутации печатает тест.
#[test]
fn r3_valid_mutation_window_exists_and_is_printed() {
    assert!(
        EXPECTED_WARMUP_EVENTS <= MAX_TAIL_EVENTS,
        "сама политика невалидна: опора {EXPECTED_WARMUP_EVENTS} выше порога {MAX_TAIL_EVENTS}"
    );

    let max_served = REGISTRY
        .iter()
        .flat_map(|(_, rs)| rs.iter())
        .filter_map(|r| match r {
            Request::Freshness { tail, served: true } => Some(*tail),
            _ => None,
        })
        .max()
        .expect("в реестре нет обслуживаемых запросов");

    assert!(
        EXPECTED_WARMUP_EVENTS < max_served,
        "валидной мутации НЕ СУЩЕСТВУЕТ: чтобы пересечь наибольшее обслуживаемое отставание \
         {max_served}, порог пришлось бы опустить ниже опоры {EXPECTED_WARMUP_EVENTS}, а это \
         делает политику невалидной — сценарий упал бы на отказе СТАРТА, доказывая не то \
         свойство (C-248 R4-1)"
    );

    // Середина окна, а не край: значение на краю зависит от правки фикстуры.
    let mutated = EXPECTED_WARMUP_EVENTS + (max_served - EXPECTED_WARMUP_EVENTS) / 2;
    println!(
        "МУТАЦИЯ ГРАНИЦЫ (для tester'а после задачи 12): MAX_TAIL_EVENTS {MAX_TAIL_EVENTS} → \
         {mutated}; политика остаётся валидной ({mutated} ≥ {EXPECTED_WARMUP_EVENTS}), \
         наибольшее обслуживаемое отставание {max_served} оказывается СВЕРХ порога ⇒ \
         соответствующий сценарий обязан покраснеть ПО СВЕЖЕСТИ, а не по отказу старта"
    );
    assert!(mutated >= EXPECTED_WARMUP_EVENTS && mutated < max_served);
}

/// СВИДЕТЕЛИ СУЩЕСТВУЮТ: стадия остановки — утверждение с предъявленным оракулом, а не
/// вывод автора (`A-039` §Q3 условие 3). Истинность свидетеля — дело самого свидетеля;
/// здесь проверяется, что он НЕ ВЫМЫШЛЕН.
#[test]
fn r4_every_short_circuit_cites_an_existing_witness() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let corpus: String = std::fs::read_dir(&dir)
        .expect("каталог тестов читается")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        corpus.len() > 1_000,
        "setup-страж: корпус тестов прочитан как {} байт — путь не тот, и проверка ниже \
         зеленела бы на пустоте",
        corpus.len()
    );

    for (name, requests) in REGISTRY {
        for r in *requests {
            if let Request::ShortCircuit { stage, witness } = r {
                assert!(
                    corpus.contains(&format!("fn {witness}")),
                    "«{name}» объявляет стадию {stage:?} со свидетелем «{witness}», которого в \
                     корпусе НЕТ. Стадия без существующего свидетеля — это «0» под другим \
                     именем: утверждение автора о том, где остановится код"
                );
            }
        }
    }
}

/// ДРАЙВЕР ДЕЙСТВИТЕЛЬНО ПИТАЕТ ФИКСТУРУ, А НЕ ЛЕЖИТ РЯДОМ (`C-251`).
///
/// Круг 6 поймал недоисполнение `A-039`: хелпер состояния слепка существовал, но его никто
/// не звал — состояние по-прежнему выбирало тело сценария, и переклассификация строки
/// «слепка нет» в «отставание 0» оставалась зелёной. Полдела здесь хуже, чем ничего:
/// реестр ВЫГЛЯДЕЛ единственным источником, не будучи им.
///
/// Проверяется по ФОРМЕ вызова в исходнике предмета, а не по слову в комментарии. Сегодня
/// это единственный способ предъявить связь: файл предмета COMPILE-RED до задач 12-13, и
/// прогнать сами сценарии нельзя. Предел назван — проверка статическая; после задачи 12
/// связь становится наблюдаемой прогоном, и переклассификация строки роняет сценарий.
#[test]
fn r5_checkpoint_state_is_driven_by_registry_not_by_fixture_bodies() {
    assert!(
        SUBJECT_SRC.contains("m87_registry::registry_checkpoint("),
        "файл предмета НЕ ЗОВЁТ registry_checkpoint — состояние слепка снова выбирает тело \
         сценария, и строку реестра можно переклассифицировать безнаказанно (C-251)"
    );

    // Построение слепка живёт ТОЛЬКО в драйвере. Исключение ровно одно и оно названо:
    // `u1_guard_trap_actually_traps` проверяет, что построение слепка НАД ЛОВУШКОЙ
    // отказывает, — это его предмет, а не оснастка.
    let advances = SUBJECT_SRC.matches("checkpoint::advance").count();
    assert_eq!(
        advances, 3,
        "построений слепка в предмете: {advances}, ожидалось 3 (две ветви драйвера + \
         страж ловушки). Лишнее построение означает второе место, где состояние слепка \
         расходится с реестром"
    );

    // Путь файла слепка тоже собирается в одном месте: иначе сценарий может испортить
    // слепок «мимо» классификации.
    let fingerprints = SUBJECT_SRC.matches("ckpt-{:016x}.bin").count();
    assert_eq!(
        fingerprints, 1,
        "адрес файла слепка собирается в {fingerprints} местах вместо одного — порча или \
         подделка версии может пройти мимо строки реестра"
    );
}

/// Тело сценария по его имени — для сверки ОЖИДАНИЯ с классом строки.
fn body_of(scenario: &str) -> String {
    let start = SUBJECT_SRC
        .find(&format!("async fn {scenario}("))
        .or_else(|| SUBJECT_SRC.find(&format!("fn {scenario}(")))
        .unwrap_or_else(|| panic!("сценарий «{scenario}» не найден в исходнике предмета"));
    let rest = &SUBJECT_SRC[start..];
    let end = rest.find("\n}\n").unwrap_or(rest.len());
    rest[..end].to_string()
}

/// Ждёт ли тело сценария СНИМКА как исхода.
///
/// Наивное «в теле встречается слово snapshot» не годится и это поймано прогоном:
/// `c5_entry_refusal_does_not_substitute_parameters` содержит `Some("snapshot")` ВНУТРИ
/// `assert_ne!` — то есть утверждает ОБРАТНОЕ. Поэтому отрицательные утверждения из тела
/// вырезаются перед проверкой.
///
/// **Предел назван:** это разбор текста, а не типов. Он различает две формы, которыми
/// корпус пользуется сегодня (`assert_eq!` и `assert_ne!`), и не претендует на большее;
/// его собственная годность предъявлена стражем ниже, а не объявлена.
fn expects_snapshot(body: &str) -> bool {
    let mut cleaned = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(i) = rest.find("assert_ne!") {
        cleaned.push_str(&rest[..i]);
        let tail = &rest[i..];

        // ОДНОСТРОЧНАЯ ФОРМА: `assert_ne!(a, b);` — конец на той же строке.
        let first_line_end = tail.find('\n').unwrap_or(tail.len());
        let first_line = &tail[..first_line_end];
        if first_line.trim_end().ends_with(");") {
            rest = &tail[first_line_end..];
            continue;
        }

        // МНОГОСТРОЧНАЯ: конец — `);` на ОТСТУПЕ ОТКРЫВАЮЩЕЙ строки, а не на четырёх
        // пробелах (`A-040` P4: вложенное отрицание имеет больший отступ, и поиск
        // фиксированного отступа обрывал вырезание не там).
        let indent: String = rest[..i].chars().rev().take_while(|c| *c == ' ').collect();
        let closer = format!("\n{indent});");
        match tail.find(&closer) {
            Some(k) => rest = &tail[k + closer.len()..],
            None => panic!(
                "разборщик ожиданий: у `assert_ne!` не найден конец на отступе {} — форма \
                 вне объявленной грамматики. Молча отбросить остаток нельзя: именно так \
                 текстовая канарейка превращается в видимость (A-040 §Q3 п. 4)",
                indent.len()
            ),
        }
    }
    cleaned.push_str(rest);
    cleaned.contains("\"snapshot\"")
}

/// КЛАСС СТРОКИ СВЕРЯЕТСЯ С ОЖИДАНИЕМ СЦЕНАРИЯ — обход `C-251`, закрытый НАБЛЮДЕНИЕМ.
///
/// Критик показал обход: строку «слепка нет» можно переписать в «отставание 0,
/// обслуживается», и всё остаётся зелёным. Драйвер делает такую переклассификацию
/// ОШИБКОЙ ПО ПОСТРОЕНИЮ — фикстура построит прогретый слепок там, где сценарий ждёт
/// отказа, — но увидеть это прогоном можно лишь после задачи 12: файл предмета
/// COMPILE-RED. Ждать до тех пор значит оставить обход открытым на всё время кругов.
///
/// Поэтому здесь — сверка по исходнику, доступная СЕГОДНЯ: сценарий, у которого ВСЕ
/// запросы отказные, не имеет права ждать снимка; сценарий с обслуживаемым запросом
/// обязан снимка ждать. Переклассификация `c1_entry_cold…` из отказа в обслуживание
/// краснеет здесь и сейчас.
#[test]
fn r6_request_class_matches_scenario_expectation() {
    for (name, requests) in REGISTRY {
        if requests.is_empty() {
            continue; // строка-заявление: сценарий сервера не поднимает
        }
        let body = body_of(name);
        let expects_snapshot = expects_snapshot(&body);
        let any_served = requests
            .iter()
            .any(|r| matches!(r, Request::Freshness { served: true, .. }));

        if any_served {
            assert!(
                expects_snapshot,
                "«{name}» объявлен в реестре как ОБСЛУЖИВАЕМЫЙ, но его тело не ждёт снимка. \
                 Либо строка переклассифицирована ошибочно (класс обхода C-251), либо \
                 сценарий изменил предмет и реестр отстал"
            );
        } else {
            assert!(
                !expects_snapshot,
                "«{name}» объявлен в реестре как ПОЛНОСТЬЮ отказной, но его тело ждёт снимка. \
                 Реестр и сценарий разошлись, и один из них лжёт"
            );
        }
    }
}

/// СТРАЖ РАЗБОРЩИКА ОЖИДАНИЙ: он обязан РАЗЛИЧАТЬ утверждение и его отрицание.
///
/// Без этого стража `r6` зеленел бы на разборщике, который просто ищет слово, — и тогда
/// сверка класса с ожиданием была бы тавтологией.
#[test]
fn r6b_expectation_parser_distinguishes_assertion_from_its_negation() {
    // (1) многострочное положительное
    let positive =
        "    assert_eq!(\n        msg.get(\"type\"),\n        Some(\"snapshot\"),\n    );\n";
    // (2) многострочное отрицание
    let negative =
        "    assert_ne!(\n        msg.get(\"type\"),\n        Some(\"snapshot\"),\n    );\n";
    // (3) ВЛОЖЕННОЕ отрицание — `A-040` P4: отступ больше четырёх пробелов
    let nested = "        assert_ne!(\n            msg.get(\"type\"),\n            Some(\"snapshot\"),\n        );\n";
    // (4) ОДНОСТРОЧНОЕ отрицание — `A-040` P5
    let one_line = "    assert_ne!(t, Some(\"snapshot\"));\n";

    assert!(
        expects_snapshot(positive),
        "не увидено ПОЛОЖИТЕЛЬНОЕ ожидание снимка"
    );
    assert!(!expects_snapshot(negative), "отрицание принято за ожидание");
    assert!(
        !expects_snapshot(nested),
        "ВЛОЖЕННОЕ отрицание принято за ожидание — поиск конца по фиксированному отступу \
         вернулся (A-040 P4)"
    );
    assert!(
        !expects_snapshot(one_line),
        "ОДНОСТРОЧНОЕ отрицание принято за ожидание (A-040 P5)"
    );
    // Положительное ПОСЛЕ каждого отрицания обязано остаться видимым: вырезание не имеет
    // права съедать хвост.
    assert!(
        expects_snapshot(&format!("{nested}{positive}")),
        "после вложенного отрицания положительное ожидание потеряно — вырезание съело хвост"
    );
    assert!(
        expects_snapshot(&format!("{one_line}{positive}")),
        "после однострочного отрицания положительное ожидание потеряно"
    );
}

/// СВЯЗЬ «СТРОКА ↔ ФУНКЦИЯ» — МЕХАНИЗМ, А НЕ ДИСЦИПЛИНА (`A-040` §Q3 п. 3, P6).
///
/// Довод «драйвер паникует на незарегистрированном имени» верен для СТРОКИ и неверен для
/// ФУНКЦИИ: сценарий `foo` может передать драйверу литерал `"bar"`, и паники не будет —
/// строка `bar` существует. Тогда фикстуру строит ЧУЖАЯ строка, а реестр выглядит
/// согласованным. Проверяется равенством литерала имени сценария — точная операция под
/// стилевым контрактом стража выше.
#[test]
fn r7_driver_literal_equals_scenario_name() {
    let drivers = ["serve_for(", "ckpt_from_registry(", "registry_tail("];
    let mut checked = 0usize;
    for (name, _) in REGISTRY {
        let body = body_of(name);
        for d in drivers {
            let mut from = 0usize;
            while let Some(i) = body[from..].find(d) {
                let at = from + i + d.len();
                let tail = &body[at..];
                // Первый строковый литерал вызова.
                let Some(q1) = tail.find('"') else { break };
                let Some(q2) = tail[q1 + 1..].find('"') else {
                    break;
                };
                let lit = &tail[q1 + 1..q1 + 1 + q2];
                assert_eq!(
                    lit, *name,
                    "«{name}» передаёт драйверу {d} литерал «{lit}» — фикстуру строит ЧУЖАЯ \
                     строка реестра. Паника драйвера этого НЕ ловит: строка «{lit}» \
                     существует, и реестр выглядит согласованным (A-040 P6)"
                );
                checked += 1;
                from = at;
            }
        }
    }
    assert!(
        checked >= 3,
        "setup-страж: проверено {checked} вызовов драйвера — меньше трёх известных, \
         значит поиск смотрит не туда и страж зеленел бы на пустоте"
    );
}
