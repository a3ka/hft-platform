# R-085 — перепроверка `gates.md` §9: branch protection (`fc53ed7`)

- **Роль:** независимый Fable-перепроверщик (`gates.md` §9), СВЕЖИЙ контекст, автор правки не проверял себя.
- **Предмет:** `origin/docs/branch-protection`, коммит `fc53ed7` — `.claude/rules/gates.md`,
  `.claude/rules/commit-discipline.md`, `.claude/agents/reviewer.md`.
- **Дерево проверки:** worktree `/tmp/recheck-prot` (detached `fc53ed7`); merge-base с `origin/main` = `41ba13c`.
- **Дата (UTC):** 2026-08-15T11:2xZ

## ВЕРДИКТ: **REJECT**

Инфраструктурная половина правки **подтверждена замером полностью** — защита включена, барьер
`docs-freeze` реален, прямой push отклоняется. Правка реджектится не за ложь о факте, а за то,
что она **привела к факту ОДНУ норму из трёх и оставила две другие предписывать невозможное** —
включая ту, по которой этот самый коммит обязан попасть в `main`. Первый же агент, исполнивший
`commit-discipline.md` п.5 или `gates.md` §9 буквально, упрётся в `GH006` и не поймёт почему;
это ровно тот класс, который правка была обязана закрыть.

Устранение — правка текста, не пересмотр решения. Círculo короткий.

---

## Покрытие §9 — пункт (а): утверждения о коде и инфраструктуре, проверенные командой

Ни одна цифра из тела коммита не принята на веру; всё воспроизведено на дереве слияния.

### (а.1) Защита включена и в заявленном режиме — ПОДТВЕРЖДЕНО

```
$ gh api repos/a3ka/hft-platform --jq '{private:.private, visibility:.visibility}'
{"private":false,"visibility":"public"}

$ gh api repos/a3ka/hft-platform/branches/main/protection --jq \
    '{strict:.required_status_checks.strict, contexts:.required_status_checks.contexts,
      enforce_admins:.enforce_admins.enabled, force_push:.allow_force_pushes.enabled,
      deletions:.allow_deletions.enabled}'
{"contexts":["All checks passed"],"deletions":false,"enforce_admins":true,
 "force_push":false,"strict":false}
```

Совпадает с заявленным: обязательный чек `All checks passed`, `enforce_admins: true`,
`strict: false`. Дополнительно (в теле коммита не названо, но существенно):
`allow_force_pushes: false`, `allow_deletions: false` — `main` защищён и от переписывания истории.
`gh api repos/a3ka/hft-platform/rulesets` → `[]` (иных правил поверх классической защиты нет).

### (а.2) Прямой push отклоняется, push в ветку проходит — СВОЯ проба, обе стороны

Проба сделана мной, не переписана из тела коммита. Предварительно убедился, что предмет
безопасен для пробы (у `fc53ed7` нет ни одного чек-рана, значит push будет отклонён, а не
случайно принят):

```
$ gh api repos/a3ka/hft-platform/commits/fc53ed7.../check-runs --jq '{total:.total_count}'
{"total":0}

$ git push origin fc53ed7:main
remote: error: GH006: Protected branch update failed for refs/heads/main.
remote: - Required status check "All checks passed" is expected.
 ! [remote rejected] fc53ed7 -> main (protected branch hook declined)
exit=1

$ git push origin fc53ed7:refs/heads/probe/recheck-control      # контроль
 * [new branch]      fc53ed7 -> probe/recheck-control
exit=0
$ git push origin --delete probe/recheck-control                 # уборка пробы
 - [deleted]         probe/recheck-control
exit=0
```

Обе стороны воспроизведены. Ветка пробы удалена — мусора не оставлено.

### (а.3) `docs-freeze` ВХОДИТ в агрегат `All checks passed` — ПОДТВЕРЖДЕНО, класс «зелёный агрегат над красным джобом» закрыт

Это единственное место, где утверждение могло быть верным по букве и ложным по делу: джоб
числится в `needs`, а агрегат всё равно печатает «All checks passed». Разобрано механически.

`.github/workflows/ci.yml:231-241`:
```yaml
  status-check:
    name: All checks passed
    needs: [build-test, security, delivery, protected-artifacts, contracts, docs-freeze, artifact-ids, design-claims]
    if: always()
    steps:
      - run: |
          if [[ "${{ needs.build-test.result }}" != "success" || ... || "${{ needs.docs-freeze.result }}" != "success" || ... ]]; then
            echo "One or more checks failed"; exit 1
          fi
          echo "All checks passed"
```

Членство проверено не глазами, а сверкой множеств:
```
$ python3  # сравнение needs[] с множеством needs.<job>.result в if-условии
needs   : ['artifact-ids','build-test','contracts','delivery','design-claims','docs-freeze','protected-artifacts','security'] 8
if-check: ['artifact-ids','build-test','contracts','delivery','design-claims','docs-freeze','protected-artifacts','security'] 8
MATCH   : True
```

`if: always()` + явная проверка `.result` каждого из восьми + `exit 1` — агрегат НЕ может
позеленеть над красным или пропущенным джобом. Имя джоба (`All checks passed`) совпадает с
контекстом в `required_status_checks.contexts`. **Утверждение верно.**

### (а.4) Барьер реален — анти-плацебо в обе стороны

Ссылки на «джоб есть» недостаточно: проверил, что барьер ПАДАЕТ без токена.

```
$ EVENT_NAME=push PUSH_BEFORE=41ba13c bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0                                   # с токеном в теле fc53ed7

# скретч-коммит: правка зоны (.claude/rules/scope-guard.md) БЕЗ токена
$ EVENT_NAME=push PUSH_BEFORE=41ba13c bash scripts/check_docs_freeze.sh; echo exit=$?
exit=1                                   # без токена — красный
$ git reset --hard fc53ed7 -q            # скретч снят, дерево чистое
```

Цепочка «токен отсутствует → `docs-freeze` красный → агрегат красный → защита отклоняет push»
замкнута на каждом звене. **Утверждение «коммит зоны без токена физически не вливается» — ВЕРНО.**

### (а.5) Подделываемость токена защитой НЕ лечится — утверждение ВЕРНО

`check_docs_freeze.sh:73-77` — предикат ровно один:
```bash
printf '%s\n' "$body" | grep -qE '^FOUNDER-APPROVED: .{12,}'
```
Проверяется НАЛИЧИЕ строки и длина причины (≥12 символов), не её истинность. Защита ветки
добавляет принуждение к исполнению предиката, но сам предикат не усиливает. Правка называет это
явно и корректно разделяет: механизировалась половина «никто не мешает влить», половина
«истинность причины» осталась `COGNITIVE-ONLY`. Возражений нет.

### (а.6) Обязательные прогоны

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [7-RFC-PATH] путей-кандидатов ... всего=272 проверено=181 пропущено=91 — все 181 существуют
VERDICT: PASS (0 нарушений)
exit=0
```

### (а.7) НЕТОЧНОСТЬ механизма — защита не требует PR (N-3)

Правка утверждает (`gates.md:293`): «Merge в `main` идёт ТОЛЬКО через PR… Прямой
`git push origin main` отклоняется защитой ветки — **это факт инфраструктуры, а не пожелание**».

Замер говорит тоньше. Настройка «Require a pull request before merging» **не включена**:

```
$ gh api repos/a3ka/hft-platform/branches/main/protection --jq 'keys'
["allow_deletions","allow_force_pushes","allow_fork_syncing","block_creations",
 "enforce_admins","lock_branch","required_conversation_resolution",
 "required_linear_history","required_signatures","required_status_checks","url"]
                       ^^^ required_pull_request_reviews ОТСУТСТВУЕТ
```

Подтверждено поведением на живом PR (не только интроспекцией API):
```
$ gh pr view 1 --json mergeStateStatus,reviewDecision
{"mergeStateStatus":"BLOCKED","reviewDecision":""}     # BLOCKED из-за pending-чека, НЕ из-за ревью
```

**Ловушка, которую я обязан назвать:** саб-ресурс
`/branches/main/protection/required_pull_request_reviews` отдаёт `200` с
`{"required_approving_review_count":1}` — остаточные данные, противоречащие родительскому
объекту. Поведение PR #1 (`reviewDecision: ""`) разрешает противоречие в пользу родителя:
ревью НЕ требуются. Проверяющий, поверивший саб-ресурсу, сделал бы ложный вывод.

Отсюда точная формулировка того, что принуждается: **на `main` попадает только SHA с зелёным
`All checks passed`** — не «только через PR». Текст сообщения об отказе это и говорит:
`Required status check … is **expected**` (чек ОТСУТСТВУЕТ), а не «Changes must be made through
a pull request». Технически SHA с зелёным чеком можно доставить в `main` и прямым
fast-forward-push'ем.

Практически вывод правки почти совпадает с фактом — но по ДРУГОЙ причине, чем она называет:
`ci.yml:3-7` даёт триггеры только `pull_request: branches:[main]` и `push: branches:[main]`,
`workflow_dispatch` отсутствует. Получить зелёный чек на ветке, не открыв PR, нечем.
**PR обязателен де-факто, потому что он единственный производитель зелёного чека, а не потому
что защита его требует.** Формулировку надо привести к этому: сейчас правило называет
механизмом то, что механизмом не является, и агент, прочитавший «прямой push отклоняется —
факт инфраструктуры», получит ложную модель (например решит, что после зелёного PR
fast-forward-push тоже отклонят).

---

## Покрытие §9 — пункт (б): полномочия

| Проверка | Результат |
|---|---|
| Токен `FOUNDER-APPROVED` покоммитно, причина ≥12 символов | **ЕСТЬ.** Тело `fc53ed7` несёт `FOUNDER-APPROVED: founder 2026-08-15 перевёл репозиторий в public и выбрал строгий режим защиты main (enforce_admins=true)`; барьер на диапазоне `41ba13c..fc53ed7` → exit=0 |
| Диапазон — один коммит, чужих нет | `git log origin/main..fc53ed7` → ровно `fc53ed7` |
| Зона правки не превышена | `git show --numstat fc53ed7` → 3 файла: `.claude/agents/reviewer.md` (2/2), `.claude/rules/commit-discipline.md` (4/2), `.claude/rules/gates.md` (21/2). Все — внутри `Writes` профиля architect'а (`.claude/**`) |
| Граница C (`gates.md` §0.1) не присвоена | **Не нарушена.** Предмет §0.1 — промоушены/веса/`ParamChange`, состав записываемых данных, переход фаз, live, деньги. Защита ветки ни в один пункт не попадает |
| Заявленный объём соответствует дифу | Соответствует: 27 добавлено / 6 удалено, ровно три файла |

**N-6 (замечание, не блокер).** `TD-124` в реестре объявлен границей C собственным текстом:
`TECH-DEBT.md:751` — «**Severity: MAJOR. Граница C ⇒ решает FOUNDER, reviewer только
констатирует.**» Следа решения founder'а в репозитории нет: `docs/PENDING-SIGNATURE.md` не
содержит ни «branch protection», ни «public», ни «TD-124» (греп пуст). Единственный носитель
разрешения — токен в теле коммита. По §11 это ровно тот уровень, который норма и обещает
(«токен — аудит-след, а НЕ подпись»), поэтому блокером не считаю. Но раз сам реестр поднял
вопрос до границы C, запись в `PENDING-SIGNATURE`/`decisions` была бы уместна — сейчас
подтверждение полномочия живёт в одной строке, которую §11 сам называет подделываемой.

---

## Покрытие §9 — пункт (в): связность и висячие ссылки

Правка закрыла §11 и §8 и оставила **три места, ставших после неё ложными**, два из которых —
нормативные и лежат на пути этого же коммита.

### B-1 (БЛОКЕР) — `commit-discipline.md:93` + `architect.md:23`: architect больше не может приземлить НИЧЕГО

Правка обновила пункт 4 списка «Auto-push» (reviewer) и **не тронула пункт 5** (architect):

`.claude/rules/commit-discipline.md:93-96`
```
5. **Architect пушит сам ТОЛЬКО чисто-процессные правки** (`.claude/rules/*`,
   `docs/04-workflow.md` и т.п.), не тронувшие код/контракты/риск.
```

С 2026-08-15 «пушит сам» в `main` невозможно — это доказано пробой (а.2). Инструкция предписывает
действие, которое отклоняется.

И это не просто устаревшая формулировка, которую агент починит догадкой. Единственный оставшийся
способ приземлить правку — `gh pr merge`, а профиль architect'а его **прямо запрещает**:

`.claude/agents/reviewer.md` — обновлён; `.claude/agents/architect.md:23` — нет:
```
- Не мержит PR и не пишет `PROJECT-STATE.md`/`TECH-DEBT.md` — это reviewer.
```

До правки противоречия не было: «пушить» и «мержить PR» — разные действия, PR'ов не
существовало. После правки они слились в одно, и architect оказался обязан пунктом 5 сделать то,
что запрещено строкой 23 его профиля. **Чисто-процессная правка теперь не имеет ни одного
разрешённого исполнителя.** Первый предмет, попавший в этот тупик, — сам `fc53ed7`.

Устранение (на выбор автора, зона его): либо пункт 5 переписать на «architect сам открывает и
мержит PR для чисто-процессных правок» + снять коллизию в `architect.md:23` (оговоркой «кроме
собственных процессных правок»), либо пункт 5 отменить и отправлять процессные правки к
reviewer'у. Молча оставить нельзя: обе нормы сейчас предписывают взаимоисключающее.

### B-2 (БЛОКЕР) — `gates.md` §9 предписывает fast-forward в `main`, который §8 той же правкой запретил

Внутри ОДНОГО файла, в 37 строках друг от друга:

`gates.md:293` (добавлено этой правкой): «**Merge в `main` идёт ТОЛЬКО через PR (с 2026-08-15).**»

`gates.md:372-375` (не тронуто):
```
Коммиты, уже сидящие на локальном `main`, публикуются
те же — под именем ветки (`git push origin main:docs/<topic>`); `main` уходит fast-forward
после перепроверки.
```
`gates.md:337` (не тронуто): «Всё остальное — изложение, нумерация, close-out-пруфы,
статус-колонки, орфография — **self-push автора**.»

«`main` уходит fast-forward после перепроверки» — прямой push, отклоняется (проба а.2). §9 — это
именно тот раздел, который описывает маршрут доc-правки в `main`, то есть маршрут **этого**
коммита. Правило, противоречащее себе через 37 строк, исполнить нельзя; агент выберет ту
формулировку, которую прочёл последней.

Отдельно: «self-push автора» для мелких правок (§9:337) после смены модели означает «автор сам
открывает и мержит PR». Это стоит назвать явно — иначе норма читается как сохранённое право
прямого push'а, которого больше нет.

### B-3 (MAJOR) — `docs/SESSION-HANDOFF.md:119-120` утверждает прямо противоположное

```
$ sed -n '119,120p' docs/SESSION-HANDOFF.md
`FOUNDER-APPROVED: <причина ≥12 символов>` в СВОЁМ теле; ... Merge это не блокирует —
branch protection недоступен (403, `TD-124`), контур предупреждающий.
```

Ложно по всем трём утверждениям: блокирует, доступен, барьерный. Вес находки задаёт не сам
файл, а его позиция: `CLAUDE.md:17` ставит `docs/SESSION-HANDOFF.md` **вторым пунктом
startup-протокола для КАЖДОЙ роли, ведущей сессию**. То есть отменённое утверждение читается
раньше, чем исправленное §11, и читается всеми. Файл в зоне `docs/**` — правится тем же
architect'ом, в том же круге; причин оставлять его расходящимся нет.

### N-1 — «`TD-124` снят» опережает реестр

`gates.md:409` утверждает: «`TD-124` снят». Реестр говорит иначе:

```
$ grep -n "TD-124" TECH-DEBT.md
748:- **TD-124** `блокировка-merge-отсутствует-как-класс-branch-protection-403-private-free`
```
Запись открыта — ни `✅ CLOSED`, ни даты (сравнение: `TECH-DEBT.md:354` — `**TD-135** ✅ **CLOSED
2026-08-14**`). Открытым `TD-124` числится и в `PROJECT-STATE.md:370,380`.

Смешаны две разные вещи: **причина** долга действительно исчезла (это доказано), **запись**
долга не закрыта. `TECH-DEBT.md` — reviewer-owned, architect закрыть её не вправе и правильно
не стал; но тогда и утверждать закрытие в `gates.md` нельзя. Корректно: «основание `TD-124`
отпало; закрытие записи — за reviewer'ом в ближайшем close-out». Иначе два документа, которые
агент читает подряд по startup-протоколу, противоречат друг другу.

### N-5 — `docs/plans/process-decisions-2026-08-14.md:270`

«**Блокировать нечем** (`TD-124`: branch protection 403) — контур предупреждающий» — тоже ложно.
Severity ниже: это датированный протокол решений от 14.08, то есть исторический журнал, а не
действующая норма. Достаточно пометки «снято 15.08», не переписывания.

**Что проверено и ложным НЕ стало** (чтобы объём находки не выглядел больше, чем он есть):
`CLAUDE.md:66` и `gates.md:255` («после push в `main`…», «push в `main` триггерит Actions») —
остаются верными: merge через PR порождает то же push-событие в `main`, `on: push: branches:
[main]` срабатывает. `.claude/agents/{engine,venue,research,signal}-dev.md` — пушат только
feat-ветки, их не задело; сверено грепом по всем девяти профилям. `.claude/wrappers/README.md:26`
(«reviewer мержит сессионную ветку в main») — верно, изменился лишь способ.

---

## Ответы по существу (заданы отдельно)

### 1. Ломает ли новая норма цепочку агентов — ДА, и это блокер

Профили `.claude/agents/*.md` прогреплены целиком. Прямое предписание приземлять в `main` было
ровно у одной роли — reviewer, и оно обновлено корректно (`reviewer.md:31,42`). Dev-роли
(`engine-dev.md:51-55` и остальные) пушат исключительно `feat/*` — не задеты, intra-chain push
сохранён явно и это верно.

Но цепочка рвётся не там, а на architect'е — см. **B-1**: пункт 5 `commit-discipline.md`
предписывает self-push в `main`, а `architect.md:23` запрещает единственную оставшуюся
альтернативу. Ответ на вопрос мандата «осталась ли хоть одна роль с прежней инструкцией» —
осталась, и это автор правки.

### 2. Противоречит ли `strict: false` норме «main всегда зелёный» — ДА, предел надо назвать явно

`strict: false` (замер а.1) означает буквально: требование «ветка актуальна относительно базы»
ВЫКЛЮЧЕНО. Следствие механическое и от внимательности не зависит:

- обязательный чек проверяется на **head-SHA ветки**;
- `gh pr merge --merge` (форма, которую правка предписывает) порождает **новый merge-коммит** —
  SHA, которого CI не видел никогда;
- при `strict: false` ветка может быть сколь угодно отставшей: зелёное снято на СТАРОЙ базе,
  а в `main` приезжает непроверенное дерево.

Дальше — точно тот сценарий, которым правка себя мотивирует. `deploy.yml:55-81` (`Gate on CI
(fail-closed)`) ждёт успеха CI **по тому же SHA**; красный merge-коммит деплой не пропустит.
Итог: `main` красный, деплой заморожен, коммиты копятся и не доезжают до прода — дословно
симптом `TD-135` («семь суток красного `main` и 49 коммитов, не доехавших до прода»), на который
правка ссылается как на устранённый.

То есть защита закрывает «красное **уже на момент проверки** не вливается» и НЕ закрывает
«зелёное на старой базе после слияния становится красным». Сейчас риск не реализован
(`git rev-list --count 41ba13c..origin/main` = 0, расхождения нет), но это состояние дня, не
свойство конструкции.

**Надо ли называть в правилах явным пределом — да, обязательно**, и по внутренней норме проекта:
§11 требует от правила объявлять свои пределы вместо того, чтобы «изображать гейт», а
`testing.md` («Целостность гейта») — предъявлять, против чего гейт покраснеет. Формулировка
уровня «`strict: false`: актуальность ветки не принуждается; перед merge отставшую ветку
обязан синхронизировать тот, кто мержит — иначе `main` краснеет уже ПОСЛЕ слияния» стоит одной
строки. Альтернатива — включить `strict: true`, но это решение founder'а (цена: каждый PR
пересобирается после каждого чужого merge, на 8 джобах с `cargo` это ощутимо), и я его не
принимаю за него; называю как развилку.

### 3. `required_pull_request_reviews` не включён — PR это церемония? — НЕТ

Прямой ответ: **не церемония, и текущая настройка правильная.**

PR несёт нагрузку, которой без него нет: `ci.yml:3-7` даёт CI ровно два триггера
(`pull_request: branches:[main]`, `push: branches:[main]`), `workflow_dispatch` в `ci.yml`
отсутствует. Значит зелёный `All checks passed` на не-`main` SHA **производится только открытием
PR**. Защита требует этот чек — PR является его единственным источником. Самослияние этого не
ослабляет: принуждается «дерево, которое приземляется, зелёное», а не «второй человек посмотрел».

Чего PR не добавляет — независимого взгляда. Но он его и не должен добавлять: независимость в
этом проекте несут ролевые гейты (§4 reviewer, §9 перепроверка, §0 арбитр), они когнитивные и
уже существуют. GitHub-ревью их не заменит.

Более того, включать `required_approving_review_count` здесь **нельзя**: GitHub запрещает автору
PR одобрять собственный PR, а все агенты работают под одной учётной записью (`gh auth status` →
`a3ka`). Второго одобряющего не существует физически — требование ревью не усилило бы гейт, а
заклинило цепочку намертво. `required_pull_request_reviews` выключен — это верный выбор, а не
пробел. (Ровно поэтому важна ловушка из а.7: саб-ресурс API показывает
`required_approving_review_count: 1`, и агент, сверившийся с ним вместо поведения, «починил» бы
несуществующую проблему в сторону блокировки всей работы.)

---

## Резюме находок

| № | Severity | Файл:строка | Суть |
|---|---|---|---|
| B-1 | **БЛОКЕР** | `.claude/rules/commit-discipline.md:93` + `.claude/agents/architect.md:23` | Пункт 5 предписывает architect'у self-push в `main` (невозможен), а профиль запрещает `gh pr merge` — единственную альтернативу. У процессной правки не осталось исполнителя |
| B-2 | **БЛОКЕР** | `.claude/rules/gates.md:374`, `:337` | §9 предписывает `main` fast-forward'ом и «self-push автора», §8 той же правкой это запретил. Самопротиворечие внутри файла, на маршруте этого же коммита |
| B-3 | MAJOR | `docs/SESSION-HANDOFF.md:119-120` | «branch protection недоступен (403), контур предупреждающий» — ложно по всем трём утверждениям; файл — п.2 startup-протокола ВСЕХ ролей |
| N-1 | NOTE | `.claude/rules/gates.md:409` | «`TD-124` снят» опережает реестр: `TECH-DEBT.md:748` открыт. Причина отпала — запись не закрыта (reviewer-owned) |
| N-2 | NOTE | `.claude/rules/gates.md:295-297` | Рецепт `gh pr checks <N>` c комментарием «ждать зелёного» не ждёт: замер → `exit=8` немедленно, нужен `--watch`. Плюс три голые строки без проверки кода возврата — форма, прямо запрещённая §3 |
| N-3 | NOTE | `.claude/rules/gates.md:293` | «Прямой push отклоняется — факт инфраструктуры»: защита требует зелёный чек на приземляемом SHA, а не PR. PR обязателен де-факто (CI не запускается вне PR/main), но по другой причине |
| N-4 | NOTE | `.claude/rules/gates.md:288-299` | `strict: false` — предел не назван: merge-коммит не тестировался, отставшая ветка вливается зелёной по старой базе → `main` краснеет ПОСЛЕ слияния → деплой fail-closed замерзает (симптом `TD-135`) |
| N-5 | MINOR | `docs/plans/process-decisions-2026-08-14.md:270` | «Блокировать нечем» — ложно; исторический журнал, достаточно пометки |
| N-6 | NOTE | `docs/PENDING-SIGNATURE.md` | `TD-124` объявлен границей C (`TECH-DEBT.md:751`), следа решения founder'а в репозитории нет — только токен в теле коммита |

**Условие APPROVED:** устранить B-1, B-2, B-3 (правка текста, зона автора). N-2, N-3, N-4
настоятельно рекомендую в том же круге — все три о том, что норма описывает механизм точнее,
чем сейчас; N-4 отдельно ценен тем, что называет единственную оставшуюся дыру в «main всегда
зелёный». N-1 — переформулировать, закрытие записи оставить reviewer'у. Повторного полного
круга §9 после текстовых правок не требуется — достаточно предъявить диф.

## Done Block

```
$ cd /tmp/recheck-prot && git log -1 --oneline
fc53ed7 docs(rules): merge в main — через PR; замок §11 стал БАРЬЕРОМ, а не предупреждением [architect]

$ git show --numstat --format='' fc53ed7
2	2	.claude/agents/reviewer.md
4	2	.claude/rules/commit-discipline.md
21	2	.claude/rules/gates.md

$ git log origin/main..fc53ed7 --format='%h %s'
fc53ed7 docs(rules): merge в main — через PR; замок §11 стал БАРЬЕРОМ, а не предупреждением [architect]

$ EVENT_NAME=push PUSH_BEFORE=41ba13c bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0

$ # анти-плацебо: тот же барьер на скретч-коммите зоны БЕЗ токена
$ EVENT_NAME=push PUSH_BEFORE=41ba13c bash scripts/check_docs_freeze.sh; echo exit=$?
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | tail -2; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ gh api repos/a3ka/hft-platform/branches/main/protection --jq '{strict:.required_status_checks.strict, contexts:.required_status_checks.contexts, enforce_admins:.enforce_admins.enabled}'
{"contexts":["All checks passed"],"enforce_admins":true,"strict":false}

$ git push origin fc53ed7:main
remote: error: GH006: Protected branch update failed for refs/heads/main.
remote: - Required status check "All checks passed" is expected.
exit=1

$ git push origin fc53ed7:refs/heads/probe/recheck-control
 * [new branch]      fc53ed7 -> probe/recheck-control
exit=0

$ gh pr checks 1 >/tmp/prchecks.txt 2>&1; echo exit=$?
exit=8

$ gh pr view 1 --json mergeStateStatus,reviewDecision
{"mergeStateStatus":"BLOCKED","reviewDecision":""}

$ bash scripts/next_artifact_id.sh R
R-085

$ df -h / | tail -1
/dev/md2        437G  284G  131G  69% /
```

Проба `probe/recheck-control` удалена (`git push origin --delete` → exit=0); скретч-коммит
анти-плацебо снят `git reset --hard fc53ed7`, дерево чистое.
