//! RED M-69 (sacred, architect-only) — **GW-I-14 в БИБЛИОТЕКЕ: отрицательное `window_ms`
//! отвергается на всех публичных входах.**
//!
//! Парный оракул к `crates/gateway-serve/tests/red_window_guard_startup.rs`. Тот доказывает, что
//! прод-бинарь не стартует с невалидным `GATEWAY_WINDOW_MS`. ЭТОТ доказывает, что мимо гварда
//! нельзя пройти в обход транспорта.
//!
//! ## Почему второй точки недостаточно без первой, и наоборот (урок TD-019/TD-020, M-47)
//!
//! `Selector` — публичная структура с публичными полями (`crates/gateway/src/lib.rs:113-122`),
//! и её собирают НАПРЯМУЮ, минуя `serve_config_from_env`: чекпоинтер (`gateway-checkpoint`,
//! M-38b), будущий shared-tailer (M-39), `research-cli`. Гвард только в конфиге транспорта
//! оставил бы им байпас-поверхность — ровно дефект «механизм есть, никто не зовёт», который
//! M-47 уже ловил на этом же коде для `timeframe_ms`.
//!
//! Замер, из-за которого оракул написан: `validate_selector`
//! (`crates/gateway/src/lib.rs:1751-1764`) проверяет ТОЛЬКО `timeframe_ms`. `window_ms` не
//! смотрит НИ ОДНА проверка НИ НА ОДНОМ входе.
//!
//! ## Что именно ломает отрицательное окно
//!
//! `Selector::window_lo_time_s` (`crates/gateway/src/lib.rs:130-133`):
//!
//! ```ignore
//! let w = self.window_ms?;        // None     → None (offline, легитимно)
//! if w <= 0 { return None; }      // Some(<0) → None (unbounded — НЕ заказывали)
//! ```
//!
//! То есть `Some(-60000)` ведёт себя как unbounded, но при этом ОТЛИЧАЕТСЯ от `None` в
//! `selector_fingerprint` (`crates/gateway/src/lib.rs:2268-2280`, M-38b). Следствие: чекпоинт
//! ключуется как «особый селектор», снят под режимом, которого оператор не просил, и остаётся
//! валидным по CRC. Дословно аргумент срочности M-47 — «чекпоинт под невалидным селектором есть
//! мусор, выглядящий валидным».
//!
//! ## Граница предмета
//!
//! `Some(0)` — НЕ ошибка: это принятый в этом коде способ выразить offline
//! (`crates/gateway/src/bin/gateway-checkpoint.rs:162-163`: «`0` ⇒ `None` offline unbounded»).
//! Оракул это фиксирует, чтобы фикс не сломал паритет argv-пути с env-путём.
//!
//! ## testing.md чек-лист
//! - п.4 **границы** — `-1` (граница знака), `-60000` (прод-значение со знаком), `0` (offline),
//!   `1` (минимальное окно), `i64::MAX`.
//! - п.6 **композиция** — отказ обязан держать на ВСЕХ трёх входах (`snapshot` / `frames_since`
//!   / `replay`), иначе байпас через merge-путь.
//! - п.7 **ПАРНЫЙ vantage** — `valid_windows_accepted` валит заглушку «всегда `Err`».
//!
//! RUNTIME-RED: против сегодняшнего кода падают все `negative_window_rejected_by_*` —
//! `validate_selector` окно не проверяет, вход принимается.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{validate_selector, Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(price: f64, size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

/// Асимметричная фикстура (testing.md п.1): разные стороны, разные размеры, разные моменты.
fn fixture() -> tempfile::TempDir {
    journal_of(vec![
        trade(100.0, 5.0, Side::Buy, D2_MS - 2_000),
        trade(101.0, 3.0, Side::Sell, D2_MS + 2_000),
    ])
}

/// `timeframe_ms` держим КОНСТАНТНЫМ и валидным (1000 делит сутки) — варьируем ТОЛЬКО
/// измеряемую величину `window_ms` (testing.md, целостность гейта, свойство 2).
fn sel(window_ms: Option<i64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms,
    }
}

/// Единственная точка правды о требуемой форме отказа — engine-dev реализует ровно это.
fn assert_rejected(what: &str, res: std::io::Result<impl std::fmt::Debug>, window_ms: i64) {
    match res {
        Err(e) => {
            assert_eq!(
                e.kind(),
                std::io::ErrorKind::InvalidInput,
                "{what}: window_ms={window_ms} отвергнут, но НЕ как InvalidInput: {e:?}"
            );
            let msg = e.to_string();
            assert!(
                msg.contains("window_ms"),
                "{what}: сообщение об отказе обязано называть поле `window_ms` (оператор должен \
                 понять, ЧТО чинить), получено: {msg:?}"
            );
        }
        Ok(v) => panic!(
            "GW-I-14 НАРУШЕН — {what}: window_ms={window_ms} отрицателен ⇒ window_lo_time_s даёт \
             None (поведение unbounded при НЕПУСТОМ window_ms), а selector_fingerprint при этом \
             отличается от offline-селектора ⇒ чекпоинт под незаказанным режимом, валидный по \
             CRC. Вход принят. Выход: {v:?}"
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RED 1 — отрицательное окно отвергается на ВСЕХ публичных входах (п.6 композиция)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn negative_window_rejected_by_snapshot() {
    let dir = fixture();
    let s = sel(Some(-60_000));
    assert_rejected(
        "gateway::snapshot",
        gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST),
        -60_000,
    );
}

#[test]
fn negative_window_rejected_by_frames_since() {
    let dir = fixture();
    let s = sel(Some(-60_000));
    assert_rejected(
        "gateway::frames_since",
        gateway::frames_since(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &s,
            Cursor::START,
            usize::MAX,
        ),
        -60_000,
    );
}

#[test]
fn negative_window_rejected_by_replay() {
    let dir = fixture();
    let s = sel(Some(-60_000));
    assert_rejected(
        "gateway::replay",
        gateway::replay(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &s,
            Cursor::START,
            Cursor::LATEST,
        ),
        -60_000,
    );
}

/// Граница знака. `catch_unwind` — страховка по образцу M-47: если гвард реализуют через
/// `assert!`/`unwrap`, оракул обязан отличить панику от честного `Err`, а не «случайно
/// позеленеть». Паника валит соединение в рантайме вместо явной ошибки конфигурации на входе.
#[test]
fn minus_one_window_rejected_not_panic() {
    let dir = fixture();
    let s = sel(Some(-1));
    let res = std::panic::catch_unwind(|| {
        gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
    });
    match res {
        Ok(r) => assert_rejected("gateway::snapshot(window=-1)", r, -1),
        Err(_) => {
            panic!("GW-I-14 НАРУШЕН: window_ms=-1 ПАНИКУЕТ вместо fail-closed Err(InvalidInput)")
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RED 2 — precondition живёт ИМЕННО в `validate_selector` (C-099 B-4).
//
// Прежняя редакция доказывала это канарейкой verify: `awk` по телу функции + `grep window_ms`.
// Критик предъявил мутанта, который её проходит: `let _ = &sel.window_ms;` при безусловном
// `Ok(())` даёт exit 0. Проверка присутствия ИМЕНИ не есть проверка ПОВЕДЕНИЯ — ровно то, что
// `testing.md` запрещает («проверка по ВЫЗОВУ, а не по тексту»). Канарейка снята, вместо неё —
// прямой вызов централизованного предусловия, который этот мутант роняет.
//
// Функциональные тесты `snapshot`/`frames_since`/`replay` выше остаются отдельным
// доказательством ПРОВОДКИ: они держат результат, но не говорят, ГДЕ стоит проверка.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_selector_itself_rejects_negative_window() {
    for w in [-1_i64, -60_000, i64::MIN + 1] {
        let s = sel(Some(w));
        match validate_selector(&s) {
            Err(e) => {
                assert_eq!(
                    e.kind(),
                    std::io::ErrorKind::InvalidInput,
                    "validate_selector(window_ms={w}): отказ обязан быть InvalidInput, получено {e:?}"
                );
                assert!(
                    e.to_string().contains("window_ms"),
                    "validate_selector(window_ms={w}): сообщение обязано называть поле \
                     `window_ms`, получено: {e}"
                );
            }
            Ok(()) => panic!(
                "GW-I-14 НАРУШЕН: централизованное предусловие `validate_selector` приняло \
                 window_ms={w}. Проверка обязана жить ЗДЕСЬ, а не только в вызывающих: иначе \
                 чекпоинтер (M-38b), shared-tailer (M-39) и research-cli собирают Selector \
                 напрямую и проходят мимо гварда — класс TD-019/TD-020."
            ),
        }
    }
}

/// Парный vantage к предыдущему: предусловие не перешироко. Валит и заглушку «всегда Err»,
/// и попытку отвергнуть легитимный offline.
#[test]
fn validate_selector_itself_accepts_valid_windows() {
    for w in [None, Some(0), Some(1), Some(60_000), Some(i64::MAX)] {
        let s = sel(w);
        assert!(
            validate_selector(&s).is_ok(),
            "GW-I-14 ПЕРЕШИРОК: validate_selector отверг валидный window_ms={w:?} \
             (None и Some(0) — легитимный offline, положительное — bounded): {:?}",
            validate_selector(&s).err()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ПАРНЫЙ vantage (п.7) — гвард не переширокий. Валит заглушку «всегда Err».
// ─────────────────────────────────────────────────────────────────────────────

/// `None` (offline) и `Some(0)` (явный offline, паритет с argv чекпоинтера) обязаны
/// приниматься, как и любое положительное окно. Отвергать их — сломать research-cli,
/// replay-tutor и чекпоинтер, у которых окна нет по построению.
#[test]
fn valid_windows_accepted() {
    let dir = fixture();
    for w in [None, Some(0), Some(1), Some(60_000), Some(i64::MAX)] {
        let s = sel(w);
        let got = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST);
        assert!(
            got.is_ok(),
            "GW-I-14 ПЕРЕШИРОК: window_ms={w:?} валиден (None и 0 — легитимный offline, \
             положительное — bounded) и обязан приниматься, но отвергнут: {:?}",
            got.err()
        );
    }
}

/// Соседний инвариант GW-I-10 (M-47) обязан остаться нетронутым: гвард окна не имеет права
/// ни ослабить, ни подменить гвард таймфрейма.
#[test]
fn timeframe_guard_untouched_by_window_guard() {
    let dir = fixture();
    let s = Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 11_000, // не делит сутки — GW-I-10 обязан отвергнуть
        bands: vec![0.001],
        window_ms: Some(60_000), // окно валидно
    };
    let got = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST);
    match got {
        Err(e) => assert!(
            e.to_string().contains("timeframe_ms"),
            "при валидном окне и невыравненном таймфрейме отказ обязан называть `timeframe_ms` \
             (GW-I-10, M-47), получено: {e}"
        ),
        Ok(_) => panic!("GW-I-10 (M-47) регрессировал: timeframe_ms=11000 принят"),
    }
}
