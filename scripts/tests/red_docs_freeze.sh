#!/usr/bin/env bash
# Проба замка процессного слоя — `scripts/check_docs_freeze.sh` (M-60a, rev4).
#
# ИНВАРИАНТ ФОРМУЛИРУЕТСЯ ОТ РЕЗУЛЬТАТА (спека §3bis.1):
#   ни одно изменение зоны замка не попадает в проверяемый диапазон без токена
#   `FOUNDER-APPROVED` в СВОЁМ коммите — независимо от способа изменения, типа события и
#   формы истории.
#
# ПОЧЕМУ ПЕРЕПИСАНО ОТ ОСЕЙ. Четыре круга критика дали четыре REJECT, и все — о пробе, а не
# о замысле: греп ловился комментарием, разбор — именем шага, набор был слеп к удалению и
# переименованию, затем к типу события. Перечисление обходов дна не имеет. Полнота теперь
# заявляется ОТНОСИТЕЛЬНО ЧЕТЫРЁХ ОСЕЙ (спека §3bis.2), и каждое значение каждой оси покрыто:
#   ось 1 «способ изменения зоны»  — M изменён · A добавлен · D удалён · R уведён
#   ось 2 «тип события»            — push · pull_request
#   ось 3 «форма истории»          — прямой коммит · side+merge · недостоверная база
#   ось 4 «член зоны»              — rules · agents · wrappers · CLAUDE.md · 04-workflow
# Плюс легитимные случаи на каждой оси: без них проходит реализация «запретить всё».
#
# ПРАВИЛО ОСТАНОВКИ (§3bis.4): предъявленная дырявая реализация обязана называть ОСЬ и
# ЗНАЧЕНИЕ. Новое значение известной оси — дыра в матрице (чинится сценарием). Новая ось —
# находка другого рода: пересматривается критерий, а не латается проба.
#
# МАНИФЕСТ вместо счётчика (`C-067` F-3): сверяется СОСТАВ исполненных сценариев по именам,
# а не их число — согласованная правка счётчика скрыла бы потерю сценария.
#
# ГЛАВНАЯ ЛОВУШКА: пока барьер не написан, `bash` вернёт 127, и негативные сценарии
# позеленели бы на пустом месте. Наличие и парсимость барьера проверяются ДО сценариев.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/check_docs_freeze.sh}"
ZERO=0000000000000000000000000000000000000000
TOKEN_OK="FOUNDER-APPROVED: перенос механизмов einhard по решению founder'а 2026-08-05"

# Манифест: имя|ось|значение. Сверяется с фактически исполненным в конце.
MANIFEST="
A1M|1 способ|M изменён
A1A|1 способ|A добавлен
A1D|1 способ|D удалён
A1R|1 способ|R уведён переименованием
A1Q|1 способ|M с негодным токеном
A2P|2 событие|pull_request
A3S|3 история|side-commit под merge с токеном
A3C|3 история|токен в чужом коммите диапазона
A3B|3 история|недостоверная база zero-SHA
A3E|3 история|недостоверная база пустая
L1|1 способ|легитимная правка с токеном
L2|1 способ|легитимная правка ВНЕ зоны
L3|2 событие|легитимный pull_request с токеном
L4|3 история|легитимный merge, токен в side-коммите
A4G|4 член зоны|.claude/agents
A4W|4 член зоны|.claude/wrappers
A4F|4 член зоны|docs/04-workflow.md
"

FAILED=0; PASSED=0; EXECUTED=""
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }
mark() { EXECUTED="${EXECUTED}$1
"; }

[ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}. Проба НЕ имеет права быть зелёной,
  пока гейт не существует: 127 от bash неотличим от честного отказа гейта."
bash -n "${BARRIER}" 2>/dev/null || die "барьер не парсится — сценарии мерили бы ошибку интерпретатора."

mk_repo() {
  local d; d="$(mktemp -d "/tmp/red-freeze-$1-XXXXXX")" || die mktemp
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

echo "── Замок процессного слоя (M-60a rev4): покрытие ЧЕТЫРЁХ ОСЕЙ ──"
echo "барьер: ${BARRIER}"
echo

# ─── ОСЬ 1 — способ изменения зоны ───────────────────────────────────────────────────
R="$(mk_repo a1m)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "без разрешения"
expect_block A1M "$R" "$B" "изменение запертого файла без токена"

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

R="$(mk_repo a1q)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" "CLAUDE.md" "правка" "FOUNDER-APPROVED: ок"
expect_block A1Q "$R" "$B" "токен без внятной причины (разрешение не должно быть ритуалом)"

R="$(mk_repo l1)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "${TOKEN_OK}"
expect_allow L1 "$R" "$B" "правка зоны С разрешением founder'а"

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

# ─── ОСЬ 3 — форма истории ───────────────────────────────────────────────────────────
R="$(mk_repo a3s)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q -b side && echo "норма" >> .claude/rules/testing.md \
  && git add -A && git commit -q -m "side: правка без разрешения" && git checkout -q - \
  && git merge -q --no-ff side -m "merge side

${TOKEN_OK}" ) || die a3s
expect_block A3S "$R" "$B" "токен в теле MERGE не покрывает side-коммит"

R="$(mk_repo a3c)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма" "без разрешения"
commit_in "$R" "docs/DESIGN.md" "постороннее" "${TOKEN_OK}"
expect_block A3C "$R" "$B" "токен в ЧУЖОМ коммите диапазона не действует задним числом"

R="$(mk_repo a3b)"; commit_in "$R" ".claude/rules/gates.md" "норма" "без разрешения"
expect_block A3B "$R" "$ZERO" "недостоверная база (zero-SHA) ⇒ fail-closed"
expect_block A3E "$R" ""      "недостоверная база (пустая) ⇒ fail-closed"

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
# оси 1-3 почти целиком: A1M/A1A/A1D/A1R/L1 трогают именно rules, а CLAUDE.md прикрыт
# только A1Q. Ось найдена при перестройке набора (см. спеку §3bis.2, примечание).
for member in ".claude/agents/architect.md:A4G" ".claude/wrappers/pi-dev.sh:A4W" "docs/04-workflow.md:A4F"; do
  path="${member%%:*}"; name="${member##*:}"
  R="$(mk_repo "${name}")"; B="$(cd "$R" && git rev-parse HEAD)"
  commit_in "$R" "${path}" "правка" "без разрешения"
  expect_block "${name}" "$R" "$B" "правка ${path} без токена"
done

# ─── Сверка МАНИФЕСТА (состав, а не число) ───────────────────────────────────────────
echo
DECL=$(printf '%s' "${MANIFEST}" | grep -c '|')
RUN=$(printf '%s' "${EXECUTED}" | grep -c .)
MISS=""
while IFS='|' read -r name axis val; do
  [ -z "${name}" ] && continue
  printf '%s' "${EXECUTED}" | grep -qx "${name}" || MISS="${MISS} ${name}(ось ${axis}/${val})"
done <<< "$(printf '%s' "${MANIFEST}" | grep '|')"
if [ -n "${MISS}" ]; then
  echo "FAIL  МАНИФЕСТ: объявлены, но не исполнены:${MISS}"
  FAILED=$((FAILED + 1))
elif [ "${RUN}" -ne "${DECL}" ]; then
  echo "FAIL  МАНИФЕСТ: исполнено ${RUN} сценариев при ${DECL} объявленных"
  FAILED=$((FAILED + 1))
else
  echo "PASS  МАНИФЕСТ: ${RUN}/${DECL}, покрытие сверено ПО СОСТАВУ, а не по счёту"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Полнота заявлена ОТНОСИТЕЛЬНО четырёх осей (спека §3bis.2);"
  echo "опровержение обязано называть ОСЬ и ЗНАЧЕНИЕ."
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/${PASSED}) — покрыты все значения четырёх осей + легитимные случаи"
