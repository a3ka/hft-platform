//! RED M-87 круг `R-200` §B7 (sacred, architect-only) — **четыре условия прошлого ревью,
//! которые гейт не видел, потому что смотрел на ПРИСУТСТВИЕ ИМЕНИ.**
//!
//! ## Почему этот файл вообще нужен
//!
//! `R-200` §B7 нашёл пять неисполненных условий `R-196` — и главное в находке не они сами,
//! а приписка: «гейт зелен при всех пяти». Шаги задач 4, 5, 6, 8 в `verify_M-87.sh` были
//! грепами имени, а присутствие имени и работа механизма — разные утверждения. Один из пяти
//! (потеря декрементов под гонкой) вынесен отдельным бинарём
//! `red_m87_slot_counter_race.rs` — счётчик процессный, и в общем бинаре замер стал бы флаком.
//! Остальные четыре здесь.
//!
//! **Класс уже стоил этому милестоуну круга дважды:** моя же канарейка задачи 20 считала
//! упоминания имени, и одно из них было импортом (`R-200` §B4). Лекарство одно — проверять
//! ВЫЗОВ либо ПОВЕДЕНИЕ.
//!
//! ## Что здесь пиннится, и чем каждое отличается от грепа имени
//!
//! | № | условие `R-196` | форма проверки |
//! |---|---|---|
//! | `c1` | №4 — ветка «нет слепка» обязана быть fail-closed | ПОВЕДЕНИЕ: клиент получает названный отказ, а не снимок |
//! | `c2` | №5 — счётчик меряет ПРОЧИТАННЫЕ байты журнала | ВЫЗОВ: чем кормится инкремент, а не встречается ли его имя |
//! | `c3` | №6 — бюджет вызова ПОДКЛЮЧЁН к пути выдачи | ВЫЗОВ: есть ли вызыватель в `src/**`, а не определение |
//! | `c4` | №8 — ручной разбор `journal.meta` согласован с независимым путём | СХОДИМОСТЬ: ручное чтение против `Journal::next_seq()` |
//!
//! ## Почему `c4` — сходимость, а не «проверка версии»
//!
//! Ревьюер потребовал разбирать `journal.meta` через API `crates/journal`. Замер показал:
//! формат метаданных — ОДИН `u64` little-endian без заголовка и без поля версии
//! (`crates/journal/src/lib.rs:9`), то есть проверять версию нечего, а публичного читателя
//! вне `Journal::open_with` не существует. Требовать новое API в чужом крейте ради этого —
//! расширение зоны без выгоды.
//!
//! Настоящий риск другой: если формат метаданных когда-нибудь получит заголовок, ручной
//! читатель прочтёт первые восемь байт заголовка КАК НОМЕР и не заметит. Против этого
//! работает не проверка версии, а сходимость с независимым путём: `c4` сверяет то, что
//! читает выдача, с тем, что говорит сам журнал. Разойдутся форматы — оракул покраснеет.
//!
//! RUNTIME-RED по построению: `c1`, `c2`, `c3` красны на ревизии находки.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::StreamExt;
use gateway::Selector;
use gateway_serve::server::{bind, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::Value;

const SECRET: &[u8] = b"m87-r196-conditions";
const FUTURE: usize = 4_000_000_000;

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m87-cond".to_string(),
            exp: FUTURE,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt")
}

fn writer_cfg() -> WriterConfig {
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
            price: to_fixed(100.0 + (i % 7) as f64),
            size: to_fixed(1.0 + (i % 3) as f64),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_752_000_000_000 + i as i64 * 100,
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

fn journal_of(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
    for i in 0..n {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("корень репозитория")
        .to_path_buf()
}

/// Строки `crates/gateway-serve/src/**`, без комментариев. Греп по тексту вместе с
/// комментариями — ровно та ошибка, за которую шаг `task8` получил находку `R-200` §N4
/// (из девяти совпадений семь были строками `//`).
fn src_code_lines() -> Vec<String> {
    let mut out = Vec::new();
    let dir = repo_root().join("crates/gateway-serve/src");
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("read_dir src") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            for line in std::fs::read_to_string(&p).expect("read src").lines() {
                let t = line.trim();
                if t.starts_with("//") || t.is_empty() {
                    continue;
                }
                out.push(t.to_string());
            }
        }
    }
    assert!(
        !out.is_empty(),
        "SETUP-СТРАЖ: код транспорта не прочитан — канарейки c2/c3 зеленели бы на пустоте"
    );
    out
}

/// **`c1` — ВЕТКА «НЕТ СЛЕПКА» ОБЯЗАНА БЫТЬ FAIL-CLOSED (условие №4).**
///
/// Сегодня при `checkpoint_dir: None` проверка готовности не зовётся ВОВСЕ, и готовность
/// объявляется значением по умолчанию `Ready` (`lib.rs:1805`). Прод-путь спасён только тем,
/// что `main.rs` всегда идёт через политику, — но сама ветка жива, и её никто не краснит.
/// Предмет милестоуна в том, чтобы недонастроенное развёртывание ОТКАЗЫВАЛО, а не
/// обслуживало: «неизвестный вход → reject» — это `RK-I-3`-класс, fail-closed по построению.
#[tokio::test]
async fn c1_missing_checkpoint_dir_refuses_instead_of_declaring_ready() {
    let dir = journal_of(200);
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: None, // ← недонастроенное развёртывание
    };
    let server = bind(cfg).await.expect("bind");
    let addr = server.local_addr();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    let token = sign();
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/?token={token}"))
        .await
        .expect("connect");
    let m = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next())
        .await
        .expect("таймаут")
        .expect("поток закрыт")
        .expect("ws error");
    let v: Value = serde_json::from_slice(m.into_data().as_ref()).expect("parse");
    // Снимок распознаётся В ЛЮБОЙ ИЗ ДВУХ ФОРМ, и это не перестраховка. Первая редакция
    // ассерта сравнивала только `type == "snapshot"` и была ЛОЖНО-ЗЕЛЁНОЙ: legacy-путь
    // отдаёт `ServeMsg::Snapshot` внешним ключом, без поля `type`, поэтому проверка v1-формы
    // на legacy-пути проходила ВСЕГДА. Поймано собственным прогоном с печатью факта
    // (`type="" keys=["Snapshot"]`) — ровно тот класс, который этот файл и заводится ловить:
    // проверка обязана смотреть на ФАКТ, а не на ожидаемую автором форму.
    let is_snapshot =
        v.get("type").and_then(Value::as_str) == Some("snapshot") || v.get("Snapshot").is_some();
    assert!(
        !is_snapshot,
        "развёртывание БЕЗ каталога слепка получило СНИМОК, то есть готовность объявлена \
         значением по умолчанию. Требование §4: исход НАЗВАН, недонастроенное развёртывание \
         ОТКАЗЫВАЕТ. Сегодня проверка готовности при отсутствии каталога не зовётся вовсе \
         (`lib.rs:1805` → `ServingOutcome::Ready`), и прод-путь спасает лишь то, что \
         `main.rs` всегда идёт через политику — сама ветка fail-open жива и достижима \
         неверным развёртыванием.\n\
         Полученное сообщение: {v}"
    );
}

/// **`c2` — СЧЁТЧИК МЕРИТ ПРОЧИТАННОЕ, А НЕ ОТПРАВЛЕННОЕ (условие №5).**
///
/// Проверяется АРГУМЕНТ вызова, а не присутствие имени: сегодня инкремент кормится
/// `snap_text.len()` — размером ОТПРАВЛЕННОГО снимка. Величина, названная
/// «journal_payload_bytes_read», обязана мерить то, что обещает (`testing.md` §«Оракул обязан
/// мерить ТО, ЧТО ОБЕЩАЕТ», п. 1: назови, что именно считает цифра).
#[test]
fn c2_payload_counter_is_not_fed_by_response_size() {
    let bad: Vec<String> = src_code_lines()
        .into_iter()
        .filter(|l| l.contains("add_journal_payload_bytes"))
        .filter(|l| l.contains("snap_text.len()") || l.contains("text.len()"))
        .collect();
    assert!(
        bad.is_empty(),
        "счётчик ПРОЧИТАННЫХ байт журнала кормится размером ОТПРАВЛЕННОГО ответа: {bad:?}.\n\
         Это главная цифра милестоуна — по ней судят, читает ли живой путь журнал. \
         Измеряя длину ответа, она отвечает на другой вопрос и будет расти даже когда \
         журнал не читался вовсе."
    );
}

/// **`c3` — БЮДЖЕТ ВЫЗОВА ПОДКЛЮЧЁН К ПУТИ ВЫДАЧИ (условие №6).**
///
/// Проверяется наличие ВЫЗЫВАТЕЛЯ в `src/**`, а не определения: сегодня `feed_tail_within`
/// определён в `admission.rs` и зовётся ТОЛЬКО из тестов — механизм существует и не работает,
/// то есть «built-not-wired» внутри самого милестоуна о предохранителе.
#[test]
fn c3_call_budget_has_a_caller_in_production_code() {
    let calls = src_code_lines()
        .iter()
        .filter(|l| l.contains("feed_tail_within("))
        .filter(|l| !l.contains("pub fn feed_tail_within"))
        .count();
    assert!(
        calls > 0,
        "`feed_tail_within` не вызывается НИ ИЗ ОДНОЙ строки `crates/gateway-serve/src/**` — \
         бюджет вызова определён и не подключён. Милестоун существует ради того, чтобы один \
         запрос не кладл сервис; бюджет, которого никто не зовёт, этого не обеспечивает \
         (`gates.md` §4 «Механизм на пути»: мержится только с подключением к пути)"
    );
}

/// **`c4` — РУЧНОЙ РАЗБОР МЕТАДАННЫХ СХОДИТСЯ С НЕЗАВИСИМЫМ ПУТЁМ (условие №8).**
///
/// Выдача читает `journal.meta` вручную (`u64` LE). Формат сегодня именно такой, поэтому
/// «проверять версию» нечего — и требовать новое API в чужом крейте ради этого значит
/// расширять зону без выгоды. Опасность в другом: если формат получит заголовок, ручной
/// читатель примет первые восемь байт заголовка ЗА НОМЕР и не заметит. Ловит это сходимость
/// с независимым путём — `Journal::next_seq()`.
#[test]
fn c4_manual_meta_read_agrees_with_journal_api() {
    let dir = journal_of(300);
    let expected = {
        let j = Journal::open_with(dir.path(), writer_cfg()).expect("reopen");
        j.next_seq()
    };
    let raw = std::fs::read(dir.path().join("journal.meta")).expect("read meta");
    assert!(
        raw.len() >= 8,
        "SETUP-СТРАЖ: метаданные короче восьми байт — сверять нечего"
    );
    let manual = u64::from_le_bytes(raw[..8].try_into().expect("8 байт"));
    assert_eq!(
        manual, expected,
        "ручное чтение `journal.meta` дало {manual}, а сам журнал говорит {expected}. \
         Форматы разошлись — значит ручной читатель в пути выдачи читает НЕ ТО, и узнать об \
         этом иначе, чем этой сверкой, нельзя: поля версии в метаданных нет"
    );
}
