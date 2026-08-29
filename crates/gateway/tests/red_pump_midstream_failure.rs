//! RED `TD-179` / `VB-I-2` (sacred, architect-only) — **ЗАКЛАДКА ДОСТАВКИ НЕ УХОДИТ ДАЛЬШЕ
//! ДОСТАВЛЕННОГО.**
//!
//! Милестоун `milestones/M-72-subscription-terminality.md`, задача 4. Источник —
//! `TECH-DEBT.md` `TD-179` (заведено reviewer'ом по `R-146` `N-3`).
//!
//! # Что сломано и почему это не видно ни одному существующему оракулу
//!
//! `M-71` сделал курсор ПОБАТЧЕВЫМ: `self.cursor = cursor` стоит ВНУТРИ цикла, на закрытии
//! каждого батча (`crates/gateway/src/lib.rs:3579`; в базе `3b49620` он стоял ОДИН раз ПОСЛЕ
//! цикла). Кадры при этом отдаются вызывателю ОДНИМ `Ok` в самом конце. Любой `Err` из
//! середины стрима (`:3551`, `let event = event?`) уносит уже собранные кадры — а события,
//! которые в них лежали, курсором уже помечены доставленными. Следующий `pump` начнёт ПОСЛЕ
//! них: серия потребителя короче реплея того же окна, и никто об этом не извещён.
//!
//! **На cap-пути дыры нет** — там отказ ТЕРМИНАЛЕН (`refuse_by_cap`, `is_cap_terminal`),
//! подписка завершается, клиент пересобирается снапшотом. На ЖУРНАЛЬНОМ отказе `live_keeps`
//! истинно, и подписка живёт дальше с молчаливым провалом. Это класс `R-140` `B-1`
//! (`VB-I-2`, `GW-I-4`) на другом пути.
//!
//! # Мера снята на границе ПОТРЕБИТЕЛЯ, а не внутри реализации
//!
//! `docs/workflow/oracle-blindness-class-2026-08-28.md`, правило **Р-1**. Потребитель `pump`
//! видит ровно две вещи: КАДРЫ, которые ему отдали, и КУРСОР, который ему объявили. Оракул
//! сравнивает ровно их и ничего больше — ни `full_applied_seq`, ни `ReadStats`, ни счётчики
//! батчей. Именно поэтому он переживёт появление второго носителя свойства: он о носителях
//! не знает.
//!
//! Правило **Р-2** того же разбора: **режим отказа — отдельный режим.** `M-1` судит успешный
//! вызов (позитивный контроль), `M-2` — состояние ПОСЛЕ отказа. Один тест на оба был бы
//! оракулом, зелёным по неверной причине.
//!
//! # Анти-плацебо в обе стороны
//!
//! `M-1` идёт ПЕРВЫМ намеренно — той же причиной, по какой `P-C1` идёт первым в
//! `red_egress_cap_paths.rs`: реализация «никогда не двигать курсор» удовлетворила бы `M-2`
//! и убила бы продукт. Страж, ломающий честную работу, выключат, и защиты не станет.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{LiveReducer, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Размер партии догона. Тот же, что в `red_egress_cap_paths.rs`, и по той же причине:
/// мелкая партия закрывает батчи ЧАСТО, то есть курсор успевает продвинуться до того, как
/// стрим упрётся в испорченный сегмент. С крупной партией предмет недостижим — единственный
/// батч не закроется, и отказ унесёт пустоту, а не собранное.
const PUMP_BATCH: usize = 256;

/// **Сегмент намеренно МАЛЕНЬКИЙ.** Предмет живёт в переходе через границу сегмента: первый
/// читается честно и закрывает батчи, второй отказывает. При дефолтных 64 MiB весь журнал лёг
/// бы в один сегмент, портить было бы нечего, и оракул проверял бы не тот сценарий
/// (`testing.md`, целостность гейта, свойство 3).
fn cfg_small() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 16,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "TD-179 midstream-failure fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
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

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о курсоре: фикстура не воспроизвела \
         сценарий, ради которого оракул написан."
    )
}

/// Журнал из `n` сделок, гарантированно РАЗЛОЖЕННЫЙ по нескольким сегментам.
fn journal_multi_segment(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("SETUP: tempdir");
    let mut j = Journal::open_with(dir.path(), cfg_small()).expect("SETUP: open_with");
    for i in 0..n as i64 {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID + i as f64 * 0.01),
                size: to_fixed(1.0),
                side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                ts_exch_ms: T0 + i,
            },
        ))
        .expect("SETUP: append");
    }
    j.flush().expect("SETUP: flush");
    dir
}

/// Сегменты фикстуры по возрастанию имени. Порядок существен: портить нужно НЕ ПЕРВЫЙ.
fn segments(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .expect("SETUP: read_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let s = p.to_string_lossy().to_string();
            s.ends_with(".jrnl") || s.ends_with(".jrnl.zst")
        })
        .collect();
    v.sort();
    v
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// M-1 — ПОЗИТИВНЫЙ КОНТРОЛЬ, первым: честный догон ДВИГАЕТ курсор и ОТДАЁТ кадры
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Реализация «никогда не двигать курсор» удовлетворила бы `M-2` и уничтожила бы продукт.
/// Контроль стоит первым, чтобы такой «фикс» валился здесь, а не проходил гейт.
#[test]
fn td_179_m1_honest_pump_advances_cursor_and_delivers_frames() {
    gateway::set_effective_max_response_bytes(usize::MAX);
    let dir = journal_multi_segment(1_500);
    let segs = segments(dir.path());
    if segs.len() < 2 {
        setup_failed(&format!(
            "фикстура дала {} сегмент(ов) — переход через границу сегмента недостижим, \
             и M-2 проверял бы не тот сценарий",
            segs.len()
        ));
    }

    let ckpt = tempfile::tempdir().expect("SETUP: ckpt tempdir");
    let (mut r, _) =
        LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .unwrap_or_else(|e| setup_failed(&format!("resume не собрался: {e}")));

    let before = r.snapshot().cursor.upto_seq;
    let (frames, _, _) = r
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH)
        .unwrap_or_else(|e| setup_failed(&format!("честный pump отказал: {e}")));
    let after = r.snapshot().cursor.upto_seq;

    if frames.len() < 2 {
        setup_failed(&format!(
            "честный pump отдал {} кадр(ов) при партии {PUMP_BATCH} — батчи не закрываются, \
             значит в M-2 курсору нечего будет продвинуть и дефект недостижим",
            frames.len()
        ));
    }
    assert!(
        after > before,
        "TD-179 M-1: честный догон отдал {} кадров, но закладку доставки НЕ продвинул \
         (before={before:?}, after={after:?}). Это не защита, а остановка продукта: \
         следующий pump переигрывал бы то же самое вечно",
        frames.len()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// M-2 — ПРЕДМЕТ: отказ середины НЕ смеет оставлять закладку впереди доставленного
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **Инвариант.** Если вызов `pump` не отдал потребителю кадры, закладка доставки обязана
/// остаться там, где была. Иначе события, свёрнутые в выброшенные кадры, объявлены
/// доставленными, и в серии потребителя появляется дыра, о которой ему не сказали
/// (`PL-I-7`: деградация никогда не выдаётся за норму).
///
/// **Развязка НЕ предписывается этим оракулом** — их две законные, и выбор за задачей 5
/// спеки: (а) отдать собранные кадры вместе с отказом; (б) откатить закладку к последнему
/// доставленному кадру и ТЕРМИНИРОВАТЬ подписку с извещением. Оракул судит РЕЗУЛЬТАТ у
/// потребителя, а не способ, которым его добились.
#[test]
fn td_179_m2_failed_pump_must_not_leave_cursor_ahead_of_delivered() {
    gateway::set_effective_max_response_bytes(usize::MAX);
    let dir = journal_multi_segment(1_500);
    let segs = segments(dir.path());
    if segs.len() < 2 {
        setup_failed(&format!(
            "фикстура дала {} сегмент(ов) — портить нечего",
            segs.len()
        ));
    }

    let ckpt = tempfile::tempdir().expect("SETUP: ckpt tempdir");
    let (mut r, _) =
        LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .unwrap_or_else(|e| setup_failed(&format!("resume не собрался: {e}")));

    // ПОРЧА СТАВИТСЯ В СЕРЕДИНУ ПЕРВОГО СЕГМЕНТА, А НЕ В ЗАГОЛОВОК ВТОРОГО — и это не
    // деталь, а условие достижимости предмета. Первая редакция портила заголовок второго
    // сегмента (приём `P7`), и оракул был ЗЕЛЁНЫМ ВАКУУМНО: стрим падает на ПЕРЕЧИСЛЕНИИ
    // сегментов («foreign segment (no magic, no declaration)»), до первого события, курсор
    // не двигается вовсе — `before=None after=None`. Тест проверял не тот сценарий и
    // рапортовал успех. Поймано прогоном, а не рассуждением (`testing.md`, анти-плацебо:
    // «оракул зелёный с первого запуска против кода, который ещё не написан правильно»).
    //
    // Один перевёрнутый байт в теле кадра ломает CRC ИМЕННО ЭТОГО кадра: предшествующие
    // декодируются честно и закрывают батчи, а отказ приходит из СЕРЕДИНЫ прохода.
    let mut bytes = std::fs::read(&segs[0])
        .unwrap_or_else(|e| setup_failed(&format!("не прочитан первый сегмент: {e}")));
    let at = bytes.len() * 3 / 5;
    bytes[at] ^= 0xFF;
    std::fs::write(&segs[0], &bytes)
        .unwrap_or_else(|e| setup_failed(&format!("не удалось испортить первый сегмент: {e}")));

    // SETUP-GUARD НЕЗАВИСИМЫМ ПУТЁМ (`testing.md`: эталон берётся из независимого пути, не
    // из проверяемого). Считаем СВОИМ проходом, сколько событий журнал отдаёт до отказа:
    // предмет требует, чтобы отказ пришёл ПОСЛЕ как минимум одной закрытой партии, иначе
    // курсору нечего продвигать и оракул вырождается в тавтологию.
    let mut yielded = 0usize;
    let mut stream_failed = false;
    match journal::stream(dir.path(), EpochFilter::OwnCaptureOnly) {
        Ok(st) => {
            for ev in st {
                match ev {
                    Ok(_) => yielded += 1,
                    Err(_) => {
                        stream_failed = true;
                        break;
                    }
                }
            }
        }
        Err(e) => setup_failed(&format!("независимый проход не открылся вовсе: {e}")),
    }
    if !stream_failed {
        setup_failed(&format!(
            "порча по смещению {at} не дала отказа: независимый проход отдал {yielded}              событий и завершился штатно. Сценарий «отказ середины» не воспроизведён"
        ));
    }
    if yielded < PUMP_BATCH {
        setup_failed(&format!(
            "отказ пришёл после {yielded} событий при партии {PUMP_BATCH} — ни одна партия              не закрылась, курсору нечего продвигать, и зелёный результат ничего не значил бы"
        ));
    }

    let before = r.snapshot().cursor.upto_seq;
    let res = r.pump(dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH);
    let after = r.snapshot().cursor.upto_seq;

    let err = match res {
        Ok((frames, _, _)) => setup_failed(&format!(
            "испорченный второй сегмент не дал отказа — pump вернул Ok с {} кадрами. \
             Предмет не воспроизведён, сравнивать нечего",
            frames.len()
        )),
        Err(e) => e,
    };

    // РАЗЛИЧИТЕЛЬ ПРИЧИНЫ. Предмет — ЖУРНАЛЬНЫЙ отказ, а не отказ по пределу: на cap-пути
    // дыра закрыта терминальностью (`M-71` §4bis.5), и оракул, случайно поймавший cap,
    // судил бы уже защищённое место. Признак — флаг редьюсера, НЕ `io::ErrorKind`
    // (`R-143` B-3: журнал отдаёт `Other` в четырёх местах).
    if r.is_cap_terminal() {
        setup_failed(&format!(
            "отказ оказался ПО ПРЕДЕЛУ (is_cap_terminal=true, refusals={}), а предмет — \
             журнальный отказ. Предел снят в usize::MAX, значит фикстура сломалась: {err}",
            r.cap_refusals()
        ));
    }

    assert_eq!(
        after, before,
        "TD-179 M-2: pump отказал и НЕ отдал потребителю ни одного кадра, но закладка \
         доставки ушла вперёд ({before:?} → {after:?}). События, свёрнутые в выброшенные \
         кадры, объявлены доставленными: следующий pump начнёт ПОСЛЕ них, серия потребителя \
         станет короче реплея того же окна (VB-I-2), и клиенту об этом не скажут. \
         Отказ был: {err}"
    );
}
