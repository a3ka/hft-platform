//! RED `MD-I-8` (sacred, architect-only) — ПОЛОСЫ ГЛУБИНЫ ЧИТАЮТ КНИГУ, А НЕ ОБРЕЗАННЫЙ
//! СНАПШОТ БИРЖИ.
//!
//! # Что пиннит оракул и почему он существует
//!
//! Решение founder'а 2026-07-24 (`research/data-quality/depth-verdict.md`) предписывает
//! строить полосы TPP на НАШЕЙ diff-реконструированной книге. Замер 2026-08-16 показал, что
//! кодом это не исполнено:
//!
//! * `crates/gateway/src/lib.rs` — `depth_within(...)` вызывается РОВНО ОДИН раз, в ветке
//!   `MdPayload::L2Snapshot`, и считает по `bids`/`asks` ПЕЙЛОАДА, а не по `self.book`;
//! * в ветке `MdPayload::L2Delta` стоит дословный комментарий
//!   «НЕ апдейтится — депт-серия остаётся snapshot-only (M-22 семантика)»;
//! * снапшот приходит от биржи капнутым: `venue-binance/src/lib.rs`
//!   `REST_DEPTH_LIMIT = "5000"` — а это и есть примерно 1.3 % от mid.
//!
//! Отсюда два следствия, и второе хуже первого:
//!
//! 1. граница 1.3 % — это НЕ «докуда дотянулась валидация», а «докуда есть данные»;
//!    оба числа суть один и тот же кап биржи;
//! 2. метка `depth_band_provenance` («diff-reconstructed, validated<=1.3%», `VB-I-5`) ЛЖЁТ
//!    о собственной серии: серия не diff-reconstructed, она snapshot-derived.
//!
//! **Данные при этом у нас ЕСТЬ.** Собственная книга держит ±60 % от mid
//! (`venue-binance/src/lib.rs` `MAX_REL_DIST = 0.60`), что покрывает весь канонический набор
//! полос (1.5/3/5/8/15/30/60 %). Требование founder'а 2026-08-17 — «нам в любом случае нужен
//! механизм или источник получения всех глубин» — закрывается ПРОВОДКОЙ, а не вендором и не
//! новым сбором: состав записываемых данных не меняется (`П-011` амендмент 2026-08-17).
//!
//! # Конструкция оракула — ДИФФЕРЕНЦИАЛЬНАЯ, и это существенно
//!
//! Оракул «полоса 60 % вернула мало» доказывал бы лишь то, что чего-то не хватает, и был бы
//! неотличим от «дальних данных нет в журнале». Поэтому сравниваются ДВА ПОТРЕБИТЕЛЯ ОДНОГО
//! входа: heatmap читает книгу, депт-серия — пейлоад снапшота. Если heatmap дальний уровень
//! ВИДИТ, а полоса его не считает, то дефект локализован в проводке выдачи, а не в данных.
//!
//! Это же снимает класс «зависимый эталон» (`testing.md`): эталон берётся из НЕЗАВИСИМОГО
//! пути — другого редьюсера над тем же журналом, а не из той же функции.
//!
//! # Анти-плацебо — обе стороны
//!
//! * `d1` роняет сегодняшнюю реализацию (полоса 60 % не видит уровня на −40 %);
//! * `d2` — SETUP-контроль: heatmap тот же уровень обязана видеть, иначе фикстура не о том
//!   (данных нет в журнале вовсе), и `d1` был бы вакуумным;
//! * `d3` роняет реализацию «отдавать всю книгу в любой полосе»: узкая полоса обязана
//!   ОСТАТЬСЯ узкой. Без него «фикс» через `band = ∞` прошёл бы `d1`.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Полоса внутри валидированной зоны — контроль «продукт не выключен».
const NEAR_BAND: f64 = 0.001;
/// Канонический край набора M-67 §4.3. Наша книга его покрывает (`MAX_REL_DIST = 0.60`).
const FAR_BAND: f64 = 0.60;
/// Дистанция дальнего уровня: ЗАВЕДОМО за капом снапшота (~1.3 %) и заведомо внутри книги.
const FAR_OFFSET: f64 = 0.40;
/// Размер дальнего уровня — крупный, чтобы его появление нельзя было списать на округление.
const FAR_SIZE: f64 = 500.0;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "MD-I-8 depth-from-book fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// **Фикстура моделирует ПРОД-ФОРМУ, а не удобную.**
///
/// Снапшот УЗКИЙ (±1 %) — ровно так его отдаёт биржа под `limit=5000`. Дальние уровни
/// приходят ТОЛЬКО дельтами, как на проде: биржа шлёт диффы каждые 100 мс, и именно из них
/// собирается наша книга до ±60 %.
fn build() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");

    // Узкий снапшот — то, что реально приходит от биржи (кап limit=5000 ≈ 1.3 %).
    let near = [0.0005_f64, 0.005, 0.010];
    let bids: Vec<Level> = near.iter().map(|o| lvl(MID * (1.0 - o), 2.0)).collect();
    let asks: Vec<Level> = near.iter().map(|o| lvl(MID * (1.0 + o), 2.0)).collect();

    for i in 0..12i64 {
        let ts = T0 + i * 100;
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: bids.clone(),
                asks: asks.clone(),
                ts_exch_ms: ts,
            },
        ))
        .expect("append snapshot");

        // ДАЛЬНИЙ уровень приходит дельтой — за капом снапшота, внутри книги.
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Delta {
                bids: vec![lvl(MID * (1.0 - FAR_OFFSET), FAR_SIZE)],
                asks: vec![lvl(MID * (1.0 + FAR_OFFSET), FAR_SIZE)],
                // Непрерывная цепочка update-id: разрыв означал бы «книга подозрительна»,
                // и реализация была бы вправе дельту отбросить — тогда красное `d1` не
                // отличалось бы от честного отказа по разрыву (`testing.md`: проба обязана
                // падать против СЛОМАННОГО, а не против собственной небрежности фикстуры).
                first_update_id: (i as u64) * 2 + 1,
                final_update_id: (i as u64) * 2 + 2,
                prev_final_update_id: if i == 0 { None } else { Some((i as u64) * 2) },
                ts_exch_ms: ts + 10,
            },
        ))
        .expect("append delta");
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

fn snap_of(dir: &std::path::Path, bands: Vec<f64>) -> gateway::Snapshot {
    gateway::snapshot(
        dir,
        EpochFilter::OwnCaptureOnly,
        &sel(bands),
        Cursor::LATEST,
    )
    .expect("snapshot обязан строиться")
}

/// Сумма глубины по стороне для полосы (последнее значение серии).
fn depth_of(snap: &gateway::Snapshot, band: f64, side: &str) -> i64 {
    let want = (band * 1e8).round() as i64;
    snap.series
        .depth_series
        .iter()
        .filter(|r| r.band_pct_e8 == want && r.side == side)
        .filter_map(|r| r.series.last().map(|&(_, v)| v))
        .max()
        .unwrap_or(0)
}

/// **D2 — SETUP-КОНТРОЛЬ, исполняется ПЕРВЫМ по смыслу.**
///
/// Heatmap читает книгу. Если она дальнего уровня НЕ видит — значит его нет в журнале, и
/// весь оракул проверял бы не тот сценарий (`testing.md`: проба обязана падать и при
/// несостоявшемся setup'е).
#[test]
fn md_i8_d2_setup_heatmap_sees_the_far_level_from_deltas() {
    let dir = build();
    let snap = snap_of(dir.path(), vec![NEAR_BAND, FAR_BAND]);

    let far_price_lo = to_fixed(MID * (1.0 - FAR_OFFSET) * 0.999);
    let far_price_hi = to_fixed(MID * (1.0 - FAR_OFFSET) * 1.001);
    let seen = snap
        .series
        .heatmap
        .iter()
        .any(|c| c.price_e8 >= far_price_lo && c.price_e8 <= far_price_hi && c.size_e8 > 0);

    assert!(
        seen,
        "SETUP не состоялся: heatmap не видит уровень на −{:.0}% от mid, пришедший ДЕЛЬТОЙ. \
         Значит дальних данных нет в журнале вовсе, и дифференциальное сравнение с депт-серией \
         ничего не доказывало бы. Чинить фикстуру, а не реализацию.",
        FAR_OFFSET * 100.0
    );
}

/// **D1 — ГЛАВНОЕ.** Полоса 60 % обязана включать уровень на −40 %, который heatmap уже
/// видит из того же журнала. Сегодня падает: депт-серия считается по пейлоаду снапшота,
/// капнутого биржей на ~1.3 %, и дельты в неё не входят.
#[test]
fn md_i8_d1_far_band_counts_levels_that_only_deltas_delivered() {
    let dir = build();
    let snap = snap_of(dir.path(), vec![NEAR_BAND, FAR_BAND]);

    let near = depth_of(&snap, NEAR_BAND, "bid");
    let far = depth_of(&snap, FAR_BAND, "bid");
    let far_level = to_fixed(FAR_SIZE);

    // ЭТАЛОН АБСОЛЮТНЫЙ, А НЕ СРАВНИТЕЛЬНЫЙ — и это принципиально.
    //
    // Первая редакция ассерта требовала `far > near` и была ВАКУУМНОЙ: широкая полоса
    // захватывает больше уровней СНАПШОТА (три против одного), поэтому неравенство
    // выполнялось само, без всякого участия дельт. Замер: near = 2.0, far = 6.0 = ровно
    // сумма снапшота, дальний уровень 500.0 отсутствует — а тест был ЗЕЛЁНЫМ.
    // `testing.md`: «ассерт „изменилось“ обязан называть, ОТ ЧЕГО». Называем: от размера
    // дальнего уровня, который обязан войти в сумму.
    assert!(
        far >= far_level,
        "MD-I-8 нарушен: полоса {far_pct:.0}% дала {far} при ожидаемом ≥{far_level} \
         (полоса {near_pct:.1}% дала {near}) — дальний уровень на −{off:.0}% от mid \
         (size={size}) в сумму НЕ ВОШЁЛ, хотя heatmap его \
         видит (d2 зелёный). Депт-серия считается по пейлоаду L2Snapshot, капнутому биржей на \
         limit=5000 (≈1.3%), а не по нашей diff-книге, которая держит ±60% \
         (MAX_REL_DIST=0.60). Решение founder'а 2026-07-24 «строить TPP на diff-книге» кодом \
         НЕ исполнено, а метка VB-I-5 «diff-reconstructed» лжёт о собственной серии.",
        far_pct = FAR_BAND * 100.0,
        near_pct = NEAR_BAND * 100.0,
        off = FAR_OFFSET * 100.0,
        size = FAR_SIZE
    );
}

/// **D3 — анти-плацебо с другой стороны.** Узкая полоса обязана ОСТАТЬСЯ узкой: реализация
/// «в любой полосе отдаём всю книгу» прошла бы `d1` и обязана падать здесь.
#[test]
fn md_i8_d3_near_band_does_not_swallow_the_far_level() {
    let dir = build();
    let snap = snap_of(dir.path(), vec![NEAR_BAND, FAR_BAND]);

    let near = depth_of(&snap, NEAR_BAND, "bid");
    let far_level = to_fixed(FAR_SIZE);

    assert!(
        near > 0,
        "D3: узкая полоса {near_pct:.1}% пуста — продукт выключен целиком, а не расширен",
        near_pct = NEAR_BAND * 100.0
    );
    assert!(
        near < far_level,
        "D3: узкая полоса {near_pct:.1}% вернула {near} ≥ размера дальнего уровня {far_level} — \
         значит в неё попало то, что лежит на −{off:.0}% от mid. «Фикс» через расширение всех \
         полос до книги прошёл бы d1 и сломал бы смысл полос.",
        near_pct = NEAR_BAND * 100.0,
        off = FAR_OFFSET * 100.0
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════
// КРУГ 2 (`C-094` B2/B3/B5) — три оракула, каждый против ПРЕДЪЯВЛЕННОГО слома.
//
// Круг 1 был зелён против ЧЕСТНОЙ реализации — и критик показал мутанта `C-M68-1`, против
// которого он тоже зелён: «обновлять от книги только `row.band >= 0.60`». Набор, не падающий
// против конкретного слома, инварианта не пиннит. Ниже — ровно три слома, предъявленные
// вердиктом, и ни одного «для полноты».
// ═══════════════════════════════════════════════════════════════════════════════════════

/// Уровень внутри УЗКОЙ полосы, приходящий ТОЛЬКО дельтой. Заведомо внутри `NEAR_BAND`
/// (0.1 %), то есть там, где снапшот биржи данные имеет, — значит красное `d4` нельзя
/// списать на «дальних данных нет».
const NARROW_OFFSET: f64 = 0.0008;
/// Размер — крупный и НЕ кратный снапшотным двойкам, чтобы вклад читался однозначно.
const NARROW_SIZE: f64 = 300.0;

/// Журнал, где УЗКАЯ полоса меняется только дельтами: снапшот от такта к такту КОНСТАНТЕН,
/// а дельта кладёт крупный уровень внутрь `NEAR_BAND`.
///
/// Прод-форма сохранена: снапшот узкий (кап биржи), дельты идут чаще, цепочка `update_id`
/// непрерывна (разрыв дал бы реализации законное право дельту отбросить — тогда красное
/// не отличалось бы от честного отказа по разрыву).
fn build_narrow() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");

    let near = [0.0005_f64, 0.005, 0.010];
    let bids: Vec<Level> = near.iter().map(|o| lvl(MID * (1.0 - o), 2.0)).collect();
    let asks: Vec<Level> = near.iter().map(|o| lvl(MID * (1.0 + o), 2.0)).collect();

    for i in 0..12i64 {
        let ts = T0 + i * 100;
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: bids.clone(),
                asks: asks.clone(),
                ts_exch_ms: ts,
            },
        ))
        .expect("append snapshot");

        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Delta {
                bids: vec![lvl(MID * (1.0 - NARROW_OFFSET), NARROW_SIZE)],
                asks: vec![lvl(MID * (1.0 + NARROW_OFFSET), NARROW_SIZE)],
                first_update_id: (i as u64) * 2 + 1,
                final_update_id: (i as u64) * 2 + 2,
                prev_final_update_id: if i == 0 { None } else { Some((i as u64) * 2) },
                ts_exch_ms: ts + 10,
            },
        ))
        .expect("append delta");
    }
    j.flush().expect("flush");
    dir
}

/// **D4 (`C-094` B2) — КАЖДАЯ полоса читает книгу, а не только дальняя.**
///
/// Мутант критика `C-M68-1` обновлял от книги ТОЛЬКО `row.band >= 0.60` — и весь набор
/// круга 1 оставался зелёным, потому что `d1` смотрит на 60 %, `d2` на heatmap, а `d3`
/// проверяет лишь то, что узкая полоса не РАЗБУХЛА. Ни один не требовал, чтобы узкая полоса
/// ДВИГАЛАСЬ от дельты.
///
/// Здесь уровень `NARROW_SIZE` приходит дельтой ВНУТРЬ `NEAR_BAND`. Снапшот в этой полосе
/// даёт ровно `2.0` (единственный уровень на `0.0005`). Значит:
///   * реализация, считающая полосу по книге, обязана вернуть `≥ 2.0 + NARROW_SIZE`;
///   * snapshot-derived (сегодня И мутант `C-M68-1`) вернёт `2.0`.
///
/// Эталон АБСОЛЮТНЫЙ, а не «больше соседа»: относительный эталон уже однажды дал зелёный
/// оракул против ненаписанной реализации (см. §5 спеки, история ошибки `d1`).
#[test]
fn md_i8_d4_narrow_band_moves_with_delta_not_only_far_band() {
    let dir = build_narrow();
    let snap = snap_of(dir.path(), vec![NEAR_BAND, FAR_BAND]);

    let want_at_least = to_fixed(2.0 + NARROW_SIZE);
    let snapshot_only = to_fixed(2.0);

    for side in ["bid", "ask"] {
        let got = depth_of(&snap, NEAR_BAND, side);
        assert!(
            got != snapshot_only,
            "MD-I-8 d4 [{side}]: узкая полоса {NEAR_BAND} вернула РОВНО снапшотное {snapshot_only} \
             — значит она snapshot-derived. Дельта положила {NARROW_SIZE} внутрь полосы \
             (offset {NARROW_OFFSET}), и книга это видит. Так ведёт себя и мутант C-M68-1 \
             («обновлять только band >= 0.60»), против которого набор круга 1 был зелен."
        );
        assert!(
            got >= want_at_least,
            "MD-I-8 d4 [{side}]: узкая полоса {NEAR_BAND} дала {got}, ожидалось ≥ {want_at_least} \
             (снапшотные 2.0 + дельта {NARROW_SIZE}). Полоса не считает по книге."
        );
    }
}

/// **D5 (`C-094` B3) — смена смысла депт-серии обязана менять ИДЕНТИЧНОСТЬ чекпоинта.**
///
/// # Почему первая редакция этого оракула была ДЕФЕКТОМ, и это оставлено здесь намеренно
///
/// Первая редакция сравнивала `snapshot_from_checkpoint` (с ПУСТЫМ каталогом чекпоинта) с
/// полным реплеем и была ЗЕЛЁНОЙ с первого запуска. Пустой каталог означает fallback на
/// полный реплей — то есть оракул сравнивал полный реплей С САМИМ СОБОЙ. Тавтология, а не
/// проверка сходимости (`testing.md`: «фикстура строит оба источника из одного входа — это
/// тавтология, а не оракул»). Записано, потому что следующий, кто будет править этот набор,
/// обязан знать, на чём тут поскользнулись дважды (первый раз — `d1` круга 1, §5 спеки).
///
/// # Что пиннится на самом деле
///
/// `C-094` B3 называет ДВЕ половины, и обе нужны:
///   1. старый чекпоинт ОТКЛОНЯЕТСЯ — «версионным bump либо отдельным cache-semantics
///      discriminator»;
///   2. после развязки `snapshot_from_checkpoint == full replay`.
///
/// Половину 2 сегодня предъявить красной НЕЛЬЗЯ по существу: пока реализация целиком
/// snapshot-derived, обе стороны совпадают, и расхождение возникнет только ПОСЛЕ фикса —
/// то есть половина 2 есть страж РЕГРЕССА, а не RED. Красной сегодня может быть только
/// половина 1, и она здесь: `CKPT_SCHEMA_VERSION` обязан смениться вместе со смыслом
/// редьюсера, иначе чекпоинт, снятый ДО фикса, останется «валидным» по отпечатку и версии,
/// и warm-start отдаст snapshot-derived депт при коде, считающем по книге.
///
/// Это тот же приём, что epoch-tripwire контрактного слоя (`gates.md` §3): версия — не
/// украшение, а единственное, чем старый кэш отличим от нового.
///
/// `C-094` B3 прямо запрещает добиваться совпадения подгонкой `selector_fingerprint` или
/// семантики heatmap-close — поэтому оракул требует смены ВЕРСИИ, а не отпечатка.
#[test]
fn md_i8_d5_checkpoint_identity_changes_with_depth_semantics() {
    // Значение на момент написания оракула снято ЗАМЕРОМ, а не взято из спеки:
    //   $ grep -n 'pub const CKPT_SCHEMA_VERSION' crates/gateway/src/lib.rs
    //     79:pub const CKPT_SCHEMA_VERSION: u32 = 2;
    // Первая редакция этого ассерта требовала `>= 2` — то есть порог, УЖЕ выполненный, и
    // оракул был зелен, не проверяя ничего. Число в RED обязано сниматься с кода, иначе
    // проба пиннит собственную опечатку.
    const AT_SPEC_TIME: u32 = 2;
    assert!(
        gateway::CKPT_SCHEMA_VERSION > AT_SPEC_TIME,
        "MD-I-8 d5: CKPT_SCHEMA_VERSION = {} — не менялась (на момент спеки было {AT_SPEC_TIME}). Депт-серия переходит со снапшота \
         на книгу, то есть меняется СМЫСЛ сериализованного состояния редьюсера, а идентичность \
         чекпоинта осталась прежней. Значит чекпоинт, снятый ДО фикса, пройдёт проверку версии \
         и отпечатка, и warm-start отдаст snapshot-derived депт при коде, считающем по книге — \
         VB-I-2 на пути, который чистый реплей не проверяет (C-094 B3).",
        gateway::CKPT_SCHEMA_VERSION
    );
}

/// **D5b — СТРАЖ РЕГРЕССА к `d5`: bump версии не смеет сломать warm-start.**
///
/// Обратный вопрос мутационного контроля (`testing.md`, «что пришлось ослабить рядом»):
/// закрыть `d5` можно, просто подняв константу, и при этом сломать переиспользование
/// чекпоинта вовсе. Этот оракул держит вторую половину `C-094` B3 — выдача из чекпоинта
/// обязана совпадать с полным реплеем.
///
/// Эталон берётся из НЕЗАВИСИМОГО пути: `gateway::snapshot` — полный реплей с нуля, а не та
/// же функция, что собирает выдачу из чекпоинта.
///
/// Сегодня он ЗЕЛЁН, и это правильно: он сторож, а не RED. Красным он станет, если развязка
/// `d5` будет сделана ценой отказа от warm-start.
#[test]
fn md_i8_d5b_checkpoint_resume_still_equals_full_replay() {
    let dir = build_narrow();
    let ckpt = tempfile::tempdir().expect("ckpt tempdir");
    let sel = sel(vec![NEAR_BAND, FAR_BAND]);

    let (from_ckpt, _stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel,
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint обязан строиться");

    let full = snap_of(dir.path(), vec![NEAR_BAND, FAR_BAND]);

    for band in [NEAR_BAND, FAR_BAND] {
        for side in ["bid", "ask"] {
            let a = depth_of(&from_ckpt, band, side);
            let b = depth_of(&full, band, side);
            assert_eq!(
                a, b,
                "MD-I-8 d5b [{side}, полоса {band}]: warm-start дал {a}, полный реплей — {b}. \
                 Развязка d5 куплена ценой поломки переиспользования чекпоинта."
            );
        }
    }
}
