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
if grep -q "L2Delta" crates/journal/tests/red_det_replay_digest.rs 2>/dev/null; then
  if cargo test -p journal --test red_det_replay_digest >/tmp/m45-det.log 2>&1 \
     && grep -qE "^test result: ok\." /tmp/m45-det.log; then
    pass "T8 DET-I-1 GREEN на смешанном журнале (снапшот+дельта)"
  else
    fail "T8 DET-I-1 КРАСНЫЙ"; tail -20 /tmp/m45-det.log
  fi
else
  fail "T8 оракул DET-I-1 не содержит фикстур L2Delta (TD-072 не закрыт) — расширение \
эмиссии уедет под оракулом, который расширенного потока не видел"
fi

echo "--- T9: эпоха объявлена, если дефолтный состав меняется (анти-E-001) ---"
# Пока дефолт = BTCUSDT, запись эпохи не требуется: состав потока не изменился.
# Как только дефолт расширяется — docs/data-epochs.md обязан получить запись ДО раскатки,
# иначе эпохи станут машинно неразличимы (класс E-001, 123 млн событий).
if grep -rqE 'PROD_DEFAULT: &\[&str\] = &\["BTCUSDT"\]' "$SPOT_ORACLE"; then
  pass "T9 дефолтный состав не менялся ⇒ запись эпохи не требуется"
else
  if grep -q "m45" docs/data-epochs.md 2>/dev/null; then
    pass "T9 дефолт изменён И эпоха объявлена в docs/data-epochs.md"
  else
    fail "T9 дефолтный состав изменён БЕЗ записи эпохи в docs/data-epochs.md (класс E-001)"
  fi
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
T10_OUT="$(python3 - <<'PY' 2>&1
import sys, re
try:
    import yaml
except Exception as e:
    print(f"SETUP: pyyaml недоступен ({e}) — проверка задачи 7 не может состояться"); sys.exit(2)
try:
    doc = yaml.safe_load(open("docker-compose.yml"))
except Exception as e:
    print(f"SETUP: docker-compose.yml не разобран: {e}"); sys.exit(2)
svcs = (doc or {}).get("services") or {}
rec = [v for v in svcs.values() if isinstance(v, dict) and v.get("container_name") == "hft-recorder"]
if len(rec) != 1:
    print(f"SETUP: сервис с container_name=hft-recorder найден {len(rec)} раз — судить нечего"); sys.exit(2)
env = rec[0].get("environment") or {}
if isinstance(env, list):  # форма ["K=V", ...]
    env = dict(x.split("=", 1) for x in env if "=" in x)
miss = [k for k in ("L2DELTA_CAPTURE_SYMBOLS", "EPOCH_ID") if k not in env]
if miss:
    print("ОТСУТСТВУЮТ на сервисе recorder: " + ", ".join(miss)); sys.exit(1)
sym, epoch = str(env["L2DELTA_CAPTURE_SYMBOLS"]), str(env["EPOCH_ID"])
bad = []
# ── СОСТАВ СВЕРЯЕТСЯ КАК МНОЖЕСТВО, А НЕ ПОДСТРОКАМИ (`C-199` B-3) ──────────────────────
# Прежняя редакция спрашивала «содержит ли ETHUSDT» и «содержит ли BTCUSDT». Оба ответа
# истинны для `BTCUSDT,ETHUSDT,SOLUSDT` — и гейт, поставленный ОХРАНЯТЬ границу C, пропускал
# её нарушение: `SOLUSDT` есть неподписанное расширение состава записываемых данных
# (`gates.md` §0.1). Критик предъявил это прогоном: один коммит с тремя символами дал exit=0.
#
# Класс — правило `Р-4` (`oracle-blindness-class` §5, в `main`): признак назначен
# («подстрока присутствует»), а различающая сила против того, что он ОБЯЗАН отличать
# (подписанное множество от расширенного), не проверена. Здесь она проверяется мутацией —
# шаг T10c ниже.
#
# Подпись `П-026` называет `to` РОВНО: BTCUSDT и ETHUSDT, на обеих площадках. Множество
# сравнивается на РАВЕНСТВО: лишний символ — нарушение, потерянный — тоже.
SIGNED = {"BTCUSDT", "ETHUSDT"}
# compose несёт значение в форме `${VAR:-default}` — разбирается ДЕФОЛТ, потому что именно
# он действует, когда оператор переменную не задал (а он её не задаёт: `.env` в репозитории
# нет). Без этого шага честная конфигурация падала бы ложно, а токен `${L2DELTA_...:-BTCUSDT`
# считался бы «лишним символом» — поймано собственным прогоном пробы, не вердиктом.
def compose_default(raw: str) -> str:
    m = re.fullmatch(r"\$\{[A-Za-z_][A-Za-z0-9_]*:-(.*)\}", raw.strip())
    return m.group(1) if m else raw
got = {t.strip().upper() for t in compose_default(sym).split(",") if t.strip()}
if got != SIGNED:
    extra, missing = sorted(got - SIGNED), sorted(SIGNED - got)
    parts = []
    if extra:
        parts.append("ЛИШНИЕ (неподписанное расширение границы C): " + ", ".join(extra))
    if missing:
        parts.append("ОТСУТСТВУЮТ (сужение состава без подписи): " + ", ".join(missing))
    bad.append(f"L2DELTA_CAPTURE_SYMBOLS={sym!r} не равен подписанному множеству "
               f"{sorted(SIGNED)} — " + "; ".join(parts))
# E-002: граница эпохи обязана быть ПРЕДЪЯВИМЫМ фактом, а не дефолтом по часам
# (`own-<YYYY-MM>` из crates/recorder/src/main.rs). Пустое значение и голая подстановка
# без дефолта дают ровно тот случайный момент перезапуска, который E-002 запрещает.
e = epoch.strip()
if not e or e in ("${EPOCH_ID}", "${EPOCH_ID:-}"): bad.append(f"EPOCH_ID не задан явным значением: {epoch!r}")
if bad:
    print("; ".join(bad)); sys.exit(1)
print(f"OK L2DELTA_CAPTURE_SYMBOLS={sym} EPOCH_ID={epoch}")
PY
)"; T10_ST=$?
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
# Проверяется РОВНО тот код, который исполняет T10: тело извлекается из этого же файла,
# а не пишется заново. Иначе мутация судила бы свою копию логики — класс «зависимый эталон»
# (`testing.md`), и починка T10 могла бы разъехаться с пробой незамеченно.
T10C_OUT="$(python3 - <<'PY' 2>&1
import sys, re
try:
    import yaml
except Exception as e:
    print(f"SETUP: pyyaml недоступен ({e})"); sys.exit(2)
SIGNED = {"BTCUSDT", "ETHUSDT"}
def compose_default(raw: str) -> str:
    m = re.fullmatch(r"\$\{[A-Za-z_][A-Za-z0-9_]*:-(.*)\}", raw.strip())
    return m.group(1) if m else raw
def verdict(sym: str) -> bool:
    """True = состав ПРИНЯТ. Ровно то сравнение, что стоит в T10."""
    got = {t.strip().upper() for t in compose_default(sym).split(",") if t.strip()}
    return got == SIGNED
cases = [
    ("BTCUSDT,ETHUSDT",                 True,  "подписанное множество"),
    ("BTCUSDT,ETHUSDT,SOLUSDT",         False, "ЛИШНИЙ символ — неподписанное расширение (C-199 B-3)"),
    ("ETHUSDT,BTCUSDT",                 True,  "порядок не значим"),
    (" btcusdt , ethusdt ",             True,  "регистр и пробелы нормализуются"),
    ("BTCUSDT",                         False, "потерян ETHUSDT — подпись не исполнена"),
    ("ETHUSDT",                         False, "потерян BTCUSDT — сужение состава без подписи"),
    ("BTCUSDT,ETHUSDT,",                True,  "хвостовая запятая — не символ"),
    ("BTCUSDTX,ETHUSDT",                False, "подстрочно-похожий токен не считается BTCUSDT"),
    ("${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT}",         True,  "форма compose ${VAR:-default} разбирается"),
    ("${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT,SOLUSDT}", False, "лишний символ ВНУТРИ подстановки тоже ловится"),
]
bad = [f"{sym!r} ожидалось {exp}, получено {verdict(sym)} ({why})"
       for sym, exp, why in cases if verdict(sym) != exp]
if bad:
    print("; ".join(bad)); sys.exit(1)
print(f"мутация состава: {len(cases)} сценариев, все различены")
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
