<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: ffb1d0231073f358d78ba7a0c49e1d4e3f8381f1
audited_head: da9fbcbb54c9385134ac5eb843e114d3c56d722f
verdict: APPROVE
-->

# R-122 — перепроверка §9, круг 2: `docs/architect-profile-facts-fix` (`da9fbcb`, снятие Б-1 вердикта `R-121`)

Независимый проверяющий по `gates.md` §9, модель Fable, свежий контекст (тот же агент, что
выносил `R-121` круга 1 — для круга 2 по СВОЕЙ находке это допустимо: проверяется снятие
моего же блокера, а не новая авторская конструкция). Предмет — коммит `da9fbcb`
(вершина ветки; под ним `27e7006` — вердикт `R-121`, под ним `a219fe4` — круг 1), файл
`.claude/agents/architect.md`, numstat 16/3 (перепроверено). Merge-превью `origin/main` =
`ffb1d02` × `da9fbcb` — отдельный worktree, слияние без конфликта (2 файла: профиль 45/11
суммарно + `R-121`).

**Вердикт: APPROVE.** Б-1 закрыт по классу, а не по клетке: дефектное предложение заменено
пятью раздельными утверждениями, КАЖДОЕ из которых подтверждено моей командой на дереве
слияния; новых ложных утверждений о коде правка не внесла; полномочия и связность в порядке.
След прежней ошибки оставлен явно, а не заменён молча.

---

## §1. Пять утверждений нового текста — каждое своей командой (дерево слияния)

| # | утверждение `da9fbcb` | моя команда | полученный вывод | совпало |
|---|---|---|---|---|
| 1 | барьер печатает РОВНО два идентификатора: `H-FACTS-SHA` и `H-FACTS` | `grep -oE '"H-FACTS[A-Z0-9-]*"' scripts/verify_design_claims.sh \| sort -u` | `"H-FACTS"` и `"H-FACTS-SHA"` — ровно две строки | ДА |
| 2 | у `H-FACTS-SHA` семь мест вызова: пять `fail`, один `pass`, один `info` (последний — когда фактур с маркером нет) | `grep -c '"H-FACTS-SHA",' …` → 7; `grep -n -B1 '"H-FACTS-SHA",' …` | :816 fail · :824 fail · :833 **info** («документов с маркером `FACTS:` в docs/plans/**: 0 — проверять нечего») · :848 fail · :860 fail · :868 fail · :876 **pass_** — итого 5/1/1 | ДА (и семантика info — дословно «нет вовсе») |
| 3 | `H-FACTS-1..19` — сценарии ПРОБЫ, 20 уникальных вместе с `H-FACTS-SHA`; в барьере — только комментарий `:618-619` | `grep -n 'H-FACTS-[0-9]' scripts/verify_design_claims.sh`; `grep -oE 'H-FACTS-[0-9]+' scripts/tests/red_verify_design_claims.sh \| sort -uV`; `grep -oE 'H-FACTS-[A-Z0-9]+' … \| sort -u \| wc -l` | в барьере — ровно строки 618 и 619 (комментарий); в пробе — H-FACTS-1…H-FACTS-19 без пропусков (19 шт.); уникальных с SHA — `20` | ДА |
| 4 | `FACTS_HEAD_LINES = 5` — лимит ГОЛОВЫ; границы пиннят `H-FACTS-3` и `H-FACTS-9` | `grep -nE 'FACTS_HEAD_LINES = ' …`; `grep -n 'H-FACTS-3 \|H-FACTS-9' scripts/tests/red_…` | `:630 FACTS_HEAD_LINES = 5 # сколько первых строк считаются «головой файла»`; red:488 «маркер в ПРОЗЕ, не в голове» (верх), red:805 «H-FACTS-3 держит верх …; этот — низ», red:822 «маркер на 5-й строке — граница головы» | ДА |
| 5 | `FACTS_NOTE_THRESHOLD = 20` — ПОРОГ; границы пиннят `H-FACTS-5` и `H-FACTS-16` | `grep -nE 'FACTS_NOTE_THRESHOLD = ' …`; `grep -n 'H-FACTS-5 \|H-FACTS-16' scripts/tests/red_…` | `:631 FACTS_NOTE_THRESHOLD = 20 # порог утверждений`; red:524 «3 утверждения < порога — NOTE НЕ печатается» (низ), red:959 «ровно 20 утверждений — граница порога» | ДА |

Сопутствующие утверждения того же абзаца: «`H-FACTS-SHA` — ревизия, названная маркером,
обязана существовать в истории» — подтверждено кругом 1 (`R-121` §1 п.4, fail-пути
:861/:869 версии `a219fe4`-превью, барьер не менялся); «`H-FACTS` — фактура БЕЗ маркера,
набравшая порог утверждений» — `sed -n 900,910p scripts/verify_design_claims.sh`:
`note("H-FACTS", …N утверждений … без маркера…)` при `n >= FACTS_NOTE_THRESHOLD`. Совпало.

## §2. Закрытие Б-1 по КЛАССУ

Дефектное предложение снято целиком (`git diff 27e7006 da9fbcb`: `-Проверок в барьере три…`).
Абзац перечитан построчно на предмет ЛЮБОГО другого утверждения о коде — все найденные
перечислены в §1 и подтверждены; утверждений, которые греп опровергает, не осталось.
Ирония зафиксирована самим текстом («дважды подряд промахнулся именно в утверждении о коде») —
след оставлен, замена не молчалива.

## §3. Полномочия и связность

- **Замок §11:** тело `da9fbcb` несёт `FOUNDER-APPROVED: снятие ложного утверждения о коде
  в профиле, тот же предмет и то же решение founder'а 2026-08-23 «9. делай»` (≥12 симв.).
  `bash scripts/check_docs_freeze.sh 27e7006 da9fbcb` → exit=0 (перепроверено, совпало с
  замером ведущего).
- **Новых обязывающих норм нет:** добавленное — описание структуры барьера и след ошибки.
- **Связность добавленных ссылок:** `R-121` — теперь РАЗРЕШАЕТСЯ на дереве слияния (ветка
  несёт `27e7006`; merge-stat: `create mode … R-121-recheck-architect-profile-facts-fix.md`);
  `:618-619` — точна (см. §1 п.3); путь `scripts/tests/red_verify_design_claims.sh` существует.
- Гейты на дереве слияния: `check_context_budgets.sh` → PASS (111943 B из 114900 B), exit=0;
  `verify_design_claims.sh` → `VERDICT: PASS (0 нарушений)`, 1 прежний NOTE `[H-FACTS]`
  (contracts-current-state.md, 23), exit=0.

## §4. Не повторяется (рамка круга 2)

`Н-1` (`R-114` вне `main`) и `Н-2` (`TD-163` ⇒ PR #75 первым) — приняты ведущим как условия
порядка приземления; состояние не изменилось, повторно не выношу. `Н-3` — принято: rebase
рвал бы `audited_head` вердикта `R-121`, расхождение зафиксировано телом `da9fbcb`.
Неизменённое кругом 2 повторно не проверялось (эскалация скрутинии остановлена).

## §5. Done Block

```
$ git rev-parse origin/main; git rev-parse da9fbcb
ffb1d0231073f358d78ba7a0c49e1d4e3f8381f1
da9fbcbb54c9385134ac5eb843e114d3c56d722f

$ git show --numstat --format='' da9fbcb -- .claude/agents/architect.md
16	3	.claude/agents/architect.md

$ (worktree merge-превью ffb1d02 × da9fbcb) → 'ort', 2 files, 226(+) 11(-), без конфликта

$ grep -oE '"H-FACTS[A-Z0-9-]*"' scripts/verify_design_claims.sh | sort -u
"H-FACTS"
"H-FACTS-SHA"
$ grep -c '"H-FACTS-SHA",' scripts/verify_design_claims.sh
7
$ grep -n 'H-FACTS-[0-9]' scripts/verify_design_claims.sh
618:# … сам лимит запиннен сценариями H-FACTS-3 (верх) и
619:# H-FACTS-9 (низ), а не этим комментарием.
$ grep -oE 'H-FACTS-[A-Z0-9]+' scripts/tests/red_verify_design_claims.sh | sort -u | wc -l
20
$ grep -nE 'FACTS_HEAD_LINES = |FACTS_NOTE_THRESHOLD = ' scripts/verify_design_claims.sh
630:FACTS_HEAD_LINES = 5          # сколько первых строк считаются «головой файла»
631:FACTS_NOTE_THRESHOLD = 20     # порог утверждений `путь:строка`, при котором молчание заметно

$ bash scripts/check_docs_freeze.sh 27e7006 da9fbcb; echo exit=$?
exit=0
$ bash scripts/check_context_budgets.sh 2>&1 | tail -1
VERDICT: PASS — 7 файлов, 111943 B из 114900 B бюджета (запас 2957 B)
$ bash scripts/verify_design_claims.sh 2>&1 | grep -E '^(FAIL|VERDICT)'
VERDICT: PASS (0 нарушений)
```

Диапазон не трогает `crates/**` — `check_review_fa.sh` даёт `SKIP`; FA-WAIVER не нужен.
