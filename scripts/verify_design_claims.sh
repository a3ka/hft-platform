#!/usr/bin/env bash
# verify_design_claims.sh — машинная проверка того, что docs/DESIGN.md говорит правду
# о состоянии кода.
#
# Мотивация (docs/ORCHESTRATION-STATE.md, ночь 31.07→01.08): за одни сутки мастер-документ
# ЧЕТЫРЕЖДЫ солгал о состоянии кода, каждый раз ПО-РАЗНОМУ:
#   C-042 F-1 — §22 заявляла покрытие RK-I = 3 оракула, фактически 0 (упоминания —
#               комментарии-аналогии в чужих крейтах, два прямо отрицают, что они гейт);
#   C-041 Ф1  — §22 заявляла VN-I = 0 тестов, фактически RED-суита из 40 тестов;
#   R-004 Б3  — §18 описывал механизм изоляции как СУЩЕСТВУЮЩИЙ, в коде — пусто;
#   R-011 Б-1 — §10 объявил фазу P2.5 пройденной, а её ворота выполненными; неверно ни то,
#               ни другое;
#   R-013 Б-2/Б-3 — ПЯТЫЙ класс, другой: гейт прогнан на ветке (PASS), но `main` ушёл
#               вперёд за те же сутки (M-50 добавил оракул JR-I-9, алертинг смержен) — на
#               merge-цели ТЕ ЖЕ числа стали ложными. Документ был правдив на ветке и стал
#               ложью в момент слияния; прогон на ветке необходим, но не достаточен.
# Ошибки разнонаправленные (завышение/занижение/ложное существование/ложное «пройдено») —
# значит дело не в невнимательности к конкретной строке, а в ОТСУТСТВИИ проверки как класса.
# Правило «сверять замером, а не переносить из прежних текстов» (testing.md) само нарушалось
# четыре раза подряд автором правила — правило, держащееся на добросовестности, не работает.
#
# СЕМЬ проверок (каждая — PASS/FAIL/INFO с причиной; INFO = не применимо/не проверяется
# машинно на этой редакции документа — НЕ считается ни PASS, ни FAIL):
#   1. Каждое `[ЕСТЬ]`, стоящее в ОТДЕЛЬНОЙ ЯЧЕЙКЕ-СТОЛБЦЕ СОСТОЯНИЯ markdown-таблицы
#      (нормативное утверждение о готовности), сопровождено пруфом, и пруф СУЩЕСТВУЕТ.
#      Пруф — любая из равноправных форм: путь к файлу/каталогу/тесту, ссылка на milestone
#      (`M-NN` → `milestones/M-NN-*.md`), ссылка на вердикт (`C-NNN` → `research/critiques/
#      C-NNN-*.md`, `R-NNN` → `research/reviews/R-NNN-*.md`), голая ссылка на документ
#      (`` `NN-name.md` ``). Нет пруфа рядом → FAIL «не проверяемо». Пруф есть, но не
#      существует → FAIL «заявлено существующим, отсутствует». Уточнено 2026-08-01
#      (docs/design-evolution дал 12 FAIL, большинство ложные): `[ЕСТЬ]` внутри
#      ASCII-схемы (блок с `│┌└─` или внутри ``` ``` ```-фенса) или в прозе/списке —
#      это картинка/отсылка к уже описанному, а не отдельное нормативное утверждение;
#      пруф там НЕ требуется и не проверяется вовсе.
#   2. Таблица покрытия инвариантов §22 (колонки «Заявлено»/«В оракулах») сверяется с
#      РЕАЛЬНЫМ подсчётом уникальных идентификаторов `XX-I-N`, привязанных к тестам в
#      `crates/**/tests/**`. Анти-плацебо (урок C-042 F-1): упоминание в комментарии
#      ЧУЖОГО крейта — не оракул. Для семейств с документированным домашним крейтом
#      (testing.md: RK-I → `crates/risk`/`crates/killswitch`) — упоминания вне него не
#      считаются НИКОГДА, независимо от формулировки. Для остальных — эвристика по
#      словам-маркерам аналогии/отрицания («зеркало», «тот же принцип», «НЕ риск-гейт» и
#      т.п.). Печатаются ОБА числа (loose = любое упоминание, strict = после фильтра) —
#      честнее ложной точности, когда однозначно отличить нельзя.
#   3. Ссылки `DESIGN.md §N`/`§N.M` по всему репозиторию (*.md/*.rs/*.sh, кроме
#      docs/archive/**, docs/plans/**) ведут на реально существующие разделы оглавления.
#   4. В docs/** нет ссылок вида `docs/<...>.md` на файлы, которых больше нет.
#   5. Статусы фаз §10, помеченные как пройденные (✅/ПРИНЯТО/...), не противоречат грубо
#      цитируемым milestone'ам (файл отсутствует ИЛИ STATUS явно открытый). Не полная
#      автоматизация — что не проверяется машинно, помечается INFO, не выдаётся за PASS.
#   6. Инцидент C-044 (ретро-документ docs/rfc/CT-RFC-05-*.md процитировал 3 из 4
#      несуществующих в main SHA как «подтверждено коммитами») — machинная проверка: КАЖДЫЙ
#      hex-токен в backtick'ах (7-40 символов) в docs/DESIGN.md И docs/rfc/**.md, стоящий в
#      ТОМ ЖЕ markdown-параграфе (блок строк без пустой строки внутри — переживает перенос
#      строки внутри одного предложения), что и слово-маркер контекста коммита («коммит...»,
#      «merge», «мёрж...»/«мерж...», «sha»), обязан (а) существовать как git-объект
#      (`git cat-file -e <sha>^{commit}`) И (б) входить в историю HEAD (или MERGE_HEAD внутри
#      --merge-preview) — `git merge-base --is-ancestor`. Анти-плацебо (C-044 F1, реальный
#      случай): (а) без (б) — плацебо, орфан-коммит с заброшенной/несмёрженной ветки
#      (`ffedc10`/`6a2c331`/`67b6159` реально существовали как git-объекты на ветке
#      `engine/M-35-arms`, `git cat-file -e` проходил, но они не входили в ancestry
#      `origin/main` — только `--is-ancestor` это ловит). Внутри ``` ```-фенсов не
#      проверяется (пример кода, не нормативная ссылка). Нет пруфа-контекста рядом → токен
#      не трогается вовсе (это не обязательно SHA — не выдаём ложных FAIL на произвольный
#      hex-текст).
#   7. Пути вида `crates/...`/`docs/...`/`scripts/...`/`research/...`/`milestones/...`/
#      `.claude/...` в backtick'ах внутри docs/rfc/**.md — каждый обязан существовать в
#      дереве репозитория (`test -e`). Glob/brace-паттерны в тексте (содержат `*`/`?`/`{`/`}`
#      — например `crates/contracts/**`, `crates/venue-*`,
#      `crates/contracts/fixtures/{valid,invalid}`) — НЕ литеральные пути, пропускаются.
#      Проверки 3 (ссылки `DESIGN.md §N`) и 4 (мёртвые `docs/*.md`-ссылки) уже репо-/
#      docs-шире (полный `os.walk` вне docs/archive/**, docs/plans/**) и потому УЖЕ
#      покрывают docs/rfc/** без отдельного кода — проверено: обе проверки реально ходят по
#      docs/rfc/*.md наравне с остальными *.md/*.rs/*.sh.
#
# Setup-guard: docs/DESIGN.md не найден/пуст/оглавление не парсится/грепа не может быть
# пустым (§22-таблица найдена, но 0 строк-семейств; docs/** без единой docs/*.md ссылки) →
# FAIL, не молчаливый PASS (урок M-40: POSIX-awk падал на скобках, тело извлекалось пустым,
# грep ничего не находил, гейт молча PASS'ил). Здесь литеральные regex на строках, без
# POSIX-awk парсинга сигнатур — тот класс дефекта не воспроизводится, но тот же принцип:
# каждый парсер обязан ЗНАТЬ, когда он ничего не нашёл, и это FAIL, а не пустой отчёт.
#
# Использование:
#   scripts/verify_design_claims.sh [ROOT]
#   ROOT по умолчанию — корень репозитория (родитель scripts/). Явный ROOT — для self-test
#   (scripts/tests/red_verify_design_claims.sh), который гоняет синтетические копии
#   документа во временных каталогах, НЕ трогая реальный docs/DESIGN.md.
#
#   scripts/verify_design_claims.sh --merge-preview <base-ref> [ROOT]
#   Режим слияния (R-013): проверяет НЕ текущее дерево ROOT, а результат
#   `<base-ref>` + HEAD(ROOT) — то дерево, куда документ реально попадёт после merge.
#   Механика: временный `git worktree add --detach` из <base-ref>, затем
#   `git merge --no-commit --no-ff <HEAD ROOT'а>` внутри него; движок читает получившийся
#   каталог. ROOT ОБЯЗАН быть git-репозиторием с разрешимым HEAD; <base-ref> — любой
#   git-ref, разрешимый из ROOT (`origin/main`, ветка, SHA). Временный worktree удаляется
#   по выходу (успех/провал/сигнал — trap EXIT), реальный ROOT не трогается: ни коммитов,
#   ни смены HEAD/ветки в нём не происходит.
#   Setup-guard: ROOT не git-репозиторий, <base-ref> не резолвится, HEAD не резолвится,
#   worktree не собрался, слияние КОНФЛИКТУЕТ — каждый из этих случаев даёт явный
#   `FAIL [SETUP]` с описанием причины и завершает выполнение; молчаливого PASS при
#   несобранном превью не бывает (тот же принцип setup-guard, что и у остальных проверок
#   ниже — гейт обязан ЗНАТЬ, когда не смог проверить, и это FAIL, а не пустой отчёт).
#
# Никакого `cmd && echo PASS || echo FAIL` — вся агрегация (FAIL-счётчик, VERDICT,
# exit-код) сделана явно внутри движка (Python — надёжнее POSIX-awk на markdown-таблицах
# и Юникод-регулярках; сам движок ниже — единственное тело проверки, без промежуточных
# файлов вне этого скрипта per scope: Пишешь ТОЛЬКО verify_design_claims.sh +
# scripts/tests/red_verify_design_claims.sh).

set -uo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

MERGE_PREVIEW=0
BASE_REF=""
if [ "${1:-}" = "--merge-preview" ]; then
  MERGE_PREVIEW=1
  BASE_REF="${2:-}"
  if [ -z "${BASE_REF}" ]; then
    echo "FAIL  [SETUP] --merge-preview требует <base-ref> вторым аргументом (пример: --merge-preview origin/main)"
    echo
    echo "VERDICT: FAIL (1 нарушений)"
    exit 1
  fi
  shift 2
fi

SOURCE_ROOT="${1:-${SCRIPT_ROOT}}"

if [ "${MERGE_PREVIEW}" -eq 1 ]; then
  if ! git -C "${SOURCE_ROOT}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "FAIL  [SETUP] --merge-preview: '${SOURCE_ROOT}' — не git-репозиторий (или git недоступен), превью слияния собрать нельзя"
    echo
    echo "VERDICT: FAIL (1 нарушений)"
    exit 1
  fi
  HEAD_SHA="$(git -C "${SOURCE_ROOT}" rev-parse HEAD 2>/dev/null)"
  if [ -z "${HEAD_SHA}" ]; then
    echo "FAIL  [SETUP] --merge-preview: не удалось определить HEAD в '${SOURCE_ROOT}' (пустой репозиторий/нет коммитов?)"
    echo
    echo "VERDICT: FAIL (1 нарушений)"
    exit 1
  fi
  if ! git -C "${SOURCE_ROOT}" rev-parse --verify "${BASE_REF}^{commit}" >/dev/null 2>&1; then
    echo "FAIL  [SETUP] --merge-preview: base-ref '${BASE_REF}' не резолвится в '${SOURCE_ROOT}' — превью собрать не из чего"
    echo
    echo "VERDICT: FAIL (1 нарушений)"
    exit 1
  fi

  PREVIEW_DIR="$(mktemp -d "${TMPDIR:-/tmp}/verify-design-merge-preview.XXXXXX")"
  cleanup_merge_preview() {
    git -C "${SOURCE_ROOT}" worktree remove --force "${PREVIEW_DIR}" >/dev/null 2>&1
    rm -rf "${PREVIEW_DIR}" 2>/dev/null
  }
  trap cleanup_merge_preview EXIT

  if ! git -C "${SOURCE_ROOT}" worktree add --detach "${PREVIEW_DIR}" "${BASE_REF}" >/dev/null 2>&1; then
    echo "FAIL  [SETUP] --merge-preview: не удалось создать временный worktree из base-ref '${BASE_REF}' — превью слияния не собрано"
    echo
    echo "VERDICT: FAIL (1 нарушений)"
    exit 1
  fi

  if ! git -C "${PREVIEW_DIR}" -c user.name=verify-design-claims -c user.email=verify-design-claims@noreply.local \
        merge --no-commit --no-ff "${HEAD_SHA}" >/dev/null 2>&1; then
    git -C "${PREVIEW_DIR}" merge --abort >/dev/null 2>&1
    echo "FAIL  [SETUP] --merge-preview: слияние base-ref '${BASE_REF}' + HEAD (${HEAD_SHA}) КОНФЛИКТУЕТ — merge-цель не собирается автоматически, документ на ней не проверяем; разреши конфликт вручную и прогони обычный режим на результате"
    echo
    echo "VERDICT: FAIL (1 нарушений)"
    exit 1
  fi

  TARGET_ROOT="${PREVIEW_DIR}"
else
  TARGET_ROOT="${SOURCE_ROOT}"
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "FAIL  [SETUP] python3 не найден в PATH — гейт не может выполниться"
  echo
  echo "VERDICT: FAIL (1 нарушений)"
  exit 1
fi

# ВАЖНО: без `exec` (в отличие от прежней версии) — `--merge-preview` регистрирует
# cleanup-trap на EXIT (удаление временного worktree), а `exec` заменяет процесс bash и
# уносит с собой все trap'ы, не дав им сработать. Без merge-preview trap не зарегистрирован
# — поведение (stdout/exit-код) не меняется, просто shell-процесс на мгновение переживает
# python3 вместо замены им.
python3 - "${TARGET_ROOT}" <<'PYEOF'
#!/usr/bin/env python3
"""Движок verify_design_claims.sh. Читает docs/DESIGN.md под ROOT (argv[1]) и репозиторий
вокруг него, печатает PASS/FAIL/INFO построчно + финальный VERDICT, выходит с 0 (PASS)
или 1 (FAIL). Ничего не пишет, только читает."""
import os
import re
import subprocess
import sys

FAILED = 0


def pass_(check, msg):
    print(f"PASS  [{check}] {msg}")


def fail(check, msg):
    global FAILED
    FAILED += 1
    print(f"FAIL  [{check}] {msg}")


def info(check, msg):
    print(f"INFO  [{check}] {msg}")


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


# ---------------------------------------------------------------------------
# CHECK 1 — `[ЕСТЬ]` в ТАБЛИЦЕ СТАТУСОВ сопровождён существующим пруфом.
#
# Контекст решает, требуется ли пруф (уточнено 2026-08-01, см. комментарий сверху файла):
#   - строка markdown-таблицы, где `[ЕСТЬ]` стоит В ОТДЕЛЬНОЙ ЯЧЕЙКЕ-СТОЛБЦЕ состояния
#     (нормативное утверждение о готовности) → пруф ОБЯЗАТЕЛЕН;
#   - ASCII-схема (внутри ``` ```-фенса — независимо от синтаксиса внутри) и проза/списки
#     → пруф НЕ требуется и не проверяется вовсе (это картинка или отсылка к уже описанному,
#     а не отдельное утверждение).
# Формы пруфа — равноправны: путь/каталог/rust-path (как раньше), голая ссылка на документ
# (`NN-name.md`), ссылка на milestone (`M-NN`), ссылка на вердикт (`C-NNN`/`R-NNN`).
# ---------------------------------------------------------------------------

PATH_TOKEN_RE = re.compile(
    r"`([^`]*?/[^`]*?\.(?:rs|md|sh|toml|json|yml|yaml))`"                       # `path/to/file.ext`
    r"|`((?:crates|docs|scripts|research|milestones|contracts|\.claude)/[^`]*)`"  # `crates/...`, `docs/...`
    r"|`([A-Za-z_][A-Za-z0-9_:]*::[A-Za-z0-9_:]+)`"                              # `journal::stream`
)
BARE_DOC_RE = re.compile(r"`([A-Za-z0-9_.\-]+\.md)`")                            # `NN-name.md` (без слэша)
MILESTONE_TOKEN_RE = re.compile(r"\bM-(\d+)\b")                                  # M-NN (голым текстом)
CRITIQUE_TOKEN_RE = re.compile(r"\bC-(\d+)\b")                                   # C-NNN (research/critiques)
REVIEW_TOKEN_RE = re.compile(r"\bR-(\d+)\b")                                     # R-NNN (research/reviews)
MARKER_RE = re.compile(r"\[ЕСТЬ\]")
CELL_STATUS_RE = re.compile(r"^[*_\s]*\[ЕСТЬ\]")                                 # маркер — начало ячейки


def resolve_candidate(root, token):
    token = token.strip()
    if "/" in token:
        clean = token.rstrip(".,;:)")
        candidate = os.path.join(root, clean)
        return os.path.exists(candidate), clean
    if "::" in token:
        crate = token.split("::")[0]
        candidate = os.path.join(root, "crates", crate)
        return os.path.isdir(candidate), token
    return False, token


def resolve_bare_doc(root, name):
    if os.path.exists(os.path.join(root, "docs", name)):
        return True
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in (".git", "target", "node_modules")]
        if name in filenames:
            return True
    return False


def resolve_ref(root, subdir, prefix, num):
    d = os.path.join(root, subdir)
    if os.path.isdir(d):
        rx = re.compile(rf"^{prefix}-{re.escape(num)}(?!\d).*\.md$")
        for fn in os.listdir(d):
            if rx.match(fn):
                return True
    return False


def gather_proofs(root, line):
    """Все равноправные формы пруфа, найденные в СТРОКЕ таблицы (может включать
    несколько ячеек одной строки): (exists: bool, human_label: str) для каждой."""
    proofs = []
    for grp in PATH_TOKEN_RE.findall(line):
        for tok in grp:
            if tok:
                exists, resolved = resolve_candidate(root, tok)
                proofs.append((exists, f"путь `{resolved}`"))
    for m in BARE_DOC_RE.finditer(line):
        name = m.group(1)
        proofs.append((resolve_bare_doc(root, name), f"документ `{name}`"))
    for m in MILESTONE_TOKEN_RE.finditer(line):
        num = m.group(1)
        proofs.append((resolve_ref(root, "milestones", "M", num), f"milestone M-{num}"))
    for m in CRITIQUE_TOKEN_RE.finditer(line):
        num = m.group(1)
        proofs.append((resolve_ref(root, os.path.join("research", "critiques"), "C", num), f"вердикт C-{num}"))
    for m in REVIEW_TOKEN_RE.finditer(line):
        num = m.group(1)
        proofs.append((resolve_ref(root, os.path.join("research", "reviews"), "R", num), f"вердикт R-{num}"))
    return proofs


def compute_fence_lines(lines):
    """Строки (1-based) внутри ``` ```-фенсов, включая маркеры фенса — исключаются из
    поиска markdown-таблиц (пример синтаксиса таблицы в коде — не нормативная таблица)."""
    fence = set()
    in_fence = False
    for idx, line in enumerate(lines, start=1):
        if line.strip().startswith("```"):
            fence.add(idx)
            in_fence = not in_fence
            continue
        if in_fence:
            fence.add(idx)
    return fence


def parse_table_status_lines(lines, fence_lines):
    """{line_no: cells} для строк-ДАННЫХ markdown-таблиц (не заголовков, не
    разделителей), вне ``` ```-фенсов."""
    table_line_no = {}
    n = len(lines)

    def is_row(ln):
        return 1 <= ln <= n and ln not in fence_lines and TABLE_ROW_RE.match(lines[ln - 1])

    def is_sep(ln):
        return 1 <= ln <= n and ln not in fence_lines and TABLE_SEP_RE.match(lines[ln - 1])

    i = 1
    while i <= n:
        if is_row(i) and is_sep(i + 1):
            i += 2
            while is_row(i):
                cells = [c.strip() for c in lines[i - 1].strip().strip("|").split("|")]
                table_line_no[i] = cells
                i += 1
        else:
            i += 1
    return table_line_no


def check1(root, design_path, design_lines):
    rel_design = os.path.relpath(design_path, root)
    fence_lines = compute_fence_lines(design_lines)
    table_line_no = parse_table_status_lines(design_lines, fence_lines)

    n_markers = n_table = n_exempt = n_without_proof = n_broken_proof = n_ok = 0

    for i, line in enumerate(design_lines, start=1):
        for _m in MARKER_RE.finditer(line):
            n_markers += 1
            cells = table_line_no.get(i)
            in_status_cell = cells is not None and any(CELL_STATUS_RE.match(c) for c in cells)
            if not in_status_cell:
                # ASCII-схема (внутри ```-фенса) или проза/список — пруф не требуется.
                n_exempt += 1
                continue

            n_table += 1
            proofs = gather_proofs(root, line)
            if not proofs:
                n_without_proof += 1
                fail(
                    "1-ЕСТЬ",
                    f"{rel_design}:{i}: `[ЕСТЬ]` в таблице статусов без пруфа "
                    f"(путь/milestone/вердикт/документ) рядом — утверждение не проверяемо. "
                    f"Строка: {line.strip()[:160]}",
                )
                continue
            line_ok = True
            for exists, label in proofs:
                if not exists:
                    line_ok = False
                    n_broken_proof += 1
                    fail(
                        "1-ЕСТЬ",
                        f"{rel_design}:{i}: `[ЕСТЬ]` в таблице статусов заявляет {label}, "
                        f"он ОТСУТСТВУЕТ в дереве репозитория",
                    )
            if line_ok:
                n_ok += 1

    if n_markers == 0:
        info("1-ЕСТЬ", "маркеров [ЕСТЬ] в docs/DESIGN.md не найдено — проверка неприменима на этой редакции документа")
    elif n_table == 0:
        info(
            "1-ЕСТЬ",
            f"{n_markers} маркеров [ЕСТЬ], все вне таблиц статусов (ASCII-схемы/проза) — "
            f"пруф не требуется, проверка неприменима",
        )
    elif n_without_proof == 0 and n_broken_proof == 0:
        pass_(
            "1-ЕСТЬ",
            f"все {n_table} маркеров [ЕСТЬ] в таблицах статусов сопровождены существующим "
            f"пруфом ({n_exempt} вне таблиц — ASCII-схемы/проза, не проверялись)",
        )
    else:
        fail(
            "1-ЕСТЬ",
            f"итог по таблицам статусов: {n_table} маркеров, {n_ok} с валидным пруфом, "
            f"{n_without_proof} без пруфа, {n_broken_proof} пруфов не существуют "
            f"({n_exempt} маркеров вне таблиц — не проверялись)",
        )


# ---------------------------------------------------------------------------
# Markdown-таблицы (общий парсер для check2/check5)
# ---------------------------------------------------------------------------

TABLE_ROW_RE = re.compile(r"^\|(.+)\|\s*$")
TABLE_SEP_RE = re.compile(r"^\|[\s:|-]+\|\s*$")


def parse_md_tables(text):
    lines = text.splitlines()
    tables = []
    i, n = 0, len(lines)
    while i < n:
        if TABLE_ROW_RE.match(lines[i]) and i + 1 < n and TABLE_SEP_RE.match(lines[i + 1]):
            start = i
            rows = [[c.strip() for c in lines[i].strip().strip("|").split("|")]]
            i += 2
            while i < n and TABLE_ROW_RE.match(lines[i]):
                rows.append([c.strip() for c in lines[i].strip().strip("|").split("|")])
                i += 1
            tables.append((start + 1, rows))
        else:
            i += 1
    return tables


# ---------------------------------------------------------------------------
# CHECK 2 — числа покрытия §22 совпадают с грепом по тестам
# ---------------------------------------------------------------------------

NEGATION_PHRASES = [
    "зеркало", "тот же принцип", "аналог", "не риск-гейт", "не является риск",
    "справочно", "не оракул", "не путать", "не риск", "аналогия",
    "как пример", "прямой предок", "предок",
]

# Семейства с ЯВНО документированным домашним крейтом (BINDING правило проекта, не
# изобретено этим скриптом — testing.md: "RK-I-1..10 ... Живут в `crates/risk/tests/` и
# `crates/killswitch/tests/`. Только architect пишет/меняет."). Урок C-042 F-1: упоминание
# в чужом крейте — комментарий-аналогия, даже без слова-маркера ("прямой предок RK-I-8" в
# crates/sim не содержит "зеркало"/"аналог", но остаётся анализом ЧУЖОГО крейта). Домашний
# крейт — надёжнее перебора формулировок словами.
HOME_CRATES = {
    "RK-I": ["risk", "killswitch"],
}

FAM_ID_RE = re.compile(r"\b([A-ZА-Я]{2,5}-I)-(\d+)\b")
FAM_ROW_RE = re.compile(r"^[A-ZА-Я]{2,5}-I$")


def grep_test_dirs(root, crates_filter=None):
    out = []
    crates_dir = os.path.join(root, "crates")
    if not os.path.isdir(crates_dir):
        return out
    names = crates_filter if crates_filter is not None else sorted(os.listdir(crates_dir))
    for crate in names:
        tests_dir = os.path.join(crates_dir, crate, "tests")
        if not os.path.isdir(tests_dir):
            continue
        for dirpath, _dirnames, filenames in os.walk(tests_dir):
            for fn in filenames:
                if not fn.endswith(".rs"):
                    continue
                fpath = os.path.join(dirpath, fn)
                try:
                    with open(fpath, encoding="utf-8") as f:
                        for lineno, line in enumerate(f, start=1):
                            out.append((fpath, lineno, line))
                except (UnicodeDecodeError, OSError):
                    continue
    return out


def check2(root, design_text):
    tables = parse_md_tables(design_text)
    coverage_table = None
    for _start_line, rows in tables:
        header = [c.lower() for c in rows[0]]
        if any("заявлен" in c for c in header) and any("оракул" in c for c in header):
            coverage_table = rows
            break

    if coverage_table is None:
        info("2-ПОКРЫТИЕ", "таблица покрытия инвариантов (Заявлено/В оракулах, §22) не найдена в docs/DESIGN.md — проверка неприменима")
        return

    header = coverage_table[0]
    try:
        fam_idx = 0
        declared_idx = next(i for i, c in enumerate(header) if "заявлен" in c.lower())
        oracles_idx = next(i for i, c in enumerate(header) if "оракул" in c.lower())
    except StopIteration:
        fail("2-ПОКРЫТИЕ", "таблица покрытия найдена, но колонки Заявлено/В оракулах не распознаны — setup-guard")
        return

    test_lines_all = grep_test_dirs(root)
    families_checked = 0

    for row in coverage_table[1:]:
        if len(row) <= max(fam_idx, declared_idx, oracles_idx):
            continue
        fam = row[fam_idx].strip().strip("*")
        if not FAM_ROW_RE.match(fam):
            continue
        declared_raw = row[declared_idx].strip()
        oracles_raw = row[oracles_idx].strip()
        declared_m = re.search(r"\d+", declared_raw)
        oracles_m = re.search(r"\d+", oracles_raw)
        if declared_m is None:
            fail("2-ПОКРЫТИЕ", f"§22: семейство {fam} — колонка 'Заявлено' не число ({declared_raw!r}), сверка невозможна")
            continue

        families_checked += 1

        if fam in HOME_CRATES:
            home_lines = grep_test_dirs(root, crates_filter=HOME_CRATES[fam])
            strict_ids = {m.group(2) for _f, _l, line in home_lines for m in FAM_ID_RE.finditer(line) if m.group(1) == fam}
            loose_ids = {m.group(2) for _f, _l, line in test_lines_all for m in FAM_ID_RE.finditer(line) if m.group(1) == fam}
        else:
            loose_ids, strict_ids = set(), set()
            for _f, _l, line in test_lines_all:
                for m in FAM_ID_RE.finditer(line):
                    if m.group(1) != fam:
                        continue
                    loose_ids.add(m.group(2))
                    if not any(neg in line.lower() for neg in NEGATION_PHRASES):
                        strict_ids.add(m.group(2))

        loose_n, strict_n = len(loose_ids), len(strict_ids)

        if oracles_m is None:
            fail(
                "2-ПОКРЫТИЕ",
                f"§22: семейство {fam} — колонка 'В оракулах' не число ({oracles_raw!r}); "
                f"реальный замер: loose={loose_n} (любое упоминание), strict={strict_n} "
                f"(без строк-аналогий/отрицаний, анти-плацебо C-042 F-1) — документ обязан "
                f"называть оба числа явно в таблице, не отсылать к сноске",
            )
            continue

        claimed = int(oracles_m.group())
        if claimed != strict_n:
            fail(
                "2-ПОКРЫТИЕ",
                f"§22: семейство {fam} — документ заявляет 'в оракулах'={claimed}, "
                f"реальный замер (анти-плацебо) strict={strict_n}, loose (любое упоминание)={loose_n}",
            )
        else:
            pass_("2-ПОКРЫТИЕ", f"§22: {fam} — заявлено={row[declared_idx].strip()}, в оракулах={claimed} — подтверждено замером (loose={loose_n})")

    if families_checked == 0:
        fail("2-ПОКРЫТИЕ", "таблица §22 найдена, но ни одной строки-семейства (XX-I) не распознано — грепа не может быть пустым (setup-guard)")


# ---------------------------------------------------------------------------
# CHECK 3 — ссылки DESIGN.md §N ведут на существующие разделы
# ---------------------------------------------------------------------------

SECTION_HEADING_RE = re.compile(r"^#{2,3}\s+§?(\d+(?:\.\d+)?)\.?\s+\S")
DESIGN_REF_RE = re.compile(r"DESIGN\.md\s*§(\d+(?:\.\d+)?)")


def check3(root, design_text):
    valid_sections = {m.group(1) for line in design_text.splitlines() if (m := SECTION_HEADING_RE.match(line))}

    if not valid_sections:
        fail("3-ССЫЛКИ", "не удалось извлечь ни одного раздела (§N) из оглавления docs/DESIGN.md — setup-guard")
        return

    exts = (".md", ".rs", ".sh")
    excluded = ("docs/archive/", "docs/plans/")
    n_refs = n_bad = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in (".git", "target", "node_modules")]
        for fn in filenames:
            if not fn.endswith(exts):
                continue
            fpath = os.path.join(dirpath, fn)
            relpath = os.path.relpath(fpath, root)
            if any(relpath.startswith(ex) for ex in excluded):
                continue
            try:
                with open(fpath, encoding="utf-8") as f:
                    for lineno, line in enumerate(f, start=1):
                        for m in DESIGN_REF_RE.finditer(line):
                            n_refs += 1
                            sec = m.group(1)
                            if sec not in valid_sections:
                                n_bad += 1
                                fail("3-ССЫЛКИ", f"{relpath}:{lineno}: ссылка `DESIGN.md §{sec}` — раздела §{sec} нет в оглавлении docs/DESIGN.md")
            except (UnicodeDecodeError, OSError):
                continue

    if n_refs == 0:
        info("3-ССЫЛКИ", "ссылок вида `DESIGN.md §N` в репозитории не найдено")
    elif n_bad == 0:
        pass_("3-ССЫЛКИ", f"все {n_refs} ссылок `DESIGN.md §N` указывают на существующие разделы")


# ---------------------------------------------------------------------------
# CHECK 4 — нет ссылок на удалённые docs/*.md
# ---------------------------------------------------------------------------

DOC_FILE_REF_RE = re.compile(r"\bdocs/[A-Za-z0-9_.\-/]+\.md\b")


def check4(root):
    docs_dir = os.path.join(root, "docs")
    if not os.path.isdir(docs_dir):
        fail("4-МЁРТВЫЕ-ФАЙЛЫ", "каталог docs/ отсутствует — setup-guard")
        return
    excluded = ("docs/archive/", "docs/plans/")
    n_refs = n_bad = 0
    for dirpath, dirnames, filenames in os.walk(docs_dir):
        dirnames[:] = [d for d in dirnames if d not in (".git",)]
        for fn in filenames:
            if not fn.endswith(".md"):
                continue
            fpath = os.path.join(dirpath, fn)
            relpath = os.path.relpath(fpath, root)
            if any(relpath.startswith(ex) for ex in excluded):
                continue
            try:
                with open(fpath, encoding="utf-8") as f:
                    for lineno, line in enumerate(f, start=1):
                        for m in DOC_FILE_REF_RE.finditer(line):
                            ref = m.group(0)
                            n_refs += 1
                            if not os.path.isfile(os.path.join(root, ref)):
                                n_bad += 1
                                fail("4-МЁРТВЫЕ-ФАЙЛЫ", f"{relpath}:{lineno}: ссылка на `{ref}` — файл не существует")
            except (UnicodeDecodeError, OSError):
                continue

    if n_refs == 0:
        fail("4-МЁРТВЫЕ-ФАЙЛЫ", "в docs/** не найдено ни одной ссылки вида docs/*.md — грепа не может быть пустым (setup-guard)")
    elif n_bad == 0:
        pass_("4-МЁРТВЫЕ-ФАЙЛЫ", f"все {n_refs} ссылок вида docs/*.md указывают на существующие файлы")


# ---------------------------------------------------------------------------
# CHECK 5 — статусы фаз §10 не противоречат milestone'ам (грубая проверка)
# ---------------------------------------------------------------------------

PHASE_DONE_MARK_RE = re.compile(r"✅|ПРИНЯТО|ЗАВЕРШЕН|ПРОЙДЕН")
MILESTONE_REF_RE = re.compile(r"\bM-(\d+)\b")
OPEN_STATUSES = {"OPEN", "PROPOSED", "IN_PROGRESS", "BLOCKED", "PLANNED"}


def check5(root, design_text):
    m10 = re.search(r"^##\s+§?10\.?\s.*$", design_text, re.MULTILINE)
    if not m10:
        info("5-ФАЗЫ", "раздел §10 (фазовый роадмап) не найден в docs/DESIGN.md — проверка неприменима")
        return
    start = m10.end()
    m_next = re.search(r"^##\s+§?11\b", design_text[start:], re.MULTILINE)
    section10 = design_text[start:start + m_next.start()] if m_next else design_text[start:]

    tables = parse_md_tables(section10)
    if not tables:
        fail("5-ФАЗЫ", "§10 найден, но ни одной таблицы фаз в нём не распознано — setup-guard")
        return

    milestones_dir = os.path.join(root, "milestones")
    milestone_status = {}
    if os.path.isdir(milestones_dir):
        for fn in os.listdir(milestones_dir):
            mm = re.match(r"M-(\d+).*\.md$", fn)
            if not mm:
                continue
            try:
                text = read(os.path.join(milestones_dir, fn))
            except OSError:
                continue
            sm = re.search(r"STATUS:\s*\**([A-ZА-Я_]+)", text)
            milestone_status[mm.group(1)] = sm.group(1) if sm else None

    n_phase_rows = n_bad = 0
    for _start_line, rows in tables:
        header = [c.lower() for c in rows[0]]
        if not any("фаза" in c for c in header):
            continue
        for row in rows[1:]:
            row_text = " | ".join(row)
            if "🚧" in row_text or "PROPOSED" in row_text.upper():
                continue
            if not PHASE_DONE_MARK_RE.search(row_text):
                continue
            n_phase_rows += 1
            phase_name = row[0].strip()
            mrefs = set(MILESTONE_REF_RE.findall(row_text))
            if not mrefs:
                info("5-ФАЗЫ", f"фаза «{phase_name}» помечена пройденной, но строка не цитирует ни одного M-NN — не проверяется машинно")
                continue
            for mnum in mrefs:
                if mnum not in milestone_status:
                    n_bad += 1
                    fail("5-ФАЗЫ", f"фаза «{phase_name}» объявлена пройденной, цитирует M-{mnum}, но milestones/M-{mnum}-*.md отсутствует")
                elif milestone_status[mnum] in OPEN_STATUSES:
                    n_bad += 1
                    fail("5-ФАЗЫ", f"фаза «{phase_name}» объявлена пройденной, но M-{mnum} имеет STATUS={milestone_status[mnum]} (не закрыт)")

    if n_phase_rows == 0:
        info("5-ФАЗЫ", "в §10 не найдено строк, явно помеченных как пройденные (✅/ПРИНЯТО) — проверка неприменима")
    elif n_bad == 0:
        pass_("5-ФАЗЫ", f"{n_phase_rows} строк(и) фаз, помеченных пройденными, не противоречат цитируемым milestone'ам (грубая проверка)")


# ---------------------------------------------------------------------------
# CHECK 6 — цитируемые SHA (docs/DESIGN.md + docs/rfc/**.md) существуют в git-истории
# (C-044: 3 из 4 SHA в §4 CT-RFC-05-margin-inventory.md — орфаны вне ancestry origin/main)
# ---------------------------------------------------------------------------

SHA_TOKEN_RE = re.compile(r"`([0-9a-f]{7,40})`")
SHA_CONTEXT_RE = re.compile(r"коммит\w*|merge\b|мёрж\w*|мерж\w*|\bsha\b", re.IGNORECASE)


def compute_paragraphs(lines):
    """Блок-и (start,end), 1-based inclusive, строк без пустой строки внутри — markdown
    "параграф", переживает перенос предложения на следующую физическую строку."""
    paragraphs = []
    start = None
    for i, line in enumerate(lines, start=1):
        if line.strip() == "":
            if start is not None:
                paragraphs.append((start, i - 1))
                start = None
        else:
            if start is None:
                start = i
    if start is not None:
        paragraphs.append((start, len(lines)))
    return paragraphs


def git_commit_exists(root, sha):
    try:
        r = subprocess.run(
            ["git", "-C", root, "cat-file", "-e", f"{sha}^{{commit}}"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return r.returncode == 0
    except OSError:
        return False


def canonical_refs(root):
    """Ref-имена, ancestor которых у SHA достаточно, чтобы считать его 'реально попавшим
    в дерево ROOT', а не просто существующим где-то в локальной object-database. Обычный
    режим — HEAD. Внутри --merge-preview (git merge --no-commit --no-ff паузит слияние)
    добавляется MERGE_HEAD — второй родитель незакоммиченного слияния, иначе коммиты,
    реально входящие в merge через сторону исходного HEAD (не base-ref), дали бы ложный
    FAIL против одного лишь base-ref."""
    refs = ["HEAD"]
    r = subprocess.run(
        ["git", "-C", root, "rev-parse", "--verify", "-q", "MERGE_HEAD"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if r.returncode == 0:
        refs.append("MERGE_HEAD")
    return refs


def git_commit_is_ancestor_of_any(root, sha, refs):
    for ref in refs:
        r = subprocess.run(
            ["git", "-C", root, "merge-base", "--is-ancestor", sha, ref],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if r.returncode == 0:
            return True
    return False


def gather_sha_refs(root, path):
    """[(relpath, lineno, sha), ...] — токены-SHA в параграфах с контекстным маркером,
    вне ``` ```-фенсов."""
    try:
        text = read(path)
    except OSError:
        return []
    relpath = os.path.relpath(path, root)
    lines = text.splitlines()
    fence_lines = compute_fence_lines(lines)
    paragraphs = compute_paragraphs(lines)
    para_has_ctx = {}
    for (s, e) in paragraphs:
        para_text = "\n".join(lines[s - 1:e])
        has_ctx = bool(SHA_CONTEXT_RE.search(para_text))
        for ln in range(s, e + 1):
            para_has_ctx[ln] = has_ctx

    refs = []
    for i, line in enumerate(lines, start=1):
        if i in fence_lines or not para_has_ctx.get(i, False):
            continue
        for m in SHA_TOKEN_RE.finditer(line):
            refs.append((relpath, i, m.group(1)))
    return refs


def check6(root):
    targets = []
    design_path = os.path.join(root, "docs", "DESIGN.md")
    if os.path.isfile(design_path):
        targets.append(design_path)
    rfc_dir = os.path.join(root, "docs", "rfc")
    if os.path.isdir(rfc_dir):
        for fn in sorted(os.listdir(rfc_dir)):
            if fn.endswith(".md"):
                targets.append(os.path.join(rfc_dir, fn))

    all_refs = []
    for path in targets:
        all_refs.extend(gather_sha_refs(root, path))

    if not all_refs:
        info(
            "6-RFC-SHA",
            "в docs/DESIGN.md и docs/rfc/**.md не найдено цитат коммитов (SHA в контексте "
            "«коммит»/«merge»/«мёрж...») — проверка неприменима",
        )
        return

    try:
        r = subprocess.run(
            ["git", "-C", root, "rev-parse", "--is-inside-work-tree"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        git_ok = r.returncode == 0 and r.stdout.strip() == b"true"
    except OSError:
        git_ok = False

    if not git_ok:
        fail(
            "6-RFC-SHA",
            f"найдено {len(all_refs)} цитат коммитов в docs/DESIGN.md/docs/rfc/**.md, но "
            f"'{root}' не git-репозиторий (или git недоступен) — существование SHA "
            f"проверить нельзя (setup-guard)",
        )
        return

    refs = canonical_refs(root)
    n_bad = 0
    for relpath, lineno, sha in all_refs:
        # Анти-плацебо (C-044 F1): `git cat-file -e` ОДНОЙ проверкой не ловит орфан-SHA —
        # объект может реально существовать (коммит с заброшенной/несмёрженной ветки,
        # напр. `engine/M-35-arms` — ffedc10/6a2c331/67b6159 в реальном инциденте), просто
        # не входить в историю HEAD. Существование — необходимо, НЕ достаточно; ancestry —
        # решающая проверка.
        if not git_commit_exists(root, sha):
            n_bad += 1
            fail(
                "6-RFC-SHA",
                f"{relpath}:{lineno}: цитируется коммит `{sha}` — не найден в "
                f"git-объектах репозитория вовсе (`git cat-file -e {sha}^{{commit}}` провалился)",
            )
            continue
        if not git_commit_is_ancestor_of_any(root, sha, refs):
            n_bad += 1
            fail(
                "6-RFC-SHA",
                f"{relpath}:{lineno}: цитируется коммит `{sha}` — существует как "
                f"git-объект, но НЕ входит в историю {'/'.join(refs)} (орфан/несмёрженная "
                f"ветка — `git merge-base --is-ancestor {sha} {refs[0]}` провалился)",
            )

    if n_bad == 0:
        pass_(
            "6-RFC-SHA",
            f"все {len(all_refs)} цитат коммитов (docs/DESIGN.md + docs/rfc/**.md) "
            f"существуют И входят в историю {'/'.join(refs)}",
        )


# ---------------------------------------------------------------------------
# CHECK 7 — пути, процитированные в docs/rfc/**.md, существуют в дереве репозитория
# (C-044 F2: список мест правки занижен, но опечатка/несуществующий путь — тот же класс лжи)
# ---------------------------------------------------------------------------

RFC_PATH_TOKEN_RE = re.compile(
    r"`((?:crates|docs|scripts|research|milestones|\.claude)/[^`]*)`"
)
# НЕ заякорен на конец строки ($) — намеренно: реальный документ кладёт внутрь ОДНОЙ пары
# backtick'ов путь + произвольный "хвост" разных форм (`path.rs:301-315` — диапазон строк,
# `path.rs::func_name` — Rust-путь, `path.md §9` — секция того же документа через ПРОБЕЛ).
# Правило простое и надёжное: как только встретилась распознанная РАСШИРЕНИЕМ граница файла
# (первая `.rs`/`.md`/`.sh`/`.toml`/`.json`/`.yml`/`.yaml` слева направо), путь на этом
# заканчивается — всё после неё, ЛЮБОЙ формы, отбрасывается без попытки его разобрать.
RFC_PATH_EXT_RE = re.compile(r"^(.*?\.(?:rs|md|sh|toml|json|yml|yaml))")
RFC_PATH_LINEREF_TAIL_RE = re.compile(r":\d+(?:-\d+)?$")


def clean_rfc_path_token(raw):
    """`crates/x/y.rs:301-315` → `crates/x/y.rs`; `crates/x/y.rs::func` → `crates/x/y.rs`;
    `docs/x.md §9` → `docs/x.md`; путь без расширения (`crates/ops`, `research/data/`) —
    как есть, за вычетом висящего `:NNN` и пунктуации."""
    m = RFC_PATH_EXT_RE.match(raw)
    if m:
        return m.group(1).rstrip(".,;:)")
    token = RFC_PATH_LINEREF_TAIL_RE.sub("", raw)
    return token.rstrip(".,;:)")


def check7(root):
    rfc_dir = os.path.join(root, "docs", "rfc")
    if not os.path.isdir(rfc_dir):
        info("7-RFC-PATH", "docs/rfc/ отсутствует — проверка неприменима")
        return

    n_refs = n_bad = 0
    for fn in sorted(os.listdir(rfc_dir)):
        if not fn.endswith(".md"):
            continue
        path = os.path.join(rfc_dir, fn)
        relpath = os.path.relpath(path, root)
        try:
            text = read(path)
        except OSError:
            continue
        lines = text.splitlines()
        fence_lines = compute_fence_lines(lines)
        for i, line in enumerate(lines, start=1):
            if i in fence_lines:
                continue
            for m in RFC_PATH_TOKEN_RE.finditer(line):
                raw = m.group(1)
                if any(ch in raw for ch in "*?{}"):
                    continue  # glob/brace-паттерн в прозе (crates/contracts/**, crates/venue-*,
                    # crates/contracts/fixtures/{valid,invalid}) — не литеральный путь
                token = clean_rfc_path_token(raw)
                if not token:
                    continue
                n_refs += 1
                candidate = os.path.join(root, token)
                if not os.path.exists(candidate):
                    n_bad += 1
                    fail(
                        "7-RFC-PATH",
                        f"{relpath}:{i}: путь `{token}` — не существует в дереве репозитория",
                    )

    if n_refs == 0:
        info(
            "7-RFC-PATH",
            "в docs/rfc/**.md путей вида crates/docs/scripts/research/milestones/.claude "
            "не найдено — проверка неприменима",
        )
    elif n_bad == 0:
        pass_(
            "7-RFC-PATH",
            f"все {n_refs} путей, процитированных в docs/rfc/**.md, существуют в дереве репозитория",
        )


def main():
    root = sys.argv[1]
    design_path = os.path.join(root, "docs", "DESIGN.md")

    if not os.path.isfile(design_path):
        fail("SETUP", f"docs/DESIGN.md не найден по пути {design_path}")
        print(f"\nVERDICT: FAIL ({FAILED} нарушений)")
        sys.exit(1)

    design_text = read(design_path)
    design_lines = design_text.splitlines()
    if len(design_lines) < 5:
        fail("SETUP", "docs/DESIGN.md подозрительно короткий (<5 строк) — возможно, обрезан/повреждён")

    check1(root, design_path, design_lines)
    check2(root, design_text)
    check3(root, design_text)
    check4(root)
    check5(root, design_text)
    check6(root)
    check7(root)

    verdict = "PASS" if FAILED == 0 else "FAIL"
    print(f"\nVERDICT: {verdict} ({FAILED} нарушений)")
    sys.exit(0 if FAILED == 0 else 1)


main()
PYEOF
py_rc=$?
exit "${py_rc}"
