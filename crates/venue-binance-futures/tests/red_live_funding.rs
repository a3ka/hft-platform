//! M-06 RED (sacred, architect) — TD-014 T3: Funding не доходит до journal (§8 reject #4:
//! L2=470/OI=54/gap=stale=429≈0, но Funding=0 в live-window).
//!
//! КОРЕНЬ (диагностирован live-capture'ом architect'а + §8): агрегированный стрим `!markPrice@arr`
//! на combined endpoint (`/stream?streams=...@depth.../!markPrice@arr`) НЕ доставляется Binance'ом
//! вместе с per-symbol стримами (capture: markPrice=0 при depth=139). Session/parser-тесты это не
//! ловят — сообщение просто не приходит. Надёжный источник funding — PER-SYMBOL `<sym>@markPrice@1s`
//! (доставляется как одиночный объект в combined-обёртке).
//!
//! Этот оракул фиксирует ТРЕБОВАНИЕ, катчабельное юнитом: `on_ws_text` обязан эмитить Funding из
//! per-symbol формы `{"stream":"<sym>@markPrice@1s","data":{markPriceUpdate obj}}`, а не только из
//! array `!markPrice@arr`. Падает на 669ce40 (обрабатывается лишь exact `stream=="!markPrice@arr"`).
//! Диагностика (req §5): отдельные ассерты различают per-symbol-shape vs (пере)распознавание stream-name.
//!
//! ⚠ Юнит НЕ может воспроизвести exchange-non-delivery — окончательный гейт Funding>0 = reviewer §8 LIVE.

use contracts::{MdPayload, Venue};
use venue_binance_futures::{FuturesSession, SessionEffect};

fn fundings(e: &[SessionEffect]) -> usize {
    e.iter()
        .filter(|x| {
            matches!(x, SessionEffect::Emit(m)
                if m.venue == Venue::BinanceFutures && matches!(m.payload, MdPayload::Funding { .. }))
        })
        .count()
}

// Per-symbol combined-обёртка: stream="<sym>@markPrice@1s", data = ОДИНОЧНЫЙ markPriceUpdate объект.
const PER_SYMBOL_MP: &str = r#"{"stream":"btcusdt@markPrice@1s","data":{"e":"markPriceUpdate","E":1600,"s":"BTCUSDT","p":"64000.5","i":"64000.0","P":"64001.0","r":"0.00010000","T":9999999}}"#;

#[test]
fn td014_t3_funding_from_per_symbol_markprice_stream() {
    let mut s = FuturesSession::new(&["BTCUSDT".to_string()]);
    let e = s.on_ws_text(PER_SYMBOL_MP);
    assert_eq!(
        fundings(&e),
        1,
        "TD-014 T3: per-symbol `<sym>@markPrice@1s` (одиночный объект) обязан дать Funding — \
         combined `!markPrice@arr` не доставляется Binance'ом (live 0 Funding); нужен per-symbol источник"
    );
    // Регрессия: агрегированная array-форма тоже обязана работать (обе формы поддержаны).
    let arr = r#"{"stream":"!markPrice@arr","data":[{"e":"markPriceUpdate","E":1601,"s":"BTCUSDT","p":"64000.5","i":"64000.0","P":"64001.0","r":"0.00010000","T":9999999}]}"#;
    assert_eq!(
        fundings(&s.on_ws_text(arr)),
        1,
        "array-форма !markPrice@arr тоже эмитит Funding"
    );
}
