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
# ЧЕГО ПРОБА НЕ ЛОВИТ, названо, а не умолчано. Список ДОПОЛНЕН по `C-183`: первая редакция
# перечисляла три предела, и обоих реальных каналов слома среди них не было — «дыра не
# названа, значит не выбрана, а пропущена».
#   • она судит ЛОКАЛЬНУЮ проводку, а не то, что GitHub реально исполнит: `actions/checkout`
#     воспроизведён его рефспеком, а не самим действием. Расхождение версии checkout'а
#     с этим воспроизведением проба не заметит;
#   • `continue-on-error: true` на шаге БАРЬЕРА (не дотягивания) делает красное барьера
#     незначащим — извлекатель этого не судит, он смотрит на шаг дотягивания;
#   • перенос дотягивания в composite action (`uses:` вместо `run:`) уводит команду из
#     текста джоба: извлекатель вернёт «шага нет» и проба покраснеет — то есть отказ
#     fail-closed, но ЛОЖНЫЙ, и правится осознанно, а не молчанием;
#   • `if:` на уровне ДЖОБА (не шага) проба не смотрит: пропущен был бы весь `gate-meta`,
#     и это поймал бы агрегат `All checks passed`, а не она;
#   • подключение джоба к агрегату она НЕ проверяет — соседний класс (`built-not-wired`);
#     замер `C-183` N-4: `status-check` требует `needs.gate-meta.result == success`, то есть
#     сегодня подключён;
#   • реф события `refs/pull/<N>/merge` назван в переписи носителей, но СВОЕЙ фикстуры
#     не имеет (`C-183` N-3): потребитель всегда чекаутится с `refs/remotes/origin/<ветка>`.
#     По `Р-3` это невыписанный член; признаётся записью, симулятор merge-рефа не строится.
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
#   CR-8  `C-183` Б-1: шаг ПЕРЕНЕСЁН за вызов барьера ⇒ отвергнут (порядок, а не присутствие)
#   CR-9  `C-183` Б-2: `if: ${{ false }}` ПЕРЕД `run:` ⇒ отвергнут
#   CR-10 `C-184` B-1 / `A-027` §3.3: тот же `if:` ПОСЛЕ `run:` ⇒ отвергнут (позиция не спасает)

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
#
# ПИННИТСЯ НЕ ПРИСУТСТВИЕ СТРОКИ, А ЕЁ ДЕЙСТВЕННОСТЬ (`C-183` Б-1/Б-2). Первая редакция
# искала строку в границах джоба и была ЗЕЛЁНОЙ (8/8) в двух случаях функционально мёртвого
# шага, оба предъявлены прогоном:
#   · шаг ПЕРЕНЕСЁН за вызов барьера — в CI барьер исполняется без спас-рефов, ложное
#     красное возвращается, и возвращается МОЛЧА (`mutA_probe_exit=0`);
#   · на шаг навешен `if: ${{ false }}` — CI его пропускает (`mutB_probe_exit=0`).
# Класс тот же, за который `C-182` B-2 отверг предыдущую редакцию, этажом выше: наблюдалось
# наличие ТЕКСТА, а свойство есть ДЕЙСТВИЕ — «клон содержит спас-рефы К МОМЕНТУ вызова
# барьера». Отсюда три условия ниже, и все три — из того же awk-разбора.
#
# Граница джоба расширена до `[A-Za-z0-9_-]` (`C-183` N-2): прежний класс `[a-z][a-z0-9-]*`
# не закрыл бы джоб с заглавной или цифрой в имени, и разбор ушёл бы за пределы `gate-meta`.
job_slice() { # $1=workflow → строки джоба gate-meta с АБСОЛЮТНЫМИ номерами
  awk '
    /^  gate-meta:/ { inj = 1; print NR "\t" $0; next }
    inj && /^  [A-Za-z0-9_-]+:/ { exit }
    inj { print NR "\t" $0 }
  ' "$1"
}

extract_salvage_fetch() { # $1=workflow → команда на stdout; 1, если шаг отсутствует/недейственен
  local slice fetch_ln barrier_ln name_ln cmd
  slice="$(job_slice "$1")"
  [ -n "${slice}" ] || return 1

  fetch_ln="$(printf '%s\n' "${slice}" | awk -F'\t' '$2 ~ /run:[[:space:]]*git fetch/ && $2 ~ /refs\/salvage/ { print $1; exit }')"
  barrier_ln="$(printf '%s\n' "${slice}" | awk -F'\t' '$2 ~ /run:.*check_gate_meta\.sh/ { print $1; exit }')"
  [ -n "${fetch_ln}" ] || return 1
  [ -n "${barrier_ln}" ] || return 1

  # (а) ПОРЯДОК: дотягивание обязано стоять РАНЬШЕ вызова барьера. Иначе барьер видит клон
  #     без спас-рефов, и шаг присутствует, ничего не меняя.
  [ "${fetch_ln}" -lt "${barrier_ln}" ] || return 2

  # (б) БЕЗУСЛОВНОСТЬ: в БЛОКЕ шага дотягивания не должно быть `if:` НИГДЕ.
  #
  # Прежняя редакция смотрела только промежуток `- name:` … `run:` — то есть пиннила
  # ПОЗИЦИЮ ключа, а не его наличие. Порядок ключей в YAML-маппинге семантику Actions не
  # меняет: `if:` ПОСЛЕ `run:` пропускает шаг ровно так же, а извлекатель при этом печатал
  # «стоит ДО барьера и безусловна» (`C-184` B-1: проба давала PASS 10/10 при пропущенном
  # шаге). Группа «позиция `if:`» имела ДВА члена, мутирован был ОДИН — буквальный урок
  # `Р-3`. Решение арбитра `A-027` §3.3: граница блока — от `- name:` шага до следующей
  # строки `- name:`/`- uses:` либо конца джоба; после фикса свойство позиционно-независимо,
  # и CR-9/CR-10 пиннят ОБЕ границы группы.
  name_ln="$(printf '%s\n' "${slice}" | awk -F'\t' -v f="${fetch_ln}" '$1 <= f && $2 ~ /^[[:space:]]*- name:/ { ln = $1 } END { print ln }')"
  [ -n "${name_ln}" ] || return 3
  end_ln="$(printf '%s\n' "${slice}" | awk -F'\t' -v a="${name_ln}" '$1 > a && $2 ~ /^[[:space:]]*- (name|uses):/ { print $1; exit }')"
  [ -n "${end_ln}" ] || end_ln=$(( $(printf '%s\n' "${slice}" | awk -F'\t' 'END { print $1 }') + 1 ))
  if printf '%s\n' "${slice}" | awk -F'\t' -v a="${name_ln}" -v b="${end_ln}" \
       '$1 > a && $1 < b && $2 ~ /^[[:space:]]*if:/ { found = 1 } END { exit !found }'; then
    return 4
  fi

  cmd="$(printf '%s\n' "${slice}" | awk -F'\t' -v f="${fetch_ln}" '$1 == f { sub(/^[[:space:]]*run:[[:space:]]*/, "", $2); print $2 }')"
  [ -n "${cmd}" ] || return 1
  printf '%s' "${cmd}"
}

why_extract_failed() { # $1=код возврата → человекочитаемая причина
  case "$1" in
    1) echo "шага дотягивания refs/salvage (или вызова барьера) в джобе gate-meta НЕТ" ;;
    2) echo "шаг дотягивания стоит ПОСЛЕ вызова барьера — барьер увидит клон без спас-рефов" ;;
    3) echo "у шага дотягивания не найден собственный «- name:» — разбор ненадёжен" ;;
    4) echo "на шаге дотягивания стоит «if:» — CI вправе его пропустить" ;;
    *) echo "неизвестная причина (код $1)" ;;
  esac
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

FETCH_CMD="$(extract_salvage_fetch "${WORKFLOW}")"; EX=$?
if [ ${EX} -ne 0 ] || [ -z "${FETCH_CMD}" ]; then
  fail "CR-0 извлечение: $(why_extract_failed ${EX}) — вердикты терминальных веток дадут ложное красное (C-182 B-2, C-183 Б-1/Б-2)"
  echo; echo "VERDICT: FAIL (${FAILED}) — проводка не несёт ДЕЙСТВЕННОГО шага, от которого зависит исход барьера"
  exit 1
fi
pass "CR-0 извлечение: команда взята ИЗ ci.yml, стоит ДО барьера и безусловна — ${FETCH_CMD}"

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

# ── CR-8 (`C-183` Б-1). Шаг НА МЕСТЕ, но ПЕРЕНЕСЁН за вызов барьера: в CI барьер увидит
# клон без спас-рефов. Первая редакция пробы была здесь ЗЕЛЁНОЙ — присутствие текста её
# устраивало. Мутация перестраивает джоб: блок шага дотягивания вырезается и вставляется
# ПОСЛЕ блока вызова барьера.
python3 - "${WORKFLOW}" "${MUT}/ci-after.yml" <<'PY' 2>/dev/null
import re, sys
src, dst = sys.argv[1], sys.argv[2]
lines = open(src).read().split('\n')
# границы джоба
try:
    start = next(i for i, l in enumerate(lines) if l.startswith('  gate-meta:'))
except StopIteration:
    sys.exit(9)
end = next((i for i in range(start + 1, len(lines))
            if re.match(r'^  [A-Za-z0-9_-]+:', lines[i])), len(lines))
job = lines[start:end]

def step_bounds(pred):
    idx = next((i for i, l in enumerate(job) if pred(l)), None)
    if idx is None:
        return None
    b = max(i for i in range(idx + 1) if re.match(r'^\s*- name:', job[i]))
    e = next((i for i in range(b + 1, len(job)) if re.match(r'^\s*- (name|uses):', job[i])), len(job))
    return b, e

f = step_bounds(lambda l: 'run:' in l and 'git fetch' in l and 'refs/salvage' in l)
g = step_bounds(lambda l: 'run:' in l and 'check_gate_meta.sh' in l)
if not f or not g:
    sys.exit(9)
block = job[f[0]:f[1]]
rest = job[:f[0]] + job[f[1]:]
# позиция барьера пересчитывается ПОСЛЕ выреза
gi = next(i for i, l in enumerate(rest) if 'run:' in l and 'check_gate_meta.sh' in l)
ge = next((i for i in range(gi + 1, len(rest)) if re.match(r'^\s*- (name|uses):', rest[i])), len(rest))
newjob = rest[:ge] + block + rest[ge:]
open(dst, 'w').write('\n'.join(lines[:start] + newjob + lines[end:]))
PY
if [ ! -s "${MUT}/ci-after.yml" ]; then
  die "CR-8 мутация НЕ ПОСТРОИЛАСЬ — сценарий засчитал бы отсутствие правки за срабатывание"
elif cmp -s "${WORKFLOW}" "${MUT}/ci-after.yml"; then
  die "CR-8 мутация НЕ ВНЕСЛАСЬ — файл совпал с оригиналом"
elif extract_salvage_fetch "${MUT}/ci-after.yml" >/dev/null 2>&1; then
  fail "CR-8 шаг перенесён ЗА вызов барьера, а извлекатель это принял — механизм мёртв, проба зелена (C-183 Б-1)"
else
  pass "CR-8 мутация порядка: шаг после барьера ⇒ извлекатель отверг ⇒ проба покраснеет"
fi

# ── CR-9 (`C-183` Б-2). Шаг НА МЕСТЕ и В ПОРЯДКЕ, но условен: `if: ${{ false }}` — CI его
# пропускает молча. Текст присутствует, действия нет.
awk '
  { print }
  /- name: Дотянуть спас-рефы/ { print "        if: ${{ false }}" }
' "${WORKFLOW}" > "${MUT}/ci-if.yml"
if cmp -s "${WORKFLOW}" "${MUT}/ci-if.yml"; then
  die "CR-9 мутация НЕ ВНЕСЛАСЬ — якорь «- name: Дотянуть спас-рефы» не найден"
elif extract_salvage_fetch "${MUT}/ci-if.yml" >/dev/null 2>&1; then
  fail "CR-9 на шаге стоит «if:», а извлекатель это принял — CI вправе шаг пропустить, проба зелена (C-183 Б-2)"
else
  pass "CR-9 мутация условности: «if:» на шаге ⇒ извлекатель отверг ⇒ проба покраснеет"
fi

# ── CR-10 (`A-027` §3.3, вторая граница группы «позиция `if:`»). Тот же ключ, но ПОСЛЕ
# `run:`. До достройки извлекатель его не видел, и проба давала PASS 10/10 при шаге,
# который CI пропускает (`C-184` B-1).
awk '
  { print }
  /run:.*refs\/salvage/ { print "        if: ${{ false }}" }
' "${WORKFLOW}" > "${MUT}/ci-if-after.yml"
if cmp -s "${WORKFLOW}" "${MUT}/ci-if-after.yml"; then
  die "CR-10 мутация НЕ ВНЕСЛАСЬ — якорь строки run: с refs/salvage не найден"
elif extract_salvage_fetch "${MUT}/ci-if-after.yml" >/dev/null 2>&1; then
  fail "CR-10 «if:» стоит ПОСЛЕ «run:», а извлекатель это принял — шаг пропускается молча, проба зелена (C-184 B-1)"
else
  pass "CR-10 мутация позиции: «if:» после «run:» ⇒ извлекатель отверг ⇒ свойство позиционно-независимо"
fi

echo
TOTAL=$((PASSED + FAILED))
if [ "${FAILED}" -eq 0 ]; then
  echo "VERDICT: PASS (${PASSED}/${TOTAL}) — рефспек клона судится как часть гейта; отсутствие шага наблюдаемо"
  exit 0
fi
echo "VERDICT: FAIL (${FAILED} из ${TOTAL})"
exit 1
