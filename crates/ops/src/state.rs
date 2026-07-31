//! Состояние watchdog'а между запусками cron'а: дедуп-окно + предыдущий heartbeat-сэмпл +
//! предыдущие `RestartCount` контейнеров. Персистится как JSON рядом с cron-маркерами
//! (`WATCHDOG_STATE_PATH`, по умолчанию `/var/lib/hft/watchdog.state.json`).
//!
//! Дедупликация — по коду инцидента (+ опциональный суффикс цели: имя контейнера/маркера,
//! собирается вызывающим в `src/bin/ops-watchdog.rs`). Задача §2: "алерт, который приходит
//! слишком часто, перестают читать — это та же тишина, только шумная" — `should_fire`
//! подавляет повтор того же ключа в пределах окна; `clear` снимает запись, когда условие
//! вернулось к норме, чтобы следующий срыв не оказался подавлен остаточным окном от
//! предыдущего инцидента.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::watchdog::HeartbeatSample;

/// По умолчанию — 30 минут. При cron раз в 5 минут это не более одного алерта на условие
/// за полчаса, пока оно не устранено — достаточно громко, чтобы не потерять сигнал, и
/// достаточно редко, чтобы не превратиться в шум, который выключат.
pub const DEFAULT_DEDUP_WINDOW_MS: i64 = 30 * 60_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WatchdogState {
    #[serde(default)]
    pub last_fired_ms: HashMap<String, i64>,
    #[serde(default)]
    pub prev_heartbeat: Option<HeartbeatSample>,
    #[serde(default)]
    pub prev_check_ms: Option<i64>,
    #[serde(default)]
    pub prev_restart_counts: HashMap<String, u64>,
}

impl WatchdogState {
    /// Читает состояние с диска; отсутствие файла / битый JSON — молча стартуем с чистого
    /// состояния (первый прогон watchdog'а на проде выглядит так же, как и после сбойного
    /// прошлого прогона — не паникуем, просто теряем дедуп-память и prev-сэмпл на один цикл).
    pub fn load_or_default(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let body = serde_json::to_string_pretty(self)
            .expect("WatchdogState::serialize — не должно паниковать на собственных типах");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)
    }

    /// `true` — можно слать (ключ не подавлен окном); при этом ОБНОВЛЯЕТ `last_fired_ms[key]`
    /// на `now_ms`. `false` — тот же ключ уже слался внутри окна, состояние НЕ трогается
    /// (окно отсчитывается от первого срабатывания серии, не скользит на каждый повтор —
    /// иначе непрерывный инцидент подавлял бы себя вечно).
    pub fn should_fire(&mut self, key: &str, now_ms: i64, window_ms: i64) -> bool {
        match self.last_fired_ms.get(key) {
            Some(&last) if now_ms.saturating_sub(last) < window_ms => false,
            _ => {
                self.last_fired_ms.insert(key.to_string(), now_ms);
                true
            }
        }
    }

    /// Условие вернулось к норме — снять дедуп-запись этого ключа.
    pub fn clear(&mut self, key: &str) {
        self.last_fired_ms.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_via_tempfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/watchdog.state.json");
        let mut state = WatchdogState::default();
        state.should_fire("WD-HB-STALE", 1_000, DEFAULT_DEDUP_WINDOW_MS);
        state.save(&path).expect("save creates parent dirs");

        let loaded = WatchdogState::load_or_default(&path);
        assert_eq!(loaded, state);
    }

    #[test]
    fn load_or_default_on_missing_file_is_empty_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let state = WatchdogState::load_or_default(&path);
        assert_eq!(state, WatchdogState::default());
    }
}
