//! SACRED (architect-only) — ПРОД-МИГРАЦИЯ M-08: что произойдёт на VPS в момент деплоя.
//!
//! На проде в каталоге журнала лежит `segment-00000000.jrnl` — **8.3 GB, без магии, без
//! заголовка, без манифеста, ЕДИНСТВЕННАЯ копия боевых данных**. Новый recorder стартует
//! `Journal::open_with(...)` ровно на этом каталоге. Этот тест фиксирует то, что до сих пор
//! не проверял ни один гейт (а проверялось руками — так и рождаются TD-011):
//!
//!  1. recorder **СТАРТУЕТ** (не `Err(ForeignSegment)`) — иначе деплой = остановка сбора;
//!  2. он пишет в **НОВЫЙ** сегмент, а legacy-файл остаётся **байт-в-байт нетронутым**;
//!  3. `seq` **продолжается** со старого журнала (ни reuse, ни отката назад);
//!  4. `stream()` на таком каталоге **ОТКАЗЫВАЕТСЯ** отдавать данные, пока legacy не
//!     задекларирован (fail-closed provenance, CT-RFC-02 rev 2);
//!  5. после `declare_legacy` `stream()` отдаёт СТАРЫЕ + НОВЫЕ события в порядке `seq`.
//!
//! Пункты 1-3 — это runbook деплоя, выраженный кодом: запись НИКОГДА не должна зависеть от
//! того, успел ли оператор выполнить декларацию. Пункты 4-5 — что до декларации данные не
//! утекают в research без эпохи.

use contracts::{DataSource, EventKind, LegacySegmentDecl, MdPayload, Side, SysEvent, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

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

/// Каталог «как на VPS»: legacy-сегмент старого формата, записанный СТАРЫМ кодом.
fn prod_like_dir() -> (tempfile::TempDir, u64, Vec<u8>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open(dir.path()).expect("legacy open");
    for i in 0..200 {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    let next = j.next_seq();
    drop(j);
    let bytes = std::fs::read(dir.path().join("segment-00000000.jrnl")).expect("read legacy");
    (dir, next, bytes)
}

/// (1)(2)(3) Деплой на прод: recorder стартует, пишет в НОВЫЙ сегмент, старый не трогает,
/// seq продолжается.
#[test]
fn prod_migration_recorder_starts_and_never_touches_legacy_bytes() {
    let (dir, legacy_next_seq, legacy_bytes) = prod_like_dir();

    let cfg = WriterConfig {
        // TD-025: disk-guard (min_free_bytes) проверяется ОТДЕЛЬНО в red_retention. Этот
        // тест — про КОРРЕКТНОСТЬ миграции (новый сегмент, legacy цел, seq), и его исход
        // НЕ СМЕЕТ зависеть от свободного места хоста (иначе StorageGuard на full-disk
        // чекауте валит tester — тест меряет окружение, а не миграцию; класс TD-023).
        min_free_bytes: 0,
        ..WriterConfig::own_capture("recorder v0 (git:test)", "own-2026-07")
    };
    let mut j = Journal::open_with(dir.path(), cfg).expect(
        "recorder ОБЯЗАН стартовать на каталоге с НЕзадекларированным legacy-сегментом: \
         иначе деплой M-08 = остановка сбора данных на проде",
    );

    assert_eq!(
        j.next_seq(),
        legacy_next_seq,
        "seq обязан ПРОДОЛЖИТЬСЯ со старого журнала (ни reuse, ни откат — тотальный порядок \
         один на журнал)"
    );

    j.append(EventKind::Sys(SysEvent::Heartbeat))
        .expect("write");
    j.flush().expect("flush");
    drop(j);

    // Старый сегмент — байт-в-байт тот же (единственная копия 8.3 GB боевых данных).
    let after = std::fs::read(dir.path().join("segment-00000000.jrnl")).expect("read legacy");
    assert_eq!(
        after, legacy_bytes,
        "legacy-сегмент ИЗМЕНЁН при старте нового recorder'а — это потеря/порча единственной \
         копии боевых данных; дописывать в безголовый сегмент запрещено"
    );

    // Новый сегмент существует и несёт заголовок.
    let new_seg = dir.path().join("segment-00000001.jrnl");
    assert!(
        new_seg.exists(),
        "новый recorder обязан открыть НОВЫЙ сегмент, а не дописывать в legacy"
    );
    let head = std::fs::read(&new_seg).expect("read new");
    assert!(
        head.starts_with(&contracts::SEGMENT_MAGIC),
        "новый сегмент обязан начинаться с магии + заголовка (CT-I-6)"
    );
}

/// (4)(5) Провенанс: до декларации research НЕ получает данные; после — получает всё,
/// в порядке seq, с названной эпохой.
#[test]
fn prod_migration_stream_blocked_until_legacy_declared() {
    let (dir, _, _) = prod_like_dir();
    {
        let cfg = WriterConfig {
            // TD-025: disk-guard (min_free_bytes) проверяется ОТДЕЛЬНО в red_retention. Этот
            // тест — про КОРРЕКТНОСТЬ миграции (новый сегмент, legacy цел, seq), и его исход
            // НЕ СМЕЕТ зависеть от свободного места хоста (иначе StorageGuard на full-disk
            // чекауте валит tester — тест меряет окружение, а не миграцию; класс TD-023).
            min_free_bytes: 0,
            ..WriterConfig::own_capture("recorder v0 (git:test)", "own-2026-07")
        };
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for i in 0..50 {
            j.append(trade(1_000 + i)).expect("append");
        }
        j.flush().expect("flush");
    }

    // (4) Пока legacy не задекларирован — чтение ОТКАЗЫВАЕТ (fail-closed provenance).
    let blocked = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .err()
        .map(|e| journal::is_foreign_segment(&e))
        .unwrap_or(false);
    assert!(
        blocked,
        "stream отдал данные при НЕзадекларированном legacy-сегменте — эпоха приписана молча \
         (ровно то, что CT-RFC-02 обязан исключать)"
    );

    // Операторская процедура деплоя: декларируем боевой сегмент.
    let path = dir.path().join("segment-00000000.jrnl");
    let decl = LegacySegmentDecl {
        file_name: "segment-00000000.jrnl".to_string(),
        fingerprint_sha256: journal::fingerprint(&path).expect("fingerprint"),
        size_bytes_at_decl: std::fs::metadata(&path).expect("meta").len(),
        source: DataSource::OwnCapture,
        provenance: "pre-RFC02 recorder capture, VPS 167.233.192.131, 2026-07-10..2026-07-13"
            .to_string(),
        epoch_id: contracts::LEGACY_EPOCH_ID.to_string(),
    };
    journal::declare_legacy(dir.path(), decl).expect("declare_legacy");

    // (5) Теперь видно ВСЁ: старые + новые события, порядок seq, эпохи названы.
    let s = journal::stream(dir.path(), EpochFilter::All).expect("stream after declare");
    let epochs: Vec<String> = s.headers().iter().map(|h| h.epoch_id.clone()).collect();
    assert!(
        epochs.contains(&contracts::LEGACY_EPOCH_ID.to_string())
            && epochs.contains(&"own-2026-07".to_string()),
        "обе эпохи (legacy + новая) обязаны быть названы в заголовках: {epochs:?}"
    );

    let evs: Vec<_> = journal::stream(dir.path(), EpochFilter::All)
        .expect("stream")
        .map(|e| e.expect("event"))
        .collect();
    assert_eq!(evs.len(), 250, "200 legacy + 50 новых");
    for w in evs.windows(2) {
        assert!(
            w[1].seq > w[0].seq,
            "события обязаны идти в порядке seq через границу legacy → новый сегмент"
        );
    }
}
