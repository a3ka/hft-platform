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
set -euo pipefail

BASE_REF="${1:-origin/main}"
base=$(git merge-base "${BASE_REF}" HEAD)

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
