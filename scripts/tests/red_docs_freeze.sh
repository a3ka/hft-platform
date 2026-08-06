#!/usr/bin/env bash
# Проба замка процессного слоя — `scripts/check_docs_freeze.sh` (M-60 G0).
#
# ЗАЧЕМ. Founder 2026-08-05: «наложить запрет на дальнейшие правки этих документов без моего
# разрешения». Запрет, живущий прозой, будет нарушен не злым умыслом, а под сжатием контекста —
# ровно так у нас уже прожили три недели предписание про git-личность (TD-101), не сработав
# НИ РАЗУ. Поэтому запрет = гейт, а разрешение = предъявляемый след в теле коммита
# (`FOUNDER-APPROVED: <причина>`), который читается через год, в отличие от переписки.
#
# ЧТО ИМЕННО ПРОБУЕТСЯ (все три свойства обязательны, testing.md «Целостность гейта»):
#   1. падает против СЛОМАННОГО    — правка зоны замка без токена не проходит;
#   2. НЕ падает против нормы      — правка вне зоны и правка с токеном проходят;
#   3. падает против несостоявшегося SETUP — см. страж ниже, это здесь главная ловушка.
#
# ГЛАВНАЯ ЛОВУШКА ЭТОЙ ПРОБЫ. Пока `check_docs_freeze.sh` не написан, `bash` на него вернёт
# 127 — ненулевой код. Все негативные сценарии («ожидаю exit≠0») стали бы ЗЕЛЁНЫМИ на пустом
# месте, и проба аттестовала бы несуществующий гейт. Поэтому наличие и исполнимость барьера
# проверяются ДО сценариев и жёстко.
#
# Проводка обязана совпадать с CI: барьер зовётся с базой ИЗ СОБЫТИЯ (`github.event.before`),
# а не от `origin/main` — иначе на push-событии диапазон пуст и гейт зелен ВСЕГДА (блокер B1,
# C-006). Пустая / zero-SHA / не-предок база ⇒ FAIL, а не пропуск.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/check_docs_freeze.sh}"
ZERO=0000000000000000000000000000000000000000
TOKEN_OK="FOUNDER-APPROVED: перенос механизмов einhard по решению founder'а 2026-08-05"

FAILED=0
PASSED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

# ── Страж барьера (см. «главная ловушка» выше) ────────────────────────────────────────
[ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}. Проба НЕ имеет права быть зелёной, пока
  гейт не существует: 127 от bash неотличим от честного отказа гейта."
[ -r "${BARRIER}" ] || die "барьер нечитаем: ${BARRIER}"
bash -n "${BARRIER}" 2>/dev/null || die "барьер не парсится (синтаксис) — сценарии ниже
  мерили бы ошибку интерпретатора, а не поведение гейта."

# ── Фикстура ──────────────────────────────────────────────────────────────────────────
mk_repo() { # $1=имя → печатает путь
  local d; d="$(mktemp -d "/tmp/red-freeze-$1-XXXXXX")" || die "mktemp"
  ( cd "$d" \
    && git init -q \
    && git config user.email a@b.c && git config user.name t \
    && mkdir -p .claude/rules .claude/agents docs \
    && echo "базовое правило" > .claude/rules/gates.md \
    && echo "базовый профиль" > .claude/agents/architect.md \
    && echo "мастер-правила"  > CLAUDE.md \
    && echo "эталон"          > docs/04-workflow.md \
    && mkdir -p .claude/wrappers && echo "лаунчер" > .claude/wrappers/pi-dev.sh \
    && echo "не зона замка"   > docs/DESIGN.md \
    && git add -A && git commit -q -m "base" ) || die "инициализация фикстуры $1"
  echo "$d"
}

commit_in() { # $1=repo $2=файл $3=текст-приписка $4=тело-коммита
  ( cd "$1" && echo "$3" >> "$2" && git add -A && git commit -q -F - <<EOF
правка $2

$4
EOF
  ) || die "коммит в $1 по $2"
}

run_barrier() { # $1=repo $2=event $3=before-sha → возвращает код барьера
  ( cd "$1" && EVENT_NAME="$2" PUSH_BEFORE="$3" PR_BASE_SHA="$3" bash "${BARRIER}" >/dev/null 2>&1 )
}

# Страж сценария: убеждаемся, что фикстура ДЕЙСТВИТЕЛЬНО описывает заявленный случай.
# Без него проба может молча тестировать не тот сценарий — плацебо самой себя.
assert_touches() { # $1=repo $2=before $3=путь
  ( cd "$1" && git diff --name-only "$2"..HEAD | grep -qx "$3" ) \
    || die "фикстура не трогает $3 в диапазоне — сценарий тестировал бы не то"
}
assert_body_has() { # $1=repo $2=подстрока
  ( cd "$1" && git log -1 --format=%B | grep -q "$2" ) || die "в теле коммита нет «$2»"
}
assert_body_lacks() { # $1=repo $2=подстрока
  ( cd "$1" && git log -1 --format=%B | grep -q "$2" ) && die "в теле коммита ЕСТЬ «$2», а не должно"
  return 0
}

echo "── Замок процессного слоя (M-60a): 12 сценариев ──"
echo "барьер: ${BARRIER}"
echo

# FR-1 — правка правил БЕЗ токена ⇒ блок
R="$(mk_repo fr1)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "новая норма" "обычное тело без разрешения"
assert_touches "$R" "$B" ".claude/rules/gates.md"; assert_body_lacks "$R" "FOUNDER-APPROVED"
run_barrier "$R" push "$B" && fail "FR-1 правка .claude/rules БЕЗ токена ПРОШЛА (замка нет)" \
                           || pass "FR-1 правка .claude/rules без токена заблокирована"

# FR-2 — та же правка С токеном ⇒ проход
R="$(mk_repo fr2)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "новая норма" "${TOKEN_OK}"
assert_touches "$R" "$B" ".claude/rules/gates.md"; assert_body_has "$R" "FOUNDER-APPROVED"
run_barrier "$R" push "$B" && pass "FR-2 правка с разрешением founder'а проходит" \
                           || fail "FR-2 ложное срабатывание: токен есть, а замок блокирует"

# FR-3 — правка ВНЕ зоны замка ⇒ проход (анти-ложноположительный)
R="$(mk_repo fr3)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" "docs/DESIGN.md" "архитектурная правка" "обычное тело"
assert_touches "$R" "$B" "docs/DESIGN.md"
run_barrier "$R" push "$B" && pass "FR-3 правка вне зоны замка не трогается" \
                           || fail "FR-3 замок вышел за свою зону — красит docs/DESIGN.md"

# FR-4 — токен есть, но причина пустая/короткая ⇒ блок (токен не должен быть ритуалом)
R="$(mk_repo fr4)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" "CLAUDE.md" "правка" "FOUNDER-APPROVED: ок"
assert_body_has "$R" "FOUNDER-APPROVED"
run_barrier "$R" push "$B" && fail "FR-4 токен без внятной причины ПРОШЁЛ — разрешение стало ритуалом" \
                           || pass "FR-4 токен с пустой причиной отвергнут"

# FR-5 — два коммита, токен НЕ у того, что трогает зону ⇒ блок
R="$(mk_repo fr5)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" "docs/DESIGN.md" "правка вне зоны" "${TOKEN_OK}"
commit_in "$R" ".claude/rules/testing.md" "правка правил" "тело без разрешения"
run_barrier "$R" push "$B" && fail "FR-5 токен из ЧУЖОГО коммита прикрыл правку правил" \
                           || pass "FR-5 токен действует только на свой коммит"

# FR-6 — профиль агента ⇒ зона замка
R="$(mk_repo fr6)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/agents/architect.md" "смена зоны роли" "тело без разрешения"
assert_touches "$R" "$B" ".claude/agents/architect.md"
run_barrier "$R" push "$B" && fail "FR-6 .claude/agents вне замка — зону роли можно менять молча" \
                           || pass "FR-6 .claude/agents под замком"

# FR-7 — эталон-конституция ⇒ зона замка
R="$(mk_repo fr7)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" "docs/04-workflow.md" "смена маршрута" "тело без разрешения"
run_barrier "$R" push "$B" && fail "FR-7 docs/04-workflow.md вне замка — маршруты можно менять молча" \
                           || pass "FR-7 docs/04-workflow.md под замком"

# FR-8 — база не установлена достоверно ⇒ FAIL-CLOSED, а не пропуск
R="$(mk_repo fr8)"; commit_in "$R" ".claude/rules/gates.md" "правка" "тело без разрешения"
run_barrier "$R" push "$ZERO" && fail "FR-8 zero-SHA база дала ПРОПУСК (force-push обходит замок)" \
                              || pass "FR-8 zero-SHA база: fail-closed"
run_barrier "$R" push ""      && fail "FR-8b пустая база дала ПРОПУСК" \
                              || pass "FR-8b пустая база: fail-closed"

# FR-9 — правка зоны, приехавшая merge-коммитом из side-ветки ⇒ блок
R="$(mk_repo fr9)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q -b side \
  && echo "норма из ветки" >> .claude/rules/scope-guard.md 2>/dev/null || echo "норма" > .claude/rules/scope-guard.md
  cd "$R" && git add -A && git commit -q -m "side: правка правил без разрешения" \
  && git checkout -q - && git merge -q --no-ff side -m "merge side" ) || die "фикстура merge"
run_barrier "$R" push "$B" && fail "FR-9 правка правил, внесённая merge'ем, обошла замок" \
                           || pass "FR-9 merge не является лазейкой"

# FR-10 — запертая правка РАНЬШЕ, токен на HEAD от постороннего коммита ⇒ блок.
# Найдено критиком (C-065): реализация, читающая тело ТОЛЬКО HEAD-коммита, проходила
# прежнюю пробу 10/10 — потому что во всех сценариях запертая правка лежала в HEAD, и
# FR-5 блокировал её по неверной причине. Сценарий «правка раньше, токен позже» —
# единственный, который такую реализацию отличает от честной.
R="$(mk_repo fr10)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/rules/gates.md" "норма без разрешения" "тело без токена"
commit_in "$R" "docs/DESIGN.md" "посторонняя правка" "${TOKEN_OK}"
assert_touches "$R" "$B" ".claude/rules/gates.md"; assert_body_has "$R" "FOUNDER-APPROVED"
run_barrier "$R" push "$B" && fail "FR-10 токен на ПОСТОРОННЕМ HEAD-коммите прикрыл правку правил, сделанную раньше" \
                           || pass "FR-10 токен не действует задним числом на прежние коммиты"

# FR-11 — запертая правка в side-ветке, токен в теле MERGE-коммита ⇒ блок.
# Вторая половина той же дыры: реализация, смотрящая merge-коммит, пропустит
# неразрешённый side-commit (C-065, F-064-6 круга 1).
R="$(mk_repo fr11)"; B="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q -b side \
  && echo "норма из ветки" >> .claude/rules/testing.md \
  && git add -A && git commit -q -m "side: правка правил без разрешения" \
  && git checkout -q - \
  && git merge -q --no-ff side -m "merge side

${TOKEN_OK}" ) || die "фикстура FR-11"
run_barrier "$R" push "$B" && fail "FR-11 токен в теле MERGE-коммита прикрыл неразрешённый side-commit" \
                           || pass "FR-11 токен merge-коммита не покрывает коммиты side-ветки"

# FR-12 — обвязка агентов ⇒ зона замка (C-065, блокер 3).
# `.claude/wrappers/pi-dev.sh` бутстрапит worktree и инжектит `dispatch-mandate.md` в
# систем-промт пяти внешних ролей: поведение агента задаётся и профилем, И обвязкой.
# Запереть `.claude/agents/**`, оставив обвязку открытой, — непоследовательно.
R="$(mk_repo fr12)"; B="$(cd "$R" && git rev-parse HEAD)"
commit_in "$R" ".claude/wrappers/pi-dev.sh" "смена бутстрапа" "тело без разрешения"
assert_touches "$R" "$B" ".claude/wrappers/pi-dev.sh"
run_barrier "$R" push "$B" && fail "FR-12 .claude/wrappers вне замка — обвязку агентов можно менять молча" \
                           || pass "FR-12 .claude/wrappers под замком"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Замок не даёт заявленной гарантии. Пока проба красная, запрет founder'а существует"
  echo "только прозой — то есть ровно в том виде, против которого он и вводился."
  exit 1
fi
# Число СЧИТАЕТСЯ, а не заявляется (урок R-032: литерал «17 сценариев» пережил добавление
# сценариев и врал о покрытии).
echo "VERDICT: PASS (${PASSED}/${PASSED}) — замок держит при ТОЙ ЖЕ проводке, какой его зовёт CI"
