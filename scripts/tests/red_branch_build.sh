#!/usr/bin/env bash
# Проба барьера «ветка собирается» — предмет: `scripts/check_branch_build.sh`.
#
# Три обязательных свойства харнесс-трека (`docs/workflow/harness-track.md` §5):
#
#   1. ПОЗИТИВНЫЙ КОНТРОЛЬ — честная конфигурация зелена целиком. Без него проба может быть
#      вечно-красной, и её «объявят шумом и выключат».
#   2. АНТИ-ПЛАЦЕБО СТАБАМИ — каждый обманный вариант обязан краснеть. Стабы построены так,
#      чтобы НАИВНАЯ проверка (греп по `branches-ignore` + греп по имени) их пропускала:
#      именно этот объём проверок назначал план §A, и замер показал, что его мало.
#   3. МУТАЦИОННЫЙ КОНТРОЛЬ (`--battery`) — каждый мутант роняет РОВНО заявленное множество
#      сценариев: ни больше (сломан сверх своей дыры), ни меньше (дыра не закреплена).
#      Равенство kill-set'ов, а не «хоть что-то упало».
#
# ГЕРМЕТИЧНОСТЬ. Проба строит фикстуры в своём каталоге под TMPDIR и в сеть не ходит вовсе.
# Ходила бы — мерила бы доступность GitHub, а не свой инвариант (класс `TD-135`), и краснела
# бы на чужих сбоях. Git здесь не нужен: барьер судит ТЕКСТ трёх YAML-файлов.
#
# ЧЕСТНО НАЗВАННЫЙ ПРЕДЕЛ БАТАРЕИ. Мутанты строятся для B2…B9. У B1 (существование и разбор
# предмета) СОБСТВЕННОГО сценария нет и быть не может: отсутствие предмета неизбежно ловится
# ниже по течению (B2 не найдёт `on:`, B3 не найдёт джоб). Роль B1 — не исход, а ПРИЧИНА:
# без него барьер падает трассировкой вместо строки «сборка ветки не существует как
# механизм». Это проверяется сценарием S1 отдельно и не изображается kill-set'ом.
#
# Прогон: bash scripts/tests/red_branch_build.sh [--battery]

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUT="${ROOT}/scripts/check_branch_build.sh"
SUT_ACTIVE="${SUT}"

PASS=0
FAIL=0
FAILED_NAMES=()

# --- уборка фикстур: РЕЕСТР В ФАЙЛЕ + trap EXIT (harness-track.md §5 п.5) ---------------
# Класс, давший 10 400 каталогов в /tmp и диск на 100 %: фикстуры, о которых знал только
# живой процесс. Реестр переживает падение процесса, поэтому уборка — файловая.
own_dirs() { find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'red-branchbuild-*' 2>/dev/null | wc -l; }
# База снимается ДО создания своего каталога: иначе обе величины содержат WORK и разность
# нулевая при ЛЮБОМ поведении cleanup() — страж был бы вакуумен (класс `C-096` B-4).
# Считаются СВОИ каталоги по префиксу, а не всё содержимое TMPDIR: иначе проба мерила бы
# окружение, а не себя (`testing.md`, целостность гейта, свойство 2).
TMP_BEFORE="$(own_dirs)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/red-branchbuild-XXXXXX")"
REGISTRY="${WORK}/.fixtures"
: > "${REGISTRY}"
register() { printf '%s\n' "$1" >> "${REGISTRY}"; }
cleanup() {
  if [ -f "${REGISTRY}" ]; then
    while IFS= read -r p; do [ -n "${p}" ] && [ -e "${p}" ] && rm -rf "${p}"; done < "${REGISTRY}"
  fi
  rm -rf "${WORK}"
}
trap cleanup EXIT
register "${WORK}"

# --- инфраструктура сценариев -----------------------------------------------------------
ok()         { PASS=$((PASS + 1)); printf 'ok         %-26s %s\n' "$1" "${2:-}"; }
nok()        { FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'FAIL       %-26s %s\n' "$1" "$2"; }
setup_fail() { FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'SETUP-FAIL %-26s %s\n' "$1" "$2"; }

CASE_N=0
# mk_case <имя> → печатает путь корня фикстуры. Копирует ТРИ файла, которые судит барьер.
mk_case() {
  CASE_N=$((CASE_N + 1))
  local d="${WORK}/case-${CASE_N}-$1"
  mkdir -p "${d}/.github/workflows" || return 1
  register "${d}"
  cp "${ROOT}/.github/workflows/branch-build.yml" "${d}/.github/workflows/" 2>/dev/null || return 1
  cp "${ROOT}/.github/workflows/ci.yml"           "${d}/.github/workflows/" 2>/dev/null || return 1
  cp "${ROOT}/.github/workflows/deploy.yml"       "${d}/.github/workflows/" 2>/dev/null || return 1
  printf '%s' "${d}"
}

# mutate <файл> <старое> <новое> — ТЕКСТОВАЯ замена с ОБЯЗАТЕЛЬНОЙ проверкой попадания.
# Setup-guard (`testing.md`, свойство 3): не найденный литерал = сценарий, молча тестирующий
# не тот случай, то есть плацебо самого себя. Такой сценарий обязан упасть как SETUP-FAIL.
mutate() {
  python3 - "$1" "$2" "$3" <<'PY'
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(path, encoding="utf-8").read()
if old not in s:
    sys.exit(3)
open(path, "w", encoding="utf-8").write(s.replace(old, new, 1))
PY
}

# run_barrier <корень-фикстуры> → OUT/RC. Прод-форма: барьер зовётся БЕЗ аргументов,
# корень приходит переменной — той же ручкой, что описана в его шапке.
run_barrier() {
  OUT="$(BRANCH_BUILD_ROOT="$1" bash "${SUT_ACTIVE}" 2>&1)"
  RC=$?
}

# expect_red <имя> <корень> <код-проверки> [фрагмент-причины]
#
# Барьер обязан покраснеть, покраснеть ПО СВОЕЙ ПРОВЕРКЕ и — там, где причина различает
# случаи, — назвать ИМЕННО ЕЁ. Четвёртый аргумент введён по находке: `branches-ignore: ['**']`
# краснело сообщением «не исключает main», а группа `deploy-main` — сообщением «не привязана
# к github.ref». Цвет верный, диагноз ложный: читатель чинит не то, что сломано, а порядок
# проверок в барьере при этом молча неверен. Проба, пиннящая только КОД, этого не видит.
expect_red() {
  local name="$1" root="$2" code="$3" reason="${4:-}"
  run_barrier "${root}"
  if [ "${RC}" -eq 0 ]; then
    nok "${name}" "барьер вернул exit=0 — стаб ПРОПУЩЕН"
    return
  fi
  if ! grep -q "FAIL  ${code}" <<<"${OUT}"; then
    nok "${name}" "красное есть (exit=${RC}), но НЕ по ${code}: $(grep -m1 '^FAIL' <<<"${OUT}")"
    return
  fi
  if [ -n "${reason}" ] && ! grep -qF "${reason}" <<<"${OUT}"; then
    nok "${name}" "${code} сработал, но ПРИЧИНА не та (нет «${reason}»): $(grep -m1 "FAIL  ${code}" <<<"${OUT}" | cut -c1-110)"
    return
  fi
  ok "${name}" "${code} — $(grep -m1 "FAIL  ${code}" <<<"${OUT}" | cut -c1-90)"
}

expect_green() {
  local name="$1" root="$2"
  run_barrier "${root}"
  if [ "${RC}" -ne 0 ]; then
    nok "${name}" "честная конфигурация признана негодной (exit=${RC}): $(grep -m1 '^FAIL' <<<"${OUT}")"
    return
  fi
  ok "${name}" "exit=0"
}

BB=".github/workflows/branch-build.yml"
CI=".github/workflows/ci.yml"
DP=".github/workflows/deploy.yml"

# ═══ Сценарии ═══════════════════════════════════════════════════════════════════════════
scenarios() {

# --- позитивный контроль --------------------------------------------------------------
d="$(mk_case honest)" || { setup_fail "P0-честная" "фикстура не собралась"; return; }
expect_green "P0-честная" "${d}"

# YAML 1.1: `on` — БУЛЕВ ключ. Закавыченная форма `"on":` даёт строковый ключ. Барьер обязан
# понимать обе; иначе он слеп к нормально написанному workflow (ловушка из deploy_catchup.py).
d="$(mk_case quoted-on)" || { setup_fail "P1-on-закавычен" "фикстура"; return; }
mutate "${d}/${BB}" $'on:\n  push:' $'"on":\n  push:' || setup_fail "P1-on-закавычен" "литерал не найден"
expect_green "P1-on-закавычен" "${d}"

# --- B1: существование и разбор предмета ------------------------------------------------
d="$(mk_case no-file)" || { setup_fail "S1-файла-нет" "фикстура"; return; }
rm -f "${d}/${BB}"
expect_red "S1-файла-нет" "${d}" "B1"

d="$(mk_case broken-yaml)" || { setup_fail "S2-битый-yaml" "фикстура"; return; }
printf '\n  : : [ не-yaml\n' >> "${d}/${BB}"
expect_red "S2-битый-yaml" "${d}" "B1"

d="$(mk_case no-name)" || { setup_fail "S3-без-name" "фикстура"; return; }
mutate "${d}/${BB}" 'name: Branch build' '# name удалён' || setup_fail "S3-без-name" "литерал"
expect_red "S3-без-name" "${d}" "B1"

# --- B2: триггер ------------------------------------------------------------------------
# Файл есть, парсится, имя на месте — а сборка не рождается событием: нужна воля автора,
# ровно то, что механизм и устраняет.
d="$(mk_case no-push)" || { setup_fail "S4-без-push" "фикстура"; return; }
mutate "${d}/${BB}" $'on:\n  push:\n    branches-ignore: [main]' $'on:\n  workflow_dispatch:' \
  || setup_fail "S4-без-push" "литерал"
expect_red "S4-без-push" "${d}" "B2"

# Опечатка в имени ключа: GitHub её игнорирует ⇒ триггер на ВСЕ ветки, включая main.
d="$(mk_case typo-key)" || { setup_fail "S5-опечатка-ключа" "фикстура"; return; }
mutate "${d}/${BB}" 'branches-ignore: [main]' 'branches_ignore: [main]' || setup_fail "S5-опечатка-ключа" "литерал"
expect_red "S5-опечатка-ключа" "${d}" "B2" "опечатка в имени ключа"

# Ключ верный, значение глушит всё: механизм мёртв при зелёном грепе.
d="$(mk_case ignore-all)" || { setup_fail "S6-исключает-всё" "фикстура"; return; }
mutate "${d}/${BB}" 'branches-ignore: [main]' "branches-ignore: ['**']" || setup_fail "S6-исключает-всё" "литерал"
expect_red "S6-исключает-всё" "${d}" "B2" "исключает ВСЁ"

d="$(mk_case include-filter)" || { setup_fail "S7-фильтр-включения" "фикстура"; return; }
mutate "${d}/${BB}" 'branches-ignore: [main]' 'branches: [main]' || setup_fail "S7-фильтр-включения" "литерал"
expect_red "S7-фильтр-включения" "${d}" "B2"

d="$(mk_case both-keys)" || { setup_fail "S8-оба-ключа" "фикстура"; return; }
mutate "${d}/${BB}" 'branches-ignore: [main]' $'branches: [feat]\n    branches-ignore: [main]' \
  || setup_fail "S8-оба-ключа" "литерал"
expect_red "S8-оба-ключа" "${d}" "B2"

d="$(mk_case ignore-not-main)" || { setup_fail "S9-main-не-исключён" "фикстура"; return; }
mutate "${d}/${BB}" 'branches-ignore: [main]' 'branches-ignore: [gh-pages]' || setup_fail "S9-main-не-исключён" "литерал"
expect_red "S9-main-не-исключён" "${d}" "B2" "не исключает \`main\`"

# --- B3: паритет состава с ci.yml -------------------------------------------------------
# «Ветка собирается» обязано значить ТО ЖЕ, что «main собирается». Все стабы ниже наивная
# проверка триггера пропускает целиком.
d="$(mk_case drop-clippy)" || { setup_fail "S10-без-clippy" "фикстура"; return; }
mutate "${d}/${BB}" $'      - name: clippy (warnings = errors)\n        run: cargo clippy --all-targets --all-features -- -D warnings\n' '' \
  || setup_fail "S10-без-clippy" "литерал"
expect_red "S10-без-clippy" "${d}" "B3"

d="$(mk_case drop-all-features)" || { setup_fail "S11-без-all-features" "фикстура"; return; }
mutate "${d}/${BB}" 'cargo clippy --all-targets --all-features -- -D warnings' 'cargo clippy --all-targets -- -D warnings' \
  || setup_fail "S11-без-all-features" "литерал"
expect_red "S11-без-all-features" "${d}" "B3"

d="$(mk_case test-not-all)" || { setup_fail "S12-test-без-all" "фикстура"; return; }
mutate "${d}/${BB}" 'run: cargo test --all' 'run: cargo test' || setup_fail "S12-test-без-all" "литерал"
expect_red "S12-test-без-all" "${d}" "B3"

# Дрейф в ОБРАТНУЮ сторону: ci.yml расширили, branch-build отстал. Проверка «наличие», а не
# «расхождение», этого не увидит никогда.
d="$(mk_case ci-drifted)" || { setup_fail "S13-дрейф-ci" "фикстура"; return; }
mutate "${d}/${CI}" $'      - name: test (RED/GREEN — наши инварианты)\n        run: cargo test --all\n' \
                    $'      - name: test (RED/GREEN — наши инварианты)\n        run: cargo test --all\n      - name: doc\n        run: cargo doc --no-deps\n' \
  || setup_fail "S13-дрейф-ci" "литерал"
expect_red "S13-дрейф-ci" "${d}" "B3"

# Без components цепочка встала бы на fmt/clippy по ЧУЖОЙ причине — «ветка не собирается»
# означало бы «инструмент не установлен».
d="$(mk_case no-components)" || { setup_fail "S14-без-components" "фикстура"; return; }
mutate "${d}/${BB}" $'        with:\n          components: rustfmt, clippy\n' '' || setup_fail "S14-без-components" "литерал"
expect_red "S14-без-components" "${d}" "B3"

# Греп-обманка: имя команды на месте, ВЫЗОВА нет. Проверка по подстроке зелена.
d="$(mk_case echo-not-call)" || { setup_fail "S15-echo-вместо-вызова" "фикстура"; return; }
mutate "${d}/${BB}" 'run: cargo clippy --all-targets --all-features -- -D warnings' \
                    'run: echo "cargo clippy --all-targets --all-features -- -D warnings"' \
  || setup_fail "S15-echo-вместо-вызова" "литерал"
expect_red "S15-echo-вместо-вызова" "${d}" "B3"

# Множество то же, порядок иной: тесты до сборки-проверок — другой смысл прогона.
d="$(mk_case reordered)" || { setup_fail "S16-порядок-шагов" "фикстура"; return; }
mutate "${d}/${BB}" $'      - name: fmt\n        run: cargo fmt --all -- --check\n      - name: clippy (warnings = errors)\n        run: cargo clippy --all-targets --all-features -- -D warnings\n' \
                    $'      - name: clippy (warnings = errors)\n        run: cargo clippy --all-targets --all-features -- -D warnings\n      - name: fmt\n        run: cargo fmt --all -- --check\n' \
  || setup_fail "S16-порядок-шагов" "литерал"
expect_red "S16-порядок-шагов" "${d}" "B3"

# SETUP-guard самого барьера: эталона нет ⇒ барьер обязан краснеть, а не молчать «паритет ок».
# Проба, которая тут зелена, — плацебо самой себя.
d="$(mk_case no-ref-job)" || { setup_fail "S17-эталона-нет" "фикстура"; return; }
mutate "${d}/${CI}" $'jobs:\n  build-test:' $'jobs:\n  build-test-renamed:' || setup_fail "S17-эталона-нет" "литерал"
expect_red "S17-эталона-нет" "${d}" "B3"

# --- B4: обезврежен на уровне джоба -----------------------------------------------------
# Паритет шагов этих двух стабов НЕ видит: ключи живут на джобе, а не в шагах.
d="$(mk_case job-if-false)" || { setup_fail "S18-if-false" "фикстура"; return; }
mutate "${d}/${BB}" $'  build-test:\n    name: fmt + clippy + test (ветка)' \
                    $'  build-test:\n    if: false\n    name: fmt + clippy + test (ветка)' \
  || setup_fail "S18-if-false" "литерал"
expect_red "S18-if-false" "${d}" "B4"

d="$(mk_case job-continue-on-error)" || { setup_fail "S19-continue-on-error" "фикстура"; return; }
mutate "${d}/${BB}" $'  build-test:\n    name: fmt + clippy + test (ветка)' \
                    $'  build-test:\n    continue-on-error: true\n    name: fmt + clippy + test (ветка)' \
  || setup_fail "S19-continue-on-error" "литерал"
expect_red "S19-continue-on-error" "${d}" "B4"

# --- B5: отклонённая правка ci.yml не просочилась ---------------------------------------
# Ровно то предписание, что отклонено исполнением: пять джобов ci.yml становятся красными
# на первом push'е ветки и на каждом force-push'е.
d="$(mk_case ci-glob)" || { setup_fail "S20-ci-glob" "фикстура"; return; }
mutate "${d}/${CI}" $'  push:\n    branches: [main]' $'  push:\n    branches: [\'**\']' || setup_fail "S20-ci-glob" "литерал"
expect_red "S20-ci-glob" "${d}" "B5"

d="$(mk_case ci-ignore)" || { setup_fail "S21-ci-ignore" "фикстура"; return; }
mutate "${d}/${CI}" $'  push:\n    branches: [main]' $'  push:\n    branches: [main]\n    branches-ignore: [gh-pages]' \
  || setup_fail "S21-ci-ignore" "литерал"
expect_red "S21-ci-ignore" "${d}" "B5"

# --- B6: прод не шелохнётся -------------------------------------------------------------
# Зелёная сборка ЛЮБОЙ ветки начала бы дёргать сторожа добора и выкатывать прод.
d="$(mk_case deploy-listens)" || { setup_fail "S22-deploy-слушает" "фикстура"; return; }
mutate "${d}/${DP}" 'workflows: ["CI"]' 'workflows: ["CI", "Branch build"]' || setup_fail "S22-deploy-слушает" "литерал"
expect_red "S22-deploy-слушает" "${d}" "B6"

d="$(mk_case deploy-branches)" || { setup_fail "S23-deploy-ветки" "фикстура"; return; }
mutate "${d}/${DP}" $'  push:\n    branches: [main]' $'  push:\n    branches: [main, feat/*]' || setup_fail "S23-deploy-ветки" "литерал"
expect_red "S23-deploy-ветки" "${d}" "B6"

# Фильтр путей деплоя накрывает НАШ файл: каждый коммит сборщика ветки стал бы редеплоем,
# то есть рестартом recorder'а и ГЭПОМ в forward-only записи (класс TD-086).
d="$(mk_case deploy-paths)" || { setup_fail "S24-deploy-paths" "фикстура"; return; }
mutate "${d}/${DP}" "      - '.github/workflows/deploy.yml'" "      - '.github/workflows/**'" \
  || setup_fail "S24-deploy-paths" "литерал"
expect_red "S24-deploy-paths" "${d}" "B6" "накрывает"

# --- B4, ШАГОВЫЙ уровень: то, что первая редакция барьера ложно объявила закрытым ---------
# Оба ключа НЕ меняют строку `run:`, поэтому паритет B3 их не видит. Проверено замером:
# до этой правки барьер давал PASS exit=0 на обоих.
d="$(mk_case step-coe)" || { setup_fail "S25-шаг-coe" "фикстура"; return; }
mutate "${d}/${BB}" $'      - name: test (RED/GREEN — наши инварианты)\n        run: cargo test --all' \
                    $'      - name: test (RED/GREEN — наши инварианты)\n        continue-on-error: true\n        run: cargo test --all' \
  || setup_fail "S25-шаг-coe" "литерал"
expect_red "S25-шаг-coe" "${d}" "B4" "continue-on-error: true\` — его падение"

d="$(mk_case step-if-false)" || { setup_fail "S26-шаг-if-false" "фикстура"; return; }
mutate "${d}/${BB}" $'      - name: test (RED/GREEN — наши инварианты)\n        run: cargo test --all' \
                    $'      - name: test (RED/GREEN — наши инварианты)\n        if: false\n        run: cargo test --all' \
  || setup_fail "S26-шаг-if-false" "литерал"
expect_red "S26-шаг-if-false" "${d}" "B4" "не исполняется никогда"

# Контроль в другую сторону: глушение кода возврата в САМОЙ команде паритет ЛОВИТ. Сценарий
# оставлен как регресс-якорь — он закрепляет, что B3 действительно покрывает эту форму, и
# отдельная проверка под неё была бы мёртвым весом.
d="$(mk_case swallow-rc)" || { setup_fail "S27-глушение-кода" "фикстура"; return; }
mutate "${d}/${BB}" 'run: cargo test --all' 'run: cargo test --all || true' || setup_fail "S27-глушение-кода" "литерал"
expect_red "S27-глушение-кода" "${d}" "B3"

# --- B2, коллизия ключа `on` (YAML 1.1) --------------------------------------------------
# Голый `on` PyYAML разворачивает в БУЛЕВ ключ True, закавыченный — в строку. При обоих
# барьер, берущий первый попавшийся, молча читает НЕ ТОТ блок: цвет может быть верным, а
# диагноз ложным, а при обратном порядке ключей — ложное ЗЕЛЁНОЕ.
d="$(mk_case on-collision)" || { setup_fail "S28-два-ключа-on" "фикстура"; return; }
mutate "${d}/${BB}" $'on:\n  push:' $'"on":\n  workflow_dispatch:\non:\n  push:' || setup_fail "S28-два-ключа-on" "литерал"
expect_red "S28-два-ключа-on" "${d}" "B2" "задан ДВАЖДЫ"

# --- B7: барьер наблюдает СОБСТВЕННУЮ проводку -------------------------------------------
# Четыре формы отключения; сегодня их не ловил никто, а `verify_design_claims.sh` на дереве
# с обоими файлами давал PASS — ложное утверждение шапки о коде проезжало.
d="$(mk_case wiring-no-job)" || { setup_fail "S29-джоба-нет" "фикстура"; return; }
mutate "${d}/${CI}" $'  branch-build-parity:\n' $'  branch-build-parity-renamed:\n' || setup_fail "S29-джоба-нет" "литерал"
expect_red "S29-джоба-нет" "${d}" "B7"

d="$(mk_case wiring-not-in-needs)" || { setup_fail "S30-не-в-needs" "фикстура"; return; }
mutate "${d}/${CI}" 'deploy-catchup, branch-build-parity]' 'deploy-catchup]' || setup_fail "S30-не-в-needs" "литерал"
expect_red "S30-не-в-needs" "${d}" "B7" "отсутствует в"

d="$(mk_case wiring-not-in-guard)" || { setup_fail "S31-не-в-условии" "фикстура"; return; }
mutate "${d}/${CI}" ' || "${{ needs.branch-build-parity.result }}" != "success"' '' || setup_fail "S31-не-в-условии" "литерал"
expect_red "S31-не-в-условии" "${d}" "B7" "не участвует в fail-closed"

d="$(mk_case wiring-echo)" || { setup_fail "S32-echo-не-вызов" "фикстура"; return; }
mutate "${d}/${CI}" 'run: bash scripts/check_branch_build.sh' 'run: echo "bash scripts/check_branch_build.sh"' \
  || setup_fail "S32-echo-не-вызов" "литерал"
expect_red "S32-echo-не-вызов" "${d}" "B7" "не ЗОВЁТ"

# --- B8: concurrency ---------------------------------------------------------------------
d="$(mk_case no-concurrency)" || { setup_fail "S33-без-concurrency" "фикстура"; return; }
mutate "${d}/${BB}" $'concurrency:\n  group: branch-build-${{ github.ref }}\n  cancel-in-progress: true\n' '' \
  || setup_fail "S33-без-concurrency" "литерал"
expect_red "S33-без-concurrency" "${d}" "B8"

d="$(mk_case group-not-ref)" || { setup_fail "S34-группа-не-по-ref" "фикстура"; return; }
mutate "${d}/${BB}" 'group: branch-build-${{ github.ref }}' 'group: branch-build' || setup_fail "S34-группа-не-по-ref" "литерал"
expect_red "S34-группа-не-по-ref" "${d}" "B8" "не привязана к"

# Самый дорогой из стабов: та же группа, что у деплоя, при `cancel-in-progress: true` ОТМЕНИТ
# идущую выкатку — а она неотменяема НАМЕРЕННО (иначе прод остаётся в промежуточном виде).
d="$(mk_case group-deploy-main)" || { setup_fail "S35-группа-деплоя" "фикстура"; return; }
mutate "${d}/${BB}" 'group: branch-build-${{ github.ref }}' 'group: deploy-main' || setup_fail "S35-группа-деплоя" "литерал"
expect_red "S35-группа-деплоя" "${d}" "B8" "ОТМЕНИТ ИДУЩИЙ ДЕПЛОЙ"

# --- B9: подделка required-контекста и права ---------------------------------------------
# Замер: branch protection `main` требует КОНТЕКСТ «All checks passed», а контекст — это
# display-имя джоба ЛЮБОГО workflow на том же SHA (все check-run'ы репозитория идут от
# одного app.id). Одноимённый джоб здесь производит требуемый контекст в обход агрегата.
d="$(mk_case name-forgery)" || { setup_fail "S36-подделка-контекста" "фикстура"; return; }
mutate "${d}/${BB}" 'name: fmt + clippy + test (ветка)' 'name: All checks passed' || setup_fail "S36-подделка-контекста" "литерал"
expect_red "S36-подделка-контекста" "${d}" "B9" "ПОДДЕЛКИ merge-гейта"

d="$(mk_case write-perms)" || { setup_fail "S37-права-записи" "фикстура"; return; }
mutate "${d}/${BB}" $'  build-test:\n    name: fmt' $'  build-test:\n    permissions: write-all\n    name: fmt' \
  || setup_fail "S37-права-записи" "литерал"
expect_red "S37-права-записи" "${d}" "B9" "права записи не нужны"

}

# ═══ Батарея мутантов ═══════════════════════════════════════════════════════════════════
# Мутант обезвреживает ОДНУ проверку барьера, вырезая её ВЫЗОВ. Ожидание — РАВЕНСТВО
# kill-set'ов: ровно заявленные сценарии становятся зелёными, и ни одного лишнего.
# B1 мутанта не имеет — обоснование в шапке.
battery() {
  local mutants=(
    "M2-B2:check_b2:S4-без-push S5-опечатка-ключа S6-исключает-всё S7-фильтр-включения S8-оба-ключа S9-main-не-исключён S28-два-ключа-on"
    "M3-B3:check_b3:S10-без-clippy S11-без-all-features S12-test-без-all S13-дрейф-ci S14-без-components S15-echo-вместо-вызова S16-порядок-шагов S17-эталона-нет S27-глушение-кода"
    "M4-B4:check_b4:S18-if-false S19-continue-on-error S25-шаг-coe S26-шаг-if-false"
    "M5-B5:check_b5:S20-ci-glob S21-ci-ignore"
    "M6-B6:check_b6:S22-deploy-слушает S23-deploy-ветки S24-deploy-paths"
    "M7-B7:check_b7:S29-джоба-нет S30-не-в-needs S31-не-в-условии S32-echo-не-вызов"
    "M8-B8:check_b8:S33-без-concurrency S34-группа-не-по-ref S35-группа-деплоя"
    "M9-B9:check_b9:S36-подделка-контекста S37-права-записи"
  )
  local bfail=0
  for spec in "${mutants[@]}"; do
    local name="${spec%%:*}" rest="${spec#*:}"
    local fn="${rest%%:*}" declared="${rest#*:}"
    local mut="${WORK}/mutant-${name}.sh"
    register "${mut}"
    # Вырезаем ВЫЗОВ проверки в main(); отступ сохраняется, иначе python не разберёт файл.
    sed "s/^    ${fn}(.*/    pass  # МУТАНТ ${name}/" "${SUT}" > "${mut}"
    if ! grep -q "МУТАНТ ${name}" "${mut}"; then
      printf 'SETUP-FAIL %-26s мутант не построен: вызов %s( не найден\n' "${name}" "${fn}"
      bfail=$((bfail + 1)); continue
    fi

    # Прогон полного набора против мутанта; собираем, что позеленело.
    local before_pass=${PASS} before_fail=${FAIL}
    local saved_names=("${FAILED_NAMES[@]+"${FAILED_NAMES[@]}"}")
    PASS=0; FAIL=0; FAILED_NAMES=()
    SUT_ACTIVE="${mut}"
    local out; out="$(scenarios 2>&1)"
    SUT_ACTIVE="${SUT}"
    # Сценарий «позеленел» = проба на нём упала со словами «стаб ПРОПУЩЕН».
    local killed; killed="$(grep 'стаб ПРОПУЩЕН' <<<"${out}" | awk '{print $2}' | sort | tr '\n' ' ')"
    PASS=${before_pass}; FAIL=${before_fail}; FAILED_NAMES=("${saved_names[@]+"${saved_names[@]}"}")

    local expect; expect="$(tr ' ' '\n' <<<"${declared}" | sed '/^$/d' | sort | tr '\n' ' ')"
    if [ "${killed}" = "${expect}" ]; then
      printf 'ok         %-26s kill-set совпал (%s)\n' "${name}" "$(wc -w <<<"${expect}") сцен."
    else
      printf 'FAIL       %-26s kill-set РАЗОШЁЛСЯ\n' "${name}"
      printf '           заявлено: %s\n' "${expect}"
      printf '           получено: %s\n' "${killed}"
      bfail=$((bfail + 1))
    fi
  done
  return ${bfail}
}

# ═══ Прогон ═════════════════════════════════════════════════════════════════════════════
if [ ! -x "${SUT}" ] && [ ! -f "${SUT}" ]; then
  echo "SETUP-FAIL: предмет ${SUT} не найден"; exit 1
fi

echo "── СЦЕНАРИИ (позитивный контроль + анти-плацебо стабами)"
scenarios
BATT_RC=0
if [ "${1:-}" = "--battery" ]; then
  echo
  echo "── БАТАРЕЯ МУТАНТОВ (равенство kill-set'ов)"
  battery || BATT_RC=$?
fi

echo
# Число сценариев ПЕЧАТАЕТ САМА ПРОБА от своего счётчика. Литерал, живущий отдельно от
# предмета, врёт — замер `gate-meta`: заявленные «27» против фактических 31.
echo "сценариев исполнено: $((PASS + FAIL))  ok: ${PASS}  FAIL: ${FAIL}"
if [ ${#FAILED_NAMES[@]} -gt 0 ]; then
  printf 'упали: %s\n' "${FAILED_NAMES[*]}"
fi

cleanup
trap - EXIT
TMP_AFTER="$(own_dirs)"
echo "каталогов red-branchbuild-* до: ${TMP_BEFORE}, после уборки: ${TMP_AFTER}"
if [ "${TMP_AFTER}" -gt "${TMP_BEFORE}" ]; then
  echo "FAIL  проба течёт: свои фикстуры не убраны"
  FAIL=$((FAIL + 1))
fi

if [ "${FAIL}" -gt 0 ] || [ "${BATT_RC}" -ne 0 ]; then
  echo "VERDICT: FAIL (сценариев: ${FAIL}, мутантов с разошедшимся kill-set: ${BATT_RC})"
  exit 1
fi
echo "VERDICT: PASS"
