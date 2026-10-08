//! M-94 (П-032) — **загрузчик профиля расчётов** + **ЕДИНСТВЕННЫЕ разборщики** четырёх
//! величин (`A-049` Р-6), которые раньше жили в `gateway-serve` и заводили второй
//! экземпляр логики для прогревателя (тогда как `gateway` зависеть от `gateway-serve` не
//! может). Носитель определения расчёта — один файл `config/calc-profile/active.env` с
//! ЗАКРЫТЫМ набором из ВОСЬМИ ключей; `sha256` байтов файла виден снаружи процесса
//! (сердцебиение выдачи, `<слепок>.profile`).
//!
//! ## Контракт (спека `milestones/M-94-calc-profile.md` §3.1 / §3.2)
//!
//! - файл `KEY=VALUE`, комментарии `#`, пустые строки;
//! - набор ключей ЗАКРЫТ и ПОЛОН: восемь (`CALC_PROFILE_VERSION` + 7 `GATEWAY_*`);
//! - `CALC_PROFILE_VERSION` — целое `>= 1`;
//! - значения разбираются ТЕМИ ЖЕ функциями, что и раньше в `gateway-serve`, с ТЕМИ ЖЕ
//!   сообщениями отказа (имена ключей сохранены; `p3`/`s2` пиннят «наличие — отказ»);
//! - `sha256` — от БАЙТОВ файла (не от разобранных значений: версия — заявление человека,
//!   хеш — факт о том, что процесс прочёл);
//! - **связь тройки** прогреваемого селектора `(timeframe_ms, window_ms, depth_cadence_ms)`
//!   ∈ `GATEWAY_ALLOWED_PROFILES` — иначе прогревается слепок, на который подписаться
//!   нельзя; проверка живёт ЗДЕСЬ (а не в `gateway-serve`/`gateway-checkpoint` раздельно).
//!
//! ## Дом единственного разборщика (`A-049` Р-6)
//!
//! Четыре разборщика (`heatmap_window_frac`, `vp_bin_width_e8`, `allowed_profiles`,
//! «тройка ∈ `ALLOWED_PROFILES`») живут ТОЛЬКО здесь. `gateway-serve` зовёт их через
//! `pub fn` — НЕ держит свою копию. Второй экземпляр этих четырёх в `gateway-serve`
//! был бы вторым правилом, и запрещён спекой §5.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// ЗАКРЫТЫЙ полный набор ключей профиля — ровно восемь (`A-049` Р-6, спека §3.1).
/// `GATEWAY_CANONICAL_BANDS` — НЕ часть профиля: канонический набор допуска
/// РАВЕН полосам профиля (милестоун §3.1, последний абзац).
pub const PROFILE_KEYS: [&str; 8] = [
    "CALC_PROFILE_VERSION",
    "GATEWAY_BANDS",
    "GATEWAY_DEPTH_CADENCE_MS",
    "GATEWAY_TIMEFRAME_MS",
    "GATEWAY_WINDOW_MS",
    "GATEWAY_ALLOWED_PROFILES",
    "GATEWAY_VP_BIN_WIDTH_E8",
    "GATEWAY_HEATMAP_WINDOW",
];

/// Восемь ключей профиля ПЛЮС `GATEWAY_CANONICAL_BANDS` — список «наличие в env — отказ»
/// (`A-049` Р-6, милестоун §3.3). Проверяется ДО разбора файла, потому что значение,
/// РАВНОЕ профилю, тоже запрещено (`П-032` п.1, оракулы `p3`/`s2`).
pub const PROFILE_AND_LEGACY_KEYS: [&str; 9] = [
    "CALC_PROFILE_VERSION",
    "GATEWAY_BANDS",
    "GATEWAY_CANONICAL_BANDS",
    "GATEWAY_DEPTH_CADENCE_MS",
    "GATEWAY_TIMEFRAME_MS",
    "GATEWAY_WINDOW_MS",
    "GATEWAY_ALLOWED_PROFILES",
    "GATEWAY_VP_BIN_WIDTH_E8",
    "GATEWAY_HEATMAP_WINDOW",
];

/// ЧЕТЫРЕ оси селектора, которые в режиме профиля НЕ ДОЛЖНЫ приходить флагом `--…`
/// (`A-049` Р-6, милестоун §3.3, оракул `p4`). `--venue`/`--symbol` НЕ здесь:
/// `venue`/`symbol` не входят в профиль (милестоун §3.1, ровно 8 ключей).
pub const PROFILE_AXIS_FLAGS: [&str; 4] = [
    "--bands",
    "--timeframe-ms",
    "--window-ms",
    "--depth-cadence-ms",
];

/// Тройка прогреваемого селектора в форме `(timeframe_ms, window_ms, depth_cadence_ms)` —
/// элемент `GATEWAY_ALLOWED_PROFILES`. Сделана в виде простого record'а, чтобы
/// `gateway` НЕ зависел от `gateway-serve::admission::LiveProfile` (циклический импорт);
/// `gateway-serve` маппит в `LiveProfile` по месту использования.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawLiveProfile {
    pub timeframe_ms: i64,
    pub window_ms: i64,
    pub depth_cadence_ms: Option<i64>,
}

/// Разобранный профиль — то, что процесс реально применил. `path` нужен heartbeat'у
/// и `<слепок>.profile` для отчёта о ПРОЧИТАННОМ файле; `raw` отдаётся наверх для
/// диагностики и для сердцебиения.
#[derive(Clone, Debug)]
pub struct AppliedProfile {
    /// Все восемь значений (`raw[KEY] = VALUE`), в исходной строковой форме —
    /// нужно для сердцебиения и обвязки «env-чтение через профиль».
    pub raw: BTreeMap<String, String>,
    pub version: u32,
    /// `sha256` БАЙТОВ файла (не значений): версия — заявление человека, хеш —
    /// факт о том, что процесс прочёл (милестоун §3.6).
    pub sha256: String,
    pub path: PathBuf,
    pub bands: Vec<f64>,
    pub timeframe_ms: i64,
    pub window_ms: i64,
    pub depth_cadence_ms: i64,
    pub heatmap_window_frac: f64,
    pub vp_bin_width_e8: i64,
    pub allowed_profiles: Vec<RawLiveProfile>,
}

/// Ошибка загрузчика / разборщика. Каждый вариант содержит имя КЛЮЧА, в stderr попадает
/// и ключ, и `GATEWAY_CALC_PROFILE` (`p3`/`s2` пиннят оба).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// Не удалось прочитать файл.
    Io(String),
    /// Строка без `=` (и не пустая, и не комментарий).
    BadLine(String),
    /// Неизвестный ключ (не входит в закрытый набор из 8).
    UnknownKey(String),
    /// Обязательный ключ отсутствует.
    MissingKey(String),
    /// Ключ повторён.
    Duplicate(String),
    /// `CALC_PROFILE_VERSION` не парсится как `u32` ИЛИ `< 1`.
    BadVersion(String),
    /// Значение ключа не разбирается ИЛИ вне разрешённого диапазона.
    BadValue { key: String, reason: String },
    /// Тройка `(timeframe_ms, window_ms, depth_cadence_ms)` НЕ найдена в
    /// `GATEWAY_ALLOWED_PROFILES` — прогревается слепок, на который подписаться нельзя.
    TripleNotInAllowed { tf: i64, window: i64, cadence: i64 },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::Io(s) => write!(f, "{s}"),
            ProfileError::BadLine(s) => write!(f, "bad profile line: {s:?}"),
            ProfileError::UnknownKey(s) => {
                write!(f, "unknown key {s:?} in profile (closed set of 8)")
            }
            ProfileError::MissingKey(s) => write!(f, "missing required key {s:?} in profile"),
            ProfileError::Duplicate(s) => write!(f, "duplicate key {s:?} in profile"),
            ProfileError::BadVersion(s) => write!(
                f,
                "CALC_PROFILE_VERSION={s:?} invalid — требуется целое >= 1"
            ),
            ProfileError::BadValue { key, reason } => {
                write!(f, "{key} invalid: {reason}")
            }
            ProfileError::TripleNotInAllowed {
                tf,
                window,
                cadence,
            } => write!(
                f,
                "GATEWAY_ALLOWED_PROFILES does not contain the triple \
                 (timeframe_ms={tf}, window_ms={window}, depth_cadence_ms={cadence}) \
                 — прогреваемый селектор вне политики допуска"
            ),
        }
    }
}

impl std::error::Error for ProfileError {}

/// `sha256` от байтов файла — то, что отдаётся наружу процесса. Отдельная функция
/// вместо инлайна, чтобы heartbeat и `.profile` звали ОДНО и то же и не разъехались
/// (тот же класс «два места вычисляют хеш по-разному», что закрывает `A-049`).
pub fn sha256_of_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Прочитать файл и вернуть `(bytes, parsed_map)`. Парсинг строк — здесь, проверка
/// замкнутости/версии/повторов/типов — в `load_profile` (чтобы можно было показать
/// ошибку в терминах ключа, а не байта).
fn read_and_parse(path: &Path) -> Result<(Vec<u8>, BTreeMap<String, String>), ProfileError> {
    let bytes = fs::read(path)
        .map_err(|e| ProfileError::Io(format!("read profile {}: {e}", path.display())))?;
    let text = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => {
            return Err(ProfileError::Io(format!(
                "profile {}: not valid UTF-8",
                path.display()
            )));
        }
    };
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (k, v) = t
            .split_once('=')
            .ok_or_else(|| ProfileError::BadLine(t.to_string()))?;
        let k = k.trim().to_string();
        let v = v.trim().to_string();
        if map.insert(k.clone(), v).is_some() {
            return Err(ProfileError::Duplicate(k));
        }
    }
    Ok((bytes, map))
}

/// Загрузить профиль из `path`. Закрытый набор, `CALC_PROFILE_VERSION >= 1`,
/// `sha256` от байтов, проверка связи тройки. **ЕДИНСТВЕННАЯ** точка, где профиль
/// разбирается целиком; `serve_config_from_env` / `admission_policy_from_env`
/// / `gateway-checkpoint` зовут её и читают поля `AppliedProfile`.
pub fn load_profile(path: &Path) -> Result<AppliedProfile, ProfileError> {
    let (bytes, map) = read_and_parse(path)?;
    let sha256 = sha256_of_bytes(&bytes);

    // ЗАКРЫТЫЙ набор (8 ключей). Неизвестный → отказ, отсутствующий → отказ, повтор →
    // отказ (`A-049` Р-6, оракулы `p5`/`s3`).
    for k in &PROFILE_KEYS {
        if !map.contains_key(*k) {
            return Err(ProfileError::MissingKey((*k).to_string()));
        }
    }
    for k in map.keys() {
        if !PROFILE_KEYS.contains(&k.as_str()) {
            return Err(ProfileError::UnknownKey(k.clone()));
        }
    }

    let version: u32 = map["CALC_PROFILE_VERSION"]
        .parse()
        .map_err(|e: std::num::ParseIntError| ProfileError::BadVersion(format!("{e:?}")))?;
    if version < 1 {
        return Err(ProfileError::BadVersion(format!("{version} (< 1)")));
    }

    let bands = parse_bands(&map["GATEWAY_BANDS"])?;
    let depth_cadence_ms = parse_depth_cadence_ms(&map["GATEWAY_DEPTH_CADENCE_MS"])?;
    let timeframe_ms = parse_timeframe_ms(&map["GATEWAY_TIMEFRAME_MS"])?;
    let window_ms = parse_window_ms(&map["GATEWAY_WINDOW_MS"])?;
    // Политика профиля (спека §3.2 (б)): `GATEWAY_WINDOW_MS=0` в профиле — отказ с
    // именем ключа. Грамматически `0` валиден (M-37 / `C-099` B-2 — легитимный offline),
    // и `parse_window_ms` его пропускает; вне профиля `serve_config_from_env`
    // канонизирует `0` в `None`, в профиле — здесь.
    if window_ms == 0 {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_WINDOW_MS".to_string(),
            reason: "0 must be > 0 (M-37: 0 = offline-unbounded; определение расчёта \
                     прода не бывает без окна, TD-020)"
                .to_string(),
        });
    }

    // ЧЕТЫРЕ разборщика — публичные, чтобы `gateway-serve` и `gateway-checkpoint`
    // звали их, а не дублировали (`A-049` Р-6, R-245 B-1). Здесь — для самой
    // загрузки (валидация тройки).
    let heatmap_window_frac = parse_heatmap_window_frac(&map["GATEWAY_HEATMAP_WINDOW"])?;
    let vp_bin_width_e8 = parse_vp_bin_width_e8(&map["GATEWAY_VP_BIN_WIDTH_E8"])?;
    let allowed_profiles = parse_allowed_profiles(&map["GATEWAY_ALLOWED_PROFILES"])?;

    // Связь тройки: прогреваемый селектор ОБЯЗАН входить в `ALLOWED_PROFILES`,
    // иначе прогревается слепок, на который подписаться нельзя (милестоун §3.2).
    check_triple_in_allowed(timeframe_ms, window_ms, depth_cadence_ms, &allowed_profiles)?;

    Ok(AppliedProfile {
        raw: map,
        version,
        sha256,
        path: path.to_path_buf(),
        bands,
        timeframe_ms,
        window_ms,
        depth_cadence_ms,
        heatmap_window_frac,
        vp_bin_width_e8,
        allowed_profiles,
    })
}

// ─────────────────────────── ШЕСТЬ разборщиков (публичные, единственный дом) ───────────────────────────
// M-94 задача 12 (`R-245` B-1; спека §3.2 «Одна грамматика на величину»): каждое значение
// судится ОДНОЙ функцией; её зовут загрузчик профиля и ветки «без профиля» обоих бинарей
// (`serve_config_from_env`, `gateway-checkpoint`: окружение и флаги). Никакой второй копии
// разбора (`A-049` Р-6, §5 запрет).
//
// Грамматика — та, что у загрузчика сегодня (строже нынешней ветки без профиля; ужесточение
// fail-closed, прод на нём не стоит после M-94). Различие режимов — только политика профиля
// (§3.2 (б): `GATEWAY_WINDOW_MS=0` в профиле — отказ с именем ключа; вне профиля — offline).

/// `GATEWAY_BANDS` — comma-separated float'ы, никаких пустых записей. Правило:
/// каждое значение — конечное (НЕ NaN, НЕ ±∞), `> 0`. Пустой список или пустой
/// элемент ⇒ отказ.
pub fn parse_bands(s: &str) -> Result<Vec<f64>, ProfileError> {
    let mut out = Vec::new();
    for p in s.split(',') {
        let t = p.trim();
        if t.is_empty() {
            return Err(ProfileError::BadValue {
                key: "GATEWAY_BANDS".to_string(),
                reason: format!("empty entry in {s:?}"),
            });
        }
        let v: f64 = t.parse().map_err(|e| ProfileError::BadValue {
            key: "GATEWAY_BANDS".to_string(),
            reason: format!("{t:?} not a float ({e})"),
        })?;
        if !v.is_finite() || v <= 0.0 {
            return Err(ProfileError::BadValue {
                key: "GATEWAY_BANDS".to_string(),
                reason: format!("{v} must be > 0 and finite"),
            });
        }
        out.push(v);
    }
    if out.is_empty() {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_BANDS".to_string(),
            reason: "empty list".to_string(),
        });
    }
    Ok(out)
}

/// `GATEWAY_TIMEFRAME_MS` — i64, делит 86_400_000 нацело, `> 0`. Парсер тот же
/// для режима профиля и для режима без профиля (`R-245` B-1). Сообщение отказа
/// называет ключ (оракул `g1`: «1000 ⇒ принят; 999, 0, -1000, abc — нет»).
pub fn parse_timeframe_ms(s: &str) -> Result<i64, ProfileError> {
    let v: i64 = s.trim().parse().map_err(|e| ProfileError::BadValue {
        key: "GATEWAY_TIMEFRAME_MS".to_string(),
        reason: format!("{s:?} not i64 ({e})"),
    })?;
    if v <= 0 {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_TIMEFRAME_MS".to_string(),
            reason: format!("{v} must be > 0 (GW-I-10: иначе бакет пересекает 00:00 UTC)"),
        });
    }
    if 86_400_000 % v != 0 {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_TIMEFRAME_MS".to_string(),
            reason: format!(
                "{v} не выравнен на границу UTC-суток \
                 (требуется 86_400_000 % v == 0)"
            ),
        });
    }
    Ok(v)
}

/// `GATEWAY_WINDOW_MS` — i64 `>= 0`. `0` грамматически ДОПУСТИМ и означает
/// «нет окна» (M-37 / `C-099` B-2): в режиме профиля это отвергается ПОЛИТИКОЙ
/// (`load_profile` отдельно), вне профиля `serve_config_from_env` канонизирует
/// в `None`. Отрицательное — отказ с именем ключа. Один грамматический путь —
/// и в профиле, и без (спека §3.2 (а)).
pub fn parse_window_ms(s: &str) -> Result<i64, ProfileError> {
    let v: i64 = s.trim().parse().map_err(|e| ProfileError::BadValue {
        key: "GATEWAY_WINDOW_MS".to_string(),
        reason: format!("{s:?} not i64 ({e})"),
    })?;
    if v < 0 {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_WINDOW_MS".to_string(),
            reason: format!(
                "{v} отрицателен — окно должно быть либо unset (offline), \
                 либо положительным числом миллисекунд; 0 — легитимный offline"
            ),
        });
    }
    Ok(v)
}

/// `GATEWAY_DEPTH_CADENCE_MS` — i64, `>= 1000` и делит 86_400_000 нацело
/// (`MD-I-8` d14: подсекундный интервал даёт ОДИН ключ в секунду молча).
/// Парсер тот же для режима профиля и для режима без профиля.
pub fn parse_depth_cadence_ms(s: &str) -> Result<i64, ProfileError> {
    let v: i64 = s.trim().parse().map_err(|e| ProfileError::BadValue {
        key: "GATEWAY_DEPTH_CADENCE_MS".to_string(),
        reason: format!("{s:?} not i64 ({e})"),
    })?;
    if v < 1000 {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_DEPTH_CADENCE_MS".to_string(),
            reason: format!(
                "{v} подсекундная — проводная форма ключуется секундами \
                 (DepthRow.series — time_s), требуется >= 1000 (MD-I-8 d14)"
            ),
        });
    }
    if 86_400_000 % v != 0 {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_DEPTH_CADENCE_MS".to_string(),
            reason: format!(
                "{v} не выравнен на границу UTC-суток \
                 (требуется 86_400_000 % v == 0)"
            ),
        });
    }
    Ok(v)
}

/// `GATEWAY_HEATMAP_WINDOW` — f64 в (0, 1). Сообщение отказа называет ключ (оракул `p5`,
/// «окно heatmap 1.5»). Тот же грамматический путь в обоих режимах (`R-245` B-1).
pub fn parse_heatmap_window_frac(s: &str) -> Result<f64, ProfileError> {
    let v: f64 = s.trim().parse().map_err(|e| ProfileError::BadValue {
        key: "GATEWAY_HEATMAP_WINDOW".to_string(),
        reason: format!("{s:?} not a float ({e})"),
    })?;
    if !v.is_finite() || v <= 0.0 || v >= 1.0 {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_HEATMAP_WINDOW".to_string(),
            reason: format!(
                "{v} вне интервала (0, 1); окно heatmap/COB должно быть \
                 положительным и меньше 1"
            ),
        });
    }
    Ok(v)
}

/// `GATEWAY_VP_BIN_WIDTH_E8` — i64 > 0. Сообщение отказа называет ключ (оракул `p5`,
/// «шаг VP 0»).
pub fn parse_vp_bin_width_e8(s: &str) -> Result<i64, ProfileError> {
    let v: i64 = s.trim().parse().map_err(|e| ProfileError::BadValue {
        key: "GATEWAY_VP_BIN_WIDTH_E8".to_string(),
        reason: format!("{s:?} not a i64 ({e})"),
    })?;
    if v <= 0 {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_VP_BIN_WIDTH_E8".to_string(),
            reason: format!("{v} must be > 0"),
        });
    }
    Ok(v)
}

/// `GATEWAY_ALLOWED_PROFILES` — список `tf/window[/cadence]` через запятую. `cadence`
/// — `none` или число. Сообщение отказа называет ключ.
pub fn parse_allowed_profiles(s: &str) -> Result<Vec<RawLiveProfile>, ProfileError> {
    let mut out = Vec::new();
    for entry in s.split(',') {
        let t = entry.trim();
        if t.is_empty() {
            return Err(ProfileError::BadValue {
                key: "GATEWAY_ALLOWED_PROFILES".to_string(),
                reason: format!("empty entry in {s:?}"),
            });
        }
        let parts: Vec<&str> = t.split('/').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(ProfileError::BadValue {
                key: "GATEWAY_ALLOWED_PROFILES".to_string(),
                reason: format!("{t:?} must be `tf/window[/cadence]` (1–3 slash-separated parts)"),
            });
        }
        let tf: i64 = parts[0]
            .trim()
            .parse()
            .map_err(|e| ProfileError::BadValue {
                key: "GATEWAY_ALLOWED_PROFILES".to_string(),
                reason: format!("timeframe_ms {:?} not i64 ({e})", parts[0]),
            })?;
        let window: i64 = parts[1]
            .trim()
            .parse()
            .map_err(|e| ProfileError::BadValue {
                key: "GATEWAY_ALLOWED_PROFILES".to_string(),
                reason: format!("window_ms {:?} not i64 ({e})", parts[1]),
            })?;
        let cadence: Option<i64> = match parts.get(2).map(|s| s.trim()) {
            None => None,
            Some(c) if c.eq_ignore_ascii_case("none") => None,
            Some(c) => Some(c.trim().parse().map_err(|e| ProfileError::BadValue {
                key: "GATEWAY_ALLOWED_PROFILES".to_string(),
                reason: format!("depth_cadence_ms {c:?} not i64 ({e})"),
            })?),
        };
        out.push(RawLiveProfile {
            timeframe_ms: tf,
            window_ms: window,
            depth_cadence_ms: cadence,
        });
    }
    if out.is_empty() {
        return Err(ProfileError::BadValue {
            key: "GATEWAY_ALLOWED_PROFILES".to_string(),
            reason: "empty list".to_string(),
        });
    }
    Ok(out)
}

/// Связь «тройка ∈ `ALLOWED_PROFILES`» — отдельный разборщик, потому что без него
/// профильный прогреватель может собрать слепок, на который НИКТО не подпишется
/// (милестоун §3.2).
pub fn check_triple_in_allowed(
    tf: i64,
    window: i64,
    cadence: i64,
    allowed: &[RawLiveProfile],
) -> Result<(), ProfileError> {
    let want_cadence = Some(cadence);
    let hit = allowed.iter().any(|p| {
        p.timeframe_ms == tf && p.window_ms == window && p.depth_cadence_ms == want_cadence
    });
    if hit {
        Ok(())
    } else {
        Err(ProfileError::TripleNotInAllowed {
            tf,
            window,
            cadence,
        })
    }
}

// ─────────────────────────── Статическая публикация применённого профиля ───────────────────────────

use std::sync::OnceLock;

/// `OnceLock<(version, sha256)>` — единственная точка, где сердцебиение выдачи
/// (`crates/gateway-serve/src/lib.rs::run_heartbeat`) узнаёт применённую версию.
/// Устанавливается ОДИН раз при старте `serve_config_from_env`; читается на каждом
/// такте heartbeat'а. `None` ⇔ режим без профиля (legacy env).
///
/// Поле — НЕ в `ServeConfig`: добавление ломает sacred-тесты с фиксированной формой
/// литерала `ServeConfig { ... }` (милестоун §5, запрет на дублирование структуры).
static EFFECTIVE_CALC_PROFILE: OnceLock<(u32, String)> = OnceLock::new();

/// Установить применённый профиль для heartbeat'а. Зовётся `serve_config_from_env`
/// ровно один раз после успешного разбора. `None` ⇔ legacy-режим (heartbeat пишет
/// `calc_profile` как `null`, чего оракул `s1` НЕ проверяет, но `o1` — сторож
/// не-ломающегося разбора, и `null` валиден: поле аддитивно).
pub fn set_effective_calc_profile(version: u32, sha256: String) {
    let _ = EFFECTIVE_CALC_PROFILE.set((version, sha256));
}

/// Прочитать применённый профиль. `None` ⇔ legacy-режим (heartbeat НЕ включает поле).
pub fn effective_calc_profile() -> Option<(u32, String)> {
    EFFECTIVE_CALC_PROFILE.get().map(|(v, s)| (*v, s.clone()))
}

// ─────────────────────────── Удобства для gateway-serve (env-чтение) ───────────────────────────

/// Загрузить профиль, если `GATEWAY_CALC_PROFILE` через getter отдаёт непустое
/// значение. Пустая строка и `None` ⇔ legacy-режим (НЕ ошибка). Милестоун §3.3
/// «`GATEWAY_CALC_PROFILE` НЕ задана ⇒ прежнее поведение».
pub fn load_from_env(
    get: impl Fn(&str) -> Option<String>,
) -> Result<Option<AppliedProfile>, ProfileError> {
    match get("GATEWAY_CALC_PROFILE") {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => Ok(Some(load_profile(Path::new(s.trim()))?)),
    }
}

/// Проверить восемь ключей профиля + `GATEWAY_CANONICAL_BANDS` в окружении.
/// В режиме профиля — «наличие — отказ» (`П-032` п.1, оракулы `p3`/`s2`); значение
/// проверяемой переменной НЕ важно (даже совпадение с профилем — отказ).
/// Возвращает `Err(String)` с ИМЕНЕМ ключа; сообщение упоминает `GATEWAY_CALC_PROFILE`
/// (дополнительно вшито в текст отказа, чтобы «наличие — отказ» было видно
/// оператору сразу).
pub fn check_no_env_overrides(get: &impl Fn(&str) -> Option<String>) -> Result<(), String> {
    for k in PROFILE_AND_LEGACY_KEYS {
        if get(k).is_some() {
            return Err(format!(
                "{k} must not be set in env when GATEWAY_CALC_PROFILE is set \
                 (П-032 п.1: «наличие — отказ», даже значение равное профилю)"
            ));
        }
    }
    Ok(())
}

/// Проверить, что в argv нет флагов оси (`--bands`/`--timeframe-ms`/`--window-ms`/
/// `--depth-cadence-ms`). Поддерживает ОБЕ формы (`--flag value` и `--flag=value`).
/// Оракул `p4`.
pub fn check_no_axis_flags(argv: &[String]) -> Result<(), String> {
    for a in argv {
        for f in PROFILE_AXIS_FLAGS {
            if a == f || a.starts_with(&format!("{f}=")) {
                return Err(format!(
                    "{f} must not be set on argv when GATEWAY_CALC_PROFILE is set \
                     (П-032 п.1: «наличие — отказ», определение оси — в профиле)"
                ));
            }
        }
    }
    Ok(())
}

/// Канонический набор полос = полосы профиля (милестоун §3.1, последний абзац):
/// в режиме профиля `canonical_bands` НЕ читается из `GATEWAY_CANONICAL_BANDS`,
/// а БЕРЁТСЯ из `profile.bands`. Вне режима профиля — старый путь, env/dep.
pub fn canonical_bands_or_profile(
    get: &impl Fn(&str) -> Option<String>,
    profile: Option<&AppliedProfile>,
) -> Result<Vec<f64>, String> {
    if let Some(p) = profile {
        return Ok(p.bands.clone());
    }
    let raw = get("GATEWAY_CANONICAL_BANDS")
        .unwrap_or_else(|| "0.015,0.03,0.05,0.08,0.15,0.30,0.60".to_string());
    let mut out = Vec::new();
    for p in raw.split(',') {
        let t = p.trim();
        if t.is_empty() {
            return Err(format!("GATEWAY_CANONICAL_BANDS: empty entry in {raw:?}"));
        }
        let v: f64 = t
            .parse()
            .map_err(|e| format!("GATEWAY_CANONICAL_BANDS parse: {e}"))?;
        out.push(v);
    }
    if out.is_empty() {
        return Err("GATEWAY_CANONICAL_BANDS must contain at least one band".to_string());
    }
    Ok(out)
}

// R-245 N-4: `applied_profile_optional_io` — мёртвая заглушка «на будущее» под
// `#[allow(dead_code)]`. Удалена: держать ради воображаемого потокового чтения
// значит держать неиспользуемый публичный символ; если расширение понадобится,
// он вернётся с осмысленной сигнатурой и тестом.

#[cfg(test)]
mod tests {
    use super::*;

    fn write_profile(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn closed_set_accepts_v1() {
        let dir = tempfile::tempdir().unwrap();
        let body = "\
CALC_PROFILE_VERSION=1
GATEWAY_BANDS=0.015,0.03,0.05,0.08,0.15,0.3,0.6
GATEWAY_DEPTH_CADENCE_MS=1000
GATEWAY_TIMEFRAME_MS=1000
GATEWAY_WINDOW_MS=60000
GATEWAY_ALLOWED_PROFILES=1000/60000/1000
GATEWAY_VP_BIN_WIDTH_E8=25000000
GATEWAY_HEATMAP_WINDOW=0.001
";
        let p = write_profile(dir.path(), "p.env", body);
        let ap = load_profile(&p).expect("v1 loads");
        assert_eq!(ap.version, 1);
        assert_eq!(ap.bands.len(), 7);
        assert_eq!(ap.depth_cadence_ms, 1000);
        assert_eq!(ap.heatmap_window_frac, 0.001);
        assert_eq!(ap.vp_bin_width_e8, 25_000_000);
        assert_eq!(ap.allowed_profiles.len(), 1);
        assert_eq!(ap.allowed_profiles[0].timeframe_ms, 1000);
    }

    #[test]
    fn unknown_key_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let body = "\
CALC_PROFILE_VERSION=1
GATEWAY_BANDS=0.015
GATEWAY_DEPTH_CADENCE_MS=1000
GATEWAY_TIMEFRAME_MS=1000
GATEWAY_WINDOW_MS=60000
GATEWAY_ALLOWED_PROFILES=1000/60000/1000
GATEWAY_VP_BIN_WIDTH_E8=25000000
GATEWAY_HEATMAP_WINDOW=0.001
GATEWAY_FOO=1
";
        let p = write_profile(dir.path(), "p.env", body);
        let err = load_profile(&p).expect_err("unknown key");
        assert!(matches!(err, ProfileError::UnknownKey(ref s) if s == "GATEWAY_FOO"));
    }

    #[test]
    fn duplicate_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let body = "\
CALC_PROFILE_VERSION=1
GATEWAY_BANDS=0.015
GATEWAY_BANDS=0.03
GATEWAY_DEPTH_CADENCE_MS=1000
GATEWAY_TIMEFRAME_MS=1000
GATEWAY_WINDOW_MS=60000
GATEWAY_ALLOWED_PROFILES=1000/60000/1000
GATEWAY_VP_BIN_WIDTH_E8=25000000
GATEWAY_HEATMAP_WINDOW=0.001
";
        let p = write_profile(dir.path(), "p.env", body);
        let err = load_profile(&p).expect_err("dup");
        assert!(matches!(err, ProfileError::Duplicate(ref s) if s == "GATEWAY_BANDS"));
    }

    #[test]
    fn version_zero_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let body = "\
CALC_PROFILE_VERSION=0
GATEWAY_BANDS=0.015
GATEWAY_DEPTH_CADENCE_MS=1000
GATEWAY_TIMEFRAME_MS=1000
GATEWAY_WINDOW_MS=60000
GATEWAY_ALLOWED_PROFILES=1000/60000/1000
GATEWAY_VP_BIN_WIDTH_E8=25000000
GATEWAY_HEATMAP_WINDOW=0.001
";
        let p = write_profile(dir.path(), "p.env", body);
        let err = load_profile(&p).expect_err("version 0");
        assert!(matches!(err, ProfileError::BadVersion(_)));
    }

    #[test]
    fn triple_outside_allowed_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let body = "\
CALC_PROFILE_VERSION=1
GATEWAY_BANDS=0.015
GATEWAY_DEPTH_CADENCE_MS=1000
GATEWAY_TIMEFRAME_MS=1000
GATEWAY_WINDOW_MS=60000
GATEWAY_ALLOWED_PROFILES=1000/300000/1000
GATEWAY_VP_BIN_WIDTH_E8=25000000
GATEWAY_HEATMAP_WINDOW=0.001
";
        let p = write_profile(dir.path(), "p.env", body);
        let err = load_profile(&p).expect_err("triple");
        assert!(matches!(err, ProfileError::TripleNotInAllowed { .. }));
    }

    #[test]
    fn heatmap_out_of_range_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let body = "\
CALC_PROFILE_VERSION=1
GATEWAY_BANDS=0.015
GATEWAY_DEPTH_CADENCE_MS=1000
GATEWAY_TIMEFRAME_MS=1000
GATEWAY_WINDOW_MS=60000
GATEWAY_ALLOWED_PROFILES=1000/60000/1000
GATEWAY_VP_BIN_WIDTH_E8=25000000
GATEWAY_HEATMAP_WINDOW=1.5
";
        let p = write_profile(dir.path(), "p.env", body);
        let err = load_profile(&p).expect_err("heatmap 1.5");
        match err {
            ProfileError::BadValue { key, .. } => assert_eq!(key, "GATEWAY_HEATMAP_WINDOW"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn vp_zero_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let body = "\
CALC_PROFILE_VERSION=1
GATEWAY_BANDS=0.015
GATEWAY_DEPTH_CADENCE_MS=1000
GATEWAY_TIMEFRAME_MS=1000
GATEWAY_WINDOW_MS=60000
GATEWAY_ALLOWED_PROFILES=1000/60000/1000
GATEWAY_VP_BIN_WIDTH_E8=0
GATEWAY_HEATMAP_WINDOW=0.001
";
        let p = write_profile(dir.path(), "p.env", body);
        let err = load_profile(&p).expect_err("vp 0");
        match err {
            ProfileError::BadValue { key, .. } => assert_eq!(key, "GATEWAY_VP_BIN_WIDTH_E8"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn sha256_independent_of_values() {
        // sha256 — от БАЙТОВ файла, а не от разобранных значений: смена комментария
        // меняет байты ⇒ меняет хеш. Защита от «содержательно одинаковые ⇒ хеш
        // одинаковый» (милестоун §3.6, p6).
        let dir = tempfile::tempdir().unwrap();
        let a = write_profile(
            dir.path(),
            "a.env",
            "# header\nCALC_PROFILE_VERSION=1\nGATEWAY_BANDS=0.015\n\
             GATEWAY_DEPTH_CADENCE_MS=1000\nGATEWAY_TIMEFRAME_MS=1000\n\
             GATEWAY_WINDOW_MS=60000\nGATEWAY_ALLOWED_PROFILES=1000/60000/1000\n\
             GATEWAY_VP_BIN_WIDTH_E8=25000000\nGATEWAY_HEATMAP_WINDOW=0.001\n",
        );
        let b = write_profile(
            dir.path(),
            "b.env",
            "# другой комментарий\nCALC_PROFILE_VERSION=1\nGATEWAY_BANDS=0.015\n\
             GATEWAY_DEPTH_CADENCE_MS=1000\nGATEWAY_TIMEFRAME_MS=1000\n\
             GATEWAY_WINDOW_MS=60000\nGATEWAY_ALLOWED_PROFILES=1000/60000/1000\n\
             GATEWAY_VP_BIN_WIDTH_E8=25000000\nGATEWAY_HEATMAP_WINDOW=0.001\n",
        );
        let ap_a = load_profile(&a).unwrap();
        let ap_b = load_profile(&b).unwrap();
        assert_ne!(ap_a.sha256, ap_b.sha256);
    }
}
