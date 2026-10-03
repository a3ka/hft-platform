#!/usr/bin/env bash
# verify_M-89.sh — acceptance-гейт milestone'а M-89 «остаток S0: работа одной выдачи
# ограничена структурно и видна снаружи процесса».
# Спека: milestones/M-89-s0-volume-guard-observability.md
#
# Решение по КОДУ ВОЗВРАТА (`gates.md` §3). Агрегатор с FAIL-счётчиком: гейт обязан
# перечислить ВСЕ провалы, а не умереть на первом.
#
# Базовая линия снимается КРАСНОЙ до работы dev'а и предъявляется в Handoff.
#
# Режимы пробы (`scripts/tests/red_verify_M-89_*.sh`) — исполняют ОДИН шаг над фикстурой,
# чтобы барьер был под пробой, а не только под доверием:
#   VERIFY_M89_MODE=task-status VERIFY_M89_SPEC=<файл>       — шаг task-status (TD-221);
#   VERIFY_M89_MODE=task5-scan  VERIFY_M89_SRC_DIR=<каталог> — сканер запрещённых имён;
#   VERIFY_M89_MODE=ci-map      VERIFY_M89_CI_FILE=<ci.yml>  — сверка таблицы CI-паритета
#                                                             (только сверка, без исполнения);
#   VERIFY_M89_MODE=task11      VERIFY_M89_COMPOSE=<compose> VERIFY_M89_CRON=<cron> — композиция.
# Старая форма `VERIFY_M89_TASK_STATUS_ONLY=1` ≡ `VERIFY_M89_MODE=task-status`.

set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP  %s\n' "$1"; }
note() { printf 'NOTE  %s\n' "$1"; }

GW=crates/gateway/src/lib.rs
JS=crates/journal/src/segments.rs
GS=crates/gateway-serve/src
SPEC="${VERIFY_M89_SPEC:-milestones/M-89-s0-volume-guard-observability.md}"
CI="${VERIFY_M89_CI_FILE:-.github/workflows/ci.yml}"
COMPOSE_FILE="${VERIFY_M89_COMPOSE:-docker-compose.yml}"
CRON_FILE="${VERIFY_M89_CRON:-deploy/cron.d/watchdog}"
MODE="${VERIFY_M89_MODE:-}"
[ "${VERIFY_M89_TASK_STATUS_ONLY:-0}" = "1" ] && MODE=task-status

# Один cargo-test бинарь → одна строка PASS/FAIL, при провале — хвост причин.
run_test() { # <метка> <описание> <cargo test args…>
  local tag="$1" desc="$2"; shift 2
  local out rc line
  out=$(cargo test "$@" 2>&1)
  rc=$?
  line=$(printf '%s\n' "$out" | grep -E '^test result' | tail -1)
  if [ $rc -eq 0 ]; then
    pass "$tag: $desc — ${line:-GREEN}"
  else
    fail "$tag: $desc КРАСЕН — ${line:-компиляция}"
    printf '%s\n' "$out" | grep -E '^(error(\[E[0-9]+\])?:|thread |SETUP|I-[0-9]|TD-|assertion)' | cut -c1-220 | head -8
  fi
}

# ─────────── круг `R-202` → `TD-221`: §Tasks НЕ ВРЁТ О СЕБЕ (перенос из verify_M-87.sh) ───────────
# Класс повторился три круга подряд (R-200 §B3 · R-201 · R-202): статус без названного остатка
# неотличим от протухшей пометки. Исправления TD-221 против редакции M-87:
#   (а) номер задачи — `[0-9]+(б|bis)?`: строка `2bis` больше не пропадает из проверки;
#   (б) `🟡`-строки ПРОВЕРЯЮТСЯ наравне с `⏳ OPEN`, а не молча отбрасываются `case`-веткой.
# ПРЕДЕЛ НАЗВАН ЧЕСТНО: барьер проверяет НАЛИЧИЕ заявления, а не его ПРАВДИВОСТЬ (мутация C
# `R-204`). Ложный, но существующий остаток пройдёт — против него круг гейта.
task_status_check() { # <spec>
  local spec="$1" bad="" open=0 line num
  [ -f "$spec" ] || { fail "task-status: спека $spec не найдена"; return; }
  while IFS= read -r line; do
    open=$((open + 1))
    num=$(printf '%s' "$line" | sed -E 's/^\| ([0-9]+(б|bis)?) \|.*/\1/')
    # Маркер ищется ПОСЛЕ колонки статуса: слово «ЧАСТИЧНО» в самой колонке `🟡 ЧАСТИЧНО` —
    # статус, а не остаток (проба s5).
    rest=$(printf '%s' "$line" | cut -d'|' -f4-)
    if ! printf '%s' "$rest" | grep -qE 'ОСТАТОК|остаток|НЕ ПОДКЛЮЧЁН|не подключён|ЧАСТИЧНО|не исполнен|долг|ОБОСНОВАНИЕ ПЕРЕПИСАНО|ещё не начат|ждёт'; then
      bad="$bad $num"
    fi
  done < <(grep -E '^\| [0-9]+(б|bis)? \| (⏳ OPEN|🟡)' "$spec")
  if [ "$open" -eq 0 ]; then
    pass "task-status: открытых/частичных задач нет — заявлять остаток нечего"
  elif [ -z "$bad" ]; then
    pass "task-status: каждая из $open открытых/частичных задач НАЗЫВАЕТ свой остаток"
  else
    fail "task-status: задачи БЕЗ названного остатка:$bad — пометка неотличима от протухшей (R-200 §B3 · R-201 · R-202; TD-221: 2bis и 🟡 теперь видны)"
  fi
}

# ─────────── задача 5: сканер запрещённых имён — В ЛЮБОЙ ФОРМЕ кода, кроме комментариев ───────────
# `C-260` N3: прежний сканер ловил лишь `struct X`/`enum X`/`trait X`/`fn x` и срезал только
# `//`-комментарии; `type CallBudget = …`, переименованная копия под тем же именем в другом
# месте, блочный комментарий `/* CallBudget */` — обходили либо путали счёт. Здесь: снять
# `/* … */` (включая многострочные) и `//…` до конца строки, затем искать имена как
# ИДЕНТИФИКАТОРЫ (`\b…\b`) — любая форма (`type`, `use`, литерал, вызов, поле) считается.
# Предел: строковый литерал `"CallBudget"` тоже засчитывается — в коде выдачи ему делать
# нечего; ложное срабатывание здесь дешевле пропуска (`testing.md`: проверка по вызову).
DEAD_NAMES='CallBudget|BudgetStop|Cancel|feed_tail_within|pump_one|PumpStep'
strip_comments() { # stdin → stdout; сначала блочные, потом строчные
  perl -0777 -pe 's{/\*.*?\*/}{}gs; s{//[^\n]*}{}g'
}
dead_name_hits() { # <каталог> → число строк-нарушителей (stdout), список — в stderr
  local dir="$1" f hits=0 h
  while IFS= read -r -d '' f; do
    h=$(strip_comments < "$f" | grep -nE "\b($DEAD_NAMES)\b" || true)
    if [ -n "$h" ]; then
      printf '%s\n' "$h" | sed -E "s|^|  $f:|" >&2
      hits=$((hits + $(printf '%s\n' "$h" | wc -l)))
    fi
  done < <(find "$dir" -type f -name '*.rs' -print0 | sort -z)
  printf '%s\n' "$hits"
}
task5_scan_check() { # <src-dir>
  local dir="$1" n
  [ -d "$dir" ] || { fail "task5: каталог $dir не найден"; return; }
  n=$(dead_name_hits "$dir" 2>/tmp/verify_m89_task5.$$)
  if [ "$n" -eq 0 ]; then
    pass "task5: бюджет-плацебо ($DEAD_NAMES) удалён из $dir — ни одного упоминания в коде (комментарии сняты)"
  else
    fail "task5: бюджет-плацебо ещё жив — $n упоминаний в коде $dir (TD-219: подключать его значило бы подключить дефект):"
    head -12 /tmp/verify_m89_task5.$$
  fi
  rm -f /tmp/verify_m89_task5.$$
}

# ─────────── CI-паритет (`gates.md` §3, `C-260` R5): ТАБЛИЦА «каждый `run:` ci.yml → пункт verify» ───────────
# Каждой команде `run:` из ci.yml соответствует РОВНО одна строка таблицы: `EXEC` (шаг
# исполняется здесь, в прод-форме или в названной локальной форме) либо `WAIVER:<предикат>`
# с причиной ПО ТРОНУТОЙ ЗОНЕ; предикат — функция `w_*`, возвращающая 0, пока waiver
# действителен (например «зона не тронута»). Сверка МЕХАНИЧЕСКАЯ в обе стороны: run без
# строки таблицы ⇒ FAIL; строка таблицы без run ⇒ FAIL (протухла). Многострочный `run: |`
# представляется ПЕРВОЙ содержательной строкой блока; форма `- run:` (шаг без `name`, как у
# агрегата `status-check`) считается наравне с `run:`. Проба обеих сторон —
# `scripts/tests/red_verify_M-89_scan.sh`.
BASE="$(git merge-base origin/main HEAD 2>/dev/null || true)"

ci_runs() { # <ci.yml> → нормализованные команды run:, по одной на строку
  awk '
    function ltrim(s) { sub(/^[[:space:]]+/, "", s); return s }
    inblk {
      if ($0 ~ /^[[:space:]]*$/) next
      match($0, /^[[:space:]]*/); n = RLENGTH
      if (ind < 0) { ind = n; l = ltrim($0)
        # УСЛОВИЕ АГРЕГАТА — по ФОРМЕ, не по тексту: оно меняется при каждом новом джобе в needs
        # (PR #232 добавил два), и дословный ключ таблицы протухал бы каждый раз. Правильность
        # самого условия держит проба red_ci_aggregate.sh (строка EXEC ниже), не эта таблица.
        if (l ~ /^if \[\[ "\$\{\{ needs\./) l = "<условие агрегата status-check>"
        print l; next }
      if (n >= ind) next
      inblk = 0
    }
    /^[[:space:]]*(- )?run:[[:space:]]*\|[[:space:]]*$/ { inblk = 1; ind = -1; next }
    /^[[:space:]]*(- )?run:[[:space:]]*/ { s = $0; sub(/^[[:space:]]*(- )?run:[[:space:]]*/, "", s); print s }
  ' "$1"
}

declare -A CI_MAP
# --- build-test: базовая тройка — EXEC всегда
CI_MAP['cargo fmt --all -- --check']='EXEC'
CI_MAP['cargo clippy --all-targets --all-features -- -D warnings']='EXEC'
CI_MAP['cargo test --all']='EXEC'
# --- security: установка инструмента — не проверка; аудит — EXEC (cargo-audit обязан быть)
CI_MAP['cargo install cargo-audit --locked']='WAIVER:w_tool_cargo_audit установка инструмента, а не проверка; предикат: cargo-audit доступен локально'
CI_MAP['cargo audit']='EXEC'
# --- delivery: структурная форма D1..; DEEP (сборка образа + запуск) — только CI (docker)
CI_MAP['bash scripts/verify_delivery_M-08.sh']='EXEC'
# --- roadmap-sync
CI_MAP['bash scripts/check_roadmap_sync.sh']='EXEC'
CI_MAP['bash scripts/tests/red_roadmap_sync.sh']='WAIVER:w_harness_untouched проба барьера; предмет не трогает scripts/check_*.sh, scripts/lib, чужие scripts/tests'
# --- secret-material
CI_MAP['bash scripts/check_secret_material.sh']='EXEC'
CI_MAP['bash scripts/tests/red_secret_material.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
# --- protected-artifacts
CI_MAP['bash scripts/check_protected_artifacts.sh']='EXEC'
CI_MAP['bash scripts/tests/red_protected_artifacts.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
CI_MAP['bash scripts/tests/red_commit_paths.sh']='WAIVER:w_harness_untouched проба pre-commit хука; .githooks не тронут'
# --- archived-refs
CI_MAP['bash scripts/check_archived_refs.sh']='EXEC'
CI_MAP['bash scripts/tests/red_archived_refs.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
# --- docs-freeze (+ пробы gc)
CI_MAP['bash scripts/check_docs_freeze.sh']='EXEC'
CI_MAP['bash scripts/tests/red_docs_freeze.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
CI_MAP['bash scripts/tests/red_gc_reclaim_args.sh']='WAIVER:w_harness_untouched проба gc_worktrees; харнесс не тронут'
CI_MAP['bash scripts/tests/red_gc_live_cwd.sh --battery']='WAIVER:w_harness_untouched проба gc_worktrees; харнесс не тронут'
# --- contracts: зона crates/contracts ЗАПРЕЩЕНА спекой §10; сами гейты дёшевы — EXEC
CI_MAP['pip install --quiet jsonschema']='WAIVER:w_py_jsonschema установка модуля, а не проверка; предикат: python3 -c "import jsonschema"'
CI_MAP['bash scripts/verify_contracts.sh']='EXEC'
CI_MAP['set -euo pipefail']='WAIVER:w_ci_plumbing шаг «Определить базу события» — проводка CI; локальная база = merge-base origin/main'
CI_MAP['bash scripts/verify_ct_rfc_atomic.sh "${{ steps.base.outputs.sha }}"']='EXEC'
CI_MAP['bash scripts/tests/red_ct_rfc_atomic.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
CI_MAP['bash scripts/diff_contract_schema.sh "${{ steps.base.outputs.sha }}"']='EXEC'
CI_MAP['bash scripts/tests/red_diff_contract_schema.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
# --- artifact-ids / reserve-ids
CI_MAP['bash scripts/check_artifact_ids.sh']='EXEC'
CI_MAP['bash scripts/tests/red_artifact_ids.sh']='WAIVER:w_harness_untouched проба барьера + батарея; харнесс не тронут'
CI_MAP['bash scripts/tests/red_reserve_id.sh']='WAIVER:w_harness_untouched проба резерва; харнесс не тронут'
CI_MAP['bash scripts/tests/red_reserve_id.sh --battery']='WAIVER:w_harness_untouched батарея резерва; харнесс не тронут'
# --- design-claims (docs тронуты — EXEC, плюс merge-preview по gates.md §8 ниже)
CI_MAP['bash scripts/verify_design_claims.sh']='EXEC'
CI_MAP['bash scripts/tests/red_verify_design_claims.sh']='WAIVER:w_harness_untouched проба гейта; харнесс не тронут'
# --- context-budgets
CI_MAP['bash scripts/check_context_budgets.sh']='EXEC'
CI_MAP['bash scripts/tests/red_context_budgets.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
# --- gate-meta (вердикты C-NNN на ветке — EXEC)
CI_MAP["git fetch --no-tags origin '+refs/salvage/*:refs/salvage/*'"]='EXEC'
CI_MAP['bash scripts/check_gate_meta.sh']='EXEC'
CI_MAP['bash scripts/tests/red_gate_meta.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
CI_MAP['bash scripts/tests/red_ci_gate_meta_refspec.sh']='WAIVER:w_harness_untouched проба проводки; .github/workflows не тронут'
CI_MAP['bash scripts/tests/red_disk_budget.sh']='WAIVER:w_harness_untouched проба диск-преамбулы; харнесс не тронут'
# --- resource-oracles (новые RED с процессным rchar — EXEC)
CI_MAP['bash scripts/check_resource_oracles.sh']='EXEC'
CI_MAP['bash scripts/tests/red_resource_oracles.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
# --- deploy-catchup (дёшево, python — EXEC)
CI_MAP['python3 scripts/deploy_catchup.py check-wiring']='EXEC'
CI_MAP['python3 scripts/check_deploy_gate.py']='EXEC'
CI_MAP['python3 scripts/deploy_catchup.py check-aggregate']='EXEC'
CI_MAP['bash scripts/tests/red_deploy_catchup.sh']='WAIVER:w_harness_untouched проба сторожа; харнесс не тронут'
CI_MAP['bash scripts/tests/red_deploy_catchup.sh --battery']='WAIVER:w_harness_untouched батарея; харнесс не тронут'
# --- branch-health: в CI `|| true` (наблюдение, не гейт) — та же форма
CI_MAP['bash scripts/check_branch_health.sh || true']='EXEC'
CI_MAP['bash scripts/tests/red_branch_health.sh --battery']='WAIVER:w_harness_untouched проба наблюдателя; харнесс не тронут'
# --- review-fa: диф трогает crates/** ⇒ вердикт R-NNN обязан назвать FA — EXEC (красен до reviewer'а)
CI_MAP['bash scripts/check_review_fa.sh']='EXEC'
CI_MAP['bash scripts/tests/red_review_fa.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
# --- rollout-composition: compose ТРОНУТ (том, две переменные) — EXEC
CI_MAP['python3 -m pip install --quiet pyyaml']='WAIVER:w_py_yaml установка модуля, а не проверка; предикат: python3 -c "import yaml"'
CI_MAP['bash scripts/check_rollout_composition.sh']='EXEC'
CI_MAP['bash scripts/tests/red_rollout_composition.sh']='WAIVER:w_harness_untouched проба барьера; харнесс не тронут'
# --- status-check: агрегат `All checks passed` — проводка
CI_MAP['<условие агрегата status-check>']='WAIVER:w_ci_plumbing агрегат «All checks passed» — проводка CI, не проверка предмета'
CI_MAP['bash scripts/tests/red_ci_aggregate.sh']='EXEC'

# Предикаты waiver'ов: 0 — waiver действителен.
harness_touched_files() {
  [ -n "$BASE" ] || { echo "<origin/main недоступен>"; return; }
  git diff --name-only "$BASE" HEAD -- 'scripts/check_*.sh' scripts/lib scripts/tests .githooks .github/workflows \
    | grep -vE '^scripts/tests/red_verify_M-89_' || true
}
w_harness_untouched() { [ -z "$(harness_touched_files)" ]; }
w_tool_cargo_audit() { command -v cargo-audit >/dev/null 2>&1; }
w_py_jsonschema() { python3 -c 'import jsonschema' >/dev/null 2>&1; }
w_py_yaml() { python3 -c 'import yaml' >/dev/null 2>&1; }
w_ci_plumbing() { [ -n "$BASE" ] && git diff --quiet "$BASE" HEAD -- .github/workflows; }

ci_map_check() { # <ci.yml> — только сверка таблицы (обе стороны)
  local f="$1" cmd missing="" stale="" n=0
  [ -f "$f" ] || { fail "CI-паритет: файл $f не найден"; return; }
  while IFS= read -r cmd; do
    [ -n "$cmd" ] || continue
    n=$((n + 1))
    if [ -z "${CI_MAP[$cmd]+x}" ]; then missing="$missing\n    $cmd"; fi
  done < <(ci_runs "$f")
  local runs; runs=$(ci_runs "$f")
  for cmd in "${!CI_MAP[@]}"; do
    printf '%s\n' "$runs" | grep -qxF -- "$cmd" || stale="$stale\n    $cmd"
  done
  if [ "$n" -eq 0 ]; then
    fail "CI-паритет: в $f не найдено ни одного run: — сверять нечего (детектор сломан)"
  elif [ -n "$missing" ]; then
    fail "CI-паритет: run: в $f БЕЗ строки таблицы соответствия (добавь EXEC либо WAIVER с причиной по зоне):$(printf "$missing")"
  elif [ -n "$stale" ]; then
    fail "CI-паритет: строки таблицы без run: в $f (протухли):$(printf "$stale")"
  else
    pass "CI-паритет: таблица соответствия покрывает все $n run: из $f и не содержит протухших строк"
  fi
}

# Исполнение EXEC-строк — в форме CI (переменные события заменены локальной базой).
ci_exec() { # <cmd>
  local cmd="$1" ev="EVENT_NAME=pull_request PR_BASE_SHA=$BASE PUSH_BEFORE=$BASE"
  case "$cmd" in
    'cargo fmt --all -- --check') cargo fmt --all -- --check >/dev/null 2>&1 ;;
    'cargo clippy --all-targets --all-features -- -D warnings') cargo clippy --all-targets --all-features -- -D warnings >/dev/null 2>&1 ;;
    'cargo test --all') cargo test --all >/dev/null 2>&1 ;;
    'cargo audit') cargo audit >/dev/null 2>&1 ;;
    'bash scripts/verify_delivery_M-08.sh') HFT_DELIVERY_DEEP=0 bash scripts/verify_delivery_M-08.sh >/dev/null 2>&1 ;;
    'bash scripts/verify_ct_rfc_atomic.sh "${{ steps.base.outputs.sha }}"') bash scripts/verify_ct_rfc_atomic.sh "$BASE" >/dev/null 2>&1 ;;
    'bash scripts/diff_contract_schema.sh "${{ steps.base.outputs.sha }}"') bash scripts/diff_contract_schema.sh "$BASE" >/dev/null 2>&1 ;;
    # review-fa судит ВЕРДИКТ reviewer'а (живой FA-инвариант в R-файле диапазона). До PR-гейта
    # вердикта нет по построению — шаг отложен и печатается SKIP с причиной; как только в
    # диапазоне появился R-файл, называющий M-89, шаг ИСПОЛНЯЕТСЯ и обязан быть зелёным.
    'bash scripts/check_review_fa.sh')
      if [ -z "$(git diff --name-only --diff-filter=A "$BASE" HEAD -- 'research/reviews/R-*.md' | xargs -r grep -l 'M-89' 2>/dev/null)" ]; then return 97; fi
      env $ev bash scripts/check_review_fa.sh >/dev/null 2>&1 ;;
    'bash scripts/tests/red_ci_aggregate.sh') bash scripts/tests/red_ci_aggregate.sh >/dev/null 2>&1 ;;
    'bash scripts/check_branch_health.sh || true') bash scripts/check_branch_health.sh >/dev/null 2>&1 || true ;;
    "git fetch --no-tags origin '+refs/salvage/*:refs/salvage/*'") git fetch --no-tags origin '+refs/salvage/*:refs/salvage/*' >/dev/null 2>&1 ;;
    bash\ scripts/check_*|bash\ scripts/verify_*) env $ev bash ${cmd#bash } >/dev/null 2>&1 ;;
    python3\ *) $cmd >/dev/null 2>&1 ;;
    *) return 99 ;;
  esac
}

ci_parity() {
  ci_map_check "$CI"
  [ -n "$BASE" ] || fail "CI-паритет: origin/main недоступен — база диапазона не определена, waiver'ы «зона не тронута» непроверяемы"
  local cmd row kind pred reason rc
  while IFS= read -r cmd; do
    [ -n "$cmd" ] || continue
    row="${CI_MAP[$cmd]-}"
    [ -n "$row" ] || continue   # уже названо в ci_map_check
    kind="${row%%:*}"
    case "$kind" in
      EXEC)
        ci_exec "$cmd"; rc=$?
        if [ $rc -eq 0 ]; then pass "CI-паритет [EXEC]: $cmd"
        elif [ $rc -eq 97 ]; then skip "CI-паритет [EXEC отложен]: $cmd — вердикта reviewer'а с M-89 в диапазоне ещё нет; судится на PR-гейте, после вердикта исполняется"
        elif [ $rc -eq 99 ]; then fail "CI-паритет [EXEC]: $cmd — строка таблицы EXEC без исполнителя в ci_exec (дефект гейта)"
        else fail "CI-паритет [EXEC]: $cmd — exit=$rc"; fi ;;
      WAIVER)
        pred="${row#WAIVER:}"; pred="${pred%% *}"; reason="${row#WAIVER:$pred }"
        if "$pred"; then pass "CI-паритет [WAIVER $pred]: $cmd — $reason"
        else fail "CI-паритет [WAIVER $pred НЕДЕЙСТВИТЕЛЕН]: $cmd — $reason; предикат не выполнен ($( [ "$pred" = w_harness_untouched ] && harness_touched_files | tr '\n' ' ' ))"; fi ;;
      *) fail "CI-паритет: строка таблицы «$row» не EXEC и не WAIVER" ;;
    esac
  done < <(ci_runs "$CI")
  # gates.md §8: документ проверяется на ДЕРЕВЕ СЛИЯНИЯ, а не на ветке.
  if [ -n "$BASE" ]; then
    if bash scripts/verify_design_claims.sh --merge-preview origin/main >/dev/null 2>&1; then
      pass "CI-паритет+: verify_design_claims.sh --merge-preview origin/main (gates.md §8: дерево слияния)"
    else
      fail "CI-паритет+: verify_design_claims.sh --merge-preview origin/main КРАСЕН — утверждения документов о коде ложны на дереве слияния"
    fi
  fi
}

# ─────────── задача 11: КОМПОЗИЦИЯ путей compose ↔ cron (функция — под пробой) ───────────
# `testing.md` §«Механизм несущего пути обязан иметь оракул точки входа», п. 2: путь producer'а и
# путь consumer'а — две строки в двух файлах, и их расхождение даёт тихий no-op.
# `C-260` R3: путь обязан быть ФАЙЛОМ — не корень монтирования, не каталог с хвостовым `/`,
# не пустой лист: `rename` в каталог невозможен, писатель проглотит ошибку, watchdog увидит MISSING.
task11_composition_check() { # <docker-compose.yml> <deploy/cron.d/watchdog>
  local compose="$1" cron="$2" hb_path mount_line host_path="" form_err="" leaf m vol rest dst mode cron_path
  [ -f "$compose" ] || { fail "task11: compose $compose не найден"; return; }
  hb_path=$(awk '/^  gateway-serve:/{s=1;next} s&&/^  [a-z]/{s=0} s&&/GATEWAY_HEARTBEAT_PATH:/{sub(/.*GATEWAY_HEARTBEAT_PATH:[ ]*/,""); gsub(/"/,""); print; exit}' "$compose")
  hb_path=${hb_path%%\#*}; hb_path=$(printf '%s' "$hb_path" | sed -E 's/^\$\{[A-Z_]+:-([^}]*)\}$/\1/' | tr -d ' ')
  mount_line=$(awk '/^  gateway-serve:/{s=1;next} s&&/^  [a-z]/{s=0} s&&/^      - /{print}' "$compose" | tr -d '"' | sed -E 's/^ *- //')
  if [ -n "$hb_path" ]; then
    leaf="${hb_path##*/}"
    case "$hb_path" in */) form_err="хвостовой / — это каталог, не файл";; esac
    [ -z "$form_err" ] && [ -z "$leaf" ] && form_err="пустой лист"
    [ -z "$form_err" ] && { [ "$leaf" = "." ] || [ "$leaf" = ".." ]; } && form_err="служебный лист $leaf"
    while IFS= read -r m; do
      [ -n "$m" ] || continue
      vol=${m%%:*}; rest=${m#*:}; dst=${rest%%:*}; mode=${rest#"$dst"}
      if [ "$hb_path" = "$dst" ] || [ "${hb_path%/}" = "$dst" ]; then form_err="корень монтирования $dst"; fi
      case "$hb_path" in
        "$dst"/?*) [ "$mode" != ":ro" ] && host_path="/var/lib/docker/volumes/hft-platform_${vol}/_data/${hb_path#"$dst"/}";;
      esac
    done <<< "$mount_line"
  fi
  cron_path=$(grep -E '^WATCHDOG_SERVING_HEARTBEAT_PATH=' "$cron" 2>/dev/null | head -1 | cut -d= -f2- | tr -d '"')
  if [ -z "$hb_path" ]; then
    fail "task11: compose не объявляет GATEWAY_HEARTBEAT_PATH у gateway-serve — композицию сверять не с чем"
  elif [ -n "$form_err" ]; then
    fail "task11: GATEWAY_HEARTBEAT_PATH=$hb_path — не путь к файлу ($form_err); <path>.tmp → rename невозможен (C-260 R3)"
  elif [ -z "$host_path" ]; then
    fail "task11: GATEWAY_HEARTBEAT_PATH=$hb_path не лежит на rw-монтировании gateway-serve (монтирования: $(echo $mount_line))"
  elif [ ! -f "$cron" ]; then
    fail "task11: нет $cron — читателю не назначен путь; на проде ops-watchdog не установлен (ssh 2026-09-27: нет /etc/cron.d/hft-watchdog, нет бинаря)"
  elif [ -z "$cron_path" ] || [ "${cron_path%/}" != "$cron_path" ] || [ -z "${cron_path##*/}" ]; then
    fail "task11: WATCHDOG_SERVING_HEARTBEAT_PATH=${cron_path:-<пусто>} в $cron — не путь к файлу (C-260 R3)"
  elif [ "$cron_path" != "$host_path" ]; then
    fail "task11: КОМПОЗИЦИЯ РАЗОШЛАСЬ — compose пишет в $host_path, cron читает $cron_path"
  elif ! grep -qE 'scripts/watchdog_cron\.sh' "$cron"; then
    fail "task11: $cron не зовёт scripts/watchdog_cron.sh — фрагмент без строки расписания"
  elif [ "$(grep -nE '^WATCHDOG_SERVING_HEARTBEAT_PATH=' "$cron" | head -1 | cut -d: -f1)" -gt \
         "$(grep -nE '^[^#]*scripts/watchdog_cron\.sh' "$cron" | head -1 | cut -d: -f1)" ]; then
    # `R-209` Н-1: в cron.d присваивание действует ТОЛЬКО на строки НИЖЕ себя — переставленная
    # копия проходила прежний греп (значение где угодно + head -1), а ops-watchdog на проде
    # получил бы дефолтный путь, то есть композиция расходилась бы молча.
    fail "task11: в $cron WATCHDOG_SERVING_HEARTBEAT_PATH= стоит НИЖЕ строки расписания — cron.d применяет присваивание только к строкам после него; ops-watchdog его не увидит"
  elif command -v crontab >/dev/null 2>&1 && ! crontab -n "$cron" >/dev/null 2>&1; then
    fail "task11: crontab -n $cron — фрагмент не устанавливается (тот же класс, что D5 M-08)"
  else
    pass "task11: композиция путей сходится: $host_path (compose ⇒ хост) == cron; файл, не каталог"
  fi
}

# ─────────── режимы пробы ───────────
case "$MODE" in
  task-status) task_status_check "$SPEC" ;;
  task5-scan)  task5_scan_check "${VERIFY_M89_SRC_DIR:?VERIFY_M89_SRC_DIR обязателен в режиме task5-scan}" ;;
  ci-map)      ci_map_check "$CI" ;;
  task11)      task11_composition_check "$COMPOSE_FILE" "$CRON_FILE" ;;
  "") ;;
  *) fail "неизвестный режим пробы VERIFY_M89_MODE=$MODE" ;;
esac
if [ -n "$MODE" ]; then
  printf '\n'
  if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
  echo "VERDICT: FAIL (провалов: $FAIL)"; exit 1
fi

# ─────────────── задача 1 — journal: договор сдвига на ПУБЛИЧНОЙ точке + счётчики стрима ───────────────
# `C-260` R2: примитив поиска — `pub(crate)`, публичного `locate_after_seq` быть НЕ ДОЛЖНО
# (иначе воскресает мутант «публичная заглушка + приватный поиск»); публичный договор —
# поведение `stream_from_at(.., Some(after), None)`, судимое `red_m89_seek_contract`.
if grep -qE '^\s*pub fn locate_after_seq\(' "$JS"; then
  fail "task1: в $JS ЕСТЬ pub fn locate_after_seq — публичный примитив запрещён формой §4.1 (C-260 R2: заглушка проходила бы греп)"
else
  pass "task1: публичного locate_after_seq в $JS нет — договор живёт на stream_from_at (§4.1)"
fi
if grep -qE 'pub fn payload_bytes_read\(&self\)' "$JS" && grep -qE 'pub fn seek_fallbacks\(&self\)' "$JS"; then
  pass "task1: EventStream несёт payload_bytes_read и seek_fallbacks (аддитивно, §4.1)"
else
  fail "task1: у EventStream в $JS нет payload_bytes_read / seek_fallbacks — форма §4.1 не выполнена"
fi
run_test "task1/договор" "red_m89_seek_contract (j1..j9: after+1, закрытый сегмент, EOF, порча, рваный, :ro, N2-эквивалентность)" -p journal --test red_m89_seek_contract
run_test "task1/откат" "red_m89_seek_fallback_observed (f1..f5: seek_fallbacks)" -p journal --test red_m89_seek_fallback_observed
# `A-042` §4 (в)/(г): стык сегментов судится ПРОТИВ ПОЛНОГО КАТАЛОГА — правый край прочитанного
# сегмента против first_seq физического преемника, левый край открытого против его шапки.
# n1..n4 красны ПОВЕДЕНЧЕСКИ (Ok через дыру вместо Err), n5 — позитивный контроль.
run_test "task1/стык" "red_m89_seek_junction (n1..n4: дыра в каталоге ⇒ Err(InvalidData); n5: законная проекция через исключённый сегмент)" -p journal --test red_m89_seek_junction
run_test "task1/байты" "red_m89_bytes_accounting (b1..b7: точный учёт payload_bytes_read)" -p journal --test red_m89_bytes_accounting
# Sacred-корпус journal — БЕЗ файлов этого набора (`red_m89_*` красны по заявлению, в т.ч.
# компиляцией; их судят три шага выше). `cargo test -p journal` целиком не годится: один
# COMPILE-RED бинарь валит сборку всех тестов крейта, и шаг краснел бы не по своему предмету.
JR_OLD_FAIL=""
for f in crates/journal/tests/*.rs; do
  n=$(basename "$f" .rs)
  case "$n" in red_m89_*) continue;; esac
  cargo test -q -p journal --test "$n" >/dev/null 2>&1 || JR_OLD_FAIL="$JR_OLD_FAIL $n"
done
if cargo test -q -p journal --lib --bins >/dev/null 2>&1 && [ -z "$JR_OLD_FAIL" ]; then
  pass "task1: sacred-корпус journal зелен (JR-I-2/JR-I-11 не задеты; red_m89_* исключены — их судят шаги выше)"
else
  fail "task1: sacred-корпус journal КРАСЕН —${JR_OLD_FAIL:- lib/bins}"
fi

# ─────────────── задачи 1+2+3 — I-1/I-2/I-4 на библиотеке (прод-масштаб, rchar ядра) ───────────────
run_test "task1+2+3" "red_m89_warm_resume_seek (w1/p1/d1..d9, d7c)" -p gateway --test red_m89_warm_resume_seek
if grep -qE '^\s*pub seek_fallbacks: u64,' "$GW"; then
  pass "task2: ReadStats несёт seek_fallbacks — откат поиска позиции НАБЛЮДАЕМ"
else
  fail "task2: в ReadStats нет seek_fallbacks — «сдвиг не сработал, всё перечиталось» невидимо в статистике"
fi
PBAC=$(grep -c 'payload_bytes_after_cursor' "$GW" || true)
if [ "$PBAC" -eq 0 ]; then
  pass "task3: payload_bytes_after_cursor удалён — read_dir+заголовки ушли с горячего пути (задача 15 M-87)"
else
  fail "task3: payload_bytes_after_cursor всё ещё в $GW ($PBAC упоминаний) — опись каталога на каждую выдачу"
fi
# Существующий корпус — БЕЗ файлов этого набора (они красны по заявлению, пока dev не сделал
# задачи 1–3; их судит шаг task1+2+3). Проверяется, что hint/тик/каталог не регрессировали.
GW_OLD_FAIL=""
for f in crates/gateway/tests/*.rs; do
  n=$(basename "$f" .rs)
  case "$n" in red_m89_*) continue;; esac
  cargo test -q -p gateway --test "$n" >/dev/null 2>&1 || GW_OLD_FAIL="$GW_OLD_FAIL $n"
done
if cargo test -q -p gateway --lib >/dev/null 2>&1 && [ -z "$GW_OLD_FAIL" ]; then
  pass "task2/D-1(б): существующий корпус gateway зелен (hint, тик, каталог не регрессировали)"
else
  fail "task2/D-1(б): существующий корпус gateway КРАСЕН —${GW_OLD_FAIL:- lib}"
fi

# ─────────────── задача 4 — счётчик выдачи включает каждый pump и РАВЕН дельте читателя ───────────────
run_test "task4" "red_m89_read_volume_truth (v: ≥ ckpt+tail, ≤ rchar, == R+P1+n·P2 реплики)" -p gateway-serve --test red_m89_read_volume_truth
run_test "task4" "red_m87_read_volume_truth::q2 остался зелёным" -p gateway-serve --test red_m87_read_volume_truth

# ─────────────── задача 5 — бюджет-плацебо удалён; граница СТРУКТУРНАЯ ───────────────
task5_scan_check "$GS"
DEAD_T=$(dead_name_hits crates/gateway-serve/tests 2>/dev/null | tail -1)
# В sacred-тестах имена живут ТОЛЬКО в комментариях-истории кругов (`red_m87_r196_conditions`,
# `red_m87_read_volume_truth`, `red_m89_structural_bound`) — сняв комментарии, кода с ними быть не должно.
if [ "${DEAD_T:-1}" -eq 0 ]; then
  pass "task5: в crates/gateway-serve/tests имена бюджета-плацебо остались только в комментариях"
else
  fail "task5: в crates/gateway-serve/tests $DEAD_T упоминаний бюджета-плацебо В КОДЕ (u5_* обязаны быть удалены architect'ом, §16)"
fi
run_test "task5/I-3" "red_m89_structural_bound (s1 v1 + s2 legacy, прод-форма 200k, rchar)" -p gateway-serve --test red_m89_structural_bound

# ─────────────── задача 6 — счётчики на экземпляр ───────────────
run_test "task6" "red_m89_counters_instance (i1/i2)" -p gateway-serve --test red_m89_counters_instance

# ─────────────── задача 7 — оракулы точки входа M-87 на ручке экземпляра; сторож SERIAL ───────────────
# `--features testing` ОБЯЗАТЕЛЕН: c4 стоит под флагом (M-87 §14.1quater), без него он не исполняется.
run_test "task7" "red_m87_entrypoint под --features testing" -p gateway-serve --features testing --test red_m87_entrypoint
run_test "task7" "red_m87_entrypoint В ФОРМЕ CI (без флага)" -p gateway-serve --test red_m87_entrypoint
run_test "task7" "red_m87_registry (биекция сценариев цела)" -p gateway-serve --test red_m87_registry
run_test "task7/TD-225" "red_m89_serial_guard (каждый async-тест берёт SERIAL ПЕРВОЙ строкой + m1..m6 мутанты сторожа)" -p gateway-serve --test red_m89_serial_guard

# ─────────────── задачи 8+9 — сердцебиение: точка входа прод-бинаря + compose ───────────────
cargo build -q -p gateway-serve --bin gateway-serve 2>/dev/null
run_test "task8+9" "red_m89_heartbeat_entrypoint (h0 compose-файл на rw-томе, h1 прод-бинарь на точном пути compose)" -p gateway-serve --test red_m89_heartbeat_entrypoint
run_test "task9" "red_m87_prod_entrypoint_argv остался зелёным (политика на compose-окружении)" -p gateway-serve --test red_m87_prod_entrypoint_argv

# ─────────────── задача 10 — ops: читатель, инциденты, бинарь ───────────────
run_test "task10" "crates/ops целиком (red_m89_serving_silence g1..g5 + существующие)" -p ops

# ─────────────── задача 11 — КОМПОЗИЦИЯ: куда пишет compose ↔ откуда читает cron ───────────────
task11_composition_check "$COMPOSE_FILE" "$CRON_FILE"
skip "task11: установка $CRON_FILE в /etc/cron.d и сборка ops-watchdog на VPS — ручной шаг founder ★ (deploy/README.md); предъявляется reviewer'ом на §8-гейте (ls /etc/cron.d/hft-watchdog, свежий /var/lib/hft/watchdog.last-success)"

# ─────────────── задача 12 — прод-замер ДО/ПОСЛЕ (только на проде) ───────────────
skip "task12: rchar/время первой выдачи новой подписки на проде ДО/ПОСЛЕ при курсоре слепка глубоко в активном сегменте (замер 2026-09-27: ≈412 МБ перед курсором) — снимает reviewer на §8-гейте, сырые строки в R-NNN"

# ─────────────── task-status (TD-221) ───────────────
task_status_check "$SPEC"

# ─────────────── паритет с CI — таблица + исполнение ───────────────
ci_parity

printf '\n'
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
fi
echo "VERDICT: FAIL (провалов: $FAIL)"
exit 1
