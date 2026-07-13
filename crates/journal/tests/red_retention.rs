//! RED M-08 (sacred, architect-only) — ретеншен (E3) и fail-closed по диску (E4).
//!
//! Два способа «остановить сбор данных», которые сегодня ничем не защищены:
//!  1. Диск кончился (2.8 GB/сут, 120 GB свободно → ~43 дня) — recorder умрёт молча.
//!  2. Ретеншен удалил сегмент, который не был выгружен в холодное хранилище — данные
//!     исчезли навсегда (у нас ОДНА копия боевого журнала).
//!
//! Защита от (2) — ТИПОВАЯ, а не дисциплинарная: `prune_segment` требует `ColdCopyProof`,
//! конструктор которого приватен и выдаётся только успешной сверкой копии
//! (тот же приём, что `RiskApproved<Order>` в риск-слое). Компилятор — часть теста:
//!
//! ```compile_fail
//! # use journal::{prune_segment, ColdCopyProof, SegmentInfo};
//! # fn f(seg: &SegmentInfo) {
//! prune_segment(seg, ColdCopyProof { });        // приватный конструктор — НЕ СОБЕРЁТСЯ
//! # }
//! ```

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{Journal, WriterConfig};

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + i as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

fn cfg(min_free_bytes: u64) -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 8 * 1024,
        min_free_bytes,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

/// E4 (FAIL-CLOSED): свободного места меньше порога → запись ОСТАНАВЛИВАЕТСЯ ЯВНО (`Err`).
/// Наивная реализация («пишем, диск сам скажет») здесь проходит `append` и падает на тесте:
/// тихо забить диск и умереть — это тот же остановленный сбор, только без предупреждения
/// и с риском потерять хвост журнала.
#[test]
fn disk_guard_halts_writes_explicitly_when_free_space_is_low() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Порог заведомо выше любого реального свободного места → guard обязан сработать сразу.
    let mut j = Journal::open_with(dir.path(), cfg(u64::MAX)).expect("open_with");
    let res = j.append(trade(0));
    assert!(
        res.is_err(),
        "свободного места < min_free_bytes → append обязан вернуть Err (fail-closed), \
         а не писать молча до отказа диска"
    );

    // Тот же журнал с адекватным порогом пишет нормально (guard не «всегда красный»).
    let mut ok = Journal::open_with(dir.path(), cfg(0)).expect("open_with");
    ok.append(trade(1))
        .expect("при достаточном месте запись обязана идти");
    ok.flush().expect("flush");
}

/// E4: `free_bytes` возвращает реальное свободное место (не заглушку 0/u64::MAX).
#[test]
fn free_bytes_reports_real_filesystem_space() {
    let dir = tempfile::tempdir().expect("tempdir");
    let free = journal::free_bytes(dir.path()).expect("free_bytes");
    assert!(
        free > 1024 * 1024,
        "free_bytes вернул {free} — это заглушка, а не реальное свободное место"
    );
    assert!(free < u64::MAX, "free_bytes вернул u64::MAX — заглушка");
}

/// E3: удалить можно ТОЛЬКО выгруженный и сверенный сегмент. Порча холодной копии →
/// сверка не проходит → proof не выдан → сегмент остаётся на диске.
#[test]
fn prune_requires_verified_cold_copy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cold = tempfile::tempdir().expect("cold");
    {
        let mut j = Journal::open_with(dir.path(), cfg(0)).expect("open_with");
        for i in 0..500 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let segs = journal::list_segments(dir.path()).expect("segments");
    assert!(segs.len() > 1, "нужен закрытый сегмент для выгрузки");
    let victim = segs[0].clone();

    // (1) Честная выгрузка → proof → удаление горячей копии разрешено.
    let proof = journal::verify_cold_copy(&victim, cold.path()).expect("выгрузка и сверка");
    journal::prune_segment(&victim, proof).expect("удаление после proof");
    assert!(
        !victim.path.exists(),
        "горячая копия удалена ПОСЛЕ подтверждённой выгрузки"
    );
    let cold_copy = cold.path().join(victim.path.file_name().expect("name"));
    assert!(cold_copy.exists(), "холодная копия обязана существовать");

    // (2) Порченая холодная копия → сверка ОБЯЗАНА провалиться, proof не выдаётся.
    let victim2 = journal::list_segments(dir.path()).expect("segments")[0].clone();
    let bad_cold = tempfile::tempdir().expect("bad cold");
    std::fs::write(
        bad_cold
            .path()
            .join(victim2.path.file_name().expect("name")),
        b"corrupted",
    )
    .expect("подложить битую копию");
    // Реализация обязана СВЕРИТЬ содержимое, а не поверить в существование файла.
    let res = journal::verify_cold_copy(&victim2, bad_cold.path());
    match res {
        Err(_) => {}
        Ok(_) => panic!(
            "сверка приняла БИТУЮ холодную копию — ретеншен удалит данные, которых в \
             холодном хранилище фактически нет (у нас одна копия боевого журнала)"
        ),
    }
    assert!(
        victim2.path.exists(),
        "сегмент без валидного proof обязан остаться на диске"
    );
}
