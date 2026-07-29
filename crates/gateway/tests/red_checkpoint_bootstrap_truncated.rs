//! RED M-48 (sacred, architect-only) — **GW-I-12: усечённая история ДЕКЛАРИРУЕТСЯ, а не
//! запрещается. Бутстрап чекпоинта на журнале со спруненным префиксом ЛЕГАЛЕН.**
//!
//! ## Дефект (TD-048, найден reviewer'ом на §8 M-38b — прод-замер, не анализ)
//!
//! M-38b смержен со всеми зелёными гейтами и **инертен в проде**: `gateway-checkpoint` падает
//! `exit=1`, чекпоинт не создаётся НИКОГДА, первый Snapshot по-прежнему 382.657 s при цели <10 s.
//!
//! ```text
//! $ docker compose --profile ops run --rm gateway-checkpoint
//! err=checkpoint::advance_to: нет валидного чекпоинта в /ckpt, но первый видимый сегмент
//!     имеет first_seq=16049334 > 0 (префикс уже спрунен). ОСТАНОВИТЕСЬ ...
//! CKPT_EXIT=1
//! ```
//!
//! Виновато правило, которое написал **я** (M-38b §(1b) rev3 #3): «нет чекпоинта И
//! `first_visible_seq > 0` → падать громко». На проде `segment-00000000` удалён purge'ем M-36
//! **необратимо**, поэтому условие истинно навсегда.
//!
//! ## Асимметрия, которую этот оракул закрывает
//!
//! Read-path при отсутствии чекпоинта СПОКОЙНО редуцирует от первого ВИДИМОГО seq и отдаёт
//! результат кокпиту как «all-time» — это ровно тот путь, что реально отработал за 382 s.
//! То есть **усечённая история уже обслуживается**; отказывался её персистить только
//! checkpoint-path. Одно и то же состояние было легально отдавать и нелегально сохранять.
//!
//! **Принятая семантика (одна на обоих путях):** «all-time» ≡ «от самого раннего seq, доступного
//! под данным `EpochFilter`». Система не отказывается отдать то, что у неё есть, — она
//! отказывается ВЫДАВАТЬ ЭТО ЗА ДРУГОЕ. Лечение — не отказ, а честность: `history_start_seq` +
//! `history_truncated` на проводе и в заголовке чекпоинта (прецедент — `depth_band_provenance`,
//! VB-I-5: серия по не-полностью-подтверждённым данным ОБЯЗАНА нести провенанс).
//!
//! **`history_start_seq` берётся из ПЕРВОГО реально свёрнутого события**, а не из
//! `header.first_seq`: у legacy-сегментов он синтезирован нулём (`segments.rs:509-512`, TD-030)
//! и соврал бы ровно там, где нужна правда.
//!
//! ## Почему 470 зелёных тестов это пропустили
//!
//! `red_checkpoint_prefix_pruned` гоняет «префикс спрунен ПОСЛЕ того, как чекпоинт уже есть».
//! Прод-случай — «чекпоинта НЕТ и уже не будет» — не покрывал ни один оракул. Класс «идеальная
//! фикстура» (`.claude/rules/testing.md`), пятый раз подряд. Поэтому фикстура здесь —
//! ПРОД-ФОРМЫ: сегмент 0 физически отсутствует, `/ckpt` пуст.
//!
//! COMPILE/RUNTIME-RED: `Snapshot.history_start_seq`/`history_truncated` ещё нет; `advance`
//! на усечённом журнале сейчас возвращает `Err`.
//!
//! testing.md: п.4 границы (пустой ckpt-каталог; разрыв ровно на `C+1` и на `C+2`), п.5
//! прод-масштаб (много сегментов, смесь raw/`.zst`), п.7 ПАРНЫЙ vantage (на НЕусечённом журнале
//! `history_truncated == false` — заглушка «всегда truncated» падает).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const N: u64 = 2_000;
const SEG_BYTES: u64 = 8 * 1024;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
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
        window_ms: None,
    }
}

fn intact_journal() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    dir
}

/// Прод-форма: удалить нижние сегменты ФИЗИЧЕСКИ (как purge M-36 / retention-prune).
/// Возвращает seq первого события, оставшегося доступным.
fn truncate_prefix(dir: &std::path::Path, drop_below_index: u32) -> u64 {
    for s in journal::list_segments(dir)
        .expect("segments")
        .iter()
        .filter(|s| s.index < drop_below_index)
    {
        std::fs::remove_file(&s.path).expect("remove segment");
    }
    journal::stream(dir, EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .next()
        .expect("хотя бы одно событие осталось")
        .expect("event")
        .seq
}

fn truncated_journal() -> (tempfile::TempDir, u64) {
    let dir = intact_journal();
    let total = journal::list_segments(dir.path()).expect("segments").len() as u32;
    assert!(total >= 6, "нужен многосегментный журнал, есть {total}");
    let earliest = truncate_prefix(dir.path(), 3);
    assert!(
        earliest > 0,
        "фикстура ОБЯЗАНА быть усечённой (прод-форма): earliest={earliest}"
    );
    (dir, earliest)
}

fn canon(s: &gateway::Snapshot) -> String {
    serde_json::to_string(s).expect("сериализация")
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Бутстрап на усечённом журнале ЛЕГАЛЕН (прод-случай TD-048)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn bootstrap_on_truncated_journal_succeeds() {
    let (dir, earliest) = truncated_journal();
    let ckpt = tempfile::tempdir().expect("ckpt"); // пуст — чекпоинта НЕТ и уже не будет

    let cursor =
        gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
            .unwrap_or_else(|e| {
                panic!(
            "GW-I-12 НАРУШЕН (TD-048): бутстрап чекпоинта на журнале со спруненным префиксом \
             отвергнут: {e}\n\
             Это прод-состояние VPS (segment-00000000 удалён purge'ем M-36 НЕОБРАТИМО, \
             earliest_seq={earliest}), поэтому отказ означает «чекпоинт не поднимется НИКОГДА» \
             и фичу, мёртвую в проде при зелёных гейтах. При этом read-path ту же самую \
             усечённую историю СПОКОЙНО отдаёт кокпиту как all-time (замер: 382.657 s). \
             Отказ ничего не восстанавливает — он лишь запрещает СОХРАНИТЬ то, что и так ОТДАЁТСЯ."
        )
            });
    assert!(
        cursor.upto_seq.is_some(),
        "бутстрап обязан вернуть достигнутый курсор"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Честность: усечённость ДЕКЛАРИРУЕТСЯ на обоих путях и значения совпадают
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn truncation_is_declared_identically_on_both_paths() {
    let (dir, earliest) = truncated_journal();
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("бутстрап (см. bootstrap_on_truncated_journal_succeeds)");

    let full = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("snapshot от START");
    let (via, _stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint");

    for (name, snap) in [("snapshot(START)", &full), ("from_checkpoint", &via)] {
        assert!(
            snap.history_truncated,
            "{name}: журнал усечён (earliest={earliest}), поле history_truncated обязано быть \
             true — иначе кокпит выдаёт неполную историю за полную (класс тихой лжи, ровно \
             тот, ради которого существует depth_band_provenance / VB-I-5)"
        );
        assert_eq!(
            snap.history_start_seq, earliest,
            "{name}: history_start_seq обязан равняться seq ПЕРВОГО реально свёрнутого события. \
             Брать его из header.first_seq нельзя: у legacy-сегментов он синтезирован нулём \
             (segments.rs:509-512, TD-030) и соврал бы именно здесь"
        );
    }

    assert_eq!(
        canon(&via),
        canon(&full),
        "GW-I-9 остаётся в силе: путь через чекпоинт байт-идентичен реплею от START на том же \
         (усечённом) журнале"
    );
}

/// ПАРНЫЙ vantage (п.7): на НЕусечённом журнале усечённость не выдумывается.
/// Заглушка «всегда truncated=true» падает здесь.
#[test]
fn intact_journal_is_not_declared_truncated() {
    let dir = intact_journal();
    let snap = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("snapshot");
    assert!(
        !snap.history_truncated,
        "журнал полный (сегмент 0 на месте) — history_truncated обязан быть false"
    );
    assert_eq!(
        snap.history_start_seq, 0,
        "на полном журнале история начинается с seq 0"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Fail-loud СУЖЕН: отказ только там, где данные реально теряются незаметно
// ─────────────────────────────────────────────────────────────────────────────

/// Разрыв «чекпоинт ↔ журнал»: валидный чекпоинт с курсором `C`, а самый ранний доступный seq
/// журнала `> C + 1`. События между ними не свёрнуты НИ ВО ЧТО. Докорм «поверх дырки» дал бы
/// состояние, не соответствующее ни одной реальной истории, — вот ЭТО обязано быть громким.
///
/// **ВАЖНО ПРО АНТИ-ПЛАЦЕБО (B1, reviewer PR-гейт).** Reviewer нейтрализовал спец-проверку
/// разрыва — и этот тест ОСТАЛСЯ зелёным, потому что его удовлетворяла ДРУГАЯ ветка («stale
/// чекпоинт-файл → Err»). То есть задача #3 была мёртвым кодом. Анти-плацебо здесь наступает
/// ТОЛЬКО В ПАРЕ с задачей #9: когда невалидный файл перестаёт быть ошибкой (GW-I-9б, тихий
/// rebuild), единственный способ пройти этот тест — настоящая проверка разрыва по ЗАГОЛОВКУ
/// чекпоинта (задача #8: `read_checkpoint_header` отдаёт `cursor` даже у непригодного к
/// использованию файла). Вместе с парным `contiguous_boundary_is_not_a_gap` (стык
/// `earliest == C+1` обязан ПРОЙТИ) это не оставляет места заглушке: «всегда Err» валится о
/// парный тест, «никогда Err» — об этот.
/// **Не удалять и не ослаблять ни один из трёх — они работают только вместе.**
#[test]
fn gap_between_checkpoint_and_journal_is_loud() {
    let dir = intact_journal();
    let ckpt = tempfile::tempdir().expect("ckpt");

    // Чекпоинт на раннем курсоре (внутри сегмента 0/1).
    let segs = journal::list_segments(dir.path()).expect("segments");
    let c = segs
        .iter()
        .find(|s| s.index == 1)
        .expect("сегмент 1")
        .header
        .first_seq;
    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        Cursor { upto_seq: Some(c) },
    )
    .expect("advance_to на раннем курсоре");

    // Теперь сносим префикс ГЛУБЖЕ курсора чекпоинта — образуется дырка.
    let earliest = truncate_prefix(dir.path(), 4);
    assert!(
        earliest > c + 1,
        "фикстура обязана создать РАЗРЫВ: earliest={earliest} должен быть > C+1={}",
        c + 1
    );

    let before = ckpt_dir_fingerprint(ckpt.path());
    let res =
        gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly);
    // C-032 R3: «вернул Err» НЕДОСТАТОЧНО — реализация могла записать чекпоинт поверх дырки И
    // вернуть ошибку. Отказ обязан быть БЕЗ ПОБОЧНЫХ ЭФФЕКТОВ: содержимое ckpt-каталога
    // байт-в-байт прежнее. Иначе ошибка уедет в лог cron и будет забыта, а порча останется на
    // диске и следующий запуск примет её за валидный чекпоинт.
    assert_eq!(
        ckpt_dir_fingerprint(ckpt.path()),
        before,
        "GW-I-12: при разрыве отказ обязан НИЧЕГО НЕ ПИСАТЬ, но содержимое ckpt-каталога изменилось"
    );
    assert!(
        res.is_err(),
        "GW-I-12: разрыв между курсором чекпоинта ({c}) и самым ранним доступным seq \
         ({earliest}) означает, что события между ними не свёрнуты ни в чекпоинт, ни в журнал. \
         Докорм поверх дырки обязан быть ГРОМКИМ отказом — это единственный случай, где отказ \
         защищает данные, а не запрещает штатное состояние. Получено: {:?}",
        res.map(|c| c.upto_seq)
    );
}

/// Граница (п.4): стык РОВНО `earliest == C + 1` — разрыва НЕТ, докорм законен.
/// Off-by-one здесь означал бы отказ на штатном стыке «чекпоинт кончился, журнал продолжается».
#[test]
fn contiguous_boundary_is_not_a_gap() {
    let dir = intact_journal();
    let ckpt = tempfile::tempdir().expect("ckpt");

    let segs = journal::list_segments(dir.path()).expect("segments");
    let boundary = segs
        .iter()
        .find(|s| s.index == 3)
        .expect("сегмент 3")
        .header
        .first_seq;

    // Чекпоинт ровно до последнего seq перед границей сегмента 3.
    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        Cursor {
            upto_seq: Some(boundary - 1),
        },
    )
    .expect("advance_to до границы");

    // Сносим ровно то, что чекпоинт уже свернул: earliest становится == C + 1.
    let earliest = truncate_prefix(dir.path(), 3);
    assert_eq!(
        earliest, boundary,
        "фикстура: earliest обязан быть ровно C+1 (стык без разрыва)"
    );

    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect(
            "стык earliest == C+1 — это НЕ разрыв: чекпоинт свернул всё до C, журнал продолжается \
         с C+1. Отказ здесь заблокировал бы штатный режим «prune покрытого префикса», ради \
         которого писалась суффикс-совместимая валидация lineage (C-030 N2)",
        );
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. C-032 R2 — ЛОВУШКА legacy `first_seq = 0` (мой пропуск в rev1 оракула)
// ─────────────────────────────────────────────────────────────────────────────

/// Спека требует брать `history_start_seq` из ПЕРВОГО РЕАЛЬНО СВЁРНУТОГО события, а не из
/// `header.first_seq`. Но фикстуры выше состоят только из v2-сегментов, у которых `first_seq`
/// ЧЕСТНЫЙ — значит реализация, читающая заголовок, проходила бы их незамеченной. Это тот же
/// класс плацебо, на котором меня уже поймал critic в C-030 R2 (`red_stream_from`).
///
/// **Ловушка:** legacy-сегмент (headerless, CT-RFC-02) несёт СИНТЕЗИРОВАННЫЙ `first_seq = 0`
/// (`segments.rs:509-512` — «безопасный дефолт», не измеренное значение), но реально содержит
/// события с `seq >= LEGACY_FIRST_SEQ > 0`. Реализация «по заголовку» отрапортует
/// `history_start_seq = 0, history_truncated = false` — то есть объявит усечённую историю
/// ПОЛНОЙ. Реализация «по свёрнутому событию» даст правду.
///
/// Фикстура сконструирована (не снята с прода): на VPS legacy-сегмент начинался с seq 0. Она
/// проверяет ПРАВИЛО, а правило написано именно потому, что заголовку legacy доверять нельзя.
const LEGACY_FIRST_SEQ: u64 = 500;
const LEGACY_COUNT: u64 = 200;

fn legacy_first_journal() -> tempfile::TempDir {
    use std::io::Write as _;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("segment-00000000.jrnl");
    {
        let f = std::fs::File::create(&path).expect("create legacy");
        let mut w = std::io::BufWriter::new(f);
        for seq in LEGACY_FIRST_SEQ..LEGACY_FIRST_SEQ + LEGACY_COUNT {
            let ev = contracts::Event {
                seq,
                ts_mono_ns: seq,
                ts_wall_ms: 1_752_000_000_000 + seq as i64,
                kind: trade(seq),
            };
            let payload = postcard::to_stdvec(&ev).expect("ser");
            w.write_all(&(payload.len() as u32).to_le_bytes()).unwrap();
            w.write_all(&payload).unwrap();
            w.write_all(&crc32fast::hash(&payload).to_le_bytes())
                .unwrap();
        }
        w.flush().unwrap();
    }
    let next = LEGACY_FIRST_SEQ + LEGACY_COUNT;
    std::fs::write(dir.path().join("journal.meta"), next.to_le_bytes()).expect("meta");
    journal::declare_legacy(
        dir.path(),
        contracts::LegacySegmentDecl {
            file_name: "segment-00000000.jrnl".to_string(),
            fingerprint_sha256: journal::fingerprint(&path).expect("fingerprint"),
            size_bytes_at_decl: std::fs::metadata(&path).expect("meta").len(),
            source: DataSource::OwnCapture,
            provenance: "M-48 fixture: headerless segment NOT starting at seq 0".to_string(),
            epoch_id: contracts::LEGACY_EPOCH_ID.to_string(),
        },
    )
    .expect("declare_legacy");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with поверх legacy");
        for i in next..next + 300 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

#[test]
fn history_start_seq_ignores_lying_legacy_header() {
    let dir = legacy_first_journal();

    // АНТИ-ПЛАЦЕБО: фикстура обязана реально содержать legacy с синтезированным нулём.
    let segs = journal::list_segments(dir.path()).expect("segments");
    let legacy = segs
        .iter()
        .find(|s| s.header.schema_version == contracts::SCHEMA_VERSION_PRE_HEADER)
        .expect("фикстура обязана содержать legacy-сегмент, иначе ловушка не взведена");
    assert_eq!(
        legacy.header.first_seq, 0,
        "у legacy `first_seq` обязан быть синтезированным нулём — в этом и ловушка"
    );

    let snap = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("snapshot");

    assert_eq!(
        snap.history_start_seq, LEGACY_FIRST_SEQ,
        "history_start_seq взят из ЗАГОЛОВКА legacy-сегмента (синтезированный 0), а не из \
         первого РЕАЛЬНО свёрнутого события ({LEGACY_FIRST_SEQ}). Заголовку legacy доверять \
         нельзя (TD-030, segments.rs:509-512) — иначе усечённая история объявляется полной"
    );
    assert!(
        snap.history_truncated,
        "история начинается с seq {LEGACY_FIRST_SEQ} > 0 ⇒ truncated=true. Реализация «по \
         заголовку» отрапортовала бы false и выдала неполную историю за полную"
    );
}

/// Снимок содержимого ckpt-каталога: отсортированные пары (относительный путь, байты).
/// Отличает «отказал» от «отказал, но всё-таки записал».
fn ckpt_dir_fingerprint(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &std::path::Path, d: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(rd) = std::fs::read_dir(d) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap_or(&p).display().to_string();
                out.push((rel, std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. C-032 R3 — немонотонность по-прежнему запрещена (перепиннивание D2 из C-030 rev3)
// ─────────────────────────────────────────────────────────────────────────────

/// Сужая fail-loud, легко потерять защиту, ради которой он вводился: `advance` не имеет права
/// заменить чекпоинт состоянием, покрывающим МЕНЬШЕ истории. Здесь валидный чекпоинт заявляет
/// историю с seq 0; затем идёт ЗАКОННЫЙ prune покрытого префикса. `advance` обязан
/// РЕЗЮМИРОВАТЬСЯ (заявленная история остаётся с 0), а не пересобрать состояние от нового
/// earliest, молча сузив историю.
#[test]
fn advance_after_covered_prune_does_not_regress_history_start() {
    let dir = intact_journal();
    let ckpt = tempfile::tempdir().expect("ckpt");

    let segs = journal::list_segments(dir.path()).expect("segments");
    let cut = 3_u32;
    let cut_first = segs
        .iter()
        .find(|s| s.index == cut)
        .expect("сегмент cut")
        .header
        .first_seq;

    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        Cursor {
            upto_seq: Some(cut_first - 1),
        },
    )
    .expect("advance_to до границы сегмента");

    let (before, _) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint до prune");
    assert_eq!(before.history_start_seq, 0, "предусловие: история с нуля");
    assert!(!before.history_truncated, "предусловие: журнал полон");

    let earliest = truncate_prefix(dir.path(), cut);
    assert_eq!(
        earliest, cut_first,
        "предусловие: earliest == C+1 (стык без разрыва)"
    );

    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance после ЗАКОННОГО prune обязан пройти (стык, а не разрыв)");

    let (after, _) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint после prune");

    assert_eq!(
        after.history_start_seq, 0,
        "РЕГРЕССИЯ ПОКРЫТИЯ: чекпоинт заявлял историю с seq 0, а после законного prune стал \
         заявлять её с {}. Значит advance пересобрал состояние от нового earliest вместо \
         резюмирования — свёрнутая история молча потеряна (D2 из C-030 rev3). Сужение fail-loud \
         в M-48 НЕ должно было снять эту защиту",
        after.history_start_seq
    );
    assert!(
        !after.history_truncated,
        "чекпоинт помнит историю с нуля ⇒ truncated остаётся false, несмотря на prune префикса"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. C-033 R2 — НЕМОНОТОННОСТЬ ПО КУРСОРУ: advance_to(меньший) не откатывает чекпоинт
// ─────────────────────────────────────────────────────────────────────────────

/// **СТАТУС ЧЕСТНО: это ЗЕЛЁНЫЕ регресс-пины, а не RED.** Прогнаны отдельно против текущего кода
/// (в обход compile-RED остального файла) — `2 passed`, в том числе с усиленным по C-034
/// требованием `Err`. Значит гвард монотонности УЖЕ реализован dev'ом в M-38b (D2 из C-030 rev3)
/// и полон: он и не пишет, и отказывает. Ценность тестов — в том, что снять его при сужении
/// fail-loud в M-48 будет нельзя.
///
/// **Что закрыл C-034 R2.** Прежняя версия проверяла ТОЛЬКО байты и допускала `Ok(no-op)`.
/// Этого мало: `Ok(low)` без записи оставляет файл нетронутым, тест зеленеет — а бинарь
/// `gateway-checkpoint` публикует `covered_through_seq` из ВОЗВРАЩЁННОГО курсора (путь B2 из
/// PR-гейта reviewer'а), и покрытие уезжает вниз при физически неизменном чекпоинте. Поэтому
/// пиннится и возвращаемое значение, и байты: два независимых канала, через которые регрессия
/// могла бы просочиться.
///
/// Критик показал дыру в rev2: `advance_after_covered_prune_does_not_regress_history_start`
/// давит только на НАЧАЛО истории. Гвард немонотонности можно удалить целиком, и все прежние
/// оракулы M-48 останутся зелёными, потому что ни один не вызывает `advance_to` с курсором
/// МЕНЬШЕ уже записанного.
///
/// Сценарий реален для ops: cron-обёртка запускается с `--cursor=LATEST`, а оператор рядом
/// гоняет усечённый прогон `--cursor <seq>` для диагностики (эта форма прямо предусмотрена
/// комментарием в `docker-compose.yml`). Если второй вызов перезапишет чекпоинт более ранним
/// состоянием, кокпит после следующего подключения молча откатится назад по времени, а
/// `covered_through_seq` уедет вниз — и retention, наоборот, перестанет прунить (или, при
/// обратном порядке, спрунит то, что уже не покрыто).
#[test]
fn advance_to_lower_cursor_does_not_regress_checkpoint() {
    let dir = intact_journal();
    let ckpt = tempfile::tempdir().expect("ckpt");

    let high = Cursor {
        upto_seq: Some(N - 1),
    };
    let low = Cursor {
        upto_seq: Some(N / 3),
    };

    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        high,
    )
    .expect("advance_to(high) обязан пройти");
    let after_high = ckpt_dir_fingerprint(ckpt.path());
    assert!(
        !after_high.is_empty(),
        "предусловие: чекпоинт записан на высоком курсоре"
    );

    // C-034 R2: сравнивать ТОЛЬКО байты — недостаточно, и прежняя формулировка «разрешено
    // вернуть Ok(no-op) или Err» была ошибкой. Реализация вправе вернуть `Ok(low)`, не тронув
    // файл: байты совпадут, тест позеленеет — а бинарь `gateway-checkpoint` публикует
    // `covered_through_seq` ИЗ ВОЗВРАЩЁННОГО курсора (это путь B2 из PR-гейта reviewer'а).
    // Тогда покрытие уедет ВНИЗ: retention начнёт принимать решения по рубежу, который ниже
    // уже опубликованного, при физически неизменном чекпоинте. Поэтому контракт строгий:
    // запрос курсора НИЖЕ уже сохранённого — это ошибка, а не тихий no-op.
    let res = gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        low,
    );
    assert!(
        res.is_err(),
        "advance_to({:?}) поверх чекпоинта на {:?} обязан вернуть Err, а не Ok. `Ok(low)` без \
         записи оставляет байты чекпоинта нетронутыми (проверка ниже это пропустит), но \
         gateway-checkpoint опубликует covered_through_seq ИЗ ВОЗВРАЩЁННОГО курсора — и \
         покрытие уедет вниз при неизменном чекпоинте. Получено: {:?}",
        low.upto_seq,
        high.upto_seq,
        res.as_ref().map(|c| c.upto_seq)
    );
    assert_eq!(
        ckpt_dir_fingerprint(ckpt.path()),
        after_high,
        "РЕГРЕССИЯ ПО КУРСОРУ: advance_to({:?}) перезаписал чекпоинт, снятый на {:?}. \
         Монотонность покрытия — та самая защита D2 (C-030 rev3), которую сужение fail-loud \
         в M-48 не имело права снять: после отката кокпит молча уезжает назад во времени, а \
         covered_through_seq опускается ниже уже опубликованного (retention принимает решения \
         по устаревшему рубежу).",
        low.upto_seq,
        high.upto_seq
    );
}

/// ПАРНЫЙ vantage (п.7): гвард не переширок — движение ВПЕРЁД обязано обновлять чекпоинт.
/// Заглушка «после первой записи не писать никогда» проходит тест выше и падает здесь.
#[test]
fn advance_to_higher_cursor_does_update_checkpoint() {
    let dir = intact_journal();
    let ckpt = tempfile::tempdir().expect("ckpt");

    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        Cursor {
            upto_seq: Some(N / 3),
        },
    )
    .expect("advance_to(low)");
    let after_low = ckpt_dir_fingerprint(ckpt.path());

    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        Cursor {
            upto_seq: Some(N - 1),
        },
    )
    .expect("advance_to(high) поверх низкого");

    assert_ne!(
        ckpt_dir_fingerprint(ckpt.path()),
        after_low,
        "движение ВПЕРЁД обязано обновлять чекпоинт — иначе гвард монотонности выродился в \
         «после первой записи не пишем никогда», и чекпоинт навсегда застрял бы на первом \
         прогоне cron'а (латентность TD-044 не вылечена)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. B2 (reviewer PR-гейт) — УСТАРЕВШИЙ чекпоинт-файл НЕ является ошибкой
// ─────────────────────────────────────────────────────────────────────────────

/// Файл чекпоинта в ckpt-каталоге (без `*.tmp`).
fn ckpt_file_path(dir: &std::path::Path) -> std::path::PathBuf {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .expect("read_dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| p.extension().and_then(|s| s.to_str()) != Some("tmp"))
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| !n.ends_with(".lock"))
        })
        .collect();
    files.sort();
    files
        .pop()
        .expect("advance обязан был создать файл чекпоинта")
}

/// Подменить `gateway_schema_version` в заголовке файла (смещение 12..16 — сразу после
/// magic и `ckpt_schema_version`). Моделирует БУДУЩИЙ штатный bump схемы: файл на диске
/// снят прежней версией кода.
fn corrupt_gw_schema_version(path: &std::path::Path) {
    let mut bytes = std::fs::read(path).expect("read ckpt");
    assert!(bytes.len() > 16, "чекпоинт подозрительно мал");
    let other = gateway::GATEWAY_SCHEMA_VERSION + 1;
    bytes[12..16].copy_from_slice(&other.to_le_bytes());
    std::fs::write(path, &bytes).expect("write ckpt");
}

/// **B2 (reviewer): устаревший чекпоинт-файл ВОСПРОИЗВОДИТ TD-048, если он фатален.**
///
/// `decode_checkpoint` отвергает файл при `gw_v != GATEWAY_SCHEMA_VERSION`, и файл ОСТАЁТСЯ
/// на диске. На проде `first_visible_seq = 16049334 > 0` **навсегда** (purge M-36 необратим),
/// поэтому реализация, которая на «файл есть, но не декодируется + префикс усечён» возвращает
/// `Err`, после ЛЮБОГО будущего бампа схемы больше не поднимет чекпоинт — до ручного
/// `rm ckpt-*.bin`. Бампы рутинны: v5 (M-23), v6 (M-36), v7 (M-38a), v8 (сам M-48).
/// Это ровно TD-048, воссозданный на другом входе.
///
/// **Правило (GW-I-9б, уже действовавшее — здесь оно ПРИБИВАЕТСЯ):** чекпоинт — КЭШ. Любая
/// невалидность (версия, фингерпринт, CRC, мусор) → ТИХИЙ rebuild и ПЕРЕЗАПИСЬ файла.
/// Никогда не ошибка, никогда не «удалите вручную». Ручное вмешательство в ops-пути — это
/// и есть инертность фичи в проде.
#[test]
fn stale_schema_version_checkpoint_rebuilds_silently_and_overwrites() {
    let (dir, earliest) = truncated_journal(); // прод-форма: префикс спрунен НАВСЕГДА
    let ckpt = tempfile::tempdir().expect("ckpt");

    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("бутстрап на усечённом журнале обязан пройти");
    let path = ckpt_file_path(ckpt.path());
    let before = std::fs::read(&path).expect("read");

    // Моделируем будущий штатный bump схемы: файл снят прежней версией кода.
    corrupt_gw_schema_version(&path);

    let res =
        gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly);
    let cursor = res.unwrap_or_else(|e| {
        panic!(
            "B2 НАРУШЕН (воспроизведение TD-048): устаревший по версии схемы чекпоинт-файл \
             сделан ФАТАЛЬНЫМ: {e}\n\
             На проде first_visible_seq={earliest}>0 навсегда, поэтому после любого будущего \
             бампа схемы (а они рутинны: v5/v6/v7/v8) gateway-checkpoint не поднимется НИКОГДА \
             до ручного rm. Чекпоинт — КЭШ (GW-I-9б): невалидный файл обязан быть тихо \
             перестроен и ПЕРЕЗАПИСАН, а не требовать оператора."
        )
    });
    assert!(cursor.upto_seq.is_some(), "rebuild обязан вернуть курсор");

    let after = std::fs::read(&path).expect("read after");
    assert_ne!(
        after, before,
        "файл обязан быть ПЕРЕЗАПИСАН свежим чекпоинтом (иначе на диске навсегда останется \
         нечитаемый файл, и каждый следующий запуск будет платить полный реплей)"
    );
    assert_eq!(
        &after[12..16],
        &gateway::GATEWAY_SCHEMA_VERSION.to_le_bytes(),
        "перезаписанный чекпоинт обязан нести ТЕКУЩУЮ версию схемы"
    );

    // И read-path обязан работать через него, а не уходить в полный реплей.
    let (via, _stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint после самолечения");
    let full = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("snapshot(START)");
    assert_eq!(
        canon(&via),
        canon(&full),
        "после самолечения путь через чекпоинт обязан быть байт-идентичен реплею от START"
    );
}
