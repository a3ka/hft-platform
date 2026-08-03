//! `wsprobe` — M-46 tasks #1/#4: WS read-path harness client + render "for eyes" без дизайна.
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
//! латентность до первого Snapshot, cursor, schema_version, history_truncated/history_start_seq)
//! и `panel.html` (автономный рендер — heatmap/candles+vwap/cvd/volume-profile/cob).
//!
//! Печатает в stdout короткую сводку + ASCII-панель — НЕ весь дамп (упирается в лимиты).

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
    println!("{}", render_ascii(&snap, n_frames));

    let html = render_html(&snap, &summary);
    std::fs::write(args.out.join("panel.html"), html)
        .map_err(|e| format!("write panel.html: {e}"))?;
    println!(
        "wrote {} (snapshot.json, frames.jsonl, summary.json, panel.html)",
        args.out.display()
    );

    Ok(())
}

// ─────────────────────────── render: ASCII (stdout) ───────────────────────────

const DENSITY: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

fn density_char(frac: f64) -> char {
    let idx = ((frac.clamp(0.0, 1.0)) * (DENSITY.len() - 1) as f64).round() as usize;
    DENSITY[idx.min(DENSITY.len() - 1)]
}

fn e8(v: i64) -> f64 {
    v as f64 / 100_000_000.0
}

fn bucket(value: f64, lo: f64, hi: f64, n: usize) -> usize {
    if hi <= lo || n == 0 {
        return 0;
    }
    let frac = ((value - lo) / (hi - lo)).clamp(0.0, 0.999_999);
    ((frac * n as f64) as usize).min(n - 1)
}

fn sparkline(vals: &[f64]) -> String {
    const BARS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if vals.is_empty() {
        return "(нет данных)".to_string();
    }
    let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    vals.iter()
        .map(|&v| {
            let frac = if hi > lo { (v - lo) / (hi - lo) } else { 0.5 };
            let idx = (frac.clamp(0.0, 1.0) * (BARS.len() - 1) as f64).round() as usize;
            BARS[idx.min(BARS.len() - 1)]
        })
        .collect()
}

/// ASCII-панель ≤100 столбцов: heatmap-сетка плотности, VWAP-спарклайн, знак CVD, топ COB.
fn render_ascii(snap: &Snapshot, frames_received: usize) -> String {
    let s = &snap.series;
    let mut out = String::new();

    out.push_str(&format!(
        "=== wsprobe panel — schema_version={} cursor={:?} frames_received={} ===\n",
        snap.schema_version, snap.cursor.upto_seq, frames_received
    ));

    // --- heatmap grid ---
    const W: usize = 60;
    const H: usize = 14;
    if s.heatmap.is_empty() {
        out.push_str("heatmap: (пусто — нет L2Snapshot/L2Delta в окне)\n");
    } else {
        let prices: Vec<f64> = s.heatmap.iter().map(|c| e8(c.price_e8)).collect();
        let times: Vec<i64> = s.heatmap.iter().map(|c| c.time_s).collect();
        let pmin = prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let pmax = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let tmin = *times.iter().min().unwrap();
        let tmax = *times.iter().max().unwrap();

        let mut grid = vec![vec![0f64; W]; H];
        for c in &s.heatmap {
            let price = e8(c.price_e8);
            let row = H - 1 - bucket(price, pmin, pmax, H);
            let col = bucket(c.time_s as f64, tmin as f64, tmax as f64, W);
            grid[row][col] += e8(c.size_e8);
        }
        let maxv = grid
            .iter()
            .flatten()
            .cloned()
            .fold(0.0_f64, f64::max)
            .max(1e-9);

        out.push_str(&format!(
            "heatmap ({} cells, price [{:.2}..{:.2}], time [{tmin}..{tmax}]s):\n",
            s.heatmap.len(),
            pmin,
            pmax
        ));
        for (i, row) in grid.iter().enumerate() {
            let price_label = pmax - (i as f64 + 0.5) * (pmax - pmin) / H as f64;
            let line: String = row.iter().map(|&v| density_char(v / maxv)).collect();
            out.push_str(&format!("{price_label:>10.2} |{line}\n"));
        }
    }

    // --- vwap / cvd ---
    let vwap_vals: Vec<f64> = s.vwap.iter().map(|(_, p)| e8(*p)).collect();
    let last_vwap = vwap_vals.last().copied();
    out.push_str(&format!(
        "vwap  (n={:>4}, last={}) {}\n",
        s.vwap.len(),
        last_vwap
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "-".to_string()),
        sparkline(&vwap_vals[vwap_vals.len().saturating_sub(W)..])
    ));

    let cvd_vals: Vec<f64> = s.cumulative_delta.iter().map(|(_, v)| e8(*v)).collect();
    let last_cvd = cvd_vals.last().copied();
    let sign = last_cvd
        .map(|v| if v >= 0.0 { '+' } else { '-' })
        .unwrap_or('?');
    out.push_str(&format!(
        "cvd   (n={:>4}, last={sign}{}) {}\n",
        s.cumulative_delta.len(),
        last_cvd
            .map(|v| format!("{:.4}", v.abs()))
            .unwrap_or_else(|| "-".to_string()),
        sparkline(&cvd_vals[cvd_vals.len().saturating_sub(W)..])
    ));

    // --- COB top levels ---
    let mut bids: Vec<&gateway::CobLevel> = s.cob.iter().filter(|l| l.side == "bid").collect();
    let mut asks: Vec<&gateway::CobLevel> = s.cob.iter().filter(|l| l.side == "ask").collect();
    bids.sort_by_key(|l| std::cmp::Reverse(l.price_e8));
    asks.sort_by_key(|l| l.price_e8);
    out.push_str(&format!("cob (n={}, top 5 each side):\n", s.cob.len()));
    out.push_str("  BID price      size   |   ASK price      size\n");
    for i in 0..5.min(bids.len().max(asks.len())) {
        let b = bids
            .get(i)
            .map(|l| format!("{:>10.2} {:>8.4}", e8(l.price_e8), e8(l.size_e8)))
            .unwrap_or_else(|| " ".repeat(19));
        let a = asks
            .get(i)
            .map(|l| format!("{:>10.2} {:>8.4}", e8(l.price_e8), e8(l.size_e8)))
            .unwrap_or_else(|| " ".repeat(19));
        out.push_str(&format!("  {b}   |   {a}\n"));
    }

    out
}

// ─────────────────────────── render: HTML (panel.html) ───────────────────────────

#[derive(serde::Serialize)]
struct RenderCandle {
    time_s: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(serde::Serialize)]
struct RenderHeatCell {
    time_s: i64,
    side: String,
    price: f64,
    size: f64,
}

#[derive(serde::Serialize)]
struct RenderCob {
    side: String,
    price: f64,
    size: f64,
}

#[derive(serde::Serialize)]
struct RenderVp {
    session_id: i64,
    poc: f64,
    vah: f64,
    val: f64,
    bins: Vec<(f64, f64)>,
}

#[derive(serde::Serialize)]
struct RenderData {
    ohlcv: Vec<RenderCandle>,
    vwap: Vec<(i64, f64)>,
    cvd: Vec<(i64, f64)>,
    heatmap: Vec<RenderHeatCell>,
    cob: Vec<RenderCob>,
    volume_profile: Vec<RenderVp>,
}

fn to_render_data(s: &gateway::SeriesBundle) -> RenderData {
    RenderData {
        ohlcv: s
            .ohlcv
            .iter()
            .map(|r| RenderCandle {
                time_s: r.time_s,
                open: e8(r.open),
                high: e8(r.high),
                low: e8(r.low),
                close: e8(r.close),
                volume: e8(r.volume),
            })
            .collect(),
        vwap: s.vwap.iter().map(|(t, p)| (*t, e8(*p))).collect(),
        cvd: s
            .cumulative_delta
            .iter()
            .map(|(t, v)| (*t, e8(*v)))
            .collect(),
        heatmap: s
            .heatmap
            .iter()
            .map(|c| RenderHeatCell {
                time_s: c.time_s,
                side: c.side.clone(),
                price: e8(c.price_e8),
                size: e8(c.size_e8),
            })
            .collect(),
        cob: s
            .cob
            .iter()
            .map(|l| RenderCob {
                side: l.side.clone(),
                price: e8(l.price_e8),
                size: e8(l.size_e8),
            })
            .collect(),
        volume_profile: s
            .volume_profile
            .iter()
            .map(|vp| RenderVp {
                session_id: vp.session_id,
                poc: e8(vp.poc_e8),
                vah: e8(vp.vah_e8),
                val: e8(vp.val_e8),
                bins: vp.bins.iter().map(|(p, v)| (e8(*p), e8(*v))).collect(),
            })
            .collect(),
    }
}

fn cob_table_rows(cob: &[gateway::CobLevel]) -> String {
    let mut bids: Vec<&gateway::CobLevel> = cob.iter().filter(|l| l.side == "bid").collect();
    let mut asks: Vec<&gateway::CobLevel> = cob.iter().filter(|l| l.side == "ask").collect();
    bids.sort_by_key(|l| std::cmp::Reverse(l.price_e8));
    asks.sort_by_key(|l| l.price_e8);
    let mut rows = String::new();
    for i in 0..10.min(bids.len().max(asks.len())) {
        let (bp, bs) = bids
            .get(i)
            .map(|l| (e8(l.price_e8), e8(l.size_e8)))
            .unwrap_or((0.0, 0.0));
        let (ap, as_) = asks
            .get(i)
            .map(|l| (e8(l.price_e8), e8(l.size_e8)))
            .unwrap_or((0.0, 0.0));
        let bid_cell = if i < bids.len() {
            format!("<td class=\"bid\">{bp:.2}</td><td class=\"bid\">{bs:.4}</td>")
        } else {
            "<td></td><td></td>".to_string()
        };
        let ask_cell = if i < asks.len() {
            format!("<td class=\"ask\">{ap:.2}</td><td class=\"ask\">{as_:.4}</td>")
        } else {
            "<td></td><td></td>".to_string()
        };
        rows.push_str(&format!("<tr>{bid_cell}{ask_cell}</tr>\n"));
    }
    rows
}

fn render_html(snap: &Snapshot, summary: &Summary) -> String {
    let data = to_render_data(&snap.series);
    let data_json = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
    let cob_rows = cob_table_rows(&snap.series.cob);

    format!(
        r##"<!doctype html>
<title>wsprobe panel — M-46</title>
<meta charset="utf-8">
<style>
  :root {{ color-scheme: dark light; }}
  body {{
    background: #0b0f14; color: #d7dee6; font: 13px/1.4 ui-monospace, monospace;
    margin: 0; padding: 16px;
  }}
  h1 {{ font-size: 16px; margin: 0 0 4px; }}
  h2 {{ font-size: 13px; margin: 12px 0 4px; color: #9fb0bf; text-transform: uppercase; letter-spacing: .04em; }}
  .meta {{ color: #7c8a97; margin-bottom: 12px; }}
  .grid {{ display: grid; grid-template-columns: 3fr 1fr; gap: 12px; align-items: start; }}
  .panel {{ background: #121821; border: 1px solid #223; border-radius: 6px; padding: 8px; }}
  canvas {{ display: block; width: 100%; background: #0e131a; border-radius: 4px; }}
  table {{ border-collapse: collapse; width: 100%; font-size: 12px; }}
  td {{ padding: 1px 4px; text-align: right; }}
  td.bid {{ color: #3ecf8e; }}
  td.ask {{ color: #e5534b; }}
  .full {{ grid-column: 1 / -1; }}
</style>
<h1>wsprobe panel — read-path без фронта (M-46)</h1>
<div class="meta">
  schema_version={sv} · cursor={cur:?} · history_start_seq={hss} · history_truncated={ht} ·
  latency_first_snapshot_ms={lat} · frames_received={fr}
</div>
<div class="grid">
  <div class="panel full">
    <h2>Heatmap</h2>
    <canvas id="heatmap" width="1000" height="360"></canvas>
  </div>
  <div class="panel">
    <h2>Candles + VWAP</h2>
    <canvas id="candles" width="720" height="220"></canvas>
  </div>
  <div class="panel">
    <h2>Volume Profile</h2>
    <canvas id="vp" width="220" height="220"></canvas>
  </div>
  <div class="panel full">
    <h2>CVD</h2>
    <canvas id="cvd" width="1000" height="120"></canvas>
  </div>
  <div class="panel full">
    <h2>COB (top 10 each side)</h2>
    <table>
      <thead><tr><th colspan="2">BID</th><th colspan="2">ASK</th></tr></thead>
      <tbody>
{cob_rows}
      </tbody>
    </table>
  </div>
</div>
<script>
const DATA = {data_json};

function ctxOf(id) {{
  const c = document.getElementById(id);
  return [c.getContext('2d'), c.width, c.height];
}}

function drawHeatmap() {{
  const [ctx, w, h] = ctxOf('heatmap');
  ctx.clearRect(0, 0, w, h);
  const cells = DATA.heatmap;
  if (!cells.length) {{ ctx.fillStyle = '#889'; ctx.fillText('heatmap: no data', 10, 20); return; }}
  const times = [...new Set(cells.map(c => c.time_s))].sort((a, b) => a - b);
  const tIdx = new Map(times.map((t, i) => [t, i]));
  const cols = Math.max(times.length, 1);
  const colW = w / cols;
  const prices = cells.map(c => c.price);
  const pmin = Math.min(...prices), pmax = Math.max(...prices) || pmin + 1;
  const maxSize = Math.max(...cells.map(c => c.size), 1e-9);
  for (const c of cells) {{
    const x = tIdx.get(c.time_s) * colW;
    const frac = (c.price - pmin) / ((pmax - pmin) || 1);
    const y = h - frac * h;
    const inten = Math.min(1, c.size / maxSize);
    const hue = c.side === 'bid' ? 140 : 0;
    ctx.fillStyle = `hsla(${{hue}},80%,45%,${{0.12 + 0.85 * inten}})`;
    ctx.fillRect(x, y - 2, Math.max(colW, 2), 4);
  }}
}}

function drawCandles() {{
  const [ctx, w, h] = ctxOf('candles');
  ctx.clearRect(0, 0, w, h);
  const rows = DATA.ohlcv, vwap = DATA.vwap;
  if (!rows.length) {{ ctx.fillStyle = '#889'; ctx.fillText('ohlcv: no data', 10, 20); return; }}
  let lo = Math.min(...rows.map(r => r.low));
  let hi = Math.max(...rows.map(r => r.high));
  if (vwap.length) {{
    lo = Math.min(lo, ...vwap.map(v => v[1]));
    hi = Math.max(hi, ...vwap.map(v => v[1]));
  }}
  if (hi === lo) hi = lo + 1;
  const y = p => h - ((p - lo) / (hi - lo)) * h;
  const cw = w / rows.length;
  rows.forEach((r, i) => {{
    const x = i * cw + cw / 2;
    ctx.strokeStyle = r.close >= r.open ? '#3ecf8e' : '#e5534b';
    ctx.beginPath(); ctx.moveTo(x, y(r.high)); ctx.lineTo(x, y(r.low)); ctx.stroke();
    ctx.fillStyle = ctx.strokeStyle;
    const bw = Math.max(cw * 0.6, 1);
    const top = y(Math.max(r.open, r.close));
    const bot = y(Math.min(r.open, r.close));
    ctx.fillRect(x - bw / 2, top, bw, Math.max(bot - top, 1));
  }});
  if (vwap.length) {{
    const tmin = rows[0].time_s, tmax = rows[rows.length - 1].time_s;
    ctx.strokeStyle = '#f2c94c'; ctx.lineWidth = 1.5; ctx.beginPath();
    vwap.forEach(([t, p], i) => {{
      const frac = tmax > tmin ? (t - tmin) / (tmax - tmin) : 0;
      const x = frac * w, yy = y(p);
      if (i === 0) ctx.moveTo(x, yy); else ctx.lineTo(x, yy);
    }});
    ctx.stroke();
  }}
}}

function drawCvd() {{
  const [ctx, w, h] = ctxOf('cvd');
  ctx.clearRect(0, 0, w, h);
  const cvd = DATA.cvd;
  if (!cvd.length) {{ ctx.fillStyle = '#889'; ctx.fillText('cvd: no data', 10, 20); return; }}
  const vmin = Math.min(0, ...cvd.map(c => c[1]));
  const vmax = Math.max(0, ...cvd.map(c => c[1])) || vmin + 1;
  const y = v => h - ((v - vmin) / ((vmax - vmin) || 1)) * h;
  const y0 = y(0);
  ctx.strokeStyle = '#445'; ctx.beginPath(); ctx.moveTo(0, y0); ctx.lineTo(w, y0); ctx.stroke();
  const cw = w / cvd.length;
  cvd.forEach(([t, v], i) => {{
    const x = i * cw;
    ctx.fillStyle = v >= 0 ? '#3ecf8e' : '#e5534b';
    const yy = y(v);
    ctx.fillRect(x, Math.min(y0, yy), Math.max(cw * 0.8, 1), Math.max(Math.abs(yy - y0), 1));
  }});
}}

function drawVp() {{
  const [ctx, w, h] = ctxOf('vp');
  ctx.clearRect(0, 0, w, h);
  const vp = DATA.volume_profile;
  if (!vp.length) {{ ctx.fillStyle = '#889'; ctx.fillText('volume_profile: no data', 10, 20); return; }}
  const last = vp[vp.length - 1];
  const bins = last.bins;
  if (!bins.length) {{ ctx.fillStyle = '#889'; ctx.fillText('volume_profile: empty session', 10, 20); return; }}
  const maxVol = Math.max(...bins.map(b => b[1]), 1e-9);
  const rowH = h / bins.length;
  bins.forEach((b, i) => {{
    const frac = b[1] / maxVol;
    ctx.fillStyle = Math.abs(b[0] - last.poc) < 1e-6 ? '#f2c94c' : '#5b8def';
    ctx.fillRect(0, i * rowH, frac * w, Math.max(rowH * 0.8, 1));
  }});
}}

drawHeatmap();
drawCandles();
drawCvd();
drawVp();
</script>
"##,
        sv = summary.schema_version,
        cur = summary.cursor_upto_seq,
        hss = summary.history_start_seq,
        ht = summary.history_truncated,
        lat = summary.latency_first_snapshot_ms,
        fr = summary.frames_received,
    )
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
