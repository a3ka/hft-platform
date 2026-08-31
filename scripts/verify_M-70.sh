#!/usr/bin/env bash
# Acceptance-гейт M-70 — включение полос глубины: правдивость метки прежде состава.
#
# ГЕЙТ НАПИСАН ДО РАБОТЫ И ОБЯЗАН БЫТЬ КРАСНЫМ. Шаг, ставший зелёным РАНЬШЕ своей задачи, —
# дефект гейта, и чинить надо гейт, а не радоваться.
#
# ЧЕМУ НАУЧЕН ЦЕНОЙ ЭТОЙ СЕССИИ — три урока, каждый оплачен вердиктом:
#   1. `A-028` §3 п.5: `cargo test` возвращает 0 при НУЛЕ исполненных тестов ⇒ шаг именованного
#      оракула решает по ЧИСЛУ ИСПОЛНЕННЫХ тестов, а не по коду возврата (`chk_named_test`).
#   2. `C-192` B-3: состав набора сверялся ШАБЛОНОМ имени, под который новый оракул не подпал,
#      и гейт мог позеленеть без него ⇒ здесь состав сверяется ПОИМЁННО.
#   3. `C-193` (этот милестоун): гейта не существовало вовсе, и REJECT был именно за это.
#
# ЗАВИСИМОСТЬ ОТ `M-75` НАЗВАНА ЗДЕСЬ, А НЕ ПОДРАЗУМЕВАЕТСЯ (спека §0.3quater): задачи 0 и 3
# меряют объём и обход предела через клиентские полосы, а `M-75` расцепляет окно карты от
# полос и меняет эти величины на два порядка. Их шаги стоят ПОСЛЕ шага-предусловия и красны,
# пока `M-75` не влит: замер, снятый раньше, пришлось бы выбросить.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { printf '\n── %s\n' "$*"; }
chk() {
  local name; name="$(printf '%s' "$1" | sed -n '1{s/^[[:space:]]*//;s/[[:space:]]*$//;p}')"
  [ -n "$name" ] || name="<многострочная проверка>"
  if ( eval "$1" ) >/dev/null 2>&1; then echo "PASS: ${name}"; else echo "FAIL: ${name}" >&2; FAIL=$((FAIL + 1)); fi
}

# ТРИ ИСХОДА, А НЕ ДВА: «оракула нет» и «оракул есть, но не собрался» — разные состояния
# задачи, и одинаковый текст отправил бы читателя искать не то.
chk_named_test() { # $1=имя шага, далее — команда cargo
  local name="$1"; shift
  local out st ran
  out="$("$@" 2>&1)"; st=$?
  ran=$(printf '%s\n' "${out}" | awk '/^test result:/ { p += $4; f += $6 } END { print p + f + 0 }')
  if [ "${ran:-0}" -eq 0 ]; then
    if printf '%s\n' "${out}" | grep -qE 'could not compile|^error\[E[0-9]'; then
      echo "FAIL: ${name} — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED): $(printf '%s\n' "${out}" | grep -m1 -E '^error' | cut -c1-100)" >&2
    else
      echo "FAIL: ${name} — НИ ОДИН тест не исполнился: фильтр не нашёл оракула. Зелёное здесь означало бы ВАКУУМ" >&2
    fi
    FAIL=$((FAIL + 1)); return
  fi
  if [ ${st} -eq 0 ]; then echo "PASS: ${name} (исполнено тестов: ${ran})"
  else echo "FAIL: ${name} (исполнено тестов: ${ran}, exit=${st})" >&2; FAIL=$((FAIL + 1)); fi
}

# ── САМОПРОВЕРКА ОБОИХ ПОМОЩНИКОВ (урок `C-187` B-4: шаг звал несуществующего помощника,
#    и `command not found` не увеличивал счётчик отказов) ─────────────────────────────────
_probe=0
chk "true"  >/dev/null 2>&1 || _probe=1
_before=${FAIL}
chk "false" >/dev/null 2>&1
_after_chk=${FAIL}
chk_named_test "самопроверка вакуума" cargo test -p gateway --test нет-такого-таргета --quiet >/dev/null 2>&1
if [ "${_after_chk}" -ne $((_before + 1)) ] || [ "${FAIL}" -ne $((_before + 2)) ] || [ "${_probe}" -ne 0 ]; then
  echo "FAIL: самопроверка помощников — chk или chk_named_test не считают отказы; весь гейт был бы зелёным ни о чём" >&2
  echo "VERDICT: FAIL (1)"; exit 1
fi
FAIL=${_before}
echo "PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются"

LIB=crates/gateway/src/lib.rs
T4=crates/gateway/tests/red_depth_point_provenance.rs

step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk "cargo fmt --all -- --check"
chk "cargo clippy --all-targets --all-features -- -D warnings"
chk "cargo test --all --quiet"

step "ПРЕДУСЛОВИЕ — M-75 влит: без него задачи 0 и 3 меряют мир, которого не будет (§0.3quater)"
# Расцепление окна карты от полос меняет объём канонического набора на два порядка
# (7 882 335 Б → под 2 000 000 Б, замер `M-75` §3). Шаг красен, пока `M-75` не в `main`.
chk "git show origin/main:crates/gateway/src/lib.rs | grep -q 'effective_heatmap_window_frac'"

step "task #4 (RED) — TD-159: метка достоверности принадлежит ТОЧКЕ, а не ряду"
chk_named_test "оракул DB-I-4 (точка/ряд + анти-плацебо)" \
  cargo test -p gateway --test red_depth_point_provenance --quiet
# Состав ПОИМЁННО (урок 2 шапки): без анти-плацебо основной тест удовлетворяется реализацией,
# метящей `not-observed` ВСЁ подряд, — она обесценивает метку и формально зеленеет.
for t in db_i_4_two_points_of_one_row_carry_their_own_provenance \
         db_i_4b_reachable_point_is_not_falsely_downgraded; do
  chk "grep -q '^fn ${t}' ${T4}"
done

step "task #5 — TD-161: словарь метки ОДИН на всю выдачу"
# Сегодня heatmap ставит метку ТОЛЬКО по ширине (`deep_thr = mid * 0.013`, `prov_str =
# \"diff-reconstructed\"`), а депт-серия знает сторону и охват. Клиент видит четыре разные
# строки от двух путей в одном ответе. Унификация обязана идти ВВЕРХ — heatmap приводится к
# словарю депт-серии, не наоборот (спека §2.1: депт-серия знает больше).
chk "! grep -q 'let prov_str = \"diff-reconstructed\".to_string()' ${LIB}"
chk "grep -q 'depth_provenance_label' ${LIB}"

step "task #6 — bump GATEWAY_SCHEMA_VERSION: смена ФОРМЫ выдачи объявлена"
# Метка на точку меняет форму `DepthRow` ⇒ версия обязана сдвинуться (`VB-I-4`, прецеденты
# M-36/M-38a/M-48/M-68). Сверка с `M-72`: два милестоуна не двигают версию одновременно.
BASE=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE}" ]; then
  echo "FAIL: merge-base с origin/main не вычислен — шаги диапазона судить не по чему" >&2
  FAIL=$((FAIL + 1))
else
  chk "git diff ${BASE}..HEAD -- ${LIB} | grep -qE '^[+-].*GATEWAY_SCHEMA_VERSION'"
fi

step "task #7 — состав GATEWAY_BANDS канонический и ДОЕЗЖАЕТ до выдачи"
chk "grep -qE 'GATEWAY_BANDS.*0\\.015' docker-compose.yml"

step "task #8 — VB-I-10 не ослаблен: предел выдачи не куплен ценой предела памяти"
chk_named_test "существующие оракулы окна памяти" \
  cargo test -p gateway --test red_gateway_bounded --quiet

step "C — границы: T1 не тронут, состав ЗАПИСИ не тронут"
if [ -n "${BASE}" ]; then
  chk "git diff --name-only ${BASE}..HEAD -- crates/contracts docs/rfc | grep -q . && exit 1 || exit 0"
  chk "git diff --name-only ${BASE}..HEAD -- crates/venue-binance crates/venue-binance-futures crates/journal | grep -q . && exit 1 || exit 0"
fi

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
