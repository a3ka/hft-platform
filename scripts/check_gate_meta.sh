#!/usr/bin/env bash
# Барьер привязки вердикта к предмету — M-60b G3 (GATE-META + subject-lock + ОТСУТСТВИЕ).
# Спецификация: scripts/tests/red_gate_meta.sh (GM-1..GM-27, GM-16 СОЖЖЁН) +
# milestones/M-60b-gate-mechanisms.md §4/§5.
#
# ЗАЧЕМ — три наших инцидента:
#   1. `C-062` (04.08): критик отработал круг в дереве ЧУЖОГО репозитория; в шапке вердикта
#      `Base: origin/main @ 9a0e48f0` — ревизия, которой в НАШЕЙ истории нет. Заметил человек.
#   2. Подмена предмета ПОСЛЕ проходного вердикта: «критик смотрел это» и «reviewer одобряет
#      то же самое» перестают совпадать. Лечится subject-lock'ом.
#   3. `TD-105`: M-32/33/34 уехали в `main` без единого артефакта гейта — и это было
#      НЕНАБЛЮДАЕМО. Проверка того, что ПОПАЛО в диапазон, слепа к ОТСУТСТВИЮ (`testing.md`,
#      целостность гейта, свойство 4).
#
# ── ЧЕГО БАРЬЕР НЕ ДЕЛАЕТ (граница, спека §4) ─────────────────────────────────────────
# Он читает ДЕКЛАРАЦИИ автора (шапку GATE-META, литерал `M-NN` в subject'е merge) и сверяет
# их с ФАКТАМИ git: существует ли ревизия, предок ли она HEAD, какие пути тронуты, есть ли
# файл в дереве слияния. СООТВЕТСТВИЕ вердикта предмету он НЕ ВЫЧИСЛЯЕТ — это суждение
# критика/reviewer'а. Класс «барьер вычисляет предмет артефакта» упразднён решением
# founder'а 12.08 по M-61 (вариант Б): шесть блокеров M-61 подряд жили ровно в нём.
#
# ── ПРОД-ФОРМА ВЫЗОВА ─────────────────────────────────────────────────────────────────
#   EVENT_NAME=push         PUSH_BEFORE=<github.event.before>            bash scripts/check_gate_meta.sh
#   EVENT_NAME=pull_request PR_BASE_SHA=<event.pull_request.base.sha>    bash scripts/check_gate_meta.sh
#   (первый позиционный аргумент — ручной прогон: база сравнения явно)
# База берётся ИЗ СОБЫТИЯ, а не из `origin/main`: на push-событии `actions/checkout` ставит
# `origin/main` на ТОЛЬКО ЧТО ЗАПУШЕННЫЙ коммит, диапазон пуст и барьер зеленел бы ВСЕГДА
# (блокер B1 `C-006`, тот же дефект, что чинили в `check_protected_artifacts.sh`).
# Переменная своего события пуста ⇒ пробуем вторую (проводка бывает неполной), затем
# fail-closed. Требуется `fetch-depth: 0`: при depth=1 любой настоящий SHA нерезолвим и
# каждая честная шапка станет ложным FAIL.
#
# ── КОДЫ ВОЗВРАТА ─────────────────────────────────────────────────────────────────────
#   0 — все тронутые вердикты диапазона привязаны к предмету, лок не нарушен, merge'и
#       milestone'ов несут вердикт reviewer'а;
#   1 — есть нарушения (печатаются ВСЕ: ни первый файл, ни первый merge не закрывают собой
#       остальные — GM-20/GM-21);
#   2 — база события не установлена достоверно / origin неизвестен (fail-closed).

set -uo pipefail

ZERO=0000000000000000000000000000000000000000
VERDICT_ENUM="REJECT NOTE APPROVE PASS CONCERNS KILL ESCALATE DECISION"
# Проходные исходы — только они запирают предмет. После REJECT/KILL/CONCERNS/ESCALATE
# правки ШТАТНЫ, и лок, красящий нормальный круг исправлений, вреднее отсутствующего
# (GM-11; спека §8 «Запретный список»).
PASSING_VERDICTS="NOTE APPROVE PASS DECISION"

FAILED=0
bad() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die() {
  echo "FAIL  $*"
  echo
  echo "Барьер fail-closed: диапазон события не установлен достоверно."
  exit 2
}

in_list() { # $1=игла $2=список через пробел
  case " $2 " in *" $1 "*) return 0 ;; *) return 1 ;; esac
}

# ── 1. База диапазона ─────────────────────────────────────────────────────────────────
raw="${1:-}"
if [ -z "${raw}" ]; then
  case "${EVENT_NAME:-}" in
    push)
      raw="${PUSH_BEFORE:-}"
      [ -n "${raw}" ] || raw="${PR_BASE_SHA:-}"
      ;;
    pull_request)
      raw="${PR_BASE_SHA:-}"
      [ -n "${raw}" ] || raw="${PUSH_BEFORE:-}"
      ;;
    "") die "событие не задано (EVENT_NAME пуст) — барьер зовут не так, как его зовёт CI" ;;
    *) die "неизвестное событие «${EVENT_NAME}» — база сравнения не определена" ;;
  esac
fi

[ -n "${raw}" ] || die "база события пуста (EVENT_NAME=${EVENT_NAME:-?}) — что вошло в push, недоказуемо"
case "${raw}" in
  *[!0]*) : ;;
  *) die "база = zero-SHA (создание ветки или force-push) — диапазон недоказуем" ;;
esac
git rev-parse -q --verify "${raw}^{commit}" >/dev/null 2>&1 \
  || die "база «${raw}» отсутствует в истории (force-push / поверхностный клон — нужен fetch-depth: 0)"
git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null \
  || die "база «${raw}» НЕ предок HEAD — история переписана; что вошло в диапазон, недоказуемо"
BASE="$(git rev-parse "${raw}^{commit}")"

# ── 2. Наш ли это репозиторий: слаг из ORIGIN, а НЕ литерал ───────────────────────────
# Сверка с зашитой константой воспроизводит ровно `C-062`: барьер, не спрашивающий origin,
# проходит весь набор и при этом не отличает наш репозиторий от чужого (GM-24 это убивает).
slug_of() { # нормализация любой формы URL → owner/name, регистр не значим
  printf '%s' "$1" | sed -E 's#/+$##; s#\.git$##; s#^.*[:/]([^:/]+/[^:/]+)$#\1#' | tr 'A-Z' 'a-z'
}
ORIGIN_URL="$(git remote get-url origin 2>/dev/null || true)"
[ -n "${ORIGIN_URL}" ] || die "у репозитория нет remote «origin» — принадлежность вердикта проверить нечем"
ORIGIN_SLUG="$(slug_of "${ORIGIN_URL}")"
[ -n "${ORIGIN_SLUG}" ] || die "origin «${ORIGIN_URL}» не приводится к виду owner/name"

echo "── GATE-META: диапазон ${BASE:0:8}..HEAD, origin=${ORIGIN_SLUG}"

# ── 3. Форма шапки, принадлежность предмету, subject-lock ─────────────────────────────
meta_of() { # $1=содержимое файла → строки ВНУТРИ блока GATE-META
  printf '%s\n' "$1" | awk '
    /<!-- GATE-META/ { inb = 1; next }
    inb && /-->/     { exit }
    inb              { print }'
}
field_of() { # $1=имя поля $2=блок шапки
  printf '%s\n' "$2" \
    | sed -n "s/^[[:space:]]*$1:[[:space:]]*//p" \
    | head -1 | sed 's/[[:space:]]*$//'
}
# Классы путей, запираемые проходным вердиктом (спека §10 — предел: только эти классы).
is_gate_class() {
  case "$1" in
    .claude/rules/*) return 0 ;;
    .github/workflows/*) return 0 ;;
    scripts/verify_*.sh) return 0 ;;
    scripts/check_*.sh) return 0 ;;
    scripts/tests/red_*.sh) return 0 ;;
    *) return 1 ;;
  esac
}

verdict_files="$(git diff --name-only --diff-filter=AM "${BASE}" HEAD -- \
  research/critiques research/reviews research/arbitration 2>/dev/null || true)"

checked=0
while IFS= read -r f; do
  [ -n "${f}" ] || continue
  case "${f}" in *.md) ;; *) continue ;; esac
  checked=$((checked + 1))

  body="$(git show "HEAD:${f}" 2>/dev/null || true)"
  if ! printf '%s\n' "${body}" | grep -q '<!-- GATE-META'; then
    bad "${f}: нет шапки GATE-META — вердикт ничем не привязан к предмету"
    continue
  fi
  meta="$(meta_of "${body}")"
  ms="$(field_of milestone "${meta}")"
  ar="$(field_of audited_repo "${meta}")"
  ab="$(field_of audited_base "${meta}")"
  ah="$(field_of audited_head "${meta}")"
  vd="$(field_of verdict "${meta}")"

  empty=""
  [ -n "${ms}" ] || empty="${empty} milestone"
  [ -n "${ar}" ] || empty="${empty} audited_repo"
  [ -n "${ab}" ] || empty="${empty} audited_base"
  [ -n "${ah}" ] || empty="${empty} audited_head"
  [ -n "${vd}" ] || empty="${empty} verdict"
  if [ -n "${empty}" ]; then
    bad "${f}: пустые поля шапки:${empty} — шапка стала ритуалом"
    continue
  fi

  # `milestone:` — ДЕКЛАРАЦИЯ (спека §4): против диффа она НЕ валидируется. Машинно —
  # непустота и форма идентификатора артефакта `КЛАСС-НОМЕР[буква]` (`gates.md` §12).
  # Форма НАМЕРЕННО шире буквального «M-NN[буква]» из спеки §4: вердикты пишутся и по
  # карточкам долга (`R-077` аудировал `TD-141`), и сузить её значило бы завести ложный
  # красный там, где предмет назван честно. Отступление названо, а не сделано молча.
  case "${ms}" in
    [A-Za-z]*-[0-9]*)
      printf '%s' "${ms}" | grep -qE '^[A-Za-z]+-[0-9]+[a-z]?$' \
        || bad "${f}: milestone «${ms}» не похож на идентификатор артефакта (КЛАСС-НОМЕР[буква])"
      ;;
    *) bad "${f}: milestone «${ms}» не похож на идентификатор артефакта (КЛАСС-НОМЕР[буква])" ;;
  esac

  if ! in_list "${vd}" "${VERDICT_ENUM}"; then
    bad "${f}: verdict «${vd}» вне перечня (${VERDICT_ENUM})"
    continue
  fi

  if [ "$(printf '%s' "${ar}" | tr 'A-Z' 'a-z')" != "${ORIGIN_SLUG}" ]; then
    bad "${f}: audited_repo «${ar}» ≠ origin этого репозитория «${ORIGIN_SLUG}» (класс C-062)"
    continue
  fi

  git rev-parse -q --verify "${ab}^{commit}" >/dev/null 2>&1 \
    || { bad "${f}: audited_base «${ab}» не существует в этой истории (класс C-062)"; continue; }
  git rev-parse -q --verify "${ah}^{commit}" >/dev/null 2>&1 \
    || { bad "${f}: audited_head «${ah}» не существует в этой истории (класс C-062)"; continue; }
  git merge-base --is-ancestor "${ah}" HEAD 2>/dev/null \
    || { bad "${f}: audited_head «${ah}» НЕ предок HEAD — аудировалась другая линия истории"; continue; }

  if in_list "${vd}" "${PASSING_VERDICTS}"; then
    touched=""
    while IFS= read -r t; do
      [ -n "${t}" ] || continue
      is_gate_class "${t}" && touched="${touched} ${t}"
    done < <(git diff --name-only "${ah}" HEAD 2>/dev/null || true)
    if [ -n "${touched}" ]; then
      if git log --format='%B' "${ah}..HEAD" 2>/dev/null | grep -q 'ALLOW-SUBJECT-CHANGE:'; then
        echo "NOTE  ${f}: subject-lock открыт явным ALLOW-SUBJECT-CHANGE (аудит-след, НЕ доказательство — F-064-6):${touched}"
      else
        bad "${f}: subject-lock — после проходного вердикта (${vd}) тронут класс «гейт»:${touched}"
        echo "      выход из лока — строка «ALLOW-SUBJECT-CHANGE: <причина>» в теле коммита диапазона"
      fi
    fi
  fi
done <<EOF
${verdict_files}
EOF

# ── 4. ОТСУТСТВИЕ вердикта (К-4): merge, называющий M-NN, обязан нести R-* в СВОЁМ дереве ─
# Судятся ВСЕ merge-коммиты диапазона, а не первый (GM-21). Не-merge коммиты не судятся:
# иначе каждый рабочий коммит потребует вердикта, и лок станет вреднее отсутствующего (GM-19).
# ПРЕДЕЛ (спека §5): merge, НЕ называющий milestone в subject'е, не покрыт — `TD-105`
# закрывается ЧАСТИЧНО, и это говорится, а не подразумевается.
merges=0
while IFS= read -r c; do
  [ -n "${c}" ] || continue
  git rev-parse -q --verify "${c}^2" >/dev/null 2>&1 || continue
  subj="$(git log -1 --format='%s' "${c}")"
  mid="$(printf '%s' "${subj}" | grep -oE 'M-[0-9]+[a-z]?' | head -1 || true)"
  [ -n "${mid}" ] || continue
  merges=$((merges + 1))
  found=""
  while IFS= read -r rf; do
    [ -n "${rf}" ] || continue
    if git show "${c}:${rf}" 2>/dev/null | grep -qE -- "${mid}([^0-9a-z]|$)"; then
      found="${rf}"
      break
    fi
  done < <(git ls-tree -r --name-only "${c}" -- research/reviews 2>/dev/null \
    | grep -E '^research/reviews/R-.*\.md$' || true)
  if [ -n "${found}" ]; then
    echo "OK    merge ${c:0:8} «${subj}» — вердикт ${found} в дереве слияния"
  else
    bad "merge ${c:0:8} называет ${mid}, но research/reviews/R-*.md с этим литералом в дереве слияния НЕТ (класс TD-105)"
  fi
done < <(git rev-list "${BASE}..HEAD" 2>/dev/null || true)

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED}) — вердикт не привязан к предмету либо merge прошёл без вердикта."
  echo "Шаблон шапки — .claude/rules/gates.md §4; проба контракта — scripts/tests/red_gate_meta.sh."
  exit 1
fi
echo "VERDICT: PASS — вердиктов проверено: ${checked}, merge'ей с milestone в subject'е: ${merges}"
