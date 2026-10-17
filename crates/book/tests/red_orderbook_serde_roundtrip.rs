//! RED M-38b (sacred, architect-only) — **`OrderBook` переживает serde-roundtrip ЦЕЛИКОМ,
//! включая chain-состояние (`last_final_update_id`) и fail-closed флаг (`stale`).**
//!
//! ## Зачем это в M-38b
//!
//! Чекпоинт read-gateway (TD-044) сериализует ПОЛНОЕ состояние `Reducer`, в котором лежит
//! `book::OrderBook`. У книги ЧЕТЫРЕ поля, и два из них — ПРИВАТНЫЕ и без геттера-конструктора
//! (`crates/book/src/lib.rs:25-36`):
//!
//! ```text
//! bids / asks               — доступны через levels(side)
//! last_final_update_id      — НЕ доступно публично
//! stale                     — читается через is_stale(), но не восстанавливается
//! ```
//!
//! Соблазнительная реализация чекпоинта — «сохранить `levels()`, восстановить
//! `apply_snapshot()`» — теряет оба приватных поля: `apply_snapshot` обнуляет
//! `last_final_update_id` (следующая дельта пойдёт по ветке **bootstrap** вместо проверки
//! непрерывности — GD-I-5) и сбрасывает `stale` в `false` (**недостоверная книга молча
//! становится достоверной**). Это ровно тот класс тихой лжи, ради которого M-30 сделал
//! gap-детекцию fail-closed.
//!
//! ## Честная граница (проверено по коду, не выведено)
//!
//! Сегодня `Reducer` применяет `book.apply_delta(...)` (`crates/gateway/src/lib.rs:857`) —
//! НЕчейнящий путь. Значит в пути gateway оба поля пока инертны, и на ВЫХОДЕ gateway разницы
//! не видно: оракула уровня gateway, который поймал бы подмену, не существует, и заявлять
//! обратное нельзя. Поэтому оракул стоит ЗДЕСЬ, где инвариант наблюдаем через публичный API
//! книги. Требование остаётся обязательным, потому что:
//!   1. чекпоинт обязан round-trip'ить структуру целиком — «сейчас неиспользуемое» поле не
//!      выбрасывается молча (иначе переход gateway на `apply_l2delta` даст тихую регрессию,
//!      а не ошибку сборки);
//!   2. gap-детекция в кокпите — живой вопрос (docs/08 R5, TD-016).
//!
//! COMPILE-RED: `#[derive(Serialize, Deserialize)]` на `OrderBook` ещё нет (как и serde в
//! `crates/book/Cargo.toml` и `serde_json` в dev-deps) — задача #1 M-38b.
//!
//! testing.md: п.3 **отсутствие** (roundtrip не «додумывает» состояние), п.4 **границы**
//! (пустая книга / книга без чейна / книга в stale), п.7 **парный vantage** (stale переживает
//! roundtrip И не-stale не становится stale).

use book::{ContinuityStatus, OrderBook};
use contracts::{to_fixed, Level, Side};

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

fn roundtrip(b: &OrderBook) -> OrderBook {
    let bytes = serde_json::to_vec(b).expect("сериализация OrderBook");
    serde_json::from_slice(&bytes).expect("десериализация OrderBook")
}

/// Книга с ЗАВЕДЁННЫМ чейном (последний `final_update_id` = 10).
fn chained_book() -> OrderBook {
    let mut b = OrderBook::new();
    b.apply_snapshot(&[lvl(100.0, 2.0), lvl(99.0, 1.0)], &[lvl(101.0, 2.0)]);
    assert_eq!(
        b.apply_l2delta(&[lvl(100.0, 3.0)], &[], 10, 10, None),
        ContinuityStatus::Applied,
        "bootstrap-дельта обязана примениться"
    );
    b
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Чейн переживает roundtrip — следующая дельта ПРОВЕРЯЕТСЯ, а не bootstrap'ится
// ─────────────────────────────────────────────────────────────────────────────

/// Ключевой тест против «`levels()` + `apply_snapshot()`»: после roundtrip книга обязана
/// ПОМНИТЬ `last_final_update_id = 10`, поэтому РАЗОРВАННАЯ дельта (`pu = 99 ≠ 10`) обязана
/// дать `Gap`. Реализация, потерявшая чейн, уйдёт в ветку bootstrap и вернёт `Applied` —
/// то есть молча ПРИМЕНИТ разорванную дельту и испортит книгу.
#[test]
fn chain_survives_roundtrip_gap_still_detected() {
    let mut restored = roundtrip(&chained_book());
    let status = restored.apply_l2delta(&[lvl(100.0, 9.0)], &[], 100, 100, Some(99));
    assert_eq!(
        status,
        ContinuityStatus::Gap,
        "last_final_update_id НЕ пережил roundtrip: разорванная дельта (pu=99, а чейн стоял на \
         10) принята как bootstrap. Так выглядит «восстановление через apply_snapshot» — \
         gap-детекция (GD-I-1..4) молча выключается на одну дельту."
    );
    assert!(
        restored.is_stale(),
        "после Gap книга обязана стать stale (fail-closed, GD-I-2/4/6)"
    );
}

/// Парный vantage: НЕразорванная дельта после roundtrip обязана примениться. Реализация,
/// которая «на всякий случай» помечает восстановленную книгу stale, падает здесь.
#[test]
fn valid_delta_after_roundtrip_still_applies() {
    let mut restored = roundtrip(&chained_book());
    assert_eq!(
        restored.apply_l2delta(&[lvl(100.0, 5.0)], &[], 11, 11, Some(10)),
        ContinuityStatus::Applied,
        "непрерывная дельта (pu=10 == last) обязана примениться после roundtrip"
    );
    assert!(
        !restored.is_stale(),
        "валидная дельта не делает книгу stale"
    );
    assert_eq!(
        restored.size_at(Side::Buy, to_fixed(100.0)),
        to_fixed(5.0),
        "дельта обязана быть применена к восстановленной книге"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. `stale` переживает roundtrip — недостоверная книга не «выздоравливает»
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn stale_survives_roundtrip() {
    let mut b = chained_book();
    assert_eq!(
        b.apply_l2delta(&[lvl(100.0, 1.0)], &[], 50, 50, Some(49)),
        ContinuityStatus::Gap,
        "фикстура: дельта с разрывом обязана дать Gap"
    );
    assert!(b.is_stale(), "фикстура: книга обязана стать stale");

    let mut restored = roundtrip(&b);
    assert!(
        restored.is_stale(),
        "stale НЕ пережил roundtrip: недостоверная книга молча стала достоверной. Кокпит \
         показал бы мёртвую ликвидность как живую — класс тихой лжи, ради которого M-30 \
         сделал gap-детекцию fail-closed."
    );
    assert_eq!(
        restored.apply_l2delta(&[lvl(100.0, 1.0)], &[], 51, 51, Some(50)),
        ContinuityStatus::Gap,
        "stale-книга обязана отвергать ВСЁ до ресинка снапшотом (GD-I-2/4/6), даже валидные \
         по виду дельты"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Уровни и границы (п.3 отсутствие, п.4 границы)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn levels_survive_roundtrip_both_sides() {
    let b = chained_book();
    let restored = roundtrip(&b);
    for side in [Side::Buy, Side::Sell] {
        assert_eq!(
            restored.levels(side),
            b.levels(side),
            "уровни стороны {side:?} обязаны совпасть побайтово после roundtrip"
        );
    }
    assert_eq!(restored.best_bid(), b.best_bid());
    assert_eq!(restored.best_ask(), b.best_ask());
    assert_eq!(restored.mid(), b.mid());
}

/// Границы: пустая книга и книга БЕЗ заведённого чейна (`last_final_update_id = None`).
/// Roundtrip не должен «додумывать» чейн — иначе первая же дельта после восстановления
/// пойдёт по ветке проверки вместо bootstrap и будет ошибочно отвергнута.
#[test]
fn empty_and_unchained_books_roundtrip() {
    let empty = roundtrip(&OrderBook::new());
    assert_eq!(empty.best_bid(), None);
    assert_eq!(empty.best_ask(), None);
    assert!(!empty.is_stale());

    let mut fresh = OrderBook::new();
    fresh.apply_snapshot(&[lvl(100.0, 1.0)], &[lvl(101.0, 1.0)]);
    let mut restored = roundtrip(&fresh);
    assert_eq!(
        restored.apply_l2delta(&[lvl(100.0, 2.0)], &[], 777, 777, Some(776)),
        ContinuityStatus::Applied,
        "книга без заведённого чейна обязана остаться такой после roundtrip: первая дельта — \
         bootstrap (GD-I-5), а не «разрыв» относительно выдуманного last_final_update_id"
    );
}
