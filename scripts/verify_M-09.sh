#!/usr/bin/env bash
# M-09 Data safety net — acceptance-гейт. ≥1 проверка на задачу §Tasks.
#
# Пока impl (crates/ops) на todo!()-скелете — RED-оракулы OPS-I-* ПАДАЮТ, и VERDICT: FAIL (это
# корректно: RED-фаза). Гейт зеленеет, когда engine-dev/venue-dev реализуют по оракулам.
# Структурные проверки (паритет OPS-I-5, OPS-I-6 «метрики не в журнал», наличие крейта, CT-RFC-03)
# проходят уже сейчас.
#
# FAIL-агрегатор (gates.md §3): считаем провалы, exit 1 при FAIL>0. НЕ маскируем через `|| echo`.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

FAILED=0
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
pass() { echo "PASS  $*"; }
run() { # $1=имя $2..=команда
  local name="$1"; shift
  if "$@" >/dev/null 2>&1; then pass "${name}"; else fail "${name} (\`$*\`)"; fi
}

# ── Task 1 (CT-RFC-03, T1) — уже смержено, обязано быть GREEN ──────────────────────────
run "T1 CT-RFC-03 (SysEvent::ReconDivergence, red_rfc03)" \
  cargo test -p contracts --test red_rfc03
run "T1 схема CT-I-4 (event.schema.json == типы)" \
  cargo test -p contracts --test red_schema

# ── Task 2 (recon + budget + метрики) — RED пока impl не готов ────────────────────────
run "T2 OPS-I-1 recon (ε_test, деградированные входы)" \
  cargo test -p ops --test red_ops_recon
run "T2 OPS-I-1 recon LIVE-режим (near-book depth-aware, §8 анти-флуд, skew-толерантность)" \
  cargo test -p ops --test red_recon_live
run "T2 OPS-I-1 recon ОКОННАЯ персистентность (churn→тишина, порча→алерт; второй §8-провал, §4.3)" \
  cargo test -p ops --test red_recon_window
run "T2 OPS-I-9 rate-budget (анти-hot-loop, TD-013)" \
  cargo test -p ops --test red_ops_budget
run "T2 OPS-I-4/7/8 метрики+тишина" \
  cargo test -p ops --test red_ops_metrics

# ── OPS-I-6 (структурно): метрики НЕ пишутся в журнал — crates/ops не зависит от journal ─
# (в рантайме; journal — ТОЛЬКО dev-dependency для restore-drill task 3). Метрики — не события
# домена (журнал детерминирован). Проверяем: в [dependencies] ops нет journal.
if awk '/^\[dependencies\]/{d=1;next} /^\[/{d=0} d' crates/ops/Cargo.toml | grep -qE '^journal\b'; then
  fail "OPS-I-6 crates/ops зависит от journal в [dependencies] — метрики могут утечь в журнал \
(журнал детерминирован; RSS/wall-clock — не события домена)"
else
  pass "OPS-I-6 метрики не в журнал (ops не зависит от journal в рантайме)"
fi

# ── OPS-I-5 (двусторонний паритет): код (METRICS) ↔ FA §3 ↔ FA §7.1 ────────────────────
# Канон имён метрик — `METRICS` (name: "…") в коде. FA §3 и §7.1 согласованы с ним В ОБЕ СТОРОНЫ.
FA="docs/fa/ops.md"
names_code=$(grep -oE 'name: "[a-z_]+"' crates/ops/src/metrics.rs | sed -E 's/name: "//; s/"//' | sort -u)
names_fa3=$(sed -n '/## §3/,/## §4/p' "${FA}" | grep '^| `' | grep -oE '`[a-z_]+(\{[^}]*\})?`' | sed -E 's/`//g; s/\{.*//' | sort -u)
names_fa71=$(sed -n '/### §7.1/,/Правило паритета/p' "${FA}" | grep '^| `' | grep -oE '`[a-z_]+(\{[^}]*\})?`' | sed -E 's/`//g; s/\{.*//' | sort -u)

# (а) каждое имя §7.1 существует в §3 И в коде (правило без метрики невозможно).
miss71=$(comm -23 <(echo "${names_fa71}") <(echo "${names_code}") | grep -v '^$' || true)
[ -z "${miss71}" ] && pass "OPS-I-5 §7.1→код: каждое правило ссылается на существующую метрику" \
  || fail "OPS-I-5 §7.1 ссылается на метрику(и) вне METRICS: ${miss71//$'\n'/ }"

# (б) каждая метрика кода объявлена в §3 (метрика без места в §3 = вне паритета).
miss3=$(comm -23 <(echo "${names_code}") <(echo "${names_fa3}") | grep -v '^$' || true)
[ -z "${miss3}" ] && pass "OPS-I-5 код→§3: каждая METRICS объявлена в §3" \
  || fail "OPS-I-5 метрика(и) кода вне §3: ${miss3//$'\n'/ }"

# (в, C-009 M1) КАЖДЫЙ КАНОНИЧЕСКИЙ КЛАСС ИНЦИДЕНТА обязан иметь строку §7.1 (иначе целый класс
# выпадает из паритета — ровно C-007 C1). Удаление, напр., строки `C1-M08` (порча книги, P0)
# обязано ВАЛИТЬ гейт. Список — канон P0/P1-классов ops.md §7; расширяется вместе с §7.1.
REQUIRED_INCIDENTS="TD-011 TD-013 TD-014 TD-016 C1-M08 TD-006 OPS-BKP OPS-SILENCE OPS-RESYNC OPS-GAP"
rows71=$(sed -n '/### §7.1/,/Правило паритета/p' "${FA}" | grep -oE '^\| `[A-Za-z0-9-]+`' | tr -d '`|' | tr -d ' ' | sort -u)
miss_inc=""
for id in ${REQUIRED_INCIDENTS}; do
  echo "${rows71}" | grep -qx "${id}" || miss_inc="${miss_inc} ${id}"
done
[ -z "${miss_inc}" ] && pass "OPS-I-5 §7.1 покрывает все канонические классы инцидентов (класс без правила невозможен)" \
  || fail "OPS-I-5 из §7.1 пропали ОБЯЗАТЕЛЬНЫЕ классы инцидентов:${miss_inc} — целый класс порчи вне алертов (регрессия C-007 C1)"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "(RED-оракулы OPS-I-* падают, пока crates/ops на todo!()-скелете — это корректная RED-фаза;"
  echo " гейт зеленеет после impl engine-dev/venue-dev.)"
  exit 1
fi
echo "VERDICT: PASS"
