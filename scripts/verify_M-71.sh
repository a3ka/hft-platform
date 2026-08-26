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
# Граница (предел / предел+1) — ОТДЕЛЬНЫМ файлом: он COMPILE-RED (ссылается на ещё не
# существующую `gateway::DEFAULT_MAX_RESPONSE_BYTES`), и в общем наборе ронял бы компиляцию,
# лишая возможности предъявить остальные оракулы КРАСНЫМИ, а не «не собралось».
chk cargo test -p gateway --test red_egress_cap_boundary --quiet

# Состав НАЗВАН ЛИТЕРАЛОМ, а не `-ge`: порог, отстающий от набора, есть ослабление наблюдения
# ОТСУТСТВИЯ — потеря оракула оставила бы шаг зелёным (класс `R-118` N-1, `TD-140`).
EXPECT_A=9
N_A=$(grep -cE '^fn pl_i_[45]_' crates/gateway/tests/red_egress_cap.rs || echo 0)
if [ "${N_A}" -eq "${EXPECT_A}" ]; then
  echo "PASS: A состав набора — ${N_A} оракулов (ожидалось ровно ${EXPECT_A}: A A-2 B C F E E-2 E-3 E-4)"
else
  echo "FAIL: A состав набора — ${N_A} при ожидаемых ${EXPECT_A}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "A2 (уровень 2, A-021) — предел судит ПОЛНЫЙ исходящий текст в обеих wire-формах"
chk cargo test -p gateway-serve --test red_egress_cap_wire --quiet
EXPECT_W=7
N_W=$(grep -cE "^async fn pl_i_5_w|^fn pl_i_5_w" crates/gateway-serve/tests/red_egress_cap_wire.rs || true); N_W=${N_W:-0}
if [ "${N_W}" -eq "${EXPECT_W}" ]; then
  echo "PASS: A2 состав набора — ${N_W} оракулов (ожидалось ровно ${EXPECT_W}: W1 W2 W3 W4 W5 W-C1 W-C3)"
else
  echo "FAIL: A2 состав набора — ${N_W} при ожидаемых ${EXPECT_W}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "A3 (A-021 Правка B) — перечень дверей проверяет МАШИНА, а не память автора"
# Два круга подряд находили дверь, которую оракул не звал (C-157 R2 — живой путь; C-158 R1 —
# serve-обёртки и v1-конверт). Проба инвентаризирует поверхность грепом и падает, если дверь
# существует, а оракул её не зовёт. Именованный остаток (макро/трейт-двери) — COGNITIVE-ONLY.
chk bash scripts/tests/red_egress_doors.sh

step "A4 (задачи 8,9 — R-133 B-2/B-3, N-3) — вердикт путей совпадает, флаг провенанса цел, принятый ответ полон"
chk cargo test -p gateway --test red_egress_cap_paths --quiet
EXPECT_P=4
N_P=$(grep -cE "^fn pl_i_5_p" crates/gateway/tests/red_egress_cap_paths.rs || true); N_P=${N_P:-0}
if [ "${N_P}" -eq "${EXPECT_P}" ]; then
  echo "PASS: A4 состав набора — ${N_P} оракулов (ожидалось ровно ${EXPECT_P}: P-C1 P1 P2 P3)"
else
  echo "FAIL: A4 состав набора — ${N_P} при ожидаемых ${EXPECT_P}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "A5 (задача 11 — R-133 B-4) — многобайтовый venue не роняет обработчик"
chk cargo test -p gateway-serve --test red_egress_cap_utf8 --quiet

step "A6 (задача 10 — R-133 B-1) — заданный предел УПРАВЛЯЕТ, а не только разбирается"
chk cargo test -p gateway-serve --test red_egress_cap_governed --quiet

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

step "C — мутационный контроль: БАЗА зелена, мутация её роняет, честная нагрузка цела"
# `C-157` R4 — находка, ради которой шаг переписан. Прежняя редакция требовала только
# «набор красен под мутацией» и печатала PASS, когда критик вставил ТОЧНЫЙ якорь в функцию,
# которую никто не вызывает: четыре оракула предмета были красны И ДО, И ПОСЛЕ мутации, и шаг
# наблюдал их ПРЕЖНЮЮ красноту, а не нейтрализацию предела. Мёртвый якорь давал зелёный шаг.
#
# Развязка — ТРИ условия вместо одного, и первое решающее:
#   (i)   БЕЗ мутации набор ЗЕЛЁН           — иначе судить нечего: краснота уже есть, и мутация
#                                             ничего не доказывает (ровно дефект R4);
#   (ii)  С мутацией набор КРАСЕН           — предел действительно нейтрализован;
#   (iii) С мутацией оракул E ЗЕЛЁН         — мутация бьёт в ДЕФЕКТ, а не роняет всё подряд.
#
# На плановом этапе (i) не выполняется по построению — реализации ещё нет, — и шаг обязан быть
# КРАСНЫМ с этой формулировкой, а не «условно пройденным». После реализации мёртвый якорь
# провалит (ii): нейтрализовать нечего, набор останется зелёным.
#
# Якорь ЗАФИКСИРОВАН СПЕКОЙ (§5.2):
#   // MUT-ANCHOR M-71-LIMIT
#   fn enforce_response_limit(series: &SeriesBundle, limit: usize) -> io::Result<()>
# Сигнатура принимает ПОСТРОЕННЫЙ ОТВЕТ целиком, а не число ячеек: ограничивается полный
# ресурс (`C-157` R1), и мутация приложима к той же точке, через которую он и считается.
MUT=$(mktemp -d "${TMPDIR:-/tmp}/red-m71-mut-XXXXXX")
trap 'rm -rf "${MUT}"' EXIT
ANCHOR='MUT-ANCHOR M-71-LIMIT'

BASE_GREEN=0
cargo test -p gateway --test red_egress_cap --quiet >/dev/null 2>&1 || BASE_GREEN=1
cargo test -p gateway --test red_egress_cap_boundary --quiet >/dev/null 2>&1 || BASE_GREEN=1
# `A-021` п.6: subject-set мутационного шага расширен на уровень 2 — иначе мутация судила бы
# половину конструкции.
cargo test -p gateway-serve --test red_egress_cap_wire --quiet >/dev/null 2>&1 || BASE_GREEN=1

if [ "${BASE_GREEN}" -ne 0 ]; then
  echo "FAIL: C НЕ ГОТОВ — набор КРАСЕН и без мутации, судить нейтрализацию предела не по чему."
  echo "      Это ожидаемое состояние ДО реализации. Шаг станет осмысленным ровно тогда, когда"
  echo "      база позеленеет; до тех пор он ОБЯЗАН быть красным (C-157 R4: прежняя редакция"
  echo "      печатала здесь PASS, наблюдая ПРЕЖНЮЮ красноту, и мёртвый якорь её удовлетворял)."
  FAIL=$((FAIL + 1))
elif ! grep -q "${ANCHOR}" crates/gateway/src/lib.rs 2>/dev/null; then
  echo "FAIL: C SETUP НЕ СОСТОЯЛСЯ — якоря мутации '${ANCHOR}' в реализации НЕТ"
  FAIL=$((FAIL + 1))
elif ! cp -a crates Cargo.toml Cargo.lock "${MUT}/" 2>/dev/null; then
  echo "FAIL: C SETUP НЕ СОСТОЯЛСЯ — копия дерева не собралась"
  FAIL=$((FAIL + 1))
else
  perl -0pi -e 's/(MUT-ANCHOR M-71-LIMIT.*?\n\s*fn enforce_response_limit\([^\n]*\{\n)/$1        return Ok(());\n/s' \
    "${MUT}/crates/gateway/src/lib.rs"
  if ! grep -q 'return Ok(());' "${MUT}/crates/gateway/src/lib.rs"; then
    echo "FAIL: C SETUP НЕ СОСТОЯЛСЯ — мутация не внесена (сигнатура разошлась со спекой §5.2)"
    FAIL=$((FAIL + 1))
  else
    MUT_ALL=0; MUT_E=0
    (cd "${MUT}" && cargo test -p gateway --test red_egress_cap --quiet >/dev/null 2>&1) || MUT_ALL=1
    (cd "${MUT}" && cargo test -p gateway-serve --test red_egress_cap_wire --quiet >/dev/null 2>&1) || MUT_ALL=1
    (cd "${MUT}" && cargo test -p gateway --test red_egress_cap pl_i_5_e --quiet >/dev/null 2>&1) || MUT_E=1
    if [ "${MUT_ALL}" -eq 1 ] && [ "${MUT_E}" -eq 0 ]; then
      echo "PASS: C база зелена, мутация роняет набор, честная нагрузка (E) цела"
    else
      echo "FAIL: C мутация дала all_red=${MUT_ALL} e_red=${MUT_E}; требуется all_red=1 e_red=0"
      echo "      all_red=0 при зелёной базе ⇒ якорь МЁРТВ: нейтрализовать нечего (C-157 R4);"
      echo "      e_red=1 ⇒ мутация роняет и честную нагрузку, то есть не привязана к дефекту"
      FAIL=$((FAIL + 1))
    fi
  fi
fi

step "D (задача 4) — предел ДОСТАВЛЕН: ручка объявлена оператору там же, где остальные"
# `gates.md` §4 DoD: механизм на несущем пути мержится только подключённым. Все ручки сервиса
# объявлены в compose с дефолтом (`GATEWAY_WINDOW_MS:139`, `GATEWAY_MAX_SUBSCRIPTIONS:145`);
# предел, не видимый оператору, — построено-не-подключено.
chk_sh "grep -q 'GATEWAY_MAX_RESPONSE_BYTES' docker-compose.yml" \
       "D GATEWAY_MAX_RESPONSE_BYTES объявлен в docker-compose.yml"

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
