<!-- GATE-META
milestone: M-100
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: ee11c3dd7c825a70f84d399172f9e10c6d51522c
verdict: NOTE
-->

# C-309 — M-100 three serving contracts, round 4: NOTE

## Verdict: NOTE

`A-054` is executed completely at the fetched subject tip.  No new blocking fact
was found in the allowed round-4 scope.  This is the RAW-gate verdict on the
frame wire surface; it also supplies the `gates.md` §9 recheck for the changed
scope.

The applicable live FA invariants are `VB-I-2` (live equals replay), `VB-I-4`
(additive wire form), and `VB-I-12` (an unambiguous book-observation update).

## A-054 §4 execution

| Item | Result | Evidence |
|---|---|---|
| A1 | PASS | `assert_frame_rows` makes `cob_observed`, a `depth_series` array, the exact requested row set, and a non-empty series for every row mandatory. `w1` invokes it for `(15_000_000, bid)`. [`red_m100_three_contracts_wire.rs:252`](../../crates/gateway-serve/tests/red_m100_three_contracts_wire.rs) and [`:341`](../../crates/gateway-serve/tests/red_m100_three_contracts_wire.rs) |
| A2 | PASS | `w2` appends a book update and applies the same guard to all fourteen bid/ask rows, including non-empty points. [`red_m100_three_contracts_wire.rs:363`](../../crates/gateway-serve/tests/red_m100_three_contracts_wire.rs) |
| A3 | PASS | After the second subscription snapshot, `w7` appends and judges the first frame as exactly `(60_000_000, ask)`. [`red_m100_three_contracts_wire.rs:492`](../../crates/gateway-serve/tests/red_m100_three_contracts_wire.rs) |
| A4 | PASS | `h1` now guards that replay contains live bid `64_980` before judging live/replay equality; the reproduced failure reaches only that final equality. [`red_m100_heatmap_frame_close.rs:158`](../../crates/gateway/tests/red_m100_heatmap_frame_close.rs) |
| A5 | PASS | §8 separates no-frame projection, M-drop, snapshot projection, no-`view` frames, and stale `ViewSpec`; it explicitly says `w1`/`w2`/`w7` judge their first live frame. [`M-100-three-serving-contracts.md:172`](../../milestones/M-100-three-serving-contracts.md) and [`:191`](../../milestones/M-100-three-serving-contracts.md) |
| A6 | PASS | §14 records `C-308` → `A-054`, the measured first-frame fact, and the A1–A6 landing. [`M-100-three-serving-contracts.md:248`](../../milestones/M-100-three-serving-contracts.md) |

The helper's exact-set comparison is at least as strong as the required `len == 1`
for the one-row cases: empty, absent, duplicate, and unselected-row frames all
fail; the per-row `series` assertion additionally catches the key-without-points
mutant.  It does not wait for another frame.

## Required RED evidence and new measured facts

- The wire test compiles (`--no-run`, exit 0).
- `w1`, `w2`, and `w7` each remain RED at the same earlier admission point:
  `invalid_selector`, `missing field bands` (exit 101).  Thus their new frame
  assertions are compiled but are not falsely presented as GREEN against the
  pre-dev implementation.
- `h1` remains RED at its final `VB-I-2`/`VB-I-12` equality (exit 101).  Its
  actual replay map contains bid `64_980`; therefore both setup guards ran
  before the intended behavioural failure.

## `gates.md` §9 recheck, limited to this change

- **(a) Code claims:** `git diff a660b3a7..HEAD -- crates/*/src` is empty.  The
  claimed test behaviour was reproduced at the subject tip, and
  `verify_design_claims.sh --merge-preview origin/main` passed.
- **(b) Authority:** the post-arbitration range changes only architect-owned
  RED tests and the M-100 milestone, plus the arbiter-owned A-054 artifact.  It
  does not touch `crates/contracts/**`, a money/risk path, data composition, or
  a boundary-C decision.
- **(c) Coherence:** the mutation table, §8 oracle description, and §14 circle
  journal agree with the executable guards; whitespace and git object checks
  passed.

Per A-054, this verdict does not reopen `C-303` R1–R3/N2/N3, `C-305` R1, or the
prescribed A1 form.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-100-three-serving-contracts
ee11c3dd7c825a70f84d399172f9e10c6d51522c
exit=0

$ git merge-base --is-ancestor ee11c3dd origin/feat/M-100-three-serving-contracts
exit=0

$ git diff --name-status f92d66f9..HEAD
M	crates/gateway-serve/tests/red_m100_three_contracts_wire.rs
M	crates/gateway/tests/red_m100_heatmap_frame_close.rs
M	milestones/M-100-three-serving-contracts.md
A	research/arbitration/A-054-m100-frame-projection-oracle.md
exit=0

$ cargo test -p gateway-serve --test red_m100_three_contracts_wire --no-run
Finished `test` profile [unoptimized + debuginfo] target(s) in 22.21s
Executable tests/red_m100_three_contracts_wire.rs (target/debug/deps/red_m100_three_contracts_wire-46942f5e68aedaf6)
exit=0

$ cargo test -p gateway-serve --test red_m100_three_contracts_wire w1 -- --nocapture
для «w1» ожидалось «snapshot», пришло: {"code":"invalid_selector","message":"selector deserialize: missing field `bands`","sub":"w1","type":"error","v":1}
test w1_server_resolves_calc_key_and_view_selects_rows ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out
exit=101 (expected RED: pre-existing mandatory `bands` admission)

$ cargo test -p gateway-serve --test red_m100_three_contracts_wire w2 -- --nocapture
для «w2» ожидалось «snapshot», пришло: {"code":"invalid_selector","message":"selector deserialize: missing field `bands`","sub":"w2","type":"error","v":1}
test w2_absent_view_shows_all_rows ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out
exit=101 (expected RED: pre-existing mandatory `bands` admission)

$ cargo test -p gateway-serve --test red_m100_three_contracts_wire w7 -- --nocapture
для «w7» ожидалось «snapshot», пришло: {"code":"invalid_selector","message":"selector deserialize: missing field `bands`","sub":"w7","type":"error","v":1}
test w7_resubscribe_changes_view_not_calc ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out
exit=101 (expected RED: pre-existing mandatory `bands` admission)

$ cargo test -p gateway --test red_m100_heatmap_frame_close h1 -- --nocapture
assertion `left == right` failed: M-100 / VB-I-2 / VB-I-12: колонка живого кадра — объединение наблюдений кадра, а не состояние бакета на последнем наблюдении: уровень, снятый внутри кадра, живёт у клиента
 left: {(1752000010, "ask", 6501000000000): 400000000, (1752000010, "bid", 6498000000000): 300000000, (1752000010, "bid", 6499000000000): 500000000}
right: {(1752000010, "ask", 6501000000000): 400000000, (1752000010, "bid", 6498000000000): 300000000}
test h1_level_removed_within_one_live_frame_disappears_for_client ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out
exit=101 (expected RED; A4 setup guards passed before this assertion)

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [6-RFC-SHA] SHA-подобных токенов: всего=40 проверено=40 пропущено=0
PASS  [7-RFC-PATH] путей-кандидатов: всего=283 проверено=183 пропущено=100
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --check origin/main...HEAD && git fsck --no-dangling --no-reflogs
exit=0

$ git show --no-patch --format='commit=%H%ncommitter=%cI%nsubject=%s' refs/reserved-cache/C-309
commit=177a3c053bc8141e3d4f59fc0f4fe74a225d50b2
committer=2026-10-11T14:55:39+00:00
subject=reserve C-309 nous 2026-10-11T14:55:39Z Ubuntu-2404-noble-amd64-base 1406659 4c2e7df0-30a2-4112-845d-c7589d91a9b4
exit=0

$ git fetch origin && git rev-parse origin/feat/M-100-three-serving-contracts
ee11c3dd7c825a70f84d399172f9e10c6d51522c
exit=0
```

=== HANDOFF: CRITIC → ENGINE-DEV ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-11T14:56Z
- Milestone: M-100-three-serving-contracts
- Статус: DONE — `C-309` NOTE; plan-time RAW gate passed for round-4 scope
- HEAD: ee11c3dd — docs(M-100): A-054 A5–A6 … [architect]

## §B — Что я сделал
- Проверил A1–A6 и воспроизвёл предписанную RED/Done Block форму.
- Проверил §9(a)–(c) только для post-arbitration range.

## §C — Артефакты / результаты
- `research/critiques/C-309-M-100-three-serving-contracts-round4.md`
- Done Block: compile exit=0; w1/w2/w7/h1 expected RED exit=101; merge-preview claims exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `engine-dev`
- **Paste-ready промпт:**
  ```
  Реализуй M-100 только по committed milestone и RED-suite на
  origin/feat/M-100-three-serving-contracts после C-309 NOTE. Сохрани A-054 A1–A4:
  первый кадр w1/w2/w7 не обходится ожиданием, а h1 остаётся live==replay oracle.
  Работай только в Allowed paths engine-dev, не меняй sacred tests/verify/milestone.
  Сделай GREEN, прогони acceptance и передай tester с Done Block.
  ```
- Push-статус: pending — critic verdict commit follows this audited tip.
- Кэш: pending cleanup after verdict delivery.

## §E — Риски / открытые вопросы
- Ожидаемые RED сейчас являются плановым состоянием до реализации; новые blocking findings отсутствуют.

=== END HANDOFF ===
