<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 1103b76633c3b37f3339dde53157fa12c47a6659
audited_head: dcda67c9c5bcb415668e63d005c85f64465d0530
verdict: REJECT
-->

# R-147 — M-68 круг 5 (задачи 23/24/25 + `N-2`/`N-6`): PR-time reviewer, **REJECTED**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-28T21:16Z
**Предмет:** `1103b76..dcda67c` на `origin/feat/M-68-rev4` — три коммита architect'а
(`159d464` задача 24 / `R-145` Б-1, `5b78185` задача 23 / `R-145` Б-2, `dcda67c` — `N-2`/`N-3`/`N-6`).
**Дерево ревью:** `/tmp/hft-rev-m68-r5`, detached на `dcda67c`, чекаут из `origin`.
**Дерево мутаций:** `/tmp/hft-mut-m68-r5`, отдельный worktree — прод-код в дереве ревью не трогался.
**Мандат:** проверить исполнение условий `R-145` (Б-1 оракул задачи 24, Б-2 оракул точки входа,
`N-1`, `N-2`/`N-3`).

**Прочитано на этой ревизии:** `CLAUDE.md`, `.claude/rules/{gates,commit-discipline,scope-guard,
branch-hygiene,testing,handoff-block}.md`, `.claude/agents/reviewer.md`, `docs/04-workflow.md`
§1/§2/§3, `docs/05-contract-layer.md` §1-§6, `docs/workflow/reading-map.md` §1/§2,
`docs/fa/viz-backend.md` (таблица инвариантов), `research/reviews/R-145-M-68-cadence-impl-round4.md`
целиком, `milestones/M-68-depth-from-book.md` (§0sexies.2sexies, §0sexies.2septies, §3, §Tasks,
строка 299 — carve-out гварда), `scripts/verify_M-68.sh` (шаги `C3ter`/`C3bis` целиком),
`crates/gateway/tests/red_checkpoint_bin_prod_argv.rs`,
`crates/gateway-serve/tests/red_depth_cadence_from_env.rs`,
`crates/gateway-serve/src/lib.rs` (`serve_config_from_env`, гвард `:2113-2130`),
`crates/gateway/src/bin/gateway-checkpoint.rs` (`:278-330`), `docker-compose.yml` (обе службы),
`scripts/check_review_fa.sh` (шаги 1-3 — применимость барьера к этому диапазону).
**Ярус C — грепом по предмету, не целиком** (`reading-map.md` §2): `TECH-DEBT.md` по
`M-68|depth_cadence|GATEWAY_DEPTH_CADENCE|TD-044|TD-019|TD-020|TD-167|TD-168|TD-173`
(`:100`, `:733`, `:1149-1187`, `:1230`, `:2334`, `:2735-2896` — живы `TD-167`, `TD-019`/`TD-020`);
`PROJECT-STATE.md` по `M-68|depth_cadence|GATEWAY_DEPTH_CADENCE` — **совпадений ноль**
(милестоун не закрыт; `PROJECT-STATE`/`TECH-DEBT` этим кругом не трогаю — merge'а нет).

**Предъявление FA (M-66).** Диапазон круга 5 трогает ТОЛЬКО `crates/*/tests/**`, поэтому барьер
`check_review_fa.sh` на нём даёт `SKIP` (`:98-105`, классификация `A-012` §1-Д п.5) — требование
здесь КОГНИТИВНОЕ, и я называю это прямо, а не выдаю за машинную проверку. **Диапазон PR
(`dd2d167..dcda67c`) — другой:** он трогает `crates/gateway/src/**` и `crates/gateway-serve/src/**`,
для обоих `FA_OF` = `docs/fa/viz-backend.md` (префиксы `VB` и `GS`), NO-FA-крейтов в нём нет ⇒
`FA-WAIVER` не требуется. Живые инварианты названы прямым чтением FA на этой ревизии:
**`VB-I-2`** (`:189` — «live == replay: серия, посчитанная на live-хвосте, бит-идентична серии из
replay того же окна журнала») — предмет задачи 25, заведённой этим кругом; **`VB-I-1`** (`:188` —
чистый редьюсер, нет wall-clock в расчёте) — held: каденция ведётся временем события.
Собственной `docs/fa/gateway.md`/`gateway-serve.md` не существует — это долг сам по себе
(`reading-map.md` §2, строка `derive`/`recorder`/`gateway-serve`), и он назван, а не обойдён
waiver'ом.

---

## Block-scope — ЧИСТО, покоммитно

| коммит | пути | вердикт |
|---|---|---|
| `159d464` | `crates/gateway-serve/tests/**`, `milestones/M-68-*.md`, `scripts/verify_M-68.sh` | ✅ зона architect'а |
| `5b78185` | `crates/gateway/tests/**`, `milestones/M-68-*.md`, `scripts/verify_M-68.sh` | ✅ зона architect'а |
| `dcda67c` | `milestones/M-68-depth-from-book.md` | ✅ зона architect'а |

`git diff --name-only 1103b76..dcda67c | grep -E 'crates/[^/]+/src/'` → **0**: impl-код диапазоном
не тронут вовсе, и это правильно — architect его не пишет (`scope-guard.md`).

**Block-C: ЧИСТО.** `crates/contracts/**` не тронут (0 файлов). T2-поля
`Selector::depth_cadence_ms` / `SeriesBundle::cadence_ms` — собственность крейта `gateway`,
T-designate по `05-contract-layer.md` §2; contract-RFC не требуется.

**RISK-BLOCK не применяется — проверено, а не предположено** (`R-145` требовал именно проверки).
`git diff --name-only 1103b76..dcda67c` не содержит ни одного пути под
`crates/risk|killswitch|oms|venue-*|contracts`. Предмет — Слой 8, read-only консюмер журнала
(`VB-I-3`); order-egress отсутствует. risk-critic по `gates.md` §5 не требуется.

## Block-commits — ЧИСТО по атомарности

Три предмета — три коммита, бандла нет; каждый subject несёт `M-68` и номер задачи либо номер
находки. `git log --format='%B' 1103b76..dcda67c | grep -c 'Co-Authored-By'` → **0**.
Токен `FOUNDER-APPROVED` не требуется: диапазон не трогает `.claude/**`, `CLAUDE.md`,
`docs/04-workflow.md` (`gates.md` §11).

## Block-DoneBlock — ВОСПРОИЗВЕДЁН СВОИМ ПРОГОНОМ

Отчёт тестера не пересказан: гейт и мутации прогнаны заново в собственных деревьях, сырой вывод —
в Done Block ниже.

---

## Условия `R-145` — что исполнено (замером, а не чтением)

### Условие 1 (Б-1) — задача 24 получила оракул. **ИСПОЛНЕНО, ЗАПИННЕНО МУТАЦИЕЙ.**

`R-145` Б-1 держался на замере: удаление гварда отношения оставляло `gateway-serve` 76/0 и
`gateway` 157/0 зелёными. **Повторил мутацию на этой ревизии — дыра закрыта:**

```
MUT-A: гвард отношения (crates/gateway-serve/src/lib.rs:2113-2130) вырезан ЦЕЛИКОМ, 18 строк
test d18e_misaligned_pair_is_rejected_at_startup ... FAILED
  GATEWAY_TIMEFRAME_MS=3000 + GATEWAY_DEPTH_CADENCE_MS=10000 приняты СТАРТОМ. 10000 % 3000 = 1000 != 0
test result: FAILED. 5 passed; 1 failed
```

Падает РОВНО `d18e`, пять соседей целы. Шаг гейта на задачу 24 существует: `C3bis` исполняет
файл (`chk cargo test -p gateway-serve --test red_depth_cadence_from_env`) и пиннит состав
числом `EXPECT_E=6`, то есть тихое удаление `d18e`/`d18f` роняет гейт.

**Парный vantage `d18f` не декоративен — проверено обратной мутацией.** Оракул, красный против
ПРАВИЛЬНОЙ реализации, хуже отсутствующего, поэтому проверял в обе стороны:

```
MUT-D: гвард расширен — условие `cadence >= timeframe` снято (реализация «отвергать всё, что не делится»)
test d18f_cadence_below_timeframe_is_not_rejected ... FAILED
  GATEWAY_DEPTH_CADENCE_MS=1000 при GATEWAY_TIMEFRAME_MS=3000 отвергнут стартом … Гвард стал шире собственного контракта
test result: FAILED. 5 passed; 1 failed
```

Carve-out, который `d18f` защищает, спекой НАЗВАН — `milestones/M-68-depth-from-book.md:299`
(«`cadence_ms % timeframe_ms != 0` **при `cadence >= timeframe`** → отказ старта»), то есть оракул
судит контракт документа, а не вкус автора. Строка задачи 24 в §Tasks теперь называет `d18e`/`d18f`
и явно снимает ложную ссылку на `d20` цитатой, а не стиранием.

**Счётный регекс исправлен по существу, а не косметически.** `^fn [a-z_]+\(\) \{` не совпал бы с
`d18e_…`/`d18f_…` (цифры в имени) — литерал дал бы ЛОЖНОЕ КРАСНОЕ на правильной правке. Проверил,
что новая форма считает ровно тесты: `grep -cE "^fn [a-z0-9_]+\(\) \{"` → **6** при
`grep -c '^#\[test\]'` → **6** (хелперы `getter`/`base_plus`/`compose_service_block` имеют
аргументы и не считаются).

### Условие 2 (Б-2) — оракул точки входа. **ИСПОЛНЕНО, ЗАПИННЕНО МУТАЦИЕЙ.**

Инвентарь `C3ter` снят, на его месте — `c3ter_writer_and_reader_agree_on_checkpoint`
(`crates/gateway/tests/red_checkpoint_bin_prod_argv.rs:388-502`): запуск НАСТОЯЩЕГО бинаря через
`env!("CARGO_BIN_EXE_gateway-checkpoint")` с argv, разобранными из `docker-compose.yml`, затем
публичный `LiveReducer::resume` и свидетель `ReadStats::events_decoded`. Мутационную таблицу
коммита `5b78185` НЕ принял на веру — воспроизвёл:

```
MUT-B: crates/gateway/src/bin/gateway-checkpoint.rs:327  Some(cadence_raw) -> None
test c3ter_writer_and_reader_agree_on_checkpoint ... FAILED
  КОМПОЗИЦИЯ РАЗОРВАНА: читатель при каденции 1000 мс декодировал 300 событий вместо 0
  left: 300   right: 0
test result: FAILED. 6 passed; 1 failed
```

Числа сошлись с заявленными в теле коммита («300 событий вместо 0»), падает ровно этот оракул,
шесть соседей в файле целы.

**Проверил парный vantage на «зелёный по неверной причине».** Контроль в оракуле построен на
`reader_selector(None)`, и мог бы держаться не на отпечатке, а на том, что путь `None` вообще не
ищет слепок. Заменил контроль на ДРУГУЮ непустую каденцию:

```
ПРОБА: control-селектор None -> Some(60_000)
test c3ter_writer_and_reader_agree_on_checkpoint ... ok
```

Оракул остаётся зелёным и на непустом контроле ⇒ он различает ИМЕННО расхождение каденции.
**Дефекта здесь нет, и я называю это явно**, потому что подозрение было предъявимым.

**Setup-guard оракула не декоративен.** При снятой ручке у службы он падает ГРОМКО, а не зеленеет:

```
MUT-C (побочно): SETUP НЕ СОСТОЯЛСЯ: в argv службы gateway-checkpoint нет --depth-cadence-ms.
  Композиция не может быть предъявлена … argv: [… "--cursor=LATEST"]
```

Это свойство 3 целостности гейта (`testing.md`): падать и против сломанного, и против
несостоявшегося setup'а.

**Вторая половина Б-2 — послужебный разбор compose. ИСПОЛНЕНО.** `R-145` замерил, что
`knob_is_declared_in_compose` искал подстроку по ВСЕМУ файлу и оставался зелёным при снятии ручки
у одной службы. Повторил ту же мутацию против новой редакции:

```
MUT-C: у службы gateway-checkpoint сняты ОБЕ строки (--depth-cadence-ms и environment);
       во ВСЁМ файле переменная ОСТАЛАСЬ (2 вхождения у gateway-serve)
test knob_is_declared_for_both_services_in_compose ... FAILED
  GATEWAY_DEPTH_CADENCE_MS не объявлена у службы `gateway-checkpoint` в …/docker-compose.yml
test result: FAILED. 5 passed; 1 failed
```

Дыра закрыта ровно по своему признаку: переменная в файле есть, страж всё равно красный.

**Ложное утверждение снято ЦИТАТОЙ, а не стиранием.** §0sexies.2sexies сохраняет прежний текст
(«…из интеграционного теста Rust недостижима — `selector_fingerprint` и `ckpt_path_for` объявлены
`pub(super)`») с пометкой «утверждение опровергнуто прогоном» и с разбором класса ошибки. Остаток
объявлен `COGNITIVE-ONLY` с НАЗВАННЫМ пределом — «оракул не исполняет `docker compose up` и не
проверяет, что оркестратор подставит те же значения; механизируется контейнерным прогоном в CI и
не сделан ПО ЦЕНЕ, а не по невозможности». Это ровно та форма, которую требовало условие 2:
предел, а не утверждение о невозможности целого.

### Условие 4 (`N-2`/`N-3`) — **ИСПОЛНЕНО.**

§3 Allowed paths приведены к факту («задачи 22, 23, 24 — carve-out»), с врезкой, называющей
собственную ошибку. Задача 25 заведена под предмет Б-2 `R-141` (`VB-I-2` под каденцией), исполненный
коммитом `6c9ed41` и не имевший своей строки; ссылка на `d20` там верна — этот оракул действительно
её предмет. `N-6` закрыт: статус задачи 11 назван по факту rev5.

---

## Б-1 (БЛОКЕР, ЕДИНСТВЕННЫЙ) — ветка НЕ СЛИВАЕТСЯ с `main`; merge-цель не собирается, и её не проверял никто

**Это не дефект круга 5.** Три коммита architect'а исполнены верно — см. весь раздел выше.
Блокер лежит в другом: предмет, который я обязан ВЛИТЬ по APPROVED, влить физически нельзя.

**Замер.** `origin/feat/M-68-rev4` отстаёт от `origin/main` на **106 коммитов**, и `main` за это
время переписал РОВНО ТЕ ЖЕ прод-файлы, что правит M-68 (милестоун `M-71` egress-cap):

```
$ git rev-list --count HEAD..origin/main
106
$ git merge --no-commit --no-ff origin/main
CONFLICT (content): Merge conflict in crates/gateway-serve/src/lib.rs
CONFLICT (content): Merge conflict in crates/gateway/src/lib.rs
CONFLICT (content): Merge conflict in docker-compose.yml
Automatic merge failed; fix conflicts and then commit the result.
```

**Гейт, предписанный `gates.md` §8 для отстающей ветки, КРАСЕН — это механизм проекта, а не моё
мнение:**

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main /tmp/hft-rev-m68-r5
FAIL  [SETUP] --merge-preview: слияние base-ref 'origin/main' + HEAD (dcda67c9…) КОНФЛИКТУЕТ —
      merge-цель не собирается автоматически, документ на ней не проверяем; разреши конфликт
      вручную и прогони обычный режим на результате
VERDICT: FAIL (1 нарушений)
exit=1
```

**Почему это блокер, а не примечание — предел `strict: false` назван в `gates.md` §8 дословно:**
защита `main` требует зелёный чек, но НЕ требует свежести ветки, поэтому «зелёный чек снят на
СТАРОЙ базе: отставшая ветка вливается зелёной, а `main` после слияния может покраснеть». Мой
прогон гейта на ветке (Done Block ниже, `VERDICT: PASS`) СУДИТ БАЗУ `dd2d167`, которой в `main`
уже нет. Merge-коммит как таковой не тестируется никем до попадания в `main` — а здесь он даже не
существует.

**Резолюция НЕ механическая — назову конкретную опасность, чтобы её не проскочили.** Второй
конфликт в `crates/gateway-serve/src/lib.rs` (`:2354-2397`) сводит рядом две вставки:

- сторона M-68 — гвард отношения задачи 24, содержащий ветки `return Err(...)`;
- сторона `main` (M-71) — `gateway::set_effective_max_response_bytes(max_response_bytes);` с
  инвариантом в комментарии: «сеттер зовётся **СТРОГО ПОСЛЕ** успешного разбора — все ветки отказа
  выше делают `return Err(...)` ДО этой строки. Класс `GW-I-14`/`R7`: отвергнутая конфигурация не
  смеет управлять сервисом. Пиннится оракулом `N1-E`».

Резолюция «взять обе стороны» в неверном ПОРЯДКЕ (гвард M-68 после сеттера M-71) тихо ломает
`PL-I-5`: отвергнутая старт-конфигурация успевает выставить глобальный предел. Оракул `N1-E`
живёт в `main` и, вероятно, это поймает — но проверить это можно только НА СОБРАННОМ ДЕРЕВЕ
СЛИЯНИЯ, которого сейчас нет. Третий конфликт (`crates/gateway/src/lib.rs:2749`) того же рода:
M-68 меняет сигнатуру вызова `read_stats_from_stream(&stream, depth_levels_visited)`, M-71 в ту же
точку добавляет `enforce_response_limit(&series, effective_max_response_bytes())?` — нужны обе
правки, и это семантическое слияние, а не выбор стороны.

**Почему разрешаю не я.** `crates/*/src/**` и `docker-compose.yml` — зона engine-dev
(`scope-guard.md`, §3 милестоуна); `.claude/agents/reviewer.md` §NEVER writes закрывает мне этот
путь прямо. Разрешить конфликт означало бы написать прод-код и самому же его одобрить — ровно то,
против чего стоит PR-гейт. `gates.md` §4 (граница reviewer↔architect) говорит то же: я ОПИСЫВАЮ
дефект и не проектирую фикс.

**Что требуется для следующего круга:**

1. engine-dev вливает `origin/main` в `feat/M-68-rev4`, разрешая три конфликта; порядок
   «гвард M-68 ДО `set_effective_max_response_bytes`» соблюдён явно и назван в теле коммита.
2. Прогон гейта — **на дереве слияния, а не на ветке**: базовая тройка CI
   (`cargo fmt --all -- --check` · `cargo clippy --all-targets --all-features -- -D warnings` ·
   `cargo test --all`) + `bash scripts/verify_M-68.sh` + `bash scripts/verify_M-71.sh`
   (M-71 приехал в `main` и его инварианты — соседи, купленные этим слиянием) +
   `bash scripts/verify_design_claims.sh --merge-preview origin/main <дерево>` → exit=0.
3. Следующий круг reviewer'а судит РЕЗОЛЮЦИЮ СЛИЯНИЯ, а не круг 5 заново: находки этого вердикта
   по задачам 23/24/25 закрыты и перепроверены мутацией — переоткрывать их незачем.

---

## `N-1` (MAJOR NOTE, ПЕРЕНЕСЁН ИЗ `R-145` — НЕ ИСПОЛНЕН, и это законно)

Условие 3 `R-145` («комментарий приводится к правде ЛИБО поведение к комментарию») диапазоном
круга 5 не исполнено: `crates/gateway/src/bin/gateway-checkpoint.rs:290` по-прежнему несёт
`// Невалидное env обрабатывается ниже как fail-closed`, тогда как `trimmed.parse::<i64>().ok()`
глотает ошибку разбора и отдаёт `None`, после чего `args.depth_cadence_ms.or(cadence_from_env)`
подставляет дефолт 1000. Проверено чтением на `dcda67c`: строки не менялись.

**Почему это НЕ блокер и почему отсутствие правки здесь законно.** Зона — `crates/gateway/src/**`,
то есть engine-dev; диапазон круга 5 — architect'а, и он правильно в неё не полез. `R-145` сам
назвал находку не-блокером: одна переменная имеет два контракта на мусор, но прод от этого не
деградирует ТИХО — `serve_config_from_env` откажет старту, контейнер не поднимется, деплой
покраснеет. Класс остаётся тот же (`TD-167`: самоописание кода расходится с кодом), и он уходит в
следующий круг вместе с резолюцией слияния — а не пропадает.

## `N-2` (NOTE) — заголовок шага `C3bis` не называет задачу 24, хотя покрывает её

`grep -nE '^step ' scripts/verify_M-68.sh` даёт 17 шагов; заголовок шага звучит
«C3bis (задача 22 — R-138 Б-3) …». Покрытие задачи 24 РЕАЛЬНО (я его запиннил мутацией MUT-A выше:
`chk` исполняет файл, `EXPECT_E=6` держит состав), но названо оно только в комментарии внутри шага
(`:156`, «ЗАДАЧА 24 была реализована БЕЗ ЕДИНОГО ОРАКУЛА»). Читающий заголовки — включая меня в
`R-145`, где `grep -c 'задача 24'` дал 0 — сделает вывод, что шага на задачу 24 нет. Тот же класс
`TD-167`; правка стоит одной строки заголовка.

## `N-3` (NOTE) — коммит `159d464` несёт два предмета

Subject называет задачу 24 (`R-145` Б-1), но коммит содержит и вторую половину Б-2 — замену
`knob_is_declared_in_compose` на послужебный разбор. Оба предмета живут в одном файле и одном
наборе, бандлом на пять задач это не является (`commit-discipline.md`), но subject о втором
предмете молчит. Замечание фиксирую, откатывать не требую.

## Что проверено и дефекта НЕ найдено — названо явно

- **Счётные пороги гейта соответствуют наборам** — не заявлены, а посчитаны:
  `C3ter` `EXPECT_T=7` при `grep -c '^#\[test\]'` → 7; `C3bis` `EXPECT_E=6` при 6.
- **Контроль оракула `C3ter` держится на отпечатке, а не на обходе слепка** — проба с
  `Some(60_000)` выше.
- **`d18f` не куплен ценой `d18e`** — обратная мутация MUT-D роняет именно `d18f`.
- **Соседние наборы не куплены:** прогон гейта целиком (Done Block) — `VERDICT: PASS`, ноль FAIL.
- **`gc_worktrees.sh` в close-out не запускался** — close-out'а нет, merge'а нет; свои деревья
  (`/tmp/hft-rev-m68-r5`, `/tmp/hft-mut-m68-r5`) убираю сам, см. Handoff §D.

---

## ВЕРДИКТ: **REJECTED**

**Один блокер, и он НЕ в предмете круга 5.** Три коммита architect'а исполнены верно: оба блокера
`R-145` закрыты, и закрыты они не заявлением, а механизмом — каждый новый оракул падает против
своего дефекта и только против него (MUT-A, MUT-B, MUT-C), а парный vantage `d18f` дополнительно
проверен обратной мутацией MUT-D. Ложное «недостижимо» снято цитатой с разбором класса ошибки,
остаток объявлен `COGNITIVE-ONLY` по ЦЕНЕ, а не по невозможности. Условия 1, 2 и 4 `R-145`
исполнены полностью.

**Merge при этом невозможен физически.** Ветка отстаёт от `main` на 106 коммитов; `main` за это
время переписал те же прод-файлы (M-71 egress-cap); слияние даёт три конфликта, и предписанный
`gates.md` §8 гейт `verify_design_claims.sh --merge-preview` возвращает `FAIL [SETUP]`, exit=1.
Дерево слияния не собрано, не собрано автоматически и не проверялось НИКЕМ — а `strict: false`
означает, что зелёный чек на ветке о состоянии `main` после merge'а не говорит ничего. Разрешение
конфликтов — прод-код (`crates/*/src/**`, `docker-compose.yml`), зона engine-dev; мне он закрыт
профилем, и разрешать его самому, чтобы самому же одобрить, — упразднить PR-гейт.

`APPROVED` здесь означал бы «вливаю», а влить нечего: merge-цели не существует. Поэтому вердикт —
`REJECTED`, с явной атрибуцией: **работа круга 5 принята по существу и переоткрытию не подлежит;
блокирует состояние ВЕТКИ, а не её содержимое.**

**Условие APPROVED (следующий круг):**

1. `origin/main` влит в `feat/M-68-rev4`, три конфликта разрешены engine-dev'ом; в
   `crates/gateway-serve/src/lib.rs` гвард отношения задачи 24 стоит **ДО**
   `gateway::set_effective_max_response_bytes(...)` — иначе отвергнутая старт-конфигурация успевает
   выставить глобальный предел и `PL-I-5` M-71 ломается молча. Порядок назван в теле коммита.
2. Гейты прогнаны **на дереве слияния**: базовая тройка CI + `verify_M-68.sh` + `verify_M-71.sh` +
   `verify_design_claims.sh --merge-preview origin/main <дерево>` → exit=0 у каждого.
3. `N-1` (комментарий `gateway-checkpoint.rs:290` лжёт о fail-closed) закрыт тем же кругом —
   он и так в зоне engine-dev, и отдельного круга не стоит.
4. `N-2` — заголовок шага `C3bis` называет задачи 22 И 24 (зона architect'а, одна строка).

**Маршрут.** `A-025` §5.5: круга критика по M-68 не существует ни при каком исходе, поэтому
REJECT уводит предмет к **founder'у** на диспетч, а не на новый круг гейта. Диспетчеризовать
следует engine-dev'а (пункты 1-3) и architect'а (пункт 4) — оба пункта в разных зонах и в одну
роль не сводятся.

**`PROJECT-STATE.md` и `TECH-DEBT.md` этим кругом не трогаю** — merge'а не было, close-out'а нет.
Карточки долга по классу «инвентарь вместо канарейки», `TD-167`, `TD-168`, `TD-173` и по остатку
`COGNITIVE-ONLY` §0sexies.2sexies завожу в close-out ПОСЛЕ merge'а, как и предписано `gates.md` §4.

---

## Done Block (сырой stdout)

```
$ pwd; git rev-parse HEAD; git status --porcelain
/tmp/hft-rev-m68-r5
dcda67c9c5bcb415668e63d005c85f64465d0530
{пусто}

$ git log --format='%h %s' 1103b76..dcda67c
dcda67c docs(M-68): R-145 N-2/N-3/N-6 — зона, недостающая задача, устаревший статус [architect]
5b78185 test(M-68): task #23 — ОРАКУЛ ТОЧКИ ВХОДА вместо инвентаря; ложное «недостижимо» снято (R-145 Б-2) [architect]
159d464 test(M-68): task #24 — RED d18e/d18f: невыравненная пара отвергается на старте (R-145 Б-1) [architect]

$ git log --format='%B' 1103b76..dcda67c | grep -c 'Co-Authored-By'
0

$ git diff --numstat 1103b76..dcda67c
108	8	crates/gateway-serve/tests/red_depth_cadence_from_env.rs
114	1	crates/gateway/tests/red_checkpoint_bin_prod_argv.rs
52	19	milestones/M-68-depth-from-book.md
45	28	scripts/verify_M-68.sh

$ git diff --name-only 1103b76..dcda67c | grep -cE 'crates/[^/]+/src/|^crates/contracts'
0

$ bash scripts/verify_M-68.sh; echo "verify_exit=$?"
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
PASS: cargo test -p gateway --test red_depth_from_book --quiet
PASS: A состав набора — 9 оракулов (ожидалось ровно 9: d1 d2 d3 d4 d5 d7 d7b d8 d8b)
PASS: B набор КРАСЕН против мутанта C-M68-1 (мутация внесена и прогнана в копии)
PASS: cargo test -p gateway --test red_depth_recompute_cost --quiet
PASS: cargo test -p gateway --test red_depth_semantics --quiet
PASS: C2 состав набора — 3 оракулов (ожидалось ровно 3: d9 d9-C d10)
PASS: cargo test -p gateway --test red_checkpoint_bin_prod_argv --quiet
PASS: C3ter состав набора — 7 оракулов (ожидалось ровно 7, включая c3ter_writer_and_reader_agree_on_checkpoint)
PASS: cargo test -p gateway-serve --test red_depth_cadence_from_env --quiet
PASS: C3bis состав набора — 6 оракулов (ожидалось ровно 6: доходит, дефолт≡пусто≡отсутствие, отказ на мусоре, объявлена у ОБЕИХ служб, d18e невыравненная пара, d18f carve-out)
PASS: cargo test -p gateway --test red_depth_cadence --quiet
PASS: C3 состав набора — 7 оракулов (ожидалось ровно 7: d12 d13 d14 d15 d16 d17 d20)
PASS: C4 самоописание согласовано (обещаний=0, собственных материализаций=2)
PASS: C4 ложное самоописание снято — снятая snapshot-only семантика поля depth_reach_bid (lib.rs:636-658)
PASS: C4 ложное самоописание снято — то же, вторая половина того же комментария
PASS: C4 ложное самоописание снято — ложное «как прежний depth_within с None mid» (lib.rs:1134-1136)
PASS: D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)
PASS: cargo test -p gateway --test red_gateway_schema_version --quiet
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
PASS: cargo test -p gateway --test red_depth_provenance_by_reach --quiet
PASS: H crates/contracts не тронут
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут (судятся только изменённые строки)
PASS: J selector_fingerprint не переписан
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: PASS
verify_exit=0

$ grep -E "^test result" <лог того же прогона> | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=962 failed=0 (блоков: 224)

$ grep -c '^PASS' <лог того же прогона>
29

$ grep -cE '^step ' scripts/verify_M-68.sh
17

$ # ── СОСТАВ НАБОРОВ: пороги ПОСЧИТАНЫ, а не приняты на веру ──
$ grep -cE "^fn [a-z0-9_]+\(\) \{" crates/gateway/tests/red_checkpoint_bin_prod_argv.rs; grep -c '^#\[test\]' crates/gateway/tests/red_checkpoint_bin_prod_argv.rs
7
7
$ grep -cE "^fn [a-z0-9_]+\(\) \{" crates/gateway-serve/tests/red_depth_cadence_from_env.rs; grep -c '^#\[test\]' crates/gateway-serve/tests/red_depth_cadence_from_env.rs
6
6
```

### Мутационный контроль (дерево `/tmp/hft-mut-m68-r5`, прод-код дерева ревью не трогался)

```
$ # MUT-A (условие 1 R-145): гвард отношения вырезан ЦЕЛИКОМ из serve_config_from_env
MUT-A: гвард отношения вырезан, строк снято: 18
$ cargo test -p gateway-serve --test red_depth_cadence_from_env
test d18f_cadence_below_timeframe_is_not_rejected ... ok
test env_cadence_reaches_the_selector ... ok
test invalid_cadence_is_rejected_naming_the_variable ... ok
test knob_is_declared_for_both_services_in_compose ... ok
---- d18e_misaligned_pair_is_rejected_at_startup stdout ----
GATEWAY_TIMEFRAME_MS=3000 + GATEWAY_DEPTH_CADENCE_MS=10000 приняты СТАРТОМ. 10000 % 3000 = 1000 != 0 …
test result: FAILED. 5 passed; 1 failed
      ← R-145 Б-1 замерил здесь 76/0 ЗЕЛЁНЫХ. Дыра закрыта.

$ # MUT-D (обратная мутация): гвард расширен — условие `cadence >= timeframe` снято
$ cargo test -p gateway-serve --test red_depth_cadence_from_env
---- d18f_cadence_below_timeframe_is_not_rejected stdout ----
GATEWAY_DEPTH_CADENCE_MS=1000 при GATEWAY_TIMEFRAME_MS=3000 отвергнут стартом … Гвард стал шире собственного контракта
test result: FAILED. 5 passed; 1 failed
      ← парный vantage не декоративен: набор красен и против ПЕРЕширокой реализации

$ # MUT-B (условие 2 R-145): писатель вшивает depth_cadence_ms: None (gateway-checkpoint.rs:327)
$ cargo test -p gateway --test red_checkpoint_bin_prod_argv
---- c3ter_writer_and_reader_agree_on_checkpoint stdout ----
КОМПОЗИЦИЯ РАЗОРВАНА: читатель при каденции 1000 мс декодировал 300 событий вместо 0 …
  left: 300
 right: 0
test result: FAILED. 6 passed; 1 failed
      ← числа сошлись с заявленными в теле 5b78185; падает РОВНО этот оракул

$ # MUT-C (вторая половина Б-2): у службы gateway-checkpoint сняты ОБЕ строки каденции
MUT-C: у службы gateway-checkpoint упоминаний было 3 стало 0
во ВСЁМ файле осталось: 2
$ cargo test -p gateway-serve --test red_depth_cadence_from_env
---- knob_is_declared_for_both_services_in_compose stdout ----
GATEWAY_DEPTH_CADENCE_MS не объявлена у службы `gateway-checkpoint` в …/docker-compose.yml
test result: FAILED. 5 passed; 1 failed
      ← R-145 замерил здесь ЗЕЛЁНОЕ (подстрока по всему файлу). Дыра закрыта.

$ # MUT-C, побочно: setup-guard оракула C3ter падает ГРОМКО, а не зеленеет
$ cargo test -p gateway --test red_checkpoint_bin_prod_argv c3ter
SETUP НЕ СОСТОЯЛСЯ: в argv службы gateway-checkpoint нет --depth-cadence-ms …
test result: FAILED. 0 passed; 1 failed

$ # ПРОБА (не мутация): контроль C3ter — держится ли он на ОТПЕЧАТКЕ, а не на обходе слепка
$ sed -i 's|&reader_selector(None),|\&reader_selector(Some(60_000)),|' crates/gateway/tests/red_checkpoint_bin_prod_argv.rs
$ cargo test -p gateway --test red_checkpoint_bin_prod_argv c3ter
test c3ter_writer_and_reader_agree_on_checkpoint ... ok
      ← контроль зелен и на НЕПУСТОЙ чужой каденции ⇒ оракул различает именно расхождение

$ git checkout -- . ; git status --porcelain
{пусто}
```

### Б-1 — предъявление merge-блокера (сырой stdout)

```
$ cd /tmp/hft-rev-m68-r5 && git rev-list --count HEAD..origin/main
106

$ git merge --no-commit --no-ff origin/main
Auto-merging crates/gateway-serve/src/lib.rs
CONFLICT (content): Merge conflict in crates/gateway-serve/src/lib.rs
Auto-merging crates/gateway/src/lib.rs
CONFLICT (content): Merge conflict in crates/gateway/src/lib.rs
Auto-merging docker-compose.yml
CONFLICT (content): Merge conflict in docker-compose.yml
Automatic merge failed; fix conflicts and then commit the result.

$ git status --porcelain | grep -E '^(UU|AA)'
UU crates/gateway-serve/src/lib.rs
UU crates/gateway/src/lib.rs
UU docker-compose.yml
$ git merge --abort

$ bash scripts/verify_design_claims.sh --merge-preview origin/main /tmp/hft-rev-m68-r5
FAIL  [SETUP] --merge-preview: слияние base-ref 'origin/main' + HEAD (dcda67c9c5bcb415668e63d005c85f64465d0530) КОНФЛИКТУЕТ — merge-цель не собирается автоматически, документ на ней не проверяем; разреши конфликт вручную и прогони обычный режим на результате
VERDICT: FAIL (1 нарушений)
exit=1

$ gh pr list --state open --json number,title,headRefName,mergeable
[]
```

**Оговорка о собственной ошибке в этом прогоне, чтобы её не повторили:**
первый вызов `verify_design_claims.sh --merge-preview origin/main` БЕЗ третьего аргумента вернул
`VERDICT: PASS`, exit=0 — ложное зелёное. Причина: `ROOT` по умолчанию берётся от расположения
скрипта (`SCRIPT_ROOT`, `:139`), то есть общий чекаут на `main`, и превью сливало `main` с `main`.
Проверяемое дерево обязано быть названо ТРЕТЬИМ аргументом. Гейт не виноват — форма
задокументирована в его шапке (`:116`); виноват вызов, и он предъявлен здесь, а не скрыт.

## Cross-references

- `R-145` (условия APPROVED этого круга: Б-1 оракул задачи 24, Б-2 оракул точки входа, `N-1`, `N-2`/`N-3`)
- `R-141` (условие 1 — «оракул точки входа, не греп», исполнено этим кругом)
- `milestones/M-68-depth-from-book.md` §0sexies.2sexies (переписан), §0sexies.2septies, §3, §Tasks 23/24/25
- `gates.md` §2 (RED-first), §3 (≥1 проверка на задачу, паритет с CI), §4 (PR-time, DoD «Механизм на
  пути», граница reviewer↔architect, GATE-META), §5 (RISK-BLOCK — н/п), §8 (**предел `strict: false`,
  merge-preview отстающей ветки — предмет Б-1**), §12 (номер выдан `reserve_artifact_id.sh`)
- `testing.md` («Механизм несущего пути обязан иметь оракул точки входа»; «Мутационный контроль —
  обязателен, и вопросов ДВА»; «Целостность гейта — 4 свойства», свойство 3 — setup-guard)
- `docs/fa/viz-backend.md` `VB-I-1` (:188), `VB-I-2` (:189), `VB-I-3` (:190)
- `scripts/check_review_fa.sh` (:98-105 — SKIP на `crates/*/tests/**`; предел предъявления назван)
- `milestones/M-71-egress-cap.md` §4bis.2 / `PL-I-5` — инвариант, который может пострадать при
  неверном порядке резолюции конфликта
- `TD-167` (самосогласованность артефактов milestone'а), `TD-019`/`TD-020` (healthy-контейнер,
  отвергающий каждое подключение), `TD-044` (цена полного реплея при разошедшемся отпечатке)
