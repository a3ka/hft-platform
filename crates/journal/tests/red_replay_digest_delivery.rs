//! SACRED (architect-only) — M-52 / **TD-067: `replay_digest` не доставляется в прод**.
//!
//! ## Дефект — класс TD-011 наоборот: «зелёные тесты при ненаблюдаемом проде»
//!
//! M-51 дал `journal::replay_digest` и доказал `DET-I-1` оракулами и подвыборкой прод-формы
//! (`det_12`/`det_13`). Но **в рантайме дайджест не считается никогда**: на VPS нет Rust
//! toolchain, а в образе (`Dockerfile`: `--bin recorder --bin journal-retention --bin
//! gateway-serve --bin gateway-checkpoint`) нет бинаря, умеющего позвать `replay_digest`.
//! Функция существует на `main`, но в проде её позвать НЕЧЕМ — ровно долг TD-020
//! («библиотека без оператора»), только по детерминизму.
//!
//! До сих пор единственной проверкой боевого журнала был sha256 ФАЙЛА (неизменность байт на
//! диске). Это НЕ воспроизводимость РЕПЛЕЯ: файл может быть бит-в-бит тем же, а декодер —
//! отдавать другой поток событий (смена schema_version, дрейф postcard, порядок сшивки
//! сегментов — см. TD-030 в этом же milestone'е). Именно этот разрыв M-51 закрывал в коде;
//! здесь он закрывается в ПОЛЕ.
//!
//! ## Контракт JR-I-12 (см. `milestones/M-52-journal-hardening.md`)
//!
//! **`DET-I-1` обязан быть НАБЛЮДАЕМ на боевом журнале средствами, которые уже доставлены
//! в прод.** Конкретно: в уже доставляемом бинаре (`journal-retention`) существует режим
//! `--mode replay-digest`, который (а) считает дайджест ПОТОКОВО (память не растёт с
//! журналом), (б) печатает `events/first_seq/last_seq/state_hash`, (в) кладёт машинную
//! запись рядом с журналом, (г) при `--expect` возвращает НЕНУЛЕВОЙ exit-код на
//! расхождении, называя обе величины, и (д) **не пишет в журнал и не мешает сбору** —
//! recorder дайджест не считает НИКОГДА.
//!
//! **Честность окна.** Дайджест ОТКРЫТОГО окна (без `--to`) на живом журнале
//! невоспроизводим ПО ПОСТРОЕНИЮ: пока идёт скан, recorder дописывает события. Поэтому
//! запись обязана НАЗЫВАТЬ окно, которое реально покрыто (`first_seq`/`last_seq`), а
//! сравнение двух прогонов имеет смысл только на ЗАКРЫТОМ окне (`--from`+`--to`). Без
//! этого оператор будет гоняться за фантомным расхождением.
//!
//! ## Почему оракул гоняет НАСТОЯЩИЙ бинарь
//!
//! Урок всего M-08 и TD-024: «функция существует в проде» доказывается ЗАПУСКОМ, а не
//! грепом по исходнику. Форма аргументов — equals (`--flag=value`): ровно она в
//! `docker-compose.yml` и в `--help` (TD-024).

mod common;

use std::process::Command;

use common::{cfg_with, dir_digest, ls};
use journal::{EpochFilter, Journal, WriterConfig};

const BIN: &str = env!("CARGO_BIN_EXE_journal-retention");

/// Контрактное имя машинной записи (канарейка `verify_M-52.sh` стоит на нём же).
const RECORD: &str = "journal.replay-digest.json";

/// Контрактный exit-код РАСХОЖДЕНИЯ дайджеста. 1/2/3 уже заняты (аргументы/сбой сверки/
/// disk_pressure — см. док-шапку бинаря), а cron обязан отличать «прогон не удался» от
/// «детерминизм НАРУШЕН».
const EXIT_DIGEST_MISMATCH: i32 = 4;

fn run(args: &[String]) -> (i32, String) {
    let out = Command::new(BIN).args(args).output().expect("запуск journal-retention");
    let code = out.status.code().unwrap_or(-1);
    let mut s = String::from_utf8_lossy(&out.stderr).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    (code, s)
}

fn cfg() -> WriterConfig {
    cfg_with(8 * 1024, "replay-digest delivery fixture")
}

/// Журнал из НЕСКОЛЬКИХ сегментов + эталонный дайджест, посчитанный БИБЛИОТЕКОЙ.
fn dir_with_journal(n: u64) -> (tempfile::TempDir, journal::ReplayDigest) {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..n {
            j.append(common::trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    assert!(
        ls(dir.path()).iter().filter(|s| s.ends_with(".jrnl")).count() >= 3,
        "setup-guard: фикстуре нужно ≥3 сегмента (сшивка — часть того, что мерит дайджест)"
    );
    let d = journal::replay_digest(dir.path(), EpochFilter::All, None, None).expect("digest");
    (dir, d)
}

fn hex32(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RD-1 — ДОСТАВКА: режим существует в УЖЕ ДОСТАВЛЯЕМОМ бинаре и печатает дайджест
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn rd_1_delivered_binary_computes_replay_digest() {
    let (dir, expect) = dir_with_journal(500);
    let d = dir.path().to_str().unwrap().to_string();

    let (code, out) = run(&[format!("--dir={d}"), "--mode=replay-digest".to_string()]);
    assert_eq!(
        code, 0,
        "JR-I-12 НАРУШЕН: доставляемый бинарь не умеет `--mode=replay-digest` (exit={code}). \
         `replay_digest` существует на main с M-51, но в проде её позвать НЕЧЕМ: на VPS нет \
         Rust toolchain, а в образе только recorder/journal-retention/gateway-*. \
         Детерминизм доказан в лаборатории и НЕ наблюдается в поле.\nВывод:\n{out}"
    );

    let hash = hex32(&expect.state_hash);
    assert!(
        out.contains(&format!("events={}", expect.events)),
        "вывод обязан называть число событий (events={}): «{out}»",
        expect.events
    );
    assert!(
        out.contains(&hash),
        "вывод обязан содержать state_hash={hash} — иначе оператору нечего сравнивать \
         между прогонами: «{out}»"
    );
    assert!(
        out.contains(&format!("first_seq={}", expect.first_seq.expect("first")))
            && out.contains(&format!("last_seq={}", expect.last_seq.expect("last"))),
        "вывод обязан называть ОКНО, которое реально покрыто (first_seq/last_seq): на живом \
         журнале открытое окно растёт под сканом, и без явного окна два прогона \
         несравнимы: «{out}»"
    );
}

/// `--help` не имеет права врать про контракт (TD-024: четвёртый виток argv-дефекта начался
/// ровно с расхождения `--help` и реального парсера).
#[test]
fn rd_2_help_advertises_the_mode() {
    let (_, out) = run(&["--help".to_string()]);
    assert!(
        out.contains("replay-digest"),
        "`--help` обязан называть режим replay-digest — оператор под инцидентом читает \
         именно его: «{out}»"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RD-3 — НАБЛЮДАЕМОСТЬ РАСХОЖДЕНИЯ: --expect, exit-код, обе величины в выводе
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn rd_3_expect_flag_reports_divergence_loudly() {
    let (dir, expect) = dir_with_journal(500);
    let d = dir.path().to_str().unwrap().to_string();
    let good = hex32(&expect.state_hash);
    let bad = "0".repeat(64);

    // (1) совпадение — тихо и exit 0.
    let (code, out) = run(&[
        format!("--dir={d}"),
        "--mode=replay-digest".to_string(),
        format!("--expect={good}"),
    ]);
    assert_eq!(
        code, 0,
        "совпавший дайджест обязан давать exit 0 — иначе cron не отличит норму от тревоги: \
         «{out}»"
    );

    // (2) расхождение — ГРОМКО, отдельным кодом, с обеими величинами.
    let (code, out) = run(&[
        format!("--dir={d}"),
        "--mode=replay-digest".to_string(),
        format!("--expect={bad}"),
    ]);
    assert_eq!(
        code, EXIT_DIGEST_MISMATCH,
        "JR-I-12 НАРУШЕН: расхождение дайджеста обязано давать выделенный exit-код \
         {EXIT_DIGEST_MISMATCH} (1/2/3 уже заняты аргументами/сверкой/disk_pressure). \
         Оператор узнаёт о нарушении DET-I-1 через код возврата, а не вчитываясь в лог: \
         «{out}»"
    );
    assert!(
        out.contains(&good) && out.contains(&bad),
        "расхождение обязано печатать ОБЕ величины (ожидалось {bad}, получено {good}) — \
         иначе непонятно, что с чем разошлось: «{out}»"
    );
}

/// Закрытое окно `--from/--to` — единственная форма, воспроизводимая на ЖИВОМ журнале.
#[test]
fn rd_4_closed_window_matches_library_and_is_reproducible() {
    let (dir, _) = dir_with_journal(500);
    let d = dir.path().to_str().unwrap().to_string();
    let lib = journal::replay_digest(dir.path(), EpochFilter::All, Some(100), Some(399))
        .expect("digest окна");
    let hash = hex32(&lib.state_hash);

    let args = vec![
        format!("--dir={d}"),
        "--mode=replay-digest".to_string(),
        "--from=100".to_string(),
        "--to=399".to_string(),
    ];
    let (code, out) = run(&args);
    assert_eq!(code, 0, "закрытое окно обязано считаться: «{out}»");
    assert!(
        out.contains(&hash) && out.contains(&format!("events={}", lib.events)),
        "дайджест окна из бинаря обязан СОВПАСТЬ с библиотечным (events={}, {hash}) — иначе \
         прод меряет не то, что доказано оракулами M-51: «{out}»",
        lib.events
    );
    assert_eq!(lib.events, 300, "setup-guard: окно [100,399] — ровно 300 событий");

    // Два прогона подряд на закрытом окне — идентичны (это и есть первый прод-прогон
    // из TD-067: «два запуска подряд с предъявлением совпавшего state_hash»).
    let (code2, out2) = run(&args);
    assert_eq!(code2, 0, "повторный прогон: «{out2}»");
    assert!(
        out2.contains(&hash),
        "два прогона на ОДНОМ закрытом окне обязаны дать один state_hash: «{out2}»"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RD-5 — МАШИННАЯ ЗАПИСЬ: рядом с журналом, атомарно, с ЯВНЫМ окном
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn rd_5_machine_record_is_written_atomically_with_its_window() {
    let (dir, expect) = dir_with_journal(500);
    let d = dir.path().to_str().unwrap().to_string();
    let (code, out) = run(&[format!("--dir={d}"), "--mode=replay-digest".to_string()]);
    assert_eq!(code, 0, "«{out}»");

    let path = dir.path().join(RECORD);
    let body = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "JR-I-12 НАРУШЕН: машинная запись {RECORD} не создана ({e}). Без неё cron/оператор \
             не могут сравнить прогоны иначе как парсингом человеческого лога."
        )
    });
    let v: serde_json::Value =
        serde_json::from_str(&body).unwrap_or_else(|e| panic!("{RECORD} — не JSON: {e}\n{body}"));
    for field in ["events", "first_seq", "last_seq", "state_hash"] {
        assert!(
            !v[field].is_null(),
            "{RECORD} обязан нести поле `{field}` (контракт ReplayDigest + окно): {body}"
        );
    }
    assert_eq!(
        v["state_hash"].as_str().unwrap_or_default(),
        hex32(&expect.state_hash),
        "запись обязана нести тот же state_hash, что напечатан и что даёт библиотека"
    );

    // Атомарность: временных файлов после прогона не остаётся.
    assert!(
        !ls(dir.path()).iter().any(|n| n.ends_with(".tmp")),
        "после прогона в каталоге журнала не должно оставаться .tmp-хвостов: {:?}",
        ls(dir.path())
    );

    // Повторный прогон перезаписывает запись, не плодя файлов.
    let before = ls(dir.path()).len();
    let (code, out) = run(&[format!("--dir={d}"), "--mode=replay-digest".to_string()]);
    assert_eq!(code, 0, "«{out}»");
    assert_eq!(
        ls(dir.path()).len(),
        before,
        "повторный прогон не имеет права плодить файлы в каталоге журнала"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RD-6 — НЕ МЕШАЕТ СБОРУ: read-only по данным, журнал после прогона не изменён
// ═════════════════════════════════════════════════════════════════════════════════════

/// Recorder держит поток на ~3 % CPU; самопроверка не имеет права ни ронять это, ни
/// трогать данные. Здесь пиннится САМОЕ ЖЁСТКОЕ из проверяемого снаружи: ни один
/// сегмент и ни один служебный файл журнала (`journal.meta`, `journal.legacy.json`) не
/// меняется прогоном дайджеста, и следующий старт recorder'а продолжает `seq` как ни в
/// чём не бывало.
#[test]
fn rd_6_digest_run_is_read_only_on_journal_data() {
    let (dir, _) = dir_with_journal(500);
    let d = dir.path().to_str().unwrap().to_string();

    // Пробный старт ДО замера: он мог открыть новый сегмент/обновить мету, и это не имеет
    // отношения к прогону дайджеста. Отпечаток снимается уже после него.
    let next_before = {
        let j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        j.next_seq()
    };
    let before: Vec<(String, String)> = dir_digest(dir.path());

    let (code, out) = run(&[format!("--dir={d}"), "--mode=replay-digest".to_string()]);
    assert_eq!(code, 0, "«{out}»");

    let after: Vec<(String, String)> = dir_digest(dir.path())
        .into_iter()
        .filter(|(n, _)| n != RECORD)
        .collect();
    assert_eq!(
        after, before,
        "JR-I-12 НАРУШЕН: прогон дайджеста ИЗМЕНИЛ файлы журнала. Самопроверка обязана быть \
         read-only по данным: она запускается на ЖИВОМ каталоге, в который в этот же момент \
         пишет recorder."
    );

    let next_after = {
        let j = Journal::open_with(dir.path(), cfg()).expect("open_with после дайджеста");
        j.next_seq()
    };
    assert_eq!(
        next_after, next_before,
        "после прогона дайджеста recorder обязан продолжить seq с той же позиции"
    );
}
