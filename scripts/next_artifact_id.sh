#!/usr/bin/env bash
# Аллокатор номера артефакта — M-61 (`TD-111`).
#
# Печатает следующий свободный номер для класса <TD|R|C|A|M>, считая максимум по
# ОБЪЕДИНЕНИЮ `refs/remotes/origin/*` и `refs/heads/*` (`git for-each-ref`).
#
# Двойной охват обязателен (`milestones/M-61-artifact-ids.md` §3.1): номер занимают чужие
# удалённые ветки (на проде), и ЛОКАЛЬНЫЕ head'ы (ветка живёт в дереве до push'а, а в
# фикстуре пробы удалённых ref'ов нет вовсе). Реализация, умеющая лишь один из двух
# источников, — либо непроверяема, либо бесполезна.
#
# FAIL-CLOSED: origin сконфигурирован (`git remote get-url origin` успешен), но ни одного
# `refs/remotes/origin/*` не существует → занятость перечислить невозможно → отказ.
# Это закрывает ось 3 «origin недоступен»: тихо падать на локальный максимум = fail-open
# в механизме, чья единственная задача — видеть чужие деревья.
#
# Формат вывода (спека §3.1):
#   M  → `M-NN`, две цифры (`M-91`).
#   *  → `PREFIX-NNN`, три цифры (`R-408`, `C-506`, `A-006`, `TD-111`).

set -uo pipefail
ZERO=0000000000000000000000000000000000000000

CLS="${1:?usage: $0 <TD|R|C|A|M>}"
case "${CLS}" in TD|R|C|A|M) :;; *) echo "FAIL  неизвестный класс '${CLS}' (ожидался TD|R|C|A|M)" >&2; exit 1;; esac

# ─── Страж базы ────────────────────────────────────────────────────────────────────
# Если origin сконфигурирован, но его ref'ов нет — мы НЕ знаем, что занято в origin.
# Это fail-closed: единственный безопасный ответ — отказ (и `verify_M-61.sh` шаг A ловит
# именно эту дисциплину). Отсутствие удалённого `origin` (репозиторий без remote) —
# легитимный сценарий для пробы; тогда читаем только `refs/heads/*`.
if git remote get-url origin >/dev/null 2>&1; then
  origin_refs="$(git for-each-ref --format='%(refname)' refs/remotes/origin 2>/dev/null || true)"
  [ -n "${origin_refs}" ] || { echo "FAIL  origin сконфигурирован, но refs/remotes/origin/* пуст — занятость перечислить невозможно" >&2; exit 1; }
fi

# ─── Подсчёт максимума по объединению ref'ов ───────────────────────────────────────
max=0
for ref in $(git for-each-ref --format='%(refname)' refs/remotes/origin refs/heads 2>/dev/null || true); do
  case "${CLS}" in
    TD)
      # TD живёт записью в `TECH-DEBT.md`, а не файлом — `git show <ref>:TECH-DEBT.md`.
      n="$(git show "${ref}:TECH-DEBT.md" 2>/dev/null \
            | grep -oE 'TD-[0-9]+' | grep -oE '[0-9]+' || true)"
      ;;
    *)
      # Прочие классы живут именами файлов: `milestones/M-NN-*.md` · `research/(reviews/critiques/arbitration)/<X>-NNN-*.md`.
      n="$(git ls-tree -r --name-only "${ref}" 2>/dev/null \
            | grep -oE "(^|/)${CLS}-[0-9]+" | grep -oE '[0-9]+' || true)"
      ;;
  esac
  for x in ${n}; do
    x=$((10#$x))
    [ "${x}" -gt "${max}" ] && max="${x}"
  done
done

# Нет ни одного артефакта этого класса ни в одном ref'е → выдавать нечего (а не номер 1).
[ "${max}" -gt 0 ] || { echo "FAIL  в объединении ref'ов нет ни одного артефакта класса ${CLS} — выдавать нечего" >&2; exit 1; }

# ─── Печать в формате ──────────────────────────────────────────────────────────────
case "${CLS}" in
  M) printf 'M-%02d\n' $((max + 1));;
  *) printf '%s-%03d\n' "${CLS}" $((max + 1));;
esac