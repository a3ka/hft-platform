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

    let res =
        gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly);
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
