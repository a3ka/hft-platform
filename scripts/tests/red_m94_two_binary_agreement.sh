#!/usr/bin/env bash
# Проба M-94 (`A-052` §3 (б); sacred, architect-only): ОРАКУЛ СОГЛАСИЯ ДВУХ БИНАРЕЙ. Всё, что
# принимает старт выдачи (`gateway_serve::serve_config_from_env`), принимает и прогреватель
# (прод-бинарь `gateway-checkpoint` на пустом журнале), и наоборот — в обоих режимах (без профиля и
# профиль). Пиннит ось «грамматика старта ↔ инвариант библиотеки `validate_selector`»: иначе выдача
# поднимается здоровой, а слепки не готовятся / библиотека отвергает каждое подключение (класс
# `TD-227`). Любое одностороннее изменение правила наблюдаемо (мутация `R-246`: ослабить общий
# `parse_timeframe_ms` ⇒ расхождение на `604800000`, `A-052` §2.3).
#
# Как: один корпус (`scripts/tests/fixtures/m94_two_binary_corpus.txt`) подаётся обеим половинам —
# `g5` (выдача, `crates/gateway-serve/tests/red_m94_single_grammar.rs`) и `w5` (прогреватель,
# `crates/gateway/tests/red_m94_single_grammar_warmer.rs`); каждая — `#[ignore]`, пишет исходы;
# проба сравнивает. Почему проба, а не один Rust-тест: путь к бинарю прогревателя
# (`CARGO_BIN_EXE_gateway-checkpoint`) доступен только тестам крейта `gateway`, а
# `serve_config_from_env` — только зависящим от `gateway-serve`; обе половины собираются своим
# `cargo test`, устаревший бинарь исключён по построению.
#
# Стражи: setup — активный профиль принят обоими в обоих режимах; число исходов каждой половины =
# 2 × число случаев корпуса; давление — по каждой из четырёх величин есть принятое и отвергнутое.
#
# M94_AGREE_TREE   — корень дерева под пробой (по умолчанию — репозиторий скрипта); проба
#                    `red_m94_shared_grammar_probe.sh` в режиме «ослабитель» зовёт эту с копией.
# M94_AGREE_CORPUS — корпус (по умолчанию — fixtures выше).
# CARGO_TARGET_DIR — наследуется (проба-ослабитель переиспользует свою сборку).
set -uo pipefail

TREE="${M94_AGREE_TREE:-$(cd "$(dirname "$0")/../.." && pwd)}"
CORPUS="${M94_AGREE_CORPUS:-$TREE/scripts/tests/fixtures/m94_two_binary_corpus.txt}"

FAIL=0
ok()   { printf 'PASS  %s\n' "$1"; }
nope() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }

[ -f "$CORPUS" ] || { nope "setup: нет корпуса $CORPUS"; echo "VERDICT: FAIL"; exit 1; }
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

half() { # <пакет> <тест> <фильтр> <вывод>
  local out
  out=$(cd "$TREE" && M94_AGREE_CORPUS="$CORPUS" M94_AGREE_OUT="$4" \
        cargo test -q -p "$1" --test "$2" -- --ignored --exact "$3" 2>&1)
  if [ $? -ne 0 ] || [ ! -s "$4" ]; then
    nope "setup: половина $1/$3 не дала исходов"; printf '%s\n' "$out" | tail -15; return 1
  fi
}
half gateway-serve red_m94_single_grammar g5_dump_outcomes_for_two_binary_agreement "$TMP/serve.tsv" || { echo "VERDICT: FAIL"; exit 1; }
half gateway red_m94_single_grammar_warmer w5_dump_outcomes_for_two_binary_agreement "$TMP/warmer.tsv" || { echo "VERDICT: FAIL"; exit 1; }

python3 - "$CORPUS" "$TMP/serve.tsv" "$TMP/warmer.tsv" <<'PY'
import sys
corpus, sp, wp = sys.argv[1:4]
cases = [l.rstrip("\n") for l in open(corpus, encoding="utf-8") if l.strip() and not l.startswith("#")]
def load(p):
    d = {}
    for l in open(p, encoding="utf-8"):
        l = l.rstrip("\n")
        if not l:
            continue
        case, mode, r = l.split("\t")
        d[(case, mode)] = r
    return d
s, w = load(sp), load(wp)
bad = []
want = 2 * len(cases)
if len(s) != want or len(w) != want:
    print(f"FAIL  setup: исходов выдача={len(s)} прогреватель={len(w)}, ждали {want} (2 × {len(cases)} случаев)")
    sys.exit(2)
for mode in ("legacy", "profile"):
    if s.get(("-", mode)) != "ACCEPT" or w.get(("-", mode)) != "ACCEPT":
        print(f"FAIL  setup: активный профиль не принят обоими в режиме {mode}: выдача={s.get(('-', mode))} прогреватель={w.get(('-', mode))}")
        sys.exit(2)
press = {}
for case in cases:
    keys = {p.split("=", 1)[0] for p in case.split(";") if p and p != "-"}
    for mode in ("legacy", "profile"):
        a, b = s[(case, mode)], w[(case, mode)]
        if len(keys) == 1:
            k = next(iter(keys))
            acc, ref = press.get(k, (0, 0))
            press[k] = (acc + (a == "ACCEPT"), ref + (a == "REFUSE"))
        if a != b:
            bad.append(f"  {mode:8} {case}: выдача={a} прогреватель={b}")
for k in ("GATEWAY_BANDS", "GATEWAY_TIMEFRAME_MS", "GATEWAY_WINDOW_MS", "GATEWAY_DEPTH_CADENCE_MS"):
    acc, ref = press.get(k, (0, 0))
    if acc < 1 or ref < 1:
        print(f"FAIL  setup: корпус не давит на {k} (принято {acc}, отвергнуто {ref})")
        sys.exit(2)
print(f"сравнений: {2 * len(cases)}, расхождений: {len(bad)}")
if bad:
    print("FAIL  согласие двух бинарей (A-052 §3 (б)): правило величины судит по-разному выдача и прогреватель")
    print("\n".join(bad))
    sys.exit(1)
print("PASS  согласие двух бинарей: выдача и прогреватель одинаково принимают/отвергают каждый случай корпуса в обоих режимах")
PY
rc=$?
[ $rc -eq 0 ] || FAIL=$((FAIL + 1))
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL"; exit 1; fi
