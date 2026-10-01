# C-082 — M-66 protocol-attestation critic verdict

**Verdict:** REJECT  
**Date (UTC):** 2026-08-14T13:05Z  
**Subject branch:** `origin/docs/M-66-protocol-attestation`  
**Subject commit:** `33c5c76`  
**Milestone:** `milestones/M-66-protocol-attestation.md`  
**Role:** critic, plan-time gate

## Scope

I audited the committed artifact set, not only the milestone prose.

Committed diff against `origin/main` contains only:

```text
milestones/M-66-protocol-attestation.md
```

The full normal hft plan-time artifact set is not present yet:

```text
check_review_fa=1
red_review_fa=1
verify_M_66=1
```

Per `.claude/agents/critic.md`, a critic is normally not launched before RED tests and the
acceptance script are committed. Because the dispatcher explicitly asked for "спека M-66",
I continued with the spec audit below. This does not remove the formal artifact-set defect.

## Findings

### B-1 — REJECT: `D1NOFA` contradicts the stated invariant

`milestones/M-66-protocol-attestation.md:71-76` states the result invariant as: a checked
range touching `crates/**` must not pass without a review file carrying at least one live
FA invariant ID for a touched crate. The same milestone then defines the only-NO-FA path as
PASS: `milestones/M-66-protocol-attestation.md:123-124`, and the RED matrix blesses
`D1NOFA` as PASS at `milestones/M-66-protocol-attestation.md:201`.

That means a merge touching only `crates/recorder/**` or `crates/derive/**` can pass with
no FA invariant echo at all. This is not just a named limitation; it contradicts the
invariant that the barrier claims to enforce.

Command-backed check of line 20 confirms the real NO-FA crates are `derive` and `recorder`,
and `research-cli` does have FA:

```text
$ for crate in $(ls crates | sort); do case "$crate" in derive|recorder) echo "$crate NO-FA";; gateway|gateway-serve) test -f docs/fa/viz-backend.md && echo "$crate docs/fa/viz-backend.md" || echo "$crate MISSING";; venue-*) test -f docs/fa/venues.md && echo "$crate docs/fa/venues.md" || echo "$crate MISSING";; *) test -f "docs/fa/$crate.md" && echo "$crate docs/fa/$crate.md" || echo "$crate MISSING";; esac; done
alpha docs/fa/alpha.md
book docs/fa/book.md
contracts docs/fa/contracts.md
derive NO-FA
gateway docs/fa/viz-backend.md
gateway-serve docs/fa/viz-backend.md
journal docs/fa/journal.md
ops docs/fa/ops.md
portfolio docs/fa/portfolio.md
recorder NO-FA
research-cli docs/fa/research-cli.md
signals docs/fa/signals.md
sim docs/fa/sim.md
strategy docs/fa/strategy.md
venue-binance docs/fa/venues.md
venue-binance-futures docs/fa/venues.md
venue-hyperliquid docs/fa/venues.md
```

Special note requested by dispatcher: `D1NOFA` is not sufficient for the stated goal. It
only proves mechanism D ("some review file exists") and makes the lack of B visible in
stdout; it does not provide the protocol attestation M-66 is about.

### B-2 — REJECT: mechanism D does not mechanize `TD-105` / `gates.md` §4 as stated

`TD-105` says the next step is a design check of `merge(M-NN) in main => author reviewer +
tree has research/reviews/R-*.md naming this milestone`. M-66 changes that to: review files
added in the range, plus files named by full path in any commit message and existing on
HEAD (`milestones/M-66-protocol-attestation.md:80-84`), with old R-files deliberately
allowed (`milestones/M-66-protocol-attestation.md:251-253`).

This permits a code-carrying range to satisfy D by naming a stale review file that happens
to contain a live invariant ID for one touched module. It does not require the review file
to name M-66's current milestone, the merge, the reviewed range, or the current reviewer
stage. That is weaker than the TD-105 target and weaker than `gates.md` §4's "review file
naming the milestone" rule.

This is not a redesign request. The presented spec must either narrow the objective to the
weaker property it actually enforces or make the claimed TD-105 property true.

### B-3 — REJECT: CI observability has a false-green path through `status-check`

M-66 requires an additive `review-fa` job and says existing jobs and their `needs` must not
change (`milestones/M-66-protocol-attestation.md:165-170`, `292-294`). Current CI has an
aggregate job:

```text
$ nl -ba .github/workflows/ci.yml | sed -n '231,241p'
   231	  status-check:
   232	    name: All checks passed
   233	    runs-on: ubuntu-latest
   234	    needs: [build-test, security, delivery, protected-artifacts, contracts, docs-freeze, artifact-ids, design-claims]
   235	    if: always()
   236	    steps:
   237	      - run: |
   238	          if [[ "${{ needs.build-test.result }}" != "success" || "${{ needs.security.result }}" != "success" || "${{ needs.delivery.result }}" != "success" || "${{ needs.protected-artifacts.result }}" != "success" || "${{ needs.contracts.result }}" != "success" || "${{ needs.docs-freeze.result }}" != "success" || "${{ needs.artifact-ids.result }}" != "success" || "${{ needs.design-claims.result }}" != "success" ]]; then
   239	            echo "One or more checks failed"; exit 1
   240	          fi
   241	          echo "All checks passed"
```

If M-66 adds `review-fa` but keeps this existing `needs` list unchanged, `review-fa` can be
red while `status-check` prints `All checks passed`. Since branch protection is already a
known limitation (`TD-124`), this is an observability defect, not merely a blocking defect.
This is the seventh unlisted limit I found: the new gate can fail outside the repository's
aggregate success signal.

### B-4 — REJECT: §7 task 5 is under-scoped by a disproved measurement

Line 24 says there are 5 profiles with `Startup reading`, and task 5 plans a line in five
profiles (`milestones/M-66-protocol-attestation.md:24`, `273`). Current tree has nine:

```text
$ rg -l "^## Startup reading|Startup reading" .claude/agents/*.md | sort
.claude/agents/architect.md
.claude/agents/critic.md
.claude/agents/engine-dev.md
.claude/agents/research-dev.md
.claude/agents/reviewer.md
.claude/agents/risk-critic.md
.claude/agents/signal-engineer.md
.claude/agents/tester.md
.claude/agents/venue-dev.md
```

The authority boundary is otherwise correct: `.claude/agents/**` and `.claude/rules/gates.md`
are process-lock territory, so task 5 correctly requires an architect/founder-authorized
commit and is outside the dev cycle. The defect is the cardinality and scope: "five
profiles" is false on the presented HEAD.

### B-5 — REJECT: the kill-set does not cover the CI wiring absence class

The kill-set table covers content-level behavior of the barrier, but it does not cover the
case where the new job exists but is not included in the aggregate gate, or where the job is
removed/miswired later. `testing.md` gate integrity property 4 requires observing absence,
not only failure; current §4 has setup scenarios for missing FA/reviews directories
(`milestones/M-66-protocol-attestation.md:218-221`) but no scenario for absent/miswired CI
membership.

This is distinct from the already named "feature branch blind" limit. It is a main-branch
CI observation hole.

## Requested Checks

1. **§2 invariant: result vs execution branch.** It is result-based (`BASE..HEAD` net diff,
   HEAD-live IDs), not execution-branch-pinned. `D1REVPAIR` explicitly skips code that
   appeared and disappeared inside the checked range. That is defensible only for a final-tree
   invariant; it is not equivalent to "every code-carrying commit/merge was reviewed."

2. **B4CROSSCUT.** The prefix logic closes the specific `DET-I-1` bypass. I verified that
   `R-053` carries only `DET-I-1`, while `docs/fa/journal.md` also carries live `JR-I-*`;
   a checker using only the touched module prefix would reject `DET-I-1` alone. Residual
   false-positive risk remains for legitimate reviews whose most precise invariant is a
   cross-cutting ID.

3. **D1REVPAIR.** The asymmetry with `check_docs_freeze.sh` is explicit. `check_docs_freeze`
   is per-commit because its invariant is "the process lock was touched"; M-66 is per-range
   because it says the final `main` tree contains code. The spec must keep this as a named
   limit because the repository history can still contain an unreviewed code commit.

4. **D1NOFA.** Not sufficient; see B-1. The visible PASS message is better than silent
   success, but it still does not satisfy the live-FA-ID invariant.

5. **CI on `push/pull_request [main]`.** Verified:

```text
$ sed -n '1,12p' .github/workflows/ci.yml
name: CI

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]
```

This is enough only for post-merge / PR-to-main detection. It is not a pre-dev or feature
branch gate. M-66 names that limit at `milestones/M-66-protocol-attestation.md:248-250`.

6. **False positives.** The notable legitimate-work red paths are: a review that correctly
   cites only a cross-cutting invariant such as `DET-I-1`; `gateway-serve` changes reviewed
   against shared `VB-I-*` invariants while the mapping only accepts `GS-I-*`; and an old
   review whose cited ID was live when written but dead on current HEAD.

7. **Seventh omitted limit.** See B-3: `review-fa` can fail while `status-check` remains
   green if the spec keeps existing `needs` unchanged.

8. **Task zones.** Task 5 is correctly process-locked and founder-approved, but under-scoped
   to five profiles; the current tree has nine startup-reading profiles.

## Measurement Recheck

I rechecked more than five §0 rows by command:

```text
$ stat -c '%n %s' PROJECT-STATE.md TECH-DEBT.md
PROJECT-STATE.md 426782
TECH-DEBT.md 512145
```

This disproves §0 row 3's byte counts on the presented HEAD.

```text
$ for f in research/reviews/R-053-M-62.md research/reviews/R-056-M-62-rev2.md research/reviews/R-066-M-62-rev3.md research/reviews/R-040-M-57-rev3.md; do printf '%s docsfa=' "$f"; grep -c 'docs/fa/' "$f"; printf '%s ids=' "$f"; grep -oE '\b[A-Z]{2,4}-I-[0-9]+\b' "$f" | sort -u | tr '\n' ' '; printf '\n'; done
research/reviews/R-053-M-62.md docsfa=0
research/reviews/R-053-M-62.md ids=DET-I-1
research/reviews/R-056-M-62-rev2.md docsfa=0
research/reviews/R-056-M-62-rev2.md ids=
research/reviews/R-066-M-62-rev3.md docsfa=0
research/reviews/R-066-M-62-rev3.md ids=
research/reviews/R-040-M-57-rev3.md docsfa=2
research/reviews/R-040-M-57-rev3.md ids=JR-I-1 JR-I-11 JR-I-2
```

Rows 4-7 confirmed.

```text
$ git diff --name-only d564617^1 d564617 | rg '^crates/' | cut -d/ -f1-2 | sort -u
crates/gateway
crates/journal

$ git diff --name-only 710b1ad^1 710b1ad | rg '^crates/' | cut -d/ -f1-2 | sort -u
crates/gateway
crates/journal
```

Rows 8-9 confirmed.

```text
$ git diff --name-only d564617^1 d564617 | rg '^research/reviews/' | sort
research/reviews/R-053-M-62.md
research/reviews/R-056-M-62-rev2.md
research/reviews/R-066-M-62-rev3.md

$ git diff --name-only 710b1ad^1 710b1ad | rg '^research/reviews/' | sort
research/reviews/R-035-M-57.md
research/reviews/R-039-M-57-rev2.md
research/reviews/R-040-M-57-rev3.md
```

Rows 10-11 confirmed.

```text
$ git show 710b1ad:docs/fa/journal.md | grep -oE 'JR-I-[0-9]+' | sort -Vu | tr '\n' ' '
JR-I-1 JR-I-2 JR-I-3 JR-I-4 JR-I-5 JR-I-6 JR-I-7 JR-I-8 JR-I-9 JR-I-10 JR-I-11 JR-I-12 JR-I-13
```

Row 12 confirmed.

```text
$ for f in docs/fa/*.md; do if grep -q 'DET-I-1' "$f"; then printf '%s\n' "$f"; fi; done | sort
docs/fa/README.md
docs/fa/_TEMPLATE.md
docs/fa/ai-copilot.md
docs/fa/alpha.md
docs/fa/book.md
docs/fa/journal.md
docs/fa/oms.md
docs/fa/portfolio.md
docs/fa/research-cli.md
docs/fa/risk.md
docs/fa/signals.md
docs/fa/sim.md
docs/fa/strategy.md
docs/fa/viz-backend.md
```

Row 13 confirmed in direction: `DET-I-1` is a cross-cutting citation, so B4CROSSCUT is a
real bypass class if the implementation accepts any ID greppable from the FA.

## §9 Recheck Coverage

Covered in this verdict:

- code/repository claims by command: artifact diff, missing scripts, CI triggers, CI
  aggregate `needs`, historical review files, historical merge diffs, FA prefix mapping;
- authority: critic wrote only `research/critiques/C-082-M-66-protocol-attestation.md`; task
  5 process-lock boundary checked against `.claude/**` / `gates.md` §11;
- connectivity: `docs/04-workflow.md`, `docs/05-contract-layer.md`, `gates.md`, `testing.md`,
  `TD-105`, `TD-124`, and the milestone line references were cross-checked.

Therefore, for this critic pass, a separate §9 recheck round is not needed. The verdict is
still REJECT.
