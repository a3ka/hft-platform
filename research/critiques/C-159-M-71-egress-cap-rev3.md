<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: b8aa62909e6bb2c43f3e94daaf52948ac4e95efc
verdict: REJECT
-->

# C-159 — M-71 egress cap rev3: REJECT

## Scope audited

Audited committed chain `3b496208a64edbf00a66b93986ff8529d0c93aa9..b8aa62909e6bb2c43f3e94daaf52948ac4e95efc`: rev3 milestone, both levels of RED oracles, separate COMPILE-RED boundary test, startup RED test, door probe, verifier, arbitration `A-021`, and the two prior critiques. This is non-T1: no `crates/contracts/**` diff and no new contract type or trait signature. `GATEWAY_BANDS` is untouched.

I applied `A-021` as binding. The former two classes are not repeated as rejections: level 1 now names `Snapshot`/`Frame`, level 2 tests both success envelopes, E-3 is a family through `N_MAX_BANDS=12`, and the three named residuals remain NOTES only. `VB-I-2` remains material: a live transport path must not expose a result that its gateway proof silently leaves unbounded.

## Verdict: REJECT

The repaired construction does **not** yet cap complete outbound text. A new, executed v1 error path emits 2,100,084 B to the client outside both levels, with a short `sub`; it is not one of `A-021`'s named residuals. Separately, B becomes false-red against a no-truncation cap because its L2-only fixture can never contain the asserted OHLCV rows. Do not dispatch dev from this artifact set.

### R1 — a client-controlled `unknown_venue` error bypasses both level 1 and level 2

The live v1 handler takes an arbitrary JSON string from `selector.venue`:

1. `wire_v1::parse_selector` returns `SelectorError::UnknownVenue(other.to_string())`.
2. `handle_v1_message` constructs `format!("unknown venue: {name}")`.
3. `send_v1_error` serializes `wire_v1::error_msg` and calls `sink.send(Message::Text(text))`.

Neither level contains this path. Level 1 can only see gateway `Snapshot`/`Frame`; level 2 invokes `serve::{snapshot_msg,frames_msgs}` and `wire_v1::{snapshot_msg,frame_msg}`. The supposedly mechanical L2 inventory is the same four literal names, so it also passes while omitting `error_msg`/`send_v1_error`.

Executable probe on audited HEAD, with `sub="s1"` and `selector.venue = "V".repeat(2_100_000)`, used the public parser and the exact v1 error builder used by the handler:

```text
OUTBOUND_V1_UNKNOWN_VENUE_ERROR_BYTES=2100084
test critic_unknown_venue_error_exceeds_founder_limit_outside_both_cap_oracles ... ok
```

The result is 100,084 B above the proposed 2,000,000-B cap. It is a real outbound `Message::Text` at `gateway-serve/src/lib.rs:1025-1031`, not a bare intermediate object. Its `sub` is deliberately two bytes, so this is not the named unbounded-sub-id residual. It is neither a macro/trait discovery nor an above-`N_MAX_BANDS` proxy. This is therefore a new class under `A-021` and a valid REJECT.

Repair the existing level-2 construction rather than adding a third resource proxy: mechanically inventory all outbound text construction/send paths and make the v1 error route fail closed on the same full-text bound. Add a RED case with a short id and a large rejected selector field. The particular error representation is not decided here; the test must demonstrate that no client-controlled error text above the bound is emitted.

### R2 — B is false-red once a cap preserves the response

`journal_prod_shape` emits only `MdPayload::L2Snapshot`. It emits no trades, so the narrow B fixture has a legitimate heatmap but no OHLCV. An independent executable reconstruction of exactly ten buckets and five levels per side printed:

```text
L2_ONLY_FIXTURE heatmap=100 ohlcv=0
test critic_l2_only_fixture_has_no_ohlcv_but_has_the_expected_heatmap ... ok
```

But B requires `s.series.ohlcv.len() == N_BUCKETS` (10). In a disposable, no-truncation 2,000,000-B guard that rejected only oversized full series, eight of the nine library oracles passed and B alone reached and failed that assertion:

```text
PL-I-5 B: OHLCV урезан — 0 баров при 10 бакетах фикстуры
```

The guard does not mutate accepted data; zero OHLCV follows from the fixture's event kind. Thus a correct cap cannot make the declared RED suite green without changing the unrelated expected fact. This contradicts the anti-false-RED requirement.

Repair B's setup, without changing output composition: either assert zero OHLCV for the explicitly L2-only fixture, or add the stipulated trade events if OHLCV presence is intended. Preserve the independent heatmap completeness assertion. The amended B must be green for a cap that rejects excess and leaves accepted responses intact.

## Checks on the rev3 repairs

- **E-3/E-4:** under the disposable full-series cap, `pl_i_5_e3_family_of_honest_multi_band_requests_is_served` and `pl_i_5_e4_equal_bytes_opposite_composition_get_the_same_verdict` both passed. This is evidence that the family admits each `n=1..12` cheap valid request and the opposite-composition pair. A proxy above 12 remains the named `A-021` NOTE residual, not a rejection.
- **Door probe:** baseline passes. Adding one temporary public `fn ...(&Selector)` that no oracle calls makes `red_egress_doors.sh` fail exactly once. Removing only `validate_selector` from `EXCLUDE` fails exactly on that name; its body returns validation errors and constructs no response, so the explicit exclusion is justified. This does not cure the L2 `error_msg` omission above.
- **W-C2:** correct. Replacing the v1 snapshot envelope by bare `Snapshot` makes W-C2 fail with `0 B`; adding a 256-byte field makes it fail with `316 B`. Its lower and upper checks catch envelope removal and expansion.
- **Mutation C:** on plan HEAD, C prints `FAIL: C НЕ ГОТОВ` and no `PASS`, as required before a green base. In a disposable live-anchor 2-MB implementation (with B's false expected OHLCV fact corrected only for this controller experiment), C printed `PASS: C база зелена, мутация роняет набор, честная нагрузка (E) цела`. Moving the anchor to an uncalled wrapper instead produced `FAIL: C мутация дала all_red=0 e_red=0`. Thus C now exercises both L1 and L2 and rejects a dead anchor; the remaining blocker is B's test fact, not a false C PASS.
- **Boundary:** `red_egress_cap_boundary.rs` is genuinely separate COMPILE-RED (`E0425` for absent `gateway::DEFAULT_MAX_RESPONSE_BYTES`) and its `lo`/`lo+1` search narrows to one trade. Its declared level-1 object is compatible with `A-021`; the full text rejection above is at level 2, not a repeat of the pre-arbitration resource-boundary complaint.

## Founder-owned 2 MB

I do not approve or set the number. It plainly constrains the reproduced harmful success cases: the current wide-book response is 7,841,085 B (3.92×), and the dense no-heatmap success envelope is 2,804,778 B (1.40×). Conversely, E asserts its honest response is below 100,000 B (at least 20× headroom), while W-C1 asserts both ordinary wire forms are below 200,000 B (at least 10× headroom).

Those margins make 2 MB a coherent provisional magnitude, but R1 means it is not yet a cap on total client egress. The choice remains founder-owned after the construction is repaired.

## Baseline and artifact checks

- `bands=[0.99]` still serves 59,980 heatmap cells versus 100 at the default; B measured the full response at 7,841,085 B.
- Dense trades still serve 2,804,765 B with empty heatmap before implementation; wire level 2 measures 2,804,778 B (`ServeMsg`) and 2,804,809 B (v1).
- `bash scripts/verify_M-71.sh` is accurately RED: `FAIL (8)`, exit 1. The eight are CI clippy, CI all-tests, L1 suite, compile boundary, L2 suite, startup suite, C-not-ready, and unannounced compose variable. The door probe and all five neighboring invariant checks pass.
- `bash scripts/verify_design_claims.sh --merge-preview origin/main` returns `PASS (0 нарушений)`, exit 0.

## Done Block

```text
$ bash scripts/reserve_artifact_id.sh C
C-159
exit=0

$ cargo test -p gateway-serve --test critic_error_egress -- --nocapture
running 1 test
OUTBOUND_V1_UNKNOWN_VENUE_ERROR_BYTES=2100084
test critic_unknown_venue_error_exceeds_founder_limit_outside_both_cap_oracles ... ok
test result: ok. 1 passed; 0 failed
exit=0

$ cargo test -p gateway --test critic_l2_only_fixture -- --nocapture
running 1 test
L2_ONLY_FIXTURE heatmap=100 ohlcv=0
test critic_l2_only_fixture_has_no_ohlcv_but_has_the_expected_heatmap ... ok
test result: ok. 1 passed; 0 failed
exit=0

$ [disposable envelope removal] cargo test -p gateway-serve --test red_egress_cap_wire pl_i_5_w_c2_envelope_overhead_is_bounded_only_at_fixed_id -- --nocapture
PL-I-5 W-C2: накладные v1-конверта при коротком id 0 Б — форма изменилась
exit=101

$ [disposable envelope growth] cargo test -p gateway-serve --test red_egress_cap_wire pl_i_5_w_c2_envelope_overhead_is_bounded_only_at_fixed_id -- --nocapture
PL-I-5 W-C2: накладные v1-конверта при коротком id 316 Б — форма изменилась
exit=101

$ bash scripts/verify_M-71.sh
VERDICT: FAIL (8)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --check 3b496208a64edbf00a66b93986ff8529d0c93aa9..b8aa62909e6bb2c43f3e94daaf52948ac4e95efc
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-25T23:11Z
- Milestone: M-71-egress-cap
- Статус: BLOCKED
- HEAD: b8aa629 — docs(M-71): спека rev3 — решение арбитра A-021 записано вместе с тремя поправками ко мне [architect]

## §B — Что я сделал
- Аудировал весь committed rev3 artifact set и применил обязательную границу A-021.
- Исполнением нашёл новый uncapped v1 error egress и независимое ложное RED в B; проверил двери, E-3/E-4, W-C2, C, boundary, оба verify-скрипта.

## §C — Артефакты / результаты
- `research/critiques/C-159-M-71-egress-cap-rev3.md`
- Done Block: `verify_M-71.sh` → FAIL (8), exit 1; design claims → PASS, exit 0; v1 error → 2,100,084 B, exit 0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь только committed plan-time artifacts M-71 на feat/M-71-egress-cap после C-159.
  Обязательные acceptance criteria: (1) полный outgoing Message::Text v1 error с коротким
  sub и client-controlled unknown venue не может превысить egress cap; L2 inventory
  механически включает этот send/construction path, а не только snapshot/frame; (2) B не
  требует OHLCV от L2-only fixture — либо исправь ожидаемое значение, либо добавь явные
  trade events и сохрани независимую проверку полноты; (3) сохрани A-021: L1 Snapshot/Frame,
  L2 обе success wire-формы, E-3 family 1..12, W-C2 и C's base/mutation/E triad; не меняй
  contracts, GATEWAY_BANDS или output composition. Запусти verify_M-71.sh и
  verify_design_claims.sh --merge-preview origin/main. Закоммить artifacts и запроси новый
  critic gate с base/head.
  ```
- Push-статус: ✅ pushed to `origin/feat/M-71-egress-cap` with this critic verdict (commit SHA recorded by the delivery command).
- ✅ кэш убран: `target/` audit worktree and all disposable mutation worktrees removed before handoff.

## §E — Риски / открытые вопросы
- R1 is a new class outside A-021's three named NOTE residuals; dev dispatch remains blocked.
- Founder alone sets the 2-MB value; no signed decision is requested by this critique.

=== END HANDOFF ===
