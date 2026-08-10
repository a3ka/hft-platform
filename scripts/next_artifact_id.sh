#!/usr/bin/env bash
# next_artifact_id.sh — аллокатор следующего свободного номера артефакта (M-61, TD-111).
#
# Спека: milestones/M-61-artifact-ids.md §3 (задача 1) + §3.1 (объединение ref'ов).
#
# ОБЛАСТЬ ПОИСКА = ОБЪЕДИНЕНИЕ `refs/remotes/origin/*` ∪ `refs/heads/*`
# (спека §3.1). Локальные heads — не тестовая условность: ветка живёт в дереве до push'а,
# и номер там уже занят. Знать только refs/remotes — непроверяемо; только refs/heads —
# бесполезно на проде.
#
# FAIL-CLOSED:
#   • origin сконфигурирован, но ни одного его ref'а нет → перечислить занятость невозможно
#     (тестовые репозитории несут только локальные heads, и если обвязка отрезала origin
#     символически — это «не знаю», а не «свободно»);
#   • класс не нашёл ни одного существующего номера в объединении → setup-guard (§6.1),
#     не «сравнить 0+1=1 с пустым эталоном».

set -uo pipefail

die() { echo "FAIL  $*" >&2; exit 1; }

CLS="${1:-}"
case "${CLS}" in
  TD|R|C|A|M) : ;;                                  # поддерживаемые классы (спека §3.1, ось 1)
  *)            die "неизвестный класс '${CLS}' — допустимы: TD R C A M";;
esac

# ── fail-closed по origin (§3.1, ось 3) ────────────────────────────────────────────
if git remote get-url origin >/dev/null 2>&1; then
  n_origin=$(git for-each-ref --format='%(refname)' refs/remotes/origin 2>/dev/null \
             | wc -l | tr -d '[:space:]')
  [ "${n_origin:-0}" -gt 0 ] \
    || die "origin сконфигурирован, но ни одного его ref'а нет — занятость перечислить невозможно"
fi

# ── объединение ref'ов: origin ∪ local heads ──────────────────────────────────────
all_refs() { git for-each-ref --format='%(refname)' refs/remotes/origin refs/heads 2>/dev/null; }

max=0
for ref in $(all_refs); do
  n=""                                              # сбрасываем на каждом ref — `n` глобальная, и `n="${n}…"` иначе копит
  case "${CLS}" in
    TD)
      # TD живёт записью в TECH-DE-B.md, а не файлом; берём слаг из строки `- **TD-NNN** \`слаг\``,
      # но для номера достаточно `TD-NNN` (число).
      n=$(git show "${ref}:TECH-DEBT.md" 2>/dev/null | grep -oE 'TD-[0-9]+' | grep -oE '[0-9]+' || true)
      ;;
    M|R|C|A)
      # Номера — в именах файлов. Классы раскладываются по каталогам согласно архитектуре:
      # M → milestones/, R → research/reviews/, C → research/critiques/, A → research/arbitration/.
      # `-z` + `read -r -d ''` ОБЯЗАТЕЛЬНАЯ пара: в текстовом режиме git КВОТИРУЕТ не-ASCII имена
      # (`"research/reviews/R-940-\320\260.md"`), grep не узнал бы ни класс, ни номер — артефакт
      # выпал бы из подсчёта, и аллокатор выдал бы уже занятый номер. Тот же приём в
      # check_artifact_ids.sh (`R-046` Б-3: «приведи обе половины к одному приёму»).
      while IFS= read -r -d '' f; do
        n="${n} $(printf '%s\n' "$f" | grep -oE "(^|/)(${CLS})-[0-9]+" | grep -oE '[0-9]+')"
      done < <(git ls-tree -r -z --name-only "${ref}" 2>/dev/null || true)
      n="${n# }"                                  # снять ведущий пробел, если строка накопилась
      ;;
  esac
  for x in ${n}; do
    x=$((10#$x))                                       # 10#… снимает ведущие нули и не падает на 0NN
    [ "${x}" -gt "${max}" ] && max="${x}"
  done
done

# Setup-guard (§6.1): если объединение пусто по классу, эталон не вычислим, и сравнивать
# в шаге N гейта нечего — лучше умереть, чем выдать «1» против пустого эталона.
[ "${max}" -gt 0 ] || die "класс ${CLS}: в объединении ref'ов нет ни одного номера — setup-guard"

# Формат печати — паритет с уже существующими в репозитории: M — двузначный (M-60), остальные — трёхзначные.
case "${CLS}" in
  M) printf 'M-%02d\n' $((max + 1)) ;;
  *) printf '%s-%03d\n' "${CLS}" $((max + 1)) ;;
esac
