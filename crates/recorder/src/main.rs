//! recorder — M-00 STUB.
//!
//! Назначение на этом этапе: доказать сквозной pipeline (сборка -> образ -> деплой на VPS
//! -> запуск с persistent-volume под журнал). НИЧЕГО рыночного пока не пишет.
//! Реальная запись HL L2 в журнал — M-01/P1.
//!
//! Пишет heartbeat-строку в файл под JOURNAL_DIR (том, переживающий редеплой) — так
//! healthcheck контейнера может проверить, что процесс жив, а persistent-том — что он
//! действительно монтируется и переживает рестарт.

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    // Каталог журнала — из env (в контейнере это смонтированный том), по умолчанию ./journal-data.
    let dir = std::env::var("JOURNAL_DIR").unwrap_or_else(|_| "./journal-data".to_string());
    let dir = PathBuf::from(dir);
    let _ = std::fs::create_dir_all(&dir);
    let hb_path = dir.join("recorder.heartbeat");

    println!(
        "recorder STUB v{} — journal_dir={} — M-00 pipeline proof (no market data yet)",
        env!("CARGO_PKG_VERSION"),
        dir.display()
    );

    // Демонстрируем, что contracts-крейт линкуется и версия схемы доступна.
    println!("contracts::SCHEMA_VERSION = {}", contracts::SCHEMA_VERSION);

    loop {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        if let Ok(mut f) = std::fs::File::create(&hb_path) {
            let _ = writeln!(f, "{}", now_ms);
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}
