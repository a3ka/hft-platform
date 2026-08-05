#!/usr/bin/env bash
# Проба привязки вердикта к предмету — `scripts/check_gate_meta.sh` (M-60 G3).
#
# ЗАЧЕМ — два инцидента, оба наши.
#
# 1. C-062 (2026-08-04). Критик отработал круг в дереве ЧУЖОГО репозитория, честно доложил
#    аномалию («нет .claude/rules/gates.md») — и вывода не сделал ни он, ни диспетчер.
#    В шапке вердикта стоит `Base: origin/main @ 9a0e48f0` — ревизия, которой в НАШЕЙ истории
#    нет вовсе. Норма «работай в своём репозитории» существовала только в голове. GM-3/GM-4 —
#    машинный реплей этого случая: вердикт с чужим repo и с несуществующей ревизией не пройдёт.
#
# 2. Подмена предмета ПОСЛЕ вердикта. Проходной вердикт по одному HEAD прикрывает merge
#    другого: «критик смотрел это» и «reviewer одобряет то же самое» перестают совпадать.
#    Лечится subject-lock'ом: диф `audited_head..HEAD` не смеет трогать пути класса «гейт».
#    Лок применяется ТОЛЬКО к проходным исходам — после REJECT правки штатны, и лок,
#    красящий нормальный круг, был бы вреднее отсутствующего (GM-11).
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
# Проверки: поля непусты; audited_repo == origin ЭТОГО репо; audited_base и audited_head
# существуют в ЭТОЙ истории; audited_head — предок HEAD; для проходных исходов —
# subject-lock. Выход из лока: `ALLOW-SUBJECT-CHANGE: <причина>` в теле коммита диапазона.
# База — ИЗ СОБЫТИЯ; пустая/zero/не-предок ⇒ FAIL (блокер B1, C-006).
# Файлы вердиктов ВНЕ диапазона не трогаются: ретроспективно править 60+ защищённых
# артефактов ради формы — вред, а не польза.

set -uo pipefail

ROOT_REPO="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT_REPO}/scripts/check_gate_meta.sh}"
ZERO=0000000000000000000000000000000000000000
GATE_CLASS_FILE="scripts/verify_M-99.sh"

FAILED=0
PASSED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

[ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}. 127 от bash неотличим от честного отказа гейта."
bash -n "${BARRIER}" 2>/dev/null || die "барьер не парсится — сценарии мерили бы ошибку интерпретатора."

# ── Фикстура ──────────────────────────────────────────────────────────────────────────
mk_repo() {
  local d; d="$(mktemp -d /tmp/red-gatemeta-XXXXXX)" || die mktemp
  ( cd "$d" && git init -q \
    && git config user.email a@b.c && git config user.name t \
    && git remote add origin https://github.com/a3ka/hft-platform.git \
    && mkdir -p research/critiques scripts .claude/rules docs .github/workflows \
    && echo base > docs/DESIGN.md && echo base > scripts/verify_M-99.sh \
    && echo base > .claude/rules/gates.md && echo base > .github/workflows/ci.yml \
    && git add -A && git commit -q -m base ) || die "инициализация фикстуры"
  echo "$d"
}

# $1=repo $2=verdict-строка $3=audited_repo $4=audited_base $5=audited_head [$6=имя]
add_verdict() {
  local r="$1" name="${6:-C-999-test.md}"
  { echo "<!-- GATE-META"
    echo "milestone: M-99"
    echo "audited_repo: $3"
    echo "audited_base: $4"
    echo "audited_head: $5"
    echo "verdict: $2"
    echo "-->"
    echo
    echo "тело вердикта"
  } > "$r/research/critiques/$name" || die "запись вердикта"
  ( cd "$r" && git add -A && git commit -q -m "docs(critic): вердикт $name" ) || die "коммит вердикта"
}

touch_file() { # $1=repo $2=путь $3=тело-коммита
  ( cd "$1" && echo "правка" >> "$2" && git add -A && git commit -q -F - <<EOF
правка $2

$3
EOF
  ) || die "коммит правки $2"
}

run_barrier() { # $1=repo $2=before-sha
  ( cd "$1" && EVENT_NAME=push PUSH_BEFORE="$2" PR_BASE_SHA="$2" bash "${BARRIER}" >/dev/null 2>&1 )
}
head_of() { ( cd "$1" && git rev-parse HEAD ); }

echo "── Привязка вердикта к предмету + subject-lock (M-60 G3): 15 сценариев ──"
echo "барьер: ${BARRIER}"
echo

# ── Блок 1: форма и принадлежность предмету ──────────────────────────────────────────

# GM-1 — вердикт без шапки
R="$(mk_repo)"; B="$(head_of "$R")"
echo "вердикт без метаданных" > "$R/research/critiques/C-001.md"
( cd "$R" && git add -A && git commit -q -m "вердикт без шапки" ) || die "GM-1"
run_barrier "$R" "$B" && fail "GM-1 вердикт БЕЗ шапки прошёл — привязки к предмету нет" \
                      || pass "GM-1 вердикт без GATE-META заблокирован"

# GM-2 — поле пустое
R="$(mk_repo)"; B="$(head_of "$R")"; H="$B"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" ""
run_barrier "$R" "$B" && fail "GM-2 пустой audited_head прошёл — шапка стала ритуалом" \
                      || pass "GM-2 пустое поле шапки отвергнуто"

# GM-3 — ЧУЖОЙ репозиторий (реплей C-062)
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "einhardsystems/einhard-runtime" "$B" "$B"
run_barrier "$R" "$B" && fail "GM-3 вердикт по ЧУЖОМУ репозиторию прошёл — C-062 повторим" \
                      || pass "GM-3 чужой audited_repo заблокирован (реплей C-062)"

# GM-4 — audited_head, которого в нашей истории НЕТ (реплей C-062: 9a0e48f0)
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "9a0e48f09a0e48f09a0e48f09a0e48f09a0e48f0"
run_barrier "$R" "$B" && fail "GM-4 несуществующая ревизия прошла — вердикт судил чужую историю" \
                      || pass "GM-4 несуществующий audited_head заблокирован (реплей C-062)"

# GM-5 — audited_base, которого нет
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" "$B"
run_barrier "$R" "$B" && fail "GM-5 несуществующий audited_base прошёл" \
                      || pass "GM-5 несуществующий audited_base заблокирован"

# GM-6 — audited_head существует, но НЕ предок HEAD (side-ветка)
R="$(mk_repo)"; B="$(head_of "$R")"
( cd "$R" && git checkout -q -b side && echo x >> docs/DESIGN.md \
  && git add -A && git commit -q -m side ) || die "GM-6 side"
SIDE="$(cd "$R" && git rev-parse HEAD)"
( cd "$R" && git checkout -q - ) || die "GM-6 back"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$SIDE"
run_barrier "$R" "$B" && fail "GM-6 audited_head не из этой линии истории прошёл" \
                      || pass "GM-6 audited_head вне линии истории заблокирован"

# GM-7 — всё корректно, после вердикта ничего не менялось
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$B"
run_barrier "$R" "$B" && pass "GM-7 корректный вердикт проходит" \
                      || fail "GM-7 ложное срабатывание на корректной шапке"

# GM-8 — старый вердикт, не тронутый в диапазоне ⇒ требований нет
R="$(mk_repo)"
echo "старый вердикт без шапки" > "$R/research/critiques/C-000-old.md"
( cd "$R" && git add -A && git commit -q -m "старый вердикт" ) || die "GM-8"
B="$(head_of "$R")"
touch_file "$R" "docs/DESIGN.md" "правка вне вердиктов"
run_barrier "$R" "$B" && pass "GM-8 старые вердикты вне диапазона не трогаются" \
                      || fail "GM-8 гейт требует шапку ретроспективно — правка защищённых артефактов"

# GM-9 — база не установлена достоверно ⇒ fail-closed
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "einhardsystems/einhard-runtime" "$B" "$B"
run_barrier "$R" "$ZERO" && fail "GM-9 zero-SHA база дала ПРОПУСК" || pass "GM-9 zero-SHA: fail-closed"
run_barrier "$R" ""      && fail "GM-9b пустая база дала ПРОПУСК" || pass "GM-9b пустая база: fail-closed"

# ── Блок 2: subject-lock ─────────────────────────────────────────────────────────────

# GM-10 — APPROVE, после него тронут verify-скрипт ⇒ блок
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${GATE_CLASS_FILE}" "правка гейта после одобрения"
run_barrier "$R" "$B" && fail "GM-10 гейт правился ПОСЛЕ APPROVE — вердикт прикрыл другой предмет" \
                      || pass "GM-10 subject-lock: правка гейта после APPROVE заблокирована"

# GM-11 — REJECT, та же правка ⇒ ПРОХОД (после REJECT правки штатны)
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "REJECT" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${GATE_CLASS_FILE}" "исправление по вердикту"
run_barrier "$R" "$B" && pass "GM-11 после REJECT правки не блокируются (штатный круг)" \
                      || fail "GM-11 лок красит нормальный круг исправлений — вреднее отсутствия лока"

# GM-12 — APPROVE, тронута зона правил ⇒ блок
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "PASS" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" ".claude/rules/gates.md" "правка правил после одобрения"
run_barrier "$R" "$B" && fail "GM-12 правила менялись после PASS — скоуп подменён" \
                      || pass "GM-12 subject-lock накрывает .claude/rules"

# GM-13 — APPROVE + явный ALLOW-SUBJECT-CHANGE ⇒ проход со следом
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "${GATE_CLASS_FILE}" "ALLOW-SUBJECT-CHANGE: правка гейта согласована с founder'ом"
run_barrier "$R" "$B" && pass "GM-13 явный ALLOW-SUBJECT-CHANGE открывает лок (след в истории)" \
                      || fail "GM-13 легального выхода из лока нет — гейт станет обходиться силой"

# GM-14 — APPROVE, тронут файл ВНЕ класса «гейт» ⇒ проход (анти-ложноположительный)
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "APPROVE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" "docs/DESIGN.md" "обычная правка документа"
run_barrier "$R" "$B" && pass "GM-14 правка вне класса «гейт» не блокируется" \
                      || fail "GM-14 лок вышел за свой класс — красит обычные правки"

# GM-15 — NOTE, тронута проводка CI ⇒ блок
R="$(mk_repo)"; B="$(head_of "$R")"
add_verdict "$R" "NOTE" "a3ka/hft-platform" "$B" "$(head_of "$R")"
touch_file "$R" ".github/workflows/ci.yml" "правка проводки после NOTE"
run_barrier "$R" "$B" && fail "GM-15 проводка CI менялась после NOTE — аудировался другой пайплайн" \
                      || pass "GM-15 subject-lock накрывает .github/workflows"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Привязка вердикта к предмету не работает. Пока проба красная, повторим C-062:"
  echo "вердикт выносится над историей, которой в этом репозитории нет."
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/${PASSED}) — вердикт привязан к предмету, лок держит и не красит REJECT-круг"
