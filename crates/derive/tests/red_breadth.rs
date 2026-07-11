//! M-06 RED (sacred, architect) — C5: funding-breadth детерминирован и корректен.
//! Падает на STUB (нули) → research-dev делает GREEN. Анти-плацебо: два разных pct + n +
//! исключение инструмента без фандинга — хардкодом не пройдёшь.

use std::collections::BTreeMap;

use derive::{funding_breadth, Breadth};

#[test]
fn c5_funding_breadth_correct_and_deterministic() {
    let universe: Vec<String> = ["BTC", "ETH", "SOL", "XRP", "DOGE", "AVAX"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // 5 из universe имеют фандинг (AVAX — нет, значит в n НЕ входит).
    let mut f = BTreeMap::new();
    f.insert("BTC".to_string(), 100); // +
    f.insert("ETH".to_string(), 50); // +
    f.insert("SOL".to_string(), -30); // -
    f.insert("XRP".to_string(), -10); // -
    f.insert("DOGE".to_string(), 0); // flat

    let got = funding_breadth(&universe, &f);
    // n=5 (AVAX исключён); positive=2/5=40%, negative=2/5=40%, flat DOGE не считается.
    assert_eq!(
        got,
        Breadth {
            pct_positive_e8: 40_000_000,
            pct_negative_e8: 40_000_000,
            n: 5,
        }
    );

    // Детерминизм: повтор даёт тот же результат.
    assert_eq!(got, funding_breadth(&universe, &f));
}
