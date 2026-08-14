#!/usr/bin/env bash
# Acceptance-гейт M-60c — возврат корпуса правил к бюджету A-003.
# Спека: milestones/M-60c-corpus-cleanup.md §7.
#
# ⚠ СЕЙЧАС КРАСЕН ПО ПОСТРОЕНИЮ — И ОБЯЗАН БЫТЬ КРАСНЫМ против нетронутого корпуса
# (спека §8а): шаг B — 1069 > 725 и 100 > 70 арифметикой; шаг M — секции ещё не переехали
# в профиль; шаг Dd — дубль замера жив в трёх файлах; шаг A — архива не существует;
# шаг T — строк G4/G5 в шаблонах нет. Зеленеет ТОЛЬКО исполнением чистки по спеке.
# Любая правка этого файла ради зелени без чистки — дефект класса «анти-плацебо», не фикс.
#
# Решение по КОДУ ВОЗВРАТА (gates.md §3). Агрегатор со счётчиком: печатаем все нарушения
# разом, exit 1 при FAIL>0.
#
# ПОРЯДОК ВАЖЕН. Шаг B (бюджет) и шаг D (нормы на месте) — ДВЕ ПОЛОВИНЫ одной проверки
# чистки: B доказывает, что вырезано достаточно, D — что вырезана ИСТОРИЯ, а не НОРМА.
# По отдельности каждая половина ложна: пройти B можно, снеся требования, а пройти D —
# не сократив ничего (R-032 поймал ровно первое: с таксономией §9 выпало требование
# risk-critic на документы safety-пути).

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

RULES_BUDGET=725      # цель A-003; спека §1
CLAUDE_BUDGET=70

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

echo "--- D: НОРМЫ НА МЕСТЕ — посекционно, спека §5 (вторая половина) ---"
# Корпус НОРМАЛИЗУЕТСЯ перед поиском: переносы строк → пробелы, пробелы схлопываются —
# приём verify_M-60.sh:67-72 ветки f0e915b (единственное, что взято из зонтичного verify).
# Причина найдена прогоном того же гейта: фраза «не делегируется никому» разорвана
# переносом, а grep работает построчно — норма на месте, проверка красная. Построчный
# вариант ломался бы от любой ПЕРЕВЁРСТКИ абзаца — то есть именно от того, что делает
# чистка. Вечно-красный шаг объявляют шумом и выключают — так защита исчезает не отменой,
# а раздражением.
# Каждая фраза валидирована грепом по ЖИВОМУ корпусу при написании гейта (2026-08-14):
# формулировки, которой нет, здесь быть не может — она дала бы вечно-красный шаг.
# Секции testing.md, ПЕРЕНОСИМЫЕ в профиль architect'а, стережёт шаг M (обе половины
# переноса), не этот список — здесь только ОСТАЮЩЕЕСЯ в впрыскиваемом ядре.
NORMS=(
  # gates.md §0 / §0.1
  "Два REJECT подряд по одной и той же причине"
  "обязательно к исполнению обеими сторонами"
  "не делегируется никому, включая арбитра"
  "арбитр решает «как ПРАВИЛЬНО»"
  # gates.md §1
  "Поднимается до сильной, как risk-critic"
  # gates.md §4
  "АРТЕФАКТ, а не сообщение"
  "Reviewer не пропускается НИКОГДА"
  "НЕ проектирует фикс"
  "built-not-wired"
  # gates.md §8
  "Push — не конец цикла"
  "RED до реализации не живёт в"
  "Intra-chain push на feat-ветку обязателен"
  "--merge-preview origin/main"
  "Push-scope и ветки"
  # gates.md §9
  "risk-critic обязателен дополнительно"
  "перепроверка §9 вердикта критика НЕ заменяет"
  "ветка пушится НЕМЕДЛЕННО"
  # gates.md §11
  "FOUNDER-APPROVED"
  "проверка ПОКОММИТНАЯ"
  "аудит-след, а НЕ подпись"
  "COGNITIVE-ONLY"
  # gates.md §12
  "next_artifact_id.sh"
  # gates.md §3
  "по КОДУ ВОЗВРАТА, а не по тексту вывода"
  "гейт, который зеленее CI"
  # testing.md — остающееся
  "НИКОГДА не включает написание тестов"
  "GREEN против заглушки"
  "Мутационный контроль"
  "ЗАПРЕТНЫЙ СПИСОК"
  # branch-hygiene.md
  "Ветка milestone'а — общая, чекаут — свой"
  "доказывай МОЛЧАНИЕ"
  "Индекс проверяется ДО коммита, диф — ПОСЛЕ"
  "commit -a\` / \`add -A\` запрещены"
  # commit-discipline.md
  "Сырой ≠ ВЕСЬ"
  "минимум один коммит"
  "меткой в конце subject"
  # handoff-block.md
  "последняя секция ответа"
  "кэш убран"
  # CLAUDE.md
  "ВЫПОЛНЯЕТСЯ ПЕРВЫМ"
  "LLM НЕ в горячем торговом цикле"
  "RiskApproved"
)
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

echo "--- M: перенос носителя — ОБЕ половины (спека §3 п.1) ---"
# Секция обязана ПОЯВИТЬСЯ в профиле architect'а И ИСЧЕЗНУТЬ из testing.md: перенос,
# а не копия (копии расходятся) и не удаление (норма умирает).
PROFILE_NORM="$(tr '\n' ' ' < .claude/agents/architect.md 2>/dev/null | tr -s ' ')"
TESTING_NORM="$(tr '\n' ' ' < .claude/rules/testing.md 2>/dev/null | tr -s ' ')"
MOVE_SECTIONS=(
  "Дегенерированный вход обязателен"
  "Форма прода снимается ЗАМЕРОМ"
  "мерить ТО, ЧТО ОБЕЩАЕТ"
  "Целостность гейта — 4 свойства"
  "RED сравнения ДВУХ источников"
)
for s in "${MOVE_SECTIONS[@]}"; do
  IN_PROFILE=no; IN_TESTING=no
  case "${PROFILE_NORM}" in *"$s"*) IN_PROFILE=yes;; esac
  case "${TESTING_NORM}" in *"$s"*) IN_TESTING=yes;; esac
  if [ "${IN_PROFILE}" = yes ] && [ "${IN_TESTING}" = no ]; then
    pass "M «${s}» переехала: в профиле есть, в testing.md нет"
  elif [ "${IN_PROFILE}" = yes ]; then
    fail "M «${s}» СКОПИРОВАНА, а не перенесена — два носителя разойдутся"
  else
    fail "M «${s}» не переехала в профиль architect'а (в профиле нет)"
  fi
done

echo "--- Dd: дедуп замера «782 MB / 105 GB» (спека §3 п.2) ---"
DUP="$(grep -rln '782 MB\|105 GB' CLAUDE.md .claude/rules/ 2>/dev/null || true)"
if [ -z "${DUP}" ]; then
  pass "Dd замер живёт только в комментарии gc_worktrees.sh"
else
  fail "Dd дубль замера в впрыскиваемом ядре: $(echo "${DUP}" | tr '\n' ' ')"
fi
grep -q '782 MB' scripts/gc_worktrees.sh 2>/dev/null \
  && pass "Dd канонический носитель замера (gc_worktrees.sh) существует" \
  || fail "Dd замер исчез И из gc_worktrees.sh — это уже не дедуп, а потеря"

echo "--- A: архив вырезанного (спека §3, хвост) ---"
if [ -s docs/archive/rules-history-2026-08.md ]; then
  pass "A docs/archive/rules-history-2026-08.md существует и непуст"
else
  fail "A архива нет или пуст — вырезанное должно переезжать, а не исчезать"
fi

echo "--- T: строки G4/G5 в шаблонах handoff-block.md (спека §4; COGNITIVE-ONLY) ---"
# Греп здесь ЗАКОНЕН: предмет — сам ТЕКСТ шаблона, а не подключённость механизма
# (запрет F-064-2 — о втором).
grep -q 'AHEAD=' .claude/rules/handoff-block.md \
  && pass "T G4: предикат AHEAD= в шаблоне §D" \
  || fail "T G4: предиката AHEAD= в шаблоне §D нет"
grep -q 'SKIPPED' .claude/rules/handoff-block.md \
  && pass "T G5: строка REQUIRED|SKIPPED в шаблоне §B" \
  || fail "T G5: строки прозрачности гейтинга критика в шаблоне §B нет"

echo "--- S: самореференция — диф ветки проходит замок §11 ---"
MB="$(git merge-base origin/main HEAD 2>/dev/null || true)"
if [ -n "${MB}" ]; then
  if ( EVENT_NAME=push PUSH_BEFORE="${MB}" bash scripts/check_docs_freeze.sh >/dev/null 2>&1 ); then
    pass "S замок §11 зелёный на диапазоне ${MB:0:7}..HEAD (токены на месте)"
  else
    fail "S замок §11 КРАСНЫЙ — коммит чистки без FOUNDER-APPROVED в зоне замка"
  fi
else
  fail "S merge-base с origin/main не вычислился — прогон не на дереве репо?"
fi

echo "--- P: регресс соседних барьеров (счёт — из счётчика самой пробы, не из шапки) ---"
for p in red_protected_artifacts red_docs_freeze red_commit_paths; do
  LOG="$(mktemp /tmp/verify-m60c-XXXXXX.log)"
  if bash "scripts/tests/${p}.sh" >"${LOG}" 2>&1; then
    N="$(grep -oE 'VERDICT: PASS \(([0-9]+)/' "${LOG}" | grep -oE '[0-9]+' | head -1 || true)"
    if [ -n "${N}" ] && [ "${N}" -ge 1 ]; then
      pass "P ${p}: зелёная (${N} исполнено)"
    else
      fail "P ${p}: зелёная, но исполнено «${N:-0}» — пустой прогон (урок M-60a: PASS (0/0))"
    fi
  else
    fail "P ${p}: КРАСНАЯ — чистка сломала соседний барьер"
    grep -E '^(FAIL|SETUP)' "${LOG}" | head -4 | sed 's/^/      ↳ /'
  fi
  rm -f "${LOG}"
done

echo "--- CI-паритет: базовый джоб целиком (gates.md §3) ---"
cargo fmt --all -- --check >/dev/null 2>&1 \
  && pass "CI cargo fmt --check" || fail "CI cargo fmt --check"
cargo clippy --all-targets --all-features -- -D warnings >/dev/null 2>&1 \
  && pass "CI cargo clippy -D warnings" || fail "CI cargo clippy -D warnings"
cargo test --all >/dev/null 2>&1 \
  && pass "CI cargo test --all" || fail "CI cargo test --all"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  exit 1
fi
echo "VERDICT: PASS"
