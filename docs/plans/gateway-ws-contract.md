<!-- FACTS: audited_head=8dccd6283a8be47bb3e60d8cf34f5dc8495ab63f collected=2026-08-02 -->
# WS-контракт `gateway-serve` — фактура для RED-оракулов

Сбор: scout, 2026-08-02. Источник — detached worktree `/tmp/hft-scout-gw` на `origin/main`
(`8dccd62 docs(critic): C-054 — аудит CT-RFC-07 rev3 ...`). Репозиторий не изменялся.

Все ссылки — `путь:строка` относительно корня репо. Где факта нет — написано «не найдено».

---

## 0. Главное, что меняет постановку задачи

Три факта, каждый подтверждён кодом/замером, ломают исходную гипотезу «неизвестно, отдаёт ли
сервис данные вообще»:

1. **Сервис ОТДАЁТ данные, и это ЗАМЕРЕНО на проде.** M-48 §8 eyes-on (`PROJECT-STATE.md:1886-1899`,
   прод `0215b34`): `LATENCY first Snapshot (от ws handshake) = 1.056 s`, `schema_version=8`,
   `history_start_seq=16049334`, `history_truncated=True`, `cursor={"upto_seq":118449099}`,
   `ohlcv len=61`, `heatmap len=1697`. Историческая кривая латентности: `409.74 → 382.657 → 1.056 s`.
   То есть полный WS-путь (handshake + JWT + snapshot) на проде проходил успешно как минимум
   один раз, после прогрева чекпоинта.
2. **Контейнер `gateway-serve` НЕДОСТИЖИМ снаружи своего netns.** В `docker-compose.yml` у сервиса
   `gateway-serve` (`docker-compose.yml:111-156`) **нет** `ports:`, **нет** `expose:`, **нет**
   `network_mode: host` — проверено по всему файлу: `grep -n "ports:\|network_mode:\|networks:"
   docker-compose.yml` → пусто. При этом bind-адрес `GATEWAY_ADDR: ${GATEWAY_ADDR:-127.0.0.1:8080}`
   (`docker-compose.yml:118`) — это loopback **внутри контейнера**. Следствие: с хоста VPS
   `127.0.0.1:8080` НЕ ведёт в gateway-serve; ssh-туннель на хостовый `127.0.0.1:8080` работать
   не будет (см. §9).
3. **Наблюдаемый в логах `ws handshake failed: Handshake not finished` каждые 30 s — это healthcheck,
   и это единственный клиент.** Healthcheck (`docker-compose.yml:146-153`) —
   `timeout 2 bash -c '</dev/tcp/127.0.0.1/8080'`, `interval: 30s`. Он выполняется ВНУТРИ контейнера
   (потому и достаёт до loopback'а), открывает TCP и сразу закрывает → `accept_hdr_async` получает EOF
   → лог пишется в `crates/gateway-serve/src/lib.rs:282` (`tracing::debug!(error = %e, "ws handshake
   failed")`). Уровень `debug` виден, т.к. дефолтный фильтр — `"info,gateway_serve=debug"`
   (`crates/gateway-serve/src/main.rs:54`).

---

## 1. Протокол подключения

### URL / путь
- Схема: **`ws://<addr>/?token=<jwt>`**. Путь не проверяется вообще — роутинга нет, любой path
  принимается. Токен читается ТОЛЬКО из query-строки.
- Точка извлечения query: `crates/gateway-serve/src/lib.rs:271` — `req.uri().query()` в
  handshake-коллбэке `accept_hdr_async`.
- Парсер токена: `crates/gateway-serve/src/lib.rs:478-488` (`fn parse_token`) — сплит по `&`, затем
  `splitn(2, '=')`, ищется ключ ровно `token`, значение непустое. **URL-decode НЕ выполняется**
  (комментарий `lib.rs:476-477`: JWT — base64url, `%`/`+` не содержит).
- Пример из смоук-теста: `crates/gateway-serve/tests/smoke_ws.rs:97` —
  `format!("ws://{addr}/?token={token}")`.
- TLS нет: `tokio-tungstenite` подключён с `rustls-tls-webpki-roots`
  (`crates/gateway-serve/Cargo.toml:23`), но сервер поднимается через голый `TcpListener`
  (`crates/gateway-serve/src/lib.rs:210`) и `accept_hdr_async` (`lib.rs:278`) — только `ws://`,
  не `wss://`. TLS/reverse-proxy объявлены инфра-слоем вне scope (`docker-compose.yml:110`).

### JWT: где проверяется, чем подписан, какие claims
- Функция верификации: `crates/gateway-serve/src/lib.rs:35-53` — `auth::verify_token(token, key)`.
- Алгоритм: **HS256**, `Validation::new(Algorithm::HS256)` (`lib.rs:42`). Ed25519 упомянут как
  «по founder» в доке (`lib.rs:34`), но НЕ реализован.
- Ключ: `DecodingKey::from_secret(secret.as_bytes())` из **`GATEWAY_JWT_SECRET`**
  (`crates/gateway-serve/src/lib.rs:554-558`, `lib.rs:629`). Общий секрет с Next.js-подписателем
  (D6). Переменная **обязательна**: отсутствует или пустая → `Err` из `serve_config_from_env`
  → бинарь выходит с `ExitCode::from(2)` (`crates/gateway-serve/src/main.rs:23-26`).
- **Обязательные claims** — структура `Claims` (`crates/gateway-serve/src/lib.rs:17-21`):
  ```rust
  pub struct Claims { pub sub: String, pub exp: usize }
  ```
  Оба поля обязательны для десериализации (`sub` — строка, `exp` — unix-секунды).
  `required_spec_claims` = `{"exp"}` по дефолту `Validation`; `exp` валидируется с leeway 60 s
  (комментарий `lib.rs:36-41`).
- **Что НЕ проверяется:** `iss`, `aud`, `nbf`, содержимое `sub`. Комментарий `lib.rs:38-41` явно:
  «мы НЕ доверяем claim-метаданным Next.js для авторизации, только самой подписи». `aud` в токене
  быть НЕ должно (`validate_aud = true` по дефолту при `aud = None` → наличие `aud` даст отказ).
- **Никакой авторизации по содержимому нет**: любой валидно подписанный не-истёкший токен с любым
  `sub` получает полный доступ (stateless, без user-БД — инвариант GS-I-2).

### Что происходит при невалидном / отсутствующем токене
Все ветки — в `handle_conn`, `crates/gateway-serve/src/lib.rs:251-325`. Общая схема: **handshake
СНАЧАЛА завершается успешно (HTTP 101)**, отказ приходит уже как WS-фрейм. Это сознательное
решение (`lib.rs:255-257`): «Мы НЕ отказываем в коллбэке — откажем позже, ПОСЛЕ verify_token».

| Ситуация | Реакция | Строка |
|---|---|---|
| query отсутствует целиком | `ServeMsg::Error("missing token query")` + Close | `lib.rs:288-294` |
| query есть, но нет `token=` / значение пустое | `ServeMsg::Error("missing token")` + Close | `lib.rs:298-304` |
| `exp` в прошлом | `ServeMsg::Error("expired token")` + Close | `lib.rs:310-314` |
| плохая подпись / чужой ключ / мусор / чужой алгоритм | `ServeMsg::Error("invalid token")` + Close | `lib.rs:315-319` |
| сбой самого WS-handshake (не-WS мусор, TCP-connect-and-close) | тихий выход, `tracing::debug!("ws handshake failed")`, клиенту НИЧЕГО | `lib.rs:278-285` |

Механика отказа — `close_with_error` (`lib.rs:328-345`): сначала `Message::Text` с JSON'ом
`ServeMsg::Error`, затем `ws.close(None)`.

**Важно для оракула:** отказ приходит НЕ как HTTP 401. `connect_async` у клиента УСПЕШЕН, и первый
фрейм — `{"Error":"..."}`. Смоук-тест это учитывает (`smoke_ws.rs:124-131`: и `Err`, и «не Snapshot»
считаются отказом).

---

## 2. Протокол подписки

**Клиент не шлёт НИЧЕГО. Поток начинается сразу после успешного verify.** Подписка отсутствует как
концепция.

- После `verify_token` → `run_authorized_session` (`crates/gateway-serve/src/lib.rs:324`).
- Первое, что делает сервер — строит и шлёт Snapshot, без ожидания сообщения от клиента
  (`lib.rs:362-384`).
- Клиентские сообщения **читаются и игнорируются**: `crates/gateway-serve/src/lib.rs:416-419` —
  ветка `Message::Text(_) | Message::Binary(_)` с комментарием «MVP: replay-контролы НЕ реализованы
  (только чтение). Будущие фреймы с cursor/window будут интерпретироваться здесь».
- Обрабатываются штатно только `Ping` → `Pong` (`lib.rs:412-414`) и `Close` → выход (`lib.rs:415`).
- **`replay` НЕ доступен через WS.** `gateway::replay` (`crates/gateway/src/lib.rs:1784`) существует
  в библиотеке, но у `gateway-serve` нет ни call-site, ни протокола для его вызова — проверено:
  `_gw` реэкспортирует только `frames_since, snapshot, snapshot_from_checkpoint, Cursor, Frame,
  ReadStats, Selector, Snapshot` (`crates/gateway-serve/src/lib.rs:499-502`), `replay` в списке нет.
  Доккомментарии `lib.rs:139`/`lib.rs:226` обещают replay — это НЕ реализовано.

### `(venue, symbol, timeframe, bands)` — ТОЛЬКО из env сервера

Клиент не может выбрать инструмент. Один процесс = одна `(venue, symbol)`
(`crates/gateway-serve/src/lib.rs:159` — «MVP — одна `(venue, symbol)`; мульти-подписка позже»).

Сборка — `serve_config_from_env` (`crates/gateway-serve/src/lib.rs:548-632`):

| ENV | Дефолт | Парсинг / отказ | Строка |
|---|---|---|---|
| `GATEWAY_JWT_SECRET` | — (**обязательна**) | unset/пусто → `Err` → exit 2 | `lib.rs:554-558` |
| `GATEWAY_ADDR` | `127.0.0.1:8080` | as-is в `TcpListener::bind` | `lib.rs:560` |
| `GATEWAY_JOURNAL_DIR` | `./journal-data` | `PathBuf` | `lib.rs:562-564` |
| `GATEWAY_VENUE` | `Binance` | только `Binance\|BinanceFutures\|Hyperliquid`, иначе `Err` | `lib.rs:566-574` |
| `GATEWAY_SYMBOL` | `BTCUSDT` | строка as-is | `lib.rs:576` |
| `GATEWAY_TIMEFRAME_MS` | `1000` | `i64`; **fail-closed гвард GW-I-10**: `<= 0` или `86_400_000 % tf != 0` → `Err` → exit 2 | `lib.rs:578-597` |
| `GATEWAY_BANDS` | `0.001` | comma-separated `f64`, ошибка парса → `Err` | `lib.rs:599-604` |
| `GATEWAY_WINDOW_MS` | `None` | **fail-closed гвард `GW-I-14` (M-69, задачи #1–#4 РЕАЛИЗОВАНЫ):** `unset`/пусто/`"0"` → `None` (offline, канонизировано); parse-error / переполнение `i64` / отрицательное → `Err` на старте с сообщением, называющим переменную; иначе `Some(положительное)`. Прежнее поведение (дефект R7) — `.parse::<i64>().ok()`, мусор молча давал `None`, то есть опечатка возвращала прод в unbounded БЕЗ отказа старта | `lib.rs:741-779` |
| `GATEWAY_CHECKPOINT_DIR` | `None` | unset/пусто → `None` (не ошибка) | `lib.rs:618-622` |

Не из env, захардкожено: **`filter: EpochFilter::OwnCaptureOnly`** (`crates/gateway-serve/src/lib.rs:627`).
Клиент не может запросить другую эпоху.

Прод-значения (`docker-compose.yml:115-135`): `GATEWAY_JOURNAL_DIR=/journal`, `GATEWAY_VENUE=Binance`,
`GATEWAY_SYMBOL=BTCUSDT`, `GATEWAY_TIMEFRAME_MS=1000`, `GATEWAY_BANDS=0.001`,
`GATEWAY_WINDOW_MS=60000`, `GATEWAY_CHECKPOINT_DIR=/ckpt`.

---

## 3. Формат сообщений сервера — точные типы

### Кодек
**JSON через `serde_json`**, не postcard. Обоснование в доке модуля
(`crates/gateway-serve/src/lib.rs:8-9`): «Wire-формат MVP — JSON (JS-декодируемо; postcard —
Rust-only, НЕ годится для фронта)».
Точки сериализации: `crates/gateway-serve/src/lib.rs:332` (Error), `lib.rs:378` (Snapshot),
`lib.rs:458` (Frame). Транспорт — `Message::Text` (UTF-8), НЕ Binary: `lib.rs:343`, `lib.rs:382`,
`lib.rs:466`.

### Конверт
`crates/gateway-serve/src/lib.rs:62-67`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServeMsg {
    Snapshot(Snapshot),
    Frame(Frame),
    Error(String),
}
```
Externally-tagged (дефолт serde для enum) ⇒ на проводе: `{"Snapshot":{...}}`, `{"Frame":{...}}`,
`{"Error":"invalid token"}`.

### Snapshot vs Frame — это РАЗНЫЕ типы

Да, разные. Snapshot — полная свёртка, Frame — приращение.

`crates/gateway/src/lib.rs:307-330`:
```rust
pub struct Snapshot {
    pub schema_version: u32,      // == GATEWAY_SCHEMA_VERSION
    pub selector: Selector,
    pub cursor: Cursor,           // до какого seq включительно свёрнуто
    pub series: SeriesBundle,
    #[serde(default)] pub history_start_seq: u64,   // M-48/VB-I-11
    #[serde(default)] pub history_truncated: bool,  // M-48/VB-I-11
}
```

`crates/gateway/src/lib.rs:334-346`:
```rust
pub struct Frame {
    pub schema_version: u32,
    pub from: Cursor,
    pub to: Cursor,
    pub delta: SeriesBundle,      // ТОТ ЖЕ тип, что Snapshot.series
    #[serde(default)] pub at_ms: i64,  // ts последнего события кадра (M-37)
}
```

**`GATEWAY_SCHEMA_VERSION = 8`** — `crates/gateway/src/lib.rs:65`. Прод-замер подтверждает
`schema_version=8` (`PROJECT-STATE.md:1894`).

### `Cursor` — `crates/gateway/src/lib.rs:147-172`
```rust
pub struct Cursor { pub upto_seq: Option<u64> }
```
JSON: `{"upto_seq":118449099}` или `{"upto_seq":null}` (= `START`).
Константы: `START = {upto_seq: None}` (`lib.rs:154`), `LATEST = {upto_seq: Some(u64::MAX)}`
(`lib.rs:156-158`).

### `Selector` — `crates/gateway/src/lib.rs:108-119`
```rust
pub struct Selector {
    pub venue: Venue,
    pub symbol: String,
    pub timeframe_ms: i64,
    pub bands: Vec<f64>,
    #[serde(default)] pub window_ms: Option<i64>,
}
```

### `SeriesBundle` — полный список полей, `crates/gateway/src/lib.rs:247-304`

| Поле | Тип | Семантика | Строка |
|---|---|---|---|
| `ohlcv` | `Vec<OhlcvRow>` | свечи per бакет | `lib.rs:249` |
| `cumulative_delta` | `Vec<(i64,i64)>` | `(time_s, running CVD)`; **ресет на 00:00 UTC** (M-38a) | `lib.rs:255` |
| `cvd_session_base` | `Vec<(i64,i64)>` `#[serde(default)]` | `(session_id, base)` — база CVD-ledger'а после эвикции | `lib.rs:268` |
| `depth_series` | `Vec<DepthRow>` | глубина per `(side, band)` | `lib.rs:269` |
| `vwap` | `Vec<(i64,i64)>` | `(time_s, price ×1e8)`, **all-time БЕЗ ресета на 00:00** | `lib.rs:274` |
| `volume_profile` | `Vec<VolumeProfileRow>` | SVP per UTC-сессия | `lib.rs:277` |
| `vp_session_max_time_s` | `Vec<(i64,i64)>` `#[serde(default)]` | `(session_id, max_time_s)` — зеркало критерия эвикции (TD-045) | `lib.rs:292` |
| `heatmap` | `Vec<HeatmapCell>` | **это то, что нужно для heatmap-задачи** | `lib.rs:297` |
| `cob` | `Vec<CobLevel>` | книга на финальном курсоре | `lib.rs:300` |
| `volume_bubbles` | `Vec<BubbleCell>` | торгованный объём `(time_s, price) → (buy,sell)` | `lib.rs:303` |

`SeriesBundle` — `#[derive(Default)]`, все поля публичные.

### Элементарные строки

```rust
// crates/gateway/src/lib.rs:176-183
pub struct OhlcvRow { pub time_s: i64, pub open: i64, pub high: i64,
                      pub low: i64, pub close: i64, pub volume: i64 }

// crates/gateway/src/lib.rs:205-211  ← ГЛАВНОЕ для heatmap
pub struct HeatmapCell {
    pub time_s: i64,
    pub side: String,          // "bid" | "ask"
    pub price_e8: i64,
    pub size_e8: i64,
    pub depth_band_provenance: Option<String>,  // None для полос ≤1.3%, Some для deep
}

// crates/gateway/src/lib.rs:216-220
pub struct CobLevel { pub side: String, pub price_e8: i64, pub size_e8: i64 }

// crates/gateway/src/lib.rs:225-230
pub struct BubbleCell { pub time_s: i64, pub price_e8: i64,
                        pub buy_vol_e8: i64, pub sell_vol_e8: i64 }

// crates/gateway/src/lib.rs:234-244
pub struct DepthRow {
    pub side: String,            // "bid" | "ask" — НЕ суммируются
    pub band_pct_e8: i64,        // доля ×1e8: 0.001 → 100_000
    pub series: Vec<(i64,i64)>,  // (time_s, depth ×1e8), close-семантика
    pub depth_band_provenance: Option<String>,
}

// crates/gateway/src/lib.rs:190-198
pub struct VolumeProfileRow {
    pub session_id: i64, pub poc_e8: i64, pub vah_e8: i64,
    pub val_e8: i64, pub va_pct_e8: i64,
    pub bins: Vec<(i64,i64)>,    // (price_e8, volume_e8), сорт по price ↑
}
```

Все цены/размеры — фиксированная точка **×1e8** (`i64`). `time_s` — UTC-секунды.

### Кадры на проводе — тайминг и объём
- Push-цикл: интервал **250 ms**, `PUSH_INTERVAL_MS` (`crates/gateway-serve/src/lib.rs:388`).
- Кап на пак: **`max_events = 256`**, `PUSH_MAX_EVENTS` (`lib.rs:389`).
- На каждом тике `frames_since` возвращает **максимум ОДИН `Frame`** (`crates/gateway/src/lib.rs:1778`
  — `Ok((vec![Frame::versioned(...)], cursor))`) либо пустой вектор, если новых событий нет
  (`lib.rs:1775-1777`). То есть «≤1 Frame раз в 250 ms», а не поток по событию.
- Snapshot в push-цикл не попадает — явный фильтр `crates/gateway-serve/src/lib.rs:455-457`.

### Поведение при ошибке чтения журнала в push-цикле
Соединение **НЕ закрывается**, ошибка логируется на `error!` и цикл продолжается
(`crates/gateway-serve/src/lib.rs:433-450`, RN-21). Клиент получает Snapshot и потом тишину —
никакого `ServeMsg::Error` в живой сессии не приходит. Это важная для оракула асимметрия:
`ServeMsg::Error` встречается ТОЛЬКО на auth-пути.

---

## 4. Снапшот-при-подключении

### Откуда берётся
Цепочка: `run_authorized_session` → `serve::snapshot_msg` (`crates/gateway-serve/src/lib.rs:362-368`)
→ `gateway::snapshot_from_checkpoint` (`crates/gateway-serve/src/lib.rs:109-116`) →
`crates/gateway/src/lib.rs:1859-1936`.

Курсор запроса — **всегда `Cursor::LATEST`** (`crates/gateway-serve/src/lib.rs:366`). Клиент не может
попросить снапшот на другом курсоре.

Алгоритм `snapshot_from_checkpoint` (`crates/gateway/src/lib.rs:1859-1936`):
1. `validate_selector(sel)?` — GW-I-10 гвард (`lib.rs:1866`).
2. `checkpoint::read_checkpoint(dir, ckpt_dir, sel, filter)` (`lib.rs:1871-1872`).
3. Если чекпоинт валиден и `ckpt.cursor <= at` — состояние восстанавливается, хвост досчитывается
   через `journal::stream_from(dir, filter, ckpt_cursor.upto_seq)` (`lib.rs:1885`). Это GW-I-11:
   сегментный skip, декодируется только хвост.
4. `history_start_seq` / `history_truncated` берутся **из заголовка чекпоинта**, а не из хвостовых
   событий (`lib.rs:1900-1910`).
5. Иначе — fallback `journal::stream` от START, полный реплей (`lib.rs:1917-1935`);
   `history_start_seq = first_folded_seq`, `history_truncated = first_folded_seq > 0`.

### Что при отсутствии чекпоинта
**Не ошибка.** `GATEWAY_CHECKPOINT_DIR` unset → `checkpoint_dir = None`
(`crates/gateway-serve/src/lib.rs:618-622`) → `snapshot_msg` подставляет `Path::new("")`
(`crates/gateway-serve/src/lib.rs:111-113`), `read_checkpoint` спотыкается на `ckpt_path.exists()`
и возвращает `Ok(None)` → тихий rebuild от START. Это **GW-I-9(б)**: любая невалидность
(нет файла / битый / чужой magic / чужая версия / фингерпринт не сошёлся / CRC / `cursor > at`) →
ТИХИЙ rebuild, без ошибки (`crates/gateway/src/lib.rs:1810-1815`, `lib.rs:1851-1855`).

Цена fallback'а измерена на проде: **382.657 s** при пустом `/ckpt`, журнал 23 GB
(`TECH-DEBT.md:1742`). С прогретым чекпоинтом — **1.056 s** (`PROJECT-STATE.md:1894`).

### `ReadStats` — честный счётчик, наружу НЕ уходит
`crates/gateway/src/lib.rs:1820-1824`:
```rust
pub struct ReadStats { pub events_decoded: u64, pub segments_opened: u32 }
```
Возвращается из `snapshot_msg` (`crates/gateway-serve/src/lib.rs:108`), но в WS **не сериализуется** —
только логируется на `debug` (`crates/gateway-serve/src/lib.rs:372-377`). Для проверки GW-I-11
через WS его не видно; нужен либо лог, либо прямой вызов библиотеки.

### Ограничение окна — `GATEWAY_WINDOW_MS` (TD-039 / TD-020)

**Это НЕ ограничение снапшота по запросу, а ограничение состояния редьюсера.**

- `Selector.window_ms: Option<i64>` (`crates/gateway/src/lib.rs:114-118`).
- `Selector::window_lo_time_s(at_ms)` (`crates/gateway/src/lib.rs:129-139`) считает нижнюю границу
  окна `[at−W, at]` в `time_s`, привязываясь к **курсору**, а не к wall-clock (`lib.rs:106-107` —
  «иначе ломается VB-I-2 live==replay под нагрузкой»).
- Эвикция вызывается в конце каждого `Reducer::apply` (`crates/gateway/src/lib.rs:943-945`).
- `None` → unbounded (offline-режим); `Some(W)` → live-cockpit (`crates/gateway/src/lib.rs:100-104`).
- Прод: `GATEWAY_WINDOW_MS=60000` (`docker-compose.yml:139`; замер на VPS 2026-08-18 подтверждает
  то же значение в живом контейнере) ⇒ ~60 бакетов при `timeframe_ms=1000`.

**След TD-039/TD-020 в коде** (класс «механизм есть, никто не зовёт»):
- `crates/gateway-serve/tests/red_serve_window_wiring.rs:1-15` — прямая формулировка: reducer был
  реализован, но НЕДОСТИЖИМ из бинаря, `build_selector` не принимал окно, `main.rs` инлайнил
  `std::env` и не читал `GATEWAY_WINDOW_MS` → прод `window_ms == None` → OOM не исправлен.
- Лечение: `build_selector` с 5-м аргументом (`crates/gateway-serve/src/lib.rs:512-526`) +
  тестируемая `serve_config_from_env` с инжектируемым getter'ом (`lib.rs:548`), `main` — тонкий
  вызыватель (`crates/gateway-serve/src/main.rs:21`).
- **Асимметрия, названная здесь 03.08, ЗАКРЫТА milestone'ом M-69 (`GW-I-14`) на ветке
  `feat/M-69-window-guard`.** Прошедшее время появилось здесь только сейчас и только потому,
  что предмет действительно свершился: документ назван фактурой для RED-оракулов, историческим
  не помечен, и опережающее «сделано» в нём было бы тиражируемой ложью класса `TD-155`
  (`A-014` B-7). Флип выполнен architect'ом ПОСЛЕ GREEN dev'а, до PR-time (`A-014` §5 п.3);
  правдивость на ДЕРЕВЕ СЛИЯНИЯ проверяет reviewer штатной нормой `gates.md` §8.
  **ПРЕЖНЕЕ поведение (дефект R7), которое M-69 снял:** `GATEWAY_WINDOW_MS` с невалидным
  числом молча давал `None` (`.parse::<i64>().ok()`); опечатка в env возвращала прод в
  unbounded-режим БЕЗ отказа старта, в отличие от `GATEWAY_TIMEFRAME_MS`, fail-closed с M-47.
  Второй путь мимо гварда: `validate_selector` не смотрел `window_ms` вовсе.
  **ДЕЙСТВУЮЩЕЕ поведение (M-69, задачи #1–#4 ✅ DONE):** offline выражается тремя формами
  (`unset`/пусто/`"0"` → `None`, канонизация — `C-099` B-2, чтобы `selector_fingerprint` не
  расщеплял offline на два ключа чекпоинта); всё прочее обязано быть корректным положительным
  `i64`, иначе отказ на старте (`PL-I-5`, `DESIGN.md:940`). Обе точки закрыты:
  `crates/gateway-serve/src/lib.rs:741-779` (старт прод-бинаря, `match` вместо `.ok()`) и
  `crates/gateway/src/lib.rs:1763-1783` (`validate_selector` — анти-байпас для
  чекпоинтера/shared-tailer/research-cli).
  Приглашение «RED-оракул может атаковать» принято; оракулы написаны и **ЗЕЛЕНЫ**:
  `crates/gateway-serve/tests/red_window_guard_startup.rs` (граница процесса) и
  `crates/gateway/tests/red_window_selector_guard.rs` (библиотека). Оба несут проверки
  ПЕРЕШИРОКОСТИ, а не только нарушения: `None` и `Some(0)` обязаны приниматься.

---

## 5. Семантика `bands`

**`GATEWAY_BANDS=0.001` — это ДОЛЯ (fraction) от mid, не шаг цены.** `0.001 = 0.1%`.

Подтверждения:
- `DepthRow.band_pct_e8` (`crates/gateway/src/lib.rs:237-238`): «Полоса в долях ×1e8
  (0.001 ×1e8 = 100000 = 0.1%)».
- Конвертация: `band_pct_e8: (band * 1e8).round() as i64` (`crates/gateway/src/lib.rs:910`).

### Функция-редуктор: `depth_within`
`crates/gateway/src/lib.rs:1197-1228`:
```rust
fn depth_within(bids: &[Level], asks: &[Level], side: Side, band: f64) -> i64
```
- `best_bid` = max цена среди `size > 0` (`lib.rs:1198-1202`); `best_ask` = min (`lib.rs:1203-1207`).
- Любая сторона пуста → возвращает `0` (`lib.rs:1208-1210`).
- `mid = (best_bid + best_ask) / 2` — **целочисленное деление** по e8-ценам (`lib.rs:1211`).
- `Side::Buy`: `threshold = (mid as f64 * (1.0 - band)) as i64`, суммируются `size` бидов с
  `price >= threshold` (`lib.rs:1213-1219`).
- `Side::Sell`: `threshold = (mid as f64 * (1.0 + band)) as i64`, суммируются `size` асков с
  `price <= threshold` (`lib.rs:1220-1226`).
- BID и ASK — **раздельные серии, не суммируются** (`crates/gateway/src/lib.rs:232-235`).

### Путь `L2Delta` / `L2Snapshot` → провод

Диспетчер — `Reducer::apply`, `crates/gateway/src/lib.rs:843-946`. Ключевая **асимметрия**, которую
architect обязан учесть в оракуле:

| Вход | Книга | heatmap | `depth_series` (полосы) | Строки |
|---|---|---|---|---|
| `MdPayload::L2Snapshot` | `book.apply_snapshot(bids, asks)` (replace) | `refresh_heatmap_bucket(time_s)` | **ОБНОВЛЯЕТСЯ** — `depth_within` по каждой `(band, side)` | `lib.rs:892-919` |
| `MdPayload::L2Delta` | `book.apply_delta(bids, asks)` (size==0 → remove, size>0 → upsert) | `refresh_heatmap_bucket(time_s)` | **НЕ ОБНОВЛЯЕТСЯ** | `lib.rs:921-935` |
| `MdPayload::Trade` | — | — | — (идёт в ohlcv/cvd/vwap/vp/bubbles) | `lib.rs:867-891` |

Явный комментарий `crates/gateway/src/lib.rs:928-929`: «depth_series (полосы) НЕ апдейтится —
депт-серия остаётся snapshot-only (M-22 семантика)». Лениво-инициализируемые аккумуляторы полос
создаются только в ветке `L2Snapshot` (`lib.rs:904-915`), поэтому **на журнале без единого
`L2Snapshot` `depth_series` будет ПУСТ навсегда**, сколько бы `L2Delta` ни пришло.

### `bands` для heatmap — другая роль
Для heatmap `bands` задают ширину ценового окна: `W = max(bands)`, ячейки только в
`[mid*(1−W), mid*(1+W)]`.
- `crates/gateway/src/lib.rs:1077` — `let w = selector.bands.iter().copied().fold(0.0_f64, f64::max);`
- Комментарии: `lib.rs:295-296`, `lib.rs:1065`.
- Функция-строитель: `build_heatmap_and_cob` (`crates/gateway/src/lib.rs:1073`).
- **Детерминизм GW-I-3 / HM-I-5**: выход нормализуется по ключу `(time_s, side, price_e8)`,
  выходы `build` и `merge` байт-идентичны (`crates/gateway/src/lib.rs:1069`, `lib.rs:1101`).

При прод-`GATEWAY_BANDS=0.001` окно heatmap = ±0.1% от mid. Замер прода дал `heatmap len=1697`
при `ohlcv len=61` (`PROJECT-STATE.md:1894-1895`).

### Прочее по редьюсеру
- `reduce_event_stream` (`crates/gateway/src/lib.rs:1230-1278`) — общая точка для
  `snapshot`/`frames_since`/`replay`. Возврат: `(SeriesBundle, Cursor, consumed, at_ms, first_folded_seq)`.
- События `seq <= after` не сворачиваются, но **питают VWAP** через `seed_vwap`
  (`lib.rs:1257-1260`) — иначе all-time VWAP был бы неверен. `seed_vwap` НЕ инкрементирует
  `consumed` (`lib.rs:1266-1268`).
- Fold на стороне клиента: `Snapshot::apply(&Frame)` (`crates/gateway/src/lib.rs:1294`) — merge
  бакетов, пересекающихся по `time_s`, без дублирования.

---

## 6. Существующее тестовое покрытие

### `crates/gateway-serve/tests/**` (6 файлов)

| Файл | Что проверяет | Тест-функции |
|---|---|---|
| `red_jwt_verify.rs` | **GS-I-2** — stateless JWT-verify: только `(token,key)`, без БД | `valid_token_ok`:27, `wrong_key_err`:35, `expired_token_err`:44, `malformed_token_err`:54 |
| `red_serve_passthrough.rs` | **GS-I-4/GS-I-5** — `ServeMsg` JSON-roundtrip + поэлементное равенство с `gateway::{snapshot,frames_since}` | `frames_msgs_passthrough_equals_library`:55, `snapshot_msg_roundtrips_and_matches_library`:91 |
| `red_serve_window_wiring.rs` | **TD-039/TD-020** — `GATEWAY_WINDOW_MS` → `Selector.window_ms` (анти-инерт) | `window_ms_env_flows_to_selector`:28, `window_ms_absent_defaults_none`:44, `build_selector_propagates_window`:55 |
| `red_serve_consumes_checkpoint.rs` | **M-38b B3** — прод-путь ДЕЙСТВИТЕЛЬНО потребляет чекпоинт (по `ReadStats`, не по grep) | `checkpoint_dir_env_flows_to_config`:98, `absent_checkpoint_dir_is_not_an_error`:115, `snapshot_msg_consumes_checkpoint_and_reads_only_tail`:129 |
| `red_timeframe_guard_startup.rs` | **GW-I-10 на СТАРТЕ бинаря** (TD-046) + парный vantage «не переширок» | `misaligned_timeframe_env_blocks_startup`:53, `zero_...`:59, `negative_...`:65, `weekly_...`:70, `aligned_timeframes_env_starts`:77, `default_timeframe_still_starts`:100 |
| `smoke_ws.rs` | **task #4 acceptance** — реальный WS-хендшейк на ephemeral-порту | `valid_jwt_receives_snapshot`:86, `invalid_jwt_rejected`:111 |

### ⚠ ЕСТЬ ЛИ СКВОЗНОЙ WS-ТЕСТ С JWT — ДА, ОДИН

**`crates/gateway-serve/tests/smoke_ws.rs`** — единственный. Он поднимает реальный сервер и делает
полный WS-handshake:
- `bind(config(dir.path(), secret)).await` + `server.serve()` в spawn (`smoke_ws.rs:91-95`);
- `addr = server.local_addr()` — ephemeral `127.0.0.1:0` (`smoke_ws.rs:76`, `smoke_ws.rs:92`);
- клиент `tokio_tungstenite::connect_async("ws://{addr}/?token={token}")` (`smoke_ws.rs:97-100`);
- JWT подписывается `jsonwebtoken::encode` с `Header::default()` (= HS256) (`smoke_ws.rs:24-29`);
- ассерт: первый msg — `ServeMsg::Snapshot(_)` (`smoke_ws.rs:101-107`);
- парный vantage: чужой ключ → отказ, никакого Snapshot (`smoke_ws.rs:111-137`).

Заголовок файла честно квалифицирует его: **«SMOKE ... НЕ детерм-оракул — IO/сеть»**
(`smoke_ws.rs:1`). `verify_M-28.sh:51` проверяет ЛИШЬ ФАКТ СУЩЕСТВОВАНИЯ файла
(`[ -f crates/gateway-serve/tests/smoke_ws.rs ]`), не его содержимое.

**Чего в smoke_ws.rs НЕТ (свободная зона для новых оракулов):**
- фикстура — 4 `Trade`-события, **ни одного `L2Snapshot`/`L2Delta`** (`smoke_ws.rs:43-55`) ⇒
  `heatmap`, `cob`, `depth_series` в тесте всегда пустые, инвариант heatmap не давится вообще;
- `window_ms: None` (`smoke_ws.rs:67`) — bounded-window путь не проверяется по WS;
- `checkpoint_dir: None` (`smoke_ws.rs:81`) — чекпоинт-путь по WS не проверяется;
- **не проверяется содержимое Snapshot** — только `matches!(parsed, ServeMsg::Snapshot(_))`;
  ни `schema_version`, ни `cursor`, ни серии;
- **не проверяется push-цикл** — ни одного `Frame` не читается, второй `next()` не вызывается;
- не проверяются ветки `missing token` / `missing token query` / `expired` по WS (только
  wrong-key);
- нет сверки WS-выдачи с независимым реплеем журнала.

### `crates/gateway/tests/**` (20 файлов)

Все — детерминированные, **на файловых фикстурах журнала**. Проверено:
`grep -rn "TcpListener\|connect_async\|jsonwebtoken\|tokio::test\|ws://" crates/gateway/tests/`
→ **пусто**. Ни один не поднимает сервер и не делает handshake.

| Файл | Инвариант / что проверяет | Тест-функции |
|---|---|---|
| `red_gateway_readonly.rs` | **GW-I-1** — байты всех сегментов журнала не меняются после read-операций | `gateway_reads_do_not_mutate_journal` |
| `red_gateway_bounded.rs` | **GW-I-2** — два свойства: stream working-set bounded (TD-011) и память по ОКНУ, а не по истории (M-37) | `snapshot_stream_working_set_bounded`, `snapshot_memory_bounded_by_window_not_history` |
| `red_gateway_live_eq_replay.rs` | **GW-I-3/GW-I-4/GW-I-8** — ядро: live≡replay, completeness mid-stream, курсор-границы, кап `max_events` | `snapshot_equals_folded_frames_from_start`, `mid_stream_snapshot_completeness_merges_same_bucket`, `frames_since_respects_max_events_cap`, `cursor_and_frame_bounds_are_correct`, `snapshot_and_replay_are_deterministic` |
| `red_gateway_export_v2.rs` | **GW-I-5/GW-I-6** — `schema_version` + аддитивность формы; провенанс на deep-полосе | `snapshot_carries_schema_version_and_is_v1_additive`, `deep_band_carries_provenance` |
| `red_gateway_epoch_filter.rs` | **GW-I-7** — фильтр эпох соблюдён в snapshot/frames_since/replay + `Explicit` отличим от обоих | `epoch_filter_is_honored_own_differs_from_all`, `explicit_epoch_selection_is_distinct_from_both_own_and_all`, `frames_since_honors_epoch_filter`, `replay_honors_epoch_filter` |
| `red_gateway_schema_version.rs` | Bump `GATEWAY_SCHEMA_VERSION` — **гейт**; имя файла намеренно версионно-агностично (C-032 R1) | `schema_version_constant_matches_expected`, `snapshot_carries_expected_schema_version`, `frame_carries_expected_schema_version` |
| `red_timeframe_session_alignment.rs` | **GW-I-10** в библиотеке — отказ на всех трёх публичных входах + парный vantage «не переширок» | `misaligned_timeframe_rejected_by_{snapshot,frames_since,replay}`, `zero_timeframe_rejected_not_panic`, `negative_timeframe_rejected`, `weekly_timeframe_longer_than_day_rejected`, `aligned_timeframes_accepted`, `aligned_timeframe_keeps_sessions_separate` |
| `red_gateway_window.rs` | **M-37/TD-039** — окно `[at−W,at]`, windowed live≡replay, CVD-base переживает эвикцию | `cvd_base_survives_window_eviction`, `vp_current_session_whole_not_bucket_windowed`, `cvd_two_sessions_live_across_midnight_window`, `windowed_live_eq_replay`, `windowed_live_eq_replay_overlap_multistep`, `windowed_live_eq_replay_past_session_survives_overlap` |
| `red_heatmap.rs` | **HM-I-1/2/3/5** — heatmap из L2Delta-книги, окно+провенанс, COB = финальная книга, детерминизм | `heatmap_reflects_l2delta_book`, `heatmap_windowed_and_provenance`, `cob_is_final_book`, `determinism` |
| `red_bubbles.rs` | **HM-I-4** — пузыри `(time_s,price)→{buy,sell}`, цены не выдумываются | `bubbles_buy_sell_and_not_invented` |
| `red_volume_profile.rs` | **VP-I-1..4** — SVP: POC, Value Area, ресет по сессии, тай-брейки | `vp_poc`, `vp_value_area`, `vp_session_reset`, `vp_prices_not_invented`, `vp_poc_tie_goes_to_lowest_price`, `vp_value_area_tie_expands_upward` |
| `red_vwap.rs` | **VW-I-1..4** — all-time VWAP БЕЗ ресета на полуночи, i128 на прод-масштабе, per-venue | `vwap_exact`, `vwap_i128_prod_scale`, `vwap_cumulative_across_midnight`, `vwap_per_venue` |
| `red_gateway_cvd_session.rs` | **TD-043/M-38a** — CVD ресетится на 00:00 UTC; асимметрия не течёт через границу | `cvd_resets_at_utc_session_boundary`, `cvd_asymmetric_imbalance_does_not_leak_across_boundary`, `cvd_multiple_trades_per_boundary_bucket_reset`, `cvd_three_sessions_each_reset_independently` |
| `red_checkpoint_byte_identity.rs` | **GW-I-9(а,г)** — байт-идентичность на ВСЕХ K (unbounded и windowed) + tamper-форсинг | `identical_for_every_k_unbounded`, `identical_for_every_k_windowed`, `checkpoint_before_midnight_carries_session_state`, `boundary_k_zero_and_k_equals_at`, `foreign_checkpoint_changes_output` |
| `red_checkpoint_is_cache.rs` | **GW-I-9(б,в)** — любая невалидность → тихий rebuild; `advance` идемпотентен побайтно | `missing_checkpoint_dir_rebuilds`, `empty_checkpoint_dir_rebuilds`, `corrupt_and_truncated_checkpoint_rebuild`, `foreign_selector_rebuilds`, `foreign_epoch_filter_rebuilds`, `checkpoint_ahead_of_requested_cursor_rebuilds`, `advance_is_idempotent_bytewise`, `incremental_advance_equals_single_advance` |
| `red_checkpoint_resource_bound.rs` | **GW-I-11** — снапшот от хвостового чекпоинта декодирует только хвост; честный отчёт без чекпоинта | `snapshot_from_tail_checkpoint_decodes_only_tail`, `without_checkpoint_full_replay_is_reported`, `budget_scales_with_distance_not_constant` |
| `red_frames_seek_bound.rs` | **GW-I-11+GW-I-8** — живой путь докармливается хвостом, кадры ≡ `frames_since` | `pumped_frames_identical_to_frames_since`, `checkpoint_plus_pumped_frames_equals_full_snapshot`, `pump_at_tail_is_bounded`, `resume_without_checkpoint_reports_full_replay` |
| `red_checkpoint_prefix_pruned.rs` | **C-030 R1/R3** — скрытый полный реплей физически невозможен; lineage переживает удаление покрытого префикса | `covered_prefix_pruned_output_still_byte_identical`, `repeated_advance_and_prune_cycles_stay_identical`, `missing_uncovered_segment_is_not_invented_from_checkpoint` |
| `red_checkpoint_bootstrap_truncated.rs` | **GW-I-12** — усечённая история декларируется; разрыв «ckpt↔журнал» громкий; stale-версия самолечится | `bootstrap_on_truncated_journal_succeeds`, `truncation_is_declared_identically_on_both_paths`, `intact_journal_is_not_declared_truncated`, `gap_between_checkpoint_and_journal_is_loud`, `contiguous_boundary_is_not_a_gap`, `history_start_seq_ignores_lying_legacy_header`, `advance_after_covered_prune_does_not_regress_history_start`, `advance_to_lower_cursor_does_not_regress_checkpoint`, `advance_to_higher_cursor_does_update_checkpoint`, `stale_schema_version_checkpoint_rebuilds_silently_and_overwrites` |
| `red_checkpoint_bin_prod_argv.rs` | **M-38b B1/B2** — прод-бинарь стартует РОВНО compose-аргументами и публикует ФАКТИЧЕСКИЙ курсор, не `u64::MAX` | `starts_with_exact_compose_argv`, `starts_with_space_separated_argv_too`, `cursor_latest_is_accepted`, `published_coverage_is_real_cursor_not_max`, `published_coverage_matches_explicit_cursor`, `empty_journal_publishes_no_coverage_claim` |

**Прод-масштабные фикстуры** (по `testing.md` «прод-масштаб для sacred I/O»):
`red_gateway_bounded.rs` (пиковая память, контрольный `read_all` превышает бюджет),
`red_checkpoint_resource_bound.rs` / `red_frames_seek_bound.rs` / `red_checkpoint_prefix_pruned.rs`
(`big_journal` + много сегментов, счётчики `ReadStats`), `red_vwap.rs::vwap_i128_prod_scale`.

**Вывод для architect'а: сама СВЁРТКА покрыта плотно** (heatmap, VP, VWAP, CVD, окно, чекпоинт,
эпохи, курсоры — всё имеет sacred-оракул). **Не покрыто — граница «библиотека → провод»:**
содержимое реального WS-Snapshot'а, push-цикл кадров по WS, поведение на прод-раскладке журнала.
Ровно туда и целится задуманный milestone.

### verify-скрипт
`scripts/verify_M-28.sh` — 11 проверок: fmt, clippy, `cargo test -p gateway-serve`, grep-канарейки
GS-I-1 (нет `postgres|sqlx|diesel|tokio_postgres|mysql|mongodb` — строка 27), GS-I-3 (нет
`Journal::open|open_with|WriterConfig|.append(|.flush(` — строка 33), позитивная канарейка
`gateway::` (строка 38), recorder не зависит от gateway-serve (строка 45), сборка бина (строка 49),
наличие smoke-файла (строка 51).

---

## 7. Инварианты `GW-I-*` (и смежные `GS-I-*`)

Канонические формулировки — таблицы §Инварианты в milestone-файлах.

| ID | Смысл (одной строкой) | Определён | Проверяется |
|---|---|---|---|
| **GW-I-1** | Read-only: gateway не пишет журнал (grep-канарейка + байтовая идентичность каталога до/после) | `milestones/M-22-read-gateway.md:71` | `crates/gateway/tests/red_gateway_readonly.rs` |
| **GW-I-2** | Bounded-memory прод-масштаб для `snapshot` И `frames_since`: O(1) по размеру журнала; запрет `read_all`/`Vec<Event>` | `milestones/M-22-read-gateway.md:72` | `crates/gateway/tests/red_gateway_bounded.rs` + verify grep |
| **GW-I-3** | live == replay + детерминизм: `snapshot([start..C])` байт-идентичен свёртке `frames_since(START..C)`; `replay` ×N байт-идентичен | `milestones/M-22-read-gateway.md:73` | `crates/gateway/tests/red_gateway_live_eq_replay.rs` |
| **GW-I-4** | Snapshot-completeness mid-stream: нет дрейфа `snapshot+deltas` против полного пересчёта | `milestones/M-22-read-gateway.md:74` | `crates/gateway/tests/red_gateway_live_eq_replay.rs` |
| **GW-I-5** | export v2 аддитивен: `schema_version` присутствует, форма меняется только с bump | `milestones/M-22-read-gateway.md:75` | `crates/gateway/tests/red_gateway_export_v2.rs` |
| **GW-I-6** | Провенанс глубины: серия глубже 1.3% от mid обязана нести непустой `depth_band_provenance` | `milestones/M-22-read-gateway.md:76` | `crates/gateway/tests/red_gateway_export_v2.rs:128` |
| **GW-I-7** | `EpochFilter` соблюдён на ВСЕХ путях (snapshot/frames_since/replay) — эпохи не смешиваются молча | `milestones/M-22-read-gateway.md:77` | `crates/gateway/tests/red_gateway_epoch_filter.rs` |
| **GW-I-8** | Cursor/Frame-bounds: контигуальность кадров (`f[i].to == f[i+1].from`), `max_events` — реальный кап | `milestones/M-22-read-gateway.md:78` | `red_gateway_live_eq_replay.rs::{cursor_and_frame_bounds, frames_since_respects_max_events_cap}` |
| **GW-I-9** | Чекпоинт — кэш, не истина: (а) байт-идентичность `snapshot_from_checkpoint(K,at) ≡ snapshot(START,at)`; (б) любая невалидность → ТИХИЙ rebuild; (в) `advance` идемпотентен; (г) tamper-форсинг — чекпоинт реально читается | `milestones/M-38b-checkpoint-reducer.md:171` | `red_checkpoint_byte_identity.rs`, `red_checkpoint_is_cache.rs` |
| **GW-I-10** | `timeframe_ms > 0` И `86_400_000 % timeframe_ms == 0` — fail-closed на ВСЕХ публичных входах библиотеки + на СТАРТЕ `gateway-serve` | `milestones/M-47-timeframe-session-guard.md:88` | `crates/gateway/src/lib.rs:1707-1720` (`validate_selector`), `crates/gateway-serve/src/lib.rs:591-597`; тесты `red_timeframe_session_alignment.rs`, `red_timeframe_guard_startup.rs` |
| **GW-I-11** | Read-путь ограничен ХВОСТОМ: `snapshot_from_checkpoint` при K у хвоста декодирует ≤ хвостовых событий; измеряется `ReadStats`, НЕ аллокатором и НЕ wall-time (урок TD-040) | `milestones/M-38b-checkpoint-reducer.md:172` | `crates/gateway/tests/red_checkpoint_resource_bound.rs` |
| **GW-I-12** | Усечённая история ДЕКЛАРИРУЕТСЯ: оба пути эмитят `history_start_seq` + `history_truncated`, значения путей совпадают; отказ только при разрыве «чекпоинт↔журнал» | `milestones/M-48-checkpoint-bootstrap-and-ops.md:79` | `red_checkpoint_bootstrap_truncated.rs`; операторский путь `deploy/bin/gateway-checkpoint-cron.sh:39,114` |

Смежные транспортные инварианты (`crates/gateway-serve`):

| ID | Смысл | Определён | Проверяется |
|---|---|---|---|
| **GS-I-1** | Плоскости разделены: нет app-БД (`postgres`/`sqlx`/`diesel`/…) в market-транспорте | `milestones/M-28-gateway-serve.md:49` | `verify_M-28.sh:27` |
| **GS-I-2** | JWT-verify stateless и корректен (валидный→Ok, подделка/чужой ключ/expired→Err), сигнатура без БД-параметра | `milestones/M-28-gateway-serve.md:50` | `red_jwt_verify.rs` |
| **GS-I-3** | Read-only на уровне бина: нет journal-writer; приём WS-фрейма не мутирует журнал | `milestones/M-28-gateway-serve.md:51` | `verify_M-28.sh:33` |
| **GS-I-4** | Wire-roundtrip + версия: `ServeMsg::{Snapshot,Frame}` → JSON → parse → байт-идентичный объект, несёт `schema_version` | `milestones/M-28-gateway-serve.md:52` | `red_serve_passthrough.rs:118` |
| **GS-I-5** | Passthrough-fidelity: транспорт оборачивает РОВНО те `Frame`/`Snapshot`, что библиотека, без трансформации серий | `milestones/M-28-gateway-serve.md:53` | `red_serve_passthrough.rs` |

Замечание по нумерации: `GW-I-10` занят M-47, переиспользовать нельзя
(`milestones/M-38b-checkpoint-reducer.md:174`).

> **Обновлено 2026-08-24 (`R-128` §9 NOTE-1).** Строка «Следующий свободный — `GW-I-13`»
> была верна на дату сбора (02.08) и с тех пор протухла: M-69 занял **`GW-I-14`**, перескочив
> тринадцатый. Замер на `origin/main @ 25470c5`:
> ```
> $ grep -rhoE '\bGW-I-[0-9]+\b' crates/ | sort -uV | tr '\n' ' '
> GW-I-1 GW-I-2 GW-I-3 GW-I-4 GW-I-5 GW-I-6 GW-I-7 GW-I-8 GW-I-9 GW-I-10 GW-I-11 GW-I-12 GW-I-14
> ```
> То есть **`GW-I-13` — дыра, а не занятый инвариант**. Нарушением это не является:
> `gates.md` §12 требует УНИКАЛЬНОСТИ, а не непрерывности, и прямо говорит «разрывы не
> занимаются» — по этому правилу следующий берётся **`GW-I-15`**, а не тринадцатый.
> **Предел назван честно:** механизма у семейств инвариантов НЕТ — `next_artifact_id.sh`
> знает только классы `M`/`R`/`C`/`A` (замер: `grep -nE '^\s+(M|R|C|A)\)' scripts/next_artifact_id.sh`
> → четыре строки, `GW` отсутствует). Требование `COGNITIVE-ONLY`: держится на том, что автор
> снял замер, а не на барьере.

---

## 8. Готовый клиент / скрипт для ручного подключения

### НЕ НАЙДЕНО — ни одного.

Проверено:
- `examples/` есть у `research-cli`, `book`, `journal`, `contracts`, `ops` — у `gateway` и
  `gateway-serve` каталога `examples/` **нет**;
- `src/bin/` во всём workspace: `crates/research-cli/src/bin/latency_probe.rs`,
  `crates/gateway/src/bin/gateway-checkpoint.rs`, `crates/journal/src/bin/journal-retention.rs`,
  `crates/ops/src/bin/ops-watchdog.rs` — **WS-клиента среди них нет**;
- `[[bin]]` секции: `gateway-checkpoint`, `gateway-serve`, `research-cli` — только сервер;
- `grep -rln "websockets\|ws://" --include=*.py --include=*.js --include=*.sh --include=*.md .`
  → **пусто**. Ни python-, ни node-, ни shell-клиента в репозитории нет;
- `scripts/verify_M-28.sh` и `scripts/verify_M-48.sh` WS не дёргают (grep по
  `websocket|ws://|python3` → пусто).

§8-замеры латентности (`PROJECT-STATE.md:1894`, `TECH-DEBT.md:1742`) делались **ad-hoc клиентом,
который в репозиторий не коммитился**. Отсюда: команда «дёрнуть WS» в проекте отсутствует как
артефакт, и её воспроизведение — часть работы предстоящего milestone'а.

Ближайший по духу существующий бинарь — `crates/gateway/src/bin/gateway-checkpoint.rs` (350 строк):
он работает с той же библиотекой, но по файловому пути, не по WS.

---

## 9. Что нужно, чтобы подключиться к проду и получить первое сообщение

### Блокер №1 — сетевая достижимость (решить ПЕРВЫМ)

`gateway-serve` слушает `127.0.0.1:8080` **внутри своего контейнера**
(`docker-compose.yml:118`), и у сервиса **нет** `ports:` / `expose:` / `network_mode: host`
(проверено grep'ом по всему `docker-compose.yml` — совпадений ноль). Значит:

- ❌ `ssh -L 8080:127.0.0.1:8080 root@167.233.192.131` **не сработает**: на хосте VPS порт 8080
  не слушается никем — это loopback контейнера, а не хоста.
- Три рабочих варианта (выбор — за architect'ом, это уже дизайн):
  1. **Sidecar в том же netns** (ничего не меняет в проде, самый безопасный для read-path проверки):
     `docker run --rm --network container:hft-gateway-serve <образ-с-клиентом> <аргументы>` —
     клиент видит `127.0.0.1:8080` целевого контейнера.
  2. **`docker exec hft-gateway-serve <клиент>`** — если клиент удастся положить в образ
     (`image: hft-platform-recorder:local`, entrypoint `/usr/local/bin/gateway-serve`,
     `docker-compose.yml:112,145`).
  3. **Изменить прод-конфиг** (`GATEWAY_ADDR=0.0.0.0:8080` + `ports: ["127.0.0.1:8080:8080"]`),
     затем обычный ssh-туннель. Это правка `docker-compose.yml` ⇒ полноценный milestone-путь
     с деплой-гейтом §8, не разведочный шаг.

### Блокер №2 — секрет

`GATEWAY_JWT_SECRET` в репозитории **отсутствует** (единственные вхождения — подстановка
`${GATEWAY_JWT_SECRET:?}` в `docker-compose.yml:135` и чтение в
`crates/gateway-serve/src/lib.rs:554`). По `PROJECT-STATE.md:1505` секрет положен founder'ом
в **`.env` на VPS с правами 600**. Прочитать его можно только на VPS:
```
ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 \
  'grep GATEWAY_JWT_SECRET /root/hft-platform/.env'   # путь .env — уточнить, см. Открытые вопросы
```
Альтернатива без чтения секрета: `docker inspect hft-gateway-serve` покажет env контейнера.

### Последовательность

1. **Убедиться, что чекпоинт прогрет** — иначе первый Snapshot придёт через ~6 минут, а не за
   секунду (`TECH-DEBT.md:1742` vs `PROJECT-STATE.md:1894`):
   ```
   ssh ... 'ls -la /var/lib/docker/volumes/hft-platform_gateway-ckpt/_data'
   ```
   Ожидание: `ckpt-<hex>.bin` + `covered_through_seq` + `zz.lock`. Cron обновляет его ежедневно
   в 04:00 (`PROJECT-STATE.md:1898`: `0 4 * * * root .../gateway-checkpoint-cron.sh`).
   Если `ckpt-*.bin` нет — соединение уйдёт в fallback-реплей всей истории.

2. **Сгенерировать JWT.** HS256, ровно два claim'а. Минимальный питон:
   ```python
   import jwt, time                       # PyJWT
   tok = jwt.encode({"sub": "scout", "exp": int(time.time()) + 3600},
                    SECRET, algorithm="HS256")
   ```
   Требования, все из кода:
   - алгоритм **HS256** (`crates/gateway-serve/src/lib.rs:42`);
   - `sub: String` и `exp: usize` обязательны (`lib.rs:17-21`);
   - **`aud` добавлять НЕЛЬЗЯ** — при `Validation.aud = None` присутствие `aud` в токене даёт отказ
     (`lib.rs:39-41`);
   - `exp` в будущем; leeway 60 s (`lib.rs:36-38`).

3. **Подключиться и НЕ слать ничего.** URL: `ws://127.0.0.1:8080/?token=<jwt>`. Никакого
   subscribe-сообщения не существует (§2). Пример:
   ```python
   import websockets, json
   async with websockets.connect(f"ws://127.0.0.1:8080/?token={tok}",
                                 max_size=None,      # Snapshot крупный: heatmap ~1700 ячеек
                                 open_timeout=30, ping_interval=None) as ws:
       first = json.loads(await ws.recv())
   ```
   `max_size=None` существенно: дефолтный лимит 1 MiB у `websockets` может обрезать Snapshot.

4. **Интерпретировать первый фрейм.**
   - `{"Snapshot":{...}}` → успех. Ожидаемые поля: `schema_version == 8`
     (`crates/gateway/src/lib.rs:65`), `cursor.upto_seq`, `series.{ohlcv,heatmap,cob,...}`,
     `history_start_seq`, `history_truncated`.
   - `{"Error":"invalid token"|"expired token"|"missing token"|"missing token query"}` → отказ auth
     (`crates/gateway-serve/src/lib.rs:291,301,313,318`).
   - Соединение принято, но фреймов нет и через минуту → почти наверняка холодный чекпоинт
     (fallback-реплей, сотни секунд).

5. **Дальше — `Frame`.** Раз в 250 ms, ≤1 штука, ≤256 событий в кадре
   (`crates/gateway-serve/src/lib.rs:388-389`). На тихом рынке кадров может не быть вовсе
   (`crates/gateway/src/lib.rs:1775-1777` — пустой вектор при `consumed == 0`).
   Клиентский fold — `Snapshot::apply(&Frame)` (`crates/gateway/src/lib.rs:1294`).

### Сверка с реплеем журнала
Прямой библиотечный путь без WS (для сверки в оракуле):
`gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel, Cursor::LATEST)`
(`crates/gateway/src/lib.rs:1726`) — байт-идентичен тому, что уходит по WS, по GS-I-5
(`crates/gateway-serve/src/lib.rs:99-101`, тест `red_serve_passthrough.rs:91`). Селектор для прода:
`Venue::Binance`, `"BTCUSDT"`, `timeframe_ms=1000`, `bands=[0.001]`, `window_ms=Some(60000)`.

---

## 10. Открытые вопросы (чтением кода не выясняются)

1. **Точный путь `.env` на VPS и текущее значение `GATEWAY_JWT_SECRET`.** В репо нет ни файла,
   ни ссылки на путь (`PROJECT-STATE.md:1505` говорит только «на VPS `.env` 600»).
2. **Прогрет ли чекпоинт СЕЙЧАС.** Последний известный замер — M-48 (`PROJECT-STATE.md:1894`,
   прод `0215b34`). С тех пор было несколько merge'ей (M-51 `d896b98`, M-52 `b0723d4`). Если
   `GATEWAY_SCHEMA_VERSION` бампался после снятия чекпоинта, файл будет отвергнут — но, по фиксу
   TD-048/B2, **тихо**, с самолечением (`TECH-DEBT.md:1725-1731`), т.е. первое подключение
   заплатит полным реплеем и внешне это будет выглядеть как зависание.
3. **Реальный размер Snapshot в байтах на проде.** `heatmap len=1697` + `ohlcv len=61` известны
   (`PROJECT-STATE.md:1895`), но JSON-объём не замерялся — влияет на выбор `max_size` у клиента и
   на то, не упрётся ли фрейм в лимиты tungstenite на стороне сервера.
4. **Приходят ли на проде `L2Snapshot`-события вообще.** От этого зависит, будет ли
   `depth_series` непустым: полосы обновляются ТОЛЬКО в ветке `L2Snapshot`
   (`crates/gateway/src/lib.rs:892-919`), а `L2Delta` их не трогает (`lib.rs:928-929`).
   Выяснимо только реплеем прод-журнала или замером на живом WS.
5. **Сколько одновременных клиентов выдержит сервис.** `serve()` спавнит таск на соединение
   (`crates/gateway-serve/src/lib.rs:234`), каждый строит СВОЙ снапшот и ведёт СВОЙ push-цикл;
   шаринга состояния нет. `TECH-DEBT.md` (в разборе TD-044) отмечает «N одновременных клиентов =
   N параллельных полных реплеев» — для прогретого чекпоинта цифра не перемерялась.
6. **Живо ли `GATEWAY_ADDR`-переопределение на проде.** `docker-compose.yml:118` допускает
   `${GATEWAY_ADDR:-...}`; если в VPS-`.env` он переопределён на `0.0.0.0:8080`, картина
   достижимости из §9 меняется. Проверяется только `docker inspect` на VPS.
7. **Почему `main.rs` использует `#[tokio::main(flavor = "current_thread")]`**
   (`crates/gateway-serve/src/main.rs:17`) при наличии неиспользуемого `_runtime_hook` с
   multi-thread (`main.rs:59-67`) и при `rt-multi-thread` в features
   (`crates/gateway-serve/Cargo.toml:22`). Однопоточный рантайм означает, что построение снапшота
   (блокирующее чтение журнала) в одном соединении **блокирует все остальные**. Намеренно это
   или недосмотр — из кода не следует.
