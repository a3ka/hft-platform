#!/usr/bin/env bash
# Acceptance-гейт M-57 — TD-109: активный сегмент не пересканируется, счётчик работы честен.
#
# Замер, из-за которого milestone существует (один сегмент, приращение ровно 3 события):
#     2 000 событий -> тик   3 488 мкс, выдано 3
#    16 000 событий -> тик  44 535 мкс, выдано 3
#   128 000 событий -> тик 200 247 мкс, выдано 3
# Сегмент вырос в 64 раза — тик в 57 раз. events_decoded во всех случаях = 3: прежний счётчик
# слеп к пересканированию, потому что считает ВЫДАННЫЕ события, а не прочитанные.
# Прод: активный сегмент до 1 GiB (~10.7 млн событий) => ~17 секунд на тик при периоде 250 мс.
set -uo pipefail

FAILS=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILS=$((FAILS + 1)); }

# ── ОЖИДАЕМОЕ СОСТОЯНИЕ ДО РЕАЛИЗАЦИИ (читать dev'у перед стартом) ────────────────────
# Оракул `red_tail_scan_bounded.rs` — COMPILE-RED: он зовёт `EventStream::events_scanned()`,
# которого ещё нет. Пока метод не добавлен, НЕ КОМПИЛИРУЕТСЯ весь тест-таргет крейта
# `journal`, поэтому КРАСНЕЮТ ЗАОДНО:
#   T2  clippy --all-targets   (собирает тесты)
#   T3  сам оракул             (по существу — это и есть предмет)
#   T4  events_scanned не найден (по существу)
#   T5  «регресс в journal» и «регресс в M-53/M-54/M-56» — ЛОЖНЫЕ: прежние тесты целы,
#       падает сборка таргета из-за отсутствующего метода.
# Итого ожидаемо `VERDICT: FAIL (5)`, из них ПО СУЩЕСТВУ — T3 и T4.
# После задачи 1 (добавлен счётчик) картина обязана стать: T2/T5 зелёные, T3 КРАСНЫЙ по делу
# (O-2 ловит пересканирование), и только задача 2 (byte-offset) делает T3 зелёным.
# ──────────────────────────────────────────────────────────────────────────────────────

echo "--- T0: оракул на месте (sacred, architect-only) ---"
if [ -f crates/journal/tests/red_tail_scan_bounded.rs ]; then
  pass "T0 crates/journal/tests/red_tail_scan_bounded.rs"
else
  fail "T0 ОТСУТСТВУЕТ оракул M-57"
fi

echo "--- T1/T2/T2b: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
if cargo build --workspace >/tmp/m57-build.log 2>&1; then pass "T1 build --workspace"
else fail "T1 build"; tail -20 /tmp/m57-build.log; fi
if cargo clippy --workspace --all-targets -- -D warnings >/tmp/m57-clippy.log 2>&1; then
  pass "T2 clippy --workspace --all-targets -D warnings"
else fail "T2 clippy"; tail -20 /tmp/m57-clippy.log; fi
if cargo fmt --all -- --check >/tmp/m57-fmt.log 2>&1; then pass "T2b fmt --check"
else fail "T2b fmt"; grep -E "^Diff in" /tmp/m57-fmt.log | sed 's|.*/crates/|crates/|' | sort -u; fi

echo "--- T3: ГЛАВНОЕ — работа тика не растёт с размером активного сегмента ---"
if cargo test -p journal --test red_tail_scan_bounded >/tmp/m57-o.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m57-o.log; then
  pass "T3 O-1..O-4 GREEN"
else
  fail "T3 TD-109 НЕ УСТРАНЁН — активный сегмент по-прежнему читается с начала"
  grep -E "O-1|O-2|O-3|O-4|panicked|ЗАМЕР|test result" /tmp/m57-o.log | head -10
fi

echo "--- T4: честный счётчик существует и подключён ---"
# Слепота прежнего измерителя — корень дефекта: events_decoded считает ВЫДАННЫЕ события,
# поэтому оракулы M-53 показывали «работа = 3» и при полном скане гигабайта.
if grep -qE "events_scanned" crates/journal/src/segments.rs; then
  pass "T4 events_scanned есть в segments.rs"
else
  fail "T4 events_scanned не найден — задача 1 не сделана, мерить пересканирование нечем"
fi
if grep -qE "events_decoded" crates/journal/src/segments.rs; then
  pass "T4 events_decoded СОХРАНЁН (на нём стоят прежние оракулы)"
else
  fail "T4 events_decoded ИСЧЕЗ — сломаны red_stream_from и red_checkpoint_resource_bound"
fi

echo "--- T5: РЕГРЕСС — прежние наборы остаются зелёными ---"
if cargo test -p journal >/tmp/m57-j.log 2>&1 \
   && ! grep -qE "^test result: FAILED" /tmp/m57-j.log; then
  pass "T5 journal GREEN (счётчики и чекпоинт-ресурс целы)"
else
  fail "T5 РЕГРЕСС в journal"; grep -E "panicked|FAILED|test result" /tmp/m57-j.log | head -8
fi
if cargo test -p gateway --test red_frames_seek_bound --test red_push_seek_bounded \
     --test red_connect_cost_single --test red_snapshot_noclone >/tmp/m57-g.log 2>&1 \
   && ! grep -qE "^test result: FAILED" /tmp/m57-g.log; then
  pass "T5 M-53/M-54/M-56 GREEN"
else
  fail "T5 РЕГРЕСС в M-53/M-54/M-56"; grep -E "panicked|FAILED|test result" /tmp/m57-g.log | head -8
fi
if cargo test -p gateway-serve >/tmp/m57-s.log 2>&1 \
   && ! grep -qE "^test result: FAILED" /tmp/m57-s.log; then
  pass "T5 gateway-serve GREEN (сверка WS↔реплей цела)"
else
  fail "T5 РЕГРЕСС в M-46"; grep -E "panicked|FAILED|test result" /tmp/m57-s.log | head -8
fi

echo "--- T6: контракты не тронуты ---"
if git diff --name-only origin/main...HEAD 2>/dev/null | grep -q "^crates/contracts/"; then
  fail "T6 crates/contracts/** тронут — M-57 не является contract-изменением"
else
  pass "T6 crates/contracts/** не тронут"
fi

echo
if [ "$FAILS" -eq 0 ]; then echo "VERDICT: PASS"; exit 0
else echo "VERDICT: FAIL ($FAILS нарушений)"; exit 1; fi
