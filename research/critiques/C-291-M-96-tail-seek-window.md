<!-- GATE-META
milestone: M-96
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: a8d0751b2deb267b5736b885b039eab43d740259
verdict: REJECT
-->

# C-291 — M-96: window bounds have no RED oracle

## Verdict: REJECT

The committed artifact set is present before implementation: new runtime-RED, verify script,
CI-map probe, and milestone. This change introduces no T-type or trait-signature, and the
existing public M-89 contract is retained by `red_m89_seek_contract`.

Task 1 must not be dispatched. The RED suite permits two implementations that violate the
normative numeric bounds in §3/§6. The branch also contains a subject change outside its own
`Allowed paths`, and the observed RED baseline differs from the mandated failure topology.

Live invariant for the touched module: **JR-I-2** — `seq` is strictly monotonic without gaps;
on the shift path the first emitted event is exactly `after + 1` (`docs/fa/journal.md:112`).
`red_m96_tail_seek_window.rs:191-210` asserts this exact range and caught the corresponding
mutant.

## Blocking findings

### B-1 — §3's 8 KiB-per-side minimum and 32 KiB total maximum are not enforced

§3 requires at least 8 KiB backwards and forwards from `approx`, with total window no larger
than 32 KiB (`milestones/M-96-tail-seek-window.md:36-39`). §6 repeats the max-window
prohibition (`:73-76`). Yet the in-window fixtures exercise only `TAIL × SKEW = 500 × 6 ≈
3,000` bytes (`red_m96_tail_seek_window.rs:59-77`); t4 is about 120,000 bytes away.

Temporary mutants, all restored, prove the gap:

| implementation | result | why it matters |
|---|---|---|
| valid 32 KiB, start `approx - 16 KiB` | t1…t4: 4/4 PASS | positive control |
| **4 KiB each side** | t1…t4: 4/4 PASS | violates the mandatory 8 KiB minimum |
| **32 KiB each side / 64 KiB total** | t1…t4: 4/4 PASS | violates the mandatory 32 KiB cap |
| from `header_end` to EOF | t4 FAILED | anti-placebo only catches an extreme full-file scan |

The 256 KiB fast-path budget (`red_m96_tail_seek_window.rs:70-77`) leaves enough slack for
the 64 KiB mutant. t4 distinguishes a full scan, not 64 KiB from the permitted 32 KiB.

**Condition for next review:** provide RED worlds/measurements that fail with margin for both
an implementation below 8 KiB on either side and an implementation above 32 KiB total, while
a legal implementation is green. Include their mutation evidence.

### B-2 — actual subject diff violates its own Allowed paths

§10 permits architect only the milestone, M-96 test, verify script and CI-map probe, declaring
everything else out of scope (`milestones/M-96-tail-seek-window.md:119-124`). But the audited
range contains `M docs/ROADMAP.md` in `a8d0751b docs(ROADMAP): TD-250 → M-96 … [architect]`.
It is a change on this M-96 subject branch but is not an allowed M-96 path.

**Condition for next review:** either remove that change from the subject range or explicitly
and justifiably include `docs/ROADMAP.md` in the milestone scope, then re-present the range.

## Notes

1. Anti-placebo is otherwise substantive. The from-`after` mutant caused t2 to fail with the
   JR-I-2 guard (`expected seq=100000, got 99999`); the full-file mutant caused t4 to fail at
   7,352,972 B for a 1,120,000 B tail, over the 1,048,576 B bisection allowance. The measure
   uses `/proc/thread-self/io`, and `assert_skew` is a setup guard.
2. `w1 ×10` is an empirical acceptance check, not a proof. It passed 10/10 on this host.
   That is consistent with §7: t3 is explicitly host-dependent. The phrase that calls `t1/t3`
   a deterministic foundation is inaccurate; only t1 is deterministic.
3. The requested baseline was “FAIL exactly task1 and I-3.” Actual verify output was task1
   FAILED, I-3 PASS 10/10, plus a second failure from `ci-parity: cargo test --all`, which
   re-runs the intentional M-96 RED test. `verify_M-96.sh:36-44` makes I-3 host-dependent and
   `:49-145` mandates the full CI rerun. Correct the claimed baseline topology.
4. §6 forbidden changes and the engine-dev area are otherwise executable: task 1 can remain in
   `seek_back_from_tail` plus its description, and M-89 retains the second
   `resolve_active_start_offset` validation. No new T-contract or trait signature is required.

## Done Block

```text
$ git rev-parse origin/feat/M-96-tail-seek-window
a8d0751b2deb267b5736b885b039eab43d740259
exit=0

$ git merge-base origin/main HEAD
ad20a7e7b844bf3882d846ebb66d3b049753c74b
exit=0

$ git diff --name-status origin/main...HEAD
A  crates/journal/tests/red_m96_tail_seek_window.rs
M  docs/ROADMAP.md
A  milestones/M-96-tail-seek-window.md
A  scripts/tests/red_verify_M-96_ci_map.sh
A  scripts/verify_M-96.sh
exit=0

$ TMPDIR=/tmp bash scripts/verify_M-96.sh
FAIL  task1: red_m96_tail_seek_window (t1…t4) (exit=101)
PASS  I-2: journal red_m89_seek_contract (j1…j9)
PASS  I-2: journal red_m89_seek_junction
PASS  I-2: journal red_m89_seek_fallback_observed
PASS  task1 / I-3: red_m89_warm_resume_seek w1 — 10 из 10 прогонов зелёные
PASS  ci-map: проба карты CI-паритета (red_verify_M-96_ci_map.sh; число миров печатает проба)
FAIL  ci-parity: cargo test --all (exit=101)
PASS  ci-parity: учтено шагов 61 из 61 (исполнено 54, исключено по карте 6)
SKIP  task2: §8-замер на проде — reviewer
VERDICT: FAIL (провалов: 2)
exit=1

$ TMPDIR=/tmp bash scripts/tests/red_verify_M-96_ci_map.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
PASS  строка дописана в блок базы ⇒ FAIL (exit=1)
PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
PASS  (i) check_review_fa + дописка ⇒ исполняется, не SKIP (exit=0)
PASS  (ii) run: |- — литеральный блок, исполняется (exit=0)
PASS  (iii) run: > — складывающий скаляр ⇒ FAIL (exit=1)
VERDICT: PASS — 10 сценариев
exit=0

$ cargo test -q -p journal --test red_m96_tail_seek_window  # temporary legal 32 KiB sample
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ cargo test -q -p journal --test red_m96_tail_seek_window  # temporary illegal 4 KiB-per-side mutant
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ cargo test -q -p journal --test red_m96_tail_seek_window  # temporary illegal 64 KiB-total mutant
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ cargo test -q -p journal --test red_m96_tail_seek_window t4_overshoot_beyond_window_falls_back_to_bisection_not_full_scan -- --exact
M-96: ... read 7352972 B at tail 1120000 B, over bisection allowance 1048576 B
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out
exit=101

$ cargo test -q -p journal --test red_m96_tail_seek_window t2_tail_frames_shorter_undershoot_is_found_by_fast_path -- --exact
t2: JR-I-2: expected seq=100000, got 99999
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out
exit=101

$ git status --porcelain  # after every temporary mutant was restored
(empty)
exit=0
```

## Handoff target

architect — revise the committed M-96 spec/RED/verify artifacts on
`feat/M-96-tail-seek-window`, then request critic round 2 against the updated origin head.
