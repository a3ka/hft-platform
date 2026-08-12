#!/usr/bin/env bash
# gc_worktrees.sh — безопасная рекультивация git-worktree'ов (branch-hygiene.md §Worktree lifecycle).
#
# ПРОБЛЕМА: правило «worktree на роль/задачу» кодифицировано, «освободи после» — нет. Каждый worktree
# компилирует свой multi-GB `target/`; никто не сносит → дисковая протечка (инцидент: 61 worktree × ~3.5 GB).
#
# ГАРАНТИЯ БЕЗОПАСНОСТИ (не теряем работу): worktree удаляется ТОЛЬКО если ВСЕ три условия:
#   (1) рабочее дерево ЧИСТОЕ (`git status --porcelain` пусто) — иначе возможна активная сессия;
#   (2) НЕТ только-локальных коммитов (`git rev-list HEAD --not --remotes` == 0) — всё на origin;
#   (3) HEAD ПОЛНОСТЬЮ на `origin/main` (`git merge-base --is-ancestor HEAD origin/main`) — милстоун смержен.
# НИКОГДА не `--force`, НИКОГДА не `rm -rf` вслепую. Активные (dirty/unpushed/не-смерженные) — не трогаются.
#
# ПОЧЕМУ ОДНОГО СНОСА ДЕРЕВЬЕВ НЕ ХВАТИЛО (инцидент 2026-08-12, диск 100 %):
# правило требовало запускать GC «в close-out после merge», а мусор производит ИМЕННО длинная
# НЕСЛИТАЯ работа: за неделю не смержено ничего, значит триггер не сработал ни разу. Плюс сам
# скрипт удаляет только ПОЛНОСТЬЮ смерженные деревья — при отсутствии merge он не удаляет
# ничего. Замер того дня: все 165 каталогов работы весили 782 MB, а `target/` — 105 GB.
# То есть механизм не покрывал 99 % объёма и не мог сработать в своём худшем случае.
# Отсюда режим `--reclaim`: он забирает КЭШ, не трогая работу, и не зависит ни от merge,
# ни от чистоты дерева — `target/` не работа ни при каком состоянии ветки.
#
# Usage:
#   scripts/gc_worktrees.sh                 # удалить все безопасно-рекультивируемые деревья
#   scripts/gc_worktrees.sh --dry-run       # только показать, что было бы удалено
#   scripts/gc_worktrees.sh --reclaim [Ч]   # снести target/ у деревьев, молчащих дольше Ч часов
#                                           # (по умолчанию 2), затем обычный безопасный GC
#   scripts/gc_worktrees.sh --reclaim-dry [Ч]
set -uo pipefail

DRY=0
MODE=gc
IDLE_H=2
case "${1:-}" in
  --dry-run)    DRY=1 ;;
  --reclaim)    MODE=reclaim; IDLE_H="${2:-2}" ;;
  --reclaim-dry) MODE=reclaim; DRY=1; IDLE_H="${2:-2}" ;;
esac

ROOT="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null | sed 's#/\.git$##')"
git fetch origin --quiet 2>/dev/null || true

MAIN_CHECKOUT="$(git worktree list --porcelain | awk '/^worktree /{print $2; exit}')"

# ─── РЕЖИМ RECLAIM: забрать кэш сборки, работу не трогать ────────────────────────────────
if [ "$MODE" = "reclaim" ]; then
  # FAIL-CLOSED: если где-то идёт сборка, её кэш сносить нельзя — прогон упадёт с невнятной
  # ошибкой компоновки, и причину будут искать в коде. Лучше отказаться и сказать почему.
  # Отказ касается ТОЛЬКО реального сноса: сухой прогон ничего не удаляет, и запрещать его
  # при живой сборке значит приучать обходить страж ради простого «посмотреть».
  if [ "$DRY" = "0" ] && { pgrep -x cargo >/dev/null 2>&1 || pgrep -x rustc >/dev/null 2>&1; }; then
    echo "ОТКАЗ: идёт сборка (cargo/rustc). Кэш сносить нельзя — прогон упадёт, и причину"
    echo "       будут искать в коде. Дождись окончания либо снеси target/ вручную у молчащих."
    echo "VERDICT: GC REFUSED (активная сборка)"
    exit 1
  fi
  now=$(date +%s); freed=0; touched=0
  git worktree list --porcelain | awk '/^worktree /{print $2}' | while read -r wt; do
    [ "$wt" = "$MAIN_CHECKOUT" ] && continue
    [ -d "$wt/target" ] || continue
    m=$(stat -c %Y "$wt/target" 2>/dev/null || echo "$now")
    idle_h=$(( (now - m) / 3600 ))
    sz=$(du -sm "$wt/target" 2>/dev/null | cut -f1)
    if [ "$idle_h" -lt "$IDLE_H" ]; then
      echo "KEEP-CACHE  $(basename "$wt") — молчит ${idle_h}ч (порог ${IDLE_H}ч), ${sz}MB"
      continue
    fi
    if [ "$DRY" = "1" ]; then
      echo "WOULD-RECLAIM  $(basename "$wt") — ${sz}MB, молчит ${idle_h}ч"
    else
      rm -rf "$wt/target" && echo "RECLAIMED  $(basename "$wt") — ${sz}MB (кэш; cargo пересоберёт)"
    fi
  done
  echo "-----"
  df -h / | tail -1
  echo
fi

removed=0; kept=0
# Список путей worktree'ов (кроме основного чекаута).
git worktree list --porcelain | awk '/^worktree /{print $2}' | while read -r wt; do
  [ "$wt" = "$MAIN_CHECKOUT" ] && continue
  name="$(basename "$wt")"

  dirty="$(git -C "$wt" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$dirty" != "0" ]; then
    echo "KEEP  $name — dirty ($dirty файл(ов); возможна активная сессия / несохранённое)"; continue
  fi
  local_only="$(git -C "$wt" rev-list HEAD --not --remotes 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$local_only" != "0" ]; then
    echo "KEEP  $name — $local_only только-локальных коммит(ов) (сначала push, потом GC)"; continue
  fi
  if ! git -C "$wt" merge-base --is-ancestor HEAD origin/main 2>/dev/null; then
    echo "KEEP  $name — не смержен в origin/main (активный/запаркованный feat)"; continue
  fi

  if [ "$DRY" = "1" ]; then
    echo "WOULD-REMOVE  $name (чист, на origin, смержен)"
  else
    if git worktree remove "$wt" 2>/dev/null; then echo "REMOVED  $name"; else echo "KEEP  $name — git worktree remove отказал"; fi
  fi
done

if [ "$DRY" = "0" ]; then
  git worktree prune
  echo "-----"
  echo "worktree'ов осталось: $(git worktree list | wc -l | tr -d ' ')"
fi
echo "VERDICT: GC $([ "$DRY" = "1" ] && echo "DRY-RUN" || echo "DONE")"
