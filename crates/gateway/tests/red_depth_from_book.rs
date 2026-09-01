//! RED `MD-I-8` (sacred, architect-only) — **ДЕПТ-СЕРИЯ ПЕРЕСЧИТЫВАЕТСЯ НА КАЖДОМ L2-СОБЫТИИ.**
//!
//! Милестоун `milestones/M-68-depth-from-book.md` **rev3**. Форма набора задана решением
//! арбитра `research/arbitration/A-018-m68-cadence-not-reach.md` §1.3.
//!
//! # Почему набор переписан целиком, а не дополнен
//!
//! Посылка rev1/rev2 («полосы глубже ~1.3 % физически пусты — биржа капит снапшот») **ЛОЖНА**
//! (`A-018` §1.1, замер по `crates/venue-binance/src/lib.rs:398-399`): пейлоад `L2Snapshot`
//! есть бакетированная проекция НАШЕЙ diff-книги, обрезанная `MAX_REL_DIST = 0.60`, а не ответ
//! REST. Дальние уровни в снимке ЕСТЬ. `C-138` доказал это мутацией ФИКСТУРЫ (не кода): стоило
//! положить дальний уровень в снапшот — и `d1` rev2 зеленел на сегодняшней реализации. Оракул,
//! который меряет свой синтетический вход, инварианта не пиннит.
//!
//! Действующий дефект — **КАДЕНЦИЯ и ХВОСТ**: `depth_series` пересчитывается только в ветке
//! `L2Snapshot` (`crates/gateway/src/lib.rs:961-963`), а ветка `L2Delta` (`:984-986`) книгу и
//! heatmap двигает, полосы — нет. Снимки 1 Гц против дельт 100 мс.
//!
//! # ПРОД-ФОРМА ФИКСТУРЫ — требование, снявшее круг 2 (`A-018` §1.3 п.1)
//!
//! 1. **Снимок — ПРОЕКЦИЯ книги:** каждый следующий `L2Snapshot` содержит уровни, ранее
//!    доставленные дельтами (`bucket_levels(book.bids/asks)`), с обрезкой ±60 %.
//! 2. **Дельт больше, чем снимков, и ПОСЛЕДНЕЕ событие — ДЕЛЬТА** (прод: 100 мс против 1 Гц).
//! 3. **`update_id` непрерывен** — `prev_final_update_id` цепляется, как у venue.
//!
//! Фикстура, чей снимок никогда не вбирает дельты, моделирует эмиттер, которого не существует.
//!
//! # Состав набора и против чего каждый обязан краснеть
//!
//! | ID | пиннит | краснеет против |
//! |---|---|---|
//! | `d1` | дельта-ХВОСТ после последнего снимка; эталон АБСОЛЮТНЫЙ | snapshot-only (сегодня) и «подмножество полос» |
//! | `d2` | SETUP: heatmap — НЕЗАВИСИМЫЙ эталон, тот же уровень из тех же дельт видит | несостоявшегося setup'а (нет данных ⇒ красное `d1` объясняется не проводкой) |
//! | `d3` | анти-бланкет: узкая полоса остаётся узкой | «расширили все полосы до книги ради зелёного `d1`» |
//! | `d4` | КАЖДАЯ полоса, не подмножество | мутанта `C-M68-1` (`row.band >= 0.60`) и родственников |
//! | `d5` | ресинк ЗАМЕЩАЕТ книгу (`apply_snapshot` = replace) | «фикса» через вечный merge |
//! | `d7` | охват снят на ТОЙ ЖЕ ветке, что и числа — в ОБЕ стороны | разведения точек съёма (спека §2bis) |
//! | `d8` | смена СЕМАНТИКИ объявлена bump'ом; stale-чекпоинт отвергается | молчаливому переиспользованию старого кэша |
//! | `d8b` | warm-путь == полный реплей НА ХВОСТЕ ДЕЛЬТ, чекпоинт РЕАЛЬНО прочитан | тавтологии `d5b` rev2 (пустой каталог ⇒ fallback ⇒ реплей == реплей) |
//!
//! Ресурсный оракул `d6` живёт в `crates/gateway/tests/red_depth_recompute_cost.rs` отдельным
//! файлом НАМЕРЕННО: он COMPILE-RED (требует поля `ReadStats::depth_levels_visited`), и в общем
//! наборе ронял бы КОМПИЛЯЦИЮ, лишая возможности предъявить `d1`…`d8b` красными. «Не собралось»
//! и «упало на ассерте» — разные вещи, RED-first требует второго.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{CobLevel, Cursor, DepthRow, Selector};
use journal::{EpochFilter, Journal, WriterConfig};
use std::collections::BTreeMap;

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Узкая полоса — прод-дефолт `GATEWAY_BANDS=0.001` (`docker-compose.yml`). Предмет актуален
/// УЖЕ на ней: отстаёт полоса 0.1 % ровно так же, как отставала бы полоса 60 %.
const NEAR_BAND: f64 = 0.001;
/// Средняя полоса — ловит мутанта `C-M68-1`: он обновляет только `band >= 0.60`.
const MID_BAND: f64 = 0.03;
/// Дальняя полоса — предел эмиссии venue (`MAX_REL_DIST = 0.60`).
const FAR_BAND: f64 = 0.60;

/// Обрезка эмиссии venue: снимок-проекция не несёт уровней дальше этого (`:33`).
const MAX_REL_DIST: f64 = 0.60;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "MD-I-8 rev3 cadence fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// **Гигиена процессно-глобального окна (`C-201` B-6).** Эффективное окно heatmap/COB —
/// состояние ПРОЦЕССА, а тесты этого файла намеренно используют РАЗНЫЕ охваты. При
/// параллельном исполнении сосед перезаписывает окно под ногами, и тест падает, НЕ БУДУЧИ
/// сломанным.
///
/// **Это ФЛАК, и он был предъявлен прогоном, а не рассуждением:** три прогона подряд дали
/// `FAILED`, `FAILED`, `ok`. Прежний замер «959 passed / 1 failed» попал на удачный прогон и
/// доказательством не был — число, снятое ОДНИМ прогоном недетерминированного набора, ничего
/// не доказывает. Приём и причина — те же, что в `red_egress_cap.rs` и
/// `red_egress_cap_governed.rs:66`.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn sel(bands: Vec<f64>) -> Selector {
    // M-75 (`C-198` B-5): окно heatmap/COB БОЛЬШЕ НЕ выводится из `Selector.bands` — оно
    // серверное. Оракулы этого файла строились, когда окно было `max(bands)`, и их предмет
    // (глубина карты/COB, объём ответа) от охвата ЗАВИСИТ. Приём восстановления предмета:
    // ставим СЕРВЕРНОЕ окно равным тому, что прежде давал селектор, — смысл каждого теста
    // сохраняется дословно, меняется лишь ИСТОЧНИК величины.
    //
    // Настройка процессно-глобальна ⇒ тесты, зависящие от охвата, идут под `serial()`.
    let w = bands.iter().copied().fold(0.0_f64, f64::max);
    if w > 0.0 {
        gateway::set_effective_heatmap_window_frac(w);
    }
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
        depth_cadence_ms: None,
    }
}

/// Книга ФИКСТУРЫ — зеркало того, что держит venue-адаптер. Существует ровно затем, чтобы
/// `L2Snapshot` был ПРОЕКЦИЕЙ накопленного состояния, а не независимо сочинённым списком:
/// именно на этом упал круг 2 (`C-138` §«Новое независимое основание»).
#[derive(Default, Clone)]
struct FixtureBook {
    bids: BTreeMap<i64, i64>,
    asks: BTreeMap<i64, i64>,
}

impl FixtureBook {
    /// Зеркалит `M-29 apply_delta`: `size == 0` → снять уровень, `size > 0` → upsert.
    fn apply(&mut self, bids: &[Level], asks: &[Level]) {
        for l in bids {
            if l.size == 0 {
                self.bids.remove(&l.price);
            } else {
                self.bids.insert(l.price, l.size);
            }
        }
        for l in asks {
            if l.size == 0 {
                self.asks.remove(&l.price);
            } else {
                self.asks.insert(l.price, l.size);
            }
        }
    }

    /// ЗАМЕЩЕНИЕ книги (ресинк): venue сбрасывает состояние и строит его заново от REST-снимка.
    fn replace(&mut self, bids: &[Level], asks: &[Level]) {
        self.bids.clear();
        self.asks.clear();
        self.apply(bids, asks);
    }

    /// Проекция состояния в пейлоад `L2Snapshot` — с той же обрезкой `MAX_REL_DIST`, что у
    /// venue. Это и есть «снимок вбирает дельты»: всё, что доехало диффами, входит в снимок.
    fn project(&self) -> (Vec<Level>, Vec<Level>) {
        let mid = MID; // mid фикстуры неподвижен по построению (см. `assert` в d1)
        let lo = to_fixed(mid * (1.0 - MAX_REL_DIST));
        let hi = to_fixed(mid * (1.0 + MAX_REL_DIST));
        let bids = self
            .bids
            .iter()
            .rev()
            .filter(|(&p, _)| p >= lo)
            .map(|(&price, &size)| Level { price, size })
            .collect();
        let asks = self
            .asks
            .iter()
            .filter(|(&p, _)| p <= hi)
            .map(|(&price, &size)| Level { price, size })
            .collect();
        (bids, asks)
    }
}

/// Строитель прод-формы: держит книгу, эмитит снимки-проекции и дельты с непрерывным
/// `update_id`. `snapshot()` и `delta()` вызываются в том порядке, в каком идут события.
struct Emitter {
    book: FixtureBook,
    events: Vec<EventKind>,
    uid: u64,
}

impl Emitter {
    fn new() -> Self {
        Self {
            book: FixtureBook::default(),
            events: Vec::new(),
            uid: 0,
        }
    }

    /// Дельта: применяется к книге фикстуры И уезжает в журнал. `prev_final_update_id`
    /// цепляется за предыдущий — как в проде.
    fn delta(&mut self, bids: &[(f64, f64)], asks: &[(f64, f64)], ts: i64) -> &mut Self {
        let b: Vec<Level> = bids.iter().map(|&(p, s)| lvl(p, s)).collect();
        let a: Vec<Level> = asks.iter().map(|&(p, s)| lvl(p, s)).collect();
        self.book.apply(&b, &a);
        let prev = if self.uid == 0 { None } else { Some(self.uid) };
        self.uid += 2;
        self.events.push(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Delta {
                bids: b,
                asks: a,
                first_update_id: self.uid - 1,
                final_update_id: self.uid,
                prev_final_update_id: prev,
                ts_exch_ms: ts,
            },
        ));
        self
    }

    /// Снимок-ПРОЕКЦИЯ накопленной книги (venue `compute_book_snapshot_effects`).
    fn snapshot(&mut self, ts: i64) -> &mut Self {
        let (bids, asks) = self.book.project();
        self.events.push(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids,
                asks,
                ts_exch_ms: ts,
            },
        ));
        self
    }

    /// РЕСИНК: venue сбрасывает книгу (`state.book = None`, `:259`) и строит её заново от
    /// REST-снимка с жёстким `REST_DEPTH_LIMIT`. Снимок ЗАМЕЩАЕТ, а не дополняет.
    fn resync(&mut self, bids: &[(f64, f64)], asks: &[(f64, f64)], ts: i64) -> &mut Self {
        let b: Vec<Level> = bids.iter().map(|&(p, s)| lvl(p, s)).collect();
        let a: Vec<Level> = asks.iter().map(|&(p, s)| lvl(p, s)).collect();
        self.book.replace(&b, &a);
        self.events.push(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: b,
                asks: a,
                ts_exch_ms: ts,
            },
        ));
        self
    }

    fn into_journal(self) -> tempfile::TempDir {
        journal_of(self.events)
    }
}

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for e in events {
        j.append(e).expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn snap_of(dir: &std::path::Path, bands: Vec<f64>) -> gateway::Snapshot {
    gateway::snapshot(
        dir,
        EpochFilter::OwnCaptureOnly,
        &sel(bands),
        Cursor::LATEST,
    )
    .expect("snapshot обязан строиться")
}

fn row<'a>(rows: &'a [DepthRow], side: &str, band: f64) -> &'a DepthRow {
    let want = (band * 1e8).round() as i64;
    rows.iter()
        .find(|r| r.side == side && r.band_pct_e8 == want)
        .unwrap_or_else(|| {
            panic!(
                "нет строки depth_series side={side} band={band}; есть: {:?}",
                rows.iter()
                    .map(|r| (r.side.clone(), r.band_pct_e8))
                    .collect::<Vec<_>>()
            )
        })
}

/// Значение полосы в ПОСЛЕДНЕМ бакете серии. Именно последний бакет несёт дельта-хвост:
/// close-семантика (последнее наблюдение бакета побеждает) сохраняется фиксом без изменений.
fn last_value(rows: &[DepthRow], side: &str, band: f64) -> i64 {
    let r = row(rows, side, band);
    r.series
        .iter()
        .max_by_key(|(t, _)| *t)
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("серия side={side} band={band} ПУСТА — setup не состоялся"))
}

/// Живой охват стороны по `cob` (`HM-I-3`) — независимый от метки свидетель того, что дельта
/// действительно подвинула книгу. Нужен `d7`: без него «наблюдение» и «живая книга»
/// неразличимы, и оракул проверял бы отсутствие предмета.
fn live_reach(cob: &[CobLevel], side: &str) -> f64 {
    let mid = MID;
    cob.iter()
        .filter(|l| l.side == side)
        .map(|l| ((l.price_e8 as f64 / 1e8) - mid).abs() / mid)
        .fold(0.0_f64, f64::max)
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// ПРОД-ФОРМА: снимок → дельты → снимок-ПРОЕКЦИЯ → ДЕЛЬТА-ХВОСТ (последнее событие — дельта)
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Уровни фикстуры. Смещения подобраны так, что **mid НЕПОДВИЖЕН**: все дельты кладутся строго
/// внутрь книги, лучшие цены задаёт только стартовый снимок. Это не удобство, а условие
/// корректности эталона: сдвиг mid сместил бы пороги ВСЕХ полос, и абсолютные числа ниже
/// перестали бы быть эталоном.
const BEST_OFF: f64 = 0.0005; // 0.05 % — лучшая цена, внутри NEAR
const NEAR_A_OFF: f64 = 0.0008; // 0.08 % — внутри NEAR
const NEAR_B_OFF: f64 = 0.0009; // 0.09 % — внутри NEAR, ХВОСТ
const MID_OFF: f64 = 0.02; // 2 %    — внутри MID, вне NEAR, ХВОСТ
const FAR_A_OFF: f64 = 0.40; // 40 %   — внутри FAR, вне MID
const FAR_B_OFF: f64 = 0.45; // 45 %   — внутри FAR, вне MID, ХВОСТ

const SZ_BEST: f64 = 2.0;
const SZ_NEAR_A: f64 = 3.0;
const SZ_NEAR_B: f64 = 7.0;
const SZ_MID: f64 = 40.0;
const SZ_FAR_A: f64 = 500.0;
const SZ_FAR_B: f64 = 900.0;

/// Эталон АБСОЛЮТНЫЙ (`A-018` §1.3 п.2), а не «far > near»: первая редакция `d1` требовала
/// неравенства и была ЗЕЛЁНОЙ с первого запуска — широкая полоса захватывает больше уровней
/// СНАПШОТА сама по себе, без участия дельт.
const EXP_NEAR: f64 = SZ_BEST + SZ_NEAR_A + SZ_NEAR_B; // 12.0
const EXP_MID: f64 = EXP_NEAR + SZ_MID; // 52.0
const EXP_FAR: f64 = EXP_MID + SZ_FAR_A + SZ_FAR_B; // 1452.0

fn bid(off: f64) -> f64 {
    MID * (1.0 - off)
}
fn ask(off: f64) -> f64 {
    MID * (1.0 + off)
}

/// Прод-форма. Бакет A = `[T0, T0+1000)`, бакет B = `[T0+1000, T0+2000)`; серия судится по
/// ПОСЛЕДНЕМУ бакету, потому что дефект живёт именно в хвосте.
fn build_prod_form() -> tempfile::TempDir {
    let mut e = Emitter::new();
    // ── бакет A: старт книги и первый снимок
    e.delta(
        &[(bid(BEST_OFF), SZ_BEST)],
        &[(ask(BEST_OFF), SZ_BEST)],
        T0 - 500,
    );
    e.snapshot(T0);
    // дельты внутри бакета A — доезжают до следующего снимка
    e.delta(
        &[(bid(NEAR_A_OFF), SZ_NEAR_A)],
        &[(ask(NEAR_A_OFF), SZ_NEAR_A)],
        T0 + 100,
    );
    e.delta(
        &[(bid(FAR_A_OFF), SZ_FAR_A)],
        &[(ask(FAR_A_OFF), SZ_FAR_A)],
        T0 + 200,
    );
    // ── бакет B: снимок-ПРОЕКЦИЯ (вбирает обе дельты выше — это и есть прод-форма)
    e.snapshot(T0 + 1_000);
    // ── ДЕЛЬТА-ХВОСТ: три дельты ПОСЛЕ последнего снимка, по одной в каждую полосу
    e.delta(
        &[(bid(NEAR_B_OFF), SZ_NEAR_B)],
        &[(ask(NEAR_B_OFF), SZ_NEAR_B)],
        T0 + 1_100,
    );
    e.delta(
        &[(bid(MID_OFF), SZ_MID)],
        &[(ask(MID_OFF), SZ_MID)],
        T0 + 1_200,
    );
    e.delta(
        &[(bid(FAR_B_OFF), SZ_FAR_B)],
        &[(ask(FAR_B_OFF), SZ_FAR_B)],
        T0 + 1_300,
    );
    e.into_journal()
}

fn all_bands() -> Vec<f64> {
    vec![NEAR_BAND, MID_BAND, FAR_BAND]
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d2 — SETUP/ДИФФЕРЕНЦИАЛЬНЫЙ КОНТРОЛЬ. Идёт первым: он обязан быть ЗЕЛЁНЫМ, иначе красное
// `d1` объясняется отсутствием данных, а не проводкой выдачи.
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`d2` — heatmap как НЕЗАВИСИМЫЙ эталон.**
///
/// heatmap читает `self.book` уже сегодня (`refresh_heatmap_bucket` зовётся в ОБЕИХ ветках
/// `apply`). Значит уровень, доставленный ТОЛЬКО дельтой хвоста, он видеть ОБЯЗАН. Если не
/// видит — фикстура не донесла данные, и весь набор ниже вакуумен.
///
/// `testing.md` §«зависимый эталон мутация ловит плохо»: эталон взят из НЕЗАВИСИМОГО пути
/// (другая подсистема, другой код), а не из той же функции, что считает полосы.
#[test]
fn md_i8_d2_setup_heatmap_sees_the_tail_delta_level() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let dir = build_prod_form();
    let s = snap_of(dir.path(), all_bands());

    let want = to_fixed(bid(FAR_B_OFF));
    let seen = s.series.heatmap.iter().any(|c| c.price_e8 == want);
    assert!(
        seen,
        "SETUP НЕ СОСТОЯЛСЯ: heatmap не видит уровень {want} (bid {:.2}), доставленный ТОЛЬКО \
         дельтой хвоста. Данные до редьюсера не доехали — красное d1 ниже объяснялось бы \
         отсутствием данных, а не проводкой. Ячеек в heatmap: {}",
        bid(FAR_B_OFF),
        s.series.heatmap.len()
    );

    // Вторая половина setup'а: серия полос вообще заведена (её заводит только L2Snapshot,
    // и снимки в фикстуре есть).
    assert!(
        !s.series.depth_series.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: depth_series пуст — селектор или фикстура не те"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d1 — РАЗЛИЧАЮЩЕЕ СОБЫТИЕ: дельта ПОСЛЕ последнего снимка, обе стороны, эталон абсолютный
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`d1` — ХВОСТ НЕ ТЕРЯЕТСЯ: дельты после последнего снимка входят в серию.**
///
/// Сегодня ветка `L2Delta` полосы не пересчитывает (`crates/gateway/src/lib.rs:984-986`),
/// поэтому последний бакет несёт числа снимка `T0+1000`, а три дельты хвоста в серию не
/// попадают вовсе. Эталон — АБСОЛЮТНЫЙ (сумма уровней, реально стоящих в книге на финальном
/// курсоре), а не сравнение полос между собой.
///
/// Обе стороны проверяются симметрично (`testing.md` §«Дегенерированный вход» п.1):
/// односторонняя фикстура прячет дефекты, где стороны расходятся.
#[test]
fn md_i8_d1_depth_series_follows_the_delta_tail_on_both_sides() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let dir = build_prod_form();
    let s = snap_of(dir.path(), all_bands());

    // СВИДЕТЕЛЬ setup'а: mid неподвижен ⇒ пороги полос стабильны ⇒ абсолютный эталон валиден.
    let best_bid = s
        .series
        .cob
        .iter()
        .filter(|l| l.side == "bid")
        .map(|l| l.price_e8)
        .max()
        .unwrap_or(0);
    let best_ask = s
        .series
        .cob
        .iter()
        .filter(|l| l.side == "ask")
        .map(|l| l.price_e8)
        .min()
        .unwrap_or(0);
    assert_eq!(
        (best_bid, best_ask),
        (to_fixed(bid(BEST_OFF)), to_fixed(ask(BEST_OFF))),
        "SETUP НЕ СОСТОЯЛСЯ: лучшие цены сдвинулись ({best_bid}, {best_ask}) — mid уехал, \
         пороги полос вместе с ним, и абсолютный эталон ниже перестал быть эталоном"
    );

    for side in ["bid", "ask"] {
        for (band, exp, name) in [
            (NEAR_BAND, EXP_NEAR, "NEAR 0.1%"),
            (MID_BAND, EXP_MID, "MID 3%"),
            (FAR_BAND, EXP_FAR, "FAR 60%"),
        ] {
            let got = last_value(&s.series.depth_series, side, band);
            assert_eq!(
                got,
                to_fixed(exp),
                "MD-I-8 d1 [{side} {name}]: последний бакет несёт {got} при эталоне {} \
                 ({exp}). Дельты ПОСЛЕ последнего снимка в серию не вошли — депт-серия \
                 остаётся snapshot-only, а книга и heatmap ушли вперёд (d2 это подтвердил). \
                 Полосы обязаны считаться из `self.book` на пути L2Delta.",
                to_fixed(exp)
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d3 — АНТИ-БЛАНКЕТ. Зелёный сегодня и обязан остаться зелёным: это контроль, а не предмет.
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`d3` — узкая полоса остаётся узкой.**
///
/// Реализация «на дельте пересчитать всё по книге целиком, не глядя на ширину» зазеленила бы
/// `d1` и была бы грубо неверна: полоса 0.1 % включила бы уровни на 40 % и 45 %. Порог ассерта
/// назван от РАЗМЕРА дальнего уровня, а не от эталона `d1`, — чтобы оракул оставался
/// содержательным и при смене чисел фикстуры.
#[test]
fn md_i8_d3_narrow_band_does_not_swallow_the_far_levels() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let dir = build_prod_form();
    let s = snap_of(dir.path(), all_bands());

    for side in ["bid", "ask"] {
        let near = last_value(&s.series.depth_series, side, NEAR_BAND);
        assert!(
            near < to_fixed(SZ_FAR_A),
            "MD-I-8 d3 [{side}]: узкая полоса 0.1 % дала {near} — это ≥ размера ДАЛЬНЕГО \
             уровня ({}), то есть в неё попало то, что лежит на 40-45 % от mid. Полоса обязана \
             фильтровать по ширине, а не отдавать книгу целиком.",
            to_fixed(SZ_FAR_A)
        );
        let mid_band = last_value(&s.series.depth_series, side, MID_BAND);
        assert!(
            mid_band < to_fixed(SZ_FAR_A),
            "MD-I-8 d3 [{side}]: полоса 3 % дала {mid_band} — дальние уровни (40 %/45 %) в неё \
             попадать не должны"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d4 — КАЖДАЯ ПОЛОСА, А НЕ ПОДМНОЖЕСТВО (мутант `C-M68-1`)
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`d4` — дельта внутри СРЕДНЕЙ полосы двигает число ИМЕННО этой полосы.**
///
/// Мутант критика (`C-094` B2): обновлять от книги только `row.band >= 0.60`. Против круга 1
/// он был зелен ЦЕЛИКОМ. Здесь дельта хвоста `MID_OFF = 2 %` лежит ВНЕ узкой полосы и ВНЕ
/// диапазона, который мутант обновляет, — значит под мутантом полоса 3 % останется
/// snapshot-derived, а под честной реализацией вырастет ровно на `SZ_MID`.
///
/// Проверяется на ДВУХ путях выдачи: полный реплей и warm-путь через записанный чекпоинт.
/// Реализация, чинящая только live-ветку и не чинящая resume, красна здесь.
#[test]
fn md_i8_d4_every_band_moves_not_only_the_far_one() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let dir = build_prod_form();
    let s = sel(all_bands());

    let full = snap_of(dir.path(), all_bands());

    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &s, EpochFilter::OwnCaptureOnly)
        .expect("advance: чекпоинт обязан сниматься");
    let (warm, _stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint");

    for (path_name, snap) in [("полный реплей", &full), ("warm-путь", &warm)] {
        for side in ["bid", "ask"] {
            let got = last_value(&snap.series.depth_series, side, MID_BAND);
            assert_eq!(
                got,
                to_fixed(EXP_MID),
                "MD-I-8 d4 [{path_name} / {side}]: полоса 3 % дала {got} при эталоне {}. \
                 Дельта на {} % от mid лежит ВНУТРИ этой полосы и обязана её двигать. \
                 Мутант C-M68-1 («обновляем только band >= 0.60») оставляет её \
                 snapshot-derived — набор обязан быть красным против него.",
                to_fixed(EXP_MID),
                MID_OFF * 100.0
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d5 — РЕСИНК ЗАМЕЩАЕТ КНИГУ. Контроль соседнего инварианта: зелёный сегодня.
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`d5` — снимок ПОСЛЕ дельты ЗАМЕЩАЕТ книгу, и серия это отражает.**
///
/// `apply_snapshot` — полная замена (ресинк после gap: `state.book = None`, затем REST-снимок
/// с жёстким `REST_DEPTH_LIMIT`). Соблазнительный «фикс» `d1` — сделать книгу вечно
/// накапливающей (merge вместо replace): дальний уровень тогда никогда не пропадает, `d1`
/// зеленеет, а ресинк-семантика тихо ломается, и полоса отдаёт данные, которых в книге нет.
///
/// Зелёный сегодня — это КОНТРОЛЬ (`testing.md` §«что пришлось ослабить рядом»). Покраснел
/// после реализации ⇒ фикс куплен ценой соседнего инварианта.
#[test]
fn md_i8_d5_resync_snapshot_replaces_the_book_not_merges() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let mut e = Emitter::new();
    e.delta(
        &[(bid(BEST_OFF), SZ_BEST)],
        &[(ask(BEST_OFF), SZ_BEST)],
        T0 - 500,
    );
    e.delta(
        &[(bid(FAR_A_OFF), SZ_FAR_A)],
        &[(ask(FAR_A_OFF), SZ_FAR_A)],
        T0 - 400,
    );
    e.snapshot(T0); // снимок-проекция: дальний уровень В НЁМ есть
                    // РЕСИНК: книга сброшена, REST отдал только ближние уровни (~0.05 %)
    e.resync(
        &[(bid(BEST_OFF), SZ_BEST)],
        &[(ask(BEST_OFF), SZ_BEST)],
        T0 + 1_000,
    );
    // хвост-дельта после ресинка — прод-форма (последнее событие всегда дельта)
    e.delta(
        &[(bid(NEAR_A_OFF), SZ_NEAR_A)],
        &[(ask(NEAR_A_OFF), SZ_NEAR_A)],
        T0 + 1_100,
    );
    let dir = e.into_journal();
    let s = snap_of(dir.path(), all_bands());

    // СВИДЕТЕЛЬ: живая книга после ресинка действительно УЗКАЯ — иначе тест проверял бы не то.
    for side in ["bid", "ask"] {
        let lr = live_reach(&s.series.cob, side);
        assert!(
            lr < 0.01,
            "SETUP НЕ СОСТОЯЛСЯ [{side}]: живой охват {lr:.6} — ресинк не сузил книгу, и \
             «замещение» с «накоплением» здесь неразличимы"
        );
    }

    // Ассерт — НЕРАВЕНСТВО, и это осознанный выбор формы. Точное значение полосы после
    // ресинка зависит от того, вошла ли хвостовая дельта в серию, — а это предмет `d1`, и
    // проверять его дважды значило бы сделать `d5` красным СЕГОДНЯ по чужой причине.
    // `d5` спрашивает ровно одно: пережил ли ресинк дальний уровень. Порог назван от размера
    // этого уровня, поэтому оракул остаётся содержательным и при смене чисел фикстуры.
    for side in ["bid", "ask"] {
        let far = last_value(&s.series.depth_series, side, FAR_BAND);
        assert!(
            far < to_fixed(SZ_FAR_A),
            "MD-I-8 d5 [{side}]: после ресинка полоса 60 % дала {far} — это ≥ размера дальнего \
             уровня ({}), стоявшего в книге ДО ресинка. Он ресинк пережил, значит книга \
             накапливает вместо замещения (`apply_snapshot` обязан быть replace), и серия \
             отдаёт глубину, которой в книге НЕТ. Это соблазнительный «фикс» d1: вечный merge \
             зеленит хвост и тихо ломает ресинк-семантику.",
            to_fixed(SZ_FAR_A)
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d7 — ОХВАТ СНИМАЕТСЯ ТАМ ЖЕ, ГДЕ ЧИСЛА (спека §2bis). Обе стороны движения книги.
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`d7` — метка описывает ТО наблюдение, из которого взяты числа. Направление ВНИЗ.**
///
/// `depth_reach_bid`/`depth_reach_ask` (`crates/gateway/src/lib.rs:975-976`) снимаются сегодня
/// в ветке `L2Snapshot` — там же, где числа. Это было верно, пока числа были snapshot-only.
/// После M-68 числа берутся из живой книги на КАЖДОМ событии, значит и охват обязан сниматься
/// на каждом: иначе метка `liveness=confirmed`, снятая секунду назад, встаёт поверх числа,
/// посчитанного по обрезанной ресинком книге. Это ровно та тихая ложь, против которой
/// подписана `П-014` (`PL-I-7`: деградация не выдаётся за норму).
#[test]
fn md_i8_d7_reach_is_sampled_where_the_numbers_are_delta_shrinks_the_book() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let mut e = Emitter::new();
    e.delta(
        &[(bid(BEST_OFF), SZ_BEST), (bid(0.05), SZ_FAR_A)],
        &[(ask(BEST_OFF), SZ_BEST), (ask(0.05), SZ_FAR_A)],
        T0 - 500,
    );
    e.snapshot(T0); // охват 5 % — полоса 3 % НАБЛЮДЕНА
                    // дельта хвоста снимает дальние уровни: живая книга сжалась до 0.05 %
    e.delta(&[(bid(0.05), 0.0)], &[(ask(0.05), 0.0)], T0 + 1_000);
    let dir = e.into_journal();
    // Полосы селектора включают дальнюю НАМЕРЕННО: окно `cob` — это `max(selector.bands)`
    // (`crates/gateway/src/lib.rs:1192`), и без неё свидетель живого охвата слеп по
    // построению — он бы «не видел» уровень, который фикстура как раз и проверяет.
    let s = snap_of(dir.path(), all_bands());

    for side in ["bid", "ask"] {
        let lr = live_reach(&s.series.cob, side);
        assert!(
            lr < 0.01,
            "SETUP НЕ СОСТОЯЛСЯ [{side}]: живой охват {lr:.6} — дельта не сняла дальние \
             уровни, и «наблюдение» с «живой книгой» неразличимы"
        );
    }

    for side in ["bid", "ask"] {
        let prov = row(&s.series.depth_series, side, MID_BAND)
            .depth_band_provenance
            .clone()
            .unwrap_or_default();
        assert!(
            prov.starts_with("not-observed"),
            "MD-I-8 d7 [{side}]: полоса 3 % несёт метку {prov:?}. После M-68 ЧИСЛО этой полосы \
             посчитано по книге, обрезанной дельтой до 0.05 %, — значит наблюдения на 3 % \
             больше нет, и метка обязана это назвать. Снимочный охват поверх дельта-числа = \
             метка описывает состояние, в серию не вошедшее."
        );
    }
}

/// **`d7` зеркало — направление ВВЕРХ. Анти-бланкет к предыдущему.**
///
/// Реализация, ошибающаяся в точке съёма охвата, ошибается в ОБЕ стороны; оракул, проверяющий
/// одно направление, ловит половину. Здесь книга РАСШИРЕНА дельтами после узкого ресинк-снимка
/// — прод-случай: REST отдаёт ~1.3 %, дальше дельты достраивают глубину. После M-68 число
/// полосы 3 % считается по расширенной книге, значит метка обязана перестать быть
/// `not-observed`.
#[test]
fn md_i8_d7b_reach_is_sampled_where_the_numbers_are_delta_grows_the_book() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let mut e = Emitter::new();
    e.delta(
        &[(bid(BEST_OFF), SZ_BEST)],
        &[(ask(BEST_OFF), SZ_BEST)],
        T0 - 500,
    );
    e.snapshot(T0); // узкий снимок: охват 0.05 %, полоса 3 % НЕ наблюдена
                    // дельты хвоста достраивают книгу вглубь до 5 %
    e.delta(
        &[(bid(0.05), SZ_FAR_A)],
        &[(ask(0.05), SZ_FAR_A)],
        T0 + 1_000,
    );
    let dir = e.into_journal();
    // Дальняя полоса в селекторе — по той же причине, что в `d7`: окно `cob` есть
    // `max(selector.bands)`, и без неё уровень на 5 % не попал бы в свидетеля вовсе.
    let s = snap_of(dir.path(), all_bands());

    for side in ["bid", "ask"] {
        let lr = live_reach(&s.series.cob, side);
        assert!(
            lr > 0.04,
            "SETUP НЕ СОСТОЯЛСЯ [{side}]: живой охват {lr:.6} — дельта не достроила книгу, \
             и направление ВВЕРХ здесь не моделируется"
        );
    }

    for side in ["bid", "ask"] {
        let prov = row(&s.series.depth_series, side, MID_BAND)
            .depth_band_provenance
            .clone()
            .unwrap_or_default();
        assert!(
            !prov.starts_with("not-observed"),
            "MD-I-8 d7b [{side}]: полоса 3 % несёт {prov:?}. После M-68 ЧИСЛО этой полосы \
             посчитано по книге, достроенной дельтами до 5 %, — наблюдение есть, и метка \
             `not-observed` его отрицает. Ошибка в ту же сторону, что d7, но обратного знака."
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d8 / d8b — ЧЕКПОИНТ: объявленная смена смысла + warm-путь на ХВОСТЕ ДЕЛЬТ
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`d8` — смена СЕМАНТИКИ выдачи объявлена bump'ом, а stale-чекпоинт отвергается.**
///
/// Две половины, и обе нужны.
///
/// **(а) Bump.** `П-014` п.3 требует bump `GATEWAY_SCHEMA_VERSION` при смене формы/семантики
/// выдачи (прецеденты `VB-I-6`: M-36 5→6, M-38a 6→7, M-48 7→8). Депт-серия меняет смысл: была
/// «глубина на момент последнего снимка», стала «глубина на момент последнего события».
/// Проверка идёт через РАНТАЙМ-ИНДИРЕКЦИЮ — поле построенного `Snapshot`, а не сравнение
/// литералов: `d5` rev2 был ассертом над compile-time константой, который
/// `clippy::assertions-on-constants` запрещает, и зазеленеть не мог НИКОГДА (`C-138` п.3).
///
/// **(б) Рычаг работает.** `read_and_validate` шаг (3) отвергает чекпоинт при
/// `gw_v != GATEWAY_SCHEMA_VERSION` (`crates/gateway/src/lib.rs:2901-2904`). Половина (б)
/// зелена и сегодня — это КОНТРОЛЬ: она предъявляет, что bump из (а) действительно
/// инвалидирует кэш, а не просто меняет число. `C-094` B3 требовал именно ЯВНОЙ инвалидации.
#[test]
fn md_i8_d8_semantics_bump_is_declared_and_invalidates_stale_checkpoint() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let dir = build_prod_form();
    let s = sel(all_bands());

    // ── (а) bump объявлен. Рантайм-индирекция: значение приходит из построенного снапшота.
    let full = snap_of(dir.path(), all_bands());
    assert!(
        full.schema_version >= 9,
        "MD-I-8 d8(а): GATEWAY_SCHEMA_VERSION = {} — смена СЕМАНТИКИ депт-серии \
         (snapshot-only → каждое событие) не объявлена bump'ом. `П-014` п.3 и прецеденты \
         VB-I-6 (M-36 5→6, M-38a 6→7, M-48 7→8) требуют его; он же — ЕДИНСТВЕННЫЙ рычаг, \
         отвергающий чекпоинт со старым смыслом (read_and_validate шаг 3).",
        full.schema_version
    );

    // ── (б) рычаг работает: чекпоинт с ЧУЖОЙ версией не переиспользуется молча.
    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &s, EpochFilter::OwnCaptureOnly)
        .expect("advance");
    let files: Vec<_> = std::fs::read_dir(ckpt.path())
        .expect("read_dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "SETUP НЕ СОСТОЯЛСЯ: ожидался ровно один файл чекпоинта, найдено {:?}",
        files
    );
    let path = &files[0];
    let mut bytes = std::fs::read(path).expect("read ckpt");
    assert!(
        bytes.len() > 16,
        "SETUP НЕ СОСТОЯЛСЯ: файл чекпоинта короче заголовка"
    );
    // Портим ИМЕННО `gateway_schema_version` (bytes[12..16]) — не magic и не ckpt-версию:
    // проверяется рычаг смысла, а не целостность файла.
    let stale = full.schema_version.saturating_sub(1);
    bytes[12..16].copy_from_slice(&stale.to_le_bytes());
    std::fs::write(path, &bytes).expect("write ckpt");

    let (from_stale, stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint при stale-чекпоинте обязан ПЕРЕСОБРАТЬ, а не упасть");

    let n_events = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .count() as u64;
    assert_eq!(
        stats.events_decoded, n_events,
        "MD-I-8 d8(б): при stale-чекпоинте декодировано {} событий из {n_events} — значит он \
         был ПЕРЕИСПОЛЬЗОВАН, а не отвергнут. Чекпоинт со старым смыслом редьюсера обязан \
         отвергаться, иначе VB-I-2 ломается в warm-start пути молча.",
        stats.events_decoded
    );
    assert_eq!(
        from_stale.series.depth_series, full.series.depth_series,
        "MD-I-8 d8(б): пересборка после отвергнутого чекпоинта разошлась с полным реплеем"
    );
}

/// **`d8b` — warm-путь тождествен полному реплею НА ХВОСТЕ ДЕЛЬТ, и чекпоинт РЕАЛЬНО прочитан.**
///
/// # Чем это отличается от `d5b` rev2, который был тавтологией
///
/// `d5b` rev2 создавал пустой каталог и сразу звал `snapshot_from_checkpoint`. Код уходил в
/// ветку (3) «Fallback: rebuild от START» (`crates/gateway/src/lib.rs:2124-2200`) — то есть
/// сравнивал полный реплей с полным реплеем. `C-138` предъявил это прогоном; арбитр
/// подтвердил (`A-018` §2.1 п.2). Здесь:
///
/// 1. чекпоинт **ЗАПИСЫВАЕТСЯ** реальным API `advance_to` на позицию ДО дельта-хвоста;
/// 2. **SETUP-GUARD доказывает, что он ПРОЧИТАН** — `events_decoded` warm-пути СТРОГО меньше
///    полного числа событий. Без этого ассерта тест снова проверял бы отсутствие предмета;
/// 3. эталон берётся НЕЗАВИСИМЫМ путём — полный реплей от `START`, а не та же функция.
#[test]
fn md_i8_d8b_warm_resume_equals_full_replay_across_the_delta_tail() {
    let _g = serial(); // C-201 B-6: окно heatmap процессно-глобально
    let dir = build_prod_form();
    let s = sel(all_bands());

    let n_events = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .count() as u64;
    assert!(
        n_events >= 6,
        "SETUP НЕ СОСТОЯЛСЯ: в фикстуре {n_events} событий — дельта-хвоста, ради которого \
         тест написан, в ней нет"
    );

    // Чекпоинт снимается ДО хвоста: последние три события (дельта-хвост) обязаны
    // досчитываться резюмом, а не входить в чекпоинт.
    let cut = Cursor {
        upto_seq: Some(n_events - 4),
    };
    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &s,
        EpochFilter::OwnCaptureOnly,
        cut,
    )
    .expect("advance_to обязан записать чекпоинт");

    let (warm, stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint");

    // SETUP-GUARD: чекпоинт ПРОЧИТАН. Если бы код ушёл в fallback полного реплея, было бы
    // `events_decoded == n_events`, и сравнение ниже сравнивало бы реплей с реплеем.
    assert!(
        stats.events_decoded < n_events,
        "SETUP НЕ СОСТОЯЛСЯ: warm-путь декодировал {} событий из {n_events} — чекпоинт НЕ был \
         прочитан, код ушёл в fallback полного реплея (`:2124-2200`), и тест выродился в \
         тавтологию «реплей == реплей» — ровно дефект d5b rev2 (C-138, A-018 §2.1 п.2)",
        stats.events_decoded
    );

    let full = snap_of(dir.path(), all_bands());
    assert_eq!(
        warm.series.depth_series, full.series.depth_series,
        "MD-I-8 d8b: warm-путь разошёлся с полным реплеем на дельта-хвосте. `VB-I-2` \
         (live == replay) обязан держаться и когда часть хвоста досчитывается резюмом: \
         реализация, чинящая только live-ветку и не чинящая resume, красна здесь."
    );
}
