# R-069 — перепроверка §9: TD-135, герметизация пробы design-claims

- **Роль:** независимый Fable-перепроверщик со свежим контекстом (`gates.md` §9); автор правки — architect, в этом круге сторона.
- **Предмет:** ветка `fix/TD-135-probe-hermeticity`, три коммита поверх `237f65d` (`origin/main`):
  `bad4fc4` (RED — герметизация пробы), `a3565d0` (GREEN — фикстура сценария 9 несёт идентичность),
  `97cbc67` (тот же класс в `scripts/verify_M-61.sh:143,178`).
- **Дата:** 2026-08-14. Рабочее дерево: `/tmp/hft-recheck-td135` (detached, ветка не занималась).

## ВЕРДИКТ: **APPROVED с NOTE**

Посылка автора воспроизведена по первоисточникам (CI-лог джоба + локальная репродукция в пустом
окружении), фикс пиннится мутацией в обе стороны, обе формы гейта на ветке зелёные без расхождения
ветка/дерево слияния, полномочия чисты, все ссылки тел коммитов разрешаются. Три NOTE ниже —
ни один не блокирует. Фикс достаточен для разблокировки `main`.

## A. Проверено исполнением (каждое утверждение — команда + исход)

**A-1. RED воспроизведён; механизм совпадает с CI.**

```
$ cd <worktree 237f65d> && env HOME=<пустой каталог> GIT_CONFIG_GLOBAL=/dev/null \
    GIT_CONFIG_SYSTEM=/dev/null bash scripts/tests/red_verify_design_claims.sh
fatal: unable to auto-detect email address (...)
FAIL  сценарий 9 (--merge-preview: конфликт слияния): ОЖИДАЛСЯ FAIL [SETUP] «КОНФЛИКТУЕТ», получено (exit=1): FAIL [1-ЕСТЬ] ... crates/foo ... ОТСУТСТВУЕТ
VERDICT: FAIL (1 нарушений)          exit=1; PASS-строк 40
```

CI (первоисточник, job 94737490534 / run 31790951740, HEAD `237f65d`): гейт сам —
`VERDICT: PASS (0 нарушений)`; затем `fatal: empty ident name (for <runner@...>)`; затем
`FAIL  сценарий 9 ...` — тот же механизм, что в локальной репродукции. `gh run list` по `main`:
красных SHA подряд **семь** (`866c29d`→`237f65d`); по джобам красен ТОЛЬКО `Design claims`,
остальные 8 джобов success — негерметичность живёт ровно там, где её назвал автор.

**A-2. Ветка зелёная в прод-форме CI.**

```
$ bash scripts/tests/red_verify_design_claims.sh          # та же argv, что в ci.yml:229
VERDICT: PASS   exit=0
$ ... | grep -cE "^PASS"   → 41    # посчитано, совпадает с заявленным и с комментарием ci.yml
```

**A-3. Дерево слияния (`gates.md` §8) — расхождения нет.**

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main ; echo $?   → VERDICT: PASS (0 нарушений), exit=0
$ bash scripts/verify_design_claims.sh ; echo $?                               → VERDICT: PASS (0 нарушений), exit=0
FAIL-строк в обоих логах: 0
```

**A-4. Мутация, вопрос 1 (привязка к дефекту).** Снятие `-c user.name=test -c user.email=test@test.local`
со строки 556 (`base:`-коммит фикстуры сценария 9) →

```
FAIL  сценарий 9 (--merge-preview: конфликт слияния): ...   VERDICT: FAIL (1 нарушений), exit=1
```

Оракул пиннит именно починенные строки. Файл восстановлен, дерево чистое (0 modified).

**A-5. Мутация, вопрос 2 (несущесть нейтрализации).** Файл целиком возвращён в состояние `main`
(`git checkout 237f65d -- scripts/tests/red_verify_design_claims.sh`: нет герметичного блока, нет `-c`),
прогон под нормальным `HOME` разработчика → `VERDICT: PASS`, exit=0. Дефект без нейтрализации
локально НЕВИДИМ — ровно состояние `main` до фикса. `bad4fc4` — несущий, не косметика.

**A-6. Отвергнутая развязка (`export GIT_AUTHOR_NAME/...` в шапке) — проверена придирчиво, тремя замерами.**

- (i) Барьер — потомок пробы (`out="$(bash "${BARRIER}" ...)"`,
  `scripts/tests/red_verify_design_claims.sh:563`), экспортированное окружение наследует. Верно.
- (ii) Git-мутация барьера, зависящая от идентичности, СУЩЕСТВУЕТ: в песочнице
  `git merge --no-commit --no-ff` без `-c` и без идентичности → **exit=128,
  «Committer identity unknown»**. Значит `-c user.name=verify-design-claims` на
  `scripts/verify_design_claims.sh:192` — несущий, а не декоративный.
- (iii) Прототип измерен на до-фиксном `main`: `env HOME=<пусто> ... GIT_AUTHOR_NAME=ci
  GIT_COMMITTER_NAME=ci ... bash scripts/tests/red_verify_design_claims.sh` → `VERDICT: PASS`,
  exit=0 — заявление автора «измеренно даёт PASS» подтверждено; СЕГОДНЯ обе развязки закрыли бы
  TD-135. Но: `git merge --no-commit` без `-c` при ambient env-идентичности → exit=0. То есть под
  export-прототипом будущая потеря `-c` барьером стала бы пробе НЕВИДИМА (зелёная проба, красная
  прод-форма), под герметичной развязкой — краснеет. **Довод автора НЕ пустой** — это защита
  реального класса, который держится на одной строке барьера. Оговорка о силе формулировки — NOTE N-3.

**A-7. Синтаксис и логика шага S.** `bash -n` обоих файлов — ok. Диф `97cbc67` меняет ТОЛЬКО
идентичность коммит-вызова (+ перенос строки); `git add -A` (внутри песочницы-клона), `|| true` и
вызов барьера нетронуты — логика шага S не смещена. Новая форма в пустом окружении создаёт коммит:
`git -c user.name=verify-m61 -c user.email=verify-m61@noreply.local commit -q -m ...` → exit=0,
`8d02c21 синтетический дубль (verify-m61)`; форма ДО в том же окружении коммит не создаёт.

## B. Находки

**N-1 (NOTE, единственная содержательная): герметизация не снимает ident-переменные окружения.**
`scripts/tests/red_verify_design_claims.sh:46-49` нейтрализует `HOME`/`GIT_CONFIG_GLOBAL`/
`GIT_CONFIG_SYSTEM`, но не `GIT_AUTHOR_NAME`/`GIT_AUTHOR_EMAIL`/`GIT_COMMITTER_NAME`/
`GIT_COMMITTER_EMAIL` (и `EMAIL`). Воспроизведение: мутант из A-4 + `env GIT_AUTHOR_NAME=leak
GIT_AUTHOR_EMAIL=leak@x GIT_COMMITTER_NAME=leak GIT_COMMITTER_EMAIL=leak@x bash <проба>` →
`VERDICT: PASS`, exit=0. На хосте с экспортированной git-идентичностью будущий голый фикстурный
коммит снова зелен локально при красном CI — остаток того же класса, что закрывает TD-135.
НЕ блокер: раннер этих переменных не несёт (прод-форма CI не затронута), дисциплина проекта
предписывает `~/.gitconfig`, а не env (`branch-hygiene.md` п.6). Рекомендация: одна строка
`unset GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL EMAIL` в
герметичном блоке закрывает класс целиком; follow-up architect'а тем же файлом.

**N-2 (NOTE, косметика тел коммитов — кода не касается, несущие утверждения верны):**
- `a3565d0`: «снять `-c` с одной строки (533)» — в дереве, куда лёг коммит, это строка **556**
  (533 — нумерация до RED-коммита, сдвиг +23);
- `bad4fc4`: «шестью SHA подряд» — на момент перепроверки их **семь** (`866c29d`…`237f65d`);
  несущее «красит подряд с 866c29d» верно;
- `97cbc67`: «из ~95» — склейка продолжений строк даёт **80** коммит-вызовов в `scripts/**`;
  несущее «РОВНО четыре голых» подтверждено точно: `main` — 4 (проба:2 + `verify_M-61.sh:143,178`),
  ветка — **0**; прочие вызовы либо несут построчный `-c` (вкл. переносы, `red_gc_reclaim_args.sh`),
  либо живут в репо с фабричным `git config user.*`; единственный кандидат вне списка
  (`check_protected_artifacts.sh:179`) — `echo`-строка, не вызов;
- `bad4fc4`: «`ci.yml:226`» — комментарий живёт на строках 225-227.

**N-3 (NOTE): формулировка «прототип НЕВЕРЕН» сильнее фактов.** По замерам A-6, сегодня прототип
тоже разблокировал бы CI: обе развязки наблюдаемо эквивалентны на текущем тексте барьера, потому
что его единственная ident-зависимая мутация уже несёт свой `-c`. Разница — в устойчивости к
будущей правке барьера (потеря `-c` под экспортом маскируется, под герметизацией краснеет), и по
этому основанию выбранная развязка строго сильнее. Довод автора по существу верен; неточна лишь
подача «уровнем глубже» как настоящего, а не латентного дефекта.

## C. Полномочия (б)

- **Зона:** диф трогает только `scripts/tests/red_verify_design_claims.sh` + `scripts/verify_M-61.sh` —
  sacred, architect-only (`scope-guard.md`); автор — architect. Соблюдено.
- **Замок §11:** на `scripts/**` НЕ распространяется (зона §11 — `.claude/rules|agents|wrappers`,
  `CLAUDE.md`, `docs/04-workflow.md`); механически:
  `EVENT_NAME=push PUSH_BEFORE=237f65d bash scripts/check_docs_freeze.sh` → exit=0, не красит.
  Токен `FOUNDER-APPROVED` не требуется.
- **Граница C:** не пересечена (ни промоушенов, ни весов/лимитов, ни состава записываемых данных).
- **Push-scope:** `git log 237f65d..97cbc67` — ровно три коммита автора, чужих нет.
- **`branch-hygiene.md` п.9:** `git show --numstat` всех трёх соответствует заявленному
  (23/0; 2/2; 4/2 — по одному файлу на коммит).

## D. Связность (в)

- **TD-135** — `TECH-DEBT.md:142`: BLOCKER, заведён reviewer'ом при close-out M-62. Разрешается. ✓
- **TD-136** — `TECH-DEBT.md:60`: «шаги N и S verify-M-61 глотают stderr», MAJOR — довод `97cbc67`
  «`|| true` не трогаю, это TD-136» опирается на существующую карточку. ✓
- **BACKLOG Н-3** — `milestones/BACKLOG.md:326` «ни один verify_M-NN.sh не исполняется в CI»;
  подтверждено: `grep -rn verify_M .github/workflows/` → пусто. ✓
- **R-067** — `research/reviews/R-067-g2-design-claims.md` существует, вердикт APPROVED —
  утверждение `bad4fc4` о нём корректно. ✓
- `gates.md` §8, `testing.md` «Целостность гейта — 4 свойства» — существуют, применены по делу. ✓

## E. Условие APPROVED

Безусловно — NOTE не блокируют. Рекомендация (не условие): закрыть N-1 отдельным коммитом
(`unset` ident-переменных в герметичном блоке) — зона и RED-дисциплина автора, круг дешёвый.

## Done Block

```
$ env HOME=/tmp/.../empty GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    bash scripts/tests/red_verify_design_claims.sh   # worktree 237f65d (до фикса)
FAIL  сценарий 9 (--merge-preview: конфликт слияния): ОЖИДАЛСЯ FAIL [SETUP] «КОНФЛИКТУЕТ», получено (exit=1):
VERDICT: FAIL (1 нарушений)
exit=1                        # PASS-строк: 40

$ bash scripts/tests/red_verify_design_claims.sh     # worktree 97cbc67 (ветка), прод-форма CI
VERDICT: PASS
exit=0                        # PASS-строк: 41 (grep -cE "^PASS"), FAIL-строк: 0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main >mp.log 2>&1; echo mp_exit=$?
mp_exit=0                     # VERDICT: PASS (0 нарушений); FAIL-строк: 0
$ bash scripts/verify_design_claims.sh >plain.log 2>&1; echo plain_exit=$?
plain_exit=0                  # VERDICT: PASS (0 нарушений); FAIL-строк: 0

$ sed -i '556s/ -c user\.name=test -c user\.email=test@test\.local//' scripts/tests/red_verify_design_claims.sh \
    && bash scripts/tests/red_verify_design_claims.sh
FAIL  сценарий 9 (--merge-preview: конфликт слияния): ...
VERDICT: FAIL (1 нарушений)
M1 exit=1                     # мутация снятия -c → проба красная; файл восстановлен, 0 modified

$ env GIT_AUTHOR_NAME=leak GIT_AUTHOR_EMAIL=leak@x GIT_COMMITTER_NAME=leak GIT_COMMITTER_EMAIL=leak@x \
    bash scripts/tests/red_verify_design_claims.sh   # тот же мутант
M1b exit=0  VERDICT: PASS     # → находка N-1 (ident-env не снимается)

$ git checkout 237f65d -- scripts/tests/red_verify_design_claims.sh && bash scripts/tests/red_verify_design_claims.sh
M2 exit=0   VERDICT: PASS     # без нейтрализации дефект локально невидим → bad4fc4 несущий

$ env HOME=<пусто> ... GIT_AUTHOR_NAME=ci GIT_COMMITTER_NAME=ci ... bash scripts/tests/red_verify_design_claims.sh
exit=0      VERDICT: PASS     # worktree 237f65d: export-прототип, заявление автора подтверждено

# песочница (HOME=<пусто>, GIT_CONFIG_GLOBAL/SYSTEM=/dev/null):
$ git merge --no-commit --no-ff side                                   → exit=128 (Committer identity unknown)
$ env GIT_AUTHOR_NAME=ci ... git merge --no-commit --no-ff side        → exit=0
$ git -c user.name=verify-m61 -c user.email=verify-m61@noreply.local commit -q -m "синтетический дубль"
exit=0      8d02c21 синтетический дубль (verify-m61)

$ bash -n scripts/tests/red_verify_design_claims.sh && bash -n scripts/verify_M-61.sh
ok / ok     exit=0

$ EVENT_NAME=push PUSH_BEFORE=237f65d bash scripts/check_docs_freeze.sh; echo freeze_exit=$?
freeze_exit=0

$ gh run view 31790951740 --json jobs ...   # HEAD=237f65d
Design claims ...: failure    # остальные 8 джобов: success
$ gh api .../jobs/94737490534/logs | grep -aE "ident|сценарий 9|VERDICT"
VERDICT: PASS (0 нарушений)                      # гейт сам зелен
fatal: empty ident name (for <runner@...>) not allowed
FAIL  сценарий 9 (--merge-preview: конфликт слияния): ОЖИДАЛСЯ FAIL [SETUP] «КОНФЛИКТУЕТ», получено (exit=1):
```

**VERDICT: APPROVED с NOTE** — фикс достаточен для разблокировки `main`; ещё один круг не нужен.
