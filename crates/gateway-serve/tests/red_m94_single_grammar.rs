//! RED `M-94` (`R-245` B-1, `C-287` B-1/B-2; sacred, architect-only) — **у каждой величины
//! профиля ОДНО правило разбора: значение судится одинаково с профилем и без него, принятое даёт ту
//! же конфигурацию выдачи; режимы различаются только тем, что названо в спеке §3.2**.
//!
//! # Почему
//!
//! `R-245` B-1: загрузчик профиля проверял полосы/таймфрейм/каденцию/окно СВОИМИ условиями, выдача
//! без профиля — своими, и правила разошлись (полосы `-0.1` без профиля приняты, в профиле — отказ).
//! Два правила одной величины — два определения расчёта, против чего заведён `П-032`.
//!
//! # Миры (прод-функция `gateway_serve::serve_config_from_env`, её зовёт `main` бинаря)
//!
//! * `g1` — СОГЛАСИЕ: корпус значений и ПАР «таймфрейм × каденция» (межключевое правило
//!   «каденция выровнена на таймфрейм» — третье возможное место расхождения, `C-287` B-2) судится
//!   дважды: (L) без профиля, ключи в окружении; (P) в профиле, `ALLOWED_PROFILES` — тройка самого
//!   файла, чтобы политика тройки не подменяла спор о грамматике. Совпадают принято/отвергнуто;
//!   принятое — селектор, окно heatmap, шаг VP. Страж давления — ПО КАЖДОЙ величине и режиму.
//! * `g2` — политика профиля §3.2 (б): окно без границы (`0`) в профиле — отказ с именем ключа; вне
//!   профиля `0` — offline (страж семантики M-37 / `C-099` B-2).
//! * `g3` — различие §3.2 (а), ИСПОЛНИМО (`C-287` B-1): отсутствие / пустое / пробельное значение
//!   вне профиля ведут себя РОВНО как сегодня (таблица — замер на `2209a05`, совпадает с
//!   `red_heatmap_window_env`: пустое окно heatmap — отказ); в профиле — отказ с именем ключа.
//! * `g4` (`#[ignore]`) — сторожевой мир пробы `scripts/tests/red_m94_shared_grammar_probe.sh`:
//!   исполняется ТОЛЬКО в дереве, где проба вживила в шесть общих `calc_profile::parse_*` отказ на
//!   сторожевом значении. Доказывает, что РЕЗУЛЬТАТ ветки без профиля и загрузчика идёт из общей
//!   функции (вызов в комментарии, мёртвой ветке или только в профиле — не проходит).
//!
//! # Сегодня (`2209a05`) — красен по предмету
//!
//! `g1`: полосы `-0.1`, `0`, `NaN`, `inf`, `0.01,-0.02` без профиля приняты, в профиле — отказ.
//! RUNTIME-RED: только существующие публичные символы (урок `A-033`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("корень репозитория")
        .to_path_buf()
}

/// Профиль разбирается здесь НЕЗАВИСИМО от загрузчика dev'а.
fn active() -> BTreeMap<String, String> {
    let p = repo_root().join("config/calc-profile/active.env");
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("SETUP НЕ СОСТОЯЛСЯ: нет {} ({e})", p.display()))
        .lines()
        .map(str::trim)
        .filter(|t| !t.is_empty() && !t.starts_with('#'))
        .filter_map(|t| t.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn base_env() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("GATEWAY_JWT_SECRET".into(), "ab".repeat(32));
    m.insert("GATEWAY_VENUE".into(), "Binance".into());
    m.insert("GATEWAY_SYMBOL".into(), "BTCUSDT".into());
    m
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Accepted {
        selector: String,
        heatmap: u64,
        vp: i64,
    },
    Refused(String),
}

fn accepted(o: &Outcome) -> bool {
    matches!(o, Outcome::Accepted { .. })
}

fn run(env: &BTreeMap<String, String>) -> Outcome {
    match gateway_serve::serve_config_from_env(|k| env.get(k).cloned()) {
        Ok(cfg) => Outcome::Accepted {
            selector: format!("{:?}", cfg.selector),
            heatmap: gateway::effective_heatmap_window_frac().to_bits(),
            vp: gateway::effective_vp_bin_width_e8(),
        },
        Err(e) => Outcome::Refused(e),
    }
}

/// Правка: `Some(v)` — задать, `None` — убрать ключ.
type Edit<'a> = (&'a str, Option<&'a str>);

/// (L): величины профиля из `active.env` в окружении, поверх — правки; без профиля.
fn legacy(edits: &[Edit]) -> Outcome {
    let mut env = base_env();
    for (key, val) in active() {
        if key == "CALC_PROFILE_VERSION" || key == "GATEWAY_ALLOWED_PROFILES" {
            continue; // не величины `serve_config_from_env` вне профиля
        }
        env.insert(key, val);
    }
    for (k, v) in edits {
        match v {
            Some(v) => env.insert((*k).into(), (*v).into()),
            None => env.remove(*k),
        };
    }
    run(&env)
}

/// (P): правки в файле профиля; ALLOWED — тройка самого файла (текстом), если `allowed_from_file`.
fn profiled(edits: &[Edit], dir: &Path, allowed_from_file: bool) -> Outcome {
    let mut prof = active();
    for (k, v) in edits {
        match v {
            Some(v) => prof.insert((*k).into(), (*v).into()),
            None => prof.remove(*k),
        };
    }
    if allowed_from_file {
        let g = |k: &str| prof.get(k).cloned().unwrap_or_default();
        let triple = format!(
            "{}/{}/{}",
            g("GATEWAY_TIMEFRAME_MS"),
            g("GATEWAY_WINDOW_MS"),
            g("GATEWAY_DEPTH_CADENCE_MS")
        );
        prof.insert("GATEWAY_ALLOWED_PROFILES".into(), triple);
    }
    let text: String = prof.iter().map(|(a, b)| format!("{a}={b}\n")).collect();
    let path = dir.join("profile.env");
    std::fs::write(&path, text).expect("запись профиля");
    let mut env = base_env();
    env.insert("GATEWAY_CALC_PROFILE".into(), path.display().to_string());
    run(&env)
}

fn agree(l: &Outcome, p: &Outcome) -> bool {
    match (l, p) {
        (Outcome::Refused(_), Outcome::Refused(_)) => true,
        (a @ Outcome::Accepted { .. }, b @ Outcome::Accepted { .. }) => a == b,
        _ => false,
    }
}

const KEYS: [&str; 6] = [
    "GATEWAY_BANDS",
    "GATEWAY_TIMEFRAME_MS",
    "GATEWAY_WINDOW_MS",
    "GATEWAY_DEPTH_CADENCE_MS",
    "GATEWAY_HEATMAP_WINDOW",
    "GATEWAY_VP_BIN_WIDTH_E8",
];

const CORPUS: &[(&str, &[&str])] = &[
    (
        "GATEWAY_BANDS",
        &[
            "0.015,0.03",
            "0.6",
            "-0.1",
            "0",
            "abc",
            "0.01,,0.02",
            "NaN",
            "inf",
            "0.01,-0.02",
        ],
    ),
    (
        "GATEWAY_TIMEFRAME_MS",
        &["1000", "500", "999", "0", "-1000", "abc", "604800000"],
    ),
    (
        "GATEWAY_WINDOW_MS",
        &["60000", "120000", "-1", "abc", "1e3"],
    ),
    (
        "GATEWAY_DEPTH_CADENCE_MS",
        &["1000", "2000", "999", "1500", "0", "-1000", "abc"],
    ),
    (
        "GATEWAY_HEATMAP_WINDOW",
        &["0.001", "0.5", "0", "1", "1.5", "-0.1", "abc", "NaN"],
    ),
    (
        "GATEWAY_VP_BIN_WIDTH_E8",
        &["25000000", "1", "0", "-1", "abc", "1.5", "1e8"],
    ),
];

/// Пары «таймфрейм × каденция» — межключевое правило (`M-68` R-141 Б-3): при каденции ≥ таймфрейма
/// каденция кратна таймфрейму; каденция МЕНЬШЕ таймфрейма допустима (`3000 × 1000` — принят).
const PAIRS: &[(&str, &str)] = &[
    ("3000", "1000"),
    ("1000", "2000"),
    ("60000", "60000"),
    ("1000", "1500"),
    ("2000", "3000"),
    ("60000", "90000"),
];

/// Всё в ОДНОЙ функции на файл: окно heatmap и шаг VP — глобальные значения процесса.
#[test]
fn g1_agreement_g2_policy_g3_absence() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Setup-страж: неизменённый активный профиль принят в ОБОИХ режимах и даёт одно и то же.
    let (l0, p0) = (legacy(&[]), profiled(&[], dir.path(), true));
    assert!(
        accepted(&l0) && l0 == p0,
        "SETUP НЕ СОСТОЯЛСЯ: активный профиль не принят одинаково:\n  L={l0:?}\n  P={p0:?}"
    );

    // ── g1 ──
    let mut diverged = Vec::new();
    // (величина, режим) → (принято, отвергнуто)
    let mut pressure: BTreeMap<(String, &str), (u32, u32)> = BTreeMap::new();
    let mut tally = |k: &str, mode: &'static str, o: &Outcome| {
        let e = pressure.entry((k.to_string(), mode)).or_default();
        if accepted(o) {
            e.0 += 1
        } else {
            e.1 += 1
        }
    };
    for (k, values) in CORPUS {
        for v in *values {
            let edit = [(*k, Some(*v))];
            let (l, p) = (legacy(&edit), profiled(&edit, dir.path(), true));
            tally(k, "без профиля", &l);
            tally(k, "профиль", &p);
            if !agree(&l, &p) {
                diverged.push(format!(
                    "  {k}={v:?}\n    без профиля: {l:?}\n    в профиле:   {p:?}"
                ));
            }
        }
    }
    let (mut pair_acc, mut pair_ref) = (0, 0);
    for (tf, cad) in PAIRS {
        let edit = [
            ("GATEWAY_TIMEFRAME_MS", Some(*tf)),
            ("GATEWAY_DEPTH_CADENCE_MS", Some(*cad)),
        ];
        let (l, p) = (legacy(&edit), profiled(&edit, dir.path(), true));
        if accepted(&l) {
            pair_acc += 1
        } else {
            pair_ref += 1
        }
        if !agree(&l, &p) {
            diverged.push(format!(
                "  пара таймфрейм={tf} × каденция={cad}\n    без профиля: {l:?}\n    в профиле:   {p:?}"
            ));
        }
    }
    // Страж давления (`C-287` B-2): КАЖДАЯ величина в КАЖДОМ режиме имеет и принятое, и
    // отвергнутое значение; пары — тоже обе стороны. Иначе разборщик, отвергающий всё по одной
    // величине, прошёл бы сравнение за счёт остальных.
    for k in KEYS {
        for mode in ["без профиля", "профиль"] {
            let (a, r) = pressure
                .get(&(k.to_string(), mode))
                .copied()
                .unwrap_or_default();
            assert!(
                a >= 1 && r >= 1,
                "SETUP НЕ СОСТОЯЛСЯ: корпус не давит на {k} в режиме «{mode}» (принято {a}, отвергнуто {r})"
            );
        }
    }
    assert!(
        pair_acc >= 2 && pair_ref >= 2,
        "SETUP НЕ СОСТОЯЛСЯ: пары таймфрейм×каденция не давят в обе стороны (принято {pair_acc}, \
         отвергнуто {pair_ref}) — межключевое правило не судится"
    );
    assert!(
        diverged.is_empty(),
        "M-94 / R-245 B-1 / спека §3.2: величина судится по-разному с профилем и без него ({} \
         расхождений):\n{}",
        diverged.len(),
        diverged.join("\n")
    );

    // ── g2: политика профиля — окно с границей ──
    let l = legacy(&[("GATEWAY_WINDOW_MS", Some("0"))]);
    match &l {
        Outcome::Accepted { selector, .. } => assert!(
            selector.contains("window_ms: None"),
            "SETUP НЕ СОСТОЯЛСЯ: вне профиля `GATEWAY_WINDOW_MS=0` не канонизирован в offline: {selector}"
        ),
        Outcome::Refused(e) => panic!(
            "SETUP НЕ СОСТОЯЛСЯ: вне профиля `GATEWAY_WINDOW_MS=0` отвергнут ({e}) — семантика offline сломана"
        ),
    }
    match profiled(&[("GATEWAY_WINDOW_MS", Some("0"))], dir.path(), true) {
        Outcome::Refused(e) => assert!(
            e.contains("GATEWAY_WINDOW_MS"),
            "M-94 / спека §3.2 (б): отказ профиля с окном 0 обязан назвать ключ: {e}"
        ),
        a => panic!(
            "M-94 / спека §3.2 (б): профиль с `GATEWAY_WINDOW_MS=0` принят как offline — определение \
             расчёта прода обязано быть с границей окна (TD-020): {a:?}"
        ),
    }

    // ── g3: отсутствие / пустое / пробельное ──
    // Вне профиля — таблица сегодняшнего поведения (замер `2209a05`): `Same(x)` — то же, что явное
    // значение `x`; `Refuse` — отказ старта.
    enum Want {
        Same(&'static str),
        Refuse,
    }
    use Want::*;
    let default_heatmap = format!("{}", gateway::DEFAULT_HEATMAP_WINDOW_FRAC);
    let default_vp = format!("{}", gateway::DEFAULT_VP_BIN_WIDTH_E8);
    let dh: &'static str = Box::leak(default_heatmap.into_boxed_str());
    let dv: &'static str = Box::leak(default_vp.into_boxed_str());
    let table: [(&str, [Want; 3]); 6] = [
        ("GATEWAY_BANDS", [Same("0.001"), Refuse, Refuse]),
        ("GATEWAY_TIMEFRAME_MS", [Same("1000"), Refuse, Refuse]),
        ("GATEWAY_WINDOW_MS", [Same("0"), Same("0"), Same("0")]),
        (
            "GATEWAY_DEPTH_CADENCE_MS",
            [Same("1000"), Same("1000"), Same("1000")],
        ),
        ("GATEWAY_HEATMAP_WINDOW", [Same(dh), Refuse, Refuse]),
        ("GATEWAY_VP_BIN_WIDTH_E8", [Same(dv), Same(dv), Same(dv)]),
    ];
    let mut wrong = Vec::new();
    for (k, wants) in &table {
        for (form, want) in [None, Some(""), Some(" ")].iter().zip(wants.iter()) {
            let got = legacy(&[(k, *form)]);
            let ok = match want {
                Same(x) => {
                    let explicit = legacy(&[(k, Some(*x))]);
                    accepted(&explicit) && got == explicit
                }
                Refuse => !accepted(&got),
            };
            if !ok {
                let w = match want {
                    Same(x) => format!("как явное {x:?}"),
                    Refuse => "отказ".into(),
                };
                wrong.push(format!(
                    "  без профиля {k} {form:?}: ждали {w}, получили {got:?}"
                ));
            }
            // В профиле: отсутствие и пустое — отказ с именем ключа (все ключи обязательны).
            let p = profiled(&[(k, *form)], dir.path(), false);
            match &p {
                Outcome::Refused(e) if e.contains(k) => {}
                other => wrong.push(format!(
                    "  профиль {k} {form:?}: ждали отказ с именем ключа, получили {other:?}"
                )),
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "M-94 / C-287 B-1 / спека §3.2 (а): отсутствие/пустое значение ведут себя не по таблице:\n{}",
        wrong.join("\n")
    );
}

/// Сторожевые значения пробы `red_m94_shared_grammar_probe.sh` — валидны по грамматике; проба
/// вживляет в `calc_profile::parse_*` отказ ровно на них с ключом `M94-SENTINEL-<величина>`.
pub const SENTINELS: [(&str, &str); 6] = [
    ("GATEWAY_BANDS", "0.777"),
    ("GATEWAY_TIMEFRAME_MS", "500"),
    ("GATEWAY_WINDOW_MS", "77000"),
    ("GATEWAY_DEPTH_CADENCE_MS", "2000"),
    ("GATEWAY_HEATMAP_WINDOW", "0.0077"),
    ("GATEWAY_VP_BIN_WIDTH_E8", "7700000"),
];

/// `g4` — ТОЛЬКО в дереве пробы. Сторожевой отказ общей функции обязан дойти до результата в обоих
/// режимах. Вне пробы этот мир красен по построению (сторожа нет) — поэтому `#[ignore]`.
#[test]
#[ignore = "исполняется только scripts/tests/red_m94_shared_grammar_probe.sh в дереве со сторожем"]
fn g4_shared_parser_result_reaches_both_modes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut missing = Vec::new();
    for (k, v) in SENTINELS {
        let tag = format!("M94-SENTINEL-{k}");
        for (mode, o) in [
            ("без профиля", legacy(&[(k, Some(v))])),
            ("профиль", profiled(&[(k, Some(v))], dir.path(), true)),
        ] {
            match &o {
                Outcome::Refused(e) if e.contains(&tag) => {}
                other => missing.push(format!("  {mode} {k}={v:?}: {other:?}")),
            }
        }
    }
    assert!(
        missing.is_empty(),
        "M-94 / C-287 B-2 / спека §3.2: результат ветки не идёт из общей `calc_profile::parse_*` — \
         сторожевой отказ не дошёл:\n{}",
        missing.join("\n")
    );
}
