#!/usr/bin/env python3
"""agent-dispatch-gate — PreToolUse-гейт на запуск субагентов (Task|Agent).

ЗАЧЕМ ЭТОТ ФАЙЛ СУЩЕСТВУЕТ (замер, а не принцип).

`CLAUDE.md` §Операционные принципы: «Пользователь — оркестрационный диспетчер. Агенты не
вызывают друг друга; передача через Handoff-блоки». `.claude/agents/architect.md`
§Делегирование: «Прочие роли отсюда НЕ ЗАПУСКАЮТСЯ — запрет, не невозможность: им —
Handoff-блок через founder'а».

2026-08-17 architect прочёл обе нормы в стартовом протоколе, процитировал одну из них в
собственном ответе — и запустил `subagent_type: reviewer`. Ничто не остановило: механизма не
существовало. Нарушение заметил founder, а не гейт.

Перенос из einhard (`~/einhard-runtime/.claude/hooks/agent-dispatch-gate.py`,
founder-директива 2026-07-11) по прямому решению founder'а 2026-08-17. Основание переноса —
их же правило `binding-requires-mechanism.md`: «Every incident → new BINDING paragraph →
bigger startup context → agents under context pressure miss the paragraph → new incident.
**Prose does not enforce; scripts do.**»

ЧТО ДЕЛАЕТ. Каждый вызов Task/Agent поднимает founder'у интерактивное одобрение
(`permissionDecision: ask`). Read-only lookup-агенты (`Explore`, `Plan`) — auto-allow: они не
пишут код и не подменяют ролевую цепочку.

ЧЕГО НЕ ДЕЛАЕТ — названо честно:
  · не отличает «founder попросил в промпте» от инициативы агента — это решает человек в
    момент запроса; хук лишь ГАРАНТИРУЕТ, что вопрос будет задан;
  · не действует во внешних сессиях, не читающих `.claude/settings.json` (у einhard тот же
    предел закрыт дублем в git-хуке — у нас такого дубля пока нет, и это названо, а не скрыто);
  · `subagent_type: architect` тоже поднимает вопрос: профиль разрешает клонов своей роли, но
    различить «клон для замера» и «клон вместо ролевой цепочки» статически нельзя. Цена
    лишнего вопроса ниже цены пропущенной подмены цепочки.
"""
import json
import sys

# Read-only разведка: не пишет, ролевую цепочку не подменяет.
ALLOW_READONLY = {"Explore", "Plan"}

# Роли ЦЕПОЧКИ гейтов. Их запуск агентом — прямое нарушение user-as-dispatcher:
# автор работы не назначает себе проверяющего.
CHAIN_ROLES = {
    "critic",
    "risk-critic",
    "reviewer",
    "tester",
    "engine-dev",
    "venue-dev",
    "signal-engineer",
    "research-dev",
}

try:
    data = json.load(sys.stdin)
except Exception:
    data = {}

tool_input = data.get("tool_input") or {}
subagent = tool_input.get("subagent_type") or "(не указан)"
desc = tool_input.get("description") or ""

if subagent in ALLOW_READONLY:
    decision = "allow"
    reason = f"read-only lookup-агент «{subagent}» — auto-allow: кода не пишет, цепочку ролей не подменяет"
elif subagent in CHAIN_ROLES:
    decision = "ask"
    reason = (
        f"⛔ «{subagent}» — РОЛЬ ЦЕПОЧКИ ГЕЙТОВ ({desc or 'без описания'}). "
        "Автор работы не назначает себе проверяющего: CLAUDE.md «агенты не вызывают друг "
        "друга», architect.md «прочие роли отсюда НЕ ЗАПУСКАЮТСЯ». Штатный путь — Handoff §D "
        "paste-ready промпт, который запускает founder. Одобряйте, ТОЛЬКО если вы сами просили "
        "этот запуск; иначе Deny — агент обязан выдать Handoff."
    )
else:
    decision = "ask"
    reason = (
        f"Запуск субагента «{subagent}» ({desc or 'без описания'}): решение founder'а "
        "(CLAUDE.md, user-as-dispatcher). Одобряйте, если просили сами или согласны с "
        "обоснованием агента."
    )

print(
    json.dumps(
        {
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": decision,
                "permissionDecisionReason": reason,
            }
        },
        ensure_ascii=False,
    )
)
