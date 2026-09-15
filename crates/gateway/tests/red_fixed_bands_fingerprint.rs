//! **ЗЕЛЁНЫЙ СТОРОЖ, не RED** (`A-033`) — свойство «полосы входят в отпечаток» верно УЖЕ
//! СЕГОДНЯ, и M-84 на нём стоит. Файл обязан быть зелёным ДО и ПОСЛЕ реализации; покраснеет —
//! значит из-под решения выбили опору.
//!
//! Тест сходимости («два клиента дают ОДИН отпечаток») отсюда УБРАН: в этом крейте он
//! сравнивал два селектора, которые тест сам же и строит одинаковыми, — тавтология, которую
//! `C-220` B-1 уже ловил. Сходимость наблюдаема только на пути КЛИЕНТА и живёт в
//! `crates/gateway-serve/tests/red_fixed_bands_entrypoint.rs`.

use contracts::Venue;
use gateway::checkpoint::selector_fingerprint;
use gateway::Selector;

/// Продуктовый набор — ЛИТЕРАЛОМ, а не ссылкой на `gateway::CANONICAL_DEPTH_BANDS`.
/// `A-033`: тест, не собирающийся из-за отсутствующего символа, глушит ВЕСЬ корпус крейта —
/// именно так три круга не увидели 209 красных из 329. Исполняемый RED строго сильнее
/// (прецедент: `red_grace_from_env.rs:12-15`, `red_heatmap_window_env.rs:19-30`).
/// Принадлежность константы крейту держит шаг гейта, а не ссылка отсюда.
const PRODUCT_BANDS: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

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
fn canonical_and_legacy_single_band_differ() {
    // Инвалидация слепков — ЗАЯВЛЕННОЕ следствие (П-029, раздел цены), а не сюрприз.
    // Прод сегодня считает ОДНУ полосу (GATEWAY_BANDS=0.001); после фиксации отпечаток
    // ОБЯЗАН смениться, иначе старый слепок будет молча выдан за новый режим.
    let legacy = sel(vec![0.001], 1_000, Some(60_000));
    let canon = sel(PRODUCT_BANDS.to_vec(), 1_000, Some(60_000));
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
    let canon = PRODUCT_BANDS.to_vec();
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
    let canon = PRODUCT_BANDS.to_vec();
    let mut shuffled = canon.clone();
    shuffled.swap(0, 1);
    assert_ne!(
        selector_fingerprint(&sel(canon, 1_000, Some(60_000))),
        selector_fingerprint(&sel(shuffled, 1_000, Some(60_000))),
        "порядок полос входит в отпечаток — значит канонический набор обязан быть \
         УПОРЯДОЧЕННОЙ последовательностью, а не множеством"
    );
}
