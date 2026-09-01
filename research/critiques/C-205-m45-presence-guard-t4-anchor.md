<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: 2e63a37e5bf454da69b0fbd69de28c043b4caf4c
audited_head: a551508bfb20420e536bb75b3914f563193aa323
verdict: REJECT
-->

# C-205 — M-45 presence-guard, совмещённый круг A-031: REJECT

Role: critic — one combined follow-up required by `A-031` §3.2.  The audited
range is `a24193a..a551508` on `docs/M-45-rollout-signature`.

## Verdict

**REJECT — do not dispatch tester or merge.**

`A-031` P-3 is incomplete.  `T4` claims that it pins the negative and
case-normalisation tests by name, but its two `grep -q` checks at
`scripts/verify_M-45.sh:134-135` accept the test-name bytes anywhere in the
source file.  They do not require a Rust `fn` declaration and T4 runs the
whole target rather than the named tests.

In a dedicated detached worktree at `a551508`, I renamed the real spot
negative test to `fn o2_outside_allowlist_rejection_mutated()`, left its body
unchanged, and inserted only this comment:

```rust
// retired test: o2_symbol_outside_allowlist_is_not_captured
```

The complete `verify_M-45.sh` returned `VERDICT: PASS`, exit 0.  Thus the
advertised name pin is satisfied by a comment while the named test is absent.
This is carrier #9 of the exact A-031 class: the guard observes a token wider
than the required code construct.  It also repeats the false-anchor failure
that A-031 expressly required the round to exclude.

The required P-3 mutation, when the function name is fully changed without
retaining its original bytes, correctly makes T4 fail.  That does not clear the
comment-anchor world above.

`VN-I-3` remains live in `docs/fa/venues.md` §I: core `venues` may not branch
on a concrete `venue_id`.  The audited rollout continues to use one shared
compose value for both Binance adapters; this finding is confined to the
architect-owned acceptance guard and does not introduce such a branch.

## Conditions to clear

Architect must make T4 establish that its named negative and case tests are
actual executable Rust test declarations, not arbitrary text, and add the
comment-anchor adversarial mutation to its Done Block.  The repaired gate must
fail when each real named test is absent even if its old name remains in a
comment.

This is the one correction permitted by `A-031` §3.4.  The same critic then
checks only that corrective diff and its mutation evidence; another REJECT
returns the subject to founder rather than starting a new full circle.

## Other required scope checks

- P-1, P-2 and P-5 passed their prescribed isolated mutations: altered
  `PROD_DEFAULT` plus old-literal comment made T3 fail; absent raw-capture
  oracle made T6 and the gate fail; and an emptied E-002 fact cell with the
  epoch literal only in prose made T9 and the gate fail.
- The baseline gate passed with 24 PASS.  `C-204` and `R-167` B-1/B-2/B-3 are
  closed at this head: T9 reads the recorder compose epoch through the shared
  CLI, requires the E-002 fact-cell value, and T10/T10c enforce the signed
  literals and declared epoch.
- The group review of every named presence guard found no further bypass after
  the T4 carrier.  T8's name marker is coupled to an exact `cargo test` filter
  and a non-zero test-count; T3, T6, T9, T10 and T10b anchor their required
  construct or parse it through the shared CLI.
- `verify_design_claims --merge-preview origin/main` and all five CI-form
  barriers passed.  `check_review_fa.sh` correctly returned `SKIP` because the
  audited range does not modify `crates/**`.

## Done Block

```text
$ git ls-remote --heads origin docs/M-45-rollout-signature
a551508bfb20420e536bb75b3914f563193aa323	refs/heads/docs/M-45-rollout-signature

$ git merge-base HEAD origin/main
2e63a37e5bf454da69b0fbd69de28c043b4caf4c

$ bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'; echo verify_exit=${PIPESTATUS[0]}
PASS  T0 оракул присутствует: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T0 оракул присутствует: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
PASS  T1 cargo build --workspace
PASS  T2 cargo clippy --workspace --all-targets -D warnings
PASS  T2b cargo fmt --all --check (совпадает с ci.yml)
PASS  T3 venue-binance: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T3 venue-binance-futures: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T3 ожидаемый дефолт в оракуле не подменён: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T3 ожидаемый дефолт в оракуле не подменён: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
PASS  T4 venue-binance: allow-list оракул GREEN (23 тестов; негативный и регистровый запиннены поимённо)
PASS  T4 venue-binance-futures: allow-list оракул GREEN (21 тестов; негативный и регистровый запиннены поимённо)
PASS  T5 venue-binance: единственный вызов l2delta_event — внутри l2delta_emission_for
PASS  T5 venue-binance-futures: единственный вызов l2delta_event — внутри l2delta_emission_for
PASS  T5 хардкод-списка тикеров в venue-src нет
PASS  T5b venue-binance: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T5b venue-binance-futures: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T6 venue-binance: оракул сырого захвата red_l2delta_capture (M-18/CT-RFC-04) GREEN
PASS  T6 venue-binance-futures: оракул сырого захвата red_l2delta_futures (M-18/CT-RFC-04) GREEN
PASS  T7 crates/contracts/** не тронут
PASS  T8 DET-I-1 GREEN на смешанном журнале (снапшот+дельта; O-5 исполнен поимённо)
PASS  T9 раскатка исполнена И эпоха 'own-2026-09-m45-ethusdt' стоит В ЯЧЕЙКЕ ФАКТА (E-002)
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT EPOCH_ID=own-2026-09-m45-ethusdt)
PASS  T10b состав и эпоха внесены ОДНИМ коммитом (f3b84d41)
PASS  T10c мутация состава: 9 миров compose (каждый под setup-guard'ом) + 7 сценариев значений через ТОТ ЖЕ CLI, что и T10
VERDICT: PASS
verify_exit=0

$ T3: const PROD_DEFAULT = &["BTCUSDT", "ETHUSDT"]; with old literal only in a comment
FAIL  T3 venue-binance: дефолт ИЗМЕНЁН или тест не выполнился — merge запрещён (Граница C)
FAIL  T3 в оракуле crates/venue-binance/tests/red_l2delta_allowlist.rs изменена эталонная константа PROD_DEFAULT — гейт потерял смысл
VERDICT: FAIL (3 нарушений)
mutation_t3_exit=1

$ T6: mv crates/venue-binance/tests/red_l2delta_capture.rs /tmp/...; bash scripts/verify_M-45.sh
FAIL  T6 venue-binance: sacred-оракул сырого захвата crates/venue-binance/tests/red_l2delta_capture.rs ОТСУТСТВУЕТ — удаление оракула не является успехом (testing.md св. 4)
VERDICT: FAIL (1 нарушений)
mutation_t6_exit=1

$ T4 prescribed: fn o2_outside_allowlist_rejection_mutated()
FAIL  T4 venue-binance: allow-list оракул КРАСНЫЙ, либо прогнано НОЛЬ тестов, либо снят поимённо запиннутый негативный (o2_symbol_outside_allowlist_is_not_captured) или регистровый (o4_config_case_does_not_silently_disable_capture) сценарий
VERDICT: FAIL (1 нарушений)
mutation_t4_exit=1

$ T4 carrier: // retired test: o2_symbol_outside_allowlist_is_not_captured
$             fn o2_outside_allowlist_rejection_mutated()
PASS  T4 venue-binance: allow-list оракул GREEN (23 тестов; негативный и регистровый запиннены поимённо)
PASS  T4 venue-binance-futures: allow-list оракул GREEN (21 тестов; негативный и регистровый запиннены поимённо)
VERDICT: PASS
t4_comment_anchor_mutation_exit=0

$ T9: E-002 `EPOCH_ID` fact cell redacted; literal retained only in E-002 prose
FAIL  T9 раскатка исполнена (EPOCH_ID='own-2026-09-m45-ethusdt'), но этого значения НЕТ В ЯЧЕЙКЕ факта раздела E-002 файла docs/data-epochs.md
VERDICT: FAIL (1 нарушений)
mutation_t9_exit=1

$ git diff --quiet -- <mutated paths>; echo tree_clean_exit=$?
tree_clean_exit=0
tree_clean_exit=0
tree_clean_exit=0
tree_clean_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_gate_meta.sh; echo exit=$?
VERDICT: PASS — вердиктов проверено: 13, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_protected_artifacts.sh; echo exit=$?
OK: защищённые артефты целы на HEAD (2e63a37..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_artifact_ids.sh; echo exit=$?
OK: ни один коммит диапазона 2e63a37..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_review_fa.sh; echo exit=$?
SKIP (диапазон не трогает crates/**)
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/next_artifact_id.sh C
C-205
allocator_exit=0
```

## Резолюция доглядки

**CLEARED — C-205 закрыт; передать M-45 тестеру.**

Это доглядка единственного корректирующего диффа `dd3c4bf..8c3d4ec` по `A-031` §3 п.4,
не новый круг. Дифф ограничен `milestones/M-45-persist-l2delta.md` и
`scripts/verify_M-45.sh`. В T4 оба поимённых пина теперь требуют именно Rust-декларацию:
`^[[:space:]]*fn ИМЯ\(`. Следовательно, имя в комментарии более не удовлетворяет
предикату; это снимает ровно носитель C-205, не меняя предмет и не открывая новый класс.

**FA:** `VN-I-3` жив в `docs/fa/venues.md` §I: core `venues` не ветвится по конкретному
`venue_id`. Доглядка ограничена acceptance-скриптом и milestone-спекой; ни кода адаптеров,
ни этого инварианта она не меняет.

### Сырые замеры

```text
$ git ls-remote --heads origin docs/M-45-rollout-signature
8c3d4ecb9b35057f01a06b65c2f7571c368e9158	refs/heads/docs/M-45-rollout-signature
exit=0

$ git diff --name-only dd3c4bf..8c3d4ec
milestones/M-45-persist-l2delta.md
scripts/verify_M-45.sh
exit=0

$ bash scripts/verify_M-45.sh
PASS  T0 оракул присутствует: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T0 оракул присутствует: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
PASS  T1 cargo build --workspace
PASS  T2 cargo clippy --workspace --all-targets -D warnings
PASS  T2b cargo fmt --all --check (совпадает с ci.yml)
PASS  T3 venue-binance: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T3 venue-binance-futures: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T3 ожидаемый дефолт в оракуле не подменён: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T3 ожидаемый дефолт в оракуле не подменён: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
PASS  T4 venue-binance: allow-list оракул GREEN (23 тестов; негативный и регистровый запиннены поимённо)
PASS  T4 venue-binance-futures: allow-list оракул GREEN (21 тестов; негативный и регистровый запиннены поимённо)
PASS  T5 venue-binance: единственный вызов l2delta_event — внутри l2delta_emission_for
PASS  T5 venue-binance-futures: единственный вызов l2delta_event — внутри l2delta_emission_for
PASS  T5 хардкод-списка тикеров в venue-src нет
PASS  T5b venue-binance: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T5b venue-binance-futures: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T6 venue-binance: оракул сырого захвата red_l2delta_capture (M-18/CT-RFC-04) GREEN
PASS  T6 venue-binance-futures: оракул сырого захвата red_l2delta_futures (M-18/CT-RFC-04) GREEN
PASS  T7 crates/contracts/** не тронут
PASS  T8 DET-I-1 GREEN на смешанном журнале (снапшот+дельта; O-5 исполнен поимённо)
PASS  T9 раскатка исполнена И эпоха 'own-2026-09-m45-ethusdt' стоит В ЯЧЕЙКЕ ФАКТА (E-002)
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT EPOCH_ID=own-2026-09-m45-ethusdt)
PASS  T10b состав и эпоха внесены ОДНИМ коммитом (f3b84d41)
PASS  T10c мутация состава: 9 миров compose (каждый под setup-guard'ом) + 7 сценариев значений через ТОТ ЖЕ CLI, что и T10
VERDICT: PASS
exit=0

$ sed -n '177p' crates/venue-binance/tests/red_l2delta_allowlist.rs
fn adversarial_renamed_negative_path() {

$ bash scripts/verify_M-45.sh  # реальный o2 переименован; старое имя оставлено ТОЛЬКО комментарием
FAIL  T4 venue-binance: allow-list оракул КРАСНЫЙ, либо прогнано НОЛЬ тестов, либо снят поимённо запиннутый негативный (o2_symbol_outside_allowlist_is_not_captured) или регистровый (o4_config_case_does_not_silently_disable_capture) сценарий
VERDICT: FAIL (1 нарушений)
t4_comment_anchor_mutation_exit=1

$ git diff --quiet -- crates/venue-binance/tests/red_l2delta_allowlist.rs
restore_diff_exit=0
```

The baseline proves the stricter anchors do not reject either honest oracle; the adversarial
mutation proves the old comment-only escape fails. Per `A-031` §3 p.4, this is **CLEARED**,
not a new critic round.

## Done Block

```text
$ git status --porcelain

$ git log -1 --oneline
8c3d4ec gate(M-45): C-205 — носитель №9, рецидив №6 у автора его же фикса [architect]
```
