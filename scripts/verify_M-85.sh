#!/usr/bin/env bash
# Acceptance-гейт M-85 — кадр `frames_since` строится ОТ КНИГИ (вторая половина `TD-199`).
#
# ГЕЙТ НАПИСАН ДО РАБОТЫ И ОБЯЗАН БЫТЬ КРАСНЫМ. Шаг, ставший зелёным РАНЬШЕ своей задачи, —
# дефект гейта, и чинить надо гейт.
#
# ═══ ПЕРЕЧЕНЬ СТРАЖЕЙ ПРИСУТСТВИЯ — ВЫПИСАН НАМЕРЕННО (`A-031` §1 п.1) ═══
#
# ПРАВИЛО ВЕДЕНИЯ: добавил страж присутствия — добавь строку сюда. Предмет наблюдения обязан
# совпадать с предметом требования; не совпадает — чини либо назови предел строкой.
#
# | шаг      | требование                                  | предмет наблюдения (чем пиннится)               |
# |----------|---------------------------------------------|--------------------------------------------------|
# | task #1  | клиент ≡ реплей на ДЕЛЬТА-хвосте            | ИСПОЛНЕНИЕ `m85_1` + `^fn` имени                 |
# | task #2  | норма (журнал из якорей) не сломана         | ИСПОЛНЕНИЕ `m85_2` + `^fn` имени                 |
# | SETUP    | предмет воспроизведён, а не выродился       | ИСПОЛНЕНИЕ `m85_3` + `^fn` имени                 |
# | task #4  | путь `pump` НЕ тронут                       | ИСПОЛНЕНИЕ оракулов `M-77`, а не `git diff`      |
# | task #5  | док-комментарий не врёт о живости пути      | греп ОТСУТСТВИЯ утверждения + страж, что предмет на месте |
# | СИГНАТУРА| публичная форма `frames_since` не менялась  | `git diff` сигнатуры от merge-base, не текст     |
# | C        | границы предмета не тронуты                 | `git diff` от merge-base                         |
#
# ТРИ НАЗВАННЫХ ПРЕДЕЛА, а не умолчание:
#   (1) `^fn ИМЯ` + `ran > 0` пиннят СУЩЕСТВОВАНИЕ имени и НЕПУСТОТУ прогона, но не то, что
#       исполнился именно НАЗВАННЫЙ тест: `#[ignore]` на нём этот набор не ловит;
#   (2) шаги SETUP и task #2 ЗЕЛЕНЫ СЕГОДНЯ, и это НЕ нарушение правила «шаг не зеленеет
#       раньше своей задачи»: оба — СТОРОЖА, а не прогресс. `m85_3` пиннит, что фикстура
#       строит предмет; `m85_2` — что норма работает. Их назначение — покраснеть, если
#       развязка сломает то, что работало. Разница названа, а не замолчана;
#   (3) ЦЕНА затравки (ось К1/К2, спека §2bis.1) этим гейтом НЕ проверяется: оракул границы
#       ресурса пишется architect'ом задачей 3 ПОСЛЕ вердикта критика по оси. До тех пор
#       шаг отсутствует, и его отсутствие объявлено здесь, а не выдано за покрытие.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { printf '\n── %s\n' "$*"; }
chk() {
  local name; name="$(printf '%s' "$1" | sed -n '1{s/^[[:space:]]*//;s/[[:space:]]*$//;p}')"
  [ -n "$name" ] || name="<многострочная проверка>"
  if ( eval "$1" ) >/dev/null 2>&1; then echo "PASS: ${name}"; else echo "FAIL: ${name}" >&2; FAIL=$((FAIL + 1)); fi
}

# ТРИ ИСХОДА, А НЕ ДВА: «оракула нет» и «оракул есть, но не собрался» — разные состояния.
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

# ── САМОПРОВЕРКА ОБОИХ ПОМОЩНИКОВ (урок `C-187` B-4) ─────────────────────────────────────
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
SERVE=crates/gateway-serve/src/lib.rs
T=crates/gateway/tests/red_m85_frames_book_continuity.rs

# База — MERGE-BASE, а не `origin/main` (урок §0bis SESSION-HANDOFF п.4: на отставшей ветке
# `origin/main` даёт ложное «база не установлена достоверно»).
BASE="$(git merge-base origin/main HEAD 2>/dev/null || echo '')"

step "ПАРИТЕТ С CI — fmt + clippy(--all-targets --all-features) + test --all"
chk "cargo fmt --all -- --check"
chk "cargo clippy --all-targets --all-features -- -D warnings"
chk "cargo test --all --quiet"

step "task #1 — клиент ≡ полный реплей на ДЕЛЬТА-хвосте (ядро предмета)"
chk "grep -q '^fn m85_1_client_assembled_series_equals_full_replay_on_delta_tail' ${T}"
chk_named_test "оракул m85_1 (VB-I-2 на пути frames_since)" \
  cargo test -p gateway --test red_m85_frames_book_continuity --quiet \
  m85_1_client_assembled_series_equals_full_replay_on_delta_tail

step "task #2 — НОРМА не сломана: журнал из одних якорей сходится (анти-плацебо)"
chk "grep -q '^fn m85_2_anchored_window_already_agrees_and_must_keep_agreeing' ${T}"
chk_named_test "сторож m85_2 (норма не куплена ценой фикса)" \
  cargo test -p gateway --test red_m85_frames_book_continuity --quiet \
  m85_2_anchored_window_already_agrees_and_must_keep_agreeing

step "SETUP-СТРАЖ — предмет воспроизведён, а не выродился в один кадр"
chk "grep -q '^fn m85_3_setup_guard_tail_frame_is_delta_only' ${T}"
chk_named_test "страж m85_3 (фикстура строит предмет)" \
  cargo test -p gateway --test red_m85_frames_book_continuity --quiet \
  m85_3_setup_guard_tail_frame_is_delta_only

step "task #4 — путь \`pump\` НЕ тронут: регресс M-77 недопустим"
# Наблюдается ИСПОЛНЕНИЕМ чужих оракулов, а не `git diff` по файлу: диф покажет касание
# и при безобидной правке, и промолчит при поломке через общую функцию.
chk_named_test "оракулы M-77 (непрерывность книги на пути pump)" \
  cargo test -p gateway --test red_m77_frame_book_continuity --quiet
chk_named_test "оракул M-77 (цена pump не выросла)" \
  cargo test -p gateway --test red_m77_pump_cost --quiet

step "task #5 — док-комментарий не объявляет живым путь с нулём вызовов (класс TD-138)"
# Страж предмета: сперва убеждаемся, что обёртка ВООБЩЕ на месте — иначе греп отсутствия
# зелен по причине «файла нет», то есть плацебо самого себя.
chk "grep -q 'pub fn frames_msgs' ${SERVE}"
chk "! grep -qE 'инкрементальный push.*frames_msgs|frames_msgs.*инкрементальный push' ${SERVE}"

step "СИГНАТУРА — публичная форма frames_since НЕ менялась (§2.1)"
# Требование именно к СИГНАТУРЕ, а не к файлу: тело меняться обязано.
chk "[ -n \"${BASE}\" ]"
chk "diff <(git show ${BASE}:${LIB} | grep -A7 '^pub fn frames_since(') <(grep -A7 '^pub fn frames_since(' ${LIB})"
chk "diff <(git show ${BASE}:${LIB} | grep -A7 '^pub fn frames_since_with_stats(') <(grep -A7 '^pub fn frames_since_with_stats(' ${LIB})"

step "C — границы: T1 не тронут, состав ЗАПИСИ не тронут, транспорт не переписан"
chk "git diff --name-only ${BASE}..HEAD -- crates/contracts docs/rfc | grep -q . && exit 1 || exit 0"
chk "git diff --name-only ${BASE}..HEAD -- crates/venue-binance crates/venue-binance-futures crates/journal | grep -q . && exit 1 || exit 0"
# Транспорт правится РОВНО одним комментарием (задача 5): больше двух изменённых строк —
# это уже переписывание чужой зоны, и его обязан судить человек, а не пропускать гейт.
# Сумма считается ОДНИМ awk с `END {print s+0}`: первая редакция шла через `paste|bc`, и на
# ПУСТОМ дифе (правки ещё нет) выдавала пустую строку — `[ "" -le 8 ]` падал синтаксисом, то
# есть шаг краснел по причине, не связанной с предметом. Поймано собственным прогоном.
chk "[ \$(git diff --numstat ${BASE}..HEAD -- ${SERVE} | awk '{s+=\$1+\$2} END {print s+0}') -le 8 ]"

if [ "${FAIL}" -eq 0 ]; then
  echo; echo "VERDICT: PASS"; exit 0
else
  echo; echo "VERDICT: FAIL (${FAIL})"; exit 1
fi
