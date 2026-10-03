#!/usr/bin/env bash
# RED M-93 — сторож `ops-watchdog` ДОСТАВЛЯЕТСЯ деплоем, а не собирается руками (`TD-231`).
#
# Замер, из-за которого проба существует (2026-10-03, ssh на VPS): cron `*/5` зовёт
# `/root/hft-platform/target/release/ops-watchdog` — бинарь, собранный ВРУЧНУЮ 2026-10-02 в
# `rust:1-slim`; в образе его нет (`Dockerfile:18` — пять бинарей), деплой его не ставит. Он
# работает («норма»), но переустановку сервера не переживёт и с кодом не обновляется.
#
# Конструкция (спека M-93 §3): бинарь собирается В ОБРАЗЕ; деплой после healthy достаёт его из
# образа РАБОТАЮЩЕГО контейнера и атомарно кладёт на хост в постоянный путь; cron зовёт этот путь.
#
# Круг 2 (`C-277` B-1): заглушка `docker` отдаёт СЛУЧАЙНЫЙ на каждый прогон образ работающего
# `hft-recorder`; любой другой образ (захардкоженный, `:latest`, чужой контейнер) даёт ЧУЖОЙ бинарь —
# поэтому `w2` доказывает поток «inspect(работающий) → create → cp», а не совпадение литерала.
# Путь назначения по умолчанию проверяется НАСТОЯЩЕЙ записью (`w1b`), а не только веткой печати.
# У каждой песочницы — страж подготовки: PATH выбирает именно заглушку.
#
# Число сценариев НЕ заявляется — считается и печатается.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALL="$ROOT/deploy/bin/install-watchdog.sh"
CRON="$ROOT/scripts/watchdog_cron.sh"
PASS=0; FAIL=0
ok()   { printf 'pass  %s\n' "$*"; PASS=$((PASS+1)); }
nope() { printf 'FAIL  %s\n' "$*"; FAIL=$((FAIL+1)); }
SANDBOX="$(mktemp -d)"; trap 'rm -rf "$SANDBOX"' EXIT

# ── заглушка docker ──────────────────────────────────────────────────────────────────
# Работающий `hft-recorder` несёт образ STUB_IMG (случайный); `inspect` любого иного объекта даёт
# `sha256:foreign`. `create <img>` → контейнер `cid-<img>`. `cp` из контейнера работающего образа
# отдаёт STUB_BIN, из любого другого — FOREIGN_BIN. STUB_FAIL ∈ {"", inspect, cp, empty}.
mk_stub() { # <каталог> → 0 при состоявшейся подготовке
  local d="$1"; mkdir -p "$d/bin" || return 1
  cat > "$d/bin/docker" <<'EOF'
#!/usr/bin/env bash
echo "$*" >> "$STUB_LOG"
case "$1" in
  inspect) [ "${STUB_FAIL:-}" = inspect ] && { echo "Error: No such object" >&2; exit 1; }
           last="${!#}"
           if [ "$last" = hft-recorder ]; then echo "$STUB_IMG"; else echo "sha256:foreign"; fi ;;
  create)  echo "cid-${!#}" ;;
  cp)      [ "${STUB_FAIL:-}" = cp ] && { echo "Error: cp failed" >&2; exit 1; }
           src="$2"; dst="$3"
           case "$src" in
             "cid-${STUB_IMG}:/usr/local/bin/ops-watchdog") body="$STUB_BIN" ;;
             *:/usr/local/bin/ops-watchdog) body="$FOREIGN_BIN" ;;
             *) echo "stub: неожиданный источник $src" >&2; exit 3 ;;
           esac
           if [ "${STUB_FAIL:-}" = empty ]; then : > "$dst"; else cp "$body" "$dst"; fi ;;
  rm)      : ;;
  *)       echo "stub: неожиданная команда $1" >&2; exit 4 ;;
esac
EOF
  chmod +x "$d/bin/docker" || return 1
  printf '#!/bin/sh\necho running-image-watchdog\n' > "$d/fixture-bin"
  printf '#!/bin/sh\necho FOREIGN-watchdog\n' > "$d/foreign-bin"
  # страж подготовки: PATH песочницы выбирает ИМЕННО заглушку
  [ "$(PATH="$d/bin:$PATH" command -v docker)" = "$d/bin/docker" ] && [ -x "$d/bin/docker" ]
}

# Прогон установки: печатает exit (99 — подготовка не состоялась); DST заранее несёт «старый» бинарь.
run_install() { # <каталог> <режим отказа>
  local d="$1" mode="$2"
  mk_stub "$d" || { echo 99; return; }
  mkdir -p "$d/dst"; printf 'OLD\n' > "$d/dst/ops-watchdog"; chmod 0755 "$d/dst/ops-watchdog"
  : > "$d/log"; [ -f "$INSTALL" ] || { echo 98; return; }
  ( PATH="$d/bin:$PATH" STUB_LOG="$d/log" STUB_FAIL="$mode" STUB_IMG="sha256:run$RANDOM$RANDOM" \
    STUB_BIN="$d/fixture-bin" FOREIGN_BIN="$d/foreign-bin" \
    HFT_WATCHDOG_DST="$d/dst/ops-watchdog" bash "$INSTALL" >"$d/out" 2>&1; echo $? )
}

[ -f "$INSTALL" ] || nope "SETUP: deploy/bin/install-watchdog.sh отсутствует — доставки нет (TD-231)"

# w1 — КОМПОЗИЦИЯ (печать): путь, который печатает установка, == путь, который зовёт cron; абсолютный, вне target/.
dst="$( env -u HFT_WATCHDOG_DST HFT_INSTALL_WATCHDOG_PRINT_DST=1 bash "$INSTALL" 2>/dev/null )"
bin="$( env -u WATCHDOG_BIN WATCHDOG_PRINT_BIN=1 bash "$CRON" 2>/dev/null )"
if [ -n "$dst" ] && [ "$dst" = "$bin" ] && [ "${dst#/}" != "$dst" ] && [ "${dst#*/target/}" = "$dst" ]; then
  ok "w1 композиция: установка печатает и cron зовёт один путь ($dst), вне target/"
else
  nope "w1 композиция: установка печатает «$dst», cron зовёт «$bin» (обязаны совпасть, абсолютный путь вне target/)"
fi

# w1b — путь по умолчанию в НАСТОЯЩЕМ режиме (без HFT_WATCHDOG_DST): заглушка `cp` записывает
# аргументы и отказывает — назначение временного файла обязано лежать рядом с путём cron'а и
# называться от него (`<путь>.…`). Запись не происходит: отказ `cp` — штатный путь `w3`.
d="$SANDBOX/w1b"
if mk_stub "$d"; then
  : > "$d/log"
  rc="$( PATH="$d/bin:$PATH" STUB_LOG="$d/log" STUB_FAIL=cp STUB_IMG="sha256:run1b" \
         STUB_BIN="$d/fixture-bin" FOREIGN_BIN="$d/foreign-bin" env -u HFT_WATCHDOG_DST \
         bash "$INSTALL" >"$d/out" 2>&1; echo $? )"
  cpdst="$(awk '$1=="cp"{print $3}' "$d/log" 2>/dev/null | head -1)"
  if [ -n "$bin" ] && [ -n "$cpdst" ] && [ "$(dirname "$cpdst")" = "$(dirname "$bin")" ] \
     && [ "${cpdst#"$bin"}" != "$cpdst" ]; then
    ok "w1b путь по умолчанию в настоящем режиме: временный файл $cpdst рядом с путём cron'а"
  else
    nope "w1b путь по умолчанию в настоящем режиме: cp пишет в «$cpdst», cron зовёт «$bin» (exit=$rc)"
  fi
else nope "w1b SETUP: заглушка docker не выбрана PATH'ом"; fi

# w2 — бинарь ИМЕННО из образа работающего hft-recorder (случайный образ; чужой образ — чужой бинарь).
d="$SANDBOX/w2"; rc="$(run_install "$d" "")"
order="$(awk '{print $1}' "$d/log" 2>/dev/null | tr '\n' ' ')"
img="$(awk '$1=="create"{print $2}' "$d/log" 2>/dev/null | head -1)"
if [ "$rc" = 0 ] && cmp -s "$d/dst/ops-watchdog" "$d/fixture-bin" && [ -x "$d/dst/ops-watchdog" ] \
   && [ "$(ls -A "$d/dst" | wc -l)" -eq 1 ] && grep -q '^inspect .*hft-recorder' "$d/log" \
   && [ "${img#sha256:run}" != "$img" ] && grep -q "^rm .*cid-$img" "$d/log"; then
  ok "w2 установка: бинарь образа работающего hft-recorder ($img), исполняемый, без хвостов ($order)"
else
  nope "w2 установка: exit=$rc, create «$img», вызовы «$order», в dst: $(head -c 40 "$d/dst/ops-watchdog" 2>/dev/null | tr '\n' ' ') — $(head -c 300 "$d/out" 2>/dev/null)"
fi

# w3..w5 — fail-closed: любой отказ ⇒ exit≠0, прежний бинарь цел, временных файлов нет.
for pair in "w3:cp" "w4:inspect" "w5:empty"; do
  w="${pair%%:*}"; m="${pair##*:}"; d="$SANDBOX/$w"
  rc="$(run_install "$d" "$m")"
  if [ "$rc" = 99 ] || [ "$rc" = 98 ]; then nope "$w SETUP не состоялся (код $rc)"; continue; fi
  extra=""
  [ "$m" = inspect ] && grep -q "^create" "$d/log" 2>/dev/null && extra="create позван после отказа inspect"
  [ "$m" = cp ] && ! grep -q "^rm .*cid-" "$d/log" 2>/dev/null && extra="контейнер не удалён после отказа cp"
  if [ "$rc" != 0 ] && [ "$(cat "$d/dst/ops-watchdog")" = OLD ] \
     && [ "$(ls -A "$d/dst" | wc -l)" -eq 1 ] && [ -z "$extra" ]; then
    ok "$w отказ «$m»: exit=$rc, прежний бинарь цел, хвостов нет"
  else
    nope "$w отказ «$m»: exit=$rc, dst=«$(head -c 40 "$d/dst/ops-watchdog" 2>/dev/null)», файлов $(ls -A "$d/dst" 2>/dev/null | wc -l) ${extra}"
  fi
done

# w6 — образ собирает и кладёт бинарь (глубокая проверка запуском образа — verify_delivery_M-08.sh D9-deep).
if grep -qE -- '--bin[[:space:]]+ops-watchdog' "$ROOT/Dockerfile" \
   && grep -qE '^COPY --from=builder /build/target/release/ops-watchdog /usr/local/bin/ops-watchdog$' "$ROOT/Dockerfile"; then
  ok "w6 Dockerfile: ops-watchdog собирается и копируется в /usr/local/bin"
else
  nope "w6 Dockerfile: нет --bin ops-watchdog и/или COPY в /usr/local/bin/ops-watchdog"
fi

# w7 — деплой зовёт установку на ОБЕИХ ветках: после healthy и после отката. Предел назван:
# deploy.yml исполняется только на VPS — здесь проверка по тексту ветвей, прод-исход — §8.
DY="$ROOT/.github/workflows/deploy.yml"
ok_branch="$(awk '/healthy \(recorder \+ gateway-serve\)/{f=1} f&&/^ *else$/{exit} f' "$DY" | grep -c 'deploy/bin/install-watchdog.sh')"
rb_branch="$(awk '/rollback to/{f=1} f&&/^ *fi$/{exit} f' "$DY" | grep -c 'deploy/bin/install-watchdog.sh')"
if [ "$ok_branch" -ge 1 ] && [ "$rb_branch" -ge 1 ]; then
  ok "w7 deploy.yml: установка сторожа на ветке healthy и на ветке отката"
else
  nope "w7 deploy.yml: установка сторожа — ветка healthy: $ok_branch, ветка отката: $rb_branch (нужно ≥1 на обеих)"
fi

# w8 — cron без бинаря по-прежнему КРИЧИТ (fail-closed обёртки не ослаблен).
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
