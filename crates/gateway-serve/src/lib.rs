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

/// Wire-конверт сообщений WS (MVP — JSON, версионированный через `schema_version` внутри Snapshot/Frame).
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
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::_gw::Selector;
    use futures_util::{SinkExt, StreamExt};
    use journal::EpochFilter;
    use jsonwebtoken::DecodingKey;
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

        // (6) Авторизован → snapshot-при-подключении + push-loop. Read-only.
        run_authorized_session(ws_stream, cfg, claims).await
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

    /// Авторизованная сессия: snapshot → push-loop → обработка клиентских сообщений.
    async fn run_authorized_session<S>(
        ws: WebSocketStream<S>,
        cfg: Arc<ServeConfig>,
        claims: super::auth::Claims,
    ) -> std::io::Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let (mut sink, mut stream) = ws.split();

        // (6a) Snapshot-при-подключении. Snapshot идёт целиком (M-22 deterministic).
        // M-38b (B3): если в конфиге есть чекпоинт — `snapshot_from_checkpoint` потребляет
        // его, иначе прозрачно rebuilds от START. `ReadStats` логируются: §8 eyes-on видит
        // «полегчало, читается хвост» через кривую latency или ручной grep.
        let (snap_msg, stats) = super::serve::snapshot_msg(
            cfg.journal_dir.as_path(),
            cfg.filter.clone(),
            &cfg.selector,
            crate::_gw::Cursor::LATEST,
            cfg.checkpoint_dir.as_deref(),
        )?;
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

        // (6b) Push-loop: периодически опрашиваем `frames_since` от последнего курсора.
        // Bounded: `max_events = 256` за вызов (GW-I-2 — лимит на пак, клиент догоняет курсор).
        const PUSH_INTERVAL_MS: u64 = 250;
        const PUSH_MAX_EVENTS: usize = 256;

        let mut cursor = match &snap_msg {
            super::wire::ServeMsg::Snapshot(s) => s.cursor,
            _ => crate::_gw::Cursor::START,
        };
        let mut push_tick = tokio::time::interval(Duration::from_millis(PUSH_INTERVAL_MS));
        push_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                // Клиент отключился / ошибка приёма → выходим.
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
                        Some(Ok(Message::Text(_))) | Some(Ok(Message::Binary(_))) => {
                            // MVP: replay-контролы НЕ реализованы (только чтение).
                            // Будущие фреймы с cursor/window будут интерпретироваться здесь.
                        }
                        Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => {}
                    }
                }
                // Периодический push: инкрементальные кадры от последнего курсора.
                _ = push_tick.tick() => {
                    let (msgs, new_cursor) = match super::serve::frames_msgs(
                        cfg.journal_dir.as_path(),
                        cfg.filter.clone(),
                        &cfg.selector,
                        cursor,
                        PUSH_MAX_EVENTS,
                    ) {
                        Ok(pair) => pair,
                        Err(e) => {
                            // RN-21 (reviewer, M-47 PR-гейт): в проде отказ
                            // `frames_msgs` — это NEW live-push канал (M-38b задача #6
                            // вводит резюмируемый `LiveReducer`). Раньше был debug — молча
                            // проглатывали, и §8 не видел проблему. Поведение соединения
                            // (молча продолжаем, НЕ закрываем WS) сохраняем — НО поднимаем
                            // до `error!` с курсором/селектором в контексте, чтобы §8 eyes-on
                            // обнаружил «чекпоинтер/reducer сломался» по логу, а не по жалобе
                            // оператора в 3 AM.
                            tracing::error!(
                                error = %e,
                                cursor = ?cursor,
                                symbol = %cfg.selector.symbol,
                                venue = ?cfg.selector.venue,
                                "frames_msgs failed (журнал/чекпоинтер недоступен) — соединение продолжается, но live-push молчит"
                            );
                            continue;
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
        frames_since, snapshot, snapshot_from_checkpoint, Cursor, Frame, ReadStats, Selector,
        Snapshot,
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
