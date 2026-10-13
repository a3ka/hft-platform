//! M-05 RED (sacred, architect) — J3: resync-толерантное чтение через рваный фрейм.
//!
//! Боевой сценарий (VPS 2026-07-11): SIGKILL посреди BufWriter-flush оставляет рваный
//! фрейм В СЕРЕДИНЕ сегмента (valid frames | торн | valid frames). `read_all` жёстко
//! падает на первом CRC-mismatch (`lib.rs:142`) → на проде читалось лишь 37% событий.
//! Фикс (M-05 task 4): `journal::recover()` РЕСИНХронизируется через рваный фрейм и
//! возвращает ВСЕ валидные события обеих сторон; `read_all` остаётся strict для
//! DET-I-1 exact-replay. Падает КОМПАЙЛОМ (recover не существует) → engine-dev делает GREEN.

use contracts::{Event, EventKind, SysEvent};

fn frame(seq: u64) -> Vec<u8> {
    let ev = Event {
        seq,
        ts_mono_ns: seq,
        ts_wall_ms: seq as i64,
        kind: EventKind::Sys(SysEvent::Heartbeat),
    };
    let payload = postcard::to_stdvec(&ev).unwrap();
    let mut out = (payload.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&payload);
    out.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    out
}

#[test]
fn recover_resyncs_across_torn_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut seg = Vec::new();
    // Слева от повреждения — 2 валидных фрейма.
    seg.extend_from_slice(&frame(10));
    seg.extend_from_slice(&frame(11));
    // РВАНЫЙ фрейм: len=8, payload 8×0xFF, crc=0 (не совпадает с crc(0xFF*8)) —
    // имитация оборванного flush'а. read_all тут падает; recover обязан ресинкнуться.
    seg.extend_from_slice(&8u32.to_le_bytes());
    seg.extend_from_slice(&[0xFF; 8]);
    seg.extend_from_slice(&0u32.to_le_bytes());
    // Справа — ещё 2 валидных фрейма (как после рестарта рекордера).
    seg.extend_from_slice(&frame(12));
    seg.extend_from_slice(&frame(13));
    std::fs::write(dir.path().join("segment-00000000.jrnl"), &seg).unwrap();

    // read_all остаётся strict — на рваном фрейме Err (инвариант DET-I-1 сохраняется).
    assert!(
        journal::read_all(dir.path()).is_err(),
        "read_all обязан ОСТАТЬСЯ strict (Err на рваном фрейме) — DET-I-1 exact-replay"
    );

    // recover() — толерантный: возвращает ВСЕ валидные события обеих сторон повреждения.
    let evs = journal::recover(dir.path()).expect("recover не должен падать");
    let seqs: Vec<u64> = evs.iter().map(|e| e.seq).collect();
    assert_eq!(
        seqs,
        vec![10, 11, 12, 13],
        "recover обязан ресинкнуться через рваный фрейм и вернуть все валидные события"
    );
}
