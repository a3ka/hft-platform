//! SACRED (architect-only) — M-50 / **TD-053: событие крупнее 64 KiB невидимо для скана пола**.
//!
//! ## Дефект (R-003 NOTE 2, достижимость доказана замером)
//!
//! `resync_max_seq` (терпимый скан пола для валидации операторской декларации
//! `journal.force-next-seq.json`) трактует фрейм, чей размер превышает
//! `READABLE_SCAN_MAX_CARRY = 64 KiB`, как порчу и ресинкает на 1 байт — при том, что
//! штатный ридер крейта (`read_frame_payload`) принимает фреймы до 64 **MiB**. Событие,
//! которое журнал ЧИТАЕТ, в пол НЕ попадает. Направление ошибки — занижение пола =
//! fail-OPEN: декларация с `next_seq` внутри занятого диапазона проходит валидацию →
//! **seq-reuse**, необратимая порча append-only журнала.
//!
//! Замер (`research/measurements/td-053-event-size.md`): реальный максимум сегодня —
//! 45 113 B (68.8% капа), но архитектурный потолок `L2Snapshot` (bucket-cap venue-binance,
//! 3000 уровней/сторона) — **66 032 B = 100.8% капа**, а `L2Delta` не ограничен вообще и
//! эмитируется в проде с M-18. Дефект достижим без единого изменения кода.
//!
//! ## Контракт JR-I-9 (см. milestones/M-50-floor-scan-large-events.md)
//!
//! Скан пола не имеет права молча исключить из пола валидный фрейм, который штатный ридер
//! принял бы. Для CRC-валидного фрейма с `len` ≤ санити-капа ридера: либо его `seq`
//! участвует в `Known`, либо пол деградирует в `Unknown` (отказ проверять декларацию).
//! «`Known` с заниженным полом» запрещён. Свойства rev5 (терпимость И O(1) памяти)
//! сохраняются; граница памяти распространяется и на размер фрейма.
//!
//! ## Форма фикстур
//!
//! Путь декларации входится ТОЛЬКО при нечитаемом хвостовом окне (4 MiB), поэтому каждая
//! фикстура пути декларации несёт мусорный хвост > `TAIL_SCAN_CHUNK`. Мусор — 0x5A
//! (как `corrupt_tail` всего набора M-49): фейковая длина 0x5A5A5A5A ≈ 1.4 GiB заведомо
//! больше санити-капа, то есть детерминированно мусор для ОБЕИХ реализаций (до/после
//! фикса). Крупному фрейму НЕ предшествует мусор — так его невидимость на текущем коде
//! детерминирована (carry перед ним ≤ капа: доказано разбором фаз роста carry).
//!
//! Эталон везде ИЗМЕРЕН (`common::max_seq_in`, капа не имеет), не выведен арифметически —
//! урок дефекта фикстуры R-002.

mod common;

use common::{
    append_bytes, assert_decl_rejected, cfg_with, event_of_frame_size, frame_of, ls, max_seq_in,
    tolerant_readable_max, trade, write_decl, DECL_APPLIED, FLOOR_SCAN_CARRY_CAP,
    PROD_L2SNAPSHOT_MAX_FRAME, TAIL_SCAN_CHUNK,
};
use journal::{Journal, WriterConfig};

const SEG0: &str = "segment-00000000.jrnl";
const SEG1: &str = "segment-00000001.jrnl";

/// Мусорный хвост строго больше окна хвостового скана — путь декларации обязан входиться.
const GARBAGE: usize = 4 * 1024 * 1024 + 512 * 1024;

const _: () = assert!(
    GARBAGE as u64 > TAIL_SCAN_CHUNK,
    "фикстура: мусор обязан перекрывать окно хвостового скана целиком — иначе в окне \
     найдётся валидный фрейм и путь декларации не входится"
);

fn cfg() -> WriterConfig {
    cfg_with(256 * 1024 * 1024, "floor-scan fixture")
}

/// Каталог формы TD-053: [journal: мелкие trade seq 0..prefix-1] + [ручные ВАЛИДНЫЕ фреймы
/// заданных размеров, seq = prefix, prefix+1, ...] + [мусорный хвост 0x5A > окна скана].
/// `journal.meta` удалена (прод-режим restore: ретеншен выгружает сегменты, не мету).
/// Возвращает (каталог, ИЗМЕРЕННЫЙ читаемый максимум).
fn dir_with_large_tail_events(prefix: u64, frames: &[usize]) -> (tempfile::TempDir, u64) {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..prefix {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let seg = dir.path().join(SEG0);
    for (k, &fsz) in frames.iter().enumerate() {
        let fb = frame_of(&event_of_frame_size(prefix + k as u64, fsz));
        assert_eq!(fb.len(), fsz, "setup-guard: фрейм собран не того размера");
        append_bytes(&seg, &fb);
    }
    append_bytes(&seg, &vec![0x5A_u8; GARBAGE]);
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let measured = tolerant_readable_max(dir.path())
        .expect("setup-guard: у каталога обязан быть читаемый префикс");
    let expect_max = prefix + frames.len() as u64 - 1;
    assert_eq!(
        measured, expect_max,
        "setup-guard: эталон (без капа) обязан ВИДЕТЬ ручные крупные фреймы — иначе \
         фикстура не давит на инвариант и оракул слеп"
    );
    let size = std::fs::metadata(&seg).expect("meta").len();
    assert!(
        size > TAIL_SCAN_CHUNK,
        "setup-guard: файл {size} B обязан быть больше окна скана {TAIL_SCAN_CHUNK} B"
    );
    (dir, measured)
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-1 — ЯДРО: прод-форма 66 032 B (архитектурный потолок L2Snapshot) участвует в поле
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_1_prod_form_large_event_is_part_of_the_floor() {
    let (dir, max) = dir_with_large_tail_events(40, &[PROD_L2SNAPSHOT_MAX_FRAME]);

    // (1) декларация РОВНО на seq крупного события — внутри занятого диапазона.
    assert_decl_rejected(
        dir.path(),
        cfg(),
        max,
        "прод-форма: последнее читаемое событие — L2Snapshot 66 032 B",
    );

    // (2) ПАРНЫЙ vantage: честная декларация обязана РАБОТАТЬ (fail-closed без выхода =
    // вечно остановленный сбор; оператору осталось бы удалить каталог — потерять историю).
    write_decl(
        dir.path(),
        max + 1,
        "хвост невосстановим: холодной копии нет",
    );
    let j = Journal::open_with(dir.path(), cfg()).unwrap_or_else(|e| {
        panic!(
            "честная декларация next_seq={} (строго больше читаемого максимума {max}) \
             обязана разблокировать старт, получено Err: {e}",
            max + 1
        )
    });
    assert_eq!(
        j.next_seq(),
        max + 1,
        "запись обязана идти РОВНО с объявленной позиции"
    );
    drop(j);
    assert!(
        ls(dir.path()).iter().any(|n| n == DECL_APPLIED),
        "применённая декларация обязана быть помечена (одноразовость + аудит)"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-2 — ГРАНИЦА, парный vantage (зелёный ДО фикса): фреймы ≤ капа видимы и остаются
// видимыми — фикс не имеет права купить крупные фреймы ценой обычных
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_2_frames_at_or_below_carry_cap_stay_visible() {
    for fsz in [FLOOR_SCAN_CARRY_CAP - 1, FLOOR_SCAN_CARRY_CAP] {
        let (dir, max) = dir_with_large_tail_events(40, &[fsz]);
        assert_decl_rejected(
            dir.path(),
            cfg(),
            max,
            &format!("фрейм {fsz} B (≤ капа carry) видим уже сегодня"),
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-3 — ГРАНИЦА: кап+1. Минимальный фрейм, который сегодняшний скан молча теряет
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_3_frame_one_byte_over_carry_cap_is_part_of_the_floor() {
    let (dir, max) = dir_with_large_tail_events(40, &[FLOOR_SCAN_CARRY_CAP + 1]);
    assert_decl_rejected(
        dir.path(),
        cfg(),
        max,
        &format!("фрейм {} B (кап carry + 1 байт)", FLOOR_SCAN_CARRY_CAP + 1),
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-4 — МНОЖЕСТВЕННОСТЬ: ПОСЛЕДНИЕ читаемые события ВСЕ крупнее капа (точное условие
// манифестации из замера §4.2) — оба seq обязаны быть в поле
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_4_two_consecutive_large_events_are_both_part_of_the_floor() {
    let (dir, max) =
        dir_with_large_tail_events(40, &[PROD_L2SNAPSHOT_MAX_FRAME, PROD_L2SNAPSHOT_MAX_FRAME]);
    for declared in [max - 1, max] {
        assert_decl_rejected(
            dir.path(),
            cfg(),
            declared,
            "два крупных события подряд в хвосте читаемого префикса",
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-5 — АСИММЕТРИЯ/кросс-сегмент: крупное событие — ЕДИНСТВЕННОЕ читаемое содержимое
// последнего сегмента; откат пола на ПРЕДЫДУЩИЙ сегмент = ровно тот seq-reuse
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_5_floor_does_not_fall_back_past_a_large_only_segment() {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with A");
        for i in 0..30 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    // Второй сегмент строится ВРУЧНУЮ из валидного header'а первого (формат дублируется
    // сознательно — см. док-коммент common/mod.rs): его единственный валидный фрейм —
    // крупное событие, дальше мусор. Писать через writer нельзя: любой мелкий валидный
    // фрейм в segment-1 дал бы полу честный seq и снял давление с инварианта.
    let raw0 = std::fs::read(dir.path().join(SEG0)).expect("read seg0");
    let hdr = common::header_end(&raw0);
    assert!(
        hdr > 0,
        "setup-guard: у сырого сегмента обязан быть валидный header"
    );
    let prev_max = max_seq_in(&raw0).expect("setup-guard: первый сегмент обязан читаться");
    assert_eq!(prev_max, 29, "setup-guard: prefix первого сегмента");

    let seg1 = dir.path().join(SEG1);
    std::fs::write(&seg1, &raw0[..hdr]).expect("write seg1 header");
    append_bytes(
        &seg1,
        &frame_of(&event_of_frame_size(30, PROD_L2SNAPSHOT_MAX_FRAME)),
    );
    append_bytes(&seg1, &vec![0x5A_u8; GARBAGE]);
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let measured = tolerant_readable_max(dir.path()).expect("префикс обязан читаться");
    assert_eq!(
        measured, 30,
        "setup-guard: эталон обязан видеть крупное событие последнего сегмента"
    );

    // next_seq=30 «строго больше 29» — примет ровно та реализация, что откатилась на
    // предыдущий сегмент, потеряв крупное событие. seq 30 ЗАНЯТ.
    assert_decl_rejected(
        dir.path(),
        cfg(),
        30,
        "крупное событие — единственное читаемое в последнем сегменте; откат на \
         предыдущий сегмент занижает пол",
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-6 — ОТСУТСТВИЕ, парный vantage (зелёный ДО фикса): мусор, ПОХОЖИЙ на длину, — не
// фрейм. Правдоподобная фейковая длина не имеет права ни занизить пол молча, ни
// протащить декларацию. Допустимы ОБЕ формы отказа: Known(честный max) ИЛИ Unknown.
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_6_garbage_that_fakes_a_plausible_length_is_not_a_frame() {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..50 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let seg = dir.path().join(SEG0);
    // Мусор с ПРАВДОПОДОБНОЙ длиной (1 MiB ≤ санити-капа: кандидат в «крупный фрейм»,
    // обязан провалить CRC) и с АБСУРДНОЙ (0xFFFFFFFF > санити-капа: мусор немедленно).
    let mut junk = Vec::new();
    junk.extend_from_slice(&(1024u32 * 1024).to_le_bytes());
    junk.extend_from_slice(&[0xA7u8; 300]);
    junk.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    junk.extend_from_slice(&[0xA7u8; 100]);
    append_bytes(&seg, &junk);
    // После мусора — снова ВАЛИДНЫЕ события (дозапись штатным writer'ом).
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("reopen");
        assert_eq!(
            j.next_seq(),
            50,
            "setup-guard: хвост читаем, дозапись с seq=50"
        );
        for i in 0..50 {
            j.append(trade(1000 + i)).expect("append");
        }
        j.flush().expect("flush");
    }
    append_bytes(&seg, &vec![0x5A_u8; GARBAGE]);
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let measured = tolerant_readable_max(dir.path()).expect("префикс обязан читаться");
    assert_eq!(
        measured, 99,
        "setup-guard: события ПОСЛЕ фейковой длины обязаны быть в эталоне"
    );

    // Внутри занятого диапазона — отказ обязателен в ЛЮБОЙ допустимой реализации.
    assert_decl_rejected(
        dir.path(),
        cfg(),
        50,
        "фейковая правдоподобная длина посреди валидных событий",
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-7 — ШТАТНЫЙ ПУТЬ, парный vantage (зелёный ДО фикса): крупное ПОСЛЕДНЕЕ событие не
// ломает перезапуск и строгое чтение — иначе ужесточение остановит сбор данных
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_7_large_last_event_does_not_break_normal_restart_or_reads() {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..30 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    append_bytes(
        &dir.path().join(SEG0),
        &frame_of(&event_of_frame_size(30, PROD_L2SNAPSHOT_MAX_FRAME)),
    );
    // Restore-форма: меты нет — next_seq обязан быть восстановлен из ХВОСТА.
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let mut j = Journal::open_with(dir.path(), cfg()).unwrap_or_else(|e| {
        panic!("здоровый каталог с крупным последним событием обязан стартовать: {e}")
    });
    assert_eq!(
        j.next_seq(),
        31,
        "next_seq обязан продолжиться ПОСЛЕ крупного события — иначе seq-reuse на \
         штатном пути перезапуска"
    );
    j.append(trade(31)).expect("append");
    j.flush().expect("flush");
    drop(j);

    let evs = journal::read_all(dir.path()).expect("строгий путь чтения обязан отдать каталог");
    assert!(
        evs.iter().any(|e| e.seq == 30),
        "строгий ридер обязан видеть крупное событие (санити-кап 64 MiB)"
    );
    assert_eq!(evs.iter().map(|e| e.seq).max(), Some(31));
}

// ═════════════════════════════════════════════════════════════════════════════════════
// FS-10 — ФОРМАТ .zst: крупное событие внутри КОМПАКТИРОВАННОГО сегмента участвует в
// поле (ветка скана через потоковую распаковку — отдельный код-путь от сырого)
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn fs_10_large_event_inside_compacted_segment_is_part_of_the_floor() {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..40 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let seg0 = dir.path().join(SEG0);
    append_bytes(
        &seg0,
        &frame_of(&event_of_frame_size(40, PROD_L2SNAPSHOT_MAX_FRAME)),
    );

    // Компактируем той же формой, что M-08: zstd поверх ПОЛНОЙ v2-структуры сегмента.
    let raw = std::fs::read(&seg0).expect("read");
    assert_eq!(
        max_seq_in(&raw),
        Some(40),
        "setup-guard: сырой сегмент до компакции обязан нести крупное событие"
    );
    let z = zstd::stream::encode_all(&raw[..], 3).expect("zstd encode");
    std::fs::write(dir.path().join("segment-00000000.jrnl.zst"), &z).expect("write zst");
    std::fs::remove_file(&seg0).expect("rm raw");

    // Последний сегмент — сырой, заголовок цел (скопирован из raw до компакции), тело
    // нечитаемо ⇒ путь декларации входится, а пол обязан прийти из .zst-сегмента.
    let hdr = common::header_end(&raw);
    assert!(
        hdr > 0,
        "setup-guard: у сырого сегмента обязан быть валидный header"
    );
    let seg1 = dir.path().join(SEG1);
    std::fs::write(&seg1, &raw[..hdr]).expect("write seg1 header");
    append_bytes(&seg1, &vec![0x5A_u8; 8 * 1024]);
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));

    let measured = tolerant_readable_max(dir.path()).expect("префикс обязан читаться");
    assert_eq!(
        measured, 40,
        "setup-guard: эталон обязан видеть крупное событие внутри .zst"
    );

    assert_decl_rejected(
        dir.path(),
        cfg(),
        40,
        "крупное событие внутри компактированного сегмента (.zst-ветка скана)",
    );
}
