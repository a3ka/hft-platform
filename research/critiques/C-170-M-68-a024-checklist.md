<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: a0d19b8e632e6e6a289c8d1f8cf7693d2acf39c7
audited_head: 1971baac46befd0ddbfa72b1f244a2c79660c7c7
verdict: REJECT
-->

# C-170 — M-68 A-024 checklist: REJECT

**Scope.** The single critic round mandated by `A-024` §8, restricted to O-1…O-8. This is
not a new plan-time finding class. The affected gateway invariant is **VB-I-2** (`live ==
replay`, `docs/fa/viz-backend.md` §5): both defects in `d12`/`d16` leave its new cadence
case without a valid oracle.

## Verdict — REJECT

Three prescribed items are not executed by their committed RED artifacts. Dev dispatch is
blocked. Per `A-024` §8.4–§8.5, this is the terminal permitted critic round: return the
subject to the arbiter rather than creating a ninth plan-time round.

### O-3 — REJECT: `d12` does not distinguish close from first-in-interval

`red_depth_cadence.rs:107-109` creates exactly one bid and one ask level for every event,
each with the invariant size `5.0`; both prices remain within the sole 0.1% band around a
65,000 mid. Therefore the depth sum is the same at the first and final event of every
interval. The claimed value assertion at `:159-190` consequently accepts first-event and
close semantics alike. It also flattens both sides before comparison (`:164-176`), so it
does not preserve the row identity it claims to check.

Required correction: make the fixture's in-band depth value change within each cadence
interval and compare each side/band's coarse close to that interval's final fine value.

### O-4 — REJECT: `d16` proves neither checkpoint consumption nor replay of a tail

`journal()` has already appended and flushed all 120 events before `checkpoint::advance`
(`red_depth_cadence.rs:99-115,344-345`). `advance` is explicitly `advance_to(...,
Cursor::LATEST)` (`crates/gateway/src/lib.rs:2628-2640`), and the test appends no event after
that checkpoint before calling `snapshot_from_checkpoint(..., Cursor::LATEST)` (`:347-356`).
Its `_stats` is discarded. Thus an empty tail and a silent fallback full replay both satisfy
the equality at `:358-364`; the unfinished cadence state can never be exercised.

Required correction: checkpoint at a cursor inside a cadence interval, append a
distinguishing suffix that closes/continues that interval, assert a checkpoint-read witness
(for example bounded decoded-tail stats or a controlled checkpoint perturbation), then
compare warm and full replay. The stipulated dev mutation can then demonstrate that `d16`
goes red; it cannot be deferred to compensate for this missing setup.

### O-6 — REJECT: the asserted extension to `timeframe_ms` is absent

The milestone claims task 17 and `d14` reject sub-second values for **both**
`Selector::depth_cadence_ms` and `timeframe_ms` (`milestones/M-68-depth-from-book.md:147-151`).
But `d14` fixes `timeframe_ms: 1_000` in its selector (`red_depth_cadence.rs:87-96`) and
varies only `depth_cadence_ms` (`:266-273`). Existing validation accepts `timeframe_ms=100`
and `250`, because it rejects only non-positive values or values that do not divide a day
(`crates/gateway/src/lib.rs:2026-2037`; `86_400_000 % 100 == 0`, `% 250 == 0`). The silent
sub-second timeframe collapse therefore remains unpinned.

Required correction: add the missing RED cases for sub-second `timeframe_ms` and specify the
fail-closed validation/representation boundary consistently with the stated two-parameter
requirement.

## Checklist results

| Item | Result | Evidence |
|---|---|---|
| O-1 | PASS | Both T2 fields are declared; 43 `depth_cadence_ms: None` initializers are present and `cargo check --workspace --all-targets` exits 0. The only source diff is the two named token sites plus `gateway/src/lib.rs`. |
| O-2 | PASS | Milestone §3 names exactly `ReadStats::depth_levels_visited`, `Selector::depth_cadence_ms`, and `SeriesBundle::cadence_ms`, with ownership. |
| O-3 | REJECT | Constant-value fixture cannot discriminate close; see finding above. |
| O-4 | REJECT | No post-checkpoint tail/read witness; see finding above. |
| O-5 | PASS | `C4` reports the required three fail-closed false-self-description failures, using counts rather than a silent `grep` exit. |
| O-6 | REJECT | `d14` never varies `timeframe_ms`; see finding above. |
| O-7 | PASS | `milestones/BACKLOG.md:46-77` records the Bookmap subject, non-representability measurement, form, P-020 ×4 cost, and M-71/M-68 prerequisites. |
| O-8 | PASS | `44d6aac` is an ancestor of the audited head; the new O-1/O-2…O-8 work is separate and the milestone states forward-only atomic work. |

## Done Block

```text
$ git status --porcelain

$ bash scripts/reserve_artifact_id.sh C
C-170
reserve_exit=0

$ cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.61s
cargo_check_exit=0

$ git diff --numstat a0d19b8..1971baa -- ':(glob)crates/*/src/**'
1       0       crates/gateway-serve/src/lib.rs
1       0       crates/gateway/src/bin/gateway-checkpoint.rs
14      0       crates/gateway/src/lib.rs

$ bash scripts/verify_M-68.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
PASS: A состав набора — 9 оракулов (ожидалось ровно 9: d1 d2 d3 d4 d5 d7 d7b d8 d8b)
PASS: B набор КРАСЕН против мутанта C-M68-1 (мутация внесена и прогнана в копии)
FAIL: cargo test -p gateway --test red_depth_semantics --quiet
FAIL: cargo test -p gateway --test red_depth_cadence --quiet
FAIL: C4 комментарий обещает переиспользование уровней heatmap (1 упом.), а recompute_depth_from_book материализует книгу сам (2 вызовов self.book.levels)
FAIL: C4 ложное самоописание ЖИВО (1 упом.) — снятая snapshot-only семантика поля depth_reach_bid (lib.rs:636-658)
FAIL: C4 ложное самоописание ЖИВО (1 упом.) — то же, вторая половина того же комментария
FAIL: C4 ложное самоописание ЖИВО (1 упом.) — ложное «как прежний depth_within с None mid» (lib.rs:1134-1136)
PASS: cargo test -p gateway --test red_gateway_schema_version --quiet
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
PASS: cargo test -p gateway --test red_depth_provenance_by_reach --quiet
PASS: H crates/contracts не тронут
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут
PASS: J selector_fingerprint не переписан
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: FAIL (7)
verify_exit=1

$ printf '86400000 mod 100 = '; echo $((86400000 % 100)); printf '86400000 mod 250 = '; echo $((86400000 % 250))
86400000 mod 100 = 0
86400000 mod 250 = 0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-26T22:30Z
- Milestone: M-68-depth-from-book
- Статус: BLOCKED — terminal REJECT in the sole A-024 critic round
- HEAD: 1971baa — docs(M-68): O-2…O-8 — предписания арбитра исполнены; базовая линия rev8 FAIL(7) [architect]

## §B — Что я сделал
- Audited committed artifacts `a0d19b8..1971baa` strictly against A-024 O-1…O-8.
- Reproduced the declared baseline: `verify_M-68.sh` ends `VERDICT: FAIL (7)`, exit 1.

## §C — Артефакты / результаты
- `research/critiques/C-170-M-68-a024-checklist.md`
- Done Block: `cargo check --workspace --all-targets` exit 0; `verify_M-68.sh` exit 1 (the stipulated RED baseline).

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter`
- **Paste-ready промпт:**
  ```
  A-024 §8.4–§8.5 requires automatic return to arbitration: M-68 has received its sole permitted checklist critic round, C-170-M-68-a024-checklist REJECT. Read A-024 §7–§8, C-170, and audited head 1971baa. Decide the route for the three failed prescribed items only: O-3 d12 cannot discriminate close because its fixture's in-band depth is constant; O-4 d16 has neither a post-checkpoint tail nor a read witness; O-6 d14 never varies timeframe_ms although task 17 says both parameters reject sub-second values. No ninth critic plan-time round may be created.
  ```
- Push-статус: pending — this verdict must be committed and pushed to `origin/feat/M-68-rev4` before handoff.
- Кэш: pending removal after commit/push.

## §E — Риски / открытые вопросы
- `A-024` §8.5 forbids another normal plan-time loop; the arbiter determines the route.
- No new plan-time finding class was opened.

=== END HANDOFF ===
