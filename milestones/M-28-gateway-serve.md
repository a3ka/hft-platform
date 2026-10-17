# M-28 — gateway-serve — WS-транспорт кокпита (snapshot+push+replay, stateless JWT-auth)

STATUS: ✅ **CLOSED** 2026-07-29 — код смержен `40b8113`, §8 закрыт через M-48.
Держался дольше всех: §8 блокировался цепочкой TD-038 (торн-crc в legacy) → TD-039 (OOM) →
**TD-044 (латентность первого снапшота)**. Разблокирован **M-48** (merge `0215b34`, reviewer APPROVED, бухгалтерия `b85f1ce`): §8 закрыт ЗАМЕРОМ на VPS — E2E JWT→Snapshot **1.056 s** против 382.657 s на M-38b (baseline TD-044: 409.74 s), чекпоинт на проде создан впервые (`covered_through_seq=118434344`), провенанс подтверждён ДЕКОДОМ снапшота (`schema_version=8`, `history_start_seq=16049334`, `history_truncated=true`). **TD-044 и TD-048 — CLOSED.**

Исходный статус: PROPOSED (2026-07-23, architect). Пивот P-COCKPIT, транспорт market-плоскости (D1/D6).
**Новый крейт `crates/gateway-serve`** ⇒ **critic ОБЯЗАТЕЛЕН** (`gates.md` §1.4). **Supersedes**
`milestones/M-22-read-gateway.md` §Design «Транспортная оболочка» + task #6: транспорт вынесен из M-22
(там был опциональной оболочкой) в отдельный milestone со своим §8 (деплой на VPS).

## Objective

`gateway-serve` — **тонкая IO-оболочка над детерминированной библиотекой `crates/gateway`** (M-22): держит
WebSocket, тейлит журнал, отдаёт фронту `code2alpha` (1) снапшот при подключении, (2) инкрементальный push,
(3) replay. **Market-плоскость (D6):** read-only, **stateless по юзеру** — auth = ТОЛЬКО верификация
подписанного JWT (без user-БД). App-плоскость (Next.js+Postgres+Auth) — **вне нашего кода** (зона founder'а).

**В scope:** WS-сервер (tokio-tungstenite), stateless JWT-verify, snapshot+frames+replay через
`gateway::{snapshot,frames_since,replay}`, JSON-формат сообщений. **НЕ в scope:** app-БД/аккаунты (founder),
тяжёлый бинарный кодек heatmap (см. §Design — отложен до M-23, отдельное founder/frontend-решение),
rate-limiting/TLS-termination (инфра-слой деплоя).

## Design (пиновка для engine-dev + critic)

- **Крейт `crates/gateway-serve`** зависит от `crates/gateway` (библиотека). Детерминированное ядро НЕ тянет
  async/WS/JWT-deps — они живут ТОЛЬКО в транспорт-крейте. Bin `gateway-serve` + lib (auth/wire/adapter).
- **Auth (D6, stateless):** `auth::verify_token(token: &str, key: &DecodingKey) -> Result<Claims, AuthError>`
  (крейт `jsonwebtoken`). Проверяет подпись + `exp`. **НЕ ходит в user-БД** — Next.js подписал, мы только
  верифицируем (Claims: `sub`, `exp`). Алгоритм: HS256 (общий секрет) или Ed25519 (единообразие с `INTG-I`) —
  founder подтвердит; RED на HS256 как базовый.
- **Wire-формат (MVP — JSON):** `wire::ServeMsg` — версионированный конверт `{ Snapshot(Snapshot) |
  Frame(Frame) | Error(String) }`, `serde_json`. **JSON, НЕ postcard:** postcard — Rust-only формат, JS-фронт
  его не декодирует; MVP-транспорт универсально JS-декодируем. `schema_version` уже в Snapshot/Frame (GW-I-5).
- **Тяжёлый бинарный кодек (heatmap/depth) — ОТЛОЖЕН до M-23** и требует **JS-декодируемого** формата
  (MessagePack `rmp-serde` ↔ `@msgpack/msgpack`, ЛИБО typed-array framing для canvas) — **НЕ postcard**. Это
  контракт Rust↔JS ⇒ joint-решение с founder/frontend (D6 app-плоскость). Здесь НЕ фиксируем.
- **Serve-adapter (тонкий passthrough):** `serve::snapshot_msg(...)` и `serve::frames_msgs(...)` ОБЯЗАНЫ
  просто оборачивать `gateway::{snapshot,frames_since,replay}` в `ServeMsg` **без трансформации серий** —
  иначе ломается live==replay (GW-I-3). Bounded (GW-I-2) наследуется от библиотеки (frames_since батчами).
- **Read-only:** `gateway-serve` НЕ импортирует journal-writer/recorder-write и НЕ импортирует app-БД-клиент
  (postgres/sqlx/diesel) — VB-I-9. WS-приём фрейма от клиента = только replay-контролы (cursor/window), НЕ
  запись в журнал/БД.

## Инварианты (RED, sacred — architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **GS-I-1** (VB-I-9a) | **Плоскости разделены — нет app-БД в market-транспорте.** grep-канарейка: `crates/gateway-serve/**` и `crates/gateway/**` НЕ импортируют `postgres`/`sqlx`/`diesel`/`tokio-postgres`/`mysql`. User-данные — не в нашем коде. | `verify_M-28.sh` grep + source-scan |
| **GS-I-2** (VB-I-9b) | **JWT-verify stateless + корректен.** `verify_token(token, key)`: валидно-подписанный не-истёкший → `Ok(Claims)`; подделанная подпись → `Err`; истёкший (`exp` в прошлом) → `Err`; чужой ключ → `Err`. Сигнатура берёт ТОЛЬКО `(token, key)` — нет БД-параметра. Анти-плацебо: always-`Ok`-impl падает на tampered/expired. | `red_jwt_verify.rs` |
| **GS-I-3** | **Read-only (GW-I-1 на уровне бина).** `gateway-serve/src/**` не импортирует journal-writer (`Journal::append/open_with/flush`/`WriterConfig`); журнал не мутируется приёмом WS-сообщений. | `verify_M-28.sh` grep |
| **GS-I-4** | **Wire-roundtrip + версия.** `ServeMsg::{Snapshot,Frame}` → `serde_json` → parse → байт-идентичный объект; несёт `schema_version` (GW-I-5). | `red_wire_roundtrip.rs` |
| **GS-I-5** | **Passthrough-fidelity (live==replay через оболочку).** `serve::frames_msgs(...)` оборачивает РОВНО те `Frame`, что `gateway::frames_since(...)` (без трансформации серий); `snapshot_msg` — ровно `gateway::snapshot`. Анти-плацебо: любая перекодировка/фильтрация серий в транспорте → расхождение. | `red_serve_passthrough.rs` |

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-28-gateway-serve.md`, `crates/gateway-serve/tests/**` (GS-I-* RED),
  `crates/gateway-serve/src/lib.rs` — ТОЛЬКО контракт-типы (`ServeMsg`, `Claims`, `AuthError`) + сигнатуры
  (`verify_token`, `snapshot_msg`, `frames_msgs`) с `unimplemented!()`, `crates/gateway-serve/Cargo.toml`,
  запись в `members` root `Cargo.toml`, `scripts/verify_M-28.sh`.
- **engine-dev (impl) — BINDING carve-out:** `crates/gateway-serve/src/**` + `crates/gateway-serve/Cargo.toml`.
  Это НЕ выход за зону: `crates/gateway-serve` закреплён за engine-dev durable в `.claude/rules/scope-guard.md`
  (WS-транспорт M-28) — milestone и scope-guard согласованы (C-024 блокер #3). Тела auth (jsonwebtoken), wire,
  serve-adapter, `server::{bind,serve}` + bin `gateway-serve` (WS: accept→verify JWT→snapshot+frames+replay).
  Свои deps в `crates/gateway-serve/Cargo.toml`. **База:** worktree на `origin/feat/M-28-gateway-serve`.
- **Forbidden:** `crates/contracts` (T1), `crates/gateway/src` (ЧИТАЕТ как lib, НЕ правит), `crates/{risk,
  killswitch,oms,venue-*,journal,recorder}`, ЛЮБОЙ app-БД-клиент (postgres/sqlx/diesel — GS-I-1), journal-writer,
  order-path.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | GS-I-* RED + crate-скелет (ServeMsg/Claims/сигнатуры unimplemented + Cargo + members) + `verify_M-28.sh` | architect | compile-RED; достижимо; fmt-clean (RN-17) |
| 2 | ⏳ | `auth::verify_token` (jsonwebtoken HS256, stateless) | engine-dev | GS-I-2 GREEN |
| 3 | ⏳ | `wire::ServeMsg` (JSON) + serve-adapter `snapshot_msg`/`frames_msgs` (тонкий passthrough) | engine-dev | GS-I-4/GS-I-5 GREEN |
| 4 | ⏳ | `server::{bind,serve,local_addr}` (tokio-tungstenite: accept→verify JWT→snapshot+push+replay) + config-парсинг в `main.rs` | engine-dev | bin компилируется; **`smoke_ws.rs` GREEN** (валидный JWT→snapshot, невалидный→отказ) |

*(smoke `tests/smoke_ws.rs` — architect-provided RED, acceptance-поверхность task #4, C-024 блокер #2; НЕ детерм-оракул — IO/сеть, но обязателен. §8 деплой-гейт — решающий.)*

## Гейты

- **critic** (§9 Class A, ОБЯЗАТЕЛЕН — новый крейт §1.4) на milestone+RED+verify ДО dispatch.
- **risk-critic N/A:** read-only, MD-only, нет order-path/safety (auth-verify ≠ order-egress; MD-only carve-out §5).
  reviewer в Block-scope подтверждает read-only + отсутствие app-БД/order-path.
- **verify_M-28.sh** ОБЯЗАН: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`
  (RN-17) + канарейки GS-I-1/GS-I-3.
- **§8 (ПОЛНЫЙ, не lite):** `gateway-serve` — **новый сервис на VPS** (в отличие от read-only compute). При деплое:
  сервис healthy, читает журнал, отдаёт снапшот, **recorder НЕ задет** (отдельный процесс; gateway-serve только
  читает журнал). Это первый деплой-меняющий milestone кокпита — §8 eyes-on обязателен и решающий.

## Место в очереди

- **Транспорт-инфра:** разблокирует M-19 (фронт получает live-данные). Может идти параллельно M-23/M-25 (сервит
  текущий SeriesBundle; новые серии — аддитивно). Зависит от D6 (merged) как контрактной основы.
- **RN-18:** dev пушит GREEN на origin/feat/M-28 ПЕРЕД handoff (hard-precondition).

## Handoff (план при старте)

critic (аудит RED+контракт+плоскость-канарейка) → engine-dev (tasks 2-4) → tester (чистый прогон incl
`cargo fmt --all --check` + smoke; бутстрап ТОЛЬКО с origin/feat/M-28 — RN-18) → reviewer (merge + **§8 полный**:
деплой gateway-serve на VPS, eyes-on healthy + recorder не задет).
