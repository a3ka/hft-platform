#!/usr/bin/env bash
# verify_M-84.sh — acceptance-гейт M-84 (фиксированные семь полос глубины).
#
# Дом решения — `П-029` (граница C, founder 2026-09-10); перепроверка записи `R-179`,
# четыре круга, APPROVE. Спека — `milestones/M-84-fixed-depth-bands.md`.
#
# ЧТО ЭТОТ ГЕЙТ МОЖЕТ И ЧЕГО НЕ МОЖЕТ — названо, а не подразумевается. Он судит КОД и
# ТЕКСТЫ: существование канонического набора, место гварда, явность отказа, поведение
# отпечатка. Он НЕ судит: живую выдачу на сокете, вес кадра на проводе и порядок выкатки
# на реальном чекпоинте — это предъявляется прогоном на VPS в Done Block'е (`gates.md` §8).
# Остаточный риск объявлен здесь, а не выдан за покрытие.
#
# ЗАДАЧА 6 (включение на проде) ЭТИМ ГЕЙТОМ НЕ ПРОВЕРЯЕТСЯ И НЕ ДОЛЖНА: она заблокирована
# двумя предусловиями (`TD-159` и барьер эмиссии), и зелёный гейт не является разрешением
# на включение. Шаг 9 ниже проверяет ОБРАТНОЕ — что включение НЕ состоялось.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 2
FAIL=0
step() { printf '\n── %s\n' "$*"; }
# Подоболочка обязательна: проверка с `exit` иначе убивает гейт с нулевым кодом — класс,
# пойманный на `verify_M-73` собственным прогоном (ложно-зелёный гейт).
chk() {
  local name; name="$(printf '%s' "$1" | sed -n '1{s/^[[:space:]]*//;s/[[:space:]]*$//;p}')"
  [ -n "$name" ] || name="<многострочная проверка>"
  if ( eval "$1" ) >/dev/null 2>&1; then echo "PASS: ${name}"; else echo "FAIL: ${name}" >&2; FAIL=$((FAIL + 1)); fi
}

step "задача 1 — паритет с CI (gates.md §3: гейт, который зеленее CI, не гейт)"
chk "cargo fmt --all -- --check"
chk "cargo clippy --all-targets --all-features -- -D warnings"
chk "cargo test --all"

step "задача 2 — канонический набор существует и РАВЕН продуктовому"
chk "cargo test -p gateway --test red_fixed_bands_canonical"
# Набор объявлен в docs/fa/viz-backend.md:42 и подписан П-014. Гейт сверяет КОД с ДОКУМЕНТОМ,
# а не код с кодом: иначе обе стороны уедут вместе и расхождения никто не заметит.
chk "grep -q '1\.5/3/5/8/15/30/60' docs/fa/viz-backend.md"

step "задача 2 — гвард живёт в КРЕЙТЕ, а не только в транспорте"
# Довод — установленный, не новый: Selector собирают напрямую research-cli, чекпоинтер, M-39.
# Гвард только в транспорте оставил бы байпас-поверхность (crates/gateway/src/lib.rs:2743-2750,
# так обосновано место GW-I-14). Проверяем ИМЕННО библиотечный валидатор.
chk "grep -q 'CANONICAL_DEPTH_BANDS' crates/gateway/src/lib.rs"
chk "cargo test -p gateway --test red_fixed_bands_canonical crate_rejects_foreign_band_set"

step "задача 3 — транспорт: отказ ЯВНЫЙ, а не молчаливая подмена"
chk "cargo test -p gateway-serve --test red_fixed_bands_wire"
# Самая вероятная «зелёная» реализация — молча заменить присланный набор каноническим.
# Она удовлетворяет и UX, и экономике, и не ловится ни одним тестом про отпечаток.
chk "cargo test -p gateway-serve --test red_fixed_bands_wire foreign_bands_rejected_not_swapped"

step "задача 3 — отпечаток: ось полос перестала различать, ПРОЧИЕ различают"
chk "cargo test -p gateway --test red_fixed_bands_fingerprint"
# Парный vantage: без второй половины реализация, сделавшая отпечаток константой, зелена.
chk "cargo test -p gateway --test red_fixed_bands_fingerprint other_axes_still_split"

step "задача 4 — проводной контракт и описание модуля приведены к решению"
chk "grep -q 'П-029' docs/rfc/CT-RFC-09-ws-session.md"
chk "bash scripts/verify_design_claims.sh"

step "задача 5 — порядок выкатки объявлен и даёт НУЛЕВОЕ окно"
# Замер 2026-09-10: холодная пересборка 965 с при интервале чекпоинтера 900 с. Наивная
# выкатка даёт окно, которое не закрывается само. Развязка: имя слепка есть отпечаток, значит
# слепки разных наборов СОСУЩЕСТВУЮТ — новый собирается заранее, переключение идёт на готовый.
chk "grep -qi 'заранее' deploy/README.md"
chk "grep -q 'ckpt-' deploy/README.md"

step "граница C — включение НЕ состоялось этим милестоуном (задача 6 заблокирована)"
# Зелёный гейт НЕ является разрешением на включение. Пока предусловия открыты, прод обязан
# считать одну полосу; смена GATEWAY_BANDS здесь — нарушение границы C, а не прогресс.
chk "grep -q 'GATEWAY_BANDS:-0.001' docker-compose.yml"
chk "grep -qE 'TD-159' milestones/M-84-fixed-depth-bands.md"

printf '\n'
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL ($FAIL)"
exit 1
