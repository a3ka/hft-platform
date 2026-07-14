#!/usr/bin/env bash
# M-08 задача 14 — ГЕЙТ ДОСТАВКИ (TD-020, второй виток).
#
# Урок, который мы выучили дважды:
#   виток 1: библиотека (ColdCopyProof/prune) написана — её НИКТО не вызывал;
#   виток 2: бинарь journal-retention написан, 7 оракулов GREEN — а в ПРОДЕ его НЕТ:
#            Dockerfile собирает только recorder, на VPS нет rust, cron нет, cold нет.
# `cargo test GREEN` ≠ «функция существует в проде». Поэтому гейт доставки — НЕ греп по
# Dockerfile, а РЕАЛЬНАЯ сборка прод-образа и ЗАПУСК бинаря внутри него.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

FAILED=0
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
pass() { echo "PASS  $*"; }

IMG="hft-m08-delivery-check:local"
DEEP="${HFT_DELIVERY_DEEP:-0}"   # 1 = реально собрать образ и ЗАПУСТИТЬ бинарь (CI-job, §8)

# ── D1: бинарь ОБЯЗАН попадать в прод-образ ───────────────────────────────────────────
# Структурно (быстро, для tester'а): Dockerfile СОБИРАЕТ и КОПИРУЕТ journal-retention.
if grep -qE -- '--bin[[:space:]]+journal-retention' Dockerfile \
   && grep -qE 'COPY --from=builder .*journal-retention' Dockerfile; then
  pass "D1 Dockerfile собирает и копирует journal-retention в прод-образ"
else
  fail "D1 Dockerfile НЕ кладёт journal-retention в прод-образ — на VPS нет rust-toolchain, \
значит ретеншен физически нечем вызвать. Это TD-020, виток 2: оператор без доставки."
fi
if grep -qE 'COPY --from=builder .*recorder' Dockerfile; then
  pass "D2 recorder в образе цел (доставка ретеншена не сломала сбор)"
else
  fail "D2 recorder исчез из Dockerfile — правка сломала СБОР ДАННЫХ"
fi

# ── D1-deep: НАСТОЯЩЕЕ доказательство — собрать образ и ЗАПУСТИТЬ бинарь ──────────────
# Греп по Dockerfile можно удовлетворить, не получив рабочего бинаря. Поэтому в CI и на §8
# гоняем deep: «функция существует в проде» доказывается ЗАПУСКОМ, а не текстом.
if [ "${DEEP}" = "1" ]; then
  if ! command -v docker >/dev/null 2>&1; then
    fail "D1-deep docker недоступен — доставку нечем ДОКАЗАТЬ"
  elif docker build -q -t "${IMG}" . >/dev/null 2>&1; then
    if docker run --rm --entrypoint /usr/local/bin/journal-retention "${IMG}" --help >/dev/null 2>&1; then
      pass "D1-deep journal-retention РЕАЛЬНО запускается из прод-образа"
    else
      fail "D1-deep бинаря нет в образе или он не запускается (Dockerfile обещал — образ не дал)"
    fi
    if docker run --rm --entrypoint /bin/sh "${IMG}" -c 'test -x /usr/local/bin/recorder'; then
      pass "D2-deep recorder в образе исполняем"
    else
      fail "D2-deep recorder не исполняем в образе"
    fi
  else
    fail "D1-deep прод-образ не собирается"
  fi
else
  echo "SKIP  D1-deep (сборка образа) — включается HFT_DELIVERY_DEEP=1 (CI-job + §8 на VPS)"
fi

# ── D3: холодное хранилище смонтировано в контейнер ретеншена ─────────────────────────
if grep -qE '/cold' docker-compose.yml && grep -qE 'JOURNAL_COLD_DIR|/mnt/journal-cold' docker-compose.yml; then
  pass "D3 docker-compose монтирует холодное хранилище в /cold"
else
  fail "D3 docker-compose НЕ монтирует холодное хранилище — --cold некуда указывать, выгрузка \
невозможна, а без выгрузки prune запрещён (ColdCopyProof) ⇒ диск не освобождается"
fi

# ── D4: сервис ретеншена объявлен (ops-профиль: не поднимается вместе с recorder) ─────
if grep -qE '^\s{2}journal-retention:' docker-compose.yml && grep -q 'profiles:' docker-compose.yml; then
  pass "D4 сервис journal-retention объявлен под ops-профилем"
else
  fail "D4 в docker-compose нет сервиса journal-retention (ops-профиль) — оператору нечем \
запускать уборку без ручного docker exec"
fi

# ── D5: планировщик В РЕПО и он РЕАЛЬНО УСТАНАВЛИВАЕТСЯ ───────────────────────────────
# Прежняя редакция D5 грепала слова (`dry-run`, `ALERT`) — и пропустила cron-файл, который
# cron ОТКАЗЫВАЕТСЯ ставить: команда была разбита переносами `\`, которых cron не понимает
# (каждая физическая строка = отдельная запись ⇒ `crontab -n` → «bad minute», exit=1).
# Поймал reviewer на PR-гейте. Тот же класс, что весь TD-020: грепом доказывается ТЕКСТ,
# а нужен ФАКТ. Поэтому ниже — парсер cron'а, а не grep, и прогон тела задания, а не чтение.
CRON="deploy/cron.d/journal-retention"
CRON_JOB="deploy/bin/journal-retention-cron.sh"

if [ ! -f "${CRON}" ]; then
  fail "D5 нет ${CRON} — планировщик существует только в голове оператора; ровно так TD-020 \
и родился (артефакт, которого нет в репо, не существует)"
elif [ ! -f "${CRON_JOB}" ]; then
  fail "D5 нет ${CRON_JOB} — cron ссылается на скрипт, которого нет в репо"
else
  d5=0

  # (1) УСТАНАВЛИВАЕМОСТЬ — то, чего не было. Валидируем ТЕМ ЖЕ парсером, что и cron.
  if command -v crontab >/dev/null 2>&1; then
    if ! crontab -n "${CRON}" >/dev/null 2>&1; then
      d5=1
      fail "D5 ${CRON} НЕ УСТАНАВЛИВАЕТСЯ (crontab -n → exit≠0):"
      crontab -n "${CRON}" 2>&1 | sed 's/^/      /'
      echo "      Частая причина: перенос команды через '\\' — cron этого НЕ поддерживает."
    fi
  else
    # Нет crontab в окружении — не молчим и не зачитываем как PASS: проверяем структурно.
    echo "NOTE  D5 crontab(1) недоступен — синтаксис проверяется структурно (в CI/на VPS он есть)"
  fi

  # (2) Переносов строк в записях быть не может — cron их не понимает (структурный дубль (1),
  #     работает и там, где crontab(1) не установлен).
  if grep -qE '\\[[:space:]]*$' "${CRON}"; then
    d5=1
    fail "D5 ${CRON} содержит перенос строки '\\' — cron трактует продолжение как НОВУЮ запись \
(«bad minute») и файл не устанавливается. Команда обязана быть ОДНОЙ строкой; логика — в ${CRON_JOB}"
  fi

  # (3) Задание вызывает наш скрипт, а не инлайн-простыню.
  if ! grep -qE '^[0-9*/,-]+[[:space:]]+.*journal-retention-cron\.sh[[:space:]]*$' "${CRON}"; then
    d5=1
    fail "D5 в ${CRON} нет записи расписания, вызывающей ${CRON_JOB} одной строкой"
  fi

  # (4) Тело задания синтаксически валидно и исполняемо.
  if ! bash -n "${CRON_JOB}" 2>/dev/null; then
    d5=1
    fail "D5 ${CRON_JOB} не проходит bash -n (синтаксическая ошибка)"
  fi
  [ -x "${CRON_JOB}" ] || { d5=1; fail "D5 ${CRON_JOB} не исполняемый (chmod +x) — cron его не запустит"; }

  # (5) ГЛАВНОЕ: argv задания обязан принимать НАСТОЯЩИЙ бинарь.
  #
  # На проде задание упало именно здесь: скрипт передавал `--dir=/journal`, а парсер бинаря
  # понимает только РАЗДЕЛЬНУЮ форму (`--dir /journal`). Прежняя редакция D5 этого не увидела,
  # потому что подставляла СТАБ `docker`, глотавший любые аргументы — **застабила ровно тот
  # контракт, который и ломался**. Стаб проверяет обвязку (алерт/exit), но НЕ контракт argv.
  # Поэтому теперь: собираем настоящий `journal-retention` и скармливаем ему ТОТ argv, который
  # задание реально отдаёт (единственный источник — сам скрипт, RETENTION_PRINT_ARGV=1).
  # Дрейф между cron-скриптом и парсером после этого невозможен: он валит гейт.
  sandbox=$(mktemp -d)
  mkdir -p "${sandbox}/journal" "${sandbox}/cold" "${sandbox}/root"

  if cargo build -q -p journal --bin journal-retention 2>/dev/null; then
    BIN="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
            | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')/debug/journal-retention"
    [ -x "${BIN}" ] || BIN="target/debug/journal-retention"

    # argv берём ИЗ ЗАДАНИЯ (не переписываем руками — иначе гейт проверял бы свою фантазию).
    mapfile -t job_argv < <(
      RETENTION_PRINT_ARGV=1 \
      RETENTION_JOURNAL_DIR="${sandbox}/journal" \
      JOURNAL_COLD_DIR="${sandbox}/cold" \
      RETENTION_MIN_FREE_GB=0 \
      bash "${CRON_JOB}"
    )

    set +e
    "${BIN}" "${job_argv[@]}" > "${sandbox}/real.out" 2>&1
    rc_real=$?
    set -e
    if [ "${rc_real}" -ne 0 ]; then
      d5=1
      fail "D5 НАСТОЯЩИЙ бинарь ОТВЕРГ argv задания (exit=${rc_real}) — ровно так задание упало \
на проде. argv: ${job_argv[*]}"
      sed 's/^/      /' "${sandbox}/real.out" | head -5
    else
      pass "D5a настоящий journal-retention ПРИНИМАЕТ argv задания (dry-run отработал, exit=0)"
    fi
  else
    d5=1
    fail "D5 не собрался journal-retention — контракт argv проверить нечем (гейт не смеет молчать)"
  fi

  # (6) Обвязка: при ненулевом exit'е задание ОБЯЗАНО поднять алерт и пробросить код.
  #     Здесь стаб уместен — мы проверяем реакцию НА сбой, а не контракт argv (его проверил (5)).
  printf '#!/bin/sh\nexit 3\n' > "${sandbox}/fake-runner"
  chmod +x "${sandbox}/fake-runner"
  set +e
  RETENTION_RUNNER="${sandbox}/fake-runner" \
    HFT_ROOT="${sandbox}/root" \
    RETENTION_LOG="${sandbox}/retention.log" \
    RETENTION_ALERT_FILE="${sandbox}/retention.alert" \
    bash "${CRON_JOB}" >/dev/null 2>&1
  rc_stub=$?
  set -e
  if [ "${rc_stub}" -ne 3 ]; then
    d5=1
    fail "D5 задание НЕ пробрасывает exit (раннер вернул 3, задание — ${rc_stub}); \
cron/монитор не узнают о disk_pressure"
  fi
  if [ ! -s "${sandbox}/retention.alert" ]; then
    d5=1
    fail "D5 задание НЕ подняло маркер алерта на exit≠0 — сбой ретеншена остался бы НЕЗАМЕЧЕННЫМ \
(2 = сверка холодной копии не прошла, 3 = disk_pressure)"
  fi
  rm -rf "${sandbox}"

  [ "${d5}" -eq 0 ] && pass "D5 cron-юнит РЕАЛЬНО устанавливается (crontab -n), задание алертит и \
пробрасывает exit≠0 (проверено прогоном со стабом, не грепом)"
fi

# ── D6: runbook доставки (кто монтирует Storage Box и как включается Apply) ───────────
if [ -f deploy/README.md ] && grep -qi 'storage box\|/mnt/journal-cold' deploy/README.md; then
  pass "D6 deploy/README описывает монтирование холодного хранилища и включение Apply"
else
  fail "D6 нет deploy/README.md с процедурой (монтирование Storage Box, первый dry-run, \
переход на Apply) — ретеншен без оператора = TD-020"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "DELIVERY: FAIL (${FAILED})"
  exit 1
fi
echo "DELIVERY: PASS"
