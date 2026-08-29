#!/usr/bin/env bash
# diff_contract_schema.sh — классифицирует изменение crates/contracts/schema/*.json между
# <base-ref> и HEAD: additive vs breaking (перенос einhard `diff_schemas.py`,
# docs/plans/contracts-einhard-inventory.md §1.5.2, план переноса П2).
#
# УСИЛЕНИЕ против оригинала einhard (у них `verify_version_bump.py` проверяет лишь факт ЛЮБОГО
# бампа при ЛЮБОМ изменении схемы, не связывая класс изменения с уровнем бампа): если диф
# классифицирован как BREAKING, скрипт ТРЕБУЕТ, чтобы `SCHEMA_VERSION` (crates/contracts/src/
# lib.rs) на HEAD был СТРОГО БОЛЬШЕ, чем на <base-ref>. ADDITIVE-диф бамп не требует (docs/05
# §4: аддитивное — minor bump, но SCHEMA_VERSION у нас счётчик эпох эмитируемых вариантов,
# не semver — см. lib.rs; этот скрипт не навязывает bump там, где инвариант его не требует).
#
# Использование: bash scripts/diff_contract_schema.sh [base-ref=origin/main]
#
# НЕ cd в каталог скрипта — как verify_ct_rfc_atomic.sh, оперирует ТЕКУЩИМ рабочим деревом
# вызывающего (git show работает из любого подкаталога репозитория).
set -uo pipefail

CLASSIFIER="$(dirname "$0")/diff_contract_schema.py"
SCHEMA_DIR="crates/contracts/schema"
LIB_RS="crates/contracts/src/lib.rs"

BASE_REF="${1:-origin/main}"

# ── setup-guard (fail-closed) ──────────────────────────────────────────────────────────
if [ ! -f "${CLASSIFIER}" ]; then
  echo "FAIL  setup-guard: классификатор отсутствует: ${CLASSIFIER}"
  echo
  echo "VERDICT: FAIL"
  exit 1
fi
command -v python3 >/dev/null 2>&1 || {
  echo "FAIL  setup-guard: python3 не найден в PATH"
  echo
  echo "VERDICT: FAIL"
  exit 1
}
if ! git rev-parse -q --verify "${BASE_REF}^{commit}" >/dev/null 2>&1; then
  echo "FAIL  setup-guard: база '${BASE_REF}' не найдена (fetch? опечатка?)"
  echo
  echo "VERDICT: FAIL"
  exit 1
fi
if ! git rev-parse -q --verify "HEAD^{commit}" >/dev/null 2>&1; then
  echo "FAIL  setup-guard: HEAD не резолвится в коммит — вне git-репозитория?"
  echo
  echo "VERDICT: FAIL"
  exit 1
fi

TMP_BASE="$(mktemp -d)"
TMP_HEAD="$(mktemp -d)"
cleanup() { rm -rf "${TMP_BASE}" "${TMP_HEAD}"; }
trap cleanup EXIT

extract_schema_at() { # $1=ref $2=out-dir — материализует crates/contracts/schema/*.json@ref
  local ref="$1" out="$2" f name
  while IFS= read -r f; do
    [ -n "${f}" ] || continue
    name="$(basename "${f}")"
    git show "${ref}:${f}" > "${out}/${name}" 2>/dev/null || true
  done < <(git ls-tree -r --name-only "${ref}" -- "${SCHEMA_DIR}" 2>/dev/null | grep '\.json$' || true)
}
extract_schema_at "${BASE_REF}" "${TMP_BASE}"
extract_schema_at "HEAD" "${TMP_HEAD}"

if [ -z "$(ls -A "${TMP_BASE}" 2>/dev/null)" ] && [ -z "$(ls -A "${TMP_HEAD}" 2>/dev/null)" ]; then
  echo "FAIL  setup-guard: ни на '${BASE_REF}', ни на HEAD нет ни одного файла в ${SCHEMA_DIR} —"
  echo "      нечего классифицировать (это FAIL, не молчаливый 'нет изменений')"
  echo
  echo "VERDICT: FAIL"
  exit 1
fi

OUT="$(python3 "${CLASSIFIER}" "${TMP_BASE}" "${TMP_HEAD}")"
PY_RC=$?
echo "${OUT}"
if [ "${PY_RC}" -ne 0 ]; then
  echo
  echo "FAIL  классификатор завершился с ошибкой (exit=${PY_RC})"
  echo "VERDICT: FAIL"
  exit 1
fi

CLASS="$(printf '%s\n' "${OUT}" | grep '^CLASS=' | tail -1 | cut -d= -f2)"
echo

case "${CLASS}" in
  none)
    echo "PASS  схема ${SCHEMA_DIR} не изменилась между ${BASE_REF} и HEAD"
    echo
    echo "VERDICT: PASS"
    exit 0
    ;;
  additive)
    echo "PASS  диф классифицирован ADDITIVE (обратно совместимо) — bump SCHEMA_VERSION не требуется этим гейтом"
    echo
    echo "VERDICT: PASS"
    exit 0
    ;;
  breaking)
    echo "диф классифицирован BREAKING — проверяю bump SCHEMA_VERSION (обязателен, усиление против einhard)"
    ;;
  *)
    echo "FAIL  классификатор вернул неожиданный класс '${CLASS:-<пусто>}'"
    echo
    echo "VERDICT: FAIL"
    exit 1
    ;;
esac

extract_schema_version() { # $1=ref -> число, либо пусто, если константа не нашлась
  git show "$1:${LIB_RS}" 2>/dev/null \
    | grep -oE 'pub const SCHEMA_VERSION: *u32 *= *[0-9]+' \
    | grep -oE '[0-9]+$' \
    | head -1
}
BASE_VER="$(extract_schema_version "${BASE_REF}")"
HEAD_VER="$(extract_schema_version "HEAD")"

if [ -z "${BASE_VER}" ] || [ -z "${HEAD_VER}" ]; then
  echo "FAIL  не удалось извлечь SCHEMA_VERSION из ${LIB_RS} (base='${BASE_VER:-?}' head='${HEAD_VER:-?}')"
  echo
  echo "VERDICT: FAIL"
  exit 1
fi

if [ "${HEAD_VER}" -le "${BASE_VER}" ]; then
  echo "FAIL  BREAKING диф БЕЗ bump'а SCHEMA_VERSION: base=${BASE_VER} head=${HEAD_VER} (обязано head > base)"
  echo
  echo "VERDICT: FAIL"
  exit 1
fi

echo "PASS  BREAKING диф корректно сопровождён bump'ом SCHEMA_VERSION: ${BASE_VER} → ${HEAD_VER}"
echo
echo "VERDICT: PASS"
