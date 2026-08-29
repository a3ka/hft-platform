#!/usr/bin/env bash
# Проба привязки вердикта к предмету — `scripts/check_gate_meta.sh` (M-60b G3).
#
# ЗАЧЕМ — три инцидента, все наши.
#
# 1. C-062 (2026-08-04). Критик отработал круг в дереве ЧУЖОГО репозитория, честно доложил
#    аномалию («нет .claude/rules/gates.md») — и вывода не сделал ни он, ни диспетчер.
#    В шапке вердикта стоит `Base: origin/main @ 9a0e48f0` — ревизия, которой в НАШЕЙ истории
#    нет вовсе. Норма «работай в своём репозитории» существовала только в голове. GM-3/GM-4 —
#    машинный реплей этого случая: вердикт с чужим repo и с несуществующей ревизией не пройдёт.
#
# 2. Подмена предмета ПОСЛЕ вердикта. Проходной вердикт по одному HEAD прикрывает merge
#    другого: «критик смотрел это» и «reviewer одобряет то же самое» перестают совпадать.
#    Лечится subject-lock'ом: СОБСТВЕННАЯ РАБОТА ВЕТКИ после `audited_head` не смеет трогать
#    пути класса «гейт». «Собственная» — это диапазон `BASE..HEAD ^audited_head`, из которого
#    на merge-ref исключён первый родитель (main-сторона), с комбинированным дифом у merge'ей.
#    Прежняя формулировка («диф `audited_head..HEAD`») описывала семантику, которой больше нет:
#    на merge-ref она приписывала ветке работу `main`а (`C-128` Б-1/Б-2, N-4).
#    Лок применяется ТОЛЬКО к проходным исходам — после REJECT правки штатны, и лок,
#    красящий нормальный круг, был бы вреднее отсутствующего (GM-11).
#
# 3. Молчаливый merge БЕЗ вердикта (TD-105: M-32/33/34 уехали в прод без единого артефакта
#    гейта, и это было ненаблюдаемо). Проверка вердиктов, ПОПАВШИХ в диапазон, слепа к их
#    ОТСУТСТВИЮ — дефект класса «наблюдает сбой, но не отсутствие» (testing.md, целостность
#    гейта, свойство 4). GM-17..GM-19 — проверка отсутствия; её предел назван в спеке
#    M-60b §5: merge, НЕ называющий milestone в subject'е, не покрыт — TD-105 закрывается
#    ЧАСТИЧНО, и это говорится, а не подразумевается.
#
# ОТСТУПЛЕНИЕ ОТ C-064 F-064-3 — НАЗВАНО ЯВНО (К-3 разбора 13.08; спека M-60b §4).
# GM-16 («RED на неверный `milestone:`») НЕ НАПИСАН, номер СОЖЖЁН: валидация поля
# `milestone:` против диффа/содержания — класс «барьер ВЫЧИСЛЯЕТ предмет артефакта»,
# упразднённый решением founder'а 12.08 по M-61 (вариант Б; шесть блокеров M-61 подряд жили
# в этом классе). Поле `milestone:` — ДЕКЛАРАЦИЯ автора (машинно — только непустота), связь
# вердикта с предметом объявляется текстом (`gates.md` §12); СООТВЕТСТВИЕ вердикта предмету
# остаётся суждением критика/reviewer'а. Следующий сценарий после GM-15 — GM-17.
#
# КОНТРАКТ ГЕЙТА (задаётся этой пробой, реализуется dev'ом). Для каждого файла
# research/{critiques,reviews,arbitration}/*.md, ДОБАВЛЕННОГО ИЛИ ИЗМЕНЁННОГО в диапазоне
# события, обязана присутствовать шапка:
#     <!-- GATE-META
#     milestone: <id>
#     audited_repo: <owner/name>
#     audited_base: <sha>
#     audited_head: <sha>
#     verdict: <REJECT|NOTE|APPROVE|PASS|CONCERNS|KILL|ESCALATE|DECISION>
#     -->
# Проверки: поля непусты; verdict — из перечня; audited_repo == origin ЭТОГО репо;
# audited_base и audited_head существуют в ЭТОЙ истории; audited_head — предок HEAD; для
# проходных исходов — subject-lock на классы `.claude/rules/**`, `.github/workflows/**`,
# `scripts/verify_*.sh`, `scripts/check_*.sh`, `scripts/tests/red_*.sh`. Выход из лока:
# `ALLOW-SUBJECT-CHANGE: <причина>` в теле коммита диапазона. База — ИЗ СОБЫТИЯ;
# пустая/zero/не-предок ⇒ FAIL (блокер B1, C-006).
# ОТСУТСТВИЕ (К-4): для каждого MERGE-коммита диапазона, subject которого называет `M-NN`,
# в дереве этого merge обязан существовать research/reviews/R-*.md, содержащий тот же
# литерал `M-NN`, — иначе FAIL. Не-merge коммиты проверкой отсутствия не судятся (GM-19:
# иначе каждый рабочий коммит потребует вердикта — лок вреднее отсутствующего).
# Файлы вердиктов ВНЕ диапазона не трогаются: ретроспективно править 60+ защищённых
# артефактов ради формы — вред, а не польза.

set -uo pipefail

ROOT_REPO="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT_REPO}/scripts/check_gate_meta.sh}"
ZERO=0000000000000000000000000000000000000000
GATE_CLASS_FILE="scripts/verify_M-99.sh"
CHECK_CLASS_FILE="scripts/check_m99.sh"
RED_CLASS_FILE="scripts/tests/red_m99.sh"

FAILED=0
PASSED=0

# Реестр фикстур ведётся в ФАЙЛЕ, а не в переменной: часть фикстур рождается в подоболочках,
# где присваивание переменной теряется (тот же класс, что дал 10 400 каталогов /tmp у
# red_docs_freeze; эталон починки — scripts/tests/red_artifact_ids.sh).
FIXTURE_REG="$(mktemp /tmp/red-gatemeta-reg-XXXXXX)"
reg_fixture() { printf '%s\n' "$1" >> "${FIXTURE_REG}"; }
cleanup_fixtures() {
  [ -n "${KEEP_FIXTURES:-}" ] && { echo "фикстуры оставлены (KEEP_FIXTURES): ${FIXTURE_REG}"; return 0; }
  while IFS= read -r d; do
    [ -n "$d" ] || continue
    case "$d" in /tmp/red-gatemeta-*) chmod -R u+rwX "$d" 2>/dev/null; rm -rf "$d" ;; esac
  done < "${FIXTURE_REG}" 2>/dev/null
  rm -f "${FIXTURE_REG}"
}
trap cleanup_fixtures EXIT
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

[ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}. 127 от bash неотличим от честного отказа гейта."
bash -n "${BARRIER}" 2>/dev/null || die "барьер не парсится — сценарии мерили бы ошибку интерпретатора."

# ── Фикстура ──────────────────────────────────────────────────────────────────────────
# Результат отдаётся ЧЕРЕЗ ИМЯ ПЕРЕМЕННОЙ ($1), а не через stdout. Причина структурная:
# при форме `mk_repo R` тело функции исполняется в ПОДОБОЛОЧКЕ, и `die` внутри неё
# завершает только её — проба продолжается с ПУСТЫМ $R. Дальше `( cd "$R" && git add -A )`
# исполняется в РАБОЧЕМ дереве (`cd ""` возвращает 0 и никуда не переходит), а сценарии
# печатают PASS против несозданной фикстуры. Присваивание по имени возвращает `die`
# в основную оболочку, где он и обязан останавливать прогон.
# Внутренние локальные названы `__fx_*` НАМЕРЕННО: в bash динамическая область видимости, и
# `local d` внутри функции ЗАТЕНЯЕТ переменную с тем же именем у вызывающего — `printf -v "$1"`
# записал бы в локальную копию. Префикс исключает класс, а не случай.
mk_repo() { # $1=имя переменной для результата, $2=origin URL (по умолчанию — наш)
  local __fx_d __fx_origin="${2:-https://github.com/a3ka/hft-platform.git}"
  __fx_d="$(mktemp -d /tmp/red-gatemeta-XXXXXX)" || die mktemp
  ( cd "$__fx_d" && git init -q \
    && git config user.email a@b.c && git config user.name t \
    && git remote add origin "$__fx_origin" \
    && mkdir -p research/critiques research/reviews scripts/tests .claude/rules docs .github/workflows \
    && echo base > docs/DESIGN.md && echo base > scripts/verify_M-99.sh \
    && echo base > scripts/check_m99.sh && echo base > scripts/tests/red_m99.sh \
    && echo base > .claude/rules/gates.md && echo base > .github/workflows/ci.yml \
    && git add -A && git commit -q -m base ) || die "инициализация фикстуры"
  reg_fixture "$__fx_d"
  printf -v "$1" '%s' "$__fx_d"
}

# Второй рубеж к тому же классу: даже если фикстура не создалась, ни одна функция не
# исполнит git-команду с пустым путём. `cd ""` возвращает 0 и остаётся в ТЕКУЩЕМ каталоге —
# то есть в рабочем дереве репозитория, где `git add -A && git commit` снёс бы чужую работу.
need_fixture() { # $1=путь $2=имя вызывающего
  [ -n "${1:-}" ] && [ -d "${1:-}" ] \
    || die "СЕТАП НЕ СОСТОЯЛСЯ: ${2} получил пустой/несуществующий путь фикстуры «${1:-}» — без этой проверки команда ушла бы в РАБОЧЕЕ дерево"
}

# $1=repo $2=verdict-строка $3=audited_repo $4=audited_base $5=audited_head [$6=имя] [$7=каталог]
add_verdict() {
  local r="$1" name="${6:-C-999-test.md}" dir="${7:-research/critiques}"
  need_fixture "$r" add_verdict
  { echo "<!-- GATE-META"
    echo "milestone: M-99"
    echo "audited_repo: $3"
    echo "audited_base: $4"
    echo "audited_head: $5"
    echo "verdict: $2"
    echo "-->"
    echo
    echo "тело вердикта"
  } > "$r/$dir/$name" || die "запись вердикта"
  ( cd "$r" && git add -A && git commit -q -m "docs(critic): вердикт $name" ) || die "коммит вердикта"
}

touch_file() { # $1=repo $2=путь $3=тело-коммита
  need_fixture "$1" touch_file
  ( cd "$1" && echo "правка" >> "$2" && git add -A && git commit -q -F - <<EOF
правка $2

$3
EOF
  ) || die "коммит правки $2"
}

run_barrier() { # $1=repo $2=before-sha [$3=EVENT_NAME] [$4=PR_BASE_SHA] [$5=mergeref]
  need_fixture "$1" run_barrier
  local st ev="${3:-push}" pb="${4-$2}" gs=""
  # $5=mergeref — ПОДТВЕРЖДЕНИЕ ПРОД-ФОРМЫ: GitHub на `pull_request` кладёт в `GITHUB_SHA`
  # именно merge-ref, и барьер включает якорь main-стороны только по этому совпадению
  # (`C-130` Б-1). Сценарий, НЕ передающий пятый аргумент, воспроизводит ручной прогон /
  # вершину ветки — там якорь обязан быть выключен, и это отдельный предмет (GM-16g).
  if [ "${5:-}" = "mergeref" ]; then gs="$( cd "$1" && git rev-parse HEAD )"; fi
  ( cd "$1" && EVENT_NAME="$ev" PUSH_BEFORE="$2" PR_BASE_SHA="$pb" GITHUB_SHA="$gs" bash "${BARRIER}" >/dev/null 2>&1 )
  st=$?
  # 126/127 — отказ СРЕДЫ, а не вердикт гейта (`positive_control` этот класс НЕ ловит:
  # он проверяет лишь годную ветку, а падать может ОТКАЗНАЯ — например, барьер зовёт
  # отсутствующий в CI `jq`/`gh`. Тогда все сценарии «гейт отказал» зеленеют против
  # механизма, который не отказывает, а падает. C-086 F-086-1 требовал именно этого различения).
  case $st in
    126|127) die "барьер вернул ${st} (не найден / не исполняется) — это отказ СРЕДЫ, а не отказ гейта; сценарий засчитал бы падение за срабатывание" ;;
  esac
  return $st
}
head_of() { need_fixture "$1" head_of; ( cd "$1" && git rev-parse HEAD ); }
positive_control() {
  local r b
  mk_repo r
  b="$(head_of "$r")"
  add_verdict "$r" "APPROVE" "a3ka/hft-platform" "$b" "$b"
  ( cd "$r" && EVENT_NAME=push PUSH_BEFORE="$b" PR_BASE_SHA="$b" bash "${BARRIER}" >/dev/null 2>&1 ) \
    || die "барьер не проходит заведомо годную фикстуру (валидная GATE-META, audited_head предок HEAD); setup не состоялся"
}

echo "── Привязка вердикта к предмету + subject-lock + отсутствие (M-60b G3) ──"
echo "── сценарии GM-1..GM-49; GM-16 СОЖЖЁН (спека M-60b §4, шапка выше) ──"
echo "барьер: ${BARRIER}"
echo
positive_control
echo "SETUP positive-control: барьер принимает заведомо годную GATE-META-фикстуру"
echo

# ── Блок 1: форма и принадлежность предмету ──────────────────────────────────────────

# GM-1 — вердикт без шапки
mk_repo R; B="$(head_of "$R")"
echo "вердикт без метаданных" > "$R/research/critiques/C-001.md"
( cd "$R" && git add -A && git commit -q -m "вердикт без шапки" ) || die "GM-1"
run_barrier "$R" "$B" && fail "GM-1 вердикт БЕЗ шапки прошёл — привязки к предмету нет" \
                      || pass "GM-1 вердикт без GATE-META заблокирован"

# GM-2 — поле пустое
mk_repo R; B="$(head_of "$R")"; H="$B"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" ""
run_barrier "$R" "$B" && fail "GM-2 пустой audited_head прошёл — шапка стала ритуалом" \
                      || pass "GM-2 пустое поле шапки отвергнуто"

# GM-3 — ЧУЖОЙ репозиторий (реплей C-062)
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "einhardsystems/einhard-runtime" "$B" "$B"
run_barrier "$R" "$B" && fail "GM-3 вердикт по ЧУЖОМУ репозиторию прошёл — C-062 повторим" \
                      || pass "GM-3 чужой audited_repo заблокирован (реплей C-062)"

# GM-4 — audited_head, которого в нашей истории НЕТ (реплей C-062: 9a0e48f0)
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "9a0e48f09a0e48f09a0e48f09a0e48f09a0e48f0"
run_barrier "$R" "$B" && fail "GM-4 несуществующая ревизия прошла — вердикт судил чужую историю" \
                      || pass "GM-4 несуществующий audited_head заблокирован (реплей C-062)"

# GM-5 — audited_base, которого нет
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" "$B"
run_barrier "$R" "$B" && fail "GM-5 несуществующий audited_base прошёл" \
                      || pass "GM-5 несуществующий audited_base заблокирован"

# GM-6 — audited_head существует, но НЕ предок HEAD (side-ветка)
mk_repo R; B="$(head_of "$R")"
( cd "$R" && git checkout -q -b side && echo x >> docs/DESIGN.md \
  && git add -A && git commit -q -m side ) || die "GM-6 side"
SIDE="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q - ) || die "GM-6 back"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$SIDE"
run_barrier "$R" "$B" && fail "GM-6 audited_head не из этой линии истории прошёл" \
                      || pass "GM-6 audited_head вне линии истории заблокирован"

# GM-7 — всё корректно, после вердикта ничего не менялось
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$B"
run_barrier "$R" "$B" && pass "GM-7 корректный вердикт проходит" \
                      || fail "GM-7 ложное срабатывание на корректной шапке"

# GM-8 — старый вердикт, не тронутый в диапазоне ⇒ требований нет
mk_repo R
echo "старый вердикт без шапки" > "$R/research/critiques/C-000-old.md"
( cd "$R" && git add -A && git commit -q -m "старый вердикт" ) || die "GM-8"
B="$(head_of "$R")"
touch_file "$R" "docs/DESIGN.md" "правка вне вердиктов"
run_barrier "$R" "$B" && pass "GM-8 старые вердикты вне диапазона не трогаются" \
                      || fail "GM-8 гейт требует шапку ретроспективно — правка защищённых артефактов"

# GM-9 — база не установлена достоверно ⇒ fail-closed
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "einhardsystems/einhard-runtime" "$B" "$B"
run_barrier "$R" "$ZERO" && fail "GM-9 zero-SHA база дала ПРОПУСК" || pass "GM-9 zero-SHA: fail-closed"
run_barrier "$R" ""      && fail "GM-9b пустая база дала ПРОПУСК" || pass "GM-9b пустая база: fail-closed"

# ── Блок 2: subject-lock ─────────────────────────────────────────────────────────────

# GM-10 — APPROVE, после него тронут verify-скрипт ⇒ блок
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта после одобрения"
run_barrier "$R" "$B" && fail "GM-10 гейт правился ПОСЛЕ APPROVE — вердикт прикрыл другой предмет" \
                      || pass "GM-10 subject-lock: правка гейта после APPROVE заблокирована"

# GM-11 — REJECT, та же правка ⇒ ПРОХОД (после REJECT правки штатны)
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${GATE_CLASS_FILE}" "исправление по вердикту"
run_barrier "$R" "$B" && pass "GM-11 после REJECT правки не блокируются (штатный круг)" \
                      || fail "GM-11 лок красит нормальный круг исправлений — вреднее отсутствия лока"

# GM-12 — APPROVE, тронута зона правил ⇒ блок
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "PASS" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" ".claude/rules/gates.md" "правка правил после одобрения"
run_barrier "$R" "$B" && fail "GM-12 правила менялись после PASS — скоуп подменён" \
                      || pass "GM-12 subject-lock накрывает .claude/rules"

# GM-13 — APPROVE + явный ALLOW-SUBJECT-CHANGE ⇒ проход со следом
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${GATE_CLASS_FILE}" "ALLOW-SUBJECT-CHANGE: правка гейта согласована с founder'ом"
run_barrier "$R" "$B" && pass "GM-13 явный ALLOW-SUBJECT-CHANGE открывает лок (след в истории)" \
                      || fail "GM-13 легального выхода из лока нет — гейт станет обходиться силой"

# GM-14 — APPROVE, тронут файл ВНЕ класса «гейт» ⇒ проход (анти-ложноположительный)
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "docs/DESIGN.md" "обычная правка документа"
run_barrier "$R" "$B" && pass "GM-14 правка вне класса «гейт» не блокируется" \
                      || fail "GM-14 лок вышел за свой класс — красит обычные правки"

# GM-15 — NOTE, тронута проводка CI ⇒ блок
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "NOTE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" ".github/workflows/ci.yml" "правка проводки после NOTE"
run_barrier "$R" "$B" && fail "GM-15 проводка CI менялась после NOTE — аудировался другой пайплайн" \
                      || pass "GM-15 subject-lock накрывает .github/workflows"

# GM-16 — СОЖЖЁН. Не написан намеренно: см. шапку (отступление от C-064 F-064-3, спека §4).

# ── Блок 2bis: ДВИЖУЩАЯСЯ БАЗА — прод-форма PR, которой у блока 2 не было ─────────────
#
# Все сценарии выше — ОДНА линейная ветка: база неподвижна, всё после вердикта сделала сама
# ветка. Форма КАЖДОГО настоящего PR другая: CI на событии `pull_request` считает `HEAD`ом
# MERGE-REF (`refs/pull/N/merge`), то есть слияние ветки с ТЕКУЩИМ `main`, а `main` за время
# кругов гейта уезжает вперёд. Всё, что приехало в `main` после вердикта ветки, попадает в
# двухточечный диф `audited_head..HEAD` и приписывается ВЕТКЕ.
#
# ИНЦИДЕНТ, ради которого блок написан (уже случившийся, 2026-08-23, дважды за одну сессию):
#   · PR #28: subject-lock сработал на `scripts/verify_design_claims.sh` — файлы PR #56,
#     влитого в `main` часом ранее; диапазон самой ветки их не касался;
#   · PR #60: subject-lock сработал по вердикту `R-108`, который принадлежит ЧУЖОМУ, уже
#     влитому предмету (PR #56) и к ветке #60 отношения не имеет вовсе.
# Оба раза merge держался ложным красным, и обходился он токеном `ALLOW-SUBJECT-CHANGE` —
# то есть барьер вырождался в формальность, проставляемую на каждом PR.

# GM-16b — вердикт PASS на ветке; гейт-класс тронул MAIN (не ветка) ⇒ ПРОХОД.
# Против двухточечного дифа `audited_head..HEAD` этот сценарий КРАСЕН — он и есть RED.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-drift ) || die "GM-16b: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
AH_BR="$(head_of "$R")"
( cd "$R" && git checkout -q - ) || die "GM-16b: возврат на базу"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта, приехавшая в БАЗУ (чужой влитый PR)"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git merge -q --no-ff feat-drift -m "merge-ref: слияние ветки в базу" )   || die "GM-16b: merge-ref не собран"
setup_ok=1
( cd "$R" && [ "$(git rev-parse HEAD^1)" = "${MAIN_TIP}" ] ) || setup_ok=0
( cd "$R" && [ "$(git rev-parse HEAD^2)" = "${AH_BR}" ] )    || setup_ok=0
( cd "$R" && git log --format='' --name-only "${AH_BR}..HEAD" | grep -qx "${GATE_CLASS_FILE}" ) || setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-16b SETUP НЕ СОСТОЯЛСЯ: merge-ref не имеет формы merge(база, ветка) либо правка \
гейта не попала в базу — сценарий проверял бы не прод-форму, а линейную историю блока 2"
else
  run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" mergeref \
    && pass "GM-16b движущаяся база: правку гейта внесла БАЗА — лок не срабатывает" \
    || fail "GM-16b лок сработал на правке, которой ВЕТКА не делала: гейт-класс приехал из \
базы (чужой влитый PR). Двухточечный диф audited_head..HEAD на merge-ref меряет ещё и то, \
что сделал main — и барьер вырождается в токен на каждом PR"
fi

# GM-16c — АНТИ-БЛАНКЕТ к GM-16b: та же форма, но гейт-класс тронула САМА ВЕТКА ⇒ БЛОК.
# Без него фикс «не считать чужое» проходится реализацией «не считать ничего».
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-drift2 ) || die "GM-16c: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта САМОЙ ВЕТКОЙ после одобрения"
AH_BR="$(cd "$R" && git rev-parse HEAD~1)"   # вердикт; правка гейта идёт ПОСЛЕ него
( cd "$R" && git checkout -q - ) || die "GM-16c: возврат на базу"
touch_file "$R" "docs/DESIGN.md" "постороннее движение базы"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git merge -q --no-ff feat-drift2 -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-16c: merge-ref не собран"
run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" mergeref \
  && fail "GM-16c ветка правила гейт ПОСЛЕ своего APPROVE, а лок промолчал — сужение зашло \
слишком далеко и открыло лок вообще" \
  || pass "GM-16c анти-бланкет: правка гейта САМОЙ веткой по-прежнему блокируется"

# GM-16d — БАЗА СОБЫТИЯ УСТАРЕЛА. `github.event.pull_request.base.sha` обновляется ЛЕНИВО и
# расходится с main-стороной merge-ref (`C-128` Б-1, предъявлено реальными CI-прогонами
# PR #60). Форма GM-16b, но барьеру подаётся база ФОРКА, а не текущая вершина базы: против
# редакции, опирающейся ТОЛЬКО на `PR_BASE_SHA`, сценарий КРАСЕН — это и есть его RED.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-stale ) || die "GM-16d: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
AH_BR="$(head_of "$R")"
( cd "$R" && git checkout -q - ) || die "GM-16d: возврат на базу"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта, приехавшая в БАЗУ после форка"
( cd "$R" && git merge -q --no-ff feat-stale -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-16d: merge-ref не собран"
setup_ok=1
( cd "$R" && [ "$(git rev-parse HEAD^2)" = "${AH_BR}" ] ) || setup_ok=0
( cd "$R" && git log --format='' --name-only "${B0}..HEAD^1" | grep -qx "${GATE_CLASS_FILE}" ) || setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-16d SETUP НЕ СОСТОЯЛСЯ: правка гейта не лежит на main-стороне между форком и \
вершиной базы — устаревание базы не воспроизведено"
else
  run_barrier "$R" "${B0}" pull_request "${B0}" mergeref \
    && pass "GM-16d устаревшая база события: якорь HEAD^1 держит, ложного красного нет" \
    || fail "GM-16d барьер опёрся на УСТАРЕВШИЙ PR_BASE_SHA и снова приписал ветке работу \
базы. У долгоживущего PR (шесть кругов гейта — норма) это возвращает ложный красный, и токен \
чеканится снова: класс закрыт наполовину"
fi

# GM-16e — ЧУЖОЙ ТОКЕН ИЗ БАЗЫ НЕ ОТКРЫВАЕТ ЛОК. Диапазон поиска токена обязан совпадать с
# диапазоном, в котором найдено нарушение (`C-128` Б-2). Канал боевой, а не теоретический:
# токен, выданный одному PR, вливается в `main` и гасит локи соседних.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-leak ) || die "GM-16e: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта САМОЙ веткой, СВОЕГО токена нет"
( cd "$R" && git checkout -q - ) || die "GM-16e: возврат на базу"
touch_file "$R" "docs/DESIGN.md" "ALLOW-SUBJECT-CHANGE: чужой токен, выданный ДРУГОМУ предмету"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git merge -q --no-ff feat-leak -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-16e: merge-ref не собран"
setup_ok=1
( cd "$R" && git log --format='%B' "${B0}..HEAD^1" | grep -q 'ALLOW-SUBJECT-CHANGE' ) || setup_ok=0
( cd "$R" && git log --format='%B' "${B0}..HEAD^2" | grep -q 'ALLOW-SUBJECT-CHANGE' ) && setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-16e SETUP НЕ СОСТОЯЛСЯ: токен обязан лежать ТОЛЬКО на стороне базы и отсутствовать \
у ветки — иначе сценарий проверяет законный выход из лока, а не утечку"
else
  run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" mergeref \
    && fail "GM-16e ЧУЖОЙ токен из базы открыл лок на РЕАЛЬНОЕ нарушение ветки: каждый ложный \
красный чеканит токен, токен уезжает в main и гасит локи соседних PR — лок перестаёт нести \
информацию в обе стороны" \
    || pass "GM-16e чужой токен из базы лок НЕ открывает (диапазон токена = диапазону нарушения)"
fi

# GM-16f — EVIL MERGE. Правка гейт-класса, вложенная В САМ merge-коммит: `git merge --no-commit`
# + правка + `commit`, две команды, конфликт не нужен. Прежняя редакция объявляла это «названным
# пределом» и приписывала невидимость флагу `--no-merges`; на деле её давал молчаливый дефолт
# `diff-merges=off` (`C-128` M-3). Комбинированный диф (`--diff-merges=cc`) печатает у merge'а
# ровно то, что отличается от ВСЕХ родителей, — то есть контрабанду, и не печатает принесённого.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-evil ) || die "GM-16f: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
( cd "$R" && git checkout -q - ) || die "GM-16f: возврат на базу"
touch_file "$R" "docs/DESIGN.md" "постороннее движение базы"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git checkout -q feat-evil \
  && git merge --no-commit --no-ff "${MAIN_TIP}" >/dev/null 2>&1
  echo "контрабанда" >> "${GATE_CLASS_FILE}" && git add -A \
  && git commit -q -m "merge базы (с вложенной правкой гейта)" ) || die "GM-16f: evil merge не собран"
EVIL_TIP="$(head_of "$R")"
( cd "$R" && git checkout -q - ) || die "GM-16f: возврат на базу"
( cd "$R" && git merge -q --no-ff feat-evil -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-16f: merge-ref не собран"
setup_ok=1
( cd "$R" && [ "$(git rev-parse HEAD^2)" = "${EVIL_TIP}" ] ) || setup_ok=0
( cd "$R" && git show --format='' --name-only --diff-merges=cc "${EVIL_TIP}" | grep -qx "${GATE_CLASS_FILE}" ) || setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-16f SETUP НЕ СОСТОЯЛСЯ: правка гейта не вложена в merge-коммит ветки — предмет \
сценария (контрабанда, невидимая обычному дифу merge'а) не воспроизведён"
else
  run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" mergeref \
    && fail "GM-16f контрабанда в merge-коммите прошла: правка гейт-класса, вложенная в merge, \
невидима барьеру — канал НАМЕРЕННОГО обхода в две команды, без всякого конфликта" \
    || pass "GM-16f контрабанда в merge-коммите заблокирована (комбинированный диф её видит)"
fi

# GM-16g — SYNC-MERGE ВЕРШИНА ВЕТКИ: якорь обязан быть ВЫКЛЮЧЕН (`C-130` Б-1).
# Штатный `git merge main` последним коммитом ветки — рутина долгоживущего PR. У такого HEAD
# `HEAD^2` СУЩЕСТВУЕТ, но стороны ОБРАТНЫЕ: `HEAD^1` — ветка, `HEAD^2` — main. Редакция,
# включавшая якорь по одному существованию `HEAD^2`, исключала здесь ВЕТКУ, судила пустоту и
# печатала «вердиктов проверено: 0» — молчаливое ложное зелёное, неотличимое от честного
# «вердиктов нет». Прод-форма не подтверждена (пятый аргумент НЕ передан ⇒ `GITHUB_SHA` пуст),
# значит якорь не применяется и судится весь диапазон: fail-closed.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-sync ) || die "GM-16g: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта САМОЙ веткой после APPROVE, токена нет"
( cd "$R" && git checkout -q - ) || die "GM-16g: возврат на базу"
touch_file "$R" "docs/DESIGN.md" "постороннее движение базы"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git checkout -q feat-sync && git merge -q --no-ff "${MAIN_TIP}" \
    -m "git merge main (sync) — ВЕРШИНА ВЕТКИ, стороны обратны merge-ref'у" ) \
  || die "GM-16g: sync-merge не собран"
setup_ok=1
( cd "$R" && [ "$(git rev-parse HEAD^2)" = "${MAIN_TIP}" ] ) || setup_ok=0
( cd "$R" && git merge-base --is-ancestor "${MAIN_TIP}" 'HEAD^1' ) && setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-16g SETUP НЕ СОСТОЯЛСЯ: вершина не имеет формы merge(ветка, main) — предмет \
(обратная ориентация родителей) не воспроизведён"
else
  run_barrier "$R" "${B0}" pull_request "${B0}" \
    && fail "GM-16g якорь применён на вершине ВЕТКИ: исключена сторона ветки, суд получил \
пустоту, нарушение subject-lock прошло МОЛЧА. Прод-форма merge-ref здесь НЕ подтверждена, и \
предпосылка «HEAD^1 — main» ложна" \
    || pass "GM-16g sync-merge вершина: прод-форма не подтверждена ⇒ якорь выключен, нарушение поймано"
fi

# GM-16h — ФИЛЬТР ВЫБОРА ВЕРДИКТОВ: чужой проходной вердикт из базы НЕ СУДИТСЯ (`C-130` Б-2).
# Форма боевая: именно так `R-108` (вердикт чужого, уже влитого PR #56) запер PR #60 — файл в
# дереве merge-ref'а есть, к предмету ветки отношения не имеет. Фильтр несущий, но до этого
# сценария его не пиннило НИЧЕГО: нейтрализация строки оставляла набор зелёным целиком.
# Ветка здесь ведёт ШТАТНЫЙ круг исправлений: свой вердикт REJECT (лок к нему не применяется)
# плюс правка гейта по этому REJECT'у — то есть законная работа, которую чужой APPROVE не
# вправе запереть.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-md2 ) || die "GM-16h: ветка не создана"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B0" "$B0"
touch_file "$R" "${GATE_CLASS_FILE}" "исправление по СВОЕМУ REJECT — штатный круг"
( cd "$R" && git checkout -q - ) || die "GM-16h: возврат на базу"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0" "R-foreign.md" "research/reviews"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git merge -q --no-ff feat-md2 -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-16h: merge-ref не собран"
setup_ok=1
( cd "$R" && git cat-file -e 'HEAD^1:research/reviews/R-foreign.md' 2>/dev/null ) || setup_ok=0
( cd "$R" && git cat-file -e 'HEAD^2:research/reviews/R-foreign.md' 2>/dev/null ) && setup_ok=0
( cd "$R" && git cat-file -e 'HEAD^2:research/critiques/C-999-test.md' 2>/dev/null ) || setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-16h SETUP НЕ СОСТОЯЛСЯ: чужой вердикт обязан лежать ТОЛЬКО на стороне базы, свой — \
только на стороне ветки; иначе сценарий не различает предметы"
else
  # БАЗА ПОДАЁТСЯ УСТАРЕВШЕЙ (`B0`, точка форка) — и это НЕ упрощение, а условие предмета.
  # При свежей базе чужой вердикт уже лежит в ней, и его отсекает сам двухточечный диф
  # `BASE..HEAD`, а не проверяемый фильтр: сценарий был бы зелен при ЛЮБОЙ его реализации.
  # Поймано собственным прогоном мутации (фильтр снят — сценарий остался зелёным), то есть
  # первая редакция этого сценария была вакуумом ровно того класса, который он назван ловить.
  # Боевая форма и была такой: `R-108` приехал в `main` ПОСЛЕ того, как у PR #60 зафиксировалась
  # база события.
  run_barrier "$R" "${B0}" pull_request "${B0}" mergeref \
    && pass "GM-16h чужой проходной вердикт из базы в суд не попадает (фильтр выбора вердиктов)" \
    || fail "GM-16h чужой APPROVE, приехавший из базы, запер ЗАКОННУЮ правку ветки по её \
собственному REJECT'у — ровно исторический ложный красный PR #60 по вердикту R-108"
fi


# ── Блок 3: ОТСУТСТВИЕ вердикта (К-4; testing.md свойство 4 — «наблюдать отсутствие») ──

# GM-17 — merge, называющий M-NN, БЕЗ research/reviews/R-* в дереве слияния ⇒ блок
mk_repo R; B="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-m42 && echo work >> docs/DESIGN.md \
  && git add -A && git commit -q -m "feat(M-42): task #1 — работа" \
  && git checkout -q - \
  && git merge -q --no-ff -m "merge M-42: milestone в main" feat-m42 ) || die "GM-17 merge"
( cd "$R" && git rev-parse -q --verify HEAD^2 >/dev/null ) \
  || die "GM-17: HEAD не merge-коммит — сценарий тестировал бы не то"
[ ! -e "$R/research/reviews/R-100-M-42.md" ] || die "GM-17: R-файл существует — фикстура не про отсутствие"
run_barrier "$R" "$B" && fail "GM-17 merge M-42 БЕЗ R-* прошёл — молчаливый merge (TD-105) не пойман" \
                      || pass "GM-17 merge milestone'а без вердикта reviewer'а заблокирован"

# GM-18 — тот же merge, R-файл с литералом M-42 в дереве слияния ЕСТЬ ⇒ проход
# (анти-ложноположительный). R-файл добавлен В ДИАПАЗОНЕ ⇒ сам несёт валидную GATE-META —
# иначе сценарий краснел бы по чужой причине (форма шапки, а не отсутствие).
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$B" "R-100-M-42.md" "research/reviews"
( cd "$R" && git show HEAD:research/reviews/R-100-M-42.md | grep -q "M-99" ) \
  || die "GM-18: R-файл не читается из дерева"
( cd "$R" && echo "вердикт по M-42" >> research/reviews/R-100-M-42.md \
  && git add -A && git commit -q --amend --no-edit ) || die "GM-18 литерал M-42"
( cd "$R" && git show HEAD:research/reviews/R-100-M-42.md | grep -q "M-42" ) \
  || die "GM-18: литерал M-42 не в дереве — фикстура не про присутствие"
( cd "$R" && git checkout -q -b feat-m42 && echo work >> docs/DESIGN.md \
  && git add -A && git commit -q -m "feat(M-42): task #1 — работа" \
  && git checkout -q - \
  && git merge -q --no-ff -m "merge M-42: milestone в main" feat-m42 ) || die "GM-18 merge"
( cd "$R" && git rev-parse -q --verify HEAD^2 >/dev/null ) || die "GM-18: HEAD не merge-коммит"
run_barrier "$R" "$B" && pass "GM-18 merge с R-файлом, называющим M-42, проходит" \
                      || fail "GM-18 ложное срабатывание: вердикт в дереве есть, а гейт блокирует"

# GM-19 — НЕ-merge коммит, называющий M-NN, без R-* ⇒ проход (судятся только merge:
# иначе каждый рабочий коммит потребует вердикта — лок вреднее отсутствующего)
mk_repo R; B="$(head_of "$R")"
( cd "$R" && echo work >> docs/DESIGN.md && git add -A \
  && git commit -q -m "feat(M-43): task #1 — обычная работа" ) || die "GM-19"
( cd "$R" && ! git rev-parse -q --verify HEAD^2 >/dev/null ) || die "GM-19: HEAD внезапно merge"
run_barrier "$R" "$B" && pass "GM-19 рабочий не-merge коммит с M-NN не требует вердикта" \
                      || fail "GM-19 проверка отсутствия вышла за merge-класс — красит каждый коммит"

# ── Блок 4: множественность и полный subject-lock класса «гейт» ───────────────────────

# GM-20 — два изменённых verdict-артефакта: первый валиден, второй нет ⇒ блок.
# Реализация, проверяющая только первый путь/первый коммит диапазона, обязана покраснеть.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$B" "C-100-valid.md"
{ echo "<!-- GATE-META"
  echo "milestone: M-99"
  echo "audited_repo: a3ka/hft-platform"
  echo "audited_base: $B"
  echo "audited_head: "
  echo "verdict: APPROVE"
  echo "-->"
  echo
  echo "второй вердикт невалиден"
} > "$R/research/critiques/C-101-invalid.md" || die "GM-20 invalid verdict"
( cd "$R" && git add -A && git commit -q -m "docs(critic): второй вердикт без audited_head" ) \
  || die "GM-20 commit"
run_barrier "$R" "$B" && fail "GM-20 два verdict-артефакта, второй невалиден, ПРОШЛИ — проверен только первый" \
                      || pass "GM-20 множественность verdict-артефактов: второй невалидный пойман"

# GM-21 — два merge-коммита: у первого R-файл есть, у второго нет ⇒ блок.
# Реализация, проверяющая только первый merge диапазона, обязана покраснеть.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$B" "R-144-M-44.md" "research/reviews"
( cd "$R" && echo "reviewer verdict for M-44" >> research/reviews/R-144-M-44.md \
  && git add -A && git commit -q --amend --no-edit ) || die "GM-21 R-file M-44"
( cd "$R" && git checkout -q -b feat-m44 && echo work44 >> docs/DESIGN.md \
  && git add -A && git commit -q -m "feat(M-44): task #1 — работа" \
  && git checkout -q - \
  && git merge -q --no-ff -m "merge M-44: milestone в main" feat-m44 ) || die "GM-21 merge M-44"
( cd "$R" && git checkout -q -b feat-m45 && echo work45 >> docs/DESIGN.md \
  && git add -A && git commit -q -m "feat(M-45): task #1 — работа" \
  && git checkout -q - \
  && git merge -q --no-ff -m "merge M-45: milestone в main" feat-m45 ) || die "GM-21 merge M-45"
( cd "$R" && git rev-parse -q --verify HEAD^2 >/dev/null ) \
  || die "GM-21: HEAD не второй merge-коммит — сценарий тестировал бы не то"
run_barrier "$R" "$B" && fail "GM-21 два merge-коммита, второй без R-* ПРОШЁЛ — проверен только первый merge" \
                      || pass "GM-21 множественность merge-коммитов: второй без verdict пойман"

# GM-22 — APPROVE, после него тронут scripts/check_*.sh ⇒ блок.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${CHECK_CLASS_FILE}" "правка check-барьера после одобрения"
run_barrier "$R" "$B" && fail "GM-22 scripts/check_*.sh правился ПОСЛЕ APPROVE — механизм-гейт выпал из subject-lock" \
                      || pass "GM-22 subject-lock накрывает scripts/check_*.sh"

# GM-23 — APPROVE, после него тронут scripts/tests/red_*.sh ⇒ блок.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${RED_CLASS_FILE}" "правка red-пробы после одобрения"
run_barrier "$R" "$B" && fail "GM-23 scripts/tests/red_*.sh правился ПОСЛЕ APPROVE — RED-оракул выпал из subject-lock" \
                      || pass "GM-23 subject-lock накрывает scripts/tests/red_*.sh"

# GM-24 — предмет живёт в ДРУГОМ репозитории, и вердикт называет ИМЕННО его ⇒ ПРОХОД.
# Контракт (шапка): `audited_repo == origin ЭТОГО репо`, а не равенство литералу
# `a3ka/hft-platform`. Барьер с зашитым слагом проходит все остальные сценарии и при этом
# НЕ СПРАШИВАЕТ origin — то есть воспроизводит ровно C-062 («критик аудировал чужой
# репозиторий»), ради которого проверка и заводилась. Этот сценарий его убивает.
R=""; mk_repo R "https://github.com/other-org/other-repo.git"; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "other-org/other-repo" "$B" "$B"
run_barrier "$R" "$B" && pass "GM-24 audited_repo сверяется с ORIGIN репозитория, а не с зашитым слагом" \
                     || fail "GM-24 вердикт, называющий origin СВОЕГО репозитория, отвергнут — сверка идёт с литералом (класс C-062)"

# GM-25 — `milestone:` ПУСТ ⇒ FAIL. Шапка обещает «поля непусты», но на пустоту давилось
# единственное поле `audited_head`: мутация «убрать ms из проверки» оставляла набор зелёным
# (24/24). Обещание без сценария — не проверка.
mk_repo R; B="$(head_of "$R")"
{ echo "<!-- GATE-META"
  echo "milestone: "
  echo "audited_repo: a3ka/hft-platform"
  echo "audited_base: $B"
  echo "audited_head: $B"
  echo "verdict: APPROVE"
  echo "-->"
  echo
  echo "вердикт с пустым milestone"
} > "$R/research/critiques/C-102-empty-ms.md" || die "GM-25 запись"
( cd "$R" && git add -A && git commit -q -m "docs(critic): вердикт с пустым milestone" ) || die "GM-25 commit"
run_barrier "$R" "$B" && fail "GM-25 пустой milestone: ПРОШЁЛ — «поля непусты» не проверяется ни для одного поля, кроме audited_head" \
                     || pass "GM-25 пустой milestone заблокирован"

# GM-26 — `verdict` ВНЕ перечня ⇒ FAIL. В наборе жили 4 значения из 8 контракта, и ни один
# сценарий не давил на само членство в перечне: реализация, вообще не сверяющая перечень,
# брала 24/24. Не-проходным доказан был только REJECT.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "MAYBE" "a3ka/hft-platform" "$B" "$B" "C-103-bad-verdict.md"
run_barrier "$R" "$B" && fail "GM-26 verdict «MAYBE» вне перечня ПРОШЁЛ — перечень допустимых исходов не сверяется" \
                     || pass "GM-26 verdict вне перечня заблокирован"

# GM-27 — ПРОД-ФОРМА pull_request. Все сценарии выше зовут барьер только как `EVENT_NAME=push`
# и подают PUSH_BEFORE и PR_BASE_SHA ОДНИМ значением, поэтому мутанты «читаю только
# PUSH_BEFORE» и «читаю только PR_BASE_SHA» оба давали 24/24. CI триггерится и на
# `pull_request`, где `github.event.before` ПУСТ, а база приходит из `PR_BASE_SHA`
# (проводка-образец — `protected-artifacts`). Гейт, проверенный не тем вызовом, каким его
# зовёт прод, не проверен (`gates.md` §0bis, урок цены семи красных прогонов).
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$B" "C-104-pr-form.md"
# Зовётся ТОЙ ЖЕ функцией, что остальные сценарии: иначе вызов не попадает в `declared`,
# которое verify сверяет с числом исполненных, и шаг G краснеет «счёт не сошёлся».
if run_barrier "$R" "" pull_request "$B"; then
  pass "GM-27 прод-форма pull_request: база берётся из PR_BASE_SHA при пустом PUSH_BEFORE"
else
  fail "GM-27 на pull_request (PUSH_BEFORE пуст, база в PR_BASE_SHA) барьер отказал — читается только push-переменная"
fi

# ── Блок 5: приземление ДО-НОРМАТИВНОГО артефакта (F-089-1) ───────────────────────────
# Предмет барьера диапазонный (GM-8), но пути ПРИЗЕМЛИТЬ вердикт, написанный ДО введения
# нормы, не было вовсе: `C-062`/`C-083` спасены с брошенной ветки, в диапазоне они «A», шапки
# нет и для `C-062` быть не может (его база `9a0e48f0` в нашей истории отсутствует — в том и
# был инцидент). Оставались три исхода, все плохие: соврать в шапке, удалить артефакт
# (запрещено `check_protected_artifacts.sh`), выключить барьер. Четвёртый — НАЗВАННОЕ
# исключение с аудит-следом, и его контракт задают GM-28..GM-31.

# GM-28 — «A» без шапки + `ARCHIVED-VERDICT`, называющий ЭТОТ путь ⇒ ПРОХОД (со следом).
mk_repo R; B="$(head_of "$R")"
echo "до-нормативный вердикт без шапки" > "$R/research/critiques/C-105-prenorm.md"
( cd "$R" && git add -A && git commit -q -m "salvage вердикта с брошенной ветки" ) || die "GM-28 salvage"
touch_file "$R" "docs/DESIGN.md" "ARCHIVED-VERDICT: research/critiques/C-105-prenorm.md — написан до введения нормы, честная шапка невозможна"
run_barrier "$R" "$B" && pass "GM-28 до-нормативный артефакт приземляется явным ARCHIVED-VERDICT" \
                      || fail "GM-28 пути приземлить до-нормативный вердикт НЕТ — остаются соврать в шапке, удалить артефакт или выключить барьер"

# GM-29 — токен называет ДРУГОЙ путь ⇒ БЛОК. Бланкетного «всё в диапазоне» быть не должно:
# иначе один токен закрывает собой все остальные артефакты (класс GM-20).
mk_repo R; B="$(head_of "$R")"
echo "до-нормативный вердикт без шапки" > "$R/research/critiques/C-106-prenorm.md"
( cd "$R" && git add -A && git commit -q -m "salvage вердикта с брошенной ветки" ) || die "GM-29 salvage"
touch_file "$R" "docs/DESIGN.md" "ARCHIVED-VERDICT: research/critiques/C-999-другой.md — приземление совсем другого артефакта"
run_barrier "$R" "$B" && fail "GM-29 токен, называющий ДРУГОЙ путь, открыл этот вердикт — исключение бланкетное, первый артефакт закрывает остальные" \
                      || pass "GM-29 исключение пофайловое: токен на другой путь не открывает этот"

# GM-30 — вердикт БЕЗ шапки, лежавший в базе, ИЗМЕНЁН диапазоном + токен на него ⇒ БЛОК.
# Приземление архива («A») и правка вердикта СЕГОДНЯ («M») — разные действия: второе целиком
# под нормой, иначе токен становится вечной отмычкой к любому старому файлу.
mk_repo R
echo "старый вердикт без шапки" > "$R/research/critiques/C-107-old.md"
( cd "$R" && git add -A && git commit -q -m "старый вердикт" ) || die "GM-30 base"
B="$(head_of "$R")"
touch_file "$R" "research/critiques/C-107-old.md" "ARCHIVED-VERDICT: research/critiques/C-107-old.md — правка под видом приземления архива"
run_barrier "$R" "$B" && fail "GM-30 ИЗМЕНЁННЫЙ вердикт прошёл по токену приземления — исключение стало отмычкой к правке любого старого файла" \
                      || pass "GM-30 токен не открывает ПРАВКУ вердикта (только приземление «A»)"

# GM-31 — токен есть, причина ритуальная ⇒ БЛОК. Порог ≥12 символов — тот же, что у
# `FOUNDER-APPROVED` в `check_docs_freeze.sh`: токен без причины неотличим от его отсутствия.
mk_repo R; B="$(head_of "$R")"
echo "до-нормативный вердикт без шапки" > "$R/research/critiques/C-108-prenorm.md"
( cd "$R" && git add -A && git commit -q -m "salvage вердикта с брошенной ветки" ) || die "GM-31 salvage"
touch_file "$R" "docs/DESIGN.md" "ARCHIVED-VERDICT: research/critiques/C-108-prenorm.md — x"
run_barrier "$R" "$B" && fail "GM-31 токен с ритуальной причиной «x» открыл лок — порог причины не проверяется" \
                      || pass "GM-31 причина короче порога не открывает исключение"

# ── Блок 6: РАЗМЕР ТЕЛА как ось (F-10, R-087) ─────────────────────────────────────────
# GM-32 — шапка ЕСТЬ, тело КРУПНЕЕ порога канала ⇒ ПРОХОД.
# Барьер читал шапку пайплайном `printf '%s\n' "$body" | grep -q '<!-- GATE-META'`.
# `grep -q` выходит на первом совпадении и закрывает канал; `printf`, не успевший дописать
# тело, получает EPIPE, и при `set -o pipefail` статусом ПАЙПЛАЙНА становится его ошибка,
# а не успех `grep`. Отсюда ложное «нет шапки GATE-META» при шапке в первой строке.
# Замер R-087 F-10 (реальный контент, 30 прогонов на точку):
#   3 818 B → 0/30 · 8 457 B → 0/30 · 16 409 B → 0/30 · 24 535 B → 18/30 · 32 768 B → 25/30
# Радиус: вердиктов крупнее 16 KB в репозитории 117 из 169 — две трети типового размера.
# Почему набор это пропускал: ВСЕ фикстуры до этой писали тело одной строкой («тело
# вердикта», 26 B), то есть ось РАЗМЕРА не варьировалась ни в одном из 31 сценария.
# Направление отказа было не только fail-closed: при ложном промахе управление уходило в
# ветку `archived_token_for`, и вердикт с СУЩЕСТВУЮЩЕЙ шапкой мог быть объявлен
# до-нормативным — то есть шапка не валидировалась вовсе.
mk_repo R; B="$(head_of "$R")"
{ echo "<!-- GATE-META"
  echo "milestone: M-99"
  echo "audited_repo: a3ka/hft-platform"
  echo "audited_base: $B"
  echo "audited_head: $B"
  echo "verdict: APPROVE"
  echo "-->"
  echo
  awk 'BEGIN { for (i = 0; i < 800; i++) print "строка тела вердикта " i " — набивка выше замеренного порога срабатывания канала" }'
} > "$R/research/critiques/C-107-bigbody.md" || die "GM-32 запись фикстуры"
# ПОРОГ ВЫБРАН ПО НАСЫЩЕНИЮ ОБНАРУЖЕНИЯ, А НЕ ПО ПЕРВОМУ ПРОЯВЛЕНИЮ (C-092 B-5).
# Замер (мутант :186, 30 прогонов на точку): 24 735 B → мутант пойман 19/30;
# 29 085 B → 25/30; 116 085 B → 30/30. Первая точка — подбрасывание монеты: guard,
# стоящий на ней, удостоверяет сценарий, который не состоится в трети прогонов, то есть
# сам порождает флак в обязательном чеке — ровно F-10. Порог назван ВЕЛИЧИНОЙ, а не
# числом строк: набивку можно ужать, контракт останется.
[ "$(wc -c < "$R/research/critiques/C-107-bigbody.md")" -ge 100000 ] \
  || die "GM-32 SETUP не состоялся: тело меньше порога насыщения 100 000 B — обнаружение дефекта было бы вероятностным (19/30 на 24 735 B)"
( cd "$R" && git add -A && git commit -q -m "docs(critic): вердикт с крупным телом" ) || die "GM-32 коммит"
run_barrier "$R" "$B" && pass "GM-32 вердикт с телом >24 KB: шапка найдена, канал не рвётся" \
                      || fail "GM-32 крупное тело даёт ложное «нет шапки GATE-META» — EPIPE под pipefail (F-10)"

# ── Блок 7: оси, которые F-10 обнажил, а GM-32 не закрыл (C-092 B-4) ──────────────────
# Проба пропускала 4 стаба из 7. Причина одна: ось РАЗМЕРА добавлена, ось ПОЛОЖЕНИЯ шапки
# и достижимость ОСТАЛЬНЫХ каналов — нет. Все четыре генератора фикстур начинались с
# `{ echo "<!-- GATE-META"`, то есть шапка ВСЕГДА стояла первой строкой.

# GM-33 — шапка НЕ в первой строке и ДАЛЬШЕ 4096 B ⇒ ПРОХОД. Убивает сразу два стаба:
# S1 («ищу только в первой строке») и S4 («herestring, но тело усечено до 4096»).
# Не гипотетика: в репозитории 4 вердикта из 19 несут шапку не на первой строке, а
# research/critiques/C-070-M-61-rev2.md — на строке 224.
mk_repo R; B="$(head_of "$R")"
{ awk 'BEGIN { for (i = 0; i < 240; i++) print "преамбула вердикта, строка " i " — текст до шапки, как в реальном C-070" }'
  echo "<!-- GATE-META"
  echo "milestone: M-99"
  echo "audited_repo: a3ka/hft-platform"
  echo "audited_base: $B"
  echo "audited_head: $B"
  echo "verdict: APPROVE"
  echo "-->"
  echo "тело после шапки"
} > "$R/research/critiques/C-108-late-header.md" || die "GM-33 запись фикстуры"
OFF="$(grep -bo -m1 '<!-- GATE-META' "$R/research/critiques/C-108-late-header.md" | cut -d: -f1)"
[ "${OFF:-0}" -gt 4096 ] \
  || die "GM-33 SETUP не состоялся: шапка на смещении ${OFF:-0} B (нужно >4096) — стаб «усечение до 4096» не давился бы"
( cd "$R" && git add -A && git commit -q -m "docs(critic): вердикт с поздней шапкой" ) || die "GM-33 коммит"
run_barrier "$R" "$B" && pass "GM-33 шапка на смещении ${OFF} B найдена: ни первой строкой, ни усечением тело не ограничено" \
                      || fail "GM-33 шапка дальше 4096 B не найдена — барьер читает только начало тела (стабы S1/S4)"

# GM-34 — ВЫХОД ИЗ SUBJECT-LOCK на крупном диапазоне (C-092 B-1).
# `ALLOW-SUBJECT-CHANGE` читался из `git log --format=%B <range>` через канал; токен лежит
# в НОВЕЙШЕМ коммите (git log печатает newest-first), поэтому grep совпадал почти сразу и
# рвал git log на хвосте диапазона. Ложное красное — 30/30 на потоке 194 342 B. Это отказ
# ЕДИНСТВЕННОГО законного выхода из лока, тем вернее, чем длиннее ветка.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$B" "C-109-lock.md"
( cd "$R" && for i in $(seq 1 40); do
    printf 'работа %s\n' "$i" >> docs/DESIGN.md
    git add -A
    git commit -q -m "chore: рабочий коммит ${i}$(printf '\n\n%s' "$(awk 'BEGIN{for(j=0;j<60;j++)print "набивка тела коммита для объёма потока git log"}')")"
  done ) || die "GM-34 набивка диапазона"
touch_file "$R" "scripts/verify_M-99.sh" "ALLOW-SUBJECT-CHANGE: правка класса «гейт» после проходного вердикта — намеренно и с аудит-следом"
STREAM="$( cd "$R" && git log --format='%B' "${B}..HEAD" | wc -c )"
[ "${STREAM}" -ge 100000 ] \
  || die "GM-34 SETUP не состоялся: поток git log ${STREAM} B (<100 000) — канал не переполнился бы, сценарий не про то"
run_barrier "$R" "$B" && pass "GM-34 ALLOW-SUBJECT-CHANGE виден на потоке ${STREAM} B: выход из лока не зависит от длины ветки" \
                      || fail "GM-34 токен в новейшем коммите НЕ УВИДЕН на крупном диапазоне — единственный законный выход из subject-lock отказал (C-092 B-1)"

# GM-35 — merge с КРУПНЫМ вердиктом в дереве слияния (C-092 B-2).
# `git show <commit>:<file> | grep -qE M-NN` — вход тот же, что у F-10, и хуже: литерал
# стоит в поле `milestone:` шапки, то есть на первых десятках байт. Чем нормативнее
# вердикт, тем раньше совпадение и тем больше непрочитанного хвоста. Было 27/30 ложных
# «merge без вердикта» (класс TD-105) на фикстуре 116 KB.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$B" "R-101-M-42.md" "research/reviews"
( cd "$R" && { echo "вердикт по M-42"
               awk 'BEGIN { for (i = 0; i < 1100; i++) print "строка тела вердикта " i " — набивка выше порога насыщения канала" }'
             } >> research/reviews/R-101-M-42.md \
  && git add -A && git commit -q --amend --no-edit ) || die "GM-35 набивка вердикта"
SZ="$( cd "$R" && git show HEAD:research/reviews/R-101-M-42.md | wc -c )"
[ "${SZ}" -ge 100000 ] || die "GM-35 SETUP не состоялся: вердикт ${SZ} B (<100 000) — канал не переполнился бы"
( cd "$R" && git checkout -q -b feat-m42 && echo work >> docs/DESIGN.md \
  && git add -A && git commit -q -m "feat(M-42): task #1 — работа" \
  && git checkout -q - \
  && git merge -q --no-ff -m "merge M-42: milestone в main" feat-m42 ) || die "GM-35 merge"
( cd "$R" && git rev-parse -q --verify HEAD^2 >/dev/null ) || die "GM-35: HEAD не merge-коммит"
run_barrier "$R" "$B" && pass "GM-35 крупный вердикт (${SZ} B) в дереве слияния найден: merge не объявлен беспризорным" \
                      || fail "GM-35 ложное «merge без вердикта» на крупном вердикте — класс TD-105 воспроизведён каналом (C-092 B-2)"

# GM-36 — ПУСТОЕ/НЕЧИТАЕМОЕ тело ⇒ БЛОК, а не тихий пропуск (стаб S2).
# Сегодня пустое тело уходит в ветку «нет шапки» → bad, и это ПРАВИЛЬНО (fail-closed:
# «прочитать не смогли» не равно «шапка есть»). Но правильность не держалась ничем: ни
# один сценарий её не проверял, и первый «оптимизирующий» рефакторинг снял бы её молча.
mk_repo R; B="$(head_of "$R")"
: > "$R/research/critiques/C-110-empty.md" || die "GM-36 запись пустого вердикта"
[ ! -s "$R/research/critiques/C-110-empty.md" ] || die "GM-36 SETUP не состоялся: файл не пуст"
( cd "$R" && git add -A && git commit -q -m "docs(critic): пустой вердикт" ) || die "GM-36 коммит"
run_barrier "$R" "$B" && fail "GM-36 пустое тело вердикта ПРОПУЩЕНО — «прочитать не смогли» неотличимо от «шапка есть» (стаб S2)" \
                      || pass "GM-36 пустое/нечитаемое тело блокирует: отказ чтения не выдаётся за наличие шапки"

# GM-37 — КОНЕЧНАЯ ТОЧКА ДИАПАЗОНА: файл, реально СЛИТЫЙ merge-ref'ом из ОБЕИХ сторон, не
# смеет считаться правкой ветки (`C-131` Б-1).
#
# Все фикстуры GM-16b..h собирают merge-ref ЧИСТЫМ слиянием, где гейт-файл правит ОДНА
# сторона: результат равен одному из родителей, комбинированный диф на merge-ref молчит, и
# конечная точка (`HEAD` против `HEAD^2`) неотличима. Давящая форма обязана нести правки ОБЕИХ
# сторон в ОДНОМ файле (`testing.md`, дегенерированный вход п.1 — здесь нужна именно
# ДВУСТОРОННОСТЬ): тогда слитый результат отличается от обоих родителей, `cc` его печатает,
# и барьер, судящий сам merge-ref, объявляет правкой ветки то, чего ветка после вердикта не
# трогала. Это ложный КРАСНЫЙ на прод-форме — ровно тот, ради устранения которого конечная
# точка и переехала на `HEAD^2`.
mk_repo R
( cd "$R" && printf 'l1\nl2\nl3\nl4\nl5\n' > "${GATE_CLASS_FILE}" \
  && git add -A && git commit -q -m "многострочный гейт-файл — общая база обеих сторон" ) \
  || die "GM-37: общая база не собрана"
B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-both ) || die "GM-37: ветка не создана"
( cd "$R" && sed -i '1s/.*/l1-правка-ветки/' "${GATE_CLASS_FILE}" \
  && git add -A && git commit -q -m "правка гейта веткой ДО вердикта — законна" ) \
  || die "GM-37: правка ветки"
AH_PRE="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "${AH_PRE}"
( cd "$R" && git checkout -q - ) || die "GM-37: возврат на базу"
( cd "$R" && sed -i '5s/.*/l5-правка-базы/' "${GATE_CLASS_FILE}" \
  && git add -A && git commit -q -m "правка ТОГО ЖЕ гейт-файла БАЗОЙ (чужой влитый PR)" ) \
  || die "GM-37: правка базы"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git merge -q --no-ff feat-both -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-37: merge-ref не собран (стороны обязаны сливаться ЧИСТО — правки в разных строках)"
setup_ok=1
# (1) merge действительно СЛИЛ обе правки: результат отличается от ОБОИХ родителей
( cd "$R" && git show --format='' --name-only --diff-merges=cc HEAD | grep -qx "${GATE_CLASS_FILE}" ) || setup_ok=0
# (2) ветка ПОСЛЕ своего вердикта гейт-класс НЕ трогала — иначе красный был бы законным
( cd "$R" && [ -z "$(git log --format='' --name-only "${AH_PRE}..HEAD^2" -- "${GATE_CLASS_FILE}")" ] ) || setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-37 SETUP НЕ СОСТОЯЛСЯ: merge-ref не сливает правки ОБЕИХ сторон в одном файле, либо \
ветка тронула гейт после вердикта. Без двусторонности сценарий не давит на конечную точку и \
зелен при ЛЮБОЙ её редакции"
else
  run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" mergeref \
    && pass "GM-37 файл, слитый merge-ref'ом из обеих сторон, не приписан ветке (конечная точка HEAD^2)" \
    || fail "GM-37 ЛОЖНЫЙ КРАСНЫЙ: барьер судит сам merge-ref, и файл, который слияние СЛИЛО \
из обеих сторон, засчитан правкой ветки. Ветка после своего вердикта гейт не трогала — \
конечная точка обязана быть вершиной ВЕТКИ, а не синтезированным merge-ref'ом"
fi

# ── GM-38 / GM-39 — SYNC-MERGE ВНУТРИ ВЕТКИ ПРИ УСТАРЕВШЕЙ БАЗЕ ──────────────────────
# Пара, названная `C-133` как минимально достаточная. Форма рутинная: долгоживущий PR
# подтягивает `main` в себя (`git merge main`), и работа базы становится ДОСТИЖИМОЙ со стороны
# ветки. При устаревшей базе события диапазон `BASE..HEAD^2` втягивает её целиком — спасает
# ТОЛЬКО исключение `^HEAD^1`. Прежние сценарии этой формы не строили: у них ветка либо не
# сливала базу в себя, либо база была свежей, и оба компонента (`^HEAD^1` и диапазон поиска
# токена) оставались незапиннутыми — мутанты переживали пробу 44/44.

# GM-38 — ЛОЖНЫЙ КРАСНЫЙ: правку гейта внесла БАЗА, ветка её лишь втянула sync-merge'ем.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-sync2 ) || die "GM-38: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
( cd "$R" && git checkout -q - ) || die "GM-38: возврат на базу"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта БАЗОЙ после форка (чужой влитый PR)"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git checkout -q feat-sync2 && git merge -q --no-ff "${MAIN_TIP}" \
    -m "sync: git merge main в ветку — рутина долгоживущего PR" ) || die "GM-38: sync-merge"
BRANCH_TIP="$(head_of "$R")"
( cd "$R" && git checkout -q - && git merge -q --no-ff feat-sync2 -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-38: merge-ref не собран"
setup_ok=1
( cd "$R" && [ "$(git rev-parse HEAD^1)" = "${MAIN_TIP}" ] && [ "$(git rev-parse HEAD^2)" = "${BRANCH_TIP}" ] ) || setup_ok=0
# ПРЕДМЕТ: правка базы ДОСТИЖИМА со стороны ветки — иначе исключать нечего и сценарий пуст.
( cd "$R" && [ -n "$(git log --format='' --name-only "${B0}..HEAD^2" -- "${GATE_CLASS_FILE}")" ] ) || setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-38 SETUP НЕ СОСТОЯЛСЯ: sync-merge не втянул правку базы в сторону ветки — компонент \
^HEAD^1 на этой фикстуре не несущий, и она ничего не пиннит"
else
  run_barrier "$R" "${B0}" pull_request "${B0}" mergeref \
    && pass "GM-38 sync-merge при устаревшей базе: правка БАЗЫ не приписана ветке (^HEAD^1 несущий)" \
    || fail "GM-38 ЛОЖНЫЙ КРАСНЫЙ: ветка втянула правку гейта из базы sync-merge'ем, и барьер \
засчитал её работой ветки. Устаревшая база + sync-merge — рутина долгоживущего PR, а не экзотика"
fi

# GM-39 — ЛОЖНОЕ ЗЕЛЁНОЕ (направление хуже): чужой токен из базы, втянутый sync-merge'ем,
# не смеет открывать лок на СОБСТВЕННОЕ нарушение ветки.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-leak2 ) || die "GM-39: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта ВЕТКОЙ после своего APPROVE, СВОЕГО токена нет"
( cd "$R" && git checkout -q - ) || die "GM-39: возврат на базу"
touch_file "$R" "docs/DESIGN.md" "ALLOW-SUBJECT-CHANGE: чужой токен, выданный ДРУГОМУ предмету"
MAIN_TIP="$(head_of "$R")"
( cd "$R" && git checkout -q feat-leak2 && git merge -q --no-ff "${MAIN_TIP}" \
    -m "sync: git merge main в ветку" ) || die "GM-39: sync-merge"
( cd "$R" && git checkout -q - && git merge -q --no-ff feat-leak2 -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-39: merge-ref не собран"
setup_ok=1
# ПРЕДМЕТ: чужой токен ДОСТИЖИМ со стороны ветки (втянут sync-merge'ем) — иначе фикстура
# вырождается в GM-16e, где он лежит только на стороне базы.
( cd "$R" && git log --format='%B' "${B0}..HEAD^2" | grep -q 'ALLOW-SUBJECT-CHANGE' ) || setup_ok=0
# и при этом СОБСТВЕННЫХ токенов у ветки нет
( cd "$R" && git log --format='%B' "${B0}..HEAD^2" "^${MAIN_TIP}" | grep -q 'ALLOW-SUBJECT-CHANGE' ) && setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-39 SETUP НЕ СОСТОЯЛСЯ: чужой токен обязан быть достижим со стороны ветки и \
отсутствовать в её собственных коммитах — иначе сценарий повторяет GM-16e и ничего нового не пиннит"
else
  run_barrier "$R" "${B0}" pull_request "${B0}" mergeref \
    && fail "GM-39 ЛОЖНОЕ ЗЕЛЁНОЕ: чужой ALLOW-SUBJECT-CHANGE, втянутый в ветку sync-merge'ем, \
открыл лок на её СОБСТВЕННОЕ нарушение. Деградация самоусиливающаяся: токен уезжает в main и \
гасит локи всех, кто потом подтянет main к себе" \
    || pass "GM-39 чужой токен, втянутый sync-merge'ем, лок НЕ открывает (диапазон токена сужен тем же исключением)"
fi

# ── GM-40 / GM-41 — ДВА ОСТАВШИХСЯ КОМПОНЕНТА ДИАПАЗОНА (`C-134` Б-1/Б-2) ────────────
# Условие `C-133` требовало: КАЖДЫЙ компонент (`BASE..`, `TOUCH_TIP`, `EXCL_MAIN`, `^ah`) —
# и в `own_touched`, И в `own_bodies`. Круг 5 закрыл `EXCL_MAIN` и `TOUCH_TIP`; `^ah` у
# ПОТРЕБИТЕЛЯ ТОКЕНА и нижняя граница `BASE..` остались незапиннутыми, и оба несущие.

# GM-40 — ТОКЕН ИЗ ДО-ВЕРДИКТНОГО КОММИТА не смеет открывать лок ПОСЛЕ вердикта.
# Форма рутинная для многокругового PR: круг 1 после NOTE законно нёс `ALLOW-SUBJECT-CHANGE`,
# круг 2 получил APPROVE, после которого ветка тронула гейт БЕЗ своего токена. Прежние
# сценарии слепы намеренно: `GM-39` guard требует ОТСУТСТВИЯ собственных токенов у ветки,
# `GM-16e` держит токен на чужой стороне — ни один не кладёт СВОЙ токен ДО `audited_head`.
# Без `^ah` у `own_bodies` любой исторический токен становится вечным ключом от будущего
# замка той же ветки (в живой истории такие есть — `893ead1`).
mk_repo R; B0="$(head_of "$R")"
touch_file "$R" "${GATE_CLASS_FILE}" "ALLOW-SUBJECT-CHANGE: законная правка гейта круга 1 по вердикту NOTE"
AH_PRE="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "${AH_PRE}"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта ПОСЛЕ APPROVE — своего токена НЕТ"
setup_ok=1
# токен обязан лежать ДО audited_head и отсутствовать ПОСЛЕ него
( cd "$R" && git log --format='%B' "${B0}..${AH_PRE}" | grep -q 'ALLOW-SUBJECT-CHANGE' ) || setup_ok=0
( cd "$R" && git log --format='%B' "${AH_PRE}..HEAD" | grep -q 'ALLOW-SUBJECT-CHANGE' ) && setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-40 SETUP НЕ СОСТОЯЛСЯ: токен обязан лежать ДО audited_head и отсутствовать ПОСЛЕ — \
иначе сценарий проверяет законный выход из лока, а не вечный ключ"
else
  run_barrier "$R" "$B0" \
    && fail "GM-40 ЛОЖНОЕ ЗЕЛЁНОЕ: токен, потраченный ДО вердикта, открыл лок ПОСЛЕ него. \
Любой многокруговой PR с одним историческим токеном получает вечный ключ от своего же \
будущего замка" \
    || pass "GM-40 до-вердиктный токен лок НЕ открывает (^ah несущий и у потребителя токена)"
fi

# GM-41 — НИЖНЯЯ ГРАНИЦА `BASE..`: push-в-main после merge не смеет судить чужую историю.
# Джоб gate-meta гоняется и на push (`ci.yml`), якорь там выключен, и `BASE..` — ЕДИНСТВЕННЫЙ
# ограничитель. Прежние сценарии слепы структурно: там, где якорь есть, `^HEAD^1` поглощает
# нижнюю границу; во всех push-сценариях `audited_head` совпадает с базой фикстуры, и `^ah`
# её маскирует. Ни один не разводил `BASE` и `ah` по РАЗНЫМ линиям истории.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-lower ) || die "GM-41: ветка не создана"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B0" "$B0"
( cd "$R" && git checkout -q - ) || die "GM-41: возврат на базу"
touch_file "$R" "scripts/check_m99.sh" "чужой влитый PR правит гейт-класс в main"
MAIN_BEFORE="$(head_of "$R")"
( cd "$R" && git merge -q --no-ff feat-lower -m "merge ветки в main (push-форма)" ) \
  || die "GM-41: merge не собран"
setup_ok=1
# чужая правка гейта обязана лежать НИЖЕ PUSH_BEFORE — то есть вне диапазона push'а
( cd "$R" && git log --format='' --name-only "${B0}..${MAIN_BEFORE}" | grep -qx 'scripts/check_m99.sh' ) || setup_ok=0
( cd "$R" && [ -n "$(git log --format='' --name-only "${MAIN_BEFORE}..HEAD" -- scripts/check_m99.sh)" ] ) && setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-41 SETUP НЕ СОСТОЯЛСЯ: чужая правка гейта обязана лежать НИЖЕ PUSH_BEFORE и \
отсутствовать в самом push-диапазоне — иначе нижняя граница на этой фикстуре не несущая"
else
  run_barrier "$R" "${MAIN_BEFORE}" push \
    && pass "GM-41 push-в-main: чужая история ниже PUSH_BEFORE не судится (BASE.. несущий)" \
    || fail "GM-41 ЛОЖНЫЙ КРАСНЫЙ: push-в-main осудил правку гейта, лежащую НИЖЕ PUSH_BEFORE, \
то есть чужую историю. Это КАЖДЫЙ push после merge ветки, чей форк старше последней чужой \
правки гейт-класса"
fi

# ── Блок 5: ТЕРМИНАЛЬНАЯ ВЕТКА — `TERMINAL-BRANCH-VERDICT` ────────────────────────────
# Токен под случай `R-154` §E: вердикт вынесен над ревизией, которая СУЩЕСТВУЕТ, но лежит на
# ветке, объявленной терминальной решением арбитра. Предком `main` она не станет никогда, и
# `GM-6` держал бы merge предмета, у которого дефекта нет.
#
# СЕМЬ ИЗ ВОСЬМИ СЦЕНАРИЕВ — АНТИ-БЛАНКЕТНЫЕ, и в них весь смысл: токен обязан открывать РОВНО
# одну проверку у РОВНО одного файла. `GM-6` выше остаётся без токена и обязан по-прежнему
# краснеть — иначе новый выход превратился бы в отмену проверки.

# GM-42 — та же side-ветка, что в GM-6, ПЛЮС явный токен ⇒ проход со следом.
mk_repo R; B="$(head_of "$R")"
( cd "$R" && git checkout -q -b side && echo x >> docs/DESIGN.md \
  && git add -A && git commit -q -m side ) || die "GM-42 side"
SIDE="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q - ) || die "GM-42 back"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$SIDE"
touch_file "$R" "docs/DESIGN.md" "TERMINAL-BRANCH-VERDICT: research/critiques/C-999-test.md — ветка объявлена терминальной решением арбитра"
run_barrier "$R" "$B" && pass "GM-42 явный TERMINAL-BRANCH-VERDICT открывает не-предковый audited_head" \
                      || fail "GM-42 легального выхода нет — вердикты на терминальной ветке держат merge вечно"

# GM-43 — АНТИ-БЛАНКЕТ: причина короче порога ⇒ токен НЕ засчитан.
# Порог 12 символов — тот же, что у FOUNDER-APPROVED и ARCHIVED-VERDICT: токен-ритуал
# неотличим от отсутствия токена.
mk_repo R; B="$(head_of "$R")"
( cd "$R" && git checkout -q -b side && echo x >> docs/DESIGN.md \
  && git add -A && git commit -q -m side ) || die "GM-43 side"
SIDE="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q - ) || die "GM-43 back"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$SIDE"
touch_file "$R" "docs/DESIGN.md" "TERMINAL-BRANCH-VERDICT: research/critiques/C-999-test.md — ок"
run_barrier "$R" "$B" && fail "GM-43 токен с причиной короче порога ОТКРЫЛ проверку — ритуал приравнен к обоснованию" \
                      || pass "GM-43 короткая причина токен не засчитывает (порог 12, как у FOUNDER-APPROVED)"

# GM-44 — АНТИ-БЛАНКЕТ: токен называет ДРУГОЙ путь ⇒ на этот файл не действует.
mk_repo R; B="$(head_of "$R")"
( cd "$R" && git checkout -q -b side && echo x >> docs/DESIGN.md \
  && git add -A && git commit -q -m side ) || die "GM-44 side"
SIDE="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q - ) || die "GM-44 back"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$SIDE"
touch_file "$R" "docs/DESIGN.md" "TERMINAL-BRANCH-VERDICT: research/critiques/C-777-другой.md — ветка объявлена терминальной решением арбитра"
run_barrier "$R" "$B" && fail "GM-44 токен на ЧУЖОЙ путь открыл лок — токен стал бланкетным" \
                      || pass "GM-44 токен действует ПОФАЙЛОВО: чужой путь не открывает"

# GM-45 — ГЛАВНЫЙ АНТИ-БЛАНКЕТ: ревизии НЕ СУЩЕСТВУЕТ вовсе (реплей C-062) + токен ⇒ БЛОК.
# Различие принципиальное: «есть, но на мёртвой ветке» — факт истории; «нет вовсе» —
# выдуманная ревизия, ровно то, ради чего барьер построен. Если токен откроет и это, он
# вернёт дыру C-062 под видом послабления.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "9a0e48f09a0e48f09a0e48f09a0e48f09a0e48f0"
touch_file "$R" "docs/DESIGN.md" "TERMINAL-BRANCH-VERDICT: research/critiques/C-999-test.md — ветка объявлена терминальной решением арбитра"
run_barrier "$R" "$B" && fail "GM-45 токен ОТКРЫЛ несуществующую ревизию — класс C-062 вернулся через новый выход" \
                      || pass "GM-45 токен НЕ открывает несуществующий audited_head (C-062 закрыт)"

# GM-46 — АНТИ-БЛАНКЕТ: токен не открывает subject-lock. Ревизия предковая, вердикт проходной,
# после него тронут гейт-класс — блок обязан остаться, токен к этой проверке отношения не имеет.
mk_repo R; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${GATE_CLASS_FILE}" "TERMINAL-BRANCH-VERDICT: research/critiques/C-999-test.md — ветка объявлена терминальной решением арбитра"
run_barrier "$R" "$B" && fail "GM-46 TERMINAL-BRANCH-VERDICT открыл SUBJECT-LOCK — токен вышел за свою проверку" \
                      || pass "GM-46 токен не трогает subject-lock: у каждой проверки свой выход"

# GM-47 — АНТИ-БЛАНКЕТ ЧЕТВЁРТОГО РОДА: ЧУЖОЙ токен из БАЗЫ не открывает проверку.
# Зеркало GM-16e для нового токена. Класс `C-128` Б-2: при устаревшей базе PR в диапазон
# попадают чужие коммиты `main`, и токен, выданный ДРУГОМУ предмету, гасил бы проверку здесь.
# На `ALLOW-SUBJECT-CHANGE` канал был боевым, а не теоретическим.
mk_repo R; B0="$(head_of "$R")"
( cd "$R" && git checkout -q -b feat-leak2 ) || die "GM-47: ветка не создана"
( cd "$R" && git checkout -q -b side2 "$B0" && echo x >> docs/DESIGN.md \
  && git add -A && git commit -q -m side2 ) || die "GM-47: side2"
SIDE2="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q feat-leak2 ) || die "GM-47: возврат на ветку"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B0" "$SIDE2"
( cd "$R" && git checkout -q "$B0" -- . 2>/dev/null; git checkout -q - ) 2>/dev/null || true
( cd "$R" && git checkout -q master 2>/dev/null || git checkout -q main 2>/dev/null ) || die "GM-47: база"
touch_file "$R" "docs/DESIGN.md" "TERMINAL-BRANCH-VERDICT: research/critiques/C-999-test.md — чужой токен, выданный ДРУГОМУ предмету"
MAIN_TIP2="$(head_of "$R")"
( cd "$R" && git merge -q --no-ff feat-leak2 -m "merge-ref: слияние ветки в базу" ) \
  || die "GM-47: merge-ref не собран"
setup_ok=1
( cd "$R" && git log --format='%B' "${B0}..HEAD^1" | grep -q 'TERMINAL-BRANCH-VERDICT' ) || setup_ok=0
( cd "$R" && git log --format='%B' "${B0}..HEAD^2" | grep -q 'TERMINAL-BRANCH-VERDICT' ) && setup_ok=0
if [ "$setup_ok" -ne 1 ]; then
  fail "GM-47 SETUP НЕ СОСТОЯЛСЯ: токен обязан лежать ТОЛЬКО на стороне базы и отсутствовать \
у ветки — иначе сценарий проверяет законный выход, а не утечку"
else
  # БАЗА СОБЫТИЯ УСТАРЕЛА (форма GM-16d): PR_BASE_SHA = B0, а `main` с тех пор уехал вперёд
  # вместе с чужим токеном. ИМЕННО ЭТО делает сценарий различающим: при широком переборе
  # `BASE..HEAD` чужой коммит В ДИАПАЗОНЕ и токен открыл бы проверку; при переборе
  # собственного диапазона сторона `main` исключена. Первая редакция звала барьер с
  # `MAIN_TIP2`, и токен не попадал в диапазон НИ ПРИ КАКОЙ реализации — сценарий был
  # ВАКУУМНЫМ и мутацию «широкий диапазон» не ловил (поймано мутационным контролем автора).
  run_barrier "$R" "${B0}" pull_request "${B0}" mergeref \
    && fail "GM-47 ЧУЖОЙ токен из базы открыл проверку — токен, выданный одному предмету, \
уезжает в main и гасит проверки соседних PR" \
    || pass "GM-47 чужой токен из базы не открывает: перебор сужен до собственного диапазона"
fi

# GM-48 — АНТИ-БЛАНКЕТ ПЯТОГО РОДА (`C-180` Б-1, воспроизведение Д-1): путь, процитированный
# В ПРИЧИНЕ, НЕ открывает проверку файлу, которого токен не называет.
# Канал не теоретический: подсказка самого барьера требует назвать решение арбитра, а файлы
# арбитража судятся ЭТИМ ЖЕ барьером. До фикса такой честный токен молча открывал соседа и
# печатал ЛОЖНЫЙ аудит-след «открыто явным токеном» для файла, который никто не называл.
mk_repo R; B="$(head_of "$R")"
( cd "$R" && git checkout -q -b side48 && echo x >> docs/DESIGN.md \
  && git add -A && git commit -q -m side48 ) || die "GM-48 side"
S48="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q - ) || die "GM-48 back"
# Каталоги вердиктов, которых базовая фикстура не создаёт. Без них `add_verdict` падает —
# и падает ГРОМКО («SETUP НЕ СОСТОЯЛСЯ»), что и поймало первую редакцию этого сценария.
( cd "$R" && mkdir -p research/arbitration research/reviews ) || die "GM-48 dirs"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$S48" "C-100-a.md" "research/critiques"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$S48" "A-018-m68.md" "research/arbitration"
touch_file "$R" "docs/DESIGN.md" "TERMINAL-BRANCH-VERDICT: research/critiques/C-100-a.md — ветка терминальна по решению research/arbitration/A-018-m68.md"
run_barrier "$R" "$B" && fail "GM-48 путь в ПРИЧИНЕ открыл проверку соседнему файлу — токен перестал быть пофайловым, а аудит-след стал ложным" \
                      || pass "GM-48 путь в причине НЕ открывает соседа: сверка идёт равенством первого поля"

# GM-49 — АНТИ-БЛАНКЕТ ШЕСТОГО РОДА (`C-180` Б-1, воспроизведение Д-2): строка из ДВУХ путей
# без причины не проходит порог. До фикса остатком после вычитания своего пути был путь
# СОСЕДА — длиннее двенадцати символов, — и нулевая причина удовлетворяла порог у обоих.
mk_repo R; B="$(head_of "$R")"
( cd "$R" && git checkout -q -b side49 && echo x >> docs/DESIGN.md \
  && git add -A && git commit -q -m side49 ) || die "GM-49 side"
S49="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q - ) || die "GM-49 back"
( cd "$R" && mkdir -p research/arbitration research/reviews ) || die "GM-49 dirs"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$S49" "C-100-a.md" "research/critiques"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$S49" "R-200-b.md" "research/reviews"
touch_file "$R" "docs/DESIGN.md" "TERMINAL-BRANCH-VERDICT: research/critiques/C-100-a.md research/reviews/R-200-b.md"
run_barrier "$R" "$B" && fail "GM-49 строка из двух путей БЕЗ причины прошла порог — путь соседа сработал как обоснование" \
                      || pass "GM-49 два пути без причины порог не проходят: повторы и соседние пути причиной не считаются"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Привязка вердикта к предмету не работает. Пока проба красная, повторимы оба класса:"
  echo "C-062 (вердикт над историей, которой в этом репозитории нет) и TD-105 (молчаливый"
  echo "merge milestone'а без единого артефакта гейта)."
  exit 1
fi

echo "VERDICT: PASS (${PASSED}/${PASSED}) — вердикт привязан к предмету, лок держит, отсутствие наблюдаемо"
