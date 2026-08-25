#!/usr/bin/env bash
# Acceptance-гейт M-71 — PL-I-4/PL-I-5: предел объёма ответа, fail-closed.
#
# Предмет (замер, не память). Селектор приходит ОТ КЛИЕНТА (`gateway-serve/src/wire_v1.rs:5`,
# `parse_selector` `:120`); единственная проверка полосы — `0 < b < 1`
# (`gateway-serve/src/session.rs:79-82`); `gateway::validate_selector` о `bands` не знает вовсе
# (`crates/gateway/src/lib.rs:1878-1917`); окно heatmap = `max(selector.bands)` (`:1192`).
# Ограничения на РАЗМЕР ответа в проекте НЕТ:
#   grep -rniE 'max_frame|max_message|max_size|frame_size|max_send' crates/gateway-serve/src → 0
# Замер architect'а на геометрии прод-книги: bands=[0.001] → 100 ячеек, bands=[0.99] → 59 980
# (×600); на полном прод-окне — 62 КБ против 43 МБ (×722). Одно сообщение подписки, без смены
# прод-конфига. `DEFAULT_MAX_SUBSCRIPTIONS = 16` ограничивает ЧИСЛО подписок, а не размер.
#
# Это ПЕРВЫЕ оракулы `PL-I-4`/`PL-I-5` — инвариантов, объявленных в `DESIGN` §22 со статусом
# «будущие RED-оракулы, PENDING».
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }
chk_sh() { if bash -c "$1" >/dev/null 2>&1; then echo "PASS: $2"; else echo "FAIL: $2"; FAIL=$((FAIL + 1)); fi; }

BASE=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE}" ]; then
  echo "FAIL: merge-base с origin/main не вычислен — шаги диапазона судить не по чему"
  FAIL=$((FAIL + 1))
fi

step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk cargo fmt --all -- --check
chk cargo clippy --all-targets --all-features -- -D warnings
chk cargo test --all --quiet

step "A (задачи 1,2,3,5,6) — библиотечные оракулы предела"
chk cargo test -p gateway --test red_egress_cap --quiet

# Состав НАЗВАН ЛИТЕРАЛОМ, а не `-ge`: порог, отстающий от набора, есть ослабление наблюдения
# ОТСУТСТВИЯ — потеря оракула оставила бы шаг зелёным (класс `R-118` N-1, `TD-140`).
EXPECT_A=6
N_A=$(grep -cE '^fn pl_i_[45]_' crates/gateway/tests/red_egress_cap.rs || echo 0)
if [ "${N_A}" -eq "${EXPECT_A}" ]; then
  echo "PASS: A состав набора — ${N_A} оракулов (ожидалось ровно ${EXPECT_A})"
else
  echo "FAIL: A состав набора — ${N_A} при ожидаемых ${EXPECT_A}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "B (задача 4) — невалидный предел не даёт стартовать прод-бинарю"
chk cargo test -p gateway-serve --test red_egress_cap_startup --quiet

EXPECT_B=10
N_B=$(grep -cE '^fn [a-z_]+\(\) \{' crates/gateway-serve/tests/red_egress_cap_startup.rs || echo 0)
if [ "${N_B}" -eq "${EXPECT_B}" ]; then
  echo "PASS: B состав набора — ${N_B} оракулов (ожидалось ровно ${EXPECT_B}: 8 отказов + 2 vantage)"
else
  echo "FAIL: B состав набора — ${N_B} при ожидаемых ${EXPECT_B}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "C — мутационный контроль ИСПОЛНЯЕТСЯ: нейтрализация предела обязана ронять набор"
# Мутация вносится в КОПИЮ дерева и набор ПРОГОНЯЕТСЯ ТАМ (`branch-hygiene`: мутации в
# отдельном дереве). Требование ДВУСТОРОННЕЕ и в этом суть: под мутацией красными обязаны стать
# оракулы ПРЕДМЕТА, а анти-ложное-КРАСНОЕ `E` — остаться ЗЕЛЁНЫМ. Мутация, роняющая ВСЁ,
# доказывала бы лишь то, что тесты вообще реагируют на код.
#
# Якорь ЗАФИКСИРОВАН СПЕКОЙ (§5 задача 1): реализация обязана нести
#   // MUT-ANCHOR M-71-LIMIT
#   fn enforce_response_limit(cells: usize, limit: usize) -> io::Result<()>
# Без стабильного якоря мутацию нельзя ВНЕСТИ, и шаг выродился бы в греп (дефект, стоивший
# круга на M-68 — `C-138` п.2).
MUT=$(mktemp -d "${TMPDIR:-/tmp}/red-m71-mut-XXXXXX")
trap 'rm -rf "${MUT}"' EXIT
ANCHOR='MUT-ANCHOR M-71-LIMIT'
if ! grep -q "${ANCHOR}" crates/gateway/src/lib.rs 2>/dev/null; then
  echo "FAIL: C SETUP НЕ СОСТОЯЛСЯ — якоря мутации '${ANCHOR}' в реализации НЕТ."
  echo "      Спека M-71 §5 задача 1 требует его. До реализации это ожидаемо."
  FAIL=$((FAIL + 1))
elif ! cp -a crates Cargo.toml Cargo.lock "${MUT}/" 2>/dev/null; then
  echo "FAIL: C SETUP НЕ СОСТОЯЛСЯ — копия дерева не собралась"
  FAIL=$((FAIL + 1))
else
  perl -0pi -e 's/(MUT-ANCHOR M-71-LIMIT.*?\n\s*fn enforce_response_limit\([^\n]*\{\n)/$1        return Ok(());\n/s' \
    "${MUT}/crates/gateway/src/lib.rs"
  if ! grep -q 'return Ok(());' "${MUT}/crates/gateway/src/lib.rs"; then
    echo "FAIL: C SETUP НЕ СОСТОЯЛСЯ — мутация не внесена (сигнатура разошлась со спекой)"
    FAIL=$((FAIL + 1))
  else
    MUT_ALL=0; MUT_E=0
    (cd "${MUT}" && cargo test -p gateway --test red_egress_cap --quiet >/dev/null 2>&1) || MUT_ALL=1
    (cd "${MUT}" && cargo test -p gateway --test red_egress_cap pl_i_5_e --quiet >/dev/null 2>&1) || MUT_E=1
    if [ "${MUT_ALL}" -eq 1 ] && [ "${MUT_E}" -eq 0 ]; then
      echo "PASS: C набор КРАСЕН без предела, а анти-ложное-КРАСНОЕ E остаётся ЗЕЛЁНЫМ"
    else
      echo "FAIL: C мутация дала all_red=${MUT_ALL} e_red=${MUT_E}; требуется all_red=1 e_red=0"
      echo "      all_red=0 ⇒ набор не пиннит предел; e_red=1 ⇒ мутация роняет и честную нагрузку,"
      echo "      то есть проба доказывает лишь реакцию на код, а не привязку к дефекту"
      FAIL=$((FAIL + 1))
    fi
  fi
fi

step "D (задача 4) — предел ДОСТАВЛЕН: ручка объявлена оператору там же, где остальные"
# `gates.md` §4 DoD: механизм на несущем пути мержится только подключённым. Все ручки сервиса
# объявлены в compose с дефолтом (`GATEWAY_WINDOW_MS:139`, `GATEWAY_MAX_SUBSCRIPTIONS:145`);
# предел, не видимый оператору, — построено-не-подключено.
chk_sh "grep -q 'GATEWAY_MAX_RESPONSE_CELLS' docker-compose.yml" \
       "D GATEWAY_MAX_RESPONSE_CELLS объявлен в docker-compose.yml"

step "E — соседние инварианты не куплены"
chk cargo test -p gateway --test red_gateway_bounded --quiet
chk cargo test -p gateway --test red_snapshot_noclone --quiet
chk cargo test -p gateway --test red_gateway_live_eq_replay --quiet
chk cargo test -p gateway-serve --test red_max_subs_config --quiet
chk cargo test -p gateway-serve --test red_window_guard_startup --quiet

step "F — Block-C: contracts не тронуты предметом"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/contracts | grep -q . && exit 1 || exit 0" \
       "F crates/contracts не тронут"

step "G — состав ВЫДАЧИ не тронут: здесь ставится ПРЕДЕЛ, а не меняется состав"
chk_sh "git diff ${BASE}..HEAD -- docker-compose.yml | grep -qE '^[+-].*GATEWAY_BANDS' && exit 1 || exit 0" \
       "G GATEWAY_BANDS не тронут (граница C, предмет M-70)"

step "H — зона предмета: чужие крейты в диапазоне не участвуют"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/book crates/venue-binance crates/venue-binance-futures crates/journal crates/contracts | grep -q . && exit 1 || exit 0" \
       "H book/venue/journal не тронуты диапазоном"

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
