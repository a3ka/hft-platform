#!/usr/bin/env bash
# Проба барьера закрытия сессии. Предмет: scripts/check_unreachable_work.sh
# Барьер, чьё КРАСНОЕ не предъявлено, считается отсутствующим (harness-track §5 п.1).
set -uo pipefail
BARRIER="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/check_unreachable_work.sh"
TMP="$(mktemp -d /tmp/red-unreach-XXXXXX)"; REG="$TMP/.reg"; echo "$TMP" > "$REG"
cleanup(){ while read -r d; do [ -n "$d" ] && rm -rf "$d"; done < "$REG"; }; trap cleanup EXIT
PASS=0; FAIL=0
chk(){ local why="$1" want="$2" root="$3" got
  ROOT="$root" bash "$BARRIER" >/dev/null 2>&1; got=$?
  if [ "$got" -eq "$want" ]; then PASS=$((PASS+1)); printf 'ok    %-52s код %s\n' "$why" "$got"
  else FAIL=$((FAIL+1)); printf 'FAIL  %-52s ожидался %s, получен %s\n' "$why" "$want" "$got"; fi; }

# Мир: bare-origin + клон + worktree с коммитом, НЕ доехавшим до origin.
# Всё на `git -C`, без хождения по каталогам: `cd` внутри подстановки уже дал ошибку.
mk(){
  local name="$1" path="${2:-}"
  local o="$TMP/$name-o" c="$TMP/$name" w="$TMP/$name-wt"
  echo "$o" >> "$REG"; echo "$c" >> "$REG"
  git init -q --bare "$o"
  git init -q "$c"
  git -C "$c" config user.email a@b; git -C "$c" config user.name t
  git -C "$c" remote add origin "$o"
  mkdir -p "$c/seed"; echo base > "$c/seed/base.txt"
  git -C "$c" add seed/base.txt >/dev/null 2>&1
  git -C "$c" commit -qm base -- seed/base.txt >/dev/null 2>&1
  git -C "$c" push -q origin HEAD:refs/heads/main >/dev/null 2>&1
  git -C "$c" fetch -q origin >/dev/null 2>&1
  if [ -n "$path" ]; then
    echo "$w" >> "$REG"
    git -C "$c" worktree add -q "$w" -b lost HEAD >/dev/null 2>&1
    mkdir -p "$w/$(dirname "$path")"; echo "содержимое" > "$w/$path"
    git -C "$w" add "$path" >/dev/null 2>&1
    git -C "$w" commit -qm "не доехал до origin" -- "$path" >/dev/null 2>&1
  fi
  printf '%s' "$c"
}

echo "--- ПОЗИТИВНЫЙ КОНТРОЛЬ ---"
chk "всё доехало до origin — зелено" 0 "$(mk clean '')"
echo "--- ОБМАННЫЕ СТАБЫ: барьер обязан покраснеть ---"
chk "НЕДОСТИЖИМ вердикт критика (реальный случай 01.09)" 1 "$(mk verdict research/critiques/C-999-x.md)"
chk "НЕДОСТИЖИМА спека милестоуна"                       1 "$(mk spec milestones/M-99-x.md)"
chk "НЕДОСТИЖИМ документ docs/"                          1 "$(mk doc docs/workflow/x.md)"
echo "--- НЕ артефакт: одноразовая фикстура критика ---"
chk "недостижим, но артефактов не несёт — PASS с NOTE"   0 "$(mk fixture crates/x/tests/red_probe.rs)"
echo "--- SETUP не состоялся ---"
mkdir -p "$TMP/notgit"; echo "$TMP/notgit" >> "$REG"
chk "не git-репозиторий" 2 "$TMP/notgit"
chk "каталога нет"       2 "$TMP/нет-такого"
echo; echo "каталогов после прогона: $(ls -d /tmp/red-unreach-* 2>/dev/null | wc -l) (trap снимет)"
echo "PASS=$PASS FAIL=$FAIL"; [ "$FAIL" -eq 0 ] || { echo "VERDICT: FAIL"; exit 1; }
echo "VERDICT: PASS — $PASS сценариев"
