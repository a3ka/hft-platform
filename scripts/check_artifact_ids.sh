#!/usr/bin/env bash
# check_artifact_ids.sh — механический барьер M-61 (TD-111).
#
# ИНВАРИАНТ ОТ РЕЗУЛЬТАТА И ОТ ДИАПАЗОНА (спека §4.1):
#   ни один коммит проверяемого диапазона не вводит номер, обозначающий ВТОРОЙ ПРЕДМЕТ —
#   ни созданием артефакта, ни переименованием, ни записью в `TECH-DEBT.md`; «второй»
#   считается относительно ОБЪЕДИНЕНИЯ `refs/remotes/origin/*` ∪ `refs/heads/*`.
#
# Почему не «никакой номер не обозначает двух предметов» (спека §4.1, `C-069` F-1):
#   пять коллизий уже существуют и переименованию не подлежат (§5). Абсолютная формулировка
#   дала бы барьер, красный навсегда, либо grandfather-список. Диапазон снимает обе беды:
#   предсуществующая коллизия в диапазон не попадает и легитимна; третий файл под занятый
#   номер — попадает и краснеет.
#
# Проводка повторяет check_protected_artifacts.sh (та же дисциплина fail-closed по базе
# события, блокер B1): пустое событие / zero-SHA / база отсутствует / база не предок HEAD —
# отказ, а не тихий пропуск.
#
# Пять правил subject_id (спека §3.1) — первое сработавшее побеждает:
#   1. носитель предмета в шапке `**Предмет:**` ИЛИ `**Контекст**`/`**Контекст.**`
#      (блок до первой пустой строки, путь на строке-продолжении засчитывается);
#   2. для TD — слаг из `- **TD-NNN** \`слаг\`` в TECH-DEBT.md;
#   3. milestone с буквенным суффиксом — семья без буквы, + требуется совпавший предмет
#      (одна буква сама по себе не доказательство, `C-070` F-2);
#   4. слаг имени: часть после `<CLS>-<NNN>-` до `.md`, без суффиксов `-rev<N>` / `-addendum*`;
#   5. слага нет — sentinel `<без-слага>`, УЧАСТВУЕТ в сравнении (а не пропускается).

set -uo pipefail

ZERO=0000000000000000000000000000000000000000
CLASS_RE='^(milestones/M|research/reviews/R|research/critiques/C|research/arbitration/A)-[0-9]+'
die() { echo "FAIL  $*" >&2; echo "      ↳ Барьер fail-closed: база не установлена достоверно." >&2; exit 1; }

# ── база из СОБЫТИЯ (та же проводка, что check_protected_artifacts.sh) ─────────────
raw="${1:-}"
if [ -z "${raw}" ]; then
  case "${EVENT_NAME:-}" in
    push)         raw="${PUSH_BEFORE:-}" ;;
    pull_request) raw="${PR_BASE_SHA:-}" ;;
    "")           die "событие не задано (EVENT_NAME пуст) — барьер зовут не так, как его зовёт CI" ;;
    *)            die "неизвестное событие '${EVENT_NAME}' — база не определена" ;;
  esac
fi
[ -n "${raw}" ] || die "база события пуста (EVENT_NAME=${EVENT_NAME:-?})"
case "${raw}" in
  *[!0]*) : ;;                                       # есть ненулевой символ — не zero-SHA
  *)      die "база = zero-SHA (создание ветки / force-push) — целостность не доказуема" ;;
esac
git rev-parse -q --verify "${raw}^{commit}" >/dev/null 2>&1 \
  || die "база '${raw}' отсутствует в истории (force-push / поверхностный клон)"
git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null \
  || die "база '${raw}' НЕ предок HEAD — история переписана (force-push); что введено, недоказуемо"

# ── helpers ───────────────────────────────────────────────────────────────────────
all_refs() { git for-each-ref --format='%(refname)' refs/remotes/origin refs/heads 2>/dev/null; }

# cls_num: выделяет «класс номер» из ПУТИ артефакта. Буквенный суффикс milestone'а (M-80a)
# отбрасывается: семья определяется числом, законность дробления решает subject_of.
#
# ⚠ Regex-парсинг опасный: `s/^C-0*([0-9]+).*/\1/` ЖАДНО съедает `0*` и оставляет `\1=1` для
# `C-001` (потому что `0 ∈ [0-9]`). Поэтому группа `\1` = ВСЕ цифры номера, а ведущие нули
# снимаются потом в bash через `10#$n`. Это та же дисциплина, что в next_artifact_id.sh.
cls_num() {
  case "$1" in
    milestones/M-*)           printf 'M %s\n' "$(basename "$1" | sed -E 's/^M-([0-9]+).*/\1/')" ;;
    research/reviews/R-*)     printf 'R %s\n' "$(basename "$1" | sed -E 's/^R-([0-9]+).*/\1/')" ;;
    research/critiques/C-*)   printf 'C %s\n' "$(basename "$1" | sed -E 's/^C-([0-9]+).*/\1/')" ;;
    research/arbitration/A-*) printf 'A %s\n' "$(basename "$1" | sed -E 's/^A-([0-9]+).*/\1/')" ;;
    *) return 1 ;;
  esac
}

# Слаг имени (правило 4): часть после `<CLS>-<NNN>-` до `.md`, без суффиксов
# `-rev<N>` и `-addendum*`. Пустая строка → sentinel `<без-слага>` (правило 5).
slug_of() {
  basename "$1" .md | sed -E 's/^(M|R|C|A)-[0-9]+[a-z]?-?//; s/-rev[0-9]+$//; s/-addendum.*$//'
}

# Предмет из тела файла (правило 1): блок от `**Предмет:**` / `**Контекст` до ПЕРВОЙ
# пустой строки; внутри — первый backtick-путь. Строка-продолжение участвует.
subject_from_body() {
  printf '%s\n' "$1" \
    | awk '/^\*\*(Предмет:|Контекст\*\*|Контекст\.\*\*)/{f=1} f{print} f&&/^$/{exit}' \
    | grep -oE '`[^`]+`' | head -1 | tr -d '`'
}

# subject_id: первое сработавшее правило из §3.1.
# $1 = ref:path (например refs/heads/main:research/reviews/R-700-alpha.md)
subject_of() {
  local body hdr path="${1#*:}"
  body=$(git show "$1" 2>/dev/null) || return 1
  hdr=$(subject_from_body "${body}")
  if [ -n "${hdr}" ]; then printf '%s\n' "${hdr}"; return 0; fi
  local s; s=$(slug_of "${path}")
  [ -z "${s}" ] && s='<без-слага>'
  printf '%s\n' "${s}"
}

# ── universe: всё, что ЗАНИМАЕТ номер в объединении ref'ов ────────────────────────
# Поток строк «класс номер subject», отсортированных и уникальных.
# `-z` + `read -r -d ''` — ОБЯЗАТЕЛЬНАЯ пара: в текстовом режиме git КВОТИРУЕТ не-ASCII имена
# (`"research/reviews/R-940-\320\260.md"`), CLASS_RE не совпал бы, и артефакт под русским
# именем выпал бы из подсчёта занятости — коллизия проходит молча. Тот же приём ниже в
# introduced() — обе половины единообразны (`R-046` Б-3).
universe() {
  local ref f cn subj
  for ref in $(all_refs); do
    while IFS= read -r -d '' f; do
      [[ "${f}" =~ ${CLASS_RE} ]] || continue
      cn=$(cls_num "${f}") || continue
      subj=$(subject_of "${ref}:${f}")
      [ -n "${subj}" ] || continue
      printf '%s %s\n' "${cn}" "${subj}"
    done < <(git ls-tree -r -z --name-only "${ref}" 2>/dev/null)
  done
  # TD — записью в TECH-DEBT.md, не файлом (правило 2). Слаг — в обратных кавычках.
  for ref in $(all_refs); do
    git show "${ref}:TECH-DEBT.md" 2>/dev/null \
      | sed -nE 's/^- \*\*TD-0*([0-9]+)\*\* `([^`]+)`.*/TD \1 \2/p'
  done
  return 0
}

# ── introduced: что ВВЕДЕНО коммитами проверяемого диапазона ───────────────────────
# Идём по `git rev-list BASE..HEAD`. `--no-renames` ломает rename-детект: переименование
# в занятый номер превращается в D + A → краснеет как «новый файл» (ось 4).
# `-z` + `read -r -d ''` — единообразно с universe() выше: см. комментарий там
# (R-046 Б-3: «приведи обе половины к одному приёму»).
#
# `--diff-filter=A` (только Added) — НЕ `--diff-filter=AM`. Правка существующего файла
# (M) предмет не вводит: инвариант §4.1 сформулирован от ВВЕДЕНИЯ. Буква M считала
# «введением» коммит, который лишь редактирует файл — барьер блокировал ОБСЛУЖИВАНИЕ
# предсуществующих коллизий (9 файлов в main: R-035, R-038, M-46, C-018, C-024), работая
# против собственного §4.1 (R-046 Б-2; architect commit 04de69c перевёл на это эталон
# и добавил сценарий L4MOD — проба краснела до закрытия дыры в барьере).
introduced() {
  local c f cn subj
  for c in $(git rev-list "${raw}..HEAD" 2>/dev/null); do
    while IFS= read -r -d '' f; do
      [[ "${f}" =~ ${CLASS_RE} ]] || continue
      cn=$(cls_num "${f}") || continue
      subj=$(subject_of "${c}:${f}")
      [ -n "${subj}" ] || continue
      printf '%s %s\n' "${cn}" "${subj}"
    done < <(git show --name-only --no-renames --diff-filter=A --format= -z "${c}" 2>/dev/null || true)
    # Новая запись в TECH-DEBT.md (TD — носитель §3.1 правило 2)
    git show "${c}" -- TECH-DEBT.md 2>/dev/null \
      | sed -nE 's/^\+- \*\*TD-0*([0-9]+)\*\* `([^`]+)`.*/TD \1 \2/p'
  done
  return 0
}

# ── main ──────────────────────────────────────────────────────────────────────────
U=$(universe | sort -u)
IN=$(introduced | sort -u)
[ -z "${IN}" ] && { echo "OK: ни один новый артефакт не введён (диапазон ${raw:0:7}..HEAD)"; exit 0; }

violations=0
while read -r cls num subj; do
  [ -z "${cls:-}" ] && continue
  num=$((10#${num}))                                  # нормализуем к без-лидирующих нулей
  while read -r c2 n2 s2; do
    [ -z "${c2}" ] && continue
    n2=$((10#${n2}))
    [ "${c2}" = "${cls}" ] && [ "${n2}" = "${num}" ] && [ "${s2}" != "${subj}" ] \
      && { echo "FAIL  ${cls}-${num}: введённый предмет «${subj}» отличается от уже занятого «${s2}»"
           violations=$((violations + 1))
           break; }
  done <<< "${U}"
done <<< "${IN}"

if [ "${violations}" -gt 0 ]; then
  echo
  echo "Нарушен инвариант §4.1: коммит диапазона вводит номер, под которым в объединении"
  echo "ref'ов уже лежит ДРУГОЙ предмет. Предсуществующая коллизия легитимна, только если"
  echo "её нет в проверяемом диапазоне — а у нас есть."
  exit 1
fi

echo "OK: ни один коммит диапазона ${raw:0:7}..HEAD не ввёл второй предмет под занятым номером"
