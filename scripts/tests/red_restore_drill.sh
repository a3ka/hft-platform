#!/usr/bin/env bash
# RED `M-74` задача 1 — DRILL ОБЯЗАН ЧИТАТЬ КОПИЮ ПРОД-ЧИТАТЕЛЕМ И РАЗЛИЧАТЬ ИСХОДЫ.
#
# СОСТОЯНИЕ: КРАСНАЯ ПО ПОСТРОЕНИЮ. Обёртки `deploy/bin/journal-restore-drill-cron.sh` и
# бинаря-читателя `journal-drill-read` ещё НЕТ — их вносит engine-dev задачами 2 и 2b. Это
# RED-first: тест — спецификация, и он написан ДО кода.
#
# ═══ ЧТО ИЗМЕНИЛОСЬ ПРОТИВ РЕДАКЦИИ `b8d989e` И ПОЧЕМУ (A-028 §3 п.2 + собственный замер) ═══
#
# Прежняя фикстура клала плоские байты `SEGMENT-0001-PAYLOAD` и манифест `{"legacy":[...]}`.
# Ни то, ни другое не принимает прод-читатель: сегмент schema ≥ 2 начинается с
# `SEGMENT_MAGIC = *b"HFTJRN02"`, а манифест — это `LegacyManifest { declarations: … }`.
# Значит позитивный контроль `H` прошёл бы ТОЛЬКО у обёртки с mock-читателем — проба толкала
# исполнителя в обход прод-пути. Арбитр назвал это блокером.
#
# Фикстуру теперь строит `crates/journal/tests/fixture_restore_drill_cold.rs` — ТЕМ ЖЕ
# писателем и той же компакцией, что работают на проде, и там же предъявлено прогоном, что
# построенное читается `journal::stream`'ом (а испорченное — нет).
#
# ═══ ФОРМА ПРОДА СНЯТА ЗАМЕРОМ 2026-08-31, А НЕ ВООБРАЖЕНА ═══
#
#   $ cat  <журнал>/journal.legacy.json          → {"declarations": []}    ← ПУСТ
#   $ ls   <журнал>/segment-* | head -1          → segment-00000001.jrnl.zst
#   $ ssh box 'ls journal/ | grep -v ^segment'   → journal.legacy.json, journal.replay-digest.json
#                                                  (journal.meta ОТСУТСТВУЕТ)
#   $ ssh box 'ls journal/ | wc -l'              → 501 = 478 .zst + 21 .jrnl + 2 sidecar
#   $ пар «и .jrnl, и .jrnl.zst» на один индекс  → 17
#
# Отсюда прямые следствия для сценариев ниже:
#  • сценарий `M` («sidecar'ов нет ⇒ legacy не читается») СНЯТ: на проде НОЛЬ legacy-деклараций,
#    отсутствие манифеста для наших данных безвредно, и сценарий был бы зелен по отсутствию
#    предмета. Его предмет — различение «нет контекста» и «порча» — перенесён в
#    `fixture_restore_drill_cold.rs::undeclared_legacy_is_a_context_error_not_a_corruption_error`,
#    где помечен FORWARD-GUARD'ом: он сторожит будущую декларацию, а не сегодняшний прод;
#  • вместо него введён сценарий `E` (ПУСТОТА), который на проде ДОСТИЖИМ и опасен: копия
#    «скачалась успешно» и не привезла ничего, а `journal::stream` на пустом каталоге
#    возвращает Ok с нулём событий — то есть drill, решающий по коду возврата читателя,
#    рапортует успех. Это ровно класс `R-157` `Б-5`;
#  • `H` обязан подтвердить, что прочитана И СЖАТАЯ форма: drill, умеющий только `.jrnl`,
#    молча пропустил бы 478 сегментов из 499 и был бы зелёным.
#
# ═══ СЦЕНАРИИ ═══
#   S  фикстура материализована                → SETUP-GUARD (иначе всё ниже вакуумно)
#   P  argv печатается ДО побочных эффектов    → путь записи fetch'а == путь чтения читателя
#   H  здоровая прод-форменная копия           → drill ПРОХОДИТ, ok=1, events_read>0, читал .zst
#   C  один сжатый сегмент повреждён           → ОТКАЗ, ok=0, причина называет ЧТЕНИЕ
#   E  холодная копия ПУСТА                    → ОТКАЗ rc=5, причина ОТЛИЧНА от C
#   F  копия полна, доставка молча не сработала → ОТКАЗ rc=5: судим ПРОЧИТАННОЕ, не завершение
#   A  оборванный файл состояния               → перезаписан целиком (канарейка обрывка исчезла)
#
# `H` — позитивный контроль. Без него `C` и `E` зелены против drill'а, который отказывает
# ВСЕГДА: «отказал на битой копии» верно и для процедуры, которая не работает никогда.
#
# ═══ ЧЕГО ПРОБА НЕ ЛОВИТ — названо ═══
#   • сеть и права Storage Box: `JOURNAL_DRILL_TRANSPORT=local`, то есть гоняется ЛОКАЛЬНАЯ
#     ветка доставки. Сетевая ветка проверяется только первым автопрогоном на VPS;
#   • порчу в НЕВЫБРАННОМ сегменте: принятый предел выборки, названный в спеке `M-74`.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DRILL="${ROOT}/deploy/bin/journal-restore-drill-cron.sh"

PASSED=0; FAILED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 2; }

REG="$(mktemp /tmp/red-drill-reg-XXXXXX)" || die mktemp
cleanup() {
  if [ -n "${KEEP_FIXTURES:-}" ]; then echo "песочницы оставлены: ${REG}"; return 0; fi
  while IFS= read -r d; do
    case "$d" in /tmp/red-drill-*) [ -d "$d" ] && rm -rf "$d" ;; esac
  done < "${REG}" 2>/dev/null
  rm -f "${REG}"
}
trap cleanup EXIT

# ── ЗАЯВЛЕННОЕ КРАСНОЕ. Обёртки нет — это состояние задачи 2, а не сбой пробы. ───────────
if [ ! -f "${DRILL}" ]; then
  echo "── RESTORE-DRILL: обёртка ещё не внесена"
  fail "обёртки ${DRILL#"${ROOT}"/} НЕ СУЩЕСТВУЕТ — RED задачи 1 (её вносит engine-dev задачей 2)"
  echo
  echo "VERDICT: FAIL (1 из 1) — RED-first: спецификация есть, реализации нет"
  exit 1
fi

# ── Фикстура строится ПРОД-ПИСАТЕЛЕМ, а не printf'ом ────────────────────────────────────
# Единственный строитель — тест-таргет крейта `journal`. Здесь только вызов и СТРАЖ того,
# что вызов дал результат: `cargo test` возвращает 0 и когда фильтр не нашёл ни одного теста.
make_fixture() { # $1=имя переменной под каталог; $2=healthy|corrupt|empty
  local d out
  d="$(mktemp -d /tmp/red-drill-XXXXXX)" || die mktemp
  printf '%s\n' "${d}" >> "${REG}"
  out="$(cd "${ROOT}" && DRILL_FIXTURE_OUT="${d}" DRILL_FIXTURE_VARIANT="$2" \
        cargo test -p journal --test fixture_restore_drill_cold --quiet \
        -- --exact materialize_for_shell_probe --nocapture 2>&1)" \
    || die "строитель фикстуры не собрался/упал (вариант $2):"$'\n'"${out}"
  printf '%s\n' "${out}" | grep -q 'DRILL_FIXTURE_READY' \
    || die "строитель НЕ отчитался о материализации (вариант $2) — шаг был бы вакуумным:"$'\n'"${out}"
  [ -d "${d}/cold" ] || die "каталог холодной копии не создан (вариант $2)"
  printf -v "$1" '%s' "${d}"
}

# Число РАЗЛИЧНЫХ индексов сегментов в каталоге (дубли raw+zst считаются один раз).
seg_indices() { ls "$1" 2>/dev/null | sed -n 's/^segment-\([0-9]\{8\}\)\.jrnl\(\.zst\)\?$/\1/p' | sort -u | wc -l; }

run_drill() { # $1=песочница → код возврата обёртки
  ( cd "$1" \
    && JOURNAL_DRILL_COLD="$1/cold" \
       JOURNAL_DRILL_RESTORE="$1/restore" \
       JOURNAL_DRILL_STATE="$1/state/journal-restore-drill.json" \
       JOURNAL_DRILL_TRANSPORT="local" \
       JOURNAL_DRILL_READER="cargo run -q --manifest-path ${ROOT}/Cargo.toml -p journal --bin journal-drill-read --" \
       bash "${DRILL}" >"$1/drill.log" 2>&1 )
  local rc=$?
  printf '%s' "${rc}" > "$1/drill.rc"
  return "${rc}"
}

drill_rc() { cat "$1/drill.rc" 2>/dev/null; }

state_field() { # $1=песочница $2=имя поля → значение или пусто
  sed -n "s/.*\"$2\"[[:space:]]*:[[:space:]]*\"\{0,1\}\([^,\"}]*\)\"\{0,1\}.*/\1/p" \
    "$1/state/journal-restore-drill.json" 2>/dev/null | head -1
}

echo "── RESTORE-DRILL: копия ЧИТАЕТСЯ прод-читателем, а не только существует"

# ── S — SETUP-GUARD ─────────────────────────────────────────────────────────────────────
make_fixture BOX_H healthy
N_IDX="$(seg_indices "${BOX_H}/cold")"
N_ZST="$(ls "${BOX_H}/cold" 2>/dev/null | grep -c '\.jrnl\.zst$')"
if [ "${N_IDX}" -ge 3 ] && [ "${N_ZST}" -ge 1 ]; then
  pass "S фикстура прод-формы: ${N_IDX} индексов сегментов, из них сжатых файлов ${N_ZST}"
else
  die "фикстура не прод-формы: индексов ${N_IDX} (нужно ≥3), сжатых ${N_ZST} (нужно ≥1). \
Правило выборки берёт ТРИ сегмента, а самый старый на проде — сжатый"
fi

# ── P — КОМПОЗИЦИЯ: куда пишет доставка, оттуда читает читатель ─────────────────────────
ARGV="$( cd "${BOX_H}" \
  && JOURNAL_DRILL_COLD="${BOX_H}/cold" \
     JOURNAL_DRILL_RESTORE="${BOX_H}/restore" \
     JOURNAL_DRILL_STATE="${BOX_H}/state/journal-restore-drill.json" \
     JOURNAL_DRILL_TRANSPORT="local" \
     JOURNAL_DRILL_READER="журнал-читатель-заглушка-для-печати" \
     HFT_CRON_PRINT_ARGV=1 bash "${DRILL}" 2>&1 )"
if printf '%s' "${ARGV}" | grep -q "RESTORE_DIR=${BOX_H}/restore" \
   && printf '%s' "${ARGV}" | grep -q "READER_DIR=${BOX_H}/restore"; then
  pass "P argv печатает КОМПОЗИЦИЮ: доставка пишет в тот же каталог, из которого читает читатель"
else
  fail "P argv не предъявляет композицию (RESTORE_DIR / READER_DIR). Рассогласование этих \
двух строк даёт тихий no-op: drill «прочитает» пустой каталог и отчитается об успехе. Вывод:"$'\n'"${ARGV}"
fi
# Печать argv обязана идти ДО побочных эффектов: файл состояния после неё не появляется.
if [ -f "${BOX_H}/state/journal-restore-drill.json" ]; then
  fail "P HFT_CRON_PRINT_ARGV=1 произвёл побочный эффект — состояние записано при печати argv"
else
  pass "P печать argv не произвела побочных эффектов"
fi

# ── H — позитивный контроль ────────────────────────────────────────────────────────────
if run_drill "${BOX_H}"; then
  ok="$(state_field "${BOX_H}" ok)"
  ev="$(state_field "${BOX_H}" events_read)"
  ck="$(state_field "${BOX_H}" checked)"
  zs="$(ls "${BOX_H}/restore" 2>/dev/null | grep -c '\.jrnl\.zst$')"
  if [ "${ok}" = "1" ] && [ "${ev:-0}" -gt 0 ] && [ "${ck}" = "3" ] && [ "${zs}" -ge 1 ]; then
    pass "H здоровая копия ⇒ drill прошёл: ok=1, прочитано событий ${ev}, сегментов ${ck}, сжатых в выборке ${zs}"
  else
    fail "H drill вернул 0, но состояние не доказывает чтения (ok=${ok}, events_read=${ev:-∅}, checked=${ck:-∅}, сжатых в выборке ${zs}). \
Ноль событий или ноль сжатых = «процедура завершилась», а не «копия читается»"
  fi
else
  fail "H здоровая прод-форменная копия ⇒ drill НЕ прошёл (exit≠0). Всё остальное в этой пробе \
зелено по отсутствию предмета. Лог: $(tail -3 "${BOX_H}/drill.log" 2>/dev/null | tr '\n' ' ')"
fi

# ── C — порча ───────────────────────────────────────────────────────────────────────────
make_fixture BOX_C corrupt
reason_c=""
if run_drill "${BOX_C}"; then
  fail "C повреждённый сегмент ⇒ drill ПРОШЁЛ — копия объявлена читаемой, не будучи ею"
else
  reason_c="$(state_field "${BOX_C}" reason)"
  rc_c="$(drill_rc "${BOX_C}")"
  if [ "$(state_field "${BOX_C}" ok)" = "0" ] && [ -n "${reason_c}" ] && [ "${rc_c}" = "4" ]; then
    pass "C повреждённый сегмент ⇒ отказ rc=4 (ЧТЕНИЕ), ok=0, причина: ${reason_c}"
  else
    fail "C отказ есть, но не КОДОМ ЧТЕНИЯ (rc=${rc_c:-∅}, ожидалось 4; ok=$(state_field "${BOX_C}" ok), причина «${reason_c}»). \
Отказ по любой причине здесь неотличим от отказа по неверной — именно так первый прогон этой пробы \
дал «PASS C» на обёртке, которая на самом деле не нашла ни одного сегмента"
  fi
fi

# ── E — пустота; причина ОБЯЗАНА отличаться от порчи ────────────────────────────────────
make_fixture BOX_E empty
if run_drill "${BOX_E}"; then
  fail "E восстановление привезло НОЛЬ сегментов ⇒ drill ПРОШЁЛ. Прод-читатель на пустом \
каталоге возвращает Ok с нулём событий — значит drill судит по коду возврата, а не по прочитанному (класс R-157 Б-5)"
else
  reason_e="$(state_field "${BOX_E}" reason)"
  rc_e="$(drill_rc "${BOX_E}")"
  if [ "$(state_field "${BOX_E}" ok)" != "0" ] || [ -z "${reason_e}" ]; then
    fail "E отказ есть, но состояние не выставлено честно (ok=$(state_field "${BOX_E}" ok), причина «${reason_e}»)"
  elif [ "${rc_e}" != "5" ] || [ "${rc_e}" = "${rc_c:-}" ]; then
    fail "E код возврата ${rc_e:-∅} (ожидалось 5) либо совпадает с кодом порчи ${rc_c:-∅} — \
машинный потребитель (обёртка → метрика → алерт) не различит пустоту и порчу"
  elif [ "${reason_e}" = "${reason_c}" ]; then
    fail "E причина ПУСТОТЫ совпадает с причиной ПОРЧИ («${reason_e}») — оператор не различит \
«копия испорчена» и «доставка ничего не привезла», а лечатся они по-разному"
  else
    pass "E пустое восстановление ⇒ отказ с ОТДЕЛЬНОЙ причиной: ${reason_e}"
  fi
fi

# ── F — ДОСТАВКА МОЛЧА НЕ СРАБОТАЛА: сегменты в копии ЕСТЬ, в восстановлении ИХ НЕТ ─────
# Отличается от `E` принципиально, и различие найдено мутационным контролем: обёртка
# отсекает ПУСТУЮ холодную копию ДО вызова читателя, поэтому `E` не судит поведение читателя
# на нуле событий вовсе. Мутация «читатель принимает ноль прочитанных событий» проходила
# набор 7/7. Здесь копия полна, а доставка отказывает по правам — так выглядит сбой сети или
# прав Storage Box, и это ровно класс `R-157` `Б-5`: процедура «завершилась», не сделав ничего.
make_fixture BOX_F healthy
chmod 000 "${BOX_F}"/cold/segment-*.jrnl* || die "снять права с сегментов копии"
if run_drill "${BOX_F}"; then
  fail "F доставка не привезла НИ ОДНОГО сегмента ⇒ drill ПРОШЁЛ. Копия объявлена читаемой \
по факту завершения процедуры, а не по прочитанным событиям"
else
  rc_f="$(drill_rc "${BOX_F}")"
  ok_f="$(state_field "${BOX_F}" ok)"
  if [ "${ok_f}" = "0" ] && [ "${rc_f}" = "5" ]; then
    pass "F доставка молча не сработала ⇒ отказ rc=5 (ПУСТОТА), ok=0"
  else
    fail "F отказ есть, но не КОДОМ ПУСТОТЫ (rc=${rc_f:-∅}, ожидалось 5; ok=${ok_f:-∅})"
  fi
fi
chmod 644 "${BOX_F}"/cold/segment-*.jrnl* 2>/dev/null

# ── A — атомарность записи состояния ───────────────────────────────────────────────────
make_fixture BOX_A healthy
mkdir -p "${BOX_A}/state" || die "каркас state"
# КАНАРЕЙКА ОБРЫВКА. Проверять «в файле есть ts_wall_ms и }» НЕДОСТАТОЧНО: обёртка,
# ДОПИСЫВАЮЩАЯ новое состояние вместо атомарной подмены, оставляет обрывок сверху, а обе
# проверки находят его в ДОПИСАННОМ объекте и зеленеют. Поймано мутационным контролем
# (`M5`) на этой же пробе: набор давал 7/7 против дописывания. Маркер решает это точно —
# он обязан ИСЧЕЗНУТЬ, а исчезнуть он может только при перезаписи файла целиком.
CANARY='ОБРЫВОК-НЕ-ДОЛЖЕН-ПЕРЕЖИТЬ-ПРОГОН'
printf '{"ok": 1, "canary": "%s", "ts_wall_ms": 178816' "${CANARY}" \
  > "${BOX_A}/state/journal-restore-drill.json" || die "обрывок"
if run_drill "${BOX_A}" && [ "$(state_field "${BOX_A}" ok)" = "1" ]; then
  if grep -q "${CANARY}" "${BOX_A}/state/journal-restore-drill.json"; then
    fail "A обрывок ПЕРЕЖИЛ прогон — состояние не подменяется атомарно (дописывание/частичная \
запись). Потребитель прочтёт первый попавшийся объект и примет мусор за успех"
  elif [ "$(grep -c '"ts_wall_ms"' "${BOX_A}/state/journal-restore-drill.json")" = "1" ] \
       && grep -q '}' "${BOX_A}/state/journal-restore-drill.json"; then
    pass "A оборванное состояние ПЕРЕЗАПИСАНО целиком (канарейка обрывка исчезла, объект ровно один)"
  else
    fail "A в состоянии не ровно один объект — потребитель прочтёт не тот"
  fi
else
  fail "A drill на здоровой копии не прошёл (ok=$(state_field "${BOX_A}" ok)) — сценарий не судит атомарность"
fi

echo
TOTAL=$((PASSED + FAILED))
if [ "${FAILED}" -eq 0 ]; then
  echo "VERDICT: PASS (${PASSED}/${TOTAL}) — копия читается прод-читателем, исходы различимы"
  exit 0
fi
echo "VERDICT: FAIL (${FAILED} из ${TOTAL})"
exit 1
