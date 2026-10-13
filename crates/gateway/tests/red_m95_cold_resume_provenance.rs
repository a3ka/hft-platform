//! RED `M-95` (`R-249` Н-4, `C-292`; sacred, architect-only) — **холодный `LiveReducer::resume`
//! (без слепка) объявляет провенанс истории по ПЕРВОМУ РЕАЛЬНО СВЁРНУТОМУ событию, а не по
//! `header.first_seq` каталога** (`VB-I-11`: «`history_start_seq` — seq первого РЕАЛЬНО свёрнутого
//! события — НЕ `header.first_seq`, который у legacy синтезирован нулём, TD-030»).
//!
//! # Почему
//!
//! M-95 перевёл холодный путь `resume` на провенанс из каталога (`header.first_seq`), а на `Err`
//! построения каталога — `(0, false)`. У сегмента СТАРОГО формата (без заголовка) `first_seq`
//! синтезирован нулём: журнал, у которого префикс удалён и первое видимое событие — `seq = 100`,
//! объявлялся бы «история полная с 0». `red_ws_honesty_sessions` `o6` этот путь не исполняет
//! (`C-292`: мутант «безусловно `(0, false)` на холодном пути» его не роняет).
//!
//! # Как судится
//!
//! Прод-API `gateway::LiveReducer::resume` с ПУСТЫМ каталогом слепков (холодный путь), затем
//! `snapshot()`. Журнал — ОДИН задекларированный сегмент старого формата (байт-в-байт как боевой
//! `segment-00000000.jrnl`, `red_segments_epochs.rs`), события с `seq = FIRST..FIRST+N`.
//!
//! * `c1` — `FIRST = 100`: `history_start_seq == 100`, `history_truncated == true`.
//! * `c0` — парный мир, `FIRST = 0`: `history_start_seq == 0`, `history_truncated == false` —
//!   иначе прошла бы реализация «всегда усечено».
//!
//! RUNTIME-RED: только существующие публичные символы (урок `A-033`).

use std::io::Write;

use contracts::{DataSource, EventKind, LegacyManifest, LegacySegmentDecl, MdPayload, Side, Venue};
use gateway::{LiveReducer, Selector};
use journal::EpochFilter;

const N: u64 = 500;
const EPOCH: &str = "own-legacy";

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + (i % 20) as i64,
            size: contracts::to_fixed(0.5),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_784_116_800_000 + i as i64 * 100,
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1000,
        bands: vec![0.015],
        window_ms: None,
        depth_cadence_ms: None,
    }
}

/// Сегмент СТАРОГО формата (без магии и заголовка) с событиями `first..first+N`, задекларированный
/// в манифесте как собственный захват — иначе `OwnCaptureOnly` его не видит.
fn legacy_journal(first: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = "segment-00000000.jrnl";
    let path = dir.path().join(file);
    {
        let mut w = std::io::BufWriter::new(std::fs::File::create(&path).expect("create"));
        for seq in first..first + N {
            let ev = contracts::Event {
                seq,
                ts_mono_ns: seq,
                ts_wall_ms: 1_784_116_800_000 + seq as i64 * 100,
                kind: trade(seq),
            };
            let p = postcard::to_stdvec(&ev).expect("ser");
            w.write_all(&(p.len() as u32).to_le_bytes()).unwrap();
            w.write_all(&p).unwrap();
            w.write_all(&crc32fast::hash(&p).to_le_bytes()).unwrap();
        }
        w.flush().unwrap();
    }
    std::fs::write(dir.path().join("journal.meta"), (first + N).to_le_bytes()).expect("meta");
    let fp = journal::fingerprint(&path).expect("fingerprint");
    let size = std::fs::metadata(&path).expect("meta").len();
    let m = LegacyManifest {
        declarations: vec![LegacySegmentDecl {
            file_name: file.to_string(),
            fingerprint_sha256: fp,
            size_bytes_at_decl: size,
            source: DataSource::OwnCapture,
            provenance: "M-95 cold resume fixture".to_string(),
            epoch_id: EPOCH.to_string(),
        }],
    };
    std::fs::write(
        dir.path().join(journal::LEGACY_MANIFEST),
        serde_json::to_vec_pretty(&m).expect("ser"),
    )
    .expect("manifest");
    // Setup-страж: журнал виден фильтром и начинается ровно с `first` — иначе мир меряет не то.
    let seen: Vec<u64> = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event").seq)
        .collect();
    assert!(
        seen.first() == Some(&first) && seen.len() as u64 == N,
        "SETUP НЕ СОСТОЯЛСЯ: задекларированный legacy-сегмент виден как {} событий с {:?}, ожидалось \
         {N} с {first}",
        seen.len(),
        seen.first()
    );
    dir
}

fn cold_snapshot(dir: &std::path::Path) -> gateway::Snapshot {
    let ckpt = tempfile::tempdir().expect("ckpt");
    let (live, stats) =
        LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &sel(), ckpt.path()).expect("resume");
    // Setup-страж: путь действительно ХОЛОДНЫЙ — слепка нет, журнал свёрнут целиком.
    assert!(
        stats.events_scanned >= N,
        "SETUP НЕ СОСТОЯЛСЯ: resume свернул {} событий из {N} — это не холодный путь",
        stats.events_scanned
    );
    live.snapshot()
}

/// `c1` — префикс удалён, сегмент старого формата: провенанс — по первому свёрнутому событию.
#[test]
fn c1_cold_resume_legacy_pruned_prefix_declares_real_first_seq() {
    let dir = legacy_journal(100);
    let s = cold_snapshot(dir.path());
    assert!(
        s.history_start_seq == 100 && s.history_truncated,
        "M-95 / R-249 Н-4 / C-292 / VB-I-11: холодный resume на журнале, первое видимое событие \
         которого seq=100 (сегмент старого формата, header.first_seq синтезирован 0), объявил \
         history_start_seq={}, history_truncated={} — ожидалось 100 и true. Провенанс холодного \
         пути — по первому РЕАЛЬНО свёрнутому событию, не по header.first_seq каталога",
        s.history_start_seq,
        s.history_truncated
    );
}

/// `c0` — парный мир: журнал с seq=0 — история полная (против «всегда усечено»).
#[test]
fn c0_cold_resume_full_history_is_complete() {
    let dir = legacy_journal(0);
    let s = cold_snapshot(dir.path());
    assert!(
        s.history_start_seq == 0 && !s.history_truncated,
        "M-95 / VB-I-11: холодный resume на полном журнале объявил history_start_seq={}, \
         history_truncated={} — ожидалось 0 и false",
        s.history_start_seq,
        s.history_truncated
    );
}
