#!/usr/bin/env python3
"""M-08 E9/M6: структурная проверка, что деплой ГЕЙТИТСЯ на CI.

Грепа `needs: ci` недостаточно (C-005 M5): его удовлетворит комментарий или чужой job.
Здесь парсится YAML и проверяется РЕАЛЬНАЯ зависимость jobs.deploy.needs → ci,
плюс что gate-job существует и fail-closed по таймауту.
"""
import sys

import yaml

with open(".github/workflows/deploy.yml", encoding="utf-8") as f:
    wf = yaml.safe_load(f)

# TD-018: без `actions: read` gate-job получает 403 на gh api и деплой не пройдёт НИКОГДА
# (fail-closed превращается в fail-forever). Гейт обязан ловить это статически.
perms = wf.get("permissions") or {}
if isinstance(perms, str):
    sys.exit(f"permissions: {perms!r} — нужен явный блок с actions: read (TD-018)")
if perms.get("actions") != "read":
    sys.exit(
        "deploy.yml: нет `permissions.actions: read` — gate-job упрётся в 403 на "
        "`gh api actions/runs` и НИКОГДА не разрешит деплой (TD-018: прод замерзает "
        "при зелёном CI)"
    )

jobs = wf.get("jobs", {})
if "deploy" not in jobs:
    sys.exit("deploy.yml: нет job `deploy`")

needs = jobs["deploy"].get("needs")
needs = [needs] if isinstance(needs, str) else (needs or [])
if "ci" not in needs:
    sys.exit(f"jobs.deploy.needs = {needs!r} — деплой НЕ гейтится на CI (красный CI выкатит прод)")

if "ci" not in jobs:
    sys.exit("deploy.yml: job `ci` (gate) отсутствует, а deploy на него ссылается")

gate = yaml.safe_dump(jobs["ci"])
if "exit 1" not in gate:
    sys.exit("gate-job `ci` не fail-closed: нет явного `exit 1` при не-success/таймауте")

print("OK: permissions.actions=read; jobs.deploy.needs=['ci']; gate-job fail-closed")
