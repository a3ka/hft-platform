//! funding — чистый редьюсер (FA §3/§7): начисление по журнальным Md(Funding)
//! событиям и открытой позиции на момент начисления; без экстраполяции между
//! известными точками. Реализация — engine-dev (M-04 task 2).

/// PnL-дельта от funding-начисления, ×1e8 USD.
/// position_qty_e8 — знаковая позиция (long > 0); rate_e8 — ставка ×1e8;
/// mark_price_e8 — цена начисления. Long платит при положительной ставке (перп-конвенция).
pub fn funding_pnl_e8(position_qty_e8: i64, mark_price_e8: i64, rate_e8: i64) -> i64 {
    let _ = (position_qty_e8, mark_price_e8, rate_e8);
    todo!("engine-dev: M-04 task 2")
}
