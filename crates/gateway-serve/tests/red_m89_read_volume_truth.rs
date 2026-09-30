//! RED `M-89` (sacred, architect-only) — **счётчик `journal_payload_bytes_read` считает
//! ПРОЧИТАННОЕ на всём пути выдачи, включая первый `pump` и активный сегмент, и получает
//! ТУ ЖЕ дельту, что и читатель** (`I-4`, задача 15 `M-87`, `C-260` R4).
//!
//! # Что не так сегодня — снято командой, не пересказом
//!
//! · (история, до `M-89`) warm-`resume` кормил счётчик `ckpt_bytes + payload_bytes_after_cursor(dir, cursor)` — функция удалена фиксом `R-208` Б-2;
//!   `payload_bytes_after_cursor` (`crates/gateway/src/lib.rs`) суммирует ТОЛЬКО сегменты с
//!   `first_seq > cursor` — сегмент, СОДЕРЖАЩИЙ курсор (на проде — активный, 585 МБ), в сумму
//!   не входит вовсе. Цена его получения при этом лежит на горячем пути:
//!   `journal::list_segments` = `read_dir` + чтение заголовков (`R-202` Н-1);
//! · периодические `pump`'ы push-цикла ОТБРАСЫВАЮТ `ReadStats` (`Ok((frames, _new_cursor,
//!   _stats))`, `crates/gateway-serve/src/lib.rs`, v1- и legacy-путь) — байты первого и всех
//!   последующих pump'ов в счётчик не попадают никогда.
//!
//! # Мера — две, и вторая закрывает мутант критика
//!
//! 1. Нижняя граница `ckpt_bytes + tail_bytes` и верхняя `rchar` — как в первой редакции;
//!    RUNTIME-RED сегодня (`counter = ckpt_bytes`).
//! 2. **Дельта транспорта = дельта читателя.** `C-260` R4: интервал пропускает синтетику
//!    (`events_scanned × 64`) и дубликат счётчика, кормящий транспорт при мёртвом публичном
//!    методе. Здесь ожидание вычисляет РЕПЛИКА того же пути в процессе теста —
//!    `LiveReducer::resume` (warm) + `pump(256)` (первый, дренирует хвост) + `pump(256)`
//!    (пустой установившийся тик) на том же журнале и слепке. Транспорт обязан прибавить
//!    РОВНО `R + P1 + n·P2`, где `R`/`P1`/`P2` — `ReadStats.payload_bytes_read` реплики, а
//!    `n ≥ 0` — число пустых тиков push-цикла (период 250 мс), ограниченное временем
//!    наблюдения. Ни одно слагаемое не сочиняется транспортом: значения читателя пиннит
//!    `crates/journal/tests/red_m89_bytes_accounting.rs` точным учётом, здесь судится ВЕРНОСТЬ
//!    ПЕРЕДАЧИ. Связь — через РУЧКУ ЭКЗЕМПЛЯРА `Server::counters_handle()` (`R-209` Б-1):
//!    процессный `serving_counters()` рос и при записи байт pump'ов мимо экземпляра, и
//!    равенство на нём не ловило регресс задачи 4. Мутация «pump'ы → процессный счётчик»
//!    обязана ронять этот оракул.
//!
//! RUNTIME-RED на ревизии набора: `counter = ckpt_bytes` < `ckpt_bytes + tail_bytes`.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::{LiveReducer, Selector};
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::server::{bind_with_policy, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m89-read-volume-truth";
const FUTURE: usize = 4_000_000_000;
const BASE_MS: i64 = 1_784_116_800_000;
const BUDGET: std::time::Duration = std::time::Duration::from_secs(60);
const CANONICAL: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];
const SEG_BYTES: u64 = 1 << 30;
/// Префикс достаточен, чтобы «сегмент целиком» и «хвост» отличались на два порядка.
const PREFIX: u64 = 20_000;
const TAIL: u64 = 500;
const MAX_TAIL_EVENTS: u64 = 1_000;
const EXPECTED_WARMUP_EVENTS: u64 = 100;
/// Прод-партия push-цикла (`PUSH_MAX_EVENTS`, `crates/gateway-serve/src/lib.rs`) и его период.
const PUMP_BATCH: usize = 256;
const PUSH_INTERVAL_MS: u64 = 250;

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m89".to_string(),
            exp: FUTURE,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt")
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m89".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(65_000.0 + (i % 997) as f64 * 0.01),
            size: to_fixed(0.001 + (i % 101) as f64 * 0.0001),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: BASE_MS + i as i64 * 7,
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: CANONICAL.to_vec(),
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        allowed_symbols: vec!["BTCUSDT".to_string()],
        canonical_bands: CANONICAL.to_vec(),
        allowed_profiles: vec![LiveProfile {
            timeframe_ms: 1_000,
            window_ms: 60_000,
            depth_cadence_ms: None,
        }],
        max_concurrent_serves: 1,
        max_tail_events: MAX_TAIL_EVENTS,
        expected_warmup_events: EXPECTED_WARMUP_EVENTS,
    }
}

fn setup_failed(what: &str) -> ! {
    panic!("SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о счётчике.")
}

fn rchar() -> u64 {
    let s = std::fs::read_to_string("/proc/self/io")
        .unwrap_or_else(|e| setup_failed(&format!("/proc/self/io недоступен ({e})")));
    for line in s.lines() {
        if let Some(v) = line.strip_prefix("rchar:") {
            return v.trim().parse().expect("rchar — число");
        }
    }
    setup_failed("в /proc/self/io нет строки rchar")
}

fn dir_bytes(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

struct Fixture {
    dir: tempfile::TempDir,
    ckpt: tempfile::TempDir,
    ckpt_bytes: u64,
    tail_bytes: u64,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..PREFIX {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    let ckpt_bytes = std::fs::metadata(gateway::checkpoint::ckpt_path_for_pub(ckpt.path(), &sel()))
        .map(|m| m.len())
        .unwrap_or_else(|e| setup_failed(&format!("файл слепка: {e}")));
    let before = dir_bytes(dir.path());
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("reopen");
        for i in PREFIX..PREFIX + TAIL {
            j.append(trade(i)).expect("append tail");
        }
        j.flush().expect("flush tail");
    }
    let tail_bytes = dir_bytes(dir.path()).saturating_sub(before);
    Fixture {
        dir,
        ckpt,
        ckpt_bytes,
        tail_bytes,
    }
}

/// Реплика прод-пути в процессе теста: (resume, первый pump, пустой тик) в байтах читателя.
struct Replica {
    resume: u64,
    first_pump: u64,
    empty_tick: u64,
}

fn replica(dir: &std::path::Path, ckpt: &std::path::Path) -> Replica {
    let (mut live, rs) = LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &sel(), ckpt)
        .unwrap_or_else(|e| setup_failed(&format!("реплика resume: {e}")));
    if rs.events_scanned != 0 {
        setup_failed("реплика: resume прочитал журнал — слепок не сработал, путь не тёплый");
    }
    let (frames, _c, p1) = live
        .pump(dir, EpochFilter::OwnCaptureOnly, PUMP_BATCH)
        .unwrap_or_else(|e| setup_failed(&format!("реплика pump #1: {e}")));
    if frames.is_empty() {
        setup_failed("реплика: первый pump не отдал кадров — хвост не прочитан");
    }
    let (frames2, _c2, p2) = live
        .pump(dir, EpochFilter::OwnCaptureOnly, PUMP_BATCH)
        .unwrap_or_else(|e| setup_failed(&format!("реплика pump #2: {e}")));
    if !frames2.is_empty() {
        setup_failed("реплика: второй pump отдал кадры — хвост не был дренирован первым");
    }
    Replica {
        resume: rs.payload_bytes_read,
        first_pump: p1.payload_bytes_read,
        empty_tick: p2.payload_bytes_read,
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn recv(ws: &mut Ws) -> Option<Value> {
    match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref()).ok(),
        _ => None,
    }
}

struct Served {
    rchar_delta: u64,
    counter_delta: u64,
    /// Время от подписки до снятия счётчика — верхняя граница числа пустых тиков.
    elapsed_ms: u64,
}

/// `subscribe` → `snapshot` → первый `frame`; счётчик снимается после паузы, чтобы push-цикл
/// зафиксировал статистику pump'а, отдавшего кадр.
async fn serve_once(f: &Fixture) -> Served {
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: f.dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(f.ckpt.path().to_path_buf()),
    };
    let server = bind_with_policy(cfg, policy())
        .await
        .expect("bind_with_policy");
    let addr = server.local_addr();
    // `R-209` Б-1: счётчик ЭКЗЕМПЛЯРА, не процессный. Процессный `serving_counters()` растёт и
    // тогда, когда байты pump'ов пишутся мимо экземпляра (`add_journal_payload_bytes_pub`), —
    // равенство ниже на нём не отличало исправный путь от регресса задачи 4.
    let counters = server
        .counters_handle()
        .unwrap_or_else(|| setup_failed("bind_with_policy не дал ручку счётчиков экземпляра"));
    tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let rchar_before = rchar();
    let counter_before = counters.snapshot().journal_payload_bytes_read;
    let started = std::time::Instant::now();
    let token = sign();
    let (mut ws, _) = tokio::time::timeout(
        BUDGET,
        tokio_tungstenite::connect_async(format!("ws://{addr}/?token={token}")),
    )
    .await
    .expect("подключение не состоялось")
    .expect("connect");
    let sub = json!({"op":"subscribe","v":1,"id":"s1","selector":{
        "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
        "bands":CANONICAL.to_vec(),"window_ms":60000}});
    ws.send(Message::Text(sub.to_string())).await.expect("send");
    let first = recv(&mut ws)
        .await
        .unwrap_or_else(|| setup_failed("сервер промолчал"));
    if first.get("type").and_then(Value::as_str) != Some("snapshot") {
        setup_failed(&format!("запрос не обслужен снимком: {first}"));
    }
    let mut got_frame = false;
    for _ in 0..8 {
        match recv(&mut ws).await {
            Some(m) if m.get("type").and_then(Value::as_str) == Some("frame") => {
                got_frame = true;
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    if !got_frame {
        setup_failed("первый кадр хвоста не пришёл — первый pump не наблюдался");
    }
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let counter_after = counters.snapshot().journal_payload_bytes_read;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    drop(ws);
    Served {
        rchar_delta: rchar().saturating_sub(rchar_before),
        counter_delta: counter_after.saturating_sub(counter_before),
        elapsed_ms,
    }
}

#[tokio::test]
async fn v_counter_includes_first_pump_and_equals_the_reader_delta() {
    let f = fixture();
    if f.tail_bytes == 0 {
        setup_failed("хвост пуст");
    }
    if journal::list_segments(f.dir.path())
        .expect("segments")
        .len()
        != 1
    {
        setup_failed("сегментов не один — прод-форма не воспроизведена");
    }
    // Реплика идёт ДО сервера: её чтение не входит в rchar-окно обслуживания.
    let r = tokio::task::spawn_blocking({
        let dir = f.dir.path().to_path_buf();
        let ckpt = f.ckpt.path().to_path_buf();
        move || replica(&dir, &ckpt)
    })
    .await
    .expect("replica join");

    let s = serve_once(&f).await;
    let lower = f.ckpt_bytes + f.tail_bytes;
    assert!(
        s.counter_delta >= lower,
        "I-4 / задача 15 M-87: счётчик выдачи объявил {} Б, а обслуживание обязано было прочитать \
         слепок ({} Б) И хвост ({} Б) = {lower} Б. Величина занижена: warm-resume считает \
         `payload_bytes_after_cursor` без сегмента, содержащего курсор, а push-цикл отбрасывает \
         `ReadStats` каждого pump'а. Счётчик, который не видит первый pump, слеп ровно к той \
         работе, ради которой M-89 существует.",
        s.counter_delta,
        f.ckpt_bytes,
        f.tail_bytes
    );
    assert!(
        s.counter_delta <= s.rchar_delta,
        "I-4: счётчик объявил {} Б, а ядро видело {} Б — учесть больше прочитанного невозможно",
        s.counter_delta,
        s.rchar_delta
    );

    // Дельта транспорта = дельта читателя (реплика): R + P1 + n·P2, n — число пустых тиков.
    let base = r.resume + r.first_pump;
    let rem = s.counter_delta.checked_sub(base).unwrap_or_else(|| {
        panic!(
            "I-4 / C-260 R4: транспорт прибавил {} Б, а читатель на том же пути отдал resume={} + \
             первый pump={} = {base} Б — транспорт потерял часть дельты читателя",
            s.counter_delta, r.resume, r.first_pump
        )
    });
    let n_max = s.elapsed_ms / PUSH_INTERVAL_MS + 2;
    if r.empty_tick == 0 {
        assert_eq!(
            rem, 0,
            "I-4 / C-260 R4: транспорт прибавил {} Б при дельте читателя resume={} + pump={} и \
             нулевом пустом тике — лишние {rem} Б сочинены транспортом (синтетика вида \
             events_scanned×64 или дубликат счётчика), а не получены от читателя",
            s.counter_delta, r.resume, r.first_pump
        );
    } else {
        assert!(
            rem % r.empty_tick == 0 && rem / r.empty_tick <= n_max,
            "I-4 / C-260 R4: транспорт прибавил {} Б = resume {} + pump {} + остаток {rem}; остаток \
             обязан быть n·{} Б (n пустых тиков ≤ {n_max}) — дельта транспорта не равна дельте \
             читателя, значения сочинены или продублированы",
            s.counter_delta, r.resume, r.first_pump, r.empty_tick
        );
    }
}
