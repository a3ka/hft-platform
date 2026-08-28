# M-47 — GW-I-10: fail-closed гвард выравнивания `timeframe_ms` (TD-046)

- **Статус:** ✅ **CLOSED** 2026-07-28 — смержен в `main` как `47577c0` (`--no-ff`), reviewer
  APPROVED, бухгалтерия `bba0656`. Цепочка: architect (спека+RED+verify `f90f170`/`c5d0c64`) →
  engine-dev (impl, `4215d79` close-out) → tester → reviewer (PR-гейт + merge + §8).
  `verify_M-47.sh` — VERDICT: PASS, exit 0 (12/12) на ветке И на merged-дереве;
  `cargo test --workspace` — 421 passed / 0 failed. **§8 деплой-гейт GREEN:** VPS на `47577c0`,
  `hft-recorder` + `hft-gateway-serve` healthy (restarts=0), E2E JWT→Snapshot v7;
  гвард проверен на ПРОД-ОБРАЗЕ (`GATEWAY_TIMEFRAME_MS=11000` и `604800000` → exit_code=2,
  `1000` принят) — не grep по коду, а поведение артефакта. **TD-046 и TD-047 — CLOSED.**
- **Автор спеки:** architect, 2026-07-28
- **Базовый HEAD:** `origin/main @ b7ac2f8`
- **Ветка:** `feat/M-47` (RED-коммит `f90f170`, спека `c5d0c64`). Отпочкована от общей ветки
  ДО артефактов M-38b: compile-RED чекпоинта иначе навсегда красил бы гейт M-47, а M-47 обязан
  был мержиться первым и независимо (см. §Порядок мержа).
- **Тех-долг:** TD-046 (severity NOTE → повышена до **MINOR**, см. §Objective)
- **Зона:** read-path (`crates/gateway`, `crates/gateway-serve`). MD-only, ордер-пути нет.

## Objective

Сделать невозможным конфиг, при котором session-anchored серии (CVD — M-38a/TD-043, SVP — M-24)
считаются по бакету, накрывающему 00:00 UTC. Такой бакет принадлежит ДВУМ UTC-сессиям, и
корректного `session_id` для него не существует — это не «неудобный конфиг», а неопределённая
семантика. Правильный ответ — **отказ**, а не правдоподобное значение (fail-closed, `CLAUDE.md`).

**Severity повышена относительно исходной записи TD-046 (NOTE → MINOR) по замеру.** TD-046
описывал только случай невыравненного timeframe. Прогон RED-оракула вскрыл ВТОРОЙ, худший
режим, в долге не описанный: при `timeframe_ms <= 0` `Reducer::bucket_time_s`
(`crates/gateway/src/lib.rs:671-677`) возвращает `None`, поэтому `ohlcv` / `cumulative_delta` /
`vwap` / `heatmap` / `volume_bubbles` выходят **ПУСТЫМИ**, а `volume_profile` — **ЗАПОЛНЕННЫМ**
(VP якорится от `utc_session_id(ts_ms)`, минуя бакет). Кокпит получил бы `Ok` без единой ошибки:
пустой график и живой профиль объёма. Паники нет — тихая полу-правда, класс «код на main ≠
функция в проде». Прод (`GATEWAY_TIMEFRAME_MS=1000`) не затронут; долг латентный, но футган
заряжен и смотрит в сторону оператора.

**Почему сейчас, а не «когда-нибудь».** M-38b замораживает форму состояния `Reducer` в чекпоинте
и ключует его `selector_fingerprint`. Чекпоинт, снятый под невалидным селектором, — мусор,
который выглядит валидным по CRC. Гвард обязан существовать ДО того, как селектор станет частью
ключа кэша.

## Дизайн

### Решение: гвард, а не вывод сессии из `ts_exch_ms`

TD-046 называл две опции. Выбрана первая. Обоснование (архитектурное, не «дешевле»):

- **Вывод сессии бакета из `ts_exch_ms`** требует, чтобы КЛЮЧ бакета на проводе нёс сессию:
  `cumulative_delta: Vec<(time_s, value)>` физически не различает две сессии с равным `time_s`
  (ровно этот симптом в замере reviewer'а). Значит — non-additive смена формы **v7→v8** +
  переработка merge-пути (`Snapshot::apply`, `evict_series_bundle_under_window`) ровно в тот
  момент, когда M-38b фиксирует форму состояния в чекпоинте. Цена высокая, а покупается ею
  поддержка конфигов, для которых session-anchored серия семантически не определена.
- **Fail-closed гвард** — честный ответ на неподдерживаемый вход. Прод-дефолт (`1000`) и все
  практические таймфреймы (1с/1м/5м/15м/1ч/1д) делят сутки нацело; отвергаются ровно те
  конфиги, для которых серия всё равно была бы ложью.

### Где живёт гвард (анти-байпас — ключевое требование)

Проверка ТОЛЬКО в `serve_config_from_env` оставила бы байпас-поверхность: `Selector` — публичная
структура с публичными полями, её собирает напрямую любой консюмер библиотеки (чекпоинтер M-38b,
shared-tailer M-39, research-cli, тесты). Поэтому:

1. **`crates/gateway` — модель владеет своим предусловием.** Именованная функция
   `validate_selector(sel: &Selector) -> io::Result<()>` (имя зафиксировано milestone'ом —
   на него завязана verify-канарейка), вызывается в НАЧАЛЕ каждого публичного входа:
   `snapshot`, `frames_since`, `replay`. Их тип уже `io::Result<_>` — смена сигнатур не нужна.
2. **`crates/gateway-serve` — отказ на СТАРТЕ.** `serve_config_from_env` возвращает `Err(String)`,
   сообщение называет `GATEWAY_TIMEFRAME_MS`. Иначе оператор с опечаткой поднимет контейнер,
   здоровый по healthcheck, отдающий ошибку каждому клиенту: §8 eyes-on увидит `(healthy)`,
   а кокпит будет пуст (класс TD-019/TD-020 «механизм есть, никто не зовёт», зеркально
   `red_serve_window_wiring`).

### Критерий

```text
валиден ⟺ timeframe_ms > 0 && 86_400_000 % timeframe_ms == 0
```

Проверять надо **делимость суток**, а не «круглость»: недельный бакет (`604_800_000`) круглый,
но накрывает 7 полуночей — отвергается. Форма отказа в библиотеке: `io::ErrorKind::InvalidInput`,
сообщение содержит подстроку `timeframe_ms`. Это не косметика — оракул это ассертит, чтобы
оператор понял, что чинить, не читая исходники.

### Инвариант

| ID | Инвариант |
|---|---|
| **GW-I-10** | **`Selector.timeframe_ms` обязан делить `86_400_000` нацело и быть > 0.** Иначе бакет пересекает 00:00 UTC и `session_id` бакета не определён ⇒ session-anchored серии (CVD/SVP) недостоверны. Отказ fail-closed (`InvalidInput`) на ВСЕХ публичных входах библиотеки (`snapshot`/`frames_since`/`replay`) — не только в конфиге транспорта, иначе байпас через прямую сборку `Selector`. `gateway-serve` дополнительно отказывает на СТАРТЕ. Анти-плацебо парный: выравненные значения (`1`, `1000`, `60_000`, `3_600_000`, `86_400_000`) обязаны ПРИНИМАТЬСЯ — заглушка «всегда Err» падает |

GW-I-9 **зарезервирован** за byte-identity чекпоинта (M-38b, `milestones/M-38-roadmap.md:59`) —
не переиспользовать.

## Allowed paths

- `crates/gateway/src/lib.rs` — `validate_selector` + вызовы в `snapshot`/`frames_since`/`replay` (engine-dev)
- `crates/gateway-serve/src/lib.rs` — отказ в `serve_config_from_env` (engine-dev)
- `milestones/M-47-timeframe-session-guard.md` — колонка Status в §Tasks (engine-dev, carve-out)

## Forbidden paths

- `crates/gateway/tests/**`, `crates/gateway-serve/tests/**` — sacred RED (architect-only).
  Тест кажется неправильным → `!!! SCOPE VIOLATION REQUEST !!!`, не правка.
- `scripts/verify_M-47.sh` — acceptance-гейт (architect-only).
- `crates/contracts/**` — T1 не трогается (форма провода НЕ меняется; `GATEWAY_SCHEMA_VERSION`
  остаётся **7** — гвард отвергает вход, а не меняет выход).
- `docker-compose.yml` — дефолт `GATEWAY_TIMEFRAME_MS=1000` уже валиден, править нечего.
- Любая семантика бакетирования/сессий (`bucket_time_s`, `utc_session_id`) — гвард ДОБАВЛЯЕТСЯ
  перед входом, поведение принятых конфигов обязано остаться байт-идентичным.

## Tasks

| # | Status | Задача | Агент | Acceptance |
|---|---|---|---|---|
| 1 | ✅ DONE | `validate_selector(&Selector) -> io::Result<()>` в `crates/gateway/src/lib.rs`: `timeframe_ms > 0 && 86_400_000 % timeframe_ms == 0`, иначе `InvalidInput` с `timeframe_ms` в сообщении. Вызвать ПЕРВОЙ строкой в `snapshot`, `frames_since`, `replay` | engine-dev | `cargo test -p gateway --test red_timeframe_session_alignment` — 8 passed |
| 2 | ✅ DONE | `serve_config_from_env`: после парса `GATEWAY_TIMEFRAME_MS` — тот же критерий, `Err(String)` с именем переменной. Дефолт `1000` и все выравненные значения обязаны стартовать | engine-dev | `cargo test -p gateway-serve --test red_timeframe_guard_startup` — 6 passed |
| 3 | ✅ DONE | Прогон всего гейта: `bash scripts/verify_M-47.sh` → `VERDICT: PASS`, exit 0 (включая канарейку ≥4 упоминаний `validate_selector` и регрессию M-38a) | engine-dev | verify exit=0, Done Block сырым выводом |

Оценка: **2-3 атомарных коммита** (< 5) — plan-time critic по §3 `gates.md` не триггерится
(контракты не тронуты, крейт не вводится, форма T1/схемы не меняется). Reviewer — UNCONDITIONAL.

## Contract impact

**Нет.** `GATEWAY_SCHEMA_VERSION` остаётся `7`. Форма `Snapshot`/`Frame`/`SeriesBundle` не
меняется. Меняется только МНОЖЕСТВО принимаемых входов (сужается до корректных) — консюмеры
провода не затронуты. CT-RFC не требуется.

## Acceptance

`bash scripts/verify_M-47.sh; echo "exit=$?"` → `VERDICT: PASS`, `exit=0`.

Гейт содержит (≥1 проверка на задачу): fmt/build/clippy; оба RED-набора; анти-байпас канарейку
(`validate_selector` встречается ≥4 раз в `crates/gateway/src/lib.rs` по КОДУ с вырезанными
комментариями = определение + 3 входа — ловит «реализовал, но подключил не везде»); канарейку
валидности прод-дефолта в `docker-compose.yml`; регрессию M-38a (`red_gateway_cvd_session`,
`red_gateway_window`, `red_gateway_schema_v7`) + полный read-path suite.

### Зафиксированный RED (архитектор прогнал ФАКТИЧЕСКИ, `f90f170`)

```text
$ cargo test -p gateway --test red_timeframe_session_alignment
test result: FAILED. 2 passed; 6 failed
  misaligned_timeframe_rejected_by_{snapshot,frames_since,replay}   FAILED
  zero_timeframe_rejected_not_panic / negative_timeframe_rejected   FAILED
  weekly_timeframe_longer_than_day_rejected                         FAILED
  aligned_timeframes_accepted / aligned_timeframe_keeps_sessions_separate   ok  ← парный vantage

$ cargo test -p gateway-serve --test red_timeframe_guard_startup
test result: FAILED. 2 passed; 4 failed

# репро reviewer'а воспроизведено буквально (timeframe_ms=11_000):
cumulative_delta: [(1752105597, 500000000), (1752105597, -300000000)]
```

**Анти-плацебо в обе стороны:** заглушка-no-op валит 6+4 отказных теста; заглушка «всегда Err»
валит `aligned_*` (парный vantage, testing.md п.7). Пройти можно только реальным критерием.

## Порядок мержа (важно)

M-47 и M-38b трогают один файл (`crates/gateway/src/lib.rs`). **M-47 мержится ПЕРВЫМ** — он
крошечный, а M-38b строит `selector_fingerprint` поверх уже валидированного селектора. Если
M-38b уходит в работу параллельно, engine-dev M-38b ребейзится на M-47 после его merge.

**✅ ВЫПОЛНЕНО.** M-47 в `main` (`47577c0`); `feat/M-38b` ребейзнут на актуальный `main`
architect'ом. Следствие для M-38b: чекпоинт под невыравненным `timeframe_ms` теперь снять
НЕВОЗМОЖНО в принципе — `validate_selector` отвергает такой селектор на всех входах, поэтому
`selector_fingerprint` ключует только валидированную конфигурацию.

## Гейты

- plan-time critic — **не требуется** (§3 `gates.md`: < 5 коммитов, контракты не тронуты,
  нового крейта нет, форма схемы не меняется). Founder вправе назначить критика по желанию.
- risk-critic — **не требуется**: read-path, MD-only, ордер-egress отсутствует
  (`gates.md` §5 MD-only carve-out).
- reviewer — **UNCONDITIONAL** + §8 post-merge деплой-гейт (прод стартует с дефолтом `1000`,
  контейнеры healthy, E2E JWT→Snapshot строится).

## Handoff-цепочка

`architect` (спека+RED+verify — ✅ сделано) → `engine-dev` (задачи 1-3) → `tester` (чистый
прогон) → `reviewer` (PR-гейт + merge + §8 + TECH-DEBT: TD-046 → CLOSED).

## Cross-references

- `TECH-DEBT.md` TD-046 (исходное описание дефекта reviewer'ом), TD-047 (закрыт коммитом `6fc6350`)
- `docs/fa/viz-backend.md` VB-I-6 (session-anchor policy), VB-I-10 (bounded-window)
- `milestones/M-38-roadmap.md` (GW-I-9 зарезервирован за чекпоинтом; порядок работ)
- `crates/gateway/src/lib.rs:671-677` (`bucket_time_s` — источник тихой полу-правды при tf≤0)
- `.claude/rules/testing.md` (чек-лист; п.7 парный vantage), `.claude/rules/gates.md` §3/§5
