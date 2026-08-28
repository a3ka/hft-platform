//! `TD-083` O-3 (architect, sacred) — **сервис остаётся живым после ухода клиента и под
//! нагрузкой push-цикла**.
//!
//! ## Что наблюдалось на ПРОДЕ (`R-025`, sidecar-прогон M-46)
//!
//! После РОВНО ОДНОГО подключения, клиент штатно отключился:
//! ```text
//! +5 s   CPU 100.30%, CLOSE_WAIT 2
//! +4 мин CPU 100.26%, CLOSE_WAIT 10, /proc/1/task = 1, docker ps → (healthy)
//! next   wsprobe → connect timeout ×2, в логе сервера НЕТ ни "ws auth ok", ни "rejected"
//! ```
//! То есть таск соединения не завершился, а accept-loop перестал исполняться вовсе.
//!
//! ## Почему две ветки выхода не сработали
//!
//! Push-цикл сидит в `tokio::select!` между `stream.next()` (приём от клиента) и тиком.
//! Блокирующий journal-read внутри тика не даёт точки await ⇒ `stream.next()` не поллится ⇒
//! уход клиента не детектируется. Вторая ветка — `sink.send(..).is_err()` — достижима ТОЛЬКО
//! когда есть кадры; кадров нет (тик не успевает) ⇒ **ни одна ветка не срабатывает**.
//!
//! ## Что проверяет этот оракул
//!
//! Три свойства, каждое отдельным тестом, потому что ломаются они независимо:
//! 1. второй клиент подключается ПОСЛЕ ухода первого (accept-loop жив);
//! 2. второй клиент подключается ПОКА первый ещё держит соединение (push-цикл одного
//!    соединения не монополизирует рантайм);
//! 3. клиент, оборвавший соединение молча (drop без close-handshake), не оставляет сервер
//!    в состоянии, где следующий не может подключиться.
//!
//! **Ограничение фикстуры названо честно:** заклинивание на проде вызывалось РАЗМЕРОМ журнала
//! (≈139M событий до курсора). На тестовом журнале тик дёшев, поэтому эти тесты НЕ
//! воспроизводят прод-зависание — они фиксируют СВОЙСТВО живости, которое обязано
//! сохраняться и тогда, когда тик дорог. Стоимость тика меряется отдельно:
//! `crates/gateway/tests/red_push_seek_bounded.rs` и `red_frames_seek_bound.rs::td083_*`.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use futures_util::StreamExt;
use gateway::Selector;
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind, ServeConfig};
use gateway_serve::wire::ServeMsg;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};

const FUTURE: usize = 9_999_999_999;
const SECRET: &[u8] = b"td083-secret";
const BASE_MS: i64 = 1_784_116_800_000;
/// Столько ждём подключения. На здоровом сервере — миллисекунды; на заклиненном не хватит и
/// минуты (на проде было два подряд connect-timeout по 30 s).
const CONNECT_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);

fn sign() -> String {
    let claims = Claims {
        sub: "td083".to_string(),
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
        max_segment_bytes: 64 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "td-083".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// Журнал с книгой и потоком сделок — чтобы push-цикл имел, что отдавать.
fn journal_busy(events: i64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![lvl(65_000.0, 2.0)],
                asks: vec![lvl(65_010.0, 1.5)],
                ts_exch_ms: BASE_MS,
            },
        ))
        .expect("snap");
        for i in 0..events {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + (i % 20) as f64),
                    size: to_fixed(0.5),
                    side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    ts_exch_ms: BASE_MS + i * 100,
                },
            ))
            .expect("trade");
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
        // Режим ПРОДА (GATEWAY_WINDOW_MS=60000).
        window_ms: Some(60_000),
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

/// Поднять сервер, вернуть адрес. Сервер живёт в фоне на всё время теста.
async fn serve(dir: &std::path::Path) -> String {
    let server = bind(config(dir)).await.expect("bind");
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    addr
}

/// Подключиться и дождаться первого `Snapshot`. `None` = не удалось в бюджет.
async fn connect_and_get_snapshot(
    addr: &str,
) -> Option<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let url = format!("ws://{addr}/?token={}", sign());
    let mut ws =
        match tokio::time::timeout(CONNECT_BUDGET, tokio_tungstenite::connect_async(url)).await {
            Ok(Ok((ws, _))) => ws,
            _ => return None,
        };
    match tokio::time::timeout(CONNECT_BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => match serde_json::from_slice::<ServeMsg>(m.into_data().as_ref()) {
            Ok(ServeMsg::Snapshot(_)) => Some(ws),
            _ => None,
        },
        _ => None,
    }
}

/// **O-3(а).** Второй клиент подключается ПОСЛЕ ухода первого.
///
/// На проде именно это и сломалось: после одного клиента accept-loop переставал исполняться,
/// и следующие получали connect-timeout при зелёном healthcheck.
#[tokio::test]
async fn td083_second_client_connects_after_first_left() {
    let dir = journal_busy(3_000);
    let addr = serve(dir.path()).await;

    let first = connect_and_get_snapshot(&addr).await;
    assert!(first.is_some(), "O-3(а): первый клиент не получил Snapshot");
    drop(first); // штатный уход клиента

    // Дать серверу такт на обработку разрыва.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let second = connect_and_get_snapshot(&addr).await;
    assert!(
        second.is_some(),
        "O-3(а) TD-083 ВОСПРОИЗВЕДЁН: после ухода первого клиента ВТОРОЙ не смог подключиться \
         за {CONNECT_BUDGET:?}. На проде это выглядело как connect-timeout ×2 при `docker ps` \
         (healthy) — accept-loop не исполняется, потому что таск первого соединения не \
         завершился и монополизировал рантайм."
    );
}

/// **O-3(б).** Второй клиент подключается, ПОКА первый ещё держит соединение.
///
/// Это про монополизацию рантайма: push-цикл ОДНОГО соединения не имеет права мешать
/// accept-loop'у. Именно здесь работает `spawn_blocking` вокруг journal-read.
#[tokio::test]
async fn td083_accept_loop_alive_while_client_connected() {
    let dir = journal_busy(3_000);
    let addr = serve(dir.path()).await;

    let _first = connect_and_get_snapshot(&addr)
        .await
        .expect("O-3(б): первый клиент не получил Snapshot");

    // Держим первого открытым и даём push-циклу поработать несколько тиков (250 ms каждый).
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    let second = connect_and_get_snapshot(&addr).await;
    assert!(
        second.is_some(),
        "O-3(б) TD-083: при ОДНОМ живом клиенте второй не подключился за {CONNECT_BUDGET:?} — \
         push-цикл первого соединения монополизирует рантайм (блокирующий journal-read без \
         spawn_blocking в однопоточном tokio)."
    );
}

/// **O-3(в) деградированный вход.** Клиент оборвал соединение МОЛЧА (drop без close-handshake) —
/// ровно то, что делает упавший процесс или разорванная сеть.
///
/// Именно этот случай не имел ветки выхода: `sink.send(..).is_err()` достижимо только когда
/// есть кадры, а `stream.next()` не поллится, пока тик блокирует поток.
#[tokio::test]
async fn td083_abrupt_client_drop_does_not_wedge_server() {
    let dir = journal_busy(3_000);
    let addr = serve(dir.path()).await;

    {
        let ws = connect_and_get_snapshot(&addr)
            .await
            .expect("O-3(в): первый клиент не получил Snapshot");
        // Роняем сокет БЕЗ close-handshake.
        std::mem::drop(ws);
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let after = connect_and_get_snapshot(&addr).await;
    assert!(
        after.is_some(),
        "O-3(в) TD-083: после ГРУБОГО обрыва клиента сервер не принимает новых соединений. \
         Уход клиента обязан детектироваться ДАЖЕ когда кадров нет — иначе таск течёт вечно, \
         а сокет остаётся в CLOSE_WAIT (на проде их накопилось 10 за 4 минуты)."
    );
}
