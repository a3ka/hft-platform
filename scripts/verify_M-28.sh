#!/usr/bin/env bash
# verify_M-28.sh — acceptance-гейт M-28 gateway-serve (WS-транспорт, architect-owned, sacred).
# GREEN-гейт: PASS только когда транспорт реализован и GS-I-* зелёные. RED-фаза → FAIL.
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

# --- GS-I-2/4/5: RED-набор ---
cargo test -p gateway-serve --tests 2>&1 | tail -25
check "GS-I-2/4/5 tests (cargo test -p gateway-serve)" "${PIPESTATUS[0]}"

# --- GS-I-1 (VB-I-9a): НЕТ app-БД клиента в market-транспорте (и в gateway) ---
# Канарейки по КОДУ (комментарии вырезаны sed).
SRC=$(find crates/gateway-serve/src crates/gateway/src -name '*.rs' 2>/dev/null)
if [ -n "$SRC" ]; then
  STRIPPED=$(sed 's://.*::' $SRC)
  DBHITS=$(printf '%s\n' "$STRIPPED" | grep -nE '\b(postgres|sqlx|diesel|tokio_postgres|tokio-postgres|mysql|mongodb)\b' || true)
  [ -z "$DBHITS" ]; check "no app-DB client in gateway-serve/gateway src (GS-I-1 / D6 plane separation)" $?
  [ -n "$DBHITS" ] && note "  ↳ $DBHITS"

  # --- GS-I-3: read-only — нет journal-writer в gateway-serve/src ---
  SSRC=$(find crates/gateway-serve/src -name '*.rs')
  WHITS=$(sed 's://.*::' $SSRC | grep -nE 'Journal::open|open_with|WriterConfig|\.append\(|\.flush\(' || true)
  [ -z "$WHITS" ]; check "no journal-writer in gateway-serve/src (GS-I-3 read-only)" $?
  [ -n "$WHITS" ] && note "  ↳ $WHITS"

  # --- positive: gateway-serve использует библиотеку gateway (тонкая оболочка, не дублирует редьюсеры) ---
  sed 's://.*::' $SSRC | grep -qE 'gateway::'
  check "gateway-serve uses gateway:: library (thin shell, not re-implementing reducers)" $?
else
  note "FAIL gateway-serve/src отсутствует"; FAIL=$((FAIL+1))
fi

# --- recorder НЕ зависит от gateway-serve ---
! grep -qE '^gateway-serve[[:space:]]*=' crates/recorder/Cargo.toml 2>/dev/null; check "recorder does NOT depend on gateway-serve" $?

note "NOTE: WS-сервер (task #4) + smoke (task #5) — интеграционные, НЕ детерм-оракулы; §8 деплой-гейт решающий."
echo "-----"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL failed)"; exit 1; fi
