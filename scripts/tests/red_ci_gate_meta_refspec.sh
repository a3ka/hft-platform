#!/usr/bin/env bash
# Проба ПРОВОДКИ gate-meta: рефспек клона — часть гейта, а не деталь окружения.
#
# ЗАЧЕМ — находка `C-182` B-2, принятая целиком.
#
# `scripts/tests/red_gate_meta.sh` ГЕРМЕТИЧНА: она строит свой репозиторий и зовёт барьер
# напрямую. Поэтому она не видит `.github/workflows/ci.yml` вовсе — и остаётся зелёной (56/56),
# даже если шаг дотягивания спас-рефов из проводки УДАЛИТЬ. То есть у механизма, от которого
# зависит исход барьера, не было наблюдателя ОТСУТСТВИЯ (`testing.md`, целостность гейта,
# свойство 4; `oracle-blindness-class` Р-2: оракул обязан судить предмет ПОД действующим
# ограничением, а ограничение здесь — рефспек, которым собран клон).
#
# ЧТО ЭТА ПРОБА ДЕЛАЕТ ИНАЧЕ. Она НЕ копирует команду дотягивания в свой текст — она
# ИЗВЛЕКАЕТ её из джоба `gate-meta` файла `.github/workflows/ci.yml` и исполняет ровно то,
# что там написано. Литерал, живущий отдельно от предмета, врёт: он остался бы зелёным
# после удаления шага. Отсюда свойство, ради которого проба существует:
#
#     удалить шаг из ci.yml  ⇒  извлечение не находит команду  ⇒  проба КРАСНАЯ.
#
# Мутационный контроль этого свойства — сценарий CR-7, он гоняет извлекатель против
# КОПИИ workflow с вырезанным шагом и требует, чтобы отсутствие было замечено.
#
# ЧЕГО ПРОБА НЕ ЛОВИТ, названо, а не умолчано:
#   • она судит ЛОКАЛЬНУЮ проводку, а не то, что GitHub реально исполнит: `actions/checkout`
#     воспроизведён его рефспеком, а не самим действием. Расхождение версии checkout'а
#     с этим воспроизведением проба не заметит;
#   • она не проверяет, что джоб `gate-meta` вообще подключён к агрегату `All checks passed`
#     — это соседний класс (`built-not-wired`), его держит `check_context_budgets`/ревью;
#   • переименование шага её не ломает (ищется команда, не имя), а вот перенос дотягивания
#     в другой джоб — сломает, и это намеренно: барьер зовут здесь.
#
# СЦЕНАРИИ (счёт печатает сама проба — литерал числа в шапке запрещён, он уже врал в соседях):
#   CR-1  позитивный контроль: вердикт в родной линии ⇒ барьер PASS
#   CR-2  наблюдение ОТСУТСТВИЯ: терминальная ревизия ТОЛЬКО под refs/salvage, клон собран
#         БЕЗ шага ⇒ барьер FAIL «не существует» (шаг несущий — вот доказательство)
#   CR-3  та же фикстура, клон собран С командой ИЗ ci.yml ⇒ барьер PASS
#   CR-4  `C-182` B-1, носитель refs/salvage: сирота без общего предка + токен ⇒ FAIL
#   CR-5  `C-182` B-1, носитель refs/heads (его вердикт не проверял): тот же сирота,
#         клон БЕЗ шага ⇒ FAIL. Дыра жила в токене, а не в дотягивании — пиннится здесь
#   CR-6  `C-062`: выдуманная ревизия при дотянутых спас-рефах ⇒ FAIL
#   CR-7  мутация проводки: шаг вырезан из копии ci.yml ⇒ извлекатель обязан это заметить

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BARRIER="${ROOT}/scripts/check_gate_meta.sh"
WORKFLOW="${ROOT}/.github/workflows/ci.yml"

PASSED=0; FAILED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

# Реестр фикстур — в ФАЙЛЕ, а не в переменной: подоболочка `$( )` теряет переменную, и
# каталоги остаются навсегда. Класс, давший 10 400 каталогов в /tmp и диск на 100 %.
FIXTURE_REG="$(mktemp /tmp/red-ciref-reg-XXXXXX)" || die mktemp
reg_fixture() { printf '%s\n' "$1" >> "${FIXTURE_REG}"; }
cleanup_fixtures() {
  [ -n "${KEEP_FIXTURES:-}" ] && { echo "фикстуры оставлены (KEEP_FIXTURES): ${FIXTURE_REG}"; return 0; }
  while IFS= read -r d; do
    case "$d" in /tmp/red-ciref-*) [ -d "$d" ] && rm -rf "$d" ;; esac
  done < "${FIXTURE_REG}" 2>/dev/null
  rm -f "${FIXTURE_REG}"
}
trap cleanup_fixtures EXIT

[ -x "${BARRIER}" ] || [ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}"
[ -f "${WORKFLOW}" ] || die "проводки нет: ${WORKFLOW}"

# ── ИЗВЛЕКАТЕЛЬ. Команда берётся ИЗ джоба gate-meta, а не из текста этой пробы.
# Границы джоба — от «^  gate-meta:» до следующего заголовка джоба на той же отбивке.
extract_salvage_fetch() { # $1=путь к workflow → команда на stdout; 1, если не нашлось
  awk '
    /^  gate-meta:/            { inj = 1; next }
    inj && /^  [a-z][a-z0-9-]*:/ { exit }
    inj && /run:[[:space:]]*git fetch/ && /refs\/salvage/ {
      sub(/^[[:space:]]*run:[[:space:]]*/, ""); print; found = 1; exit
    }
    END { if (!found) exit 1 }
  ' "$1"
}

# ── ФИКСТУРА: bare-origin + линии истории.
# Слаг вычисляется так же, как его выведет барьер из пути origin: <родитель>/<имя без .git>.
build_origin() { # $1=имя переменной для каталога стенда
  local d bare work base
  d="$(mktemp -d /tmp/red-ciref-XXXXXX)" || die mktemp
  reg_fixture "$d"
  bare="$d/fixorigin.git"
  ( cd "$d" && git init -q --bare fixorigin.git ) || die "bare origin"
  work="$d/work"
  ( git init -q "$work" \
    && cd "$work" \
    && git config user.email a@b.c && git config user.name t \
    && git remote add origin "$bare" \
    && mkdir -p research/critiques research/reviews scripts \
    && echo base > research/reviews/.keep \
    && git add -A && git commit -q -m base \
    && git branch -M main \
    && git push -q origin main ) || die "инициализация фикстуры"
  printf -v "$1" '%s' "$d"
}

slug_of_fixture() { printf 'red-ciref-%s/fixorigin' "$(basename "$1" | sed 's/^red-ciref-//')"; }

# Вердикт кладётся в РАБОЧУЮ линию и пушится как ветка предмета.
add_verdict_branch() { # $1=стенд $2=audited_head $3=имя файла $4=токен(да/нет) $5=ветка
  local d="$1" work="$1/work" slug
  slug="$(git -C "$work" remote get-url origin)"
  slug="$(basename "$(dirname "$slug")")/$(basename "$slug" .git)"
  ( cd "$work" && git checkout -q main \
    && { echo "<!-- GATE-META"
         echo "milestone: M-99"
         echo "audited_repo: ${slug}"
         echo "audited_base: $(git rev-parse main)"
         echo "audited_head: $2"
         echo "verdict: REJECT"
         echo "-->"
         echo "тело вердикта"; } > "research/critiques/$3" \
    && git add "research/critiques/$3" ) || die "запись вердикта $3"
  if [ "$4" = "да" ]; then
    ( cd "$work" && git commit -q -F - -- "research/critiques/$3" <<EOF
test: вердикт $3

TERMINAL-BRANCH-VERDICT: research/critiques/$3 — ветка объявлена терминальной решением арбитра
EOF
    ) || die "коммит вердикта с токеном"
  else
    ( cd "$work" && git commit -q -m "test: вердикт $3" -- "research/critiques/$3" ) || die "коммит вердикта"
  fi
  ( cd "$work" && git push -qf origin "HEAD:refs/heads/$5" ) || die "push ветки предмета"
}

# Потребитель собирается РЕФСПЕКОМ actions/checkout; шаг дотягивания — по требованию.
make_consumer() { # $1=имя переменной $2=стенд $3=ветка $4=команда дотягивания или ""
  local d out
  d="$(mktemp -d /tmp/red-ciref-XXXXXX)" || die mktemp
  reg_fixture "$d"
  ( cd "$d" && git init -q \
    && git remote add origin "$2/fixorigin.git" \
    && git fetch -q --no-tags --prune origin '+refs/heads/*:refs/remotes/origin/*' \
    && git checkout -q --detach "refs/remotes/origin/$3" ) || die "сборка потребителя"
  if [ -n "$4" ]; then
    out="$( cd "$d" && eval "$4" 2>&1 )" || die "команда дотягивания из ci.yml упала: ${out}"
  fi
  printf -v "$1" '%s' "$d"
}

run_barrier() { # $1=каталог потребителя $2=база → код возврата барьера
  local st base
  [ -n "${1:-}" ] && [ -d "${1:-}" ] || die "run_barrier получил пустой путь — команда ушла бы в РАБОЧЕЕ дерево"
  base="$2"
  ( cd "$1" && EVENT_NAME=pull_request PUSH_BEFORE= PR_BASE_SHA="$base" \
      GITHUB_SHA="$(git rev-parse HEAD)" bash "${BARRIER}" >/dev/null 2>&1 )
  st=$?
  case $st in
    126|127) die "барьер вернул ${st} — отказ СРЕДЫ, а не гейта; сценарий засчитал бы падение за срабатывание" ;;
  esac
  return $st
}

# ВЫВОД СНАЧАЛА В ПЕРЕМЕННУЮ, и это не стиль. При `set -o pipefail` конвейер
# «барьер | grep» возвращает код БАРЬЕРА (1 на FAIL), а не находку grep'а — и каждый
# сценарий «отказ по нужной причине» докладывал бы «отказ есть, но не по той». Поймано
# первым же прогоном этой пробы на четырёх сценариях из восьми.
barrier_says() { # $1=каталог $2=база $3=подстрока → 0, если найдена
  local out
  out="$( cd "$1" && EVENT_NAME=pull_request PUSH_BEFORE= PR_BASE_SHA="$2" \
      GITHUB_SHA="$(git rev-parse HEAD)" bash "${BARRIER}" 2>&1 )"
  printf '%s' "${out}" | grep -q -- "$3"
}

echo "── ПРОВОДКА gate-meta: рефспек клона как часть гейта"

FETCH_CMD="$(extract_salvage_fetch "${WORKFLOW}")" || FETCH_CMD=""
if [ -z "${FETCH_CMD}" ]; then
  fail "CR-0 извлечение: в джобе gate-meta файла ci.yml НЕТ шага дотягивания refs/salvage — вердикты терминальных веток дадут ложное красное (C-182 B-2)"
  echo; echo "VERDICT: FAIL (${FAILED}) — проводка не несёт шага, от которого зависит исход барьера"
  exit 1
fi
pass "CR-0 извлечение: команда взята ИЗ ci.yml — ${FETCH_CMD}"

build_origin STAND
BASE="$( cd "$STAND/work" && git rev-parse main )"

# ── Терминальная линия: РОДНАЯ (общий предок с main есть), носитель — только refs/salvage.
( cd "$STAND/work" && git checkout -q -b terminal-line main \
  && echo терминальная > research/reviews/terminal.txt \
  && git add research/reviews/terminal.txt \
  && git commit -q -m "terminal: работа, запрещённая к merge'у" ) || die "терминальная линия"
TERM_SHA="$( cd "$STAND/work" && git rev-parse HEAD )"
( cd "$STAND/work" && git push -q origin "${TERM_SHA}:refs/salvage/terminal-line" ) || die "push спас-рефа"

# ── Сиротская линия: общего предка НЕТ.
( cd "$STAND/work" && git checkout -q --orphan orphan-line \
  && git rm -rqf . 2>/dev/null; true )
( cd "$STAND/work" && echo чужое > unrelated.txt && git add unrelated.txt \
  && git commit -q -m "orphan: чужая линия истории" ) || die "сиротская линия"
ORPHAN_SHA="$( cd "$STAND/work" && git rev-parse HEAD )"

# Страж setup'а: линии обязаны РАЗЛИЧАТЬСЯ по родству, иначе сценарии судят одно и то же.
( cd "$STAND/work" && git merge-base "${TERM_SHA}" main >/dev/null 2>&1 ) \
  || die "терминальная линия НЕ родная — фикстура не воспроизводит предмет"
( cd "$STAND/work" && git merge-base "${ORPHAN_SHA}" main >/dev/null 2>&1 ) \
  && die "сиротская линия оказалась роднёй — фикстура не воспроизводит предмет"

# ── CR-1 позитивный контроль: вердикт в родной линии, ревизия — предок.
add_verdict_branch "$STAND" "${BASE}" "C-901-inline.md" нет pr-inline
make_consumer C1 "$STAND" pr-inline ""
if run_barrier "$C1" "${BASE}"; then
  pass "CR-1 позитивный контроль: вердикт родной линии ⇒ PASS"
else
  fail "CR-1 позитивный контроль КРАСНЫЙ — стенд негоден, остальные сценарии зелены по чужой причине"
fi

# ── CR-2 наблюдение ОТСУТСТВИЯ шага: ревизия только под refs/salvage, клон без дотягивания.
add_verdict_branch "$STAND" "${TERM_SHA}" "C-902-terminal.md" да pr-terminal
make_consumer C2 "$STAND" pr-terminal ""
if run_barrier "$C2" "${BASE}"; then
  fail "CR-2 БЕЗ шага барьер дал PASS — значит шаг ничего не несёт и проба его отсутствие не заметит"
elif barrier_says "$C2" "${BASE}" "не существует в этой истории"; then
  pass "CR-2 без шага: FAIL «не существует» — шаг несущий, его отсутствие наблюдаемо"
else
  fail "CR-2 без шага барьер отказал, но НЕ по причине отсутствия ревизии — сценарий судит не то"
fi

# ── CR-3 та же фикстура, клон собран КОМАНДОЙ ИЗ ci.yml.
make_consumer C3 "$STAND" pr-terminal "${FETCH_CMD}"
if run_barrier "$C3" "${BASE}"; then
  pass "CR-3 с шагом из ci.yml: PASS — ложное красное снято"
else
  fail "CR-3 с шагом из ci.yml барьер по-прежнему красный — ремонт не работает"
fi

# ── CR-4 C-182 B-1, носитель refs/salvage: сирота + токен обязаны быть отвергнуты.
( cd "$STAND/work" && git push -q origin "${ORPHAN_SHA}:refs/salvage/orphan-carrier" ) || die "push сироты в спас-реф"
add_verdict_branch "$STAND" "${ORPHAN_SHA}" "C-903-orphan-salvage.md" да pr-orphan-salvage
make_consumer C4 "$STAND" pr-orphan-salvage "${FETCH_CMD}"
if run_barrier "$C4" "${BASE}"; then
  fail "CR-4 сирота под спас-рефом + токен дали PASS — C-182 B-1 жив"
elif barrier_says "$C4" "${BASE}" "НЕ имеет общего предка"; then
  pass "CR-4 сирота под спас-рефом + токен: FAIL по РОДСТВУ — C-182 B-1 закрыт"
else
  fail "CR-4 отказ есть, но не по родству — проверка родства не сработала"
fi

# ── CR-5 тот же сирота, носитель refs/heads, БЕЗ дотягивания: дыра жила в токене.
( cd "$STAND/work" && git push -q origin "${ORPHAN_SHA}:refs/heads/orphan-branch" ) || die "push сироты в ветку"
add_verdict_branch "$STAND" "${ORPHAN_SHA}" "C-904-orphan-heads.md" да pr-orphan-heads
make_consumer C5 "$STAND" pr-orphan-heads ""
if run_barrier "$C5" "${BASE}"; then
  fail "CR-5 сирота под ОБЫЧНОЙ веткой + токен дали PASS — дыра токена открыта и без спас-рефов"
elif barrier_says "$C5" "${BASE}" "НЕ имеет общего предка"; then
  pass "CR-5 сирота под обычной веткой: FAIL по РОДСТВУ — носитель значения не имеет"
else
  fail "CR-5 отказ есть, но не по родству"
fi

# ── CR-6 C-062: выдуманная ревизия при дотянутых спас-рефах.
add_verdict_branch "$STAND" "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" "C-905-fake.md" да pr-fake
make_consumer C6 "$STAND" pr-fake "${FETCH_CMD}"
if run_barrier "$C6" "${BASE}"; then
  fail "CR-6 выдуманная ревизия прошла — класс C-062 открыт"
elif barrier_says "$C6" "${BASE}" "не существует в этой истории"; then
  pass "CR-6 выдуманная ревизия: FAIL «не существует» — C-062 закрыт"
else
  fail "CR-6 отказ есть, но не по отсутствию ревизии"
fi

# ── CR-7 мутационный контроль ПРОВОДКИ: шаг вырезан из копии ci.yml.
MUT="$(mktemp -d /tmp/red-ciref-XXXXXX)" || die mktemp
reg_fixture "$MUT"
grep -v "refs/salvage" "${WORKFLOW}" > "${MUT}/ci.yml" || die "копия workflow"
if cmp -s "${WORKFLOW}" "${MUT}/ci.yml"; then
  die "мутация НЕ ВНЕСЛАСЬ — копия совпала с оригиналом; сценарий засчитал бы отсутствие правки за срабатывание"
fi
if extract_salvage_fetch "${MUT}/ci.yml" >/dev/null 2>&1; then
  fail "CR-7 извлекатель нашёл шаг в проводке, где его вырезали — удаление шага останется незамеченным"
else
  pass "CR-7 мутация проводки: шаг вырезан ⇒ извлекатель это заметил ⇒ проба покраснеет"
fi

echo
TOTAL=$((PASSED + FAILED))
if [ "${FAILED}" -eq 0 ]; then
  echo "VERDICT: PASS (${PASSED}/${TOTAL}) — рефспек клона судится как часть гейта; отсутствие шага наблюдаемо"
  exit 0
fi
echo "VERDICT: FAIL (${FAILED} из ${TOTAL})"
exit 1
