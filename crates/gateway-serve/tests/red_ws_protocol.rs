//! M-46 O-3/O-4/O-5 (architect, sacred) — протокол WS: кадры, авторизация, окно и чекпоинт.
//!
//! Дыры `smoke_ws.rs`, которые здесь закрываются (см. `docs/plans/gateway-ws-contract.md` §6):
//! - **push-цикл не проверялся вообще** — второй `next()` не вызывался, ни одного `Frame`;
//! - из пяти веток отказа авторизации по WS проверялась **ОДНА** (чужой ключ);
//! - `window_ms: None` и `checkpoint_dir: None` ⇒ bounded-окно и чекпоинт по WS-пути слепы.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use futures_util::StreamExt;
use gateway::{Cursor, Selector, Snapshot};
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind, ServeConfig};
use gateway_serve::wire::ServeMsg;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};

const FUTURE: usize = 9_999_999_999;
const PAST: usize = 1_000_000_000; // 2001-09-09 — заведомо истёкший
const SECRET: &[u8] = b"m46-secret";
const BASE_MS: i64 = 1_784_116_800_000;

fn sign_with(secret: &[u8], exp: usize) -> String {
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

/// Журнал с книгой и сделками; `extra_trades` дописываются ПОСЛЕ (для push-цикла).
fn journal_seed(dir: &std::path::Path) {
    let mut j = Journal::open_with(dir, writer_cfg()).expect("open_with");
    j.append(EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: vec![lvl(65_000.0, 2.0)],
            asks: vec![lvl(65_010.0, 1.5)],
            ts_exch_ms: BASE_MS,
        },
    ))
    .expect("append snap");
    j.append(EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(65_005.0),
            size: to_fixed(1.0),
            side: Side::Buy,
            ts_exch_ms: BASE_MS + 1_000,
        },
    ))
    .expect("append trade");
    j.flush().expect("flush");
}

fn append_more(dir: &std::path::Path, n: i64, from_ms: i64) {
    let mut j = Journal::open_with(dir, writer_cfg()).expect("reopen");
    for i in 0..n {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(65_100.0 + i as f64),
                size: to_fixed(0.5),
                side: Side::Sell,
                ts_exch_ms: from_ms + i * 1_000,
            },
        ))
        .expect("append more");
    }
    j.flush().expect("flush");
}

fn sel(window_ms: Option<i64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms,
        depth_cadence_ms: None,
    }
}

fn config(
    dir: &std::path::Path,
    window_ms: Option<i64>,
    ckpt: Option<&std::path::Path>,
) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(window_ms),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: ckpt.map(|p| p.to_path_buf()),
    }
}

/// Подключиться и вернуть поток; сервер живёт в фоне.
async fn connect(
    cfg: ServeConfig,
    url_token: Option<&str>,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    (),
> {
    let server = bind(cfg).await.expect("bind");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    let url = match url_token {
        Some(t) => format!("ws://{addr}/?token={t}"),
        None => format!("ws://{addr}/"),
    };
    match tokio_tungstenite::connect_async(url).await {
        Ok((ws, _)) => Ok(ws),
        Err(_) => Err(()),
    }
}

/// Пришёл ли `Snapshot` первым сообщением. `false` = отказ в любой форме.
async fn got_snapshot(cfg: ServeConfig, url_token: Option<&str>) -> bool {
    let mut ws = match connect(cfg, url_token).await {
        Ok(ws) => ws,
        Err(()) => return false, // отказ на хендшейке
    };
    match ws.next().await {
        None | Some(Err(_)) => false,
        Some(Ok(m)) => serde_json::from_slice::<ServeMsg>(m.into_data().as_ref())
            .map(|sm| matches!(sm, ServeMsg::Snapshot(_)))
            .unwrap_or(false),
    }
}

// ─────────────────────────── O-4 ───────────────────────────

/// **O-4.** Матрица отказов авторизации — ВСЕ пять веток (`gateway-serve/src/lib.rs:287-318`).
/// До M-46 по WS проверялась одна (чужой ключ, `smoke_ws.rs:111`).
#[tokio::test]
async fn o4_auth_matrix_fail_closed() {
    // (а) валидный — контрольная точка: путь вообще работает
    {
        let dir = tempfile::tempdir().expect("tempdir");
        journal_seed(dir.path());
        let ok = got_snapshot(
            config(dir.path(), None, None),
            Some(&sign_with(SECRET, FUTURE)),
        )
        .await;
        assert!(ok, "O-4(контроль): валидный JWT обязан получать Snapshot");
    }
    // (б) query вообще отсутствует → "missing token query"
    {
        let dir = tempfile::tempdir().expect("tempdir");
        journal_seed(dir.path());
        let got = got_snapshot(config(dir.path(), None, None), None).await;
        assert!(!got, "O-4: без query-строки Snapshot выдаваться НЕ должен");
    }
    // (в) `?token=` пустой → "missing token"
    {
        let dir = tempfile::tempdir().expect("tempdir");
        journal_seed(dir.path());
        let got = got_snapshot(config(dir.path(), None, None), Some("")).await;
        assert!(!got, "O-4: пустой token Snapshot выдаваться НЕ должен");
    }
    // (г) истёкший exp → "expired token"
    {
        let dir = tempfile::tempdir().expect("tempdir");
        journal_seed(dir.path());
        let got = got_snapshot(
            config(dir.path(), None, None),
            Some(&sign_with(SECRET, PAST)),
        )
        .await;
        assert!(!got, "O-4: истёкший JWT Snapshot выдаваться НЕ должен");
    }
    // (д) чужой ключ → "invalid token"
    {
        let dir = tempfile::tempdir().expect("tempdir");
        journal_seed(dir.path());
        let got = got_snapshot(
            config(dir.path(), None, None),
            Some(&sign_with(b"attacker", FUTURE)),
        )
        .await;
        assert!(!got, "O-4: JWT чужим ключом Snapshot выдаваться НЕ должен");
    }
    // (е) мусор вместо JWT → "invalid token"
    {
        let dir = tempfile::tempdir().expect("tempdir");
        journal_seed(dir.path());
        let got = got_snapshot(config(dir.path(), None, None), Some("не-жвт-вовсе")).await;
        assert!(!got, "O-4: malformed token Snapshot выдаваться НЕ должен");
    }
}

// ─────────────────────────── O-3 ───────────────────────────

/// **O-3.** Push-цикл: `snapshot(C)` + применённые `Frame`ы ≡ `snapshot(LATEST)`.
///
/// Это ядро live-режима: клиент получает снапшот один раз, дальше живёт кадрами. Если
/// сходимость нарушена, экран «уезжает» от реальности тем сильнее, чем дольше открыт.
/// До M-46 ни один тест не читал по WS даже ОДИН кадр.
#[tokio::test]
async fn o3_frames_converge_to_latest() {
    let dir = tempfile::tempdir().expect("tempdir");
    journal_seed(dir.path());

    let mut ws = connect(
        config(dir.path(), None, None),
        Some(&sign_with(SECRET, FUTURE)),
    )
    .await
    .expect("connect");

    // (1) снапшот-при-подключении
    let first = ws.next().await.expect("msg").expect("ok");
    let mut acc: Snapshot =
        match serde_json::from_slice::<ServeMsg>(first.into_data().as_ref()).expect("parse") {
            ServeMsg::Snapshot(s) => s,
            other => panic!("первым обязан быть Snapshot, получено {other:?}"),
        };

    // (2) журнал растёт ПОСЛЕ подключения — push-цикл обязан это заметить
    append_more(dir.path(), 3, BASE_MS + 10_000);

    // (3) собираем кадры, пока push-цикл (250 мс) их не отдаст
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    let mut frames_seen = 0usize;
    while tokio::time::Instant::now() < deadline {
        let next = tokio::time::timeout(tokio::time::Duration::from_millis(500), ws.next()).await;
        let Ok(Some(Ok(m))) = next else { continue };
        if let Ok(ServeMsg::Frame(f)) = serde_json::from_slice::<ServeMsg>(m.into_data().as_ref()) {
            acc.apply(&f);
            frames_seen += 1;
            let latest = gateway::snapshot(
                dir.path(),
                EpochFilter::OwnCaptureOnly,
                &sel(None),
                Cursor::LATEST,
            )
            .expect("replay LATEST");
            if acc.cursor == latest.cursor {
                assert_eq!(
                    acc.series, latest.series,
                    "O-3: snapshot(C) + frames ≢ snapshot(LATEST) — live-режим расходится с реальностью"
                );
                return; // сошлось
            }
        }
    }
    panic!("O-3: за 5 s не пришло ни одного сходящегося кадра (кадров получено: {frames_seen}) — push-цикл не доставляет приращения");
}

// ─────────────────────────── O-5 ───────────────────────────

/// **O-5(а).** `window_ms` реально ограничивает окно НА WS-ПУТИ (анти-TD-039/TD-020).
///
/// `window_ms: None` = unbounded ⇒ снапшот растёт с историей (на проде это давало ООМ).
/// Проверяем не «конфиг долетел» (это уже покрыто `red_serve_window_wiring`), а что
/// ОТДАННЫЕ ПО WS данные под узким окном действительно короче.
#[tokio::test]
async fn o5_bounded_window_shrinks_ws_payload() {
    let dir = tempfile::tempdir().expect("tempdir");
    journal_seed(dir.path());
    append_more(dir.path(), 30, BASE_MS + 10_000); // ~30 s истории

    let unbounded = {
        let mut ws = connect(
            config(dir.path(), None, None),
            Some(&sign_with(SECRET, FUTURE)),
        )
        .await
        .expect("connect unbounded");
        let m = ws.next().await.expect("msg").expect("ok");
        match serde_json::from_slice::<ServeMsg>(m.into_data().as_ref()).expect("parse") {
            ServeMsg::Snapshot(s) => s,
            other => panic!("ожидался Snapshot, получено {other:?}"),
        }
    };

    let bounded = {
        let mut ws = connect(
            config(dir.path(), Some(5_000), None),
            Some(&sign_with(SECRET, FUTURE)),
        )
        .await
        .expect("connect bounded");
        let m = ws.next().await.expect("msg").expect("ok");
        match serde_json::from_slice::<ServeMsg>(m.into_data().as_ref()).expect("parse") {
            ServeMsg::Snapshot(s) => s,
            other => panic!("ожидался Snapshot, получено {other:?}"),
        }
    };

    assert!(
        !unbounded.series.ohlcv.is_empty(),
        "O-5: unbounded-снапшот пуст — фикстура не даёт истории, тест ничего не доказывает"
    );
    assert!(
        bounded.series.ohlcv.len() < unbounded.series.ohlcv.len(),
        "O-5: окно 5 s не сузило выдачу (bounded={} vs unbounded={}) — window_ms не действует на WS-пути",
        bounded.series.ohlcv.len(),
        unbounded.series.ohlcv.len()
    );
}

/// **O-5(б) деградированный вход.** Невалидный/пустой каталог чекпоинта — НЕ ошибка:
/// путь обязан тихо свалиться в rebuild (GW-I-9(б)), а не отказать клиенту.
#[tokio::test]
async fn o5_broken_checkpoint_falls_back_not_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    journal_seed(dir.path());

    let ckpt = tempfile::tempdir().expect("ckpt dir");
    std::fs::write(ckpt.path().join("ckpt-deadbeef.bin"), b"not-a-checkpoint")
        .expect("write мусор вместо чекпоинта");

    let ok = got_snapshot(
        config(dir.path(), None, Some(ckpt.path())),
        Some(&sign_with(SECRET, FUTURE)),
    )
    .await;
    assert!(
        ok,
        "O-5: битый чекпоинт обязан приводить к тихому rebuild (GW-I-9б), а не к отказу клиенту"
    );
}
