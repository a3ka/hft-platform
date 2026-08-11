#!/usr/bin/env bash
# Проба механизма выдачи номеров артефактов — M-61 (`TD-111`).
#
# ИНВАРИАНТ ОТ РЕЗУЛЬТАТА И ОТ ДИАПАЗОНА (спека §4.1):
#   ни один коммит проверяемого диапазона не вводит номер, обозначающий ВТОРОЙ ПРЕДМЕТ —
#   ни созданием артефакта, ни переименованием, ни записью в `TECH-DEBT.md`; «второй»
#   считается относительно объединения `refs/remotes/origin/*` и `refs/heads/*`.
#
# ПОЧЕМУ ДИАПАЗОН, А НЕ ВСЯ ИСТОРИЯ (`C-069` F-1): пять коллизий уже существуют и
# переименованию не подлежат (на них ссылаются вердикты и шапки sacred-оракулов).
# Абсолютный инвариант дал бы барьер, красный НАВСЕГДА, либо grandfather-список — то есть
# «мешок случаев», запрещённый `A-005` §2 поправка 1.
#
# ШЕСТЬ ОСЕЙ (спека §4.2) — члены предложения-инварианта, а не список случаев:
#   1 класс артефакта · 2 носитель номера · 3 область поиска занятости ·
#   4 способ занятия · 5 проверяемый срез · 6 носитель идентичности предмета
#
# ТРИ СВЕРКИ, каждая в обе стороны: манифест ⇄ исполнение (по именам) · манифест ⇄ таблица
# §4.2 спеки (по составу троек) · у каждой оси есть легитимный сценарий (§4.3).
#
# ПРАВИЛО ОСТАНОВКИ (§4.4): находка о полноте называет ОСЬ и ЗНАЧЕНИЕ либо предъявляет
# седьмую ось структурно. Иначе категория (iii) — NOTE, merge не блокирует.
#
# ДВА ПРЕДМЕТА ПРОВЕРКИ: барьер `check_artifact_ids.sh` (оси 1,2,4,5,6) и аллокатор
# `next_artifact_id.sh` (ось 3). У них разные контракты, и проба обязана давить на оба.
#
# ЛОКАЛЬНЫЕ ВЕТКИ В ФИКСТУРЕ — НЕ УСЛОВНОСТЬ (спека §3.1): удалённых ref'ов в тестовом
# репозитории нет вовсе, поэтому механизм обязан читать объединение origin ∪ refs/heads.
# Реализация, знающая только `refs/remotes`, непроверяема; знающая только `refs/heads` —
# бесполезна на проде.

set -uo pipefail

SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/check_artifact_ids.sh}"
ALLOC="${ALLOC:-${ROOT}/scripts/next_artifact_id.sh}"
SPEC="${SPEC:-${ROOT}/milestones/M-61-artifact-ids.md}"
ZERO=0000000000000000000000000000000000000000

# ─── МАНИФЕСТ: имя|ось|вид|значение ──────────────────────────────────────────────────
# Значения совпадают ПОСИМВОЛЬНО с атомами таблицы §4.2 спеки (сверка 2).
# Одна фикстура вправе нести несколько claim'ов — каждый отдельной строкой.
#
# АТРИБУЦИЯ МУТАНТА — по ОБЪЯВЛЕННОМУ KILL-SET'у, а не по принадлежности оси (круг 4, ревизия
# после адверсарной проверки). Первая редакция стража требовала «нет падений на сценариях ЧУЖИХ
# осей» и была ослаблена собственным лекарством: чтобы оправдать `absolute`, фикстуре `L4MOD`
# добавили claim по оси 5 — и тем открыли всю ось 5 всякому мутанту, сломанному сверх своей оси
# (проверено: `rangeblind` с добавленным дефектом оси 4 проходил батарею). Кроме того у условия
# не было НИЖНЕЙ границы: мутант, не уронивший НИ ОДНОГО сценария (например, уронивший setup
# пробы), проходил как «красен по своей оси» — то есть страж не отличал «дыра закреплена» от
# «проба не запустилась». Обе щели закрывает РАВЕНСТВО множеств: §4.5 объявляет поимённо, какие
# сценарии мутант обязан уронить, и наблюдаемое множество обязано совпасть с объявленным —
# ни больше (сломан сверх своей оси), ни меньше (дыра не закреплена).
MANIFEST="
B1TD|1|V|TD
B1TD|2|V|запись в TECH-DEBT.md
B1TD|4|V|новая запись в TECH-DEBT.md
B1R|1|V|R
B1R|2|V|имя файла
B1R|4|V|новый файл
B1R|5|V|новая коллизия внесена диапазоном
B1C|1|V|C
B1A|1|V|A
B1M|1|V|M
L1X|1|L|неизвестный префикс вне зоны
L2TXT|2|L|упоминание в тексте
N3LOCAL|3|V|только своё дерево
N3HEAD|3|V|только origin, локальный head пропущен
N3ORIG|3|V|только refs/heads, origin пропущен
B3ORIG|3|V|только refs/heads, origin пропущен
N3TD|3|V|только своё дерево
N3TD|1|V|TD
N3NOORIG|3|V|origin недоступен
L3OK|3|L|origin ∪ refs/heads
B4REN|4|V|переименование в занятый номер
B4Q|4|V|имя, требующее квотирования
L4DEL|4|L|удаление артефакта
L4MOD|4|L|правка существующего артефакта
B5THIRD|5|V|усиление существующей коллизии
B5MID|5|V|коллизия в не-вершинном коммите диапазона
B5BASE|5|V|недостоверная база
B5PR|5|V|срез, заданный событием pull_request
B5NOBASE|5|V|база отсутствует в истории
B5NOTANC|5|V|база не предок HEAD
L5PRE|5|L|предсуществующая коллизия вне диапазона
L5FIX|5|L|ошибка исправлена перенумерацией внутри диапазона
B6SLUG|6|V|разные слаги под одним номером
B6NOSLUG|6|V|slugless против слага
B6HDR|6|V|шапка «Предмет» расходится
B6SPLIT|6|V|split-суффикс без совпавшего предмета
L6SLUG|6|L|совпал слаг
L6HDR|6|L|совпала шапка «Предмет»/«Контекст»
L6CONT|6|L|предмет со строки-продолжения
L6SPLIT|6|L|split-суффикс с совпавшим предметом
L6REV|6|L|ревизия того же предмета
"

FAILED=0; PASSED=0; EXECUTED=""
# Реестр фикстур — ФАЙЛ, а не переменная. Причина поведенческая, не стилистическая:
# `R="$(mk_repo …)"` исполняет функцию в ПОДОБОЛОЧКЕ, поэтому присваивание `FIXTURES=…`
# внутри неё в родителя не возвращается — `cleanup()` перебирал пустую строку и не удалял
# НИЧЕГО. Механизм был объявлен (`cleanup` + `trap` + ручка `KEEP_FIXTURES`) и мёртв: замер
# 2026-08-11 — 37 547 каталогов `/tmp/red-ids-*` при диске 88 %, накопленных с 08.08. Запись
# в файл переживает подоболочку, поэтому реестр ведётся дескриптором, а не переменной.
FIXTURES_REG="$(mktemp /tmp/red-ids-reg-XXXXXX)" || { echo "не создан реестр фикстур" >&2; exit 1; }
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }
mark() { EXECUTED="${EXECUTED}$1
"; }

cleanup() {
  [ "${KEEP_FIXTURES:-0}" = "1" ] && { echo "фикстуры сохранены: ${FIXTURES_REG}" >&2; return; }
  local d
  while IFS= read -r d; do
    [ -n "$d" ] && [ -d "$d" ] && case "$d" in /tmp/red-ids-*) rm -rf "$d";; esac
  done < "${FIXTURES_REG}"
  rm -f "${FIXTURES_REG}"
}
trap cleanup EXIT

mk_repo() {
  local d; d="$(mktemp -d "/tmp/red-ids-$1-XXXXXX")" || die mktemp
  printf '%s\n' "$d" >> "${FIXTURES_REG}"
  ( cd "$d" && git init -q -b main && git config user.email a@b.c && git config user.name t \
    && mkdir -p research/reviews research/critiques research/arbitration milestones docs \
    && printf '# TECH-DEBT\n\n' > TECH-DEBT.md \
    && echo base > docs/base.md && git add -A && git commit -q -m base ) || die "фикстура $1"
  echo "$d"
}

# Артефакт с необязательной шапкой `Предмет:`; $4 — тело шапки целиком (может быть пустым).
art() { ( cd "$1" && mkdir -p "$(dirname "$2")" && { printf '# %s\n\n' "$(basename "$2")"
        [ -n "${3:-}" ] && printf '%s\n\n' "$3"; printf 'тело\n'; } > "$2" ) || die "art $2"; }
commit_all() { ( cd "$1" && git add -A && git commit -q -m "${2:-правка}" ) || die "commit $2"; }
td_entry() { ( cd "$1" && printf -- '- **%s** `%s`\n' "$2" "$3" >> TECH-DEBT.md ) || die td; }

# ПРОД-ФОРМА ВЫЗОВА, а не удобная (testing.md, целостность гейта, свойство 1: «гейт,
# проверенный не тем вызовом, каким его зовёт прод, не проверен»). CI триггерится ДВУМЯ
# событиями (ci.yml: on.pull_request и on.push) и передаёт базу РАЗНЫМИ переменными:
# PUSH_BEFORE=github.event.before либо PR_BASE_SHA=github.event.pull_request.base.sha.
# Прежняя редакция клала одно значение в ОБЕ переменные и всегда звала с EVENT_NAME=push —
# то есть не могла различить, из какой переменной барьер читает базу: реализация, целиком
# выключенная на pull_request, проходила пробу 30/30. Теперь каждое событие подставляет
# ТОЛЬКО свою переменную, а чужую оставляет пустой.
run_check() {  # $1=repo $2=база $3=событие (push|pull_request)
  if [ "${3:-push}" = pull_request ]; then
    ( cd "$1" && EVENT_NAME=pull_request PUSH_BEFORE="" PR_BASE_SHA="$2" bash "${BARRIER}" >/dev/null 2>&1 )
  else
    ( cd "$1" && EVENT_NAME=push PUSH_BEFORE="$2" PR_BASE_SHA="" bash "${BARRIER}" >/dev/null 2>&1 )
  fi; }
run_alloc() { ( cd "$1" && bash "${ALLOC}" "$2" 2>/dev/null ); }

# Setup-guard НА КАЖДЫЙ сценарий — на все 31 (`testing.md`, целостность гейта, свойство 3: «проба, молча
# тестирующая не тот сценарий, — плацебо самой себя»). Фикстуры строятся цепочками вида
# `( cd … && … ) && commit_all`: если подоболочка молча откажет, коммита не будет, диапазон
# окажется ПУСТ, барьер выйдет нулём по раннему `[ -z "$IN" ]`, и expect_allow напечатает
# PASS, не проверив ничего. Диапазон обязан содержать хотя бы один коммит — кроме сценария с
# заведомо недостоверной базой (zero-SHA), где пустота и есть предмет проверки.
range_guard() {
  [ "$3" = "${ZERO}" ] && return 0
  # База, заведомо недостоверная, — ПРЕДМЕТ fail-closed-сценариев, а не отказ фикстуры.
  ( cd "$2" && git cat-file -e "$3" 2>/dev/null ) || return 0
  local n; n="$( ( cd "$2" && git rev-list --count "$3..HEAD" 2>/dev/null ) || echo 0 )"
  [ "${n:-0}" -ge 1 ] || die "$1: диапазон ПУСТ — фикстура не в задуманном состоянии,
  сценарий проверил бы пустоту вместо предмета"
}
# Для сценариев АЛЛОКАТОРА диапазона нет, и range_guard к ним неприменим — но предмет у них
# тоже может исчезнуть при молчаливом отказе setup'а. Замер: если сломать построение фикстуры
# N3HEAD, номер C-505 остаётся на main, ожидание C-506 становится достижимым ОБОИМИ путями, и
# сценарий печатает PASS против ЗАВЕДОМО дефектного аллокатора. Поэтому каждая alloc-фикстура
# заявляет условие, которое делает её сценарием, — и умирает, если оно не выполнено.
# Условие — КОНЪЮНКЦИЯ «предмет ЕСТЬ там, где должен» И «его НЕТ там, где не должен»: одно
# лишь отрицание выполняется ВАКУУМНО, когда фикстура не построилась вовсе, и страж молчит
# ровно тогда, когда обязан кричать (проверено: первая редакция этих стражей не сработала).
setup_assert() {  # $1=имя $2=repo $3=почему $4=условие (shell, обязано вернуть 0)
  ( cd "$2" && eval "$4" ) >/dev/null 2>&1 || die "$1: SETUP не состоялся — $3"
}
expect_block() { mark "$1"; range_guard "$1" "$2" "$3"
  if run_check "$2" "$3" "${5:-push}"; then fail "$1 $4 — ПРОШЛО"; else pass "$1 $4 — заблокировано"; fi; }
expect_allow() { mark "$1"; range_guard "$1" "$2" "$3"
  if run_check "$2" "$3" "${5:-push}"; then pass "$1 $4 — пропущено"; else fail "$1 $4 — ложное срабатывание"; fi; }
# КОД ВОЗВРАТА — часть контракта аллокатора, а не только stdout: он обязан быть fail-closed,
# и «напечатал верный номер, но умер ненулём» — это отказ, который прежняя редакция засчитывала
# как успех на всех шести сценариях оси 3.
expect_alloc() { mark "$1"; local got rc; got="$(run_alloc "$2" "$3")"; rc=$?
  if [ "$got" = "$4" ] && [ "$rc" -eq 0 ]; then pass "$1 $5 — выдал $got"
  else fail "$1 $5 — выдал '${got}' (exit=$rc) при ожидании '$4' (exit=0)"; fi; }
expect_alloc_fails() { mark "$1"
  if run_alloc "$2" "$3" >/dev/null 2>&1; then fail "$1 $4 — ВЫДАЛ номер вместо отказа"
  else pass "$1 $4 — fail-closed"; fi; }

# ═══ БАТАРЕЯ ════════════════════════════════════════════════════════════════════════
run_battery() {
  local d rc bad=0 n=0
  d="$(mktemp -d /tmp/red-ids-battery-XXXXXX)" || die mktemp
  printf '%s\n' "$d" >> "${FIXTURES_REG}"
  bash "${ROOT}/scripts/tests/mk_ref_artifact_ids.sh" "$d" 2>/dev/null \
    || die "эталон не собран: нет ${ROOT}/scripts/tests/mk_ref_artifact_ids.sh
  Батарея требует генератор эталона и мутантов — он часть набора (спека §4.5)."
  echo "══ БАТАРЕЯ (спека §4.5): эталон зелён, каждый мутант красный ПО СВОЕЙ ОСИ ══"

  # ── СВЕРКА СОСТАВА: §4.5 ⇄ генератор ⇄ исполняемый список ────────────────────────────
  # Б-5 (R-052) + A-006 §2.4: раньше список был захардкожен, недостающие пропускались
  # молчаливым `[ -f … ] || continue`, а знаменатель считал ИСПОЛНЕННОЕ — печаталось
  # BATTERY: PASS (10/10) при 7 объявленных из 12. Теперь исполняемый список ВЫВОДИТСЯ из
  # спеки, знаменатель считает ОБЪЯВЛЕННОЕ, а любое расхождение — отказ, не строка в логе.
  local spec="${ROOT}/milestones/M-61-artifact-ids.md"
  [ -f "$spec" ] || die "нет спеки $spec — состав батареи сверять не с чем"
  local DECL AXIS_OF BUILT
  # Строка §4.5 = имя | ось | KILL-SET. Колонки берутся ПОЗИЦИОННО (-F'|'), а не поиском по
  # всей строке: kill-set обязан читаться только из своей колонки, иначе `C-058` из колонки
  # «значение» попал бы в множество сценариев.
  DECL="$(awk -F'|' '/^### 4\.5/{i=1;next} /^## /{i=0}
            i && $2 ~ /^ *`[a-z]+`/ {
              name=$2; sub(/^[^`]*`/,"",name); sub(/`.*/,"",name)
              ax=$3;   sub(/^[^0-9]*/,"",ax);  sub(/[^0-9].*/,"",ax)
              ks=$4;   gsub(/[^A-Z0-9]/," ",ks); gsub(/  +/," ",ks); sub(/^ +/,"",ks); sub(/ +$/,"",ks)
              print name "|" ax "|" ks
            }' "$spec" | sort -u)"
  [ -n "$DECL" ] || die "разбор §4.5 дал ПУСТО — парсер сломан либо раздел переименован"
  BUILT="$(ls "$d" | sed -n 's/-check\.sh$//p' | grep -v '^ref$' | sort -u)"
  local decl_names miss_built miss_decl
  decl_names="$(printf '%s\n' "$DECL" | cut -d'|' -f1 | sort -u)"
  miss_built="$(comm -23 <(printf '%s\n' "$decl_names") <(printf '%s\n' "$BUILT") | tr '\n' ' ')"
  miss_decl="$(comm -13 <(printf '%s\n' "$decl_names") <(printf '%s\n' "$BUILT") | tr '\n' ' ')"
  if [ -z "$miss_built" ] && [ -z "$miss_decl" ]; then
    echo "PASS  состав: §4.5 ⇄ генератор совпали, $(printf '%s\n' "$decl_names" | grep -c .) мутантов"
  else
    [ -n "$miss_built" ] && { echo "FAIL  состав: объявлены в §4.5 и НЕ ПОСТРОЕНЫ: $miss_built"; bad=$((bad+1)); }
    [ -n "$miss_decl" ]  && { echo "FAIL  состав: построены и НЕ ОБЪЯВЛЕНЫ в §4.5: $miss_decl"; bad=$((bad+1)); }
  fi

  # ── ЭТАЛОН ──────────────────────────────────────────────────────────────────────────
  BARRIER="$d/ref-check.sh" ALLOC="$d/ref-next.sh" bash "${SELF}" > "$d/ref.log" 2>&1; rc=$?
  n=$((n + 1))
  if [ $rc -eq 0 ]; then
    echo "PASS  эталон → exit=0 $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' "$d/ref.log"|head -1)"
  else
    echo "FAIL  эталон → exit=$rc (позитивный контроль сломан)"; bad=$((bad+1))
    grep -E '^(FAIL|SETUP)' "$d/ref.log"|head -6|sed 's/^/      ↳ /'
  fi

  # ── МУТАНТЫ: красен + КРАСЕН ПО СВОЕЙ ОСИ ───────────────────────────────────────────
  # Б-4bis (R-052) + A-006 §2.2-2.3: прежний страж проверял лишь «мутант отличается от
  # эталона». Этого мало: quotedname отличался, был красен и при этом СЛОМАН ЦЕЛИКОМ —
  # падал на 11 сценариях, включая чисто-ASCII. Мутант, красный не по своей причине,
  # доказывает не ту дыру, ради которой построен. Ось мутанта берётся из §4.5, ось
  # сценария — из MANIFEST пробы (сценарий может нести несколько осей).
  local m axis want got
  for m in $(printf '%s\n' "$decl_names"); do
    n=$((n + 1))
    axis="$(printf '%s\n' "$DECL" | grep "^${m}|" | cut -d'|' -f2 | head -1)"
    want="$(printf '%s\n' "$DECL" | grep "^${m}|" | cut -d'|' -f3 | head -1 | tr ' ' '\n' | grep . | sort -u | tr '\n' ' ')"
    [ -n "${want// /}" ] || die "мутант $m объявлен в §4.5 БЕЗ kill-set'а — сверять не с чем.
  Колонка «ОБЯЗАН уронить» обязательна: без неё страж вырождается в «мутант хоть как-то красен»."
    if [ ! -f "$d/$m-check.sh" ]; then
      echo "FAIL  $m ОБЪЯВЛЕН в §4.5, но не построен — молчаливый пропуск запрещён"; bad=$((bad+1)); continue
    fi
    BARRIER="$d/$m-check.sh" ALLOC="$d/$m-next.sh" bash "${SELF}" > "$d/$m.log" 2>&1; rc=$?
    got="$(grep -oE '^FAIL +[A-Z0-9]+' "$d/$m.log" | awk '{print $2}' | sort -u | tr '\n' ' ')"
    if [ "$got" = "$want" ] && [ $rc -ne 0 ]; then
      echo "PASS  $m → exit=$rc, ось $axis, уронил ровно объявленное: ${got}"
    else
      echo "FAIL  $m: kill-set РАЗОШЁЛСЯ с §4.5 (exit=$rc)"
      echo "      ↳ объявлено:  ${want:-—}"
      echo "      ↳ наблюдается: ${got:-— (ни одного сценария)}"
      [ -z "${got// /}" ] && echo "      ↳ пусто ⇒ мутант ничего не пиннит: дыра не закреплена либо упал SETUP пробы"
      echo "      ↳ БОЛЬШЕ объявленного ⇒ мутант сломан сверх своей оси и доказывает не ту дыру;"
      echo "      ↳ МЕНЬШЕ объявленного ⇒ сценарий перестал ловить дефект, ради которого стоит"
      bad=$((bad+1))
    fi
  done

  # ── страж «нет барьера» ─────────────────────────────────────────────────────────────
  BARRIER="$d/НЕТ.sh" ALLOC="$d/НЕТ.sh" bash "${SELF}" > "$d/nobar.log" 2>&1; rc=$?
  n=$((n + 1))
  if [ $rc -ne 0 ] && grep -q 'SETUP НЕ СОСТОЯЛСЯ' "$d/nobar.log"; then
    echo "PASS  без барьера → exit=$rc, «SETUP НЕ СОСТОЯЛСЯ»"
  else echo "FAIL  без барьера → exit=$rc — проба зеленеет на пустом месте"; bad=$((bad+1)); fi
  echo
  [ "$bad" -gt 0 ] && { echo "BATTERY: FAIL (${bad} из ${n})"; return 1; }
  echo "BATTERY: PASS (${n}/${n})"; return 0
}
[ "${1:-}" = "--battery" ] && { run_battery; exit $?; }

# ═══ СТРАЖИ SETUP'А ═════════════════════════════════════════════════════════════════
[ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}. Проба НЕ имеет права быть зелёной,
  пока гейт не существует: 127 от bash неотличим от честного отказа."
bash -n "${BARRIER}" 2>/dev/null || die "барьер не парсится"
[ -f "${ALLOC}" ] || die "аллокатора нет: ${ALLOC} (ось 3 непроверяема)"
bash -n "${ALLOC}" 2>/dev/null || die "аллокатор не парсится"
[ -f "${SPEC}" ] || die "спеки нет: ${SPEC}. Состав осей сверять не с чем."

echo "── Номера артефактов (M-61): ШЕСТЬ осей ──"
echo "барьер: ${BARRIER}"; echo "аллокатор: ${ALLOC}"; echo "спека: ${SPEC}"; echo

# ─── ОСЬ 1 + 2 + 4 + 5: класс, носитель, способ, срез ────────────────────────────────
R="$(mk_repo b1td)"; td_entry "$R" TD-200 "первый-предмет"; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; td_entry "$R" TD-200 "совсем-другой-предмет"; commit_all "$R" "второй TD-200"
expect_block B1TD "$R" "$B" "TD-200 второй записью в TECH-DEBT.md (класс TD · носитель — запись)"

R="$(mk_repo b1r)"; art "$R" research/reviews/R-200-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/reviews/R-200-beta.md ""; commit_all "$R" "второй R-200"
expect_block B1R "$R" "$B" "R-200 вторым файлом (класс R · носитель — имя · новый файл · новая коллизия)"

R="$(mk_repo b1c)"; art "$R" research/critiques/C-200-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/critiques/C-200-beta.md ""; commit_all "$R" "второй C-200"
expect_block B1C "$R" "$B" "C-200 вторым файлом (класс C)"

R="$(mk_repo b1a)"; art "$R" research/arbitration/A-200-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/arbitration/A-200-beta.md ""; commit_all "$R" "второй A-200"
expect_block B1A "$R" "$B" "A-200 вторым файлом (класс A)"

R="$(mk_repo b1m)"; art "$R" milestones/M-70-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" milestones/M-70-beta.md ""; commit_all "$R" "второй M-70"
expect_block B1M "$R" "$B" "M-70 вторым файлом (класс M)"

R="$(mk_repo l1x)"; art "$R" docs/X-200-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" docs/X-200-beta.md ""; commit_all "$R" "второй X-200"
expect_allow L1X "$R" "$B" "X-200 — префикс вне зоны, барьер не его сторож"

R="$(mk_repo l2txt)"; art "$R" research/reviews/R-210-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && printf 'см. R-210 у одних и R-210 у других\n' >> docs/base.md ) && commit_all "$R" "упоминания в тексте"
expect_allow L2TXT "$R" "$B" "R-210 дважды УПОМЯНУТ в тексте — упоминание не занимает номер"

R="$(mk_repo b4ren)"; art "$R" research/reviews/R-220-alpha.md ""; art "$R" research/reviews/R-999-other.md ""
commit_all "$R" base2; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git mv research/reviews/R-999-other.md research/reviews/R-220-other.md ) && commit_all "$R" "увод в занятый номер"
expect_block B4REN "$R" "$B" "переименование в ЗАНЯТЫЙ номер"

# B4Q — имя артефакта, которое git КВОТИРУЕТ в текстовом выводе (не-ASCII, кавычка,
# обратный слэш). Реализация, читающая `ls-tree`/`show` ПОСТРОЧНО, получает имя уже
# экранированным (`"research/reviews/R-940-\320\260.md"`) и не узнаёт ни класс, ни номер —
# коллизия проходит молча. Тот же класс, что закрыт в M-60a значением `квотируемое имя члена
# зоны` (мутант quotedpath); в M-61 ось 4 унаследована БЕЗ него, и потому ДВЕ независимые
# реализации engine-dev прошли 25/25, обе пропуская эту коллизию (замер architect'а 2026-08-10).
# Лечится сменой КАНАЛА на `-z`, а не доработкой разбора кавычек.
R="$(mk_repo b4q)"; art "$R" 'research/reviews/R-940-альфа.md' ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"
art "$R" 'research/reviews/R-940-бета.md' ""; commit_all "$R" "второй R-940 с не-ASCII именем"
expect_block B4Q "$R" "$B" "коллизия под именами, требующими квотирования"

R="$(mk_repo l4del)"; art "$R" research/reviews/R-230-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; ( cd "$R" && git rm -q research/reviews/R-230-alpha.md ) && commit_all "$R" "удаление"
expect_allow L4DEL "$R" "$B" "удаление артефакта номер не занимает"

# L4MOD — ПРАВКА существующего артефакта предмета не вводит (R-046 Б-2). Реализация,
# собирающая введённое через `--diff-filter=AM`, считает буквой M «введением» коммит, который
# лишь редактирует файл. Радиус — девять предсуществующих коллизий в main (R-035, R-038, M-46,
# C-018, C-024), которые §5 ЗАПРЕЩАЕТ переименовывать: барьер начинал блокировать их
# обслуживание вместо ввода новых, то есть работал против §4.1.
R="$(mk_repo l4mod)"; art "$R" research/reviews/R-950-alpha.md ""; art "$R" research/reviews/R-950-beta.md ""
commit_all "$R" "предсуществующая коллизия"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && echo дополнение >> research/reviews/R-950-alpha.md ) && commit_all "$R" "правка существующего"
expect_allow L4MOD "$R" "$B" "правка существующего артефакта под коллизионным номером"

R="$(mk_repo b5third)"; art "$R" research/reviews/R-300-alpha.md ""; art "$R" research/reviews/R-300-beta.md ""
commit_all "$R" "предсуществующая коллизия"; B="$(cd "$R" && git rev-parse HEAD)"
art "$R" research/reviews/R-300-gamma.md ""; commit_all "$R" "третий R-300"
expect_block B5THIRD "$R" "$B" "УСИЛЕНИЕ существующей коллизии третьим файлом"

# B5MID — коллизия введена НЕ ВЕРШИННЫМ коммитом диапазона (ось 5, новое значение; A-006 §2.3).
# Реализация, разбирающая только итоговый дифф или только вершину `$BASE..HEAD`, видит на
# вершине постороннюю правку и пропускает коллизию, лежащую коммитом глубже. (Реализация,
# разбирающая ИТОГОВЫЙ дифф диапазона, этим сценарием НЕ ловится — проверено исполнением;
# B5MID пиннит ровно tip-only, и обещать больше он не вправе.) На проде это
# нормальная форма ветки: milestone копит 5-15 коммитов, артефакт вводится в середине, а
# push сравнивается с состоянием ДО всей серии. Мутант `rangeblind` (`rev-list -n 1`) —
# ровно эта реализация; без такого сценария он проходил пробу целиком (§4.5 нарушен).
R="$(mk_repo b5mid)"; art "$R" research/reviews/R-330-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"
art "$R" research/reviews/R-330-beta.md ""; commit_all "$R" "коллизия — НЕ вершина диапазона"
( cd "$R" && echo x >> docs/base.md ) && commit_all "$R" "вершина диапазона — постороннее"
expect_block B5MID "$R" "$B" "коллизия в НЕ-ВЕРШИННОМ коммите диапазона"

R="$(mk_repo l5pre)"; art "$R" research/reviews/R-310-alpha.md ""; art "$R" research/reviews/R-310-beta.md ""
commit_all "$R" "предсуществующая коллизия"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && echo x >> docs/base.md ) && commit_all "$R" "постороннее"
expect_allow L5PRE "$R" "$B" "предсуществующая коллизия ВНЕ диапазона — не предмет суда"

# L5FIX — ветка исправила СВОЮ ЖЕ ошибку перенумерацией: артефакт под занятым номером
# появился коммитом диапазона и тем же диапазоном уведён на свободный. В РЕЗУЛЬТАТЕ коллизии
# нет, и инвариант §4.1 («от РЕЗУЛЬТАТА») требует пропустить. Барьер, обходящий коммиты без
# сверки с HEAD, судит промежуточное состояние и краснеет — ложный красный на рабочей форме:
# в истории репозитория она уже случалась (`f0e915b`: R-038-M-60a → R-042, снятие блокера
# F-2 `R-041`). Парный ЛЕГИТИМНЫЙ сценарий к значению «глубина диапазона» (§4.3 усл. 2):
# без него ось 5 закреплена только со стороны «слишком мягко», и реализация «запретить всё»
# проходит набор.
R="$(mk_repo l5fix)"; art "$R" research/reviews/R-038-branch-hygiene.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"
art "$R" research/reviews/R-038-M-60a.md ""; commit_all "$R" "ошибка: вердикт под занятым номером"
( cd "$R" && git mv research/reviews/R-038-M-60a.md research/reviews/R-042-M-60a.md ) || die "l5fix setup"
commit_all "$R" "исправлено: перенумерация R-038 → R-042"
expect_allow L5FIX "$R" "$B" "ошибка исправлена перенумерацией ВНУТРИ диапазона — в результате коллизии нет"

R="$(mk_repo b5base)"; art "$R" research/reviews/R-320-alpha.md ""; commit_all "$R" base2
art "$R" research/reviews/R-320-beta.md ""; commit_all "$R" "второй"
expect_block B5BASE "$R" "$ZERO" "недостоверная база (zero-SHA) ⇒ fail-closed"

# B5PR / B5NOBASE / B5NOTANC — прод-формы вызова, которых проба не знала.
# Барьер разбирает ТРИ fail-closed-ветки (`cat-file -e`, `merge-base --is-ancestor`) и ДВА
# события. Пиннилась одна: zero-SHA на push. Сегодня прод-код на этих путях ВЕРЕН — дефекта
# нет; отсутствовал ОРАКУЛ, то есть будущее ослабление уехало бы молча при всех зелёных гейтах.
R="$(mk_repo b5pr)"; art "$R" research/reviews/R-810-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/reviews/R-810-beta.md ""; commit_all "$R" "второй R-810"
expect_block B5PR "$R" "$B" "коллизия на событии pull_request — база живёт в PR_BASE_SHA, не в PUSH_BEFORE" pull_request

# B5NOBASE пиннит ПАРУ guard'"'"'ов (существование базы И «база — предок»), а не строку: снятие
# одной из них подстраховывает вторая, и сценарий краснеет только когда сняты ОБЕ. Названо
# здесь прямо — оракул не вправе обещать разрешающую силу, которой у него нет.
R="$(mk_repo b5nobase)"; art "$R" research/reviews/R-820-alpha.md ""; commit_all "$R" base2
art "$R" research/reviews/R-820-beta.md ""; commit_all "$R" "второй R-820"
expect_block B5NOBASE "$R" "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" "база отсутствует в истории ⇒ fail-closed"

# ВАЖНО: в диапазоне НЕТ коллизии. Иначе барьер заблокировал бы «за компанию» — по находке
# основной логики, а не по проверке предка, и сценарий не различал бы её снятие (проверено:
# первая редакция этой фикстуры несла коллизию и мутацию НЕ ловила). Отказ обязан приходить
# ровно от недостоверности базы.
R="$(mk_repo b5notanc)"; art "$R" research/reviews/R-830-alpha.md ""; commit_all "$R" base2
( cd "$R" && git checkout -q -b divergent ) && art "$R" docs/side.md "" && commit_all "$R" "чужая ветка"
FOREIGN="$(cd "$R" && git rev-parse HEAD)"; ( cd "$R" && git checkout -q main ) || die "b5notanc setup"
( cd "$R" && echo x >> docs/base.md ) && commit_all "$R" "постороннее на main"
setup_assert B5NOTANC "$R" "база обязана быть НЕ предком HEAD, а диапазон — БЕЗ коллизии" \
  "! git merge-base --is-ancestor $FOREIGN HEAD && [ \"\$(git ls-tree -r --name-only HEAD | grep -c R-830)\" -eq 1 ]"
expect_block B5NOTANC "$R" "$FOREIGN" "база НЕ предок HEAD ⇒ fail-closed (коллизии в диапазоне нет)"

# B3ORIG — ось 3 у БАРЬЕРА, а не у аллокатора. rev2 закрыл origin-направление только для
# аллокатора, и это оказалось половиной работы: `universe()` барьера перечисляет ТЕ ЖЕ ref'ы,
# и барьер, потерявший `refs/remotes/origin`, не видит номера, занятого чужой УДАЛЁННОЙ веткой,
# — то есть пропускает ровно тот дефект, ради которого milestone написан (§1). Ни один сценарий
# и ни один мутант этого не пиннили: фикстуры барьера жили на локальных ветках, где потеря
# origin ничего не меняет. Проверено: снятие origin из all_refs() барьера оставляло пробу
# зелёной по всем барьерным сценариям.
R="$(mk_repo b3orig)"
( cd "$R" && git checkout -q -b tmp ) && art "$R" research/reviews/R-860-alpha.md "" && commit_all "$R" "занято в origin"
( cd "$R" && git update-ref refs/remotes/origin/tmp "$(git rev-parse HEAD)" && git checkout -q main && git branch -qD tmp ) || die "b3orig setup"
B="$(cd "$R" && git rev-parse HEAD)"
art "$R" research/reviews/R-860-beta.md ""; commit_all "$R" "второй R-860 — занят только в origin"
setup_assert B3ORIG "$R" "R-860 обязан быть занят ТОЛЬКО origin-ref'ом, вне локальных голов" \
  '[ "$(git ls-tree -r --name-only refs/remotes/origin/tmp | grep -c R-860)" -ge 1 ] && [ "$(git ls-tree -r --name-only main | grep -c R-860-alpha)" -eq 0 ]'
expect_block B3ORIG "$R" "$B" "номер занят ТОЛЬКО удалённой веткой — барьер обязан её видеть"

# ─── ОСЬ 3: область поиска занятости (предмет — АЛЛОКАТОР) ───────────────────────────
# Номер занят в СОСЕДНЕЙ ветке: свободен локально, занят в объединении.
R="$(mk_repo n3local)"; art "$R" research/reviews/R-400-a.md ""; commit_all "$R" base2
( cd "$R" && git checkout -q -b side && : ) && art "$R" research/reviews/R-407-side.md "" && commit_all "$R" "на соседней"
( cd "$R" && git checkout -q main )
setup_assert N3LOCAL "$R" "номер 407 обязан жить ТОЛЬКО в соседней ветке — иначе сценарий не про объединение" '[ "$(for h in $(git for-each-ref --format="%(refname)" refs/heads); do git ls-tree -r --name-only "$h"; done | grep -c R-407)" -ge 1 ] && [ "$(git ls-tree -r --name-only main | grep -c R-407)" -eq 0 ]'
expect_alloc N3LOCAL "$R" R "R-408" "максимум по ОБЪЕДИНЕНИЮ, а не по своему дереву"

# Номер занят ТОЛЬКО локальным head'ом (в origin его нет).
R="$(mk_repo n3head)"; art "$R" research/critiques/C-500-a.md ""; commit_all "$R" base2
( cd "$R" && git update-ref refs/remotes/origin/main HEAD && git checkout -q -b local-only )
art "$R" research/critiques/C-505-local.md ""; commit_all "$R" "только локально"
( cd "$R" && git checkout -q main )
setup_assert N3HEAD "$R" "C-505 обязан жить ТОЛЬКО в локальной ветке, вне origin/main — иначе ожидание достижимо обоими путями" '[ -n "$(git rev-parse -q --verify refs/remotes/origin/main)" ] && [ "$(for h in $(git for-each-ref --format="%(refname)" refs/heads); do git ls-tree -r --name-only "$h"; done | grep -c C-505)" -ge 1 ] && [ "$(git ls-tree -r --name-only refs/remotes/origin/main | grep -c C-505)" -eq 0 ]'
expect_alloc N3HEAD "$R" C "C-506" "локальный head участвует в подсчёте занятости"

# Origin сконфигурирован, но ref'ов нет — перечислить занятость невозможно.
R="$(mk_repo n3noorig)"; art "$R" research/reviews/R-600-a.md ""; commit_all "$R" base2
( cd "$R" && git remote add origin /nonexistent-remote-path )
setup_assert N3NOORIG "$R" "origin обязан быть СКОНФИГУРИРОВАН и без единого ref\047а — иначе проверяется не fail-closed" 'git remote get-url origin && [ -z "$(git for-each-ref --format="%(refname)" refs/remotes/origin)" ]'
expect_alloc_fails N3NOORIG "$R" R "origin сконфигурирован, но недоступен ⇒ fail-closed"

# N3ORIG — ЗЕРКАЛО N3HEAD: номер занят ТОЛЬКО в origin-ref'е, локальных голов с ним нет.
# Без этого сценария ось 3 была покрыта в ОДНУ сторону: реализация, у которой из перечисления
# выпал `refs/remotes/origin`, оставляла пробу зелёной (28/28) — то есть корневой дефект §1
# («номер, свободный локально, занят в соседней ветке») не пиннился ничем. Найдено адверсарной
# проверкой круга 4; категория (i) §4.4 — новое значение известной оси.
R="$(mk_repo n3orig)"; art "$R" research/reviews/R-600-a.md ""; commit_all "$R" base2
( cd "$R" && git checkout -q -b tmp ) && art "$R" research/reviews/R-650-orig.md "" && commit_all "$R" "номер, живущий только в origin"
( cd "$R" && git update-ref refs/remotes/origin/tmp "$(git rev-parse HEAD)" && git checkout -q main && git branch -qD tmp ) || die "n3orig setup"
setup_assert N3ORIG "$R" "R-650 обязан быть НЕДОСТИЖИМ из локальных голов — иначе сценарий не про origin" '[ "$(git ls-tree -r --name-only refs/remotes/origin/tmp | grep -c R-650)" -ge 1 ] && [ "$(for h in $(git for-each-ref --format="%(refname)" refs/heads); do git ls-tree -r --name-only "$h"; done | grep -c R-650)" -eq 0 ]'
expect_alloc N3ORIG "$R" R "R-651" "номер занят ТОЛЬКО origin-ref'ом — локальной головы с ним нет"

# N3TD — та же ось 3, но КЛАСС TD: он живёт записью в `TECH-DEBT.md`, а не именем файла, и
# потому ходит отдельной веткой кода аллокатора. Эта ветка не пиннилась ничем: шаг N гейта
# перебирал только M/R/C/A, а сценариев на неё не было вовсе — дефект «только своё дерево»,
# внесённый в TD-ветку, оставлял пробу 28/28 и батарею зелёными, при том что аллокатор выдавал
# номер, уже занятый соседней веткой. Найдено адверсарной проверкой круга 4.
R="$(mk_repo n3td)"; td_entry "$R" TD-300 "первый-предмет"; commit_all "$R" base2
( cd "$R" && git checkout -q -b side ) && td_entry "$R" TD-307 "долг соседней ветки" && commit_all "$R" "TD на соседней ветке"
( cd "$R" && git checkout -q main ) || die "n3td setup"
setup_assert N3TD "$R" "TD-307 обязан отсутствовать в TECH-DEBT.md своего дерева — иначе сценарий не про объединение" '[ "$(git show side:TECH-DEBT.md | grep -c TD-307)" -ge 1 ] && [ "$(grep -c TD-307 TECH-DEBT.md)" -eq 0 ]'
expect_alloc N3TD "$R" TD "TD-308" "класс TD: занятость по объединению, а не по своему дереву"

R="$(mk_repo l3ok)"; art "$R" milestones/M-90-a.md ""; commit_all "$R" base2
( cd "$R" && git update-ref refs/remotes/origin/main HEAD )
setup_assert L3OK "$R" "origin-ref обязан существовать — иначе штатный случай не воспроизводится" '[ -n "$(git for-each-ref --format="%(refname)" refs/remotes/origin)" ]'
expect_alloc L3OK "$R" M "M-91" "штатный случай: максимум по origin ∪ refs/heads"

# ─── ОСЬ 6: носитель идентичности предмета ──────────────────────────────────────────
HDR_A='**Предмет:** `docs/plans/alpha.md`'
HDR_B='**Предмет:** `docs/plans/beta.md`'
HDR_CONT='**Контекст.** Второй критик по тому же предмету
(`docs/plans/alpha.md`), первый считался зависшим.'

R="$(mk_repo b6slug)"; art "$R" research/reviews/R-700-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/reviews/R-700-beta.md ""; commit_all "$R" "другой слаг"
expect_block B6SLUG "$R" "$B" "разные слаги под одним номером без шапок"

R="$(mk_repo b6noslug)"; art "$R" research/critiques/C-710.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/critiques/C-710-foo.md ""; commit_all "$R" "слаг против slugless"
expect_block B6NOSLUG "$R" "$B" "slugless против слага — это РАЗНЫЕ предметы, а не пропуск"

R="$(mk_repo b6hdr)"; art "$R" research/critiques/C-720-one.md "$HDR_A"; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/critiques/C-720-two.md "$HDR_B"; commit_all "$R" "шапки расходятся"
expect_block B6HDR "$R" "$B" "шапки называют РАЗНЫЕ предметы"

R="$(mk_repo b6split)"; art "$R" milestones/M-80a-alpha.md "$HDR_A"; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" milestones/M-80c-foreign.md "$HDR_B"; commit_all "$R" "чужой split"
expect_block B6SPLIT "$R" "$B" "split-суффикс без совпавшего предмета — буква не доказательство"

R="$(mk_repo l6slug)"; art "$R" research/reviews/R-730-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && echo дополнение >> research/reviews/R-730-alpha.md ) && commit_all "$R" "правка того же файла"
expect_allow L6SLUG "$R" "$B" "тот же слаг — тот же предмет"

R="$(mk_repo l6hdr)"; art "$R" research/critiques/C-740-scale-architecture.md "$HDR_A"; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/critiques/C-740-addendum-critic2.md "$HDR_A"; commit_all "$R" "второй критик"
expect_allow L6HDR "$R" "$B" "слаги РАЗНЫЕ, шапка одна — законный второй критик (случай C-058)"

R="$(mk_repo l6cont)"; art "$R" research/critiques/C-750-first.md "$HDR_A"; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/critiques/C-750-addendum.md "$HDR_CONT"; commit_all "$R" "предмет со строки-продолжения"
expect_allow L6CONT "$R" "$B" "путь предмета на СТРОКЕ-ПРОДОЛЖЕНИИ блока «Контекст»"

R="$(mk_repo l6split)"; art "$R" milestones/M-85-mechanisms.md "$HDR_A"; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" milestones/M-85b-part.md "$HDR_A"; commit_all "$R" "законное дробление"
expect_allow L6SPLIT "$R" "$B" "split с СОВПАВШИМ предметом — законное дробление семьи"

R="$(mk_repo l6rev)"; art "$R" research/reviews/R-760-alpha.md ""; commit_all "$R" base2
B="$(cd "$R" && git rev-parse HEAD)"; art "$R" research/reviews/R-760-alpha-rev2.md ""; commit_all "$R" "ревизия"
expect_allow L6REV "$R" "$B" "ревизия того же предмета (-rev2)"

# ═══ СВЕРКА 1 — манифест ⇄ исполнение (по ИМЕНАМ, в обе стороны) ════════════════════
echo
DECL="$(printf '%s' "${MANIFEST}" | grep '|' | cut -d'|' -f1 | sort -u)"
RUN="$(printf '%s' "${EXECUTED}" | grep . | sort -u)"
MISS="$(comm -23 <(printf '%s\n' "$DECL") <(printf '%s\n' "$RUN") | tr '\n' ' ')"
EXTRA="$(comm -13 <(printf '%s\n' "$DECL") <(printf '%s\n' "$RUN") | tr '\n' ' ')"
if [ -n "${MISS// /}" ] || [ -n "${EXTRA// /}" ]; then
  [ -n "${MISS// /}" ]  && { echo "FAIL  МАНИФЕСТ: объявлены, НЕ исполнены: ${MISS}"; FAILED=$((FAILED+1)); }
  [ -n "${EXTRA// /}" ] && { echo "FAIL  МАНИФЕСТ: исполнены, НЕ объявлены: ${EXTRA}"; FAILED=$((FAILED+1)); }
else
  echo "PASS  МАНИФЕСТ ⇄ исполнение: $(printf '%s\n' "$RUN" | grep -c .) сценариев, состав совпал в обе стороны"
fi

# ═══ СВЕРКА 2 — манифест ⇄ таблица §4.2 спеки ═══════════════════════════════════════
spec_pairs() {
  awk -F'|' '
    function emit(a, kind, cell,   n, p, i) {
      n = split(cell, p, "`"); for (i = 2; i <= n; i += 2) if (p[i] != "") print a "|" kind "|" p[i] }
    /^#/ { inside = ($0 ~ /^### 4\.2/); next }
    inside && /^\|[[:space:]]*\*\*[0-9]+\./ {
      if (!match($2, /\*\*[0-9]+\./)) next
      axis = substr($2, RSTART + 2, RLENGTH - 3); emit(axis, "V", $3); emit(axis, "L", $4) }
  ' "$1" | sort -u
}
SP="$(spec_pairs "${SPEC}")"
MP="$(printf '%s' "${MANIFEST}" | grep '|' | cut -d'|' -f2- | sort -u)"
[ -n "$(printf '%s' "$SP" | grep .)" ] || die "таблица §4.2 в ${SPEC} не разобрана — сверять не с чем"
ONLY_S="$(comm -23 <(printf '%s\n' "$SP") <(printf '%s\n' "$MP"))"
ONLY_M="$(comm -13 <(printf '%s\n' "$SP") <(printf '%s\n' "$MP"))"
if [ -n "${ONLY_S}" ] || [ -n "${ONLY_M}" ]; then
  [ -n "${ONLY_S}" ] && { echo "FAIL  СПЕКА⇄МАНИФЕСТ: объявлено в §4.2, НЕ покрыто:"; printf '%s\n' "$ONLY_S" | sed 's/^/        ось /'; FAILED=$((FAILED+1)); }
  [ -n "${ONLY_M}" ] && { echo "FAIL  СПЕКА⇄МАНИФЕСТ: покрыто, НЕ объявлено в §4.2:"; printf '%s\n' "$ONLY_M" | sed 's/^/        ось /'; FAILED=$((FAILED+1)); }
else
  echo "PASS  СПЕКА⇄МАНИФЕСТ: $(printf '%s\n' "$SP" | grep -c .) пар (ось,вид,значение) совпали в обе стороны"
fi

# ═══ СВЕРКА 3 — §4.3(2): у каждой оси есть легитимный сценарий ══════════════════════
NOLEGIT=""
while IFS= read -r ax; do
  [ -z "$ax" ] && continue
  printf '%s\n' "$MP" | grep -q "^${ax}|L|" || NOLEGIT="${NOLEGIT} ${ax}"
done <<< "$(printf '%s' "$SP" | grep . | cut -d'|' -f1 | sort -u)"
if [ -n "${NOLEGIT// /}" ]; then
  echo "FAIL  §4.3(2): у осей${NOLEGIT} нет легитимного сценария — набор проходит «запретить всё»"
  FAILED=$((FAILED+1))
else
  echo "PASS  §4.3(2): у каждой оси есть легитимный сценарий"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Полнота заявлена ОТНОСИТЕЛЬНО шести осей (спека §4.2);"
  echo "опровержение обязано называть ОСЬ и ЗНАЧЕНИЕ (§4.4)."
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/${PASSED}) — все значения шести осей покрыты, состав сверен со спекой"
