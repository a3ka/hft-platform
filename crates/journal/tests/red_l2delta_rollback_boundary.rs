//! RED M-18 / CT-RFC-04 (sacred, architect-only): ROLLBACK-SAFETY (C-018 risk-critic blocking).
//!
//! Проблема: pre-M18 бинарь НЕ декодит `MdPayload::L2Delta` (postcard-дискриминант 6). В hot-пути
//! `scan_tail_for_last_seq` postcard-Err на L2Delta-фрейме → SKIP → `next_seq` может НЕДОСЧИТАТЬСЯ
//! → SEQ REUSE (тихая порча, хуже громкого краха); в offline `read_all` — hard `Err`. Значит
//! silent-откат на pre-M18 бинарь ПРОТИВ ТОГО ЖЕ журнала ЗАПРЕЩЁН (deploy.yml auto-rollback —
//! небезопасен для schema-forward деплоя). Единственная безопасная процедура (docs/fa/ops.md §5.1):
//! stop recorder → quarantine post-M18 сегмент → pre-M18 бинарь. Это ВЫПОЛНИМО только если L2Delta
//! НИКОГДА не дописывается в pre-M18 сегмент.
//!
//! Инвариант держит `decide_open_segment`: writer с ДРУГОЙ `provenance` открывает НОВЫЙ сегмент
//! (M-18 бинарь несёт новый git-sha в provenance → граница автоматическая). Анти-плацебо: если
//! reuse начнёт игнорировать provenance и дописывать L2Delta в чужой сегмент — тест падает, и
//! quarantine-runbook становится невыполним (pre-M18 данные смешаны с variant-6).

use contracts::{DataSource, EventKind, Level, MdPayload, Side, Venue};
use journal::{Journal, WriterConfig};

fn cfg(provenance: &str) -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: provenance.to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: 6_500_000_000_000 + i as i64,
            size: 10_000_000,
            side: Side::Buy,
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

fn l2delta() -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: vec![Level {
                price: 6_500_050_000_000,
                size: 30_000_000,
            }],
            asks: vec![],
            first_update_id: 101,
            final_update_id: 103,
            prev_final_update_id: None,
            ts_exch_ms: 1_752_000_000_499,
        },
    )
}

fn is_l2delta(k: &EventKind) -> bool {
    matches!(k, EventKind::Md(md) if matches!(md.payload, MdPayload::L2Delta { .. }))
}

/// Pre-M18 (provenance P1) пишет только варианты 0..5; M-18 (provenance P2) дописывает L2Delta —
/// ОБЯЗАН в НОВЫЙ сегмент. P1-сегмент несёт P1 и не смешан с variant-6 ⇒ откат = чистый
/// file-move P2-сегмента; seq не переиспользуется через границу.
#[test]
fn l2delta_isolated_in_new_provenance_segment() {
    let dir = tempfile::tempdir().expect("tempdir");
    const P1: &str = "recorder v0.0.0 (git:pre18aa)"; // pre-M18 бинарь
    const P2: &str = "recorder v0.0.0 (git:m18bbbb)"; // M-18 бинарь (новый git-sha)

    // Фаза 1 — pre-M18 бинарь: только Trade (варианты 0..5).
    {
        let mut j = Journal::open_with(dir.path(), cfg(P1)).unwrap();
        for i in 0..5 {
            j.append(trade(i)).unwrap();
        }
        j.flush().unwrap();
    }
    let pre = journal::list_segments(dir.path()).unwrap();
    assert_eq!(pre.len(), 1, "pre-M18: ровно один сегмент");
    let p1_index = pre[0].index;

    // Фаза 2 — M-18 бинарь (другая provenance): дописывает L2Delta + Trade.
    {
        let mut j = Journal::open_with(dir.path(), cfg(P2)).unwrap();
        j.append(l2delta()).unwrap();
        j.append(trade(100)).unwrap();
        j.flush().unwrap();
    }

    let segs = journal::list_segments(dir.path()).unwrap();
    assert!(
        segs.len() >= 2,
        "M-18 provenance ОБЯЗАН открыть НОВЫЙ сегмент (не reuse) — иначе L2Delta смешан с pre-M18, \
         quarantine невозможен; получено сегментов: {}",
        segs.len()
    );

    // Pre-M18 сегмент несёт P1 (откат его НЕ трогает).
    let p1 = segs.iter().find(|s| s.index == p1_index).unwrap();
    assert_eq!(
        p1.header.provenance, P1,
        "pre-M18 сегмент сохранил свою provenance"
    );

    // Post-M18 сегмент идентифицируем по provenance P2 (критерий quarantine) и стоит ПОСЛЕ p1.
    let p2 = segs
        .iter()
        .find(|s| s.header.provenance == P2)
        .expect("post-M18 сегмент обязан существовать и нести provenance M-18 бинаря");
    assert!(
        p2.index > p1_index,
        "post-M18 сегмент идёт после pre-M18 (чистая граница для quarantine)"
    );

    // Полный replay: L2Delta присутствует; seq СТРОГО возрастает (нет reuse через границу).
    let events = journal::read_all(dir.path()).unwrap();
    assert!(
        events.iter().any(|e| is_l2delta(&e.kind)),
        "L2Delta обязан быть в журнале (M-18 писал его)"
    );
    let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    for w in seqs.windows(2) {
        assert!(
            w[1] > w[0],
            "seq строго возрастает — нет reuse через provenance-границу (rollback-safety): {seqs:?}"
        );
    }
}
