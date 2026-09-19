#!/usr/bin/env bash
# Проба барьера `check_retention_composition.sh`. Барьер, не проверенный сам, — намерение.
#
# КАЖДЫЙ сценарий несёт SETUP-GUARD: без него «красное» неотличимо от «проба построила не тот
# файл». На этом проекте вакуумный сценарий уже стоил кругов гейта.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BAR="${ROOT}/scripts/check_retention_composition.sh"
PASSED=0; FAILED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED+1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED+1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 2; }
[ -x "${BAR}" ] || die "барьера нет или он не исполняем: ${BAR}"

BOX="$(mktemp -d /tmp/red-retcomp-XXXXXX)" || die mktemp
trap 'rm -rf "${BOX}"' EXIT

# Обёртка-заглушка: печатает argv так же, как прод-обёртка в режиме HFT_CRON_PRINT_ARGV=1.
mk_cron() { # $1=путь, который «просит» потребитель
  # Имя УНИКАЛЬНО на вызов. Первая редакция брала `$$` (PID пробы) — обе обёртки писались в
  # ОДИН файл, вторая затирала первую, и сценарий `S5` судил бы не то. Поймал setup-guard.
  local f; f="$(mktemp "${BOX}/cron-XXXXXX.sh")"
  printf '#!/usr/bin/env bash\n[ "${HFT_CRON_PRINT_ARGV:-0}" = "1" ] && echo "--dir /journal --checkpoint-coverage=%s"\n' "$1" > "${f}"
  chmod +x "${f}"; printf '%s' "${f}"
}

# Синтетический compose: $1=монтирование производителя, $2=монтирование потребителя,
# $3=объявленный --coverage-out (пусто ⇒ не объявлен)
mk_compose() {
  local f="${BOX}/compose-$RANDOM.yml"
  {
    echo "services:"
    echo "  gateway-checkpoint:"
    echo "    command:"
    [ -n "$3" ] && echo "      - --coverage-out=$3"
    echo "    volumes:"
    echo "      - journal-data:/journal:ro"
    [ -n "$1" ] && echo "      - $1"
    echo "  journal-retention:"
    echo "    command:"
    echo "      - --dir=/journal"
    echo "    volumes:"
    echo "      - journal-data:/journal:ro"
    [ -n "$2" ] && echo "      - $2"
    echo "  recorder:"
    echo "    image: x"
  } > "${f}"
  printf '%s' "${f}"
}

run() { RETENTION_COMPOSE="$1" RETENTION_CRON="$2" bash "${BAR}" >"${BOX}/out" 2>&1; echo $?; }

# ── S1 — ЧЕСТНАЯ конфигурация: барьер обязан быть ЗЕЛЁН ───────────────────────────────
C="$(mk_compose 'gateway-ckpt:/ckpt' 'gateway-ckpt:/ckpt:ro' '/ckpt/covered_through_seq')"
K="$(mk_cron '/ckpt/covered_through_seq')"
grep -q 'gateway-ckpt:/ckpt:ro' "${C}" || die "S1 setup: монтирование потребителя не построено"
RC="$(run "${C}" "${K}")"
if [ "${RC}" -eq 0 ]; then
  pass "S1 честная конфигурация ⇒ барьер ЗЕЛЁН (иначе он запрещал бы верное)"
else
  fail "S1 честная конфигурация ОТВЕРГНУТА (rc=${RC}) — барьер шумит на норме:"$'\n'"$(cat "${BOX}/out")"
fi

# ── S2 — СЕГОДНЯШНИЙ ДЕФЕКТ: у потребителя нет монтирования ───────────────────────────
C="$(mk_compose 'gateway-ckpt:/ckpt' '' '/ckpt/covered_through_seq')"
grep -qc 'gateway-ckpt:/ckpt' "${C}" >/dev/null && ! grep -A6 'journal-retention:' "${C}" | grep -q '/ckpt' \
  || die "S2 setup: у потребителя всё-таки есть /ckpt — судился бы не тот сценарий"
RC="$(run "${C}" "${K}")"
if [ "${RC}" -ne 0 ] && grep -q 'ПОТРЕБИТЕЛЬ НЕ МОНТИРУЕТ' "${BOX}/out"; then
  pass "S2 потребитель без монтирования ПОЙМАН — ровно дефект, найденный на проде 2026-09-08"
else
  fail "S2 потребитель без монтирования ПРОПУЩЕН (rc=${RC}) — барьер не ловит то, ради чего заведён"
fi

# ── S3 — РАЗНЫЕ ТОМА при одинаковом пути: строковая канарейка это пропускала ──────────
C="$(mk_compose 'gateway-ckpt:/ckpt' 'other-vol:/ckpt:ro' '/ckpt/covered_through_seq')"
grep -q 'other-vol:/ckpt' "${C}" || die "S3 setup: второй том не построен"
RC="$(run "${C}" "${K}")"
if [ "${RC}" -ne 0 ] && grep -q 'тома РАЗНЫЕ' "${BOX}/out"; then
  pass "S3 РАЗНЫЕ тома при совпадающем пути ПОЙМАНЫ — то, что сравнение строк не видит"
else
  fail "S3 разные тома ПРОПУЩЕНЫ (rc=${RC}): оба сервиса видят /ckpt, но это РАЗНЫЕ данные"
fi

# ── S4 — производитель не объявляет артефакт вовсе ───────────────────────────────────
C="$(mk_compose 'gateway-ckpt:/ckpt' 'gateway-ckpt:/ckpt:ro' '')"
grep -q 'coverage-out' "${C}" && die "S4 setup: --coverage-out всё-таки объявлен"
RC="$(run "${C}" "${K}")"
if [ "${RC}" -ne 0 ]; then
  pass "S4 отсутствие производителя артефакта ПОЙМАНО"
else
  fail "S4 производителя нет, а барьер ЗЕЛЁН (rc=0) — он проверял бы пустоту"
fi

# ── S5 — пути расходятся ─────────────────────────────────────────────────────────────
C="$(mk_compose 'gateway-ckpt:/ckpt' 'gateway-ckpt:/ckpt:ro' '/ckpt/covered_through_seq')"
K2="$(mk_cron '/ckpt/ДРУГОЙ-ФАЙЛ')"
[ "$(HFT_CRON_PRINT_ARGV=1 bash "${K2}")" != "$(HFT_CRON_PRINT_ARGV=1 bash "${K}")" ] \
  || die "S5 setup: обёртки печатают одно и то же — расхождения нет"
RC="$(run "${C}" "${K2}")"
if [ "${RC}" -ne 0 ]; then
  pass "S5 расхождение путей производителя и потребителя ПОЙМАНО"
else
  fail "S5 пути разошлись, а барьер ЗЕЛЁН (rc=0)"
fi

echo
TOTAL=$((PASSED+FAILED))
if [ "${FAILED}" -eq 0 ]; then echo "VERDICT: PASS (${PASSED}/${TOTAL})"; exit 0; fi
echo "VERDICT: FAIL (${FAILED} из ${TOTAL})"; exit 1
