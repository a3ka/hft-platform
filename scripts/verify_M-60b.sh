#!/usr/bin/env bash
# Acceptance-гейт M-60b — механизмы гейтов: бюджет чтения (G1), GATE-META (G3),
# диск-преамбула (G6.2). Спека: milestones/M-60b-gate-mechanisms.md §11.
#
# ⚠ СЕЙЧАС КРАСЕН ПО ПОСТРОЕНИЮ — И ОБЯЗАН БЫТЬ КРАСНЫМ: барьеров
# check_context_budgets.sh / check_gate_meta.sh / check_disk_budget.sh НЕ СУЩЕСТВУЕТ
# (RED-first: этот гейт — спецификация dev-цикла, а не проверка постфактум). Зеленеет он
# ТОЛЬКО реализацией барьеров по контрактам RED-проб. Любая правка ЭТОГО файла или проб
# ради зелени без реализации — дефект класса «анти-плацебо» (testing.md), не фикс.
#
# Решение по КОДУ ВОЗВРАТА (gates.md §3). Агрегатор со счётчиком: печатаем все нарушения
# разом, exit 1 при FAIL>0 — первый красный шаг не должен скрывать остальные.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

CB_BARRIER=scripts/check_context_budgets.sh
GM_BARRIER=scripts/check_gate_meta.sh
DB_BARRIER=scripts/check_disk_budget.sh
CB_PROBE=scripts/tests/red_context_budgets.sh
GM_PROBE=scripts/tests/red_gate_meta.sh
DB_PROBE=scripts/tests/red_disk_budget.sh
CI=.github/workflows/ci.yml

# Число исполненных проверок пробы СЧИТАЕТСЯ, а не цитируется из шапки (урок verify_M-59:
# литерал, живущий отдельно от предмета, врёт). declared — по вызовам run_barrier в файле
# пробы; executed — из VERDICT-строки, которую проба печатает от своего счётчика.
declared_of() { grep -cE '^(if )?run_barrier ' "$1"; }
executed_of() { grep -oE 'VERDICT: PASS \(([0-9]+)/' "$1" | grep -oE '[0-9]+' | head -1; }

run_probe() { # $1=шаг $2=проба $3=описание
  local step="$1" probe="$2" what="$3" log declared executed
  log="$(mktemp /tmp/verify-m60b-XXXXXX.log)"
  if bash "${probe}" >"${log}" 2>&1; then
    declared="$(declared_of "${probe}")"
    executed="$(executed_of "${log}" || true)"
    if [ -n "${executed}" ] && [ "${executed}" -ge 1 ] && [ "${executed}" -eq "${declared}" ]; then
      pass "${step} ${what}: зелёная, счёт сошёлся (${executed}/${declared} по факту файла)"
    else
      fail "${step} ${what}: зелёная, но счёт НЕ сошёлся (исполнено «${executed:-0}», в файле ${declared}) — часть сценариев не исполнилась"
    fi
  else
    fail "${step} ${what}: КРАСНАЯ — $(grep -E '^(VERDICT|SETUP)' "${log}" | head -1)"
    grep -E '^(FAIL|SETUP)' "${log}" | head -6 | sed 's/^/      ↳ /'
  fi
  rm -f "${log}"
}

echo "--- Pre: диск-преамбула — прод-форма вызова (самореференция; задача 9) ---"
# Этот verify сам зовёт check_disk_budget.sh ПЕРВОЙ проверкой — той же формой, какой
# обязан звать каждый новый verify. Красный ЗДЕСЬ — либо барьера нет (RED-стадия),
# либо диск/target реально негодны: «названный красный» вместо ENOSPC посреди прогона.
if [ -f "${DB_BARRIER}" ]; then
  if bash "${DB_BARRIER}"; then
    pass "Pre ${DB_BARRIER} — диск и CARGO_TARGET_DIR в норме"
  else
    fail "Pre ${DB_BARRIER} назвал красное — см. первую строку его вывода выше"
  fi
else
  fail "Pre ${DB_BARRIER} не существует (RED-стадия: задача 9 не сделана)"
fi

echo "--- A: барьеры существуют и парсятся (задачи 2, 4-6, 9) ---"
for b in "${CB_BARRIER}" "${GM_BARRIER}" "${DB_BARRIER}"; do
  if [ -f "${b}" ] && bash -n "${b}" 2>/dev/null; then
    pass "A ${b} на месте и парсится"
  else
    fail "A ${b} отсутствует или не парсится"
  fi
done

echo "--- C: проба бюджета CB-1..CB-6b (задачи 1-2) ---"
run_probe "C" "${CB_PROBE}" "red_context_budgets"

echo "--- G: проба GATE-META GM-1..GM-19 (задачи 3-6) ---"
run_probe "G" "${GM_PROBE}" "red_gate_meta"
# GM-16 СОЖЖЁН (спека §4, отступление от C-064 F-064-3 — названо в шапке пробы):
# сценария с этим номером быть НЕ ДОЛЖНО; упоминание в шапке-обосновании — законно.
if grep -qE '(pass|fail) +"GM-16' "${GM_PROBE}"; then
  fail "G в пробе появился СЦЕНАРИЙ GM-16 — класс «барьер вычисляет предмет» вернулся (M-61-Б)"
else
  pass "G GM-16 сожжён: сценария нет (упоминание только в шапке-обосновании)"
fi
for gm in GM-17 GM-18 GM-19; do
  if grep -qE "(pass|fail) +\"${gm}" "${GM_PROBE}"; then
    pass "G сценарий ${gm} (отсутствие вердикта, К-4) присутствует"
  else
    fail "G сценария ${gm} нет — проверка ОТСУТСТВИЯ выпала, «закрывает TD-105» стало ложью"
  fi
done

echo "--- B: проба диск-преамбулы DB-1..DB-6 (задачи 8-9) ---"
run_probe "B" "${DB_PROBE}" "red_disk_budget"

echo "--- D: default-режим бюджета на КОРНЕ репо (F-064-4; задача 2) ---"
# Проба ходит только через BUDGET_TABLE — пустая ВСТРОЕННАЯ таблица прошла бы все сценарии.
# Поэтому здесь: (D1) default-прогон на корне зелёный; (D2) на пустом дереве — красный
# (значит встроенная таблица непуста и её файлы ОБЯЗАТЕЛЬНЫ); (D3) исчезновение КАЖДОГО
# файла ядра ловится поимённо. Всё — поведением барьера, не чтением его текста.
if [ -f "${CB_BARRIER}" ]; then
  if bash "${CB_BARRIER}" >/dev/null 2>&1; then
    pass "D1 default-режим на корне репо зелёный"
  else
    fail "D1 default-режим на корне репо КРАСНЫЙ — ядро не в бюджете либо таблица кривая"
  fi
  EMPTY="$(mktemp -d /tmp/verify-m60b-empty-XXXXXX)"
  if ( ROOT="${EMPTY}" bash "${CB_BARRIER}" >/dev/null 2>&1 ); then
    fail "D2 пустое дерево ПРОШЛО default-режим — встроенная таблица пуста или не обязательна"
  else
    pass "D2 пустое дерево красное — встроенная таблица непуста, файлы обязательны"
  fi
  rm -rf "${EMPTY}"
  CORE_FILES=(.claude/rules/branch-hygiene.md .claude/rules/commit-discipline.md \
              .claude/rules/gates.md .claude/rules/handoff-block.md \
              .claude/rules/scope-guard.md .claude/rules/testing.md CLAUDE.md)
  for f in "${CORE_FILES[@]}"; do
    T="$(mktemp -d /tmp/verify-m60b-minus-XXXXXX)"
    mkdir -p "${T}/.claude/rules"
    cp .claude/rules/*.md "${T}/.claude/rules/" && cp CLAUDE.md "${T}/"
    rm -f "${T}/${f}"
    if ( ROOT="${T}" bash "${CB_BARRIER}" >/dev/null 2>&1 ); then
      fail "D3 исчезновение ${f} НЕ поймано — файл не накрыт встроенной таблицей поимённо"
    else
      pass "D3 ${f} накрыт таблицей (исчезновение ловится)"
    fi
    rm -rf "${T}"
  done
else
  fail "D ${CB_BARRIER} не существует — default-режим проверять не на чем (RED-стадия)"
fi

echo "--- W: проводка CI — РАЗБОРОМ workflow, не грепом (F-064-2; задача 10) ---"
# Разбор: выделяем БЛОК конкретного джоба (от «  <job>:» до следующего ключа того же
# уровня) и требуем внутри него НАСТОЯЩУЮ run:-строку (комментарий начинается с «#» и
# формой «^\s*run:» не проходит). Имя скрипта в комментарии греп бы удовлетворило — блок+run: нет.
job_block() { awk -v job="$1" '
  $0 ~ "^  "job":" {inb=1; next}
  inb && /^  [A-Za-z0-9_-]+:/ {inb=0}
  inb {print}' "${CI}"; }
for j in context-budgets gate-meta; do
  if [ -n "$(job_block "$j")" ]; then
    pass "W джоб ${j} существует в ${CI}"
  else
    fail "W джоба ${j} нет в ${CI} (RED-стадия: задача 10 не сделана)"
  fi
done
job_block context-budgets | grep -qE '^\s*run: bash scripts/check_context_budgets\.sh' \
  && pass "W context-budgets зовёт барьер настоящей run:-строкой" \
  || fail "W в блоке context-budgets нет run: bash scripts/check_context_budgets.sh"
job_block context-budgets | grep -qE '^\s*run: bash scripts/tests/red_context_budgets\.sh' \
  && pass "W context-budgets гоняет свою red-пробу" \
  || fail "W в блоке context-budgets нет run: своей red-пробы (анти-плацебо проводки)"
job_block gate-meta | grep -qE '^\s*run: bash scripts/check_gate_meta\.sh' \
  && pass "W gate-meta зовёт барьер настоящей run:-строкой" \
  || fail "W в блоке gate-meta нет run: bash scripts/check_gate_meta.sh"
job_block gate-meta | grep -qE '^\s*run: bash scripts/tests/red_gate_meta\.sh' \
  && pass "W gate-meta гоняет свою red-пробу" \
  || fail "W в блоке gate-meta нет run: своей red-пробы"
job_block gate-meta | grep -qE '^\s*fetch-depth: 0' \
  && pass "W gate-meta несёт fetch-depth: 0 (иначе ревизии шапок нерезолвимы)" \
  || fail "W у gate-meta нет fetch-depth: 0 — depth=1 сделает каждый настоящий SHA ложным FAIL"
grep -qE '^\s*run: bash scripts/tests/red_disk_budget\.sh' "${CI}" \
  && pass "W red_disk_budget.sh гоняется в CI (сам чек — преамбула verify, не джоб: §8 спеки)" \
  || fail "W red_disk_budget.sh не гоняется в CI"
NEEDS="$(job_block status-check | grep -E '^\s*needs:' | head -1)"
for j in context-budgets gate-meta; do
  if echo "${NEEDS}" | grep -q "${j}"; then
    pass "W status-check.needs включает ${j}"
  else
    fail "W status-check.needs НЕ включает ${j} — без членства провал джоба ничего не блокирует; needs=«${NEEDS}»"
  fi
done

echo "--- T: шаблон GATE-META в gates.md §4 (задача 7) ---"
TMPL="$(awk '/<!-- GATE-META/{inb=1} inb{print} inb&&/-->/{exit}' .claude/rules/gates.md)"
if [ -n "${TMPL}" ]; then
  LINES="$(printf '%s\n' "${TMPL}" | wc -l)"
  if [ "${LINES}" -le 8 ]; then
    pass "T шаблон GATE-META в gates.md есть, ${LINES} строк (≤8)"
  else
    fail "T шаблон GATE-META раздут: ${LINES} строк при лимите 8"
  fi
  MISS=""
  for fld in milestone: audited_repo: audited_base: audited_head: verdict:; do
    printf '%s\n' "${TMPL}" | grep -q "${fld}" || MISS="${MISS} ${fld}"
  done
  if [ -z "${MISS}" ]; then
    pass "T поля шаблона совпадают с контрактом пробы"
  else
    fail "T в шаблоне нет полей:${MISS} — шаблон и контракт барьера разошлись"
  fi
else
  fail "T шаблона GATE-META в .claude/rules/gates.md нет (RED-стадия: задача 7 не сделана)"
fi

echo "--- P: регресс соседних барьеров (Forbidden §2 — судится только прогоном их проб) ---"
for p in red_protected_artifacts red_docs_freeze red_artifact_ids red_commit_paths; do
  LOG="$(mktemp /tmp/verify-m60b-XXXXXX.log)"
  if bash "scripts/tests/${p}.sh" >"${LOG}" 2>&1; then
    N="$(executed_of "${LOG}" || true)"
    if [ -n "${N}" ] && [ "${N}" -ge 1 ]; then
      pass "P ${p}: зелёная (${N} исполнено — счёт из её собственного счётчика)"
    else
      fail "P ${p}: зелёная, но исполнено «${N:-0}» — пустой прогон (VERDICT: PASS (0/0) — урок M-60a)"
    fi
  else
    fail "P ${p}: КРАСНАЯ — соседний барьер сломан этим milestone'ом"
    grep -E '^(FAIL|SETUP)' "${LOG}" | head -4 | sed 's/^/      ↳ /'
  fi
  rm -f "${LOG}"
done

echo "--- CI-паритет: базовый джоб целиком (gates.md §3 — гейт не смеет быть зеленее CI) ---"
cargo fmt --all -- --check >/dev/null 2>&1 \
  && pass "CI cargo fmt --check" || fail "CI cargo fmt --check"
cargo clippy --all-targets --all-features -- -D warnings >/dev/null 2>&1 \
  && pass "CI cargo clippy -D warnings" || fail "CI cargo clippy -D warnings"
cargo test --all >/dev/null 2>&1 \
  && pass "CI cargo test --all" || fail "CI cargo test --all"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  exit 1
fi
echo "VERDICT: PASS"
