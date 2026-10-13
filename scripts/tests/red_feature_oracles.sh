#!/usr/bin/env bash
# Проба барьера `scripts/check_feature_oracles.sh` (TD-253) — АНТИ-ПЛАЦЕБО В ОБЕ СТОРОНЫ.
#
# Краснеет: шага нет; шаг сужен `--test`; код возврата заглушён (`|| true`,
# `continue-on-error`); шаг условный (`if:` — `R-257` Б-1); шаг закомментирован; шаг в
# чужом джобе; покрыт не каждый крейт, в т.ч. второй крейт в форме `cfg(all(…))` (Н-1);
# покрыта не та фича; на дереве не найдено ни одного оракула под фичей (сломанный поиск).
# Молчит: точный шаг в build-test; атрибут только внутри строкового литерала.
# Плюс ЖИВОЕ дерево репозитория — та форма, которую судит CI.
#
# `FO_BARRIER=<файл>` — ТОЛЬКО для мутационного контроля (подмена испытуемого барьера).
# Число сценариев НЕ заявляется — считается и печатается.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BARRIER="${FO_BARRIER:-$ROOT/scripts/check_feature_oracles.sh}"
[ -f "$BARRIER" ] || { echo "SETUP НЕ СОСТОЯЛСЯ: барьер $BARRIER отсутствует"; exit 1; }

PASS=0; FAIL=0
ok()   { printf 'pass  %s\n' "$*"; PASS=$((PASS+1)); }
nope() { printf 'FAIL  %s\n' "$*"; FAIL=$((FAIL+1)); }

SANDBOX="$(mktemp -d)"; trap 'rm -rf "$SANDBOX"' EXIT

# Песочница: crate=<имя>:<фича>:<форма атрибута inner|outer|literal|all|any> через запятую;
# ci — тело файла workflow; envx — доп. окружение; must — фрагмент, который фикстура workflow
# ОБЯЗАНА нести (setup-страж и на workflow, `R-257` Н-6). Печатает exit барьера либо 99.
run_case() {
  local crates="$1" ci="$2" envx="${3:-}" must="${4:-}" dir spec c f form
  dir="$SANDBOX/c$RANDOM$RANDOM"; mkdir -p "$dir/.github/workflows" || { echo 99; return; }
  IFS=',' read -ra spec <<<"$crates"
  for s in "${spec[@]}"; do
    [ -n "$s" ] || continue
    IFS=':' read -r c f form <<<"$s"
    mkdir -p "$dir/crates/$c/tests" || { echo 99; return; }
    case "$form" in
      inner)   printf '#![cfg(feature = "%s")]\n#[test]\nfn t() {}\n' "$f" > "$dir/crates/$c/tests/t.rs" ;;
      outer)   printf '#[cfg(feature = "%s")]\n#[test]\nfn t() {}\n' "$f" > "$dir/crates/$c/tests/t.rs" ;;
      all)     printf '#![cfg(all(feature = "%s", unix))]\n#[test]\nfn t() {}\n' "$f" > "$dir/crates/$c/tests/t.rs" ;;
      any)     printf '#[cfg(any(test, feature = "%s"))]\n#[test]\nfn t() {}\n' "$f" > "$dir/crates/$c/tests/t.rs" ;;
      literal) printf '#[test]\nfn t() { let _ = "#[cfg(feature = \\"%s\\")]"; }\n' "$f" > "$dir/crates/$c/tests/t.rs" ;;
    esac
    [ -s "$dir/crates/$c/tests/t.rs" ] || { echo 99; return; }
  done
  printf '%s\n' "$ci" > "$dir/.github/workflows/ci.yml"
  # setup-страж: фикстура обязана нести ровно то, что заявлено, иначе сценарий судит пустоту
  for s in "${spec[@]}"; do
    [ -n "$s" ] || continue; IFS=':' read -r c f form <<<"$s"
    grep -q "feature = \\\\\?\"$f" "$dir/crates/$c/tests/t.rs" || { echo 99; return; }
  done
  if [ -n "$must" ]; then grep -qF -- "$must" "$dir/.github/workflows/ci.yml" || { echo 99; return; }; fi
  env $envx FO_ROOT="$dir" FO_CI=".github/workflows/ci.yml" bash "$BARRIER" >"$dir/out" 2>&1
  echo $?
}

expect() { # $1 имя, $2 ожидаемый (0|1), $3 фактический
  if [ "$3" = 99 ]; then nope "$1 — SETUP НЕ СОСТОЯЛСЯ"; elif [ "$3" = "$2" ]; then ok "$1 (exit=$3)"; else nope "$1 — ожидался exit=$2, получен $3"; fi
}

HDR='name: CI
on: [push]
jobs:
  build-test:
    runs-on: ubuntu-latest
    steps:
      - name: test
        run: cargo test --all'
TAIL='  other:
    runs-on: ubuntu-latest
    steps:
      - run: echo x'
STEP='      - name: feature oracles
        run: cargo test -p gs --features testing'

expect "s1 шага нет → FAIL"                         1 "$(run_case gs:testing:inner "$HDR
$TAIL")"
expect "s2 точный шаг в build-test → PASS"          0 "$(run_case gs:testing:inner "$HDR
$STEP
$TAIL")"
expect "s3 шаг сужен --test → FAIL"                 1 "$(run_case gs:testing:inner "$HDR

      - run: cargo test -p gs --features testing --test t
$TAIL" "" "--test t")"
expect "s4 код возврата заглушён || true → FAIL"   1 "$(run_case gs:testing:inner "$HDR
      - run: cargo test -p gs --features testing || true
$TAIL" "" "|| true")"
expect "s5 шаг закомментирован → FAIL"              1 "$(run_case gs:testing:inner "$HDR
      # - run: cargo test -p gs --features testing
$TAIL" "" "# - run: cargo test")"
expect "s6 шаг в чужом джобе → FAIL"                1 "$(run_case gs:testing:inner "$HDR
$TAIL
      - run: cargo test -p gs --features testing")"
expect "s7 continue-on-error в build-test → FAIL"   1 "$(run_case gs:testing:inner "$HDR
$STEP
        continue-on-error: true
$TAIL" "" "continue-on-error: true")"
expect "s8 два крейта, покрыт один → FAIL"          1 "$(run_case gs:testing:inner,jr:testing:outer "$HDR
$STEP
$TAIL")"
expect "s9 покрыта не та фича → FAIL"               1 "$(run_case gs:slow:outer "$HDR
$STEP
$TAIL")"
expect "s10 атрибут только в литерале → PASS"       0 "$(run_case gs:testing:literal "$HDR
$TAIL" FO_ALLOW_EMPTY=1)"
expect "s11 ни одного оракула под фичей → FAIL"     1 "$(run_case gs:testing:literal "$HDR
$TAIL")"
expect "s12 внешний атрибут, точный шаг → PASS"     0 "$(run_case gs:testing:outer "$HDR
$STEP
$TAIL")"
expect "s13 if: false на шаге → FAIL"               1 "$(run_case gs:testing:inner "$HDR
      - name: feature oracles
        if: false
        run: cargo test -p gs --features testing
$TAIL" "" "if: false")"
expect "s14 if: только на push → FAIL"              1 "$(run_case gs:testing:inner "$HDR
      - name: feature oracles
        if: github.event_name == 'push'
        run: cargo test -p gs --features testing
$TAIL" "" "if: github.event_name")"
expect "s15 второй крейт в форме cfg(all) без шага → FAIL" 1 "$(run_case gs:testing:inner,jr:testing:all "$HDR
$STEP
$TAIL")"
expect "s16 форма cfg(any) с точным шагом → PASS"   0 "$(run_case gs:testing:any "$HDR
$STEP
$TAIL")"

# Живое дерево — прод-форма: CI судит именно его.
( cd "$ROOT" && FO_ROOT="$ROOT" bash "$BARRIER" >/dev/null 2>&1 ); expect "live: дерево репозитория → PASS" 0 "$?"

echo
echo "сценариев: $((PASS + FAIL)), pass=$PASS, FAIL=$FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL"; exit 1
