#!/usr/bin/env bash
# verify_M-35.sh — acceptance-гейт M-35 (CT-RFC-05 MarginInventory + margin-inventory collector).
# Агрегатор+FAIL-счётчик. CI-точно (RN-17/TD-035).
set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
ok()  { echo "PASS: $1"; }
bad() { echo "FAIL: $1"; FAIL=$((FAIL+1)); }

# ── Гейт 0: fmt + WORKSPACE build (T1-enum-вариант ⇒ exhaustive-match во ВСЕХ крейтах!) ──────────
cargo fmt --all -- --check >/dev/null 2>&1 && ok "fmt clean" || bad "cargo fmt --all -- --check"
# КРИТИЧНО (урок reviewer 2026-07-25, класс RN-8): новый MdPayload-вариант ломает exhaustive
# `match` в journal/sim/research-cli (E0004). Скоуп `-p contracts` СЛЕП к этому → workspace-build обязателен.
cargo build --workspace >/dev/null 2>&1 \
  && ok "cargo build --workspace (все exhaustive-match покрывают MarginInventory)" \
  || bad "cargo build --workspace — E0004 non-exhaustive match на MarginInventory (journal/sim/research-cli)"
cargo clippy --workspace --all-targets --all-features -- -D warnings >/dev/null 2>&1 \
  && ok "clippy --workspace 0 warnings" || bad "clippy --workspace -D warnings"

# ── Task 1: MI-I-1 CT-RFC-05 roundtrip/аддитивность GREEN ────────────────────────────────────────
cargo test -p contracts --test ct_rfc05 >/dev/null 2>&1 \
  && ok "MI-I-1 (ct_rfc05) GREEN" || bad "MI-I-1 ct_rfc05"

# ── CT-I-4: схема == типы (regen после нового варианта) ──────────────────────────────────────────
cargo test -p contracts --test red_schema >/dev/null 2>&1 \
  && ok "CT-I-4 схема==типы (event.schema.json regen)" || bad "CT-I-4 red_schema (перегенерируй gen_schema)"

# ── Регресс: старые CT-RFC roundtrip не сломаны ──────────────────────────────────────────────────
cargo test -p contracts --test ct_rfc01 --test red_rfc02 >/dev/null 2>&1 \
  && ok "CT-RFC-01/02 регресс-GREEN" || bad "регресс ct_rfc01/red_rfc02"

# ── Task 1/2: MI-I-2/4 parse GREEN ──────────────────────────────────────────────────────────────
cargo test -p venue-binance --test red_margin_inventory >/dev/null 2>&1 \
  && ok "MI-I-2/4 (red_margin_inventory) GREEN" || bad "MI-I-2/4 red_margin_inventory"

# ── SCHEMA_VERSION == 4 (новая эпоха CT-RFC-05) ──────────────────────────────────────────────────
grep -qE 'SCHEMA_VERSION: u32 = 4' crates/contracts/src/lib.rs \
  && ok "SCHEMA_VERSION == 4" || bad "SCHEMA_VERSION не 4"

# ── MI-I-3 canary: margin-путь read-only (нет order-egress) ──────────────────────────────────────
# available-inventory poll не должен соседствовать с submit/cancel/order-подписью.
if grep -nE 'available.?inventory|available_inventory' crates/venue-binance/src/*.rs >/dev/null 2>&1; then
  if grep -nE 'newOrder|/order|submit_order|cancel_order|POST.*order' crates/venue-binance/src/*.rs >/dev/null 2>&1; then
    bad "MI-I-3 canary: order-egress рядом с margin-путём (read-only нарушен!)"
  else
    ok "MI-I-3 canary: margin-путь read-only (нет order-egress)"
  fi
else
  ok "MI-I-3 canary: margin-путь ещё не реализован (task 2) — n/a"
fi

echo "---"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL)"; exit 1; fi
