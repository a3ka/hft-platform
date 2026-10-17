//! RED-оракулы alpha (AL-I-1..5, docs/fa/strategy-brain.md §7). SACRED — architect-only.
//!
//! Анти-плацебо: каждый тест ПАДАЕТ на наивной реализации, не только на `todo!()`:
//! - AL-I-2 — конкретная арифметика весов (наивное «сложить values» даёт другое число);
//! - AL-I-3 — «просуммировать всё, что пришло» даёт неверный edge;
//! - AL-I-4 — «последнее значение живёт вечно» → форкаст есть там, где его быть не должно;
//! - AL-I-5 — «edge = value» без clamp → выход за ±1e8.

use alpha::{Alpha, Forecast, Instrument, LinearAlpha, SignalWeight, EDGE_SCALE};
use contracts::{Event, EventKind, Level, MdPayload, Venue};
use signals::{RegistryStatus, SignalId, SignalMeta, SignalOut};

const MS: u64 = 1_000_000;

fn instrument() -> Instrument {
    Instrument::new(Venue::Binance, "BTCUSDT")
}

fn ev(seq: u64, ts_mono_ns: u64) -> Event {
    Event {
        seq,
        ts_mono_ns,
        ts_wall_ms: 1_752_000_000_000 + seq as i64,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![Level {
                    price: contracts::to_fixed(100.0),
                    size: contracts::to_fixed(1.0),
                }],
                asks: vec![Level {
                    price: contracts::to_fixed(101.0),
                    size: contracts::to_fixed(1.0),
                }],
                ts_exch_ms: 1_752_000_000_000,
            },
        ),
    }
}

fn out(id: &str, ts_mono_ns: u64, value_e8: i64, horizon_ms: i64) -> SignalOut {
    SignalOut {
        signal_id: SignalId::parse(id).expect("valid signal id"),
        ts_event_mono_ns: ts_mono_ns,
        value: value_e8,
        status: RegistryStatus::Candidate,
        meta: SignalMeta { horizon_ms },
    }
}

fn weight(id: &str, weight_e8: i64) -> SignalWeight {
    SignalWeight {
        signal_id: SignalId::parse(id).expect("valid signal id"),
        instrument: instrument(),
        weight_e8,
    }
}

/// AL-I-2: комбинация двух сигналов с весами — ТОЧНАЯ арифметика.
/// w1=1e8 (v=+0.6e8), w2=3e8 (v=-0.2e8): edge = (1·0.6 + 3·(−0.2)) / (1+3) = 0.0e8.
/// Наивное «среднее значений» дало бы +0.2e8 → тест падает. Проверяем и ненулевой кейс.
#[test]
fn al_i_2_weighted_combination_is_exact() {
    let mut a = LinearAlpha::new(vec![
        weight("S-001-obi-asym", EDGE_SCALE),
        weight("S-002-lead-lag", 3 * EDGE_SCALE),
    ])
    .expect("weights valid");

    let e = ev(1, 1_000 * MS);
    let f = a.update(
        &e,
        &[
            out("S-001-obi-asym", e.ts_mono_ns, 60_000_000, 5_000),
            out("S-002-lead-lag", e.ts_mono_ns, -20_000_000, 5_000),
        ],
    );
    assert_eq!(f.len(), 1, "один инструмент → один форкаст");
    assert_eq!(f[0].edge_e8, 0, "(1·0.6 + 3·(−0.2))/4 = 0");
    assert_eq!(f[0].horizon_ms, 5_000, "horizon = max по участвующим");
    assert_eq!(
        f[0].confidence_e8, EDGE_SCALE,
        "весь вес живой → confidence = 1.0"
    );

    // Второй кейс: w1=3e8 (+0.4), w2=1e8 (−0.4) → (3·0.4 − 1·0.4)/4 = +0.2
    let mut b = LinearAlpha::new(vec![
        weight("S-001-obi-asym", 3 * EDGE_SCALE),
        weight("S-002-lead-lag", EDGE_SCALE),
    ])
    .expect("weights valid");
    let e2 = ev(2, 2_000 * MS);
    let g = b.update(
        &e2,
        &[
            out("S-001-obi-asym", e2.ts_mono_ns, 40_000_000, 1_000),
            out("S-002-lead-lag", e2.ts_mono_ns, -40_000_000, 3_000),
        ],
    );
    assert_eq!(g[0].edge_e8, 20_000_000, "(3·0.4 − 1·0.4)/4 = +0.2");
    assert_eq!(g[0].horizon_ms, 3_000, "max(1000, 3000)");
}

/// AL-I-3: `SignalOut` с сигналом, которого нет в весах ансамбля, НЕ влияет на edge
/// (fail-closed: неизвестное не двигает деньги — зеркало RK-I-3).
#[test]
fn al_i_3_unknown_signal_id_is_ignored() {
    let mut a = LinearAlpha::new(vec![weight("S-001-obi-asym", EDGE_SCALE)]).expect("valid");

    let e = ev(1, 1_000 * MS);
    let with_stranger = a.update(
        &e,
        &[
            out("S-001-obi-asym", e.ts_mono_ns, 50_000_000, 1_000),
            out("S-099-unknown", e.ts_mono_ns, -100_000_000, 1_000),
        ],
    );
    assert_eq!(with_stranger.len(), 1);
    assert_eq!(
        with_stranger[0].edge_e8, 50_000_000,
        "чужой сигнал не входит в комбинацию"
    );

    let mut b = LinearAlpha::new(vec![weight("S-001-obi-asym", EDGE_SCALE)]).expect("valid");
    let solo = b.update(
        &e,
        &[out("S-001-obi-asym", e.ts_mono_ns, 50_000_000, 1_000)],
    );
    assert_eq!(with_stranger, solo, "чужой сигнал не меняет НИЧЕГО");
}

/// AL-I-4: stale-expiry. Сэмпл живёт ровно `horizon_ms` (event-time). Протухшие выпадают;
/// все протухли → форкаста НЕТ (отсутствие мнения ≠ мнение «ноль»).
/// Наивный «последний value навсегда» падает на шаге t=3000ms.
#[test]
fn al_i_4_stale_sample_expires_by_horizon() {
    let mut a = LinearAlpha::new(vec![
        weight("S-001-obi-asym", EDGE_SCALE),
        weight("S-002-lead-lag", EDGE_SCALE),
    ])
    .expect("valid");

    // t=1000ms: оба сигнала, horizon 1000ms и 5000ms.
    let e1 = ev(1, 1_000 * MS);
    let f1 = a.update(
        &e1,
        &[
            out("S-001-obi-asym", e1.ts_mono_ns, 80_000_000, 1_000),
            out("S-002-lead-lag", e1.ts_mono_ns, -40_000_000, 5_000),
        ],
    );
    assert_eq!(f1[0].edge_e8, 20_000_000, "(0.8 − 0.4)/2");
    assert_eq!(f1[0].confidence_e8, EDGE_SCALE);

    // t=3000ms: новых выходов нет. S-001 (horizon 1000ms) ПРОТУХ, S-002 (5000ms) жив.
    let e2 = ev(2, 3_000 * MS);
    let f2 = a.update(&e2, &[]);
    assert_eq!(f2.len(), 1, "живой сигнал ещё даёт форкаст");
    assert_eq!(f2[0].edge_e8, -40_000_000, "остался только S-002");
    assert_eq!(
        f2[0].confidence_e8,
        EDGE_SCALE / 2,
        "жив 1 из 2 равных весов → confidence 0.5"
    );

    // t=7000ms: протухли оба → форкаста НЕТ (не edge=0).
    let e3 = ev(3, 7_000 * MS);
    let f3: Vec<Forecast> = a.update(&e3, &[]);
    assert!(
        f3.is_empty(),
        "все сэмплы протухли → форкаста нет вовсе (не «edge = 0»)"
    );
}

/// AL-I-5: edge зажат в ±1e8 при мусорном value (сигнал вне контракта D1) — и без
/// переполнения i64 (арифметика i128). Наивный «edge = Σw·v/Σ|w|» без clamp падает.
#[test]
fn al_i_5_edge_is_clamped_and_overflow_safe() {
    let mut a = LinearAlpha::new(vec![weight("S-001-obi-asym", EDGE_SCALE)]).expect("valid");
    let e = ev(1, 1_000 * MS);

    let f = a.update(&e, &[out("S-001-obi-asym", e.ts_mono_ns, i64::MAX, 1_000)]);
    assert_eq!(f[0].edge_e8, EDGE_SCALE, "мусорный +∞ зажат в +1.0");
    assert!((0..=EDGE_SCALE).contains(&f[0].confidence_e8));

    let mut b = LinearAlpha::new(vec![weight("S-001-obi-asym", i64::MAX)]).expect("valid");
    let g = b.update(&e, &[out("S-001-obi-asym", e.ts_mono_ns, i64::MIN, 1_000)]);
    assert_eq!(g[0].edge_e8, -EDGE_SCALE, "мусорный −∞ зажат в −1.0");
}

/// AL-I-1: детерминизм — один и тот же поток событий/сигналов, два независимых прогона →
/// побайтово идентичные форкасты (DESIGN §1).
#[test]
fn al_i_1_replay_is_deterministic() {
    let run = || -> Vec<Forecast> {
        let mut a = LinearAlpha::new(vec![
            weight("S-001-obi-asym", EDGE_SCALE),
            weight("S-002-lead-lag", 2 * EDGE_SCALE),
        ])
        .expect("valid");
        let mut all = Vec::new();
        for i in 1..=50u64 {
            let e = ev(i, i * 100 * MS);
            let value = ((i as i64 * 7) % 200 - 100) * 1_000_000; // детерминированная «пила»
            let outs = if i % 3 == 0 {
                vec![out("S-001-obi-asym", e.ts_mono_ns, value, 500)]
            } else if i % 5 == 0 {
                vec![out("S-002-lead-lag", e.ts_mono_ns, -value, 1_500)]
            } else {
                vec![]
            };
            all.extend(a.update(&e, &outs));
        }
        all
    };

    let a = run();
    let b = run();
    assert!(!a.is_empty(), "прогон обязан что-то произвести");
    assert_eq!(a, b, "DET: два прогона одного потока обязаны совпасть");
}

/// Валидация конфига ансамбля fail-closed: пустые веса / нулевой вес / дубль — Err,
/// а не «молча проигнорируем».
#[test]
fn al_config_validation_is_fail_closed() {
    assert!(LinearAlpha::new(vec![]).is_err(), "пустой ансамбль → Err");
    assert!(
        LinearAlpha::new(vec![weight("S-001-obi-asym", 0)]).is_err(),
        "нулевой вес → Err (не «сигнал без влияния»)"
    );
    assert!(
        LinearAlpha::new(vec![
            weight("S-001-obi-asym", EDGE_SCALE),
            weight("S-001-obi-asym", EDGE_SCALE),
        ])
        .is_err(),
        "дубль (signal_id, instrument) → Err"
    );
}
