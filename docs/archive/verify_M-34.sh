#!/usr/bin/env bash
# verify_M-34.sh — acceptance-гейт M-34 (funding-breadth: все перпы + даунсэмпл). Агрегатор+FAIL-счётчик.
set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
ok()  { echo "PASS: $1"; }
bad() { echo "FAIL: $1"; FAIL=$((FAIL+1)); }

# ── Гейт 0: fmt + clippy CI-точно (TD-035 pin 1.97.0) ───────────────────────────────────────────
cargo fmt --all -- --check >/dev/null 2>&1 && ok "fmt clean" || bad "cargo fmt --all -- --check"
cargo clippy -p venue-binance-futures --all-targets --all-features -- -D warnings >/dev/null 2>&1 \
  && ok "clippy venue-binance-futures 0 warnings" || bad "clippy venue-binance-futures -D warnings"

# ── Задача 1/2: FB-I-1 breadth GREEN ────────────────────────────────────────────────────────────
cargo test -p venue-binance-futures --test red_funding_breadth >/dev/null 2>&1 \
  && ok "FB-I-1 (red_funding_breadth) GREEN" || bad "FB-I-1 red_funding_breadth"

# ── Регресс: существующие funding/parse тесты GREEN ─────────────────────────────────────────────
cargo test -p venue-binance-futures --test red_funding --test red_funding_poll --test red_parse >/dev/null 2>&1 \
  && ok "venue funding/parse регресс-GREEN" || bad "регресс red_funding/red_funding_poll/red_parse"

# ── Задача 2: даунсэмпл 60с применён ────────────────────────────────────────────────────────────
if grep -qE 'FUNDING_POLL_PERIOD.*from_secs\(60\)' crates/venue-binance-futures/src/lib.rs; then
  ok "FUNDING_POLL_PERIOD = 60с (даунсэмпл ~1/мин)"
else
  bad "FUNDING_POLL_PERIOD не 60с (даунсэмпл не применён)"
fi

echo "---"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL)"; exit 1; fi
