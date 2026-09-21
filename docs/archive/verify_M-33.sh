#!/usr/bin/env bash
# verify_M-33.sh — acceptance-гейт M-33 (полоса 30–60% различима + переснята). Агрегатор+FAIL-счётчик.
set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
ok()  { echo "PASS: $1"; }
bad() { echo "FAIL: $1"; FAIL=$((FAIL+1)); }

# ── Гейт 0: fmt + clippy CI-точно (TD-035 pin 1.97.0) ───────────────────────────────────────────
cargo fmt --all -- --check >/dev/null 2>&1 && ok "fmt clean" || bad "cargo fmt --all -- --check"
cargo clippy -p research-cli --all-targets --all-features -- -D warnings >/dev/null 2>&1 \
  && ok "clippy research-cli 0 warnings" || bad "clippy research-cli -D warnings"

# ── Задача 1/2: DV-I-9 полоса [3000,6000) различима GREEN ────────────────────────────────────────
cargo test -p research-cli --test red_depth_band_3060 >/dev/null 2>&1 \
  && ok "DV-I-9 (red_depth_band_3060) GREEN" || bad "DV-I-9 red_depth_band_3060"

# ── Регресс: DV-I-1..8 остаются GREEN (расширение BANDS_BPS не ломает M-32) ──────────────────────
cargo test -p research-cli --test red_depth_lifetime --test red_orderflow_faith >/dev/null 2>&1 \
  && ok "DV-I-1..6 регресс-GREEN" || bad "DV-I-1..6 регресс red_depth_lifetime/orderflow_faith"
cargo test -p research-cli --test red_depth_scale --release >/dev/null 2>&1 \
  && ok "DV-I-7/8 bounded регресс-GREEN" || bad "DV-I-7/8 регресс red_depth_scale"

# ── Задача 2: числа 30–60% сняты в memo ─────────────────────────────────────────────────────────
M=research/data-quality/depth-lifetime-results.md
if [ -f "$M" ] && grep -qE '30.?60|3000, ?6000|\[3000,6000\)' "$M"; then
  ok "memo содержит полосу 30–60%"
else
  bad "memo не содержит числа полосы 30–60% ($M)"
fi

echo "---"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL)"; exit 1; fi
