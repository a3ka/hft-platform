#!/usr/bin/env bash
# verify_M-05.sh — acceptance-гейт M-05 (data-foundation). Агрегатор + FAIL-счётчик
# (gates.md §3). Написан ДО impl — сейчас FAIL (J2 runtime-RED, J3 compile-RED), станет
# PASS когда engine/venue-dev реализуют clean-shutdown/seq-из-сегмента/recover/anti-phantom.
set -uo pipefail
cd "$(dirname "$0")/.."
FAIL=0
run() { local l="$1"; shift; if "$@" >/dev/null 2>&1; then echo "  PASS  $l"; else echo "  FAIL  $l"; FAIL=$((FAIL+1)); fi; }

echo "== Journal integrity =="
run "J2 next_seq из сегмента (нет seq-reuse)" cargo test -p journal --test red_shutdown
run "J3 recover ресинк через рваный фрейм"     cargo test -p journal --test red_recover
echo "  PENDING  J1 clean-shutdown (recorder SIGTERM integration) — оракул при task 2"
FAIL=$((FAIL+1))

echo "== Deep-book quality =="
echo "  PENDING  B1 resnapshot anti-phantom (venue-binance book) — оракул при task 5"
FAIL=$((FAIL+1))

echo "== Acceptance-число (проверено вручную на прод-сегменте) =="
echo "  NOTE  recover(прод VPS segment) обязан вернуть 1 954 182 события (4 сегмента, 3 границы)"

echo "== Gates =="
run "fmt --check"        cargo fmt -p journal -p book -- --check
run "clippy journal+book (lib)" cargo clippy -p journal -p book --lib -- -D warnings

echo "--------"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL)"; exit 1; fi
