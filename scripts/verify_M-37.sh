#!/usr/bin/env bash
# Acceptance-гейт M-37 — bounded-memory snapshot (Путь А, TD-039).
# Покрывает CODE-контракт. §8 E2E на VPS (снапшот СТРОИТСЯ + RSS bounded, замер плато) — reviewer
# на деплой-гейте (gates.md §8), не юнит-тестируемо.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }

step "task #0 — fmt + build --workspace + clippy --all-targets"
chk cargo fmt --all -- --check
chk cargo build --workspace --quiet
chk cargo clippy --all-targets --workspace --quiet -- -D warnings

step "task #2/#5 — память ограничена ОКНОМ, не историей (multi-bucket/multi-day + memory-budget)"
chk cargo test -p gateway --test red_gateway_bounded --quiet

step "task #3/#4/#6 — split-retention (CVD-база, VP whole-session) + windowed live==replay"
chk cargo test -p gateway --test red_gateway_window --quiet

step "task #7 — GATEWAY_WINDOW_MS доходит до прод-Selector (wiring gateway-serve, TD-020)"
chk cargo test -p gateway-serve --test red_serve_window_wiring --quiet

step "регрессия — live==replay (VB-I-2) сохранён под окном"
chk cargo test -p gateway --test red_gateway_live_eq_replay --quiet

step "регрессия — весь gateway/journal suite не сломан окном/эвикцией"
chk cargo test -p gateway --quiet
chk cargo test -p journal --quiet

echo "---"
# Вне гейта (§8 eyes-on, reviewer): валидный JWT → Snapshot СТРОИТСЯ (не OOM) + замер RssAnon
# (плато, не рост ~90MB/s) на прод-журнале VPS. Пруф в close-out.
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAIL проверок упало)"
  exit 1
fi
