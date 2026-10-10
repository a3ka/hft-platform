<!-- GATE-META
milestone: M-65
audited_repo: a3ka/hft-platform
audited_base: 40c7cce30ff10eb787caaafccb4b809794c503ee
audited_head: 41c07e97b983b3f82bd5cdc897fc91203502293c
verdict: PASS
-->

# C-126 — CT-RFC-09 §2.6 correction after C-125: PASS

**Role:** critic, strong model — RAW gate (`gates.md` §1, contract-RFC class), fresh context.  
**Subject:** `fix/M-65-battery-recalibration` at `41c07e97b983b3f82bd5cdc897fc91203502293c`.  
**Merge preview:** `origin/main` `40c7cce30ff10eb787caaafccb4b809794c503ee` + subject → tree `1cf10a75271118c2e3a55723b655f940dec3a4cb`.

## Verdict: PASS

No new arbitration is required.  `41c07e9` does not alter the binding outcome of `A-015`
§3: its normative bullets remain, word for word in substance, (1) absent **or empty**
`GATEWAY_MAX_SUBSCRIPTIONS` → signed `16` plus a `warn` naming the variable, applied value,
and “продуктовая норма”; (2) `0`, negative, and non-numeric values → startup refusal.

The commit only removes the false claim that an empty value is rejected and explains why
empty and absent are the same outcome.  That correction executes the already binding
decision; it neither changes the signed value nor introduces a third behaviour.  It is not a
new factual dispute or a Boundary-C decision, so `A-015` §5 step 2 does not require a new
arbiter round.

## Findings and adversarial checks

1. The only current empty/rejected co-occurrence in the whole RFC says that the **previous
   revision** contained the defect found by C-125.  It is historical description, not an
   operative disposition.  The current §2.6 outcome for an empty value is uniquely `16 +
   warn`; the preserved `<details>` text is explicitly the prior edition and does not name an
   empty string.
2. §2.1–2.5, §2.7–2.9, and §6 contain no competing outcome for this variable.  Their other
   uses of “empty” concern a missing client selector or an empty snapshot, not
   `GATEWAY_MAX_SUBSCRIPTIONS`.
3. `docker-compose.yml:145` uses `${GATEWAY_MAX_SUBSCRIPTIONS:-16}`, so both a host-unset and
   host-empty value reach production as the bounded, explicit `16`.  No unbounded path is
   created.
4. Current `serve_config_from_env` and `red_max_subs_config.rs` still reject absent and empty
   values.  This is an acknowledged, staged contract/implementation mismatch—not a second
   RFC contradiction: A-015 §5 expressly orders the RED rewrite only **after** this critic
   gate (step 3), then implementation (step 4).  This PASS does not approve that old runtime
   behaviour; steps 3–4 remain mandatory before M-65 can advance.
5. The correction's sole diff hunk is at lines 138–146.  The original amendment's normative
   lines 123–125 are untouched; §6 is byte-identical to `f4f0a49`.  There are no changes to
   `crates/contracts/src/**` or schema JSON paths, so the seven-artifact T1 shape-change set
   is inapplicable and `CT-I-2` remains intact.

## Done Block

```text
$ RESERVE_ROLE=critic bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-126 …
C-126
reserve: резерв C-126 взят
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [6-RFC-SHA] … все 38 проверенных существуют И входят в историю HEAD/MERGE_HEAD
PASS  [7-RFC-PATH] … все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_ct_rfc_atomic.sh origin/main
PASS  crates/contracts/src/** не тронут — атомарность CT-RFC пакета не применима
VERDICT: PASS
exit=0

$ git diff --unified=0 41c07e9^ 41c07e9 -- docs/rfc/CT-RFC-09-ws-session.md
@@ -138,2 +138,9 @@
-  `0`, отрицательное и пустая строка отвергаются и после амендмента. …
+  `0` и отрицательное отвергаются и после амендмента. …
+  **Пустая строка — то же, что отсутствие, и это НЕ оговорка.** `A-015` §3 п.1 …

$ git show 41c07e9:docs/rfc/CT-RFC-09-ws-session.md | sed -n '121,126p'
- `max_subscriptions_per_connection` — **АМЕНДМЕНТ 2026-08-22 по арбитражу `A-015`**
  - **отсутствие переменной И пустая строка ⇒ подписанная норма `16`** с наблюдаемой
    записью при старте (`warn`: имя переменной, применённое значение, «продуктовая норма»);
  - **невалидное ЗНАЧЕНИЕ** (`0`, отрицательное, нечисловое) ⇒ **отказ старта** — без
    изменений.

$ for rev in f4f0a49 41c07e9; do git show "$rev:docs/rfc/CT-RFC-09-ws-session.md" | awk '/^## 6\./,/^## 7\./' | sha256sum; done
c1cdd01cf8e2f1c9a858f55c8d2ab6fd61c9855f853fff7f88b86acc768bc236  -
c1cdd01cf8e2f1c9a858f55c8d2ab6fd61c9855f853fff7f88b86acc768bc236  -

$ git diff --quiet a9f0bf5...41c07e9 -- crates/contracts/src ':(glob)**/schema/*.json'; echo exit=$?
exit=0

$ grep -n -F GATEWAY_MAX_SUBSCRIPTIONS docker-compose.yml
145:      GATEWAY_MAX_SUBSCRIPTIONS: ${GATEWAY_MAX_SUBSCRIPTIONS:-16}
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-22T23:00Z
- Milestone: M-65-ws-session
- Статус: DONE (contract-RFC correction PASS; implementation remains pending)
- HEAD: 41c07e9 — C-125 correction [architect]

## §B — Что я сделал
- Audited the corrected §2.6 on its merge-preview and checked every empty/absent reference in the RFC.
- Confirmed the correction executes A-015 rather than changing it; no arbiter trigger.

## §C — Артефакты / результаты
- `research/critiques/C-126-ct-rfc-09-a015-correction.md`
- Done Block: required merge-preview gates both exit 0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  A-015 contract-RFC gate is PASS at C-126 on fix/M-65-battery-recalibration. Execute A-015
  §5 step 3 only: rewrite the sacred red_max_subs_config oracle and M-65 verify step so absent
  and empty start with literal 16 + warn, invalid values refuse startup, and the RED tests fail
  against the current B implementation. Do not write implementation; next step is engine-dev.
  ```
- Push-статус: pending this verdict commit and push to `origin/fix/M-65-battery-recalibration`.
- Кэш: no `target/` created in this critic worktree.

## §E — Риски / открытые вопросы
- M-65 runtime code deliberately remains B until A-015 steps 3–4; it must not be represented as approved implementation.

=== END HANDOFF ===
