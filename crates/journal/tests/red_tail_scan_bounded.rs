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

/// Один тик: стрим от `after` (`None` — с самого начала) до исчерпания.
/// Возвращает `(выдано, прочитано)`.
///
/// ⚠️ `after: Option<u64>`, а не `u64` — по находке `C-059` §3.3. **`seq` в журнале начинается
/// с НУЛЯ**, поэтому `Some(0)` означает «отбросить событие seq=0», а не «взять всё с начала».
/// Первая редакция O-3 звала `tick(dir, 0)` для первого прохода и теряла первое событие
/// навсегда: оно не попадало ни в первый тик (отброшено фильтром), ни во второй (тот стартовал
/// уже дальше). Тест падал детерминированно и НЕЗАВИСИМО ОТ РЕАЛИЗАЦИИ — то есть был сломан
/// сам, а не ловил дефект.
fn tick(dir: &std::path::Path, after: Option<u64>) -> (u64, u64) {
    let mut s = journal::stream_from(dir, EpochFilter::OwnCaptureOnly, after).expect("stream_from");
    let mut yielded = 0u64;
    while let Some(r) = s.next() {
        r.expect("event");
        yielded += 1;
    }
    // `events_scanned` — задача 1: сколько событий РЕАЛЬНО прочитано и декодировано,
    // включая отброшенные фильтром. Без него измерить пересканирование нечем.
    (yielded, s.events_scanned())
}

/// Последний `seq` в журнале — честный курсор «я дочитал досюда».
fn tail_seq(dir: &std::path::Path) -> u64 {
    let mut s = journal::stream_from(dir, EpochFilter::OwnCaptureOnly, None).expect("stream_from");
    let mut last = 0u64;
    while let Some(r) = s.next() {
        last = r.expect("event").seq;
    }
    last
}

/// **O-1 (переписан по `C-059` §3.2).** Счётчик считает ПРОЧИТАННОЕ — доказывается ПРЯМОЙ
/// НИЖНЕЙ ГРАНИЦЕЙ от размера журнала, а не сравнением с самим собой.
///
/// ## Чем была плоха первая редакция
///
/// Она требовала лишь `scanned >= yielded` и `scanned > 0`. Критик показал эмпирически:
/// реализация `fn events_scanned(&self) -> u64 { self.events_decoded }` — **буквальный алиас,
/// без единой строчки реального фикса** — проходила и O-1, и O-2 (`ratio = 1.00` при пороге
/// `< 2.5`). То есть весь acceptance-гейт был проходим БЕЗ задачи 2, а P0-регресс на проде
/// остался бы неисправленным при формально закрытом milestone'е.
///
/// Корень: `events_scanned` не имел НИ ОДНОЙ точки верификации, независимой от
/// `events_decoded`. O-2 сравнивал счётчик сам с собой на двух размерах — а если он
/// тождественно равен `yielded` (жёстко 3 в обоих прогонах), сравнение не отличает «честно
/// измерено и мало» от «нечестно скопировано и мало».
///
/// ## Что проверяется теперь
///
/// Курсор ставится в СЕРЕДИНУ журнала: выдаётся половина событий, а вторая половина —
/// `N/2` штук — обязана быть ПРОЧИТАНА и отброшена фильтром (сегодня, до фикса) либо
/// пропущена seek'ом (после фикса). В обоих случаях верно одно: **`scanned` не может быть
/// меньше числа ВЫДАННЫХ**, и при этом он обязан отличаться от `yielded` хотя бы в одном из
/// двух режимов — иначе это алиас.
///
/// Ключ — вторая проверка: тот же журнал читается ПОЛНОСТЬЮ (`after = None`). Там выдаются
/// все `N` событий. Если `scanned` — алиас `yielded`, то на полном проходе он равен `N`, а на
/// частичном равен `N/2`, и отношение `scanned_full / scanned_half` = 2.0. Честный счётчик на
/// сегодняшней реализации даёт `N` в ОБОИХ случаях (оба прохода читают весь журнал), то есть
/// отношение ≈1.0. **Алиас и честный счётчик здесь различимы, и различие не зависит от того,
/// реализована ли задача 2.**
#[test]
fn o1_scanned_counts_reads_not_yields() {
    const N: i64 = 4_000;
    let dir = tempfile::tempdir().expect("tempdir");
    write_n(dir.path(), 0, N, cfg_single_segment());

    // Курсор в середине: выдаётся вторая половина, первая обязана быть прочитана или пропущена.
    let mid = (N / 2 - 1) as u64;
    let (yielded_half, scanned_half) = tick(dir.path(), Some(mid));
    // Полный проход: выдаётся всё.
    let (yielded_full, scanned_full) = tick(dir.path(), None);

    assert_eq!(
        yielded_full as i64, N,
        "O-1: полный проход выдал {yielded_full} из {N} — фикстура сломана"
    );
    assert!(
        yielded_half > 0 && (yielded_half as i64) < N,
        "O-1: частичный проход выдал {yielded_half} из {N} — курсор не в середине, \
         сравнение выродилось"
    );
    assert!(
        scanned_half >= yielded_half,
        "O-1: scanned ({scanned_half}) < yielded ({yielded_half}) — невозможно при честном учёте"
    );

    // ГЛАВНОЕ: алиас `scanned == yielded` даёт ratio ≈ 2.0 (N против N/2).
    // Честный счётчик на сегодняшней реализации (полный скан в обоих случаях) даёт ≈1.0.
    let ratio = scanned_full as f64 / scanned_half.max(1) as f64;
    eprintln!(
        "ЗАМЕР O-1: scanned полный={scanned_full} частичный={scanned_half} (×{ratio:.2}); \
         yielded {yielded_full}/{yielded_half}"
    );
    assert!(
        ratio < 1.5,
        "O-1 (TD-098): events_scanned ведёт себя как АЛИАС events_decoded. Полный проход \
         прочитал {scanned_full}, частичный — {scanned_half} (×{ratio:.2}), тогда как ОБА \
         читают один и тот же журнал из {N} событий и обязаны показать сопоставимую работу. \
         Значит счётчик считает выданное, а не прочитанное, и измерить пересканирование им \
         нельзя. Это ровно тот дефект прибора, ради которого milestone и существует."
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
        let after = tail_seq(dir.path());
        write_n(dir.path(), n, n + 3, cfg_single_segment());
        let (yielded, scanned) = tick(dir.path(), Some(after));
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

    // Первый тик: забираем ВСЁ (after=None — иначе Some(0) отбросил бы событие seq=0,
    // C-059 §3.3). Курсор берём фактический — последний seq, а не число событий.
    let (y1, _s1) = tick(dir.path(), None);
    assert!(y1 > 0, "O-3: первый тик пуст — фикстура не давит");
    let after = tail_seq(dir.path());

    // Дозапись, гарантированно вызывающая ротацию (сегмент 32 KiB).
    write_n(dir.path(), 400, 900, cfg_rotating());

    let (y2, _s2) = tick(dir.path(), Some(after));

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
    let after = tail_seq(dir.path()) / 2;
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
