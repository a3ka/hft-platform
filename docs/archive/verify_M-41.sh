#!/usr/bin/env bash
# Acceptance-гейт M-41 — venue-hyperliquid RED-суита (риск R4, docs/08).
#
# Агрегирующий гейт: НЕ `set -e` на проверках (первый FAIL не скрывает остальные),
# FAIL-счётчик + явный exit 1. Никакого `cmd && echo PASS || echo FAIL`.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1
FAILS=0
TESTS_DIR=crates/venue-hyperliquid/tests
SRC=crates/venue-hyperliquid/src/lib.rs

ok()  { echo "PASS  $1"; }
bad() { echo "FAIL  $1"; FAILS=$((FAILS + 1)); }

echo "=== M-41 acceptance ==="
echo

# ── Задача 1: структура суиты (sacred RED-набор на месте и не выхолощен) ────────────
for f in red_parse_trades red_parse_l2book red_fail_closed_values red_malformed_envelope red_provenance_md_only; do
  if [ -s "$TESTS_DIR/$f.rs" ]; then
    ok "T1 $f.rs присутствует и непуст"
  else
    bad "T1 $f.rs отсутствует/пуст — RED-суита неполна"
  fi
done

N_TESTS=$(grep -rh '^\s*#\[test\]' "$TESTS_DIR" 2>/dev/null | wc -l)
if [ "$N_TESTS" -ge 35 ]; then
  ok "T1 в суите $N_TESTS оракулов (>= 35)"
else
  bad "T1 в суите $N_TESTS оракулов (< 35) — суита усечена"
fi

if grep -rn '#\[ignore\]\|#\[should_panic\]' "$TESTS_DIR" >/dev/null 2>&1; then
  bad "T1 найден #[ignore]/#[should_panic] в sacred-тестах:"
  grep -rn '#\[ignore\]\|#\[should_panic\]' "$TESTS_DIR" | sed 's/^/      /'
else
  ok "T1 нет #[ignore]/#[should_panic]"
fi

# ── Задача 2: публичный API-контракт суиты ──────────────────────────────────────────
if grep -rn 'use venue_hyperliquid::parse_message' "$TESTS_DIR" >/dev/null 2>&1; then
  ok "T2 суита специфицирует venue_hyperliquid::parse_message"
else
  bad "T2 суита не ссылается на parse_message — спецификация API потеряна"
fi

# ── MD-only carve-out (gates.md §5): без risk-critic ТОЛЬКО пока нет order-egress ───
EGRESS=$(sed 's://.*::' "$SRC" | grep -icE 'order|cancel|signature|wallet|private_key|/exchange' || true)
if [ "$EGRESS" -eq 0 ]; then
  ok "CARVE-OUT src без order-egress (MD-only; risk-critic не требуется)"
else
  bad "CARVE-OUT в src найдены торговые токены ($EGRESS строк) — MD-only недействителен, нужен RISK-BLOCK:"
  sed 's://.*::' "$SRC" | grep -inE 'order|cancel|signature|wallet|private_key|/exchange' | sed 's/^/      /'
fi

# ── Задачи 2–5: суита зелёная против реализации (D0–D3 устранены) ───────────────────
TEST_OUT=$(cargo test -p venue-hyperliquid 2>&1)
TEST_EXIT=$?
SUMMARY=$(echo "$TEST_OUT" | grep -E '^test result' | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f}')
if [ "$TEST_EXIT" -eq 0 ]; then
  ok "T2-T5 cargo test -p venue-hyperliquid: $SUMMARY"
else
  bad "T2-T5 cargo test -p venue-hyperliquid FAILED ($SUMMARY) — упавшие блоки:"
  echo "$TEST_OUT" | grep -E '^test .* FAILED|^error' | sed 's/^/      /' | head -25
fi

# Точечные канарейки дефектов (падение здесь при зелёном общем прогоне = переименование
# sacred-теста — правит architect, не dev):
for t in \
  "red_parse_trades trade_side_b_is_buy_official_notation D0-инверсия-сторон" \
  "red_parse_l2book missing_time_drops_message_not_fabricates_zero D1-фабрикация-ts0" \
  "red_fail_closed_values nan_price_dropped_not_zero D2-NaN-в-нули"; do
  set -- $t
  if cargo test -q -p venue-hyperliquid --test "$1" "$2" 2>&1 | grep -q '^test result: ok. 1 passed'; then
    ok "T3-T5 канарейка $3 ($2) зелёная"
  else
    bad "T3-T5 канарейка $3 ($2) НЕ зелёная/не найдена"
  fi
done

# ── Гигиена крейта ──────────────────────────────────────────────────────────────────
if cargo clippy -p venue-hyperliquid --all-targets -- -D warnings >/dev/null 2>&1; then
  ok "HYGIENE clippy -D warnings чист"
else
  bad "HYGIENE clippy -D warnings FAILED (cargo clippy -p venue-hyperliquid --all-targets)"
fi

if cargo fmt -p venue-hyperliquid -- --check >/dev/null 2>&1; then
  ok "HYGIENE rustfmt чист"
else
  bad "HYGIENE rustfmt --check FAILED"
fi

echo
if [ "$FAILS" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAILS проверок)"
  exit 1
fi
