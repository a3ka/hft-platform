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
#      несуществующих в main SHA как «подтверждено коммитами») — машинная проверка: КАЖДЫЙ
#      hex-токен SHA-формы в backtick'ах (7-64 символа) в docs/DESIGN.md И docs/rfc/**.md
#      (рекурсивно) вне ``` ```-фенсов обязан (а) существовать как git-объект
#      (`git cat-file -e <sha>^{commit}`) И (б) входить в историю HEAD (или MERGE_HEAD внутри
#      --merge-preview) — `git merge-base --is-ancestor`. Анти-плацебо (C-044 F1, реальный
#      случай): (а) без (б) — плацебо, орфан-коммит с заброшенной/несмёрженной ветки
#      (`ffedc10`/`6a2c331`/`67b6159` реально существовали как git-объекты на ветке
#      `engine/M-35-arms`, `git cat-file -e` проходил, но они не входили в ancestry
#      `origin/main` — только `--is-ancestor` это ловит).
#      КОНТЕКСТ НЕ ФИЛЬТРУЕТ (исправлено по R-020 B-1). Прежняя редакция смотрела токен
#      только если в том же параграфе стояло слово из рукописного списка синонимов
#      («коммит»/«merge»/«мёрж»/«sha»), а остальные НЕ проверяла И НЕ СЧИТАЛА. Замер на
#      merge-цели: 20 токенов, проверялось 17, молча пропускалось 3, из них два —
#      нормативные утверждения о коммитах («подтверждено отдельным ИСПРАВЛЕНИЕМ `b3a5a95`»,
#      «reviewer close-out (`41d3526`)»); на документе из одних выдуманных SHA гейт печатал
#      «проверка неприменима». Это fail-open внутри fail-closed гейта, и обходился он
#      обычным русским синонимом. Теперь: кандидат — КАЖДЫЙ токен; список причин пропуска
#      ЗАКРЫТ (SKIP-LEN64 — 64 символа, sha256-дайджест; SKIP-DECLARED — явный маркер, см.
#      ниже); неизвестная форма → ПРОВЕРЯЕТСЯ. Цифровые токены проверяются наравне с
#      остальными: правило «цифровое — не SHA» вывело бы из-под гейта `0000000`/`1111111`.
#      Каждый непроверенный токен ПЕЧАТАЕТСЯ строкой `SKIP-<ПРИЧИНА> <файл>:<строка>`,
#      и итог всегда несёт баланс `всего=N проверено=K пропущено=M` (K+M==N).
#      «Проверка неприменима» допустима ТОЛЬКО при всего=0.
#
#      МАРКЕР not-a-commit (единственный способ объявить токен не-коммитом):
#          <!-- not-a-commit: <token> -->
#      HTML-комментарий в ТОМ ЖЕ .md-файле; действует на все вхождения этого токена в этом
#      файле; регистр не важен. Применение — hex-подобные идентификаторы, которые коммитами
#      не являются (партии данных, fixed-point константы вида `100000000`). Смысл формы:
#      исключение живёт В ДОКУМЕНТЕ и видно в дифе, а не в молчаливом правиле скрипта.
#   7. Пути, процитированные в docs/rfc/**.md (рекурсивно), существуют в дереве репозитория.
#      Кандидат — КАЖДЫЙ backtick-токен, содержащий `/`, вне ``` ```-фенсов (исправлено по
#      R-020 N-1: прежний whitelist префиксов crates|docs|scripts|research|milestones|.claude
#      РЕШАЛ, смотреть ли токен, и молча пропускал крейт-относительные формы, которыми RFC
#      пользуются свободно — `contracts/src/lib.rs:46`, `recorder/src/main.rs:58`,
#      `journal/src/segments.rs`, `tests/red_schema.rs`; замер: проверялось 67, молча
#      пропускалось 49). Резолв — три равноправные попытки: прямо от корня; крейт-
#      относительно (`journal/src/x.rs` → `crates/journal/src/x.rs`); как суффикс
#      существующего пути дерева (`tests/red_schema.rs`, база названа в прозе рядом).
#      Не резолвится и ЯКОРИТСЯ в дерево (первый сегмент — существующая запись верхнего
#      уровня ИЛИ имя крейта, либо токен называет файл распознанным расширением) → FAIL.
#      Не резолвится и НЕ якорится → SKIP-NOTREPO (имя ветки `feat/M-08`, перечисление
#      `Ord/Risk/Ctl`). Прочие причины пропуска: SKIP-GLOB (`*`/`?`/`{`/`}`), SKIP-URL,
#      SKIP-ABS (эндпоинт вида `/sapi/v1/...`), SKIP-PROSE (фрагмент текста между соседними
#      inline-code вставками — артефакт разметки, не путь). КАЖДЫЙ пропуск печатается
#      строкой с файлом/строкой/токеном/причиной; итог несёт тот же баланс
#      `всего=N проверено=K пропущено=M`.
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


def note(check, msg):
    print(f"NOTE  [{check}] {msg}")


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

# ---------------------------------------------------------------------------
# МАРКЕР `FACTS:` — исполнение решения арбитра `A-010` §H (задачи H-1…H-5).
#
# Решение существовало с 18.08, маркер уже проставлялся ВРУЧНУЮ в живых документах — и не
# читался НИЧЕМ (`grep -rln FACTS scripts/ .github/` → пусто). Класс «решено-не-построено»,
# зеркальный к `TD-155` («построено-не-проведено»).
#
# ПРАВИЛО (`A-010` §H): `docs/plans/**` остаётся исключённым из check3/check4, КРОМЕ файлов,
# несущих маркер. Документ, объявивший себя фактурой, отвечает за свои ссылки; документ без
# маркера — рабочая записка, с неё спроса нет.
#
# МАРКЕР ЗАСЧИТЫВАЕТСЯ ТОЛЬКО В ГОЛОВЕ ФАЙЛА — оборона от класса «документ О маркере судится
# как документ С маркером»; тот же класс, что шапка `GATE-META`, процитированная примером
# внутри код-фенса.
# ЧЕСТНОЕ УТОЧНЕНИЕ (`C-116` F-2): прежняя редакция этого комментария ссылалась на
# `fable-review-2026-08-18-open-questions.md:265` как на живой случай — замер её не
# подтверждает: строка несёт ПЛЕЙСХОЛДЕР (`audited_head=<полный SHA>`), который регэксп не
# матчит ни при каком лимите головы, к тому же внутри код-фенса. Оборона верна, но
# обосновывающий её пример был неверен; сам лимит запиннен сценариями H-FACTS-3 (верх) и
# H-FACTS-9 (низ), а не этим комментарием.
#
# ПРЕДЕЛ, НАЗВАННЫЙ ЧЕСТНО (`A-010` H-5): `файл:строка` НЕ проверяется и проверяться не будет.
# Маркер конвертирует «молча ложно» в «явно датировано» — не более. Дрейф содержимого он
# ОБЪЯВЛЯЕТ, а не лечит.
FACTS_MARKER_RE = re.compile(r"<!--\s*FACTS:.*?audited_head=([0-9a-fA-F]{7,40}).*?-->")
# СЕМАНТИКА `audited_head` (закреплена по `C-116` F-9): это ревизия ДЕРЕВА, НА КОТОРОМ
# СНЯТЫ ФАКТЫ, а не коммит, которым документ создан. На первых же двух посевах она разошлась:
# `gateway-ws-contract.md` нёс ревизию сбора, `journal-sharding-facts.md` — коммит создания
# документа (родитель дерева сбора). Разница в один коммит вреда не дала, но поле, чья
# семантика не закреплена, разъезжается тем быстрее, чем больше носителей.
FACTS_HEAD_LINES = 5          # сколько первых строк считаются «головой файла»
FACTS_NOTE_THRESHOLD = 20     # порог утверждений `путь:строка`, при котором молчание заметно


# Объявление себя фактурой и ВАЛИДНЫЙ маркер — разные события, и их различение есть
# половина находки `C-116` F-1 («молчаливый даунгрейд»): строка `FACTS:` с опечаткой или
# коротким SHA не давала НИ FAIL, НИ NOTE — автор считал документ под гейтом, гейт молчал.
# Это «наблюдение отсутствия» (`testing.md`, целостность гейта, свойство 4).
FACTS_DECL_RE = re.compile(r"<!--\s*FACTS:")


# `C-117` F-18: `_facts_head_scan` глотал `UnicodeDecodeError` и возвращал (False, None) —
# документ с ВАЛИДНЫМ маркером и одним байтом cp1251 во второй строке молча выпадал из ВСЕХ
# проверок: ни опт-ина, ни FAIL «НЕ распарсен», ни NOTE. Python декодирует чтение чанком,
# поэтому битый байт строки 2 валит итерацию ДО отдачи строки 1. Это тот самый «молчаливый
# даунгрейд», устранение которого объявлено достижением этой ветки, — воспроизводимый одним
# символом из чужого буфера. Различаем: нечитаемая голова — отдельный наблюдаемый исход.
def _head_is_utf8(fpath):
    try:
        with open(fpath, encoding="utf-8") as f:
            for i, _line in enumerate(f):
                if i >= FACTS_HEAD_LINES:
                    break
        return True
    except UnicodeDecodeError:
        return False
    except OSError:
        # `C-117` F-24: прежний комментарий утверждал «её ловят соседние проверки» — замер
        # это опроверг: chmod 000 не ловит НИКТО (head-scan → (False, None) ⇒ исключён из
        # ссылок; NOTE-проверка глотает тот же OSError). Принятый предел, а не покрытие:
        # git режим 000 не хранит (`ls-files -s` → 100644), свежий чекаут кейс не
        # воспроизводит — радиус ограничен локальными деревьями.
        return True


def _facts_head_scan(fpath):
    """(declared, sha) по ГОЛОВЕ файла.
    declared — в голове есть строка, объявляющая документ фактурой;
    sha       — распарсенная ревизия сбора, либо None (объявление есть, форма негодна)."""
    declared = False
    try:
        with open(fpath, encoding="utf-8") as f:
            for i, line in enumerate(f):
                if i >= FACTS_HEAD_LINES:
                    break
                m = FACTS_MARKER_RE.search(line)
                if m:
                    return True, m.group(1)
                if FACTS_DECL_RE.search(line):
                    declared = True
    except (UnicodeDecodeError, OSError):
        return False, None
    return declared, None


def _has_facts_marker(fpath):
    _declared, sha = _facts_head_scan(fpath)
    return sha is not None


# АРХИВНЫЙ КЛАСС ВЕРДИКТОВ (решение founder'а 2026-08-22; исполнение `TD-064`).
# Вердикт гейта — ДАТИРОВАННЫЙ СНИМОК: он отвечает за ревизию, названную в его шапке
# `GATE-META: audited_head`, а не за то, как выглядит дерево сегодня. Требовать от него
# живых ссылок вечно — то же, что требовать от протокола собрания, чтобы упомянутые в нём
# документы не исчезали. `TD-064` (заведено reviewer'ом 2026-08-01, `R-016` N-5) называет
# и корень: гейт не отличает УТВЕРЖДЕНИЕ от ЦИТАТЫ утверждения, а вердикт цитирует чужой
# вывод как улику — черновик `R-016`, приведший вывод гейта дословно, сделал гейт красным
# на самом себе, и обходили это искажением улики в аудит-трейле.
# Альтернатива (выровнять охват) отвергнута founder'ом: она требует править ссылки в 28
# вердиктах и рецидивирует при каждом следующем удалённом файле.
VERDICT_CLASS_DIRS = ("research/critiques/", "research/reviews/", "research/arbitration/")


def is_excluded(relpath, fpath):
    """Единое правило исключения для check3/check4 — один разбор на двоих (`A-010` §G)."""
    if relpath.startswith("docs/archive/"):
        return True
    if relpath.startswith(VERDICT_CLASS_DIRS):
        return True
    if relpath.startswith("docs/plans/"):
        return not _has_facts_marker(fpath)
    return False


SECTION_HEADING_RE = re.compile(r"^#{2,3}\s+§?(\d+(?:\.\d+)?)\.?\s+\S")
DESIGN_REF_RE = re.compile(r"DESIGN\.md\s*§(\d+(?:\.\d+)?)")


def check3(root, design_text):
    valid_sections = {m.group(1) for line in design_text.splitlines() if (m := SECTION_HEADING_RE.match(line))}

    if not valid_sections:
        fail("3-ССЫЛКИ", "не удалось извлечь ни одного раздела (§N) из оглавления docs/DESIGN.md — setup-guard")
        return

    exts = (".md", ".rs", ".sh")
    # исключение — общее правило: `docs/plans/**` входит в проверку, если несёт `FACTS:`
    n_refs = n_bad = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in (".git", "target", "node_modules")]
        for fn in filenames:
            if not fn.endswith(exts):
                continue
            fpath = os.path.join(dirpath, fn)
            relpath = os.path.relpath(fpath, root)
            if is_excluded(relpath, fpath):
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


# ---------------------------------------------------------------------------
# NOTE-часть решения `A-010` §H: документ в `docs/plans/**`, несущий ≥20 утверждений вида
# `путь:строка` и НЕ несущий маркера, объявляется молчащим о своей ревизии. Это NOTE, а не
# FAIL: такой документ по-прежнему рабочая записка, и требовать от неё гарантий нельзя —
# но её молчание перестаёт быть невидимым.
# Разбор `путь:строка` ПЕРЕИСПОЛЬЗОВАН (`RFC_PATH_TOKEN_RE` + `RFC_PATH_LINEREF_TAIL_RE`),
# третий парсер не заводится (`A-010` §G).
# ---------------------------------------------------------------------------
# ---------------------------------------------------------------------------
# `A-010` §H, пункт решения, НЕ исполненный в первой редакции (`C-116` F-1, ранг REJECT):
# «плюс проверка SHA той же механикой, что check6 (существует, предок HEAD)». Захват
# `audited_head=(…)` в регэкспе был МЁРТВЫМ кодом: группа не потреблялась нигде, документ
# с выдуманной ревизией опт-инился и не судился ничем. Весь смысл маркера по `A-010` §H —
# ИМЕНОВАННАЯ ревизия сбора; ревизия, которой нет, не именует ничего.
# Механика переиспользована, не продублирована (`A-010` §G): те же `git_commit_exists` и
# `git_commit_is_ancestor_of_any`, что у check6, включая MERGE_HEAD внутри --merge-preview.
# ---------------------------------------------------------------------------
# `C-117` F-17: после F-11 в plans-зоне жили ТРИ обхода с ТРЕМЯ охватами — `is_excluded`
# рекурсивен (startswith), NOTE-проверка рекурсивна (os.walk), SHA-проверка шла os.listdir
# верхним уровнем. Дыра F-1 воспроизводилась одним уровнем глубже: в подкаталоге фиктивная
# ревизия опт-инилась и не судилась, а малформ снова молчал. Обход теперь ОДИН на всех.
def plans_md_files(root):
    plans = os.path.join(root, "docs", "plans")
    if not os.path.isdir(plans):
        return []
    out = []
    for dirpath, dirnames, filenames in os.walk(plans):
        dirnames[:] = [d for d in dirnames if d not in (".git",)]
        for fn in filenames:
            if fn.endswith(".md"):
                out.append(os.path.join(dirpath, fn))
    return sorted(out)


def check_facts_sha(root):
    plan_files = plans_md_files(root)
    if not plan_files:
        return

    declared_bad = []   # объявил себя фактурой, маркер не распарсен
    unreadable = []     # голова не читается как UTF-8 — проверка невозможна
    marked = []         # (relpath, sha)
    for fpath in plan_files:
        declared, sha = _facts_head_scan(fpath)
        relpath = os.path.relpath(fpath, root)
        if sha is not None:
            marked.append((relpath, sha))
        elif declared:
            declared_bad.append(relpath)
        elif not _head_is_utf8(fpath):
            unreadable.append(relpath)

    # `C-117` F-18: нечитаемая голова — наблюдаемый исход, а не тишина.
    for relpath in unreadable:
        fail(
            "H-FACTS-SHA",
            f"{relpath}: голова файла не читается как UTF-8 — наличие маркера `FACTS:` "
            f"проверить невозможно; документ молча выпал бы из проверки ссылок",
        )

    # Молчаливый даунгрейд перестаёт быть молчаливым (C-116 F-1, вторая половина).
    for relpath in declared_bad:
        fail(
            "H-FACTS-SHA",
            f"{relpath}: в голове файла есть строка `FACTS:`, но маркер НЕ распарсен "
            f"(нужен `audited_head=<7..40 hex>`) — документ объявил себя фактурой и молча "
            f"выпал из проверки ссылок; исправьте маркер или уберите объявление",
        )

    if not marked:
        if not declared_bad and not unreadable:
            info("H-FACTS-SHA", "документов с маркером `FACTS:` в docs/plans/**: 0 — проверять нечего")
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

    # Fail-closed, как у check6: нечем проверить — это НАРУШЕНИЕ, а не молчаливый пропуск.
    if not git_ok:
        fail(
            "H-FACTS-SHA",
            f"маркеров `FACTS:` найдено {len(marked)}, но '{root}' не git-репозиторий "
            f"(или git недоступен) — существование ревизии сбора проверить нельзя (setup-guard)",
        )
        return

    refs = canonical_refs(root)
    n_bad = 0
    for relpath, sha in marked:
        if not git_commit_exists(root, sha):
            n_bad += 1
            fail(
                "H-FACTS-SHA",
                f"{relpath}: маркер называет ревизию сбора `{sha}` — такого коммита в "
                f"репозитории НЕТ вовсе; документ объявлен фактурой на несуществующем дереве",
            )
            continue
        if not git_commit_is_ancestor_of_any(root, sha, refs):
            n_bad += 1
            fail(
                "H-FACTS-SHA",
                f"{relpath}: маркер называет ревизию сбора `{sha}` — коммит существует, но "
                f"НЕ входит в историю {'/'.join(refs)} (орфан/несмёрженная ветка): факты "
                f"собраны на дереве, которого в этой истории нет",
            )

    if n_bad == 0:
        pass_(
            "H-FACTS-SHA",
            f"маркеров `FACTS:` проверено {len(marked)} — все ревизии сбора существуют и "
            f"входят в историю {'/'.join(refs)}",
        )


def check_facts_note(root):
    plans = os.path.join(root, "docs", "plans")
    if not os.path.isdir(plans):
        return
    # `C-116` F-11: `is_excluded` покрывает `docs/plans/**` рекурсивно (startswith), а эта
    # проверка шла `os.listdir` — верхним уровнем. Подкаталогов сегодня ноль, расхождение было
    # латентным: первый же подкаталог получил бы проверку ссылок, но не наблюдение молчания.
    silent = 0
    for fpath in plans_md_files(root):
        fn = os.path.relpath(fpath, plans)
        if _has_facts_marker(fpath):
            continue
        n = 0
        try:
            with open(fpath, encoding="utf-8") as f:
                for line in f:
                    for m in RFC_PATH_TOKEN_RE.finditer(line):
                        if RFC_PATH_LINEREF_TAIL_RE.search(m.group(1)):
                            n += 1
        except (UnicodeDecodeError, OSError):
            continue
        if n >= FACTS_NOTE_THRESHOLD:
            silent += 1
            note("H-FACTS", f"docs/plans/{fn}: {n} утверждений `путь:строка` без маркера "
                           f"`FACTS:` — документ не называет ревизию сбора, основанием для "
                           f"спеки или оракула не является")
    if silent == 0:
        info("H-FACTS", "фактур без маркера с порогом утверждений не найдено")



def check_verdict_class(root):
    """Архивный класс ОБЪЯВЛЯЕТСЯ — списком каталогов, а НЕ счётчиком.

    Первая редакция печатала числа: сколько вердиктов исключено и сколько мёртвых ссылок
    внутри. Числа сняты после ДВУХ адверсарных кругов (`R-100` F-1, `R-101` F-1/F-2/F-3),
    каждый из которых нашёл новый обманный стаб — и КАЖДЫЙ раз стаб был про счётчик, ни
    разу про само исключение. Причина структурная, а не «недоработали пробу»: счётчик
    фальсифицируем по трём независимым осям (величина, каталог, множество файлов), и каждая
    ось требует своей пары фикстур с РАЗНЫМИ числами. Матрица растёт быстрее, чем пиннится.

    Решающее — числа никому не нужны. Решение founder'а 22.08 гласит: вердикт стареет
    ЗАКОННО. Значит «84 мёртвые ссылки» не действие, а декорация: по ней никто ничего не
    делает, а фальшивое значение создаёт ложную уверенность. `TD-064` описывает инцидент
    «гейт краснеет на ЦИТАТУ битой ссылки» — его закрывает ИСКЛЮЧЕНИЕ, не счёт.

    Что осталось: строка называет, ЧТО именно выведено из-под проверок, и берёт список из
    того же `VERDICT_CLASS_DIRS`, которым исключение и делается. Один носитель — расходиться
    нечему; фальсифицировать нечего, кроме самого списка, а он запиннен V-5/V-6 и V-2.

    Названный предел: сколько мёртвых ссылок накопилось в вердиктах, гейт больше не
    сообщает. Это осознанный размен — наблюдение, которое нельзя защитить от подделки
    дешевле двух кругов, хуже честного молчания о величине при явном объявлении класса.
    """
    present = [rel for rel in VERDICT_CLASS_DIRS if os.path.isdir(os.path.join(root, rel))]
    if not present:
        info("V-АРХИВ", "каталогов вердиктов нет — класс неприменим")
        return
    info(
        "V-АРХИВ",
        "архивный класс выведен из проверок ссылок как датированные снимки "
        f"(решение 2026-08-22, `TD-064`): {' '.join(present)} — ссылки внутри них не судятся, "
        "вердикт отвечает за ревизию из своей шапки `GATE-META`, а не за сегодняшнее дерево",
    )


def check4(root):
    docs_dir = os.path.join(root, "docs")
    if not os.path.isdir(docs_dir):
        fail("4-МЁРТВЫЕ-ФАЙЛЫ", "каталог docs/ отсутствует — setup-guard")
        return
    # исключение — общее правило: `docs/plans/**` входит в проверку, если несёт `FACTS:`
    n_refs = n_bad = 0
    for dirpath, dirnames, filenames in os.walk(docs_dir):
        dirnames[:] = [d for d in dirnames if d not in (".git",)]
        for fn in filenames:
            if not fn.endswith(".md"):
                continue
            fpath = os.path.join(dirpath, fn)
            relpath = os.path.relpath(fpath, root)
            if is_excluded(relpath, fpath):
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

# R-020 B-1: НЕ контекстные слова решают, проверять ли токен. Кандидат — КАЖДЫЙ hex-токен
# SHA-формы в backtick'ах вне фенсов. Прежний `SHA_CONTEXT_RE` (рукописный список русских
# синонимов) удалён как ФИЛЬТР: он давал fail-open — «подтверждено отдельным ИСПРАВЛЕНИЕМ
# `b3a5a95`» проходило мимо гейта молча, потому что автор выбрал синоним. Замер на реальном
# корпусе (merge-цель origin/main): 20 токенов, проверялось 17, молча пропускалось 3, из них
# два — нормативные утверждения о коммитах.
SHA_TOKEN_RE = re.compile(r"`([0-9a-f]{7,64})`")

# TD-074 (закрыто 2026-08-03): SHA вне backtick'ов раньше был гейту НЕВИДИМ — «подтверждено
# коммитом b3a5a95» без кавычек не попадало даже в баланс `всего=N`. Тот же класс, что B-1:
# решала форма разметки, а не смысл.
#
# Почему не расширили кандидата «в лоб» на любой голый hex: замер reviewer'а (R-023 §8) —
# 5 таких токенов в docs/rfc/, ВСЕ являются числовыми литералами fixed-point/timestamp
# (`6500050000000`, `1752000000123`, …). Fail-closed на них дал бы 5 ложных FAIL, и гейт
# начали бы глушить маркерами — то есть лечение хуже болезни.
#
# Правило: голый токен становится кандидатом ТОЛЬКО в контексте цитирования коммита —
# рядом (в той же строке) стоит слово commit/коммит/SHA/merge/мерж/HEAD. Числовой литерал
# такого соседства не имеет, а ложь вида «подтверждено коммитом X» — имеет по определению:
# без этого слова утверждение перестаёт быть утверждением о коммите.
BARE_SHA_RE = re.compile(r"(?<![`0-9a-zA-Z])([0-9a-f]{7,40})(?![`0-9a-zA-Z])")
COMMIT_CONTEXT_RE = re.compile(
    r"(?i)(commit|коммит|мерж|merge\b|HEAD\b|SHA\b|ревизи)"
)

# Единственный способ вывести токен из-под проверки ЯВНО: машинный маркер в том же файле.
# Форма (документирована здесь и только здесь):  <!-- not-a-commit: <token> -->
# Ставится в том же .md-файле, где стоит токен; действует на ВСЕ вхождения этого токена в
# ЭТОМ файле. Всё, что не объявлено маркером и не попало в закрытый список причин ниже,
# ПРОВЕРЯЕТСЯ (fail-closed): неизвестная форма → проверка, а не пропуск.
NOT_A_COMMIT_DECL_RE = re.compile(r"<!--\s*not-a-commit:\s*([0-9a-fA-F]{7,64})\s*-->")


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


def rfc_md_files(root):
    """docs/rfc/**.md — РЕКУРСИВНО (R-020 N-2: шапка обещает `**`, прежний код читал
    только верхний уровень через os.listdir; подкаталогов сегодня нет, но расхождение
    «документ говорит не то, что делает код» — ровно тот класс, ради которого гейт написан)."""
    rfc_dir = os.path.join(root, "docs", "rfc")
    out = []
    if not os.path.isdir(rfc_dir):
        return out
    for dirpath, dirnames, filenames in os.walk(rfc_dir):
        dirnames[:] = sorted(d for d in dirnames if d != ".git")
        for fn in sorted(filenames):
            if fn.endswith(".md"):
                out.append(os.path.join(dirpath, fn))
    return out


# Закрытый список причин, по которым SHA-подобный токен допустимо НЕ проверять.
# Всё, что сюда не попало, — проверяется (fail-closed). Каждый пропуск ПЕЧАТАЕТСЯ.
SHA_SKIP_REASONS = {
    "SKIP-LEN64": "длина 64 — sha256-дайджест содержимого; репозиторий на sha1, коммитом "
                  "такой токен быть не может",
    "SKIP-DECLARED": "документ ЯВНО объявил токен не-коммитом маркером "
                     "<!-- not-a-commit: <token> --> в том же файле",
}
# ПРО ЦИФРОВЫЕ ТОКЕНЫ (R-020 N-3). Соблазнительное правило «чисто цифровой токен —
# десятичный литерал, не проверяем» ОТВЕРГНУТО: оно выводит из-под гейта `0000000` и
# `1111111` — канонические выдуманные SHA (ими пользуются и self-тест, и репро R-020), то
# есть заново открывает дыру B-1. Замер на merge-цели origin/main: чисто цифровой токен в
# docs/DESIGN.md+docs/rfc/ РОВНО ОДИН (`0999929`) и это НАСТОЯЩИЙ коммит; ни одной
# fixed-point константы в backtick'ах нет. Ambiguity «литерал или SHA» разрешается В ПОЛЬЗУ
# ПРОВЕРКИ; ложный FAIL на будущую константу снимается тем же явным маркером
# <!-- not-a-commit: 100000000 -->, что и любой другой не-коммит — то есть ФАЙЛОМ, который
# видно в дифе, а не молчаливым правилом.


def classify_sha_token(tok, declared, root=None):
    """Причина пропуска (ключ SHA_SKIP_REASONS) либо None → токен ПРОВЕРЯЕТСЯ.
    Список ЗАКРЫТ: неизвестная форма → проверка, а не пропуск (fail-closed).

    TD-073 (закрыто 2026-08-03): маркер `<!-- not-a-commit: X -->` БОЛЬШЕ НЕ
    самообслуживаемый. Объявление проверяется машиной: если X на самом деле ЯВЛЯЕТСЯ
    коммитом репозитория, то маркер лжёт — и это FAIL ("LIAR-DECL"), а не пропуск.
    Раньше автор мог заглушить провал на выдуманном SHA одной строкой вместо того,
    чтобы исправить пруф; барьер стоял на внимательности reviewer'а, а не на машине.
    Логика: объявить не-коммитом можно ТОЛЬКО то, что не-коммит."""
    low = tok.lower()
    if low in declared:
        if root is not None and git_commit_exists(root, low):
            return "LIAR-DECL"
        return "SKIP-DECLARED"
    if len(tok) == 64:
        return "SKIP-LEN64"
    return None


def gather_sha_tokens(root, path):
    """[(relpath, lineno, sha, skip_reason|None), ...] — КАЖДЫЙ hex-токен SHA-формы в
    backtick'ах вне ``` ```-фенсов. Контекстные слова больше не фильтруют (B-1)."""
    try:
        text = read(path)
    except OSError:
        return []
    relpath = os.path.relpath(path, root)
    lines = text.splitlines()
    fence_lines = compute_fence_lines(lines)
    declared = {m.group(1).lower() for m in NOT_A_COMMIT_DECL_RE.finditer(text)}

    out = []
    for i, line in enumerate(lines, start=1):
        if i in fence_lines:
            continue
        # G2 (M-60 §0): токен ВНУТРИ маркера <!-- not-a-commit: X --> — метаданные ОБ
        # утверждении, а не утверждение о коммите; вхождением НЕ считается, иначе баланс
        # `всего=N` раздувается и перестаёт отвечать на вопрос «сколько утверждений о
        # коммитах делает документ». Без этого исключения строка маркера сама проходит
        # COMMIT_CONTEXT_RE (слово `commit` внутри `not-a-commit`) и BARE_SHA_RE считал
        # токен объявления лишним вхождением (замер: RFC-SHA-balance давал всего=4 при 3).
        # Токен, стоящий на той же строке ВНЕ маркера, по-прежнему считается.
        decl_spans = [m.span() for m in NOT_A_COMMIT_DECL_RE.finditer(line)]
        seen_spans = []
        for m in SHA_TOKEN_RE.finditer(line):
            tok = m.group(1)
            seen_spans.append(m.span(1))
            out.append((relpath, i, tok, classify_sha_token(tok, declared, root)))
        # TD-074: голый (без backtick'ов) токен — кандидат ТОЛЬКО в контексте цитаты коммита.
        if COMMIT_CONTEXT_RE.search(line):
            for m in BARE_SHA_RE.finditer(line):
                if any(a <= m.start(1) < b for a, b in seen_spans):
                    continue  # уже учтён как backtick-токен
                if any(a <= m.start(1) < b for a, b in decl_spans):
                    continue  # стоит внутри маркера объявления — не вхождение (G2)
                tok = m.group(1)
                out.append((relpath, i, tok, classify_sha_token(tok, declared, root)))
    return out


def check6(root):
    targets = []
    design_path = os.path.join(root, "docs", "DESIGN.md")
    if os.path.isfile(design_path):
        targets.append(design_path)
    targets.extend(rfc_md_files(root))

    all_tokens = []
    for path in targets:
        all_tokens.extend(gather_sha_tokens(root, path))

    total = len(all_tokens)
    # TD-073: ЛЖИВОЕ объявление — не пропуск, а нарушение. Обрабатывается ДО остальных,
    # иначе оно попало бы в `skipped` и выглядело бы как легальное исключение.
    liars = [t for t in all_tokens if t[3] == "LIAR-DECL"]
    for relpath, lineno, tok, _ in liars:
        fail(
            "6-RFC-SHA",
            f"{relpath}:{lineno} `{tok}` объявлен маркером <!-- not-a-commit: {tok} -->, "
            f"но ЯВЛЯЕТСЯ коммитом репозитория — маркер лжёт (TD-073). Объявить не-коммитом "
            f"можно только то, что действительно не коммит; уберите маркер или исправьте токен",
        )
    skipped = [t for t in all_tokens if t[3] is not None and t[3] != "LIAR-DECL"]
    all_refs = [t for t in all_tokens if t[3] is None]

    # Остаток печатается ПЕРВЫМ и построчно: файл, строка, токен, причина. Проверка обязана
    # ЗНАТЬ и СООБЩАТЬ, чего она не проверила (тот же принцип setup-guard, урок M-40).
    for relpath, lineno, tok, reason in skipped:
        info("6-RFC-SHA", f"{reason} {relpath}:{lineno} `{tok}` — {SHA_SKIP_REASONS[reason]}")

    if total == 0:
        # Единственный случай, когда «неприменима» допустимо: SHA-подобных токенов НОЛЬ.
        info(
            "6-RFC-SHA",
            "SHA-подобных токенов в docs/DESIGN.md и docs/rfc/**.md: всего=0 проверено=0 "
            "пропущено=0 — проверять нечего, проверка неприменима",
        )
        return

    balance = (
        f"SHA-подобных токенов (docs/DESIGN.md + docs/rfc/**.md): всего={total} "
        f"проверено={len(all_refs)} пропущено={len(skipped)}"
    )

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
            f"{balance}; но '{root}' не git-репозиторий (или git недоступен) — "
            f"существование SHA проверить нельзя (setup-guard)",
        )
        return

    refs = canonical_refs(root)
    n_bad = 0
    for relpath, lineno, sha, _reason in all_refs:
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

    # Баланс печатается ВСЕГДА — и на PASS, и на FAIL: «все N проверены» без знаменателя
    # цитировалось бы в close-out'ах как «все SHA документа проверены» (R-020 B-1, п.3).
    if n_bad == 0:
        pass_(
            "6-RFC-SHA",
            f"{balance} — все {len(all_refs)} проверенных существуют И входят в историю "
            f"{'/'.join(refs)}",
        )
    else:
        info("6-RFC-SHA", f"{balance} — из них {n_bad} нарушений (перечислены выше)")


# ---------------------------------------------------------------------------
# CHECK 7 — пути, процитированные в docs/rfc/**.md, существуют в дереве репозитория
# (C-044 F2: список мест правки занижен, но опечатка/несуществующий путь — тот же класс лжи)
# ---------------------------------------------------------------------------

# R-020 N-1: тот же принцип, что и в check6 — whitelist префиксов
# (crates|docs|scripts|research|milestones|.claude) РЕШАЛ, смотреть ли токен, и потому молча
# пропускал крейт-относительные формы, которыми реальные RFC пользуются свободно
# (`contracts/src/lib.rs:46`, `recorder/src/main.rs:58`, `journal/src/segments.rs`,
# `tests/red_schema.rs`). Замер reviewer'а: проверялось 67 путей, молча пропускалось 49.
# Теперь КАНДИДАТ — каждый backtick-токен, содержащий `/`, вне фенсов; непроверенный обязан
# быть перечислен с ПОИМЕНОВАННОЙ причиной из закрытого списка ниже.
RFC_PATH_TOKEN_RE = re.compile(r"`([^`\n]*/[^`\n]*)`")

PATH_SKIP_REASONS = {
    "SKIP-GLOB": "glob/brace-паттерн (`*`/`?`/`{`/`}`) — не литеральный путь",
    "SKIP-URL": "URL — ресурс вне дерева репозитория",
    "SKIP-ABS": "абсолютный путь/эндпоинт API (начинается с `/`) — не путь в дереве репозитория",
    "SKIP-PROSE": "фрагмент прозы между соседними inline-code вставками (пробелы/скобки/"
                  "кириллица вокруг слэша), а не путь",
    "SKIP-NOTREPO": "не резолвится в дереве и ничем не якорится в него (первый сегмент — не "
                    "каталог репозитория и не имя крейта, расширения файла нет): имя ветки, "
                    "перечисление типов через `/` и т.п.",
}
PATH_PROSE_CHARS = set(" \t()[]<>«»→…;,")
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


def build_path_suffix_index(root):
    """Множество ВСЕХ хвостовых суффиксов путей дерева (файлов и каталогов), нормализованных
    через `/`. Нужно, чтобы честно резолвить формы, база которых названа в прозе рядом, а не
    внутри backtick'ов: `tests/red_schema.rs` → crates/contracts/tests/red_schema.rs,
    `fixtures/invalid/x.json` → crates/contracts/fixtures/invalid/x.json. Это РАСШИРЯЕТ
    множество резолвимого (fail-closed сохраняется: то, что не резолвится нигде и якорится
    в дерево, — FAIL)."""
    index = set()
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in (".git", "target", "node_modules")]
        rel_dir = os.path.relpath(dirpath, root)
        base = "" if rel_dir == "." else rel_dir.replace(os.sep, "/")
        entries = list(dirnames) + list(filenames)
        for name in entries:
            rel = f"{base}/{name}" if base else name
            parts = rel.split("/")
            for i in range(len(parts)):
                index.add("/".join(parts[i:]))
    return index


def resolve_rfc_path(root, token, suffix_index):
    """(resolved: bool, как_именно: str). Три равноправные попытки — прямая, крейт-
    относительная (`journal/src/x.rs` → `crates/journal/src/x.rs`) и суффиксная."""
    if os.path.exists(os.path.join(root, token)):
        return True, "прямо от корня репозитория"
    if os.path.exists(os.path.join(root, "crates", token)):
        return True, "крейт-относительно (crates/<name>/...)"
    if token in suffix_index:
        return True, "как суффикс существующего пути дерева"
    return False, ""


def path_token_is_anchored(root, token):
    """Токен ЗАЯВЛЯЕТ путь в дереве репозитория (и потому его отсутствие — ложь документа),
    если хотя бы одно: первый сегмент — существующая запись верхнего уровня; первый сегмент —
    имя существующего крейта; токен называет файл распознанным расширением."""
    first = token.split("/")[0]
    if first and os.path.exists(os.path.join(root, first)):
        return True
    if first and os.path.isdir(os.path.join(root, "crates", first)):
        return True
    return bool(re.search(r"\.(?:rs|md|sh|toml|json|yml|yaml)$", token))


def classify_rfc_path_token(root, raw, suffix_index):
    """(skip_reason|None, token, how). skip_reason=None → токен ПРОВЕРЕН (резолвится) либо
    ЯКОРЕН и обязан дать FAIL (how == "")."""
    token = clean_rfc_path_token(raw).rstrip("/")
    if not token or "/" not in raw:
        return "SKIP-PROSE", raw, ""
    low = token.lower()
    if low.startswith("http://") or low.startswith("https://"):
        return "SKIP-URL", token, ""
    if token.startswith("/"):
        return "SKIP-ABS", token, ""
    if any(ch in token for ch in "*?{}"):
        return "SKIP-GLOB", token, ""
    if any(ch in PATH_PROSE_CHARS for ch in token) or any(ord(ch) > 127 for ch in token):
        return "SKIP-PROSE", token, ""
    resolved, how = resolve_rfc_path(root, token, suffix_index)
    if resolved:
        return None, token, how
    if path_token_is_anchored(root, token):
        return None, token, ""   # якорен и не резолвится → нарушение (FAIL у вызывающего)
    return "SKIP-NOTREPO", token, ""


def check7(root):
    rfc_files = rfc_md_files(root)
    if not os.path.isdir(os.path.join(root, "docs", "rfc")):
        info("7-RFC-PATH", "docs/rfc/ отсутствует — проверка неприменима")
        return

    suffix_index = build_path_suffix_index(root)
    total = n_checked = n_skipped = n_bad = 0
    skips = []
    bads = []
    for path in rfc_files:
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
                total += 1
                reason, token, how = classify_rfc_path_token(root, raw, suffix_index)
                if reason is not None:
                    n_skipped += 1
                    skips.append((reason, relpath, i, token))
                    continue
                n_checked += 1
                if not how:
                    n_bad += 1
                    bads.append((relpath, i, token))

    # Остаток — построчно, с ПОИМЕНОВАННОЙ причиной; молчаливых пропусков не бывает.
    for reason, relpath, i, token in skips:
        info("7-RFC-PATH", f"{reason} {relpath}:{i} `{token}` — {PATH_SKIP_REASONS[reason]}")
    for relpath, i, token in bads:
        fail(
            "7-RFC-PATH",
            f"{relpath}:{i}: путь `{token}` — не существует в дереве репозитория "
            f"(не найден ни прямо, ни как crates/{token}, ни суффиксом существующего пути)",
        )

    if total == 0:
        info(
            "7-RFC-PATH",
            "путей-кандидатов (токены со слэшем в backtick'ах) в docs/rfc/**.md: всего=0 "
            "проверено=0 пропущено=0 — проверять нечего, проверка неприменима",
        )
        return

    balance = (
        f"путей-кандидатов (токены со слэшем в backtick'ах, docs/rfc/**.md): всего={total} "
        f"проверено={n_checked} пропущено={n_skipped}"
    )
    if n_bad == 0:
        pass_("7-RFC-PATH", f"{balance} — все {n_checked} проверенных существуют в дереве репозитория")
    else:
        info("7-RFC-PATH", f"{balance} — из них {n_bad} нарушений (перечислены выше)")


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
    check_verdict_class(root)
    check_facts_sha(root)
    check_facts_note(root)
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
