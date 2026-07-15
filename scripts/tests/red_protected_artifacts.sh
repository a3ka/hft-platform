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

# ── SETUP-GUARD ДЛЯ MERGE-СЦЕНАРИЕВ (блокер rev10, critic) ────────────────────────────
# Тот же класс, что P15: `git merge`/`git commit` в песочнице могут молча не состояться (конфликт,
# checkout не туда, пустой мерж), а `expect` этого не видит — сценарий рапортует «evil merge» /
# «merge-born», хотя проверяет ЛИНЕЙНОЕ удаление. Каждый merge-сценарий обязан ДОКАЗАТЬ свою
# форму: (1) в диапазоне есть merge-коммит; (2) артефакт достиг заявленного HEAD-состояния;
# для born-in-merge — (3) артефакт действительно СУЩЕСТВОВАЛ где-то в диапазоне.
setup_has_merge() { # $1=repo $2=before $3=имя → 0, если в before..HEAD есть merge-коммит
  local n; n=$(git -C "$1" rev-list --merges "$2"..HEAD 2>/dev/null | wc -l | tr -d ' ')
  if [ "${n:-0}" -lt 1 ]; then
    fail "$3 — SETUP НЕ СОСТОЯЛСЯ: в ${2:0:7}..HEAD НЕТ merge-коммита (мерж молча не прошёл); \
проверялось бы линейное удаление, а не заявленный merge-сценарий"
    return 1
  fi
  return 0
}
setup_head_absent() { # $1=repo $2=path $3=имя → 0, если пути НЕТ на HEAD (сценарий удаления)
  if git -C "$1" cat-file -e "HEAD:$2" 2>/dev/null; then
    fail "$3 — SETUP НЕ СОСТОЯЛСЯ: $2 всё ещё на HEAD — удаление, которое сценарий обязан \
проверять, не случилось"
    return 1
  fi
  return 0
}
setup_existed_in_range() { # $1=repo $2=path $3=before $4=имя → 0, если путь был в дереве диапазона
  local c
  git -C "$1" cat-file -e "$3:$2" 2>/dev/null && return 0
  for c in $(git -C "$1" rev-list "$3"..HEAD); do
    git -C "$1" cat-file -e "${c}:$2" 2>/dev/null && return 0
  done
  fail "$4 — SETUP НЕ СОСТОЯЛСЯ: $2 не существовал НИ В ОДНОМ коммите диапазона — \
born-in-merge/side не подготовлен (add молча не прошёл)"
  return 1
}

# ── SETUP-GUARD для НЕ-merge сценариев (блокер rev11, critic) ─────────────────────────
# Тот же класс: если scenario-defining команда (git mv / git rm+trailer / выбор base) молча не
# сработала, сценарий проходит по ДРУГОЙ причине (P4 деградирует в «чистая ветка», P8/P9 — в
# «обычное удаление»), а проба рапортует заявленное покрытие. Каждый сценарий доказывает форму.
setup_renamed_within() { # $1=repo $2=старый-путь $3=новый-путь $4=before $5=имя
  # старого нет на HEAD, новый ЕСТЬ на HEAD как файл, и переименование видно в диапазоне (статус R).
  if git -C "$1" cat-file -e "HEAD:$2" 2>/dev/null; then
    fail "$5 — SETUP НЕ СОСТОЯЛСЯ: старый путь $2 всё ещё на HEAD — переименование не случилось"; return 1
  fi
  if [ "$(head_mode "$1" "$3")" != "100644" ] && [ "$(head_mode "$1" "$3")" != "100755" ]; then
    fail "$5 — SETUP НЕ СОСТОЯЛСЯ: новый путь $3 не файл на HEAD — переименование не случилось"; return 1
  fi
  if ! git -C "$1" log --diff-filter=R -M --name-status --format='' "$4"..HEAD \
        | grep -qE "^R[0-9]*	${2}	${3}$"; then
    fail "$5 — SETUP НЕ СОСТОЯЛСЯ: git не видит переименование $2 → $3 (статус R) в диапазоне — \
проверялась бы просто чистая ветка, а не rename внутри защиты"; return 1
  fi
  return 0
}
setup_deleted_with_override() { # $1=repo $2=путь $3=before $4=имя
  # артефакта нет на HEAD, и КОММИТ, удаливший его, сам несёт ALLOW-ARTIFACT-DELETE в теле.
  if git -C "$1" cat-file -e "HEAD:$2" 2>/dev/null; then
    fail "$4 — SETUP НЕ СОСТОЯЛСЯ: $2 всё ещё на HEAD — удаление не случилось, override нечего \
подтверждать"; return 1
  fi
  local c; c=$(git -C "$1" log --diff-filter=D --format='%H' "$3"..HEAD -- "$2" | head -1)
  if [ -z "${c}" ]; then
    fail "$4 — SETUP НЕ СОСТОЯЛСЯ: ни один коммит диапазона не удалял $2"; return 1
  fi
  if ! git -C "$1" log -1 --format='%B' "${c}" | grep -q '^ALLOW-ARTIFACT-DELETE:'; then
    fail "$4 — SETUP НЕ СОСТОЯЛСЯ: удаляющий коммит НЕ несёт ALLOW-ARTIFACT-DELETE — \
проверялось бы обычное удаление, а не осознанный override"; return 1
  fi
  return 0
}
setup_base_is_zero() { # $1=аргумент-базы $2=имя → 0, если это zero-SHA
  case "$1" in
    *[!0]*) fail "$2 — SETUP НЕ СОСТОЯЛСЯ: база '$1' НЕ zero-SHA — отказ пришёл бы от обычного \
удаления, а не от zero-SHA fail-closed"; return 1 ;;
    "")     fail "$2 — SETUP НЕ СОСТОЯЛСЯ: база пуста, а не zero-SHA"; return 1 ;;
    *) return 0 ;;
  esac
}
setup_base_not_ancestor() { # $1=repo $2=аргумент-базы $3=имя → 0, если база НЕ предок HEAD
  if git -C "$1" merge-base --is-ancestor "$2" HEAD 2>/dev/null; then
    fail "$3 — SETUP НЕ СОСТОЯЛСЯ: база '${2:0:7}' ЯВЛЯЕТСЯ предком HEAD — отказ пришёл бы от \
обычного удаления, а не от проверки «база не предок»"; return 1
  fi
  return 0
}
setup_source_only_commit() { # $1=repo $2=before $3=имя → 0, если диапазон непуст и изменён ТОЛЬКО незащищённый
  if [ "$(git -C "$1" rev-parse HEAD)" = "$2" ]; then
    fail "$3 — SETUP НЕ СОСТОЯЛСЯ: диапазон ПУСТ (коммит не случился) — барьер тривиально \
пропускает пустой диапазон, «чистый push» не проверен"; return 1
  fi
  local changed p; changed=$(git -C "$1" diff --name-only "$2" HEAD)
  [ -n "${changed}" ] || { fail "$3 — SETUP НЕ СОСТОЯЛСЯ: коммит ничего не изменил"; return 1; }
  for p in ${changed}; do
    if is_protected "${p}"; then
      fail "$3 — SETUP НЕ СОСТОЯЛСЯ: изменён ЗАЩИЩЁННЫЙ ${p} — это не «чистый source-only push»"
      return 1
    fi
  done
  return 0
}
setup_file_content_changed() { # $1=repo $2=path $3=before $4=имя → 0, если blob изменён и это нормальный непустой файл
  if [ "$(git -C "$1" rev-parse HEAD)" = "$3" ]; then
    fail "$4 — SETUP НЕ СОСТОЯЛСЯ: диапазон ПУСТ (правка не закоммичена) — барьер пропускает \
пустой диапазон, «правка содержимого» не проверена"; return 1
  fi
  local b0 b1; b0=$(git -C "$1" rev-parse "$3:$2" 2>/dev/null || echo A)
  b1=$(git -C "$1" rev-parse "HEAD:$2" 2>/dev/null || echo B)
  if [ "${b0}" = "${b1}" ]; then
    fail "$4 — SETUP НЕ СОСТОЯЛСЯ: содержимое $2 НЕ изменилось (тот же blob) — правка не случилась"
    return 1
  fi
  setup_is "$1" "$2" 100644 "$4" || return 1   # остался нормальным файлом (не подмена типа)
  [ "$(git -C "$1" cat-file -s "${b1}" 2>/dev/null || echo 0)" -gt 0 ] \
    || { fail "$4 — SETUP НЕ СОСТОЯЛСЯ: $2 на HEAD пуст — это уже не «правка», а выхолащивание"; return 1; }
  return 0
}

# ── P1: чистый push (артефакты не тронуты) — барьер обязан ПРОПУСТИТЬ ─────────────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
echo "правка" >> "$r/src.rs"; git -C "$r" commit -qam "feat: обычная правка кода"
if setup_source_only_commit "$r" "$before" "P1"; then
  expect "P1 чистый push пропускается" ok "$(run_barrier "$r" push "$before")"
fi

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
if setup_renamed_within "$r" docs/rfc/CT-RFC-01.md docs/rfc/CT-RFC-01-renamed.md "$before" "P4"; then
  expect "P4 переезд внутри защиты пропускается" ok "$(run_barrier "$r" push "$before")"
fi

# ── P5: осознанное удаление с override В ТЕЛЕ ТОГО ЖЕ коммита ─────────────────────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" rm -q milestones/M-01.md
git -C "$r" commit -qm "chore: удаляю милестоун

ALLOW-ARTIFACT-DELETE: спека слита в M-02, согласовано founder'ом"
if setup_deleted_with_override "$r" milestones/M-01.md "$before" "P5"; then
  expect "P5 ALLOW-ARTIFACT-DELETE в том же коммите пропускается" ok "$(run_barrier "$r" push "$before")"
fi

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
if setup_has_merge "$r" "$before" "P7" && setup_head_absent "$r" research/critiques/C-001.md "P7"; then
  expect "P7 артефакт, выброшенный мержем, ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"
fi

# ── P8: fail-closed — zero-SHA before (создание ветки / force-push) ───────────────────
# «Базы нет» НЕ ЗНАЧИТ «проверять нечего»: это значит, что мы не можем гарантировать целостность.
r=$(new_repo); git -C "$r" rm -q research/critiques/C-001.md; git -C "$r" commit -qm "снос под видом новой ветки"
ZERO=0000000000000000000000000000000000000000
if setup_base_is_zero "$ZERO" "P8"; then
  expect "P8 zero-SHA база — fail-closed (не пропуск)" deny "$(run_barrier "$r" push "$ZERO")"
fi

# ── P9: fail-closed — база не предок HEAD (история переписана force-push'ем) ──────────
r=$(new_repo)
main_br=$(git -C "$r" branch --show-current)
base_root=$(git -C "$r" rev-parse HEAD)
git -C "$r" rm -q research/critiques/C-001.md; git -C "$r" commit -qm "снос при переписанной истории"
# Расходящийся СИБЛИНГ от корня В ТОМ ЖЕ репо: объект СУЩЕСТВУЕТ в объектной базе $r (иначе барьер
# отклонил бы по ветке «объект отсутствует», а не «не предок»), но предком HEAD НЕ является
# (у alt и HEAD общий предок base_root, но alt — параллельная ветка). Так P9 бьёт именно в
# проверку «база не предок» (история переписана force-push'ем), а не в missing-object.
git -C "$r" checkout -q -b alt "${base_root}"
git -C "$r" commit -q --allow-empty -m "divergent (переписанная история)"
alt=$(git -C "$r" rev-parse HEAD)
git -C "$r" checkout -q "${main_br}"   # назад на ветку с удалением (её видит run_barrier)
if setup_base_not_ancestor "$r" "${alt}" "P9"; then
  expect "P9 база не предок HEAD — fail-closed" deny "$(run_barrier "$r" push "${alt}")"
fi

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
if setup_has_merge "$r" "$before" "P11" \
   && setup_existed_in_range "$r" research/critiques/C-002.md "$before" "P11" \
   && setup_head_absent "$r" research/critiques/C-002.md "P11"; then
  expect "P11 артефакт, пришедший МЕРЖЕМ, и удалённый потом — ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"
fi

# ── P12 (rev7): артефакт СОЗДАН ПРЯМО В ТЕЛЕ merge-коммита — и удалён потом ───────────
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" checkout -qb side3
echo "правка" >> "$r/src.rs"; git -C "$r" commit -qam "side3: правка"
git -C "$r" checkout -q -; git -C "$r" merge -q --no-ff --no-commit side3 >/dev/null 2>&1
echo "вердикт, рождённый в мерже" > "$r/research/critiques/C-003.md"
git -C "$r" add research/critiques/C-003.md >/dev/null
git -C "$r" commit -qm "merge: side3 (+ вердикт C-003 прямо в теле мержа)"
git -C "$r" rm -q research/critiques/C-003.md; git -C "$r" commit -qm "chore: прибрал"
if setup_has_merge "$r" "$before" "P12" \
   && setup_existed_in_range "$r" research/critiques/C-003.md "$before" "P12" \
   && setup_head_absent "$r" research/critiques/C-003.md "P12"; then
  expect "P12 артефакт, рождённый В МЕРЖЕ, и удалённый потом — ВАЛИТ гейт" deny "$(run_barrier "$r" push "$before")"
fi

# ── P13 (rev7, ЛОЖНОЕ СРАБАТЫВАНИЕ): артефакт пришёл мержем и ЦЕЛ — пропускается ──────
# Расширение множества «существовавших» не смеет сделать барьер параноиком: honest merge,
# приносящий вердикт критика, обязан проходить.
r=$(new_repo); before=$(git -C "$r" rev-parse HEAD)
git -C "$r" checkout -qb side4
echo "вердикт" > "$r/research/critiques/C-004.md"
git -C "$r" add research/critiques/C-004.md >/dev/null; git -C "$r" commit -qm "critic: вердикт C-004"
git -C "$r" checkout -q -; git -C "$r" merge -q --no-ff -m "merge: side4" side4
if setup_has_merge "$r" "$before" "P13" && setup_is "$r" research/critiques/C-004.md 100644 "P13"; then
  expect "P13 артефакт, пришедший мержем и ЦЕЛЫЙ, пропускается (нет ложных срабатываний)" ok \
    "$(run_barrier "$r" push "$before")"
fi

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
if setup_file_content_changed "$r" research/critiques/C-001.md "$before" "P17"; then
  expect "P17 правка содержимого артефакта пропускается (нет ложных срабатываний)" ok \
    "$(run_barrier "$r" push "$before")"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Барьер не даёт заявленной гарантии. Пока проба красная, gates.md §9 обещает то,"
  echo "чего в пайплайне нет — а это хуже отсутствия правила."
  exit 1
fi
echo "VERDICT: PASS (17/17) — барьер держит при ТОЙ ЖЕ проводке, какой его зовёт CI"
