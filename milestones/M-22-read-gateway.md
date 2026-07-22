# M-22 — Read Gateway (snapshot + live-push + replay) — enabling-инфра кокпита

STATUS: **PROPOSED — re-spin r2** (2026-07-22, architect; после critic C-022 REJECT). Пивот P-COCKPIT,
Трек B (MVP-1), критический путь. r2 закрывает C-022: B1 (GW-I-7 EpochFilter-оракул + канарейка по коду),
B2 (GW-I-8 cursor/frame-bounds), B3 (GW-I-2 расширен на frames_since; `max_events`-кап).
Doc-гейт §9 Class A: **новый крейт `crates/gateway`** ⇒ critic ОБЯЗАТЕЛЕН (`gates.md` §1.4) на
milestone+RED+verify ДО dispatch dev'а. Источники: `docs/07-cockpit-backend-roadmap.md` §5 (D1/D4),
`docs/fa/viz-backend.md` §3 (подсистема B) + §5 (VB-I-1..8), `research/exports/format.md` (export v1),
C-021 NOTE-2 (bounded streaming — зашит в acceptance ниже).

## Objective

Read Gateway — **read-only консюмер журнала** (Граница A), отдающий фронту `code2alpha` три вещи над
ОДНИМ кодом детерминированных редьюсеров:
1. **Snapshot при подключении** — полная детерминированная свёртка серий для `(venue, symbol)` окна
   `[start .. cursor]`.
2. **Инкрементальный live-push** — кадры (`Frame`) приращения серий за событиями после курсора; хвост
   журнала доопрашивается по `seq`-курсору (встроенного follow в `journal::stream` НЕТ — см. §Design).
3. **Replay** — детерминированный проигрыш окна (те же кадры, что live → **live == replay**, VB-I-2).

**MVP-1 scope (BINDING):** транспорт/стриминг-слой поверх УЖЕ СУЩЕСТВУЮЩИХ M-17 редьюсеров (OHLCV, CVD,
depth-series над `Trade`/`L2Snapshot`). **НЕ входит в M-22:** heatmap/L2Delta-реконструкция (M-23, требует
`Books::apply(L2Delta)` + TD-016 эвикцию), VWAP (M-20), Volume Profile (M-24), TPP-полосы (Трек C после
корректности книги), WS-сервер как продукт (см. §Design — транспортная оболочка тонкая, вне детерм-ядра).

**Почему сейчас:** это enabling-инфра — без gateway ни один виз-примитив не доезжает до фронта в live.
Крейт устанавливает контракт «snapshot + cursor-frames + replay», которому следуют M-23/24/25 (добавляют
серии в `SeriesBundle` аддитивно, export v2).

## Design (пиновка для dev + critic)

- **Прод-путь чтения — ТОЛЬКО `journal::stream(dir, EpochFilter)`** (bounded-memory, посегментный
  итератор). `journal::read_all()`/`Vec<Event>` в `crates/gateway/src/**` **ЗАПРЕЩЁН** (C-021 NOTE-2;
  `read_all` в самом журнале помечен «ТОЛЬКО для тестов/малых фикстур», `segments.rs:846`). Прецедент
  для копирования: `research-cli/src/data_quality.rs` и `grid.rs::run_grid_streamed` — «BUILT на
  `journal::stream`, без материализации, память O(1) на 8.3 GB».
- **EpochFilter обязан быть НАЗВАН** (CT-RFC02-2): gateway принимает `EpochFilter` явно (дефолт прод-пути —
  `OwnCaptureOnly`); вендор/синтетика не подмешиваются молча.
- **Live-механизм:** `journal::stream` перечисляет сегменты ОДИН раз и завершается на текущем хвосте
  (`finished=true`). Follow строится в gateway: `frames_since(after, max_events)` заново открывает stream и
  отдаёт события с `seq > after` (батч ≤ `max_events`), свёрнутые тем же редьюсером, что snapshot/replay;
  клиент пампит до сходимости курсора. **Пропуск к `after` — СТРИМОМ, без материализации истории** (GW-I-2:
  память O(1) по журналу, не по `after`). Отсюда **live == replay архитектурно** (один код, разный источник
  хвоста), а не проверка постфактум.
- **EpochFilter (GW-I-7):** gateway НИКОГДА не сворачивает эпохи, не прошедшие переданный `EpochFilter`
  (`OwnCaptureOnly` дефолт прод-пути). Vendor/Synthetic не подмешиваются в own-серии молча (CT-RFC02-2) —
  оракул проверяет на смешанном журнале, что own-свёртка ≠ all-свёртка.
- **Курсор** — `Cursor{after_seq: Option<u64>}` (`None` = с начала; `Some(s)` = события `seq > s`).
  Монотонный, сериализуемый — фронт хранит его между push'ами.
- **Транспортная оболочка (WS) — вне детерминированного ядра M-22.** Библиотека `crates/gateway`
  (детерминированная, sacred-тестируемая) отдаёт сериализуемые `Snapshot`/`Frame` (JSON; тяжёлые серии —
  `postcard` бинарные фреймы). Реальный WS-сервер — Fastify founder'а (D1) ИЛИ тонкий reference-бинарь
  `gateway-serve` (tokio-tungstenite) — **транспорт, не контракт**: smoke-тест, НЕ детерминизм-оракул.
  M-22 обязателен: библиотека + сериализация-контракт. Reference-WS-бинарь — опциональная task #6.

## Contract impact (T1) — НЕТ; export v2 — T-designate аддитивно

- **T1-ядро (`crates/contracts`, `Event`/`EventKind`) НЕ трогаем** — только читаем (Граница A). CT-RFC НЕ нужен.
- **`GATEWAY_SCHEMA_VERSION` / `Snapshot` / `Frame` / `Cursor` / `SeriesBundle`** — T-designate типы в
  `crates/gateway` (не в `crates/contracts`), версионируются аддитивно (VB-I-4; тот же класс, что
  `EXPORT_SCHEMA_VERSION`). Промоушен в `crates/contracts` — только при кросс-языковом консюмере (TD-008-паттерн);
  сейчас консюмер (code2alpha) читает JSON-форму, не Rust-тип → промоушен НЕ требуется.
- **`SemanticEvent` здесь НЕ вводится** (C-021 NOTE-1 — это M-26; имя `Event` за T1 закреплено).

## Инварианты (RED, sacred — architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **GW-I-1** (VB-I-3) | **Read-only.** Gateway НЕ пишет журнал: (а) grep-канарейка — `crates/gateway/src/**` не импортирует `Journal::append/open_with/flush`/writer-символы; (б) функционально — байты журнал-каталога до и после `snapshot`/`frames_since`/`replay` идентичны (sha). recorder НЕ зависит от gateway (обратной deps нет). | `red_gateway_readonly.rs` + verify grep |
| **GW-I-2** (VB-I-2/NOTE-2) | **Bounded-memory прод-масштаб — И snapshot, И frames_since.** ОБА пути над journal — память O(1) по размеру журнала: пик(64 MiB) − пик(16 MiB) < 1 MiB И абсолютный пик < бюджет на 64 MiB (ловит `read_all` И «пропуск к курсору через материализацию истории» в live-tail). Анти-плацебо: контрольный `read_all` того же журнала ПРЕВЫШАЕТ бюджет. Канарейка: нет `read_all`/`Vec<Event>` в `gateway/src`. | `red_gateway_bounded.rs` (snapshot+frames_since) + verify grep |
| **GW-I-3** (VB-I-2) | **live == replay + детерминизм.** `snapshot([start..C])` БАЙТ-идентичен свёртке `frames_since(START..C)` от пустого курсора (JSON/postcard-сериализация совпадает); `replay(окно)` ×N байт-идентичен. Деградированные входы (testing.md): АСИММЕТРИЯ (тик, где меняется только bid-сторона книги), МНОЖЕСТВЕННОСТЬ (2+ сделки в одном бакете/тике), ГРАНИЦА (окно через границу сегмента). | `red_gateway_live_eq_replay.rs` |
| **GW-I-4** | **Snapshot-completeness mid-stream.** Клиент, подключившийся на курсоре C (не с начала), получает snapshot == полной свёртке `[start..C]`; последующие `frames`, применённые к snapshot, на каждом следующем курсоре C' остаются идентичны свёртке-с-нуля `[start..C']` (нет дрейфа snapshot+deltas против полного пересчёта). | `red_gateway_live_eq_replay.rs` |
| **GW-I-5** (VB-I-4) | **export v2 аддитивен.** `Snapshot`/`Frame` несут `schema_version = GATEWAY_SCHEMA_VERSION`; сериализация форма-стабильна: v1-shaped потребитель (roundtrip-фикстура старой формы) не ломается; форма меняется ТОЛЬКО с bump версии. Новые серии (M-23+) добавляются как `Option`/новые поля, не переопределяют старые. | `red_gateway_export_v2.rs` |
| **GW-I-6** (VB-I-5) | **Провенанс глубины.** Любая depth-серия глубже 1.3% от mid несёт `depth_band_provenance` (непустой); отсутствие поля на такой серии → snapshot невалиден (честность измерителя). MVP-1 Binance BTCUSDT depth-series ≤0.5% — поле `None` допустимо; ≥3% полос в M-22 НЕТ (Трек C). Оракул фиксирует ИНВАРИАНТ формы (поле обязательно на deep-серии), готовя Трек C. | `red_gateway_export_v2.rs` |
| **GW-I-7** (CT-RFC02-2) | **EpochFilter соблюдён — эпохи не смешиваются молча.** На СМЕШАННОМ журнале (OwnCapture+Vendor+Synthetic сегменты) `snapshot(OwnCaptureOnly)` сворачивает ТОЛЬКО own-события и ОТЛИЧАЕТСЯ от `snapshot(All)`; `Explicit([own,vendor])` — своё третье значение. Анти-плацебо: impl, игнорирующий фильтр (хардкод `All`) → own==all → падение. Канарейка stream — по КОДУ (call-form, комментарии вырезаны), не по doc. | `red_gateway_epoch_filter.rs` + verify grep |
| **GW-I-8** | **Cursor/Frame-bounds контракт.** `snapshot(at).cursor` отражает свёрнутое окно (`LATEST`→`Some(last_seq)`, `START`→`START` + пустая серия); `frames_since(after, max)` возвращает НОВЫЙ курсор = последний свёрнутый `seq`; кадры контигуальны и в окне: первый `.from == after`, `f[i].to == f[i+1].from`, последний `.to ==` возвращённый курсор. Анти-плацебо: impl с неверным/игнорируемым курсором или дырами between-frame → падение. | `red_gateway_live_eq_replay.rs::cursor_and_frame_bounds` |

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-22-read-gateway.md`, `crates/gateway/tests/**` (GW-I-* RED),
  `crates/gateway/src/lib.rs` — ТОЛЬКО T-designate контракт-типы (`Snapshot/Frame/Cursor/SeriesBundle/
  Selector` + `GATEWAY_SCHEMA_VERSION`) и СИГНАТУРЫ `snapshot/frames_since/replay` с `unimplemented!()`
  телами (RED-bootstrap нового крейта — типовой контракт), `crates/gateway/Cargo.toml` (регистрация нового
  крейта — структурное решение, critic-гейт), запись `crates/gateway` в `members` root `Cargo.toml`,
  `scripts/verify_M-22.sh`.
- **engine-dev (impl, зона расширена этим milestone'ом):** `crates/gateway/src/**` — тела
  `snapshot/frames_since/replay` через `journal::stream` (bounded) + переиспользование M-17 редьюсеров
  (`research-cli` reducer-функции как зависимость); (опц. task #6) reference-WS-бинарь. МОЖЕТ добавлять
  СВОИ deps в `crates/gateway/Cargo.toml` (`[dependencies]`, shared-access правило scope-guard).
- **Forbidden (безусловно):** `crates/contracts` (T1), `crates/{risk,killswitch,oms,journal,recorder,venue-*}`
  (gateway их ЧИТАЕТ через public API, НЕ правит), `research-cli/src` (переиспользует как lib-зависимость, не
  меняет), `journal::read_all`/`Vec<Event>`-материализация в `gateway/src`, любой journal-writer/order-path.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | GW-I-* RED-набор + crate-скелет (контракт-типы + `unimplemented!()`-сигнатуры + Cargo + members) | architect | `cargo test -p gateway` КОМПИЛИРУЕТСЯ и ПАДАЕТ (unimplemented/assert); достижимо; GW-I-2 прод-масштаб обязателен |
| 2 | ⏳ | `verify_M-22.sh` (clippy -D warnings, test -p gateway, grep-канарейки read_all/writer-import) | architect | exit=0 только на GREEN |
| 3 | ⏳ | `snapshot(dir, EpochFilter, sel, at)` — полная свёртка серий через `journal::stream` (bounded), фильтр эпох соблюдён | engine-dev | GW-I-1/GW-I-2/GW-I-6/GW-I-7 GREEN |
| 4 | ⏳ | `frames_since(after, max_events)` + `replay(window)` + `Snapshot::apply` — инкрементальный/детерм. хвост, cursor-контракт | engine-dev | GW-I-3/GW-I-4/GW-I-8 GREEN |
| 5 | ⏳ | Сериализация `Snapshot`/`Frame` (JSON + postcard бинарь для тяжёлых серий) + `schema_version` | engine-dev | GW-I-5 GREEN; roundtrip |
| 6 | ⏳ | (опц.) reference-WS-бинарь `gateway-serve` (tokio-tungstenite) — smoke, НЕ детерм-оракул | engine-dev | подключение → snapshot+push; транспорт-smoke |

## Гейты

- **critic** (§9 Class A, ОБЯЗАТЕЛЕН — новый крейт §1.4) на milestone+RED+verify ДО dispatch.
- **risk-critic N/A:** gateway read-only, MD-only, нет order-path/safety-поверхности (MD-only carve-out
  `gates.md` §5). reviewer в Block-scope подтверждает read-only (нет journal-writer/order-egress).
- **reviewer** (PR-time UNCONDITIONAL) — scope, Done Block, read-only канарейки GREEN.
- **§8 деплой-гейт:** M-22 — библиотека + reference-бинарь; прод-recorder НЕ трогается (read-only, обратной
  deps нет → recorder-образ идентичен). Если reference-WS-бинарь деплоится на VPS — §8 применяется к НЕМУ
  (новый сервис healthy, читает журнал, не мешает recorder'у), НЕ к recorder'у.

## Место в очереди (зависимости)

- **Блокирует:** M-23/24/25 (виз-серии доезжают до фронта через gateway) и M-19 (фронт founder'а).
- **Не блокируется:** M-17 export (DONE) + M-18 L2Delta (DONE) достаточны; heatmap/L2Delta-путь НЕ нужен
  для MVP-1 (M-22 стримит Trade/L2Snapshot-редьюсеры). TD-016 (эвикция книги) — предусловие Трека C
  (TPP-полосы), НЕ M-22.
- **Параллельно:** M-20 (VWAP) / M-23 (heatmap-серии) — расчёт не зависит от gateway-транспорта; вливаются
  в `SeriesBundle` аддитивно после своих milestone'ов.

## Handoff (план при старте)

critic (аудит RED+контракт+bounded-канарейка) → engine-dev (tasks 3-5, impl через `journal::stream`) →
tester (чистый прогон + verify) → reviewer (scope+read-only канарейки → merge feat/M-22→main + §8 если
деплой reference-бинаря). Architect: GW-I-* RED + verify + crate-контракт-скелет.
