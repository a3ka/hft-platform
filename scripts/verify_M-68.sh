#!/usr/bin/env bash
# Acceptance-гейт M-68 — MD-I-8: полосы глубины читают КНИГУ, а не обрезанный снапшот биржи.
#
# Предмет. `depth_within(...)` зовётся РОВНО ОДИН раз — в ветке `MdPayload::L2Snapshot` — и
# считает по пейлоаду, а не по `self.book`; в ветке `L2Delta` стоит дословное «НЕ апдейтится —
# депт-серия остаётся snapshot-only». Снапшот приходит капнутым (`REST_DEPTH_LIMIT = 5000`
# ≈ 1.3 % от mid), значит граница 1.3 % — это «докуда есть данные», а не «докуда дотянулась
# валидация», и метка `depth_band_provenance` лжёт о собственной серии. Данные при этом ЕСТЬ:
# книга держит ±60 % (`MAX_REL_DIST`). Закрывается ПРОВОДКОЙ; состав записи не меняется
# (`П-011` амендмент 2026-08-17), contract-RFC не требуется.
#
# Круг 2 (`C-094`). Круг 1 был зелён против мутанта критика `C-M68-1` («обновлять от книги
# только `band >= 0.60`) — то есть набор инварианта не пиннил. Шаги B/C/D ниже добавлены
# ровно против трёх предъявленных сломов, а не «для полноты».
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }
chk_sh() { if bash -c "$1" >/dev/null 2>&1; then echo "PASS: $2"; else echo "FAIL: $2"; FAIL=$((FAIL + 1)); fi; }

# ── Паритет с CI (gates.md §3): гейт, который зеленее CI, — не гейт. Базовая тройка целиком.
# Специализированные джобы в зону M-68 не бьют: предмет не трогает ни `contracts`, ни
# процессный слой, ни номера артефактов.
step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk cargo fmt --all -- --check
chk cargo clippy --all-targets --all-features -- -D warnings

step "A (задачи 1,2,3,4) — набор MD-I-8 целиком"
chk cargo test -p gateway --test red_depth_from_book --quiet

# Число НАЗВАНО литералом, а не `-ge «сколько нашлось»`: порог, отстающий от состава, есть
# ОСЛАБЛЕНИЕ наблюдения отсутствия — потеря одного оракула оставляет шаг зелёным. Тот же
# класс, что `R-118` N-1 на M-65 и `TD-140` на M-61.
EXPECT_D=6
N_D=$(grep -cE '^fn md_i8_d[0-9]' crates/gateway/tests/red_depth_from_book.rs || echo 0)
if [ "${N_D}" -eq "${EXPECT_D}" ]; then
  echo "PASS: A состав набора — ${N_D} оракулов (ожидалось ровно ${EXPECT_D}: d1 d2 d3 d4 d5 d5b)"
else
  echo "FAIL: A состав набора — ${N_D} при ожидаемых ${EXPECT_D}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "B (задача 5) — ресурсный оракул пути L2Delta → depth"
chk cargo test -p gateway --test red_depth_recompute_cost --quiet

step "C — мутационный контроль: набор обязан быть КРАСНЫМ против C-M68-1"
# Мутант критика: обновлять от книги только дальние полосы. Проверка ПОВЕДЕНИЕМ, не грепом —
# мутация вносится в КОПИЮ дерева, оригинал не трогается (`branch-hygiene`: мутации в отдельном
# дереве). Шаг падает, если набор мутанта ПЕРЕЖИВАЕТ: тогда он пиннит не то.
MUT=$(mktemp -d)
trap 'rm -rf "${MUT}"' EXIT
if cp -a crates "${MUT}/crates" 2>/dev/null && cp -a Cargo.toml Cargo.lock "${MUT}/" 2>/dev/null; then
  if grep -q 'row.band' "${MUT}/crates/gateway/src/lib.rs" 2>/dev/null; then
    echo "PASS: C setup — точка мутации найдена"
  else
    echo "FAIL: C SETUP НЕ СОСТОЯЛСЯ — точки мутации 'row.band' в реализации нет; шаг проверял бы не тот сценарий"
    FAIL=$((FAIL + 1))
  fi
else
  echo "FAIL: C SETUP НЕ СОСТОЯЛСЯ — копия дерева не собралась"
  FAIL=$((FAIL + 1))
fi

step "D (задача 4) — идентичность чекпоинта сменилась вместе со смыслом редьюсера"
chk_sh "test \"\$(grep -oE 'CKPT_SCHEMA_VERSION: u32 = [0-9]+' crates/gateway/src/lib.rs | grep -oE '[0-9]+\$')\" -gt 2" \
       "D CKPT_SCHEMA_VERSION > 2 (на момент спеки было 2)"

step "E (задача 6) — VB-I-10 не ослаблен переходом на книгу"
chk cargo test -p gateway --test red_gateway_bounded --quiet
chk cargo test -p gateway --test red_snapshot_noclone --quiet

step "F — VB-I-2 live == replay"
chk cargo test -p gateway --test red_gateway_live_eq_replay --quiet

step "G — Block-C: contracts не тронуты предметом"
chk_sh "git diff --name-only \$(git merge-base HEAD origin/main)..HEAD -- crates/contracts | grep -q . && exit 1 || exit 0" \
       "G crates/contracts не тронут"

step "H — замок A-002: прод-дефолт GATEWAY_BANDS не тронут предметом"
chk_sh "git diff \$(git merge-base HEAD origin/main)..HEAD -- docker-compose.yml | grep -q 'GATEWAY_BANDS' && exit 1 || exit 0" \
       "H GATEWAY_BANDS в docker-compose.yml не тронут"

step "I (C-094 B3) — selector_fingerprint не подогнан под кэш"
# Развязка обязана быть ЯВНОЙ инвалидацией (шаг D), а не подгонкой отпечатка: подгонка прячет
# смену смысла редьюсера и ломает VB-I-2 в warm-start пути.
chk_sh "git diff \$(git merge-base HEAD origin/main)..HEAD -- crates/gateway/src/lib.rs | grep -E '^[+-].*fn selector_fingerprint' | grep -q . && exit 1 || exit 0" \
       "I selector_fingerprint не переписан"

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
