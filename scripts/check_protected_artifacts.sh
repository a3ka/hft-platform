#!/usr/bin/env bash
# Механический барьер: артефакты гейтов нельзя удалить или увести из-под защиты.
#
# Защищено:
#   research/critiques/*.md   — вердикты critic/risk-critic (аудит-трейл гейтов)
#   milestones/*.md           — спеки, по которым исполняют dev-агенты
#   docs/rfc/**               — contract-RFC (T1-governance; КАНОНИЧЕСКИЙ путь)
#   docs/contract-rfc/**      — исторический путь тех же RFC (защищаем, пока существует)
#
# Что ловим (findings C-006 rev3 — прошлая версия проверяла только НЕТТО-диф и обходилась):
#   1. покоммитно, от merge-base до HEAD → add→delete внутри ветки больше не «схлопывается»;
#   2. rename ИЗ защищённого в НЕзащищённый путь (увод из-под защиты) = удаление;
#      rename защищённый→защищённый разрешён;
#   3. override действует ТОЛЬКО в ТОМ ЖЕ коммите, который удаляет (не «где-то в диапазоне»);
#   4. оба каталога RFC;
#   5. MERGE-КОММИТЫ (находка C-006 rev4): merge может САМ выбросить файл, присутствующий во
#      ВСЕХ родителях («злой мерж»). `--no-merges` это пропускал — критик воспроизвёл: merge,
#      удаляющий milestones/*.md, давал exit=0.
#
# Критерий нарушения: артефакт удалён/уведён коммитом ветки И ОТСУТСТВУЕТ НА HEAD.
# Так ловится и add→delete внутри ветки (на HEAD его нет), и «снёс чужой вердикт»
# (на HEAD его нет), но НЕ наказывается легитимный сценарий «удалил → восстановил»
# (инцидент 139b399 → 352b1db: вердикт снесли по ошибке и вернули — на HEAD он есть).
#
# Осознанное удаление: строка `ALLOW-ARTIFACT-DELETE: <причина>` в теле ЭТОГО коммита.
set -euo pipefail

BASE_REF="${1:-origin/main}"
base=$(git merge-base "${BASE_REF}" HEAD)

is_protected() {
  case "$1" in
    research/critiques/*.md|milestones/*.md|docs/rfc/*|docs/contract-rfc/*) return 0 ;;
    *) return 1 ;;
  esac
}

violations=0

# ── Слой B: MERGE-КОММИТЫ («злой мерж», C-006 rev4) ───────────────────────────────────
# Файл, который есть во ВСЕХ родителях, но отсутствует в самом merge-коммите, выброшен ИМ.
# (Если файл удалён на одной из веток — он отсутствует в этом родителе, и это не «злой мерж»:
#  такое удаление ловится слоем A на коммите, который его сделал.)
for mc in $(git rev-list --merges --reverse "${base}..HEAD"); do
  parents=$(git log -1 --format='%P' "${mc}")
  body=$(git log -1 --format='%B' "${mc}")
  subject=$(git log -1 --format='%h %s' "${mc}")
  override=0
  printf '%s' "${body}" | grep -q '^ALLOW-ARTIFACT-DELETE:' && override=1

  first_parent=$(echo "${parents}" | awk '{print $1}')
  # Кандидаты: защищённые пути, удалённые относительно ПЕРВОГО родителя.
  while IFS=$'\t' read -r status path _rest; do
    [ -z "${status:-}" ] && continue
    case "${status}" in D*) ;; *) continue ;; esac
    is_protected "${path}" || continue

    in_all_parents=1
    for p in ${parents}; do
      git cat-file -e "${p}:${path}" 2>/dev/null || in_all_parents=0
    done
    [ "${in_all_parents}" -eq 1 ] || continue   # удалён на ветке-родителе → слой A уже разобрал

    if git cat-file -e "HEAD:${path}" 2>/dev/null; then
      echo "NOTE  ${subject}: merge выбрасывал ${path}, но на HEAD артефакт ПРИСУТСТВУЕТ"
    elif [ "${override}" -eq 1 ]; then
      echo "NOTE  ${subject}: удаление ${path} в merge разрешено ALLOW-ARTIFACT-DELETE"
    else
      echo "FAIL  ${subject}: MERGE-КОММИТ выбрасывает защищённый артефакт: ${path}"
      echo "      (файл есть во ВСЕХ родителях — значит его выбросил сам merge, «злой мерж»)"
      violations=$((violations + 1))
    fi
  done < <(git diff --name-status "${first_parent}" "${mc}")
done

# ── Слой A: обычные коммиты ────────────────────────────────────────────────────────────
for commit in $(git rev-list --no-merges --reverse "${base}..HEAD"); do
  body=$(git log -1 --format='%B' "${commit}")
  override=0
  if printf '%s' "${body}" | grep -q '^ALLOW-ARTIFACT-DELETE:'; then
    override=1
  fi
  subject=$(git log -1 --format='%h %s' "${commit}")

  # -M: детекция переименований. Формат: "D<TAB>path" | "R100<TAB>old<TAB>new"
  while IFS=$'\t' read -r status a b; do
    [ -z "${status:-}" ] && continue
    case "${status}" in
      D*)
        if is_protected "${a}"; then
          if git cat-file -e "HEAD:${a}" 2>/dev/null; then
            echo "NOTE  ${subject}: удалял ${a}, но на HEAD артефакт ПРИСУТСТВУЕТ (восстановлен)"
          elif [ "${override}" -eq 1 ]; then
            echo "NOTE  ${subject}: удаление ${a} разрешено ALLOW-ARTIFACT-DELETE"
          else
            echo "FAIL  ${subject}: УДАЛЯЕТ защищённый артефакт (и его нет на HEAD): ${a}"
            violations=$((violations + 1))
          fi
        fi
        ;;
      R*)
        if is_protected "${a}"; then
          if is_protected "${b}"; then
            : # защищённый → защищённый: легитимный переезд (docs/contract-rfc → docs/rfc)
          elif git cat-file -e "HEAD:${a}" 2>/dev/null; then
            echo "NOTE  ${subject}: уводил ${a}, но на HEAD артефакт ПРИСУТСТВУЕТ"
          elif [ "${override}" -eq 1 ]; then
            echo "NOTE  ${subject}: увод ${a} → ${b} разрешён ALLOW-ARTIFACT-DELETE"
          else
            echo "FAIL  ${subject}: УВОДИТ артефакт из-под защиты: ${a} → ${b}"
            echo "      (переименование в незащищённый путь = то же удаление, только тихое)"
            violations=$((violations + 1))
          fi
        fi
        ;;
    esac
  done < <(git show -M --name-status --format='' "${commit}")
done

if [ "${violations}" -gt 0 ]; then
  echo
  echo "Артефакты гейтов — аудит-трейл, а не черновики. Почти всегда FAIL означает:"
  echo "  • ты не на своей ветке (общий чекаут переключили), ЛИБО"
  echo "  • сделан 'git commit -a' / 'git add -A' (запрещено, .claude/rules/branch-hygiene.md)."
  echo "Осознанное удаление — строкой в теле ТОГО ЖЕ коммита:"
  echo "  ALLOW-ARTIFACT-DELETE: <причина>"
  exit 1
fi

echo "OK: защищённые артефакты целы (${base:0:7}..HEAD, покоммитно, с учётом переименований)"
