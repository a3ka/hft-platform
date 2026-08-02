#!/usr/bin/env bash
# RED-проба для scripts/verify_design_claims.sh — анти-плацебо self-test.
#
# ЗАЧЕМ. Задача verify_design_claims.sh — ловить ровно ЧЕТЫРЕ класса лжи, которые
# docs/DESIGN.md уже допустил за одни сутки (docs/ORCHESTRATION-STATE.md): [ЕСТЬ] на
# несуществующий путь, покрытие инвариантов завышено, покрытие занижено, битая ссылка на
# раздел, ссылка на удалённый файл. Гейт, не проверенный на срабатывание на КАЖДОМ из этих
# классов, — сам заглушка (testing.md «Анти-плацебо»). Плюс контрольный PASS на корректном
# документе — гейт, который падает всегда, так же бесполезен, как гейт, который не падает
# никогда.
#
# Каждый сценарий строит СИНТЕТИЧЕСКУЮ копию мини-репозитория во временном каталоге (НЕ
# трогает реальный docs/DESIGN.md) и зовёт РЕАЛЬНЫЙ scripts/verify_design_claims.sh с этим
# каталогом как ROOT — тот же вызов, каким его гоняет reviewer/CI.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/verify_design_claims.sh}"

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

TMP_BASE="$(mktemp -d)"
cleanup() { rm -rf "${TMP_BASE}"; }
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Общий каркас фикстуры: минимальный "хороший" мини-репозиторий, который сам по себе
# обязан давать VERDICT: PASS. Каждый BAD-сценарий начинается с копии этого каркаса и
# портит РОВНО ОДНУ вещь.
#
# §1 (проза) и §2 (ASCII-схема во фенсе) НАРОЧНО содержат `[ЕСТЬ]` БЕЗ какого-либо пруфа
# рядом — это ровно те ложные срабатывания (docs/design-evolution, 12 FAIL), которые
# уточнение 2026-08-01 обязано было устранить. Если check1 регрессирует к "пруф нужен
# везде", scenario_good упадёт из-за именно этих строк.
#
# §3 — таблица статусов с ЧЕТЫРЬМЯ равноправными формами пруфа (путь/milestone/вердикт-
# critique/вердикт-review), каждая — существующая. Bad-сценарии портят их по одной.
# ---------------------------------------------------------------------------
build_good_fixture() { # $1 = каталог назначения
  local d="$1"
  mkdir -p "${d}/docs" "${d}/crates/foo/tests" "${d}/crates/xx/tests" "${d}/milestones" \
           "${d}/research/critiques" "${d}/research/reviews"

  cat > "${d}/docs/OTHER.md" <<'EOF'
# Другой документ
Существует, на него можно ссылаться.
EOF

  cat > "${d}/crates/xx/tests/red_xx.rs" <<'EOF'
//! RED-оракулы XX-I-1 и XX-I-2 (sacred).
#[test]
fn xx_i_1_something_holds() {
    assert!(true, "XX-I-1: инвариант держится");
}

#[test]
fn xx_i_2_something_else_holds() {
    assert!(true, "XX-I-2: инвариант держится");
}
EOF

  cat > "${d}/milestones/M-01-bar.md" <<'EOF'
# M-01 — bar
STATUS: ✅ DONE
EOF

  cat > "${d}/research/critiques/C-01-bar.md" <<'EOF'
# C-01 — Critic Verdict — bar
Вердикт: PASS.
EOF

  cat > "${d}/research/reviews/R-01-bar.md" <<'EOF'
# R-01 — PR-гейт bar
Вердикт: APPROVED.
EOF

  cat > "${d}/docs/DESIGN.md" <<'EOF'
# Test Design Doc

## §0. Тезис
Вводный раздел, ссылается на `docs/OTHER.md` для деталей.

## §1. Компонент foo (проза)
Компонент foo реализован и работает [ЕСТЬ] — как было описано в §0. Пруф рядом не нужен,
это отсылка к уже сказанному, не отдельное нормативное утверждение.

## §2. Схема потока данных (ASCII, во фенсе)

```
┌────────────────┐
│ приёмник WS [ЕСТЬ]  │──┐
└────────────────┘        │
  нормализация событий [ЕСТЬ]
```

## §3. Таблица статусов компонентов

| Компонент | Пруф | Статус |
|---|---|---|
| foo | `crates/foo` | [ЕСТЬ] |
| bar | M-01 | [ЕСТЬ] |
| baz | C-01 | [ЕСТЬ] |
| qux | R-01 | [ЕСТЬ] |

## §10. Фазовый роадмап

| Фаза | Содержимое | Ворота | founder |
|---|---|---|---|
| P0 тест | заглушка | — | — |

## §22. Инварианты платформы

| Семейство | Зона | Заявлено | В оракулах | Статус |
|---|---|---|---|---|
| XX-I | тестовая зона | 2 | 2 | [ЧАСТИЧНО] |

См. `DESIGN.md §0` и `DESIGN.md §1` выше.
EOF

  git -C "${d}" init -q 2>/dev/null || true
}

run_verify() { # $1 = fixture dir → печатает stdout, возвращает exit-код в $?
  bash "${BARRIER}" "$1"
}

# ---------------------------------------------------------------------------
# Сценарий 0 — корректный документ → PASS (контроль против «гейт падает всегда»)
# ---------------------------------------------------------------------------
scenario_good() {
  local d="${TMP_BASE}/good"
  build_good_fixture "${d}"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && echo "${out}" | grep -q '^VERDICT: PASS'; then
    pass "сценарий 0 (корректный документ): гейт даёт VERDICT: PASS, exit=0"
  else
    fail "сценарий 0 (корректный документ): ОЖИДАЛСЯ PASS, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 1 — [ЕСТЬ] в ТАБЛИЦЕ СТАТУСОВ с несуществующим путём → FAIL [1-ЕСТЬ]
# ---------------------------------------------------------------------------
scenario_bad_est_missing_path() {
  local d="${TMP_BASE}/bad1"
  build_good_fixture "${d}"
  sed -i 's#| foo | `crates/foo` | \[ЕСТЬ\] |#| foo | `crates/does-not-exist` | [ЕСТЬ] |#' "${d}/docs/DESIGN.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[1-ЕСТЬ\].*does-not-exist.*ОТСУТСТВУЕТ'; then
    pass "сценарий 1 ([ЕСТЬ] в таблице на несуществующий путь): гейт даёт FAIL [1-ЕСТЬ], exit=${rc}"
  else
    fail "сценарий 1 ([ЕСТЬ] в таблице на несуществующий путь): ОЖИДАЛСЯ FAIL [1-ЕСТЬ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 1b — [ЕСТЬ] в ТАБЛИЦЕ СТАТУСОВ вовсе без пруфа рядом → FAIL [1-ЕСТЬ] «не
# проверяемо» (строгость таблиц статусов ОБЯЗАНА остаться, это не то, что уточнение снимает)
# ---------------------------------------------------------------------------
scenario_bad_est_no_path() {
  local d="${TMP_BASE}/bad1b"
  build_good_fixture "${d}"
  sed -i 's#| foo | `crates/foo` | \[ЕСТЬ\] |#| foo | — | [ЕСТЬ] |#' "${d}/docs/DESIGN.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[1-ЕСТЬ\].*таблице статусов без пруфа'; then
    pass "сценарий 1b ([ЕСТЬ] в таблице без пруфа вовсе): гейт даёт FAIL [1-ЕСТЬ] «не проверяемо», exit=${rc}"
  else
    fail "сценарий 1b ([ЕСТЬ] в таблице без пруфа вовсе): ОЖИДАЛСЯ FAIL [1-ЕСТЬ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 1c — [ЕСТЬ] в ТАБЛИЦЕ СТАТУСОВ, пруф-milestone `M-NN`, которого НЕ существует
# → FAIL [1-ЕСТЬ] (форма пруфа "milestone" равноправна пути, но так же проверяется замером)
# ---------------------------------------------------------------------------
scenario_bad_est_milestone_missing() {
  local d="${TMP_BASE}/bad1c"
  build_good_fixture "${d}"
  sed -i 's#| bar | M-01 | \[ЕСТЬ\] |#| bar | M-999 | [ЕСТЬ] |#' "${d}/docs/DESIGN.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[1-ЕСТЬ\].*milestone M-999.*ОТСУТСТВУЕТ'; then
    pass "сценарий 1c ([ЕСТЬ] в таблице, milestone-пруф не существует): гейт даёт FAIL [1-ЕСТЬ], exit=${rc}"
  else
    fail "сценарий 1c ([ЕСТЬ] в таблице, milestone-пруф не существует): ОЖИДАЛСЯ FAIL [1-ЕСТЬ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 1d — [ЕСТЬ] в ТАБЛИЦЕ СТАТУСОВ, пруф-вердикт `C-NNN` (critique), которого НЕ
# существует → FAIL [1-ЕСТЬ]
# ---------------------------------------------------------------------------
scenario_bad_est_critique_missing() {
  local d="${TMP_BASE}/bad1d"
  build_good_fixture "${d}"
  sed -i 's#| baz | C-01 | \[ЕСТЬ\] |#| baz | C-999 | [ЕСТЬ] |#' "${d}/docs/DESIGN.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[1-ЕСТЬ\].*вердикт C-999.*ОТСУТСТВУЕТ'; then
    pass "сценарий 1d ([ЕСТЬ] в таблице, critique-пруф не существует): гейт даёт FAIL [1-ЕСТЬ], exit=${rc}"
  else
    fail "сценарий 1d ([ЕСТЬ] в таблице, critique-пруф не существует): ОЖИДАЛСЯ FAIL [1-ЕСТЬ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 1e — [ЕСТЬ] в ТАБЛИЦЕ СТАТУСОВ, пруф-вердикт `R-NNN` (review), которого НЕ
# существует → FAIL [1-ЕСТЬ]
# ---------------------------------------------------------------------------
scenario_bad_est_review_missing() {
  local d="${TMP_BASE}/bad1e"
  build_good_fixture "${d}"
  sed -i 's#| qux | R-01 | \[ЕСТЬ\] |#| qux | R-999 | [ЕСТЬ] |#' "${d}/docs/DESIGN.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[1-ЕСТЬ\].*вердикт R-999.*ОТСУТСТВУЕТ'; then
    pass "сценарий 1e ([ЕСТЬ] в таблице, review-пруф не существует): гейт даёт FAIL [1-ЕСТЬ], exit=${rc}"
  else
    fail "сценарий 1e ([ЕСТЬ] в таблице, review-пруф не существует): ОЖИДАЛСЯ FAIL [1-ЕСТЬ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 1f — [ЕСТЬ] в ASCII-схеме БЕЗ пруфа → PASS (ложное срабатывание устранено).
# Уже верно и в scenario_good (там же живёт схема), но здесь — точечная проверка ИМЕННО
# отсутствия FAIL по этим строкам, чтобы регрессия "пруф снова нужен везде" ловилась явно.
# ---------------------------------------------------------------------------
scenario_est_ascii_schema_no_proof_passes() {
  local d="${TMP_BASE}/good_schema_check"
  build_good_fixture "${d}"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'FAIL  \[1-ЕСТЬ\].*приёмник WS\|FAIL  \[1-ЕСТЬ\].*нормализация событий'; then
    pass "сценарий 1f ([ЕСТЬ] в ASCII-схеме без пруфа): гейт НЕ падает на схему, exit=${rc}"
  else
    fail "сценарий 1f ([ЕСТЬ] в ASCII-схеме без пруфа): ОЖИДАЛСЯ PASS (без FAIL по строкам схемы), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 1g — [ЕСТЬ] в ПРОЗЕ БЕЗ пруфа → PASS (ложное срабатывание устранено).
# ---------------------------------------------------------------------------
scenario_est_prose_no_proof_passes() {
  local d="${TMP_BASE}/good_prose_check"
  build_good_fixture "${d}"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'FAIL  \[1-ЕСТЬ\].*как было описано'; then
    pass "сценарий 1g ([ЕСТЬ] в прозе без пруфа): гейт НЕ падает на прозу, exit=${rc}"
  else
    fail "сценарий 1g ([ЕСТЬ] в прозе без пруфа): ОЖИДАЛСЯ PASS (без FAIL по прозе), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 2 — покрытие ЗАВЫШЕНО против грепа → FAIL [2-ПОКРЫТИЕ]
# ---------------------------------------------------------------------------
scenario_bad_coverage_overstated() {
  local d="${TMP_BASE}/bad2"
  build_good_fixture "${d}"
  # в тестах реально 2 (XX-I-1, XX-I-2); документ заявит 3 — завышение (класс C-042 F-1)
  sed -i 's#| XX-I | тестовая зона | 2 | 2 | \[ЧАСТИЧНО\] |#| XX-I | тестовая зона | 3 | 3 | [ЧАСТИЧНО] |#' "${d}/docs/DESIGN.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q "FAIL  \[2-ПОКРЫТИЕ\].*XX-I.*заявляет 'в оракулах'=3.*strict=2"; then
    pass "сценарий 2 (покрытие завышено): гейт даёт FAIL [2-ПОКРЫТИЕ], exit=${rc}"
  else
    fail "сценарий 2 (покрытие завышено): ОЖИДАЛСЯ FAIL [2-ПОКРЫТИЕ] заявлено=3/реально=2, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 3 — покрытие ЗАНИЖЕНО против грепа → FAIL [2-ПОКРЫТИЕ] (класс C-041 Ф1)
# ---------------------------------------------------------------------------
scenario_bad_coverage_understated() {
  local d="${TMP_BASE}/bad3"
  build_good_fixture "${d}"
  # добавляем ТРЕТИЙ реально привязанный тест XX-I-3, но документ по-прежнему пишет "2"
  cat >> "${d}/crates/xx/tests/red_xx.rs" <<'EOF'

#[test]
fn xx_i_3_third_invariant_holds() {
    assert!(true, "XX-I-3: инвариант держится");
}
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q "FAIL  \[2-ПОКРЫТИЕ\].*XX-I.*заявляет 'в оракулах'=2.*strict=3"; then
    pass "сценарий 3 (покрытие занижено): гейт даёт FAIL [2-ПОКРЫТИЕ], exit=${rc}"
  else
    fail "сценарий 3 (покрытие занижено): ОЖИДАЛСЯ FAIL [2-ПОКРЫТИЕ] заявлено=2/реально=3, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 3b — анти-плацебо C-042 F-1: упоминание в ЧУЖОМ (не домашнем) крейте не считается
# оракулом для RK-I, даже без слова-маркера аналогии.
# ---------------------------------------------------------------------------
scenario_bad_rk_foreign_crate_not_counted() {
  local d="${TMP_BASE}/bad3b"
  build_good_fixture "${d}"
  mkdir -p "${d}/crates/foreign/tests"
  cat > "${d}/crates/foreign/tests/red_foreign.rs" <<'EOF'
//! Комментарий-аналогия в ЧУЖОМ крейте: упоминает RK-I-1, но это НЕ risk/killswitch.
#[test]
fn foreign_test_mentions_rk_i_1_by_analogy() {
    assert!(true, "прямой предок будущего RK-I-1, но это не тот крейт");
}
EOF
  cat >> "${d}/docs/DESIGN.md" <<'EOF'

| RK-I | риск (execution) | 10 | 0 | PENDING |
EOF
  # вставляем строку RK-I в таблицу §22 (после заголовка/разделителя таблицы уже есть XX-I —
  # добавляем через sed, чтобы остаться внутри той же markdown-таблицы)
  python3 - "${d}/docs/DESIGN.md" <<'PYEOF'
import sys
p = sys.argv[1]
text = open(p, encoding="utf-8").read()
text = text.replace(
    "| XX-I | тестовая зона | 2 | 2 | [ЧАСТИЧНО] |\n\nСм.",
    "| XX-I | тестовая зона | 2 | 2 | [ЧАСТИЧНО] |\n| RK-I | риск (execution) | 10 | 0 | PENDING |\n\nСм.",
)
open(p, "w", encoding="utf-8").write(text)
PYEOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  # RK-I: документ заявляет 0, реальных оракулов в crates/risk|killswitch тоже 0 (крейтов
  # нет вовсе) — упоминание в crates/foreign НЕ должно засчитаться → PASS для RK-I,
  # и это ДОЛЖНО оставаться PASS (иначе анти-плацебо C-042 F-1 не работает).
  if [ "${rc}" -eq 0 ] && echo "${out}" | grep -q "PASS  \[2-ПОКРЫТИЕ\].*RK-I.*в оракулах=0"; then
    pass "сценарий 3b (анти-плацебо C-042: чужой крейт не считается оракулом RK-I): гейт корректно НЕ засчитал упоминание, exit=${rc}"
  else
    fail "сценарий 3b (анти-плацебо C-042): ОЖИДАЛСЯ PASS [2-ПОКРЫТИЕ] RK-I в оракулах=0 (упоминание в чужом крейте не считается), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 4 — ссылка §99 на несуществующий раздел → FAIL [3-ССЫЛКИ]
# ---------------------------------------------------------------------------
scenario_bad_broken_section_ref() {
  local d="${TMP_BASE}/bad4"
  build_good_fixture "${d}"
  # ВАЖНО: ссылка строится через printf из отдельных кусков (не литеральным текстом
  # "DESIGN.md" + "§" + номер подряд нигде в этом файле) — иначе CHECK 3 самого
  # verify_design_claims.sh, сканируя РЕАЛЬНЫЙ репозиторий (в котором лежит этот .sh-файл
  # как обычный committed-файл), нашёл бы этот пример как настоящую битую ссылку на
  # несуществующий раздел (self-referential ложный FAIL при прогоне гейта на самом себе).
  # Тот же приём применён и здесь, в комментарии, — намеренно не пишем магическую строку
  # рядом друг с другом.
  local fake_section="99"
  printf 'Ссылка на несуществующий раздел: DESIGN.md §%s.\n' "${fake_section}" >> "${d}/milestones/M-01-fake.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[3-ССЫЛКИ\].*§99.*нет в оглавлении'; then
    pass "сценарий 4 (битая ссылка §99): гейт даёт FAIL [3-ССЫЛКИ], exit=${rc}"
  else
    fail "сценарий 4 (битая ссылка §99): ОЖИДАЛСЯ FAIL [3-ССЫЛКИ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 5 — ссылка на удалённый файл docs/*.md → FAIL [4-МЁРТВЫЕ-ФАЙЛЫ]
# ---------------------------------------------------------------------------
scenario_bad_dead_file_ref() {
  local d="${TMP_BASE}/bad5"
  build_good_fixture "${d}"
  cat >> "${d}/docs/DESIGN.md" <<'EOF'

Подробности были в `docs/GHOST.md` (файл удалён при миграции, ссылка осталась).
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[4-МЁРТВЫЕ-ФАЙЛЫ\].*docs/GHOST\.md.*не существует'; then
    pass "сценарий 5 (ссылка на удалённый файл): гейт даёт FAIL [4-МЁРТВЫЕ-ФАЙЛЫ], exit=${rc}"
  else
    fail "сценарий 5 (ссылка на удалённый файл): ОЖИДАЛСЯ FAIL [4-МЁРТВЫЕ-ФАЙЛЫ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 6 — §10 объявляет фазу пройденной, а её milestone отсутствует → FAIL [5-ФАЗЫ]
# (класс R-011 Б-1: фаза объявлена пройденной, а её ворота — нет)
# ---------------------------------------------------------------------------
scenario_bad_phase_milestone_missing() {
  local d="${TMP_BASE}/bad6"
  build_good_fixture "${d}"
  sed -i 's#| P0 тест | заглушка | — | — |#| P0 тест ✅ | заглушка, цитирует M-77 | пройдено | — |#' "${d}/docs/DESIGN.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[5-ФАЗЫ\].*M-77.*отсутствует'; then
    pass "сценарий 6 (фаза пройдена, milestone отсутствует): гейт даёт FAIL [5-ФАЗЫ], exit=${rc}"
  else
    fail "сценарий 6 (фаза пройдена, milestone отсутствует): ОЖИДАЛСЯ FAIL [5-ФАЗЫ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 7 — setup-guard: docs/DESIGN.md отсутствует вовсе → FAIL [SETUP], не тихий PASS
# ---------------------------------------------------------------------------
scenario_bad_setup_guard_missing_design() {
  local d="${TMP_BASE}/bad7"
  mkdir -p "${d}/docs"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[SETUP\].*docs/DESIGN.md не найден'; then
    pass "сценарий 7 (setup-guard: DESIGN.md отсутствует): гейт даёт FAIL [SETUP], exit=${rc}"
  else
    fail "сценарий 7 (setup-guard: DESIGN.md отсутствует): ОЖИДАЛСЯ FAIL [SETUP], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 8 — --merge-preview (R-013): документ ПРАВДИВ на ветке (HEAD) и ЛОЖЕН после
# слияния с base-ref, потому что новый оракул приземлился в base ПОКА ветка отделялась.
# Класс ровно тот, что поймал R-013 на JR-I (M-50 добавил JR-I-9 в main, пока
# docs/design-evolution ждала гейтов): счётчик покрытия, верный для ветки, стал ложным
# в дереве, куда документ реально едет.
#
# Анти-плацебо (testing.md): линии ОБЯЗАНЫ по-настоящему РАСХОДИТЬСЯ, иначе тест не отличит
# реальное слияние от суррогата «--merge-preview втихую смотрит только на одну сторону».
# Общий предок — ПОЧТИ пустой репозиторий (только .gitkeep, БЕЗ docs/DESIGN.md и БЕЗ
# оракулов). От него — два независимых потомка:
#   HEAD (branch-tip)  — полная good-фикстура (build_good_fixture): DESIGN.md ЕСТЬ,
#                         XX-I: заявлено=2, оракулов=2 — ПРАВДА для ЭТОЙ ветки.
#   base_main          — НЕ содержит DESIGN.md вовсе, добавляет ТОЛЬКО новый файл-оракул
#                         XX-I-3 (имитация main, ушедшего вперёд независимо).
# Три возможных исхода различимы:
#   • обычный режим (родное рабочее дерево = HEAD)              → VERDICT: PASS
#   • плацебо «--merge-preview смотрит только на base, не мержит HEAD» → FAIL [SETUP]
#     «docs/DESIGN.md не найден» (в base его вообще нет) — ДРУГОЙ класс ошибки
#   • настоящее слияние (--merge-preview base_main)              → FAIL [2-ПОКРЫТИЕ]
#     claimed=2/strict=3 — оракул XX-I-3 из base ВМЕСТЕ с XX-I-1/2 и DESIGN.md из HEAD.
# Тест проверяет ИМЕННО третий исход — если реализация врёт и даёт один из первых двух,
# сценарий обязан упасть.
# ---------------------------------------------------------------------------
scenario_merge_preview_catches_branch_vs_merge_drift() {
  local d="${TMP_BASE}/mergeprev8"
  mkdir -p "${d}"
  git -C "${d}" init -q
  : > "${d}/.gitkeep"
  git -C "${d}" add -A
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "общий предок: почти пусто, без DESIGN.md и без оракулов"
  local ancestor
  ancestor="$(git -C "${d}" rev-parse HEAD)"

  # HEAD (branch-tip): полная good-фикстура поверх предка.
  build_good_fixture "${d}"
  git -C "${d}" add -A
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "branch tip: XX-I=2, правда здесь"
  local branch_tip
  branch_tip="$(git -C "${d}" rev-parse HEAD)"

  # base_main: НЕЗАВИСИМАЯ ветка от ТОГО ЖЕ предка (не от branch_tip!) — не содержит
  # DESIGN.md вовсе, добавляет только новый оракул. Реальная divergence, не suffix-commit.
  git -C "${d}" checkout -q -b base_main "${ancestor}"
  mkdir -p "${d}/crates/xx/tests"
  cat > "${d}/crates/xx/tests/red_xx_extra.rs" <<'EOF'
//! Прилетело в base (main) отдельным файлом, ПОКА ветка проходила круги гейтов —
//! ровно класс M-50/JR-I-9 из R-013 (новый оракул в домашнем крейте, ветка о нём не знает).
#[test]
fn xx_i_3_landed_in_base_while_branch_was_in_review() {
    assert!(true, "XX-I-3: третий оракул семейства, появился в base");
}
EOF
  git -C "${d}" add -A
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "base moved on: новый оракул XX-I-3, DESIGN.md здесь вообще нет"

  # вернуться на исходный branch-tip (рабочее дерево = ровно то, что было в build_good_fixture)
  git -C "${d}" checkout -q "${branch_tip}"

  local out_branch rc_branch out_preview rc_preview
  out_branch="$(run_verify "${d}")"; rc_branch=$?
  out_preview="$(bash "${BARRIER}" --merge-preview base_main "${d}")"; rc_preview=$?

  if [ "${rc_branch}" -eq 0 ] && echo "${out_branch}" | grep -q '^VERDICT: PASS' \
     && [ "${rc_preview}" -ne 0 ] \
     && echo "${out_preview}" | grep -q "FAIL  \[2-ПОКРЫТИЕ\].*XX-I.*заявляет 'в оракулах'=2.*strict=3" \
     && ! echo "${out_preview}" | grep -q 'FAIL  \[SETUP\].*docs/DESIGN.md не найден'; then
    pass "сценарий 8 (--merge-preview ловит дрейф ветка↔слияние, дерево РЕАЛЬНО расходится): ветка PASS (exit=${rc_branch}), --merge-preview FAIL [2-ПОКРЫТИЕ] (exit=${rc_preview})"
  else
    fail "сценарий 8 (--merge-preview): ОЖИДАЛСЯ ветка=PASS(0) + merge-preview=FAIL[2-ПОКРЫТИЕ] claimed=2/strict=3 (НЕ setup-guard «DESIGN.md не найден» — иначе слияние не настоящее), получено:"
    echo "      --- обычный режим (exit=${rc_branch}) ---"
    echo "${out_branch}" | sed 's/^/      /'
    echo "      --- --merge-preview base_main (exit=${rc_preview}) ---"
    echo "${out_preview}" | sed 's/^/      /'
  fi

  local wt_count
  wt_count="$(git -C "${d}" worktree list --porcelain | grep -c '^worktree ')"
  if [ "${wt_count}" -eq 1 ]; then
    pass "сценарий 8b (--merge-preview): временный preview-worktree убран после прогона (worktree list = 1, нет утечки)"
  else
    fail "сценарий 8b (--merge-preview): временный preview-worktree НЕ убран после прогона (worktree list = ${wt_count}, ожидался 1):"
    git -C "${d}" worktree list | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарий 9 — --merge-preview: слияние КОНФЛИКТУЕТ → FAIL [SETUP] с объяснением, НЕ
# молчаливый PASS. Обе линии правят ОДНУ И ТУ ЖЕ строку docs/DESIGN.md по-разному.
# ---------------------------------------------------------------------------
scenario_merge_preview_conflict_is_setup_guard_fail() {
  local d="${TMP_BASE}/mergeprev9"
  build_good_fixture "${d}"
  git -C "${d}" add -A
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "branch tip"
  local branch_tip
  branch_tip="$(git -C "${d}" rev-parse HEAD)"

  git -C "${d}" checkout -q -b base_main_conflict "${branch_tip}"
  sed -i 's#Компонент foo реализован и работает#Компонент foo НЕ реализован, работа не начата#' "${d}/docs/DESIGN.md"
  git -C "${d}" commit -qam "base: правит ту же строку §1"

  git -C "${d}" checkout -q "${branch_tip}"
  sed -i 's#Компонент foo реализован и работает#Компонент foo реализован и полностью протестирован#' "${d}/docs/DESIGN.md"
  git -C "${d}" commit -qam "branch: правит ту же строку §1 иначе"

  local out rc
  out="$(bash "${BARRIER}" --merge-preview base_main_conflict "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[SETUP\].*КОНФЛИКТУЕТ' \
     && echo "${out}" | grep -q '^VERDICT: FAIL'; then
    pass "сценарий 9 (--merge-preview: конфликт слияния): гейт даёт FAIL [SETUP] с объяснением, exit=${rc}"
  else
    fail "сценарий 9 (--merge-preview: конфликт слияния): ОЖИДАЛСЯ FAIL [SETUP] «КОНФЛИКТУЕТ», получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
  git -C "${d}" merge --abort >/dev/null 2>&1 || true
}

# ---------------------------------------------------------------------------
# Сценарий 10 — --merge-preview: base-ref не резолвится → FAIL [SETUP], не PASS/крэш
# ---------------------------------------------------------------------------
scenario_merge_preview_bad_base_ref_fails() {
  local d="${TMP_BASE}/mergeprev10"
  build_good_fixture "${d}"
  git -C "${d}" add -A
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "only commit"
  local out rc
  out="$(bash "${BARRIER}" --merge-preview no-such-ref-anywhere "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[SETUP\].*base-ref .no-such-ref-anywhere. не резолвится'; then
    pass "сценарий 10 (--merge-preview: base-ref не резолвится): гейт даёт FAIL [SETUP], exit=${rc}"
  else
    fail "сценарий 10 (--merge-preview: base-ref не резолвится): ОЖИДАЛСЯ FAIL [SETUP], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# RFC-gate self-test (C-044): docs/rfc/**.md SHA-цитаты и path-цитаты проверяются
# машинно против РЕАЛЬНОГО git (объекты коммитов) и РЕАЛЬНОГО дерева фикстуры — не
# гипотетически. Каждый сценарий строит good-фикстуру, коммитит её (чтобы иметь
# существующий SHA под рукой), затем добавляет ОДИН docs/rfc/*.md файл, портящий/
# подтверждающий ровно одну вещь.
# ---------------------------------------------------------------------------
build_rfc_fixture_base() { # $1=dir → коммитит good-фикстуру, печатает SHA коммита в stdout
  local d="$1"
  build_good_fixture "${d}"
  mkdir -p "${d}/docs/rfc"
  git -C "${d}" add -A
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "rfc-gate self-test base"
  git -C "${d}" rev-parse HEAD
}

# --- сценарий: RFC цитирует РЕАЛЬНЫЙ SHA (существующий коммит фикстуры) → PASS [6-RFC-SHA] ---
scenario_rfc_sha_real_passes() {
  local d="${TMP_BASE}/rfc_sha_real"
  local real_sha
  real_sha="$(build_rfc_fixture_base "${d}")"
  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-real.md" <<EOF
# CT-RFC-TEST — реальный SHA
Изменение подтверждено коммитом \`${real_sha}\`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && echo "${out}" | grep -q '^VERDICT: PASS' \
     && echo "${out}" | grep -q 'PASS  \[6-RFC-SHA\]'; then
    pass "сценарий RFC-SHA-real (реальный SHA в docs/rfc/, C-044): гейт даёт PASS [6-RFC-SHA], VERDICT: PASS, exit=${rc}"
  else
    fail "сценарий RFC-SHA-real: ОЖИДАЛСЯ PASS [6-RFC-SHA] + VERDICT: PASS, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- сценарий: RFC цитирует ВЫДУМАННЫЙ SHA (0000000, не существует) → FAIL [6-RFC-SHA] ---
# Класс C-044 F1: §4 CT-RFC-05-margin-inventory.md процитировал 3 из 4 SHA, не входящих в
# ancestry origin/main (орфаны с другого прохода той же задачи на feat-ветке).
scenario_rfc_sha_fake_fails() {
  local d="${TMP_BASE}/rfc_sha_fake"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-fake.md" <<'EOF'
# CT-RFC-TEST — выдуманный SHA
Изменение подтверждено коммитом `0000000`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[6-RFC-SHA\].*`0000000`.*не найден в git-объектах'; then
    pass "сценарий RFC-SHA-fake (выдуманный SHA 0000000 в docs/rfc/, C-044 F1): гейт даёт FAIL [6-RFC-SHA], exit=${rc}"
  else
    fail "сценарий RFC-SHA-fake: ОЖИДАЛСЯ FAIL [6-RFC-SHA] на 0000000, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- анти-плацебо (C-044 F1, РЕАЛЬНЫЙ инцидент): SHA СУЩЕСТВУЕТ как git-объект (коммит на
# отдельной, никогда не смёрженной ветке), но НЕ входит в историю HEAD — ровно класс
# `ffedc10`/`6a2c331`/`67b6159`, реальных объектов на заброшенной ветке `engine/M-35-arms`,
# которые `git cat-file -e` находил, а критик поймал только `git merge-base --is-ancestor`.
# Если бы check6 проверял только существование объекта (как в первой, неверной реализации
# этого файла), этот сценарий давал бы ложный PASS — регресс-тест на именно эту ошибку.
scenario_rfc_sha_orphan_exists_but_not_ancestor_fails() {
  local d="${TMP_BASE}/rfc_sha_orphan"
  local main_sha orphan_sha
  main_sha="$(build_rfc_fixture_base "${d}")"

  git -C "${d}" checkout -q -b orphan-branch
  echo "orphan work, never merged" > "${d}/orphan.txt"
  git -C "${d}" add -A
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "orphan branch commit, never merged into main"
  orphan_sha="$(git -C "${d}" rev-parse HEAD)"
  git -C "${d}" checkout -q "${main_sha}"

  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-orphan.md" <<EOF
# CT-RFC-TEST — orphan SHA (объект существует, HEAD не содержит)
Изменение подтверждено коммитом \`${orphan_sha}\`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q "FAIL  \[6-RFC-SHA\].*\`${orphan_sha}\`.*НЕ входит в историю"; then
    pass "сценарий RFC-SHA-orphan (SHA — реальный git-объект вне ancestry HEAD, C-044 F1 класс): гейт даёт FAIL [6-RFC-SHA], exit=${rc}"
  else
    fail "сценарий RFC-SHA-orphan: ОЖИДАЛСЯ FAIL [6-RFC-SHA] «НЕ входит в историю» (анти-плацебо C-044 F1 — SHA существует, но не ancestor HEAD), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ===========================================================================
# B-1 (R-020): ОСТАТОК ОБЯЗАН БЫТЬ ВИДЕН.
#
# Прежний сценарий `RFC-SHA-no-context` (удалён вместе с этим блоком-заменой) утверждал,
# что hex-токен БЕЗ слова-маркера («коммит»/«merge»/«мёрж»/«sha») не проверяется — и тем
# самым ЗАКРЕПЛЯЛ слепое пятно как желаемое поведение. Замер reviewer'а на реальном корпусе
# (merge-цель origin/main): 20 hex-токенов, проверено 17, МОЛЧА пропущено 3, из них два —
# нормативные утверждения о коммитах в docs/rfc/CT-RFC-05-margin-inventory.md:77 и :163
# («подтверждено отдельным ИСПРАВЛЕНИЕМ `b3a5a95`», «reviewer close-out (`41d3526`)»).
# Обход возникает от обычного русского синонима, а не от злого умысла: детектор ключевался
# на рукописном списке слов. Это fail-open внутри fail-closed гейта.
#
# Требуемое поведение (принцип): контекстные слова НЕ решают, проверять ли токен. Кандидат —
# КАЖДЫЙ hex-токен SHA-формы в backtick'ах вне фенсов. Непроверенный токен допустим ТОЛЬКО
# по узкому закрытому списку причин (SKIP-DIGITS / SKIP-LEN64 / SKIP-DECLARED), каждая —
# ПЕРЕЧИСЛЕНА построчно с файлом, строкой и токеном. Всё остальное — проверяется.
# Итоговая строка обязана печатать баланс `всего=N проверено=K пропущено=M`, и
# «проверка неприменима» недопустима, когда SHA-подобные токены в корпусе ЕСТЬ.
# ===========================================================================

# --- B-1 ядро: ВЫДУМАННЫЙ SHA в параграфе БЕЗ слова-маркера («исправлением» вместо
# «коммитом») → гейт ОБЯЗАН упасть. Ровно форма из docs/rfc/CT-RFC-05-margin-inventory.md:77.
scenario_rfc_sha_fake_without_marker_word_fails() {
  local d="${TMP_BASE}/rfc_sha_fake_nomarker"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-nomarker.md" <<'EOF'
# CT-RFC-TEST — форма «подтверждено отдельным исправлением»

Это подтверждено отдельным исправлением `0000000deadbee` («fix(M-99): нечто»), которого
в репозитории не существует вовсе.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[6-RFC-SHA\].*`0000000deadbee`'; then
    pass "сценарий RFC-SHA-fake-nomarker (B-1: выдуманный SHA в параграфе БЕЗ слова-маркера): гейт даёт FAIL [6-RFC-SHA], exit=${rc}"
  else
    fail "сценарий RFC-SHA-fake-nomarker (B-1): ОЖИДАЛСЯ FAIL [6-RFC-SHA] на 0000000deadbee — слово-маркер НЕ должен решать, проверять ли токен, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- B-1: SHA-orphan (реальный git-объект вне ancestry HEAD) в параграфе БЕЗ слова-маркера
# → FAIL. Сочетание двух обходов сразу: орфан + синоним.
scenario_rfc_sha_orphan_without_marker_word_fails() {
  local d="${TMP_BASE}/rfc_sha_orphan_nomarker"
  local main_sha orphan_sha
  main_sha="$(build_rfc_fixture_base "${d}")"

  git -C "${d}" checkout -q -b orphan-branch-nomarker
  echo "orphan work, never merged" > "${d}/orphan-nomarker.txt"
  git -C "${d}" add -A
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "orphan commit, never merged"
  orphan_sha="$(git -C "${d}" rev-parse HEAD)"
  git -C "${d}" checkout -q "${main_sha}"

  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-orphan-nomarker.md" <<EOF
# CT-RFC-TEST — orphan SHA без слова-маркера

Это зафиксировано отдельной правкой \`${orphan_sha}\` — reviewer close-out того же дня.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q "FAIL  \[6-RFC-SHA\].*\`${orphan_sha}\`.*НЕ входит в историю"; then
    pass "сценарий RFC-SHA-orphan-nomarker (B-1: орфан + синоним вместо слова-маркера): гейт даёт FAIL [6-RFC-SHA], exit=${rc}"
  else
    fail "сценарий RFC-SHA-orphan-nomarker (B-1): ОЖИДАЛСЯ FAIL [6-RFC-SHA] «НЕ входит в историю» без слова-маркера, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- N-3 (R-020) закрывается НЕ безусловным пропуском цифровых токенов, а явным маркером.
#
# ПОЧЕМУ НЕ SKIP-DIGITS. Правило «чисто цифровой токен не проверяется» выводит из-под гейта
# канонические выдуманные SHA `0000000` и `1111111` — ровно те, которыми пользуется и
# существующий сценарий RFC-SHA-fake, и репро самого R-020. То есть закрытый список причин
# заново открыл бы дыру, ради которой B-1 и заведён (fail-open в fail-closed гейте).
# Замер на реальном корпусе (merge-цель origin/main): чисто цифровой токен ровно ОДИН —
# `0999929`, и это НАСТОЯЩИЙ коммит; ни одной fixed-point константы в backtick'ах в
# docs/DESIGN.md + docs/rfc/ нет. Значит fail-closed на цифрах не стоит ничего, а ложный FAIL
# на будущую константу закрывается тем же машинным маркером, что и любой другой не-коммит:
# <!-- not-a-commit: 100000000 -->. Ambiguity разрешается В ПОЛЬЗУ ПРОВЕРКИ.
scenario_rfc_sha_digits_token_is_fail_closed() {
  local d="${TMP_BASE}/rfc_sha_digits"
  local real_sha
  real_sha="$(build_rfc_fixture_base "${d}")"
  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-digits.md" <<EOF
# CT-RFC-TEST — десятичный литерал в параграфе про коммит

Коммит \`${real_sha}\` ввёл fixed-point ×1e8: цена хранится как целое, множитель
\`100000000\` — это константа, а не идентификатор коммита.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[6-RFC-SHA\].*`100000000`'; then
    pass "сценарий RFC-SHA-digits-failclosed (цифровой токен БЕЗ маркера): гейт fail-closed → FAIL [6-RFC-SHA], exit=${rc}"
  else
    fail "сценарий RFC-SHA-digits-failclosed: ОЖИДАЛСЯ FAIL [6-RFC-SHA] на 100000000 — неизвестная форма ПРОВЕРЯЕТСЯ, а не пропускается (иначе выдуманные 0000000/1111111 уходят из-под гейта), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi

  # Тот же токен, объявленный маркером, — не FAIL, но ОБЯЗАН быть перечислен (N-3 закрыт).
  local d2="${TMP_BASE}/rfc_sha_digits_declared"
  local real_sha2
  real_sha2="$(build_rfc_fixture_base "${d2}")"
  cat > "${d2}/docs/rfc/CT-RFC-TEST-sha-digits-declared.md" <<EOF
# CT-RFC-TEST — тот же литерал, объявленный маркером

<!-- not-a-commit: 100000000 -->
Коммит \`${real_sha2}\` ввёл fixed-point ×1e8: множитель \`100000000\` — константа.
EOF
  local out2 rc2
  out2="$(run_verify "${d2}")"; rc2=$?
  if [ "${rc2}" -eq 0 ] && ! echo "${out2}" | grep -q 'FAIL  \[6-RFC-SHA\]' \
     && echo "${out2}" | grep -q '\[6-RFC-SHA\] SKIP-DECLARED.*100000000'; then
    pass "сценарий RFC-SHA-digits-declared (N-3: маркер not-a-commit): ложного FAIL нет И токен перечислен как SKIP-DECLARED, exit=${rc2}"
  else
    fail "сценарий RFC-SHA-digits-declared (N-3): ОЖИДАЛОСЬ отсутствие FAIL [6-RFC-SHA] И строка SKIP-DECLARED с токеном 100000000, получено (exit=${rc2}):"
    echo "${out2}" | sed 's/^/      /'
  fi
}

# --- B-1 (закрытый список причин, п.2 SKIP-LEN64): 64-символьный hex — sha256-дайджест,
# не идентификатор коммита. Не FAIL, но ОБЯЗАН быть перечислен.
scenario_rfc_sha_len64_skipped_and_listed() {
  local d="${TMP_BASE}/rfc_sha_len64"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-len64.md" <<'EOF'
# CT-RFC-TEST — sha256-дайджест, не коммит

Контрольная сумма сегмента журнала:
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` — это sha256 содержимого,
а не коммит.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'FAIL  \[6-RFC-SHA\]' \
     && echo "${out}" | grep -q '\[6-RFC-SHA\] SKIP-LEN64.*e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'; then
    pass "сценарий RFC-SHA-len64 (sha256-дайджест): ложного FAIL нет И токен перечислен как SKIP-LEN64, exit=${rc}"
  else
    fail "сценарий RFC-SHA-len64: ОЖИДАЛОСЬ отсутствие FAIL [6-RFC-SHA] И строка SKIP-LEN64 с 64-символьным токеном, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- B-1 (закрытый список причин, п.3 SKIP-DECLARED): единственный способ вывести токен
# из-под проверки — ЯВНЫЙ машинный маркер в том же файле. Анти-плацебо: тот же токен БЕЗ
# маркера обязан давать FAIL (иначе «объявление» ничего не значит и это просто дыра).
scenario_rfc_sha_declared_not_commit_skipped_and_listed() {
  local d="${TMP_BASE}/rfc_sha_declared"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-declared.md" <<'EOF'
# CT-RFC-TEST — токен явно объявлен не-коммитом

<!-- not-a-commit: abc1234def -->
Идентификатор партии данных: `abc1234def` — не коммит, объявлено маркером выше.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'FAIL  \[6-RFC-SHA\]' \
     && echo "${out}" | grep -q '\[6-RFC-SHA\] SKIP-DECLARED.*abc1234def'; then
    pass "сценарий RFC-SHA-declared (машинный маркер not-a-commit): FAIL нет И токен перечислен как SKIP-DECLARED, exit=${rc}"
  else
    fail "сценарий RFC-SHA-declared: ОЖИДАЛОСЬ отсутствие FAIL [6-RFC-SHA] И строка SKIP-DECLARED с токеном abc1234def, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
    return
  fi

  # анти-плацебо: убираем маркер — тот же токен обязан стать нарушением
  local d2="${TMP_BASE}/rfc_sha_declared_removed"
  build_rfc_fixture_base "${d2}" >/dev/null
  cat > "${d2}/docs/rfc/CT-RFC-TEST-sha-undeclared.md" <<'EOF'
# CT-RFC-TEST — тот же токен БЕЗ маркера

Идентификатор партии данных: `abc1234def` — маркера not-a-commit нет.
EOF
  local out2 rc2
  out2="$(run_verify "${d2}")"; rc2=$?
  if [ "${rc2}" -ne 0 ] && echo "${out2}" | grep -q 'FAIL  \[6-RFC-SHA\].*`abc1234def`'; then
    pass "сценарий RFC-SHA-declared/анти-плацебо (тот же токен БЕЗ маркера): гейт даёт FAIL [6-RFC-SHA], exit=${rc2}"
  else
    fail "сценарий RFC-SHA-declared/анти-плацебо: ОЖИДАЛСЯ FAIL [6-RFC-SHA] на abc1234def БЕЗ маркера (иначе маркер — фикция), получено (exit=${rc2}):"
    echo "${out2}" | sed 's/^/      /'
  fi
}

# --- B-1: «проверка неприменима» ЗАПРЕЩЕНА, когда SHA-подобные токены в корпусе ЕСТЬ.
# Ровно воспроизведение из R-020: документ, целиком состоящий из выдуманных SHA, сегодня
# получает INFO «не найдено цитат коммитов … — проверка неприменима».
scenario_rfc_sha_never_inapplicable_when_tokens_exist() {
  local d="${TMP_BASE}/rfc_sha_inapplicable"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-99-probe.md" <<'EOF'
# CT-RFC-99 — проба формы «подтверждено исправлением»

Это подтверждено отдельным исправлением `0000000deadbee` («fix(M-99): нечто»),
которого в репозитории не существует вовсе.

Здесь же close-out ревьюера (`1111111f`) — тоже выдуманный, и партия `1111111`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if ! echo "${out}" | grep '\[6-RFC-SHA\]' | grep -q 'неприменима'; then
    pass "сценарий RFC-SHA-no-inapplicable: при наличии SHA-подобных токенов гейт НЕ печатает «проверка неприменима», exit=${rc}"
  else
    fail "сценарий RFC-SHA-no-inapplicable: гейт напечатал «проверка неприменима» на документе, где SHA-подобные токены ЕСТЬ (fail-open), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
  # Второй выдуманный токен живёт в СОСЕДНЕМ параграфе (в реальном репро R-020 он там и
  # не ловился даже после замены синонима на «коммитом») — параграф больше не влияет.
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[6-RFC-SHA\].*`1111111f`'; then
    pass "сценарий RFC-SHA-no-inapplicable/второй токен: 1111111f в СОСЕДНЕМ параграфе тоже проверен → FAIL, exit=${rc}"
  else
    fail "сценарий RFC-SHA-no-inapplicable/второй токен: ОЖИДАЛСЯ FAIL [6-RFC-SHA] и на 1111111f (соседний параграф — не оправдание пропуска), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
  # Третий токен — чисто цифровой выдуманный `1111111` (форма из репро R-020). Он ОБЯЗАН
  # падать так же, как hex-форма: «цифровой» не является причиной пропуска (см. обоснование
  # у сценария RFC-SHA-digits-failclosed).
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[6-RFC-SHA\].*`1111111`'; then
    pass "сценарий RFC-SHA-no-inapplicable/цифровой выдуманный: 1111111 тоже проверен → FAIL, exit=${rc}"
  else
    fail "сценарий RFC-SHA-no-inapplicable/цифровой выдуманный: ОЖИДАЛСЯ FAIL [6-RFC-SHA] на 1111111 (цифровая форма — не причина пропуска), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- B-1: баланс печатается ВСЕГДА и СХОДИТСЯ: всего=N проверено=K пропущено=M, K+M==N.
# Фикстура смешанная: 1 реальный SHA (проверяется), 1 sha256-дайджест (SKIP-LEN64),
# 1 объявленный маркером (SKIP-DECLARED) → всего=3 проверено=1 пропущено=2.
scenario_rfc_sha_balance_line_reconciles() {
  local d="${TMP_BASE}/rfc_sha_balance"
  local real_sha
  real_sha="$(build_rfc_fixture_base "${d}")"
  cat > "${d}/docs/rfc/CT-RFC-TEST-sha-balance.md" <<EOF
# CT-RFC-TEST — баланс

<!-- not-a-commit: abc1234def -->
Изменение внесено коммитом \`${real_sha}\`; sha256 сегмента
\`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\`; партия \`abc1234def\`.
EOF
  local out rc line total checked skipped
  out="$(run_verify "${d}")"; rc=$?
  line="$(echo "${out}" | grep '\[6-RFC-SHA\]' | grep -o 'всего=[0-9]* проверено=[0-9]* пропущено=[0-9]*' | head -1)"
  if [ -z "${line}" ]; then
    fail "сценарий RFC-SHA-balance: строка баланса «всего=N проверено=K пропущено=M» в выводе [6-RFC-SHA] ОТСУТСТВУЕТ (проверка обязана знать, чего она не проверила), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
    return
  fi
  total="$(echo "${line}" | sed 's/.*всего=\([0-9]*\).*/\1/')"
  checked="$(echo "${line}" | sed 's/.*проверено=\([0-9]*\).*/\1/')"
  skipped="$(echo "${line}" | sed 's/.*пропущено=\([0-9]*\).*/\1/')"
  if [ "${total}" -eq 3 ] && [ "${checked}" -eq 1 ] && [ "${skipped}" -eq 2 ] \
     && [ $((checked + skipped)) -eq "${total}" ]; then
    pass "сценарий RFC-SHA-balance: баланс сходится и точен (${line}), exit=${rc}"
  else
    fail "сценарий RFC-SHA-balance: ОЖИДАЛОСЬ всего=3 проверено=1 пропущено=2 (реальный SHA + цифровой + объявленный), получено «${line}» (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- N-2 (R-020): шапка обещает docs/rfc/**.md, код обходит только верхний уровень
# (os.listdir). Подкаталог с выдуманным SHA и несуществующим путём обязан быть увиден.
scenario_rfc_subdirectory_is_scanned() {
  local d="${TMP_BASE}/rfc_subdir"
  build_rfc_fixture_base "${d}" >/dev/null
  mkdir -p "${d}/docs/rfc/sub"
  cat > "${d}/docs/rfc/sub/CT-RFC-TEST-nested.md" <<'EOF'
# CT-RFC-TEST — вложенный RFC

Изменение подтверждено коммитом `0000000`, затронут файл `crates/xx/tests/nope.rs`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] \
     && echo "${out}" | grep -q 'FAIL  \[6-RFC-SHA\].*sub/CT-RFC-TEST-nested\.md' \
     && echo "${out}" | grep -q 'FAIL  \[7-RFC-PATH\].*sub/CT-RFC-TEST-nested\.md'; then
    pass "сценарий RFC-SUBDIR (N-2: docs/rfc/**.md рекурсивно): вложенный RFC проверен обеими проверками, exit=${rc}"
  else
    fail "сценарий RFC-SUBDIR (N-2): ОЖИДАЛИСЬ FAIL [6-RFC-SHA] И FAIL [7-RFC-PATH] на docs/rfc/sub/CT-RFC-TEST-nested.md (шапка обещает **, код обязан обходить рекурсивно), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ===========================================================================
# N-1 (R-020): тот же принцип для ПУТЕЙ. Whitelist префиксов
# crates|docs|scripts|research|milestones|.claude пропускает крейт-относительную форму,
# которой реальные RFC пользуются свободно (`contracts/src/lib.rs:46`,
# `recorder/src/main.rs:58`, `journal/src/segments.rs`, `tests/red_schema.rs`).
# Замер reviewer'а: проверяется 67 путей, молча пропускается 49.
# ===========================================================================

# --- N-1: крейт-относительный НЕСУЩЕСТВУЮЩИЙ файл → FAIL (резолв через crates/<name>/ —
# обязателен; молчание недопустимо).
scenario_rfc_path_crate_relative_missing_fails() {
  local d="${TMP_BASE}/rfc_path_crate_rel_missing"
  build_rfc_fixture_base "${d}" >/dev/null
  mkdir -p "${d}/crates/journal/src"
  echo "// пусто" > "${d}/crates/journal/src/lib.rs"
  cat > "${d}/docs/rfc/CT-RFC-TEST-path-crate-rel-missing.md" <<'EOF'
# CT-RFC-TEST — крейт-относительный путь, которого нет
Затронутый файл: `journal/src/nosuchfile.rs`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[7-RFC-PATH\].*journal/src/nosuchfile\.rs'; then
    pass "сценарий RFC-PATH-crate-rel-missing (N-1: крейт-относительная форма, файла нет): гейт даёт FAIL [7-RFC-PATH], exit=${rc}"
  else
    fail "сценарий RFC-PATH-crate-rel-missing (N-1): ОЖИДАЛСЯ FAIL [7-RFC-PATH] на journal/src/nosuchfile.rs (crates/journal существует → форма якорится в дерево), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- N-1: крейт-относительный СУЩЕСТВУЮЩИЙ файл → не FAIL И засчитан как проверенный
# (пропущено=0), а не «тихо пропущен».
scenario_rfc_path_crate_relative_real_is_counted() {
  local d="${TMP_BASE}/rfc_path_crate_rel_real"
  build_rfc_fixture_base "${d}" >/dev/null
  mkdir -p "${d}/crates/journal/src"
  echo "// пусто" > "${d}/crates/journal/src/segments.rs"
  cat > "${d}/docs/rfc/CT-RFC-TEST-path-crate-rel-real.md" <<'EOF'
# CT-RFC-TEST — крейт-относительный путь, файл существует
Затронутый файл: `journal/src/segments.rs`.
EOF
  local out rc line
  out="$(run_verify "${d}")"; rc=$?
  line="$(echo "${out}" | grep '\[7-RFC-PATH\]' | grep -o 'всего=[0-9]* проверено=[0-9]* пропущено=[0-9]*' | head -1)"
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'FAIL  \[7-RFC-PATH\]' \
     && [ -n "${line}" ] && echo "${line}" | grep -q 'пропущено=0'; then
    pass "сценарий RFC-PATH-crate-rel-real (N-1: форма резолвится через crates/<name>/): FAIL нет И токен ЗАСЧИТАН (${line}), exit=${rc}"
  else
    fail "сценарий RFC-PATH-crate-rel-real (N-1): ОЖИДАЛОСЬ отсутствие FAIL [7-RFC-PATH] И баланс с пропущено=0 (токен обязан быть ПРОВЕРЕН, не пропущен), получено «${line}» (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- N-1: баланс путей печатается ВСЕГДА и сходится; законные классы пропуска названы
# ПОИМЕНОВАННОЙ причиной (glob / абсолютный эндпоинт / фрагмент прозы), а не молчанием.
scenario_rfc_path_balance_line_reconciles_with_named_skips() {
  local d="${TMP_BASE}/rfc_path_balance"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-path-balance.md" <<'EOF'
# CT-RFC-TEST — баланс путей и поименованные пропуски

Реальный файл `crates/xx/tests/red_xx.rs`; паттерн `crates/xx/**`; эндпоинт биржи
`/sapi/v1/margin/available-inventory?type=MARGIN`; перечисление типов `Ord/Risk/Ctl`.
EOF
  local out rc line total checked skipped
  out="$(run_verify "${d}")"; rc=$?
  line="$(echo "${out}" | grep '\[7-RFC-PATH\]' | grep -o 'всего=[0-9]* проверено=[0-9]* пропущено=[0-9]*' | head -1)"
  if [ -z "${line}" ]; then
    fail "сценарий RFC-PATH-balance: строка баланса «всего=N проверено=K пропущено=M» в выводе [7-RFC-PATH] ОТСУТСТВУЕТ, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
    return
  fi
  total="$(echo "${line}" | sed 's/.*всего=\([0-9]*\).*/\1/')"
  checked="$(echo "${line}" | sed 's/.*проверено=\([0-9]*\).*/\1/')"
  skipped="$(echo "${line}" | sed 's/.*пропущено=\([0-9]*\).*/\1/')"
  if [ "${rc}" -eq 0 ] && [ $((checked + skipped)) -eq "${total}" ] && [ "${total}" -eq 4 ] \
     && [ "${checked}" -eq 1 ] && [ "${skipped}" -eq 3 ] \
     && echo "${out}" | grep -q '\[7-RFC-PATH\] SKIP-GLOB.*crates/xx/\*\*' \
     && echo "${out}" | grep -q '\[7-RFC-PATH\] SKIP-.*sapi/v1/margin' \
     && echo "${out}" | grep -q '\[7-RFC-PATH\] SKIP-.*Ord/Risk/Ctl'; then
    pass "сценарий RFC-PATH-balance: баланс сходится (${line}) И все три пропуска перечислены поименованной причиной, exit=${rc}"
  else
    fail "сценарий RFC-PATH-balance: ОЖИДАЛОСЬ всего=4 проверено=1 пропущено=3 + перечисленные SKIP-* для glob/эндпоинта/перечисления типов, получено «${line}» (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- сценарий: RFC цитирует РЕАЛЬНЫЙ путь (существующий в дереве фикстуры) → PASS [7-RFC-PATH] ---
scenario_rfc_path_real_passes() {
  local d="${TMP_BASE}/rfc_path_real"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-path-real.md" <<'EOF'
# CT-RFC-TEST — реальный путь
Затронутый файл: `crates/xx/tests/red_xx.rs`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && echo "${out}" | grep -q '^VERDICT: PASS' \
     && echo "${out}" | grep -q 'PASS  \[7-RFC-PATH\]'; then
    pass "сценарий RFC-PATH-real (реальный путь в docs/rfc/): гейт даёт PASS [7-RFC-PATH], VERDICT: PASS, exit=${rc}"
  else
    fail "сценарий RFC-PATH-real: ОЖИДАЛСЯ PASS [7-RFC-PATH] + VERDICT: PASS, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- анти-плацебо: путь с ХВОСТОМ внутри ТОЙ ЖЕ пары backtick'ов (` path.md §9 `,
# `path.rs:301-315`, `path.rs::func_name`) — хвост отбрасывается, путь резолвится по
# файлу. Реальный ложный FAIL, пойманный при прогоне check7 на CT-RFC-05: токен
# `research/data-quality/margin-source-survey.md §9` (пробел+секция ВНУТРИ backtick'ов)
# резолвился как несуществующий литеральный путь до этого фикса.
scenario_rfc_path_trailing_section_ref_stripped() {
  local d="${TMP_BASE}/rfc_path_section_tail"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-path-section-tail.md" <<'EOF'
# CT-RFC-TEST — путь с хвостом-секцией внутри backtick'ов
Мотивация зафиксирована в `crates/xx/tests/red_xx.rs §9`; тот же файл ещё раз через
Rust-путь `crates/xx/tests/red_xx.rs::xx_i_1_something_holds` и через диапазон строк
`crates/xx/tests/red_xx.rs:1-5`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && echo "${out}" | grep -q '^VERDICT: PASS' \
     && echo "${out}" | grep -q 'PASS  \[7-RFC-PATH\]'; then
    pass "сценарий RFC-PATH-section-tail (хвост §N/::func/:NNN внутри backtick'ов отброшен): гейт даёт PASS [7-RFC-PATH], exit=${rc}"
  else
    fail "сценарий RFC-PATH-section-tail: ОЖИДАЛСЯ PASS [7-RFC-PATH] (хвост отброшен, путь резолвится), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- сценарий: RFC ссылается на НЕСУЩЕСТВУЮЩИЙ путь → FAIL [7-RFC-PATH] (класс C-044 F2:
# документ занижает/искажает список мест правки — опечатка/несуществующий путь та же ложь) ---
scenario_rfc_path_missing_fails() {
  local d="${TMP_BASE}/rfc_path_fake"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-path-fake.md" <<'EOF'
# CT-RFC-TEST — несуществующий путь
Затронутый файл: `crates/xx/tests/does-not-exist.rs`.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[7-RFC-PATH\].*does-not-exist\.rs.*не существует'; then
    pass "сценарий RFC-PATH-fake (несуществующий путь в docs/rfc/, C-044 F2 класс): гейт даёт FAIL [7-RFC-PATH], exit=${rc}"
  else
    fail "сценарий RFC-PATH-fake: ОЖИДАЛСЯ FAIL [7-RFC-PATH], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- анти-плацебо: glob/brace-паттерн (`crates/xx/**`, `crates/xx/fixtures/{valid,invalid}`)
# — НЕ литеральный путь, не должен считаться отсутствующим файлом.
scenario_rfc_path_glob_pattern_skipped() {
  local d="${TMP_BASE}/rfc_path_glob"
  build_rfc_fixture_base "${d}" >/dev/null
  cat > "${d}/docs/rfc/CT-RFC-TEST-path-glob.md" <<'EOF'
# CT-RFC-TEST — glob-паттерн, не литеральный путь
Затрагивает `crates/xx/**` и `crates/xx/fixtures/{valid,invalid}` — примеры паттерна, не файлы.
EOF
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && echo "${out}" | grep -q '^VERDICT: PASS' \
     && ! echo "${out}" | grep -q 'FAIL  \[7-RFC-PATH\]'; then
    pass "сценарий RFC-PATH-glob (glob/brace-паттерн — не литеральный путь): гейт НЕ падает, exit=${rc}"
  else
    fail "сценарий RFC-PATH-glob: ОЖИДАЛСЯ PASS (glob-паттерн пропущен), получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

echo "=== RED self-test: scripts/verify_design_claims.sh (BARRIER=${BARRIER}) ==="
scenario_good
scenario_bad_est_missing_path
scenario_bad_est_no_path
scenario_bad_est_milestone_missing
scenario_bad_est_critique_missing
scenario_bad_est_review_missing
scenario_est_ascii_schema_no_proof_passes
scenario_est_prose_no_proof_passes
scenario_bad_coverage_overstated
scenario_bad_coverage_understated
scenario_bad_rk_foreign_crate_not_counted
scenario_bad_broken_section_ref
scenario_bad_dead_file_ref
scenario_bad_phase_milestone_missing
scenario_bad_setup_guard_missing_design
scenario_merge_preview_catches_branch_vs_merge_drift
scenario_merge_preview_conflict_is_setup_guard_fail
scenario_merge_preview_bad_base_ref_fails
scenario_rfc_sha_real_passes
scenario_rfc_sha_fake_fails
scenario_rfc_sha_orphan_exists_but_not_ancestor_fails
scenario_rfc_sha_fake_without_marker_word_fails
scenario_rfc_sha_orphan_without_marker_word_fails
scenario_rfc_sha_digits_token_is_fail_closed
scenario_rfc_sha_len64_skipped_and_listed
scenario_rfc_sha_declared_not_commit_skipped_and_listed
scenario_rfc_sha_never_inapplicable_when_tokens_exist
scenario_rfc_sha_balance_line_reconciles
scenario_rfc_subdirectory_is_scanned
scenario_rfc_path_real_passes
scenario_rfc_path_trailing_section_ref_stripped
scenario_rfc_path_missing_fails
scenario_rfc_path_glob_pattern_skipped
scenario_rfc_path_crate_relative_missing_fails
scenario_rfc_path_crate_relative_real_is_counted
scenario_rfc_path_balance_line_reconciles_with_named_skips

echo
if [ "${FAILED}" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL (${FAILED} нарушений)"
  exit 1
fi
