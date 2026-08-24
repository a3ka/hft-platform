#!/usr/bin/env bash
# verify_M-20.sh — acceptance-гейт M-20 VWAP (session-anchored) в gateway (architect-owned, sacred).
# GREEN-гейт: PASS только когда VWAP реализован и VW-I-1..4 зелёные. RED-фаза (нет поля vwap) → FAIL.
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

# --- Task #2-4: VW-I-1..4 RED (+ не сломаны GW-I-1..8) ---
cargo test -p gateway --tests 2>&1 | tail -30
check "gateway tests (VW-I-1..4 + GW-I-1..8)" "${PIPESTATUS[0]}"

# --- Явная проверка VWAP-оракула (VW-I-1..4) ---
cargo test -p gateway --test red_vwap 2>&1 | tail -15
check "VW-I-1..4 (red_vwap: exact/i128/session-reset/per-venue)" "${PIPESTATUS[0]}"

# --- Детерминизм/bounded/read-only не деградировали (gateway-инварианты) ---
cargo test -p gateway --test red_gateway_live_eq_replay 2>&1 | grep -qE "test result: ok"
check "GW-I-3/4/8 live==replay всё ещё GREEN (VWAP не сломал свёртку)" $?

# --- i128-канарейка (VW-I-2): аккумуляция НЕ на f64/i64 — нет f64 в vwap-пути (детерминизм) ---
SRC=$(find crates/gateway/src -name '*.rs')
if [ -n "$SRC" ]; then
  # грубая эвристика: в vwap-контексте не должно быть f64-аккумуляции sum. Комментарии вырезаем.
  BADF=$(sed 's://.*::' $SRC | grep -nE 'sum.*f64|f64.*sum' || true)
  [ -z "$BADF" ]; check "no f64 sum-accumulation heuristic in gateway/src (VW-I-2 determinism)" $?
fi

echo "-----"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL failed)"; exit 1; fi
