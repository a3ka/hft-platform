//! M-87 — **ИСТОЧНИК общего порога и реестр его потребителей** (решение `A-039`).
//!
//! ## Зачем этот модуль существует
//!
//! Оракулы точки входа делят ОДНУ политику допуска. В ней два связанных числа —
//! порог свежести слепка и опора, ниже которой политика невалидна, — и разные сценарии
//! имеют разное отставание слепка от хвоста журнала. Одно значение порога СВЯЗЫВАЕТ
//! сценарии между собой, поэтому его выбор обязан быть доказан по ВСЕМ потребителям.
//!
//! Три круга гейта подряд (`C-247` R3-1 → `C-248` R4-2 → `C-249`) поймали один и тот же
//! класс: утверждение о наборе, не предъявленное набором. «Щедрый порог не мешает» —
//! ложь; «прочие сценарии ≈0» — ложь (один дописывает 4 000 событий); «все потребители в
//! таблице» — ложь (трёх не было). Арбитраж `A-039` §Q4 назвал причину точно: утверждение
//! и факт жили в РАЗНЫХ местах — числа в телах фикстур, порог в хелпере, связь в
//! комментарии, скопированном в милестоун. Три места, ни одно не проверяет другое.
//!
//! ## Что здесь изменено по существу
//!
//! **Реестр ПИТАЕТ фикстуру, а не описывает её** (`A-039` §Q2, обязательная часть).
//! Предложение «список + сверка» закрывало пропуск сценария и арифметику, но НЕ закрывало
//! главного: автор реестра мог написать `tail: 0` сценарию, который дописывает 4 000, и
//! обе проверки остались бы зелёными. Реестр, переписывающий числа фикстур, — та же
//! таблица в другой одежде. Поэтому число живёт ЗДЕСЬ и только здесь, а тело сценария
//! берёт его отсюда: утверждение «c9 отстаёт на 200» не существует отдельно от факта —
//! оно И ЕСТЬ факт.
//!
//! **У short-circuit-запроса нет числового поля ПО ТИПУ.** Сценарий, останавливающийся до
//! сверки свежести (слепка нет / повреждён / чужая версия / отказ допуска / занят слот),
//! отставания не имеет — оно к нему неприменимо. Прежние таблицы писали таким «0», и это
//! не доказательство, а его видимость. Написать «0» здесь НЕВОЗМОЖНО: варианта с числом
//! у `ShortCircuit` не существует (`A-039` §Q3 условие 2).
//!
//! **Стадия остановки названа вариантом закрытого перечисления и несёт СВИДЕТЕЛЯ** —
//! имя оракула, который пиннит предшествование этой стадии сверке свежести (`A-039` §Q3
//! условие 3). «Не задет» — это вывод автора; стадия со свидетелем — проверяемое
//! утверждение. Бинарь реестра проверяет, что свидетель существует в корпусе.
//!
//! ## Чего этот реестр НЕ доказывает — названо, чтобы на него не опирались лишнего
//!
//! Он доказывает ПОЛНОТУ перечисления, ЕДИНСТВЕННОСТЬ чисел и АРИФМЕТИКУ коридора. Он НЕ
//! доказывает, что ожидаемый исход строки ВЕРЕН по существу — что при отставании 200
//! выдача действительно обязана состояться. Это утверждение контракта (§14.1quinquies), и
//! проверяет его реализация плюс мутация границы, остающаяся обязанностью tester'а после
//! задачи 12.

#![allow(dead_code)] // модуль подключается двумя бинарями; каждый берёт свою часть

/// Порог свежести слепка в событиях: отставание курсора слепка от хвоста журнала СВЕРХ
/// этого числа ⇒ отказ по свежести.
///
/// Коридор допустимых значений задаётся не вкусом, а реестром и проверяется прогоном
/// (`red_m87_registry.rs`): строго больше наибольшего обслуживаемого отставания и строго
/// меньше наименьшего отвергаемого. На момент решения `A-039` это `200 < X < 4 000`;
/// выбрана середина, а не край — значение на краю ломалось бы от любой правки фикстуры.
pub const MAX_TAIL_EVENTS: u64 = 1_000;

/// Опора: объём, который прод производит между двумя прогревами слепка. Политика с
/// `MAX_TAIL_EVENTS < EXPECTED_WARMUP_EVENTS` невалидна и валит СТАРТ (§14.1quinquies).
///
/// Значение выбрано так, чтобы существовала ВАЛИДНАЯ мутация порога ниже наибольшего
/// обслуживаемого отставания: иначе мутация делала бы политику невалидной, и сценарий
/// падал бы на отказе старта, доказывая не то свойство (`C-248` R4-1). Существование
/// такой мутации проверяется прогоном, а её число ПЕЧАТАЕТ тест — оно не хранится в прозе.
pub const EXPECTED_WARMUP_EVENTS: u64 = 100;

/// Стадия, на которой запрос останавливается, НЕ дойдя до сверки свежести.
///
/// Порядок стадий взят из §4.0bis спеки (`admit → try_acquire → readiness → работа`) и из
/// тела `readiness` (файла нет → заголовок не читается → версия провода → сверка
/// отставания), а не придуман здесь.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Отказ допуска: полоса/инструмент/профиль вне политики. До чтения состояния не дошли.
    AdmitUnsupported,
    /// Слот вычислительной работы занят: `try_acquire` стоит ДО `readiness`.
    SlotOverloaded,
    /// Файла слепка нет.
    CkptMissing,
    /// Файл есть, заголовок не читается.
    CkptCorrupt,
    /// Заголовок читается, версия провода чужая.
    CkptIncompatible,
}

/// Один ЗАПРОС сценария. Сценарий может делать несколько запросов с разными исходами —
/// агрегировать их одной клеткой значило бы повторить ошибку «клетке приписана не та
/// причина» (`A-039` §Q2 (iii)).
#[derive(Clone, Copy, Debug)]
pub enum Request {
    /// Запрос ДОХОДИТ до сверки свежести. `tail` — отставание слепка от хвоста в событиях;
    /// именно это число дописывает фикстура, взяв его ОТСЮДА.
    Freshness { tail: u64, served: bool },
    /// Запрос останавливается РАНЬШЕ сверки. Числа отставания у него нет по типу.
    /// `witness` — имя оракула, пиннящего предшествование стадии.
    ShortCircuit { stage: Stage, witness: &'static str },
}

use Request::{Freshness, ShortCircuit};
use Stage::{AdmitUnsupported, CkptCorrupt, CkptIncompatible, CkptMissing, SlotOverloaded};

/// Свидетели предшествования стадий. Все существуют в `red_m87_admission.rs` на этой
/// ревизии; бинарь реестра проверяет их наличие грепом по корпусу.
const W_MISSING: &str = "u2_readiness_missing_checkpoint_over_trap";
const W_CORRUPT: &str = "u2_readiness_corrupt_checkpoint_over_trap";
const W_INCOMPAT: &str = "u2_readiness_incompatible_checkpoint";
const W_ADMIT: &str = "form_unknown_symbol_is_unsupported";
/// Порядок «слот берётся ДО сверки готовности» пиннит сам `c4` шагом (2): второй клиент
/// получает `overloaded`, пока первая работа стоит на рандеву. Свидетель здесь совпадает с
/// потребителем, и это НАЗВАННЫЙ предел: строка опирается на тот же оракул, который её
/// использует. Внешнего свидетеля этому порядку в корпусе сегодня нет (`A-039` §Q3).
const W_SLOT: &str = "c4_slot_lifecycle_hold_refuse_release";

/// РЕЕСТР: по строке на каждый тестовый сценарий `red_m87_entrypoint.rs`, по элементу на
/// каждый его запрос. Полнота (биекция с файлом) проверяется прогоном, а не обещанием.
pub const REGISTRY: &[(&str, &[Request])] = &[
    (
        "c1_entry_cold_request_named_outcome_without_reading_journal",
        &[ShortCircuit {
            stage: CkptMissing,
            witness: W_MISSING,
        }],
    ),
    (
        "c1_entry_ready_state_is_actually_served",
        &[Freshness {
            tail: 0,
            served: true,
        }],
    ),
    (
        "c5_entry_unbounded_profile_is_refused",
        &[ShortCircuit {
            stage: AdmitUnsupported,
            witness: W_ADMIT,
        }],
    ),
    (
        "c5_entry_refusal_does_not_substitute_parameters",
        &[ShortCircuit {
            stage: AdmitUnsupported,
            witness: W_ADMIT,
        }],
    ),
    (
        "c5_entry_refusal_keeps_connection_and_neighbours_alive",
        &[
            Freshness {
                tail: 0,
                served: true,
            },
            ShortCircuit {
                stage: AdmitUnsupported,
                witness: W_ADMIT,
            },
        ],
    ),
    (
        "c4_slot_lifecycle_hold_refuse_release",
        &[
            Freshness {
                tail: 0,
                served: true,
            },
            ShortCircuit {
                stage: SlotOverloaded,
                witness: W_SLOT,
            },
            Freshness {
                tail: 0,
                served: true,
            },
        ],
    ),
    (
        "c6_counters_are_emitted_by_the_real_serving_path",
        &[
            ShortCircuit {
                stage: CkptMissing,
                witness: W_MISSING,
            },
            ShortCircuit {
                stage: AdmitUnsupported,
                witness: W_ADMIT,
            },
        ],
    ),
    (
        "c9_four_positions_differ_and_stale_snapshot_does_not_stop_serving",
        &[Freshness {
            tail: 200,
            served: true,
        }],
    ),
    // Ловушка не поднимает сервер и политики не касается: реестр обязан ЗАЯВИТЬ это
    // пустым списком запросов, а не молча пропустить строку (`A-039` §Q2 п. 3).
    ("u1_guard_trap_actually_traps", &[]),
    (
        "u1_entry_missing_checkpoint_over_trap_gives_named_outcome",
        &[ShortCircuit {
            stage: CkptMissing,
            witness: W_MISSING,
        }],
    ),
    (
        "u1_entry_corrupt_checkpoint_over_trap_gives_named_outcome",
        &[ShortCircuit {
            stage: CkptCorrupt,
            witness: W_CORRUPT,
        }],
    ),
    (
        "u1_entry_incompatible_checkpoint_over_trap_gives_named_outcome",
        &[ShortCircuit {
            stage: CkptIncompatible,
            witness: W_INCOMPAT,
        }],
    ),
    (
        "u1_entry_stale_checkpoint_beyond_budget_gives_named_outcome",
        &[Freshness {
            tail: 4_000,
            served: false,
        }],
    ),
    (
        "u1_warm_path_is_served_over_trapped_head",
        &[Freshness {
            tail: 0,
            served: true,
        }],
    ),
    (
        "u1_served_request_with_tail_increments_payload_counter",
        &[Freshness {
            tail: 50,
            served: true,
        }],
    ),
];

/// Отставание, которое фикстура сценария ОБЯЗАНА создать. Единственный источник числа.
///
/// Паникует на незарегистрированном имени: сценарий, которого нет в реестре, не имеет
/// права строить фикстуру вслепую — это и есть та дыра, через которую трижды прошла ложь.
/// Отставание, если строка его объявляет; `None` — сценарий до сверки свежести не доходит.
/// Нужен драйверу: он материализует строку ЦЕЛИКОМ и обязан отличать «отставания нет» от
/// «строка не зарегистрирована» (второе — паника, первое — законный случай).
pub fn registry_tail_opt(scenario: &str) -> Option<u64> {
    let requests = REGISTRY
        .iter()
        .find(|(name, _)| *name == scenario)
        .unwrap_or_else(|| panic!("сценарий «{scenario}» не зарегистрирован в REGISTRY"))
        .1;
    requests.iter().find_map(|r| match r {
        Freshness { tail, .. } => Some(*tail),
        ShortCircuit { .. } => None,
    })
}

pub fn registry_tail(scenario: &str) -> u64 {
    let requests = REGISTRY
        .iter()
        .find(|(name, _)| *name == scenario)
        .unwrap_or_else(|| panic!("сценарий «{scenario}» не зарегистрирован в REGISTRY"))
        .1;
    let tails: Vec<u64> = requests
        .iter()
        .filter_map(|r| match r {
            Freshness { tail, .. } => Some(*tail),
            ShortCircuit { .. } => None,
        })
        .collect();
    match tails.as_slice() {
        [] => panic!(
            "сценарий «{scenario}» зарегистрирован как не доходящий до сверки свежести — \
             дописка ему не положена"
        ),
        [single] => *single,
        many => {
            let first = many[0];
            assert!(
                many.iter().all(|t| *t == first),
                "сценарий «{scenario}» объявляет РАЗНЫЕ отставания {many:?} — одна фикстура \
                 не может создать два; разведи сценарии"
            );
            first
        }
    }
}

/// Состояние слепка, которое обязана создать фикстура сценария. Выводится ИЗ реестра, а не
/// выбирается в теле теста: иначе стадия остановки снова стала бы утверждением автора.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckpointFixture {
    /// Валидный прогретый слепок.
    Warm,
    /// Файла нет.
    Missing,
    /// Файл есть, содержимое — мусор.
    Corrupt,
    /// Валидный слепок с чужой версией провода.
    Incompatible,
}

pub fn registry_checkpoint(scenario: &str) -> CheckpointFixture {
    let requests = REGISTRY
        .iter()
        .find(|(name, _)| *name == scenario)
        .unwrap_or_else(|| panic!("сценарий «{scenario}» не зарегистрирован в REGISTRY"))
        .1;
    // Стадия слепка, если она объявлена хотя бы одним запросом сценария.
    for r in requests {
        if let ShortCircuit { stage, .. } = r {
            match stage {
                CkptMissing => return CheckpointFixture::Missing,
                CkptCorrupt => return CheckpointFixture::Corrupt,
                CkptIncompatible => return CheckpointFixture::Incompatible,
                AdmitUnsupported | SlotOverloaded => continue,
            }
        }
    }
    CheckpointFixture::Warm
}
