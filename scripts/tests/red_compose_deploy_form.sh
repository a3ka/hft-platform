#!/usr/bin/env bash
# red_compose_deploy_form.sh — ДВУСТОРОННЯЯ ПРОБА шага D10 (scripts/lib/compose_deploy_form.sh;
# M-92, Deploy 37242446986, R-241 Н-2: «несущие флаги шага не пиннит ничто»).
#
# Каждый сценарий — отдельный каталог проекта с docker-compose.yml. Каждое свойство проверки
# пиннится своим сценарием: очищенное окружение (C5), игнор `.env` каталога (C4), чтение
# override как у деплоя (C6), две переменные прод-`.env` (C7), все профили и сверка источника
# (C1/C3), интерполяция всего файла (C8). Страж setup'а у каждого сценария.
# COMPOSE_DEPLOY_FORM_LIB — путь к проверяемой библиотеке (для мутантов).

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LIB="${COMPOSE_DEPLOY_FORM_LIB:-${ROOT}/scripts/lib/compose_deploy_form.sh}"
N=0; BAD=0
ok()  { N=$((N+1)); printf 'ok    %s\n' "$1"; }
bad() { N=$((N+1)); BAD=$((BAD+1)); printf 'FAIL  %s\n' "$1"; }

if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
  echo "FAIL  docker compose недоступен — пробу гонять нечем"; exit 1
fi
[ -f "${LIB}" ] || { echo "FAIL  нет библиотеки ${LIB}"; exit 1; }
# shellcheck source=../lib/compose_deploy_form.sh
source "${LIB}"

TMPROOT="${TMPDIR:-/tmp}"
tmp_before=$(find "$TMPROOT" -maxdepth 1 -name 'tmp.*' -type f 2>/dev/null | wc -l)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT INT TERM
WANT=/var/lib/hft/retention-work

# proj <имя> <строка тома /work> — каталог проекта; дополнительные файлы кладёт сценарий
proj() {
  local d="${TMP}/$1"; mkdir -p "$d"
  cat > "$d/docker-compose.yml" <<EOF
services:
  recorder:
    image: busybox
    environment:
      GATEWAY_JWT_SECRET: \${GATEWAY_JWT_SECRET:?GATEWAY_JWT_SECRET must be set}
      GATEWAY_BANDS: \${GATEWAY_BANDS:?GATEWAY_BANDS must be set}
  decoy:
    image: busybox
    volumes:
      - "/tmp/decoy:/work"
  journal-retention:
    image: busybox
    profiles: ["ops"]
    volumes:
      - "$2:/work"
EOF
  printf '%s' "$d"
}

expect() { # <pass|fail> <имя> <dir> <подстрока причины|""> [ИМЯ=значение — экспорт на время вызова...]
  local want="$1" name="$2" d="$3" why="${4:-}" rc out
  shift 4 || shift $#
  out=$(for kv in "$@"; do export "$kv"; done; compose_deploy_form_check "$d" "${WANT_OVERRIDE:-$WANT}" 2>&1); rc=$?
  if [ "$want" = fail ] && [ "$rc" -ne 0 ] && [ -n "$why" ] && ! printf '%s' "$out" | grep -qF -- "$why"; then
    bad "$name — отказ есть, но не по причине «$why»"; printf '%s\n' "$out" | sed 's/^/      /'; return
  fi
  if { [ "$want" = pass ] && [ "$rc" -eq 0 ]; } || { [ "$want" = fail ] && [ "$rc" -ne 0 ]; }; then
    ok "$name (rc=$rc)"
  else
    bad "$name — ожидалось ${want}, rc=$rc"; printf '%s\n' "$out" | sed 's/^/      /'
  fi
}

# C1 позитив: умолчание верное; служба под профилем видна, источник сверен — PASS
d=$(proj c1 '${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}')
grep -q 'profiles: \["ops"\]' "$d/docker-compose.yml" || bad "C1 SETUP НЕ СОСТОЯЛСЯ"
expect pass "C1 верное умолчание, служба под профилем" "$d" ""

# C2 умолчания нет (прод-дефект 086069b) — FAIL на разборе
d=$(proj c2 '${RETENTION_WORK_DIR}')
grep -qF '${RETENTION_WORK_DIR}:/work' "$d/docker-compose.yml" || bad "C2 SETUP НЕ СОСТОЯЛСЯ"
expect fail "C2 подстановка без умолчания" "$d" "empty section between colons"

# C3 неверное умолчание — FAIL на сверке источника
d=$(proj c3 '${RETENTION_WORK_DIR:-/tmp/wrong}')
expect fail "C3 неверное умолчание" "$d" "ожидался"

# C4 умолчания нет, но .env каталога задаёт переменную — .env НЕ читается — FAIL
d=$(proj c4 '${RETENTION_WORK_DIR}')
printf 'RETENTION_WORK_DIR=/var/lib/hft/retention-work\n' > "$d/.env"
[ -f "$d/.env" ] || bad "C4 SETUP НЕ СОСТОЯЛСЯ"
expect fail "C4 .env каталога не маскирует дефект" "$d" "empty section between colons"

# C5 умолчания нет, но окружение вызывающего задаёт переменную — окружение очищено — FAIL
d=$(proj c5 '${RETENTION_WORK_DIR}')
expect fail "C5 окружение оператора не маскирует дефект" "$d" "empty section between colons" \
  RETENTION_WORK_DIR=/var/lib/hft/retention-work

# C6 верное умолчание, но override рядом с подстановкой без умолчания — деплой его читает — FAIL
d=$(proj c6 '${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}')
cat > "$d/docker-compose.override.yml" <<'EOF'
services:
  recorder:
    volumes:
      - "${OVR_NO_DEFAULT}:/ovr"
EOF
grep -qF '${OVR_NO_DEFAULT}' "$d/docker-compose.override.yml" || bad "C6 SETUP НЕ СОСТОЯЛСЯ"
expect fail "C6 override каталога читается, как у деплоя" "$d" "OVR_NO_DEFAULT"

# C7 обязательная переменная прод-.env (GATEWAY_JWT_SECRET :?) — две переменные заданы — PASS
d=$(proj c7 '${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}')
grep -qF 'GATEWAY_JWT_SECRET:?' "$d/docker-compose.yml" || bad "C7 SETUP НЕ СОСТОЯЛСЯ"
expect pass "C7 обязательная переменная прод-.env задана" "$d" ""

# C8 подстановка без умолчания в ДРУГОМ, не поднимаемом сервисе — compose судит весь файл — FAIL
d=$(proj c8 '${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}')
cat >> "$d/docker-compose.yml" <<'EOF'
  other:
    image: busybox
    profiles: ["never"]
    volumes:
      - "${OTHER_NO_DEFAULT}:/x"
EOF
grep -q OTHER_NO_DEFAULT "$d/docker-compose.yml" || bad "C8 SETUP НЕ СОСТОЯЛСЯ"
expect fail "C8 дефект в другом сервисе роняет весь разбор" "$d" "OTHER_NO_DEFAULT"

# C10 проверка идёт ЧЕРЕЗ docker compose (R-243 Н-1): шим, у которого `compose config` падает — FAIL
d=$(proj c10 '${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}')
mkdir -p "${TMP}/shim"
cat > "${TMP}/shim/docker" <<'EOF2'
#!/bin/sh
[ "$1 $2" = "compose version" ] && { echo "Docker Compose version shim"; exit 0; }
echo "shim: compose config отказ" >&2; exit 17
EOF2
chmod +x "${TMP}/shim/docker"
[ -x "${TMP}/shim/docker" ] || bad "C10 SETUP НЕ СОСТОЯЛСЯ"
expect fail "C10 разбор делает настоящий docker compose" "$d" "shim: compose config отказ" PATH="${TMP}/shim:${PATH}"

# C11 ожидаемый источник — параметр, а не константа (R-243 Н-2) — PASS
d=$(proj c11 '${RETENTION_WORK_DIR:-/tmp/other-work}')
WANT_OVERRIDE=/tmp/other-work expect pass "C11 иное ожидание источника" "$d" ""

# C9 настоящий docker-compose.yml репозитория — PASS (прод-форма, как в D10)
expect pass "C9 настоящий docker-compose.yml" "${ROOT}" ""

rm -rf "$TMP"; trap - EXIT INT TERM
left=0; [ -e "$TMP" ] && left=1
tmp_after=$(find "$TMPROOT" -maxdepth 1 -name 'tmp.*' -type f 2>/dev/null | wc -l)
[ "$tmp_after" -le "$tmp_before" ] || { left=$((left + tmp_after - tmp_before)); echo "FAIL  библиотека оставила временных файлов: $((tmp_after - tmp_before))"; }
echo
echo "сценариев: ${N}, провалов: ${BAD}, каталогов фикстур после прогона: ${left}"
[ "$left" -eq 0 ] || BAD=$((BAD+1))
if [ "${BAD}" -gt 0 ]; then echo "VERDICT: FAIL"; exit 1; fi
echo "VERDICT: PASS"
