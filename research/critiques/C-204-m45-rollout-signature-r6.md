<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: 2e63a37e5bf454da69b0fbd69de28c043b4caf4c
audited_head: a24193a72ddafbb19fda57b6d34748d648f2d26a
verdict: REJECT
-->

# C-204 — M-45 rollout signature, round 6: REJECT

Role: critic — RAW plan-time gate (gates.md §1/§9)  
Audited range: 2e63a37..a24193a on docs/M-45-rollout-signature  
FA citation: VN-I-3 is live in docs/fa/venues.md §I. The rollout preserves the
single shared compose value for both Binance consumers; this finding concerns the
epoch-gate oracle, not venue-specific branching.

## Verdict

REJECT — do not dispatch tester/reviewer or merge.

R-167 Б-1 requires E-002 itself to name the declared epoch. Current E-002 does
name own-2026-09-m45-ethusdt, but the newly claimed mechanical closure does not
protect that fact. T9 accepts the epoch when its byte string occurs anywhere in
docs/data-epochs.md; it does not bind the value to the E-002 record or its
EPOCH_ID после fact.

## B-1 — T9 is green when E-002 no longer names the epoch

The success branch is the document-wide search at
scripts/verify_M-45.sh:226:

    elif grep -qF "$T9_EPOCH" docs/data-epochs.md 2>/dev/null; then

In a fresh detached worktree at a24193a, I made one adversarial world:

1. Replaced E-002's EPOCH_ID после value with REDACTED-EPOCH-FOR-MUTATION.
2. Put the original literal only into E-001 as an unrelated archival example.

This violates R-167 Б-1: the E-002 record no longer tells a future reader what
marks the ETHUSDT rollout boundary. Yet the full acceptance gate stays green,
including T9, because the generic grep -qF finds the unrelated E-001 occurrence.

This is not prose-only. E-002 is the registry record analysts must use for the
ETHUSDT boundary; accepting a matching token in another epoch record recreates
the ambiguous lookup that B-1 was intended to eliminate. The requested deletion
mutation proves only total absence from the whole file, not association with E-002.

Condition to clear REJECT: make T9 verify the declared value in E-002's specific
record/fact, and add an anti-placebo mutation that removes it from E-002 while
retaining the same literal outside E-002. That mutation must make T9 and
verify_M-45.sh fail.

## Positive checks reproduced

- The committed set contains the milestone, existing T1 context (no T1 change),
  RED suites, a real verify gate, compose rollout, and the shared comparator.
- Baseline verify_M-45.sh passed with 24 PASS and exit 0.
- All seven mandated independent mutations failed their claimed step: E-002-token
  deletion → T9; declared-epoch removal → T10c; broken T9 extract → T9 setup;
  weak symbol comparator and disabled compose wiring → T10c; disabled strip_keys
  → T10c setup; clock epoch in real compose → T10.
- verify_design_claims --merge-preview origin/main and all five CI-form barriers
  passed. check_review_fa correctly returned SKIP because the range does not
  modify crates/**.

## Scope and routing

The new comparator is named in M-45 Allowed paths. Its absence from the
process-layer/harness protection sets remains explicitly declared in M-45 §5; it
is not silently treated as closed here.

This is the next REJECT on the R-167 B-1 epoch-record condition. Under gates.md
§0 and the critic profile, do not start a third architect↔critic loop for this
same cause: send this artifact and R-167 to a fresh-context arbiter.

## Done Block

    $ git ls-remote --heads origin docs/M-45-rollout-signature
    a24193a72ddafbb19fda57b6d34748d648f2d26a refs/heads/docs/M-45-rollout-signature

    $ git rev-parse HEAD
    a24193a72ddafbb19fda57b6d34748d648f2d26a

    $ git merge-base HEAD origin/main
    2e63a37e5bf454da69b0fbd69de28c043b4caf4c

    $ bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'; echo exit=$?
    PASS  T9 раскатка исполнена И эпоха 'own-2026-09-m45-ethusdt' названа в docs/data-epochs.md
    PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT EPOCH_ID=own-2026-09-m45-ethusdt)
    PASS  T10b состав и эпоха внесены ОДНИМ коммитом (f3b84d41)
    PASS  T10c мутация состава: 9 миров compose (каждый под setup-guard'ом) + 7 сценариев значений через ТОТ ЖЕ CLI, что и T10
    VERDICT: PASS
    exit=0

    $ CARGO_TARGET_DIR=<shared-cache> bash scripts/verify_M-45.sh  # seven isolated mutation worktrees
    m1 epoch absent from registry          → FAIL T9; verify_exit=1
    m2 DELIBERATE_EPOCH disabled           → FAIL T10c; verify_exit=1
    m3 T9 --extract path broken            → FAIL T9 SETUP НЕ СОСТОЯЛСЯ; verify_exit=1
    m4 if got == SIGNED → if got           → FAIL T10c; verify_exit=1
    m5 bad = check_*() → bad = []          → FAIL T10c; verify_exit=1
    m6 strip_keys → return text            → FAIL T10c SETUP НЕ СОСТОЯЛСЯ; verify_exit=1
    m7 compose epoch → own-2026-08         → FAIL T10; verify_exit=1

    $ rg -n 'REDACTED-EPOCH-FOR-MUTATION|архивный пример own-2026-09-m45-ethusdt' docs/data-epochs.md
    53:| EPOCH_ID после | **REDACTED-EPOCH-FOR-MUTATION** — ...
    93:**Что не так.** ... архивный пример own-2026-09-m45-ethusdt.

    $ CARGO_TARGET_DIR=<shared-cache> bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'; echo exit=$?
    PASS  T9 раскатка исполнена И эпоха 'own-2026-09-m45-ethusdt' названа в docs/data-epochs.md
    PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT EPOCH_ID=own-2026-09-m45-ethusdt)
    PASS  T10c мутация состава: 9 миров compose (каждый под setup-guard'ом) + 7 сценариев значений через ТОТ ЖЕ CLI, что и T10
    VERDICT: PASS
    exit=0

    $ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
    VERDICT: PASS (0 нарушений)
    exit=0

    $ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_gate_meta.sh; echo exit=$?
    VERDICT: PASS — вердиктов проверено: 11, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
    exit=0

    $ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_protected_artifacts.sh; echo exit=$?
    OK: защищённые артефты целы на HEAD (2e63a37..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
    exit=0

    $ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_docs_freeze.sh; echo exit=$?
    exit=0

    $ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_artifact_ids.sh; echo exit=$?
    OK: ни один коммит диапазона 2e63a37..HEAD не ввёл второй носитель под занятым идентификатором
    exit=0

    $ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_review_fa.sh; echo exit=$?
    SKIP (диапазон не трогает crates/**)
    exit=0

    $ git diff --check a24193a^..a24193a; echo exit=$?
    exit=0
