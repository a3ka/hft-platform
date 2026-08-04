# Commit Discipline — атомарные коммиты, Done Block, auto-push

Источник: `docs/04-workflow.md` §4, `CLAUDE.md` "Операционные принципы". Применимо к
каждому агенту, пишущему код/тесты/докс в рамках milestone'а.

## Атомарные коммиты

- Одна задача из §Tasks milestone'а = **минимум один коммит**. Бандл на несколько задач
  одним коммитом = авто-reject reviewer'ом.
- Формат subject: **conventional commit** `type(scope): subject`, где `scope` —
  milestone/крейт: `feat(M-05): task #3 — risk gate pre-trade barrier`.
- Обязательная ссылка на milestone/task в теле или subject (не голое "wip"/"fix"/"checkpoint").
- Разные задачи, даже мелкие и связанные, — разные коммиты, в порядке зависимостей
  milestone'а.
- Quality-fix, покрывающий несколько задач (`cargo clippy`/`cargo fmt` cleanup), —
  ОДИН коммит с явной пометкой диапазона: `fix(M-05): clippy cleanup across tasks 2-4`.

## Без co-author трейлера

**НИКОГДА** не добавлять `Co-Authored-By: ...` в тело коммита — это переопределяет
дефолт харнесса (`CLAUDE.md` "Commit protocol"). Founder-инструкция BINDING.

## Роль в коммите — метка в subject'е, не git-личность

Автор всех коммитов — владелец репозитория (глобальный `~/.gitconfig`); роль указывается
меткой в конце subject'а: `feat(M-NN): task #k — <...> [engine-dev]`. Ролевые `user.name`
не выставляются.

Прежнее правило («git identity = роль») отменено замером: личность ставится один раз при
создании worktree и наследуется копированием — все 14 worktree несли подпись `reviewer`
независимо от того, кто в них работал, включая dev'ские. Как признак роли git-личность не
работает в принципе, а не «работает криво» (`R-031` §C.3 — развести постфактум reviewer'а и
dev'а по логу невозможно). Аудит-трейл держится на предъявляемых артефактах гейтов
(`gates.md` §4) и метке в subject'е. Полное правило — `branch-hygiene.md` п.6.

## Done Block — обязателен перед "готово"

Перед тем как сказать "done"/"готово"/"ready for review", агент вставляет **сырой
stdout**, не пересказ:

```
## Done Block

$ git status --porcelain
{пусто}

$ git log -1 --oneline
{<hash> <subject>}

$ cargo test -p <crate> 2>&1 | tail -10
{N passed; 0 failed}

$ cargo clippy -p <crate> -- -D warnings 2>&1 | tail -5
{пусто / явный TD-ref при известном допустимом отклонении}

$ bash scripts/verify_M-NN.sh; echo "exit=$?"
{... VERDICT: PASS
exit=0}
```

- Пересказ ("тесты прошли", "verify зелёный") без сырого вывода = "NOT REVIEWED —
  RESUBMIT WITH DONE BLOCK" от reviewer'а.
- **Сырой ≠ ВЕСЬ (закреплено 2026-07-14, инцидент: tester трижды оборвался на печати).**
  Полный дамп `cargo test --workspace` (десятки блоков) и полный вывод verify упираются в
  лимит ответа и УБИВАЮТ агента до Handoff'а. Сырой вывод обязан быть **агрегированным**:
  - тесты: `cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END
    {print "passed="p" failed="f" (блоков: "NR")"}'` + ПОЛНЫЙ вывод только упавших блоков;
  - verify: `bash scripts/verify_M-NN.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?`
    (при FAIL — добавить хвост упавшей проверки);
  - clippy/fmt: `... 2>&1 | tail -5; echo exit=$?`.
  Правило: **каждое утверждение подкреплено командой и её exit-кодом**; объём — минимальный,
  достаточный для перепроверки. Зелёное агрегируется, красное печатается целиком.
- Ожидаемые/известные FAIL (например RED-тест ещё не реализованного риск-инварианта в
  ранней задаче того же milestone'а) — перечисляются явно с обоснованием, не скрываются
  за общим `tail`.
- Работа в процессе НЕ называется "done" — используй "WIP, blocking on X" + список того,
  что осталось. WIP-отчёт не требует Done Block, но требует явный список остатка.

## Auto-push — только при зелёных гейтах

Агент имеет право сам сделать `git push` ТОЛЬКО когда:

1. `scripts/verify_M-NN.sh` — exit 0.
2. `git status --porcelain` — пусто (рабочее дерево чистое).
3. Для milestone'ов, тронувших `risk`/`killswitch`/`oms`/`venue-*`/`contracts` —
   risk-critic вердикт PASS (или CONCERNS явно принят founder'ом) И reviewer APPROVED
   уже в цепочке (`.claude/rules/gates.md` §4/§5).
4. Reviewer (не dev, не architect) — последний в цепочке для substantive-изменений;
   именно reviewer делает push после APPROVED и обновляет `PROJECT-STATE.md` +
   `TECH-DEBT.md`.
5. **Architect пушит сам ТОЛЬКО чисто-процессные правки** (`.claude/rules/*`,
   `docs/04-workflow.md` и т.п.), не тронувшие код/контракты/риск. Правка, меняющая ФОРМУ
   архитектурного документа (инварианты, границы, фазы, новая milestone-спека), идёт через
   critic по триггерам `gates.md` §9.

Если любой гейт не прошёл — **не пушить**, вернуть Handoff §E с явным блокером
(`.claude/rules/handoff-block.md`).

## Что нарушение выглядит как

- Один коммит покрывает 4 задачи milestone'а без пометки "quality-fix across tasks".
- Subject без ссылки на milestone/task ("fix bug", "update").
- `Co-Authored-By` трейлер в теле.
- "Готово" без Done Block или с пересказом вместо сырого вывода.
- Push при `cargo test` FAIL или при незакрытом risk-critic вердикте на
  risk/killswitch/oms/venue milestone'е.

## Cross-references

- `.claude/rules/gates.md` §4 (PR-time reviewer UNCONDITIONAL), §5 (RISK-BLOCK)
- `.claude/rules/handoff-block.md` (§D push-статус в Handoff)
- `docs/04-workflow.md` §4
