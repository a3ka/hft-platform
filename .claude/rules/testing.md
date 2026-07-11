# Testing Rules — RED-first TDD

Источник: `docs/04-workflow.md` §2/§3 ("RED-first"), `docs/DESIGN.md` §4 (`RK-I-1..10`),
§1 (`DET-I-1`), `docs/03-integration-contract.md` §6 (`INTG-I-*`). Обязательный порядок:
**Research → Plan → TEST (RED) → Implement (GREEN) → Refactor → Review.** Шаг 3 не
пропускается никогда.

## Тест — это спецификация, не проверка постфактум

Тест пишется architect'ом ДО кода и определяет, ЧТО код обязан делать. Vague-задача
("сделай risk-gate") даёт код, который "выглядит правильно" и может тонко ошибаться.
Конкретный тест не оставляет места для галлюцинации:

```rust
#[test]
fn rk_i_1_no_order_reaches_venue_without_risk_approved() {
    // Типовой барьер: venue-адаптер принимает ТОЛЬКО RiskApproved<Order>,
    // не Order. Компилятор — часть теста.
    let raw = Order::new(/* ... */);
    // let _ = venue.submit(raw); // <- НЕ ДОЛЖНО КОМПИЛИРОВАТЬСЯ
    let approved = risk_gate.check(raw).expect("должно быть Approved");
    assert!(venue.submit(approved).is_ok());
}
```

## Анти-плацебо — тест GREEN против заглушки = дефект

Урок hft-core-rs (`docs/DESIGN.md` §9, risk_guard fail-open инцидент): тест обязан
ПАДАТЬ, если реализация — no-op/заглушка/`Ok(())` без реальной проверки. Перед тем как
считать RED-тест валидным, architect мысленно (или явно) прогоняет его против
заглушечной реализации и убеждается, что он FAIL. Пример неправильного теста:
`assert!(risk_gate.check(order).is_ok())` — проходит даже если `check()` всегда
возвращает `Ok`. Правильно: assert конкретного отказа на конкретном invalid-входе
(`RK-I-3` — неизвестный инструмент → `Reject`, не default-лимиты).

## Sacred RED-оракулы

- **`RK-I-1..10`** (`docs/DESIGN.md` §4) — риск-инварианты. Живут в `crates/risk/tests/`
  и `crates/killswitch/tests/`. Только architect пишет/меняет.
- **`DET-I-1`** (бит-идентичный replay журнала, `docs/DESIGN.md` §1) — детерминизм-тест
  журнала; P0 acceptance-ворота (`docs/DESIGN.md` §10).
- **`INTG-I-1..7`** (`docs/03-integration-contract.md` §6) — границы A/B/C; тест
  подтверждает ОТСУТСТВИЕ альтернативного write-пути в рантайм (не наличие проверки —
  отсутствие обхода).
- **`CT-I-1..6`** (`docs/05-contract-layer.md` §6) — контрактный слой; grep-канарейка
  `EventKind` определён ровно в одном крейте, roundtrip-фикстуры старых версий журнала.

Эти тесты dev НЕ трогает даже при рефакторинге вокруг них. Показался неправильным →
`!!! SCOPE VIOLATION REQUEST !!!` (`.claude/rules/scope-guard.md`).

## Каждая публичная функция — с тестом

- **Домен без I/O** (`crates/journal`, `crates/book`, `crates/signals`, `crates/alpha`,
  `crates/portfolio`) — чистые функции, детерминированные, без wall-clock/`rand()`/
  неупорядоченной итерации по `HashMap` в редьюсерах (журнал-принцип, `docs/DESIGN.md` §1).
  Сигнал не имеет доступа к будущему — только к потоку `Event` до текущего момента.
- **Границы I/O** (`venue-*`, `journal` writer, `oms` submit) — интеграционные тесты,
  явно поименованные (`test_venue_hyperliquid_reconnect_no_orders.rs` и т.п.).
- **Детерминизм-тест обязателен для каждого сигнала**: одинаковый вход → одинаковый
  выход, независимо от порядка вызова (`docs/02-quant-desk.md` §3.2).

## Покрытие

- **80%+** для `crates/risk`, `crates/killswitch`, `crates/oms` (пре-trade path),
  контрактных roundtrip-тестов.
- **60%+** для остального (`signals`, `book`, `alpha`, `portfolio`, `research-cli`).

## Команда прогона

```bash
cargo test -p <crate>              # один крейт
cargo test --workspace             # весь workspace (перед PR-time reviewer)
cargo test -p risk -p killswitch   # sacred-путь явно, при любом касании oms/venues
```

Python research-тулинг (`crates/research-cli` обвязка / ноутбуки, если появятся):
`pytest research/ -v` — только консюмер контрактов (`docs/05-contract-layer.md` §5),
не пишет T1 в рантайм.

## Что нарушение выглядит как

- Реализация закоммичена раньше или в одном коммите с первым RED-тестом.
- Тест дергает `RiskApproved` конструктор напрямую вместо приватного конструктора
  крейта `risk` (обходит типовой барьер `RK-I-1`).
- Тест на сигнал не проверяет детерминизм (два прогона с одним `Event`-потоком дают
  разный результат — незамечено).
- `#[ignore]` или `#[should_panic]` без обоснования на sacred-тесте.

## Прод-масштаб для sacred I/O-путей (урок TD-011, 2026-07-11)

RED-оракул sacred-пути с I/O (journal `open`/`read`/`recover`, recorder writer,
venue book-maintenance) ОБЯЗАН включать **прод-масштабный кейс**, не только крошечные
in-memory фикстуры. Инцидент TD-011: `Journal::open` делал `read_to_end` ВСЕГО сегмента
(прод 2.65 GiB) в RAM → recorder переставал писать (OOM-риск); юнит-RED на фикстурах в
десятки байт этого не поймал, CI зелёный по компиляции + «Deploy success» замаскировали —
регрессию поймал ТОЛЬКО eyes-on §8 на VPS (ssh-проверка живого прода).

Требования к таким оракулам:
- **Граница ресурса, не только корректность.** Для `open()`/загрузки — большой сегмент
  (десятки MiB+) + проверка ОГРАНИЧЕННОЙ памяти (счётчик аллокаций через global allocator)
  и/или времени. Оракул обязан ПАДАТЬ на наивной реализации (full-read) — анти-плацебо
  (пример: `crates/journal/tests/red_open_bounded.rs`).
- **Зелёные юнит-тесты + Deploy-success ≠ рабочий прод.** `.claude/rules/gates.md` §8
  eyes-on (контейнер пишет, heartbeat свежий, сегмент растёт, CPU/MEM в норме) —
  обязателен и решающий; healthcheck можно обмануть тихой деградацией.

## Cross-references

- `.claude/rules/gates.md` §2 (RED-first в цикле гейтов), §5 (RISK-BLOCK)
- `.claude/rules/scope-guard.md` (тесты — sacred, dev не правит)
- `docs/DESIGN.md` §4 (RK-I-*), §1 (DET-I-1), §5 (честность симулятора — та же дисциплина
  для fill-model)
- `docs/03-integration-contract.md` §6 (INTG-I-*)
- `docs/05-contract-layer.md` §6 (CT-I-*)
