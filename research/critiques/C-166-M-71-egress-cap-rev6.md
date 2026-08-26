<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: fcbcd4a950edd19b978458dabf28ef849551fb1d
audited_head: a60c9aa5ffb472551c2dfe9c1be7d97dc4e0d2cf
verdict: REJECT
-->

# C-166 — M-71 egress cap rev6: plan-time **REJECT**

**Audited range:** `fcbcd4a..a60c9aa` on `origin/feat/M-71-rev6`.

## Gate result

The committed set is present: milestone, real `verify_M-71.sh`, and three new RED artifacts.
`contracts/`, `GATEWAY_BANDS`, and the forbidden engine paths are absent from the audited
delta. `VB-I-2`, `VB-I-10`, and `VB-I-11` are live invariants in
`docs/fa/viz-backend.md`; the new P1/P2/P3 suite correctly reaches the present live-path
defects instead of the former vacuum.

The set nevertheless cannot enter dev: task 10's COMPILE-RED is structurally unable to become
GREEN under the stated role boundaries, and its runtime oracle bypasses the production
configuration route.

## B-1 — task 10 cannot become GREEN without a forbidden test edit

`crates/gateway-serve/tests/red_egress_cap_governed.rs:233-242` intentionally names the new
`ServeConfig::max_response_bytes` field, so current compilation is RED with exactly `E0560`.
But the same test file already contains another complete `ServeConfig` literal at
`crates/gateway-serve/tests/red_egress_cap_governed.rs:126-135`, which omits that field.

When dev adds the required field, Rust must type-check that existing literal even though
`config()` is currently unused. The present `E0560` will therefore turn into `E0063: missing
field max_response_bytes` at line 127. The dev role may not edit `*/tests/**`; the submitted
artifact has no architect-owned update for that literal. The same mechanical scan finds eleven
`ServeConfig {` carriers in `crates/gateway-serve`, including the new UTF-8 suite and eight
pre-existing test suites. Thus the promised GREEN transition for task 10 is impossible from
this artifact set and `cargo test --all` / clippy all-targets will remain blocked.

**Required condition for a re-review:** the architect must commit a coherent RED artifact set
whose existing sacred test literals can compile after the planned type change, without requiring
the dev to edit tests. Its expected baseline and verify checks must then be refreshed from that
set.

## B-2 — N1 does not prove that the operator's env value reaches `ServeConfig`

The new socket test builds a private configuration directly:

- `red_egress_cap_governed.rs:233-246` constructs `ServeConfig { max_response_bytes: cap }`
  and calls `bind`;
- `red_egress_cap_startup.rs:46-52,123-145` calls `serve_config_from_env`, but observes only
  `Result::is_ok()`, not the field carried by the returned config or behaviour of a bound
  server.

Consequently a future implementation can parse the env value, retain the current atomic write,
return `ServeConfig { max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES, .. }`, and have `bind`
honour its field. N1 passes because it supplies the field directly; all startup tests pass
because parsing still returns `Ok`; an operator's `GATEWAY_MAX_RESPONSE_BYTES=1000` is ignored.
That is R-133 B-1 again, not a distinct failure mode.

**Required condition for a re-review:** a RED oracle must exercise the production route from a
specific valid environment value through `serve_config_from_env`, `bind`, and the socket-visible
outcome, with a positive control for a generous value. It must fail against the configuration
discard mutant above.

## Checked non-blocking facts

- The cited `П-020` is not in `a60c9aa`'s copy of `docs/PENDING-SIGNATURE.md`, but it is already
  on `origin/main` (`bd20428`), and `git merge-tree --write-tree origin/main a60c9aa` succeeds;
  the merge-preview contains the signed entry. This is not the reason for this REJECT.
- `P-C1` and `U-C1` both pass; P1/P2/P3 and U1 fail against today's implementation as intended.
- The requested `PUMP_BATCH=usize::MAX` source mutation was not applied: a critic may not edit
  `crates/**`, including a test fixture. The committed test distinguishes `PumpRefused` from
  `SnapshotRefused`; the reviewer evidence and the source make the stated reachability claim
  inspectable, but a fresh mutation result must be supplied by the architect in the next
  artifact set.

## Done Block

```text
$ git diff --name-status fcbcd4a..a60c9aa
A       crates/gateway-serve/tests/red_egress_cap_governed.rs
A       crates/gateway-serve/tests/red_egress_cap_utf8.rs
A       crates/gateway/tests/red_egress_cap_paths.rs
M       milestones/M-71-egress-cap.md
M       scripts/verify_M-71.sh
exit=0

$ git diff --numstat fcbcd4a..a60c9aa
297     0       crates/gateway-serve/tests/red_egress_cap_governed.rs
315     0       crates/gateway-serve/tests/red_egress_cap_utf8.rs
291     0       crates/gateway/tests/red_egress_cap_paths.rs
88      1       milestones/M-71-egress-cap.md
17      0       scripts/verify_M-71.sh
exit=0

$ cargo test -p gateway --test red_egress_cap_paths --quiet
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ cargo test -p gateway-serve --test red_egress_cap_utf8 --quiet
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ cargo test -p gateway-serve --test red_egress_cap_governed --no-run
error[E0560]: struct `ServeConfig` has no field named `max_response_bytes`
  --> crates/gateway-serve/tests/red_egress_cap_governed.rs:241:9
exit=101

$ cargo test -p gateway --test red_egress_cap_paths pl_i_5_p_c1_honest_load_passes_both_paths_identically --quiet
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out
exit=0

$ cargo test -p gateway-serve --test red_egress_cap_utf8 pl_i_5_u_c1_long_ascii_venue_gets_an_honest_error --quiet
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
exit=0

$ cargo test -p gateway-serve --test <each sibling except red_egress_cap_governed> --no-run --quiet
18 sibling targets compiled; siblings_overall=0
exit=0

$ git merge-tree --write-tree origin/main a60c9aa
ef16d832772752e85f13bde7933e60245224c1b5
exit=0

$ git show ef16d832772752e85f13bde7933e60245224c1b5:docs/PENDING-SIGNATURE.md | rg '^## П-020'
## П-020 — ПОДПИСАНО 2026-08-26: предел объёма ответа gateway — 2 000 000 байт
exit=0

$ bash scripts/reserve_artifact_id.sh --list C | rg '^C-166'
C-166      0 дн  reserve C-166 nous 2026-08-26T18:35:07Z Ubuntu-2404-noble-amd64-base 2895473 f648d347-fd30-4532-adbd-a08890ca6330
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC, ISO-8601): 2026-08-26T19:05Z
- Milestone: M-71-egress-cap, rev6
- Status: BLOCKED — REJECT
- HEAD: a60c9aa — test(PL-I-5): спека rev6 + два оракула по R-133 — дегенерированный вход и «ручка управляет»; гейт расширен [architect]

## §B — What I did
- Audited the committed artifact range, the R-133 review, M-71 milestone, verify script, T2 signatures, RED suites, FA invariants, RFC and merge-preview authority.
- Executed the new RED suites and their positive controls; verified the C-166 CAS reservation.

## §C — Artifacts / results
- `research/critiques/C-166-M-71-egress-cap-rev6.md`
- Done Block: recorded above; REJECT due to B-1 and B-2.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  Rework M-71 rev6 after C-166 REJECT on origin/feat/M-71-rev6. Before dev is dispatched,
  commit an architect-owned RED artifact set that (1) can turn GREEN after adding
  ServeConfig::max_response_bytes without any dev edit to */tests/**, including every affected
  Sacred ServeConfig literal, and (2) proves the actual production configuration route:
  GATEWAY_MAX_RESPONSE_BYTES env -> serve_config_from_env -> returned ServeConfig -> bind ->
  socket-visible response. The oracle must fail if the parsed value is discarded and the config
  receives the default. Refresh the verify baseline and re-submit the committed range for critic.
  ```
- Push status: pending this critic verdict commit to `origin/feat/M-71-rev6`.
- Cache: ⏸ left in `/tmp/hft-critic-m71-r6` while this worktree holds the verdict pending push.

## §E — Risks / open questions
- The requested `usize::MAX` reachability mutation cannot be performed by critic without editing
  `crates/**`; include raw architect-owned mutation output in the corrected artifact set.
- No founder decision is requested: the signed numeric authority is present in the clean
  origin/main merge-preview.

=== END HANDOFF ===
