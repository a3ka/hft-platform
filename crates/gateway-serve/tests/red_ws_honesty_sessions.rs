//! M-46 O-6/O-7 (architect, sacred) — честность истории и поведение серий на границе UTC-суток.
//!
//! Два класса дефекта, которые НЕ видны глазами на экране (цифра есть и выглядит правдоподобно):
//! - **O-6:** API молча выдаёт усечённую историю за полную. Консюмер (кокпит/AI) обязан знать,
//!   что префикс журнала спрунен (`VB-I-11`), иначе строит выводы на обрезанных данных.
//! - **O-7:** CVD и VWAP по-разному относятся к границе 00:00 UTC. CVD ОБЯЗАН сбрасываться
//!   (`M-38a`/`TD-043`), VWAP — НЕТ (`M-36`, all-time). M-37 уже давал баг ровно здесь:
//!   единая сумма через все дни вместо посессионной.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::StreamExt;
use gateway::{Selector, Snapshot};
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind, ServeConfig};
use gateway_serve::wire::ServeMsg;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};

const FUTURE: usize = 9_999_999_999;
const SECRET: &[u8] = b"m46-secret";

/// 2026-07-15T23:59:00Z — до полуночи; 2026-07-16T00:01:00Z — после. Граница UTC-суток МЕЖДУ.
const BEFORE_MIDNIGHT_MS: i64 = 1_784_159_940_000;
const AFTER_MIDNIGHT_MS: i64 = 1_784_160_060_000;

fn sign() -> String {
    let claims = Claims {
        sub: "m46".to_string(),
        exp: FUTURE,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET),
    )
    .expect("encode")
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m46".to_string(),
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
        depth_cadence_ms: None,
    }
}

fn config(dir: &std::path::Path) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: None,
    }
}

async fn ws_snapshot(dir: &std::path::Path) -> Snapshot {
    let server = bind(config(dir)).await.expect("bind");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    let token = sign();
    let url = format!("ws://{addr}/?token={token}");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");
    let m = ws.next().await.expect("msg").expect("ok");
    match serde_json::from_slice::<ServeMsg>(m.into_data().as_ref()).expect("parse") {
        ServeMsg::Snapshot(s) => s,
        other => panic!("ожидался Snapshot, получено {other:?}"),
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

// ─────────────────────────── O-7 ───────────────────────────

/// **O-7 (парный).** Через 00:00 UTC: CVD сбрасывается, VWAP — НЕТ.
///
/// Устроен так, что реализация, ресетящая ОБЕ серии, и реализация, не ресетящая НИ ОДНУ,
/// падают по-разному. Одинаковое поведение двух серий = дефект по определению.
#[tokio::test]
async fn o7_cvd_resets_at_utc_midnight_vwap_does_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        // Сессия 1 (до полуночи): сильный ПОКУПАТЕЛЬНЫЙ перекос — CVD уходит вверх
        for i in 0..3i64 {
            j.append(trade(
                65_000.0,
                2.0,
                Side::Buy,
                BEFORE_MIDNIGHT_MS + i * 1_000,
            ))
            .expect("append d1");
        }
        // Сессия 2 (после полуночи): одна маленькая ПРОДАЖА — новый CVD стартует с нуля и уходит ВНИЗ
        j.append(trade(65_000.0, 1.0, Side::Sell, AFTER_MIDNIGHT_MS))
            .expect("append d2");
        j.flush().expect("flush");
    }

    let snap = ws_snapshot(dir.path()).await;
    let s = &snap.series;

    assert!(
        !s.cumulative_delta.is_empty(),
        "O-7: CVD пуст — фикстура не давит"
    );
    assert!(!s.vwap.is_empty(), "O-7: VWAP пуст — фикстура не давит");

    // CVD: последняя точка относится ко ВТОРОЙ сессии и обязана отражать ТОЛЬКО её
    // (одна продажа 1.0) — то есть быть ОТРИЦАТЕЛЬНОЙ. Без ресета там было бы
    // +6.0 (три покупки) − 1.0 = +5.0 > 0.
    let (_, last_cvd) = *s.cumulative_delta.last().expect("есть точки CVD");
    assert!(
        last_cvd < 0,
        "O-7: CVD НЕ сбросился на границе 00:00 UTC — последняя точка {last_cvd} >= 0, \
         значит суммирование протекло из прошлых суток (регрессия класса M-37/TD-043)"
    );

    // VWAP: all-time, БЕЗ ресета — обязан оставаться положительной ценой и охватывать
    // обе сессии. Ресет обнулил бы/переопределил бы серию на границе.
    let (_, last_vwap) = *s.vwap.last().expect("есть точки VWAP");
    assert!(
        last_vwap > 0,
        "O-7: VWAP <= 0 после границы суток — похоже на ошибочный ресет all-time серии"
    );
    assert!(
        s.vwap.len() >= 2,
        "O-7: VWAP охватывает {} точек — ожидались обе сессии; серия, обрезанная границей \
         суток, означает ошибочно применённый к VWAP посессионный ресет",
        s.vwap.len()
    );
}

// ─────────────────────────── O-6 ───────────────────────────

/// **O-6(а).** На ПОЛНОМ журнале `history_truncated == false`.
/// Анти-плацебо: заглушка «всегда true» падает здесь.
#[tokio::test]
async fn o6_full_journal_is_not_truncated() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..4i64 {
            j.append(trade(
                65_000.0,
                1.0,
                Side::Buy,
                BEFORE_MIDNIGHT_MS + i * 1_000,
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }

    let snap = ws_snapshot(dir.path()).await;
    assert!(
        !snap.history_truncated,
        "O-6: полный журнал помечен усечённым — API врёт о полноте истории в СТОРОНУ ПАНИКИ"
    );
    assert_eq!(
        snap.history_start_seq, 0,
        "O-6: на полном журнале первое свёрнутое событие обязано иметь seq=0"
    );
}

/// **O-6(б) деградированный вход.** УСЕЧЁННЫЙ журнал: первый сегмент удалён (эмуляция
/// retention-prune) ⇒ `history_truncated == true` и `history_start_seq > 0`.
/// Анти-плацебо: заглушка «всегда false» падает здесь.
///
/// Это тот класс, что `VB-I-11`: консюмер, не знающий об усечении, выдаст обрезанную
/// серию за полную историю инструмента.
#[tokio::test]
async fn o6_pruned_journal_is_honestly_marked() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        // Мелкие сегменты, чтобы гарантированно получить их несколько.
        let cfg = WriterConfig {
            max_segment_bytes: 512,
            ..writer_cfg()
        };
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for i in 0..60i64 {
            j.append(trade(
                65_000.0 + i as f64,
                1.0,
                Side::Buy,
                BEFORE_MIDNIGHT_MS + i * 1_000,
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }

    // Эмуляция prune: удаляем САМЫЙ РАННИЙ сегмент — префикс истории исчезает.
    let mut segs: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("segment-") && n.ends_with(".jrnl"))
        })
        .collect();
    segs.sort();
    assert!(
        segs.len() >= 2,
        "O-6: фикстура дала {} сегмент(ов) — усечение эмулировать нечем, тест ничего не доказывает",
        segs.len()
    );
    std::fs::remove_file(&segs[0]).expect("remove earliest segment");

    let snap = ws_snapshot(dir.path()).await;
    assert!(
        snap.history_truncated,
        "O-6: у журнала удалён префикс, но history_truncated=false — \
         API выдаёт усечённую историю за полную (VB-I-11)"
    );
    assert!(
        snap.history_start_seq > 0,
        "O-6: history_start_seq={} при удалённом префиксе — провенанс истории не отражает усечение",
        snap.history_start_seq
    );
}
