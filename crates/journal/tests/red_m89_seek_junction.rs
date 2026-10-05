//! RED M-89 (sacred, architect-only) — СТЫК сегментов на пути сдвига: `JR-I-2` судится
//! ПРОТИВ КАТАЛОГА (`research/arbitration/A-042-m89-epoch-boundary.md` §1 п. 1, §4, §5).
//!
//! ## Предмет
//!
//! Путь `stream_from_at(dir, filter, Some(after), None)`. Обязательство читателя — не
//! потерять молча ни одного события, которое ПОЛНЫЙ каталог (состав ДО фильтра) обещает его
//! проекции. Видимый разрыв `seq` между двумя выданными событиями законен тогда и только
//! тогда, когда все `seq` разрыва лежат в сегментах, которые фильтр ИСКЛЮЧИЛ, — и это
//! проверяется по шапкам, не чтением исключённых сегментов (спека §5.2 п. 9):
//!
//! - (в) правый край каждого ПРОЧИТАННОГО сегмента `S`: `last_scanned(S) + 1 == first_seq`
//!   ФИЗИЧЕСКОГО преемника `S` в полном каталоге, принят тот фильтром или нет (`n1`, `n3`,
//!   `n4`);
//! - (г) левый край каждого сегмента, открытого с `header_end`: первое декодированное
//!   событие `== header.first_seq` (`n2`);
//! - ожидание при переходе на следующий ПРИНЯТЫЙ сегмент сбрасывается на его
//!   `header.first_seq`, а не переносится через стык — перенос отвергает законную проекцию
//!   (`n5`).
//!
//! ## Живые инварианты (`docs/fa/journal.md`)
//!
//! `JR-I-2` («`seq` строго монотонен без дыр; разрыв при чтении → abort (не „пропустить“)») —
//! здесь в форме «против каталога». `JR-I-11` (монотонность сшивки) по-прежнему держит
//! ПОРЯДОК стыка; (в)/(г) — дополнение по `seq`, не замена.
//!
//! ## Эталон — явное ожидание по каталогу, НЕ `stream()`
//!
//! `stream()`/`stream_from` дыры в каталоге не проверяют (`A-042` §2.2 P3, спека §11 п. 12) —
//! сравнивать с ними значило бы требовать того же отклонения. Ожидание каждого сценария
//! выведено из ФОРМЫ КАТАЛОГА, снятой независимым разбором файлов (шапки — `list_segments`,
//! кадры — собственный парсер `[u32 LE len][payload][u32 LE crc32]`, как в `common`).
//!
//! ## Setup-стражи (`testing.md` «Целостность гейта» п. 3)
//!
//! Каждый сценарий сначала доказывает, что каталог имеет ЗАЯВЛЕННУЮ форму — по шапкам и по
//! кадрам, — и только затем зовёт предмет. Строка, вносящая дефект, помечена `// DEFECT:nK`:
//! её нейтрализация обязана давать `SETUP НЕ СОСТОЯЛСЯ`, а не ложную зелень — это
//! предъявляется мутационным контролем architect'а (доступен без реализации).
//!
//! ## Состояние на ревизии набора
//!
//! `n1`, `n2`, `n3`, `n4` — RUNTIME-RED поведенчески: сегодняшний читатель проходит дыру
//! МОЛЧА («`Ok` через дыру вместо `Err`»); compile-RED здесь нет намеренно — `seek_fallbacks`
//! не читается (метода нет на ревизии набора; ассерт `== 0` для `n1`/`n5` — мутационный, в
//! Done Block dev'а, `A-042` §5). `n5` — ПОЗИТИВНЫЙ КОНТРОЛЬ (класс `mn_10`): зелен сегодня,
//! его ценность предъявляется мутацией «ожидание переносится через стык → `n5` FAILED» в
//! Done Block dev'а.
//!
//! Мутанты реализации, которых убивает набор (спека §14): «(в) удалена» → `n1`/`n3`;
//! «(г) удалена» → `n2`; «(в) только при принятом преемнике» → `n4`; «перенос ожидания через
//! стык» / «сравнение с `first_seq` следующего ПРИНЯТОГО вместо физического» → `n5`.

mod common;

use std::io;
use std::path::{Path, PathBuf};

use common::{cfg_with, header_end, tolerant_bytes, trade};
use contracts::{DataSource, Event};
use journal::{EpochFilter, Journal, WriterConfig};

/// Ротация ≈ 80 событий на сегмент (`A-042` §2.2 P3: 0..86 | 87..171 | 172..255 | 256..).
const SEG_4K: u64 = 4096;
/// Без ротации: сегмент закрывается только сменой шапки (эпохи/источника).
const BIG_SEG: u64 = 1 << 30;

const EPOCH_A: &str = "own-A";
const EPOCH_B: &str = "own-B";
const EPOCH_VENDOR: &str = "vendor-X";

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о стыке: фикстура не воспроизвела \
         форму каталога, ради которой оракул написан."
    )
}

fn cfg(source: DataSource, epoch: &str, seg: u64) -> WriterConfig {
    WriterConfig {
        source,
        epoch_id: epoch.to_string(),
        ..cfg_with(seg, "M-89 seek junction")
    }
}

/// Дописать `n` событий писателем с данной шапкой; `seq` назначает журнал (глобальная
/// нумерация сквозь эпохи — `A-042` §2.1). Писатель закрывается по выходу.
fn write(dir: &Path, cfg: WriterConfig, n: u64) {
    let mut j =
        Journal::open_with(dir, cfg).unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
    for _ in 0..n {
        let seq = j.next_seq();
        j.append(trade(seq))
            .unwrap_or_else(|e| setup_failed(&format!("append #{seq}: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
}

/// Снимок сегмента каталога, снятый ПО ШАПКЕ (`list_segments`), без чтения тела.
#[derive(Clone, Debug)]
struct Seg {
    index: u32,
    first_seq: u64,
    source: DataSource,
    epoch: String,
    zst: bool,
    path: PathBuf,
}

fn catalog(dir: &Path) -> Vec<Seg> {
    let mut v: Vec<Seg> = journal::list_segments(dir)
        .unwrap_or_else(|e| setup_failed(&format!("list_segments: {e}")))
        .into_iter()
        .map(|s| Seg {
            index: s.index,
            first_seq: s.header.first_seq,
            source: s.header.source,
            epoch: s.header.epoch_id.clone(),
            zst: s.path.to_string_lossy().ends_with(".jrnl.zst"),
            path: s.path,
        })
        .collect();
    v.sort_by_key(|s| s.index);
    v
}

fn firsts(cat: &[Seg]) -> Vec<u64> {
    cat.iter().map(|s| s.first_seq).collect()
}

/// СТРОГИЙ независимый разбор кадров тела сегмента (после header-кадра): `(смещение, длина
/// кадра, seq)` до первого невалидного кадра. Ресинка нет намеренно — оракул стыка судит
/// ФОРМУ файла, а не терпимость читателя.
fn frames_in(data: &[u8]) -> Vec<(usize, usize, u64)> {
    let mut i = header_end(data);
    let mut out = Vec::new();
    while i + 8 <= data.len() {
        let len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        let Some(end) = i.checked_add(4 + len + 4) else {
            break;
        };
        if end > data.len() {
            break;
        }
        let payload = &data[i + 4..i + 4 + len];
        let crc = u32::from_le_bytes(data[end - 4..end].try_into().unwrap());
        if crc32fast::hash(payload) != crc {
            break;
        }
        let Ok(ev) = postcard::from_bytes::<Event>(payload) else {
            break;
        };
        out.push((i, end - i, ev.seq));
        i = end;
    }
    out
}

/// Кадры сегмента, сырого или `.zst` (терпимая распаковка — `common::tolerant_bytes`).
fn frames_of(seg: &Seg) -> Vec<(usize, usize, u64)> {
    frames_in(&tolerant_bytes(&seg.path))
}

fn first_seq_in_body(seg: &Seg) -> Option<u64> {
    frames_of(seg).first().map(|f| f.2)
}

fn last_seq_in_body(seg: &Seg) -> Option<u64> {
    frames_of(seg).last().map(|f| f.2)
}

/// Страж «честного каталога»: тело каждого сегмента начинается со своего `first_seq`, а его
/// последний кадр + 1 равен `first_seq` физического преемника. До внесения дефекта каталог
/// обязан быть ровно таким — иначе дефект, который сценарий вносит, не единственный.
fn assert_honest_catalog(tag: &str, cat: &[Seg]) {
    for (k, s) in cat.iter().enumerate() {
        let body_first = first_seq_in_body(s)
            .unwrap_or_else(|| setup_failed(&format!("{tag}: сегмент {} пуст", s.index)));
        if body_first != s.first_seq {
            setup_failed(&format!(
                "{tag}: сегмент {} — тело начинается с {body_first}, шапка обещает {}",
                s.index, s.first_seq
            ));
        }
        if let Some(next) = cat.get(k + 1) {
            let body_last = last_seq_in_body(s).unwrap();
            if body_last + 1 != next.first_seq {
                setup_failed(&format!(
                    "{tag}: правый край сегмента {} = {body_last}, преемник {} начинается с {}",
                    s.index, next.index, next.first_seq
                ));
            }
        }
    }
}

/// Прогнать ПРЕДМЕТ до конца или до первой ошибки: (seq выданных, класс ошибки).
fn subject(dir: &Path, filter: EpochFilter, after: u64) -> (Vec<u64>, Option<io::ErrorKind>) {
    let s = journal::stream_from_at(dir, filter, Some(after), None)
        .unwrap_or_else(|e| setup_failed(&format!("stream_from_at: {e}")));
    let mut out = Vec::new();
    let mut err = None;
    for item in s {
        match item {
            Ok(ev) => out.push(ev.seq),
            Err(e) => {
                err = Some(e.kind());
                break;
            }
        }
    }
    (out, err)
}

fn range(from: u64, to_inclusive: u64) -> Vec<u64> {
    (from..=to_inclusive).collect()
}

// ─────────── дефекты каталога (каждый — ОДНА строка вызова в сценарии, помечена DEFECT) ───────────

fn remove_segment(seg: &Seg) {
    std::fs::remove_file(&seg.path)
        .unwrap_or_else(|e| setup_failed(&format!("remove {}: {e}", seg.path.display())));
}

/// Вырезать ПЕРВЫЙ кадр тела из сырого сегмента; шапка (и её `first_seq`) не тронута.
fn cut_first_frame(seg: &Seg) {
    let bytes = std::fs::read(&seg.path).expect("read");
    let (off, len, _) = frames_in(&bytes)
        .first()
        .copied()
        .unwrap_or_else(|| setup_failed("cut_first_frame: тело пусто"));
    let mut out = Vec::with_capacity(bytes.len() - len);
    out.extend_from_slice(&bytes[..off]);
    out.extend_from_slice(&bytes[off + len..]);
    std::fs::write(&seg.path, &out).expect("write");
}

/// Усечь сырой сегмент на ПОСЛЕДНИЙ кадр (`set_len` на его начало): EOF чистый, файл
/// заканчивается предпоследним событием.
fn truncate_last_frame(seg: &Seg) {
    let bytes = std::fs::read(&seg.path).expect("read");
    let (off, _, _) = frames_in(&bytes)
        .last()
        .copied()
        .unwrap_or_else(|| setup_failed("truncate_last_frame: тело пусто"));
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(&seg.path)
        .expect("open rw");
    f.set_len(off as u64).expect("set_len");
}

/// Каталог одной эпохи из ≥ 4 сегментов по 4 КиБ (форма P3 `A-042` §2.2). Возвращает
/// (dir, каталог до дефекта, last_seq сегмента 0).
fn four_segments_one_epoch(tag: &str) -> (tempfile::TempDir, Vec<Seg>, u64) {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        cfg(DataSource::OwnCapture, EPOCH_A, SEG_4K),
        300,
    );
    let cat = catalog(dir.path());
    if cat.len() < 4 {
        setup_failed(&format!(
            "{tag}: ротация по 4 КиБ дала {} сегментов, нужно ≥ 4 (индексы {:?})",
            cat.len(),
            firsts(&cat)
        ));
    }
    if cat.iter().any(|s| s.zst) {
        setup_failed(&format!("{tag}: в свежем каталоге есть .zst"));
    }
    assert_honest_catalog(tag, &cat);
    let last0 = last_seq_in_body(&cat[0]).unwrap();
    if last0 < 8 {
        setup_failed(&format!(
            "{tag}: сегмент 0 слишком мал (last={last0}), курсор «в середине» не поставить"
        ));
    }
    (dir, cat, last0)
}

/// Страж «удалённый средний сегмент»: в каталоге нет сегмента с `first_seq` жертвы, а
/// преемник сегмента 0 начинается НЕ с `last0 + 1` — дыра реальна и ничем не объяснена.
fn assert_middle_removed(tag: &str, dir: &Path, victim: &Seg, last0: u64) -> Vec<Seg> {
    let cat = catalog(dir);
    if cat.iter().any(|s| s.index == victim.index) {
        setup_failed(&format!(
            "{tag}: сегмент {} всё ещё в каталоге — дыра не построена",
            victim.index
        ));
    }
    let succ = cat
        .get(1)
        .unwrap_or_else(|| setup_failed(&format!("{tag}: у сегмента 0 нет преемника")));
    if succ.first_seq == last0 + 1 {
        setup_failed(&format!(
            "{tag}: преемник сегмента 0 начинается с {} == last0+1 — дыры в каталоге нет",
            succ.first_seq
        ));
    }
    cat
}

fn assert_gap_refused(
    tag: &str,
    got: &[u64],
    err: Option<io::ErrorKind>,
    want: &[u64],
    hole_from: u64,
) {
    // Не `assert_eq!`: при провале он печатает весь вектор (сотни seq), а суть — в форме.
    assert!(
        got == want,
        "{tag} / JR-I-2 (A-042 §4 (в)/(г)): выдача обязана быть РОВНО {want:?}, затем abort; \
         получено {} событий, первое {:?}, последнее {:?} — дыра с {hole_from} пройдена МОЛЧА \
         («Ok через дыру вместо Err»)",
        got.len(),
        got.first(),
        got.last()
    );
    assert_eq!(
        err,
        Some(io::ErrorKind::InvalidData),
        "{tag} / JR-I-2: разрыв, не объяснённый шапками, обязан дать Err(InvalidData), получено {err:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// n1 — удалённый СРЕДНИЙ сегмент: правый край seg0 не сходится с first_seq преемника
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// 0: A 0..last0 · 1: A УДАЛЁН · 2: A … · 3: A … (активный). `All`, курсор на последнем
/// событии seg0 ⇒ выдано 0, `Err(InvalidData)`; курсор в середине seg0 ⇒ РОВНО
/// `after+1 ..= last0`, затем `Err`. Убивает «сброс ожидания на стыке» / отсутствие (в).
#[test]
fn n1_removed_middle_segment_is_refused_at_seg0_right_edge() {
    let (dir, cat, last0) = four_segments_one_epoch("n1");
    let victim = cat[1].clone();
    remove_segment(&victim); // DEFECT:n1
    let after_cat = assert_middle_removed("n1", dir.path(), &victim, last0);

    let (got, err) = subject(dir.path(), EpochFilter::All, last0);
    assert_gap_refused(
        &format!(
            "n1/after={last0} (курсор — конец seg0; преемник по каталогу {} ≠ {})",
            after_cat[1].first_seq,
            last0 + 1
        ),
        &got,
        err,
        &[],
        last0 + 1,
    );

    let mid = last0 - 4;
    let (got, err) = subject(dir.path(), EpochFilter::All, mid);
    assert_gap_refused(
        &format!("n1/after={mid} (курсор в середине seg0)"),
        &got,
        err,
        &range(mid + 1, last0),
        last0 + 1,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// n2 — вырезан ПЕРВЫЙ кадр преемника при честной шапке: правый край сходится, дыру видит
//       только ЛЕВЫЙ край (г)
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// 0: A 0..last0 · 1: A шапка `first_seq = last0+1`, кадр `last0+1` ВЫРЕЗАН (первый кадр
/// тела — `last0+2`) · 2: A … `All`, курсор в середине seg0 ⇒ РОВНО `after+1 ..= last0`,
/// затем `Err(InvalidData)`; событие `last0+2` не выдано. Убивает отсутствие (г): (в) на
/// стыке seg0→seg1 проходит (`last0 + 1 == first_seq(seg1)` по шапке).
#[test]
fn n2_first_frame_of_successor_cut_is_refused_at_left_edge() {
    let (dir, cat, last0) = four_segments_one_epoch("n2");
    let seg1 = cat[1].clone();
    cut_first_frame(&seg1); // DEFECT:n2

    // Стражи: шапка seg1 не тронута (каталог тот же), а тело начинается с first_seq+1.
    let after_cat = catalog(dir.path());
    if firsts(&after_cat) != firsts(&cat) {
        setup_failed(&format!(
            "n2: шапки изменились: {:?} → {:?}",
            firsts(&cat),
            firsts(&after_cat)
        ));
    }
    let body_first =
        first_seq_in_body(&after_cat[1]).unwrap_or_else(|| setup_failed("n2: тело seg1 пусто"));
    if body_first != seg1.first_seq + 1 {
        setup_failed(&format!(
            "n2: тело seg1 начинается с {body_first}, ожидалось first_seq+1 = {} — кадр не вырезан",
            seg1.first_seq + 1
        ));
    }
    if last0 + 1 != seg1.first_seq {
        setup_failed(
            "n2: правый край seg0 не сходится с шапкой seg1 — сценарий перестал быть «только (г)»",
        );
    }

    let mid = last0 - 4;
    let (got, err) = subject(dir.path(), EpochFilter::All, mid);
    assert!(
        !got.contains(&(seg1.first_seq + 1)),
        "n2 / JR-I-2 (A-042 §4 (г)): событие {} выдано, хотя кадр {} (первый по шапке seg1) отсутствует — \
         левый край открытого сегмента не сверен с его шапкой",
        seg1.first_seq + 1,
        seg1.first_seq
    );
    assert_gap_refused(
        &format!("n2/after={mid}"),
        &got,
        err,
        &range(mid + 1, last0),
        seg1.first_seq,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// n3 — как n1, но seg0 — `.zst`: (в) обязана применяться и к Passive-ветке читателя
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// 0: A `.zst` 0..last0 · 1: A УДАЛЁН · 2: A … · 3: A … `All`, курсор в середине seg0 ⇒
/// РОВНО `after+1 ..= last0`, затем `Err(InvalidData)`. Убивает «(в) не применяется к
/// `.zst`-читателю».
#[test]
fn n3_removed_middle_segment_after_zst_seg0_is_refused_too() {
    let (dir, cat, last0) = four_segments_one_epoch("n3");
    let segs = journal::list_segments(dir.path()).expect("segments");
    journal::compact_segment(&segs[0], journal::DEFAULT_COMPACT_LEVEL)
        .unwrap_or_else(|e| setup_failed(&format!("n3: компакция seg0: {e}")));
    let zcat = catalog(dir.path());
    if !zcat[0].zst || zcat.iter().skip(1).any(|s| s.zst) {
        setup_failed(&format!(
            "n3: после компакции .zst не ровно у seg0: {:?}",
            zcat.iter().map(|s| s.zst).collect::<Vec<_>>()
        ));
    }
    if last_seq_in_body(&zcat[0]) != Some(last0) {
        setup_failed(&format!(
            "n3: .zst seg0 читается не до {last0}: {:?}",
            last_seq_in_body(&zcat[0])
        ));
    }
    let victim = cat[1].clone();
    remove_segment(&victim); // DEFECT:n3
    assert_middle_removed("n3", dir.path(), &victim, last0);

    let mid = last0 - 4;
    let (got, err) = subject(dir.path(), EpochFilter::All, mid);
    assert_gap_refused(
        &format!("n3/after={mid} (seg0 — .zst)"),
        &got,
        err,
        &range(mid + 1, last0),
        last0 + 1,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// n4 — усечён хвост ПРИНЯТОГО сегмента перед ИСКЛЮЧЁННЫМ: (в) судится против физического
//       преемника, принят он фильтром или нет
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Каталог трёх эпох A/B/A (или A/Vendor/A) без ротации: `first_seq` = [0, 10, 30].
fn aba_catalog(tag: &str, middle: (DataSource, &str)) -> (tempfile::TempDir, Vec<Seg>) {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        cfg(DataSource::OwnCapture, EPOCH_A, BIG_SEG),
        10,
    );
    write(dir.path(), cfg(middle.0, middle.1, BIG_SEG), 20);
    write(
        dir.path(),
        cfg(DataSource::OwnCapture, EPOCH_A, BIG_SEG),
        10,
    );
    let cat = catalog(dir.path());
    if firsts(&cat) != [0, 10, 30] {
        setup_failed(&format!(
            "{tag}: ожидались first_seq [0, 10, 30], каталог {:?}",
            firsts(&cat)
        ));
    }
    let shape: Vec<(DataSource, &str)> = cat.iter().map(|s| (s.source, s.epoch.as_str())).collect();
    let want = [
        (DataSource::OwnCapture, EPOCH_A),
        middle,
        (DataSource::OwnCapture, EPOCH_A),
    ];
    if shape != want {
        setup_failed(&format!("{tag}: шапки {shape:?} ≠ ожидаемым {want:?}"));
    }
    assert_honest_catalog(tag, &cat);
    (dir, cat)
}

/// 0: A 0..9 · 1: B 10..29 (`Explicit([A])` исключает) · 2: A 30..39; у seg0 усечён
/// ПОСЛЕДНИЙ кадр (файл кончается на 8). `Explicit([A])`, `after = 5` ⇒ РОВНО 6..=8, затем
/// `Err(InvalidData)`; 30.. не выдано. Убивает «(в) только когда преемник принят».
#[test]
fn n4_truncated_tail_of_accepted_before_excluded_is_refused() {
    let (dir, cat) = aba_catalog("n4", (DataSource::OwnCapture, EPOCH_B));
    let seg0 = cat[0].clone();
    truncate_last_frame(&seg0); // DEFECT:n4

    let after_cat = catalog(dir.path());
    if firsts(&after_cat) != [0, 10, 30] {
        setup_failed(&format!("n4: шапки изменились: {:?}", firsts(&after_cat)));
    }
    let body_last =
        last_seq_in_body(&after_cat[0]).unwrap_or_else(|| setup_failed("n4: seg0 пуст"));
    if body_last != 8 {
        setup_failed(&format!(
            "n4: seg0 кончается на {body_last}, ожидалось 8 — хвост не усечён"
        ));
    }

    let (got, err) = subject(
        dir.path(),
        EpochFilter::Explicit(vec![EPOCH_A.to_string()]),
        5,
    );
    assert!(
        !got.iter().any(|s| *s >= 30),
        "n4 / JR-I-2 (A-042 §4 (в)): выдано {got:?} — читатель перешёл на seg2 через дыру 9 на \
         правом краю seg0, потому что физический преемник (seg1, B) фильтром исключён; (в) обязана \
         судить против ФИЗИЧЕСКОГО преемника"
    );
    assert_gap_refused("n4/Explicit([A])/after=5", &got, err, &range(6, 8), 9);
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// n5 — ПОЗИТИВНЫЙ КОНТРОЛЬ: законная проекция через исключённый сегмент выдаётся без ошибки
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Зелен на ревизии набора (класс `mn_10`). P1: A/B/A под `Explicit([A])`; P2: A/Vendor/A
/// под `OwnCaptureOnly`. `after = 9` ⇒ РОВНО 30..=39, без ошибки; `after = 5` ⇒ РОВНО
/// 6..=9, 30..=39; парный контроль `All`, `after = 9` ⇒ 10..=39. Убивает «перенос ожидания
/// через стык» и «сравнение с `first_seq` следующего ПРИНЯТОГО вместо физического»
/// (последний мутант на этой раскладке отверг бы 30 после 9, потому что следующий
/// принятый — seg2 с `first_seq = 30 ≠ 10`; правильное (в) сверяет 9+1 с seg1.first_seq = 10).
#[test]
fn n5_positive_control_legit_filtered_projection_passes_with_all_as_pair() {
    let layouts: [(&str, (DataSource, &str), EpochFilter); 2] = [
        (
            "P1 A/B/A",
            (DataSource::OwnCapture, EPOCH_B),
            EpochFilter::Explicit(vec![EPOCH_A.to_string()]),
        ),
        (
            "P2 A/Vendor/A",
            (DataSource::Vendor, EPOCH_VENDOR),
            EpochFilter::OwnCaptureOnly,
        ),
    ];
    for (tag, middle, filter) in layouts {
        let (dir, _cat) = aba_catalog(&format!("n5/{tag}"), middle);

        let (got, err) = subject(dir.path(), filter.clone(), 9);
        assert_eq!(
            (got.clone(), err),
            (range(30, 39), None),
            "n5/{tag}/after=9: законная проекция (дыра 10..29 целиком в исключённом сегменте, \
             объяснена шапками) обязана дать РОВНО 30..=39 без ошибки; получено {got:?} / {err:?} — \
             ожидание перенесено через стык вместо сброса на header.first_seq"
        );

        let (got, err) = subject(dir.path(), filter.clone(), 5);
        let mut want = range(6, 9);
        want.extend(range(30, 39));
        assert_eq!(
            (got.clone(), err),
            (want, None),
            "n5/{tag}/after=5: ожидалось 6..=9,30..=39 без ошибки; получено {got:?} / {err:?}"
        );

        let (got, err) = subject(dir.path(), EpochFilter::All, 9);
        assert_eq!(
            (got.clone(), err),
            (range(10, 39), None),
            "n5/{tag}/All/after=9 (парный контроль): без фильтра дыры нет — 10..=39 без ошибки; \
             получено {got:?} / {err:?}"
        );
    }
}
