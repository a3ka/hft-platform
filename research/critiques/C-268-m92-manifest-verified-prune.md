# C-268 — M-92 manifest-verified prune: REJECT

<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 6ef8c672f3895e85ed73628c81ab3d3089638d69
audited_head: b48084bf8f6f1384e7fa940f34340614c8ef8546
verdict: REJECT
-->

## Verdict

**REJECT.** The originally handed-off artifact set has two production-path
oracle holes and an insufficient audit-trail oracle. More importantly, the
subject branch advanced while this audit was running. The current branch head
`0f7a8bf255afa0bfaeba9318fc9459ee637ef78f` is not cleared by this verdict;
it needs a new handoff and a fresh audit.

## Subject, scope, and artifact completeness

The branch named in the handoff was fetched first and its then-current head
was `b48084bf8f6f1384e7fa940f34340614c8ef8546` (the `audited_head` above).
The complete architect set existed there, in the required commit order:

| required artifact | evidence |
|---|---|
| milestone | `milestones/M-92-manifest-verified-prune.md` — `59d70d5` |
| T2 API shape | `ColdManifest::parse` and `retention_execute_with_manifest` are specified in milestone §4; no `contracts/**` T1 change or trait signature is in scope |
| RED suite | `crates/journal/tests/red_m92_manifest_prune.rs` — `455ddaa` |
| acceptance gate | `scripts/verify_M-92.sh` — `089080b` |
| CI-map adversarial probe | `scripts/tests/red_verify_M-92_ci_map.sh` — `b48084b` |

This is a RAW gate: it changes the mechanism that can remove journal data.
The relevant live invariants are `JR-I-2` (no gap in `seq`) and `JR-I-11`
(a reader must not silently stitch a non-monotonic segment catalogue).

## Findings

### C-268-1 — `c1` does not observe the production Compose path (BLOCKER)

`c1` is described as the production-cron oracle, and milestone §4 requires a
rw journal mount, `gateway-ckpt:/ckpt:ro`, and a host-to-container `/work`
mount. Its `RETENTION_RUNNER` seam instead throws away every argument through
the `journal-retention` service name and executes the host binary after string
replacement (`red_m92_manifest_prune.rs:570-577`). It never invokes or inspects
`docker-compose.yml`.

Consequently, an implementation that leaves the deployment composition wrong
passes `c1`: on the audited head, the service has `journal-data:/journal:ro`,
a `/cold` mount, and none of the required `/ckpt` or `/work` mounts
(`docker-compose.yml:63-87`). That is a surrogate, not the deploy entry point
required by `testing.md`'s carrier-path oracle. It would turn an intended apply
into a fail-closed no-op at best, while claiming the end-to-end task passed.

The repaired RED suite must exercise the real `docker compose run ...
journal-retention` composition (or an equivalently executable isolated
composition), proving the actual rw/ro mounts and absence of `/cold`; replacing
container paths in a host-binary shim is not evidence of that wiring.

### C-268-2 — `c2` cannot catch the prohibited cron default `apply` (BLOCKER)

`run_wrapper` imports variables from `deploy/cron.d/journal-retention`, but
then unconditionally overwrites `RETENTION_MODE` with its caller argument
(`red_m92_manifest_prune.rs:595-615`). `c2` calls it with the literal
`"dry-run"` (`:704-716`). Thus this mutation passes the claimed default-mode
test:

```text
deploy/cron.d/journal-retention: RETENTION_MODE=apply
```

It is exactly the boundary-C regression the milestone says must be prevented:
automatic apply without a founder decision. The existing cron file happens to
say `RETENTION_MODE=dry-run` (`deploy/cron.d/journal-retention:34-42`), but the
oracle never measures that fact. The RED suite must let the cron-file default
drive the test and include an `apply`-default mutant that demonstrably fails.

### C-268-3 — the required audit trail is reduced to a nonempty-directory check (BLOCKER)

Milestone §4 requires the plan, remote manifest, and report in
`RETENTION_AUDIT_DIR`. `c1` only asserts that the directory is nonempty
(`red_m92_manifest_prune.rs:688-698`). A single marker or report with neither
the selected plan nor the remote checksum evidence passes, so the automated
path can delete files without a reconstructible proof of why each name was
eligible. The oracle must bind all three required records to the selected
names and preserve the mismatch evidence that causes exit 2.

## Branch movement: no clearance of the current head

After the above inspection and test runs, a required re-fetch reported:

```text
audited head: b48084bf8f6f1384e7fa940f34340614c8ef8546
current branch: 0f7a8bf255afa0bfaeba9318fc9459ee637ef78f
new commits:
0ff1698 test(M-92): режим удаления — только файл-переключатель на хосте; c3 …
0a5ef9a docs(pending): П-031 — включение автоудаления журнала …
0f7a8bf docs(M-92): спека — включение по П-031 …
```

Those commits were not part of the requested artifact set or this audit.
They may address a finding, but cannot be credited without a new branch-head
handoff and a fresh critic measurement.

## Required before re-review

1. Repair C-268-1 through C-268-3 with RED oracles that fail against the
   stated deployment/default/audit-trail mutants.
2. Commit and push the revised complete artifact set.
3. Hand off the new `origin/feat/M-92-manifest-verified-prune` head; do not
   reuse this verdict as approval of the moved branch.

## Done Block

```text
$ git fetch origin --prune && git rev-parse origin/feat/M-92-manifest-verified-prune
b48084bf8f6f1384e7fa940f34340614c8ef8546
exit=0

$ cargo test -p journal --test red_m92_manifest_prune
error[E0432]: unresolved import `journal::ColdManifest`
error[E0425]: cannot find function `retention_execute_with_manifest` in crate `journal`
error[E0425]: cannot find function `retention_execute_with_manifest` in crate `journal`
error[E0425]: cannot find function `retention_execute_with_manifest` in crate `journal`
error[E0425]: cannot find function `retention_execute_with_manifest` in crate `journal`
error[E0425]: cannot find function `retention_execute_with_manifest` in crate `journal`
error[E0425]: cannot find function `retention_execute_with_manifest` in crate `journal`
error: could not compile `journal` (test "red_m92_manifest_prune") due to 7 previous errors
exit=101  # expected COMPILE-RED: only ColdManifest / retention_execute_with_manifest

$ bash scripts/tests/red_verify_M-92_ci_map.sh
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

$ git status --short
<empty before verdict creation>
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC): 2026-10-02
- Milestone: M-92-manifest-verified-prune
- Status: BLOCKED — REJECT
- Audited HEAD: b48084b; branch advanced to 0f7a8bf during audit

## §B — What I did
- Audited the committed M-92 artifact set, the existing retention implementation,
  the cron wrapper, Compose, and the CI-map probe.
- Measured the intended compile-RED and CI-map anti-plaque probe.

## §C — Result
- `C-268-m92-manifest-verified-prune.md` — REJECT.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```text
  Repair C-268 on M-92. First take the current origin/feat/M-92-manifest-verified-prune
  head as the subject. Make c1 observe the real journal-retention Compose path and its
  rw /journal, /ckpt:ro, /work, no-/cold topology; make c2 prove the cron-file default
  cannot become apply; and make c1 prove plan, manifest, and report records bind the
  selected names and mismatch evidence. Commit/push the revised artifacts, then hand
  off the exact new branch head for a fresh critic audit.
  ```
- Push status: pending this verdict commit.
- Cache: no build-cache cleanup performed; this detached audit worktree remains available.

## §E — Risks / open questions
- The branch moved to `0f7a8bf` during this audit. Its added c3/P-031 work was not judged.

=== END HANDOFF ===
