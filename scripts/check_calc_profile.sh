#!/usr/bin/env bash
# scripts/check_calc_profile.sh — профиль расчётов меняется только со ссылкой на подпись founder'а
# и ростом версии (M-94 задача 9, `П-032` п.3; спека `docs/archive/M-94-calc-profile.md` §3.8).
#
# Инвариант: для КАЖДОГО коммита проверяемого диапазона, чей дифф (с merge-cc содержимым) трогает
# `config/calc-profile/**`:
#   1. в ТЕЛЕ коммита есть строка `CALC-PROFILE-DECISION: П-NNN` (subject и содержимое файлов
#      не считаются — самоавторизация запрещена, тот же довод, что у `check_docs_freeze.sh`);
#   2. заголовок `## П-NNN` существует в `docs/PENDING-SIGNATURE.md` НА ЭТОМ коммите;
#   3. для каждого тронутого файла профиля, у которого изменились строки `KEY=VALUE` (комментарии
#      не считаются): `CALC_PROFILE_VERSION` — целое, строго больше прежнего у того же файла
#      (первый родитель); у НОВОГО файла — строго больше версии `active.env` первого родителя;
#   4. `active.env` не удаляется (без него выдача не стартует в режиме профиля).
# Семантика покоммитная: реверт-пара без ссылки — нарушение, даже если итоговый дифф пуст.
#
# База — ИЗ СОБЫТИЯ, fail-closed (образец `check_docs_freeze.sh`, блокер B1 `C-006`): push →
# PUSH_BEFORE, pull_request → PR_BASE_SHA; пусто / zero-SHA / нет в истории / не предок HEAD ⇒ FAIL.
#
# ПРЕДЕЛ, названный честно: барьер ловит ОТСУТСТВИЕ ссылки и невозросшую версию, но не то, что
# `П-NNN` действительно разрешает ИМЕННО эти значения; заголовок `П-NNN`, добавленный тем же
# коммитом, засчитывается. Подделываемо, как `FOUNDER-APPROVED` (`gates.md` §11).
#
# Зовётся CI ровно так: `EVENT_NAME/PUSH_BEFORE/PR_BASE_SHA bash scripts/check_calc_profile.sh`
# (джоб `calc-profile`). Проба — `scripts/tests/red_calc_profile.sh`.

set -uo pipefail
ZERO=0000000000000000000000000000000000000000

# ─── БАЗА СРАВНЕНИЯ — fail-closed при ЛЮБОЙ ошибке установки ─────────────────────────
raw="${1:-}"
if [ -z "${raw}" ]; then
  case "${EVENT_NAME:-}" in
    push)         raw="${PUSH_BEFORE:-}" ;;
    pull_request) raw="${PR_BASE_SHA:-}" ;;
    "")           echo "FAIL  событие не задано (EVENT_NAME пуст) — барьер зовут не так, как его зовёт CI" >&2; exit 1 ;;
    *)            echo "FAIL  неизвестное событие '${EVENT_NAME}' — база сравнения не определена" >&2; exit 1 ;;
  esac
fi

[ -n "${raw}" ] || { echo "FAIL  база события пуста (EVENT_NAME=${EVENT_NAME:-?}) — fail-closed" >&2; exit 1; }
case "${raw}" in
  *[!0]*) : ;;  # есть хоть один ненулевой символ — не zero-SHA
  *)      echo "FAIL  база = zero-SHA (создание ветки или force-push) — целостность процессного слоя не доказуема" >&2; exit 1 ;;
esac
git rev-parse -q --verify "${raw}^{commit}" >/dev/null 2>&1 \
  || { echo "FAIL  база '${raw}' отсутствует в истории (переписана force-push'ем / поверхностный клон)" >&2; exit 1; }
git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null \
  || { echo "FAIL  база '${raw}' НЕ предок HEAD — история переписана (force-push); что в зоне менялось, недоказуемо" >&2; exit 1; }

BASE=$(git rev-parse "${raw}^{commit}")

# ─── ЗОНА ────────────────────────────────────────────────────────────────────────────
ZONE_PREFIX="config/calc-profile/"
ACTIVE="config/calc-profile/active.env"
PS="docs/PENDING-SIGNATURE.md"

zone_files() { # <коммит> → тронутые файлы зоны (merge-cc, без переименований), по строке
  local f
  while IFS= read -r -d '' f; do
    case "$f" in "${ZONE_PREFIX}"*) printf '%s\n' "$f" ;; esac
  done < <(git show --cc --name-only --no-renames -z --format= "$1")
}

kv_of() { # <ревизия> <путь> → отсортированные строки KEY=VALUE (без комментариев/пустых)
  git show "$1:$2" 2>/dev/null | sed 's/\r$//' \
    | grep -vE '^[[:space:]]*(#|$)' | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//' | LC_ALL=C sort
}

ver_of() { # <ревизия> <путь> → значение CALC_PROFILE_VERSION (как есть) или пусто
  kv_of "$1" "$2" | sed -n 's/^CALC_PROFILE_VERSION=//p' | head -1
}

is_uint() { case "$1" in ''|*[!0-9]*) return 1 ;; *) return 0 ;; esac; }

FAILS=0; JUDGED=0
bad() { echo "FAIL  $*"; FAILS=$((FAILS + 1)); }

for c in $(git rev-list --reverse "${BASE}..HEAD"); do
  files=$(zone_files "$c")
  [ -n "$files" ] || continue
  JUDGED=$((JUDGED + 1))
  short=$(git rev-parse --short "$c")
  body=$(git log -1 --format='%b' "$c")
  id=$(printf '%s\n' "$body" | sed -nE 's/^CALC-PROFILE-DECISION: (П-[0-9]+[a-z]?)[[:space:]]*$/\1/p' | head -1)
  if [ -z "$id" ]; then
    bad "$short: правка профиля расчётов без строки «CALC-PROFILE-DECISION: П-NNN» в ТЕЛЕ коммита (П-032 п.3)"
  elif ! git show "$c:$PS" 2>/dev/null | grep -qE "^## ${id}( |$)"; then
    bad "$short: ссылка ${id} — в ${PS} на этом коммите нет заголовка «## ${id}»"
  fi
  parent=$(git rev-parse -q --verify "$c^1" 2>/dev/null || true)
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    if ! git cat-file -e "$c:$f" 2>/dev/null; then
      [ "$f" = "$ACTIVE" ] && bad "$short: удалён ${ACTIVE} — выдача в режиме профиля не стартует"
      continue
    fi
    new_kv=$(kv_of "$c" "$f")
    old_kv=""; [ -n "$parent" ] && old_kv=$(kv_of "$parent" "$f")
    [ "$new_kv" = "$old_kv" ] && continue   # только комментарии — рост версии не требуется
    vnew=$(ver_of "$c" "$f")
    if ! is_uint "$vnew" || [ "$vnew" -lt 1 ]; then
      bad "$short: $f — CALC_PROFILE_VERSION «${vnew}» не целое ≥ 1"; continue
    fi
    if [ -n "$parent" ] && git cat-file -e "$parent:$f" 2>/dev/null; then
      vold=$(ver_of "$parent" "$f"); ref="прежней версии того же файла"
    elif [ -n "$parent" ] && git cat-file -e "$parent:$ACTIVE" 2>/dev/null; then
      vold=$(ver_of "$parent" "$ACTIVE"); ref="версии ${ACTIVE} родителя"
    else
      vold=0; ref="нуля (первый профиль)"
    fi
    is_uint "$vold" || vold=0
    if [ "$vnew" -le "$vold" ]; then
      bad "$short: $f — значения изменены, а CALC_PROFILE_VERSION=${vnew} не больше ${ref} (${vold})"
    fi
  done <<<"$files"
done

if [ "$FAILS" -gt 0 ]; then
  echo "VERDICT: FAIL (нарушений: ${FAILS}; коммитов с правкой профиля: ${JUDGED})"; exit 1
fi
echo "VERDICT: PASS (коммитов с правкой профиля: ${JUDGED})"
exit 0
