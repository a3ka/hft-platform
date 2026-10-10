# M-43 — ops hardening: ресурс-лимиты + проброс порта + watchdog (R10, ШАГ 1c/4)

**Статус:** PLANNED (стаб). **Риск:** R10 HIGH (`docs/08`). Предусловие живого кокпита.

## Objective
- **(а) 0 ресурс-лимитов в compose** → скачок памяти gateway-serve (класс TD-039/044) уводит ВЕСЬ хост в
  OOM и роняет recorder за компанию (нет cgroup-изоляции виновника). Recorder = сбор данных, защитить приоритетно.
- **(б) gateway-serve на loopback, нет `ports:`/reverse-proxy** → WS-эндпоинт кокпита НЕДОСТИЖИМ ниоткуда.
  Блокирует продуктовую цель P-COCKPIT. Healthcheck не ловит (стучит в loopback изнутри контейнера).
- **(в) push-алертинг не задеплоен** → слепота между ssh-проверками §8.

## Allowed paths
- `docker-compose.yml` (mem_limit, ports) · deploy/ops-скрипты (reverse-proxy/tunnel runbook, cron-watchdog) · `docs/fa/ops.md`.

## Задачи
1. `mem_limit` (+cpus) на КАЖДЫЙ сервис; recorder защитить приоритетно (виновник-OOM не должен ронять сбор).
2. Проброс порта gateway-serve + reverse-proxy/ssh-туннель runbook (синхронизировать с M-38b/M-39 — там shared-tailer).
3. Минимальный cron-watchdog→Telegram (heartbeat свежесть + disk_free) как мост; полный Prometheus — BACKLOG.

## §8: изменение compose трогает деплой-путь → post-merge деплой-гейт обязателен (контейнеры healthy, recorder не задет).
## Гейты: reviewer + §8. risk-critic не нужен (нет order-path). Осторожно: mem_limit НЕ должен душить recorder (проверить eyes-on).
## Cross-ref: docs/08 R10, docs/fa/ops.md.
