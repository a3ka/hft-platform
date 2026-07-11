//! M-05 RED (sacred, architect-only) — целостность журнала при unclean-рестарте.
//!
//! Кодирует БОЕВОЙ дефект (найден на прод-журнале VPS 2026-07-11): при SIGKILL
//! рекордера мета (`next_seq`) ОТСТАЁТ от сегмента (персистится только в flush()),
//! и при рестарте seq ОТКАТЫВАЕТСЯ → переиспользуется для ДРУГИХ событий (проверено:
//! seq 713710 = L2Snapshot в одном сегменте и Trade в другом). Нарушение JR-I
//! «seq монотонный, переживает рестарт» + DET-I-1.
//!
//! Фикс (M-05 task 3): `Journal::open` выводит next_seq из СКАНА последнего валидного
//! фрейма сегмента, а не из (возможно отстающей) меты. Тест обязан ПАДАТЬ на текущем
//! коде (берёт мету) и на заглушке (return meta) — анти-плацебо.

use std::fs;

use contracts::{EventKind, SysEvent};
use journal::{read_all, Journal};

/// J2 — next_seq авторитетен из сегмента, НЕ из отстающей меты; нет reuse seq.
#[test]
fn next_seq_authoritative_from_segment_not_stale_meta() {
    let dir = tempfile::tempdir().unwrap();

    // Пишем 200 фреймов (seq 0..199), честно флашим → сегмент содержит 200 фреймов.
    {
        let mut j = Journal::open(dir.path()).unwrap();
        for _ in 0..200 {
            j.append(EventKind::Sys(SysEvent::Heartbeat)).unwrap();
        }
        j.flush().unwrap();
    }
    let before = read_all(dir.path()).unwrap();
    assert_eq!(before.len(), 200, "предусловие: 200 фреймов в сегменте");

    // Имитация unclean-рестарта: мета ОТСТАЁТ от сегмента (как после SIGKILL посреди
    // батча — last flush меты был на seq=150, а сегмент уже дописал до 199).
    fs::write(dir.path().join("journal.meta"), 150u64.to_le_bytes()).unwrap();

    // Рестарт: открываем заново и дописываем одно событие.
    let new_seq = {
        let mut j = Journal::open(dir.path()).unwrap();
        let ev = j.append(EventKind::Sys(SysEvent::Heartbeat)).unwrap();
        j.flush().unwrap();
        ev.seq
    };

    // ТРЕБОВАНИЕ: новое событие обязано получить seq=200 (сразу за реальным концом
    // сегмента), НЕ 150 (отставшая мета). На текущем коде new_seq==150 → коллизия с
    // уже существующим seq 150 → тест ПАДАЕТ (RED). После фикса — GREEN.
    assert_eq!(
        new_seq, 200,
        "seq REUSE: next_seq взят из отставшей меты (150), а не из конца сегмента (200) \
         → коллизия seq, нарушение JR-I/DET-I-1"
    );

    // Дополнительно: в журнале не должно быть двух фреймов с одинаковым seq.
    let after = read_all(dir.path()).unwrap();
    let mut seqs: Vec<u64> = after.iter().map(|e| e.seq).collect();
    seqs.sort_unstable();
    let dups = seqs.windows(2).filter(|w| w[0] == w[1]).count();
    assert_eq!(dups, 0, "обнаружены дублирующиеся seq — reuse через рестарт");
}
