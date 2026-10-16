# ═══════════════ АРХИВНАЯ ВРЕЗКА — добавлена при архивации, не часть оригинала ═══════════
#
#   АРХИВ: зонтичный acceptance-гейт M-60 (сдан в архив 2026-08-15)
#
#   ЭТОТ ФАЙЛ НЕ ИСПОЛНЯЕТСЯ И НЕ ПОДКЛЮЧЁН НИ К ЧЕМУ.
#   Он лежит в `docs/archive/`, не в `scripts/`; бит +x снят; ни один job CI и ни один
#   `verify_*.sh` его не зовёт. Запускать его нельзя: он проверяет корпус и раскладку
#   августа 2026 и против текущего `main` даст ложное красное.
#
#   ── Почему архивирован ────────────────────────────────────────────────────────────────
#   Зонт M-60 разрезан на M-60a / M-60b / M-60c решением founder'а 2026-08-06. Гейт зонта
#   пережил свой предмет: у каждого наследника собственный acceptance-скрипт.
#   Ветка-носитель `feat/M-60-mechanisms` мертва с 2026-08-07.
#
#   Распоряжение по этому пути — `milestones/M-60b-gate-mechanisms.md` §12:
#     «частично перенесён: только нормализация корпуса `verify_M-60.sh:67-72` живёт в
#      `scripts/verify_M-60c.sh`; весь старый зонтичный verify не переносится, founder
#      либо архивирует вместе с зонтом, либо списывает».
#   Решение founder'а от 2026-08-15: АРХИВИРОВАТЬ. Настоящий файл — исполнение решения.
#
#   ── Чем заменён ───────────────────────────────────────────────────────────────────────
#     `scripts/verify_M-60a.sh`  — замок процессного слоя (M-60a, смержен `71611d0`)
#     `scripts/verify_M-60b.sh`  — бюджеты чтения · GATE-META · verify_design_claims в CI
#     `scripts/verify_M-60c.sh`  — чистка корпуса; сюда перенесена нормализация :67-72
#   Зонтичная спека архивирована рядом: `docs/archive/M-60-mechanisms-umbrella-2026-08.md`
#
#   ── Форма файла ───────────────────────────────────────────────────────────────────────
#   Ниже маркера конца врезки — ТЕЛО ОРИГИНАЛА БАЙТ-В-БАЙТ (`scripts/verify_M-60.sh` ветки
#   `feat/M-60-mechanisms`, HEAD `f0e915b`), включая его собственный shebang. Из-за врезки
#   shebang больше не первая строка — это намеренно: файл архивный и исполняться не должен.
#
# ═══════════════════ КОНЕЦ ВРЕЗКИ · ДАЛЕЕ — ОРИГИНАЛ БАЙТ-В-БАЙТ ═══════════════════

#!/usr/bin/env bash
# Acceptance-гейт M-60 — «механизм вместо прозы»: замок процессного слоя, перенос механизмов
# einhard, возврат корпуса к бюджету A-003.
#
# Решение принимается по КОДУ ВОЗВРАТА (gates.md §3). Агрегатор со счётчиком: печатаем все
# нарушения разом, exit 1 при FAIL>0 — иначе первый же красный шаг скрыл бы остальные.
#
# ПОРЯДОК ВАЖЕН. Шаг B (бюджет) и шаг D (нормы на месте) — это ДВЕ ПОЛОВИНЫ одной проверки
# чистки: B доказывает, что вырезано достаточно, D — что вырезана ИСТОРИЯ, а не НОРМА.
# По отдельности каждая половина ложна: пройти B можно, снеся требования, а пройти D —
# не сократив ничего. R-032 поймал ровно первое: на прошлом откате вместе с текстом выпало
# требование risk-critic на документы safety-пути — единственный сегодня невакуумный
# риск-гейт (крейтов risk/killswitch/oms не существует, RK-I-* живут текстом в docs/fa/*).

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

RULES_BUDGET=725      # цель A-003 §3
CLAUDE_BUDGET=70      # цель A-003 §3

echo "--- A: гейт-скрипты существуют, исполнимы, парсятся ---"
for s in check_docs_freeze check_context_budgets check_gate_meta; do
  if [ -f "scripts/${s}.sh" ] && bash -n "scripts/${s}.sh" 2>/dev/null; then
    pass "A ${s}.sh на месте и парсится"
  else
    fail "A ${s}.sh отсутствует или не парсится"
  fi
done

echo "--- B: БЮДЖЕТ обязательного чтения (первая половина проверки чистки) ---"
RULES_LINES=$(cat .claude/rules/*.md 2>/dev/null | wc -l)
CLAUDE_LINES=$(wc -l < CLAUDE.md 2>/dev/null || echo 99999)
if [ "${RULES_LINES}" -le "${RULES_BUDGET}" ]; then
  pass "B .claude/rules = ${RULES_LINES} строк (бюджет ${RULES_BUDGET})"
else
  fail "B .claude/rules = ${RULES_LINES} строк, бюджет ${RULES_BUDGET} — превышение на $((RULES_LINES - RULES_BUDGET))"
fi
if [ "${CLAUDE_LINES}" -le "${CLAUDE_BUDGET}" ]; then
  pass "B CLAUDE.md = ${CLAUDE_LINES} строк (бюджет ${CLAUDE_BUDGET})"
else
  fail "B CLAUDE.md = ${CLAUDE_LINES} строк, бюджет ${CLAUDE_BUDGET} — превышение на $((CLAUDE_LINES - CLAUDE_BUDGET))"
fi

echo "--- D: НОРМЫ НА МЕСТЕ (вторая половина; чистка режет историю, не требования) ---"
# Список валидирован грепом по живому корпусу при написании гейта: формулировка, которой
# нет, дала бы вечно-красный шаг и была бы вычеркнута как «шумная» — то есть защита
# исчезла бы обходным путём.
NORMS=(
  "risk-critic обязателен дополнительно"      # R-032 F-1: единственный невакуумный риск-гейт
  "не делегируется никому, включая арбитра"   # граница C не уходит даже арбитру
  "RssAnon"                                   # TD-021: docker stats считает page cache
  "sanity свежих событий"                     # TD-031: rollback не ловит тихую порчу данных
  "CPU/MEM в норме"                           # TD-011: поймано ТОЛЬКО ресурсным взглядом
  "НИКОГДА не включает написание тестов"      # RED-first — правило о ПОРЯДКЕ, не о зонах
  "GREEN против заглушки"                     # анти-плацебо
  "по КОДУ ВОЗВРАТА, а не по тексту"          # gates.md §3
  "research/reviews/R-NNN.md"                 # вердикт reviewer'а есть артефакт
  "гейт, который зеленее CI"                  # паритет verify с CI
  'Никогда `git commit -a`'                   # чужая работа сносится одной командой
  "Push-scope перед КАЖДЫМ push"              # чужие коммиты не уезжают в main
)
# Корпус НОРМАЛИЗУЕТСЯ перед поиском: переносы строк → пробелы, пробелы схлопываются.
# Причина найдена прогоном этого же гейта: фраза «не делегируется никому» разорвана
# переносом (gates.md:45-46), а grep работает построчно — норма на месте, проверка красная.
# Построчный вариант ломался бы и от любой ПЕРЕВЁРСТКИ абзаца, то есть именно от того, что
# делает предстоящая чистка. Вечно-красный шаг объявляют шумом и выключают — так защита
# исчезает не отменой, а раздражением.
CORPUS="$(cat .claude/rules/*.md CLAUDE.md 2>/dev/null | tr '\n' ' ' | tr -s ' ')"
MISSING=0
for n in "${NORMS[@]}"; do
  case "${CORPUS}" in
    *"$n"*) : ;;
    *) echo "      ↳ пропала норма: «${n}»"; MISSING=$((MISSING + 1)) ;;
  esac
done
if [ "${MISSING}" -eq 0 ]; then
  pass "D все ${#NORMS[@]} контрольных норм на месте"
else
  fail "D чистка снесла ${MISSING} норм(ы) из ${#NORMS[@]} — вырезана НОРМА, а не история"
fi

echo "--- F/C/G: RED-пробы гейтов (число СЧИТАЕТСЯ пробой, не заявляется здесь) ---"
for probe in red_docs_freeze:F red_context_budgets:C red_gate_meta:G; do
  p="${probe%%:*}"; tag="${probe##*:}"
  if bash "scripts/tests/${p}.sh" >/tmp/m60-${p}.log 2>&1; then
    pass "${tag} ${p}: $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' /tmp/m60-${p}.log | head -1)"
  else
    fail "${tag} ${p} КРАСНАЯ — $(grep -E '^(VERDICT|SETUP)' /tmp/m60-${p}.log | head -1)"
    grep -E '^FAIL' /tmp/m60-${p}.log | head -5 | sed 's/^/      ↳ /'
  fi
done

echo "--- S: САМОРЕФЕРЕНЦИЯ — M-60 обязан пройти собственный замок ---"
# Механизм, не проходящий сам себя, не готов (C-062 §d). Диапазон — вся ветка над main.
if [ -f scripts/check_docs_freeze.sh ]; then
  BASE=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
  if [ -n "${BASE}" ] && EVENT_NAME=push PUSH_BEFORE="${BASE}" PR_BASE_SHA="${BASE}" \
       bash scripts/check_docs_freeze.sh >/dev/null 2>&1; then
    pass "S собственный диф M-60 проходит замок (токен founder'а на месте)"
  else
    fail "S собственный диф M-60 НЕ проходит замок — механизм не выполняет себя"
  fi
else
  fail "S замка нет — самореференцию проверять нечем"
fi

echo "--- W: ПРОВОДКА в CI (грепом И прогоном: файл в репо ≠ механизм на пути) ---"
CI=.github/workflows/ci.yml
for s in check_docs_freeze check_context_budgets check_gate_meta verify_design_claims; do
  grep -q "${s}" "${CI}" 2>/dev/null && pass "W ${s} зовётся из ci.yml" \
                                     || fail "W ${s} НЕ зовётся из ci.yml — механизм построен, но не подключён"
done
for probe in red_docs_freeze red_context_budgets red_gate_meta red_verify_design_claims; do
  grep -q "${probe}" "${CI}" 2>/dev/null && pass "W проба ${probe} в ci.yml" \
                                         || fail "W проба ${probe} НЕ в ci.yml — гейт без анти-плацебо"
done
if grep -A3 'status-check' "${CI}" 2>/dev/null | grep -q 'needs'; then
  NEEDS=$(grep -A3 'needs:' "${CI}" | grep -oE '\[.*\]' | head -1)
  case "${NEEDS}" in
    *docs-freeze*|*context-budgets*|*gate-meta*) pass "W новые джобы в status-check.needs" ;;
    *) fail "W новые джобы вне status-check.needs — красное не блокирует merge: ${NEEDS}" ;;
  esac
else
  fail "W status-check.needs не найден"
fi

echo "--- P: РЕГРЕСС — соседний барьер артефактов цел ---"
if bash scripts/tests/red_protected_artifacts.sh >/tmp/m60-prot.log 2>&1; then
  pass "P $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' /tmp/m60-prot.log | head -1)"
else
  fail "P барьер артефактов сломан этим milestone'ом — цена чистки уплачена соседним инвариантом"
fi

echo "--- T: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
cargo fmt --all -- --check >/dev/null 2>&1 && pass "T fmt" || fail "T fmt --check"
cargo clippy --workspace --all-targets --all-features -- -D warnings >/tmp/m60-clippy.log 2>&1 \
  && pass "T clippy" || { fail "T clippy"; tail -5 /tmp/m60-clippy.log | sed 's/^/      ↳ /'; }
cargo test --all >/tmp/m60-test.log 2>&1 \
  && pass "T cargo test --all" || { fail "T cargo test --all"; grep -E '^test .* FAILED|^error' /tmp/m60-test.log | head -5 | sed 's/^/      ↳ /'; }

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений)"
  exit 1
fi
echo "VERDICT: PASS"
