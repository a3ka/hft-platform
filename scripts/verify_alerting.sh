#!/usr/bin/env bash
# Acceptance-гейт переделки ветки `feat/alerting` по вердикту PR-гейта R-005
# (`research/reviews/R-005-alerting.md`, ВЕРДИКТ: REJECTED — 7 находок).
#
# Правила гейта — `.claude/rules/gates.md` §3: явный агрегатор с FAIL-счётчиком, exit≠0 при
# FAIL>0, финальная строка VERDICT, минимум одна проверка на находку. Никаких
# `cmd && echo PASS || echo FAIL` (маскирует провал) — каждая проверка идёт через `check`,
# который берёт РЕАЛЬНЫЙ exit-код.
#
# Прогон групп оракулов идёт через `run_oracles <префикс> <сколько_ожидаем>`: фильтр
# `cargo test -- <substring>` сам по себе даёт exit 0, если не нашлось НИ ОДНОГО теста, —
# без сверки количества гейт можно было бы «пройти», удалив оракулы.
#
# ВАЖНО: до реализации engine-dev'ом этот скрипт обязан быть КРАСНЫМ — оракулы написаны
# architect'ом ДО кода (RED-first; F-4 из R-005 — прошлый круг делал наоборот).
#
# ── Круг 3 (R-009, `research/reviews/R-009-alerting-rev3.md`) ────────────────────────────────
# Reviewer назвал структурный симптом цикла: оракулы пишутся на находки ПРЕДЫДУЩЕГО круга, а
# исправления ТЕКУЩЕГО въезжают без оракулов. Итог — работающий фикс, который любой рефакторинг
# откатит незаметно для ВСЕХ гейтов (мутации A/B2 reviewer'а: блокер переоткрыт, `VERDICT: PASS`).
# Секции 9–11 закрывают это: каждая находка круга 3 получила поведенческий оракул ДО dev'а.
#   * §9  F-9  — редакция тела не-2xx ответа (фикс `5d55914` уже есть; оракул ЗАКРЕПЛЯЕТ его);
#   * §10 F-10 — регрессия `next_seq` + сброс якоря (фикс `62f56cd` есть; оракул закрепляет);
#   * §11 F-11 — `TELEGRAM_BOT_TOKEN` в `Referer` при 3xx: защиты ЕЩЁ НЕТ, оракул КРАСНЫЙ.
# Следовательно этот скрипт КРАСНЫЙ на §11 до реализации engine-dev'ом — так и задумано.

set -uo pipefail

cd "$(dirname "$0")/.." || exit 1

FAILED=0
PASSED=0

check() { # check "<описание>" <команда...>
  local desc="$1"
  shift
  local out rc
  out="$("$@" 2>&1)"
  rc=$?
  if [ "$rc" -eq 0 ]; then
    PASSED=$((PASSED + 1))
    echo "PASS  ${desc}"
  else
    FAILED=$((FAILED + 1))
    echo "FAIL  ${desc}  (exit=${rc})"
    echo "${out}" | tail -25 | sed 's/^/      | /'
  fi
}

run_oracles_in() { # <test-таргет> <фильтр-подстрока> <ожидаемое число тестов>
  local target="$1" filter="$2" expected="$3"
  local out rc
  out="$(cargo test -p ops --test "${target}" -- "${filter}" 2>&1)"
  rc=$?
  echo "${out}"
  [ "${rc}" -eq 0 ] || return 1
  echo "${out}" | grep -Eq "test result: ok\. ${expected} passed" || {
    echo ">>> ожидалось ${expected} зелёных оракулов по фильтру '${filter}' в ${target} — оракулы удалены/переименованы?"
    return 2
  }
}

run_oracles() { # <фильтр-подстрока> <ожидаемое число тестов> — сценарные оракулы слоя склейки
  run_oracles_in red_ops_watchdog_cycle "$1" "$2"
}

grep_present() { grep -Eq -- "$2" "$1"; }
grep_absent() { ! grep -Eq -- "$2" "$1"; }

BIN=crates/ops/src/bin/ops-watchdog.rs
TRANSPORT=crates/ops/src/transport.rs
CYCLE_TEST=crates/ops/tests/red_ops_watchdog_cycle.rs
REDACT_TEST=crates/ops/tests/red_ops_transport_redaction.rs
REDIRECT_TEST=crates/ops/tests/red_ops_transport_redirect.rs

echo "=== 0. Гигиена сборки ==========================================================="
check "cargo fmt --check (весь workspace)" cargo fmt --all -- --check
check "cargo clippy -p ops --all-targets -D warnings" \
  cargo clippy -p ops --all-targets -- -D warnings
check "бинарь ops-watchdog собирается" cargo build -p ops --bin ops-watchdog

echo "=== 1. Оракулы на месте (architect-only, dev их не удаляет и не правит) ========="
check "существует ${CYCLE_TEST}" test -f "${CYCLE_TEST}"
check "существует ${REDACT_TEST}" test -f "${REDACT_TEST}"
check "существует ${REDIRECT_TEST}" test -f "${REDIRECT_TEST}"

echo "=== 2. F-1 — детектор застоя не выключается интервалом cron'а ==================="
check "оракул интервал-независимости присутствует" \
  grep_present "${CYCLE_TEST}" 'fn f1_seq_stall_is_detected_at_every_realistic_cron_interval'
check "оракулы F-1 зелёные (6)" run_oracles f1_ 6
check "склейка переехала в библиотеку: бинарь зовёт run_cycle" grep_present "${BIN}" 'run_cycle'
check "старой склейки в бинаре нет (run_heartbeat_checks)" \
  grep_absent "${BIN}" 'fn run_heartbeat_checks'
check "старой склейки в бинаре нет (push_or_clear)" grep_absent "${BIN}" 'fn push_or_clear'

echo "=== 3. F-2 — секрет не попадает в лог cron'а ===================================="
check "оракулы редакции секрета зелёные (включая сквозной прогон бинаря)" \
  cargo test -p ops --test red_ops_transport_redaction
check "сырой reqwest-error больше не кладётся в TransportError" \
  grep_absent "${TRANSPORT}" 'TransportError::Http\(e\.to_string\(\)\)'
check "хардкоженых токенов в crates/ops/src, scripts, deploy нет" \
  bash -c '! grep -rEn "bot[0-9]{6,}:[A-Za-z0-9_-]{20,}" crates/ops/src scripts deploy'

echo "=== 4. F-3 — прогноз диска переживает ночное окно обслуживания =================="
check "оракулы F-3 зелёные (5)" run_oracles f3_ 5

echo "=== 5. F-5 — «не смог оценить» не стирает дедуп-память =========================="
check "оракулы F-5 зелёные (3)" run_oracles f5_ 3

echo "=== 6. F-6 — маркер <job>.alert («прогон УПАЛ») читается ========================"
check "оракулы F-6 зелёные (4)" run_oracles f6_ 4
check "код инцидента WD-CRON-FAILED объявлен в крейте" \
  grep_present crates/ops/src/watchdog.rs 'WD-CRON-FAILED'

echo "=== 7. F-7 — рестарт-петля продолжает о себе сообщать ==========================="
check "оракулы F-7 зелёные (3)" run_oracles f7_ 3

echo "=== 8. Сквозные vantage + весь крейт ==========================================="
check "здоровый прод не шумит; состояние ограничено за неделю (+F-10, всего 29)" run_oracles _ 29

echo "=== 9. F-9 (R-009) — тело не-2xx ответа не уходит наружу ========================"
# Мутация A reviewer'а (возврат `{status}: {body}` в TransportError) обязана валить эти оракулы;
# проверено architect'ом фактически, включая «щадящий» вариант с усечением тела до 200 байт.
check "оракулы F-9 зелёные (5, включая сквозной прогон бинаря)" \
  run_oracles_in red_ops_transport_redaction f9_ 5

echo "=== 10. F-10 (R-009) — регрессия next_seq: событие + сброс якоря ================"
# Мутация B (удаление ветки) и B2 (удаление сброса якоря — ядро фикса R-008 F-8) обязаны
# валить эти оракулы ПОВЕДЕНЧЕСКИ, а не через clippy: в R-009 линтер был единственным, кто
# заметил мутацию B, и он же молчал на B2.
check "оракулы F-10 зелёные (6)" run_oracles f10_ 6
check "код инцидента WD-SEQ-REGRESSED объявлен в крейте" \
  grep_present crates/ops/src/watchdog.rs 'WD-SEQ-REGRESSED'

echo "=== 11. F-11 (R-009) — токен не уезжает на чужой хост при 3xx ==================="
# КРАСНАЯ секция до реализации engine-dev'ом (RED-first, gates.md §2): оракул на КОНФИГУРАЦИЮ
# redirect-политики HTTP-клиента, а не на форматирование строк.
check "оракулы F-11 зелёные (8)" run_oracles_in red_ops_transport_redirect f11_ 8

echo "=== 12. Весь крейт + workspace =================================================="
check "весь крейт ops зелёный" cargo test -p ops
check "workspace зелёный" cargo test --workspace

echo
echo "checks passed=${PASSED} failed=${FAILED}"
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL"
  exit 1
fi
echo "VERDICT: PASS"
exit 0
