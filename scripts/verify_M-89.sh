#!/usr/bin/env bash
# verify_M-89.sh — acceptance-гейт milestone'а M-89 «остаток S0: работа одной выдачи
# ограничена структурно и видна снаружи процесса».
# Спека: milestones/M-89-s0-volume-guard-observability.md
#
# Решение по КОДУ ВОЗВРАТА (`gates.md` §3). Агрегатор с FAIL-счётчиком: гейт обязан
# перечислить ВСЕ провалы, а не умереть на первом.
#
# Базовая линия снимается КРАСНОЙ до работы dev'а и предъявляется в Handoff.
#
# Режим пробы (`scripts/tests/red_verify_M-89_task_status.sh`):
#   VERIFY_M89_TASK_STATUS_ONLY=1 VERIFY_M89_SPEC=<файл> bash scripts/verify_M-89.sh
# исполняет ТОЛЬКО шаг task-status над указанной спекой — чтобы барьер был под пробой, а не
# только под доверием (TD-221: барьер проверяет наличие заявления, а не правдивость — предел
# назван; проба ловит хотя бы форму).

set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP  %s\n' "$1"; }

GW=crates/gateway/src/lib.rs
JS=crates/journal/src/segments.rs
GS=crates/gateway-serve/src
SPEC="${VERIFY_M89_SPEC:-milestones/M-89-s0-volume-guard-observability.md}"

# Один cargo-test бинарь → одна строка PASS/FAIL, при провале — хвост причин.
run_test() { # <метка> <описание> <cargo test args…>
  local tag="$1" desc="$2"; shift 2
  local out rc line
  out=$(cargo test "$@" 2>&1)
  rc=$?
  line=$(printf '%s\n' "$out" | grep -E '^test result' | tail -1)
  if [ $rc -eq 0 ]; then
    pass "$tag: $desc — ${line:-GREEN}"
  else
    fail "$tag: $desc КРАСЕН — ${line:-компиляция}"
    printf '%s\n' "$out" | grep -E '^(error(\[E[0-9]+\])?:|thread |SETUP|I-[0-9]|TD-|assertion)' | cut -c1-220 | head -8
  fi
}

# ─────────── круг `R-202` → `TD-221`: §Tasks НЕ ВРЁТ О СЕБЕ (перенос из verify_M-87.sh) ───────────
# Класс повторился три круга подряд (R-200 §B3 · R-201 · R-202): статус без названного остатка
# неотличим от протухшей пометки. Исправления TD-221 против редакции M-87:
#   (а) номер задачи — `[0-9]+(б|bis)?`: строка `2bis` больше не пропадает из проверки;
#   (б) `🟡`-строки ПРОВЕРЯЮТСЯ наравне с `⏳ OPEN`, а не молча отбрасываются `case`-веткой.
# ПРЕДЕЛ НАЗВАН ЧЕСТНО: барьер проверяет НАЛИЧИЕ заявления, а не его ПРАВДИВОСТЬ (мутация C
# `R-204`). Ложный, но существующий остаток пройдёт — против него круг гейта.
task_status_check() { # <spec>
  local spec="$1" bad="" open=0 line num
  [ -f "$spec" ] || { fail "task-status: спека $spec не найдена"; return; }
  while IFS= read -r line; do
    open=$((open + 1))
    num=$(printf '%s' "$line" | sed -E 's/^\| ([0-9]+(б|bis)?) \|.*/\1/')
    # Маркер ищется ПОСЛЕ колонки статуса: слово «ЧАСТИЧНО» в самой колонке `🟡 ЧАСТИЧНО` —
    # статус, а не остаток (проба s5).
    rest=$(printf '%s' "$line" | cut -d'|' -f4-)
    if ! printf '%s' "$rest" | grep -qE 'ОСТАТОК|остаток|НЕ ПОДКЛЮЧЁН|не подключён|ЧАСТИЧНО|не исполнен|долг|ОБОСНОВАНИЕ ПЕРЕПИСАНО|ещё не начат|ждёт'; then
      bad="$bad $num"
    fi
  done < <(grep -E '^\| [0-9]+(б|bis)? \| (⏳ OPEN|🟡)' "$spec")
  if [ "$open" -eq 0 ]; then
    pass "task-status: открытых/частичных задач нет — заявлять остаток нечего"
  elif [ -z "$bad" ]; then
    pass "task-status: каждая из $open открытых/частичных задач НАЗЫВАЕТ свой остаток"
  else
    fail "task-status: задачи БЕЗ названного остатка:$bad — пометка неотличима от протухшей (R-200 §B3 · R-201 · R-202; TD-221: 2bis и 🟡 теперь видны)"
  fi
}

if [ "${VERIFY_M89_TASK_STATUS_ONLY:-0}" = "1" ]; then
  task_status_check "$SPEC"
  printf '\n'
  if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
  echo "VERDICT: FAIL (провалов: $FAIL)"; exit 1
fi

# ─────────────── задача 1 — journal: примитив поиска позиции + счётчик байт стрима ───────────────
if grep -qE '^\s*pub fn locate_after_seq\(' "$JS" && grep -qE 'pub fn payload_bytes_read\(&self\)' "$JS"; then
  pass "task1: journal несёт locate_after_seq и EventStream::payload_bytes_read (аддитивно)"
else
  fail "task1: в $JS нет pub fn locate_after_seq / EventStream::payload_bytes_read — форма §4.1 не выполнена"
fi
run_test "task1" "sacred-корпус journal зелен (JR-I-2/JR-I-11 не задеты)" -p journal

# ─────────────── задачи 1+2+3 — I-1/I-2/I-4 на библиотеке (прод-масштаб, rchar ядра) ───────────────
run_test "task1+2+3" "red_m89_warm_resume_seek (w1/p1/d1..d9)" -p gateway --test red_m89_warm_resume_seek
if grep -qE '^\s*pub seek_fallbacks: u64,' "$GW"; then
  pass "task2: ReadStats несёт seek_fallbacks — откат поиска позиции НАБЛЮДАЕМ"
else
  fail "task2: в ReadStats нет seek_fallbacks — «сдвиг не сработал, всё перечиталось» невидимо в статистике"
fi
PBAC=$(grep -c 'payload_bytes_after_cursor' "$GW" || true)
if [ "$PBAC" -eq 0 ]; then
  pass "task3: payload_bytes_after_cursor удалён — read_dir+заголовки ушли с горячего пути (задача 15 M-87)"
else
  fail "task3: payload_bytes_after_cursor всё ещё в $GW ($PBAC упоминаний) — опись каталога на каждую выдачу"
fi
# Существующий корпус — БЕЗ файлов этого набора (они красны по заявлению, пока dev не сделал
# задачи 1–3; их судит шаг task1+2+3). Проверяется, что hint/тик/каталог не регрессировали.
GW_OLD_FAIL=""
for f in crates/gateway/tests/*.rs; do
  n=$(basename "$f" .rs)
  case "$n" in red_m89_*) continue;; esac
  cargo test -q -p gateway --test "$n" >/dev/null 2>&1 || GW_OLD_FAIL="$GW_OLD_FAIL $n"
done
if cargo test -q -p gateway --lib >/dev/null 2>&1 && [ -z "$GW_OLD_FAIL" ]; then
  pass "task2/D-1(б): существующий корпус gateway зелен (hint, тик, каталог не регрессировали)"
else
  fail "task2/D-1(б): существующий корпус gateway КРАСЕН —${GW_OLD_FAIL:- lib}"
fi

# ─────────────── задача 4 — счётчик выдачи включает каждый pump (WS, rchar ядра) ───────────────
run_test "task4" "red_m89_read_volume_truth (v)" -p gateway-serve --test red_m89_read_volume_truth
run_test "task4" "red_m87_read_volume_truth::q2 остался зелёным" -p gateway-serve --test red_m87_read_volume_truth

# ─────────────── задача 5 — бюджет-плацебо удалён; граница СТРУКТУРНАЯ ───────────────
# СТРОКИ КОДА, не текст (`testing.md`: проверка по ВЫЗОВУ, не по имени). В трёх sacred-тестах
# (`red_m87_r196_conditions.rs`, `red_m87_read_volume_truth.rs`, `red_m89_structural_bound.rs`)
# имена живут в КОММЕНТАРИЯХ-истории кругов — dev их править не вправе, и счёт по тексту
# держал бы шаг красным навсегда. Комментарии (`//`, `///`, `//!`) отсекаются до счёта; в `src`
# — весь крейт, не один файл: перенос определения в соседний модуль шаг не обманет.
code_hits() { grep -rhE "$1" "${@:2}" 2>/dev/null | grep -vE '^[[:space:]]*//' | wc -l; }
DEAD=$(code_hits 'fn feed_tail_within|fn pump_one|struct CallBudget|enum BudgetStop|trait Cancel|struct PumpStep' "$GS")
DEAD_T=$(code_hits 'feed_tail_within\(|CallBudget \{|BudgetStop::' crates/gateway-serve/tests/)
if [ "$DEAD" -eq 0 ] && [ "$DEAD_T" -eq 0 ]; then
  pass "task5: бюджет-плацебо (feed_tail_within/pump_one/CallBudget/BudgetStop/Cancel) удалён из src и tests"
else
  fail "task5: бюджет-плацебо ещё жив — упоминаний в admission.rs: $DEAD, файлов тестов с вызовами: $DEAD_T (TD-219: подключать его значило бы подключить дефект)"
fi
run_test "task5/I-3" "red_m89_structural_bound (s1, прод-форма 200k, rchar)" -p gateway-serve --test red_m89_structural_bound

# ─────────────── задача 6 — счётчики на экземпляр ───────────────
run_test "task6" "red_m89_counters_instance (i1/i2)" -p gateway-serve --test red_m89_counters_instance

# ─────────────── задача 7 — оракулы точки входа M-87 на ручке экземпляра; сторож SERIAL ───────────────
# `--features testing` ОБЯЗАТЕЛЕН: c4 стоит под флагом (M-87 §14.1quater), без него он не исполняется.
run_test "task7" "red_m87_entrypoint под --features testing" -p gateway-serve --features testing --test red_m87_entrypoint
run_test "task7" "red_m87_entrypoint В ФОРМЕ CI (без флага)" -p gateway-serve --test red_m87_entrypoint
run_test "task7" "red_m87_registry (биекция сценариев цела)" -p gateway-serve --test red_m87_registry
run_test "task7/TD-225" "red_m89_serial_guard (tokio::test == SERIAL.lock, пока SERIAL объявлен)" -p gateway-serve --test red_m89_serial_guard

# ─────────────── задачи 8+9 — сердцебиение: точка входа прод-бинаря + compose ───────────────
cargo build -q -p gateway-serve --bin gateway-serve 2>/dev/null
run_test "task8+9" "red_m89_heartbeat_entrypoint (h0 compose, h1 прод-бинарь)" -p gateway-serve --test red_m89_heartbeat_entrypoint
run_test "task9" "red_m87_prod_entrypoint_argv остался зелёным (политика на compose-окружении)" -p gateway-serve --test red_m87_prod_entrypoint_argv

# ─────────────── задача 10 — ops: читатель, инциденты, бинарь ───────────────
run_test "task10" "crates/ops целиком (red_m89_serving_silence + существующие)" -p ops

# ─────────────── задача 11 — КОМПОЗИЦИЯ: куда пишет compose ↔ откуда читает cron ───────────────
# `testing.md` §«Механизм несущего пути обязан иметь оракул точки входа», п. 2: путь producer'а и
# путь consumer'а — две строки в двух файлах, и их расхождение даёт тихий no-op.
hb_path=$(awk '/^  gateway-serve:/{s=1;next} s&&/^  [a-z]/{s=0} s&&/GATEWAY_HEARTBEAT_PATH:/{sub(/.*GATEWAY_HEARTBEAT_PATH:[ ]*/,""); gsub(/"/,""); print; exit}' docker-compose.yml)
hb_path=${hb_path%%\#*}; hb_path=$(printf '%s' "$hb_path" | sed -E 's/^\$\{[A-Z_]+:-([^}]*)\}$/\1/' | tr -d ' ')
mount_line=$(awk '/^  gateway-serve:/{s=1;next} s&&/^  [a-z]/{s=0} s&&/^      - /{print}' docker-compose.yml | tr -d '"' | sed -E 's/^ *- //')
host_path=""
if [ -n "$hb_path" ]; then
  while IFS= read -r m; do
    vol=${m%%:*}; rest=${m#*:}; dst=${rest%%:*}; mode=${rest#"$dst"}
    case "$hb_path" in
      "$dst"/*) [ "$mode" != ":ro" ] && host_path="/var/lib/docker/volumes/hft-platform_${vol}/_data/${hb_path#"$dst"/}";;
    esac
  done <<< "$mount_line"
fi
CRON=deploy/cron.d/watchdog
cron_path=$(grep -E '^WATCHDOG_SERVING_HEARTBEAT_PATH=' "$CRON" 2>/dev/null | head -1 | cut -d= -f2-)
if [ -z "$hb_path" ]; then
  fail "task11: compose не объявляет GATEWAY_HEARTBEAT_PATH у gateway-serve — композицию сверять не с чем"
elif [ -z "$host_path" ]; then
  fail "task11: GATEWAY_HEARTBEAT_PATH=$hb_path не лежит на rw-монтировании gateway-serve (монтирования: $(echo $mount_line))"
elif [ ! -f "$CRON" ]; then
  fail "task11: нет $CRON — читателю не назначен путь; на проде ops-watchdog не установлен (ssh 2026-09-27: нет /etc/cron.d/hft-watchdog, нет бинаря)"
elif [ "$cron_path" != "$host_path" ]; then
  fail "task11: КОМПОЗИЦИЯ РАЗОШЛАСЬ — compose пишет в $host_path, cron читает ${cron_path:-<пусто>}"
elif ! grep -qE 'scripts/watchdog_cron\.sh' "$CRON"; then
  fail "task11: $CRON не зовёт scripts/watchdog_cron.sh — фрагмент без строки расписания"
else
  pass "task11: композиция путей сходится: $host_path (compose ⇒ хост) == cron"
fi
skip "task11: установка $CRON в /etc/cron.d и сборка ops-watchdog на VPS — ручной шаг founder ★ (deploy/README.md); предъявляется reviewer'ом на §8-гейте (ls /etc/cron.d/hft-watchdog, свежий /var/lib/hft/watchdog.last-success)"

# ─────────────── задача 12 — прод-замер ДО/ПОСЛЕ (только на проде) ───────────────
skip "task12: rchar/время первой выдачи новой подписки на проде ДО/ПОСЛЕ при курсоре слепка глубоко в активном сегменте (замер 2026-09-27: ≈412 МБ перед курсором) — снимает reviewer на §8-гейте, сырые строки в R-NNN"

# ─────────────── task-status (TD-221) ───────────────
task_status_check "$SPEC"

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
