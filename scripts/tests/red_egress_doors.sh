#!/usr/bin/env bash
# Проба-ИНВЕНТАРИЗАЦИЯ дверей выдачи — M-71, исполнение `A-021` Вопрос 3, Правка B.
#
# ЗАЧЕМ. Два круга гейта подряд находили дверь, которую оракул не звал: `C-157` R2 — живой
# WS-путь, `C-158` R1 — serve-обёртки и v1-конверт. Перечень «по построению» покрывает лишь
# то, о чём вспомнил автор. Правило границы `A-020` требует смены КОНСТРУКЦИИ, а не третьей
# двери: **список дверей проверяет машина.**
#
# ЧЕМ ЭТО ОТЛИЧАЕТСЯ ОТ КАНАРЕЙКИ, ЗАПРЕЩЁННОЙ `A-020`. Там лексический инструмент судил
# ПРОВЕНАНС ЗНАЧЕНИЯ в языке с открытым множеством каналов (`local X="$X"`, nameref, `eval`) —
# и не мог сойтись в принципе. Здесь греп ИНВЕНТАРИЗУЕТ закрытую синтаксически регулярную
# поверхность (сигнатуры Rust), а его отказ — не суждение о данных, а требование «дверь не
# названа в оракуле, допиши». Разница названа арбитром и записана здесь, чтобы следующий круг
# не спорил о ней заново.
#
# ИМЕНОВАННЫЙ ОСТАТОК (`COGNITIVE-ONLY`): двери, порождённые макросами или приходящие через
# трейт-объекты, лексический перебор НЕ ВИДИТ. Находки этого рода — NOTE, не REJECT (`A-021`).
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 2

L1=crates/gateway/tests/red_egress_cap.rs
L2=crates/gateway-serve/tests/red_egress_cap_wire.rs
FAIL=0

for f in "${L1}" "${L2}"; do
  [ -f "${f}" ] || { echo "FAIL: SETUP — нет файла оракула ${f}"; exit 1; }
done

echo "=== УРОВЕНЬ 1: публичные строители gateway, принимающие &Selector ==="
# Свободные функции крейта + методы `LiveReducer`. Имя двери — то, чем её зовут из оракула.
DOORS1=$(grep -hoE '^pub fn [a-z_]+\(' crates/gateway/src/lib.rs | sed 's/^pub fn //; s/($//; s/(//' | sort -u)
LIVE=$(grep -hoE '^    pub fn (resume|pump)\(' crates/gateway/src/lib.rs | sed 's/^ *pub fn //; s/(//' | sort -u)

# ИСКЛЮЧЕНИЯ — список ЯВНЫЙ и с причиной, а не молчаливый фильтр. Дверь, выведенная отсюда,
# видна читателю: если причина перестанет быть верной, это заметят, а не пропустят.
#   validate_selector — САМ ГВАРД, а не строитель ответа: он принимает `&Selector`, но ничего
#                       не строит и ничего не отдаёт клиенту. Звать его из оракула предела
#                       нечего; его собственные инварианты — `GW-I-10`/`GW-I-14`.
EXCLUDE="validate_selector"

for d in ${DOORS1}; do
  case " ${EXCLUDE} " in *" ${d} "*) echo "SKIP: ${d} — в явных исключениях (см. EXCLUDE)"; continue;; esac
  # Строителем ответа считаем функцию, чья сигнатура принимает `&Selector`.
  if ! awk -v fn="pub fn ${d}(" 'index($0,fn){f=1} f&&/&Selector/{print;exit}' crates/gateway/src/lib.rs | grep -q .; then
    continue
  fi
  if grep -q "gateway::${d}(" "${L1}"; then
    echo "PASS: дверь L1 ${d} — названа в оракуле"
  else
    echo "FAIL: дверь L1 ${d} принимает &Selector, но оракул ${L1} её НЕ ЗОВЁТ"
    FAIL=$((FAIL + 1))
  fi
done

for d in ${LIVE}; do
  if grep -q "\.${d}(\|LiveReducer::${d}(" "${L1}"; then
    echo "PASS: дверь L1 LiveReducer::${d} — названа в оракуле"
  else
    echo "FAIL: дверь L1 LiveReducer::${d} не названа в оракуле ${L1}"
    FAIL=$((FAIL + 1))
  fi
done

echo "=== УРОВЕНЬ 2: строители исходящего текста в gateway-serve ==="
# `C-159` R1: прежняя редакция перечисляла ЧЕТЫРЕ ЛИТЕРАЛА и потому пропустила `error_msg` —
# путь, которым клиент управляет исходящим текстом через ОТКАЗ. Перечень литералов не
# инвентаризация, а та же память автора, только в скрипте. Теперь двери НАХОДЯТСЯ:
#   (а) все `pub fn *_msg(` в `wire_v1` — соглашение об именовании строителей исходящих форм;
#   (б) все `pub fn` внутри `pub mod serve` — serve-адаптеры.
DOORS_WIRE=$(grep -hoE '^pub fn [a-z_]+_msg\(' crates/gateway-serve/src/wire_v1.rs | sed 's/^pub fn //; s/(//' | sort -u)
DOORS_SERVE=$(awk '/^pub mod serve \{/{m=1} m&&/^    pub fn /{sub(/\(.*/,"");sub(/^    pub fn /,"");print} m&&/^\}/{m=0}' crates/gateway-serve/src/lib.rs | sort -u)

if [ -z "${DOORS_WIRE}" ] || [ -z "${DOORS_SERVE}" ]; then
  echo "FAIL: SETUP — перебор дверей уровня 2 не нашёл НИ ОДНОЙ (wire='${DOORS_WIRE}' serve='${DOORS_SERVE}')."
  echo "      Пустой перебор зелен по построению — это ровно тот вакуум, против которого проба и написана."
  FAIL=$((FAIL + 1))
fi

for d in ${DOORS_WIRE}; do
  if grep -q "wire_v1::${d}(" "${L2}"; then
    echo "PASS: дверь L2 wire_v1::${d} — названа в оракуле"
  else
    echo "FAIL: дверь L2 wire_v1::${d} строит исходящий текст, но оракул ${L2} её НЕ ЗОВЁТ"
    FAIL=$((FAIL + 1))
  fi
done

for d in ${DOORS_SERVE}; do
  if grep -q "serve::${d}(" "${L2}"; then
    echo "PASS: дверь L2 serve::${d} — названа в оракуле"
  else
    echo "FAIL: дверь L2 serve::${d} не названа в оракуле ${L2}"
    FAIL=$((FAIL + 1))
  fi
done

# ОСТАТОК, названный честно: точка отправки, которая строит текст МИМО `*_msg`-строителя,
# этому перебору не видна. Числом она наблюдаема — печатаем её, чтобы расхождение бросалось
# в глаза при ревизии, но НЕ роняем шаг: это `COGNITIVE-ONLY` (`A-021` Правка B).
N_SEND=$(grep -c 'Message::Text(' crates/gateway-serve/src/lib.rs || echo 0)
echo "NOTE: точек Message::Text( в lib.rs — ${N_SEND} (приём и отправка вместе). Текст, собранный"
echo "      МИМО *_msg-строителя, лексическому перебору не виден — COGNITIVE-ONLY, не гейт."

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS — все найденные двери названы в оракулах"; exit 0; fi
echo "VERDICT: FAIL (${FAIL}) — дверь существует, а оракул её не зовёт"; exit 1
