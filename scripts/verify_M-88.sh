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

# ─────────────── задача 2 — источник ОБЪЯВЛЯЕТ полный срез (утвердительно) ───────────────
# C-233 R2: прежняя проверка отвергала ОДНУ фразу — удаление комментария её проходило.
# Теперь требуется НАЛИЧИЕ новой семантики И отсутствие старой: ослабить можно только
# сломав обе половины сразу.
OBS_DOC=$(awk '/struct BookSeriesObservation/,/^}/' "$GW")
if printf '%s' "$OBS_DOC" | grep -qE 'ПОЛНЫЙ срез|полный срез бакета' \
   && ! printf '%s' "$OBS_DOC" | grep -q 'замещает раннее'; then
  pass "task2: источник объявляет наблюдение ПОЛНЫМ СРЕЗОМ и не предписывает объединение"
else
  fail "task2: док-комментарий источника не объявляет полный срез бакета (или всё ещё предписывает объединение)"
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
# C-233 R2: прежняя проверка искала #[must_use] ГДЕ УГОДНО в файле — посторонняя
# аннотация её удовлетворяла. Теперь атрибут обязан стоять в трёх строках перед сигнатурой.
APPLY_LINE=$(grep -nE 'pub fn apply\(&mut self, frame: &Frame\) -> ApplyOutcome' "$GW" | head -1 | cut -d: -f1)
if [ -n "$APPLY_LINE" ] && [ "$APPLY_LINE" -gt 3 ] \
   && sed -n "$((APPLY_LINE - 3)),$((APPLY_LINE - 1))p" "$GW" | grep -qE '#\[must_use\]'; then
  pass "task7: apply возвращает ApplyOutcome и помечен must_use НЕПОСРЕДСТВЕННО перед сигнатурой"
else
  fail "task7: apply не возвращает ApplyOutcome либо #[must_use] не стоит рядом с сигнатурой"
fi

# ─────────────── задача 8 — ЗАМЕР размера кадра, а не обещание ───────────────
# C-233 R2: прежняя проверка принимала ЛЮБОЙ файл с тремя цифрами. Теперь требуется
# датированная фактура (маркер FACTS), раздел результата ПОСЛЕ реализации и числа до/после.
MEAS=docs/plans/m88-frame-size-measurement.md
if [ -f "$MEAS" ] \
   && head -5 "$MEAS" | grep -qE '<!-- FACTS: audited_head=[0-9a-f]{40}' \
   && grep -qE '^## Результат ПОСЛЕ реализации' "$MEAS" \
   && grep -qE 'до:.*[0-9]{3,}' "$MEAS" && grep -qE 'после:.*[0-9]{3,}' "$MEAS"; then
  pass "task8: замер до/после на прод-форме предъявлен ($MEAS)"
else
  fail "task8: в $MEAS нет датированной фактуры с разделом '## Результат ПОСЛЕ реализации' и числами до/после"
fi

# ─────────────── задача 9 — ревизия оракулов, поимённо ───────────────
# C-233 R2: прежняя проверка проходила ПО НАЛИЧИЮ ЗАГОЛОВКА и печатала PASS при
# незавершённой задаче — базовая линия это показала. Теперь плейсхолдер отвергается.
if grep -q '^## 14. Ревизия оракулов' "$SPEC" \
   && grep -q '^### 14.1. Решения по изменённым ожиданиям' "$SPEC" \
   && ! grep -q 'на момент коммита набора изменённых ожиданий нет' "$SPEC"; then
  pass "task9: таблица решений по изменённым ожиданиям заполнена"
else
  fail "task9: таблица §14.1 пуста или содержит плейсхолдер — решение по каждому ожиданию не записано"
fi

# ─────────────── R4 (C-233) — ДВЕ изолированные мутации предъявлены ───────────────
# ВТОРОЙ РЕЦИДИВ КЛАССА, пойманный собственной базовой линией: первая редакция этого шага
# искала слово FAILED ГДЕ УГОДНО в спеке и печатала PASS при ПУСТОЙ таблице. Тот же дефект,
# что C-233 R2 нашёл в task9. Теперь плейсхолдер отвергается явно, и проверяются ОБА мутанта.
MUT_TABLE=$(awk '/^### 9.4. Результат мутационного контроля/,/^### 9.3/' "$SPEC")
if printf '%s' "$MUT_TABLE" | grep -q 'merge_heatmap' \
   && printf '%s' "$MUT_TABLE" | grep -q 'пустой полный срез' \
   && printf '%s' "$MUT_TABLE" | grep -cE 'FAILED' | grep -qE '^[2-9]' \
   && ! printf '%s' "$MUT_TABLE" | grep -q 'заполняется'; then
  pass "R4: результат ОБЕИХ изолированных мутаций записан (плейсхолдеров нет)"
else
  fail "R4: таблица §9.4 не заполнена — нужны ДВА результата вида «нейтрализация X → тест Y FAILED», без плейсхолдеров"
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
