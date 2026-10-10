#!/usr/bin/env bash
# Acceptance-гейт M-50 — «скан пола видит крупные события» (JR-I-9, TD-053): валидный
# фрейм крупнее капа carry (64 KiB), но в пределах санити-капа ридера (64 MiB), НЕ имеет
# права молча выпадать из пола валидации операторской декларации (иначе seq-reuse).
#
# Агрегатор с FAIL-счётчиком (не `set -e`: первый FAIL не должен скрывать остальные).
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1
FAILS=0
SEG=crates/journal/src/segments.rs

ok()  { echo "PASS  $1"; }
bad() { echo "FAIL  $1"; FAILS=$((FAILS + 1)); }

# Тело функции: от строки-заголовка до первой закрывающей скобки нулевого отступа.
# Сопоставление ЛИТЕРАЛЬНОЕ (index==1): сигнатуры содержат `(` и `<R: Read>`, а POSIX-awk
# трактует их как синтаксис — на этом уже был ложный PASS в M-40.
fn_body() {
  awk -v pat="$1" '
    index($0, pat) == 1 {f = 1}
    f {print}
    f && /^\}/ {exit}
  ' "$SEG" | sed 's://.*::'
}

echo "=== M-50 acceptance ==="
echo

# ── Задача 1: ЕДИНАЯ константа санити-капа длины фрейма ──────────────────────────────
# Корень TD-053 — ДВЕ независимые константы (64 KiB carry против 64 MiB ридера),
# дрейфующие врозь. Кап ридера и кап «валидного фрейма» для скана пола обязаны быть
# ОДНИМ именем. Имя контрактное (milestone задача 1).
N_CAP=$(grep -c 'FRAME_LEN_SANITY_CAP' "$SEG" 2>/dev/null || echo 0)
if [ "$N_CAP" -eq 0 ]; then
  bad "T-CAP константа FRAME_LEN_SANITY_CAP не найдена в $SEG — санити-кап ридера и кап
      скана пола остаются ДВУМЯ независимыми числами (корень TD-053)"
elif [ "$N_CAP" -lt 3 ]; then
  bad "T-CAP FRAME_LEN_SANITY_CAP упоминается лишь $N_CAP раз(а) — ожидается определение
      + использование И в read_frame_payload, И в скане пола (>=3)"
else
  ok "T-CAP единая константа FRAME_LEN_SANITY_CAP объявлена и используется ($N_CAP вхождений)"
fi
# Голый литерал 64 MiB вне строки-определения константы = дрейф возможен снова.
STRAY=$(grep -n '64 \* 1024 \* 1024' "$SEG" 2>/dev/null | grep -vc 'FRAME_LEN_SANITY_CAP')
if [ "$STRAY" -gt 0 ]; then
  bad "T-CAP голый литерал 64 * 1024 * 1024 в $SEG вне определения константы ($STRAY шт.) —
      кап снова может дрейфовать врозь:
$(grep -n '64 \* 1024 \* 1024' "$SEG" | grep -v 'FRAME_LEN_SANITY_CAP' | head -5 | sed 's/^/      /')"
else
  ok "T-CAP голого литерала 64 MiB вне определения константы не осталось"
fi

# ── Задача 2: скан пола различает «крупный кандидат» и «мусор» по константе ──────────
BODY="$(fn_body 'fn resync_max_seq')"
if [ -z "$BODY" ]; then
  bad "T2 канарейка не смогла извлечь тело resync_max_seq (переименована/перемещена?).
      Гейт НЕ проверен — правь канарейку, а не игнорируй."
elif ! printf '%s' "$BODY" | grep -q 'FRAME_LEN_SANITY_CAP'; then
  bad "T2 resync_max_seq не ссылается на FRAME_LEN_SANITY_CAP: крупный кандидат
      по-прежнему неотличим от мусора ⇒ фрейм > 64 KiB невидим ⇒ пол занижен (TD-053)"
else
  ok "T2 resync_max_seq различает крупный кандидат по FRAME_LEN_SANITY_CAP"
fi

# ── Sacred-оракулы на месте и не выпотрошены ─────────────────────────────────────────
for f in crates/journal/tests/red_floor_scan.rs \
         crates/journal/tests/red_floor_scan_bounded.rs \
         crates/journal/tests/red_floor_scan_prodscale.rs \
         crates/journal/tests/common/mod.rs; do
  if [ -s "$f" ]; then
    ok "sacred-оракул на месте: $f"
  else
    bad "sacred-оракул отсутствует или пуст: $f (тесты architect-only, dev их не правит)"
  fi
done
if grep -qE '#\[(ignore|should_panic)' crates/journal/tests/red_floor_scan.rs \
     crates/journal/tests/red_floor_scan_bounded.rs \
     crates/journal/tests/red_floor_scan_prodscale.rs 2>/dev/null; then
  bad "в sacred-оракулах M-50 появился #[ignore]/#[should_panic] — гейт отключён молча"
else
  ok "в sacred-оракулах M-50 нет #[ignore]/#[should_panic]"
fi

# ── Оракулы M-50 (все --release: фикстуры пишут и сканируют 5-22 MiB) ────────────────
echo
echo "--- fs_1..fs_7, fs_10: видимость крупных фреймов, границы, терпимость, .zst ---"
if cargo test -p journal --test red_floor_scan --release \
     -- --test-threads=1 2>&1 | tail -30 | grep -qE 'test result: ok'; then
  ok "T2/T3/T5 red_floor_scan зелёный (граница капа, множественность, кросс-сегмент, .zst, штатный путь)"
else
  bad "T2/T3/T5 red_floor_scan не зелёный — запусти:
      cargo test -p journal --test red_floor_scan --release -- --test-threads=1"
fi

echo
echo "--- fs_8: граница ПАМЯТИ — тело крупного фрейма не буферизуется ---"
if cargo test -p journal --test red_floor_scan_bounded --release \
     -- --test-threads=1 2>&1 | tail -20 | grep -qE 'test result: ok'; then
  ok "T4 fs_8: пол видит событие 16 MiB И пик памяти ограничен, не растёт с размером события"
else
  bad "T4 fs_8 не зелёный — либо крупное событие невидимо полу (fail-open, TD-053), либо
      верификация буферизует тело фрейма (класс TD-011, запрещённый размен rev5)"
fi

echo
echo "--- fs_9: прод-масштаб — крупное событие за префиксом больше окна скана ---"
if cargo test -p journal --test red_floor_scan_prodscale --release \
     -- --test-threads=1 2>&1 | tail -20 | grep -qE 'test result: ok'; then
  ok "T3 fs_9: пол честен на прод-форме (префикс > TAIL_SCAN_CHUNK, декларация работает)"
else
  bad "T3 fs_9 не зелёный — на прод-форме сегмента пол не видит крупное событие
      (или честная декларация перестала разблокировать старт)"
fi

# ── Регресс: фикс НЕ имеет права купить крупные фреймы ценой существующих контрактов ─
# M-50 правит resync_max_seq/tolerant_max_seq_from_start — на них стоит весь путь
# операторской декларации M-49 (op_1..op_8) и рядом живут строгие пути (ti_6).
# Перечисляем СУЩЕСТВУЮЩИЕ таргеты явно и НЕ включаем оракулы M-50 — иначе на RED-стадии
# регресс-блок ловит собственные красные тесты (дефект первой редакции гейта M-49).
echo
echo "--- регресс journal (включая ПОЛНЫЙ набор M-49) ---"
REG=$(cargo test -p journal --lib \
  --test red_cli_argv --test red_compaction --test red_l2delta_persist \
  --test red_l2delta_rollback_boundary --test red_open_bounded --test red_prod_migration \
  --test red_read_all_v2 --test red_recover --test red_restore_from_cold \
  --test red_restore_next_seq_bounded --test red_retention_checkpoint_coverage \
  --test red_retention_compacted --test red_retention_operator --test red_retention \
  --test red_rotation --test red_seg0_removed --test red_segments_epochs --test red_shutdown \
  --test red_stream_bounded --test red_stream_from \
  --test red_tail_integrity --test red_tail_integrity_operator 2>&1)
if printf '%s' "$REG" | grep -qE 'test result: FAILED'; then
  bad "регресс: фикс сломал существующие оракулы journal"
  printf '%s' "$REG" | grep -E '^(test|---- ).*(FAILED|stdout)' | head -20 | sed 's/^/      /'
else
  N_OK=$(printf '%s' "$REG" | grep -cE 'test result: ok')
  ok "регресс: все блоки journal зелёные ($N_OK блоков) — M-49/restore/recover/bounded целы"
fi
echo
echo "--- регресс прод-масштаба M-49 (--release: op_5, op_8, ti_7/ti_8) ---"
REG49=$(cargo test -p journal --release \
  --test red_tail_integrity_prodscale --test red_tail_integrity_operator_prodscale \
  --test red_tail_integrity_bounded -- --test-threads=1 2>&1)
if printf '%s' "$REG49" | grep -qE 'test result: FAILED'; then
  bad "регресс: прод-масштабные оракулы M-49 (op_5/op_8/ti_7/ti_8) сломаны — терпимость
      или граница памяти куплены фиксом M-50 (запрещённый размен)"
  printf '%s' "$REG49" | grep -E '^(test|---- ).*(FAILED|stdout)' | head -12 | sed 's/^/      /'
else
  ok "регресс: op_5/op_8/ti_7/ti_8 зелёные — терпимость и граница памяти M-49 не тронуты"
fi

# ── CI-паритет: verify ⊇ терминальные гейты CI (gates.md §3) ─────────────────────────
echo
if cargo fmt --all -- --check > /tmp/m50_fmt.log 2>&1; then
  ok "CI-паритет: cargo fmt --all --check чист"
else
  bad "CI-паритет: cargo fmt --all --check не чист (main покраснеет)"
  tail -8 /tmp/m50_fmt.log | sed 's/^/      /'
fi
if cargo clippy --workspace --all-targets -- -D warnings > /tmp/m50_ws_clippy.log 2>&1; then
  ok "CI-паритет: clippy --workspace --all-targets чист"
else
  bad "CI-паритет: workspace-clippy не чист (main покраснеет)"
  tail -10 /tmp/m50_ws_clippy.log | sed 's/^/      /'
fi
if cargo clippy -p journal --all-targets -- -D warnings > /tmp/m50_clippy.log 2>&1; then
  ok "clippy -p journal --all-targets чист"
else
  bad "clippy -p journal не чист (хвост в /tmp/m50_clippy.log)"
  tail -12 /tmp/m50_clippy.log | sed 's/^/      /'
fi

echo
echo "=== итог: FAIL=$FAILS ==="
if [ "$FAILS" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL"
  exit 1
fi
