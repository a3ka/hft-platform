//! RED DV-I-7 — ПРОД-МАСШТАБ / bounded-work оракул для `orderflow::consistency` (sacred, architect).
//!
//! Урок TD-011 (`testing.md` «Прод-масштаб для sacred I/O-путей») применён к M-32: микро-фикстуры
//! DV-I-6 проверяют КОРРЕКТНОСТЬ, но НЕ СЛОЖНОСТЬ. Инцидент 2026-07-24: наивный `consistency`
//! пересобирал running-книгу С НУЛЯ на КАЖДУЮ сделку (`for prev in &events[..i]`) → O(n²) → 2 часа
//! на 1 сегменте (99.9% CPU, RSS 118 MB — не OOM, чистый компьют). Зелёные DV-I-1..6 этого не
//! поймали (n=2..5). Этот оракул ПАДАЕТ на O(n²), проходит на single-pass O(n·window).
//!
//! Контракт сложности (research-dev impl): `consistency` — ОДИН forward-проход; running-книга
//! поддерживается ИНКРЕМЕНТАЛЬНО (не пересобирается на сделку); ожидающие сделки резолвятся из
//! оконного буфера. Никакого `events[..i]`-rebuild и никакого повторного полного скана префикса.
//!
//! Анти-плацебо: O(n²)-реализация не укладывается в timeout → FAIL; O(n) укладывается с запасом
//! на 3+ порядка. compile-RED против отсутствия символа.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use contracts::{Level, Side};
use research_cli::orderflow::{consistency, FaithEvent};

const UNIT: i64 = 100_000_000;
const BASE: i64 = 64_000 * UNIT;
const TICK: i64 = 100_000; // шаг цены

/// Детерминированный синтетический поток (без rand — вариация по индексу): на каждый шаг
/// Delta(set)→Trade→Delta(decrement) на «плавающей» цене. `n` шагов ⇒ 3·n событий.
fn synth(n: usize) -> Vec<FaithEvent> {
    let mut ev = Vec::with_capacity(3 * n);
    for i in 0..n {
        let ts = i as i64; // 1 «мс» на шаг ⇒ окно 1000мс покрывает ~1000 шагов (плотное окно)
        let price = BASE + ((i % 500) as i64) * TICK;
        let d = FaithEvent::Delta {
            ts_ms: ts,
            bids: vec![Level {
                price,
                size: 100 * UNIT,
            }],
            asks: vec![],
        };
        let t = FaithEvent::Trade {
            ts_ms: ts,
            price,
            side: Side::Sell,
            size: (1 + (i % 5) as i64) * UNIT,
        };
        let dec = FaithEvent::Delta {
            ts_ms: ts + 1,
            bids: vec![Level {
                price,
                size: 50 * UNIT,
            }],
            asks: vec![],
        };
        ev.push(d);
        ev.push(t);
        ev.push(dec);
    }
    ev
}

// ── DV-I-7: 150k шагов (450k событий) обязаны обработаться за bounded-время (single-pass O(n)) ───
#[test]
fn dv_i_7_prodscale_consistency_is_bounded() {
    let events = synth(150_000);
    let (tx, rx) = mpsc::channel();
    // Отдельный поток: O(n²)-реализация НЕ завершится в timeout → recv_timeout ловит регресс.
    thread::spawn(move || {
        let r = consistency(&events, 1_000);
        let _ = tx.send(r.checked);
    });
    match rx.recv_timeout(Duration::from_secs(20)) {
        Ok(checked) => {
            assert_eq!(checked, 150_000, "проверены все сделки (single-pass прошёл)");
        }
        Err(_) => panic!(
            "consistency не завершился за 20с на 450k событий — O(n²) регресс \
             (пересборка книги на сделку / полный скан префикса). Требуется single-pass O(n)."
        ),
    }
}
