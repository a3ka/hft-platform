<!-- GATE-META
milestone: TD-222
audited_repo: a3ka/hft-platform
audited_base: 939d1b91df081731cb114dab956a8af60f93cf2c
audited_head: ff93de128a2d70aef142b589dbc71a70ca0aad98
verdict: DECISION
-->

# A-041 — TD-222, ESCALATE `C-259`: достижимость вызова пробы агрегата — сменить ОСНОВАНИЕ, а не чинить разбор

**Предмет и предел.** Арбитраж по `gates.md` §0 (триггер п.2: третий круг по одному предмету —
`C-257` REJECT → `C-258` REJECT → `C-259` ESCALATE). Предмет — ветка
`origin/harness/ci-aggregate-all-needs` (PR #232), вершина взята командой: `ff93de1` = вердикт
`C-259` поверх `116ac46`; `git diff --name-status 116ac46..ff93de1` — только
`research/critiques/C-259-*.md`, то есть судимая критиком ревизия и вершина совпадают по
предмету. База — `939d1b9`. Маршрут — харнесс-трек (`docs/workflow/harness-track.md`): предмет
не исполняется прод-процессом, не трогает нормы, не касается журнала/позиции/денег. Границы C
нет — спор инженерный. Живого инварианта `docs/fa/*` у предмета нет (диапазон не трогает
`crates/**`): `FA-WAIVER: harness-only — диапазон 939d1b9..ff93de1 не содержит crates/**`.

Сужу РОВНО пять вопросов мандата: (а) воспроизводится ли R4 и какие ещё формы обходят;
(б) сходится ли класс при уточнении разбора и какое основание сходится; (в) цена каждого
варианта; (г) блокирует ли R4 merge по существу; (д) что делает ведущий architect дальше.
**НЕ переоткрываю:** `C-257` R1/R2 и `C-258` R3 закрыты и проверены критиком; форму
рукописного условия (отказ от `toJSON(needs)`) не пересматриваю.

---

## 1. Замер (а) — R4 воспроизводится, и это КЛАСС, а не форма

Стенд: каждая форма — мутация `status-check` реального `ci.yml` вершины, затем ДВЕ колонки:
`checker` — exit `python3 scripts/deploy_catchup.py check-aggregate` на мутанте (0 = принял);
`reach`/`step_rc` — исполнение `run` шага пробы формой раннера
(`bash --noprofile --norc -eo pipefail -c`) в песочнице, где по пути
`scripts/tests/red_ci_aggregate.sh` лежит стаб `touch marker; exit 1`. `reach=1` — до пробы
дошли; `step_rc≠0` — её провал дошёл до джоба. ЛАЗЕЙКА = checker принял, а раннер либо не
дошёл, либо проглотил провал. Сырой вывод — Done Block §5.

| форма шага пробы | checker | reach | step_rc | итог |
|---|---|---|---|---|
| честная `bash scripts/tests/red_ci_aggregate.sh` | 0 | 1 | 1 | эталон |
| **R4** `true \|\| P` | 0 | 0 | 0 | **ЛАЗЕЙКА** (`C-259`) |
| `P \|\| echo failed` | 0 | 1 | 0 | **ЛАЗЕЙКА** — суффикс не в `_SWALLOW_SUFFIXES` |
| `P \|\| exit 0; echo done` | 0 | 1 | 0 | **ЛАЗЕЙКА** — глушитель в середине строки |
| `set +e` / `P` / `true` (три строки) | 0 | 1 | 0 | **ЛАЗЕЙКА** |
| `cat <<EOF` / `P` / `EOF` | 0 | 0 | 0 | **ЛАЗЕЙКА** — heredoc |
| `P &` | 0 | 1 | 0 | **ЛАЗЕЙКА** — фон, код не ждётся |
| `set +o pipefail` / `P \| cat` | 0 | 1 | 0 | **ЛАЗЕЙКА** |
| `if: github.event_name == 'never'` на шаге | 0 | — | — | **ЛАЗЕЙКА по семантике `if`** (не литерал `false`; стенд выражений не вычисляет) |
| `working-directory: fixtures` (+ подложная проба там) | 0 | 0 | 0 | **ЛАЗЕЙКА** |
| `timeout-minutes: 0` | 0 | — | — | не моделируется (назван `C-259`) |
| `false && P` | 1 | 0 | 1 | fail-closed |
| `X=bash; $X …`, `eval "P"`, `(P) \|\| true`, `! P`, `while false`, `if [ -e /nonexistent ]` | 1 | — | — | fail-closed — **случайно**: `_invokes` их не распознаёт, а не потому что понимает |
| `if: ${{ false }}`, `continue-on-error`, `shell: python`, `uses:` вместо `run` | 1 | — | — | fail-closed |
| `env: CI_AGG_YML=/dev/null` на шаге | 1 | 1 | 1 | fail-closed — **случайно**: A3 исполняет склейку ВСЕХ `run` `status-check` на корне репо, проба реально запускается на `/dev/null` и роняет «зелёную» модель |
| лишний шаг ДО пробы: шим `bash` через `$GITHUB_PATH` | 1 | 0 | 0 | fail-closed — **случайно**: та же склейка A3 исполнила шим-шаг в моём дереве (остался каталог `shim/`, удалён) и упала на `>> ""` |

Три вывода из таблицы.

1. **R4 подтверждён, класс шире на порядок:** восемь исполняемых лазеек плюс одна по семантике
   `if`. `C-259` назвал одну и правильно потребовал не «ещё форму», а «fail-closed определение
   достижимого вызова».
2. **Половина «fail-closed» исходов — не защита, а побочный эффект.** `_invokes` не понимает
   `$X`/`eval`/скобки — и потому отказывает; A3 исполняет склейку всех шагов на живом дереве —
   и потому спотыкается о шим и `env`. Оба свойства не спроектированы и не пиннятся ни одним
   сценарием: измени `_invokes` «в лучшую сторону» (научи скобкам) — и `(P) || true` станет
   лазейкой. Это признак того, что основание выбрано неверно, а не что «парсер надо дочинить».
3. **Класс — не новый и не внесён этим PR.** `_invokes` и `_SWALLOW_SUFFIXES` живут в
   `origin/main` (`C-096`, строки `612`/`571` — Done Block) и обслуживают A1 и A6 —
   проверку того, что `deploy-catchup` зовёт барьер и что барьер зовёт САМ СЕБЯ. Замер:
   `true || python3 scripts/deploy_catchup.py check-aggregate` → `check-aggregate` exit 0;
   `… check-aggregate || echo x` → exit 0; `true || bash scripts/tests/red_deploy_catchup.sh`
   → exit 0. Architect в `116ac46` ПЕРЕИСПОЛЬЗОВАЛ хелпер с честно названным в докстринге
   пределом (`eval`/переменная) для проверки, где предел решает. Формулировка «внёс дыру» —
   неверна; верна «применил открытое основание там, где нужно закрытое».

## 2. Решения

### (б) Класс НЕ сходится при уточнении разбора — критик прав; основание — ЗАКРЫТАЯ ФОРМА гейт-джобов

`_invokes` — чёрный список: перечень разделителей команд + перечень глушащих суффиксов.
Каждая форма из §1 добавила бы строку в перечень, и ряд не имеет конца: синтаксис оболочки
(перенаправления, группировка, фоновые задачи, `set`, heredoc, функции, `trap`, `exec`)
открыт, а сверх него — ключи шага GitHub (`if` с выражением, `working-directory`, `env`,
`shell`, `timeout-minutes`) и структура джоба (лишний шаг до пробы, `with:` у checkout,
`defaults`). Это тот же класс, что `A-040` Q2: текст — ВХОД вычисления (здесь — bash-раннера),
читать вход и заявлять результат можно, только повторив вычисление; а повторить его локально
(с `$GITHUB_PATH`, `if`-выражениями, `timeout-minutes`) нельзя.

**Сходится белый список над объявленным контрактом формы** (`A-040` Q2, второй абзац): не
«распознать вызов в произвольном тексте», а «джоб остаётся в форме, где ошибиться нельзя, и
всё вне формы — FAIL с именем места». Прототип-замер (§5, `closed_form.py` — не продукт):

- **`status-check`**: ключи джоба ⊆ `{name, runs-on, needs, if, steps}`;
  `name == "All checks passed"` (имя, на котором стоит защита ветки), `runs-on == ubuntu-latest`,
  `if == always()`; шагов РОВНО три и в этом порядке: условие (ключи ⊆ `{name, run}`, `run`
  содержит `needs.`; тело исполняют A3 и `red_ci_aggregate.sh`), `{uses: actions/checkout@v4}`
  РОВНО (без `with`/`env`/`if`), проба (ключи ⊆ `{name, run}`, `run` после нормализации
  пробелов `" ".join(run.split())` РАВЕН `bash scripts/tests/red_ci_aggregate.sh`).
- **`deploy-catchup`**: ключи ⊆ `{name, runs-on, steps}`; шагов РОВНО шесть в порядке:
  checkout РОВНО, затем пять командных шагов (ключи ⊆ `{name, run}`) с `run`, равными
  `python3 scripts/deploy_catchup.py check-wiring` · `python3 scripts/check_deploy_gate.py` ·
  `python3 scripts/deploy_catchup.py check-aggregate` · `bash scripts/tests/red_deploy_catchup.sh`
  · `bash scripts/tests/red_deploy_catchup.sh --battery`.

Прогон прототипа: три честные формы (вершина; `run: |` с переводом строки; `name` у шага
условия) — PASS; **все 39 форм обхода** из §1 и из `red_deploy_catchup.sh` A4/A6/A7/A8/A9/A10/
A11/A14 — FAIL с названным местом (`status-check.steps[2].run = … ≠ канон`,
`лишние ключи шага ['if']`, `шагов 4, канон 3`, `deploy-catchup.steps[3].run = 'true || …'`);
`ci.yml` с `origin/main` — FAIL `шагов 1, канон 3` (пробы там нет — ровно то, что A4
обязан видеть). Сырой вывод — §5.

Почему форма обязана покрывать ВЕСЬ джоб, а не только шаг пробы (кандидат (1) мандата в
узкой редакции): шим `bash` в `$GITHUB_PATH` шагом ДО пробы, `with: ref:` у checkout,
`defaults.run.shell`/`env` на уровне джоба обходят проверку одного шага при идеальной строке
`run`. Список ключей шага и джоба у GitHub конечен — потому белый список по ключам замкнут,
в отличие от списка форм bash.

**Кандидаты мандата — решение по каждому.**

| кандидат | решение | причина |
|---|---|---|
| (1) шаг пробы = ровно одна команда + белый список | **ПРИНЯТ, расширен до закрытой формы ДВУХ джобов** | одна строка `run` не защищает от шага-шима, `with:` у checkout, ключей джоба (§1, j1/j2/j4–j6) |
| (2) маркер результата в логе прогона | ОТВЕРГНУТ | локально не пробуется — проба без оракула (`harness-track.md` §5 п.1–2 неисполнимы) |
| (3) проба отдельным джобом в `needs` | ОТВЕРГНУТ | рекурсия: провал джоба-пробы держит merge только через условие, которое проба и охраняет; вопрос «а зовётся ли проба внутри нового джоба» никуда не делся — он переехал |
| (E) исполнение `run` со стаб-пробой (маркер + распространение кода) | ОТВЕРГНУТ как основание | не видит пре-команд, меняющих мир пробы: `sed` по файлу пробы, `export CI_AGG_YML=…`, `git checkout <старый> -- проба` перед честным вызовом — маркер есть, код доходит, проба другая. Годится лишь как позитивный контроль, а его уже даёт сам CI, исполняя шаг |
| (4) признать предел | **ПРИНЯТ для терминального звена, НЕ для R4** | см. ниже |

**Терминальное звено — предел назван, а не изображён закрытым.** `check-aggregate` вуча́ет за
собственный вызов (A6). Если `deploy-catchup` обезврежен, A6 не исполняется, и всё, что он
проверяет, — вакуум. Внутри `ci.yml` эту рекурсию не замкнуть ничем: любой сторож сторожа
сидит в том же файле. Замыкается она ВНЕ файла — чтением диффа `ci.yml` адверсарием/ревьюером
(закрытая форма делает дифф гейт-джобов коротким и однозначным: любое отклонение от канона —
одна красная строка барьера, которую видно и без чтения) и обязательным чеком защиты ветки.
Это `COGNITIVE-ONLY`, и записывается в докстринг канона в `deploy_catchup.py` явно. TD-карта на
этот предел не заводится: он — граница класса «сторож в том же файле, что предмет», а не
дефект реализации.

### (в) Цена — что ломает, что ловит, что нет

**Ломает:** ничего из действующих проб. Замер прототипом на существующих мутантах
`red_deploy_catchup.sh`: A0 (честный) PASS; A4/A6/A7/A8/A9/A10/A11/A14 FAIL. Условие A1/A2/A5
проверяется A3 исполнением как прежде — форма шага условия не пиннит текст, только ключи и
наличие `needs.`. `red_review_fa.sh` W8 смотрит текст `status-check` — не трогается.
`red_ci_aggregate.sh` — не трогается (`ci.yml` не меняется: форма вершины УЖЕ канонична, замер
«00 честная ветка» PASS). `--battery` покрывает только `decide` (`red_deploy_catchup.sh:468-541`)
— не задевается.

**Стоит:** любая честная правка `status-check`/`deploy-catchup` (новый шаг, аргумент, версия
checkout) требует правки канона в `deploy_catchup.py` и сценария пробы на новую форму. Цена
намеренная: гейт merge'а меняется редко и обязан меняться осознанно; расширение канона —
единственная разрешённая правка, и только с пробой (правило заморозки `A-040` Q3 п.2).

**Не ловит и не обещает:** содержимое файлов (`red_ci_aggregate.sh`, заменённый на `exit 0`;
`deploy_catchup.py`, отредактированный) — это зона адверсария и ревью диффа, как у любого
барьера; семантику тега `actions/checkout@v4` и раннера (`ubuntu-latest`); терминальное звено
A6 (выше).

**Побочная чистка, обязательная:** `_invokes` после переписывания не имеет вызывающих —
удалить; мёртвый хелпер с честным докстрингом о пределах — ловушка для следующего критика.
`_step_is_disarmed`/`_SWALLOW_SUFFIXES` остаются ТОЛЬКО для `check-wiring` (`deploy.yml`,
многострочные шаги, свой контракт); из `check-aggregate` их циклы убираются — запрет ключей
шага их покрывает (A7/A8/A12 краснеют по форме, замер §5).

### (г) R4 блокирует merge — но по другой причине, чем «дефект открыт»

Исходный дефект (16 из 18 в условии) закрыт в `1df8e6d`, проверен исполнением на 18 джобах,
живой PR зелёный — это не оспаривается и в этом architect прав. Блокирует другое: A4 в
редакции `116ac46` **обещает** «агрегат ЗОВЁТ пробу, шаг не обезврежен» и **не меряет** этого
(`testing.md`: «оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ»). Барьер, обещающий больше, чем меряет,
хуже отсутствующего: следующий читатель `ci.yml` увидит «проводка под сторожем» и перестанет
смотреть сам.

**Асимметрия цены.** Ещё один круг: ~60–80 строк Python (канон + сравнение), ~10 сценариев
пробы, один прогон адверсария по ЗАКРЫТОМУ перечню (§(д) п.5) — порядка часов. Незакрытая
лазейка: сама по себе дешёвая (нужен намеренный `true ||` в диффе гейт-джоба, который каждый
знает как гейт), но класс сидит в ОБЩЕМ хелпере с четырьмя вызовами, включая A6 — вызов
самого барьера; TD-карта «проверка достижимости принимает недостижимое» будет переоткрыта
следующим критиком на следующем же предмете `check-aggregate`, и тогда круг всё равно
состоится — на худшем основании и без готового решения. Merge сейчас + TD — дороже.

**Решение: исправить В ЭТОМ PR, ОДНИМ ограниченным кругом, с правилом заморозки.**

### (д) Что делает ведущий architect — обязательно к исполнению

1. **`scripts/deploy_catchup.py`, `cmd_check_aggregate`:** ввести канон формы двух джобов
   (§(б): константы `STEP_KEYS`, `CHECKOUT`, форма `status-check`, форма `deploy-catchup`;
   сравнение `run` после `" ".join(run.split())`; каждое отклонение — `FAIL` с путём
   `job.steps[i].<ключ>` и каноном рядом, а не «форма не совпала»). Коды сохранить: **A1** —
   форма `deploy-catchup` (шаги/ключи/команды), **A6** — присутствие и точная форма
   собственного вызова `check-aggregate`, **A4** — форма `status-check` (три шага, checkout
   ровно, проба ровно, `name`/`if`/`runs-on` пиннуты). A2/A3 — без изменений. Удалить
   `_invokes`; убрать вызовы `_step_is_disarmed` из `check-aggregate` (оставить функцию для
   `check-wiring`). Докстринг канона называет: (i) почему белый список, а не разбор (ссылка на
   этот файл и `A-040` Q2); (ii) терминальное звено как `COGNITIVE-ONLY`; (iii) правило
   расширения канона — только с новым сценарием пробы.
2. **`scripts/tests/red_deploy_catchup.sh` — обязательные RED-сценарии (все через `mutate_ci`
   с guard `mut`; ожидание exit=1, кроме позитивных):**
   - позитивные: `A0` как есть; **`A0b`** — `run: |` с переводом строки + `name` у шага условия
     → exit 0 (нормализация не даёт ложного красного);
   - ось «строка `run` пробы»: **R4** `true || P`; `P || echo failed`; `set +e`/`P`/`true`;
     heredoc; `P &`;
   - ось «ключи шага»: `if:` с НЕ-литеральным выражением на шаге пробы; `env:` на шаге пробы;
     `working-directory:`; `timeout-minutes: 0` (закрывает названный `C-259` предел без
     выяснения семантики GitHub);
   - ось «структура джоба»: лишний шаг ДО пробы (шим в `$GITHUB_PATH`); `with:` у checkout;
     `env:` на уровне джоба `status-check`; `name` джоба ≠ `All checks passed`;
   - ось «A6, тот же класс»: `true || python3 scripts/deploy_catchup.py check-aggregate`;
     лишний шаг ДО `check-aggregate` в `deploy-catchup`.
   A11–A14 сохраняются — они обязаны краснеть и по новому основанию.
3. **Мутационный контроль — в Done Block сырым выводом** (`--battery` сюда не дотягивается,
   он про `decide`): нейтрализация сравнения `run` → сценарии оси «строка» FAIL;
   нейтрализация запрета ключей → ось «ключи» FAIL; нейтрализация счёта шагов → A11 и
   «лишний шаг» FAIL; нейтрализация пина `name` → сценарий имени FAIL. Четыре нейтрализации,
   четыре красных набора, затем возврат и зелёный прогон.
4. **`ci.yml` НЕ правится.** Форма вершины канонична (замер). Допустима одна строка
   комментария у `status-check` со ссылкой на канон — не обязательна.
5. **Правило заморозки (обязательно для следующего критика).** После приземления канона
   новая находка вида «ещё одна форма `run`/ключ шага/шаг в этих двух джобах» круг НЕ
   открывает: она либо ≠ канон (FAIL по построению — показывается одним прогоном), либо
   предмет расширения канона. Следующий адверсарий проверяет РОВНО: честная форма и `A0b` —
   PASS; перечисленные в п.2 мутанты — FAIL с названным местом; мутационный контроль п.3
   предъявлен; A0–A14, `W*`, `red_ci_aggregate.sh` 8/8, `CI=true red_review_fa.sh` 50/50 не
   регрессируют; PR #232 зелёный. Находки ВНЕ этого перечня — в TD, не в REJECT.
6. **Merge — автор-architect** (`harness-track.md` §3), после вердикта адверсария файлом на
   ветке. Ветку после merge удалить, резерв `A-041` снят мной после push этого файла.

## 3. Кто прав — по пунктам

| пункт | критик | architect |
|---|---|---|
| R4 воспроизводится, требуется fail-closed определение достижимости | **прав** | — |
| «уточнить разбор» как путь | — | **не прав**: чёрный список не сходится (§1 таблица, §(б)) |
| исходный дефект закрыт, A11–A14 держат формы `C-258` | — | **прав** |
| класс внесён этим PR | — | **прав, что нет**: `_invokes` на `main` с `C-096`, A1/A6 обходятся так же |
| маршрут — арбитр, не founder | **прав** (граница C не затронута) | — |
| merge сейчас с TD | — | **не принимается** (§(г)) |

Несогласие любой стороны фиксируется в этом файле дополнением, решение не отменяет.

---

## 4. Замечания вне решения (не блокируют, в TD при close-out по усмотрению)

- A3 исполняет **склейку всех `run`-шагов `status-check` на корне живого дерева** — включая
  пробу; на мутантах с побочными командами это пишет в рабочее дерево (у меня появился
  `shim/`). После закрытой формы посторонних шагов быть не может, но исполнять A3 стоит только
  над шагом условия, не над склейкой, — иначе проверка условия зависит от того, что проба
  делает с каталогом. Не входит в п.(д): отдельная правка, свой сценарий.
- Половина «fail-closed» исходов §1 — побочные эффекты нераспознавания. С закрытой формой
  они становятся спроектированными; до неё пиннить их сценариями бессмысленно.

## 5. Done Block

```text
$ git fetch origin && git rev-parse origin/harness/ci-aggregate-all-needs
ff93de128a2d70aef142b589dbc71a70ca0aad98
$ git merge-base origin/main origin/harness/ci-aggregate-all-needs
939d1b91df081731cb114dab956a8af60f93cf2c
$ git merge-base --is-ancestor 116ac4621a108126ba2d0253495d8bc8b49d053e origin/harness/ci-aggregate-all-needs; echo exit=$?
exit=0
$ git diff --name-status 116ac46..ff93de1
A	research/critiques/C-259-ci-aggregate-all-needs-r3.md
$ gh pr view 232 --json state,headRefName,baseRefName,statusCheckRollup --jq '.state, .headRefName, .baseRefName, ([.statusCheckRollup[] | select(.conclusion!="SUCCESS")] | length)'
OPEN
harness/ci-aggregate-all-needs
main
0
$ git worktree add --detach /tmp/hft-arbiter-td222 origin/harness/ci-aggregate-all-needs
HEAD is now at ff93de1 docs(TD-222): C-259 — escalate aggregate probe reachability [critic]

--- базовая линия на вершине (мой прогон, не пересказ C-259) ---
$ python3 scripts/deploy_catchup.py check-aggregate | tail -1; echo exit=$?
VERDICT: PASS
exit=0
$ bash scripts/tests/red_deploy_catchup.sh | grep -E 'сценариев:|VERDICT'; echo exit=${PIPESTATUS[0]}
сценариев: 55   PASS: 55   FAIL: 0
VERDICT: PASS
exit=0
$ bash scripts/tests/red_deploy_catchup.sh --battery | grep -E 'батарея|VERDICT'; echo exit=${PIPESTATUS[0]}
--- батарея: мутантов 11, расхождений kill-set 0
VERDICT: PASS
exit=0
$ bash scripts/tests/red_ci_aggregate.sh | tail -1; echo exit=${PIPESTATUS[0]}
VERDICT: PASS — сценариев: 8, расхождений: 0
exit=0
$ CI=true bash scripts/tests/red_review_fa.sh | grep -E 'W8OK|^VERDICT'; echo exit=${PIPESTATUS[0]}
PASS  W8OK реальный ci.yml чекаута несёт полную проводку — wiring OK
VERDICT: PASS (50/50)
exit=0
$ bash --noprofile --norc -eo pipefail -x -c 'true || bash scripts/tests/red_ci_aggregate.sh'; echo exit=$?
+ true
exit=0

--- провенанс класса: _invokes на origin/main ---
$ git show origin/main:scripts/deploy_catchup.py | grep -n '_invokes\|_SWALLOW_SUFFIXES = '
571:_SWALLOW_SUFFIXES = ("|| true", "|| :", "; true", "; :", "|| exit 0")
612:def _invokes(job, needle):
731:        if not _invokes(job, "python3 scripts/deploy_catchup.py"):
734:        if not _invokes(job, "bash scripts/tests/red_deploy_catchup.sh"):
739:        if not _invokes(job, "python3 scripts/deploy_catchup.py check-aggregate"):
$ grep -n '_invokes(' scripts/deploy_catchup.py        # на ветке: те же три + A4
612:def _invokes(job, needle):
731: 734: 739: 786:    if not _invokes(gate, AGG_PROBE):
$ sed -n '/def cmd_check_wiring/,/def cmd_check_aggregate/p' scripts/deploy_catchup.py | grep -c '_invokes'
0

--- стенд §1: checker = exit check-aggregate на мутанте; reach/step_rc = раннер-форма со стабом exit 1 ---
$ python3 $SP/mut.py /tmp/hft-arbiter-td222
форма                                        checker reach step_rc  вердикт
00 честная (эталон)                          checker=0 reach=1 step_rc=1   → ok-честно
R4  true || P                                checker=0 reach=0 step_rc=0   → ЛАЗЕЙКА
f1  false && P                               checker=1 reach=0 step_rc=1   → fail-closed
f2  P || echo failed                         checker=0 reach=1 step_rc=0   → ЛАЗЕЙКА
f3  P || exit 0 ; echo done                  checker=0 reach=1 step_rc=0   → ЛАЗЕЙКА
f4  set +e; P; true (три строки)             checker=0 reach=1 step_rc=0   → ЛАЗЕЙКА
f5  X=bash; $X scripts/...                   checker=1 reach=1 step_rc=1   → fail-closed
f6  eval "P"                                 checker=1 reach=1 step_rc=1   → fail-closed
f7  if [ -e /nonexistent ]; then P; fi       checker=1 reach=0 step_rc=0   → fail-closed
f8  heredoc cat <<EOF P EOF                  checker=0 reach=0 step_rc=0   → ЛАЗЕЙКА
f9  P & (в фон) + wait || true               checker=1 reach=1 step_rc=0   → fail-closed
f9b P & без wait                             checker=0 reach=1 step_rc=0   → ЛАЗЕЙКА
f10 P 2>/dev/null || :                       checker=1 reach=1 step_rc=0   → fail-closed
f11 [ -z "$X" ] && [ -n "$X" ] && P          checker=1 reach=0 step_rc=1   → fail-closed
f12 timeout 0.001 P                          checker=1 reach=0 step_rc=124 → fail-closed
f13 (P) || true в скобках                    checker=1 reach=1 step_rc=0   → fail-closed
f14 ! P                                      checker=1 reach=1 step_rc=0   → fail-closed
f15 P ; exit 0 (второй строкой)              checker=0 reach=1 step_rc=1   → ok-честно (-e прерывает раньше)
f16 set +o pipefail; P | cat                 checker=0 reach=1 step_rc=0   → ЛАЗЕЙКА
f17 while false; do P; done                  checker=1 reach=0 step_rc=0   → fail-closed
k1  if: ${{ false }}                         checker=1 reach=0 step_rc=0   → fail-closed
k2  if: github.event_name == 'never'         checker=0 reach=1 step_rc=1   → стенд if не вычисляет; по семантике шага — ЛАЗЕЙКА
k3  continue-on-error: true                  checker=1 reach=1 step_rc=0   → fail-closed
k4  timeout-minutes: 0 (не моделируется)     checker=0 reach=1 step_rc=1   → не моделируется
k5  shell: python + print(P)                 checker=1 reach=0 step_rc=0   → fail-closed
k6  working-directory: fixtures              checker=0 reach=0 step_rc=0   → ЛАЗЕЙКА
k7  env: CI_AGG_YML=/dev/null                checker=1 reach=1 step_rc=1   → fail-closed (побочно: A3 исполнил пробу на /dev/null)
j1  доп. шаг ДО пробы: shim в GITHUB_PATH    checker=1 reach=0 step_rc=0   → fail-closed (побочно: A3 упал на `>> ""`; в дереве остался shim/)
j2  checkout с ref: старого SHA              checker=0 reach=1 step_rc=1   → не моделируется
j3  uses: ./.github/actions/x вместо пробы   checker=1 reach=0 step_rc=0   → fail-closed
A6  true || python3 ... check-aggregate      checker=0   → ЛАЗЕЙКА (класс на main до PR)
A1  true || bash .../red_deploy_catchup.sh   checker=0   → ЛАЗЕЙКА (класс на main до PR)
A6  python3 ... check-aggregate || echo x    checker=0   → ЛАЗЕЙКА (класс на main до PR)
exit=0
$ ls -d shim && rm -rf shim; git status --porcelain; echo status-exit=$?
shim
status-exit=0

--- прототип закрытой формы (scratchpad closed_form.py, НЕ продукт) ---
$ python3 sweep над теми же мутантами + A-сценарии + origin/main
форма                                      закрытая-форма   первое нарушение
00 честная ветка                           PASS
00b честная: run: | с переводом строки     PASS
00c честная: name у шага-условия           PASS
R4 true || P                               FAIL     status-check.steps[2].run = 'true || bash scripts/tests/red_ci_aggrega
f2 P || echo failed                        FAIL     status-check.steps[2].run = 'bash scripts/tests/red_ci_aggregate.sh ||
f3 P || exit 0; echo done                  FAIL     status-check.steps[2].run = …
f4 set +e; P; true                         FAIL     status-check.steps[2].run = 'set +e bash scripts/tests/red_ci_aggregat
f5 X=bash; $X …                            FAIL     status-check.steps[2].run = 'X=bash $X scripts/tests/red_ci_aggregate.
f6 eval                                    FAIL     status-check.steps[2].run = 'eval "bash scripts/tests/red_ci_aggregate
f7 if [ -e /nonexistent ]                  FAIL     status-check.steps[2].run = 'if [ -e /nonexistent ]; then bash scripts
f8 heredoc                                 FAIL     status-check.steps[2].run = 'cat <<EOF bash scripts/tests/red_ci_aggre
f9b P &                                    FAIL     status-check.steps[2].run = 'bash scripts/tests/red_ci_aggregate.sh &'
f13 (P) || true                            FAIL     status-check.steps[2].run = '(bash scripts/tests/red_ci_aggregate.sh)
f14 ! P                                    FAIL     status-check.steps[2].run = '! bash scripts/tests/red_ci_aggregate.sh'
f16 set +o pipefail; P | cat               FAIL     status-check.steps[2].run = 'set +o pipefail bash scripts/tests/red_ci
f17 while false                            FAIL     status-check.steps[2].run = 'while false; do bash scripts/tests/red_ci
k1 if: ${{ false }}                        FAIL     status-check.steps[2]: лишние ключи шага ['if']
k2 if: event_name=='never'                 FAIL     status-check.steps[2]: лишние ключи шага ['if']
k3 continue-on-error                       FAIL     status-check.steps[2]: лишние ключи шага ['continue-on-error']
k4 timeout-minutes: 0                      FAIL     status-check.steps[2]: лишние ключи шага ['timeout-minutes']
k5 shell: python                           FAIL     status-check.steps[2]: лишние ключи шага ['shell']
k6 working-directory                       FAIL     status-check.steps[2]: лишние ключи шага ['working-directory']
k7 env CI_AGG_YML                          FAIL     status-check.steps[2]: лишние ключи шага ['env']
j1 шим в GITHUB_PATH шагом до пробы        FAIL     status-check: шагов 4, канон 3
j2 checkout with ref                       FAIL     status-check.steps[1] = {'uses': 'actions/checkout@v4', 'with': {'ref'
j3 uses: ./.github/actions/x               FAIL     status-check.steps[2]: лишние ключи шага ['uses']
j4 job continue-on-error                   FAIL     status-check: лишние ключи джоба ['continue-on-error']
j5 job env CI_AGG_YML                      FAIL     status-check: лишние ключи джоба ['env']
j6 job defaults.run.shell                  FAIL     status-check: лишние ключи джоба ['defaults']
j7 rename job → защита теряет чек          FAIL     status-check.name = 'All checks passed ', канон 'All checks passed'
A11 проба удалена                          FAIL     status-check: шагов 2, канон 3
A14 echo P                                 FAIL     status-check.steps[2].run = 'echo bash scripts/tests/red_ci_aggregate.
A6 true || python3 … check-aggregate       FAIL     deploy-catchup.steps[3].run = 'true || python3 scripts/deploy_catchup.
A6b … check-aggregate || echo x            FAIL     deploy-catchup.steps[3].run = 'python3 scripts/deploy_catchup.py check
A9 check-aggregate шаг удалён              FAIL     deploy-catchup: шагов 5, канон 6
A10 echo с обоими именами                  FAIL     deploy-catchup: шагов 1, канон 6
A4 джоб-пустышка                           FAIL     deploy-catchup: шагов 1, канон 6
A7 cond continue-on-error                  FAIL     status-check.steps[0]: лишние ключи шага ['continue-on-error']
A8 cond if: false                          FAIL     status-check.steps[0]: лишние ключи шага ['if']
dc1 шим-шаг ДО check-aggregate             FAIL     deploy-catchup: шагов 7, канон 6
MAIN origin/main ci.yml (пробы нет)        FAIL     status-check: шагов 1, канон 3
exit=0

--- резерв, дрейф, диск ---
$ bash scripts/reserve_artifact_id.sh A
A-041
exit=0
$ git fetch origin; echo head_after_fetch=$(git rev-parse origin/harness/ci-aggregate-all-needs)
head_after_fetch=ff93de128a2d70aef142b589dbc71a70ca0aad98
judged=ff93de128a2d70aef142b589dbc71a70ca0aad98   drift=none
$ df -h / | tail -1
/dev/md2        437G  334G   81G  81% /
$ date -u
2026-09-27T14:55Z
```
