#!/usr/bin/env bash
# red_commit_paths.sh — проба барьера `.githooks/pre-commit`.
#
# ПРЕДМЕТ. `git commit` без явных путей коммитит ВЕСЬ индекс, а `git add <файл>` перед ним
# коммит не сужает — он лишь ДОБАВЛЯЕТ к уже лежащему. 13.08 это унесло шесть файлов вместо
# одного и откатило работу другого агента в общей ветке.
#
# ПОЧЕМУ ПРОБА, А НЕ ДОВЕРИЕ К ХУКУ. Барьер, не проверенный исполнением, — описание намерения:
# `test -x .githooks/pre-commit` зелен и у хука, который пропускает всё. Проба гоняет ТУ ЖЕ
# форму вызова, какой пользуется человек и агент, и требует обоих исходов: запрещённая форма
# ОТВЕРГНУТА и НИЧЕГО не закоммичено; каждая законная форма ПРОШЛА.
#
# АНТИ-ПЛАЦЕБО ВСТРОЕНО: последний сценарий снимает хук и требует, чтобы запрещённая форма
# прошла. Без него весь набор зелен и против сломанного git, и против пустой песочницы.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HOOK="${HOOK_UNDER_TEST:-${ROOT}/.githooks/pre-commit}"
FAILED=0
RAN=0
EXPECT_SCENARIOS=8

pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
setup_fail() { echo "SETUP НЕ СОСТОЯЛСЯ  $*"; FAILED=$((FAILED + 1)); }

[ -x "$HOOK" ] || { echo "SETUP НЕ СОСТОЯЛСЯ: нет исполняемого хука $HOOK"; exit 1; }

# ─── песочница: свой git-репозиторий с установленным хуком ───────────────────────────────
# Каталог свой, НЕ /tmp/<короткое имя>: машина общая с посторонними пользователями, и
# фиксированный путь уже стоил коммита с чужим телом (13.08).
mk_repo() {
  local s hooks
  s="$(mktemp -d "${TMPDIR:-/tmp}/red-commitpaths-XXXXXX")" || return 1
  git init -q "$s/repo" >/dev/null 2>&1 || return 1
  hooks="$s/hooks"
  mkdir -p "$hooks"
  cp "$HOOK" "$hooks/pre-commit"
  chmod +x "$hooks/pre-commit"
  (
    cd "$s/repo" || exit 1
    git config core.hooksPath "$hooks"
    git config user.email probe@local
    git config user.name probe
    printf 'base\n' >tracked.txt
    printf 'base\n' >theirs1.txt
    printf 'base\n' >theirs2.txt
    git add tracked.txt theirs1.txt theirs2.txt
    # Первый коммит делается в обход хука: предмет пробы — последующие коммиты.
    git commit -q --no-verify -m init
  ) || return 1
  printf '%s\n' "$s"
}

# Грязный индекс: две ЧУЖИЕ правки уже лежат, агент добавляет свою.
dirty_index() {
  local r="$1"
  ( cd "$r" || exit 1
    printf 'theirs-edit\n' >>theirs1.txt
    printf 'theirs-edit\n' >>theirs2.txt
    git add theirs1.txt theirs2.txt
    printf 'mine\n' >>tracked.txt
    git add tracked.txt )
}

files_in_head() { git -C "$1" show --numstat --format='' HEAD | wc -l | tr -d ' '; }

# ─── 1. ЗАПРЕЩЁННАЯ ФОРМА: git commit без путей при грязном индексе ──────────────────────
RAN=$((RAN + 1))
S="$(mk_repo)" || setup_fail "песочница NOPATHS"
R="$S/repo"
dirty_index "$R"
BEFORE="$(git -C "$R" rev-parse HEAD)"
OUT="$(cd "$R" && git commit -m "должно быть отвергнуто" 2>&1)"; RC=$?
AFTER="$(git -C "$R" rev-parse HEAD)"
if [ "$RC" -ne 0 ] && [ "$BEFORE" = "$AFTER" ]; then
  pass "NOPATHS: \`git commit\` без путей отвергнут (exit=$RC), HEAD не сдвинулся"
else
  fail "NOPATHS: exit=$RC, HEAD $( [ "$BEFORE" = "$AFTER" ] && echo 'на месте' || echo 'СДВИНУЛСЯ' ) \
— барьер пропустил форму, которая берёт весь индекс. В коммит ушло файлов: $(files_in_head "$R")"
fi
rm -rf "$S"

# ─── 2. ЗАКОННАЯ ФОРМА: явный путь отслеживаемого файла ──────────────────────────────────
RAN=$((RAN + 1))
S="$(mk_repo)" || setup_fail "песочница EXPLICIT"
R="$S/repo"
dirty_index "$R"
OUT="$(cd "$R" && git commit -m "явный путь" -- tracked.txt 2>&1)"; RC=$?
N="$(files_in_head "$R")"
if [ "$RC" -eq 0 ] && [ "$N" = "1" ]; then
  pass "EXPLICIT: \`git commit -- tracked.txt\` прошёл, в коммите ровно 1 файл"
else
  fail "EXPLICIT: exit=$RC, файлов в коммите=$N (ожидался 1). Барьер обязан пропускать законную \
форму — иначе он приучает обходить себя \`--no-verify\`, и это хуже отсутствия барьера"
  printf '%s\n' "$OUT" | head -3 | sed 's/^/      ↳ /'
fi
rm -rf "$S"

# ─── 3. НОВЫЙ ФАЙЛ: add обязателен, и это ДОМИНИРУЮЩИЙ случай зоны ───────────────────────
# Каждый артефакт гейта (`R-NNN.md`, милестоун, RED-файл) рождается новым. Первая редакция
# правила требовала `git commit -- <новый>` и была для них НЕИСПОЛНИМА: pathspec не совпадает.
RAN=$((RAN + 1))
S="$(mk_repo)" || setup_fail "песочница NEWFILE"
R="$S/repo"
dirty_index "$R"
( cd "$R" && printf 'new artifact\n' >R-999-probe.md && git add R-999-probe.md )
OUT="$(cd "$R" && git commit -m "новый артефакт" -- R-999-probe.md 2>&1)"; RC=$?
N="$(files_in_head "$R")"
if [ "$RC" -eq 0 ] && [ "$N" = "1" ]; then
  pass "NEWFILE: \`git add <новый> && git commit -- <новый>\` прошёл, в коммите ровно 1 файл"
else
  fail "NEWFILE: exit=$RC, файлов=$N. Новый файл — доминирующий случай: если барьер его не \
пропускает, правило неисполнимо для каждого артефакта гейта"
  printf '%s\n' "$OUT" | head -3 | sed 's/^/      ↳ /'
fi
rm -rf "$S"

# ─── 4. `commit -a` — уже запрещён правилами, обязан отвергаться и барьером ───────────────
RAN=$((RAN + 1))
S="$(mk_repo)" || setup_fail "песочница COMMIT-A"
R="$S/repo"
( cd "$R" && printf 'theirs\n' >>theirs1.txt && printf 'mine\n' >>tracked.txt )
BEFORE="$(git -C "$R" rev-parse HEAD)"
OUT="$(cd "$R" && git commit -am "должно быть отвергнуто" 2>&1)"; RC=$?
AFTER="$(git -C "$R" rev-parse HEAD)"
if [ "$RC" -ne 0 ] && [ "$BEFORE" = "$AFTER" ]; then
  pass "COMMIT-A: \`git commit -a\` отвергнут (exit=$RC)"
else
  fail "COMMIT-A: exit=$RC, HEAD $( [ "$BEFORE" = "$AFTER" ] && echo 'на месте' || echo 'СДВИНУЛСЯ' ). \
Пропуск по белому списку обязан быть УЗКИМ: \`commit -a\` идёт под \`index.lock\` и проскочит, \
если пропускать всё, что не равно \`index\`"
fi
rm -rf "$S"

# ─── 5. `--amend` только сообщения: staged пуст, барьер не при чём ───────────────────────
RAN=$((RAN + 1))
S="$(mk_repo)" || setup_fail "песочница AMEND"
R="$S/repo"
OUT="$(cd "$R" && git commit --amend -m "переписанное сообщение" 2>&1)"; RC=$?
if [ "$RC" -eq 0 ]; then
  pass "AMEND: правка только сообщения прошла (staged пуст — не предмет барьера)"
else
  fail "AMEND: exit=$RC — барьер мешает законной операции; сегодня переписывание сообщения \
понадобилось дважды за сутки"
  printf '%s\n' "$OUT" | head -3 | sed 's/^/      ↳ /'
fi
rm -rf "$S"

# ─── 6. MERGE: pre-commit при слиянии не вызывается вовсе ────────────────────────────────
# 82 merge-коммита за месяц. Если барьер их ломает, он ломает штатную работу reviewer'а.
RAN=$((RAN + 1))
S="$(mk_repo)" || setup_fail "песочница MERGE"
R="$S/repo"
(
  cd "$R" || exit 1
  git checkout -q -b feat
  printf 'feat\n' >feat.txt
  git add feat.txt
  git commit -q --no-verify -m feat
  git checkout -q -
  printf 'main-side\n' >main.txt
  git add main.txt
  git commit -q --no-verify -m mainside
) || setup_fail "MERGE: ветки не собрались"
OUT="$(cd "$R" && git merge --no-ff -m "merge feat" feat 2>&1)"; RC=$?
if [ "$RC" -eq 0 ]; then
  pass "MERGE: слияние прошло — барьер штатную работу не ломает"
else
  fail "MERGE: exit=$RC — барьер ломает merge; 82 таких коммита за месяц"
  printf '%s\n' "$OUT" | head -3 | sed 's/^/      ↳ /'
fi
rm -rf "$S"

# ─── 7. СООБЩЕНИЕ ОБЯЗАНО НАЗЫВАТЬ ЛЕКАРСТВО, А НЕ ТОЛЬКО ЗАПРЕТ ────────────────────────
# Отказ, не говорящий КАК правильно, приучает обходить себя `--no-verify`.
RAN=$((RAN + 1))
S="$(mk_repo)" || setup_fail "песочница MESSAGE"
R="$S/repo"
dirty_index "$R"
OUT="$(cd "$R" && git commit -m x 2>&1)"
if printf '%s' "$OUT" | grep -q 'git commit -- ' && printf '%s' "$OUT" | grep -q 'theirs1.txt'; then
  pass "MESSAGE: отказ называет форму лекарства И перечисляет, что лежит в индексе"
else
  fail "MESSAGE: отказ не называет либо форму \`git commit -- <путь>\`, либо содержимое индекса. \
Барьер, говорящий «нельзя» без «как можно», обходят"
  printf '%s\n' "$OUT" | head -4 | sed 's/^/      ↳ /'
fi
rm -rf "$S"

# ─── 8. АНТИ-ПЛАЦЕБО: без хука запрещённая форма ПРОХОДИТ ────────────────────────────────
# Без этого сценария весь набор зелен и против пустой песочницы, и против git, который
# почему-то отказывает по своей причине.
RAN=$((RAN + 1))
S="$(mk_repo)" || setup_fail "песочница NOHOOK"
R="$S/repo"
rm -f "$S/hooks/pre-commit"
dirty_index "$R"
BEFORE="$(git -C "$R" rev-parse HEAD)"
OUT="$(cd "$R" && git commit -m "без хука обязано пройти" 2>&1)"; RC=$?
AFTER="$(git -C "$R" rev-parse HEAD)"
N="$(files_in_head "$R")"
if [ "$RC" -eq 0 ] && [ "$BEFORE" != "$AFTER" ] && [ "$N" = "3" ]; then
  pass "NOHOOK (анти-плацебо): без барьера форма проходит и уносит все 3 файла индекса — \
сценарий 1 краснеет ИМЕННО от барьера"
else
  fail "NOHOOK: exit=$RC, файлов=$N (ожидалось 3). Если без хука форма НЕ проходит, сценарий 1 \
зелен по чужой причине, и вся проба ничего не доказывает"
fi
rm -rf "$S"

# ─── число исполненного СЧИТАЕТСЯ, а не заявляется ───────────────────────────────────────
echo
if [ "$RAN" -ne "$EXPECT_SCENARIOS" ]; then
  echo "FAIL  исполнено ${RAN} сценариев при объявленных ${EXPECT_SCENARIOS}"
  FAILED=$((FAILED + 1))
fi
if [ "$FAILED" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений из ${RAN} сценариев)"
  exit 1
fi
echo "VERDICT: PASS (${RAN}/${EXPECT_SCENARIOS} сценариев)"
