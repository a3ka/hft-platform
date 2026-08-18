#!/usr/bin/env bash
# check_milestone_shape.sh — барьер формы milestone-спеки.
#
# ЗАЧЕМ (замер, а не принцип). `docs/04-workflow.md` §6 задаёт форму milestone-контракта, но
# механизма у неё не было. 2026-08-18 architect подал набор M-69 без раздела `Allowed paths`;
# критик отклонил его (`C-099` B-1) — то есть класс поймал ЧЕЛОВЕКО-круг гейта, стоивший
# полного цикла: правка на десять строк против вердикта, номера артефакта и диспетчеризации.
# `docs/workflow/binding-requires-mechanism.md`: проза не принуждает, принуждают скрипты.
#
# ЧТО ПРОВЕРЯЕТ. Только milestone-файлы, ВВЕДЁННЫЕ (`--diff-filter=A`) в проверяемом диапазоне.
# Четыре несущих раздела: Objective · Allowed paths · §Tasks · Acceptance.
#
# ПОЧЕМУ ТОЛЬКО ВВЕДЁННЫЕ — замер 2026-08-18 по `milestones/M-*.md` (53 файла):
#     без `Allowed paths` — 36 · без `§Tasks` — 31 · без `Acceptance` — 30 · без `Objective` — 20
# Барьер, проверяющий ВСЕ спеки, был бы красным с рождения, а такой барьер «объявляют шумом и
# выключают» (`docs/workflow/harness-track.md` §3). Проверка ИЗМЕНЁННЫХ файлов тоже отвергнута:
# правка одной строки в старой спеке заставляла бы дописывать четыре раздела и блокировала бы
# работу, не связанную с формой.
#
# ПРЕДЕЛ, НАЗВАННЫЙ ЧЕСТНО. Барьер НЕ заставляет 36 старых спек дорасти до формы — миграция
# остаётся открытой и не блокирует. Барьер проверяет НАЛИЧИЕ раздела, а не его осмысленность:
# `## Allowed paths` с пустым телом пройдёт. Это тот же класс предела, что у `FOUNDER-APPROVED`
# (`gates.md` §11) — токен ловит отсутствие строки, а не ложность причины.
set -uo pipefail

die() { echo "FAIL  $*" >&2; echo "      ↳ Барьер fail-closed: база не установлена достоверно." >&2; exit 1; }

# ── база из СОБЫТИЯ (та же проводка, что check_artifact_ids.sh / check_protected_artifacts.sh) ──
raw="${1:-}"
if [ -z "${raw}" ]; then
  case "${EVENT_NAME:-}" in
    push)         raw="${PUSH_BEFORE:-}" ;;
    pull_request) raw="${PR_BASE_SHA:-}" ;;
    "")           die "событие не задано (EVENT_NAME пуст) — барьер зовут не так, как его зовёт CI" ;;
    *)            die "неизвестное событие '${EVENT_NAME}' — база не определена" ;;
  esac
fi
[ -n "${raw}" ] || die "база события пуста (EVENT_NAME=${EVENT_NAME:-?})"
case "${raw}" in
  *[!0]*) : ;;
  *)      die "база = zero-SHA (создание ветки / force-push) — что введено, недоказуемо" ;;
esac
git rev-parse -q --verify "${raw}^{commit}" >/dev/null 2>&1 \
  || die "база '${raw}' отсутствует в истории (force-push / поверхностный клон)"
git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null \
  || die "база '${raw}' НЕ предок HEAD — история переписана; что введено, недоказуемо"

BASE="${raw}"

# ── введённые milestone-спеки в диапазоне ────────────────────────────────────────────
# `-z`: git КВОТИРУЕТ не-ASCII имена в текстовом режиме, и файл под русским именем выпал бы
# из проверки молча (тот же класс, что `R-046` Б-3 в check_artifact_ids.sh).
mapfile -d '' -t added < <(
  git diff --diff-filter=A --name-only -z "${BASE}" HEAD -- 'milestones/M-*.md' 2>/dev/null
)

if [ "${#added[@]}" -eq 0 ]; then
  echo "OK: в диапазоне ${BASE:0:7}..HEAD новых milestone-спек нет — проверять нечего"
  exit 0
fi

# ── обязательные разделы: имя → regex заголовка ──────────────────────────────────────
# Форма допускает `## X` и `### X`; `§` перед Tasks необязателен. Регистр игнорируется:
# спеки пишут и «Objective», и «objective».
FAIL=0
check_section() {
  local file="$1" human="$2" re="$3"
  if git show "HEAD:${file}" 2>/dev/null | grep -qiE "${re}"; then
    return 0
  fi
  echo "FAIL  ${file}: отсутствует обязательный раздел «${human}»" >&2
  FAIL=$((FAIL + 1))
}

for f in "${added[@]}"; do
  [ -n "$f" ] || continue
  echo "=== проверяю форму: $f ==="
  check_section "$f" "Objective"     '^#{2,3} *Objective'
  check_section "$f" "Allowed paths" '^#{2,3} *Allowed paths'
  check_section "$f" "§Tasks"        '^#{2,3} *§?Tasks'
  check_section "$f" "Acceptance"    '^#{2,3} *Acceptance'
done

if [ "$FAIL" -ne 0 ]; then
  cat >&2 <<'MSG'

Форма milestone-контракта задана `docs/04-workflow.md` §6. Без `Allowed paths` critic не может
выполнить scope-блок (`gates.md` §4), а dev не знает границы своей зоны — и заполняет пробел
по-своему, имея на это право (`testing.md` §«Спека правки существующего кода»).
Основание барьера: `C-099` B-1 — набор M-69 подан без этого раздела и стоил круга гейта.
MSG
  exit 1
fi

echo "OK: все введённые milestone-спеки несут обязательные разделы формы (§6)"
exit 0
