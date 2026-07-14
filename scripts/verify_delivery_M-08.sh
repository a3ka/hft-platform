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

# ── D5: планировщик В РЕПО (а не «настроим на VPS руками») ────────────────────────────
CRON="deploy/cron.d/journal-retention"
if [ -f "${CRON}" ]; then
  if grep -q 'dry-run' "${CRON}" && grep -qE 'logger|ALERT|retention\.alert' "${CRON}"; then
    pass "D5 cron-юнит в репо: dry-run по расписанию + алерт на ненулевой exit"
  else
    fail "D5 ${CRON} есть, но не запускает dry-run и/или не алертит на exit≠0 \
(2 = сверка холодной копии не прошла, 3 = disk_pressure — молчать про них нельзя)"
  fi
else
  fail "D5 нет ${CRON} — планировщик существует только в голове оператора; ровно так TD-020 \
и родился (артефакт, которого нет в репо, не существует)"
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
