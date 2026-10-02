#!/usr/bin/env bash
# Проба барьера `scripts/check_ps_on_archive.sh` — АНТИ-ПЛАЦЕБО В ОБЕ СТОРОНЫ.
#
# Предъявляются ОБА свойства:
#   · КРАСНЕЕТ, когда спека уехала в архив, а `PROJECT-STATE.md` закрытия не записал
#     (нет маркера, маркер «открыт», маркер только в цитате/блоке кода/рабочем дереве,
#     маркер чужого номера, заголовок раздела всё ещё твердит «close-out за architect»);
#   · МОЛЧИТ на честной работе — включая ИСТОРИЧЕСКУЮ ПРОЗУ, где «НЕ ЗАКРЫТ» стоит в цитате
#     (форма раздела M-69 на дереве 2026-10-02) или относится к другому предмету (M-88).
#     Барьер, краснеющий на честном реестре, будет обойдён — это не мягкость, а условие жизни.
# Плюс РЕПЛЕЙ реальных переездов M-69/M-75/M-77/M-85/M-87: барьер обязан покраснеть на каждом —
# это прод-форма инцидента, а не суррогат.
#
# Каждый сценарий зовёт барьер ТОЙ ЖЕ проводкой, что CI (`EVENT_NAME`/`PUSH_BEFORE`).
# `PS_BARRIER=<файл>` — ТОЛЬКО для мутационного контроля: подменяет испытуемый барьер копией
# с нейтрализованной проверкой. Прод-форма (CI) зовёт пробу без переменной.
#
# Число сценариев НЕ ЗАЯВЛЯЕТСЯ — оно СЧИТАЕТСЯ и печатается в итоге.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BARRIER="${PS_BARRIER:-$ROOT/scripts/check_ps_on_archive.sh}"
[ -f "$BARRIER" ] || { echo "SETUP НЕ СОСТОЯЛСЯ: барьер $BARRIER отсутствует — проба судила бы пустоту"; exit 1; }

PASS=0; FAIL=0
ok()   { printf 'pass  %s\n' "$*"; PASS=$((PASS+1)); }
nope() { printf 'FAIL  %s\n' "$*"; FAIL=$((FAIL+1)); }

# Реестр фикстур — В ФАЙЛЕ, уборка по trap: каталоги и временные worktree реального репо.
SANDBOX="$(mktemp -d)"
WT_LIST="$SANDBOX/.worktrees"; : > "$WT_LIST"
cleanup() {
  while read -r wt; do [ -n "$wt" ] && git -C "$ROOT" worktree remove --force "$wt" >/dev/null 2>&1; done < "$WT_LIST"
  git -C "$ROOT" worktree prune >/dev/null 2>&1
  rm -rf "$SANDBOX"
}
trap cleanup EXIT

Q=$'\x60'   # обратная кавычка — чтобы не путать heredoc

# Песочница: base-коммит со спекой M-NN в milestones/ и реестром $PS_BASE;
# событие — по флагам. Печатает exit барьера либо 99 при несостоявшемся setup.
#   $1 id           — номер милестоуна (M-69)
#   $2 do_archive   — yes|no
#   $3 ps_base      — функция-генератор PS в базе
#   $4 ps_event     — функция-генератор PS в событии ("-" = не трогать)
#   $5 extra        — доп. действие: uncommitted | rm_ps | archive_too:<id> | ""
run_case() {
  local id="$1" do_archive="$2" ps_base="$3" ps_event="$4" extra="${5:-}" dir base
  dir="$SANDBOX/case-$RANDOM$RANDOM"
  mkdir -p "$dir/scripts" "$dir/milestones" "$dir/docs/archive" || { echo 99; return; }
  git -C "$dir" init -q 2>/dev/null || { echo 99; return; }
  git -C "$dir" config user.email t@t.local; git -C "$dir" config user.name t
  cp "$BARRIER" "$dir/scripts/check_ps_on_archive.sh"
  printf '# %s — предмет пробы\n' "$id" > "$dir/milestones/$id-probe.md"
  case "$extra" in archive_too:*) local other="${extra#archive_too:}"
    printf '# %s\n' "$other" > "$dir/docs/archive/$other-old.md" ;; esac
  "$ps_base" "$id" > "$dir/PROJECT-STATE.md"
  git -C "$dir" add -A >/dev/null 2>&1; git -C "$dir" commit -qm base >/dev/null 2>&1 || { echo 99; return; }
  base="$(git -C "$dir" rev-parse HEAD)"

  if [ "$do_archive" = "yes" ]; then
    git -C "$dir" mv "milestones/$id-probe.md" "docs/archive/$id-probe.md" >/dev/null 2>&1 || { echo 99; return; }
  else
    printf 'правка, не связанная с close-out\n' >> "$dir/milestones/$id-probe.md"
  fi
  if [ "$ps_event" != "-" ] && [ "$extra" != "uncommitted" ]; then "$ps_event" "$id" > "$dir/PROJECT-STATE.md"; fi
  [ "$extra" = "rm_ps" ] && git -C "$dir" rm -q PROJECT-STATE.md >/dev/null 2>&1
  git -C "$dir" add -A >/dev/null 2>&1; git -C "$dir" commit -qm event >/dev/null 2>&1 || { echo 99; return; }
  # SETUP-guard: переезд обязан реально стоять в диапазоне, иначе сценарий судит не то.
  if [ "$do_archive" = "yes" ] && [ -z "$(git -C "$dir" diff --no-renames --name-only --diff-filter=A "$base..HEAD" -- "docs/archive/$id-probe.md")" ]; then echo 99; return; fi
  # Незакоммиченная правка реестра — ПОСЛЕ коммита, в рабочем дереве.
  [ "$extra" = "uncommitted" ] && "$ps_event" "$id" > "$dir/PROJECT-STATE.md"

  ( cd "$dir" && EVENT_NAME=push PUSH_BEFORE="$base" bash scripts/check_ps_on_archive.sh >/dev/null 2>&1; echo "$?" )
}

expect() {
  local name="$1" want="$2" got="$3"
  if [ "$got" = "99" ]; then nope "$name — SETUP не состоялся, сценарий не судил предмет"
  elif [ "$got" = "$want" ]; then ok "$name"
  else nope "$name — ожидался exit=$want, получен exit=$got"; fi
}

# ── генераторы реестра ──────────────────────────────────────────────────────────────
ps_empty()        { printf '# PROJECT-STATE\n\n## Инфраструктура\nтекст\n'; }
ps_open_phrase()  { printf '# PROJECT-STATE\n\n## %s «предмет» — КОД В MAIN; milestone **НЕ ЗАКРЫТ — close-out за architect'"'"'ом**\nтекст\n' "$1"; }
ps_closed()       { printf '# PROJECT-STATE\n\n## %s «предмет» — КОД В MAIN; milestone **✅ ЗАКРЫТ**\n<!-- MS-STATE: %s CLOSED -->\nтекст\n' "$1" "$1"; }
ps_closed_prose_only() { printf '# PROJECT-STATE\n\n## %s «предмет» — milestone **✅ ЗАКРЫТ**\nтекст\n' "$1"; }
ps_marker_open()  { printf '# PROJECT-STATE\n\n## %s «предмет» — milestone **✅ ЗАКРЫТ**\n<!-- MS-STATE: %s OPEN -->\n' "$1" "$1"; }
ps_closed_stale_heading() { printf '# PROJECT-STATE\n\n## %s «предмет» — milestone **НЕ ЗАКРЫТ — close-out за architect'"'"'ом**\n<!-- MS-STATE: %s CLOSED -->\n' "$1" "$1"; }
# Форма M-69 на дереве 2026-10-02: фраза стоит в цитате — историческая проза, не утверждение.
ps_closed_quoted() { printf '# PROJECT-STATE\n\n## %s «предмет» — milestone **✅ ЗАКРЫТ** — переезд СОСТОЯЛСЯ. Строка «НЕ ЗАКРЫТ — close-out за architect'"'"'ом» стояла здесь **38 суток после переезда**\n<!-- MS-STATE: %s CLOSED -->\n' "$1" "$1"; }
# Форма M-88: «НЕ ЗАКРЫТ» относится к ДРУГОМУ предмету, а не к милестоуну.
ps_closed_other_subject() { printf '# PROJECT-STATE\n\n## %s «предмет» — ✅ **МИЛЕСТОУН ЗАКРЫТ**; **предмет %sP0-CORR%s этим НЕ закрыт — открыт TD-217**\n<!-- MS-STATE: %s CLOSED -->\n' "$1" "$Q" "$Q" "$1"; }
# Фраза в %sкоде%s — тоже цитата.
ps_closed_code_quote() { printf '# PROJECT-STATE\n\n## %s — ✅ ЗАКРЫТ; прежняя строка %sНЕ ЗАКРЫТ — close-out за architect%s снята\n<!-- MS-STATE: %s CLOSED -->\n' "$1" "$Q" "$Q" "$1"; }
ps_closed_in_fence() { printf '# PROJECT-STATE\n\n## %s — milestone **✅ ЗАКРЫТ**\nпример маркера:\n%s%s%s\n<!-- MS-STATE: %s CLOSED -->\n%s%s%s\n' "$1" "$Q" "$Q" "$Q" "$1" "$Q" "$Q" "$Q"; }
ps_closed_wrong_id() { printf '# PROJECT-STATE\n\n## %s0 — milestone **✅ ЗАКРЫТ**\n<!-- MS-STATE: %s0 CLOSED -->\n## %s — ✅ ЗАКРЫТ\n' "$1" "$1" "$1"; }
ps_closed_suffix_id() { printf '# PROJECT-STATE\n\n<!-- MS-STATE: %sa CLOSED -->\n## %s — ✅ ЗАКРЫТ\n' "$1" "$1"; }
ps_malformed()    { printf '# PROJECT-STATE\n\n## %s — ✅ ЗАКРЫТ\n<!-- MS-STATE: %s closed -->\n<!-- MS-STATE: %s CLOSED -->\n' "$1" "$1" "$1"; }
# Соседний номер в той же песочнице: у M-NN0 открытый раздел со старой фразой — к M-NN не относится.
ps_closed_neighbor_open() { printf '# PROJECT-STATE\n\n## %s — ✅ ЗАКРЫТ\n<!-- MS-STATE: %s CLOSED -->\n## %s0 — **НЕ ЗАКРЫТ — close-out за architect'"'"'ом**\n<!-- MS-STATE: %s0 OPEN -->\n' "$1" "$1" "$1" "$1"; }
ps_live_open()    { printf '# PROJECT-STATE\n\n## %s — в работе\n<!-- MS-STATE: %s OPEN -->\n' "$1" "$1"; }
# ── C-272 B-1: маркер принадлежит РАЗДЕЛУ своего милестоуна ─────────────────────────
# Владелец раздела — ПЕРВЫЙ номер в `## `-заголовке (на реальном реестре заголовок M-89 упоминает
# ещё M-62 и M-90: «заголовок называет номер» засчитал бы маркер M-90 в разделе M-89).
# Дословная фикстура критика C-272: валидный маркер M-69 в разделе M-68, раздел M-69 «в работе».
ps_foreign_marker() { printf '# PROJECT-STATE\n\n## M-68 unrelated item — CLOSED\n<!-- MS-STATE: %s CLOSED -->\n\n## %s probe — still IN PROGRESS; reviewer has not closed it\n' "$1" "$1"; }
# Номер упомянут в заголовке чужого раздела — упоминание не есть владение.
ps_mention_marker() { printf '# PROJECT-STATE\n\n## M-68 «предмет» — ✅ ЗАКРЫТ (остаток переехал из %s)\n<!-- MS-STATE: %s CLOSED -->\n\n## %s — в работе\n' "$1" "$1" "$1"; }
# Маркер до первого раздела — ничей.
ps_orphan_marker() { printf '# PROJECT-STATE\n<!-- MS-STATE: %s CLOSED -->\n\n## %s — в работе\n' "$1" "$1"; }
# Противоречие внутри своего раздела: CLOSED и OPEN одного номера.
ps_contradict()    { printf '# PROJECT-STATE\n\n## %s — в работе\n<!-- MS-STATE: %s CLOSED -->\n<!-- MS-STATE: %s OPEN -->\n' "$1" "$1" "$1"; }
# Позитивы: у номера несколько разделов, маркер во втором; маркер в ### подразделе; заголовок
# своего раздела упоминает чужие номера ПОСЛЕ своего (форма M-89 на дереве 2026-10-02).
ps_second_section() { printf '# PROJECT-STATE\n\n## %s часть 1 — история\nтекст\n\n## M-70 другое\nтекст\n\n## %s часть 2 — ✅ ЗАКРЫТ\n<!-- MS-STATE: %s CLOSED -->\n' "$1" "$1" "$1"; }
ps_subsection()     { printf '# PROJECT-STATE\n\n## %s «предмет» — ✅ ЗАКРЫТ\n### Состояние\n<!-- MS-STATE: %s CLOSED -->\n' "$1" "$1"; }
ps_owner_mentions() { printf '# PROJECT-STATE\n\n## %s «предмет» — ✅ ЗАКРЫТ (остаток → M-91, задача из M-62)\n<!-- MS-STATE: %s CLOSED -->\n' "$1" "$1"; }
ps_archived_open(){ printf '# PROJECT-STATE\n\n## %s — в работе\n<!-- MS-STATE: %s OPEN -->\n<!-- MS-STATE: M-11 OPEN -->\n' "$1" "$1"; }

echo "── МОЛЧИТ на честной работе (позитивный контроль) ───────────────────────────"
expect "P1 переезд + маркер CLOSED коммитом reviewer'а в том же диапазоне" 0 "$(run_case M-69 yes ps_open_phrase ps_closed)"
expect "P2 маркер CLOSED уже в базе (reviewer закрыл раньше), переезд один"  0 "$(run_case M-69 yes ps_closed -)"
expect "P3 нет переезда, реестр без маркеров"                                0 "$(run_case M-69 no ps_empty -)"
expect "P4 историческая проза: «НЕ ЗАКРЫТ — close-out…» в ЦИТАТЕ (форма M-69)" 0 "$(run_case M-69 yes ps_open_phrase ps_closed_quoted)"
expect "P5 «НЕ закрыт» о ДРУГОМ предмете (форма M-88)"                       0 "$(run_case M-88 yes ps_open_phrase ps_closed_other_subject)"
expect "P6 фраза внутри \`кода\` — цитата"                                     0 "$(run_case M-69 yes ps_open_phrase ps_closed_code_quote)"
expect "P7 соседний номер M-690 открыт со старой фразой — к M-69 не относится" 0 "$(run_case M-69 yes ps_open_phrase ps_closed_neighbor_open)"
expect "P8 OPEN у ЖИВОГО (невынесенного) милестоуна, без переезда"           0 "$(run_case M-69 no ps_empty ps_live_open)"
expect "P9 буквенный номер: переезд M-38a + маркер M-38a"                    0 "$(run_case M-38a yes ps_empty ps_closed)"
expect "P10 несколько разделов номера, маркер во ВТОРОМ своём"               0 "$(run_case M-69 yes ps_empty ps_second_section)"
expect "P11 маркер в ### подразделе своего раздела"                          0 "$(run_case M-69 yes ps_empty ps_subsection)"
expect "P12 заголовок своего раздела упоминает чужие номера ПОСЛЕ своего"     0 "$(run_case M-69 yes ps_empty ps_owner_mentions)"

echo
echo "── КРАСНЕЕТ там, где обязан (мутанты реестра) ───────────────────────────────"
expect "K1 переезд БЕЗ раздела и без маркера"                                1 "$(run_case M-69 yes ps_empty -)"
expect "K2 переезд: раздел с прозой «ЗАКРЫТ», маркера НЕТ"                   1 "$(run_case M-69 yes ps_empty ps_closed_prose_only)"
expect "K3 переезд при маркере OPEN"                                         1 "$(run_case M-69 yes ps_empty ps_marker_open)"
expect "K4 переезд при старой фразе (форма 5/5 инцидентов), реестр не тронут" 1 "$(run_case M-69 yes ps_open_phrase -)"
expect "K5 маркер CLOSED есть, но заголовок твердит «close-out за architect»" 1 "$(run_case M-69 yes ps_empty ps_closed_stale_heading)"
expect "K6 маркер CLOSED только ВНУТРИ блока кода"                           1 "$(run_case M-69 yes ps_empty ps_closed_in_fence)"
expect "K7 маркер CLOSED у ЧУЖОГО номера M-690"                              1 "$(run_case M-69 yes ps_empty ps_closed_wrong_id)"
expect "K8 маркер CLOSED у буквенного соседа M-69a"                          1 "$(run_case M-69 yes ps_empty ps_closed_suffix_id)"
expect "K9 маркер CLOSED только в РАБОЧЕМ дереве, не в коммите"              1 "$(run_case M-69 yes ps_empty ps_closed uncommitted)"
expect "K10 маркер не по форме (опечатка) при годном соседнем"               1 "$(run_case M-69 yes ps_empty ps_malformed)"
expect "K11 БЕЗ переезда: OPEN у милестоуна, чья спека в архиве"             1 "$(run_case M-69 no ps_empty ps_archived_open archive_too:M-11)"
expect "K12 реестр снесён в диапазоне"                                        1 "$(run_case M-69 no ps_empty - rm_ps)"
expect "K13 C-272: маркер M-69 в разделе M-68, раздел M-69 «в работе» (фикстура критика)" 1 "$(run_case M-69 yes ps_empty ps_foreign_marker)"
expect "K14 маркер в разделе, чей заголовок лишь УПОМИНАЕТ номер"             1 "$(run_case M-69 yes ps_empty ps_mention_marker)"
expect "K15 маркер ДО первого раздела — ничей"                               1 "$(run_case M-69 yes ps_empty ps_orphan_marker)"
expect "K16 БЕЗ переезда: маркер в чужом разделе"                            1 "$(run_case M-69 no ps_empty ps_foreign_marker)"
expect "K17 БЕЗ переезда: CLOSED и OPEN одного номера в своём разделе"       1 "$(run_case M-69 no ps_empty ps_contradict)"

echo
echo "── FAIL-CLOSED: «не могу проверить» ≠ «нечего проверять» ────────────────────"
mk_min() {
  local dir="$SANDBOX/fc-$RANDOM$RANDOM"
  mkdir -p "$dir/scripts"; git -C "$dir" init -q
  git -C "$dir" config user.email t@t.local; git -C "$dir" config user.name t
  cp "$BARRIER" "$dir/scripts/check_ps_on_archive.sh"; ps_empty > "$dir/PROJECT-STATE.md"
  git -C "$dir" add -A >/dev/null 2>&1; git -C "$dir" commit -qm base >/dev/null 2>&1
  printf '%s' "$dir"
}
d="$(mk_min)"; rc="$( cd "$d" && EVENT_NAME= PUSH_BEFORE= bash scripts/check_ps_on_archive.sh >/dev/null 2>&1; echo $? )"
expect "F1 событие не задано → отказ" 1 "$rc"
d="$(mk_min)"; rc="$( cd "$d" && EVENT_NAME=push PUSH_BEFORE=0000000000000000000000000000000000000000 bash scripts/check_ps_on_archive.sh >/dev/null 2>&1; echo $? )"
expect "F2 zero-SHA → отказ" 1 "$rc"
d="$(mk_min)"; rc="$( cd "$d" && EVENT_NAME=push PUSH_BEFORE=deadbeefdeadbeefdeadbeefdeadbeefdeadbeef bash scripts/check_ps_on_archive.sh >/dev/null 2>&1; echo $? )"
expect "F3 база вне истории → отказ" 1 "$rc"

echo
echo "── ПРОД-ФОРМА: реальное дерево и реплей реальных инцидентов ─────────────────"
# Реальное дерево: барьер применим и молчит на вершине (переезда в последнем коммите нет).
# Барьер кладётся в worktree БЕЗ чекаута на HEAD: он читает состояние из объектов git, и так
# прогон идентичен прогону «на месте», но годен и для копии-мутанта (`PS_BARRIER`).
wt="$SANDBOX/wt-head"
if git -C "$ROOT" worktree add --no-checkout --detach "$wt" HEAD >/dev/null 2>&1; then
  echo "$wt" >> "$WT_LIST"; mkdir -p "$wt/scripts"; cp "$BARRIER" "$wt/scripts/check_ps_on_archive.sh"
  rc="$( cd "$wt" && EVENT_NAME=push PUSH_BEFORE="$(git rev-parse HEAD~1 2>/dev/null || git rev-parse HEAD)" bash scripts/check_ps_on_archive.sh >/dev/null 2>&1; echo $? )"
  if [ "$rc" -eq 0 ]; then ok "R0 реальное дерево: барьер применим и молчит"
  else nope "R0 реальное дерево: exit=$rc — барьер непригоден к включению в CI"; fi
else nope "R0 реальное дерево: SETUP worktree не состоялся"; fi

# Реплей: worktree БЕЗ чекаута на коммите переезда (барьер читает HEAD из объектов git,
# файлы не нужны), диапазон = <коммит>^..<коммит>. Каждый — инцидент из `R-213` §3.
for pair in "M-69:636a3ec" "M-75:70ca5c8" "M-77:a9058d3" "M-85:c576514" "M-87:8d81b44"; do
  id="${pair%%:*}"; sha="${pair##*:}"
  if ! git -C "$ROOT" cat-file -e "${sha}^{commit}" 2>/dev/null; then
    nope "R ${id}: коммит ${sha} отсутствует в истории (мелкий клон? нужен fetch-depth: 0)"; continue
  fi
  wt="$SANDBOX/wt-$id"
  git -C "$ROOT" worktree add --no-checkout --detach "$wt" "$sha" >/dev/null 2>&1 || { nope "R ${id}: SETUP worktree не состоялся"; continue; }
  echo "$wt" >> "$WT_LIST"
  # SETUP-guard: в диапазоне реально стоит переезд этой спеки.
  if [ -z "$(git -C "$wt" diff --no-renames --name-only --diff-filter=A "${sha}^..${sha}" -- "docs/archive/${id}-*.md")" ]; then
    nope "R ${id}: SETUP — в ${sha} нет переезда ${id}, реплей судил бы не то"; continue
  fi
  mkdir -p "$wt/scripts"; cp "$BARRIER" "$wt/scripts/check_ps_on_archive.sh"
  rc="$( cd "$wt" && EVENT_NAME=push PUSH_BEFORE="$(git rev-parse "${sha}^")" bash scripts/check_ps_on_archive.sh >/dev/null 2>&1; echo $? )"
  expect "R ${id}: реплей переезда ${sha} (реестр твердил «close-out за architect») → отказ" 1 "$rc"
done

echo
echo "────────────────────────────────────────────────────────────────────────────"
echo "сценариев: $((PASS + FAIL))   pass=$PASS   FAIL=$FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL"; exit 1; fi
