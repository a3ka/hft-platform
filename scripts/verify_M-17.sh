#!/usr/bin/env bash
# M-17 order-flow Phase A (бэкенд: сигналы + экспорт под code2alpha) — acceptance-гейт.
#
# RED-фаза: red_* ПАДАЮТ (compile-RED), пока research-dev не реализовал модули
# research_cli::{depth_series, orderflow, export} → VERDICT: FAIL (корректно). Зеленеет после impl.
#
# FAIL-агрегатор (gates.md §3): считаем провалы, exit 1 при FAIL>0. НЕ маскируем через `|| echo`.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

FAILED=0
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
pass() { echo "PASS  $*"; }
run() { local name="$1"; shift; if "$@" >/dev/null 2>&1; then pass "${name}"; else fail "${name} (\`$*\`)"; fi; }

# ── OF-I-6 депт-ряды (BID/ASK раздельно, полосы, таймфрейм → линейный график) ──────────
run "OF-I-6 depth time-series (BID≠ASK, band-монотонность, close-семантика, детерминизм)" \
  cargo test -p research-cli --test red_depth_series

# ── OF-I-2/3 trade-flow (знаковая агрессия из уже пишемой Trade.side) ───────────────────
run "OF-I-2/3 footprint + cumulative delta (знаковая buy−sell, running, стороны не перепутаны)" \
  cargo test -p research-cli --test red_footprint

# ── OF-I-4 свечи из сделок (UDF-бары под code2alpha DataFeed) ───────────────────────────
run "OF-I-4 OHLCV-бары (open=first/high=max/low=min/close=last/volume=Σsize, 1s база)" \
  cargo test -p research-cli --test red_ohlcv

# ── Структурно: сигнал OBI существует (Граница A); экспорт-формат документирован (когда готов) ─
if [ -f crates/signals/src/obi.rs ]; then
  pass "signals::obi существует (Граница A — база order-flow семьи S-002+)"
else
  fail "crates/signals/src/obi.rs отсутствует — база сигналов не на месте"
fi
if [ -d research/exports ] && ls research/exports/*.md >/dev/null 2>&1; then
  pass "research/exports/ несёт документир. формат экспорта для founder-фронта"
else
  echo "NOTE  research/exports/ ещё не создан (research-dev task 5 — схема+пример под code2alpha)"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "(red_* падают, пока research-dev не реализовал research_cli::{depth_series,orderflow,export} —"
  echo " корректная RED-фаза; гейт зеленеет после impl.)"
  exit 1
fi
echo "VERDICT: PASS"
