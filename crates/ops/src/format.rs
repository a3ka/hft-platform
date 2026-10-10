//! Человекочитаемое форматирование `watchdog::Alert` для транспорта (`transport.rs`).
//!
//! Требование задачи: сообщение должно быть понятно спросонья — что сломалось, когда,
//! какие цифры, что делать. `watchdog::Alert.message` уже несёт цифры (собраны детектором,
//! который их и вычислил); здесь добавляется заголовок с уровнем/кодом, метка хоста, момент
//! обнаружения и ссылка на runbook. Чистая функция (`detected_at_ms` — параметр, не
//! `SystemTime::now()`) — детерминированный рендер, тестируется без реальных часов.

use crate::watchdog::{Alert, Incident};

/// Путь до runbook'а (относительно корня репозитория) — упоминается в каждом сообщении,
/// чтобы "что делать" не приходилось придумывать спросонья. Якорь — код инцидента в нижнем
/// регистре (см. `docs/runbooks/alerting.md`, разделы озаглавлены кодами).
pub const RUNBOOK_PATH: &str = "docs/runbooks/alerting.md";

/// Собрать итоговое сообщение. `host` — метка источника (например `hft-recorder@vps`),
/// `detected_at_ms` — момент обнаружения (epoch ms, часы снаружи).
pub fn format_alert(alert: &Alert, host: &str, detected_at_ms: i64) -> String {
    let ts = format_utc_ts(detected_at_ms);
    let anchor = anchor_for(alert.incident);
    format!(
        "[{level}] {code} — {message}\nhost: {host}\nобнаружено: {ts}\nrunbook: {RUNBOOK_PATH}#{anchor}",
        level = alert.level.label(),
        code = alert.incident.code(),
        message = alert.message,
    )
}

fn anchor_for(incident: Incident) -> String {
    incident.code().to_lowercase()
}

/// UTC ISO-8601 (`YYYY-MM-DDTHH:MM:SSZ`) из epoch-миллисекунд. Без `chrono` — формат
/// фиксирован и простой, реализован через civil-calendar алгоритм Ховарда Хиннанта
/// (`civil_from_days`, https://howardhinnant.github.io/date_algorithms.html), который не
/// тянет новую зависимость ради одного формата вывода.
pub fn format_utc_ts(ms: i64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_unix_ms(ms);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn civil_from_unix_ms(ms: i64) -> (i64, u32, u32, u32, u32, u32) {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let h = rem / 3600;
    let mi = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, m, d) = civil_from_days(days);
    (y, m, d, h as u32, mi as u32, s as u32)
}

/// Дни с эпохи Unix (1970-01-01 = 0) → (год, месяц[1..12], день[1..31]), пролептический
/// григорианский календарь. Корректен для любого `i64` (в т.ч. отрицательных дней).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    //! Узел-инварианты форматтера даты (публичный контракт — в
    //! `crates/ops/tests/red_ops_format.rs`, с реальными прод-замерами).
    use super::*;

    #[test]
    fn epoch_zero_is_1970() {
        assert_eq!(format_utc_ts(0), "1970-01-01T00:00:00Z");
    }
}
