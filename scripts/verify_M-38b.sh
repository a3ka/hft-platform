#!/usr/bin/env bash
# Acceptance-гейт M-38b — checkpoint-reducer + live-seek (TD-044, GW-I-9/GW-I-11).
# Прод-замер до фикса: первый Snapshot 409.74 s, >21 GiB прочитано, на КАЖДОЕ подключение.
# §8 eyes-on (замер первого Snapshot на проде ПОСЛЕ прогрева чекпоинта) — reviewer, вне гейта.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }

step "task #0 — fmt + build --workspace + clippy --all-targets"
chk cargo fmt --all -- --check
chk cargo build --workspace --quiet
chk cargo clippy --all-targets --workspace --quiet -- -D warnings

# M-38b (инцидент 2026-07-28): в обход красного clippy в workspace-Cargo.toml было добавлено
# `unused_must_use = "allow"` — ГЛОБАЛЬНО, включая прод-код. Этот линт ловит проигнорированный
# `Result`; в journal-first fail-closed проекте его отключение прячет «запись не удалась, но мы
# поехали дальше». Заявленная причина (стилистический `manual_is_multiple_of` в тестах) к нему
# отношения не имела. Правильный путь — починить КОД (сделано: 4 теста → `is_multiple_of`),
# а не глушить линт. Канарейка держит это свойство: blanket-allow не возвращается тихо.
step "канарейка — линты не отключены глушением (unused_must_use и др. blanket-allow)"
chk bash -c "! grep -rnE '^\s*\"?(unused_must_use|unused_results|manual_is_multiple_of|manual_unwrap_or)\"?\s*=\s*(\"allow\"|\{[^}]*allow)' Cargo.toml crates/*/Cargo.toml"
# Прямая проверка: с ЯВНО возвращёнными линтами clippy обязан оставаться зелёным.
chk cargo clippy --all-targets --workspace --quiet -- -D warnings -D unused_must_use -D clippy::manual_is_multiple_of -D clippy::manual_unwrap_or

step "task #1 — OrderBook переживает serde-roundtrip целиком (чейн + stale, §Findings)"
chk cargo test -p book --test red_orderbook_serde_roundtrip --quiet

step "task #2-4 — GW-I-9(а): байт-идентичность на ВСЕХ K + форсинг «чекпоинт реально читается»"
chk cargo test -p gateway --test red_checkpoint_byte_identity --quiet

step "task #3-4 — GW-I-9(б,в): кэш-не-истина (инвалидация → тихий rebuild) + идемпотентность"
chk cargo test -p gateway --test red_checkpoint_is_cache --quiet

step "task #4 — GW-I-11: снапшот из чекпоинта у хвоста не декодирует историю (прод-масштаб)"
chk cargo test -p gateway --test red_checkpoint_resource_bound --quiet

step "task #5 — journal::stream_from: полнота хвоста + сегментный пропуск + РЕАЛЬНЫЙ legacy-сегмент"
chk cargo test -p journal --test red_stream_from --quiet

# rev2 (C-030 R3): третий форсинг — покрытый префикс физически удалён, скрытый полный реплей
# невозможен. Первые два форсинга наблюдают самоотчёт реализации; этот наблюдает физику диска.
step "task #4 rev2 — байт-идентичность после prune ПОКРЫТОГО префикса + суффикс-lineage"
chk cargo test -p gateway --test red_checkpoint_prefix_pruned --quiet

# rev2 (C-030 R1): строгая связка prune ↔ покрытие чекпоинтом; offload при этом НЕ блокируется
# (иначе строгость останавливает R1 — offsite-бэкап, экзистенциальный риск docs/08).
step "task #5b — retention: prune только при доказанном покрытии, иначе skip-репорт"
chk cargo test -p journal --test red_retention_checkpoint_coverage --quiet

step "task #6 — резюмируемый live-путь: кадры идентичны frames_since, докорм ограничен"
chk cargo test -p gateway --test red_frames_seek_bound --quiet

# ── Анти-байпас канарейки (класс TD-019/TD-020 «механизм есть, никто не зовёт») ──
# Все — по КОДУ с вырезанными комментариями, не по документации.

# VB-I-3 read-only, расширенный на бинарь чекпоинтера: gateway (включая src/bin) НЕ импортирует
# писательский API журнала. Чекпоинтер читает журнал и пишет ТОЛЬКО в свой ckpt-каталог; единственный
# писатель журнала — recorder (JR-I-1).
step "канарейка — gateway (вкл. bin) не использует journal-writer API"
chk bash -c "! sed 's://.*::' crates/gateway/src/lib.rs crates/gateway/src/bin/*.rs \
  | grep -nE 'Journal::open|\.append\(|journal::Journal'"

# Том журнала у ops-сервиса чекпоинтера обязан быть READ-ONLY: гарантия JR-I-1 на уровне
# развёртывания, а не только кода (тот же приём, что у journal-retention).
step "канарейка — ops-сервис gateway-checkpoint монтирует журнал :ro"
chk bash -c "grep -A 30 '^  gateway-checkpoint:' docker-compose.yml | grep -qE 'journal-data:/journal:ro'"

# Бинарь объявлен И вызывается: «библиотека без вызывателя» — ровно TD-020.
step "канарейка — бинарь gateway-checkpoint существует и заведён в compose"
chk test -f crates/gateway/src/bin/gateway-checkpoint.rs
chk grep -q 'gateway-checkpoint' docker-compose.yml

# rev4 (reviewer B1/B2): канарейки выше — `test -f` и grep — были ЗЕЛЁНЫМИ при бинаре, который
# не стартует ни при какой форме argv. Единственная честная проверка ops-бинаря — ЗАПУСТИТЬ его
# теми аргументами, что стоят в docker-compose.yml, и посмотреть на выход и на артефакт.
step "task #13/#14 — прод-argv из docker-compose.yml + covered_through_seq = достигнутый курсор"
chk cargo test -p gateway --test red_checkpoint_bin_prod_argv --quiet

# rev4 (reviewer B3): Objective недостижим, пока snapshot-при-подключении не читает чекпоинт.
step "task #15 — gateway-serve потребляет чекпоинт (проводка env + поведение, не grep)"
chk cargo test -p gateway-serve --test red_serve_consumes_checkpoint --quiet
chk bash -c "sed 's://.*::' crates/gateway-serve/src/lib.rs | grep -q 'snapshot_from_checkpoint'"
chk bash -c "sed -n '/^  gateway-serve:/,/^  [a-z-]*:\$/p' docker-compose.yml | grep -qE 'gateway-ckpt:/ckpt:ro'"
chk bash -c "sed -n '/^  gateway-serve:/,/^  [a-z-]*:\$/p' docker-compose.yml | grep -q 'GATEWAY_CHECKPOINT_DIR'"

# Версия формата чекпоинта объявлена отдельно от GATEWAY_SCHEMA_VERSION: чекпоинт — внутренний
# кэш (T3), его форма эволюционирует независимо от контракта провода.
# rev2 (C-030 R1): артефакт покрытия обязан ПУБЛИКОВАТЬСЯ чекпоинтером и ПОТРЕБЛЯТЬСЯ retention'ом.
# «Объявлено ⟹ вызвано» — иначе гейт prune существует в коде и не работает в проде (класс TD-020).
step "канарейка — covered_through_seq публикуется чекпоинтером и читается retention-бинарём"
chk bash -c "sed 's://.*::' crates/gateway/src/bin/gateway-checkpoint.rs | grep -q 'covered_through_seq'"
chk bash -c "sed 's://.*::' crates/journal/src/bin/journal-retention.rs | grep -q 'checkpoint-coverage'"

# rev2 NOTE (critic C-031): escape-hatch prune обязан оставаться ВЫКЛЮЧЕННЫМ в проде.
# Канарейка поведенческая, а не «флаг существует»: проверяем, что ops-сервис ретеншена в
# docker-compose НЕ передаёт --allow-prune-without-checkpoint. Иначе fail-closed дефолт
# формально есть в коде, а прод молча работает с открытым футганом (класс TD-020 наоборот).
step "канарейка — прод НЕ включает allow-prune-without-checkpoint, но флаг объявлен явно"
chk bash -c "! grep -q 'allow-prune-without-checkpoint' <(sed -n '/^  journal-retention:/,/^  [a-z]/p' docker-compose.yml)"
chk bash -c "sed 's://.*::' crates/journal/src/bin/journal-retention.rs | grep -q 'allow-prune-without-checkpoint'"

# C-033 R1: здесь стоял жёсткий пин `GATEWAY_SCHEMA_VERSION: u32 = 7;`. Его НАМЕРЕНИЕ было
# milestone-областным — «M-38b не трогает контракт провода» — и оно своё отработало. Но пин
# пережил свой milestone и стал блокировать законный bump 7→8 в M-48: dev не смог бы поднять
# версию, не правя чужой verify-скрипт. Это ТРЕТЬЕ повторение одного класса (оракул/канарейка,
# прибитые к конкретной версии, превращаются в блокер контракта).
# Оставлен ДОЛГОВЕЧНЫЙ инвариант: версия формата чекпоинта объявлена ОТДЕЛЬНО от версии провода
# (чекпоинт — внутренний кэш T3, эволюционирует независимо). Пиннинг самой версии провода —
# зона версионно-агностичного оракула `red_gateway_schema_version` (шаг ниже).
step "канарейка — ckpt_schema_version объявлен ОТДЕЛЬНО от GATEWAY_SCHEMA_VERSION"
chk bash -c "sed 's://.*::' crates/gateway/src/*.rs | grep -qE 'ckpt_schema_version|CKPT_SCHEMA_VERSION'"
chk bash -c "sed 's://.*::' crates/gateway/src/lib.rs | grep -q 'GATEWAY_SCHEMA_VERSION'"

step "регрессия — форма провода и session-семантика M-38a не сдвинуты"
chk cargo test -p gateway --test red_gateway_schema_version --quiet
chk cargo test -p gateway --test red_gateway_cvd_session --quiet
chk cargo test -p gateway --test red_gateway_window --quiet
chk cargo test -p gateway --test red_gateway_live_eq_replay --quiet

step "регрессия — read-only гарантии gateway + весь затронутый периметр"
chk cargo test -p gateway --test red_gateway_readonly --quiet
chk cargo test -p gateway --quiet
chk cargo test -p journal --quiet
chk cargo test -p book --quiet
chk cargo test -p gateway-serve --quiet

echo "---"
# Вне гейта (§8 eyes-on, reviewer, ОБЯЗАТЕЛЕН): на VPS — ops-сервис gateway-checkpoint отработал
# (файл чекпоинта существует и свежий), E2E JWT→Snapshot строится за СЕКУНДЫ (не 409.74 s),
# recorder жив, heartbeat свежий. DECODE, не grep: «код на main ≠ функция в проде» (TD-020).
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAIL проверок)"
  exit 1
fi
