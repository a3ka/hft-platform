#!/usr/bin/env bash
# Проба наблюдателя за ветками — предмет: `scripts/check_branch_health.sh`.
#
# Три обязательных свойства (`docs/workflow/harness-track.md` §5):
#   1. ПОЗИТИВНЫЙ КОНТРОЛЬ — честная фикстура даёт ожидаемые агрегаты и exit=0;
#   2. АНТИ-ПЛАЦЕБО — наблюдатель обязан УВИДЕТЬ висяк и дубль там, где они есть, и НЕ
#      выдумывать их там, где их нет; на несостоявшемся setup'е обязан КРАСНЕТЬ, а не
#      печатать пустой счастливый список;
#   3. МУТАЦИОННЫЙ КОНТРОЛЬ (`--battery`) — нейтрализация каждого агрегата роняет РОВНО
#      свой сценарий.
#
# ГЕРМЕТИЧНОСТЬ. Проба строит СВОЙ git-репозиторий в TMPDIR и в сеть не ходит вовсе: состояние
# PR подаётся файлом через `BRANCH_HEALTH_PRS`. Ходила бы в сеть — мерила бы доступность
# GitHub, а не свой инвариант (класс `TD-135`, образец — `red_reserve_id.sh`).
#
# Прогон: bash scripts/tests/red_branch_health.sh [--battery]

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUT="${ROOT}/scripts/check_branch_health.sh"
SUT_ACTIVE="${SUT}"

PASS=0; FAIL=0; FAILED_NAMES=()
ok()  { PASS=$((PASS + 1)); printf 'ok         %-26s %s\n' "$1" "${2:-}"; }
nok() { FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'FAIL       %-26s %s\n' "$1" "$2"; }
sfail(){ FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'SETUP-FAIL %-26s %s\n' "$1" "$2"; }

own_dirs(){ find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'red-brhealth-*' 2>/dev/null | wc -l; }
TMP_BEFORE="$(own_dirs)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/red-brhealth-XXXXXX")"
REG="${WORK}/.fixtures"; : > "${REG}"
register(){ printf '%s\n' "$1" >> "${REG}"; }
cleanup(){ [ -f "${REG}" ] && while IFS= read -r p; do [ -n "$p" ] && [ -e "$p" ] && rm -rf "$p"; done < "${REG}"; rm -rf "${WORK}"; }
trap cleanup EXIT
register "${WORK}"

# mk_repo <имя> <ветки…> → печатает путь. Строит герметичный репозиторий с origin-рефами.
#
# Каталог берётся `mktemp -d`, а НЕ счётчиком: `scenarios` вызывается второй раз внутри
# батареи через `$(…)`, то есть в ПОДОБОЛОЧКЕ, и инкремент глобального счётчика туда не
# переживает. Со счётчиком имена фикстур сталкивались, `git init` ложился на существующий
# репозиторий, и батарея валила ВСЕ сценарии сразу — то есть мерила собственный дефект,
# а не мутанта.
mk_repo() {
  local name="$1"; shift
  local d; d="$(mktemp -d "${WORK}/case-${name}-XXXXXX")" || return 1
  register "${d}"
  (
    cd "${d}" || exit 1
    git init -q .
    git config user.email t@t; git config user.name t
    echo base > f.txt; git add f.txt; git commit -qm base
    git update-ref refs/remotes/origin/main HEAD
    for br in "$@"; do
      git checkout -q -b "tmp-${br//\//-}" HEAD
      echo "$br" > "${br//\//-}.txt"; git add .; git commit -qm "work $br"
      git update-ref "refs/remotes/origin/${br}" HEAD
      git checkout -q - 2>/dev/null || git checkout -q master 2>/dev/null || true
    done
  ) || return 1
  printf '%s' "${d}"
}

run_sut() { OUT="$(BRANCH_HEALTH_ROOT="$1" BRANCH_HEALTH_PRS="${2:-/dev/null}" BRANCH_HEALTH_STALE_DAYS="${3:-0}" bash "${SUT_ACTIVE}" 2>&1)"; RC=$?; }

# expect <имя> <корень> <prs> <ожидаемый-код> <обязательная-подстрока|-> <запрещённая-подстрока|->
expect() {
  local name="$1" root="$2" prs="$3" wantrc="$4" must="$5" mustnot="$6"
  run_sut "${root}" "${prs}" 0
  if [ "${RC}" -ne "${wantrc}" ]; then
    nok "${name}" "exit=${RC}, ожидался ${wantrc}: $(grep -m1 -E '^(FAIL|VERDICT)' <<<"${OUT}")"; return
  fi
  if [ "${must}" != "-" ] && ! grep -qF "${must}" <<<"${OUT}"; then
    nok "${name}" "нет ожидаемого «${must}»"; return
  fi
  if [ "${mustnot}" != "-" ] && grep -qF "${mustnot}" <<<"${OUT}"; then
    nok "${name}" "ЛОЖНОЕ срабатывание: есть «${mustnot}»"; return
  fi
  ok "${name}" "exit=${RC}"
}


# --- поддельный `gh` в PATH: прогоняем ЖИВОЙ путь барьера без сети ---------------------
# Инъекция BRANCH_HEALTH_PRS обходит весь блок работы с gh — именно поэтому первая редакция
# пробы не покрывала отказы источника (`C-107` F-106-3). Здесь подменяется САМ `gh`, значит
# исполняется прод-ветка кода.
#   mk_gh <каталог> <режим-list> <спец-checks>
#     режим-list: ok:<ветка>:<num>[,<ветка>:<num>…] | fail
#     спец-checks: строки вида <num>=green|pending|red|nochecks|boom, через запятую
mk_gh() {
  local d="$1" listmode="$2" chk="$3"
  mkdir -p "${d}/bin" || return 1
  {
    printf '#!/usr/bin/env bash\n'
    printf 'if [ "$1" = "pr" ] && [ "$2" = "list" ]; then\n'
    if [ "${listmode}" = "fail" ]; then
      printf '  echo "could not resolve to a Repository" >&2; exit 1\n'
    else
      printf '  cat <<ROWS\n%s\nROWS\n  exit 0\n' "$(printf '%s' "${listmode#ok:}" | tr ',' '\n' | awk -F: 'NF==2{printf "%s\t%s\n",$1,$2}')"
    fi
    printf 'fi\n'
    printf 'if [ "$1" = "pr" ] && [ "$2" = "checks" ]; then\n'
    printf '  case "$3" in\n'
    local pair
    for pair in ${chk//,/ }; do
      local num="${pair%%=*}" st="${pair##*=}"
      case "${st}" in
        green)    printf '    %s) printf "All checks passed\\tpass\\t3s\\turl\\n"; exit 0 ;;\n' "${num}" ;;
        pending)  printf '    %s) printf "job\\tpending\\t0\\turl\\n"; exit 8 ;;\n' "${num}" ;;
        red)      printf '    %s) printf "job\\tfail\\t5s\\turl\\n"; exit 1 ;;\n' "${num}" ;;
        nochecks) printf '    %s) echo "no checks reported on the branch" >&2; exit 1 ;;\n' "${num}" ;;
        boom)     printf '    %s) echo "HTTP 503 upstream" >&2; exit 1 ;;\n' "${num}" ;;
      esac
    done
    printf '    *) echo "unexpected" >&2; exit 4 ;;\n'
    printf '  esac\n'
    printf 'fi\n'
    printf 'exit 4\n'
  } > "${d}/bin/gh"
  chmod +x "${d}/bin/gh"
}

# run_live <корень> <каталог-с-gh> — БЕЗ BRANCH_HEALTH_PRS, то есть по живому пути
run_live() {
  OUT="$(PATH="$2/bin:${PATH}" BRANCH_HEALTH_ROOT="$1" BRANCH_HEALTH_STALE_DAYS=0 bash "${SUT_ACTIVE}" 2>&1)"; RC=$?
}

expect_live() {
  local name="$1" root="$2" ghd="$3" wantrc="$4" must="$5" mustnot="$6"
  run_live "${root}" "${ghd}"
  if [ "${RC}" -ne "${wantrc}" ]; then
    nok "${name}" "exit=${RC}, ожидался ${wantrc}: $(grep -m1 -E '^(FAIL|VERDICT)' <<<"${OUT}")"; return
  fi
  if [ "${must}" != "-" ] && ! grep -qF "${must}" <<<"${OUT}"; then nok "${name}" "нет «${must}»"; return; fi
  if [ "${mustnot}" != "-" ] && grep -qF "${mustnot}" <<<"${OUT}"; then nok "${name}" "ЛОЖНОЕ: есть «${mustnot}»"; return; fi
  ok "${name}" "exit=${RC}"
}

scenarios() {

# --- позитивный контроль: две разные ветки, PR-ов нет ------------------------------------
d="$(mk_repo clean feat/M-01-a docs/M-02-b)" || { sfail "P0-честная" "фикстура"; return; }
expect "P0-честная" "${d}" /dev/null 0 "VERDICT: PASS" "NOTE  ДУБЛЬ"

# --- ВИСЯК: зелёный PR без merge'а --------------------------------------------------------
d="$(mk_repo stale feat/M-03-ready)" || { sfail "S1-висяк-зелёный" "фикстура"; return; }
printf 'feat/M-03-ready\t77\tgreen\n' > "${d}/prs.tsv"
expect "S1-висяк-зелёный" "${d}" "${d}/prs.tsv" 0 "ВИСЯК: feat/M-03-ready" "-"

# Анти-плацебо в ДРУГУЮ сторону: красный и pending висяком НЕ являются.
d="$(mk_repo red feat/M-04-broken)" || { sfail "S2-красный-не-висяк" "фикстура"; return; }
printf 'feat/M-04-broken\t78\tred\n' > "${d}/prs.tsv"
expect "S2-красный-не-висяк" "${d}" "${d}/prs.tsv" 0 "-" "NOTE  ВИСЯК"

d="$(mk_repo pend feat/M-05-wait)" || { sfail "S3-pending-не-висяк" "фикстура"; return; }
printf 'feat/M-05-wait\t79\tpending\n' > "${d}/prs.tsv"
expect "S3-pending-не-висяк" "${d}" "${d}/prs.tsv" 0 "-" "NOTE  ВИСЯК"

# --- ДУБЛЬ: один предмет на нескольких ветках --------------------------------------------
d="$(mk_repo dup feat/M-66 feat/M-66-fixture docs/M-66-attest)" || { sfail "S4-дубль" "фикстура"; return; }
expect "S4-дубль" "${d}" /dev/null 0 "ДУБЛЬ: предмет M-66 живёт на 3 ветках" "-"

# Разные предметы дублем НЕ являются — иначе агрегат кричал бы всегда.
d="$(mk_repo nodup feat/M-70-x feat/M-71-y feat/M-72-z)" || { sfail "S5-разные-не-дубль" "фикстура"; return; }
expect "S5-разные-не-дубль" "${d}" /dev/null 0 "-" "NOTE  ДУБЛЬ"

# Идентификатор берётся из ИМЕНИ ветки; ветка без ID не создаёт ложных пар.
d="$(mk_repo noid feature-one feature-two)" || { sfail "S6-без-ID-не-дубль" "фикстура"; return; }
expect "S6-без-ID-не-дубль" "${d}" /dev/null 0 "-" "NOTE  ДУБЛЬ"

# --- FAIL-CLOSED на несостоявшемся setup'е -----------------------------------------------
# Не-git каталог: «наблюдать нечего» ОБЯЗАНО краснеть, а не печатать пустой список.
d="$(mktemp -d "${WORK}/notgit-XXXXXX")"; register "${d}"
expect "S7-не-репозиторий" "${d}" /dev/null 1 "SETUP" "VERDICT: PASS"

# Репозиторий есть, origin/main нет — отставание считать не от чего.
d="$(mktemp -d "${WORK}/nomain-XXXXXX")"; register "${d}"
( cd "${d}" && git init -q . && git config user.email t@t && git config user.name t \
  && echo x > a && git add a && git commit -qm x ) || { sfail "S8-без-origin-main" "фикстура"; return; }
expect "S8-без-origin-main" "${d}" /dev/null 1 "origin/main не существует" "VERDICT: PASS"

# Нечитаемый источник PR — тоже несостоявшийся setup, а не «PR-ов нет».
d="$(mk_repo unreadable feat/M-06-q)" || { sfail "S9-нечитаемый-PRS" "фикстура"; return; }
expect "S9-нечитаемый-PRS" "${d}" "${d}/no-such-file.tsv" 1 "нечитаем" "VERDICT: PASS"

# --- наблюдение ОТСУТСТВИЯ: ноль веток — законно, но должно быть НАПЕЧАТАНО ---------------
d="$(mk_repo empty)" || { sfail "S10-ноль-веток" "фикстура"; return; }
expect "S10-ноль-веток" "${d}" /dev/null 0 "веток кроме main: 0" "-"

# --- ЖИВОЙ ПУТЬ gh: четыре состояния и частичный отказ (C-107 F-106-3) --------------------
d="$(mk_repo ghfail feat/M-20-x)" || { sfail "S11-gh-list-отказ" "фикстура"; return; }
mk_gh "${d}" fail "" || { sfail "S11-gh-list-отказ" "поддельный gh"; return; }
expect_live "S11-gh-list-отказ" "${d}" "${d}" 1 "gh pr list отказал" "VERDICT: PASS"

d="$(mk_repo ghboom feat/M-21-y)" || { sfail "S12-checks-отказ" "фикстура"; return; }
mk_gh "${d}" "ok:feat/M-21-y:21" "21=boom" || { sfail "S12-checks-отказ" "gh"; return; }
expect_live "S12-checks-отказ" "${d}" "${d}" 1 "НЕИЗВЕСТНО" "VERDICT: PASS"

# ЧАСТИЧНЫЙ отказ — прямое требование C-107: известный результат обязан УЦЕЛЕТЬ,
# неизвестный — быть назван, прогон — красным.
d="$(mk_repo ghpartial feat/M-22-ok feat/M-23-bad)" || { sfail "S13-частичный-отказ" "фикстура"; return; }
mk_gh "${d}" "ok:feat/M-22-ok:22,feat/M-23-bad:23" "22=green,23=boom" || { sfail "S13-частичный-отказ" "gh"; return; }
run_live "${d}" "${d}"
if [ "${RC}" -eq 0 ]; then nok "S13-частичный-отказ" "exit=0 — отказ проглочен"
elif ! grep -qF "НЕИЗВЕСТНО: feat/M-23-bad" <<<"${OUT}"; then nok "S13-частичный-отказ" "не назван недоступный PR"
elif ! grep -qF "ВИСЯК: feat/M-22-ok" <<<"${OUT}"; then nok "S13-частичный-отказ" "ПОТЕРЯН известный результат соседнего PR"
else ok "S13-частичный-отказ" "известное уцелело, неизвестное названо, exit=${RC}"; fi

# «чеков нет вовсе» — ДОСТОВЕРНЫЙ ответ, а не отказ: наблюдение состоялось.
d="$(mk_repo ghnochecks feat/M-24-z)" || { sfail "S14-без-чеков" "фикстура"; return; }
mk_gh "${d}" "ok:feat/M-24-z:24" "24=nochecks" || { sfail "S14-без-чеков" "gh"; return; }
expect_live "S14-без-чеков" "${d}" "${d}" 0 "БЕЗ ЧЕКОВ: feat/M-24-z" "НЕИЗВЕСТНО: feat/M-24-z"

# красное остаётся красным и висяком не считается
d="$(mk_repo ghred feat/M-25-r)" || { sfail "S15-живой-red" "фикстура"; return; }
mk_gh "${d}" "ok:feat/M-25-r:25" "25=red" || { sfail "S15-живой-red" "gh"; return; }
expect_live "S15-живой-red" "${d}" "${d}" 0 "-" "NOTE  ВИСЯК"

# gh отсутствует вовсе — сценарий СНЯТ, и причина названа, а не умолчана.
# Замер: `gh` лежит СРАЗУ В ДВУХ каталогах PATH (/usr/bin и /bin). Убрать оба — значит унести
# вместе с ним bash, git, grep и весь инструментарий, и сценарий начал бы мерить хирургию над
# PATH вместо своего инварианта (`testing.md`: оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ).
# Класс «источник недоступен» при этом покрыт: S11 (отказ `gh pr list`) исполняет ту же ветку
# fail-closed по тому же коду возврата. Отдельный мутант M4 пиннит именно её.

}

# ═══ Батарея мутантов ═══════════════════════════════════════════════════════════════════
battery() {
  # Мутант целится в НЕСУЩУЮ строку — ту, что печатает находку, а не в счётчик рядом.
  # Первая редакция нейтрализовала `dups=$((dups+1))` и не убила НИ ОДНОГО сценария: счёт
  # управляет только строкой «ни один предмет не живёт больше чем на одной ветке», а сама
  # находка печаталась по-прежнему. Мутант, не роняющий свой сценарий, ничего не пиннит.
  local mutants=(
    # S13 принадлежит ДВУМ мутантам, и это не дефект: сценарий частичного отказа требует,
    # чтобы известный результат соседнего PR УЦЕЛЕЛ (агрегат ВИСЯК) и одновременно чтобы
    # недоступный был назван (агрегат НЕИЗВЕСТНО). Нейтрализация любого из двух его роняет.
    # Заявляем обе принадлежности явно — иначе kill-set «сойдётся» по недосмотру.
    "M1-ВИСЯК:STALE_GREEN+=:S1-висяк-зелёный S13-частичный-отказ"
    "M2-ДУБЛЬ:note \"ДУБЛЬ: предмет:S4-дубль"
    "M3-НЕИЗВЕСТНО:UNKNOWN_PRS+=:S12-checks-отказ S13-частичный-отказ"
    "M4-LIST-FAIL:LIST_RC}\" -eq 0 ]:S11-gh-list-отказ"
  )
  local bfail=0
  for spec in "${mutants[@]}"; do
    local name="${spec%%:*}" rest="${spec#*:}"
    local needle="${rest%%:*}" declared="${rest##*:}"
    local mut="${WORK}/mutant-${name}.sh"; register "${mut}"
    # Нейтрализуем накопитель агрегата: строка, добавляющая находку, вырезается.
    # Замена на no-op, а не вырезание: удаление строки внутри `if` оставляет пустое тело
    # и мутант перестаёт парситься — тогда проба мерила бы синтаксис, а не инвариант.
    awk -v n="${needle}" 'index($0,n){ sub(/[^ ].*/, ":"); } {print}' "${SUT}" > "${mut}"
    if cmp -s "${SUT}" "${mut}"; then
      printf 'SETUP-FAIL %-26s мутант не построен: «%s» не найдено\n' "${name}" "${needle}"
      bfail=$((bfail + 1)); continue
    fi
    if ! bash -n "${mut}" 2>/dev/null; then
      printf 'SETUP-FAIL %-26s мутант не парсится\n' "${name}"; bfail=$((bfail + 1)); continue
    fi
    local bp=${PASS} bf=${FAIL}; local saved=("${FAILED_NAMES[@]+"${FAILED_NAMES[@]}"}")
    PASS=0; FAIL=0; FAILED_NAMES=()
    SUT_ACTIVE="${mut}"; local out; out="$(scenarios 2>&1)"; SUT_ACTIVE="${SUT}"
    local killed; killed="$(grep '^FAIL' <<<"${out}" | awk '{print $2}' | sort | tr '\n' ' ')"
    PASS=${bp}; FAIL=${bf}; FAILED_NAMES=("${saved[@]+"${saved[@]}"}")
    local expect_set; expect_set="$(tr ' ' '\n' <<<"${declared}" | sed '/^$/d' | sort | tr '\n' ' ')"
    if [ "${killed}" = "${expect_set}" ]; then
      printf 'ok         %-26s kill-set совпал (%s)\n' "${name}" "${expect_set}"
    else
      printf 'FAIL       %-26s kill-set РАЗОШЁЛСЯ\n' "${name}"
      printf '           заявлено: %s\n           получено: %s\n' "${expect_set}" "${killed}"
      bfail=$((bfail + 1))
    fi
  done
  return ${bfail}
}

# ═══ Прогон ═════════════════════════════════════════════════════════════════════════════
[ -f "${SUT}" ] || { echo "SETUP-FAIL: предмет ${SUT} не найден"; exit 1; }
echo "── СЦЕНАРИИ (позитивный контроль + анти-плацебо в обе стороны + fail-closed setup)"
scenarios
BATT=0
if [ "${1:-}" = "--battery" ]; then
  echo; echo "── БАТАРЕЯ МУТАНТОВ (равенство kill-set'ов)"; battery || BATT=$?
fi

echo
echo "сценариев исполнено: $((PASS + FAIL))  ok: ${PASS}  FAIL: ${FAIL}"
[ ${#FAILED_NAMES[@]} -gt 0 ] && printf 'упали: %s\n' "${FAILED_NAMES[*]}"
cleanup; trap - EXIT
TMP_AFTER="$(own_dirs)"
echo "каталогов red-brhealth-* до: ${TMP_BEFORE}, после уборки: ${TMP_AFTER}"
[ "${TMP_AFTER}" -gt "${TMP_BEFORE}" ] && { echo "FAIL  проба течёт"; FAIL=$((FAIL + 1)); }
if [ "${FAIL}" -gt 0 ] || [ "${BATT}" -ne 0 ]; then
  echo "VERDICT: FAIL (сценариев: ${FAIL}, мутантов с разошедшимся kill-set: ${BATT})"; exit 1
fi
echo "VERDICT: PASS"
