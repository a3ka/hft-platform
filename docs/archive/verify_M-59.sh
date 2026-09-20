#!/usr/bin/env bash
# Acceptance-гейт M-59 — граница ПАМЯТИ per-life анализатора (долг TD-107).
#
# Решение принимается по КОДУ ВОЗВРАТА (gates.md §3). Агрегатор со счётчиком: печатаем все
# нарушения разом, exit 1 при FAIL>0 — иначе первый красный шаг скрыл бы остальные.
#
# ВСТРОЕННЫЙ УРОК 2026-08-05. Проверка вида `grep -q 'test result: ok'` даёт ЗЕЛЁНЫЙ на
# строке `test result: ok. 0 passed; 0 failed; N filtered out` — то есть когда фильтр не
# совпал НИ С ЧЕМ и не исполнено ничего. В этот день так прошли пять «зелёных» прогонов
# мутационного контроля подряд, не выполнив ни одного теста. Поэтому здесь ни один шаг не
# смотрит на слово `ok`: `run_tests` СЧИТАЕТ исполненные тесты и валит шаг, если их меньше
# ожидаемого. Замер по репозиторию в тот же день: 28 из 40 проверок «test result» в наших
# гейтах этого счётчика не имели.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

# declared_tests <файл теста> — сколько #[test] объявлено в файле.
# Порог берётся ИЗ КОДА, а не из головы: первая редакция этого гейта требовала «>=9»
# в red_depth_lifetime.rs, где тестов 6, и давала ЛОЖНЫЙ КРАСНЫЙ. Страж, введённый
# против ложных зелёных, сам нёс невымеренное число — та же болезнь этажом выше.
declared_tests() { grep -c '^#\[test\]' "$1" 2>/dev/null || echo 0; }

# run_oracle <имя-теста-без-.rs> <описание>
# Зелёный ⇔ (а) команда вернула 0, (б) исполнено РОВНО столько, сколько объявлено в
# файле, (в) ни одного failed. Равенство, а не «не меньше»: «>=» пропускает и пустой
# прогон при нулевом пороге, и удаление тестов; равенство ловит и промах фильтра
# («test result: ok. 0 passed» — ложный зелёный, стоивший пяти пустых прогонов
# 2026-08-05), и молчаливую пропажу оракула.
run_oracle() {
  local name="$1" what="$2"
  local file="crates/research-cli/tests/${name}.rs"
  local log="/tmp/m59-${name}.log"
  if [ ! -f "${file}" ]; then fail "${what} — файла ${file} нет"; return 1; fi
  local want; want=$(declared_tests "${file}")
  if [ "${want}" -lt 1 ]; then fail "${what} — в ${file} не объявлено ни одного #[test]"; return 1; fi
  cargo test -p research-cli --test "${name}" >"${log}" 2>&1
  local rc=$? p f
  p=$(grep -hoE '^test result: [a-zA-Z]+\. [0-9]+ passed' "${log}" | awk '{s+=$4} END{print s+0}')
  f=$(grep -hoE '[0-9]+ failed' "${log}" | awk '{s+=$1} END{print s+0}')
  if [ "${rc}" -ne 0 ] || [ "${f}" -ne 0 ]; then
    fail "${what} — исполнено ${p}/${want}, упало ${f}, exit=${rc}"
    grep -E '^(test .* FAILED|thread .* panicked|DV-I-|error)' "${log}" | head -5 | sed 's/^/      ↳ /'
    return 1
  fi
  if [ "${p}" -ne "${want}" ]; then
    fail "${what} — исполнено ${p}, объявлено ${want}: прогон НЕДЕЙСТВИТЕЛЕН (промах фильтра или пропажа теста)"
    return 1
  fi
  pass "${what} — исполнено ${p}/${want}, упало 0"
  return 0
}

echo "--- T0: оракул на месте (sacred, architect-only) ---"
ORACLE=crates/research-cli/tests/red_lifetime_memory_bounded.rs
if [ -f "${ORACLE}" ] && grep -q 'fn dv_i_15_lifetime_memory_bounded' "${ORACLE}"; then
  pass "T0 ${ORACLE}"
else
  fail "T0 ${ORACLE} отсутствует или не содержит DV-I-15"
fi
# Ровно один тест в файле — иначе замер глобального счётчика недействителен (см. шапку оракула).
NT=$(grep -c '^#\[test\]' "${ORACLE}" 2>/dev/null || echo 0)
[ "${NT}" -eq 1 ] && pass "T0 ровно один #[test] в файле (изоляция замера аллокаций)" \
                  || fail "T0 в файле ${NT} тестов — параллельный прогон испортит счётчик памяти"

echo "--- T1/T2/T2b: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
cargo build --workspace >/dev/null 2>&1 && pass "T1 build --workspace" || fail "T1 build --workspace"
cargo clippy --workspace --all-targets --all-features -- -D warnings >/tmp/m59-clippy.log 2>&1 \
  && pass "T2 clippy" || { fail "T2 clippy"; tail -5 /tmp/m59-clippy.log | sed 's/^/      ↳ /'; }
cargo fmt --all -- --check >/dev/null 2>&1 && pass "T2b fmt --check" || fail "T2b fmt --check"
# F-3 (R-038): заголовок шага ЗАЯВЛЯЛ паритет с CI-job «fmt+clippy+test», а `cargo test --all`
# в скрипте отсутствовал. Гейт, который зеленее CI, — не гейт (gates.md §3). Повтор R-033 F-4:
# ту же дыру уже находили на M-58, и она воспроизвелась здесь.
cargo test --all >/tmp/m59-testall.log 2>&1 \
  && pass "T2c cargo test --all" \
  || { fail "T2c cargo test --all"; grep -E "^test .* FAILED|^error" /tmp/m59-testall.log | head -5 | sed "s/^/      ↳ /"; }

echo "--- T3: ГЛАВНОЕ — DV-I-15, память не растёт с числом ЖИЗНЕЙ ---"
run_oracle red_lifetime_memory_bounded "T3 DV-I-15"
# ЗАМЕР ПЕЧАТАЕТСЯ ИЛИ ШАГ КРАСНЕЕТ (`R-076` N-4). Прежняя редакция грепала лог `run_oracle`,
# но тот зовёт `cargo test` БЕЗ `--nocapture`, а харнесс отдаёт stdout только у УПАВШИХ
# тестов. В зелёном прогоне грепу нечего было найти, и строка печаталась ПУСТОЙ — шаг
# выглядел исполненным, не исполнившись. Замер `R-076`: `grep -c 'DV-I-15:'` = 0.
# Молчащая строка хуже отсутствующей: она создаёт видимость измерения. Поэтому отдельный
# прогон с `--nocapture` и fail-closed на отсутствие числа.
cargo test -p research-cli --test red_lifetime_memory_bounded -- --nocapture \
  > /tmp/m59-dvi15-measure.log 2>&1 || true
M59_MEASURE=$(grep -hoE 'DV-I-15: .*' /tmp/m59-dvi15-measure.log 2>/dev/null | head -1)
if [ -n "${M59_MEASURE}" ]; then
  echo "      ЗАМЕР: ${M59_MEASURE}"
else
  fail "T3 ЗАМЕР DV-I-15 НЕ ПОЛУЧЕН — оракул не напечатал число. Шаг обязан предъявлять \
величину, на которой стоит граница памяти, а не молчать: пустая строка неотличима от \
исполненного замера (R-076 N-4)"
fi

echo "--- T4: РЕГРЕСС — DV-I-1..14 остаются зелёными ---"
# Состав ИЗМЕРЕН грепом по tests/, а не взят по памяти: первая редакция гоняла один
# файл и заявляла в тексте «DV-I-1..9», тогда как DV-I-6 живёт в red_orderflow_faith,
# DV-I-7/8 — в red_depth_scale, DV-I-9 — в red_depth_band_3060. Шаг ВРАЛ о том, что
# меряет: три оракула из девяти не запускались вовсе (testing.md «оракул обязан мерить
# ТО, ЧТО ОБЕЩАЕТ»).
run_oracle red_depth_lifetime          "T4 DV-I-1..5 (жизненный цикл)"
run_oracle red_depth_lifetime_perlife  "T4 DV-I-10..14 (per-life)"
run_oracle red_orderflow_faith         "T4 DV-I-6 (достоверность потока)"
run_oracle red_depth_scale             "T4 DV-I-7,8 (масштаб/время)"
run_oracle red_depth_band_3060         "T4 DV-I-9 (полоса 30-60%)"

echo "--- T5: ЧИСЛА прогона не поехали (публичный контракт и артефакт замера) ---"
# Фикс обязан менять РАСХОД, а не РЕЗУЛЬТАТ. Артефакт под founder-решением П-011 —
# research/data-quality/m58-rerun-segment78.txt; расхождение чисел = смена семантики.
ART=research/data-quality/m58-rerun-segment78.txt
if [ ! -f "${ART}" ]; then
  fail "T5 артефакт замера ${ART} отсутствует — сверять не с чем"
elif [ -z "${M59_JOURNAL:-}" ]; then
  # Fail-closed: «нет журнала» не значит «проверять нечего». Пересъёмка требует
  # M59_JOURNAL=<путь к копии сегмента 78>; молчаливый пропуск запрещён.
  fail "T5 пересъёмка НЕ выполнена: задай M59_JOURNAL=<путь к журналу segment 78>. \
Пропуск этой проверки означал бы, что фикс мог изменить ЧИСЛА, а не только расход"
elif [ ! -d "${M59_JOURNAL}" ]; then
  # ФОРМА ПУТИ (два независимых прогона tester'а 14.08). `examples/depth_lifetime.rs`
  # зовёт `journal::stream(&dir, EpochFilter::OwnCaptureOnly)` — тот принимает КАТАЛОГ
  # сегментов. Передача ФАЙЛА `segment-00000078.jrnl` роняла пример паникой
  # `NotADirectory` (exit=101), stdout оставался пустым, и шаг рапортовал «ЧИСЛА
  # РАЗОШЛИСЬ» — обвиняя семантику фикса вместо собственного входа. Прежняя фикстура
  # `/tmp/m33-journal` была каталогом, поэтому дефект спал. Теперь форма названа.
  fail "T5 M59_JOURNAL='${M59_JOURNAL}' — не КАТАЛОГ. Нужен каталог сегментов \
(например /home/nous/fixtures/m59-segment78), а не путь до файла .jrnl: \
depth_lifetime зовёт journal::stream(dir), и на файле он паникует NotADirectory"
else
  # ВЫХОД ПРИМЕРА ПРОВЕРЯЕТСЯ ДО ДИФФА, stderr СОХРАНЯЕТСЯ (`gates.md` §3: решение
  # по КОДУ ВОЗВРАТА, не по тексту; `testing.md`: гейт обязан падать против
  # несостоявшегося SETUP, а не выдавать его за содержательную находку).
  # Прежняя редакция глотала stderr в /dev/null и не смотрела на код возврата —
  # молчание примера превращалось в «числа разошлись». Класс `TD-136`.
  RERUN_RC=0
  cargo run --release -p research-cli --example depth_lifetime -- "${M59_JOURNAL}" \
    >/tmp/m59-rerun.txt 2>/tmp/m59-rerun.stderr || RERUN_RC=$?
  if [ "${RERUN_RC}" -ne 0 ]; then
    fail "T5 SETUP НЕ СОСТОЯЛСЯ: пример depth_lifetime завершился с exit=${RERUN_RC} — \
числа НЕ сверялись. Это отказ инструмента, а не расхождение семантики"
    sed 's/^/      ↳ /' /tmp/m59-rerun.stderr | tail -6
  elif [ ! -s /tmp/m59-rerun.txt ]; then
    fail "T5 SETUP НЕ СОСТОЯЛСЯ: пример отработал (exit=0), но выдача ПУСТА — сверять нечего"
  else
  # Сравниваются ЧИСЛА СТРОК ДАННЫХ, а не все числа файла. Первая редакция грепала
  # все цифры подряд и потому сравнивала ПОДПИСИ: артефакт снят под префиксом [M-32]
  # и путём /tmp/m33-journal, откуда в него попали числа 32, 33, 2026, 07. После
  # TD-108 подписи стали [M-58], эти числа исчезли — и гейт объявил «семантика
  # изменилась», хотя изменились ярлыки. Оракул обязан мерить предмет, а не оформление.
  data_nums() { grep -E '^(bid|ask|NEAR|FAR|orderflow)' "$1" | grep -oE '[0-9]+(\.[0-9]+)?' | tr '\n' ' '; }
  if [ -z "$(data_nums "${ART}")" ]; then
    fail "T5 в артефакте ${ART} не нашлось строк данных (bid/ask/NEAR/FAR/orderflow) — сверять нечего"
  elif diff <(data_nums "${ART}") <(data_nums /tmp/m59-rerun.txt) >/dev/null 2>&1; then
    pass "T5 числа пересъёмки идентичны артефакту (изменился расход, не результат)"
  else
    fail "T5 ЧИСЛА РАЗОШЛИСЬ с ${ART} — фикс изменил семантику, а не только память"
    diff <(data_nums "${ART}" | tr ' ' '\n') <(data_nums /tmp/m59-rerun.txt | tr ' ' '\n') | head -8 | sed 's/^/      ↳ /'
  fi
  fi
fi

echo "--- T6: публичный контракт не тронут ---"
SRC=crates/research-cli/src/depth_lifetime.rs
MISS=0
for f in lives_born lives_cancelled lives_frozen lives_censored; do
  grep -q "pub ${f}: u64" "${SRC}" || { echo "      ↳ пропало поле ${f}"; MISS=$((MISS+1)); }
done
[ "${MISS}" -eq 0 ] && pass "T6 поля BandReport.lives_* на месте" \
                    || fail "T6 публичный контракт изменён: пропало ${MISS} поле(й)"
grep -q 'crates/contracts' <(git diff --name-only origin/main...HEAD 2>/dev/null) \
  && fail "T6 затронут crates/contracts — M-59 не T1-milestone" \
  || pass "T6 crates/contracts не тронут"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений)"
  exit 1
fi
echo "VERDICT: PASS"
