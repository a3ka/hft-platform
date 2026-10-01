#!/usr/bin/env bash
# verify_contracts.sh — гейт паритета контрактного слоя (T1), обещанный
# docs/05-contract-layer.md §5 («Гейт паритета: verify_contracts.sh — Rust-типы ↔ JSON Schema
# ↔ фикстуры согласованы; роундтрип-тест сериализации»), но физически не существовавший
# (docs/plans/contracts-current-state.md, дыра Д3).
#
# Реальный гейт per .claude/rules/gates.md §3: FAIL-агрегатор + exit≠0 на FAIL, никакого
# `cmd && echo PASS || echo FAIL`, финальная строка VERDICT: PASS/FAIL.
#
# ЧЕТЫРЕ проверки:
#   S0  setup-guard      — генератор/схемы/фикстуры/python-jsonschema реально на месте
#                           (урок M-40: гейт, который «прошёл», потому что нечего было
#                           проверить, хуже отсутствующего — .claude/rules/testing.md).
#   S1  схема ↔ типы     — cargo run -p contracts --example gen_schema, diff с закоммиченным.
#   S2  фикстуры ↔ схема — valid/* ОБЯЗАНЫ пройти JSON Schema, invalid/* ОБЯЗАНЫ быть отвергнуты
#                           (второе важнее: фикстура, которая должна падать и не падает, значит
#                           схема ничего не проверяет).
#   S3  CT-I-1 EventKind — grep-канарейка единственности `enum EventKind` (docs/05 §4 называет
#                           EventKind ПРИМЕРОМ канарейки, но crates/contracts/tests/ct_rfc01.rs
#                           покрывает только Venue/MdPayload — дыра Д4). Живёт ЗДЕСЬ, не в
#                           sacred-тесте: crates/contracts/** architect-only, я его не трогаю.
#   S4  roundtrip        — cargo test -p contracts (весь RFC RED-suite крейта).
#
# ВАЖНО: S1 временно перезаписывает crates/contracts/schema/*.json (регенерация — часть
# самой проверки), но ГАРАНТИРОВАННО восстанавливает исходное содержимое (trap на EXIT,
# сработает даже при аварийном обрыве) — рабочее дерево остаётся чистым после любого исхода.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

CONTRACTS_DIR="crates/contracts"
SCHEMA_DIR="${CONTRACTS_DIR}/schema"
FIXTURES_VALID="${CONTRACTS_DIR}/fixtures/valid"
FIXTURES_INVALID="${CONTRACTS_DIR}/fixtures/invalid"
GEN_EXAMPLE="${CONTRACTS_DIR}/examples/gen_schema.rs"
FIXTURE_VALIDATOR="scripts/contracts_validate_fixtures.py"

FAILED=0
STEP_LOG="$(mktemp -t verify_contracts.XXXXXX.log)"
SCHEMA_BACKUP_DIR=""

restore_schema_backup() {
  if [ -n "${SCHEMA_BACKUP_DIR}" ] && [ -d "${SCHEMA_BACKUP_DIR}" ]; then
    cp "${SCHEMA_BACKUP_DIR}"/*.json "${SCHEMA_DIR}/" 2>/dev/null || true
    rm -rf "${SCHEMA_BACKUP_DIR}"
    SCHEMA_BACKUP_DIR=""
  fi
}
cleanup() {
  rm -f "${STEP_LOG}"
  restore_schema_backup
}
trap cleanup EXIT

pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

check() {
  local label="$1"; shift
  if "$@" >"${STEP_LOG}" 2>&1; then
    pass "${label}"
  else
    fail "${label}"
    tail -30 "${STEP_LOG}"
  fi
}

# ── S0: setup-guard (fail-closed — недостающее — FAIL, не молчаливый пропуск) ─────────
setup_guard() {
  [ -f "${GEN_EXAMPLE}" ] || { echo "генератор схемы отсутствует: ${GEN_EXAMPLE}"; return 1; }
  [ -d "${SCHEMA_DIR}" ] || { echo "каталог схем отсутствует: ${SCHEMA_DIR}"; return 1; }
  compgen -G "${SCHEMA_DIR}/*.json" >/dev/null 2>&1 || { echo "каталог схем пуст: ${SCHEMA_DIR}"; return 1; }
  [ -d "${FIXTURES_VALID}" ] || { echo "каталог valid-фикстур отсутствует: ${FIXTURES_VALID}"; return 1; }
  compgen -G "${FIXTURES_VALID}/*.json" >/dev/null 2>&1 || { echo "каталог valid-фикстур пуст"; return 1; }
  [ -d "${FIXTURES_INVALID}" ] || { echo "каталог invalid-фикстур отсутствует: ${FIXTURES_INVALID}"; return 1; }
  compgen -G "${FIXTURES_INVALID}/*.json" >/dev/null 2>&1 || { echo "каталог invalid-фикстур пуст"; return 1; }
  [ -f "${FIXTURE_VALIDATOR}" ] || { echo "валидатор фикстур отсутствует: ${FIXTURE_VALIDATOR}"; return 1; }
  command -v python3 >/dev/null 2>&1 || { echo "python3 не найден в PATH"; return 1; }
  python3 -c "import jsonschema" >/dev/null 2>&1 || {
    echo "python3 модуль jsonschema не установлен (pip install jsonschema) — S2 не может выполниться"
    return 1
  }
  command -v cargo >/dev/null 2>&1 || { echo "cargo не найден в PATH"; return 1; }
  return 0
}
check "S0 setup-guard (генератор+схемы+фикстуры+python-jsonschema+cargo)" setup_guard

if [ "${FAILED}" -gt 0 ]; then
  fail "S1 схема ↔ типы — SKIPPED (setup-guard не прошёл)"
  fail "S2 фикстуры ↔ схема — SKIPPED (setup-guard не прошёл)"
  fail "S3 CT-I-1 EventKind канарейка — SKIPPED (setup-guard не прошёл)"
  fail "S4 cargo test -p contracts — SKIPPED (setup-guard не прошёл)"
  echo
  echo "VERDICT: FAIL (${FAILED})"
  exit 1
fi

# ── S1: схема регенерируется из типов и совпадает с закоммиченной (CT-I-4) ────────────
regen_diff_schema() {
  SCHEMA_BACKUP_DIR="$(mktemp -d)"
  cp "${SCHEMA_DIR}"/*.json "${SCHEMA_BACKUP_DIR}/" || return 1

  if ! cargo run -q -p contracts --example gen_schema >/dev/null 2>&1; then
    echo "генератор gen_schema завершился с ошибкой"
    restore_schema_backup
    return 1
  fi

  local diff_out
  diff_out="$(diff -rq "${SCHEMA_BACKUP_DIR}" "${SCHEMA_DIR}" 2>&1 || true)"
  restore_schema_backup

  if [ -n "${diff_out}" ]; then
    echo "схема разошлась с Rust-типами — перегенерируй и закоммить:"
    echo "  cargo run -p contracts --example gen_schema"
    echo "${diff_out}"
    return 1
  fi
  return 0
}
check "S1 схема ↔ типы (regen == committed, CT-I-4)" regen_diff_schema

# ── S2: фикстуры против РЕАЛЬНОГО JSON Schema валидатора (не serde-парсинг типов) ─────
check "S2 фикстуры ↔ схема (valid PASS / invalid REJECT)" python3 "${FIXTURE_VALIDATOR}"

# ── S3: CT-I-1 канарейка на EventKind (docs/05 §4 называет его примером; ct_rfc01.rs —
#        нет, покрывает только Venue/MdPayload — дыра Д4 аудита). Живёт здесь: тесты крейта
#        contracts — sacred/architect-only, я их не трогаю (scope-guard).
canary_eventkind_single_definition() {
  local hits count
  hits="$(grep -rl "enum EventKind {" --include="*.rs" crates 2>/dev/null | grep -v "/target/" | sort || true)"
  if [ -z "${hits}" ]; then
    echo "grep 'enum EventKind {' по crates/**/*.rs дал ПУСТО — ожидалась ровно одна дефиниция"
    return 1
  fi
  count="$(printf '%s\n' "${hits}" | grep -c . || true)"
  if [ "${count}" -ne 1 ]; then
    echo "enum EventKind определён не РОВНО в одном месте (CT-I-1 нарушен):"
    printf '%s\n' "${hits}"
    return 1
  fi
  if ! printf '%s' "${hits}" | grep -q "crates/contracts/src/lib.rs$"; then
    echo "единственная дефиниция enum EventKind НЕ в crates/contracts/src/lib.rs: ${hits}"
    return 1
  fi
  echo "единственная дефиниция: ${hits}"
  return 0
}
check "S3 CT-I-1 канарейка EventKind (единственная дефиниция в contracts)" canary_eventkind_single_definition

# ── S4: roundtrip / весь RFC RED-suite крейта contracts ───────────────────────────────
check "S4 cargo test -p contracts (roundtrip + RFC RED-suite GREEN)" cargo test -p contracts

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} провал(ов))"
  exit 1
fi
echo "VERDICT: PASS"
