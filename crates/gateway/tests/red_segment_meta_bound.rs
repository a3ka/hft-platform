//! `SM-0..SM-6` — цена тика не зависит от ЧИСЛА СЕГМЕНТОВ (M-62, `A-004` §C.2 / `TD-120`).
//!
//! ЗАЧЕМ. M-57 снял пересканирование АКТИВНОГО сегмента, но под ним обнажился пол ≈19 % CPU
//! на сессию, к пересканированию отношения не имеющий (`R-044` §D). Замер по коду:
//! `journal::stream_from_at` на КАЖДОМ вызове зовёт `segments(dir)`
//! (`crates/journal/src/segments.rs:1354`), а тот на каждом тике делает `read_dir` всего
//! каталога плюс `classify_segment` на КАЖДЫЙ сегмент — `fs::metadata`, а для сырого ещё и
//! ДВА `File::open` (`:545`, `:514`, `:561`), для сжатого — `File::open` + инициализацию
//! zstd-декодера (`:614`). На проде 205 сегментов, и число растёт каждые ~2 часа НАВСЕГДА:
//! стоимость тика привязана к величине, которая никогда не убывает.
//!
//! ПОЧЕМУ ЭТОГО НЕ ВИДЯТ СУЩЕСТВУЮЩИЕ ОРАКУЛЫ. Все три счётчика `ReadStats`
//! (`events_decoded`, `events_scanned`, `segments_opened`) инкрементируются ПОСЛЕ того, как
//! каталог уже обойдён. `segments_opened` считает открытия в `open_next_segment`
//! (`segments.rs:1081`), то есть сегменты, оставшиеся после фильтрации: при hint'е у хвоста
//! он равен 1, тогда как `classify_segment` отработал 205 раз. Замер `red_frames_seek_bound`
//! это уже зафиксировал: `segments_opened` = 1 и при 1 000 событий, и при 8 000, «счётчик
//! обхода каталога не видит по конструкции».
//!
//! ПОЧЕМУ МЕРА — СЧЁТЧИК, А НЕ ВРЕМЯ. Форма «тик быстрее X мс» дала в линии M-53→M-57 шесть
//! слепых оракулов, и дважды слепота сидела в самой мере: `td083` мерил ОТНОШЕНИЕ времён и
//! инвертировал знак (`A-004`: мутант зелёный 2.3–2.7, здоровый красный 4.9–6.2), плюс флак
//! от загрузки машины. Здесь — детерминированный счётчик операций с АБСОЛЮТНЫМ бюджетом:
//! удешевление любой другой части системы его не двигает.
//!
//! СОСТОЯНИЕ НАБОРА: RED. `ReadStats::segment_meta_ops` не существует — его вводит задача 1
//! (engine-dev). До неё файл НЕ КОМПИЛИРУЕТСЯ, и это и есть красный: спецификация написана
//! раньше кода (`gates.md` §2).
//!
//! ГДЕ ОСТАЛЬНОЕ. Оракулы guard'а `hint.pos` (задача 4, §7 спеки) вынесены в отдельный файл
//! `red_hint_pos_guard.rs` СОЗНАТЕЛЬНО: они не зависят от счётчика и обязаны быть
//! запускаемыми СЕГОДНЯ. Держать их здесь значило бы заблокировать проверку задачи 4 до
//! выполнения задачи 1 — дефект компоновки, а не набора.
//!
//! АНТИ-ПЛАЦЕБО В ОБЕ СТОРОНЫ (§4.4 спеки). Набор обязан краснеть против `nocache`
//! (сегодняшний `origin/main` и есть эта реализация), `staleforever`, `dirshared`,
//! `countfake` — и НЕ краснеть против честного удешевления. Мутант `countfake` не ловится
//! верхним порогом в принципе: реализация, инкрементирующая счётчик раз за вызов
//! `segments()`, проходит любое «≤ N». Его ловит ПОЗИТИВНЫЙ ассерт `sm1` и `sm3`: первый тик
//! обязан показать `>= число сегментов`.

use std::fs;
use std::path::Path;

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::Selector;
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;

/// Прод-масштаб воспроизводится ПО ЧИСЛУ сегментов, а не по объёму: 205 сегментов на проде
/// (замер §1.1 спеки), 473 MB активного воспроизводить незачем — прецедент
/// `red_det_prodscale.rs` («кап прода 1 GiB воспроизводить незачем»). Мелкий сегмент даёт то
/// же N дёшево: ~200 сегментов ≈ 30 KB на диске.
const SEG_BYTES: u64 = 256;
/// Целевое число сегментов. Ось 2: реализация может быть O(N) с малым коэффициентом и
/// пройти на трёх сегментах — на 200 не пройдёт.
const N_SEGMENTS: usize = 200;
/// Приращение между тиками — столько recorder успевает записать за период push'а (250 мс).
const INCREMENT: u64 = 3;
/// АБСОЛЮТНЫЙ бюджет операций с метаданными на УСТАНОВИВШЕМСЯ тике. Число выбрано так, чтобы
/// покрыть честную реализацию (дешёвая проба каталога + открытие активного сегмента) и не
/// покрыть O(N): при N=200 разница в 25 раз, порог не на границе.
const BUDGET_META: u64 = 8;
/// Бюджет ПРОЧИТАННЫХ событий там, где проверяется, что тик не свалился в полный скан.
const BUDGET_SCANNED: u64 = INCREMENT * 4;

// ─────────────────────────────────────────────────────────────────────────────────────────
// Манифест покрытия. Сверка «манифест ⇄ исполнение» — в каждом оракуле (`claims`), сверка
// «манифест ⇄ таблица §4.2 спеки» — в `scripts/verify_M-62.sh`. Число сценариев СЧИТАЕТСЯ
// (`MANIFEST.len()`), а не заявляется литералом: урок `TD-125` («ci.yml называет 26 при 27»).
// ─────────────────────────────────────────────────────────────────────────────────────────

/// `(id оракула, ось, значение, вид)`; вид: `V` — нарушение (обязано краснеть),
/// `L` — легитимный случай (обязан зеленеть).
const MANIFEST: &[(&str, u8, &str, char)] = &[
    ("sm1", 1, "счётчик считает не то", 'V'),
    ("sm1", 1, "meta-ops растут с N", 'V'),
    ("sm1", 1, "meta-ops постоянны", 'L'),
    ("sm2", 2, "N прод-масштаба (200+)", 'V'),
    ("sm2", 2, "N после роста втрое", 'V'),
    ("sm2", 3, "установившийся тик платит за все сегменты", 'V'),
    ("sm3", 3, "первый тик сессии платит полную цену", 'L'),
    ("sm3", 2, "N мал (2-3)", 'L'),
    ("sm4", 4, "новый сегмент не замечен", 'V'),
    ("sm4", 3, "тик после ротации платит полную цену", 'L'),
    ("sm5", 4, "компакция не замечена", 'V'),
    ("sm5", 4, "каталог не менялся — переучёта нет", 'L'),
    ("sm6", 5, "состояние общее на каталог", 'V'),
    ("sm6", 5, "две сессии независимы", 'L'),
];

/// Сверка «исполнение ⇒ манифест»: оракул объявляет, какое значение он покрывает, и падает,
/// если такого значения в манифесте нет. Обратное направление («манифест ⇒ исполнение»)
/// проверяет `verify_M-62.sh` грепом имён по этому файлу.
fn claims(id: &str, axis: u8, value: &str) {
    assert!(
        MANIFEST
            .iter()
            .any(|(i, a, v, _)| *i == id && *a == axis && *v == value),
        "МАНИФЕСТ НЕ СОДЕРЖИТ заявленного покрытия: {id} / ось {axis} / «{value}». \
         Оракул, покрывающий значение вне манифеста, делает перечень осей ложью."
    );
}

#[test]
fn sm0_manifest_covers_every_axis_in_both_directions() {
    for axis in 1..=5u8 {
        let v = MANIFEST
            .iter()
            .filter(|(_, a, _, k)| *a == axis && *k == 'V')
            .count();
        let l = MANIFEST
            .iter()
            .filter(|(_, a, _, k)| *a == axis && *k == 'L')
            .count();
        assert!(
            v >= 1,
            "ось {axis} не имеет НИ ОДНОГО значения-нарушения: перечень осей §4.2 объявлен, \
             но не исполняется"
        );
        assert!(
            l >= 1,
            "ось {axis} не имеет легитимного значения. Без него набор проходит реализация \
             «запретить всё» — для оси 3 это «вообще не смотреть на каталог», и сессия \
             перестанет видеть новые сегменты (§4.1)"
        );
    }
    eprintln!(
        "MANIFEST: {} сценариев (число СЧИТАНО, не заявлено)",
        MANIFEST.len()
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// Фикстуры
// ─────────────────────────────────────────────────────────────────────────────────────────

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m62".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(100.0 + (i % 5) as f64),
            size: to_fixed(1.0 + (i % 3) as f64),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: D2_MS + (i as i64 * 100),
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
    }
}

fn append_range(dir: &Path, from: u64, to: u64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in from..to {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
}

fn n_segments(dir: &Path) -> usize {
    journal::list_segments(dir).expect("list_segments").len()
}

/// Каталог прод-ФОРМЫ: `target` сегментов, СМЕШАННЫХ raw + `.zst`.
///
/// Смешанность обязательна (advisory `C-072`): на проде 198 из 205 сжаты, и `classify_segment`
/// для сжатого идёт другой веткой (`File::open` + zstd-декодер) — фикстура из одних raw
/// меряла бы не ту работу.
fn build_prod_form(target: usize) -> (tempfile::TempDir, u64) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut n = 0u64;
    while n_segments(dir.path()) < target {
        append_range(dir.path(), n, n + 32);
        n += 32;
    }
    journal::compact_closed_segments(dir.path(), 4, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact_closed_segments");

    // СТРАЖ SETUP'а: без него проба молча мерила бы не тот каталог.
    let names: Vec<String> = fs::read_dir(dir.path())
        .expect("read_dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    let raw = names.iter().filter(|s| s.ends_with(".jrnl")).count();
    let zst = names.iter().filter(|s| s.ends_with(".jrnl.zst")).count();
    assert!(
        n_segments(dir.path()) >= target,
        "SETUP НЕ СОСТОЯЛСЯ: сегментов {} при цели {target}",
        n_segments(dir.path())
    );
    assert!(
        raw > 0 && zst > 0,
        "SETUP НЕ СОСТОЯЛСЯ: каталог не СМЕШАННЫЙ (raw={raw}, zst={zst}). Прод — 198 сжатых \
         из 205; фикстура из одних raw не воспроизводит ветку classify_compacted_segment"
    );
    (dir, n)
}

fn new_session(dir: &Path, ckpt: &Path) -> gateway::LiveReducer {
    let s = sel();
    let (live, _st) =
        gateway::LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &s, ckpt).expect("resume");
    live
}

fn catch_up(live: &mut gateway::LiveReducer, dir: &Path) {
    while let Ok((frames, _c, _st)) = live.pump(dir, EpochFilter::OwnCaptureOnly, 10_000) {
        if frames.is_empty() {
            break;
        }
    }
}

/// Независимый эталон: полный проход журнала БЕЗ курсора и без кеша.
/// Сверять выдачу с самим же seek-путём нельзя — это тавтология (`testing.md`).
fn reference_seqs(dir: &Path, after: Option<u64>) -> Vec<u64> {
    let mut out = Vec::new();
    let mut s = journal::stream_from(dir, EpochFilter::OwnCaptureOnly, after).expect("stream_from");
    for ev in s.by_ref() {
        out.push(ev.expect("event").seq);
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// SM-1 — ось 1: мера считает РЕСУРС, а не прокси
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn sm1_counter_measures_operations_not_calls() {
    claims("sm1", 1, "счётчик считает не то");
    claims("sm1", 1, "meta-ops растут с N");
    claims("sm1", 1, "meta-ops постоянны");

    let (big, _n_big) = build_prod_form(N_SEGMENTS);
    let small = tempfile::tempdir().expect("tempdir");
    append_range(small.path(), 0, 6);

    let ck_b = tempfile::tempdir().expect("ck");
    let ck_s = tempfile::tempdir().expect("ck");
    let mut lb = new_session(big.path(), ck_b.path());
    let mut ls = new_session(small.path(), ck_s.path());

    let (_f, _c, sb) = lb
        .pump(big.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump big");
    let (_f, _c, ss) = ls
        .pump(small.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump small");

    let n_big = n_segments(big.path()) as u64;
    let n_small = n_segments(small.path()) as u64;
    eprintln!(
        "SM-1: big N={n_big} meta_ops={} · small N={n_small} meta_ops={}",
        sb.segment_meta_ops, ss.segment_meta_ops
    );

    // ПОЗИТИВНЫЙ ассерт — единственное, что ловит `countfake`. Реализация, считающая вызовы
    // `segments()` вместо операций, покажет здесь 1 и провалится.
    assert!(
        sb.segment_meta_ops >= n_big,
        "SM-1: ПЕРВЫЙ тик на каталоге из {n_big} сегментов показал segment_meta_ops={}, \
         что МЕНЬШЕ числа сегментов. Первый тик обязан реально обойти каталог: он читает \
         манифест, делает read_dir и классифицирует КАЖДЫЙ сегмент (metadata + один-два \
         open). Счётчик, показывающий меньше, считает не операции, а вызовы — мутант \
         `countfake`, и верхний порог его не ловит в принципе.",
        sb.segment_meta_ops
    );
    assert!(
        sb.segment_meta_ops > ss.segment_meta_ops,
        "SM-1: каталоги в {}× по числу сегментов дали одинаковую цену обхода ({} против {}). \
         Мера слепа к предмету: ровно так `segments_opened` показывал 1 и при 1 000, и при \
         8 000 событий (`red_frames_seek_bound` §замер).",
        n_big / n_small.max(1),
        sb.segment_meta_ops,
        ss.segment_meta_ops
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// SM-2 — оси 2+3: установившийся тик при ПРОД-масштабе N
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn sm2_steady_tick_is_independent_of_segment_count() {
    claims("sm2", 2, "N прод-масштаба (200+)");
    claims("sm2", 2, "N после роста втрое");
    claims("sm2", 3, "установившийся тик платит за все сегменты");

    let (dir, n) = build_prod_form(N_SEGMENTS);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let mut live = new_session(dir.path(), ckpt.path());
    catch_up(&mut live, dir.path());

    // Установившийся режим: сессия уже сделала тик, состав каталога с тех пор не менялся,
    // кроме append'а в АКТИВНЫЙ сегмент (ровно то, что делает recorder между push'ами).
    append_range(dir.path(), n, n + INCREMENT);
    let (_f, _c, st) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump");

    let n_seg = n_segments(dir.path()) as u64;
    eprintln!(
        "SM-2: N={n_seg} meta_ops={} scanned={}",
        st.segment_meta_ops, st.events_scanned
    );
    assert!(
        st.segment_meta_ops <= BUDGET_META,
        "SM-2: установившийся тик при N={n_seg} сегментов выполнил {} операций с \
         метаданными при бюджете {BUDGET_META}. Это O(N): на проде N=205 и растёт каждые \
         ~2 часа НАВСЕГДА, то есть цена тика привязана к величине, которая не убывает. При \
         цели 10 000 сессий и периоде 250 мс — порядка 16 млн syscall'ов в секунду только на \
         метаданные. Перечень сегментов обязан жить в памяти СЕССИИ и переиспользоваться, \
         пока состав каталога не изменился.",
        st.segment_meta_ops
    );
    assert!(
        st.events_scanned <= BUDGET_SCANNED,
        "SM-2: тик прочитал {} событий при приращении {INCREMENT} — сломан курсор хвоста \
         M-57, а не только метаданные-путь. Цена M-62 не имеет права быть уплачена соседним \
         инвариантом (§5 запретного списка).",
        st.events_scanned
    );

    // ── ось 2, второе значение: N ВЫРОС ВТРОЕ ────────────────────────────────────────────
    // Абсолютный бюджет проверяет «не больше X». Он не отличает O(1) от O(N) с крошечным
    // коэффициентом, попавшим под порог. Отличает только СРАВНЕНИЕ двух N: цена
    // установившегося тика не имеет права вырасти вместе с каталогом.
    let n_before = n_segments(dir.path()) as u64;
    let mut m = n + INCREMENT;
    while (n_segments(dir.path()) as u64) < n_before * 3 {
        append_range(dir.path(), m, m + 64);
        m += 64;
    }
    let n_after = n_segments(dir.path()) as u64;
    assert!(
        n_after >= n_before * 3,
        "SETUP НЕ СОСТОЯЛСЯ: сегментов {n_after} при цели {}",
        n_before * 3
    );
    catch_up(&mut live, dir.path());
    append_range(dir.path(), m, m + INCREMENT);
    let (_f, _c, grown) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump grown");

    eprintln!(
        "SM-2: N {n_before} -> {n_after}; meta_ops {} -> {}",
        st.segment_meta_ops, grown.segment_meta_ops
    );
    assert!(
        grown.segment_meta_ops <= BUDGET_META,
        "SM-2: после роста каталога втрое ({n_before} -> {n_after}) установившийся тик \
         выполнил {} операций при бюджете {BUDGET_META}. Журнал append-only: N растёт \
         НАВСЕГДА, и реализация, зависящая от него, откладывает отказ, а не устраняет.",
        grown.segment_meta_ops
    );
    assert!(
        grown.segment_meta_ops <= st.segment_meta_ops,
        "SM-2: цена установившегося тика ВЫРОСЛА вместе с каталогом ({} при N={n_before} -> \
         {} при N={n_after}), пусть и в пределах бюджета. Это O(N) с малым коэффициентом: \
         сегодня проходит порог, через месяц роста — нет. Инвариант §4.1 требует ЧИСЛА, НЕ \
         ЗАВИСЯЩЕГО от N, а не «пока помещается».",
        st.segment_meta_ops,
        grown.segment_meta_ops
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// SM-3 — ось 3, ЛЕГИТИМНЫЕ значения: первый тик платит полную цену, и это законно
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn sm3_first_tick_legitimately_pays_full_price() {
    claims("sm3", 3, "первый тик сессии платит полную цену");
    claims("sm3", 2, "N мал (2-3)");

    let (dir, _n) = build_prod_form(N_SEGMENTS);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let mut live = new_session(dir.path(), ckpt.path());

    let (_f, _c, first) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump 1");
    let n_seg = n_segments(dir.path()) as u64;

    assert!(
        first.segment_meta_ops >= n_seg,
        "SM-3: первый тик сессии показал {} операций при {n_seg} сегментах. Он ОБЯЗАН обойти \
         каталог целиком — иначе реализация не строит перечень вовсе, а «оптимизация», \
         которая не смотрит на каталог, перестаёт замечать новые сегменты (§4.1). Это \
         легитимное значение оси 3, и оно обязано ЗЕЛЕНЕТЬ; красный здесь означает, что \
         кеш подменил собой наблюдение.",
        first.segment_meta_ops
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// SM-4 — ось 4: новый сегмент между тиками обязан быть замечен
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn sm4_new_segment_between_ticks_is_noticed() {
    claims("sm4", 4, "новый сегмент не замечен");
    claims("sm4", 3, "тик после ротации платит полную цену");

    let (dir, n) = build_prod_form(N_SEGMENTS);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let mut live = new_session(dir.path(), ckpt.path());
    catch_up(&mut live, dir.path());

    let before = n_segments(dir.path());
    // При max_segment_bytes=256 этот append гарантированно вызывает ротацию.
    append_range(dir.path(), n, n + 64);
    let after = n_segments(dir.path());
    assert!(
        after > before,
        "SETUP НЕ СОСТОЯЛСЯ: ротации не произошло ({before} → {after}); сценарий оси 4 не \
         воспроизведён, и тест проверял бы не то"
    );

    let mut got: Vec<u64> = Vec::new();
    for _ in 0..8 {
        let (frames, _c, _st) = live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 4_096)
            .expect("pump");
        if frames.is_empty() {
            break;
        }
        got.push(frames.len() as u64);
    }

    let expect = reference_seqs(dir.path(), None).len() as u64;
    let seen: u64 = got.iter().sum::<u64>();
    assert!(
        seen > 0,
        "SM-4: после ротации сессия не получила НИ ОДНОГО кадра, хотя в журнале {expect} \
         событий и появился новый сегмент ({before} → {after}). Кеш перечня сегментов живёт \
         без инвалидации — мутант `staleforever`. Тихая деградация при зелёном healthcheck: \
         сессия молча перестаёт отдавать данные, тот же класс, что `TD-031`."
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// SM-5 — ось 4: КОМПАКЦИЯ (raw → .zst с УДАЛЕНИЕМ raw) замечена; неизменный каталог — нет
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn sm5_compaction_is_noticed_and_quiet_dir_is_not_rescanned() {
    claims("sm5", 4, "компакция не замечена");
    claims("sm5", 4, "каталог не менялся — переучёта нет");

    let (dir, n) = build_prod_form(N_SEGMENTS);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let mut live = new_session(dir.path(), ckpt.path());
    catch_up(&mut live, dir.path());

    // (1) ЛЕГИТИМНОЕ значение: каталог не менялся — установившийся тик дёшев.
    append_range(dir.path(), n, n + INCREMENT);
    let (_f, _c, quiet) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump quiet");
    assert!(
        quiet.segment_meta_ops <= BUDGET_META,
        "SM-5: каталог не менялся, а тик всё равно выполнил {} операций (бюджет \
         {BUDGET_META}) — переучёт без события. Это и есть предмет milestone'а.",
        quiet.segment_meta_ops
    );

    // (2) НАРУШЕНИЕ: компакция. Она обязана быть НАСТОЯЩЕЙ — с исчезновением `.jrnl`.
    // При коллизии индексов `dedup_indexed_paths` выбирает СЫРОЙ сегмент (`D-COMP-1`),
    // поэтому каталог, где `.zst` лежит РЯДОМ с `.jrnl`, даёт тот же состав, что до
    // компакции: оракул стал бы плацебо (§4.5(б)).
    let raw_before: Vec<String> = fs::read_dir(dir.path())
        .expect("read_dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|s| s.ends_with(".jrnl"))
        .collect();
    journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact");
    let raw_after: Vec<String> = fs::read_dir(dir.path())
        .expect("read_dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|s| s.ends_with(".jrnl"))
        .collect();
    assert!(
        raw_after.len() < raw_before.len(),
        "SETUP НЕ СОСТОЯЛСЯ: сырых сегментов было {}, стало {} — компакция не удалила ни \
         одного `.jrnl`. Событие оси 4 не воспроизведено: при коллизии индексов побеждает \
         СЫРОЙ сегмент, состав `segments()` не изменился, и любой ассерт ниже был бы \
         плацебо (§4.5(б)).",
        raw_before.len(),
        raw_after.len()
    );

    append_range(dir.path(), n + INCREMENT, n + 2 * INCREMENT);
    let (_f, _c, _after) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("SM-5: тик после компакции обязан работать, а не падать");

    let expect = reference_seqs(dir.path(), None);
    assert!(
        !expect.is_empty(),
        "SM-5: после компакции независимый эталон пуст — журнал стал нечитаем"
    );

    // (3) «ЗАМЕЧЕНА» проверяется ПОВЕДЕНИЕМ, а не тем, что тик не упал.
    //
    // Батарея мутантов (задача 7) предъявила: до этой половины `staleforever` — кеш,
    // НИКОГДА не инвалидирующийся, — проходил SM-5 целиком. Причина в том, что сессия
    // `live` к моменту компакции уже ушла в хвост: закрытые сегменты ей больше не нужны,
    // и устаревший перечень её не задевает. Тест обещал «компакция замечена», а мерил
    // «тик не паникует» — ровно класс «оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ».
    //
    // Наблюдаемой компакция становится для ОТСТАВШЕЙ сессии: ей закрытые сегменты ещё
    // предстоит прочитать. Её кеш построен ДО компакции и указывает на `.jrnl`, которых
    // больше нет; без инвалидации она недосчитается событий — тихая деградация при
    // зелёном healthcheck, тот же класс, что `TD-031`.
    let ck_lag = tempfile::tempdir().expect("ckpt lagging");
    let mut lagging = new_session(dir.path(), ck_lag.path());
    let (first, _c, s_first) = lagging
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 8)
        .expect("SM-5: первый тик отставшей сессии");
    assert!(
        !first.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: отставшая сессия не прочитала ни одного кадра ДО компакции —          её кеш не построен, и проверять инвалидацию не на чем"
    );
    // Мера — СОБЫТИЯ, а не кадры: кадр несёт несколько событий, и сравнение `frames.len()`
    // с длиной `reference_seqs` сравнивало бы разные величины (замер: 101 против 806).
    let mut seen = s_first.events_decoded as usize;
    let raw2_before = fs::read_dir(dir.path())
        .expect("rd")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jrnl"))
        .count();
    journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact 2");
    let raw2_after = fs::read_dir(dir.path())
        .expect("rd")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jrnl"))
        .count();
    assert!(
        raw2_after < raw2_before,
        "SETUP НЕ СОСТОЯЛСЯ: вторая компакция не удалила ни одного `.jrnl` ({raw2_before} -> \
         {raw2_after}) — событие оси 4 для ОТСТАВШЕЙ сессии не воспроизведено (§4.5(б))"
    );
    for _ in 0..64 {
        let (frames, _c, s_tick) = lagging
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 4_096)
            .expect("SM-5: тик отставшей сессии после компакции");
        if frames.is_empty() {
            break;
        }
        seen += s_tick.events_decoded as usize;
    }
    let total = reference_seqs(dir.path(), None).len();
    assert!(
        seen >= total,
        "SM-5: отставшая сессия после компакции получила {seen} событий из {total}. Её кеш          перечня сегментов построен ДО компакции и указывает на `.jrnl`, которых больше          нет: компакция НЕ ЗАМЕЧЕНА. Сессия молча недоотдаёт данные при зелёном          healthcheck — мутант `staleforever`."
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// SM-6 — ось 5: ДВЕ сессии, ЧЕРЕДУЮЩИЕСЯ тики
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn sm6_two_sessions_alternating_ticks_stay_within_budget() {
    claims("sm6", 5, "состояние общее на каталог");
    claims("sm6", 5, "две сессии независимы");

    let (dir, n) = build_prod_form(N_SEGMENTS);
    let ck_a = tempfile::tempdir().expect("ck a");
    let ck_b = tempfile::tempdir().expect("ck b");
    let mut a = new_session(dir.path(), ck_a.path());
    let mut b = new_session(dir.path(), ck_b.path());
    catch_up(&mut a, dir.path());
    catch_up(&mut b, dir.path());

    // ЧЕРЕДОВАНИЕ обязательно: A₁ B₁ A₂ B₂. Последовательные прогоны (сперва все тики A,
    // потом все B) общий кеш НЕ обнаруживают — каждая сессия успевает прогреть его под себя.
    let mut worst_a = 0u64;
    let mut worst_b = 0u64;
    for k in 0..3u64 {
        let base = n + k * INCREMENT;
        append_range(dir.path(), base, base + INCREMENT);
        let (_f, _c, sa) = a
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
            .expect("pump A");
        let (_f, _c, sb) = b
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
            .expect("pump B");
        worst_a = worst_a.max(sa.segment_meta_ops);
        worst_b = worst_b.max(sb.segment_meta_ops);
    }

    assert!(
        worst_a <= BUDGET_META && worst_b <= BUDGET_META,
        "SM-6: при ДВУХ сессиях с чередующимися тиками худший тик выполнил A={worst_a}, \
         B={worst_b} операций с метаданными (бюджет {BUDGET_META}). Состояние общее на \
         КАТАЛОГ, а сессий столько, сколько подключений: соседняя сессия обесценивает кеш, \
         и все прочие проваливаются в полный обход. Точная реплика `F-035-2` — выигрыш, \
         существующий при ОДНОМ зрителе, цели 10 000 сессий не достигает."
    );
}
