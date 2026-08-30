<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: ae7afb18213d6abd04da309ca47284a657e20a63
verdict: REJECT
-->

# C-169 — M-71 egress-cap rev8: REJECT

## Scope audited

- Subject: `origin/feat/M-71-rev6` at
  `ae7afb18213d6abd04da309ca47284a657e20a63`; stakes: normal.
- Complete M-71 artifact set was inspected: the milestone, seven RED artifacts,
  `scripts/verify_M-71.sh`, and the T2 bridge contract. The range has no
  `crates/contracts/**` change, so no T1 contract-RFC is required.
- `VB-I-2` (live == replay) and `VB-I-11` (the meaning of
  `history_truncated`) are live invariants of the affected viz/gateway surface
  and remain exercised by `red_egress_cap_paths.rs`.

## REJECT — dev must not be dispatched

### R1 — absence/empty policy remains contradictory inside the committed milestone

The requested three primary carriers now agree about an **absent** variable:

- Task 4 says absence selects the signed `П-020` default while invalid input
  refuses startup (`milestones/M-71-egress-cap.md:612`).
- Oracle D requires the same for absence
  (`crates/gateway-serve/tests/red_egress_cap_startup.rs:139-147`) and rejects
  an empty value as invalid (`:114-117`).
- N1-C likewise requires absence to produce the signed default
  (`crates/gateway-serve/tests/red_egress_cap_governed.rs:139-159`).

But two normative statements in the same milestone still prescribe the
opposite outcomes:

1. The RED-oracle table says an **absent** configuration prevents the binary
   from starting (`milestones/M-71-egress-cap.md:669`).
2. The declared “current policy” says an **empty** value selects `П-020`
   (`:503-505`), while Oracle D explicitly fails startup for that value.

This is the same absence-policy defect rejected in C-168, merely left in two
other normative carriers. A dev cannot satisfy one unambiguous milestone
contract until the policy is made singular. Align the table and policy prose
with the selected Task-4/RED outcomes, then resubmit the complete artifact
set.

### R2 — the bridge contract says “one startup install,” but the RED suite permits runtime rewrites

The bridge specification requires a public setter to be called exactly once at
startup, never for a runtime change, and never after an env parse error
(`milestones/M-71-egress-cap.md:463-475`). The committed test suite does not
pin either negative condition:

- In one serial integration test, N1b first invokes
  `serve_config_from_env` with the base/default configuration (`red_egress_cap_governed.rs:193`)
  and then invokes it again with `GATEWAY_MAX_RESPONSE_BYTES=1000` (`:211`),
  requiring the second value to take effect.
- No RED oracle observes the effective gateway value after an `Err` parse, so
  a setter called before returning that error also passes.

Consequently a public atomic `store` setter callable repeatedly at runtime
satisfies the signature, N1a, and N1b. The stated one-start/no-runtime
semantics are therefore prose only; the suite requires the contrary
multi-install behavior in its own process. Add a legal, anti-placebo RED
surface that rejects repeated production installation and an error path that
would install a value, while resolving the test-process setup conflict. The
design of that seam remains architect work.

## Checked passes and non-blockers

- `cargo clippy --workspace --all-targets --all-features -- -D warnings` is
  red only for the intended missing
  `gateway::effective_max_response_bytes()` bridge (`E0425`); C-168’s dead
  code diagnostics are gone.
- A disposable minimal getter stub makes the workspace Clippy gate green and
  leaves `red_egress_doors.sh` green. Thus the bridge compile-RED is reachable,
  not an additional impossible-GREEN condition.
- The normal door inventory passes with 12 named doors. In an isolated copy,
  muting **both** `gateway::snapshot(` calls makes it fail exactly once for
  the missing `snapshot` door; the subject tree was not modified.
- `scripts/verify_M-71.sh` is a real aggregate gate (`set -uo pipefail`,
  explicit `FAIL` counter and non-zero exit) and its clean-cache baseline is
  the declared `FAIL (5)`.
- `git diff --numstat 3111a10..ae7afb1 -- 'crates/*/src'` is empty. The full
  audited range has no `crates/contracts/**` change.

## Routing

This is the second consecutive REJECT for the same absence-policy subject
after C-168. Per `gates.md` §0, the matter goes to a fresh-context arbiter,
not to a fifth architect↔critic repair round.

## Done Block

    $ git rev-parse HEAD
    ae7afb18213d6abd04da309ca47284a657e20a63

    $ git merge-base HEAD origin/main
    3b496208a64edbf00a66b93986ff8529d0c93aa9

    $ bash -n scripts/verify_M-71.sh
    exit=0

    $ cargo clippy --workspace --all-targets --all-features -- -D warnings
    error[E0425]: cannot find function `effective_max_response_bytes` in crate `gateway`
       --> crates/gateway-serve/tests/red_egress_cap_governed.rs:136:17
    error: could not compile `gateway-serve` (test "red_egress_cap_governed") due to 1 previous error
    exit=101

    $ CARGO_TARGET_DIR=<isolated-stub-target> cargo clippy --workspace --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.77s
    exit=0

    $ bash scripts/tests/red_egress_doors.sh
    PASS: дверь L1 snapshot — названа в оракуле
    VERDICT: PASS — все найденные двери названы в оракулах
    exit=0

    $ # isolated copy: both gateway::snapshot( call sites muted
    $ bash scripts/tests/red_egress_doors.sh
    FAIL: дверь L1 snapshot принимает &Selector, но оракул crates/gateway/tests/red_egress_cap.rs её НЕ ЗОВЁТ
    VERDICT: FAIL (1) — дверь существует, а оракул её не зовёт
    exit=1

    $ bash scripts/verify_M-71.sh
    FAIL: cargo clippy --all-targets --all-features -- -D warnings
    FAIL: cargo test --all --quiet
    PASS: bash scripts/tests/red_egress_doors.sh
    FAIL: cargo test -p gateway --test red_egress_cap_paths --quiet
    FAIL: cargo test -p gateway-serve --test red_egress_cap_utf8 --quiet
    FAIL: cargo test -p gateway-serve --test red_egress_cap_governed --quiet
    PASS: cargo test -p gateway-serve --test red_egress_cap_startup --quiet
    PASS: C база зелена, мутация роняет набор, честная нагрузка (E) цела
    PASS: F crates/contracts не тронут
    PASS: G GATEWAY_BANDS не тронут (граница C, предмет M-70)
    PASS: H book/venue/journal не тронуты диапазоном
    VERDICT: FAIL (5)
    exit=1

    $ git diff --numstat 3111a10..ae7afb1 -- 'crates/*/src'
    exit=0

    $ git diff --name-only 3b496208a64edbf00a66b93986ff8529d0c93aa9..ae7afb1 -- crates/contracts
    exit=0

    $ git ls-remote origin 'refs/reserved/C-169'
    c5a11d0c793c31b7c4a4e864e2dcb9ad4f4f3daf	refs/reserved/C-169
    exit=0
