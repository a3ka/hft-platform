#!/usr/bin/env bash
# verify_M-90 — у селектора прогревателя один источник (TD-227).
# Спека: milestones/M-90-warmer-selector-single-source.md §9.
# Агрегатор с FAIL-счётчиком; решение — по коду возврата каждого шага, не по тексту.
set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP  %s\n' "$1"; }

run_step() { # <метка> <команда…>
  local tag="$1"; shift
  local out rc
  out=$("$@" 2>&1); rc=$?
  if [ $rc -eq 0 ]; then
    pass "$tag"
  else
    fail "$tag (exit=$rc)"
    printf '%s\n' "$out" | grep -E '^(FAIL|error|thread |test .* FAILED|TD-|SETUP|VERDICT)' | cut -c1-220 | head -8
  fi
}

# VERIFY_M90_CI_DRY=1 — только сверка карты CI (без исполнения шагов и без task-шагов): для пробы карты.
DRY="${VERIFY_M90_CI_DRY:-0}"

if [ "$DRY" != "1" ]; then
# --- task1-3: оракулы предмета
run_step "task1-3: red_m90_warmer_cron_composition (w1 прод-.env, w2 контроль, w3 отказ на CHECKPOINT_*)" \
  cargo test -p gateway --test red_m90_warmer_cron_composition
run_step "task1: red_checkpoint_bin_prod_argv (прод-argv compose, c3ter) не сломан" \
  cargo test -p gateway --test red_checkpoint_bin_prod_argv

# --- task2: контракт HFT_CRON_PRINT_ARGV и композиция покрытия (M-48)
run_step "task2: verify_M-48.sh (PRINT_ARGV, coverage-out == checkpoint-coverage ретеншена)" \
  bash scripts/verify_M-48.sh

run_step "ci-map: проба карты CI-паритета (red_verify_M-90_ci_map.sh — 3 мира)" \
  bash scripts/tests/red_verify_M-90_ci_map.sh
fi # DRY

# --- паритет с CI (gates.md §3, C-265 F2): КАЖДЫЙ шаг run: из ci.yml — однострочный И многострочный —
# либо исполняется здесь, либо стоит в ТОЧНОЙ карте исключений с причиной. Карта сверяется в обе
# стороны: исключение, не совпавшее ни с одним шагом, — FAIL (протухло); шаг с `${{ … }}`, который
# здесь нечем подставить и который не в карте, — FAIL (не учтён).
BASE="$(git merge-base origin/main HEAD 2>/dev/null || true)"
CI_FILE="${VERIFY_M90_CI_FILE:-.github/workflows/ci.yml}"

ci_steps() { # → записи (полный текст шага run:), разделённые NUL
  awk '
    function flush() { if (have) { printf "%s%c", buf, 0 }; have = 0; buf = "" }
    inblk {
      if ($0 ~ /^[[:space:]]*$/) next
      match($0, /^[[:space:]]*/); n = RLENGTH
      if (ind < 0) ind = n
      if (n >= ind) { l = substr($0, ind + 1); buf = (have ? buf "\n" : "") l; have = 1; next }
      inblk = 0; flush()
    }
    /^[[:space:]]*(- )?run:[[:space:]]*\|[[:space:]]*$/ { flush(); inblk = 1; ind = -1; next }
    /^[[:space:]]*(- )?run:[[:space:]]*/ {
      flush(); x = $0; sub(/^[[:space:]]*(- )?run:[[:space:]]*/, "", x); printf "%s%c", x, 0
    }
    END { flush() }
  ' "$1"
}

# ТОЧНАЯ карта исключений (`C-266` F2): ключ — отпечаток ПОЛНОГО текста шага (sha256, 16 знаков),
# а не первой строки: строка, дописанная в исключённый блок, меняет отпечаток ⇒ исключение не
# срабатывает, шаг уходит на исполнение, а запись карты — «протухла» (FAIL). Единственная
# нормализация — строка условия агрегата `status-check` (меняется с каждым джобом в `needs`), и
# только если она СТРОГО той формы, где нет ничего, кроме сравнений `needs.<джоб>.result`.
AGG_RE='^if \[\[ ("\$\{\{ needs\.[a-z0-9-]+\.result \}\}" != "success"( \|\| )?)+ \]\]; then$'
step_key() { # <полный текст шага> → отпечаток
  local t="$1" first rest
  first="${t%%$'\n'*}"
  if [ "$t" != "$first" ]; then rest="${t#*$'\n'}"; else rest=""; fi
  if printf '%s\n' "$first" | grep -qE "$AGG_RE"; then first='<условие агрегата status-check>'; fi
  if [ -n "$rest" ] || [ "$t" != "${t%%$'\n'*}" ]; then t="$first"$'\n'"$rest"; else t="$first"; fi
  printf '%s' "$t" | sha256sum | cut -c1-16
}
declare -A EXCL=(
  ['188b5f08be3448fb']='«cargo install cargo-audit --locked» — установка инструмента (cargo audit исполняется ниже)'
  ['0ada681c1a994d65']='«pip install --quiet jsonschema» — установка инструмента'
  ['ba2ff42976ef05e2']='«python3 -m pip install --quiet pyyaml» — установка инструмента'
  ['48fbd5e3f735e6ad']='«git fetch … refs/salvage/*» — плумбинг CI: спас-рефы в свежий клон; локальный клон их несёт'
  ['ddd7a1d0ce254659']='шаг «база события» (12 строк, пишет sha в GITHUB_OUTPUT); локальный эквивалент — merge-base origin/main HEAD выше'
  ['eb31b754c4a50419']='агрегат «All checks passed» (условие по needs); его правильность исполняет red_ci_aggregate.sh (EXEC)'
)
declare -A EXCL_HIT=()

if [ -z "$BASE" ]; then
  fail "ci-parity: merge-base origin/main HEAD не вычислен — нет базы события"
else
  have_review=$(git diff --name-only --diff-filter=A "$BASE" HEAD -- 'research/reviews/R-*.md' \
    | xargs -r grep -l 'M-90' 2>/dev/null | head -1)
  nsteps=0; nexec=0
  while IFS= read -r -d '' step; do
    nsteps=$((nsteps + 1))
    first="${step%%$'\n'*}"
    key=$(step_key "$step")
    if [ -n "${EXCL[$key]+x}" ]; then
      EXCL_HIT[$key]=1
      skip "ci-parity: [$key] ${EXCL[$key]}"
      continue
    fi
    if [ "$first" = "cargo audit" ] && ! command -v cargo-audit >/dev/null 2>&1; then
      fail "ci-parity: cargo audit — cargo-audit недоступен локально"; continue
    fi
    if [ "$first" = "bash scripts/check_review_fa.sh" ] && [ -z "$have_review" ]; then
      skip "ci-parity: check_review_fa — вердикта R-NNN по M-90 в диапазоне ещё нет (зеленеет на PR-гейте)"; continue
    fi
    cmd=$(printf '%s' "$step" | sed "s/\${{ steps.base.outputs.sha }}/$BASE/g")
    case "$cmd" in
      *'${{'*) fail "ci-parity: шаг «$first» несёт выражение CI, которое здесь нечем подставить, и не стоит в карте исключений"; continue ;;
    esac
    nexec=$((nexec + 1))
    if [ "$DRY" = "1" ]; then pass "ci-parity(dry): исполнилось бы «$first»"; continue; fi
    # Форма события — ТОЛЬКО барьерам (check_*.sh читают EVENT_NAME/PR_BASE_SHA, как в CI);
    # пробы red_*.sh — самодостаточные батареи и в CI этих переменных не получают.
    label="$first"; [ "$step" != "$first" ] && label="$first (+ $(printf '%s\n' "$step" | wc -l | tr -d ' ') строк блока)"
    case "$cmd" in
      "bash scripts/check_"*) run_step "ci-parity: $label" env EVENT_NAME=pull_request PR_BASE_SHA="$BASE" bash -ec "$cmd" ;;
      *) run_step "ci-parity: $label" env -u EVENT_NAME -u PR_BASE_SHA bash -ec "$cmd" ;;
    esac
  done < <(ci_steps "$CI_FILE")
  expect=$(grep -cE '^[[:space:]]*(- )?run:' "$CI_FILE")
  [ "$nsteps" -eq "$expect" ] || fail "ci-parity: разобрано шагов $nsteps, а строк run: в $CI_FILE — $expect (разбор сломан)"
  for k in "${!EXCL[@]}"; do
    [ -n "${EXCL_HIT[$k]+x}" ] || fail "ci-parity: исключение [$k] ${EXCL[$k]} не совпало ни с одним шагом CI — карта протухла (шаг изменён или удалён)"
  done
  pass "ci-parity: учтено шагов $nsteps из $expect (исполнено $nexec, исключено по карте ${#EXCL_HIT[@]})"
fi

# --- task4: прод
[ "$DRY" = "1" ] || skip "task4: §8-гейт — ручная строка CHECKPOINT_BANDS стёрта деплоем И сервер находит слепок cron'а (snapshot, не not_ready) — снимает reviewer после деплоя"

if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (провалов: $FAIL)"; exit 1
