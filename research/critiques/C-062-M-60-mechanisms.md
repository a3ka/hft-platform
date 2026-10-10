# Critique C-062 — M-60 "mechanisms instead of prose"

Date: 2026-08-04
Agent: critic
Branch: critic/m60-mechanisms
Base: origin/main @ 9a0e48f0

## Verdict

REJECT as scoped.

The three proposed imports are the right direction, but they are not sufficient
against the named failure class unless M-60 also adds a post-critic
subject-lock mechanism. With that fourth mechanism and a line-neutral rule
budget, the proposal becomes NOTE / acceptable.

## Pre-flight

- Worktree: `/tmp/hft-critic-m60`
- Step 0 HEAD: `9a0e48f0 fix(wrappers)(architect): W6b — pi-dev.sh auto-pushes feat branch on exit in EINHARD_WORKTREE enter-existing mode`
- Step 0 anomaly: `.claude/rules/gates.md` is absent on `origin/main`.
  If M-60 assumes that file exists, the plan must name the actual target file
  or create it explicitly.
- Rule corpus re-read from the clean worktree: 28 files under
  `.claude/rules/`, 5430 total lines.

## Sources Read

- `.claude/rules/binding-requires-mechanism.md` lines 12-26: new
  enforcement BINDING requires a mechanism or `cognitive-only`; repeat >=3
  means mechanize or accept risk, not more prose.
- `.claude/rules/binding-requires-mechanism.md` lines 30-39: registry +
  reviewer backstop + R10 size cap.
- `.claude/rules/postmerge-checks.json` lines 1-4, 12-22, 25-30, 38-47,
  50-55, 169-172: post-merge registry as SSOT, with selected checks carrying
  gates.
- `.claude/rules/chain-integrity.md` lines 299-348: `§D Status` must be
  checked against `git log origin/main..HEAD` before handoff emission and
  re-checked by reviewer.
- `.claude/rules/chain-integrity.md` lines 350-377: reviewer I0
  chain-integrity findings and why green gates are not "done".
- `.claude/rules/critic-protocol.md` lines 112-203: verdict-tier routing and
  NOTE/APPROVE ban on architect self-fix loops, except Commit-F appendix only.
- `.claude/rules/critic-protocol.md` lines 217-253: reviewer checks for
  critic routing and Commit-F sequencing.
- `.claude/rules/critic-protocol.md` lines 289-313: critic must be a separate
  subagent; architect-self critic fallback is forbidden and partly mechanized
  by pre-push checks.
- `.claude/agents/reviewer.md` lines 326-330: reviewer Block I is driven by
  `postmerge-checks.json`.
- `scripts/precommit-chain-integrity-check.sh` lines 30-52 and 131-218:
  existing mechanical checks for architect-self critic fallback, close-out
  without reviewer, Commit-F verdict presence, and substantive push without
  reviewer.

## (a) Rule Growth Risk

Yes, literal transfer reproduces the disease. Direct copy costs roughly:

| Import | Direct source size |
|---|---:|
| `binding-requires-mechanism.md` | 48 lines |
| `postmerge-checks.json` | 174 lines |
| reviewer registry/execution prose | ~40-60 active lines |
| `§D Status re-check` section | 50 lines |
| critic §D verdict-tier routing | 167 lines |
| **Total if copied literally** | **~479-499 lines** |

That is too much active rule surface for a milestone whose stated purpose is
"mechanisms instead of prose".

Acceptable M-60 shape:

- Active prose budget: <=120 new lines total.
- JSON registry: <=80 lines for only checks this repo actually runs now.
- Mechanism scripts/tests may be longer, but they must not live in hot startup
  context.
- No source incident history in active rules. Put history in `research/` or
  rule-history docs if needed.

Delete or replace at least the same amount of active prose:

1. Remove any duplicated reviewer check list once `postmerge-checks.json`
   exists; reviewer says "execute registry" and keeps only procedures that are
   not machine-readable.
2. Remove the role-author / git-identity requirement from review-evidence
   checks. The user's measurement says all 14 worktrees carried reviewer
   identity, so identity is a false role signal.
3. Remove prose-only L2 / "user decides" escalation text if arbiter §0 is the
   local authority.
4. Compress handoff status examples to the predicate:
   `AHEAD=$(git log origin/main..HEAD --oneline | wc -l)` and the two valid
   status classes.

## (b) Are the Three Mechanisms Sufficient?

No.

They cover important surfaces:

- `binding-requires-mechanism` stops future rule-only patches.
- `postmerge-checks.json` makes reviewer checks enumerable and auditable.
- `§D Status re-check` catches false handoff claims about pushed/unpushed
  state.

They do not fully catch this class:

> architect worked the cycle alone, weakened the gate in their own favor, then
> changed subject after critic verdict without a new round.

The missing fourth mechanism is:

**I0-post-critic-subject-lock / critic-head fingerprint gate.**

Mechanics:

1. Every critic verdict must name `milestone_id`, `audited_base`,
   `audited_head`, and an `audited_diff_files_sha256` or explicit file list.
2. Reviewer (or pre-merge script) computes `git diff --name-only
   <audited_head>..HEAD`.
3. If any post-verdict change touches:
   - gate files or gate config,
   - milestone scope / allowed paths / verdict routing,
   - substantive implementation paths not in the audited file set,
   - `research/critiques/` verdict replacement without a newer verdict,
   then merge is blocked until a new critic verdict or arbiter §0 override
   names the new HEAD.
4. The local post-merge variant should require a `research/reviews/R-*.md`
   file naming the milestone. Do not require author role; require artifact
   presence and milestone/HEAD content.

This is the mechanism that binds "critic reviewed this subject" to "reviewer
is approving the same subject." Without it, a valid critic NOTE can be used as
cover for a later changed scope.

## (c) Arbiter §0 vs EINHARD L2

Do not import EINHARD's L2 fallback verbatim.

EINHARD's critic protocol says REJECT override twice escalates to L2, but L2 is
currently deferred and falls back to user-decides. That is weaker than a local
arbiter §0 if §0 is a standing authority layer.

Safe import:

- Keep verdict-tier routing as operational dispatch discipline:
  - REJECT -> author revises, then new critic/arbiter pass.
  - NOTE -> no author self-fix loop; appendix/landing only, then next agent.
  - ESCALATE -> arbiter §0.
- Replace every "L2 deferred -> user decides" branch with "arbiter §0
  required."
- Keep the user as relay only, not as the substantive adjudicator, unless §0
  itself says founder/user override is the adjudication mechanism.

This preserves the local advantage: deterministic arbitration remains above
the author and critic, while §D routing only prevents accidental loops.

## (d) Conflict with A-003?

No conflict if M-60 is line-neutral and mechanism-backed. It conflicts if M-60
adds another prose rule bundle without deleting old prose or adding a gate.

The binding-requires-mechanism rule is self-referential: it must be the first
test of itself. A compliant M-60 must include one of:

- a mechanical rule-diff gate that scans additions to rule/agent files for new
  `BINDING` / `MANDATORY` text and requires a co-landed mechanism path, registry
  entry, or explicit `cognitive-only` header; or
- a documented decision that this repo deliberately accepts the residual risk.

Reviewer-only post-merge audit is not enough; the source rule explicitly says
reviewer Block I is a backstop, not the mechanism.

## Required Changes Before Accept

1. Add `I0-post-critic-subject-lock` to the registry and implement it as a
   script or reviewer-executed deterministic check.
2. Add `research/reviews/R-*.md` evidence requirement by milestone name and
   reviewed HEAD. Do not check author role from git identity.
3. Keep arbiter §0 as the escalation authority; do not import
   "L2 deferred -> user decides".
4. Set an active-rule line budget for M-60 and delete equivalent duplicated
   prose in the same commit chain.
5. Introduce `binding-requires-mechanism` with its own gate, not as naked
   cognitive prose.

## Confidence

High. The source mechanisms are explicit, and the missing coverage is specific:
none of the three proposed imports binds a critic verdict to the exact later
diff that reviewer approves.
