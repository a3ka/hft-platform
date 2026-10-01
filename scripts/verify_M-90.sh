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

# --- task1-3: оракулы предмета
run_step "task1-3: red_m90_warmer_cron_composition (w1 прод-.env, w2 контроль, w3 отказ на CHECKPOINT_*)" \
  cargo test -p gateway --test red_m90_warmer_cron_composition
run_step "task1: red_checkpoint_bin_prod_argv (прод-argv compose, c3ter) не сломан" \
  cargo test -p gateway --test red_checkpoint_bin_prod_argv

# --- task2: контракт HFT_CRON_PRINT_ARGV и композиция покрытия (M-48)
run_step "task2: verify_M-48.sh (PRINT_ARGV, coverage-out == checkpoint-coverage ретеншена)" \
  bash scripts/verify_M-48.sh

# --- паритет с CI (gates.md §3): каждая однострочная run: из ci.yml — в форме pull_request
BASE="$(git merge-base origin/main HEAD 2>/dev/null || true)"
if [ -z "$BASE" ]; then
  fail "ci-parity: merge-base origin/main HEAD не вычислен — нет базы события"
else
  export EVENT_NAME=pull_request PR_BASE_SHA="$BASE" GITHUB_BASE_REF=main
  mapfile -t RUNS < <(grep -E '^[[:space:]]*(- )?run:[[:space:]]*[^|[:space:]]' .github/workflows/ci.yml \
    | sed -E 's/^[[:space:]]*(- )?run:[[:space:]]*//')
  [ "${#RUNS[@]}" -ge 40 ] || fail "ci-parity: из ci.yml разобрано ${#RUNS[@]} команд — ожидалось ≥ 40 (разбор сломан?)"
  have_review=$(git diff --name-only --diff-filter=A "$BASE" HEAD -- 'research/reviews/R-*.md' \
    | xargs -r grep -l 'M-90' 2>/dev/null | head -1)
  for r in "${RUNS[@]}"; do
    case "$r" in
      "cargo install"*|"pip install"*|"python3 -m pip install"*)
        skip "ci-parity: «$r» — установка инструмента, не проверка"; continue ;;
      "git fetch"*)
        skip "ci-parity: «$r» — плумбинг CI (спас-рефы), не проверка"; continue ;;
      "cargo audit")
        if ! command -v cargo-audit >/dev/null 2>&1; then fail "ci-parity: cargo-audit недоступен локально"; continue; fi ;;
      "bash scripts/check_review_fa.sh")
        if [ -z "$have_review" ]; then skip "ci-parity: check_review_fa — вердикта R-NNN по M-90 в диапазоне ещё нет (зеленеет на PR-гейте)"; continue; fi ;;
    esac
    cmd=$(printf '%s' "$r" | sed "s/\${{ steps.base.outputs.sha }}/$BASE/g")
    run_step "ci-parity: $cmd" bash -c "$cmd"
  done
  nmulti=$(grep -cE '^[[:space:]]*(- )?run:[[:space:]]*\|' .github/workflows/ci.yml)
  skip "ci-parity: многострочных run-блоков $nmulti (база события, проверка базы, агрегат) — плумбинг CI; агрегат держит red_ci_aggregate.sh выше"
fi

# --- task4: прод
skip "task4: §8-гейт — ручная строка CHECKPOINT_BANDS стёрта деплоем И сервер находит слепок cron'а (snapshot, не not_ready) — снимает reviewer после деплоя"

if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (провалов: $FAIL)"; exit 1
