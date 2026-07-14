#!/usr/bin/env bash
# Механический барьер: артефакт гейта, который СУЩЕСТВОВАЛ, обязан существовать на HEAD.
#
# Защищено:
#   research/critiques/*.md   — вердикты critic/risk-critic (аудит-трейл гейтов)
#   milestones/*.md           — спеки, по которым исполняют dev-агенты
#   docs/rfc/**               — contract-RFC (КАНОН); docs/contract-rfc/** — исторический путь
#
# ── Почему критерий именно такой (эволюция после трёх итераций критика) ──────────────
# Прежние версии гонялись за СПОСОБОМ исчезновения (D в коммите, R в коммите, D в мерже…) —
# и каждый раз находился новый способ: `git mv` внутри merge-коммита (статус R, не D);
# `merge -s ours`, выбрасывающий файл, который жил только в side-ветке (относительно первого
# родителя удаления нет вовсе). Латать частные случаи бесполезно.
#
# Инвариант формулируется от РЕЗУЛЬТАТА, а не от способа:
#   если защищённый путь существовал (в базе ИЛИ был добавлен в этой ветке),
#   он ОБЯЗАН существовать на HEAD — либо под тем же именем, либо переехав в другой
#   защищённый путь, либо его удалил коммит с явным `ALLOW-ARTIFACT-DELETE:` в СВОЁМ теле.
# Любой способ исчезновения (delete, rename-out, evil merge, -s ours, add→delete) ловится
# автоматически, потому что все они дают один и тот же результат: файла нет на HEAD.
#
# ── БАЗА СРАВНЕНИЯ = СОСТОЯНИЕ ДО СОБЫТИЯ (блокер B1, reviewer, C-006) ────────────────
# Прежняя проводка (`check_protected_artifacts.sh origin/main`) делала гейт ЛОЖНЫМ: на
# `push`-событии `actions/checkout` ставит `origin/main` на ТОЛЬКО ЧТО ЗАПУШЕННЫЙ коммит,
# поэтому `merge-base(origin/main, HEAD) == HEAD`, диапазон пуст и скрипт печатал «OK»
# ВСЕГДА. PR в этом репо не используются (все прогоны — event=push на main), так что барьер
# не срабатывал ни разу: коммит, сносящий вердикт критика, проходил CI зелёным. Ложный гейт
# опаснее отсутствующего — он создаёт ощущение защиты.
#
# Правильная база берётся из САМОГО СОБЫТИЯ:
#   push         → `github.event.before` (состояние ветки ДО пуша);
#   pull_request → `github.event.pull_request.base.sha`.
# Всё, что мешает установить базу достоверно (пустое событие, zero-SHA при создании ветки
# или force-push, база отсутствует в истории, база не предок HEAD ⇒ история переписана),
# → **FAIL, а не пропуск**: «базы нет» не значит «проверять нечего», это значит «не могу
# гарантировать целостность». Fail-closed — та же дисциплина, что у риск-гейта.
#
# Проба: `scripts/tests/red_protected_artifacts.sh` (10 сценариев, зовёт барьер ТОЙ ЖЕ
# проводкой, какой его зовёт CI; против пред-фиксной версии падает 7/10).
set -uo pipefail

die() { echo "FAIL  $*"; echo; echo "Барьер fail-closed: база сравнения не установлена достоверно."; exit 1; }

raw="${1:-}"
if [ -z "${raw}" ]; then
  case "${EVENT_NAME:-}" in
    push)         raw="${PUSH_BEFORE:-}" ;;
    pull_request) raw="${PR_BASE_SHA:-}" ;;
    "")           die "событие не задано (EVENT_NAME пуст) — барьер зовут не так, как его зовёт CI" ;;
    *)            die "неизвестное событие '${EVENT_NAME}' — база сравнения не определена" ;;
  esac
fi

[ -n "${raw}" ] || die "база события пуста (EVENT_NAME=${EVENT_NAME:-?})"
case "${raw}" in
  *[!0]*) : ;;  # есть хоть один ненулевой символ — не zero-SHA
  *)      die "база = zero-SHA (создание ветки или force-push) — целостность артефактов не доказуема" ;;
esac
git rev-parse -q --verify "${raw}^{commit}" >/dev/null 2>&1 \
  || die "база '${raw}' отсутствует в истории (переписана force-push'ем / поверхностный клон)"
git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null \
  || die "база '${raw}' НЕ предок HEAD — история переписана (force-push); что исчезло, недоказуемо"

base=$(git rev-parse "${raw}^{commit}")

is_protected() {
  case "$1" in
    research/critiques/*.md|milestones/*.md|docs/rfc/*|docs/contract-rfc/*) return 0 ;;
    *) return 1 ;;
  esac
}

# 1) Защищённые пути, которые СУЩЕСТВОВАЛИ: в базе + добавленные/переименованные в диапазоне.
existed=$(
  { git ls-tree -r --name-only "${base}"
    git log --diff-filter=AR -M --name-only --format='' "${base}..HEAD"
  } | sort -u | grep -vE '^$' || true
)

violations=0
for path in ${existed}; do
  is_protected "${path}" || continue
  git cat-file -e "HEAD:${path}" 2>/dev/null && continue     # цел — вопросов нет

  # Файла нет на HEAD. Ищем коммит(ы), которые его убрали.
  removed_by=$(git log --diff-filter=DR -M --format='%H' "${base}..HEAD" -- "${path}" || true)

  ok=0
  reason=""
  for c in ${removed_by}; do
    # (а) переезд в ДРУГОЙ защищённый путь — легитимная миграция (docs/contract-rfc → docs/rfc)
    newp=$(git show -M --name-status --format='' "${c}" \
             | awk -v p="${path}" '$1 ~ /^R/ && $2 == p {print $3}' | head -1)
    # Достаточно, чтобы новый путь тоже был ЗАЩИЩЁН: он сам проверяется этим же циклом
    # (цепочка переименований A→B→C внутри защиты легитимна; требовать существования B на
    # HEAD нельзя — он мог переехать дальше).
    if [ -n "${newp}" ] && is_protected "${newp}"; then
      ok=1; reason="переехал в ${newp} (остался под защитой)"; break
    fi
    # (б) осознанное удаление — override в ТЕЛЕ ЭТОГО коммита
    if git log -1 --format='%B' "${c}" | grep -q '^ALLOW-ARTIFACT-DELETE:'; then
      ok=1; reason="ALLOW-ARTIFACT-DELETE в $(git log -1 --format='%h' "${c}")"; break
    fi
  done

  if [ "${ok}" -eq 1 ]; then
    echo "NOTE  ${path}: ${reason}"
  else
    violations=$((violations + 1))
    if [ -z "${removed_by}" ]; then
      echo "FAIL  ${path}: артефакт ИСЧЕЗ с HEAD, и ни один коммит его не удалял"
      echo "      (значит его выбросил MERGE — evil merge / -s ours / rename внутри мержа)"
    else
      echo "FAIL  ${path}: артефакт удалён без ALLOW-ARTIFACT-DELETE"
      for c in ${removed_by}; do echo "      $(git log -1 --format='%h %s' "${c}")"; done
    fi
  fi
done

if [ "${violations}" -gt 0 ]; then
  echo
  echo "Артефакты гейтов — аудит-трейл, а не черновики. Почти всегда FAIL означает:"
  echo "  • ты не на своей ветке (общий чекаут переключили), ЛИБО"
  echo "  • 'git commit -a' / 'git add -A' (запрещено, .claude/rules/branch-hygiene.md)."
  echo "Осознанное удаление — строкой в теле ТОГО ЖЕ коммита:"
  echo "  ALLOW-ARTIFACT-DELETE: <причина>"
  exit 1
fi

echo "OK: защищённые артефакты целы на HEAD (${base:0:7}..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)"
