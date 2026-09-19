//! RED `M-86` `V1` (sacred, architect-only) — **РАЗМЕР СОБРАННОГО КАДРА НА ПРОД-ФОРМЕ
//! ЖУРНАЛА УКЛАДЫВАЕТСЯ В ПОДПИСАННЫЙ ПРЕДЕЛ.**
//!
//! Милестоун `milestones/M-86-vp-bin-width.md`, задача 2, оракул `V1`.
//!
//! ## Почему этот оракул живёт ОТДЕЛЬНЫМ файлом
//!
//! Он поднимает ПРОЦЕССНЫЙ предел ответа (`set_effective_max_response_bytes`), а тестовый
//! бинарь гоняет свои тесты в потоках параллельно: соседи в том же файле увидели бы чужой
//! предел. Один тест — один бинарь — гонки нет. Тот же приём, что у `M-71`
//! (`red_egress_cap_governed.rs`).
//!
//! ## ПОЧЕМУ ПРИЗНАК «ВЕРНУЛСЯ `Err`» ЗДЕСЬ ЗАПРЕЩЁН
//!
//! Это центральное требование спеки, и оно выведено из уже оплаченной ошибки. Сегодняшний
//! мир — БЕЗ исправления — тоже отдаёт `Err`: его отдаёт предел `M-71`, впустую, потому что
//! кадр неподъёмен. Оракул, проверяющий «пришёл `Err`», был бы ЗЕЛЁН и до фикса, и после, а
//! значит не пиннил бы ничего. Та же ловушка названа в `TD-198`.
//!
//! Поэтому здесь предел ПОДНИМАЕТСЯ до заведомо недостижимого, кадр собирается ЦЕЛИКОМ, и
//! судится его ДЛИНА В БАЙТАХ против `DEFAULT_MAX_RESPONSE_BYTES`. Мерится ресурс, а не
//! прокси и не исход (`testing.md` §«Оракул границы ресурса меряет ресурс, а не прокси»).
//!
//! ## Форма прода СНЯТА ЗАМЕРОМ, а не воображена
//!
//! `docs/plans/vp-frame-size-measurement-2026-09-18.md` §4, прод 2026-09-18T21:05Z:
//! 211 381 различная цена в ОДНОЙ UTC-сессии, шаг цен 0.01 USD (тик Binance), ход
//! 5 104 USD, заполнение тиковой сетки 41 %, профиль — 87.41 % кадра (4 836 478 Б),
//! 22.88 Б на корзину. Фикстура воспроизводит ИМЕННО эту геометрию, включая НЕПОЛНОЕ
//! заполнение: сплошная сетка дала бы другую плотность и проверяла бы не ту форму.
//!
//! ## COMPILE-RED → RUNTIME-RED
//!
//! Сегодня файл не компилируется (`DEFAULT_VP_BIN_WIDTH_E8` не существует). После того как
//! dev введёт константу, но ДО того как применит сетку, тест обязан падать ПО ДЛИНЕ —
//! и это его настоящее красное состояние.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

/// Геометрия прод-сессии (замер 2026-09-18).
const TICK_E8: i64 = 1_000_000; // 0.01 USD
const LOW_E8: i64 = 76_296 * 100_000_000; // нижняя цена сессии
const TICKS_IN_RANGE: i64 = 510_401; // ход 5 104.00 USD в тиках
/// Заполнение 41 %: берём 2 тика из каждых 5. Даёт ≈204 160 различных цен — выше порога
/// «прод-масштаб» (≥ 200 000), при этом сетка НЕ сплошная, как и на проде.
const KEEP_OF_5: i64 = 2;
const MIN_DISTINCT_PRICES: usize = 200_000;

const T: i64 = 1_752_000_010_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000), // прод-окно
        depth_cadence_ms: Some(1_000),
    }
}

/// **V1.** Кадр, собранный на прод-форме журнала, укладывается в подписанный предел.
#[test]
fn v1_production_shaped_frame_fits_signed_limit() {
    // 1. Снимаем предел, чтобы кадр СОБРАЛСЯ целиком и его можно было измерить.
    //    Без этого `snapshot` вернул бы `Err` и оракул судил бы исход вместо ресурса.
    let huge = 1_usize << 40;
    gateway::set_effective_max_response_bytes(huge);

    // 2. Строим журнал прод-геометрии.
    let dir = tempfile::tempdir().expect("tempdir");
    let mut distinct = 0_usize;
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..TICKS_IN_RANGE {
            if i % 5 >= KEEP_OF_5 {
                continue; // пропуски — прод заполняет лишь 41 % тиковой сетки
            }
            let price = LOW_E8 + i * TICK_E8;
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price,
                    size: to_fixed(0.001),
                    side: Side::Buy,
                    ts_exch_ms: T, // одна секунда ⇒ одна UTC-сессия, окно ничего не режет
                },
            ))
            .expect("append");
            distinct += 1;
        }
        j.flush().expect("flush");
    }

    // 3. SETUP-СТРАЖ: фикстура обязана быть прод-масштабной. Проба, молча тестирующая
    //    мелкий журнал, есть плацебо самой себя (`testing.md` §«Целостность гейта» св. 3).
    assert!(
        distinct >= MIN_DISTINCT_PRICES,
        "SETUP НЕ СОСТОЯЛСЯ: фикстура дала {distinct} различных цен при пороге \
         {MIN_DISTINCT_PRICES} — это не прод-масштаб, и вывод о размере недействителен"
    );

    let s = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("snapshot обязан собраться: предел поднят");

    // 4. Второй SETUP-СТРАЖ: профиль обязан быть НЕПУСТ и односессионен, иначе мы меряем
    //    размер пустоты.
    assert_eq!(
        s.series.volume_profile.len(),
        1,
        "SETUP НЕ СОСТОЯЛСЯ: ожидалась одна сессия, получено {}",
        s.series.volume_profile.len()
    );
    let bins = s.series.volume_profile[0].bins.len();
    assert!(
        bins > 0,
        "SETUP НЕ СОСТОЯЛСЯ: профиль пуст — сделки не долетели до редьюсера"
    );

    // 5. ПРЕДМЕТ: длина сериализованного кадра против ПОДПИСАННОГО предела.
    let bytes = serde_json::to_vec(&s).expect("serialize snapshot").len();
    let limit = gateway::DEFAULT_MAX_RESPONSE_BYTES;

    assert!(
        bytes <= limit,
        "V1 НАРУШЕН: кадр прод-формы весит {bytes} Б при подписанном пределе {limit} Б \
         ({:.2}×). Корзин в профиле: {bins} при {distinct} различных ценах на входе. \
         Ширина корзины {} e8. Если корзин ≈ числу цен — сетка не применена вовсе; \
         если корзин мало, а кадр велик — раздулась другая серия.",
        bytes as f64 / limit as f64,
        gateway::DEFAULT_VP_BIN_WIDTH_E8
    );

    // 6. Анти-плацебо в обратную сторону: кадр не смеет оказаться подозрительно пустым.
    //    Заглушка «отдавать пустой профиль» проходит проверку размера и убивает продукт.
    assert!(
        bins >= 1_000,
        "V1 НАРУШЕН (обратная сторона): в профиле всего {bins} корзин при {distinct} ценах \
         на входе — данные уничтожены, а не огрублены"
    );
}
