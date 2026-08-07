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

# ── ЧЕМ ЭТОТ ГЕЙТ КРАСНЕЕТ ПРОТИВ ВОЗВРАТА ДЕФЕКТА (круг 3, вердикт `R-039` F-1/F-2) ───
# Прежняя редакция не исполняла прод-форменные оракулы круга 2 ВООБЩЕ
# (`grep -c red_tail_cursor_prod_form scripts/verify_M-57.sh` → 0), и была зеленее CI:
# CI гоняет `cargo test --all`, гейт — четыре именованных таргета крейта `gateway`.
# Замер `R-039` §C.2: композит из двух строк (откат `stream_from_at` → `stream_from` плюс
# подмена проброса `events_scanned` → `events_decoded`) возвращал P0-дефект ЦЕЛИКОМ и
# оставлял гейт зелёным 11/11. Теперь против этого стоят три зуба, и каждый проверен
# мутацией, а не рассуждением:
#   T3   O-1..O-4        — работа тика на уровне журнала;
#   T3b  f035_1/f035_2   — прод-форма (RO-каталог, две сессии) + f035_3 (проброс измерителя);
#   T5d  `cargo test --all` — паритет с CI (`gates.md` §3: гейт, который зеленее CI, — не гейт).
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
# `--all-features` — как в CI (`ci.yml`: clippy --all-targets --all-features -D warnings).
# Без него гейт собирал не ту конфигурацию, что CI, и был зеленее его.
if cargo clippy --workspace --all-targets --all-features -- -D warnings >/tmp/m57-clippy.log 2>&1; then
  pass "T2 clippy --workspace --all-targets --all-features -D warnings"
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

echo "--- T3b: ПРОД-ФОРМА круга 2 + проброс измерителя (R-039 F-1/F-2) ---"
# Этих оракулов гейт не исполнял ВООБЩЕ — отсюда и REJECT круга 2.
#   f035_1/f035_2 — условия прода: каталог `:ro` и ДВЕ сессии над одним каталогом
#                   (замер R-035 §D: 8003 / 8009 событий на тик вместо 3);
#   f035_3        — сам ИЗМЕРИТЕЛЬ: `ReadStats.events_scanned` обязан равняться журнальному
#                   счётчику, а не `events_decoded`. Без него подмена одной строки слепит
#                   разом все оракулы уровня gateway, включая два верхних (R-039 §C.2).
cargo test -p gateway --test red_tail_cursor_prod_form --test red_read_stats_passthrough \
  >/tmp/m57-pf.log 2>&1; PF=$?
# Число зелёных блоков СЧИТАЕТСЯ: `test result: ok. 0 passed` — зелёная строка, не
# исполнившая ничего. Таргетов ровно два, в каждом обязан пройти минимум один тест.
PF_OK=$(grep -cE "^test result: ok\. [1-9]" /tmp/m57-pf.log)
if [ "$PF" -eq 0 ] && [ "$PF_OK" -eq 2 ]; then
  pass "T3b f035_1/f035_2/f035_3 GREEN (прод-форма + проброс измерителя), блоков ok: ${PF_OK}/2"
else
  fail "T3b прод-форма/проброс: exit=${PF}, зелёных блоков ${PF_OK}/2 — механизм не работает в условиях прода, измеритель подменён либо оракул не исполнился"
  grep -E "F-035|panicked|test result|SETUP" /tmp/m57-pf.log | head -10
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

echo "--- T5d: ПАРИТЕТ С CI — cargo test --all (gates.md §3) ---"
# «Гейт, который зеленее CI, — не гейт». CI гоняет `cargo test --all`; прежняя редакция
# ограничивалась четырьмя именованными таргетами gateway, поэтому новый оракул мог быть
# написан и не исполнен ни разу. Именованные шаги выше сохранены НАМЕРЕННО: они дают
# атрибуцию (какой инвариант сломан), `--all` даёт полноту.
if cargo test --all >/tmp/m57-all.log 2>&1; then
  pass "T5d cargo test --all: $(grep -E '^test result' /tmp/m57-all.log | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}')"
else
  fail "T5d cargo test --all КРАСНЫЙ (паритет с CI)"
  grep -E "^test .* FAILED|^error" /tmp/m57-all.log | head -8
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
