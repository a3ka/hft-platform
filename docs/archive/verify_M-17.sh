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

# ── OF-I-4 PER-PRICE footprint bins (полный footprint для custom-series фронта; C-016) ──
run "OF-I-4 footprint BINS per-price (price→{buy_vol,sell_vol,delta}, цены не слиты)" \
  cargo test -p research-cli --test red_footprint_bins

# ── OF-I-4 свечи из сделок (UDF-бары под code2alpha DataFeed) ───────────────────────────
run "OF-I-4 OHLCV-бары (open=first/high=max/low=min/close=last/volume=Σsize, 1s база)" \
  cargo test -p research-cli --test red_ohlcv

# ── Структурно: сигнал OBI существует (Граница A); экспорт-формат документирован (когда готов) ─
if [ -f crates/signals/src/obi.rs ]; then
  pass "signals::obi существует (Граница A — база order-flow семьи S-002+)"
else
  fail "crates/signals/src/obi.rs отсутствует — база сигналов не на месте"
fi
# ── ОБЯЗАТЕЛЬНО (C-016): экспорт-контракт документирован (схема + ПРИМЕР под code2alpha) ─
# M-17 OF-I-4/task 5 обещают экспорт-формат; M-19 фронт строится ПРОТИВ него. Без файла — false-green
# (impl мог отдать серии без документир./стабильного формата). Требуем: схема-спека + пример UDF-бар и
# footprint-bin + серий (LineData/HistogramData) с `export_schema_version`.
if [ -f research/exports/format.md ] && grep -q "export_schema_version" research/exports/format.md; then
  pass "OF-I-4 экспорт-формат документирован (research/exports/format.md + export_schema_version)"
else
  fail "OF-I-4 нет research/exports/format.md с export_schema_version — экспорт-контракт под code2alpha \
не зафиксирован (C-016: обещан в OF-I-4/task 5, M-19 фронт строится против него; без него false-green)"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "(red_* падают, пока research-dev не реализовал research_cli::{depth_series,orderflow,export} —"
  echo " корректная RED-фаза; гейт зеленеет после impl.)"
  exit 1
fi
echo "VERDICT: PASS"
