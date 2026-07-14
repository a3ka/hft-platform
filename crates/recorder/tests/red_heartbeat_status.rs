//! SACRED (architect-only) — TD-019: heartbeat обязан нести СОСТОЯНИЕ, а не только время.
//!
//! Находка §8 (reviewer): heartbeat-файл = 13 байт таймстампа. E4 обещал наблюдаемость
//! `storage_status` «без ssh» — её нет. Safety цел (запись при нехватке места падает громко),
//! дыра именно в НАБЛЮДАЕМОСТИ: «жив» мы видим, «пишет ли и есть ли место» — нет.
//! Это ровно тот класс, из-за которого пять инцидентов подряд ловились глазами (TD-011/013/
//! 014/016 + C1): healthcheck отвечает «процесс жив», а не «процесс делает то, что должен».
//!
//! Контракт: heartbeat — JSON-объект с полями наблюдаемости. Читается `cat`'ом и внешним
//! мониторингом; в журнал НЕ пишется (детерминизм — `OPS-I-6`).

use contracts::{EventKind, SysEvent};
use journal::Journal;
use recorder::run_writer;

#[tokio::test]
async fn td019_heartbeat_carries_storage_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let journal = Journal::open(dir.path()).expect("journal");
    let hb = dir.path().join("recorder.heartbeat");

    let (tx, rx) = tokio::sync::mpsc::channel::<EventKind>(100);
    for _ in 0..50 {
        tx.send(EventKind::Sys(SysEvent::Heartbeat))
            .await
            .expect("send");
    }
    let (sd_tx, sd_rx) = tokio::sync::oneshot::channel::<()>();
    sd_tx.send(()).expect("shutdown");

    run_writer(rx, journal, hb.clone(), async move {
        let _ = sd_rx.await;
    })
    .await
    .expect("run_writer");

    let body = std::fs::read_to_string(&hb).expect("heartbeat записан");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_else(|e| {
        panic!(
            "heartbeat обязан быть JSON с состоянием, а не голым таймстампом ({e}); \
             содержимое: {body:?}. Без этого деградация видна ТОЛЬКО через ssh — ровно то, \
             из-за чего пять инцидентов подряд ловились глазами"
        )
    });

    for field in [
        "ts_wall_ms",     // жив
        "next_seq",       // ПИШЕТ (а не просто жив — урок TD-011)
        "segment_index",  // ротация работает
        "free_bytes",     // сколько места осталось
        "min_free_bytes", // порог disk-guard
        "writable",       // запись разрешена (E4 fail-closed наблюдаем)
    ] {
        assert!(
            v.get(field).is_some(),
            "в heartbeat нет поля `{field}` — наблюдаемость обещана E4/TD-019 и не выполнена"
        );
    }
    assert!(
        v["next_seq"].as_u64().expect("next_seq число") >= 50,
        "next_seq в heartbeat обязан отражать РЕАЛЬНЫЙ прогресс записи"
    );
    assert_eq!(
        v["writable"].as_bool(),
        Some(true),
        "при свободном диске writable обязан быть true"
    );
}
