//! M-28 gateway-serve — WS-транспорт кокпита (market-плоскость, D1/D6).
//!
//! ТОНКАЯ IO-оболочка над детерминированной библиотекой `crates/gateway` (M-22): держит WS, тейлит
//! журнал, отдаёт snapshot+frames+replay. **Read-only, stateless по юзеру** — auth = ТОЛЬКО verify
//! подписанного JWT (без user-БД, GS-I-2). App-плоскость (Next.js+Postgres) — вне этого кода (D6).
//!
//! ЭТОТ ФАЙЛ (architect, sacred): контракт-типы + сигнатуры с `unimplemented!()`. Тела —
//! engine-dev (tasks 2-4). Wire-формат MVP — JSON (JS-декодируемо; postcard — Rust-only, НЕ годится
//! для фронта). Тяжёлый бинарь heatmap — отдельно (M-23, JS-декодируемый кодек, не postcard).

/// Stateless JWT-аутентификация (D6): верификация подписи, БЕЗ обращения в user-БД.
pub mod auth {
    use jsonwebtoken::DecodingKey;
    use serde::{Deserialize, Serialize};

    /// Клеймы токена, выпущенного Next.js (app-плоскость). `exp` — unix-секунды истечения.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Claims {
        pub sub: String,
        pub exp: usize,
    }

    /// Причина отказа авторизации (без утечки деталей наружу).
    #[derive(Debug)]
    pub enum AuthError {
        /// Подпись/формат невалидны или ключ чужой.
        Invalid,
        /// Токен истёк (`exp` в прошлом).
        Expired,
    }

    /// Верифицировать подписанный JWT. **Stateless (GS-I-2):** берёт ТОЛЬКО `(token, key)`, НЕ ходит в
    /// user-БД. Валидная подпись + не истёк → `Ok(Claims)`; иначе `Err`. engine-dev (M-28 task #2):
    /// `jsonwebtoken::decode` с `Validation` (проверка `exp`), алгоритм HS256 (Ed25519 — по founder).
    pub fn verify_token(token: &str, key: &DecodingKey) -> Result<Claims, AuthError> {
        let _ = (token, key);
        unimplemented!("M-28 task #2 (engine-dev): stateless jsonwebtoken decode + validate exp")
    }
}

/// Wire-конверт сообщений WS (MVP — JSON, версионирован через `schema_version` внутри Snapshot/Frame).
pub mod wire {
    use gateway::{Frame, Snapshot};
    use serde::{Deserialize, Serialize};

    /// Сообщение сервер→клиент. JSON (JS-декодируемо). Тяжёлый бинарь (heatmap) — отдельный кодек (M-23).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub enum ServeMsg {
        Snapshot(Snapshot),
        Frame(Frame),
        Error(String),
    }
}

/// Serve-adapter — ТОНКИЙ passthrough над `gateway::{snapshot,frames_since}` (GS-I-5: без трансформации
/// серий → live==replay цел). engine-dev (M-28 task #3).
pub mod serve {
    use std::io;
    use std::path::Path;

    use gateway::{Cursor, Selector};
    use journal::EpochFilter;

    use super::wire::ServeMsg;

    /// Снапшот-при-подключении: `gateway::snapshot(..)` → `ServeMsg::Snapshot`. Read-only.
    pub fn snapshot_msg(
        dir: impl AsRef<Path>,
        filter: EpochFilter,
        sel: &Selector,
        at: Cursor,
    ) -> io::Result<ServeMsg> {
        let _ = (dir.as_ref(), filter, sel, at);
        unimplemented!("M-28 task #3 (engine-dev): wrap gateway::snapshot → ServeMsg::Snapshot")
    }

    /// Инкрементальные кадры: `gateway::frames_since(..)` → `Vec<ServeMsg::Frame>` + новый курсор.
    /// РОВНО те же кадры, что библиотека (GS-I-5). Bounded (GS-I-2 наследуется от frames_since).
    pub fn frames_msgs(
        dir: impl AsRef<Path>,
        filter: EpochFilter,
        sel: &Selector,
        after: Cursor,
        max_events: usize,
    ) -> io::Result<(Vec<ServeMsg>, Cursor)> {
        let _ = (dir.as_ref(), filter, sel, after, max_events);
        unimplemented!(
            "M-28 task #3 (engine-dev): wrap gateway::frames_since → Vec<ServeMsg::Frame>"
        )
    }
}

/// WS-сервер (bin-путь, task #4). ТОНКАЯ IO-оболочка: accept → verify JWT (`auth::verify_token`) →
/// snapshot (`serve::snapshot_msg`) + инкрементальный push (`serve::frames_msgs`) + replay. Read-only,
/// stateless по юзеру. Токен передаётся клиентом в query (`?token=<jwt>`). Тела — engine-dev (task #4).
pub mod server {
    use std::net::SocketAddr;
    use std::path::PathBuf;

    use gateway::Selector;
    use journal::EpochFilter;
    use jsonwebtoken::DecodingKey;

    /// Конфиг сервиса (bin читает из env/args). MVP — одна `(venue, symbol)`; мульти-подписка позже.
    pub struct ServeConfig {
        /// Адрес bind, напр. `"127.0.0.1:8080"` или `"127.0.0.1:0"` (ephemeral для тестов).
        pub addr: String,
        pub journal_dir: PathBuf,
        pub filter: EpochFilter,
        pub selector: Selector,
        /// Ключ верификации JWT (выпущен Next.js; D6). Stateless — без user-БД.
        pub decoding_key: DecodingKey,
    }

    /// Забинденный сервер, готовый принимать WS. `local_addr` даёт реальный порт (для ephemeral-тестов).
    pub struct Server {
        _private: (),
    }

    /// Забиндить WS-listener на `cfg.addr`. engine-dev (task #4): `tokio::net::TcpListener`.
    pub async fn bind(cfg: ServeConfig) -> std::io::Result<Server> {
        let _ = cfg;
        unimplemented!("M-28 task #4 (engine-dev): bind WS listener (tokio) на cfg.addr")
    }

    impl Server {
        /// Фактический адрес (ephemeral-порт разрешён в реальный) — для smoke-теста.
        pub fn local_addr(&self) -> SocketAddr {
            unimplemented!("M-28 task #4 (engine-dev): listener.local_addr()")
        }

        /// Accept-loop: на соединение — verify JWT из query; успех → snapshot + push + replay; провал →
        /// закрыть с отказом. Read-only (GS-I-3): приём фрейма = только replay-контролы, не запись.
        pub async fn serve(self) -> std::io::Result<()> {
            unimplemented!("M-28 task #4 (engine-dev): accept loop + per-conn snapshot/push/replay")
        }
    }
}
