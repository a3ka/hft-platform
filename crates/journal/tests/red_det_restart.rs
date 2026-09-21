//! RED M-51 — **DET-I-1 через границу ПРОЦЕССА** (sacred, architect-only).
//!
//! ## Зачем отдельный оракул, если `det_1` уже гоняет реплей дважды
//!
//! Внутрипроцессное повторение НЕ ловит целый класс недетерминизма: `std::collections::
//! HashMap` берёт `RandomState` из потоко-локального сида, инициализируемого случайно ОДИН
//! РАЗ НА ПРОЦЕСС. Реализация, чей порядок обхода зависит от хэш-сида, может быть идеально
//! стабильна внутри одного прогона и разъезжаться между запусками — а пользователь продукта
//! видит именно РАЗНЫЕ ЗАПУСКИ («открыл график вчера и сегодня»). То же относится к порядку
//! `fs::read_dir` (порядок ФС не гарантирован и не обязан совпадать между процессами) и к
//! любому обходу, зависящему от адресов в памяти.
//!
//! `docs/DESIGN.md` §1 требует `failover == replay` — «резерв догоняет журнал». Резерв — это
//! ДРУГОЙ ПРОЦЕСС по определению. Инвариант, проверенный только внутри одного процесса, этого
//! равенства не доказывает.
//!
//! ## Как устроено
//!
//! Родительский тест перезапускает СВОЙ ЖЕ тест-бинарь (`current_exe`) в режиме ребёнка
//! (`#[test] det_child_emit_digest`, включается переменной окружения `M51_CHILD_DIR`) и
//! сравнивает напечатанный ребёнком дайджест. Два независимых процесса над одним каталогом
//! обязаны дать один `state_hash`.
//!
//! ## Анти-плацебо
//!
//! Константная заглушка дала бы совпадение родителя с ребёнком автоматически. Поэтому
//! `det_10` требует, чтобы ребёнок РАЗЛИЧАЛ полный журнал и его префикс: совпадение
//! проверяется вместе с различением, иначе оракул доказывает только «функция вызвалась».
//!
//! Различение берётся по ОКНАМ ОДНОГО журнала, а не по двум разным журналам: `append`
//! штампует `ts_wall_ms`/`ts_mono_ns` из `SystemTime::now()` (`journal/src/lib.rs:205`),
//! поэтому два независимо записанных журнала различаются ВСЕГДА — «различение» на них
//! зелено даже у реализации, которая хэширует что попало. (Проверено прогоном: первая
//! редакция этого оракула была зелена по неверной причине.)

mod common;

use common::{cfg_with, snap, trade};

use journal::{EpochFilter, Journal};

const CHILD_ENV: &str = "M51_CHILD_DIR";
const MARKER: &str = "M51_DIGEST=";

fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn build(dir: &std::path::Path, n: u64, salt: u64) {
    let mut j = Journal::open_with(dir, cfg_with(16 * 1024, "det-restart")).expect("open");
    for i in 0..n {
        let kind = if i % 3 == 0 {
            snap(i + salt)
        } else {
            trade(i + salt)
        };
        j.append(kind).expect("append");
    }
    j.flush().expect("flush");
}

const CHILD_TO_SEQ: &str = "M51_CHILD_TO_SEQ";

fn digest_in_child(dir: &std::path::Path) -> String {
    digest_in_child_upto(dir, None)
}

/// Посчитать дайджест В ОТДЕЛЬНОМ ПРОЦЕССЕ (перезапуск того же тест-бинаря).
fn digest_in_child_upto(dir: &std::path::Path, to_seq: Option<u64>) -> String {
    let exe = std::env::current_exe().expect("current_exe");
    let mut cmd = std::process::Command::new(exe);
    cmd.args(["--exact", "--nocapture", "det_child_emit_digest"])
        .env(CHILD_ENV, dir);
    if let Some(t) = to_seq {
        cmd.env(CHILD_TO_SEQ, t.to_string());
    }
    let out = cmd.output().expect("запуск дочернего процесса");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix(MARKER))
        .unwrap_or_else(|| {
            panic!(
                "дочерний процесс не напечатал {MARKER}; status={:?}\n--- stdout ---\n{stdout}\n\
                 --- stderr ---\n{}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            )
        });
    assert!(
        out.status.success(),
        "дочерний процесс завершился с {:?}",
        out.status
    );
    line.to_string()
}

/// Режим ребёнка. Без `M51_CHILD_DIR` — no-op, чтобы обычный `cargo test` не падал и не
/// выполнял лишней работы (тест обязан существовать как `#[test]`, чтобы харнесс его нашёл).
#[test]
fn det_child_emit_digest() {
    let Ok(dir) = std::env::var(CHILD_ENV) else {
        return;
    };
    let to_seq = std::env::var(CHILD_TO_SEQ)
        .ok()
        .map(|s| s.parse::<u64>().expect("M51_CHILD_TO_SEQ — число"));
    let d = journal::replay_digest(&dir, EpochFilter::OwnCaptureOnly, None, to_seq)
        .expect("replay_digest в дочернем процессе");
    println!("{MARKER}{}", hex(&d.state_hash));
    println!("M51_EVENTS={}", d.events);
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_9 — реплей после перезапуска процесса даёт то же самое.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_9_replay_across_process_restart_is_identical() {
    let dir = tempfile::tempdir().expect("tempdir");
    build(dir.path(), 150, 0);

    let parent = hex(
        &journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
            .expect("parent")
            .state_hash,
    );
    let child_1 = digest_in_child(dir.path());
    let child_2 = digest_in_child(dir.path());

    assert_eq!(
        parent, child_1,
        "DET-I-1: реплей в ДРУГОМ процессе дал другой state_hash — `failover == replay` \
         (DESIGN §1) не выполняется: резерв, догнавший журнал, увидит другую реальность"
    );
    assert_eq!(
        child_1, child_2,
        "DET-I-1: два независимых процесса над одним журналом разошлись — недетерминизм, \
         зависящий от per-process состояния (хэш-сид HashMap, порядок read_dir, адреса)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_10 — АНТИ-ПЛАЦЕБО к det_9: ребёнок обязан РАЗЛИЧАТЬ журналы.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_10_child_digest_discriminates_between_windows() {
    // ⚠ НЕ сравниваем два независимо записанных журнала: `append` штампует wall-clock
    // (`journal/src/lib.rs:205`), поэтому они различаются ВСЕГДА и такое «различение» зелено
    // даже у реализации, которая просто хэширует что попало. Различение берём на ОДНОМ
    // журнале — по окнам, где источник различия контролируем.
    let dir = tempfile::tempdir().expect("tempdir");
    build(dir.path(), 150, 0);
    let d =
        journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None).expect("full");
    let last = d.last_seq.expect("непустой журнал");

    let full = digest_in_child(dir.path());
    let prefix = digest_in_child_upto(dir.path(), Some(last - 1));
    assert_ne!(
        full, prefix,
        "АНТИ-ПЛАЦЕБО: дочерний процесс напечатал ОДИН дайджест для полного журнала и для его \
         префикса без последнего события — det_9 проверяет не детерминизм, а факт вызова \
         функции"
    );
    // ...и каждое окно через границу процесса стабильно.
    assert_eq!(
        prefix,
        digest_in_child_upto(dir.path(), Some(last - 1)),
        "DET-I-1: окно, посчитанное в двух разных процессах, разошлось"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_11 — append-only: дозапись НЕ переписывает историю (реплей прошлого окна неизменен).
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_11_append_does_not_change_digest_of_past_window() {
    // Продуктовое следствие: цифра, показанная пользователю вчера, обязана воспроизвестись
    // сегодня — при том что журнал с тех пор вырос. Ловит реализацию, где дайджест
    // подмешивает «хвостовое» состояние (next_seq, размер файла, метаданные активного
    // сегмента) вместо чистой свёртки событий окна.
    let dir = tempfile::tempdir().expect("tempdir");
    build(dir.path(), 100, 0);

    let last_before = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("before")
        .last_seq
        .expect("непустой журнал");
    let window_before = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        None,
        Some(last_before),
    )
    .expect("window before");

    // Перезапуск писателя (новый `open_with` — как после рестарта recorder'а) + дозапись.
    build(dir.path(), 60, 500);

    let window_after = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        None,
        Some(last_before),
    )
    .expect("window after");
    assert_eq!(
        window_before.events, window_after.events,
        "число событий в ЗАКРЫТОМ окне изменилось после дозаписи"
    );
    assert_eq!(
        window_before.state_hash, window_after.state_hash,
        "DET-I-1: дайджест ПРОШЛОГО окна изменился после дозаписи — журнал перестал быть \
         append-only с точки зрения реплея; вчерашняя цифра пользователя не воспроизводится"
    );

    // И то же самое — через границу процесса (полный дайджест уже другой: журнал вырос).
    let full_now =
        hex(
            &journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
                .expect("full now")
                .state_hash,
        );
    assert_eq!(
        full_now,
        digest_in_child(dir.path()),
        "DET-I-1: после дозаписи и рестарта писателя дочерний процесс увидел другой журнал"
    );
}
