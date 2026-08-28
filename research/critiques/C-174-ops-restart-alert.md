<!-- GATE-META
milestone: M-09
audited_repo: a3ka/hft-platform
audited_base: f4c00b9ac3ebb1fefff3345f63f0bd9179e88323
audited_head: 539054a9780f76caa571a2589adf0431b747bf33
verdict: REJECT
-->

# C-174 — OPS-RESTART: reset-счётчик не доказывает рестарт

## Verdict: REJECT

`539054a` корректно делает отсутствующее правило наблюдаемым RED-условием, но ещё
не является достаточным закоммиченным plan-time набором для передачи engine-dev. Реализация
`OPS-RESTART` может сделать verify зелёным, не детектируя рестарт и не давая безопасной
политики при restart-loop.

## Audit scope

- Subject branch: `origin/docs/ops-restart-alert` at
  `539054a9780f76caa571a2589adf0431b747bf33`.
- Base: `f4c00b9ac3ebb1fefff3345f63f0bd9179e88323`.
- Committed diff has exactly `docs/fa/ops.md` and `scripts/verify_M-09.sh`; no implementation
  was audited or requested.
- Live invariant read and applied: `OPS-I-5` — a P0/P1 incident class without a rule is an
  observability defect.

## Blocking findings

### R1 — `resets()` is a decrease detector, not a restart identity

The proposed claim that `resets(journal_frames_written_total[1h]) > 0` is a canonical
restart signal is false. Prometheus defines `resets()` for float counters as **any decrease
between consecutive samples**; it cannot distinguish a process restart from another decrease
source. [Prometheus `resets()` reference](https://prometheus.io/docs/prometheus/3.5/querying/functions/#resets)
states that rule directly.

Locally the counter is an `AtomicI64` and `inc_counter()` uses `fetch_add` in
`crates/ops/src/metrics.rs`; a wrap/decrease therefore has the same observable shape. A pure
scrape gap does not itself create a decrease, but it removes the adjacent observations that
the query needs: a restart wholly unobserved (or followed by enough frames to exceed the prior
sample) is not detected. The proposed formula is a useful *best-effort symptom* only, not the
stated proof of a writer restart.

Before implementation, architect must correct the FA claim and add a sacred RED oracle that
pins the intended expression and its observation contract: detected restart, a missed-scrape
case, and a non-restart decrease/wrap case. The current test set has no `OPS-RESTART` or
`resets(` oracle.

### R2 — the committed RED only proves incident-ID presence; a wrong alert expression passes

Adding `OPS-RESTART` to `REQUIRED_INCIDENTS` gives the requested single targeted RED failure,
but `scripts/verify_M-09.sh:102-106` tests only that an `AlertRule.incident` exists. It does
not assert the required `resets(...)` expression, its window, or its P1 severity.

This is materially exploitable by the current planned implementation: `expr_for()` in
`crates/ops/src/alerts.rs:191-248` falls back for an unknown `(incident, metric)` pair to
`<metric> > 0`. An engine-dev can add the new incident to `ALERT_RULES`, make the verifier
green, and deploy an always-true `journal_frames_written_total > 0` rule instead of restart
detection. `red_ops_alerts.rs` repeats the old ten-item canonical list and likewise has no
expression-semantic oracle.

Architect must commit the RED change before dispatch: it must require the `OPS-RESTART`
incident, `P1`, the exact intended expression/window, and a rendered deploy artifact that
matches it. The existing M-09 task-4 specification already makes this architect-owned RED
work, so its absence is not an engine-dev task.

### R3 — the proposal neither reconciles nor repairs the existing restart observer

The repository already contains `WD-CONTAINER-RESTARTED`: `ops-watchdog` compares Docker
`RestartCount` with a persisted previous count, and explicitly bypasses dedup for a restart
loop (`crates/ops/src/watchdog_cycle.rs:355-364`; `docs/runbooks/alerting.md:42`). That
detector records the very event the new text says nobody can see, including the healthy-after-
restart case.

The measurement `RestartCount = 55` therefore first demonstrates that this existing observer
is not operationally delivering an alert (or lacks a prior baseline), not that a process-local
metric is necessarily the right canonical detector. `DESIGN.md` §23.1 also says the push
channel and cron setup are pending founder action. The FA must state which detector is the
canonical `OPS-RESTART` source, why the other is insufficient, and how the alert reaches a
human outside the recorder. A second, unconnected incident family is not PL-I-8 compliance.

### R4 — P1 names the unbounded-loss problem but provides no policy bound

The text does name the restart-batch limit, so this is not an omitted sentence. It remains a
hole in the executable contract: the only implementable rule is one P1 reset event, with the
generic P1 `for: 5m`; no count, cumulative data-gap, or escalation condition turns a restart
loop into P0. The gap is forward-only and grows with every restart, while the proposed class
stays P1 indefinitely.

Architect must specify and RED-pin either a bounded P1 policy with an explicit escalation
condition or the accepted P0 condition. This verdict does not choose the threshold/severity.

## Non-blocking checks

- The parser concern is resolved in this revision. The §7.1 extractor first accepts only rows
  beginning `| \`` and then exact backtick-delimited `[a-z_]+` names. In the new row,
  `healthy` is bold text and the only matched metric token is
  `journal_frames_written_total`; prose `` `healthy` `` is discarded before metric extraction.
  No remaining accidental metric token was found.
- The claimed historic count is not reproducible on the audited base: it produced 21 PASS
  lines, not 13. The substantive delta is correct: head has those 21 PASS lines plus exactly
  one addressed FAIL for `OPS-RESTART`.

## Required resubmission evidence

1. Corrected FA semantics for the restart detector and its blind spots.
2. Architect-owned RED tests for detector semantics, exact rendered PromQL, severity, and the
   restart-loop escalation policy; include the updated canonical list in the Rust oracle.
3. A documented reconciliation of `OPS-RESTART` with `WD-CONTAINER-RESTARTED`, including the
   operational delivery path required by PL-I-8.
4. Re-run `bash scripts/verify_M-09.sh` on the revised artifact set. RED is acceptable before
   implementation only when it fails for the specified, behavior-bearing oracle rather than
   merely a missing incident string.

## Done Block

```text
$ git diff --name-status f4c00b9ac3ebb1fefff3345f63f0bd9179e88323..539054a9780f76caa571a2589adf0431b747bf33
M       docs/fa/ops.md
M       scripts/verify_M-09.sh
exit=0

$ bash scripts/verify_M-09.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'; rc=${PIPESTATUS[0]}; echo "verify_exit=$rc"  # base f4c00b9
PASS  T1 CT-RFC-03 (SysEvent::ReconDivergence, red_rfc03)
PASS  T1 схема CT-I-4 (event.schema.json == типы)
PASS  T2 OPS-I-1 recon (ε_test, деградированные входы)
PASS  T2 OPS-I-1 recon LIVE-режим (near-book depth-aware, §8 анти-флуд, skew-толерантность)
PASS  T2 OPS-I-1 recon РАНТАЙМ-КОНТРАКТ B2 (best-only+seed-gate; персистентный объём→ТИШИНА, best-порча→эмит; §4.3.2)
PASS  T2 OPS-I-1 recon SINK B2 (эмиссия ⟺ best: персистентный объём→тишина, best-десинк→Sys+метрики)
PASS  T2 OPS-I-9 rate-budget (анти-hot-loop, TD-013)
PASS  T2 OPS-I-4/7/8 метрики+тишина
PASS  T4A OPS-I-4 /metrics HTTP-сервер ЧИСТЫЙ (GET/metrics→200+тело, 404, 405; ops::server)
PASS  T4A OPS-I-4 /metrics socket (recorder биндит loopback, реальный TCP GET→200+тело)
PASS  T4B OPS-I-5 правила алертов + rule-паритет (правило→метрика, класс→правило, рендер)
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (прогон writer/feeder/sampler → SAMPLE, не HELP/TYPE; TD-027)
PASS  OPS-I-6 метрики не в журнал (ops не зависит от journal в рантайме)
PASS  OPS-I-5 §7.1→код: каждое правило ссылается на существующую метрику
PASS  OPS-I-5 код→§3: каждая METRICS объявлена в §3
PASS  OPS-I-5 §7.1 покрывает все канонические классы инцидентов (класс без правила невозможен)
PASS  OPS-I-5 каталог правил покрывает все обязательные классы §7.1
PASS  OPS-I-5 каталог правил → §7.1: нет правил-сирот (все привязаны к классу)
PASS  OPS-I-10 каждая §3-метрика покрыта emission-оракулом или классифицирована event/elsewhere
PASS  OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main
VERDICT: PASS
verify_exit=0

$ bash scripts/verify_M-09.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'; rc=${PIPESTATUS[0]}; echo "verify_exit=$rc"  # audited head 539054a
PASS  T1 CT-RFC-03 (SysEvent::ReconDivergence, red_rfc03)
PASS  T1 схема CT-I-4 (event.schema.json == типы)
PASS  T2 OPS-I-1 recon (ε_test, деградированные входы)
PASS  T2 OPS-I-1 recon LIVE-режим (near-book depth-aware, §8 анти-флуд, skew-толерантность)
PASS  T2 OPS-I-1 recon РАНТАЙМ-КОНТРАКТ B2 (best-only+seed-gate; персистентный объём→ТИШИНА, best-порча→эмит; §4.3.2)
PASS  T2 OPS-I-1 recon SINK B2 (эмиссия ⟺ best: персистентный объём→тишина, best-десинк→Sys+метрики)
PASS  T2 OPS-I-9 rate-budget (анти-hot-loop, TD-013)
PASS  T2 OPS-I-4/7/8 метрики+тишина
PASS  T4A OPS-I-4 /metrics HTTP-сервер ЧИСТЫЙ (GET/metrics→200+тело, 404, 405; ops::server)
PASS  T4A OPS-I-4 /metrics socket (recorder биндит loopback, реальный TCP GET→200+тело)
PASS  T4B OPS-I-5 правила алертов + rule-паритет (правило→метрика, класс→правило, рендер)
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (прогон writer/feeder/sampler → SAMPLE, не HELP/TYPE; TD-027)
PASS  OPS-I-6 метрики не в журнал (ops не зависит от journal в рантайме)
PASS  OPS-I-5 §7.1→код: каждое правило ссылается на существующую метрику
PASS  OPS-I-5 код→§3: каждая METRICS объявлена в §3
PASS  OPS-I-5 §7.1 покрывает все канонические классы инцидентов (класс без правила невозможен)
FAIL  OPS-I-5 ALERT_RULES не покрывает классы §7.1: OPS-RESTART — класс без правила (rule-side дыра)
PASS  OPS-I-5 каталог правил → §7.1: нет правил-сирот (все привязаны к классу)
PASS  OPS-I-10 каждая §3-метрика покрыта emission-оракулом или классифицирована event/elsewhere
PASS  OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main
VERDICT: FAIL (1)
verify_exit=1

$ names_fa71=$(sed -n '/### §7.1/,/Правило паритета/p' docs/fa/ops.md | grep '^| `' | grep -oE '`[a-z_]+(\{[^}]*\})?`' | sed -E 's/`//g; s/\{.*//' | sort -u); printf '%s\n' "$names_fa71"
backup_restore_drill_ok
book_levels
book_resync_total
journal_disk_free_bytes
journal_frames_written_total
journal_seq_gaps_total
md_event_age_ms
md_events_total
recorder_rss_anon_bytes
venue_http_status_total
exit=0

$ EVENT_NAME=push PUSH_BEFORE=539054a9780f76caa571a2589adf0431b747bf33 bash scripts/check_artifact_ids.sh
FAIL  C-172: второй носитель «research/critiques/C-172-ops-restart-alert.md» под идентификатором, занятым «research/critiques/C-172-harness-milestone-shape-r2.md»
exit=1

$ bash scripts/reserve_artifact_id.sh --release C-172
reserve: резерв C-172 снят
exit=0

$ bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-173 ← b06bf63f81e6f637eb88fa8e413ac4a4cce02ccb
reserve:   C-173 занят; следующий кандидат — C-174
reserve: попытка 2/8 — C-174 ← 38a0caa22f62a80b4c2d5c1341d08bfde8d6bded
C-174
reserve: резерв C-174 взят
exit=0

$ EVENT_NAME=push PUSH_BEFORE=539054a9780f76caa571a2589adf0431b747bf33 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 539054a..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ EVENT_NAME=push PUSH_BEFORE=539054a9780f76caa571a2589adf0431b747bf33 bash scripts/check_gate_meta.sh
── GATE-META: диапазон 539054a9..HEAD, origin=a3ka/hft-platform
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
```
