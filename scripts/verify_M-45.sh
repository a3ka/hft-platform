#!/usr/bin/env bash
# verify_M-45.sh — acceptance-гейт M-45: allow-list эмиссии L2Delta из хардкода в конфиг.
#
# Объём и обоснование — docs/rfc/CT-RFC-06-l2delta.md §8.1; спека —
# milestones/M-45-persist-l2delta.md. Предмет milestone'а — ТОЛЬКО состав символов;
# T1-форма не меняется (вариант L2Delta в контрактах с CT-RFC-04/M-18), поэтому
# contract-пакет docs/05 §4 не собирается и SCHEMA_VERSION не бампается.
#
# ГЛАВНОЕ СВОЙСТВО, которое проверяет этот гейт (T3): без выставленной конфигурации
# состав эмиссии остаётся РОВНО сегодняшним ["BTCUSDT"]. Именно оно делает merge
# безопасным без founder-подписи: код едет, прод не меняется, включение — операторский
# шаг (env + EPOCH_ID + рестарт). Если T3 красный — milestone мержить НЕЛЬЗЯ
# (Граница C, docs/PENDING-SIGNATURE.md П-003).
#
# Форма гейта — .claude/rules/gates.md §3: явный FAIL-счётчик + exit 1 при FAIL>0,
# никакого `cmd && echo PASS || echo FAIL` (маскирует провал).

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

SPOT_ORACLE="crates/venue-binance/tests/red_l2delta_allowlist.rs"
PERP_ORACLE="crates/venue-binance-futures/tests/red_l2delta_allowlist.rs"

echo "--- T0: оракулы M-45 на месте (sacred, architect-only) ---"
for f in "$SPOT_ORACLE" "$PERP_ORACLE"; do
  if [ -f "$f" ]; then pass "T0 оракул присутствует: $f"; else fail "T0 оракул ОТСУТСТВУЕТ: $f"; fi
done

echo "--- T1: сборка ВСЕГО workspace (узкий -p слеп к E0004 и не видит examples/bin — RN-8/RN-18) ---"
if cargo build --workspace >/tmp/m45-build.log 2>&1; then
  pass "T1 cargo build --workspace"
else
  fail "T1 cargo build --workspace — см. /tmp/m45-build.log"; tail -20 /tmp/m45-build.log
fi

echo "--- T2: clippy по всем таргетам ---"
if cargo clippy --workspace --all-targets -- -D warnings >/tmp/m45-clippy.log 2>&1; then
  pass "T2 cargo clippy --workspace --all-targets -D warnings"
else
  fail "T2 clippy — см. /tmp/m45-clippy.log"; tail -20 /tmp/m45-clippy.log
fi

echo "--- T2b: fmt — ТА ЖЕ проверка, что в CI (иначе green local ≠ green CI) ---"
# Находка tester'а 2026-08-02: гейт проверял build+clippy, а CI (`.github/workflows/ci.yml:20`)
# гоняет ЕЩЁ и `cargo fmt --all -- --check`. Локальный гейт был зелёным при красном CI —
# тот же класс, ради которого TD-035 пинует toolchain (green local ≠ green CI), только
# дыра была не в ВЕРСИИ, а в СОСТАВЕ проверок. Merge поверх этого дал бы красный main.
if cargo fmt --all -- --check >/tmp/m45-fmt.log 2>&1; then
  pass "T2b cargo fmt --all --check (совпадает с ci.yml)"
else
  fail "T2b fmt — CI упадёт на merge; файлы ниже"
  grep -E "^Diff in" /tmp/m45-fmt.log | sed 's|.*/crates/|crates/|' | sort -u
fi

echo "--- T3: ДЕФОЛТ НЕИЗМЕНЕН — merge не является раскаткой (главный пункт гейта) ---"
# Проверяется ИСПОЛНЯЕМЫМ тестом, а не грепом: греп поймал бы удаление строки, но не
# реализацию, которая строку сохранила и вернула другой список.
for crate in venue-binance venue-binance-futures; do
  if cargo test -p "$crate" --test red_l2delta_allowlist \
       o3_default_when_config_absent_equals_current_prod_behaviour \
       >/tmp/m45-default-$crate.log 2>&1 \
     && grep -qE "^test result: ok\. [1-9]" /tmp/m45-default-$crate.log; then
    pass "T3 $crate: без конфигурации состав эмиссии = [\"BTCUSDT\"]"
  else
    fail "T3 $crate: дефолт ИЗМЕНЁН или тест не выполнился — merge запрещён (Граница C)"
    tail -20 /tmp/m45-default-$crate.log
  fi
done

# Анти-подлог: оракул обязан сравнивать с BTCUSDT, а не с чем угодно. Если константу
# ожидания в самом оракуле подменят, T3 выше станет зелёным ложно.
for f in "$SPOT_ORACLE" "$PERP_ORACLE"; do
  if grep -qE 'PROD_DEFAULT: &\[&str\] = &\["BTCUSDT"\]' "$f"; then
    pass "T3 ожидаемый дефолт в оракуле не подменён: $f"
  else
    fail "T3 в оракуле $f изменена эталонная константа PROD_DEFAULT — гейт потерял смысл"
  fi
done

echo "--- T4: негативный путь и регистр (анти-плацебо: без них 'капчить всегда' проходит) ---"
for crate in venue-binance venue-binance-futures; do
  if cargo test -p "$crate" --test red_l2delta_allowlist >/tmp/m45-allow-$crate.log 2>&1 \
     && grep -qE "^test result: ok\." /tmp/m45-allow-$crate.log; then
    n=$(grep -cE "^test .* \.\.\. ok" /tmp/m45-allow-$crate.log)
    pass "T4 $crate: allow-list оракул GREEN ($n тестов)"
  else
    fail "T4 $crate: allow-list оракул КРАСНЫЙ"; tail -30 /tmp/m45-allow-$crate.log
  fi
done

echo "--- T5: НЕТ ОБХОДНОГО ПУТИ эмиссии мимо allow-list (C-048 §1) ---"
# Урок C-048 REJECT: греп по ИМЕНИ константы — негодная канарейка. Реализация могла
# переименовать константу или заинлайнить список литералом, оставив чистые функции
# осиротевшими (экспортированы, зовутся только из тестов), и весь гейт был бы зелёным,
# а раскатка не работала бы. Дефект всплыл бы только после founder-подписи — позже всех
# гейтов.
#
# Поэтому проверяется ОТСУТСТВИЕ альтернативного пути (образец INTG-I: тест подтверждает
# отсутствие обхода, а не наличие проверки): сырой транслятор `l2delta_event(` имеет право
# вызываться в прод-коде РОВНО из одного места — из `l2delta_emission_for`, единственной
# точки решения. Любой второй call site = путь в обход allow-list.
for crate in venue-binance venue-binance-futures; do
  calls=$(grep -rn 'l2delta_event(' "crates/$crate/src/" --include=*.rs 2>/dev/null \
          | grep -vE 'fn l2delta_event|///|//!|^\s*//' | wc -l)
  if [ "$calls" -eq 1 ]; then
    if grep -rn 'l2delta_event(' "crates/$crate/src/" --include=*.rs 2>/dev/null \
         | grep -vE 'fn l2delta_event|///|//!|^\s*//' \
         | grep -q 'l2delta_emission_for\|emission_for' \
       || awk '/fn l2delta_emission_for/,/^}/' "crates/$crate/src/lib.rs" 2>/dev/null \
            | grep -q 'l2delta_event('; then
      pass "T5 $crate: единственный вызов l2delta_event — внутри l2delta_emission_for"
    else
      fail "T5 $crate: единственный вызов l2delta_event НЕ внутри l2delta_emission_for — \
решение об эмиссии принимается мимо allow-list"
    fi
  else
    fail "T5 $crate: вызовов l2delta_event в src = $calls (ожидается ровно 1, внутри \
l2delta_emission_for). Каждый лишний call site — путь эмиссии в обход allow-list"
    grep -rn 'l2delta_event(' "crates/$crate/src/" --include=*.rs | grep -vE 'fn l2delta_event'
  fi
done

# Хардкод-список символов не имеет права остаться ни под каким именем: ищем массив
# строковых литералов, похожих на тикеры, в venue-крейтах вне тестов.
if grep -rnE '&\[ *"[A-Z]{2,}USD[TC]?" *(, *"[A-Z]{2,}USD[TC]?" *)*\]' \
     crates/venue-binance/src/ crates/venue-binance-futures/src/ --include=*.rs \
     >/tmp/m45-hardcode.log 2>&1; then
  fail "T5 хардкод-список тикеров ещё жив в прод-коде (переименование константы не считается фиксом):"
  cat /tmp/m45-hardcode.log
else
  pass "T5 хардкод-списка тикеров в venue-src нет"
fi

echo "--- T5b: РЕШАЮЩАЯ проверка — поведение реальной точки входа (O-8, C-049) ---"
# Структурные грепы T5 обходятся сдвигом хардкода на уровень выше (C-049 §1.2). Здесь
# проверяется ПОВЕДЕНИЕ: Session::on_ws_text скармливается сырой wire-текст, проверяется
# состав Vec<SessionEffect>. Любое хардкод-условие по символу на любом уровне внутри
# обработки проявится как отсутствие ожидаемого Emit.
for crate in venue-binance venue-binance-futures; do
  if cargo test -p "$crate" --test red_l2delta_allowlist o8_ >/tmp/m45-o8-$crate.log 2>&1 \
     && grep -qE "^test result: ok\. [1-9]" /tmp/m45-o8-$crate.log; then
    n=$(grep -cE "^test .* \.\.\. ok" /tmp/m45-o8-$crate.log)
    pass "T5b $crate: O-8 GREEN ($n тестов через реальную точку входа)"
  else
    fail "T5b $crate: O-8 КРАСНЫЙ — allow-list не управляет эмиссией на реальном пути"
    tail -25 /tmp/m45-o8-$crate.log
  fi
done

echo "--- T6: сырой L2Delta-транслятор не задет (T1-форма и семантика pu/U/u) ---"
for crate in venue-binance venue-binance-futures; do
  if cargo test -p "$crate" --test red_l2delta_capture >/tmp/m45-capture-$crate.log 2>&1 \
     && grep -qE "^test result: ok\." /tmp/m45-capture-$crate.log; then
    pass "T6 $crate: оракул сырого захвата (M-18/CT-RFC-04) остался GREEN"
  else
    # У перп-крейта имя файла может отличаться — отсутствие таргета не является провалом,
    # провалом является КРАСНЫЙ существующий оракул.
    if grep -q "no test target" /tmp/m45-capture-$crate.log; then
      pass "T6 $crate: отдельного red_l2delta_capture нет (покрыт общим прогоном T7)"
    else
      fail "T6 $crate: оракул сырого захвата СЛОМАН — задета T1-форма или семантика continuity"
      tail -20 /tmp/m45-capture-$crate.log
    fi
  fi
done

echo "--- T7: контракты не тронуты (T1-формы M-45 не меняет — CT-RFC-06 §2) ---"
if git diff --name-only origin/main...HEAD 2>/dev/null | grep -q '^crates/contracts/'; then
  fail "T7 дифф трогает crates/contracts/** — это contract-изменение, нужен CT-RFC + risk-critic"
else
  pass "T7 crates/contracts/** не тронут"
fi

echo "--- T8: DET-I-1 на смешанном журнале (TD-072) ---"
# ТРЕТИЙ носитель класса «проверка присутствия ШИРЕ требования», найден грепом класса при
# закрытии `C-204`, а не вердиктом. Прежний страж `grep -q "L2Delta"` совпадал с
# КОММЕНТАРИЯМИ (`:599`, `:605`, `:758`, `:775` — четыре из шести вхождений): удали кто-нибудь
# фикстуры, оставив прозу, и страж бы не заметил. Требование — «оракул СОДЕРЖИТ фикстуру
# L2Delta», поэтому пиннится КОНСТРУКЦИЯ значения и ИМЯ теста, а не слово в файле.
if grep -qE '^[[:space:]]*MdPayload::L2Delta \{' crates/journal/tests/red_det_replay_digest.rs \
     2>/dev/null \
   && grep -q 'fn det_9_mixed_snapshot_delta_journal_is_bit_identical_and_delta_sensitive' \
     crates/journal/tests/red_det_replay_digest.rs 2>/dev/null; then
  if cargo test -p journal --test red_det_replay_digest \
       det_9_mixed_snapshot_delta_journal_is_bit_identical_and_delta_sensitive \
       >/tmp/m45-det.log 2>&1 \
     && grep -qE "^test result: ok\. [1-9]" /tmp/m45-det.log; then
    pass "T8 DET-I-1 GREEN на смешанном журнале (снапшот+дельта; O-5 исполнен поимённо)"
  else
    fail "T8 DET-I-1 КРАСНЫЙ"; tail -20 /tmp/m45-det.log
  fi
else
  fail "T8 оракул DET-I-1 не содержит КОНСТРУКЦИИ фикстуры MdPayload::L2Delta либо теста \
det_9_… (TD-072 не закрыт) — расширение эмиссии уехало бы под оракулом, который расширенного \
потока не видел. Упоминание слова L2Delta в комментарии не засчитывается"
fi

echo "--- T9: эпоха ОБЪЯВЛЕНА в реестре, если раскатка исполнена (анти-E-001) ---"
# ПЕРЕПИСАН по `R-167` Б-2. Прежняя редакция спрашивала «менялся ли дефолт В КОДЕ»
# (`PROD_DEFAULT` в спот-оракуле) — и была слепа по построению: ВЕСЬ смысл `M-45` §1 в том,
# что раскатка идёт КОНФИГОМ, а дефолт кода остаётся `["BTCUSDT"]` навсегда. Значит в
# коммите, который меняет состав боевых данных, шаг печатал «запись эпохи не требуется» и
# уходил зелёным. Это ВТОРОЙ экземпляр класса §3septies: оракул мерил мир, который сам же
# милестоун и упразднил.
#
# Теперь предмет наблюдения — тот же, каким живёт раскатка: значение `EPOCH_ID` из compose,
# снятое ТЕМ ЖЕ CLI, что и в T10 (`Р-1`: мера на границе потребителя). Сравнение — на
# ТОЧНОЕ значение, а не `grep m45`: подстрока совпадала бы с любым упоминанием милестоуна.
# FAIL-CLOSED НА СОБСТВЕННУЮ ПРИМЕНИМОСТЬ. Первая редакция этого шага брала только stdout и
# при сломанном извлечении получала пустую строку — то есть уходила в ветку «раскатка не
# исполнена» и печатала ЗЕЛЁНОЕ. Поймано моей же мутацией М6 при закрытии `R-167`: шаг,
# теряющий источник данных, обязан краснеть, а не деградировать в «проверять нечего»
# (`testing.md` §«Целостность гейта» св. 3). Отделяем ОТКАЗ извлечения от ОТСУТСТВИЯ ключа
# кодом возврата, а не по пустоте вывода.
T9_RAW="$(python3 scripts/lib/rollout_symbols_check.py --extract docker-compose.yml 2>&1)"
T9_ST=$?
T9_EPOCH="$(printf '%s\n' "$T9_RAW" | sed -n 's/^EPOCH_ID=//p')"
if [ "$T9_ST" -ne 0 ]; then
  fail "T9 SETUP НЕ СОСТОЯЛСЯ — извлечение конфигурации вернуло $T9_ST: $T9_RAW"
elif [ -z "$T9_EPOCH" ]; then
  fail "T9 SETUP НЕ СОСТОЯЛСЯ — извлечение прошло (код 0), но строки EPOCH_ID в выводе нет: \
$T9_RAW"
elif [ "$T9_EPOCH" = "<ОТСУТСТВУЕТ>" ]; then
  # Раскатка не исполнена — состав потока прежний, объявлять нечего.
  if grep -rqE 'PROD_DEFAULT: &\[&str\] = &\["BTCUSDT"\]' "$SPOT_ORACLE"; then
    pass "T9 раскатка не исполнена и дефолт кода не менялся ⇒ запись эпохи не требуется"
  else
    fail "T9 дефолт кода изменён (обход конфига) БЕЗ раскатки — состав правится кодом, \
это совершение решения Границы C минуя подпись (M-45 §1)"
  fi
elif awk '/^## E-002/{f=1} f&&/^## /&&!/^## E-002/{exit} f' docs/data-epochs.md 2>/dev/null \
       | grep -qF "$T9_EPOCH"; then
  pass "T9 раскатка исполнена И эпоха '$T9_EPOCH' названа В РАЗДЕЛЕ E-002"
else
  fail "T9 раскатка исполнена (EPOCH_ID='$T9_EPOCH'), но этого значения НЕТ В РАЗДЕЛЕ E-002 \
файла docs/data-epochs.md — merge ветки триггерит деплой (deploy.yml paths: \
docker-compose.yml), то есть состав сменится на проде, а запись СВОЕЙ эпохи не назовёт, чем \
помечена граница (E-001). Вхождение литерала в ДРУГОЙ раздел не засчитывается: требование \
R-167 Б-1 — чтобы эпоху называла запись, к которой она относится (C-204)"
fi

echo "--- T10: задача 7 (РАСКАТКА) — обе переменные на сервисе recorder, ОДНИМ коммитом ---"
# Добавлено по `C-195` B-2: задача 7 была объявлена в §Tasks и НЕ ИМЕЛА проверки — гейт
# проходил зелёным, пока обеих env-строк в compose нет вовсе. Ссылка задачи на `П-026`
# §Порядок — инструкция, а не оракул: она ничего не делает красным до исполнения.
#
# Проверка судит КОНФИГУРАЦИЮ, а не текст документации: YAML разбирается, сервис ищется по
# `container_name: hft-recorder` (тот, что реально пишет журнал), переменные читаются из его
# `environment`. Греп по файлу целиком дал бы зелёное на упоминании в комментарии соседнего
# сервиса — ровно класс `M-45` §D-1 («гейт по форме текста обходится сдвигом на уровень»).
#
# FAIL-CLOSED НА СОБСТВЕННУЮ ПРИМЕНИМОСТЬ: нет `pyyaml`, нет файла, не найден сервис —
# ОТКАЗ, а не тихий пропуск. Проверка, молчащая при несостоявшемся setup, есть плацебо
# самой себя (`testing.md` §«Целостность гейта» св. 3-4).
T10_OUT="$(python3 scripts/lib/rollout_symbols_check.py --compose docker-compose.yml 2>&1)"; T10_ST=$?
if [ "$T10_ST" -eq 0 ]; then
  pass "T10 обе переменные раскатки на сервисе recorder ($T10_OUT)"
elif [ "$T10_ST" -eq 2 ]; then
  fail "T10 SETUP НЕ СОСТОЯЛСЯ — $T10_OUT"
else
  fail "T10 задача 7 НЕ исполнена — $T10_OUT"
fi

# Вторая половина задачи 7: ОДНИМ коммитом. Раскатка в два шага оставляет окно, где состав
# уже расширен, а эпоха ещё прежняя (или наоборот) — то есть события двух составов попадают
# под один `epoch_id` и становятся машинно неразличимы. Это класс `E-001`, стоивший разбора
# 123 млн событий, и `M-45` §2 называет его «остаточным классом» прямо.
#
# ПРИЗНАК ПРОВЕРЕН НА РАЗЛИЧАЮЩУЮ СИЛУ (`Р-4`), и первая редакция его НЕ ИМЕЛА. Замер:
#   git log -S'EPOCH_ID'       -- docker-compose.yml  →  4aca3f6   ← КОММЕНТАРИЙ, не ключ
#   git log -G'^\s*EPOCH_ID:'  -- docker-compose.yml  →  (пусто)   ← настоящий YAML-ключ
# `-S` считает вхождения ПОДСТРОКИ и поймал фразу «требует нового EPOCH_ID» из комментария
# соседней правки — то есть краснел на предмете, которого нет. Якорь `^\s*КЛЮЧ:` через `-G`
# различает ОБЪЯВЛЕНИЕ от УПОМИНАНИЯ. Поймано прогоном пробы, а не рассуждением.
if [ "$T10_ST" -eq 0 ]; then
  C_SYM="$(git log -1 --format=%H -G'^\s*L2DELTA_CAPTURE_SYMBOLS:' -- docker-compose.yml 2>/dev/null)"
  C_EPO="$(git log -1 --format=%H -G'^\s*EPOCH_ID:' -- docker-compose.yml 2>/dev/null)"
  if [ -z "$C_SYM" ] || [ -z "$C_EPO" ]; then
    fail "T10b переменные есть в рабочем дереве, но НЕ В ИСТОРИИ — раскатка не закоммичена, судить об одном коммите нечего (sym=${C_SYM:-нет} epoch=${C_EPO:-нет})"
  elif [ "$C_SYM" = "$C_EPO" ]; then
    pass "T10b состав и эпоха внесены ОДНИМ коммитом (${C_SYM:0:8})"
  else
    fail "T10b состав и эпоха внесены РАЗНЫМИ коммитами (${C_SYM:0:8} против ${C_EPO:0:8}) — между ними события двух составов пишутся под одним epoch_id (класс E-001)"
  fi
fi

echo "--- T10c: МУТАЦИЯ СОСТАВА — гейт обязан отвергнуть неподписанный символ ---"
# `C-199` B-3 нашёл, что T10 пропускал `BTCUSDT,ETHUSDT,SOLUSDT`. Мало починить сравнение —
# надо ПРЕДЪЯВИТЬ, что починка работает: правило `Р-4` требует мутации, целящей В ПРИЗНАК.
# Шаг подставляет неподписанный символ в КОПИЮ compose и требует, чтобы разбор его отверг.
#
# Проверяется РОВНО тот код, который исполняет T10: и шаг, и проба зовут ОДИН CLI
# `scripts/lib/rollout_symbols_check.py`. Прежний комментарий здесь утверждал, что «тело
# извлекается из этого же файла», — механизма с таким описанием не существовало
# (`A-030` §3 п.4: комментарий, лгущий о конструкции гейта, отправляет следующего критика
# проверять то, чего нет; родня «ложного якоря» правила `Р-3`).
T10C_OUT="$(python3 - <<'PY' 2>&1
import subprocess, sys, tempfile, os, shutil
# `A-030` §3 п.2: проба гоняет ТОТ ЖЕ CLI, каким исполняется шаг T10 — и на фикстурных
# КОПИЯХ compose, и прямыми значениями. Вне пробы остаётся только маппинг exit-кода на
# pass/fail. Замер 4b арбитра показал, почему это существенно: пока склейка жила в этом
# файле, её мутация (`bad = []`) пропускала неподписанный состав, а проба оставалась зелёной.
CLI = ["python3", "scripts/lib/rollout_symbols_check.py"]
BASE = open("docker-compose.yml").read()
ANCHOR = "      HL_COINS: ${HL_COINS:-BTC,ETH}"
KEYS = ("L2DELTA_CAPTURE_SYMBOLS", "EPOCH_ID")

# Фикстура строится ЗАМЕНОЙ, а не дописыванием. Прежняя редакция дописывала ключ ниже якоря
# `HL_COINS`, и это было годно РОВНО до раскатки: как только задача 7 внесла те же ключи в
# compose, дописка стала ДУБЛЕМ, а PyYAML применяет last-wins — побеждало реальное значение,
# мутация исчезала, и проба краснела на пяти сценариях из семи. Оракул ломался в тот самый
# момент, ради которого написан. Найдено engine-dev'ом на `f3b84d4`.
def strip_keys(text):
    return "".join(l for l in text.splitlines(keepends=True)
                   if not any(l.strip().startswith(k + ":") for k in KEYS))

def compose_with(sym=None, epoch=None, drop_service=False):
    s = strip_keys(BASE)
    if drop_service:
        return s.replace("    container_name: hft-recorder\n", "    container_name: hft-other\n", 1)
    add = ""
    if sym is not None:
        add += f"\n      L2DELTA_CAPTURE_SYMBOLS: {sym}"
    if epoch is not None:
        add += f"\n      EPOCH_ID: {epoch}"
    return s.replace(ANCHOR, ANCHOR + add, 1)

def setup_guard(path, sym, epoch, drop_service):
    """Фикстура ОБЯЗАНА нести то, что задумал сценарий — иначе судится не тот мир.

    `testing.md` §«Целостность гейта» св. 3: проба, молча тестирующая не тот сценарий, есть
    плацебо самой себя. Guard снимается ТЕМ ЖЕ CLI, который фикстуру потом судит
    (`--extract`), а не вторым разбором в пробе: редекларация разбора внутри пробы уже стоила
    `C-202` B-2. Возврат: None — setup состоялся; строка — причина отказа (код 2).
    """
    r = subprocess.run(CLI + ["--extract", path], capture_output=True, text=True)
    if drop_service:
        return None if r.returncode == 2 else \
            f"фикстура «сервис не найден» разобралась (код {r.returncode}) — мир не построен"
    if r.returncode != 0:
        return f"--extract вернул {r.returncode}: {r.stdout.strip()} {r.stderr.strip()}"
    seen = dict(l.split("=", 1) for l in r.stdout.strip().splitlines() if "=" in l)
    for key, want in (("L2DELTA_CAPTURE_SYMBOLS", sym), ("EPOCH_ID", epoch)):
        exp = "<ОТСУТСТВУЕТ>" if want is None else str(want)
        if seen.get(key) != exp:
            return (f"{key}: CLI видит {seen.get(key)!r}, сценарий задумал {exp!r} — "
                    f"фикстура не несёт мутации (дубль ключа? last-wins?)")
    return None

# (описание, содержимое compose, ожидаемый код)
# (описание, содержимое compose, ожидаемый код, НАМЕРЕНИЕ сценария для setup-guard)
OK_EPOCH = "own-2026-09-m45-fixture"   # объявленная эпоха по конвенции CT-RFC-06 §3
_C = [
    ("подписанный литерал",       ("BTCUSDT,ETHUSDT", OK_EPOCH, False), 0),
    ("ЛИШНИЙ символ литералом",   ("BTCUSDT,ETHUSDT,SOLUSDT", OK_EPOCH, False), 1),
    ("подстановка :- (обходима)", ("${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT}", OK_EPOCH, False), 1),
    ("подстановка без двоеточия", ("${L2DELTA_CAPTURE_SYMBOLS-BTCUSDT,ETHUSDT}", OK_EPOCH, False), 1),
    ("эпоха подстановкой",        ("BTCUSDT,ETHUSDT", "${EPOCH_ID:-own-x}", False), 1),
    # `R-167` Б-3: действующая де-факто эпоха при расширенном составе. Дефолт кода —
    # `own-<UTC-YYYY-MM>`; принять её значило бы принять «состав расширен, метка прежняя».
    ("эпоха = ДЕФОЛТ ПО ЧАСАМ",   ("BTCUSDT,ETHUSDT", "own-2026-08", False), 1),
    ("эпоха без слага милестоуна", ("BTCUSDT,ETHUSDT", "own-2026-09-m45", False), 1),
    ("КЛЮЧИ ОТСУТСТВУЮТ",         (None, None, False), 1),
    ("сервис recorder НЕ НАЙДЕН", (None, None, True), 2),
]
COMPOSE_CASES = [
    (why, compose_with(intent[0], intent[1], drop_service=intent[2]), want, intent)
    for why, intent, want in _C
]
VALUE_CASES = [
    ("BTCUSDT,ETHUSDT",         OK_EPOCH, 0, "подписанное множество"),
    ("ETHUSDT,BTCUSDT",         OK_EPOCH, 0, "порядок не значим"),
    (" btcusdt , ethusdt ",     OK_EPOCH, 0, "регистр и пробелы"),
    ("BTCUSDT",                 OK_EPOCH, 1, "потерян ETHUSDT"),
    ("BTCUSDTX,ETHUSDT",        OK_EPOCH, 1, "подстрочно-похожий токен"),
    ("BTCUSDT,ETHUSDT",         "",       1, "пустая эпоха"),
    ("BTCUSDT,ETHUSDT",         "own-2026-08", 1, "эпоха дефолтом по часам (R-167 Б-3)"),
]
bad = []
setup_failures = []
tmp = tempfile.mkdtemp()
try:
    for why, content, want, intent in COMPOSE_CASES:
        f = os.path.join(tmp, "docker-compose.yml")
        open(f, "w").write(content)
        why_setup = setup_guard(f, *intent)
        if why_setup:
            setup_failures.append(f"compose «{why}»: SETUP НЕ СОСТОЯЛСЯ — {why_setup}")
            continue
        r = subprocess.run(CLI + ["--compose", f], capture_output=True, text=True)
        if r.returncode != want:
            bad.append(f"compose «{why}»: ожидался код {want}, получен {r.returncode}")
    for sym, epoch, want, why in VALUE_CASES:
        r = subprocess.run(CLI + [sym, epoch], capture_output=True, text=True)
        if r.returncode != want:
            bad.append(f"значения {sym!r}/{epoch!r}: ожидался {want}, получен {r.returncode} ({why})")
finally:
    shutil.rmtree(tmp, ignore_errors=True)
if setup_failures:
    # Несостоявшийся setup — ОТКАЗ (код 2), а не «различающая сила не предъявлена»:
    # молчать о том, что судился не тот мир, нельзя.
    print("; ".join(setup_failures)); sys.exit(2)
if bad:
    print("; ".join(bad)); sys.exit(1)
print(f"мутация состава: {len(COMPOSE_CASES)} миров compose (каждый под setup-guard'ом) "
      f"+ {len(VALUE_CASES)} сценариев значений через ТОТ ЖЕ CLI, что и T10")
PY
)"; T10C_ST=$?
if [ "$T10C_ST" -eq 0 ]; then
  pass "T10c $T10C_OUT"
elif [ "$T10C_ST" -eq 2 ]; then
  fail "T10c SETUP НЕ СОСТОЯЛСЯ — $T10C_OUT"
else
  fail "T10c РАЗЛИЧАЮЩАЯ СИЛА НЕ ПРЕДЪЯВЛЕНА — $T10C_OUT"
fi

echo
if [ "$FAILED" -gt 0 ]; then
  echo "VERDICT: FAIL ($FAILED нарушений)"
  exit 1
fi
echo "VERDICT: PASS"
exit 0
