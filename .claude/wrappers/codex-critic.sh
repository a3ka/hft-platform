#!/usr/bin/env bash
# hft-codex-critic — Codex launcher for the hft-platform critic role.
#
# This is intentionally separate from the global einhard `codex-dev`.
# It uses hft-platform's .claude persona/rules directly; hft has no .codex
# mirror yet.

set -euo pipefail

REPO="${HFT_REPO:-/home/nous/hft-platform}"
BRANCH="${HFT_BRANCH:-main}"
# Модель называется ЯВНО и передаётся в codex через -m. Пусто ⇒ берётся дефолт из
# ~/.codex/config.toml. Для харнесс-трека founder указывает сильную модель (Terra):
#   HFT_CODEX_MODEL=<имя> bash .claude/wrappers/codex-critic.sh
CODEX_MODEL="${HFT_CODEX_MODEL:-}"

if [[ ! -d "$REPO/.git" ]]; then
  echo "hft-platform repo not found at $REPO" >&2
  echo "Set HFT_REPO or run from the hft-platform checkout." >&2
  exit 1
fi

if [[ ! -f "$REPO/.claude/agents/critic.md" ]]; then
  echo "hft critic profile missing: $REPO/.claude/agents/critic.md" >&2
  exit 1
fi

OWNED_WORKTREE=0
if [[ -n "${HFT_WORKTREE:-}" ]]; then
  if [[ ! -d "$HFT_WORKTREE" ]]; then
    echo "HFT_WORKTREE does not exist: $HFT_WORKTREE" >&2
    exit 1
  fi
  WORKTREE_PATH="$HFT_WORKTREE"
  echo "Worktree (HFT_WORKTREE override): $WORKTREE_PATH"
else
  WORKTREE_PATH="/tmp/hft-codex-critic-$(date +%s)"
  GIT_SSH_COMMAND="ssh -o StrictHostKeyChecking=accept-new -o BatchMode=yes" \
    git -C "$REPO" fetch origin --quiet 2>/dev/null \
    || echo "warning: git fetch failed; using local origin/$BRANCH ref" >&2
  if ! git -C "$REPO" worktree add "$WORKTREE_PATH" "origin/$BRANCH" >/dev/null 2>&1; then
    git -C "$REPO" worktree add --detach "$WORKTREE_PATH" "$BRANCH" >/dev/null 2>&1 \
      || { echo "git worktree add failed for origin/$BRANCH and $BRANCH" >&2; exit 1; }
  fi
  OWNED_WORKTREE=1
  echo "Worktree (fresh bootstrap from origin/$BRANCH): $WORKTREE_PATH"
fi

cd "$WORKTREE_PATH"

BOOTSTRAP="BOOTSTRAP INVOCATION — no architect handoff yet.

Adopt the hft-platform critic persona by reading these files in order:
  - CLAUDE.md
  - .claude/rules/gates.md
  - .claude/rules/scope-guard.md
  - .claude/rules/testing.md
  - .claude/rules/commit-discipline.md
  - .claude/rules/branch-hygiene.md
  - .claude/rules/handoff-block.md
  - .claude/agents/critic.md
  - docs/workflow/harness-track.md   (если предмет — харнесс: scripts/**, .github/workflows/**)

After completing the reads above, reply with EXACTLY:

  Bootstrap complete. Acting as hft critic.
  Model: <model actually in use>
  Scope: plan-time gate AFTER architect commits artifacts
  Output: research/critiques/C-NNN-<topic>.md  (committed and pushed to the subject branch)
  Verdict tiers: REJECT / NOTE / ESCALATE
  Awaiting architect commit-chain reference + milestone path.

Then STOP and wait for the user to paste the architect handoff
(commit-chain reference + milestone path + stakes:high|normal).

When user paste arrives: audit the committed artifact set, not plan text
alone. Verify the full hft artifact set per .claude/agents/critic.md:
T-contracts, trait signatures, RED tests, verify script, and milestone
file. Then run the hft critic checks from CLAUDE.md + .claude/rules/*.

VERDICT DELIVERY IS PART OF THE GATE, NOT A FORMALITY (gates.md section 4):

  1. Allocate the id with the mechanism, never by hand:
       bash scripts/next_artifact_id.sh C
     A taken number turns check_artifact_ids.sh red; the race is real
     (twelve occurrences in three days).
  2. Write the verdict to research/critiques/C-NNN-<topic>.md, including a
     GATE-META header (milestone / audited_repo / audited_base / audited_head /
     verdict) and a Done Block with raw command output and exit codes.
  3. COMMIT IT TO THE SUBJECT BRANCH AND PUSH, with explicit paths:
       git add research/critiques/C-NNN-<topic>.md
       git commit -- research/critiques/C-NNN-<topic>.md
       git push origin HEAD:<subject branch>
     Use the role tag [critic] in the subject line. No Co-Authored-By trailer.
     `git commit -a` and `git add -A` are blocked by .githooks/pre-commit.

A verdict that exists only in this transcript, or only under /tmp, is NOT an
audit trail: it does not survive the session and cannot be shown to the next
agent. This cost the project ~300k tokens once (M-49: two REJECTs vanished with
a subagent transcript) and repeated on 2026-08-15, when an M-60 verdict was
written to .omc/plans/ and had to be recovered by hand.

Never edit milestones/*.md, docs/**, contracts/**, crates/**,
PROJECT-STATE.md, or TECH-DEBT.md. research/critiques/ is yours to write."

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "                  AGENT: hft critic (Codex)"
echo "═══════════════════════════════════════════════════════════════"
echo "Model: ${CODEX_MODEL:-<default from ~/.codex/config.toml>}"
echo "CWD:   $WORKTREE_PATH"
echo "Branch:$BRANCH"
echo ""

CODEX_EXIT=0
if [[ -n "$CODEX_MODEL" ]]; then
  codex -m "$CODEX_MODEL" --dangerously-bypass-approvals-and-sandbox "$BOOTSTRAP" || CODEX_EXIT=$?
else
  codex --dangerously-bypass-approvals-and-sandbox "$BOOTSTRAP" || CODEX_EXIT=$?
fi

if [[ "$OWNED_WORKTREE" -eq 1 ]]; then
  cd /
  DIRTY=$(git -C "$WORKTREE_PATH" status --porcelain 2>/dev/null | wc -l)
  UNPUSHED=$(git -C "$WORKTREE_PATH" log --oneline "origin/$BRANCH..HEAD" 2>/dev/null | wc -l)
  # Дерево сносится ТОЛЬКО когда работа уехала в origin. Незапушенный вердикт — потерянный
  # аудит-трейл, и молчаливый снос дерева хуже оставленного каталога.
  if [[ "$DIRTY" -eq 0 && "$UNPUSHED" -eq 0 ]]; then
    git -C "$REPO" worktree remove --force "$WORKTREE_PATH" >/dev/null 2>&1 \
      && echo "Worktree cleaned: $WORKTREE_PATH"
  else
    echo "Worktree kept: $WORKTREE_PATH ($UNPUSHED unpushed commit(s), $DIRTY dirty file(s))"
  fi
fi

exit "$CODEX_EXIT"
