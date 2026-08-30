<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: fbc10a20b8dfa088e4ae3d9ad5b695620a2fc47a
audited_head: 2b0474431b2af32b740ebd6ec5455dab863563fd
verdict: REJECT
-->

# C-171 — M-71: A-026 checklist — REJECT

## Scope

Reserved id: **171** (`bash scripts/reserve_artifact_id.sh C`).  Audit scope is exactly
`fbc10a2..2b04744`: one architect commit, with no `crates/*/src/**` change under the
correct glob pathspec.  This is a plan-time audit of committed artifacts, not a review of
the implementation below the range.

The touched gateway surface is subject to the live FA invariant **VB-I-2** (`live ==
replay`); it was opened in `docs/fa/viz-backend.md` before use.  The existing path oracle
also retains VB-I-11.  Neither `gateway` nor `gateway-serve` has its own FA: the explicit
absence is recorded by the viz-backend FA.  The map's claim of 12 `GW-I-*` oracles is stale:
the measured set is `GW-I-1..12` plus `GW-I-14` (13).

## Verdict — REJECT

**O-8 / reachability control is not reproducible in the pinned environment.**  Step C of
`scripts/verify_M-71.sh` creates its isolated mutation tree with only `crates`,
`Cargo.toml`, and `Cargo.lock`; it omits `rust-toolchain.toml`.  In this checkout that
exact copy resolves `cargo` to 1.94.1, while the checked-in toolchain is 1.97.0.  Adding the
toolchain file changes it back to 1.97.0.  Thus the committed verifier still measures the
host toolchain in precisely the TD-035 class that the commit message says its reachability
probe discovered and repaired.

The current red baseline happens to complete with the predicted six failing steps, and C
prints PASS here; that does not repair the environment mismatch.  Once the intended
implementation makes the base green, C is the evidence that the mutation suite remains
reachable.  Its result cannot establish CI-parity while the copy silently selects another
compiler/toolchain.  The defect is in the allowed verification zone and makes the claimed
GREEN proof non-reproducible, not merely advisory.

Routing is therefore the exceptional path in A-026 §4(4)(b): a newly demonstrated instance
of the reachable-GREEN failure class in the permitted zone.  It is **not** an oracle
sufficiency dispute and must not be converted into a TECH-DEBT card.  No tenth critic
round is authorized.

## A-026 §3 checklist

| Item | Result | Evidence |
|---|---|---|
| O-1 | PASS | §6 row D now distinguishes invalid values (startup reject) from absent/empty/blank-after-trim (P-020), including the implementation it must reject. |
| O-2 | PASS | Startup's `empty_and_blank_are_same_as_absent` checks the Result-side pair; governed N1-D compares `Result<usize, _>` against absence and then pins `Ok(DEFAULT_MAX_RESPONSE_BYTES)`.  It therefore rejects `empty => Ok(other value)`, not merely `both Ok`. |
| O-3 | PASS | Task 4 now makes absent, empty, and whitespace forms one incomplete configuration under A-015 §3 p.1, with the planned source repair still assigned to engine-dev. |
| O-4 | PASS | §4bis.2bis preserves the visible A-026 trace of the former opposite empty-value oracle and identifies all four divergent carriers. |
| O-5 | PASS | The bridge wording is decomposed honestly: N1-E protects failed parsing, L3 is a textual caller inventory, and the unpinnable in-process runtime reset is expressly reviewer-owned debt per A-026 §3bis/P-022. |
| O-6 | PASS | N1-E first installs non-default `V=5000`, proves that installation, then supplies `abc`, requires `Err`, and reads the gateway effective value back as `5000`.  Removing the first phase would make “never called setter” a false green; it is present. |
| O-7 | PASS | L3 is labelled inventory rather than a milestone RED oracle; zero callers fails, exactly one caller in `serve_config_from_env` passes.  The isolated rename probe changes zero callers to FAIL (1).  The current old serve-side getter has zero readers and its doc string falsely says it is read on every snapshot/frames/pump call; Task 10 explicitly requires removal in the implementation commit. |
| O-8 | **REJECT** | B composition is correctly pinned as 10 total / 7 startup rejects plus N1-D/N1-E presence, and the six-step baseline reproduces.  Its step-C isolated mutation environment nevertheless omits the pinned toolchain, invalidating the required reachability evidence above. |

## Required disposition

Arbiter: decide the A-026 §4(4)(b) return on this reproducibility defect.  The factual
repair is bounded: the isolated mutation/probe tree must contain the checked-in
`rust-toolchain.toml`, and the reachability/mutation evidence must be rerun under 1.97.0.
The already-declared reviewer close-out debt remains separate and unchanged:
`runtime-переустановка-эффективного-предела-ответа-не-наблюдаема-оракулом`, severity
MINOR, as specified verbatim by A-026 §3bis.  It is not introduced by this verdict.

## Done Block

```text
$ git rev-parse HEAD
2b0474431b2af32b740ebd6ec5455dab863563fd
$ git merge-base --is-ancestor fbc10a2 2b04744
exit=0
$ git log --oneline fbc10a2..2b04744
2b04744 docs(M-71): исполнение A-026 O-1..O-8 — политика отсутствия/пустоты едина по A-015, «один раз» моста переоформлен, непокрытое названо [architect]
$ git diff --numstat fbc10a2..2b04744 -- ':(glob)crates/*/src/**'
(empty)
exit=0
$ git show --numstat --format='' 2b04744
86	0	crates/gateway-serve/tests/red_egress_cap_governed.rs
48	6	crates/gateway-serve/tests/red_egress_cap_startup.rs
65	9	milestones/M-71-egress-cap.md
57	0	scripts/tests/red_egress_doors.sh
28	3	scripts/verify_M-71.sh

$ bash scripts/verify_M-71.sh
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: cargo test -p gateway --test red_egress_cap_paths --quiet
FAIL: cargo test -p gateway-serve --test red_egress_cap_utf8 --quiet
FAIL: cargo test -p gateway-serve --test red_egress_cap_governed --quiet
FAIL: cargo test -p gateway-serve --test red_egress_cap_startup --quiet
PASS: B состав набора — 10 оракулов (ожидалось 10: 7 отказов + 1 равенство исходов + 2 vantage)
PASS: B2 парный N1-D на месте — равенство «пусто ≡ отсутствие» судится по эффективному значению
PASS: B3 N1-E на месте — при Err разбора эффективное значение не устанавливается
PASS: C база зелена, мутация роняет набор, честная нагрузка (E) цела
PASS: D GATEWAY_MAX_RESPONSE_BYTES объявлен в docker-compose.yml
PASS: E соседние инварианты (включая VB-I-2) не куплены
PASS: F crates/contracts не тронут
PASS: G GATEWAY_BANDS не тронут
PASS: H book/venue/journal не тронуты диапазоном
VERDICT: FAIL (6)
exit=1

$ bash scripts/tests/red_egress_doors.sh
PASS: L3 set_effective_max_response_bytes — ровно один вызыватель, и он внутри serve_config_from_env
VERDICT: PASS — все найденные двери названы в оракулах
exit=0
$ (isolated no-.git copy with the setter call renamed) bash scripts/tests/red_egress_doors.sh
FAIL: L3 SETUP НЕ СОСТОЯЛСЯ — вызывателей set_effective_max_response_bytes( в crates/*/src не найдено НИ ОДНОГО.
VERDICT: FAIL (1) — дверь существует, а оракул её не зовёт
exit=1

$ m71_probe=$(mktemp -d /tmp/hft-critic-m71-toolchain-XXXXXX)
$ cp -a crates Cargo.toml Cargo.lock "$m71_probe/"
$ test ! -e "$m71_probe/.git"; echo "git_metadata=absent"
git_metadata=absent
$ (cd "$m71_probe" && cargo --version)
cargo 1.94.1 (29ea6fb6a 2026-03-24)
$ cp -a rust-toolchain.toml "$m71_probe/"
$ (cd "$m71_probe" && cargo --version)
cargo 1.97.0 (c980f4866 2026-06-30)
exit=0

$ grep -n 'cp -a crates Cargo.toml Cargo.lock' scripts/verify_M-71.sh
166:elif ! cp -a crates Cargo.toml Cargo.lock "${MUT}/" 2>/dev/null; then
$ bash -n scripts/verify_M-71.sh; bash -n scripts/tests/red_egress_doors.sh
exit=0

$ EVENT_NAME=push PUSH_BEFORE=2b0474431b2af32b740ebd6ec5455dab863563fd bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
$ EVENT_NAME=push PUSH_BEFORE=2b0474431b2af32b740ebd6ec5455dab863563fd bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 2b04744..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ grep -n 'M-71\|egress\|TD-167' TECH-DEBT.md
exit=1
$ grep -n 'M-71' PROJECT-STATE.md milestones/BACKLOG.md
exit=1
$ grep -rhoE '\bGW-I-[0-9]+\b' crates/ | sort -uV
GW-I-1 ... GW-I-12
GW-I-14
exit=0
```
