#!/usr/bin/env bash
# M-09 Data safety net — acceptance-гейт. ≥1 проверка на задачу §Tasks.
#
# Task 1/2 (CT-RFC-03, recon+budget+метрики) — GREEN на main (смержено). Task 4 (метрики+алерты:
# /metrics HTTP-сервер + правила P0/P1/P2) — RED, пока engine-dev не создал ops::server / ops::alerts /
# recorder::metrics_server (compile-RED, изолированные test-бинарники). VERDICT: FAIL до impl task 4 —
# это корректная RED-фаза. Структурные проверки (паритет OPS-I-5, OPS-I-6) проходят по мере готовности.
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
run "T2 OPS-I-1 recon РАНТАЙМ-КОНТРАКТ B2 (best-only+seed-gate; персистентный объём→ТИШИНА, best-порча→эмит; §4.3.2)" \
  cargo test -p ops --test red_recon_runtime
run "T2 OPS-I-1 recon SINK B2 (эмиссия ⟺ best: персистентный объём→тишина, best-десинк→Sys+метрики)" \
  cargo test -p ops --test red_recon_sink
run "T2 OPS-I-9 rate-budget (анти-hot-loop, TD-013)" \
  cargo test -p ops --test red_ops_budget
run "T2 OPS-I-4/7/8 метрики+тишина" \
  cargo test -p ops --test red_ops_metrics

# ── Task 4 (метрики+алерты): /metrics HTTP-сервер + правила P0/P1/P2 + rule-паритет ────
# RED пока engine-dev не создал ops::server / ops::alerts / recorder::metrics_server (compile-RED,
# изолированные test-бинарники — не ломают task-2 оракулы).
run "T4A OPS-I-4 /metrics HTTP-сервер ЧИСТЫЙ (GET/metrics→200+тело, 404, 405; ops::server)" \
  cargo test -p ops --test red_ops_server
run "T4A OPS-I-4 /metrics socket (recorder биндит loopback, реальный TCP GET→200+тело)" \
  cargo test -p recorder --test red_metrics_endpoint
run "T4B OPS-I-5 правила алертов + rule-паритет (правило→метрика, класс→правило, рендер)" \
  cargo test -p ops --test red_ops_alerts
run "T4C OPS-I-10 живая ЭМИССИЯ метрик (прогон writer/feeder/sampler → SAMPLE, не HELP/TYPE; TD-027)" \
  cargo test -p recorder --test red_metrics_emission

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

# (г, task 4B) КАТАЛОГ ПРАВИЛ (ops::alerts::ALERT_RULES) ↔ FA §7.1 incident-IDs В ОБЕ СТОРОНЫ.
# Rust-канон правил и FA §7.1 не смеют расходиться: правило на класс вне §7.1 = сирота; класс §7.1
# без правила в каталоге = дыра. incident-ID берём из `incident: "…"` в alerts.rs (compile-RED, пока
# engine-dev не создал файл → каталог пуст → все обязательные классы «пропали» → FAIL, корректный RED).
ALERTS_SRC="crates/ops/src/alerts.rs"
if [ -f "${ALERTS_SRC}" ]; then
  cat_ids=$(grep -oE 'incident: "[A-Za-z0-9-]+"' "${ALERTS_SRC}" | sed -E 's/incident: "//; s/"//' | sort -u)
  # обязательный класс §7.1 без правила в каталоге → дыра
  miss_rule=""
  for id in ${REQUIRED_INCIDENTS}; do
    echo "${cat_ids}" | grep -qx "${id}" || miss_rule="${miss_rule} ${id}"
  done
  [ -z "${miss_rule}" ] && pass "OPS-I-5 каталог правил покрывает все обязательные классы §7.1" \
    || fail "OPS-I-5 ALERT_RULES не покрывает классы §7.1:${miss_rule} — класс без правила (rule-side дыра)"
  # правило-сирота: incident каталога вне строк §7.1 (${rows71})
  orphan=""
  for id in ${cat_ids}; do
    echo "${rows71}" | grep -qx "${id}" || orphan="${orphan} ${id}"
  done
  [ -z "${orphan}" ] && pass "OPS-I-5 каталог правил → §7.1: нет правил-сирот (все привязаны к классу)" \
    || fail "OPS-I-5 ALERT_RULES содержит правила на классы вне §7.1:${orphan} — паритет односторонний"
else
  fail "OPS-I-5 task 4B: ${ALERTS_SRC} отсутствует — каталог правил не создан (compile-RED до engine-dev)"
fi

# (д, task 4C / OPS-I-10) EMISSION-КАНАРЕЙКА: КАЖДАЯ объявленная метрика §3 покрыта либо
# emission-оракулом (red_metrics_emission.rs — прогон продюсера, SAMPLE-ассерт), либо явно
# классифицирована event/elsewhere (эмитится по триггеру / уже wired в другом продюсере). Метрика
# вне обоих множеств = потенциальный TD-027 (объявлена, но никто не проверяет её РАНТАЙМ-эмиссию).
EMIT_TEST="crates/recorder/tests/red_metrics_emission.rs"
# event/elsewhere: эмитятся по триггеру (event) или уже wired в отдельном продюсере (elsewhere,
# со своим RED). Расширяется вместе с §3 продюсер-картой — новая метрика без покрытия ВАЛИТ гейт.
EVENT_OR_ELSEWHERE="venue_ws_reconnects_total venue_http_status_total journal_write_errors_total journal_seq_gaps_total book_divergence_bps book_resync_total backup_restore_drill_ok"
if [ -f "${EMIT_TEST}" ]; then
  uncovered=""
  for m in ${names_code}; do
    if grep -q "\"${m}\"" "${EMIT_TEST}"; then continue; fi
    echo " ${EVENT_OR_ELSEWHERE} " | grep -q " ${m} " && continue
    uncovered="${uncovered} ${m}"
  done
  [ -z "${uncovered}" ] && pass "OPS-I-10 каждая §3-метрика покрыта emission-оракулом или классифицирована event/elsewhere" \
    || fail "OPS-I-10 метрики без проверки РАНТАЙМ-эмиссии:${uncovered} — объявлена, но никто не ассертит продюсера (TD-027-риск)"
else
  fail "OPS-I-10 task 4C: ${EMIT_TEST} отсутствует — emission-оракул не создан"
fi

# (е, task 4C / OPS-I-10 LIVE-WIRING, C-014 gap-2) Продюсеры, живущие ОТДЕЛЬНО от run_writer
# (book_levels — feeder-loop; recorder_rss_anon_bytes — sampler), ОБЯЗАНЫ вызываться в ЖИВОМ main.
# «Хелпер работает в тесте, но main его не зовёт» = helper-only-non-live = рекурсия TD-027 (C-014).
# run_writer/journal/md — уже в main (recorder пишет), их liveness пиннит сам emission-тест.
MAIN="crates/recorder/src/main.rs"
if [ -f "${MAIN}" ]; then
  wiring_miss=""
  grep -q "run_books_feeder" "${MAIN}" || wiring_miss="${wiring_miss} run_books_feeder(book_levels)"
  grep -q "sample_rss" "${MAIN}" || wiring_miss="${wiring_miss} sample_rss(recorder_rss_anon_bytes)"
  [ -z "${wiring_miss}" ] && pass "OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main" \
    || fail "OPS-I-10 продюсеры НЕ вызваны в живом main:${wiring_miss} — helper-only non-live (TD-027 рекурсия, C-014 gap-2)"
else
  fail "OPS-I-10 live-wiring: ${MAIN} отсутствует"
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "(RED-оракулы OPS-I-* падают, пока crates/ops на todo!()-скелете — это корректная RED-фаза;"
  echo " гейт зеленеет после impl engine-dev/venue-dev.)"
  exit 1
fi
echo "VERDICT: PASS"
