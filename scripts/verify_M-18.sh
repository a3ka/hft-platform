#!/usr/bin/env bash
# M-18 CT-RFC-04 L2Delta — персист сырых book-дельт. Acceptance-гейт.
#
# RED-фаза: venue-capture/journal-persist/workspace-build ПАДАЮТ, пока venue-dev не добавил
# `l2delta_event` + emit, а engine-dev — арм `MdPayload::L2Delta` в journal/sim (E0004). Зеленеет
# после impl. Контрактный слой (red_rfc04/red_schema) — GREEN сразу (T1 via RFC — зона architect).
#
# §8 live-emit (L2Delta реально пишется с боевого WS) — ОТДЕЛЬНЫЙ гейт reviewer'а на VPS, здесь
# не проверяется (юнит ≠ live, урок TD-014). FAIL-агрегатор (gates.md §3): exit 1 при FAIL>0.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

FAILED=0
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
pass() { echo "PASS  $*"; }
run() { local name="$1"; shift; if "$@" >/dev/null 2>&1; then pass "${name}"; else fail "${name} (\`$*\`)"; fi; }

# ── Задача 1 (architect): T1-форма + RFC-пакет (контрактный слой) ───────────────────────
run "CT-RFC-04 red_rfc04 (роундтрип, дискриминанты 0..6, историч. блоб CT-I-3, losslessness, pu, фикстуры)" \
  cargo test -p contracts --test red_rfc04
run "CT-I-4 red_schema (event.schema.json СГЕНЕРИРОВАНА из типов и согласована)" \
  cargo test -p contracts --test red_schema

# ── Задача 1 структурно: вариант, магия/версия, CHANGELOG, фикстуры ─────────────────────
if grep -qE "^\s*L2Delta\s*\{" crates/contracts/src/lib.rs; then
  pass "MdPayload::L2Delta присутствует в contracts"
else
  fail "MdPayload::L2Delta отсутствует в crates/contracts/src/lib.rs"
fi
# SEGMENT_MAGIC и SCHEMA_VERSION НЕ ДОЛЖНЫ меняться (§3): смена сломала бы чтение боевых сегментов.
if grep -qE 'SEGMENT_MAGIC.*b"HFTJRN02"' crates/contracts/src/lib.rs \
   && grep -qE "SCHEMA_VERSION: u32 = 2;" crates/contracts/src/lib.rs; then
  pass "SEGMENT_MAGIC=HFTJRN02 и SCHEMA_VERSION=2 НЕ тронуты (аддитивность §3)"
else
  fail "SEGMENT_MAGIC/SCHEMA_VERSION изменены — L2Delta аддитивен, bump запрещён (сломает прод-чтение)"
fi
if grep -q "CT-RFC-04" crates/contracts/CHANGELOG.md; then
  pass "CHANGELOG несёт запись CT-RFC-04"
else
  fail "crates/contracts/CHANGELOG.md без записи CT-RFC-04 (05 §4: пакет неполон)"
fi
for fx in fixtures/valid/event-l2delta-spot.json fixtures/valid/event-l2delta-futures.json \
          fixtures/invalid/event-l2delta-missing-final-id.json; do
  if [ -f "crates/contracts/${fx}" ]; then pass "фикстура ${fx}"; else fail "нет фикстуры crates/contracts/${fx}"; fi
done

# ── Задача 3 (venue-dev): СПОТ капча сырого diff как L2Delta (без потерь) ────────────────
run "venue-binance red_l2delta_capture (U/u/E + size==0 + асимметрия сохранены; prev_final=None)" \
  cargo test -p venue-binance --test red_l2delta_capture

# ── Задача 4 (venue-dev): ФЬЮЧЕРС капча с prev_final=Some(pu) ────────────────────────────
run "venue-binance-futures red_l2delta_futures (pu → prev_final_update_id; continuity перпа)" \
  cargo test -p venue-binance-futures --test red_l2delta_futures

# ── Задача 3/4 wiring-канарейка: l2delta_event ВЫЗВАН, а не только ОПРЕДЕЛЁН (C-017 blocker 2) ─
# Голое `grep l2delta_event` проходит на ОДНОМ определении helper'а (helper-only green). Считаем
# CALL-сайты: строки с `l2delta_event(` МИНУС строка определения (`fn l2delta_event`) и комментарии.
# ≥1 call на venue ⇒ функция подключена к emit-пути, а не мертва. §8 на VPS — окончательный
# live-emit гейт (unit/структура ≠ live-доставка с боевого WS, урок TD-014).
for v in venue-binance venue-binance-futures; do
  f="crates/${v}/src/lib.rs"
  calls=$(grep -nE "l2delta_event[[:space:]]*\(" "$f" 2>/dev/null | grep -vE "fn[[:space:]]+l2delta_event" | grep -vcE "^[0-9]+:[[:space:]]*//")
  if [ "${calls:-0}" -ge 1 ]; then
    pass "${v}: l2delta_event ВЫЗВАН в emit-пути (${calls} call-site, не только определение)"
  else
    fail "${v}: l2delta_event только ОПРЕДЕЛЁН, не вызван — капча не подключена к WS (helper-only green, C-017 b2)"
  fi
done

# ── Задача 5 (engine-dev): консюмер-армы + sacred write-path персистит L2Delta ───────────
run "journal red_l2delta_persist (L2Delta переживает write→read_all exact; spot+futures)" \
  cargo test -p journal --test red_l2delta_persist
# ── Rollback-safety (C-018 risk-critic): L2Delta изолирован в M-18-provenance сегменте ──
run "journal red_l2delta_rollback_boundary (L2Delta в НОВОМ сегменте; pre-M18 чист → quarantine; нет seq-reuse)" \
  cargo test -p journal --test red_l2delta_rollback_boundary
# ── Структурно: runbook отката задокументирован (C-018 mitigation) ──────────────────────
if grep -q "L2Delta" docs/rfc/CT-RFC-04-l2delta.md && grep -qE "§5\.1|Runbook: откат" docs/fa/ops.md; then
  pass "rollback-runbook задокументирован (RFC §10 + ops.md §5.1) — C-018 mitigation"
else
  fail "нет rollback-runbook (RFC §10 / ops.md §5.1) — C-018 blocking concern не закрыт"
fi
# Полный workspace собирается ⇒ ВСЕ исчерпывающие match MdPayload получили арм L2Delta (E0004 закрыты).
run "workspace собирается со всеми armами L2Delta (E0004 закрыты: journal/sim + любой оставшийся)" \
  cargo build --workspace --all-targets

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "(venue-capture/journal-persist/workspace-build падают, пока venue-dev не добавил l2delta_event+emit"
  echo " и engine-dev — арм MdPayload::L2Delta в journal segment_last_ts + sim exchange — корректная RED-фаза.)"
  exit 1
fi
echo "VERDICT: PASS"
