#!/usr/bin/env bash
# Acceptance-гейт M-65 — подписка есть параметр СЕССИИ, а не конфигурация процесса.
#
# Решение по КОДУ ВОЗВРАТА (`gates.md` §3). Агрегатор со счётчиком: печатаем все нарушения,
# exit 1 при FAIL>0 — первый красный шаг не должен скрывать остальные.
#
# УРОКИ, ВСТРОЕННЫЕ ЗАРАНЕЕ (каждый оплачен ложным вердиктом в этом проекте):
#  1. Логи — в СВОЙ каталог, а не по фиксированным путям /tmp. При одновременных прогонах
#     сосед переписывает файл между запуском и разбором, и пруф в Done Block принадлежит
#     чужому процессу; на M-61 шаг T из-за этого переворачивался с красного на зелёное.
#  2. Число исполненного СЧИТАЕТСЯ, а не заявляется: `0 passed` — зелёная строка, не
#     исполнившая ничего.
#  3. Диапазон оракулов назван ЧИСЛОМ и проверяется: строка acceptance уже пережила круг
#     критика со «старым» O-1..O-8 (`C-078` N-2). Умолчи гейт про два оракула — и он
#     зеленеет, не исполнив ровно те, ради которых был REJECT.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1
FAILED=0
LOGD="$(mktemp -d /tmp/m65-verify-XXXXXX)" || { echo "не создан каталог логов" >&2; exit 1; }
trap 'rm -rf "${LOGD}"' EXIT
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

SET="crates/gateway-serve/tests/red_ws_session.rs"
BATTERY="scripts/tests/red_ws_session_battery.sh"
SPEC="milestones/M-65-ws-session.md"

echo "--- A: RED-набор и батарея на месте, парсятся, форматированы ---"
if [ -f "${SET}" ]; then pass "A ${SET} на месте"; else fail "A ${SET} отсутствует"; fi
if cargo fmt --all -- --check >"${LOGD}/fmt.log" 2>&1; then
  pass "A форматирование (паритет с CI-шагом fmt)"
else
  fail "A rustfmt --check красный"; tail -5 "${LOGD}/fmt.log" | sed 's/^/      ↳ /'
fi

echo "--- N: манифест набора ⇄ таблица осей §4.2, в ОБЕ стороны ---"
# Механизм живёт в самом наборе (`o0`) — гейт его ИСПОЛНЯЕТ, а не пересказывает.
# Setup-guard: спека обязана существовать, иначе сверять не с чем и «зелено» ничего не значит.
if [ ! -f "${SPEC}" ]; then
  fail "N SETUP НЕ СОСТОЯЛСЯ: нет ${SPEC} — состав осей сверять не с чем"
elif cargo test -p gateway-serve --test red_ws_session o0_manifest >"${LOGD}/n.log" 2>&1; then
  pass "N манифест ⇄ §4.2 совпал в обе стороны; у каждой из восьми осей есть легитимный сценарий"
else
  fail "N манифест разошёлся со спекой"
  grep -E 'объявлено в §4.2|покрыто набором|усл\. 2' "${LOGD}/n.log" | head -6 | sed 's/^/      ↳ /'
fi

echo "--- F: RED-набор O-1..O-10 GREEN ---"
# Диапазон назван числом НАМЕРЕННО (C-078 N-2): проверяется, что исполнены ВСЕ десять
# оракулов плюс манифест-сверка, а не «сколько нашлось».
EXPECT_TESTS=11
if cargo test -p gateway-serve --test red_ws_session >"${LOGD}/f.log" 2>&1; then
  N_RUN=$(grep -cE '^test o[0-9]+' "${LOGD}/f.log")
  if [ "${N_RUN}" -ge "${EXPECT_TESTS}" ]; then
    pass "F набор GREEN: исполнено ${N_RUN} оракулов (ожидалось ≥ ${EXPECT_TESTS}: o0 + O-1..O-10)"
  else
    fail "F набор зелёный, но исполнено ${N_RUN} оракулов при ожидаемых ${EXPECT_TESTS} — \
часть O-1..O-10 отсутствует или отфильтрована; «зелено» здесь ничего не доказывает"
  fi
else
  fail "F набор КРАСНЫЙ"
  grep -E '^test .* FAILED|^---- ' "${LOGD}/f.log" | head -12 | sed 's/^/      ↳ /'
fi

echo "--- F2: батарея мутантов §4.5 — FAIL-CLOSED до задачи 9 ---"
# Мутанты суть правки РЕАЛИЗАЦИИ, которой ещё нет: батарею физически нельзя написать раньше
# задач 1-6. Шаг объявлен ЯВНО КРАСНЫМ, а не пропущенным: «ещё не написано» обязано быть
# видно гейту, иначе milestone закроется без анти-плацебо.
if [ -f "${BATTERY}" ]; then
  if bash "${BATTERY}" --battery >"${LOGD}/f2.log" 2>&1; then
    pass "F2 $(grep -oE 'BATTERY: PASS \([0-9]+/[0-9]+\)' "${LOGD}/f2.log" | head -1)"
  else
    fail "F2 батарея КРАСНАЯ"
    grep -E '^(FAIL|SETUP)' "${LOGD}/f2.log" | head -8 | sed 's/^/      ↳ /'
  fi
else
  fail "F2 батареи ${BATTERY} НЕТ — анти-плацебо не предъявлено (задача 9, пишется architect'ом ПОСЛЕ задач 1-6)"
fi

echo "--- L: лимит подписок fail-closed В ОБЕ СТОРОНЫ ---"
# Половина «превышение ⇒ error» живёт в O-4. Здесь — ВТОРАЯ половина, недоступная изнутри
# тест-бинаря: невалидный конфиг обязан валить СТАРТ ПРОЦЕССА. Проверяется ИСПОЛНЕНИЕМ той
# же формы вызова, какой процесс зовёт прод (`testing.md`: гейт, проверенный не тем вызовом,
# каким его зовёт прод, не проверен). Валидный диапазон — целое >= 1 (`CT-RFC-09` §2.6).
if cargo build -p gateway-serve --bin gateway-serve >"${LOGD}/build.log" 2>&1; then
  # Каталог сборки берётся из CARGO_TARGET_DIR, если он задан: иначе шаг падал «бинарь не
  # найден» на любом прогоне с внешним target/ — дефект ГЕЙТА, а не кода (поймано базовой линией).
  TDIR="${CARGO_TARGET_DIR:-target}"
  BIN="${TDIR}/debug/gateway-serve"
  [ -x "${BIN}" ] || BIN="$(find "${TDIR}" -maxdepth 3 -name gateway-serve -type f -perm -u+x 2>/dev/null | head -1)"
  if [ -z "${BIN}" ] || [ ! -x "${BIN}" ]; then
    fail "L SETUP НЕ СОСТОЯЛСЯ: бинарь gateway-serve не найден после сборки"
  else
    JD="${LOGD}/journal"; mkdir -p "${JD}"
    for bad in 0 -1 abc; do
      OUT="${LOGD}/l-${bad}.log"
      ( GATEWAY_ADDR="127.0.0.1:0" GATEWAY_JOURNAL_DIR="${JD}" \
        GATEWAY_VENUE="Binance" GATEWAY_SYMBOL="BTCUSDT" GATEWAY_TIMEFRAME_MS="1000" \
        GATEWAY_BANDS="0.001" GATEWAY_WINDOW_MS="60000" GATEWAY_JWT_SECRET="m65" \
        GATEWAY_MAX_SUBSCRIPTIONS="${bad}" \
        timeout 5 "${BIN}" >"${OUT}" 2>&1 )
      RC=$?
      # timeout вернул 124 ⇒ процесс ЖИВ через 5 с, то есть стартовал с невалидным лимитом.
      if [ "${RC}" -eq 124 ]; then
        fail "L лимит «${bad}» НЕ уронил старт — процесс живёт. Соединение, которому нельзя \
подписаться ни на что, это не «выключенная функция», а тихо сломанный сервер (§2.6); \
отсутствие предела при цели 10 000 подключений отдаёт узел одному клиенту"
      elif [ "${RC}" -eq 0 ]; then
        fail "L лимит «${bad}»: процесс завершился УСПЕХОМ — старт не отвергнут"
      else
        pass "L невалидный лимит «${bad}» ⇒ отказ старта (exit=${RC})"
      fi
    done

    # A-015 §3 п.1 (решение арбитража, `gates.md` §0) — ОТСУТСТВИЕ и ПУСТАЯ строка ⇒ бинарь
    # СТАРТУЕТ с подписанной нормой 16 и наблюдаемой записью. До этого дополнения шаг L гонял
    # ТОЛЬКО невалидные значения (`A-015` F14): поведение, о котором шёл весь спор, гейтом не
    # покрывалось ВООБЩЕ — ни в одну сторону.
    #
    # Прод-форма вызова: тот же бинарь, тот же способ подачи окружения, что в `docker-compose.yml`.
    # Юнит-оракул `red_max_subs_config.rs` инъецирует `get`-замыкание и границу процесса НЕ
    # пересекает; здесь пересекается она (`testing.md` §«Механизм несущего пути обязан иметь
    # оракул точки входа»).
    #
    # Решение — по КОДУ ВОЗВРАТА (`gates.md` §3), не по тексту: `timeout 5` вернул 124 ⇒ процесс
    # ЖИВ через 5 с ⇒ старт состоялся. Любой другой код — процесс умер, то есть старт отвергнут.
    for case in absent empty; do
      OUT="${LOGD}/l-${case}.log"
      if [ "${case}" = absent ]; then
        ( GATEWAY_ADDR="127.0.0.1:0" GATEWAY_JOURNAL_DIR="${JD}" \
          GATEWAY_VENUE="Binance" GATEWAY_SYMBOL="BTCUSDT" GATEWAY_TIMEFRAME_MS="1000" \
          GATEWAY_BANDS="0.001" GATEWAY_WINDOW_MS="60000" GATEWAY_JWT_SECRET="m65" \
          timeout 5 "${BIN}" >"${OUT}" 2>&1 )
      else
        ( GATEWAY_ADDR="127.0.0.1:0" GATEWAY_JOURNAL_DIR="${JD}" \
          GATEWAY_VENUE="Binance" GATEWAY_SYMBOL="BTCUSDT" GATEWAY_TIMEFRAME_MS="1000" \
          GATEWAY_BANDS="0.001" GATEWAY_WINDOW_MS="60000" GATEWAY_JWT_SECRET="m65" \
          GATEWAY_MAX_SUBSCRIPTIONS="" \
          timeout 5 "${BIN}" >"${OUT}" 2>&1 )
      fi
      RC=$?
      if [ "${RC}" -eq 124 ]; then
        pass "L лимит «${case}» ⇒ старт СОСТОЯЛСЯ (процесс жив, A-015 §3 п.1)"
        # Наблюдаемость ухода на норму — половина основания решения A-015 (§2.3 (iii)):
        # дефолт выбран вместо отказа старта ИМЕННО потому, что «конфиг не доехал» видно.
        # Молчаливый дефолт превращает решение в ту тихую ложь, против которой возражала
        # отвергнутая позиция, — и тогда права была бы она.
        if grep -q "GATEWAY_MAX_SUBSCRIPTIONS" "${OUT}"; then
          pass "L лимит «${case}» ⇒ уход на норму НАЗВАН в логе старта"
        else
          fail "L лимит «${case}»: старт состоялся МОЛЧА — записи о переменной в логе нет. \
Наблюдаемость не украшение: без неё «конфиг не доехал» неотличимо от штатного запуска"
        fi
      else
        fail "L лимит «${case}» ⇒ старт ОТВЕРГНУТ (exit=${RC}), а A-015 §3 п.1 требует старта \
с подписанной нормой 16. Отказ старта на ОТСУТСТВИЕ — снятая позиция: её основание \
(«parse-error → unbounded», урок R7) к отсутствию не относится — отсутствие даёт величину \
ограниченную и равную подписанной"
        tail -3 "${OUT}" | sed 's/^/      ↳ /'
      fi
    done
  fi
else
  fail "L бинарь не собрался — проверять отказ старта не на чем"
  tail -5 "${LOGD}/build.log" | sed 's/^/      ↳ /'
fi

echo "--- M: регресс — цена M-65 не уплачена соседним инвариантом ---"
if cargo test -p gateway-serve --no-fail-fast \
     --test red_serve_passthrough --test red_ws_protocol --test red_jwt_verify \
     >"${LOGD}/m.log" 2>&1; then
  N_OK=$(grep -cE '^test .* \.\.\. ok' "${LOGD}/m.log")
  if [ "${N_OK}" -gt 0 ]; then
    pass "M соседние оракулы gateway-serve GREEN (${N_OK} тестов)"
  else
    fail "M прогон зелен, но исполнено 0 тестов — «зелено» ничего не значит"
  fi
else
  fail "M соседние оракулы КРАСНЫЕ — M-65 сломал чужой инвариант"
  grep -E '^test .* FAILED' "${LOGD}/m.log" | head -8 | sed 's/^/      ↳ /'
fi

echo "--- T: паритет с CI + НЕНУЛЕВОЕ число исполненных тестов ---"
cargo clippy --workspace --all-targets --all-features -- -D warnings >"${LOGD}/clippy.log" 2>&1 \
  && pass "T clippy" || { fail "T clippy"; tail -5 "${LOGD}/clippy.log" | sed 's/^/      ↳ /'; }
if cargo test --all >"${LOGD}/t.log" 2>&1; then
  N_PASS=$(grep -E '^test result' "${LOGD}/t.log" | awk '{p+=$4} END {print p+0}')
  if [ "${N_PASS:-0}" -gt 0 ]; then pass "T cargo test --all: passed=${N_PASS}"
  else fail "T вернул 0, но исполнил 0 тестов — прогон не состоялся"; fi
else
  fail "T cargo test --all"
  grep -E '^test .* FAILED' "${LOGD}/t.log" | head -8 | sed 's/^/      ↳ /'
fi


# ── Шаг O12 (задача 12в, `R-093` Н-1) — гейт обязан наблюдать ОТСУТСТВИЕ оракула ────────────
#
# Находка, породившая шаг: два круга подряд verify давал VERDICT: PASS, тогда как условие
# `R-086` §5 п.1 (оракул на гонку switch × in-flight pump) НЕ выполнено. Гейт видел только то,
# что прогнанные тесты зелены, и был слеп к тому, что нужного теста нет вовсе. Механизм,
# наблюдающий сбой, но не ОТСУТСТВИЕ, — ровно та слепота, против которой он написан
# (`testing.md`, целостность гейта, свойство 4).
#
# `O-12` живёт под `--features testing`, поэтому обычный `cargo test` его не исполняет: без
# этого шага он молча отсутствовал бы, а гейт оставался зелёным.
echo
echo "── O12: оракул на гонку switch × in-flight pump (задача 12) ──"
O12LOG="${LOGD}/o12.log"
if cargo test -p gateway-serve --features testing --test red_ws_session \
     o12_switch_during_inflight_pump >"${O12LOG}" 2>&1; then
  # ДЕФЕКТ, НАЙДЕННЫЙ НА ПРИЁМКЕ ЗАДАЧИ 13 (architect, 2026-08-22). Прежний паттерн
  # `^test o12_switch_during_inflight_pump .* ok$` требовал ПРОБЕЛА сразу после `pump`, а
  # тест зовётся `o12_switch_during_inflight_pump_does_not_restore_old_sub`. Совпасть он не
  # мог НИКОГДА — и это хуже ложного красного: шаг написан, чтобы наблюдать ОТСУТСТВИЕ
  # оракула, а говорил «НЕ ИСПОЛНЕН» одинаково и когда тест есть, и когда его нет. То есть
  # сам шаг был вакуумен ровно тем способом, против которого написан (`testing.md`,
  # целостность гейта, свойство 4). Суффикс имени теперь допускается явно.
  O12_RAN="$(grep -cE '^test o12_switch_during_inflight_pump[A-Za-z0-9_]* \.\.\. ok$' "${O12LOG}" || true)"
  if [ "${O12_RAN}" -ge 1 ]; then
    pass "O12 оракул на находку ИСПОЛНЕН и зелён (тестов: ${O12_RAN})"
  else
    fail "O12 прогон вернул 0, но оракул НЕ ИСПОЛНЕН (исполнено ${O12_RAN}) — тест отсутствует либо отфильтрован"
    tail -5 "${O12LOG}" | sed 's/^/      ↳ /'
  fi
else
  fail "O12 оракул на находку не собирается или падает — задача 12 НЕ ЗАКРЫТА (R-086 §5 п.1)"
  grep -E '^error(\[|:)|^test .* FAILED' "${O12LOG}" | head -4 | sed 's/^/      ↳ /'
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений)"
  exit 1
fi
echo "VERDICT: PASS"
