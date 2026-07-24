#!/usr/bin/env bash
# verify_M-31.sh — acceptance-гейт M-31 book eviction (TD-016) (architect-owned, sacred).
# GREEN-гейт: PASS только когда enforce_cap/reconcile_near реализованы и EV-I-1..6 зелёные. RED → FAIL.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2
FAIL=0
note() { printf '%s\n' "$*"; }
check() { if [ "$2" -eq 0 ]; then note "PASS $1"; else note "FAIL $1"; FAIL=$((FAIL+1)); fi; }

# --- RN-17 + TD-035: CI-точные команды + toolchain-пин (rust-toolchain.toml=1.97.0) ---
cargo fmt --all -- --check 2>&1 | tail -10
check "cargo fmt --all -- --check" "${PIPESTATUS[0]}"

cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -15
check "cargo clippy --all-targets --all-features -D warnings (БИТ-В-БИТ ci.yml)" "${PIPESTATUS[0]}"

# --- EV-I-1..6 ---
cargo test -p book --tests 2>&1 | tail -25
check "book tests (EV-I-1..6 + существующие M-29/M-30)" "${PIPESTATUS[0]}"

cargo test -p book --test red_eviction 2>&1 | tail -15
check "EV-I-1..6 (red_eviction: asymmetric/recon-near/cap/absence/backstop/determinism)" "${PIPESTATUS[0]}"

# --- M-29/M-30 пути не сломаны (регрессия) ---
cargo test -p book >/dev/null 2>&1
check "M-29 apply_delta / M-30 gap-detection / L2Snapshot не сломаны (cargo test -p book exit)" $?

echo "-----"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL failed)"; exit 1; fi
