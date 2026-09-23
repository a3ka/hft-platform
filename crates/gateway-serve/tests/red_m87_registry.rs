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

/// Распознан ли атрибут как ТЕСТОВЫЙ.
///
/// `C-251`: прежний парсер знал ровно две формы — `#[test]` и `#[tokio::test…]` — и всё
/// остальное молча считал НЕ тестом. Условный атрибут `#[cfg_attr(test, tokio::test)]`
/// законен, объявляет тест и проходил мимо биекции БЕЗ следа: сценарий под ним не попадал
/// ни в найденные, ни в требуемые, и реестр «покрывал» файл, не покрывая его.
///
/// Лечение — не добавить третью форму в список (список снова окажется неполным), а
/// РАЗДЕЛИТЬ три исхода: распознан, НЕ похож на тест, похож на тест но не распознан.
/// Третий исход — отказ, а не молчание (`testing.md`, целостность гейта: гейт обязан
/// наблюдать ОТСУТСТВИЕ, а не только сбой).
#[derive(PartialEq, Eq, Debug)]
enum Attr {
    /// Распознанная тестовая форма.
    Test,
    /// Атрибут, упоминающий `test`, но не распознанный — повод ОТКАЗАТЬ, а не пропустить.
    SuspiciousTest,
    /// К тестам отношения не имеет.
    Other,
}

fn classify_attr(line: &str) -> Attr {
    let t = line.trim_start();
    let Some(inner) = t.strip_prefix("#[") else {
        return Attr::Other;
    };
    // ГОЛОВА атрибута — имя до скобки: именно она объявляет, ЧЕМ является элемент ниже.
    // Разбор по голове, а не по вхождению подстроки: `#[cfg(feature = "testing")]` содержит
    // «test», но НИЧЕГО не объявляет — это предикат сборки. Прежняя редакция этого стража
    // ловила его как подозрительный и краснела на исправном файле (поймано прогоном здесь же).
    let head: String = inner
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
        .collect();
    let body = inner;

    // Распознанные формы объявления теста.
    if head == "test" || head.ends_with("::test") {
        return Attr::Test;
    }
    // `cfg`/`cfg_attr`: первый ничего не объявляет; второй МОЖЕТ объявить тест условно —
    // и именно эту форму C-251 показал как проезжающую мимо.
    if head == "cfg" {
        return Attr::Other;
    }
    if head == "cfg_attr" && body.contains("test") {
        return Attr::SuspiciousTest;
    }
    // Чужие тестовые обёртки: `rstest`, `test_case`, `tokio_test` и прочее, чего мы не знаем.
    if head.ends_with("test") || head.starts_with("test") {
        return Attr::SuspiciousTest;
    }
    Attr::Other
}

/// Атрибуты, похожие на тестовые, но не распознанные. Непустой список — отказ.
fn suspicious_attrs_in(src: &str) -> Vec<String> {
    src.lines()
        .filter(|l| classify_attr(l) == Attr::SuspiciousTest)
        .map(|l| l.trim().to_string())
        .collect()
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
            // Прочие атрибуты между тестовым и сигнатурой законны (`#[cfg(...)]`).
            Attr::Other if line.trim_start().starts_with("#[") => continue,
            Attr::SuspiciousTest => continue,
            Attr::Other => {}
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

/// FAIL-CLOSED НА НЕРАСПОЗНАННУЮ ФОРМУ (`C-251`).
///
/// Парсер, знающий закрытый список форм, слеп к любой шестнадцатой — и молчит об этом.
/// Здесь он обязан ОТКАЗАТЬ: неизвестная тестовая форма в файле предмета означает, что
/// биекция ниже считает не всё множество, а её зелёный цвет ничего не стоит.
#[test]
fn r0b_unrecognised_test_attribute_is_refused_not_ignored() {
    let found = suspicious_attrs_in(SUBJECT_SRC);
    assert!(
        found.is_empty(),
        "в файле предмета есть атрибуты, ПОХОЖИЕ на тестовые, но не распознанные: {found:?}. \
         Пока форма не распознана, сценарий под ней не попадает ни в найденные, ни в \
         требуемые — реестр «покрывает» файл, не покрывая его (C-251)"
    );

    // АНТИ-ПЛАЦЕБО ЗДЕСЬ ЖЕ: страж обязан ловить ровно ту форму, которой он не знал.
    // Без этой проверки «список пуст» означало бы лишь, что смотреть научились не туда.
    let synthetic = "#[cfg_attr(test, tokio::test)]\nasync fn sneaky_scenario() {}\n";
    assert_eq!(
        suspicious_attrs_in(synthetic).len(),
        1,
        "страж НЕ УВИДЕЛ условный тестовый атрибут — ровно тот обход, который нашёл C-251"
    );
    assert!(
        scenarios_in(synthetic).is_empty(),
        "парсер вынул имя из нераспознанной формы — тогда отказ выше не нужен, а поведение \
         двусмысленно: форма либо распознана, либо отвергнута, третьего не дано"
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
        // Конец макро-вызова: строка вида `    );` на своём уровне.
        let tail = &rest[i..];
        match tail.find("\n    );") {
            Some(j) => rest = &tail[j + 7..],
            None => {
                rest = "";
                break;
            }
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
    let positive =
        "    assert_eq!(\n        msg.get(\"type\"),\n        Some(\"snapshot\"),\n    );\n";
    let negative =
        "    assert_ne!(\n        msg.get(\"type\"),\n        Some(\"snapshot\"),\n    );\n";
    assert!(
        expects_snapshot(positive),
        "разборщик не увидел ПОЛОЖИТЕЛЬНОГО ожидания снимка"
    );
    assert!(
        !expects_snapshot(negative),
        "разборщик принял ОТРИЦАНИЕ за ожидание — сверка класса стала бы тавтологией \
         (поймано прогоном на c5_entry_refusal_does_not_substitute_parameters)"
    );
}
