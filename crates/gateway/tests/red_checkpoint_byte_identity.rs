//! RED M-38b (sacred, architect-only) — **GW-I-9: чекпоинт-редьюсер даёт БАЙТ-ИДЕНТИЧНЫЙ
//! результат реплею от START, и при этом РЕАЛЬНО ЧИТАЕТСЯ.**
//!
//! TD-044: `gateway::snapshot` реплеит журнал от `Cursor::START` на КАЖДОЕ подключение — прод-замер
//! первого `Snapshot` **409.74 s** при >21 GiB прочитанного. Лечение — чекпоинт полного состояния
//! `Reducer`, от которого снапшот досчитывается хвостом. Цена ошибки — тихое расхождение данных
//! кокпита (DET-I-1), поэтому инвариант байтовый, а не «примерно тот же».
//!
//! COMPILE-RED: `gateway::checkpoint::advance_to`, `gateway::snapshot_from_checkpoint`,
//! `gateway::ReadStats` ещё не существуют.
//!
//! ## Почему одной байт-идентичности НЕДОСТАТОЧНО (читать перед правкой этого файла)
//!
//! Реализация, которая чекпоинт ИГНОРИРУЕТ и всегда реплеит от START, проходит **все** тесты
//! байт-идентичности и **все** тесты инвалидации — и является самым вероятным «зелёным» исходом
//! (класс «идеальная фикстура», пойманный ЧЕТЫРЕ раза подряд: M-07, M-08, TD-042, TD-045).
//! Форсингов ровно два, оба обязаны жить в наборе:
//!   1. `foreign_checkpoint_changes_output` (здесь) — подменный, но ПОЛНОСТЬЮ ВАЛИДНЫЙ чекпоинт
//!      обязан изменить выход;
//!   2. `red_checkpoint_resource_bound` (отдельный файл) — счётчик декодированных событий.
//! Убрать любой из них = снять доказательство того, что чекпоинт вообще используется.
//!
//! ## testing.md чек-лист
//! - п.1 **асимметрия** — односторонние `L2Delta` (меняется только bid), сделки только по одну
//!   сторону границы сессии;
//! - п.2 **множественность** — 2+ сделки в одном бакете, 2+ бакета в сессии;
//! - п.3 **отсутствие** — дельта, не упоминающая уровень, не должна его стирать (класс TD-016);
//! - п.4 **границы** — K=0 (пустой чекпоинт), K=at (досчитывать нечего), K на последнем seq,
//!   K в середине бакета, K между `L2Snapshot` и `L2Delta`, K перед 00:00 UTC при `at` после;
//! - п.5 прод-масштаб — в `red_checkpoint_resource_bound` (там граница ресурса);
//! - п.6 **композиция стадий** — `advance_to` ×2 (инкрементальный чекпоинт) в
//!   `red_checkpoint_is_cache`; здесь — композиция «чекпоинт → досчёт хвостом»;
//! - п.7 **ПАРНЫЙ vantage** — байт-идентичность (чекпоинт не искажает) И
//!   `foreign_checkpoint_changes_output` (чекпоинт не игнорируется). Одно без другого
//!   удовлетворяется заглушкой.
//!
//! **Позиции K не выбираются вручную** — `identical_for_every_k` перебирает ВСЕ `K in 0..=n`.
//! Фикстура построена так, чтобы каждая деградированная позиция из списка выше в этот перебор
//! попадала по построению (см. `rich_journal`). Точечные тесты ниже — для читаемости отчёта.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
/// Граница UTC-суток (та же, что в оракулах M-38a/TD-046).
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

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

fn l2snap(bids: Vec<Level>, asks: Vec<Level>, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids,
            asks,
            ts_exch_ms: ts,
        },
    )
}

fn l2delta(bids: Vec<Level>, asks: Vec<Level>, ts: i64, u: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids,
            asks,
            first_update_id: u,
            final_update_id: u,
            prev_final_update_id: Some(u.saturating_sub(1)),
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

fn sel(window_ms: Option<i64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms,
    }
}

/// Фикстура, содержащая КАЖДУЮ деградированную позицию из чек-листа, чтобы перебор `K in 0..=n`
/// накрывал их по построению:
/// - сделки ДО и ПОСЛЕ 00:00 UTC (CVD per-session ledger + VP whole-session drop);
/// - 2+ сделки в ОДНОМ бакете (множественность) и 2+ бакета в каждой сессии;
/// - `L2Snapshot`, затем `L2Delta` — есть K строго МЕЖДУ ними;
/// - **односторонние** дельты (меняется только bid) — после них нет двусторонних обновлений
///   книги, значит `HeatmapBucketState.mid` восстановим ТОЛЬКО из чекпоинта (path-dependent кэш);
/// - дельта, не упоминающая уровень (п.3 «отсутствие»): ask не трогается и обязан выжить;
/// - события в середине бакета (ts не кратен `timeframe_ms`).
fn rich_journal() -> tempfile::TempDir {
    journal_of(vec![
        // S1 (до полуночи): книга + сделки, 2 сделки в одном бакете.
        l2snap(
            vec![lvl(100.0, 2.0), lvl(99.0, 3.0)],
            vec![lvl(101.0, 2.0), lvl(102.0, 1.0)],
            D2_MS - 9_500,
        ),
        trade(100.5, 5.0, Side::Buy, D2_MS - 9_400),
        trade(100.5, 2.0, Side::Buy, D2_MS - 9_100), // тот же бакет
        // Двусторонняя дельта — mid пересчитывается.
        l2delta(
            vec![lvl(100.0, 4.0)],
            vec![lvl(101.0, 5.0)],
            D2_MS - 8_500,
            10,
        ),
        trade(99.5, 1.0, Side::Sell, D2_MS - 7_200),
        // ОДНОСТОРОННЯЯ дельта: только bid. ask не упомянут — обязан выжить (п.3).
        l2delta(vec![lvl(99.0, 7.0)], vec![], D2_MS - 6_300, 11),
        trade(100.0, 3.0, Side::Buy, D2_MS - 5_100),
        // Ещё одна односторонняя — после неё двусторонних обновлений НЕТ до конца журнала,
        // поэтому `mid` живёт только в кэше бакета.
        l2delta(vec![lvl(98.0, 1.0)], vec![], D2_MS - 2_400, 12),
        trade(100.0, 4.0, Side::Sell, D2_MS - 1_100),
        // ── 00:00 UTC ──
        trade(101.0, 6.0, Side::Sell, D2_MS + 800),
        trade(101.0, 1.0, Side::Sell, D2_MS + 900), // тот же бакет
        l2delta(vec![lvl(99.5, 2.0)], vec![], D2_MS + 1_700, 13),
        trade(102.0, 2.0, Side::Buy, D2_MS + 3_300),
        trade(102.0, 8.0, Side::Buy, D2_MS + 6_600),
    ])
}

/// Каноническая байтовая форма снапшота. Структурное `==` дополняется байтовым сравнением:
/// расхождение, невидимое в `PartialEq` (порядок ключей коллекций при сериализации), обязано
/// быть поймано — это и есть DET-I-1 «бит-идентично».
fn canon(s: &gateway::Snapshot) -> Vec<u8> {
    serde_json::to_vec(s).expect("сериализация снапшота")
}

fn from_start(dir: &std::path::Path, w: Option<i64>, at: Cursor) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(w), at).expect("snapshot от START")
}

/// Снять чекпоинт на `k` и построить снапшот на `at` через него.
fn via_ckpt(
    dir: &std::path::Path,
    ckpt: &std::path::Path,
    w: Option<i64>,
    k: Cursor,
    at: Cursor,
) -> gateway::Snapshot {
    let s = sel(w);
    gateway::checkpoint::advance_to(dir, ckpt, &s, EpochFilter::OwnCaptureOnly, k)
        .expect("advance_to");
    let (snap, _stats) =
        gateway::snapshot_from_checkpoint(dir, EpochFilter::OwnCaptureOnly, &s, ckpt, at)
            .expect("snapshot_from_checkpoint");
    snap
}

fn n_events(dir: &std::path::Path) -> u64 {
    journal::stream(dir, EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .count() as u64
}

// ─────────────────────────────────────────────────────────────────────────────
// GW-I-9(а) — байт-идентичность на ВСЕХ K, а не на выбранных руками
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn identical_for_every_k_unbounded() {
    let dir = rich_journal();
    let n = n_events(dir.path());
    let want = from_start(dir.path(), None, Cursor::LATEST);
    let want_b = canon(&want);

    for k in 0..n {
        let ckpt = tempfile::tempdir().expect("ckpt dir");
        let got = via_ckpt(
            dir.path(),
            ckpt.path(),
            None,
            Cursor { upto_seq: Some(k) },
            Cursor::LATEST,
        );
        assert_eq!(
            canon(&got),
            want_b,
            "GW-I-9 НАРУШЕН при K={k}: snapshot_from_checkpoint ≢ snapshot(START). \
             Расхождение данных кокпита — DET-I-1. Диф: got={got:?}"
        );
    }
}

/// То же под АКТИВНЫМ окном: эвикция срабатывает и ДО, и ПОСЛЕ K — состояние, попавшее в
/// чекпоинт, уже урезано окном, и досчёт хвостом обязан прийти к тому же результату.
/// Окно 6 с при журнале ~16 с ⇒ эвикция гарантированно происходит по обе стороны любого K.
#[test]
fn identical_for_every_k_windowed() {
    let dir = rich_journal();
    let n = n_events(dir.path());
    let w = Some(6_000);
    let want_b = canon(&from_start(dir.path(), w, Cursor::LATEST));

    for k in 0..n {
        let ckpt = tempfile::tempdir().expect("ckpt dir");
        let got = via_ckpt(
            dir.path(),
            ckpt.path(),
            w,
            Cursor { upto_seq: Some(k) },
            Cursor::LATEST,
        );
        assert_eq!(
            canon(&got),
            want_b,
            "GW-I-9 НАРУШЕН под окном при K={k}: чекпоинт хранит уже-эвиктнутое состояние, \
             досчёт хвостом обязан дать тот же результат (VB-I-10 + VB-I-2)"
        );
    }
}

/// K ПЕРЕД 00:00 UTC, `at` ПОСЛЕ: чекпоинт обязан нести per-session CVD ledger
/// (`cvd: BTreeMap<session_id, CvdSession>`) и `session_max_time_s` (форма M-38a). Реализация,
/// сохранившая скалярную базу M-37, склеит сессии — падение здесь.
#[test]
fn checkpoint_before_midnight_carries_session_state() {
    let dir = rich_journal();
    let ckpt = tempfile::tempdir().expect("ckpt dir");
    // seq 8 — последняя сделка ДО полуночи; `at` — весь журнал (обе сессии).
    let got = via_ckpt(
        dir.path(),
        ckpt.path(),
        Some(6_000),
        Cursor { upto_seq: Some(8) },
        Cursor::LATEST,
    );
    let want = from_start(dir.path(), Some(6_000), Cursor::LATEST);
    assert_eq!(
        canon(&got),
        canon(&want),
        "GW-I-9/M-38a: чекпоинт, снятый ДО 00:00 UTC, обязан нести per-session CVD ledger и \
         session_max_time_s — иначе сессии склеиваются или VP-сессия теряется. \
         got.cvd_session_base={:?} want.cvd_session_base={:?}",
        got.series.cvd_session_base,
        want.series.cvd_session_base
    );
}

/// Границы (п.4): K=0 — чекпоинт пуст; K=at — досчитывать нечего.
#[test]
fn boundary_k_zero_and_k_equals_at() {
    let dir = rich_journal();
    let n = n_events(dir.path());
    let at = Cursor {
        upto_seq: Some(n - 1),
    };
    let want_b = canon(&from_start(dir.path(), None, at));

    for (name, k) in [
        ("K=0 (пустой чекпоинт)", Cursor { upto_seq: Some(0) }),
        ("K=at (хвоста нет)", at),
    ] {
        let ckpt = tempfile::tempdir().expect("ckpt dir");
        assert_eq!(
            canon(&via_ckpt(dir.path(), ckpt.path(), None, k, at)),
            want_b,
            "GW-I-9 НАРУШЕН на границе {name}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GW-I-9(г) — ФОРСИНГ: чекпоинт обязан РЕАЛЬНО ЧИТАТЬСЯ
// ─────────────────────────────────────────────────────────────────────────────

/// Подменный чекпоинт: снят с ДРУГОГО журнала, но проходит ВСЮ валидацию — тот же селектор,
/// та же схема, тот же `journal_lineage` (он считается по заголовкам сегментов —
/// `(index, epoch_id, source, first_seq)` — а не по содержимому; оба журнала: один сегмент,
/// index 0, epoch `own-test`, `first_seq` 0).
///
/// **Это ЕДИНСТВЕННЫЙ тест, который отличает «чекпоинт используется» от «чекпоинт игнорируется
/// и всё реплеится от START».** Вторая реализация проходит все остальные тесты этого файла.
///
/// Байт-флип с пересчётом CRC для этой цели НЕ годится: испорченный postcard, скорее всего, не
/// десериализуется → штатный тихий rebuild (GW-I-9б) → тест позеленел бы на неправильной
/// реализации.
///
/// Остаточный риск назван в milestone'е явно: `journal_lineage` — НЕ контентный хэш, чекпоинт
/// доверяется в пределах фингерпринт-конверта. Контентный хэш означал бы перечитывание всего
/// журнала, т.е. ровно то, что M-38b устраняет.
#[test]
fn foreign_checkpoint_changes_output() {
    let real = rich_journal();
    // Другой журнал с ТЕМ ЖЕ заголовком сегмента, но другими сделками.
    let other = journal_of(vec![
        l2snap(vec![lvl(50.0, 1.0)], vec![lvl(51.0, 1.0)], D2_MS - 9_500),
        trade(50.0, 99.0, Side::Sell, D2_MS - 9_400),
        trade(50.0, 77.0, Side::Sell, D2_MS - 9_100),
    ]);

    let s = sel(None);
    let ckpt = tempfile::tempdir().expect("ckpt dir");
    // Чекпоинт снят с ЧУЖОГО журнала на позиции, лежащей и в реальном журнале.
    gateway::checkpoint::advance_to(
        other.path(),
        ckpt.path(),
        &s,
        EpochFilter::OwnCaptureOnly,
        Cursor { upto_seq: Some(2) },
    )
    .expect("advance_to на чужом журнале");

    let (got, _stats) = gateway::snapshot_from_checkpoint(
        real.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint");

    let honest = from_start(real.path(), None, Cursor::LATEST);
    assert_ne!(
        canon(&got),
        canon(&honest),
        "ФОРСИНГ ПРОВАЛЕН: подменный чекпоинт (валидный по всем фингерпринтам, но снятый с \
         другого журнала) НЕ изменил выход. Значит чекпоинт не читается — реализация тихо \
         реплеит от START, и вся байт-идентичность выше ничего не доказывает. \
         Это ровно тот «зелёный», который milestone обязан не пропустить."
    );
}
