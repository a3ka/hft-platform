<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: 1adb7ac790a7830e04eb53f68fdc2499ddeec464
audited_head: 0a42885babfc7bd89877c6fc0cf991c59bb04dbc
verdict: REJECT
-->

# C-227 — M-70: C-226 B-1 closure is incomplete

## Verdict: REJECT

Scope is deliberately limited to closing `C-226` B-1 at `0a42885`. The new
predicate rejects every mutation explicitly claimed by rev12.1, but it still
accepts a wide *effective* production default when text follows the parsed
interpolation.

## B-1 remains open — the predicate does not accept only `0.001`

`scripts/verify_M-70.sh:367` extracts only the contents of
`${GATEWAY_BANDS:-…}` and its `.*` suffix discards everything after `}`. Thus
this valid Compose scalar passes the exact task-#7 guard:

```yaml
GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001},0.02
```

With `GATEWAY_BANDS` unset, Compose renders that line as
`GATEWAY_BANDS: 0.001,0.02`: a wide default before the R-185 hollow-window
measurement. This is the same unsafe class as C-226's original counterexample,
not a formatting-only variation. The guard nevertheless reports that the
default is exactly narrow because it compares the captured substring `0.001`
rather than the complete environment value.

**Condition to clear:** the task-#7 acceptance guard must reject the exact
suffix mutation above, while retaining the positive `0.001` control. Record
that mutation as a regression case in the architect-owned acceptance artifact.

## Requested limited checks

- The declared controls were executed against the actual guard body
  (`verify_M-70.sh:344-378`): `0.001,0.02,0.04`, `0.02`, the seven canonical
  values, a hard-coded literal, a whitespace-default, and an empty default all
  returned FAIL; the exact `0.001` and quoted exact scalar returned PASS.
  A `GATEWAY_BANDS` decoy outside `gateway-serve` also returned FAIL. None
  cures the suffix bypass above.
- The four named historical carriers are retained as struck history:
  `verify_M-70.sh:40`, the A-031 row at
  `milestones/M-70-depth-bands-enablement.md:922`, the checkpoint default at
  `:977`, and Objective at `:365`. The class grep also found the already struck
  task-table history at `:815`; it is not an unmarked fifth current requirement.
  The current requirement agrees across the four carriers: delivery is wired;
  the production default is exactly `0.001` until the separate operator action.
- `R-185` supplies the live reason this remains a blocker: after resync, a
  distant update can make `liveness=confirmed` describe understated values
  (`R-185-m70-enablement-second-opinion.md` §1). A broadened default therefore
  enables a known false output, rather than merely changing configuration.
- The subject range `1adb7ac..0a42885` contains only the task-#7 guard and
  milestone artifact changes; `git diff --check` passed. The full subject
  acceptance script itself reports PASS, exit 0, but does not exercise the
  suffix form and so cannot clear this finding.

## Done Block

```text
$ git log --oneline 1adb7ac..0a42885
0a42885 spec(M-70): rev12.1 — ЧЕТЫРЕ носителя старого требования, четвёртый нашёл греп класса [architect]
9ff366a gate(M-70): C-226 B-1 — предикат task #7 ловит КЛАСС, а не экземпляр [architect]
exit=0

$ git diff --name-status 1adb7ac..0a42885; git diff --check 1adb7ac..0a42885
M	milestones/M-70-depth-bands-enablement.md
M	scripts/verify_M-70.sh
exit=0

$ # Exact task-#7 predicate from scripts/verify_M-70.sh:344-378;
$ # candidates were supplied via FD 3, without modifying the subject worktree.
positive-exact                         PASS  exit=0
counterexample-0.001,0.02,0.04         FAIL  exit=1
single-wide-0.02                       FAIL  exit=1
canonical-seven                        FAIL  exit=1
hardcoded-literal                      FAIL  exit=1
default-with-space                     FAIL  exit=1
quoted-exact                           PASS  exit=0
empty-default                          FAIL  exit=1
outside-gateway-serve-only             FAIL  exit=1
suffix-wide-after-interpolation         PASS  exit=0
PASS: task #7 доставка состава под оракулом DB-I-7, прод-дефолт РОВНО 0.001, ручка переопределяема
CASE_RESULT fail=0
exit=0

$ env -u GATEWAY_BANDS GATEWAY_JWT_SECRET=critic docker compose -f /dev/fd/3 config
      GATEWAY_BANDS: 0.001,0.02
docker_compose_exit=0

$ rg -n '~~Включить канонический набор|~~«канонический состав ДОЕХАЛ|~~СТРОКА ключа YAML|~~дефолт `--bands`|~~Состав `GATEWAY_BANDS`|~~канонический состав ДОЕХАЛ' milestones/M-70-depth-bands-enablement.md scripts/verify_M-70.sh
scripts/verify_M-70.sh:40:# | task #7     | ~~канонический состав ДОЕХАЛ до сервиса~~ **СНЯТО rev12** — теперь: доставка
milestones/M-70-depth-bands-enablement.md:365:**ПЕРЕОПРЕДЕЛЕНА rev12.1.** ~~Включить канонический набор полос глубины в прод-выдачу~~ —
milestones/M-70-depth-bands-enablement.md:815:| 7 | ~~Состав `GATEWAY_BANDS` меняется на канонический~~ **ПЕРЕОПРЕДЕЛЕНА rev12: прод-дефолт остаётся УЗКИМ**
milestones/M-70-depth-bands-enablement.md:922:| 4 | ~~«канонический состав ДОЕХАЛ до сервиса» (task #7)~~ **ТРЕБОВАНИЕ СНЯТО rev12** — действующее: «доставка работает, а прод-дефолт РОВНО узкий»
milestones/M-70-depth-bands-enablement.md:977:| `ce592f3` | ~~дефолт `--bands` у `gateway-checkpoint` → канонические семь~~ **ПЕРЕОПРЕДЕЛЕНО rev12 (`705c0aa`): оба блока идут через `${GATEWAY_BANDS:-0.001}`**, дефолт УЗКИЙ.
exit=0

$ bash scripts/verify_M-70.sh; echo exit=$?
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
PASS: оракул DB-I-7 (состав из окружения доезжает до селектора) (исполнено тестов: 4)
PASS: task #7 доставка состава под оракулом DB-I-7, прод-дефолт РОВНО 0.001, ручка переопределяема
PASS: существующие оракулы окна памяти (исполнено тестов: 2)
VERDICT: PASS
exit=0

$ bash scripts/next_artifact_id.sh C
C-227
exit=0

$ EVENT_NAME=push PUSH_BEFORE=0a42885babfc7bd89877c6fc0cf991c59bb04dbc PR_BASE_SHA='' bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 0a42885..HEAD не ввёл второй носитель под занятым идентификатором
artifact_ids_exit=0

$ EVENT_NAME=push PUSH_BEFORE=0a42885babfc7bd89877c6fc0cf991c59bb04dbc PR_BASE_SHA='' bash scripts/check_gate_meta.sh
── GATE-META: диапазон 0a42885b..HEAD, origin=a3ka/hft-platform
   якорь main-стороны НЕ применён (прод-форма merge-ref не подтверждена) — судится весь диапазон
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
gate_meta_exit=0
```
