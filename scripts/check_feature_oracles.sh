#!/usr/bin/env bash
# check_feature_oracles.sh — ОРАКУЛ ПОД `--features testing` ОБЯЗАН ИСПОЛНЯТЬСЯ В CI (TD-253).
#
# ЗАЧЕМ. Тесты крейта, стоящие под `#[cfg(feature = "testing")]` (точки остановки
# `test_sync::rendezvous` и т.п.), обычный `cargo test --all` НЕ ИСПОЛНЯЕТ: без фичи они не
# компилируются в тестовый бинарь вовсе — и не краснеют, и не значатся пропущенными. До
# TD-253 их исполнял ТОЛЬКО `verify_M-NN.sh` своего милестоуна; после переезда гейта в
# `docs/archive/` по норме Р-2 сторож замолкал навсегда. Замер `R-255` Н-2 на `48d8bbc0`:
# `red_m95_provenance_fresh` f0…f6 (fail-closed провенанс `VB-I-11`), `red_m91_legacy_counters`
# l4, часть `red_m87_entrypoint`, `red_ws_session` — ни одного живого исполнителя, кроме
# `verify_M-65.sh` для одного файла. `clippy --all-features` их КОМПИЛИРУЕТ, но не ГОНЯЕТ.
#
# ИНВАРИАНТ. Для КАЖДОГО крейта, в `tests/` которого есть атрибут `cfg(feature = "<f>")`
# (вне строкового литерала), джоб `build-test` в `.github/workflows/ci.yml` несёт шаг
# РОВНО `run: cargo test -p <крейт> --features <f>` — весь крейт, без `--test`-сужения:
# новый файл под фичей покрывается без правки CI. Шаг не смеет глушить код возврата
# (`|| true`, `continue-on-error`).
#
# ПРЕДЕЛ, НАЗВАННЫЙ ЧЕСТНО. Барьер проверяет ПРОВОДКУ (шаг есть и красит джоб), а не то,
# что оракулы содержательны: это работа мутационного контроля при их заведении. Атрибут,
# собранный макросом или `cfg_attr`, барьер не видит — форма в дереве одна (`#[cfg(feature`
# и `#![cfg(feature`), её он и считает.
#
# ПЕРЕМЕННЫЕ (для пробы `scripts/tests/red_feature_oracles.sh`): FO_ROOT — корень дерева,
# FO_CI — путь к workflow относительно корня.

set -uo pipefail
ROOT="${FO_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
CI="${FO_CI:-.github/workflows/ci.yml}"
cd "$ROOT" || { echo "FAIL  корень $ROOT недоступен"; echo "VERDICT: FAIL"; exit 1; }

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }

[ -f "$CI" ] || { fail "нет $CI — проверять нечего, это не успех"; echo "VERDICT: FAIL"; exit 1; }

# Тело джоба build-test: от строки `  build-test:` до следующего ключа джоба (2 пробела + имя).
JOB=$(awk '/^  build-test:[[:space:]]*$/{f=1; print; next} f && /^  [A-Za-z0-9_-]+:[[:space:]]*$/{exit} f{print}' "$CI")
[ -n "$JOB" ] || { fail "в $CI нет джоба build-test"; echo "VERDICT: FAIL"; exit 1; }

if grep -qE '^[[:space:]]*continue-on-error:[[:space:]]*true' <<<"$JOB"; then
  fail "build-test несёт continue-on-error: true — красный шаг не красит джоб"
fi

FOUND=0
for tdir in crates/*/tests; do
  [ -d "$tdir" ] || continue
  crate=$(basename "$(dirname "$tdir")")
  # Атрибут — строка, НАЧИНАЮЩАЯСЯ с `#[cfg(feature` / `#![cfg(feature` (литерал внутри
  # строки кода начинается иначе — `red_m87_registry.rs:196`).
  feats=$(grep -rhoE '^[[:space:]]*#!?\[cfg\(feature[[:space:]]*=[[:space:]]*"[A-Za-z0-9_-]+"' "$tdir" 2>/dev/null \
          | sed -E 's/.*"([A-Za-z0-9_-]+)"/\1/' | sort -u)
  for f in $feats; do
    FOUND=$((FOUND + 1))
    n=$(grep -rlE '^[[:space:]]*#!?\[cfg\(feature[[:space:]]*=[[:space:]]*"'"$f"'"' "$tdir" | wc -l)
    want="cargo test -p ${crate} --features ${f}"
    if grep -qE "^[[:space:]]*(-[[:space:]]+)?run:[[:space:]]*${want}[[:space:]]*$" <<<"$JOB"; then
      pass "$crate: оракулы под feature \"$f\" ($n файл(ов)) исполняет build-test — «$want»"
    else
      fail "$crate: $n файл(ов) tests/ под feature \"$f\", а build-test не несёт шага ровно «run: $want» — CI их не исполняет"
    fi
  done
done

# Наблюдать ОТСУТСТВИЕ: ноль найденных на живом дереве означает, что сломан поиск, а не что
# долга нет (на момент заведения — gateway-serve/testing, четыре файла).
if [ "$FOUND" -eq 0 ] && [ -z "${FO_ALLOW_EMPTY:-}" ]; then
  fail "не найдено ни одного крейта с оракулами под feature — поиск сломан либо дерево не то"
fi

echo
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (провалов: $FAIL)"; exit 1
