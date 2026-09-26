#!/usr/bin/env bash
# red_archived_refs.sh — ДВУСТОРОННЯЯ ПРОБА барьера висячих ссылок.
#
# Барьер сам под пробой (`gates.md` §9): односторонняя проверка зелена и у барьера,
# который всегда возвращает 0. Здесь предъявляются ОБЕ стороны и, отдельно, два свойства
# базовой линии — иначе список унаследованного долга превращается в вечную амнистию.
#
# Проба строит ОДНОРАЗОВЫЙ репозиторий: барьер работает через `git grep`, и подсунуть ему
# файлы мимо git нельзя. Число сценариев СЧИТАЕТСЯ прогоном, а не заявляется.
#
# Сценарии 8-12 (харнесс-трек 2026-09-26, `R-203` §5): ГРАНИЦА идентификатора. Первая
# редакция барьера искала подстрокой и краснела на живом `M-60b` при вынесенном `M-60`;
# ложное красное гасили базовой линией — восемь амнистий на живые файлы. Каждая пара ниже
# держит обе стороны: граница НЕ убивает обнаружение (позитивный контроль в том же репо).
# У каждого сценария — страж setup'а: проба, молча гоняющая не тот сценарий, — плацебо.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
N=0; BAD=0
ok()  { N=$((N+1)); printf 'ok    %s\n' "$1"; }
bad() { N=$((N+1)); BAD=$((BAD+1)); printf 'FAIL  %s\n' "$1"; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT INT TERM

# Одноразовый репозиторий с той же раскладкой, что у настоящего.
mk_repo() {
  local d="$1"
  rm -rf "$d"; mkdir -p "$d/scripts/lib" "$d/docs/archive" "$d/crates/gateway/src"
  cp "$ROOT/scripts/check_archived_refs.sh" "$d/scripts/"
  git -C "$d" init -q
  git -C "$d" config user.email probe@local
  git -C "$d" config user.name probe
  : > "$d/docs/archive/M-99-probe-subject.md"
  : > "$d/scripts/lib/archived_refs_baseline.txt"
}
commit_all() { git -C "$1" add -A >/dev/null 2>&1; git -C "$1" commit -q -m probe --no-verify >/dev/null 2>&1; }
run() { ( cd "$1" && bash scripts/check_archived_refs.sh >"$TMP/last.out" 2>&1; echo $? ); }
# СТРАЖ SETUP'А: фикстура обязана быть ПОД GIT и содержать ровно тот текст, который сценарий
# судит; иначе сценарий проверяет не то, что заявляет (`testing.md` §«Целостность гейта» п.3).
# Возвращает 1 и пишет FAIL — вызывающий сценарий не выполняется.
guard() { # guard <repo> <pathspec> <literal>
  if git -C "$1" grep -q -F -- "$3" -- "$2" 2>/dev/null; then return 0; fi
  bad "SETUP не состоялся: «$3» не найден под git в $2 — сценарий не судит то, что заявляет"
  return 1
}

# ─── (1) ИЗВЕСТНЫЙ СЛУЧАЙ: живой код ссылается на вынесенный артефакт ⇒ ОТКАЗ ───
R="$TMP/case1"; mk_repo "$R"
printf '// спека milestones/M-99-probe-subject.md §4\n' > "$R/crates/gateway/src/lib.rs"
commit_all "$R"
if [ "$(run "$R")" -ne 0 ]; then
  ok "известный случай: висячая ссылка в живом коде ⇒ отказ"
else
  bad "известный случай ПРОПУЩЕН — барьер не ловит то, ради чего заведён"
fi

# ─── (2) ПОСЛЕ ПОЧИНКИ: путь исправлен ⇒ ПРИНЯТИЕ ───
printf '// спека docs/archive/M-99-probe-subject.md §4\n' > "$R/crates/gateway/src/lib.rs"
commit_all "$R"
if [ "$(run "$R")" -eq 0 ]; then
  ok "после починки пути: принятие"
else
  bad "исправленный путь всё ещё считается висячим — барьер краснеет на правде"
fi

# ─── (3) НОВОЕ НАРУШЕНИЕ при НЕПУСТОЙ базовой линии ⇒ ОТКАЗ ───
# Без этого сценария амнистия одного файла означала бы амнистию всех.
R2="$TMP/case3"; mk_repo "$R2"
printf '// старый путь milestones/M-99-probe-subject.md\n' > "$R2/crates/gateway/src/lib.rs"
mkdir -p "$R2/crates/journal/src"
printf '// тоже старый путь milestones/M-99\n' > "$R2/crates/journal/src/lib.rs"
printf 'crates/gateway/src/lib.rs|milestones/M-99\n' > "$R2/scripts/lib/archived_refs_baseline.txt"
commit_all "$R2"
if [ "$(run "$R2")" -ne 0 ]; then
  ok "новое нарушение при непустой базовой линии ⇒ отказ (амнистия не распространяется)"
else
  bad "новое нарушение ПРОЩЕНО базовой линией — список стал амнистией для всего дерева"
fi

# ─── (4) БАЗОВАЯ ЛИНИЯ ОБЯЗАНА СОКРАЩАТЬСЯ: протухшая строка ⇒ ОТКАЗ ───
R3="$TMP/case4"; mk_repo "$R3"
printf '// путь уже исправлен: docs/archive/M-99-probe-subject.md\n' > "$R3/crates/gateway/src/lib.rs"
printf 'crates/gateway/src/lib.rs|milestones/M-99\n' > "$R3/scripts/lib/archived_refs_baseline.txt"
commit_all "$R3"
if [ "$(run "$R3")" -ne 0 ]; then
  ok "протухшая строка базовой линии ⇒ отказ (список обязан сокращаться)"
else
  bad "протухшая строка ПРИНЯТА — список амнистирует то, чего уже нет, и слабеет молча"
fi

# ─── (5) FAIL-CLOSED: базовой линии нет вовсе ⇒ ОТКАЗ ───
R4="$TMP/case5"; mk_repo "$R4"
rm -f "$R4/scripts/lib/archived_refs_baseline.txt"
printf '// чисто\n' > "$R4/crates/gateway/src/lib.rs"
commit_all "$R4"
if [ "$(run "$R4")" -ne 0 ]; then
  ok "отсутствие базовой линии ⇒ отказ (fail-closed)"
else
  bad "барьер работает БЕЗ базовой линии — молчание принято за чистоту"
fi

# ─── (6) SETUP-СТРАЖ: пустой архив ⇒ ОТКАЗ, а не «нарушений нет» ───
R5="$TMP/case6"; mk_repo "$R5"
rm -f "$R5/docs/archive/M-99-probe-subject.md"
printf '// что угодно\n' > "$R5/crates/gateway/src/lib.rs"
commit_all "$R5"
if [ "$(run "$R5")" -ne 0 ]; then
  ok "пустой архив ⇒ отказ (барьер не судит пустоту)"
else
  bad "пустой архив принят за чистоту — барьер зеленел бы, ничего не проверяя"
fi

# ─── (7) ИСКЛЮЧЕНИЕ СОБСТВЕННОЙ ОСНАСТКИ НЕ ОСЛЕПЛЯЕТ БАРЬЕР В `scripts/**` ───
# Барьер исключает из поиска три своих файла (они по определению называют мёртвые пути).
# Проверяем, что исключение точечное: ДРУГОЙ файл под `scripts/**` по-прежнему судится.
R6="$TMP/case7"; mk_repo "$R6"
mkdir -p "$R6/scripts/tests"
printf '// чисто\n' > "$R6/crates/gateway/src/lib.rs"
printf '# ссылка на milestones/M-99-probe-subject.md\n' > "$R6/scripts/some_other_gate.sh"
commit_all "$R6"
if [ "$(run "$R6")" -ne 0 ]; then
  ok "исключение оснастки точечное: чужой файл под scripts/** по-прежнему судится"
else
  bad "исключение оснастки ослепило весь scripts/** — барьер перестал видеть свою же зону"
fi

# ─── (8) ГРАНИЦА СПЕКИ: вынесен M-99, живая ссылка на СОСЕДА `M-99b` ⇒ ПРИНЯТИЕ ───
# Ложное красное первой редакции: `milestones/M-99` — подстрока `milestones/M-99b-…`.
R7="$TMP/case8"; mk_repo "$R7"
printf '// живой сосед: milestones/M-99b-other-subject.md §1\n' > "$R7/crates/gateway/src/lib.rs"
commit_all "$R7"
if guard "$R7" 'crates/**' 'milestones/M-99b-other-subject.md'; then
  if [ "$(run "$R7")" -eq 0 ]; then
    ok "граница спеки: ссылка на живой M-99b при вынесенном M-99 ⇒ принятие"
  else
    bad "ссылка на ЖИВОЙ M-99b покраснела — граница идентификатора не работает (подстрока)"
  fi
fi

# ─── (9) ПОЗИТИВНЫЙ КОНТРОЛЬ в ТОМ ЖЕ репо: ссылка на сам M-99 ⇒ ОТКАЗ ───
# Граница — не пробел и не дефис: `milestones/M-99 §3` (конец слова) тоже висячая.
printf '// спека milestones/M-99 §3 и файл milestones/M-99-probe-subject.md\n' > "$R7/crates/gateway/src/lib.rs"
commit_all "$R7"
if guard "$R7" 'crates/**' 'milestones/M-99 §3'; then
  if [ "$(run "$R7")" -ne 0 ]; then
    ok "позитивный контроль: ссылка на вынесенный M-99 (конец слова и дефис) ⇒ отказ"
  else
    bad "граница УБИЛА обнаружение — ссылка на вынесенный M-99 принята"
  fi
fi

# ─── (10) ГРАНИЦА ГЕЙТА: вынесен verify_M-99.sh, живая ссылка на verify_M-99b.sh ⇒ ПРИНЯТИЕ ───
R8="$TMP/case10"; mk_repo "$R8"
rm -f "$R8/docs/archive/M-99-probe-subject.md"
: > "$R8/docs/archive/verify_M-99.sh"
printf '# гоняем bash scripts/verify_M-99b.sh — живой гейт\n' > "$R8/crates/gateway/src/lib.rs"
commit_all "$R8"
if [ -z "$(git -C "$R8" ls-files docs/archive/verify_M-99.sh)" ]; then
  bad "SETUP не состоялся: docs/archive/verify_M-99.sh не под git — сценарий 10 судил бы пустой архив"
elif guard "$R8" 'crates/**' 'scripts/verify_M-99b.sh'; then
  if [ "$(run "$R8")" -eq 0 ]; then
    ok "граница гейта: ссылка на живой verify_M-99b.sh при вынесенном verify_M-99.sh ⇒ принятие"
  else
    bad "ссылка на ЖИВОЙ verify_M-99b.sh покраснела — граница гейта не работает"
  fi
fi
# позитивный контроль в том же репо
printf '# гоняем bash scripts/verify_M-99.sh — вынесенный гейт\n' > "$R8/crates/gateway/src/lib.rs"
commit_all "$R8"
if guard "$R8" 'crates/**' 'scripts/verify_M-99.sh'; then
  if [ "$(run "$R8")" -ne 0 ]; then
    ok "позитивный контроль гейта: ссылка на вынесенный verify_M-99.sh ⇒ отказ"
  else
    bad "граница гейта УБИЛА обнаружение — ссылка на вынесенный verify_M-99.sh принята"
  fi
fi

# ─── (11) СУФФИКС ГЕЙТА: вынесен `verify_M-98-umbrella-2026-08.sh`, ссылка на прежний путь ⇒ ОТКАЗ ───
# Первая редакция такой файл артефактом не считала (`verify_M-60-umbrella-2026-08.sh` в
# настоящем архиве) — его ссылки не проверялись вовсе.
# Спека M-99 в архиве ОСТАЁТСЯ (без живых ссылок): иначе барьер, не видящий суффиксный гейт,
# упал бы на «пустой архив», и проба приняла бы fail-closed за обнаружение — ровно так
# первая редакция этого сценария и была зелёной против старого барьера.
R9="$TMP/case11"; mk_repo "$R9"
: > "$R9/docs/archive/verify_M-98-umbrella-2026-08.sh"
printf '# прежний путь: bash scripts/verify_M-98.sh\n' > "$R9/crates/gateway/src/lib.rs"
commit_all "$R9"
# стражи: в архиве НЕТ `verify_M-98.sh` без суффикса (обнаружение обязано идти через суффиксную
# форму), а барьер обязан НАСЧИТАТЬ два артефакта — отказ по «пустому архиву» не засчитывается.
if [ -e "$R9/docs/archive/verify_M-98.sh" ]; then
  bad "SETUP не состоялся: в архиве есть verify_M-98.sh без суффикса — сценарий 11 не проверяет суффикс"
elif guard "$R9" 'crates/**' 'scripts/verify_M-98.sh'; then
  rc="$(run "$R9")"
  if ! grep -q 'вынесенных артефактов найдено: 2' "$TMP/last.out"; then
    bad "суффикс гейта: барьер насчитал не 2 артефакта — суффиксный гейт не признан артефактом (или архив судился как пустой)"
  elif [ "$rc" -ne 0 ]; then
    ok "суффикс гейта: verify_M-98-umbrella-*.sh считается артефактом (найдено 2), ссылка на scripts/verify_M-98.sh ⇒ отказ"
  else
    bad "вынесенный гейт с суффиксом признан артефактом, но ссылка на его прежний путь принята"
  fi
fi

# ─── (12) ЛОЖНАЯ АМНИСТИЯ: строка базовой линии, чей файл ссылается ТОЛЬКО на живой M-99b ⇒ ОТКАЗ ───
# Проверка протухания обязана использовать ТУ ЖЕ границу, что поиск: иначе восемь строк
# `…|milestones/M-60` на живые `M-60a/b/c` жили бы вечно.
R10="$TMP/case12"; mk_repo "$R10"
printf '// живой сосед: milestones/M-99b-other-subject.md\n' > "$R10/crates/gateway/src/lib.rs"
printf 'crates/gateway/src/lib.rs|milestones/M-99\n' > "$R10/scripts/lib/archived_refs_baseline.txt"
commit_all "$R10"
if guard "$R10" 'crates/**' 'milestones/M-99b' && guard "$R10" 'scripts/lib/**' 'crates/gateway/src/lib.rs|milestones/M-99'; then
  if [ "$(run "$R10")" -ne 0 ]; then
    ok "ложная амнистия: строка на файл, ссылающийся только на живой M-99b, ⇒ отказ (протухла)"
  else
    bad "ложная амнистия ПРИНЯТА — проверка протухания ищет подстрокой и видит M-99 в M-99b"
  fi
fi

printf '\n'
if [ "$BAD" -eq 0 ]; then
  echo "VERDICT: PASS — сценариев: $N, расхождений: 0"
  exit 0
fi
echo "VERDICT: FAIL — сценариев: $N, расхождений: $BAD"
exit 1
