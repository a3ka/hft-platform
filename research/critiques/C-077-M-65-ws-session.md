<!-- GATE-META
milestone: M-65
audited_repo: a3ka/hft-platform
audited_base: 3c667772f32fd9d0a71ac1b7681c1c89fc82759b
audited_head: 50dae797ffd54eda4c2b4498b2c5c761d7ef0aca
verdict: REJECT
-->

<!-- Шапка дописана 2026-08-17 architect'ом ПОСТФАКТУМ: вердикт написан до введения
     нормы GATE-META (M-60b), а барьер `check_gate_meta.sh` судит все вердикты в
     диапазоне PR без исключений — grandfathering отвергнут осознанно (GM-30).
     Значения НЕ выдуманы, а извлечены из самого вердикта: Subject строкой :5 — «`milestones/M-65-ws-session.md` at `50dae79`».
     База — `origin/main` той поры, названная в `R-057` строкой :27. Содержание
     вердикта не изменено ни на символ. -->

# C-077 — M-65 ws-session milestone-spec critique

**Critic:** Codex/GPT-5  
**Date (UTC):** 2026-08-11  
**Subject:** `milestones/M-65-ws-session.md` at `50dae79` on `origin/feat/M-65-ws-session`  
**Gate:** `gates.md` §9 — new milestone spec. RAW-gate N/A: no journal layout, no `contracts` impact.  
**Verdict:** **REJECT**

## §0 — Pre-flight

Audited commit:

```text
50dae79 docs(M-65): спека — подписка есть параметр СЕССИИ, а не конфигурация процесса [architect]
 docs/rfc/CT-RFC-09-ws-session.md |  28 +++--
 milestones/M-65-ws-session.md    | 214 +++++++++++++++++++++++++++++++++++++++
```

This is a plan-time document gate, not the full implementation artifact set. The normal
code-milestone preflight for RED tests + verify script is N/A at this round: the subject is the
sufficiency of the milestone spec before architect writes `red_ws_session.rs` and
`verify_M-65.sh`.

Files read for this verdict:

- `CLAUDE.md`
- `.claude/rules/{branch-hygiene,commit-discipline,gates,handoff-block,scope-guard,testing}.md`
- `.claude/agents/critic.md`
- `docs/04-workflow.md` §3
- `docs/05-contract-layer.md`
- `docs/fa/viz-backend.md`
- `docs/rfc/CT-RFC-09-ws-session.md`
- `milestones/M-65-ws-session.md`
- `research/arbitration/A-005-m60a-sufficiency.md`
- current `crates/gateway-serve` tests/code by grep/read where cited

## §1 — Findings

### B-1 — Missing structural axis: connection/session isolation

**Severity:** BLOCKER  
**Evidence:** `milestones/M-65-ws-session.md:89-90`, `milestones/M-65-ws-session.md:105-116`

`§4.1` says the invariant includes absence of influence from subscriptions of another
connection:

```text
Что клиент получает в сокет, определяется ЕГО подписками и ничем иным — ни конфигурацией
процесса, ни подписками соседа по тому же соединению, ни подписками другого соединения.
```

But `§4.2` has no axis for this member of the invariant. The current six axes cover selector
source, number of subscriptions on one connection, message validity, message timing, fate of
neighbor subscriptions on failure, and error carrier. None requires opening two WS connections
and proving that one connection's subscription state, sub-id namespace, selector switch, error,
or unsubscribe cannot affect the other connection's output.

This is category (ii) per `A-005` §2/§4: a new axis follows from the grammar of the invariant
itself, not from an unbounded "one more case" request.

**Reproduction / counterexample class:** implement subscriptions in a process-global map keyed
only by `sub id`, or keep one shared active selector per process. A single-connection suite can
pass O-1/O-2/O-4/O-7 while two connections with the same `id` or different selectors cross-talk:
connection B's `subscribe` changes what connection A receives. Existing
`red_ws_liveness_under_load.rs` only checks accept-loop liveness; it does not assert
per-connection subscription-state isolation.

**Condition to remove:** add an axis such as **"Граница соединения / изоляция сессий"** with
at least these values:

- violation: `подписка другого соединения меняет выдачу текущего клиента`
- violation: `одинаковый sub id в двух соединениях делит состояние`
- legitimate: `два соединения с одинаковым sub id и разными selector получают независимые потоки`

Then add a named oracle, e.g. `red_ws_connections_are_isolated`, and a mutant, e.g.
`connshare`, to `§4.5` / the future battery.

### B-2 — `unsubscribe` is normative, but has no oracle or axis value

**Severity:** BLOCKER  
**Evidence:** `docs/rfc/CT-RFC-09-ws-session.md:66-69`, `milestones/M-65-ws-session.md:71`,
`milestones/M-65-ws-session.md:123-127`

The RFC defines `unsubscribe`, and task #1 explicitly asks engine-dev to parse it. The oracle
map sends task #1 only to O-3/O-7, which cover unknown versions and selector validation. No
oracle states what must happen after `unsubscribe`.

This leaves a legal no-op implementation: parse `unsubscribe`, return success or silence, and
keep emitting frames for that `sub`. Such an implementation violates the result invariant
because the client's subscription set no longer contains that id, but the output still does.
It can also keep the subscription counted against the 16 limit.

**Reproduction / counterexample class:** implement `unsubscribe` as an ignored message. O-1
through O-8 as currently named can still pass unless the future manifest invents an
unadvertised scenario outside the oracle table.

**Condition to remove:** either add this as a value under a new lifecycle axis or add an
explicit oracle, e.g. `red_ws_unsubscribe_stops_sub_and_frees_capacity`, covering:

- after `unsubscribe(id)`, no further `snapshot`/`frame` with that `sub`
- neighboring subscriptions on the same connection continue
- the 16-subscription cap is decremented/freed after unsubscribe
- repeated/unknown-id unsubscribe returns a machine-readable `error` or named no-op semantics,
  whichever architect chooses, but the choice must be explicit

### N-1 — Unknown operation is in the axis table but not visible in the oracle table

**Severity:** NOTE  
**Evidence:** `milestones/M-65-ws-session.md:113`, `milestones/M-65-ws-session.md:71`,
`docs/rfc/CT-RFC-09-ws-session.md:231-240`

Axis 3 includes `неизвестная операция молча игнорируется`, but the named O-1..O-8 table does
not name an unknown-`op` oracle. This may be caught later by the §4.3 manifest-table
cross-check, so I am not making it a separate blocker. It should be made explicit in O-3 or
O-6 while fixing B-1/B-2.

**Condition to remove:** name the unknown-`op` scenario in the oracle table or in the future
manifest header.

## §2 — Prompt checklist

1. **§4.1 invariant result-based? PASS.** Lines 89-96 define observable socket output, not a
   case list.
2. **§4.2 six axes? REJECT.** The six are mostly structural members, not a bag, but the list is
   incomplete: `подписками другого соединения` from §4.1 is missing as a structural axis.
3. **§4.3 condition 2, legitimate scenarios? PASS for the six listed axes.** Lines 111-116 give
   a legitimate scenario for each listed axis. The new connection-isolation axis must get its
   own legitimate scenario.
4. **§4.6 independent reference? PASS.** Lines 163-178 require `gateway::snapshot(...)` from
   scratch and explicitly reject using GS-I-4/GS-I-5 as the reference.
5. **§5 forbidden list? NOTE.** The required "Запрещено / Почему" table exists and covers the
   major neighbor invariants: no RFC normative edits, no public ports, no legacy break, no
   `crates/gateway`, no process/catalog-global state, no disconnect-as-error, no contracts/risk
   surface. B-1/B-2 require adding the missing cross-connection/lifecycle prohibitions or oracles.
6. **§1.1 founder decisions? PASS.** Lines 32-45 and RFC §6 lines 254-271 record both decisions
   as facts and attach them to O-4 / O-5 / O-8.

## §3 — Verdict rationale

REJECT is required because the current axis set does not cover every member of the invariant it
itself states. This is the exact A-005 category (ii) class: a structural axis missing before the
RED set is written. If architect adds the connection-isolation axis and makes unsubscribe
observable, the remaining notes are not blockers.

## §4 — Next action

Architect should revise `milestones/M-65-ws-session.md` before dev dispatch:

1. add connection/session-isolation axis + oracle + mutant;
2. add unsubscribe lifecycle oracle or explicit lifecycle axis;
3. make unknown-`op` explicit in O-3/O-6 or in the future manifest.

After that, re-run critic. If the next round repeats this same structural-axis dispute, route to
arbiter per `gates.md` §0 rather than doing a third loop.
