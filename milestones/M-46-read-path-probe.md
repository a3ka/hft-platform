# M-46 — сквозная проверка read-path БЕЗ фронта: все серии, сверка с реплеем

**Статус:** СПЕКА ГОТОВА · **Дата:** 2026-08-02 · **Гейты:** critic (средняя модель) →
engine-dev → tester → reviewer → §8 деплой-гейт → sidecar-прогон против прода.
**RISK-BLOCK НЕ применяется:** read-only консюмер журнала, никакого order-egress и
journal-writer (`VB-I-3`, `GS-I-1`). **Граница C НЕ затрагивается:** состав записываемых
данных не меняется, env на VPS не выставляется, прод не конфигурируется.

**Приоритет founder'а (2026-08-02T22:40Z):** «первый работающий экран», проверяемый **без
фронта** — дизайнеры его ещё готовят. Founder отдельно попросил включить **все серии**,
не только heatmap.

---

## 0. Что уже известно ЗАМЕРОМ (постановка исправлена дважды)

Разведка контракта (`docs/plans/gateway-ws-contract.md`) опровергла две мои посылки —
записываю, чтобы milestone не строился на них:

1. **«Неизвестно, отдаёт ли сервис данные» — НЕВЕРНО.** Отдавал: eyes-on M-48
   (`PROJECT-STATE.md:1886-1899`) — первый `Snapshot` за **1.056 s**, `schema_version=8`,
   `ohlcv len=61`, `heatmap len=1697`, `history_truncated=True`. Кривая латентности
   `409.74 → 382.657 → 1.056 s`.
2. **`ws handshake failed` каждые 30 s — это healthcheck**, а не сломанный клиент:
   `timeout 2 bash -c '</dev/tcp/127.0.0.1/8080'` (`docker-compose.yml:146-153`) открывает
   TCP и рвёт ⇒ `accept_hdr_async` получает EOF (`gateway-serve/src/lib.rs:282`).

**Что при этом ДЕЙСТВИТЕЛЬНО не сделано никогда** — и составляет предмет milestone'а:

- **корректность серий не сверялась с реплеем ни разу.** Что серии отдаются — замерено;
  что они ВЕРНЫ — нет. Это прямое обещание продукта («каждая цифра выводится реплеем»);
- проверка была **однократной, руками, при закрытии M-48** — регрессию никто не поймает;
- единственный сквозной WS-тест `smoke_ws.rs` **структурно не может** проверить половину
  серий: его фикстура — 4 `Trade` и **ни одного `L2Snapshot`/`L2Delta`**
  (`smoke_ws.rs:43-55`) ⇒ `heatmap`, `cob`, `depth_series` в нём ВСЕГДА пусты;
- `verify_M-28.sh:51` проверяет лишь **факт существования** файла `smoke_ws.rs`
  (`[ -f ... ]`), не его содержимое — гейт формы, не поведения.

## 1. Objective

Доказать оракулами, что **весь `SeriesBundle`, отданный по WS реальным сервером, поэлементно
равен независимому реплею журнала**, и получить артефакт, который можно посмотреть глазами
без фронта.

## 2. Предмет: десять полей `SeriesBundle` (`gateway/src/lib.rs:247-304`)

| Поле | Семантика | Визуализируется |
|---|---|---|
| `ohlcv` | свечи per бакет | да (база панели) |
| `cumulative_delta` | CVD, **ресет на 00:00 UTC** (M-38a, TD-043) | да |
| `cvd_session_base` | `(session_id, base)` после эвикции | служебное |
| `depth_series` | глубина per `(side, band)` | да |
| `vwap` | all-time, **БЕЗ ресета** на 00:00 (M-36) | да (линия) |
| `volume_profile` | SVP per UTC-сессия, только торгованные цены (`VP-I-4`) | да (сбоку) |
| `vp_session_max_time_s` | зеркало критерия эвикции (TD-045) | служебное |
| `heatmap` | `HeatmapCell` | да (главная) |
| `cob` | книга на финальном курсоре | да |
| `volume_bubbles` | торгованный объём `(time_s, price) → (buy,sell)` (M-23) | да |

**Пара `cumulative_delta` / `vwap` — ключевая для оракула:** они ОБЯЗАНЫ вести себя
по-разному на границе UTC-суток (CVD сбрасывается, VWAP нет). Одинаковое поведение любой из
них = дефект. M-37 уже давал этот баг в обратную сторону (единая сумма через все дни).

## 3. §Tasks

| # | Задача | Зона | Статус | Оракул/гейт |
|---|---|---|---|---|
| 1 | WS-харнесс `wsprobe`: подключение с JWT, приём `Snapshot` + N `Frame`, дамп в JSON | engine-dev | ⏳ OPEN | O-1, T2 |
| 2 | Сверка с независимым реплеем: `gateway::snapshot(..)` того же журнала == WS-выдача поэлементно по ВСЕМ полям | engine-dev | ⏳ OPEN | **O-2** |
| 3 | Применение кадров: `snapshot(C) + frames_since(C..)` ≡ `snapshot(LATEST)` через WS-путь | engine-dev | ⏳ OPEN | O-3 |
| 4 | Рендер без дизайна: ASCII-панель в stdout + автономный HTML-файл (инлайн, без внешних ресурсов) | engine-dev | ⏳ OPEN | O-7, T5 |
| 5 | Sidecar-прогон против прода (`docker run --network container:hft-gateway-serve`), артефакты в отчёт | engine-dev | ⏳ OPEN | T6 |
| 6 | RED-оракулы O-1..O-7 | **architect** (sacred) | ⏳ OPEN | — |
| 7 | `scripts/verify_M-46.sh` | **architect** (sacred) | ⏳ OPEN | — |

## 4. RED-оракулы (architect, `crates/gateway-serve/tests/`)

Каждый обязан ПАДАТЬ на заглушке и содержать деградированный вход
(`.claude/rules/testing.md`, чек-лист «фикстура счастливого пути»).
**Общая фикстура — СМЕШАННАЯ:** `Trade` + `L2Snapshot` + `L2Delta`, минимум две UTC-сессии,
асимметричный дифф (обновляется одна сторона книги), мульти-филл в одном такте.
Это прямое закрытие дыры `smoke_ws.rs` (только `Trade` ⇒ полсерий пусты).

| # | Оракул | Свойство | Деградированный вход |
|---|---|---|---|
| **O-1** | `red_ws_snapshot_all_series_present` | на смешанной фикстуре НИ ОДНО из 10 полей не пусто без причины; пустое поле обязано иметь причину в фикстуре | фикстура только-`Trade` (как в `smoke_ws`) ⇒ оракул обязан ПАДАТЬ, доказывая, что он давит |
| **O-2** | `red_ws_series_equal_independent_replay` | **главный.** WS-выдача поэлементно == `gateway::snapshot` того же журнала, по КАЖДОМУ полю отдельным ассертом | серия, где отличается ровно один элемент в середине; пустая серия против непустой |
| **O-3** | `red_ws_frames_converge_to_latest` | `snapshot(C)` + применённые `Frame`ы ≡ `snapshot(LATEST)` | кадры разной длины, кадр без событий, граница окна `GATEWAY_WINDOW_MS` |
| **O-4** | `red_ws_auth_matrix_fail_closed` | отказ на: отсутствующий токен, пустой `?token=`, истёкший, чужой ключ, malformed. Ни в одном случае НЕ приходит `Snapshot` | все пять веток (сегодня по WS проверена ОДНА — wrong-key) |
| **O-5** | `red_ws_bounded_window_and_checkpoint` | `GATEWAY_WINDOW_MS` реально ограничивает окно по WS-пути; `checkpoint_dir=Some` реально потребляется (по `ReadStats`, не по grep) | `window_ms=None` (unbounded, TD-039) и невалидный чекпоинт ⇒ тихий rebuild, не паника |
| **O-6** | `red_ws_history_honesty` | `history_truncated`/`history_start_seq` НЕ врут: на усечённом журнале `true`, на полном `false` | заглушка «всегда true» падает на неусечённом, «всегда false» — на усечённом (анти-плацебо в обе стороны) |
| **O-7** | `red_session_boundary_cvd_resets_vwap_does_not` | **парный:** через 00:00 UTC `cumulative_delta` сбрасывается, `vwap` — НЕТ | журнал, пересекающий полночь; реализация, ресетящая обе серии, обязана краснеть |

## 5. Allowed paths

**Разрешено:**
- `crates/gateway-serve/src/**` (харнесс, рендер, сверка) — engine-dev;
- `crates/gateway-serve/tests/**` — **architect only** (sacred);
- `scripts/verify_M-46.sh` — **architect only**;
- `research/reports/M-46-*.md` + артефакты прогона — dev/tester;
- `[dependencies]` крейта `gateway-serve` — engine-dev (свои, не удаляя чужих).

**Запрещено безусловно:** `crates/contracts/**` (T1) · `crates/risk/**`, `crates/killswitch/**` ·
`crates/journal/src/**` (харнесс — консюмер, не писатель) · любая правка прод-конфига
(`docker-compose.yml`, env на VPS) · `crates/gateway/src/**` сверх чтения — **изменение
семантики серий в этом milestone'е ЗАПРЕЩЕНО**: если сверка покажет расхождение, это
находка, а не повод править редьюсер (`gates.md` §4, граница reviewer↔architect: dev
ОПИСЫВАЕТ, architect проектирует фикс).

## 6. Acceptance — `scripts/verify_M-46.sh`

Обязателен паритет с CI-job `fmt + clippy + test` (`gates.md` §3, урок M-45 T2b):

1. `cargo fmt --all -- --check` · `cargo clippy --workspace --all-targets -- -D warnings` ·
   `cargo build --workspace`;
2. `cargo test -p gateway-serve -p gateway` — O-1..O-7 GREEN;
3. **T3 (главный):** O-2 (сверка с реплеем) зелён — и проверяется ИСПОЛНЯЕМЫМ тестом, не грепом;
4. **T4 анти-плацебо:** O-1 обязан ПАДАТЬ на только-`Trade`-фикстуре — гейт проверяет сам
   факт наличия этой негативной проверки внутри оракула;
5. T5: рендер порождает непустой HTML и ASCII на фикстуре (артефакт существует и содержит
   данные, а не только разметку);
6. T6: `wsprobe` собирается как бинарь и печатает usage (готов к sidecar-прогону);
7. Финальная строка `VERDICT: PASS|FAIL`, exit-код соответствует.

## 7. Прогон против прода (задача 5) — sidecar, БЕЗ правки прода

`gateway-serve` слушает loopback **внутри контейнера**; `ports`/`expose`/`network_mode` у
сервиса нет (проверено грепом по `docker-compose.yml`) ⇒ **ssh-туннель на хостовый
`127.0.0.1:8080` не подключится никуда**.

Способ: `docker run --rm --network container:hft-gateway-serve <образ> wsprobe ...` —
клиент попадает в сетевой namespace целевого контейнера и видит его loopback.
Прод при этом **не меняется**: ни портов, ни env, ни рестарта.

Перед прогоном убедиться, что чекпоинт прогрет (иначе первый `Snapshot` придёт через минуты,
а не за секунду): `ls /var/lib/docker/volumes/hft-platform_gateway-ckpt/_data` ⇒ `ckpt-*.bin`.

**В close-out:** сырой дамп первого `Snapshot` (усечённо), латентность до него, длины всех
десяти серий, путь к HTML-артефакту.

## 8. Граница C

**Не затрагивается.** Milestone read-only: не меняет состав записываемых данных, не трогает
env на VPS, не двигает промоушены/веса/лимиты. Подпись founder'а НЕ требуется.

## 9. Cross-references

- `docs/plans/gateway-ws-contract.md` — фактура контракта (разведка 2026-08-02)
- `docs/09-roadmap-v2.md` §Ф2/§Ф3 — куда это ложится (экран ≠ Ф2; см. `ORCHESTRATION-STATE`)
- `.claude/rules/testing.md` — деградированный вход, анти-плацебо
- `.claude/rules/gates.md` §3 — паритет verify с CI
