#!/usr/bin/env bash
# Проба замка процессного слоя — `scripts/check_docs_freeze.sh` (M-60a, rev6).
#
# ИНВАРИАНТ ФОРМУЛИРУЕТСЯ ОТ РЕЗУЛЬТАТА (спека §3bis.1):
#   ни одно изменение зоны замка не попадает в проверяемый диапазон без токена
#   `FOUNDER-APPROVED` в СВОЁМ коммите — независимо от способа изменения, типа события и
#   формы истории.
#
# ПОЧЕМУ НАБОР ПОСТРОЕН ОТ ОСЕЙ. Четыре круга критика дали четыре REJECT, и все — о пробе,
# а не о замысле. Перечисление обходов дна не имеет; полнота заявляется ОТНОСИТЕЛЬНО ПЯТИ
# ОСЕЙ (спека §3bis.2), и это заявление здесь ПРОВЕРЯЕТСЯ, а не декларируется:
#   ось 1 «способ изменения зоны»  — M · A · D · R
#   ось 2 «тип события»            — push · pull_request
#   ось 3 «форма истории»          — значения выведены из кванторов инварианта (A-005 §2)
#   ось 4 «член зоны»              — пять путей, не один префикс
#   ось 5 «носитель токена»        — тело (легитимный) · subject · содержимое · путь · негодная причина
#
# ТРИ СВЕРКИ, каждая — в обе стороны (`C-067` F-3: согласованная правка счётчика скрывает
# потерю сценария; `A-005` §2 поправка 3: манифест, сверяемый сам с собой, не ловит
# расхождения со спекой):
#   (1) манифест ⇄ фактически исполненное — по ИМЕНАМ, а не по числу;
#   (2) манифест ⇄ таблица §3bis.2 спеки — по СОСТАВУ (ось, вид, значение). Спека —
#       единственный источник осей и значений; значение, объявленное там и не покрытое
#       здесь, валит прогон (так была найдена дыра «база не-предок», A-005 §4 #4);
#   (3) §3bis.3(2) механически: у КАЖДОЙ оси есть легитимный сценарий — иначе набор
#       проходит реализация «запретить всё» (A-005 §4 #6: у оси 4 его не было).
#
# ОДНА ФИКСТУРА МОЖЕТ НЕСТИ НЕСКОЛЬКО CLAIM'ОВ покрытия (A1M — это и «M изменён» оси 1, и
# «push» оси 2, и «.claude/rules/**» оси 4). Каждый claim — ОТДЕЛЬНАЯ строка манифеста:
# перекрытие видно в таблице, а не подразумевается. Дублировать фикстуру ради 1:1 хуже.
#
# ПРАВИЛО ОСТАНОВКИ (§3bis.4): предъявленная дырявая реализация обязана называть ОСЬ и
# ЗНАЧЕНИЕ. Новое значение известной оси — дыра в матрице (чинится сценарием). Новая ось —
# находка другого рода: пересматривается критерий, а не латается проба.
#
# АНТИ-ПЛАЦЕБО — БАТАРЕЯ В РЕПОЗИТОРИИ: `bash scripts/tests/red_docs_freeze.sh --battery`
# строит эталонный барьер и восемь дырявых и проверяет, что проба зелёная против эталона и
# красная против каждого мутанта. Батарея лежит здесь, а не в /tmp сессии, по причине из
# `A-005` §6.5: сырые прогоны круга 1 были сняты пробой, не существовавшей ни в одной
# ревизии этого репозитория, и четыре круга этого никто не заметил. Заявление «проба ловит
# X» проверяется командой, а не читается в отчёте.
#
# ГЛАВНАЯ ЛОВУШКА: пока барьер не написан, `bash` вернёт 127, и негативные сценарии
# позеленели бы на пустом месте. Наличие и парсимость барьера проверяются ДО сценариев.

set -uo pipefail

SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/check_docs_freeze.sh}"
SPEC="${SPEC:-${ROOT}/milestones/M-60a-docs-freeze.md}"
ZERO=0000000000000000000000000000000000000000
TOKEN_OK="FOUNDER-APPROVED: перенос механизмов einhard по решению founder'а 2026-08-05"

# ─── МАНИФЕСТ: имя|ось|вид|значение ──────────────────────────────────────────────────
# вид: V — сценарий-НАРУШЕНИЕ (обязан краснеть), L — ЛЕГИТИМНЫЙ (обязан зеленеть).
# Значения обязаны СОВПАДАТЬ ПОСИМВОЛЬНО с атомами таблицы §3bis.2 спеки (сверка 2).
MANIFEST="
A1M|1|V|M изменён
A1M|2|V|push
A1M|4|V|.claude/rules/**
A1A|1|V|A добавлен
A1D|1|V|D удалён
A1R|1|V|R уведён переименованием
A2P|2|V|pull_request
A3B|3|V|база zero-SHA
A3E|3|V|база пустая
A3NP|3|V|база не-предок
A3EM|3|V|evil merge
A3RP|3|V|реверт-пара
A3C|3|V|токен позже
A3E1|3|V|токен раньше
A3S|3|V|тело merge
A4G|4|V|.claude/agents/**
A4W|4|V|.claude/wrappers/**
A4C|4|V|CLAUDE.md
A4F|4|V|docs/04-workflow.md
A5S|5|V|subject
A5C|5|V|содержимое файла
A5P|5|V|путь файла
A5Q|5|V|негодная причина
L1|1|L|правка зоны с токеном
L1|5|L|токен в теле коммита
L2|1|L|правка вне зоны
L3|2|L|pull_request с токеном
L4|3|L|merge с токеном в side-коммите
L5|4|L|не-член зоны под .claude/
"

FAILED=0; PASSED=0; EXECUTED=""; FIXTURES=""
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }
mark() { EXECUTED="${EXECUTED}$1
"; }

# Фикстуры — эфемерны. Без уборки один прогон батареи оставляет ~230 git-репозиториев в
# /tmp (за сессию оттуда вычищено 93 GB — цена уже предъявлена). KEEP_FIXTURES=1 сохраняет
# их для разбора красного прогона.
cleanup() {
  [ "${KEEP_FIXTURES:-0}" = "1" ] && { echo "фикстуры сохранены (KEEP_FIXTURES=1)" >&2; return; }
  local d
  while IFS= read -r d; do
    [ -n "$d" ] && [ -d "$d" ] && case "$d" in /tmp/red-freeze-*) rm -rf "$d";; esac
  done <<< "${FIXTURES}"
}
trap cleanup EXIT

mk_repo() {
  local d; d="$(mktemp -d "/tmp/red-freeze-$1-XXXXXX")" || die mktemp
  FIXTURES="${FIXTURES}${d}
"
  ( cd "$d" && git init -q && git config user.email a@b.c && git config user.name t \
    && mkdir -p .claude/rules .claude/agents .claude/wrappers docs \
    && echo "правило"  > .claude/rules/gates.md \
    && echo "профиль"  > .claude/agents/architect.md \
    && echo "лаунчер"  > .claude/wrappers/pi-dev.sh \
    && echo "мастер"   > CLAUDE.md \
    && echo "эталон"   > docs/04-workflow.md \
    && echo "вне зоны" > docs/DESIGN.md \
    && git add -A && git commit -q -m base ) || die "фикстура $1"
  echo "$d"
}
commit_in() { ( cd "$1" && echo "$3" >> "$2" && git add -A && git commit -q -F - <<EOF
правка $2

$4
EOF
) || die "коммит $2"; }

# Проводка совпадает с CI: база берётся ИЗ СОБЫТИЯ. push → github.event.before;
# pull_request → github.event.pull_request.base.sha. Реализация, знающая лишь одно поле,
# обязана краснеть (ось 2).
run_barrier() { ( cd "$1" && EVENT_NAME="$2" PUSH_BEFORE="$3" PR_BASE_SHA="$3" \
                  bash "${BARRIER}" >/dev/null 2>&1 ); }
run_pr() { ( cd "$1" && EVENT_NAME=pull_request PUSH_BEFORE="" PR_BASE_SHA="$2" \
             bash "${BARRIER}" >/dev/null 2>&1 ); }

expect_block() { mark "$1"
  if run_barrier "$2" push "$3"; then fail "$1 $4 — ПРОШЛО"; else pass "$1 $4 — заблокировано"; fi; }
expect_allow() { mark "$1"
  if run_barrier "$2" push "$3"; then pass "$1 $4 — пропущено"; else fail "$1 $4 — ложное срабатывание"; fi; }

# ═══ БАТАРЕЯ (--battery): эталон + восемь дырявых ═══════════════════════════════════
# Барьеры собираются из ЧАСТЕЙ: каждый мутант отличается от эталона ровно одной частью,
# названной в таблице ожиданий. Отличие проверяется `cmp` — sed, не нашедший якоря, дал бы
# копию эталона, и «мутант» молча тестировал бы не то (страж setup'а для самой батареи).

PART_HEAD='#!/usr/bin/env bash
set -uo pipefail
ZERO=0000000000000000000000000000000000000000
case "${EVENT_NAME:-}" in
  push)         BASE="${PUSH_BEFORE:-}";;
  pull_request) BASE="${PR_BASE_SHA:-}";;
  *)            exit 1;;
esac
[ -n "$BASE" ] || exit 1
[ "$BASE" != "$ZERO" ] || exit 1
git cat-file -e "$BASE" 2>/dev/null || exit 1'

PART_ANCESTOR='git merge-base --is-ancestor "$BASE" HEAD 2>/dev/null || exit 1'
PART_ANCESTOR_NONE='# (мутант existsbase: проверки предка НЕТ)'

PART_ZONE_EXACT='in_zone() { case "$1" in
  .claude/rules/*|.claude/agents/*|.claude/wrappers/*|CLAUDE.md|docs/04-workflow.md) return 0;;
  *) return 1;; esac; }'
PART_ZONE_BROAD='in_zone() { case "$1" in
  .claude/*|CLAUDE.md|docs/04-workflow.md) return 0;;
  *) return 1;; esac; }'

PART_TOK_BODY='tok() { git log -1 --format="%b" "$1" | grep -qE "^FOUNDER-APPROVED: .{12,}"; }'
PART_TOK_SUBJ='tok() { git log -1 --format="%s%n%b" "$1" | grep -qE "^FOUNDER-APPROVED: .{12,}"; }'
PART_TOK_SHOW='tok() { git show "$1" | grep -qE "FOUNDER-APPROVED: .{12,}"; }'

PART_TOUCH_SHOWCC='touches() { local f
  while IFS= read -r f; do
    [ -n "$f" ] && in_zone "$f" && return 0
  done < <(git show --cc --name-only --no-renames --format= "$1")
  return 1; }'
PART_TOUCH_TREEDIFF='touches() { local f
  while IFS= read -r f; do
    [ -n "$f" ] && in_zone "$f" && return 0
  done < <(git diff-tree -r --no-commit-id --name-only --no-renames "$1")
  return 1; }'

PART_LOOP_PERCOMMIT='for c in $(git rev-list "$BASE..HEAD"); do
  if touches "$c"; then tok "$c" || exit 1; fi
done
exit 0'
PART_LOOP_EARLYTOK='seen=0
for c in $(git rev-list --reverse "$BASE..HEAD"); do
  if tok "$c"; then seen=1; continue; fi
  if touches "$c"; then [ "$seen" = "1" ] || exit 1; fi
done
exit 0'

emit_barrier() { # $1=файл $2=ancestor $3=zone $4=tok $5=touch $6=loop
  printf '%s\n%s\n%s\n%s\n%s\n%s\n' "${PART_HEAD}" "$2" "$3" "$4" "$5" "$6" > "$1"
  bash -n "$1" || die "сгенерированный барьер не парсится: $1"
}

run_battery() {
  local d rc bad=0 checks=0
  d="$(mktemp -d /tmp/red-freeze-battery-XXXXXX)" || die mktemp
  FIXTURES="${FIXTURES}${d}
"
  emit_barrier "$d/ref.sh"        "${PART_ANCESTOR}"      "${PART_ZONE_EXACT}" "${PART_TOK_BODY}" "${PART_TOUCH_SHOWCC}"   "${PART_LOOP_PERCOMMIT}"
  emit_barrier "$d/showgrep.sh"   "${PART_ANCESTOR}"      "${PART_ZONE_EXACT}" "${PART_TOK_SHOW}" "${PART_TOUCH_SHOWCC}"   "${PART_LOOP_PERCOMMIT}"
  emit_barrier "$d/subjtok.sh"    "${PART_ANCESTOR}"      "${PART_ZONE_EXACT}" "${PART_TOK_SUBJ}" "${PART_TOUCH_SHOWCC}"   "${PART_LOOP_PERCOMMIT}"
  emit_barrier "$d/earlytok.sh"   "${PART_ANCESTOR}"      "${PART_ZONE_EXACT}" "${PART_TOK_BODY}" "${PART_TOUCH_SHOWCC}"   "${PART_LOOP_EARLYTOK}"
  emit_barrier "$d/existsbase.sh" "${PART_ANCESTOR_NONE}" "${PART_ZONE_EXACT}" "${PART_TOK_BODY}" "${PART_TOUCH_SHOWCC}"   "${PART_LOOP_PERCOMMIT}"
  emit_barrier "$d/treediff.sh"   "${PART_ANCESTOR}"      "${PART_ZONE_EXACT}" "${PART_TOK_BODY}" "${PART_TOUCH_TREEDIFF}" "${PART_LOOP_PERCOMMIT}"
  emit_barrier "$d/overbroad.sh"  "${PART_ANCESTOR}"      "${PART_ZONE_BROAD}" "${PART_TOK_BODY}" "${PART_TOUCH_SHOWCC}"   "${PART_LOOP_PERCOMMIT}"
  printf '#!/usr/bin/env bash\nexit 0\n' > "$d/always0.sh"
  printf '#!/usr/bin/env bash\nexit 1\n' > "$d/always1.sh"

  # Страж батареи: мутант, совпавший с эталоном, тестировал бы эталон под чужим именем.
  local m
  for m in showgrep subjtok earlytok existsbase treediff overbroad; do
    cmp -s "$d/ref.sh" "$d/$m.sh" && die "мутант $m НЕ ПОСТРОЕН — совпал с эталоном"
  done

  echo "══ БАТАРЕЯ (A-005 §10): эталон обязан быть зелёным, каждый мутант — красным ══"
  echo "барьеры: $d"
  echo
  # эталон
  BARRIER="$d/ref.sh" bash "${SELF}" > "$d/ref.log" 2>&1; rc=$?
  checks=$((checks + 1))
  if [ $rc -eq 0 ]; then echo "PASS  эталон → exit=0  $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' "$d/ref.log" | head -1)"
  else echo "FAIL  эталон → exit=$rc (позитивный контроль сломан)"; bad=$((bad + 1))
       grep -E '^(FAIL|SETUP)' "$d/ref.log" | head -8 | sed 's/^/      ↳ /'; fi

  # мутанты: ось и значение, ради которых мутант построен (правило §3bis.4)
  for m in "showgrep:ось 5 / содержимое файла" "subjtok:ось 5 / subject" \
           "earlytok:ось 3 / токен раньше" "existsbase:ось 3 / база не-предок" \
           "treediff:ось 3 / evil merge" "overbroad:ось 4 / нет легитимного сценария" \
           "always0:вырожденный — пропускает всё" "always1:вырожденный — блокирует всё"; do
    local name="${m%%:*}" why="${m##*:}"
    BARRIER="$d/$name.sh" bash "${SELF}" > "$d/$name.log" 2>&1; rc=$?
    checks=$((checks + 1))
    if [ $rc -ne 0 ]; then
      echo "PASS  $name → exit=$rc  $(grep -oE 'VERDICT: FAIL \([0-9]+\)' "$d/$name.log" | head -1)  [$why]"
    else
      echo "FAIL  $name ПРОШЁЛ пробу (exit=0) — дыра: $why"; bad=$((bad + 1))
    fi
  done

  # страж setup'а: без барьера проба не имеет права быть зелёной
  BARRIER="$d/НЕТ-ТАКОГО.sh" bash "${SELF}" > "$d/nobar.log" 2>&1; rc=$?
  checks=$((checks + 1))
  if [ $rc -ne 0 ] && grep -q 'SETUP НЕ СОСТОЯЛСЯ' "$d/nobar.log"; then
    echo "PASS  без барьера → exit=$rc, «SETUP НЕ СОСТОЯЛСЯ» (страж на месте)"
  else
    echo "FAIL  без барьера → exit=$rc — проба зеленеет на пустом месте"; bad=$((bad + 1))
  fi

  echo
  if [ "$bad" -gt 0 ]; then echo "BATTERY: FAIL (${bad} из ${checks})"; return 1; fi
  echo "BATTERY: PASS (${checks}/${checks}) — эталон зелён, все мутанты красные, страж жив"
  return 0
}

if [ "${1:-}" = "--battery" ]; then run_battery; exit $?; fi

# ═══ СТРАЖ SETUP'А ══════════════════════════════════════════════════════════════════
[ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}. Проба НЕ имеет права быть зелёной,
  пока гейт не существует: 127 от bash неотличим от честного отказа гейта."
bash -n "${BARRIER}" 2>/dev/null || die "барьер не парсится — сценарии мерили бы ошибку интерпретатора."
[ -f "${SPEC}" ] || die "спеки нет: ${SPEC}. Состав осей и значений сверять не с чем."

echo "── Замок процессного слоя (M-60a rev6): покрытие ПЯТИ ОСЕЙ ──"
echo "барьер: ${BARRIER}"
echo "спека:  ${SPEC}"
echo

# ─── ОСЬ 1 — способ изменения зоны ───────────────────────────────────────────────────
R="$(mk_repo a1m)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "без разрешения"
expect_block A1M "$R" "$B" "изменение запертого файла без токена (ось 1 M · ось 2 push · ось 4 rules)"

R="$(mk_repo a1a)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && echo "новое правило" > .claude/rules/new-rule.md && git add -A \
  && git commit -q -m "добавление правила без разрешения" ) || die a1a
expect_block A1A "$R" "$B" "ДОБАВЛЕНИЕ файла в зону без токена"

R="$(mk_repo a1d)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git rm -q .claude/rules/gates.md && git commit -q -m "снос без разрешения" ) || die a1d
expect_block A1D "$R" "$B" "УДАЛЕНИЕ запертого файла без токена"

R="$(mk_repo a1r)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git mv .claude/rules/gates.md docs/moved.md && git commit -q -m "увод без разрешения" ) || die a1r
expect_block A1R "$R" "$B" "УВОД из зоны переименованием без токена"

R="$(mk_repo l1)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "${TOKEN_OK}"
expect_allow L1 "$R" "$B" "правка зоны С разрешением founder'а (ось 1 · ось 5: токен в ТЕЛЕ)"

R="$(mk_repo l2)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" "docs/DESIGN.md" "архитектурная правка" "обычное тело"
expect_allow L2 "$R" "$B" "правка ВНЕ зоны без токена"

# ─── ОСЬ 2 — тип события ─────────────────────────────────────────────────────────────
# Реализация, читающая только `github.event.before`, на pull_request не увидит диапазона
# и пропустит всё. Ось закрывается парой: нарушение и легитимный случай.
R="$(mk_repo a2p)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "без разрешения"
mark A2P
if run_pr "$R" "$B"; then fail "A2P правка зоны в pull_request без токена — ПРОШЛА"
else pass "A2P pull_request без токена — заблокировано"; fi

R="$(mk_repo l3)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "${TOKEN_OK}"
mark L3
if run_pr "$R" "$B"; then pass "L3 pull_request С токеном — пропущено"
else fail "L3 pull_request с токеном — ложное срабатывание"; fi

# ─── ОСЬ 3 — форма истории (значения выведены из кванторов инварианта, A-005 §2) ─────
# (в) токен в чужом коммите: тело merge
R="$(mk_repo a3s)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q -b side && echo "норма" >> .claude/rules/testing.md \
  && git add -A && git commit -q -m "side: правка без разрешения" && git checkout -q - \
  && git merge -q --no-ff side -m "merge side

${TOKEN_OK}" ) || die a3s
expect_block A3S "$R" "$B" "токен в теле MERGE не покрывает side-коммит"

# (в) токен в чужом коммите: токен ПОЗЖЕ
R="$(mk_repo a3c)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "без разрешения"
commit_in "$R" "docs/DESIGN.md" "постороннее" "${TOKEN_OK}"
expect_block A3C "$R" "$B" "токен в ЧУЖОМ коммите диапазона не действует задним числом"

# (в) токен в чужом коммите: токен РАНЬШЕ. Был в rev1-rev3 как FR-5 и ПОТЕРЯН при переписи
# rev4 (A-005 §8.6) — реализация, наследующая токен более раннего коммита, пропускает
# неодобренную правку, уехавшую следом в том же push.
R="$(mk_repo a3e1)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" "docs/DESIGN.md" "одобренное постороннее" "${TOKEN_OK}"
commit_in "$R" ".claude/rules/gates.md" "норма следом" "тело без разрешения"
expect_block A3E1 "$R" "$B" "токен РАНЬШЕ не покрывает правку, уехавшую позже"

# (а) диапазон вычислен неверно: база zero-SHA / пустая
R="$(mk_repo a3b)"; commit_in "$R" ".claude/rules/gates.md" "норма" "без разрешения"
expect_block A3B "$R" "$ZERO" "недостоверная база (zero-SHA) ⇒ fail-closed"
expect_block A3E "$R" ""      "недостоверная база (пустая) ⇒ fail-closed"

# (а) база существует, но НЕ предок HEAD (force-push откат). Значение было объявлено в
# спеке §3bis.2 и не покрыто ни одним сценарием — найдено A-005 §4 #4.
R="$(mk_repo a3np)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "${TOKEN_OK}"
OLD="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git reset --hard -q "$B" ) || die a3np
expect_block A3NP "$R" "$OLD" "база существует, но НЕ предок HEAD ⇒ fail-closed"

# (б) коммит пропущен обходом: evil merge — правка зоны живёт ТОЛЬКО в дереве merge-коммита,
# ни в одном родителе. Плоский `diff-tree` на merge молчит; честная реализация — `git show --cc`.
R="$(mk_repo a3em)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q -b evil && echo "ветка" >> docs/DESIGN.md \
  && git add -A && git commit -q -m "side: вне зоны" && git checkout -q - \
  && git merge -q --no-ff --no-commit evil >/dev/null 2>&1
  cd "$R" && echo "внесено САМИМ merge" >> .claude/rules/gates.md && git add -A \
  && git commit -q -m "merge evil" ) || die a3em
expect_block A3EM "$R" "$B" "evil merge: правка зоны внесена самим merge-коммитом"

# (б) коммит пропущен обходом: реверт-пара — net-diff чист, но коммит без токена БЫЛ.
# Семантика per-commit нормативна (§3bis.5).
R="$(mk_repo a3rp)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "тело без разрешения"
( cd "$R" && git revert --no-edit HEAD >/dev/null 2>&1 ) || die a3rp
expect_block A3RP "$R" "$B" "реверт-пара: net-diff чист, но коммит без токена в диапазоне был"

R="$(mk_repo l4)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q -b side2 && echo "норма" >> .claude/rules/testing.md \
  && git add -A && git commit -q -F - <<EOF
side: правка правил

${TOKEN_OK}
EOF
  git checkout -q - && git merge -q --no-ff side2 -m "merge side2" ) || die l4
expect_allow L4 "$R" "$B" "легитимный merge: токен в САМОМ side-коммите"

# ─── ОСЬ 4 — член зоны ──────────────────────────────────────────────────────────────
# Зона — не один путь, а пять. Реализация, стерегущая префикс `.claude/rules/`, прошла бы
# оси 1-3 почти целиком. `.claude/rules/**` покрыт A1M (claim в манифесте), остальные —
# отдельными сценариями; `CLAUDE.md` получил СВОЙ сценарий (A4C): прежде он был прикрыт
# только A1Q, где блокировка наступает из-за негодного токена, а не из-за членства в зоне.
for member in ".claude/agents/architect.md:A4G" ".claude/wrappers/pi-dev.sh:A4W" \
              "CLAUDE.md:A4C" "docs/04-workflow.md:A4F"; do
  path="${member%%:*}"; name="${member##*:}"
  R="$(mk_repo "${name}")"; B="$(cd "$R" && git rev-parse HEAD)"
  commit_in "$R" "${path}" "правка" "без разрешения"
  expect_block "${name}" "$R" "$B" "правка ${path} без токена"
done

# Легитимный случай оси 4 (нарушение §3bis.3(2), найдено A-005 §4 #6): без него набор
# проходила реализация с зоной, расширенной до всего `.claude/**`, краснея на файле,
# который членом зоны не является.
R="$(mk_repo l5)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && echo '{}' > .claude/settings.json && git add -A \
  && git commit -q -m "правка настроек харнеса без разрешения" ) || die l5
expect_allow L5 "$R" "$B" "НЕ-член зоны под .claude/ (settings.json) без токена"

# ─── ОСЬ 5 — носитель токена ────────────────────────────────────────────────────────
# Спека §1 требует токен в ТЕЛЕ коммита. Реализация, ищущая его шире, авторизует не то.
R="$(mk_repo a5s)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && echo "норма" >> .claude/rules/gates.md && git add -A \
  && git commit -q -m "${TOKEN_OK}" ) || die a5s
expect_block A5S "$R" "$B" "токен в SUBJECT, тело пустое — не разрешение"

# A5C — САМОАВТОРИЗУЮЩИЙСЯ КОММИТ: токен лежит в СОДЕРЖИМОМ запертого файла; реализация,
# ищущая его в выводе `git show` (текст диффа), авторизует правку сама собой (A-005 §4 #1).
R="$(mk_repo a5c)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && printf '%s\n' "${TOKEN_OK}" >> .claude/rules/gates.md && git add -A \
  && git commit -q -m "правка правил" ) || die a5c
expect_block A5C "$R" "$B" "токен в СОДЕРЖИМОМ файла — самоавторизация запрещена"

# A5P — токен в ПУТИ добавленного файла. Тот же класс, что A5C: `git show`/`--name-only`
# печатают путь, и реализация, грепающая вывод целиком, считает его разрешением.
R="$(mk_repo a5p)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && echo "норма" >> .claude/rules/gates.md \
  && echo "прикрытие" > "docs/${TOKEN_OK}.md" && git add -A \
  && git commit -q -m "правка правил" ) || die a5p
expect_block A5P "$R" "$B" "токен в ПУТИ файла — не разрешение"

# A5Q — токен в правильном носителе (тело), но негодный: причина короче 12 символов.
# Разрешение не должно быть ритуалом. Значение живёт на оси 5, потому что ось отвечает на
# вопрос «ЧЕМ авторизовано» (A-005 §4): негодная причина не авторизует ничем.
R="$(mk_repo a5q)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" "CLAUDE.md" "правка" "FOUNDER-APPROVED: ок"
expect_block A5Q "$R" "$B" "токен без внятной причины (<12 символов)"

# ═══ СВЕРКА 1 — манифест ⇄ фактически исполненное (по ИМЕНАМ, в обе стороны) ════════
echo
DECL_NAMES="$(printf '%s' "${MANIFEST}" | grep '|' | cut -d'|' -f1 | sort -u)"
RUN_NAMES="$(printf '%s' "${EXECUTED}" | grep . | sort -u)"
MISS="$(comm -23 <(printf '%s\n' "${DECL_NAMES}") <(printf '%s\n' "${RUN_NAMES}") | tr '\n' ' ')"
EXTRA="$(comm -13 <(printf '%s\n' "${DECL_NAMES}") <(printf '%s\n' "${RUN_NAMES}") | tr '\n' ' ')"
if [ -n "${MISS// /}" ] || [ -n "${EXTRA// /}" ]; then
  [ -n "${MISS// /}" ]  && { echo "FAIL  МАНИФЕСТ: объявлены, но НЕ исполнены: ${MISS}"; FAILED=$((FAILED + 1)); }
  [ -n "${EXTRA// /}" ] && { echo "FAIL  МАНИФЕСТ: исполнены, но НЕ объявлены: ${EXTRA}"; FAILED=$((FAILED + 1)); }
else
  echo "PASS  МАНИФЕСТ ⇄ исполнение: $(printf '%s\n' "${RUN_NAMES}" | grep -c .) сценариев, состав совпал в обе стороны"
fi

# ═══ СВЕРКА 2 — манифест ⇄ таблица §3bis.2 спеки (состав ЗНАЧЕНИЙ) ══════════════════
# Спека — единственный источник осей и значений (A-005 §2, поправка 3). Машиночитаемы
# атомы в обратных кавычках: колонка 2 — нарушения (V), колонка 3 — легитимные (L).
spec_pairs() {
  awk -F'|' '
    function emit(a, kind, cell,   n, p, i) {
      n = split(cell, p, "`")
      for (i = 2; i <= n; i += 2) if (p[i] != "") print a "|" kind "|" p[i]
    }
    /^#/ { inside = ($0 ~ /^### 3bis\.2/); next }
    inside && /^\|[[:space:]]*\*\*[0-9]+\./ {
      if (!match($2, /\*\*[0-9]+\./)) next
      axis = substr($2, RSTART + 2, RLENGTH - 3)
      emit(axis, "V", $3); emit(axis, "L", $4)
    }
  ' "$1" | sort -u
}
SPEC_PAIRS="$(spec_pairs "${SPEC}")"
MAN_PAIRS="$(printf '%s' "${MANIFEST}" | grep '|' | cut -d'|' -f2- | sort -u)"
SPEC_AXES="$(printf '%s' "${SPEC_PAIRS}" | grep . | cut -d'|' -f1 | sort -u)"
[ -n "$(printf '%s' "${SPEC_PAIRS}" | grep . )" ] \
  || die "таблица §3bis.2 в ${SPEC} не разобрана — сверять состав не с чем.
  Ожидается: строки вида '| **N. Ось** | \`значение\` · … | \`легитимное\` | … |'"

ONLY_SPEC="$(comm -23 <(printf '%s\n' "${SPEC_PAIRS}") <(printf '%s\n' "${MAN_PAIRS}"))"
ONLY_MAN="$(comm -13 <(printf '%s\n' "${SPEC_PAIRS}") <(printf '%s\n' "${MAN_PAIRS}"))"
if [ -n "${ONLY_SPEC}" ] || [ -n "${ONLY_MAN}" ]; then
  [ -n "${ONLY_SPEC}" ] && { echo "FAIL  СПЕКА⇄МАНИФЕСТ: объявлено в §3bis.2, НЕ покрыто сценарием:";
    printf '%s\n' "${ONLY_SPEC}" | sed 's/^/        ось /'; FAILED=$((FAILED + 1)); }
  [ -n "${ONLY_MAN}" ] && { echo "FAIL  СПЕКА⇄МАНИФЕСТ: покрыто сценарием, НЕ объявлено в §3bis.2:";
    printf '%s\n' "${ONLY_MAN}" | sed 's/^/        ось /'; FAILED=$((FAILED + 1)); }
else
  echo "PASS  СПЕКА⇄МАНИФЕСТ: $(printf '%s\n' "${SPEC_PAIRS}" | grep -c .) пар (ось,вид,значение) совпали в обе стороны"
fi

# ═══ СВЕРКА 3 — §3bis.3(2) механически: у каждой оси есть легитимный сценарий ═══════
NOLEGIT=""
while IFS= read -r ax; do
  [ -z "${ax}" ] && continue
  printf '%s\n' "${MAN_PAIRS}" | grep -q "^${ax}|L|" || NOLEGIT="${NOLEGIT} ${ax}"
done <<< "${SPEC_AXES}"
if [ -n "${NOLEGIT// /}" ]; then
  echo "FAIL  §3bis.3(2): у осей${NOLEGIT} нет легитимного сценария — набор проходит «запретить всё»"
  FAILED=$((FAILED + 1))
else
  echo "PASS  §3bis.3(2): у каждой из осей ($(printf '%s' "${SPEC_AXES}" | tr '\n' ' ')) есть легитимный сценарий"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Полнота заявлена ОТНОСИТЕЛЬНО пяти осей (спека §3bis.2);"
  echo "опровержение обязано называть ОСЬ и ЗНАЧЕНИЕ (§3bis.4)."
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/${PASSED}) — все значения пяти осей покрыты, состав сверен со спекой"
