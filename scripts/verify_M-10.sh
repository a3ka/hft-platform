#!/usr/bin/env bash
# M-10 R-001 (OBI Трек A, KILL-SCREEN) — acceptance-гейт. Анти-оверфит §6 как исполняемый гейт.
#
# RED-фаза: `red_killscreen` падает, пока research-dev не реализовал `classify_verdict`/`Verdict`/
# новые поля отчёта → VERDICT: FAIL (корректно). Структурные проверки (пре-рег карточка) проходят уже.
# Отчёт R-001 проверяется, КОГДА research-dev его сгенерит (task 4).
#
# FAIL-агрегатор (gates.md §3): считаем провалы, exit 1 при FAIL>0. НЕ маскируем через `|| echo`.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

FAILED=0
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
pass() { echo "PASS  $*"; }
run() { local name="$1"; shift; if "$@" >/dev/null 2>&1; then pass "${name}"; else fail "${name} (\`$*\`)"; fi; }

# ── Task 1 (KS-I-* RED): kill-screen честность ────────────────────────────────────────
run "KS-I-* kill-screen (PASS запрещён на шуме; пре-рег критерий→Kill; Pass достижим)" \
  cargo test -p research-cli --test red_killscreen

# ── KS-I-2 пре-регистрация (§4.1): H-карточка с критериями фальсификации СУЩЕСТВУЕТ ────
HCARD="research/hypotheses/H-20260710-obi-asym.md"
if [ -f "${HCARD}" ] && grep -q "критерии фальсификации" "${HCARD}"; then
  pass "KS-I-2 пре-регистрация: H-карточка + критерии фальсификации существуют ДО test"
else
  fail "KS-I-2 нет пре-рег карточки/критериев (${HCARD}) — отчёт невалиден (§4.1)"
fi

# ── Отчёт R-001: валиден ТОЛЬКО с span/se/verdict/эпохой (проверяется, когда сгенерён) ──
REPORT_JSON=$(ls research/reports/R-001*obi*trackA*.json 2>/dev/null | head -1)
if [ -n "${REPORT_JSON}" ]; then
  # KS-I-1: обязательные поля честности присутствуют.
  for field in data_span_days se_sharpe verdict code_hash; do
    grep -q "\"${field}\"" "${REPORT_JSON}" \
      && pass "KS-I-1 отчёт несёт \`${field}\`" \
      || fail "KS-I-1 отчёт R-001 БЕЗ \`${field}\` — недостоверный вердикт (kill-screen требует span+SE+эпоху)"
  done
  # KS-I-1: ложный Pass на шуме — грубая проверка (verdict=Pass, но нижняя CI-граница ≤ BAR).
  #   sharpe−2·se ≤ 0.5 при verdict Pass → ложный промоушен.
  if grep -qi '"verdict"[^}]*[Pp]ass' "${REPORT_JSON}"; then
    sharpe=$(grep -oE '"sharpe"[[:space:]]*:[[:space:]]*-?[0-9.]+' "${REPORT_JSON}" | head -1 | grep -oE '\-?[0-9.]+$')
    se=$(grep -oE '"se_sharpe"[[:space:]]*:[[:space:]]*-?[0-9.]+' "${REPORT_JSON}" | head -1 | grep -oE '\-?[0-9.]+$')
    lower=$(awk -v s="${sharpe:-0}" -v e="${se:-99}" 'BEGIN{print s-2*e}')
    if awk -v l="${lower}" 'BEGIN{exit !(l>0.5)}'; then
      pass "KS-I-1 отчёт с Pass имеет нижнюю CI-границу > BAR (промоушабелен честно)"
    else
      fail "KS-I-1 ЛОЖНЫЙ PASS: verdict=Pass, но sharpe−2·se=${lower} ≤ BAR=0.5 — промоушен на шуме (kill-screen пробит)"
    fi
  else
    pass "KS-I-1 отчёт НЕ заявляет Pass (Kill/Inconclusive — честно для короткого окна)"
  fi
  # KS-I-3 эпоха (TD-015): отчёт называет эпоху (code_hash присутствует выше); ledger-эпоха — risk-critic пункт 0.
else
  echo "NOTE  отчёт R-001 ещё не сгенерён (research-dev task 4) — проверки валидности отчёта отложены"
fi

# ── KS-I-3 (структурно): trials-ledger append-only механизм (не ручная правка) ─────────
LEDGER="research/trials-ledger.json"
if [ -f "${LEDGER}" ]; then
  pass "KS-I-3 trials-ledger существует (append-only + hash-chain — RC-I-*; эпоха TD-015 проверяет risk-critic)"
else
  echo "NOTE  trials-ledger ещё не создан (появится на первом прогоне грида)"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "(red_killscreen падает, пока research-dev не реализовал classify_verdict/Verdict/поля отчёта —"
  echo " корректная RED-фаза; гейт зеленеет после impl + генерации валидного R-001.)"
  exit 1
fi
echo "VERDICT: PASS"
