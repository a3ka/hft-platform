#!/bin/bash
# pi-dev.sh — hft-platform UNIFIED launcher для pi-агентов (адаптация einhard-runtime
# W1-W3: динамическое role-resolution по имени симлинка, Step-0 bootstrap worktree +
# идентичность роли, инжект dispatch-mandate в системный промт).
#
# Usage:
#   pi-engine-dev [pi-args...]            # роль из имени симлинка
#   pi-dev.sh <role> [pi-args...]         # роль первым аргументом
#   pi-dev.sh <role> --dry-run            # bootstrap-смоук: worktree+identity, cleanup, exit 0
#   pi-engine-dev --branch feat/x         # bootstrap от origin/feat/x (по умолчанию main)
#   HFT_WORKTREE=/tmp/dir pi-engine-dev   # войти в СУЩЕСТВУЮЩИЙ worktree (без bootstrap)
#   pi-engine-dev -p "prompt"             # неинтерактивно
#
# Роли: engine-dev venue-dev research-dev signal-engineer tester
# (architect/reviewer/critic/risk-critic — Claude-native канонично; pi = fallback-лейн).

set -u

PROJECT_DIR="${HFT_REPO:-/home/nous/hft-platform}"

# ── Role resolution ──
SELF_BASE="$(basename "$0")"
ROLE=""
case "$SELF_BASE" in
  pi-dev.sh|pi-dev) ROLE="${1:-}"; [ -n "$ROLE" ] && shift ;;
  pi-*.sh) ROLE="${SELF_BASE#pi-}"; ROLE="${ROLE%.sh}" ;;
  pi-*)    ROLE="${SELF_BASE#pi-}" ;;
esac
# namespace-префикс hft- (коллизии имён с einhard-симлинками в ~/bin): pi-hft-tester → tester
ROLE="${ROLE#hft-}"

if [ -z "$ROLE" ]; then
  echo "Usage: pi-dev.sh <role> [pi-args...]  (или через симлинк pi-<role>)" >&2
  echo "Roles: engine-dev venue-dev research-dev signal-engineer tester" >&2
  exit 1
fi

AGENT_FILE="$PROJECT_DIR/.claude/agents/$ROLE.md"
if [ ! -f "$AGENT_FILE" ]; then
  echo "❌ Unknown role '$ROLE' — нет $AGENT_FILE" >&2
  exit 1
fi

export AGENT="$ROLE" AGENT_NAME="$ROLE"

case "$ROLE" in
  architect|reviewer|critic|risk-critic)
    echo "ℹ️  Premium-роль '$ROLE' через pi/MiniMax — это FALLBACK. Канонично: Claude-native (сильная модель; risk-critic/reviewer НЕ экономим — CLAUDE.md маршрутизация)." >&2 ;;
esac

# ── Флаги обвязки ──
DRY_RUN=0
BRANCH="${HFT_BRANCH:-main}"
PASS_ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --branch)  shift; BRANCH="${1:-main}" ;;
    *)         PASS_ARGS+=("$1") ;;
  esac
  shift
done

# ── Step-0 bootstrap: свежий worktree ИЛИ вход в существующий ──
OWNED_WORKTREE=0
if [ -n "${HFT_WORKTREE:-}" ]; then
  [ -d "$HFT_WORKTREE" ] || { echo "❌ HFT_WORKTREE='$HFT_WORKTREE' не существует" >&2; exit 1; }
  WORKTREE_PATH="$HFT_WORKTREE"
  echo "📂 Worktree (HFT_WORKTREE — вход в существующий): $WORKTREE_PATH"
else
  WORKTREE_PATH="/tmp/hft-${ROLE}-$(date +%s)"
  GIT_SSH_COMMAND="ssh -o StrictHostKeyChecking=accept-new -o BatchMode=yes" \
    git -C "$PROJECT_DIR" fetch origin --quiet 2>/dev/null \
    || echo "⚠️  git fetch не прошёл — bootstrap от ЛОКАЛЬНОГО origin/$BRANCH (может отставать)" >&2
  if ! git -C "$PROJECT_DIR" worktree add "$WORKTREE_PATH" "origin/$BRANCH" >/dev/null 2>&1; then
    if ! git -C "$PROJECT_DIR" worktree add --detach "$WORKTREE_PATH" "$BRANCH" >/dev/null 2>&1; then
      echo "❌ git worktree add не прошёл ни для origin/$BRANCH, ни для $BRANCH" >&2
      exit 1
    fi
  fi
  OWNED_WORKTREE=1
  echo "📂 Worktree (свежий bootstrap от origin/$BRANCH): $WORKTREE_PATH"
fi

cd "$WORKTREE_PATH" || { echo "❌ cd '$WORKTREE_PATH' не прошёл"; exit 1; }

if [ "$OWNED_WORKTREE" -eq 1 ]; then
  # Пер-сессионная ветка (никогда не checkout -B main — общий ref linked-worktree'ов)
  git checkout -B "${ROLE}-$(basename "$WORKTREE_PATH")" --quiet 2>/dev/null || true
fi

# Личность НЕ переустанавливается (branch-hygiene.md п.6, commit-discipline).
# Автор коммитов — владелец репозитория; роль указывается меткой в конце subject'а:
#   feat(M-NN): task #k — <...> [${ROLE}]
# Прежний блок ставил ролевой user.name/email per-worktree; замером установлено, что как
# признак роли git-личность не работает (все worktree несли подпись предыдущей роли).
[ -d "$PROJECT_DIR/.githooks" ] && git config core.hooksPath .githooks

IDENT_EMAIL="$(git config user.email)"

# ── --dry-run смоук ──
if [ "$DRY_RUN" -eq 1 ]; then
  FAIL=0
  [ -n "$IDENT_EMAIL" ] || { echo "DRY-RUN FAIL: git identity не настроена"; FAIL=1; }
  git status --porcelain | grep -q . && { echo "DRY-RUN FAIL: worktree не чистый"; FAIL=1; }
  [ -f "$PROJECT_DIR/.claude/wrappers/dispatch-mandate.md" ] || { echo "DRY-RUN FAIL: dispatch-mandate.md отсутствует"; FAIL=1; }
  echo "DRY-RUN: role=$ROLE worktree=$WORKTREE_PATH identity=$IDENT_EMAIL"
  cd / || exit 1
  if [ "$OWNED_WORKTREE" -eq 1 ]; then
    git -C "$PROJECT_DIR" worktree remove --force "$WORKTREE_PATH" >/dev/null 2>&1 || true
    git -C "$PROJECT_DIR" branch -D "${ROLE}-$(basename "$WORKTREE_PATH")" >/dev/null 2>&1 || true
  fi
  [ "$FAIL" -eq 0 ] && echo "DRY-RUN: PASS" && exit 0
  echo "DRY-RUN: FAIL"; exit 1
fi

# ── Инжект персоны + дисциплины в системный промт ──
AGENT_CONTENT="$(cat "$AGENT_FILE")"
MANDATE_FILE="$PROJECT_DIR/.claude/wrappers/dispatch-mandate.md"
MANDATE_CONTENT=""
if [ -f "$MANDATE_FILE" ]; then
  MANDATE_CONTENT="

$(cat "$MANDATE_FILE")"
else
  echo "⚠️  dispatch-mandate.md отсутствует — Done-Block/STOP-мандат НЕ инжектирован" >&2
fi

SYSTEM_IDENTITY="

═══════════════════════════════════════════════════════════════
AGENT IDENTITY: $ROLE (hft-platform)
═══════════════════════════════════════════════════════════════
Ты — $ROLE. Идентичность ФИКСИРОВАНА на сессию.
Обвязка уже подготовила рабочую копию:
  CWD:          $WORKTREE_PATH  (свежий чекаут origin/$BRANCH, если не переопределено)
  git identity: $IDENT_EMAIL  (личность владельца — НЕ переустанавливай)
  роль в коммите: метка в конце subject'а — [${ROLE}]
НЕ переходи в $PROJECT_DIR (основной чекаут founder'а).
Первым делом проверь: git status -sb && git log --oneline -3
═══════════════════════════════════════════════════════════════
"

PI_SESSION_ROOT="${HFT_PI_SESSION_ROOT:-$HOME/.local/state/hft-platform/pi/sessions}"
umask 002
AGENT_SESSION_DIR="$PI_SESSION_ROOT/$ROLE"
mkdir -p "$AGENT_SESSION_DIR"

PI_PROVIDER="${HFT_PI_PROVIDER:-minimax}"
PI_MODEL="${HFT_PI_MODEL:-MiniMax-M3}"

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "               🏷️  AGENT: $ROLE (pi, hft-platform)"
echo "═══════════════════════════════════════════════════════════════"
echo "📋 Model:    $PI_PROVIDER/$PI_MODEL (thinking: medium)"
echo "📁 Sessions: $AGENT_SESSION_DIR"
echo "📁 CWD:      $WORKTREE_PATH"
echo "🔑 Identity: $IDENT_EMAIL"
echo ""

# Свежая сессия по умолчанию; -c/-r — громкое предупреждение о протухшем handoff
set -- "${PASS_ARGS[@]+"${PASS_ARGS[@]}"}"
case " $* " in
  *" -c "*|*" --continue "*|*" -r "*|*" --resume "*|*" --session "*|*" --session-id "*)
    echo "⚠️  ПРОДОЛЖЕНИЕ прошлой сессии $ROLE — её последнее сообщение может быть ПРОТУХШИМ handoff'ом." ;;
  *)
    echo "🆕 Свежая сессия $ROLE ($(date -u +%FT%TZ))."
    set -- --name "${ROLE}-$(date +%s)" "$@" ;;
esac

pi \
    --provider "$PI_PROVIDER" \
    --model "$PI_MODEL" \
    --thinking medium \
    --append-system-prompt "$AGENT_CONTENT$SYSTEM_IDENTITY$MANDATE_CONTENT" \
    --session-dir "$AGENT_SESSION_DIR" \
    --no-context-files \
    "$@"
PI_EXIT=$?

# ── Self-cleanup: убрать НАШ worktree, только если ничего не потеряем ──
if [ "$OWNED_WORKTREE" -eq 1 ]; then
  cd / || exit "$PI_EXIT"
  DIRTY=$(git -C "$WORKTREE_PATH" status --porcelain 2>/dev/null | wc -l)
  UNPUSHED=$(git -C "$WORKTREE_PATH" log --oneline "origin/$BRANCH..HEAD" 2>/dev/null | wc -l)
  if [ "$DIRTY" -eq 0 ] && [ "$UNPUSHED" -eq 0 ]; then
    git -C "$PROJECT_DIR" worktree remove --force "$WORKTREE_PATH" >/dev/null 2>&1 \
      && echo "🧹 Worktree убран (ничего незапушенного): $WORKTREE_PATH"
    git -C "$PROJECT_DIR" branch -D "${ROLE}-$(basename "$WORKTREE_PATH")" >/dev/null 2>&1
  else
    echo "📌 Worktree ОСТАВЛЕН: $WORKTREE_PATH ($UNPUSHED незапушенных коммитов, $DIRTY грязных файлов)."
    echo "   Дальше по цепочке: tester/reviewer входят туда через HFT_WORKTREE=$WORKTREE_PATH pi-hft-tester"
    echo "   (или reviewer Claude-native делает merge ветки $(git -C "$WORKTREE_PATH" rev-parse --abbrev-ref HEAD 2>/dev/null) в main)."
  fi
fi
exit "$PI_EXIT"
