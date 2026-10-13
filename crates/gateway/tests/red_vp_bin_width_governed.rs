//! RED `M-86` (sacred, architect-only) — **РУЧКА ДОХОДИТ ДО СЕТКИ: установленное значение
//! РЕАЛЬНО меняет разбиение, а не лежит мёртвым глобалом.**
//!
//! Милестоун `milestones/M-86-vp-bin-width.md`, задача 1. Один тест — один бинарь: тест
//! меняет ПРОЦЕССНОЕ значение, и делить бинарь с соседями ему нельзя.
//!
//! ## Класс дефекта, против которого написан
//!
//! `R-133` B-1 (`M-71`): значение разбиралось, клалось в глобал и НЕ ЧИТАЛОСЬ ни одной
//! точкой применения — «built-not-wired» внутри одного процесса. Док-строка при этом
//! утверждала, что читается на каждом вызове. Ложное самоописание прошло два круга гейта.
//! Здесь тот же шов, и он пиннится ДО того, как dev к нему прикоснётся.
//!
//! COMPILE-RED: `set_effective_vp_bin_width_e8` не существует.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_752_000_010_000;

fn journal_of(prices_e8: &[i64]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(
            dir.path(),
            WriterConfig {
                max_segment_bytes: 1 << 26,
                min_free_bytes: 0,
                source: DataSource::OwnCapture,
                provenance: "test".to_string(),
                epoch_id: "own-test".to_string(),
            },
        )
        .expect("open_with");
        for (i, &p) in prices_e8.iter().enumerate() {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: p,
                    size: to_fixed(1.0),
                    side: Side::Buy,
                    ts_exch_ms: T + i as i64,
                },
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
        depth_cadence_ms: None,
    }
}

fn bins_now(dir: &std::path::Path) -> Vec<(i64, i64)> {
    let s = gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST)
        .expect("snapshot");
    assert_eq!(s.series.volume_profile.len(), 1, "SETUP: одна сессия");
    s.series.volume_profile[0].bins.clone()
}

/// Установленное значение обязано менять РАЗБИЕНИЕ, а не только читаться геттером.
#[test]
fn set_effective_vp_bin_width_actually_changes_binning() {
    let base = 77_000 * 100_000_000_i64;
    let w = gateway::DEFAULT_VP_BIN_WIDTH_E8;
    // Четыре цены, разнесённые на полширины: при W они дают 2 корзины, при 4·W — одну.
    let prices = [base, base + w / 2, base + w, base + 3 * w / 2];
    let dir = journal_of(&prices);

    // 1. Дефолт: геттер обязан вернуть подписанную норму ДО любой установки.
    assert_eq!(
        gateway::effective_vp_bin_width_e8(),
        w,
        "дефолт эффективного значения обязан равняться DEFAULT_VP_BIN_WIDTH_E8"
    );
    let at_default = bins_now(dir.path());
    assert_eq!(
        at_default.len(),
        2,
        "SETUP НЕ СОСТОЯЛСЯ: при ширине {w} ожидались 2 корзины, получено {} — \
         фикстура не давит на разбиение",
        at_default.len()
    );

    // 2. Ставим вчетверо шире — разбиение ОБЯЗАНО стать грубее.
    gateway::set_effective_vp_bin_width_e8(4 * w);
    let at_coarse = bins_now(dir.path());
    assert_eq!(
        at_coarse.len(),
        1,
        "НАРУШЕНО: после установки ширины {} ожидалась 1 корзина, получено {} — \
         значение не доходит до точки применения (класс R-133 B-1: built-not-wired)",
        4 * w,
        at_coarse.len()
    );
    assert_eq!(
        at_coarse[0].0 % (4 * w),
        0,
        "НАРУШЕНО: ключ корзины не на установленной сетке"
    );
    // Объём сохраняется и при смене ширины.
    let sum_default: i128 = at_default.iter().map(|&(_, v)| i128::from(v)).sum();
    let sum_coarse: i128 = at_coarse.iter().map(|&(_, v)| i128::from(v)).sum();
    assert_eq!(
        sum_default, sum_coarse,
        "НАРУШЕНО: смена ширины изменила суммарный объём ({sum_default} → {sum_coarse})"
    );

    // 3. Возвращаем дефолт — иначе процесс уедет с чужим значением, если сюда когда-нибудь
    //    добавят второй тест (см. шапку: пока файл однотестовый намеренно).
    gateway::set_effective_vp_bin_width_e8(w);
}
