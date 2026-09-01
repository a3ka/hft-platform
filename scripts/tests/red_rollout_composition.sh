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
  { echo "# epochs"; local i=0
    while [ "$i" -lt "$decl_n" ]; do
      echo "<!-- ACTIVE-COMPOSITION"; echo "epoch_id: $d_epoch"
      echo "l2delta_symbols: $d_syms"; echo "signature: $d_sign"; echo "-->"; i=$((i+1))
    done; } > "$d/epochs.md"
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

echo
echo "каталогов после прогона: $(ls -d /tmp/red-rollout-comp-* 2>/dev/null | wc -l) (до уборки; trap EXIT снимет)"
echo "PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ] || { echo "VERDICT: FAIL"; exit 1; }
echo "VERDICT: PASS — $PASS сценариев"
