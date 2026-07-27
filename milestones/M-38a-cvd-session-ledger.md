# M-38a — CVD session-anchored ledger (TD-043)

**Статус:** PROPOSED (RED+doc+verify закоммичены architect'ом; C-028 K1/K2 исправлены; ждёт re-review critic → engine-dev)
**Закрывает:** TD-043 (CVD single-running через 00:00 UTC — M-37 §TD-042 «явно ОТЛОЖЕНО, завести
отдельный TD»; `Reducer::finish` lib.rs:836-842 несёт пометку «для multi-session нужен per-session
ledger … в M-37 не покрыт»).
**Разблокирует:** M-38b (checkpoint-reducer) — фиксирует модель сессии CVD ДО чекпоинта; иначе схема
чекпоинта замрёт на скалярной базе `cvd_session_base: i64` → выброшенные чекпоинты при миграции.
**Не входит (M-38b):** латентность / checkpoint / live-seek / stream_from. Здесь ТОЛЬКО семантика
CVD + форма v7.
**Ветка:** `feat/M-38a-cvd-session-ledger` (от origin/main cb28145)
**Founder-подпись:** 2026-07-27 — CVD сбрасывается на 00:00 UTC, per-session ledger зеркально VP.

## Objective

M-37 сделал CVD «сессионно-скалярным»: единая running-сумма `cumulative_delta` + одна скалярная
база `cvd_session_base: i64` — БЕЗ reset на 00:00 UTC. Это единственный индикатор, где заявленный
session-anchor (VB-I-6) НЕ реализован: `Reducer::finish` копит running через границу дня
(lib.rs:843-856), а `evict_window_state`/`merge_cvd_running` оперируют одной скалярной базой. На
окне, пересекающем полночь (штатно для суб-сессионного 60s-окна около 00:00 UTC), CVD «протекает»
из прошлой сессии в текущую → кривая неверна.

**Фикс (founder-подпись 2026-07-27):** CVD зеркалит Volume Profile — per-session ledger. Каждая
UTC-сессия (`utc_session_id`) — свой running с нуля; running обнуляется на 00:00 UTC. Прошлая
сессия эвиктится ЦЕЛИКОМ (тот же критерий `max_time_s < lo`, что VP); бакеты внутри текущей сессии
вне окна → фолдятся в base ЭТОЙ сессии. Смена отдаваемых значений ⇒ bump `GATEWAY_SCHEMA_VERSION`
**6→7**.

## Дизайн

**Состояние (`Reducer`):**
- `cvd: BTreeMap<session_id, CvdSession>`, где `CvdSession { base: i64, bucket_delta: BTreeMap<i64 time_s, i64> }`.
  Заменяет плоские `bucket_delta: BTreeMap<i64,i64>` + `cvd_session_base: i64`.
- **ОДНА** структура session-max-времён на CVD и VP — убрать дублирование `vp_session_max_time_s`
  (переименовать в общий `session_max_time_s: BTreeMap<session_id, i64>`, обновляется на КАЖДОЙ
  сделке, используется и VP-, и CVD-эвикцией).

**Эвикция (`evict_window_state`):**
- Бакеты внутри ТЕКУЩЕЙ (удержанной) сессии с `time_s < lo` → их delta фолдится в `base` ЭТОЙ
  сессии (скалярная арифметика баз — источник TD-042 — теперь ЛОКАЛЬНА для сессии, проще).
- Сессия целиком в прошлом (`session_max_time_s[sid] < lo`) → удаляется из `cvd` (base+bucket_delta),
  как VP whole-session. Окно через полночь → 2+ сессии живы одновременно (норма).

**Форма (`SeriesBundle`):**
- `cumulative_delta: Vec<(i64,i64)>` — running РЕСЕТИТСЯ на границе сессии (конкатенация per-session
  running-серий в порядке `session_id`; на первом бакете новой сессии running = base(sid) + δ).
- `cvd_session_base: i64` → `cvd_session_base: Vec<(session_id, base)>` (per-session, сорт по
  `session_id`; сессия без ненулевой базы может отсутствовать — трактуется как 0; байт-идентичность
  finish↔merge — оракул).
- `GATEWAY_SCHEMA_VERSION: 6 → 7` + doc-комментарий (аддитивность нарушена — семантика CVD и форма
  `cvd_session_base` меняются).

**Merge (`Snapshot::apply` / `merge_cvd_running` / `evict_series_bundle_under_window`):**
- Переписать PER-SESSION: группировать `cumulative_delta` по `session_of(time_s)`, дельты извлекать
  с «previous» = base(sid) каждой стороны, суммировать по (session, time_s), ре-деривить running от
  new_base(sid) = base_e(sid) + base_i(sid). Whole-session drop existing при `session_max < final_lo`.
- Инвариант GW-I-4/VB-I-2 сохранён: `snapshot(C) + frames_since(C..) ≡ snapshot(LATEST)` байт-идентично
  ПОД окном, пересекающим 00:00 UTC.

**Границы:** `session_of(time_s) = time_s.div_euclid(86_400)` эквивалентен `utc_session_id(ts_ms)`
(бакет-`time_s = ts_ms/1000`). T1 (`Event`) НЕ трогается — сессия выводится из `ts_exch_ms`, не из
журнала. gateway-serve прозрачен к v7 (JSON passthrough `schema_version`, потребитель-фронт вне репо).

## Allowed paths

| Путь | Роль |
|---|---|
| `crates/gateway/tests/red_gateway_cvd_session.rs` (новый — session-reset анти-плацебо) | architect |
| `crates/gateway/tests/red_gateway_schema_v7.rs` (новый — C-028 K1: константа/Snapshot/Frame ==7, runtime-RED) | architect |
| `crates/gateway/tests/red_gateway_window.rs` (обновить: cvd_base_survives + 2-session live + overlap-multistep у ГРАНИЦЫ 00:00 UTC с whole-drop pre/post-asserts — C-028 K2 + форма v7) | architect |
| `scripts/verify_M-38a.sh`, `milestones/M-38a-*.md` | architect |
| `docs/fa/viz-backend.md` (VB-I-6 CVD session-anchored + VB-I-10 per-session ledger) | architect |
| `crates/gateway/src/lib.rs` (`CvdSession`; `cvd` per-session; unified `session_max_time_s`; эвикция/merge per-session; форма `cvd_session_base: Vec<(i64,i64)>`; bump `GATEWAY_SCHEMA_VERSION`=7) | **engine-dev** |

## Forbidden paths

`crates/risk/**`, `crates/killswitch/**`, `crates/contracts/**` (T1 НЕ трогаем — session выводится из
`ts_exch_ms`, CT-RFC не нужен), `crates/venue-*/**`, order-путь, `crates/gateway-serve/**` (v7 прозрачен
через serde — правки не требуются; если понадобится — SCOPE VIOLATION REQUEST). Checkpoint/`stream_from`/
персистентность — M-38b.

## Tasks

| # | Задача | Оракул | Роль | Статус |
|---|---|---|---|---|
| 1 | (RED) `red_gateway_cvd_session.rs`: reset на 00:00 UTC (reset/асимметрия/множественность/3 сессии), runtime-анти-плацебо (падает на single-running) | — | architect | ✅ DONE (анти-плацебо: 4 FAIL — 13e8/58e8/7e8 vs session-local) |
| 2 | (RED) обновить `red_gateway_window.rs`: `cvd_base_survives` (per-session base v7), `cvd_two_sessions_live_across_midnight_window` (2 ledger живы), `windowed_live_eq_replay_overlap_multistep` (курсор У ГРАНИЦЫ 00:00 UTC → whole-drop S1 на bundle-merge, pre/post-asserts — C-028 K2) | — | architect | ✅ DONE (compile-RED формы v7: `&i64`→`&[(i64,i64)]`; K2 vantage у границы) |
| 3 | (RED) `docs/fa/viz-backend.md` VB-I-6/VB-I-10 + `verify_M-38a.sh` | — | architect | ✅ DONE |
| 4 | `CvdSession`+`cvd: BTreeMap<session_id, CvdSession>`; заменить плоские `bucket_delta`/`cvd_session_base:i64` | red_gateway_cvd_session + window | engine-dev | ✅ DONE |
| 5 | Unified `session_max_time_s` (убрать дубль `vp_session_max_time_s`), общий для VP+CVD эвикции | red_gateway_window (VP POC регрессия) | engine-dev | ✅ DONE |
| 6 | `evict_window_state` per-session: фолд внутрисессионного префикса в base(sid); whole-session drop | red_gateway_window (2-session) | engine-dev | ✅ DONE |
| 7 | `Reducer::finish` per-session running (reset на границе) + форма `cvd_session_base: Vec<(session_id,base)>` | red_gateway_cvd_session | engine-dev | ✅ DONE |
| 8 | `merge_cvd_running`/`evict_series_bundle_under_window` per-session (byte-identity live==replay через полночь) | red_gateway_window (overlap-multistep) | engine-dev | ✅ DONE |
| 9 | bump `GATEWAY_SCHEMA_VERSION` 6→7 + doc-комментарий (семантика CVD + форма cvd_session_base) | red_gateway_schema_v7 (константа/Snapshot/Frame ==7; C-028 K1); red_gateway_export_v2 (v1-аддитивность как регрессия) | engine-dev | ✅ DONE |

**Анти-плацебо (задачи 1-2, ПРОВЕРЕНО фактически):** `red_gateway_cvd_session` — 4 runtime-FAIL на
текущем single-running CVD (S2 несёт running S1 через 00:00: 13e8 vs 3e8, 58e8 vs −7e8, 7e8 vs −3e8,
7e8 vs −4e8). `red_gateway_window` — compile-RED формы v7 (`cvd_session_base: &i64` vs `&[(i64,i64)]`).
testing.md чек-лист: п.1 асимметрия (buy-heavy S1 / sell-only S2), п.2 множественность (2+ филла в
бакете границы), п.3 отсутствие (S2 не наследует S1), п.4 границы (переход 00:00 UTC), п.5 прод-масштаб
— N/A (CVD чистый compute, ресурс-оракул — M-38b), п.7 vantage (C-028 K2: overlap-курсор У ГРАНИЦЫ
d2+35s → snapshot(C) = хвост S1 + голова S2, финальное окно целиком в S2 → whole-drop S1 на пути
bundle-merge, ЯВНЫЕ pre/post-asserts вокруг multistep-fold'а).

**C-028 K1 (schema-оракул RED, ПРОВЕРЕНО фактически):** `red_gateway_schema_v7` — 3 runtime-FAIL на
текущем `GATEWAY_SCHEMA_VERSION=6` (константа 6!=7, `Snapshot.schema_version` 6!=7, `Frame.schema_version`
[6]!=7; frames non-empty). Тавтологичный `snap.schema_version==GATEWAY_SCHEMA_VERSION` из
`red_gateway_export_v2` больше НЕ named-гейт bump'а (остаётся как v1-аддитивность/провенанс регрессия).

## Contract impact

- **T1 (crates/contracts):** НЕ трогается. Session выводится из `ts_exch_ms`, не из `Event`. CT-RFC не нужен.
- **GATEWAY_SCHEMA_VERSION 6→7 (T-designate, не T1):** ОБЯЗАТЕЛЕН — меняется семантика `cumulative_delta`
  (session-reset) И форма `cvd_session_base` (скаляр→Vec). Не аддитивно → bump сигналит будущему фронту.
- gateway-serve: конверт несёт `schema_version` (GS-I-4) — фронт увидит 7. Правки serve не требуются.

## Acceptance

`bash scripts/verify_M-38a.sh; echo exit=$?` → `VERDICT: PASS`. Покрывает: fmt + build --workspace +
clippy --all-targets + red_gateway_cvd_session (reset) + red_gateway_window (форма v7/2-session/overlap
у границы) + red_gateway_schema_v7 (константа/Snapshot/Frame ==7, C-028 K1) + red_gateway_export_v2
(v1-аддитивность регрессия) + регрессия gateway (VP/VWAP/heatmap/epoch/live==replay) + journal
+ gateway-serve (v7 passthrough).

**§8 E2E (reviewer, деплой-гейт):** валидный JWT → Snapshot **v7** СТРОИТСЯ на прод-журнале VPS;
конверт несёт `schema_version=7`; CVD-серия обнуляется на 00:00 UTC (sanity свежих событий у границы
дня, если журнал её покрывает). Пруф в close-out.

## Гейты
- **critic (plan-time):** ДА — gateway-reducer + смена СЕМАНТИКИ схемы (v7, не аддитивно) + ≥5 задач.
- **risk-critic:** НЕ требуется — read-path (gateway), нет order-egress.
- **reviewer (PR + §8):** UNCONDITIONAL. §8: Snapshot v7 строится + schema_version=7 в конверте.

## Handoff-цепочка
architect (RED задачи 1-3 + milestone + verify + doc) → critic (plan-time) → engine-dev (задачи 4-9)
→ tester → reviewer (PR + §8 E2E: v7 строится) → founder (закрытие M-38a, разблокировка M-38b).
