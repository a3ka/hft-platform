#!/usr/bin/env bash
# Проба M-94 (`C-287` B-2; sacred, architect-only): РЕЗУЛЬТАТ каждой ветки разбора идёт из ОБЩЕЙ
# функции `gateway::calc_profile::parse_*` — а не из своей копии, совпадающей сегодня по правилу.
#
# Почему не греп: вызов в комментарии, в мёртвой ветке или только в профиле проходит любой подсчёт
# текста (`C-287` B-2). Здесь — МУТАЦИЯ: в копии дерева в начало каждой из шести общих функций
# вживляется отказ на сторожевом значении (валидном по грамматике) с ключом
# `M94-SENTINEL-<величина>`, затем исполняются сторожевые миры:
#   * `g4` (`crates/gateway-serve/tests/red_m94_single_grammar.rs`) — выдача, без профиля и в
#     профиле, шесть величин;
#   * `w4` (`crates/gateway/tests/red_m94_single_grammar_warmer.rs`) — прогреватель, окружение,
#     флаги и профиль, четыре величины.
# Отказ с ключом сторожа может прийти ТОЛЬКО из вживлённой строки — значит, результат ветки
# вычислен общей функцией.
#
# Контракт сигнатуры (спека §3.2): `pub fn parse_<имя>(s: &str) -> Result<_, ProfileError>`,
# `ProfileError::BadValue { key: String, reason: String }`, `Display` ошибки несёт `key`, и ветки
# без профиля передают текст `ProfileError` в сообщение отказа. Нет функции с такой сигнатурой —
# FAIL (setup), а не пропуск.
#
# Режим «ОСЛАБИТЕЛЬ» (`A-052` §3 (в).1) — вторая половина того же свойства «результат = общая
# функция»: сторожевой ОТКАЗ обязан дойти до результата (выше), сторожевой ПРИЁМ — тоже. В ту же копию
# в `parse_bands` вживляется приём строки `M94-LOOSE-NAN` как `[NaN]` (значение вне грамматики), и
# оракул согласия двух бинарей (`red_m94_two_binary_agreement.sh`) обязан показать СОГЛАСИЕ: если
# у прогревателя поверх общей функции осталась своя проверка NaN, он отвергнет, а выдача примет.
# Негативный контроль — приём `M94-LOOSE-TF` как `604800000` в `parse_timeframe_ms`: расхождение
# ОБЯЗАНО быть, его даёт инвариант библиотеки `validate_selector` (`A-052` §2.6: грамматика
# не может быть слабее инварианта; поведенчески эта ось отличить копию от инварианта не может —
# копии таймфрейма/каденции ловит текстовый шаг verify task12).
#
# Дерево копируется ВМЕСТЕ с незакоммиченными правками (dev проверяет себя до коммита); сборка — в
# своём каталоге, удаляется в конце. Долго: холодная сборка двух тестов.
#
# M94_PROBE_SRC — корень дерева под пробой (по умолчанию — репозиторий скрипта).
# M94_PROBE_FUNCS — список функций для вживления (ТОЛЬКО для проверки самой пробы).
set -uo pipefail

ROOT="${M94_PROBE_SRC:-$(cd "$(dirname "$0")/../.." && pwd)}"
FUNCS="${M94_PROBE_FUNCS:-parse_bands:GATEWAY_BANDS:0.777 parse_timeframe_ms:GATEWAY_TIMEFRAME_MS:500 parse_window_ms:GATEWAY_WINDOW_MS:77000 parse_depth_cadence_ms:GATEWAY_DEPTH_CADENCE_MS:2000 parse_heatmap_window_frac:GATEWAY_HEATMAP_WINDOW:0.0077 parse_vp_bin_width_e8:GATEWAY_VP_BIN_WIDTH_E8:7700000}"

FAIL=0
ok()   { printf 'PASS  %s\n' "$1"; }
nope() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
TREE="$TMP/tree"
mkdir -p "$TREE"
( cd "$ROOT" && tar --exclude=./target --exclude=./.git -cf - . ) | ( cd "$TREE" && tar -xf - ) \
  || { nope "setup: копия дерева не состоялась"; echo "VERDICT: FAIL"; exit 1; }

CP="$TREE/crates/gateway/src/calc_profile.rs"
[ -f "$CP" ] || { nope "setup: нет $CP"; echo "VERDICT: FAIL"; exit 1; }

injected=0
inject() { # <функция> <строка Rust>
  FN="$1" LINE="$2" perl -0777 -i -pe '
    my $fn = $ENV{FN}; my $line = "\n    " . $ENV{LINE};
    $c = s/(pub fn \Q$fn\E\(s: &str\)[^{]*\{)/$1$line/g;
    END { print STDERR "$c\n" }' "$CP" 2>&1 >/dev/null | tail -1
}

for spec in $FUNCS; do
  fn="${spec%%:*}"; rest="${spec#*:}"; key="${rest%%:*}"; val="${rest#*:}"
  n=$(FN="$fn" KEY="$key" VAL="$val" perl -0777 -i -pe '
      my $fn = $ENV{FN}; my $key = $ENV{KEY}; my $val = $ENV{VAL};
      my $line = "\n    if s.trim() == \"$val\" { return Err(ProfileError::BadValue { key: \"M94-SENTINEL-$key\".to_string(), reason: String::new() }); }";
      $c = s/(pub fn \Q$fn\E\(s: &str\)[^{]*\{)/$1$line/g;
      END { print STDERR "$c\n" }' "$CP" 2>&1 >/dev/null | tail -1)
  if [ "${n:-0}" = "1" ]; then
    injected=$((injected + 1))
  else
    nope "setup: \`pub fn $fn(s: &str)\` в calc_profile.rs найдена ${n:-0} раз (нужно ровно 1) — сигнатура не по спеке §3.2"
  fi
done
want=$(wc -w <<<"$FUNCS")
if [ "$injected" -ne "$want" ]; then
  echo "VERDICT: FAIL"; exit 1
fi
ok "setup: сторож вживлён в $injected из $want общих функций"

export CARGO_TARGET_DIR="$TMP/target"
run() { # <метка> <пакет> <тест> <фильтр>
  local label="$1" pkg="$2" test="$3" filt="$4" out
  out=$(cd "$TREE" && cargo test -q -p "$pkg" --test "$test" -- --ignored --exact "$filt" 2>&1)
  local rc=$?
  if [ $rc -eq 0 ] && grep -q '1 passed' <<<"$out"; then ok "$label"
  else nope "$label (exit=$rc)"; printf '%s\n' "$out" | grep -E '^  |M-94|SETUP|error' | head -30; fi
}
run "g4: выдача — сторожевой отказ общей функции доходит до результата без профиля и в профиле" \
  gateway-serve red_m94_single_grammar g4_shared_parser_result_reaches_both_modes
run "w4: прогреватель — сторожевой отказ доходит в окружении, флагах и профиле" \
  gateway red_m94_single_grammar_warmer w4_shared_parser_result_reaches_every_form

# ── режим «ослабитель» ──
if [ -z "${M94_PROBE_FUNCS:-}" ]; then
  n1=$(inject parse_bands 'if s.trim() == "M94-LOOSE-NAN" { return Ok(vec![f64::NAN]); }')
  n2=$(inject parse_timeframe_ms 'if s.trim() == "M94-LOOSE-TF" { return Ok(604_800_000); }')
  if [ "$n1" = "1" ] && [ "$n2" = "1" ]; then
    ok "setup: ослабитель вживлён (parse_bands → NaN, parse_timeframe_ms → 604800000)"
    base_corpus="$TREE/scripts/tests/fixtures/m94_two_binary_corpus.txt"
    { cat "$base_corpus"; echo "GATEWAY_BANDS=M94-LOOSE-NAN"; } > "$TMP/loose_nan.txt"
    { cat "$base_corpus"; echo "GATEWAY_TIMEFRAME_MS=M94-LOOSE-TF"; } > "$TMP/loose_tf.txt"
    out=$(M94_AGREE_TREE="$TREE" M94_AGREE_CORPUS="$TMP/loose_nan.txt" bash "$TREE/scripts/tests/red_m94_two_binary_agreement.sh" 2>&1); rc=$?
    if [ $rc -eq 0 ]; then
      ok "ослабитель NaN: приём общей parse_bands дошёл до ОБОИХ бинарей — своей проверки NaN поверх общей функции нет"
    else
      nope "ослабитель NaN (A-052 §3 (в).1, R-246 B-1): приём общей parse_bands не дошёл до одного из бинарей — у него своя копия правила"
      printf '%s\n' "$out" | grep -E '^  |^FAIL' | head -10
    fi
    out=$(M94_AGREE_TREE="$TREE" M94_AGREE_CORPUS="$TMP/loose_tf.txt" bash "$TREE/scripts/tests/red_m94_two_binary_agreement.sh" 2>&1); rc=$?
    if [ $rc -ne 0 ] && grep -qE '^  legacy +GATEWAY_TIMEFRAME_MS=M94-LOOSE-TF: выдача=ACCEPT прогреватель=REFUSE' <<<"$out"; then
      ok "негативный контроль: ослабленный таймфрейм расходится (держит инвариант validate_selector) — ось согласия чувствительна"
    else
      nope "негативный контроль: ослабленный таймфрейм НЕ дал расхождения (exit=$rc) — оракул согласия ослеп или инвариант снят"
      printf '%s\n' "$out" | tail -6
    fi
  else
    nope "setup: ослабитель не вживлён (parse_bands $n1, parse_timeframe_ms $n2 — нужно по 1)"
  fi
  total=6
else
  total=3
fi

echo "сценариев: $total, провалов: $FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL"; exit 1; fi
