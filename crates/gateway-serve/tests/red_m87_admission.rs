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

/// Политика для сценариев ЭТОГО файла: пороги заведомо щедрые.
///
/// Круг `R-196` B2 сделал порог свежести параметром политики, и `readiness` теперь его
/// принимает. Сценарии ниже судят ПРИГОДНОСТЬ СЛЕПКА (нет/битый/чужая версия), а не
/// свежесть: если бы они молча унаследовали строгий порог, их исход определялся бы
/// посторонней величиной, и файл начал бы мерить окружение вместо своего предмета
/// (`testing.md`, целостность гейта п.2). Предмет свежести живёт отдельно —
/// `red_m87_staleness_budget.rs`.
fn pol() -> AdmissionPolicy {
    AdmissionPolicy {
        allowed_symbols: vec!["BTCUSDT".to_string()],
        canonical_bands: vec![0.001],
        allowed_profiles: vec![LiveProfile {
            timeframe_ms: 1_000,
            window_ms: 60_000,
            depth_cadence_ms: None,
        }],
        max_concurrent_serves: 1,
        max_tail_events: 1_000_000,
        expected_warmup_events: 1,
    }
}

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
        &pol(),
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
        journal_payload_bytes_read: 0,
        slots_in_flight: 0,
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
        journal_payload_bytes_read: 0,
        slots_in_flight: 0,
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
        journal_payload_bytes_read: 0,
        slots_in_flight: 0,
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

// ════════ У-2 (`A-037`) — четыре состояния слепка ПЕРЕЕХАЛИ сюда как юнит-оракулы ════════
//
// Прежний файл `crates/gateway/tests/red_m87_cold_path_reads_nothing.rs` ИЗЪЯТ: он пиннил на
// ОБЩЕЙ функции `LiveReducer::resume` поведение, противоположное двум другим sacred-оракулам
// того же корпуса на той же функции. Набор был невыполним против самого себя: позеленеть он
// мог только правкой общей библиотеки, которую §10 спеки запрещает, а `A-033` признал
// негодной. Дефект конструкции, а не изложения; найден гейтом, решён арбитражем.
//
// Здесь те же четыре состояния судятся там, где предохранитель ЖИВЁТ, — в транспорте, через
// `readiness()`, которая на `LiveReducer::resume` не смотрит вовсе.

fn poison_first_segment(dir: &std::path::Path) {
    let mut segs: Vec<_> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jrnl"))
        .collect();
    segs.sort();
    let target = segs.first().expect("setup-страж: сегментов нет");
    let mut bytes = std::fs::read(target).expect("read segment");
    assert!(bytes.len() > 512, "setup-страж: сегмент короче 512 Б");
    let mid = bytes.len() / 2;
    for b in bytes.iter_mut().skip(mid).take(64) {
        *b = !*b;
    }
    std::fs::write(target, &bytes).expect("write poisoned");
}

/// Состояние 1 — слепка нет. `readiness` обязана ответить исходом, НЕ создав прогрев и НЕ
/// прочитав полезной нагрузки: журнал под ловушкой, любое чтение содержимого упало бы.
#[test]
fn u2_readiness_missing_checkpoint_over_trap() {
    let dir = journal_of(40);
    poison_first_segment(dir.path());
    let ckpt = tempfile::tempdir().expect("ckpt");

    let before = std::fs::read_dir(ckpt.path()).expect("read_dir").count();
    let outcome = readiness(
        ckpt.path(),
        dir.path(),
        &sel_with("BTCUSDT", vec![0.001]),
        &pol(),
    );
    let after = std::fs::read_dir(ckpt.path()).expect("read_dir").count();

    assert_eq!(
        outcome,
        ServingOutcome::NotReady,
        "отсутствие слепка обязано давать НАЗВАННЫЙ исход"
    );
    assert_eq!(
        after, before,
        "проверка готовности создала файлы в каталоге слепков — значит запустила прогрев"
    );
}

/// Состояние 2 — слепок повреждён.
#[test]
fn u2_readiness_corrupt_checkpoint_over_trap() {
    let dir = journal_of(40);
    poison_first_segment(dir.path());
    let ckpt = tempfile::tempdir().expect("ckpt");
    let s = sel_with("BTCUSDT", vec![0.001]);
    std::fs::write(
        ckpt.path().join(format!(
            "ckpt-{:016x}.bin",
            gateway::checkpoint::selector_fingerprint(&s)
        )),
        vec![0xABu8; 4096],
    )
    .expect("write corrupt");
    assert_eq!(
        readiness(ckpt.path(), dir.path(), &s, &pol()),
        ServingOutcome::NotReady,
        "повреждённый слепок обязан давать названный исход, а не уводить в пересчёт"
    );
}

/// Состояние 3 — слепок несовместим по версии провода.
#[test]
fn u2_readiness_incompatible_checkpoint() {
    let dir = journal_of(40);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let s = sel_with("BTCUSDT", vec![0.001]);
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &s, EpochFilter::OwnCaptureOnly)
        .expect("advance");
    let path = ckpt.path().join(format!(
        "ckpt-{:016x}.bin",
        gateway::checkpoint::selector_fingerprint(&s)
    ));
    let mut bytes = std::fs::read(&path).expect("read");
    let declared = u32::from_le_bytes(bytes[12..16].try_into().expect("4 байта"));
    assert_eq!(
        declared,
        gateway::GATEWAY_SCHEMA_VERSION,
        "setup-страж: не то поле"
    );
    bytes[12..16].copy_from_slice(&(declared + 1).to_le_bytes());
    std::fs::write(&path, &bytes).expect("write");
    poison_first_segment(dir.path());

    assert_eq!(
        readiness(ckpt.path(), dir.path(), &s, &pol()),
        ServingOutcome::NotReady,
        "несовместимый по версии слепок обязан давать названный исход"
    );
}

/// Состояние 4 — слепок валиден, но отстал сверх бюджета докормки. Порог — КОНФИГ политики,
/// не константа теста (`A-037` D-1, следствие 1).
#[test]
fn u2_readiness_stale_beyond_budget() {
    let dir = journal_of(40);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let s = sel_with("BTCUSDT", vec![0.001]);
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &s, EpochFilter::OwnCaptureOnly)
        .expect("advance");
    // Журнал уезжает далеко вперёд.
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..500u64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::L2Snapshot {
                    bids: vec![Level {
                        price: to_fixed(64_900.0 + i as f64),
                        size: to_fixed(1.0),
                    }],
                    asks: vec![Level {
                        price: to_fixed(65_100.0 + i as f64),
                        size: to_fixed(1.0),
                    }],
                    ts_exch_ms: T0 + 1_000_000 + (i as i64) * 100,
                },
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }
    assert_eq!(
        readiness(ckpt.path(), dir.path(), &s, &pol()),
        ServingOutcome::NotReady,
        "слепок, отставший сверх бюджета докормки, обязан давать названный исход: иначе \
         'валидный слепок' становится обходным путём к тому же неограниченному пересчёту"
    );
}

/// АНТИ-ПЛАЦЕБО к четырём выше: свежий пригодный слепок обязан давать `Ready`.
/// Без него реализация `readiness → NotReady` всегда проходит все четыре.
#[test]
fn u2_readiness_fresh_checkpoint_is_ready() {
    let dir = journal_of(40);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let s = sel_with("BTCUSDT", vec![0.001]);
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &s, EpochFilter::OwnCaptureOnly)
        .expect("advance");
    assert_eq!(
        readiness(ckpt.path(), dir.path(), &s, &pol()),
        ServingOutcome::Ready,
        "свежий пригодный слепок не признан готовым — 'отказывать всегда' решением не является"
    );
}

// ════════ У-5 (`A-037`) — каждый объявленный тип имеет ОРАКУЛ ════════

/// Бюджет исчерпан по событиям ⇒ работа остановлена с НАЗВАННОЙ причиной.
#[test]
fn u5_budget_exhausted_by_events_is_named() {
    use gateway_serve::admission::{BudgetStop, CallBudget};
    let budget = CallBudget {
        max_events: 8,
        max_payload_bytes: u64::MAX,
        max_wall_ms: u64::MAX,
        max_output_bytes: u64::MAX,
        max_state_bytes: u64::MAX,
    };
    let dir = journal_of(200);
    let stop = gateway_serve::admission::feed_tail_within(
        &dir,
        &sel_with("BTCUSDT", vec![0.001]),
        budget,
        &NeverCancel,
    );
    assert_eq!(
        stop,
        Some(BudgetStop::Events),
        "исчерпание бюджета по событиям обязано быть НАЗВАНО, а не проявиться усечением"
    );
}

/// Бюджет исчерпан по ПРОЧИТАННЫМ БАЙТАМ ⇒ та же дисциплина. Счётчик событий этого не
/// ловит (план §15.1): сегмент можно прочитать, не декодируя.
#[test]
fn u5_budget_exhausted_by_payload_bytes_is_named() {
    use gateway_serve::admission::{BudgetStop, CallBudget};
    let budget = CallBudget {
        max_events: u64::MAX,
        max_payload_bytes: 512,
        max_wall_ms: u64::MAX,
        max_output_bytes: u64::MAX,
        max_state_bytes: u64::MAX,
    };
    let dir = journal_of(200);
    let stop = gateway_serve::admission::feed_tail_within(
        &dir,
        &sel_with("BTCUSDT", vec![0.001]),
        budget,
        &NeverCancel,
    );
    assert_eq!(
        stop,
        Some(BudgetStop::PayloadBytes),
        "предел по байтам не назван"
    );
}

/// Кооперативная отмена: признак, взведённый после k порций, останавливает обработчик
/// ДО конца хвоста. Внешне прервать блокирующую задачу нельзя — это факт о рантайме;
/// проверять признак МЕЖДУ порциями можно и нужно (`A-037` У-5).
#[test]
fn u5_cooperative_cancel_stops_between_chunks() {
    use gateway_serve::admission::{BudgetStop, CallBudget, Cancel};
    struct AfterK {
        seen: std::sync::atomic::AtomicUsize,
        k: usize,
    }
    impl Cancel for AfterK {
        fn cancelled(&self) -> bool {
            self.seen.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= self.k
        }
    }
    let budget = CallBudget {
        max_events: u64::MAX,
        max_payload_bytes: u64::MAX,
        max_wall_ms: u64::MAX,
        max_output_bytes: u64::MAX,
        max_state_bytes: u64::MAX,
    };
    let dir = journal_of(400);
    let cancel = AfterK {
        seen: std::sync::atomic::AtomicUsize::new(0),
        k: 2,
    };
    let stop = gateway_serve::admission::feed_tail_within(
        &dir,
        &sel_with("BTCUSDT", vec![0.001]),
        budget,
        &cancel,
    );
    assert_eq!(
        stop,
        Some(BudgetStop::Cancelled),
        "признак отмены не остановил обработчик между порциями"
    );
    assert!(
        cancel.seen.load(std::sync::atomic::Ordering::Relaxed) >= 2,
        "признак отмены не опрашивался между порциями вовсе"
    );
}

struct NeverCancel;
impl gateway_serve::admission::Cancel for NeverCancel {
    fn cancelled(&self) -> bool {
        false
    }
}

/// Слот: занят — отказ; освобождён — обслуживание. ДЕТЕРМИНИРОВАННО, без гонок: слот
/// удерживается САМИМ тестом, а не «долгой работой», исход которой зависит от хоста
/// (`testing.md`: гейт меряет свой инвариант, не окружение; `A-037` У-6).
#[test]
fn u6_slot_held_by_work_not_released_by_waiting() {
    use gateway_serve::admission::ServingSlots;
    let slots = ServingSlots::new(1);
    let guard = slots.try_acquire().expect("первый слот обязан выдаваться");
    assert_eq!(slots.in_flight(), 1, "занятый слот не наблюдаем снаружи");
    assert!(
        slots.try_acquire().is_none(),
        "второй слот выдан при пределе 1 — ограничителя нет"
    );
    drop(guard);
    assert_eq!(
        slots.in_flight(),
        0,
        "слот не освобождён завершением РАБОТЫ"
    );
    assert!(
        slots.try_acquire().is_some(),
        "после завершения работы слот обязан выдаваться снова"
    );
}
