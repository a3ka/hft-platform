//! RED `PL-I-5` — **ГРАНИЦА ПРЕДЕЛА: предел и предел+1** (`C-157` R1, требование «boundary
//! cases at limit / limit+1»).
//!
//! # Отдельным файлом — НАМЕРЕННО
//!
//! Это **COMPILE-RED**: оракул ссылается на `gateway::DEFAULT_MAX_RESPONSE_BYTES`, которого
//! ещё нет. Оставленный в общем наборе, он ронял бы КОМПИЛЯЦИЮ всего бинаря, и остальные
//! оракулы `red_egress_cap.rs` нельзя было бы ПРЕДЪЯВИТЬ красными: «не собралось» и «упало на
//! ассерте» — разные вещи, а RED-first требует второго. Цена смешения уже плачена на `M-68`
//! (`C-138` п.3).
//!
//! # Почему граница проверяется НАБЛЮДАЕМО, а не подгонкой байтов
//!
//! Построить ответ РОВНО в `limit` байт нельзя: размер дискретен и зависит от сериализации.
//! Поэтому граница берётся так, как её видит клиент:
//!
//! 1. растим нагрузку, пока ответ обслуживается;
//! 2. **последний ПРИНЯТЫЙ** ответ обязан весить `<= limit` — иначе предел не предел;
//! 3. **следующий шаг** обязан быть ОТВЕРГНУТ — иначе предел не срабатывает на границе.
//!
//! Размер отвергнутого не наблюдаем и наблюдаться не должен: его не построили, в этом и смысл.
//!
//! Шаг нагрузки — ОДНА сделка (≈ десятки байт), поэтому «предел+1» здесь буквален с точностью
//! до одной выдаваемой сущности, а не «на порядок больше».

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "PL-I-5 boundary fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
    }
}

/// Журнал из `n` сделок с РАЗНЫМИ ценами: каждая добавляет bin профиля объёма и пузырь, то
/// есть растит ответ мелким предсказуемым шагом. Ни одного L2-события — heatmap пуст, и
/// граница проверяется на той части ответа, которую первая редакция не видела вовсе.
fn journal_of_trades(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..n as i64 {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID + i as f64 * 0.01),
                size: to_fixed(1.0),
                side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                ts_exch_ms: T0 + i,
            },
        ))
        .expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn try_snapshot(dir: &std::path::Path) -> std::io::Result<gateway::Snapshot> {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST)
}

/// **Граница: последний принятый ответ ≤ предела, следующий — отвергнут.**
#[test]
fn pl_i_5_boundary_last_accepted_fits_and_next_is_refused() {
    let limit = gateway::DEFAULT_MAX_RESPONSE_BYTES;
    assert!(
        limit > 0,
        "PL-I-5 SETUP: предел объявлен нулевым — сервис не может отдать ни одного ответа"
    );

    // Грубый поиск верхней границы: удваиваем, пока не отвергнут.
    let mut hi = 1_usize;
    while try_snapshot(journal_of_trades(hi).path()).is_ok() {
        hi *= 2;
        assert!(
            hi <= 4_000_000,
            "PL-I-5 SETUP НЕ СОСТОЯЛСЯ: {hi} сделок обслужены без отказа при пределе {limit} Б \\
             — предела нет либо он недостижимо велик, и границу проверять не на чем"
        );
    }
    let mut lo = hi / 2; // последний ЗАВЕДОМО принятый

    // Уточнение до ОДНОЙ сделки: `lo` принят, `lo + 1` отвергнут.
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if try_snapshot(journal_of_trades(mid).path()).is_ok() {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let accepted = try_snapshot(journal_of_trades(lo).path())
        .expect("PL-I-5: `lo` найден как принятый — он обязан приниматься и при перепроверке");
    let bytes = serde_json::to_vec(&accepted).expect("сериализуем").len();

    assert!(
        bytes <= limit,
        "PL-I-5 ГРАНИЦА: последний ПРИНЯТЫЙ ответ весит {bytes} Б при пределе {limit} Б. \\
         Предел, пропускающий ответ БОЛЬШЕ себя, не ограничивает ничего — он лишь называется \\
         пределом. ({lo} сделок)"
    );
    assert!(
        try_snapshot(journal_of_trades(lo + 1).path()).is_err(),
        "PL-I-5 ГРАНИЦА: {} сделок отвергнуты, а {} — приняты; порядок нарушен, и граница \\
         не воспроизводима",
        hi,
        lo + 1
    );
}
