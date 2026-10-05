#!/usr/bin/env bash
# RED M-94 — гейт деплоя профиля расчётов: выдача НЕ перезапускается, если (1) host `.env` несёт
# величину профиля, (2) слепка профиля нового образа нет в томе («прогреть, потом переключить»).
# Спека: milestones/M-94-calc-profile.md §3.7, инвариант I-6, оракулы g0…g7.
#
# Контракт вызова гейта (спека §3.7): `deploy/bin/calc-profile-gate.sh`; швы `HFT_ROOT` (каталог
# с `.env`), `CALC_GATE_RUNNER` (вместо `docker`), `CALC_GATE_CKPT_HOST_DIR` (том слепков на хосте).
# Имя слепка гейт берёт у НОВОГО образа: `<runner> compose run --rm --no-deps gateway-checkpoint
# --print-ckpt-name`.
#
# Заглушка runner'а пишет argv в журнал и печатает STUB_NAME с кодом STUB_RC — так проба различает
# «runner не звался» (`.env` отвергнут ДО него) и «звался с не тем argv».
#
# Проводка и откат деплоя — в соседней пробе `red_m94_deploy_apply.sh` (исполняется, не читается).
#
# Число сценариев НЕ заявляется — считается и печатается. Решение — по коду возврата.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# M94_GATE_UNDER_TEST — только для мутационной проверки САМОЙ пробы (эталон и мутанты гейта).
GATE="${M94_GATE_UNDER_TEST:-$ROOT/deploy/bin/calc-profile-gate.sh}"
PASS=0; FAIL=0
ok()   { printf 'pass  %s\n' "$*"; PASS=$((PASS+1)); }
nope() { printf 'FAIL  %s\n' "$*"; FAIL=$((FAIL+1)); }
SANDBOX="$(mktemp -d)"; trap 'rm -rf "$SANDBOX"' EXIT

NAME="ckpt-8f69809dd707e8c9.bin"
KEYS=(GATEWAY_BANDS GATEWAY_DEPTH_CADENCE_MS GATEWAY_TIMEFRAME_MS GATEWAY_WINDOW_MS
      GATEWAY_ALLOWED_PROFILES GATEWAY_VP_BIN_WIDTH_E8 GATEWAY_HEATMAP_WINDOW GATEWAY_CANONICAL_BANDS)

# ── мир: <имя> <текст .env> <слепок есть: 1|0> <STUB_NAME> <STUB_RC> → печатает каталог мира ──
mk_world() {
  local w="$SANDBOX/$1"; mkdir -p "$w/root" "$w/ckpt" "$w/bin" || return 1
  printf '%s' "$2" > "$w/root/.env" || return 1
  [ "$3" = 1 ] && : > "$w/ckpt/$NAME"
  cat > "$w/bin/runner" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$w/runner.log"
printf '%s\n' "$4"
exit $5
EOF
  chmod +x "$w/bin/runner" || return 1
  # страж подготовки: мир собран так, как заявлен
  [ -f "$w/root/.env" ] && [ -x "$w/bin/runner" ] \
    && { [ "$3" = 0 ] || [ -f "$w/ckpt/$NAME" ]; } && echo "$w"
}

run_gate() { # <каталог мира> → печатает вывод, код в RC
  local w="$1"
  OUT=$(HFT_ROOT="$w/root" CALC_GATE_RUNNER="$w/bin/runner" CALC_GATE_CKPT_HOST_DIR="$w/ckpt" \
        bash "$GATE" 2>&1); RC=$?
}

# ── g0: гейт существует ──────────────────────────────────────────────────────────────
if [ ! -f "$GATE" ]; then
  nope "g0 SETUP: гейта $GATE нет — деплой перезапускает выдачу без проверки профиля (I-6)"
  printf 'VERDICT: FAIL (pass=%d fail=%d; остальные миры не исполнялись — судить нечего)\n' "$PASS" "$FAIL"
  exit 1
fi
ok "g0 гейт существует"

# ── g1: чистый .env, слепок есть ⇒ проход; runner позван ровно с нужным argv ─────────
if w=$(mk_world g1 $'GATEWAY_JWT_SECRET=x\n' 1 "$NAME" 0); then
  run_gate "$w"
  if [ "$RC" -eq 0 ] && grep -qxF 'compose run --rm --no-deps gateway-checkpoint --print-ckpt-name' "$w/runner.log" 2>/dev/null; then
    ok "g1 чистый .env + слепок есть ⇒ проход, имя спрошено у нового образа"
  else
    nope "g1 ожидался проход с вызовом runner'а; RC=$RC; runner: $(cat "$w/runner.log" 2>/dev/null); вывод: ${OUT:0:200}"
  fi
else nope "g1 SETUP не состоялся"; fi

# ── g2: .env несёт GATEWAY_BANDS с ПРОД-значением ⇒ отказ ДО runner'а ───────────────
if w=$(mk_world g2 $'GATEWAY_JWT_SECRET=x\nGATEWAY_BANDS=0.015,0.03,0.05,0.08,0.15,0.3,0.6\n' 1 "$NAME" 0); then
  run_gate "$w"
  if [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -q 'GATEWAY_BANDS' && [ ! -s "$w/runner.log" ]; then
    ok "g2 .env с GATEWAY_BANDS (равным профилю) ⇒ отказ с именем ключа, runner не звался"
  else
    nope "g2 .env с GATEWAY_BANDS: RC=$RC, ключ назван: $(printf '%s' "$OUT" | grep -c GATEWAY_BANDS), runner: $(cat "$w/runner.log" 2>/dev/null)"
  fi
else nope "g2 SETUP не состоялся"; fi

# ── g3: каждый из восьми ключей, три формы строки ⇒ отказ ДО runner'а ───────────────
for k in "${KEYS[@]}"; do
  i=0
  for form in "${k}=1" "export ${k}=1" "  ${k}=1"; do
    i=$((i+1))
    if w=$(mk_world "g3-$k-$i" "GATEWAY_JWT_SECRET=x"$'\n'"$form"$'\n' 1 "$NAME" 0); then
      run_gate "$w"
      if [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -q "$k" && [ ! -s "$w/runner.log" ]; then
        ok "g3 .env «$form» ⇒ отказ"
      else
        nope "g3 .env «$form»: RC=$RC, ключ назван: $(printf '%s' "$OUT" | grep -c "$k"), runner звался: $([ -s "$w/runner.log" ] && echo да || echo нет)"
      fi
    else nope "g3 SETUP «$form» не состоялся"; fi
  done
done

# ── g3-контроль: закомментированный ключ и ключ с другим префиксом — НЕ отказ ───────
if w=$(mk_world g3c $'GATEWAY_JWT_SECRET=x\n# GATEWAY_BANDS=0.015\nMY_GATEWAY_BANDS_NOTE=1\n' 1 "$NAME" 0); then
  run_gate "$w"
  if [ "$RC" -eq 0 ]; then ok "g3c комментарий и чужое имя не считаются ключом профиля (проба не ложно-строгая)"
  else nope "g3c ложный отказ на комментарии/чужом имени: RC=$RC; ${OUT:0:200}"; fi
else nope "g3c SETUP не состоялся"; fi

# ── g4: слепка нет ⇒ отказ с именем файла ───────────────────────────────────────────
if w=$(mk_world g4 $'GATEWAY_JWT_SECRET=x\n' 0 "$NAME" 0); then
  run_gate "$w"
  if [ "$RC" -ne 0 ] && printf '%s' "$OUT" | grep -qF "$NAME"; then
    ok "g4 слепка профиля нет ⇒ отказ, имя файла названо"
  else
    nope "g4 слепка нет: RC=$RC; вывод: ${OUT:0:200}"
  fi
else nope "g4 SETUP не состоялся"; fi

# ── g5: runner печатает не имя слепка ⇒ отказ (даже если такой «файл» лежит в томе) ──
if w=$(mk_world g5 $'GATEWAY_JWT_SECRET=x\n' 1 "covered_through_seq" 0); then
  : > "$w/ckpt/covered_through_seq"
  run_gate "$w"
  if [ "$RC" -ne 0 ]; then ok "g5 не-имя слепка от runner'а ⇒ отказ"
  else nope "g5 гейт принял «covered_through_seq» за имя слепка"; fi
else nope "g5 SETUP не состоялся"; fi

# ── g6: runner падает ⇒ отказ ───────────────────────────────────────────────────────
if w=$(mk_world g6 $'GATEWAY_JWT_SECRET=x\n' 1 "$NAME" 1); then
  run_gate "$w"
  if [ "$RC" -ne 0 ]; then ok "g6 падение runner'а ⇒ отказ"
  else nope "g6 гейт прошёл при упавшем runner'е"; fi
else nope "g6 SETUP не состоялся"; fi

# ── g7 СНЯТ (C-280 R3): порядок строк deploy.yml не доказывал откат. Проводка и откат теперь
# ИСПОЛНЯЮТСЯ пробой scripts/tests/red_m94_deploy_apply.sh (a1…a4, a7).

printf 'сценариев: %d (pass=%d fail=%d)\n' "$((PASS+FAIL))" "$PASS" "$FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL"; exit 1
