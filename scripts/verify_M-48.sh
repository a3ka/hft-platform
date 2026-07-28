#!/usr/bin/env bash
# Acceptance-гейт M-48 — бутстрап чекпоинта на усечённом журнале + доставка ops-цепочки (TD-048).
# Контекст: M-38b смержен со всеми зелёными гейтами и ИНЕРТЕН в проде (382.657 s, чекпоинт не
# поднимается никогда). Поэтому гейт проверяет ВЫЗЫВАТЕЛЕЙ, а не наличие вызываемого.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }

step "task #0 — fmt + build --workspace + clippy --all-targets"
chk cargo fmt --all -- --check
chk cargo build --workspace --quiet
chk cargo clippy --all-targets --workspace --quiet -- -D warnings

step "task #1-#3 — GW-I-12: бутстрап на усечённом журнале + декларация + сужённый fail-loud"
chk cargo test -p gateway --test red_checkpoint_bootstrap_truncated --quiet

step "task #4 — bump схемы 7→8 и проброс полей на провод"
chk bash -c "grep -qE 'GATEWAY_SCHEMA_VERSION: u32 = 8;' crates/gateway/src/lib.rs"
chk bash -c "sed 's://.*::' crates/gateway/src/lib.rs | grep -q 'history_truncated'"

# ── ОПЕРАТОРСКИЙ ПУТЬ: канарейка проверяет ВЫЗЫВАТЕЛЯ ──────────────────────────
# Урок TD-048: задача #5c M-38b была закрыта канарейкой «бинарь существует + заведён в
# compose» — grep-green артефакт при отсутствующем cron-вызывателе. Проверять надо того, КТО
# зовёт, и С КАКИМИ аргументами (класс TD-019/TD-020).

step "task #5 — cron-обёртка чекпоинтера существует, исполняема и заведена в cron.d"
chk test -x deploy/bin/gateway-checkpoint-cron.sh
chk bash -c "grep -rq 'gateway-checkpoint-cron.sh' deploy/cron.d/"

step "task #6 — retention-обёртка ПЕРЕДАЁТ --checkpoint-coverage (без него retention — no-op)"
chk bash -c "sed 's:#.*::' deploy/bin/journal-retention-cron.sh | grep -q -- '--checkpoint-coverage'"

# Порядок в cron существен: чекпоинт обязан отработать ДО retention, иначе покрытие устаревшее
# и prune идёт по вчерашнему рубежу. Парсить расписание cron надёжно — дороже пользы, поэтому
# канарейка проверяет ТОЛЬКО присутствие обеих записей и ЯВНО отдаёт порядок на §8 eyes-on.
# Слабая, но честная проверка лучше запутанной, которая выглядит сильной (урок TD-048).
step "канарейка — обе cron-записи присутствуют (порядок проверяет reviewer на §8)"
chk bash -c "grep -rq 'journal-retention-cron.sh' deploy/cron.d/"

step "регрессия — read-path M-38b не сломан bump-ом схемы и новыми полями"
chk cargo test -p gateway --test red_checkpoint_byte_identity --quiet
chk cargo test -p gateway --test red_checkpoint_is_cache --quiet
chk cargo test -p gateway --test red_checkpoint_prefix_pruned --quiet
chk cargo test -p gateway --test red_checkpoint_resource_bound --quiet
chk cargo test -p gateway --test red_checkpoint_bin_prod_argv --quiet
chk cargo test -p gateway-serve --test red_serve_consumes_checkpoint --quiet

step "регрессия — весь затронутый периметр"
chk cargo test -p gateway --quiet
chk cargo test -p gateway-serve --quiet
chk cargo test -p journal --quiet

echo "---"
# Вне гейта (§8 eyes-on, reviewer — РЕШАЮЩИЙ, ОБЯЗАТЕЛЕН):
#   1) `docker compose --profile ops run --rm gateway-checkpoint` → exit=0, чекпоинт создан;
#   2) `covered_through_seq` — реальное число, НЕ u64::MAX;
#   3) повторный E2E JWT→Snapshot < 10 s (baseline TD-044: 409.74 s, M-38b: 382.657 s);
#   4) Snapshot несёт history_truncated=true и history_start_seq=16049334 (прод-журнал
#      после purge M-36) — DECODE, не grep.
# Без (1)-(4) TD-044/TD-048 не закрываются и M-28/M-36/M-38b/M-48 не закрываются.
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAIL проверок)"
  exit 1
fi
