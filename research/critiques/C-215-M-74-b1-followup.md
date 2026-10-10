<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: bcb667882a89d3a78f2372da4ad0fa26288a2513
audited_head: 8c0f3d6d1d015da7ae6e0ea32cebe543292a6100
verdict: REJECT
-->

# C-215 — M-74 delivery follow-up: REJECT

## Verdict

**REJECT — do not dispatch engine-dev.** `C-214` B-1 is not closed: the new
`delivery_ok` accepts a directory containing per-file symlinks to `cold`, and
`h_verdict` then accepts its genuine digest. This is still cold data being read,
not a separately materialized restore. It violates the operational boundary
required to demonstrate **JR-I-6**: old journal data must be readable by the
new path after restoration, not merely via an alias back to the source.

`C-214` B-2 is also not closed. The milestone still has active rev-4 observer/W
claims and an obsolete route/round count, so it does not define one unambiguous
rev-5 artifact set for the implementation agent.

This is the second consecutive REJECT on the same delivery-proof surface. Per
`gates.md` §0 and the critic profile, the next agent is the independent
arbiter—not an architect self-fix round.

## B-1 — `delivery_ok` misses per-file symlink delivery

`delivery_ok` rejects only a symlinked *directory* at
`scripts/tests/red_restore_drill.sh:491`, compares only the two directory
realpaths (`:493-494`), and asks `find -type f` for inodes (`:497-500`).
`find` does not treat a symlinked segment as `-type f`. A fake can therefore:

1. create a real, distinct `restore/` directory;
2. place `segment-*.jrnl*` (and sidecars) there as symlinks to `cold/`;
3. invoke the reader on `restore/` and write that real digest into state.

The committed predicate returns success, and the direct journal helper reads
10,953 events through those symlinks and yields a real digest. The H predicate
then accepts `ok=1`, `events_read>0`, `checked=3`, a compressed segment, and
the matching digest. Scenario X exercises only `restore -> cold`; it does not
exercise a distinct restore directory whose contents alias cold.

The inode member is not inert for the attack it names: directory hardlinks are
rejected while the `-L` and directory-realpath members pass. The defect is the
uncovered per-file-link form, not a request to remove redundancy.

**Condition to clear:** the RED artifact must contain and execute a per-file
symlink attack with a genuine digest, then reject it specifically as failed
delivery. The acceptance condition must cover the materialization of every
restored input that can be opened by the reader—not just the `restore/`
directory object. The test must remain red after task 2b exists.

## B-2 — stale rev-4 claims remain active

The promised class-wide cleanup did not reach these active claims:

- `milestones/M-74-restore-drill.md:293` says the probe supplies a transparent
  observer, despite rev 5 removing it.
- `:586-590` says F guards delivery; the newly committed source itself explains
  at `scripts/tests/red_restore_drill.sh:461-464` that F misses the link form.
- `:596` is a live task row but inventories `S/W/P×2/H/C/E/F/A` and an observer.
- `:604` says the script *today contains* W and `JOURNAL_DRILL_READER` observer
  scenarios even though its status is DONE.
- `:648` records C-189 as closed by observer/W and points to a non-existent
  observer section in the script.
- `:694-696` retains the superseded A-028 round limit.
- `:700-703` dispatches critic round 3 only for A-028 §3; founder's rev-5 reset
  and the current C-214/C-215 delivery gate are not represented.

Historical discussion may remain, but it must be explicitly historical. The
contract table, task inventory, closure map, route, and handoff are current
instructions and cannot contradict the actual rev-5 probe.

## Artifact-set audit

| Required artifact | Result | Evidence |
|---|---|---|
| T2 contracts / signatures | Present | Wrapper, reader, state and selection contracts remain in the milestone; no `contracts/**` change. |
| RED tests | **Incomplete — REJECT** | D/X/R run, but X covers only a directory symlink; per-file symlinks pass `delivery_ok` and H. |
| Acceptance gate | Present, real | Counted failure and exit 1; it delegates task 1 to the incomplete probe. |
| Milestone | **Contradictory — REJECT** | Seven current rev-4/delivery/route claims above conflict with the rev-5 source and founder reset. |
| Scope / T1 | Pass | The audited commit changes only the allowed shell probe and milestone; no T1/risk change. |

## Done Block

```text
$ git log --oneline bcb6678..8c0f3d6
8c0f3d6 fix(M-74): C-214 B-1/B-2 — доставка обязана быть материализована, спека приведена к rev 5 [architect]
exit=0

$ git diff --check bcb6678 8c0f3d6
exit=0

$ bash -n scripts/tests/red_restore_drill.sh
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  обёртки deploy/bin/journal-restore-drill-cron.sh НЕ СУЩЕСТВУЕТ — RED задачи 1
PASS  D выдуманный отпечаток ПОЙМАН при ЧЕСТНО восстановленном каталоге
PASS  X подмена каталога восстановления ПОЙМАНА при СОВПАВШЕМ отпечатке
PASS  R отпечаток прошлого прогона ОТВЕРГНУТ — фикстура одноразова
VERDICT: FAIL (1 из 4)
exit=1

$ cargo test -p journal --test fixture_restore_drill_cold --quiet
running 6 tests
......
test result: ok. 6 passed; 0 failed
exit=0

$ adversarial per-file symlink restore; exact delivery_ok + H predicate
delivery_ok=PASS for per-file-symlink attack
DRILL_DIGEST events=10953 digest=afc0678c700d0beb248daa307c8419aeaf022a9f8e97435fe0a89bcea4dbd4c8
H-predicate=PASS with state.digest=afc0678c700d… events_read>0 checked=3 zst=7
exit=0

$ per-member filesystem checks
directory-symlink L=reject realpath=reject inode=reject
per-file-symlink L=pass realpath=pass inode=pass
hardlink L=pass realpath=pass inode=reject
reflink unsupported
exit=0

$ bash scripts/verify_M-74.sh
PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
PASS: фикстура прод-формы читается journal::stream (исполнено тестов: 6)
FAIL: bash scripts/tests/red_restore_drill.sh
FAIL: test -x deploy/bin/journal-restore-drill-cron.sh
FAIL: test -f crates/journal/src/bin/journal-drill-read.rs
FAIL: test -f deploy/cron.d/journal-restore-drill
VERDICT: FAIL (15)
exit=1

$ bash scripts/next_artifact_id.sh C
C-215
next_artifact_id_exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-08T09:52Z
- Milestone: M-74-restore-drill
- Статус: BLOCKED — REJECT, rev-5 round 2 on the delivery-proof surface
- HEAD: 8c0f3d6 — fix(M-74): C-214 B-1/B-2 — доставка обязана быть материализована

## §B — Что я сделал
- Audited the committed `bcb6678..8c0f3d6` diff, including T2 contracts, RED probe, verify gate, and milestone.
- Reproduced an accepted per-file-symlink restore with a real journal digest; checked all three stated filesystem guards independently.

## §C — Артефакты / результаты
- `research/critiques/C-215-M-74-b1-followup.md`
- Done Block: intentional RED exit=1; fixture exit=0; per-file symlink bypass exit=0; verify exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter`
- Push-статус: ✅ pushed to `origin/docs/M-73-closeout-architect` with the C-215 verdict commit.
- Кэш: ✅ audit `target/` cache removed; temporary adversarial fixtures were moved to trash after measurement.
- **Paste-ready промпт:**
  ```
  You are the independent arbiter for M-74 restore-drill with fresh context. Read C-214 and C-215 plus the committed artifact set at 8c0f3d6. Decide the required delivery-proof boundary: C-214's directory-alias bypass was patched, but C-215 demonstrates that a distinct restore directory with each readable file symlinked to cold passes delivery_ok and H with a genuine digest. Resolve whether the next correction must prove materialization of reader inputs, and resolve the stale active rev-4 claims/round route. Do not write implementation; commit an arbitration decision to the subject branch.
  ```

## §E — Риски / открытые вопросы
- A CoW/reflink probe is unsupported on this filesystem. It is not the rejection basis: the per-file symlink bypass is executable and accepted.
- No founder-only boundary-C decision is requested; the escalation is the mandatory second-REJECT arbitration route.

=== END HANDOFF ===
