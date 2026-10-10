//! RED `M-100` (sacred, architect-only) — **«что пользователь СМОТРИТ» отделено от «что сервер
//! СЧИТАЕТ»**: проекция отображения выбирает строки глубины из готового расчёта и не меняет
//! расчёт (`П-029`: «галочка переключает ОТОБРАЖЕНИЕ, а не заказывает ВЫЧИСЛЕНИЕ»).
//!
//! # Объявленная форма (спека §4.2 — дословно)
//!
//! ```ignore
//! // crates/gateway/src/view.rs
//! #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
//! pub struct DepthRowKey { pub band_pct_e8: i64, pub side: String }        // "bid" | "ask"
//! #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
//! pub struct ViewSpec { #[serde(default)] pub depth_rows: Option<Vec<DepthRowKey>> }
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! pub enum ViewError { EmptyDepthRows, UnknownDepthRow(DepthRowKey), DuplicateDepthRow(DepthRowKey) }
//! impl ViewSpec { pub fn validate(&self, calc: &Selector) -> Result<(), ViewError>; }
//! pub fn project_view(series: &SeriesBundle, view: &ViewSpec) -> SeriesBundle;
//! ```
//!
//! COMPILE-RED до появления `gateway::view` (engine-dev, задача 2).
use contracts::Venue;
use gateway::view::{project_view, DepthRowKey, ViewError, ViewSpec};
use gateway::{DepthRow, Selector, SeriesBundle};

const SEVEN: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6];

fn calc() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1000,
        bands: SEVEN.to_vec(),
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

/// Бандл как у прода: 14 строк (семь полос × две стороны) + непустые прочие ряды.
fn bundle() -> SeriesBundle {
    let mut b = SeriesBundle::default();
    for band in SEVEN {
        for side in ["bid", "ask"] {
            b.depth_series.push(DepthRow {
                side: side.to_string(),
                band_pct_e8: (band * 1e8).round() as i64,
                series: vec![(1, 100), (2, 200)],
                series_provenance: vec![None, None],
            });
        }
    }
    b.cumulative_delta = vec![(1, 5)];
    b.cob_observed = true;
    b.heatmap_observed_time_s = vec![1, 2];
    b
}

fn rows(b: &SeriesBundle) -> Vec<(i64, String)> {
    b.depth_series
        .iter()
        .map(|r| (r.band_pct_e8, r.side.clone()))
        .collect()
}

/// **`p1` — без выбора — тождество** (клиент, не приславший `view`, видит всё, как сегодня).
#[test]
fn p1_absent_selection_is_identity() {
    let b = bundle();
    assert_eq!(project_view(&b, &ViewSpec::default()), b);
}

/// **`p2` — выбор оставляет РОВНО выбранные строки, в порядке расчёта; прочие ряды не тронуты.**
#[test]
fn p2_selection_keeps_exactly_selected_rows_and_nothing_else_changes() {
    let b = bundle();
    let v = ViewSpec {
        depth_rows: Some(vec![key(0.6, "ask"), key(0.15, "bid")]),
    };
    let p = project_view(&b, &v);
    assert_eq!(
        rows(&p),
        vec![
            (15_000_000, "bid".to_string()),
            (60_000_000, "ask".to_string())
        ],
        "M-100: проекция обязана оставить ровно выбранные строки в порядке строк расчёта"
    );
    let mut rest = p.clone();
    rest.depth_series = b.depth_series.clone();
    assert_eq!(rest, b, "M-100: проекция тронула ряды, кроме строк глубины");
    for r in &p.depth_series {
        let src = b
            .depth_series
            .iter()
            .find(|x| x.band_pct_e8 == r.band_pct_e8 && x.side == r.side)
            .expect("строка есть в расчёте");
        assert_eq!(
            r, src,
            "M-100: содержимое выбранной строки изменено проекцией"
        );
    }
}

/// **`p3` — выбор проверяется против КЛЮЧА РАСЧЁТА:** пустой список, неизвестная строка
/// (полоса вне набора, неверная сторона), дубль — названные ошибки; «молча показать всё» запрещено.
#[test]
fn p3_validate_rejects_empty_unknown_and_duplicate() {
    let c = calc();
    assert_eq!(
        ViewSpec {
            depth_rows: Some(vec![])
        }
        .validate(&c),
        Err(ViewError::EmptyDepthRows)
    );
    let unknown = key(0.2, "bid");
    assert_eq!(
        ViewSpec {
            depth_rows: Some(vec![unknown.clone()])
        }
        .validate(&c),
        Err(ViewError::UnknownDepthRow(unknown))
    );
    let side = key(0.15, "mid");
    assert_eq!(
        ViewSpec {
            depth_rows: Some(vec![side.clone()])
        }
        .validate(&c),
        Err(ViewError::UnknownDepthRow(side))
    );
    let d = key(0.15, "bid");
    assert_eq!(
        ViewSpec {
            depth_rows: Some(vec![d.clone(), d.clone()])
        }
        .validate(&c),
        Err(ViewError::DuplicateDepthRow(d))
    );
    assert_eq!(
        ViewSpec {
            depth_rows: Some(vec![key(0.015, "bid"), key(0.6, "ask")])
        }
        .validate(&c),
        Ok(())
    );
    assert_eq!(ViewSpec::default().validate(&c), Ok(()));
}

/// **`p4` — проводная форма выбора:** `{"depth_rows":[{"band_pct_e8":15000000,"side":"bid"}]}`;
/// отсутствие поля — «всё».
#[test]
fn p4_wire_form_of_view() {
    let v: ViewSpec =
        serde_json::from_str(r#"{"depth_rows":[{"band_pct_e8":15000000,"side":"bid"}]}"#)
            .expect("parse");
    assert_eq!(v.depth_rows, Some(vec![key(0.15, "bid")]));
    let all: ViewSpec = serde_json::from_str("{}").expect("parse");
    assert_eq!(all, ViewSpec::default());
}
