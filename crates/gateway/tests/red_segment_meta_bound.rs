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
// «манифест ⇄ таблица §4.2 спеки» — гейт M-62, сдан в архив по норме Р-2
// (`docs/archive/verify_M-62.sh`), сверка была НА ПРИЁМКЕ. Число сценариев СЧИТАЕТСЯ
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
    (
        "sm8",
        4,
        "посегментная компакция стирает сегмент из кеша",
        'V',
    ),
    (
        "sm9",
        4,
        "промежуточное состояние компакции даёт дубль индекса",
        'V',
    ),
    ("sm10", 4, "посторонний файл в каталоге", 'L'),
];

/// Сверка «исполнение ⇒ манифест»: оракул объявляет, какое значение он покрывает, и падает,
/// если такого значения в манифесте нет. Обратное направление («манифест ⇒ исполнение»)
/// проверял грепом имён по этому файлу гейт M-62 (архив: `docs/archive/verify_M-62.sh`).
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
        depth_cadence_ms: None,
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

// ─────────────────────────────────────────────────────────────────────────────────────────
// SM-8..SM-10 — ИНКРЕМЕНТАЛЬНАЯ ветка `is_fresh` (R-053, блокеры Б-1/Б-2/Б-3)
//
// ПОЧЕМУ ИХ НЕ БЫЛО. SM-4/SM-5 звали `compact_closed_segments`, который сжимает ВСЕ закрытые
// сегменты разом: diff каталога получается большой, `small_change == false`, и отрабатывает
// полный `refresh()`, где дедупликация корректна. Прод компактирует ПОСЕГМЕНТНО (cron 03:50
// зовёт `compact_segment` в цикле), и тик живой сессии — 250 мс — попадает ровно в маленький
// diff, то есть в ветку, которую набор не исполнял НИ РАЗУ. Пробел был назван честно в шапке
// батареи с оговоркой «либо инвалидация по компакции не нужна для корректности выдачи»;
// замер reviewer'а ответил: нужна, и там три дефекта.
//
// ЭТАЛОН — НЕЗАВИСИМЫЙ ПУТЬ: `journal::list_segments(dir)` обходит каталог с нуля и применяет
// `D-COMP-1` (`dedup_indexed_paths`). Сверять кеш с кешем — тавтология (testing.md).
//
// SETUP-GUARD НА КАЖДЫЙ СЦЕНАРИЙ обязателен, НО СТЕРЕЖЁТ ОН ДИСК, А НЕ ВЕТКУ (круг 3).
// Прежняя редакция требовала `assert!(fresh, "SETUP НЕ СОСТОЯЛСЯ: is_fresh=false ⇒ отработал
// полный refresh()")`. Замер круга 3 показал, что этот страж пиннил ВЕТКУ ИСПОЛНЕНИЯ:
// развязка «при коллизии индекса уходить в refresh()», прямо разрешённая вердиктом `R-056`
// (Условие APPROVED п.1) и корректная end-to-end, роняла SM-8 не на предмете, а на самом
// страже. Инвариант милестоуна — «каталог ПРАВДИВ и цена УСТАНОВИВШЕГОСЯ такта не зависит от
// N»; какой веткой это достигнуто, инвариантом не является.
//
// Ветку заменяют ДВА фикс-агностичных наблюдения:
//   (1) состояние ДИСКА до такта (`files_of_index`) — сценарий воспроизведён, проба меряет то,
//       что обещает; это и есть настоящее содержание «setup состоялся»;
//   (2) БЮДЖЕТ такта (`tick` + `BUDGET_META`) — то, что прежний страж защищал КОСВЕННО:
//       «уходить в refresh() всегда» есть лазейка к O(N) на каждом тике, и её обязан ловить
//       ЯВНЫЙ сторож бюджета, а не побочный эффект требования `fresh == true`.
// Замер, подтверждающий, что замена не ослабила набор: мутант «всегда refresh» роняет
// переделанные SM-8/SM-9 по бюджету с названным числом (411 против 8), тогда как прежняя
// редакция умирала на setup-guard'е — случай (б) из шапки `red_segment_meta_battery.sh`,
// который батарея сама называет «записью "пиннит дыру" НЕ является».
// ─────────────────────────────────────────────────────────────────────────────────────────

fn cache_indices(cat: &journal::SegmentCatalog) -> Vec<u32> {
    let mut v: Vec<u32> = cat.segments().iter().map(|s| s.index).collect();
    v.sort_unstable();
    v
}
fn truth_indices(dir: &Path) -> Vec<u32> {
    let mut v: Vec<u32> = journal::list_segments(dir)
        .expect("эталон: journal::segments")
        .iter()
        .map(|s| s.index)
        .collect();
    v.sort_unstable();
    v
}

/// ПУТИ, а не только индексы. Развязка «не удалять запись, пока индекс есть в `cur_names`»
/// оставляет в кеше `SegmentInfo` с путём на УДАЛЁННЫЙ файл: множества ИНДЕКСОВ при этом
/// совпадают, а `pump()` умирает `Os { code: 2, NotFound }`. Сверка одних индексов пропускает
/// такой фикс целиком (замер разведки: все 10 SM зелены).
fn cache_paths(cat: &journal::SegmentCatalog) -> Vec<String> {
    let mut v: Vec<String> = cat
        .segments()
        .iter()
        .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}
fn truth_paths(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = journal::list_segments(dir)
        .expect("эталон: journal::segments")
        .iter()
        .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// Сколько файлов каталога принадлежат этому индексу (`.jrnl` + `.jrnl.zst`).
/// Это НАБЛЮДЕНИЕ ДИСКА — годится в setup-guard, в отличие от наблюдения ветки реализации.
fn files_of_index(dir: &Path, index: u32) -> usize {
    let pat = format!("{index:08}");
    fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(&pat))
        .count()
}

/// Полная цена обхода ЭТОГО каталога — ЗАМЕР, а не литерал: `SegmentCatalog::open` исполняет
/// ровно то же, что `refresh()`. Служит потолком для такта, несущего событие каталога.
fn full_scan_cost(dir: &Path) -> u64 {
    let (_c, ops) = journal::SegmentCatalog::open(dir).expect("эталон цены: catalog open");
    ops
}

/// Такт сессии в ПРОД-ФОРМЕ вызывающего (`stream_from_at_with_catalog`,
/// `crates/journal/src/segments.rs:1789-1799`): `is_fresh`, и при `false` — `refresh()`.
/// Возвращает `(ops такта, ушла ли реализация в полный refresh)`.
///
/// ПОЧЕМУ ИМЕННО ТАК, А НЕ `assert!(fresh)`. Инвариант милестоуна — «каталог правдив И цена
/// установившегося тика не зависит от N»; КАКОЙ веткой это достигнуто, инвариантом не
/// является. Прежний setup-guard `assert!(fresh, "SETUP НЕ СОСТОЯЛСЯ …")` пиннил ВЕТКУ и тем
/// запрещал развязку «при коллизии индекса уходить в refresh()», санкционированную вердиктом
/// `R-056` (замер разведки: она корректна end-to-end, но роняла SM-8 на его собственном
/// страже). Ветку заменяют ДВА наблюдения, оба фикс-агностичные: состояние ДИСКА (setup
/// состоялся) и БЮДЖЕТ такта (корректность не куплена ценой O(N) на каждом тике).
fn tick(cat: &mut journal::SegmentCatalog, dir: &Path) -> (u64, bool) {
    let (fresh, mut ops) = cat.is_fresh(dir).expect("is_fresh");
    if !fresh {
        ops += cat.refresh(dir).expect("refresh");
    }
    (ops, !fresh)
}

/// Правдивость каталога сессии против НЕЗАВИСИМОГО пути (`journal::list_segments` — обход с
/// нуля). Три сверки, потому что у потребителя ровно три наблюдаемых различия: существование
/// файла, состав индексов и адресация. Порядок сверок — от самого жёсткого симптома к самому
/// мягкому: несуществующий путь = ENOENT в `pump()`, состав = недоотдача событий, адресация =
/// чтение не того представления.
///
/// Расхождение печатается РАЗНИЦЕЙ, а не двумя списками по 200 имён: вердикт гейта читают
/// люди.
fn assert_catalog_truthful(cat: &journal::SegmentCatalog, dir: &Path, stage: &str) {
    // СВИДЕТЕЛЬ (`docs/plans/two-barriers-step1-2026-08-23.md` §4). Все три проверки ниже —
    // негативные ассерты над результатом ФИЛЬТРА, и все три ИСТИННЫ на пустом каталоге:
    // реализация, отдающая пустой кеш, проходила бы «сверку правдивости» на всех шести
    // тактах. Свидетель говорит, что сверять БЫЛО ЧТО, и снимается он с НЕЗАВИСИМОГО обхода
    // (`truth_indices` — `journal::list_segments` с нуля), а не с проверяемого кеша: иначе
    // пустой кеш подтверждал бы сам себя.
    let observed = truth_indices(dir);
    assert!(
        !observed.is_empty(),
        "{stage}: SETUP НЕ СОСТОЯЛСЯ — независимый обход каталога не нашёл НИ ОДНОГО сегмента. \
         Три проверки ниже истинны на пустоте, и такт сертифицировал бы отсутствие предмета"
    );
    // (1) СУЩЕСТВОВАНИЕ: кеш не смеет адресовать файл, которого нет.
    let dangling: Vec<String> = cat
        .segments()
        .iter()
        .filter(|s| !s.path.exists())
        .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        dangling.is_empty(),
        "{stage}: кеш держит пути на НЕСУЩЕСТВУЮЩИЕ файлы {dangling:?}. Это не расхождение \
         учёта, а жёсткий Err на прод-пути: `pump()` откроет такой путь и вернёт \
         `Os {{ code: 2, kind: NotFound }}` — сессия перестанет отдавать кадры вовсе. Ровно \
         сюда приводит развязка «не удалять запись, пока индекс есть в cur_names»: множества \
         ИНДЕКСОВ при ней совпадают, и сверка одних индексов её пропускает."
    );
    // (2) СОСТАВ: те же индексы, что у независимого обхода.
    let (ci, ti) = (cache_indices(cat), truth_indices(dir));
    let lost: Vec<u32> = ti.iter().filter(|i| !ci.contains(i)).copied().collect();
    let extra: Vec<u32> = ci.iter().filter(|i| !ti.contains(i)).copied().collect();
    assert!(
        lost.is_empty() && extra.is_empty(),
        "{stage}: СОСТАВ кеша разошёлся с каталогом (эталон — `journal::list_segments`, \
         независимый обход с нуля). ПОТЕРЯНЫ индексы {lost:?}; ЛИШНИЕ {extra:?}. \
         Инкрементальная ветка правит кеш по разнице ИМЁН, тогда как истина есть функция \
         СОДЕРЖИМОГО каталога, а `self.file_names = cur_names` коммитится безусловно — \
         расхождение больше НИКОГДА не наблюдаемо изнутри: ни ротация, ни посегментная \
         компакция refresh не зовут (diff ≤2, `segments.rs:278`). Отставшая сессия \
         недосчитается всех событий сегмента при зелёном healthcheck — класс TD-031."
    );
    // (3) АДРЕСАЦИЯ: тот же файл на индекс, что выбрал бы независимый обход (D-COMP-1).
    let (cp, tp) = (cache_paths(cat), truth_paths(dir));
    let wrong: Vec<&String> = cp.iter().filter(|n| !tp.contains(n)).collect();
    assert!(
        wrong.is_empty(),
        "{stage}: индексы совпали, а ПУТИ разошлись: кеш адресует {wrong:?}, независимый \
         обход — другое представление тех же сегментов. Правило D-COMP-1 (при коллизии \
         побеждает СЫРОЙ) применяется только на полном пути; кеш, разошедшийся с ним по \
         выбору файла, читает не то, что читает всякий другой потребитель."
    );
}

#[test]
fn sm8_per_segment_compaction_keeps_catalog_truthful() {
    claims("sm8", 4, "посегментная компакция стирает сегмент из кеша");

    let (dir, n) = build_prod_form(N_SEGMENTS);
    let (mut cat, _ops) = journal::SegmentCatalog::open(dir.path()).expect("catalog open");

    // Потолок цены такта — ЗАМЕР этого каталога, не литерал.
    let full = full_scan_cost(dir.path());
    assert!(
        full > BUDGET_META * 2,
        "SETUP НЕ СОСТОЯЛСЯ: полный обход стоит {full} при бюджете {BUDGET_META} — на таком \
         каталоге бюджетный сторож не отличает кеш от его отсутствия"
    );

    let all = journal::list_segments(dir.path()).expect("segments");
    let max_idx = all.iter().map(|s| s.index).max().expect("непустой каталог");
    // Жертва — закрытый СЫРОЙ сегмент, НЕ последний по индексу: исчезновение `latest_path`
    // ловится отдельным `stat`ом (segments.rs:241-258) и уводит в refresh само по себе,
    // то есть на последнем индексе сценарий подменяется другим.
    let victim = all
        .iter()
        .find(|s| {
            s.path.extension().map(|e| e == "jrnl").unwrap_or(false)
                && s.index > 0
                && s.index < max_idx
        })
        .cloned()
        .expect("SETUP: нужен закрытый сырой сегмент НЕ последнего индекса");

    // ── ТАКТ 1 — шаг 6 компакции: `.zst` опубликован, оригинал `.jrnl` ЕЩЁ НА МЕСТЕ ────────
    // Прод компактирует ДВУМЯ файловыми событиями (segments.rs:4003 rename, :4011 remove), а
    // тик сессии — 250 мс: попадание МЕЖДУ ними на проде ~13 раз в сутки (замер ширины окна
    // unlink 1 GiB: 254/261/728 мс). Однотактовая форма (compact_segment целиком между двумя
    // тиками) от дефекта Б-4 слепа: она даёт added+removed в ОДНОМ diff'е, где порядок
    // remove→add уже верен.
    let raw_bytes = fs::read(&victim.path).expect("read raw");
    journal::compact_segment(&victim, journal::DEFAULT_COMPACT_LEVEL).expect("compact one");
    fs::write(&victim.path, &raw_bytes).expect("вернуть .jrnl рядом с .zst — шаг 7 ещё не сделан");
    assert_eq!(
        files_of_index(dir.path(), victim.index),
        2,
        "SETUP НЕ СОСТОЯЛСЯ: у индекса {} на диске не ДВА файла — промежуточное состояние \
         компакции (шаг 6 сделан, шаг 7 нет) не воспроизведено, и такт 1 проверял бы не то",
        victim.index
    );
    let (ops1, refreshed1) = tick(&mut cat, dir.path());
    assert_catalog_truthful(&cat, dir.path(), "SM-8 такт 1 (шаг 6: .zst рядом с .jrnl)");

    // ── ТАКТ 2 — шаг 7 компакции: оригинал удалён ─────────────────────────────────────────
    fs::remove_file(&victim.path).expect("шаг 7: remove(src)");
    assert_eq!(
        files_of_index(dir.path(), victim.index),
        1,
        "SETUP НЕ СОСТОЯЛСЯ: у индекса {} на диске не РОВНО ОДИН файл после шага 7",
        victim.index
    );
    let (ops2, refreshed2) = tick(&mut cat, dir.path());
    assert_catalog_truthful(&cat, dir.path(), "SM-8 такт 2 (шаг 7: .jrnl удалён)");

    // ── ТАКТЫ 3-4 — УСТАНОВИВШИЙСЯ режим: состав каталога больше не меняется ──────────────
    // Здесь и стоит настоящий предмет прежнего `assert!(fresh)`: корректность не имеет права
    // быть куплена ценой «уходить в refresh() всегда». Такт БЕЗ события каталога обязан быть
    // дешёвым независимо от того, какой веткой реализация закрыла такты 1-2.
    append_range(dir.path(), n, n + INCREMENT);
    let (ops3, _r3) = tick(&mut cat, dir.path());
    assert_catalog_truthful(&cat, dir.path(), "SM-8 такт 3 (append в активный)");
    let (ops4, refreshed4) = tick(&mut cat, dir.path());
    assert_catalog_truthful(&cat, dir.path(), "SM-8 такт 4 (каталог не менялся)");

    eprintln!(
        "SM-8: full_scan={full} ops1={ops1}(refresh={refreshed1}) ops2={ops2}(refresh={refreshed2}) \
         ops3={ops3} ops4={ops4}(refresh={refreshed4})"
    );

    assert!(
        ops4 <= BUDGET_META,
        "SM-8: такт БЕЗ единого изменения состава каталога стоил {ops4} операций при бюджете \
         {BUDGET_META} (полный обход этого каталога — {full}). Корректность куплена ценой \
         «refresh() на каждом тике»: при цели 10 000 сессий и периоде 250 мс это возвращает \
         ровно ту цену, ради устранения которой заведён M-62. Прежний setup-guard \
         `assert!(fresh)` защищал это КОСВЕННО — и заодно запрещал верную развязку; предмет \
         его защиты живёт здесь, в бюджете установившегося такта."
    );
    assert!(
        ops3 <= BUDGET_META,
        "SM-8: такт с одним лишь append'ом в АКТИВНЫЙ сегмент стоил {ops3} операций при \
         бюджете {BUDGET_META}. Рост активного файла — норма между тиками, а не событие \
         каталога, и переучёта не требует (§4.1)."
    );
    // Потолок такта-с-событием = дешёвая проба (≤ BUDGET_META) + ОДИН полный обход. Развязка
    // «уходить в refresh()» платит ровно столько (замер: 3 + 409 = 412); двойной обход даёт
    // ≥ 2×full и здесь краснеет.
    let ceiling = full + BUDGET_META;
    assert!(
        ops1 <= ceiling && ops2 <= ceiling,
        "SM-8: такт, несущий событие каталога, стоил больше ПОЛНОГО обхода (ops1={ops1}, \
         ops2={ops2} при потолке {ceiling} = полный обход {full} + проба {BUDGET_META}) — \
         реализация обходит каталог ДВАЖДЫ за такт. Дороже одного честного холодного пути \
         платить не за что."
    );
    let expensive = [ops1, ops2, ops3, ops4]
        .iter()
        .filter(|o| **o > BUDGET_META)
        .count();
    assert!(
        expensive <= 2,
        "SM-8: дорогих тактов {expensive} при ДВУХ событиях каталога в серии из четырёх \
         тактов. Полную цену законно платит такт, НЕСУЩИЙ событие (это легитимное значение \
         оси 3, `sm4`); такт после него обязан вернуться в бюджет. Больше дорогих тактов, чем \
         событий, — признак того, что кеш не восстанавливается и переучёт стал постоянным."
    );
}

#[test]
fn sm9_compaction_midstate_does_not_duplicate_index() {
    claims(
        "sm9",
        4,
        "промежуточное состояние компакции даёт дубль индекса",
    );

    let (dir, _n) = build_prod_form(N_SEGMENTS);
    let (mut cat, _ops) = journal::SegmentCatalog::open(dir.path()).expect("catalog open");
    let full = full_scan_cost(dir.path());

    let all = journal::list_segments(dir.path()).expect("segments");
    let max_idx = all.iter().map(|s| s.index).max().expect("непустой каталог");
    let victim = all
        .iter()
        .find(|s| {
            s.path.extension().map(|e| e == "jrnl").unwrap_or(false)
                && s.index > 0
                && s.index < max_idx
        })
        .cloned()
        .expect("SETUP: нужен закрытый сырой сегмент НЕ последнего индекса");
    // Промежуточное состояние `compact_segment`: rename .tmp→.zst сделан (шаг 6), remove
    // оригинала (шаг 7) ЕЩЁ НЕ сделан. По комментарию segments.rs:3964-3972 оба файла лежат
    // рядом «минуты». Воспроизводим возвратом сырого файла после компакции.
    let raw_bytes = fs::read(&victim.path).expect("read raw");
    journal::compact_segment(&victim, journal::DEFAULT_COMPACT_LEVEL).expect("compact");
    fs::write(&victim.path, &raw_bytes).expect("вернуть сырой рядом с .zst");

    let both = fs::read_dir(dir.path())
        .expect("rd")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.contains(&format!("{:08}", victim.index))
        })
        .count();
    assert!(
        both >= 2,
        "SETUP НЕ СОСТОЯЛСЯ: рядом с индексом {} лежит {both} файл(ов) — промежуточное \
         состояние компакции не воспроизведено, и тест проверял бы не то",
        victim.index
    );

    let (ops_mid, refreshed) = tick(&mut cat, dir.path());
    assert_eq!(
        cache_indices(&cat),
        truth_indices(dir.path()),
        "SM-9: в кеше появился ДУБЛЬ индекса. Инкрементальная ветка кладёт .zst рядом с уже \
         лежащим .jrnl того же индекса, минуя dedup_indexed_paths — единственный путь, \
         обходящий общий хелпер. Правило D-COMP-1 (при коллизии побеждает СЫРОЙ) не \
         применяется, и сессия получит события сегмента ДВАЖДЫ. Это дословно блокер PR-гейта \
         M-08: «3000 событий читалось как 3172, DET-I-1 молча нарушался»."
    );
    assert_catalog_truthful(&cat, dir.path(), "SM-9 такт промежуточного состояния");

    // Бюджет вместо `assert!(fresh)`: такт, несущий событие каталога, вправе заплатить полную
    // цену (легитимное значение оси 3, `sm4`) — но СЛЕДУЮЩИЙ, без события, обязан вернуться в
    // бюджет. Прежний страж требовал КОНКРЕТНОЙ ветки и тем запрещал развязку «уходить в
    // refresh() при коллизии индекса», которую вердикт `R-056` разрешает явно.
    let (ops_quiet, refreshed_quiet) = tick(&mut cat, dir.path());
    assert_catalog_truthful(&cat, dir.path(), "SM-9 такт без события каталога");
    eprintln!(
        "SM-9: full_scan={full} ops_mid={ops_mid}(refresh={refreshed}) \
         ops_quiet={ops_quiet}(refresh={refreshed_quiet})"
    );
    assert!(
        ops_mid <= full + BUDGET_META,
        "SM-9: такт промежуточного состояния стоил {ops_mid} при потолке {} = полный обход \
         {full} + дешёвая проба {BUDGET_META}: реализация обходит каталог ДВАЖДЫ за такт",
        full + BUDGET_META
    );
    assert!(
        ops_quiet <= BUDGET_META,
        "SM-9: такт БЕЗ изменения состава каталога стоил {ops_quiet} при бюджете \
         {BUDGET_META}. Пара `.jrnl`+`.jrnl.zst` живёт на диске часами (крах компакции между \
         шагами 6 и 7, возврат сырого из бэкапа); реализация, которая на такой раскладке \
         уходит в полный обход КАЖДЫЙ тик, платит O(N) всё это время — цена M-62 отменена."
    );
}

#[test]
fn sm10_foreign_file_does_not_break_session() {
    claims("sm10", 4, "посторонний файл в каталоге");

    let (dir, _n) = build_prod_form(N_SEGMENTS);
    let (mut cat, _ops) = journal::SegmentCatalog::open(dir.path()).expect("catalog open");

    // Файлы, которые пишет САМ проект в тот же каталог: journal.meta.tmp — каждые 64 события
    // recorder'а (journal/src/lib.rs:350-353 ← flush() :305); segment-*.jrnl.zst.tmp — живёт
    // МИНУТЫ при компакции (segments.rs:3886); replay-digest.tmp — cron 04:07.
    for foreign in [
        "journal.meta.tmp",
        "segment-00000002.jrnl.zst.tmp",
        "replay-digest.tmp",
    ] {
        fs::write(dir.path().join(foreign), b"x").expect("создать посторонний файл");
        let before = truth_indices(dir.path());
        let res = cat.is_fresh(dir.path());
        assert!(
            res.is_ok(),
            "SM-10: появление файла «{foreign}» уронило is_fresh ⇒ pump() сессии вернёт Err. \
             classify_segment зовётся на КАЖДОЕ новое имя без фильтра parse_segment_index_any \
             и на не-сегментном имени даёт Err. До M-62 такие файлы были безвредны: \
             dedup_indexed_paths их просто не выбирал. Эти файлы пишет сам проект, и при цели \
             10k сессий × 4 тика/с попадание неизбежно: ошибка = {:?}",
            res.as_ref().err()
        );
        let _ = res;
        assert_eq!(
            cache_indices(&cat),
            before,
            "SM-10: посторонний файл «{foreign}» изменил состав кеша — он не сегмент и не \
             обязан ни на что влиять"
        );
        fs::remove_file(dir.path().join(foreign)).expect("убрать посторонний файл");
        let (_f, _o) = cat.is_fresh(dir.path()).expect("is_fresh после уборки");
    }
}
