//! RED `F-036` / `VB-I-10` (sacred, architect-only) — **ЦЕНА ТИКА НЕ ЗАВИСИТ ОТ ДЛИНЫ
//! АКТИВНОГО СЕГМЕНТА, И МЕРИТСЯ ЭТО ФАКТИЧЕСКИМ ЧТЕНИЕМ, А НЕ СЧЁТЧИКОМ УЧАСТНИКА.**
//!
//! Милестоун `milestones/M-71-egress-cap.md`. Исполнение вердикта
//! `research/reviews/R-143-M-71-egress-cap-impl-rev6-r4.md` блокер **`B-1`**: транзакционный
//! `commit_batch` вернул на тиковый путь `journal::stream_from`, то есть полный перескан
//! активного сегмента — ровно P0-дефект, ради устранения которого существовали `M-57`
//! (`TD-109`) и `M-62` (`TD-120`).
//!
//! # Почему НОВЫЙ оракул, а не правка `red_tail_cursor_prod_form.rs`
//!
//! Существующие `f035_*` и `SM-*` мерят `ReadStats` / `segment_meta_ops`. Эта мера ведётся
//! ТОЛЬКО первичным стримом: `read_stats_from_stream(&stream)`
//! (`crates/gateway/src/lib.rs:3551`), а старый API `stream_from` кладёт
//! `segment_meta_ops: 0` с явным комментарием (`crates/journal/src/segments.rs:1903`).
//! Поэтому оба оракула остаются ЗЕЛЁНЫМИ на цифрах `events_scanned=3`,
//! `segment_meta_ops=3` — при 1.7 МБ фактического чтения. Это `TD-148` (MAJOR) дословно:
//! «регресс к O(N) на такте не покраснит НИЧЕГО».
//!
//! # Правило конструкции, которое этот оракул исполняет (Р-1)
//!
//! `docs/workflow/oracle-blindness-class-2026-08-28.md` §5: **мера снимается на границе
//! ПОТРЕБИТЕЛЯ, а не с внутреннего участника.** `rchar` из `/proc/self/io` — байты, реально
//! прочитанные ПРОЦЕССОМ вызовами `read`. Он считает и первичный стрим, и второй, и десятый,
//! потому что не знает о них ничего; счётчик, который ведёт один из путей, устойчивости к
//! добавлению второго пути не имеет по построению. Та же мера, которой ревьюер предъявил
//! `B-1` (×25.9 при 32 000 событий в сегменте).
//!
//! Это же и норма `testing.md` §«Оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ» п.2 — «оракул
//! границы ресурса меряет ресурс, а не прокси» — написанная задолго до этого дефекта и им
//! нарушенная дословно.
//!
//! # Что судится и чего оракул НЕ предписывает
//!
//! Судится ОТНОШЕНИЕ двух замеров, различающихся ТОЛЬКО длиной активного сегмента (×4).
//! Абсолютные байты не пиннятся намеренно: они зависят от размера кадра, буферизации и
//! файловой системы, а инвариант `VB-I-10` — про ЗАВИСИМОСТЬ, а не про величину. Конструкцию
//! (курсор, каталог сегментов, hint) оракул не выбирает: он краснеет против ЛЮБОЙ, где
//! бюджет тика растёт с длиной сегмента.
//!
//! # Предел, названный честно
//!
//! `rchar` — процессная величина, и замер обязан быть единственным в процессе в свой момент:
//! отсюда `serial()` на КАЖДОМ тесте файла (`testing.md` §«Целостность гейта» св. 2). Тесты
//! разных файлов живут в РАЗНЫХ процессах (Rust собирает бинарь на файл), поэтому соседние
//! файлы замеру не мешают. `rchar` учитывает page-cache-попадания как чтение вызовом `read`,
//! то есть меряет РАБОТУ, а не обращения к диску — для инварианта «бюджет тика» это верная
//! величина, и она названа, а не подразумевается.
//!
//! Оракул Linux-специфичен. На платформе без `/proc/self/io` он объявляет
//! **SETUP НЕ СОСТОЯЛСЯ**, а не тихо зеленеет: гейт, зеленеющий от исчезновения предмета, —
//! не гейт (`testing.md` §«Целостность гейта» св. 4).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{LiveReducer, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Прод-константа партии: `PUSH_MAX_EVENTS` / `LEGACY_DRAIN_BATCH`
/// (`crates/gateway-serve/src/lib.rs:1120`, `:1411`).
const PUMP_BATCH: usize = 256;

/// Событий в тике. Прод-форма: приращение мало, сегмент велик — именно на этом контрасте
/// дефект и виден (тик из ТРЁХ событий читал журнал целиком).
const TICK_EVENTS: usize = 3;

/// Две точки замера. Различаются РОВНО в `N_LONG / N_SHORT` раз; всё остальное — константа
/// (`testing.md`: конфаундинг-величину держать КОНСТАНТНОЙ, варьировать только измеряемую).
const N_SHORT: usize = 8_000;
const N_LONG: usize = 32_000;

/// Допуск отношения. Замер ревьюера на `cde723d`: 65 875 Б → 518 853 Б (короткий сегмент) и
/// 65 880 Б → 1 703 434 Б (длинный), то есть у ЧЕСТНОЙ реализации отношение ≈ 1.00, у
/// дефектной ≈ 3.28 при четырёхкратной разнице длины. Порог 1.5 разделяет их с запасом в обе
/// стороны и не требует от реализации константы до байта: буферизация чтения даёт разброс.
const MAX_RATIO: f64 = 1.5;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "F-036 tick read cost fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
    }
}

/// Замер обязан быть единственным в процессе в свой момент: `rchar` считает ВЕСЬ процесс.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о цене тика: фикстура не воспроизвела \
         сценарий, ради которого оракул написан."
    )
}

/// Байты, реально прочитанные процессом вызовами `read` (Linux `/proc/self/io`).
fn rchar() -> u64 {
    let text = std::fs::read_to_string("/proc/self/io").unwrap_or_else(|e| {
        setup_failed(&format!(
            "/proc/self/io недоступен ({e}) — меры границы процесса нет, и подменять её \
             счётчиком участника запрещено (это и есть предмет B-1)"
        ))
    });
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("rchar:") {
            return v
                .trim()
                .parse::<u64>()
                .unwrap_or_else(|e| setup_failed(&format!("rchar не разобран из «{line}»: {e}")));
        }
    }
    setup_failed("в /proc/self/io нет строки rchar")
}

fn append_trades(j: &mut Journal, from: usize, count: usize) {
    for i in from..(from + count) {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID + i as f64 * 0.01),
                size: to_fixed(1.0),
                side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                ts_exch_ms: T0 + i as i64,
            },
        ))
        .unwrap_or_else(|e| setup_failed(&format!("append #{i}: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
}

/// Число сегментов на диске — прод-форма `B-1` требует РОВНО ОДИН активный сегмент, иначе
/// замер смешивает «цену длины сегмента» с «ценой их числа» (это разные инварианты: первый —
/// `M-57`, второй — `M-62`).
fn segment_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.ends_with(".jrnl") || n.ends_with(".jrnl.zst")
                })
                .count()
        })
        .unwrap_or(0)
}

/// Байты, прочитанные процессом за ОДИН тик на журнале из `prefill` событий.
///
/// Шаги повторяют прод: сессия поднимается (`resume`), догоняет хвост, и только ПОСЛЕ этого
/// приходит приращение. Замер берётся вокруг ОДНОГО `pump` — того самого вызова, который на
/// проде происходит на каждом закрытии батча.
fn tick_read_bytes(prefill: usize) -> (u64, usize) {
    let dir = tempfile::tempdir().unwrap_or_else(|e| setup_failed(&format!("tempdir: {e}")));
    let mut j = Journal::open_with(dir.path(), cfg())
        .unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
    append_trades(&mut j, 0, prefill);

    let ckpt = tempfile::tempdir().unwrap_or_else(|e| setup_failed(&format!("ckpt tempdir: {e}")));
    let (mut r, _) =
        LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .unwrap_or_else(|e| setup_failed(&format!("resume: {e}")));

    // Догон: сессия обязана стоять В КОНЦЕ хвоста, иначе замеряется догон, а не тик.
    let mut caught = 0usize;
    loop {
        match r.pump(dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH) {
            Ok((frames, _, _)) if frames.is_empty() => break,
            Ok((frames, _, _)) => caught += frames.len(),
            Err(e) => setup_failed(&format!("догон отказал на prefill={prefill}: {e}")),
        }
    }
    if caught == 0 {
        setup_failed(&format!(
            "prefill={prefill}: догон не отдал ни одного кадра — сессия не наполнилась, и \
             последующий «тик» мерил бы пустоту"
        ));
    }

    let segs = segment_files(dir.path());
    if segs != 1 {
        setup_failed(&format!(
            "prefill={prefill}: сегментов {segs}, а прод-форма B-1 требует РОВНО одного \
             активного — иначе замер смешивает длину сегмента с их числом"
        ));
    }

    // Приращение тика — ровно TICK_EVENTS событий.
    append_trades(&mut j, prefill, TICK_EVENTS);

    let before = rchar();
    let (frames, _, _) = r
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, PUMP_BATCH)
        .unwrap_or_else(|e| {
            setup_failed(&format!("тиковый pump отказал на prefill={prefill}: {e}"))
        });
    let after = rchar();

    if frames.is_empty() {
        setup_failed(&format!(
            "prefill={prefill}: тиковый pump не увидел дописанных {TICK_EVENTS} событий — \
             мерился ХОЛОСТОЙ вызов, а не тик"
        ));
    }
    let delta = after.saturating_sub(before);
    if delta == 0 {
        setup_failed(&format!(
            "prefill={prefill}: rchar не изменился за тик — мера ничего не наблюдает, и \
             зелёное такого оракула не значит ничего"
        ));
    }
    (delta, frames.len())
}

// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`F-036` — бюджет тика не растёт с длиной активного сегмента.**
///
/// Красный против `f50aac7` (транзакционный `commit_batch` с `journal::stream_from`), зелёный
/// против конструкции `M-57`/`M-62`, где работа тика пропорциональна ПРИРАЩЕНИЮ.
///
/// Прямо пиннит то, что `TD-117` назвал незапиненным: «если один из вызовов `stream_from`
/// окажется на тиковом пути, гейт смолчит». Больше не смолчит — потому что мера не спрашивает
/// у кода, каким путём он читал.
#[test]
fn f036_tick_read_cost_is_independent_of_active_segment_length() {
    let _g = serial();

    let (short_bytes, short_frames) = tick_read_bytes(N_SHORT);
    let (long_bytes, long_frames) = tick_read_bytes(N_LONG);

    // SETUP-GUARD: тик обязан быть ОДИНАКОВЫМ на обеих точках, иначе сравниваются разные вещи.
    if short_frames != long_frames {
        setup_failed(&format!(
            "кадров за тик: {short_frames} при N={N_SHORT} против {long_frames} при \
             N={N_LONG} — работа тика различается САМА, и отношение байтов уже не про длину \
             сегмента"
        ));
    }

    let ratio = long_bytes as f64 / short_bytes.max(1) as f64;
    let n_ratio = N_LONG as f64 / N_SHORT as f64;

    assert!(
        ratio <= MAX_RATIO,
        "F-036 / VB-I-10 (R-143 B-1): тик из {TICK_EVENTS} событий прочитал {long_bytes} Б при \
         активном сегменте в {N_LONG} событий против {short_bytes} Б при {N_SHORT} — отношение \
         {ratio:.2}× при {n_ratio:.0}-кратной разнице длины сегмента (допуск {MAX_RATIO:.1}×). \
         Бюджет тика пропорционален ДЛИНЕ СЕГМЕНТА, а не приращению: это возврат P0-дефекта, \
         ради устранения которого существовали M-57 (TD-109) и M-62 (TD-120). \
         PROJECT-STATE.md: «работа тика становится пропорциональна приращению, а не длине \
         сегмента». Мера — rchar из /proc/self/io, то есть ФАКТИЧЕСКОЕ чтение процесса: \
         ReadStats и segment_meta_ops к добавленному стриму слепы по построению (TD-148), и \
         зелёное на них ничего не доказывает."
    );
}
