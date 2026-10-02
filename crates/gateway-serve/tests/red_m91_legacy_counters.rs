//! RED `M-91` (sacred, architect-only) — **legacy-путь выдачи считает клиентов ТАК ЖЕ, как
//! v1-путь: попытку — до проверок, отказ — классом исхода, успех — после отправки снимка**
//! (`TD-228`).
//!
//! # Дефект — замер на проде (`R-210` §5, `R-211` §5.2)
//!
//! Две подписки, отвергнутые `not_ready`, и три УСПЕШНЫЕ (снимок доставлен) оставили сердцебиение
//! выдачи с `attempts:0, successes:0, refusals_supported:0`. `inc_attempts`/`inc_successes`/
//! `inc_refusals_*` стоят только в `handle_v1_message`; в `run_authorized_session` (legacy:
//! клиент молчит grace-окно, селектор из окружения) — ни одного вызова ни на отказе, ни на
//! успехе. Единственный реальный клиент (`wsprobe`) ходит legacy-путём ⇒ правило тишины
//! `OPS-I-8` (дельты `attempts`/`successes`/`refusals_supported`) слепо ко ВСЕМУ трафику.
//!
//! # Класс отказа — решение architect'а, не новое поле
//!
//! v1-путь (M-87) уже относит `not_ready`/`warming` к `refusals_supported` — «поддерживаемый
//! запрос, которому отказали» — и правило тишины построено ровно на этой величине. Legacy-путь
//! селектор не выбирает (берёт серверный), поэтому `refusals_unsupported` у него не возникает.
//! Новой величины и смены `schema` сердцебиения НЕ требуется: требуется симметрия.
//!
//! # Мера — РАВЕНСТВО дельт на ручке ЭКЗЕМПЛЯРА
//!
//! Равенство, а не `> 0`: ассерт «выросло» проходит и при двойном счёте (успех засчитан дважды
//! или попытка — на каждом тике). Ручка экземпляра (`counters_handle`, `I-5` M-89) — тесты
//! изолированы, `SERIAL` не нужен.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::StreamExt;
use gateway::Selector;
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::metrics::{ServingCounters, ServingCountersHandle};
use gateway_serve::server::{bind_with_policy, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::Value;
use std::sync::Arc;

const SECRET: &[u8] = b"m91-legacy-counters";
const FUTURE: usize = 4_000_000_000;
const BASE_MS: i64 = 1_784_116_800_000;
const BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
const CANONICAL: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m91".to_string(),
            exp: FUTURE,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt")
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 30,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m91".to_string(),
        epoch_id: "own-test".to_string(),
    }
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
        max_concurrent_serves: 2,
        max_tail_events: 1_000,
        expected_warmup_events: 100,
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(65_000.0 + (i % 20) as f64),
            size: to_fixed(0.5),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: BASE_MS + i as i64 * 100,
        },
    )
}

/// Журнал из 300 событий; `warm` ⇒ слепок по серверному селектору + хвост 40 событий
/// (`readiness = Ready`); иначе каталог слепков ПУСТ (`readiness = NotReady`).
fn fixture(warm: bool) -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..300u64 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let ckpt = tempfile::tempdir().expect("ckpt");
    if warm {
        gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
            .expect("advance");
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("reopen");
        for i in 300..340u64 {
            j.append(trade(i)).expect("append tail");
        }
        j.flush().expect("flush");
    }
    let files = std::fs::read_dir(ckpt.path()).expect("ckpt dir").count();
    assert_eq!(
        files > 0,
        warm,
        "SETUP НЕ СОСТОЯЛСЯ: каталог слепков содержит {files} файлов при warm={warm}"
    );
    (dir, ckpt)
}

async fn serve(
    dir: &std::path::Path,
    ckpt: &std::path::Path,
) -> (String, Arc<ServingCountersHandle>) {
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(ckpt.to_path_buf()),
    };
    let server = bind_with_policy(cfg, policy())
        .await
        .expect("bind_with_policy");
    let addr = server.local_addr().to_string();
    let counters = server
        .counters_handle()
        .expect("bind_with_policy обязан дать ручку счётчиков экземпляра");
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    (addr, counters)
}

#[derive(Debug, PartialEq)]
enum Legacy {
    /// OLD wire `{"Snapshot":…}` — обслужен.
    Snapshot,
    /// v1-форма ошибки с этим `code` — отказ.
    Refused(String),
}

/// Legacy-клиент: подключается и МОЛЧИТ grace-окно ⇒ сервер берёт селектор из окружения.
/// Возвращает исход по ПЕРВОМУ сообщению сервера и закрывает соединение.
async fn legacy_session(addr: &str) -> Legacy {
    let url = format!("ws://{addr}/?token={}", sign());
    let (mut ws, _) = tokio::time::timeout(BUDGET, tokio_tungstenite::connect_async(url))
        .await
        .expect("connect timeout")
        .expect("connect");
    let first: Value = match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref())
            .unwrap_or_else(|e| panic!("SETUP НЕ СОСТОЯЛСЯ: первое сообщение — не JSON: {e}")),
        other => panic!("SETUP НЕ СОСТОЯЛСЯ: сервер промолчал / закрыл ({other:?})"),
    };
    let out = if first.get("Snapshot").is_some() {
        Legacy::Snapshot
    } else if first.get("type").and_then(Value::as_str) == Some("error") {
        Legacy::Refused(
            first
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        )
    } else {
        panic!("SETUP НЕ СОСТОЯЛСЯ: первое сообщение не OLD-wire Snapshot и не ошибка: {first}")
    };
    let _ = ws.close(None).await;
    out
}

/// Дождаться, пока сессия сервера завершит учёт (успех считается ПОСЛЕ отправки снимка).
async fn settle() {
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
}

fn delta(a: &ServingCounters, b: &ServingCounters) -> (u64, u64, u64, u64) {
    (
        b.attempts - a.attempts,
        b.successes - a.successes,
        b.refusals_supported - a.refusals_supported,
        b.refusals_unsupported - a.refusals_unsupported,
    )
}

/// **`l1` — успешная legacy-сессия: ровно одна попытка и один успех, отказов нет.**
/// Сегодня RED: `(0, 0, 0, 0)`.
#[tokio::test]
async fn l1_legacy_success_counts_one_attempt_and_one_success() {
    let (dir, ckpt) = fixture(true);
    let (addr, counters) = serve(dir.path(), ckpt.path()).await;
    let before = counters.snapshot();
    let outcome = legacy_session(&addr).await;
    assert_eq!(
        outcome,
        Legacy::Snapshot,
        "SETUP НЕ СОСТОЯЛСЯ: при тёплом слепке legacy-сессия не обслужена"
    );
    settle().await;
    let after = counters.snapshot();
    assert_eq!(
        delta(&before, &after),
        (1, 1, 0, 0),
        "TD-228: успешная legacy-сессия (снимок доставлен) дала дельты \
         (attempts, successes, refusals_supported, refusals_unsupported) = {:?} вместо (1, 1, 0, 0). \
         Сердцебиение не видит обслуженного клиента — правило тишины OPS-I-8 слепо к реальному трафику",
        delta(&before, &after)
    );
}

/// **`l2` — отказ `not_ready` в legacy-пути: одна попытка, один отказ класса
/// `refusals_supported`, успехов ноль** — ровно дельта, на которой срабатывает правило тишины.
/// Сегодня RED: `(0, 0, 0, 0)`.
#[tokio::test]
async fn l2_legacy_not_ready_counts_attempt_and_supported_refusal() {
    let (dir, ckpt) = fixture(false);
    let (addr, counters) = serve(dir.path(), ckpt.path()).await;
    let before = counters.snapshot();
    let outcome = legacy_session(&addr).await;
    assert_eq!(
        outcome,
        Legacy::Refused("not_ready".to_string()),
        "SETUP НЕ СОСТОЯЛСЯ: без слепка legacy-сессия ожидалась отказом not_ready"
    );
    settle().await;
    let after = counters.snapshot();
    assert_eq!(
        delta(&before, &after),
        (1, 0, 1, 0),
        "TD-228: отказ not_ready в legacy-пути дал дельты {:?} вместо (1, 0, 1, 0). \
         Класс — refusals_supported, как в v1-пути (M-87): «поддерживаемый запрос, которому \
         отказали». Без этого «отказано всем» неотличимо от «клиентов нет»",
        delta(&before, &after)
    );
}

/// **`l3` — множественность: две успешные legacy-сессии подряд — РОВНО (2, 2, 0, 0).**
/// Ловит и пропуск, и двойной счёт (успех на каждом тике push-цикла, попытка на каждом
/// сообщении), которые `l1` с одной сессией может не различить.
#[tokio::test]
async fn l3_two_legacy_sessions_count_exactly_two() {
    let (dir, ckpt) = fixture(true);
    let (addr, counters) = serve(dir.path(), ckpt.path()).await;
    let before = counters.snapshot();
    for _ in 0..2 {
        assert_eq!(
            legacy_session(&addr).await,
            Legacy::Snapshot,
            "SETUP: сессия не обслужена"
        );
    }
    settle().await;
    let after = counters.snapshot();
    assert_eq!(
        delta(&before, &after),
        (2, 2, 0, 0),
        "TD-228: две успешные legacy-сессии дали {:?} вместо (2, 2, 0, 0)",
        delta(&before, &after)
    );
}

/// **`l4` — исход `Warming` в legacy-пути — тоже отказ класса `refusals_supported`: `(1, 0, 1, 0)`,
/// ответ `warming`** (`C-269`).
///
/// На текущем коде `admission::readiness` `Warming` не возвращает (исход объявлен для будущего
/// пути прогрева), поэтому мир строится ПОДСТАНОВКОЙ исхода: тестовый крючок
/// `Server::with_readiness_override` существует ТОЛЬКО под `feature = "testing"` — в прод-сборке
/// способа подменить проверку готовности нет (иначе это был бы обход предохранителя M-87).
/// Мутант, который ловится: «считать отказом только `NotReady`» — `l2` зелен, `l4` даёт `(1,0,0,0)`.
#[cfg(feature = "testing")]
#[tokio::test]
async fn l4_legacy_warming_is_a_supported_refusal() {
    use gateway_serve::admission::ServingOutcome;
    let (dir, ckpt) = fixture(true);
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(ckpt.path().to_path_buf()),
    };
    let server = bind_with_policy(cfg, policy())
        .await
        .expect("bind_with_policy")
        .with_readiness_override(|| ServingOutcome::Warming);
    let addr = server.local_addr().to_string();
    let counters = server.counters_handle().expect("ручка экземпляра");
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    let before = counters.snapshot();
    let outcome = legacy_session(&addr).await;
    assert_eq!(
        outcome,
        Legacy::Refused("warming".to_string()),
        "SETUP НЕ СОСТОЯЛСЯ: подставленный Warming не дал ответа warming — крючок не дошёл до legacy-пути"
    );
    settle().await;
    let after = counters.snapshot();
    assert_eq!(
        delta(&before, &after),
        (1, 0, 1, 0),
        "TD-228 / C-269: отказ warming в legacy-пути дал дельты {:?} вместо (1, 0, 1, 0) — правило \
         тишины OPS-I-8 не увидит отказ «сервер прогревается»",
        delta(&before, &after)
    );
}
