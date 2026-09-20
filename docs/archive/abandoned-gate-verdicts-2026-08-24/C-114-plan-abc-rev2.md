<!-- GATE-META
milestone: C-113
audited_repo: a3ka/hft-platform
audited_base: f6e9337b90e3e262e9006495764452e7122f9924
audited_head: fc99fd6ed8f698e9fcb059a2f5798c26c1a19b73
verdict: ESCALATE
-->

# C-114 — PLAN-ABC rev2: ESCALATE

## Предмет, граница и набор артефактов

Аудирован commit-chain `f6e9337..fc99fd6` на
`origin/docs/plan-abc-2026-08-20`, а не handoff как источник фактов. По §0
плана не вынесен вердикт о порядке полос, сроках либо сессиях. Проверены
заявленные снятия B1–B4, §4.3 и числа §10.

Полный mandatory-набор plan-time critic отсутствует. Двухточечный diff содержит
только `docs/plans/plan-A-B-C-2026-08-20.md` и предыдущий `C-113`; в нём нет
закоммиченных T-контракта/trait-signature, RED, `verify_M-*.sh` либо milestone
файла для resync integrity / исполнения П-014. Поэтому этот chain не может
разрешить dispatch какой-либо реализации: §4.4 — полезный проект контракта, не
предъявленный RED-артефакт. Это ровно порог `critic.md`: `NOT REVIEWED —
ARCHITECT ARTIFACTS INCOMPLETE` для dev-dispatch.

Проверены живые FA-инварианты `VB-I-2`, `VB-I-5` и `VB-I-10`
(`docs/fa/viz-backend.md:115-125`).

## Подтверждённые исправления

### §4.1 и §4.3 — сам диагноз и поправка VB-I-2 верны

`DiffAction::Gap` сбрасывает venue-книгу, буферизует diff и запрашивает REST
snapshot (`crates/venue-binance/src/lib.rs:254-266`); REST жёстко получает
`limit=5000` (`:27`, `:852-855`). После reconciliation `tick()` смотрит только
на `state.book` и event-time, не на `resyncing` (`:366-409`), а gateway заменяет
свою книгу этим `L2Snapshot` (`crates/gateway/src/lib.rs:903-930`; `book` чистит
обе стороны в `crates/book/src/lib.rs:79-94`). Источник данных даёт BTC
`-1.29%/+1.33%` и ETH `-4.50%/+4.40%`
(`research/data-quality/depth-probe-binance.md:120-131`). Следовательно новая
формулировка о symbol-dependent REST-окне корректна; текущий default 0.1% также
подтверждён (`docker-compose.yml:134,197`).

Architect прав по §4.3: я снимаю часть C-113, относившую этот дефект к
`VB-I-2`. Live reducer применяет journal events в `LiveReducer::pump`
(`crates/gateway/src/lib.rs:3166-3194`), а `replay()` применяет тот же
`Reducer::apply` к тому же journal stream (`:1902-1919`). Усечённый snapshot
искажает общий вход обоих путей, но не предъявлен путь, который делает их
разными. Затронут `VB-I-5`, а не доказано нарушение `VB-I-2`.

`docs/fa/viz-backend.md:89-95` действительно всё ещё называет З-1 suspended и
снимаемым автоматически через M-58, тогда как `docs/PENDING-SIGNATURE.md:768-773`
фиксирует снятие З-1 явной founder-подписью. Новое предусловие §4.2bis названо
честно; `scripts/verify_M-58.sh:190-198` действительно ещё проверяет старый
default-lock.

### B3 — удаление не разрешено

§6.2 больше не выдаёт удаление ref'ов за готовое действие и требует per-ref
артефакт до отдельного critic-круга. Проверено: `/home/nous/salvage-2026-08-19`
отсутствует, а двухточечный `--diff-filter=A` для `feat/M-10-rebased` всё ещё
даёт три файла. Следовательно У-1 не выполнено и никакое удаление не должно
начинаться. Этот участок rev2 не является основанием для dispatch.

## Нерешённые основания эскалации

### E1 — B1 не может считаться снятым контрактом, которого в chain нет

§4.4 (`docs/plans/plan-A-B-C-2026-08-20.md:154-175`) задаёт шесть сценариев,
но это только проза. В chain нет ни RED-теста, ни trait/T2 решения, ни verifier;
`cargo test -p venue-binance --lib` прогоняет 18 существующих unit-тестов и не
содержит resync scenario. Кроме отсутствия артефакта, S3–S5 не задают ожидаемый
GREEN результат после усечённого snapshot: перечислены входы, но не определено,
когда/что разрешено эмитить либо какую полноту надо доказать. Поэтому заявленные
«красный сейчас» и compiled-run `60.000% → 1.300%, 5.00 → 4.00` в §4.1 не
воспроизводятся из §10. Строка `$ ssh …` там не является исполнимой командой.

Условие снятия: до dispatch закоммитить отдельный milestone set с T2/signature
decision, production-seam RED через `on_ws_text` → `on_snapshot_result` →
`tick`, конкретным post-gap GREEN свойством, setup/absence guard, детерминизмом
и mutation proof; verifier обязан запускать его с exit-кодом. Либо оставить
§4.4 явно будущим требованием и не называть B1 снятым.

### E2 — §4.5 неполно и ложно называет сокращение «дословным»

План утверждает, что дословно переносит общее условие C-094
(`docs/plans/plan-A-B-C-2026-08-20.md:192-196`), но обрывает его после
«полной CI-parity». Оригинал требует также **«проверкой каждого task»**
(`origin/feat/M-68-depth-from-book:research/critiques/C-094-M-68.md:117-121`).
Кроме того, в пересказе B4 пропущены её самостоятельные маршрутные факты:
непредъявленный П-011 amendment и единственный положительно подтверждённый
узкий факт — M-68 не менял `GATEWAY_BANDS`, З-1 оставался в силе (`C-094:85-89`).
Это противоречит словам §4.5:198-209, что перенесены *все* маршрутные следствия.

Условие снятия: вернуть в §4.5 полный condition-to-clear без слова «дословно»
над сокращением, включая проверку каждой task, и явно передать весь остаток B4
либо объяснить с первоисточником, почему он не маршрутный. `C-094` по-прежнему
не закрыт и dev по M-68 запрещён.

### E3 — B4: K-4 не обеспечивает APPROVED, а байтовый бюджет неверен

§6.5:323-339 верно перестал называть строку Handoff механизмом, но затем
приписывает `check_gate_meta.sh` физическое обеспечение обязательного reviewer
`APPROVED`. K-4 ищет только *какой-либо* `research/reviews/R-*.md`, содержащий
литерал `M-NN` (`scripts/check_gate_meta.sh:265-291`). Он не читает verdict
`APPROVED` и не проверяет роль автора. Следовательно он не доказывает требование
`gates.md:152-178,205-206`; обязательность reviewer approval должна быть либо
правильно маркирована `COGNITIVE-ONLY` с причиной, либо обеспечена реальным
механизмом, который наблюдает его отсутствие.

Также таблица §6.5 называет свой дифф точным, но его же литералы дают `и делает
push` = 20 B и `; приземляет architect` = 32 B (без перевода строки), то есть
+12 B, не +11 B. Вместе с +1 B первой правки из сегодняшнего запаса 17 B
получается 4 B, не заявленные 5 B. Для +246 B и +224 B точный новый текст не
приведён, поэтому повторный подсчёт невозможен. Процессная норма в зоне §11 не
готова к изменению по такой спецификации.

Условие снятия: предъявить буквальные три диффа и пересчитанные байты; не
приписывать K-4 то, чего он не наблюдает. Затем выбрать механизм для APPROVED
или честно пометить именно эту норму `COGNITIVE-ONLY` по
`binding-requires-mechanism.md`, не выдавая audit-trace за enforcement.

### N1 — §10 всё ещё не воспроизводит число артефактов

Заявленная команда `ls research/critiques research/reviews research/arbitration |
grep -c '\\.md$'` возвращает 184, а не 80. Число 80 подтверждается другой
командой: `git log origin/main --since='14 days ago' --diff-filter=A --name-only
--format='' -- research/critiques research/reviews research/arbitration | sort -u
| grep -c .`. Заменить Done Block на фактически исполнимую команду с её выводом;
`ssh …` также заменить точной read-only командой либо убрать из доказательства.

## Verdict

**ESCALATE — dispatch запрещён.** B3 снят; фактический диагноз §4.1 и поправка
`VB-I-2` верны. Но полный artifact set отсутствует, а E1–E3 оставляют B1, B2 и
B4 неснятыми. Это второй круг по тем же основаниям C-113; согласно профилю
critic следующий разбор идёт к арбитру, а не в третий самостоятельный REJECT.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-114
exit=0

$ git rev-parse HEAD
fc99fd6ed8f698e9fcb059a2f5798c26c1a19b73
exit=0

$ git diff --name-status f6e9337..fc99fd6
A  docs/plans/plan-A-B-C-2026-08-20.md
A  research/critiques/C-113-plan-abc.md
exit=0

$ git diff --name-only f6e9337..fc99fd6 -- crates contracts scripts milestones
<empty>
exit=0

$ cargo test -p venue-binance --lib -- --nocapture
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ git log origin/main --since='14 days ago' --format='%H' | wc -l
433
$ git log origin/main --since='14 days ago' --format='%H' -- 'crates/*/src/**' | wc -l
14
$ git log origin/main --since='14 days ago' --name-only --format='' -- 'crates/*/src/**' | sort -u | grep -c .
4
$ git show origin/feat/M-68-depth-from-book:research/critiques/C-094-M-68.md | grep -cE '^### B[0-9]'
6
exit=0

$ test ! -e /home/nous/salvage-2026-08-19; echo $?
0
$ git diff --name-status --diff-filter=A origin/main origin/feat/M-10-rebased
A  crates/research-cli/tests/red_killscreen.rs
A  crates/research-cli/tests/red_stack_honesty.rs
A  research/reports/R-001-obi-trackA.md
exit=0

$ printf '%s' 'и делает push' | wc -c; printf '%s' '; приземляет architect' | wc -c
20
32
$ printf '%s' 'которую reviewer мержит' | wc -c; printf '%s' 'которую architect мержит' | wc -c
37
38
exit=0

$ ls research/critiques research/reviews research/arbitration | grep -c '\\.md$'
184
$ git log origin/main --since='14 days ago' --diff-filter=A --name-only --format='' -- research/critiques research/reviews research/arbitration | sort -u | grep -c .
80
exit=0

$ bash scripts/check_context_budgets.sh | grep -E 'gates|commit-disc|handoff'
OK    .claude/rules/commit-discipline.md          8579 B / 9000 B (421 B)
OK    .claude/rules/gates.md                     47883 B / 47900 B (17 B)
OK    .claude/rules/handoff-block.md              6610 B / 6900 B (290 B)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=f6e9337b90e3e262e9006495764452e7122f9924 bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

§A — Метаданные

- Дата (UTC, ISO-8601): 2026-08-20T12:53Z
- Milestone: C-113
- Статус: ESCALATE — второй круг C-113, dispatch запрещён
- HEAD: fc99fd6ed8f698e9fcb059a2f5798c26c1a19b73

§B — Что проверено

- §4.1: resync действительно делает REST-ограниченный snapshot эмитируемым; §4.3 корректно
  снимает ложную часть C-113 о `VB-I-2`.
- B3: У-1 отсутствует и удаление ref'ов из плана изъято.
- Найдены E1–E3 и N1 выше: неполный artifact set, сокращённое condition-to-clear C-094,
  K-4 без проверки APPROVED, неверный byte count и неисполняемая команда §10.

§C — Артефакт / результаты

- `research/critiques/C-114-plan-abc-rev2.md` — этот вердикт.
- `cargo test -p venue-binance --lib` — 18 passed, exit 0.
- `check_gate_meta`, `check_context_budgets`, `verify_design_claims --merge-preview` — exit 0;
  они не доказывают E1–E3.

§D — Следующий агент + инвокация

- Следующий агент: arbiter.
- Paste-ready промпт:

  ```text
  Ты — арбитр второго круга C-113. Работай от fc99fd6 на
  origin/docs/plan-abc-2026-08-20, база f6e9337. Прочитай целиком
  research/critiques/C-113-plan-abc.md и C-114-plan-abc-rev2.md, затем первичные
  C-094, critic.md, A-011, gates.md §4/§9/§11 и binding-requires-mechanism.md.

  Реши три вопроса, не редактируя plan:
  (1) может ли PLAN-ABC получить plan-time пропуск без закоммиченного T/trait/RED/verify/
      milestone set, когда он объявляет C-2 и П-014 предусловиями будущей реализации;
  (2) обязан ли §4.5 перенести из C-094 буквально «проверкой каждого task» и остаток B4:85-89;
  (3) достаточен ли K-4, который проверяет только существование R-файла с M-NN, как механизм
      обязательного reviewer APPROVED, и как считать указанный буквальный byte diff.

  Вынеси committed arbitration artifact с raw commands, условиями исполнения и явным
  маршрутом: architect либо исправляет набор/план, либо document остаётся несудимым
  расписанием без разрешения dispatch. До решения не разрешай C-2, П-014, M-68,
  удаление ref'ов или процессную правку §11.
  ```

- Push-статус: ✅ pushed to `origin/docs/plan-abc-2026-08-20` — C-114 critic commit.
- Кэш: ✅ `/tmp/hft-critic-plan2/target` удалён; `df -h /` = 81%.

§E — Риски / открытые вопросы

- P-014 не исполняется: resync integrity и FA-против-подписи остаются предусловиями.
- У-1 отсутствует; восстановление или пересмотр условия — отдельный предмет удаления.
- Повторный саморедактирующий круг по B1/B2/B4 запрещён профилем critic; требуется
  арбитражное решение, а не третий REJECT.
