//! RED M-38b (sacred, architect-only) — **GW-I-9(б,в): чекпоинт это КЭШ, а не истина.**
//!
//! Прецедент — journal `JR-I-4` (`docs/fa/journal.md:87`: «снапшот — оптимизация старта, НЕ
//! истина»). Любая невалидность чекпоинта обязана давать **тихий rebuild от START** с тем же
//! результатом: без ошибки наружу, без деградации данных, без «частичного» состояния. Кокпит
//! не должен уметь отличить «кэш был» от «кэша не было» ничем, кроме скорости.
//!
//! COMPILE-RED: `gateway::checkpoint::{advance, advance_to}`, `gateway::snapshot_from_checkpoint`
//! ещё не существуют.
//!
//! **Важно (см. `red_checkpoint_byte_identity::foreign_checkpoint_changes_output`):** ВСЕ тесты
//! этого файла проходят на реализации, которая чекпоинт вообще игнорирует. Они доказывают
//! «не хуже, чем без кэша», а не «кэш работает». Доказательство работы кэша — форсинги в
//! `byte_identity` (подменный чекпоинт) и `resource_bound` (счётчик событий). Не удалять их.
//!
//! testing.md: п.4 границы (нет файла / пустой / мусор / обрезанный / чужой селектор / чужая
//! эпоха / `cursor > at`); п.6 композиция стадий (`advance` ×2 — инкрементальный чекпоинт
//! поверх существующего); п.3 отсутствие (нет ckpt-каталога — не ошибка).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
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

fn fixture() -> tempfile::TempDir {
    journal_of(vec![
        trade(100.0, 5.0, Side::Buy, D2_MS - 4_000),
        trade(100.5, 2.0, Side::Sell, D2_MS - 3_000),
        trade(101.0, 1.0, Side::Buy, D2_MS - 1_500),
        trade(101.5, 3.0, Side::Sell, D2_MS + 500),
        trade(102.0, 4.0, Side::Buy, D2_MS + 2_500),
    ])
}

fn sel_with(symbol: &str, bands: Vec<f64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: symbol.to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: Some(10_000),
        depth_cadence_ms: None,
    }
}

fn sel() -> Selector {
    sel_with("BTCUSDT", vec![0.001])
}

fn canon(s: &gateway::Snapshot) -> Vec<u8> {
    serde_json::to_vec(s).expect("сериализация")
}

fn honest(dir: &std::path::Path) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST).expect("snapshot")
}

/// Общий ассерт: что бы ни лежало в `ckpt_dir`, выход обязан совпасть с реплеем от START.
fn assert_rebuilds(case: &str, dir: &std::path::Path, ckpt: &std::path::Path) {
    let res = gateway::snapshot_from_checkpoint(
        dir,
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt,
        Cursor::LATEST,
    );
    let (got, _stats) = match res {
        Ok(v) => v,
        Err(e) => panic!(
            "GW-I-9(б) НАРУШЕН [{case}]: невалидный чекпоинт обязан вызвать ТИХИЙ rebuild, \
             а не ошибку наружу. Кэш не может ронять запрос — получено: {e:?}"
        ),
    };
    assert_eq!(
        canon(&got),
        canon(&honest(dir)),
        "GW-I-9(б) НАРУШЕН [{case}]: rebuild дал результат, отличный от snapshot(START)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// GW-I-9(б) — любая невалидность → тихий rebuild
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn missing_checkpoint_dir_rebuilds() {
    let dir = fixture();
    let ckpt = tempfile::tempdir().expect("ckpt");
    let missing = ckpt.path().join("нет-такого-каталога");
    assert_rebuilds("каталога чекпоинтов не существует", dir.path(), &missing);
}

#[test]
fn empty_checkpoint_dir_rebuilds() {
    let dir = fixture();
    let ckpt = tempfile::tempdir().expect("ckpt");
    assert_rebuilds("каталог есть, чекпоинта нет", dir.path(), ckpt.path());
}

/// Мусор и ОБРЕЗАННЫЙ файл — разные пути отказа (парсинг заголовка vs CRC/длина).
#[test]
fn corrupt_and_truncated_checkpoint_rebuild() {
    let dir = fixture();

    // (а) чистый мусор вместо чекпоинта
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    let file = ckpt_file(ckpt.path());
    std::fs::write(&file, b"NOT-A-CHECKPOINT-JUST-GARBAGE").expect("write");
    assert_rebuilds("мусор вместо чекпоинта", dir.path(), ckpt.path());

    // (б) валидный чекпоинт, обрезанный посередине (CRC/длина не сойдутся)
    let ckpt2 = tempfile::tempdir().expect("ckpt2");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt2.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let f2 = ckpt_file(ckpt2.path());
    let bytes = std::fs::read(&f2).expect("read");
    assert!(
        bytes.len() > 8,
        "чекпоинт подозрительно мал: {}",
        bytes.len()
    );
    std::fs::write(&f2, &bytes[..bytes.len() / 2]).expect("truncate");
    assert_rebuilds("обрезанный чекпоинт", dir.path(), ckpt2.path());
}

/// Единственный файл в ckpt-каталоге — чекпоинт. Если реализация раскладывает по подкаталогам
/// (ключ = selector_fingerprint), берём первый найденный файл рекурсивно.
fn ckpt_file(dir: &std::path::Path) -> std::path::PathBuf {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(d).expect("read_dir").flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out.into_iter()
        .next()
        .expect("advance обязан был создать файл чекпоинта")
}

/// Чекпоинт снят под ДРУГИМ селектором → фингерпринт не сойдётся → rebuild.
/// Второй кейс — `bands` отличаются в последнем бите мантиссы: фингерпринт обязан считаться
/// по `f64::to_bits`, а не по `Display` (иначе `0.001` и `0.001 + ε` неразличимы и чекпоинт
/// чужого конфига будет принят как свой).
#[test]
fn foreign_selector_rebuilds() {
    let dir = fixture();

    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &sel_with("ETHUSDT", vec![0.001]),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    assert_rebuilds("чекпоинт другого символа", dir.path(), ckpt.path());

    let ckpt2 = tempfile::tempdir().expect("ckpt2");
    let eps = f64::from_bits(0.001_f64.to_bits() + 1);
    assert_ne!(eps.to_bits(), 0.001_f64.to_bits());
    gateway::checkpoint::advance(
        dir.path(),
        ckpt2.path(),
        &sel_with("BTCUSDT", vec![eps]),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    assert_rebuilds(
        "bands отличаются на 1 ulp (фингерпринт обязан быть по to_bits, не по Display)",
        dir.path(),
        ckpt2.path(),
    );
}

/// Чекпоинт снят под другим `EpochFilter` → эпохи не смешиваются молча (GW-I-7 класс).
#[test]
fn foreign_epoch_filter_rebuilds() {
    let dir = fixture();
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::All)
        .expect("advance");
    assert_rebuilds(
        "чекпоинт снят под EpochFilter::All",
        dir.path(),
        ckpt.path(),
    );
}

/// Просят снапшот РАНЬШЕ чекпоинта (`ckpt.cursor > at`) — «отмотать назад» нельзя,
/// состояние не обратимо. Обязан быть rebuild от START до `at`, а не выдача более позднего
/// состояния (это была бы ложь о курсоре — кокпит увидел бы будущее).
#[test]
fn checkpoint_ahead_of_requested_cursor_rebuilds() {
    let dir = fixture();
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance до LATEST");

    let at = Cursor { upto_seq: Some(1) };
    let (got, _stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        at,
    )
    .expect("не ошибка, а rebuild");
    let want = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), at)
        .expect("snapshot(START, at)");
    assert_eq!(
        canon(&got),
        canon(&want),
        "GW-I-9(б): при ckpt.cursor > at обязан быть rebuild до at. Выдача состояния ПОЗЖЕ \
         запрошенного курсора = кокпит видит будущее (нарушение курсор-контракта GW-I-8)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// GW-I-9(в) — идемпотентность и композиция стадий (п.6)
// ─────────────────────────────────────────────────────────────────────────────

/// Два `advance` без новых событий → байт-идентичный файл. Недетерминизм в чекпоинте
/// (wall-clock в заголовке, итерация по неупорядоченной коллекции) вылезет здесь.
#[test]
fn advance_is_idempotent_bytewise() {
    let dir = fixture();
    let ckpt = tempfile::tempdir().expect("ckpt");

    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance #1");
    let a = std::fs::read(ckpt_file(ckpt.path())).expect("read #1");

    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance #2");
    let b = std::fs::read(ckpt_file(ckpt.path())).expect("read #2");

    assert_eq!(
        a, b,
        "GW-I-9(в) НАРУШЕН: два advance без новых событий дали РАЗНЫЕ байты. Источник — \
         недетерминизм в чекпоинте (wall-clock/итерация по HashMap). DET-I-1 sacred."
    );
}

/// Композиция стадий (testing.md п.6): инкрементальный `advance` поверх существующего чекпоинта
/// обязан дать то же, что один `advance` сразу до конца. Cron вызывает `advance` многократно —
/// именно эта композиция и работает в проде, а не одиночная стадия.
#[test]
fn incremental_advance_equals_single_advance() {
    let dir = fixture();

    let step = tempfile::tempdir().expect("ckpt step");
    for k in [1_u64, 2, 4] {
        gateway::checkpoint::advance_to(
            dir.path(),
            step.path(),
            &sel(),
            EpochFilter::OwnCaptureOnly,
            Cursor { upto_seq: Some(k) },
        )
        .expect("advance_to по шагам");
    }

    let once = tempfile::tempdir().expect("ckpt once");
    gateway::checkpoint::advance_to(
        dir.path(),
        once.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        Cursor { upto_seq: Some(4) },
    )
    .expect("advance_to сразу");

    assert_eq!(
        std::fs::read(ckpt_file(step.path())).expect("read step"),
        std::fs::read(ckpt_file(once.path())).expect("read once"),
        "GW-I-9(в) НАРУШЕН: чекпоинт, доведённый до курсора ПО ШАГАМ (как это делает cron), \
         отличается от снятого одним вызовом. Стадия зелена в изоляции, композиция — нет."
    );
}
