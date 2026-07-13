#!/usr/bin/env bash
# verify_M-07 — acceptance-гейт milestone M-07 (Strategy brain: alpha → portfolio → strategy).
# Реальный гейт per .claude/rules/gates.md §3: агрегатор FAIL-счётчика + exit≠0 на любом FAIL.
# Минимум одна проверка на задачу §Tasks. Никаких `cmd && echo PASS || echo FAIL`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

STEP_LOG="$(mktemp -t verify_m07.XXXXXX.log)"
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

# T1 (задача 1): воркспейс форматирован и без clippy-предупреждений — ВСЕ крейты
# (урок RN-8: fmt-гейт обязан покрывать все тронутые крейты, не подмножество).
check "T1a cargo fmt --all --check" cargo fmt --all -- --check
check "T1b clippy --workspace -D warnings" cargo clippy --workspace --all-targets -- -D warnings

# T2 (задача 2): alpha RED-suite — AL-I-1..5
check "T2 alpha tests (AL-I-1..5)" cargo test -p alpha

# T3 (задача 3): portfolio RED-suite — PF-I-1..4 (fail-safe кап позиции)
check "T3 portfolio tests (PF-I-1..4)" cargo test -p portfolio

# T4 (задача 4): strategy RED-suite — ST-I-1..7 (diff, in-flight, детерминизм, no-lookahead)
check "T4 strategy tests (ST-I-1..7)" cargo test -p strategy

# T5 (задача 5): sim — интеграция strategy→BacktestExchange (ST-I-8) + регрессия SM-I-*
check "T5 sim tests (ST-I-8 + SM-I-* регрессия)" cargo test -p sim
# T5b (rev 3, reviewer-находка): ФОРМА equity-кривой (D7) — одна точка на СОБЫТИЕ с филлами,
# ноль точек на бесфилловых. Отдельная строка, потому что искажение кривой не роняет
# компиляцию и не видно в агрегате: оно тихо завышает Sharpe в ValidationReport →
# trials-ledger → подпись founder'а (gates §6/§7).
check "T5b ST-I-8g/8h форма equity-кривой (D7)" cargo test -p sim --test red_strategy_backtest equity

# T6 (задача 6): research-cli на настоящем strategy-пайплайне (RC-I-* регрессия)
check "T6 research-cli tests (RC-I-* регрессия)" cargo test -p research-cli

# T7: регрессия нижних слоёв — мозг не должен был их сломать
check "T7 регрессия contracts/journal/book/signals" bash -c 'cargo test -p contracts -p journal -p book -p signals'

# T8: структурные грепы (дубль структурных тестов на уровне скрипта — ST-I-6/7 + D1/D3)
check "T8a strategy не зависит от sim/venue/journal/risk" bash -c '! grep -nE "^(sim|venue-[a-z-]+|journal|risk|killswitch|oms|tokio|reqwest|rand|fastrand) *=" crates/strategy/Cargo.toml crates/alpha/Cargo.toml crates/portfolio/Cargo.toml'
check "T8b мозг без wall-clock/rand/IO" bash -c '! grep -rnE "SystemTime|Instant::now|std::time|rand::|thread_rng|std::fs|std::net" crates/alpha/src crates/portfolio/src crates/strategy/src'
check "T8c мозг без HashMap/HashSet (детерминизм обхода)" bash -c '! grep -rnE "HashMap<|HashSet<|collections::HashMap|collections::HashSet" crates/alpha/src crates/portfolio/src crates/strategy/src'
check "T8d OrderIntent определён ровно один раз" bash -c '[ "$(grep -rln "pub struct OrderIntent" crates/*/src | wc -l)" -eq 1 ]'
check "T8e OrderIntent живёт в strategy (Слой 4)" bash -c 'grep -rln "pub struct OrderIntent" crates/*/src | grep -q "^crates/strategy/src/"'

# T9 (задача 6): грид ОБЯЗАН гонять настоящий strategy-пайплайн. Гейт — ПОВЕДЕНЧЕСКИЙ
# (C-004 C2: грепы удовлетворяются комментарием/мёртвым кодом, поэтому они здесь вторичны).
# GR-I-6/7 падают на любом harness'е, игнорирующем блок `strategy` ячейки.
check "T9a GR-I-1..7 grid на strategy-пайплайне (ПОВЕДЕНЧЕСКИЙ гейт)" cargo test -p research-cli --test red_grid_strategy
check "T9b ad-hoc harness удалён из research-cli" bash -c '! grep -rnE "struct OpenPosition|enum Action" crates/research-cli/src'
# Грепы ниже игнорируют строки-комментарии (`//`, `//!`) — упоминание в доке не считается
# использованием (C-004 C2).
check "T9c grid РЕАЛЬНО инстанцирует StrategyBacktest" bash -c 'grep -rn "StrategyBacktest" crates/research-cli/src | grep -vE "^[^:]+:[0-9]+: *//" | grep -q "StrategyBacktest::new"'
check "T9d grid РЕАЛЬНО строит стратегию" bash -c 'grep -rn "DirectionalStrategy" crates/research-cli/src | grep -vE "^[^:]+:[0-9]+: *//" | grep -q "DirectionalStrategy::new"'
check "T9e grid считает returns по D7 (equity/capital_ref)" bash -c 'grep -rn "returns_from_equity\|capital_ref_e8" crates/research-cli/src/grid.rs | grep -vE "^[^:]+:[0-9]+: *//" | grep -q .'

# T10: M-07 инертен для прода — recorder НЕ должен был получить новые зависимости
# (мозг стратегии не торгует и не пишет журнал; §8 деплой-гейт ожидает НУЛЕВОЕ изменение
# поведения recorder'а).
check "T10 recorder не зависит от мозга стратегии" bash -c '! grep -nE "^(alpha|portfolio|strategy|sim) *=" crates/recorder/Cargo.toml'

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} провалов)"
  exit 1
fi
echo "VERDICT: PASS"
