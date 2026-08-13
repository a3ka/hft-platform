#!/usr/bin/env bash
# Батарея мутантов M-62 (`TD-120`), задачи 7 и 12 спеки §3, критерий — §4.4/§4.5/§4.5bis.
#
# ЗАЧЕМ. RED-набор зелен против реализации задач 1-3 и 14-15. «Зелёный оракул» и «оракул,
# который ЛОВИТ», — разные утверждения (`A-005` §6.5: четыре круга M-60a сертифицировались
# пробой, не существовавшей ни в одной ревизии). Батарея предъявляет второе: каждый мутант —
# правка РЕАЛИЗАЦИИ, названная осью и значением, и набор обязан на ней краснеть. Эталон
# (честное удешевление, без мутаций) обязан зеленеть — анти-плацебо в ОБЕ стороны (§4.4):
# `td083` однажды покраснел на строгом УЛУЧШЕНИИ, потому что мерил отношение.
#
# ЧЕМ ЭТА БАТАРЕЯ ОТЛИЧАЕТСЯ ОТ M-60a/M-61. Там мутировался shell-барьер, здесь — RUST:
# готового каркаса мутации в репозитории нет (замер: ни одного `[features]`, ни одного
# `env::var("HFT_*")` в `crates/*/src`). Поэтому мутация вносится в КОПИЮ дерева, а не в
# рабочее: прод-исходники не трогаются ни на одну секунду. Копия одна и переиспользуется —
# иначе каждый мутант тянул бы полную компиляцию зависимостей.
#
# ═══ ЧТО ИЗМЕНИЛ КРУГ 3 (2026-08-13) ═══════════════════════════════════════════════════
#
# 1. ОРАКУЛОВ ДВА, И ЭТО НЕ УДОБСТВО, А УСЛОВИЕ СУЩЕСТВОВАНИЯ МАНИФЕСТА. Прежняя батарея
#    была зашита на `red_segment_meta_bound`. Замер круга 3: из мутантов классов A и B
#    ПЯТЬ не роняют в этом наборе ни одного теста — их kill-set пуст ПО ПОСТРОЕНИЮ, и
#    батарея объявила бы их «не пойманными», хотя ловит их другой оракул. Теперь
#    `ORACLES` — список, а kill-set мутанта есть ОБЪЕДИНЕНИЕ упавших по всем оракулам.
# 2. МУТАНТЫ КЛАССОВ A и B ЯКОРЯТСЯ НА ФОРМУ ФИКСА. Спека (§4.5bis, задачи 14-15) требует
#    от реализации ДВЕ именованные точки: `reattach_survivor(...)` (класс A) и
#    `self.validate_like_full_path(...)` (класс B). Реализация вправе назвать их иначе — но
#    тогда обязана перенести якоря здесь: подстановка, не совпавшая с исходником, есть
#    ОТКАЗ батареи, а не строка в логе. Мутант, равный эталону, тестировал бы эталон под
#    чужим именем.
# 3. KILL-SET'Ы ЗАМЕРЕНЫ, А НЕ ВЫВЕДЕНЫ ПО АНАЛОГИИ. Таблица — §4.5bis спеки; там же назван
#    ЕДИНСТВЕННЫЙ пункт, полученный смертью на setup-guard'е (`countfake` → `sm8`), и три
#    мутанта с ПУСТЫМ kill-set'ом (`namescommit`, `classifyskip`, `addfirst`), которые
#    поэтому в манифест НЕ внесены.
#
# РАЗЛИЧИЕ, КОТОРОГО МЕХАНИЗМ НЕ ВИДИТ, И ПОТОМУ ОНО НАЗВАНО ЗДЕСЬ. Сценарий может попасть в
# kill-set ДВУМЯ разными способами: (а) он проверил инвариант и тот нарушен — настоящий «kill»;
# (б) он умер на SETUP-GUARD'е, потому что мутант сделал предпосылку сценария недостижимой.
# Равенство множеств различить (а) от (б) не умеет — предел конструкции, а не недосмотр.
# После переделки setup-guard'ов круга 3 (`assert!(fresh)` → сторож БЮДЖЕТА) такой пункт
# остался ровно один и назван в §4.5bis.
#
# ВТОРОЙ ПРЕДЕЛ, НАЗВАННЫЙ ЧЕСТНО: `indexblind` ≡ `pathblind` и `guardskip` ≡ `guardstub` по
# kill-set'у. Правило «уронил РОВНО объявленное» на каждом из них выполняется, но АТРИБУЦИЯ
# внутри пары не работает: `red_catalog_equivalence` гоняет всю последовательность оси 6
# одним `#[test]`. Лечится разложением цикла по буквам оси на отдельные `#[test]` — §9 спеки.
#
# АТРИБУЦИЯ ПО KILL-SET'У (урок M-61, `R-052` Б-4bis). Мутант обязан уронить РОВНО объявленные
# тесты — ни больше, ни меньше:
#   больше ⇒ мутант сломан сверх своей оси и доказывает не ту дыру (класс `quotedname`);
#   меньше ⇒ тест перестал ловить дефект, ради которого стоит.
#
# ПРОБЕЛ ЗАКРЫТ ЗАМЕРОМ (R-053, круг 1 PR-гейта). Ниже — прежняя формулировка; она оказалась
# верной в постановке и НЕВЕРНОЙ в одной из двух развилок, и это стоит сохранить как есть.
# Вопрос стоял так: «либо инвалидация по компакции наблюдаема иначе, либо она и не нужна для
# корректности выдачи». Reviewer ответил замером: НУЖНА, и в инкрементальной ветке `is_fresh`
# живут ТРИ дефекта (Б-1 стирание сегмента, Б-2 дубль индекса, Б-3 падение на постороннем
# файле). Причина слепоты набора названа точно: SM-4/SM-5 компактировали через
# `compact_closed_segments` — все закрытые сегменты разом, diff большой, `small_change=false`,
# отрабатывал полный `refresh()`. Прод компактирует ПОСЕГМЕНТНО, и тик сессии попадает ровно
# в маленький diff. Круг 3 добавил к этому четвёртый дефект (Б-4) и ВТОРОЙ КЛАСС — проверки
# полного пути, не унаследованные инкрементальной веткой.
#
# ПОЧЕМУ МУТАНТА `dirshared` ЗДЕСЬ НЕТ (решение architect'а, §4.4 спеки). Он был объявлен как
# «состав на каталог, а не на сессию» по аналогии с `F-035-2`. Замер аналогию опроверг: кеш
# ПЕРЕЧНЯ СЕГМЕНТОВ не зависит от позиции сессии, поэтому две сессии на ОДНОМ каталоге (форма
# SM-6) делят его безвредно — построенный мутант дал SM-2 = 7 операций при бюджете 8, то есть
# честно уложился. Красным он выглядел лишь в общем прогоне: глобальный слот толкали соседние
# ТЕСТЫ на своих каталогах, а не соседняя сессия, — падение по чужой причине. Свойство, которое
# §5 запрещает на самом деле, — общая ПОЗИЦИЯ (`tail_hint`), и оно закреплено оракулом M-57
# `f035_2_two_sessions_do_not_share_one_cursor`, который шаг M гейта гоняет. Покрытие не
# потеряно, оно расположено в другом месте; молчаливо выбросить мутанта было бы недопустимо,
# поэтому вывод назван здесь. Код мутации сохранён ниже намеренно — как след замера.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
IMPL_REL="crates/journal/src/segments.rs"
# Оракулы перечислены ЯВНО. Добавление файла в `crates/gateway/tests/` его сюда не заносит:
# батарея обязана знать, ЧТО она судит, а не подбирать это по маске.
ORACLES="red_segment_meta_bound red_catalog_equivalence"
# Каталог дерева-копии — СВОЙ на прогон. Фиксированный путь означал, что два одновременных
# прогона (а они здесь norm: гейт зовут и tester, и reviewer, и architect) топчут одно дерево:
# один восстанавливает `.orig` ровно тогда, когда другой применил мутацию. Это тот же класс,
# что фиксированные пути логов `verify`, из-за которых на M-61 шаг T переворачивался с красного
# на зелёное. Переопределяется через `M62_BATT_TREE` — переиспользовать прогретый target/
# по-прежнему можно, но ЯВНО и на свой страх.
TREE="${M62_BATT_TREE:-$(mktemp -d /tmp/m62-batt-XXXXXX)}"
# Кеш сборки по умолчанию живёт ВНУТРИ дерева-копии (изоляция от рабочего). Переопределяется
# так же явно: общий `CARGO_TARGET_DIR` экономит минуты, но два прогона в нём встанут в
# очередь на блокировке — это осознанный размен, а не дефолт.
BATT_TARGET="${M62_BATT_TARGET:-${TREE}/target}"

pass() { echo "PASS  $*"; }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

# ─── МАНИФЕСТ: мутант|ось/значение|ОБЯЗАН уронить (kill-set) ─────────────────────────
# Оси — из §4.2 спеки; kill-set — имена тестов ЛЮБОГО из `ORACLES`, ЗАМЕРЕННЫЕ (§4.5bis).
# Мутанты с ПУСТЫМ замеренным kill-set'ом (`namescommit`, `classifyskip`, `addfirst`) сюда
# не вносятся: пустой kill-set = «набор не отличает мутанта от честной реализации», и
# объявить его значило бы подделать атрибуцию. Их предмет назван в §9 спеки как непокрытый.
MANIFEST="
nocache|1 O(N) на каждом тике|sm2_steady_tick_is_independent_of_segment_count sm5_compaction_is_noticed_and_quiet_dir_is_not_rescanned sm6_two_sessions_alternating_ticks_stay_within_budget sm8_per_segment_compaction_keeps_catalog_truthful sm9_compaction_midstate_does_not_duplicate_index sm11_warm_catalog_stays_equivalent_to_cold_across_tact_sequence sm11c_warm_verdict_inherits_fail_closed_guard_of_cold_path
staleforever|4 кеш без инвалидации|sm4_new_segment_between_ticks_is_noticed sm8_per_segment_compaction_keeps_catalog_truthful sm11_warm_catalog_stays_equivalent_to_cold_across_tact_sequence sm11c_warm_verdict_inherits_fail_closed_guard_of_cold_path
countfake|1 счётчик считает вызовы, а не операции|sm1_counter_measures_operations_not_calls sm3_first_tick_legitimately_pays_full_price sm8_per_segment_compaction_keeps_catalog_truthful sm11_warm_catalog_stays_equivalent_to_cold_across_tact_sequence sm11c_warm_verdict_inherits_fail_closed_guard_of_cold_path
indexblind|6 удаление по индексу слепо к ВЫЖИВШИМ на диске (Б-4)|sm8_per_segment_compaction_keeps_catalog_truthful sm11_warm_catalog_stays_equivalent_to_cold_across_tact_sequence
pathblind|6 удаление пропущено, выживший НЕ переклассифицирован (развязка №1)|sm8_per_segment_compaction_keeps_catalog_truthful sm11_warm_catalog_stays_equivalent_to_cold_across_tact_sequence
nodedup|4 D-COMP-1 не применяется в инкрементальной ветке|sm8_per_segment_compaction_keeps_catalog_truthful sm9_compaction_midstate_does_not_duplicate_index sm11_warm_catalog_stays_equivalent_to_cold_across_tact_sequence
guardskip|6 проверки полного пути не переоцениваются (I2)|sm11c_warm_verdict_inherits_fail_closed_guard_of_cold_path
guardstub|6 проверка зовётся, её отказ проглочен (I2)|sm11c_warm_verdict_inherits_fail_closed_guard_of_cold_path
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
# Якоря ФОРМЫ ФИКСА круга 3 (§4.5bis п.3) — ИМЕНА точек, а не их вёрстка. Держатся
# отдельными константами, чтобы переименование в реализации правилось здесь одной строкой.
REATTACH = "reattach_survivor("
VALIDATE = "validate_like_full_path("


def stmt_span(src, needle, frm=0):
    """Границы ОПЕРАТОРА, содержащего `needle`: от начала его строки до `;` включительно.

    Якорь по ИМЕНИ, а не по точному тексту, — не удобство, а требование корректности:
    вызов `reattach_survivor(&mut self.segments, dir, idx, &cur_names, &manifest, &mut ops)?;`
    длиннее 100 колонок, и `rustfmt` (шаг `T` гейта его требует) разложит его на пять строк.
    Подстановка по точному однострочному тексту после этого молча не сработала бы — а
    молча не сработавшая мутация даёт мутант, РАВНЫЙ эталону.
    """
    i = frm - 1
    while True:
        i = src.find(needle, i + 1)
        if i < 0:
            return None
        beg = src.rfind("\n", 0, i) + 1
        # ОБЪЯВЛЕНИЕ — не вызов. Без этой проверки мутация вырезала бы `fn reattach_survivor(`
        # до первой `;` внутри тела, если реализация объявит функцию ВЫШЕ точки вызова, —
        # и мутант «удалено удаление» на деле сломал бы компиляцию в другом месте.
        if "fn " not in src[beg:i]:
            break
    end = src.find(";", i)
    if end < 0:
        return None
    end += 1
    if src[end:end + 1] == "\n":
        end += 1
    return beg, end


def indent_of(line):
    return line[:len(line) - len(line.lstrip())]

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
elif name == 'indexblind':
    # КЛАСС A: удаление по ИНДЕКСУ, слепое к выжившим на диске файлам того же индекса —
    # ровно дефект `3115628` (Б-4 и его зеркало «удалён .zst при живом .jrnl»).
    sp = stmt_span(src, REATTACH)
    if sp is None:
        print("ANCHOR-MISS: reattach_survivor", file=sys.stderr); sys.exit(3)
    out = src[:sp[0]] + src[sp[1]:]
elif name == 'pathblind':
    # КЛАСС A: развязка №1 из `R-056` («не удалять запись, если индекс ещё присутствует в
    # cur_names») БЕЗ переклассификации. Множества ИНДЕКСОВ она сохраняет — и потому
    # сверка одних индексов её пропускает; красное приходит по ПУТИ и по ВЫДАЧЕ (ENOENT).
    sp = stmt_span(src, REATTACH)
    if sp is None:
        print("ANCHOR-MISS: reattach_survivor", file=sys.stderr); sys.exit(3)
    out = src[:sp[0]] + src[sp[1]:]
    # Удаление становится условным: индекс, у которого на диске остался ЛЮБОЙ файл, не
    # трогается вовсе — и запись продолжает адресовать УДАЛЁННЫЙ файл.
    loop = out.find("for removed_name")
    rt = stmt_span(out, "self.segments.retain(", loop if loop >= 0 else 0)
    if loop < 0 or rt is None:
        print("ANCHOR-MISS: retain в ветке удаления", file=sys.stderr); sys.exit(3)
    stmt = out[rt[0]:rt[1]]
    ind = indent_of(stmt)
    new = (f"{ind}// МУТАНТ pathblind (= развязка №1): запись удаляется, только если у индекса\n"
           f"{ind}// на диске не осталось НИ ОДНОГО имени; путь выжившей не переоценивается.\n"
           f"{ind}if !cur_names\n"
           f"{ind}    .iter()\n"
           f"{ind}    .any(|n| parse_segment_index_any(n) == Some(idx))\n"
           f"{ind}{{\n"
           f"{ind}    {stmt.strip()}\n"
           f"{ind}}}\n")
    out = out[:rt[0]] + new + out[rt[1]:]
elif name == 'nodedup':
    # Ось 4: правило D-COMP-1 (при коллизии индекса побеждает СЫРОЙ) в инкрементальной
    # ветке не применяется — кеш начинает адресовать `.zst` там, где полный путь берёт сырой.
    old = ("            if existing_is_raw && new_is_zst {\n"
           "                // D-COMP-1: в кеше уже сырой сегмент этого индекса — оставляем его.\n"
           "                continue;\n"
           "            }\n")
    out = src.replace(old, "", 1)
elif name == 'guardskip':
    # КЛАСС B: инкрементальная ветка перестаёт переоценивать проверки полного пути.
    sp = stmt_span(src, VALIDATE)
    if sp is None:
        print("ANCHOR-MISS: validate_like_full_path", file=sys.stderr); sys.exit(3)
    out = src[:sp[0]] + src[sp[1]:]
elif name == 'guardstub':
    # КЛАСС B, сильнее предыдущего: вызов НА МЕСТЕ, но его отказ проглочен. Оракул обязан
    # пиннить РЕЗУЛЬТАТ проверки, а не наличие строки с её именем.
    sp = stmt_span(src, VALIDATE)
    if sp is None:
        print("ANCHOR-MISS: validate_like_full_path", file=sys.stderr); sys.exit(3)
    stmt = src[sp[0]:sp[1]]
    ind = indent_of(stmt)
    body = stmt.strip().rstrip(";").rstrip()
    if not body.endswith("?"):
        print("ANCHOR-MISS: вызов guard'а без `?` — fail-closed не выражен", file=sys.stderr); sys.exit(3)
    out = src[:sp[0]] + f"{ind}let _ = {body[:-1]}; // МУТАНТ guardstub\n" + src[sp[1]:]
elif name == 'dirshared':
    # ВЫВЕДЕН из манифеста (см. шапку) — код сохранён как след замера, а не как мутант.
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
import sys
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
  for o in ${ORACLES}; do
    [ -f "${ROOT}/crates/gateway/tests/${o}.rs" ] || die "нет оракула ${o}.rs (задачи 5/11)"
  done
  if [ ! -d "${TREE}/crates" ]; then
    mkdir -p "${TREE}" || die "mkdir ${TREE}"
  fi
  # обновляем исходники; кеш сборки (target/) сохраняем, если он был
  ( cd "${ROOT}" && tar -cf - crates Cargo.toml Cargo.lock 2>/dev/null ) \
    | ( cd "${TREE}" && tar -xf - ) || die "копия дерева не создана"
  cp "${TREE}/${IMPL_REL}" "${TREE}/${IMPL_REL}.orig" || die "не сохранён исходник"
}

# ПРОГОН и РАЗБОР разведены намеренно. Слитая версия вида `FAILS="$(run_oracle …)"`
# исполняла функцию в ПОДОБОЛОЧКЕ, и присваивание `RC=$?` в родителя не возвращалось —
# ровно тот класс, что дал мёртвый `cleanup()` в пробе M-61 (реестр в переменной вместо
# файла). Код возврата обязан пережить вызов, поэтому он ставится в родительской оболочке.
#
# ОРАКУЛЫ ГОНЯЮТСЯ ВСЕ, а не до первого красного: kill-set есть ОБЪЕДИНЕНИЕ упавших, и
# остановка на первом сделала бы его зависимым от ПОРЯДКА перечисления.
run_oracle() {  # $1 = префикс лога; выставляет RC в РОДИТЕЛЬСКОЙ оболочке
  RC=0
  for o in ${ORACLES}; do
    ( cd "${TREE}" && CARGO_TARGET_DIR="${BATT_TARGET}" \
        cargo test -p gateway --no-fail-fast --test "${o}" >"$1.${o}.log" 2>&1 )
    local rc=$?
    [ "${rc}" -ne 0 ] && RC="${rc}"
  done
}
fails_of() {  # $1 = префикс лога
  local f=""
  for o in ${ORACLES}; do
    f="${f} $(grep -oE '^test [a-z0-9_]+ \.\.\. FAILED' "$1.${o}.log" 2>/dev/null | awk '{print $2}')"
  done
  printf '%s\n' ${f} | grep . | sort -u | tr '\n' ' '
}
compile_broken() {  # $1 = префикс лога
  for o in ${ORACLES}; do
    grep -qE 'error\[E[0-9]+\]|could not compile' "$1.${o}.log" 2>/dev/null && return 0
  done
  return 1
}

# ═══ БАТАРЕЯ ════════════════════════════════════════════════════════════════════════
[ "${1:-}" = "--battery" ] || { echo "usage: $0 --battery" >&2; exit 2; }

prepare_tree
echo "══ БАТАРЕЯ M-62 (§4.4/§4.5bis): эталон зелён, каждый мутант красен РОВНО по своему kill-set'у ══"
echo "   оракулы: ${ORACLES}"
bad=0; n=0

# ── ЭТАЛОН ──────────────────────────────────────────────────────────────────────────
cp "${TREE}/${IMPL_REL}.orig" "${TREE}/${IMPL_REL}"
run_oracle "${TREE}/ref"; FAILS="$(fails_of "${TREE}/ref")"; n=$((n + 1))
if [ "${RC}" -eq 0 ] && [ -z "${FAILS// /}" ]; then
  pass "эталон → exit=0, $(grep -hoE 'test result: ok\. [0-9]+ passed' "${TREE}"/ref.*.log | tr '\n' ' ')"
else
  echo "FAIL  эталон КРАСЕН (exit=${RC}), упали: ${FAILS:-—}"
  echo "      ↳ позитивный контроль сломан: батарея не вправе судить мутантов,"
  echo "      ↳ пока честная реализация не зелена (анти-плацебо §4.4 в обратную сторону)"
  grep -hE '^(error|test .* FAILED)' "${TREE}"/ref.*.log | head -6 | sed 's/^/      ↳ /'
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
    echo "      ↳ для мутантов классов A/B причина почти всегда одна: реализация назвала точку"
    echo "      ↳ иначе, чем требует §4.5bis п.3 (reattach_survivor / validate_like_full_path)"
    bad=$((bad + 1)); continue
  fi
  [ "${m}" = dirshared ] && { finish_dirshared || { echo "FAIL  ${m}: вторая половина мутации не применилась"; bad=$((bad+1)); continue; }; }
  if cmp -s "${TREE}/${IMPL_REL}" "${TREE}/${IMPL_REL}.orig"; then
    echo "FAIL  ${m}: файл СОВПАЛ с эталоном после мутации"; bad=$((bad + 1)); continue
  fi

  run_oracle "${TREE}/${m}"; GOT="$(fails_of "${TREE}/${m}")"
  WANT="$(printf '%s\n' "${want}" | tr ' ' '\n' | grep . | sort -u | tr '\n' ' ')"
  if compile_broken "${TREE}/${m}"; then
    echo "FAIL  ${m}: мутант НЕ КОМПИЛИРУЕТСЯ — это не дыра, а сломанная правка"
    grep -hE '^error(\[|:)' "${TREE}/${m}".*.log | head -3 | sed 's/^/      ↳ /'
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
