<!-- GATE-META
milestone: M-96
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: 18d3c97ba63a575ad956fa3e200a82734076eb24
verdict: REJECT
-->

# C-294 — M-96 tail seek window, round 2

## Verdict: REJECT

The C-291 numeric mutants now distinguish the two directions and the named cost
ceiling on this host.  The artifact set remains unfit to dispatch: its `t5`/`t6`
setup guard is not the production estimator, and the branch advanced with a
path/role violation after the stated handoff head.

Live invariant for the affected module: **JR-I-2** (`docs/fa/journal.md`) — `seq`
is strictly monotonic with no gaps; the first event emitted after a cursor is
exactly `after + 1`.  All six M-96 worlds assert that exact range.

## Blocking findings

### B-1 — `t5`/`t6` do not guard the displacement that production computes

The tests describe `estimate_error` as the error “as in code”, but it calculates
`floor(file_len / N)` (`crates/journal/tests/red_m96_tail_seek_window.rs:228-238`).
`seek_back_from_tail` uses `floor((file_len - header_end) / N)`
(`crates/journal/src/segments.rs:3654-3665`).  Those integer quotients can differ
by one; with `TAIL_MID = 1,000`, the supposedly guarded displacement can therefore
differ by 1,000 B.

That is material to a 4 KiB/8 KiB discriminator.  In particular, a `t6` setup
accepted by the test guard at -4,864 B can be -3,864 B by the implementation’s
arithmetic, allowing the prohibited 4 KiB-forward variant to pass.  The current
host happens to give ±6,000 B for both formulae, but that is evidence about this
host, not a fail-closed setup guard against the varint-time mode the milestone
explicitly says is host-dependent.

**Condition for next review:** make the setup guard derive and assert the target
displacement using the exact production arithmetic (including `header_end` and
the same floors), then re-present the two 4 KiB mutations and both legal controls.

### B-2 — current branch tip adds an architect commit outside M-96’s allowed paths

After the supplied handoff head `481c646e`, the subject branch advanced to
`18d3c97b` with `docs(adversary): C-290 — TD-250 w1 [architect]`, adding
`research/critiques/C-290-td250-w1-adversary.md`.  M-96 §10 permits architect
only the milestone, M-96 RED test, verify script, CI-map probe, and the listed
`docs/ROADMAP.md` line; it declares everything else out of scope.  `research/`
verdicts are the critic/adversary write zone, not architect’s (`scope-guard.md`).

The transferred C-290 evidence is relevant, but relevance does not create a
scope exception.  The subject must be made compliant by its proper owner or a
documented, authorized exception before it is presented again.

## Notes

1. The requested seam results were reproduced after the temporary code change
   was fully reverted: 16 KiB back / 32 KiB wide and 8 KiB back / 16 KiB wide
   both passed 6/6; 4 KiB back failed only t5; 4 KiB forward (28 KiB back in a
   32 KiB window) failed only t6; 32 KiB back / 64 KiB total failed five worlds
   on the 172 KiB `rchar` bound.  So the present-host mutation evidence is real.
2. The 172 KiB replacement is honest only as an **I/O-cost** requirement.  It
   does not bound configured forward width or allocation when the read reaches
   EOF, exactly as §3 acknowledges.  It is not a reinstatement of the former
   “window ≤32 KiB” rule; the milestone should not describe it as one.
3. §10 now correctly permits the `docs/ROADMAP.md` TD-250 line.  `04-workflow.md`
   §2 assigns roadmap ordering to architect, so C-291 B-2 is resolved.  The new
   C-290 transfer is a separate, later scope issue.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-96-tail-seek-window
18d3c97ba63a575ad956fa3e200a82734076eb24
exit=0

$ git merge-base origin/main origin/feat/M-96-tail-seek-window
ad20a7e7b844bf3882d846ebb66d3b049753c74b
exit=0

$ git diff --name-status 481c646e..18d3c97b
A  research/critiques/C-290-td250-w1-adversary.md
exit=0

$ C96_BACK_BYTES=16384 C96_WINDOW_BYTES=32768 cargo test -q -p journal --test red_m96_tail_seek_window
test result: ok. 6 passed; 0 failed; finished in 39.50s
exit=0

$ C96_BACK_BYTES=8192 C96_WINDOW_BYTES=16384 cargo test -q -p journal --test red_m96_tail_seek_window
test result: ok. 6 passed; 0 failed; finished in 38.81s
exit=0

$ C96_BACK_BYTES=4096 C96_WINDOW_BYTES=32768 cargo test -q -p journal --test red_m96_tail_seek_window
t5_overshoot_between_4_and_8_kib_needs_8_kib_back --- FAILED
test result: FAILED. 5 passed; 1 failed; finished in 37.97s
exit=101

$ C96_BACK_BYTES=28672 C96_WINDOW_BYTES=32768 cargo test -q -p journal --test red_m96_tail_seek_window
t6_undershoot_between_4_and_8_kib_needs_8_kib_forward --- FAILED
test result: FAILED. 5 passed; 1 failed; finished in 37.05s
exit=101

$ C96_BACK_BYTES=32768 C96_WINDOW_BYTES=65536 cargo test -q -p journal --test red_m96_tail_seek_window
t1,t2,t3,t5,t6 --- FAILED (rchar exceeds tail + 176128 B)
test result: FAILED. 1 passed; 5 failed; finished in 38.38s
exit=101

$ C96_PROBE=1 C96_BACK_BYTES=16384 C96_WINDOW_BYTES=32768 cargo test -q -p journal --test red_m96_tail_seek_window t5_overshoot_between_4_and_8_kib_needs_8_kib_back -- --exact --nocapture
C96_PROBE approx_minus_target=6000 back=16384 width=32768
test result: ok. 1 passed; 0 failed; finished in 31.50s
exit=0

$ C96_PROBE=1 C96_BACK_BYTES=16384 C96_WINDOW_BYTES=32768 cargo test -q -p journal --test red_m96_tail_seek_window t6_undershoot_between_4_and_8_kib_needs_8_kib_forward -- --exact --nocapture
C96_PROBE approx_minus_target=-6000 back=16384 width=32768
test result: ok. 1 passed; 0 failed; finished in 30.69s
exit=0

$ git -C /tmp/hft-critic-m96r2-mut status --porcelain  # after seam rollback
(empty)
exit=0

$ TMPDIR=/tmp bash scripts/verify_M-96.sh
started in background on the handed-off tree; still running at verdict write, so no exit code is claimed.
```

## Handoff target

architect — resolve B-1 and B-2 on `feat/M-96-tail-seek-window`, commit the
replacement artifact set, and request a new critic round against the then-current
origin head.
