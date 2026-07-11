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
pub fn funding_breadth(_universe: &[String], _latest_funding: &BTreeMap<String, i64>) -> Breadth {
    // STUB — research-dev (M-06 task 5). RED: crates/derive/tests/red_breadth.rs
    Breadth {
        pct_positive_e8: 0,
        pct_negative_e8: 0,
        n: 0,
    }
}
