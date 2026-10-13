#!/usr/bin/env bash
# Гейт деплоя профиля расчётов (M-94, §3.7, инвариант I-6).
#
# НАЗНАЧЕНИЕ.
# Препятствует перезапуску выдачи под профилем, который:
#  (1) дублируется в host `.env` (милестоун §3.7 шаг 1) — тогда «наличие»
#      уже не отказ: оператор переопределил ось через env, и `GATEWAY_CALC_PROFILE`
#      на проде превратился в тыкву; `deploy.yml` поднимет контейнер,
#      `gateway-serve` стартанёт, профиль проигнорируется, выдача
#      разойдётся с прогревателем — класс TD-227-2023-08-14 (8 суток отказа);
#  (2) не имеет слепка в томе слепков (милестоун §3.7 шаг 2) — тогда после
#      `up -d` `gateway-serve` будет отдавать `not_ready` каждому клиенту
#      до тех пор, пока `gateway-checkpoint` не пройдёт ПОЛНЫЙ цикл прогрева
#      (≈16–23 мин на проде, замер M-84/R-187). Скрыть это health-гейтом
#      нельзя: контейнер будет `healthy` (TCP-bind жив), а кокпит пуст.
#
# ШОВ ГЕЙТА (§3.7, контракт пробы `red_m94_deploy_gate.sh`):
#   HFT_ROOT              — каталог с `config/calc-profile/active.env` (на проде
#                          /root/hft-platform). Используется только для чтения
#                          `.env`; в compose этот путь тоже лежит (милестоун
#                          §3.4 — образ собирает «приложение» в себя);
#   CALC_GATE_RUNNER      — команда, заменяющая `docker compose run ...` для
#                          получения имени слепка нового образа (на проде `docker`,
#                          в пробе — заглушка). По умолчанию `docker`;
#   CALC_GATE_CKPT_HOST_DIR — путь к RW-тому `gateway-ckpt` НА ХОСТЕ
#                          (на проде /var/lib/docker/volumes/.../_data; в
#                          пробе — temp-каталог).
#
# КОНТРАКТ ВЫХОДА (милестоун §3.7, оракулы g1..g6):
#   exit 0  — оба отказа отсутствуют; runner позван с `compose run --rm --no-deps
#             gateway-checkpoint --print-ckpt-name`, имя валидно (`ckpt-<16hex>.bin`)
#             и файл существует в `$CALC_GATE_CKPT_HOST_DIR`;
#   exit != 0 — ключ профиля в `.env` ИЛИ слепок отсутствует ИЛИ runner
#             упал ИЛИ runner напечатал не-имя. Имя ключа/файла ВСЕГДА в stderr
#             (оператор видит, ГДЕ править).
#
# БОКОВЫХ ДЕЙСТВИЙ НЕТ: гейт НЕ устанавливает cron, НЕ делает `docker compose up`,
# НЕ трогает git-чекаут. Это ИСКЛЮЧИТЕЛЬНО проверка; следующий шаг деплоя
# (или его откат) — тело `deploy/bin/deploy-apply.sh` (контракт §3.7).
set -uo pipefail

# ──────────────── швы ────────────────
HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
CALC_GATE_RUNNER="${CALC_GATE_RUNNER:-docker}"
CALC_GATE_CKPT_HOST_DIR="${CALC_GATE_CKPT_HOST_DIR:-/var/lib/docker/volumes/hft-platform_gateway-ckpt/_data}"

# ──────────────── (1) host `.env` без ключей профиля ────────────────
#
# Источник запрета — `П-032` п.1: «наличие ключа в env при `GATEWAY_CALC_PROFILE`
# — отказ». Если host `.env` всё ещё несёт `GATEWAY_BANDS=0.015,…` (как до M-94),
# выдача с новым образом ЗАПУСТИТСЯ, но профиль увидит «наложение» и стартанёт
# нет. Здесь это поймано РАНЬШЕ подъёма — деплой физически не доходит до
# `up -d`. Градации — `KEY=`, `export KEY=`, с ведущими пробелами (оракул `g3`).
#
# Ровно эти 9 ключей проверяются: 7 профиля + `GATEWAY_CANONICAL_BANDS`
# (третий носитель полос, выводится из оборота — милестоун §3.1) + `CALC_PROFILE_VERSION`
# (теперь часть профиля, его наличие в env вне профиля неуместно). `g3c` —
# контр-мир: комментарий `# KEY=…` и посторонняя `MY_KEY=…` НЕ считаются.
if [ -f "${HFT_ROOT}/.env" ]; then
  bad_keys=$(grep -E '^[[:space:]]*(export[[:space:]]+)?(GATEWAY_BANDS|GATEWAY_CANONICAL_BANDS|GATEWAY_DEPTH_CADENCE_MS|GATEWAY_TIMEFRAME_MS|GATEWAY_WINDOW_MS|GATEWAY_ALLOWED_PROFILES|GATEWAY_VP_BIN_WIDTH_E8|GATEWAY_HEATMAP_WINDOW|CALC_PROFILE_VERSION)=' "${HFT_ROOT}/.env" || true)
  if [ -n "${bad_keys}" ]; then
    echo "calc-profile-gate: REFUSED: host .env несёт ключ(и) профиля — деплой переключит выдачу на ПРОШЛЫЙ источник оси (TD-227, 8 суток отказа выдачи):" >&2
    echo "${bad_keys}" | sed 's/^/  /' >&2
    echo "Удалите эти строки из ${HFT_ROOT}/.env и повторите деплой. После merge M-94 строка GATEWAY_BANDS в host .env БОЛЬШЕ не нужна (милестоун задача 11)." >&2
    exit 1
  fi
fi

# ──────────────── (2) имя слепка от НОВОГО образа + файл существует ────────────────
#
# Имя запрашивается у НОВОГО образа: `${CALC_GATE_RUNNER} compose run --rm --no-deps
# gateway-checkpoint --print-ckpt-name`. `--no-deps` — чтобы не дёргать
# recorder/journal; `--rm` — одноразовый контейнер.
#
# Ожидаемая форма stdout: `ckpt-<16hex>.bin` (ровно 16 hex и суффикс `.bin`).
# ЛЮБОЕ другое содержимое — отказ (милестоун §3.7: «runner печатает не имя
# слепка» ⇒ отказ; оракул `g5`).
ckpt_name=$(${CALC_GATE_RUNNER} compose run --rm --no-deps gateway-checkpoint --print-ckpt-name 2>/dev/null) || {
  echo "calc-profile-gate: REFUSED: runner ${CALC_GATE_RUNNER} compose run … упал или не напечатал имя — гейт без имени слепка не пропустит (I-6, §3.7 шаг 2)" >&2
  exit 1
}

# Форма имени: жёсткая проверка. `DefaultHasher` детерминирован для
# `selector_fingerprint`, `fnv1a` не используется (милестоун §2, `selector_fingerprint`).
if ! [[ "${ckpt_name}" =~ ^ckpt-[0-9a-f]{16}\.bin$ ]]; then
  echo "calc-profile-gate: REFUSED: runner напечатал «${ckpt_name}» — это не ckpt-<16hex>.bin (I-6, §3.7)" >&2
  exit 1
fi

# Файл в томе: `${CALC_GATE_CKPT_HOST_DIR}/${ckpt_name}`.
if [ ! -f "${CALC_GATE_CKPT_HOST_DIR}/${ckpt_name}" ]; then
  echo "calc-profile-gate: REFUSED: слепок нового профиля отсутствует в ${CALC_GATE_CKPT_HOST_DIR}/${ckpt_name} — прогревать, потом переключать (I-6, §3.7 шаг 2). Возможные причины: (а) холодная пересборка не завершилась (≈16–23 мин, замер M-84/R-187); (б) `gateway-checkpoint` упал при прогреве (лог /var/log/hft/gateway-checkpoint.log)." >&2
  exit 1
fi

# Оба отказа отсутствуют. Гейт пропустит.
exit 0
