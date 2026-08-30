//! RED M-51 — **DET-I-1** (sacred, architect-only): бит-идентичный реплей event-store.
//!
//! ## Что здесь пиннится
//!
//! `DET-I-1` — стержневой инвариант проекта (`docs/DESIGN.md` §1: «`replay(journal) ==
//! реальность` — бит-в-бит») и торговое предложение продукта (`§0`: «продаётся не сигнал, а
//! **доказуемость**: каждая цифра на экране выводится реплеем из журнала»). До M-51 он был
//! ЗАЯВЛЕН (`§22`: `DET-I-1 [ЕСТЬ]`), но не имел исполнимой формы: `TECH-DEBT.md` TD-007
//! честнее дока — «реализован ЧАСТИЧНО (seq+read_all); state_hash — нет». Аудит
//! (`research/measurements/td-007-determinism-coverage.md`) подтвердил замером: `state_hash`
//! не существует (`grep -rn "fn state_hash" crates/*/src` → 0), а `JR-I-4`
//! («`snapshot + tail == full replay` по `state_hash`», `docs/fa/journal.md:114`) объявлен
//! вместе с именем теста `test_snapshot_equals_full_replay`, которого в крейте НЕТ.
//!
//! ## Контракт, который обязана удовлетворить реализация
//!
//! ```text
//! pub struct ReplayDigest { events: u64, first_seq: Option<u64>, last_seq: Option<u64>,
//!                          state_hash: [u8; 32] }
//! pub fn replay_digest(dir, filter: EpochFilter,
//!                      from_seq: Option<u64>, to_seq: Option<u64>) -> io::Result<ReplayDigest>
//! ```
//! `state_hash` = SHA-256 над конкатенацией, для КАЖДОГО события в порядке возрастания `seq`:
//! `u32 LE (длина postcard-payload) ‖ postcard(Event)`. Длина в префиксе — не украшение:
//! без неё конкатенация неоднозначна (два разных потока дают одни байты).
//!
//! Дайджест обязан быть функцией **потока событий**, а не файлов на диске: сжатие сегмента
//! (`.zst`), иная нарезка на сегменты, иной порядок файлов в каталоге не имеют права его
//! изменить (`det_4`). Это ровно то, что продаёт продукт: COLD-архив сжат, а цифра на экране
//! обязана воспроизвестись.
//!
//! ## Анти-плацебо
//!
//! Заглушка `fn replay_digest(..) -> Ok(ReplayDigest::default())` (константный хэш) прошла бы
//! `det_1`/`det_4` — обе стороны равны. Поэтому оракул НЕ ограничивается самосравнением:
//!  - `det_2` сверяет дайджест с **НЕЗАВИСИМЫМ эталоном**, посчитанным в самом тесте
//!    (принцип `common/mod.rs`: оракул не имеет права опираться на ту функцию крейта,
//!    корректность которой он проверяет);
//!  - `det_3` требует РАЗЛИЧИЯ на различающихся входах (одно поле одного события; префикс
//!    против полного журнала) — константная заглушка падает здесь немедленно.

mod common;

use common::{cfg_with, snap, trade};

use contracts::{DataSource, Event, EventKind, MdPayload};
use journal::{EpochFilter, Journal, WriterConfig};
use sha2::{Digest, Sha256};

/// НЕЗАВИСИМЫЙ эталон `state_hash`: считается ЗДЕСЬ, из `Vec<Event>`, без единого вызова
/// проверяемой функции. Формат дублируется сознательно (см. док-коммент `common/mod.rs`).
fn reference_state_hash(events: &[Event]) -> [u8; 32] {
    let mut h = Sha256::new();
    for ev in events {
        let p = postcard::to_stdvec(ev).expect("postcard ser");
        h.update((p.len() as u32).to_le_bytes());
        h.update(&p);
    }
    h.finalize().into()
}

fn cfg_epoch(source: DataSource, epoch: &str) -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 64 * 1024,
        min_free_bytes: 0,
        source,
        provenance: "det-test".to_string(),
        epoch_id: epoch.to_string(),
    }
}

/// Журнал из `n` крупных событий с МЕЛКИМ `max_segment_bytes` → заведомо несколько сегментов
/// (граница сегмента внутри окна — штатная форма прода, не экзотика).
fn build(n: u64) -> (tempfile::TempDir, Vec<u64>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut seqs = Vec::new();
    {
        let mut j = Journal::open_with(dir.path(), cfg_with(16 * 1024, "det-test")).expect("open");
        for i in 0..n {
            // Чередуем крупные (snap ~2.4 KiB) и мелкие (trade ~48 B) — МНОЖЕСТВЕННОСТЬ форм
            // событий в одном потоке; равномерный поток скрыл бы зависимость от формы.
            let kind = if i % 3 == 0 { snap(i) } else { trade(i) };
            seqs.push(j.append(kind).expect("append").seq);
        }
        j.flush().expect("flush");
    }
    (dir, seqs)
}

fn all_events(dir: &std::path::Path, filter: EpochFilter) -> Vec<Event> {
    journal::stream(dir, filter)
        .expect("stream")
        .map(|e| e.expect("event"))
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_1 — повторный реплей одного и того же диапазона бит-идентичен.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_1_replay_twice_is_bit_identical() {
    let (dir, _seqs) = build(120);

    let a = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("digest a");
    let b = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("digest b");
    let c = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("digest c");

    assert_eq!(
        a.state_hash, b.state_hash,
        "DET-I-1: два реплея одного диапазона дали РАЗНЫЙ state_hash — «replay(journal) == \
         реальность бит-в-бит» не выполняется"
    );
    assert_eq!(
        b.state_hash, c.state_hash,
        "DET-I-1: третий реплей разошёлся"
    );
    assert_eq!(
        (a.events, a.first_seq, a.last_seq),
        (b.events, b.first_seq, b.last_seq)
    );

    // Предусловие фикстуры: журнал РЕАЛЬНО многосегментный (иначе «граница сегмента» не
    // проверяется и det_1 деградирует в тривиальный кейс).
    let n_seg = journal::list_segments(dir.path()).expect("segments").len();
    assert!(
        n_seg >= 2,
        "фикстура обязана дать >=2 сегмента, а дала {n_seg}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_2 — дайджест есть функция ПОТОКА СОБЫТИЙ (сверка с независимым эталоном).
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_2_digest_equals_independent_reference_over_event_stream() {
    let (dir, seqs) = build(120);
    let events = all_events(dir.path(), EpochFilter::OwnCaptureOnly);
    assert_eq!(
        events.len(),
        120,
        "фикстура: поток обязан отдать все события"
    );

    let d = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("digest");

    assert_eq!(
        d.state_hash,
        reference_state_hash(&events),
        "state_hash не совпал с НЕЗАВИСИМЫМ эталоном sha256(len_le32 ‖ postcard(Event)) по \
         событиям в порядке seq — дайджест считает не то, что объявлено контрактом \
         (или считает не поток событий, а байты файлов)"
    );
    assert_eq!(
        d.events, 120,
        "events обязан считать РЕАЛЬНО пройденные события"
    );
    assert_eq!(d.first_seq, Some(seqs[0]), "first_seq");
    assert_eq!(
        d.last_seq,
        Some(*seqs.last().expect("seqs непуст")),
        "last_seq"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_3 — АНТИ-ПЛАЦЕБО: различающиеся входы обязаны давать различающиеся дайджесты.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_3_digest_discriminates_different_streams() {
    // ⚠ ЗАПРЕЩЁННАЯ формулировка (грабли, на которые эта редакция оракула уже наступила):
    // «записать ДВА журнала с разным содержимым и потребовать разных дайджестов» ДОКАЗЫВАЕТ
    // НИЧЕГО. `Journal::append` штампует `ts_wall_ms`/`ts_mono_ns` из `SystemTime::now()`
    // (`journal/src/lib.rs:205`) — два независимо записанных журнала различаются ВСЕГДА, даже
    // при идентичной полезной нагрузке. Такой assert зелен и на константной заглушке, если она
    // хоть как-то смотрит на вход, и зелен по неверной причине во всех остальных случаях.
    // Различение проверяется на ОДНОМ журнале — где источник различия контролируем.

    // (а) Дайджест обязан СОВПАСТЬ с эталоном по реальному потоку и РАЗОЙТИСЬ с эталоном по
    //     потоку, в котором изменено ОДНО поле ОДНОГО события. Оба эталона считаются здесь,
    //     из одних и тех же `Event` — wall-clock как источник различия исключён по построению.
    let (dir, seqs) = build(120);
    let events = all_events(dir.path(), EpochFilter::OwnCaptureOnly);
    let d = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("digest");

    let mut perturbed = events.clone();
    let mut bumped = false;
    for ev in perturbed.iter_mut() {
        if let EventKind::Md(md) = &mut ev.kind {
            if let MdPayload::Trade { size, .. } = &mut md.payload {
                *size += 1; // один тик fixed-point — минимальное возможное различие
                bumped = true;
                break;
            }
        }
    }
    assert!(
        bumped,
        "фикстура: в потоке обязан быть хотя бы один Trade для возмущения"
    );

    assert_eq!(
        d.state_hash,
        reference_state_hash(&events),
        "дайджест разошёлся с эталоном по РЕАЛЬНОМУ потоку"
    );
    assert_ne!(
        d.state_hash,
        reference_state_hash(&perturbed),
        "АНТИ-ПЛАЦЕБО: поток, отличающийся ОДНИМ полем одного события, дал ТОТ ЖЕ state_hash — \
         дайджест не зависит от данных и не доказывает ничего"
    );

    // (б) Префикс обязан отличаться от полного журнала (иначе дайджест не видит длину).
    let prefix = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        None,
        Some(seqs[118]),
    )
    .expect("prefix");
    assert_eq!(prefix.events, 119, "префикс обязан покрыть 119 событий");
    assert_ne!(
        d.state_hash, prefix.state_hash,
        "АНТИ-ПЛАЦЕБО: полный журнал и его префикс без последнего события дали ОДИН state_hash"
    );

    // (в) Два РАЗНЫХ окна одного журнала обязаны различаться — источник различия здесь
    //     заведомо не wall-clock, а содержимое окна.
    let w1 = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(seqs[0]),
        Some(seqs[59]),
    )
    .expect("w1");
    let w2 = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(seqs[60]),
        Some(seqs[119]),
    )
    .expect("w2");
    assert_eq!(w1.events, w2.events, "фикстура: окна одинаковой ДЛИНЫ");
    assert_ne!(
        w1.state_hash, w2.state_hash,
        "АНТИ-ПЛАЦЕБО: два разных окна одинаковой длины дали ОДИН state_hash — дайджест видит \
         только количество событий, но не их содержимое"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_4 — сжатие сегмента НЕ меняет дайджест (raw ≡ .zst). Ядро продуктового обещания.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_4_compaction_does_not_change_digest() {
    let (dir, _seqs) = build(200);

    let before = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("before");
    let files_before = common::ls(dir.path());

    let reports = journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact");
    assert!(
        !reports.is_empty(),
        "фикстура: компакция обязана реально сжать хотя бы один закрытый сегмент, иначе \
         свойство raw≡.zst не проверяется"
    );
    let files_after = common::ls(dir.path());
    assert_ne!(
        files_before, files_after,
        "фикстура: набор файлов на диске обязан ИЗМЕНИТЬСЯ (иначе тест ничего не проверил)"
    );
    assert!(
        files_after.iter().any(|f| f.ends_with(".jrnl.zst"))
            && files_after.iter().any(|f| f.ends_with(".jrnl")),
        "фикстура: обязана получиться СМЕШАННАЯ форма (raw + .zst в одном каталоге) — \
         замеренная форма прода, а не одноформатная: {files_after:?}"
    );

    let after =
        journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None).expect("after");
    assert_eq!(
        before.state_hash, after.state_hash,
        "DET-I-1: компакция изменила state_hash — дайджест считает БАЙТЫ ФАЙЛОВ, а не поток \
         событий. Продуктовое следствие: цифра, посчитанная до архивации, не воспроизводится \
         после неё"
    );
    assert_eq!(
        (before.events, before.first_seq, before.last_seq),
        (after.events, after.first_seq, after.last_seq),
        "DET-I-1: компакция изменила счётчики реплея"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_5 — окно [from, to] включительно; окно пересекает границу сегмента И границу raw/.zst.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_5_window_is_inclusive_and_crosses_segment_and_format_boundary() {
    let (dir, seqs) = build(200);
    journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact");

    let events = all_events(dir.path(), EpochFilter::OwnCaptureOnly);
    let from = seqs[40];
    let to = seqs[150];

    let d = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(from),
        Some(to),
    )
    .expect("window");
    let expect: Vec<Event> = events
        .iter()
        .filter(|e| e.seq >= from && e.seq <= to)
        .cloned()
        .collect();
    assert_eq!(
        expect.len(),
        111,
        "фикстура: окно [40..150] включительно = 111 событий"
    );
    assert_eq!(
        d.state_hash,
        reference_state_hash(&expect),
        "окно посчитано не по контракту (границы обязаны быть ВКЛЮЧИТЕЛЬНЫМИ, окно обязано \
         сшивать сегменты и оба формата — raw и .zst)"
    );
    assert_eq!(
        (d.events, d.first_seq, d.last_seq),
        (111, Some(from), Some(to))
    );

    // Повторный вызов того же окна — бит-идентичен (DET-I-1 на уровне окна, не только журнала).
    let d2 = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(from),
        Some(to),
    )
    .expect("window 2");
    assert_eq!(
        d.state_hash, d2.state_hash,
        "DET-I-1: повторный реплей окна разошёлся"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_6 — ДЕГРАДИРОВАННЫЙ вход: пусто / один / вырожденное окно / окно вне диапазона.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_6_degenerate_inputs_are_defined_and_stable() {
    // ГРАНИЦА «пусто»: журнал без событий. Пустой поток — не ошибка, а определённое состояние.
    let empty_a = tempfile::tempdir().expect("tempdir");
    let empty_b = tempfile::tempdir().expect("tempdir");
    for d in [&empty_a, &empty_b] {
        let mut j = Journal::open_with(d.path(), cfg_with(16 * 1024, "det-test")).expect("open");
        j.flush().expect("flush");
    }
    let ea = journal::replay_digest(empty_a.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("empty a");
    let eb = journal::replay_digest(empty_b.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("empty b");
    assert_eq!(
        (ea.events, ea.first_seq, ea.last_seq),
        (0, None, None),
        "пустой журнал"
    );
    assert_eq!(
        ea.state_hash, eb.state_hash,
        "два пустых журнала обязаны дать ОДИН state_hash (дайджест не имеет права зависеть от \
         пути каталога/времени создания)"
    );
    assert_eq!(
        ea.state_hash,
        reference_state_hash(&[]),
        "state_hash пустого потока обязан быть sha256 пустого входа — определён, не sentinel"
    );

    // ГРАНИЦА «один элемент».
    let one = tempfile::tempdir().expect("tempdir");
    let s1 = {
        let mut j = Journal::open_with(one.path(), cfg_with(16 * 1024, "det-test")).expect("open");
        let s = j.append(trade(0)).expect("append").seq;
        j.flush().expect("flush");
        s
    };
    let d1 =
        journal::replay_digest(one.path(), EpochFilter::OwnCaptureOnly, None, None).expect("one");
    assert_eq!(
        (d1.events, d1.first_seq, d1.last_seq),
        (1, Some(s1), Some(s1))
    );
    assert_ne!(
        d1.state_hash, ea.state_hash,
        "журнал из одного события не имеет права совпасть с пустым"
    );

    // ГРАНИЦА «окно из одного seq» (from == to).
    let (dir, seqs) = build(60);
    let point = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(seqs[7]),
        Some(seqs[7]),
    )
    .expect("point");
    assert_eq!(
        point.events, 1,
        "окно from==to обязано покрыть РОВНО одно событие"
    );

    // ОТСУТСТВИЕ: окно за пределами журнала — пустой результат, НЕ ошибка и НЕ «весь журнал».
    let last = *seqs.last().expect("seqs непуст");
    let beyond = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(last + 1_000),
        None,
    )
    .expect("beyond");
    assert_eq!(
        (beyond.events, beyond.first_seq, beyond.last_seq),
        (0, None, None),
        "окно за пределами журнала обязано дать ПУСТО, а не молча весь журнал"
    );
    assert_eq!(
        beyond.state_hash, ea.state_hash,
        "пустое окно ≡ пустой поток"
    );

    // ГРАНИЦА «перевёрнутое окно» (from > to) — пусто, не паника и не весь журнал.
    let inverted = journal::replay_digest(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        Some(seqs[30]),
        Some(seqs[10]),
    )
    .expect("inverted");
    assert_eq!(
        inverted.events, 0,
        "перевёрнутое окно (from > to) обязано дать ПУСТО — реализация не имеет права \
         додумывать за вызывающего"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_7 — ЛЕГИТИМНОЕ расхождение: разный EpochFilter = разные ОКНА, а не разные ответы.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_7_epoch_filter_is_a_window_not_a_divergence() {
    // Три эпохи в одном каталоге — реальный сценарий после докупки истории
    // (форма из red_segments_epochs.rs).
    let dir = tempfile::tempdir().expect("tempdir");
    for (src, epoch, n) in [
        (DataSource::OwnCapture, "own-2026-07", 10u64),
        (DataSource::Vendor, "vendor-2024", 20),
        (DataSource::Synthetic, "synth-x", 30),
    ] {
        let mut j = Journal::open_with(dir.path(), cfg_epoch(src, epoch)).expect("open epoch");
        for i in 0..n {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let own =
        journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None).expect("own");
    let all = journal::replay_digest(dir.path(), EpochFilter::All, None, None).expect("all");

    assert_eq!(
        own.events, 10,
        "OwnCaptureOnly обязан видеть только собственный захват"
    );
    assert_eq!(all.events, 60, "All обязан видеть все три эпохи");
    assert_ne!(
        own.state_hash, all.state_hash,
        "разные окна обязаны давать разные дайджесты (иначе фильтр не влияет на реплей)"
    );

    // Но КАЖДОЕ окно стабильно — это и есть «легитимное расхождение, не дефект DET-I-1».
    for f in [EpochFilter::OwnCaptureOnly, EpochFilter::All] {
        let x = journal::replay_digest(dir.path(), f.clone(), None, None).expect("x");
        let y = journal::replay_digest(dir.path(), f.clone(), None, None).expect("y");
        assert_eq!(
            x.state_hash, y.state_hash,
            "DET-I-1: реплей с фиксированным EpochFilter {f:?} недетерминирован"
        );
    }

    // Явно перечисленная эпоха — тоже окно, и тоже стабильна.
    let explicit = EpochFilter::Explicit(vec!["vendor-2024".to_string()]);
    let e1 = journal::replay_digest(dir.path(), explicit.clone(), None, None).expect("e1");
    let e2 = journal::replay_digest(dir.path(), explicit, None, None).expect("e2");
    assert_eq!(
        e1.events, 20,
        "Explicit(vendor-2024) обязан отдать ровно свою эпоху"
    );
    assert_eq!(
        e1.state_hash, e2.state_hash,
        "DET-I-1: Explicit-окно недетерминировано"
    );
    assert_ne!(
        e1.state_hash, own.state_hash,
        "разные эпохи — разные дайджесты"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_8 — дайджест согласован с ОБОИМИ путями чтения крейта и со склейкой окон.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_8_digest_agrees_with_both_reader_paths_and_window_composition() {
    // ⚠ Здесь НЕЛЬЗЯ проверять «нарезка на сегменты не влияет» сравнением ДВУХ ЖУРНАЛОВ,
    // записанных с разным `max_segment_bytes`: `append` штампует wall-clock (см. det_3), и
    // такие журналы различаются по построению — оракул падал бы на КОРРЕКТНОЙ реализации.
    // Независимость от хранения проверяется на ОДНОМ журнале: `det_4` (компакция меняет файлы,
    // но не дайджест) и здесь — согласованность с двумя независимыми путями чтения.
    let (dir, seqs) = build(150);
    let n_seg = journal::list_segments(dir.path()).expect("segments").len();
    assert!(
        n_seg >= 3,
        "фикстура: ожидается >=3 сегмента, получено {n_seg}"
    );

    let d = journal::replay_digest(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("digest");

    // (а) `read_all` — путь, независимый от `stream`. Реализация, ошибающаяся на границе
    //     сегмента в одном из путей, здесь разойдётся.
    let via_read_all = journal::read_all(dir.path()).expect("read_all");
    assert_eq!(
        via_read_all.len(),
        150,
        "фикстура: read_all обязан отдать все события"
    );
    assert_eq!(
        d.state_hash,
        reference_state_hash(&via_read_all),
        "дайджест разошёлся с эталоном по пути `read_all` — два пути чтения крейта дают разный \
         поток событий (расхождение на границе сегмента/формата)"
    );

    // (б) СКЛЕЙКА ОКОН: дайджест целого обязан совпасть с дайджестом, посчитанным по
    //     последовательным непересекающимся окнам. Ловит потерю/дубль события на стыке —
    //     ровно то, что делает «догон от чекпоинта» (DESIGN §14) неверным.
    let cuts = [0usize, 37, 88, 121, 150];
    let mut joined: Vec<Event> = Vec::new();
    let mut total = 0u64;
    for w in cuts.windows(2) {
        let (lo, hi) = (w[0], w[1] - 1);
        let part = journal::replay_digest(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            Some(seqs[lo]),
            Some(seqs[hi]),
        )
        .expect("part");
        total += part.events;
        joined.extend(
            via_read_all
                .iter()
                .filter(|e| e.seq >= seqs[lo] && e.seq <= seqs[hi])
                .cloned(),
        );
    }
    assert_eq!(
        total, d.events,
        "сумма окон обязана покрыть журнал РОВНО один раз"
    );
    assert_eq!(
        d.state_hash,
        reference_state_hash(&joined),
        "склейка последовательных окон разошлась с полным реплеем — событие потеряно или \
         продублировано на стыке окон"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_9 — TD-072: DET-I-1 на СМЕШАННОМ журнале (снапшот + дельта).
//
// Почему оракул понадобился (R-019 F6 → TD-072). До этого теста все входы DET-I-*
// в этом файле были снапшот-/sys-только: `grep -c L2Delta` по файлу → 0. Довод
// «`replay_digest` тип-агностичен, значит смешанный поток безопасен» верен ПО СУЩЕСТВУ,
// но это рассуждение, а не оракул: CT-RFC-06 §5 сам называет его аргументом. Прод пишет
// смешанный поток для BTC с 2026-07-21 (M-18), а M-45 расширяет состав символов —
// то есть под оракулом обязан быть тот поток, который реально лежит в журнале.
//
// Анти-плацебо — САМОЕ ВАЖНОЕ здесь. Реализация, которая молча ПРОПУСКАЕТ `L2Delta`
// (например `_ => continue` в обходе payload'ов), пройдёт проверку «реплей ×3 бит-идентичен»
// с блеском: она стабильно игнорирует одно и то же. Поэтому одной стабильности мало —
// §2 ниже требует, чтобы дайджест РАЗЛИЧАЛ два журнала, отличающиеся ТОЛЬКО содержимым
// дельты. Тест, состоящий из одного лишь §1, был бы фикстурой счастливого пути
// (`.claude/rules/testing.md`).
//
// Чек-лист деградированного входа пройден намеренно:
//   • асимметрия     — дельта, где обновлена ТОЛЬКО одна сторона (asks пуст) — штатная форма
//                      диффа, а не экзотика; симметричный вход скрыл бы зависимость от стороны;
//   • отсутствие     — уровень, которого в дельте НЕТ, не значит «удалить» (семантика §1
//                      CT-RFC-06); поток фиксируется как есть, реплей не додумывает;
//   • множественность— несколько дельт подряд между двумя якорями, а не одна;
//   • границы        — `size == 0` (явный remove от биржи) и ПУСТАЯ дельта (обе стороны
//                      пусты) — вырожденные, но легальные формы;
//   • обе рыночные   — `prev_final_update_id: None` (спот) и `Some(..)` (перп, чейн по `pu`).
//     семантики
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Дельта с управляемым содержимым. `bid_size == 0` → явный remove-уровень (wire-семантика
/// Binance `@depth`: qty="0"), пустые векторы → вырожденная, но легальная дельта.
fn delta(i: u64, bid_size: i64, with_asks: bool, pu: Option<u64>) -> EventKind {
    let bids = if bid_size < 0 {
        Vec::new()
    } else {
        vec![contracts::Level {
            price: 6_400_000_000_000 + i as i64,
            size: bid_size,
        }]
    };
    let asks = if with_asks {
        vec![contracts::Level {
            price: 6_400_100_000_000 + i as i64,
            size: 500 + i as i64,
        }]
    } else {
        Vec::new()
    };
    EventKind::md(
        contracts::Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids,
            asks,
            first_update_id: 1_000 + i,
            final_update_id: 1_000 + i,
            prev_final_update_id: pu,
            ts_exch_ms: common::T0 + i as i64,
        },
    )
}

/// Смешанный журнал: якорь-снапшот, затем НЕСКОЛЬКО дельт разной формы, затем сделка.
/// `delta_size` управляет содержимым ОДНОЙ дельты — на этом строится §2 (дискриминация).
fn build_mixed(delta_size: i64) -> (tempfile::TempDir, Vec<u64>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut seqs = Vec::new();
    {
        let mut j = Journal::open_with(dir.path(), cfg_with(16 * 1024, "det-test")).expect("open");
        for blk in 0..6u64 {
            // якорь
            seqs.push(j.append(snap(blk)).expect("append").seq);
            // дельта, обновляющая ТОЛЬКО bid — АСИММЕТРИЯ (asks не упомянут ⇒ не изменился)
            seqs.push(
                j.append(delta(blk * 10, delta_size, false, None))
                    .expect("append")
                    .seq,
            );
            // дельта-remove: size == 0 — граница, а не "пустое поле"
            seqs.push(
                j.append(delta(blk * 10 + 1, 0, false, None))
                    .expect("append")
                    .seq,
            );
            // перп-форма: чейн по `pu` (Some) + обе стороны — МНОЖЕСТВЕННОСТЬ форм
            seqs.push(
                j.append(delta(blk * 10 + 2, 700, true, Some(1_000 + blk * 10 + 1)))
                    .expect("append")
                    .seq,
            );
            // вырожденная: обе стороны пусты — легальна, дайджест обязан её УЧЕСТЬ
            seqs.push(
                j.append(delta(blk * 10 + 3, -1, false, None))
                    .expect("append")
                    .seq,
            );
            seqs.push(j.append(trade(blk)).expect("append").seq);
        }
        j.flush().expect("flush");
    }
    (dir, seqs)
}

#[test]
fn det_9_mixed_snapshot_delta_journal_is_bit_identical_and_delta_sensitive() {
    let (dir, seqs) = build_mixed(1_234);

    // Смешанность — предусловие теста, а не побочный факт: если фикстура перестанет
    // содержать дельты (например, изменится helper), тест обязан упасть ЗДЕСЬ, а не
    // молча выродиться в ещё один снапшот-only прогон (это и был дефект TD-072).
    let evs = all_events(dir.path(), EpochFilter::All);
    let n_delta = evs
        .iter()
        .filter(|e| {
            matches!(&e.kind, EventKind::Md(md) if matches!(md.payload, MdPayload::L2Delta { .. }))
        })
        .count();
    let n_snap = evs
        .iter()
        .filter(|e| {
            matches!(&e.kind, EventKind::Md(md) if matches!(md.payload, MdPayload::L2Snapshot { .. }))
        })
        .count();
    assert!(
        n_delta >= 20 && n_snap >= 6,
        "фикстура перестала быть СМЕШАННОЙ (дельт={n_delta}, снапшотов={n_snap}) — \
         оракул TD-072 потерял предмет проверки"
    );

    // ─── §1. Бит-идентичность ×3 на смешанном потоке ───────────────────────────────────
    let lo = seqs[0];
    let hi = *seqs.last().expect("seqs");
    let a =
        journal::replay_digest(dir.path(), EpochFilter::All, Some(lo), Some(hi)).expect("digest a");
    let b =
        journal::replay_digest(dir.path(), EpochFilter::All, Some(lo), Some(hi)).expect("digest b");
    let c =
        journal::replay_digest(dir.path(), EpochFilter::All, Some(lo), Some(hi)).expect("digest c");
    assert_eq!(
        a.state_hash, b.state_hash,
        "DET-I-1: два реплея СМЕШАННОГО журнала (снапшот+дельта) дали разный state_hash"
    );
    assert_eq!(
        a.state_hash, c.state_hash,
        "DET-I-1: третий реплей разошёлся"
    );
    assert_eq!(a.events, b.events, "DET-I-1: счётчик событий нестабилен");

    // ─── §1b. Совпадение с НЕЗАВИСИМЫМ эталоном ────────────────────────────────────────
    // Ловит реализацию, которая стабильно, но НЕВЕРНО сворачивает поток с дельтами.
    assert_eq!(
        a.state_hash,
        reference_state_hash(&evs),
        "DET-I-1: дайджест смешанного журнала разошёлся с независимым эталоном — \
         поток свёрнут не как «каждое событие ровно один раз»"
    );
    assert_eq!(
        a.events as usize,
        evs.len(),
        "DET-I-1: часть событий смешанного потока не попала в дайджест"
    );

    // ─── §2. АНТИ-ПЛАЦЕБО: дайджест обязан РАЗЛИЧАТЬ содержимое дельты ─────────────────
    // Два журнала отличаются ТОЛЬКО полем `size` внутри L2Delta. Реализация, молча
    // пропускающая дельты (`_ => continue`), даст одинаковый хэш и упадёт здесь —
    // при том что §1 она проходит.
    let (dir2, seqs2) = build_mixed(9_999);
    let d2 = journal::replay_digest(
        dir2.path(),
        EpochFilter::All,
        Some(seqs2[0]),
        Some(*seqs2.last().expect("seqs2")),
    )
    .expect("digest2");
    assert_eq!(
        a.events, d2.events,
        "фикстуры обязаны совпадать по числу событий — иначе §2 сравнивает не то"
    );
    assert_ne!(
        a.state_hash, d2.state_hash,
        "DET-I-1/TD-072: два журнала, отличающиеся ТОЛЬКО содержимым L2Delta, дали \
         ОДИНАКОВЫЙ state_hash — значит реплей игнорирует дельты. Это тот самый дефект, \
         ради которого TD-072 заведён: снапшот-only фикстуры его не видят"
    );
}
