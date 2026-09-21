//! RED-оракулы portfolio (PF-I-1..4, docs/fa/strategy-brain.md §7). SACRED — architect-only.
//!
//! ⚠ Это pre-risk sanity, НЕ риск-гейт. Настоящий fail-closed барьер (`RK-I-1..10`,
//! `RiskApproved<Order>`) приходит в M-08 (`crates/risk`) и встаёт МЕЖДУ strategy и oms.
//! Зелёный PF-I-2 не означает «риск реализован».
//!
//! Анти-плацебо: PF-I-2 подаёт edge ВНЕ контракта (i64::MAX) — наивный
//! `edge·max/1e8` без clamp переполняется/превышает кап и падает.

use alpha::{Forecast, Instrument, EDGE_SCALE};
use contracts::Venue;
use portfolio::{size, Position, RiskBudget, TargetPosition};

fn btc() -> Instrument {
    Instrument::new(Venue::Binance, "BTCUSDT")
}
fn eth() -> Instrument {
    Instrument::new(Venue::Binance, "ETHUSDT")
}

fn fc(instrument: Instrument, edge_e8: i64) -> Forecast {
    Forecast {
        instrument,
        ts_mono_ns: 1_000_000_000,
        edge_e8,
        horizon_ms: 1_000,
        confidence_e8: EDGE_SCALE,
    }
}

/// max_position = 2.0 (×1e8).
fn budget_btc() -> RiskBudget {
    RiskBudget::new(vec![(btc(), 2 * EDGE_SCALE)]).expect("limit valid")
}

/// PF-I-1: сайзинг = clamp(edge · max_pos / 1e8, ±max_pos) — точные числа.
#[test]
fn pf_i_1_sizing_is_edge_times_limit() {
    let b = budget_btc();

    // edge = +0.5 → target = +1.0 (половина от лимита 2.0)
    let t = size(&[fc(btc(), 50_000_000)], &[], &b);
    assert_eq!(
        t,
        vec![TargetPosition {
            instrument: btc(),
            qty_e8: EDGE_SCALE
        }]
    );

    // edge = −0.25 → target = −0.5
    let t = size(&[fc(btc(), -25_000_000)], &[], &b);
    assert_eq!(t[0].qty_e8, -EDGE_SCALE / 2);

    // edge = 0 → target = 0 (мнение «нейтрально» ≠ отсутствие мнения; закрываемся)
    let t = size(&[fc(btc(), 0)], &[], &b);
    assert_eq!(t[0].qty_e8, 0);
}

/// PF-I-2 (FAIL-SAFE): |target| ≤ max_position ВСЕГДА — при любом, в т.ч. невалидном, edge.
/// Ни один вход не должен уметь выразить позицию больше капа.
#[test]
fn pf_i_2_target_never_exceeds_limit() {
    let b = budget_btc();
    let cap = 2 * EDGE_SCALE;

    for edge in [
        EDGE_SCALE,
        -EDGE_SCALE,
        10 * EDGE_SCALE,
        -10 * EDGE_SCALE,
        i64::MAX,
        i64::MIN,
        i64::MAX / 2,
    ] {
        let t = size(&[fc(btc(), edge)], &[], &b);
        assert_eq!(t.len(), 1, "форкаст с лимитом обязан дать target");
        assert!(
            t[0].qty_e8.abs() <= cap,
            "FAIL-SAFE НАРУШЕН: edge={edge} → target={} > cap={cap}",
            t[0].qty_e8
        );
    }

    // Граница достижима: edge = +1.0 → ровно кап.
    let t = size(&[fc(btc(), EDGE_SCALE)], &[], &b);
    assert_eq!(t[0].qty_e8, cap, "edge=+1.0 → ровно max_position");
}

/// PF-I-3: инструмент БЕЗ лимита в бюджете → target 0 (fail-closed, не «дефолтные лимиты» —
/// прямой урок risk_guard из hft-core-rs, DESIGN §9).
#[test]
fn pf_i_3_unknown_instrument_gets_zero_not_default_limit() {
    let b = budget_btc(); // лимит есть только на BTC

    let t = size(&[fc(eth(), EDGE_SCALE)], &[], &b);
    assert_eq!(t.len(), 1, "инструмент назван явно, а не молча пропущен");
    assert_eq!(t[0].instrument, eth());
    assert_eq!(
        t[0].qty_e8, 0,
        "нет лимита → нулевой target (НЕ default-лимит)"
    );
}

/// PF-I-4: позиция есть, форкаста нет (сигнал умер/протух) → target 0 = flatten.
/// Наивная реализация «итерируем только по форкастам» падает: инвентарь висел бы вечно.
#[test]
fn pf_i_4_held_position_without_forecast_is_flattened() {
    let b = budget_btc();
    let held = vec![Position {
        instrument: btc(),
        qty_e8: EDGE_SCALE, // +1.0 в позиции
    }];

    let t = size(&[], &held, &b);
    assert_eq!(t.len(), 1, "позиция без форкаста обязана попасть в выход");
    assert_eq!(t[0].instrument, btc());
    assert_eq!(t[0].qty_e8, 0, "нет мнения → выходим в ноль, а не держим");
}

/// Детерминизм + стабильный порядок выхода (сортировка по инструменту).
#[test]
fn pf_output_is_sorted_and_deterministic() {
    let b = RiskBudget::new(vec![(btc(), 2 * EDGE_SCALE), (eth(), EDGE_SCALE)]).expect("valid");
    let forecasts = vec![fc(eth(), 30_000_000), fc(btc(), -60_000_000)];
    let held = vec![Position {
        instrument: eth(),
        qty_e8: 0,
    }];

    let a = size(&forecasts, &held, &b);
    let c = size(&forecasts, &held, &b);
    assert_eq!(a, c, "чистая функция: одинаковый вход → одинаковый выход");
    assert_eq!(a.len(), 2);
    assert!(
        a[0].instrument < a[1].instrument,
        "выход обязан быть отсортирован по инструменту (детерминизм)"
    );
}

/// Конфиг бюджета fail-closed: лимит ≤ 0 и дубли — Err, не «нулевой лимит по умолчанию».
#[test]
fn pf_budget_validation_is_fail_closed() {
    assert!(RiskBudget::new(vec![(btc(), 0)]).is_err(), "лимит 0 → Err");
    assert!(
        RiskBudget::new(vec![(btc(), -EDGE_SCALE)]).is_err(),
        "отрицательный лимит → Err"
    );
    assert!(
        RiskBudget::new(vec![(btc(), EDGE_SCALE), (btc(), 2 * EDGE_SCALE)]).is_err(),
        "дубль лимита → Err (двусмысленный конфиг)"
    );
}
