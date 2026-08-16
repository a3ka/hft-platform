#!/usr/bin/env bash
# Acceptance-гейт M-67 (rev2) — рыночный слой.
# Гейт архитектора; dev его не правит (scope-guard.md).
#
# Дисциплина (gates.md §3): решение принимается по КОДУ ВОЗВРАТА. Никаких
# `cmd && echo PASS || echo FAIL` — каждая проверка инкрементирует FAIL-счётчик;
# setup, который не смог выполниться, — это FAIL, а не тихий PASS.
#
# ДВА РЕЖИМА, потому что у M-67 два разных момента предъявления:
#   --plan-time : полнота НАБОРА артефактов + оракулы честно КРАСНЫЕ (гейт критика).
#   (без флага) : полный acceptance — оракулы ЗЕЛЁНЫЕ + паритет с CI (гейт после dev).
# Режим не смягчает гейт: plan-time ТРЕБУЕТ красноты и падает, если оракул зелен против
# незаписанной реализации (это было бы плацебо), а полный режим требует обратного.
set -euo pipefail

MODE="full"
[ "${1:-}" = "--plan-time" ] && MODE="plan"

FAILURES=0
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

say()  { printf '%s\n' "$*"; }
pass() { say "PASS  $*"; }
fail() { say "FAIL  $*"; FAILURES=$((FAILURES + 1)); }

say "=== M-67 acceptance (режим: $MODE) ==="
say "дерево: $(pwd)"
say "HEAD:   $(git log -1 --format='%h %s' 2>/dev/null || echo '<не git>')"
say

SPEC="milestones/M-67-market-layer.md"
MEAS="docs/plans/M-67-capacity-2026-08-16.md"
O_HOT="crates/journal/tests/red_md_i2_hot_window.rs"
O_BAND="crates/gateway/tests/red_md_i7_band_lock.rs"
O_JF="crates/gateway/tests/red_md_i6_journal_first.rs"
O_ALLOW="crates/venue-binance/tests/red_l2delta_allowlist.rs"

# ── SETUP-GUARD: предмет обязан существовать, иначе проверять нечего ───────────────────────
for f in "$SPEC" "$MEAS" "$O_HOT" "$O_BAND" "$O_JF" "$O_ALLOW"; do
  if [ -f "$f" ]; then pass "предмет на месте: $f"; else fail "предмет ОТСУТСТВУЕТ: $f"; fi
done
[ "$FAILURES" -eq 0 ] || { say; say "VERDICT: FAIL (setup не состоялся)"; exit 1; }

# ── Шаг M — задача A4: артефакт ёмкости воспроизводим, а не «числа из головы» ──────────────
say
say "--- шаг M: артефакт замера ёмкости (C-091 F-5) ---"
# C-091 требовал поимённо: метка времени, команда, набор символов, определение байта,
# длительность выборки. Проверяется НАЛИЧИЕ каждого признака, а не объём файла.
declare -A M_NEEDLES=(
  ["метка времени"]='2026-08-16'
  ["команда замера"]='ssh|python3'
  ["набор символов"]='475|527|1002'
  ["определение байта"]='байт провода|payload'
  ["длительность выборки"]='90 с|100 с|200 с|90/200|90–200'
)
for k in "${!M_NEEDLES[@]}"; do
  if grep -qE "${M_NEEDLES[$k]}" "$MEAS"; then
    pass "шаг M — артефакт называет $k"
  else
    fail "шаг M — в артефакте НЕТ признака «$k» (C-091 F-5 требовал его поимённо)"
  fi
done
# Множитель 6.8x отменён замером — он не имеет права вернуться в спеку как действующий.
if grep -qE '6\.8[×x]' "$SPEC" && ! grep -qE 'отмен|неверен|отменён' <<<"$(grep -E '6\.8[×x]' "$SPEC")"; then
  fail "шаг M — спека снова использует множитель 6.8x без пометки об отмене"
else
  pass "шаг M — множитель 6.8x не используется как действующий"
fi

# ── Шаг B — замок A-002 З-1: дефолт И семантика артефакта снятия ───────────────────────────
say
say "--- шаг B: замок полос A-002 З-1 ---"
BANDS_DEFAULT="$(grep -oE 'GATEWAY_BANDS:-[0-9.]+' docker-compose.yml | head -1 | sed 's/.*:-//' || true)"
if [ -z "$BANDS_DEFAULT" ]; then
  fail "шаг B — не удалось прочитать дефолт GATEWAY_BANDS (проверка НЕ состоялась)"
elif awk -v b="$BANDS_DEFAULT" 'BEGIN { exit (b <= 0.013) ? 0 : 1 }'; then
  pass "шаг B — дефолт полос $BANDS_DEFAULT внутри валидированной зоны (<=0.013)"
else
  fail "шаг B — замок СНЯТ МОЛЧА: дефолт полос $BANDS_DEFAULT глубже 1.3%"
fi

# Снятие замка требует СЕМАНТИЧЕСКИ годного артефакта, а не любого непустого файла
# (C-091, строка MD-I-7). Первая редакция этой проверки грепала 'per-life|M-58' и давала
# PASS — ЛОЖНЫЙ: файл упоминает M-58 ровно там, где говорит «замок ОСТАЁТСЯ». Греп по
# имени механизма ловит и опровержение этого механизма (`testing.md`: проверка по ВЫЗОВУ
# и по СМЫСЛУ, не по присутствию строки).
VERDICT_F="research/data-quality/depth-verdict.md"
LOCK_STANDS=0
if [ ! -f "$VERDICT_F" ]; then
  fail "шаг B — артефакт $VERDICT_F отсутствует: состояние замка нечем установить"
else
  if grep -qE 'замок A-002 ОСТАЁТСЯ|З-1/З-2 остаются в силе|замок НЕ снят' "$VERDICT_F"; then
    LOCK_STANDS=1
  fi
  if [ "$LOCK_STANDS" -eq 1 ]; then
    pass "шаг B — вердикт прямо фиксирует: замок A-002 ОСТАЁТСЯ (M-58 переснято, исход смешанный)"
    # Замок стоит ⇒ прод-дефолт обязан быть внутри зоны. Это конъюнкция, а не дубль:
    # выше проверен дефолт, здесь — согласованность дефолта с ДЕЙСТВУЮЩИМ вердиктом.
    if awk -v b="${BANDS_DEFAULT:-1}" 'BEGIN { exit (b <= 0.013) ? 0 : 1 }'; then
      pass "шаг B — дефолт согласован с действующим замком"
    else
      fail "шаг B — вердикт держит замок, а прод-дефолт $BANDS_DEFAULT его нарушает"
    fi
  elif grep -qE 'замок (A-002 )?СНЯТ' "$VERDICT_F"; then
    # Снятие возможно ТОЛЬКО подписью founder'а: исход M-58 смешанный ⇒ путь (a)
    # автоматического снятия не применим (A-002).
    if grep -qE 'FOUNDER|подпис' "$VERDICT_F"; then
      pass "шаг B — замок снят и снятие несёт founder-подпись"
    else
      fail "шаг B — замок объявлен снятым БЕЗ founder-подписи (исход M-58 смешанный, граница C)"
    fi
  else
    fail "шаг B — вердикт не заявляет состояние замка ни явно, ни отрицанием: состояние неопределимо"
  fi
fi

# ── Шаг A — оракулы присутствуют, исполняются и НЕ ВАКУУМНЫ ────────────────────────────────
say
say "--- шаг A: оракулы MD-I-* ---"
# ВНИМАНИЕ: функция НЕ трогает `errexit`. Опции оболочки глобальны, и `set -e` внутри
# функции остался бы включённым после возврата — тогда следующий же ненулевой код
# оракула убил бы скрипт (наблюдалось: exit=101 вместо вердикта).
run_oracle() { # $1=crate $2=test-target $3=logfile
  cargo test -p "$1" --test "$2" >"$3" 2>&1
}

set +e
run_oracle journal red_md_i2_hot_window "$TMP/i2.log"; RC_I2=$?
run_oracle gateway red_md_i7_band_lock  "$TMP/i7.log"; RC_I7=$?
run_oracle gateway red_md_i6_journal_first "$TMP/i6.log"; RC_I6=$?
run_oracle venue-binance red_l2delta_allowlist "$TMP/i1.log"; RC_I1=$?
set -e

# Вакуум-контроль: оракул, не запустивший НИ ОДНОГО сценария, зелен бессмысленно.
for pair in "i2:$TMP/i2.log:md_i2_" "i7:$TMP/i7.log:md_i7_" "i6:$TMP/i6.log:md_i6_"; do
  id="${pair%%:*}"; rest="${pair#*:}"; logf="${rest%%:*}"; pfx="${rest#*:}"
  n="$(grep -cE "^test ${pfx}" "$logf" || true)"
  if [ "${n:-0}" -ge 2 ]; then
    pass "шаг A — $id запустил $n сценариев (обе стороны анти-плацебо присутствуют)"
  else
    fail "шаг A — $id запустил ${n:-0} сценариев (<2): одностороннего оракула недостаточно"
  fi
done

if [ "$MODE" = "plan" ]; then
  # PLAN-TIME: реализации ещё нет ⇒ MD-I-2 и MD-I-7 ОБЯЗАНЫ падать. Зелёный здесь
  # означал бы, что оракул не давит на инвариант (анти-плацебо, gates.md §2).
  [ "$RC_I2" -ne 0 ] && pass "шаг A — MD-I-2 КРАСНЫЙ до реализации (как и требуется)" \
                     || fail "шаг A — MD-I-2 ЗЕЛЁНЫЙ против ненаписанной реализации = плацебо"
  [ "$RC_I7" -ne 0 ] && pass "шаг A — MD-I-7 КРАСНЫЙ до реализации (как и требуется)" \
                     || fail "шаг A — MD-I-7 ЗЕЛЁНЫЙ против ненаписанной реализации = плацебо"
  # Позитивные стороны обязаны быть ЗЕЛЁНЫМИ даже сейчас — иначе оракул «всё красное».
  grep -qE '^test md_i2_b2_.* ok$'  "$TMP/i2.log" \
    && pass "шаг A — MD-I-2 b2 (обратная сторона) зелёная: не «красное на всё»" \
    || fail "шаг A — MD-I-2 b2 не зелёная: оракул не различает сломанное и рабочее"
  grep -qE '^test md_i7_p1_.* ok$'  "$TMP/i7.log" \
    && pass "шаг A — MD-I-7 p1 (позитив) зелёная: реализация «отказывать всегда» была бы поймана" \
    || fail "шаг A — MD-I-7 p1 не зелёная: оракул не отличает замок от выключения продукта"
else
  # FULL: реализация обязана быть, оракулы ЗЕЛЁНЫЕ.
  [ "$RC_I2" -eq 0 ] && pass "шаг A — MD-I-2 GREEN (периметр ретеншена режет по данным)" \
                     || { fail "шаг A — MD-I-2 не зелёный:"; tail -25 "$TMP/i2.log"; }
  [ "$RC_I7" -eq 0 ] && pass "шаг A — MD-I-7 GREEN (замок полос стоит кодом)" \
                     || { fail "шаг A — MD-I-7 не зелёный:"; tail -25 "$TMP/i7.log"; }
fi

# MD-I-6 — СТОРОЖ: зелёный на ЛЮБОЙ стадии. Красный = регресс journal-first.
[ "$RC_I6" -eq 0 ] && pass "шаг A — MD-I-6 сторож journal-first зелёный" \
                   || { fail "шаг A — MD-I-6 КРАСНЫЙ: значение доходит до выдачи мимо журнала"; tail -25 "$TMP/i6.log"; }
# MD-I-1 покрыт существующим оракулом M-45 — дубликат не заводится, но обязан быть зелёным.
[ "$RC_I1" -eq 0 ] && pass "шаг A — MD-I-1 покрыт red_l2delta_allowlist (M-45), GREEN" \
                   || { fail "шаг A — allow-list fail-closed сломан"; tail -25 "$TMP/i1.log"; }

# ── Шаг D — заблокированные задачи НЕ диспетчеризованы ─────────────────────────────────────
say
say "--- шаг D: развилка §5 не обойдена кодом ---"
# Пока форма L3 не решена founder'ом (P1) и не проведена contract-RFC (P2), кода
# DepthAggregate/MarketTicker существовать не должно ни в одном крейте.
LEAK="$(git grep -lE 'DepthAggregate|MarketTicker' -- 'crates/*/src' 2>/dev/null || true)"
if [ -z "$LEAK" ]; then
  pass "шаг D — DepthAggregate/MarketTicker отсутствуют в crates/*/src (развилка §5 не обойдена)"
else
  fail "шаг D — форма L3 появилась в коде БЕЗ разрешения развилки §5 и contract-RFC: $LEAK"
fi
# L5 не имеет права строиться на потоке, который замером даёт 0 сообщений.
if git grep -qE '!markPrice@arr' -- 'crates/*/src' 2>/dev/null; then
  fail "шаг D — код подписывается на !markPrice@arr: замер 2026-08-16 даёт 0 сообщений (спека §4.1)"
else
  pass "шаг D — L5 не строится на недоставляемом !markPrice@arr"
fi

# ── Шаг C — contracts не тронуты вне contract-RFC ──────────────────────────────────────────
say
say "--- шаг C: contract Block-C ---"
BASE="$(git merge-base HEAD origin/main 2>/dev/null || echo '')"
if [ -z "$BASE" ]; then
  fail "шаг C — не удалось вычислить merge-base с origin/main (проверка НЕ состоялась)"
else
  CH="$(git diff --name-only "$BASE"..HEAD -- crates/contracts 2>/dev/null || true)"
  if [ -z "$CH" ]; then
    pass "шаг C — crates/contracts не тронут"
  elif ls docs/contract-rfc/*.md >/dev/null 2>&1 || git diff --name-only "$BASE"..HEAD -- docs/rfc | grep -q 'CT-RFC'; then
    pass "шаг C — contracts тронут вместе с contract-RFC"
  else
    fail "шаг C — crates/contracts тронут БЕЗ contract-RFC (CT-I-2, авто-REJECT): $CH"
  fi
fi

# ── Шаг CI — паритет с базовым job'ом build-test (gates.md §3) ─────────────────────────────
say
say "--- шаг CI: паритет с .github/workflows/ci.yml (build-test) ---"
if [ "$MODE" = "plan" ]; then
  # На plan-time RED-оракулы КРАСНЫЕ ⇒ `cargo test --all` красный ЗАКОНОМЕРНО.
  # Поэтому здесь гоняются только те проверки CI, которые обязаны быть зелёными
  # всегда: форматирование и линт. Это НЕ послабление: ветка с RED в main не идёт
  # (gates.md §8), а `cargo test --all` предъявляется в полном режиме.
  set +e
  cargo fmt --all -- --check >"$TMP/fmt.log" 2>&1; RC_FMT=$?
  cargo clippy --all-targets --all-features -- -D warnings >"$TMP/clippy.log" 2>&1; RC_CLIPPY=$?
  set -e
  [ "$RC_FMT" -eq 0 ]    && pass "CI — cargo fmt --all -- --check"    || { fail "CI — fmt"; tail -15 "$TMP/fmt.log"; }
  [ "$RC_CLIPPY" -eq 0 ] && pass "CI — cargo clippy -D warnings"      || { fail "CI — clippy"; tail -25 "$TMP/clippy.log"; }
  say "INFO  CI — cargo test --all не гоняется на plan-time: RED-оракулы красны намеренно"
else
  set +e
  cargo fmt --all -- --check >"$TMP/fmt.log" 2>&1; RC_FMT=$?
  cargo clippy --all-targets --all-features -- -D warnings >"$TMP/clippy.log" 2>&1; RC_CLIPPY=$?
  cargo test --all >"$TMP/test.log" 2>&1; RC_TEST=$?
  set -e
  [ "$RC_FMT" -eq 0 ]    && pass "CI — cargo fmt --all -- --check"    || { fail "CI — fmt"; tail -15 "$TMP/fmt.log"; }
  [ "$RC_CLIPPY" -eq 0 ] && pass "CI — cargo clippy -D warnings"      || { fail "CI — clippy"; tail -25 "$TMP/clippy.log"; }
  [ "$RC_TEST" -eq 0 ]   && pass "CI — cargo test --all ($(grep -cE '^test result: ok' "$TMP/test.log" || echo 0) блоков ok)" \
                         || { fail "CI — cargo test --all:"; grep -E '^(test result: FAILED|failures:)' -A 5 "$TMP/test.log" | head -40; }
fi

# ── Вердикт ────────────────────────────────────────────────────────────────────────────────
say
if [ "$FAILURES" -eq 0 ]; then
  say "VERDICT: PASS"
  exit 0
else
  say "VERDICT: FAIL ($FAILURES проверок не прошли)"
  exit 1
fi
