<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: 08741d9b9bd21a2b40ba7bef97c57d97b2204099
audited_head: 85ae7de9bb600676bd95432bcb76551d683c1556
verdict: REJECT
-->

# C-219 — M-74 restore-drill, round 6: REJECT

## Verdict

**REJECT — do not dispatch engine-dev.** Founder authorization for round 6 is accepted; this
verdict neither reopens `A-032`'s settled pseudonym-form/local-network boundary nor the closed
`C-218` continuity and selected-file-form findings. It finds two other executable omissions in
the committed artifact set.

The changed `crates/journal/tests/**` artifact concerns the live invariant **JR-I-6**: old
journals must remain readable by new code. The milestone's operational objective is also
**OPS-I-3**: the cold copy is downloaded and read. The complete plan-time set is present:
the T2 state/wrapper/reader contracts and signatures are in the milestone, the architect-owned
fixture and RED probe exist, and `scripts/verify_M-74.sh` is a counted, nonzero-on-FAIL gate.
The range `08741d9..85ae7de` changes only the allowed fixture, probe, and milestone paths.

### B-1 — `INV-DELIVERY` is racy: source absence is not held while it is judged

`inv_delivery` renames `cold` to `cold-hidden-$$`, reads `restore`, then tries to rename it
back (`scripts/tests/red_restore_drill.sh:395-404`). It has no fence or postcondition that
the original source name remained unavailable. A wrapper can leave a child watching that name;
after the probe moves `cold`, the child recreates `cold` as a link to the hidden source. The
restored per-file links then read successfully, so `inv_delivery` and therefore `h_verdict`
accept a delivery which contains no bytes of its own.

This is not an `A-032`-settled alias-form request. It is the explicitly named source-rename
race, reproduced by execution. The attacker keeps the declared members, forms, state digest,
and live-source digest valid; only the temporal assertion "source is unavailable" is false.

**Condition to clear:** add an executed RED scenario in which the wrapper leaves this rebinding
watcher behind. Its setup must prove the watcher re-created the source name after the probe's
rename while all member/digest/form guards remain valid; the delivery predicate must reject it.
The proof must also establish that the source remains unavailable for the entire independent
read, not merely that its first rename succeeded.

### B-2 — required sidecars can be silently omitted or substituted

The selection contract says that the sidecars present in the cold copy are restored
(`milestones/M-74-restore-drill.md:486-489`). The decisive predicate instead checks only
segment-member identity, segment forms, state digest, and source independence
(`scripts/tests/red_restore_drill.sh:411-419`). No predicate inventories or compares
`*.json` sidecars.

Replacing the reference wrapper's sidecar-copy loop with a no-op left `H` green and the probe
at only its two declared production RED lines. `journal::stream` can still read this fixture,
so no existing segment, digest, or form guard detects the loss. The same blind surface permits
a substituted sidecar with valid selected segments.

**Condition to clear:** execute an adversarial scenario where every selected segment, form,
state field, digest, and `INV-DELIVERY` condition remains valid, but a sidecar present in
`cold` is omitted or replaced. It must fail specifically on a declared sidecar-delivery
property; the contract must make the required identity/integrity relation explicit rather
than relying on the reader to happen to consume a particular sidecar today.

## Confirmed closures

- `C-218` B-1 is closed: shifting every header-derived boundary makes the healthy non-adjacent
  oracle fail conservatively; removing the reset fails the same oracle, and an always-true
  continuity mutation fails the intra-segment-gap oracle.
- `C-218` B-2 is closed for selected segment forms: removing the form guard accepts A10; its
  exclusive scenario therefore pins the guard.
- The five declared per-member probe guards and the group mutation behave as stated: digest
  exposes D/R, membership exposes A8, report agreement exposes A9, forms expose A10, and
  source removal exposes A1. A four-member delivery (`00/01/03/07`) is rejected by exact
  member identity.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-219
allocator_exit=0

$ git diff --name-status 08741d9..85ae7de
M	crates/journal/tests/fixture_restore_drill_cold.rs
M	milestones/M-74-restore-drill.md
M	scripts/tests/red_restore_drill.sh
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m74-r6/target cargo test -p journal --test fixture_restore_drill_cold --quiet
running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
fixture_suite_exit=0

$ boundary-reset mutation: cargo test -- --exact non_adjacent_healthy_selection_is_continuous
non_adjacent_healthy_selection_is_continuous --- FAILED
boundary_reset_mutation_exit=101

$ always-true mutation: cargo test -- --exact intra_segment_gap_is_discontinuous
intra_segment_gap_is_discontinuous --- FAILED
always_true_mutation_exit=101

$ header-first-seq mismatch mutation: cargo test -- --exact non_adjacent_healthy_selection_is_continuous
non_adjacent_healthy_selection_is_continuous --- FAILED
header_first_seq_mismatch_exit=101

$ digest guard mutation
FAIL  D обёртка, восстановившая копию, но ВЫДУМАВШАЯ отпечаток, признана честной.
FAIL  R отпечаток ЧУЖОГО прогона принят как свой.
VERDICT: FAIL (4 из 22)
digest_mutation_exit=1

$ membership guard mutation
FAIL  A8 обёртка, доставившая ТРИ НАСТОЯЩИХ, но НЕ ТЕ сегмента, признана честной.
VERDICT: FAIL (3 из 22)
identity_mutation_exit=1

$ report-agreement guard mutation
FAIL  A9 обёртка, доставившая верную выборку, но объявившая checked=99, признана честной.
VERDICT: FAIL (3 из 22)
report_mutation_exit=1

$ forms guard mutation
FAIL  A10 обёртка, не привёзшая СЫРУЮ форму индекса 00000003 при верном составе, признана честной.
VERDICT: FAIL (3 из 22)
forms_mutation_exit=1

$ source-removal guard mutation
FAIL  A1 обёртка, положившая ССЫЛКИ вместо байтов, признана честной.
VERDICT: FAIL (3 из 22)
inv_mutation_exit=1

$ group mutation of h_verdict
FAIL  D ... признана честной
FAIL  X ... признана честной
FAIL  A1 ... признана честной
FAIL  A6 ... признана честной
FAIL  A7 ... признана честной
FAIL  A8 ... признана честной
FAIL  A9 ... признана честной
FAIL  A10 ... признана честной
FAIL  R ... принят как свой
VERDICT: FAIL (11 из 22)
group_mutation_exit=1

$ extra-member attack (00/01/03/07)
PASS  A8 НЕ ТЕ члены выборки ПОЙМАНЫ: доставлено «00000000 00000001 00000003 00000007» вместо «00000000 00000003 00000007»
VERDICT: FAIL (2 из 22)
extra_member_attack_exit=1

$ source-rename race attack
FAIL  A1 обёртка, положившая ССЫЛКИ вместо байтов, признана честной. Доставки не было:
VERDICT: FAIL (3 из 22)
rename_race_attack_exit=1

$ sidecar-omission mutation
PASS  H здоровая копия ⇒ drill прошёл: ok=1, событий 4125, сегментов 3, сжатых в выборке 2
PASS  Q читатель говорит ПОЛНЫМ объявленным протоколом (шесть полей) и на здоровом несмежном восстановлении объявляет непрерывность ВНУТРИ сегментов: true
VERDICT: FAIL (2 из 22)
sidecar_omission_mutation_exit=1

$ CARGO_TARGET_DIR=/tmp/hft-critic-m74-r6/target bash scripts/verify_M-74.sh
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
PASS: фикстура прод-формы читается journal::stream (исполнено тестов: 9)
FAIL: bash scripts/tests/red_restore_drill.sh
FAIL: test -x deploy/bin/journal-restore-drill-cron.sh
FAIL: test -f crates/journal/src/bin/journal-drill-read.rs
FAIL: test -f deploy/cron.d/journal-restore-drill
VERDICT: FAIL (15)
verify_M74_exit=1
```

=== HANDOFF: CRITIC → FOUNDER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-08T22:42Z
- Milestone: M-74-restore-drill
- Статус: BLOCKED — REJECT in founder-authorized round 6
- HEAD: 85ae7de — fix(M-74): C-218 closure

## §B — Что я сделал
- Audited the committed round-6 artifacts and full declared mutation set.
- Reproduced the source-rebinding race and sidecar-delivery omission against the committed probe.

## §C — Артефакты / результаты
- `research/critiques/C-219-M-74-round6.md`
- Done Block: fixture suite exit=0; declared RED/verify paths remain red; two new executable omissions block dispatch.

## §D — Следующий агент + инвокация
- **Следующий агент:** `founder`
- Push-статус: recorded after this verdict commit.
- Кэш: recorded after handoff.
- **Paste-ready промпт:**
  ```
  M-74 round 6 is REJECT by C-219. Do not dispatch engine-dev. The committed plan-time set has two new executable gaps: INV-DELIVERY accepts a wrapper that rebinds cold after the probe renames it, and required JSON sidecars can be omitted/replaced while H remains green. Founder authorization is required before any further round; an architect-owned RED closure must make both attacks fail by their named properties.
  ```

## §E — Риски / открытые вопросы
- The alias-form and local-network limits decided by A-032 were not reopened.
- The missing production wrapper and reader remain the two declared RED lines; they do not excuse plan-time oracle gaps.

=== END HANDOFF ===
