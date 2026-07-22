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
# Usage:
#   scripts/gc_worktrees.sh            # удалить все безопасно-рекультивируемые
#   scripts/gc_worktrees.sh --dry-run  # только показать, что было бы удалено
set -uo pipefail

DRY=0
[ "${1:-}" = "--dry-run" ] && DRY=1

ROOT="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null | sed 's#/\.git$##')"
git fetch origin --quiet 2>/dev/null || true

MAIN_CHECKOUT="$(git worktree list --porcelain | awk '/^worktree /{print $2; exit}')"

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
