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

/// Вынуть имена функций, идущих за тестовым атрибутом.
fn scenarios_in_subject() -> Vec<String> {
    let mut names = Vec::new();
    let mut armed = false;
    for line in SUBJECT_SRC.lines() {
        let t = line.trim_start();
        if t.starts_with("#[tokio::test") || t == "#[test]" {
            armed = true;
            continue;
        }
        if !armed {
            continue;
        }
        // Между атрибутом и сигнатурой законны другие атрибуты (`#[cfg(...)]`).
        if t.starts_with("#[") {
            continue;
        }
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

/// SETUP-СТРАЖ: парсер, молча нашедший ноль, есть плацебо самого себя.
#[test]
fn r0_parser_finds_every_test_attribute() {
    let attrs = SUBJECT_SRC
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("#[tokio::test") || t == "#[test]"
        })
        .count();
    let found = scenarios_in_subject();
    assert!(
        attrs > 0,
        "setup-страж: в исходнике предмета НЕ НАЙДЕНО ни одного тестового атрибута — \
         либо включён не тот файл, либо парсер слеп; всё ниже судило бы пустоту"
    );
    assert_eq!(
        found.len(),
        attrs,
        "setup-страж: атрибутов {attrs}, а имён вынуто {} — парсер теряет сценарии, и \
         биекция ниже доказывала бы полноту ПО НЕПОЛНОМУ множеству. Найдено: {found:?}",
        found.len()
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
