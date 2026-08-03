#!/usr/bin/env bash
# Acceptance-гейт M-58 — метрика жизненного цикла уровня считается НА ЖИЗНЬ, не на цену.
# Гейт архитектора; dev его не правит (scope-guard.md).
#
# Дисциплина (gates.md §3): никакого `cmd && echo PASS || echo FAIL` — каждая проверка
# инкрементирует FAIL-счётчик; setup, который не смог выполниться, — это FAIL, а не тихий PASS.
set -euo pipefail

FAILURES=0
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

say()  { printf '%s\n' "$*"; }
pass() { say "PASS  $*"; }
fail() { say "FAIL  $*"; FAILURES=$((FAILURES + 1)); }

say "=== M-58 acceptance ==="
say "дерево: $(pwd)"
say "HEAD:   $(git log -1 --format='%h %s' 2>/dev/null || echo '<не git>')"
say

# ── setup-guard: предмет проверки обязан существовать ──────────────────────────────────────
ORACLE="crates/research-cli/tests/red_depth_lifetime_perlife.rs"
IMPL="crates/research-cli/src/depth_lifetime.rs"
for f in "$ORACLE" "$IMPL" "milestones/M-58-depth-metric.md"; do
  if [ -f "$f" ]; then
    pass "предмет на месте: $f"
  else
    fail "предмет ОТСУТСТВУЕТ: $f — проверять нечего"
  fi
done
[ "$FAILURES" -eq 0 ] || { say; say "VERDICT: FAIL (setup)"; exit 1; }

# ── Задача 1 — новые оракулы GREEN ────────────────────────────────────────────────────────
say
if cargo test -p research-cli --test red_depth_lifetime_perlife 2>&1 | tee /tmp/m58_new.log \
   | grep -qE '^test result: ok\.'; then
  pass "задача 1 — DV-I-10..14 GREEN ($(grep -E '^test result' /tmp/m58_new.log | tail -1))"
else
  fail "задача 1 — DV-I-10..14 не зелёные:"
  tail -30 /tmp/m58_new.log
fi

# Все пять оракулов обязаны реально ПРИСУТСТВОВАТЬ (защита от «зелено, потому что пусто»).
RAN="$(grep -cE '^test dv_i_1[01234]_' /tmp/m58_new.log || true)"
if [ "${RAN:-0}" -eq 5 ]; then
  pass "задача 1 — прогнаны все 5 оракулов DV-I-10..14"
else
  fail "задача 1 — прогнано оракулов DV-I-10..14: ${RAN:-0}, ожидалось 5"
fi

# ── Регресс — DV-I-1..9 остаются GREEN ────────────────────────────────────────────────────
say
for t in red_depth_lifetime red_depth_band_3060; do
  if cargo test -p research-cli --test "$t" 2>&1 | tee "/tmp/m58_$t.log" \
     | grep -qE '^test result: ok\.'; then
    pass "регресс — $t GREEN"
  else
    fail "регресс — $t сломан переспекой:"
    tail -20 "/tmp/m58_$t.log"
  fi
done

# ── Задача 1/2 — семантика в типах: счётчики называются lives_* ───────────────────────────
say
if grep -qE 'pub lives_born:\s*u64' "$IMPL" \
   && grep -qE 'pub lives_cancelled:\s*u64' "$IMPL" \
   && grep -qE 'pub lives_frozen:\s*u64' "$IMPL" \
   && grep -qE 'pub lives_censored:\s*u64' "$IMPL"; then
  pass "задача 1 — счётчики переименованы в lives_* (смысл виден в типе)"
else
  fail "задача 1 — в $IMPL нет полного набора lives_born/cancelled/frozen/censored"
fi

if grep -qE 'pub (born|cancelled|frozen|censored):\s*u64' "$IMPL"; then
  fail "задача 1 — остались СТАРЫЕ поля born/cancelled/frozen/censored: имя означает другую величину"
else
  pass "задача 1 — старых полей не осталось"
fi

# ── Задача 2 — сборка всех целей крейта и чистый clippy ───────────────────────────────────
say
# Решение по КОДУ ВОЗВРАТА, а не по виду вывода: `... | grep -qv '^error'` истинно от любой
# не-error строки и маскирует провал (gates.md §3).
if cargo build -p research-cli --all-targets > /tmp/m58_build.log 2>&1; then
  pass "задача 2 — cargo build --all-targets прошёл (call-sites обновлены)"
else
  fail "задача 2 — сборка целей крейта падает: call-sites не обновлены"
  tail -15 /tmp/m58_build.log
fi

if cargo clippy -p research-cli --all-targets -- -D warnings > /tmp/m58_clippy.log 2>&1; then
  pass "задача 2 — clippy чист"
else
  fail "задача 2 — clippy ругается:"
  tail -15 /tmp/m58_clippy.log
fi

# ── Задача 3 — числа пересъёмки предъявлены ───────────────────────────────────────────────
say
RESULTS="research/data-quality/depth-lifetime-results.md"
if grep -q 'M-58' "$RESULTS" && grep -q 'lives_' "$RESULTS"; then
  pass "задача 3 — §M-58 с per-life числами дописан в $RESULTS"
else
  fail "задача 3 — в $RESULTS нет секции M-58 с per-life числами (пересъёмка не предъявлена)"
fi

# «segment 78» упомянут в файле с 24.07 — на нём этот гейт давал ложный PASS. Требуем
# ЗАПИСАННЫЕ УСЛОВИЯ именно этого прогона (DESIGN.md §16.3: замер без условий недействителен).
if grep -q 'УСЛОВИЯ ПРОГОНА M-58' "$RESULTS" && grep -qiE 'segment[ -]?78' "$RESULTS"; then
  pass "задача 3 — условия прогона M-58 записаны и сегмент назван"
else
  fail "задача 3 — нет блока 'УСЛОВИЯ ПРОГОНА M-58' (сегмент, окно, число дельт): замер без записанных условий недействителен"
fi

# ── Задача 4 — вердикт обновлён и явно говорит про замок ──────────────────────────────────
say
VERDICT_DOC="research/data-quality/depth-verdict.md"
# ВНИМАНИЕ. Просто «упоминается M-58 и A-002» — ложный PASS: обе строки попали в документ
# ещё пометкой заморозки, ДО всякой пересъёмки (поймано прогоном этого гейта до impl).
# Требуем маркер, который физически нечем поставить, кроме как обновив вердикт числами.
if grep -q 'M-58 ПЕРЕСНЯТО' "$VERDICT_DOC" && grep -q 'lives_cancelled' "$VERDICT_DOC"; then
  pass "задача 4 — вердикт обновлён по НОВЫМ per-life числам (маркер + lives_cancelled)"
else
  fail "задача 4 — $VERDICT_DOC не обновлён: нет маркера 'M-58 ПЕРЕСНЯТО' с per-life числами"
fi

if grep -qE 'замок A-002 (СНЯТ|ОСТАЁТСЯ)' "$VERDICT_DOC"; then
  pass "задача 4 — вердикт явно называет судьбу замка"
else
  fail "задача 4 — вердикт не говорит прямо: 'замок A-002 СНЯТ' или 'замок A-002 ОСТАЁТСЯ'"
fi

# ── Задача 5 — ограничение M-33 названо ───────────────────────────────────────────────────
say
# Тот же класс ловушки: слова «по обе стороны» есть в документе с 24.07 (формулировка
# founder-флага). Требуем именованное ОГРАНИЧЕНИЕ, а не совпадение словосочетания.
if grep -q 'ОГРАНИЧЕНИЕ M-33' "$VERDICT_DOC"; then
  pass "задача 5 — слепота founder-флага M-33 к односторонней заморозке названа ограничением"
else
  fail "задача 5 — в вердикте нет раздела 'ОГРАНИЧЕНИЕ M-33' (конъюнкция «по обе стороны»)"
fi

# ── Замок A-002 З-1 — не снят молча ───────────────────────────────────────────────────────
say
BANDS_DEFAULT="$(grep -oE 'GATEWAY_BANDS:-[0-9.]+' docker-compose.yml | head -1 | sed 's/.*:-//' || true)"
if [ -z "$BANDS_DEFAULT" ]; then
  fail "замок — не удалось прочитать дефолт GATEWAY_BANDS из docker-compose.yml (проверка не состоялась)"
elif awk -v b="$BANDS_DEFAULT" 'BEGIN { exit (b <= 0.013) ? 0 : 1 }'; then
  pass "замок A-002 З-1 держится: дефолт полос $BANDS_DEFAULT внутри валидированной зоны (<=0.013)"
else
  fail "замок A-002 З-1 СНЯТ МОЛЧА: дефолт полос $BANDS_DEFAULT глубже 1.3% без обновлённого вердикта"
fi

say
if [ "$FAILURES" -eq 0 ]; then
  say "VERDICT: PASS"
  exit 0
else
  say "VERDICT: FAIL ($FAILURES проверок не прошло)"
  exit 1
fi
