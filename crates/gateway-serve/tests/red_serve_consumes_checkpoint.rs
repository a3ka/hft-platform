//! RED M-38b rev4 (sacred, architect-only) — **B3 (reviewer PR-гейт): прод-путь `gateway-serve`
//! ОБЯЗАН потреблять чекпоинт, иначе Objective milestone'а недостижим.**
//!
//! ## Это пробел МОЕЙ спеки, а не забывчивость dev'а
//!
//! §Tasks rev1-rev3 содержали задачу #4 («библиотечная `snapshot_from_checkpoint`») и задачу #6
//! («резюмируемый редьюсер в live-цикле»), но задачи «snapshot-при-подключении читает чекпоинт»
//! НЕ БЫЛО. Dev выполнил §Tasks буквально и корректно. Замер reviewer'а: `snapshot_from_checkpoint`
//! имеет **0 call-site вне `tests/`**, `serve::snapshot_msg` по-прежнему зовёт
//! `gateway::snapshot(..)` = O(история), ckpt-том сервису `gateway-serve` в compose не смонтирован,
//! env с путём к чекпоинту нет. То есть заявленная цель «первый Snapshot секунды вместо 409.74 s»
//! на этом коде недостижима, хотя все 32 проверки гейта зелёные.
//!
//! **Решение architect'а по вопросу reviewer'а «M-38b или отдельный milestone»:** это ЗАДАЧА
//! M-38b. Milestone, не достигающий собственного Objective, закрывать нельзя; выносить
//! потребление чекпоинта в отдельный milestone означало бы смержить инфраструктуру, которая
//! никем не используется, и оставить TD-044 открытым при «зелёном» M-38b — ровно тот
//! бухгалтерский обман, который §8 и придуман ловить.
//!
//! ## Что здесь проверяется
//!
//! 1. **Проводка env → конфиг** (анти-инерт, урок TD-019/TD-020, зеркально
//!    `red_serve_window_wiring`): `GATEWAY_CHECKPOINT_DIR` доходит до `ServeConfig`.
//! 2. **Поведение**: `serve::snapshot_msg` с чекпоинтом у хвоста декодирует ХВОСТ, а не историю
//!    (`ReadStats`), и отдаёт БАЙТ-ИДЕНТИЧНЫЙ снапшот. Канарейки-grep для этого недостаточно —
//!    именно grep-канарейки и пропустили B1/B3.
//! 3. **Парный vantage**: без чекпоинта путь обязан продолжать работать (полный реплей) —
//!    чекпоинт это КЭШ (GW-I-9б), его отсутствие не ломает сервис.
//!
//! COMPILE-RED: `ServeConfig.checkpoint_dir` и параметр `ckpt_dir` + возврат `ReadStats`
//! у `serve::snapshot_msg` ещё не существуют.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use gateway_serve::serve_config_from_env;
use journal::{EpochFilter, Journal, WriterConfig};
use std::collections::HashMap;

const N: u64 = 1_500;

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 8 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(100.0 + (i % 5) as f64),
            size: to_fixed(1.0),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_752_000_000_000 + i as i64 * 100,
        },
    )
}

fn journal_of(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..n {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Проводка env → ServeConfig (анти-инерт)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn checkpoint_dir_env_flows_to_config() {
    let cfg = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        // `CT-RFC-09` §2.6: `max_subscriptions_per_connection` — конфиг, ОТСУТСТВИЕ
        // либо невалидное значение ⇒ отказ старта (задача 13 N-2). Прод всегда его
        // подаёт (`docker-compose.yml`, дефолт 16), поэтому фикстура БЕЗ переменной
        // никогда не была прод-формой (`testing.md` §«Форма прода снимается ЗАМЕРОМ»).
        // Ассерты ниже про лимит ничего не утверждают — добавление их не ослабляет.
        ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
        ("GATEWAY_CHECKPOINT_DIR", "/ckpt"),
    ]))
    .expect("config собран");
    assert_eq!(
        cfg.checkpoint_dir.as_deref(),
        Some(std::path::Path::new("/ckpt")),
        "GATEWAY_CHECKPOINT_DIR не дошёл до ServeConfig — прод-сервис не узнает о чекпоинте, \
         и первый Snapshot останется O(история) (TD-044). Класс TD-020: механизм есть, \
         никто не зовёт."
    );
}

/// Парный vantage: без переменной сервис обязан работать (чекпоинт — кэш, а не предусловие).
#[test]
fn absent_checkpoint_dir_is_not_an_error() {
    let cfg = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        // `CT-RFC-09` §2.6: `max_subscriptions_per_connection` — конфиг, ОТСУТСТВИЕ
        // либо невалидное значение ⇒ отказ старта (задача 13 N-2). Прод всегда его
        // подаёт (`docker-compose.yml`, дефолт 16), поэтому фикстура БЕЗ переменной
        // никогда не была прод-формой (`testing.md` §«Форма прода снимается ЗАМЕРОМ»).
        // Ассерты ниже про лимит ничего не утверждают — добавление их не ослабляет.
        ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
    ]))
    .expect("config без GATEWAY_CHECKPOINT_DIR обязан собираться");
    assert_eq!(
        cfg.checkpoint_dir, None,
        "без переменной — None (полный реплей), а не выдуманный путь"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. ПОВЕДЕНИЕ: snapshot-при-подключении реально потребляет чекпоинт
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn snapshot_msg_consumes_checkpoint_and_reads_only_tail() {
    let jdir = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");

    // Чекпоинт почти у хвоста.
    let k = Cursor {
        upto_seq: Some(N - 1 - 20),
    };
    gateway::checkpoint::advance_to(
        jdir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        k,
    )
    .expect("advance_to");

    let (msg, stats) = gateway_serve::serve::snapshot_msg(
        jdir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
        Some(ckpt.path()),
    )
    .expect("snapshot_msg с чекпоинтом");

    assert!(
        stats.events_decoded < N,
        "B3 НАРУШЕН: snapshot-при-подключении декодировал {} событий из {N} — чекпоинт не \
         потребляется, путь остался O(история). Именно это и есть TD-044 (прод: 409.74 s), \
         ради которого существует M-38b.",
        stats.events_decoded
    );
    assert!(
        stats.events_decoded <= 100,
        "чекпоинт стоит в 20 событиях от конца — декодировано {} (бюджет 100)",
        stats.events_decoded
    );

    // Байт-идентичность: ускорение не имеет права менять данные (GW-I-9а / VB-I-2).
    let (msg_full, stats_full) = gateway_serve::serve::snapshot_msg(
        jdir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
        None,
    )
    .expect("snapshot_msg без чекпоинта");
    assert_eq!(
        serde_json::to_string(&msg).expect("ser"),
        serde_json::to_string(&msg_full).expect("ser"),
        "снапшот из чекпоинта обязан быть байт-идентичен полному реплею"
    );
    assert_eq!(
        stats_full.events_decoded, N,
        "парный vantage: без чекпоинта декодируется весь журнал ({N}) — счётчик честен"
    );
}
