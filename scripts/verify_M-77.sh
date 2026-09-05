#!/usr/bin/env bash
# Acceptance-гейт M-77 — «кадр строится БЕЗ книги: VB-I-2 нарушен на прод-пути pump».
# Спека: milestones/M-77-frame-book-continuity.md. Зона: architect (sacred).
#
# РЕШЕНИЕ ПРИНИМАЕТСЯ ПО КОДУ ВОЗВРАТА, а не по тексту вывода (`gates.md` §3):
# ни одного `cmd && echo PASS || echo FAIL` — каждый шаг пишет результат в счётчик.
#
# ФАЗЫ. Милестоун RED-first: набор задачи 1 красен ПО ПОСТРОЕНИЮ, пока задача 3 не сделана.
# Гейт это ОБЪЯВЛЯЕТ строкой `RED-ФАЗА` и возвращает FAIL — он не имеет права печатать PASS
# над нереализованным предметом (`A-031` §маршрут; предписание арбитра по `M-72`:
# «пока база красна, шаг обязан говорить это явно, а не печатать PASS»).
# Шаг T5 при этом требует, чтобы красными были РОВНО три предметных теста и НИЧЕГО больше:
# так «ожидаемая краснота» остаётся проверяемой, а не превращается в амнистию.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

FAILED=0
pass() { printf 'PASS  %s\n' "$*"; }
fail() { printf 'FAIL  %s\n' "$*"; FAILED=$((FAILED + 1)); }
info() { printf 'INFO  %s\n' "$*"; }

ORACLE=crates/gateway/tests/red_m77_frame_book_continuity.rs
CONTROL=vb_i_2_c_client_equals_replay_when_the_tail_carries_snapshots
SUBJECT_TESTS=(
  vb_i_2_client_depth_values_equal_replay_in_prod_steady_state
  vb_i_2_client_keeps_the_point_when_the_tail_delta_is_one_sided
  vb_i_2_client_equals_replay_across_a_resync_then_delta_tail
)

echo "=== M-77 acceptance ==="

# ─── T0 (задача 1): набор СУЩЕСТВУЕТ и несёт объявленный состав ────────────────────
if [ -f "$ORACLE" ]; then
  pass "T0 RED-набор на месте: $ORACLE"
else
  fail "T0 RED-набора НЕТ: $ORACLE — задача 1 не исполнена, судить нечего"
fi

MISSING=0
for t in "$CONTROL" "${SUBJECT_TESTS[@]}"; do
  grep -q "fn ${t}(" "$ORACLE" 2>/dev/null || { fail "T0 тест '${t}' в наборе ОТСУТСТВУЕТ"; MISSING=1; }
done
[ "$MISSING" -eq 0 ] && pass "T0 состав набора полон: контроль + ${#SUBJECT_TESTS[@]} предметных"

# ─── T1 (задача 1): мера снимается на ПРОД-ПУТИ и в ПРОД-ФОРМЕ ─────────────────────
# Страж против ослабления, названного запретным списком спеки §6: набор обязан ходить
# `pump` (а не только `frames_since`) и держать прод-форму селектора. Проверяется по
# ВЫЗОВУ в тексте оракула — здесь это законно: предмет проверки и есть текст спецификации.
if grep -q '\.pump(' "$ORACLE" && grep -q 'LiveReducer::resume' "$ORACLE"; then
  pass "T1 набор исполняет прод-путь resume+pump"
else
  fail "T1 набор НЕ исполняет resume+pump — судит offline-путь, предмет M-77 не покрыт"
fi
if grep -q 'window_ms: Some(60_000)' "$ORACLE" && grep -q 'depth_cadence_ms: Some(1_000)' "$ORACLE"; then
  pass "T1 селектор в ПРОД-ФОРМЕ (window_ms=60000, depth_cadence_ms=1000) — Р-2"
else
  fail "T1 прод-форма селектора снята — под снятым ограничением судится ДРУГОЙ предмет (Р-2)"
fi

# ─── T2 (задача 1): КОНТРОЛЬ обязан быть ЗЕЛЁН ВСЕГДА ──────────────────────────────
# Он зелен и до реализации, и после. Красный контроль означает, что сравнение негодно
# само по себе, и краснота предметных тестов ничего не доказывает.
CTRL_LOG=$(mktemp)
cargo test -p gateway --test red_m77_frame_book_continuity "$CONTROL" -- --exact >"$CTRL_LOG" 2>&1
CTRL_RC=$?
if [ "$CTRL_RC" -eq 0 ]; then
  pass "T2 контроль '$CONTROL' ЗЕЛЁН (exit=0)"
else
  fail "T2 контроль КРАСЕН (exit=$CTRL_RC) — сравнение негодно, остальное недоказуемо"
  tail -20 "$CTRL_LOG"
fi
rm -f "$CTRL_LOG"

# ─── T3 (задача 5): паритет с базовым CI-job'ом ────────────────────────────────────
FMT_LOG=$(mktemp); cargo fmt --all -- --check >"$FMT_LOG" 2>&1; FMT_RC=$?
[ "$FMT_RC" -eq 0 ] && pass "T3 cargo fmt --all -- --check (exit=0)" \
  || { fail "T3 cargo fmt --check exit=$FMT_RC"; tail -15 "$FMT_LOG"; }
rm -f "$FMT_LOG"

CLIPPY_LOG=$(mktemp)
cargo clippy --all-targets --all-features -- -D warnings >"$CLIPPY_LOG" 2>&1; CLIPPY_RC=$?
[ "$CLIPPY_RC" -eq 0 ] && pass "T3 cargo clippy --all-targets --all-features -D warnings (exit=0)" \
  || { fail "T3 clippy exit=$CLIPPY_RC"; tail -25 "$CLIPPY_LOG"; }
rm -f "$CLIPPY_LOG"

# ─── T4/T5 (задачи 2-4): состояние реализации — по КОДУ ВОЗВРАТА суиты ─────────────
SUITE_LOG=$(mktemp)
cargo test --all --no-fail-fast >"$SUITE_LOG" 2>&1; SUITE_RC=$?
mapfile -t RED_TESTS < <(awk '/^failures:$/{f=1;next} /^$/{f=0} f && /^    [a-zA-Z0-9_:]+$/{gsub(/ /,"");print}' "$SUITE_LOG" | sort -u)

if [ "$SUITE_RC" -eq 0 ]; then
  pass "T4 cargo test --all ЗЕЛЁН (exit=0) — развязка внесена (задачи 2-3 исполнены)"
  # После реализации весь набор M-77 обязан быть зелёным — иначе развязка чинит не то.
  M77_LOG=$(mktemp)
  cargo test -p gateway --test red_m77_frame_book_continuity >"$M77_LOG" 2>&1; M77_RC=$?
  [ "$M77_RC" -eq 0 ] && pass "T5 набор M-77 целиком ЗЕЛЁН (exit=0) — VB-I-2 держится на прод-пути" \
    || { fail "T5 суита зелена, а набор M-77 КРАСЕН (exit=$M77_RC) — невозможное состояние"; tail -20 "$M77_LOG"; }
  rm -f "$M77_LOG"
else
  info "RED-ФАЗА: cargo test --all exit=$SUITE_RC — задача 3 не исполнена, это ОЖИДАЕМО"
  # Ожидаемая краснота обязана быть ЛОКАЛИЗОВАНА: красными имеют право быть РОВНО
  # предметные тесты M-77 и ничего больше. Иначе «ожидаемо» стало бы амнистией всему.
  UNEXPECTED=0
  for t in "${RED_TESTS[@]}"; do
    known=0
    for k in "${SUBJECT_TESTS[@]}"; do [ "$t" = "$k" ] && known=1; done
    [ "$known" -eq 0 ] && { fail "T5 КРАСЕН ПОСТОРОННИЙ тест: $t — краснота НЕ локализована предметом M-77"; UNEXPECTED=1; }
  done
  MISSING_RED=0
  for k in "${SUBJECT_TESTS[@]}"; do
    printf '%s\n' "${RED_TESTS[@]}" | grep -qx "$k" || { fail "T5 предметный тест '$k' НЕ КРАСЕН в RED-фазе — он ничего не пиннит либо не исполняется"; MISSING_RED=1; }
  done
  [ "$UNEXPECTED" -eq 0 ] && [ "$MISSING_RED" -eq 0 ] \
    && pass "T5 краснота локализована: ровно ${#SUBJECT_TESTS[@]} предметных теста M-77, посторонних нет"
  fail "T4 задачи 2-4 не исполнены — милестоун не закрыт (RED-фаза, см. INFO выше)"
fi
rm -f "$SUITE_LOG"

# ─── T6: запретный список спеки §6/§7 не нарушен диапазоном ветки ──────────────────
BASE=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
if [ -n "$BASE" ]; then
  BAD=$(git diff --name-only "$BASE"..HEAD -- crates/contracts crates/gateway-serve/src docker-compose.yml 2>/dev/null)
  if [ -z "$BAD" ]; then
    pass "T6 запретные пути не тронуты (contracts / gateway-serve/src / docker-compose.yml)"
  else
    fail "T6 тронуты ЗАПРЕЩЁННЫЕ пути: $(echo "$BAD" | tr '\n' ' ')"
  fi
else
  fail "T6 база диапазона не определяется — scope не проверен, fail-closed"
fi

echo "---"
if [ "$FAILED" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAILED)"
  exit 1
fi
