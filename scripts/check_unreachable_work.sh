#!/usr/bin/env bash
# Барьер закрытия сессии — работа, недостижимая из `origin`.
# Проба: scripts/tests/red_unreachable_work.sh
#
# ЗАЧЕМ — замер 2026-09-01, а не предосторожность. На вопрос founder'а «ничего не потеряем
# с уходом из сессии?» architect был готов ответить «всё сохранено». Проверка вместо ответа
# нашла `research/critiques/C-201-m45-rollout-signature-r4.md` — вердикт критика на 316
# строк, живший В ОДНОМ локальном дереве и недостижимый из `origin` НИ ПО ОДНОЙ ветке. Уход
# из сессии стёр бы его: это дословно инцидент `M-49` («около 300k токенов работы гейта
# испарились»), только на новом носителе.
#
# КЛАСС ОШИБКИ, из-за которого он был бы потерян: **проверялся признак У́ЖЕ требования.**
# Автор смотрел на СВОИ ветки и СВОИ коммиты, а требование — «ничего не потеряется», то есть
# про ВСЕ деревья, включая оставшиеся от критиков, адверсариев и dev-агентов. Один раз из
# 51 дерева разница оказалась настоящей.
#
# ЧТО ПРОВЕРЯЕТСЯ: для КАЖДОГО зарегистрированного worktree — достижим ли его HEAD хоть из
# одной удалённой ссылки (`git branch -r --contains` + `refs/salvage/**`). Недостижимый HEAD,
# несущий АРТЕФАКТ ГЕЙТА или СПЕКУ (`research/**`, `milestones/**`, `docs/**`), — FAIL.
# Недостижимый HEAD без таких файлов (фикстуры мутаций критика) — NOTE: он одноразов, его
# смысл записан в вердикте.
# Грязное дерево — NOTE со СПОСОБОМ спасения, а не FAIL: незакоммиченное не всегда работа,
# но решать это обязан человек, а не молчание.
#
# КОДЫ: 0 — потерь нет; 1 — есть недостижимый артефакт гейта; 2 — предмет не установлен
# (не репозиторий, нет доступа к origin-ссылкам). Fail-closed: «проверять нечего» ≠ «чисто».

set -uo pipefail
ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$ROOT" 2>/dev/null || { echo "FAIL  SETUP: $ROOT недоступен"; exit 2; }
# Эти два стража ИЗБЫТОЧНЫ ДРУГ ДРУГУ по исходу и оставлены ради ДИАГНОСТИКИ: мутация
# первого пробу не роняет, потому что тот же мир ловит второй с тем же кодом. Названо здесь,
# а не оставлено миной следующему редактору (класс `C-206` Н-2).
git rev-parse --git-dir >/dev/null 2>&1 || { echo "FAIL  SETUP: $ROOT не git-репозиторий"; exit 2; }
git show-ref >/dev/null 2>&1 || { echo "FAIL  SETUP: ссылки недоступны"; exit 2; }

ARTIFACT_RE='^(research/|milestones/|docs/)'
LOST=0; NOTES=0

reachable() {  # достижим ли коммит из origin-веток ИЛИ из спас-рефов
  [ -n "$(git branch -r --contains "$1" 2>/dev/null)" ] && return 0
  git for-each-ref --format='%(objectname)' 'refs/remotes/**' 2>/dev/null | grep -qF "$1" && return 0
  for r in $(git ls-remote origin 'refs/salvage/*' 2>/dev/null | cut -f1); do
    [ "$r" = "$1" ] && return 0
    git merge-base --is-ancestor "$1" "$r" 2>/dev/null && return 0
  done
  return 1
}

while read -r d; do
  [ -d "$d" ] || { echo "NOTE  мёртвая регистрация worktree: $d (git worktree prune)"; NOTES=$((NOTES+1)); continue; }
  h="$(git -C "$d" rev-parse HEAD 2>/dev/null)" || continue
  short="${h:0:7}"; name="$(basename "$d")"
  if ! reachable "$h"; then
    files="$(git -C "$d" show --numstat --format='' "$h" 2>/dev/null | awk '{print $3}' | grep -E "$ARTIFACT_RE" || true)"
    if [ -n "$files" ]; then
      echo "FAIL  НЕДОСТИЖИМ ИЗ origin и несёт артефакт: $name ($short)"
      printf '        %s\n' $files
      echo "        спасти: git push origin $short:refs/salvage/\$(date +%F)/<имя>"
      LOST=$((LOST+1))
    else
      echo "NOTE  недостижим, но артефактов не несёт (фикстура?): $name ($short)"
      NOTES=$((NOTES+1))
    fi
  fi
  dirty=$(git -C "$d" status --porcelain 2>/dev/null | grep -cv '^?? ' || true)
  if [ "${dirty:-0}" -gt 0 ]; then
    echo "NOTE  незакоммичено ($dirty) в $name — снять неразрушающе:"
    echo "        S=\$(git -C $d stash create) && git push origin \$S:refs/salvage/\$(date +%F)/$name"
    NOTES=$((NOTES+1))
  fi
done < <(git worktree list --porcelain | grep '^worktree ' | sed 's/^worktree //')

echo
if [ "$LOST" -gt 0 ]; then
  echo "VERDICT: FAIL ($LOST недостижимых артефактов гейта) — сессию закрывать НЕЛЬЗЯ"
  exit 1
fi
echo "VERDICT: PASS — артефактов гейта вне origin нет (замечаний: $NOTES)"
exit 0
