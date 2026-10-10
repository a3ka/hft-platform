//! SACRED (architect-only) — `M-74` задача 1: **строитель ПРОД-ФОРМЕННОЙ фикстуры холодной
//! копии** + доказательство того, что её принимает НАСТОЯЩИЙ читатель (`journal::stream`).
//!
//! ## Зачем отдельный строитель, а не `printf` в shell-пробе
//!
//! Первая редакция `scripts/tests/red_restore_drill.sh` клала в «холодную копию» плоские
//! байты `SEGMENT-0001-PAYLOAD` и манифест `{"legacy":[...]}`. Такую копию не примет ни один
//! прод-читатель: сегмент schema ≥ 2 начинается с `SEGMENT_MAGIC = *b"HFTJRN02"`
//! (`crates/contracts/src/lib.rs:43`), а манифест — это `LegacyManifest { declarations: … }`
//! (`crates/contracts/src/lib.rs:68-70`), а не `{"legacy": …}`. Следствие, названное
//! арбитром `A-028` §3 п.2: позитивный контроль `H` прошёл бы ТОЛЬКО у обёртки с
//! mock-читателем, то есть проба толкала исполнителя В ОБХОД прод-пути.
//!
//! Здесь фикстура строится ТЕМ ЖЕ писателем, что пишет прод (`journal::Journal`), и той же
//! компакцией (`journal::compact_closed_segments`). Проверка «фикстура прод-формы» перестаёт
//! быть утверждением автора и становится прогоном: `prod_form_cold_copy_is_read_by_real_reader`
//! открывает построенное `journal::stream`'ом и считает события.
//!
//! ## ФОРМА ПРОДА СНЯТА ЗАМЕРОМ 2026-08-31, А НЕ ВООБРАЖЕНА (`testing.md` §«Форма прода»)
//!
//! Спека `M-74` в редакции `b8d989e` описывала форму копии НЕВЕРНО в трёх местах. Замеры
//! (ssh на прод и на Storage Box, вывод — в вердикте круга):
//!
//! ```text
//! $ cat $J/journal.legacy.json                    → {"declarations": []}   ← ПУСТ
//! $ ls $J/segment-* | head -1                     → segment-00000001.jrnl.zst
//! $ head -c8 segment-00000001.jrnl.zst | xxd      → 28b5 2ffd …  (zstd, не HFTJRN02)
//! $ ssh box 'ls journal/ | grep -v ^segment'      → journal.legacy.json
//!                                                    journal.replay-digest.json
//!                                                   ← journal.meta ОТСУТСТВУЕТ
//! $ ssh box 'ls journal/ | wc -l'                 → 501  (478 .zst + 21 .jrnl + 2 sidecar)
//! $ дублей «и .jrnl, и .jrnl.zst» по одному индексу → 17
//! ```
//!
//! Отсюда три свойства, которые фикстура ОБЯЗАНА нести и несёт:
//!
//! 1. **самый старый сегмент СЖАТ** (`.jrnl.zst`), а не legacy и не сырой. Drill, умеющий
//!    только `.jrnl`, молча пропустил бы 478 сегментов из 499 — и был бы зелёным;
//! 2. **`journal.meta` в копии НЕТ** — он переписывается ежесекундно, и `find -mmin +15`
//!    офсайт-обёртки его не видит. Drill, требующий `journal.meta`, отказывал бы ВСЕГДА;
//! 3. **один индекс присутствует в ДВУХ формах** (`.jrnl` и `.jrnl.zst`) — следствие
//!    осознанного отсутствия `--delete` у rsync плюс компакции на проде. 17 таких пар
//!    замерено. Отбор выборки обязан идти ПО ИНДЕКСУ, иначе «первый / средний / последний»
//!    по ИМЕНИ выберет два файла одного сегмента и один настоящий.
//!
//! Манифест фикстуры — настоящий `LegacyManifest`, сериализованный `serde_json`, с ПУСТЫМ
//! `declarations`: ровно то, что лежит на проде. Требование `A-028` §3 п.2 («манифест —
//! реальный `LegacyManifest`») выполнено по форме типа, а содержимое взято из замера.
//!
//! ## Что этот файл НЕ доказывает
//!
//! Он не проверяет сеть, права Storage Box и саму обёртку drill'а — только то, что
//! построенная им копия читается прод-читателем, а испорченная и пустая — нет. Прод-пруф
//! (первый автопрогон на VPS) остаётся отдельным шагом `M-74`, и здесь не изображается.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use contracts::{DataSource, EventKind, LegacyManifest, LegacySegmentDecl, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig, DEFAULT_COMPACT_LEVEL};

/// Сколько событий кладём в staging-журнал. Подобрано так, чтобы при `max_segment_bytes`
/// ниже получилось ≥4 сегмента: три закрытых (их и копируем) плюс активный (его копия
/// НЕ БЕРЁТ — на проде `find -mmin +15` его не видит).
const EVENTS: u64 = 12_000;
/// Мелкая ротация — единственный способ получить прод-РАСКЛАДКУ (несколько сегментов,
/// часть сжата) в тесте за доли секунды. Прод-масштаб байт здесь не нужен: предмет —
/// РАСКЛАДКА и ФОРМАТ, а не объём (объём меряется на VPS отдельным шагом).
const MAX_SEG: u64 = 64 * 1024;

/// Имя файла состояния drill'а внутри песочницы (совпадает со спекой `M-74`).
pub const STATE_FILE: &str = "journal-restore-drill.json";

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: MAX_SEG,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-74 restore-drill fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

/// **ОДНОРАЗОВОЕ ЗНАЧЕНИЕ ПРОГОНА — задача 6b, `Р-4` обязанность (а).**
///
/// Зачем. `digest` (ядро rev 5) доказывает чтение только если его НЕЛЬЗЯ НАЗВАТЬ ЗАРАНЕЕ.
/// Все прежние фикстуры были детерминированы линейным `i`, то есть повторный прогон давал
/// ТЕ ЖЕ события и ТОТ ЖЕ отпечаток — обёртка, однажды увидевшая правильное значение,
/// подставила бы его константой, и «мир ¬P не несёт признака» оказалось бы неверным.
///
/// Конструкция скопирована с `scripts/reserve_artifact_id.sh` — там структурно ТА ЖЕ
/// задача («значение обязано быть невоспроизводимо снаружи до факта»), и она уже прошла
/// круги гейта:
///   · источник — `/proc/sys/kernel/random/uuid`, иначе `/dev/urandom`;
///   · **отката на слабый источник НЕТ НАМЕРЕННО** — лучше отказ, чем предсказуемый нонс;
///   · **самопроверка на ВЫРОЖДЕНИЕ**: два независимых чтения обязаны различаться.
///
/// Паника здесь законна и обязательна: фикстура без одноразовости молча превратила бы
/// оракул отпечатка в проверку константы, то есть в плацебо.
fn drill_nonce() -> u64 {
    fn draw() -> Option<u64> {
        if let Ok(uuid) = fs::read_to_string("/proc/sys/kernel/random/uuid") {
            let hex: String = uuid
                .chars()
                .filter(|c| c.is_ascii_hexdigit())
                .take(16)
                .collect();
            if hex.len() == 16 {
                if let Ok(v) = u64::from_str_radix(&hex, 16) {
                    return Some(v);
                }
            }
        }
        let bytes = fs::read("/dev/urandom").ok()?;
        let head = bytes.get(..8)?;
        Some(u64::from_le_bytes(head.try_into().ok()?))
    }
    let a = draw().expect(
        "источника одноразового значения нет (ни /proc/sys/kernel/random/uuid, ни /dev/urandom); \
         отката на слабый источник нет НАМЕРЕННО: предсказуемый нонс делает оракул отпечатка плацебо",
    );
    let b = draw().expect("второе чтение источника одноразового значения не удалось");
    assert_ne!(
        a, b,
        "источник одноразового значения ВЫРОЖДЕН: два независимых чтения дали одно значение. \
         Фикстура с предсказуемым нонсом превращает digest в константу, и обёртка подставит \
         его, не читая копию (Р-4 обязанность (а))"
    );
    a
}

/// Фиксированное значение для тестов, которым одноразовость НЕ НУЖНА: они судят
/// читаемость формы, а не различающую силу отпечатка. Разделение явное, чтобы случайность
/// не просочилась туда, где она сделала бы тест недетерминированным без пользы.
const STATIC_NONCE: u64 = 0;

fn ev(i: u64, nonce: u64) -> EventKind {
    // Нонс входит в `price` — поле, участвующее в формуле `digest`
    // (`sha256(Σ "<seq>:<ts_exch_ms>:<price_e8>\n")`, спека §«Поле digest»). Через `ts_exch_ms`
    // его вносить нельзя: время участвует в раскладке по сегментам и окнам, и сдвиг менял бы
    // ФОРМУ фикстуры, а не только её содержимое. Диапазон сужен остатком, чтобы цена
    // оставалась правдоподобной величиной, а не переполняла разумные границы.
    let spread = (nonce % 1_000_000) as i64 * 1_000;
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: 6_400_000_000_000 + spread + i as i64,
            size: 100 + (i as i64 % 7),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

/// Сериализовать ПУСТОЙ `LegacyManifest` — байт-в-байт по форме прода
/// (`{"declarations": []}` — замер 2026-08-31, 25 Б).
fn empty_manifest_bytes() -> Vec<u8> {
    serde_json::to_vec_pretty(&LegacyManifest::default()).expect("serialize LegacyManifest")
}

/// Построить STAGING-журнал прод-раскладки и вернуть путь.
/// Результат: несколько закрытых сегментов, часть сжата в `.jrnl.zst`, активный — сырой.
fn build_staging(root: &Path, nonce: u64) -> PathBuf {
    let stage = root.join("stage");
    fs::create_dir_all(&stage).expect("mkdir stage");

    let mut j = Journal::open_with(&stage, cfg()).expect("open_with");
    for i in 0..EVENTS {
        j.append(ev(i, nonce)).expect("append");
    }
    j.flush().expect("flush");
    drop(j);

    let segs = journal::list_segments(&stage).expect("list_segments");
    assert!(
        segs.len() >= 4,
        "SETUP НЕ СОСТОЯЛСЯ: нужно ≥4 сегмента для прод-раскладки (закрытые + активный), \
         получено {}. Увеличь EVENTS или уменьши MAX_SEG",
        segs.len()
    );

    // Сохраняем СЫРУЮ копию сегмента ДО компакции — она станет тем самым дублем
    // «и .jrnl, и .jrnl.zst», которых на боевой коробке замерено 17.
    //
    // ДУБЛЬ ЛЕЖИТ НА ВЫБИРАЕМОМ ИНДЕКСЕ, И ЭТО ТРЕБОВАНИЕ, А НЕ СЛУЧАЙНОСТЬ (`C-218` B-2).
    // Прежде дубль строился на `segs[1]` — индексе `00000001`, которого правило выборки НЕ
    // берёт (оно берёт первый / `floor(N/2)` / последний, то есть `00000000`/`00000003`/
    // `00000007`). Из-за этого обязанность спеки «для каждого выбранного индекса
    // восстанавливаются ОБЕ формы» была ВАКУУМНА: ни один выбранный индекс не имел двух форм,
    // и обёртку, роняющую одну из них, отвергнуть было НЕЧЕМ. Дубль перенесён на `segs[3]`
    // (индекс `00000003` — средний член выборки); страж ниже это утверждает исполнением.
    let dup_idx = segs[3].index;
    let dup_raw = root.join(format!("dup-segment-{dup_idx:08}.jrnl"));
    fs::copy(&segs[3].path, &dup_raw).expect("сохранить сырой дубль до компакции");

    // `keep_raw = 1`: последний ЗАКРЫТЫЙ остаётся сырым, остальные закрытые сжимаются.
    // Это и есть прод-пропорция (замер: 478 `.zst` против 21 `.jrnl`).
    let reports =
        journal::compact_closed_segments(&stage, 1, DEFAULT_COMPACT_LEVEL).expect("compact");
    assert!(
        !reports.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: компакция не сжала ни одного сегмента — фикстура не прод-формы \
         (на проде самый СТАРЫЙ сегмент сжат)"
    );

    // Возвращаем сырой дубль на место рядом со сжатым — воспроизводим измеренную пару.
    let dup_dst = stage.join(format!("segment-{dup_idx:08}.jrnl"));
    fs::copy(&dup_raw, &dup_dst).expect("вернуть сырой дубль");
    assert!(
        stage
            .join(format!("segment-{dup_idx:08}.jrnl.zst"))
            .exists(),
        "SETUP НЕ СОСТОЯЛСЯ: сжатой формы дубля нет — пара raw+zst не воспроизведена"
    );

    stage
}

/// Собрать «холодную копию» ИЗ staging'а ровно так, как её собирает офсайт-обёртка:
/// все файлы, КРОМЕ активного сегмента и `journal.meta`/`recorder.heartbeat`.
///
/// Возвращает число скопированных СЕГМЕНТНЫХ файлов.
fn build_cold(stage: &Path, cold: &Path) -> usize {
    fs::create_dir_all(cold).expect("mkdir cold");

    let segs = journal::list_segments(stage).expect("list_segments");
    let active = segs.iter().map(|s| s.index).max().expect("есть сегменты");

    let mut copied = 0usize;
    for entry in fs::read_dir(stage).expect("read_dir stage") {
        let p = entry.expect("dirent").path();
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .expect("имя файла")
            .to_string();

        // `journal.meta` в копию НЕ ПОПАДАЕТ — замер Storage Box 2026-08-31: его там нет,
        // потому что recorder переписывает его ежесекундно и `find -mmin +15` его не видит.
        if name == "journal.meta" || name == "recorder.heartbeat" {
            continue;
        }
        if name.starts_with("segment-") {
            let base = name.trim_end_matches(".zst");
            let idx: u32 = base
                .trim_start_matches("segment-")
                .trim_end_matches(".jrnl")
                .parse()
                .expect("индекс сегмента");
            // Активный сегмент не копируется (mtime-фильтр обёртки).
            if idx == active {
                continue;
            }
            copied += 1;
        }
        fs::copy(&p, cold.join(&name)).expect("копия файла в cold");
    }

    // Sidecar'ы, которые НА КОРОБКЕ ЕСТЬ (замер): манифест и replay-digest.
    fs::write(cold.join(journal::LEGACY_MANIFEST), empty_manifest_bytes()).expect("манифест");
    fs::write(
        cold.join("journal.replay-digest.json"),
        br#"{"through_seq":0,"digest":"sha256:fixture","code_version":"M-74-fixture"}"#,
    )
    .expect("replay-digest");

    assert!(
        copied >= 3,
        "SETUP НЕ СОСТОЯЛСЯ: в холодной копии {copied} сегментных файлов, а правило выборки \
         спеки берёт ТРИ (первый / средний / последний закрытый)"
    );
    copied
}

/// Убрать ОДИН ЦЕЛЫЙ фрейм события из СЫРОГО сегмента. Обрамление остаётся ЗАКОННЫМ (каждый
/// фрейм `len u32 LE | payload | crc32 u32 LE` цел), а `seq` внутри ОДНОГО сегмента получает
/// дыру — ровно тот случай, который `intra_segment_continuous` обязан объявлять `false`.
///
/// Заведено закрытием `C-218` B-1: без него «сквозная непрерывность» и «непрерывность внутри
/// сегмента» дают ОДИН И ТОТ ЖЕ ответ на здоровой выборке, и неверная реализация неотличима
/// от верной. Здоровый несмежный отбор обязан давать `true`, дыра внутри сегмента — `false`;
/// один без другого не различает миры.
fn drop_one_event_frame(path: &Path, skip_events: usize) {
    let bytes = fs::read(path).expect("read segment");
    assert!(
        bytes.starts_with(b"HFTJRN02"),
        "SETUP НЕ СОСТОЯЛСЯ: {} не сырой сегмент schema-v2 — фреймы не разобрать",
        path.display()
    );
    let read_len = |off: usize| -> usize {
        u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]) as usize
    };
    // 8 байт MAGIC, затем фрейм ЗАГОЛОВКА, затем фреймы событий.
    let mut off = 8usize;
    off += 4 + read_len(off) + 4; // заголовок
    for _ in 0..skip_events {
        assert!(
            off < bytes.len(),
            "SETUP НЕ СОСТОЯЛСЯ: событий меньше, чем нужно пропустить"
        );
        off += 4 + read_len(off) + 4;
    }
    assert!(
        off < bytes.len(),
        "SETUP НЕ СОСТОЯЛСЯ: выедать нечего — сегмент кончился на {off} из {}",
        bytes.len()
    );
    let victim_len = 4 + read_len(off) + 4;
    let mut out = Vec::with_capacity(bytes.len() - victim_len);
    out.extend_from_slice(&bytes[..off]);
    out.extend_from_slice(&bytes[off + victim_len..]);
    fs::write(path, &out).expect("write segment without one frame");
}

/// ПОРЧА: испортить байты внутри `.zst`-сегмента копии, оставив имя и размер прежними.
/// Именно так выглядит тихая порча носителя — файл на месте, `ls` ничего не показывает.
fn corrupt_one_compacted(cold: &Path) -> PathBuf {
    let mut victim: Option<PathBuf> = None;
    let mut names: Vec<PathBuf> = fs::read_dir(cold)
        .expect("read_dir cold")
        .map(|e| e.expect("dirent").path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.ends_with(".jrnl.zst"))
                .unwrap_or(false)
        })
        .collect();
    names.sort();
    if let Some(p) = names.first() {
        let mut bytes = fs::read(p).expect("read victim");
        assert!(
            bytes.len() > 64,
            "SETUP НЕ СОСТОЯЛСЯ: сегмент-жертва слишком мал ({} Б) — порча не отличима от пустоты",
            bytes.len()
        );
        // Ломаем СЕРЕДИНУ zstd-потока: заголовок цел, распаковка обрывается — ровно
        // «на вид файл есть, читатель отказывает».
        let mid = bytes.len() / 2;
        for b in bytes.iter_mut().skip(mid).take(32) {
            *b ^= 0xFF;
        }
        fs::write(p, &bytes).expect("write corrupted");
        victim = Some(p.clone());
    }
    victim.expect("SETUP НЕ СОСТОЯЛСЯ: в холодной копии нет ни одного .zst — порчу негде внести")
}

/// Сколько событий отдаёт НАСТОЯЩИЙ читатель на каталоге. `Err` — отдельный исход.
/// **НЕЗАВИСИМЫЙ ЭТАЛОН ОТПЕЧАТКА — опора сценариев `D`/`R` пробы.**
///
/// Формула объявлена спекой дословно и обязана считаться ДВУМЯ сторонами одинаково:
/// `sha256( Σ по событиям в порядке чтения: "<seq>:<ts_exch_ms>:<price_e8>\n" )`.
///
/// **Почему здесь, а не через бинарь `journal-drill-read`.** Эталон обязан приходить из
/// НЕЗАВИСИМОГО пути (`testing.md`: «эталон берётся из независимого пути: полный реплей,
/// вторая реализация, записанная фикстура»). Бинарь — то, что зовёт судимая обёртка;
/// брать эталон оттуда значило бы сверять источник с самим собой. Здесь свёртка считается
/// прямо библиотекой `journal`, и у пробы появляется вторая, не зависящая от бинаря опора.
///
/// Практическое следствие, ради которого это и сделано: сценарии `D`/`R` (самопроверка
/// различающей силы) исполняются УЖЕ СЕЙЧАС, до задачи 2b. Без независимого эталона они
/// печатали бы `PASS` вакуумно — «поймали подделку» было бы неотличимо от «красно всё,
/// потому что читателя нет». Такой зелёный и есть тот класс, за который набор отвергали
/// четыре круга подряд.
fn digest_of_dir(dir: &Path) -> std::io::Result<(usize, String)> {
    read_dir_full(dir).map(|r| (r.events_read, r.digest))
}

/// ПОЛНЫЙ объявленный протокол успеха читателя drill'а (`M-74` §«Сигнатура читателя»).
/// Заведён закрытием `C-217` N-1: прежде эталон отдавал ДВА поля из шести объявленных,
/// то есть проба судила удобное себе подмножество контракта, называя его контрактом.
struct DrillRead {
    segments_read: usize,
    events_read: usize,
    seq_first: u64,
    seq_last: u64,
    intra_segment_continuous: bool,
    digest: String,
}

fn read_dir_full(dir: &Path) -> std::io::Result<DrillRead> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    let mut n = 0usize;
    let mut seq_first: Option<u64> = None;
    let mut seq_last: u64 = 0;
    let mut prev: Option<u64> = None;
    // ═══ НЕПРЕРЫВНОСТЬ — ВНУТРИ СЕГМЕНТА, И СБРОС НА ГРАНИЦЕ ТЕПЕРЬ ЕСТЬ (`C-218` B-1) ═══
    //
    // Прежняя редакция несла ЭТОТ ЖЕ комментарий («отсюда — сброс на границе») и сброса НЕ
    // ДЕЛАЛА: `prev` тянулся через весь поток. На здоровой выборке несмежных индексов
    // (`00000000`/`00000003`/`00000007`) читатель объявлял `intra_segment_continuous=false`,
    // хотя каждый выбранный сегмент по отдельности непрерывен. Комментарий, описывающий
    // поведение, которого в коде нет, — ложный якорь: он читается как покрытие.
    //
    // Спека (§«Правило выборки», СЕМАНТИКА НЕПРЕРЫВНОСТИ) прямо запрещает заявлять сквозную
    // непрерывность на выборке: три несмежных сегмента её проверить не могут ПО ПОСТРОЕНИЮ.
    // Границы берутся из заголовков сегментов (`first_seq`), а не угадываются по величине
    // разрыва: «большой разрыв = граница» было бы догадкой, и порча внутри сегмента,
    // выевшая много событий подряд, проехала бы как «стык».
    let bounds: std::collections::HashSet<u64> = journal::list_segments(dir)
        .map(|v| v.iter().map(|s| s.header.first_seq).collect())
        .unwrap_or_default();
    let mut continuous = true;
    for e in journal::stream(dir, EpochFilter::All)? {
        let ev = e?;
        if seq_first.is_none() {
            seq_first = Some(ev.seq);
        }
        seq_last = ev.seq;
        // Первое событие сегмента — законный разрыв: сравнивать его с хвостом предыдущего
        // нечего. Если заголовок соврал и `first_seq` не совпал с фактом, сброса не будет и
        // ответ окажется КОНСЕРВАТИВНЫМ (`false`), а не оптимистичным.
        if bounds.contains(&ev.seq) {
            prev = None;
        }
        if let Some(p) = prev {
            if ev.seq > p + 1 {
                continuous = false;
            }
        }
        prev = Some(ev.seq);
        if let EventKind::Md(md) = &ev.kind {
            if let MdPayload::Trade {
                price, ts_exch_ms, ..
            } = &md.payload
            {
                h.update(format!("{}:{}:{}\n", ev.seq, ts_exch_ms, price).as_bytes());
                n += 1;
            }
        }
    }
    let segments_read = std::fs::read_dir(dir)
        .map(|rd| {
            let mut idx: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter_map(|f| {
                    f.strip_prefix("segment-").and_then(|r| {
                        r.strip_suffix(".jrnl")
                            .or_else(|| r.strip_suffix(".jrnl.zst"))
                            .map(|s| s.to_string())
                    })
                })
                .collect();
            idx.sort();
            idx.dedup();
            idx.len()
        })
        .unwrap_or(0);
    Ok(DrillRead {
        segments_read,
        events_read: n,
        seq_first: seq_first.unwrap_or(0),
        seq_last,
        intra_segment_continuous: continuous,
        digest: format!("{:x}", h.finalize()),
    })
}

/// Точка входа для shell-пробы: посчитать отпечаток каталога и напечатать его.
/// Каталог приходит в `DRILL_DIGEST_DIR`; без переменной тест не утверждает ничего —
/// он гоняется в общем `cargo test --all`, где считать нечего.
/// **`C-218` B-1, обязанность (а): здоровый НЕСМЕЖНЫЙ отбор непрерывен.**
///
/// Три выбранных индекса (первый / `floor(N/2)` / последний) по построению НЕ СОСЕДНИ, и
/// между ними в `seq` зияют дыры — законные. Читатель обязан объявить
/// `intra_segment_continuous=true`, потому что семантика поля — непрерывность ВНУТРИ
/// сегмента (спека §«Правило выборки»). Прежняя реализация тянула `prev` через весь поток и
/// отвечала `false` на ЗДОРОВОЙ копии: протокол был well-formed и семантически ложен.
#[test]
fn non_adjacent_healthy_selection_is_continuous() {
    let root = std::env::temp_dir().join(format!("drill-cont-ok-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("mkdir root");
    let stage = build_staging(&root, drill_nonce());
    let cold = root.join("cold");
    build_cold(&stage, &cold);

    let sel = root.join("sel");
    fs::create_dir_all(&sel).expect("mkdir sel");
    let picked = select_by_contract(&cold);
    assert_eq!(
        picked.len(),
        3,
        "SETUP НЕ СОСТОЯЛСЯ: отбор дал {} индексов вместо трёх",
        picked.len()
    );
    let mut copied_forms = 0usize;
    for idx in &picked {
        for suffix in [".jrnl", ".jrnl.zst"] {
            let src = cold.join(format!("segment-{idx:08}{suffix}"));
            if src.exists() {
                fs::copy(&src, sel.join(format!("segment-{idx:08}{suffix}"))).expect("copy");
                copied_forms += 1;
            }
        }
    }
    for side in ["journal.legacy.json", "journal.replay-digest.json"] {
        let src = cold.join(side);
        if src.exists() {
            fs::copy(&src, sel.join(side)).expect("copy sidecar");
        }
    }
    // Страж НЕСМЕЖНОСТИ: иначе тест был бы зелен и на сквозной семантике, то есть не
    // различал бы верную реализацию и неверную.
    assert!(
        picked[2] > picked[1] + 1 || picked[1] > picked[0] + 1,
        "SETUP НЕ СОСТОЯЛСЯ: выбранные индексы {picked:?} СОСЕДНИ — на них сквозная и \
         внутрисегментная семантика дают ОДИН ответ, и тест ничего не различает"
    );
    assert!(
        copied_forms > picked.len(),
        "SETUP НЕ СОСТОЯЛСЯ: скопировано {copied_forms} файлов на {} индексов — пара raw+zst \
         на выбранном индексе не воспроизведена (`C-218` B-2)",
        picked.len()
    );

    let r = read_dir_full(&sel).expect("читатель обязан открыть здоровую выборку");
    assert_eq!(r.segments_read, 3, "выборка обязана нести три индекса");
    assert!(r.events_read > 0, "выборка обязана нести события");
    assert!(
        r.intra_segment_continuous,
        "ЗДОРОВЫЙ несмежный отбор объявлен РАЗРЫВНЫМ: seq_first={} seq_last={} — это `C-218` \
         B-1, семантика поля читается как сквозная вместо внутрисегментной",
        r.seq_first, r.seq_last
    );
    let _ = fs::remove_dir_all(&root);
}

/// **`C-218` B-1, обязанность (б): дыра ВНУТРИ одного сегмента разрывна.**
///
/// Обратная сторона того же поля. Без неё «всегда `true`» прошло бы обязанность (а) и
/// поле не значило бы ничего — оракул обязан падать против СЛОМАННОГО, а не только
/// проходить против здорового (`testing.md` §«Целостность гейта» св. 3).
#[test]
fn intra_segment_gap_is_discontinuous() {
    let root = std::env::temp_dir().join(format!("drill-cont-gap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("mkdir root");
    let stage = build_staging(&root, drill_nonce());

    // Берём ОДИН сырой закрытый сегмент — «внутри сегмента» требует ровно одного.
    let segs = journal::list_segments(&stage).expect("list_segments");
    let raw = segs
        .iter()
        .find(|s| {
            s.path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".jrnl"))
                .unwrap_or(false)
                && s.index < segs.iter().map(|x| x.index).max().unwrap()
        })
        .expect("SETUP НЕ СОСТОЯЛСЯ: сырого закрытого сегмента нет");

    let one = root.join("one");
    fs::create_dir_all(&one).expect("mkdir one");
    let dst = one.join(raw.path.file_name().expect("имя"));
    fs::copy(&raw.path, &dst).expect("copy raw segment");
    fs::write(one.join(journal::LEGACY_MANIFEST), empty_manifest_bytes()).expect("манифест");

    let before = read_dir_full(&one).expect("здоровый одиночный сегмент обязан читаться");
    assert!(
        before.intra_segment_continuous,
        "SETUP НЕ СОСТОЯЛСЯ: сегмент разрывен ДО порчи — тест судил бы не тот предмет"
    );

    drop_one_event_frame(&dst, 5);

    let after = read_dir_full(&one).expect("сегмент с выеденным фреймом обязан читаться");
    assert!(
        after.events_read > 0,
        "SETUP НЕ СОСТОЯЛСЯ: после выедания фрейма событий не осталось — судится пустота, \
         а не разрыв"
    );
    assert!(
        !after.intra_segment_continuous,
        "ДЫРА ВНУТРИ СЕГМЕНТА объявлена непрерывной (событий {}, seq {}..{}). Поле \
         `intra_segment_continuous` не значит ничего",
        after.events_read, after.seq_first, after.seq_last
    );
    let _ = fs::remove_dir_all(&root);
}

/// Отбор ПО КОНТРАКТУ спеки §«Правило выборки»: индексы, дедуплицированные по форме имени,
/// затем первый / `floor(N/2)` / последний. Живёт ЗДЕСЬ, а не в shell-пробе: проба сверяет
/// доставленное с ЛИТЕРАЛОМ и правило не пересчитывает (редекларация судимой логики стоила
/// круга на `M-45`). Здесь это строитель фикстуры, а не судья.
fn select_by_contract(cold: &Path) -> Vec<u32> {
    let mut idx: Vec<u32> = fs::read_dir(cold)
        .expect("read_dir cold")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|n| {
            n.strip_prefix("segment-").and_then(|r| {
                r.strip_suffix(".jrnl")
                    .or_else(|| r.strip_suffix(".jrnl.zst"))
                    .and_then(|s| s.parse::<u32>().ok())
            })
        })
        .collect();
    idx.sort_unstable();
    idx.dedup();
    let n = idx.len();
    if n < 3 {
        return idx;
    }
    let mid = (n + 1) / 2;
    vec![idx[0], idx[mid - 1], idx[n - 1]]
}

#[test]
fn digest_for_shell_probe() {
    let Ok(dir) = std::env::var("DRILL_DIGEST_DIR") else {
        eprintln!("digest_for_shell_probe: DRILL_DIGEST_DIR не задан — расчёт не запрошен");
        return;
    };
    match digest_of_dir(Path::new(&dir)) {
        Ok((n, d)) => println!("DRILL_DIGEST events={n} digest={d}"),
        Err(e) => println!("DRILL_DIGEST_ERROR {e}"),
    }
}

/// **ЭТАЛОННЫЙ ЧИТАТЕЛЬ для пробы — закрытие корня вместе с эталонной обёрткой.**
///
/// Прод-читатель (`crates/journal/src/bin/journal-drill-read.rs`, задача 2b) — зона
/// engine-dev, и его ещё нет. Пока его нет, сценарии `H`/`C`/`F`/`A`/`T` пробы не могут
/// исполниться ВООБЩЕ: обёртке некого звать. Именно так две трети набора и оставались
/// непроверенным кодом — корень серии из семи отказов (`C-216`, решение founder'а 08.09).
///
/// Здесь — ТЕСТОВАЯ реализация объявленного контракта читателя: та же формула отпечатка и
/// те же классы исхода, что обязан дать прод-бинарь. Она НЕ заменяет его и в прод не идёт;
/// она делает набор исполнимым СЕЙЧАС, чтобы позитивный контроль был настоящим.
///
/// Коды исхода — контракт спеки: `0` прочитано · `4` ПОРЧА · `5` ПУСТОТА/ниже минимума.
/// Различение 4 и 5 существенно: без него сценарии `C` и `F` судят один и тот же исход.
#[test]
fn reference_reader_for_shell_probe() {
    let Ok(dir) = std::env::var("DRILL_READER_DIR") else {
        eprintln!("reference_reader_for_shell_probe: DRILL_READER_DIR не задан");
        return;
    };
    let min: usize = std::env::var("DRILL_READER_MIN_EVENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    // Печатается ПОЛНЫЙ объявленный протокол (`C-217` N-1). Поля отдаются одной строкой
    // ключ=значение; JSON собирает shell-обёртка эталона — она и есть «читатель» для пробы.
    match read_dir_full(Path::new(&dir)) {
        Err(e) => println!("DRILL_READER rc=4 events_read=0 digest= reason=corrupt:{e}"),
        Ok(r) if r.events_read < min => println!(
            "DRILL_READER rc=5 segments_read={} events_read={} seq_first={} seq_last={} \
intra_segment_continuous={} digest= reason=empty-below-min",
            r.segments_read, r.events_read, r.seq_first, r.seq_last, r.intra_segment_continuous
        ),
        Ok(r) => println!(
            "DRILL_READER rc=0 segments_read={} events_read={} seq_first={} seq_last={} \
intra_segment_continuous={} digest={} reason=",
            r.segments_read,
            r.events_read,
            r.seq_first,
            r.seq_last,
            r.intra_segment_continuous,
            r.digest
        ),
    }
}

fn read_events(dir: &Path) -> std::io::Result<usize> {
    let mut n = 0usize;
    for e in journal::stream(dir, EpochFilter::All)? {
        e?;
        n += 1;
    }
    Ok(n)
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// ГЛАВНОЕ: три исхода, СНЯТЫЕ ПРОД-ЧИТАТЕЛЕМ. Это и есть предъявление того, что фикстура
// прод-формы: не «я так написал», а «`journal::stream` её принял».
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn prod_form_cold_copy_is_read_by_real_reader() {
    let root = tempfile::tempdir().expect("tempdir");
    let stage = build_staging(root.path(), STATIC_NONCE);
    let cold = root.path().join("cold");
    build_cold(&stage, &cold);

    // Позитивный контроль фикстуры: сжатая + сырая формы, дубль индекса, пустой манифест,
    // отсутствующий `journal.meta` — и всё это ЧИТАЕТСЯ.
    let n = read_events(&cold).expect("прод-читатель обязан открыть здоровую холодную копию");
    assert!(
        n > 0,
        "холодная копия прочитана, но событий НОЛЬ — drill на такой копии рапортовал бы успех, \
         ничего не прочитав (класс R-157 Б-5)"
    );

    // Раскладка предъявляется числами, а не словами.
    let files: Vec<String> = fs::read_dir(&cold)
        .expect("read_dir cold")
        .map(|e| e.expect("dirent").file_name().to_string_lossy().to_string())
        .collect();
    let zst = files.iter().filter(|f| f.ends_with(".jrnl.zst")).count();
    let raw = files.iter().filter(|f| f.ends_with(".jrnl")).count();
    assert!(
        zst >= 1 && raw >= 1,
        "прод-раскладка требует ОБЕ формы: сжатых {zst}, сырых {raw}"
    );
    assert!(
        !files.iter().any(|f| f == "journal.meta"),
        "`journal.meta` попал в холодную копию — на боевой коробке его НЕТ (замер 2026-08-31), \
         и фикстура, которая его кладёт, разрешит drill'у на него опереться"
    );
    assert!(
        files.iter().any(|f| f == journal::LEGACY_MANIFEST),
        "манифест обязан быть в копии — на коробке он есть"
    );

    // Дубль индекса (17 пар замерено на коробке) НЕ ломает прод-читателя: `dedup_indexed_paths`
    // при коллизии предпочитает сырой (D-COMP-1). Пиннится здесь, потому что от этого зависит
    // правило выборки drill'а: считать надо ИНДЕКСЫ, а не файлы.
    let idx_of = |f: &String| -> Option<u32> {
        f.strip_prefix("segment-")
            .map(|r| r.trim_end_matches(".zst").trim_end_matches(".jrnl"))
            .and_then(|r| r.parse().ok())
    };
    let mut indices: Vec<u32> = files.iter().filter_map(idx_of).collect();
    let seg_files = indices.len();
    indices.sort_unstable();
    indices.dedup();
    assert!(
        seg_files > indices.len(),
        "SETUP НЕ СОСТОЯЛСЯ: в копии {seg_files} сегментных файлов на {} индексов — дубль \
         raw+zst не воспроизведён, а на коробке таких пар 17",
        indices.len()
    );
}

#[test]
fn corrupted_cold_copy_is_rejected_by_real_reader() {
    let root = tempfile::tempdir().expect("tempdir");
    let stage = build_staging(root.path(), STATIC_NONCE);
    let cold = root.path().join("cold");
    build_cold(&stage, &cold);

    let before = read_events(&cold).expect("предусловие: здоровая копия читается");
    let victim = corrupt_one_compacted(&cold);

    match read_events(&cold) {
        Err(e) => {
            assert!(
                !journal::is_foreign_segment(&e),
                "порча байт обязана давать ошибку ЧТЕНИЯ, а не «чужой сегмент» — иначе drill \
                 назовёт порчу отсутствием контекста: {e}"
            );
        }
        Ok(after) => panic!(
            "порча {} прошла НЕЗАМЕЧЕННОЙ прод-читателем: было {before} событий, стало {after}. \
             Drill на такой копии объявит бэкап живым",
            victim.display()
        ),
    }
}

#[test]
fn empty_restore_yields_zero_events_not_an_error() {
    // Отдельный исход, и его нельзя путать с порчей: копия скачалась «успешно», но НИЧЕГО
    // не привезла (сеть, права, пустой фильтр). Читатель при этом НЕ падает — он честно
    // отдаёт ноль событий. Именно поэтому drill, решающий по `exit=0` читателя, объявит
    // успех на пустоте — класс `R-157` `Б-5`, и он ловится ЗДЕСЬ.
    let root = tempfile::tempdir().expect("tempdir");
    let empty = root.path().join("empty-restore");
    fs::create_dir_all(&empty).expect("mkdir");
    fs::write(empty.join(journal::LEGACY_MANIFEST), empty_manifest_bytes()).expect("манифест");

    let n = read_events(&empty).expect("пустой каталог читается без ошибки — это и есть ловушка");
    assert_eq!(
        n, 0,
        "предусловие сценария: на пустом восстановлении событий ноль"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// FORWARD-GUARD (НЕ прод-форма). На проде `journal.legacy.json` ПУСТ (замер 2026-08-31:
// `{"declarations": []}`), поэтому сценарий «legacy без манифеста» сегодня НЕДОСТИЖИМ.
// Он оставлен намеренно и помечен: как только появится хоть одна декларация, drill,
// восстановивший сегменты БЕЗ манифеста, объявит ЗДОРОВУЮ копию битой. Различать этот
// исход обязан код возврата читателя, и вот его отличительный признак.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn undeclared_legacy_is_a_context_error_not_a_corruption_error() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("legacy");
    fs::create_dir_all(&dir).expect("mkdir");

    // Legacy-сегмент: postcard-фреймы + crc32, БЕЗ магии и заголовка (форма боевого
    // `segment-00000000.jrnl` до CT-RFC-02).
    let path = dir.join("segment-00000000.jrnl");
    {
        let f = fs::File::create(&path).expect("create legacy");
        let mut w = std::io::BufWriter::new(f);
        for seq in 0..200u64 {
            let e = contracts::Event {
                seq,
                ts_mono_ns: seq,
                ts_wall_ms: 1_752_000_000_000 + seq as i64,
                kind: ev(seq, STATIC_NONCE),
            };
            let payload = postcard::to_stdvec(&e).expect("ser");
            w.write_all(&(payload.len() as u32).to_le_bytes())
                .expect("len");
            w.write_all(&payload).expect("payload");
            w.write_all(&crc32fast::hash(&payload).to_le_bytes())
                .expect("crc");
        }
        w.flush().expect("flush");
    }

    // (1) БЕЗ декларации — «нет контекста», и это ОТДЕЛЬНЫЙ признак.
    let err = read_events(&dir).expect_err("незадекларированный legacy обязан быть отвергнут");
    assert!(
        journal::is_foreign_segment(&err),
        "признак «чужой/незадекларированный сегмент» обязан отличаться от порчи, иначе drill \
         скажет «копия битая» там, где не хватает манифеста: {err}"
    );

    // (2) С декларацией — та же копия читается. Без этой половины первая половина зелена
    // и у процедуры, которая не работает никогда.
    journal::declare_legacy(
        &dir,
        LegacySegmentDecl {
            file_name: "segment-00000000.jrnl".to_string(),
            fingerprint_sha256: String::new(), // считает сам `declare_legacy`
            size_bytes_at_decl: 0,             // и размер тоже
            source: DataSource::OwnCapture,
            provenance: "M-74 forward-guard fixture".to_string(),
            epoch_id: contracts::LEGACY_EPOCH_ID.to_string(),
        },
    )
    .expect("declare_legacy");

    let n = read_events(&dir).expect("задекларированный legacy обязан читаться");
    assert_eq!(n, 200, "все legacy-события обязаны быть прочитаны");
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// МАТЕРИАЛИЗАЦИЯ ДЛЯ SHELL-ПРОБЫ.
//
// `scripts/tests/red_restore_drill.sh` не умеет писать postcard-фреймы и zstd-потоки, и не
// должен: строитель обязан быть один. Проба зовёт этот тест с `DRILL_FIXTURE_OUT=<каталог>`
// и вариантом в `DRILL_FIXTURE_VARIANT`, а затем проверяет, что каталог наполнился.
//
// Без переменной тест НИЧЕГО не строит и НИЧЕГО не утверждает — это осознанно: он гоняется
// в общем `cargo test --all`, где строить некуда. Страж от вакуума стоит НА СТОРОНЕ ПРОБЫ:
// она обязана убедиться, что файлы появились, и объявить SETUP НЕ СОСТОЯЛСЯ, если нет.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn materialize_for_shell_probe() {
    let Ok(out) = std::env::var("DRILL_FIXTURE_OUT") else {
        eprintln!(
            "materialize_for_shell_probe: DRILL_FIXTURE_OUT не задан — материализация не запрошена"
        );
        return;
    };
    let variant = std::env::var("DRILL_FIXTURE_VARIANT").unwrap_or_else(|_| "healthy".to_string());
    // Одноразовость ПРОГОНА (задача 6b). Проба обязана знать значение, чтобы предъявить
    // `Р-4` (а): нонс не встречается ни в одном артефакте, доступном обёртке ДО чтения.
    // Он печатается в stdout строителя, а НЕ кладётся в каталог фикстуры — иначе обёртка
    // прочла бы его оттуда, и признак перестал бы различать миры.
    let nonce = drill_nonce();
    let out = PathBuf::from(out);
    fs::create_dir_all(&out).expect("mkdir out");

    let cold = out.join("cold");
    match variant.as_str() {
        "empty" => {
            // Пустое восстановление: sidecar'ы есть, сегментов нет.
            fs::create_dir_all(&cold).expect("mkdir cold");
            fs::write(cold.join(journal::LEGACY_MANIFEST), empty_manifest_bytes())
                .expect("манифест");
        }
        "healthy" | "corrupt" => {
            let stage = build_staging(&out, nonce);
            build_cold(&stage, &cold);
            if variant == "corrupt" {
                corrupt_one_compacted(&cold);
            }
            // staging больше не нужен — он не часть холодной копии.
            fs::remove_dir_all(&stage).expect("убрать staging");
        }
        other => panic!("неизвестный DRILL_FIXTURE_VARIANT={other}"),
    }

    fs::create_dir_all(out.join("restore")).expect("mkdir restore");
    fs::create_dir_all(out.join("state")).expect("mkdir state");
    println!(
        "DRILL_FIXTURE_READY variant={variant} nonce={nonce} cold={}",
        cold.display()
    );
}
