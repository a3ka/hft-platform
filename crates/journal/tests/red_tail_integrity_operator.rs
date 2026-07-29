//! SACRED (architect-only) — M-49 / **операторский путь при нечитаемом хвосте**.
//!
//! `red_tail_integrity.rs` требует fail-closed: нечитаемый хвост ⇒ `open_with` = `Err`,
//! recorder не стартует. Без выхода это означало бы **вечно остановленный сбор данных**:
//! оператор упёрся в отказ и может только удалить каталог (то есть потерять историю —
//! ровно то, от чего защищаемся). Поэтому fail-closed обязан иметь ЯВНЫЙ операторский
//! выход — по образцу уже работающих у нас escape-hatch'ей:
//! `allow_prune_without_checkpoint` (M-38b) и `journal.legacy.json` (`declare_legacy`).
//!
//! ## Контракт (architect)
//!
//! **Файловая декларация, а не новый Rust-API.** Оператор кладёт в каталог журнала
//! `journal.force-next-seq.json`:
//! ```json
//! { "next_seq": 512, "reason": "segment-00000001.jrnl.zst невосстановим: bit-rot,
//!   холодная копия отсутствует", "declared_at_ms": 1785362203969 }
//! ```
//! Выбор формы намеренный: (а) не расширяет публичный API крейта; (б) повторяет уже
//! существующий манифест `journal.legacy.json`, то есть оператору знаком формат;
//! (в) остаётся в каталоге как след для аудита до момента применения.
//!
//! Правила (все проверяются оракулами ниже):
//! 1. **Декларация действует ТОЛЬКО при нечитаемом хвосте.** Если хвост читается, она не
//!    имеет силы — иначе забытый файл молча переопределял бы честный `next_seq`.
//! 2. **`next_seq` обязан быть СТРОГО БОЛЬШЕ** максимального `seq`, который удалось
//!    прочитать из ЧИТАЕМЫХ сегментов. Иначе escape-hatch сам становится каналом
//!    seq-reuse — то есть дырой в защите, которую он обслуживает.
//! 3. **Одноразовость.** После успешного применения декларация помечается применённой
//!    (переименование в `journal.force-next-seq.applied.json`) — забыть её в каталоге
//!    невозможно, повторный старт не подхватит её снова.
//! 4. **Аудит.** Факт применения виден: помеченный файл сохраняет `reason` и время.
//!
//! Это тот же принцип, что `pruned_without_checkpoint_coverage` в M-38b: обход разрешён,
//! но обязан быть НАЗВАН и не может быть тихим.

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

const T0: i64 = 1_752_000_000_000;
const N: u64 = 400;
const DECL: &str = "journal.force-next-seq.json";
const DECL_APPLIED: &str = "journal.force-next-seq.applied.json";

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 8 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "tail-integrity operator fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + i as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: T0 + i as i64,
        },
    )
}

fn ls(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

fn readable_max_seq(dir: &std::path::Path) -> Option<u64> {
    journal::stream(dir, EpochFilter::All)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.seq)
        .max()
}

/// Восстановленный из холодного хранилища каталог (только `.zst`, без `journal.meta`)
/// с УСЕЧЁННЫМ сегментом максимального индекса — то есть в состоянии, когда по
/// `red_tail_integrity.rs` старт обязан быть отказан.
fn restored_with_unreadable_tail() -> (tempfile::TempDir, u64) {
    let src = tempfile::tempdir().expect("src");
    {
        let mut j = Journal::open_with(src.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(src.path(), 0, 3).expect("compact");

    let dst = tempfile::tempdir().expect("dst");
    for n in ls(src.path()) {
        if n.ends_with(".zst") {
            std::fs::copy(src.path().join(&n), dst.path().join(&n)).expect("copy");
        }
    }
    let victim = ls(dst.path())
        .into_iter()
        .rfind(|n| n.ends_with(".zst"))
        .expect("есть .zst");
    let p = dst.path().join(&victim);
    let bytes = std::fs::read(&p).expect("read");
    std::fs::write(&p, &bytes[..bytes.len() * 2 / 3]).expect("truncate");

    // Максимум по ЧИТАЕМЫМ сегментам (усечённый не читается).
    let readable = readable_max_seq(dst.path()).unwrap_or(0);
    (dst, readable)
}

fn write_decl(dir: &std::path::Path, next_seq: u64, reason: &str) {
    let json = format!(
        r#"{{"next_seq": {next_seq}, "reason": "{reason}", "declared_at_ms": 1785362203969}}"#
    );
    std::fs::write(dir.join(DECL), json).expect("write decl");
}

// ═════════════════════════════════════════════════════════════════════════════════════
// OP-1 — валидная декларация РАЗБЛОКИРУЕТ старт (иначе сбор данных стоит навсегда)
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn op_1_valid_declaration_unblocks_start_and_is_marked_applied() {
    let (dir, readable_max) = restored_with_unreadable_tail();

    // Предусловие: без декларации старт ОБЯЗАН быть отказан (контракт red_tail_integrity).
    assert!(
        Journal::open_with(dir.path(), cfg()).is_err(),
        "предусловие OP-1 не выполнено: нечитаемый хвост обязан давать Err (JR-I-8). \
         Если здесь Ok — сначала должен быть реализован red_tail_integrity.rs"
    );

    let declared = readable_max + 1_000; // заведомо выше всего читаемого
    write_decl(
        dir.path(),
        declared,
        "segment невосстановим: bit-rot, холодной копии нет",
    );

    let mut j = Journal::open_with(dir.path(), cfg()).unwrap_or_else(|e| {
        panic!(
            "валидная операторская декларация обязана РАЗБЛОКИРОВАТЬ старт.\n\
             ДОЛЖНО БЫТЬ: open_with = Ok при наличии {DECL} с next_seq={declared} \
             (> читаемого максимума {readable_max})\nПОЛУЧЕНО: Err: {e}\n\
             Без выхода fail-closed означает вечно остановленный сбор данных, и единственный \
             доступный оператору шаг — удалить каталог, то есть потерять историю."
        )
    });
    j.append(trade(9_999)).expect("append");
    j.flush().expect("flush");
    drop(j);

    // Новый seq обязан продолжать объявленную позицию, а не занятый диапазон.
    let after = readable_max_seq(dir.path()).expect("что-то читается");
    assert!(
        after >= declared,
        "запись обязана идти с объявленного next_seq={declared}, получено max_seq={after}"
    );

    // Одноразовость + аудит: декларация помечена применённой, reason сохранён.
    let files = ls(dir.path());
    assert!(
        !files.iter().any(|n| n == DECL),
        "декларация обязана быть помечена применённой (одноразовость): {DECL} всё ещё \
         лежит в каталоге и будет подхвачена при следующем старте.\nКаталог: {files:?}"
    );
    let applied = files.iter().find(|n| n.as_str() == DECL_APPLIED);
    assert!(
        applied.is_some(),
        "факт применения обязан остаться СЛЕДОМ для аудита ({DECL_APPLIED}).\n\
         Каталог: {files:?}\nОбход разрешён, но не может быть тихим (принцип M-38b: \
         pruned_without_checkpoint_coverage называет каждый обход поимённо)."
    );
    let body = std::fs::read_to_string(dir.path().join(DECL_APPLIED)).expect("read applied");
    assert!(
        body.contains("bit-rot"),
        "помеченная декларация обязана сохранять reason оператора (аудит): «{body}»"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// OP-2 — декларация НЕ МОЖЕТ стать каналом seq-reuse
// ═════════════════════════════════════════════════════════════════════════════════════

/// Escape-hatch, позволяющий объявить `next_seq` НИЖЕ уже записанного, — это дыра в той
/// самой защите, которую он обслуживает (оператор ошибётся под давлением инцидента).
/// Требование: `next_seq` строго больше максимального ЧИТАЕМОГО `seq`, иначе отказ.
#[test]
fn op_2_declaration_below_readable_max_is_rejected() {
    let (dir, readable_max) = restored_with_unreadable_tail();
    assert!(
        readable_max > 0,
        "фикстура: часть истории обязана читаться (иначе нечего сравнивать)"
    );

    // Оператор ошибся: объявил позицию ВНУТРИ уже записанного диапазона.
    write_decl(
        dir.path(),
        readable_max,
        "ошибка оператора: seq внутри истории",
    );

    let err = Journal::open_with(dir.path(), cfg())
        .err()
        .unwrap_or_else(|| {
            panic!(
                "декларация next_seq={readable_max} ≤ читаемого максимума {readable_max} обязана \
             быть ОТВЕРГНУТА: иначе escape-hatch сам переиспользует seq и порча уходит в \
             журнал с формальным одобрением оператора."
            )
        });
    let msg = err.to_string();
    assert!(
        msg.contains("next_seq") || msg.to_lowercase().contains("seq"),
        "отказ обязан объяснить, что именно неверно в декларации: «{msg}»"
    );

    // И декларация НЕ помечается применённой — она не применялась.
    assert!(
        !ls(dir.path()).iter().any(|n| n == DECL_APPLIED),
        "отвергнутая декларация не должна помечаться применённой"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// OP-3 — при ЧИТАЕМОМ хвосте декларация не имеет силы (забытый файл не опасен)
// ═════════════════════════════════════════════════════════════════════════════════════

/// Парный vantage к OP-1. Если декларация действует всегда, то забытый в каталоге файл
/// молча переопределит честный `next_seq` — и мы получим дыру там, где всё было исправно.
#[test]
fn op_3_declaration_is_inert_when_tail_is_readable() {
    // Здоровый каталог: сжатая история БЕЗ порчи.
    let src = tempfile::tempdir().expect("src");
    {
        let mut j = Journal::open_with(src.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(src.path(), 0, 3).expect("compact");
    let dir = tempfile::tempdir().expect("dst");
    for n in ls(src.path()) {
        if n.ends_with(".zst") {
            std::fs::copy(src.path().join(&n), dir.path().join(&n)).expect("copy");
        }
    }
    let history_max = readable_max_seq(dir.path()).expect("история читается");

    // Забытая декларация с абсурдно большим значением.
    write_decl(
        dir.path(),
        history_max + 1_000_000,
        "забытый файл прошлого инцидента",
    );

    let mut j = Journal::open_with(dir.path(), cfg())
        .expect("здоровый каталог обязан стартовать (хвост читается)");
    j.append(trade(9_999)).expect("append");
    j.flush().expect("flush");
    drop(j);

    let after = readable_max_seq(dir.path()).expect("читается");
    assert!(
        after <= history_max + 10,
        "декларация НЕ должна иметь силы при читаемом хвосте: честный next_seq был бы \
         ~{}, а получено {after} — забытый файл переопределил позицию записи и создал \
         разрыв в seq на миллион.",
        history_max + 1
    );
    assert!(
        ls(dir.path()).iter().any(|n| n == DECL),
        "неприменённая декларация должна остаться на месте (её не применяли), чтобы \
         оператор увидел и убрал её сам"
    );
}
