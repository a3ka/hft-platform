#!/usr/bin/env bash
# red_m88_mutants.sh — БАТАРЕЯ МУТАНТОВ M-88 (анти-плацебо RED-набора).
#
# Основание — `A-036` §5.3: шаг `R4` гейта перестал быть текстовым предикатом и зовёт эту
# батарею. Прецедент формы — `scripts/verify_M-65.sh` F2 (шаг КРАСЕН, пока батареи нет).
#
# ЧТО ДОКАЗЫВАЕТСЯ. Зелёный набор сам по себе не значит ничего: он мог бы быть зелёным и
# против сломанной реализации. Батарея ломает реализацию ДВУМЯ изолированными способами и
# требует, чтобы набор покраснел — с НАЗВАННЫМ упавшим тестом, а не «где-то что-то упало».
#
# ИЗОЛИРОВАННОСТЬ СУЩЕСТВЕННА (`A-036` §5.3, `C-233` R4): на сегодняшнем коде обе семантики
# сломаны сразу, и вторую мутацию отдельно не предъявить — поэтому батарея гоняется ПОСЛЕ
# GREEN реализации и каждый мутант применяется к здоровому коду по одному.
#
# Прогон: bash scripts/tests/red_m88_mutants.sh --battery
#
# БЕЗОПАСНОСТЬ: файл реализации мутируется В ТЕКУЩЕМ дереве и ВОССТАНАВЛИВАЕТСЯ всегда —
# `trap` срабатывает и при ошибке, и при прерывании. Оставить дерево мутированным нельзя:
# следующий прогон судил бы сломанный код, не зная об этом.

set -uo pipefail
cd "$(dirname "$0")/../.."

GW=crates/gateway/src/lib.rs
SUITE="red_m88_update_contract"
BACKUP="$(mktemp /tmp/m88-mutants-backup-XXXXXX.rs)"
FAILURES=0

cleanup() {
  if [ -f "$BACKUP" ]; then
    cp "$BACKUP" "$GW"
    rm -f "$BACKUP"
  fi
}
trap cleanup EXIT INT TERM

cp "$GW" "$BACKUP"

pass() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAILURES=$((FAILURES + 1)); }

# ─────────────── SETUP-СТРАЖ: база обязана быть ЗЕЛЁНОЙ ───────────────
# Мутация против уже красного набора не доказывает ничего: красное останется красным.
BASE=$(cargo test -p gateway --test "$SUITE" 2>&1)
if printf '%s\n' "$BASE" | grep -qE '^test result: ok'; then
  pass "SETUP: база ЗЕЛЁНАЯ — $(printf '%s\n' "$BASE" | grep -E '^test result' | tail -1)"
else
  fail "SETUP: база НЕ зелёная — мутационный контроль невозможен. $(printf '%s\n' "$BASE" | grep -E '^test result' | tail -1)"
  echo
  echo "VERDICT: FAIL — батарея гоняется ПОСЛЕ GREEN реализации (A-036 §5.3; спека §12 шаг 2)"
  exit 1
fi

# ─────────────── общий прогон мутанта ───────────────
# $1 — имя мутанта, $2 — python-скрипт мутации, $3 — тест, который ОБЯЗАН упасть
run_mutant() {
  local name="$1" script="$2" expect_test="$3"
  cp "$BACKUP" "$GW"

  if ! python3 -c "$script"; then
    fail "$name: МУТАЦИЯ НЕ ПРИМЕНИЛАСЬ — форма кода изменилась, мутант судит не то"
    return
  fi
  if cmp -s "$BACKUP" "$GW"; then
    fail "$name: файл не изменился после мутации — сценарий вырожден"
    return
  fi

  local out
  out=$(cargo test -p gateway --test "$SUITE" 2>&1)
  if printf '%s\n' "$out" | grep -qE "^test ${expect_test} \.\.\. FAILED"; then
    pass "$name: нейтрализация → тест ${expect_test} FAILED"
  elif printf '%s\n' "$out" | grep -qE '^test result: FAILED'; then
    local who
    who=$(printf '%s\n' "$out" | grep -E '^test .* FAILED' | head -3 | tr '\n' ' ')
    fail "$name: набор покраснел, но НЕ на ожидаемом тесте (${expect_test}); упали: ${who}"
  else
    fail "$name: набор ОСТАЛСЯ ЗЕЛЁНЫМ — оракулы не пиннят эту семантику"
  fi
  cp "$BACKUP" "$GW"
}

# ─────────────── МУТАНТ А: возврат к ОБЪЕДИНЕНИЮ ───────────────
# Снимается фильтр по объявленным колонкам: existing сохраняется ЦЕЛИКОМ, то есть снятый
# уровень снова живёт вечно — исходный дефект M-88.
MUT_A='
import sys
p = "crates/gateway/src/lib.rs"
s = open(p, encoding="utf-8").read()
old = "        if !observed.contains(&cell.time_s) {"
if old not in s:
    sys.exit(1)
s = s.replace(old, "        if true {", 1)
open(p, "w", encoding="utf-8").write(s)
'
run_mutant "МУТАНТ-А (merge_heatmap → объединение)" "$MUT_A" "s1_removed_level_disappears_in_current_bucket"

# ─────────────── МУТАНТ Б: пустой полный срез = отсутствие обновления ───────────────
# Объявленные колонки игнорируются, когда срез пуст: «наблюдали пустоту» снова неотличимо
# от «не наблюдали». Это вторая половина дефекта, и она обязана иметь СВОЙ оракул.
MUT_B='
import sys, re
p = "crates/gateway/src/lib.rs"
s = open(p, encoding="utf-8").read()
old = "    let observed: std::collections::BTreeSet<i64> =\n        incoming_observed_time_s.iter().copied().collect();"
if old not in s:
    sys.exit(1)
new = ("    let observed: std::collections::BTreeSet<i64> = if incoming.is_empty() {\n"
       "        std::collections::BTreeSet::new()\n"
       "    } else {\n"
       "        incoming_observed_time_s.iter().copied().collect()\n"
       "    };")
s = s.replace(old, new, 1)
open(p, "w", encoding="utf-8").write(s)
'
run_mutant "МУТАНТ-Б (пустой полный срез → NoChange)" "$MUT_B" "s3_empty_full_slice_replaces_previous"

# ─────────────── итог ───────────────
cp "$BACKUP" "$GW"
printf '\n'
if [ "$FAILURES" -eq 0 ]; then
  echo "VERDICT: PASS — обе изолированные мутации красят набор на названных тестах"
  exit 0
fi
echo "VERDICT: FAIL — расхождений: $FAILURES"
exit 1
