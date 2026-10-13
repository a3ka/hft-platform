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
# ГЕРМЕТИЧНОСТЬ К ОКРУЖЕНИЮ (TD-135). `testing.md` «Целостность гейта» свойство 2: исход
# пробы обязан зависеть от ЕЁ инварианта, а не от хоста. Здесь он зависел от хоста: у
# разработчика есть глобальный `~/.gitconfig` (`branch-hygiene.md` п.6 требует единой
# личности владельца репозитория), у раннера GitHub Actions его НЕТ. Фикстурный коммит без
# явной идентичности проходил локально и падал в CI с `fatal: empty ident name` — сценарий
# не строился, проба молча проверяла НЕ ТОТ сценарий, а анти-плацебо честно краснело.
# Цена: 6 SHA подряд с красным `main`, деплой fail-closed не выпускал НИЧЕГО.
#
# Лечение — УБРАТЬ окружение, а не добавить идентичность. Обратный ход (`export
# GIT_AUTHOR_NAME=…` в шапке) даёт зелёный вердикт и ВОСПРОИЗВОДИТ тот же класс на уровень
# глубже: переменные унаследовал бы БАРЬЕР, которого проба и испытывает, — и он получил бы
# идентичность, которой у него в CI нет. Проба снова мерила бы условия мягче прод-формы.
#
# Отсюда: ambient-идентичность снимается для всей пробы и её потомков. Каждый фикстурный
# коммит ОБЯЗАН нести `-c user.name/-c user.email` явно; забывший это сценарий краснеет
# ОДИНАКОВО на машине разработчика и на раннере. Локальный прогон становится прод-формой,
# а не её суррогатом.
export HOME="${TMP_BASE}/hermetic-home"
mkdir -p "${HOME}"
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_SYSTEM=/dev/null
# `R-069` N-1: конфигом идентичность НЕ исчерпывается — git берёт её и из окружения, причём
# ПРИОРИТЕТНЕЕ конфига. Хост с экспортированными `GIT_AUTHOR_*`/`GIT_COMMITTER_*`/`EMAIL`
# вернул бы ровно ту же слепоту: голый фикстурный коммит зелен локально, красен в CI.
# Раннер этих переменных не несёт, поэтому прод-форма не затронута — но дыра закрывается
# здесь, а не оговоркой в тексте.
unset GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL EMAIL

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

# F-1 (C-116): проверка ревизии сбора требует ЖИВОЙ истории — без коммита `HEAD` не
# существует, и фикстура могла бы проверять только отрицательный случай.
# Коммит делается ЯВНЫМ вызовом, а НЕ внутри `build_good_fixture`: та используется всеми
# сценариями, и `build_rfc_fixture_base` коммитит сама. Первая редакция правила коммит прямо
# в общую фикстуру — второй коммит становился пустым, ancestry менялась, и ЧЕТЫРЕ RFC-SHA
# сценария падали. Ровно «что пришлось ослабить рядом» (`testing.md`, мутационный контроль,
# второй вопрос): побочный эффект правки ловится соседним оракулом, а не рассуждением.
fixture_commit_base() { # $1=dir → печатает SHA
  git -C "$1" add -A >/dev/null 2>&1 || true
  git -C "$1" -c user.name=test -c user.email=test@test.local \
      commit -q -m "фикстура H-FACTS: базовое дерево" >/dev/null 2>&1 || true
  git -C "$1" rev-parse HEAD 2>/dev/null
}

fixture_head_sha() { git -C "$1" rev-parse HEAD 2>/dev/null; }

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
# ---------------------------------------------------------------------------
# Сценарии H-FACTS — маркер `FACTS:` (исполнение `A-010` §H, задача H-1).
# Анти-плацебо в ОБЕ стороны: с маркером документ судится, без маркера — нет.
# Плюс ловушка, найденная замером: маркер, процитированный В ПРОЗЕ (не в голове файла),
# включать документ НЕ ДОЛЖЕН — иначе документ О маркере опт-инится сам по себе. Тот же
# класс, что шапка GATE-META внутри код-фенса.
# ---------------------------------------------------------------------------
mk_plan_with_dead_ref() {   # $1=каталог фикстуры $2=шапка файла (может быть пустой)
  mkdir -p "$1/docs/plans"
  { [ -n "$2" ] && printf '%s\n' "$2"
    echo "# фактура"
    echo "Ссылка на \`docs/GHOSTPLAN.md\` — файла нет."
  } > "$1/docs/plans/facts.md"
}

scenario_facts_marked_plan_is_checked() {
  local d="${TMP_BASE}/facts1"
  build_good_fixture "${d}"
  fixture_commit_base "${d}" >/dev/null
  mk_plan_with_dead_ref "${d}" "<!-- FACTS: audited_head=$(fixture_head_sha "${d}") collected=2026-08-02 -->"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[4-МЁРТВЫЕ-ФАЙЛЫ\].*docs/GHOSTPLAN\.md' \
     && ! echo "${out}" | grep -q 'FAIL  \[H-FACTS-SHA\]'; then
    pass "H-FACTS-1 (план С маркером): судится, мёртвая ссылка поймана, exit=${rc}"
  else
    fail "H-FACTS-1 (план С маркером): ОЖИДАЛСЯ FAIL [4-МЁРТВЫЕ-ФАЙЛЫ], получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

scenario_facts_unmarked_plan_is_excluded() {
  local d="${TMP_BASE}/facts2"
  build_good_fixture "${d}"
  mk_plan_with_dead_ref "${d}" ''
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'GHOSTPLAN'; then
    pass "H-FACTS-2 (план БЕЗ маркера): исключён, ложного красного нет, exit=${rc}"
  else
    fail "H-FACTS-2 (план БЕЗ маркера): ОЖИДАЛСЯ PASS без упоминания GHOSTPLAN, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

scenario_facts_marker_in_prose_ignored() {
  local d="${TMP_BASE}/facts3"
  build_good_fixture "${d}"
  mkdir -p "${d}/docs/plans"
  { echo "# документ О маркере"
    for i in $(seq 1 12); do echo "строка прозы ${i}"; done
    # F-2 (C-116): плейсхолдер `<SHA>` регэкспом НЕ матчится НИ ПРИ КАКОМ лимите головы —
    # сценарий был зелен по неверной причине и лимит НЕ пиннил (стаб FACTS_HEAD_LINES=10**9
    # его проходил). Здесь стоит РЕАЛЬНЫЙ hex: документ обязан не опт-иниться ИМЕННО потому,
    # что маркер вне головы файла.
    echo 'Формат: `<!-- FACTS: audited_head=0123456789abcdef0123456789abcdef01234567 collected=2026-08-02 -->`.'
    echo "Ссылка на \`docs/GHOSTPLAN.md\` — файла нет."
  } > "${d}/docs/plans/about-marker.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'GHOSTPLAN'; then
    pass "H-FACTS-3 (маркер в ПРОЗЕ, не в голове): документ не опт-инится, exit=${rc}"
  else
    fail "H-FACTS-3 (маркер в ПРОЗЕ): ОЖИДАЛСЯ PASS — документ О маркере не должен судиться (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

scenario_facts_note_on_silent_plan() {
  local d="${TMP_BASE}/facts4"
  build_good_fixture "${d}"
  mkdir -p "${d}/docs/plans"
  { echo "# молчащая фактура"
    for i in $(seq 1 25); do echo "- см. \`crates/journal/src/lib.rs:${i}\`"; done
  } > "${d}/docs/plans/silent.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && echo "${out}" | grep -q 'NOTE  \[H-FACTS\].*silent\.md'; then
    pass "H-FACTS-4 (фактура без маркера, ≥20 утверждений): NOTE напечатан, прогон не свален, exit=${rc}"
  else
    fail "H-FACTS-4: ОЖИДАЛСЯ NOTE [H-FACTS] про silent.md при exit=0, получено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- F-3 (C-116): порог NOTE не был запиннен снизу. Стаб FACTS_NOTE_THRESHOLD=0 проходил
# пробу, а на живом корпусе давал 26 NOTE против 1 честного — флуд, хоронящий сигнал.
scenario_facts_note_threshold_pinned_below() {
  local d="${TMP_BASE}/facts5"
  build_good_fixture "${d}"
  mkdir -p "${d}/docs/plans"
  { echo "# короткая записка"
    for i in 1 2 3; do echo "- см. \`crates/journal/src/lib.rs:${i}\`"; done
  } > "${d}/docs/plans/tiny-note.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'NOTE  \[H-FACTS\].*tiny-note\.md'; then
    pass "H-FACTS-5 (записка без маркера, 3 утверждения < порога): NOTE НЕ печатается, exit=${rc}"
  else
    fail "H-FACTS-5: ОЖИДАЛСЯ exit=0 БЕЗ NOTE про tiny-note.md — порог не держится снизу (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- F-4 (C-116): маркер БЕЗ `audited_head` не обязан опт-инить документ. Весь смысл маркера
# по A-010 §H — ИМЕНОВАННАЯ ревизия сбора; стаб с регэкспом без `audited_head` пробу проходил.
scenario_facts_marker_without_head_not_opted_in() {
  local d="${TMP_BASE}/facts6"
  build_good_fixture "${d}"
  mk_plan_with_dead_ref "${d}" '<!-- FACTS: collected=2026-08-02 -->'
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  # Два утверждения сразу: (1) опт-ина нет — мёртвая ссылка НЕ ловится; (2) молчания тоже
  # нет — документ объявил себя фактурой негодной формой, и это названо (C-116 F-1, вторая
  # половина: «молчаливый даунгрейд»).
  if ! echo "${out}" | grep -q 'GHOSTPLAN' \
     && echo "${out}" | grep -q 'FAIL  \[H-FACTS-SHA\].*facts\.md.*НЕ распарсен'; then
    pass "H-FACTS-6 (маркер БЕЗ audited_head): опт-ина нет И молчания нет, exit=${rc}"
  else
    fail "H-FACTS-6: ОЖИДАЛОСЬ отсутствие GHOSTPLAN И FAIL [H-FACTS-SHA] про нераспарсенный маркер (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- F-5 (C-116): A-010 §H говорит про check3 И check4. H-FACTS-1 пиннил только check4;
# подмена исключения в check3 на старый tuple не роняла ничего.
# Литерал `DESIGN.md §<номер>` в тексте пробы НЕ пишется: сам гейт (check3) сканирует .sh и
# счёл бы его битой ссылкой репозитория — ровно тот класс, что красит вердикты (Н-3 передачи).
scenario_facts_marked_plan_checked_by_check3() {
  local d="${TMP_BASE}/facts7"
  build_good_fixture "${d}"
  fixture_commit_base "${d}" >/dev/null
  mkdir -p "${d}/docs/plans"
  local bad_sec=97
  { echo '<!-- FACTS: audited_head=0123456789abcdef0123456789abcdef01234567 collected=2026-08-02 -->'
    echo "# фактура с битой секцией"
    echo "Основание — \`DESIGN.md §${bad_sec}\`, раздела нет в оглавлении."
  } > "${d}/docs/plans/facts-badsec.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q "FAIL  \[3-ССЫЛКИ\].*facts-badsec\.md"; then
    pass "H-FACTS-7 (маркированный план, битая §-ссылка): check3 судит его, exit=${rc}"
  else
    fail "H-FACTS-7: ОЖИДАЛСЯ FAIL [3-ССЫЛКИ] про facts-badsec.md — check3-проводка не запиннена (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- F-6 (C-116): рефакторинг перенёс исключение `docs/archive/` из двух inline-tuple в общую
# функцию, и владение строкой перешло к ней — а сценария на неё нет. Стаб с выключенной веткой
# давал на живом корпусе 6 ложных FAIL: класс «ложное красное блокирует ВСЕ merge'и».
scenario_archive_exclusion_still_holds() {
  local d="${TMP_BASE}/facts8"
  build_good_fixture "${d}"
  mkdir -p "${d}/docs/archive"
  { echo "# архивный документ"
    echo "Ссылка на \`docs/GHOSTARCH.md\` — файла нет, и это НОРМА для архива."
  } > "${d}/docs/archive/old-plan.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'GHOSTARCH'; then
    pass "H-FACTS-8 (docs/archive/ с мёртвой ссылкой): исключение держится, exit=${rc}"
  else
    fail "H-FACTS-8: ОЖИДАЛСЯ exit=0 без GHOSTARCH — исключение archive не запиннено (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# ---------------------------------------------------------------------------
# Сценарии V-АРХИВ — архивный класс вердиктов (решение founder'а 2026-08-22, `TD-064`).
# Анти-плацебо в ТРИ стороны, потому что у исключения три способа быть неверным:
#   V-1  вердикт с мёртвой `docs/*.md` ссылкой не роняет гейт. ЧТО ИМЕННО он пиннит —
#        названо точно, потому что первый прогон мутаций показал обратное ожидаемому:
#        снятие исключения его НЕ роняет (check4 обходит только `docs/` и до вердиктов не
#        доходит ни с исключением, ни без). Он пиннит ГРАНИЦУ ПРИ РАСШИРЕНИИ ОХВАТА —
#        то есть ровно ту альтернативу, которую founder отверг: мутация «check4 обходит
#        весь репозиторий» + исключение на месте → V-1 зелен; та же мутация со снятым
#        исключением → V-1 краснеет. Вакуумным он не является, но и сегодняшнее поведение
#        check4 он не доказывает;
#   V-2  вердикт с битой ссылкой на раздел не роняет check3 (исключение симметрично —
#        до решения check3 обходил весь репозиторий и вердикты СУДИЛ, а check4 ходил
#        только по docs/ и не судил; асимметрия была случайной);
#   V-3  ГРАНИЦА: `research/reports/**` — НЕ вердикт и остаётся под судом. Без этого
#        сценария расширение исключения на весь `research/` прошло бы незамеченным;
#   V-4  объявление называет ВЕСЬ класс и берёт его из того же `VERDICT_CLASS_DIRS`,
#        которым делается исключение. Прежняя редакция пиннила СЧЁТЧИК (число мёртвых
#        ссылок); счётчик снят из наблюдателя после двух адверсарных кругов — `R-100` F-1
#        и `R-101` F-1/F-2/F-3 нашли четыре обманных стаба, и все четыре были про число,
#        ни один про исключение. Число фальсифицируемо по трём независимым осям (величина,
#        каталог, множество файлов), пиннится дороже, чем стоит, и никем не используется:
#        по решению founder'а вердикт стареет ЗАКОННО.
# Литерал `DESIGN.md` + `§` + номер подряд не пишется (см. сценарий 4): check3 сканирует
# .sh, и проба нашла бы собственный пример как настоящую битую ссылку.
# ---------------------------------------------------------------------------
scenario_verdict_class_dead_doc_ref_excluded() {
  local d="${TMP_BASE}/vclass1"
  build_good_fixture "${d}"
  { echo "# C-99 — вердикт, цитирующий улику"
    echo "Гейт ругался на \`docs/GHOSTVERDICT.md\` — файла нет, и это НОРМА для вердикта."
  } > "${d}/research/critiques/C-99-cite.md"
  # F-3 (`R-100`): ассерт НЕГАТИВНЫЙ («гейт не покраснел»), поэтому сорвавшаяся запись
  # фикстуры оставила бы сценарий зелёным на пустоте. Свидетель setup обязателен.
  [ -s "${d}/research/critiques/C-99-cite.md" ] || { fail "V-1: фикстура не создана — setup"; return; }
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'FAIL.*GHOSTVERDICT'; then
    pass "V-1 (вердикт с мёртвой docs-ссылкой): исключение держится, exit=${rc}"
  else
    fail "V-1: ОЖИДАЛСЯ exit=0 без FAIL по GHOSTVERDICT (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

scenario_verdict_class_broken_section_ref_excluded() {
  local d="${TMP_BASE}/vclass2"
  build_good_fixture "${d}"
  local fake_section="97"
  { echo "# R-99 — вердикт, цитирующий битую ссылку на раздел"
    printf 'Цитата вывода гейта: DESIGN.md §%s.\n' "${fake_section}"
  } > "${d}/research/reviews/R-99-cite.md"
  [ -s "${d}/research/reviews/R-99-cite.md" ] || { fail "V-2: фикстура не создана — setup"; return; }
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q '3-ССЫЛКИ.*97'; then
    pass "V-2 (вердикт с битой ссылкой на раздел): check3 его не судит, exit=${rc}"
  else
    fail "V-2: ОЖИДАЛСЯ exit=0 без FAIL [3-ССЫЛКИ] §97 (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

scenario_verdict_class_does_not_leak_to_reports() {
  local d="${TMP_BASE}/vclass3"
  build_good_fixture "${d}"
  mkdir -p "${d}/research/reports"
  local fake_section="96"
  { echo "# отчёт — НЕ вердикт, судится наравне со всеми"
    printf 'Утверждение отчёта: DESIGN.md §%s.\n' "${fake_section}"
  } > "${d}/research/reports/R-99-report.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[3-ССЫЛКИ\].*§96'; then
    pass "V-3 (research/reports вне класса): граница держится, гейт краснеет, exit=${rc}"
  else
    fail "V-3: ОЖИДАЛСЯ FAIL [3-ССЫЛКИ] §96 — исключение протекло на весь research/ (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}



# --- F-2 (`R-100`, блокер): матрица КЛАСС × ПРОВЕРКА была не закрыта. Класс — три каталога,
# но единственная проверка, которая СЕГОДНЯ реально доходит до `research/` (check3, обходит
# весь root), была запиннена только для `reviews/` (V-2). Выпадение `critiques/` или
# `arbitration/` из кортежа проходило всю пробу зелёным — и на реальном дереве тоже, потому
# что битых `§`-ссылок там сегодня нет. Регресс обнаружился бы следующим красным на
# аудит-артефакте, то есть ровно тем отложенным взрывом, который правка объявила снятым.
scenario_verdict_class_declaration_names_all_dirs() {
  # Заменяет два прежних СЧЁТНЫХ сценария (V-4/V-7). Счётчик снят из наблюдателя после двух
  # адверсарных кругов: `R-100` F-1 и `R-101` F-1/F-2/F-3 нашли четыре обманных стаба, и все
  # четыре были про число, ни один — про исключение. Пиннить осталось одно: объявление
  # называет ВЕСЬ класс и берёт его из того же `VERDICT_CLASS_DIRS`, которым исключение и
  # делается. Один носитель ⇒ расходиться нечему.
  local d="${TMP_BASE}/vclass4"
  build_good_fixture "${d}"
  mkdir -p "${d}/research/arbitration"
  echo "# A-99 — файл нужен, чтобы каталог существовал и попал в объявление" \
    > "${d}/research/arbitration/A-99-cite.md"
  [ -s "${d}/research/arbitration/A-99-cite.md" ] || { fail "V-4: фикстура не создана — setup"; return; }
  local out rc got want
  out="$(run_verify "${d}")"; rc=$?
  got="$(echo "${out}" | grep 'V-АРХИВ')"
  want="$(expected_varhiv 'research/critiques/ research/reviews/ research/arbitration/')"
  if [ "${rc}" -eq 0 ] && [ "${got}" = "${want}" ]; then
    pass "V-4 (строка V-АРХИВ ≡ ожидаемой целиком, полный класс), exit=${rc}"
  else
    fail "V-4: строка V-АРХИВ обязана СОВПАСТЬ ЦЕЛИКОМ.\n      ОЖИДАЛОСЬ: ${want}\n      ПОЛУЧЕНО: ${got}\n      exit=${rc}"
    echo "${out}" | grep 'V-АРХИВ' | sed 's/^/      /'
  fi
}

# Ожидаемая строка `V-АРХИВ` ЦЕЛИКОМ. Сравнение — СТРОКОВОЕ РАВЕНСТВО всего наблюдаемого
# вывода наблюдателя с этой строкой; никакого извлечения-прокси.
#
# Так — потому что пять кругов адверсария (`R-100`…`R-107`) были одной и той же ошибкой:
# ассерт ставился на ПРОКСИ, прокси сужался, и каждый следующий круг находил, чего он не
# видит. Счётчик -> перечень грепов -> образ регулярного извлечения: последний оказался
# НЕИНЪЕКТИВЕН — молча ронял имя без завершающего слэша и с точкой, нормализовал вложенный
# путь к родителю. Пять лгущих строк проходили 67/67 (`R-107` F-1).
#
# Равенство строки закрывает канал целиком: любое наблюдаемое отклонение — имя в любом
# написании, слэш, точка, вложенность, дубль, ПОРЯДОК, проза, вторая строка — есть
# неравенство. Остаток назван и недоступен чёрному ящику в принципе (`R-103` F-2/F-3):
# экстенсионально тождественная копия носителя и пустой каталог класса.
expected_varhiv() {   # $1 = список каталогов через пробел, В ПОРЯДКЕ печати наблюдателем
  printf 'INFO  [V-АРХИВ] архивный класс выведен из проверок ссылок как датированные снимки (решение 2026-08-22, `TD-064`): %s — ссылки внутри них не судятся, вердикт отвечает за ревизию из своей шапки `GATE-META`, а не за сегодняшнее дерево' "$1"
}

scenario_verdict_class_declaration_equals_tuple() {
  # `R-102` F-1/F-2 (блокеры). V-4 пиннил объявление СНИЗУ: «каждый из трёх назван».
  # Инвариант конструкции сильнее — `объявление ≡ кортеж`, и верхней границы не было:
  #   NC1  `present + ["research/reports/"]`      -> 0 FAIL: INFO ЛЖЁТ, что reports исключён;
  #   mc8b печать заменена литералом трёх имён    -> 0 FAIL: объявление больше НЕ читает
  #        константу, то есть возвращается второй носитель — невидимо для пробы.
  # Мой собственный стаб MC8 (замена на `research/`) падал только потому, что терял имена
  # со слэшами; в теле коммита `5939258` строка «MC8 объявление захардкожено -> V-4 FAILED»
  # была сформулирована ШИРЕ того, что реально прогонялось, и адверсарий её фальсифицировал.
  #
  # Закрытие КОНЕЧНО и матрицу не растит: одна фикстура, в которой класс представлен
  # НЕПОЛНО, а рядом лежит каталог ВНЕ класса. Объявление обязано назвать ровно то, что
  # есть и входит в класс:
  #   * нет `arbitration/`   => не назван  (валит и `present = list(VERDICT_CLASS_DIRS)`,
  #                                         и литерал трёх имён);
  #   * есть `reports/`      => не назван  (валит дописывание лишнего к `present`).
  local d="${TMP_BASE}/vclass8"
  build_good_fixture "${d}"          # каркас несёт critiques/ и reviews/, arbitration/ — НЕТ
  mkdir -p "${d}/research/reports"
  echo "# отчёт — НЕ вердикт, под судом, в объявлении класса ему не место" \
    > "${d}/research/reports/R-98-report.md"
  [ -s "${d}/research/reports/R-98-report.md" ] || { fail "V-7: фикстура не создана — setup"; return; }
  [ ! -d "${d}/research/arbitration" ] || { fail "V-7: каркас неожиданно создал arbitration/ — setup"; return; }
  local out rc got want
  out="$(run_verify "${d}")"; rc=$?
  got="$(echo "${out}" | grep 'V-АРХИВ')"
  want="$(expected_varhiv 'research/critiques/ research/reviews/')"
  if [ "${rc}" -eq 0 ] && [ "${got}" = "${want}" ]; then
    pass "V-7 (строка V-АРХИВ ≡ ожидаемой целиком, наличный класс), exit=${rc}"
  else
    fail "V-7: строка V-АРХИВ обязана СОВПАСТЬ ЦЕЛИКОМ.\n      ОЖИДАЛОСЬ: ${want}\n      ПОЛУЧЕНО: ${got}\n      exit=${rc}"
    echo "${out}" | grep 'V-АРХИВ' | sed 's/^/      /'
  fi
}

scenario_verdict_class_critiques_pinned_for_check3() {
  local d="${TMP_BASE}/vclass5"
  build_good_fixture "${d}"
  local fake_section="95"
  { echo "# C-98 — вердикт в critiques, цитирующий битую ссылку на раздел"
    printf 'Цитата вывода гейта: DESIGN.md §%s.\n' "${fake_section}"
  } > "${d}/research/critiques/C-98-cite.md"
  [ -s "${d}/research/critiques/C-98-cite.md" ] || { fail "V-5: фикстура не создана — setup"; return; }
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q '3-ССЫЛКИ.*95'; then
    pass "V-5 (critiques × check3): каталог запиннен отдельно, exit=${rc}"
  else
    fail "V-5: ОЖИДАЛСЯ exit=0 без FAIL [3-ССЫЛКИ] §95 — critiques выпал из класса (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

scenario_verdict_class_arbitration_pinned_for_check3() {
  local d="${TMP_BASE}/vclass6"
  build_good_fixture "${d}"
  mkdir -p "${d}/research/arbitration"
  local fake_section="94"
  { echo "# A-98 — арбитраж, цитирующий битую ссылку на раздел"
    printf 'Цитата вывода гейта: DESIGN.md §%s.\n' "${fake_section}"
  } > "${d}/research/arbitration/A-98-cite.md"
  [ -s "${d}/research/arbitration/A-98-cite.md" ] || { fail "V-6: фикстура не создана — setup"; return; }
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q '3-ССЫЛКИ.*94'; then
    pass "V-6 (arbitration × check3): каталог запиннен отдельно, exit=${rc}"
  else
    fail "V-6: ОЖИДАЛСЯ exit=0 без FAIL [3-ССЫЛКИ] §94 — arbitration выпал из класса (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- F-1 (`R-100`, блокер): у наблюдателя была ОДНА счётная точка (V-4, число 2), и любой
# стаб, печатающий константу 2, проходил пробу неотличимо от рабочего счёта. Одна точка не
# отличает счёт от константы В ПРИНЦИПЕ — нужна ВТОРАЯ с ДРУГИМ числом: тогда константа
# обязана провалить хотя бы одну. Пиннится и число файлов: иначе константа в `n_files`
# остаётся такой же дырой, только в соседнем поле.


# --- граница головы файла пиннится с ОБЕИХ сторон (testing.md §«Дегенерированный вход», п.4).
# H-FACTS-3 держит верх (маркер вне головы не считается); этот — низ: маркер НА последней
# строке головы обязан считаться, иначе стаб FACTS_HEAD_LINES=1 проходит незамеченным.
scenario_facts_marker_on_last_head_line_counts() {
  local d="${TMP_BASE}/facts9"
  build_good_fixture "${d}"
  fixture_commit_base "${d}" >/dev/null
  mkdir -p "${d}/docs/plans"
  { echo "# заголовок"
    echo ""
    echo "вводная строка"
    echo ""
    echo "<!-- FACTS: audited_head=$(fixture_head_sha "${d}") collected=2026-08-02 -->"
    echo "Ссылка на \`docs/GHOSTPLAN.md\` — файла нет."
  } > "${d}/docs/plans/marker-line5.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[4-МЁРТВЫЕ-ФАЙЛЫ\].*marker-line5\.md'; then
    pass "H-FACTS-9 (маркер на 5-й строке — граница головы): засчитан, документ судится, exit=${rc}"
  else
    fail "H-FACTS-9: ОЖИДАЛСЯ FAIL про marker-line5.md — граница головы не держится снизу (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- F-1 (C-116, ранг REJECT): решение арбитра `A-010` §H требовало «проверку SHA той же
# механикой, что check6 (существует, предок HEAD)». Захват `audited_head=(…)` в регэкспе был
# МЁРТВЫМ кодом — группа не потреблялась нигде, и документ с выдуманной ревизией опт-инился,
# не будучи судим ничем. Три сценария ниже пиннят обе половины: саму проверку и наблюдение
# отсутствия («молчаливый даунгрейд»).
scenario_facts_sha_fake_fails() {
  local d="${TMP_BASE}/facts10"
  build_good_fixture "${d}"
  mkdir -p "${d}/docs/plans"
  local fake=1111111111111111111111111111111111111111
  { echo "<!-- FACTS: audited_head=${fake} collected=2026-08-02 -->"
    echo "# фактура на несуществующем дереве"
    echo "- см. \`crates/journal/src/lib.rs:1\`"
  } > "${d}/docs/plans/fake-rev.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q "FAIL  \[H-FACTS-SHA\].*fake-rev\.md.*НЕТ вовсе"; then
    pass "H-FACTS-10 (маркер с ВЫДУМАННОЙ ревизией): FAIL, документ не проходит опт-ином, exit=${rc}"
  else
    fail "H-FACTS-10: ОЖИДАЛСЯ FAIL [H-FACTS-SHA] про несуществующий коммит (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

scenario_facts_sha_orphan_fails() {
  local d="${TMP_BASE}/facts11"
  build_good_fixture "${d}"
  local base_sha orphan_sha
  base_sha="$(fixture_commit_base "${d}")"
  git -C "${d}" checkout -q -b orphan-facts
  echo "работа, которую никуда не влили" > "${d}/orphan-facts.txt"
  git -C "${d}" add orphan-facts.txt
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "орфан: ветка не влита"
  orphan_sha="$(git -C "${d}" rev-parse HEAD)"
  git -C "${d}" checkout -q "${base_sha}"
  mkdir -p "${d}/docs/plans"
  { echo "<!-- FACTS: audited_head=${orphan_sha} collected=2026-08-02 -->"
    echo "# фактура, собранная на невлитой ветке"
    echo "- см. \`crates/journal/src/lib.rs:1\`"
  } > "${d}/docs/plans/orphan-rev.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  # Анти-плацебо того же класса, что C-044 F1: существование НЕОБХОДИМО, но НЕ достаточно —
  # ревизия сбора обязана входить в историю, иначе «датировано» ничего не значит.
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q "FAIL  \[H-FACTS-SHA\].*orphan-rev\.md.*НЕ входит в историю"; then
    pass "H-FACTS-11 (ревизия существует, но вне ancestry): FAIL, exit=${rc}"
  else
    fail "H-FACTS-11: ОЖИДАЛСЯ FAIL [H-FACTS-SHA] «НЕ входит в историю» — существования мало (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

scenario_facts_malformed_marker_is_not_silent() {
  local d="${TMP_BASE}/facts12"
  build_good_fixture "${d}"
  mk_plan_with_dead_ref "${d}" '<!-- FACTS: audited_head=012345 collected=2026-08-02 -->'
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  # 6 hex < минимума {7,40}: автор думает «документ под гейтом», механизм молчал бы.
  # Обе половины: опт-ина нет И молчания нет.
  if ! echo "${out}" | grep -q 'GHOSTPLAN' \
     && echo "${out}" | grep -q 'FAIL  \[H-FACTS-SHA\].*facts\.md.*НЕ распарсен'; then
    pass "H-FACTS-12 (короткий SHA, 6 hex): молчаливого даунгрейда нет — назван, exit=${rc}"
  else
    fail "H-FACTS-12: ОЖИДАЛСЯ FAIL [H-FACTS-SHA] про нераспарсенный маркер и отсутствие опт-ина (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- `C-117` F-13 (REJECT, стаб S17): исключение МАРКИРОВАННЫХ документов из NOTE не было
# запиннено ничем. Симметрия к F-3: H-FACTS-4 держит «немаркированный + много → NOTE есть»,
# H-FACTS-5 — «мало → NOTE нет», и НИКТО не утверждал «маркированный + много → NOTE НЕТ».
# Живой радиус немедленный, не латентный: стаб давал 3 NOTE вместо 1, и оба посева маркера
# получали «документ не называет ревизию сбора», НАЗЫВАЯ её.
scenario_facts_marked_plan_gets_no_note() {
  local d="${TMP_BASE}/facts13"
  build_good_fixture "${d}"
  fixture_commit_base "${d}" >/dev/null
  mkdir -p "${d}/docs/plans"
  { echo "<!-- FACTS: audited_head=$(fixture_head_sha "${d}") collected=2026-08-02 -->"
    echo "# честная фактура: маркер есть, утверждений много"
    for i in $(seq 1 25); do echo "- см. \`crates/journal/src/lib.rs:${i}\`"; done
  } > "${d}/docs/plans/marked-heavy.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'NOTE  \[H-FACTS\].*marked-heavy\.md'; then
    pass "H-FACTS-13 (маркер ЕСТЬ + 25 утверждений): NOTE не печатается, exit=${rc}"
  else
    fail "H-FACTS-13: ОЖИДАЛСЯ exit=0 БЕЗ NOTE — маркированный документ ревизию НАЗЫВАЕТ (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- `C-117` F-14 (REJECT, стаб S18): fail-closed на не-git корне не был запиннен — все
# фикстуры пробы были git-репозиториями. Радиус: tar-экспорт/копия без `.git` (класс `C-062`
# — прогон не на том дереве) с фиктивной ревизией проходил бы МОЛЧА.
# `testing.md` «Целостность гейта», свойство 3: гейт обязан падать против несостоявшегося setup.
scenario_facts_sha_non_git_is_setup_guard() {
  local d="${TMP_BASE}/facts14"
  build_good_fixture "${d}"
  mkdir -p "${d}/docs/plans"
  { echo '<!-- FACTS: audited_head=1111111111111111111111111111111111111111 collected=2026-08-02 -->'
    echo "# фактура в дереве без истории"
    echo "- см. \`crates/journal/src/lib.rs:1\`"
  } > "${d}/docs/plans/no-git-rev.md"
  rm -rf "${d}/.git"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[H-FACTS-SHA\].*не git-репозиторий.*setup-guard'; then
    pass "H-FACTS-14 (маркер в НЕ-git корне): setup-guard FAIL, не молчание, exit=${rc}"
  else
    fail "H-FACTS-14: ОЖИДАЛСЯ FAIL [H-FACTS-SHA] setup-guard — нечем проверить есть НАРУШЕНИЕ (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- `C-117` F-16 (NOTE, стаб S20): граница порога `n == 20` не проверялась никем —
# H-FACTS-4 кладёт 25, H-FACTS-5 — 3, а off-by-one (`>` вместо `>=`) проходил пробу, и
# фактура РОВНО в 20 утверждений молча теряла NOTE.
# `testing.md` «Дегенерированный вход» п.4: граница пиннится ОТДЕЛЬНЫМ входом.
scenario_facts_note_exact_threshold() {
  local d="${TMP_BASE}/facts16"
  build_good_fixture "${d}"
  mkdir -p "${d}/docs/plans"
  { echo "# фактура ровно на пороге"
    for i in $(seq 1 20); do echo "- см. \`crates/journal/src/lib.rs:${i}\`"; done
  } > "${d}/docs/plans/exactly-twenty.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && echo "${out}" | grep -q 'NOTE  \[H-FACTS\].*exactly-twenty\.md'; then
    pass "H-FACTS-16 (ровно 20 утверждений — граница порога): NOTE есть, exit=${rc}"
  else
    fail "H-FACTS-16: ОЖИДАЛСЯ NOTE про exactly-twenty.md при exit=0 — граница порога не держится (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- `C-117` F-15 (NOTE, стаб S19): обязательность хвоста `-->` не была запиннена.
# Оба исхода громкие (незакрытый маркер даёт FAIL «НЕ распарсен» через FACTS_DECL_RE), но
# поведение всё равно должно быть зафиксировано — иначе снятие хвоста молча меняет контракт.
scenario_facts_marker_unterminated_is_named() {
  local d="${TMP_BASE}/facts15"
  build_good_fixture "${d}"
  mk_plan_with_dead_ref "${d}" '<!-- FACTS: audited_head=1111111111111111111111111111111111111111 collected=2026-08-02'
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if ! echo "${out}" | grep -q 'GHOSTPLAN' \
     && echo "${out}" | grep -q 'FAIL  \[H-FACTS-SHA\].*facts\.md.*НЕ распарсен'; then
    pass "H-FACTS-15 (маркер без закрывающего хвоста): опт-ина нет, названо явно, exit=${rc}"
  else
    fail "H-FACTS-15: ОЖИДАЛСЯ FAIL про нераспарсенный маркер и отсутствие опт-ина (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- `C-117` F-17 (CONCERNS): дыра F-1 воспроизводилась ОДНИМ УРОВНЕМ ГЛУБЖЕ — обход
# `check_facts_sha` шёл верхним уровнем, `is_excluded` и NOTE-проверка рекурсивны.
scenario_facts_subdir_is_scanned() {
  local d="${TMP_BASE}/facts17"
  build_good_fixture "${d}"
  fixture_commit_base "${d}" >/dev/null
  mkdir -p "${d}/docs/plans/sub"
  { echo '<!-- FACTS: audited_head=1111111111111111111111111111111111111111 collected=2026-08-02 -->'
    echo "# фактура в подкаталоге с выдуманной ревизией"
    echo "- см. \`crates/journal/src/lib.rs:1\`"
  } > "${d}/docs/plans/sub/fake-rev.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[H-FACTS-SHA\].*sub/fake-rev\.md.*НЕТ вовсе'; then
    pass "H-FACTS-17 (подкаталог + выдуманная ревизия): судится наравне с верхним уровнем, exit=${rc}"
  else
    fail "H-FACTS-17: ОЖИДАЛСЯ FAIL про sub/fake-rev.md — обход не рекурсивен (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- `C-117` F-18 (CONCERNS): один байт cp1251 в голове — и документ с ВАЛИДНЫМ маркером
# выпадал из ВСЕХ проверок молча. Python декодирует чанком: битый байт строки 2 валит
# итерацию ДО отдачи строки 1. Ровно тот «молчаливый даунгрейд», устранение которого эта
# ветка объявила своим достижением.
scenario_facts_non_utf8_head_is_named() {
  local d="${TMP_BASE}/facts18"
  build_good_fixture "${d}"
  fixture_commit_base "${d}" >/dev/null
  mkdir -p "${d}/docs/plans"
  { echo "<!-- FACTS: audited_head=$(fixture_head_sha "${d}") collected=2026-08-02 -->"
    printf '# \x96 битый байт из cp1251-буфера\n'
    echo "Ссылка на \`docs/GHOSTPLAN.md\` — файла нет."
  } > "${d}/docs/plans/broken-encoding.md"
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -ne 0 ] && echo "${out}" | grep -q 'FAIL  \[H-FACTS-SHA\].*broken-encoding\.md.*не читается как UTF-8'; then
    pass "H-FACTS-18 (не-UTF-8 байт в голове): назван, не молчание, exit=${rc}"
  else
    fail "H-FACTS-18: ОЖИДАЛСЯ FAIL [H-FACTS-SHA] про нечитаемую голову (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

# --- `C-117` F-12 (REJECT, стаб S16): комментарий барьера ОБЕЩАЕТ, что механика
# переиспользована «включая MERGE_HEAD внутри --merge-preview», но ни один сценарий не
# строил merge-состояние с маркером — стаб `refs = ["HEAD"]` проходил пробу целиком.
# Радиус — ложное КРАСНОЕ в режиме, где джоб гоняет предмет чаще всего: ревизия, вошедшая
# в дерево только через сторону слияния, объявлялась орфаном, и это блокировало бы ВСЕ
# merge'и репозитория — класс, которым `A-010` §H мотивирует направление отказа.
scenario_facts_sha_merge_head_side() {
  # каталог именуется по СВОЕМУ номеру: `facts12` занят сценарием malformed-маркера, и
  # переиспользование давало чужую фикстуру в прогоне — поймано первым же запуском.
  local d="${TMP_BASE}/facts19"
  build_good_fixture "${d}"
  local base_sha main_sha
  base_sha="$(fixture_commit_base "${d}")"
  # ветка отстаёт от main; ревизия сбора появляется ТОЛЬКО на стороне main
  git -C "${d}" checkout -q -b feature-side
  echo "работа ветки" > "${d}/side.txt"
  git -C "${d}" add side.txt
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "ветка: своя работа"
  git -C "${d}" checkout -q "${base_sha}"
  git -C "${d}" checkout -q -B main-side
  echo "работа main" > "${d}/on-main.txt"
  mkdir -p "${d}/docs/plans"
  git -C "${d}" add on-main.txt
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "main: коммит, который станет ревизией сбора"
  main_sha="$(git -C "${d}" rev-parse HEAD)"
  { echo "<!-- FACTS: audited_head=${main_sha} collected=2026-08-02 -->"
    echo "# фактура, собранная на стороне main"
    echo "- см. \`crates/journal/src/lib.rs:1\`"
  } > "${d}/docs/plans/on-main.md"
  git -C "${d}" add docs/plans/on-main.md
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -q -m "main: маркированный план"
  main_sha="$(git -C "${d}" rev-parse HEAD)"
  # состояние merge-preview: слияние приостановлено, MERGE_HEAD существует
  git -C "${d}" checkout -q feature-side
  git -C "${d}" -c user.name=test -c user.email=test@test.local merge --no-commit --no-ff main-side >/dev/null 2>&1 || true
  # `C-117` F-21: в merge-состоянии дизъюнкция refs ДВУСТОРОННЯЯ (HEAD ∪ MERGE_HEAD), а
  # сценарий пиннил только MERGE_HEAD-край: стаб `canonical_refs(root)[-1:]` проходил все 60,
  # в обычном режиме будучи неотличим от честного. Второй маркированный план называет ревизию
  # СТОРОНЫ ВЕТКИ — живой кейс PR, который везёт и фактуру, и её ревизию.
  local branch_sha
  branch_sha="$(git -C "${d}" rev-parse HEAD)"
  { echo "<!-- FACTS: audited_head=${branch_sha} collected=2026-08-02 -->"
    echo "# фактура, собранная на стороне ветки"
    echo "- см. \`crates/journal/src/lib.rs:2\`"
  } > "${d}/docs/plans/on-branch.md"
  # setup-guard: сценарий обязан тестировать ИМЕННО merge-состояние, а не обычное дерево
  setup_ok=0
  git -C "${d}" rev-parse --verify -q MERGE_HEAD >/dev/null 2>&1 && setup_ok=1
  if [ "${setup_ok}" -ne 1 ]; then
    fail "H-FACTS-19: setup не состоялся — MERGE_HEAD отсутствует, сценарий тестировал бы не то"
    return
  fi
  # и ревизия обязана быть НЕдостижима от одного HEAD — иначе сценарий вакуумен
  if git -C "${d}" merge-base --is-ancestor "${main_sha}" HEAD >/dev/null 2>&1; then
    fail "H-FACTS-19: setup не состоялся — ревизия достижима от HEAD, стаб S16 такой вход не отличит"
    return
  fi
  local out rc
  out="$(run_verify "${d}")"; rc=$?
  if [ "${rc}" -eq 0 ] && ! echo "${out}" | grep -q 'FAIL  \[H-FACTS-SHA\]'; then
    pass "H-FACTS-19 (ревизии с ОБЕИХ сторон слияния — HEAD и MERGE_HEAD): ложного красного нет, exit=${rc}"
  else
    fail "H-FACTS-19: ОЖИДАЛСЯ exit=0 без FAIL [H-FACTS-SHA] — MERGE_HEAD-проводка не работает (exit=${rc}):"
    echo "${out}" | sed 's/^/      /'
  fi
}

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
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -qam "base: правит ту же строку §1"

  git -C "${d}" checkout -q "${branch_tip}"
  sed -i 's#Компонент foo реализован и работает#Компонент foo реализован и полностью протестирован#' "${d}/docs/DESIGN.md"
  git -C "${d}" -c user.name=test -c user.email=test@test.local commit -qam "branch: правит ту же строку §1 иначе"

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
scenario_facts_marked_plan_is_checked
scenario_facts_unmarked_plan_is_excluded
scenario_facts_marker_in_prose_ignored
scenario_facts_note_on_silent_plan
scenario_facts_note_threshold_pinned_below
scenario_facts_marker_without_head_not_opted_in
scenario_facts_marked_plan_checked_by_check3
scenario_archive_exclusion_still_holds
scenario_verdict_class_dead_doc_ref_excluded
scenario_verdict_class_broken_section_ref_excluded
scenario_verdict_class_does_not_leak_to_reports
scenario_verdict_class_declaration_names_all_dirs
scenario_verdict_class_declaration_equals_tuple
scenario_verdict_class_critiques_pinned_for_check3
scenario_verdict_class_arbitration_pinned_for_check3
scenario_facts_marker_on_last_head_line_counts
scenario_facts_sha_fake_fails
scenario_facts_sha_orphan_fails
scenario_facts_malformed_marker_is_not_silent
scenario_facts_marked_plan_gets_no_note
scenario_facts_sha_non_git_is_setup_guard
scenario_facts_note_exact_threshold
scenario_facts_marker_unterminated_is_named
scenario_facts_subdir_is_scanned
scenario_facts_non_utf8_head_is_named
scenario_facts_sha_merge_head_side
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
