//! SACRED (architect-only) — M-40 / риск **R2** (`docs/08` CRITICAL): **ретеншен обязан ВИДЕТЬ
//! сжатые сегменты.**
//!
//! ## Дефект (замерен на `origin/main` @ 30f5ab0, не гипотеза)
//!
//! `retention_plan()` перечисляет каталог СВОИМ `fs::read_dir` с фильтром
//! `p.extension() == Some("jrnl")`. Для `segment-00000042.jrnl.zst` расширение — `"zst"`,
//! поэтому сжатые сегменты выпадают из плана БЕЗУСЛОВНО. Замер (probe architect'а):
//!
//! ```text
//! до компакции : prune=[0,1,2,3]  skipped=[5 active, 4 keep_min]
//! после compact: prune=[3]        skipped=[5 active, 4 keep_min]   ← 0,1,2 исчезли ЦЕЛИКОМ
//! каталог из одних .zst: prune=[]                                   ← план ПУСТ
//! ```
//!
//! Сегменты 0,1,2 не попали НИ в `offload_and_prune`, НИ в `offload_only`, НИ в `skipped` —
//! это не «консервативный пропуск», а полная невидимость: оператор не узнаёт о них ниоткуда.
//!
//! ## Почему это CRITICAL, а не косметика
//!
//! На проде (2026-07-29) ~122 сегмента, подавляющее большинство — `.zst` (компакция по cron
//! в 03:50). R1 (offsite-бэкап журнала, `docs/08`) — единственный экзистенциальный риск, и
//! его первый реальный `--mode apply` запланирован founder'ом ~2026-08-10. Запуск «как есть»
//! означает: в холодное хранилище уедут ЕДИНИЦЫ свежих raw-сегментов, а вся сжатая ИСТОРИЯ
//! останется на NVMe в единственной копии — бэкап будет дырявым ровно там, где лежит то,
//! что невосполнимо. Дефект тихий: отчёт ретеншена отрапортует успех по тем файлам, которые
//! увидел, и ни одной строкой не упомянет остальные 120.
//!
//! ## Корень — ТРИ независимых энумератора сегментов в одном крейте
//!
//! `dedup_indexed_paths` (:595, знает `.zst`, дедуплицирует по индексу — D-COMP-1),
//! `retention_plan` (:1506, свой `read_dir`, слеп к `.zst`),
//! `latest_segment_index` (:1130, свой `read_dir`, слеп к `.zst` — см.
//! `red_restore_from_cold.rs`). Заплатка «добавить ещё одно условие в фильтр
//! `retention_plan`» сохраняет корень и породит ЧЕТВЁРТОЕ расхождение. Поэтому контракт
//! M-40 — не «починить фильтр», а **свести перечисление к ОДНОМУ хелперу**
//! (`dedup_indexed_paths`), как это уже сделано для `segments()`/`iter_segments_sorted()`
//! решением D-COMP-1.
//!
//! ## Контракт (architect)
//!
//! - **RT-Z-1** Компакция НЕ МЕНЯЕТ решения ретеншена. План над каталогом до компакции и
//!   после неё совпадает по индексам во ВСЕХ трёх корзинах (`offload_and_prune`,
//!   `offload_only`, `skipped`). Сжатие — производная операция хранения; она не имеет права
//!   влиять на то, что считается устаревшим.
//! - **RT-Z-2** Каталог, где сжаты ВСЕ закрытые сегменты (прод-раскладка VPS), планируется
//!   так же, как сырой.
//! - **RT-Z-3** Гейт покрытия чекпоинтом (C-030 R1, M-38b) работает на `.zst` ТОЧНО ТАК ЖЕ:
//!   покрытый сжатый сегмент прунится, непокрытый уходит в `offload_only` + skip-репорт.
//!   Граница проверяется ровно на `last_seq` и `last_seq − 1`.
//! - **RT-Z-4** `Apply` на сжатом сегменте физически освобождает диск и оставляет
//!   сверенную холодную копию.
//! - **RT-Z-5** `last_seq(сегмент)` (= `first_seq` следующего − 1) ИНВАРИАНТЕН к компакции.
//!   Это ответ на вопрос «как считается `last_seq`, когда `.zst` и raw перемешаны»:
//!   никакого отдельного правила для смеси НЕТ и быть не должно — требуется лишь, чтобы
//!   перечисление было ПОЛНЫМ. Дырявый список (сегодняшнее состояние) даёт ЗАВЫШЕННЫЙ
//!   `last_seq` соседа (считается по дальнему следующему), из-за чего покрытые сегменты
//!   молча признаются непокрытыми и место не освобождается НИКОГДА.
//! - **RT-Z-6** Файл сегмента, который не удалось классифицировать, обязан быть НАЗВАН в
//!   `skipped` со своим настоящим индексом — включая `.zst`. Молчание = оператор не знает.
//!
//! ## Дисциплина фикстуры (`.claude/rules/testing.md`)
//!
//! - **п.6 прод-РЕЖИМ значения.** Возраст сегмента считается по `ts_exch_ms` ПЕРВОГО СОБЫТИЯ
//!   (`first_event_data_ts`), а при неудаче чтения — по `header.created_wall_ms`. В проде
//!   эти величины близки (recorder пишет в реальном времени), поэтому фикстура с «удобным»
//!   `T0` замаскировала бы реализацию, которая для `.zst` молча уходит в fallback. Здесь
//!   `T0 = сейчас − 30 суток`: данные СТАРЫЕ, а `created_wall_ms` СВЕЖИЙ, и две семантики
//!   возраста дают ПРОТИВОПОЛОЖНЫЙ ответ. Поэтому оракул краснеет и на реализации, которая
//!   включила `.zst` в перечисление, но не научилась читать из них первое событие.
//!   Обращение к стенным часам здесь ОБЯЗАТЕЛЬНО и не является недетерминизмом: план строится
//!   от `now_wall_ms`, переданного аргументом, а `created_wall_ms` пишет сам `Journal` из
//!   `SystemTime::now()` — воспроизвести прод-режим иначе невозможно.
//! - **п.2 множественность.** В плане обязано оказаться ≥2 сжатых сегмента (реализация,
//!   считающая «один», не проходит).
//! - **п.4 границы.** Смесь raw+`.zst`; каталог из одних `.zst`; покрытие ровно на `last_seq`
//!   и на `last_seq − 1`; активный сегмент.
//! - **п.7 парный vantage.** Каждый запрет предъявлен вместе с разрешением: непокрытый .zst
//!   НЕ прунится / покрытый .zst прунится — иначе гвард вырождается в «не пруним никогда».

use contracts::{DataSource, EventKind, LegacySegmentDecl, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, RetentionMode, RetentionPlan, RetentionPolicy, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const N: u64 = 900;
/// Замер: N=900 при 8 KiB на сегмент → 6 сегментов (0..5), активный — 5.
const SEG_BYTES: u64 = 8 * 1024;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_millis() as i64
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "retention-compacted fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(t0: i64, i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + i as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: t0 + i as i64,
        },
    )
}

/// Журнал прод-режима: данные помечены `t0` (30 суток назад), `created_wall_ms` — сейчас.
fn build(dir: &std::path::Path, t0: i64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in 0..N {
        j.append(trade(t0, i)).expect("append");
    }
    j.flush().expect("flush");
}

fn policy(cold: &std::path::Path, covered: Option<u64>) -> RetentionPolicy {
    RetentionPolicy {
        retain_days: 1,
        keep_min_segments: 1,
        cold_root: cold.to_path_buf(),
        min_free_bytes: 0,
        checkpoint_covered_through_seq: covered,
        allow_prune_without_checkpoint: false,
    }
}

fn idx(v: &[journal::SegmentInfo]) -> Vec<u32> {
    let mut o: Vec<u32> = v.iter().map(|s| s.index).collect();
    o.sort_unstable();
    o
}

fn skipped_idx(plan: &RetentionPlan) -> Vec<u32> {
    let mut o: Vec<u32> = plan.skipped.iter().map(|(s, _)| s.index).collect();
    o.sort_unstable();
    o
}

/// Имена файлов каталога (для сообщений об ошибке — оператор должен видеть раскладку).
fn ls(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

fn zst_count(dir: &std::path::Path) -> usize {
    ls(dir).iter().filter(|n| n.ends_with(".zst")).count()
}

/// `last_seq(idx)` = `first_seq` следующего по индексу − 1; `None` для последнего.
fn last_seq_of(dir: &std::path::Path, index: u32) -> Option<u64> {
    let mut segs = journal::list_segments(dir).expect("list_segments");
    segs.sort_by_key(|s| s.index);
    let pos = segs.iter().position(|s| s.index == index)?;
    segs.get(pos + 1).map(|next| next.header.first_seq - 1)
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-1 — ГЛАВНЫЙ: компакция не меняет решения ретеншена (композиция стадий)
// ═════════════════════════════════════════════════════════════════════════════════════

/// Две независимо-зелёные стадии конвейера (компакция ✅ `red_compaction.rs`, ретеншен ✅
/// `red_retention_operator.rs`) дают сломанную КОМПОЗИЦИЮ — системный паттерн №1 из
/// `docs/08`. Оракул гоняет их последовательно, ровно как cron на VPS: 03:50 компакция,
/// 04:07 ретеншен.
#[test]
fn rt_z_1_compaction_does_not_change_the_retention_plan() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);

    let now = t0 + 10 * DAY_MS; // данные старше retain_days=1
    let pol = policy(cold.path(), Some(u64::MAX)); // покрытие не мешает: изолируем ОДИН фактор

    let before = journal::retention_plan(dir.path(), &pol, now).expect("plan before");
    // Setup-guard (свойство 3 «целостности гейта»): фикстура обязана ДОКАЗАТЬ, что до
    // компакции план непуст — иначе тест сравнивал бы пустоту с пустотой и был бы плацебо.
    assert!(
        idx(&before.offload_and_prune).len() >= 3,
        "фикстура не состоялась: до компакции в плане {} сегментов (нужно ≥3, чтобы после \
         компакции было что терять). Каталог: {:?}",
        idx(&before.offload_and_prune).len(),
        ls(dir.path())
    );

    // Компакция ровно как на проде: свежие остаются сырыми, старые сжимаются.
    let reports = journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    assert!(
        reports.len() >= 2 && zst_count(dir.path()) >= 2,
        "фикстура не состоялась: сжато {} сегментов, .zst в каталоге {} (нужно ≥2 — правило \
         МНОЖЕСТВЕННОСТИ, реализация «увидели один» не должна проходить). Каталог: {:?}",
        reports.len(),
        zst_count(dir.path()),
        ls(dir.path())
    );

    let after = journal::retention_plan(dir.path(), &pol, now).expect("plan after");

    assert_eq!(
        idx(&after.offload_and_prune),
        idx(&before.offload_and_prune),
        "R2 НАРУШЕН: компакция изменила план ретеншена.\n\
         ДОЛЖНО БЫТЬ (план до компакции): {:?}\n\
         ПОЛУЧЕНО (план после компакции): {:?}\n\
         Сжатые сегменты выпали из плана: они НЕ уедут в холодное хранилище и НЕ освободят \
         диск. На проде это ~120 из ~122 сегментов — R1 (offsite-бэкап) станет дырявым ровно \
         на исторической части. Каталог: {:?}",
        idx(&before.offload_and_prune),
        idx(&after.offload_and_prune),
        ls(dir.path())
    );
    assert_eq!(
        idx(&after.offload_only),
        idx(&before.offload_only),
        "компакция изменила состав offload_only: должно быть {:?}, получено {:?}",
        idx(&before.offload_only),
        idx(&after.offload_only)
    );
    assert_eq!(
        skipped_idx(&after),
        skipped_idx(&before),
        "компакция изменила состав skipped: должно быть {:?}, получено {:?}. Сегмент, \
         выпавший из ВСЕХ трёх корзин, для оператора не существует — он не увидит его \
         ни в плане, ни в причинах пропуска.",
        skipped_idx(&before),
        skipped_idx(&after)
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-2 — прод-раскладка VPS: сжаты ВСЕ закрытые сегменты
// ═════════════════════════════════════════════════════════════════════════════════════

/// Граница «только .zst» (`testing.md` п.4). Это НЕ синтетика: на VPS 2026-07-29 сырыми
/// остаются единицы свежих сегментов, остальные сжаты cron'ом.
#[test]
fn rt_z_2_all_closed_segments_compacted_still_plans() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);

    let now = t0 + 10 * DAY_MS;
    let pol = policy(cold.path(), Some(u64::MAX));
    let expected = idx(&journal::retention_plan(dir.path(), &pol, now)
        .expect("plan raw")
        .offload_and_prune);

    journal::compact_closed_segments(dir.path(), 0, 3).expect("compact all closed");
    let raw_left = ls(dir.path())
        .iter()
        .filter(|n| n.ends_with(".jrnl"))
        .count();
    assert!(
        zst_count(dir.path()) >= 3 && raw_left == 1,
        "фикстура не состоялась: ожидалась прод-раскладка «все закрытые сжаты, активный сырой», \
         получено .zst={} raw={}. Каталог: {:?}",
        zst_count(dir.path()),
        raw_left,
        ls(dir.path())
    );

    let plan = journal::retention_plan(dir.path(), &pol, now).expect("plan zst");
    assert_eq!(
        idx(&plan.offload_and_prune),
        expected,
        "R2 НАРУШЕН на прод-раскладке: каталог, где сжаты все закрытые сегменты, планируется \
         иначе, чем сырой.\n\
         ДОЛЖНО БЫТЬ: {:?}\nПОЛУЧЕНО:    {:?}\n\
         Пустой план здесь означает, что на боевом VPS первый реальный `--mode apply` не \
         выгрузит НИЧЕГО из истории. Каталог: {:?}",
        expected,
        idx(&plan.offload_and_prune),
        ls(dir.path())
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-3 — гейт покрытия чекпоинтом (C-030 R1) на сжатом сегменте: обе стороны + граница
// ═════════════════════════════════════════════════════════════════════════════════════

/// Композиция ДВУХ гейтов: «ретеншен видит .zst» (M-40) × «prune требует покрытия
/// чекпоинтом» (M-38b). Проверяется на ОДНОМ И ТОМ ЖЕ сжатом сегменте, ровно на границе
/// покрытия — `covered == last_seq` (прунится) и `covered == last_seq − 1` (не прунится).
/// Односторонняя проверка здесь недопустима: реализация «никогда не пруню .zst» прошла бы
/// половину, а реализация «пруню .zst не спрашивая покрытие» — другую.
#[test]
fn rt_z_3_checkpoint_coverage_gate_applies_to_compacted_segments() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");

    // Цель — СЖАТЫЙ сегмент (не первый: у первого нет предшественника, а нам нужен
    // обычный, ничем не примечательный сегмент в середине истории).
    let target: u32 = 2;
    let target_is_zst = ls(dir.path())
        .iter()
        .any(|n| n == &format!("segment-{target:08}.jrnl.zst"));
    assert!(
        target_is_zst,
        "фикстура не состоялась: сегмент {target} обязан быть СЖАТЫМ, иначе оракул проверяет \
         не тот путь. Каталог: {:?}",
        ls(dir.path())
    );
    let l = last_seq_of(dir.path(), target).expect("last_seq у сегмента в середине истории");
    let now = t0 + 10 * DAY_MS;

    // (а) Покрытие ровно на last_seq → сжатый сегмент ОБЯЗАН пруниться.
    let pol_covered = policy(cold.path(), Some(l));
    let plan = journal::retention_plan(dir.path(), &pol_covered, now).expect("plan covered");
    assert!(
        idx(&plan.offload_and_prune).contains(&target),
        "покрытый чекпоинтом СЖАТЫЙ сегмент {target} (last_seq={l}, covered={l}) обязан быть в \
         offload_and_prune.\nДОЛЖНО БЫТЬ: содержит {target}\nПОЛУЧЕНО: offload_and_prune={:?}, \
         offload_only={:?}, skipped={:?}\n\
         Если он отсутствует ВЕЗДЕ — ретеншен его не видит (R2). Если он в offload_only — \
         гейт покрытия ошибочно считает его непокрытым.",
        idx(&plan.offload_and_prune),
        idx(&plan.offload_only),
        skipped_idx(&plan)
    );

    // (б) Покрытие на единицу МЕНЬШЕ → тот же сегмент НЕ прунится, но бэкапится.
    let pol_uncovered = policy(cold.path(), Some(l - 1));
    let plan2 = journal::retention_plan(dir.path(), &pol_uncovered, now).expect("plan uncovered");
    assert!(
        !idx(&plan2.offload_and_prune).contains(&target),
        "C-030 R1 НАРУШЕН на сжатом сегменте: {target} (last_seq={l}) попал в prune при \
         covered={} — read-путь ещё не свернул его события, а retention удалил бы локальную \
         копию. offload_and_prune={:?}",
        l - 1,
        idx(&plan2.offload_and_prune)
    );
    assert!(
        idx(&plan2.offload_only).contains(&target),
        "непокрытый СЖАТЫЙ сегмент {target} обязан уйти в offload_only (бэкап R1 не \
         блокируется строгостью prune).\nДОЛЖНО БЫТЬ: offload_only содержит {target}\n\
         ПОЛУЧЕНО: offload_only={:?}, offload_and_prune={:?}, skipped={:?}",
        idx(&plan2.offload_only),
        idx(&plan2.offload_and_prune),
        skipped_idx(&plan2)
    );
    assert!(
        plan2
            .skipped
            .iter()
            .any(|(s, r)| s.index == target && r.contains("checkpoint")),
        "skip-репорт обязан НАЗЫВАТЬ причину «нет покрытия чекпоинтом» для сжатого сегмента \
         {target}: оператор должен понимать, почему место не освобождается. Причины: {:?}",
        plan2
            .skipped
            .iter()
            .map(|(s, r)| (s.index, r.clone()))
            .collect::<Vec<_>>()
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-4 — Apply на сжатых сегментах: диск реально освобождается, копия сверена
// ═════════════════════════════════════════════════════════════════════════════════════

/// План — это ещё не освобождённый диск (класс «код на main ≠ функция в проде», `docs/08`
/// системный паттерн №2). Оракул исполняет ВЫЗЫВАТЕЛЯ: `retention_execute(Apply)` — и
/// проверяет ФАЙЛОВУЮ СИСТЕМУ, а не поля отчёта.
#[test]
fn rt_z_4_apply_offloads_and_prunes_compacted_segments() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");

    let zst_before = zst_count(dir.path());
    assert!(
        zst_before >= 2,
        "фикстура не состоялась: нужно ≥2 сжатых сегмента (множественность), есть {zst_before}"
    );

    let now = t0 + 10 * DAY_MS;
    let pol = policy(cold.path(), Some(u64::MAX));
    let plan = journal::retention_plan(dir.path(), &pol, now).expect("plan");
    let report =
        journal::retention_execute(dir.path(), &plan, &pol, RetentionMode::Apply).expect("apply");

    let pruned_zst = report
        .pruned
        .iter()
        .filter(|p| p.to_string_lossy().ends_with(".zst"))
        .count();
    assert!(
        pruned_zst >= 2,
        "Apply обязан удалить ≥2 СЖАТЫХ сегмента (было {zst_before} .zst в каталоге).\n\
         ДОЛЖНО БЫТЬ: pruned содержит ≥2 путей на .zst\nПОЛУЧЕНО: pruned={:?}\n\
         Диск на проде не освободится: ~120 сжатых сегментов останутся лежать вечно.",
        report.pruned
    );
    assert!(
        report.failed.is_empty(),
        "сверка холодной копии сжатого сегмента провалилась: {:?}",
        report.failed
    );
    assert!(
        report.pruned_without_checkpoint_coverage.is_empty(),
        "при полном покрытии (covered=u64::MAX) ни один prune не должен числиться \
         «без покрытия»: {:?}",
        report.pruned_without_checkpoint_coverage
    );

    // Файловая система, а не отчёт.
    let cold_zst = ls(cold.path())
        .iter()
        .filter(|n| n.ends_with(".zst"))
        .count();
    assert!(
        cold_zst >= 2,
        "холодная копия сжатых сегментов не создана.\nДОЛЖНО БЫТЬ: ≥2 файлов .zst в cold\n\
         ПОЛУЧЕНО: {:?}\nЭто и есть R1-дыра: история не уехала в бэкап.",
        ls(cold.path())
    );
    assert!(
        zst_count(dir.path()) < zst_before,
        "горячая копия не уменьшилась: было {zst_before} .zst, стало {} — freed_bytes={} \
         рапортует освобождение, которого нет",
        zst_count(dir.path()),
        report.freed_bytes
    );
    assert!(
        report.freed_bytes > 0,
        "freed_bytes=0 при удалённых сегментах: отчёт оператора лжёт о результате"
    );

    // Остаток каталога обязан оставаться читаемым (prune не порвал последовательность).
    let n = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .filter(|e| e.is_ok())
        .count();
    assert!(
        n > 0,
        "после Apply журнал перестал читаться — prune повредил каталог"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-5 — last_seq инвариантен к компакции (ответ на вопрос «как считать на смеси»)
// ═════════════════════════════════════════════════════════════════════════════════════

/// Гейт покрытия (M-38b) считает `last_seq(S) = first_seq(следующий) − 1` по СВОЕМУ
/// перечислению. Если оно пропускает `.zst`, «следующим» оказывается ДАЛЬНИЙ сегмент, и
/// `last_seq` завышается — покрытый сегмент молча признаётся непокрытым, место не
/// освобождается никогда, а причина в отчёте выглядит правдоподобно. Никакого особого
/// правила для смеси не требуется: требуется ПОЛНОЕ перечисление.
///
/// ⚠ Величину нельзя мерить через `journal::list_segments` — он `.zst` уже видит, и оракул
/// был бы плацебо (зелёным при сломанном ретеншене; поймано прогоном первой редакции). Мерим
/// ПОРОГ, наблюдаемый через сам `retention_plan`: минимальный `covered_through_seq`, при
/// котором сегмент становится prunable. Он обязан совпадать с `last_seq` — и до, и после
/// компакции, для КАЖДОГО закрытого сегмента (множественность, `testing.md` п.2).
#[test]
fn rt_z_5_prunable_threshold_matches_last_seq_before_and_after_compaction() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);
    let now = t0 + 10 * DAY_MS;

    // Порог, наблюдаемый через retention_plan: prunable при covered==L и НЕ prunable при L−1.
    let check = |dir: &std::path::Path, target: u32, stage: &str| {
        let l = last_seq_of(dir, target).unwrap_or_else(|| {
            panic!("{stage}: у закрытого сегмента {target} обязан быть last_seq")
        });
        let at = |c: u64| {
            idx(
                &journal::retention_plan(dir, &policy(cold.path(), Some(c)), now)
                    .expect("plan")
                    .offload_and_prune,
            )
        };
        assert!(
            at(l).contains(&target),
            "{stage}: сегмент {target} обязан стать prunable ровно при covered=last_seq={l}.\n\
             ДОЛЖНО БЫТЬ: offload_and_prune содержит {target}\nПОЛУЧЕНО: {:?}\n\
             Порог, по которому ретеншен принимает решение, не совпадает с реальным last_seq — \
             значит его перечисление сегментов НЕПОЛНОЕ (сосед взят через дыру).",
            at(l)
        );
        assert!(
            !at(l - 1).contains(&target),
            "{stage}: сегмент {target} стал prunable при covered={} < last_seq={l} — граница \
             покрытия C-030 R1 сдвинута, prune уходит вперёд чекпоинтера.\nПОЛУЧЕНО: {:?}",
            l - 1,
            at(l - 1)
        );
    };

    let closed: Vec<u32> = vec![0, 1, 2, 3];
    for t in &closed {
        check(dir.path(), *t, "до компакции");
    }

    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    assert!(
        zst_count(dir.path()) >= 2,
        "фикстура не состоялась: нужно ≥2 сжатых сегмента, есть {}",
        zst_count(dir.path())
    );
    for t in &closed {
        check(dir.path(), *t, "ПОСЛЕ компакции");
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-6 — наблюдение ОТСУТСТВИЯ: неклассифицируемый .zst обязан быть НАЗВАН
// ═════════════════════════════════════════════════════════════════════════════════════

/// Свойство 4 «целостности гейта» (`testing.md`): монитор обязан замечать не только сбой, но
/// и молчание. Файл `segment-00000042.jrnl.zst`, который не удалось классифицировать, сегодня
/// не попадает НИ В ОДНУ корзину плана — оператор не узнает о нём никогда. Дополнительно
/// оракул пиннит ИНДЕКС: синтетический `SegmentInfo` для непрочитанного файла строится через
/// `parse_segment_index` (не знает про `.zst`) и даёт `u32::MAX` — оператор увидит
/// «сегмент 4294967295» вместо 42.
#[test]
fn rt_z_6_unclassifiable_compacted_file_is_named_in_skipped() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);

    // Файл с именем сегмента, но не zstd-потоком: обрыв копирования / битый диск / чужой файл.
    let bogus = dir.path().join("segment-00000042.jrnl.zst");
    std::fs::write(&bogus, b"this is not a zstd stream").expect("write bogus");

    let now = t0 + 10 * DAY_MS;
    let plan = journal::retention_plan(dir.path(), &policy(cold.path(), Some(u64::MAX)), now)
        .expect("plan");

    let named: Vec<(u32, String)> = plan
        .skipped
        .iter()
        .filter(|(s, _)| {
            s.path
                .file_name()
                .is_some_and(|n| n == "segment-00000042.jrnl.zst")
        })
        .map(|(s, r)| (s.index, r.clone()))
        .collect();
    assert!(
        !named.is_empty(),
        "нечитаемый segment-00000042.jrnl.zst не назван в плане НИ В ОДНОЙ корзине — оператор \
         не узнает о повреждённом файле ниоткуда.\nДОЛЖНО БЫТЬ: запись в skipped с причиной\n\
         ПОЛУЧЕНО: skipped={:?}",
        plan.skipped
            .iter()
            .map(|(s, r)| (
                s.index,
                s.path.file_name().map(|n| n.to_string_lossy().to_string()),
                r.clone()
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        named[0].0,
        42,
        "индекс нечитаемого сжатого сегмента разобран неверно.\nДОЛЖНО БЫТЬ: 42\n\
         ПОЛУЧЕНО: {} (u32::MAX = {} означает, что имя разбиралось парсером, не знающим \
         про суффикс .zst)",
        named[0].0,
        u32::MAX
    );

    // Один битый файл не имеет права отменить план по здоровым сегментам.
    assert!(
        !plan.offload_and_prune.is_empty(),
        "битый файл обнулил план по остальным сегментам — оператор получил «нечего делать» \
         вместо «вот это сделаю, а вот с этим разберись»"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-7 — ops-шов: канарейка ИСПОЛНЯЕТ ВЫЗЫВАТЕЛЯ (не библиотеку)
// ═════════════════════════════════════════════════════════════════════════════════════

/// «Код на main ≠ функция в проде» (`docs/08`, системный паттерн №2). Библиотечный фикс,
/// не доехавший до операторского вывода, оставляет оператора с тем же слепым отчётом.
/// Поэтому оракул запускает НАСТОЯЩИЙ бинарь `journal-retention` (тот же приём, что
/// `red_cli_argv.rs`), в той же форме argv, что `docker-compose.yml`.
///
/// Проверяются ДВЕ вещи:
/// (а) сжатые сегменты НАЗВАНЫ в выводе плана — сегодня их там нет вообще;
/// (б) корзина `offload_only` печатается. Сегодня `print_plan` печатает только
///     `offload_and_prune` и `skipped`, а `offload_only` — НЕТ. Без покрытия чекпоинтом
///     (штатный fail-closed режим, в котором прод и живёт сейчас) ВСЕ кандидаты попадают
///     именно в `offload_only`: оператор видит «offload_and_prune: 0» и заключает, что
///     ретеншен не сделает ничего, — хотя `Apply` скопирует эти сегменты в холодное
///     хранилище. Отчёт, умалчивающий о том, что будет сделано, — это тихая ложь ops-пути.
#[test]
fn rt_z_7_retention_binary_reports_compacted_segments_and_offload_only() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    assert!(
        zst_count(dir.path()) >= 2,
        "фикстура: нужен смешанный каталог с ≥2 .zst"
    );

    let now = t0 + 10 * DAY_MS;
    // Equals-форма argv — ровно та, что в `docker-compose.yml command:` (TD-024).
    // БЕЗ `--checkpoint-coverage`: это сегодняшний прод-режим (артефакт покрытия ещё не
    // подан ретеншену) ⇒ fail-closed ⇒ всё уходит в offload_only.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_journal-retention"))
        .args([
            format!("--dir={}", dir.path().display()),
            format!("--cold={}", cold.path().display()),
            format!("--now-wall-ms={now}"),
            "--mode=dry-run".to_string(),
            "--retain-days=1".to_string(),
            "--keep-min=1".to_string(),
            "--min-free-gb=0".to_string(),
        ])
        .output()
        .expect("запуск journal-retention");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(
        out.status.code().unwrap_or(-1),
        0,
        "dry-run обязан завершаться успешно.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    assert!(
        stdout.contains(".jrnl.zst"),
        "операторский вывод НЕ НАЗЫВАЕТ ни одного сжатого сегмента.\n\
         ДОЛЖНО БЫТЬ: в плане перечислены segment-*.jrnl.zst (в каталоге их {})\n\
         ПОЛУЧЕНО:\n{stdout}\n\
         Оператор запускает dry-run перед первым реальным apply (R1) и по этому выводу решает, \
         что уедет в холодное хранилище. Сжатая история в нём не упомянута ни строкой.",
        zst_count(dir.path())
    );

    // C-037 N1: раздельные проверки `contains(".jrnl.zst")` и `contains("offload_only")`
    // выполнимы выводом, где .zst перечислены под `skipped`, а `offload_only` — пустой
    // заголовок со счётчиком. Это ровно класс «оракул мерит не то, что обещает» (TD-021),
    // поэтому связь проверяется ПО СЕКЦИИ: сжатый сегмент обязан быть НАЗВАН именно в
    // корзине offload_only.
    let section = section_lines(&stdout, "offload_only");
    assert!(
        !section.is_empty(),
        "в выводе нет секции offload_only.\nДОЛЖНО БЫТЬ: печатается наравне с \
         offload_and_prune и skipped\nПОЛУЧЕНО:\n{stdout}\n\
         Без артефакта покрытия чекпоинтом (сегодняшний прод-режим) ВСЕ кандидаты попадают \
         именно туда: оператор видит «offload_and_prune: 0» и делает вывод, что apply ничего \
         не сделает, хотя копирование в холодное хранилище произойдёт."
    );
    assert!(
        section.iter().any(|l| l.contains(".jrnl.zst")),
        "секция offload_only не НАЗЫВАЕТ ни одного сжатого сегмента.\n\
         ДОЛЖНО БЫТЬ: под заголовком offload_only перечислен хотя бы один segment-*.jrnl.zst \
         (в каталоге их {}, покрытия нет ⇒ все кандидаты обязаны быть там)\n\
         ПОЛУЧЕНО в секции:\n{}\n\nПОЛНЫЙ вывод:\n{stdout}\n\
         Счётчика в заголовке недостаточно: оператор решает по ИМЕНАМ, что именно уедет \
         в холодное хранилище.",
        zst_count(dir.path()),
        section.join("\n")
    );
}

/// Строки ПОД заголовком секции операторского вывода (`  <name>: N сегмент(ов)`) — до
/// следующего заголовка того же уровня. Нужна, чтобы проверять принадлежность строки
/// КОРЗИНЕ, а не факт её присутствия где-то в stdout.
fn section_lines(stdout: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in stdout.lines() {
        let header = line.trim_start();
        let is_header = line.starts_with("  ")
            && !line.starts_with("    ")
            && header.contains(':')
            && !header.starts_with('-');
        if is_header {
            if inside {
                break;
            }
            inside = header.starts_with(name);
            continue;
        }
        if inside {
            out.push(line.to_string());
        }
    }
    out
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-8 — C-037 B1: синтетический `first_seq = 0` соседа НЕ ЕСТЬ ФАКТ (TD-030, fail-open)
// ═════════════════════════════════════════════════════════════════════════════════════

/// **Добавлено по вердикту critic C-037 (B1). Architect ошибся, критик прав — проверено
/// замером.**
///
/// Гейт покрытия (C-030 R1) считает `last_seq(S) = first_seq(следующий) − 1`. Но у
/// ЗАДЕКЛАРИРОВАННОГО legacy-сегмента `first_seq` не читается из файла — он СИНТЕЗИРУЕТСЯ
/// нулём (`segments.rs:509-512`, `SegmentHeader::from_legacy_decl`). Тогда
/// `last_seq(предшественник) = 0.saturating_sub(1) = 0`, и проверка `0 <= covered` истинна
/// при ЛЮБОМ покрытии, включая `covered = 0`. Предшественник объявляется покрытым и
/// удаляется, хотя чекпоинт не свернул ни одного его события.
///
/// **Возражение architect'а, которое НЕ выдержало проверки:** «legacy лежит только под
/// индексом 0, значит у него нет предшественника». Это историческое наблюдение о старом
/// писателе, а не инвариант: `LegacySegmentDecl.file_name` — обычная `String`, и
/// `declare_legacy` принимает любое имя сегмента. Замер (probe architect'а, origin/feat/M-40
/// @ 492b01e):
///
/// ```text
/// declare_legacy для segment-00000003.jrnl — ПРИНЯТ
///   seg 2 first_seq=342 schema_version=4
///   seg 3 first_seq=0   schema_version=1   ← синтетический ноль
/// PLAN при covered=0: offload_and_prune=[2]   ← события seq 342..510 удаляются
///                     offload_only=[0, 1]      ← соседи защищены корректно
/// ```
///
/// Дефект точечный и потому особенно опасен: соседние сегменты обрабатываются правильно,
/// поэтому отчёт оператора выглядит здоровым.
///
/// **Контракт (architect):** `last_seq` обязан быть ФАКТОМ. Если заголовок следующего
/// сегмента не даёт достоверного `first_seq` (legacy: `schema_version ==
/// SCHEMA_VERSION_PRE_HEADER`), реализация ОБЯЗАНА установить `last_seq` иначе — прямым
/// чтением хвоста самого кандидата (`tail_last_seq_of` уже bounded, TD-011). Принимать
/// синтетический ноль за факт ЗАПРЕЩЕНО. «Нет информации» не равно «покрыт».
///
/// ПАРНЫЙ vantage обязателен: при покрытии, реально включающем последний seq кандидата,
/// он ОБЯЗАН пруниться — иначе гвард выродится в «рядом с legacy не пруним никогда», и
/// место не освободится (тот же класс, что covered_segments_are_pruned в C-030).
#[test]
fn rt_z_8_synthetic_zero_first_seq_of_legacy_neighbour_is_not_a_fact() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);
    let now = t0 + 10 * DAY_MS;

    // ФАКТИЧЕСКИЙ last_seq кандидата фиксируем ДО того, как сосед станет legacy:
    // пока сегмент 3 нормальный, его first_seq достоверен.
    let target: u32 = 2;
    let real_last_seq = last_seq_of(dir.path(), target).expect("last_seq до мутации соседа");

    // Сосед (сегмент 3) превращается в задекларированный legacy — ровно операторская
    // процедура, которой на проде объявлен segment-00000000.jrnl (15 GB истории).
    let victim = dir.path().join("segment-00000003.jrnl");
    std::fs::write(
        &victim,
        b"LEGACY-RAW-FRAMES-NO-V2-MAGIC-payload-payload-payload",
    )
    .expect("write legacy");
    journal::declare_legacy(
        dir.path(),
        LegacySegmentDecl {
            file_name: "segment-00000003.jrnl".to_string(),
            fingerprint_sha256: String::new(), // declare_legacy пересчитает сам
            size_bytes_at_decl: 0,             // и размер тоже
            source: DataSource::OwnCapture,
            provenance: "legacy fixture (M-40 B1)".to_string(),
            epoch_id: "own-test".to_string(),
        },
    )
    .expect("declare_legacy обязан принять индекс != 0 — иначе фикстура не воспроизводит форму");

    // Setup-guard (свойство 3 «целостности гейта»): фикстура обязана ДОКАЗАТЬ, что создала
    // именно ту форму, ради которой написана, а не «просто битый файл».
    let segs = journal::list_segments(dir.path()).expect("segments");
    let legacy = segs
        .iter()
        .find(|s| s.index == 3)
        .expect("сегмент 3 обязан остаться в перечислении");
    assert_eq!(
        legacy.header.schema_version,
        contracts::SCHEMA_VERSION_PRE_HEADER,
        "фикстура не состоялась: сегмент 3 не распознан как LEGACY (schema_version={}). \
         Без этого оракул проверяет не ту форму и вырождается в плацебо.",
        legacy.header.schema_version
    );
    assert_eq!(
        legacy.header.first_seq, 0,
        "фикстура не состоялась: у legacy-сегмента first_seq={} вместо синтетического 0 — \
         ловушка TD-030 не воспроизведена",
        legacy.header.first_seq
    );

    // (а) FAIL-OPEN ЗАПРЕЩЁН: чекпоинт свернул только seq 0 — кандидат НЕ покрыт.
    let plan0 = journal::retention_plan(dir.path(), &policy(cold.path(), Some(0)), now)
        .expect("plan covered=0");
    assert!(
        !idx(&plan0.offload_and_prune).contains(&target),
        "C-030 R1 НАРУШЕН (fail-open через TD-030): сегмент {target} попал в prune при \
         covered=0, хотя его события — seq ..{real_last_seq}.\n\
         ДОЛЖНО БЫТЬ: offload_and_prune НЕ содержит {target} (и {target} уходит в offload_only)\n\
         ПОЛУЧЕНО: offload_and_prune={:?}, offload_only={:?}\n\
         Причина: last_seq взят из СИНТЕТИЧЕСКОГО first_seq=0 задекларированного legacy-соседа \
         и выродился в 0, из-за чего «0 <= covered» истинно при любом покрытии. Соседние \
         сегменты при этом обрабатываются верно, поэтому отчёт оператора выглядит здоровым, \
         а история молча удаляется.",
        idx(&plan0.offload_and_prune),
        idx(&plan0.offload_only)
    );
    assert!(
        idx(&plan0.offload_only).contains(&target),
        "непокрытый сегмент {target} обязан уйти в offload_only (бэкап R1 не блокируется).\n\
         ПОЛУЧЕНО: offload_only={:?}, skipped={:?}",
        idx(&plan0.offload_only),
        skipped_idx(&plan0)
    );

    // (б) ПАРНЫЙ vantage: при реальном покрытии кандидат ОБЯЗАН пруниться. Иначе
    // реализация «сосед legacy ⇒ никогда не покрыт» заморозит место навсегда.
    let plan_full = journal::retention_plan(dir.path(), &policy(cold.path(), Some(u64::MAX)), now)
        .expect("plan covered=MAX");
    assert!(
        idx(&plan_full.offload_and_prune).contains(&target),
        "сегмент {target} (last_seq={real_last_seq}) не прунится даже при covered=u64::MAX.\n\
         ДОЛЖНО БЫТЬ: offload_and_prune содержит {target}\nПОЛУЧЕНО: {:?}\n\
         Гвард выродился в «рядом с legacy не пруним никогда»: «нет информации» подменило \
         «не покрыт», и диск не освобождается. last_seq обязан быть УСТАНОВЛЕН фактически \
         (чтение хвоста кандидата), а не объявлен неизвестным.",
        idx(&plan_full.offload_and_prune)
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RT-Z-9 — C-037 B2: деградированный каталог не имеет права делать отчёт лживым
// ═════════════════════════════════════════════════════════════════════════════════════

/// **Добавлено по вердикту critic C-037 (B2).** Задача #4 milestone'а требует, чтобы
/// `retention_execute` перечислял сегменты ТЕМ ЖЕ способом, что и `retention_plan`, но
/// `rt_z_4` гоняет только ЗДОРОВЫЙ каталог и этого не наблюдает.
///
/// `retention_execute` вызывает `segments(_dir).unwrap_or_default()` (`segments.rs:1825`).
/// `segments()` — fail-closed: ОДИН чужой/битый файл делает весь вызов `Err`, а
/// `unwrap_or_default()` превращает его в ПУСТОЙ список. Дальше `last_seq_opt = None` для
/// каждого сегмента ⇒ `is_uncovered = true` ⇒ все удаления попадают в
/// `pruned_without_checkpoint_coverage`.
///
/// Замер (probe architect'а) — ПОЛНОЕ покрытие `covered = u64::MAX`, один чужой файл:
/// ```text
/// APPLY pruned=4
/// APPLY pruned_without_checkpoint_coverage=4  ← все четыре, при полном покрытии
/// ```
///
/// Направление ошибки безопасное (over-reporting, не удаление лишнего), но аудит-трейл
/// операторского override становится ЛОЖЬЮ: поле, существующее ровно для того, чтобы
/// назвать поимённо каждое удаление без покрытия, называет удаления, которые покрыты
/// полностью. Оператор, увидев такой отчёт, либо перестанет ему верить, либо начнёт
/// расследовать несуществующий инцидент. Дефект живой на `main` и воспроизводится даже
/// без единого `.zst`.
#[test]
fn rt_z_9_foreign_file_does_not_corrupt_coverage_audit_of_healthy_prunes() {
    let t0 = now_ms() - 30 * DAY_MS;
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    build(dir.path(), t0);
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");

    // Чужой файл с именем сегмента: незадекларированный legacy / обрыв копирования / мусор.
    // Ровно то, из-за чего segments() отдаёт Err целиком.
    std::fs::write(
        dir.path().join("segment-00000042.jrnl"),
        b"not a journal segment at all",
    )
    .expect("write foreign");

    // Setup-guard: доказать, что фикстура создала ОБА условия — деградацию и здоровый .zst.
    assert!(
        journal::list_segments(dir.path()).is_err(),
        "фикстура не состоялась: segments() обязан быть Err из-за чужого файла — иначе \
         деградированный путь не воспроизведён и оракул проверяет не то"
    );
    assert!(
        zst_count(dir.path()) >= 2,
        "фикстура не состоялась: нужно ≥2 здоровых сжатых сегмента, есть {}",
        zst_count(dir.path())
    );

    let now = t0 + 10 * DAY_MS;
    let pol = policy(cold.path(), Some(u64::MAX)); // покрыто ВСЁ, без всякого override
    let plan = journal::retention_plan(dir.path(), &pol, now).expect("plan");
    assert!(
        !plan.offload_and_prune.is_empty(),
        "один чужой файл обнулил план по здоровым сегментам: offload_and_prune пуст, \
         skipped={:?}",
        skipped_idx(&plan)
    );

    let report =
        journal::retention_execute(dir.path(), &plan, &pol, RetentionMode::Apply).expect("apply");

    assert!(
        report.pruned_without_checkpoint_coverage.is_empty(),
        "АУДИТ-ТРЕЙЛ ЛЖЁТ: при covered=u64::MAX и allow_prune_without_checkpoint=false отчёт \
         называет удаления «без покрытия чекпоинтом».\n\
         ДОЛЖНО БЫТЬ: pruned_without_checkpoint_coverage пуст\n\
         ПОЛУЧЕНО: {:?}\n\
         pruned={:?}\n\
         Причина: execute перечисляет сегменты через segments(_dir).unwrap_or_default(), и один \
         чужой файл схлопывает список в пустой ⇒ last_seq неизвестен для ВСЕХ ⇒ каждое удаление \
         помечено как непокрытое. Поле, созданное для поимённого аудита операторского override, \
         перестаёт что-либо значить.",
        report.pruned_without_checkpoint_coverage,
        report.pruned
    );
    assert!(
        report.failed.is_empty(),
        "здоровые сегменты не должны попадать в failed из-за чужого файла: {:?}",
        report.failed
    );
    assert!(
        report
            .pruned
            .iter()
            .any(|p| p.to_string_lossy().ends_with(".zst")),
        "ни один сжатый сегмент не удалён — деградированный каталог не отменяет работу по \
         здоровым кандидатам.\nПОЛУЧЕНО: pruned={:?}",
        report.pruned
    );
    assert!(
        report.freed_bytes > 0,
        "freed_bytes=0 при непустом pruned — отчёт оператора недостоверен"
    );
}
