#!/usr/bin/env bash
# Проба сторожа «деплой не состоялся» (Р-4, docs/plans/process-decisions-2026-08-14.md).
#
# Предмет — `scripts/deploy_catchup.py`: решение о доборе (DEPLOY/SKIP/HOLD) и барьер
# проводки `deploy.yml`. Проба держит ТРИ обязательных свойства харнесс-трека
# (`docs/workflow/harness-track.md` §5):
#
#   1. ПОЗИТИВНЫЙ КОНТРОЛЬ — честная реализация зелена целиком;
#   2. АНТИ-ПЛАЦЕБО стабами — обманная проводка и обманные раны обязаны краснеть;
#   3. МУТАЦИОННЫЙ КОНТРОЛЬ (`--battery`) — каждый мутант роняет РОВНО заявленное
#      множество сценариев: ни больше (сломан сверх своей дыры), ни меньше (дыра не
#      закреплена). Равенство kill-set'ов, а не «хоть что-то упало».
#
# Главный стаб, ради которого проба существует: **«сторож молчит, когда деплоя не было»**
# (мутант M1). Механизм, наблюдающий сбой, но не ОТСУТСТВИЕ, — это ровно та слепота,
# которую Р-4 закрывает (`testing.md`, целостность гейта, свойство 4).
#
# Сценарии — по ВЫЗОВУ (исполнением), а не по тексту: grep по имени функции был бы зелен
# и против мёртвого кода. У каждого сценария setup-guard: не состоявшаяся подготовка —
# FAIL сценария, а не тихий пропуск (свойство 3 там же).
#
# Прогон: bash scripts/tests/red_deploy_catchup.sh [--battery]

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUT="${ROOT}/scripts/deploy_catchup.py"
DEPLOY_YML="${ROOT}/.github/workflows/deploy.yml"
CI_YML="${ROOT}/.github/workflows/ci.yml"

# Код возврата, которым HOLD ЗОВЁТ ЧЕЛОВЕКА (`C-093` R-2). Ноль здесь означал бы, что
# «сторож сработал» и «всё хорошо» — одно и то же состояние для GitHub Actions: job зелёный,
# deploy skipped, ран терминально зелёный, то есть ВНЕШНЕ НЕОТЛИЧИМЫЙ от успешной доставки.
# Единица занята отказом входа (`Fail`): сливать «зову человека» с «я сломался» нельзя —
# это разные состояния с разными действиями оператора.
HOLD_RC=2

PASS=0
FAIL=0
FAILED_NAMES=()

# --- уборка фикстур: РЕЕСТР В ФАЙЛЕ + trap EXIT (harness-track.md §5 п.5) ---------------
# Класс, давший 10 400 каталогов в /tmp и диск на 100 %: фикстуры, о которых знал только
# живой процесс. Реестр переживает падение процесса, поэтому уборка — файловая.
WORK="$(mktemp -d "${TMPDIR:-/tmp}/red-catchup-XXXXXX")"
REGISTRY="${WORK}/.fixtures"
: > "${REGISTRY}"
# Замер уборки считает СВОИ каталоги по префиксу, а не всё содержимое TMPDIR.
#
# Прежняя редакция брала `find "${TMPDIR}" -maxdepth 1 -type d | wc -l` — то есть мерила
# ОКРУЖЕНИЕ, а не собственный инвариант (`testing.md`, целостность гейта, свойство 2).
# Замер 17.08 при параллельно работавшем агенте: «до 4539, после 4543» при НУЛЕ своих
# каталогов — четыре `tmp.*` завёл чужой процесс. Величина, которую читают как «проба
# течёт», на деле зависела от того, кто ещё работает на хосте. Тот же класс, что барьер
# ресурсных оракулов: «мерит прокси вместо ресурса».
own_dirs() { find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'red-catchup-*' 2>/dev/null | wc -l; }
TMP_BEFORE="$(own_dirs)"

register() { printf '%s\n' "$1" >> "${REGISTRY}"; }

cleanup() {
  if [ -f "${REGISTRY}" ]; then
    while IFS= read -r path; do
      [ -n "${path}" ] && [ -e "${path}" ] && rm -rf "${path}"
    done < "${REGISTRY}"
  fi
  rm -rf "${WORK}"
}
trap cleanup EXIT
register "${WORK}"

# --- инфраструктура сценариев -----------------------------------------------------------

setup_fail() {
  FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1")
  printf 'SETUP-FAIL %-28s %s\n' "$1" "$2"
}

ok()   { PASS=$((PASS + 1)); printf 'ok   %-28s %s\n' "$1" "${2:-}"; }
nok()  { FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'FAIL %-28s %s\n' "$1" "$2"; }

# Прогон предмета в режиме decide. Возвращает stdout в OUT, код в RC.
run_decide() {
  local target="$1" deployed="$2" runs="$3" yml="${4:-${DEPLOY_YML}}" root="${5:-${FIXREPO}}"
  # SUT_ACTIVE подменяется батареей на мутанта; в обычном прогоне — сам предмет.
  OUT="$(CATCHUP_REPO_ROOT="${root}" \
         CATCHUP_DEPLOY_YML="${yml}" \
         CATCHUP_TARGET_SHA="${target}" \
         CATCHUP_DEPLOYED_SHA="${deployed}" \
         CATCHUP_RUNS_JSON="${runs}" \
         python3 "${SUT_ACTIVE:-${SUT}}" decide 2>&1)"
  RC=$?
}

# Сценарий decide: ожидаемое решение + обязательная подстрока ПРИЧИНЫ.
# Причина проверяется не для красоты: без неё мутант, дающий верное решение по неверному
# основанию, проходит незамеченным (у предмета эшелонированная защита, и внешний исход
# у разных веток совпадает).
expect_decision() {
  local name="$1" want="$2" want_reason="$3" target="$4" deployed="$5" runs="$6"
  [ -f "${runs}" ] || { setup_fail "${name}" "фикстура ранов не создана: ${runs}"; return; }
  run_decide "${target}" "${deployed}" "${runs}"
  local got; got="$(printf '%s\n' "${OUT}" | sed -n 's/^decision=//p' | head -1)"
  # ЭСКАЛАЦИЯ — часть контракта решения, а не косметика вывода. HOLD обязан выйти
  # НЕНУЛЕВЫМ (иначе он не наблюдаем человеком), DEPLOY/SKIP — нулевым (иначе «падать
  # всегда» прошло бы проверку HOLD вакуумно, а рабочий добор был бы сломан).
  local want_rc=0
  [ "${want}" = "HOLD" ] && want_rc="${HOLD_RC}"
  if [ "${RC}" -ne "${want_rc}" ]; then
    nok "${name}" "exit=${RC}, ожидалось ${want_rc} при решении ${want}; вывод: $(printf '%s' "${OUT}" | head -2 | tr '\n' ' ')"
    return
  fi
  if [ "${got}" != "${want}" ]; then
    nok "${name}" "решение '${got}', ожидалось '${want}'"
    return
  fi
  if ! printf '%s' "${OUT}" | grep -qF -- "${want_reason}"; then
    nok "${name}" "решение ${want} верно, но ПРИЧИНА не содержит '${want_reason}': $(printf '%s' "${OUT}" | sed -n 's/^reason=//p')"
    return
  fi
  ok "${name}" "${want}"
}

# Сценарий decide, обязанный ОТКАЗАТЬ (негодный вход ⇒ решения нет вовсе).
expect_refusal() {
  local name="$1" target="$2" deployed="$3" runs="$4"
  run_decide "${target}" "${deployed}" "${runs}"
  if [ "${RC}" -eq 0 ]; then
    nok "${name}" "негодный вход принят (exit=0): $(printf '%s' "${OUT}" | head -1)"
    return
  fi
  if printf '%s' "${OUT}" | grep -q '^decision=DEPLOY'; then
    nok "${name}" "отказ, но при этом выдан DEPLOY — fail-closed нарушен"
    return
  fi
  ok "${name}" "отказ exit=${RC}"
}

# Сценарий проводки: ожидаемый код возврата check-wiring на подсунутом deploy.yml.
expect_wiring() {
  local name="$1" want_rc="$2" yml="$3" guard="${4:-}"
  [ -f "${yml}" ] || { setup_fail "${name}" "фикстура yml не создана: ${yml}"; return; }
  # setup-guard: мутация обязана ОТЛИЧАТЬСЯ от эталона, иначе сценарий тестирует не то,
  # что заявляет, и молча зеленеет (плацебо самого себя).
  if [ -n "${guard}" ] && cmp -s "${DEPLOY_YML}" "${yml}"; then
    setup_fail "${name}" "мутация не состоялась — файл совпадает с эталоном (${guard})"
    return
  fi
  local out rc
  out="$(CATCHUP_DEPLOY_YML="${yml}" python3 "${SUT}" check-wiring 2>&1)"; rc=$?
  if [ "${rc}" -ne "${want_rc}" ]; then
    nok "${name}" "exit=${rc}, ожидалось ${want_rc}; вывод: $(printf '%s' "${out}" | head -2 | tr '\n' ' ')"
    return
  fi
  ok "${name}" "exit=${rc}"
}

# --- фикстура-репозиторий ---------------------------------------------------------------
# Каждый коммит трогает РОВНО один класс путей, чтобы дельта была адресной. Классы взяты
# из реального фильтра `deploy.yml`, включая тот, которого в нём НЕТ вовсе (`scripts/**`,
# TD-150 п.2) — именно он был причиной эпизода 13-14.08.

FIXREPO="${WORK}/repo"
build_fixture_repo() {
  mkdir -p "${FIXREPO}" && register "${FIXREPO}"
  git -C "${FIXREPO}" init -q -b main 2>/dev/null || return 1
  git -C "${FIXREPO}" config user.email fixture@local
  git -C "${FIXREPO}" config user.name fixture
  mkdir -p "${FIXREPO}/crates/foo/src" "${FIXREPO}/crates/foo/tests" \
           "${FIXREPO}/scripts/tests" "${FIXREPO}/docs"

  echo base > "${FIXREPO}/README.md"
  git -C "${FIXREPO}" add README.md && git -C "${FIXREPO}" commit -qm base
  SHA_BASE="$(git -C "${FIXREPO}" rev-parse HEAD)"

  echo doc > "${FIXREPO}/docs/x.md"
  git -C "${FIXREPO}" add docs/x.md && git -C "${FIXREPO}" commit -qm docs
  SHA_DOCS="$(git -C "${FIXREPO}" rev-parse HEAD)"

  echo 'fn a() {}' > "${FIXREPO}/crates/foo/src/lib.rs"
  git -C "${FIXREPO}" add crates/foo/src/lib.rs && git -C "${FIXREPO}" commit -qm code
  SHA_CODE="$(git -C "${FIXREPO}" rev-parse HEAD)"

  echo '#[test] fn t() {}' > "${FIXREPO}/crates/foo/tests/t.rs"
  git -C "${FIXREPO}" add crates/foo/tests/t.rs && git -C "${FIXREPO}" commit -qm tests
  SHA_TESTS="$(git -C "${FIXREPO}" rev-parse HEAD)"

  echo 'echo probe' > "${FIXREPO}/scripts/tests/red_x.sh"
  git -C "${FIXREPO}" add scripts/tests/red_x.sh && git -C "${FIXREPO}" commit -qm harness
  SHA_HARNESS="$(git -C "${FIXREPO}" rev-parse HEAD)"

  echo 'fn b() {}' >> "${FIXREPO}/crates/foo/src/lib.rs"
  echo '#[test] fn u() {}' >> "${FIXREPO}/crates/foo/tests/t.rs"
  git -C "${FIXREPO}" add crates/foo/src/lib.rs crates/foo/tests/t.rs
  git -C "${FIXREPO}" commit -qm mixed
  SHA_MIXED="$(git -C "${FIXREPO}" rev-parse HEAD)"
  return 0
}

# --- фикстуры ранов ---------------------------------------------------------------------

runs_file() { printf '%s' "$2" > "${WORK}/$1.json"; echo "${WORK}/$1.json"; }

mk_run() {  # id, run_status, run_conclusion, head, vps_job_conclusion|ABSENT|NOJOBS
  local id="$1" st="$2" cn="$3" head="$4" vps="$5" jobs
  case "${vps}" in
    NOJOBS)  jobs='"jobs": "не список"' ;;
    ABSENT)  jobs='"jobs": [{"name":"Gate on CI (fail-closed)","status":"completed","conclusion":"failure"}]' ;;
    *)       jobs="\"jobs\": [{\"name\":\"Gate on CI (fail-closed)\",\"status\":\"completed\",\"conclusion\":\"failure\"},{\"name\":\"Deploy (build on VPS)\",\"status\":\"completed\",\"conclusion\":\"${vps}\"}]" ;;
  esac
  printf '{"databaseId": %s, "status": "%s", "conclusion": %s, "headSha": "%s", %s}' \
         "${id}" "${st}" "${cn}" "${head}" "${jobs}"
}

# ========================================================================================
# ЧАСТЬ 1 — РЕШЕНИЕ (decide)
# ========================================================================================

if ! build_fixture_repo; then
  echo "SETUP-FAIL: фикстурный репозиторий не собран — сценарии decide не запускались"
  exit 1
fi

EMPTY="$(runs_file empty '[]')"

echo "--- decide: класс «добирать» --------------------------------------------------"
# D1 — позитивный контроль: кодовая дельта есть, ранов нет вовсе. Это и есть эпизод,
#      когда деплой не стартовал НИКОГДА (фильтр путей не совпал).
expect_decision D1-нет-ранов-код-дельта DEPLOY "не стартовал" "${SHA_CODE}" "${SHA_BASE}" "${EMPTY}"
# D6 — смешанный коммит: `src` + `tests`. Семантика GitHub «последний совпавший паттерн
#      побеждает» обязана дать ДЕПЛОЙ (на это опирается комментарий deploy.yml:38-41).
expect_decision D6-смешанный-src-и-tests DEPLOY "не стартовал" "${SHA_MIXED}" "${SHA_TESTS}" "${EMPTY}"
# D7 — РЕАЛЬНЫЙ наблюдённый случай 61f452e: гейт CI упал, VPS-джоб skipped.
R_SKIPPED="$(runs_file skipped "[$(mk_run 111 completed '"failure"' "${SHA_CODE}" skipped)]")"
expect_decision D7-гейт-упал-VPS-skipped DEPLOY "не стартовал" "${SHA_CODE}" "${SHA_BASE}" "${R_SKIPPED}"

echo "--- decide: класс «добирать нечего» -------------------------------------------"
expect_decision D2-уже-на-вершине SKIP "уже на целевой вершине" "${SHA_CODE}" "${SHA_CODE}" "${EMPTY}"
expect_decision D3-только-docs SKIP "нет кодовой дельты" "${SHA_DOCS}" "${SHA_BASE}" "${EMPTY}"
# D4 — TD-086: push ОРАКУЛОВ не смеет передеплоивать прод (каждый редеплой = гэп записи).
expect_decision D4-только-crates-tests SKIP "нет кодовой дельты" "${SHA_TESTS}" "${SHA_CODE}" "${EMPTY}"
# D5 — TD-150 п.2: `scripts/**` в `paths` не входит ВООБЩЕ (а не «исключён»).
expect_decision D5-только-scripts-tests SKIP "нет кодовой дельты" "${SHA_HARNESS}" "${SHA_TESTS}" "${EMPTY}"
R_INFLIGHT="$(runs_file inflight "[$(mk_run 222 in_progress null "${SHA_CODE}" skipped)]")"
expect_decision D10-ран-в-полёте SKIP "в полёте" "${SHA_CODE}" "${SHA_BASE}" "${R_INFLIGHT}"

echo "--- decide: класс «человеку» (HOLD) — сердце Р-4 ------------------------------"
# D8 — ГЛАВНЫЙ различитель: VPS трогали и он упал. Авто-ретрай запрещён — молотил бы прод
#      rollback-циклами, а `fa/ops.md` §5.1 запрещает слепой откат за schema-forward деплоем.
R_VPSFAIL="$(runs_file vpsfail "[$(mk_run 333 completed '"failure"' "${SHA_CODE}" failure)]")"
expect_decision D8-VPS-упал HOLD "УПАЛ на VPS" "${SHA_CODE}" "${SHA_BASE}" "${R_VPSFAIL}"
# D9 — джоба нет в списке (переименование/обрезанный ответ): классифицировать нечем ⇒ человек.
R_ABSENT="$(runs_file absent "[$(mk_run 444 completed '"failure"' "${SHA_CODE}" ABSENT)]")"
expect_decision D9-VPS-джоба-нет HOLD "нет джоба" "${SHA_CODE}" "${SHA_BASE}" "${R_ABSENT}"
# D11 — деплой успешен для этой вершины, а на VPS её нет: состояние менялось мимо пайплайна.
R_SUCCESS="$(runs_file success "[$(mk_run 555 completed '"success"' "${SHA_CODE}" success)]")"
expect_decision D11-успех-но-VPS-не-там HOLD "мимо пайплайна" "${SHA_CODE}" "${SHA_BASE}" "${R_SUCCESS}"
# D12 — МНОЖЕСТВЕННОСТЬ (testing.md, дегенерированный вход п.2): два рана одной вершины,
#       и «безопасный» стоит ПЕРВЫМ. Наивная реализация смотрит только на первый.
R_TWO="$(runs_file two "[$(mk_run 666 completed '"failure"' "${SHA_CODE}" skipped),$(mk_run 777 completed '"failure"' "${SHA_CODE}" failure)]")"
expect_decision D12-два-рана-второй-упал HOLD "УПАЛ на VPS" "${SHA_CODE}" "${SHA_BASE}" "${R_TWO}"
# D13 — отмена посреди `docker compose up` оставляет VPS в промежуточном состоянии.
R_CANCEL="$(runs_file cancel "[$(mk_run 888 completed '"cancelled"' "${SHA_CODE}" cancelled)]")"
expect_decision D13-VPS-отменён HOLD "не доказано" "${SHA_CODE}" "${SHA_BASE}" "${R_CANCEL}"
R_NOJOBS="$(runs_file nojobs "[$(mk_run 999 completed '"failure"' "${SHA_CODE}" NOJOBS)]")"
expect_decision D14-список-джобов-негоден HOLD "классифицировать" "${SHA_CODE}" "${SHA_BASE}" "${R_NOJOBS}"

echo "--- decide: негодный вход ⇒ решения нет вовсе (fail-closed) -------------------"
R_ALIEN="$(runs_file alien "[$(mk_run 1010 completed '"failure"' "${SHA_BASE}" skipped)]")"
expect_refusal D15-ран-чужой-вершины "${SHA_CODE}" "${SHA_BASE}" "${R_ALIEN}"
# D16 — сокращённый SHA берётся от РЕАЛЬНОГО коммита фикстуры, а не выдуманный. Выдуманный
# отвергался бы позже и по другой причине («ревизии нет в истории»), и сценарий пиннил бы
# не то, что заявляет: требование ПОЛНОГО 40-hex — это и есть SHA-якорность (TD-150 п.1),
# сокращённая форма неоднозначна и разрешается по-разному в разных клонах.
expect_refusal D16-сокращённый-target-sha "${SHA_CODE:0:7}" "${SHA_BASE}" "${EMPTY}"
expect_refusal D17-target-не-hex "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz" "${SHA_BASE}" "${EMPTY}"
expect_refusal D18-deployed-нет-в-истории "${SHA_CODE}" "0000000000000000000000000000000000000000" "${EMPTY}"
expect_refusal D19-файла-ранов-нет "${SHA_CODE}" "${SHA_BASE}" "${WORK}/несуществующий.json"
BADJSON="$(runs_file badjson '{"не":"список"}')"
expect_refusal D20-раны-не-список "${SHA_CODE}" "${SHA_BASE}" "${BADJSON}"
TORN="$(runs_file torn '[{"databaseId": 1,')"
expect_refusal D21-раны-битый-json "${SHA_CODE}" "${SHA_BASE}" "${TORN}"

# D22 — деплой-манифест без `on.push.paths`: фильтр кода неопределим ⇒ отказ, а не
#       «дельта пуста» (иначе пропажа фильтра ТИХО глушила бы сторож навсегда).
NOPATHS="${WORK}/no-paths.yml"
python3 - "$DEPLOY_YML" "$NOPATHS" <<'PY'
import sys, yaml
wf = yaml.safe_load(open(sys.argv[1], encoding="utf-8"))
key = "on" if "on" in wf else True
wf[key]["push"].pop("paths", None)
yaml.safe_dump(wf, open(sys.argv[2], "w", encoding="utf-8"), allow_unicode=True)
PY
if [ -f "${NOPATHS}" ]; then
  run_decide "${SHA_CODE}" "${SHA_BASE}" "${EMPTY}" "${NOPATHS}"
  if [ "${RC}" -eq 0 ]; then nok D22-манифест-без-paths "принят (exit=0)"; else ok D22-манифест-без-paths "отказ exit=${RC}"; fi
else
  setup_fail D22-манифест-без-paths "фикстура манифеста не создана"
fi

# ========================================================================================
# ЧАСТЬ 2 — ПРОВОДКА (check-wiring): обманные deploy.yml
# ========================================================================================

echo "--- wiring: позитивный контроль и обманные манифесты --------------------------"
expect_wiring W0-эталон-проводки 0 "${DEPLOY_YML}"

# Мутатор манифеста: правка через YAML, а не sed — иначе «мутация» может не состояться
# незаметно (setup-guard в expect_wiring это ловит, но чинить дешевле сразу).
mutate_yml() {
  local name="$1" code="$2" out="${WORK}/wf-$1.yml"
  python3 - "$DEPLOY_YML" "$out" "$code" <<'PY' 2>/dev/null
import sys, yaml
wf = yaml.safe_load(open(sys.argv[1], encoding="utf-8"))
KEY = "on" if "on" in wf else True
on, jobs = wf[KEY], wf["jobs"]
exec(sys.argv[3])
yaml.safe_dump(wf, open(sys.argv[2], "w", encoding="utf-8"), allow_unicode=True)
PY
  echo "${out}"
}

expect_wiring W1-нет-workflow_run        1 "$(mutate_yml w1  'on.pop("workflow_run")')" mut
expect_wiring W2-workflow_run-чужой-CI   1 "$(mutate_yml w2  'on["workflow_run"]["workflows"]=["Other"]')" mut
expect_wiring W3-workflow_run-чужая-ветка 1 "$(mutate_yml w3 'on["workflow_run"]["branches"]=["dev"]')" mut
expect_wiring W4-нет-джоба-catchup       1 "$(mutate_yml w4  'jobs.pop("catchup")')" mut
expect_wiring W5-catchup-без-success     1 "$(mutate_yml w5  'jobs["catchup"]["if"]="github.event_name == \x27workflow_run\x27"')" mut
expect_wiring W6-deploy-без-needs-catchup 1 "$(mutate_yml w6 'jobs["deploy"]["needs"]=["ci"]')" mut
expect_wiring W7-deploy-без-решения      1 "$(mutate_yml w7  'jobs["deploy"]["if"]="always() && needs.ci.result == \x27success\x27"')" mut
expect_wiring W8-deploy-без-CI-гейта     1 "$(mutate_yml w8  'jobs["deploy"]["if"]="always() && needs.catchup.outputs.decision == \x27DEPLOY\x27"')" mut
expect_wiring W9-cancel-in-progress-true 1 "$(mutate_yml w9  'wf["concurrency"]["cancel-in-progress"]=True')" mut
expect_wiring W10-concurrency-группа     1 "$(mutate_yml w10 'wf["concurrency"]["group"]="deploy-other"')" mut
expect_wiring W11-права-расширены        1 "$(mutate_yml w11 'wf["permissions"]["contents"]="write"')" mut
expect_wiring W12-нет-actions-read       1 "$(mutate_yml w12 'wf["permissions"].pop("actions")')" mut
expect_wiring W13-rollback-снесён        1 "$(mutate_yml w13 'jobs["deploy"]["steps"][-1]["with"]["script"]=jobs["deploy"]["steps"][-1]["with"]["script"].replace(chr(39)+"git reset --hard -q \"$PREV\""+chr(39),"").replace("git reset --hard -q \"$PREV\"","true")')" mut
expect_wiring W14-CI-гейт-снесён         1 "$(mutate_yml w14 'jobs.pop("ci")')" mut
expect_wiring W15-CI-гейт-не-fail-closed 1 "$(mutate_yml w15 'jobs["ci"]["steps"][0]["run"]=jobs["ci"]["steps"][0]["run"].replace("exit 1","exit 0")')" mut
# W16 — TD-150 п.1: возврат к привязке по ВЕТКЕ. Самый вероятный регресс: строка короче
#       и «работает», а на прод уезжает не та вершина, чей CI проверялся.
expect_wiring W16-выкатка-по-ветке       1 "$(mutate_yml w16 'jobs["deploy"]["steps"][-1]["with"]["script"]=jobs["deploy"]["steps"][-1]["with"]["script"].replace("git reset --hard -q \"$TARGET_SHA\"","git reset --hard -q origin/main")')" mut


# ========================================================================================
# ЧАСТЬ 2bis — CI-АГРЕГАТ (`C-093` R-1): красный джоб обязан РОНЯТЬ «All checks passed»
# ========================================================================================
#
# Дыра, ради которой эта часть существует, замерена критиком: `ci.yml` не была в универсуме
# пробы ВООБЩЕ — она знала ровно предмет и `deploy.yml`. Поэтому два независимых стаба
# («джоб выкинут из условия агрегата» и «выкинут и из `needs`, и из условия») сохраняли
# полный зелёный прогон 39/39: для пробы стаб и честная проводка были НЕОТЛИЧИМЫ.
#
# Проверка идёт ПО ВЫЗОВУ: барьер извлекает условие агрегата, подставляет модель результатов
# и ИСПОЛНЯЕТ полученный bash. `grep` по имени джоба здесь бесполезен — он зелен и против
# закомментированной строки, и против упоминания в соседнем `echo`.

mutate_ci() {                       # имя, python-выражение над `wf`/`jobs`
  local tag="$1" expr="$2"
  local out="${WORK}/ci-${tag}.yml"
  CI_SRC="${CI_YML}" CI_OUT="${out}" MUT_EXPR="${expr}" python3 - <<'PY' || return 1
import os, sys, yaml
src, out, expr = os.environ["CI_SRC"], os.environ["CI_OUT"], os.environ["MUT_EXPR"]
wf = yaml.safe_load(open(src, encoding="utf-8"))
jobs = wf["jobs"]
exec(expr)
yaml.safe_dump(wf, open(out, "w", encoding="utf-8"), allow_unicode=True, sort_keys=False)
PY
  register "${out}"
  printf '%s' "${out}"
}

expect_aggregate() {                # имя, ожидаемый rc, путь к ci.yml, guard?
  local name="$1" want_rc="$2" yml="$3" guard="${4:-}"
  [ -f "${yml}" ] || { setup_fail "${name}" "фикстура ci.yml не создана: ${yml}"; return; }
  # setup-guard: мутация обязана ОТЛИЧАТЬСЯ от эталона. Проба, молча тестирующая не тот
  # сценарий, — плацебо самой себя (`testing.md`, целостность гейта, свойство 3).
  if [ -n "${guard}" ]; then
    local a b
    a="$(python3 -c 'import yaml,sys;print(yaml.safe_dump(yaml.safe_load(open(sys.argv[1],encoding="utf-8")),sort_keys=False,allow_unicode=True))' "${CI_YML}")"
    b="$(cat "${yml}")"
    if [ "${a}" = "${b}" ]; then
      setup_fail "${name}" "мутация не состоялась — ci.yml совпал с эталоном (${guard})"
      return
    fi
  fi
  local out rc
  out="$(CATCHUP_CI_YML="${yml}" CATCHUP_REPO_ROOT="${ROOT}" python3 "${SUT}" check-aggregate 2>&1)"; rc=$?
  if [ "${rc}" -ne "${want_rc}" ]; then
    nok "${name}" "exit=${rc}, ожидалось ${want_rc}; вывод: $(printf '%s' "${out}" | head -2 | tr '\n' ' ')"
    return
  fi
  ok "${name}" "exit=${rc}"
}

# A0 — ПОЗИТИВНЫЙ КОНТРОЛЬ. Без него вся группа могла бы быть вечно-красной, и её «объявили
#      бы шумом и выключили» (harness-track.md §5 п.2).
expect_aggregate A0-честный-ci-агрегат 0 "${CI_YML}"

# A1/A2 — ДВА СТАБА КРИТИКА, дословно из `C-093` R-1. Оба давали 39/39 PASS до этой части.
expect_aggregate A1-джоб-вне-условия 1 \
  "$(mutate_ci a1 'st=jobs["status-check"]["steps"][0]; st["run"]=st["run"].replace(" || \"${{ needs.deploy-catchup.result }}\" != \"success\"","")')" mut
expect_aggregate A2-джоб-вне-needs-и-условия 1 \
  "$(mutate_ci a2 'st=jobs["status-check"]["steps"][0]; st["run"]=st["run"].replace(" || \"${{ needs.deploy-catchup.result }}\" != \"success\"",""); jobs["status-check"]["needs"]=[n for n in jobs["status-check"]["needs"] if n!="deploy-catchup"]')" mut

# A3 — джоба нет вовсе: сторож не проводится в CI.
expect_aggregate A3-джоба-нет 1 \
  "$(mutate_ci a3 'jobs.pop("deploy-catchup"); jobs["status-check"]["needs"]=[n for n in jobs["status-check"]["needs"] if n!="deploy-catchup"]')" mut

# A4 — джоб-ПУСТЫШКА: имя на месте, в needs и в условии, но предмет не зовётся. Ровно тот
#      случай, который grep по имени пропускает, а проверка по вызову обязана поймать.
expect_aggregate A4-джоб-пустышка 1 \
  "$(mutate_ci a4 'jobs["deploy-catchup"]["steps"]=[{"run":"echo deploy_catchup.py ок"}]')" mut

# A5 — АНТИ-ПЛАЦЕБО С ДРУГОЙ СТОРОНЫ: условие «падать всегда» проходит A1-A4 вакуумно и
#      обязано падать здесь — иначе барьер не различает fail-closed от вечно-красного.
expect_aggregate A5-агрегат-падает-всегда 1 \
  "$(mutate_ci a5 'jobs["status-check"]["steps"][0]["run"]="echo forced; exit 1"')" mut

# ========================================================================================
# ЧАСТЬ 3 — МУТАЦИОННЫЙ КОНТРОЛЬ (--battery): равенство kill-set'ов
# ========================================================================================

battery() {
  echo
  echo "=== БАТАРЕЯ МУТАНТОВ: каждый обязан уронить РОВНО свой набор ==================="
  local bfail=0 mutants=0

  # Формат: имя | sed-выражение | ожидаемый kill-set (через запятую)
  #
  # M1 — ГЛАВНЫЙ СТАБ: «сторож молчит, когда деплоя не было». Одна строка — и механизм,
  #      наблюдающий сбой, перестаёт наблюдать ОТСУТСТВИЕ, оставаясь внешне живым:
  #      джоб зелёный, решения печатаются, добор не случается никогда.
  # M3 — заметен ТОЛЬКО по причине, не по решению: у предмета эшелонированная защита
  #      (снятие частного различителя даёт тот же HOLD через общий catch-all). Поэтому
  #      набор сверяет и ПРИЧИНУ — оператору важен класс, а не одно слово.
  local -a M=(
    "M1-сторож-молчит|s/^DEPLOY = \"DEPLOY\"/DEPLOY = \"SKIP\"/|D1-нет-ранов-код-дельта,D6-смешанный-src-и-tests,D7-гейт-упал-VPS-skipped"
    "M2-раны-не-смотрим|s/^def classify_runs(runs, target, job_name):/def classify_runs(runs, target, job_name):\\n    return None, \"\"/|D8-VPS-упал,D9-VPS-джоба-нет,D10-ран-в-полёте,D11-успех-но-VPS-не-там,D12-два-рана-второй-упал,D13-VPS-отменён,D14-список-джобов-негоден,D15-ран-чужой-вершины"
    "M3-упавший-VPS-не-особый|s/            if concl == \"failure\":/            if False:/|D8-VPS-упал,D12-два-рана-второй-упал"
    "M4-фильтр-путей-снят|s/^def path_matches(path, patterns):/def path_matches(path, patterns):\\n    return True/|D3-только-docs,D4-только-crates-tests,D5-только-scripts-tests"
    "M5-отрицание-не-разбирается|s/        negated = pattern.startswith(\"!\")/        negated = False/|D4-только-crates-tests"
    "M6-своя-выборка-не-сверяется|s/^        if head and head != target:/        if False:/|D15-ран-чужой-вершины"
    "M7-полёт-не-учитывается|s/^        if status and status != \"completed\":/        if False:/|D10-ран-в-полёте"
    "M8-равенство-вершин-не-проверяется|s/^    if target == deployed:/    if False:/|D2-уже-на-вершине"
    "M9-форма-SHA-не-проверяется|s/^    if not SHA_RE.match(raw):/    if False:/|D16-сокращённый-target-sha"
    "M10-джоб-по-имени-не-ищется|s/^        if not vps:/        if False:/|D9-VPS-джоба-нет"
    # M11 — СТАБ, ради которого написана эскалация (`C-093` R-2): решение верное, причина
    #       верная, добора нет — и всё же дефект. Сторож ЗОВЁТ человека нулевым кодом, то
    #       есть не зовёт: job зелёный, ран зелёный, HOLD неотличим от успешной доставки.
    #       Обязан уронить РОВНО шесть HOLD-сценариев и ни одного больше.
    "M11-HOLD-молчит-зелёным|s/^        return HOLD_RC if verdict == HOLD else 0/        return 0/|D8-VPS-упал,D9-VPS-джоба-нет,D11-успех-но-VPS-не-там,D12-два-рана-второй-упал,D13-VPS-отменён,D14-список-джобов-негоден"
  )

  local orig="${WORK}/sut-orig.py"
  cp "${SUT}" "${orig}" || { echo "SETUP-FAIL: предмет не скопирован"; return 1; }

  # Позитивный контроль батареи: против ЧЕСТНОГО предмета kill-set обязан быть ПУСТ.
  # Без него непустой kill-set мутанта не значит ничего — набор мог быть красным и так.
  local baseline; baseline="$(run_decide_suite)"
  if [ -n "$(printf '%s' "${baseline}" | tr -d ' ')" ]; then
    echo "SETUP-FAIL батарея: честный предмет уже роняет [${baseline}] — мутанты бессмысленны"
    return 1
  fi
  echo "ok   базовая линия                     честный предмет: kill-set пуст"

  for spec in "${M[@]}"; do
    local name="${spec%%|*}" rest="${spec#*|}"
    local expr="${rest%%|*}" want="${rest##*|}"
    mutants=$((mutants + 1))
    local mut="${WORK}/mut-${name}.py"
    sed "${expr}" "${orig}" > "${mut}"
    # setup-guard мутанта: sed обязан ИЗМЕНИТЬ файл. Не изменил — «мутант не убил ничего»
    # было бы ложным зелёным о несуществующей мутации.
    if cmp -s "${orig}" "${mut}"; then
      echo "SETUP-FAIL ${name}: мутация не состоялась (sed ничего не заменил)"
      bfail=$((bfail + 1)); continue
    fi
    if ! python3 -c "import ast,sys; ast.parse(open(sys.argv[1]).read())" "${mut}" 2>/dev/null; then
      echo "SETUP-FAIL ${name}: мутант не парсится питоном"
      bfail=$((bfail + 1)); continue
    fi

    # Прогоняем ВЕСЬ набор decide против мутанта и собираем фактический kill-set.
    local killed; SUT_ACTIVE="${mut}" ; killed="$(run_decide_suite)" ; unset SUT_ACTIVE
    local want_sorted got_sorted
    want_sorted="$(printf '%s' "${want}" | tr ',' '\n' | sort | tr '\n' ' ')"
    got_sorted="$(printf '%s' "${killed}" | tr ' ' '\n' | grep -v '^$' | sort | tr '\n' ' ')"
    if [ "${want_sorted}" = "${got_sorted}" ]; then
      printf 'ok   %-34s kill-set совпал: %s\n' "${name}" "${got_sorted}"
    else
      printf 'FAIL %-34s ожидалось [%s], получено [%s]\n' "${name}" "${want_sorted}" "${got_sorted}"
      bfail=$((bfail + 1))
    fi
  done

  echo "--- батарея: мутантов ${mutants}, расхождений kill-set ${bfail}"
  return "${bfail}"
}

# Прогон набора decide против предмета (или мутанта, через SUT_ACTIVE); печатает имена
# УПАВШИХ сценариев. Сверяются И решение, И причина: см. комментарий к M3 выше.
run_decide_suite() {
  local killed=""
  chk() {  # имя, ожидание(решение|REFUSE), подстрока-причины, target, deployed, runs
    local n="$1" want="$2" why="$3"
    run_decide "$4" "$5" "$6"
    local got; got="$(printf '%s\n' "${OUT}" | sed -n 's/^decision=//p' | head -1)"
    if [ "${want}" = "REFUSE" ]; then
      [ "${RC}" -ne 0 ] || killed="${killed} ${n}"
      return
    fi
    local want_rc=0
    [ "${want}" = "HOLD" ] && want_rc="${HOLD_RC}"
    if [ "${RC}" -ne "${want_rc}" ] || [ "${got}" != "${want}" ]; then
      killed="${killed} ${n}"; return
    fi
    printf '%s' "${OUT}" | grep -qF -- "${why}" || killed="${killed} ${n}"
  }
  chk D1-нет-ранов-код-дельта   DEPLOY "не стартовал"      "${SHA_CODE}"    "${SHA_BASE}"  "${EMPTY}"
  chk D6-смешанный-src-и-tests  DEPLOY "не стартовал"      "${SHA_MIXED}"   "${SHA_TESTS}" "${EMPTY}"
  chk D7-гейт-упал-VPS-skipped  DEPLOY "не стартовал"      "${SHA_CODE}"    "${SHA_BASE}"  "${R_SKIPPED}"
  chk D2-уже-на-вершине         SKIP   "уже на целевой"    "${SHA_CODE}"    "${SHA_CODE}"  "${EMPTY}"
  chk D3-только-docs            SKIP   "нет кодовой"       "${SHA_DOCS}"    "${SHA_BASE}"  "${EMPTY}"
  chk D4-только-crates-tests    SKIP   "нет кодовой"       "${SHA_TESTS}"   "${SHA_CODE}"  "${EMPTY}"
  chk D5-только-scripts-tests   SKIP   "нет кодовой"       "${SHA_HARNESS}" "${SHA_TESTS}" "${EMPTY}"
  chk D10-ран-в-полёте          SKIP   "в полёте"          "${SHA_CODE}"    "${SHA_BASE}"  "${R_INFLIGHT}"
  chk D8-VPS-упал               HOLD   "УПАЛ на VPS"       "${SHA_CODE}"    "${SHA_BASE}"  "${R_VPSFAIL}"
  chk D9-VPS-джоба-нет          HOLD   "нет джоба"         "${SHA_CODE}"    "${SHA_BASE}"  "${R_ABSENT}"
  chk D11-успех-но-VPS-не-там   HOLD   "мимо пайплайна"    "${SHA_CODE}"    "${SHA_BASE}"  "${R_SUCCESS}"
  chk D12-два-рана-второй-упал  HOLD   "УПАЛ на VPS"       "${SHA_CODE}"    "${SHA_BASE}"  "${R_TWO}"
  chk D13-VPS-отменён           HOLD   "не доказано"       "${SHA_CODE}"    "${SHA_BASE}"  "${R_CANCEL}"
  chk D14-список-джобов-негоден HOLD   "классифицировать"  "${SHA_CODE}"    "${SHA_BASE}"  "${R_NOJOBS}"
  chk D15-ран-чужой-вершины     REFUSE ""                  "${SHA_CODE}"    "${SHA_BASE}"  "${R_ALIEN}"
  chk D16-сокращённый-target-sha REFUSE ""                 "${SHA_CODE:0:7}" "${SHA_BASE}" "${EMPTY}"
  printf '%s' "${killed}"
}

BATTERY_RC=0
if [ "${1:-}" = "--battery" ]; then
  battery; BATTERY_RC=$?
fi

# ========================================================================================

TMP_AFTER="$(own_dirs)"
echo
echo "=============================================================================="
echo "сценариев: $((PASS + FAIL))   PASS: ${PASS}   FAIL: ${FAIL}"
# Уборка — не примечание, а условие merge'"'"'а (harness-track.md §5 п.5): класс, давший
# 10 400 каталогов в /tmp и диск на 100 %. Расхождение РОНЯЕТ прогон, а не печатается.
echo "своих каталогов (red-catchup-*): до ${TMP_BEFORE}, после ${TMP_AFTER} (уборка — trap EXIT + реестр)"
if [ "${TMP_AFTER}" -gt "${TMP_BEFORE}" ]; then
  echo "УТЕЧКА ФИКСТУР: осталось $((TMP_AFTER - TMP_BEFORE)) своих каталогов — реестр/trap не отработали"
  echo "VERDICT: FAIL"
  exit 1
fi
if [ "${FAIL}" -ne 0 ]; then
  printf 'упали: %s\n' "${FAILED_NAMES[*]}"
  echo "VERDICT: FAIL"
  exit 1
fi
if [ "${BATTERY_RC}" -ne 0 ]; then
  echo "VERDICT: FAIL (батарея: ${BATTERY_RC} расхождений kill-set)"
  exit 1
fi
echo "VERDICT: PASS"
exit 0
