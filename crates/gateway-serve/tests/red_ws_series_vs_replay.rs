//! M-46 O-1/O-2 (architect, sacred) — ГЛАВНЫЕ оракулы read-path.
//!
//! Предмет: то, что `gateway-serve` отдаёт по WS, обязано поэлементно совпадать с
//! НЕЗАВИСИМЫМ реплеем того же журнала (`gateway::snapshot`). Это прямое обещание продукта
//! («каждая цифра выводится реплеем из журнала»), и до M-46 оно не проверялось НИ РАЗУ.
//!
//! **Почему этого не покрывал `smoke_ws.rs`.** Его фикстура — 4 `Trade` и ни одного
//! `L2Snapshot`/`L2Delta` (`smoke_ws.rs:43-55`) ⇒ `heatmap`/`cob`/`depth_series` там ВСЕГДА
//! пусты, и код, строящий книгу, по WS-пути не исполнялся вообще. Плюс `smoke_ws` проверяет
//! лишь `matches!(parsed, ServeMsg::Snapshot(_))` — не содержимое.
//!
//! **Статус RED/GREEN.** M-46 не добавляет функциональность серверу, а ПРОВЕРЯЕТ её. Поэтому
//! эти оракулы могут быть зелёными с первого запуска — и это РЕЗУЛЬТАТ (read-path корректен),
//! а не дефект оракула. Обязательное условие валидности — мутационный контроль
//! (`milestones/M-46-read-path-probe.md` §4): каждый ассерт обязан краснеть на искажённой
//! реализации. Негативный vantage `only_trade_fixture_leaves_book_series_empty` встроен ниже:
//! он доказывает, что фикстура M-46 реально давит там, где фикстура `smoke_ws` слепа.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use futures_util::StreamExt;
use gateway::{Cursor, Selector, Snapshot};
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind, ServeConfig};
use gateway_serve::wire::ServeMsg;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};

const FUTURE: usize = 9_999_999_999;
const SECRET: &[u8] = b"m46-secret";

/// 2026-07-15T12:00:00Z и 2026-07-16T12:00:00Z — ДВЕ UTC-сессии (граница 00:00 между ними).
const D1_NOON_MS: i64 = 1_784_116_800_000;
const D2_NOON_MS: i64 = 1_784_203_200_000;

fn sign(secret: &[u8], exp: usize) -> String {
    let claims = Claims {
        sub: "m46".to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
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

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// СМЕШАННАЯ фикстура — прямое закрытие слепоты `smoke_ws.rs`.
///
/// Чек-лист `.claude/rules/testing.md` «фикстура счастливого пути — дефект оракула»:
/// - **множественность:** несколько `Trade` в одном такте (мульти-филл);
/// - **асимметрия:** `L2Delta`, где обновляется ТОЛЬКО одна сторона (бид не трогаем) —
///   ровно тот вход, на котором M-08/TD-016 стирал живые уровни;
/// - **отсутствие:** дифф молчит о неупомянутых уровнях — они обязаны выжить;
/// - **границы:** события по обе стороны 00:00 UTC (две сессии);
/// - обе стороны книги заполнены снапшотом до диффов.
fn journal_mixed() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");

        // — сессия 1 —
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![lvl(65_000.0, 2.0), lvl(64_990.0, 3.0)],
                asks: vec![lvl(65_010.0, 1.5), lvl(65_020.0, 4.0)],
                ts_exch_ms: D1_NOON_MS,
            },
        ))
        .expect("append snap");

        // мульти-филл в одном такте: две сделки с одной меткой времени
        for (px, side) in [(65_005.0, Side::Buy), (64_995.0, Side::Sell)] {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(px),
                    size: to_fixed(1.0),
                    side,
                    ts_exch_ms: D1_NOON_MS + 1_000,
                },
            ))
            .expect("append trade");
        }

        // АСИММЕТРИЧНЫЙ дифф: меняются только аски; о бидах дифф МОЛЧИТ ⇒ бид обязан выжить
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Delta {
                bids: vec![],
                asks: vec![lvl(65_010.0, 0.5)],
                first_update_id: 1,
                final_update_id: 2,
                prev_final_update_id: None,
                ts_exch_ms: D1_NOON_MS + 2_000,
            },
        ))
        .expect("append delta");

        // — сессия 2 (через границу 00:00 UTC) —
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(66_000.0),
                size: to_fixed(2.0),
                side: Side::Buy,
                ts_exch_ms: D2_NOON_MS,
            },
        ))
        .expect("append trade d2");

        // Асимметричный дифф сессии 2: только биды. Цена ВНУТРИ спреда (65_005 между
        // best bid 65_000 и best ask 65_010) — книга остаётся НЕскрещенной.
        //
        // Замер при написании оракула: первая версия ставила сюда bid 65_990, то есть ВЫШЕ
        // асков ⇒ скрещенная книга ⇒ `mid` уезжал на ~65_500, а окно COB `mid ± max(bands)`
        // (`gateway/src/lib.rs:1136-1148`, ±0.1%) не захватывало НИ ОДНОГО уровня ⇒ `cob`
        // пуст. Это была ошибка фикстуры, а не дефект: `O-2` при этом оставался зелёным
        // (WS == реплей — пусто в обоих). Оставлено в комментарии, чтобы следующий, кто
        // увидит пустой COB, не начал чинить редьюсер.
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Delta {
                bids: vec![lvl(65_005.0, 0.8)],
                asks: vec![],
                first_update_id: 3,
                final_update_id: 4,
                prev_final_update_id: Some(2),
                ts_exch_ms: D2_NOON_MS + 1_000,
            },
        ))
        .expect("append delta d2");

        j.flush().expect("flush");
    }
    dir
}

/// Фикстура-двойник `smoke_ws.rs`: ТОЛЬКО сделки, ни одного события книги.
fn journal_trades_only() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..4i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + i as f64),
                    size: to_fixed(1.0),
                    side: Side::Buy,
                    ts_exch_ms: D1_NOON_MS + i,
                },
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }
    dir
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

/// Поднять сервер на ephemeral-порту, подключиться валидным JWT, вернуть первый `Snapshot`.
async fn ws_snapshot(dir: &std::path::Path) -> Snapshot {
    let server = bind(config(dir)).await.expect("bind");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let token = sign(SECRET, FUTURE);
    let url = format!("ws://{addr}/?token={token}");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect с валидным JWT");
    let msg = ws
        .next()
        .await
        .expect("сервер обязан прислать первое сообщение")
        .expect("сообщение читается");
    let parsed: ServeMsg =
        serde_json::from_slice(msg.into_data().as_ref()).expect("парсинг ServeMsg (JSON)");
    match parsed {
        ServeMsg::Snapshot(s) => s,
        other => panic!("первым сообщением обязан быть Snapshot, получено: {other:?}"),
    }
}

// ─────────────────────────── O-1 ───────────────────────────

/// **O-1.** На СМЕШАННОЙ фикстуре ни одна серия не пуста без причины.
///
/// Падает на заглушке `SeriesBundle::default()` (все поля пусты) и на любой реализации,
/// растерявшей часть серий по WS-пути.
#[tokio::test]
async fn o1_all_series_present_on_mixed_fixture() {
    let dir = journal_mixed();
    let snap = ws_snapshot(dir.path()).await;
    let s = &snap.series;

    assert!(
        !s.ohlcv.is_empty(),
        "O-1: ohlcv пуст — сделки в фикстуре есть"
    );
    assert!(
        !s.cumulative_delta.is_empty(),
        "O-1: cumulative_delta (CVD) пуст — знаковая агрессия в фикстуре есть"
    );
    assert!(
        !s.vwap.is_empty(),
        "O-1: vwap пуст — сделки в фикстуре есть"
    );
    assert!(
        !s.volume_profile.is_empty(),
        "O-1: volume_profile пуст — торгованные цены в фикстуре есть"
    );
    assert!(
        !s.volume_bubbles.is_empty(),
        "O-1: volume_bubbles пуст — торгованный объём в фикстуре есть"
    );
    // ↓↓↓ именно эти три СТРУКТУРНО не покрыты `smoke_ws.rs` (там нет L2-событий)
    assert!(
        !s.depth_series.is_empty(),
        "O-1: depth_series пуст, хотя в фикстуре есть L2Snapshot+L2Delta — \
         книга по WS-пути не доезжает"
    );
    assert!(
        !s.heatmap.is_empty(),
        "O-1: heatmap пуст, хотя в фикстуре есть L2Snapshot+L2Delta — \
         главная серия экрана по WS-пути не доезжает"
    );
    assert!(
        !s.cob.is_empty(),
        "O-1: cob пуст, хотя книга наполнена снапшотом — \
         DOM по WS-пути не доезжает"
    );
}

/// **O-1 парный vantage (анти-плацебо).** Доказывает, что O-1 давит: на фикстуре-двойнике
/// `smoke_ws.rs` (только `Trade`) книжные серии ПУСТЫ. Если этот тест когда-нибудь станет
/// красным, значит книга берётся не из событий журнала, а откуда-то ещё.
#[tokio::test]
async fn only_trade_fixture_leaves_book_series_empty() {
    let dir = journal_trades_only();
    let snap = ws_snapshot(dir.path()).await;
    let s = &snap.series;

    assert!(
        !s.ohlcv.is_empty(),
        "фикстура из сделок обязана давать свечи"
    );
    assert!(
        s.heatmap.is_empty() && s.cob.is_empty() && s.depth_series.is_empty(),
        "на фикстуре БЕЗ L2-событий книжные серии обязаны быть пусты — \
         именно поэтому smoke_ws.rs не может проверить heatmap/cob/depth_series"
    );
}

// ─────────────────────────── O-2 ───────────────────────────

/// **O-2 (главный).** WS-выдача поэлементно равна независимому реплею того же журнала.
///
/// Сравнение ПОПОЛЬНОЕ, отдельным ассертом на серию: иначе один `assert_eq!(a, b)` на весь
/// bundle сообщил бы «не равно», не сказав ГДЕ, и находка потеряла бы адресность.
#[tokio::test]
async fn o2_ws_series_equal_independent_replay() {
    let dir = journal_mixed();
    let ws = ws_snapshot(dir.path()).await;
    let replay = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("независимый реплей журнала");

    assert_eq!(
        ws.schema_version, replay.schema_version,
        "O-2: schema_version WS != реплей"
    );
    assert_eq!(ws.cursor, replay.cursor, "O-2: cursor WS != реплей");

    let (a, b) = (&ws.series, &replay.series);
    assert_eq!(a.ohlcv, b.ohlcv, "O-2: ohlcv WS != реплей");
    assert_eq!(
        a.cumulative_delta, b.cumulative_delta,
        "O-2: cumulative_delta (CVD) WS != реплей"
    );
    assert_eq!(
        a.cvd_session_base, b.cvd_session_base,
        "O-2: cvd_session_base WS != реплей"
    );
    assert_eq!(
        a.depth_series, b.depth_series,
        "O-2: depth_series WS != реплей"
    );
    assert_eq!(a.vwap, b.vwap, "O-2: vwap WS != реплей");
    assert_eq!(
        a.volume_profile, b.volume_profile,
        "O-2: volume_profile WS != реплей"
    );
    assert_eq!(
        a.vp_session_max_time_s, b.vp_session_max_time_s,
        "O-2: vp_session_max_time_s WS != реплей"
    );
    assert_eq!(a.heatmap, b.heatmap, "O-2: heatmap WS != реплей");
    assert_eq!(a.cob, b.cob, "O-2: cob WS != реплей");
    assert_eq!(
        a.volume_bubbles, b.volume_bubbles,
        "O-2: volume_bubbles WS != реплей"
    );

    assert_eq!(
        ws.history_start_seq, replay.history_start_seq,
        "O-2: history_start_seq WS != реплей"
    );
    assert_eq!(
        ws.history_truncated, replay.history_truncated,
        "O-2: history_truncated WS != реплей"
    );
}

/// **O-2 деградированный вход.** Пустой журнал: WS и реплей обязаны совпасть И на нём
/// (в частности, `history_truncated == false` — пусто не значит усечено).
#[tokio::test]
async fn o2_empty_journal_ws_equals_replay() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        j.flush().expect("flush");
    }

    let ws = ws_snapshot(dir.path()).await;
    let replay = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("реплей пустого журнала");

    assert_eq!(
        ws.series, replay.series,
        "O-2: пустой журнал — bundle != реплей"
    );
    assert!(
        !ws.history_truncated,
        "O-2: пустой журнал НЕ является усечённым (первое свёрнутое событие отсутствует)"
    );
}
