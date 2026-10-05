<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: d9e015bdb9598bc6aba87bc212aa200be25715d6
verdict: REJECT
-->

# C-197 — M-45 rollout signature: executor and subject lock are both unresolved

**Verdict: REJECT.** The T10/T10b oracle added for C-195 B-2 is a real, discriminating
gate: it fails on the current compose, passes only when both rollout variables are on
`hft-recorder` in one commit, and fails again when their history is split. The two output
blocks corrected after R-165 also reproduce byte-for-byte.

However, C-195 B-1 is not closed in the committed subject artifact set. The remote `main`
does contain `2e63a37` (PR #135), which grants `engine-dev` the root
`docker-compose.yml` operator-handles carve-out. But `2e63a37` is not an ancestor of the
audited subject head `d9e015b`; their merge-base remains `d77398d`. The audited
`scope-guard.md` still ends the engine-dev deployment zone at `deploy/**`, and task 7's
assignee is still explicitly **not assigned**. A rule that exists only on the other side
of an unmerged divergence cannot authorize dispatch from this artifact set.

Independently, the GATE-META barrier rejects the same range because the task-7 verify
change followed the passing `R-162` NOTE without an `ALLOW-SUBJECT-CHANGE` audit trail.

## B-1 — task 7 has no authorized executor in `d9e015b`

`M-45` §3ter itself says that, after the scope rule reaches `main`, the same round fills
the task's “who” cell with `engine-dev`. At `d9e015b` neither condition is present:

- `git merge-base --is-ancestor 2e63a37 d9e015b` exits 1;
- the audited scope table does not include root `docker-compose.yml`;
- task 7 says `⛔ исполнитель НЕ НАЗНАЧЕН — §3ter`.

This is not a request to change the global rule again: its merged form is correct and
allows only operator handles of the role's own services, while preserving the Boundary-C
signature requirement. It is a committed-artifact failure. Until the subject incorporates
that rule and its task row names `engine-dev`, dispatch would rely on a future merge rather
than the milestone it is asked to execute.

## B-2 — the accepted subject was changed without the mandatory subject-lock trace

`bash scripts/check_gate_meta.sh d77398d` exits 1. It identifies `R-162` as the latest
passing verdict (`NOTE`) and `scripts/verify_M-45.sh` as the subsequently changed protected
class. The task-7 oracle was needed and is good (see below), but adding it changed the
accepted subject. C-195's later REJECT does not erase that history. The range contains no
`ALLOW-SUBJECT-CHANGE: <reason>` line, so the mechanism correctly remains red.

Architect must add an explicit, truthful `ALLOW-SUBJECT-CHANGE` trailer to the commit range
when closing this round; the reason must name the C-195 B-2 task-7 acceptance change. This
is a trace of why a passing subject changed, not permission to weaken T10/T10b.

## C-195 B-2 — closed and mutation-checked

The new config-boundary oracle is not placebo:

- baseline `d9e015b`: both variables are absent on `hft-recorder`; T10 fails, as an
  unexecuted rollout must;
- isolated commit `de05bd23`: adding both `L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT` and
  an explicit `EPOCH_ID` on that service makes T10 and T10b pass;
- isolated history `056db2fa` then `4ca0f135`: the same final YAML makes T10 pass but T10b
  fail because the latest key-introducing commits differ.

Thus T10 observes the real YAML service boundary and T10b observes the required atomic
history, rather than only documentation text. T1 remains unchanged; the committed T2/T3
entry points, RED suites, DET-I-1 fixture, verify script, and milestone file are present.

## Other checked requirements

- The corrected §3ter log block and the R-165 F-2 addressable `sed` block in
  `docs/plans/scope-check-m45-m70-2026-08-31.md` each match their shown command exactly
  (`diff` exit 0).
- `bash scripts/verify_design_claims.sh --merge-preview origin/main` passes against current
  `origin/main` `2e63a37`; this verifies document links on the merge tree, not the missing
  executor declaration in the audited commit.
- The relevant live invariants remain **VN-I-3** (venue-specific branching is confined to
  adapters) and **BK-I-2** (a gap becomes `Stale` synchronously before any next event).
  The rollout must preserve the former's shared spot/futures configuration behavior; the
  later is why the separate future anchor/resync work discussed in §4bis cannot be silently
  folded into this rollout.

## Condition to remove REJECT

1. Incorporate `origin/main` `2e63a37` (or its equivalent approved scope commit) into the
   subject branch.
2. In the same architect-owned milestone update, replace task 7's unassigned-role cell with
   `engine-dev`, update §3ter's pre-merge wording, and add a truthful
   `ALLOW-SUBJECT-CHANGE` audit line naming C-195 B-2's task-7 acceptance change.
3. Push that new subject head and request a new critic round. The task remains OPEN until an
   engine-dev performs the signed rollout; at that point T10/T10b must turn green on the
   real commit and the §8 deployment evidence is still required.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-197
artifact_id_exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=d9e015bdb9598bc6aba87bc212aa200be25715d6 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона d9e015b..HEAD не ввёл второй носитель под занятым идентификатором
artifact_ids_exit=0

$ git rev-parse origin/main origin/docs/M-45-rollout-signature
2e63a37e5bf454da69b0fbd69de28c043b4caf4c
d9e015bdb9598bc6aba87bc212aa200be25715d6
$ git merge-base origin/main origin/docs/M-45-rollout-signature
d77398d7b22396c452d2651e90498033186055dd
$ git merge-base --is-ancestor 2e63a37 d9e015b; echo exit=$?
exit=1

$ git show d9e015b:.claude/rules/scope-guard.md | grep -F '`docker-compose.yml`'; echo exit=$?
exit=1
$ git show 2e63a37:.claude/rules/scope-guard.md | grep -F '`docker-compose.yml`' | grep -o '`docker-compose.yml`'; echo exit=${PIPESTATUS[0]}
`docker-compose.yml`
exit=0
$ git show d9e015b:milestones/M-45-persist-l2delta.md | grep -F 'исполнитель НЕ НАЗНАЧЕН'
| 7 | **Раскатка:** объявить `L2DELTA_CAPTURE_SYMBOLS` и `EPOCH_ID` на сервисе `hft-recorder` в корневом `docker-compose.yml` ОДНИМ коммитом, деплой-гейт `gates.md` §8 с sanity по КАЖДОЙ площадке отдельно | **⛔ исполнитель НЕ НАЗНАЧЕН — §3ter** | ⏳ OPEN | **`T10`+`T10b`** в `verify_M-45.sh` |

$ bash scripts/verify_M-45.sh 2>&1 | grep -E '^--- T10|^(PASS|FAIL).*T10|^VERDICT'; printf 'gate_exit=%s\n' "${PIPESTATUS[0]}"
--- T10: задача 7 (РАСКАТКА) — обе переменные на сервисе recorder, ОДНИМ коммитом ---
FAIL  T10 задача 7 НЕ исполнена — ОТСУТСТВУЮТ на сервисе recorder: L2DELTA_CAPTURE_SYMBOLS, EPOCH_ID
VERDICT: FAIL (1 нарушений)
gate_exit=1

$ (cd /tmp/hft-critic-m45-t10probe-1788210824 && bash scripts/verify_M-45.sh) 2>&1 | grep -E '^--- T10|^(PASS|FAIL).*T10|^VERDICT'; printf 'gate_exit=%s\n' "${PIPESTATUS[0]}"
--- T10: задача 7 (РАСКАТКА) — обе переменные на сервисе recorder, ОДНИМ коммитом ---
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT EPOCH_ID=own-2026-08-m45-ethusdt)
PASS  T10b состав и эпоха внесены ОДНИМ коммитом (de05bd23)
VERDICT: PASS
gate_exit=0

$ (cd /tmp/hft-critic-m45-t10split-1788210824 && bash scripts/verify_M-45.sh) 2>&1 | grep -E '^--- T10|^(PASS|FAIL).*T10|^VERDICT'; printf 'gate_exit=%s\n' "${PIPESTATUS[0]}"
--- T10: задача 7 (РАСКАТКА) — обе переменные на сервисе recorder, ОДНИМ коммитом ---
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT EPOCH_ID=own-2026-08-m45-ethusdt)
FAIL  T10b состав и эпоха внесены РАЗНЫМИ коммитами (056db2fa против 4ca0f135) — между ними события двух составов пишутся под одним epoch_id (класс E-001)
VERDICT: FAIL (1 нарушений)
gate_exit=1

$ git diff --name-only d77398d..d9e015b -- crates/contracts contracts
exit=0
$ rg -n 'pub (fn parse_capture_symbols|fn should_capture_l2delta|fn l2delta_emission_for|struct FuturesSession|enum SessionEffect|struct SpotSession)' crates/venue-binance/src/lib.rs crates/venue-binance-futures/src/lib.rs
crates/venue-binance/src/lib.rs:131:pub enum SessionEffect {
crates/venue-binance/src/lib.rs:141:pub struct SpotSession {
crates/venue-binance/src/lib.rs:778:pub fn parse_capture_symbols(raw: Option<&str>) -> Vec<String> {
crates/venue-binance/src/lib.rs:796:pub fn should_capture_l2delta(symbols: &[String], symbol: &str) -> bool {
crates/venue-binance/src/lib.rs:807:pub fn l2delta_emission_for(
crates/venue-binance-futures/src/lib.rs:464:pub fn parse_capture_symbols(raw: Option<&str>) -> Vec<String> {
crates/venue-binance-futures/src/lib.rs:482:pub fn should_capture_l2delta(symbols: &[String], symbol: &str) -> bool {
crates/venue-binance-futures/src/lib.rs:493:pub fn l2delta_emission_for(stream: &str, data: &Value, symbols: &[String]) -> Option<EventKind> {
crates/venue-binance-futures/src/lib.rs:544:pub enum SessionEffect {
crates/venue-binance-futures/src/lib.rs:567:pub struct FuturesSession {
exit=0
$ test -f crates/venue-binance/tests/red_l2delta_allowlist.rs && test -f crates/venue-binance-futures/tests/red_l2delta_allowlist.rs && test -f crates/journal/tests/red_det_replay_digest.rs && test -f scripts/verify_M-45.sh && test -f milestones/M-45-persist-l2delta.md
exit=0
$ bash -n scripts/verify_M-45.sh
exit=0

$ diff -u <(sed -n '130,137p' milestones/M-45-persist-l2delta.md) <(git log --format='%h %cs %s' --no-merges -- docker-compose.yml | grep -F '[engine-dev]'); echo exit=$?
exit=0
$ diff -u <(sed -n '34,37p' docs/plans/scope-check-m45-m70-2026-08-31.md) <(sed -n '374p;388p' crates/recorder/src/main.rs; sed -n '28,29p' docker-compose.yml); echo exit=$?
exit=0

$ git diff --check d77398d..d9e015b
exit=0
$ bash scripts/check_gate_meta.sh d77398d7b22396c452d2651e90498033186055dd
── GATE-META: диапазон d77398d7..HEAD, origin=a3ka/hft-platform
FAIL  research/reviews/R-162-decisions-recheck-r4.md: subject-lock — после проходного вердикта (NOTE) тронут класс «гейт»: scripts/verify_M-45.sh
      выход из лока — строка «ALLOW-SUBJECT-CHANGE: <причина>» в теле коммита диапазона
VERDICT: FAIL (1) — вердикт не привязан к предмету либо merge прошёл без вердикта.
gate_meta_exit=1
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -1
VERDICT: PASS (0 нарушений)
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31T21:29Z
- Milestone: M-45-persist-l2delta
- Статус: BLOCKED
- HEAD: d9e015b — spec(M-45): R-165 Ф-1 — мой же фикс был дефектен; вывод теперь производится командой [architect]

## §B — Что я сделал
- Аудировал committed artifact set `d77398d..d9e015b`, включая T-контрактный дифф, T2/T3 entry points, RED suites, M-45 verify, milestone, P-026/P-027, E-002, FA venues/book, C-195 and R-159…R-162.
- Воспроизвёл T10/T10b на базовом, атомарном и разнесённом history состояниях в отдельных detached worktrees.
- Сверил исправленные R-165 output blocks через `diff` с выводом указанных команд.

## §C — Артефакты / результаты
- `research/critiques/C-197-m45-rollout-signature-r2.md`
- Done Block: baseline T10 exit=1; atomic T10/T10b exit=0; split T10b exit=1; gate-meta exit=1; merge-preview exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Устрани REJECT C-197 на M-45. Предмет — committed head d9e015b не содержит scope commit 2e63a37 и task 7 всё ещё имеет «исполнитель НЕ НАЗНАЧЕН»; кроме того, `check_gate_meta.sh d77398d` красный: после NOTE R-162 изменён scripts/verify_M-45.sh, но нет ALLOW-SUBJECT-CHANGE. Внеси current origin/main (или тот же approved scope commit) в subject branch, затем architect-only правкой M-45 назови engine-dev исполнителем задачи 7 и приведи §3ter к этому состоянию. В теле этого commit-range добавь правдивый `ALLOW-SUBJECT-CHANGE`, называющий C-195 B-2 task-7 acceptance change. Не меняй П-026 и не ослабляй T10/T10b. Запушь новый head и запроси новый critic-круг; rollout пока не выполняй.
  ```
- Push-статус: ⏸ verdict commit is prepared locally; next step is explicit push to `origin/docs/M-45-rollout-signature` after the commit.
- Кэш: ⏸ probe worktrees retain cargo caches until this gate artifact is committed and pushed.

## §E — Риски / открытые вопросы
- Task 7 must not be dispatched before the subject carries scope authority, the named role, and a green subject-lock.
- The real rollout still requires P-026, T10/T10b GREEN on its actual commit, and the §8 deployment/sanity evidence for spot and futures separately.

=== END HANDOFF ===
