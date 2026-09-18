# hft-platform — crypto mid-freq trading platform (working catalog)

Created 2026-07-10. Durable working directory (NOT /tmp — the hft-core-rs-explore
clone at /tmp/hft-core-rs-explore is ephemeral and dies on reboot; reusable code
will be ported FROM it, docs and new code live HERE until the founder decides the
target git repo: same `a3ka/hft-core-rs-` vs a new repo).

## Decisions agreed so far (founder, 2026-07-09/10)
- Goal: systematic trading platform, top-firm architecture DNA, realistic tier:
  **crypto mid-frequency (ms–sec)**, market-making + directional signals.
- Venues: **Hyperliquid first** (testnet available, maker rebates), **Binance second**
  (also as lead-lag signal source for HL).
- Core idiom: sequencer/event-journal, bit-identical replay, backtest=paper=live-micro=live
  as four modes of one strategy codebase.
- Risk layer: EINHARD-validator pattern ported (fail-closed pre-trade gate, independent
  kill switch, reconciliation, no-bypass-flag RED oracles, HDP approvals for param changes).
- Research platform: AI-quant agent team running hypothesis→backtest→critic→human-gate
  loop; LLM never in the hot trading loop; deterministic engines grade all homework.
- Founder's seed strategy #1: order-book imbalance rule (bid-depth 3% band vs ask-depth
  8% band, threshold — to be formalized as a parametrized SignalSpec family + grid).
- Economy directive: Fable authors architecture/specs; bulk reading/drafting → cheaper
  subagents; backtests → pure Rust compute, no LLM tokens in evaluation.

## Layout
- docs/DESIGN.md   — MASTER design document (source of truth: §-structure, invariants, roadmap)
- docs/00 — strategy origin + AI-quant team (answers to founder's 4 questions)
- docs/01 — Rust engine architecture
- docs/02 — LLM quant-desk (agent research team)
- docs/03 — integration contract (engine↔desk seam, three borders)
- docs/04 — project-building workflow (EINHARD-style operating model: roles, milestones, gates)
- docs/05 — governed contract layer (T1 schemas, contract-RFC discipline)
- research/hypotheses/ — SignalSpec drafts + hypothesis backlog (one file per hypothesis)
- research/reports/    — backtest/validation reports (reproducible artifacts)
- (to create at M-00) PROJECT-STATE.md, TECH-DEBT.md, milestones/, process/, contracts/, crates/

## Next
1. ✅ Strategy origin + AI-quant team → docs/00
2. ✅ Master design + engine/desk/seam/contract/workflow → docs/DESIGN.md + 01–05
3. Founder decision: target git repo (new clean repo recommended — old repo has no LICENSE)
4. Then: hypothesis card H-20260710-obi-asym → M-00 bootstrap → M-01 journal (P0)
