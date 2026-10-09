#!/usr/bin/env bash
# scripts/check_review_fa.sh — барьер M-66 protocol-attestation.
#
# Инвариант (милестоун §2, формулировка от результата):
#   ни один проверяемый диапазон (push в main / PR в main), чей суммарный diff
#   трогает crates/**, не проходит без файла research/reviews/R-*.md — введённого этим
#   диапазоном или названного полным путём в %B любого коммита диапазона И существующего
#   на HEAD, — несущего хотя бы один живой инвариант-ID из U (объединения ЖИВЫХ множеств
#   тронутых FA-крейтов), и КАЖДЫЙ тронутый NO-FA крейт предъявлен строкой FA-WAIVER в
#   одном из этих review-файлов. Пробел покрытия NO-FA показывается, не извиняется.
#
# СЕМАНТИКА per-range (спека §3.1): диф берётся суммарно BASE..HEAD, реверт-пара даёт
# чистый net-diff и молчание. Это правильно по предмету: норма — «в main не входит код
# без вердикта», а не «ни один коммит диапазона не прошёл без вердикта».
#
# БАЗА СРАВНЕНИЯ — из СОБЫТИЯ (fail-closed, как docs-freeze / protected-artifacts):
#   push         → PUSH_BEFORE
#   pull_request → PR_BASE_SHA
#   иначе        → FAIL
# Любая недостоверная форма (пусто, zero-SHA, отсутствует в истории, не предок HEAD) →
# FAIL, а не пропуск: «базы нет» не значит «проверять нечего».
#
# Этот скрипт зовётся CI ровно так: EVENT_NAME/PUSH_BEFORE/PR_BASE_SHA из события
# `.github/workflows/ci.yml` (job `review-fa`); проба `scripts/tests/red_review_fa.sh`
# зовёт его ТОЙ ЖЕ проводкой (`testing.md` §«Целостность гейта»).

set -uo pipefail

ZERO=0000000000000000000000000000000000000000

# ─── БАЗА СРАВНЕНИЯ — из СОБЫТИЯ, fail-closed при ЛЮБОЙ ошибке установки ─────────
raw="${1:-}"
if [ -z "${raw}" ]; then
  case "${EVENT_NAME:-}" in
    push)         raw="${PUSH_BEFORE:-}" ;;
    pull_request) raw="${PR_BASE_SHA:-}" ;;
    "")           echo "FAIL  событие не задано (EVENT_NAME пуст) — барьер зовут не так, как его зовёт CI" >&2; exit 1 ;;
    *)            echo "FAIL  неизвестное событие '${EVENT_NAME}' — база сравнения не определена" >&2; exit 1 ;;
  esac
fi

[ -n "${raw}" ] || { echo "FAIL  база события пуста (EVENT_NAME=${EVENT_NAME:-?}) — fail-closed" >&2; exit 1; }
case "${raw}" in
  *[!0]*) : ;;  # есть хоть один ненулевой символ — не zero-SHA
  *)      echo "FAIL  база = zero-SHA (создание ветки или force-push) — целостность FA-предъявления не доказуема" >&2; exit 1 ;;
esac
git rev-parse -q --verify "${raw}^{commit}" >/dev/null 2>&1 \
  || { echo "FAIL  база '${raw}' отсутствует в истории (переписана force-push'ем / поверхностный клон)" >&2; exit 1; }
git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null \
  || { echo "FAIL  база '${raw}' НЕ предок HEAD — история переписана (force-push); что в FA менялось, недоказуемо" >&2; exit 1; }

BASE=$(git rev-parse "${raw}^{commit}")

# ─── ШАГ 1: SKIP, если ПРОД-КОД крейтов не тронут (per-range семантика) ───────────
# `C-115` B-1: правило «любой путь под crates/» давало ЛОЖНОЕ КРАСНОЕ на реальном
# прод-диапазоне. Замер: merge PR #15 в `main` (`2c56a34..de40f48`, форма `push`,
# `PUSH_BEFORE=2c56a34`) → `FAIL … S=∅`, exit=1. Диапазон трогал РОВНО ОДИН файл —
# `crates/gateway/tests/red_snapshot_noclone.rs`, sacred RED-спеку architect'а: фикс оракула
# по вердикту гейта, идущий харнесс-треком, где вердикт reviewer'а по маршруту не требуется
# (`docs/workflow/harness-track.md` §3). Барьер требовал его — и краснел на законной истории.
#
# Граница НЕ изобретается здесь: она уже проведена в проде и обоснована замером.
# `.github/workflows/deploy.yml:34-38` (TD-086) исключает `crates/*/tests/**` из триггера
# деплоя — «тесты в рантайм-бинарь не входят». Проверено исполнением: `Dockerfile:18`
# собирает `--bin recorder|journal-retention|gateway-serve|gateway-checkpoint|wsprobe`
# явным списком, тестовые цели не участвуют. Признак `harness-track.md` §4 — «если код
# запускается на VPS или его результат попадает в журнал» — для тестов ложен.
#
# Защита НЕ ослаблена: диапазон, тронувший `crates/*/src/**`, судится как прежде (проверено
# анти-плацебо на `61f452e` — exit=1). Диапазон, тронувший ТОЛЬКО не-прод пути, объявляется
# неприменимым ЯВНО, с перечислением файлов — «наблюдение отсутствия» (`testing.md`,
# целостность гейта, свойство 4), а не молчание.
#
# КЛАССИФИКАЦИЯ ПУТЕЙ (`C-118` C-1, `A-012` §1-Д п.5). Критерий один и он проверяем:
# ВХОДИТ ЛИ ПУТЬ В ПРОД-ОБРАЗ. Замер 2026-08-21 на этом дереве:
#   `Dockerfile:18` — `cargo build --release --bin recorder --bin journal-retention
#   --bin gateway-serve --bin gateway-checkpoint --bin wsprobe` — сборка идёт ЯВНЫМ
#   списком бинарей; ни тестовые, ни example-, ни bench-цели в него не входят.
#   `find crates -maxdepth 2 -name examples -type d` → 5; `-name benches` → 0; `build.rs` → 0.
#
#   судится            | `crates/*/src/**`      — исполняется прод-процессом
#   судится            | `crates/*/Cargo.toml`  — определяет, ЧТО и КАК собрано в бинарь
#   судится            | `crates/*/build.rs`    — исполняется при сборке бинаря (сейчас 0 шт.,
#                      |                          правило заведено до появления первого)
#   SKIP               | `crates/*/tests/**`    — sacred RED-спеки, в образ не входят
#   SKIP               | `crates/*/examples/**` — в образ не входят; их поломку ловит
#                      |                          `clippy --all-targets` (`gates.md` §3)
#   SKIP               | `crates/*/benches/**`  — то же основание (сейчас 0 шт.)
#
# Каждая строка этой таблицы пиннится сценарием пробы — иначе классификация есть намерение,
# а не механизм.
crates_all=$(git diff --name-only "${BASE}" HEAD 2>/dev/null \
  | awk -F/ '$1=="crates" && $2!="" {print $2}' | sort -u)
crates=$(git diff --name-only "${BASE}" HEAD 2>/dev/null \
  | grep -vE '^crates/[^/]+/(tests|examples|benches)/' \
  | awk -F/ '$1=="crates" && $2!="" {print $2}' | sort -u)
if [ -z "${crates}" ]; then
  if [ -n "${crates_all}" ]; then
    echo "SKIP (диапазон трогает ТОЛЬКО не-прод пути крейтов — tests/examples/benches, в прод-образ не входят; C-115 B-1, классификация A-012 §1-Д п.5)"
    git diff --name-only "${BASE}" HEAD 2>/dev/null | grep -E '^crates/[^/]+/(tests|examples|benches)/' | sed 's/^/      ↳ /'
  else
    echo "SKIP (диапазон не трогает crates/**)"
  fi
  exit 0
fi

# ─── ШАГ 2: ТАБЛИЦА МАППИНГА (спека §3.2, нормативная, живёт в скрипте) ───────────
# Каждый крейт имеет FA-файл и собственный префикс; NO-FA крейты (`recorder`, `derive`)
# — видимый пробел покрытия, требуют FA-WAIVER. Незнакомое имя → FAIL (M2UNKNOWN):
# rename/новый крейт ломает барьер до явной правки таблицы через критика milestone'а
# (тихое устаревание маппинга конвертировано в видимое красное).
declare -A FA_OF PFX_OF
LIVE_CRA=()
NOFA_CRA=()
for c in ${crates}; do
  case "${c}" in
    recorder|derive)
      NOFA_CRA+=("${c}")
      ;;
    venue-*)
      FA_OF["${c}"]="docs/fa/venues.md"
      PFX_OF["${c}"]="VN"
      LIVE_CRA+=("${c}")
      ;;
    journal)
      FA_OF["${c}"]="docs/fa/journal.md"
      PFX_OF["${c}"]="JR"
      LIVE_CRA+=("${c}")
      ;;
    book)
      FA_OF["${c}"]="docs/fa/book.md"
      PFX_OF["${c}"]="BK"
      LIVE_CRA+=("${c}")
      ;;
    contracts)
      FA_OF["${c}"]="docs/fa/contracts.md"
      PFX_OF["${c}"]="CT"
      LIVE_CRA+=("${c}")
      ;;
    oms)
      FA_OF["${c}"]="docs/fa/oms.md"
      PFX_OF["${c}"]="OM"
      LIVE_CRA+=("${c}")
      ;;
    risk)
      FA_OF["${c}"]="docs/fa/risk.md"
      PFX_OF["${c}"]="RK"
      LIVE_CRA+=("${c}")
      ;;
    killswitch)
      FA_OF["${c}"]="docs/fa/killswitch.md"
      PFX_OF["${c}"]="KS"
      LIVE_CRA+=("${c}")
      ;;
    sim)
      FA_OF["${c}"]="docs/fa/sim.md"
      PFX_OF["${c}"]="SM"
      LIVE_CRA+=("${c}")
      ;;
    runner)
      FA_OF["${c}"]="docs/fa/runner.md"
      PFX_OF["${c}"]="RN"
      LIVE_CRA+=("${c}")
      ;;
    alpha)
      FA_OF["${c}"]="docs/fa/alpha.md"
      PFX_OF["${c}"]="AL"
      LIVE_CRA+=("${c}")
      ;;
    portfolio)
      FA_OF["${c}"]="docs/fa/portfolio.md"
      PFX_OF["${c}"]="PF"
      LIVE_CRA+=("${c}")
      ;;
    strategy)
      FA_OF["${c}"]="docs/fa/strategy.md"
      PFX_OF["${c}"]="ST"
      LIVE_CRA+=("${c}")
      ;;
    signals)
      FA_OF["${c}"]="docs/fa/signals.md"
      PFX_OF["${c}"]="SG"
      LIVE_CRA+=("${c}")
      ;;
    research-cli)
      FA_OF["${c}"]="docs/fa/research-cli.md"
      PFX_OF["${c}"]="RC"
      LIVE_CRA+=("${c}")
      ;;
    gateway)
      FA_OF["${c}"]="docs/fa/viz-backend.md"
      PFX_OF["${c}"]="VB"
      LIVE_CRA+=("${c}")
      ;;
    gateway-serve)
      FA_OF["${c}"]="docs/fa/viz-backend.md"
      PFX_OF["${c}"]="GS"
      LIVE_CRA+=("${c}")
      ;;
    ops)
      FA_OF["${c}"]="docs/fa/ops.md"
      PFX_OF["${c}"]="OPS"
      LIVE_CRA+=("${c}")
      ;;
    *)
      echo "FAIL  незнакомый крейт crates/${c} — добавь строку §3.2 спеки M-66 или FA" >&2
      exit 1
      ;;
  esac
done

# ─── ШАГ 3: U — объединение ЖИВЫХ ID каждого FA-крейта по СВОЕМУ префиксу ─────────
# Сквозные ID (`DET-I-1`, `CT-I-*` цитируемые в `docs/fa/journal.md`) НЕ входят в U:
# они цитируются, но не доказывают открытия FA тронутого модуля (запиннено R-053,
# §0 стр. 4/13). FA-файл из таблицы, отсутствующий на HEAD → FAIL (не «пустое
# множество»; F6NOFAFILE/F6NODIR).
U=()
declare -A SEEN_U
for c in "${LIVE_CRA[@]}"; do
  fa="${FA_OF[${c}]}"
  pfx="${PFX_OF[${c}]}"
  if [ ! -f "${fa}" ]; then
    echo "FAIL  FA-файл отсутствует на HEAD: ${fa} (тронут crates/${c}) — fail-closed (§3.1 шаг 3)" >&2
    exit 1
  fi
  while IFS= read -r id; do
    [ -n "${id}" ] || continue
    if [ -z "${SEEN_U[${id}]:-}" ]; then
      U+=("${id}")
      SEEN_U["${id}"]=1
    fi
  done < <(grep -hoE "\b${pfx}-I-[0-9]+\b" "${fa}" 2>/dev/null || true)
done

# ─── ШАГ 4: S — review-файлы (механизм D) ─────────────────────────────────────────
# F6NOREV: каталог research/reviews/ отсутствует → S пуст → через механизм D → FAIL.
# Названный полным путём в %B, но НЕ существующий на HEAD — НЕ входит в S (ghost;
# спекa §3.1 шаг 4: «существующего на HEAD»). Это та же fail-closed логика, что у
# D-механизма docs-freeze: указатель, не токен.
REV_DIR="research/reviews"
[ -d "${REV_DIR}" ] || { echo "FAIL  каталог ${REV_DIR}/ отсутствует — S=∅, механизм D" >&2; exit 1; }

S=()
declare -A SEEN_S
# (a) review-файлы, добавленные диапазоном
while IFS= read -r f; do
  [ -n "${f}" ] || continue
  if [ -z "${SEEN_S[${f}]:-}" ]; then
    S+=("${f}")
    SEEN_S["${f}"]=1
  fi
done < <(git diff --name-status --diff-filter=A "${BASE}" HEAD -- "${REV_DIR}/*.md" 2>/dev/null \
  | awk '$1=="A"{print $2}')
# (b) review-файлы, названные полным путём в %B любого коммита диапазона И существующие на HEAD
while IFS= read -r f; do
  [ -n "${f}" ] || continue
  [ -f "${f}" ] || continue
  if [ -z "${SEEN_S[${f}]:-}" ]; then
    S+=("${f}")
    SEEN_S["${f}"]=1
  fi
done < <(git log --format=%B "${BASE}..HEAD" 2>/dev/null \
  | grep -oE 'research/reviews/R-[^[:space:])]+\.md' | sort -u)

# D: S = ∅ → FAIL
if [ "${#S[@]}" -eq 0 ]; then
  echo "FAIL  ни один review-файл не введён диапазоном и не назван полным путём в %B (S=∅) — механизм D" >&2
  exit 1
fi

# ─── ШАГ 5: W (waiver) — per-NO-FA-crate, НЕ per-range (спека §2 W, §3.1 шаг 5) ────
# Живое эхо FA-крейта пробел NO-FA НЕ гасит (предикат per-crate); waiver должен
# называть ИМЕННО тронутый крейт (W7WRONG) с причиной ≥12 символов после `— ` (W7EMPTY,
# W7SHORT — нижняя сторона границы, W7EXACT — верхняя; `C-085` B-1).
WAIVER_OK=1
for c in "${NOFA_CRA[@]}"; do
  hit=0
  for f in "${S[@]}"; do
    if [ -f "${f}" ] && grep -qE "^FA-WAIVER: crates/${c} — .{12,}\$" "${f}" 2>/dev/null; then
      hit=1
      break
    fi
  done
  if [ "${hit}" -ne 1 ]; then
    WAIVER_OK=0
    echo "FAIL  тронут NO-FA crates/${c} — нет waiver в S; заведи docs/fa/${c}.md (задача §7.7) или предъяви 'FA-WAIVER: crates/${c} — <причина ≥12 символов>' в одном из review-файлов" >&2
  fi
done

# ─── ШАГ 6: B (union-B echo) — хотя бы один файл из S несёт живой ID из U ─────────
# Эхо требует СЛОВОЦЕЛОГО совпадения ID: `grep -E "\b${id}\b"`. Фиксированная строка
# (`grep -F`) МАТЧИТ ПОДСТРОКУ без границы слова — живой `JR-I-1` подстрока мёртвых
# `JR-I-14`/`JR-I-999`/..., предикат засчитывает эхо, которого в вердикте нет (R-079 Б-1).
# Синтетическая FA пробы (`red_review_fa.sh:148`) несёт ровно один живой ID
# (`JR-I-1`), суффикс-свободный относительно `JR-I-999` — проба проходила случайностью
# состава фикстуры. Прод-форма (`docs/fa/journal.md` с `JR-I-1..13`) — суффикс-непустая,
# без `\b` барьер зелен на мёртвом ID.
#
# U = ∅: предикат вакуумен ТОЛЬКО для NO-FA-только дифов (спека §3.1 шаг 6). Если при
# этом `LIVE_CRA` НЕ пуст (FA-файл есть, но его собственный префикс не дал НИ ОДНОГО
# живого ID — файл пуст или содержит чужие префиксы), это ТИХИЙ ОТКАЗ гейта, а не
# вакуумный PASS (R-079 Б-2). FAIL с диагностикой: что открывать, чтобы починить.
ECHO_OK=1
if [ "${#U[@]}" -eq 0 ]; then
  if [ "${#LIVE_CRA[@]}" -gt 0 ]; then
    echo "FAIL  U = ∅ при непустом LIVE_CRA — FA-файлы тронутых крейтов существуют, но не несут ни одного ID своего префикса:" >&2
    for c in "${LIVE_CRA[@]}"; do
      fa="${FA_OF[${c}]}"
      pfx="${PFX_OF[${c}]}"
      echo "      • crates/${c} → ${fa} (ожидались \b${pfx}-I-[0-9]+, найдено: 0)" >&2
    done
    echo "      проверь, что FA-док перечисляет живой инвариант-ID; иначе гейт молча гасит весь путь крейта" >&2
    exit 1
  fi
  # U = ∅ И LIVE_CRA пуст → тронуты ТОЛЬКО NO-FA крейты: B вакуумен, PASS по waiver'у.
  ECHO_OK=0
else
  for id in "${U[@]}"; do
    for f in "${S[@]}"; do
      if [ -f "${f}" ] && grep -qE "\\b${id}\\b" "${f}" 2>/dev/null; then
        echo "${f}: ${id}"
        ECHO_OK=0
      fi
    done
  done
fi

# ─── ВЕРДИКТ ──────────────────────────────────────────────────────────────────────
# Отказ ОБЯЗАН назвать причину. Первая редакция выходила `exit 1` молча: агент видел в CI
# красное с пустым выводом и не мог понять, чего от него хотят — а барьер существует именно
# для того, чтобы НЕЧТЕНИЕ FA стало наблюдаемым. Молчащий барьер эту задачу не решает: он
# лишь блокирует, не обучая. Диагностика печатает, ЧТО искали и ГДЕ, и называет обе
# законные развязки (эхо или waiver). Логика вердикта НЕ изменена — только вывод.
if [ "${WAIVER_OK}" -ne 1 ] || [ "${ECHO_OK}" -ne 0 ]; then
  if [ "${ECHO_OK}" -ne 0 ]; then
    echo "FAIL  ни один вердикт диапазона не назвал ЖИВОЙ инвариант FA тронутых крейтов" >&2
    for c in "${LIVE_CRA[@]}"; do
      echo "      • crates/${c} → ${FA_OF[${c}]} (ожидался любой из живых \b${PFX_OF[${c}]}-I-[0-9]+)" >&2
    done
    if [ "${#U[@]}" -gt 0 ]; then
      echo "      искали ID: ${U[*]}" >&2
    fi
    if [ "${#S[@]}" -gt 0 ]; then
      echo "      в вердиктах: ${S[*]}" >&2
    else
      echo "      вердиктов в диапазоне НЕТ — merge без вердикта запрещён (gates.md §4)" >&2
    fi
    echo "      развязка: назвать живой ID в вердикте ЛИБО предъявить пробел явно —" >&2
    echo "                FA-WAIVER: crates/<name> — <причина ≥12 символов> в теле коммита" >&2
  fi
  if [ "${WAIVER_OK}" -ne 1 ]; then
    echo "FAIL  waiver назван, но не покрывает тронутые крейты (waiver — не токен на предъявителя)" >&2
  fi
  exit 1
fi

for c in "${NOFA_CRA[@]}"; do
  echo "WAIVED: crates/${c} (пробел покрытия открыт — §7.7 follow-up)"
done
exit 0