#!/usr/bin/env bash
# Наблюдатель за ветками: висяки и дубликаты предмета.
#
# §C.1 плана `docs/plans/plan-branches-and-ci-2026-08-19.md`, утверждённого founder'ом 18.08.
# Спека и пределы — `docs/plans/process-package-2026-08-19.md` §B.
#
# ЗАМЕР, РАДИ КОТОРОГО ОН СУЩЕСТВУЕТ (2026-08-19): 28 невлитых веток; 13 из них ни разу не
# собирались в CI; ЧЕТЫРЕ PR с полностью зелёными чеками стояли невлитыми — работа сделана,
# гейт пройден, приземления нет; кластеры-дубликаты (M-66 — четыре ветки, M-65 — три).
# Величину «сколько веток и в каком они состоянии» до этого дня не видел НИКТО: она получена
# впервые командой, которую никто не запускал.
#
# ЧТО ЭТО НЕ ЕСТЬ. Он НАБЛЮДАЕТ, а не блокирует — уровень 5 таблицы носителей
# (`binding-requires-mechanism.md`), и в агрегат `All checks passed` он НЕ входит. Красный
# наблюдатель не имеет права останавливать чужой merge: он сообщает о состоянии репозитория,
# а не судит предъявленный диф. Предел назван честно: отчёт никого ни к чему не обязывает, и
# `docs/plans/process-layer-audit-2026-08-13.md` §4 меряет эффективность такого канала как
# низкую. Он переводит «ненаблюдаемое» в «видно в логе» — не выше.
#
# ДВА АГРЕГАТА, ради которых он написан:
#   ВИСЯК  — открытый PR, у которого ВСЕ чеки зелёные, а merge'а нет. Это наблюдение ИСХОДА
#            пропуска «дерева решения» (пакет A1): правило нарушено не было, вопрос не задан.
#   ДУБЛЬ  — на один идентификатор предмета (`M-NN`, `TD-NNN`) живых веток больше одной.
#            Класс, давший четыре ветки M-66 и три M-65; у einhard он же дал пять веток на
#            `M-EH-CASCADE-4-PHASE-0-P0` — заимствовать механизм было не у кого.
#
# ПРЕДЕЛ «ВИСЯКА», НАЗВАННЫЙ ПО ПЕРВОМУ ЖЕ ПРОГОНУ. Он видит состояние ЧЕКОВ, а не состояние
# ВЕРДИКТОВ. Первый прогон на живом репозитории пометил висяками `feat/harness-doc-integrity`
# (PR #33) и `fix/resource-oracle-barrier` (PR #28) — обе с зелёными чеками и обе законно
# удерживаемые: первую блокирует `C-101` REJECT, вторую — `R-095` до закрытия H-11. То есть
# `NOTE ВИСЯК` читается как «проверь, почему не влито», а НЕ как «влей». Механизировать
# различие нельзя без чтения вердиктов, а вердикт — суждение; поэтому агрегат и оставлен
# наблюдательным, а не блокирующим (`testing.md`: оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ —
# здесь он обещает «есть готовая на вид ветка», и ровно это меряет).
#
# FAIL-CLOSED НА НЕСОСТОЯВШЕМСЯ SETUP'Е (`testing.md`, целостность гейта, свойство 4):
# «веток не найдено» и «источник данных недоступен» — РАЗНЫЕ состояния, и второе обязано
# краснеть. Наблюдатель, печатающий пустой счастливый список, когда предмет наблюдения
# исчез, — та же слепота, против которой он написан.
#
# ПРОД-ФОРМА — БЕЗ АРГУМЕНТОВ, из корня репозитория. Ручки ниже существуют для ПРОБЫ
# (`scripts/tests/red_branch_health.sh`) и прод-путём не задаются:
#   BRANCH_HEALTH_ROOT   — корень репозитория
#   BRANCH_HEALTH_PRS    — файл с состоянием PR вместо вызова `gh` (герметичность пробы:
#                          проба, ходящая в сеть, мерила бы доступность GitHub, а не свой
#                          инвариант — класс `TD-135`)
#   BRANCH_HEALTH_STALE_DAYS — порог «висяка» в сутках (по умолчанию 1)
#
# Формат BRANCH_HEALTH_PRS — по строке на ветку: `<ветка>\t<номер PR>\t<checks|none>`,
# где `checks` = `green` | `red` | `pending`. Ветка без строки считается «PR нет».
#
# Прогон: bash scripts/check_branch_health.sh

set -uo pipefail

ROOT="${BRANCH_HEALTH_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
STALE_DAYS="${BRANCH_HEALTH_STALE_DAYS:-1}"
cd "${ROOT}" 2>/dev/null || { echo "FAIL  SETUP: корень '${ROOT}' недоступен"; exit 1; }

FAILED=0
NOTES=0
bad()  { FAILED=$((FAILED + 1)); echo "FAIL  $*"; }
note() { NOTES=$((NOTES + 1));   echo "NOTE  $*"; }

# ─── источник веток ─────────────────────────────────────────────────────────────────────
# Именно refs/remotes/origin: локальные ветки — личное дело worktree, предмет наблюдения —
# то, что видят ДРУГИЕ агенты.
mapfile -t BRANCHES < <(
  git for-each-ref --format='%(refname:short)%09%(committerdate:unix)' refs/remotes/origin 2>/dev/null \
    | sed 's#^origin/##' \
    | grep -vE '^(HEAD|main)\b' || true
)

if ! git rev-parse --git-dir >/dev/null 2>&1; then
  bad "SETUP: '${ROOT}' не git-репозиторий — наблюдать нечего, и это НЕ «всё хорошо»"
  echo; echo "VERDICT: FAIL (${FAILED})"; exit 1
fi
if ! git rev-parse -q --verify origin/main >/dev/null 2>&1; then
  bad "SETUP: origin/main не существует — отставание считать не от чего"
  echo; echo "VERDICT: FAIL (${FAILED})"; exit 1
fi

# ─── источник состояния PR ──────────────────────────────────────────────────────────────
# FAIL-CLOSED НА ОТКАЗЕ ИСТОЧНИКА (`C-107` F-106-3 / `C-106`). Первая редакция глушила отказ
# `gh pr list` через `|| true` — тогда «источник недоступен» превращалось в «открытых PR нет»,
# и агрегат ВИСЯК молчал при полном PASS. Отдельно любой ненулевой код `gh pr checks`, кроме
# pending, помечался `red` — то есть «не смогли спросить» было неотличимо от «чеки красные».
# Оба состояния теперь РАЗДЕЛЕНЫ и оба краснеют: неизвестность — не наблюдение.
#
# Различитель `gh pr checks` (коды gh): 0 — все прошли; 8 — есть pending; 1 — есть упавшие,
# НО этот же код приходит и при ошибке транспорта. Поэтому 1 засчитывается как `red` ТОЛЬКО
# если на stdout пришли строки чеков (в них есть таб); иначе — `unknown`.
PRS_FILE=""
SRC_MODE="injected"
if [ -n "${BRANCH_HEALTH_PRS:-}" ]; then
  [ -r "${BRANCH_HEALTH_PRS}" ] || { bad "SETUP: BRANCH_HEALTH_PRS='${BRANCH_HEALTH_PRS}' нечитаем"; echo; echo "VERDICT: FAIL (${FAILED})"; exit 1; }
  PRS_FILE="${BRANCH_HEALTH_PRS}"
else
  SRC_MODE="live"
  PRS_FILE="$(mktemp)"; trap 'rm -f "${PRS_FILE}"' EXIT
  # Страж ОДНОСТРОЧНЫЙ намеренно: многострочный конструкт нельзя нейтрализовать мутантом,
  # не сломав синтаксис, — тогда батарея мерила бы разбор, а не инвариант.
  command -v gh >/dev/null 2>&1 || { bad "SETUP: живой источник PR недоступен — gh не найден; это НЕ «открытых PR нет», агрегат ВИСЯК посчитать нечем"; echo; echo "VERDICT: FAIL (${FAILED}) — источник наблюдения недостоверен"; exit 1; }
  command -v python3 >/dev/null 2>&1 || { bad "SETUP: python3 не найден — разобрать ответ источника нечем"; echo; echo "VERDICT: FAIL (${FAILED}) — источник наблюдения недостоверен"; exit 1; }

  # КОНТРАКТ ЧТЕНИЯ: «валидный ДОКУМЕНТ либо unknown» (`A-011` §3).
  #
  # Три круга гейта спорили об одном классе — «источник ответил не то, что мы думали», — и
  # каждый круг чинил ОЧЕРЕДНОЙ СЦЕНАРИЙ: сперва `|| true` на отказе, потом код возврата 1
  # без строк чеков, потом код 0 с пустым телом. Арбитраж назвал причину: чинилась не ось.
  # Пока состояние выводится из КОДА ВОЗВРАТА и сниффинга текста, форм «успешно, но тело
  # непригодно» бесконечно много.
  #
  # Здесь состояние выводится из СТРУКТУРЫ. Ответ обязан быть валидным JSON ожидаемой формы;
  # всё остальное — `unknown`, и `unknown` роняет прогон. Пустота при этом отличается от
  # поломки САМИМ ИСТОЧНИКОМ, а не догадкой: `gh pr list --json` на нуле PR отдаёт `[]`,
  # `statusCheckRollup` на PR без прогонов отдаёт `[]` (замер: PR #6). Раньше и то и другое
  # было неотличимо от обрыва.
  LIST_JSON="$(gh pr list --state open --json number,headRefName 2>/dev/null)"
  PR_ROWS="$(printf '%s' "${LIST_JSON}" | python3 -c '
import json,sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(3)                      # тело непригодно — НЕ «PR-ов нет»
if not isinstance(d, list):
    sys.exit(3)
for pr in d:
    if not isinstance(pr, dict) or "number" not in pr or "headRefName" not in pr:
        sys.exit(3)
    print("%s\t%s" % (pr["headRefName"], pr["number"]))
' 2>/dev/null)"; LIST_RC=$?
  [ "${LIST_RC}" -eq 0 ] || { bad "SETUP: ответ gh pr list непригоден как документ (не JSON-массив ожидаемой формы) — пустой список от обрыва неотличим, наблюдение НЕ состоялось"; echo; echo "VERDICT: FAIL (${FAILED}) — источник наблюдения недостоверен"; exit 1; }

  while IFS=$'\t' read -r br num; do
    [ -n "${br:-}" ] || continue
    ROLL="$(gh pr view "${num}" --json statusCheckRollup 2>/dev/null)"
    st="$(printf '%s' "${ROLL}" | python3 -c '
import json,sys
try:
    d = json.load(sys.stdin)
except Exception:
    print("unknown"); sys.exit(0)
if not isinstance(d, dict) or "statusCheckRollup" not in d:
    print("unknown"); sys.exit(0)
r = d["statusCheckRollup"]
if r is None or (isinstance(r, list) and len(r) == 0):
    print("nochecks"); sys.exit(0)    # источник САМ говорит «чеков нет», это наблюдение
if not isinstance(r, list):
    print("unknown"); sys.exit(0)
bad_ = {"FAILURE","CANCELLED","TIMED_OUT","ACTION_REQUIRED","ERROR"}
wait = {"PENDING","IN_PROGRESS","QUEUED","WAITING","REQUESTED","EXPECTED"}
saw_bad = saw_wait = False
for c in r:
    if not isinstance(c, dict):
        print("unknown"); sys.exit(0)
    concl = (c.get("conclusion") or "").upper()
    state = (c.get("state") or c.get("status") or "").upper()
    if concl in bad_ or state in bad_: saw_bad = True
    elif concl == "" and state in wait: saw_wait = True
    elif state in wait and concl == "": saw_wait = True
print("red" if saw_bad else ("pending" if saw_wait else "green"))
' 2>/dev/null)"
    [ -n "${st}" ] || st=unknown
    printf '%s\t%s\t%s\n' "${br}" "${num}" "${st}" >> "${PRS_FILE}"
  done <<< "${PR_ROWS}"
fi

pr_of() { awk -F'\t' -v b="$1" '$1==b {print $2"\t"$3; exit}' "${PRS_FILE}"; }

# ─── таблица ────────────────────────────────────────────────────────────────────────────
NOW=$(git log -1 --format='%ct' origin/main 2>/dev/null || echo 0)
DAY=86400
printf '%-40s %6s %6s %6s %-10s %s\n' ВЕТКА ОТСТ СВОИХ НОВЫХ PR ВОЗРАСТ
printf '%s\n' '────────────────────────────────────────────────────────────────────────────────────'

declare -A ID_SEEN=()
STALE_GREEN=()
GREEN_YOUNG=()
UNKNOWN_PRS=()
NOCHECK_PRS=()
TOTAL=0

for row in "${BRANCHES[@]}"; do
  br="${row%%$'\t'*}"; ts="${row##*$'\t'}"
  [ -n "${br}" ] || continue
  TOTAL=$((TOTAL + 1))
  read -r behind own < <(git rev-list --left-right --count "origin/main...origin/${br}" 2>/dev/null || echo "? ?")
  newf=$(git diff --name-only --diff-filter=A "origin/main...origin/${br}" 2>/dev/null | wc -l)
  age=$(( (NOW - ts) / DAY ))
  prinfo="$(pr_of "${br}")"
  if [ -n "${prinfo}" ]; then
    num="${prinfo%%$'\t'*}"; st="${prinfo##*$'\t'}"
    prcol="#${num}/${st}"
    if [ "${st}" = "green" ]; then
      if [ "${age}" -ge "${STALE_DAYS}" ]; then
        STALE_GREEN+=("${br} (PR #${num}, ${age} сут)")
      else
        # Зелёный и невлитый, но МОЛОЖЕ порога. Висяком не объявляется — свежий PR ждёт
        # ревью законно, — но и умалчиваться не имеет права: именно умолчание делало
        # строку «не найдено» ложной при собственной колонке `#N/green` (см. агрегат ниже).
        GREEN_YOUNG+=("${br} (PR #${num}, ${age} сут)")
      fi
    fi
    # `unknown` — состояние «спросить не удалось». Оно НЕ зелёное, НЕ красное и НЕ «PR нет»:
    # про эту ветку наблюдение не состоялось, и прогон обязан быть красным (`C-107` F-106-3).
    # Известные результаты соседних PR при этом СОХРАНЯЮТСЯ — частичный отказ не обнуляет
    # то, что удалось узнать.
    if [ "${st}" = "unknown" ]; then
      UNKNOWN_PRS+=("${br} (PR #${num})")
    fi
    if [ "${st}" = "nochecks" ]; then
      NOCHECK_PRS+=("${br} (PR #${num}, ${age} сут)")
    fi
  else
    prcol="—"
  fi
  printf '%-40s %6s %6s %6s %-10s %s сут\n' "${br}" "${behind}" "${own}" "${newf}" "${prcol}" "${age}"

  # идентификатор предмета из имени ветки
  id="$(grep -oE '\b(M|TD|A|C|R)-[0-9]+[a-z]?\b' <<<"${br}" | head -1 || true)"
  [ -n "${id}" ] && ID_SEEN["${id}"]="${ID_SEEN[${id}]:-}${br} "
done

echo

# ─── агрегат 1: ВИСЯК ───────────────────────────────────────────────────────────────────
# АГРЕГАТ ОБЯЗАН УТВЕРЖДАТЬ РОВНО ТО, ЧТО ЗАМЕРИЛ (`testing.md` §«Оракул обязан мерить ТО,
# ЧТО ОБЕЩАЕТ»). Прежняя редакция печатала «веток с зелёным PR и без merge'а не найдено»,
# тогда как условие выше отбирает только зелёные СТАРШЕ порога. Замер 2026-08-24: в одном
# прогоне колонка показала `#89/green`, а агрегат — «не найдено»; наблюдатель противоречил
# сам себе на одних и тех же данных. Порог сохранён (свежий PR — не висяк), но фраза
# приведена к нему, а зелёные моложе порога названы ЧИСЛОМ: молчание о них и было дефектом.
if [ ${#STALE_GREEN[@]} -gt 0 ]; then
  for s in "${STALE_GREEN[@]}"; do
    note "ВИСЯК: ${s} — все чеки зелёные, merge'а нет. Работа готова, приземления не случилось"
  done
else
  echo "ok    ВИСЯК: зелёных PR без merge'а старше ${STALE_DAYS} сут не найдено"
fi
if [ ${#GREEN_YOUNG[@]} -gt 0 ]; then
  for g in "${GREEN_YOUNG[@]}"; do
    echo "ok    ЗЕЛЁНЫЙ-СВЕЖИЙ: ${g} — зелен и не влит, но моложе порога ${STALE_DAYS} сут; висяком не считается"
  done
fi

# ─── агрегат 1bis: НЕИЗВЕСТНО (fail-closed, `C-107` F-106-3) ───────────────────────────
if [ ${#UNKNOWN_PRS[@]} -gt 0 ]; then
  for u in "${UNKNOWN_PRS[@]}"; do
    bad "НЕИЗВЕСТНО: ${u} — состояние чеков получить не удалось. Это не «зелено» и не «красно»: \
наблюдение по этой ветке НЕ состоялось"
  done
else
  echo "ok    НЕИЗВЕСТНО: состояние чеков получено по всем PR, о которых спрашивали"
fi

# ─── агрегат 1ter: PR БЕЗ ЕДИНОГО ЧЕКА ─────────────────────────────────────────────────
# Это НЕ ошибка источника: ответ получен и он достоверен. Наблюдение полезное — открытый PR,
# по которому не прогонялось ничего, ровно та слепота, из-за которой M-69 прошёл два круга
# критика, ни разу не собравшись.
if [ ${#NOCHECK_PRS[@]} -gt 0 ]; then
  for n in "${NOCHECK_PRS[@]}"; do
    note "БЕЗ ЧЕКОВ: ${n} — PR открыт, но не прогонялось НИ ОДНОГО чека"
  done
fi

# ─── агрегат 2: ДУБЛЬ ───────────────────────────────────────────────────────────────────
dups=0
for id in "${!ID_SEEN[@]}"; do
  # shellcheck disable=SC2206
  arr=(${ID_SEEN[$id]})
  if [ ${#arr[@]} -gt 1 ]; then
    dups=$((dups + 1))
    note "ДУБЛЬ: предмет ${id} живёт на ${#arr[@]} ветках — ${arr[*]}"
  fi
done
[ "${dups}" -eq 0 ] && echo "ok    ДУБЛЬ: ни один предмет не живёт больше чем на одной ветке"

echo
# Наблюдение ОТСУТСТВИЯ: ноль веток при существующем origin/main — законное состояние
# (всё влито), и оно ПЕЧАТАЕТСЯ явно, чтобы «пусто» не читалось как «не запускалось».
echo "веток кроме main: ${TOTAL}; замечаний: ${NOTES}"
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED}) — источник наблюдения недостоверен"
  exit 1
fi
echo "VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)"
exit 0
