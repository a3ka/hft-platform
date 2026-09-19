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

# Состав наборов НАЗВАН ЛИТЕРАЛОМ, а не `-ge`: порог, отстающий от набора, есть ослабление
# наблюдения ОТСУТСТВИЯ — потеря оракула оставила бы шаг зелёным (класс `R-118` N-1, `TD-140`).
step "E — состав наборов (потеря оракула обязана быть ВИДНА)"
EXPECT_A=8
N_A=$(grep -cE '^fn v[0-9]' crates/gateway/tests/red_vp_bin_width.rs || echo 0)
if [ "${N_A}" -eq "${EXPECT_A}" ]; then
  echo "PASS: red_vp_bin_width.rs несёт ${N_A} оракулов (ожидается ${EXPECT_A})"
else
  echo "FAIL: red_vp_bin_width.rs несёт ${N_A} оракулов, ожидается ${EXPECT_A}"
  FAIL=$((FAIL + 1))
fi
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
chk_sh "! grep -qE 'GATEWAY_VP_BIN_WIDTH_E8:-(0|1)\\}' docker-compose.yml" \
  "дефолт ручки НЕ выключает сетку (не 0 и не 1 e8 = тик)"

step "G (задача 6) — VP-I-4 приведён к факту: корзина есть ДИАПАЗОН, а не точка"
chk_sh "grep -q 'диапазон' docs/fa/viz-backend.md" \
  "docs/fa/viz-backend.md описывает корзину профиля как диапазон"

step "H — МУТАЦИОННЫЙ КОНТРОЛЬ: нейтрализация сетки обязана ронять B и A, но НЕ V6"
# Изолированная копия с ПИННУТЫМ тулчейном. `rust-toolchain.toml` копируется обязательно:
# без него копия резолвит cargo по системному дефолту и мутация судится ДРУГИМ компилятором,
# чем прод-гейт (`C-171`, класс `TD-035`). Сравнивается РЕЗУЛЬТАТ (`cargo --version`), а не
# наличие файла: файл не доказывает, что тулчейн взвёлся.
MUT=$(mktemp -d)
cp -a crates Cargo.toml Cargo.lock rust-toolchain.toml "${MUT}/" 2>/dev/null
V_REPO=$(cargo --version 2>/dev/null)
V_MUT=$(cd "${MUT}" && cargo --version 2>/dev/null)
if [ -n "${V_REPO}" ] && [ "${V_REPO}" = "${V_MUT}" ]; then
  echo "PASS: тулчейн копии совпал с репозиторием (${V_MUT})"
else
  echo "FAIL: тулчейн копии (${V_MUT}) != репозиторий (${V_REPO}) — мутация судила бы другим компилятором"
  FAIL=$((FAIL + 1))
fi

# Нейтрализация: ширина корзины принудительно = 1 e8 (тик) — сетки как будто нет.
MUT_SRC="${MUT}/gateway/src/lib.rs"
if [ -f "${MUT_SRC}" ] && grep -q 'DEFAULT_VP_BIN_WIDTH_E8' "${MUT_SRC}"; then
  sed -i 's/pub const DEFAULT_VP_BIN_WIDTH_E8: i64 = [0-9_]*;/pub const DEFAULT_VP_BIN_WIDTH_E8: i64 = 1;/' "${MUT_SRC}"
  if (cd "${MUT}" && cargo test -p gateway --test red_vp_bin_width_size --quiet >/dev/null 2>&1); then
    echo "FAIL: мутация (сетка = тик) НЕ уронила оракул размера — он не пиннит предмет"
    FAIL=$((FAIL + 1))
  else
    echo "PASS: мутация (сетка = тик) уронила оракул размера, как и обязана"
  fi
  if (cd "${MUT}" && cargo test -p gateway --test red_vp_bin_width --quiet >/dev/null 2>&1); then
    echo "FAIL: мутация (сетка = тик) НЕ уронила оракулы сетки"
    FAIL=$((FAIL + 1))
  else
    echo "PASS: мутация (сетка = тик) уронила оракулы сетки"
  fi
else
  echo "FAIL: точка мутации не найдена — DEFAULT_VP_BIN_WIDTH_E8 отсутствует (набор ещё RED)"
  FAIL=$((FAIL + 1))
fi
rm -rf "${MUT}"

step "I — ЗОНА: запретный список §4 спеки соблюдён диапазоном"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/contracts | grep -q . && exit 1 || exit 0" \
  "crates/contracts не тронут (T1 не меняется)"
chk_sh "git diff ${BASE}..HEAD -- crates/gateway/src/lib.rs | grep -qE '^[+-].*GATEWAY_SCHEMA_VERSION' && exit 1 || exit 0" \
  "GATEWAY_SCHEMA_VERSION не бампнут (форма выдачи не меняется)"
chk_sh "git diff ${BASE}..HEAD -- crates/gateway/src/lib.rs | grep -qE '^[+-].*fn selector_fingerprint' && exit 1 || exit 0" \
  "selector_fingerprint не тронут (слепки не инвалидируются)"
chk_sh "git diff ${BASE}..HEAD -- docker-compose.yml | grep -qE '^[+-].*GATEWAY_BANDS' && exit 1 || exit 0" \
  "GATEWAY_BANDS не тронут (состав выдачи — граница C, чужой предмет)"
chk_sh "git diff ${BASE}..HEAD -- crates/gateway/src/lib.rs | grep -qE '^[+-].*DEFAULT_MAX_RESPONSE_BYTES: usize' && exit 1 || exit 0" \
  "подписанный предел П-020 не сдвинут"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/book crates/venue-binance crates/venue-binance-futures crates/journal | grep -q . && exit 1 || exit 0" \
  "книга, venue-адаптеры и журнал не тронуты"
chk_sh "git diff --name-only ${BASE}..HEAD -- TECH-DEBT.md PROJECT-STATE.md | grep -q . && exit 1 || exit 0" \
  "reviewer-owned файлы не тронуты"

if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
