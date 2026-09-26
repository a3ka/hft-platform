#!/usr/bin/env bash
# verify_M-88.sh — acceptance-гейт milestone'а M-88 «контракт обновления книго-зависимых серий».
# Спека: milestones/M-88-liquidity-removal-contract.md
#
# Решение принимается по КОДУ ВОЗВРАТА (`gates.md` §3). Форма — агрегатор с FAIL-счётчиком:
# `set -e` здесь не годится, потому что гейт обязан ПЕРЕЧИСЛИТЬ все провалы, а не умереть
# на первом; `exit 1` при FAIL>0 даёт ту же строгость.
#
# Базовая линия снимается КРАСНОЙ до работы dev'а и предъявляется в Handoff: гейт, ни разу
# не покрасневший, ничего не доказывает.

set -uo pipefail

cd "$(dirname "$0")/.."

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP  %s\n' "$1"; }

# shellcheck source=lib/m88_predicates.sh
. "$(dirname "$0")/lib/m88_predicates.sh"

GW=crates/gateway/src/lib.rs
SPEC=milestones/M-88-liquidity-removal-contract.md

# ─────────────────── задача 1 — форма провода и бамп версии ───────────────────
if grep -qE '^\s*pub heatmap_observed_time_s: Vec<i64>,' "$GW" \
   && grep -qE '^\s*pub cob_observed: bool,' "$GW"; then
  pass "task1: поля контракта объявлены в SeriesBundle"
else
  fail "task1: нет полей heatmap_observed_time_s / cob_observed в $GW"
fi

if grep -qE '^pub const GATEWAY_SCHEMA_VERSION: u32 = 11;' "$GW"; then
  pass "task1: GATEWAY_SCHEMA_VERSION поднят до 11"
else
  fail "task1: GATEWAY_SCHEMA_VERSION не равен 11 (смена формы ⇒ бамп обязателен, VB-I-4)"
fi

# ─────────────── задача 2 — семантика привязана К ПОЛЮ, а не к структуре ───────────────
# Предикат вынесен в scripts/lib/m88_predicates.sh и покрыт отрицательной пробой
# scripts/tests/red_verify_M-88.sh (A-036 §5.3).
if m88_task2 "$GW"; then
  pass "task2: док-комментарий ПОЛЯ heatmap_cells объявляет полный срез и не предписывает объединение"
else
  fail "task2: комментарий ПЕРЕД heatmap_cells не объявляет полный срез бакета либо предписывает объединение"
fi

# ─────────────── задачи 3-6 — производство и склейка (предметный набор) ───────────────
# Оракул, а не греп: семь сценариев идут через формирование → сериализацию → применение.
UP_OUT=$(cargo test -p gateway --test red_m88_update_contract 2>&1)
UP_RC=$?
UP_LINE=$(printf '%s\n' "$UP_OUT" | grep -E '^test result' | tail -1)
if [ $UP_RC -eq 0 ]; then
  pass "task3-6: red_m88_update_contract — ${UP_LINE:-GREEN}"
else
  fail "task3-6: red_m88_update_contract КРАСЕН — ${UP_LINE:-компиляция}"
  printf '%s\n' "$UP_OUT" | grep -E '^(thread |assertion|---- )' | head -30
fi

# ─────────────── задачи 1+7 — форма и явный исход применения ───────────────
FM_OUT=$(cargo test -p gateway --test red_m88_contract_form 2>&1)
FM_RC=$?
FM_LINE=$(printf '%s\n' "$FM_OUT" | grep -E '^test result' | tail -1)
if [ $FM_RC -eq 0 ]; then
  pass "task1+7: red_m88_contract_form — ${FM_LINE:-GREEN}"
else
  fail "task1+7: red_m88_contract_form КРАСЕН — ${FM_LINE:-компиляция}"
  printf '%s\n' "$FM_OUT" | grep -E '^(error|thread|assertion)' | head -20
fi

# ─────────────── задача 7 — молчаливый no-op запрещён структурно ───────────────
# Предикат вынесен в библиотеку и покрыт отрицательной пробой. Остаток назван и принят:
# блочный комментарий и `cfg` предикат обойдут — существо сторожат поведенческие оракулы
# form_apply_reports_outcome, S6 и S7.
if m88_task7 "$GW"; then
  pass "task7: apply возвращает ApplyOutcome и помечен must_use НЕПОСРЕДСТВЕННО перед сигнатурой"
else
  fail "task7: apply не возвращает ApplyOutcome либо #[must_use] не стоит некомментарной строкой рядом с сигнатурой"
fi

# ─────────────── задача 8 — ЗАМЕР размера кадра, а не обещание ───────────────
# Предикат вынесен в библиотеку и покрыт отрицательной пробой.
# ПРЕДЕЛ НАЗВАН: истинность замера гейт не проверяет — её проверяет tester повтором команды
# по дословному мандату §12.1 спеки. Требовать от текстового гейта доказательства прогона —
# требовать невозможного (A-036 §5.3).
MEAS=docs/plans/m88-frame-size-measurement.md
if m88_task8 "$MEAS"; then
  pass "task8: результат до/после датирован существующей ревизией и снят названной командой"
else
  fail "task8: в $MEAS нет раздела '## Результат ПОСЛЕ реализации' с собственным маркером FACTS (существующая ревизия ≠ базы), строкой wsprobe и числами до/после"
fi

# ─────────────── задача 9 — ревизия оракулов, поимённо ───────────────
# Предикат вынесен в библиотеку и покрыт отрицательной пробой. Он верен В ОБЕ СТОРОНЫ:
# каждый изменённый файл обязан быть назван, а ПУСТОЕ множество — заявлено явно.
MB=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
if [ -z "$MB" ]; then
  fail "task9: merge-base с origin/main не вычислен — состав изменённых ожиданий не установлен"
elif MISS=$(m88_task9 "$SPEC" "$MB"); then
  pass "task9: решение записано по каждому изменённому ожиданию (дифф от $MB)"
else
  fail "task9: в §14.1 нет решения по изменённым файлам:$MISS"
fi

# ─────────────── R4 (C-233) — ДВЕ изолированные мутации предъявлены ───────────────
# A-036 §5.3: текстовый предикат ЗАМЕНЁН исполняемой батареей — прецедент `verify_M-65.sh`
# F2. Батарея применяет ДВЕ изолированные мутации на одноразовой копии дерева и требует по
# каждой красного `red_m88_update_contract` с НАЗВАННЫМ упавшим тестом. Пока файла нет —
# шаг КРАСЕН (fail-closed): батарею физически нельзя написать до GREEN dev'а.
# Таблица §9.4 остаётся ЗАПИСЬЮ вывода для человека и предикатом БОЛЬШЕ НЕ ЯВЛЯЕТСЯ.
MUT_BATTERY=scripts/tests/red_m88_mutants.sh
if [ -f "$MUT_BATTERY" ]; then
  if bash "$MUT_BATTERY" --battery >/dev/null 2>&1; then
    pass "R4: батарея мутантов ЗЕЛЕНА — все изолированные мутации красят набор"
  else
    fail "R4: батарея мутантов КРАСНАЯ — мутация не роняет набор, оракулы ничего не пиннят"
  fi
else
  fail "R4: батареи $MUT_BATTERY НЕТ — анти-плацебо не предъявлено (пишется architect'ом ПОСЛЕ GREEN dev'а, §12 шаг 2)"
fi

# ─────────── A-036 §4.3 п.1 — МАТЕРИАЛ внешней проверки фронта ───────────
# Отсрочка оракула реального клиента законна только при наличии эталонных векторов.
# Оракул сверяет их ДВУСТОРОННЕ: производитель воспроизводит байты, модель сходится с
# ожиданием. Отсутствие фикстур — ОТКАЗ, не пропуск (порождаются architect'ом после GREEN).
GV_OUT=$(cargo test -p gateway --test red_m88_golden_vectors 2>&1)
GV_RC=$?
GV_LINE=$(printf '%s\n' "$GV_OUT" | grep -E '^test result' | tail -1)
if [ $GV_RC -eq 0 ]; then
  pass "A-036 п.1: эталонные векторы предъявлены и сходятся — ${GV_LINE:-GREEN}"
else
  fail "A-036 п.1: эталонные векторы не предъявлены или разошлись — ${GV_LINE:-компиляция}"
fi

# ─────────────── ОТРИЦАТЕЛЬНАЯ ПРОБА предикатов (A-036 §5.3) ───────────────
# Гейт, чьи предикаты не покрыты пробой, обходится молча — это доказано трижды. Проба
# двусторонняя: честная фикстура обязана дать PASS, подделанная — FAIL.
if bash scripts/tests/red_verify_M-88.sh >/dev/null 2>&1; then
  pass "проба предикатов: все сценарии сошлись (scripts/tests/red_verify_M-88.sh)"
else
  fail "проба предикатов КРАСНАЯ — текстовые шаги гейта не отвергают подделку"
  bash scripts/tests/red_verify_M-88.sh 2>&1 | grep -E '^FAIL' | head -5
fi

# ─────────── задача 10 (`R-195` Б-1) — список наблюдений подчинён ОКНУ, не истории ───────────
# Два оракула, и оба нужны: первый судит ФОРМУ (что объявлено вне окна), второй — ЦЕНУ
# (что при этом отказывает выдача целиком). Форма без цены выглядит косметикой, цена без
# формы не говорит, где чинить.
OW_OUT=$(cargo test -p gateway --test red_m88_observed_window 2>&1)
OW_RC=$?
OW_LINE=$(printf '%s\n' "$OW_OUT" | grep -E '^test result' | tail -1)
if [ $OW_RC -eq 0 ]; then
  pass "task10: red_m88_observed_window — ${OW_LINE:-GREEN}"
else
  fail "task10: red_m88_observed_window КРАСЕН — ${OW_LINE:-компиляция}"
  printf '%s\n' "$OW_OUT" | grep -E '^(thread |assertion|VB-I-10|---- )' | head -20
fi

RL_OUT=$(cargo test -p gateway --test red_m88_observed_response_limit 2>&1)
RL_RC=$?
RL_LINE=$(printf '%s\n' "$RL_OUT" | grep -E '^test result' | tail -1)
if [ $RL_RC -eq 0 ]; then
  pass "task10: бюджет ответа не съеден списком наблюдений — ${RL_LINE:-GREEN}"
else
  fail "task10: red_m88_observed_response_limit КРАСЕН — ${RL_LINE:-компиляция}"
  printf '%s\n' "$RL_OUT" | grep -E '^(thread |PL-I-5|R-195)' | head -10
fi

# ─────────── задача 11 (`R-195` Б-2) — текст поля и его атрибут говорят ОДНО ───────────
# Сравниваются ДВА факта в одном месте файла: что утверждает док-комментарий поля и какой
# атрибут стоит на нём фактически. Проверка fail-closed: поле не найдено ⇒ FAIL, а не SKIP —
# исчезнувшее поле означает, что гейт судит несуществующий предмет.
if m88_task11 "$GW"; then
  pass "task11: док-комментарий heatmap_buckets_observed не противоречит своему serde-атрибуту"
else
  fail "task11: текст поля heatmap_buckets_observed утверждает механизм, которого нет (класс TD-138, R-195 Б-2)"
fi

# ─────────────── паритет с CI (`gates.md` §3): гейт не может быть зеленее CI ───────────────
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

ALL_OUT=$(cargo test --all 2>&1)
ALL_RC=$?
if [ $ALL_RC -eq 0 ]; then
  pass "CI-паритет: cargo test --all"
else
  fail "CI-паритет: cargo test --all"
  printf '%s\n' "$ALL_OUT" | grep -E '^(error|test result: FAILED)' | head -20
fi

printf '\n'
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
fi
echo "VERDICT: FAIL (провалов: $FAIL)"
exit 1
