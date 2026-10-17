<!-- GATE-META
milestone: M-93
audited_repo: a3ka/hft-platform
audited_base: 81132e10c0d51a2a9d8ccfe2d74dac991fe38d19
audited_head: 744e81501c5d40559d424cdde6f43d8ce75882c1
verdict: REJECT
-->

# C-277 — M-93 watchdog delivery

## Verdict: REJECT

Audited the committed artifact set on `origin/feat/M-93-watchdog-delivery`, not the
milestone prose alone.  The set is structurally present: milestone, RED probe,
verify script, CI-map probe, and D9-deep delta.  No T-contract or trait-signature
change is in the diff, so those artifacts are N/A.  `OPS-I-10` is the live relevant
FA invariant: a declared watchdog that never reaches the host is not a producer that
actually emits.

The baseline is correctly RED (`8/9`) and the two non-delivery gates pass, but two
blockers leave a green implementation path to an old/foreign host binary and give
task 3 to no permitted writer.

## Blocking findings

### B-1 — `w2` does not prove image identity flows from the running container

`scripts/tests/red_m93_watchdog_delivery.sh:33-39` makes every successful
`docker inspect` return the same literal `sha256:img0123`; `w2` then accepts only
the equally literal `create sha256:img0123` at lines 72-74.  A mutant which calls
`inspect hft-recorder` but ignores its result and always creates that stale/foreign
image passes the probe.  Thus the oracle does not establish I-2's required edge
`inspect(running hft-recorder).Image -> create -> cp`, and a deploy can be green
while `/usr/local/lib/hft/ops-watchdog` came from an old or unrelated image.

The same gap exists for I-1: `w1` observes only the installer print-only branch
(lines 59-66), while `w2`–`w5` force `HFT_WATCHDOG_DST` (line 54).  A normal-mode
default different from the printed default passes every scenario: cron names the
new path, but deployment writes the old one.

Reproduce from the committed probe:

```text
$ nl -ba scripts/tests/red_m93_watchdog_delivery.sh | sed -n '33,74p'
34             echo "sha256:img0123" ;;
35   create)  [ "${STUB_FAIL:-}" = create ] && exit 1; echo "cid0123" ;;
38             [ "$src" = "cid0123:/usr/local/bin/ops-watchdog" ] || { ...; }
53   ( PATH="$d/bin:$PATH" ... HFT_WATCHDOG_DST="$d/dst/ops-watchdog" bash "$INSTALL" ... )
60 dst="$( env -u HFT_WATCHDOG_DST HFT_INSTALL_WATCHDOG_PRINT_DST=1 bash "$INSTALL" ... )"
74    && grep -q '^create sha256:img0123' "$d/log" && grep -q '^rm .*cid0123' "$d/log"; then
```

Condition for a renewed audit: the committed probe must vary the image returned by
the `inspect` stub and demonstrate failure for a hard-coded/stale image; it must
also exercise the normal default destination rather than trusting a print-only
branch while the install scenarios use an override.  Every fixture setup must be
asserted before the scenario: `mk_stub` currently has no fail-closed checks for
creation/executability of its stub or for PATH selecting it, so w4/w5 can treat an
unrelated execution failure as their expected failure.

### B-2 — task 3 assigns `.github/workflows/deploy.yml` outside engine-dev's zone

M-93 authorizes `engine-dev` to modify `.github/workflows/deploy.yml`
(`milestones/M-93-watchdog-delivery.md:89-104`).  The binding scope table permits
that role only its listed crate `src/**`, `deploy/**`, and the root
`docker-compose.yml` (`.claude/rules/scope-guard.md:12`); the table expressly
forbids everything else.  A milestone's Allowed paths cannot expand the role's
zone.  The stated independent §9 recheck is necessary for a workflow change, but
does not grant write ownership.  Task 3 therefore cannot be lawfully dispatched.

The committed range additionally changes `docs/ROADMAP.md`, but §7's architect
Allowed paths omit it:

```text
$ git diff --name-status origin/main...HEAD
M	docs/ROADMAP.md
A	milestones/M-93-watchdog-delivery.md
A	scripts/tests/red_m93_watchdog_delivery.sh
A	scripts/tests/red_verify_M-93_ci_map.sh
A	scripts/verify_M-93.sh
M	scripts/verify_delivery_M-08.sh
```

Condition for a renewed audit: give task 3 to an explicitly authorized role (or
obtain the required scope decision without silently widening this milestone), and
make the actual committed architect paths agree with §7.  Retain the independent
Fable §9 recheck for the eventual `deploy.yml` change.

## Checks that passed / limits accepted as named

- The production read-only measurement matches the claimed baseline: the VPS has
  only the manually built `/root/hft-platform/target/release/ops-watchdog`, no
  `/usr/local/lib/hft/ops-watchdog`; cron calls the wrapper, Dockerfile does not
  build the binary, and the watchdog log has fresh normal cycles.
- Keeping the watchdog host-side is justified: the host cron process can observe a
  failed Docker daemon/container; `ops-watchdog` actually calls `docker ps` and
  `docker inspect`.  Its external alert transport remains intentionally disabled
  pending P-003, which the milestone does not alter.
- §6 is honest about w7 being a `deploy.yml` text oracle and D9-deep checking image
  executability rather than starting a Docker-observing watchdog.  Those limits do
  not cure B-1's missing identity dataflow proof.
- `verify_M-93.sh` is a real FAIL-counting gate.  Its CI-map probe passes all ten
  declared worlds; D9-deep is wired to the existing CI delivery job through
  `HFT_DELIVERY_DEEP=1`.

## Done Block

```text
$ bash scripts/tests/red_m93_watchdog_delivery.sh; rc=$?; printf 'exit=%s\n' "$rc"
FAIL  SETUP: deploy/bin/install-watchdog.sh отсутствует — доставки нет (TD-231)
FAIL  w1 композиция: деплой кладёт «», cron зовёт «/root/hft-platform/target/release/ops-watchdog» (обязаны совпасть, абсолютный путь вне target/)
FAIL  w2 установка: exit=99, вызовы «», в dst:  —
FAIL  w3 отказ «cp»: exit=99, dst=«», файлов 0 контейнер не удалён после отказа cp
FAIL  w4 отказ «inspect»: exit=99, dst=«», файлов 0
FAIL  w5 отказ «empty»: exit=99, dst=«», файлов 0
FAIL  w6 Dockerfile: нет --bin ops-watchdog и/или COPY в /usr/local/bin/ops-watchdog
FAIL  w7 deploy.yml: установка сторожа — ветка healthy: 0, ветка отката: 0 (нужно ≥1 на обеих)
pass  w8 cron без бинаря: exit=1, ALERT в логе и файле тревоги
сценариев: 9   pass=1   FAIL=8
VERDICT: FAIL
exit=1

$ VERIFY_M93_CI_DRY=1 bash scripts/verify_M-93.sh
PASS  ci-parity: учтено шагов 57 из 57 (исполнено 50, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_verify_M-93_ci_map.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
PASS  строка дописана в блок базы ⇒ FAIL (C-266 F2) (exit=1)
PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
PASS  (i) check_review_fa + дописка ⇒ исполняется, не SKIP (exit=0)
PASS  (ii) run: |- — литеральный блок, исполняется (exit=0)
PASS  (iii) run: > — складывающий скаляр ⇒ FAIL (exit=1)
VERDICT: PASS — 10 сценариев
exit=0

$ bash -n scripts/tests/red_m93_watchdog_delivery.sh scripts/tests/red_verify_M-93_ci_map.sh scripts/verify_M-93.sh scripts/verify_delivery_M-08.sh
bash-n=0

$ git diff --check origin/main...HEAD
diff-check=0

$ ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 '<read-only measurement>'
== git ==
1d119d9ebe97573df5614c5846aa92d5e794e322
== watchdog paths ==
ls: cannot access '/usr/local/lib/hft/ops-watchdog': No such file or directory
-rwxr-xr-x 1 root root 5030640 Oct  2 10:41 /root/hft-platform/target/release/ops-watchdog
== running recorder image ==
sha256:dbd53a1ee651c8314306b5b0feaa2566f8ab805b8996e88492250cee87bb1077
== host OS ==
Ubuntu 26.04 LTS 26.04
== watchdog log tail ==
[ops-watchdog] 1791030601832 — норма, ни одно условие не сработало
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-03T12:00Z
- Milestone: M-93-watchdog-delivery
- Статус: BLOCKED (REJECT)
- HEAD: 744e815 — docs(ROADMAP): строка 4bis — M-93 доставка сторожа деплоем [architect]

## §B — Что я сделал
- Audited the committed M-93 artifact set and its actual VPS baseline.
- Reproduced the intended 8/9 RED baseline and both passing CI-map checks.

## §C — Артефакты / результаты
- `research/critiques/C-277-m93-watchdog-delivery.md`
- Done Block above; RED baseline exit=1 is expected before implementation, CI-map gates exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Resolve C-277 on origin/feat/M-93-watchdog-delivery.  Amend the committed M-93 artifact set so its RED oracle proves dynamic image identity from docker inspect of the running hft-recorder through docker create/cp, proves normal default destination composition, and fail-closes each fixture setup.  Correct task-3 ownership for .github/workflows/deploy.yml under scope-guard and include the already changed docs/ROADMAP.md in Allowed paths or remove the mismatch.  Keep the independent Fable §9 recheck requirement.  Commit and push the artifacts, then request a fresh critic round.
  ```
- Push-статус: pending this verdict commit to `origin/feat/M-93-watchdog-delivery`.
- Кэш: N/A — read-only audit; no build cache created.

## §E — Риски / открытые вопросы
- A hard-coded image source can make a deployment appear successful while installing an old or foreign watchdog binary.

=== END HANDOFF ===
