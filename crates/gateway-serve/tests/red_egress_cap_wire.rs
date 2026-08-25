//! RED `PL-I-5` УРОВЕНЬ 2 (sacred, architect-only) — **ПРЕДЕЛ СУДИТ ПОЛНЫЙ ИСХОДЯЩИЙ ТЕКСТ.**
//!
//! Милестоун `milestones/M-71-egress-cap.md` rev3, исполнение решения арбитра
//! `research/arbitration/A-021-m71-egress-resource-boundary.md` (Вопрос 2, Вопрос 3 Правка A).
//!
//! # Почему уровень ДВА и почему он в ЭТОМ крейте
//!
//! Два круга гейта (`C-157`, `C-158`) нашли экземпляры ОДНОГО класса: измеряемая величина
//! оказывалась ПОДМНОЖЕСТВОМ настоящего ресурса. `heatmap` ⊂ `Snapshot` ⊂ wire-сообщение.
//! Арбитр запретил третий экземпляр покрытия и потребовал смены конструкции: **два
//! инварианта, у каждого свой объект и свой крейт.**
//!
//! * **Уровень 1** (`crates/gateway/tests/red_egress_cap.rs`) — байты СОБСТВЕННЫХ объектов
//!   `gateway` (`Snapshot`, `Frame`). Там живёт анти-байпас: шесть строителей плюс прямые
//!   сборщики `Selector` (чекпоинтер M-38b, shared-tailer M-39, `research-cli`, replay).
//! * **Уровень 2 — здесь** — ПОЛНЫЙ исходящий текст сообщения, в ОБЕИХ wire-формах.
//!
//! Почему не одним оракулом в `gateway`: `ServeMsg` и v1-конверт живут в `gateway-serve`, а
//! зависимость направлена `gateway-serve → gateway` (`crates/gateway-serve/Cargo.toml:29`;
//! обратной нет). Арбитр проверил, что dev-dependency-цикл технически возможен, и **отверг
//! его**: он сделал бы sacred-набор нижнего слоя компилируемо зависимым от его потребителя,
//! то есть инвертировал бы в тестах ровно то слоение, ради которого анти-байпас и сажают вниз.
//! Прецедент `M-69` устроен так же и правильно: библиотечный оракул — в `gateway/tests`,
//! транспортный — в `gateway-serve/tests`.
//!
//! # Две wire-формы, а не одна — поправка арбитра к `C-158`
//!
//! `C-158` предписал мерить `ServeMsg`. Это конверт **только legacy-пути**
//! (`crates/gateway-serve/src/lib.rs:1401`, `:1719`). Путь **v1** — единственный, которым
//! клиентский селектор вообще попадает в систему (`subscribe`, `CT-RFC-09` §2.2), — шлёт
//! другой конверт: `wire_v1::{snapshot_msg,frame_msg}`, `{"type","v","sub","data"}`
//! (`:837`, `:923`, `:1270`, `:1613`). Исполнить `C-158` буквально значило бы получить
//! законную находку следующего круга. Обе формы судятся здесь.
//!
//! # Именованный ОСТАТОК: длина клиентского `sub`-id
//!
//! v1-конверт echo'ит `sub`-id клиента, и его длина сегодня не ограничена ничем
//! (`grep 'id.len\|MAX_ID' crates/gateway-serve/src/` → 0). Накладные v1 поэтому НЕ
//! константа: ≥45 Б + `|sub|`. Внутри `M-71` это закрыто тем, что уровень 2 судит ПОЛНЫЙ
//! текст — echo попадает внутрь судимой величины. Ограничение самой длины id — правка
//! протокола (`CT-RFC-09` §2.2), **отдельный маршрут, не предмет `M-71`** (`A-021`, Владельцы).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use gateway_serve::serve;
use gateway_serve::wire::ServeMsg;
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Плотный НЕ-heatmap сценарий: ровно тот ресурс, который `C-158` R1 предъявил на непокрытых
/// формах как 2 804 666 Б. Ни одного L2-события — heatmap и COB пусты.
const DENSE_TRADES: usize = 25_000;
/// Честная нагрузка — прод-дефолт `GATEWAY_BANDS` (`docker-compose.yml:134,203`).
const PROD_BAND: f64 = 0.001;
/// Короткий фиксированный `sub`-id — для оракула-связки: он пиннит ПОСТОЯНСТВО ФОРМЫ
/// конверта, а не глобальную константу накладных (её не существует, см. остаток выше).
const SHORT_SUB: &str = "s1";

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "PL-I-5 wire-level fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![PROD_BAND],
        window_ms: None,
    }
}

fn journal_of_trades(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
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
        .expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn bytes_of<T: serde::Serialize>(v: &T) -> usize {
    serde_json::to_vec(v)
        .expect("сообщение сериализуемо — иначе оно не ушло бы клиенту")
        .len()
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// КОНТРОЛИ — идут первыми: страж, ломающий честную работу, будет выключен, и защиты не станет
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **W-C1 — честная нагрузка проходит ОБЕ wire-формы.**
#[test]
fn pl_i_5_w_c1_prod_default_passes_both_wire_forms() {
    let dir = journal_of_trades(200);
    let (msg, _stats) = serve::snapshot_msg(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
        None,
    )
    .expect("PL-I-5 W-C1: обычная нагрузка обязана проходить serve-адаптер");

    let legacy = bytes_of(&msg);
    let ServeMsg::Snapshot(ref snap) = msg else {
        panic!("W-C1 SETUP: serve::snapshot_msg вернул не Snapshot-конверт")
    };
    let v1 = bytes_of(&gateway_serve::wire_v1::snapshot_msg(SHORT_SUB, snap));

    assert!(
        legacy < 200_000 && v1 < 200_000,
        "PL-I-5 W-C1 SETUP НЕ СОСТОЯЛСЯ: честный ответ весит legacy={legacy} v1={v1} Б — \
         фикстура не разводит честный и плотный случаи на порядки, и оракулы ниже начинают \
         зависеть от точной величины предела (её назначает founder, спека §5.1)"
    );
}

/// **W-C2 (оракул-связка, `A-021` Правка A) — накладные конверта при ФИКСИРОВАННОМ коротком
/// id постоянны и малы; глобальной константой они НЕ являются.**
///
/// Это ровно та правка, которой арбитр исправил моё предложение. Я утверждал «накладные
/// конверта ограничены константой, поэтому предел на внутреннем объекте влечёт предел на
/// внешнем». Для legacy это верно (13–15 Б), для **v1 — ЛОЖНО**: 45 Б + `|sub|`, а длина
/// `sub` не ограничена ничем. Поэтому связка пиннит ПОСТОЯНСТВО ФОРМЫ при фиксированном id,
/// а fail-closed обеспечивается тем, что уровень 2 судит ПОЛНЫЙ текст, — не этой оценкой.
#[test]
fn pl_i_5_w_c2_envelope_overhead_is_bounded_only_at_fixed_id() {
    let dir = journal_of_trades(200);
    let (msg, _) = serve::snapshot_msg(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
        None,
    )
    .expect("snapshot_msg");
    let ServeMsg::Snapshot(ref snap) = msg else {
        panic!("W-C2 SETUP: не Snapshot-конверт")
    };

    let bare = bytes_of(snap);
    let legacy = bytes_of(&msg);
    let v1_short = bytes_of(&gateway_serve::wire_v1::snapshot_msg(SHORT_SUB, snap));
    let long_id = "x".repeat(10_000);
    let v1_long = bytes_of(&gateway_serve::wire_v1::snapshot_msg(&long_id, snap));

    assert!(
        legacy > bare && legacy - bare < 64,
        "PL-I-5 W-C2: накладные legacy-конверта {} Б — форма изменилась",
        legacy - bare
    );
    assert!(
        v1_short > bare && v1_short - bare < 128,
        "PL-I-5 W-C2: накладные v1-конверта при коротком id {} Б — форма изменилась",
        v1_short - bare
    );
    // ГЛАВНОЕ утверждение оракула: накладные v1 РАСТУТ с длиной клиентского id, то есть
    // константой не ограничены. Пока это так, «предел на внутреннем объекте» НЕ влечёт
    // «предел на исходящем тексте», и судить обязан уровень 2.
    assert!(
        v1_long - v1_short >= 9_000,
        "PL-I-5 W-C2: длинный `sub`-id не увеличил конверт ({} Б против {} Б). Если echo id \
         перестал попадать в сообщение или его длина где-то ограничена — предпосылка \
         конструкции изменилась, и спеку M-71 §0 надо перечитать, а не чинить этот ассерт.",
        v1_long,
        v1_short
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// ПРЕДМЕТ
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **W1 — плотный ответ отвергается на serve-адаптере `snapshot_msg`.**
///
/// `C-158` R1 дословно: тот же ресурс достигает форм, которых уровень 1 не покрывал, на
/// 2 804 666 Б — 1.402× предложенного предела 2 МБ. Здесь он судится в точке, откуда уходит
/// на провод.
#[test]
fn pl_i_5_w1_dense_response_is_refused_at_serve_snapshot() {
    let dir = journal_of_trades(DENSE_TRADES);
    match serve::snapshot_msg(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
        None,
    ) {
        Err(_) => {}
        Ok((msg, _)) => panic!(
            "PL-I-5 W1 НАРУШЕН: serve::snapshot_msg отдал {} Б исходящего текста при пустом \
             heatmap. Селектор прод-дефолтный — злоупотребления шириной полосы не требуется. \
             Это точка, из которой сообщение уходит клиенту (`lib.rs:1401`).",
            bytes_of(&msg)
        ),
    }
}

/// **W2 — плотный ответ отвергается на serve-адаптере `frames_msgs`.**
///
/// Push-путь: им клиент живёт после первого снапшота. Отдельная дверь от `snapshot_msg`, и
/// `C-158` предъявил ресурс именно на ней.
#[test]
fn pl_i_5_w2_dense_frames_are_refused_at_serve_frames() {
    let dir = journal_of_trades(DENSE_TRADES);
    match serve::frames_msgs(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::START,
        usize::MAX,
    ) {
        Err(_) => {}
        Ok((msgs, _)) => {
            let worst = msgs.iter().map(bytes_of).max().unwrap_or(0);
            // v1-КОНВЕРТ КАДРА — отдельная дверь, и её отсутствие в оракуле нашла
            // проба-инвентаризация `scripts/tests/red_egress_doors.sh`, а не следующий круг
            // критика. Ровно ради этого конструкция и менялась (`A-021` Правка B).
            let worst_v1 = msgs
                .iter()
                .filter_map(|m| match m {
                    ServeMsg::Frame(f) => {
                        Some(bytes_of(&gateway_serve::wire_v1::frame_msg(SHORT_SUB, f)))
                    }
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            panic!(
                "PL-I-5 W2 НАРУШЕН: serve::frames_msgs отдал {} кадров, крупнейший — {worst} Б. \
                 Кадр уходит на провод целиком (`lib.rs:1270`/`:1613` в v1-конверте, `:1719` в \
                 legacy). В v1-конверте тот же кадр весит {worst_v1} Б. Предел, поставленный \
                 только на снапшот, оставляет открытым именно тот путь, которым идёт основной \
                 трафик.",
                msgs.len()
            )
        }
    }
}

/// **W3 — v1-конверт судится тем же пределом, что legacy.**
///
/// Путь v1 — единственный, которым клиентский селектор попадает в систему. Оракул строит
/// плотный ответ и предъявляет, что в v1-форме он тоже уходит целиком: реализация,
/// закрывшая legacy и забывшая v1, красна здесь.
#[test]
fn pl_i_5_w3_v1_envelope_is_capped_too() {
    let dir = journal_of_trades(DENSE_TRADES);
    // Если уровень 1/serve уже отвергли — предмет закрыт, оракул зелен: судить нечего.
    let Ok((msg, _)) = serve::snapshot_msg(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
        None,
    ) else {
        return;
    };
    let ServeMsg::Snapshot(ref snap) = msg else {
        panic!("W3 SETUP: не Snapshot-конверт")
    };
    let v1 = gateway_serve::wire_v1::snapshot_msg(SHORT_SUB, snap);
    panic!(
        "PL-I-5 W3 НАРУШЕН: v1-конверт построен и весит {} Б (legacy — {} Б). Обе формы уходят \
         на провод и обе обязаны судиться одним пределом; `C-158` предписывал мерить только \
         `ServeMsg`, и это конверт ЛИШЬ legacy-пути (`A-021` Вопрос 1).",
        bytes_of(&v1),
        bytes_of(&msg)
    );
}
