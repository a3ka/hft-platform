#!/usr/bin/env bash
# red_diff_contract_schema.sh — авторский self-test для scripts/diff_contract_schema.sh
# (+ scripts/diff_contract_schema.py), R-006 F-3.
#
# До этого self-test у diff_contract_schema.sh (214 строк содержательной классификационной
# логики — самый сложный компонент набора трёх контрактных гейтов) не было МАШИННОГО
# доказательства "ловит breaking / не поднимает тревогу на additive". В CI он выполнялся
# только на реальном дифе, который в подавляющем большинстве прогонов пуст (CLASS=none) —
# F-1 (перенацеливание $ref классифицировалось как CLASS=none и печатало ФАКТИЧЕСКИ ЛОЖНОЕ
# "схема не изменилась") выжила именно поэтому: ручные мутационные проверки reviewer'а
# доказали дефект, но в репозитории не осталось МАШИННОГО оракула, который бы ловил регресс.
#
# Каждый сценарий строит синтетическую repo-песочницу (минимальная JSON Schema с полями,
# oneOf-вариантами, $ref-свойством и массивом с $ref-items) и проверяет И класс (CLASS=...),
# И VERDICT, И exit-код — не только "упало/не упало" (в отличие от expect() в
# red_ct_rfc_atomic.sh, см. F-5 reviewer'а: различаем ПРИЧИНУ, не только факт отказа).
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/diff_contract_schema.sh}"

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

# ── база-репозиторий: минимальная схема с properties/required/$ref/oneOf/array-items ──
new_base_repo() {
  local d; d=$(mktemp -d)
  git -C "${d}" init -q
  git -C "${d}" config user.email t@t.local
  git -C "${d}" config user.name t
  mkdir -p "${d}/crates/contracts/schema" "${d}/crates/contracts/src"
  cat > "${d}/crates/contracts/src/lib.rs" <<'EOF'
pub const SCHEMA_VERSION: u32 = 1;
EOF
  cat > "${d}/crates/contracts/schema/event.schema.json" <<'EOF'
{
  "definitions": {
    "Widget": {
      "type": "object",
      "required": ["name"],
      "properties": {
        "name": {"type": "string"},
        "payload": {"$ref": "#/definitions/PayloadA"},
        "tags": {"type": "array", "items": {"$ref": "#/definitions/Tag"}}
      }
    },
    "PayloadA": {"type": "object", "properties": {"a": {"type": "string"}}},
    "PayloadB": {"type": "object", "properties": {"b": {"type": "string"}}},
    "Tag": {"type": "string"},
    "Mode": {
      "oneOf": [
        {"required": ["X"], "properties": {"X": {"type": "object"}}},
        {"required": ["Y"], "properties": {"Y": {"type": "object"}}}
      ]
    }
  }
}
EOF
  echo 'unrelated readme' > "${d}/README.md"
  git -C "${d}" add -A >/dev/null
  git -C "${d}" commit -qm "base: T1-скелет схемы"
  echo "${d}"
}

# $1=repo $2=python-выражение над словарём `d` (schema JSON уже загружен в `d`)
# ВЫЗЫВАТЬ В ОДИНАРНЫХ КАВЫЧКАХ, чтобы $ref/литералы не разворачивались текущим shell'ом.
mutate_schema() {
  local repo="$1" expr="$2"
  python3 -c "
import json, sys
p = sys.argv[1]
d = json.load(open(p))
${expr}
json.dump(d, open(p, 'w'), indent=2)
" "${repo}/crates/contracts/schema/event.schema.json"
}

bump_version() { # $1=repo $2=новая версия
  python3 -c "
import re, sys
p = sys.argv[1]
s = open(p).read()
s = re.sub(r'SCHEMA_VERSION: u32 = \d+', f'SCHEMA_VERSION: u32 = {sys.argv[2]}', s)
open(p, 'w').write(s)
" "${1}/crates/contracts/src/lib.rs" "$2"
}

commit_change() { # $1=repo $2=message
  git -C "$1" add -A >/dev/null
  git -C "$1" commit -qm "$2"
}

# Барьер зовётся ровно тем интерфейсом, каким его зовёт реальный вызов: позиционный
# base-ref-аргумент, рабочий каталог = корень репо-песочницы (diff_contract_schema.sh НЕ cd,
# оперирует ТЕКУЩИМ рабочим деревом вызывающего).
OUT=""
RC=0
run_barrier() { # $1=repo $2=base-ref-arg -> заполняет OUT/RC
  OUT="$(cd "$1" && bash "${BARRIER}" "$2" 2>&1)"
  RC=$?
}

# Проверяем ОДНОВРЕМЕННО класс, VERDICT-строку и exit-код (не только факт отказа, F-5 reviewer'а
# для соседнего self-test — здесь различаем причину с самого начала).
expect_class_verdict() { # $1=label $2=expected-class $3=expected-verdict(PASS|FAIL) $4=ok|deny
  local label="$1" exp_class="$2" exp_verdict="$3" exp_rc="$4" ok=1
  if ! printf '%s\n' "${OUT}" | grep -q "^CLASS=${exp_class}\$"; then ok=0; fi
  if ! printf '%s\n' "${OUT}" | grep -q "^VERDICT: ${exp_verdict}"; then ok=0; fi
  if [ "${exp_rc}" = "ok" ] && [ "${RC}" -ne 0 ]; then ok=0; fi
  if [ "${exp_rc}" = "deny" ] && [ "${RC}" -eq 0 ]; then ok=0; fi
  if [ "${ok}" -eq 1 ]; then
    pass "${label}"
  else
    fail "${label} — rc=${RC}, ожидание CLASS=${exp_class} VERDICT=${exp_verdict} exit=${exp_rc}"
    printf '%s\n' "${OUT}" | sed 's/^/    /'
  fi
}

# ── D1: схема не менялась (но HEAD != base-коммит — не тривиальный "тот же SHA") ──────
scenario_none() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  echo "unrelated change" >> "${d}/README.md"
  commit_change "${d}" "docs only, схема не тронута"
  run_barrier "${d}" "${base}"
  expect_class_verdict "D1 схема не менялась — CLASS=none, PASS" "none" "PASS" "ok"
  rm -rf "${d}"
}

# ── D2: новое ОПЦИОНАЛЬНОЕ свойство — additive, PASS без бампа ────────────────────────
scenario_additive_property() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  mutate_schema "${d}" 'd["definitions"]["Widget"]["properties"]["extra"] = {"type": "string"}'
  commit_change "${d}" "additive: новое опциональное свойство"
  run_barrier "${d}" "${base}"
  expect_class_verdict "D2 новое опциональное свойство — CLASS=additive, PASS" "additive" "PASS" "ok"
  rm -rf "${d}"
}

# ── новый файл схемы — additive ────────────────────────────────────────────────────────
scenario_additive_new_file() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  echo '{"definitions": {}}' > "${d}/crates/contracts/schema/new-type.schema.json"
  commit_change "${d}" "additive: новый файл схемы (новый T1-тип)"
  run_barrier "${d}" "${base}"
  expect_class_verdict "новый файл схемы — CLASS=additive, PASS" "additive" "PASS" "ok"
  rm -rf "${d}"
}

# ── W2: новое ОБЯЗАТЕЛЬНОЕ свойство без бампа — breaking, FAIL ────────────────────────
scenario_breaking_required_no_bump() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  mutate_schema "${d}" 'd["definitions"]["Widget"]["properties"]["extra_req"] = {"type": "string"}; d["definitions"]["Widget"]["required"].append("extra_req")'
  commit_change "${d}" "breaking: новое обязательное свойство, без бампа"
  run_barrier "${d}" "${base}"
  expect_class_verdict "W2 новое ОБЯЗАТЕЛЬНОЕ свойство без бампа — CLASS=breaking, FAIL" "breaking" "FAIL" "deny"
  rm -rf "${d}"
}

# ── D4: тот же BREAKING + бамп SCHEMA_VERSION — PASS (связь класс↔бамп, усиление против einhard) ──
scenario_breaking_required_with_bump() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  mutate_schema "${d}" 'd["definitions"]["Widget"]["properties"]["extra_req"] = {"type": "string"}; d["definitions"]["Widget"]["required"].append("extra_req")'
  bump_version "${d}" 2
  commit_change "${d}" "breaking: новое обязательное свойство + бамп SCHEMA_VERSION"
  run_barrier "${d}" "${base}"
  expect_class_verdict "D4 BREAKING + бамп SCHEMA_VERSION — PASS" "breaking" "PASS" "ok"
  rm -rf "${d}"
}

# ── W3: смена type существующего поля без бампа — breaking, FAIL ──────────────────────
scenario_breaking_type_change_no_bump() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  mutate_schema "${d}" 'd["definitions"]["Tag"]["type"] = "integer"'
  commit_change "${d}" "breaking: смена type string->integer, без бампа"
  run_barrier "${d}" "${base}"
  expect_class_verdict "W3 смена type — CLASS=breaking, FAIL" "breaking" "FAIL" "deny"
  rm -rf "${d}"
}

# ── D3: удалён вариант oneOf без бампа — breaking, FAIL ────────────────────────────────
scenario_breaking_oneof_removed_no_bump() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  mutate_schema "${d}" 'd["definitions"]["Mode"]["oneOf"] = [v for v in d["definitions"]["Mode"]["oneOf"] if v.get("required") != ["Y"]]'
  commit_change "${d}" "breaking: удалён вариант oneOf 'Y', без бампа"
  run_barrier "${d}" "${base}"
  expect_class_verdict "D3 удалён вариант oneOf — CLASS=breaking, FAIL" "breaking" "FAIL" "deny"
  rm -rf "${d}"
}

# ── файл схемы удалён целиком — breaking, FAIL ─────────────────────────────────────────
scenario_breaking_file_removed() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  rm "${d}/crates/contracts/schema/event.schema.json"
  commit_change "${d}" "breaking: файл схемы удалён"
  run_barrier "${d}" "${base}"
  expect_class_verdict "файл схемы удалён — CLASS=breaking, FAIL" "breaking" "FAIL" "deny"
  rm -rf "${d}"
}

# ── F-1 РЕГРЕССИЯ-ГВАРД: $ref цель изменена (свойство) — до фикса CLASS=none (ЛОЖНОЕ
#    "схема не изменилась"), после фикса ОБЯЗАН быть breaking, FAIL без бампа ──────────
scenario_ref_retarget_property_no_bump() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  mutate_schema "${d}" 'd["definitions"]["Widget"]["properties"]["payload"] = {"$ref": "#/definitions/PayloadB"}'
  commit_change "${d}" "breaking: \$ref-цель payload PayloadA->PayloadB, без бампа"
  run_barrier "${d}" "${base}"
  expect_class_verdict "F-1 РЕГРЕССИЯ-ГВАРД: \$ref-цель свойства изменена — CLASS=breaking (НЕ none!), FAIL" "breaking" "FAIL" "deny"
  rm -rf "${d}"
}

# ── F-1 РЕГРЕССИЯ-ГВАРД: $ref внутри items массива изменена (5 массивов из R-006 F-1) ──
scenario_ref_retarget_array_items_no_bump() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  mutate_schema "${d}" 'd["definitions"]["Widget"]["properties"]["tags"]["items"] = {"$ref": "#/definitions/PayloadA"}'
  commit_change "${d}" "breaking: \$ref-цель tags.items Tag->PayloadA, без бампа"
  run_barrier "${d}" "${base}"
  expect_class_verdict "F-1 РЕГРЕССИЯ-ГВАРД: \$ref внутри items массива изменена — CLASS=breaking, FAIL" "breaking" "FAIL" "deny"
  rm -rf "${d}"
}

# ── D5: setup-guard — несуществующий base-ref → FAIL, не молчаливый пропуск ───────────
scenario_bad_base_ref() {
  local d; d=$(new_base_repo)
  run_barrier "${d}" "nonexistent-ref-xyz123"
  if [ "${RC}" -ne 0 ] && printf '%s\n' "${OUT}" | grep -q "^VERDICT: FAIL"; then
    pass "D5 несуществующий base-ref — FAIL (setup-guard, fail-closed)"
  else
    fail "D5 несуществующий base-ref — rc=${RC} (ожидался FAIL)"
    printf '%s\n' "${OUT}" | sed 's/^/    /'
  fi
  rm -rf "${d}"
}

# ── D6: setup-guard — классификатор .py отсутствует → FAIL, не молчаливый пропуск ─────
scenario_missing_classifier() {
  local d base fake_dir out rc
  d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  fake_dir="$(mktemp -d)"
  cp "${ROOT}/scripts/diff_contract_schema.sh" "${fake_dir}/diff_contract_schema.sh"
  # Намеренно НЕ копируем diff_contract_schema.py рядом — классификатор ищется относительно
  # каталога .sh-скрипта (dirname "$0"), не .py в реальном репозитории.
  out="$(cd "${d}" && bash "${fake_dir}/diff_contract_schema.sh" "${base}" 2>&1)"
  rc=$?
  if [ "${rc}" -ne 0 ] && printf '%s\n' "${out}" | grep -q "классификатор отсутствует"; then
    pass "D6 классификатор .py отсутствует — FAIL (setup-guard, fail-closed)"
  else
    fail "D6 классификатор .py отсутствует — rc=${rc} (ожидался FAIL с сообщением про классификатор)"
    printf '%s\n' "${out}" | sed 's/^/    /'
  fi
  rm -rf "${fake_dir}" "${d}"
}

scenario_none
scenario_additive_property
scenario_additive_new_file
scenario_breaking_required_no_bump
scenario_breaking_required_with_bump
scenario_breaking_type_change_no_bump
scenario_breaking_oneof_removed_no_bump
scenario_breaking_file_removed
scenario_ref_retarget_property_no_bump
scenario_ref_retarget_array_items_no_bump
scenario_bad_base_ref
scenario_missing_classifier

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  exit 1
fi
echo "VERDICT: PASS"
