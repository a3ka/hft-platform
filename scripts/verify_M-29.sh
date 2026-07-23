#!/usr/bin/env bash
# verify_M-29.sh — acceptance-гейт M-29 book L2Delta-применение (architect-owned, sacred).
# GREEN-гейт: PASS только когда apply_delta реализован и BL-I-1..6 зелёные. RED-фаза → FAIL.
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

# --- BL-I-1..6 ---
cargo test -p book --tests 2>&1 | tail -25
check "book tests (BL-I-1..6 + существующие)" "${PIPESTATUS[0]}"

cargo test -p book --test red_l2delta_apply 2>&1 | tail -15
check "BL-I-1..6 (red_l2delta_apply: set/remove/asymmetry/empty-side/determinism/scale/Books-route)" "${PIPESTATUS[0]}"

# --- L2Snapshot-путь не сломан (регрессия) ---
cargo test -p book 2>&1 | grep -qE "test result: ok"
check "L2Snapshot-путь и прочие book-тесты не сломаны" $?

echo "-----"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL failed)"; exit 1; fi
