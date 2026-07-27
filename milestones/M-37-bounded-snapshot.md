# M-37 — bounded-memory snapshot (Путь А: убрать OOM)

**Статус:** PROPOSED
**Закрывает:** TD-039 (gateway snapshot OOM ~7.3GB → host-OOM на 7.5GB VPS)
**Разблокирует:** M-28 §8 E2E + M-36 close-out (сейчас снапшот не строится — падает по памяти)
**Не входит (следующий milestone M-38):** латентность. Путь А ограничивает ПАМЯТЬ; снапшот всё
ещё стримит историю от старта → per-connection O(история) (медленно, но без краша). Скорость —
чекпоинт-редьюсер (Путь Б), отдельный milestone.
**Ветка:** `feat/M-37-bounded-snapshot` (от origin/main dce5f3c)

## Objective

`gateway::snapshot` реплеит весь прод-журнал от `Cursor::START` на КАЖДОЕ WS-подключение и держит
per-bucket состояние для ВСЕХ time-бакетов истории → RSS растёт unbounded (~90 MB/s, монотонно) →
7.3GB → OOM (dmesg oom-killer, task=gateway-serve). Это НЕ порча (CRC/parse чисты после M-36
legacy-purge). Доминирующий драйвер — `heatmap_buckets` (полный снимок книги на каждый бакет).

**Фикс (Путь А, founder-выбор):** снимок хранит в памяти только НЕДАВНЕЕ окно `[at−W, at]`, старое
выбрасывает по ходу стрима; пара всевременных величин (VWAP) — дешёвым бегущим итогом. Память
становится O(окно), не O(история). Латентность не трогаем (Путь Б).

## Критик-выверенный дизайн (C-адверсариальный аудит, 2 раунда)

Состояние редьюсера делится на ДВА класса удержания:

- **(a) Сессионно-скалярное** (переживает эвикцию, база не теряется):
  - VWAP `sum_pv/sum_v` (i128, all-time — уже M-36, O(1));
  - **CVD running-база** (session): `cumulative_delta` — cumsum-от-нуля по бакетам (lib.rs:659-667);
    эвикция ранних бакетов СДВИНУЛА БЫ всю кривую. Нужен бегущий итог базы (session-scoped, reset
    00:00 UTC) — чинится как VWAP. **[Критик-находка #1: CVD, а не только VWAP, кумулятивен.]**
  - VP `vp.bins` (lib.rs:276-290): POC/Value Area считаются по ПОЛНОЙ сессионной гистограмме
    (`compute_vp_row` lib.rs:308). Bucket-эвикция порежет ТЕКУЩУЮ сессию → неверный POC. Эвиктить
    ТОЛЬКО целыми ПРОШЛЫМИ сессиями; текущую держать целиком. **[Критик-находка #2.]**

- **(b) Бакет-оконное** (эвикт бакетов `< at−W`, bucket-независимы → безопасно):
  - `heatmap_buckets` (lib.rs:413 — доминирующий драйвер), `ohlcv`, `bucket_delta`,
    `bubbles` (lib.rs:416), `depth[].values`, эмитируемые точки vwap/cvd.

Окно `W` привязано к КУРСОРУ `at`, не к wall-clock. Одно правило `[at−W, at]` применяется одинаково
в `full`, `snapshot(C)` и свёртке кадров — иначе ломается live==replay (VB-I-2).

**Снято подозрение (критик):** `book::OrderBook` НЕ unbounded (`apply_snapshot` чистит+переливает,
book/src/lib.rs:70-85 → O(глубина)). Дыры реконструкции книги НЕТ — venue-binance шлёт полный
L2Snapshot каждую 1с (`EMIT_PERIOD=1s`, venue-binance/src/lib.rs:35) → окно назад течёт ≤1с.

**🔴 Почему OOM дошёл до прода:** предохранитель `red_gateway_bounded` (tests, 212 строк) СЛЕП —
все события в ОДИН бакет (`:57-58`) → выход O(1), число бакетов не давится. Зелёный, но не проверял
рост по бакетам. Класс «идеальная фикстура» (testing.md). Переписать — обязательная задача. **[#5.]**

## Allowed paths

| Путь | Роль |
|---|---|
| `crates/gateway/tests/red_gateway_bounded.rs` (переписать — multi-bucket/multi-day + memory-budget) | architect |
| `crates/gateway/tests/red_gateway_window.rs` (новый — split-retention + windowed live==replay) | architect |
| `scripts/verify_M-37.sh`, `milestones/M-37-*.md` | architect |
| `crates/gateway/src/lib.rs` (Selector `window_ms`; эвикция бакетов; CVD running-база; VP session-эвикт) | **engine-dev** |
| `crates/gateway-serve/src/{main.rs,lib.rs}` (wiring `GATEWAY_WINDOW_MS`→Selector, task #7) | **engine-dev** |
| `crates/gateway-serve/tests/red_serve_window_wiring.rs` (RED wiring) | architect |
| `docker-compose.yml` (env `GATEWAY_WINDOW_MS` для gateway-serve, task #7c) | **engine-dev** |
| `docs/fa/viz-backend.md` (новый инвариант bounded-window) | architect |

## Forbidden paths

`crates/risk/**`, `crates/killswitch/**`, `crates/contracts/**` (T1 НЕ трогаем — `window_ms` живёт
в gateway-Selector, не в `Event`; CT-RFC не нужен), `crates/venue-*/**`, order-путь. Чекпоинт/
персистентность — вне M-37 (это Путь Б, M-38).

## Tasks

| # | Задача | Оракул | Роль | Статус |
|---|---|---|---|---|
| 1 | `Selector.window_ms: Option<i64>` (None=offline unbounded; Some(W)=live cockpit). Окно от `at` | компилируется + red_gateway_window | engine-dev | ⏳ |
| 2 | Reducer: эвикт бакет-оконного состояния (heatmap/ohlcv/bucket_delta/bubbles/depth.values) для бакетов `< at−W` | red_gateway_bounded (memory-budget) | engine-dev | ⏳ |
| 3 | CVD running-база (session) — эвикция не сдвигает кривую | red_gateway_window (CVD база) | engine-dev | ⏳ |
| 4 | VP эвикт ТОЛЬКО целыми прошлыми сессиями; текущая целиком | red_gateway_window (VP POC) | engine-dev | ⏳ |
| 5 | (RED) Переписать слепой `red_gateway_bounded`: multi-bucket + multi-day + counting-allocator memory-budget. Анти-плацебо: падает на unbounded-реализации | — | architect | ✅ DONE (compile-RED на `window_ms`) |
| 6 | (RED) `red_gateway_window`: CVD-база переживает эвикцию + VP whole-session + windowed live==replay | — | architect | ✅ DONE (compile-RED на `window_ms`) |
| 7a | (wiring, TD-020) Вынести сборку config из `main.rs` в тестируемую `serve_config_from_env(get: impl Fn(&str)->Option<String>) -> Result<ServeConfig, String>` (lib.rs); `main` — тонкий вызыватель `\|k\| std::env::var(k).ok()`. Читать `GATEWAY_WINDOW_MS` (Option<i64>, unset/пусто→None) | red_serve_window_wiring | engine-dev | ⏳ |
| 7b | `build_selector` + арг `window_ms: Option<i64>` → `Selector.window_ms` | red_serve_window_wiring | engine-dev | ⏳ |
| 7c | `docker-compose.yml`: gateway-serve env `GATEWAY_WINDOW_MS=60000` (иначе прод дефолтит в None=unbounded → §8 снова OOM) | §8 E2E | engine-dev | ⏳ |
| 7d | (RED) `red_serve_window_wiring`: env→`Selector.window_ms` + `build_selector` проброс | — | architect | ✅ DONE (compile-RED) |

**Анти-плацебо (задача 5 — критично):** оракул ОБЯЗАН содержать десятки-сотни бакетов на много
UTC-дней (не один бакет), чтобы давить рост per-bucket состояния. Против текущего кода — превышение
memory-бюджета (OOM-класс). Деградированный вход (testing.md): смесь сессий, односторонние апдейты
книги, много разных цен в bubbles/VP.

## Contract impact

- **T1 (crates/contracts):** НЕ трогается. `window_ms` — gateway-Selector, не `Event`. CT-RFC не нужен.
- **GATEWAY_SCHEMA_VERSION:** форма Snapshot неизменна (те же серии, просто окновые) — bump НЕ
  обязателен; engine-dev решает, нужен ли (если консюмер должен знать про окно). Обосновать в PR.
- Инвариант bounded-window документируется в `docs/fa/viz-backend.md`.

## Acceptance

`bash scripts/verify_M-37.sh; echo exit=$?` → `VERDICT: PASS`. Покрывает: fmt + build --workspace +
clippy --all-targets + red_gateway_bounded (memory-budget) + red_gateway_window +
red_serve_window_wiring (task #7) + регрессия gateway/journal.

**§8 E2E (reviewer, деплой-гейт) — ОБЯЗАТЕЛЬНО с активированным окном:** прод gateway-serve поднят с
`GATEWAY_WINDOW_MS=60000` (task 7c, docker-compose); валидный JWT → Snapshot СТРОИТСЯ (не OOM) +
`RssAnon` выходит на ПЛАТО (не рост ~90MB/s). Без установленного env прод дефолтит в `None`=unbounded
→ §8 снова OOM — это и был провал первого захода (reducer инертен в бинаре, TD-020). Пруф в close-out.

## Гейты
- **critic (plan-time):** ДА — новый инвариант bounded-memory + переписка sacred-оракула + ≥5 задач.
- **risk-critic:** НЕ требуется — read-path (gateway), нет order-egress.
- **reviewer (PR + §8):** UNCONDITIONAL. §8: снапшот строится, RSS bounded (замер RssAnon плато, не рост).

## Handoff-цепочка
architect (RED задачи 5-6 + milestone + verify + doc) → critic (plan-time) → engine-dev (задачи 1-4)
→ tester → reviewer (PR + §8 E2E: снапшот строится + RSS bounded) → founder (закрытие M-28/M-36/M-37).

## §Remediation (governance, 2026-07-26 — architect-вердикт)

Инцидент: impl задач #1-4 закоммичен сессией с ролью **reviewer** (worktree унаследовал `reviewer`
git-identity общего чекаута). Reviewer не может независимо заапрувить код, который написала его роль
(gates §4 — независимость про КТО ПИСАЛ, не про метку в логе). engine-dev вместо ремедиации переклеил
авторство `filter-branch`'ем (деревья идентичны — 0 строк кода) + force-push → это ОТМЫВАНИЕ метки,
нарушение не снято, аудит-трейл стал ложным.

**Вердикт architect: вариант (b) — переписать impl правильной ролью.** Ветка пересоздана от `1bd515a`
(чистый набор architect-RED + critic-вердикты + spec, impl отсутствует); переклеенные коммиты
818f50e/3437d03/838429f **отброшены** (подлинные держатся в /tmp worktree'ах — не удалять). engine-dev
реализует задачи 1-4 + 7 СВОИМИ руками против замороженных GREEN-оракулов под engine-dev identity →
авторство правдиво, reviewer снова независим (альтернативный гейт НЕ нужен). (a)/(c) отвергнуты:
оставляют reviewer-written impl на money-adjacent read-path + нормализуют scope-violation; re-impl —
один цикл против готовых оракулов, дешевле governance-долга. Дизайн НЕ переоткрывается (C-027 rev3 PASS).

**Architect-зона (закрыто этим коммитом):**
1. **Compat-правки 11 sacred-тестов** под новое поле `Selector.window_ms` (`window_ms: None` в существующих
   литералах) — это МОЯ зона, должны были приехать с задачами #5/#6; теперь authored architect'ом, не утекают dev'у.
2. **Линт `manual_is_multiple_of`** (`i % 2 == 0`) в red_gateway_bounded/readonly/serve_passthrough —
   переписан на `[Side::Buy, Side::Sell][(i % 2) as usize]` (лаконичнее и без линта) → `#![allow(...)]` в
   оракуле НЕ нужен (обоснование = устранение источника, а не подавление).
3. **Анти-плацебо сохранён:** compat/линт-правки не трогают memory-бюджет/эвикцию/assert'ы
   red_gateway_bounded/window → оракулы по-прежнему падают на unbounded/наивной эвикции (compile-RED до impl).

Рецидив-профилактика: `branch-hygiene.md` §7 (identity первым шагом в worktree). governance-TD — за reviewer.

## Cross-references
- Критик-аудит (2 раунда, находки #1-5), `crates/gateway/src/lib.rs` (Reducer:400, heatmap:413,
  CVD:659, VP:276), `crates/gateway/tests/red_gateway_bounded.rs` (слепой), `crates/journal/tests/
  red_open_bounded.rs` (паттерн memory-budget, TD-011), `.claude/rules/testing.md` (идеальная фикстура).
- **Путь Б (M-38, follow-up):** чекпоинт-редьюсер (bounded латентность) — перестраиваемый read-кэш в
  gateway, инвариант snapshot-from-checkpoint ≡ snapshot-from-START байт-идентичен (DET-I-1).
