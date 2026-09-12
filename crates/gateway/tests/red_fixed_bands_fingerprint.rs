//! RED `M-84` (sacred, architect-only) — **фиксация набора делает отпечаток ОБЩИМ, и это
//! предъявляется ИМЕНЕМ СЛЕПКА, а не рассуждением.**
//!
//! COMPILE-RED: `gateway::CANONICAL_DEPTH_BANDS` ещё не существует.
//!
//! ## ЧТО ИМЕННО ДОКАЗЫВАЕТСЯ — и почему формулировка узкая
//!
//! `П-029` разбиралась четыре круга перепроверки (`R-179`) в том числе потому, что первая
//! редакция обещала «ОДИН расчёт на инструмент». Это НЕВЕРНО: отпечаток расщепляется по
//! ШЕСТИ осям (`crates/gateway/src/lib.rs:3449-3470`) — venue, symbol, `timeframe_ms`,
//! `window_ms`, полосы, `depth_cadence_ms`. Две из них (`timeframe_ms`, `window_ms`)
//! присылает КЛИЕНТ и после этой работы они остаются различающими.
//!
//! Поэтому здесь доказывается РОВНО ОДНО: ось полос перестаёт различать. Ни больше.
//! Тест `other_axes_still_split` держит границу утверждения — он ОБЯЗАН остаться зелёным,
//! и его зелёный есть признание предела, а не недоработка.
//!
//! ## `testing.md` чек-лист
//! - п.1 **асимметрия** — селекторы различаются ТОЛЬКО полосами, всё прочее совпадает;
//! - п.7 **ПАРНЫЙ vantage** — «полосы больше не различают» И «прочие оси различают».
//!   Одно без другого удовлетворяется заглушкой: отпечаток-константа прошёл бы первый тест.

use contracts::Venue;
use gateway::checkpoint::selector_fingerprint;
use gateway::Selector;

fn sel(bands: Vec<f64>, timeframe_ms: i64, window_ms: Option<i64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms,
        bands,
        window_ms,
        depth_cadence_ms: None,
    }
}

#[test]
fn band_axis_no_longer_splits_fingerprint() {
    // ПРЕЖНЯЯ РЕДАКЦИЯ ЭТОГО ТЕСТА БЫЛА ТАВТОЛОГИЕЙ (`C-220` B-1): она строила ДВА селектора,
    // оба уже канонические, и проверяла, что их отпечатки равны. Это свойство хеш-функции, а
    // не свойство решения: равенство выполнялось бы и при полностью неисправной фиксации.
    //
    // Здесь проверяется то, что действительно утверждает П-029: селекторы, различавшиеся
    // ПОЛОСАМИ, после фиксации перестают различаться. Эталон различия берётся ДО фиксации —
    // иначе сравнивать нечего.
    let anya_before = sel(vec![0.15], 1_000, Some(60_000));
    let boris_before = sel(vec![0.03, 0.15], 1_000, Some(60_000));
    assert_ne!(
        selector_fingerprint(&anya_before),
        selector_fingerprint(&boris_before),
        "SETUP НЕ СОСТОЯЛСЯ: ДО фиксации разные наборы обязаны давать разные отпечатки — \
         иначе предмет решения отсутствует и сходимость ниже ничего не доказывает"
    );

    let anya_after = sel(gateway::CANONICAL_DEPTH_BANDS.to_vec(), 1_000, Some(60_000));
    let boris_after = sel(gateway::CANONICAL_DEPTH_BANDS.to_vec(), 1_000, Some(60_000));
    assert_eq!(
        selector_fingerprint(&anya_after),
        selector_fingerprint(&boris_after),
        "после фиксации те же две сессии обязаны давать ОДИН отпечаток — иначе слепок не \
         переиспользуется и П-010 не получает даже необходимого условия"
    );
}

#[test]
fn canonical_and_legacy_single_band_differ() {
    // Инвалидация слепков — ЗАЯВЛЕННОЕ следствие (П-029, раздел цены), а не сюрприз.
    // Прод сегодня считает ОДНУ полосу (GATEWAY_BANDS=0.001); после фиксации отпечаток
    // ОБЯЗАН смениться, иначе старый слепок будет молча выдан за новый режим.
    let legacy = sel(vec![0.001], 1_000, Some(60_000));
    let canon = sel(gateway::CANONICAL_DEPTH_BANDS.to_vec(), 1_000, Some(60_000));
    assert_ne!(
        selector_fingerprint(&legacy),
        selector_fingerprint(&canon),
        "смена набора ОБЯЗАНА менять отпечаток: имя слепка есть отпечаток \
         (crates/gateway/src/lib.rs:3305-3314), и совпадение означало бы, что слепок старого \
         режима описывает новый — тихая ложь класса TD-019/TD-020"
    );
}

#[test]
fn other_axes_still_split() {
    // ГРАНИЦА УТВЕРЖДЕНИЯ. Этот тест обязан быть ЗЕЛЁНЫМ и обязан остаться таким: он не
    // даёт объявить «один расчёт на инструмент». Заодно это анти-плацебо: реализация,
    // сделавшая отпечаток константой, прошла бы первый тест и падает здесь.
    let canon = gateway::CANONICAL_DEPTH_BANDS.to_vec();
    let a = sel(canon.clone(), 1_000, Some(60_000));
    let b = sel(canon.clone(), 5_000, Some(60_000));
    let c = sel(canon, 1_000, Some(300_000));
    assert_ne!(
        selector_fingerprint(&a),
        selector_fingerprint(&b),
        "timeframe_ms обязан ПРОДОЛЖАТЬ различать: его присылает клиент, и решение M-84 его \
         не трогает. Если он перестал различать — сломана инвалидация чужого режима"
    );
    assert_ne!(
        selector_fingerprint(&a),
        selector_fingerprint(&c),
        "window_ms обязан ПРОДОЛЖАТЬ различать — тот же довод, что для timeframe_ms"
    );
}

#[test]
fn fingerprint_is_order_sensitive_within_bands() {
    // Почему набор обязан быть ИМЕННО последовательностью, а не множеством: отпечаток
    // хеширует значения по порядку (crates/gateway/src/lib.rs:3460). Переставленный набор —
    // другой слепок, и это причина, по которой канонический набор объявлен упорядоченным.
    let canon = gateway::CANONICAL_DEPTH_BANDS.to_vec();
    let mut shuffled = canon.clone();
    shuffled.swap(0, 1);
    assert_ne!(
        selector_fingerprint(&sel(canon, 1_000, Some(60_000))),
        selector_fingerprint(&sel(shuffled, 1_000, Some(60_000))),
        "порядок полос входит в отпечаток — значит канонический набор обязан быть \
         УПОРЯДОЧЕННОЙ последовательностью, а не множеством"
    );
}
