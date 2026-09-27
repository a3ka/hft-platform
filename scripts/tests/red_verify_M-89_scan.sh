#!/usr/bin/env bash
# Проба двух механических шагов гейта `scripts/verify_M-89.sh` (круг `C-260`):
#   · `task5-scan` — сканер имён бюджета-плацебо В ЛЮБОЙ ФОРМЕ кода, кроме комментариев (N3);
#   · `ci-map`     — сверка таблицы CI-паритета с `ci.yml` в ОБЕ стороны (R5).
#
# Три свойства (`docs/workflow/harness-track.md` §5) на каждый шаг:
#   1. ПОЗИТИВНЫЙ КОНТРОЛЬ — чистый вход даёт PASS;
#   2. АНТИ-ПЛАЦЕБО — мутанты, которые прежняя редакция пропускала, дают FAIL и НАЗЫВАЮТСЯ;
#   3. SETUP-СТРАЖ — режим пробы обязан исполнять ИМЕННО свой шаг (строка `task5:` / `CI-паритет:`).
#
# Прогон: bash scripts/tests/red_verify_M-89_scan.sh

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUT="${ROOT}/scripts/verify_M-89.sh"

PASS=0; FAIL=0
ok()  { PASS=$((PASS + 1)); printf 'ok         %-44s %s\n' "$1" "${2:-}"; }
nok() { FAIL=$((FAIL + 1)); printf 'FAIL       %-44s %s\n' "$1" "$2"; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/red-m89-scan-XXXXXX")"
trap 'rm -rf "${WORK}"' EXIT

# ─────────────── task5-scan ───────────────
scan() { # <dir> → OUT/RC
  OUT=$(VERIFY_M89_MODE=task5-scan VERIFY_M89_SRC_DIR="$1" bash "$SUT" 2>&1); RC=$?
}
expect_scan() { # <имя> <dir> <rc> <описание>
  scan "$2"
  if ! printf '%s\n' "$OUT" | grep -q 'task5:'; then
    nok "$1" "SETUP-СТРАЖ: шаг task5 не исполнялся: $OUT"; return
  fi
  if [ "$RC" -eq "$3" ]; then ok "$1" "$4 (rc=$RC)"; else nok "$1" "$4 — ожидался rc=$3, получен rc=$RC: $(printf '%s' "$OUT" | grep task5)"; fi
}
mk() { # <имя каталога> <содержимое lib.rs>
  mkdir -p "$WORK/$1"; printf '%s\n' "$2" > "$WORK/$1/lib.rs"; printf '%s' "$WORK/$1"
}

expect_scan "t1-clean"            "$(mk clean 'pub fn feed() -> u8 { 1 }')"                      0 "чистый каталог ⇒ PASS"
expect_scan "t2-type-alias"       "$(mk alias 'pub type CallBudget = u8;')"                      1 "type CallBudget = … ⇒ FAIL (прежний сканер не видел)"
expect_scan "t3-block-comment"    "$(mk blk $'/* CallBudget\n   BudgetStop */\nfn f() {}')"       0 "многострочный /* … */ ⇒ PASS (комментарий снят)"
expect_scan "t4-line-comment"     "$(mk line '// CallBudget lives here')"                        0 "// … ⇒ PASS"
expect_scan "t5-word-boundary"    "$(mk wb 'struct CallBudget2; fn cancelled() {} type Cancelled = u8;')" 0 "суффиксы не совпадают (\\b) ⇒ PASS"
expect_scan "t6-use-import"       "$(mk use 'use crate::admission::feed_tail_within;')"           1 "use …::feed_tail_within ⇒ FAIL"
expect_scan "t7-code-after-block" "$(mk mixed $'/* x */ let b = CallBudget::default();')"        1 "код после блочного комментария в той же строке ⇒ FAIL"
expect_scan "t8-nested-dir"       "$(mk nested 'fn f() {}')"                                     0 "подкаталог без нарушений ⇒ PASS"
mkdir -p "$WORK/nested/sub"; printf 'fn g(_: PumpStep) {}\n' > "$WORK/nested/sub/m.rs"
expect_scan "t9-nested-hit"       "$WORK/nested"                                                  1 "нарушение в подкаталоге ⇒ FAIL"
scan "$WORK/alias"
if printf '%s' "$OUT" | grep -q 'lib.rs:1:'; then ok "t10-names-file-line" "нарушитель назван файлом и строкой"; else nok "t10-names-file-line" "нарушитель не назван: $OUT"; fi

# ─────────────── ci-map ───────────────
cimap() { OUT=$(VERIFY_M89_MODE=ci-map VERIFY_M89_CI_FILE="$1" bash "$SUT" 2>&1); RC=$?; }
expect_map() { # <имя> <файл> <rc> <описание> [grep]
  cimap "$2"
  if ! printf '%s\n' "$OUT" | grep -q 'CI-паритет:'; then
    nok "$1" "SETUP-СТРАЖ: шаг CI-паритет не исполнялся: $OUT"; return
  fi
  if [ "$RC" -ne "$3" ]; then nok "$1" "$4 — ожидался rc=$3, получен rc=$RC: $(printf '%s' "$OUT" | grep -m1 'CI-паритет')"; return; fi
  if [ -n "${5:-}" ] && ! printf '%s' "$OUT" | grep -qF -- "$5"; then nok "$1" "$4 — в выводе нет «$5»: $OUT"; return; fi
  ok "$1" "$4 (rc=$RC)"
}
REAL="$ROOT/.github/workflows/ci.yml"
expect_map "c1-real-ci"     "$REAL" 0 "настоящий ci.yml ⇒ таблица полна и не протухла"

EXTRA="$WORK/ci-extra.yml"; cp "$REAL" "$EXTRA"
printf '      - name: новый шаг\n        run: bash scripts/check_brand_new.sh\n' >> "$EXTRA"
expect_map "c2-new-run"     "$EXTRA" 1 "новый run: без строки таблицы ⇒ FAIL и назван" "check_brand_new.sh"

EXTRA_BLK="$WORK/ci-extra-block.yml"; cp "$REAL" "$EXTRA_BLK"
printf '      - run: |\n          bash scripts/check_other_new.sh\n          echo done\n' >> "$EXTRA_BLK"
expect_map "c3-new-block"   "$EXTRA_BLK" 1 "новый многострочный - run: | ⇒ FAIL и назван первой строкой" "check_other_new.sh"

LESS="$WORK/ci-less.yml"; grep -v 'run: bash scripts/check_archived_refs.sh' "$REAL" > "$LESS"
expect_map "c4-removed-run" "$LESS" 1 "run: исчез из ci.yml ⇒ строка таблицы протухла ⇒ FAIL" "check_archived_refs.sh"

: > "$WORK/ci-empty.yml"
expect_map "c5-empty-ci"    "$WORK/ci-empty.yml" 1 "ci.yml без run: ⇒ FAIL (детектор, а не пропуск)"

# ─────────────── task11 — композиция compose ↔ cron (R3: путь обязан быть ФАЙЛОМ) ───────────────
compose_with() { # <путь GATEWAY_HEARTBEAT_PATH> <строка монтирования> → файл
  local f="$WORK/compose-$RANDOM.yml"
  cat > "$f" <<EOF_C
services:
  recorder:
    image: x
  gateway-serve:
    image: x
    environment:
      GATEWAY_JOURNAL_DIR: /journal
      GATEWAY_HEARTBEAT_PATH: $1
    volumes:
      - journal-data:/journal:ro
      - $2
  other:
    image: y
volumes:
  journal-data:
  gateway-state:
EOF_C
  printf '%s' "$f"
}
cron_with() { # <путь> → файл
  local f="$WORK/cron-$RANDOM"
  printf 'SHELL=/bin/bash\nWATCHDOG_SERVING_HEARTBEAT_PATH=%s\n*/5 * * * * root /root/hft-platform/scripts/watchdog_cron.sh\n' "$1" > "$f"
  printf '%s' "$f"
}
t11() { OUT=$(VERIFY_M89_MODE=task11 VERIFY_M89_COMPOSE="$1" VERIFY_M89_CRON="$2" bash "$SUT" 2>&1); RC=$?; }
expect_t11() { # <имя> <compose> <cron> <rc> <описание> [grep]
  t11 "$2" "$3"
  if ! printf '%s\n' "$OUT" | grep -q 'task11:'; then nok "$1" "SETUP-СТРАЖ: шаг task11 не исполнялся: $OUT"; return; fi
  if [ "$RC" -ne "$4" ]; then nok "$1" "$5 — ожидался rc=$4, получен rc=$RC: $(printf '%s' "$OUT" | grep -m1 task11)"; return; fi
  if [ -n "${6:-}" ] && ! printf '%s' "$OUT" | grep -qF -- "$6"; then nok "$1" "$5 — в выводе нет «$6»: $(printf '%s' "$OUT" | grep -m1 task11)"; return; fi
  ok "$1" "$5 (rc=$RC)"
}
HOST=/var/lib/docker/volumes/hft-platform_gateway-state/_data
expect_t11 "p1-good"          "$(compose_with /state/gateway-serve.heartbeat gateway-state:/state)" "$(cron_with $HOST/gateway-serve.heartbeat)" 0 "файл на rw-томе == cron ⇒ PASS"
expect_t11 "p2-trailing-slash" "$(compose_with /state/ gateway-state:/state)"                        "$(cron_with $HOST/)"                          1 "compose /state/ ⇒ FAIL (не файл)" "не путь к файлу"
expect_t11 "p3-mount-root"    "$(compose_with /state gateway-state:/state)"                          "$(cron_with $HOST)"                           1 "compose /state (корень) ⇒ FAIL" "корень монтирования"
expect_t11 "p4-cron-dir"      "$(compose_with /state/gateway-serve.heartbeat gateway-state:/state)" "$(cron_with $HOST/)"                          1 "cron с хвостовым / ⇒ FAIL" "не путь к файлу"
expect_t11 "p5-mismatch"      "$(compose_with /state/gateway-serve.heartbeat gateway-state:/state)" "$(cron_with $HOST/other.heartbeat)"           1 "пути разошлись ⇒ FAIL" "РАЗОШЛАСЬ"
expect_t11 "p6-ro-mount"      "$(compose_with /state/gateway-serve.heartbeat gateway-state:/state:ro)" "$(cron_with $HOST/gateway-serve.heartbeat)" 1 ":ro-монтирование ⇒ FAIL" "rw-монтировании"
expect_t11 "p7-no-cron"       "$(compose_with /state/gateway-serve.heartbeat gateway-state:/state)" "$WORK/absent-cron"                            1 "нет фрагмента cron ⇒ FAIL" "нет "
expect_t11 "p8-dot-leaf"      "$(compose_with /state/. gateway-state:/state)"                        "$(cron_with $HOST/.)"                         1 "служебный лист . ⇒ FAIL" "не путь к файлу"
expect_t11 "p9-off-mount"     "$(compose_with /tmp/hb gateway-state:/state)"                         "$(cron_with /tmp/hb)"                         1 "путь вне монтирований ⇒ FAIL" "не лежит"

printf '\nитого: ok=%d fail=%d\n' "$PASS" "$FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL"; exit 1
