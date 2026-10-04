#!/usr/bin/env bash
# red_runbook_markers.sh — ДВУСТОРОННЯЯ ПРОБА проверки маркеров runbook'а
# (scripts/lib/runbook_markers.sh, шаг D6b гейта доставки; TD-243, A-047 §2 п.1).
#
# Каждый сценарий строит ОДНОРАЗОВЫЙ git-репозиторий: проверка ищет через `git grep`, и
# подсунуть ей файл мимо git нельзя. У каждого сценария — страж setup'а: проба, молча
# гоняющая не тот сценарий, — плацебо самой себя. Число сценариев СЧИТАЕТСЯ прогоном.
#
# RUNBOOK_MARKERS_LIB — путь к проверяемой библиотеке (по умолчанию настоящая); нужен
# мутационному контролю: мутант подставляется сюда, проба обязана покраснеть.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
LIB="${RUNBOOK_MARKERS_LIB:-${ROOT}/scripts/lib/runbook_markers.sh}"
N=0; BAD=0
ok()  { N=$((N+1)); printf 'ok    %s\n' "$1"; }
bad() { N=$((N+1)); BAD=$((BAD+1)); printf 'FAIL  %s\n' "$1"; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT INT TERM

[ -f "${LIB}" ] || { echo "FAIL  нет библиотеки ${LIB}"; exit 1; }
# shellcheck source=../lib/runbook_markers.sh
source "${LIB}"

# mk <имя> — пустой репозиторий с runbook'ом deploy/README.md (содержимое — stdin)
mk() {
  local d="${TMP}/$1"
  mkdir -p "$d/deploy/bin" "$d/scripts/lib"
  git -C "$d" init -q
  git -C "$d" config user.email probe@local; git -C "$d" config user.name probe
  cat > "$d/deploy/README.md"
  cp "${LIB}" "$d/scripts/lib/runbook_markers.sh"
  printf '%s' "$d"
}
# put <repo> <путь> <содержимое> — файл, ЗАКОММИЧЕННЫЙ в репо
put() { mkdir -p "$(dirname "$1/$2")"; printf '%s\n' "$3" > "$1/$2"; git -C "$1" add -- "$2"; }
commit() { git -C "$1" add -- deploy/README.md scripts/lib/runbook_markers.sh; git -C "$1" commit -qm fixture; }
# tracked <repo> <путь> — страж: файл действительно отслеживается
tracked() { git -C "$1" ls-files --error-unmatch -- "$2" >/dev/null 2>&1; }

# expect <pass|fail> <имя сценария> <repo> [<причина>]
# Отказ засчитывается, только если вывод называет ОЖИДАЕМУЮ причину (фиксированная подстрока):
# отказ «по любой причине» пропустил бы мутанта, который краснеет на всём подряд.
expect() {
  local want="$1" name="$2" d="$3" why="${4:-}" rc
  runbook_markers_check "$d/deploy/README.md" "$d" > "${TMP}/out.$N" 2>&1; rc=$?
  if [ "$want" = fail ] && [ "$rc" -ne 0 ] && [ -n "$why" ] && ! grep -qF -- "$why" "${TMP}/out.$N"; then
    bad "$name — отказ есть, но не по причине «$why»"; sed 's/^/      /' "${TMP}/out.$N"; return
  fi
  if { [ "$want" = pass ] && [ "$rc" -eq 0 ]; } || { [ "$want" = fail ] && [ "$rc" -ne 0 ]; }; then
    ok "$name (rc=$rc)"
  else
    bad "$name — ожидалось ${want}, rc=$rc"; sed 's/^/      /' "${TMP}/out.$N"
  fi
}

# ── S1 позитивный контроль: маркер печатает скрипт — PASS ──────────────────────────────
d=$(mk s1 <<'EOF'
Ищите в логе `=== WATCHDOG INSTALL FAILED ===`.
EOF
)
put "$d" deploy/bin/install.sh 'cp x y || { echo "=== WATCHDOG INSTALL FAILED ===" >&2; exit 1; }'
commit "$d"
tracked "$d" deploy/bin/install.sh || bad "S1 SETUP НЕ СОСТОЯЛСЯ"
expect pass "S1 маркер печатается кодом" "$d"

# ── S2 позитивный: подстановка (…, <sha>) — сравнение по префиксу — PASS ───────────────
d=$(mk s2 <<'EOF'
Строка `=== DEPLOY FAILED — logs + rollback to … ===`, полностью
`=== DEPLOY FAILED — logs + rollback to <sha> ===`; и `=== deployed $SHA ===`.
EOF
)
put "$d" .github/workflows/deploy.yml '  echo "=== DEPLOY FAILED — logs + rollback to $(git rev-parse --short "$PREV") ==="
  echo "=== deployed $(git rev-parse HEAD) ==="'
commit "$d"
tracked "$d" .github/workflows/deploy.yml || bad "S2 SETUP НЕ СОСТОЯЛСЯ"
expect pass "S2 маркер с подстановкой сверяется по префиксу" "$d"

# ── S3 маркера в коде нет (исходный TD-239: «INSTALL OK») — FAIL ───────────────────────
d=$(mk s3 <<'EOF'
Успех: `=== WATCHDOG INSTALL OK ===`.
EOF
)
put "$d" deploy/bin/install.sh 'echo "=== WATCHDOG INSTALL FAILED ==="'
commit "$d"
grep -qF 'INSTALL FAILED' "$d/deploy/bin/install.sh" || bad "S3 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S3 маркер, которого код не печатает" "$d" "miss  === WATCHDOG INSTALL OK ==="

# ── S4 усечённый маркер без подстановки (R-231 N-1) — FAIL ─────────────────────────────
d=$(mk s4 <<'EOF'
Ищите `=== DEPLOY FAILED ===`.
EOF
)
put "$d" .github/workflows/deploy.yml 'echo "=== DEPLOY FAILED — logs + rollback to $X ==="'
commit "$d"
tracked "$d" .github/workflows/deploy.yml || bad "S4 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S4 усечённая цитата без места подстановки" "$d" "miss  === DEPLOY FAILED ==="

# ── S5 маркер есть ТОЛЬКО в документах — runbook не подтверждает сам себя — FAIL ───────
d=$(mk s5 <<'EOF'
Ищите `=== ONLY IN DOCS ===`.
EOF
)
put "$d" docs/notes.md 'см. `=== ONLY IN DOCS ===`'
put "$d" deploy/OTHER.md '=== ONLY IN DOCS ==='
put "$d" research/r.txt '=== ONLY IN DOCS ==='
commit "$d"
{ tracked "$d" docs/notes.md && tracked "$d" research/r.txt; } || bad "S5 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S5 маркер только в *.md / docs / research" "$d" "miss  === ONLY IN DOCS ==="

# ── S6 маркер только в тестах — тест проверяет печать, а не печатает — FAIL ────────────
d=$(mk s6 <<'EOF'
Ищите `=== ONLY IN TESTS ===`.
EOF
)
put "$d" crates/a/tests/t.rs 'assert!(out.contains("=== ONLY IN TESTS ==="));'
put "$d" scripts/tests/red_x.sh 'grep -q "=== ONLY IN TESTS ===" out'
commit "$d"
tracked "$d" crates/a/tests/t.rs || bad "S6 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S6 маркер только в */tests/*" "$d" "miss  === ONLY IN TESTS ==="

# ── S7 маркер только в НЕОТСЛЕЖИВАЕМОМ файле — FAIL ────────────────────────────────────
d=$(mk s7 <<'EOF'
Ищите `=== UNTRACKED ONLY ===`.
EOF
)
commit "$d"
printf 'echo "=== UNTRACKED ONLY ==="\n' > "$d/deploy/bin/local.sh"
{ [ -f "$d/deploy/bin/local.sh" ] && ! tracked "$d" deploy/bin/local.sh; } || bad "S7 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S7 маркер только в файле вне git" "$d" "miss  === UNTRACKED ONLY ==="

# ── S8 ноль маркеров — извлечение пусто, проверять нечего — FAIL ───────────────────────
d=$(mk s8 <<'EOF'
Runbook без единого маркера.
EOF
)
commit "$d"
grep -q '===' "$d/deploy/README.md" && bad "S8 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S8 runbook без маркеров (fail-closed)" "$d" "ни одного маркера"

# ── S9 подстановка в самом начале — префикс пуст — FAIL ────────────────────────────────
d=$(mk s9 <<'EOF'
Ищите `=== <что угодно> ===`.
EOF
)
put "$d" deploy/bin/x.sh 'echo "=== что угодно ==="'
commit "$d"
tracked "$d" deploy/bin/x.sh || bad "S9 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S9 маркер без опознаваемого префикса" "$d" "префикс до подстановки пуст"

# ── S10 один найден, другой нет — всё-или-ничего — FAIL ────────────────────────────────
d=$(mk s10 <<'EOF'
`=== GOOD ===` и `=== MISSING ===`.
EOF
)
put "$d" deploy/bin/x.sh 'echo "=== GOOD ==="'
commit "$d"
tracked "$d" deploy/bin/x.sh || bad "S10 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S10 частично найденные маркеры" "$d" "miss  === MISSING ==="

# ── S11 маркер печатает Rust вне tests — PASS (область поиска шире deploy/**) ─────────
d=$(mk s11 <<'EOF'
Бинарь печатает `=== план ретеншена ===`.
EOF
)
put "$d" crates/journal/src/bin/journal-retention.rs 'println!("=== план ретеншена ===");'
commit "$d"
tracked "$d" crates/journal/src/bin/journal-retention.rs || bad "S11 SETUP НЕ СОСТОЯЛСЯ"
expect pass "S11 маркер печатает код крейта" "$d"

# ── S12 настоящий runbook репозитория — PASS (прод-форма вызова, как в D6b) ────────────
if [ -f "${ROOT}/deploy/README.md" ] && grep -q '===' "${ROOT}/deploy/README.md"; then
  expect pass "S12 настоящий deploy/README.md против настоящего кода" "${ROOT}"
else
  bad "S12 SETUP НЕ СОСТОЯЛСЯ: в настоящем deploy/README.md нет маркеров"
fi

echo
rm -rf "$TMP"; trap - EXIT INT TERM
left=0; [ -e "$TMP" ] && left=1
echo "сценариев: ${N}, провалов: ${BAD}, каталогов фикстур после прогона: ${left}"
[ "$left" -eq 0 ] || BAD=$((BAD+1))
if [ "${BAD}" -gt 0 ]; then echo "VERDICT: FAIL"; exit 1; fi
echo "VERDICT: PASS"
