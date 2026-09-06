#!/usr/bin/env bash
# Acceptance-гейт M-77 — «кадр строится БЕЗ книги: VB-I-2 нарушен на прод-пути pump».
# Спека: milestones/M-77-frame-book-continuity.md. Зона: architect (sacred).
#
# РЕШЕНИЕ ПРИНИМАЕТСЯ ПО КОДУ ВОЗВРАТА, а не по тексту вывода (`gates.md` §3):
# ни одного `cmd && echo PASS || echo FAIL` — каждый шаг пишет результат в счётчик.
#
# ФАЗЫ. Милестоун RED-first: предметные оракулы красны ПО ПОСТРОЕНИЮ, пока задача 3 не
# сделана. Гейт это ОБЪЯВЛЯЕТ строкой `RED-ФАЗА` и возвращает FAIL — он не имеет права
# печатать PASS над нереализованным предметом (`A-031` §маршрут; предписание арбитра по
# `M-72`: «пока база красна, шаг обязан говорить это явно, а не печатать PASS»).
# Шаг T5 при этом требует, чтобы красными были РОВНО предметные тесты и НИЧЕГО больше:
# так «ожидаемая краснота» остаётся проверяемой, а не превращается в амнистию.
#
# ПОКРЫТИЕ ЗАДАЧ (условие iii вердикта `C-211`: «не менее одной механической проверки на
# задачу»; прежняя редакция закрывала задачи 2 и 4 одним общим статусом суиты и была
# ПРИЗНАНА ЗЕЛЕНЕЕ ПРЕДМЕТА):
#   задача 1 → T0, T1, T2
#   задача 2 → T7 (исполняет оракул окна доставки; плюс presence §6bis с названным пределом)
#   задача 3 → T4/T5
#   задача 4 → T8 (исполняет сторож цены на границе `pump`)
#   задача 5 → T3 (паритет с базовым CI-job'ом)

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

FAILED=0
pass() { printf 'PASS  %s\n' "$*"; }
fail() { printf 'FAIL  %s\n' "$*"; FAILED=$((FAILED + 1)); }
info() { printf 'INFO  %s\n' "$*"; }

SPEC=milestones/M-77-frame-book-continuity.md
ORACLE=crates/gateway/tests/red_m77_frame_book_continuity.rs
WINDOW=crates/gateway/tests/red_m77_delivery_window.rs
COST=crates/gateway/tests/red_m77_pump_cost.rs

CONTROL=vb_i_2_c_client_equals_replay_when_the_tail_carries_snapshots
WINDOW_GUARD=vb_i_2_w1_refusal_by_cap_is_reachable_and_signals_terminality
COST_GUARD=vb_i_10_pump_cost_does_not_grow_with_the_number_of_batches

# Тесты, красные в RED-фазе и обязанные позеленеть после развязки. Ровно эти и никакие
# другие: список — предмет проверки T5.
SUBJECT_TESTS=(
  vb_i_2_client_depth_values_equal_replay_in_prod_steady_state
  vb_i_2_client_keeps_the_point_when_the_tail_delta_is_one_sided
  vb_i_2_client_equals_replay_across_a_resync_then_delta_tail
  vb_i_2_client_bundle_equals_replay_in_prod_steady_state
  vb_i_2_client_equals_replay_when_the_tick_spans_a_batch_rollover
  vb_i_2_w2_client_equals_replay_after_refusals_are_retried
  vb_i_2_w3_client_equals_replay_when_refusal_hits_a_batch_rollover
)

echo "=== M-77 acceptance ==="

# ─── T0 (задача 1): наборы СУЩЕСТВУЮТ и несут объявленный состав ───────────────────
T0_BAD=0
for f in "$ORACLE" "$WINDOW" "$COST"; do
  [ -f "$f" ] || { fail "T0 набора НЕТ: $f — судить нечего"; T0_BAD=1; }
done
[ "$T0_BAD" -eq 0 ] && pass "T0 все три набора на месте"

MISSING=0
for t in "$CONTROL" "$WINDOW_GUARD" "$COST_GUARD" "${SUBJECT_TESTS[@]}"; do
  grep -qh "fn ${t}(" "$ORACLE" "$WINDOW" "$COST" 2>/dev/null \
    || { fail "T0 тест '${t}' в наборах ОТСУТСТВУЕТ"; MISSING=1; }
done
[ "$MISSING" -eq 0 ] && pass "T0 состав полон: 2 контроля + 1 сторож цены + ${#SUBJECT_TESTS[@]} предметных"

# ─── T1 (задача 1): мера снимается на ПРОД-ПУТИ и в ПРОД-ФОРМЕ ─────────────────────
# Страж против ослабления, названного запретным списком спеки §6. Проверяется по ВЫЗОВУ в
# тексте оракула — здесь это законно: предмет проверки и есть текст спецификации.
T1_BAD=0
for f in "$ORACLE" "$WINDOW"; do
  grep -q '\.pump(' "$f" && grep -q 'LiveReducer::resume' "$f" \
    || { fail "T1 $f НЕ исполняет resume+pump — судит offline-путь, предмет M-77 не покрыт"; T1_BAD=1; }
  grep -q 'window_ms: Some(60_000)' "$f" && grep -q 'depth_cadence_ms: Some(1_000)' "$f" \
    || { fail "T1 $f снял прод-форму селектора — под снятым ограничением судится ДРУГОЙ предмет (Р-2)"; T1_BAD=1; }
done
[ "$T1_BAD" -eq 0 ] && pass "T1 оба набора исполняют resume+pump в ПРОД-ФОРМЕ (Р-2)"

# ─── T2 (задача 1): КОНТРОЛИ обязаны быть ЗЕЛЕНЫ ВСЕГДА ────────────────────────────
# Они зелены и до реализации, и после. Красный контроль означает, что сравнение негодно
# само по себе (или что опасное окно недостижимо), и краснота предметных ничего не доказывает.
CTRL_LOG=$(mktemp)
cargo test -p gateway --test red_m77_frame_book_continuity "$CONTROL" -- --exact >"$CTRL_LOG" 2>&1
CTRL_RC=$?
if [ "$CTRL_RC" -eq 0 ]; then
  pass "T2 контроль снимочного хвоста ЗЕЛЁН (exit=0)"
else
  fail "T2 контроль снимочного хвоста КРАСЕН (exit=$CTRL_RC) — сравнение негодно"
  tail -20 "$CTRL_LOG"
fi
rm -f "$CTRL_LOG"

WG_LOG=$(mktemp)
cargo test -p gateway --test red_m77_delivery_window "$WINDOW_GUARD" -- --exact >"$WG_LOG" 2>&1
WG_RC=$?
if [ "$WG_RC" -eq 0 ]; then
  pass "T2 дискриминатор окна отказа ЗЕЛЁН (exit=0) — окно достижимо"
else
  fail "T2 дискриминатор окна отказа КРАСЕН (exit=$WG_RC) — W2/W3 судили бы вакуум"
  tail -20 "$WG_LOG"
fi
rm -f "$WG_LOG"

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

# ─── T4/T5 (задача 3): состояние реализации — по КОДУ ВОЗВРАТА суиты ───────────────
SUITE_LOG=$(mktemp)
cargo test --all --no-fail-fast >"$SUITE_LOG" 2>&1; SUITE_RC=$?
mapfile -t RED_TESTS < <(awk '/^failures:$/{f=1;next} /^$/{f=0} f && /^    [a-zA-Z0-9_:]+$/{gsub(/ /,"");print}' "$SUITE_LOG" | sort -u)

if [ "$SUITE_RC" -eq 0 ]; then
  pass "T4 cargo test --all ЗЕЛЁН (exit=0) — развязка внесена (задачи 2-3 исполнены)"
  M77_LOG=$(mktemp)
  cargo test -p gateway --test red_m77_frame_book_continuity >"$M77_LOG" 2>&1; M77_RC=$?
  cargo test -p gateway --test red_m77_delivery_window >>"$M77_LOG" 2>&1; W_RC=$?
  if [ "$M77_RC" -eq 0 ] && [ "$W_RC" -eq 0 ]; then
    pass "T5 оба набора M-77 целиком ЗЕЛЕНЫ — VB-I-2 держится на прод-пути и в окне отказа"
  else
    fail "T5 суита зелена, а наборы M-77 КРАСНЫ (continuity=$M77_RC window=$W_RC) — невозможное состояние"
    tail -20 "$M77_LOG"
  fi
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
  fail "T4 задача 3 не исполнена — милестоун не закрыт (RED-фаза, см. INFO выше)"
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

# ─── T7 (ЗАДАЧА 2): контракт развязки Б предъявлен ИСПОЛНЕНИЕМ ─────────────────────
# `C-211`: «task 2 должен буквально закрепить signature/shape и source rule для каждой
# series, включая связь depth-delta с delivery cursor», и verify обязан это проверять
# ОТДЕЛЬНО. Проверка — ИСПОЛНЕНИЕМ оракула, который правило пиннит: набор окна доставки
# судит ровно расхождение `full_applied_seq` и `cursor`.
#
# ПРЕДЕЛ НАЗВАН, А НЕ ИЗОБРАЖЁН (`A-031` §1 п.1): текстовая половина шага — присутствие
# раздела §6bis в спеке — есть СТРАЖ ПРИСУТСТВИЯ, и он не отличает написанный контракт от
# заголовка без содержания. Он оставлен потому, что кадр без committed-текста контракта
# нечем судить следующему кругу, и он ПОДКРЕПЛЁН исполнением оракула рядом; сам по себе
# доказательством не служит.
if [ -f "$SPEC" ] && grep -q '^## 6bis\.' "$SPEC" \
   && grep -q 'fn book_series_in' "$SPEC" && grep -q 'fn set_book_series' "$SPEC"; then
  pass "T7 контракт развязки Б объявлен в спеке (§6bis, сигнатура названа) — присутствие"
else
  fail "T7 §6bis (контракт развязки Б с сигнатурой) в $SPEC ОТСУТСТВУЕТ — задача 2 не закрыта"
fi

W_LOG=$(mktemp)
cargo test -p gateway --test red_m77_delivery_window >"$W_LOG" 2>&1; W_ONLY_RC=$?
W_RED=$(grep -cE '^test vb_i_2_w[23]_.* FAILED$' "$W_LOG")
if [ "$SUITE_RC" -eq 0 ]; then
  [ "$W_ONLY_RC" -eq 0 ] \
    && pass "T7 оракул окна доставки ЗЕЛЁН (exit=0) — контракт диапазона держится под отказом" \
    || { fail "T7 оракул окна доставки КРАСЕН (exit=$W_ONLY_RC) при зелёной суите"; tail -20 "$W_LOG"; }
else
  [ "$W_RED" -eq 2 ] \
    && pass "T7 RED-фаза: оба предметных теста окна доставки красны, дискриминатор зелён" \
    || { fail "T7 RED-фаза: предметных красных в окне доставки $W_RED, ожидалось 2 — оракул не пиннит контракт"; tail -20 "$W_LOG"; }
fi
rm -f "$W_LOG"

# ─── T8 (ЗАДАЧА 4): цена развязки Б на границе `pump` ──────────────────────────────
# `C-211`: «task 4 должен иметь проверку цены Б на границе `pump` (не только `snapshot()`),
# а verify обязан запускать и требовать этот оракул». Сторож зелен в ОБЕИХ фазах: цена
# сегодня не платится, и её появление обязано краснить гейт немедленно, а не после merge.
COST_LOG=$(mktemp)
cargo test -p gateway --test red_m77_pump_cost >"$COST_LOG" 2>&1; COST_RC=$?
if [ "$COST_RC" -eq 0 ]; then
  pass "T8 сторож цены на границе pump ЗЕЛЁН (exit=0) — работа тика не растёт с числом батчей"
else
  fail "T8 сторож цены на границе pump КРАСЕН (exit=$COST_RC) — VB-I-10 ослаблен развязкой"
  grep -m1 'VB-I-10 НАРУШЕН' "$COST_LOG" || tail -20 "$COST_LOG"
fi
rm -f "$COST_LOG"

echo "---"
if [ "$FAILED" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAILED)"
  exit 1
fi
