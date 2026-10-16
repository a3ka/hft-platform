<!-- GATE-META
milestone: M-90
audited_repo: a3ka/hft-platform
audited_base: 009d82a71377877d0b9ab239790f0e6a3c28a1b8
audited_head: 6936f0b802b4723aeeaf54c6a97522e06d8f49bf
verdict: REJECT
-->

# C-266 — M-90 warmer selector round 2: non-injective CI map and unguarded legacy cadence source

## Verdict

**REJECT — dev remains blocked.** The architect committed the complete plan-time
artifact set on the audited branch tip, and the new `w4a`…`w4e` worlds do close
the missing per-fingerprint-axis coverage identified in C-265-F1. Two defects
remain: the purported exact CI map can silently omit commands appended to a
mapped multiline block, and the prohibition of a second `CHECKPOINT_*` selector
source omits cadence.

`C-266-F2` is the second consecutive REJECT of the C-265-F2 CI-map cause. Under
`gates.md` §0 and the critic profile, this verdict must go to a fresh-context
**arbiter**, not directly into a third architect↔critic loop. Dev must not be
dispatched.

## Audited subject and artifact set

- Subject: `origin/feat/M-90-warmer-selector-single-source`.
- The handoff reference `6936f0b` equals the fetched branch tip
  `6936f0b802b4723aeeaf54c6a97522e06d8f49bf`; the final fetch found no drift.
- Audit base: `009d82a71377877d0b9ab239790f0e6a3c28a1b8` (`origin/main`).
- The committed plan-time set is present: M-90 milestone, sacred RED suite,
  acceptance gate, and CI-map anti-placebo probe. The range has no
  `contracts/**` change and no new T2 type or trait signature; the existing
  `CHECKPOINT_RUNNER` seam is the test boundary.
- Scope conforms to the milestone: architect owns the test/gate/milestone
  artifacts; planned production changes are within the engine-dev gateway and
  deploy/compose paths. This is a production-path milestone, not the relaxed
  harness track.

The applicable live invariants are **VB-I-11** (writer and reader must not use
different checkpoint history silently) and **OPS-I-8** (a live process that does
no useful work is an incident). The red suite uses the required production
composition path, and its fixture remains a direct check of that boundary.

## C-265-F1 disposition

**Closed.** `w4a`…`w4e` each make exactly one non-default fingerprint input in
host `.env` and use a control that differs from both that world and the compose
default. The current baseline is correctly RED for venue, symbol, timeframe,
and window; cadence is green because the existing compose environment already
supplies it. The full suite reports `2 passed; 6 failed`: `w2` and `w4e` are
valid positive controls, while `w1`, `w3`, and `w4a`…`w4d` are expected REDs.

## Blocking findings

### C-266-F1 — cadence is a selector axis but has no legacy-source rejection

M-90 §1 defines cadence as part of the one-source selector, and
`selector_fingerprint` hashes `depth_cadence_ms`. Yet the legacy-source oracle
enumerates only five variables in `SCRIPT_SELECTOR_VARS`:
`CHECKPOINT_{VENUE,SYMBOL,TIMEFRAME_MS,BANDS,WINDOW_MS}`. Neither the RED suite,
the milestone I-3 table, nor the forbidden list rejects
`CHECKPOINT_DEPTH_CADENCE_MS`.

An implementation can therefore reintroduce an own cron source only for
cadence: use `GATEWAY_DEPTH_CADENCE_MS` when it is set (so `w4e` passes), but
fall back to or honour `CHECKPOINT_DEPTH_CADENCE_MS` otherwise. It violates the
objective whenever an operator puts that legacy variable in cron, yet all
current `w1`…`w4e` pass after implementation because none supplies that input.
This is the same two-source failure class as TD-227, applied to an axis that
changes the checkpoint key.

**Required correction:** make cadence explicit in I-3, §4, and
`SCRIPT_SELECTOR_VARS`; add a RED case showing that
`CHECKPOINT_DEPTH_CADENCE_MS` causes a named non-zero refusal and the runner is
not called. Add the temporary mutant that honours only that variable and record
the failing oracle in the next artifact set.

### C-266-F2 — the CI-map exclusion is keyed only by a multiline block’s first line

The revised parser returns the complete `run: |` text, but `verify_M-90.sh`
reduces every map key to `first="${step%%$'\n'*}"`. The sole base-event
exclusion is therefore keyed as `set -euo pipefail`, rather than by the unique
full block (or a unique job/step identity). Any substantive command appended to
that block is silently excluded with the base-event plumbing.

Measured adversary: in a temporary copy of `ci.yml`, append
`bash scripts/tests/red_artifact_ids.sh --battery` after the base step writes
`sha` to `GITHUB_OUTPUT`. `VERIFY_M90_CI_DRY=1` still returns 0, emits the
`set -euo pipefail` exclusion, and reports `55 of 55` steps. The supplied
three-world probe cannot see this class: it adds a new block whose first line is
different, rather than adding work to an existing mapped block.

Thus the claimed exact map is not exact and fails the C-265-F2 requirement to
fail when CI gains unaccounted substantive work. A future gate command can be
placed in a mapped block and avoid execution by this acceptance script.

**Required correction for arbiter decision:** make the mapping injective over a
complete CI step—e.g. map the full normalized block together with its job/step
identity, never the first command alone. Preserve the named base-event
exception only when the complete known base block matches. Extend
`red_verify_M-90_ci_map.sh` with a fourth setup-guarded world that appends a
new command to that mapped base block and requires `VERIFY_M90_CI_DRY=1` to
fail. The arbiter should decide the exact stable identity/normalization and
whether an equivalent implementation fully discharges C-265-F2.

## Disposition

The branch is not approved for engine-dev. Because F2 repeats the same rejected
CI-parity-map cause from C-265, founder should dispatch a fresh-context
**arbiter** under `gates.md` §0. The arbiter reads C-265, this verdict, M-90,
the RED suite, `verify_M-90.sh`, the CI-map probe, and `ci.yml`; it measures the
adversary above and decides the required map identity. The architect then makes
the resulting correction plus C-266-F1 before any new critic cycle.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-90-warmer-selector-single-source
6936f0b802b4723aeeaf54c6a97522e06d8f49bf
$ git merge-base --is-ancestor 6936f0b802b4723aeeaf54c6a97522e06d8f49bf origin/feat/M-90-warmer-selector-single-source
mandate_sha_ancestor_exit=0
$ git merge-base origin/main origin/feat/M-90-warmer-selector-single-source
009d82a71377877d0b9ab239790f0e6a3c28a1b8
$ final git fetch
audited_start=6936f0b802b4723aeeaf54c6a97522e06d8f49bf
head_after_final_fetch=6936f0b802b4723aeeaf54c6a97522e06d8f49bf
drift=none

$ git diff --name-status origin/main...HEAD
A	crates/gateway/tests/red_m90_warmer_cron_composition.rs
A	milestones/M-90-warmer-selector-single-source.md
A	research/critiques/C-265-m90-warmer-selector.md
A	scripts/tests/red_verify_M-90_ci_map.sh
A	scripts/verify_M-90.sh
$ git diff --check origin/main...HEAD
exit=0

$ cargo test -p gateway --test red_m90_warmer_cron_composition
test result: FAILED. 2 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ bash scripts/tests/red_verify_M-90_ci_map.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
VERDICT: PASS — 3 сценария
exit=0

$ VERIFY_M90_CI_DRY=1 VERIFY_M90_CI_FILE=<temporary ci with an extra command in the mapped base block> bash scripts/verify_M-90.sh
SKIP  ci-parity: «set -euo pipefail» — шаг «база события» (пишет sha в GITHUB_OUTPUT); локальный эквивалент — merge-base origin/main HEAD выше
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
VERDICT: PASS
mutation_exit=0

$ rg -n 'CHECKPOINT_DEPTH_CADENCE_MS' milestones/M-90-warmer-selector-single-source.md crates/gateway/tests/red_m90_warmer_cron_composition.rs deploy/bin/gateway-checkpoint-cron.sh docker-compose.yml
exit=1
$ rg -n 'SCRIPT_SELECTOR_VARS|GATEWAY_DEPTH_CADENCE_MS|depth-cadence-ms' crates/gateway/tests/red_m90_warmer_cron_composition.rs docker-compose.yml
crates/gateway/tests/red_m90_warmer_cron_composition.rs:60:const SCRIPT_SELECTOR_VARS: [&str; 5] = [
crates/gateway/tests/red_m90_warmer_cron_composition.rs:365:        depth_cadence_ms: Some(ms("GATEWAY_DEPTH_CADENCE_MS")),
crates/gateway/tests/red_m90_warmer_cron_composition.rs:582:    axis_world("depth_cadence_ms", "GATEWAY_DEPTH_CADENCE_MS=2000", |s| {
docker-compose.yml:320:      - --depth-cadence-ms=${GATEWAY_DEPTH_CADENCE_MS:-1000}
exit=0

$ bash scripts/verify_M-90.sh
FAIL  task1-3: red_m90_warmer_cron_composition (exit=101)
FAIL  task2: verify_M-48.sh (exit=1; it includes the expected M-90 RED suite)
PASS  ci-map: проба карты CI-паритета (red_verify_M-90_ci_map.sh — 3 мира)
FAIL  ci-parity: cargo test --all (exit=101; the expected M-90 RED suite)
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
SKIP  task4: §8-гейт — reviewer after deploy
VERDICT: FAIL (провалов: 3)
verify_exit=1

$ bash scripts/next_artifact_id.sh C
C-266
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-10-01T17:56Z
- Milestone: M-90-warmer-selector-single-source
- Статус: BLOCKED — REJECT C-266; C-265-F2 repeats
- HEAD: 6936f0b — architect round-2 artifact set

## §B — Что я сделал

- Audited the fetched branch tip and the complete committed plan-time artifact set.
- Reproduced the baseline RED suite, the supplied CI-map probe, the full acceptance gate, and the CI-map mapped-block adversary.
- Found an untested legacy cadence selector source and an ambiguous exact-map identity.

## §C — Артефакты / результаты

- `research/critiques/C-266-m90-warmer-selector-round2.md`
- Done Block: RED suite exit=101; CI-map probe exit=0; mapped-block adversary incorrectly exit=0; full `verify_M-90.sh` exit=1.

## §D — Следующий агент + инвокация

- **Следующий агент:** `arbiter` (fresh context, strong model)
- **Paste-ready промпт:**
  ```
  Роль: независимый arbiter на сильной модели со свежим контекстом. Разреши второй
  REJECT по M-90 на origin/feat/M-90-warmer-selector-single-source. Прочти
  research/critiques/C-265-m90-warmer-selector.md и
  research/critiques/C-266-m90-warmer-selector-round2.md, milestone M-90,
  crates/gateway/tests/red_m90_warmer_cron_composition.rs,
  scripts/verify_M-90.sh, scripts/tests/red_verify_M-90_ci_map.sh и ci.yml.
  Замером проверь: (1) append команды в уже исключённый multiline base block с
  first line `set -euo pipefail` всё ещё даёт VERIFY_M90_CI_DRY=1 exit 0; (2)
  CHECKPOINT_DEPTH_CADENCE_MS отсутствует в I-3/forbidden list/SCRIPT_SELECTOR_VARS,
  хотя GATEWAY_DEPTH_CADENCE_MS входит в selector_fingerprint. Вынеси обязательное
  решение: точная стабильная identity CI step, которую должен использовать map, и
  требуется ли отдельный RED на legacy cadence. Запиши A-NNN verdict, commit+push
  на subject branch. Не пиши код.
  ```
- Push-статус: pending this verdict commit to `origin/feat/M-90-warmer-selector-single-source`
- Кэш: ✅ кэш убран (22 GB generated `target/`)

## §E — Риски / открытые вопросы

- C-265-F2 is repeated; architect and critic must not enter a third loop without an arbiter decision (`gates.md` §0).
- The temporary production `CHECKPOINT_BANDS` mitigation remains erased by the next relevant deploy; M-90 implementation is still blocked.

=== END HANDOFF ===
