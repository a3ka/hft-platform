//! RED `M-84` (sacred, architect-only) — **слепки РАЗНЫХ наборов СОСУЩЕСТВУЮТ, поэтому новый
//! собирается при живом старом и окно переключения НУЛЕВОЕ.**
//!
//! Заведён закрытием `C-220` B-4, переписан по `C-221` B-2. COMPILE-RED:
//! `gateway::CANONICAL_DEPTH_BANDS` ещё нет.
//!
//! ## ПОЧЕМУ ПЕРЕПИСАН — ОРАКУЛ ТРЕБОВАЛ ДЫРЫ В ГВАРДЕ
//!
//! Прежняя редакция строила «живой слепок сегодняшнего прода» вызовом
//! `checkpoint::advance_to` с легаси-набором `[0.001]`. Но `advance_to` первой строкой зовёт
//! `gateway::validate_selector` (`crates/gateway/src/lib.rs:3541`), а
//! `red_fixed_bands_canonical.rs` требует от того же гварда легаси-набор ОТВЕРГНУТЬ.
//! Единственный способ удовлетворить оба требования — убрать проверку из `advance_to`, то
//! есть открыть обход: через прогрев в систему заезжает ЛЮБАЯ сетка. Критик это и собрал —
//! мутация «точный гвард + удалить вызов из `advance_to`» оставила ЧЕТЫРЕ набора M-84
//! зелёными (`C-221` B-2, Done Block).
//!
//! ## РАЗВЯЗКА: ЛЕГАСИ-СЛЕПОК НЕ СОЗДАЁТСЯ, ОН УЖЕ ЛЕЖИТ
//!
//! На проде слепок старого набора **уже существует** — его написал СЕГОДНЯШНИЙ код, до
//! M-84. Новому коду не нужно уметь его создавать; ему нужно не затереть чужое имя и
//! построить своё рядом. Поэтому фикстура кладёт легаси-файл НАПРЯМУЮ: берутся байты
//! настоящего слепка (построенного законным путём, каноническим набором) и копируются под
//! ЛЕГАСИ-ИМЕНЕМ. Форма файла при этом настоящая, а не мусор, — отличается ровно то, что и
//! отличается в проде: отпечаток в имени.
//!
//! Имя выводится из `checkpoint::selector_fingerprint` — чистой функции, которая НЕ
//! валидирует селектор. Построение `Selector`-значения и его проверка — разные вещи, и это
//! не обход: на диск ничего не пишется гвардованным путём.
//!
//! **Мутация, которую файл теперь ловит:** удалить `validate_selector(sel)?` из
//! `advance_to` ⇒ падает `legacy_selector_is_refused_by_the_normal_checkpoint_api`.
//!
//! ## ЧТО ИМЕННО ДОКАЗЫВАЕТСЯ — и почему это НЕ тавтология
//!
//! Замер прода 2026-09-10: холодная пересборка **965 с** при интервале чекпоинтера **900 с**.
//! Пересборка ДОЛЬШЕ периода запуска, поэтому наивная выкатка («сменили набор — ждём») даёт
//! окно, которое не закрывается само: каждое подключение идёт полным проходом журнала.
//!
//! Развязка держится на ОДНОМ свойстве: имя слепка детерминировано отпечатком
//! (`ckpt-<fp_hex16>.bin`), значит слепки разных наборов лежат РЯДОМ и не затирают друг друга.
//! Свойство проверяемо исполнением — и проверяется здесь, а не обещается в README.
//!
//! ## `testing.md` чек-лист
//! - **setup-guard на каждый сценарий**: имя, которое выводит тест, сверяется с именем,
//!   которое РЕАЛЬНО произвела реализация. Разойдутся — тест кричит о несостоявшемся
//!   setup, а не судит не тот файл;
//! - п.4 **границы** — прогрев ДО переключения, то есть старый слепок ещё живой;
//! - п.7 **ПАРНЫЙ vantage** — «оба файла существуют» И «байты старого не изменились».
//!   Первое без второго удовлетворяется реализацией, переписывающей старый файл заново.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_700_000_000_000;

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
    let cfg = WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for e in events {
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn sel(bands: Vec<f64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
        depth_cadence_ms: None,
    }
}

fn ckpt_files(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read_dir ckpt")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("ckpt-") && n.ends_with(".bin"))
        .collect();
    v.sort();
    v
}

/// Имя слепка, выведенное из отпечатка. Дублирует формулу реализации (`RN-23`,
/// `ckpt-<fp_hex16>.bin`) — и ровно поэтому каждый сценарий сверяет вывод с настоящим
/// именем, произведённым `advance_to`. Формула разойдётся — упадёт setup-guard, а не
/// смысловой ассерт.
fn fp_name(s: &Selector) -> String {
    format!(
        "ckpt-{:016x}.bin",
        gateway::checkpoint::selector_fingerprint(s)
    )
}

/// Настоящий слепок канонического набора, построенный ЗАКОННЫМ путём. Возвращает его байты
/// — из них делается легаси-фикстура (та же форма файла, другое имя).
fn real_checkpoint_bytes(jdir: &std::path::Path) -> Vec<u8> {
    let scratch = tempfile::tempdir().expect("tempdir scratch");
    let canonical = sel(gateway::CANONICAL_DEPTH_BANDS.to_vec());
    gateway::checkpoint::advance_to(
        jdir,
        scratch.path(),
        &canonical,
        EpochFilter::OwnCaptureOnly,
        Cursor::LATEST,
    )
    .expect("слепок канонического набора обязан строиться законным путём");
    let produced = ckpt_files(scratch.path());
    assert_eq!(
        produced,
        vec![fp_name(&canonical)],
        "SETUP НЕ СОСТОЯЛСЯ: имя, выведенное тестом из отпечатка, разошлось с тем, что \
         произвела реализация. Дальше судить нечего — тест смотрел бы не на тот файл"
    );
    std::fs::read(scratch.path().join(&produced[0])).expect("read ckpt bytes")
}

#[test]
fn legacy_selector_is_refused_by_the_normal_checkpoint_api() {
    // МУТАЦИЯ-УБИЙЦА (`C-221` B-2). Обычный публичный путь чекпоинта обязан оставаться
    // гвардованным: убери проверку из `advance_to` ради «прогрева старого набора» — и
    // через прогрев в систему заедет любая сетка, мимо решения П-029.
    let jdir = journal_of(vec![trade(65_000.0, 1.0, Side::Buy, T)]);
    let ckpt = tempfile::tempdir().expect("tempdir ckpt");
    let err = gateway::checkpoint::advance_to(
        jdir.path(),
        ckpt.path(),
        &sel(vec![0.001]),
        EpochFilter::OwnCaptureOnly,
        Cursor::LATEST,
    )
    .expect_err(
        "легаси-набор обязан быть ОТВЕРГНУТ обычным API чекпоинта. Успех здесь означает \
         дыру в гварде: прогрев становится входом для произвольной сетки (C-221 B-2)",
    );
    assert!(
        err.to_string().contains("bands") || err.to_string().contains("полос"),
        "отказ обязан НАЗЫВАТЬ причину — оператор выкатки читает именно это. Получено: {err}"
    );
    assert!(
        ckpt_files(ckpt.path()).is_empty(),
        "отказ обязан быть fail-closed: при отвергнутом селекторе на диск не ложится ничего"
    );
}

#[test]
fn prewarmed_canonical_checkpoint_coexists_with_live_legacy() {
    let jdir = journal_of(vec![
        trade(65_000.0, 2.0, Side::Buy, T),
        trade(65_010.0, 1.0, Side::Sell, T + 1),
    ]);
    let ckpt = tempfile::tempdir().expect("tempdir ckpt");

    // ЖИВОЙ слепок сегодняшнего прода. Он не СОЗДАЁТСЯ новым кодом — он там УЖЕ ЛЕЖИТ,
    // написанный кодом до M-84. Фикстура повторяет именно это: настоящая форма файла под
    // легаси-именем. Звать `advance_to` с легаси-набором нельзя — см. шапку файла.
    let legacy = sel(vec![0.001]);
    let legacy_name = fp_name(&legacy);
    let bytes = real_checkpoint_bytes(jdir.path());
    std::fs::write(ckpt.path().join(&legacy_name), &bytes).expect("положить легаси-слепок");
    assert_eq!(
        ckpt_files(ckpt.path()),
        vec![legacy_name.clone()],
        "SETUP НЕ СОСТОЯЛСЯ: в каталоге обязан лежать РОВНО легаси-слепок"
    );

    // ПРОГРЕВ: слепок канонического набора собирается ПРИ ЖИВОМ старом.
    let canonical = sel(gateway::CANONICAL_DEPTH_BANDS.to_vec());
    gateway::checkpoint::advance_to(
        jdir.path(),
        ckpt.path(),
        &canonical,
        EpochFilter::OwnCaptureOnly,
        Cursor::LATEST,
    )
    .expect("слепок канонического набора обязан строиться РЯДОМ, а не вместо");

    let after = ckpt_files(ckpt.path());
    assert_eq!(
        after.len(),
        2,
        "слепки РАЗНЫХ наборов обязаны СОСУЩЕСТВОВАТЬ: на этом стоит нулевое окно выкатки. \
         Получено {after:?}. Замер прода: холодная пересборка 965 с при интервале \
         чекпоинтера 900 с — без сосуществования окно не закрывается само"
    );
    assert!(
        after.contains(&legacy_name),
        "старый слепок ЗАТЁРТ прогревом ({legacy_name} → {after:?}). Переключение тогда \
         пойдёт на пустоту, и окно полных проходов откроется ровно в момент выкатки"
    );
    assert!(
        after.contains(&fp_name(&canonical)),
        "прогретого слепка нет под ожидаемым именем: {after:?}"
    );
    // ПАРНЫЙ vantage: мало «файл на месте» — он обязан быть ТЕМ ЖЕ. Реализация, честно
    // пересобирающая старый слепок под тем же именем, съела бы ровно то время, ради
    // экономии которого вся конструкция и затевалась.
    let still = std::fs::read(ckpt.path().join(&legacy_name)).expect("read legacy after");
    assert_eq!(
        still, bytes,
        "байты живого слепка изменились во время прогрева: он не сосуществует, а \
         перезаписывается"
    );
}

#[test]
fn checkpoint_name_differs_by_band_set() {
    // ПАРНЫЙ vantage к предыдущему: два файла могли бы существовать и по другой причине.
    // Здесь проверяется ПРИЧИНА — имя определяется отпечатком, в который входят полосы.
    let jdir = journal_of(vec![trade(65_000.0, 1.0, Side::Buy, T)]);
    let canonical = sel(gateway::CANONICAL_DEPTH_BANDS.to_vec());
    let dir = tempfile::tempdir().expect("dir");
    gateway::checkpoint::advance_to(
        jdir.path(),
        dir.path(),
        &canonical,
        EpochFilter::OwnCaptureOnly,
        Cursor::LATEST,
    )
    .expect("advance_to");
    assert_eq!(
        ckpt_files(dir.path()),
        vec![fp_name(&canonical)],
        "имя произведённого слепка обязано выводиться из отпечатка канонического селектора"
    );
    assert_ne!(
        fp_name(&canonical),
        fp_name(&sel(vec![0.001])),
        "имена слепков обязаны РАЗЛИЧАТЬСЯ по набору полос — иначе прогретый слепок затрёт \
         живой, и вся конструкция нулевого окна рассыпается"
    );
}
