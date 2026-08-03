//! M-57 (`TD-098`) — **активный сегмент не смеет пересканироваться на каждом тике**,
//! и счётчик работы обязан это ВИДЕТЬ.
//!
//! ## Замер, из-за которого milestone существует
//!
//! `stream_from` пропускает по `first_seq` только ЗАКРЫТЫЕ сегменты. Активный читается
//! С НАЧАЛА при каждом вызове: forward-скан с декодированием каждого события, лишние
//! отбрасываются фильтром (`crates/journal/src/segments.rs`, ветка `if ev.seq <= after`).
//!
//! Прямая проба (один сегмент, курсор на хвосте, приращение РОВНО 3 события):
//!
//! ```text
//!   2 000 событий в сегменте -> тик   3 488 мкс, выдано 3
//!  16 000 событий            -> тик  44 535 мкс, выдано 3
//! 128 000 событий            -> тик 200 247 мкс, выдано 3
//! ```
//!
//! Сегмент вырос в 64 раза — тик в 57 раз. На проде активный сегмент растёт до 1 GiB
//! (≈10.7 млн событий) ⇒ полный скан ≈17 СЕКУНД на тик при периоде push 250 мс.
//!
//! ## Почему прежние оракулы этого не поймали
//!
//! `ReadStats.events_decoded` инкрементируется ТОЛЬКО для ВЫДАННЫХ событий — пропущенные
//! фильтром не считаются. Во всех трёх строках замера выше счётчик показывает `3`. Sacred-оракул
//! M-53 `td083_tick_wallclock_does_not_grow_with_history` меряет именно его и считает свойство
//! доказанным; он показал бы те же 3 и при пересканировании гигабайта.
//!
//! **Слепота встроена в ИЗМЕРИТЕЛЬ, а не в фикстуру** — поэтому мутационный контроль её не
//! ловит: мутация меняет реализацию, а врёт прибор. Отсюда задача 1 milestone'а — честный
//! счётчик `events_scanned`, и только на нём строится главный оракул O-2.
//!
//! COMPILE-RED: поля `ReadStats.events_scanned` ещё не существует (задача 1).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

const BASE_MS: i64 = 1_784_116_800_000;

/// Сегмент намеренно ОГРОМНЫЙ (1 GiB) — ротации в тестах не происходит, и предметом
/// проверки остаётся ровно внутрисегментное поведение, как на проде.
fn cfg_single_segment() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 30,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m57".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

/// Маленький сегмент — для O-3 (ротация обязана происходить).
fn cfg_rotating() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 32 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m57".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(65_000.0 + (i % 13) as f64),
            size: to_fixed(0.5),
            side: if i % 3 == 0 { Side::Sell } else { Side::Buy },
            ts_exch_ms: BASE_MS + i * 10,
        },
    )
}

fn write_n(dir: &std::path::Path, from: i64, to: i64, cfg: WriterConfig) {
    let mut j = Journal::open_with(dir, cfg).expect("open_with");
    for i in from..to {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
}

/// Один тик: стрим от `after` до исчерпания. Возвращает (выдано, прочитано).
fn tick(dir: &std::path::Path, after: u64) -> (u64, u64) {
    let mut s =
        journal::stream_from(dir, EpochFilter::OwnCaptureOnly, Some(after)).expect("stream_from");
    let mut yielded = 0u64;
    while let Some(r) = s.next() {
        r.expect("event");
        yielded += 1;
    }
    // `events_scanned` — задача 1: сколько событий РЕАЛЬНО прочитано и декодировано,
    // включая отброшенные фильтром. Без него измерить пересканирование нечем.
    (yielded, s.events_scanned())
}

/// **O-1.** Счётчик честен: он видит прочитанное, а не только выданное.
///
/// На хвосте большого сегмента выдаётся 3 события. Слепой счётчик покажет 3 и при полном
/// скане, и при seek — то есть не различит исправную реализацию от сломанной. Честный обязан
/// показать РАЗНИЦУ: до фикса ≈ размеру сегмента, после ≈ приращению.
///
/// Здесь проверяется только СВОЙСТВО счётчика (`scanned >= yielded`, счётчик не константа),
/// а величина — в O-2. Разделено намеренно: O-1 может быть зелёным на сломанной реализации,
/// и это нормально — он про прибор, а не про предмет.
#[test]
fn o1_scanned_counter_is_honest() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_n(dir.path(), 0, 5_000, cfg_single_segment());
    let after = 4_999u64;
    write_n(dir.path(), 5_000, 5_003, cfg_single_segment());

    let (yielded, scanned) = tick(dir.path(), after);

    assert_eq!(yielded, 3, "O-1: фикстура сломана — выдано не 3 события");
    assert!(
        scanned >= yielded,
        "O-1: events_scanned ({scanned}) < events_decoded ({yielded}) — счётчик считает \
         меньше, чем выдано; это невозможно при честном учёте"
    );
    // Анти-вырождение: счётчик не должен быть константой или копией `yielded` по построению.
    // Проверяется в O-2 сравнением двух размеров; здесь фиксируем лишь ненулевое значение.
    assert!(
        scanned > 0,
        "O-1: events_scanned = 0 при трёх выданных событиях — счётчик не подключён"
    );
}

/// **O-2 (ГЛАВНЫЙ).** Работа тика пропорциональна ПРИРАЩЕНИЮ, а не размеру активного сегмента.
///
/// Сравнительное свойство: два журнала, отличающиеся ТОЛЬКО размером сегмента (×8), с
/// ОДИНАКОВЫМ приращением в 3 события. Выдаётся поровну, значит и читать нужно поровну.
///
/// Порог `2.5×` — с запасом в обе стороны и не зависит ни от машины, ни от подбора числа:
/// - исправная реализация (seek к позиции): отношение ≈1;
/// - сегодняшняя (скан с начала): отношение ≈8 (замер: время растёт линейно, 57× при 64×).
///
/// Абсолютный порог здесь был бы негоден — он зависит от ширины фикстуры и уже подводил в
/// M-56, где первая редакция оракула ПРОШЛА на коде с дефектом.
#[test]
fn o2_tick_scans_only_increment_not_whole_segment() {
    let scan_for = |n: i64| -> (u64, u64) {
        let dir = tempfile::tempdir().expect("tempdir");
        write_n(dir.path(), 0, n, cfg_single_segment());
        let after = (n - 1) as u64;
        write_n(dir.path(), n, n + 3, cfg_single_segment());
        let (yielded, scanned) = tick(dir.path(), after);
        assert_eq!(yielded, 3, "O-2: при {n} событиях выдано не 3");
        std::mem::forget(dir);
        (yielded, scanned)
    };

    let (_, small) = scan_for(2_000);
    let (_, big) = scan_for(16_000);

    let ratio = big as f64 / small.max(1) as f64;
    eprintln!("ЗАМЕР O-2: сегмент ×8 → scanned {small} → {big} (×{ratio:.2})");
    assert!(
        ratio < 2.5,
        "O-2 (TD-098): сегмент вырос в 8 раз — работа тика выросла в {ratio:.1} раза \
         ({small} → {big} прочитанных событий) при ОДИНАКОВОМ приращении в 3 события. \
         Значит активный сегмент читается С НАЧАЛА. На проде сегмент растёт до 1 GiB \
         (≈10.7 млн событий) ⇒ ≈17 секунд на тик при периоде push 250 мс."
    );
}

/// **O-3.** Позиция переживает РОТАЦИЮ сегмента: ни одно событие не потеряно и не выдано дважды.
///
/// Деградированный вход: дозапись, вызывающая переход на новый сегмент МЕЖДУ тиками. Это
/// граница, на которой byte-offset обязан сброситься — иначе смещение из старого файла
/// применится к новому и съест его начало.
#[test]
fn o3_position_survives_segment_rotation() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_n(dir.path(), 0, 400, cfg_rotating());

    // Первый тик: забираем всё, запоминаем хвост.
    let (y1, _s1) = tick(dir.path(), 0);
    assert!(y1 > 0, "O-3: первый тик пуст — фикстура не давит");
    let after = y1; // seq последнего выданного (seq начинается с 1)

    // Дозапись, гарантированно вызывающая ротацию (сегмент 32 KiB).
    write_n(dir.path(), 400, 900, cfg_rotating());

    let (y2, _s2) = tick(dir.path(), after);

    // Полный независимый проход — эталон, построенный ДРУГИМ путём.
    let mut full =
        journal::stream_from(dir.path(), EpochFilter::OwnCaptureOnly, None).expect("full stream");
    let mut total = 0u64;
    let mut seqs = Vec::new();
    while let Some(r) = full.next() {
        let ev = r.expect("event");
        seqs.push(ev.seq);
        total += 1;
    }

    assert_eq!(
        y1 + y2,
        total,
        "O-3: сумма двух тиков ({} + {} = {}) != полному проходу ({}). Значит при ротации \
         события потеряны или выданы дважды.",
        y1,
        y2,
        y1 + y2,
        total
    );

    let mut sorted = seqs.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        seqs.len(),
        "O-3: в полном проходе есть ДУБЛИКАТЫ seq — журнал или чтение нарушают порядок"
    );
}

/// **O-4.** Ускорение не смеет менять ДАННЫЕ: события seek-пути идентичны полному проходу.
///
/// `GW-I-8`/`VB-I-2`: ускорение, меняющее выдачу, — это расхождение, а не ускорение.
/// Эталон строится независимо: полный проход `stream_from(None)` с ручной фильтрацией.
#[test]
fn o4_seek_path_yields_identical_events() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_n(dir.path(), 0, 3_000, cfg_single_segment());
    let after = 2_000u64;
    write_n(dir.path(), 3_000, 3_010, cfg_single_segment());

    // Путь под проверкой.
    let mut s = journal::stream_from(dir.path(), EpochFilter::OwnCaptureOnly, Some(after))
        .expect("stream_from");
    let mut via_seek = Vec::new();
    while let Some(r) = s.next() {
        via_seek.push(r.expect("event"));
    }

    // Независимый эталон: полный проход + фильтрация вручную.
    let mut f = journal::stream_from(dir.path(), EpochFilter::OwnCaptureOnly, None).expect("full");
    let mut via_full = Vec::new();
    while let Some(r) = f.next() {
        let ev = r.expect("event");
        if ev.seq > after {
            via_full.push(ev);
        }
    }

    assert!(
        !via_seek.is_empty(),
        "O-4: seek-путь не выдал НИЧЕГО — сравнение выродилось"
    );
    assert_eq!(
        via_seek.len(),
        via_full.len(),
        "O-4: seek-путь выдал {} событий, полный проход — {}",
        via_seek.len(),
        via_full.len()
    );
    for (a, b) in via_seek.iter().zip(via_full.iter()) {
        assert_eq!(
            a.seq, b.seq,
            "O-4: расхождение seq между seek-путём и полным проходом"
        );
        assert_eq!(
            a.kind, b.kind,
            "O-4: событие seq={} различается по содержимому — ускорение ИЗМЕНИЛО данные",
            a.seq
        );
    }
}
