//! SACRED (architect-only) — `M-74` задача 1: **строитель ПРОД-ФОРМЕННОЙ фикстуры холодной
//! копии** + доказательство того, что её принимает НАСТОЯЩИЙ читатель (`journal::stream`).
//!
//! ## Зачем отдельный строитель, а не `printf` в shell-пробе
//!
//! Первая редакция `scripts/tests/red_restore_drill.sh` клала в «холодную копию» плоские
//! байты `SEGMENT-0001-PAYLOAD` и манифест `{"legacy":[...]}`. Такую копию не примет ни один
//! прод-читатель: сегмент schema ≥ 2 начинается с `SEGMENT_MAGIC = *b"HFTJRN02"`
//! (`crates/contracts/src/lib.rs:43`), а манифест — это `LegacyManifest { declarations: … }`
//! (`crates/contracts/src/lib.rs:68-70`), а не `{"legacy": …}`. Следствие, названное
//! арбитром `A-028` §3 п.2: позитивный контроль `H` прошёл бы ТОЛЬКО у обёртки с
//! mock-читателем, то есть проба толкала исполнителя В ОБХОД прод-пути.
//!
//! Здесь фикстура строится ТЕМ ЖЕ писателем, что пишет прод (`journal::Journal`), и той же
//! компакцией (`journal::compact_closed_segments`). Проверка «фикстура прод-формы» перестаёт
//! быть утверждением автора и становится прогоном: `prod_form_cold_copy_is_read_by_real_reader`
//! открывает построенное `journal::stream`'ом и считает события.
//!
//! ## ФОРМА ПРОДА СНЯТА ЗАМЕРОМ 2026-08-31, А НЕ ВООБРАЖЕНА (`testing.md` §«Форма прода»)
//!
//! Спека `M-74` в редакции `b8d989e` описывала форму копии НЕВЕРНО в трёх местах. Замеры
//! (ssh на прод и на Storage Box, вывод — в вердикте круга):
//!
//! ```text
//! $ cat $J/journal.legacy.json                    → {"declarations": []}   ← ПУСТ
//! $ ls $J/segment-* | head -1                     → segment-00000001.jrnl.zst
//! $ head -c8 segment-00000001.jrnl.zst | xxd      → 28b5 2ffd …  (zstd, не HFTJRN02)
//! $ ssh box 'ls journal/ | grep -v ^segment'      → journal.legacy.json
//!                                                    journal.replay-digest.json
//!                                                   ← journal.meta ОТСУТСТВУЕТ
//! $ ssh box 'ls journal/ | wc -l'                 → 501  (478 .zst + 21 .jrnl + 2 sidecar)
//! $ дублей «и .jrnl, и .jrnl.zst» по одному индексу → 17
//! ```
//!
//! Отсюда три свойства, которые фикстура ОБЯЗАНА нести и несёт:
//!
//! 1. **самый старый сегмент СЖАТ** (`.jrnl.zst`), а не legacy и не сырой. Drill, умеющий
//!    только `.jrnl`, молча пропустил бы 478 сегментов из 499 — и был бы зелёным;
//! 2. **`journal.meta` в копии НЕТ** — он переписывается ежесекундно, и `find -mmin +15`
//!    офсайт-обёртки его не видит. Drill, требующий `journal.meta`, отказывал бы ВСЕГДА;
//! 3. **один индекс присутствует в ДВУХ формах** (`.jrnl` и `.jrnl.zst`) — следствие
//!    осознанного отсутствия `--delete` у rsync плюс компакции на проде. 17 таких пар
//!    замерено. Отбор выборки обязан идти ПО ИНДЕКСУ, иначе «первый / средний / последний»
//!    по ИМЕНИ выберет два файла одного сегмента и один настоящий.
//!
//! Манифест фикстуры — настоящий `LegacyManifest`, сериализованный `serde_json`, с ПУСТЫМ
//! `declarations`: ровно то, что лежит на проде. Требование `A-028` §3 п.2 («манифест —
//! реальный `LegacyManifest`») выполнено по форме типа, а содержимое взято из замера.
//!
//! ## Что этот файл НЕ доказывает
//!
//! Он не проверяет сеть, права Storage Box и саму обёртку drill'а — только то, что
//! построенная им копия читается прод-читателем, а испорченная и пустая — нет. Прод-пруф
//! (первый автопрогон на VPS) остаётся отдельным шагом `M-74`, и здесь не изображается.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use contracts::{DataSource, EventKind, LegacyManifest, LegacySegmentDecl, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig, DEFAULT_COMPACT_LEVEL};

/// Сколько событий кладём в staging-журнал. Подобрано так, чтобы при `max_segment_bytes`
/// ниже получилось ≥4 сегмента: три закрытых (их и копируем) плюс активный (его копия
/// НЕ БЕРЁТ — на проде `find -mmin +15` его не видит).
const EVENTS: u64 = 12_000;
/// Мелкая ротация — единственный способ получить прод-РАСКЛАДКУ (несколько сегментов,
/// часть сжата) в тесте за доли секунды. Прод-масштаб байт здесь не нужен: предмет —
/// РАСКЛАДКА и ФОРМАТ, а не объём (объём меряется на VPS отдельным шагом).
const MAX_SEG: u64 = 64 * 1024;

/// Имя файла состояния drill'а внутри песочницы (совпадает со спекой `M-74`).
pub const STATE_FILE: &str = "journal-restore-drill.json";

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: MAX_SEG,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-74 restore-drill fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn ev(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: 6_400_000_000_000 + i as i64,
            size: 100 + (i as i64 % 7),
            side: if i.is_multiple_of(2) { Side::Buy } else { Side::Sell },
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

/// Сериализовать ПУСТОЙ `LegacyManifest` — байт-в-байт по форме прода
/// (`{"declarations": []}` — замер 2026-08-31, 25 Б).
fn empty_manifest_bytes() -> Vec<u8> {
    serde_json::to_vec_pretty(&LegacyManifest::default()).expect("serialize LegacyManifest")
}

/// Построить STAGING-журнал прод-раскладки и вернуть путь.
/// Результат: несколько закрытых сегментов, часть сжата в `.jrnl.zst`, активный — сырой.
fn build_staging(root: &Path) -> PathBuf {
    let stage = root.join("stage");
    fs::create_dir_all(&stage).expect("mkdir stage");

    let mut j = Journal::open_with(&stage, cfg()).expect("open_with");
    for i in 0..EVENTS {
        j.append(ev(i)).expect("append");
    }
    j.flush().expect("flush");
    drop(j);

    let segs = journal::list_segments(&stage).expect("list_segments");
    assert!(
        segs.len() >= 4,
        "SETUP НЕ СОСТОЯЛСЯ: нужно ≥4 сегмента для прод-раскладки (закрытые + активный), \
         получено {}. Увеличь EVENTS или уменьши MAX_SEG",
        segs.len()
    );

    // Сохраняем СЫРУЮ копию второго сегмента ДО компакции — она станет тем самым дублем
    // «и .jrnl, и .jrnl.zst», которых на боевой коробке замерено 17.
    let dup_idx = segs[1].index;
    let dup_raw = root.join(format!("dup-segment-{dup_idx:08}.jrnl"));
    fs::copy(&segs[1].path, &dup_raw).expect("сохранить сырой дубль до компакции");

    // `keep_raw = 1`: последний ЗАКРЫТЫЙ остаётся сырым, остальные закрытые сжимаются.
    // Это и есть прод-пропорция (замер: 478 `.zst` против 21 `.jrnl`).
    let reports =
        journal::compact_closed_segments(&stage, 1, DEFAULT_COMPACT_LEVEL).expect("compact");
    assert!(
        !reports.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: компакция не сжала ни одного сегмента — фикстура не прод-формы \
         (на проде самый СТАРЫЙ сегмент сжат)"
    );

    // Возвращаем сырой дубль на место рядом со сжатым — воспроизводим измеренную пару.
    let dup_dst = stage.join(format!("segment-{dup_idx:08}.jrnl"));
    fs::copy(&dup_raw, &dup_dst).expect("вернуть сырой дубль");
    assert!(
        stage
            .join(format!("segment-{dup_idx:08}.jrnl.zst"))
            .exists(),
        "SETUP НЕ СОСТОЯЛСЯ: сжатой формы дубля нет — пара raw+zst не воспроизведена"
    );

    stage
}

/// Собрать «холодную копию» ИЗ staging'а ровно так, как её собирает офсайт-обёртка:
/// все файлы, КРОМЕ активного сегмента и `journal.meta`/`recorder.heartbeat`.
///
/// Возвращает число скопированных СЕГМЕНТНЫХ файлов.
fn build_cold(stage: &Path, cold: &Path) -> usize {
    fs::create_dir_all(cold).expect("mkdir cold");

    let segs = journal::list_segments(stage).expect("list_segments");
    let active = segs.iter().map(|s| s.index).max().expect("есть сегменты");

    let mut copied = 0usize;
    for entry in fs::read_dir(stage).expect("read_dir stage") {
        let p = entry.expect("dirent").path();
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .expect("имя файла")
            .to_string();

        // `journal.meta` в копию НЕ ПОПАДАЕТ — замер Storage Box 2026-08-31: его там нет,
        // потому что recorder переписывает его ежесекундно и `find -mmin +15` его не видит.
        if name == "journal.meta" || name == "recorder.heartbeat" {
            continue;
        }
        if name.starts_with("segment-") {
            let base = name.trim_end_matches(".zst");
            let idx: u32 = base
                .trim_start_matches("segment-")
                .trim_end_matches(".jrnl")
                .parse()
                .expect("индекс сегмента");
            // Активный сегмент не копируется (mtime-фильтр обёртки).
            if idx == active {
                continue;
            }
            copied += 1;
        }
        fs::copy(&p, cold.join(&name)).expect("копия файла в cold");
    }

    // Sidecar'ы, которые НА КОРОБКЕ ЕСТЬ (замер): манифест и replay-digest.
    fs::write(cold.join(journal::LEGACY_MANIFEST), empty_manifest_bytes()).expect("манифест");
    fs::write(
        cold.join("journal.replay-digest.json"),
        br#"{"through_seq":0,"digest":"sha256:fixture","code_version":"M-74-fixture"}"#,
    )
    .expect("replay-digest");

    assert!(
        copied >= 3,
        "SETUP НЕ СОСТОЯЛСЯ: в холодной копии {copied} сегментных файлов, а правило выборки \
         спеки берёт ТРИ (первый / средний / последний закрытый)"
    );
    copied
}

/// ПОРЧА: испортить байты внутри `.zst`-сегмента копии, оставив имя и размер прежними.
/// Именно так выглядит тихая порча носителя — файл на месте, `ls` ничего не показывает.
fn corrupt_one_compacted(cold: &Path) -> PathBuf {
    let mut victim: Option<PathBuf> = None;
    let mut names: Vec<PathBuf> = fs::read_dir(cold)
        .expect("read_dir cold")
        .map(|e| e.expect("dirent").path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.ends_with(".jrnl.zst"))
                .unwrap_or(false)
        })
        .collect();
    names.sort();
    if let Some(p) = names.first() {
        let mut bytes = fs::read(p).expect("read victim");
        assert!(
            bytes.len() > 64,
            "SETUP НЕ СОСТОЯЛСЯ: сегмент-жертва слишком мал ({} Б) — порча не отличима от пустоты",
            bytes.len()
        );
        // Ломаем СЕРЕДИНУ zstd-потока: заголовок цел, распаковка обрывается — ровно
        // «на вид файл есть, читатель отказывает».
        let mid = bytes.len() / 2;
        for b in bytes.iter_mut().skip(mid).take(32) {
            *b ^= 0xFF;
        }
        fs::write(p, &bytes).expect("write corrupted");
        victim = Some(p.clone());
    }
    victim.expect("SETUP НЕ СОСТОЯЛСЯ: в холодной копии нет ни одного .zst — порчу негде внести")
}

/// Сколько событий отдаёт НАСТОЯЩИЙ читатель на каталоге. `Err` — отдельный исход.
fn read_events(dir: &Path) -> std::io::Result<usize> {
    let mut n = 0usize;
    for e in journal::stream(dir, EpochFilter::All)? {
        e?;
        n += 1;
    }
    Ok(n)
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// ГЛАВНОЕ: три исхода, СНЯТЫЕ ПРОД-ЧИТАТЕЛЕМ. Это и есть предъявление того, что фикстура
// прод-формы: не «я так написал», а «`journal::stream` её принял».
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn prod_form_cold_copy_is_read_by_real_reader() {
    let root = tempfile::tempdir().expect("tempdir");
    let stage = build_staging(root.path());
    let cold = root.path().join("cold");
    build_cold(&stage, &cold);

    // Позитивный контроль фикстуры: сжатая + сырая формы, дубль индекса, пустой манифест,
    // отсутствующий `journal.meta` — и всё это ЧИТАЕТСЯ.
    let n = read_events(&cold).expect("прод-читатель обязан открыть здоровую холодную копию");
    assert!(
        n > 0,
        "холодная копия прочитана, но событий НОЛЬ — drill на такой копии рапортовал бы успех, \
         ничего не прочитав (класс R-157 Б-5)"
    );

    // Раскладка предъявляется числами, а не словами.
    let files: Vec<String> = fs::read_dir(&cold)
        .expect("read_dir cold")
        .map(|e| e.expect("dirent").file_name().to_string_lossy().to_string())
        .collect();
    let zst = files.iter().filter(|f| f.ends_with(".jrnl.zst")).count();
    let raw = files.iter().filter(|f| f.ends_with(".jrnl")).count();
    assert!(
        zst >= 1 && raw >= 1,
        "прод-раскладка требует ОБЕ формы: сжатых {zst}, сырых {raw}"
    );
    assert!(
        !files.iter().any(|f| f == "journal.meta"),
        "`journal.meta` попал в холодную копию — на боевой коробке его НЕТ (замер 2026-08-31), \
         и фикстура, которая его кладёт, разрешит drill'у на него опереться"
    );
    assert!(
        files.iter().any(|f| f == journal::LEGACY_MANIFEST),
        "манифест обязан быть в копии — на коробке он есть"
    );

    // Дубль индекса (17 пар замерено на коробке) НЕ ломает прод-читателя: `dedup_indexed_paths`
    // при коллизии предпочитает сырой (D-COMP-1). Пиннится здесь, потому что от этого зависит
    // правило выборки drill'а: считать надо ИНДЕКСЫ, а не файлы.
    let idx_of = |f: &String| -> Option<u32> {
        f.strip_prefix("segment-")
            .map(|r| r.trim_end_matches(".zst").trim_end_matches(".jrnl"))
            .and_then(|r| r.parse().ok())
    };
    let mut indices: Vec<u32> = files.iter().filter_map(idx_of).collect();
    let seg_files = indices.len();
    indices.sort_unstable();
    indices.dedup();
    assert!(
        seg_files > indices.len(),
        "SETUP НЕ СОСТОЯЛСЯ: в копии {seg_files} сегментных файлов на {} индексов — дубль \
         raw+zst не воспроизведён, а на коробке таких пар 17",
        indices.len()
    );
}

#[test]
fn corrupted_cold_copy_is_rejected_by_real_reader() {
    let root = tempfile::tempdir().expect("tempdir");
    let stage = build_staging(root.path());
    let cold = root.path().join("cold");
    build_cold(&stage, &cold);

    let before = read_events(&cold).expect("предусловие: здоровая копия читается");
    let victim = corrupt_one_compacted(&cold);

    match read_events(&cold) {
        Err(e) => {
            assert!(
                !journal::is_foreign_segment(&e),
                "порча байт обязана давать ошибку ЧТЕНИЯ, а не «чужой сегмент» — иначе drill \
                 назовёт порчу отсутствием контекста: {e}"
            );
        }
        Ok(after) => panic!(
            "порча {} прошла НЕЗАМЕЧЕННОЙ прод-читателем: было {before} событий, стало {after}. \
             Drill на такой копии объявит бэкап живым",
            victim.display()
        ),
    }
}

#[test]
fn empty_restore_yields_zero_events_not_an_error() {
    // Отдельный исход, и его нельзя путать с порчей: копия скачалась «успешно», но НИЧЕГО
    // не привезла (сеть, права, пустой фильтр). Читатель при этом НЕ падает — он честно
    // отдаёт ноль событий. Именно поэтому drill, решающий по `exit=0` читателя, объявит
    // успех на пустоте — класс `R-157` `Б-5`, и он ловится ЗДЕСЬ.
    let root = tempfile::tempdir().expect("tempdir");
    let empty = root.path().join("empty-restore");
    fs::create_dir_all(&empty).expect("mkdir");
    fs::write(empty.join(journal::LEGACY_MANIFEST), empty_manifest_bytes()).expect("манифест");

    let n = read_events(&empty).expect("пустой каталог читается без ошибки — это и есть ловушка");
    assert_eq!(
        n, 0,
        "предусловие сценария: на пустом восстановлении событий ноль"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// FORWARD-GUARD (НЕ прод-форма). На проде `journal.legacy.json` ПУСТ (замер 2026-08-31:
// `{"declarations": []}`), поэтому сценарий «legacy без манифеста» сегодня НЕДОСТИЖИМ.
// Он оставлен намеренно и помечен: как только появится хоть одна декларация, drill,
// восстановивший сегменты БЕЗ манифеста, объявит ЗДОРОВУЮ копию битой. Различать этот
// исход обязан код возврата читателя, и вот его отличительный признак.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn undeclared_legacy_is_a_context_error_not_a_corruption_error() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("legacy");
    fs::create_dir_all(&dir).expect("mkdir");

    // Legacy-сегмент: postcard-фреймы + crc32, БЕЗ магии и заголовка (форма боевого
    // `segment-00000000.jrnl` до CT-RFC-02).
    let path = dir.join("segment-00000000.jrnl");
    {
        let f = fs::File::create(&path).expect("create legacy");
        let mut w = std::io::BufWriter::new(f);
        for seq in 0..200u64 {
            let e = contracts::Event {
                seq,
                ts_mono_ns: seq,
                ts_wall_ms: 1_752_000_000_000 + seq as i64,
                kind: ev(seq),
            };
            let payload = postcard::to_stdvec(&e).expect("ser");
            w.write_all(&(payload.len() as u32).to_le_bytes())
                .expect("len");
            w.write_all(&payload).expect("payload");
            w.write_all(&crc32fast::hash(&payload).to_le_bytes())
                .expect("crc");
        }
        w.flush().expect("flush");
    }

    // (1) БЕЗ декларации — «нет контекста», и это ОТДЕЛЬНЫЙ признак.
    let err = read_events(&dir).expect_err("незадекларированный legacy обязан быть отвергнут");
    assert!(
        journal::is_foreign_segment(&err),
        "признак «чужой/незадекларированный сегмент» обязан отличаться от порчи, иначе drill \
         скажет «копия битая» там, где не хватает манифеста: {err}"
    );

    // (2) С декларацией — та же копия читается. Без этой половины первая половина зелена
    // и у процедуры, которая не работает никогда.
    journal::declare_legacy(
        &dir,
        LegacySegmentDecl {
            file_name: "segment-00000000.jrnl".to_string(),
            fingerprint_sha256: String::new(), // считает сам `declare_legacy`
            size_bytes_at_decl: 0,             // и размер тоже
            source: DataSource::OwnCapture,
            provenance: "M-74 forward-guard fixture".to_string(),
            epoch_id: contracts::LEGACY_EPOCH_ID.to_string(),
        },
    )
    .expect("declare_legacy");

    let n = read_events(&dir).expect("задекларированный legacy обязан читаться");
    assert_eq!(n, 200, "все legacy-события обязаны быть прочитаны");
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// МАТЕРИАЛИЗАЦИЯ ДЛЯ SHELL-ПРОБЫ.
//
// `scripts/tests/red_restore_drill.sh` не умеет писать postcard-фреймы и zstd-потоки, и не
// должен: строитель обязан быть один. Проба зовёт этот тест с `DRILL_FIXTURE_OUT=<каталог>`
// и вариантом в `DRILL_FIXTURE_VARIANT`, а затем проверяет, что каталог наполнился.
//
// Без переменной тест НИЧЕГО не строит и НИЧЕГО не утверждает — это осознанно: он гоняется
// в общем `cargo test --all`, где строить некуда. Страж от вакуума стоит НА СТОРОНЕ ПРОБЫ:
// она обязана убедиться, что файлы появились, и объявить SETUP НЕ СОСТОЯЛСЯ, если нет.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn materialize_for_shell_probe() {
    let Ok(out) = std::env::var("DRILL_FIXTURE_OUT") else {
        eprintln!(
            "materialize_for_shell_probe: DRILL_FIXTURE_OUT не задан — материализация не запрошена"
        );
        return;
    };
    let variant = std::env::var("DRILL_FIXTURE_VARIANT").unwrap_or_else(|_| "healthy".to_string());
    let out = PathBuf::from(out);
    fs::create_dir_all(&out).expect("mkdir out");

    let cold = out.join("cold");
    match variant.as_str() {
        "empty" => {
            // Пустое восстановление: sidecar'ы есть, сегментов нет.
            fs::create_dir_all(&cold).expect("mkdir cold");
            fs::write(cold.join(journal::LEGACY_MANIFEST), empty_manifest_bytes())
                .expect("манифест");
        }
        "healthy" | "corrupt" => {
            let stage = build_staging(&out);
            build_cold(&stage, &cold);
            if variant == "corrupt" {
                corrupt_one_compacted(&cold);
            }
            // staging больше не нужен — он не часть холодной копии.
            fs::remove_dir_all(&stage).expect("убрать staging");
        }
        other => panic!("неизвестный DRILL_FIXTURE_VARIANT={other}"),
    }

    fs::create_dir_all(out.join("restore")).expect("mkdir restore");
    fs::create_dir_all(out.join("state")).expect("mkdir state");
    println!(
        "DRILL_FIXTURE_READY variant={variant} cold={}",
        cold.display()
    );
}
