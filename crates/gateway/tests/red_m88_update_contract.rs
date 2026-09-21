//! RED M-88 (sacred, architect-only) — КОНТРАКТ ОБНОВЛЕНИЯ книго-зависимых серий.
//!
//! Предмет: снятый уровень книги обязан исчезнуть и у КЛИЕНТА, собирающего состояние из
//! снимка и кадров. Сегодня не исчезает: `merge_heatmap` объединяет ячейки по ключу
//! `(time_s, side, price_e8)` из `existing.chain(incoming)`, а путь полного пересчёта
//! отбрасывает `size <= 0` — живая склейка расходится с пересчётом, нарушая `VB-I-2`
//! (`docs/fa/viz-backend.md` §5). Спека — `milestones/M-88-liquidity-removal-contract.md`.
//!
//! ПОЧЕМУ ЭТОГО НЕ ЛОВИЛ НИ ОДИН ОРАКУЛ (замер спеки §3): пересечение множеств
//! {тесты со склейкой кадров} × {тесты со снятием уровня на входе} × {тесты с ассертом по
//! карте} в `crates/gateway/tests` ∪ `crates/gateway-serve/tests` было ПУСТО. Предмет
//! дефекта ни разу не подавался на путь, где дефект живёт. Этот файл закрывает пересечение.
//!
//! ПУТЬ ПРОВЕРКИ — ПРОД-ФОРМА ЦЕЛИКОМ: формирование (`frames_since`/`replay`) →
//! СЕРИАЛИЗАЦИЯ (`serde_json`, тот же кодек, что у `gateway-serve`) → применение
//! (`Snapshot::apply`). Серверная функция слияния в одиночку не засчитывается
//! (`docs/plans/scale-program-2026-09-21.md` §15.2).
//!
//! ЭТАЛОН — полный пересчёт `gateway::snapshot(.., Cursor::LATEST)` (независимый путь), И
//! ДОПОЛНИТЕЛЬНО литеральное ожидание (`independent_example_*`), потому что оба пути делят
//! формулу окна и могут ошибаться одинаково (`plan §11`).
//!
//! АНТИ-ПЛАЦЕБО: набор обязан краснеть на ДВУХ мутациях (спека §9.2 п.3) — (а) возврат
//! `merge_heatmap` к объединению; (б) трактовка пустого полного среза как отсутствия
//! обновления. Проверяется мутационным контролем, результат — в Done Block.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Frame, Selector, Snapshot};
use journal::{EpochFilter, Journal, WriterConfig};

/// Один бакет B0 при `timeframe_ms = 1000`.
const T: i64 = 1_752_000_010_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvls(v: &[(f64, f64)]) -> Vec<Level> {
    v.iter()
        .map(|&(p, s)| Level {
            price: to_fixed(p),
            size: to_fixed(s),
        })
        .collect()
}

fn snapshot_ev(bids: &[(f64, f64)], asks: &[(f64, f64)], ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lvls(bids),
            asks: lvls(asks),
            ts_exch_ms: ts,
        },
    )
}

/// Дельта книги. `size = 0.0` — СНЯТИЕ уровня (`crates/book/src/lib.rs:60,67`:
/// `size == 0` ⇒ `remove`). Именно этот вход отсутствовал во всём корпусе выдачи.
fn delta_ev(bids: &[(f64, f64)], asks: &[(f64, f64)], u0: u64, u1: u64, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: lvls(bids),
            asks: lvls(asks),
            first_update_id: u0,
            final_update_id: u1,
            prev_final_update_id: None,
            ts_exch_ms: ts,
        },
    )
}

fn trade_ev(price: f64, size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

fn journal_of(events: Vec<EventKind>) -> (tempfile::TempDir, Vec<u64>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut seqs = Vec::new();
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            seqs.push(j.append(e).expect("append").seq);
        }
        j.flush().expect("flush");
    }
    (dir, seqs)
}

/// Окно heatmap/COB — состояние ПРОЦЕССА (`set_effective_heatmap_window_frac`), а тесты
/// этого файла ставят разные охваты. Без сериализации сосед перезаписывает окно под ногами
/// и тест падает, НЕ БУДУЧИ сломанным (тот же приём и та же причина, что в `red_heatmap.rs`).
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn sel(bands: Vec<f64>) -> Selector {
    let w = bands.iter().copied().fold(0.0_f64, f64::max);
    if w > 0.0 {
        gateway::set_effective_heatmap_window_frac(w);
    }
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
        depth_cadence_ms: None,
    }
}

fn full_snapshot(dir: &std::path::Path, s: &Selector) -> Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, s, Cursor::LATEST).expect("snapshot LATEST")
}

fn snapshot_at(dir: &std::path::Path, s: &Selector, at: Cursor) -> Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, s, at).expect("snapshot at")
}

/// ПРОВОД: кадр уезжает клиенту сериализованным (`gateway-serve` шлёт `serde_json::to_vec`
/// целиком). Оракул обязан гонять ту же форму, иначе проверен не тот путь
/// (`testing.md` §«Целостность гейта», свойство 1).
fn via_wire(f: &Frame) -> Frame {
    let bytes = serde_json::to_vec(f).expect("frame → wire");
    serde_json::from_slice::<Frame>(&bytes).expect("wire → frame")
}

/// Клиент: собирает состояние из снимка и кадров, полученных ПО ПРОВОДУ.
fn client_fold(base: Snapshot, frames: &[Frame]) -> Snapshot {
    let mut acc = base;
    for f in frames {
        let wired = via_wire(f);
        let _ = acc.apply(&wired);
    }
    acc
}

/// Все кадры от курсора до хвоста, малыми батчами (как их тянет `gateway-serve`).
fn drain_frames(dir: &std::path::Path, s: &Selector, from: Cursor) -> Vec<Frame> {
    let mut cur = from;
    let mut out = Vec::new();
    loop {
        let (batch, next) =
            gateway::frames_since(dir, EpochFilter::OwnCaptureOnly, s, cur, 2).expect("frames");
        if batch.is_empty() {
            break;
        }
        out.extend(batch);
        assert!(next > cur, "непустой батч обязан продвинуть курсор");
        cur = next;
    }
    out
}

/// Карта в канонической форме сравнения: `(time_s, side, price_e8) → size_e8`.
fn heatmap_of(s: &Snapshot) -> std::collections::BTreeMap<(i64, String, i64), i64> {
    s.series
        .heatmap
        .iter()
        .map(|c| ((c.time_s, c.side.clone(), c.price_e8), c.size_e8))
        .collect()
}

fn cob_of(s: &Snapshot) -> std::collections::BTreeMap<(String, i64), i64> {
    s.series
        .cob
        .iter()
        .map(|l| ((l.side.clone(), l.price_e8), l.size_e8))
        .collect()
}

// ─────────────────────────── S1 — исчезновение в ТЕКУЩЕМ бакете ───────────────────────────

/// Уровень стоит, затем снят дельтой ВНУТРИ ТОГО ЖЕ бакета; курсор клиента режет между.
/// Это тот самый вход, которого не было ни в одном оракуле корпуса.
#[test]
fn s1_removed_level_disappears_in_current_bucket() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(
            &[(64_990.0, 5.0), (64_980.0, 3.0)],
            &[(65_010.0, 4.0), (65_020.0, 1.0)],
            T,
        ),
        // СНЯТИЕ bid 64990 — тот же бакет (ts отличается на 1 мс, `time_s` тот же).
        delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 1),
    ]);

    let full = full_snapshot(dir.path(), &s);
    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));
    let frames = drain_frames(dir.path(), &s, Cursor::at(seqs[0]));
    assert!(
        !frames.is_empty(),
        "setup-страж: кадров нет — проверять нечего"
    );
    let acc = client_fold(base, &frames);

    assert!(
        !heatmap_of(&full).contains_key(&(T / 1000, "bid".to_string(), to_fixed(64_990.0))),
        "setup-страж: полный пересчёт ОБЯЗАН не содержать снятый уровень"
    );
    assert_eq!(
        heatmap_of(&acc),
        heatmap_of(&full),
        "VB-I-2: клиент, собравший состояние из снимка и кадров, обязан совпасть с полным \
         пересчётом. Расхождение = снятая ликвидность живёт на экране"
    );
}

/// Независимый пример (`plan §15.9` п.1): ожидаемое множество ячеек выписано ЛИТЕРАЛОМ,
/// без вызова кода выдачи. Ловит случай «оба пути ошибаются одинаково».
#[test]
fn s1b_independent_example_literal_expectation() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(
            &[(64_990.0, 5.0), (64_980.0, 3.0)],
            &[(65_010.0, 4.0), (65_020.0, 1.0)],
            T,
        ),
        delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 1),
    ]);
    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));
    let frames = drain_frames(dir.path(), &s, Cursor::at(seqs[0]));
    let acc = client_fold(base, &frames);

    // После снятия 64990: лучший bid = 64980, лучший ask = 65010 ⇒ mid = 64995,
    // окно ±0.1 % = [64930.005, 65059.995] — все три оставшихся уровня внутри.
    let t0 = T / 1000;
    let expected: std::collections::BTreeMap<(i64, String, i64), i64> = [
        ((t0, "bid".to_string(), to_fixed(64_980.0)), to_fixed(3.0)),
        ((t0, "ask".to_string(), to_fixed(65_010.0)), to_fixed(4.0)),
        ((t0, "ask".to_string(), to_fixed(65_020.0)), to_fixed(1.0)),
    ]
    .into_iter()
    .collect();

    assert_eq!(
        heatmap_of(&acc),
        expected,
        "независимый пример: после снятия bid 64990 у клиента обязаны остаться РОВНО три ячейки"
    );
}

// ─────────────────────────── S2 — сохранность ЗАКРЫТОГО прошлого ───────────────────────────

/// СТОРОЖ ОБРАТНОГО ОСЛАБЛЕНИЯ (`testing.md`, второй вопрос мутационного контроля).
/// Замена «объединить» на «заменить колонку» сильнее объединения и способна стереть то,
/// чего кадр не донёс. Ликвидность ЗАКРЫТОГО прошлого бакета обязана уцелеть.
#[test]
fn s2_closed_past_bucket_is_not_erased() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        // Следующий бакет: книга другая, прошлый бакет больше не наблюдается.
        snapshot_ev(&[(64_995.0, 7.0)], &[(65_005.0, 2.0)], T + 1_000),
    ]);
    let full = full_snapshot(dir.path(), &s);
    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));
    let acc = client_fold(base, &drain_frames(dir.path(), &s, Cursor::at(seqs[0])));

    let t0 = T / 1000;
    assert!(
        heatmap_of(&acc).contains_key(&(t0, "bid".to_string(), to_fixed(64_990.0))),
        "закрытый прошлый бакет стёрт — это ошибка, симметричная призраку"
    );
    assert_eq!(heatmap_of(&acc), heatmap_of(&full), "VB-I-2");
}

// ─────────────────────────── S3 — ПУСТОЙ полный срез ───────────────────────────

/// Дельта снимает ВСЕ уровни бакета. Полный срез существует и пуст — это НЕ «наблюдения
/// не было». Сегодня различить нельзя: `merge_cob` возвращает existing при пустом incoming
/// (`crates/gateway/src/lib.rs:2753`), а `merge_heatmap` просто ничего не удаляет.
#[test]
fn s3_empty_full_slice_replaces_previous() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0), (64_980.0, 3.0)], &[(65_010.0, 4.0)], T),
        delta_ev(
            &[(64_990.0, 0.0), (64_980.0, 0.0)],
            &[(65_010.0, 0.0)],
            1,
            2,
            T + 1,
        ),
    ]);
    let full = full_snapshot(dir.path(), &s);
    assert!(
        full.series.heatmap.is_empty(),
        "setup-страж: после снятия всех уровней полный пересчёт обязан дать пустую карту, \
         а дал {} ячеек",
        full.series.heatmap.len()
    );

    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));
    assert!(
        !base.series.heatmap.is_empty(),
        "setup-страж: до снятия карта обязана быть непустой"
    );
    let acc = client_fold(base, &drain_frames(dir.path(), &s, Cursor::at(seqs[0])));

    assert_eq!(heatmap_of(&acc), heatmap_of(&full), "VB-I-2: карта");
    assert_eq!(cob_of(&acc), cob_of(&full), "VB-I-2: стакан");
}

// ─────────────────────────── S4 — ОТСУТСТВИЕ обновления ───────────────────────────

/// Кадр без L2-событий не имеет права ничего изменить в книго-зависимых сериях.
/// Это `NoChange`, а не «пустой срез» — различение обязано быть выразимо.
#[test]
fn s4_absent_observation_changes_nothing() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        trade_ev(65_000.0, 1.0, Side::Buy, T + 10),
        trade_ev(65_000.0, 2.0, Side::Sell, T + 20),
    ]);
    let full = full_snapshot(dir.path(), &s);
    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));
    let before = heatmap_of(&base);
    let acc = client_fold(base, &drain_frames(dir.path(), &s, Cursor::at(seqs[0])));

    assert_eq!(
        heatmap_of(&acc),
        before,
        "кадр без L2-событий изменил карту — `NoChange` не соблюдён"
    );
    assert_eq!(heatmap_of(&acc), heatmap_of(&full), "VB-I-2");
}

// ─────────────────── S5 — выход цены из наблюдаемой области ≠ снятие ───────────────────

/// Середина уезжает, уровень выходит за окно карты. Это НЕ снятие ликвидности с рынка:
/// у клиента он обязан исчезнуть РОВНО так же, как в полном пересчёте, и ни метка
/// достоверности, ни ноль не подставляются вместо него.
#[test]
fn s5_price_leaving_window_matches_recompute() {
    let _g = serial();
    let s = sel(vec![0.001]); // окно ±0.1 %
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        // Середина уезжает вверх: 64990 выпадает за нижнюю границу окна.
        delta_ev(&[(65_400.0, 6.0)], &[(65_420.0, 6.0)], 1, 2, T + 1),
    ]);
    let full = full_snapshot(dir.path(), &s);
    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));
    let acc = client_fold(base, &drain_frames(dir.path(), &s, Cursor::at(seqs[0])));

    assert_eq!(
        heatmap_of(&acc),
        heatmap_of(&full),
        "VB-I-2: выход цены из окна обязан выглядеть у клиента так же, как в пересчёте"
    );
    assert!(
        acc.series.heatmap.iter().all(|c| c.size_e8 != 0),
        "ноль как размер ячейки запрещён: величина не подменяет метку достоверности"
    );
}

// ─────────────────────────── S6 — ПОВТОРНОЕ сообщение ───────────────────────────

/// Тот же кадр применён дважды. Суммируемые ряды (`volume_bubbles` складывается,
/// `ohlcv.volume +=`) обязаны не удвоиться: `Snapshot::apply` сегодня НЕ сверяет
/// `frame.from` с `self.cursor` (`crates/gateway/src/lib.rs:2575-2728`).
#[test]
fn s6_duplicate_frame_does_not_double_count() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        trade_ev(65_000.0, 1.5, Side::Buy, T + 10),
    ]);
    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));
    let frames = drain_frames(dir.path(), &s, Cursor::at(seqs[0]));
    assert!(!frames.is_empty(), "setup-страж: кадров нет");

    let once = client_fold(base.clone(), &frames);
    let twice = client_fold(client_fold(base, &frames), &frames);

    assert_eq!(
        serde_json::to_string(&twice.series).expect("ser"),
        serde_json::to_string(&once.series).expect("ser"),
        "повторно применённый кадр изменил состояние — суммируемые ряды удвоены"
    );
    assert_eq!(twice.cursor, once.cursor, "курсор сдвинут повтором");
}

// ─────────────────────────── S7 — ЗАПОЗДАЛОЕ сообщение ───────────────────────────

/// Кадр старой версии приходит после более нового. Он обязан быть отвергнут БЕЗ изменения
/// состояния: сегодня `self.cursor = frame.to` выполняется безусловно (`:2728`) и курсор
/// откатывается назад.
#[test]
fn s7_stale_frame_does_not_rewind_state() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        trade_ev(65_000.0, 1.0, Side::Buy, T + 10),
        trade_ev(65_000.0, 2.0, Side::Sell, T + 20),
        delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 30),
    ]);
    let early = drain_frames(dir.path(), &s, Cursor::at(seqs[0]));
    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));
    let advanced = client_fold(base, &early);

    let cursor_before = advanced.cursor;
    let series_before = serde_json::to_string(&advanced.series).expect("ser");

    // Тот же самый старый кадр приходит ЕЩЁ РАЗ, уже после продвижения курсора.
    let stale = early.first().expect("кадр").clone();
    let mut client = advanced;
    let _ = client.apply(&via_wire(&stale));

    assert_eq!(
        client.cursor, cursor_before,
        "запоздалый кадр откатил курсор назад"
    );
    assert_eq!(
        serde_json::to_string(&client.series).expect("ser"),
        series_before,
        "запоздалый кадр изменил состояние"
    );
}

// ─────────────────── S8 — путь ПЕРЕСЧЁТА обязан давать ту же склейку ───────────────────

/// Кадры, произведённые `replay` (путь пересчёта, без живого редьюсера), обязаны склеиваться
/// в тот же результат. Ловит производителя, забывшего заполнить новые поля контракта:
/// у клиента карта замерла бы МОЛЧА.
#[test]
fn s8_replay_frames_fold_to_same_state() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, _seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0), (64_980.0, 3.0)], &[(65_010.0, 4.0)], T),
        delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 1),
        snapshot_ev(&[(64_985.0, 2.0)], &[(65_015.0, 3.0)], T + 1_000),
    ]);
    let full = full_snapshot(dir.path(), &s);
    let empty = snapshot_at(dir.path(), &s, Cursor::START);
    let frames = gateway::replay(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        Cursor::LATEST,
    )
    .expect("replay");
    assert!(!frames.is_empty(), "setup-страж: replay не дал кадров");

    let acc = client_fold(empty, &frames);
    assert_eq!(
        heatmap_of(&acc),
        heatmap_of(&full),
        "VB-I-2: склейка кадров реплея разошлась с полным пересчётом"
    );
}

// ─────────────── S9 — два кадра по ОДНОМУ бакету: срез обязан быть ПОЛНЫМ ───────────────

/// Реализация, отдающая ЧАСТИЧНУЮ колонку (только изменённые ячейки), проходит S1, но
/// ломается здесь: второй кадр того же бакета обязан нести полный срез, иначе замена
/// колонки сотрёт неизменившиеся уровни.
#[test]
fn s9_second_frame_of_same_bucket_carries_full_slice() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(
            &[(64_990.0, 5.0), (64_980.0, 3.0), (64_970.0, 7.0)],
            &[(65_010.0, 4.0)],
            T,
        ),
        delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 1), // кадр A: снятие
        delta_ev(&[(64_980.0, 9.0)], &[], 2, 3, T + 2), // кадр B: обновление другого уровня
    ]);
    let full = full_snapshot(dir.path(), &s);
    let base = snapshot_at(dir.path(), &s, Cursor::at(seqs[0]));

    // Кадры тянутся ПО ОДНОМУ событию — два отдельных кадра на один и тот же бакет.
    let mut cur = Cursor::at(seqs[0]);
    let mut frames = Vec::new();
    loop {
        let (batch, next) =
            gateway::frames_since(dir.path(), EpochFilter::OwnCaptureOnly, &s, cur, 1)
                .expect("frames");
        if batch.is_empty() {
            break;
        }
        frames.extend(batch);
        cur = next;
    }
    assert!(
        frames.len() >= 2,
        "setup-страж: нужно ≥2 кадра одного бакета, получено {}",
        frames.len()
    );

    let acc = client_fold(base, &frames);
    assert_eq!(
        heatmap_of(&acc),
        heatmap_of(&full),
        "VB-I-2: колонка собрана из частичных наблюдений — неизменившиеся уровни потеряны \
         либо призрак уцелел"
    );
}

// ─────────── Проверка 2 плана §15.9 — РАЗНЫЕ границы кадров, тот же итог ───────────

/// Те же события, нарезанные на кадры ПО-РАЗНОМУ, обязаны дать побайтно один итог.
/// Ловит правило склейки, зависящее от того, как поток разбит на сообщения.
#[test]
fn different_frame_boundaries_yield_identical_state() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let events = vec![
        snapshot_ev(&[(64_990.0, 5.0), (64_980.0, 3.0)], &[(65_010.0, 4.0)], T),
        delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 1),
        trade_ev(65_000.0, 1.0, Side::Buy, T + 5),
        delta_ev(&[(64_985.0, 8.0)], &[], 2, 3, T + 7),
        snapshot_ev(&[(64_985.0, 8.0)], &[(65_015.0, 2.0)], T + 1_000),
    ];
    let (dir, seqs) = journal_of(events);
    let from = Cursor::at(seqs[0]);

    let fold_with_batch = |max: usize| -> String {
        let base = snapshot_at(dir.path(), &s, from);
        let mut cur = from;
        let mut frames = Vec::new();
        loop {
            let (batch, next) =
                gateway::frames_since(dir.path(), EpochFilter::OwnCaptureOnly, &s, cur, max)
                    .expect("frames");
            if batch.is_empty() {
                break;
            }
            frames.extend(batch);
            cur = next;
        }
        serde_json::to_string(&client_fold(base, &frames).series).expect("ser")
    };

    let by_one = fold_with_batch(1);
    let by_all = fold_with_batch(usize::MAX);
    assert_eq!(
        by_one, by_all,
        "итог зависит от нарезки потока на кадры — правило склейки не инвариантно к границам"
    );
}
