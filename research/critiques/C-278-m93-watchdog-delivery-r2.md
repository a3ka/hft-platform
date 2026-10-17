<!-- GATE-META
milestone: M-93
audited_repo: a3ka/hft-platform
audited_base: 81132e10c0d51a2a9d8ccfe2d74dac991fe38d19
audited_head: ede3d989d651840ba4a6c220d8a0448067b7ffd0
verdict: REJECT
-->

# C-278 — M-93 watchdog delivery, round 2

## Verdict: REJECT

The branch tip named in the mandate was fetched and audited as the subject:
`origin/feat/M-93-watchdog-delivery` at
`ede3d989d651840ba4a6c220d8a0448067b7ffd0`.  It is unchanged at verdict time.

The full architect set is committed: M-93 milestone, RED delivery probe, M-93
acceptance gate, CI-map adversary, and the M-08 D9-deep extension.  There is no
`contracts/**`, `crates/**`, Cargo, T-contract, or trait-signature delta, so T1
artifacts are N/A rather than missing.  The relevant declared live invariant is
`OPS-I-10`: the watchdog must be actually deliverable/executable, not merely
declared or manually built.

`C-277` B-1's two requested discriminators are now present: under a controlled
temporary reference form, the honest delivery path gives 9/9; a hard-coded image
despite an `inspect` call makes `w2` fail with the foreign binary, and a printer
that names the new path while normal mode writes the old path makes `w1b` fail.
Those are not reissued.  `C-277` B-2 is also closed: task 3 is assigned to
architect, `docs/ROADMAP.md` is in architect paths, the actual round-2 diff is
only the milestone and RED probe, and the milestone requires the full cycle plus
an independent `gates.md` §9 recheck for `deploy.yml`.

## Blocking finding

### B-3 — `w1b` has no green positive control in its own unprivileged setup

`w1b` unsets `HFT_WATCHDOG_DST` and invokes the real installer at the production
default path (`scripts/tests/red_m93_watchdog_delivery.sh:86-99`).  A conforming
fresh-host installer must create the parent of
`/usr/local/lib/hft/ops-watchdog` before its atomic copy.  In the actual test
environment the audit user is UID 1002 and `/usr/local/lib/hft` is absent.  The
reference form from M-93 §4 therefore stops at `mkdir -p /usr/local/lib/hft`
with exit 1 before it invokes the PATH-selected docker stub; `w1b` reports an
empty `cp` destination and the suite is 8/9, not 9/9.

This is a false-red setup failure, not a delivery invariant.  The test does not
establish the required positive control from `harness-track.md` §5(1), so it
cannot be accepted as the RED oracle that dev will turn GREEN.  A test-only
privilege seam makes the reference 9/9 and demonstrates that the two round-2
mutants are caught, but that seam is not in the committed probe; it is evidence
of the defect, not a substitute for the required committed setup guard.

Condition for the next audit: make the committed `w1b` self-contained under its
normal unprivileged invocation, such that an honest M-93 §4 delivery form reaches
the docker stub and passes all nine scenarios without a host-specific privilege
shim.  Retain and re-present the two round-2 mutations against that same
committed setup: hard-coded image → `w2` FAIL; print-new/write-old → `w1b` FAIL.

## Checks that passed

- B-2 ownership and path accounting are corrected as above; no `deploy.yml`
  implementation has been assigned to engine-dev.
- The existing unimplemented-branch baseline remains intentionally RED: only
  `w8` passes, and the probe exits 1.
- CI dry parity accounts for 57/57 CI `run:` steps and exits 0; the CI-map
  adversary passes all 10 worlds and exits 0.
- `verify_M-93.sh` is an explicit FAIL-counting aggregator; no prohibited
  `cmd && echo PASS || echo FAIL` form was found.  `bash -n` and diff whitespace
  checks pass.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-93-watchdog-delivery
ede3d989d651840ba4a6c220d8a0448067b7ffd0
exit=0

$ bash scripts/next_artifact_id.sh C
C-278
exit=0

$ bash scripts/tests/red_m93_watchdog_delivery.sh
FAIL  SETUP: deploy/bin/install-watchdog.sh отсутствует — доставки нет (TD-231)
FAIL  w1 композиция: установка печатает «», cron зовёт «/root/hft-platform/target/release/ops-watchdog» (обязаны совпасть, абсолютный путь вне target/)
FAIL  w1b путь по умолчанию в настоящем режиме: cp пишет в «», cron зовёт «/root/hft-platform/target/release/ops-watchdog» (exit=127)
FAIL  w2 установка: exit=98, create «», вызовы «», в dst: OLD
FAIL  w3 SETUP не состоялся (код 98)
FAIL  w4 SETUP не состоялся (код 98)
FAIL  w5 SETUP не состоялся (код 98)
FAIL  w6 Dockerfile: нет --bin ops-watchdog и/или COPY в /usr/local/bin/ops-watchdog
FAIL  w7 deploy.yml: установка сторожа — ветка healthy: 0, ветка отката: 0 (нужно ≥1 на обеих)
pass  w8 cron без бинаря: exit=1, ALERT в логе и файле тревоги
сценариев: 10   pass=1   FAIL=9
VERDICT: FAIL
exit=1  # expected RED before dev implementation

$ VERIFY_M93_CI_DRY=1 bash scripts/verify_M-93.sh
PASS  ci-parity: учтено шагов 57 из 57 (исполнено 50, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_verify_M-93_ci_map.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
PASS  строка дописана в блок базы ⇒ FAIL (exit=1)
PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
PASS  (i) check_review_fa + дописка ⇒ исполняется, не SKIP (exit=0)
PASS  (ii) run: |- — литеральный блок, исполняется (exit=0)
PASS  (iii) run: > — складывающий скаляр ⇒ FAIL (exit=1)
VERDICT: PASS — 10 сценариев
exit=0

$ bash -n scripts/tests/red_m93_watchdog_delivery.sh scripts/tests/red_verify_M-93_ci_map.sh scripts/verify_M-93.sh scripts/verify_delivery_M-08.sh
exit=0

$ git diff --check origin/main...HEAD
exit=0

$ test -d /usr/local/lib/hft; echo "dir=$?"; id -u
dir=1
1002

$ bash scripts/tests/red_m93_watchdog_delivery.sh  # temporary M-93 §4 reference form, native test setup
pass  w1 композиция: установка печатает и cron зовёт один путь (/usr/local/lib/hft/ops-watchdog), вне target/
FAIL  w1b путь по умолчанию в настоящем режиме: cp пишет в «», cron зовёт «/usr/local/lib/hft/ops-watchdog» (exit=1)
pass  w2 установка: бинарь образа работающего hft-recorder, исполняемый, без хвостов
pass  w3 отказ «cp»: exit=1, прежний бинарь цел, хвостов нет
pass  w4 отказ «inspect»: exit=1, прежний бинарь цел, хвостов нет
pass  w5 отказ «empty»: exit=1, прежний бинарь цел, хвостов нет
pass  w6 Dockerfile: ops-watchdog собирается и копируется в /usr/local/bin
pass  w7 deploy.yml: установка сторожа на ветке healthy и на ветке отката
pass  w8 cron без бинаря: exit=1, ALERT в логе и файле тревоги
сценариев: 9   pass=8   FAIL=1
VERDICT: FAIL
exit=1

$ PATH=<temporary privilege seam> bash scripts/tests/red_m93_watchdog_delivery.sh  # same temporary reference
сценариев: 9   pass=9   FAIL=0
VERDICT: PASS
exit=0

$ PATH=<temporary privilege seam> bash scripts/tests/red_m93_watchdog_delivery.sh  # inspect called, image hard-coded
FAIL  w2 установка: exit=0, create «sha256:run-hard-coded», вызовы «inspect create cp rm », в dst: #!/bin/sh echo FOREIGN-watchdog
сценариев: 9   pass=8   FAIL=1
VERDICT: FAIL
exit=1

$ PATH=<temporary privilege seam> bash scripts/tests/red_m93_watchdog_delivery.sh  # print new path, write old target path
FAIL  w1b путь по умолчанию в настоящем режиме: cp пишет в «/root/hft-platform/target/release/ops-watchdog.new.<pid>», cron зовёт «/usr/local/lib/hft/ops-watchdog» (exit=1)
сценариев: 9   pass=8   FAIL=1
VERDICT: FAIL
exit=1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-03T18:25Z
- Milestone: M-93-watchdog-delivery
- Статус: BLOCKED (REJECT)
- HEAD: ede3d98 — docs(M-93): спека круг 2 — задача 3 (deploy.yml) → architect, ROADMAP в путях, w1b/w2 в таблице, журнал кругов C-277 [architect]

## §B — Что я сделал
- Audited the committed branch tip, complete artifact set, C-277 corrections, and task-3 ownership.
- Built the specified temporary reference form and both requested round-2 mutants; retained their raw outcomes above.

## §C — Артефакты / результаты
- `research/critiques/C-278-m93-watchdog-delivery-r2.md`
- Done Block: expected branch RED exit=1; CI dry-parity exit=0; CI-map adversary exit=0; syntax and diff checks exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Resolve C-278 B-3 on origin/feat/M-93-watchdog-delivery.  Keep the C-277 round-2 image and real-default-path discriminators.  Make the committed w1b oracle self-contained for an unprivileged normal invocation, so an honest M-93 §4 delivery form reaches the docker stub and passes 9/9 without a host-specific privilege shim.  Re-run both mutations in that same committed setup: inspect-called/hard-coded image must fail w2; print-new/write-old must fail w1b.  Commit and push only the architect-owned M-93 artifacts, then request a fresh critic audit.
  ```
- Push-статус: ✅ pushed to `origin/feat/M-93-watchdog-delivery` with this verdict commit.
- Кэш: ⏸ кэш оставлен — temporary audit worktrees retain mutation evidence until this gate handoff is consumed.

## §E — Риски / открытые вопросы
- A permanently red `w1b` can be dismissed as environment noise and leaves the default-path assertion without a usable positive control.

=== END HANDOFF ===
