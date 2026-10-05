#!/usr/bin/env bash
# Acceptance-гейт M-86 — профиль объёма отдаётся на ФИКСИРОВАННОЙ ценовой сетке.
#
# Предмет — ЗАМЕР, не память (`docs/plans/vp-frame-size-measurement-2026-09-18.md`, прод
# 2026-09-18T21:05Z, снято прод-формой вызова):
#   кадр 5 533 287 Б против подписанного предела 2 000 000 Б (`П-020`) ⇒ выдача отвергается
#   fail-closed, клиент получает НОЛЬ байт;
#   volume_profile — 4 836 478 Б = 87.41 % кадра, 211 381 корзина = одна торгованная цена;
#   depth_series — 3 619 Б = 0.07 % (полосы глубины к аварии отношения НЕ имеют);
#   число корзин растёт внутри UTC-суток и обнуляется в 00:00 UTC.
#
# Решение по КОДУ ВОЗВРАТА, не по тексту вывода (`gates.md` §3).
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

step "A (задачи 2,3) — сетка: принадлежность, сохранение объёма, метрики, склейка"
chk cargo test -p gateway --test red_vp_bin_width --quiet

step "B (задача 2) — РАЗМЕР кадра на ПРОД-ФОРМЕ (отдельный бинарь: меняет процессный предел)"
chk cargo test -p gateway --test red_vp_bin_width_size --quiet

step "C (задача 1) — ручка доходит до РАЗБИЕНИЯ, а не только до глобала (класс R-133 B-1)"
chk cargo test -p gateway --test red_vp_bin_width_governed --quiet

step "D (задача 4) — политика конфигурации на СТАРТЕ"
chk cargo test -p gateway-serve --test red_vp_bin_width_startup --quiet

step "D2 (задача 4, C-229 R4) — МОСТ env → сетка (и отвергнутый старт не трогает значение)"
chk cargo test -p gateway-serve --test red_vp_bin_width_bridge --quiet

# Состав наборов НАЗВАН ЛИТЕРАЛОМ, а не `-ge`: порог, отстающий от набора, есть ослабление
# наблюдения ОТСУТСТВИЯ — потеря оракула оставила бы шаг зелёным (класс `R-118` N-1, `TD-140`).
step "E — ИМЕНОВАННЫЙ состав обязательств (потеря оракула обязана быть ВИДНА)"
# `C-229` R3: счёт функций по шаблону `^fn v[0-9]` пропустил ОТСУТСТВИЕ названного `V7` —
# таблица спеки его требовала, набор не содержал, счёт сходился. Считать число мало:
# проверяется присутствие КАЖДОГО обязательства поимённо.
for ORACLE in v1_ v1b v2_ v2b v3_ v4_ v6_ v6b v6c v7_ v8_ v8b; do
  if grep -rqE "(fn ${ORACLE}|V1b НАРУШЕН|${ORACLE^^})" crates/gateway/tests/red_vp_bin_width.rs crates/gateway/tests/red_vp_bin_width_size.rs; then
    echo "PASS: обязательство ${ORACLE} предъявлено"
  else
    echo "FAIL: обязательство ${ORACLE} НЕ предъявлено ни одним оракулом"
    FAIL=$((FAIL + 1))
  fi
done
EXPECT_D=10
N_D=$(grep -c '#\[test\]' crates/gateway-serve/tests/red_vp_bin_width_startup.rs || echo 0)
if [ "${N_D}" -eq "${EXPECT_D}" ]; then
  echo "PASS: red_vp_bin_width_startup.rs несёт ${N_D} тестов (ожидается ${EXPECT_D})"
else
  echo "FAIL: red_vp_bin_width_startup.rs несёт ${N_D} тестов, ожидается ${EXPECT_D}"
  FAIL=$((FAIL + 1))
fi

step "F (задача 5) — ручка ДОСТАВЛЕНА оператору с РАБОЧИМ дефолтом"
# Файл в репозитории при деплое, который его не устанавливает, инертен (`testing.md`
# §«Механизм несущего пути»). Дефолт обязан быть рабочим, а не выключенным (§2.4 спеки):
# `П-014` п.4 подписана 2026-08-17 и не исполнена до сих пор именно потому, что требовала
# строки оператора.
chk_sh "grep -qE 'GATEWAY_VP_BIN_WIDTH_E8: \\\$\\{GATEWAY_VP_BIN_WIDTH_E8:-[0-9]+\\}' docker-compose.yml" \
  "docker-compose.yml объявляет GATEWAY_VP_BIN_WIDTH_E8 с дефолтом"
# ОТРИЦАНИЕ ГРЕПА ЗЕЛЕНО НА ПУСТОМ МЕСТЕ — поймано базовой линией 2026-09-18: при полном
# отсутствии ручки `! grep -q` давал PASS. Проверка обязана НАБЛЮДАТЬ ОТСУТСТВИЕ
# (`testing.md` §«Целостность гейта» св. 4), поэтому значение ИЗВЛЕКАЕТСЯ и сравнивается.
# `C-229` R5.2: «любой дефолт > 1» пропускал `${GATEWAY_VP_BIN_WIDTH_E8:-2}` — строка,
# практически выключающую огрубление и возвращающую аварию. Подписанная величина ОДНА
# (§2.1 спеки), поэтому сверяется ТОЧНЫЙ литерал, а не диапазон.
EXPECT_W=25000000
VP_DEF=$(grep -oE 'GATEWAY_VP_BIN_WIDTH_E8:-[0-9]+' docker-compose.yml 2>/dev/null | head -1 | sed 's/.*-//')
if [ "${VP_DEF:-}" = "${EXPECT_W}" ]; then
  echo "PASS: дефолт ручки равен подписанному литералу (${EXPECT_W} e8 = 0.25 USD)"
else
  echo "FAIL: дефолт ручки = ${VP_DEF:-<нет строки>}, ожидается РОВНО ${EXPECT_W}"
  FAIL=$((FAIL + 1))
fi

step "G (задача 6) — VP-I-4 приведён к факту: корзина есть ДИАПАЗОН, а не точка"
# ПОЙМАНО БАЗОВОЙ ЛИНИЕЙ: `grep -q 'диапазон'` был зелен по ЧУЖОЙ строке — единственное
# вхождение слова в файле относится к «диапазон 1.5–60 %» (полосы глубины, `:186`), а не к
# корзинам профиля. Проверка по слову — описание намерения, а не проверка. Требуется якорь,
# привязанный К ПРЕДМЕТУ: идентификатор инварианта обязан присутствовать в FA (сегодня его
# там нет вовсе — он живёт только в `docs/archive/M-24-volume-profile.md`) И нести рядом
# слово «диапазон».
chk_sh "grep -q 'VP-I-4' docs/fa/viz-backend.md" \
  "docs/fa/viz-backend.md вообще называет VP-I-4 (сегодня инвариант живёт только в архивной спеке)"
chk_sh "grep -qE 'VP-I-4.*(диапазон|ДИАПАЗОН)' docs/fa/viz-backend.md" \
  "VP-I-4 в FA описывает корзину профиля как ДИАПАЗОН, а не как точку"

step "H/H2 — МУТАЦИОННЫЙ КОНТРОЛЬ через scripts/lib/mutation_gate.sh (A-035 §2.3 M1)"
# ПЕРЕПИСАНО rev4 по `C-231` B3 и решению арбитра `A-035` §2.2.
# Прежняя форма `if (cd MUT && cargo test …); then FAIL else PASS` принимала ЛЮБОЙ
# ненулевой код за доказательство мутации — включая провал КОМПИЛЯЦИИ. Замер `A-035` Ф-3:
# `cargo test` возвращает 101 и на упавшем ассерте, и на несобравшемся крейте, поэтому
# «внимательному автору» тут нечего было читать. Успех обратной проверки доказывается
# ТРЕМЯ свидетелями (W0 baseline GREEN, W1 мутант собрался, W2 упал СВОИМ диагнозом);
# код возврата по-прежнему решает ОТКАЗ.
# shellcheck source=scripts/lib/mutation_gate.sh
. scripts/lib/mutation_gate.sh

# ── H — мутация СЕТКИ: ширина принудительно = 1 e8 (тик), то есть сетки как бы нет ──────
MUT=$(mktemp -d)
cp -a crates Cargo.toml Cargo.lock rust-toolchain.toml "${MUT}/" 2>/dev/null
V_REPO=$(cargo --version 2>/dev/null)
V_MUT=$(cd "${MUT}" && cargo --version 2>/dev/null)
if [ -z "${V_REPO}" ] || [ "${V_REPO}" != "${V_MUT}" ]; then
  echo "FAIL: H тулчейн копии (${V_MUT}) != репозиторий (${V_REPO}) — мутация судила бы другим компилятором"
  FAIL=$((FAIL + 1))
else
  echo "PASS: H тулчейн копии совпал с репозиторием (${V_MUT})"
fi
MUT_SRC="${MUT}/crates/gateway/src/lib.rs"
if [ ! -f "${MUT_SRC}" ] || ! grep -q 'DEFAULT_VP_BIN_WIDTH_E8' "${MUT_SRC}"; then
  echo "FAIL: H SETUP — точка мутации не найдена (DEFAULT_VP_BIN_WIDTH_E8 отсутствует: набор ещё plan-time RED)"
  FAIL=$((FAIL + 1))
else
  sed -i 's/pub const DEFAULT_VP_BIN_WIDTH_E8: i64 = [0-9_]*;/pub const DEFAULT_VP_BIN_WIDTH_E8: i64 = 1;/' "${MUT_SRC}"
  if ! grep -q 'DEFAULT_VP_BIN_WIDTH_E8: i64 = 1;' "${MUT_SRC}"; then
    echo "FAIL: H SETUP НЕ СОСТОЯЛСЯ — мутация не внесена (сигнатура константы разошлась со спекой §2.1)"
    FAIL=$((FAIL + 1))
  else
    echo "--- H.1: V1 обязан упасть ПО РАЗМЕРУ кадра ---"
    if mutant_must_fail "${MUT}" gateway red_vp_bin_width_size \
         v1_production_shaped_frame_fits_signed_limit 'V1 НАРУШЕН: кадр прод-формы весит'; then
      echo "PASS: H.1 мутация (сетка = тик) уронила V1 своим диагнозом"
    else
      echo "FAIL: H.1 мутация (сетка = тик) НЕ доказана тремя свидетелями"
      FAIL=$((FAIL + 1))
    fi
    echo "--- H.2: V2 обязан упасть, а V6 обязан ОСТАТЬСЯ ЗЕЛЁНЫМ (он же свидетель сборки) ---"
    if mutant_must_fail "${MUT}" gateway red_vp_bin_width \
         v2_every_emitted_price_is_on_the_grid 'V2 НАРУШЕН' \
         v6_grid_aligned_input_passes_through_unchanged; then
      echo "PASS: H.2 V2 упал своим диагнозом, V6 остался зелёным"
    else
      echo "FAIL: H.2 не сошлось: либо V2 упал не своим диагнозом, либо V6 не пережил мутацию"
      FAIL=$((FAIL + 1))
    fi
  fi
fi
rm -rf "${MUT}"

# ── H2 — мутация ЭВИКЦИИ: whole-session drop отключён (`C-230` B2, `VB-I-10`) ───────────
MUT2=$(mktemp -d)
cp -a crates Cargo.toml Cargo.lock rust-toolchain.toml "${MUT2}/" 2>/dev/null
MUT2_SRC="${MUT2}/crates/gateway/src/lib.rs"
if [ ! -f "${MUT2_SRC}" ] || ! grep -q 'self.vp.bins.remove(&sid);' "${MUT2_SRC}"; then
  echo "FAIL: H2 SETUP — точка мутации не найдена: whole-session drop изменён, сторож ослеп"
  FAIL=$((FAIL + 1))
else
  sed -i 's|self\.vp\.bins\.remove(&sid);|/* MUT: whole-session drop отключён */|' "${MUT2_SRC}"
  if grep -q 'self.vp.bins.remove(&sid);' "${MUT2_SRC}"; then
    echo "FAIL: H2 SETUP НЕ СОСТОЯЛСЯ — мутация не внесена"
    FAIL=$((FAIL + 1))
  elif mutant_must_fail "${MUT2}" gateway red_vp_bin_width_size \
         v1_production_shaped_frame_fits_signed_limit 'VB-I-10.*в кадре 2 строк'; then
    echo "PASS: H2 мутация (эвикция отключена) уронила V1 диагнозом про две строки профиля"
  else
    echo "FAIL: H2 мутация (эвикция отключена) НЕ доказана тремя свидетелями"
    FAIL=$((FAIL + 1))
  fi
fi
rm -rf "${MUT2}"

step "I — ЗОНА: запретный список §4 спеки соблюдён диапазоном"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/contracts | grep -q . && exit 1 || exit 0" \
  "crates/contracts не тронут (T1 не меняется)"
chk_sh "git diff ${BASE}..HEAD -- crates/gateway/src/lib.rs | grep -qE '^[+-].*GATEWAY_SCHEMA_VERSION' && exit 1 || exit 0" \
  "GATEWAY_SCHEMA_VERSION не бампнут (форма выдачи не меняется)"
# `C-229` R5.1: греп по строке `fn selector_fingerprint` ловил только смену ОБЪЯВЛЕНИЯ.
# Добавление `effective_vp_bin_width_e8().hash(&mut h);` в ТЕЛО функции эту строку не
# трогает — сторож печатал PASS, пока все чекпоинты становились недействительными
# (имя слепка детерминировано отпечатком, `crates/gateway/src/lib.rs:3710-3731`).
# Сверяется ХЕШ ТЕЛА целиком.
FP_EXPECT=e99e808bbce3b2f98a24bc3317230e7896439f4e39616c8ee3e54cb2ec44e181
FP_BODY=$(sed -n '/pub fn selector_fingerprint/,/^    }$/p' crates/gateway/src/lib.rs)
FP_GOT=$(printf '%s\n' "${FP_BODY}" | sha256sum | cut -d' ' -f1)
FP_LINES=$(printf '%s\n' "${FP_BODY}" | wc -l)
if [ "${FP_LINES}" -lt 10 ]; then
  echo "FAIL: тело selector_fingerprint извлечено неверно (${FP_LINES} строк) — сторож слеп"
  FAIL=$((FAIL + 1))
elif [ "${FP_GOT}" = "${FP_EXPECT}" ]; then
  echo "PASS: тело selector_fingerprint не тронуто (sha256 ${FP_GOT})"
else
  echo "FAIL: тело selector_fingerprint ИЗМЕНЕНО (sha256 ${FP_GOT} != ${FP_EXPECT}) — \
отпечаток определяет имя слепка, значит все чекпоинты стали недействительны (§4 запрет 2)"
  FAIL=$((FAIL + 1))
fi
chk_sh "git diff ${BASE}..HEAD -- docker-compose.yml | grep -qE '^[+-].*GATEWAY_BANDS' && exit 1 || exit 0" \
  "GATEWAY_BANDS не тронут (состав выдачи — граница C, чужой предмет)"
chk_sh "git diff ${BASE}..HEAD -- crates/gateway/src/lib.rs | grep -qE '^[+-].*DEFAULT_MAX_RESPONSE_BYTES: usize' && exit 1 || exit 0" \
  "подписанный предел П-020 не сдвинут"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/book crates/venue-binance crates/venue-binance-futures crates/journal | grep -q . && exit 1 || exit 0" \
  "книга, venue-адаптеры и журнал не тронуты"
chk_sh "git diff --name-only ${BASE}..HEAD -- TECH-DEBT.md PROJECT-STATE.md | grep -q . && exit 1 || exit 0" \
  "reviewer-owned файлы не тронуты"

step "J (C-229 R5) — АНТИ-ПЛАЦЕБО САМИХ СТОРОЖЕЙ: они обязаны РЕАГИРОВАТЬ"
# Сторож, про который сказано «он краснеет», но не предъявлено — описание намерения.
# Оба новых сторожа проверяются мутацией их ВХОДА, без полной пересборки.

# J1 — внедряем hash ширины в ТЕЛО fingerprint (ровно то нарушение §4 запрет 2, которое
# прежняя редакция шага I пропускала) и убеждаемся, что хеш тела РАСХОДИТСЯ с эталоном.
J_TMP=$(mktemp)
sed -n '/pub fn selector_fingerprint/,/^    }$/p' crates/gateway/src/lib.rs \
  | sed 's/^        h.finish()$/        effective_vp_bin_width_e8().hash(\&mut h);\n        h.finish()/' > "${J_TMP}"
J_MUT_SHA=$(sha256sum "${J_TMP}" | cut -d' ' -f1)
if [ "${J_MUT_SHA}" != "${FP_EXPECT}" ] && [ "$(wc -l < "${J_TMP}")" -gt 10 ]; then
  echo "PASS: сторож тела fingerprint РАЗЛИЧАЕТ мутацию (внедрённый hash ширины меняет sha256)"
else
  echo "FAIL: мутация тела fingerprint НЕ отличается от эталона — сторож слеп к §4 запрету 2"
  FAIL=$((FAIL + 1))
fi
rm -f "${J_TMP}"

# J2 — подсовываем compose-дефолт `:-2` (он «> 1», то есть прежнюю проверку проходил) и
# убеждаемся, что сверка с ТОЧНЫМ литералом его отвергает.
J_DEF=$(printf 'GATEWAY_VP_BIN_WIDTH_E8: ${GATEWAY_VP_BIN_WIDTH_E8:-2}\n' \
  | grep -oE 'GATEWAY_VP_BIN_WIDTH_E8:-[0-9]+' | head -1 | sed 's/.*-//')
if [ "${J_DEF}" != "${EXPECT_W}" ]; then
  echo "PASS: сторож дефолта РАЗЛИЧАЕТ выключающее значение (:-2 отвергается литералом ${EXPECT_W})"
else
  echo "FAIL: сторож дефолта не отличил :-2 от подписанного ${EXPECT_W}"
  FAIL=$((FAIL + 1))
fi

if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
