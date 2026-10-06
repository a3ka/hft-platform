//! RED `M-94` (`C-287` B-2; sacred, architect-only) — **прогреватель судит каждую величину ОДНИМ
//! правилом в трёх формах входа: окружение без профиля, флаги без профиля, файл профиля**
//! (спека §3.2 «Одна грамматика на величину»). Пара к `gateway-serve/tests/red_m94_single_grammar.rs`
//! — там вторая точка входа (выдача).
//!
//! # Как судится — прод-бинарь, граница процесса
//!
//! `gateway-checkpoint` запускается на ПУСТОМ журнале во временном каталоге (замер: на пустом журнале
//! валидная конфигурация даёт exit `0` и пишет `ckpt-<отпечаток>.bin`, отвергнутая — exit `≠ 0`).
//! Исход — принято/отвергнуто; у принятого — ИМЯ слепка (отпечаток селектора: полосы, таймфрейм,
//! окно, каденция). Прогреватель без профиля читает четыре величины из шести (окно heatmap и шаг VP —
//! только в профиле; замер `grep` на `2209a05`), их и судит файл.
//!
//! * `w1` — согласие трёх форм на корпусе и парах «таймфрейм × каденция»; страж давления по каждой
//!   величине и форме.
//! * `w2` — политика профиля: окно `0` в профиле — отказ с именем ключа; без профиля — принято.
//! * `w3` — отсутствие / пустое / пробельное без профиля — как сегодня (замер `2209a05`; у
//!   прогревателя таблица СВОЯ, отличная от выдачи — предсуществующее различие бинарей, спека §3.2
//!   (а)); в профиле — отказ с именем ключа.
//! * `w4` (`#[ignore]`) — сторожевой мир пробы `red_m94_shared_grammar_probe.sh`.
//!
//! # Сегодня (`2209a05`) — красен по предмету
//!
//! `w1`: полосы `-0.1`, `0`, `NaN`, `inf` без профиля приняты (окружение и флаги), в профиле — отказ.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("корень репозитория")
        .to_path_buf()
}

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

/// Ключ окружения → флаг прогревателя.
const AXES: [(&str, &str); 4] = [
    ("GATEWAY_BANDS", "--bands"),
    ("GATEWAY_TIMEFRAME_MS", "--timeframe-ms"),
    ("GATEWAY_WINDOW_MS", "--window-ms"),
    ("GATEWAY_DEPTH_CADENCE_MS", "--depth-cadence-ms"),
];

#[derive(Debug, PartialEq, Clone)]
enum Outcome {
    Accepted(Vec<String>),
    Refused(String),
}

fn accepted(o: &Outcome) -> bool {
    matches!(o, Outcome::Accepted(_))
}

/// Один прогон на пустом журнале. `env` — ПОЛНОЕ окружение процесса (кроме PATH).
fn run(env: &BTreeMap<String, String>, extra_args: &[String]) -> Outcome {
    let t = tempfile::tempdir().expect("tempdir");
    let (j, c) = (t.path().join("journal"), t.path().join("ckpt"));
    std::fs::create_dir_all(&j).unwrap();
    std::fs::create_dir_all(&c).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_gateway-checkpoint"))
        .current_dir(t.path())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .envs(env)
        .arg("--dir")
        .arg(&j)
        .arg("--ckpt-dir")
        .arg(&c)
        .arg("--coverage-out")
        .arg(t.path().join("covered_through_seq"))
        .args(extra_args)
        .output()
        .expect("запуск gateway-checkpoint");
    if out.status.success() {
        let mut names: Vec<String> = std::fs::read_dir(&c)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("ckpt-") && n.ends_with(".bin"))
            .collect();
        names.sort();
        assert!(
            !names.is_empty(),
            "SETUP НЕ СОСТОЯЛСЯ: exit 0, но слепка нет — исход не различает селекторы"
        );
        Outcome::Accepted(names)
    } else {
        Outcome::Refused(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn venue_symbol() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("GATEWAY_VENUE".into(), "Binance".into());
    m.insert("GATEWAY_SYMBOL".into(), "BTCUSDT".into());
    m
}

type Edit<'a> = (&'a str, Option<&'a str>);

/// Четыре оси без профиля: значения `active.env` + правки.
fn legacy_values(edits: &[Edit]) -> BTreeMap<String, Option<String>> {
    let a = active();
    let mut m: BTreeMap<String, Option<String>> = AXES
        .iter()
        .map(|(k, _)| (k.to_string(), a.get(*k).cloned()))
        .collect();
    for (k, v) in edits {
        m.insert(k.to_string(), v.map(str::to_string));
    }
    m
}

fn legacy_env(edits: &[Edit]) -> Outcome {
    let mut env = venue_symbol();
    for (k, v) in legacy_values(edits) {
        if let Some(v) = v {
            env.insert(k, v);
        }
    }
    run(&env, &[])
}

fn legacy_flags(edits: &[Edit]) -> Outcome {
    let mut args = Vec::new();
    for (k, v) in legacy_values(edits) {
        let flag = AXES.iter().find(|(e, _)| *e == k).unwrap().1;
        if let Some(v) = v {
            args.push(format!("{flag}={v}"));
        }
    }
    run(&venue_symbol(), &args)
}

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
    let mut env = venue_symbol();
    env.insert("GATEWAY_CALC_PROFILE".into(), path.display().to_string());
    run(&env, &[])
}

fn agree(a: &Outcome, b: &Outcome) -> bool {
    match (a, b) {
        (Outcome::Refused(_), Outcome::Refused(_)) => true,
        (Outcome::Accepted(x), Outcome::Accepted(y)) => x == y,
        _ => false,
    }
}

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
];

const PAIRS: &[(&str, &str)] = &[
    ("3000", "1000"),
    ("1000", "2000"),
    ("60000", "60000"),
    ("1000", "1500"),
    ("2000", "3000"),
    ("60000", "90000"),
];

#[test]
fn w1_agreement_w2_policy_w3_absence() {
    let dir = tempfile::tempdir().expect("tempdir");
    let forms = |edit: &[Edit]| -> [(&'static str, Outcome); 3] {
        [
            ("окружение", legacy_env(edit)),
            ("флаги", legacy_flags(edit)),
            ("профиль", profiled(edit, dir.path(), true)),
        ]
    };

    let base = forms(&[]);
    assert!(
        base.iter().all(|(_, o)| accepted(o))
            && agree(&base[0].1, &base[1].1)
            && agree(&base[0].1, &base[2].1),
        "SETUP НЕ СОСТОЯЛСЯ: активный профиль не принят одинаково тремя формами: {base:?}"
    );

    // ── w1 ──
    let mut diverged = Vec::new();
    let mut pressure: BTreeMap<(String, &str), (u32, u32)> = BTreeMap::new();
    for (k, values) in CORPUS {
        for v in *values {
            let r = forms(&[(*k, Some(*v))]);
            for (form, o) in &r {
                let e = pressure.entry((k.to_string(), *form)).or_default();
                if accepted(o) {
                    e.0 += 1
                } else {
                    e.1 += 1
                }
            }
            if !(agree(&r[0].1, &r[1].1) && agree(&r[0].1, &r[2].1)) {
                diverged.push(format!("  {k}={v:?}: {r:?}"));
            }
        }
    }
    let (mut pa, mut pr) = (0, 0);
    for (tf, cad) in PAIRS {
        let r = forms(&[
            ("GATEWAY_TIMEFRAME_MS", Some(*tf)),
            ("GATEWAY_DEPTH_CADENCE_MS", Some(*cad)),
        ]);
        if accepted(&r[0].1) {
            pa += 1
        } else {
            pr += 1
        }
        if !(agree(&r[0].1, &r[1].1) && agree(&r[0].1, &r[2].1)) {
            diverged.push(format!("  пара таймфрейм={tf} × каденция={cad}: {r:?}"));
        }
    }
    for (k, _) in AXES {
        for form in ["окружение", "флаги", "профиль"] {
            let (a, r) = pressure
                .get(&(k.to_string(), form))
                .copied()
                .unwrap_or_default();
            assert!(
                a >= 1 && r >= 1,
                "SETUP НЕ СОСТОЯЛСЯ: корпус не давит на {k} в форме «{form}» (принято {a}, отвергнуто {r})"
            );
        }
    }
    assert!(
        pa >= 2 && pr >= 2,
        "SETUP НЕ СОСТОЯЛСЯ: пары не давят в обе стороны (принято {pa}, отвергнуто {pr})"
    );
    assert!(
        diverged.is_empty(),
        "M-94 / C-287 B-2 / спека §3.2: прогреватель судит величину по-разному в формах входа ({} \
         расхождений):\n{}",
        diverged.len(),
        diverged.join("\n")
    );

    // ── w2 ──
    let l = legacy_env(&[("GATEWAY_WINDOW_MS", Some("0"))]);
    assert!(
        accepted(&l),
        "SETUP НЕ СОСТОЯЛСЯ: без профиля окно 0 (offline) отвергнуто: {l:?}"
    );
    match profiled(&[("GATEWAY_WINDOW_MS", Some("0"))], dir.path(), true) {
        Outcome::Refused(e) => assert!(
            e.contains("GATEWAY_WINDOW_MS"),
            "M-94 / спека §3.2 (б): отказ профиля с окном 0 обязан назвать ключ: {e}"
        ),
        a => panic!("M-94 / спека §3.2 (б): профиль с окном 0 принят: {a:?}"),
    }

    // ── w3 ── таблица прогревателя без профиля (замер `2209a05`): пустое = отсутствие
    // (`env_string`), пробельное — отказ, кроме каденции (там пробельное = дефолт).
    enum Want {
        Same(&'static str),
        Refuse,
    }
    use Want::*;
    let table: [(&str, [Want; 3]); 4] = [
        ("GATEWAY_BANDS", [Same("0.001"), Same("0.001"), Refuse]),
        ("GATEWAY_TIMEFRAME_MS", [Same("1000"), Same("1000"), Refuse]),
        ("GATEWAY_WINDOW_MS", [Same("60000"), Same("60000"), Refuse]),
        (
            "GATEWAY_DEPTH_CADENCE_MS",
            [Same("1000"), Same("1000"), Same("1000")],
        ),
    ];
    let mut wrong = Vec::new();
    for (k, wants) in &table {
        for (form, want) in [None, Some(""), Some(" ")].iter().zip(wants.iter()) {
            let got = legacy_env(&[(k, *form)]);
            let ok = match want {
                Same(x) => {
                    let explicit = legacy_env(&[(k, Some(*x))]);
                    accepted(&explicit) && got == explicit
                }
                Refuse => !accepted(&got),
            };
            if !ok {
                wrong.push(format!("  без профиля {k} {form:?}: получили {got:?}"));
            }
            match profiled(&[(k, *form)], dir.path(), false) {
                Outcome::Refused(e) if e.contains(k) => {}
                other => wrong.push(format!(
                    "  профиль {k} {form:?}: ждали отказ с именем ключа, получили {other:?}"
                )),
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "M-94 / C-287 B-1 / спека §3.2 (а): отсутствие/пустое у прогревателя не по таблице:\n{}",
        wrong.join("\n")
    );
}

/// Сторожевые значения — те же, что у выдачи (`red_m94_single_grammar.rs`).
const SENTINELS: [(&str, &str); 4] = [
    ("GATEWAY_BANDS", "0.777"),
    ("GATEWAY_TIMEFRAME_MS", "500"),
    ("GATEWAY_WINDOW_MS", "77000"),
    ("GATEWAY_DEPTH_CADENCE_MS", "2000"),
];

#[test]
#[ignore = "исполняется только scripts/tests/red_m94_shared_grammar_probe.sh в дереве со сторожем"]
fn w4_shared_parser_result_reaches_every_form() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut missing = Vec::new();
    for (k, v) in SENTINELS {
        let tag = format!("M94-SENTINEL-{k}");
        let edit = [(k, Some(v))];
        for (form, o) in [
            ("окружение", legacy_env(&edit)),
            ("флаги", legacy_flags(&edit)),
            ("профиль", profiled(&edit, dir.path(), true)),
        ] {
            match &o {
                Outcome::Refused(e) if e.contains(&tag) => {}
                other => missing.push(format!("  {form} {k}={v:?}: {other:?}")),
            }
        }
    }
    assert!(
        missing.is_empty(),
        "M-94 / C-287 B-2: результат прогревателя не идёт из общей `calc_profile::parse_*`:\n{}",
        missing.join("\n")
    );
}
