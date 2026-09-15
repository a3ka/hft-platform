#!/usr/bin/env bash
# Acceptance-гейт M-51 — DET-I-1/2/3: бит-идентичный реплей становится ИСПОЛНИМЫМ.
#
# Закрывает TD-007 («DET-I-1 реализован ЧАСТИЧНО (seq+read_all); state_hash — нет»,
# не менявшийся с 2026-07-10) по замеру
# `research/measurements/td-007-determinism-coverage.md`:
#   DET-I-1 — реплей потока бит-идентичен (в т.ч. через границу процесса/формата/окна);
#   DET-I-2 — проекция: инкремент == пересборка реплеем (PL-I-1, JR-I-4);
#   DET-I-3 — доменный редьюсер не зависит от хэш-сида/порядка ФС.
#
# Агрегатор с FAIL-счётчиком (НЕ `set -e`: первый FAIL не должен скрывать остальные).
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1
FAILS=0
SEG=crates/journal/src/segments.rs
JLIB=crates/journal/src/lib.rs
BOOK=crates/book/src/lib.rs
EXCH=crates/sim/src/exchange.rs

ok()  { echo "PASS  $1"; }
bad() { echo "FAIL  $1"; FAILS=$((FAILS + 1)); }

# Тело функции: от строки-заголовка до первой закрывающей скобки нулевого отступа.
# Сопоставление ЛИТЕРАЛЬНОЕ (index==1): сигнатуры содержат `(` и `<`, а POSIX-awk трактует
# их как синтаксис регулярного выражения — на этом уже был ложный PASS в M-40.
fn_body() {
  awk -v pat="$2" '
    index($0, pat) == 1 {f = 1}
    f {print}
    f && /^\}/ {exit}
  ' "$1" | sed 's://.*::'
}

echo "=== M-51 acceptance — DET-I-1/2/3 ==="
echo

# ── Задача 1: контрактный примитив реплея (TD-007 «state_hash» дословно) ─────────────
# Имена КОНТРАКТНЫЕ — на них стоят и оракулы, и эта канарейка.
# ВНИМАНИЕ: `$(grep -c ... || echo 0)` НЕЛЬЗЯ — при нуле совпадений grep печатает "0" И
# выходит с кодом 1, поэтому `||` добавляет ВТОРОЙ "0", и `[ "$N" -eq 0 ]` ломается,
# давая ЛОЖНЫЙ PASS (поймано прогоном этого гейта на RED-состоянии M-51).
N_RD=$(grep -c 'fn replay_digest' "$SEG" 2>/dev/null | head -1)
N_RD=${N_RD:-0}
if [ "$N_RD" -eq 0 ]; then
  bad "T1 функция replay_digest не найдена в $SEG — TD-007 п.3 (state_hash) по-прежнему
      буквально отсутствует; «бит-идентичный реплей» остаётся непроверяемым утверждением"
else
  ok "T1 replay_digest объявлена в $SEG"
fi
if grep -q 'pub struct ReplayDigest' "$SEG" 2>/dev/null; then
  # Контрактная форма: без этих полей оракулы не могут отличить «пусто» от «не посчитано».
  MISSING=""
  for f in 'events' 'first_seq' 'last_seq' 'state_hash'; do
    grep -A8 'pub struct ReplayDigest' "$SEG" | grep -q "pub $f" || MISSING="$MISSING $f"
  done
  if [ -n "$MISSING" ]; then
    bad "T1 в ReplayDigest нет обязательных полей:$MISSING (контракт: events, first_seq,
        last_seq, state_hash)"
  else
    ok "T1 ReplayDigest несёт контрактные поля (events/first_seq/last_seq/state_hash)"
  fi
else
  bad "T1 тип ReplayDigest не найден в $SEG"
fi
if grep -q 'replay_digest' "$JLIB" 2>/dev/null && grep -q 'ReplayDigest' "$JLIB" 2>/dev/null; then
  ok "T1 replay_digest/ReplayDigest ре-экспортированы из крейта journal"
else
  bad "T1 replay_digest/ReplayDigest не ре-экспортированы в $JLIB — оракулы и потребители
      (gateway/research) не увидят примитив"
fi

# ── Задача 2: реплей ПОТОКОВЫЙ, а не через read_all (TD-011: прод 27 GB) ─────────────
BODY="$(fn_body "$SEG" 'pub fn replay_digest')"
if [ -z "$BODY" ]; then
  bad "T2 канарейка не смогла извлечь тело replay_digest (переименована/перемещена?).
      Гейт НЕ проверен — правь канарейку, а не игнорируй."
else
  if printf '%s' "$BODY" | grep -q 'read_all'; then
    bad "T2 replay_digest вызывает read_all — весь журнал в RAM. На проде это 27 GB /
        145 992 262 события (замер 2026-08-01): ровно класс TD-011, когда recorder
        переставал писать. Реплей обязан идти через stream/stream_from"
  else
    ok "T2 replay_digest не опирается на read_all (потоковый путь)"
  fi
  if printf '%s' "$BODY" | grep -qE 'stream_from|stream\('; then
    ok "T2 replay_digest использует потоковое чтение (stream/stream_from)"
  else
    bad "T2 в теле replay_digest не видно stream/stream_from — потоковость не подтверждена"
  fi
fi

# ── Задача 3: проекция перечислима детерминированно (DET-I-2 / PL-I-1) ───────────────
if grep -q 'fn iter_sorted' "$BOOK" 2>/dev/null; then
  ok "T3 Books::iter_sorted объявлен — проекция перечислима без обхода HashMap"
else
  bad "T3 Books::iter_sorted не найден в $BOOK: состояние проекции невозможно снять целиком,
      не положившись на порядок HashMap ⇒ «проекция воспроизводима реплеем» (DESIGN §14,
      PL-I-1) остаётся НЕПРОВЕРЯЕМЫМ утверждением"
fi

# ── Задача 4: активные ордера симулятора — упорядоченная структура (DET-I-3) ─────────
if grep -qE '^\s*active:\s*(std::collections::)?HashMap<' "$EXCH" 2>/dev/null; then
  bad "T4 BacktestExchange.active остаётся HashMap ($EXCH) — порядок Vec<SimFill> на такте
      задаётся хэш-сидом процесса (аудит TD-007 §3.1, прямое нарушение запрета CLAUDE.md)"
else
  ok "T4 BacktestExchange.active не HashMap — порядок филлов задаётся данными"
fi

# ── Sacred-оракулы на месте и не выпотрошены ─────────────────────────────────────────
ORACLES="crates/journal/tests/red_det_replay_digest.rs
crates/journal/tests/red_det_restart.rs
crates/journal/tests/red_det_prodscale.rs
crates/journal/tests/red_det_sources.rs
crates/sim/tests/red_det_fill_order.rs
crates/book/tests/red_det_projection.rs"
for f in $ORACLES; do
  if [ -s "$f" ]; then
    ok "sacred-оракул на месте: $f"
  else
    bad "sacred-оракул отсутствует или пуст: $f (тесты architect-only, dev их не правит)"
  fi
done
# shellcheck disable=SC2086
if grep -qE '#\[(ignore|should_panic)' $ORACLES 2>/dev/null; then
  bad "в sacred-оракулах M-51 появился #[ignore]/#[should_panic] — гейт отключён молча"
else
  ok "в sacred-оракулах M-51 нет #[ignore]/#[should_panic]"
fi
# Waiver — механизм с НАЗВАННОЙ причиной; пустой маркер обесценивает аудит-трейл.
STRAY_WAIVER=$(grep -rnE 'DET-OK:\s*$' crates/*/src --include=*.rs 2>/dev/null | wc -l)
if [ "$STRAY_WAIVER" -gt 0 ]; then
  bad "пустой waiver 'DET-OK:' без причины ($STRAY_WAIVER шт.) — причина обязательна:
$(grep -rnE 'DET-OK:\s*$' crates/*/src --include=*.rs | head -5 | sed 's/^/      /')"
else
  ok "все waiver'ы DET-OK несут названную причину"
fi

# ── Оракулы M-51 ────────────────────────────────────────────────────────────────────
echo
echo "--- det_1..det_8: реплей потока бит-идентичен (DET-I-1) ---"
if cargo test -p journal --test red_det_replay_digest 2>&1 | tail -30 | grep -qE 'test result: ok'; then
  ok "det_1..det_8 зелёные (эталон, дискриминация, компакция, окна, эпохи, деградации)"
else
  bad "det_1..det_8 не зелёные — запусти:
      cargo test -p journal --test red_det_replay_digest"
fi

echo
echo "--- det_9..det_11: через границу ПРОЦЕССА (failover == replay) ---"
if cargo test -p journal --test red_det_restart 2>&1 | tail -30 | grep -qE 'test result: ok'; then
  ok "det_9..det_11 зелёные (рестарт процесса, дискриминация окон, append-only прошлого)"
else
  bad "det_9..det_11 не зелёные — реплей зависит от per-process состояния (хэш-сид, read_dir,
      адреса) ЛИБО дозапись переписывает дайджест прошлого окна"
fi

echo
echo "--- det_22..det_25: источники недетерминизма в доменном коде (DET-I-3) ---"
if cargo test -p journal --test red_det_sources 2>&1 | tail -30 | grep -qE 'test result: ok'; then
  ok "det_22..det_25 зелёные (обход хэш-карт, read_dir, rand/rayon, отчёт ретеншена)"
else
  bad "det_22..det_25 не зелёные — в доменном коде остался необъявленный источник
      недетерминизма (см. вывод теста: файл:строка)"
fi

echo
echo "--- det_14..det_17: порядок филлов симулятора (DET-I-3, аудит §3.1) ---"
if cargo test -p sim --test red_det_fill_order 2>&1 | tail -30 | grep -qE 'test result: ok'; then
  ok "det_14..det_17 зелёные (множественность, асимметрия, дефицитный объём, масштаб 64)"
else
  bad "det_14..det_17 не зелёные — Vec<SimFill> на такте зависит от хэш-сида: два реплея
      одного журнала дают разный ответ «какой ордер исполнился первым» (DESIGN §1)"
fi

echo
echo "--- det_18..det_21: проекция == пересборка реплеем (DET-I-2 / PL-I-1 / JR-I-4) ---"
if cargo test -p book --test red_det_projection 2>&1 | tail -30 | grep -qE 'test result: ok'; then
  ok "det_18..det_21 зелёные (live==replay, префикс+догон через компакцию, канон различает)"
else
  bad "det_18..det_21 не зелёные — цифра, посчитанная инкрементально, НЕ воспроизводится
      реплеем: то, что продукт продаёт (DESIGN §0), не выполняется"
fi

echo
echo "--- det_12..det_13: прод-форма и граница ПАМЯТИ (--release) ---"
if cargo test -p journal --test red_det_prodscale --release \
     -- --test-threads=1 2>&1 | tail -20 | grep -qE 'test result: ok'; then
  ok "det_12/det_13 зелёные (154-сегментная форма raw+.zst; пик памяти не растёт с размером)"
else
  bad "det_12/det_13 не зелёные — либо реплей недетерминирован на прод-форме, либо память
      реплея НЕ O(1) (на 27 GB прода такая реализация неработоспособна, класс TD-011)"
fi

# ── Регресс: детерминизм не имеет права быть куплен ценой существующих контрактов ────
# M-51 правит journal/segments.rs (новый примитив), book/lib.rs (обход), sim/exchange.rs
# (структура active). На них стоят M-49/M-50 (пол/хвост), M-30 (gap-детекция), SM-I-*.
# Оракулы M-51 в регресс-блок НЕ включаются — иначе на RED-стадии он ловит собственные
# красные тесты (дефект первой редакции гейта M-49).
echo
echo "--- регресс journal (полный набор M-49/M-50 и остальное) ---"
REG=$(cargo test -p journal --lib \
  --test red_cli_argv --test red_compaction --test red_floor_scan \
  --test red_floor_scan_bounded --test red_l2delta_persist \
  --test red_l2delta_rollback_boundary --test red_open_bounded --test red_prod_migration \
  --test red_read_all_v2 --test red_recover --test red_restore_from_cold \
  --test red_restore_next_seq_bounded --test red_retention_checkpoint_coverage \
  --test red_retention_compacted --test red_retention_operator --test red_retention \
  --test red_rotation --test red_seg0_removed --test red_segments_epochs --test red_shutdown \
  --test red_stream_bounded --test red_stream_from \
  --test red_tail_integrity --test red_tail_integrity_operator 2>&1)
if printf '%s' "$REG" | grep -qE 'test result: FAILED'; then
  bad "регресс: M-51 сломал существующие оракулы journal"
  printf '%s' "$REG" | grep -E '^(test|---- ).*(FAILED|stdout)' | head -20 | sed 's/^/      /'
else
  N_OK=$(printf '%s' "$REG" | grep -cE 'test result: ok')
  ok "регресс journal: все блоки зелёные ($N_OK) — M-49/M-50/restore/recover/bounded целы"
fi

echo
echo "--- регресс прод-масштаба (--release: M-49 op_*/ti_*, M-50 fs_9) ---"
REG49=$(cargo test -p journal --release \
  --test red_tail_integrity_prodscale --test red_tail_integrity_operator_prodscale \
  --test red_tail_integrity_bounded --test red_floor_scan_prodscale \
  -- --test-threads=1 2>&1)
if printf '%s' "$REG49" | grep -qE 'test result: FAILED'; then
  bad "регресс: прод-масштабные оракулы M-49/M-50 сломаны — терпимость или граница памяти
      куплены изменениями M-51 (запрещённый размен)"
  printf '%s' "$REG49" | grep -E '^(test|---- ).*(FAILED|stdout)' | head -12 | sed 's/^/      /'
else
  ok "регресс: op_*/ti_*/fs_9 зелёные — терпимость и граница памяти не тронуты"
fi

echo
echo "--- регресс book / sim / strategy / portfolio / alpha ---"
# Таргеты перечислены ЯВНО и БЕЗ оракулов M-51: иначе на RED-стадии регресс-блок ловит
# собственные красные тесты (дефект первой редакции гейта M-49).
# Проверяется ЭКЗИТ-КОД, а не только отсутствие «test result: FAILED»: крейт, который НЕ
# СОБРАЛСЯ, не печатает ни одной строки результата — и первая редакция этого гейта выдавала
# на нём «все блоки зелёные (0)» (ложный PASS, поймано прогоном на RED-состоянии M-51).
run_reg() {
  local label="$1"; shift
  local out rc n
  out=$("$@" 2>&1); rc=$?
  n=$(printf '%s' "$out" | grep -cE 'test result: ok')
  if [ "$rc" -ne 0 ] || printf '%s' "$out" | grep -qE 'test result: FAILED'; then
    bad "регресс $label: exit=$rc (0 = собралось и прошло). M-51 сломал существующие контракты
        либо крейт не компилируется:"
    printf '%s' "$out" | grep -E '^(error|test|---- ).*(FAILED|stdout|\[E[0-9])' | head -12 \
      | sed 's/^/      /'
  elif [ "$n" -eq 0 ]; then
    bad "регресс $label: НИ ОДНОГО блока «test result: ok» при exit=0 — гейт не проверил
        ничего (таргеты отфильтрованы?). Правь канарейку, а не игнорируй."
  else
    ok "регресс $label: все блоки зелёные ($n)"
  fi
}

run_reg "book (M-30 gap-детекция, BK-I-*)" cargo test -p book --lib \
  --test red_gap_detection --test red_l2delta_apply --test red_orderbook_serde_roundtrip \
  --test test_levels_access --test test_top_n_depth
run_reg "sim (SM-I-*)" cargo test -p sim --lib \
  --test red_md_expansion --test red_sim --test red_strategy_backtest --test structural
run_reg "strategy/portfolio/alpha (ST-I-*)" cargo test -p strategy -p portfolio -p alpha
run_reg "gateway/research-cli (GW-I-*, RC-I-5)" cargo test -p gateway -p research-cli

# ── CI-паритет: verify ⊇ терминальные гейты CI (gates.md §3) ─────────────────────────
echo
if cargo fmt --all -- --check > /tmp/m51_fmt.log 2>&1; then
  ok "CI-паритет: cargo fmt --all --check чист"
else
  bad "CI-паритет: cargo fmt --all --check не чист (main покраснеет)"
  tail -8 /tmp/m51_fmt.log | sed 's/^/      /'
fi
if cargo clippy --workspace --all-targets -- -D warnings > /tmp/m51_ws_clippy.log 2>&1; then
  ok "CI-паритет: clippy --workspace --all-targets чист"
else
  bad "CI-паритет: workspace-clippy не чист (main покраснеет)"
  tail -12 /tmp/m51_ws_clippy.log | sed 's/^/      /'
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
