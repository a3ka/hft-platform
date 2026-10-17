//! **`M-77` задача 4 — СТОРОЖ ЦЕНЫ РАЗВЯЗКИ Б НА ГРАНИЦЕ `pump`** (sacred, architect-only).
//!
//! Милестоун `milestones/M-77-frame-book-continuity.md`. Инвариант — `VB-I-10`
//! (bounded-window snapshot, `docs/fa/viz-backend.md:207`).
//!
//! # Зачем файл существует
//!
//! `C-211` выбрал развязку Б и отверг А не за неработоспособность, а за цену: «клон самого
//! дорогого поля на КАЖДЫЙ batch, и цену эту не сторожит никто». Последнее — замер, а не
//! опасение: `red_snapshot_noclone` меряет аллокации `snapshot()`, а `pump` зовёт лишь для
//! выхода на хвост, ВНЕ измеряемого участка (`red_snapshot_noclone.rs:189-197`, `:221`).
//! Под кандидат-развязкой А суита крейта зелена целиком (`M-77` §6). Этот файл — условие
//! выбора Б: цена перестаёт быть обещанием.
//!
//! Мера снимается на границе `pump` — там, где живёт свойство (`Р-1`,
//! `docs/workflow/oracle-blindness-class-2026-08-28.md` §5), а не на соседнем методе.
//!
//! # Ось измерения — ЧИСЛО БАТЧЕЙ, а не размер книги. Это выбрано ЗАМЕРОМ
//!
//! Первая редакция этого файла варьировала размер книги (×8) по образцу `O-1` и оказалась
//! КРАСНОЙ ПРОТИВ СЕГОДНЯШНЕГО КОДА — отношение `4.39` при потолке `2.5`. Причина
//! измерена, а не додумана: `refresh_heatmap_bucket` зовёт `book.levels(side)` на КАЖДОМ
//! `L2`-событии (`crates/gateway/src/lib.rs:1150`, `:1177`), то есть тик уже сегодня стоит
//! `O(размер книги)` НА СОБЫТИЕ. Это предсуществующее свойство, к `M-77` отношения не
//! имеющее; оракул на этой оси судил бы ЧУЖОЙ предмет и краснел бы до и после любой
//! развязки. Замер назван отдельным долгом в `M-77` §9ter, а не спрятан в порог.
//!
//! Ось «число батчей» от этого свойства СВОБОДНА: per-event стоимость постоянна при любом
//! дроблении тика, а `клон на каждый batch` растёт ровно с числом батчей. Это дословно та
//! величина, которую назвал `C-211`.
//!
//! # Мерится СУММА аллокаций, а не ПИК — и это тоже установлено замером
//!
//! Клон, освобождаемый в конце батча, пика не поднимает: на оси «число батчей» пиковая
//! мера дала `0.85`–`1.11` и НЕ РАЗЛИЧАЛА миры (замер на обоих деревьях). Сумма
//! аллоцированных байт различает их decisively — числа в `M-77` §9bis.
//!
//! # Почему аллокации, а не время
//!
//! Урок `TD-078`, дважды подтверждённый: потолок wall-clock превращает оракул в измеритель
//! CI-машины и флакает. Аллоцированные байты детерминированы.
//!
//! # Учёт ПОТОКОВЫЙ, а не процессный
//!
//! `cargo test` гоняет тесты бинаря параллельными потоками, и процессный счётчик ловил бы
//! аллокации соседа (флак `main` 2026-08-16, `red_snapshot_noclone`). Потоковый учёт делает
//! конфаундер структурно недостижимым, а не удерживает его дисциплиной запуска
//! (`testing.md` «Целостность гейта», свойство 2).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::Selector;
use journal::{EpochFilter, Journal, WriterConfig};

thread_local! {
    static T_TOTAL: Cell<usize> = const { Cell::new(0) };
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let _ = T_TOTAL.try_with(|t| t.set(t.get().saturating_add(l.size())));
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
    }
}
#[global_allocator]
static GA: Counting = Counting;

/// СУММА аллоцированных байт за время `f` (не пик — см. шапку).
fn total_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = T_TOTAL.with(|c| c.get());
    let r = f();
    (r, T_TOTAL.with(|c| c.get()).saturating_sub(base))
}

const BASE_MS: i64 = 1_784_116_800_000;
const MID_BID: f64 = 65_000.0;
const MID_ASK: f64 = 65_010.0;

/// Книга широкая НАМЕРЕННО: клон стоит `O(книги)`, и на узкой книге его не отличить от шума.
const LEVELS: usize = 16_000;
/// Событий в измеряемом тике. 64 даёт плечо между «один батч» и «батч на событие».
const TICK_EVENTS: u64 = 64;

/// Потолок отношения. Замер обоих миров на этой фикстуре (`M-77` §9bis):
/// сегодняшний код `0.994`, кандидат А с клоном на каждый batch `1.93`. Порог лежит
/// посередине с запасом в обе стороны и НЕ подобран под ответ: он отделяет «работа
/// пропорциональна СОБЫТИЯМ» от «работа пропорциональна БАТЧАМ».
const RATIO_CEILING: f64 = 1.35;

fn setup_failed(what: &str) -> ! {
    panic!("SETUP НЕ СОСТОЯЛСЯ: {what} — тест НЕ судил предмет, зелёное было бы вакуумом");
}

/// `max_segment_bytes` заведомо больше фикстуры: число СЕГМЕНТОВ обязано быть одинаковым в
/// обоих замерах, иначе оно само стало бы варьируемой величиной (`testing.md`
/// «Целостность гейта», свойство 2 — конфаундер держится КОНСТАНТНЫМ).
fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 256 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-77 pump cost".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// Селектор ПРОД-ФОРМЫ (`Р-2`): замер `docker-compose.yml:135,136,142,154` на `origin/main`.
fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001, 0.02],
        window_ms: Some(60_000),
        depth_cadence_ms: Some(1_000),
    }
}

/// Журнал с книгой в `LEVELS` уровней на сторону: уровни идут ВГЛУБЬ шагом 1.0 при mid
/// 65 005, то есть за пределы окна heatmap (прод-дефолт `0.001`) — состояние велико, выход мал.
fn journal_wide_book() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap_or_else(|e| setup_failed(&format!("tempdir: {e}")));
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg())
            .unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
        let bids: Vec<Level> = (0..LEVELS)
            .map(|i| lvl(MID_BID - i as f64, 1.0 + (i % 7) as f64))
            .collect();
        let asks: Vec<Level> = (0..LEVELS)
            .map(|i| lvl(MID_ASK + i as f64, 1.0 + (i % 5) as f64))
            .collect();
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids,
                asks,
                ts_exch_ms: BASE_MS,
            },
        ))
        .unwrap_or_else(|e| setup_failed(&format!("snapshot: {e}")));
        for i in 0..200_i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(MID_BID + (i % 5) as f64),
                    size: to_fixed(0.5),
                    side: if i % 3 == 0 { Side::Sell } else { Side::Buy },
                    ts_exch_ms: BASE_MS + i * 100,
                },
            ))
            .unwrap_or_else(|e| setup_failed(&format!("trade: {e}")));
        }
        j.flush()
            .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
    }
    dir
}

/// Тик из `TICK_EVENTS` дельт. ОДИНАКОВ в обоих замерах — вся разница между ними в том,
/// на сколько батчей `pump` его порежет.
fn append_tick(dir: &std::path::Path, n: u64) {
    let mut j = Journal::open_with(dir, writer_cfg())
        .unwrap_or_else(|e| setup_failed(&format!("open_with tick: {e}")));
    for k in 0..TICK_EVENTS {
        let seq = n * 1_000 + k + 1;
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Delta {
                bids: vec![lvl(MID_BID - 2.0, 3.0 + (k % 5) as f64)],
                asks: vec![lvl(MID_ASK + 2.0, 3.0 + (k % 5) as f64)],
                ts_exch_ms: BASE_MS + 20_000 + (n as i64) * 20_000 + (k as i64) * 120,
                first_update_id: seq,
                final_update_id: seq,
                prev_final_update_id: Some(seq.saturating_sub(1)),
            },
        ))
        .unwrap_or_else(|e| setup_failed(&format!("tick delta: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush tick: {e}")));
}

fn live_at_tail(dir: &std::path::Path) -> gateway::LiveReducer {
    let ckpt = tempfile::tempdir().unwrap_or_else(|e| setup_failed(&format!("ckpt: {e}")));
    let (mut live, _) =
        gateway::LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .unwrap_or_else(|e| setup_failed(&format!("resume: {e}")));
    for _ in 0..1_000 {
        match live.pump(dir, EpochFilter::OwnCaptureOnly, 4_096) {
            Ok((frames, _, _)) if frames.is_empty() => break,
            Ok(_) => continue,
            Err(e) => setup_failed(&format!("pump выхода на хвост: {e}")),
        }
    }
    std::mem::forget(ckpt); // каталог обязан пережить `live`
    live
}

/// **СТОРОЖ.** Стоимость `pump` не растёт вместе с ЧИСЛОМ БАТЧЕЙ, на которые он режет один
/// и тот же тик.
///
/// Развязка Б выбрана `C-211` именно потому, что не делает per-batch работы, пропорциональной
/// состоянию. Сторож краснеет против кандидата А и против любой будущей правки, вернувшей
/// такую работу на путь тика.
#[test]
fn vb_i_10_pump_cost_does_not_grow_with_the_number_of_batches() {
    // Оба замера — на ОДНОМ дереве и ОДНОМ редьюсере: состояние, книга и каталог общие,
    // варьируется единственная величина — `max_events`.
    let dir = journal_wide_book();
    let mut live = live_at_tail(dir.path());

    // Прогрев: первый тик после выхода на хвост тянет ленивые инициализации и первое
    // построение каталога сегментов — они к предмету не относятся.
    append_tick(dir.path(), 1);
    let warm = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 4_096)
        .unwrap_or_else(|e| setup_failed(&format!("прогревочный pump: {e}")));
    if warm.0.is_empty() {
        setup_failed("прогревочный pump не отдал кадра — тик не доехал, мерить нечего");
    }

    let mut run = |n: u64, max_events: usize| -> (usize, usize, usize) {
        append_tick(dir.path(), n);
        let (out, allocated) = total_delta(|| {
            live.pump(dir.path(), EpochFilter::OwnCaptureOnly, max_events)
                .unwrap_or_else(|e| setup_failed(&format!("измеряемый pump: {e}")))
        });
        let (frames, _, _) = out;
        let wire: usize = frames
            .iter()
            .map(|f| {
                serde_json::to_vec(&f.delta)
                    .unwrap_or_else(|e| setup_failed(&format!("сериализация кадра: {e}")))
                    .len()
            })
            .sum();
        (frames.len(), allocated, wire)
    };

    let (n_one, alloc_one, wire_one) = run(2, TICK_EVENTS as usize);
    let (n_many, alloc_many, wire_many) = run(3, 1);
    std::mem::forget(dir); // каталог обязан пережить `live`

    // ── SETUP-GUARD НА ФИКСТУРЕ, А НЕ НА ИСХОДЕ ─────────────────────────────────────
    // Сценарий существует, только если дробление РЕАЛЬНО состоялось. Guard проверяет
    // достижимость сценария, а не то, что реализация повела себя ожидаемо (урок `P6`:
    // guard на исход краснел против ПРАВИЛЬНОГО фикса).
    if n_one != 1 {
        setup_failed(&format!(
            "при max_events={TICK_EVENTS} тик обязан лечь в ОДИН кадр, получено {n_one} — \
             плечо оси не построено"
        ));
    }
    if n_many < TICK_EVENTS as usize / 2 {
        setup_failed(&format!(
            "при max_events=1 тик обязан дать около {TICK_EVENTS} кадров, получено {n_many} — \
             дробление не состоялось, ось не варьируется"
        ));
    }
    if alloc_one == 0 {
        setup_failed("аллокаций НОЛЬ — счётчик не работает, замер вакуумен");
    }
    // Рост ВЫХОДА не имеет права объяснять рост аллокаций: полезная нагрузка обязана быть
    // пренебрежимой на фоне измеряемой величины, иначе оракул мерил бы сериализацию.
    let wire_share = wire_many as f64 / alloc_many as f64;
    if wire_share > 0.10 {
        setup_failed(&format!(
            "выход дроблёного тика ({wire_many} Б) составляет {:.1} % аллокаций \
             ({alloc_many} Б) — рост объясним полезной нагрузкой, а не per-batch работой; \
             фикстура не изолирует предмет",
            wire_share * 100.0
        ));
    }

    let ratio = alloc_many as f64 / alloc_one as f64;
    assert!(
        ratio < RATIO_CEILING,
        "VB-I-10 НАРУШЕН НА ГРАНИЦЕ `pump` (M-77 задача 4): ОДИН И ТОТ ЖЕ тик из \
         {TICK_EVENTS} событий при книге в {LEVELS} уровней на сторону стоит \
         {alloc_one} Б одним кадром ({n_one} кадр, выход {wire_one} Б) и {alloc_many} Б \
         при дроблении на {n_many} кадров (выход {wire_many} Б) — отношение {ratio:.2} при \
         потолке {RATIO_CEILING}. Работа стала пропорциональна ЧИСЛУ БАТЧЕЙ, а не числу \
         событий: это ровно та цена, за которую `C-211` отверг развязку А («клон самого \
         дорогого поля на КАЖДЫЙ batch»), и та, которую развязка Б обязана не платить. \
         Событий в тике поровну в обоих замерах, книга и каталог общие — варьировался \
         ТОЛЬКО `max_events`. Смежные предметы: `M-56`/`TD-097` (клон состояния на пути \
         построения ответа), `VB-I-10` (`docs/fa/viz-backend.md:207`)."
    );
}
