#!/usr/bin/env bash
# Механический барьер: коммит НЕ ИМЕЕТ ПРАВА удалять артефакты гейтов и milestone'ов.
#
# Повод (три инцидента подряд): коммит reviewer'а `git commit -am` в общем чекауте снёс
# research/critiques/C-006-doc-gate.md — вердикт критика, то есть аудит-трейл гейта.
# Правило в branch-hygiene.md было — его нарушали. Правило, которое можно нарушить молча,
# правилом не является: нужен барьер, а не пожелание.
#
# Защищено:
#   research/critiques/*.md   — вердикты critic / risk-critic (артефакты гейтов)
#   milestones/*.md           — спеки milestone'ов (по ним исполняют dev-агенты)
#   docs/rfc/*                — contract-RFC (T1-governance)
#
# Переименование (R) и правка (M) разрешены; запрещено УДАЛЕНИЕ (D).
# Осознанное удаление (например, милестоун слит в другой) → founder-override:
#   в теле коммита строка `ALLOW-ARTIFACT-DELETE: <причина>`.
set -euo pipefail

BASE="${1:-origin/main}"
RANGE="${BASE}...HEAD"

deleted=$(git diff --name-status "${RANGE}" \
  | awk '$1 ~ /^D/ {print $2}' \
  | grep -E '^(research/critiques/.*\.md|milestones/.*\.md|docs/rfc/)' || true)

if [ -z "${deleted}" ]; then
  echo "OK: защищённые артефакты не удаляются (${RANGE})"
  exit 0
fi

# Осознанный founder-override в любом коммите диапазона.
if git log "${RANGE}" --format='%B' | grep -q '^ALLOW-ARTIFACT-DELETE:'; then
  echo "NOTE: удаление разрешено явным ALLOW-ARTIFACT-DELETE:"
  echo "${deleted}"
  exit 0
fi

echo "FAIL: коммит УДАЛЯЕТ артефакты гейтов/милестоунов — это аудит-трейл, а не черновик:"
echo "${deleted}" | sed 's/^/  - /'
echo
echo "Почти всегда это значит: ты не на своей ветке (общий чекаут переключили) ИЛИ сделал"
echo "'git commit -a' / 'git add -A' (запрещено — .claude/rules/branch-hygiene.md)."
echo "Если удаление ОСОЗНАННОЕ — добавь в тело коммита строку:"
echo "  ALLOW-ARTIFACT-DELETE: <причина>"
exit 1
