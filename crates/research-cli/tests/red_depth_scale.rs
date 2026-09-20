//! RED DV-I-7/8 — ПРОД-МАСШТАБ / bounded-work оракулы для `analyze` и `consistency`
//! (sacred, architect). Урок TD-011 (`testing.md` «Прод-масштаб для sacred I/O-путей») → M-32.
//!
//! Инцидент 2026-07-24: прогон на 1 сегменте (1 GiB) НЕ завершился за 2 ЧАСА (99.9% CPU, RSS
//! 118 MB — чистый компьют, не OOM). Микро-фикстуры DV-I-1..6 (n=2..5) этого не поймали. Два
//! независимых O(n²):
//!   • `analyze` — `attribute_unborn` СКАНИТ ВЕСЬ ever-growing `states`-map КАЖДЫЙ тик
//!     (states не чистится: cancelled/censored остаются) ⇒ O(n·states), states растёт ⇒ O(n²).
//!   • `consistency` — пересборка running-книги С НУЛЯ на КАЖДУЮ сделку (`for prev in events[..i]`) ⇒ O(n²).
//! Оба замаскированы в юнитах капом distinct-цен; на реальном стакане (±60%, churn часами)
//! distinct-цен десятки тысяч → квадрат разворачивается.
//!
//! Контракт сложности (research-dev impl): ОБА — single-pass O(n) с ОГРАНИЧЕННОЙ работой на
//! событие. `analyze`: атрибуция полосы ПРИ РОЖДЕНИИ (O(1)), без per-tick full-scan states.
//! `consistency`: инкрементальная running-книга + оконный буфер pending-сделок, без rebuild.
//!
//! Анти-плацебо (доказано против impl инцидента): O(n²) НЕ укладывается в timeout → FAIL;
//! single-pass укладывается на 2+ порядка. compile-RED против отсутствия символов.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use contracts::{Level, Side};
use research_cli::depth_lifetime::{analyze, DeltaTick};
use research_cli::orderflow::{consistency, FaithEvent};

const UNIT: i64 = 100_000_000;
const MID: i64 = 64_000 * UNIT;
const BUDGET: Duration = Duration::from_secs(15);

/// Запустить `f` в отдельном потоке; паника, если не уложился в BUDGET (O(n²)-регресс).
fn assert_bounded<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let r = f();
        let _ = tx.send(r);
    });
    if rx.recv_timeout(BUDGET).is_err() {
        panic!(
            "{what}: не завершился за {}с — O(n²) регресс (per-item работа не ограничена). \
             Требуется single-pass O(n).",
            BUDGET.as_secs()
        );
    }
}

// ── DV-I-7: analyze на РАСТУЩИХ distinct-ценах (states→n) обязан быть bounded ─────────────────────
#[test]
fn dv_i_7_prodscale_analyze_bounded() {
    // Каждый тик: фикс near-bid/near-ask (стабильный mid) + УНИКАЛЬНЫЙ far-bid (никогда не
    // отменяется) ⇒ states растёт до n. O(n²)-attribute_unborn (full-scan states/тик) не уложится.
    let n = 120_000usize;
    let near_bid = (MID as f64 * 0.9995) as i64;
    let near_ask = (MID as f64 * 1.0005) as i64;
    let mut ticks = Vec::with_capacity(n);
    for i in 0..n {
        let far = (MID as f64 * 0.95) as i64 - (i as i64) * 100; // ~5% ниже mid, каждая уникальна
        ticks.push(DeltaTick {
            bids: vec![
                Level {
                    price: near_bid,
                    size: 20 * UNIT,
                },
                Level {
                    price: far,
                    size: 10 * UNIT,
                },
            ],
            asks: vec![Level {
                price: near_ask,
                size: 20 * UNIT,
            }],
            first_update_id: i as u64 + 1,
            final_update_id: i as u64 + 1,
            prev_final_update_id: None,
            ts_exch_ms: i as i64,
        });
    }
    assert_bounded(
        "analyze(120k растущих distinct-уровней)",
        move || analyze(&ticks).bands.len(),
    );
}

// ── DV-I-8: consistency на 400k сделок обязан быть bounded (без prefix-rebuild) ──────────────────
#[test]
fn dv_i_8_prodscale_consistency_bounded() {
    let n = 400_000usize;
    let mut ev = Vec::with_capacity(3 * n);
    for i in 0..n {
        let price = MID - (i as i64) * 100; // распределённые distinct-цены → книга растёт
        ev.push(FaithEvent::Delta {
            ts_ms: i as i64,
            bids: vec![Level {
                price,
                size: 100 * UNIT,
            }],
            asks: vec![],
        });
        ev.push(FaithEvent::Trade {
            ts_ms: i as i64,
            price,
            side: Side::Sell,
            size: UNIT,
        });
        ev.push(FaithEvent::Delta {
            ts_ms: i as i64 + 1,
            bids: vec![Level {
                price,
                size: 50 * UNIT,
            }],
            asks: vec![],
        });
    }
    assert_bounded("consistency(400k сделок)", move || {
        consistency(&ev, 1_000).checked
    });
}
