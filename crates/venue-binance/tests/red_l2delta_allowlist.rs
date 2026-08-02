//! RED M-45 (sacred, architect-only): allow-list эмиссии `L2Delta` — из хардкод-константы
//! в КОНФИГ, с дефолтом, равным сегодняшнему поведению.
//!
//! # Зачем это milestone, а не однострочная правка
//!
//! Сегодня набор символов — `const L2DELTA_CAPTURE_SYMBOLS: &[&str] = &["BTCUSDT"]`, и
//! doc-comment рядом с ним говорит прямо: «расширение набора требует отдельного решения
//! founder'а». То есть правка константы = совершение решения Границы C **кодом**, а merge
//! такого кода = раскатка. M-45 разрывает эту сцепку: состав эмиссии становится
//! конфигурацией, дефолт сохраняет прод байт-в-байт, а включение — операторский шаг
//! (env + новый `EPOCH_ID` + рестарт), не коммит.
//!
//! Обоснование объёма — `docs/rfc/CT-RFC-06-l2delta.md` §8.1 (critic `C-045` PASS,
//! reviewer `R-019`/`R-021`). Вариант `L2Delta` УЖЕ в T1 с `CT-RFC-04`/M-18 ⇒ contract-пакет
//! не собирается, `SCHEMA_VERSION` не бампается, карта из пяти exhaustive-`match` (§8.2) к
//! этому milestone'у НЕ применяется: нового варианта не вводится, E0004 не возникает.
//!
//! # API-контракт, который обязана предоставить реализация (venue-dev)
//!
//! ```ignore
//! /// Разбор allow-list из СЫРОЙ строки конфигурации. ЧИСТАЯ функция.
//! pub fn parse_capture_symbols(raw: Option<&str>) -> Vec<String>;
//!
//! /// Решение об эмиссии: символ из wire против разобранного списка.
//! pub fn should_capture_l2delta(symbols: &[String], symbol: &str) -> bool;
//! ```
//!
//! **Почему разбор обязан быть ЧИСТОЙ функцией, а не чтением `env` внутри:** переменные
//! окружения — глобальное состояние процесса, а `cargo test` гоняет тесты в потоках
//! параллельно. Оракул, дёргающий `set_var`, стал бы гонкой и дал бы разный результат при
//! разном порядке запуска — прямое нарушение принципа детерминизма (`CLAUDE.md`: в доменном
//! коде нет недетерминизма). Поэтому проверяется чистое ядро, а чтение `env` остаётся
//! тонкой обёрткой над ним.
//!
//! # Анти-плацебо
//!
//! Реализация «эмитить всегда» проходит позитивные проверки и падает на `o2_*`.
//! Реализация «вернуть дефолт на любой вход» проходит `o3_default_*` и падает на `o1_*`.
//! Реализация без нормализации регистра проходит всё, кроме `o4_*` — а именно она
//! молча выключила бы запись BTC в проде (см. ниже).

use venue_binance::{parse_capture_symbols, should_capture_l2delta};

/// Сегодняшнее прод-поведение, зафиксированное как константа ОРАКУЛА (а не импорт из
/// реализации): если реализация изменит свой дефолт, тест обязан упасть, а не «согласиться».
const PROD_DEFAULT: &[&str] = &["BTCUSDT"];

// ─────────────────────────────────────────────────────────────────────────────────────────
// O-3 — ГРАНИЦЫ и главная гарантия merge'а: без конфига прод не меняется.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn o3_default_when_config_absent_equals_current_prod_behaviour() {
    let got = parse_capture_symbols(None);
    assert_eq!(
        got,
        PROD_DEFAULT.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        "БЕЗ конфигурации состав эмиссии обязан остаться ровно сегодняшним ({PROD_DEFAULT:?}). \
         Это условие, при котором merge M-45 не является раскаткой: прод после деплоя пишет \
         то же, что писал до него. Если этот тест красный — milestone нельзя мержить без \
         решения founder'а о составе символов (Граница C, docs/PENDING-SIGNATURE.md П-003)."
    );
}

#[test]
fn o3_empty_config_is_default_not_all_and_not_nothing() {
    // Пустая строка — самая вероятная операторская опечатка (`export VAR=`).
    // Два опасных прочтения, оба обязаны быть исключены:
    //   «пусто ⇒ эмитить ВСЁ»    — тихий взрыв объёма записи и необъявленная смена эпохи;
    //   «пусто ⇒ не эмитить НИЧЕГО» — тихая ПОТЕРЯ данных, forward-only и невосстановимая.
    for raw in ["", "   ", ",", " , , "] {
        let got = parse_capture_symbols(Some(raw));
        assert_eq!(
            got,
            PROD_DEFAULT.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "пустая/вырожденная конфигурация {raw:?} обязана означать ДЕФОЛТ, \
             не «эмитить всё» и не «эмитить ничего»"
        );
    }
}

#[test]
fn o3_garbage_elements_do_not_become_symbols() {
    // Мусорный элемент в середине списка (двойная запятая, пробелы) не имеет права
    // превратиться в «символ» — иначе allow-list молча получает пустой элемент, который
    // при небрежном сравнении матчит что угодно.
    let got = parse_capture_symbols(Some("BTCUSDT, ,ETHUSDT,"));
    assert_eq!(
        got,
        vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
        "разбор обязан отбросить пустые элементы и обрезать пробелы, сохранив порядок"
    );
    assert!(
        !got.iter().any(|s| s.is_empty()),
        "пустая строка не имеет права попасть в allow-list"
    );
}

#[test]
fn o3_single_element_and_whitespace_padding() {
    assert_eq!(parse_capture_symbols(Some("ETHUSDT")), vec!["ETHUSDT"]);
    assert_eq!(parse_capture_symbols(Some("  ETHUSDT  ")), vec!["ETHUSDT"]);
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// O-4 — РЕГИСТР. Не косметика: дефект-кандидат с необратимой ценой.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn o4_config_case_does_not_silently_disable_capture() {
    // Замер по коду (не предположение): символ из wire нормализуется в ВЕРХНИЙ регистр
    // (`crates/venue-binance/src/lib.rs`, `stream.split('@').next()?.to_uppercase()`),
    // а сравнение с allow-list сегодня ТОЧНОЕ (`.contains(&symbol.as_str())`).
    //
    // Значит оператор, написавший env в нижнем регистре, получил бы НЕ ошибку, а тишину:
    // эмиссия для BTC просто выключилась бы. Данные forward-only — потерянную
    // суб-секундную историю не восстановить никогда. Отказ обязан быть невозможен, а не
    // диагностируем.
    for raw in ["btcusdt", "BtcUsdt", "BTCUSDT"] {
        let syms = parse_capture_symbols(Some(raw));
        assert!(
            should_capture_l2delta(&syms, "BTCUSDT"),
            "конфигурация {raw:?} обязана включать эмиссию для wire-символа BTCUSDT; \
             без нормализации регистра запись молча выключается — тихая потеря данных"
        );
    }
}

#[test]
fn o4_normalization_does_not_make_everything_match() {
    // Обратная половина O-4: нормализация не имеет права выродиться в «матчит всё»
    // (например, если реализация начнёт сравнивать по подстроке или игнорировать длину).
    let syms = parse_capture_symbols(Some("btcusdt"));
    assert!(
        !should_capture_l2delta(&syms, "ETHUSDT"),
        "нормализация регистра не имеет права расширять allow-list"
    );
    assert!(
        !should_capture_l2delta(&syms, "BTC"),
        "префикс/подстрока не является совпадением символа"
    );
    assert!(
        !should_capture_l2delta(&syms, "BTCUSDT_PERP"),
        "символ, СОДЕРЖАЩИЙ разрешённый как подстроку, не разрешён"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// O-1 — МНОЖЕСТВЕННОСТЬ: список из нескольких символов, все действуют.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn o1_multiple_symbols_all_captured() {
    let syms = parse_capture_symbols(Some("BTCUSDT,ETHUSDT,SOLUSDT"));
    assert_eq!(syms.len(), 3, "разобраны все три символа");
    for s in ["BTCUSDT", "ETHUSDT", "SOLUSDT"] {
        assert!(
            should_capture_l2delta(&syms, s),
            "{s} есть в allow-list ⇒ обязан капчиться; реализация, учитывающая только \
             первый элемент списка, падает здесь"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// O-2 — НЕГАТИВ (главный анти-плацебо): вне списка — не капчить.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn o2_symbol_outside_allowlist_is_not_captured() {
    let syms = parse_capture_symbols(Some("BTCUSDT,ETHUSDT"));
    for s in ["SOLUSDT", "XRPUSDT", "DOGEUSDT"] {
        assert!(
            !should_capture_l2delta(&syms, s),
            "{s} НЕ в allow-list ⇒ эмиссии быть не должно. Без этой проверки реализация \
             «капчить всегда» проходит все позитивные тесты"
        );
    }
}

#[test]
fn o2_default_config_does_not_capture_beyond_btc() {
    // Прямое следствие o3_default_*, вынесено отдельно: именно это свойство §8-eyes-on
    // проверяет на живом проде после деплоя («в свежих событиях L2Delta только по BTC»).
    let syms = parse_capture_symbols(None);
    assert!(should_capture_l2delta(&syms, "BTCUSDT"));
    for s in ["ETHUSDT", "SOLUSDT"] {
        assert!(
            !should_capture_l2delta(&syms, s),
            "дефолтная конфигурация не имеет права расширять состав эмиссии на {s} — \
             это была бы раскатка без решения founder'а"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// O-6 — ДЕТЕРМИНИЗМ разбора (DET-I-1 распространяется на всё, что решает состав журнала).
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn o6_parse_is_deterministic_and_order_preserving() {
    let raw = Some("BTCUSDT, ethusdt ,SOLUSDT");
    let a = parse_capture_symbols(raw);
    let b = parse_capture_symbols(raw);
    let c = parse_capture_symbols(raw);
    assert_eq!(a, b, "повторный разбор той же строки обязан дать тот же результат");
    assert_eq!(a, c, "третий разбор разошёлся");
    assert_eq!(
        a.len(),
        3,
        "порядок и состав стабильны; никакой итерации по HashMap в решении о составе журнала"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// O-7 — ВЫЗОВ НА РЕАЛЬНОМ ПУТИ, а не только существование функций (C-048 REJECT).
//
// Находка critic'а C-048 §1: оракулов O-1..O-6 НЕДОСТАТОЧНО. Реализация может выполнить
// контракт `parse_capture_symbols`/`should_capture_l2delta` дословно, оставить их
// экспортированными — и НЕ подключить к реальной точке решения (обработчик WS-сообщения,
// `crates/venue-binance/src/lib.rs`, ветка `stream.contains("@depth")`). Достаточно
// переименовать константу или заинлайнить список литералом, и:
//   • O-1..O-6 зелёные   — они дёргают чистые функции напрямую, минуя обработчик;
//   • T5 зелёный         — греп искал БУКВАЛЬНОЕ имя `L2DELTA_CAPTURE_SYMBOLS`;
//   • §8 eyes-on зелёный — дефолт совпадает со старым поведением байт-в-байт.
// Дефект всплыл бы ТОЛЬКО когда founder подпишет состав и оператор выставит env —
// максимально поздно, после прохождения всех гейтов. Худший класс тихой деградации.
//
// Поэтому предмет проверки смещается: не «существуют ли функции», а «проходит ли РЕАЛЬНОЕ
// wire-сообщение через allow-list». Реализация обязана свести решение об эмиссии в ОДНУ
// чистую функцию, покрывающую весь путь (разбор stream/data → символ → allow-list → событие):
//
// ```ignore
// /// ЕДИНСТВЕННАЯ точка решения об эмиссии L2Delta. Чистая: без I/O, без env, без async.
// /// `Some(event)` ⇔ сообщение — валидный depth-diff И символ разрешён списком.
// pub fn l2delta_emission_for(
//     stream: &str,
//     data: &serde_json::Value,
//     symbols: &[String],
// ) -> Option<EventKind>;
// ```
//
// Обработчик обязан ДЕЛЕГИРОВАТЬ ей, а не дублировать сравнение символов у себя. Список
// приходит явным параметром — чтение `env` схлопывается в однострочную обёртку на самом
// верху (`connect`/`main`), проверяемую глазами. Отсутствие обходного пути закрепляет
// канарейка T5 в `scripts/verify_M-45.sh` (`l2delta_event(` вызывается в `src/` ровно
// один раз — внутри `l2delta_emission_for`), по образцу INTG-I: тест подтверждает
// ОТСУТСТВИЕ альтернативного пути, а не наличие проверки.
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Синтетическое wire-сообщение Binance `@depth` для произвольного символа.
/// Форма — как в проде: `data` содержит `U`/`u`/`b`/`a`, цены/размеры строками.
fn depth_data(sym_lower: &str) -> (String, serde_json::Value) {
    let stream = format!("{sym_lower}@depth@100ms");
    let data = serde_json::json!({
        "e": "depthUpdate",
        "E": 1_752_000_000_499i64,
        "s": sym_lower.to_uppercase(),
        "U": 101,
        "u": 103,
        // асимметрия: только биды; из них один — remove (size "0")
        "b": [["65000.5", "0.3"], ["65000.4", "0"]],
        "a": []
    });
    (stream, data)
}

#[test]
fn o7_allowed_symbol_emits_through_real_message_path() {
    let syms = parse_capture_symbols(Some("ETHUSDT"));
    let (stream, data) = depth_data("ethusdt");
    let got = venue_binance::l2delta_emission_for(&stream, &data, &syms);
    assert!(
        got.is_some(),
        "символ ETHUSDT разрешён конфигурацией ⇒ РЕАЛЬНЫЙ путь разбора wire-сообщения обязан \
         дать L2Delta-событие. Если здесь None — allow-list не подключён к обработчику, \
         и раскатка не заработает (C-048 §1)"
    );
}

#[test]
fn o7_disallowed_symbol_does_not_emit_through_real_message_path() {
    // Главная половина: конфигурация обязана УМЕТЬ не пускать. Реализация с захардкоженным
    // списком на реальном call site вернёт Some (BTC-хардкод) или Some всегда — и упадёт тут.
    let syms = parse_capture_symbols(Some("ETHUSDT"));
    let (stream, data) = depth_data("solusdt");
    assert!(
        venue_binance::l2delta_emission_for(&stream, &data, &syms).is_none(),
        "SOLUSDT НЕ в allow-list ⇒ на реальном пути эмиссии быть не должно"
    );
}

#[test]
fn o7_default_config_still_emits_btc_and_only_btc_on_real_path() {
    // Прод-инвариант merge'а, проверенный через реальный разбор, а не через чистый фильтр.
    let syms = parse_capture_symbols(None);

    let (s_btc, d_btc) = depth_data("btcusdt");
    assert!(
        venue_binance::l2delta_emission_for(&s_btc, &d_btc, &syms).is_some(),
        "дефолтная конфигурация обязана продолжать писать BTC — иначе merge ТЕРЯЕТ данные, \
         которые прод пишет с 2026-07-21 (forward-only, невосстановимо)"
    );
    for other in ["ethusdt", "solusdt"] {
        let (s, d) = depth_data(other);
        assert!(
            venue_binance::l2delta_emission_for(&s, &d, &syms).is_none(),
            "дефолт не имеет права расширять состав эмиссии на {other} — это раскатка без \
             решения founder'а (Граница C)"
        );
    }
}

#[test]
fn o7_case_insensitive_config_works_on_real_path() {
    // Регистр, проверенный СКВОЗЬ реальный разбор: wire даёт "ETHUSDT", конфиг — "ethusdt".
    let syms = parse_capture_symbols(Some("ethusdt"));
    let (stream, data) = depth_data("ethusdt");
    assert!(
        venue_binance::l2delta_emission_for(&stream, &data, &syms).is_some(),
        "конфигурация в нижнем регистре обязана работать на реальном пути — иначе запись \
         молча выключается при операторской опечатке"
    );
}

#[test]
fn o7_emitted_event_is_lossless_and_not_altered_by_allowlist() {
    // Allow-list решает «эмитить ли», а не «что эмитить»: содержимое обязано остаться
    // тем же, что даёт сырой транслятор M-18 (CT-RFC-04, L2D-I-2).
    use contracts::{EventKind, MdEvent, MdPayload};

    let syms = parse_capture_symbols(Some("ETHUSDT"));
    let (stream, data) = depth_data("ethusdt");
    let ev = venue_binance::l2delta_emission_for(&stream, &data, &syms)
        .expect("разрешённый символ обязан дать событие");

    let EventKind::Md(MdEvent {
        symbol,
        payload:
            MdPayload::L2Delta {
                bids,
                asks,
                first_update_id,
                final_update_id,
                prev_final_update_id,
                ..
            },
        ..
    }) = ev
    else {
        panic!("обязан быть EventKind::Md(L2Delta)");
    };

    assert_eq!(symbol, "ETHUSDT", "символ нормализован в верхний регистр");
    assert_eq!((first_update_id, final_update_id), (101, 103), "U/u сохранены");
    assert_eq!(
        prev_final_update_id, None,
        "СПОТ: `pu` в wire отсутствует ⇒ None (перп-семантика сюда не протекает — урок TD-014)"
    );
    assert_eq!(bids.len(), 2, "оба бид-уровня сохранены (множественность)");
    assert_eq!(bids[1].size, 0, "size==0 remove сохранён как явный маркер");
    assert!(
        asks.is_empty(),
        "пустая сторона остаётся пустой — «не упомянуто» не значит «очистить»"
    );
}

#[test]
fn o7_non_depth_stream_never_emits_regardless_of_allowlist() {
    // Граница: даже если символ разрешён, НЕ-depth поток не имеет права давать L2Delta.
    let syms = parse_capture_symbols(Some("ETHUSDT"));
    let data = serde_json::json!({"e": "trade", "s": "ETHUSDT", "p": "65000.5", "q": "0.3"});
    assert!(
        venue_binance::l2delta_emission_for("ethusdt@trade", &data, &syms).is_none(),
        "поток @trade не является depth-диффом — L2Delta из него не строится"
    );
}

#[test]
fn o7_malformed_depth_payload_is_fail_closed() {
    // Отсутствие/порча полей ⇒ None (не эмитим мусор), а не паника и не «додумать».
    let syms = parse_capture_symbols(Some("ETHUSDT"));
    for bad in [
        serde_json::json!({"e": "depthUpdate"}),
        serde_json::json!({"e": "depthUpdate", "U": 101}),
        serde_json::json!({"e": "depthUpdate", "U": "не число", "u": 103, "b": [], "a": []}),
        serde_json::json!({"e": "depthUpdate", "U": 101, "u": 103, "b": [["дичь", "0.3"]], "a": []}),
    ] {
        let got = venue_binance::l2delta_emission_for("ethusdt@depth@100ms", &bad, &syms);
        assert!(
            got.is_none(),
            "malformed depth-diff обязан быть fail-closed (None), а не эмиссией мусора: {bad}"
        );
    }
}
