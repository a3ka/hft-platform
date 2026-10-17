#!/usr/bin/env bash
# install-watchdog.sh — доставка `ops-watchdog` из образа работающего контейнера
# `hft-recorder` на хост (M-93, TD-231; DESIGN §23.1).
#
# КОНСТРУКЦИЯ.
# Деплой после healthy-гейта (задача 3, `.github/workflows/deploy.yml`) вызывает этот скрипт.
# Скрипт читает `docker inspect -f '{{.Image}}' hft-recorder` — образ РАБОТАЮЩЕГО контейнера
# (на ветке healthy — новая сборка, на ветке отката — `PREV`; на хосте всегда бинарь того
# кода, что в контейнерах, §3 I-5), поднимает из него временный контейнер (`docker create`),
# копирует `/usr/local/bin/ops-watchdog` во временный файл РЯДОМ с целевым, удаляет временный
# контейнер, проверяет непустоту, `chmod 0755` и `mv -f` в целевой путь.
#
# На любом отказе (`inspect`/`create`/`cp`/пустой файл) — `exit≠0`, ПРЕЖНИЙ бинарь цел
# (`mv` атомарен: `rename(2)` в той же директории), временный контейнер удалён (EXIT-trap),
# хвостов в каталоге назначения нет (EXIT-trap снимает `DST.new.$$` при ненулевом коде).
# Это и есть §3 I-3 (fail-closed установки) — деплой увидит «=== WATCHDOG INSTALL FAILED ===»
# и красный шаг, ничего не «протекает» в прод-каталог.
#
# ШОВ ГЕЙТА (A-046 §2, класс DESTDIR / RETENTION_RUNNER / CHECKPOINT_RUNNER).
# Три переменные — `HFT_WATCHDOG_ROOT`, `HFT_WATCHDOG_DST`, `HFT_WATCHDOG_CONTAINER` — на
# проде НЕ задаются. Их роль — ДЕФОЛТЫ и ПЕСОЧНИЦА гейта, не подмена прода:
#   • `HFT_WATCHDOG_DST`       — полный путь установки (на проде пуст, путь неизменен);
#   • `HFT_WATCHDOG_ROOT`      — корень установки (префикс пути; `DESTDIR`-класс). На проде
#     пуст; в пробе `w1b` подставляет каталог песочницы, и ОБЕ стороны (этот скрипт и
#     `scripts/watchdog_cron.sh`) читают ту же переменную, поэтому пути согласованы;
#   • `HFT_WATCHDOG_CONTAINER` — имя контейнера, чей `inspect` даёт образ; на проде
#     `hft-recorder`.
# Ни `deploy.yml` (задача 3, после этой), ни `deploy/cron.d/watchdog`, ни `/etc/cron.d/
# hft-watchdog`, ни `/etc/environment` их НЕ задают — это и есть `I-1` (путь установки =
# путь cron'а, шов одинаков на обеих сторонах). Проверяет §8 задача 4 (reviewer,
# `grep -c 'HFT_WATCHDOG_ROOT\|HFT_WATCHDOG_DST\|WATCHDOG_BIN' /etc/cron.d/hft-watchdog
# /etc/environment` → 0 на обоих).
#
# `HFT_INSTALL_WATCHDOG_PRINT_DST=1` — печатает `DST` в stdout, выходит 0, docker НЕ зовёт.
# Используется `w1`/`w1b` оракула композиции: путь, который печатает установка, РАВЕН пути,
# который зовёт cron (тот же `HFT_WATCHDOG_ROOT` с обеих сторон, см. `scripts/watchdog_cron.sh`).
#
# Деплой исполняется root'ом (A-046 §5) — `sudo` НЕ нужен: `mkdir -p /usr/local/lib/hft` в
# первом деплое создаёт корень-каталог, последующие деплои кладут бинарь. Если когда-нибудь
# `VPS_USER` сменится — `mkdir` откажет, `set -e` даст exit≠0, деплой красный. Это fail-closed
# поведение, не тихое — ровно то, что требует §3 I-3.

set -euo pipefail

DST="${HFT_WATCHDOG_DST:-${HFT_WATCHDOG_ROOT:-}/usr/local/lib/hft/ops-watchdog}"
CONTAINER="${HFT_WATCHDOG_CONTAINER:-hft-recorder}"

# Печать пути — ДО любых side-эффектов (mkdir/docker). Это контракт композиции w1/w1b:
# установка и cron вычисляют один путь при одинаковом `HFT_WATCHDOG_ROOT`.
if [ "${HFT_INSTALL_WATCHDOG_PRINT_DST:-0}" = "1" ]; then
  printf '%s\n' "${DST}"
  exit 0
fi

# Каталог назначения. Отказ `mkdir` = отказ установки (`set -e` ⇒ exit≠0; `deploy.yml`
# увидит «=== WATCHDOG INSTALL FAILED ===» и красный шаг, fail-closed).
mkdir -p "$(dirname "${DST}")"

CID=""
TMP="${DST}.new.$$"

# EXIT-trap: на ЛЮБОМ исходе контейнер удалён (в т.ч. при отказе `cp`, w3) и на ненулевом
# коде временный файл снят (w3/w5). `return` здесь — только для чистоты стека; exit-code
# скрипта определяется местом, где `set -e` сработал, не return'ом trap'а.
cleanup() {
  local rc=$?
  [ -n "${CID}" ] && docker rm "${CID}" >/dev/null 2>&1 || true
  if [ "$rc" -ne 0 ]; then
    [ -e "${TMP}" ] && rm -f "${TMP}" >/dev/null 2>&1 || true
  fi
  return "$rc"
}
trap cleanup EXIT

# Шаг 1: образ РАБОТАЮЩЕГО контейнера. Не задаётся литералом — `inspect` берёт то, что
# реально крутится (запрет «образа захардкожен» из §9 M6: в пробе `w2` образ случайный,
# в проде `deploy.yml` уже поднял новый контейнер к этому моменту).
IMG="$(docker inspect -f '{{.Image}}' "${CONTAINER}")" || exit 1

# Шаг 2: поднять временный контейнер из этого образа. На отказе `inspect` (w4) `set -e`
# уже вышел, `create` НЕ зовётся — `w4` судит именно это: «после отказа inspect create не
# позван».
CID="$(docker create "${IMG}")" || exit 1

# Шаг 3: атомарная копия во временный файл РЯДОМ с DST (не в DST — `mv` сработает одним
# `rename(2)` в той же директории). На отказе `cp` (w3) — `set -e` → trap удаляет контейнер
# (`rm cid-…` в логе) И снимает `DST.new.$$`. Прежний бинарь цел, в каталоге только он.
if ! docker cp "${CID}:/usr/local/bin/ops-watchdog" "${TMP}"; then
  exit 1
fi

# Шаг 4: контейнер удаляется ДО проверки размера. Иначе при пустом файле (w5) остался бы
# хвост в `docker ps -a` — ровно тот класс «деплой зелёный, мусор остался», что и
# устраняет эта явная очистка.
docker rm "${CID}" >/dev/null 2>&1 || true
CID=""

# Шаг 5: защита от пустого файла (`cp` мог «успешно» отдать 0 байт — w5). НЕ `mv` —
# прежний бинарь цел, в каталоге только он.
if [ ! -s "${TMP}" ]; then
  exit 1
fi

# Шаг 6: права и атомарная подмена. `mv -f` в той же директории = один `rename(2)`.
chmod 0755 "${TMP}"
mv -f "${TMP}" "${DST}"
