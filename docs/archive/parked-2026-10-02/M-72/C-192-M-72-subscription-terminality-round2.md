<!-- GATE-META
milestone: M-72
audited_repo: a3ka/hft-platform
audited_base: 04c8f8f0713ca104392936021939998634b89abe
audited_head: bd1a5578045000e85d47066e647eedde78aa941f
verdict: REJECT
-->

# C-192 — M-72 subscription terminality, round 2 — REJECT

## Scope and decision

**REJECT. Do not dispatch engine-dev.** This is an audit of the committed artifact
set at `bd1a5578045000e85d47066e647eedde78aa941f`, not of the handoff prose.
Immediately before this verdict, remote
`feat/M-72-subscription-terminality` resolved to that SHA. The architect range
reviewed is `04c8f8f0713ca104392936021939998634b89abe..bd1a5578045000e85d47066e647eedde78aa941f`.
The merge base with `origin/main` is `d1221b1ca932d0b8e95403c2849308ed6e7b9ce2`.

The required committed set is present: M-72, T2/RFC text, the entrypoint RED,
TD-179, TD-180, and `scripts/verify_M-72.sh`. `crates/contracts` is untouched,
so T1/Block-C is not implicated. The new public v1 error form, however, still
needs a concrete T2 helper/signature in the milestone before its implementation
can be dispatched (B-4 below).

Round-1 B-1 is closed: TD-177 now uses the existing, testing-only
`test_sync::rendezvous` contract and its two hooks do hold the old pump across the
same subscription id. Round-1 B-4 is closed: only the engine-dev-owned
`crates/gateway/src/lib.rs` carrier remains for task 8. The chosen round-1 form is
not reopened: `subscription_terminated` with mandatory `reason` is the applicable
CT-RFC-09 §2.10 form.

## Blocking findings

### B-1 — C-190 B-2 is not closed: the claimed fresh ETH frame is not identifiable

TD-177 does append after switch and release, but `ETH_PRICE` is the same `3000.0`
for both the pre-switch append (`red_ws_terminality_entrypoint.rs:570`) and the
post-switch append (`:622`). The purported proof at `:643-650` calls the frame
fresh when its maximum price is below BTC's price. It therefore distinguishes BTC
from **any** ETH frame, but cannot distinguish an old, queued ETH frame from the
new append. The timestamp argument to `append_priced` is likewise not examined in
the WebSocket-frame predicate.

An implementation which sends a pre-switch ETH frame after the resubscription,
without pumping the post-switch append, satisfies the present predicate. That is
the same causal defect as C-190 B-2: the oracle does not prove delivery of a fresh
post-switch event. Make the expected post-switch data independently identifiable
at the consumer boundary and assert that identity; retaining merely an ETH/BTC
price-class check is insufficient. This is R-1/R-2 oracle blindness, not a choice
of implementation.

### B-2 — E-3 permits a terminal error to kill the connection and neighbour subscriptions

CT-RFC-09 §2.10 requires that terminality ends the affected subscription while
the connection and neighbour subscriptions remain live. E-3 opens only one
subscription (`red_ws_terminality_entrypoint.rs:730-732`). After the error, its
no-more-frame loop accepts `None` by breaking (`:826-840`); the receive helper also
returns `None` for timeout, close, and non-frame conditions (`:226-236`). Hence a
server which sends the specified error and closes the whole WebSocket passes E-3.

The artifact must establish an independent neighbouring subscription, cause an
identifiable later event on it after the affected subscription terminates, and
assert its frame while the failed subscription remains silent. An equivalent
active proof of both a live socket and a live neighbour is acceptable, but mere
absence of the affected frame is not.

### B-3 — the acceptance gate can pass without E-3, and its mutation control is vacuous

The task-1 count in `scripts/verify_M-72.sh:103-109` admits only functions named
`td_17[0-9]_e...`; E-3 is named `e3_non_cap_midstream...` (`:722`) and is not
counted. Task 5 (`verify_M-72.sh:146-150`) runs only the reducer suite, not the
wire entrypoint suite. Consequently a future green implementation can omit E-3
and still satisfy both checks. This violates the per-task executable-oracle
requirement and the absence property in `.claude/rules/testing.md`.

The mutation check at `verify_M-72.sh:224-247` also accepts only `E2 != 0` and
`E1 == 0`. Its unmutated E-2 is already RED because the current server emits
`invalid_selector` instead of `subscription_terminated`, as the direct E-2 run
shows. Thus `E2 != 0` cannot attribute failure to the mutation. The gate needs a
green baseline/control that isolates terminality before using the mutant as
evidence. Its seam check (`:178-194`) still searches the removed imaginary names
`pump_gate|pump_started|test_seam`, rather than the actual
`test_sync::rendezvous`, and so does not establish that the production build lacks
the real testing seam.

### B-4 — the milestone has neither the T2 wire signature nor an allowed path for it

CT-RFC-09 §2.10 makes `reason` part of the v1 terminal-error wire envelope. Yet
the only committed helper remains
`gateway_serve::wire_v1::error_msg(sub, code, message)`
(`crates/gateway-serve/src/wire_v1.rs:180-195`), and the two sender sites call that
three-argument helper (`lib.rs:1091-1096`). The milestone names
`crates/gateway-serve/src/{lib.rs,wire_v1.rs}` as required work for task 5, but its
top-level Allowed paths omit `wire_v1.rs` (M-72 `:47-55`). It also does not specify
the changed T2 boundary or how ordinary v1 errors retain their existing form.

Before dispatch, record the helper/type signature and its normal-error versus
terminal-error semantics in the committed artifact set, and add the necessary
implementation path to M-72's allowed scope. This is required T2/scope precision,
not a request to change production code in this gate.

## FA and checked non-blockers

`crates/gateway` and `crates/gateway-serve` have no dedicated FA documents. This
is declared debt, not an implicit waiver. The audit applies `docs/fa/viz-backend.md`
**VB-I-2** (live equals replay) and DESIGN §22's `GW-I` inverse-drift measurement
(declared 0, in oracles 13). `check_review_fa.sh` is not proof here: for this range
it reports `SKIP` because the production paths are untouched.

FA-WAIVER: crates/gateway — dedicated FA absent; audit uses viz-backend VB-I-2 and
DESIGN §22 GW-I inverse drift (declared 0, oracle 13).

FA-WAIVER: crates/gateway-serve — dedicated FA absent; audit uses viz-backend
gateway-serve boundary and DESIGN §22 GW-I inverse drift (declared 0, oracle 13).

The intentional reducer REDs remain isolated: TD-179 M-2 fails while M-1 passes,
and TD-180 S-2 independently fails. No finding reopens their surface independence.

## Done Block

```text
$ git ls-remote --heads origin feat/M-72-subscription-terminality
bd1a5578045000e85d47066e647eedde78aa941f	refs/heads/feat/M-72-subscription-terminality
exit=0

$ git rev-parse HEAD
bd1a5578045000e85d47066e647eedde78aa941f
$ git merge-base origin/main HEAD
d1221b1ca932d0b8e95403c2849308ed6e7b9ce2
exit=0

$ cargo test -p gateway-serve --test red_ws_terminality_entrypoint --quiet
running 3 tests
td_178_e1_cap_terminality_has_no_post_error_frame ... ok
e3_non_cap_midstream_failure_is_terminal_on_the_wire ... FAILED
td_178_e2_stale_pump_does_not_kill_new_sub ... FAILED
test result: FAILED. 1 passed; 2 failed
exit=101

$ cargo test -p gateway-serve --features testing --test red_ws_terminality_entrypoint td177_stale_pump_does_not_kill_new_sub --quiet
td177_stale_pump_does_not_kill_new_sub ... FAILED
actual error code: invalid_selector; expected subscription_terminated
exit=101

$ cargo test -p gateway --test red_pump_midstream_failure --quiet
td_179_m2_failed_pump_must_not_leave_cursor_ahead_of_delivered ... FAILED
test result: FAILED. 1 passed; 1 failed
left: Some(767)
right: None
exit=101

$ cargo test -p gateway --test red_snapshot_cursor_honesty --quiet
td_180_s2_snapshot_declares_state_position_not_delivery_bookmark ... FAILED
test result: FAILED. 1 passed; 1 failed
left: None
right: Some(24999)
exit=101

$ bash scripts/verify_M-72.sh
PASS: M нейтрализация терминальности → E-2 FAILED (exit=101), E-1 цел (exit=0); BUILD_EXIT=0
VERDICT: FAIL (8)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS [2-COVERAGE] §22 GW-I declared=0,in oracles=13
VERDICT: PASS (0 нарушений)
exit=0

$ git grep -n 'scripts/verify_M-71' -- crates/ deploy/
crates/gateway/src/lib.rs:1964:/// ЯКОРЬ МУТАЦИИ (`scripts/verify_M-71.sh` шаг C). Сигнатура зафиксирована спекой;
match-file-count=1
exit=0

$ bash scripts/next_artifact_id.sh C
C-192
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git merge-base HEAD origin/main) bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона d1221b1..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```

## Handoff

### §A — Status

REJECT, round 2. M-72 remains blocked; no engine-dev dispatch is authorized.

### §B — Why the next agent is an arbiter

B-1 is the same unclosed causal reason as C-190 B-2: the TD-177 oracle still cannot
prove fresh post-switch delivery. Under `.claude/rules/gates.md` §0 and the critic
profile, a second REJECT on one reason goes to an arbiter rather than a third
architect/critic self-fix loop. B-2 through B-4 are additional blockers for the
same arbitration record.

### §C — Audited artifact and push target

Audited remote head: `bd1a5578045000e85d47066e647eedde78aa941f` on
`feat/M-72-subscription-terminality`. This verdict is committed and pushed to that
same subject branch.

### §D — Next agent: arbiter

Paste-ready prompt:

> You are the arbiter for M-72-subscription-terminality after C-190 and C-192.
> Read the full milestone, CT-RFC-09 §2.10, `.claude/rules/gates.md`,
> `.claude/rules/testing.md`, the full C-190 and C-192 verdicts, and the committed
> artifact set at C-192's audited head. Decide whether TD-177's identical ETH price
> class proves a post-switch append, and bind an executable condition that does;
> adjudicate the neighbour-liveness proof and the verify-script/mutation controls.
> Preserve the already chosen `subscription_terminated` plus `reason` form. Return
> a committed arbitration record; do not dispatch engine-dev until the record is
> resolved.

### §E — Recheck commands

After arbitration and any architect artifacts, audit the new remote SHA and rerun
the four named RED commands, `bash scripts/verify_M-72.sh`,
`bash scripts/check_artifact_ids.sh`, and
`bash scripts/verify_design_claims.sh --merge-preview origin/main`.
