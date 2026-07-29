#!/usr/bin/env bash
# Acceptance-гейт M-49 — «честность next_seq»: нечитаемый хвост НЕ ИМЕЕТ ПРАВА дать старт
# с meta_seq (TD-049; класс R2b, оставшийся жить на пути порчи после M-40).
#
# Агрегатор с FAIL-счётчиком (не `set -e`: первый FAIL не должен скрывать остальные).
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1
FAILS=0
SEG=crates/journal/src/segments.rs

ok()  { echo "PASS  $1"; }
bad() { echo "FAIL  $1"; FAILS=$((FAILS + 1)); }

# Тело функции: от строки-заголовка до первой закрывающей скобки нулевого отступа.
# Сопоставление ЛИТЕРАЛЬНОЕ (index==1): сигнатуры содержат `(` и `pub(crate)`, а POSIX-awk
# трактует их как группировку — на этом уже падал гейт M-40 (ложный PASS при пустом теле).
fn_body() {
  awk -v pat="$1" '
    index($0, pat) == 1 {f = 1}
    f {print}
    f && /^\}/ {exit}
  ' "$SEG" | sed 's://.*::'
}

echo "=== M-49 acceptance ==="
echo

# ── Задача 1: путь отказа существует и диагностичен ──────────────────────────────────
BODY="$(fn_body 'pub(crate) fn tail_last_seq_of(')"
if [ -z "$BODY" ]; then
  bad "T1 канарейка не смогла извлечь тело tail_last_seq_of (переименована/перемещена?).
      Гейт НЕ проверен — правь канарейку, а не игнорируй."
elif ! printf '%s' "$BODY" | grep -q 'Err('; then
  bad "T1 в tail_last_seq_of НЕТ ни одного пути отказа: любая порча по-прежнему трактуется
      как «сегментов нет» ⇒ старт с meta_seq ⇒ seq-reuse (замер: перекрытие 342 событий)"
else
  ok "T1 tail_last_seq_of имеет путь отказа (Err) на нечитаемом хвосте"
fi

# Диагностика обязана называть файл, причину И действие — иначе оператор в тупике
# (он видит только «recorder не поднялся») и удалит каталог, потеряв историю.
if grep -q 'journal tail unreadable' "$SEG" 2>/dev/null; then
  ok "T1 диагностическое сообщение отказа присутствует"
  if grep -A6 'journal tail unreadable' "$SEG" | grep -qiE 'runbook|re-fetch|declare|перекач|объяв'; then
    ok "T1 сообщение подсказывает ДЕЙСТВИЕ оператору (runbook/перекачать/объявить)"
  else
    bad "T1 сообщение отказа не подсказывает действие — fail-closed без выхода толкает
      оператора удалить каталог (то есть потерять историю)"
  fi
else
  bad "T1 нет диагностического сообщения об отказе (ожидается маркер «journal tail unreadable»)"
fi

# ── Задачи 3-5: операторская декларация ──────────────────────────────────────────────
if grep -qE 'force-next-seq' "$SEG" crates/journal/src/lib.rs 2>/dev/null; then
  ok "T3 операторская декларация journal.force-next-seq.json обрабатывается"
else
  bad "T3 операторской декларации нет: fail-closed без выхода = вечно остановленный сбор
      данных (единственный шаг оператора — удалить каталог)"
fi

# ── Sacred-оракулы на месте и не выпотрошены ─────────────────────────────────────────
for f in crates/journal/tests/red_tail_integrity.rs \
         crates/journal/tests/red_tail_integrity_operator.rs; do
  if [ -s "$f" ]; then
    ok "sacred-оракул на месте: $f"
  else
    bad "sacred-оракул отсутствует или пуст: $f (тесты architect-only, dev их не правит)"
  fi
done
if grep -qE '#\[(ignore|should_panic)' crates/journal/tests/red_tail_integrity.rs \
     crates/journal/tests/red_tail_integrity_operator.rs 2>/dev/null; then
  bad "в sacred-оракулах M-49 появился #[ignore]/#[should_panic] — гейт отключён молча"
else
  ok "в sacred-оракулах M-49 нет #[ignore]/#[should_panic]"
fi

# ── RED-набор M-49 ───────────────────────────────────────────────────────────────────
echo
echo "--- cargo test: RED-набор M-49 ---"
if cargo test -p journal --test red_tail_integrity --test red_tail_integrity_operator \
     -- --test-threads=1 2>&1 | tail -30 | grep -qE 'test result: ok'; then
  ok "T1-T5 red_tail_integrity + red_tail_integrity_operator зелёные"
else
  bad "T1-T5 RED-набор M-49 не зелёный — запусти:
      cargo test -p journal --test red_tail_integrity --test red_tail_integrity_operator -- --test-threads=1"
fi

# ── Регресс: ужесточение НЕ имеет права сломать существующие контракты ───────────────
# M-49 правит tail_last_seq_of — функцию, на которой стоят восстановление после сбоя,
# прод-миграция legacy, bounded open (TD-011) и весь restore-путь M-40.
echo
echo "--- регресс journal (ужесточение точное?) ---"
# ВАЖНО: перечисляем СУЩЕСТВУЮЩИЕ таргеты явно и НЕ включаем оракулы M-49 — иначе на
# RED-стадии регресс-блок ловит собственные красные тесты и рапортует «регресс сломан»
# (дефект первой редакции гейта, поймано прогоном).
REG=$(cargo test -p journal --lib \
  --test red_cli_argv --test red_compaction --test red_l2delta_persist \
  --test red_l2delta_rollback_boundary --test red_open_bounded --test red_prod_migration \
  --test red_read_all_v2 --test red_recover --test red_restore_from_cold \
  --test red_restore_next_seq_bounded --test red_retention_checkpoint_coverage \
  --test red_retention_compacted --test red_retention_operator --test red_retention \
  --test red_rotation --test red_seg0_removed --test red_segments_epochs --test red_shutdown \
  --test red_stream_bounded --test red_stream_from 2>&1)
if printf '%s' "$REG" | grep -qE 'test result: FAILED'; then
  bad "регресс: ужесточение сломало существующие оракулы journal"
  printf '%s' "$REG" | grep -E '^(test|---- ).*(FAILED|stdout)' | head -20 | sed 's/^/      /'
else
  N_OK=$(printf '%s' "$REG" | grep -cE 'test result: ok')
  ok "регресс: все блоки journal зелёные ($N_OK блоков) — recover/prod-migration/open-bounded/restore целы"
fi

# ── Клиппи ───────────────────────────────────────────────────────────────────────────
echo
if cargo clippy -p journal --all-targets -- -D warnings > /tmp/m49_clippy.log 2>&1; then
  ok "clippy -p journal --all-targets чист"
else
  bad "clippy -p journal не чист (хвост в /tmp/m49_clippy.log)"
  tail -12 /tmp/m49_clippy.log | sed 's/^/      /'
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
