#!/usr/bin/env bash
# RED-проба анти-плацебо для scripts/verify_ct_rfc_atomic.sh (задача 2, инструкция инженера
# явно требует: «собери синтетические good/bad диффы и убедись, что плохой ловится, хороший
# проходит» — прямое применение .claude/rules/testing.md к гейту, у которого до сих пор не
# было машинного аналога).
#
# Каждый bad-сценарий убирает РОВНО ОДИН из 6 обязательных артефактов атомарного CT-RFC
# пакета из иначе-полного диффа — доказывает, что гейт реально СМОТРИТ на каждый из них
# (а не просто на факт «что-то в дифе есть»), тот же принцип, что red_protected_artifacts.sh.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/verify_ct_rfc_atomic.sh}"

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

expect() { # $1=имя $2=ok|deny $3=actual-exit
  if [ "$2" = "ok" ] && [ "$3" -eq 0 ]; then pass "$1"
  elif [ "$2" = "deny" ] && [ "$3" -ne 0 ]; then pass "$1"
  else fail "$1 — exit=$3, ожидалось $2"; fi
}

# ── база-репозиторий: минимальный скелет T1-пакета, одна база-коммит ──────────────────
new_base_repo() {
  local d; d=$(mktemp -d)
  git -C "${d}" init -q
  git -C "${d}" config user.email t@t.local
  git -C "${d}" config user.name t
  mkdir -p "${d}/docs/rfc" "${d}/crates/contracts/src" "${d}/crates/contracts/schema" \
           "${d}/crates/contracts/fixtures/valid" "${d}/crates/contracts/fixtures/invalid" \
           "${d}/crates/contracts/tests"
  echo 'pub struct Event {}' > "${d}/crates/contracts/src/lib.rs"
  echo '{}' > "${d}/crates/contracts/schema/event.schema.json"
  echo '# contracts CHANGELOG' > "${d}/crates/contracts/CHANGELOG.md"
  echo '{"base":true}' > "${d}/crates/contracts/fixtures/valid/event-base.json"
  echo '{"base":true}' > "${d}/crates/contracts/fixtures/invalid/event-base.json"
  echo 'fn base_test() {}' > "${d}/crates/contracts/tests/base.rs"
  echo 'unrelated readme' > "${d}/README.md"
  git -C "${d}" add -A >/dev/null
  git -C "${d}" commit -qm "base: T1-скелет"
  echo "${d}"
}

# Барьер зовётся ровно тем интерфейсом, каким его зовёт реальный вызов: позиционный
# base-ref-аргумент, рабочий каталог = корень репо-песочницы.
run_barrier() { # $1=repo $2=base-ref-arg -> exit code
  ( cd "$1" && bash "${BARRIER}" "$2" >/dev/null 2>&1 )
  echo $?
}

# ── setup-guard пробы: доказываем, что мутация РЕАЛЬНО коснулась (или НЕ коснулась) src ──
touches_src() { # $1=repo $2=base-sha
  git -C "$1" diff --name-only "$2" -- . | grep -q "^crates/contracts/src/"
}

# ── P0: правка вне crates/contracts/src — тривиальный PASS ────────────────────────────
scenario_no_touch() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  echo "docs change, no contracts touch" >> "${d}/README.md"
  git -C "${d}" add -A >/dev/null
  git -C "${d}" commit -qm "docs only"
  if touches_src "${d}" "${base}"; then
    fail "P0 SETUP: правка НЕОЖИДАННО коснулась crates/contracts/src — сценарий сломан"
    rm -rf "${d}"; return
  fi
  local rc; rc=$(run_barrier "${d}" "${base}")
  expect "P0 без правки crates/contracts/src — тривиальный PASS" ok "${rc}"
  rm -rf "${d}"
}

# ── P1: good — полный пакет из 6 артефактов ────────────────────────────────────────────
scenario_good_full() {
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  echo 'pub enum NewVariant {}' >> "${d}/crates/contracts/src/lib.rs"
  echo '# CT-RFC-06 — новый вариант' > "${d}/docs/rfc/CT-RFC-06-new-variant.md"
  echo '{"changed":true}' > "${d}/crates/contracts/schema/event.schema.json"
  echo '## schema_version bump — CT-RFC-06' >> "${d}/crates/contracts/CHANGELOG.md"
  echo '{"v":1}' > "${d}/crates/contracts/fixtures/valid/event-new-variant.json"
  echo '{"v":0}' > "${d}/crates/contracts/fixtures/invalid/event-new-variant-bad.json"
  echo 'fn new_variant_test() {}' > "${d}/crates/contracts/tests/new_variant.rs"
  git -C "${d}" add -A >/dev/null
  git -C "${d}" commit -qm "good: полный CT-RFC пакет"
  if ! touches_src "${d}" "${base}"; then
    fail "P1 SETUP НЕ СОСТОЯЛСЯ: правка не коснулась crates/contracts/src — проба тестировала бы не то"
    rm -rf "${d}"; return
  fi
  local rc; rc=$(run_barrier "${d}" "${base}")
  expect "P1 полный пакет (rfc+schema+changelog+valid+invalid+test) — PASS" ok "${rc}"
  rm -rf "${d}"
}

# ── BAD-*: тот же good-пакет, но РОВНО одного артефакта не хватает ────────────────────
scenario_bad_missing() { # $1=что пропускаем (rfc|schema|changelog|valid|invalid|test)
  local skip="$1"
  local d base; d=$(new_base_repo); base=$(git -C "${d}" rev-parse HEAD)
  echo 'pub enum NewVariant {}' >> "${d}/crates/contracts/src/lib.rs"
  [ "${skip}" = "rfc" ]       || echo '# CT-RFC-06' > "${d}/docs/rfc/CT-RFC-06-new-variant.md"
  [ "${skip}" = "schema" ]    || echo '{"changed":true}' > "${d}/crates/contracts/schema/event.schema.json"
  [ "${skip}" = "changelog" ] || echo '## bump' >> "${d}/crates/contracts/CHANGELOG.md"
  [ "${skip}" = "valid" ]     || echo '{"v":1}' > "${d}/crates/contracts/fixtures/valid/event-new-variant.json"
  [ "${skip}" = "invalid" ]   || echo '{"v":0}' > "${d}/crates/contracts/fixtures/invalid/event-new-variant-bad.json"
  [ "${skip}" = "test" ]      || echo 'fn t() {}' > "${d}/crates/contracts/tests/new_variant.rs"
  git -C "${d}" add -A >/dev/null
  git -C "${d}" commit -qm "bad: missing ${skip}"
  if ! touches_src "${d}" "${base}"; then
    fail "BAD-${skip} SETUP НЕ СОСТОЯЛСЯ: правка не коснулась crates/contracts/src"
    rm -rf "${d}"; return
  fi
  # Доказываем, что setup реально НЕ содержит пропущенный артефакт (иначе проба плацебо сама).
  local changed; changed=$(git -C "${d}" diff --name-only "${base}" -- .)
  case "${skip}" in
    rfc)       echo "${changed}" | grep -q "^docs/rfc/" && { fail "BAD-rfc SETUP: rfc-файл всё же в дифе"; rm -rf "${d}"; return; } ;;
    schema)    echo "${changed}" | grep -q "^crates/contracts/schema/" && { fail "BAD-schema SETUP: схема всё же в дифе"; rm -rf "${d}"; return; } ;;
    changelog) echo "${changed}" | grep -q "^crates/contracts/CHANGELOG.md$" && { fail "BAD-changelog SETUP: changelog всё же в дифе"; rm -rf "${d}"; return; } ;;
    valid)     echo "${changed}" | grep -q "^crates/contracts/fixtures/valid/" && { fail "BAD-valid SETUP: valid-фикстура всё же в дифе"; rm -rf "${d}"; return; } ;;
    invalid)   echo "${changed}" | grep -q "^crates/contracts/fixtures/invalid/" && { fail "BAD-invalid SETUP: invalid-фикстура всё же в дифе"; rm -rf "${d}"; return; } ;;
    test)      echo "${changed}" | grep -q "^crates/contracts/tests/" && { fail "BAD-test SETUP: тест всё же в дифе"; rm -rf "${d}"; return; } ;;
  esac
  local rc; rc=$(run_barrier "${d}" "${base}")
  expect "BAD без '${skip}' — гейт ОБЯЗАН FAIL (анти-плацебо)" deny "${rc}"
  rm -rf "${d}"
}

# ── setup-guard: несуществующий base-ref — FAIL, не молчаливый PASS ───────────────────
scenario_bad_base_ref() {
  local d; d=$(new_base_repo)
  local rc
  ( cd "${d}" && bash "${BARRIER}" nonexistent-ref-xyz123 >/dev/null 2>&1 )
  rc=$?
  expect "setup-guard: несуществующий base-ref — FAIL (fail-closed)" deny "${rc}"
  rm -rf "${d}"
}

scenario_no_touch
scenario_good_full
for artifact in rfc schema changelog valid invalid test; do
  scenario_bad_missing "${artifact}"
done
scenario_bad_base_ref

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  exit 1
fi
echo "VERDICT: PASS"
