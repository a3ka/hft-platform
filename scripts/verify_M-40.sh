#!/usr/bin/env bash
# Acceptance-гейт M-40 — «ретеншен и писатель видят сжатые сегменты» (риск R2/R2b, docs/08).
#
# Гейт агрегирующий: НЕ `set -e` на проверках (иначе первый FAIL скрывает остальные), но
# FAIL-счётчик + явный `exit 1`. Никакого `cmd && echo PASS || echo FAIL` — маскирует провал.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1
FAILS=0
SEG=crates/journal/src/segments.rs

ok()   { echo "PASS  $1"; }
bad()  { echo "FAIL  $1"; FAILS=$((FAILS + 1)); }

# Тело функции Rust: от строки-заголовка до первой закрывающей скобки нулевого отступа.
# Комментарии вырезаются (`// ...`), чтобы канарейка ловила КОД, а не упоминание в доке.
#
# Сопоставление ЛИТЕРАЛЬНОЕ (`index($0,pat)==1`), а не регэкспом: сигнатуры содержат `(` и
# `pub(crate)`, и POSIX-awk трактует их как группировку — первая редакция гейта на этом
# падала с `fatal: invalid regexp`, fn_body возвращала ПУСТО, grep ничего не находил и
# канарейка печатала PASS. Ложный гейт хуже отсутствующего, поэтому ниже — setup-guard.
fn_body() {
  awk -v pat="$1" '
    index($0, pat) == 1 {f = 1}
    f {print}
    f && /^\}/ {exit}
  ' "$SEG" | sed 's://.*::'
}

# Канарейка «в функции нет собственного read_dir», которая ОБЯЗАНА краснеть, если не сумела
# извлечь тело функции (иначе доказывает не то, что заявлено).
no_own_readdir() {
  local sig="$1" name="$2" task="$3" body
  body="$(fn_body "$sig")"
  if [ -z "$body" ]; then
    bad "$task канарейка не смогла извлечь тело $name (сигнатура «$sig» не найдена —
      функция переименована/перемещена?). Гейт НЕ проверен: правь канарейку, а не игнорируй."
  elif printf '%s' "$body" | grep -q 'read_dir'; then
    bad "$task $name перечисляет каталог САМ (собственный read_dir)"
  else
    ok "$task $name не имеет собственного read_dir (тело: $(printf '%s' "$body" | wc -l) строк)"
  fi
}

echo "=== M-40 acceptance ==="
echo

# ── Задача 1+7: ОДИН энумератор сегментов на крейт ──────────────────────────────────
# Корень дефекта — ТРИ независимых обхода каталога (dedup_indexed_paths / retention_plan /
# latest_segment_index), из которых два не знают про суффикс `.jrnl.zst`. Заплатка «дописать
# ещё одно условие в фильтр» сохраняет корень; гейт требует сведения к общему хелперу.
N_READDIR=$(sed 's://.*::' "$SEG" | grep -c 'fs::read_dir')
if [ "$N_READDIR" -eq 1 ]; then
  ok "T1/T7 в segments.rs ровно ОДИН fs::read_dir (общий энумератор dedup_indexed_paths)"
else
  bad "T1/T7 в segments.rs $N_READDIR вызовов fs::read_dir (ожидался 1). Каждый лишний обход —
      новый источник расхождения «кто видит .zst». Если новый обход обоснован, гейт правит
      architect вместе с обоснованием, а не dev по ходу задачи."
  sed 's://.*::' "$SEG" | grep -n 'fs::read_dir' | sed 's/^/      /'
fi

no_own_readdir 'pub fn retention_plan(' 'retention_plan' 'T1'
no_own_readdir 'pub(crate) fn latest_segment_index(' 'latest_segment_index' 'T7'

# `extension() == Some("jrnl")` — та самая конструкция, из-за которой `.jrnl.zst` (extension
# = "zst") выпадал из обхода. В коде крейта её быть не должно нигде.
N_EXT=$(sed 's://.*::' "$SEG" | grep -c 'extension().*"jrnl"')
if [ "$N_EXT" -eq 0 ]; then
  ok "T1/T7 нигде не осталось фильтра extension()==\"jrnl\" (он и прятал .jrnl.zst)"
else
  bad "T1/T7 фильтр extension()==\"jrnl\" ещё присутствует ($N_EXT шт.) — .zst продолжит выпадать"
  sed 's://.*::' "$SEG" | grep -n 'extension().*"jrnl"' | sed 's/^/      /'
fi

# ── Sacred-оракулы на месте (не удалены/не выпотрошены) ─────────────────────────────
for f in crates/journal/tests/red_retention_compacted.rs \
         crates/journal/tests/red_restore_from_cold.rs \
         crates/journal/tests/red_restore_next_seq_bounded.rs; do
  if [ -s "$f" ]; then
    ok "sacred-оракул на месте: $f"
  else
    bad "sacred-оракул отсутствует или пуст: $f (тесты — architect-only, dev их не правит)"
  fi
done
# `#[ignore]`/`#[should_panic]` в sacred-наборе = тихое отключение гейта.
if grep -qE '#\[(ignore|should_panic)' crates/journal/tests/red_retention_compacted.rs \
      crates/journal/tests/red_restore_from_cold.rs \
      crates/journal/tests/red_restore_next_seq_bounded.rs; then
  bad "в sacred-оракулах M-40 появился #[ignore]/#[should_panic] — гейт отключён молча"
else
  ok "в sacred-оракулах M-40 нет #[ignore]/#[should_panic]"
fi

# ── Задачи 1-6: RED-набор M-40 ──────────────────────────────────────────────────────
echo
echo "--- cargo test: RED-набор M-40 ---"
if cargo test -p journal --test red_retention_compacted --test red_restore_from_cold \
     -- --test-threads=1 2>&1 | tail -40 | grep -qE 'test result: ok'; then
  ok "T1-T6 red_retention_compacted + red_restore_from_cold зелёные"
else
  bad "T1-T6 RED-набор M-40 не зелёный — запусти:
      cargo test -p journal --test red_retention_compacted --test red_restore_from_cold -- --test-threads=1"
fi

# ── Задача 8: прод-масштаб (bounded memory при открытии) ────────────────────────────
# Только release: в debug счётчик аллокаций ловит отладочную обвязку, а объём фикстуры
# (60k событий × ~2 KB) делает debug-прогон неоправданно долгим.
echo
echo "--- cargo test --release: прод-масштабный оракул ---"
if cargo test -p journal --test red_restore_next_seq_bounded --release \
     -- --test-threads=1 2>&1 | tail -20 | grep -qE 'test result: ok'; then
  ok "T8 next_seq поверх сжатой истории: корректен и укладывается в бюджет памяти"
else
  bad "T8 red_restore_next_seq_bounded не зелёный (корректность next_seq ИЛИ граница памяти).
      Наивная распаковка сегмента целиком = OOM при каждом старте recorder'а (класс TD-011)"
fi

# ── Регресс: контракты M-08 и M-38b обязаны остаться зелёными ───────────────────────
# M-40 трогает ровно тот код, на котором стоит гейт покрытия чекпоинтом (C-030 R1) и
# инвариант холодной копии (ColdCopyProof). Их оракулы — бэкстоп против «починили .zst,
# сломали prune».
echo
echo "--- регресс M-08 / M-38b / TD-024 ---"
if cargo test -p journal --test red_retention --test red_retention_operator \
     --test red_retention_checkpoint_coverage --test red_compaction --test red_cli_argv \
     -- --test-threads=1 2>&1 | tail -60 | grep -qE 'test result: ok' \
   && ! cargo test -p journal --test red_retention --test red_retention_operator \
        --test red_retention_checkpoint_coverage --test red_compaction --test red_cli_argv \
        -- --test-threads=1 2>&1 | grep -qE 'test result: FAILED'; then
  ok "регресс: C-030 (покрытие чекпоинтом), ColdCopyProof, компакция, argv-контракт — зелёные"
else
  bad "регресс: сломан один из контрактов M-08/M-38b/TD-024 — prune/компакция/argv"
fi

# ── Клиппи по крейту (fail-closed на предупреждения) ────────────────────────────────
echo
if cargo clippy -p journal --all-targets -- -D warnings > /tmp/m40_clippy.log 2>&1; then
  ok "clippy -p journal --all-targets чист"
else
  bad "clippy -p journal не чист (хвост в /tmp/m40_clippy.log)"
  tail -15 /tmp/m40_clippy.log | sed 's/^/      /'
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
