<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: 2e63a37e5bf454da69b0fbd69de28c043b4caf4c
audited_head: fa0c98066870b1f4c4933801dd1fb500bef21dc8
verdict: REJECT
-->

# C-202 — M-45 rollout signature, round 4: Boundary-C gate is bypassable and its replay evidence remains false

**Verdict: REJECT.** C-199's three literal static-history cases are now correct: a
one-commit extra `SOLUSDT` fails T10 with `ЛИШНИЕ`; the signed one-commit pair passes the
whole gate; the two-commit history fails T10b. C-197 B-1/B-2, `П-026`, and R-162 §5 are
accepted as closed and are not reopened.

The claimed B-3/B-4 closure fails on the effective configuration, on the anti-placebo
probe, and on raw-command reproducibility. Do not dispatch task 7.

## B-1 — BLOCKER: T10 validates YAML spelling, not the effective recorder configuration

The signed-looking form `${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT}` passes T10 even when
the caller supplies `BTCUSDT,ETHUSDT,SOLUSDT`. `docker compose config` gives the recorder
the three-symbol value. This is an unsigned expansion of the exact P-026 set and the whole
verify gate exits 0.

**Condition:** validate the effective release configuration (including interpolation), or
make external override impossible and prove that policy. The effective-SOLUSDT probe must
be red.

## B-2 — BLOCKER: T10c is an independent copied comparator, not a mutation of T10

T10c redefines `compose_default` and the set comparison instead of executing T10. Changing
only T10 from `if got != SIGNED:` to `if not got:` makes static three-symbol rollout green;
the ten T10c scenarios stay green. Thus its stated anti-placebo property is false.

**Condition:** invoke one authoritative parser/comparator from T10 and its probe, or mutate
and execute the real T10 path. A weakened real T10 comparison must fail the relevant probe.

## B-3 — BLOCKER: a valid unset-only Compose default is a false red absent from T10c

`${L2DELTA_CAPTURE_SYMBOLS-BTCUSDT,ETHUSDT}` resolves to the exact signed set when unset,
but T10 calls it both extra and missing. T10c omits this Compose form despite claiming to
cover both substitution forms.

**Condition:** support this honest form, or explicitly prohibit it as checked rollout
policy; in either case cover the policy in T10c.

## B-4 — BLOCKER: the refreshed scope-check still contains three non-reproducible claims

The `ls-remote` block records `1bbb3b8` but the identical live command returns `fa0c980`.
Its raw recorder/compose block contains five narrative `# Правка ...` lines that `sed; sed`
cannot print. Finally, it declares collection in `origin/main=d77398d`, where its task-count
command prints 8, not displayed 9; 9 is available only on the subject revision.

Five other scope-check/M-45 §3ter pairs reproduce. These three are the same C-199 B-4 class:
output is either edited or silently sourced from another revision.

## B-5 — BLOCKER: fa0c980 adds the same transcript defect

The post-audit commit calls its six depth-plan pairs identical and says comments were moved
outside raw output. In its declared `d77398d` checkout, the sixth displayed `grep | wc -l`
output has `15` plus an indented comment; the command produces only `15`.

## Required disposition

C-199 B-4 and B-4 above are a second REJECT for the same cause. `gates.md` §0 and the
critic profile require a fresh-context **arbiter**, not a third architect↔critic loop. The
arbiter decides the immutable historical-fact form and the necessary T10/T10c repair;
implementation remains architect-owned. No new Boundary-C choice is requested: P-026 is
enforced exactly as signed.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-202
allocator_exit=0

$ cd /tmp/hft-critic-m45-r4-extra-pristine-1788219117
$ set -o pipefail; env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-extra-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'exit=%s\n' "$rc"
FAIL  T10 задача 7 НЕ исполнена — L2DELTA_CAPTURE_SYMBOLS='BTCUSDT,ETHUSDT,SOLUSDT' не равен подписанному множеству ['BTCUSDT', 'ETHUSDT'] — ЛИШНИЕ (неподписанное расширение границы C): SOLUSDT
PASS  T10c мутация состава: 10 сценариев, все различены
VERDICT: FAIL (1 нарушений)
exit=1

$ cd /tmp/hft-critic-m45-r4-atomic-pristine-1788219117
$ set -o pipefail; env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-atomic-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'exit=%s\n' "$rc"
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
$ git diff -- scripts/verify_M-45.sh | sed -n '1,13p'
-if got != SIGNED:
+if not got:
$ set -o pipefail; env -u L2DELTA_CAPTURE_SYMBOLS -u EPOCH_ID CARGO_TARGET_DIR=/tmp/hft-critic-m45-r4-extra-1788219117/target bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL)  T10|^VERDICT'; rc=${PIPESTATUS[0]}; printf 'verify_exit=%s\n' "$rc"
PASS  T10 обе переменные раскатки на сервисе recorder (OK L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT,SOLUSDT EPOCH_ID=own-2026-08-m45-eth-sol)
PASS  T10b состав и эпоха внесены ОДНИМ коммитом (dabf6ef1)
PASS  T10c мутация состава: 10 сценариев, все различены
VERDICT: PASS
verify_exit=0

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

$ cd /tmp/hft-critic-m45-r4-facts-1788219117
$ diff -u <(git -C /tmp/hft-critic-m45-r4-publish-1788219117 show HEAD:docs/plans/scope-check-m45-m70-2026-08-31.md | sed -n '36,44p') <(sed -n '374p;388p' crates/recorder/src/main.rs; sed -n '28,29p' docker-compose.yml)
@@ -2,8 +2,3 @@
-# Правка 2026-08-31 (класс R-164 Б-1): прежняя редакция показывала `grep … | head -4` с
-# выпиской, которую эта команда не даёт — реальный `head -4` отдаёт строки compose плюс
-# main.rs:6 и :312 (комментарии), а не :374/:388. Факты под выпиской были ВЕРНЫ, но
-# предъявлены командой, их не производящей. Заменено адресным `sed -n`, дающим ровно
-# показанное. Класс найден грепом по всем носителям, а не по названному месту.
exit=1
$ diff -u <(git -C /tmp/hft-critic-m45-r4-publish-1788219117 show HEAD:docs/plans/scope-check-m45-m70-2026-08-31.md | sed -n '51p') <(grep -c '^| [0-9]' milestones/M-45-persist-l2delta.md)
@@ -1 +1 @@
-9
+8
exit=1
$ six depth-plan pairs in declared d77398d checkout: pairs 1–5 exit=0; pair 6 diff
@@ -1,2 +1 @@
 15
-                    # укрупнения разрешения ПО ГЛУБИНЕ в корпусе НЕТ
exit=1

$ cd /tmp/hft-critic-m45-r4-publish-1788219117
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
- Дата (UTC, ISO-8601): 2026-09-01T00:15Z
- Milestone: M-45-persist-l2delta
- Статус: BLOCKED
- HEAD: fa0c980 — docs(depth-delivery): шестое срабатывание класса выписок — найдено СВОЕЙ сверкой [architect]

## §B — Что я сделал
- Аудировал committed artifact set `2e63a37..fa0c980`; принятые C-197 B-1/B-2, П-026 и R-162 §5 не пересуждал.
- Воспроизвёл три требуемые истории T10/T10b и независимые effective-config, false-red и T10c-drift пробы.

## §C — Артефакты / результаты
- `research/critiques/C-202-m45-rollout-signature-r4.md`
- Done Block: signed atomic gate exit=0; static-extra/split/fallback subject probes exit=1; effective unsigned override and weakened T10 exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter` (fresh context, strong model; second same-cause REJECT trigger)
- **Paste-ready промпт:**
  ```
  Арбитраж M-45, fresh context. Прочитай C-199 и C-202 целиком, scripts/verify_M-45.sh,
  docs/plans/scope-check-m45-m70-2026-08-31.md, docs/plans/depth-delivery-architecture-2026-08-31.md,
  M-45 §3quater/§3ter, gates.md §0 и harness-track. Реши: (1) повторяет ли B-4 C-199;
  (2) удерживает ли T10 П-026 при override ${VAR:-default}; (3) засчитывается ли T10c,
  который дублирует comparator. Вынеси обязательное решение и следующий технический шаг; код не пиши.
  ```
- Push-статус: pending explicit-path commit, artifact-id barrier and push.
- Кэш: critic-owned fixture caches will be removed after successful push.

## §E — Риски / открытые вопросы
- Task 7 remains OPEN; no rollout or deploy is authorized.
- П-026 is not reopened; enforcement and transcript reproducibility are the dispute.

=== END HANDOFF ===
