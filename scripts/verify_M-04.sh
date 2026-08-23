#!/usr/bin/env bash
# verify_M-04 — acceptance-гейт milestone M-04 (Research core: sim + signals + research-cli).
# Реальный гейт per .claude/rules/gates.md §3: set -euo pipefail, exit≠0 на любом FAIL.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

STEP_LOG="$(mktemp -t verify_m04.XXXXXX.log)"
trap 'rm -f "${STEP_LOG}"' EXIT

FAILED=0
check() {
  local label="$1"; shift
  if "$@" >"${STEP_LOG}" 2>&1; then
    echo "PASS  ${label}"
  else
    echo "FAIL  ${label}"
    tail -20 "${STEP_LOG}"
    FAILED=$((FAILED + 1))
  fi
}

# T1: воркспейс собирается и отформатирован
check "T1a cargo fmt --check" cargo fmt --all -- --check
check "T1b clippy -D warnings" cargo clippy --workspace --all-targets -- -D warnings

# T2: sim RED-suite (SM-I-1..10; задача 2)
check "T2 sim tests" cargo test -p sim

# T3: signals RED-suite (SG-I-1..11 + OBI; задача 3)
check "T3 signals tests" cargo test -p signals

# T4: research-cli RED-suite (RC-I-1..11; задача 4)
check "T4 research-cli tests" cargo test -p research-cli

# T5: артефакты честности (задача 5) — латентность+тарифы с provenance
check "T5a latency artifact существует" bash -c 'ls research/latency/*.json >/dev/null'
check "T5b latency artifact несёт provenance" bash -c 'grep -l "provenance" research/latency/*.json >/dev/null'
check "T5c fees artifact существует + provenance" bash -c 'grep -l "provenance" research/fees/*.json >/dev/null'

# T6: регрессия — прежние крейты живы. ВНИМАНИЕ: book несёт RED-тест top_n_depth
# (carve-out C-001 C1) — T6 красный, пока engine-dev не закрыл задачу 2.
check "T6 workspace tests (contracts+journal+book)" bash -c 'cargo test -p contracts -p journal -p book'

# T7: SignalSpec-карточка S-001 (задача 7) сверена с H-карточкой
check "T7a S-001 spec существует" test -f research/specs/S-001-obi-asym.md
check "T7b H-карточка пре-регистрирована" bash -c 'grep -qi "критерии фальсификации" research/hypotheses/H-20260710-obi-asym.md'

# Задача 8 (прогон OBI + R-001) НАМЕРЕННО без check-строки (critic C-001 m1):
# она гейтится накоплением full-book данных + вердиктом risk-critic + подписью
# founder ★ — верифицируется людьми по research/reports/R-001*, не этим скриптом.

# T8: инвариантные грепы (дубль структурных тестов на уровне скрипта)
check "T8a sim не зависит от venue-*" bash -c '! grep -E "venue-(binance|hyperliquid)" crates/sim/Cargo.toml'
check "T8b нет cfg(sim) в workspace" bash -c '! grep -rn "cfg(sim)" crates/*/src'
check "T8c research-cli без LLM/сети" bash -c '! grep -riE "openai|anthropic|reqwest" crates/research-cli/Cargo.toml crates/research-cli/src'
check "T8d research-cli не трогает signals.json" bash -c '! grep -rn "signals.json" crates/research-cli/src'

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} провалов)"
  exit 1
fi
echo "VERDICT: PASS"
