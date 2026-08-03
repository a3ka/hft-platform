//! `wsprobe` — M-46 task #1: WS read-path harness client (подключение, snapshot+frames, дамп).
//!
//! Read-only клиент для `gateway-serve` (никогда не пишет боевой журнал — единственный
//! writer-путь в этом бинаре существует ТОЛЬКО под `--self-test`, и пишет он в ЭФЕМЕРНЫЙ
//! `tempfile::tempdir()`, никогда в прод-журнал; см. `research/reports/M-46-engine-dev-report.md`
//! §Находки за разбор границы с GS-I-3/verify_M-28.sh).
//!
//! ## Использование
//!
//! ```text
//! wsprobe --url ws://127.0.0.1:8080 --token <JWT> --frames 20 --seconds 10 --out ./out
//! wsprobe --url ws://127.0.0.1:8080 --secret <hex|str> --out ./out
//! wsprobe --self-test --out ./out          # без сети: своя фикстура, свой сервер, свой клиент
//! ```
//!
//! Подключается, принимает первый `ServeMsg::Snapshot`, затем до `--frames` кадров или до
//! истечения `--seconds` (что раньше — push-цикл сервера `PUSH_INTERVAL_MS=250`, `docs/plans/
//! gateway-ws-contract.md` §3). Пишет `snapshot.json` (сырой wire-JSON), `frames.jsonl` (по
//! кадру на строку, сырой wire-JSON), `summary.json` (длины всех 10 серий SeriesBundle,
//! латентность до первого Snapshot, cursor, schema_version, history_truncated/history_start_seq).
//!
//! Рендер "для глаз" (ASCII-панель + `panel.html`) — task #4, следующий коммит.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use gateway::Snapshot;
use gateway_serve::wire::ServeMsg;
use jsonwebtoken::{encode, EncodingKey, Header};
use tokio_tungstenite::tungstenite::Message;

type ProbeResult<T> = Result<T, String>;

// ─────────────────────────── CLI ───────────────────────────

#[derive(Debug)]
struct Args {
    url: String,
    token: Option<String>,
    secret: Option<String>,
    frames: usize,
    seconds: u64,
    out: PathBuf,
    self_test: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:8080".to_string(),
            token: None,
            secret: None,
            frames: 20,
            seconds: 10,
            out: PathBuf::from("./wsprobe-out"),
            self_test: false,
        }
    }
}

fn parse_args() -> ProbeResult<Args> {
    // Тот же normalize-паттерн, что у `gateway-checkpoint`/`journal-retention`: `--flag=value`
    // раскладывается в два токена ДО разбора, чтобы `--flag value` и `--flag=value` были
    // равноправны.
    let raw: Vec<String> = std::env::args()
        .skip(1)
        .flat_map(|a| {
            if let Some(stripped) = a.strip_prefix("--") {
                if let Some((k, v)) = stripped.split_once('=') {
                    return vec![format!("--{k}"), v.to_string()];
                }
            }
            vec![a]
        })
        .collect();

    let mut a = Args::default();
    let mut i = 0;
    while i < raw.len() {
        let flag = raw[i].as_str();
        let next = |i: usize| -> ProbeResult<&str> {
            raw.get(i + 1)
                .map(String::as_str)
                .ok_or_else(|| format!("флаг `{flag}` требует значение"))
        };
        match flag {
            "--url" => {
                a.url = next(i)?.to_string();
                i += 2;
            }
            "--token" => {
                a.token = Some(next(i)?.to_string());
                i += 2;
            }
            "--secret" => {
                a.secret = Some(next(i)?.to_string());
                i += 2;
            }
            "--frames" => {
                a.frames = next(i)?
                    .parse::<usize>()
                    .map_err(|e| format!("--frames: {e}"))?;
                i += 2;
            }
            "--seconds" => {
                a.seconds = next(i)?
                    .parse::<u64>()
                    .map_err(|e| format!("--seconds: {e}"))?;
                i += 2;
            }
            "--out" => {
                a.out = PathBuf::from(next(i)?);
                i += 2;
            }
            "--self-test" => {
                a.self_test = true;
                i += 1;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("неизвестный флаг `{other}` (попробуй --help)")),
        }
    }
    Ok(a)
}

fn print_help() {
    println!(
        "wsprobe — M-46 read-path harness для gateway-serve (WS-клиент, только чтение)\n\
         \n\
         USAGE:\n\
         \x20\x20wsprobe [--url ws://HOST:PORT] (--token <JWT> | --secret <hex|str>) [--frames N] [--seconds S] [--out DIR]\n\
         \x20\x20wsprobe --self-test [--out DIR]\n\
         \n\
         FLAGS:\n\
         \x20\x20--url <ws://..>     дефолт ws://127.0.0.1:8080\n\
         \x20\x20--token <JWT>       готовый подписанный токен\n\
         \x20\x20--secret <hex|str>  подписать HS256 самому (claims sub=wsprobe, exp=+1h);\n\
         \x20\x20                    строка из ТОЛЬКО hex-символов чётной длины → декодируется как hex,\n\
         \x20\x20                    иначе — как есть (UTF-8 байты секрета)\n\
         \x20\x20--frames N          сколько Frame принять максимум (дефолт 20)\n\
         \x20\x20--seconds S         или сколько секунд ждать (дефолт 10) — что раньше\n\
         \x20\x20--out DIR           куда писать snapshot.json/frames.jsonl/summary.json/panel.html\n\
         \x20\x20--self-test         БЕЗ сети: своя фикстура-журнал, свой сервер, свой клиент —\n\
         \x20\x20                    для проверки рендера без прода (gate T9)\n"
    );
}

fn parse_secret(s: &str) -> Vec<u8> {
    let is_hex =
        !s.is_empty() && s.len().is_multiple_of(2) && s.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex {
        let mut out = Vec::with_capacity(s.len() / 2);
        let bytes = s.as_bytes();
        let mut ok = true;
        for chunk in bytes.chunks(2) {
            let hi = (chunk[0] as char).to_digit(16);
            let lo = (chunk[1] as char).to_digit(16);
            match (hi, lo) {
                (Some(h), Some(l)) => out.push(((h << 4) | l) as u8),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return out;
        }
    }
    s.as_bytes().to_vec()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_secs()
}

fn sign_hs256(secret: &[u8], sub: &str, exp: u64) -> ProbeResult<String> {
    let claims = gateway_serve::auth::Claims {
        sub: sub.to_string(),
        exp: exp as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|e| format!("jwt encode: {e}"))
}

// ─────────────────────────── self-test fixture ───────────────────────────

/// Смешанная фикстура: `L2Snapshot` + мульти-филл `Trade` + асимметричные `L2Delta` по обе
/// стороны границы UTC-суток. Тот же чек-лист «фикстура счастливого пути — дефект оракула»
/// (`.claude/rules/testing.md`), что и sacred-оракулы M-46 (`red_ws_series_vs_replay.rs`), но
/// это НЕЗАВИСИМАЯ копия для self-test харнесса — не читает и не импортирует sacred-тесты.
fn build_fixture_journal(dir: &std::path::Path) -> std::io::Result<()> {
    use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
    use journal::{Journal, WriterConfig};

    const D1_NOON_MS: i64 = 1_784_116_800_000; // 2026-07-15T12:00:00Z
    const D2_NOON_MS: i64 = 1_784_203_200_000; // 2026-07-16T12:00:00Z — следующая UTC-сессия

    let lvl = |price: f64, size: f64| Level {
        price: to_fixed(price),
        size: to_fixed(size),
    };

    let cfg = WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "wsprobe-self-test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    let mut j = Journal::open_with(dir, cfg)?;

    j.append(EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: vec![lvl(65_000.0, 2.0), lvl(64_990.0, 3.0)],
            asks: vec![lvl(65_010.0, 1.5), lvl(65_020.0, 4.0)],
            ts_exch_ms: D1_NOON_MS,
        },
    ))?;

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
        ))?;
    }

    // Асимметричный дифф: только аски меняются, бид молчит ⇒ обязан выжить.
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
    ))?;

    j.append(EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(66_000.0),
            size: to_fixed(2.0),
            side: Side::Buy,
            ts_exch_ms: D2_NOON_MS,
        },
    ))?;

    // Асимметричный дифф сессии 2: только биды, цена внутри спреда (книга не скрещена).
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
    ))?;

    j.flush()
}

const SELF_TEST_SECRET: &[u8] = b"wsprobe-self-test-secret";

/// Поднять эфемерный сервер на своей фикстуре. Возвращает (держатель tempdir — не дропать!,
/// адрес). Никакого io — только tokio TcpListener на `127.0.0.1:0`.
async fn start_self_test_server() -> ProbeResult<(tempfile::TempDir, std::net::SocketAddr)> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    build_fixture_journal(dir.path()).map_err(|e| format!("build_fixture_journal: {e}"))?;

    let cfg = gateway_serve::server::ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.path().to_path_buf(),
        filter: journal::EpochFilter::OwnCaptureOnly,
        selector: gateway_serve::build_selector(
            contracts::Venue::Binance,
            "BTCUSDT".to_string(),
            1_000,
            vec![0.001],
            None,
        ),
        decoding_key: jsonwebtoken::DecodingKey::from_secret(SELF_TEST_SECRET),
        checkpoint_dir: None,
    };
    let server = gateway_serve::server::bind(cfg)
        .await
        .map_err(|e| format!("bind self-test server: {e}"))?;
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    Ok((dir, addr))
}

// ─────────────────────────── probe ───────────────────────────

#[derive(serde::Serialize)]
struct SeriesLengths {
    ohlcv: usize,
    cumulative_delta: usize,
    cvd_session_base: usize,
    depth_series: usize,
    vwap: usize,
    volume_profile: usize,
    vp_session_max_time_s: usize,
    heatmap: usize,
    cob: usize,
    volume_bubbles: usize,
}

fn series_lengths(s: &gateway::SeriesBundle) -> SeriesLengths {
    SeriesLengths {
        ohlcv: s.ohlcv.len(),
        cumulative_delta: s.cumulative_delta.len(),
        cvd_session_base: s.cvd_session_base.len(),
        depth_series: s.depth_series.len(),
        vwap: s.vwap.len(),
        volume_profile: s.volume_profile.len(),
        vp_session_max_time_s: s.vp_session_max_time_s.len(),
        heatmap: s.heatmap.len(),
        cob: s.cob.len(),
        volume_bubbles: s.volume_bubbles.len(),
    }
}

#[derive(serde::Serialize)]
struct Summary {
    schema_version: u32,
    cursor_upto_seq: Option<u64>,
    history_start_seq: u64,
    history_truncated: bool,
    latency_first_snapshot_ms: u128,
    frames_received: usize,
    series_lengths: SeriesLengths,
}

fn text_of(msg: &Message) -> ProbeResult<String> {
    match msg {
        Message::Text(t) => Ok(t.clone()),
        other => Err(format!("ожидался Text-фрейм, получено {other:?}")),
    }
}

async fn run(args: Args) -> ProbeResult<()> {
    std::fs::create_dir_all(&args.out).map_err(|e| format!("--out {}: {e}", args.out.display()))?;

    // (kept alive until end of `run` — server reads this dir on every connection)
    let _fixture_guard;
    let url: String;
    let token: String;

    if args.self_test {
        let (dir, addr) = start_self_test_server().await?;
        _fixture_guard = Some(dir);
        url = format!("ws://{addr}");
        token = sign_hs256(
            SELF_TEST_SECRET,
            "wsprobe-self-test",
            now_unix_secs() + 3600,
        )?;
    } else {
        _fixture_guard = None;
        url = args.url.clone();
        token = match (&args.token, &args.secret) {
            (Some(t), _) => t.clone(),
            (None, Some(s)) => {
                let key = parse_secret(s);
                sign_hs256(&key, "wsprobe", now_unix_secs() + 3600)?
            }
            (None, None) => return Err("нужен --token, --secret, либо --self-test".to_string()),
        };
    }

    // Путь не проверяется сервером (`docs/plans/gateway-ws-contract.md` §1), но HTTP request-line
    // ОБЯЗАНА содержать `/` перед query — без него получается `GET ?token=... HTTP/1.1`,
    // невалидный формат (замечено на self-test: сервер логировал `HTTP format error: invalid
    // format` и рвал handshake). `ws://host:port` (без пути) — самый частый ввод (дефолт
    // `--url`, self-test), поэтому нормализуем ЯВНО, а не полагаемся, что вызывающий допишет `/`.
    let has_path_after_authority = url
        .find("://")
        .map(|i| url[i + 3..].contains('/'))
        .unwrap_or_else(|| url.contains('/'));
    let with_path = if has_path_after_authority {
        url.clone()
    } else {
        format!("{url}/")
    };
    let sep = if with_path.contains('?') { "&" } else { "?" };
    let full_url = format!("{with_path}{sep}token={token}");

    let connect_timeout = Duration::from_secs(if args.self_test { 10 } else { 30 });
    let (ws_stream, _resp) =
        tokio::time::timeout(connect_timeout, tokio_tungstenite::connect_async(&full_url))
            .await
            .map_err(|_| format!("connect timeout после {connect_timeout:?}"))?
            .map_err(|e| format!("connect_async({url}): {e}"))?;
    let mut ws = ws_stream;

    // Холодный чекпоинт на проде мерился в минутах (382.657 s, docs/plans/gateway-ws-contract.md
    // §4/§9) — таймаут ожидания первого сообщения ОБЯЗАН быть щедрым для прод-режима.
    // В self-test журнал крошечный — сервер отвечает почти мгновенно.
    let snapshot_wait = Duration::from_secs(if args.self_test { 10 } else { 600 });
    let t0 = Instant::now();
    let first = tokio::time::timeout(snapshot_wait, ws.next())
        .await
        .map_err(|_| {
            format!(
                "нет сообщения от сервера за {snapshot_wait:?} — холодный чекпоинт? \
                 (см. docs/plans/gateway-ws-contract.md §4/§9)"
            )
        })?
        .ok_or_else(|| "соединение закрыто до первого сообщения".to_string())?
        .map_err(|e| format!("ws read error: {e}"))?;
    let latency_first_ms = t0.elapsed().as_millis();

    let raw_snapshot = text_of(&first)?;
    std::fs::write(args.out.join("snapshot.json"), &raw_snapshot)
        .map_err(|e| format!("write snapshot.json: {e}"))?;

    let parsed: ServeMsg =
        serde_json::from_str(&raw_snapshot).map_err(|e| format!("parse первого сообщения: {e}"))?;
    let snap: Snapshot = match parsed {
        ServeMsg::Snapshot(s) => s,
        ServeMsg::Error(e) => return Err(format!("сервер отказал в авторизации: {e}")),
        ServeMsg::Frame(_) => {
            return Err(
                "первым сообщением пришёл Frame, ожидался Snapshot (протокол нарушен)".to_string(),
            )
        }
    };

    // Push-цикл: до `--frames` кадров ИЛИ до истечения `--seconds` — что раньше. Отсутствие
    // кадров в пределах дедлайна — НЕ ошибка (тихий рынок / self-test без новых событий).
    let deadline = Instant::now() + Duration::from_secs(args.seconds);
    let mut frames_file = std::fs::File::create(args.out.join("frames.jsonl"))
        .map_err(|e| format!("create frames.jsonl: {e}"))?;
    let mut n_frames = 0usize;
    while n_frames < args.frames {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, ws.next()).await {
            Err(_) => break,   // deadline
            Ok(None) => break, // connection closed
            Ok(Some(Err(e))) => {
                eprintln!("wsprobe: read error в push-цикле: {e}");
                break;
            }
            Ok(Some(Ok(Message::Text(t)))) => {
                if let Ok(ServeMsg::Frame(_)) = serde_json::from_str::<ServeMsg>(&t) {
                    writeln!(frames_file, "{t}").map_err(|e| format!("write frames.jsonl: {e}"))?;
                    n_frames += 1;
                }
            }
            Ok(Some(Ok(Message::Close(_)))) => break,
            Ok(Some(Ok(_))) => {} // Ping/Pong/Binary — игнор (read-only harness)
        }
    }

    let summary = Summary {
        schema_version: snap.schema_version,
        cursor_upto_seq: snap.cursor.upto_seq,
        history_start_seq: snap.history_start_seq,
        history_truncated: snap.history_truncated,
        latency_first_snapshot_ms: latency_first_ms,
        frames_received: n_frames,
        series_lengths: series_lengths(&snap.series),
    };
    std::fs::write(
        args.out.join("summary.json"),
        serde_json::to_string_pretty(&summary).map_err(|e| format!("serialize summary: {e}"))?,
    )
    .map_err(|e| format!("write summary.json: {e}"))?;

    println!(
        "wsprobe: schema_version={} cursor={:?} history_start_seq={} history_truncated={} \
         latency_first_snapshot_ms={} frames_received={}",
        summary.schema_version,
        summary.cursor_upto_seq,
        summary.history_start_seq,
        summary.history_truncated,
        summary.latency_first_snapshot_ms,
        summary.frames_received,
    );
    println!(
        "series lengths: ohlcv={} cvd={} vwap={} depth_series={} volume_profile={} heatmap={} cob={} volume_bubbles={}",
        summary.series_lengths.ohlcv,
        summary.series_lengths.cumulative_delta,
        summary.series_lengths.vwap,
        summary.series_lengths.depth_series,
        summary.series_lengths.volume_profile,
        summary.series_lengths.heatmap,
        summary.series_lengths.cob,
        summary.series_lengths.volume_bubbles,
    );
    println!();
    println!(
        "wrote {} (snapshot.json, frames.jsonl, summary.json) — render (task #4) не реализован\n",
        args.out.display()
    );

    Ok(())
}

// ─────────────────────────── main ───────────────────────────

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("wsprobe: {e}\n");
            print_help();
            return ExitCode::from(2);
        }
    };
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wsprobe: error: {e}");
            ExitCode::from(1)
        }
    }
}
