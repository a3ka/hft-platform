//! RED `TD-180` (sacred, architect-only) — **СНИМОК ОБЯЗАН ОБЪЯВЛЯТЬ ПОЗИЦИЮ СОСТОЯНИЯ,
//! А НЕ ЗАКЛАДКУ ДОСТАВКИ.**
//!
//! Милестоун `milestones/M-72-subscription-terminality.md`, задача 6; форма решена там же
//! (§«Форма задачи 6»). Источник — `TECH-DEBT.md` `TD-180`, заведено reviewer'ом по
//! `R-146` `N-4`.
//!
//! # Предмет
//!
//! У `LiveReducer` ДВЕ позиции, и это объявлено самим кодом (`crates/gateway/src/lib.rs:3249`):
//!
//! | поле | смысл |
//! |---|---|
//! | `cursor` | до какого seq кадр УЖЕ ОТДАН потребителю (закладка ДОСТАВКИ) |
//! | `full_applied_seq` | до какого seq СОСТОЯНИЕ уже свёрнуто |
//!
//! Инвариант `full_applied_seq >= cursor.upto_seq`; равенство — установившийся режим.
//! Строгое неравенство — окно «батч свёрнут в состояние, но отвергнут пределом и потому не
//! отдан»: `self.cursor = cursor` в `pump` исполняется ТОЛЬКО после успешной проверки предела.
//!
//! `snapshot()` берёт `series` из `self.full` (состояние), а `cursor` кладёт `self.cursor`
//! (доставку). В окне расхождения снимок внутренне несогласован.
//!
//! # Чем это вредит потребителю
//!
//! Клиент, получив `snapshot(C)`, подписывается «с C». Если `C` — закладка ДОСТАВКИ, то
//! отвергнутый батч УЖЕ лежит в сериях снимка И придёт ещё раз ⇒ двойное применение
//! (`GW-I-4`). Если `C` — позиция СОСТОЯНИЯ, повторно он не придёт.
//!
//! # Почему мера снимается ТАК, а не по приватному полю (`Р-1`)
//!
//! `full_applied_seq` приватно, и лезть к нему оракулу НЕ НАДО: правило `Р-1` разбора класса
//! (`docs/workflow/oracle-blindness-class-2026-08-28.md`) требует снимать величину ТАМ, ГДЕ
//! ЕЁ ВИДИТ ПОТРЕБИТЕЛЬ. Потребитель видит только публичное: `cursor()` и `snapshot()`.
//! Эталон «до какого seq состояние свёрнуто» берётся из НЕЗАВИСИМОГО прогона — того же
//! журнала при заведомо достаточном пределе (`S-1`), где обе позиции обязаны совпасть.
//! Оракул `X == Y`, где `Y` вычисляется через `X`, невалиден (`testing.md`, мутационный
//! контроль); здесь `Y` приходит из другого прогона другого редьюсера.
//!
//! # Анти-плацебо в обе стороны
//!
//! `S-1` — позитивный контроль: без него `S-2` был бы зелен и против фикстуры, которая
//! вообще ничего не свернула. `S-2` несёт СТРАЖ SETUP'а: если окно расхождения не
//! воспроизвелось (закладка доставки не отстала), тест объявляет несостоявшийся setup, а не
//! выносит вердикт — проба, молча судящая не тот сценарий, есть плацебо самой себя
//! (`testing.md`, целостность гейта, свойство 3).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{LiveReducer, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Заведомо СВЕРХ предела `TINY` ниже и заведомо ПОД дефолтным пределом.
const TRADES: usize = 25_000;

/// Размер батча ≥ числа событий: весь журнал закрывается ОДНИМ батчем. Это условие
/// достижимости предмета, а не украшение: при мелком батче состояние ушло бы вперёд лишь
/// на один батч, и «позиция состояния» перестала бы совпадать с хвостом журнала — эталон
/// `S-1` стал бы неприменим, а не «менее точен».
const ONE_BATCH: usize = TRADES * 4;

/// Предел, при котором батч из `TRADES` сделок ОТВЕРГАЕТСЯ. `M-71` §0: 25 000 сделок дают
/// 2 804 765 Б; 20 000 Б отвергают их с запасом.
const TINY: usize = 20_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "TD-180 snapshot cursor honesty".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
    }
}

fn journal_of_trades(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("SETUP: tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("SETUP: open_with");
    for i in 0..n as i64 {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID + i as f64 * 0.01),
                size: to_fixed(1.0),
                side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                ts_exch_ms: T0 + i,
            },
        ))
        .expect("SETUP: append");
    }
    j.flush().expect("SETUP: flush");
    dir
}

/// Предел — ПРОЦЕССНАЯ величина, и её обязан вернуть даже упавший тест: иначе соседний
/// оракул в том же бинаре судит чужой предел. Возврат через `Drop`, а не последней строкой
/// (M-72, `d8c7654` — тот же приём, что в `red_ws_terminality_entrypoint.rs`).
struct CapGuard;
impl Drop for CapGuard {
    fn drop(&mut self) {
        gateway::set_effective_max_response_bytes(usize::MAX);
    }
}

/// Замер идёт по ГЛОБАЛЬНОЙ процессной величине, поэтому сценарии обязаны быть
/// single-threaded по замеру (`testing.md`, целостность гейта, свойство 2).
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn setup_failed(what: &str) -> ! {
    panic!(
        "SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о честности курсора: фикстура не \
         воспроизвела сценарий, ради которого оракул написан."
    )
}

/// Прогнать журнал при ЗАВЕДОМО ДОСТАТОЧНОМ пределе и вернуть позицию, до которой
/// состояние свёрнуто. В установившемся режиме обе позиции совпадают, поэтому публичный
/// `cursor()` здесь И ЕСТЬ позиция состояния — но снят он ДРУГИМ редьюсером на ДРУГОМ
/// прогоне, то есть эталон независим от предмета `S-2`.
fn state_frontier_of(dir: &std::path::Path) -> Option<u64> {
    let ckpt = tempfile::tempdir().expect("SETUP: ckpt tempdir");
    gateway::set_effective_max_response_bytes(usize::MAX);
    let (mut r, _) = LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
        .unwrap_or_else(|e| setup_failed(&format!("resume эталона не собрался: {e}")));
    loop {
        match r.pump(dir, EpochFilter::OwnCaptureOnly, ONE_BATCH) {
            Ok((frames, _, _)) if frames.is_empty() => break,
            Ok(_) => continue,
            Err(e) => setup_failed(&format!(
                "эталонный прогон при пределе usize::MAX отказал: {e} — предел протёк из \
                 соседнего сценария либо отказ не по объёму"
            )),
        }
    }
    r.cursor().upto_seq
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// S-1 — ПОЗИТИВНЫЙ КОНТРОЛЬ: в установившемся режиме снимок и закладка совпадают
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// Без этой половины `S-2` зелен и против фикстуры, которая ничего не свернула: «курсор
/// снимка равен позиции состояния» верно и когда обе величины `None`.
#[test]
fn td_180_s1_steady_state_snapshot_cursor_equals_delivery_bookmark() {
    let _g = serial();
    let _cap = CapGuard;
    let dir = journal_of_trades(TRADES);

    let frontier = state_frontier_of(dir.path());
    if frontier.is_none() {
        setup_failed("эталонный прогон не свернул НИ ОДНОГО события — журнал пуст либо селектор мимо");
    }

    gateway::set_effective_max_response_bytes(usize::MAX);
    let ckpt = tempfile::tempdir().expect("SETUP: ckpt tempdir");
    let (mut r, _) =
        LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .unwrap_or_else(|e| setup_failed(&format!("resume не собрался: {e}")));
    loop {
        match r.pump(dir.path(), EpochFilter::OwnCaptureOnly, ONE_BATCH) {
            Ok((frames, _, _)) if frames.is_empty() => break,
            Ok(_) => continue,
            Err(e) => setup_failed(&format!("pump при пределе usize::MAX отказал: {e}")),
        }
    }

    assert_eq!(
        r.cursor().upto_seq,
        frontier,
        "TD-180 S-1: в установившемся режиме закладка доставки обязана совпасть с позицией \
         состояния (эталон независимого прогона = {frontier:?}), а она {:?}. Расходятся они \
         только в окне отвергнутого батча — здесь предел снят, окна быть не должно",
        r.cursor().upto_seq
    );
    assert_eq!(
        r.snapshot().cursor.upto_seq,
        frontier,
        "TD-180 S-1: снимок в установившемся режиме обязан нести ту же позицию. Если он её \
         не несёт при СОВПАДАЮЩИХ величинах — дефект не в TD-180, а в самом строителе снимка"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// S-2 — ПРЕДМЕТ: в окне расхождения снимок обязан объявить позицию СОСТОЯНИЯ
// ═══════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn td_180_s2_snapshot_declares_state_position_not_delivery_bookmark() {
    let _g = serial();
    let _cap = CapGuard;
    let dir = journal_of_trades(TRADES);

    // Эталон снимается ПЕРВЫМ и при снятом пределе — независимым прогоном.
    let frontier = state_frontier_of(dir.path());
    let frontier = frontier.unwrap_or_else(|| {
        setup_failed("эталонный прогон не свернул ни одного события — сравнивать не с чем")
    });

    // Теперь — предмет: тот же журнал, предел ОТВЕРГАЕТ батч целиком.
    gateway::set_effective_max_response_bytes(TINY);
    let ckpt = tempfile::tempdir().expect("SETUP: ckpt tempdir");
    let (mut r, _) =
        LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .unwrap_or_else(|e| setup_failed(&format!("resume не собрался: {e}")));

    let refusal = match r.pump(dir.path(), EpochFilter::OwnCaptureOnly, ONE_BATCH) {
        Err(e) => e.to_string(),
        Ok((frames, _, _)) => setup_failed(&format!(
            "pump НЕ отказал при пределе {TINY} Б — батч из {TRADES} сделок прошёл \
             ({} кадров). Предмет (окно «свёрнуто, но не отдано») не воспроизведён",
            frames.len()
        )),
    };

    // ── СТРАЖ SETUP'А: окно расхождения обязано БЫТЬ. ────────────────────────────────────
    // Закладка доставки обязана отстать от позиции состояния — иначе судить нечего, и
    // ассерт ниже был бы зелен по отсутствию предмета.
    let bookmark = r.cursor().upto_seq;
    if bookmark == Some(frontier) {
        setup_failed(&format!(
            "закладка доставки НЕ отстала (обе позиции = {frontier}) — окно расхождения не \
             воспроизведено, хотя pump отказал ({refusal})"
        ));
    }

    // ── ПРЕДМЕТ ─────────────────────────────────────────────────────────────────────────
    let snap_cursor = r.snapshot().cursor.upto_seq;
    assert_eq!(
        snap_cursor,
        Some(frontier),
        "TD-180 НАРУШЕН: батч свёрнут в состояние (позиция состояния = {frontier}), отвергнут \
         пределом и потому НЕ ОТДАН (закладка доставки = {bookmark:?}, отказ: {refusal}). \
         `snapshot()` обязан объявить позицию СОСТОЯНИЯ, потому что серии он берёт ИМЕННО \
         ОТТУДА, — а объявил {snap_cursor:?}. Клиент, подписавшийся «с {snap_cursor:?}», \
         получит уже лежащий в снимке батч ВТОРОЙ РАЗ: двойное применение, GW-I-4. \
         Форма решена в milestones/M-72-subscription-terminality.md §«Форма задачи 6»: \
         курсор снимка строится из `full_applied_seq`, сигнатура и схема не меняются"
    );
}
