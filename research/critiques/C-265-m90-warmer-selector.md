<!-- GATE-META
milestone: M-90
audited_repo: a3ka/hft-platform
audited_base: 009d82a71377877d0b9ab239790f0e6a3c28a1b8
audited_head: b999daaff9ba0a5106cdd05b5c6f8520e9224c09
verdict: REJECT
-->

# C-265 — M-90 warmer selector: incomplete selector oracle and CI-parity hole

## Verdict

**REJECT — dev is blocked.** The committed architect artifact set is present and
correctly RED on the current branch tip, but its oracle does not protect the full
selector promised by M-90, and its acceptance script is not CI-parity complete.
Either defect permits a green implementation that remains wrong on the production
warm-resume path.

## Audited subject and artifact set

- Subject: `origin/feat/M-90-warmer-selector-single-source`.
- The handoff reference `0ec1b11` is an ancestor of the fetched tip
  `b999daaff9ba0a5106cdd05b5c6f8520e9224c09`; this audit judges that tip.
- Base: `009d82a71377877d0b9ab239790f0e6a3c28a1b8` (`origin/main`). The final
  fetch before this record found no branch drift.
- The plan-time set is complete: milestone, sacred RED suite, and fail-counter
  `verify_M-90.sh` were committed before dev. The range contains no
  `contracts/**` change, so there is no T1 contract-RFC; it introduces no T2
  type or trait signature either. The existing `CHECKPOINT_RUNNER` seam remains
  the planned test boundary.
- The allowed implementation paths conform to the role table: engine-dev owns
  `crates/gateway/src/**`, `deploy/**`, and its gateway-checkpoint operator
  handle in root compose; the architect owns the three committed artifacts.
  This is an executable production path, not the relaxed harness track.

The applicable live invariants are **VB-I-11**: writer and reader must not
silently use different checkpoint history, and **OPS-I-8**: a system that is
alive but serves no useful work is an incident. `w1` and `w3` are RED on the
tip while `w2` is a useful green positive control.

## Blocking findings

### C-265-F1 — `w1`/`w2` cover only bands, not every checkpoint-key selector axis

M-90 §1 promises one source for venue, symbol, timeframe, bands, window, and
cadence. All six influence `selector_fingerprint` in
`crates/gateway/src/lib.rs:4257-4272`. Yet the only non-default input supplied
to `prod_path` is `GATEWAY_BANDS` (`red_m90_warmer_cron_composition.rs:458`);
the positive control supplies no selector override (`:474`). `w3` rejects the
five legacy `CHECKPOINT_*` names, but it never varies the corresponding
`GATEWAY_*` value, and it does not vary cadence.

A wrong implementation can therefore pass `w1`–`w3`: reject every
`CHECKPOINT_*`, remove only the bands flag from the cron argv, export and parse
only `GATEWAY_BANDS`, and retain hard-coded/default venue, symbol, timeframe,
window, or cadence. It passes both current input worlds, but a host `.env` with
for example `GATEWAY_WINDOW_MS=30000` produces a writer checkpoint under the
default 60000-ms key while gateway-serve reads the 30000-ms key. The result is
the same silent cold replay / `not_ready` class that M-90 claims to exclude.

**Required correction:** add behavioural RED worlds which independently make
each selector axis that participates in the fingerprint non-default and prove
the cron-written checkpoint is found by the server selector. Include a mutant
that transports only `GATEWAY_BANDS` (or leaves any one other axis on its old
default) and demonstrate that the relevant new oracle fails. Preserve a
distinguishable wrong-selector control for every new world.

### C-265-F2 — `verify_M-90.sh` silently omits substantive multiline CI work

`verify_M-90.sh:40` parses only one-line `run:` steps and `:64-65` skips every
multiline block as generic plumbing. That class includes the artifact-ID
anti-placebo test and its mutant battery in `.github/workflows/ci.yml:266-269`:
`red_artifact_ids.sh` and `red_artifact_ids.sh --battery`. They are substantive
gate tests (51 scenarios in the direct run), not base-event plumbing or the CI
aggregate.

Thus the acceptance script's CI-parity claim is false and can stay green while
that CI coverage is absent or red. `gates.md` §3 requires every CI command, or
an explicitly named and justified non-applicable exception; a category-wide
multiline skip is neither.

**Required correction:** execute the two artifact-ID commands in the same
forms as CI, and replace the generic multiline exclusion with an exact,
reviewable mapping. Only a specifically named command whose effect is truly
CI plumbing may be excluded, with its reason recorded; the map must fail when
CI gains an unaccounted `run:` block.

## Disposition

Architect must repair the RED suite and acceptance script, commit the amended
artifact set, and request a new critic round. Do not dispatch engine-dev while
F1 or F2 remains open. This is the first M-90 REJECT, so arbitration is not
yet triggered.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-90-warmer-selector-single-source
b999daaff9ba0a5106cdd05b5c6f8520e9224c09
$ git merge-base origin/main origin/feat/M-90-warmer-selector-single-source
009d82a71377877d0b9ab239790f0e6a3c28a1b8
$ git merge-base --is-ancestor 0ec1b115038cf9c73e2342c4456ea45525b5d5f5 origin/feat/M-90-warmer-selector-single-source
mandate_sha_ancestor_exit=0
$ final git fetch
audited_start=b999daaff9ba0a5106cdd05b5c6f8520e9224c09
head_after_final_fetch=b999daaff9ba0a5106cdd05b5c6f8520e9224c09
drift=none

$ git diff --name-status origin/main...HEAD
A	crates/gateway/tests/red_m90_warmer_cron_composition.rs
A	milestones/M-90-warmer-selector-single-source.md
A	scripts/verify_M-90.sh
$ git diff --check origin/main...HEAD
exit=0

$ cargo test -p gateway --test red_m90_warmer_cron_composition -- --nocapture
test w3_script_refuses_own_selector_copy_and_names_it ... FAILED
TD-227: при CHECKPOINT_VENUE в окружении cron'а скрипт позвал прогреватель
test w1_cron_warmer_snapshot_is_found_by_server_with_prod_dotenv ... FAILED
left: 300
right: 0
test w2_default_dotenv_snapshot_is_found_positive_control ... ok
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ bash -n scripts/verify_M-90.sh
exit=0
$ sed -n '4255,4273p' crates/gateway/src/lib.rs
format!("{:?}", sel.venue).hash(&mut h);
sel.symbol.hash(&mut h);
sel.timeframe_ms.hash(&mut h);
sel.window_ms.hash(&mut h);
... sel.bands ...
sel.depth_cadence_ms.hash(&mut h);
exit=0
$ rg -n 'prod_path\\(|GATEWAY_(BANDS|VENUE|SYMBOL|TIMEFRAME_MS|WINDOW_MS)' crates/gateway/tests/red_m90_warmer_cron_composition.rs
458: let (found, ctrl, argv) = prod_path(... GATEWAY_BANDS={SEVEN} ...)
474: let (found, ctrl, argv) = prod_path("GATEWAY_JWT_SECRET=x\\n")
exit=0

$ sed -n '40,66p' scripts/verify_M-90.sh
mapfile -t RUNS < <(grep ... '[^|[:space:]]' .github/workflows/ci.yml ...)
skip "ci-parity: многострочных run-блоков $nmulti ... — плумбинг CI ..."
exit=0
$ sed -n '260,270p' .github/workflows/ci.yml
run: |
  bash scripts/tests/red_artifact_ids.sh
  bash scripts/tests/red_artifact_ids.sh --battery
exit=0
$ bash scripts/tests/red_artifact_ids.sh
VERDICT: PASS (51/51) — все значения семи осей покрыты, состав сверен со спекой
exit=0

$ bash scripts/next_artifact_id.sh C
C-265
exit=0
$ bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-265 ← 29c3113fe8996b72302e9a9e31345ae7c2c27f70
C-265
reserve: резерв C-265 взят
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-10-01T13:30Z
- Milestone: M-90-warmer-selector-single-source
- Статус: BLOCKED — REJECT C-265
- HEAD: b999daa — verify-form correction [architect]

## §B — Что я сделал

- Audited the committed branch tip and reproduced the intended `w1`/`w3` REDs
  with `w2` as its positive control.
- Found the unprotected fingerprint axes and the omitted CI multiline test
  block; recorded both with reproduction and acceptance conditions.

## §C — Артефакты / результаты

- `research/critiques/C-265-m90-warmer-selector.md`
- Done Block: RED suite exit=101; syntax and diff checks exit=0; artifact-ID
  self-test exit=0; C-265 reserved atomically.

## §D — Следующий агент + инвокация

- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-90 is REJECTED by C-265 on origin/feat/M-90-warmer-selector-single-source.
  Amend only the architect artifact set: (1) add behavioural RED worlds for
  every selector axis in selector_fingerprint, including a mutant that wires
  only GATEWAY_BANDS and a wrong-selector control for each world; (2) make
  verify_M-90 execute the substantive multiline CI artifact-ID test and battery
  and fail on any unmapped CI run block. Preserve HFT_CRON_PRINT_ARGV and the
  existing cron→runner→real-binary→LiveReducer production-path oracle. Commit
  and push the revised milestone/RED/verify artifacts, then request critic
  round 2. Do not dispatch engine-dev first.
  ```
- Push-статус: pending this verdict commit to origin/feat/M-90-warmer-selector-single-source
- Кэш: pending cleanup after push

## §E — Риски / открытые вопросы

- A single bands-only repair can appear green while a later host selector
  override recreates TD-227; do not reduce the objective to the one measured
  bands incident.
- N/A

=== END HANDOFF ===
