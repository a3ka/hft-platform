#!/usr/bin/env bash
# RED-проба барьера защищённых артефактов (блокер B1, C-006).
#
# ЗАЧЕМ. Барьер `scripts/check_protected_artifacts.sh` проверяет ПРАВИЛЬНУЮ вещь (артефакт,
# который существовал, обязан существовать на HEAD), но был подключён к событию НЕВЕРНО:
# `ci.yml` звал его как `check_protected_artifacts.sh origin/main`, а на `push`-событии
# `actions/checkout` ставит `origin/main` на ТОЛЬКО ЧТО ЗАПУШЕННЫЙ коммит ⇒
# `merge-base(origin/main, HEAD) == HEAD` ⇒ диапазон пуст ⇒ **PASS всегда**.
# PR в этом репо не используются (все прогоны — event=push на main), поэтому барьер не
# срабатывал НИКОГДА: коммит, сносящий вердикт критика, проходил CI зелёным. Ложный гейт
# хуже отсутствующего — он создаёт ощущение защиты.
#
# ЭТА ПРОБА ЗОВЁТ БАРЬЕР РОВНО ТАК, КАК ЕГО ЗОВЁТ CI (через env события), а не «как удобно».
# Гейт, проверенный не тем вызовом, каким его дёргает прод, — не проверен.
#
# АНТИ-ПЛАЦЕБО: против пред-фиксной проводки проба ОБЯЗАНА ПАДАТЬ (сценарий P2 даёт exit=0
# там, где обязан быть отказ). Проверяется так:
#   git show 2aaa870:scripts/check_protected_artifacts.sh > /tmp/old.sh
#   BARRIER=/tmp/old.sh bash scripts/tests/red_protected_artifacts.sh   # → FAIL, и это правильно
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/check_protected_artifacts.sh}"

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

# Песочница: настоящий git-репозиторий, где мы воспроизводим семантику GitHub-события.
new_repo() {
  local d
  d=$(mktemp -d)
  git -C "${d}" init -q
  git -C "${d}" config user.email t@t.local
  git -C "${d}" config user.name t
  mkdir -p "${d}/research/critiques" "${d}/milestones" "${d}/docs/rfc"
  echo "вердикт критика" > "${d}/research/critiques/C-001.md"
  echo "спека" > "${d}/milestones/M-01.md"
  echo "контракт" > "${d}/docs/rfc/CT-RFC-01.md"
  echo "код" > "${d}/src.rs"
  git -C "${d}" add -A >/dev/null
  git -C "${d}" commit -qm "base: артефакты гейтов на месте"
  # Так выглядит main ДО пуша. Это и есть `github.event.before`.
  git -C "${d}" branch -f main HEAD >/dev/null
  echo "${d}"
}

# Вызов барьера ровно как из ci.yml: событие + его база приходят через env.
#
# ВАЖНО для честности пробы: перед вызовом мы воспроизводим то, что делает `actions/checkout`
# на push-событии — ставит `refs/remotes/origin/main` на ТОЛЬКО ЧТО ЗАПУШЕННЫЙ коммит (HEAD).
# Без этого песочница «добрее» прода: старый барьер падал бы с exit=128 (нет origin/main), и
# проба зачла бы КРАХ скрипта как отказ гейта — то есть мерила бы не то. С этим ref'ом
# пред-фиксный барьер ведёт себя ровно как в CI: merge-base(origin/main, HEAD) == HEAD ⇒
# диапазон пуст ⇒ PASS всегда.
run_barrier() { # $1=repo $2=event $3=before/base-sha
  git -C "$1" update-ref refs/remotes/origin/main HEAD
  ( cd "$1" && EVENT_NAME="$2" PUSH_BEFORE="$3" PR_BASE_SHA="$3" bash "${BARRIER}" >/dev/null 2>&1 )
  echo $?
}

expect() { # $1=имя $2=ожидаемый-исход(ok|deny) $3=actual-exit
  if [ "$2" = "ok" ] && [ "$3" -eq 0 ]; then pass "$1"
  elif [ "$2" = "deny" ] && [ "$3" -ne 0 ]; then pass "$1"
  else fail "$1 — exit=$3, ожидалось $2"; fi
}

# ── SETUP ОБЯЗАН БЫТЬ FAIL-CLOSED (блокер rev9, critic) ───────────────────────────────
# P15 «подмена симлинком» НЕ СОЗДАВАЛА симлинк: `git rm` удалял единственный файл в
# `research/critiques/`, каталог исчезал, `ln -s` падал с «No such file or directory» — а проба
# молча продолжала и печатала PASS, тестируя на самом деле обычное удаление. То есть проба,
# написанная ПРОТИВ плацебо-гейтов, сама оказалась плацебо: несостоявшийся setup зачитывался
# за успех. Теперь сценарий обязан ДОКАЗАТЬ, что подготовил ровно то состояние, которое
# собирается проверять; не доказал — FAIL, а не «ну и ладно».
head_mode() { git -C "$1" ls-tree HEAD -- "$2" 2>/dev/null | awk '{print $1}'; }

setup_is() { # $1=repo $2=path $3=ожидаемый-режим $4=имя-сценария → 0, если состояние подготовлено
  local m; m=$(head_mode "$1" "$2")
  if [ "${m}" != "$3" ]; then
    fail "$4 — SETUP НЕ СОСТОЯЛСЯ: по пути $2 режим '${m:-<нет>}', ожидался $3. \
Проба тестировала бы НЕ ТО, что заявляет (ровно плацебо, ради которого её и писали)"
    return 1
  fi
  return 0
}

# ── P1: чистый push (артефакты не тронуты) — барьер обязан ПРОПУСТИТЬ ─────────────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
echo "правка" >> "$r/src.rs"; git -C "$r" commit -qam "feat: обычная правка кода"
expect "P1 чистый push пропускается" ok "$(run_barrier "$r" push "$before")"

# ── P2: ГЛАВНЫЙ (B1) — push-коммит СНОСИТ вердикт критика ─────────────────────────────
# Ровно тот инцидент, ради которого барьер писался (139b399 удалил C-006 вместе с §8-правками).
# Пред-фиксная проводка возвращала здесь 0.
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" rm -q research/critiques/C-001.md; git -C "$r" commit -qm "docs: §8 пруфы (и вердикт уехал вместе с git commit -a)"
expect "P2 удаление вердикта в push-коммите ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"

# ── P3: rename защищённого пути В НЕЗАЩИЩЁННЫЙ — исчезновение под видом переезда ──────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
mkdir -p "$r/notes"; git -C "$r" mv research/critiques/C-001.md notes/C-001.md
git -C "$r" commit -qm "chore: прибрал каталог"
expect "P3 переезд артефакта из-под защиты ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"

# ── P4: переезд в ДРУГОЙ защищённый путь — легитимная миграция, пропускается ──────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" mv docs/rfc/CT-RFC-01.md docs/rfc/CT-RFC-01-renamed.md
git -C "$r" commit -qm "docs: переименование RFC внутри защиты"
expect "P4 переезд внутри защиты пропускается" ok "$(run_barrier "$r" push "$before")"

# ── P5: осознанное удаление с override В ТЕЛЕ ТОГО ЖЕ коммита ─────────────────────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" rm -q milestones/M-01.md
git -C "$r" commit -qm "chore: удаляю милестоун

ALLOW-ARTIFACT-DELETE: спека слита в M-02, согласовано founder'ом"
expect "P5 ALLOW-ARTIFACT-DELETE в том же коммите пропускается" ok "$(run_barrier "$r" push "$before")"

# ── P6: override в ЧУЖОМ коммите диапазона не легитимизирует удаление ─────────────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" rm -q milestones/M-01.md; git -C "$r" commit -qm "chore: удаляю милестоун (без обоснования)"
echo x >> "$r/src.rs"; git -C "$r" commit -qam "feat: другой коммит

ALLOW-ARTIFACT-DELETE: обоснование не в том коммите"
expect "P6 override в чужом коммите НЕ спасает" deny "$(run_barrier "$r" push "$before")"

# ── P7: «злой мерж» — файл выброшен мержем, ни один коммит его не удалял ──────────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" checkout -qb side
echo "правка" >> "$r/src.rs"; git -C "$r" commit -qam "side: правка"
git -C "$r" checkout -q main >/dev/null 2>&1 || git -C "$r" checkout -q -
git -C "$r" merge -q --no-ff -m "merge: side" side >/dev/null 2>&1
git -C "$r" rm -q research/critiques/C-001.md
git -C "$r" commit -q --amend --no-edit >/dev/null 2>&1   # артефакт исчез ВНУТРИ merge-коммита
expect "P7 артефакт, выброшенный мержем, ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"

# ── P8: fail-closed — zero-SHA before (создание ветки / force-push) ───────────────────
# «Базы нет» НЕ ЗНАЧИТ «проверять нечего»: это значит, что мы не можем гарантировать целостность.
r=$(new_repo); git -C "$r" rm -q research/critiques/C-001.md; git -C "$r" commit -qm "снос под видом новой ветки"
expect "P8 zero-SHA база — fail-closed (не пропуск)" deny \
  "$(run_barrier "$r" push "0000000000000000000000000000000000000000")"

# ── P9: fail-closed — база не предок HEAD (история переписана force-push'ем) ──────────
r=$(new_repo); orphan=$(new_repo); alien=$(git -C "$orphan" rev-parse HEAD)
git -C "$r" rm -q research/critiques/C-001.md; git -C "$r" commit -qm "снос при переписанной истории"
expect "P9 база не предок HEAD — fail-closed" deny "$(run_barrier "$r" push "${alien}")"

# ── P10: fail-closed — событие не задано (барьер зовут «как удобно», а не как CI) ─────
r=$(new_repo)
expect "P10 без события — fail-closed" deny "$(run_barrier "$r" "" "")"

# ── P11 (rev7): артефакт РОДИЛСЯ В SIDE-ВЕТКЕ, пришёл мержем — и удалён потом ─────────
# Дыра, которую нашёл критик: множество «существовавших» собиралось через
# `git log --diff-filter=AR`, а git log НЕ показывает диффы merge-коммитов ⇒ артефакт,
# пришедший мержем, барьер не видел вовсе и его удаление пропускал (exit=0).
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" checkout -qb side2
echo "вердикт, рождённый в ветке" > "$r/research/critiques/C-002.md"
git -C "$r" add research/critiques/C-002.md >/dev/null; git -C "$r" commit -qm "critic: вердикт C-002"
git -C "$r" checkout -q -; git -C "$r" merge -q --no-ff -m "merge: side2 (вердикт приезжает мержем)" side2
git -C "$r" rm -q research/critiques/C-002.md; git -C "$r" commit -qm "docs: правки (вердикт уехал за компанию)"
expect "P11 артефакт, пришедший МЕРЖЕМ, и удалённый потом — ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"

# ── P12 (rev7): артефакт СОЗДАН ПРЯМО В ТЕЛЕ merge-коммита — и удалён потом ───────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" checkout -qb side3
echo "правка" >> "$r/src.rs"; git -C "$r" commit -qam "side3: правка"
git -C "$r" checkout -q -; git -C "$r" merge -q --no-ff --no-commit side3 >/dev/null 2>&1
echo "вердикт, рождённый в мерже" > "$r/research/critiques/C-003.md"
git -C "$r" add research/critiques/C-003.md >/dev/null
git -C "$r" commit -qm "merge: side3 (+ вердикт C-003 прямо в теле мержа)"
git -C "$r" rm -q research/critiques/C-003.md; git -C "$r" commit -qm "chore: прибрал"
expect "P12 артефакт, рождённый В МЕРЖЕ, и удалённый потом — ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"

# ── P13 (rev7, ЛОЖНОЕ СРАБАТЫВАНИЕ): артефакт пришёл мержем и ЦЕЛ — пропускается ──────
# Расширение множества «существовавших» не смеет сделать барьер параноиком: honest merge,
# приносящий вердикт критика, обязан проходить.
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" checkout -qb side4
echo "вердикт" > "$r/research/critiques/C-004.md"
git -C "$r" add research/critiques/C-004.md >/dev/null; git -C "$r" commit -qm "critic: вердикт C-004"
git -C "$r" checkout -q -; git -C "$r" merge -q --no-ff -m "merge: side4" side4
expect "P13 артефакт, пришедший мержем и ЦЕЛЫЙ, пропускается (нет ложных срабатываний)" ok \
  "$(run_barrier "$r" push "$before")"

# ── P14 (rev8): файл подменён КАТАЛОГОМ на том же пути ────────────────────────────────
# `git cat-file -e HEAD:path` говорит «объект есть» и для ДЕРЕВА — артефакт уничтожен, барьер молчал.
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" rm -q research/critiques/C-001.md
mkdir -p "$r/research/critiques/C-001.md"
echo "мусор" > "$r/research/critiques/C-001.md/README.md"
git -C "$r" add research/critiques/C-001.md >/dev/null
git -C "$r" commit -qm "chore: на месте вердикта теперь каталог"
# `ls-tree HEAD -- path` для каталога отдаёт запись дерева (040000) — это и есть подмена типа.
if setup_is "$r" research/critiques/C-001.md 040000 "P14"; then
  expect "P14 файл подменён КАТАЛОГОМ — ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"
fi

# ── P15 (rev8): файл подменён СИМЛИНКОМ ───────────────────────────────────────────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" rm -q research/critiques/C-001.md
# `git rm` унёс единственный файл ⇒ каталога больше нет. Без mkdir симлинк НЕ создавался, и
# проба тихо тестировала обычное удаление (блокер rev9). Каталог восстанавливаем ЯВНО.
mkdir -p "$r/research/critiques"
ln -s /dev/null "$r/research/critiques/C-001.md"
git -C "$r" add research/critiques/C-001.md >/dev/null
git -C "$r" commit -qm "chore: вердикт теперь симлинк в /dev/null"
if setup_is "$r" research/critiques/C-001.md 120000 "P15"; then
  expect "P15 файл подменён СИМЛИНКОМ — ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"
fi

# ── P16 (rev8): файл усечён в НОЛЬ БАЙТ — то же удаление, только вежливое ─────────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
: > "$r/research/critiques/C-001.md"
git -C "$r" commit -qam "chore: вердикт выпотрошен до нуля байт"
if setup_is "$r" research/critiques/C-001.md 100644 "P16" \
   && [ "$(git -C "$r" cat-file -s "$(git -C "$r" rev-parse HEAD:research/critiques/C-001.md)")" -eq 0 ]; then
  expect "P16 артефакт усечён в 0 байт — ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"
else
  fail "P16 — SETUP НЕ СОСТОЯЛСЯ: файл не пуст, проба тестировала бы не то"
fi

# ── P17 (rev8, ЛОЖНОЕ СРАБАТЫВАНИЕ): обычная правка содержимого — пропускается ────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
echo "дополнение вердикта" >> "$r/research/critiques/C-001.md"
git -C "$r" commit -qam "critic: дополнил вердикт"
expect "P17 правка содержимого артефакта пропускается (нет ложных срабатываний)" ok \
  "$(run_barrier "$r" push "$before")"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Барьер не даёт заявленной гарантии. Пока проба красная, gates.md §9 обещает то,"
  echo "чего в пайплайне нет — а это хуже отсутствия правила."
  exit 1
fi
echo "VERDICT: PASS (17/17) — барьер держит при ТОЙ ЖЕ проводке, какой его зовёт CI"
