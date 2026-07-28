#!/usr/bin/env bash
# Acceptance-гейт M-47 — GW-I-10: fail-closed гвард выравнивания timeframe_ms (TD-046).
# Невыравненный на сутки timeframe делает session-anchored серии (CVD/SVP) неопределёнными:
# бакет накрывает 00:00 UTC → сделки ДВУХ сессий в ОДНОМ bucket_time_s. Отказ, не «правдоподобное»
# значение. Гвард в crates/gateway (модель владеет предусловием, нет байпаса) + старт gateway-serve.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }

step "task #0 — fmt + build --workspace + clippy --all-targets"
chk cargo fmt --all -- --check
chk cargo build --workspace --quiet
chk cargo clippy --all-targets --workspace --quiet -- -D warnings

step "task #1 — GW-I-10 в библиотеке: отказ на snapshot/frames_since/replay + границы + парный vantage"
chk cargo test -p gateway --test red_timeframe_session_alignment --quiet

step "task #2 — GW-I-10 на СТАРТЕ gateway-serve (конфиг, делающий сервис нерабочим, не даёт стартовать)"
chk cargo test -p gateway-serve --test red_timeframe_guard_startup --quiet

# Анти-байпас (класс TD-019/TD-020 «механизм есть, никто не зовёт»): гвард обязан быть в
# библиотеке gateway, а не ТОЛЬКО в конфиге транспорта — иначе чекпоинтер (M-38b), shared-tailer
# (M-39) и research-cli соберут Selector напрямую и пройдут мимо проверки. Канарейка по КОДУ
# (комментарии вырезаны sed), а не по документации: имя `validate_selector` зафиксировано
# milestone'ом именно ради этой проверки. Ожидание ≥4 упоминаний = определение + 3 входа
# (snapshot / frames_since / replay). Порог ловит «реализовал, но подключил не везде» —
# ровно то, чем был TD-020. НЕ ослаблять до `-q`: тогда одного определения хватит для PASS.
step "канарейка — гвард в crates/gateway/src подключён на ВСЕХ входах (≥4 упоминания)"
chk bash -c "[ \"\$(sed 's://.*::' crates/gateway/src/lib.rs | grep -c 'validate_selector')\" -ge 4 ]"

# Прод-дефолт обязан остаться рабочим: 86_400_000 % 1000 == 0. Если гвард сломает дефолт,
# прод не поднимется — проверяем явно, а не «по смыслу» (docker-compose.yml:122).
step "канарейка — прод-дефолт GATEWAY_TIMEFRAME_MS=1000 делит сутки нацело"
chk bash -c "grep -qE 'GATEWAY_TIMEFRAME_MS[:=][[:space:]]*.?1000' docker-compose.yml"

step "регрессия — session-семантика M-38a (CVD ledger/окно/схема v7) не сдвинута гвардом"
chk cargo test -p gateway --test red_gateway_cvd_session --quiet
chk cargo test -p gateway --test red_gateway_window --quiet
chk cargo test -p gateway --test red_gateway_schema_v7 --quiet

step "регрессия — весь read-path suite"
chk cargo test -p gateway --quiet
chk cargo test -p gateway-serve --quiet

echo "---"
# Вне гейта (§8 eyes-on, reviewer): прод стартует с дефолтным GATEWAY_TIMEFRAME_MS=1000,
# контейнер healthy, E2E JWT→Snapshot строится. Пруф в close-out.
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAIL проверок)"
  exit 1
fi
