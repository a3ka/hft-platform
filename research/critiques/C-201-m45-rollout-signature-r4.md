<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: 2e63a37e5bf454da69b0fbd69de28c043b4caf4c
audited_head: fa0c98066870b1f4c4933801dd1fb500bef21dc8
verdict: REJECT
-->

# C-201 — M-45 rollout signature, круг 4: T10c is detached from T10; effective Compose can exceed the signature

**Verdict: REJECT.** The exact three C-199 B-3 history cases are now correct: an
unsigned static `SOLUSDT` addition fails with `ЛИШНИЕ`, the exact signed one-commit
configuration passes the whole gate, and split history fails T10b. C-197 B-1/B-2,
`П-026`, and `R-162` §5 are not reopened.

The claimed B-3 closure is nevertheless unsafe in two independent ways. T10 judges the
literal YAML default, not the effective Compose configuration, so a host environment can
expand the record beyond `П-026` while the entire verify gate passes. T10c also does not
exercise T10: it duplicates the comparator despite claiming to extract T10's body. A
one-line weakening of T10 leaves all ten T10c cases green and makes the full gate pass
with `SOLUSDT`. This is anti-placebo failure on the Boundary-C gate itself.

## B-1 — BLOCKER: T10 accepts an effective unsigned expansion through Compose interpolation

In an isolated one-commit fixture, the recorder has the apparently signed source form
`L2DELTA_CAPTURE_SYMBOLS: ${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT}` and an explicit
`EPOCH_ID`. With the host value `L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT,SOLUSDT`, T10
reads only the fallback text and passes; T10b and T10c also pass, so the full gate is
green. The same input to `docker compose config` produces the recorder environment with
`SOLUSDT`.

`П-026` signs exactly `{BTCUSDT, ETHUSDT}`. Its bound applies to the configuration that
the recorder receives, not to an unevaluated YAML token. The existing comment that the
operator "does not set" the variable is not a fail-closed check and cannot substitute for
the signed boundary.

**Condition to remove:** T10 must validate the effective release configuration (including
interpolation) against the exact signed set, or the rollout form must make such an override
impossible and prove that policy. The RED/mutation case must make the effective `SOLUSDT`
configuration red.

## B-2 — BLOCKER: T10c is a duplicated oracle, not a mutation of T10

T10c states that it extracts and runs the code from T10. It does neither: lines 328–352
define a separate `compose_default`, set comparison, and ten local cases. Mutating only
T10's comparison from `if got != SIGNED:` to `if not got:` in a detached fixture makes its
static three-symbol configuration pass T10 and T10b; T10c remains green because its copied
comparison is unchanged, and the complete verify script exits 0.

This directly contradicts the anti-dependent-reference requirement in `testing.md`: the
test can drift with the implementation it claims to pin. The ten current cases therefore
do not establish T10's distinguishing power.

**Condition to remove:** one authoritative parser/comparator must be invoked by both T10
and the probe, or T10c must mutate and execute the actual T10 path. Demonstrate that
weakening the actual T10 comparison makes the relevant T10c scenario fail.

## B-3 — BLOCKER: a valid unset-only Compose fallback is falsely rejected and absent from T10c

Compose supports both default forms. In an isolated fixture,
`${L2DELTA_CAPTURE_SYMBOLS-BTCUSDT,ETHUSDT}` resolves to the signed set when the variable is
unset (`docker compose config`, exit 0), but T10 reports both a fabricated extra token and
both signed symbols missing. T10c still passes because it has no case for this form; its
claim to cover “both forms of substitution” is false.

The gate must either support this honest effective configuration or explicitly prohibit it
as a checked rollout policy. It may not silently describe coverage it does not have.

## B-4 — BLOCKER: the refreshed scope-check still fails its own mechanical transcript rule

Five scope-check/M-45 §3ter command/output pairs match. Three do not. First, the refreshed
`git ls-remote` pair records C-199 head `1bbb3b8`, while the exact command on the audited
subject returns `fa0c980`; a live remote query makes its historical output unreproducible on
the very push that contains the claimed fix. Second, the displayed recorder/compose result
also contains five `# Правка ...` narrative lines inside the raw-output fence; its shown
`sed; sed` command cannot print them. Third, the document explicitly says its facts were
collected in the `origin/main=d77398d` checkout, but its M-45 task-count command prints `8`
there, not the displayed `9`; `9` is the result only on the subject revision with task 7.

These are the C-199 B-4 class rather than its closure: raw output is still being edited or
silently taken from a different revision.

Use an immutable command/ref for a historical fact, or label the result as historical rather
than claiming it is the current command's raw output. The replay must then be exact.

## B-5 — BLOCKER: the post-audit “sixth occurrence” commit adds the same raw-output defect

The subject advanced during this audit from `e4d1932` to `fa0c980`. The new committed
`docs/plans/depth-delivery-architecture-2026-08-31.md` says comments were moved outside
raw output and that all six pairs are identical. In the `d77398d` checkout declared by that
document, pairs 1–5 do reproduce. Pair 6 does not: its displayed output contains `15` **and**
an indented `# укрупнения ...` comment, whereas the displayed `grep | wc -l` command prints
only `15`. This is mechanically the same presentation defect in a block added by the alleged
fix.

This added commit does not repair B-4; it independently confirms that raw transcripts are
still being edited for narrative. The arbiter disposition below therefore remains mandatory.

## Confirmed artifacts and accepted predecessors

- No `contracts/` or `crates/contracts/` path is in `origin/main..HEAD`; T1 shape is unchanged.
- The committed T2/T3 signatures remain present in both Binance adapters; the two RED
  allow-list suites, `DET-I-1` fixture, verify script, and M-45 milestone all exist.
- The atomic fixture ran T0–T10c green; the audited subject's own verify is expected red
  only because task 7 is still OPEN and its two variables are absent.
- Live FA invariants checked on this revision: **VN-I-3** (venue-specific branching stays in
  adapters, `docs/fa/venues.md:176`) and **BK-I-2** (a sequence gap becomes `Stale`
  synchronously, `docs/fa/book.md:140`).

## Required disposition

Do not dispatch task 7. C-199 B-4 and this B-4 are two consecutive REJECTs for the same
cause — an alleged raw command output is not produced by that command — so `gates.md` §0 and
the critic profile require a fresh-context **arbiter**, not a third architect↔critic loop.
The arbiter must decide the reproducible historical-facts form and whether the T10/T10c
failures are a repair of C-199 B-3 or a distinct new class; the implementation remains
architect-owned after that decision.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-201
exit=0

$ cd /tmp/hft-critic-m45-r4-extra-pristine-1788219117
$ set -o pipefail; env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-extra-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'exit=%s\n' "$rc"
FAIL  T10 задача 7 НЕ исполнена — L2DELTA_CAPTURE_SYMBOLS='BTCUSDT,ETHUSDT,SOLUSDT' не равен подписанному множеству ['BTCUSDT', 'ETHUSDT'] — ЛИШНИЕ (неподписанное расширение границы C): SOLUSDT
PASS  T10c мутация состава: 10 сценариев, все различены
VERDICT: FAIL (1 нарушений)
exit=1

$ cd /tmp/hft-critic-m45-r4-atomic-pristine-1788219117
$ set -o pipefail; env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-atomic-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'; rc=${PIPESTATUS[0]}; printf 'exit=%s\n' "$rc"
PASS  T0 оракул присутствует: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T0 оракул присутствует: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
PASS  T1 cargo build --workspace
PASS  T2 cargo clippy --workspace --all-targets -D warnings
PASS  T2b cargo fmt --all --check (совпадает с ci.yml)
PASS  T3 venue-binance: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T3 venue-binance-futures: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T3 ожидаемый дефолт в оракуле не подменён: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T3 ожидаемый дефолт в оракуле не подменён: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
PASS  T4 venue-binance: allow-list оракул GREEN (23 тестов)
PASS  T4 venue-binance-futures: allow-list оракул GREEN (21 тестов)
PASS  T5 venue-binance: единственный вызов l2delta_event — внутри l2delta_emission_for
PASS  T5 venue-binance-futures: единственный вызов l2delta_event — внутри l2delta_emission_for
PASS  T5 хардкод-списка тикеров в venue-src нет
PASS  T5b venue-binance: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T5b venue-binance-futures: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T6 venue-binance: оракул сырого захвата (M-18/CT-RFC-04) остался GREEN
PASS  T6 venue-binance-futures: отдельного red_l2delta_capture нет (покрыт общим прогоном T7)
PASS  T7 crates/contracts/** не тронут
PASS  T8 DET-I-1 GREEN на смешанном журнале (снапшот+дельта)
PASS  T9 дефолтный состав не менялся ⇒ запись эпохи не требуется
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT EPOCH_ID=own-2026-08-m45-ethusdt)
PASS  T10b состав и эпоха внесены ОДНИМ коммитом (3ea31826)
PASS  T10c мутация состава: 10 сценариев, все различены
VERDICT: PASS
exit=0

$ cd /tmp/hft-critic-m45-r4-split-1788219117
$ set -o pipefail; env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-split-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'exit=%s\n' "$rc"
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT EPOCH_ID=own-2026-08-m45-ethusdt)
FAIL  T10b состав и эпоха внесены РАЗНЫМИ коммитами (76af681b против 8dcc60ee) — между ними события двух составов пишутся под одним epoch_id (класс E-001)
VERDICT: FAIL (1 нарушений)
exit=1

$ cd /tmp/hft-critic-m45-r4-atomic-1788219117
$ set -o pipefail; env EPOCH_ID=own-2026-08-m45-ethusdt L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT,SOLUSDT CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-atomic-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'verify_exit=%s\n' "$rc"
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT} EPOCH_ID=own-2026-08-m45-ethusdt-override-proof)
PASS  T10b состав и эпоха внесены ОДНИМ коммитом (1e3f46be)
PASS  T10c мутация состава: 10 сценариев, все различены
VERDICT: PASS
verify_exit=0

$ env L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT,SOLUSDT GATEWAY_JWT_SECRET=probe docker compose config | grep -F 'L2DELTA_CAPTURE_SYMBOLS:'; rc=$?; printf 'compose_grep_exit=%s\n' "$rc"
      L2DELTA_CAPTURE_SYMBOLS: BTCUSDT,ETHUSDT,SOLUSDT
compose_grep_exit=0

$ cd /tmp/hft-critic-m45-r4-extra-1788219117
$ set -o pipefail; env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-extra-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'verify_exit=%s\n' "$rc"
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT,SOLUSDT EPOCH_ID=own-2026-08-m45-eth-sol)
PASS  T10b состав и эпоха внесены ОДНИМ коммитом (dabf6ef1)
PASS  T10c мутация состава: 10 сценариев, все различены
VERDICT: PASS
verify_exit=0

$ git diff -- scripts/verify_M-45.sh | sed -n '1,13p'
diff --git a/scripts/verify_M-45.sh b/scripts/verify_M-45.sh
index c1bb95b..90422c8 100755
--- a/scripts/verify_M-45.sh
+++ b/scripts/verify_M-45.sh
@@ -261,7 +261,7 @@ def compose_default(raw: str) -> str:
     m = re.fullmatch(r"\$\{[A-Za-z_][A-Za-z0-9_]*:-(.*)\}", raw.strip())
     return m.group(1) if m else raw
 got = {t.strip().upper() for t in compose_default(sym).split(",") if t.strip()}
-if got != SIGNED:
+if not got:
mutation_diff_exit=0

$ cd /tmp/hft-critic-m45-r4-fallback-1788219117
$ env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID GATEWAY_JWT_SECRET=probe docker compose config | grep -F 'L2DELTA_CAPTURE_SYMBOLS:'; rc=$?; printf 'compose_grep_exit=%s\n' "$rc"
      L2DELTA_CAPTURE_SYMBOLS: BTCUSDT,ETHUSDT
compose_grep_exit=0
$ set -o pipefail; env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-atomic-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'verify_exit=%s\n' "$rc"
FAIL  T10 задача 7 НЕ исполнена — L2DELTA_CAPTURE_SYMBOLS='${L2DELTA_CAPTURE_SYMBOLS-BTCUSDT,ETHUSDT}' не равен подписанному множеству ['BTCUSDT', 'ETHUSDT'] — ЛИШНИЕ (неподписанное расширение границы C): ${L2DELTA_CAPTURE_SYMBOLS-BTCUSDT, ETHUSDT}; ОТСУТСТВУЮТ (сужение состава без подписи): BTCUSDT, ETHUSDT
PASS  T10c мутация состава: 10 сценариев, все различены
VERDICT: FAIL (1 нарушений)
verify_exit=1

$ diff -u <(sed -n '28,29p' docs/plans/scope-check-m45-m70-2026-08-31.md) <(git ls-remote --heads origin docs/M-45-rollout-signature docs/depth-delivery-architecture)
-1bbb3b88197c7229a6a0f16c021683bbb885e107 refs/heads/docs/M-45-rollout-signature
+fa0c98066870b1f4c4933801dd1fb500bef21dc8 refs/heads/docs/M-45-rollout-signature
exit=1

$ cd /tmp/hft-critic-m45-r4-facts-1788219117  # declared d77398d checkout
$ diff -u <(git -C /tmp/hft-codex-critic-m45-round4-1788219117 show HEAD:docs/plans/scope-check-m45-m70-2026-08-31.md | sed -n '36,44p') <(sed -n '374p;388p' crates/recorder/src/main.rs; sed -n '28,29p' docker-compose.yml)
--- /dev/fd/63
+++ /dev/fd/62
@@ -2,8 +2,3 @@
                 let syms = env_csv("BINANCE_FUTURES_SYMBOLS", &["BTCUSDT", "ETHUSDT"]);
       BINANCE_SYMBOLS: ${BINANCE_SYMBOLS:-BTCUSDT,ETHUSDT}
       BINANCE_FUTURES_SYMBOLS: ${BINANCE_FUTURES_SYMBOLS:-BTCUSDT,ETHUSDT}
-# Правка 2026-08-31 (класс R-164 Б-1): прежняя редакция показывала `grep … | head -4` с
-# выпиской, которую эта команда не даёт — реальный `head -4` отдаёт строки compose плюс
-# main.rs:6 и :312 (комментарии), а не :374/:388. Факты под выпиской были ВЕРНЫ, но
-# предъявлены командой, их не производящей. Заменено адресным `sed -n`, дающим ровно
-# показанное. Класс найден грепом по всем носителям, а не по названному месту.
exit=1

$ diff -u <(git -C /tmp/hft-codex-critic-m45-round4-1788219117 show HEAD:docs/plans/scope-check-m45-m70-2026-08-31.md | sed -n '51p') <(grep -c '^| [0-9]' milestones/M-45-persist-l2delta.md)
@@ -1 +1 @@
-9
+8
exit=1
$ git -C /tmp/hft-codex-critic-m45-round4-1788219117 show HEAD:milestones/M-45-persist-l2delta.md | grep -c '^| [0-9]'; rc=$?; printf 'subject_count_exit=%s\n' "$rc"
9
subject_count_exit=0

$ diff -u <(git -C /tmp/hft-codex-critic-m45-round4-1788219117 show HEAD:docs/plans/scope-check-m45-m70-2026-08-31.md | sed -n '31,34p') <(grep -n 'L2DELTA_CAPTURE_SYMBOLS' crates/venue-binance/src/lib.rs crates/venue-binance-futures/src/lib.rs)
exit=0
$ diff -u <(git -C /tmp/hft-codex-critic-m45-round4-1788219117 show HEAD:docs/plans/scope-check-m45-m70-2026-08-31.md | sed -n '46,49p') <(grep -n 'PROD_DEFAULT' crates/venue-binance/src/lib.rs crates/venue-binance-futures/src/lib.rs)
exit=0
$ diff -u <(git -C /tmp/hft-codex-critic-m45-round4-1788219117 show HEAD:docs/plans/scope-check-m45-m70-2026-08-31.md | sed -n '53p') <(grep -n 'состав записываемых данных' .claude/rules/gates.md)
exit=0
$ cd /tmp/hft-codex-critic-m45-round4-1788219117
$ diff -u <(sed -n '158,165p' milestones/M-45-persist-l2delta.md) <(git log --format='%h %cs %s' --no-merges -- docker-compose.yml | grep -F '[engine-dev]')
exit=0
$ diff -u <(sed -n '168p' milestones/M-45-persist-l2delta.md) <(git log --format='%s' --no-merges -- docker-compose.yml | grep -cF '[engine-dev]')
exit=0

$ six depth-delivery command/output diffs in its declared d77398d checkout
pairs 1–5: exit=0
pair 6:
--- displayed grep | wc -l output
+++ actual grep | wc -l output
@@ -1,2 +1 @@
 15
-                    # укрупнения разрешения ПО ГЛУБИНЕ в корпусе НЕТ
exit=1

$ cd /tmp/hft-codex-critic-m45-round4-1788219117
$ set -o pipefail; CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-atomic-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'exit=%s\n' "$rc"
FAIL  T10 задача 7 НЕ исполнена — ОТСУТСТВУЮТ на сервисе recorder: L2DELTA_CAPTURE_SYMBOLS, EPOCH_ID
PASS  T10c мутация состава: 10 сценариев, все различены
VERDICT: FAIL (1 нарушений)
exit=1

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 7, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
exit=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
$ git diff --check origin/main..HEAD
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-01T00:02Z
- Milestone: M-45-persist-l2delta
- Статус: BLOCKED
- HEAD: fa0c980 — docs(depth-delivery): шестое срабатывание класса выписок — найдено СВОЕЙ сверкой [architect]

## §B — Что я сделал
- Аудировал committed artifact set `2e63a37..fa0c980`; П-026, C-197 B-1/B-2 и R-162 §5 не пересуждал.
- Воспроизвёл три требуемые истории T10/T10b и независимые effective-config, false-red, and T10c-drift probes.

## §C — Артефакты / результаты
- `research/critiques/C-201-m45-rollout-signature-r4.md`
- Done Block: atomic verify exit=0; unsigned/split/false-red subject probes exit=1; effective unsigned override exit=0; gate-meta/design-claims/diff-check exit=0; new depth transcript pair 6 exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter` (fresh context, strong model; second same-cause REJECT trigger)
- **Paste-ready промпт:**
  ```
  Арбитраж M-45, fresh context. Прочитай C-199 и C-201 целиком, scripts/verify_M-45.sh,
  docs/plans/scope-check-m45-m70-2026-08-31.md, docs/plans/depth-delivery-architecture-2026-08-31.md,
  M-45 §3quater/§3ter, gates.md §0 and harness-track. Реши: (1) является ли live ls-remote
  transcript и новый raw-output с комментарием повтором C-199 B-4 и
  какая воспроизводимая форма исторической фактуры обязательна; (2) закрывает ли T10
  подпись П-026, если effective docker-compose config can override ${VAR:-default};
  (3) засчитывается ли T10c, который дублирует comparator instead of exercising T10.
  Вынеси обязательное решение и следующий технический шаг; код не пиши.
  ```
- Push-статус: ⏸ на момент записи verdict ожидает explicit-path commit и успешный push; без них гейт не завершён.
- Кэш: ⏸ fixture worktree caches retained until the verdict is pushed; only critic-owned fixture caches will then be removed.

## §E — Риски / открытые вопросы
- Task 7 remains OPEN; no rollout or deploy is authorized.
- П-026 is not a subject of this arbitration; only enforcement and reproducibility are disputed.

=== END HANDOFF ===
