#!/usr/bin/env bash
# RED M-93 — сторож `ops-watchdog` ДОСТАВЛЯЕТСЯ деплоем, а не собирается руками (`TD-231`).
#
# Замер, из-за которого проба существует (2026-10-03, ssh на VPS): cron `*/5` зовёт
# `/root/hft-platform/target/release/ops-watchdog` — бинарь, собранный ВРУЧНУЮ 2026-10-02 в
# `rust:1-slim`; в образе его нет (`Dockerfile:18` — пять бинарей), деплой его не ставит. Он
# работает («норма»), но переустановку сервера не переживёт и с кодом не обновляется: правка
# `crates/ops` уезжает в main и в прод НЕ попадает.
#
# Конструкция (спека M-93 §3): бинарь собирается В ОБРАЗЕ; деплой после healthy достаёт его из
# образа РАБОТАЮЩЕГО контейнера и атомарно кладёт на хост в постоянный путь; cron зовёт этот путь.
# Сторож остаётся ХОСТОВЫМ процессом намеренно: живущий в docker сторож не сообщит о смерти docker.
#
# Сценарии зовут скрипты ТЕМ ЖЕ способом, что прод (cron/деплой), с заглушкой `docker` в PATH.
# Число сценариев НЕ заявляется — считается и печатается.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALL="$ROOT/deploy/bin/install-watchdog.sh"
CRON="$ROOT/scripts/watchdog_cron.sh"
PASS=0; FAIL=0
ok()   { printf 'pass  %s\n' "$*"; PASS=$((PASS+1)); }
nope() { printf 'FAIL  %s\n' "$*"; FAIL=$((FAIL+1)); }
SANDBOX="$(mktemp -d)"; trap 'rm -rf "$SANDBOX"' EXIT

# ── заглушка docker: журнал вызовов + управляемые отказы ─────────────────────────────
# STUB_FAIL ∈ {"", inspect, create, cp, empty}; STUB_BIN — файл, который «лежит в образе».
mk_stub() {
  local d="$1"; mkdir -p "$d/bin"
  cat > "$d/bin/docker" <<'EOF'
#!/usr/bin/env bash
echo "$*" >> "$STUB_LOG"
case "$1" in
  inspect) [ "${STUB_FAIL:-}" = inspect ] && { echo "Error: No such object" >&2; exit 1; }
           echo "sha256:img0123" ;;
  create)  [ "${STUB_FAIL:-}" = create ] && exit 1; echo "cid0123" ;;
  cp)      [ "${STUB_FAIL:-}" = cp ] && { echo "Error: cp failed" >&2; exit 1; }
           src="$2"; dst="$3"
           [ "$src" = "cid0123:/usr/local/bin/ops-watchdog" ] || { echo "stub: неожиданный источник $src" >&2; exit 3; }
           if [ "${STUB_FAIL:-}" = empty ]; then : > "$dst"; else cp "$STUB_BIN" "$dst"; fi ;;
  rm)      : ;;
  *)       echo "stub: неожиданная команда $1" >&2; exit 4 ;;
esac
EOF
  chmod +x "$d/bin/docker"
  printf '#!/bin/sh\necho new-watchdog\n' > "$d/fixture-bin"
}

# Прогон установки в песочнице: печатает exit; DST заранее несёт «старый» бинарь.
run_install() { # <каталог> <режим отказа>
  local d="$1" mode="$2"
  mkdir -p "$d/dst"; printf 'OLD\n' > "$d/dst/ops-watchdog"; chmod 0755 "$d/dst/ops-watchdog"
  : > "$d/log"
  ( PATH="$d/bin:$PATH" STUB_LOG="$d/log" STUB_FAIL="$mode" STUB_BIN="$d/fixture-bin" \
    HFT_WATCHDOG_DST="$d/dst/ops-watchdog" bash "$INSTALL" >"$d/out" 2>&1; echo $? )
}

[ -f "$INSTALL" ] || nope "SETUP: deploy/bin/install-watchdog.sh отсутствует — доставки нет (TD-231)"

# w1 — КОМПОЗИЦИЯ: куда кладёт деплой == откуда зовёт cron; путь абсолютный и НЕ рабочий каталог сборки.
dst="$( env -u HFT_WATCHDOG_DST HFT_INSTALL_WATCHDOG_PRINT_DST=1 bash "$INSTALL" 2>/dev/null )"
bin="$( env -u WATCHDOG_BIN WATCHDOG_PRINT_BIN=1 bash "$CRON" 2>/dev/null )"
if [ -n "$dst" ] && [ "$dst" = "$bin" ] && [ "${dst#/}" != "$dst" ] && [ "${dst#*/target/}" = "$dst" ]; then
  ok "w1 композиция: деплой кладёт и cron зовёт один путь ($dst), вне target/"
else
  nope "w1 композиция: деплой кладёт «$dst», cron зовёт «$bin» (обязаны совпасть, абсолютный путь вне target/)"
fi

# w2 — счастливый путь: бинарь из образа РАБОТАЮЩЕГО контейнера hft-recorder, атомарно.
d="$SANDBOX/w2"; mk_stub "$d"
if [ -f "$INSTALL" ]; then rc="$(run_install "$d" "")"; else rc=99; fi
order="$(awk '{print $1}' "$d/log" 2>/dev/null | tr '\n' ' ')"
if [ "$rc" = 0 ] && cmp -s "$d/dst/ops-watchdog" "$d/fixture-bin" && [ -x "$d/dst/ops-watchdog" ] \
   && [ "$(ls -A "$d/dst" | wc -l)" -eq 1 ] && grep -q '^inspect .*hft-recorder' "$d/log" \
   && grep -q '^create sha256:img0123' "$d/log" && grep -q '^rm .*cid0123' "$d/log"; then
  ok "w2 установка: бинарь из образа hft-recorder, исполняемый, без хвостов ($order)"
else
  nope "w2 установка: exit=$rc, вызовы «$order», в dst: $(ls -A "$d/dst" 2>/dev/null | tr '\n' ' ') — $(head -c 300 "$d/out" 2>/dev/null)"
fi

# w3..w5 — fail-closed: любой отказ ⇒ exit≠0, прежний бинарь цел, временных файлов нет.
for pair in "w3:cp" "w4:inspect" "w5:empty"; do
  w="${pair%%:*}"; m="${pair##*:}"; d="$SANDBOX/$w"; mk_stub "$d"
  if [ -f "$INSTALL" ]; then rc="$(run_install "$d" "$m")"; else rc=99; fi
  extra=""
  [ "$m" = inspect ] && grep -q "^create" "$d/log" 2>/dev/null && extra="create позван после отказа inspect"
  [ "$m" = cp ] && ! grep -q "^rm .*cid0123" "$d/log" 2>/dev/null && extra="контейнер не удалён после отказа cp"
  if [ "$rc" != 0 ] && [ "$rc" != 99 ] && [ "$(cat "$d/dst/ops-watchdog")" = OLD ] \
     && [ "$(ls -A "$d/dst" | wc -l)" -eq 1 ] && [ -z "$extra" ]; then
    ok "$w отказ «$m»: exit=$rc, прежний бинарь цел, хвостов нет"
  else
    nope "$w отказ «$m»: exit=$rc, dst=«$(head -c 40 "$d/dst/ops-watchdog" 2>/dev/null)», файлов $(ls -A "$d/dst" 2>/dev/null | wc -l) ${extra}"
  fi
done

# w6 — образ собирает и кладёт бинарь (глубокая проверка запуском образа — verify_delivery_M-08.sh D1-deep).
if grep -qE -- '--bin[[:space:]]+ops-watchdog' "$ROOT/Dockerfile" \
   && grep -qE '^COPY --from=builder /build/target/release/ops-watchdog /usr/local/bin/ops-watchdog$' "$ROOT/Dockerfile"; then
  ok "w6 Dockerfile: ops-watchdog собирается и копируется в /usr/local/bin"
else
  nope "w6 Dockerfile: нет --bin ops-watchdog и/или COPY в /usr/local/bin/ops-watchdog"
fi

# w7 — деплой зовёт установку на ОБЕИХ ветках: после healthy и после отката (бинарь = образ, который
# реально крутится). Предел назван: deploy.yml исполняется только на VPS — здесь проверка по тексту
# ветвей, прод-исход предъявляет §8.
DY="$ROOT/.github/workflows/deploy.yml"
ok_branch="$(awk '/healthy \(recorder \+ gateway-serve\)/{f=1} f&&/^ *else$/{exit} f' "$DY" | grep -c 'deploy/bin/install-watchdog.sh')"
rb_branch="$(awk '/rollback to/{f=1} f&&/^ *fi$/{exit} f' "$DY" | grep -c 'deploy/bin/install-watchdog.sh')"
if [ "$ok_branch" -ge 1 ] && [ "$rb_branch" -ge 1 ]; then
  ok "w7 deploy.yml: установка сторожа на ветке healthy и на ветке отката"
else
  nope "w7 deploy.yml: установка сторожа — ветка healthy: $ok_branch, ветка отката: $rb_branch (нужно ≥1 на обеих)"
fi

# w8 — cron без бинаря по новому пути по-прежнему КРИЧИТ (fail-closed обёртки не ослаблен).
d="$SANDBOX/w8"; mkdir -p "$d"
rc="$( WATCHDOG_BIN="$d/absent" WATCHDOG_LOG="$d/log" WATCHDOG_ALERT_FILE="$d/alert" \
       WATCHDOG_LAST_SUCCESS="$d/ok" bash "$CRON" >/dev/null 2>&1; echo $? )"
if [ "$rc" != 0 ] && grep -q 'ALERT' "$d/log" 2>/dev/null && [ -s "$d/alert" ]; then
  ok "w8 cron без бинаря: exit=$rc, ALERT в логе и файле тревоги"
else
  nope "w8 cron без бинаря: exit=$rc — тревоги нет"
fi

echo "сценариев: $((PASS+FAIL))   pass=$PASS   FAIL=$FAIL"
[ "$FAIL" -eq 0 ] && { echo "VERDICT: PASS"; exit 0; } || { echo "VERDICT: FAIL"; exit 1; }
