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
#               ни другое.
# Ошибки разнонаправленные (завышение/занижение/ложное существование/ложное «пройдено») —
# значит дело не в невнимательности к конкретной строке, а в ОТСУТСТВИИ проверки как класса.
# Правило «сверять замером, а не переносить из прежних текстов» (testing.md) само нарушалось
# четыре раза подряд автором правила — правило, держащееся на добросовестности, не работает.
#
# ПЯТЬ проверок (каждая — PASS/FAIL/INFO с причиной; INFO = не применимо/не проверяется
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
# Никакого `cmd && echo PASS || echo FAIL` — вся агрегация (FAIL-счётчик, VERDICT,
# exit-код) сделана явно внутри движка (Python — надёжнее POSIX-awk на markdown-таблицах
# и Юникод-регулярках; сам движок ниже — единственное тело проверки, без промежуточных
# файлов вне этого скрипта per scope: Пишешь ТОЛЬКО verify_design_claims.sh +
# scripts/tests/red_verify_design_claims.sh).

set -uo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_ROOT="${1:-${SCRIPT_ROOT}}"

if ! command -v python3 >/dev/null 2>&1; then
  echo "FAIL  [SETUP] python3 не найден в PATH — гейт не может выполниться"
  echo
  echo "VERDICT: FAIL (1 нарушений)"
  exit 1
fi

exec python3 - "${TARGET_ROOT}" <<'PYEOF'
#!/usr/bin/env python3
"""Движок verify_design_claims.sh. Читает docs/DESIGN.md под ROOT (argv[1]) и репозиторий
вокруг него, печатает PASS/FAIL/INFO построчно + финальный VERDICT, выходит с 0 (PASS)
или 1 (FAIL). Ничего не пишет, только читает."""
import os
import re
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

    verdict = "PASS" if FAILED == 0 else "FAIL"
    print(f"\nVERDICT: {verdict} ({FAILED} нарушений)")
    sys.exit(0 if FAILED == 0 else 1)


main()
PYEOF
