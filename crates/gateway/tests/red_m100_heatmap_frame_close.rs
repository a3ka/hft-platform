//! RED `M-100` (sacred, architect-only) — **колонка карты в ЖИВОМ кадре есть СОСТОЯНИЕ бакета на
//! последнем наблюдении кадра (close), а не объединение всех наблюдений кадра** (`VB-I-2`,
//! `VB-I-12`; спека `milestones/M-100-three-serving-contracts.md` §5 «семантика heatmap»).
//!
//! # Дефект — по коду и прогоном
//!
//! Живой путь (`LiveReducer::pump` → `book_series_in`, `crates/gateway/src/lib.rs` на `a660b3a7`)
//! собирает ячейки кадра в `heatmap_map.insert(..)` по ВСЕМ наблюдениям диапазона кадра: ключ,
//! присутствовавший в раннем наблюдении бакета и снятый более поздним наблюдением ТОГО ЖЕ бакета
//! в ТОМ ЖЕ кадре, в колонке остаётся. Колонка объявлена наблюдённой (`heatmap_observed_time_s`)
//! и применяется клиентом как `Replace` (`M-88`) — снятый уровень переживает у клиента, пока
//! бакет не придёт снова. Полный пересчёт (`gateway::snapshot`) берёт последнее состояние бакета
//! целиком — живое ≠ пересчитанное. `M-88` `s1` этого не ловил: у него снимок и снятие лежали в
//! РАЗНЫХ кадрах (базовый снимок резал между ними).
//!
//! Найдено разведкой по коду (`scratchpad/heatmap-facts.md` п.6) и подтверждено прогоном этого
//! файла на `a660b3a7` (см. §8 спеки).
use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, LiveReducer, Selector, Snapshot};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_752_000_010_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m100".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lv(v: &[(f64, f64)]) -> Vec<Level> {
    v.iter()
        .map(|(p, s)| Level {
            price: to_fixed(*p),
            size: to_fixed(*s),
        })
        .collect()
}

fn snap(bids: &[(f64, f64)], asks: &[(f64, f64)], ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lv(bids),
            asks: lv(asks),
            ts_exch_ms: ts,
        },
    )
}

fn delta(bids: &[(f64, f64)], asks: &[(f64, f64)], u0: u64, u1: u64, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: lv(bids),
            asks: lv(asks),
            first_update_id: u0,
            final_update_id: u1,
            prev_final_update_id: None,
            ts_exch_ms: ts,
        },
    )
}

fn trade(ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(65_000.0),
            size: to_fixed(0.1),
            side: Side::Buy,
            ts_exch_ms: ts,
        },
    )
}

fn append(dir: &std::path::Path, evs: Vec<EventKind>) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for e in evs {
        j.append(e).expect("append");
    }
    j.flush().expect("flush");
}

fn sel() -> Selector {
    gateway::set_effective_heatmap_window_frac(0.001);
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
        depth_cadence_ms: None,
    }
}

fn heatmap_of(s: &Snapshot) -> std::collections::BTreeMap<(i64, String, i64), i64> {
    s.series
        .heatmap
        .iter()
        .map(|c| ((c.time_s, c.side.clone(), c.price_e8), c.size_e8))
        .collect()
}

/// **`h1` — уровень поставлен и СНЯТ внутри одного бакета, оба события в ОДНОМ живом кадре.**
/// Клиент, собравший снимок и живые кадры (через провод), обязан совпасть с полным пересчётом.
#[test]
fn h1_level_removed_within_one_live_frame_disappears_for_client() {
    let s = sel();
    let dir = tempfile::tempdir().expect("tmp");
    append(dir.path(), vec![trade(T - 10_000)]);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let (mut live, _) =
        LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .expect("resume");
    let base = live.snapshot();
    // ОДИН бакет: снимок с bid 64990, затем дельта, снимающая его (ts +1 мс).
    append(
        dir.path(),
        vec![
            snap(&[(64_990.0, 5.0), (64_980.0, 3.0)], &[(65_010.0, 4.0)], T),
            delta(&[(64_990.0, 0.0)], &[], 1, 2, T + 1),
        ],
    );
    let (frames, _cur, _st) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, usize::MAX)
        .expect("pump");
    assert_eq!(
        frames.len(),
        1,
        "setup-страж: оба события обязаны уйти ОДНИМ кадром — иначе это сценарий M-88 s1"
    );
    let mut acc = base;
    for f in &frames {
        let wired: gateway::Frame =
            serde_json::from_slice(&serde_json::to_vec(f).expect("ser")).expect("de");
        let out = acc.apply(&wired);
        assert_eq!(
            out,
            gateway::ApplyOutcome::Applied,
            "кадр не применён: {out:?}"
        );
    }
    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("full");
    let key = (T / 1000, "bid".to_string(), to_fixed(64_990.0));
    assert!(
        !heatmap_of(&full).contains_key(&key),
        "setup-страж: полный пересчёт ОБЯЗАН не содержать снятый уровень"
    );
    assert_eq!(
        heatmap_of(&acc),
        heatmap_of(&full),
        "M-100 / VB-I-2 / VB-I-12: колонка живого кадра — объединение наблюдений кадра, а не \
         состояние бакета на последнем наблюдении: уровень, снятый внутри кадра, живёт у клиента"
    );
}

/// Последовательность одного бакета: снимок (bid 64990×5, 64980×3; ask 65010×4) → снятие 64990 →
/// постановка 64970×2. Свежие значения на каждый вызов (`EventKind` строится заново).
fn h2_events() -> Vec<EventKind> {
    vec![
        snap(&[(64_990.0, 5.0), (64_980.0, 3.0)], &[(65_010.0, 4.0)], T),
        delta(&[(64_990.0, 0.0)], &[], 1, 2, T + 1),
        delta(&[(64_970.0, 2.0)], &[], 3, 4, T + 2),
    ]
}

/// Клиентская склейка для одного РАЗБИЕНИЯ событий на кадры. Каждая группа дописывается в журнал
/// и отдаётся ОТДЕЛЬНЫМ вызовом `pump`; кадры идут через провод (`serde_json`). **Setup-страж на
/// КАЖДЫЙ вызов (`C-305` R1):** группа обязана дать хотя бы один живой кадр, курсор после вызова
/// обязан стоять ровно на последнем событии группы, и последний кадр обязан его покрывать — иначе
/// разбиение молча слилось бы с соседним и сценарий судил бы не ту границу. Сумма кадров по
/// разбиению стражем НЕ является.
type Hm = std::collections::BTreeMap<(i64, String, i64), i64>;
fn fold_partition(name: &str, groups: &[&[usize]]) -> (Hm, Hm) {
    let s = sel();
    let dir = tempfile::tempdir().expect("tmp");
    append(dir.path(), vec![trade(T - 10_000)]);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let (mut live, _) =
        LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .expect("resume");
    let mut acc = live.snapshot();
    // Преамбула (сделка в другом бакете) прокачивается ДО групп: после холодного `resume`
    // курсор пуст, и первая группа иначе слилась бы с преамбулой в один вызов.
    let (pre, _c, _s) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, usize::MAX)
        .expect("pump преамбулы");
    for f in &pre {
        let wired: gateway::Frame =
            serde_json::from_slice(&serde_json::to_vec(f).expect("ser")).expect("de");
        assert_eq!(acc.apply(&wired), gateway::ApplyOutcome::Applied);
    }
    for (gi, g) in groups.iter().enumerate() {
        let mut evs = h2_events();
        let picked: Vec<EventKind> = g
            .iter()
            .map(|&i| std::mem::replace(&mut evs[i], trade(0)))
            .collect();
        let before = live
            .cursor()
            .upto_seq
            .expect("SETUP НЕ СОСТОЯЛСЯ: курсор живого редуктора пуст до группы");
        append(dir.path(), picked);
        let want_upto = before + g.len() as u64;
        let (frames, cur, _s) = live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, usize::MAX)
            .expect("pump");
        assert!(
            !frames.is_empty(),
            "SETUP НЕ СОСТОЯЛСЯ: разбиение {name}, группа {gi} {g:?} не дала НИ ОДНОГО живого \
             кадра — граница кадра не построена"
        );
        assert_eq!(
            cur.upto_seq,
            Some(want_upto),
            "SETUP НЕ СОСТОЯЛСЯ: разбиение {name}, группа {gi}: курсор после pump ≠ seq последнего \
             события группы — группа не закрыта своим вызовом"
        );
        assert_eq!(
            frames.last().map(|f| f.to.upto_seq),
            Some(Some(want_upto)),
            "SETUP НЕ СОСТОЯЛСЯ: разбиение {name}, группа {gi}: последний кадр не покрывает \
             события группы"
        );
        for f in &frames {
            let wired: gateway::Frame =
                serde_json::from_slice(&serde_json::to_vec(f).expect("ser")).expect("de");
            assert_eq!(acc.apply(&wired), gateway::ApplyOutcome::Applied);
        }
    }
    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("full");
    (heatmap_of(&acc), heatmap_of(&full))
}

/// **`h2` — те же события, РАЗНЫЕ границы кадров на ЖИВОМ пути (план SCALE §15.9 п.2; `C-297`
/// M-100 R1, `C-305` R1) — один и тот же итог у клиента, равный полному пересчёту.** Разбиения:
/// всё одним кадром · по событию на кадр · снятие с постановкой вместе · постановка отдельно.
///
/// **Исход КАЖДОГО разбиения печатается одним сообщением** (`H2_PARTITIONS`) до вердикта: сценарий
/// не обрывается на первом расхождении. Так прогон сам предъявляет, какие разбиения сегодня
/// расходятся с пересчётом (`C-305`: утверждение спеки — исполняемым доказательством, не прозой).
#[test]
fn h2_same_events_different_live_frame_boundaries_same_state() {
    let parts: [(&str, &[&[usize]]); 4] = [
        ("[1 2 3]", &[&[0, 1, 2]]),
        ("[1][2][3]", &[&[0], &[1], &[2]]),
        ("[1 2][3]", &[&[0, 1], &[2]]),
        ("[1][2 3]", &[&[0], &[1, 2]]),
    ];
    let removed = (T / 1000, "bid".to_string(), to_fixed(64_990.0));
    let added = (T / 1000, "bid".to_string(), to_fixed(64_970.0));
    let mut report = Vec::new();
    let mut diverged = Vec::new();
    let mut clients = Vec::new();
    for (name, groups) in parts {
        let (client, full) = fold_partition(name, groups);
        assert!(
            !full.contains_key(&removed) && full.contains_key(&added),
            "SETUP НЕ СОСТОЯЛСЯ: полный пересчёт обязан не содержать 64990 и содержать 64970 ({name})"
        );
        let ok = client == full;
        report.push(format!(
            "{name}: {} (снятый 64990 у клиента: {})",
            if ok {
                "== пересчёт"
            } else {
                "РАСХОДИТСЯ"
            },
            client.contains_key(&removed)
        ));
        if !ok {
            diverged.push(name);
        }
        clients.push((name, client));
    }
    println!("H2_PARTITIONS {}", report.join(" | "));
    assert!(
        diverged.is_empty(),
        "M-100 / VB-I-2 / §15.9 п.2: разбиения {diverged:?} расходятся с полным пересчётом. \
         Исходы всех четырёх: {}",
        report.join(" | ")
    );
    for w in clients.windows(2) {
        assert_eq!(
            w[0].1, w[1].1,
            "M-100: разбиения {} и {} дали РАЗНЫЙ итог у клиента",
            w[0].0, w[1].0
        );
    }
}
