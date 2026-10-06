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

# ── rev5 задача 5a: три состояния пола, `unwrap_or(0)` устранён ──────────────────────
# Корень блокера R-002 глубже потери префикса: `Option<u64>` не различает «сегментов нет»
# и «сегменты есть, но пол установить не удалось», а `unwrap_or(0)` схлопывает второе в
# первое — то есть трактует ОТСУТСТВИЕ ЗНАНИЯ как РАЗРЕШЕНИЕ. Пока эта строка жива,
# декларация с любым next_seq >= 1 проходит на полностью нечитаемом каталоге.
BODY_DECL="$(fn_body 'pub(crate) fn resolve_next_seq_or_declared(')"
if [ -z "$BODY_DECL" ]; then
  bad "T5a канарейка не смогла извлечь тело resolve_next_seq_or_declared (переименована?).
      Гейт НЕ проверен — правь канарейку, а не игнорируй."
elif printf '%s' "$BODY_DECL" | grep -q 'unwrap_or(0)'; then
  bad "T5a в валидации декларации остался unwrap_or(0): отсутствие знания о поле трактуется
      как разрешение (пол=0) ⇒ проходит ЛЮБОЙ next_seq >= 1 ⇒ escape-hatch выдаёт seq-reuse
      с формальным одобрением оператора (R-002, задача 5a)"
else
  ok "T5a unwrap_or(0) устранён из валидации операторской декларации"
fi
if grep -q 'NoSegments' "$SEG" && grep -q 'Unknown' "$SEG"; then
  ok "T5a пол различает три состояния (Known/NoSegments/Unknown)"
else
  bad "T5a три состояния пола не заведены: ожидаются варианты NoSegments и Unknown в $SEG.
      Имена контрактные (milestone rev5, таблица состояний) — оракулы op_6/op_7 стоят на них"
fi

# ── Sacred-оракулы на месте и не выпотрошены ─────────────────────────────────────────
for f in crates/journal/tests/red_tail_integrity.rs \
         crates/journal/tests/red_tail_integrity_operator.rs \
         crates/journal/tests/red_tail_integrity_prodscale.rs \
         crates/journal/tests/red_tail_integrity_operator_prodscale.rs \
         crates/journal/tests/red_tail_integrity_bounded.rs \
         crates/journal/tests/common/mod.rs; do
  if [ -s "$f" ]; then
    ok "sacred-оракул на месте: $f"
  else
    bad "sacred-оракул отсутствует или пуст: $f (тесты architect-only, dev их не правит)"
  fi
done
if grep -qE '#\[(ignore|should_panic)' crates/journal/tests/red_tail_integrity.rs \
     crates/journal/tests/red_tail_integrity_operator.rs \
     crates/journal/tests/red_tail_integrity_operator_prodscale.rs \
     crates/journal/tests/red_tail_integrity_bounded.rs 2>/dev/null; then
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

# ── Прод-масштаб (блокер rev3, R-001 находка 1) ──────────────────────────────────────
# Все остальные фикстуры набора — 8 KiB, то есть проверяют ТОЛЬКО ветку had_header==true.
# Прод пишет сегментами до 1 GiB при окне хвостового скана 4 MiB. Оракул обязателен по
# `.claude/rules/testing.md` §«Прод-масштаб для sacred I/O-путей» (урок TD-011).
# --release: фикстура пишет >4 MiB, в debug это неоправданно долго.
echo
echo "--- прод-масштаб: JR-I-8 на сегменте больше окна скана ---"
if cargo test -p journal --test red_tail_integrity_prodscale --release \
     -- --test-threads=1 2>&1 | tail -20 | grep -qE 'test result: ok'; then
  ok "T-PS ti_7/ti_8: страж держится на сегменте > TAIL_SCAN_CHUNK, здоровый большой стартует"
else
  bad "T-PS прод-масштабный оракул не зелёный — JR-I-8 не держится для файлов прод-размера
      (буфер скана не достаёт до начала файла ⇒ had_header=false ⇒ страж молчит ⇒ seq-reuse)"
fi

# ── rev5 задача 5b: ПУТЬ ДЕКЛАРАЦИИ на прод-масштабном СЫРОМ сегменте (блокер R-002) ──
# ti_7/ti_8 проверяют путь resolve_next_seq, а не путь декларации; комбинация «сырой
# сегмент > окна скана + декларация» не покрывалась НИ ОДНИМ оракулом — и именно там
# защита op_2 обнулилась. Прод-форма: активный сегмент сырой, 868 MB → 1 GiB.
echo
echo "--- прод-масштаб: путь операторской декларации на СЫРОМ сегменте (op_5) ---"
if cargo test -p journal --test red_tail_integrity_operator_prodscale --release \
     -- --test-threads=1 2>&1 | tail -20 | grep -qE 'test result: ok'; then
  ok "T-OP5 пол защиты учитывает читаемый ПРЕФИКС повреждённого прод-масштабного сегмента"
else
  bad "T-OP5 op_5 не зелёный — декларация проходит внутрь занятого диапазона seq на
      прод-форме (сырой сегмент больше окна скана): escape-hatch стал каналом seq-reuse"
fi

# ── rev5 задача 5b: терпимость НЕ ценой памяти (иначе чиним один дефект другим) ───────
# Наивный фикс блокера — «вернуть read_segment_events» — даёт 1 GiB в RAM в момент разбора
# инцидента (класс TD-011). Оба свойства проверяются ОДНИМ тестом и не подлежат размену.
echo
echo "--- граница ресурса: пол честен И стоит O(1) памяти (op_8) ---"
if cargo test -p journal --test red_tail_integrity_bounded --release \
     -- --test-threads=1 2>&1 | tail -20 | grep -qE 'test result: ok'; then
  ok "T-OP8 пол вычисляется терпимо И при ограниченной, не растущей с размером памяти"
else
  bad "T-OP8 op_8 не зелёный — либо пол нечестен (декларация внутри занятого диапазона
      принята), либо куплен памятью (загрузка сегмента целиком, класс TD-011)"
fi

# ── CI-паритет (R-001 находка 4): verify ⊇ терминальные гейты CI (gates.md §3) ───────
echo
if cargo fmt --all -- --check > /tmp/m49_fmt.log 2>&1; then
  ok "CI-паритет: cargo fmt --all --check чист"
else
  bad "CI-паритет: cargo fmt --all --check не чист (main покраснеет)"
  tail -8 /tmp/m49_fmt.log | sed 's/^/      /'
fi
if cargo clippy --workspace --all-targets -- -D warnings > /tmp/m49_ws_clippy.log 2>&1; then
  ok "CI-паритет: clippy --workspace --all-targets чист"
else
  bad "CI-паритет: workspace-clippy не чист (main покраснеет)"
  tail -10 /tmp/m49_ws_clippy.log | sed 's/^/      /'
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
