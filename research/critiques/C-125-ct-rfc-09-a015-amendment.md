<!-- GATE-META
milestone: M-65
audited_repo: a3ka/hft-platform
audited_base: 40c7cce30ff10eb787caaafccb4b809794c503ee
audited_head: f4f0a495d68e63b31a414a981cb6b473d6f927d7
verdict: REJECT
-->

# C-125 — CT-RFC-09 §2.6 amendment under A-015: REJECT

**Role:** critic, strong model — RAW gate (`gates.md` §1, contract-RFC class), fresh context.  
**Subject:** `fix/M-65-battery-recalibration` at `f4f0a495d68e63b31a414a981cb6b473d6f927d7`.  
**Merge preview:** `origin/main` `40c7cce30ff10eb787caaafccb4b809794c503ee` + subject → tree `0882b29e617424696553d95b7a6863239832f722`.

## Verdict: REJECT

`A-015` §3 is mandatory and says exactly: absence **and an empty string** yield the signed
value `16` with a `warn`; only `0`, negative, and non-numeric values refuse startup.  The
new normative bullets at RFC lines 123–126 correctly reproduce that decision.  But the
new explanatory paragraph at lines 135–140 says the opposite: that “мусор, `0`,
отрицательное **и пустая строка отвергаются** и после амендмента.”

Thus one §2.6 amendment assigns two mutually exclusive outcomes to an empty value.  An empty
value cannot both select bounded signed default `16` and be rejected at startup.  This is not
a harmless wording issue: the explanation is the stated reason the RFC claims to preserve
fail-closed behaviour, and is available to the architect who must next rewrite the sacred RED
oracle and verify step.  It does **not** open an unbounded path—the explicit default is bounded
at `16`—but it makes the contract internally contradictory about the required implementation.

This is a new fact against the artifact, not a choice of implementation direction.  Under
`A-015` §5 step 2 and `gates.md` §0, this REJECT returns to the independent arbiter; neither
side should self-select a textual resolution.  Steps 3–5 of A-015 remain blocked.

## Confirmed, non-blocking facts

- The commit changes only `docs/rfc/CT-RFC-09-ws-session.md`.  Its numstat is `31` additions
  and `2` deletions: the two removed pre-amendment lines are the sole deleted content, and the
  only hunk begins at original lines 121–122.  The added audit trail and rationale are within
  that §2.6 hunk; §6 and the original following lines remain byte-identical.
- The old rule existed in original commit `5bf2b4b` on 2026-08-03.  Founder signatures were
  added later by `50dae79` on 2026-08-11, so the chronology claimed by the amendment is
  confirmed.
- No `crates/contracts/src/**` or schema JSON path changes exist in either the amendment or
  the full `origin/main...f4f0a49` range.  `verify_ct_rfc_atomic.sh` passes, so the seven-item
  T1 shape-change package is correctly inapplicable.  `CT-I-2` remains respected.
- The milestone, existing RED test, and M-65 verify script are present.  Their required
  rewrite is intentionally *after* this contract-RFC gate in A-015 §5 steps 3–4; they are not
  evidence that the new behaviour is already implemented.
- I agree with A-015 §4 that this amendment is not Boundary C: the signed value remains `16`,
  compose always passes `GATEWAY_MAX_SUBSCRIPTIONS` (defaulting it to `16`), and no money,
  signal parameter, or live-trading control is changed.  The disputed case is only an omitted
  configuration outside that production compose path.
- The amended bullets require the missing/empty fallback to log `warn` with the variable,
  applied value, and “продуктовая норма”.  Invalid values remain startup failures.  The
  contradiction above—not an unbounded fallback—is the blocker.

## Required route

**Next agent: independent arbiter, fresh strong-model context.**  Read `A-015` and this
verdict as primary artifacts; determine whether the contradictory “пустая строка
отвергается” claim is compatible with the binding §3 decision.  The result must be committed
as a new arbitration artifact on `fix/M-65-battery-recalibration`.  Do not dispatch A-015
steps 3–5 until that decision exists.

## Done Block

```text
$ RESERVE_ROLE=critic bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-124 …
reserve:   C-124 занят; следующий кандидат — C-125
reserve: попытка 2/8 — C-125 …
C-125
reserve: резерв C-125 взят
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [6-RFC-SHA] SHA-подобных токенов … все 38 проверенных существуют И входят в историю HEAD/MERGE_HEAD
PASS  [7-RFC-PATH] путей-кандидатов … все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_ct_rfc_atomic.sh origin/main
PASS  crates/contracts/src/** не тронут — атомарность CT-RFC пакета не применима
VERDICT: PASS
exit=0

$ git diff --numstat f4f0a49^ f4f0a49
31	2	docs/rfc/CT-RFC-09-ws-session.md

$ git diff f4f0a49^ f4f0a49 | grep '^-'
--- a/docs/rfc/CT-RFC-09-ws-session.md
-- `max_subscriptions_per_connection` — конфиг, отсутствие/невалидное значение ⇒ **отказ
-  старта** (`gates.md`: «parse-error → unbounded — запрещено», урок R7);
exit=0

$ git show 5bf2b4b:docs/rfc/CT-RFC-09-ws-session.md | sed -n '104,114p'
### 2.6 Лимиты (fail-closed)
- `max_subscriptions_per_connection` — конфиг, отсутствие/невалидное значение ⇒ **отказ
  старта** (`gates.md`: «parse-error → unbounded — запрещено», урок R7);

$ git show -s --date=iso-strict --format='%H %ad' 5bf2b4b 50dae79
5bf2b4b24dffc36d23ee09d193e97dc439bc8990 2026-08-03T16:51:59+00:00
50dae797ffd54eda4c2b4498b2c5c761d7ef0aca 2026-08-11T21:50:04+00:00

$ git diff --quiet f4f0a49^ f4f0a49 -- crates/contracts/src ':(glob)**/schema/*.json'; echo exit=$?
exit=0

$ for rev in f4f0a49^ f4f0a49; do git show "$rev:docs/rfc/CT-RFC-09-ws-session.md" | awk '/^## 6\\./,/^## 7\\./' | sha256sum; done
c1cdd01cf8e2f1c9a858f55c8d2ab6fd61c9855f853fff7f88b86acc768bc236  -
c1cdd01cf8e2f1c9a858f55c8d2ab6fd61c9855f853fff7f88b86acc768bc236  -

$ grep -n -F 'GATEWAY_MAX_SUBSCRIPTIONS' docker-compose.yml
145:      GATEWAY_MAX_SUBSCRIPTIONS: ${GATEWAY_MAX_SUBSCRIPTIONS:-16}

$ nl -ba docs/rfc/CT-RFC-09-ws-session.md | sed -n '122,140p'
123	  - **отсутствие переменной И пустая строка ⇒ подписанная норма `16`** с наблюдаемой
124	    записью при старте (`warn`: имя переменной, применённое значение, «продуктовая норма»);
125	  - **невалидное ЗНАЧЕНИЕ** (`0`, отрицательное, нечисловое) ⇒ **отказ старта** — без
138	  `0`, отрицательное и пустая строка отвергаются и после амендмента.
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-22T23:00Z
- Milestone: M-65-ws-session
- Статус: BLOCKED
- HEAD: f4f0a49 — docs(contract-rfc): CT-RFC-09 §2.6 narrow amendment [architect]

## §B — Что я сделал
- Audited the committed RFC amendment and its merge-preview against `origin/main`.
- Established the new internal contradiction against A-015 §3.

## §C — Артефакты / результаты
- `research/critiques/C-125-ct-rfc-09-a015-amendment.md`
- Done Block: both required merge-preview gates exit 0; verdict remains REJECT on the documented contradiction.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter`
- **Paste-ready промпт:**
  ```
  Fresh-context arbitration for M-65 on fix/M-65-battery-recalibration. Read A-015 §3–§5,
  C-125, and f4f0a49 CT-RFC-09 §2.6. Decide whether its lines 123–126 (empty → 16 + warn)
  and line 138 (empty is rejected) can coexist. This is a new-fact REJECT under A-015 §5
  step 2; do not implement or alter M-65. Commit the decision as A-NNN on the subject branch.
  ```
- Push-статус: pending this verdict commit and push to `origin/fix/M-65-battery-recalibration`.
- Кэш: no `target/` created in this critic worktree.

## §E — Риски / открытые вопросы
- A-015 steps 3–5 must not proceed until the arbiter resolves the contradictory contract text.

=== END HANDOFF ===
