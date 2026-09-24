//! RED M-87 задача 20 (sacred, architect-only) — **СБОЙ вычисления провенанса истории
//! не имеет права молчать: не знаем — значит НЕ ОБЕЩАЕМ полноту.**
//!
//! ## Находка, и она того же класса, что задача 19
//!
//! Задача 17 завела честный пересчёт: `LiveReducer` держит `history_*` ЗАМОРОЖЕННЫМИ из
//! слепка, а ретеншен между снятием слепка и обслуживанием мог удалить ранние сегменты —
//! значит `history_truncated = false` из слепка врёт. Пересчёт против ТЕКУЩЕГО начала
//! журнала это лечит.
//!
//! Лечит — пока считается. Оба вызывателя в транспорте написаны так:
//!
//! ```text
//! if let Ok((live_start, live_truncated)) = current_history_provenance(dir, filter) {
//!     snap.history_start_seq = live_start;
//!     snap.history_truncated = live_truncated;
//! }
//! ```
//!
//! При `Err` перезапись МОЛЧА не происходит, и клиенту уходит замороженное значение —
//! ровно та ложь, ради устранения которой задача 17 и заводилась. Никто не узнает:
//! ни кода ошибки, ни метки, ни записи в журнале.
//!
//! **Это тот же класс, что задача 19** (`§14.1nonies`), которую тем же кругом пришлось
//! откатывать: механизм честности, отключающийся при сбое, честности не даёт. Разница
//! только в том, что там ошибку глотал `advance`, а здесь — транспорт.
//!
//! ## Почему проверяется ФУНКЦИЯ, а не сценарий — предел назван честно
//!
//! Уронить провенанс ОТДЕЛЬНО от чтения журнала нельзя: `current_history_provenance` и
//! `journal::stream` входят в одну дверь (`journal::list_segments`), и битый каталог
//! роняет снимок раньше, чем дело дойдёт до провенанса. На проде расхождение достижимо
//! ГОНКОЙ — компакция переименовывает `.jrnl` → `.jrnl.zst` между двумя вызовами, — но
//! гонка не воспроизводится детерминированно, а недетерминированный оракул есть флак
//! (`testing.md` §«Целостность гейта», свойство 2).
//!
//! Поэтому предмет оракула — ОБРАБОТКА ОШИБКИ, вынесенная в названную функцию. Это не
//! обходной путь, а требование к форме: механизм, который нельзя предъявить прогоном,
//! существует только на словах.
//!
//! ## Что обязан сделать dev (задача 20)
//!
//! Завести в `gateway::checkpoint` функцию с точной сигнатурой
//!
//! ```text
//! pub fn history_provenance_for_serve(
//!     dir: impl AsRef<Path>,
//!     filter: EpochFilter,
//!     frozen_start_seq: u64,
//! ) -> (u64, bool)
//! ```
//!
//! · `Ok((start, truncated))` от `current_history_provenance` → вернуть КАК ЕСТЬ;
//! · `Err(_)` → вернуть `(frozen_start_seq, true)`.
//!
//! И заменить ОБА `if let Ok(...)` в `crates/gateway-serve/src/lib.rs` вызовом этой
//! функции. `current_history_provenance` остаётся — она честная, глотал ошибку не она.
//!
//! ## Почему консервативно, а не отказом и не новым полем
//!
//! Три развилки, выбор назван, а не подразумевается:
//!
//! · **отказ выдачи** — роняет исправную выдачу из-за секундного сбоя чтения каталога.
//!   `VB-I-11` говорит обратное: «система НЕ отказывается отдать то, что есть»;
//! · **новое поле «провенанс неизвестен»** — честнее всего, но меняет форму провода
//!   (`GATEWAY_SCHEMA_VERSION` 11→12) и ломает потребителя ради краевого случая.
//!   Назван кандидатом на СЛЕДУЮЩИЙ бамп формы, а не отвергнут;
//! · **консервативная трактовка** (выбрана) — не знаем, значит не обещаем полноту.
//!   Асимметрия цены очевидна: сказать «может быть неполна» о полной истории — потеря
//!   функции у клиента; сказать «полная» об усечённой — порча его данных.
//!
//! ## COMPILE-RED по построению
//!
//! `history_provenance_for_serve` ещё не существует — набор не собирается, и это
//! ЗАЯВЛЕННОЕ состояние, а не авария.
//!
//! testing.md п.1 асимметрия: успех и сбой судятся ПОРОЗНЬ · п.3 отсутствие: сбой не
//! есть сигнал «всё хорошо» · п.4 границы: целый журнал / усечённый / нечитаемый путь /
//! несуществующий путь · парный vantage (`h1`): реализация «всегда truncated» падает.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

const N: u64 = 2_000;
const SEG_BYTES: u64 = 8 * 1024;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
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
            price: to_fixed(100.0 + (i % 7) as f64),
            size: to_fixed(1.0 + (i % 3) as f64),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_752_000_000_000 + i as i64 * 100,
        },
    )
}

fn intact_journal() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    dir
}

/// Прод-форма усечения: нижние сегменты удаляются ФИЗИЧЕСКИ (purge M-36 / retention-prune).
fn truncated_journal() -> (tempfile::TempDir, u64) {
    let dir = intact_journal();
    let total = journal::list_segments(dir.path()).expect("segments").len() as u32;
    assert!(total >= 6, "нужен многосегментный журнал, есть {total}");
    for s in journal::list_segments(dir.path())
        .expect("segments")
        .iter()
        .filter(|s| s.index < 3)
    {
        std::fs::remove_file(&s.path).expect("remove segment");
    }
    let earliest = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .next()
        .expect("хотя бы одно событие осталось")
        .expect("event")
        .seq;
    assert!(
        earliest > 0,
        "SETUP-СТРАЖ: фикстура обязана быть усечённой, earliest={earliest}"
    );
    (dir, earliest)
}

/// **h0 — SETUP-СТРАЖ: пути, которыми моделируется сбой, ДЕЙСТВИТЕЛЬНО его дают.**
///
/// Без этого `h3`/`h4` могли бы проверять не тот сценарий: если бы
/// `current_history_provenance` на этих путях возвращала `Ok`, оба сценария зеленели бы
/// на ЛЮБОЙ реализации, включая сегодняшнюю. Проба, молча тестирующая не тот случай, —
/// плацебо самой себя (`testing.md` §«Целостность гейта», свойство 3).
#[test]
fn h0_setup_guard_broken_paths_really_fail() {
    let f = tempfile::NamedTempFile::new().expect("tempfile");
    assert!(
        gateway::checkpoint::current_history_provenance(f.path(), EpochFilter::OwnCaptureOnly)
            .is_err(),
        "SETUP-СТРАЖ: путь к ФАЙЛУ обязан давать Err — иначе h3 судит не тот сценарий"
    );

    let missing = std::path::Path::new("/nonexistent-hft-m87-task20");
    assert!(
        gateway::checkpoint::current_history_provenance(missing, EpochFilter::OwnCaptureOnly)
            .is_err(),
        "SETUP-СТРАЖ: несуществующий путь обязан давать Err — иначе h4 судит не тот сценарий"
    );
}

/// **h1 — ПАРНЫЙ VANTAGE: на целом журнале история НЕ объявляется усечённой.**
///
/// Ловит заглушку «всегда truncated», которой иначе `h3`/`h4` были бы удовлетворены.
/// Замороженное значение при этом заведомо ЛОЖНОЕ (`u64::MAX`) — если реализация вернёт
/// его на исправном пути, тест покраснеет.
#[test]
fn h1_intact_journal_is_not_declared_truncated() {
    let dir = intact_journal();
    let (start, truncated) = gateway::checkpoint::history_provenance_for_serve(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        u64::MAX,
    );
    assert!(
        !truncated,
        "целый журнал объявлен усечённым — «всегда truncated» честностью не является"
    );
    assert_eq!(
        start,
        0,
        "на целом журнале начало истории — 0, получено {start}; \
         значение {} означало бы, что вернули ЗАМОРОЖЕННОЕ вместо вычисленного",
        u64::MAX
    );
}

/// **h2 — усечённый журнал объявляется усечённым, и начало берётся ВЫЧИСЛЕННОЕ.**
#[test]
fn h2_truncated_journal_is_declared_truncated() {
    let (dir, earliest) = truncated_journal();
    let (start, truncated) = gateway::checkpoint::history_provenance_for_serve(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        0,
    );
    assert!(
        truncated,
        "у журнала удалён префикс, но история не объявлена усечённой (VB-I-11)"
    );
    assert_eq!(
        start, earliest,
        "начало истории обязано быть ВЫЧИСЛЕННЫМ ({earliest}), а не замороженным нулём"
    );
}

/// **h3 — ГЛАВНЫЙ: сбой вычисления НЕ выдаётся за «история полная».**
///
/// Замороженное из слепка значение говорит «полная» (`truncated=false` — именно так
/// выглядит слепок, снятый ДО чистки журнала). Вычислить текущее состояние не удалось.
/// Сегодняшний код в этом месте молчит и отдаёт замороженное; требование — обратное.
#[test]
fn h3_unreadable_journal_does_not_claim_complete_history() {
    let f = tempfile::NamedTempFile::new().expect("tempfile");
    const FROZEN: u64 = 16_049_334;
    let (start, truncated) = gateway::checkpoint::history_provenance_for_serve(
        f.path(),
        EpochFilter::OwnCaptureOnly,
        FROZEN,
    );
    assert!(
        truncated,
        "провенанс не вычислился, а выдача объявила историю ПОЛНОЙ. Это ровно та ложь, \
         ради устранения которой заведена задача 17: слепок снят до чистки журнала и \
         помнит «полная», а проверить это сейчас не удалось. Не знаем — не обещаем."
    );
    assert_eq!(
        start, FROZEN,
        "начало истории при сбое берётся ЗАМОРОЖЕННОЕ (лучшее, что известно), а не \
         выдумывается: получено {start}"
    );
}

/// **h4 — тот же контракт на втором виде сбоя: каталога нет вовсе.**
///
/// Два разных кода ошибки ввода-вывода (`NotADirectory` и `NotFound`) обязаны давать
/// ОДИН исход. Реализация, разбирающая коды ошибок поимённо, рано или поздно встретит
/// третий и снова замолчит.
#[test]
fn h4_missing_journal_dir_does_not_claim_complete_history() {
    const FROZEN: u64 = 7;
    let (start, truncated) = gateway::checkpoint::history_provenance_for_serve(
        std::path::Path::new("/nonexistent-hft-m87-task20"),
        EpochFilter::OwnCaptureOnly,
        FROZEN,
    );
    assert!(
        truncated,
        "отсутствующий каталог журнала — тоже НЕЗНАНИЕ, а не подтверждение полноты"
    );
    assert_eq!(start, FROZEN, "начало истории при сбое — замороженное");
}
