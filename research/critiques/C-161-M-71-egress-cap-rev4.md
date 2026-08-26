<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: 7083c4952ed784e4ed306104cc42a6d78dc5fb29
verdict: REJECT
-->

# C-161 — M-71 egress cap rev4: REJECT

## Scope audited

Audited committed delta `5e540b7..7083c495` (`89e60ca`, `21cf6b3`, `7083c49`) on the declared
chain `3b496208..7083c495`, with binding arbitration `A-021` applied.  The complete M-71
plan-time set is present: Level-1 RED suite (9 tests), separate COMPILE-RED boundary,
Level-2 wire suite (7 tests), startup RED suite (10 tests), mechanical-door probe,
`verify_M-71.sh`, and rev4 milestone.  This is non-T1: the range has no
`crates/contracts/**` change and introduces no contract type or trait signature.  No
`crates/**/src/**` file is changed by this three-commit delta; its implementation surface
remains exactly the declared future `gateway` / `gateway-serve` work.

`VB-I-10` is material: the visible snapshot must remain a bounded operational object.  Here
that includes the error `Message::Text` emitted by the v1 session, not merely the successful
snapshot/frame forms.

## Verdict: REJECT

`C-159 R2` is closed: an independent, deliberately incomplete draft byte guard made **8 of
9** Level-1 tests green, including B; C alone stayed red because `frames_since` still emitted
7,840,977 B.  The B repair therefore no longer demands an impossible fact of an honest cap.

`C-159 R1` closes the *door discovery* defect, but W4 introduces a distinct executed
false-RED: it does not exercise the live `handle_v1_message → send_v1_error → sink.send`
path that it claims to judge.  Instead it duplicates the old handler's error-text transform
in test-local `describe()`.  A correct implementation that chooses the expressly permitted
remedy “do not echo the unknown venue” leaves actual outbound error text small, yet W4 still
fails with 2,100,084 B from its private duplicate.  Thus a conforming implementation cannot
make the artifact set green without changing an oracle fact unrelated to its observed egress.

This is an executable new class, not any named A-021 residual:

- it is not unbounded `sub` (the probe uses `sub="s1"`);
- it is not a macro/trait door or a proxy above `N_MAX_BANDS`;
- it is not text built outside a `*_msg` builder (the target is `wire_v1::error_msg`, which
  the mechanical probe finds);
- it is not the named cognitive-only `Message::Text` residual, which the probe prints as
  NOTE.

Per A-021, this is its final allocated correction round: return automatically to arbitration;
do not dispatch dev from this plan-time artifact set.

### F-161-1 — W4 judges a test reconstruction, not outgoing behavior

The production route in `gateway-serve/src/lib.rs` parses the selector, applies its own
`UnknownVenue` message choice, then invokes `send_v1_error`, which serializes and sends the
text.  W4 calls only `parse_selector`, its own `describe`, and `wire_v1::error_msg`; it never
calls the handler or a sink.  Its comment says that no echo is an allowed treatment, yet
`describe` unconditionally does `format!("unknown venue: {name}")`.

In a disposable mutation I changed the live handler only from
`UnknownVenue(name) => format!("unknown venue: {name}")` to
`UnknownVenue(_) => "unknown venue".to_string()`.  That is an allowed bounded live egress
treatment.  W4 nevertheless failed at 2,100,084 B because it regenerated the old echo in the
test.  This is not a dispute over the founder-owned number: the false RED remains for every
finite proposed cap below its synthetic 2.1-MB reconstruction.

The critic does not prescribe the implementation form.  Arbitration must decide the artifact
correction required for an oracle that observes the actual session egress while retaining the
existing bounded-error and non-degeneration requirements.

## Required checks and bounded findings

### R1 doors and unobserved text

R1's mechanical Level-2 inventory is repaired.  A temporary public
`wire_v1::critic_uncovered_msg` not called by the oracle makes the probe fail exactly once.
Baseline finds `error_msg`, both v1 success builders, and both `serve` builders.  The direct
`Message::Text` construction outside `*_msg` remains explicitly printed as a cognitive-only
NOTE, exactly the named A-021 residual.  I found no outgoing-text class outside the four named
residuals other than F-161-1's false oracle; F-161-1 is about the test's unobserved live route,
not a missed builder door.

### W4's paired vantage and mutation C

W-C3 does reject the degenerate “no error is supplied” builder mutation: replacing
`error_msg` by `{}` makes its required error code absent.  That is useful but does not repair
W4's failure to observe the live handler/sink path.

C now names both levels: its mutation runs the Level-1 suite and the Level-2 wire suite, and
requires the honest E control to stay green.  Its base-green prerequisite prevents the former
dead-anchor PASS.  On this intentional RED plan it correctly prints `C НЕ ГОТОВ`, so its
positive mutation phase cannot be claimed yet.  More importantly, F-161-1 makes the complete
set falsely red even after the bounded live-handler treatment above; therefore no conclusion
that C is green for a fully honest implementation is available from rev4.  This is the same
new false-RED blocker, not an additional C finding.

### Founder-owned 2,000,000 B

I neither approve nor set the value.  The evidence still makes **2,000,000 B** a coherent
provisional operating magnitude for the error route and success routes:

- ordinary wire controls are under 200,000 B (at least 10× headroom);
- the repaired cheap eight-band request measured 152,588 B (13.1× headroom);
- the client-controlled error is 2,100,084 B (1.05×); dense success wire is 2,804,778 B
  (1.40×); the wide-book path is 7,841,085 B (3.92×);
- at the configured maximum of 16 subscriptions, a per-message 2-MB ceiling implies a
  nominal 32,000,000-B connection-scale ceiling, while it does not decide output composition
  or `GATEWAY_BANDS`.

The number is large relative to demonstrated normal traffic and constrains reproduced abusive
egress.  Founder retains the decision; the blocker is that W4 does not measure whichever
bounded live treatment implementation chooses.

## Done Block

```text
$ git ls-remote origin refs/reserved/C-161
1be875c03b1e82e31bb284fb0a20d8b1cd686dd7	refs/reserved/C-161
exit=0

$ [temporary live-handler mutation: UnknownVenue(_) => "unknown venue"] \
  CARGO_TARGET_DIR=/tmp/hft-critic-m71-r4-w4-mutation/target \
  cargo test -p gateway-serve --test red_egress_cap_wire \
  pl_i_5_w4_client_controlled_error_text_is_capped -- --nocapture
running 1 test
PL-I-5 W4 НАРУШЕН: наружу уходит 2100084 Б текста ОШИБКИ при `sub` длиной 2
test pl_i_5_w4_client_controlled_error_text_is_capped ... FAILED
exit=101

$ [temporary uncalled public builder] bash scripts/tests/red_egress_doors.sh
FAIL: дверь L2 wire_v1::critic_uncovered_msg строит исходящий текст, но оракул \
crates/gateway-serve/tests/red_egress_cap_wire.rs её НЕ ЗОВЁТ
VERDICT: FAIL (1) — дверь существует, а оракул её не зовёт
exit=1

$ [temporary error_msg => {}] CARGO_TARGET_DIR=/tmp/hft-critic-m71-r4-w4-mutation/target \
  cargo test -p gateway-serve --test red_egress_cap_wire \
  pl_i_5_w_c3_honest_error_is_still_delivered -- --nocapture
running 1 test
PL-I-5 W-C3: сообщение об ошибке обязано нести КОД (unknown_venue); получено: {}
test pl_i_5_w_c3_honest_error_is_still_delivered ... FAILED
exit=101

$ [temporary snapshot-only, named InvalidInput byte guard] \
  CARGO_TARGET_DIR=/tmp/hft-critic-m71-r4-w4-mutation/target \
  cargo test -p gateway --test red_egress_cap -- --nocapture
running 9 tests
test pl_i_5_b_no_silent_truncation_on_either_side ... ok
test pl_i_4_c_limit_has_no_bypass_across_entry_points ... FAILED
test result: FAILED. 8 passed; 1 failed
PL-I-4 C [широкая книга]: `frames_since` отдал 1 кадров, крупнейший — 7840977 Б
exit=101

$ bash scripts/verify_M-71.sh
PASS: A состав набора — 9 оракулов (ожидалось ровно 9: A A-2 B C F E E-2 E-3 E-4)
PASS: A2 состав набора — 7 оракулов (ожидалось ровно 7: W1 W2 W3 W4 W-C1 W-C2 W-C3)
VERDICT: PASS — все найденные двери названы в оракулах
PASS: B состав набора — 10 оракулов (ожидалось ровно 10: 8 отказов + 2 vantage)
FAIL: C НЕ ГОТОВ — набор КРАСЕН и без мутации, судить нейтрализацию предела не по чему.
VERDICT: FAIL (8)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --check 3b496208a64edbf00a66b93986ff8529d0c93aa9..7083c4952ed784e4ed306104cc42a6d78dc5fb29
exit=0

$ bash scripts/check_artifact_ids.sh 7083c4952ed784e4ed306104cc42a6d78dc5fb29
OK: ни один коммит диапазона 7083c49..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ bash scripts/check_gate_meta.sh 7083c4952ed784e4ed306104cc42a6d78dc5fb29
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone \
в subject'е: 0
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Metadata
- Date (UTC, ISO-8601): 2026-08-26T07:54:45Z
- Milestone: M-71-egress-cap, final correction round allocated by A-021
- Status: BLOCKED / REJECT
- Audited base/head: `3b496208a64edbf00a66b93986ff8529d0c93aa9` / `7083c4952ed784e4ed306104cc42a6d78dc5fb29`

## §B — What was audited
- Audited committed rev4 artifacts, not plan prose alone; executed R1 door and W4/W-C3 mutations,
  an independent R2 draft, both verifier commands, and range/scope checks.

## §C — Artifact and result
- `research/critiques/C-161-M-71-egress-cap-rev4.md`
- New executed class: W4 reconstructs the old echo in test-local code and rejects a bounded
  actual handler treatment; REJECT is outside A-021's named NOTE residuals.

## §D — Next agent and paste-ready invocation
- **Next agent:** `arbiter` (automatic per A-021 final-round rule).
- **Invocation:**
  ```text
  Arbitrate C-161 against A-021 for M-71.  Audit committed HEAD 7083c495 with base
  3b496208.  The final-round critic executed a handler-only mutation that replaces unknown-
  venue echo with a fixed short message; actual handler egress is then bounded, but W4 still
  fails at 2,100,084 B because W4 calls parse_selector + test-local describe() + error_msg,
  never handle_v1_message/send_v1_error/sink.  Decide whether this is the new false-RED class
  claimed by C-161 and issue the binding disposition.  Preserve founder ownership of 2 MB and
  do not decide GATEWAY_BANDS/output composition.
  ```
- Push status: ✅ pushed by `git push origin HEAD:feat/M-71-egress-cap`; the active subject
  worktree must use `git pull --ff-only` before its next action.
- Cache: ✅ `rm -rf /tmp/hft-critic-m71-r4/target` semantic equivalent completed
  (`find … -depth -delete`; the directory is absent), as is the disposable mutation target.

## §E — Boundaries
- The door probe catches a new `*_msg`; text assembled outside such a builder remains the named
  cognitive-only NOTE residual.
- C's positive mutation phase remains unavailable until a correct full artifact set is green;
  do not treat plan-time RED as a C PASS.

=== END HANDOFF ===
