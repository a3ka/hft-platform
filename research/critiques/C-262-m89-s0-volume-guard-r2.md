<!-- GATE-META
milestone: M-89
audited_repo: a3ka/hft-platform
audited_base: dcb435f80109030ab6efb3157f86df2f53b609d9
audited_head: c657628a82b734911ef746e2560ee120e01328cc
verdict: REJECT
-->

# C-262 — M-89 S0: volume guard and serving observability, round 2

## Verdict

**REJECT.** Round 2 closes the five findings in C-260 at the plan/RED-gate
level: the heartbeat has three states; the public contract is now
`stream_from_at`; path composition is exercised as a file path; reader bytes
are tied to a replica; and CI parity has an executable two-way map.  The
complete RED, verify, milestone, measurement, and signature artifact set is
present.  The pre-implementation baseline is correctly red and its guards
are green.

One new RAW journal-read finding remains.  The new `f1` oracle constructs a
valid-CRC sequence gap exactly at `after+1`, observes a fallback, and then
blesses delivery of `after+2`.  That permits a seek implementation to fall
back honestly yet silently skip the missing event.  It violates the live
**JR-I-2** invariant: a read-time sequence gap must abort, not be skipped.
Do not begin implementation from this set.

## Audited subject

- Subject: `origin/feat/M-89-s0-volume-guard-observability`.
- Mandated SHA and audited head:
  `c657628a82b734911ef746e2560ee120e01328cc`.
- Base: `dcb435f80109030ab6efb3157f86df2f53b609d9`.
- The second fetch immediately before writing found no drift; `c657628` is an
  ancestor of the fetched tip (both checks exit 0).
- This is a RAW gate: tasks 1--2 alter the journal read path in
  `crates/journal/src/segments.rs`.  Live invariants: **JR-I-2** (strictly
  monotonic, gap-free sequence; gap while reading aborts) and **JR-I-11**
  (no public catalog-stitching read path silently stitches a non-monotonic
  catalog).

The branch contains the M-89 milestone and measurement basis, the three new
journal RED binaries, amended warm-resume/read-volume/structural/heartbeat/
serial/ops REDs, `verify_M-89.sh`, both verifier probes, and the DESIGN §22
7-to-8 claim.  `crates/contracts/**` and production trait/API definitions
have no delivered diff: the additive interfaces are intentionally declared in
the milestone and compile-REDs.  This is appropriate for the asserted RED
baseline, but makes the oracle semantics the gate's binding contract.

## C-260 closure review

These are not repeat rejections.

- **R1:** the milestone and `g1..g5` now name `Disabled`,
  `ConfiguredMissing`, and `Present`; `g4b` executes legacy-wrapper
  equivalence and `g5` exercises the configured-but-missing file path.
- **R2:** task 1 forbids public `locate_after_seq`, and `j1..j9` directly
  call public `stream_from_at(.., Some(after), None)`.  The old public-stub
  mutant is no longer applicable.
- **R3/R4:** `h0`/task 11 reject `/state/`, a mount root, and a directory
  cron path; `h1` uses the compose path.  `b1..b7` count at `EventStream` and
  `v` requires equality to the replica delta, so `events_scanned * 64` and a
  dead reader counter cannot satisfy the stated oracle.
- **R5:** the `CI_MAP` table covers all CI `run:` entries in both directions;
  the c1--c5 probe tests added/stale runs and waiver predicates execute.
- **N1--N4:** `d7c`, `j8`, legacy `s2`, comment/type/use scanner mutations
  t1--t10, and per-function SERIAL mutations m1--m6 are present.  The two
  verifier probes pass (8/8 and 24/24).

The DESIGN §22 increment is in the §9 gate zone and was checked on the merge
preview.  Selected plan code claims were also opened at their cited
`segments.rs` locations; no separate unsupported claim was found.  The
exception is R6 below, where that source check exposes the semantic hole.

## Blocking finding

### R6 — fallback turns a valid sequence gap into silent partial delivery

**Evidence.** [JR-I-2](../../docs/fa/journal.md#L112) requires a strict,
gap-free `seq` and says that a gap on read must abort.  M-89 itself says that
the public `stream_from_at(.., Some(after), None)` contract starts **exactly**
at `after+1` and that a gap is `Err`, not a skip
([M-89 I-2](../../milestones/M-89-s0-volume-guard-observability.md#L86)).
Section 5.2 correctly rejects a seek candidate whose sequence is not
`after+1`, increments `seek_fallbacks`, and reopens from `header_end`
([§5.2](../../milestones/M-89-s0-volume-guard-observability.md#L250)).

But `f1` writes `0..2999`, appends valid framed events `3001..3200`, and uses
`after = 2999`: event 3000 (`after+1`) is absent
([f1 fixture](../../crates/journal/tests/red_m89_seek_fallback_observed.rs#L108)).
It then asks the independent reader for the desired result and explicitly
accepts its first event as 3001 with no error
([lines 127--146](../../crates/journal/tests/red_m89_seek_fallback_observed.rs#L127)).
The asserted outcome is merely fallback `== 1`, rescanning from the header,
and equality to that permissive independent result.

The current production iterator demonstrates the surviving mutant: after a
fallback, it increments `events_scanned`, filters only `ev.seq <= after`, and
returns every larger event; it retains neither an expected-sequence value nor
a `seq == previous + 1` guard
([`next`](../../crates/journal/src/segments.rs#L1744)).  A new private search
can therefore reject candidate 3001, increment `seek_fallbacks`, perform a
real full rescan (satisfying f1's counter checks), and the ordinary iterator
will emit 3001.  It passes j1--j9/f1--f5 as specified while silently dropping
3000.  The absence is valid CRC, so corruption tests do not cover it.

**Required correction.** Add a RED oracle using this valid-frame gap which
requires the public `stream_from_at` path to return a read error (and to make
no post-gap event observable) rather than emit 3001.  Its independent oracle
must enforce the same JR-I-2 contract, not define the skip as the reference.
If the intended contract is instead to tolerate gaps, that needs an explicit
FA/spec authority change before resubmission; it cannot be introduced by
fallback behavior.  Re-run the complete verify baseline and resubmit the new
commit chain.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-89-s0-volume-guard-observability
c657628a82b734911ef746e2560ee120e01328cc
$ git merge-base origin/main c657628a82b734911ef746e2560ee120e01328cc
dcb435f80109030ab6efb3157f86df2f53b609d9
$ git merge-base --is-ancestor c657628 origin/feat/M-89-s0-volume-guard-observability
exit=0

$ git fetch origin  # immediately before verdict write
audited_start=c657628a82b734911ef746e2560ee120e01328cc
head_after_fetch=c657628a82b734911ef746e2560ee120e01328cc
base_after_fetch=dcb435f80109030ab6efb3157f86df2f53b609d9
drift=none
mandate_c657628_is_ancestor=yes
exit=0

$ git diff --check dcb435f..c657628
diff_check_exit=0

$ bash scripts/tests/red_verify_M-89_task_status.sh
итого ok=8 fail=0
VERDICT: PASS
task_status_exit=0

$ bash scripts/tests/red_verify_M-89_scan.sh
итого ok=24 fail=0
VERDICT: PASS
scan_exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [7-RFC-PATH] путей-кандидатов ... все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
verify_design_claims_exit=0

$ bash scripts/verify_M-89.sh                 # complete red-baseline run
PASS  task1: публичного locate_after_seq в crates/journal/src/segments.rs нет ...
FAIL  task1: у EventStream нет payload_bytes_read / seek_fallbacks ...
FAIL  task1/договор: red_m89_seek_contract ... КРАСЕН
FAIL  task1/откат: red_m89_seek_fallback_observed ... КРАСЕН
FAIL  task1/байты: red_m89_bytes_accounting ... КРАСЕН
... sacred journal/gateway corpus and CI-parity executed to completion ...
exit=1  # required red baseline before tasks 1--6, 8--11 are implemented

$ cargo test -q -p journal --test red_m89_seek_fallback_observed
error[E0599]: no method named `seek_fallbacks` found for struct `EventStream` in the current scope
error: could not compile `journal` (test "red_m89_seek_fallback_observed") due to 1 previous error
seek_fallback_observed_exit=101

$ nl -ba crates/journal/tests/red_m89_seek_fallback_observed.rs | sed -n '108,146p'
108  /// f1 — кандидат с неверным seq. ... PREFIX пропущен ...
113      const PREFIX: u64 = 3_000;
116      write(dir.path(), 0, PREFIX, BIG_SEG);
118      for seq in PREFIX + 1..PREFIX + 1 + TAIL {
127      let after = PREFIX - 1;
128      let (want, want_err) = independent(dir.path(), after);
129      if want.first() != Some(&(PREFIX + 1)) && want_err.is_none() {
132      let (got, err, scanned, fallbacks) = run(dir.path(), after);
133      assert_eq!(fallbacks, 1, ...);
143      assert_eq!((got, err), (want, want_err), ...);
exit=0

$ nl -ba crates/journal/src/segments.rs | sed -n '1744,1796p'
1745  Ok(Some(ev)) => {
1753      self.events_scanned += 1;
1790      if let Some(after) = self.after_seq {
1791          if ev.seq <= after {
1792              continue;
1795      self.events_decoded += 1;
1796      return Some(Ok(ev));
exit=0
```

## Handoff

**Status:** BLOCKED — architect must close R6 in a new committed artifact
chain before this RAW gate can be reconsidered.

**Next role:** architect.

**Required input:** corrected milestone/FA-aligned RED oracle and verify
artifact, committed on `feat/M-89-s0-volume-guard-observability`, followed by
a fresh commit-chain reference and milestone path for another critic round.
