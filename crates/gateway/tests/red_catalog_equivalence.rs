//! `SM-11` — тёплый каталог сессии НАБЛЮДАТЕЛЬНО ЭКВИВАЛЕНТЕН холодному наблюдению
//! (M-62 / `TD-120`, круг 3; блокер `Б-4` из `R-056`, классы A и B из
//! `docs/plans/M-62-class-sweep-round3.md` §1).
//!
//! # ЗАЧЕМ ЕЩЁ ОДИН НАБОР, КОГДА ЕСТЬ `SM-0..SM-10`
//!
//! Батарея `red_segment_meta_bound.rs` сертифицировала ЦЕНУ тика (оси 1-5 спеки §4.2) и
//! проверяет СОСТАВ кеша сверкой МНОЖЕСТВ ИНДЕКСОВ (`cache_indices` vs `truth_indices`).
//! Замер круга 3 показал, что этого мало ровно в двух местах, и оба — не про цену:
//!
//! 1. **Множество индексов слепо к ПУТИ.** Развязка «не удалять запись из кеша, пока
//!    `parse_segment_index_any` этого индекса есть в каком-нибудь имени каталога» оставляет
//!    в кеше `SegmentInfo`, чей `path` указывает на УДАЛЁННЫЙ файл. Индексы при этом
//!    совпадают с эталоном, выдача умирает `Os { code: 2, NotFound }` на прод-пути
//!    `LiveReducer::pump` — и ВСЕ 10 оракулов `SM` при этом зелены
//!    (`M-62-class-sweep-round3.md` §Р-2, `test result: ok. 10 passed`). Неверный фикс
//!    прошёл бы сегодняшний гейт целиком.
//! 2. **`assert!(fresh, ...)` пиннит ВЕТКУ ИСПОЛНЕНИЯ, а не инвариант.**
//!    `sm8_per_segment_compaction_keeps_catalog_truthful`
//!    (`red_segment_meta_bound.rs:695-700`) требует `is_fresh == true`. Развязка «при
//!    коллизии индекса уходить в `refresh()`» даёт КОРРЕКТНЫЙ end-to-end результат и всё
//!    равно роняет `sm8` — на его СОБСТВЕННОМ setup-guard'е (§Р-3). Сегодняшний набор
//!    запрещает развязку, которую сам вердикт `R-056` разрешает.
//!
//! Отсюда конструкция SM-11: **ни одного утверждения о том, КАКОЙ веткой достигнут
//! результат.** Проверяется только то, что наблюдает потребитель, и всё — против
//! НЕЗАВИСИМОГО пути.
//!
//! # ИНВАРИАНТ (I2 «правдивость», `M-62-class-sweep-round3.md` §4)
//!
//! > После ЛЮБОЙ последовательности событий каталога тёплый кеш наблюдательно эквивалентен
//! > холодному наблюдению: тот же ВЕРДИКТ, тот же СОСТАВ (индексы И ПУТИ И `first_seq`),
//! > та же ВЫДАЧА — и всё это в пределах бюджета (иначе «всегда `refresh()`» проходит
//! > эквивалентность даром).
//!
//! Это НЕ вариант оси 4 («что изменилось в каталоге»). Ось 6 — **длина и композиция
//! ИСТОРИИ между полными наблюдениями**: одно событие, разнесённое на ДВА такта; два
//! события в ОДИН такт; `k` тактов без `refresh()`, включая ротацию (замерено: ротация
//! порчу НЕ лечит, `segments.rs:278`); граница `small_change` 2/2 против 3.
//!
//! # ЧЕТЫРЕ СВЕРКИ НА КАЖДОМ ТАКТЕ, И ПРОТИВ ЧЕГО КАЖДАЯ
//!
//! | # | сверка | эталон | падает против |
//! |---|---|---|---|
//! | 1 | ВЕРДИКТ: `warm.is_ok() == list_segments(dir).is_ok()` | холодный полный путь | класс B целиком — тёплая сессия принимает каталог, который холодная ОТВЕРГАЕТ (`JR-I-11`), причём не зная, КАКОЙ guard нарушен |
//! | 2 | СОСТАВ: `(index, имя файла, first_seq)` кеша == тот же список от `list_segments` | холодный полный путь | Б-4 (запись пропала), развязка №1 (путь на удалённый файл), дубль индекса |
//! | 3 | ВЫДАЧА: `seq` тёплого потока == `seq` полного реплея `stream_from` от ТОЙ ЖЕ позиции | НЕЗАВИСИМЫЙ реплей | недоотдача/дубли/`ENOENT` при iterate — то, что видит клиент |
//! | 4 | БЮДЖЕТ: `segment_meta_ops` на УСТАНОВИВШЕМСЯ такте ≤ `BUDGET_META`, на первом ≥ `N` | абсолютный счётчик | «всегда `refresh()`» (эквивалентность даром) и `countfake` (первый тик обязан реально обойти каталог) |
//!
//! **Почему пути ОБЯЗАТЕЛЬНЫ в (2):** без них развязка №1 зелена (§Р-2).
//! **Почему `first_seq` в (2):** запись, «сохранённая» вместо переклассификации, несёт
//! заголовок УДАЛЁННОГО файла. `size_bytes` в сверку НЕ входит сознательно: у активного
//! сегмента он законно растёт между тиками, и его сравнение давало бы красное по НЕВЕРНОЙ
//! причине (`testing.md` §«фикстура не должна стоять РОВНО на границе»).
//!
//! # ЭТАЛОН — НЕЗАВИСИМЫЙ ПУТЬ, А НЕ ТОТ ЖЕ КЕШ
//!
//! `journal::list_segments` строит перечень с нуля (`segments_counted`: manifest → dedup →
//! classify → sort → guard), `journal::stream_from` делает полный реплей без кеша и без
//! `hint`. Сверять кеш с кешем — тавтология (`testing.md` §«Зависимый эталон мутация ловит
//! плохо»). Предмет — прод-точка входа `journal::stream_from_at_with_catalog`: её и только
//! её зовёт `LiveReducer::pump` (`gateway/src/lib.rs:3150`).
//!
//! # SETUP-GUARD НА КАЖДЫЙ СЦЕНАРИЙ — НО НЕ НА ВЕТКУ
//!
//! Guard'ы здесь проверяют, что СЦЕНАРИЙ СОСТОЯЛСЯ НА ДИСКЕ (пара файлов появилась; ровно
//! один файл исчез; за такт добавилось ровно три файла; события жертвы вообще видны
//! из-под курсора), и ни один не проверяет, КАКОЙ веткой прошла реализация. Проба, молча
//! тестирующая не тот сценарий, есть плацебо самой себя; проба, требующая конкретной
//! ветки, — запрет на верный фикс (урок `sm8`, §Р-3).
//!
//! **Курсор стоит ПЕРЕД жертвой — и это guard, а не подразумеваемое.** При курсоре у хвоста
//! расхождение выдачи равно НУЛЮ даже на испорченном кеше: жертвой всегда оказывается
//! старый сегмент, а отбор по `after_seq` (`segments.rs:1819-1836`) его и так выбрасывает
//! (замер `L3-1` §ЧТО ОПРОВЕРГНУТО). Поэтому перед каждым сценарием проверяется, что
//! события жертвы РЕАЛЬНО присутствуют в эталонном реплее от текущего курсора.
//!
//! # ДЕГЕНЕРИРОВАННЫЙ ВХОД (`testing.md` §«Дегенерированный вход обязателен»)
//!
//! - **асимметрия** — обновляется ОДНА сторона пары `.jrnl`/`.jrnl.zst` (шаг 7 компакции,
//!   self-heal, уборка `.zst` при живом сыром);
//! - **множественность** — два файловых события в ОДИН такт и три файла за такт (граница
//!   `small_change` 2/2 против 3), плюс НЕСКОЛЬКО ЖИЗНЕЙ одного индекса: сегмент рождается
//!   сырым, обрастает `.zst`, теряет сырой;
//! - **отсутствие** — исчезновение файла НЕ равно исчезновению сегмента: у индекса остаётся
//!   второй файл, и реализация не имеет права додумывать за источник;
//! - **границы** — такт без единого изменения (ранний выход), такт с тремя файлами
//!   (`refresh`), посторонний `.tmp` (не сегмент вообще).
//!
//! # ЧЕГО ЭТОТ ОРАКУЛ НЕ ЛОВИТ — ЗАМЕРЕНО, А НЕ ОГОВОРЕНО
//!
//! Предел, названный числом, дешевле предела, найденного следующим кругом.
//!
//! 1. **Цена такта С СОБЫТИЕМ каталога не ограничена ничем** (`Budget::Free`). Это
//!    сознательно: требовать бюджет на такте изменения значило бы запретить развязку
//!    «уходить в `refresh()`» — ошибка `sm8`, пиннившего ветку. Следствие названо числом:
//!    развязка №2 платит 415-418 операций на такте события против 6-11 у переклассификации
//!    (замер приёмки). Вырождение «`refresh()` на КАЖДОМ такте» ловится сверкой (4) на
//!    установившемся такте (мутант `alwaysrefresh`: 409-413 при бюджете 8), поэтому дыры в
//!    защите нет — есть неизмеряемая здесь величина.
//! 2. **`Err` В СЕРЕДИНЕ применения дифа (G5) этим оракулом не наблюдаем — структурно.**
//!    Замер: мутант `namescommit` (коммит `file_names` ПЕРЕД применением дифа) проходит
//!    набор с ПУСТЫМ kill-set'ом. Причина не в выборе тактов: прод-точка входа
//!    `stream_from_at_with_catalog` принимает каталог ПО ЗНАЧЕНИЮ и на `Err` его теряет,
//!    поэтому «тёплый кеш, переживший отказ» через неё не выражается вовсе. Оракул на G5
//!    обязан наблюдать `SegmentCatalog::is_fresh` напрямую — это ДРУГОЙ предмет и другой
//!    файл; заготовка сценария есть у мутант-агента круга 3.
//! 3. **Три проверки ВНУТРИ `classify_segment`** (усечение legacy ниже декларации,
//!    fingerprint-подмена, битый v2-заголовок) не отделены от `check_first_seq_monotonic`:
//!    мутант `classifyskip` (отказ классификации проглочен `continue`) проходит набор с
//!    ПУСТЫМ kill-set'ом. Сверка (1) наследует ВСЕ проверки полного пути разом — и потому
//!    же не различает, какая из них выключена.
//! 4. **G1** — изменение содержимого при неизменном имени: `sm11d` под `#[ignore]`,
//!    вид `G` в манифесте (см. выше).
//!
//! # СОСТОЯНИЕ НАБОРА: RED против `3115628`
//!
//! `sm11` красен на такте «шаг 7 компакции» и на его зеркале; `sm11c` красен на инъекции
//! немонотонного сегмента. Это спецификация, написанная раньше кода (`gates.md` §2), а не
//! отчёт о поломке.

use std::fs;
use std::path::Path;

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, SegmentCatalog, SegmentInfo, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;

/// Прод-масштаб воспроизводится ПО ЧИСЛУ сегментов (205 на проде, замер §1.1 спеки), а не
/// по объёму — тот же приём, что в `red_segment_meta_bound.rs`.
const SEG_BYTES: u64 = 256;
const N_SEGMENTS: usize = 200;
/// Тот же абсолютный бюджет, что у батареи `SM-0..SM-6`. Дублируется намеренно: файл
/// самостоятельный, а расхождение констант между наборами — отдельный класс дефекта.
const BUDGET_META: u64 = 8;

// ─────────────────────────────────────────────────────────────────────────────────────────
// Манифест оси 6. Число сценариев СЧИТАЕТСЯ, а не заявляется литералом (урок `TD-125`:
// «ci.yml называет 26 при 27»), и сверка идёт в ОБЕ стороны: каждый исполненный такт
// обязан быть в манифесте (`claims`), каждый пункт манифеста обязан быть исполнен
// (`assert_manifest_executed`). Иначе перечень значений оси — декларация, а не покрытие.
// ─────────────────────────────────────────────────────────────────────────────────────────

/// `(метка такта, НОМЕР ОСИ, значение оси, вид)` — та же форма кортежа, что в
/// `red_segment_meta_bound.rs`, и это не косметика: шаг `N` гейта
/// (гейт M-62, архив: `docs/archive/verify_M-62.sh`) машинно сверял НА ПРИЁМКЕ манифесты
/// ВСЕХ файлов набора с таблицей
/// §4.2 спеки в обе стороны, а сверять он умеет ровно эту форму. Манифест в собственной
/// форме гейту невидим — и «полнота относительно перечня» снова становится декларацией.
///
/// Вид: `V` — обязано ловиться как нарушение при сломанной реализации; `L` — легитимный
/// случай, обязан оставаться зелёным ВСЕГДА; `G` — **значение объявлено, но НЕ покрыто**:
/// сценарий существует под `#[ignore]` и даёт гейту НОЛЬ. `G` намеренно невидим шагу `N`
/// (он извлекает только `V`/`L`): заявить непокрытое значение в таблице осей значило бы
/// сертифицировать воображение. Каждое `G` обязано быть названо в §7 спеки «чего
/// milestone НЕ делает» и нести TD — иначе дыра наследуется молча (`gates.md` §4,
/// «built-not-wired»).
///
/// `MANIFEST_AXIS6_A` — последовательность КЛАССА A (`sm11`), сверяется в ОБЕ стороны.
const MANIFEST_AXIS6_A: &[(&str, u8, &str, char)] = &[
    (
        "t0-first",
        6,
        "первое полное наблюдение (кеша ещё нет)",
        'L',
    ),
    ("idle", 6, "каталог не менялся — история пуста", 'L'),
    ("rotate", 6, "одно событие за такт: +1 файл", 'L'),
    (
        "compact-step6",
        6,
        "одно событие за такт: +1 файл рядом с живым",
        'L',
    ),
    (
        "compact-step7",
        6,
        "ОДНО изменение, разнесённое на ДВА такта (Б-4)",
        'V',
    ),
    (
        "compact-one-tact",
        6,
        "то же изменение ОДНИМ тактом (контроль)",
        'L',
    ),
    (
        "selfheal-remove-raw",
        6,
        "k тактов между появлением пары и уборкой сырого",
        'V',
    ),
    (
        "remove-zst-alive-raw",
        6,
        "зеркало: уборка .zst при живом сыром",
        'V',
    ),
    ("foreign-tmp", 6, "не-сегментный файл появился и исчез", 'L'),
    (
        "two-in-one-tact",
        6,
        "ДВА файловых события в один такт",
        'L',
    ),
    (
        "three-files-tact",
        6,
        "граница small_change: 3 файла за такт",
        'L',
    ),
    (
        "rotate-after-damage",
        6,
        "ротация ПОСЛЕ порчи — лечит ли она (замер: нет)",
        'V',
    ),
];

/// `MANIFEST_AXIS6_B` — такты КЛАССА B и G1 (`sm11c`, `sm11d`): они живут на СВОИХ фикстурах, потому
/// что нарушение делает каталог нечитаемым и остаток последовательности мерил бы хвост
/// предыдущего сценария, а не свой предмет.
const MANIFEST_AXIS6_B: &[(&str, u8, &str, char)] = &[
    (
        "inject-foreign-space",
        6,
        "новое ИМЯ несёт немонотонный first_seq (класс B)",
        'V',
    ),
    (
        "content-rewrite",
        6,
        "содержимое изменилось, имена — нет (G1)",
        'G',
    ),
];

fn claims(label: &str) {
    assert!(
        MANIFEST_AXIS6_A
            .iter()
            .chain(MANIFEST_AXIS6_B.iter())
            .any(|(l, _, _, _)| *l == label),
        "МАНИФЕСТ НЕ СОДЕРЖИТ такта «{label}»: перечень значений оси 6 стал бы ложью. \
         Такт, исполняемый вне манифеста, — покрытие, о котором никто не знает."
    );
}

fn assert_manifest_executed(executed: &[String]) {
    let missing: Vec<&str> = MANIFEST_AXIS6_A
        .iter()
        .map(|(l, _, _, _)| *l)
        .filter(|l| !executed.iter().any(|e| e == l))
        .collect();
    assert!(
        missing.is_empty(),
        "МАНИФЕСТ ⇒ ИСПОЛНЕНИЕ: значения оси 6 объявлены, но НЕ прогнаны: {missing:?}. \
         Набор, перечисляющий больше, чем исполняет, сертифицирует воображение."
    );
    eprintln!(
        "AXIS6: {} значений объявлено (A) + {} (B), {} тактов исполнено \
         (числа СЧИТАНЫ, не заявлены)",
        MANIFEST_AXIS6_A.len(),
        MANIFEST_AXIS6_B.len(),
        executed.len()
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// Фикстура прод-ФОРМЫ
// ─────────────────────────────────────────────────────────────────────────────────────────

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m62".to_string(),
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
            ts_exch_ms: D2_MS + (i as i64 * 100),
        },
    )
}

fn append_range(dir: &Path, from: u64, to: u64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in from..to {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
}

fn n_files(dir: &Path) -> usize {
    fs::read_dir(dir).expect("read_dir").count()
}

fn n_segments(dir: &Path) -> usize {
    journal::list_segments(dir).expect("list_segments").len()
}

/// Каталог прод-ФОРМЫ: `target` сегментов, СМЕШАННЫХ raw + `.zst` (advisory `C-072`: на проде
/// 198 из 205 сжаты, и `classify_segment` идёт для них ДРУГОЙ веткой).
fn build_prod_form(target: usize) -> (tempfile::TempDir, u64) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut n = 0u64;
    while n_segments(dir.path()) < target {
        append_range(dir.path(), n, n + 32);
        n += 32;
    }
    journal::compact_closed_segments(dir.path(), 4, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact_closed_segments");

    let names: Vec<String> = fs::read_dir(dir.path())
        .expect("read_dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    let raw = names.iter().filter(|s| s.ends_with(".jrnl")).count();
    let zst = names.iter().filter(|s| s.ends_with(".jrnl.zst")).count();
    assert!(
        n_segments(dir.path()) >= target,
        "SETUP НЕ СОСТОЯЛСЯ: сегментов {} при цели {target}",
        n_segments(dir.path())
    );
    assert!(
        raw >= 2 && zst > 0,
        "SETUP НЕ СОСТОЯЛСЯ: каталог не СМЕШАННЫЙ (raw={raw}, zst={zst}). Сценариям нужны \
         И сжатые (другая ветка classify), И как минимум два ЗАКРЫТЫХ сырых сегмента — \
         жертвы компакционных тактов."
    );
    (dir, n)
}

/// Дописывать по одному событию, пока число сегментов не вырастет на `k`. Форма прода:
/// recorder дописывает в активный сегмент, ротация случается сама по достижении кванта.
fn grow_segments(dir: &Path, n: &mut u64, k: usize) {
    let before = n_segments(dir);
    let mut guard = 0;
    while n_segments(dir) < before + k {
        append_range(dir, *n, *n + 1);
        *n += 1;
        guard += 1;
        assert!(
            guard < 500,
            "SETUP НЕ СОСТОЯЛСЯ: за {guard} событий каталог не вырос на {k} сегмент(ов) \
             (было {before}, стало {}). Квант ротации изменился — фикстура мерила бы не тот \
             сценарий.",
            n_segments(dir)
        );
    }
}

/// НЕЗАВИСИМЫЙ эталон выдачи: полный реплей без кеша и без `hint`.
fn reference_seqs(dir: &Path, after: u64) -> Vec<u64> {
    let mut s = journal::stream_from(dir, EpochFilter::OwnCaptureOnly, Some(after))
        .expect("эталон: stream_from");
    let mut out = Vec::new();
    for ev in s.by_ref() {
        out.push(ev.expect("эталон: событие").seq);
    }
    out
}

/// Состав каталога в сравнимой форме. `size_bytes` НЕ включён сознательно — см. шапку.
fn shape(segs: &[SegmentInfo]) -> Vec<(u32, String, u64)> {
    segs.iter()
        .map(|s| {
            (
                s.index,
                s.path
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                s.header.first_seq,
            )
        })
        .collect()
}

/// Сколько событий ЖЕРТВЫ реально видно из-под текущего курсора. Ноль ⇒ сверка выдачи в
/// этом сценарии ничего не проверяет, и об этом нужно знать ДО прогона, а не после.
fn victim_events_visible(dir: &Path, anchor: u64, victim: &SegmentInfo) -> usize {
    let cold = journal::list_segments(dir).expect("эталон: list_segments");
    let pos = cold
        .iter()
        .position(|s| s.index == victim.index)
        .expect("жертва обязана быть в каталоге на момент проверки");
    let lo = cold[pos].header.first_seq;
    let hi = cold
        .get(pos + 1)
        .map(|s| s.header.first_seq)
        .unwrap_or(u64::MAX);
    reference_seqs(dir, anchor)
        .iter()
        .filter(|s| **s >= lo && **s < hi)
        .count()
}

/// Закрытый СЫРОЙ сегмент, годный в жертвы: не активный (не максимальный индекс), ещё не
/// использованный, и его события лежат ПОСЛЕ курсора.
fn pick_closed_raw(dir: &Path, used: &[u32], anchor: u64) -> SegmentInfo {
    let cold = journal::list_segments(dir).expect("list_segments");
    let max_idx = cold
        .iter()
        .map(|s| s.index)
        .max()
        .expect("непустой каталог");
    let v = cold
        .iter()
        .find(|s| {
            s.index != max_idx
                && !used.contains(&s.index)
                && s.header.first_seq > anchor
                && s.path
                    .file_name()
                    .map(|f| f.to_string_lossy().ends_with(".jrnl"))
                    .unwrap_or(false)
        })
        .cloned();
    v.unwrap_or_else(|| {
        panic!(
            "SETUP НЕ СОСТОЯЛСЯ: не нашлось ЗАКРЫТОГО СЫРОГО сегмента после курсора \
             (использованы {used:?}, курсор {anchor}). Компакционные сценарии требуют \
             сырую жертву; без неё такт молча проверял бы не тот сценарий."
        )
    })
}

fn path_of(dir: &Path, name: &str) -> std::path::PathBuf {
    dir.join(name)
}

fn file_name_of(s: &SegmentInfo) -> String {
    s.path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// Наблюдение такта: ЧЕТЫРЕ сверки, ни одной — о ветке исполнения
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Какой бюджет применим к такту. `Steady` — только там, где состав каталога НЕ менялся:
/// это единственный такт, на котором ЛЮБАЯ честная реализация обязана уложиться в
/// `BUDGET_META` (ранний выход по совпадению имён). Требовать бюджет на такте С
/// ИЗМЕНЕНИЕМ значило бы запретить развязку «уходить в `refresh()`», то есть повторить
/// ошибку `sm8` — пиннить ветку вместо инварианта.
#[derive(Clone, Copy, PartialEq)]
enum Budget {
    /// Первый такт сессии: обязан РЕАЛЬНО обойти каталог (`>= N`), иначе кеш собран из
    /// воздуха — это ловит `countfake`, которого верхний порог не ловит в принципе.
    First,
    /// Состав каталога не менялся ⇒ `<= BUDGET_META`.
    Steady,
    /// Каталог менялся: цена — предмет батареи `SM-0..SM-6`, здесь не судится.
    Free,
}

struct Tick {
    cat: Option<SegmentCatalog>,
    findings: Vec<String>,
    executed: Vec<String>,
}

impl Tick {
    fn new() -> Self {
        Self {
            cat: None,
            findings: Vec::new(),
            executed: Vec::new(),
        }
    }

    /// Один такт живой сессии + четыре сверки против НЕЗАВИСИМОГО пути.
    ///
    /// Находки НАКАПЛИВАЮТСЯ, а не роняют тест на первой: круг 3 закрывает ДВА класса, и
    /// автору фикса нужна вся таблица, а не первая строка. Setup-guard'ы, наоборот, роняют
    /// немедленно — несостоявшийся setup не находка, а отсутствие замера.
    fn observe(&mut self, dir: &Path, anchor: u64, label: &str, budget: Budget) {
        claims(label);
        self.executed.push(label.to_string());

        // (0) Холодное наблюдение — НЕЗАВИСИМЫЙ путь: manifest → dedup → classify → sort →
        //     guard (`segments_counted`, :1056-1088).
        let cold_res = journal::list_segments(dir);
        let cold_ok = cold_res.is_ok();
        // Предмет — прод-точка входа: ровно её зовёт `LiveReducer::pump`.
        let warm_res = journal::stream_from_at_with_catalog(
            dir,
            EpochFilter::OwnCaptureOnly,
            Some(anchor),
            None,
            self.cat.take(),
        );

        // ── СВЕРКА 1: ВЕРДИКТ ────────────────────────────────────────────────────────────
        // Падает против класса B: тёплая сессия ПРИНИМАЕТ каталог, который холодный полный
        // путь ОТВЕРГАЕТ (`JR-I-11` монотонность `first_seq`, усечение legacy ниже
        // декларации, fingerprint-подмена, битый v2-заголовок). Сверка не знает, КАКОЙ
        // guard нарушен, — и это её сила: она наследует ВСЕ проверки полного пути, включая
        // те, что появятся позже.
        if cold_res.is_ok() != warm_res.is_ok() {
            self.findings.push(format!(
                "[{label}] ВЕРДИКТ: холодный путь {}, тёплая сессия {}. fail-closed guard \
                 полного пути (`segments_counted` → `check_first_seq_monotonic` и проверки \
                 внутри `classify_segment`) на тёплом пути НЕ исполняется: сигнал «стоп, \
                 руки оператора» у кокпита исчезает ровно тогда, когда журнал чинят руками. \
                 Холодный: {:?}",
                if cold_res.is_ok() { "Ok" } else { "Err" },
                if warm_res.is_ok() { "Ok" } else { "Err" },
                cold_res.as_ref().err().map(|e| e.to_string()),
            ));
        }

        let (cold, stream, cat_out) = match (cold_res, warm_res) {
            (Ok(c), Ok((s, k))) => (c, s, k),
            (_, Ok((_s, k))) => {
                // Холодный отверг, тёплая приняла — расхождение уже записано. Кеш
                // сохраняем: следующий такт покажет, ЛИПУЧЕЕ ли расхождение.
                self.cat = k;
                return;
            }
            (_, Err(e)) => {
                // Тёплый путь вернул ошибку: кеш не возвращается (он ушёл в вызов).
                // Находка — ТОЛЬКО если холодный путь при этом здоров: одинаковый `Err` с
                // обеих сторон есть КОРРЕКТНОЕ наследование fail-closed, а не расхождение
                // (иначе оракул краснел бы против верного фикса — обратная сторона
                // анти-плацебо, `testing.md` §«падает и против слишком строгой»).
                if cold_ok {
                    self.findings.push(format!(
                        "[{label}] тёплый путь вернул ошибку «{e}» на каталоге, который \
                         холодный полный путь принимает. Ложное срабатывание fail-closed: \
                         сессия ляжет на ШТАТНОМ событии каталога (посторонний `.tmp`, \
                         ротация, промежуточное состояние компакции) — блокер Б-3 в новой \
                         одежде."
                    ));
                }
                // Пересобираем кеш явно — иначе остаток последовательности не прогонится.
                // `open` на нечитаемом каталоге сам вернёт `Err` — тогда следующий такт
                // пойдёт холодным путём, что и есть корректное поведение.
                self.cat = SegmentCatalog::open(dir).ok().map(|(c, _)| c);
                return;
            }
        };

        // ── СВЕРКА 4 (снимается ДО чтения потока: счётчик выставлен построением) ─────────
        let ops = stream.segment_meta_ops();
        let n = cold.len() as u64;

        // ── СВЕРКА 3: ВЫДАЧА ────────────────────────────────────────────────────────────
        let mut warm_seqs = Vec::new();
        let mut read_errs = Vec::new();
        for ev in stream {
            match ev {
                Ok(e) => warm_seqs.push(e.seq),
                Err(e) => read_errs.push(e.to_string()),
            }
        }
        let ref_seqs = reference_seqs(dir, anchor);

        if !read_errs.is_empty() {
            self.findings.push(format!(
                "[{label}] ВЫДАЧА оборвалась ошибкой чтения: {:?}. Кеш держит запись, чей \
                 `path` не соответствует диску — ровно то, чем оборачивается «не удалять \
                 запись, пока индекс есть в каком-нибудь имени»: `SegmentInfo` остаётся с \
                 путём на УДАЛЁННЫЙ файл, и `pump()` умирает `Os {{ code: 2, NotFound }}` \
                 на прод-пути. Тихая потеря данных превращается в жёсткий Err.",
                read_errs
            ));
        }
        if warm_seqs != ref_seqs {
            let missing: Vec<u64> = ref_seqs
                .iter()
                .filter(|s| !warm_seqs.contains(s))
                .copied()
                .collect();
            let extra: Vec<u64> = warm_seqs
                .iter()
                .filter(|s| !ref_seqs.contains(s))
                .copied()
                .collect();
            self.findings.push(format!(
                "[{label}] ВЫДАЧА разошлась с полным реплеем: тёплая сессия отдала {} \
                 событий, эталон {}. НЕДОСТАЁТ {} (первые: {:?}), ЛИШНИХ {} (первые: {:?}). \
                 Это то, что видит клиент WS-кокпита при зелёном healthcheck — класс TD-031.",
                warm_seqs.len(),
                ref_seqs.len(),
                missing.len(),
                missing.iter().take(6).collect::<Vec<_>>(),
                extra.len(),
                extra.iter().take(6).collect::<Vec<_>>(),
            ));
        }

        // ── СВЕРКА 2: СОСТАВ (индексы И ПУТИ И first_seq) ───────────────────────────────
        if let Some(cat) = cat_out.as_ref() {
            let warm_shape = shape(cat.segments());
            let cold_shape = shape(&cold);
            if warm_shape != cold_shape {
                let only_warm: Vec<&(u32, String, u64)> = warm_shape
                    .iter()
                    .filter(|x| !cold_shape.contains(x))
                    .collect();
                let only_cold: Vec<&(u32, String, u64)> = cold_shape
                    .iter()
                    .filter(|x| !warm_shape.contains(x))
                    .collect();
                self.findings.push(format!(
                    "[{label}] СОСТАВ кеша разошёлся с каталогом. ТОЛЬКО В КЕШЕ: {:?}; \
                     ТОЛЬКО НА ДИСКЕ: {:?}. Сверяются ИНДЕКС + ИМЯ ФАЙЛА + first_seq: \
                     множества одних индексов совпадают и у записи, чей путь указывает на \
                     удалённый файл (§Р-2 разведки: все 10 оракулов SM при этом зелены).",
                    only_warm.iter().take(4).collect::<Vec<_>>(),
                    only_cold.iter().take(4).collect::<Vec<_>>(),
                ));
            }
        }

        // ── СВЕРКА 4: БЮДЖЕТ ────────────────────────────────────────────────────────────
        match budget {
            Budget::First => {
                if ops < n {
                    self.findings.push(format!(
                        "[{label}] БЮДЖЕТ: первый такт сессии выполнил {ops} операций с \
                         метаданными при {n} сегментах. Первый такт ОБЯЗАН реально обойти \
                         каталог; меньшее число означает счётчик вызовов вместо счётчика \
                         операций (мутант `countfake`) — верхний порог его не ловит в принципе."
                    ));
                }
            }
            Budget::Steady => {
                if ops > BUDGET_META {
                    self.findings.push(format!(
                        "[{label}] БЮДЖЕТ: такт БЕЗ изменения состава каталога выполнил \
                         {ops} операций при бюджете {BUDGET_META} (N={n}). Эквивалентность, \
                         купленная полным обходом на каждом тике, отменяет M-62: при 10 000 \
                         сессий × 4 тика/с это ~16 млн syscall'ов в секунду только на \
                         метаданные. Правдивость обязана достигаться НЕ ценой O(N) на \
                         установившемся такте."
                    ));
                }
            }
            Budget::Free => {}
        }

        eprintln!(
            "  такт [{label}]: N={n} ops={ops} выдача={} эталон={} состав={}",
            warm_seqs.len(),
            ref_seqs.len(),
            if cat_out
                .as_ref()
                .map(|c| shape(c.segments()) == shape(&cold))
                .unwrap_or(false)
            {
                "=="
            } else {
                "РАСХОЖДЕНИЕ"
            }
        );
        self.cat = cat_out;
    }

    /// Явный полный пересчёт ПОСЛЕ зафиксированной находки: следующий сценарий обязан
    /// мерить СВОЙ дефект, а не хвост предыдущего (атрибуция kill-set'а — `R-052` Б-4bis).
    fn resync_after_finding(&mut self, dir: &Path, before: usize) {
        if self.findings.len() > before {
            if let Some(cat) = self.cat.as_mut() {
                let _ = cat.refresh(dir).expect("refresh");
            }
            eprintln!("    ↳ находка зафиксирована; кеш пересобран явным refresh()");
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════════
// SM-11 — КЛАСС A: диф ИМЁН выдаётся за наблюдение СОСТОЯНИЯ
// ═════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn sm11_warm_catalog_stays_equivalent_to_cold_across_tact_sequence() {
    let (dir, mut n) = build_prod_form(N_SEGMENTS);
    let d = dir.path();

    // Курсор ставится ПЕРЕД зоной жертв (`~10` сегментов от хвоста): при курсоре У ХВОСТА
    // расхождение выдачи равно нулю по конструкции отбора `after_seq`, и сверка 3 молча
    // не проверяла бы ничего (замер `L3-1`).
    let cold0 = journal::list_segments(d).expect("list_segments");
    let anchor = cold0[cold0.len() - 10].header.first_seq - 1;
    assert!(
        reference_seqs(d, anchor).len() >= 8,
        "SETUP НЕ СОСТОЯЛСЯ: из-под курсора {anchor} видно {} событий — слишком мало, чтобы \
         потеря целого сегмента была отличима от шума",
        reference_seqs(d, anchor).len()
    );

    let mut t = Tick::new();
    let mut used: Vec<u32> = Vec::new();

    // ── t0: первое полное наблюдение. Кеша нет ⇒ полный обход обязателен. ────────────────
    t.observe(d, anchor, "t0-first", Budget::First);
    // ── холостой такт: ранний выход по совпадению имён; бюджет судится ЗДЕСЬ и только здесь.
    t.observe(d, anchor, "idle", Budget::Steady);

    // ── СЦЕНАРИЙ 1: ротация (+1 файл). Легитимный такт: сессия обязана УВИДЕТЬ новый
    //    сегмент. Падает против `staleforever` («кеш навсегда»).
    {
        let files_before = n_files(d);
        grow_segments(d, &mut n, 1);
        assert_eq!(
            n_files(d),
            files_before + 1,
            "SETUP НЕ СОСТОЯЛСЯ: ротация добавила не один файл"
        );
        let before = t.findings.len();
        t.observe(d, anchor, "rotate", Budget::Free);
        t.observe(d, anchor, "idle", Budget::Steady);
        t.resync_after_finding(d, before);
    }

    // ── СЦЕНАРИЙ 2+3: компакция, РАЗНЕСЁННАЯ НА ДВА ТАКТА (Б-4). ─────────────────────────
    // Шаг 6 (`segments.rs:3998-4011`): rename .tmp→.zst сделан, remove сырого ЕЩЁ НЕТ —
    // на диске ОБА файла. Шаг 7: сырой исчезает. Прод разносит их микросекундами, но тик
    // сессии — 250 мс, и попадание между ними неизбежно при 10k сессий; self-heal (`:3956`)
    // разносит те же два события на ЧАСЫ.
    {
        let victim = pick_closed_raw(d, &used, anchor);
        used.push(victim.index);
        let vname = file_name_of(&victim);
        let visible = victim_events_visible(d, anchor, &victim);
        assert!(
            visible > 0,
            "SETUP НЕ СОСТОЯЛСЯ: у жертвы {vname} нет НИ ОДНОГО события после курсора — \
             сверка выдачи в этом сценарии не проверяла бы ничего (при курсоре у хвоста \
             расхождение = 0 даже на испорченном кеше)"
        );

        // Шаг 6: пара на диске (как её оставляет `compact_segment` между :4003 и :4011).
        let raw_bytes = fs::read(&victim.path).expect("read raw");
        journal::compact_segment(&victim, journal::DEFAULT_COMPACT_LEVEL).expect("compact");
        fs::write(&victim.path, &raw_bytes).expect("вернуть сырой рядом с .zst");
        let zst_name = format!("{vname}.zst");
        assert!(
            path_of(d, &vname).exists() && path_of(d, &zst_name).exists(),
            "SETUP НЕ СОСТОЯЛСЯ: промежуточное состояние компакции не воспроизведено — на \
             диске нет ОБОИХ файлов индекса {}",
            victim.index
        );
        let before = t.findings.len();
        t.observe(d, anchor, "compact-step6", Budget::Free);
        t.resync_after_finding(d, before);

        // Шаг 7: сырой удалён. ЭТО ТОТ САМЫЙ ТАКТ Б-4 — удаление ПО ИНДЕКСУ сносит запись,
        // хотя у индекса на диске остался `.zst`; `is_fresh` при этом переписывает
        // `file_names`, и расхождение становится ненаблюдаемым изнутри навсегда.
        let before = t.findings.len();
        fs::remove_file(path_of(d, &vname)).expect("шаг 7: удалить сырой");
        assert!(
            !path_of(d, &vname).exists() && path_of(d, &zst_name).exists(),
            "SETUP НЕ СОСТОЯЛСЯ: после шага 7 обязан остаться РОВНО .zst"
        );
        t.observe(d, anchor, "compact-step7", Budget::Free);
        // Липучесть: холостой такт после порчи. `cur_names == file_names` ⇒ ранний выход,
        // расхождения никто не увидит — порча живёт до конца сессии.
        t.observe(d, anchor, "idle", Budget::Steady);
        // И ротация ПОСЛЕ порчи: замер разведки (§Р-1) опроверг «самолечение ротацией» —
        // +1 файл это `small_change`, то есть ТА ЖЕ инкрементальная ветка, а не `refresh()`.
        grow_segments(d, &mut n, 1);
        t.observe(d, anchor, "rotate-after-damage", Budget::Free);
        t.resync_after_finding(d, before);
    }

    // ── СЦЕНАРИЙ 4 (КОНТРОЛЬ, обязан быть ЗЕЛЁНЫМ): та же компакция ОДНИМ тактом. ────────
    // Форма `sm8`. Разница с Б-4 только в композиции ИСТОРИИ — это и есть ось 6. Если
    // красное появляется и здесь, дефект не в разнесении, а в компакции как таковой.
    {
        let victim = pick_closed_raw(d, &used, anchor);
        used.push(victim.index);
        let vname = file_name_of(&victim);
        assert!(
            victim_events_visible(d, anchor, &victim) > 0,
            "SETUP НЕ СОСТОЯЛСЯ: события жертвы {vname} не видны из-под курсора"
        );
        let before = t.findings.len();
        journal::compact_segment(&victim, journal::DEFAULT_COMPACT_LEVEL).expect("compact");
        assert!(
            !path_of(d, &vname).exists() && path_of(d, &format!("{vname}.zst")).exists(),
            "SETUP НЕ СОСТОЯЛСЯ: однотактовая компакция не завершилась (сырой на месте либо \
             .zst не создан)"
        );
        t.observe(d, anchor, "compact-one-tact", Budget::Free);
        t.observe(d, anchor, "idle", Budget::Steady);
        t.resync_after_finding(d, before);
    }

    // ── СЦЕНАРИЙ 5: self-heal через k ХОЛОСТЫХ ТАКТОВ (`segments.rs:3956-3957`). ─────────
    // Пара живёт часами (прерванная компакция, возврат сырого из бэкапа — комментарий
    // `:320`), потом cron 03:50 удаляет РОВНО ОРИГИНАЛ. Между появлением пары и уборкой —
    // сколько угодно тактов: значение оси 6, которого нет ни у одного сегодняшнего оракула.
    {
        let victim = pick_closed_raw(d, &used, anchor);
        used.push(victim.index);
        let vname = file_name_of(&victim);
        assert!(
            victim_events_visible(d, anchor, &victim) > 0,
            "SETUP НЕ СОСТОЯЛСЯ: события жертвы {vname} не видны из-под курсора"
        );
        let raw_bytes = fs::read(&victim.path).expect("read raw");
        journal::compact_segment(&victim, journal::DEFAULT_COMPACT_LEVEL).expect("compact");
        fs::write(&victim.path, &raw_bytes).expect("вернуть сырой");
        let before = t.findings.len();
        t.observe(d, anchor, "compact-step6", Budget::Free);
        for _ in 0..3 {
            t.observe(d, anchor, "idle", Budget::Steady);
        }
        fs::remove_file(path_of(d, &vname)).expect("self-heal: удалить оригинал");
        assert!(
            !path_of(d, &vname).exists() && path_of(d, &format!("{vname}.zst")).exists(),
            "SETUP НЕ СОСТОЯЛСЯ: self-heal обязан снести РОВНО сырой"
        );
        t.observe(d, anchor, "selfheal-remove-raw", Budget::Free);
        t.observe(d, anchor, "idle", Budget::Steady);
        t.resync_after_finding(d, before);
    }

    // ── СЦЕНАРИЙ 6 (ЗЕРКАЛО): уборка `.zst` при ЖИВОМ сыром (`segments.rs:3947`). ────────
    // Ветка расхождения sha сносит ТОЛЬКО `.zst`, оставляя сырой ГОРЯЧИМ, — а кеш держал
    // именно сырой (D-COMP-1). Асимметрия по `testing.md`: обновляется ОДНА сторона пары,
    // и симметричная фикстура этот дефект прячет. Корень один: удаление по ИНДЕКСУ не
    // спрашивает, кто из файлов индекса выжил.
    {
        let victim = pick_closed_raw(d, &used, anchor);
        used.push(victim.index);
        let vname = file_name_of(&victim);
        let zst_name = format!("{vname}.zst");
        assert!(
            victim_events_visible(d, anchor, &victim) > 0,
            "SETUP НЕ СОСТОЯЛСЯ: события жертвы {vname} не видны из-под курсора"
        );
        let raw_bytes = fs::read(&victim.path).expect("read raw");
        journal::compact_segment(&victim, journal::DEFAULT_COMPACT_LEVEL).expect("compact");
        fs::write(&victim.path, &raw_bytes).expect("вернуть сырой");
        let before = t.findings.len();
        t.observe(d, anchor, "compact-step6", Budget::Free);
        fs::remove_file(path_of(d, &zst_name)).expect("удалить .zst при живом сыром");
        assert!(
            path_of(d, &vname).exists() && !path_of(d, &zst_name).exists(),
            "SETUP НЕ СОСТОЯЛСЯ: обязан остаться РОВНО сырой"
        );
        t.observe(d, anchor, "remove-zst-alive-raw", Budget::Free);
        t.observe(d, anchor, "idle", Budget::Steady);
        t.resync_after_finding(d, before);
    }

    // ── СЦЕНАРИЙ 7 (ЛЕГИТИМНЫЙ): посторонние файлы, которые пишет САМ проект. ────────────
    // `journal.meta.tmp` — каждые 64 события recorder'а; `segment-*.jrnl.zst.tmp` — минуты
    // при компакции. Появление и ИСЧЕЗНОВЕНИЕ такого файла не смеет менять ни вердикт, ни
    // состав: «отсутствие» из чек-листа дегенерированного входа.
    {
        let before = t.findings.len();
        for foreign in ["journal.meta.tmp", "segment-00000002.jrnl.zst.tmp"] {
            fs::write(path_of(d, foreign), b"x").expect("создать посторонний файл");
        }
        t.observe(d, anchor, "foreign-tmp", Budget::Free);
        for foreign in ["journal.meta.tmp", "segment-00000002.jrnl.zst.tmp"] {
            fs::remove_file(path_of(d, foreign)).expect("убрать посторонний файл");
        }
        t.observe(d, anchor, "foreign-tmp", Budget::Free);
        t.observe(d, anchor, "idle", Budget::Steady);
        t.resync_after_finding(d, before);
    }

    // ── СЦЕНАРИЙ 8: ДВА файловых события в ОДИН такт (множественность). ─────────────────
    // Ротация И компакция другого сегмента между двумя тиками: `added=2`, `removed=1` —
    // всё ещё `small_change`, то есть инкрементальная ветка, но диф уже не «одно имя».
    {
        let victim = pick_closed_raw(d, &used, anchor);
        used.push(victim.index);
        let vname = file_name_of(&victim);
        assert!(
            victim_events_visible(d, anchor, &victim) > 0,
            "SETUP НЕ СОСТОЯЛСЯ: события жертвы {vname} не видны из-под курсора"
        );
        let files_before = n_files(d);
        let before = t.findings.len();
        grow_segments(d, &mut n, 1);
        journal::compact_segment(&victim, journal::DEFAULT_COMPACT_LEVEL).expect("compact");
        assert_eq!(
            n_files(d),
            files_before + 1,
            "SETUP НЕ СОСТОЯЛСЯ: за такт ожидались +1 (ротация) и ±0 (компакция: -1 сырой, \
             +1 сжатый) — итого +1 файл"
        );
        t.observe(d, anchor, "two-in-one-tact", Budget::Free);
        t.observe(d, anchor, "idle", Budget::Steady);
        t.resync_after_finding(d, before);
    }

    // ── СЦЕНАРИЙ 9: ГРАНИЦА `small_change` — ТРИ файла за такт (`segments.rs:278`). ──────
    // `added.len() <= 2 && removed.len() <= 2` — та самая граница, где обе развязки меняют
    // поведение; оракула на ней не было. Три файла ⇒ полный `refresh()` ⇒ состав обязан
    // быть верен, и это ЛЕГИТИМНЫЙ дорогой такт (бюджет здесь не судится).
    {
        let files_before = n_files(d);
        let before = t.findings.len();
        grow_segments(d, &mut n, 3);
        assert!(
            n_files(d) >= files_before + 3,
            "SETUP НЕ СОСТОЯЛСЯ: за такт добавилось {} файлов, а граница small_change \
             проверяется ТРЕМЯ",
            n_files(d) - files_before
        );
        t.observe(d, anchor, "three-files-tact", Budget::Free);
        t.observe(d, anchor, "idle", Budget::Steady);
        t.resync_after_finding(d, before);
    }

    assert_manifest_executed(&t.executed);

    if !t.findings.is_empty() {
        let body = t
            .findings
            .iter()
            .enumerate()
            .map(|(i, f)| format!("{:>2}. {f}", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "SM-11 (класс A): тёплый каталог НЕ эквивалентен холодному наблюдению — {} \
             расхождений на последовательности из {} тактов.\n\n{body}\n\n\
             Инвариант I2: после ЛЮБОЙ последовательности событий каталога тёплый кеш даёт \
             тот же вердикт, тот же состав (индексы И ПУТИ И first_seq) и ту же выдачу, что \
             холодное наблюдение. Ни одна из сверок НЕ требует конкретной ветки исполнения: \
             `refresh()`, переклассификация выжившего или точечная правка записи — любая \
             развязка, дающая эквивалентность в пределах бюджета, зелена.",
            t.findings.len(),
            t.executed.len(),
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════════
// SM-11c — КЛАСС B: повторено ПОСТРОЕНИЕ перечня, не унаследованы ПРОВЕРКИ полного пути
// ═════════════════════════════════════════════════════════════════════════════════════════

/// `segments_counted` (`:1056-1088`) = manifest → dedup → classify → sort → **guard**.
/// Инкрементальная ветка воспроизводит первые четыре шага и НИ ОДНОГО проверочного:
/// `check_first_seq_monotonic` (`JR-I-11`, M-52, заведён по факту прода `TD-030`) зовётся
/// только из `segments_counted`, то есть из `open()`/`refresh()` — и не зовётся ни разу за
/// всю жизнь установившейся сессии.
///
/// ЧТО ЗДЕСЬ ПРОВЕРЯЕТСЯ И ЧТО НЕТ. Проверяется ВЕРДИКТ, а не механизм: тёплая сессия не
/// имеет права ПРИНИМАТЬ каталог, который холодный полный путь ОТВЕРГАЕТ. Оракул не знает
/// имени guard'а и не требует конкретного места вызова — он наследует все проверки полного
/// пути разом, включая будущие.
///
/// ПОЧЕМУ ЭТО ЗАКРЫВАЕМО ДЁШЕВО (и потому — жёсткий ассерт): нарушение приходит С НОВЫМ
/// ИМЕНЕМ, а `check_first_seq_monotonic` работает над уже готовым перечнем
/// `(имя, schema_version, first_seq)` и делает НОЛЬ syscall'ов на здоровом каталоге
/// (`carries_events` зовётся лениво, только при равенстве). Бюджет `BUDGET_META` он не
/// двигает — замер §5 разведки.
///
/// АНТИ-ПЛАЦЕБО: setup-guard требует, чтобы ХОЛОДНЫЙ путь действительно вернул `Err` —
/// иначе сценарий не состоялся и проба молча сертифицировала бы здоровый каталог. Guard
/// проверяет СОСТОЯНИЕ ДИСКА и вердикт эталона, а не ветку исполнения предмета.
#[test]
fn sm11c_warm_verdict_inherits_fail_closed_guard_of_cold_path() {
    let (dir, _n) = build_prod_form(N_SEGMENTS);
    let d = dir.path();
    let cold0 = journal::list_segments(d).expect("list_segments");
    let anchor = cold0[cold0.len() - 10].header.first_seq - 1;

    let mut t = Tick::new();
    t.observe(d, anchor, "t0-first", Budget::First);
    t.observe(d, anchor, "idle", Budget::Steady);

    // Инъекция ровно по рунбуку-НАРУШЕНИЮ (`docs/ops-journal-tail-unreadable.md` §4 её
    // запрещает — значит она случается): архивный сегмент СТАРОГО seq-пространства
    // приложен под СВОБОДНЫМ старшим индексом. Один файл, одно событие каталога, `added=1`
    // ⇒ `small_change` ⇒ инкрементальная ветка. Ротация выглядит для кеша ТАК ЖЕ.
    let max_idx = cold0
        .iter()
        .map(|s| s.index)
        .max()
        .expect("непустой каталог");
    let donor = cold0
        .iter()
        .find(|s| {
            s.header.first_seq < cold0[cold0.len() - 1].header.first_seq
                && file_name_of(s).ends_with(".jrnl.zst")
        })
        .cloned()
        .expect("SETUP: нужен сжатый сегмент-донор с МАЛЫМ first_seq");
    let inj = d.join(format!("segment-{:08}.jrnl.zst", max_idx + 1));
    fs::copy(&donor.path, &inj).expect("инъекция архивного сегмента");

    let cold_after = journal::list_segments(d);
    assert!(
        cold_after.is_err(),
        "SETUP НЕ СОСТОЯЛСЯ: холодный путь ПРИНЯЛ каталог с немонотонным first_seq \
         (донор idx={} first_seq={} лёг под индексом {}). Без отказа эталона сверка \
         вердиктов сравнивала бы Ok с Ok и не проверяла бы ничего.",
        donor.index,
        donor.header.first_seq,
        max_idx + 1
    );
    eprintln!(
        "  ХОЛОДНЫЙ путь отверг каталог: {}",
        cold_after.err().map(|e| e.to_string()).unwrap_or_default()
    );

    t.observe(d, anchor, "inject-foreign-space", Budget::Free);
    // Липучесть: `file_names` переписан ⇒ расхождения не увидит НИКТО и НИКОГДА до конца
    // сессии. Замер разведки: 8/8 тиков fail-open, `meta_ops=7` (кеш-хит) — окно не
    // закрывается ни ротацией, ни временем, только перезапуском сессии.
    t.observe(d, anchor, "idle", Budget::Steady);

    assert!(
        t.findings.is_empty(),
        "SM-11c — КЛАСС B («повторено ПОСТРОЕНИЕ перечня, не унаследованы ПРОВЕРКИ»). \
         ЧИТАТЬ ПЕРВЫМ, ЕСЛИ ВЫ ПОЧИНИЛИ КЛАСС A: развязки класса A (переклассификация \
         выжившего, уход в `refresh()` при коллизии) этого теста НЕ КАСАЮТСЯ — он останется \
         красным, пока инкрементальная ветка не наследует проверки полного пути. \
         `1 passed; 1 failed` в этом файле означает «класс A закрыт, класс B нет», а не \
         «фикс не прошёл гейт».\n\nРасхождений: {}.\n{}\n\nГейт `JR-I-11` объявлен fail-closed для \
         ТРЁХ путей чтения, но после M-62 на прод-пути gateway он не исполняется ни разу за \
         жизнь сессии: `is_fresh` строит перечень своим мини-обходом и guard не наследует. \
         Кокпит остаётся зелёным на каталоге, который research/backtest/CLI уже отвергают, — \
         ровно тот сигнал, на который рунбук опирается как на «инцидент не закрыт», исчезает \
         в момент, когда оператор чинит журнал руками. Валидация обязана стоять ОДНИМ \
         именованным вызовом перед коммитом `self.file_names = cur_names` — чтобы её можно \
         было и мутировать одной строкой (мутант `noguard`), и удерживать в бюджете.",
        t.findings.len(),
        t.findings.join("\n")
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════════
// SM-11d — G1: изменение СОДЕРЖИМОГО при НЕИЗМЕННОМ имени
// ═════════════════════════════════════════════════════════════════════════════════════════

/// **ЭТОТ ОРАКУЛ — ТОЧКА РЕШЕНИЯ, А НЕ ТРЕБОВАНИЕ К DEV'У. Читать до того, как чинить.**
///
/// `is_fresh` наблюдает РОВНО две величины: множество имён файлов (`scan_dir_layout`
/// `:521-557`) и размер ОДНОГО файла — `latest_path` (`:241-258`). О содержимом ЗАКРЫТЫХ
/// сегментов не наблюдается ничего. Значит изменение содержимого под тем же именем —
/// усечение закрытого сегмента, подмена файла (ровно то, от чего заведён
/// `fingerprint_limited`, `:1088`), битый v2-заголовок — тёплой сессией НЕ НАБЛЮДАЕМО
/// СТРУКТУРНО, а холодным путём ловится и даёт `Err`.
///
/// **Дешёвого фикса здесь НЕТ, и это замер, а не мнение:** отличить подменённое содержимое
/// можно только `stat`/чтением ПО КАЖДОМУ сегменту, то есть `O(N)` на такте — ровно та
/// цена, которую M-62 и снимает (`BUDGET_META=8` при `N=200`). Поэтому набор НЕ требует
/// от dev'а закрыть G1 кодом. Он предъявляет ЧИСЛО и передаёт решение architect'у:
///
/// 1. **Заплатить** — валидировать содержимое реже, чем каждый тик, но чаще, чем никогда
///    (например, по таймеру/по счётчику тактов), и назвать период в спеке; либо
/// 2. **Назвать понижение явно** — §5 «Запрещено/Почему» спеки M-62 обязана получить
///    строку «частота переоценки fail-closed проверок содержимого понижена с КАЖДОГО тика
///    до момента появления ИМЕНИ», плюс запись `TECH-DEBT` severity MAJOR по образцу
///    «built-not-wired» (`gates.md` §4). Молча наследовать понижение нельзя: до M-62
///    `stream_from_at` звал `segments(dir)` каждый тик и наследовал ВСЕ проверки — это
///    записано в самом `TD-120`.
///
/// Пока решения нет, тест помечен `#[ignore]` с явной причиной: держать в обязательном
/// гейте требование, которое НЕЛЬЗЯ выполнить в бюджете, значит превратить гейт в
/// неисполнимый — а молча выбросить сценарий значит спрятать единственную дыру, которую
/// не нашла ни одна линза. Запускается явно:
/// `cargo test -p gateway --test red_catalog_equivalence -- --ignored`.
#[test]
#[ignore = "G1: точка решения architect'а (заплатить за переоценку содержимого либо назвать понижение в §5 + TD). В бюджете BUDGET_META не закрывается — см. док-комментарий"]
fn sm11d_content_rewritten_under_unchanged_name_is_invisible_to_warm_session() {
    for scenario in ["truncate-closed", "substitute-content"] {
        let (dir, _n) = build_prod_form(N_SEGMENTS);
        let d = dir.path();
        let cold0 = journal::list_segments(d).expect("list_segments");
        let anchor = cold0[cold0.len() - 10].header.first_seq - 1;

        let mut t = Tick::new();
        t.observe(d, anchor, "t0-first", Budget::First);
        t.observe(d, anchor, "idle", Budget::Steady);

        let victim = cold0[cold0.len() - 20].clone();
        let donor = cold0[cold0.len() - 3].clone();
        let names_before = n_files(d);
        let save = fs::read(&victim.path).expect("read victim");
        match scenario {
            // Шаг 3 рунбука `ops-journal-tail-unreadable.md`: оператор усекает/чинит файл
            // НА МЕСТЕ. Имя то же.
            "truncate-closed" => fs::write(&victim.path, &save[..8]).expect("усечь"),
            // Подмена файла под знакомым именем — единственный смысл `fingerprint_limited`.
            _ => {
                let bytes = fs::read(&donor.path).expect("read donor");
                fs::write(&victim.path, &bytes).expect("подменить содержимое")
            }
        }
        assert_eq!(
            n_files(d),
            names_before,
            "SETUP НЕ СОСТОЯЛСЯ: множество ИМЁН изменилось — сценарий G1 требует, чтобы \
             изменилось только СОДЕРЖИМОЕ"
        );
        assert!(
            journal::list_segments(d).is_err(),
            "SETUP НЕ СОСТОЯЛСЯ ({scenario}): холодный путь ПРИНЯЛ подменённое содержимое — \
             сравнивать было бы нечего"
        );

        t.observe(d, anchor, "content-rewrite", Budget::Steady);
        assert!(
            t.findings.is_empty(),
            "SM-11d ({scenario}): {}\n\nЭто НЕ баг реализации задач 1-3, а НАЗВАННОЕ \
             следствие конструкции: наблюдаются имена, а меняется содержимое. Решение — \
             architect'а (см. док-комментарий теста), не dev'а.",
            t.findings.join("\n")
        );
    }
}
