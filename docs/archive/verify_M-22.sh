#!/usr/bin/env bash
# verify_M-22.sh — acceptance-гейт M-22 Read Gateway (архитектор-owned, sacred).
# GREEN-гейт: PASS только когда gateway реализован и все GW-I-* зелёные.
# В RED-фазе (тела `unimplemented!()`) — FAIL ОЖИДАЕМ (тесты падают паникой).
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2
FAIL=0
note() { printf '%s\n' "$*"; }
check() { # check "<name>" <exit-of-last-cmd>
  if [ "$2" -eq 0 ]; then note "PASS $1"; else note "FAIL $1"; FAIL=$((FAIL+1)); fi
}

# --- fmt-гейт (RN-8): МАТЧИТ CI (ci.yml build-test: cargo fmt --all -- --check) ---
# Без этого acceptance даёт false-green при красном CI fmt-gate (инцидент reviewer B2, M-22).
cargo fmt --all -- --check 2>&1 | tail -10
check "cargo fmt --all -- --check (matches CI fmt-gate)" "${PIPESTATUS[0]}"

# --- Task #1/#3/#4/#5: RED-набор GW-I-1..8 зелён (компилируется + проходит) ---
cargo test -p gateway --tests 2>&1 | tail -30
check "GW-I-1..8 tests (cargo test -p gateway)" "${PIPESTATUS[0]}"

# --- Task #2: clippy-гейт (весь workspace, все таргеты, без варнингов) ---
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -15
check "clippy --workspace --all-targets -D warnings" "${PIPESTATUS[0]}"

# --- C-021 NOTE-2 / GW-I-2 канарейка: НЕТ read_all / Vec<Event> в gateway/src (только stream) ---
# Комментарии вырезаем (sed 's://.*::'), чтобы doc-упоминания запрета не ложнили.
SRC_FILES=$(find crates/gateway/src -name '*.rs' 2>/dev/null)
if [ -n "$SRC_FILES" ]; then
  # Все канарейки — по КОДУ, а не по doc-комментариям (C-022 B1): вырезаем `//...` до EOL.
  STRIPPED=$(sed 's://.*::' $SRC_FILES)

  READ_ALL_HITS=$(printf '%s\n' "$STRIPPED" | grep -nE 'read_all|Vec[[:space:]]*<[[:space:]]*Event[[:space:]]*>' || true)
  [ -z "$READ_ALL_HITS" ]; check "no read_all / Vec<Event> materialization in gateway/src (NOTE-2)" $?
  [ -n "$READ_ALL_HITS" ] && note "  ↳ $READ_ALL_HITS"

  # --- GW-I-1 канарейка: НЕТ journal-writer символов в gateway/src (read-only) ---
  WRITER_HITS=$(printf '%s\n' "$STRIPPED" | grep -nE 'Journal::open|open_with|WriterConfig|\.append\(|\.flush\(' || true)
  [ -z "$WRITER_HITS" ]; check "no journal-writer symbols in gateway/src (GW-I-1 read-only)" $?
  [ -n "$WRITER_HITS" ] && note "  ↳ $WRITER_HITS"

  # --- NOTE-2 positive: прод-путь чтения — ВЫЗОВ journal::stream( в КОДЕ (не doc, не комментарий) ---
  printf '%s\n' "$STRIPPED" | grep -qE 'journal::stream[[:space:]]*\('
  check "gateway/src CALLS journal::stream( (bounded read path — code, not doc)" $?

  # --- GW-I-7 positive: EpochFilter реально прокинут в код (не молчаливое смешение эпох) ---
  printf '%s\n' "$STRIPPED" | grep -qE 'EpochFilter|filter'
  check "gateway/src threads EpochFilter (GW-I-7 no silent epoch mix)" $?
else
  note "FAIL gateway/src отсутствует"; FAIL=$((FAIL+1))
fi

# --- Read-only обратной зависимости: recorder НЕ зависит от gateway ---
! grep -qE '^gateway[[:space:]]*=' crates/recorder/Cargo.toml 2>/dev/null; check "recorder does NOT depend on gateway (no reverse dep)" $?

# --- Task #6 (опц.): reference-WS-бинарь — smoke, НЕ детерм-оракул. Не гейтит M-22-core. ---
note "NOTE task#6 (reference-WS-бинарь) — опционален; не входит в этот VERDICT."

echo "-----"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL failed)"; exit 1; fi
