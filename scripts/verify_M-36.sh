#!/usr/bin/env bash
# Acceptance-гейт M-36 — gateway snapshot на проде: legacy purge + VWAP all-time.
# Покрывает CODE-контракт. Ops-purge legacy + замер latency snapshot на проде — §8 eyes-on
# (не юнит-тестируемо: удаление прод-данных + прод-масштаб). См. milestones/M-36 §Ops.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }

step "task #0 — fmt + build + clippy (T1-масштаб: build --workspace, clippy --all-targets)"
chk cargo fmt --all -- --check
chk cargo build --workspace --quiet
chk cargo clippy --all-targets --workspace --quiet -- -D warnings

step "task #1/#2 — VWAP all-time оракул (VW-I-1..4, кросс-полночь БЛЕНДИТ, не reset)"
chk cargo test -p gateway --test red_vwap --quiet

step "task #2 — live == replay (VB-I-2): build==merge байт-идентичность несёт all-time VWAP-суммы"
chk cargo test -p gateway --test red_gateway_live_eq_replay --quiet

step "task #3 — journal терпит удаление нижнего сегмента (guard прод-purge legacy)"
chk cargo test -p journal --test red_seg0_removed --quiet

step "регрессия — остальной gateway/journal suite не сломан сменой семантики VWAP"
chk cargo test -p gateway --quiet
chk cargo test -p journal --quiet

echo "---"
# ЯВНО вне гейта (§8 eyes-on, milestones/M-36 §Ops):
#  • физическое удаление legacy segment-00000000.jrnl + записи journal.legacy.json на VPS;
#  • re-probe оставшегося журнала (нет ДРУГОЙ порчи) через crates/journal/examples;
#  • замер latency gateway::snapshot на ~9GB post-purge → решение по чекпоинт-редьюсеру.
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAIL проверок упало)"
  exit 1
fi
