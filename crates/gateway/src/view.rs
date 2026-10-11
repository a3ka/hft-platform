//! M-100 (`milestones/M-100-three-serving-contracts.md` §4.2, `CT-RFC-09` §2.10.2) —
//! **проекция отображения** (`view`): выбор строк глубины из ГОТОВОГО расчёта. Не меняет
//! расчёт (`calc_key_id` остаётся тот же — `П-029`: «галочка переключает ОТОБРАЖЕНИЕ, а не
//! заказывает ВЫЧИСЛЕНИЕ»). Применяется ТРАНСПОРТОМ к снимку и к каждому кадру подписки
//! ПОСЛЕ проверки предела объёма (предел меряет полный расчёт — проекция его только
//! уменьшает).
//!
//! Форма выбора строки — `(band_pct_e8, side)` (`"bid"` | `"ask"`). Стороны НЕ
//! суммируются. Прочие ряды `SeriesBundle` (`ohlcv`, `cumulative_delta`, `vwap`,
//! `volume_profile`, `cvd_session_base`, `vp_session_max_time_s`, `heatmap`,
//! `cob`, `volume_bubbles`, `heatmap_observed_time`, `cob_observed`, `cadence_ms`)
//! проекцией не затрагиваются.

use serde::{Deserialize, Serialize};

use super::{DepthRow, SeriesBundle, Selector};

/// Ключ одной строки глубины: `(band_pct_e8, side)`. `band_pct_e8` хранится в долях ×1e8
/// (то же представление, что в `DepthRow.band_pct_e8`, `С-185` B-2 / `MD-I-8` —
/// «`time_s` в секундах, `band_pct_e8` в `i64`», детерминированная форма).
///
/// `side` — `"bid" \| "ask"`. Прочие значения — `validate` отвергает как
/// `UnknownDepthRow`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DepthRowKey {
    pub band_pct_e8: i64,
    pub side: String,
}

/// Выбор ОТОБРАЖЕНИЯ (НЕ выбора РАСЧЁТА). Применяется ТРАНСПОРТОМ к снимку и к каждому
/// кадру подписки после проверки предела объёма.
///
/// `depth_rows == None` или пустой `Vec` (после `#[serde(default)]`) — показывать все
/// строки расчёта (тождество, `p1`); невалидный список (`EmptyDepthRows` /
/// `UnknownDepthRow` / `DuplicateDepthRow`) — отказ `invalid_view` (`w6`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewSpec {
    #[serde(default)]
    pub depth_rows: Option<Vec<DepthRowKey>>,
}

/// Ошибка валидации выбора. Каждая содержит КЛЮЧ строки, на которой валидация упала —
/// для диагностики оператора (с указанием `band_pct_e8` и `side`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// Пустой `depth_rows` (НЕ `None` — это «всё»).
    EmptyDepthRows,
    /// `(band_pct_e8, side)` отсутствует в ключе расчёта (нет такой полосы или
    /// сторона не `"bid"`/`"ask"`).
    UnknownDepthRow(DepthRowKey),
    /// Два и более вхождения одного и того же `(band_pct_e8, side)`.
    DuplicateDepthRow(DepthRowKey),
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDepthRows => write!(f, "depth_rows пуст"),
            Self::UnknownDepthRow(k) => write!(
                f,
                "depth_rows содержит неизвестную строку (band_pct_e8={}, side={:?}) — \
                 отсутствует в ключе расчёта; для выбора строк используйте диапазон полос \
                 серверного профиля",
                k.band_pct_e8, k.side
            ),
            Self::DuplicateDepthRow(k) => write!(
                f,
                "depth_rows содержит дубль строки (band_pct_e8={}, side={:?})",
                k.band_pct_e8, k.side
            ),
        }
    }
}

impl ViewSpec {
    /// Пустой выбор (`depth_rows == None`) — тождество, всегда `Ok`.
    pub fn is_empty(&self) -> bool {
        self.depth_rows.as_ref().is_none_or(|v| v.is_empty())
    }

    /// Валидация против ключа расчёта (`П-029`/`П-032`):
    /// - `depth_rows == None` или пустой `Vec` → `Ok` (тождество);
    /// - `side` не `"bid"`/`"ask"` или `band_pct_e8` не в полосах ключа → `UnknownDepthRow`;
    /// - дубль `(band_pct_e8, side)` → `DuplicateDepthRow` (первый встреченный дубль);
    /// - пустой список при явном `Some(vec![])` → `EmptyDepthRows` (`p3`: «молча показать
    ///   всё» запрещено).
    pub fn validate(&self, calc: &Selector) -> Result<(), ViewError> {
        let Some(rows) = &self.depth_rows else {
            return Ok(());
        };
        if rows.is_empty() {
            return Err(ViewError::EmptyDepthRows);
        }
        // Детектим дубли через BTreeSet: один проход, O(N log N) — для типичных
        // `depth_rows.len() ≤ 14` (одна строка на полосу × сторону) это шум.
        let mut seen: std::collections::BTreeSet<(i64, &str)> =
            std::collections::BTreeSet::new();
        for k in rows {
            // Нормализуем side (lowercase, как в `DepthRow.side`) для валидации.
            let side_norm: &str = match k.side.as_str() {
                "bid" | "ask" => &k.side,
                _ => return Err(ViewError::UnknownDepthRow(k.clone())),
            };
            if !seen.insert((k.band_pct_e8, side_norm)) {
                return Err(ViewError::DuplicateDepthRow(k.clone()));
            }
            let band_ok = calc.bands.iter().any(|b| {
                let pct = (b * 1e8).round() as i64;
                pct == k.band_pct_e8
            });
            if !band_ok {
                return Err(ViewError::UnknownDepthRow(k.clone()));
            }
        }
        Ok(())
    }
}

/// Проекция отображения: `SeriesBundle` → `SeriesBundle`, оставляет в `depth_series`
/// ровно выбранные строки (в порядке строк расчёта, `p2`); содержимое выбранных строк
/// (включая `series` и `series_provenance`) — байт-в-байт как в источнике (`p2`); прочие
/// ряды (`ohlcv`, `cumulative_delta`, `vwap`, `volume_profile`, `cvd_session_base`,
/// `vp_session_max_time_s`, `heatmap`, `cob`, `volume_bubbles`, `heatmap_observed_time`,
/// `cob_observed`, `cadence_ms`) — не тронуты (`p2`).
///
/// `view.is_empty()` (== НЕ задан) → тождество (`p1`).
///
/// **Применяется ТРАНСПОРТОМ к снимку И к каждому кадру подписки ПОСЛЕ проверки предела
/// объёма** (`П-029`: предел меряет полный расчёт — проекция только уменьшает; `§1`
/// спеки: «применять `view` ДО проверки предела объёма» — ЗАПРЕЩЕНО).
pub fn project_view(series: &SeriesBundle, view: &ViewSpec) -> SeriesBundle {
    if view.is_empty() {
        return series.clone();
    }
    let chosen = view.depth_rows.as_ref().expect("is_empty ⇒ None");
    // BTreeMap<(band, side), &DepthRow> для O(log N) lookup и сохранения порядка
    // строк расчёта при итерации по source-рядам (не по chosen — это и даёт
    // «в порядке строк расчёта» из `p2`).
    let chosen_set: std::collections::BTreeSet<(i64, &str)> = chosen
        .iter()
        .map(|k| (k.band_pct_e8, k.side.as_str()))
        .collect();
    let mut out = series.clone();
    out.depth_series = series
        .depth_series
        .iter()
        .filter(|r| chosen_set.contains(&(r.band_pct_e8, r.side.as_str())))
        .cloned()
        .collect();
    // Каждая строка выводится ДОСЛОВНО (содержимое `series` и `series_provenance`) —
    // это та же проверка `p2` «содержимое выбранной строки изменено проекцией»,
    // и `p4` сторожит, что проекция НЕ переставила порядок точек.
    out
}

/// Сравнение ДВУХ строк глубины на равенство ключа (используется тестами для сравнения
/// множеств строк). Извлекает `(band_pct_e8, side)` из `DepthRow`.
pub fn depth_row_key(r: &DepthRow) -> (i64, String) {
    (r.band_pct_e8, r.side.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc() -> Selector {
        Selector {
            venue: Venue::Binance,
            symbol: "BTCUSDT".to_string(),
            timeframe_ms: 1000,
            bands: vec![0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6],
            window_ms: Some(60_000),
            depth_cadence_ms: Some(1000),
        }
    }

    fn key(band: f64, side: &str) -> DepthRowKey {
        DepthRowKey {
            band_pct_e8: (band * 1e8).round() as i64,
            side: side.to_string(),
        }
    }

    #[test]
    fn empty_or_none_is_identity_ok() {
        assert!(ViewSpec::default().validate(&calc()).is_ok());
        assert!(ViewSpec {
            depth_rows: None
        }
        .validate(&calc())
        .is_ok());
    }

    #[test]
    fn empty_vec_is_error() {
        assert_eq!(
            ViewSpec {
                depth_rows: Some(vec![])
            }
            .validate(&calc()),
            Err(ViewError::EmptyDepthRows)
        );
    }

    #[test]
    fn unknown_band_or_side_is_error() {
        let u = key(0.2, "bid"); // не в полосах ключа
        assert_eq!(
            ViewSpec {
                depth_rows: Some(vec![u.clone()])
            }
            .validate(&calc()),
            Err(ViewError::UnknownDepthRow(u))
        );
        let s = key(0.15, "mid"); // невалидная сторона
        assert_eq!(
            ViewSpec {
                depth_rows: Some(vec![s.clone()])
            }
            .validate(&calc()),
            Err(ViewError::UnknownDepthRow(s))
        );
    }

    #[test]
    fn duplicate_is_error() {
        let d = key(0.15, "bid");
        assert_eq!(
            ViewSpec {
                depth_rows: Some(vec![d.clone(), d.clone()])
            }
            .validate(&calc()),
            Err(ViewError::DuplicateDepthRow(d))
        );
    }

    #[test]
    fn valid_subset_ok() {
        assert!(ViewSpec {
            depth_rows: Some(vec![key(0.015, "bid"), key(0.6, "ask")])
        }
        .validate(&calc())
        .is_ok());
    }
}