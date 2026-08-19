//! RED M-08 (sacred, architect-only) — ретеншен (E3) и fail-closed по диску (E4).
//!
//! Два способа «остановить сбор данных», которые сегодня ничем не защищены:
//!  1. Диск кончился (2.8 GB/сут, 120 GB свободно → ~43 дня) — recorder умрёт молча.
//!  2. Ретеншен удалил сегмент, который не был выгружен в холодное хранилище — данные
//!     исчезли навсегда (у нас ОДНА копия боевого журнала).
//!
//! Защита от (2) — ТИПОВАЯ, а не дисциплинарная: `prune_segment` требует `ColdCopyProof`,
//! конструктор которого приватен и выдаётся только успешной сверкой копии (тот же приём,
//! что `RiskApproved<Order>` в риск-слое). ИСПОЛНЯЕМЫЙ compile_fail-доктест этого барьера
//! живёт в публичных доках `journal::segments::prune_segment` (N1 из C-005: комментарий в
//! тесте — не гейт).

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

/// E4 (FAIL-CLOSED, усилено по C-005 M4): порог берётся от РЕАЛЬНОГО свободного места
/// (`free_bytes + 1`), а не от синтетического `u64::MAX` — оракул обязан ловить прод-режим
/// «диск подходит к концу», а не только «порог задан абсурдно».
///
/// Проверяется ТРИ вещи (наивная реализация «просто вернуть Err» не проходит):
///  1. `append` возвращает ИМЕННО storage-guard-ошибку (`journal::is_storage_guard`);
///  2. состояние журнала НЕ сдвинулось: `next_seq` тот же, файл не вырос — событие не
///     записано частично (рваный фрейм = порванный реплей);
///  3. `storage_status()` наблюдаем и говорит `writable == false` — деградация видна БЕЗ ssh
///     (recorder публикует это в heartbeat; урок TD-011/TD-016: healthcheck молчит).
#[test]
fn disk_guard_halts_writes_explicitly_when_free_space_is_low() {
    let dir = tempfile::tempdir().expect("tempdir");
    let free = journal::free_bytes(dir.path()).expect("free_bytes");

    // Порог = реальное свободное место + 1 байт → guard обязан сработать (прод-режим).
    let cfg_tight = cfg(free.saturating_add(1));
    let mut j = Journal::open_with(dir.path(), cfg_tight.clone()).expect("open_with");
    let seq_before = j.next_seq();
    let size_before = segment_bytes(dir.path());

    let err = match j.append(trade(0)) {
        Err(e) => e,
        Ok(_) => panic!(
            "свободного места меньше порога, а append записал событие — recorder будет \
             молча писать до отказа диска (сбор остановится без предупреждения)"
        ),
    };
    assert!(
        journal::is_storage_guard(&err),
        "ожидалась storage-guard ошибка, получено: {err}"
    );
    assert_eq!(
        j.next_seq(),
        seq_before,
        "неудачный append НЕ смеет двигать seq (иначе дыра в тотальном порядке)"
    );
    assert_eq!(
        segment_bytes(dir.path()),
        size_before,
        "неудачный append НЕ смеет оставлять байты в сегменте (рваный фрейм ломает реплей)"
    );

    let st = journal::storage_status(dir.path(), &cfg_tight).expect("storage_status");
    assert!(
        !st.writable,
        "storage_status обязан ЯВНО сообщать, что запись остановлена (наблюдаемость без ssh)"
    );

    // Контроль: при адекватном пороге тот же путь пишет — guard не «всегда красный».
    let ok_cfg = cfg(0);
    let mut ok = Journal::open_with(dir.path(), ok_cfg.clone()).expect("open_with");
    ok.append(trade(1))
        .expect("при достаточном месте запись обязана идти");
    ok.flush().expect("flush");
    assert!(
        journal::storage_status(dir.path(), &ok_cfg)
            .expect("status")
            .writable
    );
}

/// Суммарный размер сегментов каталога (для проверки «ни байта не записано»).
fn segment_bytes(dir: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "jrnl") {
                total += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
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
