# shellcheck shell=bash
# compose_deploy_form.sh — compose разбирается в ПРОД-ФОРМЕ окружения деплоя (шаг D10 гейта
# доставки; M-92, Deploy 37242446986, R-241).
#
# Деплой зовёт `docker compose up -d --build recorder gateway-serve` в каталоге проекта:
#   * без `-f` — значит compose читает и `docker-compose.override.yml`, если он лежит рядом;
#   * с `.env`, где ровно две переменные (замер VPS 2026-10-04: GATEWAY_JWT_SECRET, GATEWAY_BANDS);
#   * compose интерполирует ВЕСЬ файл, а не только поднимаемые сервисы: переменная без значения
#     по умолчанию в ЛЮБОМ сервисе роняет деплой целиком.
# Проверка повторяет эту форму: окружение очищено до двух имён, `.env` каталога не читается
# (`--env-file /dev/null`, чтобы локальный `.env` разработчика не маскировал дефект), включены
# все профили (служба уборки живёт под профилем `ops`), источник тома `/work` у
# `journal-retention` обязан быть значением по умолчанию.
#
# ПРЕДЕЛ, названный честно: файлы, которые есть только на VPS (например override оператора),
# проверка не видит; сегодня override на VPS не содержит подстановок (R-241 Н-1). Подстановка
# без умолчания в `environment:` даёт лишь предупреждение и пустое значение — деплой не падает,
# проверка тоже (R-241 Н-4, вне объёма).
#
# Использование: compose_deploy_form_check <каталог проекта> <ожидаемый источник /work>
# Код возврата 0 — разбор прошёл и источник совпал; печатает одну итоговую строку и, при
# отказе, причину.

compose_deploy_form_check() {
  local dir="$1" want="$2" err json src rc
  err=$(mktemp)
  json=$(cd "${dir}" && env -i PATH="${PATH}" HOME="${HOME:-/tmp}" \
           GATEWAY_JWT_SECRET=d10-probe GATEWAY_BANDS=d10-probe \
           docker compose --env-file /dev/null --profile "*" config --format json 2>"${err}") \
    && rc=0 || rc=$?
  if [ "${rc}" -ne 0 ]; then
    echo "compose-deploy-form: разбор НЕ прошёл в окружении деплоя (rc=${rc}):"
    sed 's/^/  /' "${err}" | head -3
    rm -f "${err}"
    return 1
  fi
  rm -f "${err}"
  src=$(printf '%s' "${json}" | python3 -c '
import json,sys
svc=json.load(sys.stdin)["services"].get("journal-retention",{})
print(next((v.get("source","") for v in svc.get("volumes",[]) if v.get("target")=="/work"),""))' 2>/dev/null) \
    || src="<разбор JSON не удался>"
  if [ "${src}" = "${want}" ]; then
    echo "compose-deploy-form: разбор прошёл; /work ← ${src}"
    return 0
  fi
  echo "compose-deploy-form: источник /work у journal-retention = '${src}', ожидался ${want}"
  return 1
}
