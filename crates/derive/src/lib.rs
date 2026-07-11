//! derive — чистые детерминированные функции-производные над T1 (journal-first).
//! Никакого I/O/wall-clock/rand. M-06 SKELETON (architect): сигнатуры + RED; impl — research-dev.

use std::collections::BTreeMap;

/// Разрез фандинга по вселенной инструментов (market breadth).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breadth {
    /// Доля инструментов с положительным фандингом, ×1e8 (0..1e8).
    pub pct_positive_e8: i64,
    /// Доля с отрицательным фандингом, ×1e8.
    pub pct_negative_e8: i64,
    /// Число инструментов вселенной, по которым ЕСТЬ фандинг (знаменатель).
    pub n: u32,
}

/// Breadth фандинга по `universe` (напр. top-300 по OI — отбирается выше).
/// `latest_funding` — последняя ставка ×1e8 на инструмент. Инструменты вселенной без
/// записи в `latest_funding` в `n` НЕ входят. rate>0 → positive, rate<0 → negative,
/// rate==0 → ни то ни другое (flat). Детерминировано (порядок не влияет).
pub fn funding_breadth(universe: &[String], latest_funding: &BTreeMap<String, i64>) -> Breadth {
    // M-06 task 5 (research-dev). Чистая детерминированная функция: без I/O/wall-clock/rand.
    // Идём по `universe` в его порядке (детерминизм входа) и смотрим наличие+знак ставки.
    // rate>0 → positive; rate<0 → negative; rate==0 → flat (n учитывается, знак — нет).
    // Инструмент в universe без записи в `latest_funding` в n НЕ входит (semantics спекa).
    let mut n: u32 = 0;
    let mut positive: u32 = 0;
    let mut negative: u32 = 0;
    for sym in universe {
        if let Some(&rate) = latest_funding.get(sym) {
            n += 1;
            match rate {
                r if r > 0 => positive += 1,
                r if r < 0 => negative += 1,
                _ => {} // flat: учтён в n, ни +, ни −.
            }
        }
    }
    // Доля ×1e8, целочисленно (round-toward-zero). n==0 → 0% (нет данных — не NaN, не паника).
    let pct_e8 = |c: u32| -> i64 {
        if n == 0 {
            0
        } else {
            (c as i64).saturating_mul(100_000_000) / n as i64
        }
    };
    Breadth {
        pct_positive_e8: pct_e8(positive),
        pct_negative_e8: pct_e8(negative),
        n,
    }
}
