#!/usr/bin/env bash
# Генератор эталона и мутантов для `red_artifact_ids.sh --battery` (M-61, спека §4.5).
# Эталон — ЧЕСТНАЯ реализация инварианта §4.1; каждый мутант отличается ровно одним
# свойством, названным осью и значением. Лежит в репозитории, а не в /tmp сессии:
# `A-005` §6.5 — сырые прогоны, снятые несуществующей пробой, четыре круга никто не заметил.
set -uo pipefail
D="${1:?каталог назначения}"; mkdir -p "$D" || exit 1

# ─── общие части ────────────────────────────────────────────────────────────────────
read -r -d '' HEAD_COMMON <<'EOF'
#!/usr/bin/env bash
set -uo pipefail
ZERO=0000000000000000000000000000000000000000
CLASS_RE='^(milestones/M|research/reviews/R|research/critiques/C|research/arbitration/A)-[0-9]+'
refs_all() { git for-each-ref --format='%(refname)' refs/remotes/origin refs/heads 2>/dev/null; }
# Класс и номер из пути артефакта. Буквенный суффикс milestone'а (M-80a) отбрасывается:
# семья определяется числом, а законность дробления решает ПРЕДМЕТ, а не буква.
cls_num() {
  case "$1" in
    milestones/M-*)            printf 'M %s\n' "$(basename "$1" | sed -E 's/^M-0*([0-9]+).*/\1/')";;
    research/reviews/R-*)      printf 'R %s\n' "$(basename "$1" | sed -E 's/^R-0*([0-9]+).*/\1/')";;
    research/critiques/C-*)    printf 'C %s\n' "$(basename "$1" | sed -E 's/^C-0*([0-9]+).*/\1/')";;
    research/arbitration/A-*)  printf 'A %s\n' "$(basename "$1" | sed -E 's/^A-0*([0-9]+).*/\1/')";;
    *) return 1;;
  esac
}
slug_of() { basename "$1" .md | sed -E 's/^(M|R|C|A)-[0-9]+[a-z]?-?//; s/-rev[0-9]+$//; s/-addendum.*$//'; }
EOF

# Носитель предмета: шапка «Предмет:»/«Контекст» — первый backtick-путь в блоке до пустой строки.
read -r -d '' SUBJ_FULL <<'EOF'
subject_of() {  # $1 = rev:path
  local body hdr
  body="$(git show "$1" 2>/dev/null)" || return 1
  hdr="$(printf '%s\n' "$body" | awk '/^\*\*(Предмет:|Контекст)/{f=1} f{print} f&&/^$/{exit}' \
        | grep -oE '`[^`]+`' | head -1 | tr -d '`')"
  if [ -n "$hdr" ]; then printf '%s\n' "$hdr"; return 0; fi
  local s; s="$(slug_of "${1#*:}")"; [ -z "$s" ] && s='<без-слага>'; printf '%s\n' "$s"
}
EOF
read -r -d '' SUBJ_SLUGONLY <<'EOF'
subject_of() { local s; s="$(slug_of "${1#*:}")"; [ -z "$s" ] && s='<без-слага>'; printf '%s\n' "$s"; }
EOF
read -r -d '' SUBJ_CTXBLIND <<'EOF'
subject_of() {  # мутант contextblind: знает только `**Предмет:**`
  local body hdr
  body="$(git show "$1" 2>/dev/null)" || return 1
  hdr="$(printf '%s\n' "$body" | grep -m1 '^\*\*Предмет:' | grep -oE '`[^`]+`' | head -1 | tr -d '`')"
  if [ -n "$hdr" ]; then printf '%s\n' "$hdr"; return 0; fi
  local s; s="$(slug_of "${1#*:}")"; [ -z "$s" ] && s='<без-слага>'; printf '%s\n' "$s"
}
EOF
read -r -d '' SUBJ_SPLITONLY <<'EOF'
subject_of() {  # мутант splitonly: буквенный суффикс сам заверяет семью
  case "${1#*:}" in milestones/M-[0-9]*[a-z]-*) printf 'СЕМЬЯ\n'; return 0;; esac
  local body hdr
  body="$(git show "$1" 2>/dev/null)" || return 1
  hdr="$(printf '%s\n' "$body" | awk '/^\*\*(Предмет:|Контекст)/{f=1} f{print} f&&/^$/{exit}' \
        | grep -oE '`[^`]+`' | head -1 | tr -d '`')"
  [ -n "$hdr" ] && { printf '%s\n' "$hdr"; return 0; }
  local s; s="$(slug_of "${1#*:}")"; [ -z "$s" ] && s='<без-слага>'; printf '%s\n' "$s"
}
EOF

# Тело барьера. $RANGE_MODE=range|all ; $NAME_FLAGS — фильтр статусов git show.
read -r -d '' BODY_CHECK <<'EOF'
case "${EVENT_NAME:-}" in
  push)         BASE="${PUSH_BEFORE:-}";;
  pull_request) BASE="${PR_BASE_SHA:-}";;
  *) exit 1;;
esac
[ -n "$BASE" ] || exit 1
[ "$BASE" != "$ZERO" ] || exit 1
git cat-file -e "$BASE" 2>/dev/null || exit 1
git merge-base --is-ancestor "$BASE" HEAD 2>/dev/null || exit 1

# Все артефакты объединения ref'ов: "класс номер предмет"
universe() {
  local ref f cn
  for ref in $(refs_all); do
    while IFS= read -r -d '' f; do
      [[ "$f" =~ $CLASS_RE ]] || continue
      cn="$(cls_num "$f")" || continue
      printf '%s %s\n' "$cn" "$(subject_of "$ref:$f")"
    done < <(git ls-tree -r --name-only -z "$ref" 2>/dev/null)
  done
  # записи TECH-DEBT.md всех ref'ов
  for ref in $(refs_all); do
    git show "$ref:TECH-DEBT.md" 2>/dev/null | sed -nE 's/^- \*\*TD-0*([0-9]+)\*\* `([^`]+)`.*/TD \1 \2/p'
  done
}
# Что ВВЕДЕНО диапазоном
introduced() {
  local c f cn
  for c in $(git rev-list "$BASE..HEAD"); do
    while IFS= read -r -d '' f; do
      [[ "$f" =~ $CLASS_RE ]] || continue
      cn="$(cls_num "$f")" || continue
      printf '%s %s\n' "$cn" "$(subject_of "$c:$f")"
    done < <(git show --cc --name-only --no-renames --diff-filter=A -z --format= "$c" 2>/dev/null)
    # новые записи TECH-DEBT.md этого коммита
    git show "$c" -- TECH-DEBT.md 2>/dev/null \
      | sed -nE 's/^\+- \*\*TD-0*([0-9]+)\*\* `([^`]+)`.*/TD \1 \2/p'
  done
}
U="$(universe | sort -u)"
IN="$(introduced | sort -u)"
[ -z "$IN" ] && exit 0
while read -r cls num subj; do
  [ -z "${cls:-}" ] && continue
  while read -r c2 n2 s2; do
    [ "$c2" = "$cls" ] && [ "$n2" = "$num" ] && [ "$s2" != "$subj" ] && exit 1
  done <<< "$U"
done <<< "$IN"
exit 0
EOF

# Мутант showall: судит ВСЮ историю, а не диапазон (ось 5).
read -r -d '' BODY_SHOWALL <<'EOF'
case "${EVENT_NAME:-}" in push) BASE="${PUSH_BEFORE:-}";; pull_request) BASE="${PR_BASE_SHA:-}";; *) exit 1;; esac
[ -n "$BASE" ] || exit 1
[ "$BASE" != "$ZERO" ] || exit 1
git cat-file -e "$BASE" 2>/dev/null || exit 1
universe() {
  local ref f cn
  for ref in $(refs_all); do
    while IFS= read -r -d '' f; do
      [[ "$f" =~ $CLASS_RE ]] || continue
      cn="$(cls_num "$f")" || continue
      printf '%s %s\n' "$cn" "$(subject_of "$ref:$f")"
    done < <(git ls-tree -r --name-only -z "$ref" 2>/dev/null)
  done
  for ref in $(refs_all); do
    git show "$ref:TECH-DEBT.md" 2>/dev/null | sed -nE 's/^- \*\*TD-0*([0-9]+)\*\* `([^`]+)`.*/TD \1 \2/p'
  done
}
U="$(universe | sort -u)"
dup="$(printf '%s\n' "$U" | awk '{print $1" "$2}' | sort | uniq -d)"
[ -n "$dup" ] && exit 1
exit 0
EOF

# Мутант renameblind: не смотрит переименования (ось 4).
# Мутант renameblind: детекция переименований ВКЛЮЧЕНА, поэтому увод показывается как R и
# фильтром A не ловится — слеп именно к переименованию, а не сломан целиком.
BODY_RENAMEBLIND="${BODY_CHECK//--no-renames /}"
# Мутант touchcounts: правка (M) считается введением предмета (ось 4 / правка существующего).
BODY_TOUCHCOUNTS="${BODY_CHECK//--diff-filter=A /--diff-filter=AM }"

# Мутант quotedname: построчное чтение вместо `-z` (ось 4 / имя, требующее квотирования).
# Б-4 (R-052): сплошная подстановка `${BODY_CHECK//-z /}` срезала `-z` НЕ ТОЛЬКО у git, но и
# у shell-тестов — `[ -z "$IN" ] && exit 0` превращалось в `[ "$IN" ] && exit 0`, то есть
# «что-то введено ⇒ выйти успехом». Мутант пропускал ЛЮБУЮ коллизию, включая чисто ASCII, и
# краснел на всех 12 блокирующих сценариях — то есть был сломан целиком, а не слеп к
# квотированию. Мутант обязан отличаться от эталона ТОЛЬКО каналом чтения имён.

# ─── аллокаторы ─────────────────────────────────────────────────────────────────────
read -r -d '' NEXT_REF <<'EOF'
CLS="${1:?класс}"
# origin сконфигурирован, но ни одного его ref'а нет ⇒ занятость перечислить невозможно.
if git remote get-url origin >/dev/null 2>&1; then
  [ -n "$(git for-each-ref --format='%(refname)' refs/remotes/origin 2>/dev/null)" ] || exit 1
fi
max=0
for ref in $(refs_all); do
  case "$CLS" in
    TD) n="$(git show "$ref:TECH-DEBT.md" 2>/dev/null | grep -oE 'TD-[0-9]+' | grep -oE '[0-9]+')";;
    *)  n="$(git ls-tree -r --name-only "$ref" 2>/dev/null | grep -oE "(^|/)${CLS}-[0-9]+" | grep -oE '[0-9]+')";;
  esac
  for x in $n; do x=$((10#$x)); [ "$x" -gt "$max" ] && max=$x; done
done
[ "$max" -eq 0 ] && exit 1
case "$CLS" in M) printf 'M-%02d\n' $((max+1));; *) printf '%s-%03d\n' "$CLS" $((max+1));; esac
EOF
read -r -d '' NEXT_LOCALMAX <<'EOF'
CLS="${1:?класс}"
max=0
case "$CLS" in
  TD) n="$(grep -oE 'TD-[0-9]+' TECH-DEBT.md 2>/dev/null | grep -oE '[0-9]+')";;
  *)  n="$(git ls-tree -r --name-only HEAD 2>/dev/null | grep -oE "(^|/)${CLS}-[0-9]+" | grep -oE '[0-9]+')";;
esac
for x in $n; do x=$((10#$x)); [ "$x" -gt "$max" ] && max=$x; done
[ "$max" -eq 0 ] && exit 1
case "$CLS" in M) printf 'M-%02d\n' $((max+1));; *) printf '%s-%03d\n' "$CLS" $((max+1));; esac
EOF

emit() { printf '%s\n%s\n%s\n' "$HEAD_COMMON" "$2" "$3" > "$D/$1"; bash -n "$D/$1" || exit 1; }
# Б-5 (R-052): пять мутантов были ОБЪЯВЛЕНЫ в §4.5 и не построены; батарея пропускала их
# молча и печатала PASS по знаменателю исполненного. Каждый — одна точка ОДНОГО из двух
# предметов пробы: барьера либо (для оси 3) аллокатора.
#   originonly — АЛЛОКАТОР: перечисление ref'ов теряет `refs/heads` (ось 3)
#   namesonly  — записи в TECH-DEBT.md не читаются вовсе (ось 2)
#   absolute   — судит всю историю, а не диапазон (ось 5)
#   rangeblind — смотрит только вершину диапазона, а не каждый коммит (ось 5)
#   slugskip   — файл без слага пропускается вместо sentinel (ось 6)


# ── Мутанты строятся SED'ом, а не подстановкой ${//} (правка круга 3) ────────────────────
# Причина не стилистическая: `${BODY//pat/repl}` с кавычками и слэшами внутри шаблона молча
# НЕ СРАБАТЫВАЕТ (originonly дал 0 изменений и совпал с эталоном) либо портит тело
# (absolute вклеивал остаток шаблона в текст скрипта). Оба отказа тихие — ровно тот класс,
# из-за которого понадобился круг 3. `sed` с одинарными кавычками предсказуем, а страж
# «мутант обязан отличаться» ниже ловит несработавшую подстановку немедленно.
mutate() { printf '%s\n' "${BODY_CHECK}" | sed "$1"; }
# originonly мутирует ШАПКУ АЛЛОКАТОРА: `refs_all()` объявлена в HEAD_COMMON, а не в теле.
HEAD_ORIGINONLY="$(printf '%s\n' "${HEAD_COMMON}" | sed 's|refs/remotes/origin refs/heads|refs/remotes/origin|')"
[ "$HEAD_ORIGINONLY" != "$HEAD_COMMON" ] || { echo "originonly: подстановка шапки НЕ СРАБОТАЛА" >&2; exit 1; }
# headsonly — ЗЕРКАЛО originonly: из перечисления выпал `refs/remotes/origin`. Это корневой
# дефект §1 («номер, свободный локально, занят в соседней ветке»), и до круга 4 его не пиннил
# ни один мутант: ось 3 была покрыта только в сторону refs/heads.
HEAD_HEADSONLY="$(printf '%s\n' "${HEAD_COMMON}" | sed 's|refs/remotes/origin refs/heads|refs/heads|')"
[ "$HEAD_HEADSONLY" != "$HEAD_COMMON" ] || { echo "headsonly: подстановка шапки НЕ СРАБОТАЛА" >&2; exit 1; }

# quotedname — Б-4 (R-052): мутант обязан отличаться от эталона ТОЛЬКО КАНАЛОМ ЧТЕНИЯ ИМЁН.
# Прежняя сплошная подстановка срезала `-z` и у shell-тестов (`[ -z "$IN" ]` → `[ "$IN" ]`),
# из-за чего мутант пропускал ЛЮБУЮ коллизию и краснел на всех 12 сценариях — то есть был
# сломан целиком, а не слеп к квотированию. Теперь трогаются ровно три места: два git-вызова
# теряют `-z` (git начинает КВОТИРОВАТЬ не-ASCII имена), читатель переходит на построчный.
BODY_QUOTEDNAME="$(mutate 's|--name-only -z |--name-only |; s|--diff-filter=A -z |--diff-filter=A |; s|read -r -d '"'"''"'"' f|read -r f|')"
# originonly — перечисление refs теряет локальные головы (ось 3)
# namesonly — записи в TECH-DEBT.md не читаются вовсе (ось 2)
BODY_NAMESONLY="$(mutate 's|:TECH-DEBT\.md|:TECH-DEBT.НЕТ.md|')"
# absolute — судит всю историю вместо диапазона (ось 5)
BODY_ABSOLUTE="$(mutate 's|git rev-list "\$BASE\.\.HEAD"|git rev-list HEAD|')"
# rangeblind — смотрит только вершину диапазона, а не каждый коммит (ось 5)
BODY_RANGEBLIND="$(mutate 's|git rev-list "\$BASE\.\.HEAD"|git rev-list -n 1 "$BASE..HEAD"|')"

# slugskip — файл без слага ПРОПУСКАЕТСЯ вместо участия в сравнении (ось 6).
# A-006 §2.3: прежняя мутация правила `subject_of` (ранний `return` вместо sentinel) слепоты
# НЕ РЕАЛИЗОВЫВАЛА — пустой subject всё равно печатался вызывающим и всё равно участвовал в
# сравнении, поэтому мутант вёл себя как эталон и проба была ЗЕЛЕНА против него. Пропуск
# возможен только в ВЫЗЫВАЮЩЕМ: `subject_of` печатает, решение «участвовать или нет»
# принимает цикл. Поэтому мутирует ТЕЛО (две точки эмиссии), а `subject_of` остаётся
# эталонным — мутант отличается ровно одним свойством: судьбой slugless-артефакта.
read -r -d '' SED_SLUGSKIP <<'EOF'
s|printf '%s %s\\n' "$cn" "$(subject_of "$ref:$f")"|sb="$(subject_of "$ref:$f")"; [ "$sb" = "<без-слага>" ] \&\& continue; printf '%s %s\\n' "$cn" "$sb"|
s|printf '%s %s\\n' "$cn" "$(subject_of "$c:$f")"|sb="$(subject_of "$c:$f")"; [ "$sb" = "<без-слага>" ] \&\& continue; printf '%s %s\\n' "$cn" "$sb"|
EOF
BODY_SLUGSKIP="$(mutate "$SED_SLUGSKIP")"

# вариант : check(subject, body) + next
emit ref-check.sh          "$SUBJ_FULL"       "$BODY_CHECK"
emit showall-check.sh      "$SUBJ_FULL"       "$BODY_SHOWALL"
emit renameblind-check.sh  "$SUBJ_FULL"       "$BODY_RENAMEBLIND"
emit slugonly-check.sh     "$SUBJ_SLUGONLY"   "$BODY_CHECK"
emit contextblind-check.sh "$SUBJ_CTXBLIND"   "$BODY_CHECK"
emit splitonly-check.sh    "$SUBJ_SPLITONLY"  "$BODY_CHECK"
emit localmax-check.sh     "$SUBJ_FULL"       "$BODY_CHECK"
emit quotedname-check.sh   "$SUBJ_FULL"       "$BODY_QUOTEDNAME"
emit touchcounts-check.sh  "$SUBJ_FULL"       "$BODY_TOUCHCOUNTS"
# originonly — БАРЬЕР эталонный: ось 3 «область поиска занятости» — предмет АЛЛОКАТОРА
# (`next_artifact_id.sh`), а не барьера; мутант строится ниже, на аллокаторе, как localmax.
# A-006 §2.3 признал точечный профиль originonly структурно недостижимым — это верно ТОЛЬКО
# для мутанта, построенного на барьере: `universe()` фикстур не несёт origin-ref'ов, поэтому
# барьер без `refs/heads` слеп ко ВСЕМУ и краснеет на всех 12 блокирующих сценариях. Замер
# круга 4: тот же дефект, внесённый в АЛЛОКАТОР, даёт ровно N3LOCAL+N3HEAD — обе оси 3, и
# N3HEAD есть дословно объявленное значение «только origin, локальный head пропущен».
# Клапан (в) `R-052` усл. 3 (вывод мутанта из §4.5) поэтому НЕ применяется: он был бы платой
# за дефект построения, а не за структурное свойство фикстур.
emit originonly-check.sh   "$SUBJ_FULL"       "$BODY_CHECK"
emit headsonly-check.sh    "$SUBJ_FULL"       "$BODY_CHECK"
emit namesonly-check.sh    "$SUBJ_FULL"       "$BODY_NAMESONLY"
emit absolute-check.sh     "$SUBJ_FULL"       "$BODY_ABSOLUTE"
emit rangeblind-check.sh   "$SUBJ_FULL"       "$BODY_RANGEBLIND"
emit slugskip-check.sh     "$SUBJ_FULL"       "$BODY_SLUGSKIP"

# Аллокаторы. Список ВЫВОДИТСЯ из построенных барьеров, а не дублируется руками: прежний
# захардкоженный перечень — то же ручное соответствие, что дрейфовало везде (A-006 §2.5).
for f in "$D"/*-check.sh; do
  v="$(basename "$f" -check.sh)"
  printf '%s\n%s\n' "$HEAD_COMMON" "$NEXT_REF" > "$D/$v-next.sh"; bash -n "$D/$v-next.sh" || exit 1
done
# Два мутанта ОСИ 3 живут в аллокаторе: localmax — максимум своего дерева; originonly —
# перечисление ref'ов теряет `refs/heads` (номер, занятый локальной веткой, не виден).
printf '%s\n%s\n' "$HEAD_COMMON"     "$NEXT_LOCALMAX" > "$D/localmax-next.sh";   bash -n "$D/localmax-next.sh"   || exit 1
printf '%s\n%s\n' "$HEAD_ORIGINONLY" "$NEXT_REF"      > "$D/originonly-next.sh"; bash -n "$D/originonly-next.sh" || exit 1
printf '%s\n%s\n' "$HEAD_HEADSONLY"  "$NEXT_REF"      > "$D/headsonly-next.sh";   bash -n "$D/headsonly-next.sh"   || exit 1

# Страж генератора: мутант, совпавший с эталоном, тестировал бы эталон под чужим именем —
# ровно то, чем оборачивается ТИХО НЕ СРАБОТАВШАЯ подстановка. Перебираются ВСЕ построенные:
# прежний список был захардкожен (7 имён при 13 мутантах) и сам являлся ручным соответствием,
# то есть стражем, не стерегущим половину состава (A-006 §2.5 — механизируй, а не сверяй глазом).
built=0
for f in "$D"/*-check.sh; do
  m="$(basename "$f" -check.sh)"
  [ "$m" = ref ] && continue
  built=$((built + 1))
  if cmp -s "$D/ref-check.sh" "$D/$m-check.sh" && cmp -s "$D/ref-next.sh" "$D/$m-next.sh"; then
    echo "мутант $m НЕ ПОСТРОЕН — совпал с эталоном И по барьеру, И по аллокатору" >&2; exit 1
  fi
done
echo "эталон и $built мутантов собраны в $D"
