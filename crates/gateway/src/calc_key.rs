//! M-100 (`milestones/M-100-three-serving-contracts.md` §4.3, `CT-RFC-09` §2.10.4) —
//! стабильный идентификатор ключа расчёта. Заменяет `DefaultHasher`-отпечаток
//! (`selector_fingerprint`), зависящий от toolchain.
//!
//! Каноническая форма — UTF-8, ровно семь строк, каждая с `\n`:
//!   `calc-key/v1` · `venue=<имя варианта>` · `symbol=<символ>` ·
//!   `timeframe_ms=<i64>` · `window_ms=<i64|none>` · `depth_cadence_ms=<i64|none>` ·
//!   `bands_pct_e8=<round(b·1e8) через запятую, в порядке селектора>`
//!
//! `calc_key_id = sha256(canonical_calc_key_bytes)`, 64 hex символа нижнего регистра.
//! Форма независима от Rust-рантайма — независимая сторона (`hashlib.sha256` в Python)
//! выдаёт тот же результат байт-в-байт (см. `crates/gateway/tests/red_m100_calc_key_id.rs`,
//! золотые значения посчитаны вне `gateway`).

use sha2::{Digest, Sha256};

use contracts::Venue;

use super::Selector;

/// Версия кодирования канонической формы. Меняется ТОЛЬКО при смене САМОЙ формы
/// (например, добавлении новой оси в ключ). Сама строка `"calc-key/v1"` ВХОДИТ в
/// канонический текст — смена версии = смена содержимого, и `sha256` расходится
/// автоматически.
pub const CALC_KEY_ENCODING: &str = "calc-key/v1";

/// Каноническая UTF-8-форма ключа расчёта — СЕМЬ строк, каждая с `\n` (включая
/// последнюю). Независима от порядка вывода `Debug`/`Display` типов Rust.
pub fn canonical_calc_key_bytes(sel: &Selector) -> Vec<u8> {
    let mut out = String::with_capacity(256);
    out.push_str(CALC_KEY_ENCODING);
    out.push('\n');
    out.push_str("venue=");
    out.push_str(venue_canonical(sel.venue));
    out.push('\n');
    out.push_str("symbol=");
    out.push_str(&sel.symbol);
    out.push('\n');
    out.push_str("timeframe_ms=");
    out.push_str(&sel.timeframe_ms.to_string());
    out.push('\n');
    out.push_str("window_ms=");
    match sel.window_ms {
        Some(w) => out.push_str(&w.to_string()),
        None => out.push_str("none"),
    }
    out.push('\n');
    out.push_str("depth_cadence_ms=");
    match sel.depth_cadence_ms {
        Some(c) => out.push_str(&c.to_string()),
        None => out.push_str("none"),
    }
    out.push('\n');
    out.push_str("bands_pct_e8=");
    let mut first = true;
    for &b in &sel.bands {
        if !first {
            out.push(',');
        }
        first = false;
        let pct = (b * 1e8).round() as i64;
        out.push_str(&pct.to_string());
    }
    out.push('\n');
    out.into_bytes()
}

/// `sha256(canonical_calc_key_bytes)`, 64 hex символа нижнего регистра. Никакого
/// зависимого от toolchain промежуточного представления — независимая сторона
/// (`python3 hashlib.sha256`) даёт тот же текст.
pub fn calc_key_id(sel: &Selector) -> String {
    let bytes = canonical_calc_key_bytes(sel);
    let mut h = Sha256::new();
    h.update(&bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Каноническое имя варианта `Venue` в канонической форме. Отдельная функция — чтобы
/// изменение имени варианта (а оно часть публичного API контрактов) не молча меняло
/// `calc_key_id` для уже выданных слепков. Имена — ЗАФИКСИРОВАНЫ: `"Binance"`,
/// `"BinanceFutures"`, `"Hyperliquid"` (порядок совпадает с CT-RFC-09 §2.7).
fn venue_canonical(v: Venue) -> &'static str {
    match v {
        Venue::Binance => "Binance",
        Venue::BinanceFutures => "BinanceFutures",
        Venue::Hyperliquid => "Hyperliquid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prod() -> Selector {
        Selector {
            venue: Venue::Binance,
            symbol: "BTCUSDT".to_string(),
            timeframe_ms: 1000,
            bands: vec![0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6],
            window_ms: Some(60_000),
            depth_cadence_ms: Some(1000),
        }
    }

    #[test]
    fn encoding_marker_is_v1() {
        assert_eq!(CALC_KEY_ENCODING, "calc-key/v1");
    }

    #[test]
    fn canonical_form_seven_lines_newline_terminated() {
        let bytes = canonical_calc_key_bytes(&prod());
        let s = std::str::from_utf8(&bytes).expect("utf8");
        assert_eq!(
            s,
            "calc-key/v1\nvenue=Binance\nsymbol=BTCUSDT\ntimeframe_ms=1000\n\
             window_ms=60000\ndepth_cadence_ms=1000\n\
             bands_pct_e8=1500000,3000000,5000000,8000000,15000000,30000000,60000000\n"
        );
        assert_eq!(s.matches('\n').count(), 7, "семь строк, семь '\\n'");
    }

    #[test]
    fn none_spelled_as_literal() {
        let mut s = prod();
        s.window_ms = None;
        s.depth_cadence_ms = None;
        let binding = canonical_calc_key_bytes(&s);
        let s = std::str::from_utf8(&binding).unwrap();
        assert!(s.contains("window_ms=none\n"));
        assert!(s.contains("depth_cadence_ms=none\n"));
        // None и Some(0) — разные ключи (GW-I-14).
        let mut z = prod();
        z.window_ms = Some(0);
        let z_id = calc_key_id(&z);
        let mut n = prod();
        n.window_ms = None;
        let n_id = calc_key_id(&n);
        assert_ne!(z_id, n_id);
    }

    #[test]
    fn id_is_64_lowercase_hex() {
        let id = calc_key_id(&prod());
        assert_eq!(id.len(), 64);
        assert!(id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    }
}
