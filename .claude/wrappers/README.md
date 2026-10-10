# Agent Launch Wrappers — hft-platform

`pi-dev.sh` (+ симлинки `pi-<role>.sh`) — единый лаунчер внешних дешёвых агентов
(pi → minimax/MiniMax-M3, thinking medium). Адаптация einhard-runtime W1-W3.

| Команда (в `~/.local/bin`) | Роль | Зона |
|---|---|---|
| `pi-engine-dev` | engine-dev | crates/{journal,book,oms,sim,runner,alpha,portfolio,strategy}/src |
| `pi-venue-dev` | venue-dev | crates/venue-*/src |
| `pi-research-dev` | research-dev | crates/research-cli/src |
| `pi-signal-engineer` | signal-engineer | crates/signals/src + research/specs |
| `pi-hft-tester` | tester | read-only прогон (`hft-`префикс — имя `pi-tester` занято einhard'ом) |

Claude-native (без обвязки, сильные модели — не экономим): architect, reviewer,
risk-critic. Critic — средняя модель: Claude-субагент или `pi-dev.sh critic` (fallback).

## Что делает лаунчер (Step-0 механизирован)

1. свежий worktree `/tmp/hft-<role>-<epoch>` от `origin/main` (или `--branch feat/x`);
2. пер-сессионная ветка + идентичность роли `git config --worktree` (04-workflow §4:
   идентичность коммиттера = роль);
3. инжект персоны `.claude/agents/<role>.md` + `dispatch-mandate.md` (sacred-зоны,
   TDD, Done Block, «НЕ пушь») в системный промт (`--no-context-files`);
4. после выхода: worktree убирается, ЕСЛИ нет незапушенных коммитов/грязи; иначе
   ОСТАВЛЯЕТСЯ с напечатанным путём — следующий в цепочке входит через
   `HFT_WORKTREE=<путь> pi-hft-tester`, reviewer мержит сессионную ветку в main.

## Usage

```bash
pi-engine-dev                       # TUI, свежая сессия
pi-engine-dev -p "<paste §D промт>" # неинтерактивно
pi-engine-dev --dry-run             # bootstrap-смоук без запуска pi
HFT_WORKTREE=/tmp/hft-engine-dev-123 pi-hft-tester   # тестер в worktree dev'а
pi-dev.sh <role> [...]              # явная роль
```

Оверрайды: `HFT_PI_PROVIDER`/`HFT_PI_MODEL` (default minimax/MiniMax-M3),
`HFT_REPO`, `HFT_BRANCH`, `HFT_PI_SESSION_ROOT`. Сессии:
`~/.local/state/hft-platform/pi/sessions/<role>/`. Креды pi: `~/.pi/agent/auth.json`.

## Новая роль

1. `.claude/agents/<role>.md`; 2. `ln -s pi-dev.sh .claude/wrappers/pi-<role>.sh`;
3. `ln -s <repo>/.claude/wrappers/pi-<role>.sh ~/.local/bin/pi-<role>`.
Кодекс-зеркало (`.codex/`) в hft-platform НЕ заведено — critic/архитектор-fallback
через Codex CLI пока вручную; завести при первой реальной нужде.
