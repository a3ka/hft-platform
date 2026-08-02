//! SACRED (architect-only) — M-52 / **TD-030: нет машинного guard'а монотонности сегментов**.
//!
//! ## Дефект
//!
//! `read_all`/`stream`/`stream_from` сшивают сегменты по ИНДЕКСУ ФАЙЛА
//! (`dedup_indexed_paths` → `BTreeMap<u32, PathBuf>` → blind `extend`) БЕЗ единой проверки,
//! что `first_seq` заголовков строго возрастает вместе с индексом. Следствие: ошибочный
//! re-stitch терминального (карантинного) архива в живой каталог даёт **ТИХИЙ беспорядок
//! seq** (`[0,1,2,3,4,7,5,6]` — probe risk-critic C-018 rev2), а не отказ. Правило «архив не
//! возвращается в live» сегодня — операторская дисциплина в runbook, НЕ машинный барьер.
//!
//! Тот же дефект живёт на пути защиты декларации: `readable_floor` идёт по сегментам С
//! КОНЦА и **останавливается на первом, где нашёлся валидный фрейм** — то есть опирается
//! ровно на ту же монотонность. Немонотонный каталог даёт ЗАНИЖЕННЫЙ пол ⇒ декларация
//! внутри занятого диапазона проходит валидацию ⇒ seq-reuse. Это условие закрытия TD-030,
//! принятое reviewer'ом в `R-002` и подтверждённое в `R-003`: закрывая TD-030, гейт обязан
//! покрыть И `readable_floor`.
//!
//! ## Контракт JR-I-11 (см. `milestones/M-52-journal-hardening.md`)
//!
//! **Ни один путь чтения журнала не имеет права молча сшить каталог, чьи СРАВНИМЫЕ
//! `first_seq` не строго возрастают по возрастанию индекса сегмента.** Обнаружив
//! немонотонность, путь обязан вернуть `Err` с диагностикой, называющей ОБА файла и их
//! `first_seq`. Покрываются `read_all`, `stream`/`stream_from` и `readable_floor`.
//!
//! **Сравнимость — по `schema_version`, НЕ по значению `first_seq`.** Сегмент, чей
//! `schema_version == SCHEMA_VERSION_PRE_HEADER` (legacy, до CT-RFC-02), несёт
//! СИНТЕЗИРОВАННЫЙ `first_seq = 0` — «безопасный дефолт», а не факт (`segments.rs`:
//! «first_seq legacy: неизвестен без чтения сегмента»). Такой сегмент ИСКЛЮЧАЕТСЯ из
//! сравнения — ни как левый, ни как правый операнд; сравниваются соседние СРАВНИМЫЕ
//! сегменты. Ровно тот же carve-out уже действует в `stream_from` для сегментного
//! пропуска («Legacy (`first_seq == 0`) НЕ пропускается НИКОГДА»), и он же — предупреждение
//! TECH-DEBT: наивный guard споткнётся о сентинел и даст прод-регрессию ХУЖЕ закрываемой
//! дисциплинарной дыры (класс TD-011).
//!
//! Отличать legacy по ЗНАЧЕНИЮ `first_seq == 0` НЕЛЬЗЯ: у первого v2-сегмента здорового
//! журнала `first_seq` тоже 0, и такое правило выключало бы guard на самом частом каталоге.

mod common;

use common::{
    append_bytes, cfg_with, first_seqs, frame_of, is_segment_name, ls, max_seq_in,
    swap_segment_files, write_decl, DECL_APPLIED, TAIL_SCAN_CHUNK,
};
use contracts::{DataSource, Event, EventKind, LegacySegmentDecl, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

/// Мусор `0x5A` в хвосте — только чтобы окно хвостового скана было нечитаемым и путь
/// операторской декларации (единственный вызов `readable_floor`) вообще входился.
const CHEAP_TAIL: usize = 4 * 1024 * 1024 + 512 * 1024;

const _: () = assert!(
    CHEAP_TAIL as u64 > TAIL_SCAN_CHUNK,
    "фикстура: мусор обязан перекрывать окно хвостового скана целиком"
);

fn cfg() -> WriterConfig {
    cfg_with(8 * 1024, "stitch-monotonic fixture")
}

fn seg_names(dir: &std::path::Path) -> Vec<String> {
    ls(dir).into_iter().filter(|n| is_segment_name(n)).collect()
}

/// Здоровый многосегментный каталог: `seq` 0..N-1, `first_seq` строго возрастают.
fn healthy_dir(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..n {
            j.append(common::trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let fs = first_seqs(dir.path());
    assert!(
        fs.len() >= 3,
        "setup-guard: фикстуре нужно ≥3 сегмента, получено {}",
        fs.len()
    );
    assert!(
        fs.windows(2).all(|w| w[0] < w[1]),
        "setup-guard: здоровый каталог обязан быть монотонным, получено {fs:?}"
    );
    dir
}

/// Немонотонный каталог: два ПОСЛЕДНИХ сегмента переставлены местами (архив вернули в
/// живой каталог под чужим индексом). Возвращает (каталог, имена переставленных).
fn restitched_dir(n: u64) -> (tempfile::TempDir, String, String) {
    let dir = healthy_dir(n);
    let names = seg_names(dir.path());
    let a = names[names.len() - 2].clone();
    let b = names[names.len() - 1].clone();
    swap_segment_files(dir.path(), &a, &b);
    let fs = first_seqs(dir.path());
    assert!(
        fs.windows(2).any(|w| w[0] >= w[1]),
        "setup-guard: перестановка обязана СЛОМАТЬ монотонность, получено {fs:?}"
    );
    (dir, a, b)
}

fn err_names_both(msg: &str, a: &str, b: &str) -> bool {
    msg.contains(a) && msg.contains(b)
}

// ═════════════════════════════════════════════════════════════════════════════════════
// MN-1 — read_all: тихий беспорядок seq вместо отказа
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn mn_1_read_all_refuses_non_monotonic_catalogue() {
    let (dir, a, b) = restitched_dir(400);

    match journal::read_all(dir.path()) {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("first_seq"),
                "диагностика обязана назвать нарушенное свойство (`first_seq`): «{msg}»"
            );
            assert!(
                err_names_both(&msg, &a, &b),
                "диагностика обязана назвать ОБА файла ({a}, {b}) — иначе оператор не \
                 знает, какой сегмент вернуть в карантин: «{msg}»"
            );
        }
        Ok(evs) => {
            let seqs: Vec<u64> = evs.iter().map(|e| e.seq).take(12).collect();
            panic!(
                "JR-I-11 НАРУШЕН: `read_all` СШИЛ немонотонный каталог молча. Первые seq \
                 выдачи: {seqs:?} — тотальный порядок журнала нарушен, и ни один \
                 потребитель (реплей, отчёты, проекции) об этом не узнает. Ошибочный \
                 re-stitch архива в живой каталог обязан быть ОТКАЗОМ, а не тихим \
                 беспорядком seq."
            );
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// MN-2 — stream/stream_from: прод-путь чтения
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn mn_2_stream_refuses_non_monotonic_catalogue() {
    let (dir, a, b) = restitched_dir(400);

    // Отказ обязан наступить ДО выдачи первого события: `stream` — прод-путь чтения,
    // потребитель не должен успеть увидеть ни одного события из перемешанного каталога.
    match journal::stream(dir.path(), EpochFilter::All) {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("first_seq") && err_names_both(&msg, &a, &b),
                "диагностика обязана назвать свойство и ОБА файла: «{msg}»"
            );
        }
        Ok(s) => {
            let seqs: Vec<u64> = s.filter_map(|e| e.ok()).map(|e| e.seq).take(12).collect();
            panic!(
                "JR-I-11 НАРУШЕН: `stream` открылся на немонотонном каталоге и отдал \
                 события {seqs:?}. Это ПРОД-путь чтения (реплей, gateway, research) — \
                 перемешанный seq уходит во все проекции сразу."
            );
        }
    }

    // Тот же каталог через `stream_from` (live-seek M-38b) — обязан отказать так же:
    // сегментный пропуск по `first_seq` на немонотонном каталоге тем более неверен.
    let (dir2, _, _) = restitched_dir(400);
    assert!(
        journal::stream_from(dir2.path(), EpochFilter::All, Some(5)).is_err(),
        "JR-I-11 НАРУШЕН: `stream_from` не проверяет монотонность — а он ещё и ПРОПУСКАЕТ \
         сегменты по `first_seq`, то есть на немонотонном каталоге режет данные молча"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// MN-3 — readable_floor: немонотонность даёт ЗАНИЖЕННЫЙ пол (условие закрытия R-002/R-003)
// ═════════════════════════════════════════════════════════════════════════════════════

/// `readable_floor` идёт с КОНЦА и останавливается на первом сегменте с валидным фреймом.
/// Если в конце каталога лежит сегмент с НИЗКИМИ seq (архив вернули под старшим индексом),
/// пол занижается до его максимума — и декларация с `next_seq` внутри диапазона,
/// занятого сегментом с ВЫСОКИМИ seq, проходит валидацию. Fail-open ⇒ seq-reuse.
#[test]
fn mn_3_readable_floor_refuses_non_monotonic_catalogue() {
    let (dir, a, b) = restitched_dir(400);
    let names = seg_names(dir.path());
    let last = names.last().expect("есть сегменты").clone();

    // Диапазон, занятый сегментом с ВЫСОКИМИ seq (он теперь под МЛАДШИМ из двух индексов).
    let fs = first_seqs(dir.path());
    let high_first = *fs.iter().max().expect("first_seq");
    // Максимум, читаемый из ХВОСТОВОГО (теперь низкого) сегмента — тот самый заниженный пол.
    let low_max = max_seq_in(&std::fs::read(dir.path().join(&last)).expect("read"))
        .expect("setup-guard: хвостовой сегмент читается");
    assert!(
        low_max < high_first,
        "setup-guard: перестановка обязана дать хвост с seq НИЖЕ занятого диапазона \
         (low_max={low_max}, high_first={high_first})"
    );

    // Хвост делаем нечитаемым — иначе путь декларации не входится (`op_3`).
    append_bytes(&dir.path().join(&last), &vec![0x5A_u8; CHEAP_TAIL]);
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    // Декларация ВНУТРИ занятого диапазона: выше заниженного пола, но внутри high-сегмента.
    let bad_next = high_first + 1;
    assert!(bad_next > low_max, "setup-guard: декларация обязана быть выше заниженного пола");
    write_decl(dir.path(), bad_next, "ошибка оператора: архив вернули в живой каталог");

    match Journal::open_with(dir.path(), cfg()) {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("first_seq") && err_names_both(&msg, &a, &b),
                "диагностика пола обязана назвать НАСТОЯЩУЮ причину (немонотонность и оба \
                 файла), иначе оператор пойдёт искать порчу там, где её нет: «{msg}»"
            );
            assert!(
                !ls(dir.path()).iter().any(|n| n == DECL_APPLIED),
                "отвергнутая декларация не должна помечаться применённой"
            );
        }
        Ok(j) => panic!(
            "JR-I-11 НАРУШЕН (fail-open на пути защиты): декларация next_seq={bad_next} \
             ПРИНЯТА (старт с {}), хотя журнал содержит сегмент с first_seq={high_first} — \
             запись пойдёт ПОВЕРХ занятого диапазона. Пол занижен до {low_max}, потому что \
             `readable_floor` останавливается на первом сегменте с конца и ВЕРИТ в \
             монотонность, которую никто не проверяет. Это ровно условие закрытия TD-030, \
             принятое reviewer'ом в R-002/R-003: guard обязан покрыть и `readable_floor`.",
            j.next_seq()
        ),
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// MN-4 — МНОЖЕСТВЕННОСТЬ: РАВНЫЕ first_seq (дубль сегмента) — не «строго возрастает»
// ═════════════════════════════════════════════════════════════════════════════════════

/// Вторая форма того же операторского промаха: архивный сегмент скопирован в живой каталог
/// под НОВЫМ индексом (а не переставлен). Индексы монотонны, `first_seq` — нет: они РАВНЫ.
/// Дубликат событий уходит в реплей молча (D-COMP-1 дедуплицирует только коллизию
/// `raw`+`.zst` ОДНОГО индекса, а здесь индексы разные).
#[test]
fn mn_4_duplicate_segment_under_new_index_is_refused() {
    let dir = healthy_dir(400);
    let names = seg_names(dir.path());
    // Копируем именно ПОСЛЕДНИЙ сегмент: тогда дубль встаёт СРАЗУ ЗА оригиналом и даёт
    // РАВНЫЕ соседние `first_seq` — форму, которую ловит «строго возрастает» и не ловит
    // «не убывает». Копия более раннего сегмента дала бы обычное убывание (форма MN-1..3).
    let src = names.last().expect("есть сегменты").clone();
    let last_idx: u32 = names
        .last()
        .and_then(|n| n.strip_prefix("segment-"))
        .and_then(|n| n.split('.').next())
        .and_then(|n| n.parse().ok())
        .expect("индекс последнего сегмента");
    let dup = format!("segment-{:08}.jrnl", last_idx + 1);
    std::fs::copy(dir.path().join(&src), dir.path().join(&dup)).expect("copy");

    let fs = first_seqs(dir.path());
    assert!(
        fs.windows(2).any(|w| w[0] == w[1]),
        "setup-guard: дубликат обязан дать РАВНЫЕ first_seq, получено {fs:?}"
    );

    let n_before = journal::read_all(dir.path()).map(|v| v.len()).unwrap_or(0);
    match journal::read_all(dir.path()) {
        Err(e) => assert!(
            e.to_string().contains("first_seq"),
            "диагностика обязана назвать нарушенное свойство: «{e}»"
        ),
        Ok(evs) => panic!(
            "JR-I-11 НАРУШЕН: дубликат сегмента под новым индексом прочитан молча — \
             {} событий вместо ожидаемых уникальных ({n_before} с дублем). Требование \
             «строго возрастает» ловит эту форму; «не убывает» — не ловит.",
            evs.len()
        ),
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// MN-5 — ПАРНЫЙ VANTAGE / LEGACY-СЕНТИНЕЛ (класс TD-011): guard НЕ смеет сработать
// ═════════════════════════════════════════════════════════════════════════════════════

/// Legacy-сегмент (без магии, до CT-RFC-02) получает СИНТЕЗИРОВАННЫЙ `first_seq = 0` —
/// это «неизвестно», а не «ноль». Каталог, где такой сегмент стоит ПОСЛЕ v2-сегментов с
/// ненулевым `first_seq`, выглядит для наивного guard'а как немонотонный — и наивный guard
/// уронил бы ЧТЕНИЕ БОЕВОГО каталога (на проде legacy-сегмент существует и задекларирован:
/// `journal.legacy.json` лежит в `_data`, замер 2026-08-02).
///
/// Оракул ЗЕЛЁНЫЙ сегодня и ОБЯЗАН остаться зелёным: он пиннит, что защита от TD-030 не
/// куплена ценой прод-регрессии класса TD-011.
#[test]
fn mn_5_legacy_sentinel_first_seq_is_not_a_violation() {
    let dir = tempfile::tempdir().expect("dir");
    // v2-сегменты с ненулевыми first_seq.
    let next_after_v2;
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..400 {
            j.append(common::trade(i)).expect("append");
        }
        j.flush().expect("flush");
        next_after_v2 = j.next_seq();
    }
    let names = seg_names(dir.path());
    let last_idx: u32 = names
        .last()
        .and_then(|n| n.strip_prefix("segment-"))
        .and_then(|n| n.split('.').next())
        .and_then(|n| n.parse().ok())
        .expect("индекс");

    // Legacy-сегмент СТАРШЕГО индекса: без магии, без заголовка — сырые фреймы.
    let legacy_name = format!("segment-{:08}.jrnl", last_idx + 1);
    let legacy_path = dir.path().join(&legacy_name);
    let mut raw: Vec<u8> = Vec::new();
    for k in 0..5u64 {
        raw.extend_from_slice(&frame_of(&Event {
            seq: next_after_v2 + k,
            ts_mono_ns: common::TS_MONO,
            ts_wall_ms: common::T0,
            kind: EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: contracts::to_fixed(65_000.0),
                    size: contracts::to_fixed(0.01),
                    side: Side::Buy,
                    ts_exch_ms: common::T0,
                },
            ),
        }));
    }
    std::fs::write(&legacy_path, &raw).expect("write legacy");

    // Декларация ДО измерения формы: незадекларированный безголовый сегмент — `ForeignSegment`
    // для `list_segments`, и `first_seqs` вернул бы пустоту (форма фикстуры не проверена бы).
    let decl = LegacySegmentDecl {
        file_name: legacy_name.clone(),
        fingerprint_sha256: journal::fingerprint(&legacy_path).expect("fingerprint"),
        size_bytes_at_decl: std::fs::metadata(&legacy_path).expect("meta").len(),
        source: DataSource::OwnCapture,
        provenance: "pre-RFC02 capture re-attached under a higher index".to_string(),
        epoch_id: contracts::LEGACY_EPOCH_ID.to_string(),
    };
    journal::declare_legacy(dir.path(), decl).expect("declare_legacy");

    let fs = first_seqs(dir.path());
    assert_eq!(
        fs.last().copied(),
        Some(0),
        "setup-guard: legacy-сегмент обязан нести СЕНТИНЕЛ first_seq=0 (а не реальный), \
         иначе фикстура не давит на инвариант; получено {fs:?}"
    );
    assert!(
        fs[..fs.len() - 1].iter().any(|&x| x > 0),
        "setup-guard: перед legacy обязан стоять v2-сегмент с НЕНУЛЕВЫМ first_seq — \
         именно это соседство наивный guard принял бы за немонотонность; {fs:?}"
    );

    // (1) read_all — офлайн-диагностика, манифеста не требует: обязан ПРОЧИТАТЬ.
    let evs = journal::read_all(dir.path()).unwrap_or_else(|e| {
        panic!(
            "КЛАСС TD-011: guard монотонности сработал на LEGACY-сентинеле — `read_all` \
             отказал на каталоге, который читается сегодня. `first_seq=0` у legacy — это \
             СИНТЕЗИРОВАННЫЙ дефолт («реальный неизвестен без чтения сегмента»), а не \
             факт: сегмент обязан быть ИСКЛЮЧЁН из сравнения по `schema_version`, а не \
             сравниваться по значению. Ошибка: {e}"
        )
    });
    assert_eq!(
        evs.len(),
        405,
        "legacy-события обязаны читаться наравне с v2 (400 + 5)"
    );

    // (2) stream — прод-путь: задекларированный legacy обязан отдавать данные.
    let n = journal::stream(dir.path(), EpochFilter::All)
        .unwrap_or_else(|e| {
            panic!(
                "КЛАСС TD-011: guard монотонности уронил ПРОД-путь чтения на \
                 задекларированном legacy-сегменте (сентинел first_seq=0): {e}"
            )
        })
        .filter_map(|e| e.ok())
        .count();
    assert_eq!(n, 405, "stream обязан отдать legacy + v2 события");
}

// ═════════════════════════════════════════════════════════════════════════════════════
// MN-6 — ГРАНИЦЫ / ОТСУТСТВИЕ: разрыв индексов после ретеншена — НЕ нарушение
// ═════════════════════════════════════════════════════════════════════════════════════

/// Ретеншен выгружает и УДАЛЯЕТ старые сегменты — дыры в нумерации индексов штатны
/// (на проде каталог начинается далеко не с `segment-00000000`). Guard обязан проверять
/// монотонность `first_seq`, а не непрерывность индексов.
///
/// Здесь же — границы: каталог из ОДНОГО сегмента и пустой каталог (сравнивать нечего).
#[test]
fn mn_6_index_gaps_single_and_empty_catalogues_are_fine() {
    // (а) дыра в индексах.
    let dir = healthy_dir(400);
    let names = seg_names(dir.path());
    std::fs::remove_file(dir.path().join(&names[1])).expect("remove middle");
    let n = journal::stream(dir.path(), EpochFilter::All)
        .unwrap_or_else(|e| panic!("разрыв индексов после ретеншена — НЕ немонотонность: {e}"))
        .filter_map(|e| e.ok())
        .count();
    assert!(n > 0, "оставшиеся сегменты обязаны читаться");
    assert!(
        journal::read_all(dir.path()).is_ok(),
        "read_all обязан читать каталог с дырой в индексах"
    );

    // (б) один сегмент — сравнивать не с чем.
    let one = tempfile::tempdir().expect("dir");
    {
        let mut j =
            Journal::open_with(one.path(), cfg_with(64 * 1024 * 1024, "single")).expect("open");
        for i in 0..10 {
            j.append(common::trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    assert_eq!(seg_names(one.path()).len(), 1, "setup-guard: ровно один сегмент");
    assert!(journal::read_all(one.path()).is_ok(), "один сегмент — не нарушение");
    assert!(
        journal::stream(one.path(), EpochFilter::All).is_ok(),
        "один сегмент — не нарушение"
    );

    // (в) пустой каталог.
    let empty = tempfile::tempdir().expect("dir");
    assert!(journal::read_all(empty.path()).is_ok(), "пустой каталог — не нарушение");
    assert!(
        journal::stream(empty.path(), EpochFilter::All).is_ok(),
        "пустой каталог — не нарушение"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// MN-7 — ПАРНЫЙ VANTAGE на СМЕШАННОМ формате (форма прода: 152 `.zst` + 6 сырых)
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn mn_7_mixed_raw_and_compacted_monotonic_catalogue_reads() {
    let dir = healthy_dir(400);
    journal::compact_closed_segments(dir.path(), 1, 3).expect("compact");
    let names = seg_names(dir.path());
    assert!(
        names.iter().any(|n| n.ends_with(".zst")) && names.iter().any(|n| n.ends_with(".jrnl")),
        "setup-guard: каталог обязан быть СМЕШАННЫМ (форма прода), получено {names:?}"
    );
    let fs = first_seqs(dir.path());
    assert!(
        fs.windows(2).all(|w| w[0] < w[1]),
        "setup-guard: смешанный каталог обязан остаться монотонным, {fs:?}"
    );
    let n = journal::stream(dir.path(), EpochFilter::All)
        .unwrap_or_else(|e| panic!("монотонный смешанный каталог обязан читаться: {e}"))
        .filter_map(|e| e.ok())
        .count();
    assert_eq!(n, 400, "смешанный каталог обязан отдать все события");
}
