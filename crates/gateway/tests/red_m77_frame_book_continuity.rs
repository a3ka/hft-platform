//! RED `VB-I-2` (sacred, architect-only) — **`M-77` задача 1: НЕПРЕРЫВНОСТЬ ДЕПТ-СЕРИИ НА
//! ПРОД-ПУТИ `LiveReducer::pump`, снятая НА ГРАНИЦЕ ПОТРЕБИТЕЛЯ.**
//!
//! Милестоун `milestones/M-77-frame-book-continuity.md`. Инвариант — `VB-I-2` («live ==
//! replay», `docs/fa/viz-backend.md:199`), живой и объявленный.
//!
//! # Предмет: состояние, собранное КЛИЕНТОМ, а не состояние сервера
//!
//! Прод-путь подписки (`crates/gateway-serve/src/lib.rs:1476-1524`, `:1695`, `:1830`):
//! `LiveReducer::resume` → `pump` (дренаж бэклога) → `snapshot_checked()` клиенту → далее
//! КАДРЫ из `pump`, которые клиент накатывает на этот снимок. Итоговое состояние клиента =
//! `snapshot(C) + Σ frames`.
//!
//! Кадр строится не из живого редьюсера, а из СВЕЖЕГО `batch = Reducer::new(&self.selector)`
//! (`crates/gateway/src/lib.rs:3900`, повтор `:4011`), в который переносятся ТОЛЬКО скаляры
//! VWAP (`:3901-3902`). Не переносятся: книга (`self.book`), состояние каденс-интервала
//! (`depth_cadence_current_bucket`), охват. Депт-серия же считается ИЗ КНИГИ на каждом
//! `L2`-событии (`M-68`: `recompute_depth_from_book`, `:1156`/`:1183`) и коммитится по
//! ролловеру каденции (`maybe_commit_depth_interval`, `:1330`).
//!
//! Следствие: кадр несёт депт-точки, посчитанные по книге, в которой лежат ТОЛЬКО уровни
//! дельт этого батча. Клиент получает не то, что в журнале.
//!
//! # Почему существующий корпус этого НЕ ловит — названо, а не подразумевается
//!
//! `md_i8_d20_live_equals_replay_under_cadence` (`red_depth_cadence.rs`) сравнивает
//! `live.snapshot()` с реплеем. `live.snapshot()` строится из `self.full` — персистентного
//! аккумулятора, У КОТОРОГО КНИГА ЕСТЬ. Он ЗЕЛЁН и обязан быть зелёным: сервер прав.
//! Мера снята с УЧАСТНИКА, а не с потребителя свойства — ровно `Р-1`
//! (`docs/workflow/oracle-blindness-class-2026-08-28.md` §5). Здесь мера снимается там, где
//! свойство видит клиент: на состоянии, собранном из ДОСТАВЛЕННЫХ кадров.
//!
//! `gw_i_4_holds_when_the_tail_frame_is_delta_only` (`red_depth_provenance_by_reach.rs`)
//! ходит `frames_since` — offline-путь сверки. Прод шлёт через `pump`. Развязка, починившая
//! `frames_since`, оставила бы прод сломанным.
//!
//! # Мера — ЗНАЧЕНИЯ точек, а не их число (и это установлено замером, а не выбрано)
//!
//! В установившемся прод-режиме (тик 250 мс, каденция 1000 мс) ЧИСЛО точек у клиента и у
//! реплея СОВПАДАЕТ, а значения расходятся: на прод-полосе `0.001` реплей даёт 4.0, клиент —
//! `0`. Оракул, сравнивающий количество, был бы зелёным против работающего дефекта.
//!
//! # Прод-форма селектора обязательна (`Р-2`)
//!
//! `window_ms = Some(60_000)`, `depth_cadence_ms = Some(1_000)`, `timeframe_ms = 1_000` —
//! замер `docker-compose.yml:135,142,154` на `origin/main`. `window_ms = None` — offline-режим
//! (`VB-I-10`), и оракул под ним судил бы ДРУГОЙ предмет.
//!
//! # Мутация, которой предъявляется сила набора (Done Block dev'а обязателен)
//!
//! Нейтрализовать перенос состояния в batch НЕЛЬЗЯ — его и нет; поэтому мутация ОБРАТНАЯ:
//! внести кандидат-развязку (например `batch.book = self.book.clone()` после
//! `crates/gateway/src/lib.rs:3900`) и предъявить, какие из тестов этого файла ПОЗЕЛЕНЕЛИ.
//! Тест, не изменивший исход ни при какой развязке, предмета не пиннит.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, DepthRow, Selector, Snapshot};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T: i64 = 1_752_000_000_000;

/// Прод-полоса: `GATEWAY_BANDS` по умолчанию (`docker-compose.yml:136`).
const PROD_BAND: f64 = 0.001;
/// Глубокая полоса — включается `П-014`/`M-70`. Судится ВТОРОЙ, чтобы дефект был предъявлен
/// и на сегодняшней проде, и на завтрашней.
const DEEP_BAND: f64 = 0.02;

/// Прод-настройки push-цикла (`crates/gateway-serve/src/lib.rs:1119-1120`).
const PUSH_MAX_EVENTS: usize = 256;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-77 frame/book continuity".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// Селектор ПРОД-ФОРМЫ. Значения — замер `docker-compose.yml` на `origin/main`.
fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![PROD_BAND, DEEP_BAND],
        window_ms: Some(60_000),
        depth_cadence_ms: Some(1_000),
    }
}

/// Якорь: широкая книга.
///
/// **Ближние уровни стоят на 0.05 % — ВНУТРИ `PROD_BAND` (0.1 %), но НЕ НА ЕГО ГРАНИЦЕ.**
/// `testing.md` §«Дегенерированный вход» п.4 запрещает ставить фикстуру ровно на границу
/// диапазона: округление уводит её в соседний. Первая редакция этого файла ставила их на
/// `MID*(1∓0.001)` — ровно на край, — и bid попадал в полосу, а ask НЕТ; поймал это
/// собственный страж различающей силы (`Р-4`) при прогоне против кандидат-развязки.
///
/// Дальний уровень `MID*(1±reach)` — снаружи обеих полос. Именно из-за ближнего реплей даёт
/// по прод-полосе НЕНУЛЕВУЮ глубину, которую книга-из-одних-дельт воспроизвести не может.
fn anchor(ts: i64, reach: f64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: vec![lvl(MID * (1.0 - reach), 3.0), lvl(MID * 0.9995, 4.0)],
            asks: vec![lvl(MID * (1.0 + reach), 3.0), lvl(MID * 1.0005, 4.0)],
            ts_exch_ms: ts,
        },
    )
}

/// Дельта, двигающая уровни на ±0.5 % — то есть ВНЕ `PROD_BAND` и ВНУТРИ `DEEP_BAND`.
/// Асимметрия управляется `two_sided` (`testing.md` §«Дегенерированный вход» п.1).
fn delta(ts: i64, seq: u64, two_sided: bool) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: vec![lvl(MID * 0.995, 7.0)],
            asks: if two_sided {
                vec![lvl(MID * 1.005, 7.0)]
            } else {
                vec![]
            },
            ts_exch_ms: ts,
            first_update_id: seq,
            final_update_id: seq,
            prev_final_update_id: Some(seq - 1),
        },
    )
}

fn append(dir: &std::path::Path, evs: Vec<EventKind>) {
    let mut j =
        Journal::open_with(dir, cfg()).unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
    for e in evs {
        j.append(e)
            .unwrap_or_else(|e| setup_failed(&format!("append: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
}

fn setup_failed(what: &str) -> ! {
    panic!("SETUP НЕ СОСТОЯЛСЯ: {what} — тест НЕ судил предмет, зелёное было бы вакуумом");
}

/// **Бэклог до подключения клиента.** Обязан покрыть ≥2 каденс-интервала, иначе снимок-при-
/// подключении не несёт НИ ОДНОЙ точки глубины, и расхождение ниже нельзя было бы отличить
/// от «клиент стартовал с пустого». Якорь + дельты на 2.2 с событийного времени.
fn backlog() -> Vec<EventKind> {
    let mut evs = vec![anchor(T, 0.05)];
    let mut ts = T + 100;
    for seq in 2..=22_u64 {
        evs.push(delta(ts, seq, true));
        ts += 100;
    }
    evs
}

/// Первый `seq`, свободный после `backlog()`.
const SEQ_AFTER_BACKLOG: u64 = 23;
/// Событийное время, следующее за `backlog()`.
const TS_AFTER_BACKLOG: i64 = T + 2_200;

/// **Тик push-цикла в ПРОД-ФОРМЕ: 250 мс событий.** Дельты биржи идут раз в 100 мс
/// (`DESIGN` §17), тик — 250 мс (`gateway-serve` `PUSH_INTERVAL_MS`), значит 2-3 события за
/// тик. Только при такой плотности граница каденс-интервала (1000 мс) попадает ВНУТРЬ
/// батча, и кадр вообще может нести точку глубины: `Reducer::new` стартует с
/// `depth_cadence_current_bucket = None`, коммит происходит лишь на ролловере
/// (`crates/gateway/src/lib.rs:1330-1344`).
///
/// `mk(ts, seq)` строит событие тика — параметр, чтобы контрольный тест гонял ТУ ЖЕ форму
/// тика снимками, а предметные — дельтами. Иначе контроль судил бы другой режим.
fn ticks_of(n: usize, mk: impl Fn(i64, u64) -> EventKind) -> Vec<Vec<EventKind>> {
    let mut ts = TS_AFTER_BACKLOG;
    let mut seq = SEQ_AFTER_BACKLOG;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut tick = Vec::with_capacity(3);
        for k in 0..3 {
            tick.push(mk(ts, seq));
            seq += 1;
            ts += if k == 2 { 50 } else { 100 };
        }
        out.push(tick);
    }
    out
}

fn row<'a>(s: &'a Snapshot, side: &str, band: f64) -> Option<&'a DepthRow> {
    let b = (band * 1e8).round() as i64;
    s.series
        .depth_series
        .iter()
        .find(|r| r.side == side && r.band_pct_e8 == b)
}

fn points(s: &Snapshot, side: &str, band: f64) -> Vec<(i64, i64)> {
    row(s, side, band)
        .map(|r| r.series.clone())
        .unwrap_or_default()
}

/// Полный реплей — независимый эталон (`testing.md`: эталон берётся из НЕЗАВИСИМОГО пути).
fn replay(dir: &std::path::Path) -> Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST)
        .unwrap_or_else(|e| setup_failed(&format!("полный реплей: {e}")))
}

/// Прод-путь подписки целиком. Возвращает `(состояние КЛИЕНТА, состояние СЕРВЕРА, кадров,
/// из них несущих точку глубины)`.
///
/// `tail` дописывается ПОСЛЕ того, как клиент получил снимок — иначе хвост уехал бы в
/// снимок и предмет (кадр, собранный без книги) не воспроизвёлся бы.
fn drive_prod_path(
    dir: &std::path::Path,
    backlog: Vec<EventKind>,
    ticks: Vec<Vec<EventKind>>,
) -> (Snapshot, Snapshot, usize, usize) {
    let ckpt = tempfile::tempdir().unwrap_or_else(|e| setup_failed(&format!("ckpt tempdir: {e}")));
    append(dir, backlog);

    let s = sel();
    let (mut live, _) =
        gateway::LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .unwrap_or_else(|e| setup_failed(&format!("resume: {e}")));
    loop {
        match live.pump(dir, EpochFilter::OwnCaptureOnly, PUSH_MAX_EVENTS) {
            Ok((f, _, _)) if f.is_empty() => break,
            Ok(_) => continue,
            Err(e) => setup_failed(&format!("pump бэклога: {e}")),
        }
    }
    // Снимок-при-подключении — ровно то, что прод шлёт клиенту (`snapshot_checked`, :1524).
    let mut client = live.snapshot();

    let mut frames_total = 0_usize;
    let mut frames_carrying_depth = 0_usize;
    for tick in ticks {
        append(dir, tick);
        match live.pump(dir, EpochFilter::OwnCaptureOnly, PUSH_MAX_EVENTS) {
            Ok((frames, _, _)) => {
                for f in &frames {
                    frames_total += 1;
                    let n: usize = f.delta.depth_series.iter().map(|r| r.series.len()).sum();
                    if n > 0 {
                        frames_carrying_depth += 1;
                    }
                    client.apply(f);
                }
            }
            Err(e) => setup_failed(&format!("pump тика: {e}")),
        }
    }
    let server = live.snapshot();
    (client, server, frames_total, frames_carrying_depth)
}

/// Общий страж: расхождение обязано быть ИМЕННО потребительским.
///
/// Сервер (`live.snapshot()`, из `self.full` — с книгой) обязан совпадать с реплеем. Если он
/// НЕ совпадает, предмет этого файла не воспроизведён: сломано что-то другое, и красное здесь
/// было бы приписано не той причине. Это дискриминатор (`Р-4`), а не судимое утверждение.
fn guard_server_matches_replay(server: &Snapshot, full: &Snapshot) {
    if server.series.depth_series != full.series.depth_series {
        setup_failed(
            "СЕРВЕР (live.snapshot()) разошёлся с полным реплеем. Предмет M-77 — расхождение \
             КЛИЕНТА при ПРАВОМ сервере; если неправ и сервер, красное ниже указывало бы на \
             другую причину",
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════════════
// КОНТРОЛЬ — анти-плацебо в сторону «слишком строгого» оракула
// ═══════════════════════════════════════════════════════════════════════════════════════

/// **КОНТРОЛЬ.** Хвост из СНИМКОВ: `apply_snapshot` ЗАМЕЩАЕТ книгу целиком, поэтому пустая
/// книга батча перестаёт быть проблемой, и клиент обязан совпасть с реплеем УЖЕ СЕГОДНЯ.
///
/// Без этого теста красное ниже нельзя отличить от «сравнение устроено так, что не сойдётся
/// никогда» — и набор был бы красен против любой реализации.
#[test]
fn vb_i_2_c_client_equals_replay_when_the_tail_carries_snapshots() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ticks = ticks_of(24, |ts, _seq| anchor(ts, 0.05));
    let (client, server, frames, with_depth) = drive_prod_path(dir.path(), backlog(), ticks);
    let full = replay(dir.path());
    guard_server_matches_replay(&server, &full);

    if frames == 0 || with_depth == 0 {
        setup_failed(&format!(
            "кадров {frames}, несущих глубину {with_depth} — сравнивать нечего"
        ));
    }
    assert_eq!(
        client.series.depth_series, full.series.depth_series,
        "КОНТРОЛЬ: при снимочном хвосте клиент ОБЯЗАН совпадать с реплеем уже сегодня \
         (`apply_snapshot` ЗАМЕЩАЕТ книгу целиком, поэтому пустая книга батча роли не \
         играет). Красное здесь означает, что сравнение негодно само по себе, и остальные \
         тесты файла ничего не доказывают."
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════
// ПРЕДМЕТ
// ═══════════════════════════════════════════════════════════════════════════════════════

/// **ЯДРО — установившийся ПРОД-режим.** Дельты каждые 100 мс, тик push-цикла 250 мс
/// (`gateway-serve` `PUSH_INTERVAL_MS`), каденция глубины 1000 мс.
///
/// Судятся ЗНАЧЕНИЯ точек: их ЧИСЛО в этом режиме совпадает, и оракул на количество был бы
/// зелёным против работающего дефекта.
#[test]
fn vb_i_2_client_depth_values_equal_replay_in_prod_steady_state() {
    let dir = tempfile::tempdir().expect("tempdir");
    // 24 тика × 250 мс = 6 с событийного времени ⇒ 6 каденс-интервалов.
    let ticks = ticks_of(24, |ts, seq| delta(ts, seq, true));
    let (client, server, frames, with_depth) = drive_prod_path(dir.path(), backlog(), ticks);
    let full = replay(dir.path());
    guard_server_matches_replay(&server, &full);

    // SETUP: кадры с глубиной обязаны быть — иначе клиент «не отстал», а просто ничего
    // не получал, и равенство/неравенство ниже было бы о другом.
    if with_depth == 0 {
        setup_failed(&format!(
            "из {frames} кадров НИ ОДИН не нёс точки глубины — режим каденции не воспроизведён"
        ));
    }

    for band in [PROD_BAND, DEEP_BAND] {
        for side in ["bid", "ask"] {
            let want = points(&full, side, band);
            let got = points(&client, side, band);
            // Р-4(а): признак обязан быть НЕДОСТУПЕН миру, где дефект починен. Здесь признак —
            // значения точек, и они различающи ТОЛЬКО если реплей даёт по этой полосе
            // ненулевую глубину, которой книга-из-одних-дельт не воспроизводит.
            if want.iter().all(|&(_, v)| v == 0) {
                setup_failed(&format!(
                    "реплей даёт по полосе {band} стороне {side} одни нули — фикстура не \
                     различает починенный мир от сломанного"
                ));
            }
            assert_eq!(
                got, want,
                "VB-I-2 НАРУШЕН НА ПРОД-ПУТИ (M-77): состояние, собранное КЛИЕНТОМ из \
                 snapshot(C)+frames, не равно полному реплею. band={band} side={side}. \
                 Кадров {frames}, из них с глубиной {with_depth}. \
                 Реплей: {want:?}; клиент: {got:?}. \
                 Причина: кадр строится свежим `Reducer::new(&self.selector)` \
                 (`crates/gateway/src/lib.rs:3900`), в который переносятся только скаляры \
                 VWAP (`:3901-3902`); книга и состояние каденс-интервала НЕ переносятся, \
                 поэтому глубина кадра посчитана по книге из одних дельт этого батча. \
                 Сервер при этом ПРАВ — расхождение потребительское (`Р-1`)."
            );
        }
    }
}

/// **ФОРМА 2 — потеря точки, а не искажение значения.** Односторонняя дельта в хвосте:
/// у книги батча нет середины ⇒ `recompute_depth_from_book` уходит в early-return
/// (`R-134` B-3, `md_i8_d9`), и точка не пишется ВООБЩЕ.
///
/// Асимметрия обязательна по `testing.md` §«Дегенерированный вход» п.1; п.3 («чего в
/// сообщении НЕТ — не сигнал к удалению») — ровно этот случай.
#[test]
fn vb_i_2_client_keeps_the_point_when_the_tail_delta_is_one_sided() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Первые 8 тиков двусторонние, дальше — ОДНОСТОРОННИЕ: у книги батча пропадает середина.
    let mut ticks = ticks_of(16, |ts, seq| delta(ts, seq, true));
    let one_sided = ticks_of(16, |ts, seq| delta(ts, seq, false));
    ticks[8..16].clone_from_slice(&one_sided[8..16]);
    let (client, server, frames, _with_depth) = drive_prod_path(dir.path(), backlog(), ticks);
    let full = replay(dir.path());
    guard_server_matches_replay(&server, &full);

    for band in [PROD_BAND, DEEP_BAND] {
        for side in ["bid", "ask"] {
            let want = points(&full, side, band);
            let got = points(&client, side, band);
            if want.is_empty() {
                setup_failed(&format!(
                    "реплей не дал ни одной точки по полосе {band} стороне {side}"
                ));
            }
            assert_eq!(
                got,
                want,
                "VB-I-2 (M-77, асимметричный хвост): клиент собрал {} точек против {} у \
                 реплея, значения {got:?} против {want:?}. band={band} side={side}. \
                 Кадров {frames}. Односторонняя дельта не даёт книге батча середины, полоса \
                 невычислима, точка не пишется — а у реплея книга полная и точка есть.",
                got.len(),
                want.len()
            );
        }
    }
}

/// **ФОРМА 3 — единственный дельта-кадр после ресинка.** Самый короткий воспроизводитель:
/// широкий якорь → дельта → УЗКИЙ ресинк-снимок → дельта-хвост. Держится отдельно от ядра,
/// потому что задевает окно ресинка (`TD-159`) и обязан остаться красным, даже если кто-то
/// решит, что установившийся режим «лечится каденцией».
#[test]
fn vb_i_2_client_equals_replay_across_a_resync_then_delta_tail() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ticks = ticks_of(16, |ts, seq| delta(ts, seq, true));
    // РЕСИНК: узкий снимок первым событием восьмого тика — книга сервера сужается,
    // и дальше дельты снова её растят. Окно ресинка — предмет `TD-159`.
    ticks[8][0] = anchor(TS_AFTER_BACKLOG + 8 * 250, 0.005);
    let (client, server, frames, with_depth) = drive_prod_path(dir.path(), backlog(), ticks);
    let full = replay(dir.path());
    guard_server_matches_replay(&server, &full);

    if with_depth == 0 {
        setup_failed(&format!("из {frames} кадров ни один не нёс глубины"));
    }
    for band in [PROD_BAND, DEEP_BAND] {
        for side in ["bid", "ask"] {
            let want = points(&full, side, band);
            let got = points(&client, side, band);
            assert_eq!(
                got, want,
                "VB-I-2 (M-77, ресинк + дельта-хвост): клиент разошёлся с реплеем. \
                 band={band} side={side}. Реплей: {want:?}; клиент: {got:?}"
            );
        }
    }
}
