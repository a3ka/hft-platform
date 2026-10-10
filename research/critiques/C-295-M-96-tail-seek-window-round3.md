<!-- GATE-META
milestone: M-96
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: 7ec51988f1ffd88536edca39923d96b0419fde59
verdict: ESCALATE
-->

# C-295 — M-96 tail seek window, round 3

## Verdict: ESCALATE (→ approved)

This is the third round of the same M-96 thread (`C-291`, `C-294`, this
verdict), so the route escalation in `gates.md` §0(2) is recorded, following
the routing precedent in `A-051` §1–§3.  The narrowed C-294 scope has no
remaining technical REJECT: the escalation is resolved toward approval, not a
new implementation defect.

Architect must make the mechanical C-295 appendix in M-96 §14, without
changing the normative artifact set, and then dispatch task 1 to `engine-dev`.
The appendix is mechanical; it does not call for a fourth critic round.

Live invariant for the affected module: **JR-I-2** (`docs/fa/journal.md`) —
the sequence is strictly monotonic with no gaps, so the stream emits exactly
`after + 1 ..= last`.  All six worlds retain that assertion and
`seek_fallbacks == 0` on a whole journal.

## C-294 B-1 — resolved: the setup estimator now reproduces production arithmetic

`estimate_error` now gets `header_end` from the fixture's actual v2 header and
uses the same arithmetic as `seek_back_from_tail`:

```text
total_events = last_seq - first_seq + 1 = PREFIX + tail
avg           = floor((file_len - header_end) / total_events)
approx_offset = min(events_back * avg, file_len - header_end)
approx_pos    = max(file_len - approx_offset, header_end)
```

For this fixture `first_seq = 0`, `last_seq = PREFIX + tail - 1`, and
`events_back = tail`; its target is the start of the appended tail,
`file_len - tail_bytes`.  Thus the guard covers the same `header_end`, both
integer floors, `min`, and `max` as production rather than the former
`floor(file_len / N)` surrogate.

The required temporary seam was applied only in this detached critic worktree
and fully reverted before this artifact was written.  It produced the required
separation: 4 KiB backward failed only `t5`; 4 KiB forward failed only `t6`;
both legal controls passed all six worlds.

## C-294 B-2 — resolved: the exception is exact and its basis is reachable

M-96 §10 names one and only one additional architect path:
`research/critiques/C-290-td250-w1-adversary.md`.  It names the founder-approved
2026-10-09 transfer, the source terminal branch
`harness/td250-w1-difference`, transfer commit `18d3c97b`, and the
`TERMINAL-BRANCH-VERDICT` trailer.  The exception prohibits changing the
adversary verdict's content.

The C-290 blob at transfer commit `18d3c97b` is byte-identical to the source
branch blob at `d9191bb7`; the source remains reachable from `origin`.  The
exception therefore preserves the M-96 evidentiary basis without reopening
C-291 or treating relevance as an implicit scope exception.

## Artifact-set and scope check

- The complete M-96 architect set is present: milestone, RED oracle,
  `verify_M-96.sh`, and CI-map probe.  M-96 introduces no T1/T2 contract or
  trait-signature change; it specifies the existing public
  `journal::stream_from_at` path and the implementation scope is confined to
  `seek_back_from_tail`.
- `b35f04d1..7ec51988` changes only the sacred M-96 RED oracle and the
  milestone, both architect-owned paths.  The B-2 transferred verdict was
  already present before this narrow C-294 response range and is now covered by
  the explicit §10 exception.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-96-tail-seek-window
7ec51988f1ffd88536edca39923d96b0419fde59
exit=0

$ git merge-base origin/main 7ec51988f1ffd88536edca39923d96b0419fde59
ad20a7e7b844bf3882d846ebb66d3b049753c74b
exit=0

$ git log --oneline b35f04d1..7ec51988
7ec51988 docs(M-96): C-294 — §10 исключение для переноса C-290 (согласие founder'а), журнал круга 2 [architect]
bf07c513 test(M-96): C-294 B-1 — страж t5/t6 повторяет арифметику seek_back_from_tail точно (header_end, floor, min/max) [architect]
exit=0

$ git diff --name-status b35f04d1..7ec51988
M	crates/journal/tests/red_m96_tail_seek_window.rs
M	milestones/M-96-tail-seek-window.md
exit=0

$ git diff --no-index --exit-code <(git show d9191bb7:research/critiques/C-290-td250-w1-adversary.md) <(git show 18d3c97b:research/critiques/C-290-td250-w1-adversary.md)
exit=0

$ M96_BACK=4096 M96_W=32768 cargo test -q -p journal --test red_m96_tail_seek_window
t5_overshoot_between_4_and_8_kib_needs_8_kib_back --- FAILED
test result: FAILED. 5 passed; 1 failed; finished in 38.18s
exit=101

$ M96_BACK=28672 M96_W=32768 cargo test -q -p journal --test red_m96_tail_seek_window
t6_undershoot_between_4_and_8_kib_needs_8_kib_forward --- FAILED
test result: FAILED. 5 passed; 1 failed; finished in 38.42s
exit=101

$ M96_BACK=16384 M96_W=32768 cargo test -q -p journal --test red_m96_tail_seek_window
test result: ok. 6 passed; 0 failed; finished in 38.53s
exit=0

$ M96_BACK=8192 M96_W=16384 cargo test -q -p journal --test red_m96_tail_seek_window
test result: ok. 6 passed; 0 failed; finished in 37.77s
exit=0

$ git restore --source=HEAD -- crates/journal/src/segments.rs && git diff --exit-code -- crates/journal/src/segments.rs
exit=0
```

## Route

`ESCALATE(→approved)` — architect records the mechanical C-295 appendix in
M-96 §14, then sends M-96 task 1 to `engine-dev`.  No technical item is left
for a new critic round.
