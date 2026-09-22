#!/usr/bin/env bash
# m88_predicates.sh — ТЕКСТОВЫЕ предикаты гейта M-88, вынесенные из `verify_M-88.sh`.
#
# Зачем отдельная библиотека (`A-036` §5.3, строка «все текстовые»): предикаты обязаны иметь
# ОТРИЦАТЕЛЬНУЮ ПРОБУ, а проба не может гонять весь `verify_M-88.sh` — тот собирает воркспейс
# и тратит минуты на сценарий. Вынесенные функции проба зовёт напрямую на фикстурах.
#
# Класс «предикат проходит по тексту» сработал ТРИЖДЫ (`C-233` R2; первая редакция шага `R4`
# самого architect'а; `C-235`), а норма профиля говорит: после третьего повторения писать
# прозу ЗАПРЕЩЕНО — механизируй. Это и есть механизм.
#
# Каждая функция: 0 — обязательство предъявлено, 1 — нет. Печать — на вызывающем.

# task2: семантика полного среза объявлена в док-комментарии НЕПОСРЕДСТВЕННО перед
# `heatmap_cells:` — и в нём же не предписано объединение.
m88_task2() {
  local gw="$1" doc
  doc=$(awk '
    /^[[:space:]]*\/\/\// { buf = buf $0 "\n"; next }
    /heatmap_cells:/ { print buf; exit }
    { buf = "" }
  ' "$gw")
  printf '%s' "$doc" | grep -qE 'ПОЛНЫЙ срез|полный срез бакета' || return 1
  printf '%s' "$doc" | grep -qE 'объедин|замещает раннее|chain' && return 1
  return 0
}

# task7: `apply` возвращает исход, и `#[must_use]` стоит НЕКОММЕНТАРНОЙ строкой рядом.
m88_task7() {
  local gw="$1" line
  line=$(grep -nE '^[[:space:]]*pub fn apply\(&mut self, frame: &Frame\) -> ApplyOutcome' "$gw" \
         | head -1 | cut -d: -f1)
  [ -n "$line" ] || return 1
  [ "$line" -gt 3 ] || return 1
  sed -n "$((line - 3)),$((line - 1))p" "$gw" | grep -qE '^[[:space:]]*#\[must_use\]' || return 1
  return 0
}

# task8: раздел результата датирован СВОИМ маркером фактуры; ревизия существует в истории и
# отличается от базы; названа команда зонда; числа до/после на месте.
# ПРЕДЕЛ НАЗВАН: истинность замера здесь не проверяется — её проверяет tester повтором
# команды по дословному мандату (`A-036` §5.3).
m88_task8() {
  local meas="$1" base_sha="${2:-d5163b5b35abbca204a8981e974bd6e5a97eb9de}" sha body
  [ -f "$meas" ] || return 1
  grep -qE '^## Результат ПОСЛЕ реализации' "$meas" || return 1
  body=$(awk '/^## Результат ПОСЛЕ реализации/,0' "$meas")
  sha=$(printf '%s' "$body" | grep -oE '<!-- FACTS: audited_head=[0-9a-f]{40}' | head -1 \
        | grep -oE '[0-9a-f]{40}')
  [ -n "$sha" ] || return 1
  [ "$sha" != "$base_sha" ] || return 1
  git cat-file -e "${sha}^{commit}" 2>/dev/null || return 1
  printf '%s' "$body" | grep -q 'wsprobe' || return 1
  printf '%s' "$body" | grep -qE 'до:.*[0-9]{3,}' || return 1
  printf '%s' "$body" | grep -qE 'после:.*[0-9]{3,}' || return 1
  return 0
}

# task9: множество изменённых ожиданий берётся из ФАКТОВ git и сверяется с §14.1 В ОБЕ
# СТОРОНЫ: каждый изменённый файл назван; пустое множество ЗАЯВЛЕНО явно.
# Печатает в stdout список недостающих файлов (пусто = всё названо).
m88_task9() {
  local spec="$1" base="$2" changed dec miss=""
  changed=$(git diff --name-only "$base"..HEAD -- \
              'crates/gateway/tests/*.rs' 'crates/gateway-serve/tests/*.rs' 2>/dev/null \
            | grep -v 'red_m88_update_contract\.rs\|red_m88_contract_form\.rs\|red_m88_golden_vectors\.rs' \
            | xargs -r -n1 basename | sort -u)
  dec=$(awk '/^### 14.1. Решения по изменённым ожиданиям/,0' "$spec")
  if [ -z "$changed" ]; then
    printf '%s' "$dec" | grep -q 'изменённых ожиданий нет' || { echo "(пустое множество не заявлено)"; return 1; }
    return 0
  fi
  local f
  for f in $changed; do
    printf '%s' "$dec" | grep -q -- "$f" || miss="$miss $f"
  done
  [ -z "$miss" ] || { echo "$miss"; return 1; }
  return 0
}
