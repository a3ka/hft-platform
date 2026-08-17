#!/usr/bin/env bash
# Проба барьера scripts/check_resource_oracles.sh (RO-1..RO-9).
#
# Барьер ловит механизм, давший ДВА флака обязательного чека `main` (TD-098/TD-129):
# ресурсный оракул вёл учёт аллокаций процессным счётчиком, а `cargo test` гоняет тесты
# одного бинаря параллельными потоками ⇒ замер включал соседей.
#
# ГЛАВНОЕ ТРЕБОВАНИЕ ТРЕКА (`harness-track.md` §5 п.1): проба обязана быть КРАСНОЙ против
# набора обманных стабов, а не только зелёной против честной реализации. Стабы ниже — не
# украшение: на M-60 четыре дефекта из четырёх нашли именно они.
#
# Сценарии — по ВЫЗОВУ (исполнением барьера), а не по тексту: grep по имени функции бывает
# зелен, когда барьер не работает ни при какой форме вызова.

set -uo pipefail

SELF="$(cd "$(dirname "$0")" && pwd)"
BARRIER="${BARRIER:-${SELF}/../check_resource_oracles.sh}"

PASSED=0
FAILED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

# ── УБОРКА ФИКСТУР: реестр в ФАЙЛЕ, а не в переменной ────────────────────────────────────
# Переменная, наполняемая в подоболочке `$( )`, теряется — этот класс дал 10 400 каталогов
# /tmp и диск на 100 % (проба red_docs_freeze.sh). Реестр в файле + trap EXIT переживает
# подоболочки и досрочный выход.
REG="$(mktemp)"
cleanup() { while read -r d; do [ -n "${d}" ] && rm -rf "${d}"; done < "${REG}"; rm -f "${REG}"; }
trap cleanup EXIT

mk() { # $1=имя переменной для пути
  local d; d="$(mktemp -d)"; echo "${d}" >> "${REG}"
  mkdir -p "${d}/crates/x/tests" "${d}/scripts"
  printf -v "$1" '%s' "${d}"
}

# Честный оракул: учёт потоковый.
honest_oracle() {
  cat > "$1" <<'RS'
use std::cell::Cell;
use std::alloc::{GlobalAlloc, Layout, System};
thread_local! {
    static T_CUR: Cell<usize> = const { Cell::new(0) };
}
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { System.alloc(l) }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p, l) }
}
#[global_allocator]
static GA: Counting = Counting;
RS
}

# Дефектный оракул: процессный счётчик, thread_local отсутствует.
broken_oracle() {
  cat > "$1" <<'RS'
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::alloc::{GlobalAlloc, Layout, System};
static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        let c = CUR.fetch_add(l.size(), SeqCst) + l.size();
        PEAK.fetch_max(c, SeqCst);
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p, l); }
}
#[global_allocator]
static GA: Counting = Counting;
#[test]
fn a() {}
#[test]
fn b() {}
RS
}

# Дефектный по форме, но БЕЗОПАСНЫЙ по существу: один тест в бинаре ⇒ соседей нет.
lonely_oracle() {
  broken_oracle "$1"
  python3 - "$1" <<'PY2'
import sys
p=sys.argv[1]; s=open(p).read()
open(p,'w').write(s.replace("#[test]\nfn b() {}\n",""))
PY2
}

run() { ROOT="$1" bash "${BARRIER}" >"$2" 2>&1; echo $?; }

# ── RO-1 — честный оракул ⇒ ПРОХОД (позитивный контроль) ─────────────────────────────────
# Без него проба может быть вечно-красной, и её «объявят шумом и выключат».
mk D; honest_oracle "${D}/crates/x/tests/red_alloc.rs"
RC="$(run "${D}" "${D}/out")"
[ "${RC}" -eq 0 ] && pass "RO-1 честный оракул (thread_local) проходит" \
                  || { fail "RO-1 честный оракул отвергнут — барьер запрещает правильное решение"; sed 's/^/      /' "${D}/out"; }

# ── RO-2 — ГЛАВНЫЙ СТАБ: процессный счётчик ⇒ БЛОК ───────────────────────────────────────
mk D; broken_oracle "${D}/crates/x/tests/red_alloc.rs"
RC="$(run "${D}" "${D}/out")"
[ "${RC}" -eq 1 ] && pass "RO-2 процессный счётчик заблокирован (exit=1)" \
                  || fail "RO-2 оракул с процессным счётчиком ПРОПУЩЕН (exit=${RC}) — это и есть механизм TD-098/TD-129"

# ── RO-3 — ноль оракулов ⇒ FAIL-CLOSED, а не зелёное ─────────────────────────────────────
# Гейт, зеленеющий от ИСЧЕЗНОВЕНИЯ предмета, — не гейт (урок CB-5).
mk D
RC="$(run "${D}" "${D}/out")"
[ "${RC}" -eq 2 ] && pass "RO-3 ноль найденных ⇒ exit=2 (детектор сломан ≠ чисто)" \
                  || fail "RO-3 пустое дерево дало exit=${RC} вместо 2 — барьер зеленеет от исчезновения предмета"

# ── RO-4 — оракул ВНЕ tests/ ⇒ найден (ширина детектора) ─────────────────────────────────
# Барьер, ищущий в одном каталоге, слеп к оракулу в соседнем.
mk D; mkdir -p "${D}/crates/y/src"; broken_oracle "${D}/crates/y/src/probe.rs"
RC="$(run "${D}" "${D}/out")"
[ "${RC}" -eq 1 ] && pass "RO-4 оракул в crates/*/src найден и заблокирован" \
                  || fail "RO-4 оракул вне tests/ ПРОПУЩЕН (exit=${RC}) — детектор привязан к каталогу"

# ── RO-5 — атрибут в иной форме ⇒ найден (уклонение от регулярки) ────────────────────────
mk D
{ echo 'use std::sync::atomic::AtomicUsize;'
  echo 'static CUR: AtomicUsize = AtomicUsize::new(0);'
  echo '#[ global_allocator ]'
  echo 'static GA: Counting = Counting;'
  # Соседи обязательны: по действующему инварианту одиночный тест НЕ дефект (RO-10),
  # и без них сценарий проверял бы не уклонение от регулярки, а отсутствие соседей.
  echo '#[test]'
  echo 'fn a() {}'
  echo '#[test]'
  echo 'fn b() {}'
} > "${D}/crates/x/tests/red_alloc.rs"
RC="$(run "${D}" "${D}/out")"
[ "${RC}" -eq 1 ] && pass "RO-5 атрибут с пробелами внутри скобок найден" \
                  || fail "RO-5 иная форма атрибута обошла детектор (exit=${RC})"

# ── RO-6 — target/ игнорируется (иначе барьер судит артефакты сборки) ────────────────────
mk D; honest_oracle "${D}/crates/x/tests/red_alloc.rs"
mkdir -p "${D}/target/debug"; broken_oracle "${D}/target/debug/copy.rs"
RC="$(run "${D}" "${D}/out")"
[ "${RC}" -eq 0 ] && pass "RO-6 копия в target/ не считается предметом" \
                  || fail "RO-6 барьер судит артефакты сборки (exit=${RC}) — красное на чужой копии"

# ── RO-7 — ROOT не каталог ⇒ exit=2 ──────────────────────────────────────────────────────
RC="$(ROOT=/nonexistent-$$ bash "${BARRIER}" >/dev/null 2>&1; echo $?)"
[ "${RC}" -eq 2 ] && pass "RO-7 негодный ROOT ⇒ exit=2" \
                  || fail "RO-7 негодный ROOT дал exit=${RC} вместо 2"

# ── RO-8 — перечисляются ВСЕ нарушители, а не первый ─────────────────────────────────────
mk D; broken_oracle "${D}/crates/x/tests/a.rs"; mkdir -p "${D}/crates/z/tests"; broken_oracle "${D}/crates/z/tests/b.rs"
RC="$(run "${D}" "${D}/out")"
N="$(grep -c '^FAIL  crates/' "${D}/out" || true)"
{ [ "${RC}" -eq 1 ] && [ "${N}" -eq 2 ]; } && pass "RO-8 названы оба нарушителя (${N})" \
                  || fail "RO-8 названо ${N} нарушителей из 2 (exit=${RC}) — чинить придётся по одному за круг"

# ── RO-9 — АНТИ-ПЛАЦЕБО САМОЙ ПРОБЫ: барьер-заглушка обязан быть пойман ──────────────────
# Проба, зелёная против барьера «всегда 0», не проверяет ничего.
STUB="$(mktemp)"; echo "${STUB}" >> "${REG}"
printf '#!/usr/bin/env bash\nexit 0\n' > "${STUB}"; chmod +x "${STUB}"
mk D; broken_oracle "${D}/crates/x/tests/red_alloc.rs"
RC="$(ROOT="${D}" bash "${STUB}" >/dev/null 2>&1; echo $?)"
[ "${RC}" -eq 0 ] && pass "RO-9 барьер-заглушка «всегда 0» даёт 0 — значит RO-2 его поймает" \
                  || fail "RO-9 контроль сломан: заглушка вернула ${RC}"

# ── RO-10 — процессный счётчик при ОДНОМ тесте ⇒ ПРОХОД (нет ложного срабатывания) ──────
# Соседей нет — портить замер некому. Работающее не чинят: барьер, требующий правки семи
# исправных оракулов, тратит круги и рискует ослепить их при касании.
mk D; lonely_oracle "${D}/crates/x/tests/red_alloc.rs"
[ "$(grep -c '^#\[test\]' "${D}/crates/x/tests/red_alloc.rs")" -eq 1 ] || die "RO-10 фикстура не одиночная"
RC="$(run "${D}" "${D}/out")"
[ "${RC}" -eq 0 ] && pass "RO-10 одиночный тест с процессным счётчиком не считается дефектом" \
                  || { fail "RO-10 ложное срабатывание на исправном файле (exit=${RC}) — барьер мерит ПРОКСИ, а не инвариант"; sed 's/^/      /' "${D}/out"; }

# ── RO-11 — МИНА: добавление второго теста делает файл дефектным ⇒ БЛОК ──────────────────
# Латентность обезвреживается именно здесь: барьер обязан покраснеть в ТОМ ЖЕ PR, где
# появился сосед, а не постфактум на красном `main`.
mk D; lonely_oracle "${D}/crates/x/tests/red_alloc.rs"
RC0="$(run "${D}" "${D}/out0")"
printf '#[test]\nfn neighbour() { let _v: Vec<u8> = Vec::with_capacity(1024); }\n' >> "${D}/crates/x/tests/red_alloc.rs"
RC1="$(run "${D}" "${D}/out1")"
{ [ "${RC0}" -eq 0 ] && [ "${RC1}" -eq 1 ]; } && pass "RO-11 второй тест переводит файл в дефектные (0 → 1)" \
                  || fail "RO-11 мина не сработала: до=${RC0} после=${RC1} (ожидалось 0 → 1)"

# ── C-095 F-1: ветка OK на СОДЕРЖАТЕЛЬНЫХ формах, а не на вырожденных ────────────────────
# Прежде КАЖДЫЙ сценарий, ждавший exit=0, кормил барьер файлом с НУЛЁМ или ОДНИМ тестом.
# Комбинация «потоковый учёт + ДВА и более теста» — то есть ровно форма всех четырёх
# правленых оракулов — не предъявлялась никогда. Поэтому стаб, судящий ТОЛЬКО по числу
# `#[test]`, давал 11/11 и при этом ложно блокировал шесть честных оракулов на реальном
# дереве. Три сценария ниже отличают ИНВАРИАНТ от ПРОКСИ.

two_tests() { printf '#[test]\nfn t_one() {}\n#[test]\nfn t_two() {}\n' >> "$1"; }

# RO-12 — честный thread_local + ДВА теста ⇒ ПРОХОД. Убивает стаб «блокирую всё, где тестов ≥2».
mk D; honest_oracle "${D}/crates/x/tests/red_alloc.rs"; two_tests "${D}/crates/x/tests/red_alloc.rs"
[ "$(grep -c '^#\[test\]' "${D}/crates/x/tests/red_alloc.rs")" -eq 2 ] || die "RO-12 фикстура не двухтестовая"
ROOT="${D}" bash "${BARRIER}" > "${D}/out" 2>&1; RC=$?
[ "${RC}" -eq 0 ] && pass "RO-12 потоковый учёт + два теста — не дефект" \
  || { fail "RO-12 ложное срабатывание (exit=${RC}): барьер судит по ЧИСЛУ ТЕСТОВ, а не по учёту"; sed 's/^/      /' "${D}/out"; }

# RO-13 — аллокатор БЕЗ единого счётчика + два теста ⇒ ПРОХОД. Убивает стаб, судящий по
# наличию `thread_local!` как таковому: счётчиков здесь нет вовсе, портиться нечему.
mk D; cat > "${D}/crates/x/tests/red_alloc.rs" <<'RS'
use std::alloc::{GlobalAlloc, Layout, System};
struct Passthrough;
unsafe impl GlobalAlloc for Passthrough {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { System.alloc(l) }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p, l) }
}
#[global_allocator]
static GA: Passthrough = Passthrough;
RS
two_tests "${D}/crates/x/tests/red_alloc.rs"
ROOT="${D}" bash "${BARRIER}" > "${D}/out" 2>&1; RC=$?
[ "${RC}" -eq 0 ] && pass "RO-13 аллокатор без счётчиков + два теста — не дефект" \
  || { fail "RO-13 ложное срабатывание (exit=${RC})"; sed 's/^/      /' "${D}/out"; }

# RO-14 — thread_local со СВОИМ именем счётчика + служебный процессный атомик + два теста
# ⇒ ПРОХОД. Убивает стаб, ищущий конкретный идентификатор (`T_CUR`) вместо потокового УЧЁТА.
mk D; cat > "${D}/crates/x/tests/red_alloc.rs" <<'RS'
use std::cell::Cell;
use std::sync::atomic::AtomicBool;
use std::alloc::{GlobalAlloc, Layout, System};
thread_local! {
    static MY_OWN_METER: Cell<usize> = const { Cell::new(0) };
}
static MEASURING: AtomicBool = AtomicBool::new(false);
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { System.alloc(l) }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p, l) }
}
#[global_allocator]
static GA: Counting = Counting;
RS
two_tests "${D}/crates/x/tests/red_alloc.rs"
ROOT="${D}" bash "${BARRIER}" > "${D}/out" 2>&1; RC=$?
[ "${RC}" -eq 0 ] && pass "RO-14 своё имя счётчика + служебный атомик — не дефект" \
  || { fail "RO-14 ложное срабатывание (exit=${RC}): барьер пиннит ИМЯ, а не потоковый учёт"; sed 's/^/      /' "${D}/out"; }

# RO-15 — F-2: `global_allocator` ТОЛЬКО в комментарии ⇒ файл НЕ оракул. Барьер обязан уйти
# в fail-closed «оракулов не найдено» (exit=2), а не молча зачесть его и не заблокировать.
mk D; cat > "${D}/crates/x/tests/red_fake.rs" <<'RS'
// Упоминание global_allocator в комментарии оракулом файл не делает.
// #[global_allocator]
static CNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
RS
two_tests "${D}/crates/x/tests/red_fake.rs"
ROOT="${D}" bash "${BARRIER}" > "${D}/out" 2>&1; RC=$?
[ "${RC}" -eq 2 ] && pass "RO-15 упоминание в комментарии не считается объявлением (fail-closed)" \
  || { fail "RO-15 exit=${RC}, ожидалось 2: барьер путает УПОМИНАНИЕ с ОБЪЯВЛЕНИЕМ"; sed 's/^/      /' "${D}/out"; }

# RO-16 — F-3: процессный счётчик, заданный ПОЛНЫМ путём типа, обязан ловиться.
mk D; cat > "${D}/crates/x/tests/red_alloc.rs" <<'RS'
use std::alloc::{GlobalAlloc, Layout, System};
pub static CUR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { System.alloc(l) }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { System.dealloc(p, l) }
}
#[global_allocator]
static GA: Counting = Counting;
RS
two_tests "${D}/crates/x/tests/red_alloc.rs"
ROOT="${D}" bash "${BARRIER}" > "${D}/out" 2>&1; RC=$?
[ "${RC}" -eq 1 ] && pass "RO-16 'pub static' с полным путём типа ловится" \
  || { fail "RO-16 exit=${RC}, ожидалось 1: регулярка не знает 'pub static' / 'std::sync::atomic::'"; sed 's/^/      /' "${D}/out"; }


# ── АГРЕГАТОР — ЕДИНСТВЕННЫЙ И ПОСЛЕДНИЙ (`C-097` H-1) ───────────────────────────────────
# Прежде их было два: ранний (после RO-11) роняющий прогон, и финальный, печатавший
# «PASS (${PASSED}/${PASSED})» — то есть ВСЕГДА «N из N», сколько бы ни упало. Сценарии
# RO-12…RO-16, добавленные ПОСЛЕ раннего блока, печатали `FAIL` и не влияли ни на вердикт,
# ни на код возврата: проба была зелёной против сломанного барьера. Это ровно то плацебо,
# против которого барьер и написан, — и оно жило в самой пробе.
echo
echo "каталогов-фикстур в реестре: $(wc -l < "${REG}") (уборка — trap EXIT)"
echo "сценариев: $((PASSED + FAILED))   PASS: ${PASSED}   FAIL: ${FAILED}"
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} из $((PASSED + FAILED)))"
  echo "Пока проба красная, повторим класс TD-098/TD-129: обязательный чек main становится"
  echo "лотереей, а правильной реакцией на красное объявляется «перезапусти»."
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/$((PASSED + FAILED))) — барьер ловит процессный учёт, не ловит ложно, fail-closed на пустоте"
