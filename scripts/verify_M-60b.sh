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
#
# CI-паритет rev3 (`gates.md` §3): базовый CI-job включён целиком:
# `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
# `cargo test --all`. Замер 2026-08-14 UTC на этом дереве:
# `CARGO_TARGET_DIR=/tmp/cx-m60-cargo-test-target-20260814 /usr/bin/time -p cargo test --all`
# → exit=0, real 1531.74s (25m31.74s), user 984.33s, sys 108.20s. Cargo-шаги НЕ
# удаляются; они обёрнуты timeout'ами, чтобы зависший прогон стал `VERDICT: FAIL`, а не
# оборванным гейтом без вердикта.
#
# Специализированные CI-команды включены только по зоне M-60b: mechanism-barriers и их
# RED-пробы (`red_context_budgets.sh`, `red_gate_meta.sh`, `red_disk_budget.sh`), разбор
# CI-проводки новых барьеров, GATE-META-шаблон, защита изменяемых mechanism-артефактов.
# НЕ включено: `cargo install cargo-audit --locked` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `cargo audit` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `bash scripts/verify_delivery_M-08.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# ВКЛЮЧЕНО: `bash scripts/check_docs_freeze.sh` — зона замка §11 milestone'ом ТРОНУТА:
#   задача 7 (§3) правит `.claude/rules/gates.md` (шаблон GATE-META), а шаг T требует его
#   наличия. Прежняя редакция исключала барьер с формулировкой «зона не тронута» — она была
#   ЛОЖНА: гейт не может позеленеть иначе как правкой файла из зоны замка, барьер которой
#   из него же исключён.
# НЕ включено: `bash scripts/tests/red_gc_reclaim_args.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `pip install --quiet jsonschema` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `bash scripts/verify_contracts.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: contracts base-resolution heredoc — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `bash scripts/verify_ct_rfc_atomic.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `bash scripts/tests/red_ct_rfc_atomic.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `bash scripts/diff_contract_schema.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `bash scripts/tests/red_diff_contract_schema.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `red_artifact_ids.sh --battery` heredoc — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `bash scripts/verify_design_claims.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: `bash scripts/tests/red_verify_design_claims.sh` — идёт в CI на том же push'е, зона milestone'ом не тронута.
# НЕ включено: status-check heredoc — агрегатор результатов CI; локальный verify принимает по собственному exit-коду.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
CARGO_FMT_TIMEOUT_SECONDS="${CARGO_FMT_TIMEOUT_SECONDS:-600}"
CARGO_CLIPPY_TIMEOUT_SECONDS="${CARGO_CLIPPY_TIMEOUT_SECONDS:-1200}"
CARGO_TEST_TIMEOUT_SECONDS="${CARGO_TEST_TIMEOUT_SECONDS:-2400}"
MB="$(git merge-base origin/main HEAD 2>/dev/null || true)"

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

run_timed_cmd() {
  local label="$1" seconds="$2" log rc
  shift 2
  log="$(mktemp /tmp/verify-m60b-cmd-XXXXXX.log)"
  if timeout "${seconds}" "$@" >"${log}" 2>&1; then
    pass "${label}"
  else
    rc=$?
    if [ "${rc}" -eq 124 ]; then
      fail "${label} timeout ${seconds}s"
    else
      fail "${label} exit=${rc}"
    fi
    tail -30 "${log}" | sed 's/^/      ↳ /'
  fi
  rm -f "${log}"
}

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

echo "--- R: ПРОД-ФОРМА check_gate_meta.sh на СОБСТВЕННОМ диапазоне ветки (C-089 F-089-1) ---"
# Гейт, проверенный не тем вызовом, каким его зовёт прод, НЕ ПРОВЕРЕН. Шаг G гоняет только
# синтетическую пробу на фикстурах — и она была зелёной, когда РЕАЛЬНЫЙ джоб `gate-meta`
# на этом же диапазоне возвращал exit=1 (C-089: два вердикта без шапки). Расхождение
# «проба зелёная / CI красный» — ровно то, ради чего заведён паритет с CI (`gates.md` §3).
# Обе прод-формы: push (база из `github.event.before`) и pull_request (база из
# `event.pull_request.base.sha`) — CI триггерится обеими, и джоб подаёт обе переменные.
if [ -n "${MB}" ]; then
  GM_LOG="$(mktemp /tmp/verify-m60b-gatemeta-XXXXXX.log)"
  if ( EVENT_NAME=push PUSH_BEFORE="${MB}" PR_BASE_SHA='' bash "${GM_BARRIER}" >"${GM_LOG}" 2>&1 ); then
    pass "R прод-форма push: ${GM_BARRIER} зелён на ${MB:0:7}..HEAD (то же, что джоб gate-meta)"
  else
    fail "R прод-форма push: ${GM_BARRIER} КРАСЕН на ${MB:0:7}..HEAD — джоб gate-meta провалится на этом же push'е"
    grep -E '^(FAIL|VERDICT)' "${GM_LOG}" | head -6 | sed 's/^/      ↳ /'
  fi
  if ( EVENT_NAME=pull_request PUSH_BEFORE='' PR_BASE_SHA="${MB}" bash "${GM_BARRIER}" >"${GM_LOG}" 2>&1 ); then
    pass "R прод-форма pull_request: ${GM_BARRIER} зелён на ${MB:0:7}..HEAD"
  else
    fail "R прод-форма pull_request: ${GM_BARRIER} КРАСЕН на ${MB:0:7}..HEAD"
    grep -E '^(FAIL|VERDICT)' "${GM_LOG}" | head -6 | sed 's/^/      ↳ /'
  fi
  rm -f "${GM_LOG}"
else
  fail "R merge-base с origin/main не вычислился — прод-форма gate-meta не запущена"
fi

echo "--- A: барьеры существуют и парсятся (задачи 2, 4-6, 9) ---"
for b in "${CB_BARRIER}" "${GM_BARRIER}" "${DB_BARRIER}"; do
  if [ -f "${b}" ] && bash -n "${b}" 2>/dev/null; then
    pass "A ${b} на месте и парсится"
  else
    fail "A ${b} отсутствует или не парсится"
  fi
done

echo "--- C: проба бюджета CB-1..CB-10b (задачи 1-2) ---"
run_probe "C" "${CB_PROBE}" "red_context_budgets"

echo "--- G: проба GATE-META GM-1..GM-27 (задачи 3-6) ---"
run_probe "G" "${GM_PROBE}" "red_gate_meta"
# GM-16 СОЖЖЁН (спека §4, отступление от C-064 F-064-3 — названо в шапке пробы):
# сценария с этим номером быть НЕ ДОЛЖНО; упоминание в шапке-обосновании — законно.
if grep -qE '(pass|fail) +"GM-16' "${GM_PROBE}"; then
  fail "G в пробе появился СЦЕНАРИЙ GM-16 — класс «барьер вычисляет предмет» вернулся (M-61-Б)"
else
  pass "G GM-16 сожжён: сценария нет (упоминание только в шапке-обосновании)"
fi
for gm in GM-17 GM-18 GM-19 GM-20 GM-21 GM-22 GM-23 GM-24 GM-25 GM-26 GM-27 \
          GM-28 GM-29 GM-30 GM-31; do
  if grep -qE "(pass|fail) +\"${gm}" "${GM_PROBE}"; then
    pass "G сценарий ${gm} присутствует"
  else
    fail "G сценария ${gm} нет — kill-set GATE-META неполон"
  fi
done

echo "--- B: проба диск-преамбулы DB-1..DB-12 (задачи 8-9) ---"
run_probe "B" "${DB_PROBE}" "red_disk_budget"

echo "--- N: диск-барьер — адверсарий DEFAULT-режима и symlink-побега (C-089 F-089-2/F-089-3) ---"
# Зеркало шага D, но для диска: проба ходила ТОЛЬКО с заданным MIN_FREE_KB, поэтому
# «проб-образный» барьер (работает лишь когда переменная задана) брал 13/13, а в проде не
# мерил НИЧЕГО. Здесь — вызовы, независимые от файла пробы: если пробу ослабят, эти три
# останутся. Конфаундер (реальное свободное место хоста) исключён df-shim'ом.
if [ -f "${DB_BARRIER}" ]; then
  N_TREE="$(mktemp -d /tmp/verify-m60b-disk-tree-XXXXXX)"
  N_OUT="$(mktemp -d /tmp/verify-m60b-disk-out-XXXXXX)"
  N_SHIM_LOW="${N_TREE}/.shim-low"; N_SHIM_HIGH="${N_TREE}/.shim-high"
  mkdir -p "${N_SHIM_LOW}" "${N_SHIM_HIGH}"
  printf '#!/usr/bin/env bash\necho "Filesystem 1024-blocks Used Available Capacity Mounted on"\necho "fixture 100000 95758 4242 96%%%% ."\n' > "${N_SHIM_LOW}/df"
  printf '#!/usr/bin/env bash\necho "Filesystem 1024-blocks Used Available Capacity Mounted on"\necho "fixture 2000000000 1000000000 999999999 50%%%% ."\n' > "${N_SHIM_HIGH}/df"
  chmod +x "${N_SHIM_LOW}/df" "${N_SHIM_HIGH}/df"
  N_LOG="$(mktemp /tmp/verify-m60b-disk-XXXXXX.log)"
  # N1 — DEFAULT-вызов (MIN_FREE_KB не задан вовсе) обязан РЕАЛЬНО померить диск
  if ( cd "${N_TREE}" && env -u MIN_FREE_KB -u CARGO_TARGET_DIR PATH="${N_SHIM_LOW}:${PATH}" \
         bash "${ROOT}/${DB_BARRIER}" >"${N_LOG}" 2>&1 ); then
    fail "N1 DEFAULT-режим при подставных 4242 KB ПРОШЁЛ — в проде барьер не мерит диск (F-089-3)"
  elif head -1 "${N_LOG}" | grep -q "диск:"; then
    pass "N1 DEFAULT-режим реально мерит диск (4242 KB против дефолтного порога названы диском)"
  else
    fail "N1 отказ есть, но первая строка не называет диск: «$(head -1 "${N_LOG}")»"
  fi
  # N2 — DEFAULT-вызов при заведомо достаточном месте обязан проверить CARGO_TARGET_DIR
  if ( cd "${N_TREE}" && env -u MIN_FREE_KB CARGO_TARGET_DIR="${N_OUT}" PATH="${N_SHIM_HIGH}:${PATH}" \
         bash "${ROOT}/${DB_BARRIER}" >"${N_LOG}" 2>&1 ); then
    fail "N2 DEFAULT-режим с target ВНЕ дерева ПРОШЁЛ — в проде CARGO_TARGET_DIR не проверяется (F-089-3)"
  elif head -1 "${N_LOG}" | grep -q "CARGO_TARGET_DIR"; then
    pass "N2 DEFAULT-режим реально проверяет target (место есть, target вне дерева назван)"
  else
    fail "N2 отказ есть, но первая строка не называет CARGO_TARGET_DIR: «$(head -1 "${N_LOG}")»"
  fi
  # N3 — symlink-побег: путь текстуально внутри дерева, физически снаружи
  ln -s "${N_OUT}" "${N_TREE}/target-escape"
  if [ ! -L "${N_TREE}/target-escape" ]; then
    fail "N3 setup не состоялся: симлинк не создан — сценарий судил бы обычный каталог"
  elif ( cd "${N_TREE}" && env -u MIN_FREE_KB CARGO_TARGET_DIR="${N_TREE}/target-escape" \
           PATH="${N_SHIM_HIGH}:${PATH}" bash "${ROOT}/${DB_BARRIER}" >"${N_LOG}" 2>&1 ); then
    fail "N3 симлинк внутри дерева, ведущий НАРУЖУ, ПРОШЁЛ — инвариант обходится физически (F-089-2)"
  elif head -1 "${N_LOG}" | grep -q "CARGO_TARGET_DIR"; then
    pass "N3 symlink-побег пойман по физическому пути, первая строка называет CARGO_TARGET_DIR"
  else
    fail "N3 отказ есть, но первая строка не называет CARGO_TARGET_DIR: «$(head -1 "${N_LOG}")»"
  fi
  rm -f "${N_LOG}"
  rm -rf "${N_TREE}" "${N_OUT}"
else
  fail "N ${DB_BARRIER} не существует — адверсарий default-режима запускать не на чем (RED-стадия)"
fi

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
  # D3-setup: ПОЛНАЯ копия обязана быть ЗЕЛЁНОЙ. Без этого контроля D3 — плацебо самого себя:
  # если копирование сломано (или барьер красен на любой копии), каждый шаг цикла напечатает
  # «исчезновение поймано», не поймав ничего. Setup-guard на КАЖДЫЙ сценарий — `testing.md`,
  # свойство 3 целостности гейта: проба обязана падать и против несостоявшегося setup'а.
  FULL="$(mktemp -d /tmp/verify-m60b-full-XXXXXX)"
  mkdir -p "${FULL}/.claude/rules"
  cp .claude/rules/*.md "${FULL}/.claude/rules/" && cp CLAUDE.md "${FULL}/"
  if ( ROOT="${FULL}" bash "${CB_BARRIER}" >/dev/null 2>&1 ); then
    pass "D3-setup полная копия корпуса зелёная — цикл ниже судит ИСЧЕЗНОВЕНИЕ, а не сломанный setup"
  else
    fail "D3-setup полная копия корпуса КРАСНАЯ — весь цикл D3 ниже недостоверен (ловил бы не исчезновение)"
  fi
  rm -rf "${FULL}"
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

echo "--- S: замок §11 на диапазоне ветки (зона тронута задачей 7 — шаблон в gates.md) ---"
if [ -n "${MB}" ]; then
  if ( EVENT_NAME=push PUSH_BEFORE="${MB}" bash scripts/check_docs_freeze.sh >/dev/null 2>&1 ); then
    pass "S замок §11 зелёный на диапазоне ${MB:0:7}..HEAD (токены на месте)"
  else
    fail "S замок §11 КРАСНЫЙ на диапазоне ${MB:0:7}..HEAD — коммит в зоне без FOUNDER-APPROVED"
  fi
else
  fail "S merge-base с origin/main не вычислился — замок §11 не запущен"
fi

echo "--- Q: защита артефактов по диапазону ветки (зона M-60b: механизм-артефакты) ---"
if [ -n "${MB}" ]; then
  if ( EVENT_NAME=push PUSH_BEFORE="${MB}" bash scripts/check_protected_artifacts.sh >/dev/null 2>&1 ); then
    pass "Q check_protected_artifacts.sh зелёный на диапазоне ${MB:0:7}..HEAD"
  else
    fail "Q check_protected_artifacts.sh красный на диапазоне ${MB:0:7}..HEAD"
  fi
  if ( EVENT_NAME=push PUSH_BEFORE="${MB}" bash scripts/check_artifact_ids.sh >/dev/null 2>&1 ); then
    pass "Q check_artifact_ids.sh зелёный на диапазоне ${MB:0:7}..HEAD"
  else
    fail "Q check_artifact_ids.sh красный на диапазоне ${MB:0:7}..HEAD"
  fi
else
  fail "Q merge-base с origin/main не вычислился — artifact protection не запущен"
fi

echo "--- CI-паритет: базовый джоб целиком (gates.md §3 — гейт не смеет быть зеленее CI) ---"
run_timed_cmd "CI cargo fmt --check" "${CARGO_FMT_TIMEOUT_SECONDS}" cargo fmt --all -- --check
run_timed_cmd "CI cargo clippy -D warnings" "${CARGO_CLIPPY_TIMEOUT_SECONDS}" cargo clippy --all-targets --all-features -- -D warnings
run_timed_cmd "CI cargo test --all (замер rev3 1531.74s)" "${CARGO_TEST_TIMEOUT_SECONDS}" cargo test --all

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  exit 1
fi
echo "VERDICT: PASS"
