# M-36 — gateway snapshot на проде: legacy purge + VWAP all-time

**Статус:** PROPOSED
**Разблокирует:** M-28 (§8 gateway-serve E2E на проде — сейчас NOT GREEN из-за TD-038)
**Тип:** bugfix (TD-038) + семантическая смена (VWAP) + прод-ops
**Ветка:** `feat/M-36-gateway-snapshot-prod`

## Objective

`gateway::snapshot` детерминированно падает `frame crc mismatch` на живом прод-журнале (TD-038).
Корневая причина **диагностирована** (не баг кода): ОДИН испорченный фрейм в замороженном 15GB
legacy-сегменте `segment-00000000.jrnl` (фрейм #713714, оффсет 203067719 ≈193.7 MiB; `len=9379`,
`stored_crc=0x0000000e` обрывок vs `calc_crc=0xc9b9e256`). Фреймы 0..713713 чисты — это порча
crc-поля (торн-райт эпохи pre-RFC02 recorder'а, инцидент TD-011). STRICT-семантика (DET-I-1)
корректно останавливается → падают ОБА ридера (`read_all` + `stream`). `gateway::snapshot`
(lib.rs:1067) реплеит ВЕСЬ журнал от `Cursor::START` по OwnCapture (включая legacy) → упирается
в битый фрейм.

**Founder-решения (зафиксированы):**
1. Legacy (15GB, история 2026-07-10..14) НЕ нужен → **физически удалить** (необратимо, подтверждено).
2. VWAP — **all-time от старта курсора** (не сессионный). Пересмотр VB-I-6/M-20.
3. VP/CVD — остаются session-anchored (00:00 UTC), не трогаются.
4. Чекпоинт-редьюсер (дешёвый snapshot без реплея истории) — **сначала замерить** latency
   post-purge, решение отдельным milestone по факту (вне M-36).

**Что делаем:** (A) убрать legacy с прода (ops) → снимает crc-блокер в корне; (B) сменить VWAP
на journal-cumulative (code); (C) замерить latency snapshot на ~9GB (gateway-serve зовёт полный
reduce НА КАЖДОЕ подключение — perf реален) и записать для решения по чекпоинту.

## Allowed paths

| Путь | Роль | Что |
|---|---|---|
| `crates/gateway/tests/red_vwap.rs` | architect | sacred-оракул: VW-I-3 инвертирован (all-time blend) ✅ сделано |
| `crates/journal/tests/red_seg0_removed.rs` | architect | guard: удаление нижнего сегмента терпимо ✅ сделано |
| `scripts/verify_M-36.sh` | architect | acceptance-гейт ✅ сделано |
| `docs/fa/viz-backend.md` | architect | VB-I-6 → per-series anchor policy ✅ сделано |
| `milestones/M-36-*.md` | architect | этот файл |
| `crates/gateway/src/lib.rs` | **engine-dev** | VwapAcc: снять session-reset; Snapshot::apply vwap-merge несёт суммы; `GATEWAY_SCHEMA_VERSION` 5→6; doc-комментарий `SeriesBundle.vwap` |
| прод VPS journal-dir | **reviewer/founder (§8)** | ops-purge legacy (см. §Ops) — НЕ в git |

## Forbidden paths

`crates/risk/**`, `crates/killswitch/**`, `crates/contracts/**` (T1 — VWAP-семантика живёт в
gateway, НЕ в T1, поэтому CT-RFC не нужен), `crates/venue-*/**`, любой order-путь. VP/CVD
аккумуляторы (`VolumeProfileAcc`, cumulative_delta) — НЕ трогать (остаются session).

## Tasks

| # | Задача | Тест-оракул | Роль | Статус |
|---|---|---|---|---|
| 1 | `VwapAcc`: убрать session-reset (строки ~249-254) → `sum_pv/sum_v` копятся all-time | `red_vwap::vwap_cumulative_across_midnight` (кросс-полночь=150, не 200) | engine-dev | ⏳ OPEN |
| 2 | `Snapshot::apply` vwap-merge: донести running-суммы, чтобы incremental == full reduce | `red_gateway_live_eq_replay` (build==merge байт-идентичность несёт vwap) | engine-dev | ⏳ OPEN |
| 3 | Bump `GATEWAY_SCHEMA_VERSION` 5→6 + обновить doc-комментарий `SeriesBundle.vwap` (session→all-time) | компилируется + suite | engine-dev | ⏳ OPEN |
| 4 | Guard: journal терпит удаление нижнего сегмента (ожидается GREEN; если FAIL — surface meta/continuity фикс ДО прода) | `red_seg0_removed` | engine-dev (verify) | ⏳ OPEN |
| 5 | Ops-purge legacy на VPS + re-probe + замер latency | §8 eyes-on (см. §Ops) | reviewer/founder | ⏳ OPEN |

**Анти-плацебо (task 1/2):** текущий M-20 impl с session-reset ДАЁТ 200 на кросс-полуночной сделке
→ `vwap_cumulative_across_midnight` (ожидает 150) ПАДАЕТ до фикса. Заглушка `vwap: Vec::new()`
падает на `.last()`. i64-аккумуляция переполняется на VW-I-2. Все три — реальное давление на инвариант.

## Contract impact

- **T1 (crates/contracts):** НЕ трогается. VWAP-семантика — gateway-внутренняя серия, не `Event`/`EventKind`.
  CT-RFC НЕ требуется.
- **GATEWAY_SCHEMA_VERSION 5→6:** форма поля `vwap: Vec<(i64,i64)>` неизменна, но СЕМАНТИКА (all-time
  vs session) меняется → bump сигналит будущему фронту (консюмеров ещё нет). Не T1, не CT-RFC.
- **VB-I-6 пересмотрен:** per-series anchor (VWAP=journal-cumulative; SVP/CVD=session). Задокументировано
  в `docs/fa/viz-backend.md`.

## §Ops — прод-purge legacy (task 5; reviewer/founder на §8, ПОСЛЕ merge code в main)

**НЕОБРАТИМО.** Порядок строгий. VPS: `ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes
root@167.233.192.131`. Журнал: `/var/lib/docker/volumes/hft-platform_journal-data/_data`.

1. **Baseline + место на диске.** `docker ps` (recorder healthy), `df -h`, зафиксировать текущий
   размер `segment-00000000.jrnl` (~15188347171 B) и наличие записи в `journal.legacy.json`.
2. **Backup манифеста (не сегмента — он одноразовый).** Скопировать `journal.legacy.json` в
   `/root/` на случай отката декларации.
3. **Снять декларацию legacy из манифеста.** Удалить/обнулить запись про `segment-00000000.jrnl`
   в `journal.legacy.json` (после удаления файла headerless-сегмента не будет — декларация станет
   висячей и НЕ должна ссылаться на несуществующий файл).
4. **Удалить сам сегмент.** `rm segment-00000000.jrnl`. seg0 заморожен (recorder пишет в активные
   89-93) → удаление безопасно на живом recorder'е. Recorder перезапуск НЕ нужен (он legacy не читает).
5. **Re-probe оставшегося журнала на ДРУГУЮ порчу.** Собрать `crates/journal/examples`-probe
   (паттерн: `read_all` + `stream(OwnCaptureOnly)` над всем каталогом), scp на VPS, прогнать
   read-only. Ожидание: OK по всем оставшимся сегментам (1-93). Если всплывёт ЕЩЁ битый фрейм в
   .zst — эскалация (компакция M-08 не должна была скопировать порчу; отдельный TD).
6. **Перезапустить gateway-serve** (сбросить возможный кэш open): `docker compose up -d gateway-serve`.
7. **Проверить snapshot Ok + ЗАМЕРИТЬ latency.** E2E (`/root/e2e_ws.py`: valid JWT → `ServeMsg::Snapshot`
   с `schema_version=6`). Замерить время построения одного snapshot на ~9GB (лог gateway-serve или
   обёртка time вокруг probe-варианта reduce). ЗАПИСАТЬ latency в close-out.
8. **Решение по чекпоинту.** latency приемлема (напр. <2-3с на подключение) → чекпоинт отдельным
   milestone позже. Неприемлема → чекпоинт-редьюсер становится обязательным (новый milestone,
   приоритет). Founder подписывает выбор.

**Откат:** удаление сегмента необратимо (backup файла не делаем — founder подтвердил ненужность).
Откат манифеста — из `/root/`-копии (шаг 2), но без файла это не восстановит данные, только
декларацию. Если после purge что-то сломалось на чтении (guard task 4 должен был это исключить) —
эскалация founder'у, НЕ импровизация.

## Acceptance

`bash scripts/verify_M-36.sh; echo exit=$?` → `VERDICT: PASS`, exit=0. Покрывает CODE-контракт
(fmt/build --workspace/clippy --all-targets + red_vwap all-time + live_eq_replay + red_seg0_removed
+ регрессия gateway/journal). Прод-purge (§Ops) + latency-замер — §8 eyes-on, пруф в close-out.

## Гейты

- **critic (plan-time):** ДА — меняется sacred-инвариант (VB-I-6/VW-I-3) + необратимая прод-ops.
  Асимметрия ошибки в runbook'е оправдывает plan-time аудит закоммиченного набора.
- **risk-critic:** НЕ требуется — read-path (gateway), нет order-egress (MD-only carve-out класс).
- **reviewer (PR-time):** UNCONDITIONAL. Проверяет: scope, Done Block, VWAP-оракул GREEN,
  `red_seg0_removed` GREEN, §8 eyes-on с latency-замером.
- **§8 деплой-гейт:** merge code → deploy → ops-purge (§Ops) → E2E snapshot schema_version=6 +
  latency. Milestone НЕ закрывается без зелёного прод-snapshot.

## Handoff-цепочка

architect (RED+milestone+verify+doc — закоммичено) → **critic** (аудит набора) → engine-dev
(tasks 1-3 impl + task 4 verify) → tester → reviewer (PR + §8 ops-purge + latency) → founder
(подпись решения по чекпоинту).

## Cross-references
- TD-038 диагноз (этот файл §Objective), `docs/fa/viz-backend.md` §5 (VB-I-6),
  `crates/gateway/src/lib.rs` (snapshot:1067, VwapAcc:240), `.claude/rules/gates.md` §8,
  `.claude/rules/testing.md` (анти-плацебо, деградированный вход).
