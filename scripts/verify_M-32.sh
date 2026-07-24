#!/usr/bin/env bash
# verify_M-32.sh — acceptance-гейт M-32 (верификация достоверности глубины стакана).
# Агрегатор с FAIL-счётчиком (не set -e: хотим ВСЕ провалы разом). CI-точно (RN-17/TD-035).
set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
ok()   { echo "PASS: $1"; }
bad()  { echo "FAIL: $1"; FAIL=$((FAIL+1)); }

# ── Гейт 0: формат + линт CI-точно (TD-035 pin 1.97.0) ──────────────────────────────────────────
cargo fmt --all -- --check >/dev/null 2>&1 && ok "fmt clean" || bad "cargo fmt --all -- --check"
cargo clippy -p research-cli --all-targets --all-features -- -D warnings >/dev/null 2>&1 \
  && ok "clippy research-cli 0 warnings" || bad "clippy research-cli -D warnings"

# ── Задача 2a/2b: DV-I-1..5 lifetime/staleness GREEN ────────────────────────────────────────────
cargo test -p research-cli --test red_depth_lifetime >/dev/null 2>&1 \
  && ok "DV-I-1..5 (red_depth_lifetime) GREEN" || bad "DV-I-1..5 red_depth_lifetime"

# ── Задача 3a/3b: DV-I-6 order-flow faithfulness GREEN ──────────────────────────────────────────
cargo test -p research-cli --test red_orderflow_faith >/dev/null 2>&1 \
  && ok "DV-I-6 (red_orderflow_faith) GREEN" || bad "DV-I-6 red_orderflow_faith"

# ── Задача 1 (Q1): depth-source survey memo существует + отвечает CONFIRMED/REFUTED ──────────────
Q1=research/data-quality/depth-sources-survey.md
if [ -f "$Q1" ]; then
  grep -qiE 'CONFIRMED|REFUTED' "$Q1" && ok "Q1 memo: паритет CONFIRMED/REFUTED" \
    || bad "Q1 memo не выносит CONFIRMED/REFUTED"
else
  bad "Q1 memo отсутствует ($Q1)"
fi

# ── Задача 5 (вердикт): depth-verdict.md называет 3 founder-решения ──────────────────────────────
V=research/data-quality/depth-verdict.md
if [ -f "$V" ]; then
  grep -qiE 'эталон|1\.3%' "$V" && grep -qiE 'достоверн|staleness|order-flow' "$V" \
    && grep -qiE 'provenance|diff-reconstruct|VB-I-5' "$V" \
    && ok "вердикт называет 3 решения (эталон / достоверность / provenance)" \
    || bad "вердикт не покрывает 3 founder-решения"
else
  bad "вердикт отсутствует ($V)"
fi

echo "---"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL)"; exit 1; fi
