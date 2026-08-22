#!/usr/bin/env bash
# verify_M-24.sh — acceptance-гейт M-24 Volume Profile (SVP) в gateway (architect-owned, sacred).
# GREEN-гейт: PASS только когда VP реализован и VP-I-1..4 зелёные. RED-фаза (нет volume_profile) → FAIL.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2
FAIL=0
note() { printf '%s\n' "$*"; }
check() { if [ "$2" -eq 0 ]; then note "PASS $1"; else note "FAIL $1"; FAIL=$((FAIL+1)); fi; }

# --- RN-17: verify ⊇ терминальные CI-гейты (fmt+clippy теми же командами, что ci.yml) ---
cargo fmt --all -- --check 2>&1 | tail -10
check "cargo fmt --all -- --check (matches CI fmt-gate)" "${PIPESTATUS[0]}"

cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -15
check "clippy --workspace --all-targets -D warnings" "${PIPESTATUS[0]}"

# --- VP-I-1..4 + не сломаны GW-I/VW-I ---
cargo test -p gateway --tests 2>&1 | tail -30
check "gateway tests (VP-I-1..4 + GW-I + VW-I)" "${PIPESTATUS[0]}"

cargo test -p gateway --test red_volume_profile 2>&1 | tail -15
check "VP-I-1..4 (red_volume_profile: poc/value_area/session-reset/prices-not-invented)" "${PIPESTATUS[0]}"

# --- live==replay не деградировал (VP тоже байт-идентичен snapshot vs replay) ---
cargo test -p gateway --test red_gateway_live_eq_replay 2>&1 | grep -qE "test result: ok"
check "GW-I-3/4/8 live==replay всё ещё GREEN (VP не сломал свёртку)" $?

# --- VB-I-6 переиспользован, не переопределён: ровно ОДИН utc_session_id в gateway/src ---
SRC=$(find crates/gateway/src -name '*.rs')
if [ -n "$SRC" ]; then
  N=$(sed 's://.*::' $SRC | grep -cE 'const fn utc_session_id')
  [ "$N" -eq 1 ]; check "VB-I-6: ровно один utc_session_id (VP переиспользует, не дублирует)" $?
fi

echo "-----"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL failed)"; exit 1; fi
