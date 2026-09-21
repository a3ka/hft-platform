//! RED M-87 (sacred, architect-only) — ФОРМА контракта допуска и бюджета.
//!
//! COMPILE-RED по построению: модулей `gateway_serve::admission` / `gateway_serve::metrics`
//! и поля `gateway::ReadStats.payload_bytes_read` в коде ЕЩЁ НЕТ. Спека —
//! `milestones/M-87-serving-circuit-breaker.md` §4, §5, §7.
//!
//! Зачем ОТДЕЛЬНЫЙ файл: предметный набор `crates/gateway/tests/red_m87_cold_path_reads_nothing.rs`
//! падает АССЕРТАМИ на сегодняшнем коде и доказывает, что дефект существует. Форма проверяется
//! здесь; в Rust каждый файл `tests/` — отдельный бинарник, поэтому компиляционный отказ здесь
//! не маскирует предметный результат там.
//!
//! ПОЧЕМУ ГРАНИЦА ПРОВЕДЕНА ИМЕННО ТАК (два разных вопроса — две разные функции):
//!  · `admit` — БЫСТРЫЙ путь без I/O: «вправе ли клиент это просить». Зеркалит нынешний
//!    `session::validate_selector`, который тоже чистый и вызывается ДО построения редьюсера;
//!  · `readiness` — «готово ли состояние», и он обязан узнать это, НЕ ЧИТАЯ журнала.
//! Слить их в одну функцию нельзя: первая обязана отвечать без единого обращения к диску,
//! вторая по определению смотрит на слепок.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::Selector;
use gateway_serve::admission::{admit, readiness, AdmissionPolicy, LiveProfile, ServingOutcome};
use gateway_serve::metrics::{serving_counters, silence_alarm, ServingCounters};
use journal::{EpochFilter, Journal, WriterConfig};

const T0: i64 = 1_752_000_000_000;

/// Канонические семь полос (`П-029`, 2026-09-10): сервер ВСЕГДА считает этот набор,
/// клиент выбирает, что ОТОБРАЖАТЬ, но сетку не задаёт.
const CANONICAL: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 16,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn journal_of(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..n {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::L2Snapshot {
                    bids: vec![Level {
                        price: to_fixed(65_000.0 + i as f64),
                        size: to_fixed(3.0),
                    }],
                    asks: vec![Level {
                        price: to_fixed(65_020.0 + i as f64),
                        size: to_fixed(4.0),
                    }],
                    ts_exch_ms: T0 + (i as i64) * 100,
                },
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn sel_with(symbol: &str, bands: Vec<f64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: symbol.to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
        depth_cadence_ms: None,
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        allowed_symbols: vec!["BTCUSDT".to_string()],
        canonical_bands: CANONICAL.to_vec(),
        // `C-234` R1: конечный перечень профилей. Без него селектор
        // `{ timeframe_ms: 1, window_ms: None }` проходит по символу и полосам,
        // оставаясь НЕОГРАНИЧЕННЫМ.
        allowed_profiles: vec![LiveProfile {
            timeframe_ms: 1_000,
            window_ms: 60_000,
            depth_cadence_ms: None,
        }],
        max_concurrent_serves: 4,
    }
}

// ─────────────────────── §4 — исходы названы, ни одного молчаливого ───────────────────────

/// Пять исходов существуют и РАЗЛИЧИМЫ. Молчаливый no-op запрещён: его нечем наблюдать,
/// а сторож молчания (§7) обязан уметь считать отказы по классам.
#[test]
fn form_five_outcomes_exist_and_differ() {
    let all = [
        ServingOutcome::Ready,
        ServingOutcome::Warming,
        ServingOutcome::NotReady,
        ServingOutcome::Unsupported,
        ServingOutcome::Overloaded,
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i != j {
                assert_ne!(
                    a, b,
                    "исходы {i} и {j} неразличимы — классификация фиктивна"
                );
            }
        }
    }
}

// ─────────────────────── §6 — политика допуска ОТКАЗЫВАЕТ, а не подменяет ───────────────────────

/// Неканонический набор полос ⇒ `Unsupported`. Сегодня не обеспечено ничем: оба валидатора
/// (`crates/gateway-serve/src/session.rs:70-106`, `crates/gateway/src/lib.rs:3022-3146`)
/// проверяют по полосам форму и ДЛИНУ, но не состав.
#[test]
fn form_non_canonical_bands_are_refused() {
    let p = policy();
    let odd = sel_with("BTCUSDT", vec![0.0123, 0.4567]);
    assert_eq!(
        admit(&p, &odd),
        ServingOutcome::Unsupported,
        "произвольный набор полос принят: клиент получает собственный отпечаток селектора, \
         то есть собственный холодный расчёт"
    );
}

/// И — ГЛАВНОЕ — параметры НЕ подменяются молча на поддержанные. Молча подставленный
/// набор даёт клиенту данные, которых он не просил (`CT-RFC-09` §2.7).
#[test]
fn form_refusal_does_not_substitute_parameters() {
    let p = policy();
    let odd = sel_with("BTCUSDT", vec![0.0123, 0.4567]);
    let before = odd.bands.clone();
    let _ = admit(&p, &odd);
    assert_eq!(
        odd.bands, before,
        "политика допуска изменила запрошенные полосы — это молчаливая подмена, а не отказ"
    );
}

/// Инструмент вне списка разрешённых ⇒ `Unsupported`, а не холодный расчёт.
#[test]
fn form_unknown_symbol_is_unsupported() {
    let p = policy();
    assert_eq!(
        admit(&p, &sel_with("DOGEUSDT", CANONICAL.to_vec())),
        ServingOutcome::Unsupported,
        "неразрешённый инструмент принят — один запрос открывает пересчёт произвольного \
         инструмента на журнале в десятки гигабайт"
    );
}

/// АНТИ-ПЛАЦЕБО: реализация «отказывать всегда» тривиально проходит три теста выше.
/// Здесь она обязана упасть.
#[test]
fn form_canonical_request_is_not_refused() {
    let p = policy();
    assert_ne!(
        admit(&p, &sel_with("BTCUSDT", CANONICAL.to_vec())),
        ServingOutcome::Unsupported,
        "канонический запрос разрешённого инструмента отвергнут — 'отказывать всегда' \
         решением не является"
    );
}

// ─────────────────────── §4 — готовность узнаётся БЕЗ чтения журнала ───────────────────────

/// Слепка нет ⇒ `NotReady`, и **прогрев НЕ создаётся автоматически**: иначе проблема
/// переезжает из соединения в очередь (спека §4).
#[test]
fn form_missing_snapshot_is_not_ready_and_starts_no_warmup() {
    let dir = journal_of(20);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let before: Vec<_> = std::fs::read_dir(ckpt.path())
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();

    let outcome = readiness(
        ckpt.path(),
        dir.path(),
        &sel_with("BTCUSDT", CANONICAL.to_vec()),
    );
    assert_eq!(
        outcome,
        ServingOutcome::NotReady,
        "отсутствие слепка обязано быть названным исходом"
    );

    let after: Vec<_> = std::fs::read_dir(ckpt.path())
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        after.len(),
        before.len(),
        "проверка готовности создала файлы в каталоге слепков — значит запустила прогрев \
         по запросу клиента"
    );
}

// ─────────────────────── §5 — бюджет считает ПРОЧИТАННЫЕ БАЙТЫ ───────────────────────

/// Счётчик декодированных событий недостаточен (план §15.1): сегмент можно открыть,
/// прочитать и распаковать, не вызвав декодер. Нужен счётчик БАЙТ полезной нагрузки.
/// Здесь проверяется, что он СУЩЕСТВУЕТ и ЖИВОЙ — на заведомо читающем пути он > 0.
#[test]
fn form_payload_bytes_are_counted_at_all() {
    let dir = journal_of(40);
    let s = sel_with("BTCUSDT", vec![0.001]);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let (_live, stats) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .expect("resume");
    assert!(
        stats.payload_bytes_read > 0,
        "счётчик прочитанных байт не растёт даже на полном реплее — мерить бюджет нечем, \
         а счётчик, который всегда ноль, обесценивает оракул отсутствия чтения"
    );
}

// ─────────────────────── §7 — сторож МОЛЧАНИЯ: ПРАВИЛО, не эмиссия ───────────────────────
//
// ГРАНИЦА НАЗВАНА (`C-234` R3): ниже судится ПРАВИЛО тревоги как чистая функция — что оно
// не звенит на мусорном трафике и на пустом окне, но звенит на голодании. Этого
// НЕДОСТАТОЧНО: `OPS-I-10` (`docs/fa/ops.md:478`) требует прогнать ПРОДЮСЕРА и предъявить
// значение. Эмиссию на реальном пути выдачи судит `red_m87_entrypoint.rs::c6_*`.

/// Успех считается РАБОТОЙ, а не кодом ответа; поддержанные и неподдержанные запросы
/// считаются ОТДЕЛЬНО — иначе поток мусора держит тревогу включённой и обесценивает её.
#[test]
fn form_silence_alarm_ignores_unsupported_traffic() {
    let noisy = ServingCounters {
        attempts: 1_000,
        successes: 0,
        refusals_supported: 0,
        refusals_unsupported: 1_000,
    };
    assert!(
        !silence_alarm(&noisy),
        "поток ЗАВЕДОМО неподдержанных запросов поднял тревогу молчания — сторож, который \
         звенит всегда, равен выключенному"
    );

    let starving = ServingCounters {
        attempts: 1_000,
        successes: 0,
        refusals_supported: 1_000,
        refusals_unsupported: 0,
    };
    assert!(
        silence_alarm(&starving),
        "просили ПОДДЕРЖАННОЕ тысячу раз, не получили ни разу — это ровно тот случай, \
         ради которого сторож существует (авария TD-206 прожила месяц при трёх зелёных \
         признаках жизни)"
    );
}

/// Отсутствие спроса — НЕ тревога и НЕ успех. Различение обязано быть выразимо:
/// «никто не просил» ≠ «просили, не получили».
#[test]
fn form_no_demand_is_not_an_alarm() {
    let idle = ServingCounters {
        attempts: 0,
        successes: 0,
        refusals_supported: 0,
        refusals_unsupported: 0,
    };
    assert!(
        !silence_alarm(&idle),
        "пустое окно без единой попытки поднято как тревога — сторож не различает \
         'никто не просил' и 'просили, не получили'"
    );
}

/// Счётчики доступны снаружи процесса-наблюдателя: сторож, чьи числа нельзя снять,
/// наблюдением не является.
#[test]
fn form_counters_are_readable() {
    let c = serving_counters();
    assert!(
        c.successes <= c.attempts,
        "успехов больше, чем попыток ({} > {}) — счётчики несогласованы",
        c.successes,
        c.attempts
    );
}

// ─────────────── §7.1 — `TD-207`: зонд и сервер обязаны трактовать секрет ОДИНАКОВО ───────────────

/// Дефект: сервер строит ключ из СЫРЫХ байт строки
/// (`DecodingKey::from_secret(secret.as_bytes())`, `crates/gateway-serve/src/lib.rs:2413`),
/// а зонд строку «только hex-символы чётной длины» ДЕКОДИРУЕТ как hex
/// (`crates/gateway-serve/src/bin/wsprobe.rs:156-160`). Прод-секрет — ровно 64 hex-символа
/// ⇒ зонд подписывает 32 байтами, сервер проверяет 64.
///
/// **Почему фикс — ОБЩАЯ функция, а не правка зонда.** Разбор секрета сегодня живёт в
/// БИНАРЕ и из библиотеки недоступен (`parse_secret` приватна, `wsprobe.rs:155`), поэтому
/// две стороны физически не могут разделить одну трактовку — и разошлись. Пока трактовок
/// две, починка одной стороны воспроизведёт тот же класс при следующем изменении.
/// Дефект описан ещё в `CT-RFC-09` §4 (2026-08-03) и заведён в реестр ТРЕТИЙ раз
/// (`TD-084` → `TD-095` отклонён как дубль → `TD-207`): запись в реестр его не закрывает.
///
/// COMPILE-RED: `gateway_serve::auth::key_material` не существует.
#[test]
fn form_secret_has_exactly_one_interpretation() {
    use gateway_serve::auth::{key_material, verify_token, Claims};
    use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};

    // ПРОД-ФОРМА: ровно 64 hex-символа (`CT-RFC-09` §4 п.1 — `/root/hft-platform/.env`).
    let prod_secret = "0123456789abcdef".repeat(4);
    assert_eq!(
        prod_secret.len(),
        64,
        "setup-страж: прод-форма — 64 символа"
    );
    assert!(
        prod_secret.chars().all(|c| c.is_ascii_hexdigit()),
        "setup-страж: прод-форма состоит только из hex-цифр — иначе дефект не воспроизводится"
    );

    let material = key_material(&prod_secret);
    let token = encode(
        &Header::default(),
        &Claims {
            sub: "m87-probe".to_string(),
            exp: 9_999_999_999,
        },
        &EncodingKey::from_secret(&material),
    )
    .expect("sign");

    // Сервер строит ключ ИЗ ТОГО ЖЕ материала — одна функция, одна трактовка.
    let key = DecodingKey::from_secret(&material);
    assert!(
        verify_token(&token, &key).is_ok(),
        "токен, подписанный общей трактовкой секрета, не принят сервером: трактовок \
         по-прежнему две (TD-207)"
    );
}

/// Профиль вне разрешённого перечня ⇒ `Unsupported` (`C-234` R1). Символ и полосы здесь
/// КАНОНИЧЕСКИЕ — отклонить такой запрос способна только проверка ПРОФИЛЯ.
#[test]
fn form_unbounded_profile_is_refused_even_with_canonical_bands() {
    let p = policy();
    let unbounded = Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1,
        bands: CANONICAL.to_vec(),
        window_ms: None,
        depth_cadence_ms: None,
    };
    assert_eq!(
        admit(&p, &unbounded),
        ServingOutcome::Unsupported,
        "селектор с window_ms=None и timeframe_ms=1 принят: это unbounded offline-свёртка \
         (crates/gateway/src/lib.rs:287-305), то есть публичный путь к полной истории"
    );
}
