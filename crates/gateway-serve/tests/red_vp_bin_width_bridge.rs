//! RED `M-86` `V5b` (sacred, architect-only) — **РАЗОБРАННАЯ КОНФИГУРАЦИЯ ДОХОДИТ ДО СЕТКИ,
//! А ОТВЕРГНУТАЯ — НЕ ТРОГАЕТ ПРЕЖНЕЕ ЗНАЧЕНИЕ.**
//!
//! Милестоун `milestones/M-86-vp-bin-width.md`, задача 4. Введён rev2 по `C-229` R4.
//!
//! ## Дыра, которую он закрывает — названа критиком, и она реальна
//!
//! `V5` (`red_vp_bin_width_startup.rs`) судит ТОЛЬКО `Result<ServeConfig, String>`, а
//! governed-оракул (`crates/gateway/tests/red_vp_bin_width_governed.rs`) зовёт сеттер
//! РУКАМИ. Между ними остаётся щель: реализация «разобрать и провалидировать
//! `GATEWAY_VP_BIN_WIDTH_E8`, но сеттер не позвать» проходит `V5`, `C`, `V1` и `V6` —
//! и конфигурация оператора становится инертной. Ровно `R-133` B-1 из `M-71`
//! («built-not-wired» внутри одного процесса), только в новом шве.
//!
//! ## Вторая половина — safety-несущая
//!
//! Отвергнутый старт НЕ СМЕЕТ изменить прежнее эффективное значение. Иначе испорченная
//! конфигурация управляет сервисом, который на ней не стартовал, — класс `GW-I-14`/R7,
//! у `M-71` он пиннится оракулом `N1-E` (`milestones/M-71-egress-cap.md` §4bis.2, часть «а»).
//!
//! ## Почему ОДИН тест, а не два
//!
//! Обе половины трогают ПРОЦЕССНОЕ значение, а тестовый бинарь гоняет тесты в потоках
//! параллельно. Один тест — одна последовательность — гонки нет. Файл однотестовый
//! НАМЕРЕННО; добавлять сюда соседа нельзя.
//!
//! COMPILE-RED: `gateway::effective_vp_bin_width_e8` не существует.

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;

const VAR: &str = "GATEWAY_VP_BIN_WIDTH_E8";

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

fn cfg_with(width: Option<&'static str>) -> Result<gateway_serve::server::ServeConfig, String> {
    let mut pairs: Vec<(&'static str, &'static str)> = vec![("GATEWAY_JWT_SECRET", "test-secret")];
    if let Some(v) = width {
        pairs.push((VAR, v));
    }
    serve_config_from_env(getter(&pairs))
}

/// **V5b.** Мост `env → эффективная ширина`, обе половины.
#[test]
fn v5b_env_reaches_the_grid_and_rejected_start_leaves_it_alone() {
    let default_w = gateway::DEFAULT_VP_BIN_WIDTH_E8;

    // ── Половина 1: валидное НЕдефолтное значение обязано ДОЙТИ до сетки ────────────────
    // Значение выбрано отличным от дефолта намеренно: равное дефолту не отличило бы
    // «сеттер вызван» от «сеттер не вызван, читается дефолт» — тест был бы тавтологией.
    let custom = default_w * 2;
    let leaked: &'static str = Box::leak(custom.to_string().into_boxed_str());
    cfg_with(Some(leaked)).unwrap_or_else(|e| {
        panic!("SETUP НЕ СОСТОЯЛСЯ: валидное значение {custom} отвергнуто на старте ({e})")
    });
    assert_eq!(
        gateway::effective_vp_bin_width_e8(),
        custom,
        "V5b НАРУШЕН: после `serve_config_from_env` с {VAR}={custom} эффективная ширина \
         осталась {}. Значение разобрано, но до точки применения НЕ ДОШЛО — конфигурация \
         оператора инертна (класс R-133 B-1: built-not-wired)",
        gateway::effective_vp_bin_width_e8()
    );

    // ── Половина 2: ОТВЕРГНУТЫЙ старт не смеет изменить прежнее значение ───────────────
    let before = gateway::effective_vp_bin_width_e8();
    let err = cfg_with(Some("abc"));
    assert!(
        err.is_err(),
        "SETUP НЕ СОСТОЯЛСЯ: мусорное значение принято на старте — половина 2 судит не то"
    );
    assert_eq!(
        gateway::effective_vp_bin_width_e8(),
        before,
        "V5b НАРУШЕН: отвергнутая конфигурация ИЗМЕНИЛА эффективную ширину ({} → {}). \
         Конфигурация, на которой сервис не стартовал, не смеет им управлять \
         (класс GW-I-14 / урок R7)",
        before,
        gateway::effective_vp_bin_width_e8()
    );

    // ── Возврат дефолта: процесс не должен уехать с чужим значением ────────────────────
    gateway::set_effective_vp_bin_width_e8(default_w);
}
