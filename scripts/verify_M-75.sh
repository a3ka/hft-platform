#!/usr/bin/env bash
# Acceptance-гейт M-75 — расцепление окна heatmap/COB от `Selector.bands`.
#
# ГЕЙТ НАПИСАН ДО РАБОТЫ И ОБЯЗАН БЫТЬ КРАСНЫМ. Задачи 2-5 открыты; шаги на них краснеют по
# построению. Шаг, ставший зелёным РАНЬШЕ своей задачи, есть дефект гейта, и его надо чинить,
# а не радоваться.
#
# ЧЕМУ ЭТОТ ГЕЙТ НАУЧЕН ЦЕНОЙ СЕГОДНЯШНЕГО ДНЯ — четыре урока, каждый оплачен вердиктом:
#   1. `A-028` §3 п.5: `cargo test` возвращает 0 при НУЛЕ исполненных тестов. Шаг, решающий
#      по коду возврата, зеленеет ВАКУУМНО, стоит фильтру ничего не найти ⇒ `chk_named_test`.
#   2. `C-192` B-3: гейт `M-72` считал функции шаблоном, под который не подпадал его же новый
#      оракул, и мог стать зелёным без него ⇒ здесь состав набора сверяется ПОИМЁННО.
#   3. `C-192` B-3 (второе): шаг того гейта искал имена, которые автор САМ УДАЛИЛ кругом
#      раньше, и стал вечно-зелёным ни о чём ⇒ канарейка связки привязана к СТРУКТУРЕ
#      функции, а не к файлу, и несёт SETUP-GUARD на собственную применимость.
#   4. `C-187` B-4: шаг звал несуществующего помощника, и `command not found` не увеличивал
#      счётчик отказов ⇒ ниже стоит самопроверка ОБОИХ помощников.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { printf '\n── %s\n' "$*"; }
chk() {
  local name; name="$(printf '%s' "$1" | sed -n '1{s/^[[:space:]]*//;s/[[:space:]]*$//;p}')"
  [ -n "$name" ] || name="<многострочная проверка>"
  if ( eval "$1" ) >/dev/null 2>&1; then echo "PASS: ${name}"; else echo "FAIL: ${name}" >&2; FAIL=$((FAIL + 1)); fi
}

# ТРИ ИСХОДА, А НЕ ДВА (образец — `verify_M-72.sh`): «оракула нет» и «оракул есть, но не
# собрался» — разные состояния задачи, и одинаковый текст отправил бы читателя искать не то.
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

# ── САМОПРОВЕРКА ОБОИХ ПОМОЩНИКОВ ───────────────────────────────────────────────────────
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
T1=crates/gateway/tests/red_heatmap_window_decoupled.rs
T5=crates/gateway/tests/red_heatmap_window_server_owned.rs
T2=crates/gateway-serve/tests/red_heatmap_window_env.rs

step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk "cargo fmt --all -- --check"
chk "cargo clippy --all-targets --all-features -- -D warnings"
chk "cargo test --all --quiet"

step "task #1 (RED) — окно карты не зависит от полос; сторож против обнуления окна"
chk_named_test "оракул расцепления (H-1 · H-3 · H-4)" \
  cargo test -p gateway --test red_heatmap_window_decoupled --quiet
# СОСТАВ НАБОРА СВЕРЯЕТСЯ ПОИМЁННО, а не шаблоном имени: шаблон уже подводил на `M-72`,
# где новый оракул под него не подпал и гейт мог позеленеть без него (`C-192` B-3).
for t in hw_i_1_heatmap_size_is_independent_of_bands \
         hw_i_3_canonical_bands_fit_under_signed_cap \
         hw_i_4_decoupling_does_not_empty_the_heatmap; do
  chk "grep -q '^async fn ${t}\|^fn ${t}' ${T1}"
done

step "task #1b (RED) — окно ПРИНАДЛЕЖИТ СЕРВЕРУ: полоса ниже конфига не сужает карту"
# Закрывает `C-194` B-2. Отдельный шаг, а не расширение предыдущего: он судит ДРУГОЙ мир —
# зажатую связку `min(max(bands), CONFIG)`, против которой ВЕСЬ набор задачи 1 зелен (замер
# architect'а мутацией: 3 passed при живой связке). Правило `Р-4` (`A-029`).
chk_named_test "оракул серверного владения окном (H-5 · H-5b)" \
  cargo test -p gateway --test red_heatmap_window_server_owned --quiet
for t in hw_i_5_below_config_band_cannot_shrink_the_map \
         hw_i_5b_server_window_still_produces_a_map_for_a_below_config_band; do
  chk "grep -q '^fn ${t}' ${T5}"
done

step "task #2 — связка снята СТРУКТУРНО: окно не выводится из селектора внутри функции"
# Канарейка привязана к ТЕЛУ функции, а не к файлу: `selector.bands` законно встречается в
# других местах `lib.rs` (депт-серия — их законный потребитель), и греп по файлу целиком дал
# бы либо ложное красное, либо, после правки, вечно-зелёное ни о чём.
#
# SETUP-GUARD на собственную применимость: если функция переименована или исчезла, извлечение
# тела даст пустоту, и проверка «в теле нет bands» станет истинной ВАКУУМНО. Поэтому сперва
# предъявляется, что тело НЕПУСТО.
BODY="$(awk '/^fn build_heatmap_and_cob\(/{f=1} f{print} f&&/^}$/{exit}' "${LIB}" 2>/dev/null)"
if [ "$(printf '%s' "${BODY}" | wc -l)" -lt 5 ]; then
  echo "FAIL: тело build_heatmap_and_cob не извлечено (функция переименована или удалена) — канарейка связки была бы ВАКУУМНО зелёной" >&2
  FAIL=$((FAIL + 1))
else
  chk "printf '%s' \"\${BODY}\" | grep -q 'window_frac'"
  chk "! printf '%s' \"\${BODY}\" | grep -q 'selector.bands'"
fi
chk "grep -q 'pub fn effective_heatmap_window_frac' ${LIB}"
chk "grep -q 'DEFAULT_HEATMAP_WINDOW_FRAC' ${LIB}"

# ── КАНАРЕЙКА МЕСТА ВЫЗОВА (`C-194` B-2, дословно: «pin the call-site/property that supplies
# that effective value, not only the callee body»).
#
# Тела функции НЕ ДОСТАТОЧНО, и это доказано мутацией, а не предположено: зажатая связка
# `min(max(selector.bands), CONFIG)`, посчитанная У ВЫЗЫВАЮЩЕГО и переданная в суженную
# сигнатуру, оставляет тело чистым от `selector.bands` — все структурные проверки выше зелены,
# а окном по-прежнему управляет клиент. Гейт обязан смотреть на ТОГО, КТО ПОСТАВЛЯЕТ значение.
#
# Предел назван честно: это структурная проверка, и её обходит сдвиг вычисления на уровень
# выше (`M-45` §D-1 — тот же класс, уже стоивший двух REJECT). Она — не доказательство, а
# дешёвый страж; доказательство несёт RED-оракул H-5 (шаг task #1b), а полное закрытие —
# задача 5b, оракул смены серверной настройки.
CALLS="$(grep -n 'build_heatmap_and_cob(' "${LIB}" 2>/dev/null | grep -v '///' | grep -vE '^[0-9]+:[[:space:]]*fn ')"
NCALLS="$(printf '%s\n' "${CALLS}" | grep -c . || true)"
if [ "${NCALLS:-0}" -lt 1 ]; then
  echo "FAIL: место вызова build_heatmap_and_cob не найдено — функция переименована, встроена или удалена; канарейка call-site была бы ВАКУУМНО зелёной" >&2
  FAIL=$((FAIL + 1))
else
  chk "printf '%s\n' \"\${CALLS}\" | grep -q 'effective_heatmap_window_frac'"
  chk "printf '%s\n' \"\${CALLS}\" | grep -qE 'selector|bands' && exit 1 || exit 0"
fi

step "task #3 — разбор env FAIL-CLOSED: невалидное значение = отказ старта, не дефолт"
chk_named_test "оракул fail-closed разбора GATEWAY_HEATMAP_WINDOW" \
  cargo test -p gateway-serve --test red_heatmap_window_env --quiet
# Состав ПОИМЁННО (урок 2 шапки): обе половины ядра и оба парных vantage'а обязаны
# существовать. Без vantage'ей требование удовлетворяется реализацией «всегда Err», то есть
# ценой неработающего сервиса; без композиции — ручкой, не доехавшей до деплоя.
for t in malformed_heatmap_window_is_rejected \
         out_of_range_heatmap_window_is_rejected \
         valid_heatmap_window_starts \
         absent_heatmap_window_starts \
         heatmap_window_is_declared_in_compose; do
  chk "grep -q '^fn ${t}' ${T2}"
done

step "task #4 — переменная объявлена в конфиге и РАВНА сегодняшнему эффективному"
chk "grep -q 'GATEWAY_HEATMAP_WINDOW' docker-compose.yml"
chk "grep -qE 'GATEWAY_HEATMAP_WINDOW.*0\\.001' docker-compose.yml"

step "C — границы: схема не бампнута, протокол и T1 не тронуты"
BASE=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE}" ]; then
  echo "FAIL: merge-base с origin/main не вычислен — шаги диапазона судить не по чему" >&2
  FAIL=$((FAIL + 1))
else
  chk "git diff ${BASE}..HEAD -- ${LIB} | grep -qE '^[+-].*GATEWAY_SCHEMA_VERSION' && exit 1 || exit 0"
  chk "git diff --name-only ${BASE}..HEAD -- crates/contracts docs/rfc | grep -q . && exit 1 || exit 0"
  chk "git diff ${BASE}..HEAD -- docker-compose.yml | grep -qE '^[+-].*GATEWAY_BANDS' && exit 1 || exit 0"
fi

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
