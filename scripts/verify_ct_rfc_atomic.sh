#!/usr/bin/env bash
# verify_ct_rfc_atomic.sh — машинная атомарность изменения T1 (docs/05-contract-layer.md §4:
# «Любое изменение T1-формы = атомарный contract-RFC ... В ОДНОМ PR»).
#
# До этого скрипта правило держалось словом, на внимательности ревьюера — и НЕ сработало:
# CT-RFC-05 («MarginInventory», SCHEMA_VERSION 3→4) живёт в 49 упоминаниях кода и в
# crates/contracts/CHANGELOG.md, но docs/rfc/CT-RFC-05-*.md не существует
# (docs/plans/contracts-current-state.md, дыра Д2). Этот скрипт делает правило машинным.
#
# Если дифф `<base-ref>`..рабочее-дерево трогает crates/contracts/src/**, в ТОМ ЖЕ диффе
# обязаны присутствовать:
#   (а) docs/rfc/CT-RFC-NNN-*.md         — сам RFC-документ (класс дефекта CT-RFC-05)
#   (б) crates/contracts/schema/*.json   — регенерированная схема
#   (в) crates/contracts/CHANGELOG.md    — запись
#   (г) crates/contracts/fixtures/valid/*.json    — фикстура на изменённую форму
#   (д) crates/contracts/fixtures/invalid/*.json  — фикстура на изменённую форму
#   (е) crates/contracts/tests/*.rs      — тест, ссылающийся на новую форму
#
# Правка crates/contracts/src/** НЕ обнаружена → гейт тривиально PASS (нечего проверять).
#
# Использование: bash scripts/verify_ct_rfc_atomic.sh [base-ref=origin/main]
# Анти-плацебо самопроверка: scripts/tests/red_ct_rfc_atomic.sh (синтетические good/bad
# диффы — good обязан PASS, каждый bad, недостающий РОВНО один артефакт, обязан FAIL).
set -uo pipefail

# НЕ cd в каталог скрипта (в отличие от verify_M-NN.sh) — гейт обязан оперировать ТЕКУЩИМ
# рабочим деревом вызывающего (как scripts/check_protected_artifacts.sh), иначе self-test
# (scripts/tests/red_ct_rfc_atomic.sh), гоняющий барьер против синтетических repo-песочниц
# в /tmp, молча проверял бы СОБСТВЕННЫЙ репозиторий скрипта вместо песочницы.

BASE_REF="${1:-origin/main}"

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

# ── setup-guard (fail-closed): база обязана существовать и иметь общего предка с HEAD ──
if ! git rev-parse -q --verify "${BASE_REF}^{commit}" >/dev/null 2>&1; then
  echo "FAIL  setup-guard: база '${BASE_REF}' не найдена (fetch? опечатка?) — гейт не может"
  echo "      определить дифф, это FAIL, не молчаливый пропуск"
  echo
  echo "VERDICT: FAIL"
  exit 1
fi

MERGE_BASE="$(git merge-base "${BASE_REF}" HEAD 2>/dev/null || true)"
if [ -z "${MERGE_BASE}" ]; then
  echo "FAIL  setup-guard: нет общего предка между '${BASE_REF}' и HEAD — история разошлась"
  echo
  echo "VERDICT: FAIL"
  exit 1
fi

# Дифф = merge-base..рабочее дерево (коммиты этой ветки + незакоммиченные правки — гейт
# полезен и ДО коммита, как самопроверка перед push, не только как CI-джоб постфактум).
mapfile -t CHANGED_PATHS < <(git diff --name-only "${MERGE_BASE}" -- . 2>/dev/null || true)

touched_contracts_src=0
for p in "${CHANGED_PATHS[@]:-}"; do
  case "${p}" in
    crates/contracts/src/*) touched_contracts_src=1 ;;
  esac
done

if [ "${touched_contracts_src}" -eq 0 ]; then
  pass "crates/contracts/src/** не тронут — атомарность CT-RFC пакета не применима"
  echo
  echo "VERDICT: PASS"
  exit 0
fi

pass "crates/contracts/src/** тронут — проверяю атомарность CT-RFC пакета (docs/05 §4)"

has_match() { # $1=extended-regex (bash [[ =~ ]]) — есть ли путь в дифе, матчащий его
  local re="$1" p
  for p in "${CHANGED_PATHS[@]:-}"; do
    if [[ "${p}" =~ ${re} ]]; then
      return 0
    fi
  done
  return 1
}

check_or_fail() { # $1=метка $2=regex $3=подсказка-при-провале
  local label="$1" re="$2" hint="$3"
  if has_match "${re}"; then
    pass "${label}"
  else
    fail "${label} — ${hint}"
  fi
}

check_or_fail \
  "RFC-документ (docs/rfc/CT-RFC-NNN-*.md)" \
  '^docs/rfc/CT-RFC-[0-9]+-.*\.md$' \
  "нет docs/rfc/CT-RFC-NNN-*.md в дифе — правка T1 без формального RFC-документа (класс CT-RFC-05, docs/plans/contracts-current-state.md Д2)"

check_or_fail \
  "регенерированная схема (crates/contracts/schema/*.json)" \
  '^crates/contracts/schema/.*\.json$' \
  "схема не в дифе — перегенерируй (\`cargo run -p contracts --example gen_schema\`) и закоммить"

check_or_fail \
  "CHANGELOG (crates/contracts/CHANGELOG.md)" \
  '^crates/contracts/CHANGELOG\.md$' \
  "нет записи в CHANGELOG — RFC без миграционной заметки/rationale в истории"

check_or_fail \
  "valid-фикстура (crates/contracts/fixtures/valid/*.json)" \
  '^crates/contracts/fixtures/valid/.*\.json$' \
  "нет valid-фикстуры на изменённую форму"

check_or_fail \
  "invalid-фикстура (crates/contracts/fixtures/invalid/*.json)" \
  '^crates/contracts/fixtures/invalid/.*\.json$' \
  "нет invalid-фикстуры на изменённую форму — без неё нельзя доказать, что схема ЧТО-ТО отвергает"

check_or_fail \
  "тест (crates/contracts/tests/*.rs)" \
  '^crates/contracts/tests/.*\.rs$' \
  "нет теста, ссылающегося на новую форму — RED-first (.claude/rules/testing.md) не соблюдён"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} недостающих артефакта(ов) атомарного CT-RFC пакета)"
  exit 1
fi
echo "VERDICT: PASS"
