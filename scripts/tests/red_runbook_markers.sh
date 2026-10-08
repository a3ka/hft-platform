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
expect fail "S9 маркер без опознаваемого префикса" "$d" "префикс до подстановки короче 3 знаков"

# ── S10 один найден, другой нет — всё-или-ничего — FAIL ────────────────────────────────
d=$(mk s10 <<'EOF'
`=== GOOD ===` и `=== MISSING ===`.
EOF
)
put "$d" deploy/bin/x.sh 'echo "=== GOOD ==="'
commit "$d"
tracked "$d" deploy/bin/x.sh || bad "S10 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S10 частично найденные маркеры" "$d" "miss  === MISSING ==="
# S10b (A-048 §3): итог обязан сосчитать ОБА маркера — «проверить только последний/первый»
# печатает тот же miss, но маркеров 1.
s10out="${TMP}/out.$((N-1))"
if grep -qF 'runbook-markers: маркеров 2, не найдено в коде 1' "$s10out"; then
  ok "S10b оба маркера сосчитаны, один не найден"
else
  bad "S10b итог не называет «маркеров 2, не найдено в коде 1» — проверены не все маркеры"; sed 's/^/      /' "$s10out"
fi

# ── S11 маркер печатает Rust вне tests — PASS (область поиска шире deploy/**) ─────────
d=$(mk s11 <<'EOF'
Бинарь печатает `=== план ретеншена ===`.
EOF
)
put "$d" crates/journal/src/bin/journal-retention.rs 'println!("=== план ретеншена ===");'
commit "$d"
tracked "$d" crates/journal/src/bin/journal-retention.rs || bad "S11 SETUP НЕ СОСТОЯЛСЯ"
expect pass "S11 маркер печатает код крейта" "$d"

# ── S13 исключение docs/** отдельно от *.md (`R-236` Б-1): не-.md файл под docs/ — FAIL ───
d=$(mk s13 <<'EOF2'
Ищите `=== ONLY IN DOCS SCRIPT ===`.
EOF2
)
put "$d" docs/archive/old_verify.sh 'echo "=== ONLY IN DOCS SCRIPT ==="'
commit "$d"
tracked "$d" docs/archive/old_verify.sh || bad "S13 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S13 маркер только в не-.md файле под docs/" "$d" "miss  === ONLY IN DOCS SCRIPT ==="

# ── S14 исключение research/** отдельно (по одному сценарию на исключение) — FAIL ──────
d=$(mk s14 <<'EOF2'
Ищите `=== ONLY IN RESEARCH ===`.
EOF2
)
put "$d" research/tool/run.sh 'echo "=== ONLY IN RESEARCH ==="'
commit "$d"
tracked "$d" research/tool/run.sh || bad "S14 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S14 маркер только под research/" "$d" "miss  === ONLY IN RESEARCH ==="

# ── S15 печать из «неожиданного» пути — область поиска весь репозиторий (`R-236` Н-1) — PASS
d=$(mk s15 <<'EOF2'
Ищите `=== FROM OPS SCRIPT ===` и `=== FROM COMPOSE ===`.
EOF2
)
put "$d" scripts/ops/x.sh 'echo "=== FROM OPS SCRIPT ==="'
put "$d" docker-compose.yml '    command: ["sh", "-c", "echo === FROM COMPOSE ==="]'
commit "$d"
tracked "$d" scripts/ops/x.sh || bad "S15 SETUP НЕ СОСТОЯЛСЯ"
expect pass "S15 маркер печатает scripts/ops и docker-compose.yml" "$d"

# ── S16 отличие только регистром — оператор ищет grep -F, регистр важен (`R-236` Н-2) — FAIL
d=$(mk s16 <<'EOF2'
Ищите `=== watchdog install failed ===`.
EOF2
)
put "$d" deploy/bin/install.sh 'echo "=== WATCHDOG INSTALL FAILED ==="'
commit "$d"
tracked "$d" deploy/bin/install.sh || bad "S16 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S16 маркер отличается от кода только регистром" "$d" "miss  === watchdog install failed ==="

# ── S17 стражи входа (`R-236` Н-3): нет runbook'а; корень не git — FAIL ──────────────
d=$(mk s17 <<'EOF2'
`=== X ===`
EOF2
)
put "$d" deploy/bin/x.sh 'echo "=== X ==="'
commit "$d"
rm -f "$d/deploy/README.md"
[ ! -e "$d/deploy/README.md" ] || bad "S17 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S17 runbook'а нет" "$d" "нет файла"
nd="${TMP}/s17-nogit"; mkdir -p "$nd/deploy"
printf '`=== X ===`\n' > "$nd/deploy/README.md"; printf 'echo "=== X ==="\n' > "$nd/x.sh"
# Мир «вне git» не зависит от хоста (`A-049` §4, тестер M-94 2026-10-06): на машине, где `/tmp`
# или `$TMPDIR` сам лежит в git-дереве, `mktemp -d` давал «каталог внутри git» и SETUP-отказ.
# `GIT_CEILING_DIRECTORIES=$TMP` запрещает git искать репозиторий выше `$TMP` — только для этого мира.
export GIT_CEILING_DIRECTORIES="$TMP"
git -C "$nd" rev-parse --is-inside-work-tree >/dev/null 2>&1 && bad "S17b SETUP НЕ СОСТОЯЛСЯ: каталог внутри git"
expect fail "S17b корень — не рабочее дерево git" "$nd" "не рабочее дерево git"
unset GIT_CEILING_DIRECTORIES

# ── S18 извлечение (`R-236` Н-4/Н-6): хвост разорванной цитаты не склеивается с соседним
#    маркером, setext-подчёркивание не маркер — честный маркер на той же строке проверен ──
d=$(mk s18 <<'EOF2'
Заголовок
=========
Строка `=== WATCHDOG INSTALL
OK ===` и `=== MISSING HERE ===`.
EOF2
)
put "$d" deploy/bin/x.sh 'echo "=== OTHER ==="'
commit "$d"
grep -q '^=========$' "$d/deploy/README.md" || bad "S18 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S18 соседний маркер после разорванной цитаты извлечён и проверен" "$d" "miss  === MISSING HERE ==="
s18out="${TMP}/out.$((N-1))"
if grep -qE '^(ok|miss) +=====|` и `' "$s18out"; then
  bad "S18b извлечён мусорный маркер (setext или склейка через бэктик)"; sed 's/^/      /' "$s18out"
else
  ok "S18b мусорных маркеров нет"
fi

# ── S19 маркер только в копии САМОЙ библиотеки — она не код (`R-238` Б-1а) — FAIL ──────
d=$(mk s19 <<'EOF2'
Ищите `=== SELF ONLY ===`.
EOF2
)
printf '# пример: === SELF ONLY ===\n' >> "$d/scripts/lib/runbook_markers.sh"
commit "$d"
grep -qF 'SELF ONLY' "$d/scripts/lib/runbook_markers.sh" || bad "S19 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S19 маркер только в файле самой проверки" "$d" "miss  === SELF ONLY ==="

# ── S20 метасимвол в маркере — сравнение ДОСЛОВНОЕ, не шаблоном (`R-238` Б-1б) — FAIL ────
d=$(mk s20 <<'EOF2'
Ищите `=== retention v1.2 ===`.
EOF2
)
put "$d" deploy/bin/x.sh 'echo "=== retention v1x2 ==="'
commit "$d"
grep -qF 'v1x2' "$d/deploy/bin/x.sh" || bad "S20 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S20 код отличается на метасимвол (.)" "$d" "miss  === retention v1.2 ==="

# ── S21 порог префикса — величина, а не только пустота (`R-238` Н-2) — FAIL ─────────────
d=$(mk s21 <<'EOF2'
Ищите `=== A… ===`.
EOF2
)
put "$d" deploy/bin/x.sh 'echo "=== ABSENT ==="'
commit "$d"
tracked "$d" deploy/bin/x.sh || bad "S21 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S21 префикс из одного знака" "$d" "префикс до подстановки короче 3 знаков"

# ── S22 tests/ в КОРНЕ репозитория — тоже не код (`R-238` Н-3) — FAIL ───────────────────
d=$(mk s22 <<'EOF2'
Ищите `=== ROOT TESTS ===`.
EOF2
)
put "$d" tests/t.sh 'grep -q "=== ROOT TESTS ===" out'
commit "$d"
tracked "$d" tests/t.sh || bad "S22 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S22 маркер только в корневом tests/" "$d" "miss  === ROOT TESTS ==="

# ── S21b порог — величина 3, не 2 (A-048, не условие) — FAIL ───────────────────────────
d=$(mk s21b <<'EOF2'
Ищите `=== AB… ===`.
EOF2
)
put "$d" deploy/bin/x.sh 'echo "=== ABSENT ==="'
commit "$d"
tracked "$d" deploy/bin/x.sh || bad "S21b SETUP НЕ СОСТОЯЛСЯ"
expect fail "S21b префикс из двух знаков" "$d" "префикс до подстановки короче 3 знаков"

# ── S23 скобки — не место подстановки: код отличается внутри скобок — FAIL (A-048) ─────
d=$(mk s23 <<'EOF2'
Ищите `=== компакция (D-COMP-3) ===`.
EOF2
)
put "$d" crates/journal/src/bin/r.rs 'println!("=== компакция (D-COMP-4) ===");'
commit "$d"
tracked "$d" crates/journal/src/bin/r.rs || bad "S23 SETUP НЕ СОСТОЯЛСЯ"
expect fail "S23 код отличается внутри скобок" "$d" "miss  === компакция (D-COMP-3) ==="

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
