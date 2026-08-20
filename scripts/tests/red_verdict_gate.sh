#!/usr/bin/env bash
# Проба барьера непогашенного отказа — предмет: `scripts/check_verdict_gate.sh`.
#
# Три обязательных свойства (`docs/workflow/harness-track.md` §5):
#   1. ПОЗИТИВНЫЙ КОНТРОЛЬ — честная фикстура (проходной вердикт, пустой диапазон) даёт exit=0.
#      Без него барьер мог бы быть вечно-красным, и его «объявят шумом и выключат».
#   2. АНТИ-ПЛАЦЕБО В ОБЕ СТОРОНЫ — отказ обязан быть ПОЙМАН там, где он есть, и НЕ выдуман
#      там, где его нет; на несостоявшемся setup'е обязан краснеть, а не печатать пустой
#      счастливый список.
#   3. МУТАЦИОННЫЙ КОНТРОЛЬ (`--battery`) — нейтрализация каждой несущей проверки роняет
#      РОВНО заявленный набор сценариев.
#
# ГЕРМЕТИЧНОСТЬ. Проба строит СВОИ git-репозитории в TMPDIR и в сеть не ходит вовсе. Ходила бы
# — мерила бы доступность GitHub, а не свой инвариант (класс `TD-135`).
#
# ОСЬ, РАДИ КОТОРОЙ ПРОБА СУЩЕСТВУЕТ: «непригодный документ → НЕИЗВЕСТНО → красное».
# Та же ось, что `A-011` §3 предписал наблюдателю веток. Значение вне перечня, пустое поле и
# отсутствующая шапка — ТРИ РАЗНЫХ состояния, и барьер обязан их различать: «похоже на
# проходной» состоянием не является.
#
# Прогон: bash scripts/tests/red_verdict_gate.sh [--battery]

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUT="${ROOT}/scripts/check_verdict_gate.sh"
LIB="${ROOT}/scripts/lib/gate_meta.sh"
CI_YML="${ROOT}/.github/workflows/ci.yml"
SUT_ACTIVE="${SUT}"

PASS=0; FAIL=0; FAILED_NAMES=()
ok()   { PASS=$((PASS + 1)); printf 'ok         %-30s %s\n' "$1" "${2:-}"; }
nok()  { FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'FAIL       %-30s %s\n' "$1" "$2"; }
sfail(){ FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'SETUP-FAIL %-30s %s\n' "$1" "$2"; }

own_dirs(){ find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'red-vgate-*' 2>/dev/null | wc -l; }
TMP_BEFORE="$(own_dirs)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/red-vgate-XXXXXX")"
REG="${WORK}/.fixtures"; : > "${REG}"
register(){ printf '%s\n' "$1" >> "${REG}"; }
cleanup(){ [ -f "${REG}" ] && while IFS= read -r p; do [ -n "$p" ] && [ -e "$p" ] && rm -rf "$p"; done < "${REG}"; rm -rf "${WORK}"; }
trap cleanup EXIT
register "${WORK}"

# Каталог мутантов несёт СВОЮ копию библиотеки: мутант резолвит `lib/gate_meta.sh` относительно
# себя, и без копии КАЖДЫЙ мутант падал бы в fail-closed «библиотека недоступна» — батарея
# мерила бы отсутствие файла, а не нейтрализованную проверку.
MUTDIR="${WORK}/mut"; mkdir -p "${MUTDIR}/lib"; cp "${LIB}" "${MUTDIR}/lib/gate_meta.sh"

# ─── фикстуры ────────────────────────────────────────────────────────────────────────────
mk_repo() { # → путь; репозиторий с одним базовым коммитом
  local name="$1"; local d
  d="$(mktemp -d "${WORK}/case-${name}-XXXXXX")" || return 1
  register "${d}"
  ( cd "${d}" && git init -q . && git config user.email t@t && git config user.name t \
    && mkdir -p research/critiques research/reviews research/arbitration \
    && echo base > f.txt && git add f.txt && git commit -qm "base" ) || return 1
  printf '%s' "${d}"
}
BASE_OF() { ( cd "$1" && git rev-list --max-parents=0 HEAD | tail -1 ); }

# mk_verdict <repo> <относительный путь> <ключ> <исход>
# Спец-значения: ключ `-` ⇒ поле milestone пустое; исход `-` ⇒ поле verdict пустое.
mk_verdict() {
  local d="$1" p="$2" key="$3" vd="$4"
  mkdir -p "${d}/$(dirname "${p}")"
  { echo "<!-- GATE-META"
    if [ "${key}" = "-" ]; then echo "milestone:"; else echo "milestone: ${key}"; fi
    echo "audited_repo: a3ka/hft-platform"
    echo "audited_base: 1111111111111111111111111111111111111111"
    echo "audited_head: 2222222222222222222222222222222222222222"
    if [ "${vd}" = "-" ]; then echo "verdict:"; else echo "verdict: ${vd}"; fi
    echo "-->"
    echo
    echo "# тело вердикта"
  } > "${d}/${p}"
}
mk_headless() { # вердикт БЕЗ шапки
  local d="$1" p="$2"
  mkdir -p "${d}/$(dirname "${p}")"
  printf '# вердикт без шапки\n' > "${d}/${p}"
}
commit_all() { ( cd "$1" && git add -A && git commit -qm "$2" ); }

# ─── прогон предмета ─────────────────────────────────────────────────────────────────────
run_sut() { # <repo> <base> [event] [prpush]
  local d="$1" base="$2" ev="${3:-pull_request}" mode="${4:-pr}"
  local pb="" pu=""
  case "${mode}" in pr) pb="${base}" ;; push) pu="${base}" ;; esac
  OUT="$( cd "${d}" && EVENT_NAME="${ev}" PUSH_BEFORE="${pu}" PR_BASE_SHA="${pb}" \
          bash "${SUT_ACTIVE}" 2>&1 )"; RC=$?
}

# expect <имя> <repo> <base> <код> <обязательно|-> <запрещено|->
expect() {
  local name="$1" d="$2" base="$3" wantrc="$4" must="$5" mustnot="$6"
  run_sut "${d}" "${base}"
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

# ─── оракул ПРОВОДКИ В АГРЕГАТ ───────────────────────────────────────────────────────────
# Проверка ИСПОЛНЕНИЕМ условия, а не грепом по имени: греп зелен и против закомментированной
# строки, и против упоминания джоба в соседнем `echo`. Замер соседа (`deploy_catchup.py
# check-aggregate`): два стаба — «джоб выкинут из условия» и «выкинут из needs и из условия» —
# давали полный зелёный прогон 39/39, то есть были НЕОТЛИЧИМЫ от честной проводки.
#
# aggregate_verdict <ci.yml> <имя джоба> → печатает OK/причину, код 0/1
aggregate_verdict() {
  local yml="$1" job="$2" cond model rc_fail rc_ok
  [ -r "${yml}" ] || { echo "нечитаем ${yml}"; return 1; }

  # (а) джоб обязан стоять в `needs` агрегата — иначе он не выполнится ДО него вовсе
  local needs
  needs="$(awk '/^  status-check:/{s=1} s&&/^    needs:/{print;exit}' "${yml}")"
  [ -n "${needs}" ] || { echo "у status-check нет строки needs"; return 1; }
  grep -qE "(\[|, )${job}(,|\])" <<<"${needs}" || { echo "джоб ${job} отсутствует в needs агрегата"; return 1; }

  # (б) условие обязано РОНЯТЬ агрегат при красном джобе — проверяется исполнением
  cond="$(grep -m1 -F 'if [[ "${{ needs.' "${yml}")" || true
  [ -n "${cond}" ] || { echo "не найдено fail-closed условие агрегата"; return 1; }

  # модель: интересующий джоб = failure, все прочие = success
  model="$(sed -E "s/\\\$\{\{ needs\.${job}\.result \}\}/failure/g; s/\\\$\{\{ needs\.[a-zA-Z-]+\.result \}\}/success/g" <<<"${cond}")"
  bash -c "${model} exit 1; fi; exit 0" >/dev/null 2>&1; rc_fail=$?

  # контроль: ВСЕ зелёные ⇒ условие не срабатывает. Без него (б) проходит вакуумно —
  # условие, красное всегда, тоже «уронило бы» агрегат.
  local allok
  allok="$(sed -E "s/\\\$\{\{ needs\.[a-zA-Z-]+\.result \}\}/success/g" <<<"${cond}")"
  bash -c "${allok} exit 1; fi; exit 0" >/dev/null 2>&1; rc_ok=$?

  if [ "${rc_fail}" -ne 1 ]; then echo "красный ${job} НЕ роняет агрегат (условие вернуло ${rc_fail})"; return 1; fi
  if [ "${rc_ok}" -ne 0 ]; then echo "агрегат красен при всех зелёных (условие вернуло ${rc_ok}) — вечно-красный гейт"; return 1; fi
  echo "OK"; return 0
}

# ═══ СЦЕНАРИИ ════════════════════════════════════════════════════════════════════════════
scenarios() {

# ── позитивный контроль ─────────────────────────────────────────────────────────────────
d="$(mk_repo pass)" || { sfail "VG-1-проходной" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/reviews/R-500-x.md M-70 APPROVE; commit_all "$d" "вердикт APPROVE" >/dev/null
expect "VG-1-проходной" "$d" "$B" 0 "VERDICT: PASS" "FAIL"

# Пустой диапазон — законное состояние, и оно ПЕЧАТАЕТСЯ (наблюдение ОТСУТСТВИЯ).
d="$(mk_repo empty)" || { sfail "VG-2-пустой-диапазон" "фикстура"; return; }
B="$(BASE_OF "$d")"
( cd "$d" && echo x > other.txt && git add -A && git commit -qm "не вердикт" ) >/dev/null
expect "VG-2-пустой-диапазон" "$d" "$B" 0 "судимых 0" "-"

# ── отказные исходы: каждый обязан держать merge ────────────────────────────────────────
for pair in "VG-3-REJECT:REJECT" "VG-4-KILL:KILL" "VG-5-ESCALATE:ESCALATE" "VG-6-CONCERNS:CONCERNS"; do
  nm="${pair%%:*}"; vv="${pair##*:}"
  d="$(mk_repo "${vv}")" || { sfail "${nm}" "фикстура"; return; }
  B="$(BASE_OF "$d")"
  mk_verdict "$d" research/critiques/C-500-x.md M-71 "${vv}"; commit_all "$d" "вердикт ${vv}" >/dev/null
  expect "${nm}" "$d" "$B" 1 "ключ M-71" "-"
done

# ── порядок решает: погашение и анти-плацебо к нему ─────────────────────────────────────
d="$(mk_repo cured)" || { sfail "VG-7-отказ-погашен" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-501-a.md M-72 REJECT; commit_all "$d" "круг 1 REJECT" >/dev/null
mk_verdict "$d" research/reviews/R-501-b.md M-72 APPROVE; commit_all "$d" "круг 2 APPROVE" >/dev/null
expect "VG-7-отказ-погашен" "$d" "$B" 0 "VERDICT: PASS" "FAIL  ключ"

# Обратный порядок обязан краснеть — иначе VG-7 доказывал бы лишь «где-то есть APPROVE».
d="$(mk_repo regressed)" || { sfail "VG-8-погашение-раньше-отказа" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/reviews/R-502-a.md M-73 APPROVE; commit_all "$d" "сначала APPROVE" >/dev/null
mk_verdict "$d" research/critiques/C-502-b.md M-73 REJECT; commit_all "$d" "потом REJECT" >/dev/null
expect "VG-8-погашение-раньше-отказа" "$d" "$B" 1 "ключ M-73" "-"

# Один коммит, два исхода на ключ: «позже» не определено ⇒ отказ побеждает (fail-closed).
#
# ПОРЯДОК ФАЙЛОВ В ФИКСТУРЕ ЗНАЧИМ, и первая редакция этого не учла: `git diff --name-status`
# отдаёт пути ОТСОРТИРОВАННЫМИ, и при отказе, стоящем первым, тай разрешается ещё веткой
# «позиция больше предыдущей» — а не тай-брейком. Батарея это и показала: мутант, снимающий
# отметку отказа при РАВНЫХ позициях, не убивал сценарий вовсе, то есть сценарий был зелен по
# чужой причине. Здесь проходной исход стоит ПЕРВЫМ (`arbitration/` < `critiques/`), и красное
# может дать только тай-брейк.
d="$(mk_repo tie)" || { sfail "VG-9-тай-в-одном-коммите" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/arbitration/A-503-a.md M-74 APPROVE
mk_verdict "$d" research/critiques/C-503-b.md M-74 REJECT
commit_all "$d" "проходной и отказной в одном коммите, проходной первым по сортировке" >/dev/null
expect "VG-9-тай-в-одном-коммите" "$d" "$B" 1 "ключ M-74" "-"

# Зеркало: отказ первым по сортировке. Оба порядка обязаны давать красное — иначе исход
# барьера зависел бы от имён файлов, то есть от случайности.
d="$(mk_repo tierev)" || { sfail "VG-34-тай-обратный-порядок" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/arbitration/A-504-a.md M-92 REJECT
mk_verdict "$d" research/reviews/R-504-b.md M-92 APPROVE
commit_all "$d" "отказной первым по сортировке" >/dev/null
expect "VG-34-тай-обратный-порядок" "$d" "$B" 1 "ключ M-92" "-"

# ── ключи не смешиваются ────────────────────────────────────────────────────────────────
d="$(mk_repo twokeys)" || { sfail "VG-10-чужой-ключ-не-гасит" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-504-a.md M-75 REJECT
mk_verdict "$d" research/reviews/R-504-b.md M-76 APPROVE
commit_all "$d" "разные ключи" >/dev/null
expect "VG-10-чужой-ключ-не-гасит" "$d" "$B" 1 "ключ M-75" "ключ M-76"

# Префикс не есть тот же ключ: M-60 и M-60b — РАЗНЫЕ предметы.
d="$(mk_repo prefix)" || { sfail "VG-11-префикс-не-тот-же-ключ" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-505-a.md M-60 REJECT
mk_verdict "$d" research/reviews/R-505-b.md M-60b APPROVE
commit_all "$d" "M-60 и M-60b" >/dev/null
expect "VG-11-префикс-не-тот-же-ключ" "$d" "$B" 1 "ключ M-60:" "-"

# Два отказных ключа — ОБА напечатаны: первый не закрывает собой второй.
d="$(mk_repo bothbad)" || { sfail "VG-12-оба-ключа-названы" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-506-a.md M-77 REJECT
mk_verdict "$d" research/critiques/C-506-b.md M-78 KILL
commit_all "$d" "два отказа" >/dev/null
run_sut "$d" "$B"
if [ "${RC}" -ne 1 ]; then nok "VG-12-оба-ключа-названы" "exit=${RC}"
elif ! grep -qF "ключ M-77" <<<"${OUT}"; then nok "VG-12-оба-ключа-названы" "M-77 не назван"
elif ! grep -qF "ключ M-78" <<<"${OUT}"; then nok "VG-12-оба-ключа-названы" "M-78 не назван — первый закрыл собой второй"
else ok "VG-12-оба-ключа-названы" "exit=${RC}"; fi

# ── ось «непригодный документ → НЕИЗВЕСТНО → красное» ───────────────────────────────────
# Значение вне перечня. `BLOCKED-ARBITER` — не выдумка: такая шапка лежит в origin/main (R-042).
d="$(mk_repo unkverd)" || { sfail "VG-13-исход-вне-перечня" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/reviews/R-507-x.md M-79 BLOCKED-ARBITER; commit_all "$d" "чужой исход" >/dev/null
expect "VG-13-исход-вне-перечня" "$d" "$B" 1 "вне перечня" "VERDICT: PASS"

d="$(mk_repo emptyverd)" || { sfail "VG-14-исход-пуст" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/reviews/R-508-x.md M-80 -; commit_all "$d" "пустой verdict" >/dev/null
expect "VG-14-исход-пуст" "$d" "$B" 1 "verdict пусто" "VERDICT: PASS"

d="$(mk_repo emptykey)" || { sfail "VG-15-ключ-пуст" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/reviews/R-509-x.md - REJECT; commit_all "$d" "пустой milestone" >/dev/null
expect "VG-15-ключ-пуст" "$d" "$B" 1 "milestone пусто" "VERDICT: PASS"

# Шапки нет вовсе — ключа нет, ключевое правило неприменимо. Барьер НЕ роняет прогон сам
# (это зона соседнего `check_gate_meta.sh`), но обязан НАЗВАТЬ файл и СОСЧИТАТЬ его: молчание
# здесь читалось бы как «вердиктов не было».
d="$(mk_repo headless)" || { sfail "VG-16-без-шапки-назван" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_headless "$d" research/critiques/C-510-x.md; commit_all "$d" "вердикт без шапки" >/dev/null
expect "VG-16-без-шапки-назван" "$d" "$B" 0 "без шапки 1" "-"

# ── токен погашения и его границы ───────────────────────────────────────────────────────
d="$(mk_repo tok)" || { sfail "VG-17-токен-гасит" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-511-x.md M-81 REJECT
( cd "$d" && git add -A && git commit -qm "отказ

VERDICT-CLEARED: M-81 — находки устранены отдельным кругом, вердикт круга 2 на ветке предмета" ) >/dev/null
expect "VG-17-токен-гасит" "$d" "$B" 0 "погашен явным VERDICT-CLEARED" "-"

# Ритуальный токен без причины не открывает: иначе он неотличим от своего отсутствия.
d="$(mk_repo tokshort)" || { sfail "VG-18-короткая-причина" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-512-x.md M-82 REJECT
( cd "$d" && git add -A && git commit -qm "отказ

VERDICT-CLEARED: M-82 — ок" ) >/dev/null
expect "VG-18-короткая-причина" "$d" "$B" 1 "ключ M-82" "-"

# Токен ПОКЛЮЧЕВОЙ: названный ключ не открывает соседний.
d="$(mk_repo tokother)" || { sfail "VG-19-токен-на-чужой-ключ" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-513-x.md M-83 REJECT
( cd "$d" && git add -A && git commit -qm "отказ

VERDICT-CLEARED: M-84 — совсем другой предмет, этот ключ здесь ни при чём" ) >/dev/null
expect "VG-19-токен-на-чужой-ключ" "$d" "$B" 1 "ключ M-83" "-"

# Граница слова: токен на `M-60` не смеет гасить `M-60b`.
d="$(mk_repo tokprefix)" || { sfail "VG-20-токен-по-границе-слова" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-514-x.md M-60b REJECT
( cd "$d" && git add -A && git commit -qm "отказ

VERDICT-CLEARED: M-60 — предмет с коротким номером, к M-60b отношения не имеет" ) >/dev/null
expect "VG-20-токен-по-границе-слова" "$d" "$B" 1 "ключ M-60b" "-"

# Токен, ЦИТИРУЕМЫЙ в теле файла, барьер сам себе не открывает (тело коммита ≠ тело файла).
d="$(mk_repo tokinfile)" || { sfail "VG-21-цитата-не-токен" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-515-x.md M-85 REJECT
printf '\nVERDICT-CLEARED: M-85 — цитата внутри файла, а не решение в теле коммита\n' >> "$d/research/critiques/C-515-x.md"
commit_all "$d" "отказ с цитатой токена в теле файла" >/dev/null
expect "VG-21-цитата-не-токен" "$d" "$B" 1 "ключ M-85" "-"

# ── граница предмета: диапазон, а не дерево ─────────────────────────────────────────────
d="$(mk_repo inbase)" || { sfail "VG-22-вердикт-в-базе-не-судится" "фикстура"; return; }
mk_verdict "$d" research/critiques/C-516-x.md M-86 REJECT; commit_all "$d" "отказ ДО базы" >/dev/null
B="$( cd "$d" && git rev-parse HEAD )"
( cd "$d" && echo y > z.txt && git add -A && git commit -qm "работа после базы" ) >/dev/null
expect "VG-22-вердикт-в-базе-не-судится" "$d" "$B" 0 "судимых 0" "-"

# Файл вне каталогов вердиктов не судится, даже неся шапку с отказом.
d="$(mk_repo outside)" || { sfail "VG-23-вне-каталогов-не-судится" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/reports/R-999-x.md M-87 REJECT; commit_all "$d" "отчёт, не вердикт" >/dev/null
expect "VG-23-вне-каталогов-не-судится" "$d" "$B" 0 "судимых 0" "-"

# ── fail-closed на несостоявшемся setup'е ───────────────────────────────────────────────
d="$(mk_repo failclosed)" || { sfail "VG-24-нет-события" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-517-x.md M-88 APPROVE; commit_all "$d" "проходной" >/dev/null

OUT="$( cd "$d" && env -u EVENT_NAME -u PUSH_BEFORE -u PR_BASE_SHA bash "${SUT_ACTIVE}" 2>&1 )"; RC=$?
if [ "${RC}" -eq 2 ]; then ok "VG-24-нет-события" "exit=2"; else nok "VG-24-нет-события" "exit=${RC}, ожидался 2"; fi

OUT="$( cd "$d" && EVENT_NAME=schedule PUSH_BEFORE="" PR_BASE_SHA="" bash "${SUT_ACTIVE}" 2>&1 )"; RC=$?
if [ "${RC}" -eq 2 ]; then ok "VG-25-чужое-событие" "exit=2"; else nok "VG-25-чужое-событие" "exit=${RC}, ожидался 2"; fi

ZERO=0000000000000000000000000000000000000000
run_sut "$d" "${ZERO}"
if [ "${RC}" -eq 2 ]; then ok "VG-26-zero-SHA" "exit=2"; else nok "VG-26-zero-SHA" "exit=${RC}, ожидался 2"; fi

# База СУЩЕСТВУЕТ, но НЕ предок HEAD — боковая линия. Отдельная ось от «базы нет вовсе»:
# объект резолвится, и барьер обязан отвергнуть его вторым условием, а не первым.
# Фикстура строит боковую ветку В ТОМ ЖЕ репозитории намеренно. Первая редакция брала базу из
# ВТОРОГО репозитория — и сценарий молча тестировал не то: `mk_repo` порождает у обоих
# побитово одинаковый корневой коммит (одно дерево, автор, сообщение и та же секунда), то есть
# ОДИН И ТОТ ЖЕ SHA, который предком быть обязан. Проба зеленела бы на честном барьере.
SIDE="$( cd "$d" && git checkout -q -b side-alien HEAD~1 \
        && echo alien > alien.txt && git add -A \
        && git commit -qm "боковая линия: существует, но не предок основной" \
        && git rev-parse HEAD )" || { sfail "VG-27-база-не-предок" "фикстура боковой ветки"; return; }
( cd "$d" && git checkout -q - ) || { sfail "VG-27-база-не-предок" "возврат на основную"; return; }
run_sut "$d" "${SIDE}"
if [ "${RC}" -eq 2 ]; then ok "VG-27-база-не-предок" "exit=2"; else nok "VG-27-база-не-предок" "exit=${RC}, ожидался 2"; fi

# База, которой в истории НЕТ вовсе (поверхностный клон / force-push) — тоже fail-closed.
run_sut "$d" "dead0000dead0000dead0000dead0000dead0000"
if [ "${RC}" -eq 2 ]; then ok "VG-33-базы-нет-в-истории" "exit=2"; else nok "VG-33-базы-нет-в-истории" "exit=${RC}, ожидался 2"; fi

# Библиотека разбора отсутствует ⇒ fail-closed. Барьер, source'ящий её через `|| true`,
# превратил бы пропажу парсера в «шапок нет» — ровно тот класс, против которого он написан.
NOLIB="${WORK}/nolib"; mkdir -p "${NOLIB}"; register "${NOLIB}"
cp "${SUT_ACTIVE}" "${NOLIB}/check_verdict_gate.sh"
d3="$(mk_repo nolib)" || { sfail "VG-28-нет-библиотеки" "фикстура"; return; }
B3="$(BASE_OF "$d3")"
mk_verdict "$d3" research/critiques/C-518-x.md M-89 APPROVE; commit_all "$d3" "проходной" >/dev/null
OUT="$( cd "$d3" && EVENT_NAME=pull_request PR_BASE_SHA="$B3" bash "${NOLIB}/check_verdict_gate.sh" 2>&1 )"; RC=$?
if [ "${RC}" -eq 2 ] && grep -qF "библиотека разбора" <<<"${OUT}"; then ok "VG-28-нет-библиотеки" "exit=2"
else nok "VG-28-нет-библиотеки" "exit=${RC}, ожидался 2 с указанием библиотеки"; fi

# ── прод-форма push: база берётся из PUSH_BEFORE ────────────────────────────────────────
d="$(mk_repo pushform)" || { sfail "VG-29-прод-форма-push" "фикстура"; return; }
B="$(BASE_OF "$d")"
mk_verdict "$d" research/critiques/C-519-x.md M-90 REJECT; commit_all "$d" "отказ" >/dev/null
run_sut "$d" "$B" push push
if [ "${RC}" -eq 1 ] && grep -qF "ключ M-90" <<<"${OUT}"; then ok "VG-29-прод-форма-push" "exit=1"
else nok "VG-29-прод-форма-push" "exit=${RC}: push-переменная не читается"; fi

# ── вердикт, РОЖДЁННЫЙ merge'ем (evil merge) ────────────────────────────────────────────
# Проверка ПОВЕДЕНИЕМ, а не ветвью реализации: файл, появившийся только в merge-коммите,
# обязан быть УВИДЕН и задержать merge — каким бы путём барьер ни определил его позицию.
# Диапазонный `git diff BASE HEAD` его видит всегда; обход коммитов — не обязательно, и
# для такого файла в барьере предусмотрен fail-closed путь «позиция неизвестна ⇒ отказ
# засчитан». Сценарий держит именно ИСХОД, поэтому переживёт правку внутренностей.
d="$(mk_repo evil)" || { sfail "VG-32-merge-born" "фикстура"; return; }
B="$(BASE_OF "$d")"
(
  cd "$d" || exit 1
  git checkout -q -b side
  echo side > side.txt && git add -A && git commit -qm "работа на боковой ветке"
  git checkout -q master 2>/dev/null || git checkout -q main
  echo trunk > trunk.txt && git add -A && git commit -qm "работа на стволе"
  git merge -q --no-commit --no-ff side >/dev/null 2>&1 || true
) || { sfail "VG-32-merge-born" "фикстура merge"; return; }
mk_verdict "$d" research/critiques/C-520-x.md M-91 REJECT
( cd "$d" && git add -A && git commit -qm "evil merge: вердикт рождён самим слиянием" ) >/dev/null
expect "VG-32-merge-born" "$d" "$B" 1 "ключ M-91" "-"

# ── ПРОВОДКА В АГРЕГАТ (исполнением условия, не грепом) ─────────────────────────────────
res="$(aggregate_verdict "${CI_YML}" verdict-gate)"
if [ "${res}" = "OK" ]; then ok "VG-30-джоб-в-агрегате" "красный verdict-gate роняет All checks passed"
else nok "VG-30-джоб-в-агрегате" "${res}"; fi

# Анти-плацебо САМОГО оракула проводки: на ci.yml, где джоб выкинут из условия, он обязан
# краснеть. Без этого VG-30 доказывал бы лишь, что функция вернула «OK».
FAKE="${WORK}/ci-unwired.yml"; register "${FAKE}"
sed -E 's/\|\| "\$\{\{ needs\.verdict-gate\.result \}\}" != "success" //' "${CI_YML}" > "${FAKE}"
if cmp -s "${CI_YML}" "${FAKE}"; then
  sfail "VG-31-оракул-проводки-ловит" "стаб не построен: условие не содержит needs.verdict-gate"
else
  res="$(aggregate_verdict "${FAKE}" verdict-gate)"
  if [ "${res}" = "OK" ]; then nok "VG-31-оракул-проводки-ловит" "оракул зелен против ci.yml БЕЗ джоба в условии"
  else ok "VG-31-оракул-проводки-ловит" "стаб пойман: ${res}"; fi
fi

}

# ═══ БАТАРЕЯ МУТАНТОВ ════════════════════════════════════════════════════════════════════
# Мутант целится в НЕСУЩУЮ строку — ту, что производит решение, а не в счётчик рядом.
battery() {
  local mutants=(
    # Отказ перестаёт отмечаться ⇒ падают ВСЕ сценарии, где отказ обязан держать merge, И
    # сценарий токена: гасить становится нечего, NOTE о погашении не печатается.
    "B1-ОТКАЗ:gate_meta_is_passing \"\${vd}\" || refused=1:VG-3-REJECT VG-4-KILL VG-5-ESCALATE VG-6-CONCERNS VG-8-погашение-раньше-отказа VG-9-тай-в-одном-коммите VG-34-тай-обратный-порядок VG-10-чужой-ключ-не-гасит VG-11-префикс-не-тот-же-ключ VG-12-оба-ключа-названы VG-17-токен-гасит VG-18-короткая-причина VG-19-токен-на-чужой-ключ VG-20-токен-по-границе-слова VG-21-цитата-не-токен VG-29-прод-форма-push VG-32-merge-born"
    # Перестаёт печататься находка по ключу ⇒ падают ключевые сценарии, но НЕ VG-17: путь
    # токена лежит ДО этой строки. Различие B1/B2 в одном сценарии — оно и показывает, что
    # мутанты пиннят РАЗНОЕ, а не дублируют друг друга.
    "B2-ПЕЧАТЬ-КЛЮЧА:bad \"ключ \${key}:VG-3-REJECT VG-4-KILL VG-5-ESCALATE VG-6-CONCERNS VG-8-погашение-раньше-отказа VG-9-тай-в-одном-коммите VG-34-тай-обратный-порядок VG-10-чужой-ключ-не-гасит VG-11-префикс-не-тот-же-ключ VG-12-оба-ключа-названы VG-18-короткая-причина VG-19-токен-на-чужой-ключ VG-20-токен-по-границе-слова VG-21-цитата-не-токен VG-29-прод-форма-push VG-32-merge-born"
    # Исход вне перечня перестаёт краснеть ⇒ падает ровно ось «НЕИЗВЕСТНО».
    # Мутируется ПЕЧАТЬ, а не строка `if … then`: замена условия на no-op оставила бы висячий
    # `fi`, мутант не распарсился бы, и проба мерила бы синтаксис вместо инварианта.
    "B3-ПЕРЕЧЕНЬ:bad \"\${f}: verdict «\${vd}» вне перечня:VG-13-исход-вне-перечня"
    # Отметка отказа при равных позициях снимается ⇒ тай перестаёт решаться fail-closed.
    "B4-ТАЙ:KEY_REFUSED[\"\${key}\"]=1:VG-9-тай-в-одном-коммите"
    # Порог причины токена снимается ⇒ ритуальный токен начинает открывать барьер.
    "B5-ПОРОГ-ПРИЧИНЫ:[ \"\${#__r}\" -ge 12 ] || continue:VG-18-короткая-причина"
    # Токен перестаёт быть поключевым ⇒ чужой ключ и префикс начинают гасить.
    "B6-КЛЮЧ-ТОКЕНА:[ \"\${tokkey}\" = \"\$1\" ] || continue:VG-19-токен-на-чужой-ключ VG-20-токен-по-границе-слова"
  )
  local bfail=0
  for spec in "${mutants[@]}"; do
    local name="${spec%%:*}" rest="${spec#*:}"
    local needle="${rest%:*}" declared="${rest##*:}"
    local mut="${MUTDIR}/mutant-${name}.sh"
    # Замена на no-op, а не вырезание: удалённая строка внутри `if` оставила бы пустое тело,
    # мутант перестал бы парситься — и проба мерила бы синтаксис, а не инвариант.
    awk -v n="${needle}" 'index($0,n){ sub(/[^ ].*/, ":"); } {print}' "${SUT}" > "${mut}"
    if cmp -s "${SUT}" "${mut}"; then
      printf 'SETUP-FAIL %-30s мутант не построен: «%s» не найдено\n' "${name}" "${needle}"
      bfail=$((bfail + 1)); continue
    fi
    if ! bash -n "${mut}" 2>/dev/null; then
      printf 'SETUP-FAIL %-30s мутант не парсится\n' "${name}"; bfail=$((bfail + 1)); continue
    fi
    local bp=${PASS} bf=${FAIL}; local saved=("${FAILED_NAMES[@]+"${FAILED_NAMES[@]}"}")
    PASS=0; FAIL=0; FAILED_NAMES=()
    SUT_ACTIVE="${mut}"; local out; out="$(scenarios 2>&1)"; SUT_ACTIVE="${SUT}"
    local killed; killed="$(grep -E '^(FAIL|SETUP-FAIL)' <<<"${out}" | awk '{print $2}' | sort | tr '\n' ' ')"
    PASS=${bp}; FAIL=${bf}; FAILED_NAMES=("${saved[@]+"${saved[@]}"}")
    local expect_set; expect_set="$(tr ' ' '\n' <<<"${declared}" | sed '/^$/d' | sort | tr '\n' ' ')"
    if [ "${killed}" = "${expect_set}" ]; then
      printf 'ok         %-30s kill-set совпал (%s сценариев)\n' "${name}" "$(wc -w <<<"${expect_set}")"
    else
      printf 'FAIL       %-30s kill-set РАЗОШЁЛСЯ\n' "${name}"
      printf '           заявлено: %s\n           получено: %s\n' "${expect_set}" "${killed}"
      bfail=$((bfail + 1))
    fi
  done
  return ${bfail}
}

# ═══ ПРОГОН ══════════════════════════════════════════════════════════════════════════════
[ -f "${SUT}" ] || { echo "SETUP-FAIL: предмет ${SUT} не найден"; exit 1; }
[ -f "${LIB}" ] || { echo "SETUP-FAIL: библиотека ${LIB} не найдена"; exit 1; }
echo "── СЦЕНАРИИ (позитивный контроль + анти-плацебо в обе стороны + fail-closed + проводка)"
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
echo "каталогов red-vgate-* до: ${TMP_BEFORE}, после уборки: ${TMP_AFTER}"
[ "${TMP_AFTER}" -gt "${TMP_BEFORE}" ] && { echo "FAIL  проба течёт"; FAIL=$((FAIL + 1)); }
if [ "${FAIL}" -gt 0 ] || [ "${BATT}" -ne 0 ]; then
  echo "VERDICT: FAIL (сценариев: ${FAIL}, мутантов с разошедшимся kill-set: ${BATT})"; exit 1
fi
echo "VERDICT: PASS"
