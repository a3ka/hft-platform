# C-023 — D6 App-plane / D1 Path-1 Doc Gate

**Date (UTC):** 2026-07-23T14:52:47Z
**Agent:** critic
**Branch audited:** `origin/docs/cockpit-d6-appplane` at `ab9aba6`
**Base:** `origin/main` at `b8c60db`
**Scope:** doc-gate §9 Class A; docs-only coherence audit
**Verdict:** REJECT

## Audited Diff

```text
M docs/07-cockpit-backend-roadmap.md
M docs/fa/ai-copilot.md
M docs/fa/viz-backend.md
```

No code, contracts, Cargo files, RED tests, verify scripts, or `crates/contracts` changes are present.
`git diff --check origin/main..HEAD` is clean.

## Blocking Finding

### B1 — D1/D6 recording still contains a live Fastify transport contradiction

The new decision text says Path 1 is accepted: Rust `gateway-serve` holds WS directly and Fastify
middle-tier is cancelled:

- `docs/07-cockpit-backend-roadmap.md:59` — Rust `gateway-serve` holds WS directly; Fastify
  middle-tier is not the required layer.
- `docs/07-cockpit-backend-roadmap.md:66` — market-plane is Rust `gateway-serve`; application-plane
  is Next.js + Postgres; Rust only verifies a JWT and does not access the user DB.
- `docs/fa/viz-backend.md:22-29` — the same market-plane/app-plane split is recorded as VB-I-9.

But the same FA still describes Read Gateway WS-push as going through founder Fastify:

- `docs/fa/viz-backend.md:63-66` — Read Gateway still says incremental WS-push is "поверх
  Fastify founder'а".

This is not a disagreement with the founder decision. It is an internal recording contradiction in an
authoritative FA. A future transport milestone can read §1 as direct Rust WS and §3 as Fastify-backed WS,
which is exactly the ambiguity D6 is meant to remove.

Secondary stale context:

- `docs/07-cockpit-backend-roadmap.md:23-24` still lists the frontend as `Next15 + lightweight-charts v5
  + Fastify`. If Fastify is fully cancelled rather than merely no longer a market-data middle tier, this
  line is stale too. If it is historical context only, it needs to stop reading as current stack authority.

## Checks That Passed

- **Scope:** docs-only, limited to the three named files.
- **DET-I-1:** D6 keeps user data, chats, strategies, and AI audit output outside the market journal.
- **VB-I-3/VB-I-9:** stateless JWT verification in `gateway-serve` is read-only if it remains signature
  verification only, with no user-DB lookup and no journal/order writer surface.
- **AI-I-1:** `ai-copilot` writes app audit/chat state to Postgres, not to the market journal.
- **Boundary A/B/C:** the decision does not add order-egress or a runtime parameter-write path. AI remains
  advisory/read-only unless a future founder-signed Boundary C mode is introduced.
- **T1 governance:** no T1 contract file changed; `Event`/`EventKind` are untouched.
- **VB-I-9 testability:** the proposed grep canary against Postgres/sqlx/diesel imports in gateway is a
  testable invariant for the future `gateway-serve` milestone.

## Non-blocking Notes

- `milestones/M-22-read-gateway.md:52-56` is historical M-22 text: WS transport is outside the deterministic
  M-22 library core, and the reference `gateway-serve` task was optional there. That does not block D6 by
  itself, but the next transport milestone should treat D6 as superseding the old Fastify-or-reference choice.
- JWT algorithm/provider choice remains a founder decision and is not evaluated here.

## Handoff

REJECT -> return to `architect` for a narrow doc re-spin that removes the remaining Fastify ambiguity from
the D6/D1 record. Do not re-open Path 1; only make the accepted decision internally coherent. Then route
back through critic before reviewer merge.
