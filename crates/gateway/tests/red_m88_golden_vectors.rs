//! RED M-88 (sacred, architect-only) — ЭТАЛОННЫЕ ВЕКТОРЫ для проверки РЕАЛЬНОГО клиента.
//!
//! Обязательство `A-036` §4.3 п.1. Оракул фронта отложен ВНЕ `M-88` законно — но только при
//! условии, что поставка отдаёт МАТЕРИАЛ, которым фронт можно проверить, не имея его кода в
//! этом репозитории. Материал — замороженные wire-байты плюс ожидаемое конечное состояние по
//! каждому из семи сценариев §9 спеки и четырёх случаев восстановления §9.1bis.
//!
//! ## Почему сверка ДВУСТОРОННЯЯ
//!
//! Вектор, который никто не перепроверяет, протухает МОЛЧА: производитель меняется, байты
//! остаются, и фронт проверяется против прошлогодней формы. Поэтому оракул требует ОБОЕГО:
//!
//! 1. **производитель воспроизводит замороженные байты** — сегодняшний `frames_since` +
//!    `serde_json` дают ровно то, что лежит в фикстуре;
//! 2. **модель сходится на них с ожиданием** — применение замороженных байтов к
//!    замороженному снимку даёт замороженное конечное состояние.
//!
//! Первое ловит дрейф формы, второе — дрейф семантики. По отдельности каждое зелено при
//! сломанном соседе.
//!
//! ## Что вектор НЕ доказывает
//!
//! Что фронт его прошёл. Это внешняя зависимость с владельцем (founder) и критерием
//! (`7 + 4` вектора), названная в спеке §9.1bis. Здесь — только материал и его годность.
//!
//! ## Как векторы порождаются
//!
//! `M88_WRITE_VECTORS=1 cargo test -p gateway --test red_m88_golden_vectors` — режим ЗАПИСИ,
//! запускается architect'ом ПОСЛЕ GREEN dev'а (спека §12 шаг 2). Без переменной — режим
//! СВЕРКИ, и отсутствие фикстур есть ОТКАЗ, а не пропуск: молчаливый скип превратил бы
//! обязательство в декорацию.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};
use std::path::PathBuf;

const T: i64 = 1_752_000_010_000;

/// Семь сценариев §9 спеки плюс четыре случая восстановления §9.1bis.
/// Имя = имя каталога фикстуры; менять нельзя — на него ссылается внешняя проверка фронта.
const VECTORS: [&str; 11] = [
    "s1_removed_in_current_bucket",
    "s2_closed_past_survives",
    "s3_empty_full_slice",
    "s4_absent_observation",
    "s5_price_leaves_window",
    "s6_duplicate_frame",
    "s7_stale_frame",
    "r1_recover_after_out_of_order",
    "r2_clear_on_empty_slice",
    "r3_keep_on_absent_observation",
    "r4_reject_foreign_schema",
];

fn vectors_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("m88")
}

fn writing() -> bool {
    std::env::var("M88_WRITE_VECTORS").is_ok_and(|v| v == "1")
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m88-vectors".to_string(),
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

fn trade_ev(price: f64, size: f64, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side: Side::Buy,
            ts_exch_ms: ts,
        },
    )
}

/// Поток событий каждого вектора. Один источник правды: и запись, и сверка берут его отсюда,
/// поэтому вектор не может разойтись со сценарием, который он представляет.
fn events_for(name: &str) -> Vec<EventKind> {
    match name {
        "s1_removed_in_current_bucket" | "r1_recover_after_out_of_order" => vec![
            snapshot_ev(
                &[(64_990.0, 5.0), (64_980.0, 3.0)],
                &[(65_010.0, 4.0), (65_020.0, 1.0)],
                T,
            ),
            delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 1),
        ],
        "s2_closed_past_survives" => vec![
            snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
            snapshot_ev(&[(64_995.0, 7.0)], &[(65_005.0, 2.0)], T + 1_000),
        ],
        "s3_empty_full_slice" | "r2_clear_on_empty_slice" | "r4_reject_foreign_schema" => vec![
            snapshot_ev(&[(64_990.0, 5.0), (64_980.0, 3.0)], &[(65_010.0, 4.0)], T),
            delta_ev(
                &[(64_990.0, 0.0), (64_980.0, 0.0)],
                &[(65_010.0, 0.0)],
                1,
                2,
                T + 1,
            ),
        ],
        "s4_absent_observation" | "r3_keep_on_absent_observation" => vec![
            snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
            trade_ev(65_000.0, 1.0, T + 10),
        ],
        "s5_price_leaves_window" => vec![
            snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
            delta_ev(&[(65_400.0, 6.0)], &[(65_420.0, 6.0)], 1, 2, T + 1),
        ],
        "s6_duplicate_frame" | "s7_stale_frame" => vec![
            snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
            trade_ev(65_000.0, 1.5, T + 10),
            delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 30),
        ],
        other => panic!(
            "вектор '{other}' не имеет потока событий — состав VECTORS разошёлся с events_for"
        ),
    }
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

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Байты вектора: снимок на первом событии, кадры после него, конечное состояние — полным
/// пересчётом. Форма — ровно та, что уходит на провод (`serde_json`).
fn produce(name: &str) -> (Vec<u8>, Vec<Vec<u8>>, Vec<u8>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut seqs = Vec::new();
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events_for(name) {
            seqs.push(j.append(e).expect("append").seq);
        }
        j.flush().expect("flush");
    }
    let s = sel();
    let base = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::at(seqs[0]),
    )
    .expect("snapshot base");
    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot full");

    let mut cur = Cursor::at(seqs[0]);
    let mut frames = Vec::new();
    loop {
        let (batch, next) =
            gateway::frames_since(dir.path(), EpochFilter::OwnCaptureOnly, &s, cur, 1)
                .expect("frames");
        if batch.is_empty() {
            break;
        }
        for f in &batch {
            frames.push(serde_json::to_vec(f).expect("frame → wire"));
        }
        cur = next;
    }
    (
        serde_json::to_vec(&base).expect("snapshot → wire"),
        frames,
        serde_json::to_vec(&full.series).expect("expected → wire"),
    )
}

/// Состав векторов и состав сценариев НЕ РАСХОДЯТСЯ: каждый из одиннадцати имеет поток.
#[test]
fn vectors_cover_seven_scenarios_and_four_recoveries() {
    let _g = serial();
    assert_eq!(
        VECTORS.len(),
        11,
        "векторов обязано быть 7 сценариев + 4 восстановления"
    );
    for name in VECTORS {
        let ev = events_for(name);
        assert!(
            ev.len() >= 2,
            "вектор '{name}': поток из {} событий не описывает сценарий",
            ev.len()
        );
    }
}

/// СВЕРКА (или ЗАПИСЬ по `M88_WRITE_VECTORS=1`). Отсутствие фикстур — ОТКАЗ, не пропуск.
#[test]
fn golden_vectors_match_producer_and_expectation() {
    let _g = serial();
    let root = vectors_root();
    let mut missing = Vec::new();

    for name in VECTORS {
        let dir = root.join(name);
        let (snap, frames, expected) = produce(name);

        if writing() {
            std::fs::create_dir_all(&dir).expect("create vector dir");
            std::fs::write(dir.join("snapshot.json"), &snap).expect("write snapshot");
            std::fs::write(dir.join("expected_series.json"), &expected).expect("write expected");
            for (i, f) in frames.iter().enumerate() {
                std::fs::write(dir.join(format!("frame_{i:02}.json")), f).expect("write frame");
            }
            continue;
        }

        if !dir.join("snapshot.json").exists() {
            missing.push(name);
            continue;
        }

        // (1) производитель воспроизводит замороженные байты — ловит дрейф ФОРМЫ.
        let frozen_snap = std::fs::read(dir.join("snapshot.json")).expect("read snapshot");
        assert_eq!(
            frozen_snap, snap,
            "вектор '{name}': производитель больше не воспроизводит замороженный снимок — \
             форма провода уехала, и внешняя проверка фронта идёт против прошлой версии"
        );
        for (i, f) in frames.iter().enumerate() {
            let p = dir.join(format!("frame_{i:02}.json"));
            let frozen = std::fs::read(&p).unwrap_or_else(|_| panic!("нет кадра {}", p.display()));
            assert_eq!(
                frozen, *f,
                "вектор '{name}': кадр {i} разошёлся с замороженным"
            );
        }

        // (2) ожидание совпадает с полным пересчётом — ловит дрейф СЕМАНТИКИ производителя.
        let frozen_expected =
            std::fs::read(dir.join("expected_series.json")).expect("read expected");
        assert_eq!(
            frozen_expected, expected,
            "вектор '{name}': ожидаемое конечное состояние разошлось с полным пересчётом"
        );

        // (3) МОДЕЛЬ ПОТРЕБИТЕЛЯ СХОДИТСЯ НА ЗАМОРОЖЕННЫХ БАЙТАХ (`C-237` R1).
        //
        // Без этой половины «двусторонняя сверка» была односторонней, и гейт справедливо
        // это отверг: пункты (1) и (2) сравнивают производителя с собой и ожидание с
        // пересчётом, но НИ ОДИН не применяет замороженные кадры через потребителя.
        // Вектор, на котором потребитель не сходится, такую проверку проходил бы — то есть
        // материал для внешней проверки фронта был бы негоден, оставаясь зелёным.
        //
        // Здесь замороженный СНИМОК и замороженные КАДРЫ проходят ровно тот путь, который
        // пройдёт фронт: разбор проводных байтов → применение → сравнение конечного
        // состояния с замороженным ожиданием.
        let mut model: gateway::Snapshot =
            serde_json::from_slice(&frozen_snap).expect("frozen snapshot → model");
        for i in 0..frames.len() {
            let fp = dir.join(format!("frame_{i:02}.json"));
            let bytes = std::fs::read(&fp).unwrap_or_else(|_| panic!("нет кадра {}", fp.display()));
            let frame: gateway::Frame =
                serde_json::from_slice(&bytes).expect("frozen frame → model");
            let outcome = model.apply(&frame);
            assert_eq!(
                outcome,
                gateway::ApplyOutcome::Applied,
                "вектор '{name}': кадр {i} отвергнут моделью ({outcome:?}) — замороженная \
                 последовательность не применима потребителем"
            );
        }
        let model_series = serde_json::to_vec(&model.series).expect("model → wire");
        assert_eq!(
            model_series, frozen_expected,
            "вектор '{name}': МОДЕЛЬ ПОТРЕБИТЕЛЯ не сошлась с замороженным ожиданием. \
             Материал внешней проверки фронта негоден: мы отдали бы байты, на которых наш \
             собственный потребитель даёт другое состояние"
        );
    }

    assert!(
        missing.is_empty() || writing(),
        "эталонных векторов НЕТ для: {missing:?}. Материал внешней проверки фронта (A-036 \
         §4.3 п.1) не предъявлен. Породить: M88_WRITE_VECTORS=1 cargo test -p gateway \
         --test red_m88_golden_vectors — architect, ПОСЛЕ GREEN dev'а (спека §12 шаг 2)"
    );
}
