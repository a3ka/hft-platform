#!/usr/bin/env bash
# verify_M-87.sh — acceptance-гейт milestone'а M-87 «предохранитель выдачи».
# Спека: milestones/M-87-serving-circuit-breaker.md
#
# Решение по КОДУ ВОЗВРАТА (`gates.md` §3). Агрегатор с FAIL-счётчиком: гейт обязан
# перечислить ВСЕ провалы, а не умереть на первом.
#
# Базовая линия снимается КРАСНОЙ до работы dev'а и предъявляется в Handoff.

set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP  %s\n' "$1"; }

GW=crates/gateway/src/lib.rs
GS=crates/gateway-serve/src
SPEC=milestones/M-87-serving-circuit-breaker.md

# ─────────────── задача 1 — контракт исходов ───────────────
if [ -f "$GS/admission.rs" ] \
   && grep -qE 'pub enum ServingOutcome' "$GS/admission.rs" \
   && grep -qE 'Ready' "$GS/admission.rs" && grep -qE 'Warming' "$GS/admission.rs" \
   && grep -qE 'NotReady' "$GS/admission.rs" && grep -qE 'Unsupported' "$GS/admission.rs" \
   && grep -qE 'Overloaded' "$GS/admission.rs"; then
  pass "task1: пять исходов объявлены в admission.rs"
else
  fail "task1: нет модуля admission.rs с пятью исходами ServingOutcome"
fi

# ─────────────── задача 2 — готовность судится ТАМ, ГДЕ ЖИВЁТ ПРЕДОХРАНИТЕЛЬ ───────────────
# A-037 D-1 следствие 1: файл `crates/gateway/tests/red_m87_cold_path_reads_nothing.rs` ИЗЪЯТ.
# Он пиннил на ОБЩЕЙ функции LiveReducer::resume поведение, противоположное двум другим
# sacred-оракулам того же корпуса на той же функции — набор был невыполним против самого
# себя. Четыре состояния слепка переехали в транспорт: юнит-оракулы `readiness()` в
# red_m87_admission.rs и WS-оракулы над журналом-ловушкой в red_m87_entrypoint.rs.
if [ -f crates/gateway/tests/red_m87_cold_path_reads_nothing.rs ]; then
  fail "task2: изъятый файл red_m87_cold_path_reads_nothing.rs вернулся — набор снова противоречит сам себе (A-037 D-1)"
else
  pass "task2: библиотечный оракул изъят; готовность судится в транспорте"
fi

# ─────────────── задачи 1+3+4+7 — форма допуска, политика, бюджет, секрет ───────────────
AD_OUT=$(cargo test -p gateway-serve --test red_m87_admission 2>&1)
AD_RC=$?
AD_LINE=$(printf '%s\n' "$AD_OUT" | grep -E '^test result' | tail -1)
if [ $AD_RC -eq 0 ]; then
  pass "task1+3+4+7: red_m87_admission — ${AD_LINE:-GREEN}"
else
  fail "task1+3+4+7: red_m87_admission КРАСЕН — ${AD_LINE:-компиляция}"
  printf '%s\n' "$AD_OUT" | grep -E '^(error|thread |assertion)' | head -20
fi

# ─────────────── задачи 2+3+5+6+8 — ОРАКУЛ ТОЧКИ ВХОДА (C-234 R2/R3) ───────────────
# Несущая проверка поставки: предохранитель судится на РЕАЛЬНОМ публичном пути
# (WS entrypoint), а не рядом с ним. Без неё GREEN достижим без подключения допуска.
EP_OUT=$(cargo test -p gateway-serve --test red_m87_entrypoint 2>&1)
EP_RC=$?
EP_LINE=$(printf '%s\n' "$EP_OUT" | grep -E '^test result' | tail -1)
if [ $EP_RC -eq 0 ]; then
  pass "task2+3+5+6+8: red_m87_entrypoint — ${EP_LINE:-GREEN}"
else
  fail "task2+3+5+6+8: red_m87_entrypoint КРАСЕН — ${EP_LINE:-компиляция}"
  printf '%s\n' "$EP_OUT" | grep -E '^(error|thread |assertion)' | head -15
fi

# ─────────────── задача 4 — счётчик ПРОЧИТАННЫХ БАЙТ существует ───────────────
if grep -qE '^\s*pub payload_bytes_read: u64,' "$GW"; then
  pass "task4: ReadStats несёт payload_bytes_read"
else
  fail "task4: в ReadStats нет payload_bytes_read — бюджет по байтам мерить нечем"
fi

# ─────────────── задача 5 — ограничитель параллелизма существует ───────────────
SEM=$(grep -rl 'Semaphore\|max_concurrent_serves' "$GS" 2>/dev/null | wc -l)
if [ "$SEM" -gt 0 ]; then
  pass "task5: ограничитель параллелизма присутствует (файлов: $SEM)"
else
  fail "task5: ограничителя параллелизма нет ни в одном файле $GS (найдено: $SEM)"
fi

# ─────────────── задача 6 — счётчики выдачи ───────────────
if [ -f "$GS/metrics.rs" ] && grep -qE 'refusals_supported' "$GS/metrics.rs" \
   && grep -qE 'refusals_unsupported' "$GS/metrics.rs"; then
  pass "task6: счётчики выдачи разделяют поддержанные и неподдержанные отказы"
else
  fail "task6: нет metrics.rs с раздельными счётчиками отказов"
fi

# ─────────────── задача 7 — единая трактовка секрета ───────────────
if grep -qE 'pub fn key_material' "$GS/lib.rs" \
   && grep -qE 'key_material' "$GS/bin/wsprobe.rs"; then
  pass "task7: секрет трактуется ОДНОЙ функцией, её зовут обе стороны"
else
  fail "task7: key_material отсутствует либо зонд её не зовёт — трактовок по-прежнему две (TD-207)"
fi

# ─────────────── задача 8 — свежесть четырьмя позициями ───────────────
FRESH=$(grep -rcE 'freshness|свежест' "$GS"/*.rs 2>/dev/null | awk -F: '{s+=$2} END{print s+0}')
if [ "$FRESH" -gt 0 ]; then
  pass "task8: свежесть представлена в коде выдачи (совпадений: $FRESH)"
else
  fail "task8: свежести нет ни в одном файле выдачи (совпадений: $FRESH)"
fi

# ─────────────── задача 9 — лимиты У КАЖДОГО ИЗ ТРЁХ КЛАССОВ СЕРВИСА ───────────────
# C-234 R4: прежняя проверка считала строки и зеленела от ЛЮБЫХ четырёх — она не связывала
# лимит с сервисом. Теперь каждый из трёх поимённо названных сервисов обязан нести И
# ограничение процессора, И ограничение памяти. Пропуск одного сервиса роняет шаг.
lim_for_service() {
  # A-037 У-8: считаются ТОЛЬКО ключи УРОВНЯ СЕРВИСА (отступ ровно 4 пробела) либо
  # `deploy.resources.limits.*`. Блоки `labels:`/`environment:` исключены — иначе шаг
  # зеленеет от строки `cpus: "2"` внутри меток, что арбитр и воспроизвёл (мутация M-9a).
  awk -v svc="  $1:" '
    $0 == svc { inblock = 1; next }
    inblock && /^  [a-z][a-z0-9_-]*:/ { inblock = 0 }
    inblock && /^    [a-z_]+:/ { sub(/:.*/, ""); sub(/^ +/, ""); cur = $0 }
    inblock && /^    (cpus|mem_limit|memory):/ { n++; next }
    inblock && /^ +(cpus|memory):/ && cur == "deploy" { n++; next }
    END { print n + 0 }
  ' docker-compose.yml
}
LIM_MISSING=""
for svc in gateway-serve recorder gateway-checkpoint; do
  n=$(lim_for_service "$svc")
  [ "$n" -ge 2 ] || LIM_MISSING="$LIM_MISSING $svc($n)"
done
if [ -z "$LIM_MISSING" ]; then
  pass "task9: процессор И память ограничены у ВСЕХ трёх классов (выдача, запись, прогреватель)"
else
  fail "task9: нет пары лимитов уровня сервиса у:$LIM_MISSING — ключи внутри labels/environment не считаются"
fi
skip "task9: ФАКТИЧЕСКИ применённые лимиты, запас для recorder'а и поведение НА лимите снимаются на проде (docker inspect) — шаг деплой-гейта §8; замер 2026-09-21 дал NanoCpus=0 Memory=0 при живом описании сервисов"

# ─────────────── задача 10 — решение по КАЖДОМУ файлу корпуса ───────────────
# C-234 R4: прежняя проверка требовала лишь исчезновения одной фразы-плейсхолдера и не
# ловила ПРОПУСК решения. Теперь состав снимается ТЕМИ ЖЕ командами, что в §16 спеки, и
# каждый найденный файл обязан быть НАЗВАН в таблице решений §16.1.
CORPUS=$( { grep -rl 'checkpoint_dir: None' crates/gateway-serve/tests/ 2>/dev/null;
            grep -rl 'full_replay\|resume_without_checkpoint\|rebuilds' crates/gateway/tests/ 2>/dev/null; } \
          | xargs -r -n1 basename | sort -u )
DECISIONS=$(awk '/^### 16.1. Решения по изменённым ожиданиям/,0' "$SPEC")
MISSING=""
for f in $CORPUS; do
  # A-037 У-8: решение ищется В ТОЙ ЖЕ строке, что имя файла, и обязано нести токен.
  # Прежний предикат принимал строку с именем и ПУСТЫМ решением (ложно-зелёный, мутация M-10b).
  printf '%s' "$DECISIONS" | grep -- "$f" | grep -qE 'ПЕРЕНОСИТСЯ|НЕ ЗАТРАГИВАЕТСЯ|УДАЛЯЕТСЯ' \
    || MISSING="$MISSING $f"
done
CORPUS_N=$(printf '%s\n' $CORPUS | grep -c . || true)
# Два ТЕСТ-уровневых решения (A-037 D-4) — по имени теста, не файла.
for t in resume_without_checkpoint_reports_full_replay o5_broken_checkpoint; do
  printf '%s' "$DECISIONS" | grep -q -- "$t" || MISSING="$MISSING тест:$t"
done
if [ -n "$CORPUS" ] && [ -z "$MISSING" ]; then
  pass "task10: решение с токеном записано по каждому из $CORPUS_N файлов и по двум тестам D-4"
else
  fail "task10: нет решения (или токена ПЕРЕНОСИТСЯ/НЕ ЗАТРАГИВАЕТСЯ/УДАЛЯЕТСЯ) для:$MISSING"
fi

# ─────────── A-037 D-1 — библиотека меняется ТОЛЬКО АДДИТИВНО ───────────
# Две команды решения, вынесенные в гейт: (а) корпус библиотеки не тронут, (б) он зелен.
# Это и есть проверяемая граница с A-033: не-аддитивный сдвиг библиотеки краснит
# существующий sacred-тест, который dev править не вправе.
MB87=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
if [ -z "$MB87" ]; then
  fail "D-1(а): merge-base не вычислен — аддитивность не предъявлена"
else
  NONADD=$(git diff --name-status "$MB87"..HEAD -- crates/gateway/tests/ \
           | grep -vE '^A[[:space:]]+crates/gateway/tests/red_m87_' || true)
  if [ -z "$NONADD" ]; then
    pass "D-1(а): библиотечный корпус тронут только добавлениями red_m87_*"
  else
    fail "D-1(а): не-аддитивные правки библиотечного корпуса:"
    printf '%s\n' "$NONADD" | head -5
  fi
fi

LIBOUT=$(cargo test -p gateway 2>&1)
LIBSUM=$(printf '%s\n' "$LIBOUT" | grep -E '^test result' | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f}')
if printf '%s\n' "$LIBOUT" | grep -qE '^test result: FAILED'; then
  fail "D-1(б): библиотечный корпус КРАСЕН — $LIBSUM"
else
  pass "D-1(б): библиотечный корпус зелен — $LIBSUM"
fi

# ─────────────── паритет с CI ───────────────
if cargo fmt --all -- --check >/dev/null 2>&1; then
  pass "CI-паритет: cargo fmt --all -- --check"
else
  fail "CI-паритет: cargo fmt --all -- --check"
fi

if cargo clippy --all-targets --all-features -- -D warnings >/dev/null 2>&1; then
  pass "CI-паритет: cargo clippy --all-targets --all-features -- -D warnings"
else
  fail "CI-паритет: cargo clippy --all-targets --all-features -- -D warnings"
fi

if cargo test --all >/dev/null 2>&1; then
  pass "CI-паритет: cargo test --all"
else
  fail "CI-паритет: cargo test --all"
fi

printf '\n'
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
fi
echo "VERDICT: FAIL (провалов: $FAIL)"
exit 1
