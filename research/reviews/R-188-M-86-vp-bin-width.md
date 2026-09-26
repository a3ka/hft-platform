<!-- GATE-META
milestone: M-86
audited_repo: a3ka/hft-platform
audited_base: 0f55ea7577386c08c22b4cedd428b354308db52e
audited_head: 0e4fb67a23ac7d2dda435f8f1c9a359ea8e6c229
verdict: APPROVE
-->

# R-188 — M-86 (профиль объёма на фиксированной ценовой сетке), PR-гейт: **APPROVED**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-09-19T21:40Z
**Предмет:** `origin/feat/M-86-vp-bin-width` @ `0e4fb67` против `origin/main` @ `10cd40f`;
база набора — `0f55ea7` (merge-base).
**Мои деревья:** `/tmp/hft-reviewer-m86` (ветка, detached `0e4fb67`) и
`/tmp/hft-rev-m86-MP` (**дерево слияния в порядке GitHub**: база = `main`, влита ветка →
`49a0774 parents=10cd40f 0e4fb67`). Порядок родителей взят по `R-187`: обратный порядок даёт
противоположные вердикты барьеров на бит-идентичном содержимом.
**Цепочка до меня:** `C-229` REJECT → `C-230` REJECT → `C-231` REJECT → `A-035` DECISION
(арбитр, Fable) → dev (задачи 1,2,4,5) → фикс sacred-теста architect'ом → tester PASS.

**Главное одной строкой.** Предмет здоров: сетка применена в ЕДИНСТВЕННОЙ точке выдачи,
состояние и отпечаток селектора не тронуты (значит слепок остаётся валидным и 23 минут
простоя мы не платим), гейт зелёный целиком и его мутационный контроль после ремонта
`A-035` действительно различает сломанное. **Два замечания к merge были найдены НЕ гейтом
ветки, а барьером на ДЕРЕВЕ СЛИЯНИЯ** (`gate-meta`, `exit=1`) — ниже §6: оба закрыты этим
вердиктом, и один из них — subject-lock — закрыт ровно тем, что предписал арбитр
(`A-035` §4 п.4: «reviewer судит H/H2 как адверсарий трека»). Четыре находки (§7) merge не
держат и уходят в close-out.

---

## 1. Block-scope — диф соответствует заявленным зонам

Диапазон `0f55ea7..0e4fb67`: 17 файлов, 3 124 вставки, 5 удалений.

| путь | кто тронул | зона |
|---|---|---|
| `crates/gateway/src/lib.rs`, `crates/gateway-serve/src/lib.rs` | engine-dev (`efe3012`, `333942f`, `0f01677`) | §5 **engine-dev** ✅ |
| `docker-compose.yml` | engine-dev (`a65b859`) | §5 **engine-dev** ✅ (объявление ручки СВОЕГО сервиса, не состав данных) |
| `crates/gateway/tests/**`, `crates/gateway-serve/tests/**` | architect | §5 **architect** ✅ (sacred) |
| `milestones/M-86-*.md`, `scripts/verify_M-86.sh`, `docs/fa/viz-backend.md`, `docs/ROADMAP.md` | architect | §5 **architect** ✅ |
| `research/critiques/C-229..231`, `research/arbitration/A-035` | critic / арбитр | зоны ролей ✅ |
| `scripts/lib/mutation_gate.sh` | architect (`2b0cfd3`) | **§5 его НЕ называет** — см. `N-1` |

**Ни один dev-коммит не тронул `*/tests/**`** — проверено покоммитно (`git show --numstat`).
Замечание линтера в sacred-тесте engine-dev вернул через SCOPE VIOLATION, а правку сделал
architect (`0e4fb67`) — маршрут исполнен так, как требует `scope-guard.md`, а не «починил по
пути».

**RED-first соблюдён:** все пять RED-файлов (`4063f68`, `f9ae56a`, `ba3d6ed`) и гейт
(`41916c7`) закоммичены ДО первой строки реализации (`efe3012`). Порядок проверен логом, а
не заявлен.

**Запретный список §4 соблюдён** — предъявлено шагом I моего прогона: `crates/contracts` не
тронут, `GATEWAY_SCHEMA_VERSION` не бампнут, тело `selector_fingerprint` совпало по sha256 с
эталоном, `GATEWAY_BANDS` и `DEFAULT_MAX_RESPONSE_BYTES` не тронуты, `book`/`venue-*`/
`journal` не тронуты, reviewer-owned файлы не тронуты.

## 2. Block Done Block — мой прогон, не пересказ тестера

Прогнано МНОЮ на чистом worktree `/tmp/hft-reviewer-m86` @ `0e4fb67`. Сырой вывод — §9.
Итог: **38 PASS, 0 FAIL, `VERDICT: PASS`, exit=0**; `cargo test` по логу — `passed=1040
failed=0` (245 блоков; больше тестерских 1017/240 ровно на прогоны baseline и мутантов
внутри шагов H/H2). Числа тестера воспроизвелись.

## 3. Block-C (контрактный слой) — не применим, и это проверено, а не предположено

`git diff --name-only 0f55ea7..0e4fb67 -- crates/contracts` — пусто. Форма
`VolumeProfileRow.bins: Vec<(i64,i64)>` не меняется, `GATEWAY_SCHEMA_VERSION` остаётся `10`:
меняются ЗНАЧЕНИЯ ключей, не структура. contract-RFC не требуется (`05-contract-layer.md`
§4). Новая поверхность (`DEFAULT_VP_BIN_WIDTH_E8` + геттер/сеттер) — T2 внутри крейта-
владельца.

## 4. Риск-блок — не применим, основание названо

Диапазон не трогает `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`,
`crates/venue-*/**` (проверено `--name-only`). `gateway`/`gateway-serve` — read-only
консюмер журнала без order-egress (`VB-I-3`), путь к деньгам не задет. `risk-critic` по
`gates.md` §5 не требуется; отсутствие его вердикта блокером НЕ является и молчанием не
прикрыто.

## 5. Адверсарий ремонта H/H2 — мандат `A-035` §4 п.4

Арбитр оставил ремонт мутационного контроля на PR-гейт. Сужу его отдельно.

**Что было:** `if (cd MUT && cargo test …); then FAIL else PASS` — PASS печатался по ЛЮБОМУ
ненулевому коду, включая провал компиляции (`C-231` B3; замер `A-035` Ф-3: `101 == 101`).

**Что стало:** `scripts/lib/mutation_gate.sh::mutant_must_fail` с тремя свидетелями. Судил
его не по комментариям, а по коду:

- **W0** (`:80-92`) — baseline на НЕМУТИРОВАННОМ дереве; RED ⇒ `return 1` с диагнозом
  «мутация НЕ СУДИМА». Плацебо «оракул и так красный» закрыто.
- **W1** (`:95-103`) — `cargo test --no-run` в мутанте; провал сборки даёт FAIL с
  печатью `^error`, а не PASS. Ровно дыра `B3`.
- **W1bis** (`:106-119`) — названные соседи обязаны остаться ЗЕЛЁНЫМИ; `ok=1` без раннего
  выхода, то есть итог не теряется.
- **W2** (`:122-141`) — три РАЗЛИЧНЫХ отказа: зелёный мутант, ненулевой код без
  `test result: FAILED`, падение НЕ своим диагнозом. Возврат — `return "${ok}"`, fail-closed.

**Предъявление — мой прогон, три полных блока свидетелей** (§9): H.1 уронил `V1` своим
диагнозом («кадр прод-формы весит 4718967 Б при пределе 2000000 Б»), H.2 уронил `V2` при
ЗЕЛЁНОМ `V6` (он же W1), H2 уронил `V1` диагнозом `VB-I-10` про две строки профиля. Setup-
стражи у обеих мутаций есть и проверяют, что точка мутации существовала И изменилась;
тулчейн копии сверен с репозиторием.

**Вердикт по ремонту: ПРИНЯТ.** Форма закрывает класс, а не экземпляр: helper — единственное
место, где он закрывается один раз (`A-035` Ф-5).

## 6. Барьеры на ДЕРЕВЕ СЛИЯНИЯ — здесь нашлось то, чего не видел гейт ветки

`strict: false` на защите `main` (`gates.md` §8) означает: зелёный чек снимается на СТАРОЙ
базе. Ветка отстала от `main` на два коммита (`dff1b33`, `10cd40f` — одна строка
`docs/ROADMAP.md`), поэтому проверял на дереве слияния.

```
check_protected_artifacts   exit=0
check_gate_meta             exit=1   ← ДВЕ находки
check_docs_freeze           exit=0
check_artifact_ids          exit=0
check_roadmap_sync          exit=0   VERDICT: PASS
verify_design_claims        exit=0   VERDICT: PASS (0 нарушений)
check_context_budgets       exit=0   112960 B из 114900 B (запас 1940 B)
```

Слияние авто-мержится без конфликта; обе правки `ROADMAP` (строка 9-A/9-A-bis ветки и
12bis из `main`) сосуществуют — проверено чтением дерева слияния.

### 6.1 `FAIL` subject-lock: гейт тронут ПОСЛЕ проходного вердикта

> `A-035…md: subject-lock — после проходного вердикта (DECISION) тронут класс «гейт»:
> scripts/verify_M-86.sh`

Барьер прав по факту: `A-035` вынесен на `a96f950`, а `2b0cfd3` переписал мутационные шаги
гейта — значит DECISION судил ДРУГУЮ форму `verify_M-86.sh`. **Это не дефект ветки, а ровно
тот случай, под который арбитр и оставил мне мандат:** `A-035` §4 п.2 предписал ремонт, п.4
предписал reviewer'у судить его на PR-гейте. Я его судил — §5 выше, с прогоном.

Лок открываю ЯВНО, строкой `ALLOW-SUBJECT-CHANGE` в теле коммита этого вердикта, и называю
предел: токен — **аудит-след, а не доказательство**; он подделываем, барьер ловит ОТСУТСТВИЕ
строки, а не ложность причины. Доказательством здесь служит §5, а не токен.

**Что должен был сделать architect (`N-2`):** нести токен в теле `2b0cfd3`. Тогда находка не
всплыла бы у меня в последний момент.

### 6.2 `FAIL` «merge называет M-86, но вердикта R-*.md в дереве слияния нет»

Механическое следствие того, что вердикт пишется сейчас. Закрывается этим файлом:
`R-188` назван литералом `M-86` и введён диапазоном. Перепроверю на дереве слияния ПОСЛЕ
коммита — сырой вывод в §9.

## 7. Находки — ни одна не держит merge

**`N-1` (MINOR, зона architect).** `scripts/lib/mutation_gate.sh` — новый файл, которого нет
в `Allowed paths` §5 спеки. Полномочие есть и выше milestone'а: `A-035` §1 (маршрут, шаг 2)
называет `scripts/lib/**` явно, `scope-guard.md` отдаёт `scripts/lib/**` architect'у. Но
спека этого не отразила — **тот же класс, что `C-229` R6 в этом же предмете** (тогда не был
назван `docs/ROADMAP.md`), и написан он собственной рукой автора: «PR-time scope-гейт судит
именно заявленный список». Исправляется одной строкой §5 на close-out.

**`N-2` (MINOR, зона architect).** См. §6.1: коммит ремонта гейта не нёс
`ALLOW-SUBJECT-CHANGE`. Барьер ловит это только на дереве слияния — на ветке гейт зелёный,
и цепочка узнаёт о проблеме в самом дорогом месте.

**`N-3` (MINOR, состояние документа).** На вершине ветки шапка спеки говорит
«🚧 PLANNED (plan-time гейт не пройден)», а §Tasks держит задачи 1, 2, 4, 5, 7 в `⏳ OPEN` —
при том что код реализован, гейт зелёный и tester дал PASS. Корпусная норма (`M-71`, `M-68`)
— шапка закрывается на close-out, колонка Status ведётся по ходу (carve-out
`scope-guard.md`). Merge не держу, но документ в `main` сейчас противоречит коду; привожу в
соответствие тем же close-out'ом, что и `PROJECT-STATE`.

**`N-4` (NOTE, атомарность).** Задача #3 (POC/VAH/VAL считаются ПОСЛЕ огрубления) своего
коммита не имеет — она реализована внутри `333942f` вместе с задачей #2. Формально это
бандл; по существу задача неотделима: `compute_vp_row` получает уже огрублённый `hist`, и
отдельный коммит был бы пустым. Оракул `V4` её покрывает и зелёный. Фиксирую, не реджекчу.

**`N-5` (NOTE, конструкция).** `vp_rows()` при `w <= 1` идёт коротким путём мимо огрубления.
Для `w == 1` это тождество (`div_euclid(1)*1 == p`), для `w <= 0` — защита от паники деления
на ноль. Прод-пути сюда не приводят: `serve_config_from_env` отвергает `0` и отрицательное
отказом СТАРТА, а сеттер зовут только sacred-тесты. Замечание в том, что ветка молча даёт
тиковую сетку вместо отказа — fail-closed здесь был бы строже. Долгом не завожу: значение
недостижимо из прода.

## 8. Предъявление FA (`gates.md` §4, M-66)

Диф трогает `crates/gateway/**` и `crates/gateway-serve/**`; оба отображаются на
`docs/fa/viz-backend.md` (префиксы `VB` и `GS`). Живые инварианты, которые я ПРОВЕРЯЛ, а не
процитировал:

- **`VB-I-8`** («цены без сделок не выдумываются») — не ослаблен: единицей ряда стал
  диапазон, пустые корзины по-прежнему не вставляются; сторож — `V2`/`V3`, оба зелёные.
- **`VB-I-10`** (bounded-window, эвикция прошлой сессии) — защищён мутацией H2: отключение
  `self.vp.bins.remove(&sid)` роняет `V1` своим диагнозом (мой прогон, §9).
- **`VB-I-2`** (`live == replay`) — сетка применена в единственной точке `vp_rows()`,
  общей для снапшота, дельт и реплея; оракул `V7` гоняет живой путь через `LiveReducer`
  против независимого полного реплея.
- **`GS-I-1`** (`gateway-serve` — тонкая stateless IO-оболочка) — не ослаблен: разбор
  `GATEWAY_VP_BIN_WIDTH_E8` не добавляет состояния, сеттер зовётся один раз после успешного
  разбора; отвергнутый старт прежнего значения не меняет (`V5b`).
- **`VP-I-4`** — заведён этим предметом; барьер `check_review_fa.sh` его в `U` НЕ считает
  (префиксы только `VB`/`GS`), поэтому называю его как предмет, а не как предъявление.

`FA-WAIVER` не нужен: NO-FA крейтов (`recorder`, `derive`) диапазон не трогает. **Waiver
тестера был ошибочным** — он искал `docs/fa/gateway.md`, тогда как маппинг барьера
(`check_review_fa.sh:189-199`) отдаёт `gateway` → `docs/fa/viz-backend.md`. На вердикт это
не влияет: FA модуля существует и прочитана.

## 9. Done Block (сырой stdout; дерево `/tmp/hft-reviewer-m86`, detached `0e4fb67`)

```text
$ git rev-parse HEAD
0e4fb67a23ac7d2dda435f8f1c9a359ea8e6c229

$ git rev-parse origin/main; git merge-base HEAD origin/main
10cd40f1... (PR #197)
0f55ea7577386c08c22b4cedd428b354308db52e

$ bash scripts/verify_M-86.sh 2>&1 | grep -aE '^(PASS|FAIL|VERDICT)'
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
PASS: cargo test -p gateway --test red_vp_bin_width --quiet
PASS: cargo test -p gateway --test red_vp_bin_width_size --quiet
PASS: cargo test -p gateway --test red_vp_bin_width_governed --quiet
PASS: cargo test -p gateway-serve --test red_vp_bin_width_startup --quiet
PASS: cargo test -p gateway-serve --test red_vp_bin_width_bridge --quiet
PASS: обязательство v1_ … v8b предъявлено            (12 строк, все PASS)
PASS: red_vp_bin_width_startup.rs несёт 10 тестов (ожидается 10)
PASS: docker-compose.yml объявляет GATEWAY_VP_BIN_WIDTH_E8 с дефолтом
PASS: дефолт ручки равен подписанному литералу (25000000 e8 = 0.25 USD)
PASS: docs/fa/viz-backend.md вообще называет VP-I-4
PASS: VP-I-4 в FA описывает корзину профиля как ДИАПАЗОН, а не как точку
PASS: H тулчейн копии совпал с репозиторием (cargo 1.97.0 (c980f4866 2026-06-30))
PASS: H.1 мутация (сетка = тик) уронила V1 своим диагнозом
PASS: H.2 V2 упал своим диагнозом, V6 остался зелёным
PASS: H2 мутация (эвикция отключена) уронила V1 диагнозом про две строки профиля
PASS: crates/contracts не тронут (T1 не меняется)
PASS: GATEWAY_SCHEMA_VERSION не бампнут (форма выдачи не меняется)
PASS: тело selector_fingerprint не тронуто (sha256 e99e808bbce3b2f98a24bc3317230e7896439f4e39616c8ee3e54cb2ec44e181)
PASS: GATEWAY_BANDS не тронут (состав выдачи — граница C, чужой предмет)
PASS: подписанный предел П-020 не сдвинут
PASS: книга, venue-адаптеры и журнал не тронуты
PASS: reviewer-owned файлы не тронуты
PASS: сторож тела fingerprint РАЗЛИЧАЕТ мутацию (внедрённый hash ширины меняет sha256)
PASS: сторож дефолта РАЗЛИЧАЕТ выключающее значение (:-2 отвергается литералом 25000000)
VERDICT: PASS
verify-exit=0

$ grep -aE '^test result' <лог того же прогона> | awk '{p+=$4; f+=$6} END {print p, f, NR}'
passed=1040 failed=0 (блоков: 245)

$ grep -aE '^  \[W[012]' <лог того же прогона>
  [W0] baseline GREEN: test result: ok. 1 passed; 0 failed; … finished in 31.63s
  [W1] мутант собрался: Finished `test` profile … in 6.53s
  [W2] упал своим диагнозом: V1 НАРУШЕН: кадр прод-формы весит 4718967 Б при подписанном пределе 2000000 Б (2.36×)…
  [W0] baseline GREEN: test result: ok. 1 passed; 0 failed; … finished in 0.01s
  [W1] мутант собрался: Finished `test` profile … in 0.27s
  [W1bis] v6_grid_aligned_input_passes_through_unchanged в мутанте GREEN: test result: ok. 1 passed…
  [W2] упал своим диагнозом: V2 НАРУШЕН: пять цен в двух корзинах шириной 1 дали 3 корзин…
  [W0] baseline GREEN: test result: ok. 1 passed; 0 failed; … finished in 31.73s
  [W1] мутант собрался: Finished `test` profile … in 6.52s
  [W2] упал своим диагнозом: V1 НАРУШЕН (`VB-I-10`): в кадре 2 строк профиля вместо одной…
```

```text
# ДЕРЕВО СЛИЯНИЯ /tmp/hft-rev-m86-MP (база=main, влита ветка; 49a0774 parents=10cd40f 0e4fb67)
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) bash scripts/<барьер>.sh
check_protected_artifacts  exit=0   OK: защищённые артефакты целы на HEAD
check_gate_meta            exit=1   FAIL ×2 → §6.1 (ALLOW-SUBJECT-CHANGE) и §6.2 (этот файл)
check_docs_freeze          exit=0
check_artifact_ids         exit=0
check_roadmap_sync         exit=0   VERDICT: PASS
verify_design_claims       exit=0   VERDICT: PASS (0 нарушений)
check_context_budgets      exit=0   VERDICT: PASS — 112960 B из 114900 B (запас 1940 B)
```

## 10. Что делаю дальше (close-out, зона reviewer)

1. merge через PR (`gh pr create` → `gh pr checks --watch` → `gh pr merge --merge
   --delete-branch`); решение — по коду возврата, не по тексту.
2. **Деплой-гейт `gates.md` §8 — обязателен и решающий:** дождаться CI+Deploy, проверить
   VPS глазами и **измерить РАЗМЕР КАДРА на прод-форме**. Зелёные юниты `TD-206` не
   закрывают: предмет долга — то, что кадр не доезжает до клиента НА ПРОДЕ.
3. `PROJECT-STATE.md` + `TECH-DEBT.md`: `TD-206` закрывается ТОЛЬКО по прод-пруфу; §8 спеки
   («пределы, названные честно») даёт кандидатов в долг — отсутствие fail-closed гарантии
   размера кадра, `W` подобран под один инструмент, состояние остаётся тиковым, Value Area
   плохо определена. Завожу их своим номером, а не растворяю в прозе.
4. Колонка «Состояние» в `docs/ROADMAP.md` (строка 9-A) — моя зона по `04-workflow.md`
   §Close-out; перечень и порядок остаются за architect'ом.
5. `bash scripts/gc_worktrees.sh` + снос своих деревьев.

**ВЕРДИКТ: APPROVED.** Merge разрешён.
