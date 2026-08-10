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
N3NOORIG|3|V|origin недоступен
L3OK|3|L|origin ∪ refs/heads
B4REN|4|V|переименование в занятый номер
B4Q|4|V|имя, требующее квотирования
L4DEL|4|L|удаление артефакта
B5THIRD|5|V|усиление существующей коллизии
B5BASE|5|V|недостоверная база
L5PRE|5|L|предсуществующая коллизия вне диапазона
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

FAILED=0; PASSED=0; EXECUTED=""; FIXTURES=""
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }
mark() { EXECUTED="${EXECUTED}$1
"; }

cleanup() {
  [ "${KEEP_FIXTURES:-0}" = "1" ] && { echo "фикстуры сохранены" >&2; return; }
  local d; while IFS= read -r d; do
    [ -n "$d" ] && [ -d "$d" ] && case "$d" in /tmp/red-ids-*) rm -rf "$d";; esac
  done <<< "${FIXTURES}"
}
trap cleanup EXIT

mk_repo() {
  local d; d="$(mktemp -d "/tmp/red-ids-$1-XXXXXX")" || die mktemp
  FIXTURES="${FIXTURES}${d}
"
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

run_check() { ( cd "$1" && EVENT_NAME="${3:-push}" PUSH_BEFORE="$2" PR_BASE_SHA="$2" \
                bash "${BARRIER}" >/dev/null 2>&1 ); }
run_alloc() { ( cd "$1" && bash "${ALLOC}" "$2" 2>/dev/null ); }

expect_block() { mark "$1"
  if run_check "$2" "$3"; then fail "$1 $4 — ПРОШЛО"; else pass "$1 $4 — заблокировано"; fi; }
expect_allow() { mark "$1"
  if run_check "$2" "$3"; then pass "$1 $4 — пропущено"; else fail "$1 $4 — ложное срабатывание"; fi; }
expect_alloc() { mark "$1"; local got; got="$(run_alloc "$2" "$3")"
  if [ "$got" = "$4" ]; then pass "$1 $5 — выдал $got"
  else fail "$1 $5 — выдал '${got}' при ожидании '$4'"; fi; }
expect_alloc_fails() { mark "$1"
  if run_alloc "$2" "$3" >/dev/null 2>&1; then fail "$1 $4 — ВЫДАЛ номер вместо отказа"
  else pass "$1 $4 — fail-closed"; fi; }

# ═══ БАТАРЕЯ ════════════════════════════════════════════════════════════════════════
run_battery() {
  local d rc bad=0 n=0
  d="$(mktemp -d /tmp/red-ids-battery-XXXXXX)" || die mktemp
  FIXTURES="${FIXTURES}${d}
"
  bash "${ROOT}/scripts/tests/mk_ref_artifact_ids.sh" "$d" 2>/dev/null \
    || die "эталон не собран: нет ${ROOT}/scripts/tests/mk_ref_artifact_ids.sh
  Батарея требует генератор эталона и мутантов — он часть набора (спека §4.5)."
  echo "══ БАТАРЕЯ (спека §4.5): эталон зелён, каждый мутант красный ══"
  for v in ref showall localmax renameblind slugonly contextblind splitonly quotedname; do
    [ -f "$d/$v-check.sh" ] || continue
    BARRIER="$d/$v-check.sh" ALLOC="$d/$v-next.sh" bash "${SELF}" > "$d/$v.log" 2>&1; rc=$?
    n=$((n + 1))
    if [ "$v" = ref ]; then
      [ $rc -eq 0 ] && echo "PASS  эталон → exit=0 $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' "$d/$v.log"|head -1)" \
        || { echo "FAIL  эталон → exit=$rc (позитивный контроль сломан)"; grep -E '^(FAIL|SETUP)' "$d/$v.log"|head -6|sed 's/^/      ↳ /'; bad=$((bad+1)); }
    else
      [ $rc -ne 0 ] && echo "PASS  $v → exit=$rc $(grep -oE 'VERDICT: FAIL \([0-9]+\)' "$d/$v.log"|head -1)" \
        || { echo "FAIL  $v ПРОШЁЛ пробу (exit=0) — дыра"; bad=$((bad+1)); }
    fi
  done
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

R="$(mk_repo b5third)"; art "$R" research/reviews/R-300-alpha.md ""; art "$R" research/reviews/R-300-beta.md ""
commit_all "$R" "предсуществующая коллизия"; B="$(cd "$R" && git rev-parse HEAD)"
art "$R" research/reviews/R-300-gamma.md ""; commit_all "$R" "третий R-300"
expect_block B5THIRD "$R" "$B" "УСИЛЕНИЕ существующей коллизии третьим файлом"

R="$(mk_repo l5pre)"; art "$R" research/reviews/R-310-alpha.md ""; art "$R" research/reviews/R-310-beta.md ""
commit_all "$R" "предсуществующая коллизия"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && echo x >> docs/base.md ) && commit_all "$R" "постороннее"
expect_allow L5PRE "$R" "$B" "предсуществующая коллизия ВНЕ диапазона — не предмет суда"

R="$(mk_repo b5base)"; art "$R" research/reviews/R-320-alpha.md ""; commit_all "$R" base2
art "$R" research/reviews/R-320-beta.md ""; commit_all "$R" "второй"
expect_block B5BASE "$R" "$ZERO" "недостоверная база (zero-SHA) ⇒ fail-closed"

# ─── ОСЬ 3: область поиска занятости (предмет — АЛЛОКАТОР) ───────────────────────────
# Номер занят в СОСЕДНЕЙ ветке: свободен локально, занят в объединении.
R="$(mk_repo n3local)"; art "$R" research/reviews/R-400-a.md ""; commit_all "$R" base2
( cd "$R" && git checkout -q -b side && : ) && art "$R" research/reviews/R-407-side.md "" && commit_all "$R" "на соседней"
( cd "$R" && git checkout -q main )
expect_alloc N3LOCAL "$R" R "R-408" "максимум по ОБЪЕДИНЕНИЮ, а не по своему дереву"

# Номер занят ТОЛЬКО локальным head'ом (в origin его нет).
R="$(mk_repo n3head)"; art "$R" research/critiques/C-500-a.md ""; commit_all "$R" base2
( cd "$R" && git update-ref refs/remotes/origin/main HEAD && git checkout -q -b local-only )
art "$R" research/critiques/C-505-local.md ""; commit_all "$R" "только локально"
( cd "$R" && git checkout -q main )
expect_alloc N3HEAD "$R" C "C-506" "локальный head участвует в подсчёте занятости"

# Origin сконфигурирован, но ref'ов нет — перечислить занятость невозможно.
R="$(mk_repo n3noorig)"; art "$R" research/reviews/R-600-a.md ""; commit_all "$R" base2
( cd "$R" && git remote add origin /nonexistent-remote-path )
expect_alloc_fails N3NOORIG "$R" R "origin сконфигурирован, но недоступен ⇒ fail-closed"

R="$(mk_repo l3ok)"; art "$R" milestones/M-90-a.md ""; commit_all "$R" base2
( cd "$R" && git update-ref refs/remotes/origin/main HEAD )
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
