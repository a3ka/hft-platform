//! RED `П-014` (sacred, architect-only) — метка достоверности глубины по НАБЛЮДЁННОМУ
//! ОХВАТУ и ПО СТОРОНЕ, а не по ширине полосы.
//!
//! # Что требует подпись founder'а
//!
//! `П-014` (ПОДПИСАНО 2026-08-17) включает полосы глубже 1.3 % и требует: «Отдаём обе
//! стороны, но каждая несёт свою метку достоверности: bid — "живость подтверждена",
//! ask на глубоких полосах — "не подтверждена, разрежённые данные"». `П-017` §«Следствие
//! для П-014» называет ДВА незакрытых предусловия, и оба сводятся сюда:
//!
//! * **(а) целостность при ресинке.** Охват книги ±60 % (`venue-binance` `MAX_REL_DIST`)
//!   верен в установившемся режиме. После старта процесса и после любого sequence-gap
//!   книга пересобирается REST-снимком с жёстким `REST_DEPTH_LIMIT = "5000"`, а это ~1.3 %
//!   на spot BTCUSDT и ~4.5 % на spot ETHUSDT. Барьера, удерживающего эмиссию до
//!   восстановления глубины, НЕТ, и venue об этом в журнал ничего не пишет — только
//!   `tracing::warn!`. Значит полоса 3 % в такие окна отдаёт глубину пятипроцентной книги,
//!   которой в ней нет, и метка об этом молчит.
//! * **(б) различение сторон.** M-58 подтвердил bid на всех семи полосах (0.713–0.992) и
//!   ОПРОВЕРГ ask на трёх из шести глубже 1.3 % (`[300,500)` 0.419, `[800,1500)` 0.247,
//!   `[3000,6000)` 0.403). Замок `A-002` З-2 не снят: `cancel_fraction` меряет насыщение,
//!   а не живость.
//!
//! # Почему это НЕ требует ни contract-RFC, ни нового милестоуна
//!
//! Всё нужное у `gateway` уже есть: книга по сторонам (`self.book.levels(Side::Buy/Sell)`)
//! и `DepthRow.side`. Охват СЧИТАЕТСЯ из той же книги, из которой считается глубина, —
//! знание о ресинке не требуется. Правка локальна одному крейту.
//!
//! # Инвариант, который пиннится (`GW-I-DP`)
//!
//! Метка `depth_band_provenance` есть функция ТРЁХ величин — ширины полосы, СТОРОНЫ и
//! НАБЛЮДЁННОГО ОХВАТА этой стороны, — а не одной ширины:
//!
//! | полоса | охват стороны | метка |
//! |---|---|---|
//! | ≤ 1.3 % | любой | `None` (валидированный эталон, прежний инвариант `VB-I-5`) |
//! | > 1.3 %, ВНУТРИ охвата, bid | достаёт | `…liveness=confirmed…` |
//! | > 1.3 %, ВНУТРИ охвата, ask | достаёт | `…liveness=unconfirmed…` |
//! | > 1.3 %, ЗА охватом | не достаёт | `not-observed…` — число полосы не наблюдалось вовсе |
//!
//! # Анти-плацебо — три стороны, и третья решает
//!
//! 1. Реализация, метящая по ширине (сегодняшняя), валит `sides_are_distinguished`.
//! 2. Реализация, не знающая охвата, валит `band_beyond_reach_is_named_not_observed`.
//! 3. **Реализация, метящая `not-observed` ВСЁ подряд, проходит (1) и (2) и валит
//!    `band_within_reach_is_not_falsely_marked`.** Без третьего теста «пометить всё»
//!    было бы дешёвым способом сделать оракул зелёным, обесценив метку: клиент,
//!    которому всё «не наблюдалось», не знает ничего.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, DepthRow, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_752_000_010_000;
const MID: f64 = 65_000.0;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvls(v: &[(f64, f64)]) -> Vec<Level> {
    v.iter()
        .map(|&(p, s)| Level {
            price: to_fixed(p),
            size: to_fixed(s),
        })
        .collect()
}

/// Книга, СИММЕТРИЧНО достающая до `reach_pct` от mid по обеим сторонам.
///
/// Симметрия здесь намеренна и проверяется отдельно: асимметричная книга — предмет
/// `reach_is_per_side`, и смешивать два предмета в одной фикстуре нельзя.
fn book_reaching(reach_pct: f64) -> EventKind {
    let mut bids = vec![(MID - 1.0, 5.0)];
    let mut asks = vec![(MID + 1.0, 5.0)];
    // уровни на 1/2 и на полном охвате — чтобы «дальний» уровень был не единственным
    for k in [0.5_f64, 1.0_f64] {
        bids.push((MID * (1.0 - reach_pct * k), 1.0));
        asks.push((MID * (1.0 + reach_pct * k), 1.0));
    }
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lvls(&bids),
            asks: lvls(&asks),
            ts_exch_ms: T,
        },
    )
}

/// То же, что `book_reaching`, но с явной меткой времени — фикстуре ниже нужны ТРИ снимка
/// книги в разных бакетах, иначе «жизни» схлопнутся в один.
fn book_reaching_at(reach_pct: f64, ts: i64) -> EventKind {
    let mut bids = vec![(MID - 1.0, 5.0)];
    let mut asks = vec![(MID + 1.0, 5.0)];
    for k in [0.5_f64, 1.0_f64] {
        bids.push((MID * (1.0 - reach_pct * k), 1.0));
        asks.push((MID * (1.0 + reach_pct * k), 1.0));
    }
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lvls(&bids),
            asks: lvls(&asks),
            ts_exch_ms: ts,
        },
    )
}

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
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

fn snap(dir: &std::path::Path, s: &Selector) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, s, Cursor::LATEST).expect("snapshot")
}

fn row<'a>(rows: &'a [DepthRow], side: &str, band: f64) -> &'a DepthRow {
    let want = (band * 1e8).round() as i64;
    rows.iter()
        .find(|r| r.side == side && r.band_pct_e8 == want)
        .unwrap_or_else(|| {
            panic!(
                "setup: строки depth для side={side} band={band} НЕТ — фикстура не построила \
                 предмет, и тест проверял бы пустоту (testing.md, целостность гейта, св-во 3). \
                 Есть: {:?}",
                rows.iter()
                    .map(|r| (r.side.as_str(), r.band_pct_e8))
                    .collect::<Vec<_>>()
            )
        })
}

/// ЯДРО (б): на ОДНОЙ и той же глубокой полосе bid и ask несут РАЗНЫЕ метки.
///
/// Сегодняшняя реализация (`crates/gateway/src/lib.rs:1035`) считает метку от
/// `row.band_pct_e8` и о стороне не знает — обе метки совпадают, тест красный.
#[test]
fn sides_are_distinguished_on_deep_band() {
    let dir = journal_of(vec![book_reaching(0.05)]);
    let s = snap(dir.path(), &sel(vec![0.001, 0.03]));
    let bid = row(&s.series.depth_series, "bid", 0.03);
    let ask = row(&s.series.depth_series, "ask", 0.03);

    let (pb, pa) = (
        bid.depth_band_provenance.as_deref().unwrap_or(""),
        ask.depth_band_provenance.as_deref().unwrap_or(""),
    );
    assert!(
        pb.contains("liveness=confirmed"),
        "П-014: bid на глубокой полосе обязан нести подтверждённую живость \
         (M-58: 0.713–0.992 на всех семи полосах). Получено: {pb:?}"
    );
    assert!(
        pa.contains("liveness=unconfirmed"),
        "П-014: ask на глубокой полосе обязан нести НЕподтверждённую живость — замок \
         A-002 З-2 не снят, M-58 опроверг ask на трёх полосах из шести. Получено: {pa:?}"
    );
    assert_ne!(
        pb, pa,
        "метка, одинаковая для обеих сторон, не различает их — это и есть дефект, \
         ради которого П-014 требует посторонней метки"
    );
}

/// ЯДРО (а): полоса ЗА пределами наблюдённого охвата названа НЕ НАБЛЮДЁННОЙ.
///
/// Книга достаёт до 5 %, спрошена полоса 10 %. Сегодня строка отдаёт глубину
/// пятипроцентной книги под видом десятипроцентной и молчит об этом.
#[test]
fn band_beyond_reach_is_named_not_observed() {
    let dir = journal_of(vec![book_reaching(0.05)]);
    let s = snap(dir.path(), &sel(vec![0.001, 0.10]));
    for side in ["bid", "ask"] {
        let r = row(&s.series.depth_series, side, 0.10);
        let p = r.depth_band_provenance.as_deref().unwrap_or("");
        assert!(
            p.starts_with("not-observed"),
            "П-017 предусловие (а): книга достаёт до 5 %, полоса 10 % НЕ наблюдалась — \
             её число обязано быть помечено как ненаблюдённое, а не выдано за факт. \
             side={side}, получено: {p:?}"
        );
    }
}

/// АНТИ-БЛАНКЕТ: полоса ВНУТРИ охвата не помечается ненаблюдённой.
///
/// Без этого теста реализация, метящая `not-observed` всё подряд, проходит оба ядра и
/// обесценивает метку.
#[test]
fn band_within_reach_is_not_falsely_marked() {
    let dir = journal_of(vec![book_reaching(0.05)]);
    let s = snap(dir.path(), &sel(vec![0.001, 0.03]));
    for side in ["bid", "ask"] {
        let r = row(&s.series.depth_series, side, 0.03);
        let p = r.depth_band_provenance.as_deref().unwrap_or("");
        assert!(
            !p.starts_with("not-observed"),
            "полоса 3 % ВНУТРИ охвата 5 % — наблюдалась. Метка «не наблюдалась» здесь \
             ложна и обесценивает метку там, где она правдива. side={side}, получено: {p:?}"
        );
    }
}

/// ГРАНИЦА прежнего инварианта `VB-I-5` не сдвинута: полоса ≤ 1.3 % метки не несёт.
///
/// Обратная мутация (`testing.md`): фикс не куплен ценой соседнего инварианта.
#[test]
fn shallow_band_carries_no_provenance() {
    let dir = journal_of(vec![book_reaching(0.05)]);
    let s = snap(dir.path(), &sel(vec![0.001, 0.03]));
    for side in ["bid", "ask"] {
        let r = row(&s.series.depth_series, side, 0.001);
        assert!(
            r.depth_band_provenance.is_none(),
            "VB-I-5: полоса 0.1 % ≤ 1.3 % — валидированный эталон, метки не несёт. \
             side={side}, получено: {:?}",
            r.depth_band_provenance
        );
    }
}

/// ОХВАТ СЧИТАЕТСЯ ПОСТОРОННЕ, а не по книге целиком.
///
/// Дегенерированный вход по `testing.md` §«Дегенерированный вход», п.1 (асимметрия):
/// bid достаёт до 5 %, ask — только до 1 % (так и выглядит книга после одностороннего
/// разрежения). Полоса 3 % наблюдалась на bid и НЕ наблюдалась на ask. Реализация,
/// считающая охват по книге целиком (max по сторонам), обе пометит одинаково и упадёт.
#[test]
fn reach_is_per_side() {
    let ev = EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lvls(&[(MID - 1.0, 5.0), (MID * 0.975, 1.0), (MID * 0.95, 1.0)]),
            asks: lvls(&[(MID + 1.0, 5.0), (MID * 1.01, 1.0)]),
            ts_exch_ms: T,
        },
    );
    let dir = journal_of(vec![ev]);
    let s = snap(dir.path(), &sel(vec![0.001, 0.03]));

    let pb = row(&s.series.depth_series, "bid", 0.03)
        .depth_band_provenance
        .clone()
        .unwrap_or_default();
    let pa = row(&s.series.depth_series, "ask", 0.03)
        .depth_band_provenance
        .clone()
        .unwrap_or_default();

    assert!(
        !pb.starts_with("not-observed"),
        "bid достаёт до 5 % — полоса 3 % наблюдалась. Получено: {pb:?}"
    );
    assert!(
        pa.starts_with("not-observed"),
        "ask достаёт лишь до 1 % — полоса 3 % на этой стороне НЕ наблюдалась. Охват, \
         посчитанный по книге целиком, а не посторонне, даёт здесь ложное «наблюдалась». \
         Получено: {pa:?}"
    );
}

/// `GW-I-4`/`VB-I-2` НА МЕТКЕ: `snapshot(C) + frames ≡ full` при МЕНЯЮЩЕМСЯ охвате.
///
/// # Блокер `R-110` Б-1 — дефект, которого мой первый набор не видел
///
/// Пять оракулов выше судят ОДИН снапшот. Но метка стала функцией НАБЛЮДЁННОГО ОХВАТА, а
/// охват меняется во времени — и merge-путь `Snapshot::apply`
/// (`crates/gateway/src/lib.rs:1452-1454`) остался с правилом «первый непустой побеждает»:
/// ```ignore
/// if current.depth_band_provenance.is_none() {
///     current.depth_band_provenance = incoming.depth_band_provenance.clone();
/// }
/// ```
/// При старой семантике (метка = чистая функция ширины полосы) это было безвредно: значение
/// не менялось никогда. При новой — метка ЗАЛИПАЕТ на первом значении.
///
/// **Прод-эффект, а не теория.** WS-клиент получает `Snapshot` ОДИН раз, дальше только
/// `Frame`. После ресинка книга обрезана, полоса не наблюдается — а клиент до самого
/// переподключения читает «liveness=confirmed» о полосе, которой в книге нет. Это ровно та
/// тихая ложь, против которой `П-014` и требует метку.
///
/// # Почему существующий `GW-I-4` слеп
///
/// `red_gateway_live_eq_replay.rs` сравнивает снапшоты целиком, но его селектор несёт
/// `bands=[0.001]` — полоса ≤ 1.3 %, метка там `None` ВСЕГДА, при любой реализации. Оракул
/// равенства, у которого сравниваемое поле константно, равенства этого поля не проверяет.
///
/// # Фикстура — ТРИ ЖИЗНИ охвата, а не два состояния
///
/// `testing.md` §«Дегенерированный вход» п.2 требует нескольких ЖИЗНЕЙ одной сущности:
/// охват ПАДАЕТ и ВОССТАНАВЛИВАЕТСЯ. Реализация, берущая минимум или максимум за окно,
/// проходит фикстуру из двух состояний и валится на трёх.
#[test]
fn provenance_survives_merge_when_reach_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        // ЧЕТЫРЕ жизни, и ПОСЛЕДНЯЯ отличается от ПЕРВОЙ — иначе залипание невидимо.
        // Первая редакция этой фикстуры шла 5 % → 1 % → 5 % и была ЗЕЛЕНА против заведомо
        // сломанного merge-пути: «первый непустой побеждает» давал тот же ответ, что реплей,
        // просто потому что охват вернулся к исходному. Оракул, зелёный против дефекта,
        // который он назван ловить, — вакуум; поймано собственным прогоном.
        j.append(book_reaching_at(0.05, T)).expect("a1"); // наблюдается
        j.append(book_reaching_at(0.01, T + 1_000)).expect("a2"); // ресинк: не наблюдается
        j.append(book_reaching_at(0.05, T + 2_000)).expect("a3"); // восстановлен
        j.append(book_reaching_at(0.01, T + 3_000)).expect("a4"); // снова обрезан — ИТОГ
        j.flush().expect("flush");
    }
    let s = sel(vec![0.001, 0.03]);

    let full = snap(dir.path(), &s);
    let base = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::START)
        .expect("snapshot(START)");
    // ДРЕНАЖ ПОБАТЧЕВО, по одному кадру. Первая редакция звала `frames_since(..., 64)` и
    // получала ОДИН кадр, уже несущий финальную метку: merge-путь при этом не задействован
    // вовсе (пустая база + один push), и оракул был ЗЕЛЕН против заведомо сломанного кода.
    // Липкая ветка `Snapshot::apply` срабатывает, только когда строка приходит ВТОРОЙ раз —
    // значит кадров обязано быть несколько, и каждый со своей меткой. Это и есть прод-форма:
    // клиент получает поток кадров, а не один агрегат.
    let mut merged = base;
    let mut cur = Cursor::START;
    let mut n_frames = 0_usize;
    loop {
        let (batch, next) =
            gateway::frames_since(dir.path(), EpochFilter::OwnCaptureOnly, &s, cur, 1)
                .expect("frames_since");
        if batch.is_empty() {
            break;
        }
        for f in &batch {
            merged.apply(f);
            n_frames += 1;
        }
        assert!(
            next > cur,
            "GW-I-8: курсор frames_since не монотонен ({next:?} <= {cur:?})"
        );
        cur = next;
    }
    assert!(
        n_frames >= 2,
        "SETUP НЕ СОСТОЯЛСЯ: кадров {n_frames} — merge-путь не задействован, и ассерт ниже \
         был бы зелен на реализации с ЛЮБОЙ семантикой слияния"
    );

    for side in ["bid", "ask"] {
        let want = row(&full.series.depth_series, side, 0.03)
            .depth_band_provenance
            .clone();
        let got = row(&merged.series.depth_series, side, 0.03)
            .depth_band_provenance
            .clone();
        assert_eq!(
            got, want,
            "GW-I-4/VB-I-2 НАРУШЕН на метке: snapshot(C)+frames расходится с полным реплеем. \
             side={side}. Полный реплей: {want:?}; собранный клиентом: {got:?}. Merge-путь \
             (`Snapshot::apply`) обязан отдавать метку, равную реплею, а не первую увиденную: \
             клиент получает Snapshot однажды и дальше живёт на Frame'ах."
        );
    }
}
