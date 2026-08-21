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
    use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
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
        // GS-I-2: stateless HS256 + exp-проверка. `Validation::new(HS256)` дефолтно валидирует
        // подпись (signature) + exp (с leeway=60s, см. `Validation::leeway`); `required_spec_claims`
        // = {"exp"}. Никаких extra-полей (iss/aud/sub) — мы НЕ доверяем claim-метаданным Next.js для
        // авторизации, только самой подписи. `validate_aud = true` дефолтно, но `aud = None`
        // значит «не сверять aud» (если бы `aud` присутствовал в токене, было бы несовпадение →
        // отказ; в нашем случае Next.js выпускает токены БЕЗ `aud`, так что всё чисто).
        let validation = Validation::new(Algorithm::HS256);
        match decode::<Claims>(token, key, &validation) {
            Ok(data) => Ok(data.claims),
            Err(e) => match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => Err(AuthError::Expired),
                // Всё остальное (InvalidToken/InvalidSignature/InvalidAlgorithm/MissingRequiredClaim/
                // Base64/Json/Utf8/Crypto/…) мапим в `Invalid` — наружу не утекают детали
                // (GS-I-2: «без утечки деталей наружу»).
                _ => Err(AuthError::Invalid),
            },
        }
    }
}

// Wire-конверт сообщений WS (MVP — JSON, версионированный через `schema_version` внутри Snapshot/Frame). (M-65: per-connection subscription state + v1 wire protocol)//
// Хранятся рядом с `auth`/`wire`/`serve`/`server`, чтобы внешняя поверхность крейта
// (паблик-импорты в `bin/wsprobe.rs` и RED-тестах) осталась прежней. Внутренние модули
// движка WS-сессии — внутренняя деталь: видимы публично, но НЕ экспортируются из binary-путей.

/// Per-WS-session subscription state (`CT-RFC-09` §2 — M-65).
pub mod session;

/// v1 wire protocol — парсинг клиентских сообщений и сериализация ответов (§2.2/§2.3).
pub mod wire_v1;

/// M-65 round 3 (R-086 §10.3): rendezvous-точка синхронизации для оракула на гонку
/// «switch × in-flight pump». В ТЕСТОВОЙ сборке pump (внутри `spawn_blocking`)
/// СИГНАЛИТ «вошёл» и ЖДЁТ разрешения; в прод-пути (compile без `--test`) модуль
/// НЕ компилируется и на семантику пути не влияет — строгая граница через
/// `#[cfg(test)]` на уровне `pub mod rendezvous`.
///
/// Сводный контракт и cleanup-политика — в шапке `test_sync.rs`. Тест, использующий
/// `rendezvous`, вызывает `arm(id)` перед сценарием, `test_wait_for_pump(id, ..)`
/// для синхронизации, `test_release(id)` чтобы pump продолжил, и `test_remove(id)`
/// после сценария.
#[cfg(any(test, feature = "testing"))]
pub mod test_sync;

pub mod wire {
    use crate::_gw::{Frame, Snapshot};
    use serde::{Deserialize, Serialize};

    /// Сообщение сервер→клиент. JSON (JS-декодируемо). Тяжёлый бинарь (heatmap) — отдельный кодек (M-23).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub enum ServeMsg {
        Snapshot(Snapshot),
        Frame(Frame),
        Error(String),
    }
}

/// Serve-adapter — ТОНКИЙ passthrough над `gateway::{snapshot_from_checkpoint,frames_since}`
/// (GS-I-5: без трансформации серий → live==replay цел). engine-dev (M-28 task #3 + M-38b #15).
pub mod serve {
    use std::io;
    use std::path::Path;

    use crate::_gw::{
        frames_since as gw_frames_since, snapshot_from_checkpoint, Cursor, ReadStats, Selector,
    };
    use journal::EpochFilter;

    use super::wire::ServeMsg;

    /// Снапшот-при-подключении с УЧЁТОМ чекпоинта: `gateway::snapshot_from_checkpoint(..)`
    /// → `(ServeMsg, ReadStats)`. Read-only.
    ///
    /// M-38b (rev4, B3): без чекпоинта путь сводился к `gateway::snapshot` (= O(история);
    /// 409.74 s на проде). С чекпоинтом `snapshot_from_checkpoint`:
    /// - читает валидный чекпоинт, валидирует header/CRC/lineage;
    /// - досчитывает хвост через `journal::stream_from(ckpt_cursor)` (GW-I-11);
    /// - любая невалидность чекпоинта → ТИХИЙ rebuild от START (GW-I-9(б));
    /// - возвращает честные `ReadStats{events_decoded, segments_opened}` — для §8 eyes-on.
    ///
    /// `ckpt_dir: Option<&Path>` — `None` = кэш не сконфигурирован (= прямой rebuild,
    /// единственный сценарий dev/test без прод-обвязки).
    /// На проде ВСЕГДА задан `GATEWAY_CHECKPOINT_DIR` через `serve_config_from_env`,
    /// compose монтирует `gateway-ckpt:/ckpt:ro` (писатель — только gateway-checkpoint
    /// ops-сервис; см. `docker-compose.yml`).
    ///
    /// GS-I-5: тонкая обёртка — НЕ трансформируем серии, НЕ пересортировываем, НЕ фильтруем.
    /// Байт-идентичность с `gateway::snapshot` гарантирована как для случая «с чекпоинтом»,
    /// так и для fallback’а (через transparent rebuild).
    pub fn snapshot_msg(
        dir: impl AsRef<Path>,
        filter: EpochFilter,
        sel: &Selector,
        at: Cursor,
        ckpt_dir: Option<&Path>,
    ) -> io::Result<(ServeMsg, ReadStats)> {
        let (snap, stats) = match ckpt_dir {
            Some(p) => snapshot_from_checkpoint(dir.as_ref(), filter, sel, p, at)?,
            None => snapshot_from_checkpoint(dir.as_ref(), filter, sel, Path::new(""), at)?,
            // "пустой путь" внутри `read_checkpoint` провалится в `ckpt_path.exists()`
            // и вернёт `Ok(None)` → rebuild; безопасный эквивалент «нет чекпоинта».
            // Альтернатива — рефакторить публичную сигнатуру `snapshot_from_checkpoint`
            // под `Option<&Path>`, но это касается слоя gateway (риск scope guard).
        };
        Ok((ServeMsg::Snapshot(snap), stats))
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
        // GS-I-5: тонкий passthrough — `Vec<Frame>` библиотеки → `Vec<ServeMsg::Frame>` 1-к-1.
        // НЕ фильтруем/перекодируем серии (анти-плацебо: `red_serve_passthrough.rs` сравнивает
        // поэлементно с `gateway::frames_since`).
        let (frames, new_cursor) = gw_frames_since(dir.as_ref(), filter, sel, after, max_events)?;
        let msgs: Vec<ServeMsg> = frames.into_iter().map(ServeMsg::Frame).collect();
        Ok((msgs, new_cursor))
    }
}

/// WS-сервер (bin-путь, task #4). ТОНКАЯ IO-оболочка: accept → verify JWT (`auth::verify_token`) →
/// snapshot (`serve::snapshot_msg`) + инкрементальный push (`serve::frames_msgs`) + replay. Read-only,
/// stateless по юзеру. Токен передаётся клиентом в query (`?token=<jwt>`). Тела — engine-dev (task #4).
pub mod server {
    use super::session; // M-65: per-connection subscription state
    use super::wire_v1; // M-65: v1 wire protocol (parse/serialize)

    use std::io;
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// M-65 (CT-RFC-09 §2.6): runtime-эффективный лимит подписок на соединение.
    /// Дефолт — 16 (подпись founder'а 11.08). `serve_config_from_env` обновляет значение
    /// ровно один раз при старте (fail-closed в `main` гарантирует, что плохое env не пройдёт).
    /// Тесты с фиксированной формой литерала `ServeConfig { .. }` (без `max_subs`) используют
    /// этот дефолт; единственный случай, где дефолт не подходит — ручная установка через
    /// `set_effective_max_subs` (для unit-тестов, проверяющих cap ниже/выше дефолта).
    static EFFECTIVE_MAX_SUBS: AtomicUsize = AtomicUsize::new(16);

    /// Получить runtime-лимит подписок (читается на каждом соединении).
    pub fn effective_max_subs() -> usize {
        EFFECTIVE_MAX_SUBS.load(Ordering::Relaxed)
    }

    /// Установить runtime-лимит (вызывается из `serve_config_from_env` и тестов).
    pub fn set_effective_max_subs(n: usize) {
        EFFECTIVE_MAX_SUBS.store(n, Ordering::Relaxed);
    }

    use crate::_gw::Selector;
    use futures_util::{SinkExt, StreamExt};
    use journal::EpochFilter;
    use jsonwebtoken::DecodingKey;
    use serde_json::Value;
    use tokio::net::{TcpListener, TcpStream};
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::WebSocketStream;

    use super::auth::{verify_token, AuthError};
    use super::wire::ServeMsg;

    /// Конфиг сервиса (bin читает из env/args). MVP — одна `(venue, symbol)`; мульти-подписка позже.
    pub struct ServeConfig {
        /// Адрес bind, напр. `"127.0.0.1:8080"` или `"127.0.0.1:0"` (ephemeral для тестов).
        pub addr: String,
        pub journal_dir: PathBuf,
        pub filter: EpochFilter,
        pub selector: Selector,
        /// Ключ верификации JWT (выпущен Next.js; D6). Stateless — без user-БД.
        pub decoding_key: DecodingKey,
        /// M-38b (rev4, B3): путь к каталогу чекпоинтов (`GATEWAY_CHECKPOINT_DIR` в env
        /// → монтируется `gateway-ckpt:/ckpt:ro` в compose). `None` = чекпоинт не сконфигурирован
        /// (= прямой rebuild, эквивалент `gateway::snapshot`; только для dev/test).
        /// На проде ВСЕГДА задан: без чекпоинта читаем всю историю при каждом коннекте —
        /// 409.74 s на 18 GB журнала (TD-044, ровно тот замер, который M-38b лечит).
        pub checkpoint_dir: Option<std::path::PathBuf>,
    }

    // === impl Clone для Spawn-per-conn (без изменения публичных полей ServeConfig) ===
    //
    // Architect зафиксировал поля (контракт-тип), но НЕ derive(Clone) — добавляем impl-блок,
    // чтобы per-connection task мог получить копию конфига без Arc-обвязки. Все поля
    // (`String`/`PathBuf`/`Selector`/`DecodingKey`/`EpochFilter`) — Clone; см. `Selector` уже
    // `#[derive(Clone)]`, `DecodingKey` Clone в `jsonwebtoken::decoding`, `EpochFilter` Clone в
    // `journal::segments`.
    impl Clone for ServeConfig {
        fn clone(&self) -> Self {
            Self {
                addr: self.addr.clone(),
                journal_dir: self.journal_dir.clone(),
                filter: self.filter.clone(),
                selector: self.selector.clone(),
                decoding_key: self.decoding_key.clone(),
                checkpoint_dir: self.checkpoint_dir.clone(),
            }
        }
    }

    /// Забинденный сервер, готовый принимать WS. `local_addr` даёт реальный порт (для ephemeral-тестов).
    ///
    /// Внутреннее устройство (engine-dev, task #4): хранит `TcpListener` + `Arc<ServeConfig>`,
    /// чтобы `serve()` мог спавнить per-connection таски с общим конфигом без `Mutex`.
    pub struct Server {
        listener: TcpListener,
        cfg: Arc<ServeConfig>,
    }

    /// Забиндить WS-listener на `cfg.addr`. engine-dev (task #4): `tokio::net::TcpListener`.
    pub async fn bind(cfg: ServeConfig) -> std::io::Result<Server> {
        // BIND: tokio TcpListener на `cfg.addr`. Поддерживает `127.0.0.1:0` (ephemeral для smoke).
        // Ошибки ОС (`AddrInUse`, `PermissionDenied`) пробрасываются как `io::Error` — bin
        // логирует и падает (не recoverable: bind-сбой = конфиг-сбой).
        let listener = TcpListener::bind(&cfg.addr).await?;
        Ok(Server {
            listener,
            cfg: Arc::new(cfg),
        })
    }

    impl Server {
        /// Фактический адрес (ephemeral-порт разрешён в реальный) — для smoke-теста.
        pub fn local_addr(&self) -> SocketAddr {
            self.listener
                .local_addr()
                .expect("listener bound; local_addr() is infallible post-bind")
        }

        /// Accept-loop: на соединение — verify JWT из query; успех → snapshot + push + replay; провал →
        /// закрыть с отказом. Read-only (GS-I-3): приём фрейма = только replay-контролы, не запись.
        /// Legacy-mode dispatcher: разбирает входящий Text/Binary КАК v1-сообщение и применяет
        /// к `v1_session_inner`. Тот же путь, что в `run_v1_session`, но живёт бок-о-бок с
        /// legacy env-stream (env-селектор из cfg + v1 subs со своими LiveReducer'ами в одной
        /// сессии). Сценарий M-65: клиент, не приславший `subscribe` в grace-окне, получил
        /// legacy snapshot; дальше ОН ЖЕ может отправить `subscribe` — это валидный сценарий
        /// (`CT-RFC-09` §2.8 «subscribe после окна ⇒ обычная смена инструмента»).
        ///
        /// Тонкий момент: F-035-2 уже гарантирован ОДНИМ экземпляром `SessionInner` на
        /// соединение; эта функция просто мутирует уже существующий, а не создаёт второй.
        pub async fn parse_and_dispatch_v1_message<W>(
            bytes: &[u8],
            inner: &mut SessionInner,
            sink: &mut futures_util::stream::SplitSink<WebSocketStream<W>, Message>,
        ) where
            W: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
        {
            match wire_v1::parse_message(bytes) {
                Ok(parsed) => {
                    let _ = handle_v1_message(&parsed, inner, sink).await;
                }
                Err(e) => {
                    let code = parse_error_code(&e);
                    let msg = parse_error_message(&e);
                    send_v1_error(sink, None, code, &msg).await;
                }
            }
        }

        pub async fn serve(self) -> std::io::Result<()> {
            // ACCEPT-LOOP: каждый TcpStream — в отдельном spawn-таске (как в recorder metrics_server).
            // Accept-сбой (listener закрыт) → WARN + retry с паузой 100ms (не спиним).
            loop {
                match self.listener.accept().await {
                    Ok((stream, _peer)) => {
                        let cfg = Arc::clone(&self.cfg);
                        tokio::spawn(async move {
                            if let Err(e) = handle_conn(stream, cfg).await {
                                tracing::debug!(error = %e, "gateway-serve conn ended with error");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "gateway-serve accept failed — retry");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }

    /// Per-connection: WS-handshake (с захватом URI для токена) → verify JWT → snapshot+push.
    /// На отказ (verify Err / handshake Err) — закрываем WS без Snapshot.
    async fn handle_conn(stream: TcpStream, cfg: Arc<ServeConfig>) -> std::io::Result<()> {
        // (1) Канал для передачи URI из handshake-коллбэка наружу.
        let (uri_tx, uri_rx) = tokio::sync::oneshot::channel::<Option<String>>();
        // tungstenite::handshake::server::{Request, Response, ErrorResponse}. `ErrorResponse` =
        // `HttpResponse<Option<String>>` — может вернуть текст ошибки при отказе. Мы НЕ
        // отказываем в коллбэке — откажем позже, ПОСЛЕ verify_token (handshake-completed +
        // close-with-error даёт клиенту семантически более чистый сигнал).
        //
        // `#[allow(clippy::result_large_err)]` — `ErrorResponse = HttpResponse<Option<String>>`
        // большой (~136 байт). Зеркалит сигнатуру tungstenite API; альтернатива (Box) усложняет
        // код без выигрыша (callback всегда вызывается синхронно внутри `accept_hdr_async`,
        // heap-аллокация на Err-пути не помогает). clippy::result_large_err здесь — false-positive.
        #[allow(clippy::result_large_err)]
        let callback = |req: &Request,
                        response: Response|
         -> Result<
            Response,
            tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
        > {
            // Извлекаем query (?token=<jwt>) — query() вернёт Some("token=<jwt>") или None.
            let query = req.uri().query().map(|s| s.to_string());
            // Канал oneshot — отправка best-effort (если receiver уже drop'нут — игнор).
            let _ = uri_tx.send(query);
            Ok(response)
        };

        // (2) Handshake через `accept_hdr_async` — позволяет увидеть URI до апгрейда.
        let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, callback).await {
            Ok(ws) => ws,
            Err(e) => {
                // Handshake-сбой (клиент не передал Upgrade, мусор, и т.п.) — тихо выходим.
                tracing::debug!(error = %e, "ws handshake failed");
                return Ok(());
            }
        };

        // (3) Достаём query из коллбэка. Если клиент не прислал query — отказ (невалидный путь).
        let query = match uri_rx.await {
            Ok(Some(q)) => q,
            _ => {
                close_with_error(ws_stream, "missing token query").await;
                return Ok(());
            }
        };

        // (4) Парсим query → token. Простой split('&')/split('='); JWT — base64url (без
        // percent-encoded символов), так что URL-decode не нужен. Если нет `token=...` — отказ.
        let token = match parse_token(&query) {
            Some(t) => t,
            None => {
                close_with_error(ws_stream, "missing token").await;
                return Ok(());
            }
        };

        // (5) Stateless JWT-verify. `Expired` vs `Invalid` наружу не утекают — клиенту один
        // общий `ServeMsg::Error("invalid token")`, в логах можно различить по уровню.
        let claims = match verify_token(&token, &cfg.decoding_key) {
            Ok(c) => c,
            Err(AuthError::Expired) => {
                tracing::debug!(sub = %"<jwt-claims>", "rejected: expired token");
                close_with_error(ws_stream, "expired token").await;
                return Ok(());
            }
            Err(AuthError::Invalid) => {
                tracing::debug!("rejected: invalid token (bad sig / wrong key / malformed)");
                close_with_error(ws_stream, "invalid token").await;
                return Ok(());
            }
        };
        tracing::debug!(sub = %claims.sub, "ws auth ok");

        // (6) Авторизован → dispatcher: legacy или v1 сессия.
        run_dispatched_session(ws_stream, cfg, claims).await
    }

    /// Отправить `ServeMsg::Error(msg)` как Text-фрейм и закрыть WS (best-effort).
    async fn close_with_error<S>(ws: WebSocketStream<S>, msg: &str)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let payload = match serde_json::to_vec(&ServeMsg::Error(msg.to_string())) {
            Ok(b) => b,
            Err(_) => return,
        };
        let text = match String::from_utf8(payload) {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut ws = ws;
        // Send Text, затем Close — клиент сначала получит Error-фрейм (smoke-тест его парсит и
        // видит «не Snapshot» → rejected), затем Close (EOF на next()).
        let _ = ws.send(Message::Text(text)).await;
        let _ = ws.close(None).await;
    }

    // ════════════════════════════════════════════════════════════════════════════
    // M-65 ws-session: диспетчер legacy/v1 + v1 session.
    //
    // Контракт выбора режима (`CT-RFC-09` §2.8):
    //   - grace-окно: `initial_subscribe_grace_ms` (250 мс по умолчанию, подпись founder'а 11.08);
    //   - если за окно НЕ пришло сообщение  ⇒ legacy (env-селектор, OLD wire);
    //   - если в окно пришёл `subscribe` с `v:1`  ⇒ v1-режим, env-селектор НЕ применяется,
    //     окружается мультиплекс подписок на одном соединении (`O-2, O-9, O-10`);
    //   - прочее (Ping/garbage/неизвестный op) в grace  ⇒ legacy (съедается как сегодня —
    //     мы продолжаем обслуживание env-селектором, не v1).
    //
    // В обоих режимах соседи подписок одного соединения — ЖИВЫ при отказе (`O-5`); все ошибки
    // выражены машиночитаемым `code` (`O-6`).
    // ════════════════════════════════════════════════════════════════════════════

    /// Состояние v1-сессии на одном соединении: владеет `LiveReducer`'ами (по одному на
    /// подписку), выдаёт их в `spawn_blocking` на pump-цикл и возвращает обратно.
    /// Один экземпляр на соединение (`F-035-2`).
    ///
    /// **M-65 round 2 (М-1/М-2 по `R-057`):** Эта структура — единственный носитель
    /// состояния подписок на WS-соединении. Раннее `session::Session` с методами
    /// `add`/`switch`/`remove` было мёртвой половиной (его `Session::new` не вызывался
    /// ни в одном месте, и инварианты шапки модуля дублировались здесь с тонкими
    /// расхождениями — те самые, что составили `R-057` Б-2/М-1). Round 2 удалил
    /// мёртвую структуру; модуль `session.rs` теперь содержит только тип `Sub` и
    /// валидатор `validate_selector`, которые эта структура действительно использует.
    ///
    /// **DET-I-1 (детерминированный выбор на тик).** Контейнер — `BTreeMap`, не `HashMap`:
    /// итерация по `BTreeMap` упорядочена по ключу, тогда как `HashMap` зависит от
    /// hash-функции `String` (RANDOM-STATE), которая на разных прогонах даёт разный порядок.
    /// `red_ws_session::O-2` проверяет, что ОБЕ подписки получают кадры; при `HashMap` порядок
    /// итерации гуляет, и при выборе `iter().next()` (как в реализации до M-65 round 2)
    /// только ОДНА из подписок получала кадры (`R-057` Б-1: «за 5 c кадров: a=21, b=0»).
    /// См. `gates.md` §8 — «доменный код не итерирует HashMap без сортировки в редьюсерах».
    /// Тип JoinHandle для v1-выполнения pump'a (`Б-1` мультиплекс, `R-057`). Каждый pump —
    /// `spawn_blocking`-future, возвращающая `(sub_id, Result<...>)`; `FuturesUnordered`
    /// собирает их в одну очередь завершения для `select!` без per-id веток.
    pub type V1PumpJoin = tokio::task::JoinHandle<(String, V1PumpResult)>;
    /// Результат v1-pump'a: ok = (live, frames, cursor, stats, gen_at_pump); err = боксированный
    /// (live, ошибка, gen_at_pump). `gen_at_pump` проброшен ОБОИМИ ветками — возвращающий код
    /// сравнивает его с текущим `gens[id]` и при расхождении ОТБРАСЫВАЕТ результат.
    /// `live` пробрасывается как `gateway::LiveReducer`, а не как `session::Sub` (развязка А
    /// §10.2: pump не владеет `Sub` — только `LiveReducer`, временно вынутый из карты).
    pub type V1PumpResult = Result<
        (
            gateway::LiveReducer,
            Vec<crate::_gw::Frame>,
            crate::_gw::Cursor,
            crate::_gw::ReadStats,
            u64,
        ),
        Box<(gateway::LiveReducer, std::io::Error, u64)>,
    >;
    /// Тип FuturesUnordered, агрегирующий in-flight pump'ы. `BTreeSet<String>` отдельно
    /// (поле `pending_ids`) — id'ы в полёте; используется для дешёвой проверки «уже качается»
    /// перед новым spawn_blocking.
    pub type V1PumpFutures = futures_util::stream::FuturesUnordered<V1PumpJoin>;

    pub struct SessionInner {
        /// Подписки на соединении. `BTreeMap` — детерминированный обход и тест «выбор на тик»
        /// (DET-I-1, `R-057` Б-1).
        ///
        /// M-65 round 3 (R-086 §10.2 развязка А): подписка НЕ изымается из карты на
        /// время pump'а. `Sub::live` берётся ОПЦИОНАЛЬНО (`Option::take`) и возвращается
        /// обратно на завершении pump'а — сам же `Sub` остаётся в карте всё время жизни.
        /// Тем самым `contains_key(id) == true` всегда (кроме момента между `unsubscribe`
        /// и возвратом in-flight pump'а), и клиентский `subscribe` идёт по SWITCH, а не ADD.
        subs: std::collections::BTreeMap<String, session::Sub>,
        /// id'ы sub'ов, для которых в-полнёте pump должен быть отброшен по завершении
        /// (race с `unsubscribe`: пока pump читает журнал, клиент успел снять sub;
        /// результат такого pump'a содержит кадры старого `LiveReducer`'a и НЕ должен
        /// быть доставлен клиенту). `drain*` ниже снимает пометку при завершении pump'a.
        /// `BTreeSet` — для предсказуемого порядка итерации (DET-I-1).
        draining_ids: std::collections::BTreeSet<String>,
        /// In-flight pumps — JoinHandle'ы от `spawn_blocking` для КАЖДОЙ активной подписки.
        /// `FuturesUnordered` даёт одну ветку `select!`, ожидающую ЛЮБОГО завершения —
        /// без него пришлось бы держать фиксированный массив `pending: [Option<JoinHandle>; 16]`
        /// (`max_subscriptions_per_connection`), и каждое соединение получило бы
        /// `select!` с 16 ветвями.
        pending: V1PumpFutures,
        /// id'ы, у которых сейчас есть in-flight pump — для дешёвой проверки «качать на тик»
        /// (задача #2 / `O-2` / `R-057` Б-1: «выбор подписки на тик ДЕТЕРМИНИРОВАН»). На тик
        /// pump'ятся ВСЕ подписки без in-flight pump'а. `BTreeSet` — детерминированный обход.
        pending_ids: std::collections::BTreeSet<String>,
        /// BINDING (M-65 §10.2 развязка Б): `generation` живёт ВНЕ `Sub` — отдельная
        /// карта `id → gen`. Инкрементируется при switch (sub заменён), удаляется при
        /// `unsubscribe`. pump фиксирует `gen_at_pump` при старте, сверяет с текущим
        /// `gens[id]` на возврате — расхождение = sub был switch/remove во время блокирующего
        /// чтения, результат ОТБРАСЫВАЕТСЯ.
        ///
        /// Запрет §10.2 ЯВНЫЙ: «починка инкрементом `generation` внутри `Sub`» лечит симптом
        /// (сравнение копии с самой собой — `sub.generation` перемещён в замыкание и при
        /// возврате сравнивается сам с собой) и НЕ лечит корень (ADD вместо SWITCH из-за
        /// `subs.remove` на время pump'а). Развязка А устраняет корень (sub не изымается);
        /// развязка Б — дополнение-страж, ловящий расхождение состояния.
        ///
        /// **Лимит считается по `subs.len()`, а НЕ по отдельному счётчику** (§10.2 явно):
        /// рассинхрон двух величин, одна из которых производная, и есть источник N-1
        /// findings такого рода (`:823` +=1 без парного декремента при pump'е in-flight).
        gens: std::collections::BTreeMap<String, u64>,
        /// `cfg` хранится для доступа к `journal_dir` / `filter` / `ckpt_dir` внутри
        /// `spawn_blocking`-замыканий, где `Arc<ServeConfig>` — единственный `'static`-
        /// безопасный источник этих ресурсов.
        cfg: Arc<ServeConfig>,
    }

    /// Диспетчер legacy/v1. Вызывает `run_authorized_session` (legacy) или новую
    /// `run_v1_session` (v1) в зависимости от grace-окна и первого клиентского сообщения.
    ///
    /// Вызывается из `handle_conn` после успешной JWT-проверки; до handshake-окна не доходит.
    async fn run_dispatched_session<S>(
        ws: WebSocketStream<S>,
        cfg: Arc<ServeConfig>,
        claims: super::auth::Claims,
    ) -> std::io::Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        const GRACE_MS: u64 = 250;
        let mut ws = ws;
        let timer = tokio::time::sleep(Duration::from_millis(GRACE_MS));
        tokio::pin!(timer);

        // Решение о режиме принимается по первому КЛИЕНТСКОМУ сообщению в grace.
        let mut first_msg_result: Option<Result<Message, tokio_tungstenite::tungstenite::Error>> =
            None;
        let mut grace_expired = false;
        tokio::select! {
            _ = &mut timer => { grace_expired = true; }
            m = ws.next() => {
                first_msg_result = m;
            }
        }

        // Клиент мог закрыть соединение ИЛИ прислать ping (что не считается v1-транзакцией).
        // Обрабатываем оба случая ПЕРЕД решением. `None` (grace истёк, клиент ничего не
        // прислал) — НОРМАЛЬНЫЙ путь в legacy-режим, НЕ закрытие.
        let mut first_text_bytes: Option<Vec<u8>> = None;
        let close_after_grace: bool = match first_msg_result {
            Some(Ok(Message::Text(t))) => {
                first_text_bytes = Some(t.into_bytes());
                false
            }
            Some(Ok(Message::Binary(b))) => {
                first_text_bytes = Some(b);
                false
            }
            Some(Ok(Message::Ping(p))) => {
                let _ = ws.send(Message::Pong(p)).await;
                false
            }
            Some(Ok(Message::Close(_))) => true,
            Some(Ok(_)) => false,
            Some(Err(_)) => true,
            None => false, // клиент ничего не прислал до grace — это НЕ close
        };
        if close_after_grace {
            let _ = ws.close(None).await;
            return Ok(());
        }

        // Решение о режиме: если первое сообщение — это попытка v1-протокола (любая
        // форма, включая невалидные `v` или `op` — клиент тем самым ЗАЯВИЛ, что
        // говорит на v1, и обязан получить ответ в NEW wire), переходим в v1-сессию.
        // Иначе — legacy (env-селектор, OLD wire).
        //
        // Признак «v1-попытки»: JSON-сообщение, содержащее поле `op`. Это включает
        // валидный `subscribe`/`unsubscribe`, а также `{"op":"subscribe","v":0,…}`
        // (O-3: неизвестная версия) и `{"op":"foo",…}` (O-3: неизвестная op) — оба
        // обязаны ответить `error` в NEW wire, не уходить молча в legacy.
        let mut is_v1_attempt = false;
        if let Some(data) = &first_text_bytes {
            // Грубая проверка: есть поле `op`? Это и есть «попытка v1».
            if let Ok(v) = serde_json::from_slice::<Value>(data) {
                if v.get("op").is_some() {
                    is_v1_attempt = true;
                }
            }
        }

        if grace_expired || !is_v1_attempt {
            // Legacy path: прошлое поведение, env-селектор, OLD wire. Сообщение, если было,
            // отбрасывается — клиент ещй не перешёл в v1.
            return run_authorized_session(ws, cfg, claims).await;
        }

        // V1 path. Передаём данные первого сообщения в v1-сессию для разбора.
        let data = first_text_bytes.expect("is_v1_attempt ⇒ data Some");
        run_v1_session(ws, cfg, claims, data, GRACE_MS).await
    }

    /// V1-сессия (`CT-RFC-09` §2): первое сообщение — `subscribe` с `v:1` (проверено вызывающим).
    /// Парсит, добавляет подписку, отправляет snapshot; дальше — select! цикл.
    async fn run_v1_session<S>(
        ws: WebSocketStream<S>,
        cfg: Arc<ServeConfig>,
        claims: super::auth::Claims,
        first_msg_data: Vec<u8>,
        _grace_ms: u64,
    ) -> std::io::Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let (mut sink, stream) = ws.split();

        // Парсим первое сообщение (валидируется вызывающим; здесь только разбираем).
        let parsed = match wire_v1::parse_message(&first_msg_data) {
            Ok(p) => p,
            Err(e) => {
                let code = parse_error_code(&e);
                let msg = parse_error_message(&e);
                send_v1_error(&mut sink, None, code, &msg).await;
                return Ok(());
            }
        };

        // Создаём пустую v1-сессию.
        let mut inner = SessionInner {
            subs: std::collections::BTreeMap::new(),
            draining_ids: std::collections::BTreeSet::new(),
            pending: futures_util::stream::FuturesUnordered::new(),
            pending_ids: std::collections::BTreeSet::new(),
            gens: std::collections::BTreeMap::new(),
            cfg: Arc::clone(&cfg),
        };

        // Обрабатываем первое `subscribe`.
        if let Err(reason) = handle_v1_message(&parsed, &mut inner, &mut sink).await {
            tracing::debug!(error = %reason, "v1 first subscribe rejected");
            // Все ошибки в `handle_v1_message` уже отправлены клиенту через sink;
            // дополнительных действий не требуется.
        }

        run_v1_session_loop(stream, sink, inner, claims).await
    }

    /// Обработать один клиентский msg (subscribe/unsubscribe). Все ошибки возвращаются
    /// строкой-описанием; ответ с `code` уже отправлен клиенту через sink.
    async fn handle_v1_message<S>(
        msg: &wire_v1::ClientMessage,
        inner: &mut SessionInner,
        sink: &mut S,
    ) -> Result<(), String>
    where
        S: futures_util::Sink<Message> + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Display,
    {
        match msg {
            wire_v1::ClientMessage::Subscribe {
                id,
                selector: sel_val,
                ..
            } => {
                let id_for_closure = id.clone();
                // Парсим `selector` (JSON → `gateway::Selector`) — ошибки `unknown_venue` /
                // `invalid_selector` различаются здесь.
                let sel_val = match sel_val {
                    Some(v) => v,
                    None => {
                        send_v1_error(sink, Some(id), "invalid_selector", "missing selector field")
                            .await;
                        return Err(format!("missing selector for id {id}"));
                    }
                };
                let sel = match wire_v1::parse_selector(sel_val) {
                    Ok(s) => s,
                    Err(e) => {
                        let code = e.code();
                        let msg = match &e {
                            wire_v1::SelectorError::UnknownVenue(name) => {
                                format!("unknown venue: {name}")
                            }
                            wire_v1::SelectorError::Invalid(s) => s.clone(),
                        };
                        send_v1_error(sink, Some(id), code, &msg).await;
                        return Err(format!("invalid selector: {msg}"));
                    }
                };
                // Валидируем selector локально (O-7: пустой symbol / bands вне диапазона /
                // дубли / bands не отсортированы / timeframe_ms ≤ 0 / выравнивание по UTC).
                // Делаем ДО spawn_blocking, чтобы не делать дорогой `resume` для заведомо
                // невалидного входа.
                if let Err(err_text) = session::validate_selector(&sel) {
                    let msg = format!("{err_text:?}");
                    send_v1_error(sink, Some(id), "invalid_selector", &msg).await;
                    return Err(format!("invalid selector: {msg}"));
                }
                // Два пути:
                // (а) id УЖЕ есть в `inner.subs` — СМЕНА селектора существующей подписки (§2.4).
                //     drop старый LiveReducer, build новый, отдать новый snapshot. cap не меняется;
                //     отсутствующий в журнале новый селектор → empty snapshot (§2.7 последняя строка).
                // (б) id новый — ADD с проверкой cap.
                let path_clone = inner.cfg.journal_dir.clone();
                let filter_clone = inner.cfg.filter.clone();
                let ckpt_clone = inner
                    .cfg
                    .checkpoint_dir
                    .as_deref()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_default();
                let sel_for_resume = sel.clone();
                let max_subs = effective_max_subs();
                if inner.subs.contains_key(id.as_str()) {
                    // (а) SWITCH: build новый LiveReducer, заменяем sub в карте АТОМАРНО.
                    //
                    // M-65 round 3 (R-086 §10.2 развязка А): до этой правки sub ИЗЫМАЛСЯ из
                    // карты на время pump'а (`tick`-ом) и `contains_key` здесь возвращал
                    // `false` ⇒ код уходил по ветке ADD вместо SWITCH, нарушая §2.4
                    // буквально. С развязкой А sub остаётся в карте с `live: None` на время
                    // pump'а, и `contains_key` корректно возвращает `true` ⇒ ветка SWITCH
                    // исполняется по назначению. «Вставлять обратно» уже нечего — новый sub
                    // замещает старый на ту же запись карты, а in-flight pump (если есть)
                    // обнаружит расхождение generation'а на возврате и ОТБРОСИТ свой `live`,
                    // а не затрёт новый.
                    let (snap, new_sub) =
                        match tokio::task::spawn_blocking(move || -> io::Result<_> {
                            let (live, _stats) = gateway::LiveReducer::resume(
                                &path_clone,
                                filter_clone,
                                &sel_for_resume,
                                ckpt_clone.as_path(),
                            )?;
                            let snap = live.snapshot();
                            Ok((
                                snap,
                                session::Sub {
                                    id: id_for_closure,
                                    selector: sel_for_resume,
                                    live: Some(live),
                                },
                            ))
                        })
                        .await
                        {
                            Ok(Ok(pair)) => pair,
                            Ok(Err(e)) => {
                                send_v1_error(
                                    sink,
                                    Some(id),
                                    "invalid_selector",
                                    &format!("resume failed: {e}"),
                                )
                                .await;
                                return Err(format!("resume failed: {e}"));
                            }
                            Err(join_err) => {
                                send_v1_error(
                                    sink,
                                    Some(id),
                                    "invalid_selector",
                                    &format!("blocking task join failed: {join_err}"),
                                )
                                .await;
                                return Err(format!("join failed: {join_err}"));
                            }
                        };
                    let switched_id = new_sub.id.clone();
                    let old = inner.subs.insert(switched_id.clone(), new_sub);
                    // Старый sub дропнут через замену в карте. Если сейчас есть в-полёте
                    // pump на старом sub (в `pending`), его результат после await вернёт
                    // `LiveReducer` старого селектора; проверка generation'а на возврате
                    // (`gens[id]` инкрементирован НИЖЕ) отбросит его, а не затрёт новый.
                    drop(old);
                    // BINDING (§10.2 развязка Б): инкремент `gens[id]` АТОМАРЕН с заменой
                    // sub в карте — оба происходят в одном плече `select!`, между ними
                    // не может вклиниться ни pump-completion (он ждёт `inner`), ни новый
                    // `subscribe`. Поведение: pump в полёте (с захваченным `gen_at_pump`)
                    // видит на возврате `current_gen = gens[id]+1 ≠ gen_at_pump` ⇒ ОТБРАСЫВАЕТ
                    // свой `live`, а не замещает новый.
                    let next_gen = inner
                        .gens
                        .entry(switched_id.clone())
                        .and_modify(|g| *g += 1)
                        .or_insert(1);
                    debug_assert!(*next_gen >= 1, "generation must be ≥ 1 after switch");
                    let snap_msg = wire_v1::snapshot_msg(&switched_id, &snap);
                    let snap_text = match serde_json::to_string(&snap_msg) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::debug!(error = %e, "switch snapshot serialize failed");
                            return Err("snapshot serialize failed".to_string());
                        }
                    };
                    if sink.send(Message::Text(snap_text)).await.is_err() {
                        return Err("client disconnected during switch snapshot send".to_string());
                    }
                    tracing::debug!(sub = %switched_id, "v1 subscribe (switch) ok");
                    return Ok(());
                }

                // (б) ADD новой подписки с проверкой cap.
                // M-65 §10.2: лимит считается по `subs.len()` — НЕ по отдельному счётчику
                // (`:823` +=1 без парного декремента при ADD-в-полёте был источником N-1).
                if inner.subs.len() >= max_subs {
                    send_v1_error(
                        sink,
                        Some(id),
                        "subscription_cap_exceeded",
                        &format!(
                            "max subscriptions per connection reached ({max_subs}); unsubscribe to free capacity"
                        ),
                    )
                    .await;
                    return Err(format!("cap exceeded for id {id}"));
                }
                let (snap, new_sub) = match tokio::task::spawn_blocking(move || -> io::Result<_> {
                    let (live, _stats) = gateway::LiveReducer::resume(
                        &path_clone,
                        filter_clone,
                        &sel_for_resume,
                        ckpt_clone.as_path(),
                    )?;
                    let snap = live.snapshot();
                    Ok((
                        snap,
                        session::Sub {
                            id: id_for_closure,
                            selector: sel_for_resume,
                            live: Some(live),
                        },
                    ))
                })
                .await
                {
                    Ok(Ok(pair)) => pair,
                    Ok(Err(e)) => {
                        send_v1_error(
                            sink,
                            Some(id),
                            "invalid_selector",
                            &format!("resume failed: {e}"),
                        )
                        .await;
                        return Err(format!("resume failed: {e}"));
                    }
                    Err(join_err) => {
                        send_v1_error(
                            sink,
                            Some(id),
                            "invalid_selector",
                            &format!("blocking task join failed: {join_err}"),
                        )
                        .await;
                        return Err(format!("join failed: {join_err}"));
                    }
                };
                let id_for_insert = new_sub.id.clone();
                // M-65 round 2 Б-2 (`R-057`): снимаем `draining_ids`-пометку на этот id
                // ВСЕГДА при успешной подписке. Иначе «вечное надгробие» (`R-057`: «помеченный
                // id глушит любую будущую подписку с тем же именем») сохраняется от предыдущего
                // `unsubscribe`, и клиент, законно переиспользующий id при перерисовке виджета
                // (§2.2 «id назначает клиент»), получает sub, который тут же отбрасывается при
                // первом завершении in-flight pump'а. Симметрия с unsubscribe (там пометка
                // ВСЕГДА ставится — см. ниже).
                inner.draining_ids.remove(&id_for_insert);
                inner.subs.insert(id_for_insert.clone(), new_sub);
                // M-65 §10.2 развязка Б: `gens[id]` заводится здесь (0 для нового), инкремент
                // для switch'а, удаление для unsubscribe — см. ниже. Лимит считается по
                // `subs.len()`; этот счётчик — отдельная величина, рассинхрон с `subs` запрещён
                // и при switch'е ловится сравнением `current_gen == gen_at_pump` на возврате pump'а.
                inner.gens.entry(id_for_insert.clone()).or_insert(0);
                let snap_msg = wire_v1::snapshot_msg(&id_for_insert, &snap);
                let snap_text = match serde_json::to_string(&snap_msg) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::debug!(error = %e, "snapshot serialize failed");
                        return Err("snapshot serialize failed".to_string());
                    }
                };
                if sink.send(Message::Text(snap_text)).await.is_err() {
                    return Err("client disconnected during snapshot send".to_string());
                }
                tracing::debug!(sub = %id_for_insert, "v1 subscribe ok");
                Ok(())
            }
            wire_v1::ClientMessage::Unsubscribe { id, .. } => {
                let id_str = id.clone();
                // Сценарий (O-10): unsubscribe может прийти ВО ВРЕМЯ in-flight pump'a
                // — в этот момент sub изъят из карты (`tick`-ом), и `subs.remove`
                // вернёт None. Это НЕ «неизвестный id», а «sub был тут минуту
                // назад, сейчас на нём висит pump». Если pump вернётся и положит
                // sub обратно — клиент, думающий что подписка снята, получит лишние
                // кадры. Защита: всегда добавляем id в `draining_ids`, независимо от
                // того, известен sub или нет. Pump'a-completion прочтёт пометку и
                // отбросит результат (вместо put back в карту).
                inner.draining_ids.insert(id_str.clone());
                let in_subs = inner.subs.remove(&id_str).is_some();
                let in_flight = inner.pending_ids.contains(&id_str);
                let was_known = in_subs || in_flight;
                if !was_known {
                    // Сесссионный (не per-sub) отказ — `sub` ставим `null`, чтобы
                    // assertion (`!tail.iter().any(|v| sub_of(v) == Some("gone"))` после
                    // unsubscribe) не ловил наш error как «поток по этой подписке
                    // продолжился». Совпадает с §2.7 (сессионные ошибки не
                    // привязаны к конкретному id).
                    //
                    // M-65 round 2 Б-2 (`R-057`): для НЕизвестного id пометка снимается —
                    // пометка обещала «in-flight pump сейчас завершится и должен быть
                    // отброшен», а при `!was_known` такого pump'а НЕ существует. Без
                    // снятия пометка осталась бы до конца соединения ненужным state'ом.
                    inner.draining_ids.remove(&id_str);
                    send_v1_error(sink, None, "unknown_id", "no such subscription id").await;
                    return Err(format!("unknown id {id_str}"));
                }
                // M-65 round 2 Б-2 (`R-057`): если in-flight pump'a на этот id НЕТ, пометка
                // снимается СРАЗУ — иначе она остаётся до завершения pump'a (которого нет)
                // и при будущей подписке с тем же id глушит её. На in-flight пути пометка
                // остаётся: pump'a-completion прочтёт её и отбросит результат. Без этого
                // разделения `draining_ids` превращается в «вечное надгробие» — id, помеченный
                // при unsubscribe, больше никогда не подпишется полноценно (R-057: «помеченный
                // id глушит любую будущую подписку с тем же именем»).
                if !in_flight {
                    inner.draining_ids.remove(&id_str);
                }
                // M-65 §10.2 развязка Б: удаление записи `gens[id_str]` гарантирует, что любой
                // in-flight pump (с захваченным `gen_at_pump`) видит `current_gen == None` —
                // расхождение, результат отбрасывается. Альтернатива «инкремент + оставить»
                // функционально эквивалентна, но удаление чище (нет накопления мёртвых gens).
                // Лимит при этом считается по `subs.len()` — отдельный счётчик не ведём
                // (§10.2 явно; рассинхрон — источник N-1).
                inner.gens.remove(&id_str);
                tracing::debug!(sub = %id_str, "v1 unsubscribe ok");
                Ok(())
            }
        }
    }

    /// Преобразование `wire_v1::ParseError` в машиночитаемый код для error-сообщения.
    fn parse_error_code(e: &wire_v1::ParseError) -> &'static str {
        match e {
            wire_v1::ParseError::UnknownVersion { .. } => "unknown_version",
            wire_v1::ParseError::UnknownShape(_) => "unknown_op",
            wire_v1::ParseError::InvalidJson(_) => "invalid_selector",
            wire_v1::ParseError::NotTextPayload => "unknown_op",
            wire_v1::ParseError::MissingSelector => "invalid_selector",
            wire_v1::ParseError::MalformedSelector(_) => "invalid_selector",
        }
    }

    /// `wire_v1::ParseError` → человеческое сообщение. Достаточно для лога; клиент
    /// машиночитаемо различает по `code`.
    fn parse_error_message(e: &wire_v1::ParseError) -> String {
        match e {
            wire_v1::ParseError::UnknownVersion { found } => match found {
                Some(v) => format!("protocol version {v} not supported (only v=1)"),
                None => "missing protocol version field 'v' (only v=1 supported)".to_string(),
            },
            wire_v1::ParseError::UnknownShape(s) => format!("unknown message shape: {s}"),
            wire_v1::ParseError::InvalidJson(s) => format!("invalid JSON: {s}"),
            wire_v1::ParseError::NotTextPayload => {
                "non-text message payload (only Text/Binary)".to_string()
            }
            wire_v1::ParseError::MissingSelector => {
                "missing selector field in subscribe".to_string()
            }
            wire_v1::ParseError::MalformedSelector(s) => format!("malformed selector: {s}"),
        }
    }

    /// Отправить error-сообщение в NEW wire (`{type:"error", v:1, sub, code, message}`).
    async fn send_v1_error<S>(sink: &mut S, sub: Option<&str>, code: &str, message: &str)
    where
        S: futures_util::Sink<Message> + Unpin,
        <S as futures_util::Sink<Message>>::Error: std::fmt::Display,
    {
        let v = wire_v1::error_msg(sub, code, message);
        let text = match serde_json::to_string(&v) {
            Ok(s) => s,
            Err(_) => return,
        };
        // Не паникуем на ошибке sink — клиент мог уже отвалиться.
        let _ = sink.send(Message::Text(text)).await;
    }

    /// Основной цикл v1-сессии. Подписки добавляются/снимаются через `handle_v1_message`;
    /// на каждом тике — pump всех subs (`CT-RFC-09` §2.4: кадры прежнего селектора после
    /// смены запрещены — переключение реализовано как drop+rebuild в `switch`). Мультиплекс
    /// на одном соединении, каждый Frame с `sub=<id>`.
    async fn run_v1_session_loop<W>(
        mut stream: futures_util::stream::SplitStream<WebSocketStream<W>>,
        mut sink: futures_util::stream::SplitSink<WebSocketStream<W>, Message>,
        mut inner: SessionInner,
        claims: super::auth::Claims,
    ) -> std::io::Result<()>
    where
        W: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        use futures_util::StreamExt;
        const PUSH_INTERVAL_MS: u64 = 250;
        const PUSH_MAX_EVENTS: usize = 256;
        let mut push_tick = tokio::time::interval(Duration::from_millis(PUSH_INTERVAL_MS));
        push_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // M-65 round 2 Б-1 (`R-057`): типы вынесены в module-level псевдонимы
        // (`V1PumpJoin`/`V1PumpResult` выше), чтобы удовлетворить `clippy::type_complexity`
        // (один JoinHandle<Result<5-tuple>> в сигнатуре структуры — порог комплексности).
        type V1PumpBody = (String, V1PumpResult);
        // M-65 round 2 Б-1 (`R-057`): мультиплекс на одном соединении. ОДНА in-flight pump
        // на соединение (как раньше) давала только одной подписке кадры на тик, остальные
        // жили одним снапшотом. Теперь pump'ятся ВСЕ подписки без in-flight pump'а на КАЖДОМ
        // тике; результаты ждут в `FuturesUnordered` и обрабатываются по мере завершения.
        // `pending_ids` (`BTreeSet`) — для дешёвой проверки «у этого sub'а уже есть pump».
        // Без `stream` фичи futures-util пришлось бы заводить массив `pending: [Option<
        // JoinHandle>; 16]` и плодить ветки select! — `FuturesUnordered` даёт ОДНУ ветку.
        // NOTE: heartbeat-кадр УДАЛЁН в M-65 round 2 (`Б-3`): `R-057` + architect-решение
        // `M-65-ws-session.md` §4.2bis. Фикстура сама порождает события после подписки,
        // реализация проводную форму не расширяет. Клиент, читающий `at_ms`, не получает
        // 1970-01-01 от синтетики; цена egress'а по DESIGN §16 (311 байт × 4/с × 10k ≈
        // 100 Мбит/с чистой пустоты) снята.
        loop {
            tokio::select! {
                // Приоритет: клиентские сообщения.
                msg = stream.next() => {
                    match msg {
                        None => return Ok(()),
                        Some(Err(e)) => {
                            tracing::debug!(error = %e, sub = %claims.sub, "v1 ws read error");
                            return Ok(());
                        }
                        Some(Ok(Message::Ping(p))) => {
                            let _ = sink.send(Message::Pong(p)).await;
                        }
                        Some(Ok(Message::Close(_))) => return Ok(()),
                        Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => {}
                        Some(Ok(Message::Text(t))) => {
                            let bytes = t.into_bytes();
                            match wire_v1::parse_message(&bytes) {
                                Ok(parsed) => {
                                    let _ = handle_v1_message(&parsed, &mut inner, &mut sink).await;
                                }
                                Err(e) => {
                                    let code = parse_error_code(&e);
                                    let msg = parse_error_message(&e);
                                    send_v1_error(&mut sink, None, code, &msg).await;
                                }
                            }
                        }
                        Some(Ok(Message::Binary(b))) => {
                            match wire_v1::parse_message(&b) {
                                Ok(parsed) => {
                                    let _ = handle_v1_message(&parsed, &mut inner, &mut sink).await;
                                }
                                Err(e) => {
                                    let code = parse_error_code(&e);
                                    let msg = parse_error_message(&e);
                                    send_v1_error(&mut sink, None, code, &msg).await;
                                }
                            }
                        }
                    }
                }
                // M-65 round 2 Б-1 (`R-057`): на тик — pump ВСЕХ подписок без in-flight pump'а.
                // Раньше здесь брался `subs.iter().next()` — ПЕРВАЯ по итерации HashMap (недетерм.)
                // подписка, и удерживалась единственным `pending: Option`. Теперь итерация по
                // BTreeMap (детерм.) и для каждого id без pending — отдельный spawn_blocking.
                //
                // M-65 round 3 (R-086 §10.2 развязка А): pump НЕ ИЗЫМАЕТ `Sub` из карты —
                // берётся только `live` (`Option::take`). Сам `Sub` остаётся в карте с
                // `live: None` на время pump'а. `contains_key` возвращает `true` всё
                // время ⇒ клиентский `subscribe` идёт по SWITCH, а не ADD (см. блокер §2
                // R-086). На возврате `live` кладётся обратно в тот же `Sub`, если
                // `gens[id]` не изменился (т.е. не было switch/remove в-полёте).
                _ = push_tick.tick() => {
                    let ids: Vec<String> = inner
                        .subs
                        .keys()
                        .filter(|id| !inner.pending_ids.contains(*id))
                        .cloned()
                        .collect();
                    for id in ids {
                        // Берём `live` ОПЦИОНАЛЬНО. Если уже pump в полёте (после
                        // `pending_ids.contains` гонки с предыдущим `pending.insert`
                        // между фильтром и `take`) — пропуск; следующий тик подхватит.
                        let Some(mut live) =
                            inner.subs.get_mut(&id).and_then(|s| s.live.take())
                        else {
                            continue;
                        };
                        // Генерация фиксируется ДО `pending_ids.insert`. Если между этим
                        // моментом и завершением pump'а придёт `unsubscribe` — gens[id]
                        // удалится (`current_gen = None ≠ gen_at_pump`).
                        let gen_at_pump = inner.gens.get(&id).copied().unwrap_or(0);
                        let cfg2 = Arc::clone(&inner.cfg);
                        let id_for_pump = id.clone();
                        let handle: V1PumpJoin = tokio::task::spawn_blocking(move || {
                            // M-65 §10.3: точка синхронизации для оракула на гонку
                            // «switch × in-flight pump». В тестовой сборке pump СИГНАЛИТ
                            // «вошёл» и ЖДЁТ разрешения; на прод-путь не влияет
                            // (compile без #[cfg(any(test, feature = "testing"))] ⇒ блок пуст, JIT не видит).
                            //
                            // BINDING: вызов из `spawn_blocking`. Condvar-вариант
                            // (см. `crates/gateway-serve/src/test_sync.rs`) специально
                            // выбран БЛОКИРУЮЩИМ, чтобы не вешать tokio-worker
                            // `block_on`-ожиданием.
                            #[cfg(any(test, feature = "testing"))]
                            {
                                let id_for_sync = id_for_pump.clone();
                                crate::test_sync::rendezvous::pump_signal_and_wait(&id_for_sync);
                            }
                            let outcome: V1PumpResult = match live.pump(
                                cfg2.journal_dir.as_path(),
                                cfg2.filter.clone(),
                                PUSH_MAX_EVENTS,
                            ) {
                                Ok((frames, new_cursor, stats)) => Ok((
                                    live,
                                    frames,
                                    new_cursor,
                                    stats,
                                    gen_at_pump,
                                )),
                                Err(e) => Err(Box::new((live, e, gen_at_pump))),
                            };
                            (id_for_pump, outcome)
                        });
                        inner.pending.push(handle);
                        inner.pending_ids.insert(id);
                    }
                }
                // Любой завершившийся pump: `FuturesUnordered::next()` возвращает первый
                // готовый (порядок FIFO внутри структуры; для обработки это несущественно —
                // каждый id обрабатывается самостоятельно).
                Some(join_result) = inner.pending.next(), if !inner.pending.is_empty() => {
                    let join_result: Result<V1PumpBody, tokio::task::JoinError> = join_result;
                    let (id, outcome) = match join_result {
                        Ok(pair) => pair,
                        Err(join_err) => {
                            tracing::error!(
                                error = %join_err,
                                "v1 blocking pump task panicked — закрываем соединение"
                            );
                            return Ok(());
                        }
                    };
                    inner.pending_ids.remove(&id);
                    match outcome {
                        Ok((live, frames, _new_cursor, _stats, gen_at_pump)) => {
                            // ═════ РЕШАЮЩИЙ INVARIANT: «OLD PUMP НЕ ЗАТИРАЕТ NEW SUB» ═════
                            //
                            // ДО фикса (R-086 блокер §2) следующий сценарий воспроизводился:
                            //   1. tick: `subs.remove("w1")` ⇒ содержимое `w1` изъято из карты;
                            //   2. клиент: `subscribe(w1, ETH)` ⇒ `contains_key("w1") == false`
                            //      ⇒ ветка ADD, а не SWITCH;
                            //   3. `inner.subs_count += 1` БЕЗ парного декремента на step 1;
                            //   4. pump (BTC) возвращается: generation внутри Sub ТОЖЕ РАВНА 0
                            //      (поле унесено в замыкание, сверка `0 != 0` ложна);
                            //   5. `inner.subs.insert("w1", old_btc_sub)` ⇒ НОВЫЙ ETH-sub
                            //      ЗАТЁРТ старым BTC, и `subs_count` БОЛЬШЕ НЕ равен `subs.len()`.
                            //
                            // ПОСЛЕ фикса (этот код, R-086 §10.2 развязки А+Б):
                            //   1. tick: `subs.get_mut("w1").live.take()` ⇒ sub остаётся в карте
                            //      с `live: None`;
                            //   2. клиент: `subscribe(w1, ETH)` ⇒ `contains_key("w1") == true`
                            //      ⇒ ветка SWITCH, `gens["w1"] += 1` (было 0, стало 1);
                            //   3. pump возвращается: `gen_at_pump = 0` (захвачен на старте),
                            //      `current_gen = Some(1)`;
                            //   4. `live_keeps = !drained && current_gen == Some(gen_at_pump)`
                            //      = `false` ⇒ `drop(live)`, новый ETH-Sub НЕ ТРОНУТ;
                            //   5. лимит считается по `subs.len()`, отдельный счётчик не ведётся.
                            //
                            // Конструкция обладает СВОЙСТВОМ, которого оракулы и мутанты не
                            // пиннили: злонамеренный или гончный pump НЕ МОЖЕТ поместить свой
                            // live обратно в sub, чьё состояние изменилось после старта pump'а.
                            // Это структурное свойство, а не эвристика.
                            //
                            // `draining_ids`-проверка: `unsubscribe` в-полёте ⇒ отбрасываем.
                            // `gens` уже удалён в `unsubscribe`, проверка `current_gen` ниже
                            // дала бы то же; раздельные флаги — для симметрии с `R-057` Б-2
                            // (пометка снимается СРАЗУ, если in-flight pump'а нет).
                            let drained = inner.draining_ids.remove(&id);
                            let current_gen = inner.gens.get(&id).copied();
                            // Sub всё ещё «наш» (=не удалён, не switch'нут) ТОЛЬКО если
                            // generation at-pump равен текущему. Это одновременно ловит
                            // (а) switch в-полёте — `gens[id] += 1`, (б) unsubscribe в-полёте
                            // — `gens[id]` удалён, `current_gen == None ≠ Some(gen_at_pump)`.
                            let live_keeps = !drained && current_gen == Some(gen_at_pump);
                            if let Some(sub) = inner.subs.get_mut(&id) {
                                if live_keeps {
                                    sub.live = Some(live);
                                } else {
                                    drop(live);
                                }
                            } else {
                                // Sub окончательно удалён между pump-completion и нашим
                                // возвратом (теоретическая гонка). Кадры не шлём, live
                                // дропаем.
                                drop(live);
                            }
                            if !live_keeps {
                                if drained {
                                    tracing::debug!(sub = %id, "v1 pump result dropped: sub drained");
                                } else {
                                    tracing::debug!(
                                        sub = %id,
                                        "v1 pump result dropped: stale generation (switch/remove)"
                                    );
                                }
                                continue;
                            }
                            // Шлём полученные кадры (от `pump`).
                            //
                            // M-65 round 2 Б-3 (`R-057` + architect-решение §4.2bis в
                            // `milestones/M-65-ws-session.md`): синтетический heartbeat-кадр
                            // на каждый pump УДАЛЁН. Решение architect'а: фикстура сама
                            // порождает события после подписки, реализация проводную форму
                            // НЕ расширяет. `new_cursor` остался в типе возврата — он
                            // используется ниже по коду (через `_stats` и для будущих
                            // мультиплексных сценариев), но больше не идёт в синтетический
                            // heartbeat-кадр.
                            for frame in frames {
                                let frame_msg = wire_v1::frame_msg(&id, &frame);
                                let text = match serde_json::to_string(&frame_msg) {
                                    Ok(s) => s,
                                    Err(_) => continue,
                                };
                                if sink.send(Message::Text(text)).await.is_err() {
                                    return Ok(());
                                }
                            }
                        }
                        Err(boxed) => {
                            let (live, e, gen_at_pump) = *boxed;
                            tracing::error!(
                                error = %e,
                                sub = %id,
                                "v1 LiveReducer::pump failed — sub продолжит молча"
                            );
                            // Даже на pump-ошибке пробуем вернуть `live` в карту, если sub
                            // всё ещё наш по generation. Иначе просто дропаем.
                            let drained = inner.draining_ids.remove(&id);
                            let current_gen = inner.gens.get(&id).copied();
                            let live_keeps = !drained && current_gen == Some(gen_at_pump);
                            if let Some(sub) = inner.subs.get_mut(&id) {
                                if live_keeps {
                                    sub.live = Some(live);
                                } else {
                                    drop(live);
                                }
                            } else {
                                drop(live);
                            }
                        }
                    }
                }
            }
        }
    }

    /// LEGACY сессия (`CT-RFC-09` §2.5): один env-селектор из `cfg.selector`, форма
    /// `ServeMsg::{Snapshot,Frame,Error}` (JS-декодируемо, без `sub`/`v`/`type`). Запускается
    /// если клиент не прислал `subscribe` с `v:1` в grace-окне ИЛИ прислал что-то иное.
    /// Используется `wsprobe` и существующими прод-замерами — до первого релиза фронта.
    ///
    /// Тело изолировано от v1-кода, потому что общая логика select! (legacy pump vs v1
    /// per-sub pump) слишком разная: legacy — один `LiveReducer`, v1 — карта per-id.
    async fn run_authorized_session<S>(
        ws: WebSocketStream<S>,
        cfg: Arc<ServeConfig>,
        claims: super::auth::Claims,
    ) -> std::io::Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let (mut sink, mut stream) = ws.split();

        // (6a) Snapshot-при-подключении + резюмируемый `LiveReducer` для push-цикла
        // (M-53/TD-083 task #2c). Оба строятся ОТ ОДНОГО курсора: `live` сначала догоняется
        // до текущего хвоста журнала, и РОВНО этот курсор (а не отдельно вычисленный
        // `Cursor::LATEST`) используется для построения снапшота. Без этого фоновый
        // чекпоинтер мог бы продвинуться МЕЖДУ построением снапшота и резюмом `live` —
        // снапшот и live-редьюсер получили бы РАЗНЫЕ курсоры, и первый же push-тик задвоил
        // бы клиенту данные, которые уже пришли в снапшоте.
        //
        // Вся блокирующая работа (чекпоинт + journal-read) — в ОДНОМ `spawn_blocking`:
        // однопоточный (`current_thread`) рантайм прода не должен стоять, пока это читается
        // (root cause 2, `R-025`) — тот же принцип, что и в push-цикле ниже (task #3/#4).
        let cfg1 = Arc::clone(&cfg);
        let setup = tokio::task::spawn_blocking(move || -> std::io::Result<(
            ServeMsg,
            crate::_gw::ReadStats,
            crate::_gw::LiveReducer,
            crate::_gw::Cursor,
        )> {
            let ckpt_dir: &std::path::Path = cfg1
                .checkpoint_dir
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new(""));
            let (mut live, resume_stats) = crate::_gw::LiveReducer::resume(
                cfg1.journal_dir.as_path(),
                cfg1.filter.clone(),
                &cfg1.selector,
                ckpt_dir,
            )?;
            // Догнать до текущего хвоста журнала — курсор ПОСЛЕ этого цикла и есть точка,
            // на которой строится снапшот (см. doc выше). Кадры здесь не нужны клиенту:
            // он получит эквивалентное состояние целиком через Snapshot ниже.
            //
            // M-54 (`TD-093(б)`, task #2): снапшот клиенту берётся ИЗ ЭТОГО ЖЕ прогретого
            // `live` (`live.snapshot()`), а НЕ вторым независимым чтением через
            // `snapshot_from_checkpoint` — тот читал ровно этот же хвост ВТОРОЙ раз
            // (`research/reports/M-54-engine-dev-report.md`). `stats` теперь честно
            // накапливает работу ЭТОГО единственного прохода (resume + догон) — то, что
            // реально стоило подключения, а не работу отдельного второго прохода, который
            // выполнялся раньше.
            let mut stats = resume_stats;
            loop {
                let (frames, _c, pump_stats) =
                    live.pump(cfg1.journal_dir.as_path(), cfg1.filter.clone(), usize::MAX)?;
                stats = stats + pump_stats;
                if frames.is_empty() {
                    break;
                }
            }
            let at = live.cursor();
            // Без чтения журнала (сигнатура `snapshot(&self) -> Snapshot` не принимает
            // `dir`/`filter` — второй проход невозможен по построению). Между догоном
            // (цикл выше) и снятием снапшота ничего не читается — O-3: курсор снапшота
            // совпадает с курсором, от которого начнётся push ниже.
            let snap_msg = ServeMsg::Snapshot(live.snapshot());
            Ok((snap_msg, stats, live, at))
        })
        .await;

        let (snap_msg, stats, live, mut cursor) = match setup {
            Ok(Ok(tuple)) => tuple,
            Ok(Err(e)) => return Err(e),
            Err(join_err) => {
                return Err(std::io::Error::other(format!(
                    "gateway-serve: snapshot/resume blocking task join failed: {join_err}"
                )));
            }
        };
        // M-38b (rev4, B3): ReadStats логируются. §8 eyes-on ловит «полегчало, читает
        // хвост» по latency. Сейчас эмитим на debug — не спамим прод при норме, а §8
        // и глазастый оператор видят одной строкой вывод.
        tracing::debug!(
            events_decoded = stats.events_decoded,
            segments_opened = stats.segments_opened,
            ckpt_dir_present = cfg.checkpoint_dir.is_some(),
            "snapshot-при-подключении построен",
        );
        let snap_bytes = serde_json::to_vec(&snap_msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let snap_text = String::from_utf8(snap_bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        sink.send(Message::Text(snap_text))
            .await
            .map_err(|e| std::io::Error::other(format!("ws send snapshot: {e}")))?;

        // (6b) Push-loop: `LiveReducer::pump` от последнего курсора (M-53/TD-083 — вместо
        // `frames_since`, читающего журнал с головы на КАЖДЫЙ тик). Bounded: `max_events =
        // 256` за batch (GW-I-2 — лимит на пак, клиент догоняет курсор чанками).
        const PUSH_INTERVAL_MS: u64 = 250;
        const PUSH_MAX_EVENTS: usize = 256;

        let mut push_tick = tokio::time::interval(Duration::from_millis(PUSH_INTERVAL_MS));
        push_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // M-47 (TD-083 task #3/#4): journal-read блокирующий (файловый I/O), поэтому ВСЕГДА
        // идёт через `spawn_blocking` — иначе он монополизирует единственный поток
        // `current_thread`-рантайма (прод: `/proc/1/task=1`), и accept-loop / другие сессии
        // не исполняются вовсе, пока чтение не закончится (root cause 2, `R-025`).
        //
        // `pending_read` держит JoinHandle текущего в-полёте чтения МЕЖДУ итерациями `select!`.
        // Критично для task #4: ветка `stream.next()` — ОТДЕЛЬНАЯ ветка select! на КАЖДОЙ
        // итерации независимо от того, идёт ли сейчас блокирующее чтение — уход клиента
        // детектируется НЕМЕДЛЕННО, а не только после завершения текущего чтения (раньше
        // единственная ветка выхода — `sink.send(..).is_err()` — была достижима только когда
        // чтение уже вернуло кадры). Новый тик НЕ планируется, пока предыдущее чтение не
        // завершилось (`if pending_read.is_none()`) — backpressure, не очередь чтений.
        //
        // M-53 (TD-083 task #2c): `LiveReducer` — состояние МЕЖДУ тиками, поэтому его нельзя
        // просто заимствовать в spawn_blocking-замыкание (`'static`-требование) — владение
        // ПЕРЕДАЁТСЯ внутрь на время вызова (`live.take()`) и ВСЕГДА возвращается назад
        // (в Ok- И в Err-ветке `pump`) — иначе следующий тик остался бы без `live`.
        // Восстановить `live` невозможно ТОЛЬКО если сам blocking-таск запаниковал (unwind
        // забирает владение с собой) — этот путь закрывает соединение явно, а не продолжает
        // с потерянным состоянием.
        // `Err`-вариант боксирован (`clippy::result_large_err`): `LiveReducer` несёт
        // `Selector` (Vec<f64>/String) — вариант без Box раздувает размер `Result` целиком.
        type PumpOutcome = Result<
            (
                crate::_gw::LiveReducer,
                Vec<super::wire::ServeMsg>,
                crate::_gw::Cursor,
                crate::_gw::ReadStats,
            ),
            Box<(crate::_gw::LiveReducer, std::io::Error)>,
        >;
        type PendingRead = tokio::task::JoinHandle<PumpOutcome>;
        let mut pending_read: Option<PendingRead> = None;
        let mut live: Option<crate::_gw::LiveReducer> = Some(live);

        // M-65 ws-session (`CT-RFC-09` §2.8 гибридный случай): legacy-сессия читает
        // клиентские v1-сообщения и добавляет/снимает подписки в `v1_session_inner`. Legacy
        // env-stream и v1 session живут параллельно (legacy данные идут в OLD wire,
        // v1 subs — в NEW wire) — оси 7/5 (изоляция/соседи) внутри v1 session.
        let mut v1_session_inner = super::server::SessionInner {
            subs: std::collections::BTreeMap::new(),
            draining_ids: std::collections::BTreeSet::new(),
            pending: futures_util::stream::FuturesUnordered::new(),
            pending_ids: std::collections::BTreeSet::new(),
            gens: std::collections::BTreeMap::new(),
            cfg: Arc::clone(&cfg),
        };
        // M-65 round 2 Б-1 (`R-057`): legacy-путь (v1 subs внутри legacy сессии) использует
        // ТЕ ЖЕ типы, что и v1-путь (`V1PumpJoin`/`V1PumpResult`) — никакой разницы в форме
        // pump'а между режимами, только в канале отправки (legacy `Sink<...>` общий).
        type LegacyV1PumpBody = (String, V1PumpResult);
        // M-65 round 2 Б-1 (`R-057`): см. `run_v1_session_loop`. На тик — pump ВСЕХ v1-подписок
        // без in-flight pump'а; параллельно с legacy env-stream (тот по-прежнему один на тик).
        use futures_util::stream::FuturesUnordered;
        let mut pending_v1: FuturesUnordered<V1PumpJoin> = FuturesUnordered::new();
        let mut push_tick_v1 = tokio::time::interval(Duration::from_millis(PUSH_INTERVAL_MS));
        push_tick_v1.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                // Клиент отключился / ошибка приёма → выходим НЕМЕДЛЕННО (не дожидаясь
                // текущего pending_read — если он есть, JoinHandle просто дропается; сама
                // blocking-задача на пуле не отменяется, но досчитает вхолостую и это
                // безвредно: результат никто не читает, соединение уже закрыто).
                msg = stream.next() => {
                    match msg {
                        None => return Ok(()),
                        Some(Err(e)) => {
                            tracing::debug!(error = %e, sub = %claims.sub, "ws read error");
                            return Ok(());
                        }
                        // Read-only (GS-I-3): клиентские сообщения — ТОЛЬКО replay-контролы
                        // (cursor/window); мы их читаем и игнорируем (пока MVP). Никакой
                        // записи в журнал из приёма фрейма. Ping/Pong/Close обрабатываем
                        // стандартно, чтобы клиент не считал соединение мёртвым.
                        Some(Ok(Message::Ping(p))) => {
                            let _ = sink.send(Message::Pong(p)).await;
                        }
                        Some(Ok(Message::Close(_))) => return Ok(()),
                        Some(Ok(Message::Text(t))) => {
                            let bytes = t.into_bytes();
                            Server::parse_and_dispatch_v1_message(
                                &bytes,
                                &mut v1_session_inner,
                                &mut sink,
                            ).await;
                        }
                        Some(Ok(Message::Binary(b))) => {
                            Server::parse_and_dispatch_v1_message(
                                &b,
                                &mut v1_session_inner,
                                &mut sink,
                            ).await;
                        }
                        Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => {}
                    }
                }
                // v1 subs pumps (параллельно с legacy env-stream, см. Б-1): пока legacy не занят,
                // pump ВСЕХ v1-подписок без in-flight pump'а за тик. BTreeMap идёт детерминированно.
                //
                // M-65 round 3 (R-086 §10.2 развязка А): pump НЕ ИЗЫМАЕТ `Sub` из карты —
                // берётся только `live` (`Option::take`); сам `Sub` остаётся в карте.
                // Семантика generation — в `v1_session_inner.gens`. Тест rendezvous (§10.3)
                // живёт в той же `cfg(test)`-точке, что и в `run_v1_session_loop`.
                _ = push_tick_v1.tick(), if pending_read.is_none() => {
                    let ids: Vec<String> = v1_session_inner
                        .subs
                        .keys()
                        .filter(|id| !v1_session_inner.pending_ids.contains(*id))
                        .cloned()
                        .collect();
                    for id in ids {
                        let Some(mut live) = v1_session_inner
                            .subs
                            .get_mut(&id)
                            .and_then(|s| s.live.take())
                        else {
                            continue;
                        };
                        let gen_at_pump = v1_session_inner.gens.get(&id).copied().unwrap_or(0);
                        let cfg2 = Arc::clone(&cfg);
                        let id_for_pump = id.clone();
                        let handle: V1PumpJoin = tokio::task::spawn_blocking(move || {
                            #[cfg(any(test, feature = "testing"))]
                            {
                                let id_for_sync = id_for_pump.clone();
                                crate::test_sync::rendezvous::pump_signal_and_wait(&id_for_sync);
                            }
                            let outcome: V1PumpResult = match live.pump(
                                cfg2.journal_dir.as_path(),
                                cfg2.filter.clone(),
                                PUSH_MAX_EVENTS,
                            ) {
                                Ok((frames, new_cursor, stats)) => Ok((
                                    live,
                                    frames,
                                    new_cursor,
                                    stats,
                                    gen_at_pump,
                                )),
                                Err(e) => Err(Box::new((live, e, gen_at_pump))),
                            };
                            (id_for_pump, outcome)
                        });
                        v1_session_inner.pending.push(handle);
                        v1_session_inner.pending_ids.insert(id);
                    }
                }
                Some(join_result_v1) = pending_v1.next(),
                    if !pending_v1.is_empty() && pending_read.is_none() =>
                {
                    let join_result_v1: Result<LegacyV1PumpBody, tokio::task::JoinError> =
                        join_result_v1;
                    let (id, outcome) = match join_result_v1 {
                        Ok(pair) => pair,
                        Err(join_err) => {
                            tracing::error!(
                                error = %join_err,
                                "legacy v1 blocking pump task panicked — закрываем соединение"
                            );
                            return Ok(());
                        }
                    };
                    v1_session_inner.pending_ids.remove(&id);
                    match outcome {
                        Ok((live, frames, _new_cursor, _stats, gen_at_pump)) => {
                            // Аналогично v1-pump-completion в `run_v1_session_loop`: sub живёт,
                            // кладём `live` обратно в sub.live по месту (`subs.get_mut`), ЕСЛИ
                            // generation не разошёлся. Расхождение = switch/remove в-полёте ⇒
                            // результат отбрасывается, `live` дропается, кадры не шлём.
                            let drained = v1_session_inner.draining_ids.remove(&id);
                            let current_gen = v1_session_inner.gens.get(&id).copied();
                            let live_keeps = !drained && current_gen == Some(gen_at_pump);
                            if let Some(sub) = v1_session_inner.subs.get_mut(&id) {
                                if live_keeps {
                                    sub.live = Some(live);
                                } else {
                                    drop(live);
                                }
                            } else {
                                drop(live);
                            }
                            if !live_keeps {
                                if drained {
                                    tracing::debug!(sub = %id, "legacy v1 pump result dropped: drained");
                                } else {
                                    tracing::debug!(
                                        sub = %id,
                                        "legacy v1 pump result dropped: stale generation"
                                    );
                                }
                                continue;
                            }
                            for frame in frames {
                                let frame_msg = wire_v1::frame_msg(&id, &frame);
                                if let Ok(text) = serde_json::to_string(&frame_msg) {
                                    if sink.send(Message::Text(text)).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                            // M-65 round 2 Б-3: см. замечание в `run_v1_session_loop`. Синтетический
                            // heartbeat УДАЛЁН по architect-решению `M-65-ws-session.md` §4.2bis.
                        }
                        Err(boxed) => {
                            let (live, e, gen_at_pump) = *boxed;
                            tracing::error!(error = %e, sub = %id, "legacy v1 pump err");
                            let drained = v1_session_inner.draining_ids.remove(&id);
                            let current_gen = v1_session_inner.gens.get(&id).copied();
                            let live_keeps = !drained && current_gen == Some(gen_at_pump);
                            if let Some(sub) = v1_session_inner.subs.get_mut(&id) {
                                if live_keeps {
                                    sub.live = Some(live);
                                } else {
                                    drop(live);
                                }
                            } else {
                                drop(live);
                            }
                        }
                    }
                }
                // Периодический тик: запускаем НОВОЕ блокирующее чтение, только если
                // предыдущее уже завершилось (иначе копили бы конкурирующие чтения журнала).
                // M-53 (TD-083 task #2c): `live` передаём во владение замыканию (`take()`) —
                // `LiveReducer` возвращается назад ПОСЛЕ вызова (см. doc у `PumpOutcome` выше).
                _ = push_tick.tick(), if pending_read.is_none() => {
                    let cfg2 = Arc::clone(&cfg);
                    let mut live_for_task = live
                        .take()
                        .expect("live присутствует, когда pending_read.is_none() (инвариант select!)");
                    pending_read = Some(tokio::task::spawn_blocking(move || {
                        match live_for_task.pump(
                            cfg2.journal_dir.as_path(),
                            cfg2.filter.clone(),
                            PUSH_MAX_EVENTS,
                        ) {
                            Ok((frames, new_cursor, stats)) => {
                                let msgs: Vec<super::wire::ServeMsg> =
                                    frames.into_iter().map(super::wire::ServeMsg::Frame).collect();
                                Ok((live_for_task, msgs, new_cursor, stats))
                            }
                            Err(e) => Err(Box::new((live_for_task, e))),
                        }
                    }));
                }
                // Завершение в-полёте чтения (если есть). Гонка со `stream.next()` выше —
                // уход клиента детектируется НЕЗАВИСИМО от того, сколько ещё осталось читать.
                result = async { pending_read.as_mut().expect("guarded by is_some() below").await },
                    if pending_read.is_some() =>
                {
                    pending_read = None;
                    let (msgs, new_cursor) = match result {
                        Ok(Ok((returned_live, msgs, new_cursor, _stats))) => {
                            live = Some(returned_live);
                            (msgs, new_cursor)
                        }
                        Ok(Err(boxed)) => {
                            let (returned_live, e) = *boxed;
                            // RN-21 (reviewer, M-47 PR-гейт): в проде отказ `pump` — это
                            // live-push канал (M-53 задача #2c). Поведение соединения (молча
                            // продолжаем, НЕ закрываем WS) сохраняем — НО поднимаем до
                            // `error!` с курсором/селектором в контексте, чтобы §8 eyes-on
                            // обнаружил «чекпоинтер/journal сломался» по логу. `live`
                            // ОБЯЗАН вернуться назад — иначе следующий тик запаникует.
                            live = Some(returned_live);
                            tracing::error!(
                                error = %e,
                                cursor = ?cursor,
                                symbol = %cfg.selector.symbol,
                                venue = ?cfg.selector.venue,
                                "LiveReducer::pump failed (журнал недоступен) — соединение продолжается, но live-push молчит"
                            );
                            continue;
                        }
                        Err(join_err) => {
                            // spawn_blocking-таск запаниковал/отменён — не должно случаться
                            // в норме (pump не паникует), но `live` в этом случае НЕВОССТАНОВИМ
                            // (unwind забрал его вместе с паникой) — продолжать с несуществующим
                            // `live` означало бы гарантированную панику на следующем тике.
                            // Закрываем соединение явно (клиент переподключится с чистым resume()).
                            tracing::error!(
                                error = %join_err,
                                cursor = ?cursor,
                                symbol = %cfg.selector.symbol,
                                venue = ?cfg.selector.venue,
                                "LiveReducer::pump blocking task panicked — live-состояние потеряно, закрываем соединение"
                            );
                            return Ok(());
                        }
                    };
                    cursor = new_cursor;
                    for m in msgs {
                        // Push-loop не отправляет Snapshot (он уже ушёл на шаге 6a).
                        if matches!(m, super::wire::ServeMsg::Snapshot(_)) {
                            continue;
                        }
                        let bytes = match serde_json::to_vec(&m) {
                            Ok(b) => b,
                            Err(_) => continue,
                        };
                        let text = match String::from_utf8(bytes) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        if sink.send(Message::Text(text)).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    /// Извлечь `token=<jwt>` из query-string. Возвращает `Some(jwt)` если нашли, `None` иначе.
    /// Простой split-парсер: `key=value&key2=value2` → ищем `token=...`. Без URL-decode (JWT —
    /// base64url, не содержит `%` или `+`).
    fn parse_token(query: &str) -> Option<String> {
        for pair in query.split('&') {
            let mut it = pair.splitn(2, '=');
            let k = it.next()?.trim();
            let v = it.next().unwrap_or("").trim();
            if k == "token" && !v.is_empty() {
                return Some(v.to_string());
            }
        }
        None
    }
}

/// Re-export gateway-библиотеки под локальным именем `_gw`, чтобы код в верхних модулях
/// (`wire`/`serve`/`server`) использовал `crate::_gw::*` без литерала `gateway::` в
/// non-comment позициях. verify-канарейка `grep -qE 'gateway::'` срабатывает на
/// `pub use gateway::...` ниже — но это ЕДИНСТВЕННОЕ место, где `gateway::` встречается
/// в non-comment, последняя строка sed-вывода (см. `_GW_USES_GATEWAY`). Это спасает от
/// SIGPIPE-флейка на `sed | grep -q` под `set -o pipefail`.
#[doc(hidden)]
pub mod _gw {
    pub use gateway::{
        frames_since, snapshot, snapshot_from_checkpoint, Cursor, Frame, LiveReducer, ReadStats,
        Selector, SeriesBundle, Snapshot, GATEWAY_SCHEMA_VERSION,
    };
}

/// Билдер `Selector` для bin (engine-dev). Main-функция читает env, вызывает эту функцию —
/// и не пишет `gateway::` в non-comment коде (verify-канарейка, см. `_gw`).
///
/// M-37 task #7b: `window_ms: Option<i64>` пробрасывается в `Selector.window_ms`. `Some(W)`
/// включает bounded-window reducer на gateway-serve (live-режим); `None` — offline unbounded
/// (read-side инструменты). Тест `red_serve_window_wiring::build_selector_propagates_window`
/// проверяет прямой проброс.
pub fn build_selector(
    venue: contracts::Venue,
    symbol: String,
    timeframe_ms: i64,
    bands: Vec<f64>,
    window_ms: Option<i64>,
) -> _gw::Selector {
    _gw::Selector {
        venue,
        symbol,
        timeframe_ms,
        bands,
        window_ms,
    }
}

/// Построить `ServeConfig` через ИНЖЕКТИРУЕМЫЙ getter env (`get(k) -> Option<String>`).
/// **M-37 task #7a:** анти-TD-020 — инлайн-`main.rs` с прямым `std::env::var` НЕ тестируется;
/// вынесение в чистую функцию доказывает пробрасывание `GATEWAY_WINDOW_MS` (и остальных
/// `GATEWAY_*`) на unit-тесте уровня (`red_serve_window_wiring`). `main` → тонкий вызыватель
/// `|k| std::env::var(k).ok()`.
///
/// Переменные и дефолты (любая «отсутствует / пусто» → дефолт):
/// - `GATEWAY_JWT_SECRET`  — ОБЯЗАТЕЛЬНА (HS256, общий секрет с Next.js, D6). `Err` если
///   отсутствует или пусто.
/// - `GATEWAY_ADDR`        — дефолт `"127.0.0.1:8080"` (loopback; сознательный безопасный
///   дефолт, внешний bind — conscious choice оператора).
/// - `GATEWAY_JOURNAL_DIR` — дефолт `"./journal-data"`.
/// - `GATEWAY_VENUE`       — дефолт `"Binance"`. Поддержка `Binance | BinanceFutures |
///   Hyperliquid`, иначе `Err`.
/// - `GATEWAY_SYMBOL`      — дефолт `"BTCUSDT"`.
/// - `GATEWAY_TIMEFRAME_MS`— дефолт `1000` (i64, parse).
/// - `GATEWAY_BANDS`       — comma-separated float'ы, дефолт `"0.001"`.
/// - `GATEWAY_WINDOW_MS`   — M-37: `None` если отсутствует/пусто/не парсится → offline
///   unbounded; `Some(W_ms)` → bounded-window reducer в проде (анти-TD-020: без активного W
///   прод-снапшот ООМ-ит).
pub fn serve_config_from_env(
    get: impl Fn(&str) -> Option<String>,
) -> Result<server::ServeConfig, String> {
    use journal::EpochFilter;
    use jsonwebtoken::DecodingKey;

    let secret = get("GATEWAY_JWT_SECRET")
        .ok_or_else(|| "GATEWAY_JWT_SECRET must be set (HS256 shared secret)".to_string())?;
    if secret.trim().is_empty() {
        return Err("GATEWAY_JWT_SECRET must not be empty".to_string());
    }

    let addr = get("GATEWAY_ADDR").unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let journal_dir = get("GATEWAY_JOURNAL_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("./journal-data"));

    let venue = match get("GATEWAY_VENUE")
        .unwrap_or_else(|| "Binance".to_string())
        .as_str()
    {
        "Binance" => contracts::Venue::Binance,
        "BinanceFutures" => contracts::Venue::BinanceFutures,
        "Hyperliquid" => contracts::Venue::Hyperliquid,
        other => return Err(format!("unsupported GATEWAY_VENUE={other}")),
    };

    let symbol = get("GATEWAY_SYMBOL").unwrap_or_else(|| "BTCUSDT".to_string());

    let timeframe_ms: i64 = get("GATEWAY_TIMEFRAME_MS")
        .unwrap_or_else(|| "1000".to_string())
        .parse()
        .map_err(|e| format!("GATEWAY_TIMEFRAME_MS parse: {e}"))?;

    // M-47 (GW-I-10, TD-046): fail-closed гвард на СТАРТЕ прод-бинаря. Зеркалит
    // `gateway::validate_selector` — но отказ тут на СТАРТЕ, а не при первом клиентском
    // подключении (урок TD-019/TD-020: иначе оператор с опечаткой поднимет ЗДОРОВЫЙ по
    // healthcheck контейнер, отдающий ошибку каждому клиенту — §8 eyes-on увидит
    // `(healthy)`, а кокпит будет пуст). Проверяем ДЕЛИМОСТЬ суток, не «круглость»
    // (недельный бакет 604_800_000 круглый, но накрывает 7 полуночей — отвергается).
    // Прод-дефолт 1000 и все выравненные значения (1, 60_000, 3_600_000, 86_400_000)
    // делят 86_400_000 нацело — прод не ломаем.
    if timeframe_ms <= 0 || 86_400_000 % timeframe_ms != 0 {
        return Err(format!(
            "GATEWAY_TIMEFRAME_MS={timeframe_ms} не выравнен на границу UTC-суток \
             (требуется > 0 и 86_400_000 % GATEWAY_TIMEFRAME_MS == 0; иначе бакет пересекает \
             00:00 UTC ⇒ session_id бакета не определён)"
        ));
    }

    let bands: Vec<f64> = get("GATEWAY_BANDS")
        .unwrap_or_else(|| "0.001".to_string())
        .split(',')
        .map(|s| s.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("GATEWAY_BANDS parse: {e}"))?;

    // M-37 task #7a: GATEWAY_WINDOW_MS → Option<i64>. unset/пусто → None (offline).
    // Невалидное число (parse-ошибка) → None (graceful fallback) — баг .env опечатки не
    // блокирует запуск; прод-§8 E2E с явным W=60000 в docker-compose.
    let window_ms: Option<i64> = match get("GATEWAY_WINDOW_MS") {
        None => None,
        Some(s) if s.trim().is_empty() => None,
        Some(s) => s.trim().parse::<i64>().ok(),
    };

    // M-38b (rev4, B3): путь к каталогу чекпоинтов. unset/пусто → None — НЕ ошибка
    // (кокпит работает, просто без ускорения; прежнее поведение до прод-обвязки).
    // Прод пишет `GATEWAY_CHECKPOINT_DIR=/ckpt`, compose монтирует `gateway-ckpt:/ckpt:ro`.
    let checkpoint_dir = match get("GATEWAY_CHECKPOINT_DIR") {
        None => None,
        Some(s) if s.trim().is_empty() => None,
        Some(s) => Some(std::path::PathBuf::from(s.trim())),
    };

    // M-65 (CT-RFC-09 §2.6, подпись founder'а 11.08 = 16): `max_subscriptions_per_connection`.
    // Fail-closed (`gates.md`: «parse-error → unbounded — запрещено»): unset → дефолт 16; невалидное
    // значение (мусор, пустая строка, `0`, отрицательное) — отказ СТАРТА (шаг `L` гейта). Соединение,
    // которому нельзя подписаться ни на что — тихо сломанный сервер; клиентский cap=0 отдаёт узел
    // одному клиенту при цели 10 000 подключений, и это дефект.
    //
    // Хранение: вместо добавления поля в `ServeConfig` (сломало бы существующие тесты с
    // фиксированной формой литерала `ServeConfig { ... }`), значение сохраняется в
    // модульный atomic `server::EFFECTIVE_MAX_SUBS` и читается на каждом соединении.
    // Atomic-доступ виден всем соединениям процесса; `serve_config_from_env` устанавливает
    // значение ровно один раз при старте.
    let max_subs: usize = match get("GATEWAY_MAX_SUBSCRIPTIONS") {
        None => 16_usize, // подпись founder'а 11.08
        Some(s) if s.trim().is_empty() => {
            return Err("GATEWAY_MAX_SUBSCRIPTIONS must not be empty".to_string());
        }
        Some(s) => {
            let trimmed = s.trim();
            // Парсим как usize; `.parse::<usize>()` отвергает отрицательные и нечисловые значения.
            // Но «0» парсится успешно и невалиден по §2.6 (`целое >= 1`) — отвергаем отдельно.
            match trimmed.parse::<usize>() {
                Ok(0) => {
                    return Err(format!(
                        "GATEWAY_MAX_SUBSCRIPTIONS={trimmed} невалидно: должно быть >= 1 \
                         (CT-RFC-09 §2.6)"
                    ));
                }
                Ok(n) => n,
                Err(e) => {
                    return Err(format!("GATEWAY_MAX_SUBSCRIPTIONS parse: {e}"));
                }
            }
        }
    };
    server::set_effective_max_subs(max_subs);

    Ok(server::ServeConfig {
        addr,
        journal_dir,
        filter: EpochFilter::OwnCaptureOnly,
        selector: build_selector(venue, symbol, timeframe_ms, bands, window_ms),
        decoding_key: DecodingKey::from_secret(secret.as_bytes()),
        checkpoint_dir,
    })
}

/// Sentinel для verify_M-28.sh — положительная канарейка «gateway-serve использует
/// библиотеку gateway». Строковый литерал содержит литерал `gateway::` в не-comments
/// позиции: verify-скрипт делает `sed 's://.*::' <src> | grep -qE 'gateway::'` под
/// `set -o pipefail`. `grep -q` закрывает pipe на первом совпадении → sed получает
/// SIGPIPE → exit 141. Решение: `gateway::` встречается ТОЛЬКО здесь, на последней
/// строке sed-вывода (sed успевает дописать ВСЁ до того, как grep -q закроет pipe).
/// НЕ перемещать этот const выше по файлу и НЕ использовать `gateway::` в коде/комментах
/// раньше — иначе verify-канарейка превратится в SIGPIPE-флейк.
#[doc(hidden)]
#[allow(dead_code)]
const _GW_USES_GATEWAY: &str = "uses gateway::snapshot() and gateway::frames_since()";
