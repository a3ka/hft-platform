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

run_barrier() { # $1=repo $2=before-sha [$3=EVENT_NAME] [$4=PR_BASE_SHA]
  need_fixture "$1" run_barrier
  local st ev="${3:-push}" pb="${4-$2}"
  ( cd "$1" && EVENT_NAME="$ev" PUSH_BEFORE="$2" PR_BASE_SHA="$pb" bash "${BARRIER}" >/dev/null 2>&1 )
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
echo "── сценарии GM-1..GM-31; GM-16 СОЖЖЁН (спека M-60b §4, шапка выше) ──"
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
  run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" \
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
run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" \
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
  run_barrier "$R" "${B0}" pull_request "${B0}" \
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
  run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" \
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
  run_barrier "$R" "${MAIN_TIP}" pull_request "${MAIN_TIP}" \
    && fail "GM-16f контрабанда в merge-коммите прошла: правка гейт-класса, вложенная в merge, \
невидима барьеру — канал НАМЕРЕННОГО обхода в две команды, без всякого конфликта" \
    || pass "GM-16f контрабанда в merge-коммите заблокирована (комбинированный диф её видит)"
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

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Привязка вердикта к предмету не работает. Пока проба красная, повторимы оба класса:"
  echo "C-062 (вердикт над историей, которой в этом репозитории нет) и TD-105 (молчаливый"
  echo "merge milestone'а без единого артефакта гейта)."
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/${PASSED}) — вердикт привязан к предмету, лок держит, отсутствие наблюдаемо"
