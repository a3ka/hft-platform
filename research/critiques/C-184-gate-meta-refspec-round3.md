<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: f0daa105383ddb596df3e585197e320e473577ae
audited_head: 258c52dfb2675c1c112da0a26fd4bb6a44500065
verdict: REJECT
-->

# C-184 — REJECT: the refspec observer still accepts a skipped gate step

**Subject:** `harness/gate-meta-salvage-refs` at
`258c52dfb2675c1c112da0a26fd4bb6a44500065` (PR #125, M-68).

**Scope:** harness track §§3/5.  I audited the committed branch, not the plan;
`C-182` and `C-183` were read first.  The measurements below use fresh refspec
clones (`git init` → `remote add` → `fetch '+refs/heads/*:refs/remotes/origin/*'`),
not the local checkout.

## Verdict

**REJECT; route directly to an independent arbiter.**  The production-wiring
observer remains syntactic.  It accepts a `gate-meta` job in which the salvage
fetch is present and ordered, but GitHub Actions skips it.  This is the third
round on the same defect class: C-182 B-2 found no observer for the effective
fetch, C-183 B-1 found an observer of text rather than action, and B-1 below
finds the repaired observer still dependent on YAML key order rather than step
semantics.  Per `gates.md` §0, do not open a fourth critic loop.

## Blocking findings

### B-1 — `if:` after `run:` leaves CR-9 and the whole probe green

`extract_salvage_fetch` only rejects `if:` whose source line lies from the
fetch step's `- name:` through its `run:` line
(`scripts/tests/red_ci_gate_meta_refspec.sh:121-126`).  YAML mapping-key order
does not constrain Actions semantics.  Therefore this production-code mutation
is equivalent to C-183's `if: false` mutation but lies after `run:`:

```yaml
- name: Дотянуть спас-рефы (вердикты терминальных веток)
  run: git fetch --no-tags origin '+refs/salvage/*:refs/salvage/*'
  if: ${{ false }}
```

The fetch is skipped; the following `check_gate_meta.sh` runs without terminal
refs.  Yet the extractor reports the step as unconditional and all ten CR
scenarios pass.  CR-9 proves only one textual placement of one semantic field,
not the field on the step.  This is a direct Р-2 miss (the action is absent
while its text remains) and a Р-3 miss (only one member of the `if`-placement
group is mutated).

### B-2 — the declared `continue-on-error` “limit” is a current bypass, not an advisory edge

The probe names `continue-on-error: true` on the barrier step as out of scope
(`red_ci_gate_meta_refspec.sh:29-30`) but does not reject it.  Adding that
production YAML key to the `check_gate_meta.sh` step leaves the probe 10/10
green.  A failing `continue-on-error` step receives final conclusion `success`;
`status-check` tests only `needs.gate-meta.result == success`
(`.github/workflows/ci.yml:523-528`).  Thus a real invalid GATE-META can make
the barrier command return 1 without blocking the required aggregate check.

Naming this in a comment is honest about the mechanism's limit, but calling it
a non-blocking limit is not: it is a one-line, in-scope change that disables the
very gate the probe claims to protect.  The proof suite neither mutates it nor
observes its job outcome.

## Lineage check: both requested counterexamples

### F-1 — a stale ancestral `audited_head` is cheaper than the named side-commit + terminal token

The new kinship branch is never entered when the author declares any old
ancestor as `audited_head`.  `check_gate_meta.sh:457-461` checks only object
existence and ancestry to current `HEAD`; `audited_base` is likewise only
resolved.  It does not bind either declared revision to the actual subject.

In a fresh refspec clone, one verdict commit declared both revisions as
`f0daa10` while the actual subject was the 258c52d branch.  Its one
`ALLOW-SUBJECT-CHANGE` audit-trace line admitted the branch's changed workflow,
barrier, and probe.  The barrier returned `PASS` although the header did not
describe an audit of that artifact set.  This costs no separate terminal
side-commit and no `TERMINAL-BRANCH-VERDICT` token.  The script's own boundary
(`check_gate_meta.sh:40-45`) says it does not compute subject correspondence;
that makes the false pass a declared design limit, not a proof that the
lineage repair binds an audit to the branch.

### N-1 — a genuinely terminal orphan line is rejected despite the terminal-verdict protocol

The terminal exception specifies a branch declared terminal by arbiter/founder;
it does not state that such a branch must share a parent with the current
`HEAD`.  A lawful emergency/orphan terminal history in this repository can have
a real object, the correct origin slug, and a path-specific
`TERMINAL-BRANCH-VERDICT`, yet `git merge-base <head> HEAD` has no result and
the new check rejects it as foreign.  The fixture reproduces that exact legal
token topology and returns exit 1.  The guard cannot distinguish it from the
attack C-182 B-1; the added ancestry requirement is therefore a trade-off that
must be explicitly decided, rather than presented as an unconditional terminal
branch repair.

## Required arbitration question

The arbiter should decide whether M-68 needs a semantic workflow interpreter or
an explicitly narrower, mechanically enforceable contract.  In particular,
the decisive property is not source-line order: it is that the required fetch
actually executes before a non-ignorable barrier step.  The arbiter must also
decide whether terminal histories are defined to require shared ancestry, and
whether the pre-existing stale-header declaration limit is acceptable for this
gate.

## Done Block

```text
$ fresh refspec clone of origin/harness/gate-meta-salvage-refs
$ git rev-parse HEAD; git for-each-ref 'refs/salvage/*' | wc -l; git fsck --no-reflogs --unreachable | wc -l
258c52dfb2675c1c112da0a26fd4bb6a44500065
0
0
exit=0

$ bash scripts/tests/red_ci_gate_meta_refspec.sh
PASS  CR-0 … стоит ДО барьера и безусловна
PASS  CR-1 … PASS
PASS  CR-2 … FAIL «не существует»
PASS  CR-3 … PASS
PASS  CR-4 … FAIL по РОДСТВУ
PASS  CR-5 … FAIL по РОДСТВУ
PASS  CR-6 … FAIL «не существует»
PASS  CR-7 …
PASS  CR-8 …
PASS  CR-9 …
VERDICT: PASS (10/10) — рефспек клона судится как часть гейта; отсутствие шага наблюдаемо
exit=0

$ mutation: add `if: ${{ false }}` AFTER the salvage step's `run:`; cmp -s original mutated-ci.yml
cmp_exit=1
$ bash scripts/tests/red_ci_gate_meta_refspec.sh
VERDICT: PASS (10/10) — рефспек клона судится как часть гейта; отсутствие шага наблюдаемо
late_step_if_probe_exit=0

$ mutation: add `continue-on-error: true` to the `check_gate_meta.sh` step; cmp -s original mutated-ci.yml
cmp_exit=1
$ bash scripts/tests/red_ci_gate_meta_refspec.sh
VERDICT: PASS (10/10) — рефспек клона судится как часть гейта; отсутствие шага наблюдаемо
continue_on_error_probe_exit=0

$ EVENT_NAME=push PUSH_BEFORE=<fixture-parent> bash scripts/check_gate_meta.sh
FAIL  research/critiques/C-998-fixture-missing-meta.md: нет шапки GATE-META — вердикт ничем не привязан к предмету
VERDICT: FAIL (1) — вердикт не привязан к предмету либо merge прошёл без вердикта.
invalid_meta_barrier_exit=1

$ stale ancestral header (audited_base=audited_head=f0daa10, actual subject=258c52d) + one ALLOW-SUBJECT-CHANGE
NOTE  research/critiques/C-997-stale-ancestor-fixture.md: subject-lock открыт явным ALLOW-SUBJECT-CHANGE: .github/workflows/ci.yml scripts/check_gate_meta.sh scripts/tests/red_ci_gate_meta_refspec.sh
VERDICT: PASS — вердиктов проверено: 3, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
stale_ancestral_head_barrier_exit=0

$ legal terminal orphan object + path-specific TERMINAL-BRANCH-VERDICT
orphan_common_ancestor_exit=1
FAIL  research/critiques/C-996-legal-orphan-terminal-fixture.md: audited_head «8add252c…» НЕ имеет общего предка с HEAD
legal_orphan_terminal_barrier_exit=1

$ bash scripts/tests/red_gate_meta.sh
VERDICT: PASS (56/56) — вердикт привязан к предмету, лок держит, отсутствие наблюдаемо
exit=0

$ fixtures: gio trash /tmp/c184-refspec-* /tmp/c184-cont-* /tmp/c184-lineage-* /tmp/c184-fp-*
remaining_c184_or_red_ciref_dirs=0
```
