#!/usr/bin/env bash
# verify_M-94 — профиль расчётов: один версионированный носитель определения расчёта (S1a-P, П-032).
# Спека: milestones/M-94-calc-profile.md §12. Блок паритета с CI — ВРЕМЕННАЯ копия verify_M-93.sh
# (решение A-043; общий шаг — TD-222). Агрегатор с FAIL-счётчиком; решение — по коду возврата.
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
    printf '%s\n' "$out" | grep -E '^(FAIL|error|thread |test .* FAILED|TD-|SETUP|VERDICT|I-[0-9])' | cut -c1-220 | head -8
  fi
}

# VERIFY_M94_CI_DRY=1 — только сверка карты CI (без исполнения шагов и без task-шагов): для пробы карты.
DRY="${VERIFY_M94_CI_DRY:-0}"

if [ "$DRY" != "1" ]; then
# --- task7: файл профиля v1 существует и несёт ровно восемь ключей (оракулы разбирают его сами)
if [ -f config/calc-profile/active.env ] \
   && [ "$(grep -cE '^[A-Z][A-Z0-9_]*=' config/calc-profile/active.env)" -eq 8 ]; then
  pass "task7: config/calc-profile/active.env — восемь ключей"
else
  fail "task7: config/calc-profile/active.env отсутствует или несёт не восемь ключей"
fi
# --- task1/2/4/5: прогреватель (c1 c2 p1…p9) — прод-путь cron'а, режим профиля, .profile, next
run_step "task1-2,4-5: red_m94_calc_profile_warmer (c1 c2 p1…p9, p8b)" \
  cargo test -q -p gateway --test red_m94_calc_profile_warmer
# --- task1/3/4: выдача (s1 s2 s3) — прод-бинарь на окружении compose
run_step "task1,3-4: red_m94_calc_profile_serve (s1 s2 s3)" \
  cargo test -q -p gateway-serve --test red_m94_calc_profile_serve
# --- task3: читатель сердцебиения не ломается полем calc_profile (сторож o1)
run_step "task3: red_m94_heartbeat_profile_compat (o1)" \
  cargo test -q -p ops --test red_m94_heartbeat_profile_compat
# --- task4: compose в прод-форме окружения деплоя (D10) + гейт доставки (мелкая форма)
run_step "task4: red_compose_deploy_form.sh (D10)" bash scripts/tests/red_compose_deploy_form.sh
run_step "task4: verify_delivery_M-08.sh (мелкая форма)" bash scripts/verify_delivery_M-08.sh
# --- task6: гейт деплоя (g0…g7)
run_step "task6: red_m94_deploy_gate.sh (g0…g6)" bash scripts/tests/red_m94_deploy_gate.sh
run_step "task6: red_m94_deploy_apply.sh (a0…a4, a7 — откат исполнением, C-280 R3)" bash scripts/tests/red_m94_deploy_apply.sh
# --- task8: корпус целиком — исполняется ниже шагом паритета `cargo test --all` (CI build-test)
# --- task9: барьер изменения профиля (задача architect'а; до сдачи — FAIL, не SKIP)
if [ -f scripts/tests/red_calc_profile.sh ] && [ -f scripts/check_calc_profile.sh ]; then
  run_step "task9: проба барьера red_calc_profile.sh" bash scripts/tests/red_calc_profile.sh
  if grep -q 'check_calc_profile.sh' .github/workflows/ci.yml; then
    pass "task9: барьер проведён в ci.yml"
  else
    fail "task9: check_calc_profile.sh не проведён в ci.yml"
  fi
else
  fail "task9: барьера check_calc_profile.sh / пробы red_calc_profile.sh нет (задача architect'а, П-032 п.3)"
fi
# --- task10: сторож verify_M-70 task #7 «прод-дефолт ровно 0.001» снят (П-032 п.6)
if grep -q 'прод-дефолт РОВНО узкий' scripts/verify_M-70.sh; then
  fail "task10: verify_M-70.sh всё ещё сторожит дефолт GATEWAY_BANDS в compose (П-032 п.6)"
else
  pass "task10: сторож verify_M-70 task #7 снят"
fi
run_step "ci-map: проба карты CI-паритета (red_verify_M-94_ci_map.sh; число миров печатает проба)" \
  bash scripts/tests/red_verify_M-94_ci_map.sh
fi # DRY

# --- паритет с CI (gates.md §3, C-265 F2): КАЖДЫЙ шаг run: из ci.yml — однострочный И многострочный —
# либо исполняется здесь, либо стоит в ТОЧНОЙ карте исключений с причиной. Карта сверяется в обе
# стороны: исключение, не совпавшее ни с одним шагом, — FAIL (протухло); шаг с `${{ … }}`, который
# здесь нечем подставить и который не в карте, — FAIL (не учтён).
BASE="$(git merge-base origin/main HEAD 2>/dev/null || true)"
CI_FILE="${VERIFY_M94_CI_FILE:-.github/workflows/ci.yml}"

ci_steps() { # → записи (полный текст шага run:), разделённые NUL
  awk '
    function flush() { if (have) { printf "%s%s%c", (folded ? "\001FOLDED\001" : ""), buf, 0 }; have = 0; buf = ""; folded = 0 }
    inblk {
      if ($0 ~ /^[[:space:]]*$/) next
      match($0, /^[[:space:]]*/); n = RLENGTH
      if (ind < 0) ind = n
      if (n >= ind) { l = substr($0, ind + 1); buf = (have ? buf "\n" : "") l; have = 1; next }
      inblk = 0; flush()
    }
    # A-043 К-2: `|`, `|-`, `|+` — литеральный блок (чомпинг на текст команд не влияет);
    # `>`, `>-`, `>+` — складывающий: строки сливаются, команда меняется ⇒ помечается и даёт FAIL.
    /^[[:space:]]*(- )?run:[[:space:]]*\|[-+]?[[:space:]]*$/ { flush(); inblk = 1; ind = -1; next }
    /^[[:space:]]*(- )?run:[[:space:]]*>[-+]?[[:space:]]*$/ { flush(); inblk = 1; ind = -1; folded = 1; next }
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
    | xargs -r grep -l 'M-94' 2>/dev/null | head -1)
  nsteps=0; nexec=0
  while IFS= read -r -d '' step; do
    nsteps=$((nsteps + 1))
    if [ "${step#$'\001'FOLDED$'\001'}" != "$step" ]; then
      step="${step#$'\001'FOLDED$'\001'}"
      fail "ci-parity: шаг «${step%%$'\n'*}» — блочный скаляр > не поддержан: складывание строк меняет команду"; continue
    fi
    first="${step%%$'\n'*}"
    key=$(step_key "$step")
    if [ -n "${EXCL[$key]+x}" ]; then
      EXCL_HIT[$key]=1
      skip "ci-parity: [$key] ${EXCL[$key]}"
      continue
    fi
    if [ "$step" = "cargo audit" ] && ! command -v cargo-audit >/dev/null 2>&1; then
      fail "ci-parity: cargo audit — cargo-audit недоступен локально"; continue
    fi
    if [ "$step" = "bash scripts/check_review_fa.sh" ] && [ -z "$have_review" ]; then
      skip "ci-parity: check_review_fa — вердикта R-NNN по M-94 в диапазоне ещё нет (зеленеет на PR-гейте)"; continue
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
[ "$DRY" = "1" ] || skip "task11: §8-гейт — строка GATEWAY_BANDS удалена из host .env до деплоя; гейт деплоя прошёл; сердцебиение calc_profile.version == 1 и sha256 == sha256sum config/calc-profile/active.env на вершине; ckpt-8f69809dd707e8c9.bin освежается, рядом .profile; WS-подписка семи полос → snapshot — снимает reviewer"

if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (провалов: $FAIL)"; exit 1
