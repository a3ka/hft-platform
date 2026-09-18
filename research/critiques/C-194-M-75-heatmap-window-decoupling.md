<!-- GATE-META
milestone: M-75
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: c3ee54b6510e5ee4bef23cf16bb747f962492770
verdict: REJECT
-->

# C-194 — M-75 heatmap-window-decoupling: REJECT

## Scope and audited artifact set

`origin/main..c3ee54b6510e5ee4bef23cf16bb747f962492770` is one architect
commit and contains exactly the milestone, `red_heatmap_window_decoupled.rs`,
and `verify_M-75.sh`.  The range does not touch `crates/contracts/**`,
`docs/rfc/**`, `GATEWAY_SCHEMA_VERSION`, `GATEWAY_BANDS`, or either signed
decision.  `П-014` and `П-020` therefore remain unopened as decisions.

The written H-1/H-3/H-4 fixture is non-vacuous: its narrow map is nonempty
(H-4 passes), while the canonical selector is rejected at 7,882,335 bytes
against the 2,000,000-byte cap (H-1/H-3 fail).  This is relevant to
**VB-I-10**: the map remains a bounded-window snapshot rather than being
"fixed" by an empty window.  The gateway crate has no dedicated FA; this
verdict relies on `docs/fa/viz-backend.md` and records that declared FA gap.

## Blocking findings

### B-1 — H-2 is absent, not sanctioned COMPILE-RED

`M-75` names H-2 as the fail-closed oracle for invalid
`GATEWAY_HEATMAP_WINDOW`, but no
`crates/gateway-serve/tests/red_heatmap_window_env.rs` exists
(`test -f` exits 1).  Neither `DEFAULT_HEATMAP_WINDOW_FRAC` nor either
declared effective-window function exists in the audited code.

The task explicitly labels this as a self-declared exception.  It is not
permitted: A-028 §1 makes the pre-dispatch set complete only when **every
named oracle exists as committed text**.  COMPILE-RED relaxes compilability,
never existence.  The acceptance script detects this accurately as a vacuum,
but detecting absence after the fact does not turn absence into a complete
architect artifact set.  The critic profile requires an immediate incomplete-
artifacts verdict in exactly this condition.

Before any dev dispatch, architect must commit H-2 against the literal §5
signature in its explicit COMPILE-RED state and make the milestone/verify
inventory name that committed test target.  It must cover both malformed and
out-of-range values failing startup, rather than a fallback default.

### B-2 — H-1/H-3 prove neither server-side ownership nor full decoupling

The asserted pair is only `bands=[0.001]` and `bands=[...0.60]`.  A mutant
caller can still derive the window from the selector, for example by clamping
`max(selector.bands)` to `0.001`, then pass that value into the narrowed
`build_heatmap_and_cob` signature.  It passes H-1, H-3, and H-4: both present
selectors produce the same nonempty 0.001 map and remain under the cap.  It
also passes the structural gate: the function body no longer contains
`selector.bands`, and unused declarations of the constant/effective getter
satisfy their grep checks.  Yet a selector below 0.001 still controls map
width and the server-side setting has no behaviour.

The depth-series positive control correctly establishes that the two current
selectors differ, but does not distinguish this clamped coupling from a
server-owned window.  This fails the claimed objective and leaves the
`PL-I-5` resource-amplification property dependent on an unproved call-site.
Architect needs a RED oracle that fails this mutant: it must observe that a
below-config `bands` value cannot shrink heatmap/COB and that changing the
declared effective server setting, rather than `Selector.bands`, changes their
window.  The acceptance canary must pin the call-site/property that supplies
that effective value, not only the callee body.

## Checks that passed or are not findings

- The dense fixture fixes the former narrow-window degeneration: H-4 passed
  under the current implementation.
- The `awk` canary's setup guard reacted correctly to a rename simulation:
  renaming `build_heatmap_and_cob` left one extracted line and produced the
  intended guard failure.  This closes the C-192 B-3 rename/vacuum case, but
  not B-2's upstream-source hole.
- `verify_design_claims.sh --merge-preview origin/main` passed, exit 0.
- The range boundary checks passed; no contract RFC, schema bump, or change to
  either already-signed decision is proposed or required by this verdict.

## Required disposition

**REJECT.** Do not dispatch any M-75 dev task until B-1 and B-2 have committed
RED artifacts.  The next critic round audits those artifacts plus any finding
of a different class.  This is the first REJECT on this subject/reason.

## Done Block

```text
$ git rev-parse origin/main; git rev-parse HEAD; git log --oneline origin/main..HEAD
d77398d7b22396c452d2651e90498033186055dd
c3ee54b6510e5ee4bef23cf16bb747f962492770
c3ee54b spec(M-75): расцепление окна heatmap от полос — набор architect'а закоммичен ПОЛНОСТЬЮ [architect]

$ git diff --name-only origin/main..HEAD
crates/gateway/tests/red_heatmap_window_decoupled.rs
milestones/M-75-heatmap-window-decoupling.md
scripts/verify_M-75.sh

$ cargo test -p gateway --test red_heatmap_window_decoupled
running 3 tests
test hw_i_4_decoupling_does_not_empty_the_heatmap ... ok
test hw_i_1_heatmap_size_is_independent_of_bands ... FAILED
test hw_i_3_canonical_bands_fit_under_signed_cap ... FAILED
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ bash scripts/verify_M-75.sh; echo exit=$?
FAIL: cargo test --all --quiet
FAIL: оракул расцепления (H-1 · H-3 · H-4) (исполнено тестов: 3, exit=101)
FAIL: оракул fail-closed разбора GATEWAY_HEATMAP_WINDOW — НИ ОДИН тест не исполнился: фильтр не нашёл оракула. Зелёное здесь означало бы ВАКУУМ
VERDICT: FAIL (9)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ cargo test --all 2>&1 | tail -3; echo exit=${PIPESTATUS[0]}
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
error: test failed, to rerun pass `-p gateway --test red_heatmap_window_decoupled`
exit=101

$ grep -n 'selector.bands' crates/gateway/src/lib.rs
1187:    /// `selector.bands`, что невозможно в `Reducer` без чекпоинт-инвалидации — `selector`
1194:            for &band in &self.selector.bands {
1259:        let bands = &self.selector.bands;
1337:        let bands = &self.selector.bands;
1557:    let w = selector.bands.iter().copied().fold(0.0_f64, f64::max);

$ grep -n 'build_heatmap_and_cob' crates/gateway/src/lib.rs
1384:    /// читаются буквально по ссылке через `build_heatmap_and_cob`/`build_volume_bubbles`/
1483:        let (heatmap, cob) = build_heatmap_and_cob(&self.selector, &self.heatmap_buckets);
1553:fn build_heatmap_and_cob(

$ test -f crates/gateway-serve/tests/red_heatmap_window_env.rs; echo exit=$?
exit=1

$ rename simulation for verify_M-75.sh body guard
1
guard=FAIL as intended
```
