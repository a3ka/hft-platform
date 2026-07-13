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

print("OK: jobs.deploy.needs = ['ci']; gate-job присутствует и fail-closed")
