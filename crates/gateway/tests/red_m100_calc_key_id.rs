//! RED `M-100` (sacred, architect-only) — **стабильный идентификатор ключа расчёта**
//! (план SCALE §5: «согласуется будущий стабильный идентификатор состояния; переключение — в S3»;
//! §1 п.5: сегодняшний отпечаток — `DefaultHasher`, смена toolchain осиротит слепки МОЛЧА).
//!
//! # Объявленная форма (спека `milestones/M-100-three-serving-contracts.md` §4.3 — дословно)
//!
//! ```ignore
//! // crates/gateway/src/calc_key.rs
//! pub const CALC_KEY_ENCODING: &str = "calc-key/v1";
//! pub fn canonical_calc_key_bytes(sel: &Selector) -> Vec<u8>;
//! pub fn calc_key_id(sel: &Selector) -> String;   // sha256(canonical bytes), 64 hex, нижний регистр
//! ```
//!
//! Каноническая форма — UTF-8, ровно семь строк, каждая с `\n`:
//! `calc-key/v1` · `venue=<Binance|BinanceFutures|Hyperliquid>` · `symbol=<символ>` ·
//! `timeframe_ms=<i64>` · `window_ms=<i64|none>` · `depth_cadence_ms=<i64|none>` ·
//! `bands_pct_e8=<round(b·1e8) через запятую, в порядке селектора>`.
//!
//! **Эталон независим от кода выдачи:** золотые значения посчитаны `python3 hashlib.sha256` по
//! тексту формы выше (команда — §8 спеки), а не вызовом `gateway`. Toolchain Rust на них не влияет.
//!
//! # Чего этот файл НЕ требует
//!
//! Имя слепка НЕ меняется в `M-100` (`k5` — сторож): переключение на стабильный идентификатор —
//! S3 (`П-032` п.8, план §7).
//!
//! COMPILE-RED до появления модуля `gateway::calc_key` (вносит engine-dev задачей 3).
use contracts::Venue;
use gateway::calc_key::{calc_key_id, canonical_calc_key_bytes, CALC_KEY_ENCODING};
use gateway::Selector;

const SEVEN: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6];

fn prod() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1000,
        bands: SEVEN.to_vec(),
        window_ms: Some(60_000),
        depth_cadence_ms: Some(1000),
    }
}

/// **`k1` — каноническая форма ДОСЛОВНО и золотой идентификатор прод-селектора.**
#[test]
fn k1_canonical_form_and_golden_id_of_prod_selector() {
    assert_eq!(CALC_KEY_ENCODING, "calc-key/v1");
    let want = "calc-key/v1\nvenue=Binance\nsymbol=BTCUSDT\ntimeframe_ms=1000\nwindow_ms=60000\n\
                depth_cadence_ms=1000\nbands_pct_e8=1500000,3000000,5000000,8000000,15000000,\
                30000000,60000000\n";
    assert_eq!(
        String::from_utf8(canonical_calc_key_bytes(&prod())).expect("utf8"),
        want,
        "M-100: каноническая форма ключа расчёта расходится с объявленной"
    );
    assert_eq!(
        calc_key_id(&prod()),
        "201e0f954c40013f39fdfc8c7db729c19fe026c85a44d79d665a1aca7653c081",
        "M-100: идентификатор прод-селектора не равен независимо посчитанному sha256"
    );
}

/// **`k2` — `None` пишется словом `none`, а не нулём и не пропуском.**
#[test]
fn k2_absent_window_and_cadence_are_spelled_none() {
    let mut s = prod();
    s.window_ms = None;
    s.depth_cadence_ms = None;
    assert_eq!(
        calc_key_id(&s),
        "0d5eec7944b3d012b435aebc10c01bbd81b3b62bc9a30928482d20d68b98e4b2"
    );
    let mut z = prod();
    z.window_ms = Some(0);
    assert_ne!(
        calc_key_id(&z),
        calc_key_id(&{
            let mut n = prod();
            n.window_ms = None;
            n
        }),
        "window_ms = Some(0) и None — разные ключи (GW-I-14 различает их)"
    );
}

/// **`k3` — каждая из шести осей различает ключ** (ни одна ось не потеряна при переходе).
#[test]
fn k3_each_of_six_axes_changes_the_id() {
    let base = calc_key_id(&prod());
    let mut v = Vec::new();
    let mut s = prod();
    s.venue = Venue::BinanceFutures;
    v.push(("venue", s));
    let mut s = prod();
    s.symbol = "ETHUSDT".to_string();
    v.push(("symbol", s));
    let mut s = prod();
    s.timeframe_ms = 5000;
    v.push(("timeframe_ms", s));
    let mut s = prod();
    s.window_ms = Some(30_000);
    v.push(("window_ms", s));
    let mut s = prod();
    s.depth_cadence_ms = Some(2000);
    v.push(("depth_cadence_ms", s));
    let mut s = prod();
    s.bands = vec![0.015, 0.03];
    v.push(("bands", s));
    for (axis, s) in v {
        assert_ne!(
            calc_key_id(&s),
            base,
            "M-100: ось {axis} не различает ключ расчёта"
        );
    }
    let mut eth = prod();
    eth.symbol = "ETHUSDT".to_string();
    assert_eq!(
        calc_key_id(&eth),
        "75a8d817d372048e837375a5ddd98a69fdc1c4c83f81ea18ca49b864e451da0c"
    );
}

/// **`k4` — форма значения:** 64 символа `[0-9a-f]`.
#[test]
fn k4_id_is_64_lowercase_hex() {
    let id = calc_key_id(&prod());
    assert_eq!(id.len(), 64);
    assert!(id
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
}

/// **`k5` — СТОРОЖ: имя слепка в `M-100` НЕ меняется** (зелёный сегодня и обязан остаться).
/// Прод-слепок прод-селектора — `ckpt-8f69809dd707e8c9.bin` (`docs/ROADMAP.md` строка `M-90`,
/// замер 2026-10-02; тот же файл видел `wsprobe` 2026-10-10). Переименование — S3.
#[test]
fn k5_checkpoint_name_is_unchanged_by_m100() {
    let p = gateway::checkpoint::ckpt_path_for_pub(std::path::Path::new("/x"), &prod());
    assert_eq!(
        p.file_name().and_then(|n| n.to_str()),
        Some("ckpt-8f69809dd707e8c9.bin"),
        "M-100 / П-032 п.8: имя слепка изменилось — это переключение S3, а не S1a"
    );
}
