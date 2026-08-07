# R-041 — PR-гейт M-60a (замок процессного слоя)

**Роль:** reviewer · **Дата:** 2026-08-07 · **Ветка:** `feat/M-60-mechanisms`
**Предмет ревью:** хэндофф тестера M-60a task #1, заявленный HEAD `b3ba11a`
**Фактически проверенный HEAD:** `705b875` (вершина `origin/feat/M-60-mechanisms`)
**Предыдущие артефакты цепочки:** `C-065`, `C-066`, `C-067` (REJECT), `C-068` (NOTE),
`A-005` (арбитраж), `R-038-M-60a-arbiter-trigger.md`

---

## Вердикт

| предмет | вердикт |
|---|---|
| **Механизм M-60a (задачи 1–4) по существу** | **APPROVED** — три гейта зелёные, воспроизведены независимо |
| **Push ветки `feat/M-60-mechanisms` в `main`** | **BLOCKED** — F-1, F-2 |
| **Итог** | **CHANGES REQUESTED** — предмет годен, носитель не готов |

Разделение намеренное. Барьер, проба и проводка сделаны хорошо: батарея из девяти дырявых
реализаций красная поимённо, шаг W перешёл с разбора текста на ИСПОЛНЕНИЕ, самореференция на
вершине содержательна. Блокируется не качество M-60a, а то, что ветка несёт сверх него.

---

## §A — Расхождение по HEAD (зафиксировано до начала работы)

Мандат называет `b3ba11a`. `origin/feat/M-60-mechanisms` на момент ревью = `705b875`:

```
$ git log --oneline b3ba11a..705b875
705b875 docs(M-60a): task #3 — норма §11, замок процессного слоя записан в gates.md [architect]
$ git log --oneline 705b875..b3ba11a
(пусто)
```

`b3ba11a` — предок. После прогона тестера architect добавил задачу #3 (11:05Z). Ревью веду по
`705b875`: в `main` уедет он, а не то, что гонял тестер. Это же расхождение делает возможной
находку **F-3**.

## §B — Done Block (сырой stdout, чистый worktree `/tmp/rev-M60a` @ `705b875`)

```
$ git worktree add /tmp/rev-M60a origin/feat/M-60-mechanisms
Preparing worktree (detached HEAD 705b875)
HEAD is now at 705b875 docs(M-60a): task #3 — норма §11, замок процессного слоя записан в gates.md

$ bash scripts/tests/red_docs_freeze.sh ; echo exit=$?
  (27 строк PASS: A1M A1A A1D A1R L1 L2 A2P L3 A3S A3C A3E1 A3B A3E A3NP A3EM A3RP L4
   A4G A4W A4C A4F A4Z L5 A5S A5C A5P A5Q)
PASS  МАНИФЕСТ ⇄ исполнение: 27 сценариев, состав совпал в обе стороны
PASS  СПЕКА⇄МАНИФЕСТ: 30 пар (ось,вид,значение) совпали в обе стороны
PASS  §3bis.3(2): у каждой из осей (1 2 3 4 5) есть легитимный сценарий
VERDICT: PASS (27/27) — все значения пяти осей покрыты, состав сверен со спекой
exit=0

$ bash scripts/tests/red_docs_freeze.sh --battery ; echo exit=$?
PASS  эталон → exit=0  VERDICT: PASS (27/27)
PASS  showgrep → exit=1  VERDICT: FAIL (3)  [ось 5 / содержимое файла]
PASS  subjtok → exit=1  VERDICT: FAIL (1)  [ось 5 / subject]
PASS  earlytok → exit=1  VERDICT: FAIL (1)  [ось 3 / токен раньше]
PASS  existsbase → exit=1  VERDICT: FAIL (1)  [ось 3 / база не-предок]
PASS  treediff → exit=1  VERDICT: FAIL (2)  [ось 3 / evil merge]
PASS  overbroad → exit=1  VERDICT: FAIL (1)  [ось 4 / нет легитимного сценария]
PASS  quotedpath → exit=1  VERDICT: FAIL (1)  [ось 4 / квотируемое имя члена зоны]
PASS  always0 → exit=1  VERDICT: FAIL (22)  [вырожденный — пропускает всё]
PASS  always1 → exit=1  VERDICT: FAIL (5)  [вырожденный — блокирует всё]
PASS  без барьера → exit=1, «SETUP НЕ СОСТОЯЛСЯ» (страж на месте)
BATTERY: PASS (11/11) — эталон зелён, все мутанты красные, страж жив
exit=0

$ set -o pipefail; bash scripts/verify_M-60a.sh 2>&1 | tail -40; echo exit=$?
PASS  A scripts/check_docs_freeze.sh на месте и парсится
PASS  A пустая база: fail-closed
PASS  F проба зелёная: VERDICT: PASS (27/27)
PASS  F2 BATTERY: PASS (11/11)
PASS  S собственный диф проходит замок (токен founder'а на месте)
PASS  W самопроверка оракула: 6 фикстур классифицированы верно
PASS  W предусловие 1: scripts/check_docs_freeze.sh ИСПОЛНЯЕТСЯ джобом(ами): docs-freeze
PASS  W предусловие 1: scripts/tests/red_docs_freeze.sh ИСПОЛНЯЕТСЯ джобом(ами): docs-freeze
PASS  W предусловие 2: docs-freeze в ключе status-check.needs
PASS  W предусловие 3 (ИСПОЛНЕНИЕМ): guard падает при result=failure и выходит нулём при всех success
PASS  P VERDICT: PASS (20/20)
PASS  T fmt
PASS  T clippy
PASS  T cargo test --all
VERDICT: PASS
exit=0
```

**Три прогона тестера подтверждаю: 27/27, 11/11, VERDICT: PASS, все exit=0.** Пересказа нет,
цифры сошлись с заявленными в хэндоффе.

## §C — Block-scope

| зона | факт | вердикт |
|---|---|---|
| dev (`5a4d885`, `363cf1b`, `b3ba11a`) | `scripts/check_docs_freeze.sh`, `.github/workflows/ci.yml` | ✅ ровно `Allowed paths` §2 |
| architect | `milestones/M-60*.md`, `scripts/tests/red_*.sh`, `scripts/verify_M-60*.sh`, `gates.md` §11 | ✅ в зоне |
| `crates/**` | не тронуты | ✅ Forbidden соблюдён |
| `scripts/check_protected_artifacts.sh` | не тронут; регресс P 20/20 | ✅ соседний барьер цел |
| тесты sacred | dev не правил ни один `*/tests/`, ни `red_*.sh` | ✅ RED-first не нарушен |
| атомарность | задача 1 → `5a4d885`+`b3ba11a`, задача 2 → `363cf1b`, задача 3 → `705b875` | ✅ бандлов нет |

**Block-C (контракты):** `crates/contracts/**` не тронут, `Contract impact: нет` — соответствует.
**Block-risk:** `crates/{risk,killswitch,oms,venue-*}` не тронуты. risk-critic не требуется.

---

## §D — Находки

### F-1 · BLOCKER · ветка несёт в `main` нереализованный RED (gates.md §8)

`feat/M-60-mechanisms` — зонт всего M-60, а зелён на ней только M-60a. Замер на `705b875`:

```
$ bash scripts/tests/red_context_budgets.sh ; echo exit=$?
SETUP НЕ СОСТОЯЛСЯ: барьера нет: scripts/check_context_budgets.sh
exit=1
$ bash scripts/tests/red_gate_meta.sh ; echo exit=$?
SETUP НЕ СОСТОЯЛСЯ: барьера нет: scripts/check_gate_meta.sh
exit=1
$ bash scripts/verify_M-60.sh          # зонтичный гейт
FAIL  A check_context_budgets.sh отсутствует или не парсится
FAIL  A check_gate_meta.sh отсутствует или не парсится
FAIL  B .claude/rules = 964 строк, бюджет 725 — превышение на 239
FAIL  B CLAUDE.md = 100 строк, бюджет 70 — превышение на 30
FAIL  C red_context_budgets КРАСНАЯ · FAIL  G red_gate_meta КРАСНАЯ
FAIL  W check_context_budgets / check_gate_meta / verify_design_claims НЕ зовутся из ci.yml
FAIL  W пробы red_context_budgets / red_gate_meta / red_verify_design_claims НЕ в ci.yml
                                        (11 FAIL)
```

`gates.md` §8: «**RED до реализации не живёт в `main`** (main всегда зелёный). Два
санкционированных пути: держать RED-коммиты локально до GREEN, либо feat-ветка `feat/M-NN`,
которую reviewer мержит **уже зелёной**». Ветка зелёной не является.

Возражение «CI этих скриптов не зовёт, поэтому `main` останется зелёным» — верное по факту и
негодное по существу: правило запрещает не красный CI, а именно RED-оракул нереализованной
работы в `main`. Ровно это и уедет — вместе со спеками M-60b/M-60c и зонтичным гейтом, который
их и валит. Плюс `verify_M-60.sh` шаг B фиксирует превышение бюджета `.claude/rules` на 239
строк — а `705b875` его ещё и увеличивает.

**Что нужно:** merge только подмножества M-60a. Хирургия истории (ветка/cherry-pick/rebase) —
решение architect'а, не моё: reviewer описывает, architect проектирует (`gates.md` §4).

### F-2 · BLOCKER · номер `R-038` занят ТРЕМЯ разными документами

```
origin/main                   :: research/reviews/R-038-branch-hygiene.md
origin/docs/M-61-artifact-ids :: research/reviews/R-038-branch-hygiene.md
origin/feat/M-62-segment-metadata :: research/reviews/R-038-branch-hygiene.md
origin/feat/M-59-lifetime-memory  :: research/reviews/R-038-M-59.md
origin/feat/M-60-mechanisms       :: research/reviews/R-038-M-60a-arbiter-trigger.md
```

Три РАЗНЫХ документа под одним номером на ветках, сходящихся в один `main`. После merge'а
`R-038` перестаёт быть адресом: `TECH-DEBT.md` TD-110 ссылается на «`R-038` §G» (гигиена
веток), `milestones/M-60a-docs-freeze.md` §3bis — на «разбор `R-038`» (триггер арбитра). Одна
и та же строка укажет на разные документы в зависимости от того, кто читает.

Это **живая материализация TD-111**, заведённого мной вчера ровно на этот класс. Долг был
записан как риск; сегодня он предъявлен фактом на трёх ветках сразу. Свой вердикт нумерую
`R-041`: `R-039`/`R-040` заняты `feat/M-57-task5` (проверено по всем `origin/*`).

**Что нужно:** перенумеровать до merge'а — дешевле, чем после (аргумент TD-111, проверенный на
TD-109→110). Кому — architect, вместе с F-1.

### F-3 · MAJOR · шаг S не имеет setup-guard'а и на `b3ba11a` прошёл ВАКУУМНО

Хэндофф тестера утверждает: «S самореференция пройдена (**токен в коммитах milestone'а на
месте**)». Замер на `b3ba11a` — том самом HEAD, что гонял тестер:

```
$ git worktree add /tmp/rev-M60a-b3 b3ba11a
коммитов диапазона:                       29
коммитов, ТРОГАЮЩИХ зону замка:            0
коммитов с токеном FOUNDER-APPROVED:       0
$ EVENT_NAME=push PUSH_BEFORE=$(git merge-base origin/main HEAD) bash scripts/check_docs_freeze.sh
exit=0
```

**Токена не было ни одного.** Задача #3 (`705b875`), которая его принесла, на тот момент не
существовала. S вернул PASS не потому, что замок пропустил правку с разрешением, а потому что
проверять было нечего — и напечатал при этом «токен founder'а на месте», то есть УТВЕРЖДЕНИЕ,
которое было ложным.

Это ровно тот дефект, против которого написан `testing.md`: «Setup-guard на КАЖДЫЙ сценарий:
проба, молча тестирующая не тот сценарий, — плацебо самой себя» и свойство 4 целостности гейта
— «наблюдает ОТСУТСТВИЕ, не только сбой». Milestone потратил четыре REJECT'а и арбитраж на
искоренение этого класса в пробе — и оставил его в собственном acceptance-скрипте.

На `705b875` шаг S содержателен (1 коммит зоны, 1 токен — проверено покоммитно), поэтому
кандидат на merge не пострадал. **Дефектен оракул, а не предмет.** Нужен guard: «в диапазоне
есть ≥1 коммит, трогающий зону; иначе S не PASS, а SKIP/FAIL». Зона — architect
(`scripts/verify_*.sh` sacred).

### F-4 · MAJOR · находка тестера №1 ЗАСЧИТАНА: шаблон Done Block предписывает запрещённый анти-паттерн

Тестер прав, и находка сильнее, чем он её заявил. Дело не в общем «шаблоне §D», а в конкретном
тексте `.claude/rules/commit-discipline.md`, секция «Сырой ≠ ВЕСЬ»:

```
verify:  bash scripts/verify_M-NN.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
clippy:  ... 2>&1 | tail -5; echo exit=$?
```

`$?` после конвейера — код ПОСЛЕДНЕГО звена. У `tail` он всегда 0. У `grep` он 0, когда строка
НАЙДЕНА — **включая `VERDICT: FAIL`**. То есть красный гейт печатается как `exit=0`.

`gates.md` §3 запрещает эту форму поимённо и теми же словами: «`gate | grep ... && commit &&
push` (`grep` возвращает 0, когда НАШЁЛ строку — в том числе `VERDICT: FAIL`, и красное уезжает
в `main`)». **Два файла правил противоречат друг другу: §3 запрещает конструкцию, которую
шаблон Done Block предписывает.** И правило требует «каждое утверждение подкреплено командой и
её exit-кодом» — а шаблон печатает не тот exit-код.

На M-60a не выстрелило: гейт был зелёным независимо. Но гейт, чей шаблон верен только когда
ответ и так «pass», — не гейт. Именно поэтому я гонял verify через `set -o pipefail` (см. §B).

**Кому.** Правка `.claude/rules/commit-discipline.md` — зона architect'а, и теперь она **внутри
замка M-60a**: коммит обязан нести `FOUNDER-APPROVED`. Изменение ФОРМЫ правила → critic по
`gates.md` §9. Долг — **TD-112** ниже. Первое реальное применение замка к процессу — уместное.

### F-5 · MINOR · норма §11 = 12 строк при спеке «≤10», спека не обновлена

`milestones/M-60a-docs-freeze.md` §3, задача 3: «Норма `gates.md` §11 (**≤10 строк**)».
Фактически 12 (тело коммита `705b875` это и признаёт: «СОДЕРЖАНИЕ НОРМЫ (12 строк…)»). Число
осталось в спеке неисправленным — тот же класс «литерал живёт отдельно от предмета», который
спека сама разбирает в §4 (rev2: «12» при 13 при пороге «≥11»). Содержание нормы возражений не
вызывает; расходится только заявленный бюджет. Плюс `verify_M-60.sh` шаг B уже красен по
бюджету `.claude/rules` (964 против 725), и §11 добавляет к превышению.

### F-6 · INFO · `verify_M-60a.sh` не исполняем

```
100755 scripts/check_docs_freeze.sh
100755 scripts/tests/red_docs_freeze.sh
100755 scripts/check_protected_artifacts.sh
100644 scripts/verify_M-60a.sh          <- один из всех
```

Работе не мешает (зовётся `bash scripts/…`), но расходится с соседями. Шаг A проверяет
исполнимость барьера и не проверяет собственную.

---

## §E — Решение по двум находкам тестера

**Находка 1 (шаблон съедает exit).** **Засчитана**, повышена до MAJOR, переадресована:
предмет — не §D хэндоффа, а `commit-discipline.md`. См. **F-4** и **TD-112**. Писать фикс —
architect (зона правил), с токеном founder'а (зона замка) и через critic (`gates.md` §9,
изменение формы). Critic на шаблон отдельно созывать не нужно: он приходит триггером на саму
правку.

**Находка 2 (флак TD-098 красит verify_M-60a).** **Нового долга НЕ завожу.** `TD-098` уже
существует, уже MAJOR и уже называет механизм точно (глобальный `#[global_allocator]` против
параллельного раннера, направление сноса — к ложному GREEN). Заводить второй номер на тот же
предмет — ровно то, за что заведён TD-111.

Что здесь действительно ново — **радиус поражения**, и он в TD-098 не записан: паритет с CI
(`gates.md` §3) обязывает шаг T КАЖДОГО `verify_M-NN.sh` гонять `cargo test --all`. Значит
флакающий оракул `crates/gateway` красит гейт **docs-only** milestone'а, не имеющего к
`gateway` никакого отношения. Дописываю это в TD-098 как замеренное следствие; severity
остаётся MAJOR, направление фикса не проектирую (`gates.md` §4: reviewer описывает, architect
проектирует). Разделение инвариантов — предмет спеки architect'а, не вердикта reviewer'а.

---

## §F — Что заводится в TECH-DEBT

- **TD-112** (MAJOR) — шаблон Done Block предписывает конструкцию, запрещённую `gates.md` §3
  (F-4).
- **TD-113** (MAJOR) — шаг S `verify_M-60a.sh` без setup-guard'а, вакуумный PASS (F-3).
- **TD-098** — дополнен радиусом поражения (находка тестера 2).
- **Задача #5 спеки** (branch protection недоступен, 403 private+free) — заводится при
  close-out, то есть ПОСЛЕ снятия F-1/F-2. Формулировка готова в спеке §3ter.1; три опции —
  граница C, решает founder.

## §G — Условие APPROVED

1. F-1 снят: в `main` идёт подмножество M-60a без `red_context_budgets.sh`,
   `red_gate_meta.sh`, `verify_M-60.sh`, спек M-60b/M-60c — либо эти механизмы доведены до
   зелёного.
2. F-2 снят: `R-038` на этой ветке перенумерован (свободен `R-041`+, `R-039`/`R-040` заняты
   `feat/M-57-task5`).
3. F-3 и F-4 — заведены долгом, merge не блокируют.
4. F-5 — правка одной строки спеки, по пути.

Push в `main` **не сделан**. `gates.md` §8 (пост-merge деплой-гейт) не применяется: merge'а
нет. Прод не тронут.

---

## §H — Done Block вердикта

```
$ git status --porcelain
(пусто, кроме добавляемого вердикта)

$ bash scripts/tests/red_docs_freeze.sh            → VERDICT: PASS (27/27)   exit=0
$ bash scripts/tests/red_docs_freeze.sh --battery  → BATTERY: PASS (11/11)   exit=0
$ set -o pipefail; bash scripts/verify_M-60a.sh    → VERDICT: PASS           exit=0
$ bash scripts/verify_M-60.sh                      → 11 FAIL                 exit=1
$ bash scripts/tests/red_context_budgets.sh        → SETUP НЕ СОСТОЯЛСЯ      exit=1
$ bash scripts/tests/red_gate_meta.sh              → SETUP НЕ СОСТОЯЛСЯ      exit=1
```

**Итог: механизм M-60a — APPROVED. Push ветки — BLOCKED (F-1, F-2).**
