#!/usr/bin/env bash
# RED M-94 (C-280 R3) — деплой ИСПОЛНЯЕТСЯ, а не читается: отказ гейта профиля возвращает чекаут,
# образ и cron к PREV, выдача не перезапускается; отказ здоровья — тоже откат к PREV, включая cron.
# Спека: milestones/M-94-calc-profile.md §3.7, инвариант I-6, оракулы a0…a4, a7.
#
# Контракт вызова (спека §3.7): `deploy.yml` после `git reset --hard "$TARGET_SHA"` зовёт
# `bash deploy/bin/deploy-apply.sh "$PREV"` из каталога чекаута. Швы:
#   DEPLOY_DOCKER           вместо `docker`                (дефолт docker)
#   DEPLOY_GATE             вместо гейта профиля           (дефолт bash deploy/bin/calc-profile-gate.sh)
#   DEPLOY_CRON_DIR         куда ставится cron             (дефолт /etc/cron.d)
#   DEPLOY_SUDO             префикс установки              (дефолт sudo; пусто — без него)
#   DEPLOY_CRON_VALIDATE    проверка файла cron            (дефолт crontab -n)
#   DEPLOY_WATCHDOG_INSTALL установка сторожа              (дефолт bash deploy/bin/install-watchdog.sh)
#   DEPLOY_HEALTH_TIMEOUT   секунды ожидания healthy       (дефолт 300)
#
# Песочница — НАСТОЯЩИЙ git-репозиторий с двумя коммитами. PREV несёт свой cron и профиль v1 и
# НЕ несёт deploy-apply.sh; TARGET — cron и профиль v2 и скрипт под пробой. Значит откат
# `reset --hard PREV` УДАЛЯЕТ исполняемый скрипт из чекаута посреди прогона — a1 требует, чтобы
# откат при этом ДОРАБОТАЛ (пересборка на PREV видна в логе). Замер при построении пробы: git
# удаляет файл через unlink, bash дочитывает открытый дескриптор — отдельного «исполнения из
# копии» это НЕ требует (мутант «без копии» на эталоне зелен), и проба его не навязывает.
# Заглушка docker пишет `<команда>@<HEAD в момент вызова>` — так видно, на какой ревизии собран
# образ и поднята выдача, без доверия к тексту скрипта.
#
# Число сценариев НЕ заявляется — считается и печатается.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# M94_APPLY_UNDER_TEST — только для мутационной проверки САМОЙ пробы (эталон и мутанты).
APPLY="${M94_APPLY_UNDER_TEST:-$ROOT/deploy/bin/deploy-apply.sh}"
DEPLOY="${M94_DEPLOY_YML_UNDER_TEST:-$ROOT/.github/workflows/deploy.yml}"
PASS=0; FAIL=0
ok()   { printf 'pass  %s\n' "$*"; PASS=$((PASS+1)); }
nope() { printf 'FAIL  %s\n' "$*"; FAIL=$((FAIL+1)); }
SANDBOX="$(mktemp -d)"; trap 'rm -rf "$SANDBOX"' EXIT

if [ ! -f "$APPLY" ]; then
  nope "a0 SETUP: $APPLY нет — тело деплоя не исполнимо пробой, откат при отказе гейта не доказуем (I-6, C-280 R3)"
  printf 'VERDICT: FAIL (pass=%d fail=%d; миры не исполнялись — судить нечего)\n' "$PASS" "$FAIL"
  exit 1
fi
ok "a0 скрипт деплоя существует"

# ── мир: <имя> <код гейта> <здоровье recorder> <здоровье gateway-serve> [код сторожа] [cron невалиден]
# Здоровье — ПО СЛУЖБЕ (`C-281` B1) и ПОСЛЕДОВАТЕЛЬНОСТЬЮ состояний (`A-049` Р-2): прод после
# `up -d` отвечает `starting` (healthcheck `start_period`), и константный ответ не отличал «ждёт
# healthy» от «принимает всё, кроме unhealthy». Язык спецификации здоровья службы:
#   healthy | unhealthy | starting (навсегда) | starting:K,healthy (K раз starting, затем healthy)
#   | missing (контейнера нет: `inspect` выходит ≠ 0)
mk_world() {
  local w="$SANDBOX/$1" gate_rc="$2" h_rec="$3" h_srv="$4" wd_rc="${5:-0}" cron_bad="${6:-0}"
  mkdir -p "$w/repo" "$w/cron" "$w/bin" || return 1
  (
    cd "$w/repo" || exit 1
    git init -q -b main . && git config user.email p@x && git config user.name probe || exit 1
    mkdir -p deploy/cron.d deploy/bin config/calc-profile
    printf '# PREV\n*/15 * * * * root echo prev\n' > deploy/cron.d/journal-retention
    printf 'CALC_PROFILE_VERSION=1\n' > config/calc-profile/active.env
    git add -A && git commit -qm prev || exit 1
    printf '# TARGET\n*/15 * * * * root echo target\n' > deploy/cron.d/journal-retention
    [ "$cron_bad" = 1 ] && printf 'INVALID-CRON-MARKER\n' >> deploy/cron.d/journal-retention
    printf 'CALC_PROFILE_VERSION=2\n' > config/calc-profile/active.env
    cp "$APPLY" deploy/bin/deploy-apply.sh
    git add -A && git commit -qm target || exit 1
  ) || return 1
  # cron хоста до деплоя — ставлен прошлым деплоем, т.е. из PREV
  printf '# PREV\n*/15 * * * * root echo prev\n' > "$w/cron/hft-journal-retention"
  printf 'W=%q\nH_REC=%q\nH_SRV=%q\n' "$w" "$h_rec" "$h_srv" > "$w/stub.env"
  cat > "$w/bin/docker" <<'STUB'
#!/usr/bin/env bash
. "$(dirname "$0")/../stub.env"
h=$(git rev-parse --short HEAD 2>/dev/null || echo nohead)
last=""; for a in "$@"; do case "$a" in hft-*) last="$a" ;; esac; done  # имя контейнера, где бы оно ни стояло
answer() { # <спецификация здоровья> <служба> — состояние на ЭТОТ вызов
  local spec="$1" svc="$2" n k
  n=$(( $(cat "$W/cnt.$svc" 2>/dev/null || echo 0) + 1 )); echo "$n" > "$W/cnt.$svc"
  case "$spec" in
    missing)    echo "Error: No such object: $svc" >&2; exit 1 ;;
    starting:*) k="${spec#starting:}"; k="${k%%,*}"
                if [ "$n" -le "$k" ]; then echo starting; else echo "${spec##*,}"; fi ;;
    *)          echo "$spec" ;;
  esac
}
case "$1" in
  compose)
    case "$2" in
      build) echo "build@$h" >> "$W/docker.log" ;;
      up)    echo "up@$h" >> "$W/docker.log" ;;
      *)     echo "other:$*@$h" >> "$W/docker.log" ;;
    esac ;;
  inspect)
    echo "inspect:$last" >> "$W/inspect.log"
    case "$last" in
      hft-recorder)      answer "$H_REC" "$last" ;;
      hft-gateway-serve) answer "$H_SRV" "$last" ;;
      *)                 echo "Error: No such object" >&2; exit 1 ;;
    esac ;;
  logs)  echo "logs:$last@$h" >> "$W/docker.log" ;;
  image) [ "$2" = prune ] && echo "prune@$h" >> "$W/docker.log" ;;
  *)     echo "other:$*@$h" >> "$W/docker.log" ;;
esac
exit 0
STUB
  # валидатор cron: отказывает на файле-маркере (`A-049` Р-3) — то, что `crontab -n` делает на проде
  printf '#!/usr/bin/env bash\nif grep -q INVALID-CRON-MARKER "$1"; then echo "invalid cron: $1" >&2; exit 1; fi\nexit 0\n' > "$w/bin/cronval"
  cat > "$w/bin/gate" <<EOF
#!/usr/bin/env bash
echo "gate@\$(git rev-parse --short HEAD)" >> "$w/docker.log"
exit $gate_rc
EOF
  printf '#!/usr/bin/env bash\necho "watchdog@$(git rev-parse --short HEAD)" >> "%s/docker.log"\nexit %s\n' "$w" "$wd_rc" > "$w/bin/watchdog"
  chmod +x "$w/bin/docker" "$w/bin/gate" "$w/bin/watchdog" "$w/bin/cronval" || return 1
  # страж подготовки: две ревизии, на TARGET есть скрипт, на PREV его нет
  [ "$(git -C "$w/repo" rev-list --count HEAD)" = 2 ] \
    && git -C "$w/repo" cat-file -e HEAD:deploy/bin/deploy-apply.sh \
    && ! git -C "$w/repo" cat-file -e HEAD~1:deploy/bin/deploy-apply.sh 2>/dev/null \
    && echo "$w"
}

run_apply() { # <каталог мира> → RC, OUT
  local w="$1" prev
  prev=$(git -C "$w/repo" rev-parse HEAD~1)
  OUT=$(cd "$w/repo" && DEPLOY_DOCKER="$w/bin/docker" DEPLOY_GATE="$w/bin/gate" \
        DEPLOY_CRON_DIR="$w/cron" DEPLOY_SUDO="" DEPLOY_CRON_VALIDATE="$w/bin/cronval" \
        DEPLOY_WATCHDOG_INSTALL="$w/bin/watchdog" DEPLOY_HEALTH_TIMEOUT=5 \
        timeout 60 bash deploy/bin/deploy-apply.sh "$prev" 2>&1); RC=$?
  PREV_S=$(git -C "$w/repo" rev-parse --short HEAD~1 2>/dev/null)
  [ "$(git -C "$w/repo" rev-parse HEAD)" = "$prev" ] && PREV_S=$(git -C "$w/repo" rev-parse --short "$prev")
  TARGET_S=$(git -C "$w/repo" rev-parse --short main)
  HEAD_S=$(git -C "$w/repo" rev-parse --short HEAD)
  PREVFULL="$prev"
}

# ── a1: гейт профиля ОТКАЗЫВАЕТ ⇒ откат чекаута, образ пересобран на PREV, cron не тронут, up нет ──
if w=$(mk_world a1 1 healthy healthy); then
  run_apply "$w"
  prev_s=$(git -C "$w/repo" rev-parse --short "$PREVFULL")
  last_build=$(grep '^build@' "$w/docker.log" 2>/dev/null | tail -1)
  reasons=""
  [ "$RC" -ne 0 ] || reasons="$reasons; exit=0 при отказе гейта"
  [ "$(git -C "$w/repo" rev-parse HEAD)" = "$PREVFULL" ] || reasons="$reasons; чекаут НЕ на PREV (HEAD=$HEAD_S)"
  [ "$last_build" = "build@$prev_s" ] || reasons="$reasons; последняя сборка образа «$last_build», а не на PREV ($prev_s) — тег образа остался за новым кодом, cron прогреет новый профиль"
  grep -q '^up@' "$w/docker.log" 2>/dev/null && reasons="$reasons; выдача перезапущена (up) при отказе гейта"
  grep -q 'echo prev' "$w/cron/hft-journal-retention" && ! grep -q 'echo target' "$w/cron/hft-journal-retention" \
    || reasons="$reasons; cron хоста несёт TARGET после отказа"
  grep -q 'CALC_PROFILE_VERSION=1' "$w/repo/config/calc-profile/active.env" 2>/dev/null \
    || reasons="$reasons; профиль в чекауте не PREV"
  grep -q '^gate@' "$w/docker.log" || reasons="$reasons; гейт не звался"
  if [ -z "$reasons" ]; then ok "a1 отказ гейта ⇒ PREV: чекаут, образ (build@$prev_s), cron, профиль; up не было"
  else nope "a1 отказ гейта:${reasons} | лог: $(tr '\n' ' ' < "$w/docker.log" 2>/dev/null) | вывод: ${OUT:0:160}"; fi
else nope "a1 SETUP не состоялся"; fi

# ── a2: гейт пропускает, выдача здорова ⇒ TARGET; порядок build → gate → cron → up; сторож ──
if w=$(mk_world a2 0 healthy healthy); then
  run_apply "$w"
  t=$(git -C "$w/repo" rev-parse --short HEAD)
  seq=$(grep -E '^(build|gate|up|watchdog)@' "$w/docker.log" 2>/dev/null | tr '\n' ' ')
  reasons=""
  grep -qx 'inspect:hft-recorder' "$w/inspect.log" 2>/dev/null || reasons="$reasons; здоровье hft-recorder не спрашивалось"
  grep -qx 'inspect:hft-gateway-serve' "$w/inspect.log" 2>/dev/null || reasons="$reasons; здоровье hft-gateway-serve не спрашивалось"
  pl=$(grep -n "^prune@$t" "$w/docker.log" 2>/dev/null | tail -1 | cut -d: -f1)
  ul=$(grep -n "^up@$t" "$w/docker.log" 2>/dev/null | tail -1 | cut -d: -f1)
  { [ -n "$pl" ] && [ -n "$ul" ] && [ "$pl" -gt "$ul" ]; } || reasons="$reasons; docker image prune после успешного up не выполнен (prune строка ${pl:-нет}, up строка ${ul:-нет})"
  grep -q '^logs:' "$w/docker.log" 2>/dev/null && reasons="$reasons; логи сняты при УСПЕШНОМ деплое"
  [ "$RC" -eq 0 ] || reasons="$reasons; exit=$RC"
  [ "$(git -C "$w/repo" rev-parse HEAD)" = "$(git -C "$w/repo" rev-parse main)" ] || reasons="$reasons; чекаут не на TARGET"
  [ "$seq" = "build@$t gate@$t up@$t watchdog@$t " ] || reasons="$reasons; порядок «$seq», ожидался build→gate→up→watchdog на $t"
  grep -q 'echo target' "$w/cron/hft-journal-retention" 2>/dev/null || reasons="$reasons; cron не установлен из TARGET"
  if [ -z "$reasons" ]; then ok "a2 гейт пропустил ⇒ TARGET, build→gate→up→watchdog, здоровье ОБЕИХ служб спрошено, prune выполнен, cron из TARGET"
  else nope "a2 нормальный деплой:${reasons}"; fi
else nope "a2 SETUP не состоялся"; fi

# ── a3: гейт пропускает, cron ставится ПОСЛЕ гейта (не до) ────────────────────────────
# Проверяется отдельным миром с отказом гейта, но cron-файл TARGET отличим: если cron ставится до
# гейта, a1 уже красен. Здесь — независимое свидетельство: время установки cron относительно
# вызова гейта (по mtime файла и записи гейта).
if w=$(mk_world a3 1 healthy healthy); then
  before=$(stat -c %Y "$w/cron/hft-journal-retention")
  sleep 1
  run_apply "$w"
  after=$(stat -c %Y "$w/cron/hft-journal-retention" 2>/dev/null || echo gone)
  if [ "$before" = "$after" ]; then ok "a3 при отказе гейта cron-файл хоста не перезаписывался (mtime цел)"
  else nope "a3 cron-файл хоста ПЕРЕЗАПИСАН при отказе гейта (mtime $before → $after) — установка до гейта"; fi
else nope "a3 SETUP не состоялся"; fi

# ── a4: служба НЕ поднялась здоровой ⇒ откат: PREV, cron из PREV, выдача поднята на PREV, логи ──
# Три мира (`C-281` B1): обе нездоровы; только recorder; только gateway-serve. Деплой, ждущий одну
# службу, проходит ровно тот мир, где нездорова другая, — и краснеет здесь.
health_world() { # <имя> <recorder> <gateway-serve>
  local name="$1" hr="$2" hs="$3" w prev_s last_up reasons
  if w=$(mk_world "$name" 0 "$hr" "$hs"); then
    run_apply "$w"
    prev_s=$(git -C "$w/repo" rev-parse --short "$PREVFULL")
    last_up=$(grep '^up@' "$w/docker.log" 2>/dev/null | tail -1)
    reasons=""
    [ "$RC" -ne 0 ] || reasons="$reasons; exit=0 при нездоровой службе (recorder=$hr, gateway-serve=$hs)"
    [ "$(git -C "$w/repo" rev-parse HEAD)" = "$PREVFULL" ] || reasons="$reasons; чекаут не на PREV"
    [ "$last_up" = "up@$prev_s" ] || reasons="$reasons; последний up «$last_up», а не на PREV"
    # `A-049` Р-1: up без пересборки поднимает ТОТ ЖЕ образ по тегу — сломанный TARGET. Последняя
    # сборка обязана быть на PREV и стоять РАНЬШЕ последнего up.
    local lb lu
    lb=$(grep -n "^build@$prev_s\$" "$w/docker.log" 2>/dev/null | tail -1 | cut -d: -f1)
    lu=$(grep -n "^up@$prev_s\$" "$w/docker.log" 2>/dev/null | tail -1 | cut -d: -f1)
    { [ -n "$lb" ] && [ -n "$lu" ] && [ "$lb" -lt "$lu" ]; } \
      || reasons="$reasons; при откате образ не пересобран на PREV до up (build@PREV строка ${lb:-нет}, up@PREV строка ${lu:-нет})"
    grep -q 'echo prev' "$w/cron/hft-journal-retention" && ! grep -q 'echo target' "$w/cron/hft-journal-retention" \
      || reasons="$reasons; cron хоста остался от TARGET после отката"
    grep -q '^logs:hft-recorder@' "$w/docker.log" 2>/dev/null || reasons="$reasons; при отказе не сняты логи hft-recorder"
    grep -q '^logs:hft-gateway-serve@' "$w/docker.log" 2>/dev/null || reasons="$reasons; при отказе не сняты логи hft-gateway-serve"
    if [ -z "$reasons" ]; then ok "$name нездорова служба (recorder=$hr, gateway-serve=$hs) ⇒ откат к PREV, cron из PREV, up на PREV, логи обеих"
    else nope "$name откат по здоровью:${reasons} | лог: $(tr '\n' ' ' < "$w/docker.log" 2>/dev/null)"; fi
  else nope "$name SETUP не состоялся"; fi
}
health_world a4  unhealthy unhealthy
health_world a4r unhealthy healthy
health_world a4s healthy   unhealthy
# `A-049` Р-2: `starting` навсегда у одной службы ⇒ откат по истечении DEPLOY_HEALTH_TIMEOUT;
# `inspect` падает (контейнера нет) ⇒ откат. Мутант «всё, кроме unhealthy, — здорово» здесь красен.
health_world a4t healthy  starting
health_world a4m missing  healthy

# ── a2s (`A-049` Р-2): starting → healthy у обеих ⇒ деплой ДОЖДАЛСЯ, а не отказал и не проскочил ──
if w=$(mk_world a2s 0 'starting:2,healthy' 'starting:2,healthy'); then
  run_apply "$w"
  nr=$(grep -cx 'inspect:hft-recorder' "$w/inspect.log" 2>/dev/null || true)
  ns=$(grep -cx 'inspect:hft-gateway-serve' "$w/inspect.log" 2>/dev/null || true)
  if [ "$RC" -eq 0 ] && [ "${nr:-0}" -ge 3 ] && [ "${ns:-0}" -ge 3 ] && grep -q '^up@' "$w/docker.log" \
     && [ "$(git -C "$w/repo" rev-parse HEAD)" = "$(git -C "$w/repo" rev-parse main)" ]; then
    ok "a2s starting→healthy ⇒ дождался: exit 0, inspect recorder=$nr serve=$ns (≥3), чекаут TARGET"
  else
    nope "a2s starting→healthy: exit=$RC, inspect recorder=${nr:-0} serve=${ns:-0} (нужно ≥3 — два starting и healthy), HEAD=$(git -C "$w/repo" rev-parse --short HEAD)"
  fi
else nope "a2s SETUP не состоялся"; fi

# ── a6 (`A-049` Р-3): cron TARGET не проходит валидацию ⇒ отказ ДО up, cron хоста цел, откат к PREV ──
# Откат к PREV (чекаут + пересборка образа) — по тому же основанию, что при отказе гейта (`C-280`
# R3): оставленный за TARGET тег образа даст cron'у прогреватель нового кода.
if w=$(mk_world a6 0 healthy healthy 0 1); then
  before=$(stat -c %Y "$w/cron/hft-journal-retention"); sleep 1
  run_apply "$w"
  prev_s=$(git -C "$w/repo" rev-parse --short "$PREVFULL")
  after=$(stat -c %Y "$w/cron/hft-journal-retention" 2>/dev/null || echo gone)
  last_build=$(grep '^build@' "$w/docker.log" 2>/dev/null | tail -1)
  reasons=""
  [ "$RC" -ne 0 ] || reasons="$reasons; exit=0 при невалидном cron"
  grep -q '^up@' "$w/docker.log" 2>/dev/null && reasons="$reasons; выдача перезапущена при невалидном cron"
  [ "$before" = "$after" ] || reasons="$reasons; cron-файл хоста перезаписан"
  grep -q 'INVALID-CRON-MARKER' "$w/cron/hft-journal-retention" 2>/dev/null && reasons="$reasons; невалидный cron установлен"
  [ "$(git -C "$w/repo" rev-parse HEAD)" = "$PREVFULL" ] || reasons="$reasons; чекаут не возвращён к PREV"
  [ "$last_build" = "build@$prev_s" ] || reasons="$reasons; последняя сборка «$last_build», а не на PREV"
  if [ -z "$reasons" ]; then ok "a6 невалидный cron ⇒ отказ до up, cron хоста цел, чекаут и образ — PREV"
  else nope "a6 невалидный cron:${reasons} | лог: $(tr '\n' ' ' < "$w/docker.log" 2>/dev/null)"; fi
else nope "a6 SETUP не состоялся"; fi

# ── a5: установка сторожа ОТКАЗАЛА на здоровом деплое ⇒ деплой красный (контракт deploy.yml M-93) ──
if w=$(mk_world a5 0 healthy healthy 1); then
  run_apply "$w"
  if [ "$RC" -ne 0 ] && grep -q '^watchdog@' "$w/docker.log"; then
    ok "a5 отказ установки сторожа ⇒ деплой красный (exit=$RC)"
  else
    nope "a5 отказ установки сторожа: exit=$RC, сторож звался: $(grep -c '^watchdog@' "$w/docker.log" 2>/dev/null) — тревоги без сторожа нет, деплой обязан быть красным"
  fi
else nope "a5 SETUP не состоялся"; fi

# ── a7: проводка deploy.yml — тело деплоя вызывается скриптом, после reset на TARGET, с PREV ──
reset_l=$(grep -n 'git reset --hard -q "\$TARGET_SHA"' "$DEPLOY" | head -1 | cut -d: -f1)
apply_l=$(grep -n 'deploy/bin/deploy-apply.sh' "$DEPLOY" | head -1 | cut -d: -f1)
inline_up=$(grep -vE '^[[:space:]]*#' "$DEPLOY" | grep -c 'docker compose up' || true)  # комментарии не исполняются
if [ -z "$reset_l" ]; then
  nope "a7 SETUP: в deploy.yml нет reset на TARGET_SHA"
elif [ -z "$apply_l" ]; then
  nope "a7 deploy.yml не зовёт deploy/bin/deploy-apply.sh — тело деплоя не то, что исполняет проба"
elif [ "$reset_l" -lt "$apply_l" ] && grep -n 'deploy/bin/deploy-apply.sh' "$DEPLOY" | head -1 | grep -q 'PREV' \
     && [ "$inline_up" -eq 0 ] && grep -qE '^[[:space:]]+command_timeout:[[:space:]]*15m[[:space:]]*$' "$DEPLOY"; then
  ok "a7 deploy.yml: reset на TARGET → deploy-apply.sh \"\$PREV\"; инлайнового docker compose up нет; command_timeout: 15m"
else
  nope "a7 проводка: reset=$reset_l apply=$apply_l, PREV передан: $(grep 'deploy/bin/deploy-apply.sh' "$DEPLOY" | grep -c PREV), инлайновых 'docker compose up': $inline_up, command_timeout 15m: $(grep -cE '^[[:space:]]+command_timeout:[[:space:]]*15m[[:space:]]*$' "$DEPLOY")"
fi

printf 'сценариев: %d (pass=%d fail=%d)\n' "$((PASS+FAIL))" "$PASS" "$FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL"; exit 1
