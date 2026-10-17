//! RED M-08 (sacred, architect-only) — `read_all`/`recover` ОБЯЗАНЫ понимать формат v2
//! (магия + `SegmentHeader`) и ВСЕ сегменты, а не только `segment-00000000.jrnl`.
//!
//! **Мина, найденная при разборе SVR research-dev (2026-07-13):** `read_all` захардкожен на
//! одно имя файла и парсит первые 4 байта как длину фрейма. На журнале v2 он читает магию
//! `HFTJRN02` как len → CRC-fail → **молча возвращает 0 событий**. При этом `read_all` зовут:
//!   - `crates/journal/examples/dump.rs` (диагностика журнала),
//!   - `crates/book/examples/bands.rs` и `obi_probe.rs` — **тем, чем мы смотрим полосы OBI**.
//!
//! То есть после деплоя M-08 вся наша диагностика начала бы показывать «данных нет», а ни один
//! гейт бы не покраснел. Это ровно класс TD-011 (тихая деградация при зелёных тестах).
//!
//! Контракт: `read_all` — путь ДИАГНОСТИКИ/replay малых журналов (не прод-масштаб, для него
//! `stream`): он обязан пройти ВСЕ сегменты каталога по возрастанию индекса, пропустить
//! магию+заголовок и вернуть события в порядке `seq`. Legacy-сегмент (без магии) читается по
//! тем же правилам классификации, что и `stream` (декларация в манифесте).
//!
//! Анти-плацебо: текущая реализация возвращает 0 событий → тест падает.

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{Journal, WriterConfig};

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + i as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 16 * 1024, // много сегментов
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "read_all fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

const N: u64 = 1_500;

/// `read_all` на журнале v2 с ротацией: ВСЕ события, в порядке seq, из ВСЕХ сегментов.
#[test]
fn read_all_understands_v2_header_and_all_segments() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    assert!(
        journal::list_segments(dir.path()).expect("segments").len() > 1,
        "предусловие: журнал ротировался"
    );

    let evs = journal::read_all(dir.path()).expect("read_all");
    assert_eq!(
        evs.len() as u64,
        N,
        "read_all вернул {} событий вместо {N} — он не понимает магию/заголовок v2 и/или \
         читает только segment-00000000. На проде это значит: `dump`, `bands`, `obi_probe` \
         молча показывают «данных нет», а гейты зелёные (класс TD-011)",
        evs.len()
    );
    for (k, e) in evs.iter().enumerate() {
        assert_eq!(e.seq, k as u64, "порядок seq сквозной через сегменты");
    }
}

/// `recover` (толерантное чтение, offline) — те же требования к формату/сегментам.
#[test]
fn recover_understands_v2_header_and_all_segments() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let evs = journal::recover(dir.path()).expect("recover");
    assert_eq!(
        evs.len() as u64,
        N,
        "recover обязан читать v2 и все сегменты (иначе аварийное восстановление журнала \
         молча теряет данные)"
    );
}
