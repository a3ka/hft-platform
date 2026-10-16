<!-- GATE-META
milestone: M-98
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: 2bb38e473f36f54164a821248b8d7f5e13e97908
verdict: APPROVE
-->

# R-259 — M-98 задача 1: провенанс истории на ветке переподписки — PR-гейт

## Вердикт: APPROVE

Задача 1 `M-98` исполнена в зоне, по спеке и без ослабления соседних путей. Снимок,
который `gateway-serve` отдаёт клиенту, переподписавшемуся с тем же `id`, теперь несёт
`history_start_seq` / `history_truncated`, посчитанные по каталогу журнала после проверки
свежести, с fail-closed `(frozen, true)` на любой `Err` — тем же правилом, что ADD и legacy
после `M-95`. Это закрывает `TD-251` в части кода; задача 2 (§8 деплой-гейт) снимается после
merge'а отдельным артефактом close-out'а.

Остаток — три карточки долга (одна найдена этим гейтом мутацией продуктового кода, две —
обязательства спеки §5 и `A-053` п.4), ни одна не блокирует: см. §5.

## §0. Предмет — ветка, вершина взята командой

```text
$ git fetch origin && git rev-parse origin/feat/M-98-switch-provenance
2bb38e473f36f54164a821248b8d7f5e13e97908
$ git rev-parse origin/main
a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
$ git merge-base --is-ancestor origin/main origin/feat/M-98-switch-provenance; echo main_is_ancestor=$?
main_is_ancestor=0
```

SHA из мандата tester'а (`2bb38e47`) совпал с вершиной. База ветки — текущий `origin/main`
(`a660b3a7`), значит дерево слияния совпадает с деревом ветки: отдельный merge-preview прогон
не даёт новой информации (`gates.md` §8, `TD-234` неприменим — ветка не отстаёт). Повторный
`fetch` перед записью вердикта — вершины те же.

## §1. Block-scope

```text
$ git diff --name-only a001512e..2bb38e47          # коммит dev'а
crates/gateway-serve/src/lib.rs
$ git show --numstat --format='' 2bb38e47
78	2	crates/gateway-serve/src/lib.rs
```

- Правка — только ветка SWITCH `handle_v1_message` (`crates/gateway-serve/src/lib.rs:1393-1480`):
  `let (mut live, …)`, `let mut snap`, точка `m98-switch-catalog:<id>` под
  `#[cfg(feature = "testing")]`, блок «каталог → `is_fresh` → (`refresh`) → провенанс →
  `put_segment_catalog`». ADD (`:1616-1698`) и legacy (`:2473-2538`) не тронуты ни строкой.
  Это в пределах §10 спеки (engine-dev: ветка SWITCH; вынос общего блока — разрешён, не
  обязателен — §3 «Форма — указание»).
- Остальной диапазон `a660b3a7..2bb38e47` — набор architect'а (спека, RED, гейт, проба
  карты, прототип/мутанты), вердикты `C-297`/`C-302`/`C-307`, арбитраж `A-053`, строка
  `docs/ROADMAP.md`. Все пути — в зонах авторов по §10 и `scope-guard.md`.
- **Запретный список §6 — проверен по коду:** SWITCH не уведён в ADD (канал
  `m98-switch-catalog` отличен от `m95-catalog`, `red_ws_session` `o1`/`o12` зелёные); `Err` не
  глотается (`Err(_) => None` → `(frozen, true)`); «всегда `true`» нет (`s0` зелёный); второго
  `SegmentCatalog::open` нет (`take_segment_catalog`/`put_segment_catalog` — тот же каталог
  `resume`); ADD/legacy без изменений; `*/tests/**`, `scripts/**` в коммите dev'а не тронуты.

## §2. Block-C и риск-блок

- `crates/contracts/**` в диапазоне не тронут → Block-C неприменим. Поля снимка и
  `GATEWAY_SCHEMA_VERSION` не менялись (§5 спеки).
- `risk`/`killswitch`/`oms`/`venue-*` не тронуты → `risk-critic` не требуется
  (`gates.md` §5). `gateway-serve` — read-only консюмер журнала (`VB-I-3`), order-path нет.

## §3. RED-first, атомарность, предъявление FA

- RED-набор `crates/gateway-serve/tests/red_m98_switch_provenance.rs` введён `ec85d138` и
  доведён по `C-297`/`A-053` коммитами architect'а **до** `2bb38e47`; коммит dev'а файлов
  `*/tests/` не трогает (`git diff --name-only` выше). Sacred-набор не переписан под реализацию.
- Одна задача — один коммит с ссылкой на milestone/task и меткой `[engine-dev]`;
  `Co-Authored-By` нет.
- **FA:** у `crates/gateway-serve` своей FA нет (долг `reading-map.md` §2); барьер
  `check_review_fa.sh:195-196` сопоставляет крейт с `docs/fa/viz-backend.md`. Живые
  инварианты предмета: **`VB-I-11`** (провенанс истории, `docs/fa/viz-backend.md:294`) —
  «не выдавать усечённое за полное и называть начало точно»; **`VB-I-2`**
  (`docs/fa/viz-backend.md:285`, live == replay) — правка меняет только метку полноты, не
  рыночные ряды.
- **Семейство `GS-I` транспорта** (барьер ищет его живое множество `GS-I-1 GS-I-2 GS-I-4`;
  дом — `milestones/M-28-gateway-serve.md:49-53`, `docs/fa/viz-backend.md:360-374` называет
  это долгом размещения): предмет касается **`GS-I-4`** (wire-roundtrip + `schema_version`,
  оракул `red_wire_roundtrip.rs`) — правка задаёт значения УЖЕ существующих полей
  `history_*` до сериализации, форма провода и версия не меняются; `red_wire_roundtrip` входит
  в `cargo test --all` шага ci-parity, зелёный. `GS-I-1` (нет app-БД) — не задет, импортов не
  добавлено. Названо честно: `GS-I-5` (`snapshot_msg` — ровно `gateway::snapshot`) в живое
  множество барьера не входит, а транспорт после `M-95` на ADD/legacy и теперь на SWITCH
  ПЕРЕЗАПИСЫВАЕТ `history_*` библиотечного снимка — это правило `M-95` §3, принятое гейтами
  (`R-251`), а не новое отступление `M-98`.

## §4. Done Block и мутационный контроль — исполнено reviewer'ом лично

Прогон в своём дереве `/tmp/hft-reviewer-M-98` (detached на `2bb38e47`).

```text
$ bash scripts/verify_M-98.sh 2>&1 | grep -E '^(PASS|FAIL|SKIP)' | awk '{print $1}' | sort | uniq -c
     64 PASS
      8 SKIP
$ bash scripts/verify_M-98.sh 2>&1 | grep -E '^(PASS|FAIL|SKIP)' | head -8
PASS  task1: red_m98_switch_provenance (s0…s4)
PASS  task1: red_m98_mutants.sh (прототип + 6 мутантов: база красна целиком, прототип зелен, мутанты — объявленные наборы)
PASS  I-2: gateway-serve red_m95_provenance_fresh (f0…f6)
PASS  I-2: gateway-serve red_m87_provenance_on_add_path
PASS  I-3: gateway-serve red_ws_session
PASS  ci-map: проба карты CI-паритета (red_verify_M-98_ci_map.sh; число миров печатает проба)
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
$ … | tail
PASS  ci-parity: учтено шагов 64 из 64 (исполнено 57, исключено по карте 6)
SKIP  ci-parity: check_review_fa — вердикта R-NNN по M-98 в диапазоне ещё нет (зеленеет на PR-гейте)
SKIP  task2: §8 деплой-гейт (healthy, heartbeat, wsprobe-снимок) — снимает reviewer
VERDICT: PASS
verify_exit=0
```

FAIL — ноль. Восемь SKIP — шесть исключений карты CI (установка инструментов, плумбинг
CI, агрегат), `check_review_fa` (этот файл его и закрывает) и `task2` (§8 после merge'а).

**Мутации ПРОДУКТОВОГО кода ветки SWITCH.** `A-053` NOTE 1 прямо предупреждает: матрица
`red_m98_mutants.sh` судит прототип на `lib.rs` базы `a660b3a7`, а не реализацию dev'а, и
`VERDICT: PASS — 8 миров` за проверку реализации не принимается. Поэтому мутации — на
`2bb38e47`, по одной, с возвратом кода (`git checkout -- crates/gateway-serve/src/lib.rs`,
`git status --porcelain` пуст после каждой):

| # | мутация (строка на `2bb38e47`) | красны | вывод |
|---|---|---|---|
| R1 | fail-closed `(snap.history_start_seq, true)` → `false` (`:1475`) | `s3 s4` | ветка `Err` запиннена |
| R2 | присвоение провенанса в снимок удалено (`:1479-1480`) | `s1 s2 s3 s4` | расчёт реально доходит до снимка |
| R3 | `is_fresh` зовётся, исход игнорируется — всегда `Ok(true)` (`:1455`) | `s3 s4` | см. ниже |
| R4 | `is_fresh` не зовётся вовсе — `Ok(true)` (`:1452-1454`) | `s2 s3 s4` | совпало с миром `nofresh` матрицы |
| R5 | `is_fresh → Ok(false)`, `refresh → Ok` ⇒ всё равно `(frozen, true)` (`:1463`) | **нет — 5/5 зелёные** | **Н-1** |
| база | код возвращён | `5 passed; 0 failed` | |

```text
##### MUT R1
test s3_switch_catalog_unlistable_is_fail_closed ... FAILED
test s4_switch_refresh_failure_after_stale_is_fail_closed ... FAILED
test result: FAILED. 3 passed; 2 failed; …
##### MUT R2
test s1_switch_after_retention_is_honest ... FAILED
test s2_switch_retention_between_catalog_and_provenance_is_honest ... FAILED
test s3_switch_catalog_unlistable_is_fail_closed ... FAILED
test s4_switch_refresh_failure_after_stale_is_fail_closed ... FAILED
test result: FAILED. 1 passed; 4 failed; …
##### MUT R3
test s3_switch_catalog_unlistable_is_fail_closed ... FAILED
test s4_switch_refresh_failure_after_stale_is_fail_closed ... FAILED
test result: FAILED. 3 passed; 2 failed; …
##### MUT R4
test s2_switch_retention_between_catalog_and_provenance_is_honest ... FAILED
test s3_switch_catalog_unlistable_is_fail_closed ... FAILED
test s4_switch_refresh_failure_after_stale_is_fail_closed ... FAILED
test result: FAILED. 2 passed; 3 failed; …
##### MUT R5
test result: ok. 5 passed; 0 failed; …
##### BASE (вернул код)
test result: ok. 5 passed; 0 failed; …
```

R3 против R4: `SegmentCatalog::is_fresh` (`crates/journal/src/segments.rs:241`) при малом
расхождении состава САМ обновляет каталог инкрементально (док-комментарий `:231-237`), поэтому
мутант, который вызов оставляет, а исход игнорирует, в мире `s2` (удалён один `.zst`) получает
уже обновлённый каталог — `s2` зелёный по существу, а не по слепоте. Мутант, убирающий вызов
(R4), ловится `s2`. Это свойство каталога, не пробел оракула.

## §5. Находки (не блокируют)

**Н-1 — мир «каталог устарел, обновление прошло» не запиннен ни на одном из трёх путей.**
R5 на SWITCH проходит `s0…s4` 5/5. Тот же мутант, наложенный на ADD (`:~1682`) и legacy
(`:~2522`) одновременно, проходит `red_m95_provenance_fresh` 7/7 и
`red_m87_provenance_on_add_path` 2/2:

```text
##### MUT R6 тот же мутант на ADD и legacy
test result: ok. 7 passed; 0 failed; …   red_m95_provenance_fresh exit=0
test result: ok. 2 passed; 0 failed; …   red_m87_provenance_on_add_path exit=0
```

То есть ветка `Ok(false) → refresh → Ok → провенанс по обновлённому каталогу` не имеет
оракула нигде — пред-существующий с `M-95` пробел класса, `M-98` его не внёс, а
воспроизвёл (набор перенесён из `red_m95_provenance_fresh`). Мир `s4`/`f5`/`f6` доходит до
`refresh`, но только с `Err`. **Почему не блокер:** направление ошибки мутанта —
перестраховка «история усечена, начало = слепок», а не ложь «история полная»; первая половина
`VB-I-11` цела, нарушается вторая («называть начало точно»), и только в окне между `resume` и
расчётом, где `is_fresh` должен вернуть `Ok(false)` (ротация/исчезновение хвоста, массовое
изменение состава). Код dev'а на этой ветке верен — пробел в наборе. Карточка `TD` — на
close-out; форма оракула (мир «≥3 изменений состава при успешном `refresh`») — architect
(`gates.md` §4, граница reviewer↔architect).

**Н-2 — обязательство `A-053` п.4 / спеки §5 строка 1.** Точный `frozen_start_seq`
fail-closed исхода не пиннится на ПУТИ выдачи ADD/legacy (`f3…f6`); `f1`/`f2`/`o6_pruned`
сверяют начало только `> 0`. Карточка `TD` — на close-out, текст — по `A-053`.

**Н-3 — обязательство спеки §5 строка 2.** На SWITCH нет проверки `is_cap_terminal()`, которая
есть на ADD (`crates/gateway-serve/src/lib.rs:1616` на `2bb38e47`): отказ по пределу объёма
на переподписке приходит кодом `invalid_selector` (через `snapshot_checked` → `PL-I-5`,
комментарий `:1399-1403`), на новой подписке — `overloaded` (`M-87` задача 18). Клиент по
неверному коду чинит параметры вместо сужения окна. Предсуществующее, вне предмета `M-98`.
Карточка `TD` — на close-out.

**Н-4 (без карточки) — блок «каталог → свежесть → провенанс» теперь живёт в трёх копиях**
(SWITCH `:1436-1478`, ADD `:1648-1696`, legacy `:2491-2536`), дословно одинаковых. Спека §3
вынос в функцию предлагала как указание, не требование, — нарушения нет. Риск — расхождение
копий при следующей правке; Н-1 закрывается оракулами на всех трёх путях, что этот риск и
покрывает.

## §6. Условие APPROVED — выполнено

1. Scope = §10 спеки, запретный список §6 соблюдён — да (§1).
2. `verify_M-98.sh` `exit=0`, FAIL = 0, сырой вывод — да (§4).
3. RED не переписан dev'ом; мутации продуктового кода роняют `s1…s4` по каждой защищаемой
   строке, кроме названного пробела Н-1 — да (§4, §5).
4. Block-C / risk-critic — неприменимы, проверено (§2).
5. FA: `VB-I-11`, `VB-I-2` — названы (§3).

Дальше: PR → зелёные чеки → merge → §8 (задача 2) → close-out (`PROJECT-STATE.md`,
`TECH-DEBT.md`: `TD-251` закрыть, Н-1…Н-3 завести).

## Handoff §D (маршрут)

APPROVE → merge делает reviewer сам. Н-1…Н-3 — карточками `TD` reviewer'ом на close-out;
форма их закрытия — **`architect`** (RED на Н-1/Н-2; решение по Н-3), не dev.
