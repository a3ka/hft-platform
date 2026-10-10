#!/usr/bin/env bash
# install_hooks.sh — ставит `.githooks` рабочим каталогом хуков для ЭТОГО клона.
#
# `.git/hooks` не версионируется, поэтому хук из репозитория сам по себе не действует:
# нужен `core.hooksPath`. Настройка живёт в `.git/config` и общая для ВСЕХ worktree клона
# (`--git-common-dir`), то есть ставится один раз на машину, а не на дерево.
#
# Контур ПРЕДУПРЕЖДАЮЩИЙ, и это объявлено честно: `--no-verify` его обходит, а на машине без
# запуска этого скрипта хуков нет вовсе. Он ловит ошибку ДО ущерба, а не после merge, — этим и
# ценен; заменой ревью он не является.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

[ -x .githooks/pre-commit ] || { echo "нет исполняемого .githooks/pre-commit" >&2; exit 1; }

PREV="$(git config --get core.hooksPath || echo '<не задан>')"
git config core.hooksPath .githooks
echo "core.hooksPath: ${PREV} -> $(git config --get core.hooksPath)"
echo "область: $(git rev-parse --absolute-git-dir | sed 's#/worktrees/.*##') (общая для всех worktree клона)"
echo
echo "проверка барьера пробой:"
bash scripts/tests/red_commit_paths.sh | tail -2
