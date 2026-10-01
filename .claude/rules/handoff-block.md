# Handoff Block — обязательный формат

Источник: `docs/04-workflow.md` §6 + `CLAUDE.md` "founder = orchestration dispatcher".
Каждый агент (architect, critic, risk-critic, engine/venue/signal/research-dev, tester,
reviewer), завершающий ответ, где есть передача дальше, обязан закончить его этим блоком
— **последняя секция ответа**, ничего после `=== END HANDOFF ===`.

Founder не вызывает агентов друг через друга — он копирует §D paste-ready промпт
следующему агенту. Блок должен быть самодостаточным: следующий агент читает ТОЛЬКО §D,
без "смотри выше".

## Обязательная структура (5 секций)

```text
=== HANDOFF: <FROM-ROLE> → <TO-ROLE> ===

## §A — Метаданные
- Дата (UTC, ISO-8601): <YYYY-MM-DDTHH:MMZ>
- Milestone: <M-NN-<name>>
- Статус: <PROPOSED | IN_PROGRESS | DONE | BLOCKED>
- HEAD: <short-SHA — subject>

## §B — Что я сделал
- <пункт 1>
- <пункт 2>

## §C — Артефакты / результаты
- <файлы созданы/изменены>
- Done Block exit-коды (см. `.claude/rules/commit-discipline.md`), либо "N/A — read-only роль"

## §D — Следующий агент + инвокация
- **Следующий агент:** `<role>`
- **Paste-ready промпт:**
  ```
  <самодостаточный промпт — милестоун, зона, acceptance-критерий>
  ```

## §E — Риски / открытые вопросы
- <блокер, если Статус=BLOCKED>
- <известное ограничение>
- N/A, если нечего сообщить

=== END HANDOFF ===
```

## Правила

1. **Позиция** — всегда последняя секция ответа.
2. **Все 5 секций присутствуют.** Пусто → пиши `- N/A`, не пропускай секцию.
3. **§D paste-ready промпт** самодостаточен — без "как указано выше". Следующий агент
   его копирует и запускает без доразбора контекста.
4. **Push-статус в §D** (аналог EINHARD F-032): явно указывать одно из:
   - `✅ pushed to origin/main at <SHA>` — если гейты зелёные и агент запушил сам
     (`.claude/rules/commit-discipline.md` "Auto-push").
   - `⏸ commits ready; next agent in chain will push` — коммиты есть, push не мой шаг.
   - `⚠ NOT pushed; blocked by gate <gate-id>` — с явным указанием, какой гейт не прошёл
     (`.claude/rules/gates.md`), и что нужно для разблокировки.
5. **Один Handoff Block на ответ.** Если передача нескольким ролям (например,
   engine-dev параллельно venue-dev) — перечислить как нумерованные инвокации в §D,
   не дублировать весь блок.
6. **Кэш сборки убран — строкой в §D, рядом с push-статусом.** `rm -rf <свой worktree>/target`,
   и в блоке `✅ кэш убран` либо `⏸ кэш оставлен — <причина>`. Правило живёт в
   `branch-hygiene.md` §Worktree lifecycle п.3, но СРАБАТЫВАТЬ обязано здесь: момент сдачи
   работы — единственный, когда агент точно знает, что его дерево больше не нужно, а этот
   файл — единственный, который он в этот момент гарантированно перечитывает. Замер, из-за
   которого пункт появился: диск на 100 %, вся работа в 165 каталогах — 782 MB, кэши — 105 GB.
   `--reclaim` здесь НЕ подходит: свой кэш свежий, он его не тронет.

## RISK-BLOCK и signed-decision handoff'ы — особый случай

Если milestone/предложение требует founder-подписи (`docs/DESIGN.md` §6, граница C —
`Ctl(ParamChange)`, промоушен сигнала `candidates→paper→live`), §D называет
**следующим агентом founder**, а §E явно перечисляет, что именно требует подписи
(`{what, from, to, rationale, report_ref}` — формат из
`docs/03-integration-contract.md` §3). Никакой агент не подставляет "автоматическое
approve" вместо явной founder-подписи.

## Пример (dev → tester)

```text
=== HANDOFF: ENGINE-DEV → TESTER ===

## §A — Метаданные
- Дата: 2026-07-10T18:40Z
- Milestone: M-01-journal-p0
- Статус: DONE (impl завершён; тестер-гейт pending)
- HEAD: a1b2c3d — feat(M-01): task #4 — replay determinism harness

## §B — Что я сделал
- Реализовал append-only journal writer + replay reader (crates/journal/src/)
- 3 атомарных коммита по задачам #2-#4

## §C — Артефакты / результаты
- crates/journal/src/{writer.rs,reader.rs,replay.rs}
- Done Block: cargo test -p journal → 12 passed; verify_M-01.sh exit=0

## §D — Следующий агент + инвокация
- **Следующий агент:** `tester`
- **Paste-ready промпт:**
  ```
  Прогони M-01-journal-p0 на чистом чекауте: cargo test -p journal --workspace,
  затем bash scripts/verify_M-01.sh; echo exit=$?. Ожидание: DET-I-1 GREEN
  (replay ×3 бит-идентичен). Верни PASS/FAIL verdict + Done Block.
  ```
- Push-статус: ⏸ commits ready; reviewer запушит после APPROVED

## §E — Риски / открытые вопросы
- N/A

=== END HANDOFF ===
```

## Cross-references

- `docs/04-workflow.md` §6 (шаблон), §1 (роли)
- `.claude/rules/commit-discipline.md` (Done Block перед handoff)
- `.claude/rules/gates.md` (какой гейт блокирует передачу дальше)
- `.claude/rules/scope-guard.md` (SCOPE VIOLATION REQUEST — отдельный формат, не Handoff)
