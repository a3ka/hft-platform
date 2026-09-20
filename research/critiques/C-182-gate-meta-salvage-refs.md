<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: f0daa105383ddb596df3e585197e320e473577ae
audited_head: 801a274015885f278c4d1fae60d136fe5a06f3cf
verdict: REJECT
-->

# C-182 — REJECT: gate-meta salvage refs reopen the C-062 trust boundary

**Audited subject:** `harness/gate-meta-salvage-refs`, `801a274015885f278c4d1fae60d136fe5a06f3cf` (`PR #125`).

## Verdict

**REJECT.** The new CI fetch changes the effective C-062 predicate from a revision
reachable through the normal checkout lineage to any commit object a writer can place under
`refs/salvage/*`.  With the existing `TERMINAL-BRANCH-VERDICT` exception this accepts a
verdict over an unrelated orphan history.  The two supplied stubs prove only missing-object
and missing-token cases; neither tests this combined carrier.

## Blocking findings

### B-1 — salvage-only orphan + terminal token gives PASS

`.github/workflows/ci.yml:332-333` fetches every `refs/salvage/*` into the CI object database.
`scripts/check_gate_meta.sh:457-465` then treats `rev-parse` success as existence and accepts
any non-ancestor through the declarative terminal token.  The token itself explicitly does
not prove terminality (`scripts/check_gate_meta.sh:348-358`).  No code binds a salvage ref to
the history that was actually audited.

I built a bare `origin` and a fresh consumer strictly through the requested checkout form:

```text
git init; git remote add origin <bare-origin>
git fetch --no-tags origin '+refs/heads/*:refs/remotes/origin/*' \
  '+refs/pull/125/merge:refs/remotes/pull/125/merge'
```

The only reference to orphan commit `8ae69e2a3107cc7afe78ba8194195d41455d0e6f` was pushed as
`refs/salvage/adversarial-orphan`; it has no `refs/heads/*` carrier.  The PR commit
`08b698468b80d0d5e600163971c307524fbd894c` adds a `REJECT` verdict with that SHA and its own
path-specific `TERMINAL-BRANCH-VERDICT` token.

```text
before salvage fetch:
FAIL  research/critiques/C-999-orphan.md: audited_head «8ae69e…» не существует в этой истории (класс C-062)
VERDICT: FAIL (1)
third_stub_before_salvage_exit=1

after git fetch --no-tags origin '+refs/salvage/*:refs/salvage/*':
NOTE  research/critiques/C-999-orphan.md: audited_head «8ae69e…» на ТЕРМИНАЛЬНОЙ ветке — открыто явным TERMINAL-BRANCH-VERDICT в 08b69846
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
third_stub_after_salvage_exit=0
```

This is the C-062 property in a new carrier: an object from no reviewed lineage becomes an
accepted audit base merely because the same credential can publish it under a fetched salvage
name.  `audited_repo` remains the declared origin slug, so it does not constrain the object.

### B-2 — the committed RED probe does not test the new CI condition (Р-2)

`scripts/tests/red_gate_meta.sh` creates and runs a hermetic repository against
`scripts/check_gate_meta.sh`; it neither reads nor executes `.github/workflows/ci.yml`.
The only workflow occurrence is fixture data at `red_gate_meta.sh:112`; `rg` found no
`actions/checkout`, `refs/heads`, or `git fetch` production wiring.  Therefore deleting
`.github/workflows/ci.yml:332-333` leaves the committed probe green:

```text
$ bash scripts/tests/red_gate_meta.sh
VERDICT: PASS (56/56) — вердикт привязан к предмету, лок держит, отсутствие наблюдаемо
red_gate_meta_exit=0
```

The real production-form command is sensitive to omission, but only when a terminal-branch
verdict is present: on a fresh actions-shaped checkout of PR #124 it produced `FAIL (3),
exit=1` before the fetch and `PASS, exit=0` after it.  Thus the missing-step observer is absent
from the test suite, exactly the Р-2 failure: the claimed mechanism is not judged under its
active checkout/refspec constraint.

## Carrier census (Р-3)

Remote measurement:

```text
heads=14
pull=127
reserved=42
salvage=64
tags=0
notes=0
```

The property here is local-object existence (`git rev-parse`), not the prose claim “in
origin”.  In this CI job its actual carriers are the checkout’s `refs/heads/*`, the event
`refs/pull/<N>/merge`, and now all fetched `refs/salvage/*`.  `refs/reserved/*` is a real,
unlisted remote carrier namespace (42 refs), but is deliberately absent from the default
refspec and from the allocator/barrier universe (`scripts/reserve_artifact_id.sh:74-76`);
historical `refs/pull/*` likewise are not fetched except the event ref.  Tags and notes are
currently empty and not fetched.  Enumerating only heads and salvage therefore hides both the
event carrier and the distinction between remote namespaces and objects actually reachable in
the job.

The supplied post-fetch stubs do behave as stated but do not distinguish B-1:

```text
no_terminal_token_exit=1
nonexistent_sha_exit=1
```

They remove one protection at a time.  The missed mutation is: preserve a real commit object,
make it salvage-only and unrelated, then add the terminal token.

## Non-finding: `verify_design_claims.sh`

The adjacent consumer is intentionally different and should not copy this fetch.  After the
salvage fetch, the adversarial orphan gave `git cat-file -e …^{commit}` exit `0`, but
`git merge-base --is-ancestor <orphan> HEAD` exit `1`.  `verify_design_claims.sh:1128-1155`
requires ancestry to `HEAD` (or `MERGE_HEAD` only for merge preview), so the orphan remains
rejected.  The author is right on this point.

## Required before re-review

1. Restore a verifiable trust boundary for terminal audited revisions; a writeable salvage
   ref alone cannot establish that the orphan is a valid terminal history of this repository.
2. Add a production-refspec oracle that runs the exact checkout plus CI fetch sequence.  It
   must be red when the workflow fetch step is absent and must include the salvage-only orphan
   plus terminal-token mutation above.
3. State and test every accepted carrier of the existence property, including the event ref;
   do not describe “origin” when the implementation only inspects fetched objects.

## Done Block

```text
$ git diff --check f0daa105383ddb596df3e585197e320e473577ae 801a274015885f278c4d1fae60d136fe5a06f3cf
exit=0

$ fresh actions-shaped PR #124 checkout; check_gate_meta before/after salvage fetch
without_salvage_exit=1
VERDICT: FAIL (3)
salvage_refs=64
with_salvage_exit=0
VERDICT: PASS

$ fresh actions-shaped PR #125 checkout; fetch salvage; check_gate_meta
pr125_gate_meta_exit=0
VERDICT: PASS — вердиктов проверено: 0, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0

$ salvage-only unrelated orphan + terminal token (fresh bare origin and exact refspec)
third_stub_before_salvage_exit=1
VERDICT: FAIL (1)
third_stub_after_salvage_exit=0
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0

$ post-fetch controls
no_terminal_token_exit=1
nonexistent_sha_exit=1

$ bash scripts/tests/red_gate_meta.sh
VERDICT: PASS (56/56) — вердикт привязан к предмету, лок держит, отсутствие наблюдаемо
red_gate_meta_exit=0

$ git cat-file -e 8ae69e2a3107cc7afe78ba8194195d41455d0e6f^{commit}; echo $?
0
$ git merge-base --is-ancestor 8ae69e2a3107cc7afe78ba8194195d41455d0e6f HEAD; echo $?
1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-29T23:07Z
- Milestone: M-68
- Статус: BLOCKED
- HEAD: 801a274 — fix(harness): gate-meta salvage refs

## §B — Что я сделал
- Audited the committed CI wiring in production-form refspec checkouts.
- Reproduced the claimed false-red repair and a third salvage-only orphan bypass.

## §C — Артефакты / результаты
- `research/critiques/C-182-gate-meta-salvage-refs.md`
- Done Block is embedded above; REJECT blocks merge.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Address C-182 on harness/gate-meta-salvage-refs. Preserve the PR #124 false-red repair only
  if terminal audited revisions remain bound to a verifiable repository lineage. Add an exact
  checkout/refspec oracle: deleting the CI salvage-fetch step must fail it, and a salvage-only
  unrelated orphan with TERMINAL-BRANCH-VERDICT must remain rejected. Commit/push the revised
  harness artifacts, then request a fresh critic pass.
  ```
- Push-статус: pending this verdict commit to origin/harness/gate-meta-salvage-refs
- ⏸ кэш оставлен — no build cache was created by this audit

## §E — Риски / открытые вопросы
- The current step fetches a writeable namespace as evidence of audit-history membership.

=== END HANDOFF ===
