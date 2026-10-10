<!-- GATE-META
milestone: M-72
audited_repo: a3ka/hft-platform
audited_base: d1221b1ca932d0b8e95403c2849308ed6e7b9ce2
audited_head: 888f18a0ab2dce1f4a5af3fc52960841716eaa50
verdict: REJECT
-->

# C-200 — M-72 subscription terminality, round 3 — REJECT

## Verdict and audited set

**REJECT. Do not dispatch engine-dev.** The audit is of the committed set at
`888f18a0ab2dce1f4a5af3fc52960841716eaa50`, not handoff prose. Its merge-base with
`origin/main` is `d1221b1ca932d0b8e95403c2849308ed6e7b9ce2`, and the remote subject
branch still resolved to the audited SHA before this verdict.

M-72, CT-RFC-09 §2.10, entrypoint RED, TD-179/TD-180 REDs, and `verify_M-72.sh` are
present. `crates/contracts/**` is absent from the range, so Block-C/T1 is not
implicated. The required terminal-error T2 boundary is still absent (B-4).

There is no dedicated FA for `gateway` or `gateway-serve`. This audit therefore
applies `docs/fa/viz-backend.md` **VB-I-2**: the delivered live series must equal
replay of the same journal window. Losing a post-switch frame or advancing past
undelivered frames violates that live/replay contract; the missing FA is debt, not a
waiver.

## A-029 B-1 closure — accepted

The TD-177 oracle now emits `ETH_SENTINEL_PRICE = 7777.77` only after switch and
release, executes `assert_sentinel_is_unique` against pre-switch ETH and BTC, and
proves fresh delivery by the sentinel interval under the re-subscribed id. The
`p >= BTC_PRICE` class remains a separate stale-BTC guard and is no longer evidence
of freshness. This satisfies A-029 §3 conditions 1–2.

The required reproduction matched the supplied matrix: as-is plus a temporary class
predicate was red on stale terminal error (a); temporary generation guards at both
removal sites plus the class predicate were green; retaining those guards but
restoring the sentinel predicate was red on absent fresh `7777.77` delivery (b).
Every temporary source/test change was removed before this verdict.

## Blocking findings

### B-2 — E-3 still permits terminal error plus whole-socket closure

`e3_non_cap_midstream_failure_terminates_with_pump_failed_reason` opens only
`SHORT_SUB` through `subscribed(&addr)`. It establishes neither an independent
neighbour nor a post-error event for one. Its post-error drain accepts `None` with
`break`, while `recv` maps timeout and socket close to the same `None`. Thus a server
that sends the expected error then closes the whole WebSocket passes E-3's silence
half. This violates CT-RFC-09 §2.10: the connection and neighbour subscriptions must
remain live.

Architect must make E-3 establish a separate neighbour, append an R-4-identifiable
event for it after the affected terminal error, assert its frame, and preserve the
affected subscription's silence assertion. Close/timeout is a setup or subject
failure, never successful evidence.

### B-3 — verify remains greener than the wire subject

1. Task 1 counts only `^async fn td_17[0-9]_e[0-9]+_` and expects two functions.
   The `e3_non_cap_midstream_failure_terminates_with_pump_failed_reason` oracle is
   outside that class, so it can disappear without failing the count.
2. Task 5 runs only `red_pump_midstream_failure`, not the gateway-serve entrypoint
   suite that owns the wire contract, reason, and required live neighbour.
3. Step S searches removed `pump_gate|pump_started|test_seam` names beneath the wrong
   `cfg(feature = "testing")` shape. The real production-wired seam is
   `test_sync::rendezvous::pump_signal_and_wait` beneath
   `cfg(any(test, feature = "testing"))`; S therefore prints PASS while observing none
   of its subject.

The mutation result is also not attributable while unmutated E-2 is RED: `E2 != 0`
is true before mutation. It must establish a green baseline in the same run, or
truthfully report the RED-phase condition instead of PASS.

### B-4 — terminal error T2 boundary is neither literal nor permitted

The Allowed paths table permits `crates/gateway-serve/src/lib.rs` but not
`crates/gateway-serve/src/wire_v1.rs`, although task 5 names `wire_v1.rs`. The only
committed helper remains ordinary three-argument
`wire_v1::error_msg(sub, code, message)`. Neither milestone nor T2 declaration gives
the literal terminal helper/type signature with mandatory `reason`, nor says how
ordinary errors retain their current form while terminal errors gain `reason`.

Architect must declare that boundary verbatim and add `wire_v1.rs` to engine-dev's
Allowed paths before dispatch. Naming a file in a task is neither an allowed path nor
a signature.

### N-1 — TD-177's second half contradicts task 3's only fix

The required measurement establishes a separate behaviour defect: adding only
`cap_terminal && current_gen == Some(gen_at_pump)` at both removal sites makes the
old class oracle green but does not deliver the fresh sentinel. The identity oracle
observed one pre-switch ETH frame near `3000.19`, then empty frames, and never
`7777.77`.

The milestone correctly records a defect rather than silently choosing an
implementation, but task 3 still says **"ТОЛЬКО сверка поколения"** and task 5 calls
that condition the whole fix. The executable RED requires behaviour which the written
scope rules out. A developer would have to invent the state-ownership/recovery design
for the new subscription after the old pump returns `live: None`; that is architect
work, not an implementation choice.

Before round 4 architect must revise the task with the exact postcondition and
allowed state transition on both named paths, update the forbidden list for this
surface, and demonstrate a prototype that makes the identity oracle green. This is a
new measured defect, not a reopening of the settled sentinel construction.

### N-2 — CI parity is independently red

Committed `append_priced` formatting in
`red_ws_terminality_entrypoint.rs:537` fails `cargo fmt --all -- --check`: rustfmt
collapses its five-line signature. `verify_M-72.sh` therefore fails CI parity apart
from intentional RED tasks. Restore formatting so gate-red has its declared meaning.

## Conditions for round 4

1. Close B-2 with a live-neighbour socket oracle.
2. Close all B-3 gate defects, including the baseline-aware mutation outcome.
3. Declare the terminal T2 boundary verbatim and permit `wire_v1.rs`.
4. Re-plan the TD-177 second half: task 3 cannot prescribe only generation equality
   while the sentinel RED remains red.
5. Restore CI formatting, commit, and push the amended artifact set.

Per A-029 §2, round 4 is final. A repeat of R-4 sign blindness or B-3's
gate-without-oracle class returns the subject to founder; do not open a fifth loop.

## Done Block

```text
$ git ls-remote --heads origin feat/M-72-subscription-terminality
888f18a0ab2dce1f4a5af3fc52960841716eaa50  refs/heads/feat/M-72-subscription-terminality
exit=0

$ git rev-parse HEAD; git merge-base origin/main HEAD
888f18a0ab2dce1f4a5af3fc52960841716eaa50
d1221b1ca932d0b8e95403c2849308ed6e7b9ce2
exit=0

$ bash scripts/next_artifact_id.sh C
C-200
exit=0

$ cargo test -p gateway-serve --features testing --test red_ws_terminality_entrypoint td177_stale_pump_does_not_kill_new_sub -- --nocapture
TD-177 (а) НАРУШЕН: ... ошибка ушла клиенту по переподписанному «s1»
test td177_stale_pump_does_not_kill_new_sub ... FAILED
exit=101

$ temporary matrix (both `subs.remove` sites guarded by current_gen == Some(gen_at_pump))
code as-is + class predicate    -> FAILED, TD-177 (a), exit=101
generation prototype + class    -> 1 passed, exit=0
generation prototype + sentinel -> FAILED, TD-177 (b): no fresh sentinel, exit=101
temporary source and test predicate restored; git status --porcelain was empty

$ cargo fmt --all -- --check
Diff in crates/gateway-serve/tests/red_ws_terminality_entrypoint.rs:537
FMT_EXIT=1

$ bash scripts/verify_M-72.sh
FAIL: cargo fmt --all -- --check
FAIL: cargo test -p gateway-serve --test red_ws_terminality_entrypoint --quiet
PASS: 1 состав набора — 2 (ожидалось ровно 2: E-1 vantage + E-2 предмет)
FAIL: 3 снятие подписки без сверки поколения — носителей 2 (TD-177 жив)
FAIL: 5 набор задачи 4 ЗЕЛЁН — форма извещения выбрана и реализована
PASS: S шва вне cfg(feature="testing") нет — прод-путь не содержит тестовых ветвлений
PASS: M нейтрализация терминальности → E-2 FAILED (exit=101), E-1 цел (exit=0); BUILD_EXIT=0
VERDICT: FAIL (9)
exit=1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31T00:00Z
- Milestone: M-72-subscription-terminality
- Статус: BLOCKED — REJECT, round 3
- HEAD: 888f18a — audited subject head; C-200 follows on the same subject ref

## §B — Что я сделал
- Audited the committed milestone, RFC, REDs, verify, C-190/C-192, and A-029.
- Reproduced the class/sentinel matrix and restored the diagnostic tree clean.
- Measured B-2, B-3, B-4 unresolved, the separate TD-177 half, and CI-format failure.

## §C — Артефакты / результаты
- `research/critiques/C-200-M-72-subscription-terminality-round3.md`
- Done Block: feature RED=101; class prototype=0; sentinel prototype=101; fmt=1; verify=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  On origin/feat/M-72-subscription-terminality repair only M-72 artifacts per C-200:
  make E-3 prove an independent neighbour's identifiable post-error frame; make verify
  count and run E-3, inspect the real test_sync seam, and attribute mutation to a green
  baseline; declare the literal terminal T2 helper/type with mandatory reason and add
  crates/gateway-serve/src/wire_v1.rs to Allowed paths; re-plan TD-177's second half so
  task 3 does not promise only generation equality while the sentinel RED stays red; run
  rustfmt. Commit and push, then request critic round 4.
  ```
- Push-статус: C-200 is committed and pushed to the subject branch by this critic handoff.
- Кэш: ⏸ кэш оставлен — critic removes its own cache after the verdict push.

## §E — Риски / открытые вопросы
- Round 4 is final under A-029 §2. A repeat of R-4 or B-3 goes to founder, not a fifth loop.
- `gateway` and `gateway-serve` still lack dedicated FA; VB-I-2 is the applicable invariant.

=== END HANDOFF ===
