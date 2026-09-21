#!/usr/bin/env bash
# verify_M-30.sh — acceptance-гейт M-30 book gap-detection (architect-owned, sacred).
# GREEN-гейт: PASS только когда apply_l2delta/ContinuityStatus реализованы и GD-I-1..6 зелёные. RED → FAIL.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2
FAIL=0
note() { printf '%s\n' "$*"; }
check() { if [ "$2" -eq 0 ]; then note "PASS $1"; else note "FAIL $1"; FAIL=$((FAIL+1)); fi; }

# --- RN-17 + TD-035: verify ⊇ CI-гейты, ТЕ ЖЕ команды + toolchain-пин (rust-toolchain.toml=1.97.0) ---
cargo fmt --all -- --check 2>&1 | tail -10
check "cargo fmt --all -- --check (matches CI)" "${PIPESTATUS[0]}"

cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -15
check "cargo clippy --all-targets --all-features -D warnings (БИТ-В-БИТ команда ci.yml, TD-035)" "${PIPESTATUS[0]}"

# --- GD-I-1..6 + Books-routing ---
cargo test -p book --tests 2>&1 | tail -25
check "book tests (GD-I-1..6 + существующие M-29/L2Snapshot)" "${PIPESTATUS[0]}"

cargo test -p book --test red_gap_detection 2>&1 | tail -15
check "GD-I-1..6 (red_gap_detection: bootstrap/spot/futures/gap-fail-closed/resync/Books-route)" "${PIPESTATUS[0]}"

# --- M-29 apply_delta + L2Snapshot-путь не сломаны (регрессия) ---
cargo test -p book >/dev/null 2>&1
check "M-29 apply_delta и L2Snapshot-путь не сломаны (cargo test -p book exit)" $?

echo "-----"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL failed)"; exit 1; fi
