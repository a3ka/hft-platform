#!/usr/bin/env bash
# verify_M-23.sh — acceptance-гейт M-23 Heatmap+COB+Bubbles в gateway (architect-owned, sacred).
# GREEN-гейт: PASS только когда heatmap/cob/bubbles реализованы и HM-I-1..5 зелёные. RED-фаза → FAIL.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2
FAIL=0
note() { printf '%s\n' "$*"; }
check() { if [ "$2" -eq 0 ]; then note "PASS $1"; else note "FAIL $1"; FAIL=$((FAIL+1)); fi; }

# --- RN-17: verify ⊇ терминальные CI-гейты ---
cargo fmt --all -- --check 2>&1 | tail -10
check "cargo fmt --all -- --check (matches CI fmt-gate)" "${PIPESTATUS[0]}"

cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -15
check "clippy --workspace --all-targets -D warnings" "${PIPESTATUS[0]}"

# --- HM-I-1..5 + не сломаны прочие gateway-серии ---
cargo test -p gateway --tests 2>&1 | tail -30
check "gateway tests (HM-I-1..5 + GW/VW/VP)" "${PIPESTATUS[0]}"

cargo test -p gateway --test red_heatmap --test red_bubbles 2>&1 | tail -15
check "HM-I-1..5 (red_heatmap: book/window/provenance/cob/det; red_bubbles: buy-sell/not-invented)" "${PIPESTATUS[0]}"

# --- live==replay не деградировал (новые серии тоже байт-идентичны snapshot vs replay) ---
cargo test -p gateway --test red_gateway_live_eq_replay >/dev/null 2>&1
check "GW-I-3/4/8 live==replay всё ещё GREEN (heatmap/cob/bubbles не сломали свёртку)" $?

echo "-----"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL failed)"; exit 1; fi
