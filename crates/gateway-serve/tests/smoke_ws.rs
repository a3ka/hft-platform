//! SMOKE M-28 task #4 (acceptance surface WS-сервера; НЕ детерм-оракул — IO/сеть).
//!
//! Даёт task #4 проверяемую поверхность (C-024 блокер #2): реальный WS-хендшейк на ephemeral-порту —
//! валидный JWT → первый msg `ServeMsg::Snapshot`; невалидный (чужой ключ) → отказ (нет snapshot).
//! RED сейчас: `server::bind` = `unimplemented!()` → оба теста FAILED (тело — engine-dev task #4).
//! Read-only, stateless: сервер верифицирует подпись (`auth::verify_token`), в user-БД не ходит (D6).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::StreamExt;
use gateway::Selector;
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind, ServeConfig};
use gateway_serve::wire::ServeMsg;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};

const FUTURE: usize = 9_999_999_999;

fn sign(secret: &[u8], exp: usize) -> String {
    let claims = Claims {
        sub: "smoke-user".to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .expect("encode")
}

fn journal_of() -> tempfile::TempDir {
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
        for i in 0..4i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + i as f64),
                    size: to_fixed(1.0),
                    side: Side::Buy,
                    ts_exch_ms: 1_752_000_010_000 + i,
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

fn config(dir: &std::path::Path, verify_secret: &[u8]) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(), // ephemeral — реальный порт из local_addr()
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(verify_secret),
    }
}

#[tokio::test]
async fn valid_jwt_receives_snapshot() {
    let secret = b"smoke-secret";
    let token = sign(secret, FUTURE);
    let dir = journal_of();

    let server = bind(config(dir.path(), secret)).await.expect("bind");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let url = format!("ws://{addr}/?token={token}");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect с валидным JWT");
    let msg = ws.next().await.expect("есть сообщение").expect("ok");
    let parsed: ServeMsg =
        serde_json::from_slice(msg.into_data().as_ref()).expect("парсинг ServeMsg (JSON)");
    assert!(
        matches!(parsed, ServeMsg::Snapshot(_)),
        "первый msg обязан быть snapshot-при-подключении"
    );
}

#[tokio::test]
async fn invalid_jwt_rejected() {
    let dir = journal_of();
    // Сервер верифицирует ключом server-secret; клиент подписывает ЧУЖИМ → отказ.
    let server = bind(config(dir.path(), b"server-secret"))
        .await
        .expect("bind");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let bad = sign(b"attacker-secret", FUTURE);
    let url = format!("ws://{addr}/?token={bad}");
    let rejected = match tokio_tungstenite::connect_async(url).await {
        Err(_) => true, // отказ на хендшейке (напр. 401)
        Ok((mut ws, _)) => match ws.next().await {
            None | Some(Err(_)) => true, // закрыто без данных
            Some(Ok(m)) => serde_json::from_slice::<ServeMsg>(m.into_data().as_ref())
                .map(|sm| !matches!(sm, ServeMsg::Snapshot(_)))
                .unwrap_or(true), // не Snapshot (напр. Error/Close) — тоже отказ
        },
    };
    assert!(
        rejected,
        "невалидный JWT обязан быть отклонён (никакого snapshot)"
    );
}
