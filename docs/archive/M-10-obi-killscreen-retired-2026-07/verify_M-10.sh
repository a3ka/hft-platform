# ═══════════════ АРХИВНАЯ ВРЕЗКА — добавлена при архивации, не часть оригинала ═══════════
#
#   АРХИВ: acceptance-гейт милестоуна M-10 (kill-screen OBI, Трек A). Сдан в архив 2026-08-15.
#
#   ЭТОТ ФАЙЛ НЕ ИСПОЛНЯЕТСЯ И НЕ ПОДКЛЮЧЁН НИ К ЧЕМУ.
#   Он лежит в `docs/archive/`, не в `scripts/`; бит +x снят; ни один job CI его не зовёт.
#   Запускать нельзя: он проверяет код, которого на `main` нет, и раскладку июля 2026.
#
#   ⛔ Милестоун RETIRED / DEFECTIVE. Гейт зелёным НЕ БЫЛ и закрыть милестоун не мог:
#      `C-020` (risk-critic, CONCERNS) заблокировал merge находкой `KS-I-1` — `sharpe_se`
#      считался от числа шагов, а не от календарного окна, из-за чего защита от ложного
#      PASS не работала. Замер 2026-08-15: дерево слияния ветки с `main` не проходит
#      `cargo test --all` (`error[E0603]: function 'sharpe_se' is private`, exit 101) —
#      ветка умерла между спецификацией фикса и его реализацией.
#
#   ПОЛНЫЙ КОНТЕКСТ, ВЕРДИКТЫ И УРОКИ — в `README.md` этого каталога. Читать его первым.
#
#   Ниже маркера конца врезки — ТЕЛО ОРИГИНАЛА БАЙТ-В-БАЙТ (`scripts/verify_M-10.sh` ветки
#   `feat/M-10-rebased`), включая его собственный shebang. Из-за врезки shebang больше не
#   первая строка — намеренно: файл архивный и исполняться не должен.
#
# ═══════════════════ КОНЕЦ ВРЕЗКИ · ДАЛЕЕ — ОРИГИНАЛ БАЙТ-В-БАЙТ ═══════════════════

#!/usr/bin/env bash
# M-10 R-001 (OBI Трек A, KILL-SCREEN) — acceptance-гейт. Анти-оверфит §6 как исполняемый гейт.
#
# РЕЖИМЫ (C-019 B1 — гейт не смеет зеленеть без артефакта, ради которого создан):
#   (дефолт, FINAL)  fail-closed: R-001 отчёт `.json` И `.md` ОБЯЗАНЫ существовать и быть валидны.
#                    Именно этот режим — close-гейт milestone'а и Done Block research-dev task 4.
#   `--red`          RED-фаза: классификатор ещё не реализован, отчёт ещё не ожидается — проверка
#                    существования отчёта СНИМАЕТСЯ (но red_killscreen всё равно гоняется). НЕ close-гейт.
#
# FAIL-агрегатор (gates.md §3): считаем провалы, exit 1 при FAIL>0. НЕ маскируем через `|| echo`.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

RED_MODE=0
[ "${1:-}" = "--red" ] && RED_MODE=1

FAILED=0
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
pass() { echo "PASS  $*"; }
run() { local name="$1"; shift; if "$@" >/dev/null 2>&1; then pass "${name}"; else fail "${name} (\`$*\`)"; fi; }

# ── Форма (соответствие CI — урок TD-031: verify без fmt/clippy разошёлся с деплоем) ──
# fmt не требует компиляции (пройдёт и в RED-фазе); clippy зеленеет только в FINAL (после impl).
run "T0a cargo fmt --all --check" cargo fmt --all -- --check
run "T0b clippy --workspace -D warnings" cargo clippy --workspace --all-targets --all-features -- -D warnings

# ── Task 1 (KS-I-* RED): классификатор + честность отчёта ──────────────────────────────
# Покрывает classify_verdict (KS-I-1/4) И validate_report_honesty (KS-I-5 gap_ref, KS-I-3 эпоха).
run "KS-I-* kill-screen + честность отчёта (classifier + gap_ref + эпоха + se-масштаб C-020 A)" \
  cargo test -p research-cli --test red_killscreen

# ── C-020 B/C/D: честность стека как ИЗМЕРИТЕЛЯ (fill-семантика, sizing/capacity, робастная V[SR]) ──
run "C-020 стек-честность (fill_probability∈[0,1], unsized→no capacity, robust V[SR])" \
  cargo test -p research-cli --test red_stack_honesty

# ── KS-I-2 пре-регистрация (§4.1): H-карточка с критериями фальсификации СУЩЕСТВУЕТ ────
HCARD="research/hypotheses/H-20260710-obi-asym.md"
if [ -f "${HCARD}" ] && grep -q "критерии фальсификации" "${HCARD}"; then
  pass "KS-I-2 пре-регистрация: H-карточка + критерии фальсификации существуют ДО test"
else
  fail "KS-I-2 нет пре-рег карточки/критериев (${HCARD}) — отчёт невалиден (§4.1)"
fi

# ── KS-I-3 (структурно): trials-ledger append-only механизм. ПУТЬ = .jsonl (C-019 B3) ──
LEDGER="research/trials-ledger.jsonl"
if [ -f "${LEDGER}" ]; then
  pass "KS-I-3 trials-ledger.jsonl существует (append-only + hash-chain — RC-I-*; эпоха TD-015 ниже + risk-critic пункт 0)"
else
  echo "NOTE  ${LEDGER} ещё не создан (появится на первом прогоне грида; отчёт всё равно обязан назвать эпоху)"
fi

# ── Отчёт R-001: FINAL — обязателен fail-closed; --red — отложен ───────────────────────
REPORT_JSON=$(ls research/reports/R-001*obi*trackA*.json 2>/dev/null | head -1)
REPORT_MD=$(ls research/reports/R-001*obi*trackA*.md 2>/dev/null | head -1)

if [ -z "${REPORT_JSON}" ] || [ -z "${REPORT_MD}" ]; then
  if [ "${RED_MODE}" -eq 1 ]; then
    echo "NOTE  [--red] R-001 отчёт (.json/.md) ещё не сгенерён — проверки отложены (RED-фаза, НЕ close-гейт)"
  else
    fail "R-001 отчёт ОТСУТСТВУЕТ: нужны И research/reports/R-001*obi*trackA*.json, И *.md (C-019 B1) — \
milestone НЕ закрывается без артефакта, ради которого создан. Для RED-фазы: verify_M-10.sh --red"
  fi
else
  pass "R-001 отчёт присутствует (json=${REPORT_JSON##*/}, md=${REPORT_MD##*/})"

  # KS-I-1/5 (C-019 B2): обязательные поля честности ПРИСУТСТВУЮТ И НЕПУСТЫ.
  #   span/se/verdict — достоверность; gap_ref — E8 честность окна; code_hash + ledger_cutoff — эпоха.
  for field in data_span_days se_sharpe verdict code_hash gap_ref ledger_cutoff; do
    val=$(grep -oE "\"${field}\"[[:space:]]*:[[:space:]]*(\"[^\"]*\"|-?[0-9.]+)" "${REPORT_JSON}" | head -1)
    if [ -z "${val}" ] || echo "${val}" | grep -qE ':[[:space:]]*""[[:space:]]*$'; then
      fail "KS-I-1/5 отчёт R-001 БЕЗ непустого \`${field}\` — kill-screen требует span+SE+verdict+gap_ref+эпоху (C-019 B2/B3)"
    else
      pass "KS-I-1/5 отчёт несёт непустой \`${field}\`"
    fi
  done

  # KS-I-3 (C-019 B3): анти-СМЕШЕНИЕ эпох (TD-015). Пре-M-07 хэш f7f4761 в отчёте = записи
  # несуществующей логики попали в метрики/deflated-Sharpe → недостоверный ориентир для подписи.
  if grep -q "f7f4761" "${REPORT_JSON}"; then
    fail "KS-I-3 отчёт содержит пре-M-07 code_hash f7f4761 (эпоха несопоставима, TD-015) — смешение эпох; \
метрики/deflated-Sharpe обязаны считаться ТОЛЬКО по записям кода ≥ 5141fd9"
  else
    pass "KS-I-3 отчёт не содержит пре-M-07 эпохи (f7f4761) — эпохи не смешаны (глубокий подсчёт — risk-critic пункт 0)"
  fi

  # C-020 B: fill-метрика честна по имени. «fill_rate» (может быть >1) переименована в fills_per_intent;
  # «доля/вероятность исполнения» ∈[0,1] живёт как fill_probability (если нужна пре-рег критерию №5).
  if grep -qE '"fill_rate"' "${REPORT_JSON}"; then
    fail "C-020 B отчёт несёт поле \`fill_rate\` (было 1.99 > 1) — имя подразумевает долю ∈[0,1]; переименуй в \`fills_per_intent\`"
  else
    pass "C-020 B поля \`fill_rate\` в отчёте нет (переименовано в fills_per_intent — честное имя)"
  fi

  # C-020 C: unsized прогон обязан НАЗВАТЬ себя и не заявлять нефизичную ёмкость.
  if grep -qE '"sizing_applied"' "${REPORT_JSON}"; then
    pass "C-020 C отчёт несёт \`sizing_applied\` (прогон честно помечен sized/unsized)"
  else
    fail "C-020 C отчёт БЕЗ \`sizing_applied\` — capacity/turnover на аккаунт \$500–2k недостоверны без пометки лимитов (§4)"
  fi

  # KS-I-1: ложный Pass на шуме — verdict=Pass, но нижняя CI-граница sharpe−2·se ≤ BAR=0.5.
  if grep -qiE '"verdict"[^}]*[Pp]ass' "${REPORT_JSON}"; then
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
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  if [ "${RED_MODE}" -eq 1 ]; then
    echo "(--red: red_killscreen падает, пока research-dev не реализовал classify_verdict/validate_report_honesty/"
    echo " Verdict/ReportHonesty — корректная RED-фаза. FINAL-гейт дополнительно требует сгенерённый R-001.)"
  fi
  exit 1
fi
echo "VERDICT: PASS"
