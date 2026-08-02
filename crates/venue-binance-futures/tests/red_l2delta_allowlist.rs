//! RED M-45 (sacred, architect-only): allow-list эмиссии `L2Delta` для ПЕРПОВ.
//!
//! Зеркало `crates/venue-binance/tests/red_l2delta_allowlist.rs`. Дублирование намеренное и
//! обязательное: константа `L2DELTA_CAPTURE_SYMBOLS` объявлена в ДВУХ крейтах независимо
//! (`venue-binance/src/lib.rs`, `venue-binance-futures/src/lib.rs`), тесты крейт-локальны, и
//! правка одного крейта не даёт никаких гарантий про второй. Оракул только на споте
//! пропустил бы ровно половину дефекта — в том числе «в споте нормализовали регистр, в
//! перпах забыли», где отказ молчалив и необратим (данные forward-only).
//!
//! Объём и обоснование — `docs/rfc/CT-RFC-06-l2delta.md` §8.1; контракт API и разбор
//! «почему разбор обязан быть чистой функцией» — в спот-версии оракула, здесь не дублируется.
//!
//! **Что в перпах отличается и НЕ должно быть задето:** семантика continuity — перп чейнится
//! по `pu` (`prev_final_update_id = Some(diff.pu)`), спот — по `U == prev.u + 1`. Путаница
//! этих семантик ломает gap-детекцию (урок TD-014). Allow-list — решение «эмитить ли
//! событие», оно не имеет права влиять на СОДЕРЖИМОЕ события; последний тест это закрепляет.

use venue_binance_futures::{parse_capture_symbols, should_capture_l2delta};

/// Сегодняшнее прод-поведение перп-адаптера, зафиксированное константой ОРАКУЛА.
const PROD_DEFAULT: &[&str] = &["BTCUSDT"];

#[test]
fn o3_default_when_config_absent_equals_current_prod_behaviour() {
    assert_eq!(
        parse_capture_symbols(None),
        PROD_DEFAULT.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        "БЕЗ конфигурации перп-адаптер обязан эмитить ровно сегодняшний состав ({PROD_DEFAULT:?}) \
         — условие, при котором merge M-45 не является раскаткой"
    );
}

#[test]
fn o3_empty_config_is_default_not_all_and_not_nothing() {
    for raw in ["", "   ", ",", " , , "] {
        assert_eq!(
            parse_capture_symbols(Some(raw)),
            PROD_DEFAULT.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "вырожденная конфигурация {raw:?} обязана означать ДЕФОЛТ, не «всё» и не «ничего»"
        );
    }
}

#[test]
fn o3_garbage_elements_do_not_become_symbols() {
    let got = parse_capture_symbols(Some("BTCUSDT, ,ETHUSDT,"));
    assert_eq!(got, vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
    assert!(!got.iter().any(|s| s.is_empty()));
}

#[test]
fn o4_config_case_does_not_silently_disable_capture() {
    // Тот же дефект-кандидат, что в споте: wire-символ приходит в верхнем регистре,
    // сравнение точное ⇒ env в нижнем регистре молча выключает запись.
    for raw in ["btcusdt", "BtcUsdt", "BTCUSDT"] {
        let syms = parse_capture_symbols(Some(raw));
        assert!(
            should_capture_l2delta(&syms, "BTCUSDT"),
            "конфигурация {raw:?} обязана включать эмиссию для wire-символа BTCUSDT"
        );
    }
}

#[test]
fn o4_normalization_does_not_make_everything_match() {
    let syms = parse_capture_symbols(Some("btcusdt"));
    assert!(!should_capture_l2delta(&syms, "ETHUSDT"));
    assert!(!should_capture_l2delta(&syms, "BTC"));
    assert!(!should_capture_l2delta(&syms, "BTCUSDT_PERP"));
}

#[test]
fn o1_multiple_symbols_all_captured() {
    let syms = parse_capture_symbols(Some("BTCUSDT,ETHUSDT,SOLUSDT"));
    assert_eq!(syms.len(), 3);
    for s in ["BTCUSDT", "ETHUSDT", "SOLUSDT"] {
        assert!(should_capture_l2delta(&syms, s), "{s} обязан капчиться");
    }
}

#[test]
fn o2_symbol_outside_allowlist_is_not_captured() {
    let syms = parse_capture_symbols(Some("BTCUSDT,ETHUSDT"));
    for s in ["SOLUSDT", "XRPUSDT"] {
        assert!(
            !should_capture_l2delta(&syms, s),
            "{s} НЕ в allow-list ⇒ эмиссии быть не должно"
        );
    }
}

#[test]
fn o2_default_config_does_not_capture_beyond_btc() {
    let syms = parse_capture_symbols(None);
    assert!(should_capture_l2delta(&syms, "BTCUSDT"));
    for s in ["ETHUSDT", "SOLUSDT"] {
        assert!(
            !should_capture_l2delta(&syms, s),
            "дефолт не имеет права расширять состав эмиссии на {s}"
        );
    }
}

#[test]
fn o6_parse_is_deterministic_and_order_preserving() {
    let raw = Some("BTCUSDT, ethusdt ,SOLUSDT");
    let a = parse_capture_symbols(raw);
    assert_eq!(a, parse_capture_symbols(raw));
    assert_eq!(a, parse_capture_symbols(raw));
    assert_eq!(a.len(), 3);
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// Перп-специфика: allow-list решает «эмитить ли», а НЕ «что именно эмитить».
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn allowlist_does_not_alter_perp_continuity_semantics() {
    use contracts::{EventKind, MdEvent, MdPayload};
    use venue_binance_futures::{l2delta_event, DepthDiff};

    // Асимметричный дифф (обновлён только бид; asks пуст — «не менялось», НЕ очистка)
    // плюс remove-уровень `size == 0` — деградированный вход, а не идеальный.
    let diff = DepthDiff {
        event_time_ms: 1_752_000_000_499,
        u_first: 101,
        u_final: 103,
        pu: 100,
        bids: vec![(6_500_050_000_000, 30_000_000), (6_500_040_000_000, 0)],
        asks: vec![],
    };

    let ev = l2delta_event("ETHUSDT", &diff);
    let EventKind::Md(MdEvent {
        payload:
            MdPayload::L2Delta {
                prev_final_update_id,
                first_update_id,
                final_update_id,
                bids,
                asks,
                ..
            },
        ..
    }) = ev
    else {
        panic!("перп-адаптер обязан эмитить EventKind::Md(L2Delta)");
    };

    assert_eq!(
        prev_final_update_id,
        Some(100),
        "перп чейнится по `pu` — введение allow-list не имеет права ронять `prev_final_update_id` \
         в None (это спот-семантика; путаница ломает gap-детекцию, урок TD-014)"
    );
    assert_eq!((first_update_id, final_update_id), (101, 103));
    assert_eq!(bids.len(), 2, "оба бид-уровня сохранены, включая size==0 remove");
    assert_eq!(bids[1].size, 0, "remove-маркер не схлопнут");
    assert!(
        asks.is_empty(),
        "пустая сторона остаётся пустой — «не упомянуто» не значит «очистить»"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// O-7 — ВЫЗОВ НА РЕАЛЬНОМ ПУТИ (C-048 REJECT), перп-зеркало.
//
// Тот же обходной путь существует и здесь: `l2delta_event(&symbol, &diff)` зовётся из
// обработчика (`crates/venue-binance-futures/src/lib.rs:643`) под хардкод-условием.
// Реализация может выполнить контракт чистых функций и не подключить их к обработчику —
// оракулы O-1..O-6 останутся зелёными, потому что дёргают функции напрямую.
//
// Контракт (зеркало спота; перп различается ТОЛЬКО наличием `pu` в wire):
// ```ignore
// pub fn l2delta_emission_for(stream: &str, data: &serde_json::Value, symbols: &[String])
//     -> Option<EventKind>;
// ```
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Синтетическое wire-сообщение fstream `@depth` (перп: присутствует `pu`).
fn depth_data(sym_lower: &str) -> (String, serde_json::Value) {
    let stream = format!("{sym_lower}@depth@100ms");
    let data = serde_json::json!({
        "e": "depthUpdate",
        "E": 1_752_000_000_499i64,
        "s": sym_lower.to_uppercase(),
        "pu": 100,
        "U": 101,
        "u": 103,
        "b": [["65000.5", "0.3"], ["65000.4", "0"]],
        "a": []
    });
    (stream, data)
}

#[test]
fn o7_allowed_symbol_emits_through_real_message_path() {
    let syms = parse_capture_symbols(Some("ETHUSDT"));
    let (stream, data) = depth_data("ethusdt");
    assert!(
        venue_binance_futures::l2delta_emission_for(&stream, &data, &syms).is_some(),
        "разрешённый символ обязан дать L2Delta на реальном пути перп-адаптера"
    );
}

#[test]
fn o7_disallowed_symbol_does_not_emit_through_real_message_path() {
    let syms = parse_capture_symbols(Some("ETHUSDT"));
    let (stream, data) = depth_data("solusdt");
    assert!(
        venue_binance_futures::l2delta_emission_for(&stream, &data, &syms).is_none(),
        "SOLUSDT НЕ в allow-list ⇒ на реальном пути эмиссии быть не должно"
    );
}

#[test]
fn o7_default_config_still_emits_btc_and_only_btc_on_real_path() {
    let syms = parse_capture_symbols(None);
    let (s_btc, d_btc) = depth_data("btcusdt");
    assert!(
        venue_binance_futures::l2delta_emission_for(&s_btc, &d_btc, &syms).is_some(),
        "дефолт обязан продолжать писать BTC-перп — иначе merge теряет данные (forward-only)"
    );
    let (s_eth, d_eth) = depth_data("ethusdt");
    assert!(
        venue_binance_futures::l2delta_emission_for(&s_eth, &d_eth, &syms).is_none(),
        "дефолт не имеет права расширять состав эмиссии (Граница C)"
    );
}

#[test]
fn o7_real_path_preserves_perp_continuity_chain() {
    // Перп-специфика СКВОЗЬ реальный разбор: `pu` обязан доехать до события.
    // Реализация, скопированная со спота (где `pu` нет), уронит его в None и сломает
    // gap-детекцию перп-книги — урок TD-014.
    use contracts::{EventKind, MdEvent, MdPayload};

    let syms = parse_capture_symbols(Some("ETHUSDT"));
    let (stream, data) = depth_data("ethusdt");
    let ev = venue_binance_futures::l2delta_emission_for(&stream, &data, &syms)
        .expect("разрешённый символ обязан дать событие");
    let EventKind::Md(MdEvent {
        payload: MdPayload::L2Delta { prev_final_update_id, bids, asks, .. },
        ..
    }) = ev
    else {
        panic!("обязан быть EventKind::Md(L2Delta)");
    };
    assert_eq!(
        prev_final_update_id,
        Some(100),
        "перп чейнится по `pu` — на реальном пути он обязан сохраниться (TD-014)"
    );
    assert_eq!(bids.len(), 2, "оба бид-уровня, включая size==0 remove");
    assert!(asks.is_empty(), "пустая сторона остаётся пустой");
}

#[test]
fn o7_malformed_depth_payload_is_fail_closed() {
    let syms = parse_capture_symbols(Some("ETHUSDT"));
    for bad in [
        serde_json::json!({"e": "depthUpdate"}),
        // `pu` отсутствует — для перпа это порча континуити, а не «ноль»
        serde_json::json!({"e": "depthUpdate", "U": 101, "u": 103, "b": [], "a": []}),
        serde_json::json!({"e": "depthUpdate", "pu": 100, "U": "нет", "u": 103, "b": [], "a": []}),
    ] {
        assert!(
            venue_binance_futures::l2delta_emission_for("ethusdt@depth@100ms", &bad, &syms)
                .is_none(),
            "malformed перп-дифф обязан быть fail-closed (None): {bad}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// O-8 — РЕШАЮЩИЙ оракул: поведение РЕАЛЬНОЙ точки входа, а не структура кода (C-049).
//
// Два круга критика подряд отвергли структурные проверки, и по одной причине: гейт,
// проверяющий ФОРМУ кода, обходится сдвигом хардкода на уровень выше. C-049 §1.2 (A)
// показал это в одну строку — `if symbol == "BTCUSDT" { … }` ПЕРЕД вызовом
// `l2delta_emission_for`: вызовов `l2delta_event` по-прежнему один, slice-литерала нет,
// весь verify зелёный, а раскатка после founder-подписи молча не работает никогда.
//
// Поэтому предмет проверки — ЭФФЕКТЫ реальной точки входа. `FuturesSession::on_ws_text` — sync,
// без сети и каналов (док-комментарий машины: «только накапливает состояние и возвращает
// Vec<SessionEffect>»), поэтому её можно звать прямо из оракула сырым wire-текстом.
// Любое хардкод-условие по символу НА ЛЮБОМ УРОВНЕ внутри обработки проявится здесь как
// отсутствие ожидаемого `Emit` — обходить нечего, проверяется поведение.
//
// Контракт: `FuturesSession` конфигурируется allow-list'ом L2Delta ЯВНЫМ параметром (рядом с
// существующим `symbol_set`), и `on_ws_text` решает об эмиссии `L2Delta` только по нему.
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Сырой текст fstream-сообщения — ровно та форма, что приходит по WS в прод.
fn ws_depth_text(sym_lower: &str) -> String {
    format!(
        r#"{{"stream":"{sym_lower}@depth@100ms","data":{{"e":"depthUpdate","E":1752000000499,"T":1752000000490,"s":"{}","pu":100,"U":101,"u":103,"b":[["65000.5","0.3"],["65000.4","0"]],"a":[]}}}}"#,
        sym_lower.to_uppercase()
    )
}

/// Сколько `Emit`-эффектов с payload `L2Delta` по данному символу.
fn count_l2delta_emits(effects: &[venue_binance_futures::SessionEffect], symbol: &str) -> usize {
    use contracts::MdPayload;
    use venue_binance_futures::SessionEffect;
    effects
        .iter()
        .filter(|e| match e {
            SessionEffect::Emit(md) => {
                md.symbol == symbol && matches!(md.payload, MdPayload::L2Delta { .. })
            }
            _ => false,
        })
        .count()
}

#[test]
fn o8_allowed_symbol_emits_l2delta_from_real_entry_point() {
    // ETHUSDT разрешён allow-list'ом ⇒ реальная обработка сообщения обязана дать Emit(L2Delta).
    // Обход (A) из C-049 (внешний `if symbol == "BTCUSDT"`) даёт здесь НОЛЬ эмиссий и падает.
    let mut s = venue_binance_futures::FuturesSession::new_with_l2delta(
        &["ETHUSDT".to_string()],
        &["ETHUSDT".to_string()],
    );
    let effects = s.on_ws_text(&ws_depth_text("ethusdt"));
    assert_eq!(
        count_l2delta_emits(&effects, "ETHUSDT"),
        1,
        "символ разрешён allow-list'ом ⇒ РЕАЛЬНАЯ точка входа обязана дать ровно один \
         Emit(L2Delta). Ноль здесь означает хардкод-условие по символу где-то на пути \
         эмиссии — раскатка после founder-подписи не заработает (C-049 §1.2)"
    );
}

#[test]
fn o8_disallowed_symbol_emits_nothing_from_real_entry_point() {
    // Подписка на символ есть (он в symbol_set), но в L2Delta-allow-list его НЕТ:
    // книга ведётся, дельты в журнал не пишутся. Реализация «эмитить всем подписанным»
    // падает здесь.
    let mut s = venue_binance_futures::FuturesSession::new_with_l2delta(
        &["ETHUSDT".to_string()],
        &["BTCUSDT".to_string()],
    );
    let effects = s.on_ws_text(&ws_depth_text("ethusdt"));
    assert_eq!(
        count_l2delta_emits(&effects, "ETHUSDT"),
        0,
        "символ НЕ в L2Delta-allow-list ⇒ дельта не имеет права попасть в журнал, \
         даже если символ подписан для книги"
    );
}

#[test]
fn o8_default_allowlist_emits_btc_and_only_btc() {
    // Прод-инвариант merge'а, проверенный на реальной точке входа: без конфигурации
    // пишется BTC и только BTC.
    let subs = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];
    let dflt = parse_capture_symbols(None);
    let mut s = venue_binance_futures::FuturesSession::new_with_l2delta(&subs, &dflt);

    let btc = s.on_ws_text(&ws_depth_text("btcusdt"));
    assert_eq!(
        count_l2delta_emits(&btc, "BTCUSDT"),
        1,
        "дефолт обязан продолжать писать BTC — иначе merge ТЕРЯЕТ данные, которые прод \
         пишет с 2026-07-21 (forward-only, невосстановимо)"
    );

    let eth = s.on_ws_text(&ws_depth_text("ethusdt"));
    assert_eq!(
        count_l2delta_emits(&eth, "ETHUSDT"),
        0,
        "дефолт не имеет права расширять состав эмиссии — это раскатка без решения \
         founder'а (Граница C)"
    );
}

#[test]
fn o8_lowercase_config_works_through_real_entry_point() {
    // Регистр — сквозь всю реальную обработку, а не только через чистый разбор.
    let mut s = venue_binance_futures::FuturesSession::new_with_l2delta(
        &["ETHUSDT".to_string()],
        &parse_capture_symbols(Some("ethusdt")),
    );
    let effects = s.on_ws_text(&ws_depth_text("ethusdt"));
    assert_eq!(
        count_l2delta_emits(&effects, "ETHUSDT"),
        1,
        "конфигурация в нижнем регистре обязана работать на реальном пути — иначе \
         операторская опечатка молча выключает запись"
    );
}

#[test]
fn o8_multiple_allowed_symbols_all_emit() {
    // Множественность на реальной точке входа: реализация, учитывающая только первый
    // элемент allow-list'а, падает здесь.
    let syms = parse_capture_symbols(Some("ETHUSDT,SOLUSDT"));
    let subs = vec!["ETHUSDT".to_string(), "SOLUSDT".to_string()];
    let mut s = venue_binance_futures::FuturesSession::new_with_l2delta(&subs, &syms);

    let e1 = s.on_ws_text(&ws_depth_text("ethusdt"));
    let e2 = s.on_ws_text(&ws_depth_text("solusdt"));
    assert_eq!(count_l2delta_emits(&e1, "ETHUSDT"), 1, "ETHUSDT обязан эмитить");
    assert_eq!(count_l2delta_emits(&e2, "SOLUSDT"), 1, "SOLUSDT обязан эмитить");
}

#[test]
fn o8_emitted_payload_is_intact_through_real_entry_point() {
    // Allow-list решает «эмитить ли», а не «что эмитить»: перп-континуити `pu` обязан
    // доехать целым сквозь реальную обработку (TD-014).
    use contracts::MdPayload;
    use venue_binance_futures::SessionEffect;

    let mut s = venue_binance_futures::FuturesSession::new_with_l2delta(
        &["ETHUSDT".to_string()],
        &["ETHUSDT".to_string()],
    );
    let effects = s.on_ws_text(&ws_depth_text("ethusdt"));
    let md = effects
        .iter()
        .find_map(|e| match e {
            SessionEffect::Emit(md) if matches!(md.payload, MdPayload::L2Delta { .. }) => Some(md),
            _ => None,
        })
        .expect("разрешённый символ обязан дать Emit(L2Delta)");

    let MdPayload::L2Delta {
        prev_final_update_id,
        first_update_id,
        final_update_id,
        bids,
        asks,
        ..
    } = &md.payload
    else {
        unreachable!("отфильтровано выше");
    };
    assert_eq!(*prev_final_update_id, Some(100), "`pu` сохранён (TD-014)");
    assert_eq!((*first_update_id, *final_update_id), (101, 103), "U/u сохранены");
    assert_eq!(bids.len(), 2, "оба бид-уровня, включая size==0 remove");
    assert!(asks.is_empty(), "пустая сторона остаётся пустой");
}
