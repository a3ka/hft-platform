//! `F-035-3` (M-57, задача 8) — **`gateway::ReadStats.events_scanned` обязан быть ПРОБРОСОМ
//! журнального счётчика, а не псевдонимом `events_decoded`.**
//!
//! ЗАЧЕМ. Вердикт `R-039` §C.2 замерил: подмена ОДНОЙ строки
//! (`gateway/src/lib.rs`, `read_stats_from_stream`: `events_scanned: stream.events_scanned()`
//! → `stream.events_decoded()`) вместе с откатом механизма круга 2 возвращает P0-дефект
//! ЦЕЛИКОМ — и оставляет `verify_M-57.sh` зелёным 11/11, а оба прод-форменных оракула
//! `f035_1`/`f035_2` — зелёными. Причина в том, что ВСЕ оракулы M-57 уровня gateway читают
//! одно и то же поле, а само поле не пиннит никто: `testing.md` — «зависимый эталон мутация
//! ловит плохо», «оракул обязан мерить ТО, ЧТО обещает».
//!
//! ПОЧЕМУ ЗАДАЧА 8 СТОЯЛА `BLOCKED` И ПОЧЕМУ БОЛЬШЕ НЕ СТОИТ. Спека §8.1 верно отвергла
//! наивный ассерт `scanned > decoded` (после верного фикса на хвосте обе величины равны, и
//! такой оракул краснел бы на исправленном коде) и заключила, что расхождение достижимо лишь
//! свежей сессией от чекпоинта у хвоста — а публичного писателя чекпоинтов нет, значит нужна
//! правка impl. Заключение неверно, и это проверяется чтением кода, а не рассуждением:
//! `segments.rs:1159` инкрементирует `events_scanned` ДО фильтра `after_seq`, а `:1201`
//! инкрементирует `events_decoded` ТОЛЬКО для выданных событий (`continue` стоит выше).
//! Значит величины расходятся на ЛЮБОМ вызове с непустым `after_seq` — чекпоинт не нужен,
//! правка impl не нужна, оракул целиком укладывается в зону architect'а.
//! (Доккомментарий `events_decoded` при этом утверждает обратное — «включает события,
//! отфильтрованные по `after_seq`»; он ложен и назван в `F-6` как правка для engine-dev.)
//!
//! ЭТАЛОН НЕЗАВИСИМ. Ожидаемое число берётся не из константы и не из того же поля, а из
//! ПРЯМОГО журнального вызова с теми же аргументами — путь, которого подмена в gateway не
//! касается. Поэтому оракул переживает будущие оптимизации: если работа тика изменится,
//! обе стороны сдвинутся вместе, а сломается только НЕРАВЕНСТВО проброса.
//!
//! АНТИ-ПЛАЦЕБО ВСТРОЕН. Равенство `st.events_scanned == j_scanned` бессмысленно, если в
//! фикстуре `scanned == decoded`: тогда подмена на `decoded` тоже прошла бы. Поэтому
//! setup-guard требует `j_scanned > j_decoded` ДО проверки предмета — фикстура, не давящая
//! на инвариант, обязана убить прогон, а не позеленеть.
//!
//! КРИТЕРИЙ ПРИЁМКИ (`R-039` §G.2): композитная мутация §C.2 обязана КРАСНЕТЬ. Сегодня она
//! зелёная на 11/11.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;
/// Событий в журнале. Один сегмент — ротации быть не должно.
const N: u64 = 400;
/// Сколько событий остаётся ПОСЛЕ курсора. Всё, что до него, реализация обязана
/// прочитать и отбросить фильтром — именно на этом расходятся два счётчика.
const TAIL: u64 = 40;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1024 * 1024 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
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
            ts_exch_ms: D2_MS - (N as i64 * 100) + (i as i64 * 100),
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

fn journal_upto(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..n {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    dir
}

/// Реальные `seq` событий журнала: номера присваивает писатель, гадать о них нельзя.
fn seqs_of(dir: &std::path::Path) -> Vec<u64> {
    let mut s = journal::stream_from(dir, EpochFilter::OwnCaptureOnly, None).expect("stream_from");
    let out: Vec<u64> = (&mut s).filter_map(|r| r.ok()).map(|ev| ev.seq).collect();
    out
}

#[test]
fn f035_3_read_stats_scanned_is_journal_scanned_not_decoded() {
    let dir = journal_upto(N);
    let seqs = seqs_of(dir.path());
    assert_eq!(
        seqs.len() as u64,
        N,
        "SETUP НЕ СОСТОЯЛСЯ: в журнале {} событий вместо {N}",
        seqs.len()
    );
    // Курсор ставится так, чтобы после него остался ровно TAIL событий.
    let after_seq = seqs[(N - TAIL - 1) as usize];

    // ── ЭТАЛОН: прямой журнальный вызов с теми же аргументами (независимый путь) ──────
    let mut js = journal::stream_from(dir.path(), EpochFilter::OwnCaptureOnly, Some(after_seq))
        .expect("stream_from");
    let yielded = (&mut js).filter_map(|r| r.ok()).count() as u64;
    let j_scanned = js.events_scanned();
    let j_decoded = js.events_decoded();

    // ── СТРАЖ SETUP'А: фикстура обязана ДАВИТЬ на инвариант ──────────────────────────
    // Если два счётчика совпали, равенство ниже проходит и для подменённой реализации:
    // оракул стал бы плацебо самого себя.
    assert!(
        j_scanned > j_decoded,
        "SETUP НЕ СОСТОЯЛСЯ: в фикстуре scanned={j_scanned} == decoded={j_decoded}. \
         Расхождение возникает только когда часть событий читается и отбрасывается \
         фильтром `after_seq`; без него проверка проброса вырождается в тавтологию."
    );
    assert_eq!(
        j_decoded, yielded,
        "SETUP НЕ СОСТОЯЛСЯ: журнальный decoded={j_decoded} не совпал с числом выданных \
         событий {yielded} — эталон измерен неверно, сравнивать с ним нечего"
    );
    assert_eq!(
        yielded, TAIL,
        "SETUP НЕ СОСТОЯЛСЯ: после курсора выдано {yielded} событий вместо {TAIL}"
    );

    // ── ПРЕДМЕТ: тот же запрос через gateway ─────────────────────────────────────────
    // `frames_since_with_stats` — публичный stateless-вход, зовущий `stream_from` БЕЗ hint:
    // ровно тот путь, чьи счётчики собирает `read_stats_from_stream`.
    let (_frames, _cursor, st) = gateway::frames_since_with_stats(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor {
            upto_seq: Some(after_seq),
        },
        1_000_000,
    )
    .expect("frames_since_with_stats");

    assert_eq!(
        st.events_scanned, j_scanned,
        "F-035-3: `ReadStats.events_scanned` = {} при журнальном scanned={j_scanned} \
         (decoded={j_decoded}). Проброс подменён: измеритель, на котором стоит ВЕСЬ гейт \
         M-57, показывает не ту величину, которую обещает. Ровно эта подмена одной строкой \
         (R-039 §C.2) возвращала P0-дефект при зелёном гейте 11/11.",
        st.events_scanned
    );
    assert_eq!(
        st.events_decoded, j_decoded,
        "F-035-3: `ReadStats.events_decoded` = {} при журнальном decoded={j_decoded} — \
         второй счётчик тоже обязан оставаться собой; оба смысла сохраняются, один не \
         заменяет другой.",
        st.events_decoded
    );
}
