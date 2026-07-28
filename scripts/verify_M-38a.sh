#!/usr/bin/env bash
# Acceptance-гейт M-38a — CVD session-anchored ledger (TD-043, founder-подпись 2026-07-27).
# CVD сбрасывается на 00:00 UTC, per-session ledger зеркально VP; форма v6→7.
# §8 E2E на VPS (Snapshot v7 СТРОИТСЯ + фронт читает schema_version=7) — reviewer на деплой-гейте.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }

step "task #0 — fmt + build --workspace + clippy --all-targets"
chk cargo fmt --all -- --check
chk cargo build --workspace --quiet
chk cargo clippy --all-targets --workspace --quiet -- -D warnings

step "task #1/#2 — CVD reset на 00:00 UTC, per-session ledger (reset/асимметрия/множественность/3 сессии)"
chk cargo test -p gateway --test red_gateway_cvd_session --quiet

step "task #2/#3 — форма v7 (per-session base Vec) + окно через границу (2 сессии живы) + overlap-multistep"
chk cargo test -p gateway --test red_gateway_window --quiet

step "task #4 — bump GATEWAY_SCHEMA_VERSION 6→7 (константа==7 в Snapshot/Frame, C-028 K1 sacred-оракул)"
chk cargo test -p gateway --test red_gateway_schema_version --quiet

step "регрессия — export v1-аддитивность + depth-провенанс (GW-I-5/6) сохранены при bump'е"
chk cargo test -p gateway --test red_gateway_export_v2 --quiet

step "регрессия — live==replay (VB-I-2) сохранён под session-ledger"
chk cargo test -p gateway --test red_gateway_live_eq_replay --quiet

step "регрессия — весь gateway suite (VP/VWAP/heatmap/epoch) не сломан session-reset CVD"
chk cargo test -p gateway --quiet
chk cargo test -p journal --quiet

step "регрессия — gateway-serve прозрачен к v7 (JSON passthrough schema_version)"
chk cargo test -p gateway-serve --quiet

echo "---"
# Вне гейта (§8 eyes-on, reviewer): валидный JWT → Snapshot v7 СТРОИТСЯ; CVD-серия обнуляется на
# 00:00 UTC на прод-журнале VPS; конверт несёт schema_version=7. Пруф в close-out.
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAIL проверок упало)"
  exit 1
fi
