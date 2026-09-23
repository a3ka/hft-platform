#!/usr/bin/env bash
# red_archived_refs.sh — ДВУСТОРОННЯЯ ПРОБА барьера висячих ссылок.
#
# Барьер сам под пробой (`gates.md` §9): односторонняя проверка зелена и у барьера,
# который всегда возвращает 0. Здесь предъявляются ОБЕ стороны и, отдельно, два свойства
# базовой линии — иначе список унаследованного долга превращается в вечную амнистию.
#
# Проба строит ОДНОРАЗОВЫЙ репозиторий: барьер работает через `git grep`, и подсунуть ему
# файлы мимо git нельзя. Число сценариев СЧИТАЕТСЯ прогоном, а не заявляется.

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
run() { ( cd "$1" && bash scripts/check_archived_refs.sh >/dev/null 2>&1; echo $? ); }

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

printf '\n'
if [ "$BAD" -eq 0 ]; then
  echo "VERDICT: PASS — сценариев: $N, расхождений: 0"
  exit 0
fi
echo "VERDICT: FAIL — сценариев: $N, расхождений: $BAD"
exit 1
