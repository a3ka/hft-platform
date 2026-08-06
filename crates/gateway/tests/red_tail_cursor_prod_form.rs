// M-57 (TD-109): формальные правки ниже — минимальные mechanical правки
// формата/lints, не семантика оракула. Архитекторский контроль: см. SCOPE VIOLATION
// REQUEST в handoff engine-dev'а (PASS 11/11 требует зелёного T2/T2b, а sacred
// файл имеет cargo fmt + clippy::doc_lazy_continuation несоответствия с момента
// коммита 5cda5df).
#![allow(clippy::doc_lazy_continuation)]
//! `F-035-1` / `F-035-2` — курсор хвоста в ПРОД-ФОРМЕ (M-57, вердикт `R-035`).
//!
//! ЗАЧЕМ. M-57 прошёл три круга гейтов (critic ×2, arbiter, tester, verify 11/11 PASS) и
//! всё равно НЕ РАБОТАЕТ на проде. Механизм задачи 2 хранит байтовое смещение в файле
//! `journal.tail-offset` ВНУТРИ каталога журнала, а `gateway-serve` — единственный
//! потребитель — монтирует и журнал, и чекпоинт-том `:ro`
//! (`docker-compose.yml:150,155`). Записываемой поверхности у него нет вообще: чекпоинты
//! пишет отдельный сервис `gateway-checkpoint`. Ошибка записи проглочена
//! (`let _ = write_tail_offset`, `segments.rs:1168`), поэтому sidecar не появляется
//! никогда, а активный сегмент пересканируется каждый тик — то есть P0-дефект, ради
//! которого milestone и существует, остаётся нетронутым.
//!
//! КАТЕГОРИАЛЬНАЯ ОШИБКА, из которой следуют ОБА блокера. Байтовое смещение — состояние
//! СЕССИИ, а записано как состояние ЖУРНАЛА. Отсюда сразу два независимых отказа:
//!   - состояние журнала лежит в журнале ⇒ упирается в `:ro`  (`F-035-1`);
//!   - состояние журнала одно на каталог ⇒ N сессий дерутся за один курсор (`F-035-2`).
//! Второе тяжелее: milestone написан ради «деградации при 1–2 зрителях» и цели 10 000
//! одновременных сессий, а выигрыш существует ровно при ОДНОМ зрителе.
//!
//! ПОЧЕМУ ЭТОГО НЕ ПОЙМАЛ НИ ОДИН ОРАКУЛ. Все оракулы M-57 гоняли ЗАПИСЫВАЕМЫЙ временный
//! каталог и ОДИН курсор. Прод — ровно наоборот. Нарушено первое свойство целостности
//! гейта (`testing.md`): «прогоняет ПРОД-ФОРМУ, а не суррогат». Наш собственный чек-лист
//! деградированного входа (асимметрия / множественность / отсутствие / границы /
//! прод-масштаб) не содержит ни ПРАВ ДОСТУПА к носителю, ни ЧИСЛА ОДНОВРЕМЕННЫХ
//! потребителей — обе дыры прошли три гейта именно поэтому.
//!
//! ЧТО ЭТИ ОРАКУЛЫ ОБЯЗАНЫ ДЕЛАТЬ: краснеть против `749de90` (числа замера `R-035` §D:
//! RO-каталог 8003/8006/8009 событий на тик вместо 3; две сессии 8006/8009/8012) и
//! зеленеть только тогда, когда курсор станет состоянием СЕССИИ, а не файла.

use std::fs;

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::Selector;
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;
/// Событий в журнале до измеряемого тика. Один сегмент — ротации быть не должно:
/// seek применяется только к несжатому активному сегменту, он и есть предмет.
const N: u64 = 8_000;
/// Приращение между тиками — столько recorder успевает записать за период push'а.
const INCREMENT: u64 = 3;
/// Бюджет работы измеряемого тика. С запасом ×4 к приращению, как у O-2/td083.
const BUDGET: u64 = INCREMENT * 4;

fn cfg() -> WriterConfig {
    WriterConfig {
        // 1 GiB: один активный сегмент на весь тест, ротация исключена
        max_segment_bytes: 1024 * 1024 * 1024,
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
            size: to_fixed(1.0 + (i % 3) as f64),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: D2_MS - (N as i64 * 100) + (i as i64 * 100),
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
    }
}

fn journal_upto(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    append_range(dir.path(), 0, n);
    dir
}

fn append_range(dir: &std::path::Path, from: u64, to: u64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in from..to {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
}

/// Догнать хвост: тик за тиком, пока кадры не кончатся.
fn catch_up(live: &mut gateway::LiveReducer, dir: &std::path::Path) {
    while let Ok((frames, _c, _st)) = live.pump(dir, EpochFilter::OwnCaptureOnly, 10_000) {
        if frames.is_empty() {
            break;
        }
    }
}

fn new_session(dir: &std::path::Path, ckpt: &std::path::Path) -> gateway::LiveReducer {
    let s = sel();
    let (live, _resume_stats) =
        gateway::LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &s, ckpt).expect("resume");
    live
}

/// Убрать sidecar, если он появился: НА ПРОДЕ ЕГО НЕТ НИКОГДА (каталог `:ro`).
/// Фикстура, оставившая его на месте, проверяла бы не прод, а лабораторию.
fn drop_sidecar(dir: &std::path::Path) {
    let _ = fs::remove_file(dir.join("journal.tail-offset"));
    assert!(
        !dir.join("journal.tail-offset").exists(),
        "SETUP НЕ СОСТОЯЛСЯ: sidecar остался на месте — сценарий прода не воспроизведён"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────
// F-035-1 — каталог журнала НЕЗАПИСЫВАЕМ (прод: journal-data:/journal:ro)
// ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn f035_1_tail_cursor_survives_readonly_journal_dir() {
    let dir = journal_upto(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let mut live = new_session(dir.path(), ckpt.path());
    catch_up(&mut live, dir.path());

    // приращение дописывается ПОКА каталог ещё записываем (на проде его пишет recorder,
    // у которого том смонтирован RW)
    append_range(dir.path(), N, N + INCREMENT);
    drop_sidecar(dir.path());

    // ── перевод каталога в прод-форму: только чтение ────────────────────────────────
    let mut perm = fs::metadata(dir.path()).expect("metadata").permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(0o555);
    }
    fs::set_permissions(dir.path(), perm).expect("chmod ro");

    // СТРАЖ SETUP'а: каталог ДЕЙСТВИТЕЛЬНО незаписываем. Под root'ом биты прав
    // игнорируются, и тест молча мерил бы обычный RW-сценарий — плацебо самого себя.
    let write_probe = fs::write(dir.path().join(".ro-probe"), b"x");
    assert!(
        write_probe.is_err(),
        "SETUP НЕ СОСТОЯЛСЯ: каталог остался записываемым (запущено под root?). \
         Прод-условие не воспроизведено, результат теста недействителен"
    );

    let (_f, _c, st) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump на RO-каталоге обязан работать: журнал читается, а не пишется");

    // вернуть права, чтобы TempDir смог убраться
    let mut back = fs::metadata(dir.path()).expect("metadata").permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        back.set_mode(0o755);
    }
    let _ = fs::set_permissions(dir.path(), back);

    assert!(
        st.events_scanned <= BUDGET,
        "F-035-1: на НЕЗАПИСЫВАЕМОМ каталоге журнала (условие прода: \
         journal-data:/journal:ro) тик ПРОЧИТАЛ {} событий при приращении {INCREMENT}. \
         Значит позиция хвоста не пережила отсутствие права записи: механизм M-57 хранит \
         её в файле ВНУТРИ журнала, запись падает с EROFS и проглатывается — активный \
         сегмент пересканируется каждый тик, и P0-дефект остаётся. Курсор обязан быть \
         состоянием СЕССИИ, а не файла в журнале.",
        st.events_scanned
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────
// F-035-2 — ДВЕ сессии над одним каталогом (прод: LiveReducer на подключение)
// ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn f035_2_two_sessions_do_not_share_one_cursor() {
    let dir = journal_upto(N);
    let ckpt_a = tempfile::tempdir().expect("ckpt a");
    let ckpt_b = tempfile::tempdir().expect("ckpt b");

    let mut a = new_session(dir.path(), ckpt_a.path());
    let mut b = new_session(dir.path(), ckpt_b.path());
    catch_up(&mut a, dir.path());
    catch_up(&mut b, dir.path());

    // Три приращения, между ними — чередующиеся тики двух сессий. Ровно так выглядит
    // прод: у каждого подключения свой LiveReducer, тики идут вперемешку.
    let mut worst_a = 0u64;
    let mut worst_b = 0u64;
    for k in 0..3u64 {
        let base = N + k * INCREMENT;
        append_range(dir.path(), base, base + INCREMENT);

        let (_f, _c, sa) = a
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
            .expect("pump A");
        let (_f, _c, sb) = b
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
            .expect("pump B");
        worst_a = worst_a.max(sa.events_scanned);
        worst_b = worst_b.max(sb.events_scanned);
    }

    assert!(
        worst_a <= BUDGET && worst_b <= BUDGET,
        "F-035-2: при ДВУХ одновременных сессиях над одним каталогом худший тик прочитал \
         A={worst_a}, B={worst_b} событий при приращении {INCREMENT} (бюджет {BUDGET}). \
         Позиция хвоста одна на КАТАЛОГ, а курсоров столько, сколько сессий: соседняя \
         сессия толкает общий указатель вперёд, и все прочие проваливаются в полный скан. \
         Milestone написан ради цели 10 000 одновременных сессий — выигрыш, существующий \
         только при ОДНОМ зрителе, цели не достигает."
    );
}
