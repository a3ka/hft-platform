#!/usr/bin/env bash
# verify_M-88.sh — acceptance-гейт milestone'а M-88 «контракт обновления книго-зависимых серий».
# Спека: milestones/M-88-liquidity-removal-contract.md
#
# Решение принимается по КОДУ ВОЗВРАТА (`gates.md` §3). Форма — агрегатор с FAIL-счётчиком:
# `set -e` здесь не годится, потому что гейт обязан ПЕРЕЧИСЛИТЬ все провалы, а не умереть
# на первом; `exit 1` при FAIL>0 даёт ту же строгость.
#
# Базовая линия снимается КРАСНОЙ до работы dev'а и предъявляется в Handoff: гейт, ни разу
# не покрасневший, ничего не доказывает.

set -uo pipefail

cd "$(dirname "$0")/.."

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP  %s\n' "$1"; }

GW=crates/gateway/src/lib.rs
SPEC=milestones/M-88-liquidity-removal-contract.md

# ─────────────────── задача 1 — форма провода и бамп версии ───────────────────
if grep -qE '^\s*pub heatmap_observed_time_s: Vec<i64>,' "$GW" \
   && grep -qE '^\s*pub cob_observed: bool,' "$GW"; then
  pass "task1: поля контракта объявлены в SeriesBundle"
else
  fail "task1: нет полей heatmap_observed_time_s / cob_observed в $GW"
fi

if grep -qE '^pub const GATEWAY_SCHEMA_VERSION: u32 = 11;' "$GW"; then
  pass "task1: GATEWAY_SCHEMA_VERSION поднят до 11"
else
  fail "task1: GATEWAY_SCHEMA_VERSION не равен 11 (смена формы ⇒ бамп обязателен, VB-I-4)"
fi

# ─────────────── задача 2 — источник больше не предписывает объединение ───────────────
# Док-комментарий BookSeriesObservation.heatmap_cells (§2 п.6 спеки) САМ называл объединение
# «close-семантикой». Проверяется отсутствие именно этого предписания, а не наличие слов.
if awk '/struct BookSeriesObservation/,/^}/' "$GW" | grep -q 'замещает раннее'; then
  fail "task2: источник наблюдения по-прежнему предписывает объединение ('замещает раннее')"
else
  pass "task2: док-комментарий источника не предписывает объединение"
fi

# ─────────────── задачи 3-6 — производство и склейка (предметный набор) ───────────────
# Оракул, а не греп: семь сценариев идут через формирование → сериализацию → применение.
UP_OUT=$(cargo test -p gateway --test red_m88_update_contract 2>&1)
UP_RC=$?
UP_LINE=$(printf '%s\n' "$UP_OUT" | grep -E '^test result' | tail -1)
if [ $UP_RC -eq 0 ]; then
  pass "task3-6: red_m88_update_contract — ${UP_LINE:-GREEN}"
else
  fail "task3-6: red_m88_update_contract КРАСЕН — ${UP_LINE:-компиляция}"
  printf '%s\n' "$UP_OUT" | grep -E '^(thread |assertion|---- )' | head -30
fi

# ─────────────── задачи 1+7 — форма и явный исход применения ───────────────
FM_OUT=$(cargo test -p gateway --test red_m88_contract_form 2>&1)
FM_RC=$?
FM_LINE=$(printf '%s\n' "$FM_OUT" | grep -E '^test result' | tail -1)
if [ $FM_RC -eq 0 ]; then
  pass "task1+7: red_m88_contract_form — ${FM_LINE:-GREEN}"
else
  fail "task1+7: red_m88_contract_form КРАСЕН — ${FM_LINE:-компиляция}"
  printf '%s\n' "$FM_OUT" | grep -E '^(error|thread|assertion)' | head -20
fi

# ─────────────── задача 7 — молчаливый no-op запрещён структурно ───────────────
if grep -qE '#\[must_use\]' "$GW" && grep -qE 'pub fn apply\(&mut self, frame: &Frame\) -> ApplyOutcome' "$GW"; then
  pass "task7: apply объявляет исход и помечен must_use"
else
  fail "task7: apply не возвращает ApplyOutcome либо не помечен must_use"
fi

# ─────────────── задача 8 — ЗАМЕР размера кадра, а не обещание ───────────────
MEAS=docs/plans/m88-frame-size-measurement.md
if [ -f "$MEAS" ] && grep -qE '[0-9]{3,}' "$MEAS"; then
  pass "task8: замер размера кадра предъявлен ($MEAS)"
else
  fail "task8: нет замера размера кадра с числами ($MEAS) — утверждение спеки §4.1 п.2 не проверено"
fi

# ─────────────── задача 9 — ревизия оракулов, поимённо ───────────────
if grep -q '^## 14. Ревизия оракулов' "$SPEC"; then
  pass "task9: раздел ревизии оракулов заполнен в спеке"
else
  fail "task9: в спеке нет раздела '## 14. Ревизия оракулов' — решение по каждому изменённому ожиданию не записано"
fi

# ─────────────── паритет с CI (`gates.md` §3): гейт не может быть зеленее CI ───────────────
if cargo fmt --all -- --check >/dev/null 2>&1; then
  pass "CI-паритет: cargo fmt --all -- --check"
else
  fail "CI-паритет: cargo fmt --all -- --check"
fi

if cargo clippy --all-targets --all-features -- -D warnings >/dev/null 2>&1; then
  pass "CI-паритет: cargo clippy --all-targets --all-features -- -D warnings"
else
  fail "CI-паритет: cargo clippy --all-targets --all-features -- -D warnings"
fi

ALL_OUT=$(cargo test --all 2>&1)
ALL_RC=$?
if [ $ALL_RC -eq 0 ]; then
  pass "CI-паритет: cargo test --all"
else
  fail "CI-паритет: cargo test --all"
  printf '%s\n' "$ALL_OUT" | grep -E '^(error|test result: FAILED)' | head -20
fi

printf '\n'
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
fi
echo "VERDICT: FAIL (провалов: $FAIL)"
exit 1
