#!/usr/bin/env bash
# Барьер состава записи — TD-196 (закрывает TD-194 и R-168 Н-2).
# Проба: scripts/tests/red_rollout_composition.sh
#
# ЗАЧЕМ. Состав записываемых данных — граница C (`gates.md` §0.1): его меняет подпись
# владельца, не инженер. Пока M-45 шёл через гейты, литерал сторожили шаги T10/T10c
# в `scripts/verify_M-45.sh`. Замер при close-out (`TD-194`): `grep -c 'verify_M-45'
# .github/workflows/ci.yml` → 0. То есть защита ИСТЕКЛА В МОМЕНТ MERGE'А — следующий
# коммит, дописавший символ в `docker-compose.yml`, не встретил бы ни одного красного
# джоба. Класс «built-not-wired» наоборот: механизм построен и отключён приёмкой.
#
# ЧТО ИМЕННО ПРОВЕРЯЕТСЯ — не «символы равны BTCUSDT,ETHUSDT», а СОГЛАСОВАННОСТЬ трёх мест:
#   1. `docker-compose.yml` — что получит recorder;
#   2. блок `ACTIVE-COMPOSITION` в `docs/data-epochs.md` — что объявлено действующим;
#   3. `docs/PENDING-SIGNATURE.md` — что подпись, названная декларацией, существует.
# Зашитых символов в барьере НЕТ намеренно: состав меняется с каждой новой подписью, и
# барьер с константой пришлось бы править кодом при каждом решении границы C — то есть
# совершать решение владельца правкой кода (`M-45` §1 против этого и написан).
#
# ── КОДЫ ВОЗВРАТА ─────────────────────────────────────────────────────────────────────
#   0 — три места согласованы; состав записан ЛИТЕРАЛОМ; подпись существует.
#   1 — расхождение (перечисляются ВСЕ, а не первое).
#   2 — предмет не установлен достоверно: нет файла, нет/дублируется блок декларации,
#       не найден сервис recorder, недоступен python/pyyaml. Fail-closed: «проверять
#       нечего» никогда не равно «всё хорошо» (`testing.md` §«Целостность гейта» св. 3).
#
# ПЕРЕМЕННЫЕ ROOT/COMPOSE/EPOCHS/SIGNATURES существуют ДЛЯ ПРОБЫ. Прод-форма — без них.

set -uo pipefail

ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
COMPOSE="${COMPOSE:-$ROOT/docker-compose.yml}"
EPOCHS="${EPOCHS:-$ROOT/docs/data-epochs.md}"
SIGNATURES="${SIGNATURES:-$ROOT/docs/PENDING-SIGNATURE.md}"
CLI="${CLI:-$ROOT/scripts/lib/rollout_symbols_check.py}"

setup_fail() { echo "FAIL  SETUP НЕ СОСТОЯЛСЯ — $*"; echo; echo "VERDICT: FAIL (setup)"; exit 2; }

for f in "$COMPOSE" "$EPOCHS" "$SIGNATURES" "$CLI"; do
  [ -r "$f" ] || setup_fail "нечитаем или отсутствует: $f"
done
command -v python3 >/dev/null 2>&1 || setup_fail "python3 недоступен"

# ── Декларация: блок обязан быть РОВНО ОДИН ───────────────────────────────────────────
# Ноль блоков = состав никем не объявлен; два = неизвестно, какой действует. Оба случая —
# отказ, а не выбор первого попавшегося.
OPEN_N=$(grep -c '^<!-- ACTIVE-COMPOSITION[[:space:]]*$' "$EPOCHS" || true)
[ "$OPEN_N" -eq 1 ] || setup_fail "блоков ACTIVE-COMPOSITION в $EPOCHS: $OPEN_N (нужен ровно 1)"

DECL="$(awk '/^<!-- ACTIVE-COMPOSITION[[:space:]]*$/{f=1;next} f&&/^-->[[:space:]]*$/{exit} f' "$EPOCHS")"
decl_field() { printf '%s\n' "$DECL" | sed -n "s/^$1:[[:space:]]*//p" | head -1; }

D_EPOCH="$(decl_field epoch_id)"
D_SYMS="$(decl_field l2delta_symbols)"
D_SIGN="$(decl_field signature)"
for pair in "epoch_id:$D_EPOCH" "l2delta_symbols:$D_SYMS" "signature:$D_SIGN"; do
  [ -n "${pair#*:}" ] || setup_fail "в декларации пусто поле ${pair%%:*}"
done

# ── Что получит recorder: снимается ТЕМ ЖЕ CLI, что судит форму значения ──────────────
# `Р-1`: мера на границе потребителя. Второй разбор YAML здесь был бы редекларацией —
# ошибка, уже стоившая круга гейта на M-45 (`C-202` B-2).
EX_OUT="$(COMPOSE_PATH="$COMPOSE" python3 "$CLI" --extract "$COMPOSE" 2>&1)"; EX_ST=$?
[ "$EX_ST" -eq 0 ] || setup_fail "извлечение конфигурации вернуло $EX_ST: $EX_OUT"
C_SYMS="$(printf '%s\n' "$EX_OUT" | sed -n 's/^L2DELTA_CAPTURE_SYMBOLS=//p')"
C_EPOCH="$(printf '%s\n' "$EX_OUT" | sed -n 's/^EPOCH_ID=//p')"
[ -n "$C_SYMS" ] && [ -n "$C_EPOCH" ] || setup_fail "в выводе --extract нет ожидаемых строк: $EX_OUT"

BAD=0
bad() { echo "FAIL  $*"; BAD=$((BAD + 1)); }
ok()  { echo "PASS  $*"; }

# ── 1. Состав: ЛИТЕРАЛ и равен объявленному ──────────────────────────────────────────
# Подстановка `${VAR:-...}` выглядит подписанной и переопределяется одной переменной
# окружения хоста — эффективный обход подписи (`M-45` §3quinquies, находка `C-202` B-1).
if printf '%s' "$C_SYMS" | grep -qE '\$\{[^}]*\}|\$[A-Za-z_][A-Za-z0-9_]*'; then
  bad "состав записан ПОДСТАНОВКОЙ ($C_SYMS) — переопределяется окружением, подпись не удержана"
elif [ "$C_SYMS" = "<ОТСУТСТВУЕТ>" ]; then
  bad "L2DELTA_CAPTURE_SYMBOLS отсутствует на сервисе recorder, а декларация объявляет '$D_SYMS'"
elif [ "$C_SYMS" != "$D_SYMS" ]; then
  bad "состав в compose ('$C_SYMS') НЕ РАВЕН объявленному в ACTIVE-COMPOSITION ('$D_SYMS') — \
изменение состава записи есть решение границы C и требует подписи владельца, а не правки конфига"
else
  ok "состав литералом и равен объявленному ($C_SYMS)"
fi

# ── 2. Эпоха: равна объявленной ──────────────────────────────────────────────────────
# Состав, сменившийся без смены эпохи, делает события двух составов машинно неразличимыми:
# класс E-001, стоивший разбора 123 млн событий постфактум.
if [ "$C_EPOCH" = "<ОТСУТСТВУЕТ>" ]; then
  bad "EPOCH_ID отсутствует на сервисе recorder, а декларация объявляет '$D_EPOCH'"
elif [ "$C_EPOCH" != "$D_EPOCH" ]; then
  bad "эпоха в compose ('$C_EPOCH') НЕ РАВНА объявленной ('$D_EPOCH') — граница эпохи разъедется с составом (E-001)"
else
  ok "эпоха равна объявленной ($C_EPOCH)"
fi

# ── 3. Подпись, названная декларацией, СУЩЕСТВУЕТ ────────────────────────────────────
# Барьер не судит содержание подписи — только что ссылка не висячая. Декларация, ссылающаяся
# в пустоту, есть та же ложь, что документ, обосновывающий инвариант несуществующим
# механизмом (класс `TD-138`).
if grep -qE "^## ${D_SIGN}([^0-9]|$)" "$SIGNATURES"; then
  ok "подпись $D_SIGN существует в $(basename "$SIGNATURES")"
else
  bad "декларация ссылается на подпись '$D_SIGN', которой НЕТ в $(basename "$SIGNATURES") — висячая ссылка"
fi

echo
if [ "$BAD" -gt 0 ]; then
  echo "VERDICT: FAIL ($BAD нарушений)"
  exit 1
fi
echo "VERDICT: PASS — состав, эпоха и подпись согласованы"
exit 0
