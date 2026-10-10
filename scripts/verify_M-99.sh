#!/usr/bin/env bash
# Acceptance-гейт M-99 — учебное восстановление по расписанию + сторож (остаток M-74, TD-193).
# Спека: milestones/M-99-restore-drill-automation.md §9. Помощники chk/chk_named_test и шаги задач
# 2/2b/3/4/5 — из verify_M-74.sh (там же — четыре урока, которыми он научен); блок паритета с CI —
# ВРЕМЕННАЯ копия verify_M-98.sh (решение A-043; общий шаг — TD-222).
#
# ГЕЙТ НАПИСАН ДО РАБОТЫ И ОБЯЗАН БЫТЬ КРАСНЫМ: задачи 2/2b/3/4/5/6 открыты. Шаг, ставший зелёным
# РАНЬШЕ своей задачи, есть дефект гейта.
#
# VERIFY_M99_CI_DRY=1 — только сверка карты CI (без исполнения шагов и без task-шагов): для пробы карты.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
ROOT="$(pwd)"

FAIL=0
step() { printf '\n── %s\n' "$*"; }
chk() {
  local name; name="$(printf '%s' "$1" | sed -n '1{s/^[[:space:]]*//;s/[[:space:]]*$//;p}')"
  [ -n "$name" ] || name="<многострочная проверка>"
  if ( eval "$1" ) >/dev/null 2>&1; then echo "PASS: ${name}"; else echo "FAIL: ${name}" >&2; FAIL=$((FAIL + 1)); fi
}

# ТРИ ИСХОДА, А НЕ ДВА (образец — `verify_M-72.sh:61-84`). Различать их обязательно:
# «оракула нет» и «оракул есть, но не собрался» — разные состояния задачи, и одинаковый
# текст отправил бы читателя искать не то. COMPILE-RED — ЗАЯВЛЕННОЕ состояние оракула
# `red_restore_drill_metric`: он написан против сигнатуры, которую вносит engine-dev
# (спека §«Сигнатура продюсера»), и до тех пор не собирается.
chk_named_test() { # $1=имя шага, далее — команда cargo
  local name="$1"; shift
  local out st ran
  out="$("$@" 2>&1)"; st=$?
  ran=$(printf '%s\n' "${out}" | awk '/^test result:/ { p += $4; f += $6 } END { print p + f + 0 }')
  if [ "${ran:-0}" -eq 0 ]; then
    if printf '%s\n' "${out}" | grep -qE 'could not compile|^error\[E[0-9]'; then
      echo "FAIL: ${name} — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED): $(printf '%s\n' "${out}" | grep -m1 -E '^error' | cut -c1-100)" >&2
    else
      echo "FAIL: ${name} — НИ ОДИН тест не исполнился: фильтр не нашёл оракула. Зелёное здесь означало бы ВАКУУМ, а не закрытую задачу" >&2
    fi
    FAIL=$((FAIL + 1))
    return
  fi
  if [ ${st} -eq 0 ]; then
    echo "PASS: ${name} (исполнено тестов: ${ran})"
  else
    echo "FAIL: ${name} (исполнено тестов: ${ran}, exit=${st})" >&2
    FAIL=$((FAIL + 1))
  fi
}

# ── САМОПРОВЕРКА ПОМОЩНИКОВ. Если `chk`/`chk_named_test` окажутся не определены или
# перестанут считать отказы, ВЕСЬ гейт станет зелёным, ничего не проверив (`C-187` B-4).
_probe=0
chk "true"  >/dev/null 2>&1 || _probe=1
_before=${FAIL}
chk "false" >/dev/null 2>&1
_after_chk=${FAIL}
# `chk_named_test` на заведомо несуществующем таргете обязан дать ВАКУУМ и посчитать отказ.
chk_named_test "самопроверка вакуума" cargo test -p journal --test нет-такого-таргета --quiet >/dev/null 2>&1
if [ "${_after_chk}" -ne $((_before + 1)) ] || [ "${FAIL}" -ne $((_before + 2)) ] || [ "${_probe}" -ne 0 ]; then
  echo "FAIL: самопроверка помощников — chk или chk_named_test не считают отказы; весь гейт был бы зелёным ни о чём" >&2
  echo "VERDICT: FAIL (1)"; exit 1
fi
FAIL=${_before}
echo "PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются"

pass() { printf 'PASS: %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP: %s\n' "$1"; }
run_step() { # <метка> <команда…>
  local tag="$1"; shift
  local out rc
  out=$("$@" 2>&1); rc=$?
  if [ $rc -eq 0 ]; then pass "$tag"; else
    fail "$tag (exit=$rc)"
    printf '%s\n' "$out" | grep -E '^(FAIL|error|thread |test .* FAILED|SETUP|VERDICT)' | cut -c1-220 | head -8 >&2
  fi
}

DRY="${VERIFY_M99_CI_DRY:-0}"
DRILL=deploy/bin/journal-restore-drill-cron.sh
EMIT=crates/recorder/src/metric_emit.rs
READER=crates/journal/src/bin/journal-drill-read.rs
CRON=deploy/cron.d/journal-restore-drill

if [ "$DRY" != "1" ]; then
step "task #1 — фикстура ПРОД-ФОРМЫ читается journal::stream; проба различает исходы и атаки (C-219 B-1/B-2)"
chk_named_test "фикстура прод-формы читается journal::stream" \
  cargo test -p journal --test fixture_restore_drill_cold --quiet
chk "bash scripts/tests/red_restore_drill.sh"

step "task #2 — обёртка существует, исполняется прод-формой, композиция путей предъявлена печатью argv"
chk "test -x ${DRILL}"
chk "! grep -q 'hft-platform_journal-data' ${DRILL}"
chk "w=\$(HFT_CRON_PRINT_ARGV=1 bash ${DRILL} 2>/dev/null | sed -n 's/^RESTORE_DIR=//p'); r=\$(HFT_CRON_PRINT_ARGV=1 bash ${DRILL} 2>/dev/null | sed -n 's/^READER_DIR=//p'); [ -n \"\$w\" ] && [ \"\$w\" = \"\$r\" ]"
# §5: дефолт пути состояния у обёртки = DEFAULT_RESTORE_DRILL_STATE сторожа (композиция продюсер→потребитель)
chk "[ \"\$(env -u JOURNAL_DRILL_STATE HFT_CRON_PRINT_ARGV=1 bash ${DRILL} 2>/dev/null | sed -n 's/^STATE=//p')\" = /var/lib/hft/journal-restore-drill.json ] && grep -q 'DEFAULT_RESTORE_DRILL_STATE: &str = \"/var/lib/hft/journal-restore-drill.json\"' crates/ops/src/restore_drill.rs"
# §4 п.3: читатель по умолчанию — journal-drill-read из образа работающего hft-recorder
chk "env -u JOURNAL_DRILL_READER HFT_CRON_PRINT_ARGV=1 bash ${DRILL} 2>/dev/null | grep -E '^READER=.*journal-drill-read' | grep -q 'hft-recorder'"
# §7: Storage Box только читается — ни --delete, ни удаления на стороне коробки
chk "! grep -qE -- '--delete|(^|[^-])rm .*JOURNAL_DRILL_COLD' ${DRILL}"

step "task #2b — читатель существует, различает коды, попадает в прод-образ"
chk "test -f ${READER}"
chk "d=\$(mktemp -d) && cargo run -q -p journal --bin journal-drill-read -- --dir \$d --min-events 1; rc=\$?; rm -rf \$d; [ \$rc -eq 5 ]"
chk "grep -qE 'cargo build --release .*--bin journal-drill-read' Dockerfile && grep -q 'COPY .*journal-drill-read' Dockerfile"

step "task #3 — метрика РЕАЛЬНО эмитится (OPS-I-10)"
chk "grep -qE 'set_gauge\(\s*\"backup_restore_drill_ok\"' ${EMIT}"
chk "! grep -qE 'backup_restore_drill_ok.*deferred' ${EMIT}"
chk_named_test "отображение «файл состояния → gauge в рендере /metrics»" \
  cargo test -p recorder --test red_restore_drill_metric --quiet

step "task #4 — расписание принимается прод-парсером и оно МЕСЯЧНОЕ (OPS-I-3)"
if ! command -v crontab >/dev/null 2>&1; then
  fail "crontab недоступен — синтаксис расписания НЕ ПРОВЕРЕН; это отказ СРЕДЫ, а не зелёный гейт"
else
  chk "test -f ${CRON}"
  chk "crontab -n ${CRON}"
  # минута, час, день месяца — числа; месяц и день недели — '*': ровно раз в месяц
  chk "grep -E '^[0-9]+[[:space:]]+[0-9]+[[:space:]]+[0-9]+[[:space:]]+\\*[[:space:]]+\\*[[:space:]]+root[[:space:]].*journal-restore-drill-cron\\.sh' ${CRON} | grep -q flock"
fi

step "task #5 — просрочка есть отказ, окно с обеих сторон"
chk_named_test "просроченный drill ⇒ 0, внутри окна ⇒ 1 (recorder)" \
  cargo test -p recorder --test red_restore_drill_metric --quiet stale

step "task #6 — сторож видит провал, просрочку и ОТСУТСТВИЕ drill'а (WD-RESTORE-DRILL)"
chk_named_test "red_m99_restore_drill_watch (v1…v3, w1…w3, b1)" \
  cargo test -p ops --test red_m99_restore_drill_watch --quiet
chk "[ \"\$(grep -rlE 'const RESTORE_DRILL_FRESH_WINDOW_MS' crates --include='*.rs' | grep -v '/tests/' | tr '\\n' ' ')\" = 'crates/ops/src/restore_drill.rs ' ]"

step "task #7 — проба исполняется в CI безусловно (иначе после архива гейта замолчит — класс TD-253)"
chk "grep -qE '^[[:space:]]*run: bash scripts/tests/red_restore_drill\\.sh[[:space:]]*\$' .github/workflows/ci.yml"

step "граница — путь удаления и логика журнала НЕ тронуты"
BASE_R=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE_R}" ]; then
  fail "merge-base с origin/main не вычислен — шаги диапазона судить не по чему"
else
  chk "git diff --name-only ${BASE_R}..HEAD | grep -E '^(deploy/bin/journal-retention|deploy/cron.d/journal-retention|crates/contracts/)' | grep -q . && exit 1 || exit 0"
  chk "git diff --name-only ${BASE_R}..HEAD -- crates/journal | grep -vE '^crates/journal/(src/bin/journal-drill-read\\.rs|tests/fixture_restore_drill_cold\\.rs)\$' | grep -q . && exit 1 || exit 0"
  chk "git diff ${BASE_R}..HEAD -- docker-compose.yml | grep -qE '^[+-].*RETENTION' && exit 1 || exit 0"
fi

run_step "ci-map: проба карты CI-паритета (red_verify_M-99_ci_map.sh; число миров печатает проба)" \
  bash scripts/tests/red_verify_M-99_ci_map.sh
fi # DRY

step "паритет с CI (gates.md §3, A-043): каждый шаг run: из ci.yml исполняется здесь либо стоит в точной карте исключений"
BASE="$(git merge-base origin/main HEAD 2>/dev/null || true)"
CI_FILE="${VERIFY_M99_CI_FILE:-.github/workflows/ci.yml}"

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
    | xargs -r grep -l 'M-99' 2>/dev/null | head -1)
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
      skip "ci-parity: check_review_fa — вердикта R-NNN по M-99 в диапазоне ещё нет (зеленеет на PR-гейте)"; continue
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


[ "$DRY" = "1" ] || skip "task #8 — первый живой прогон на VPS TRANSPORT=remote (A-032 §2.4) — снимает reviewer"

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (провалов: ${FAIL})"; exit 1
