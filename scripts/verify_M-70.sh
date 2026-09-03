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

# ═══ ПЕРЕЧЕНЬ СТРАЖЕЙ ПРИСУТСТВИЯ — ВЫПИСАН НАМЕРЕННО (`A-031` §1 п.1) ═══
#
# Решение арбитра обязательно обеим сторонам и сформулировано как КЛАСС: закрывает его не
# привязка НАЗВАННЫХ стражей, а ВЫПИСАННАЯ ГРУППА — правило `Р-3` на уровне самих стражей,
# «опасна ровно та группа, которая НЕ ВЫПИСАНА». В `verify_M-45.sh` носители №5–№8 жили именно
# потому, что перечня не существовало и каждый чинился по указанию вердикта.
#
# ПРАВИЛО ВЕДЕНИЯ: добавил страж присутствия — добавь строку сюда. Предмет наблюдения обязан
# совпадать с предметом требования; не совпадает — чини либо назови предел строкой.
#
# | шаг         | требование                                 | предмет наблюдения (чем пиннится)              |
# |-------------|--------------------------------------------|------------------------------------------------|
# | ПРЕДУСЛОВИЕ | `M-75` влит в `main`                       | `^pub fn …(` БЕЗ конвейера + самопроверка обоих исходов (`C-208` B-2) |
# | task #3     | предел полос отвергает ДО построения | ИСПОЛНЕНИЕ `DB-I-3` + `^pub const MAX_BANDS` + равенство чисел |
# | task #4     | два сценария `DB-I-4` живы                 | `^fn ИМЯ` в T4 + ИСПОЛНЕНИЕ через chk_named_test |
# | task #4b    | адаптер оракула читает метку КАЖДОЙ точки  | форма `^pub struct DepthPoint` + отсутствие заглушки-однострочника + ИСПОЛНЕНИЕ |
# | task #5     | словарь метки ОДИН на всю выдачу           | ИСПОЛНЕНИЕ `DB-I-5` + поимённый состав + гвард `serial()` |
# | task #6     | форма объявлена БАМПОМ                     | ЧИСЛО версии на HEAD строго больше, чем в базе  |
# | task #6b    | sacred-пины версии подняты вместе с бампом | `EXPECTED_SCHEMA_VERSION` == константа `lib.rs` |
# | task #7     | канонический состав ДОЕХАЛ до сервиса      | СТРОКА ключа YAML + семь значений + `DB-I-7` ДВУХ уровней (парсер И кадр на проводе) |
# | task #8     | `VB-I-10` не ослаблен                      | ИСПОЛНЕНИЕ существующих оракулов окна памяти    |
# | C           | границы предмета не тронуты                | `git diff` от merge-base, не текст файла        |
#
# ТРИ НАЗВАННЫХ ПРЕДЕЛА, а не умолчание:
#   (1) `^fn ИМЯ` + `ran > 0` пиннят СУЩЕСТВОВАНИЕ имени и НЕПУСТОТУ прогона, но не то, что
#       исполнился именно НАЗВАННЫЙ тест: `#[ignore]` на нём этот набор не ловит;
#   (2) шаг task #5 больше не грепает текст `lib.rs`: прежний его греп
#       (`grep -q 'depth_provenance_label'`) был зелен УЖЕ СЕГОДНЯ, до всякой работы —
#       нарушение собственной шапки этого же гейта. Теперь шаг ИСПОЛНЯЕТ `DB-I-5`;
#   (2b) шаг task #6b ЗЕЛЁН СЕГОДНЯ, и это НЕ нарушение правила «шаг не зеленеет раньше своей
#       задачи»: он сторожит СОГЛАСОВАННОСТЬ двух чисел, а не прогресс. Сегодня оба равны 9 —
#       согласованность есть; он покраснеет ровно в момент бампа без правки sacred-пина, то
#       есть в единственном мире, ради которого заведён. Разница названа, а не замолчана;
#   (3) канонический состав полос назван ЧИСЛАМИ здесь, потому что `П-014` п.4 говорит
#       «канонический набор», а перечисляет его `docs/fa/viz-backend.md` §2 (1.5/3/5/8/15/30/60 %).
#       Смена состава = правка FA И этой строки; расхождение двух мест ловится глазами, а не
#       машиной, и это названный предел.

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

step "ПАРИТЕТ С CI — fmt + clippy(--all-targets --all-features) + test --all"
# Шаг НЕ называется «task #0»: у милестоуна task 0 — это ЗАМЕР объёма (`DB-I-0`), и одинаковое
# имя у двух разных предметов отправило бы читателя вердикта искать не то. Свой шаг у замера — ниже.
chk "cargo fmt --all -- --check"
chk "cargo clippy --all-targets --all-features -- -D warnings"
chk "cargo test --all --quiet"

step "ПРЕДУСЛОВИЕ — M-75 влит: без него задачи 0 и 3 меряют мир, которого не будет (§0.3quater)"
# Расцепление окна карты от полос меняет объём канонического набора на два порядка
# (7 882 335 Б → под 2 000 000 Б, замер `M-75` §3). Шаг красен, пока `M-75` не в `main`.
# Якорь КОНСТРУКЦИИ, а не вхождения имени: упоминание в doc-комментарии `main` (например в
# спеке-цитате внутри кода) удовлетворило бы прежний греп, не означая расцепления (`A-031`
# носитель №6 — тот же класс).
#
# ФОРМА ПРОВЕРКИ — БЕЗ КОНВЕЙЕРА, и это не стиль (`C-208` B-2). Прежняя редакция писала
# `git show … | grep -q …`. Под `set -o pipefail` это ЛОЖНОЕ КРАСНОЕ: `grep -q` выходит на
# первом совпадении и закрывает конвейер, `git show` получает SIGPIPE и отдаёт 141, а
# `pipefail` берёт худший код — предусловие «не выполнено» при ВЫПОЛНЕННОМ требовании
# (замер критика: `stages=141 0`). Гейт, который не может стать зелёным после сделанной
# работы, — дефект гейта. Здесь источник читается в переменную, предикат — на herestring:
# конвейера нет, SIGPIPE неоткуда взяться.
m75_has_decoupling() { # $1 — текст файла; 0 = конструкция есть, 1 = нет
  grep -qE '^pub fn effective_heatmap_window_frac\(' <<< "$1"
}
M75_SRC="$(git show origin/main:crates/gateway/src/lib.rs 2>/dev/null || true)"

# САМОПРОВЕРКА ПРЕДИКАТА НА ОБОИХ ИСХОДАХ (`C-208` требование 2): предикат, у которого не
# предъявлены ОБА кода возврата, мог бы быть вечно-красным или вечно-зелёным, и мы бы этого
# не увидели. Проверяется на синтетических входах, а не на реальном файле.
if m75_has_decoupling 'pub fn effective_heatmap_window_frac() -> f64 {' \
   && ! m75_has_decoupling '/// см. pub fn effective_heatmap_window_frac() — упоминание'; then
  echo "PASS: самопроверка предиката M-75 — истинный вход даёт 0, ложный (комментарий) даёт 1"
else
  echo "FAIL: самопроверка предиката M-75 — предикат не различает наличие конструкции и упоминание в комментарии; предусловие судило бы не то" >&2
  FAIL=$((FAIL + 1))
fi

if [ -z "${M75_SRC}" ]; then
  echo "FAIL: ПРЕДУСЛОВИЕ SETUP НЕ СОСТОЯЛСЯ — origin/main:crates/gateway/src/lib.rs не прочитан (нет origin? не тот путь?): вывод о расцеплении был бы ложным при любой реализации" >&2
  FAIL=$((FAIL + 1))
elif m75_has_decoupling "${M75_SRC}"; then
  echo "PASS: ПРЕДУСЛОВИЕ — M-75 влит в main (^pub fn effective_heatmap_window_frac( присутствует)"
else
  echo "FAIL: ПРЕДУСЛОВИЕ — M-75 НЕ влит в main: задачи 0 и 3 мерили бы мир, которого не будет (§0.3quater)" >&2
  FAIL=$((FAIL + 1))
fi

step "task #0 — объём канонического набора СНЯТ ЗАМЕРОМ в байтах (DB-I-0)"
# Задача 0 закрыта не текстом в спеке, а ИСПОЛНЯЕМЫМ замером: два наших документа называли
# 22 МБ и 43 МБ, обе величины — арифметика по числу ячеек и обе относились к миру ДО `M-75`.
# Оракул меряет РЕСУРС (байты сериализованного `Snapshot`) на фикстуре прод-РАЗМЕРА (60
# бакетов = прод-окно 60 с при `timeframe_ms=1000`) и сторожит два регресса: канонический
# набор перестал помещаться под подписанный предел, либо карта снова растёт от полос.
chk_named_test "оракул DB-I-0 (замер объёма + карта не растёт от полос)" \
  cargo test -p gateway --test red_depth_egress_canonical --quiet
for t in db_i_0_canonical_set_fits_under_signed_cap \
         db_i_0b_growth_is_the_depth_series_not_the_map; do
  chk "grep -q '^fn ${t}' crates/gateway/tests/red_depth_egress_canonical.rs"
done

step "task #3 — предел ЧИСЛА полос: отказ ДО построения ответа (DB-I-3)"
# Требование — «клиент не может заставить сервер собрать ответ, чтобы затем его отвергнуть».
# Замер: 4096 полос = 14 077 293 Б и 18.1 с работы РАДИ ОТКАЗА (§2bis.3). Предел `M-71`
# срабатывает ПОСЛЕ построения, значит признак «вернулся Err» доступен и миру без гварда —
# оракул различает миры структурно, вызывая чистую `validate_selector`.
chk_named_test "оракул DB-I-3 (предел полос + анти-плацебо на подписанный состав)" \
  cargo test -p gateway --test red_depth_bands_cap --quiet
for t in db_i_3_selector_with_too_many_bands_is_rejected_before_any_work \
         db_i_3b_snapshot_path_rejects_by_band_cap_not_by_response_size \
         db_i_3c_signed_canonical_set_is_accepted \
         db_i_3d_boundary_is_inclusive_and_exact; do
  chk "grep -q '^fn ${t}' crates/gateway/tests/red_depth_bands_cap.rs"
done
# Гвард обязан жить в `gateway`, а не только в транспорте: `Selector` собирают напрямую
# (research-cli, чекпоинтер, replay), и предел в `gateway-serve` оставил бы байпас-поверхность.
chk "grep -qE '^pub const MAX_BANDS' ${LIB}"
# ЧИСЛО согласовано между реализацией и оракулом — как EXPECTED_SCHEMA_VERSION.
M_LIB="$(sed -n 's/^pub const MAX_BANDS: usize = \([0-9]\+\);.*/\1/p' "${LIB}" | head -1)"
M_ORC="$(sed -n 's/^const EXPECTED_MAX_BANDS: usize = \([0-9]\+\);.*/\1/p' crates/gateway/tests/red_depth_bands_cap.rs | head -1)"
if [ -z "${M_ORC}" ]; then
  echo "FAIL: task #3 SETUP НЕ СОСТОЯЛСЯ — EXPECTED_MAX_BANDS в оракуле не извлёкся: сравнивать предел не с чем" >&2
  FAIL=$((FAIL + 1))
elif [ -z "${M_LIB}" ]; then
  echo "FAIL: task #3 — MAX_BANDS в ${LIB} нет: гвард не введён (оракул ждёт ${M_ORC})" >&2
  FAIL=$((FAIL + 1))
elif [ "${M_LIB}" = "${M_ORC}" ]; then
  echo "PASS: task #3 предел полос согласован реализацией и оракулом (${M_LIB})"
else
  echo "FAIL: task #3 — предел разошёлся: MAX_BANDS=${M_LIB} в реализации, EXPECTED_MAX_BANDS=${M_ORC} в оракуле" >&2
  FAIL=$((FAIL + 1))
fi

step "task #4 (RED) — TD-159: метка достоверности принадлежит ТОЧКЕ, а не ряду"
chk_named_test "оракул DB-I-4 (точка/ряд + анти-плацебо)" \
  cargo test -p gateway --test red_depth_point_provenance --quiet
# Состав ПОИМЁННО (урок 2 шапки): без анти-плацебо основной тест удовлетворяется реализацией,
# метящей `not-observed` ВСЁ подряд, — она обесценивает метку и формально зеленеет.
for t in db_i_4_two_points_of_one_row_carry_their_own_provenance \
         db_i_4b_reachable_point_is_not_falsely_downgraded; do
  chk "grep -q '^fn ${t}' ${T4}"
done

step "task #4b — адаптер sacred-оракула читает метку КАЖДОЙ точки (спека §2bis)"
# Между задачей 4 (dev меняет форму) и этой (architect правит адаптер) набор красен —
# объявлено ЗАРАНЕЕ, а не обнаружено потом (образец `M-75` задача 2b).
#
# SETUP-GUARD НА СОБСТВЕННОЕ УТВЕРЖДЕНИЕ (`A-031` §1): пока формы нет, «оракул зелен» ничего
# не сказало бы о задаче 4b — метке точки взяться неоткуда. Поэтому сперва форма, потом адаптер.
if grep -qE '^pub struct DepthPoint' "${LIB}"; then
  chk "! grep -qE 'vec!\\[row\\.depth_band_provenance' ${T4}"
  chk_named_test "DB-I-4 против НОВОЙ формы (адаптер обновлён)" \
    cargo test -p gateway --test red_depth_point_provenance --quiet
else
  echo "FAIL: task #4b — формы 'pub struct DepthPoint' в ${LIB} нет: задача 4 не исполнена, и адаптер править не подо что (спека §2bis объявляет форму дословно)" >&2
  FAIL=$((FAIL + 1))
fi

step "task #5 — TD-161: словарь метки ОДИН на всю выдачу"
# ПРЕЖНЯЯ РЕДАКЦИЯ ШАГА БЫЛА ДЕФЕКТНОЙ И ЭТО ПРЕДЪЯВЛЕНО, а не заявлено: она грепала текст
# `lib.rs`, и второй её греп (`grep -q 'depth_provenance_label'`) зелен УЖЕ СЕГОДНЯ — функция
# существует с `П-014` и к задаче 5 отношения не имеет. Шаг, зелёный раньше своей задачи, —
# дефект гейта по шапке этого же файла.
#
# Требование говорит о ПОВЕДЕНИИ выдачи: глубокая ячейка heatmap и глубокая строка депт-серии
# В ОДНОМ ответе описаны ОДИНАКОВО, и обе — по своему наблюдению (спека §2bis.1). Такое
# наблюдается оракулом, а не грепом. Оракул ещё НЕ НАПИСАН — шаг честно краснеет ВАКУУМОМ,
# и это видимое состояние набора, а не скрытая дыра.
chk_named_test "оракул DB-I-5 (один словарь на всю выдачу)" \
  cargo test -p gateway --test red_depth_label_dictionary --quiet
for t in db_i_5_one_dictionary_for_cell_and_row_in_the_same_response \
         db_i_5b_map_labels_discriminate_side_like_the_series_does \
         db_i_5c_series_does_not_lose_liveness_to_unification \
         db_i_5d_cell_keeps_the_provenance_of_its_own_observation; do
  chk "grep -q '^fn ${t}' crates/gateway/tests/red_depth_label_dictionary.rs"
done
# Гвард гигиены: окно heatmap процессно-глобально, а этому файлу нужно ШИРОКОЕ окно —
# без `serial()` сосед перезапишет его под ногами (`C-201` B-6, тот же класс).
chk "grep -qE '^fn serial\(\)' crates/gateway/tests/red_depth_label_dictionary.rs"
# Дешёвый структурный страж ПОВЕРХ оракула, не вместо него: унификация идёт ВВЕРХ — heatmap
# приводится к словарю депт-серии, не наоборот (спека §2.1: депт-серия знает сторону и охват).
chk "! grep -q 'let prov_str = \"diff-reconstructed\".to_string()' ${LIB}"

step "task #6 — bump GATEWAY_SCHEMA_VERSION: смена ФОРМЫ выдачи объявлена"
# Метка на точку меняет форму `DepthRow` ⇒ версия обязана сдвинуться (`VB-I-4`, прецеденты
# M-36/M-38a/M-48/M-68). Сверка с `M-72`: два милестоуна не двигают версию одновременно.
BASE=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE}" ]; then
  echo "FAIL: merge-base с origin/main не вычислен — шаги диапазона судить не по чему" >&2
  FAIL=$((FAIL + 1))
else
  # ЧИСЛО, А НЕ ФАКТ ПРАВКИ СТРОКИ. Прежний греп по диффу зеленел от любого касания строки —
  # включая правку комментария рядом и возврат к прежнему значению. Требование говорит
  # «форма объявлена БАМПОМ», то есть версия обязана СТАТЬ БОЛЬШЕ.
  V_BASE="$(git show "${BASE}:${LIB}" 2>/dev/null | sed -n 's/^pub const GATEWAY_SCHEMA_VERSION: u32 = \([0-9]\+\);.*/\1/p' | head -1)"
  V_HEAD="$(sed -n 's/^pub const GATEWAY_SCHEMA_VERSION: u32 = \([0-9]\+\);.*/\1/p' "${LIB}" | head -1)"
  if [ -z "${V_BASE}" ] || [ -z "${V_HEAD}" ]; then
    echo "FAIL: task #6 SETUP НЕ СОСТОЯЛСЯ — версия схемы не извлеклась (база='${V_BASE}' HEAD='${V_HEAD}'): объявление сменило форму, и сравнивать нечего" >&2
    FAIL=$((FAIL + 1))
  elif [ "${V_HEAD}" -gt "${V_BASE}" ]; then
    echo "PASS: task #6 версия схемы поднята ${V_BASE} → ${V_HEAD}"
  else
    echo "FAIL: task #6 версия схемы НЕ поднята (база ${V_BASE}, HEAD ${V_HEAD}): смена формы выдачи обязана объявляться бампом (VB-I-4)" >&2
    FAIL=$((FAIL + 1))
  fi
fi

step "task #6b — sacred-пины версии подняты ВМЕСТЕ с бампом (иначе бамп роняет чужой оракул)"
# `EXPECTED_SCHEMA_VERSION` объявляет ДЕЙСТВУЮЩЕЕ значение в sacred-тесте и роняет три своих
# сценария в момент бампа. Dev его не трогает (`scope-guard.md`) — правит architect задачей 6b.
# Найдено ДО диспетча, а не следующим кругом гейта: ровно класс `C-198` B-5, где на `M-75`
# такие оракулы обнаружились числом десять уже ПОСЛЕ правки.
T6B=crates/gateway/tests/red_gateway_schema_version.rs
E_VER="$(sed -n 's/^const EXPECTED_SCHEMA_VERSION: u32 = \([0-9]\+\);.*/\1/p' "${T6B}" 2>/dev/null | head -1)"
if [ -z "${E_VER}" ] || [ -z "${V_HEAD:-}" ]; then
  echo "FAIL: task #6b SETUP НЕ СОСТОЯЛСЯ — не извлеклись значения (EXPECTED='${E_VER}' LIB='${V_HEAD:-}'): сравнивать нечего, объявление сменило форму" >&2
  FAIL=$((FAIL + 1))
elif [ "${E_VER}" = "${V_HEAD}" ]; then
  echo "PASS: task #6b sacred-пин версии согласован с константой (${E_VER})"
else
  echo "FAIL: task #6b — EXPECTED_SCHEMA_VERSION=${E_VER} в ${T6B}, а константа=${V_HEAD}: бамп уронил чужой sacred-оракул, править его обязан architect (задача 6b), не dev" >&2
  FAIL=$((FAIL + 1))
fi

step "task #7 — канонический состав GATEWAY_BANDS ДОЕХАЛ до сервиса-потребителя"
# ДВЕ ПОЛОВИНЫ, и файл покрывает только первую. Запись в compose говорит, что ручку ОБЪЯВИЛИ;
# что значение доезжает до селектора, по которому строится ответ, — отдельное утверждение, и
# его судит оракул. Мир «включили в конфиге, а выдача прежняя» — класс built-not-wired.
chk_named_test "оракул DB-I-7 (состав из окружения доезжает до селектора)" \
  cargo test -p gateway-serve --test red_depth_bands_delivery --quiet
# `^(async )?fn` — сценарий уровня доставки асинхронный (`#[tokio::test]`), и якорь `^fn`
# его не находил: поимённая сверка поймала это на мне же, ровно за тем и стоит.
for t in db_i_7_canonical_bands_from_env_reach_the_selector \
         db_i_7b_absent_bands_fall_back_to_prod_default_not_to_refusal \
         db_i_7c_canonical_and_default_are_actually_distinguishable \
         db_i_7d_canonical_bands_reach_the_frame_on_the_wire; do
  chk "grep -qE '^(async )?fn ${t}' crates/gateway-serve/tests/red_depth_bands_delivery.rs"
done
# Требование — «оператор отдаёт канонический набор ИЗ СЕРВИСА, который его читает». Прежний
# греп смотрел вхождение `GATEWAY_BANDS.*0.015` В ФАЙЛ: его удовлетворяла закомментированная
# строка и объявление у чужого сервиса, и он не замечал потери шести полос из семи
# (`A-031` носитель №6 — тот же класс).
BANDS_BLOCK="$(awk '/^  gateway-serve:/{f=1} f&&/^  [a-z-]+:$/&&!/gateway-serve/{exit} f' docker-compose.yml 2>/dev/null)"
if ! printf '%s\n' "${BANDS_BLOCK}" | grep -qE '^[[:space:]]+GATEWAY_[A-Z_]+:'; then
  echo "FAIL: task #7 SETUP НЕ СОСТОЯЛСЯ — блок сервиса gateway-serve не извлечён (в нём нет ни одной записи GATEWAY_*): вывод о составе был бы ложным при любой реализации" >&2
  FAIL=$((FAIL + 1))
else
  BANDS_LINE="$(printf '%s\n' "${BANDS_BLOCK}" | grep -E '^[[:space:]]+GATEWAY_BANDS:')"
  if [ -z "${BANDS_LINE}" ]; then
    echo "FAIL: task #7 — GATEWAY_BANDS не объявлен ЗАПИСЬЮ env в блоке gateway-serve (упоминание в комментарии записью не является)" >&2
    FAIL=$((FAIL + 1))
  else
    # ВСЕ СЕМЬ полос канонического набора (`docs/fa/viz-backend.md` §2: 1.5/3/5/8/15/30/60 %).
    # Проверяется КАЖДАЯ: потеря одной — это молча суженная выдача, а не «почти включили».
    MISSING=""
    for b in 0.015 0.03 0.05 0.08 0.15 0.3 0.6; do
      printf '%s\n' "${BANDS_LINE}" | grep -qE "(^|[^0-9.])${b}([^0-9]|$)" || MISSING="${MISSING} ${b}"
    done
    if [ -n "${MISSING}" ]; then
      echo "FAIL: task #7 — в записи GATEWAY_BANDS нет полос:${MISSING} (строка: ${BANDS_LINE}). Канонический набор — семь полос, docs/fa/viz-backend.md §2" >&2
      FAIL=$((FAIL + 1))
    else
      echo "PASS: task #7 канонический набор из семи полос объявлен записью в блоке gateway-serve"
    fi
  fi
fi

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
