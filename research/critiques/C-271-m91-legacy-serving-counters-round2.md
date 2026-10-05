# C-271 — M-91 legacy serving counters, round 2: NOTE

<!-- GATE-META
milestone: M-91
audited_repo: a3ka/hft-platform
audited_base: 395323c5f6a403f9b6c17306cc7f9f98f521e155
audited_head: 243ac7b669a3b0ca99d3b696e56a52d07413abd9
verdict: NOTE
-->

## Verdict

**NOTE.** The complete committed architect set closes C-269. Development may proceed after architect adds this verdict as the mechanical milestone appendix and dispatches the named engine-dev task.

l4 supplies the missing Warming world on the legacy entry path. It asserts the actual warming reply and exact per-instance counter deltas (attempts, successes, refusals_supported, refusals_unsupported) = (1, 0, 1, 0). The mutant that counts only NotReady leaves l1-l3 green but makes l4 fail, so the former blind spot is now discriminated.

## Subject and artifact completeness

The fetched subject was origin/feat/M-91-legacy-serving-counters at 243ac7b669a3b0ca99d3b696e56a52d07413abd9; the final fetch reported the same head.

| Required artifact | Evidence |
|---|---|
| milestone | milestones/M-91-legacy-serving-counters.md, updated at 243ac7b |
| T-contract / trait signatures | none: no contracts/** change; the existing ServingCountersHandle is consumed |
| RED suite | crates/gateway-serve/tests/red_m91_legacy_counters.rs |
| acceptance gate | scripts/verify_M-91.sh, explicit FAIL-counting aggregator |
| CI-map anti-plaque probe | scripts/tests/red_verify_M-91_ci_map.sh |

The live operational invariants are OPS-I-8 (a live but silent flow alerts) and OPS-I-10 (declared observability is emitted by its producer). gateway-serve has no dedicated FA on this revision, a limitation that the milestone names.

## Checks

- l1-l3 remain behavioural RED on the existing legacy path: success, not_ready, and two-session exact-delta worlds each fail with the observed zero tuple.
- Under feature testing, l4 is compile-RED with exactly E0599 for Server::with_readiness_override. Once engine-dev adds the test-only seam and connects it to the legacy readiness outcome, l4 will exercise a real WebSocket session and reject an implementation that increments refusals_supported only for NotReady.
- The planned seam is constrained by milestone §5 to cfg(feature = testing). The production Dockerfile uses cargo build --release with no feature flag, and no deployment/Compose build command enables testing. The acceptance script explicitly runs the feature test because the ordinary CI cargo test --all does not. This boundary is named rather than assumed.

No T1 change, unsafe scope expansion, acceptance-script masking, or Boundary-C action is present.

## Done Block

    $ git fetch origin --prune && git rev-parse origin/feat/M-91-legacy-serving-counters
    243ac7b669a3b0ca99d3b696e56a52d07413abd9
    exit=0

    $ cargo test -p gateway-serve --test red_m91_legacy_counters
    test l2_legacy_not_ready_counts_attempt_and_supported_refusal ... FAILED
    test l1_legacy_success_counts_one_attempt_and_one_success ... FAILED
    test l3_two_legacy_sessions_count_exactly_two ... FAILED
    assertion failed: not_ready legacy delta was (0, 0, 0, 0), expected (1, 0, 1, 0)
    assertion failed: legacy success delta was (0, 0, 0, 0), expected (1, 1, 0, 0)
    assertion failed: two legacy sessions delta was (0, 0, 0, 0), expected (2, 2, 0, 0)
    test result: FAILED. 0 passed; 3 failed
    exit=101  # expected runtime-RED

    $ cargo test -p gateway-serve --features testing --test red_m91_legacy_counters
    error[E0599]: no method named with_readiness_override found for struct gateway_serve::server::Server in the current scope
    error: could not compile gateway-serve (test red_m91_legacy_counters) due to 1 previous error
    exit=101  # expected compile-RED; only the new test-only hook is absent

    $ bash scripts/tests/red_verify_M-91_ci_map.sh
    PASS  честный ci.yml: всё учтено (exit=0)
    PASS  новый шаг с expressions outside map ⇒ FAIL (exit=1)
    PASS  исключение без шага ⇒ карта протухла (exit=1)
    PASS  строка дописана в блок базы ⇒ FAIL (exit=1)
    PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
    PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
    PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
    PASS  check_review_fa + дописка ⇒ исполняется, не SKIP (exit=0)
    PASS  run: |- ⇒ EXEC
    PASS  run: > ⇒ FAIL
    VERDICT: PASS — 10 scenarios
    exit=0

    $ bash -n scripts/verify_M-91.sh
    exit=0

    $ VERIFY_M91_CI_DRY=1 bash scripts/verify_M-91.sh
    PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
    VERDICT: PASS
    exit=0

    $ git diff --check origin/main..HEAD
    exit=0

    $ git status --short
    <empty before verdict creation>
    exit=0

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC): 2026-10-02
- Milestone: M-91-legacy-serving-counters
- Status: DONE — NOTE
- HEAD: 243ac7b — docs(M-91): спека круг 2 — I-3bis, l4 ... [architect]

## §B — What I did
- Audited the committed M-91 set, C-269, legacy/v1 readiness and counter paths, RED suite, acceptance gate, and test-feature build boundary.
- Reproduced l1-l3 runtime RED, l4 compile RED, and the 10-world CI-map probe.

## §C — Artifacts / results
- research/critiques/C-271-m91-legacy-serving-counters-round2.md — NOTE.

## §D — Next agent + invocation
- **Next agent:** architect.
- **Paste-ready prompt:**
    Add C-271 as the mechanical appendix to M-91 without changing its approved design, then dispatch engine-dev only for milestone task 1. Implement the legacy-path counters and Server::with_readiness_override exclusively under cfg(feature = testing). Preserve l1-l4; green the feature test through scripts/verify_M-91.sh; commit and push the green implementation before the tester handoff.
- Push status: pending this verdict commit and push to feat/M-91-legacy-serving-counters.
- Cache: target/ will be removed after the commit/push.

## §E — Risks / open questions
- The test-only hook must not be compiled into the production build; the current Docker build omits the testing feature.
- The narrow known limitation about disconnecting exactly between counting and send remains named in milestone §7.

=== END HANDOFF ===
