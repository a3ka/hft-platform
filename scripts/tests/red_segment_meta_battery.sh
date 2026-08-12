#!/usr/bin/env bash
# Батарея мутантов M-62 (`TD-120`), задача 7 спеки §3, критерий — §4.4/§4.5.
#
# ЗАЧЕМ. RED-набор `red_segment_meta_bound` (SM-0..SM-6) зелен против реализации задач 1-3.
# «Зелёный оракул» и «оракул, который ЛОВИТ» — разные утверждения (`A-005` §6.5: четыре круга
# M-60a сертифицировались пробой, не существовавшей ни в одной ревизии). Батарея предъявляет
# второе: каждый мутант — правка РЕАЛИЗАЦИИ, названная осью и значением, и набор обязан на ней
# краснеть. Эталон (честное удешевление, без мутаций) обязан зеленеть — анти-плацебо в ОБЕ
# стороны (§4.4): `td083` однажды покраснел на строгом УЛУЧШЕНИИ, потому что мерил отношение.
#
# ЧЕМ ЭТА БАТАРЕЯ ОТЛИЧАЕТСЯ ОТ M-60a/M-61. Там мутировался shell-барьер, здесь — RUST:
# готового каркаса мутации в репозитории нет (замер: ни одного `[features]`, ни одного
# `env::var("HFT_*")` в `crates/*/src`). Поэтому мутация вносится в КОПИЮ дерева, а не в
# рабочее: прод-исходники не трогаются ни на одну секунду. Копия одна и переиспользуется —
# иначе каждый мутант тянул бы полную компиляцию зависимостей.
#
# РАЗЛИЧИЕ, КОТОРОГО МЕХАНИЗМ НЕ ВИДИТ, И ПОТОМУ ОНО НАЗВАНО ЗДЕСЬ. Сценарий может попасть в
# kill-set ДВУМЯ разными способами: (а) он проверил инвариант и тот нарушен — настоящий «kill»;
# (б) он умер на SETUP-GUARD'е, потому что мутант сделал предпосылку сценария недостижимой.
# Второе — случай `nocache` против `sm8`/`sm9`: эти сценарии стерегут ИНКРЕМЕНТАЛЬНУЮ ветку и
# требуют `is_fresh == true`, а `nocache` по определению форсит `false`. Под ним они не могут
# не упасть по построению. Включать их в kill-set правильно (иначе батарея краснеет на честном
# мутанте), но записью «пиннит дыру» это НЕ является: инвариант там не проверялся. Равенство
# множеств различить (а) от (б) не умеет — предел конструкции, а не недосмотр.
#
# АТРИБУЦИЯ ПО KILL-SET'У (урок M-61, `R-052` Б-4bis + адверсарный круг 11.08). Мутант обязан
# уронить РОВНО объявленные тесты — ни больше, ни меньше:
#   больше ⇒ мутант сломан сверх своей оси и доказывает не ту дыру (класс `quotedname`);
#   меньше ⇒ тест перестал ловить дефект, ради которого стоит.
# Проверка «мутант просто красный» этого класса не ловит — предъявлено на M-61 замером.
#
# ПОДСТАНОВКА, НЕ СРАБОТАВШАЯ МОЛЧА, — ОТКАЗ, А НЕ СТРОКА В ЛОГЕ (тот же урок): каждая правка
# сверяется с исходником, и совпадение = смерть батареи.
#
# ПРОБЕЛ ЗАКРЫТ ЗАМЕРОМ (R-053, круг 1 PR-гейта). Ниже — прежняя формулировка; она оказалась
# верной в постановке и НЕВЕРНОЙ в одной из двух развилок, и это стоит сохранить как есть.
# Вопрос стоял так: «либо инвалидация по компакции наблюдаема иначе, либо она и не нужна для
# корректности выдачи». Reviewer ответил замером: НУЖНА, и в инкрементальной ветке `is_fresh`
# живут ТРИ дефекта (Б-1 стирание сегмента, Б-2 дубль индекса, Б-3 падение на постороннем
# файле). Причина слепоты набора названа точно: SM-4/SM-5 компактировали через
# `compact_closed_segments` — все закрытые сегменты разом, diff большой, `small_change=false`,
# отрабатывал полный `refresh()`. Прод компактирует ПОСЕГМЕНТНО, и тик сессии попадает ровно
# в маленький diff. Оракулы SM-8/SM-9/SM-10 закрывают эту ветку; мутант на неё заводится
# ПОСЛЕ фикса — пока реализация красна, эталон батареи зелёным быть не может по построению.
#
# ПРЕЖНЯЯ ФОРМУЛИРОВКА (сохранена намеренно): `staleforever` ловится ТОЛЬКО через `sm4` (ротация).
# Компакционная половина оси 4 мутантом не пиннится: `sm5` получил поведенческую проверку
# (отставшая сессия обязана добрать все события после НАСТОЯЩЕЙ компакции, с setup-guard'ом
# на исчезновение `.jrnl`), и против честной реализации она зелена, но кеш БЕЗ инвалидации
# её всё равно проходит — устаревший перечень чтению не мешает. Значит либо инвалидация по
# компакции наблюдаема иначе, либо она и не нужна для корректности выдачи. Вопрос открыт и
# записан как есть: заявить `sm5` в kill-set'е, зная, что он не срабатывает, значило бы
# подделать атрибуцию — ровно то, против чего эта батарея построена.
#
# ПОЧЕМУ МУТАНТА `dirshared` ЗДЕСЬ НЕТ (решение architect'а, §4.4 спеки). Он был объявлен как
# «состав на каталог, а не на сессию» по аналогии с `F-035-2`. Замер аналогию опроверг: кеш
# ПЕРЕЧНЯ СЕГМЕНТОВ не зависит от позиции сессии, поэтому две сессии на ОДНОМ каталоге (форма
# SM-6) делят его безвредно — построенный мутант дал SM-2 = 7 операций при бюджете 8, то есть
# честно уложился. Красным он выглядел лишь в общем прогоне: глобальный слот толкали соседние
# ТЕСТЫ на своих каталогах, а не соседняя сессия, — падение по чужой причине, ровно тот класс,
# ради которого стоит сверка kill-set'ов. Свойство, которое §5 запрещает на самом деле, —
# общая ПОЗИЦИЯ (`tail_hint`), и оно закреплено оракулом M-57 `f035_2_two_sessions_do_not_share
# _one_cursor`, который шаг M гейта гоняет. Покрытие не потеряно, оно расположено в другом
# месте; молчаливо выбросить мутанта было бы недопустимо, поэтому вывод назван здесь.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
IMPL_REL="crates/journal/src/segments.rs"
ORACLE="red_segment_meta_bound"
# Каталог дерева-копии — СВОЙ на прогон. Фиксированный путь означал, что два одновременных
# прогона (а они здесь norm: гейт зовут и tester, и reviewer, и architect) топчут одно дерево:
# один восстанавливает `.orig` ровно тогда, когда другой применил мутацию. Это тот же класс,
# что фиксированные пути логов `verify`, из-за которых на M-61 шаг T переворачивался с красного
# на зелёное. Переопределяется через `M62_BATT_TREE` — переиспользовать прогретый target/
# по-прежнему можно, но ЯВНО и на свой страх.
TREE="${M62_BATT_TREE:-$(mktemp -d /tmp/m62-batt-XXXXXX)}"

pass() { echo "PASS  $*"; }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

# ─── МАНИФЕСТ: мутант|ось/значение|ОБЯЗАН уронить (kill-set) ─────────────────────────
# Оси — из §4.2 спеки; kill-set — имена тестов `red_segment_meta_bound`.
MANIFEST="
nocache|1 O(N) на каждом тике|sm2_steady_tick_is_independent_of_segment_count sm5_compaction_is_noticed_and_quiet_dir_is_not_rescanned sm6_two_sessions_alternating_ticks_stay_within_budget sm8_per_segment_compaction_keeps_catalog_truthful sm9_compaction_midstate_does_not_duplicate_index
staleforever|4 кеш без инвалидации|sm4_new_segment_between_ticks_is_noticed
countfake|3 счётчик считает вызовы, а не операции|sm1_counter_measures_operations_not_calls sm3_first_tick_legitimately_pays_full_price
"

# ─── мутации: python-патчи по ТОЧНОМУ тексту, с проверкой применения ────────────────
apply_mutation() {  # $1 = имя мутанта; правит "${TREE}/${IMPL_REL}"
  python3 - "$1" "${TREE}/${IMPL_REL}" <<'PY'
import sys
name, path = sys.argv[1], sys.argv[2]
src = open(path, encoding='utf-8').read()

FRESH_HEAD = ("    pub fn is_fresh(&mut self, dir: &Path) -> io::Result<(bool, SegmentOps)> {\n"
              "        let mut ops: SegmentOps = 0;\n")
OPEN_TAIL  = ("            },\n"
              "            ops,\n"
              "        ))\n")

if name == 'nocache':
    # Кеш никогда не признаётся свежим ⇒ каждый тик платит полный обход: O(N) всегда.
    new = FRESH_HEAD + "        return Ok((false, ops)); // МУТАНТ nocache\n"
    out = src.replace(FRESH_HEAD, new, 1)
elif name == 'staleforever':
    # Кеш признаётся свежим ВСЕГДА ⇒ появление/исчезновение сегмента не замечается.
    new = FRESH_HEAD + "        return Ok((true, ops)); // МУТАНТ staleforever\n"
    out = src.replace(FRESH_HEAD, new, 1)
elif name == 'countfake':
    # Счётчик рапортует ОДНУ операцию за вызов вместо реальных syscall'ов построения.
    out = src.replace(OPEN_TAIL, "            },\n            1, // МУТАНТ countfake\n        ))\n", 1)
elif name == 'dirshared':
    # Состояние живёт на КАТАЛОГ (process-global), а не на сессию: переданный сессией
    # каталог игнорируется, берётся общий слот. Сессий столько, сколько подключений.
    anchor = "pub fn stream_from_at_with_catalog(\n"
    if anchor not in src:
        print("ANCHOR-MISS", file=sys.stderr); sys.exit(3)
    shared = (
        "static SHARED_CATALOG: std::sync::Mutex<Option<(std::path::PathBuf, SegmentCatalog)>> =\n"
        "    std::sync::Mutex::new(None); // МУТАНТ dirshared\n\n"
    )
    out = src.replace(anchor, shared + anchor, 1)
    old_take = "    let (all, catalog_out) = match catalog_in {"
    if old_take not in out:
        print("TAKE-MISS", file=sys.stderr); sys.exit(3)
    new_take = (
        "    // МУТАНТ dirshared: каталог берётся из ОБЩЕГО слота, а сессионный отбрасывается.\n"
        "    let catalog_in = {\n"
        "        let mut g = SHARED_CATALOG.lock().unwrap();\n"
        "        match g.take() {\n"
        "            Some((d, c)) if d == dir => Some(c),\n"
        "            _ => None,\n"
        "        }\n"
        "    };\n"
        "    let (all, catalog_out) = match catalog_in {"
    )
    out = out.replace(old_take, new_take, 1)
else:
    print("UNKNOWN", file=sys.stderr); sys.exit(3)

if out == src:
    print("NO-OP", file=sys.stderr); sys.exit(3)
open(path, 'w', encoding='utf-8').write(out)
PY
}

# dirshared обязан ещё и класть каталог обратно в общий слот — иначе он просто «без кеша».
finish_dirshared() {
  python3 - "${TREE}/${IMPL_REL}" <<'PY'
import sys, re
path = sys.argv[1]
src = open(path, encoding='utf-8').read()
OLD_RET = "        catalog_out,\n    ))\n"
if OLD_RET not in src:
    print("RET-MISS", file=sys.stderr); sys.exit(3)
# Блок-выражение на месте значения: состояние уходит в ОБЩИЙ слот, сессии — None.
NEW_RET = ("        {\n"
           "            if let Some(c) = catalog_out {\n"
           "                *SHARED_CATALOG.lock().unwrap() = Some((dir.to_path_buf(), c));\n"
           "            }\n"
           "            None\n"
           "        },\n    ))\n")
open(path, 'w', encoding='utf-8').write(src.replace(OLD_RET, NEW_RET, 1))
PY
}

# ─── подготовка дерева-копии ────────────────────────────────────────────────────────
prepare_tree() {
  [ -f "${ROOT}/${IMPL_REL}" ] || die "нет реализации ${IMPL_REL} — мутировать нечего"
  [ -f "${ROOT}/crates/gateway/tests/${ORACLE}.rs" ] || die "нет оракула ${ORACLE}.rs (задача 5)"
  if [ ! -d "${TREE}/crates" ]; then
    mkdir -p "${TREE}" || die "mkdir ${TREE}"
    ( cd "${ROOT}" && tar -cf - crates Cargo.toml Cargo.lock 2>/dev/null ) \
      | ( cd "${TREE}" && tar -xf - ) || die "копия дерева не создана"
  else
    # обновляем исходники, кеш сборки (target/) сохраняем
    ( cd "${ROOT}" && tar -cf - crates Cargo.toml Cargo.lock 2>/dev/null ) \
      | ( cd "${TREE}" && tar -xf - ) || die "копия дерева не обновлена"
  fi
  cp "${TREE}/${IMPL_REL}" "${TREE}/${IMPL_REL}.orig" || die "не сохранён исходник"
}

# ПРОГОН и РАЗБОР разведены намеренно. Слитая версия вида `FAILS="$(run_oracle …)"`
# исполняла функцию в ПОДОБОЛОЧКЕ, и присваивание `RC=$?` в родителя не возвращалось —
# ровно тот класс, что дал мёртвый `cleanup()` в пробе M-61 (реестр в переменной вместо
# файла). Код возврата обязан пережить вызов, поэтому он ставится в родительской оболочке.
run_oracle() {  # $1 = лог; выставляет RC в РОДИТЕЛЬСКОЙ оболочке
  ( cd "${TREE}" && CARGO_TARGET_DIR="${TREE}/target" \
      cargo test -p gateway --no-fail-fast --test "${ORACLE}" >"$1" 2>&1 )
  RC=$?
}
fails_of() { grep -oE '^test [a-z0-9_]+ \.\.\. FAILED' "$1" | awk '{print $2}' | sort -u | tr '\n' ' '; }

# ═══ БАТАРЕЯ ════════════════════════════════════════════════════════════════════════
[ "${1:-}" = "--battery" ] || { echo "usage: $0 --battery" >&2; exit 2; }

prepare_tree
echo "══ БАТАРЕЯ M-62 (§4.4): эталон зелён, каждый мутант красен РОВНО по своему kill-set'у ══"
bad=0; n=0

# ── ЭТАЛОН ──────────────────────────────────────────────────────────────────────────
cp "${TREE}/${IMPL_REL}.orig" "${TREE}/${IMPL_REL}"
run_oracle "${TREE}/ref.log"; FAILS="$(fails_of "${TREE}/ref.log")"; n=$((n + 1))
if [ "${RC}" -eq 0 ] && [ -z "${FAILS// /}" ]; then
  pass "эталон → exit=0, $(grep -oE 'test result: ok\. [0-9]+ passed' "${TREE}/ref.log" | head -1)"
else
  echo "FAIL  эталон КРАСЕН (exit=${RC}), упали: ${FAILS:-—}"
  echo "      ↳ позитивный контроль сломан: батарея не вправе судить мутантов,"
  echo "      ↳ пока честная реализация не зелена (анти-плацебо §4.4 в обратную сторону)"
  grep -E '^(error|test .* FAILED)' "${TREE}/ref.log" | head -6 | sed 's/^/      ↳ /'
  bad=$((bad + 1))
fi

# ── МУТАНТЫ ─────────────────────────────────────────────────────────────────────────
while IFS='|' read -r m axis want; do
  [ -z "${m}" ] && continue
  n=$((n + 1))
  cp "${TREE}/${IMPL_REL}.orig" "${TREE}/${IMPL_REL}"
  if ! apply_mutation "${m}"; then
    echo "FAIL  ${m}: мутация НЕ ПРИМЕНИЛАСЬ (подстановка не совпала с исходником)"
    echo "      ↳ молча не сработавшая правка даёт мутант, равный эталону, — он тестировал бы"
    echo "      ↳ эталон под чужим именем; это отказ батареи, а не строка в логе"
    bad=$((bad + 1)); continue
  fi
  [ "${m}" = dirshared ] && { finish_dirshared || { echo "FAIL  ${m}: вторая половина мутации не применилась"; bad=$((bad+1)); continue; }; }
  if cmp -s "${TREE}/${IMPL_REL}" "${TREE}/${IMPL_REL}.orig"; then
    echo "FAIL  ${m}: файл СОВПАЛ с эталоном после мутации"; bad=$((bad + 1)); continue
  fi

  run_oracle "${TREE}/${m}.log"; GOT="$(fails_of "${TREE}/${m}.log")"
  WANT="$(printf '%s\n' "${want}" | tr ' ' '\n' | grep . | sort -u | tr '\n' ' ')"
  if grep -qE 'error\[E[0-9]+\]|could not compile' "${TREE}/${m}.log"; then
    echo "FAIL  ${m}: мутант НЕ КОМПИЛИРУЕТСЯ — это не дыра, а сломанная правка"
    grep -E '^error(\[|:)' "${TREE}/${m}.log" | head -3 | sed 's/^/      ↳ /'
    bad=$((bad + 1)); continue
  fi
  if [ "${GOT}" = "${WANT}" ] && [ "${RC}" -ne 0 ]; then
    pass "${m} → exit=${RC}, ось ${axis}, уронил ровно объявленное: ${GOT}"
  else
    echo "FAIL  ${m}: kill-set РАЗОШЁЛСЯ с манифестом (exit=${RC})"
    echo "      ↳ объявлено:   ${WANT:-—}"
    echo "      ↳ наблюдается: ${GOT:-— (ни одного теста)}"
    [ -z "${GOT// /}" ] && echo "      ↳ пусто ⇒ дыра НЕ закреплена: набор не отличает мутанта от честной реализации"
    bad=$((bad + 1))
  fi
done <<< "$(printf '%s\n' "${MANIFEST}" | grep '|')"

cp "${TREE}/${IMPL_REL}.orig" "${TREE}/${IMPL_REL}"
echo
[ "${bad}" -gt 0 ] && { echo "BATTERY: FAIL (${bad} из ${n})"; exit 1; }
echo "BATTERY: PASS (${n}/${n})"
