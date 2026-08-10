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
    done < <(git show --cc --name-only --no-renames --diff-filter=AM -z --format= "$c" 2>/dev/null)
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
BODY_RENAMEBLIND="${BODY_CHECK//--no-renames --diff-filter=AM/--diff-filter=A}"

# Мутант quotedname: построчное чтение вместо `-z` (ось 4 / имя, требующее квотирования).
BODY_QUOTEDNAME="${BODY_CHECK//-z /}"
BODY_QUOTEDNAME="${BODY_QUOTEDNAME//read -r -d \'\' f/read -r f}"

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
# вариант : check(subject, body) + next
emit ref-check.sh          "$SUBJ_FULL"       "$BODY_CHECK"
emit showall-check.sh      "$SUBJ_FULL"       "$BODY_SHOWALL"
emit renameblind-check.sh  "$SUBJ_FULL"       "$BODY_RENAMEBLIND"
emit slugonly-check.sh     "$SUBJ_SLUGONLY"   "$BODY_CHECK"
emit contextblind-check.sh "$SUBJ_CTXBLIND"   "$BODY_CHECK"
emit splitonly-check.sh    "$SUBJ_SPLITONLY"  "$BODY_CHECK"
emit localmax-check.sh     "$SUBJ_FULL"       "$BODY_CHECK"
emit quotedname-check.sh   "$SUBJ_FULL"       "$BODY_QUOTEDNAME"
for v in ref showall renameblind slugonly contextblind splitonly quotedname; do
  printf '%s\n%s\n' "$HEAD_COMMON" "$NEXT_REF" > "$D/$v-next.sh"; bash -n "$D/$v-next.sh" || exit 1
done
printf '%s\n%s\n' "$HEAD_COMMON" "$NEXT_LOCALMAX" > "$D/localmax-next.sh"; bash -n "$D/localmax-next.sh" || exit 1

# Страж генератора: мутант, совпавший с эталоном, тестировал бы эталон под чужим именем.
for m in showall renameblind slugonly contextblind splitonly quotedname; do
  cmp -s "$D/ref-check.sh" "$D/$m-check.sh" && { echo "мутант $m НЕ ПОСТРОЕН — совпал с эталоном" >&2; exit 1; }
done
cmp -s "$D/ref-next.sh" "$D/localmax-next.sh" && { echo "мутант localmax НЕ ПОСТРОЕН" >&2; exit 1; }
echo "эталон и 6 мутантов собраны в $D"
