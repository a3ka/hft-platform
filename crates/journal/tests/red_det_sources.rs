//! RED M-51 — **DET-I-3**: источники недетерминизма в доменном коде (sacred, architect-only).
//!
//! ## Что пиннится
//!
//! `CLAUDE.md` («Операционные принципы», BINDING) запрещает буквально: «в доменном коде —
//! никакого недетерминизма (нет wall-clock/`rand()`/**итерации по HashMap без сортировки в
//! редьюсерах**)». Запрет существовал с первого дня и НИ РАЗУ не был исполнимым: аудит
//! `research/measurements/td-007-determinism-coverage.md` §3.1 нашёл его нарушение
//! (`crates/sim/src/exchange.rs:240`), прожившее в проде всё время существования крейта.
//!
//! Правило, у которого нет канарейки, — не правило, а пожелание. Здесь оно становится гейтом.
//!
//! ## Механика и оговорка про её честность
//!
//! Канарейка **текстовая**, не типовая: она находит идентификаторы, объявленные как
//! `HashMap<`/`HashSet<`, и ищет по ним конструкции ОБХОДА (`.iter()`, `.values()`, `.keys()`,
//! `.drain()`, `for .. in ..`). Точечный доступ (`get`/`entry`/`insert`/`remove`/
//! `contains_key`) недетерминизма не создаёт и не флагуется — именно поэтому `Books.map`,
//! `BacktestExchange.books` и `ReconBooks` молчат, а `BacktestExchange.active` — нет.
//!
//! Замер калибровки (2026-08-01, весь workspace): **ровно 2 совпадения** — реальный дефект
//! (`sim/exchange.rs:240`) и легитимный обход с немедленной сортировкой
//! (`research-cli/src/export_io.rs:283`, `RC-I-5`). Канарейка не шумит.
//!
//! **Оговорка (названа, не спрятана):** текстовый анализ не видит алиасы (`let m = &self.map;
//! m.iter()`), типы через `type`-алиасы и обходы через промежуточные функции. Канарейка — не
//! доказательство отсутствия, а СТОП-СИГНАЛ на самом частом способе внести дефект. Поведенческие
//! оракулы (`crates/sim/tests/red_det_fill_order.rs`, `crates/book/tests/red_det_projection.rs`)
//! — основная линия защиты; эта — вторая.
//!
//! ## Waiver
//!
//! Осознанный обход разрешён маркером `// DET-OK: <причина>` на той же или предыдущей строке.
//! Причина ОБЯЗАТЕЛЬНА и непуста: «разрешено молча» и «разрешено с названной причиной» — разные
//! режимы, и аудит-трейл держится на втором.

use std::fs;
use std::path::{Path, PathBuf};

mod common;

use common::cfg_with;
use journal::{Journal, RetentionPolicy};

/// Доменные крейты — редьюсеры над потоком журнала и производные от них. Список ЯВНЫЙ:
/// новый крейт обязан быть внесён сюда осознанно (иначе он молча выпадает из-под правила).
const DOMAIN_CRATES: &[&str] = &[
    "journal",
    "book",
    "sim",
    "strategy",
    "portfolio",
    "alpha",
    "signals",
    "gateway",
    "derive",
    "recorder",
    "research-cli",
    "contracts",
];

const WAIVER: &str = "DET-OK:";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort(); // сама канарейка обязана быть детерминированной
        for p in entries {
            if p.is_dir() {
                rs_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
}

fn domain_sources() -> Vec<(PathBuf, String)> {
    let root = workspace_root();
    let mut out = Vec::new();
    for c in DOMAIN_CRATES {
        let mut files = Vec::new();
        rs_files(&root.join("crates").join(c).join("src"), &mut files);
        assert!(
            !files.is_empty(),
            "канарейка не нашла ни одного .rs в crates/{c}/src — список DOMAIN_CRATES \
             разошёлся с деревом (крейт переименован/удалён). Правь канарейку, а не игнорируй"
        );
        for f in files {
            let text = fs::read_to_string(&f).expect("read source");
            out.push((f, text));
        }
    }
    out
}

/// Есть ли waiver на строке `idx` (0-based) или на предыдущей — и не пуст ли он.
fn waived(lines: &[&str], idx: usize) -> bool {
    let has = |l: &str| -> bool {
        match l.find(WAIVER) {
            Some(p) => !l[p + WAIVER.len()..].trim().is_empty(),
            None => false,
        }
    };
    has(lines[idx]) || (idx > 0 && has(lines[idx - 1]))
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

/// Имена, объявленные с типом `HashMap<`/`HashSet<` (поле структуры, `let`, параметр).
fn hash_container_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let Some(pos) = line.find("HashMap<").or_else(|| line.find("HashSet<")) else {
            continue;
        };
        // Идентификатор перед ближайшим `:` слева от типа.
        let Some(colon) = line[..pos].rfind(':') else {
            continue;
        };
        // `std::collections::HashMap` — двоеточия пути, не объявление.
        if line[..colon].ends_with(':') {
            continue;
        }
        let ident: String = line[..colon]
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<char>>()
            .into_iter()
            .rev()
            .collect();
        if !ident.is_empty()
            && ident
                .chars()
                .next()
                .is_some_and(|c| c.is_lowercase() || c == '_')
        {
            names.push(ident);
        }
    }
    names.sort();
    names.dedup();
    names
}

const ITER_METHODS: &[&str] = &[
    ".iter()",
    ".iter_mut()",
    ".values()",
    ".values_mut()",
    ".keys()",
    ".drain()",
    ".into_iter()",
];

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_22 — обход хэш-контейнера в доменном коде запрещён без названной причины.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_22_no_unordered_hash_iteration_in_domain_code() {
    let root = workspace_root();
    let mut hits: Vec<String> = Vec::new();

    for (path, text) in domain_sources() {
        let names = hash_container_names(&text);
        if names.is_empty() {
            continue;
        }
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or(line);
            for n in &names {
                let iterates = ITER_METHODS.iter().any(|m| {
                    code.contains(&format!("self.{n}{m}")) || code.contains(&format!("{n}{m}"))
                }) || code.contains(&format!(" in self.{n} "))
                    || code.contains(&format!(" in &self.{n}"))
                    || code.contains(&format!(" in {n} "))
                    || code.contains(&format!(" in &{n}"));
                if iterates && !waived(&lines, i) {
                    hits.push(format!(
                        "{}:{}  [{}]  {}",
                        rel(&root, &path),
                        i + 1,
                        n,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "DET-I-3: обход хэш-контейнера в доменном коде без объявленной причины — прямое \
         нарушение запрета CLAUDE.md («итерации по HashMap без сортировки в редьюсерах»). \
         Порядок обхода задаётся хэш-сидом процесса, то есть реплей журнала дважды даёт \
         разный результат.\n\nЛибо перейти на упорядоченную структуру (`BTreeMap`) / \
         отсортировать перед использованием, либо — если порядок доказуемо не влияет на вывод \
         — поставить `// {WAIVER} <причина>` на той же или предыдущей строке.\n\nНайдено \
         {} шт.:\n  {}",
        hits.len(),
        hits.join("\n  ")
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_23 — обход каталога: порядок ФС не имеет права протечь в результат.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_23_directory_iteration_is_declared() {
    // `fs::read_dir` не даёт НИКАКИХ гарантий порядка — ни между процессами, ни между
    // файловыми системами. `dedup_indexed_paths` это уже нормализует (`BTreeMap` по индексу)
    // и САМА объявляет обязанность: «Публичные пути ОБЯЗАНЫ использовать ЭТОТ хелпер».
    // Обязанность, записанная только в комментарии, гейтом не является — вот гейт.
    let root = workspace_root();
    let mut hits: Vec<String> = Vec::new();
    for (path, text) in domain_sources() {
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or(line);
            if code.contains("read_dir(") && !waived(&lines, i) {
                hits.push(format!("{}:{}  {}", rel(&root, &path), i + 1, line.trim()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "DET-I-3: `read_dir` в доменном коде без объявленной причины. Порядок обхода каталога \
         не гарантирован; если он протекает в результат (список сегментов, отчёт, порядок \
         применения) — реплей недетерминирован между машинами и процессами. Нормализуй \
         порядок явной сортировкой и поставь `// {WAIVER} <как нормализован порядок>`.\n\n\
         Найдено {} шт.:\n  {}",
        hits.len(),
        hits.join("\n  ")
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_24 — нулевая база: `rand`/`rayon` в доменном коде не появляются.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_24_no_rand_or_parallel_iteration_in_domain_code() {
    // ЧЕСТНО: этот тест ЗЕЛЁНЫЙ с первого запуска (замер: 0 совпадений во всём workspace) —
    // он не вскрывает дефект, а фиксирует нулевую базу. Его работа — поймать БУДУЩУЮ
    // деградацию (класс канареек `structural.rs`). Недетерминизм порядка `rayon` и
    // `rand::thread_rng` неисправим постфактум: он уже уехал в журнал/отчёт.
    // `sim` использует СОБСТВЕННЫЙ `SplitMix64` с обязательным seed (SM-I-2) — не `rand`.
    let root = workspace_root();
    let mut hits: Vec<String> = Vec::new();
    for (path, text) in domain_sources() {
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or(line);
            let bad = code.contains("rand::")
                || code.contains("thread_rng")
                || code.contains("rayon")
                || code.contains(".par_iter");
            if bad && !waived(&lines, i) {
                hits.push(format!("{}:{}  {}", rel(&root, &path), i + 1, line.trim()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "DET-I-3: `rand`/`rayon`/`par_iter` в доменном коде ({} шт.). Недетерминизм этого \
         класса уезжает в журнал и отчёт необратимо:\n  {}",
        hits.len(),
        hits.join("\n  ")
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_25 — поведенческий: операторский отчёт ретеншена воспроизводим.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_25_retention_report_order_is_deterministic() {
    // Найдено architect'ом при проверке §6 п.8 аудита (пробел, который аудит честно не закрыл).
    //
    // `enumerate_retention_segments` синтезирует для файла с НЕРАСПОЗНАННЫМ именем
    // `SegmentInfo { index: u32::MAX, .. }` (`segments.rs:2380`), а затем сортирует
    // `foreign_skipped.sort_by_key(|(s, _)| s.index)` — с комментарием «Стабильная сортировка
    // по индексу — критична для воспроизводимости плана (R6)».
    //
    // Для этой группы сортировка НЕ УПОРЯДОЧИВАЕТ НИЧЕГО: у всех ключ `u32::MAX`, а
    // `sort_by_key` стабилен ⇒ сохраняется исходный порядок `read_dir`, то есть порядок ФС.
    // Оператор, запустивший отчёт дважды, видит разный порядок строк; воспроизводимость,
    // которую комментарий объявляет, не выполняется. Severity MINOR (отчёт, не журнал), но
    // класс — ровно DET-I-1, и код сам утверждает обратное.
    //
    // Контракт: записи с нераспознанным именем упорядочены ПО ПУТИ (тотальный порядок,
    // не зависящий от ФС).
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j =
            Journal::open_with(dir.path(), cfg_with(64 * 1024, "det-canary")).expect("open");
        j.append(common::trade(0)).expect("append");
        j.flush().expect("flush");
    }
    // 12 нераспознанных имён: вероятность, что порядок ФС СЛУЧАЙНО совпадёт с сортировкой
    // по пути, пренебрежимо мала (~1/12!) — зелёный тест означает явную сортировку.
    let names: Vec<String> = (0..12)
        .map(|i| format!("segment-{}-weird.jrnl", (b'a' + (i * 7 % 12) as u8) as char))
        .collect();
    for n in &names {
        fs::write(dir.path().join(n), b"not a real segment").expect("write");
    }

    let policy = RetentionPolicy {
        retain_days: 1,
        keep_min_segments: 1,
        cold_root: dir.path().join("cold"),
        min_free_bytes: 0,
        checkpoint_covered_through_seq: None,
        allow_prune_without_checkpoint: false,
    };
    let plan = journal::retention_plan(dir.path(), &policy, 1_785_000_000_000).expect("plan");

    let weird: Vec<String> = plan
        .skipped
        .iter()
        .map(|(s, _)| s.path.display().to_string())
        .filter(|p| p.ends_with("-weird.jrnl"))
        .collect();
    assert_eq!(
        weird.len(),
        names.len(),
        "фикстура: все {} нераспознанных файла обязаны попасть в отчёт (иначе порядок \
         проверять не на чем)",
        names.len()
    );

    let mut sorted = weird.clone();
    sorted.sort();
    assert_eq!(
        weird, sorted,
        "DET-I-3: операторский отчёт ретеншена перечисляет файлы с нераспознанным именем в \
         порядке `read_dir` (порядок ФС), а не в тотальном порядке по пути. У всех таких \
         записей `index == u32::MAX`, поэтому `sort_by_key(index)` их не упорядочивает, а \
         стабильность сортировки сохраняет порядок каталога — при том что код объявляет \
         «критично для воспроизводимости плана (R6)». Два запуска отчёта дают разный вывод"
    );
}
