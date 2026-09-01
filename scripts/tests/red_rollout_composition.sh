#!/usr/bin/env bash
# Проба барьера состава записи — TD-196. Предмет: scripts/check_rollout_composition.sh
#
# ГЛАВНОЕ ТРЕБОВАНИЕ ТРЕКА (`harness-track.md` §5 п.1): проба зелёная против честной
# реализации и КРАСНАЯ против обманных стабов. Барьер, чьё красное не предъявлено,
# считается отсутствующим.
#
# Фикстуры — во ВРЕМЕННОМ каталоге, реестр в ФАЙЛЕ, уборка через `trap EXIT`: класс,
# давший 10 400 каталогов в /tmp и диск на 100 % (`harness-track.md` §5 п.5).

set -uo pipefail
ROOT_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BARRIER="$ROOT_REPO/scripts/check_rollout_composition.sh"
CLI="$ROOT_REPO/scripts/lib/rollout_symbols_check.py"

TMP="$(mktemp -d /tmp/red-rollout-comp-XXXXXX)"
REGISTRY="$TMP/.registry"; : > "$REGISTRY"; echo "$TMP" >> "$REGISTRY"
cleanup() { while read -r d; do [ -n "$d" ] && rm -rf "$d"; done < "$REGISTRY"; }
trap cleanup EXIT

PASS=0; FAIL=0
# Ожидание задаётся КОДОМ ВОЗВРАТА, а не текстом вывода (`gates.md` §3).
run_case() {
  local why="$1" want="$2" dir="$3"
  local got; ROOT="$dir" COMPOSE="$dir/docker-compose.yml" EPOCHS="$dir/epochs.md" \
    SIGNATURES="$dir/signatures.md" CLI="$CLI" bash "$BARRIER" >/dev/null 2>&1
  got=$?
  if [ "$got" -eq "$want" ]; then PASS=$((PASS+1)); printf 'ok    %-58s код %s\n' "$why" "$got"
  else FAIL=$((FAIL+1)); printf 'FAIL  %-58s ожидался %s, получен %s\n' "$why" "$want" "$got"; fi
}

# Мир строится ЦЕЛИКОМ под сценарий: дописка к готовому файлу дала бы дубль ключа, а YAML
# берёт последний — мутация исчезла бы молча (дефект, стоивший круга гейта на M-45).
make_world() {
  local d="$TMP/$1"; shift
  local syms="$1" epoch="$2" d_syms="$3" d_epoch="$4" d_sign="$5" sig_present="$6" decl_n="${7:-1}"
  mkdir -p "$d"; echo "$d" >> "$REGISTRY"
  { echo "services:"; echo "  recorder:"; echo "    image: x"
    echo "    container_name: hft-recorder"; echo "    environment:"
    [ "$syms" != "-" ] && echo "      L2DELTA_CAPTURE_SYMBOLS: $syms"
    [ "$epoch" != "-" ] && echo "      EPOCH_ID: $epoch"; } > "$d/docker-compose.yml"
  # DECL_MODE управляет ФОРМОЙ блока: normal | unclosed | dupfield | crlf.
  # Каждый режим — свой страж (`C-206` Б-3); по `Р-3` страж без своей фикстуры не запиннен.
  { echo "# epochs"; local i=0
    while [ "$i" -lt "$decl_n" ]; do
      echo "<!-- ACTIVE-COMPOSITION"; echo "epoch_id: $d_epoch"
      echo "l2delta_symbols: $d_syms"; echo "signature: $d_sign"
      [ "${DECL_MODE:-normal}" = dupfield ] && echo "signature: П-999"
      [ "${DECL_MODE:-normal}" = unclosed ] || echo "-->"
      i=$((i+1))
    done; echo "проза после блока"; } > "$d/epochs.md"
  [ "${DECL_MODE:-normal}" = crlf ] && sed -i 's/$/\r/' "$d/epochs.md"
  { echo "# signatures"; [ "$sig_present" = "yes" ] && echo "## $d_sign — ПОДПИСАНО"; } > "$d/signatures.md"
  printf '%s' "$d"
}

echo "--- ПОЗИТИВНЫЙ КОНТРОЛЬ: честный мир обязан быть зелёным ---"
# Без него проба может быть вечно-красной, и её «объявят шумом и выключат».
run_case "честный мир: три места согласованы" 0 "$(make_world w_ok 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"

echo "--- ОБМАННЫЕ СТАБЫ: каждый обязан покраснеть ---"
run_case "состав в compose РАСШИРЕН против декларации"      1 "$(make_world w_extra 'BTCUSDT,ETHUSDT,SOLUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
run_case "состав в compose СУЖЕН против декларации"          1 "$(make_world w_less 'BTCUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
run_case "состав ПОДСТАНОВКОЙ (обходится окружением)"        1 "$(make_world w_sub '${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT}' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
run_case "состав подстановкой БЕЗ двоеточия"                 1 "$(make_world w_sub2 '${L2DELTA_CAPTURE_SYMBOLS-BTCUSDT,ETHUSDT}' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
run_case "эпоха в compose РАЗЪЕХАЛАСЬ с декларацией"         1 "$(make_world w_ep 'BTCUSDT,ETHUSDT' own-СТАРАЯ 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
# РАЗЛИЧАЮЩИЙ сценарий запрета подстановки: compose и декларация несут ОДИН И ТОТ ЖЕ текст,
# то есть сверка равенства проходит, и красное обязана дать ИМЕННО проверка формы. Без него
# запрет подстановки не был запиннен ничем — вскрыто мутацией М3, а не рассуждением.
run_case "подстановка И в compose, И в декларации (равенство проходит)" 1 "$(make_world w_subeq '${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT}' own-e '${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT}' own-e П-026 yes)"
run_case "подпись декларации НЕ СУЩЕСТВУЕТ (висячая ссылка)" 1 "$(make_world w_sig 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-999 no)"
run_case "ключа состава НЕТ на сервисе recorder"             1 "$(make_world w_nosym '-' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
run_case "ключа эпохи НЕТ на сервисе recorder"               1 "$(make_world w_noep 'BTCUSDT,ETHUSDT' '-' 'BTCUSDT,ETHUSDT' own-e П-026 yes)"

echo "--- SETUP НЕ СОСТОЯЛСЯ: отказ, а не тихий пропуск (код 2) ---"
run_case "декларации НЕТ вовсе"                              2 "$(make_world w_nodecl 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes 0)"
run_case "деклараций ДВЕ — какая действует, неизвестно"      2 "$(make_world w_2decl 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes 2)"

D_ABS="$(make_world w_abs 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"; rm -f "$D_ABS/docker-compose.yml"
run_case "compose отсутствует" 2 "$D_ABS"

D_SVC="$(make_world w_svc 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
sed -i 's/container_name: hft-recorder/container_name: hft-other/' "$D_SVC/docker-compose.yml"
run_case "сервис recorder НЕ НАЙДЕН" 2 "$D_SVC"

D_EMPTY="$(make_world w_empty 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
sed -i 's/^l2delta_symbols:.*/l2delta_symbols:/' "$D_EMPTY/epochs.md"
run_case "поле декларации ПУСТО" 2 "$D_EMPTY"

echo "--- НОВЫЕ СТРАЖИ (C-206 Б-3, Н-1): форма декларации ---"
DECL_MODE=unclosed run_case "блок НЕ ЗАКРЫТ '-->' (awk брал бы всё до EOF)"  2 "$(DECL_MODE=unclosed make_world w_unclosed 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
DECL_MODE=dupfield run_case "поле signature ДВАЖДЫ (судился бы первый)"      2 "$(DECL_MODE=dupfield make_world w_dup 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
DECL_MODE=crlf     run_case "CRLF в декларации — честный мир, ЛОЖНОГО красного быть не должно" 0 "$(DECL_MODE=crlf make_world w_crlf 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e П-026 yes)"
run_case "signature — ОБРАЗЕЦ '.*', а не идентификатор (инъекция в grep)"    1 "$(make_world w_inj 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e '.*' yes)"
run_case "signature не в форме П-<число>"                                    1 "$(make_world w_form 'BTCUSDT,ETHUSDT' own-e 'BTCUSDT,ETHUSDT' own-e 'П-абв' yes)"

echo "--- СВЯЗКА ВО ВРЕМЕНИ (C-206 Б-2): состав изменён ⇒ новая эпоха И новая подпись ---"
# Мир строится git-репозиторием: база — коммит с прежней декларацией, HEAD — с изменённой.
link_world() {
  local name="$1" syms2="$2" epoch2="$3" sign2="$4"
  local d="$TMP/$name"; mkdir -p "$d"; echo "$d" >> "$REGISTRY"
  git -C "$d" init -q 2>/dev/null; git -C "$d" config user.email a@b; git -C "$d" config user.name t
  mkdir -p "$d/docs"
  { echo "services:"; echo "  recorder:"; echo "    image: x"; echo "    container_name: hft-recorder"
    echo "    environment:"; echo "      L2DELTA_CAPTURE_SYMBOLS: BTCUSDT,ETHUSDT"; echo "      EPOCH_ID: own-base"; } > "$d/docker-compose.yml"
  printf '<!-- ACTIVE-COMPOSITION\nepoch_id: own-base\nl2delta_symbols: BTCUSDT,ETHUSDT\nsignature: П-026\n-->\n' > "$d/docs/e.md"
  printf '# sig\n## П-026 — ПОДПИСАНО\n## П-027 — ПОДПИСАНО\n' > "$d/docs/s.md"
  git -C "$d" add -A >/dev/null; git -C "$d" commit -qm base >/dev/null
  local base; base=$(git -C "$d" rev-parse HEAD)
  sed -i "s/L2DELTA_CAPTURE_SYMBOLS: .*/L2DELTA_CAPTURE_SYMBOLS: $syms2/; s/EPOCH_ID: .*/EPOCH_ID: $epoch2/" "$d/docker-compose.yml"
  printf '<!-- ACTIVE-COMPOSITION\nepoch_id: %s\nl2delta_symbols: %s\nsignature: %s\n-->\n' "$epoch2" "$syms2" "$sign2" > "$d/docs/e.md"
  git -C "$d" add -A >/dev/null; git -C "$d" commit -qm head >/dev/null
  printf '%s|%s' "$d" "$base"
}
run_link() {
  local why="$1" want="$2" spec="$3"; local d="${spec%|*}" base="${spec#*|}"
  local got; ROOT="$d" COMPOSE="$d/docker-compose.yml" EPOCHS="$d/docs/e.md" \
    SIGNATURES="$d/docs/s.md" CLI="$CLI" PR_BASE_SHA="$base" bash "$BARRIER" >/dev/null 2>&1
  got=$?
  if [ "$got" -eq "$want" ]; then PASS=$((PASS+1)); printf 'ok    %-58s код %s\n' "$why" "$got"
  else FAIL=$((FAIL+1)); printf 'FAIL  %-58s ожидался %s, получен %s\n' "$why" "$want" "$got"; fi
}
run_link "состав РАСШИРЕН, эпоха и подпись ПРЕЖНИЕ"            1 "$(link_world l_same 'BTCUSDT,ETHUSDT,SOLUSDT' own-base П-026)"
run_link "состав расширен, эпоха новая, подпись ПРЕЖНЯЯ"       1 "$(link_world l_sign 'BTCUSDT,ETHUSDT,SOLUSDT' own-new П-026)"
run_link "состав расширен, подпись новая, эпоха ПРЕЖНЯЯ"       1 "$(link_world l_ep   'BTCUSDT,ETHUSDT,SOLUSDT' own-base П-027)"
run_link "состав расширен вместе с новой эпохой И подписью"    0 "$(link_world l_ok   'BTCUSDT,ETHUSDT,SOLUSDT' own-new П-027)"
run_link "состав НЕ менялся — связка молчит"                   0 "$(link_world l_nop  'BTCUSDT,ETHUSDT'         own-base П-026)"

echo
echo "каталогов после прогона: $(ls -d /tmp/red-rollout-comp-* 2>/dev/null | wc -l) (до уборки; trap EXIT снимет)"
echo "PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ] || { echo "VERDICT: FAIL"; exit 1; }
echo "VERDICT: PASS — $PASS сценариев"
