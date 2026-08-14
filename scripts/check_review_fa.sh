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

# ─── ШАГ 1: SKIP, если crates/** не тронут (per-range семантика) ──────────────────
crates=$(git diff --name-only "${BASE}" HEAD 2>/dev/null \
  | awk -F/ '$1=="crates" && $2!="" {print $2}' | sort -u)
[ -z "${crates}" ] && { echo "SKIP (диапазон не трогает crates/**)"; exit 0; }

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
# U = ∅ (тронуты ТОЛЬКО NO-FA крейты): B вакуумен — предикат считается выполненным
# УЖЕ на уровне waiver'а (печать WAIVED ниже).
ECHO_OK=1
if [ "${#U[@]}" -eq 0 ]; then
  ECHO_OK=0
else
  for id in "${U[@]}"; do
    for f in "${S[@]}"; do
      if [ -f "${f}" ] && grep -qF "${id}" "${f}" 2>/dev/null; then
        echo "${f}: ${id}"
        ECHO_OK=0
      fi
    done
  done
fi

# ─── ВЕРДИКТ ──────────────────────────────────────────────────────────────────────
if [ "${WAIVER_OK}" -ne 1 ] || [ "${ECHO_OK}" -ne 0 ]; then
  exit 1
fi

for c in "${NOFA_CRA[@]}"; do
  echo "WAIVED: crates/${c} (пробел покрытия открыт — §7.7 follow-up)"
done
exit 0