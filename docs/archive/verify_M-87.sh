#!/usr/bin/env bash
# verify_M-87.sh — acceptance-гейт milestone'а M-87 «предохранитель выдачи».
# Спека: milestones/M-87-serving-circuit-breaker.md
#
# Решение по КОДУ ВОЗВРАТА (`gates.md` §3). Агрегатор с FAIL-счётчиком: гейт обязан
# перечислить ВСЕ провалы, а не умереть на первом.
#
# Базовая линия снимается КРАСНОЙ до работы dev'а и предъявляется в Handoff.

set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP  %s\n' "$1"; }

GW=crates/gateway/src/lib.rs
GS=crates/gateway-serve/src
SPEC=milestones/M-87-serving-circuit-breaker.md

# ─────────────── задача 1 — контракт исходов ───────────────
if [ -f "$GS/admission.rs" ] \
   && grep -qE 'pub enum ServingOutcome' "$GS/admission.rs" \
   && grep -qE 'Ready' "$GS/admission.rs" && grep -qE 'Warming' "$GS/admission.rs" \
   && grep -qE 'NotReady' "$GS/admission.rs" && grep -qE 'Unsupported' "$GS/admission.rs" \
   && grep -qE 'Overloaded' "$GS/admission.rs"; then
  pass "task1: пять исходов объявлены в admission.rs"
else
  fail "task1: нет модуля admission.rs с пятью исходами ServingOutcome"
fi

# ─────────────── задача 2 — готовность судится ТАМ, ГДЕ ЖИВЁТ ПРЕДОХРАНИТЕЛЬ ───────────────
# A-037 D-1 следствие 1: файл `crates/gateway/tests/red_m87_cold_path_reads_nothing.rs` ИЗЪЯТ.
# Он пиннил на ОБЩЕЙ функции LiveReducer::resume поведение, противоположное двум другим
# sacred-оракулам того же корпуса на той же функции — набор был невыполним против самого
# себя. Четыре состояния слепка переехали в транспорт: юнит-оракулы `readiness()` в
# red_m87_admission.rs и WS-оракулы над журналом-ловушкой в red_m87_entrypoint.rs.
if [ -f crates/gateway/tests/red_m87_cold_path_reads_nothing.rs ]; then
  fail "task2: изъятый файл red_m87_cold_path_reads_nothing.rs вернулся — набор снова противоречит сам себе (A-037 D-1)"
else
  pass "task2: библиотечный оракул изъят; готовность судится в транспорте"
fi

# ─────────────── задачи 1+3+4+7 — форма допуска, политика, бюджет, секрет ───────────────
AD_OUT=$(cargo test -p gateway-serve --test red_m87_admission 2>&1)
AD_RC=$?
AD_LINE=$(printf '%s\n' "$AD_OUT" | grep -E '^test result' | tail -1)
if [ $AD_RC -eq 0 ]; then
  pass "task1+3+4+7: red_m87_admission — ${AD_LINE:-GREEN}"
else
  fail "task1+3+4+7: red_m87_admission КРАСЕН — ${AD_LINE:-компиляция}"
  printf '%s\n' "$AD_OUT" | grep -E '^(error|thread |assertion)' | head -20
fi

# ─────────────── задачи 2+3+5+6+8 — ОРАКУЛ ТОЧКИ ВХОДА (C-234 R2/R3) ───────────────
# Несущая проверка поставки: предохранитель судится на РЕАЛЬНОМ публичном пути
# (WS entrypoint), а не рядом с ним. Без неё GREEN достижим без подключения допуска.
EP_OUT=$(cargo test -p gateway-serve --test red_m87_entrypoint 2>&1)
EP_RC=$?
EP_LINE=$(printf '%s\n' "$EP_OUT" | grep -E '^test result' | tail -1)
if [ $EP_RC -eq 0 ]; then
  pass "task2+3+5+6+8: red_m87_entrypoint — ${EP_LINE:-GREEN}"
else
  fail "task2+3+5+6+8: red_m87_entrypoint КРАСЕН — ${EP_LINE:-компиляция}"
  printf '%s\n' "$EP_OUT" | grep -E '^(error|thread |assertion)' | head -15
fi

# ─────────────── задача 4 — счётчик ПРОЧИТАННЫХ БАЙТ существует ───────────────
if grep -qE '^\s*pub payload_bytes_read: u64,' "$GW"; then
  pass "task4: ReadStats несёт payload_bytes_read"
else
  fail "task4: в ReadStats нет payload_bytes_read — бюджет по байтам мерить нечем"
fi

# ─────────────── задача 5 — ограничитель параллелизма существует ───────────────
SEM=$(grep -rl 'Semaphore\|max_concurrent_serves' "$GS" 2>/dev/null | wc -l)
if [ "$SEM" -gt 0 ]; then
  pass "task5: ограничитель параллелизма присутствует (файлов: $SEM)"
else
  fail "task5: ограничителя параллелизма нет ни в одном файле $GS (найдено: $SEM)"
fi

# ─────────────── задача 6 — счётчики выдачи ───────────────
if [ -f "$GS/metrics.rs" ] && grep -qE 'refusals_supported' "$GS/metrics.rs" \
   && grep -qE 'refusals_unsupported' "$GS/metrics.rs"; then
  pass "task6: счётчики выдачи разделяют поддержанные и неподдержанные отказы"
else
  fail "task6: нет metrics.rs с раздельными счётчиками отказов"
fi

# ─────────────── задача 7 — единая трактовка секрета ───────────────
if grep -qE 'pub fn key_material' "$GS/lib.rs" \
   && grep -qE 'key_material' "$GS/bin/wsprobe.rs"; then
  pass "task7: секрет трактуется ОДНОЙ функцией, её зовут обе стороны"
else
  fail "task7: key_material отсутствует либо зонд её не зовёт — трактовок по-прежнему две (TD-207)"
fi

# ─────────── `A-039` — РЕЕСТР ОБЩЕГО ПОРОГА: полнота предъявляется ПРОГОНОМ ───────────
# Решение арбитра A-039 (§Q2): прозаическая таблица потребителей ошибалась тремя способами
# и дважды протухла. Доказательство переехало из прозы в прогон — и, в отличие от прочих
# шагов этого гейта, работает УЖЕ СЕЙЧАС: бинарь реестра не импортирует admission и зеленеет
# на COMPILE-RED ветке. Доказательство, которое нельзя прогнать сегодня, этот класс не
# закрывает — три круга подряд провалились ровно на «утверждении без прогона».
RG_OUT=$(cargo test -p gateway-serve --test red_m87_registry 2>&1)
RG_RC=$?
RG_LINE=$(printf '%s\n' "$RG_OUT" | grep -E '^test result' | tail -1)
if [ $RG_RC -eq 0 ]; then
  pass "A-039: реестр общего порога — ${RG_LINE:-GREEN}"
else
  fail "A-039: реестр общего порога КРАСЕН — ${RG_LINE:-компиляция}"
  printf '%s\n' "$RG_OUT" | grep -E '^(thread |сценарии файла|строки реестра)' | head -6
fi

# Канарейки единственности источника — ПО ФОРМЕ вызова, не по слову в комментарии.
# (1) дописка в файле предмета идёт ТОЛЬКО через хелпер реестра: литерал длины дописки
#     вернул бы второе место, где число могло бы разойтись с реестром.
LIT=$(grep -cE 'for i in 0\.\.[0-9_]+i64' crates/gateway-serve/tests/red_m87_entrypoint.rs || true)
if [ "$LIT" -eq 0 ]; then
  pass "A-039: дописок-литералов в предмете нет — число живёт в реестре и только там"
else
  fail "A-039: в предмете $LIT литерал(ов) дописки — реестр перестал быть единственным источником"
fi
# (2) числа политики берутся из источника, а не повторяются в хелпере.
SRC_OK=$(grep -cE 'max_tail_events: m87_registry::MAX_TAIL_EVENTS|expected_warmup_events: m87_registry::EXPECTED_WARMUP_EVENTS' crates/gateway-serve/tests/red_m87_entrypoint.rs || true)
if [ "$SRC_OK" -eq 2 ]; then
  pass "A-039: политика предмета берёт оба числа из источника"
else
  fail "A-039: политика предмета берёт из источника $SRC_OK из 2 чисел — второе место расхождения вернулось"
fi

# ─────── `A-040`: ИСТИНА ПОЛНОТЫ — ПЕРЕЧЕНЬ ХАРНЕССА, А НЕ РАЗБОР ИСХОДНИКА ───────
# Решение арбитра A-040 §Q3 п. 1. Три круга подряд находки шли в РАЗБОР ТЕКСТА: чёрный
# список форм не сходится (cfg_attr → quickcheck → следующая). Истина «что является
# тестом» принадлежит харнессу, и спросить её можно одной командой.
#
# `--features testing` ОБЯЗАТЕЛЕН: c4_slot_lifecycle стоит под cfg(feature = "testing"),
# без фичи он не в перечне, и его строка реестра выглядела бы мёртвой.
#
# Шаг КРАСЕН СЕГОДНЯ — в том же ряду, что прочие COMPILE-RED-шаги (§14.1quinquies-ter):
# предмет не собирается до задач 12-13, значит и перечня у харнесса нет. После задачи 12
# он ЗАМЕЩАЕТ парсер как основание полноты.
HL_OUT=$(cargo test -p gateway-serve --features testing --test red_m87_entrypoint -- --list 2>&1)
HL_RC=$?
if [ $HL_RC -ne 0 ]; then
  fail "A-040: перечень харнесса недоступен — предмет не собирается (ожидаемо до задач 12-13)"
else
  HARNESS=$(printf '%s\n' "$HL_OUT" | sed -n 's/^\([a-z0-9_]*\): test$/\1/p' | sort -u)
  # Реестр тоже спрашивается у ПРОГРАММЫ, а не разбирается как текст. Первая редакция
  # вынимала строки `sed`'ом (`^ *"\(...\)",$`) — второй парсер, ровно тот класс, который
  # арбитр `A-040` и велел убрать, и он немедленно ошибся: однострочная запись
  # `("name", &[]),` под выражение не попадала, реестр отдавался короче на строку, и
  # биекция краснела на ИСПРАВНОМ предмете.
  REG_OUT=$(cargo test -p gateway-serve --test red_m87_registry -- --exact --nocapture \
            r8_registry_rows_are_printed_for_the_gate 2>&1)
  REG_RC=$?
  REG=$(printf '%s\n' "$REG_OUT" | sed -n 's/^M87-REGISTRY-ROW: \([a-z0-9_]*\)$/\1/p' | sort -u)
  if [ $REG_RC -ne 0 ] || [ -z "$REG" ]; then
    fail "A-040: выдача реестра недоступна (rc=$REG_RC) — сводить биекцию не с чем"
    printf '%s\n' "$REG_OUT" | grep -E '^(error|thread )' | head -4
  else
    ONLY_H=$(comm -23 <(printf '%s\n' "$HARNESS") <(printf '%s\n' "$REG"))
    ONLY_R=$(comm -13 <(printf '%s\n' "$HARNESS") <(printf '%s\n' "$REG"))
    if [ -z "$ONLY_H" ] && [ -z "$ONLY_R" ] && [ -n "$HARNESS" ]; then
      pass "A-040: биекция харнесса — перечни совпали ($(printf '%s\n' "$HARNESS" | wc -l) сценариев)"
    else
      fail "A-040: биекция харнесса НЕ сошлась"
      [ -n "$ONLY_H" ] && printf '      только у харнесса: %s\n' "$(echo $ONLY_H)"
      [ -n "$ONLY_R" ] && printf '      только в реестре:  %s\n' "$(echo $ONLY_R)"
    fi
  fi
fi

# ─────────── круг `R-196` задача 11 — НАПРАВЛЕНИЕ трактовки секрета (B3) ───────────
# Прежний шаг task7 проверял ЕДИНСТВО трактовки («одна функция, её зовут обе стороны») и
# был зелен, когда обе стороны свели к НЕВЕРНОЙ. Единство — не истинность. Направление
# судится оракулом, который воспроизводит ВНЕШНЕГО подписывателя.
SEC_OUT=$(cargo test -p gateway-serve --test red_m87_secret_form 2>&1)
SEC_RC=$?
SEC_LINE=$(printf '%s\n' "$SEC_OUT" | grep -E '^test result' | tail -1)
if [ $SEC_RC -eq 0 ]; then
  pass "task11: направление трактовки секрета — ${SEC_LINE:-GREEN}"
else
  fail "task11: red_m87_secret_form КРАСЕН — ${SEC_LINE:-компиляция}"
  printf '%s\n' "$SEC_OUT" | grep -E '^(thread |R-196)' | head -6
fi

# ─────────── круг `R-196` задача 12 — порог свежести против каденции прогрева (B2) ───────────
ST_OUT=$(cargo test -p gateway-serve --test red_m87_staleness_budget 2>&1)
ST_RC=$?
ST_LINE=$(printf '%s\n' "$ST_OUT" | grep -E '^test result' | tail -1)
if [ $ST_RC -eq 0 ]; then
  pass "task12: порог свежести переживает измеренную каденцию — ${ST_LINE:-GREEN}"
else
  fail "task12: red_m87_staleness_budget КРАСЕН — ${ST_LINE:-компиляция}"
  printf '%s\n' "$ST_OUT" | grep -E '^(thread |R-196|error\[)' | head -6
fi

# ─────────── круг `R-196` задача 13 — оракул под флагом ОБЯЗАН БЫТЬ ПРОГНАН ───────────
# Названный предел корпуса, найденный этим кругом: CI гоняет `cargo test --all` БЕЗ
# `--all-features`, поэтому всё под `#[cfg(feature = "testing")]` в CI не исполняется
# ВООБЩЕ — оно только компилируется линтером. Так уже живёт `O-12` из M-65. Пока это не
# закрыто на уровне корпуса (долг ревьюера), milestone закрывает дыру у себя: гейт зовёт
# набор точки входа ЯВНО с флагом. Без этого шага C4 не исполняется никогда и является
# украшением, а не гейтом.
# Различитель подменного пути (`C-245` R2) — ЗЕЛЁНЫЙ контроль, работающий уже сегодня:
# он краснеет, если удержание ADD-работы завязано на канал периодического pump'а M-65.
# Проверен мутацией: с подмешанным рандеву на голом id — FAILED, после возврата кода — ok.
RZ_OUT=$(cargo test -p gateway-serve --features testing --test red_m87_rendezvous_discriminator 2>&1)
RZ_RC=$?
RZ_LINE=$(printf '%s\n' "$RZ_OUT" | grep -E '^test result' | tail -1)
if [ $RZ_RC -eq 0 ]; then
  pass "task13: различитель подменного пути ЗЕЛЁН — ${RZ_LINE:-GREEN}"
else
  fail "task13: различитель подменного пути КРАСЕН — удержание завязано на чужой канал (${RZ_LINE:-компиляция})"
  printf '%s\n' "$RZ_OUT" | grep -E '^(thread |C-245)' | head -5
fi

EPF_OUT=$(cargo test -p gateway-serve --features testing --test red_m87_entrypoint 2>&1)
EPF_RC=$?
EPF_LINE=$(printf '%s\n' "$EPF_OUT" | grep -E '^test result' | tail -1)
if [ $EPF_RC -eq 0 ]; then
  pass "task13: набор точки входа ПОД ФЛАГОМ testing — ${EPF_LINE:-GREEN}"
else
  fail "task13: red_m87_entrypoint под --features testing КРАСЕН — ${EPF_LINE:-компиляция}"
  printf '%s\n' "$EPF_OUT" | grep -E '^(thread |error\[|setup-страж)' | head -8
fi

# ─────────────── задача 8 — свежесть четырьмя позициями ───────────────
FRESH=$(grep -rcE 'freshness|свежест' "$GS"/*.rs 2>/dev/null | awk -F: '{s+=$2} END{print s+0}')
if [ "$FRESH" -gt 0 ]; then
  pass "task8: свежесть представлена в коде выдачи (совпадений: $FRESH)"
else
  fail "task8: свежести нет ни в одном файле выдачи (совпадений: $FRESH)"
fi

# ─────────────── задача 9 — лимиты У КАЖДОГО ИЗ ТРЁХ КЛАССОВ СЕРВИСА ───────────────
# C-234 R4: прежняя проверка считала строки и зеленела от ЛЮБЫХ четырёх — она не связывала
# лимит с сервисом. Теперь каждый из трёх поимённо названных сервисов обязан нести И
# ограничение процессора, И ограничение памяти. Пропуск одного сервиса роняет шаг.
lim_for_service() {
  # A-037 У-8: считаются ТОЛЬКО ключи УРОВНЯ СЕРВИСА (отступ ровно 4 пробела) либо
  # `deploy.resources.limits.*`. Блоки `labels:`/`environment:` исключены — иначе шаг
  # зеленеет от строки `cpus: "2"` внутри меток, что арбитр и воспроизвёл (мутация M-9a).
  awk -v svc="  $1:" '
    $0 == svc { inblock = 1; next }
    inblock && /^  [a-z][a-z0-9_-]*:/ { inblock = 0 }
    inblock && /^    [a-z_]+:/ { sub(/:.*/, ""); sub(/^ +/, ""); cur = $0 }
    inblock && /^    (cpus|mem_limit|memory):/ { n++; next }
    inblock && /^ +(cpus|memory):/ && cur == "deploy" { n++; next }
    END { print n + 0 }
  ' docker-compose.yml
}
LIM_MISSING=""
for svc in gateway-serve recorder gateway-checkpoint; do
  n=$(lim_for_service "$svc")
  [ "$n" -ge 2 ] || LIM_MISSING="$LIM_MISSING $svc($n)"
done
if [ -z "$LIM_MISSING" ]; then
  pass "task9: процессор И память ограничены у ВСЕХ трёх классов (выдача, запись, прогреватель)"
else
  fail "task9: нет пары лимитов уровня сервиса у:$LIM_MISSING — ключи внутри labels/environment не считаются"
fi
skip "task9: ФАКТИЧЕСКИ применённые лимиты, запас для recorder'а и поведение НА лимите снимаются на проде (docker inspect) — шаг деплой-гейта §8; замер 2026-09-21 дал NanoCpus=0 Memory=0 при живом описании сервисов"

# ─────────────── задача 10 — решение по КАЖДОМУ файлу корпуса ───────────────
# C-234 R4: прежняя проверка требовала лишь исчезновения одной фразы-плейсхолдера и не
# ловила ПРОПУСК решения. Теперь состав снимается ТЕМИ ЖЕ командами, что в §16 спеки, и
# каждый найденный файл обязан быть НАЗВАН в таблице решений §16.1.
CORPUS=$( { grep -rl 'checkpoint_dir: None' crates/gateway-serve/tests/ 2>/dev/null;
            grep -rl 'full_replay\|resume_without_checkpoint\|rebuilds' crates/gateway/tests/ 2>/dev/null; } \
          | xargs -r -n1 basename | sort -u )
DECISIONS=$(awk '/^### 16.1. Решения по изменённым ожиданиям/,0' "$SPEC")
MISSING=""
for f in $CORPUS; do
  # A-037 У-8: решение ищется В ТОЙ ЖЕ строке, что имя файла, и обязано нести токен.
  # Прежний предикат принимал строку с именем и ПУСТЫМ решением (ложно-зелёный, мутация M-10b).
  printf '%s' "$DECISIONS" | grep -- "$f" | grep -qE 'ПЕРЕНОСИТСЯ|НЕ ЗАТРАГИВАЕТСЯ|УДАЛЯЕТСЯ' \
    || MISSING="$MISSING $f"
done
CORPUS_N=$(printf '%s\n' $CORPUS | grep -c . || true)
# Два ТЕСТ-уровневых решения (A-037 D-4) — по имени теста, не файла.
for t in resume_without_checkpoint_reports_full_replay o5_broken_checkpoint; do
  printf '%s' "$DECISIONS" | grep -q -- "$t" || MISSING="$MISSING тест:$t"
done
if [ -n "$CORPUS" ] && [ -z "$MISSING" ]; then
  pass "task10: решение с токеном записано по каждому из $CORPUS_N файлов и по двум тестам D-4"
else
  fail "task10: нет решения (или токена ПЕРЕНОСИТСЯ/НЕ ЗАТРАГИВАЕТСЯ/УДАЛЯЕТСЯ) для:$MISSING"
fi

# ─────────── A-037 D-1 — библиотека меняется ТОЛЬКО АДДИТИВНО ───────────
# Две команды решения, вынесенные в гейт: (а) корпус библиотеки не тронут, (б) он зелен.
# Это и есть проверяемая граница с A-033: не-аддитивный сдвиг библиотеки краснит
# существующий sacred-тест, который dev править не вправе.
MB87=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
if [ -z "$MB87" ]; then
  fail "D-1(а): merge-base не вычислен — аддитивность не предъявлена"
else
  NONADD=$(git diff --name-status "$MB87"..HEAD -- crates/gateway/tests/ \
           | grep -vE '^A[[:space:]]+crates/gateway/tests/red_m87_' || true)
  if [ -z "$NONADD" ]; then
    pass "D-1(а): библиотечный корпус тронут только добавлениями red_m87_*"
  else
    fail "D-1(а): не-аддитивные правки библиотечного корпуса:"
    printf '%s\n' "$NONADD" | head -5
  fi
fi

# `R-200` §B5: решение по КОДУ ВОЗВРАТА, а не по тексту вывода (`gates.md` §3). Прежняя
# редакция искала строку `test result: FAILED` грепом — то есть принимала решение по тексту, и
# ошибка СБОРКИ (когда строки `test result` нет вовсе) читалась бы как зелёное.
LIBOUT=$(cargo test -p gateway 2>&1)
LIBRC=$?
LIBSUM=$(printf '%s\n' "$LIBOUT" | grep -E '^test result' | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f}')
if [ $LIBRC -eq 0 ]; then
  pass "D-1(б): библиотечный корпус зелен — ${LIBSUM:-GREEN}"
else
  fail "D-1(б): библиотечный корпус КРАСЕН (rc=$LIBRC) — ${LIBSUM:-сборка не прошла}"
  printf '%s\n' "$LIBOUT" | grep -E '^(error|thread )' | head -5
fi

# ─────── круг `R-198`+ задача 20 — СБОЙ провенанса истории НЕ МОЛЧИТ (§14.1decies) ───────
# Задача 17 завела честный пересчёт провенанса, но оба вызывателя в транспорте писали
# `if let Ok(...)`: при ошибке перезапись МОЛЧА пропускалась и клиенту уходило замороженное
# из слепка `history_truncated=false` — ровно та ложь, ради устранения которой задача 17 и
# заводилась. Тот же класс, что задача 19 (`§14.1nonies`), только ошибку глотал транспорт.
HP_OUT=$(cargo test -p gateway --test red_m87_history_provenance_failclosed 2>&1)
HP_RC=$?
HP_LINE=$(printf '%s\n' "$HP_OUT" | grep -E '^test result' | tail -1)
if [ $HP_RC -eq 0 ]; then
  pass "task20: сбой провенанса не выдаётся за полную историю — ${HP_LINE:-GREEN}"
else
  fail "task20: red_m87_history_provenance_failclosed КРАСЕН — ${HP_LINE:-компиляция}"
  printf '%s\n' "$HP_OUT" | grep -E '^(error|thread |провенанс|целый)' | head -6
fi

# Канарейка ФОРМЫ. Первая редакция считала УПОМИНАНИЯ ИМЕНИ, и это был дефект того самого
# класса, который милестоун ловит: из трёх упоминаний одно — импорт, поэтому порог «≥2»
# выполнялся при ОДНОМ живом вызове. Найдено ревьюером мутацией (`R-200` §B4): вызов оставлен,
# запись признака снята — не покраснело НИЧЕГО. Теперь считаются ВЫЗОВЫ (имя со скобкой), а
# строка импорта исключена явно — своё же требование «проверка по ВЫЗОВУ, а не по присутствию
# имени» канарейка обязана исполнять, а не только предъявлять другим.
HPS=$(grep -cE 'history_provenance_for_serve\(' crates/gateway-serve/src/lib.rs || true)
CHP=$(grep -cE 'current_history_provenance\(' crates/gateway-serve/src/lib.rs || true)
SITES=$(grep -cE '(snap|s)\.history_truncated *=' crates/gateway-serve/src/lib.rs || true)
if [ "$CHP" -eq 0 ] && [ "$HPS" -eq 2 ] && [ "$SITES" -eq 2 ]; then
  pass "task20: оба сайта зовут fail-closed обёртку (вызовов: $HPS, записей признака: $SITES)"
else
  fail "task20: вызовов обёртки $HPS (нужно РОВНО 2), прямых вызовов current_history_provenance $CHP (нужно 0), записей признака $SITES (нужно 2) — молчаливое поглощение или потеря сайта возможны"
fi

# Оракул ВТОРОГО сайта — путь подписки (`handle_v1_message`). Покрытие legacy-пути на него не
# переносится: `o6` судит ПЕРВОЕ сообщение соединения, а клиент выбирает инструмент подпиской.
ADDP_OUT=$(cargo test -p gateway-serve --test red_m87_provenance_on_add_path 2>&1)
ADDP_RC=$?
ADDP_LINE=$(printf '%s\n' "$ADDP_OUT" | grep -E '^test result' | tail -1)
if [ $ADDP_RC -eq 0 ]; then
  pass "task20: честность истории на пути ПОДПИСКИ — ${ADDP_LINE:-GREEN}"
else
  fail "task20: red_m87_provenance_on_add_path КРАСЕН — ${ADDP_LINE:-компиляция}"
  printf '%s\n' "$ADDP_OUT" | grep -E '^(error|thread |снимок|целый)' | head -6
fi

# ─────── круг `R-200` §B6 — ТОЧКА ВХОДА ПРОД-БИНАРЯ ИСПОЛНЯЕТСЯ, а не компилируется ───────
# Задача 14 была предъявлена только сборкой. Написание оракула немедленно вскрыло, что за этим
# пряталось: `admission_policy_from_env` требует ПЯТЬ переменных как обязательные, и НИ ОДНОЙ
# из них нет в `docker-compose.yml` — после выкатки сервис выдачи не поднялся бы вообще
# (замер: `EXIT=2`, `policy error: GATEWAY_ALLOWED_SYMBOLS must be set`).
# Бинарь обязан быть СОБРАН до прогона: оракул ИСПОЛНЯЕТ границу процесса, а не проверяет
# наличие файла (`testing.md` §«Механизм несущего пути обязан иметь оракул точки входа»).
cargo build -q -p gateway-serve --bin gateway-serve 2>/dev/null
ENTRY_OUT=$(cargo test -p gateway-serve --test red_m87_prod_entrypoint_argv 2>&1)
ENTRY_RC=$?
ENTRY_LINE=$(printf '%s\n' "$ENTRY_OUT" | grep -E '^test result' | tail -1)
if [ $ENTRY_RC -eq 0 ]; then
  pass "task14: прод-бинарь поднимается на окружении ИЗ compose — ${ENTRY_LINE:-GREEN}"
else
  fail "task14: red_m87_prod_entrypoint_argv КРАСЕН — ${ENTRY_LINE:-компиляция}"
  printf '%s\n' "$ENTRY_OUT" | grep -E '^(error|thread |docker-compose|прод-бинарь|порог)' | head -6
fi

# ─────── круг `R-200` §B7 — ПЯТЬ условий `R-196`, проверяемых ПО РАБОТЕ, а не по имени ───────
# Главное в находке §B7 — не сами пять условий, а приписка «гейт зелен при всех пяти»: шаги
# задач 4/5/6/8 были грепами ИМЕНИ, а присутствие имени и работа механизма — разные
# утверждения. Тот же класс дважды стоил круга (моя канарейка задачи 20 считала упоминания,
# одно из которых было импортом). Лекарство одно: проверять ВЫЗОВ либо ПОВЕДЕНИЕ.
# `R-201`: оракулы c2/c3 были ПЛАЦЕБО (проверяли текст) и УДАЛЕНЫ. Их предмет — объём реально
# прочитанного — переехал в отдельный бинарь, где эталон берёт ЯДРО (`/proc/self/io` `rchar`):
# независимо и от нашего счётчика, и от текста исходника. Первый прогон дал число, которого
# прежние два дать не могли: прочитано 448 831 Б при каталоге 194 703 Б и хвосте 465 Б.
VOL_OUT=$(cargo test -p gateway-serve --test red_m87_read_volume_truth 2>&1)
VOL_RC=$?
VOL_LINE=$(printf '%s\n' "$VOL_OUT" | grep -E '^test result' | tail -1)
if [ $VOL_RC -eq 0 ]; then
  pass "task16: объём чтения и счётчик согласованы с замером ЯДРА — ${VOL_LINE:-GREEN}"
else
  fail "task16: red_m87_read_volume_truth КРАСЕН — ${VOL_LINE:-компиляция}"
  printf '%s\n' "$VOL_OUT" | grep -E '^(error|thread |ФАКТИЧЕСКИ|счётчик|SETUP)' | head -5
fi

COND_OUT=$(cargo test -p gateway-serve --test red_m87_r196_conditions 2>&1)
COND_RC=$?
COND_LINE=$(printf '%s\n' "$COND_OUT" | grep -E '^test result' | tail -1)
if [ $COND_RC -eq 0 ]; then
  pass "task16: четыре условия R-196 (fail-closed без слепка · счётчик · бюджет · метаданные) — ${COND_LINE:-GREEN}"
else
  fail "task16: red_m87_r196_conditions КРАСЕН — ${COND_LINE:-компиляция}"
  printf '%s\n' "$COND_OUT" | grep -E '^(error|thread |счётчик|развёртывание|`feed_tail|ручное)' | head -6
fi

# Пятое условие — ОТДЕЛЬНЫМ бинарём: глобальный счётчик слотов ПРОЦЕССНЫЙ, и в общем бинаре
# соседи брали бы слоты параллельно, превращая замер во флак (`testing.md` §Целостность гейта,
# свойство 2: гейт меряет СВОЙ инвариант, не окружение).
RACE_OUT=$(cargo test -p gateway-serve --test red_m87_slot_counter_race 2>&1)
RACE_RC=$?
RACE_LINE=$(printf '%s\n' "$RACE_OUT" | grep -E '^test result' | tail -1)
if [ $RACE_RC -eq 0 ]; then
  pass "task16: глобальный счётчик слотов не теряет декременты под гонкой — ${RACE_LINE:-GREEN}"
else
  fail "task16: red_m87_slot_counter_race КРАСЕН — ${RACE_LINE:-компиляция}"
  printf '%s\n' "$RACE_OUT" | grep -E '^(error|thread |ГЛОБАЛЬНЫЙ|SETUP)' | head -4
fi

# ─── круг `R-202` — §Tasks НЕ ВРЁТ О СЕБЕ: барьер, а не третий абзац прозы (`gates.md` §11) ───
# Класс повторился ТРИ круга подряд: R-200 §B3 (19 задач стояли OPEN при готовом коде),
# R-201 (две пометки DONE при КРАСНЫХ оракулах), R-202 (три задачи OPEN при зелёных шагах плюс
# два обоснования, ссылавшихся на оракулы, удалённые этой же поставкой). Цена одинакова каждый
# раз: следующий исполнитель читает §Tasks и идёт чинить то, чего в коде нет, либо переделывать
# работающее. `gates.md` §11 прямо запрещает после третьего повторения писать прозу — либо
# механизм, либо явная запись остаточного риска.
#
# ЧТО ПРОВЕРЯЕТСЯ. Задача со статусом `⏳ OPEN` обязана НАЗЫВАТЬ, что именно не исполнено —
# любым из маркеров остатка. Статус без названного остатка неотличим от протухшей пометки:
# читатель не может понять, задача ещё не начата, наполовину сделана или просто забыта.
#
# ПРЕДЕЛ НАЗВАН ЧЕСТНО: барьер проверяет НАЛИЧИЕ заявления, а не его ПРАВДИВОСТЬ. Ложный, но
# существующий остаток он пропустит — против этого работает круг гейта, как и у прочих наших
# токенов. Он ловит ровно тот класс, который случился трижды: молчаливую пометку.
TASKS_BAD=""
TASKS_OPEN=0
while IFS= read -r line; do
  case "$line" in
    '| '*' | ⏳ OPEN'*)
      TASKS_OPEN=$((TASKS_OPEN + 1))
      num=$(printf '%s' "$line" | sed -E 's/^\| ([0-9]+б?) \|.*/\1/')
      # Маркеры остатка: любой из них означает «автор назвал, чего не хватает».
      if ! printf '%s' "$line" | grep -qE 'ОСТАТОК|остаток|НЕ ПОДКЛЮЧЁН|не подключён|ЧАСТИЧНО|не исполнен|долг|ОБОСНОВАНИЕ ПЕРЕПИСАНО|ещё не начат'; then
        TASKS_BAD="$TASKS_BAD $num"
      fi
      ;;
  esac
done < <(grep -E '^\| [0-9]+б? \| (⏳ OPEN|✅ DONE|🟡)' "$SPEC")
if [ "$TASKS_OPEN" -eq 0 ]; then
  pass "task-status: открытых задач нет — заявлять остаток нечего"
elif [ -z "$TASKS_BAD" ]; then
  pass "task-status: каждая из $TASKS_OPEN открытых задач НАЗЫВАЕТ свой остаток"
else
  fail "task-status: открытые задачи БЕЗ названного остатка:$TASKS_BAD — пометка неотличима от протухшей (R-200 §B3 · R-201 · R-202, три круга подряд)"
fi

# ─────────────── паритет с CI ───────────────
if cargo fmt --all -- --check >/dev/null 2>&1; then
  pass "CI-паритет: cargo fmt --all -- --check"
else
  fail "CI-паритет: cargo fmt --all -- --check"
fi

if cargo clippy --all-targets --all-features -- -D warnings >/dev/null 2>&1; then
  pass "CI-паритет: cargo clippy --all-targets --all-features -- -D warnings"
else
  fail "CI-паритет: cargo clippy --all-targets --all-features -- -D warnings"
fi

if cargo test --all >/dev/null 2>&1; then
  pass "CI-паритет: cargo test --all"
else
  fail "CI-паритет: cargo test --all"
fi

printf '\n'
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
fi
echo "VERDICT: FAIL (провалов: $FAIL)"
exit 1
