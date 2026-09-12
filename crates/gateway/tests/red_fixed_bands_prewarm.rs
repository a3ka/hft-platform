//! RED `M-84` (sacred, architect-only) — **слепки РАЗНЫХ наборов СОСУЩЕСТВУЮТ, поэтому новый
//! собирается при живом старом и окно переключения НУЛЕВОЕ.**
//!
//! Заведён закрытием `C-220` B-4. COMPILE-RED: `gateway::CANONICAL_DEPTH_BANDS` ещё нет.
//!
//! ## ПОЧЕМУ ЭТОТ ОРАКУЛ ЗАМЕНЯЕТ ГРЕП ПО СЛОВАМ
//!
//! Прежний шаг гейта искал в `deploy/README.md` слова «заранее» и `ckpt-`. Критик назвал
//! последствие точно: замена ключевых слов прозой после реализации делала шаг зелёным БЕЗ
//! предварительного прогрева и без сосуществования слепков. Гейт проверял орфографию.
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
//! Тавтологии нет: тест строит слепок СТАРОГО набора, затем слепок КАНОНИЧЕСКОГО и требует,
//! чтобы ОБА остались на диске и были различимы. Реализация, затирающая слепок при смене
//! набора (например, «один слепок на каталог»), падает здесь и проходит все прочие тесты M-84.
//!
//! ## `testing.md` чек-лист
//! - п.4 **границы** — прогрев ДО переключения, то есть старый слепок ещё живой;
//! - п.7 **ПАРНЫЙ vantage** — «оба файла существуют» И «их имена различны». Первое без
//!   второго удовлетворяется реализацией, пишущей оба под одним именем по очереди.

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

#[test]
fn prewarmed_canonical_checkpoint_coexists_with_live_legacy() {
    let jdir = journal_of(vec![
        trade(65_000.0, 2.0, Side::Buy, T),
        trade(65_010.0, 1.0, Side::Sell, T + 1),
    ]);
    let ckpt = tempfile::tempdir().expect("tempdir ckpt");

    // ЖИВОЙ слепок сегодняшнего прода: одна полоса (GATEWAY_BANDS=0.001).
    gateway::checkpoint::advance_to(
        jdir.path(),
        ckpt.path(),
        &sel(vec![0.001]),
        EpochFilter::OwnCaptureOnly,
        Cursor::LATEST,
    )
    .expect("слепок старого набора обязан строиться");
    let after_legacy = ckpt_files(ckpt.path());
    assert_eq!(
        after_legacy.len(),
        1,
        "SETUP НЕ СОСТОЯЛСЯ: после первого прогона обязан лежать РОВНО один слепок, \
         получено {after_legacy:?}"
    );

    // ПРОГРЕВ: слепок канонического набора собирается ПРИ ЖИВОМ старом.
    gateway::checkpoint::advance_to(
        jdir.path(),
        ckpt.path(),
        &sel(gateway::CANONICAL_DEPTH_BANDS.to_vec()),
        EpochFilter::OwnCaptureOnly,
        Cursor::LATEST,
    )
    .expect("слепок канонического набора обязан строиться РЯДОМ, а не вместо");

    let after_prewarm = ckpt_files(ckpt.path());
    assert_eq!(
        after_prewarm.len(),
        2,
        "слепки РАЗНЫХ наборов обязаны СОСУЩЕСТВОВАТЬ: на этом стоит нулевое окно выкатки. \
         Получено {after_prewarm:?}. Замер прода: холодная пересборка 965 с при интервале \
         чекпоинтера 900 с — без сосуществования окно не закрывается само"
    );
    assert!(
        after_prewarm.contains(&after_legacy[0]),
        "старый слепок ЗАТЁРТ прогревом ({after_legacy:?} → {after_prewarm:?}). Переключение \
         тогда пойдёт на пустоту, и окно полных проходов откроется ровно в момент выкатки"
    );
}

#[test]
fn checkpoint_name_differs_by_band_set() {
    // ПАРНЫЙ vantage: два файла могли бы существовать и по другой причине. Здесь
    // проверяется ПРИЧИНА — имя определяется отпечатком, в который входят полосы.
    let jdir = journal_of(vec![trade(65_000.0, 1.0, Side::Buy, T)]);
    let a = tempfile::tempdir().expect("a");
    let b = tempfile::tempdir().expect("b");
    for (dir, s) in [
        (a.path(), sel(vec![0.001])),
        (b.path(), sel(gateway::CANONICAL_DEPTH_BANDS.to_vec())),
    ] {
        gateway::checkpoint::advance_to(
            jdir.path(),
            dir,
            &s,
            EpochFilter::OwnCaptureOnly,
            Cursor::LATEST,
        )
        .expect("advance_to");
    }
    assert_ne!(
        ckpt_files(a.path()),
        ckpt_files(b.path()),
        "имена слепков обязаны РАЗЛИЧАТЬСЯ по набору полос — иначе прогретый слепок затрёт \
         живой, и вся конструкция нулевого окна рассыпается"
    );
}
