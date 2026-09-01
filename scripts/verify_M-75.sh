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

# ═══ ПЕРЕЧЕНЬ СТРАЖЕЙ ПРИСУТСТВИЯ — ВЫПИСАН НАМЕРЕННО (`A-031` §1 п.1) ═══
#
# Решение арбитра обязательно обеим сторонам: класс «страж проверяет ПРИЗНАК ШИРЕ, чем
# требование» закрывает не привязка НАЗВАННЫХ стражей, а ВЫПИСАННАЯ ГРУППА — правило `Р-3`
# (`docs/workflow/oracle-blindness-class-2026-08-28.md` §5) на уровне самих стражей: «опасна
# ровно та группа, которая НЕ ВЫПИСАНА». В `verify_M-45.sh` носители №5–№8 жили именно
# потому, что перечня не существовало и каждый чинился по указанию вердикта.
#
# ПРАВИЛО ВЕДЕНИЯ: добавил страж присутствия — добавь строку сюда. Предмет наблюдения обязан
# совпадать с предметом требования; не совпадает — чини либо назови предел строкой.
#
# | шаг       | требование                                  | предмет наблюдения (чем пиннится)             |
# |-----------|---------------------------------------------|-----------------------------------------------|
# | task #1   | три теста расцепления живы                  | `^fn ИМЯ` в T1 + ИСПОЛНЕНИЕ через chk_named_test |
# | task #1b  | два теста серверного владения живы          | `^fn ИМЯ` в T5 + ИСПОЛНЕНИЕ                    |
# | task #2   | окно не выводится из селектора В ТЕЛЕ        | тело `build_heatmap_and_cob` + setup-guard непустоты |
# | task #2   | сигнатуры §5 ОБЪЯВЛЕНЫ, а не упомянуты      | `^pub const DEFAULT_…`, `^pub fn effective_…(` |
# | task #2   | значение поставляет ВЫЗЫВАТЕЛЬ              | строки вызова БЕЗ комментариев: есть effective_…, нет selector/bands |
# | task #2b  | приём применён в четырёх чужих оракулах     | ВЫЗОВ сеттера вне комментария (`^[^/]*…(`)     |
# | task #2b  | гонка за процессно-глобальное окно закрыта  | guard'ов `let _ = serial();` ≥ числа `#[test]` В КАЖДОМ файле |
# | task #2b  | чужие оракулы зелены ПРИ СЕРВЕРНОМ ОКНЕ     | их ИСПОЛНЕНИЕ + setup-guard «сеттер существует в LIB» |
# | task #3   | пять сценариев `H-2` живы                   | `^fn ИМЯ` в T2 + ИСПОЛНЕНИЕ                    |
# | task #5b  | два сценария `H-6` живы                     | `^fn ИМЯ` в T6 + ИСПОЛНЕНИЕ                    |
# | task #4   | ручка ДОЕХАЛА до сервиса-потребителя        | СТРОКА ключа YAML внутри блока `gateway-serve:` + значение 0.001 |
# | C         | границы предмета не тронуты                 | `git diff` от merge-base, не текст файла       |
#
# ТРИ НАЗВАННЫХ ПРЕДЕЛА, а не умолчание:
#   (1) `^fn ИМЯ` + `ran > 0` пиннят СУЩЕСТВОВАНИЕ имени и НЕПУСТОТУ прогона, но не то, что
#       исполнился именно НАЗВАННЫЙ тест: `#[ignore]` на нём этот набор не ловит;
#   (2) структурные стражи привязаны к конструкции, но сдвиг вычисления окна на уровень выше
#       вызывателя их обходит (`M-45` §D-1, тот же класс). Доказательство несут `H-5`/`H-6`;
#   (3) чекпоинт в перечне НЕ фигурирует, и это ЗАМЕР, а не забывчивость: окно применяется на
#       ПОСТРОЕНИИ (`lib.rs:1557`), а `heatmap_buckets` хранят полный снимок книги
#       (`lib.rs:1182` `entry.refresh(bids, asks)`) — состояние чекпоинта от окна не зависит,
#       инвалидация не нужна, `selector_fingerprint` не расширяется. Разбор — спека §4bis.

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
T6=crates/gateway-serve/tests/red_heatmap_window_effective_setting.rs

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
# Сигнатуры §5 пиннятся КОНСТРУКЦИЕЙ ОБЪЯВЛЕНИЯ, а не вхождением имени в файл (`A-031`
# носитель №6: там страж принимал за эталон литерал, лежащий в комментарии). Doc-комментарий
# `/// см. effective_heatmap_window_frac` больше не удовлетворяет требование «функция есть».
chk "grep -qE '^pub fn effective_heatmap_window_frac\\(' ${LIB}"
chk "grep -qE '^pub const DEFAULT_HEATMAP_WINDOW_FRAC' ${LIB}"

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
# Фильтр отбрасывает ОБЪЯВЛЕНИЕ (`fn …`) и КОММЕНТАРИИ обеих форм. Строка `//`-комментария,
# цитирующая вызов, — не вызов: без этого фильтра комментарий с `selector` давал бы ложное
# КРАСНОЕ, а комментарий с `effective_…` — ложное зелёное (`A-031` побочная находка `T5`:
# префикс `grep -n` ломает якорь `^\s*//`, поэтому здесь якорь ставится ПОСЛЕ префикса).
CALLS="$(grep -n 'build_heatmap_and_cob(' "${LIB}" 2>/dev/null | grep -vE '^[0-9]+:[[:space:]]*(//|///|//!)' | grep -vE '^[0-9]+:[[:space:]]*fn ')"
NCALLS="$(printf '%s\n' "${CALLS}" | grep -c . || true)"
if [ "${NCALLS:-0}" -lt 1 ]; then
  echo "FAIL: место вызова build_heatmap_and_cob не найдено — функция переименована, встроена или удалена; канарейка call-site была бы ВАКУУМНО зелёной" >&2
  FAIL=$((FAIL + 1))
else
  chk "printf '%s\n' \"\${CALLS}\" | grep -q 'effective_heatmap_window_frac'"
  chk "printf '%s\n' \"\${CALLS}\" | grep -qE 'selector|bands' && exit 1 || exit 0"
fi

step "task #2b — затронутые ЧУЖИЕ оракулы восстановлены при серверном окне (C-198 B-5, C-201 B-7)"
# ШАГ ИСПОЛНЯЕМЫЙ, А НЕ ТЕКСТОВЫЙ (`C-201` B-7). Прежняя редакция грепала имя функции — и
# зеленела от ОДНОГО КОММЕНТАРИЯ с этим именем; предъявлено прогоном. Это разобранный класс
# («grep по имени ловит и лог-строки», `testing.md`), повторённый автором.
#
# Три исхода различает `chk_named_test`: ВАКУУМ (тестов не нашлось) ≠ COMPILE-RED (сеттера
# ещё нет, задача 2 не сделана) ≠ исполнено-и-упало (восстановление неверно). До задачи 2 шаг
# КРАСЕН как COMPILE-RED — это правильное состояние, а не дефект гейта.
#
# SETUP-GUARD НА СОБСТВЕННОЕ УТВЕРЖДЕНИЕ (`A-031` §1, класс «страж шире требования»).
# Требование шага — «чужие оракулы зелены ПРИ СЕРВЕРНОМ ОКНЕ». Пока сеттера в `lib.rs` нет,
# окна не существует вовсе, и зелёный прогон этих четырёх наборов говорит о СТАРОЙ связке
# `max(selector.bands)` — то есть предмет наблюдения (просто «зелены») ШИРЕ предмета
# требования. Без этого guard'а шаг печатал бы PASS с ложной подписью.
SETTER_RE='^pub fn set_effective_heatmap_window_frac\('
for t in red_depth_from_book red_depth_provenance_by_reach red_heatmap red_egress_cap; do
  if grep -qE "${SETTER_RE}" "${LIB}"; then
    chk_named_test "затронутый оракул ${t} зелен при серверном окне" \
      cargo test -p gateway --test "${t}" --quiet
  else
    echo "FAIL: затронутый оракул ${t} — задача 2 НЕ исполнена (нет '^pub fn set_effective_heatmap_window_frac('): серверного окна не существует, и зелёный прогон подтвердил бы СТАРУЮ связку, а не восстановление" >&2
    FAIL=$((FAIL + 1))
  fi
done
# Дешёвый страж поверх исполнения: вызов обязан быть ВЫЗОВОМ, а не упоминанием в комментарии.
# Строка, начинающаяся с `//`, не считается — это и есть дыра, найденная `C-201` B-7.
for f in crates/gateway/tests/red_depth_from_book.rs \
         crates/gateway/tests/red_depth_provenance_by_reach.rs \
         crates/gateway/tests/red_heatmap.rs \
         crates/gateway/tests/red_egress_cap.rs; do
  chk "grep -E '^[^/]*set_effective_heatmap_window_frac\\(' ${f} >/dev/null"
done
# Гигиена процессно-глобального окна во ВСЕХ четырёх файлах (`C-201` B-6: гонка в red_heatmap
# была флаком — три прогона дали FAILED/FAILED/ok; serial стоял лишь в одном файле).
#
# СЧИТАЕТСЯ ПОКРЫТИЕ, А НЕ НАЛИЧИЕ ОПРЕДЕЛЕНИЯ (`A-031` §1, `Р-3`). Требование — «КАЖДЫЙ тест
# файла держит guard»; `grep -q 'fn serial()'` наблюдал бы «в файле есть определение», что
# истинно и при нуле использований: группа тестов НЕ ВЫПИСАНА, и один незакрытый член вернул
# бы ровно тот флак, из-за которого `C-201` B-6 и вынесен. Поэтому число guard'ов
# сравнивается с числом тестов ФАЙЛА; setup-guard — «тесты вообще найдены», иначе сравнение
# `0 >= 0` было бы вакуумно истинным при переименовании атрибута или сносе файла.
for f in crates/gateway/tests/red_egress_cap.rs \
         crates/gateway/tests/red_heatmap.rs \
         crates/gateway/tests/red_depth_from_book.rs \
         crates/gateway/tests/red_depth_provenance_by_reach.rs; do
  n_tests=$(grep -cE '^[[:space:]]*#\[(tokio::)?test\]' "${f}" 2>/dev/null || true)
  n_guards=$(grep -cE '^[[:space:]]*let _[a-zA-Z_]+ = serial\(\);' "${f}" 2>/dev/null || true)
  n_def=$(grep -cE '^fn serial\(\)' "${f}" 2>/dev/null || true)
  if [ "${n_tests:-0}" -lt 1 ]; then
    echo "FAIL: гигиена окна — в ${f} не найдено ни одного #[test]: сравнивать guard'ы не с чем (файл снесён, переименован или атрибут сменил форму)" >&2
    FAIL=$((FAIL + 1))
  elif [ "${n_def:-0}" -lt 1 ] || [ "${n_guards:-0}" -lt "${n_tests}" ]; then
    echo "FAIL: гигиена окна — ${f}: определений serial() ${n_def}, guard'ов ${n_guards} при ${n_tests} тестах; непокрытый тест возвращает флак C-201 B-6" >&2
    FAIL=$((FAIL + 1))
  else
    echo "PASS: гигиена окна — ${f}: ${n_guards} guard'ов на ${n_tests} тестов"
  fi
done

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

step "task #5b (RED) — СЕРВЕРНАЯ настройка управляет охватом карты"
# Закрывает `C-194` B-2 (вторая половина) и `C-196` B-3. Отдельный шаг, потому что он судит
# ТРЕТИЙ мир: «жёсткая константа в теле, конфиг игнорируется». Тот мир проходит ВСЕ
# предыдущие оракулы — замер `C-196` B-3 воспроизведён architect'ом: `w = 0.001` даёт
# 3/3 + 2/2 PASS. Ловится только сменой серверной настройки на прод-пути
# `env → serve_config_from_env → выдача`.
chk_named_test "оракул эффективной серверной настройки (H-6 · H-6b)" \
  cargo test -p gateway-serve --test red_heatmap_window_effective_setting --quiet
for t in hw_i_6_effective_server_setting_controls_map_extent \
         hw_i_6b_both_settings_produce_a_nonempty_map; do
  chk "grep -q '^fn ${t}' ${T6}"
done

step "task #4 — ручка ДОЕХАЛА до сервиса-потребителя и РАВНА сегодняшнему эффективному"
# Требование — «оператор может задать окно у сервиса, который его читает». Прежняя редакция
# наблюдала вхождение имени В ФАЙЛЕ: комментарий `# GATEWAY_HEATMAP_WINDOW: 0.001` удовлетворял
# ОБЕ проверки, а ручка при этом не существовала — дословно носитель №6 `A-031` (`grep` по
# литералу берётся комментарием). Наблюдается СТРОКА КЛЮЧА YAML внутри блока `gateway-serve:` —
# единственного сервиса, читающего окно: `gateway-checkpoint` хранит состояние, от окна не
# зависящее (предел (3) в шапке).
HW_BLOCK="$(awk '/^  gateway-serve:/{f=1} f&&/^  [a-z-]+:$/&&!/gateway-serve/{exit} f' docker-compose.yml 2>/dev/null)"
HW_NEIGHBOR="$(printf '%s\n' "${HW_BLOCK}" | grep -cE '^[[:space:]]+GATEWAY_BANDS:' || true)"
if [ "${HW_NEIGHBOR:-0}" -lt 1 ]; then
  echo "FAIL: task #4 SETUP НЕ СОСТОЯЛСЯ — блок сервиса gateway-serve не извлечён (в нём нет даже GATEWAY_BANDS): вывод об отсутствии нашей ручки был бы ложным при любой реализации" >&2
  FAIL=$((FAIL + 1))
else
  HW_LINE="$(printf '%s\n' "${HW_BLOCK}" | grep -E '^[[:space:]]+GATEWAY_HEATMAP_WINDOW:')"
  if [ -z "${HW_LINE}" ]; then
    echo "FAIL: task #4 — GATEWAY_HEATMAP_WINDOW не объявлен ЗАПИСЬЮ env в блоке gateway-serve (упоминание в комментарии записью не является)" >&2
    FAIL=$((FAIL + 1))
  elif printf '%s\n' "${HW_LINE}" | grep -qE '(^|[^0-9])0\.001([^0-9]|$)'; then
    echo "PASS: task #4 — ручка объявлена записью в блоке gateway-serve и равна 0.001"
  else
    echo "FAIL: task #4 — запись есть, но значение не равно сегодняшнему эффективному 0.001: '${HW_LINE}'. Иное значение меняет ВЫДАЧУ в момент внедрения, тогда как §5 обещает нулевое изменение данных" >&2
    FAIL=$((FAIL + 1))
  fi
fi

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
