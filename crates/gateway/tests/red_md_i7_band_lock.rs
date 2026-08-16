//! RED M-67 `MD-I-7` (sacred, architect-only) — замок `A-002` З-1 обязан быть КОДОМ,
//! а не дефолтом переменной окружения.
//!
//! # Что замерено и почему это блокер, а не примечание
//!
//! `A-002` (арбитраж, 2026-08-03) и `docs/fa/viz-backend.md` (действующий абзац
//! «⛔ SUSPENDED») устанавливают: статус полос **1.5–60 %** — `verification pending`;
//! полосы глубже **1.3 %** НЕ включаются в прод-выдачу.
//!
//! **Замок НЕ снят и автоматически не снимется.** `M-58` переснял segment 78 исправленной
//! per-life метрикой 2026-08-04 (`research/data-quality/depth-verdict.md` §«M-58 ПЕРЕСНЯТО
//! … замок A-002 ОСТАЁТСЯ»). Исход СМЕШАННЫЙ: bid подтверждает живость на всех семи полосах
//! (0.713–0.992), **ask опровергает на трёх из шести глубже 1.3 %** — `[300,500) = 0.419`,
//! `[800,1500) = 0.247`, `[3000,6000) = 0.403`. `A-002` снимает замок автоматически ТОЛЬКО
//! при подтверждении; при смешанном исходе вопрос уходит founder'у (граница C).
//!
//! Замер architect'а 2026-08-16 (`grep -rn '1_300_000\|0\.013'`) даёт полный перечень мест,
//! где число 1.3 % вообще упоминается:
//!
//! * `crates/gateway/src/lib.rs:1035` — проставляет ПРОВЕНАНС-МЕТКУ для полос > 1.3 %;
//! * `crates/gateway/src/lib.rs:1140` — порог «глубокой» ячейки heatmap;
//! * `scripts/verify_M-58.sh:195` — проверяет ДЕФОЛТ `GATEWAY_BANDS` в `docker-compose.yml`.
//!
//! Барьера в коде НЕТ ни одного. `validate_selector` (`lib.rs:1751-1764`) проверяет только
//! `timeframe_ms`. То есть замок держится исключительно на том, что никто не выставит
//! `GATEWAY_BANDS=0.015` — а `A-002` §0(5) прямо оговаривает, что живой VPS-env оттуда не
//! проверялся. Провенанс-метка замком не является: `viz-backend.md` говорит это дословно —
//! «провенанс-метка НЕ является разрешением включать полосы глубже 1.3 %».
//!
//! `M-67` rev1 §6.1 читал ИСТОРИЧЕСКИЙ абзац `viz-backend.md` («живость доказана для
//! 1.5–30 %») как действующий и потому гейтил только 30–60 %. Это ошибка провенанса
//! документа: действующий абзац запирает ВСЕ семь полос набора, а не две дальние.
//!
//! # Контракт, который обязана предоставить реализация
//!
//! Пока замок не снят, `gateway` обязан ОТКАЗАТЬ на селекторе с полосой глубже 1.3 %
//! (fail-closed), а не отдать её с меткой. Отказ — на входе (`validate_selector`), то есть
//! на всех путях сразу (`snapshot`/`frames_since`/`replay`), а не в одной ветке.
//!
//! # Анти-плацебо — обе стороны обязательны
//!
//! * `n1` роняет сегодняшнюю реализацию (отдаёт глубокие полосы);
//! * `p1` роняет реализацию «отказывать всегда» / «отдавать пусто», которая тривиально
//!   прошла бы `n1`. Требуется НЕПУСТАЯ и НЕНУЛЕВАЯ серия по разрешённой полосе —
//!   «непустой ответ» без значений тоже плацебо (`C-091` строка MD-I-7).
//!
//! Семантическая проверка артефакта, снимающего замок (что это ПЕРЕСНЯТАЯ per-life метрика
//! `M-58`, а не любой непустой файл), живёт в `scripts/verify_M-67.sh` шаг B: она про
//! содержимое репозитория, а не про инвариант кода, и в юните была бы хрупкой.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
/// Граница валидированной зоны (`A-002` З-1): 1.3 % от mid.
const VALIDATED_MAX_BAND: f64 = 0.013;
/// Самая мелкая полоса канонического набора M-67 §4.3 — уже ГЛУБЖЕ замка.
const M67_SHALLOWEST_BAND: f64 = 0.015;

/// Книга шириной ±2 % от mid: полоса 1.5 % захватывает уровни, которых нет в 0.1 %.
/// Без этого негативный кейс был бы вакуумным — «отказано в том, чего и так нет».
fn build() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-67 MD-I-7 band-lock fixture".to_string(),
        epoch_id: "own-test".to_string(),
    };
    // Уровни на 0.05 %, 0.5 %, 1.0 %, 1.4 %, 1.8 % от mid с каждой стороны.
    let offsets = [0.0005_f64, 0.005, 0.010, 0.014, 0.018];
    let bids: Vec<Level> = offsets
        .iter()
        .map(|o| Level {
            price: to_fixed(MID * (1.0 - o)),
            size: to_fixed(2.0),
        })
        .collect();
    let asks: Vec<Level> = offsets
        .iter()
        .map(|o| Level {
            price: to_fixed(MID * (1.0 + o)),
            size: to_fixed(2.0),
        })
        .collect();
    let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
    for i in 0..20i64 {
        let ts = 1_752_000_000_000 + i * 100;
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: bids.clone(),
                asks: asks.clone(),
                ts_exch_ms: ts,
            },
        ))
        .expect("append");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID),
                size: to_fixed(1.0),
                side: [Side::Buy, Side::Sell][(i % 2) as usize],
                ts_exch_ms: ts + 5,
            },
        ))
        .expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn sel(bands: Vec<f64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
    }
}

/// **N1 — ГЛАВНОЕ.** Полоса глубже 1.3 % обязана быть ОТКЛОНЕНА, пока замок `A-002` З-1
/// не снят. Сегодня падает: селектор принимается и серия отдаётся с меткой провенанса.
#[test]
fn md_i7_n1_deep_band_is_refused_while_lock_stands() {
    let dir = build();
    let deep = sel(vec![0.001, M67_SHALLOWEST_BAND]);

    let res = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &deep,
        Cursor::LATEST,
    );

    match res {
        Err(_) => { /* требуемое поведение: fail-closed на входе */ }
        Ok(snap) => {
            let emitted: Vec<i64> = snap
                .series
                .depth_series
                .iter()
                .map(|r| r.band_pct_e8)
                .filter(|&b| b > (VALIDATED_MAX_BAND * 1e8) as i64)
                .collect();
            panic!(
                "MD-I-7 нарушен: полоса {M67_SHALLOWEST_BAND} глубже валидированной зоны \
                 {VALIDATED_MAX_BAND} принята, снапшот построен, отданы полосы \
                 band_pct_e8={emitted:?}. Замок A-002 З-1 держится только дефолтом \
                 GATEWAY_BANDS в docker-compose.yml — барьера в коде нет. \
                 Провенанс-метка замком не является (viz-backend.md, действующий абзац)."
            );
        }
    }
}

/// **N2.** Отказ обязан быть на ВХОДЕ, а не в одной ветке: `validate_selector` — общая точка
/// для `snapshot`/`frames_since`/`replay`. Реализация, закрывшая только `snapshot`,
/// оставила бы дыру на push-пути.
#[test]
fn md_i7_n2_refusal_is_at_the_entry_point() {
    let deep = sel(vec![0.001, M67_SHALLOWEST_BAND]);
    assert!(
        gateway::validate_selector(&deep).is_err(),
        "MD-I-7 нарушен: validate_selector принимает полосу {M67_SHALLOWEST_BAND}. Отказ, \
         поставленный не в общей точке входа, закрывает один путь и оставляет остальные."
    );
}

/// **P1 — анти-плацебо.** Разрешённая полоса обязана работать и давать НЕНУЛЕВЫЕ значения.
/// Реализация «отказывать всегда» или «отдавать пустое» проходит N1/N2 и падает здесь.
#[test]
fn md_i7_p1_validated_band_still_produces_real_depth() {
    let dir = build();
    let ok = sel(vec![0.001, VALIDATED_MAX_BAND]);

    assert!(
        gateway::validate_selector(&ok).is_ok(),
        "P1: полоса внутри валидированной зоны ({VALIDATED_MAX_BAND}) обязана приниматься — \
         иначе замок выключает продукт целиком, а не дальние полосы"
    );

    let snap = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &ok, Cursor::LATEST)
        .expect("P1: снапшот по валидированной полосе обязан строиться");

    assert!(
        !snap.series.depth_series.is_empty(),
        "P1: серия глубины пуста — реализация «ничего не отдавать» прошла бы N1 вакуумно"
    );
    let nonzero = snap
        .series
        .depth_series
        .iter()
        .any(|r| r.series.iter().any(|&(_, v)| v > 0));
    assert!(
        nonzero,
        "P1: все значения глубины нулевые — «непустой ответ» без значений есть то же плацебо \
         (C-091, строка MD-I-7): нужен ЗАМЕР, а не форма ответа"
    );

    for r in &snap.series.depth_series {
        assert!(
            r.band_pct_e8 <= (VALIDATED_MAX_BAND * 1e8) as i64,
            "P1: в ответе полоса band_pct_e8={} глубже валидированной зоны",
            r.band_pct_e8
        );
    }
}
