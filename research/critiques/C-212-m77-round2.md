<!-- GATE-META
milestone: M-77
audited_repo: a3ka/hft-platform
audited_base: 28324874f33704e605198971f6b42d2358afc59e
audited_head: 1b359116a86f0370bde8729a243b70b477ccab3e
verdict: REJECT
-->

# C-212 — M-77 round 2: T7 loses the named delivery-window oracle after GREEN

**Verdict: REJECT.** The three conditions of `C-211` are present and the current RED
phase is correctly localized, but task 5 does not retain the mandated three-outcome
gate once task 3 makes the workspace GREEN. Dev dispatch is blocked.

## Audited scope

This is the narrowed round-2 scope from `M-77` §11, not a re-review of choice B or
the four round-1 tests. The committed range `2832487..1b35911` contains the task-2
contract (§6bis), the delivery-window RED suite, the `pump`-cost suite, the separate
`T7`/`T8` steps, and the milestone. No T1 contract changes are present; §6bis declares
the required crate-private T3 `BookSeriesSlice`/`Reducer::book_series_in` and
`SeriesBundle::set_book_series` signatures before implementation.

`VB-I-2` is the live invariant: a series built on the live tail must be bit-identical
to replay of the same journal window (`docs/fa/viz-backend.md` §5). The scope expansion
is justified: the §2bis measurement shows `heatmap` differs as well as depth, whereas
the whole-`SeriesBundle` oracle avoids the non-enumerated-carrier hole of Р-3. All three
§13 immutable revision targets exist as `commit` objects.

The setup guards are correctly aimed at scenario reachability, not an expected
implementation outcome: W1 establishes cap/refusal/terminality and an unchanged cursor;
W3 establishes refusal and actual multi-frame splitting; the cost oracle establishes
one-vs-many batch arms, nonzero allocation, and negligible wire share. Its 1.35 ceiling
uses total allocations at `pump`, varying batches rather than book size, so it separates
the observed 0.994 current world from 1.96 candidate A without charging the unrelated
existing book-size cost (the declared §9ter debt).

## B-1 — T7 becomes vacuum-green in the GREEN branch

**Where:** `scripts/verify_M-77.sh:213-218`.

**Defect:** The RED branch counts the two named subject tests, but when
`cargo test --all` is green, T7 executes the entire `red_m77_delivery_window` binary and
accepts its exit code alone. Cargo returns 0 when the named W2/W3 tests are ignored or
otherwise execute zero times. This violates the three-outcome requirement of `A-028`
§3 p.5 exactly where the step claims to require the delivery-window contract. T2 and T8
correctly use `run_named` and reject `VACUUM`; T7 must do so after GREEN as well.

**Reproduction:** In a separate mutation worktree, W2 and W3 were changed by code-level
`#[ignore]` attributes (the post-edit lines were `red_m77_delivery_window.rs:452-454` and
`:504-506`). The target then printed `1 passed; 0 failed; 2 ignored` and returned 0.
With the GREEN T7 predicate (`SUITE_RC=0`), the exact branch printed
`PASS T7 оракул окна доставки ЗЕЛЁН (exit=0)`. Thus the gate accepts a world where neither
of task 2's two subject oracles ran.

**Condition to remove REJECT:** Architect changes only the T7 GREEN branch so it invokes
the existing `run_named` helper for both `vb_i_2_w2_client_equals_replay_after_refusals_are_retried`
and `vb_i_2_w3_client_equals_replay_when_refusal_hits_a_batch_rollover`, requiring a
non-vacuum `OK` outcome for each. `VACUUM` and `FAILED` must increment `FAILED`. Provide
the same ignore mutation as a raw proof that the GREEN branch becomes FAIL, then return
for the prescribed diff-only follow-up review.

## Confirmed round-2 closures

- §6bis states the source rule by construction, covers every current `SeriesBundle` field,
  binds book-derived frames to `(from, upto]`, and defines retry/rollover requirements K-1
  through K-3. This is sufficient for the selected B contract; no reopened finding on B.
- Against the full two-site candidate A mutation, the current suites produced continuity
  `5 passed / 1 failed`, delivery window `2 / 1`, and pump cost `0 / 1`. The four original
  task-1 subject tests were green under A; the added rollover/cost checks are therefore
  material rather than ceremonial.
- The exact current head produces continuity `1 / 5`, delivery window `1 / 2`, and pump
  cost `1 / 0`. `bash scripts/verify_M-77.sh` reports only its declared T4 RED failure
  and exits 1; every other named acceptance step passes.

## Done Block

```text
$ git rev-parse 28324874f33704e605198971f6b42d2358afc59e 1b359116a86f0370bde8729a243b70b477ccab3e
28324874f33704e605198971f6b42d2358afc59e
1b359116a86f0370bde8729a243b70b477ccab3e
exit=0

$ git diff --name-status 2832487..1b35911
A	crates/gateway/tests/red_m77_delivery_window.rs
M	crates/gateway/tests/red_m77_frame_book_continuity.rs
A	crates/gateway/tests/red_m77_pump_cost.rs
M	milestones/M-77-frame-book-continuity.md
M	scripts/verify_M-77.sh
exit=0

$ bash scripts/verify_M-77.sh
PASS  T0 все три набора на месте
PASS  T0 состав полон: 2 контроля + 1 сторож цены + 7 предметных
PASS  T1 оба набора исполняют resume+pump в ПРОД-ФОРМЕ (Р-2)
PASS  T2 контроль снимочного хвоста ЗЕЛЁН (исполнено 1)
PASS  T2 дискриминатор окна отказа ЗЕЛЁН (исполнено 1) — окно достижимо
PASS  T3 cargo fmt --all -- --check (exit=0)
PASS  T3 cargo clippy --all-targets --all-features -D warnings (exit=0)
INFO  RED-ФАЗА: cargo test --all exit=101 — задача 3 не исполнена, это ОЖИДАЕМО
PASS  T5 краснота локализована: ровно 7 предметных теста M-77, посторонних нет
FAIL  T4 задача 3 не исполнена — милестоун не закрыт (RED-фаза, см. INFO выше)
PASS  T6 запретные пути не тронуты (contracts / gateway-serve/src / docker-compose.yml)
PASS  T7 контракт развязки Б объявлен в спеке (§6bis, сигнатура названа) — присутствие
PASS  T7 RED-фаза: оба предметных теста окна доставки красны, дискриминатор зелён
PASS  T8 сторож цены на границе pump ЗЕЛЁН (исполнено 1) — работа тика не растёт с числом батчей
---
VERDICT: FAIL (1)
exit=1

$ for sha in dcff9aad6f41b55cc81a1da944cf2ea9d92d5358 fbd2aef710122bdda2ab3ea5b4b2e4efe48ef378 67cf2a75d3b015bcefc7af91e491bd765d9ae5fb; do git cat-file -t "$sha"; done
commit
commit
commit
exit=0

$ cargo test -p gateway --test red_m77_frame_book_continuity -- --test-threads=1
test result: FAILED. 1 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
exit=101
$ cargo test -p gateway --test red_m77_delivery_window -- --test-threads=1
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
exit=101
$ cargo test -p gateway --test red_m77_pump_cost -- --test-threads=1
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ nl -ba crates/gateway/tests/red_m77_delivery_window.rs | sed -n '450,506p'
452	#[test]
453	#[ignore = "critic mutation: T7 green branch must reject a vacuum of its named subject tests"]
454	fn vb_i_2_w2_client_equals_replay_after_refusals_are_retried() {
504	#[test]
505	#[ignore = "critic mutation: T7 green branch must reject a vacuum of its named subject tests"]
506	fn vb_i_2_w3_client_equals_replay_when_refusal_hits_a_batch_rollover() {
$ cargo test -p gateway --test red_m77_delivery_window -- --test-threads=1
test result: ok. 1 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
mut_window_exit=0
$ SUITE_RC=0; [ "$SUITE_RC" -eq 0 ] && [ "$W_ONLY_RC" -eq 0 ] && echo 'PASS T7 оракул окна доставки ЗЕЛЁН (exit=0) — контракт диапазона держится под отказом'
PASS T7 оракул окна доставки ЗЕЛЁН (exit=0) — контракт диапазона держится под отказом
green_branch_exit=0

$ cargo test -p gateway --test red_m77_frame_book_continuity -- --test-threads=1
test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101
$ cargo test -p gateway --test red_m77_delivery_window -- --test-threads=1
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101
$ cargo test -p gateway --test red_m77_pump_cost -- --test-threads=1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ for c in gateway gateway-serve derive recorder; do test -e "docs/fa/$c.md"; printf 'docs/fa/%s.md exit=%s\n' "$c" "$?"; done
docs/fa/gateway.md exit=1
docs/fa/gateway-serve.md exit=1
docs/fa/derive.md exit=1
docs/fa/recorder.md exit=1

$ bash scripts/next_artifact_id.sh C
C-212
id_exit=0
$ git ls-remote origin refs/reserved/C-212
ff969932ccf9856ef229abe73c997797bdc76de3	refs/reserved/C-212
exit=0
```

The four missing per-crate FA documents are an existing architectural-documentation debt;
per `reading-map.md` §2 this audit relies on `docs/fa/viz-backend.md`. They are not a new
blocker for the narrowed M-77 round.
