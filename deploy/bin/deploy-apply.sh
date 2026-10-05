#!/usr/bin/env bash
# Тело деплоя (M-94, §3.7; C-280 R3): шаги после `git reset --hard TARGET_SHA`
# (их делает `deploy.yml` через ssh-action) и до успешного healthy обеих служб
# (recorder + gateway-serve). Проводка деплоя — вызов ЭТОГО скрипта с PREV-аргументом
# через `bash deploy/bin/deploy-apply.sh "$PREV"` после reset на TARGET.
#
# ШОВ ДЕПЛОЯ (контракт пробы `red_m94_deploy_apply.sh`, спека §3.7):
#   PREV (аргумент)    — SHA, на который откатываемся при ЛЮБОМ отказе
#                         (gate / cron / health / watchdog). Это та ревизия,
#                         которая реально крутилась на VPS до этого деплоя
#                         (замер deploy.yml через `git rev-parse HEAD` ДО reset).
#   DEPLOY_DOCKER      — замена `docker` (по умолчанию). Проба подставляет
#                         заглушку, которая отвечает на `inspect`/`logs`/`prune`/
#                         `compose build|up` и логирует SHA в момент вызова.
#   DEPLOY_GATE        — замена `bash deploy/bin/calc-profile-gate.sh` (default).
#                         Проба подставляет шум с заданным `gate_rc`.
#   DEPLOY_CRON_DIR    — куда ставятся cron-файлы (default `/etc/cron.d`).
#   DEPLOY_SUDO        — префикс `install` (default `sudo`; пусто — без него).
#   DEPLOY_CRON_VALIDATE — валидатор cron-файлов (default `crontab -n`). Проба
#                         подставляет стаб, отвергающий `INVALID-CRON-MARKER`.
#   DEPLOY_WATCHDOG_INSTALL — доставка сторожа (default `bash deploy/bin/
#                         install-watchdog.sh`). Проба подставляет стаб, логирующий
#                         вызов.
#   DEPLOY_HEALTH_TIMEOUT — секунды ожидания healthy (default 300).
#
# ПОРЯДОК ШАГОВ (спека §3.7 таблица). Все 5 — fail-closed; откат — ВСЕГДА
# `git reset --hard -q "$PREV" && docker compose build && (cron install) && up -d
# на PREV`. После отката чекаут = PREV, тег образа = PREV (команда `up -d`
# без пересборки подняла бы старый тег от TARGET — `A-049` Р-1), cron = PREV
# (важно: `C-281` B1 — откат переустанавливает cron).
#
#   1. `docker compose build` (образ нового кода).
#   2. гейт профиля (отказ ⇒ шаг отказа).
#   3. валидация cron, затем установка (отказ валидации/установки ⇒ шаг отказа).
#   4. `docker compose up -d --build recorder gateway-serve` + ожидание
#      `healthy` ОБЕИХ служб (отказ любой ⇒ шаг отказа + логи).
#   5. `bash deploy/bin/install-watchdog.sh` (отказ ⇒ exit≠0, отката нет —
#      тревоги без сторожа быть не должно, контракт M-93).
#   6. `docker image prune -f` (best-effort).
set -euo pipefail

PREV="${1:?deploy-apply.sh: требуется PREV (SHA) аргументом}"
# `${VAR-default}`: дефолт ТОЛЬКО если переменная НЕ ЗАДАНО (в отличие от
# `${VAR:-default}`, который срабатывает и на unset, и на empty). Это
# критично для `DEPLOY_SUDO=""` (проба `red_m94_deploy_apply.sh` снимает
# префикс `sudo` для теста; `${VAR:-sudo}` бы всё равно подставил sudo).
DEPLOY_DOCKER="${DEPLOY_DOCKER-docker}"
DEPLOY_GATE="${DEPLOY_GATE-bash deploy/bin/calc-profile-gate.sh}"
DEPLOY_CRON_DIR="${DEPLOY_CRON_DIR-/etc/cron.d}"
DEPLOY_SUDO="${DEPLOY_SUDO-sudo}"
DEPLOY_CRON_VALIDATE="${DEPLOY_CRON_VALIDATE-crontab -n}"
DEPLOY_WATCHDOG_INSTALL="${DEPLOY_WATCHDOG_INSTALL-bash deploy/bin/install-watchdog.sh}"
DEPLOY_HEALTH_TIMEOUT="${DEPLOY_HEALTH_TIMEOUT-300}"

# Внутренние функции (без отдельных скриптов — проба читает ТЕКСТ этого
# файла через `grep`, и вынесение в обёртку ухудшило бы её видимость).

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }

# reset_and_build_prev — общий шаг отказа ДО выдачи: reset чекаута +
# пересборка образа на PREV. НИКАКОГО `up` здесь нет (милестоун §3.7
# таблица шагов 2 и 3: «up нет» при отказе гейта/валидации cron; оракулы
# `a1`/`a6`/«g0» — проба ПРОВЕРЯЕТ отсутствие `up@` в `docker.log`).
# Именно так оставленный за TARGET тег образа, доехавший до cron,
# может прогреть новый профиль, пока выдача работает на старом.
# Пересборка ОБЯЗАНА идти ПОСЛЕ reset (A-049 Р-1: `up -d` без пересборки
# поднял бы старый тег от TARGET).
reset_and_build_prev() {
  local reason="$1"
  log "=== ROLLBACK (no up) to PREV=$PREV reason=$reason ==="
  git reset --hard -q "$PREV"
  ${DEPLOY_DOCKER} compose build || log "WARN: build на PREV провалился (нештатно)"
}

# full_rollback_prev — откат ПОСЛЕ частичного подъёма (health_failed). cron
# и up ОБЯЗАНЫ быть на PREV (милестоун §3.7 шаг 4: «cron заново из PREV,
# build на PREV, up на PREV»; `C-281` B1: прежний деплой оставлял cron
# от TARGET после отката — дыра).
full_rollback_prev() {
  local reason="$1"
  log "=== ROLLBACK (with up) to PREV=$PREV reason=$reason ==="
  ${DEPLOY_DOCKER} compose build || log "WARN: build на PREV провалился (нештатно)"
  # cron ПЕРЕСОБРАН из PREV (текущий чекаут = PREV).
  for src in deploy/cron.d/*; do
    [ -f "$src" ] || continue
    name="$(basename "$src")"
    if ! ${DEPLOY_CRON_VALIDATE} "$src"; then
      log "WARN: cron файл PREV $src не прошёл валидацию — пропускаю"
      continue
    fi
    ${DEPLOY_SUDO} install -m 0644 "$src" "${DEPLOY_CRON_DIR}/hft-${name}" \
      || log "WARN: install $src → ${DEPLOY_CRON_DIR}/hft-${name} упал (некритично)"
  done
  ${DEPLOY_DOCKER} compose up -d --build recorder gateway-serve || {
    log "WARN: up на PREV упал — прод в рассогласованном состоянии (см. логи контейнеров)"
  }
}

# Шаг 1: сборка образа нового кода (TARGET). На отказе — `reset_and_build_prev`
# с причиной "build failed" + exit ≠ 0. НИКАКОГО `up` здесь нет (спека §3.7
# шаг 1: «на отказе — exit ≠ 0»; сборка НЕ поднимает выдачу, только тег).
log "=== STEP 1: docker compose build (TARGET) ==="
if ! ${DEPLOY_DOCKER} compose build; then
  log "=== BUILD FAILED — rollback (no up) ==="
  reset_and_build_prev "build_failed"
  exit 1
fi

# Шаг 2: гейт профиля (милестоун §3.7 шаг 1+2). Отказ ⇒ reset чекаута,
# пересборка образа на PREV, `up` НЕТ. Контракт оракулов `a1`: в `docker.log`
# НЕ должно быть строки `up@` после отказа гейта.
log "=== STEP 2: calc-profile-gate ==="
if ! ${DEPLOY_GATE}; then
  log "=== GATE REFUSED — rollback (no up) ==="
  reset_and_build_prev "gate_refused"
  exit 1
fi

# Шаг 3: валидация cron до установки. `crontab -n` — стандартный «dry-run»
# cron'а. Файлы, не прошедшие парсер, НЕ устанавливаются (не накапливаются
# полузаписанные записи, которые приведут к «bad minute» — инцидент M-48 rev3,
# R-160: «cron-файл инертен на проде»). `A-049` Р-3: отказ валидации
# БЛОКИРУЕТ установку ВСЕХ файлов, не только битого (иначе частично
# установленный cron = смесь старого и нового). На отказе валидации:
# reset + build + exit; `up` НЕТ (спека §3.7 шаг 3: «up нет»).
log "=== STEP 3: validate cron files ==="
cron_bad=0
for src in deploy/cron.d/*; do
  [ -f "$src" ] || continue
  if ! ${DEPLOY_CRON_VALIDATE} "$src"; then
    log "=== CRON INVALID (NOT INSTALLED): $src — abort ==="
    cron_bad=1
    break
  fi
done
if [ "$cron_bad" -ne 0 ]; then
  log "=== CRON VALIDATION FAILED — rollback (no up) ==="
  reset_and_build_prev "cron_invalid"
  exit 1
fi

# Установка ПОСЛЕ успешной валидации. На отказе установки (т.е. `install` упал
# ДЛЯ КОНКРЕТНОГО файла) — `full_rollback_prev`: выдача УЖЕ может крутиться
# (часть cron установлена, up ещё не вызван, но docker compose build
# уже выполнен — образ TARGET собран). Здесь спека прямо не говорит «up
# нет» — но сценарий «частично установленный cron» + «выдача не
# перезапущена» не отличался бы от успешного деплоя по поведению кокпита
# (cron наполовину от TARGET ⇒ разные активные конфиги), а на откат без up
# оператор увидит прод с битым cron. Безопаснее: up на PREV, как при
# отказе здоровья (т.е. тот же `full_rollback_prev`).
for src in deploy/cron.d/*; do
  [ -f "$src" ] || continue
  name="$(basename "$src")"
  if ! ${DEPLOY_SUDO} install -m 0644 "$src" "${DEPLOY_CRON_DIR}/hft-${name}"; then
    log "=== CRON INSTALL FAILED ($src) — rollback (with up) ==="
    git reset --hard -q "$PREV"
    full_rollback_prev "cron_install_failed"
    exit 1
  fi
  log "installed ${DEPLOY_CRON_DIR}/hft-${name}"
done

# Шаг 4: `up -d --build` ОБЕИХ служб + ожидание healthy КАЖДОЙ (милестоун
# §3.7 шаг 4: «health-гейт обеих fail-closed»; `A-049` Р-2: «состояние, не
# константа» — ждём ПОСЛЕДОВАТЕЛЬНОСТЬ `starting → healthy`, а не «всё кроме
# unhealthy = здорово»). `starting` вечно ⇒ таймаут.
log "=== STEP 4: docker compose up -d --build recorder gateway-serve ==="
${DEPLOY_DOCKER} compose up -d --build recorder gateway-serve || {
  log "=== UP FAILED — rollback (with up) ==="
  git reset --hard -q "$PREV"
  full_rollback_prev "up_failed"
  exit 1
}

wait_healthy() {
  local svc="$1" deadline=$(( $(date +%s) + DEPLOY_HEALTH_TIMEOUT ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    status="$(${DEPLOY_DOCKER} inspect -f '{{.State.Health.Status}}' "$svc" 2>/dev/null || true)"
    case "$status" in
      healthy) return 0 ;;
      unhealthy|missing) return 1 ;;
      starting|"") sleep 1; continue ;;
      *) sleep 1; continue ;;
    esac
  done
  return 1
}

healthy_ok=1
if ! wait_healthy hft-recorder; then
  log "=== hft-recorder NOT healthy within ${DEPLOY_HEALTH_TIMEOUT}s — rollback ==="
  healthy_ok=0
fi
if ! wait_healthy hft-gateway-serve; then
  log "=== hft-gateway-serve NOT healthy within ${DEPLOY_HEALTH_TIMEOUT}s — rollback ==="
  healthy_ok=0
fi
if [ "$healthy_ok" -ne 1 ]; then
  # Логи ОБЕИХ служб (милестоун §3.7 шаг 4: «при отказе здоровья снимать
  # логи обеих»; иначе §8-глазам не на что смотреть). stderr в `inspect`
  # даст «No such object», если контейнер не успел подняться — это
  # тоже сигнал (не молчание).
  ${DEPLOY_DOCKER} logs hft-recorder --tail 50 || true
  ${DEPLOY_DOCKER} logs hft-gateway-serve --tail 50 || true
  git reset --hard -q "$PREV"
  full_rollback_prev "health_failed"
  exit 1
fi

# Шаг 5: установка сторожа (M-93, TD-231). Контракт `install-watchdog.sh`:
# `exit≠0` = отказ доставки (бинарь не атомарен; `|| true` НЕ используем —
# тревоги без сторожа быть не должно).
log "=== STEP 5: install watchdog ==="
if ! ${DEPLOY_WATCHDOG_INSTALL}; then
  log "=== WATCHDOG INSTALL FAILED ==="
  exit 1
fi

# Шаг 6: очистка висящих образов (после `up` — оставляем сначала тег, чтобы
# `up` сам себя не убил). Best-effort: на отказе `prune` деплой НЕ
# проваливается (образы переживут ещё один деплой).
log "=== STEP 6: docker image prune -f ==="
${DEPLOY_DOCKER} image prune -f || true

log "=== DEPLOY OK: TARGET deployed, both services healthy, watchdog installed ==="
exit 0
