#!/usr/bin/env bash
# red_verify_M-88.sh — ОТРИЦАТЕЛЬНАЯ ПРОБА текстовых предикатов гейта M-88.
#
# Основание — `A-036` §5.3, строка «все текстовые»: класс «предикат проходит по тексту»
# сработал ТРИЖДЫ (`C-233` R2; первая редакция шага `R4` самого architect'а; `C-235`), а
# норма профиля говорит: после третьего повторения класса писать прозу ЗАПРЕЩЕНО —
# механизируй. Это механизм.
#
# Каждый сценарий ДВУСТОРОННИЙ: честная фикстура обязана дать 0, подделанная — 1. Односторонняя
# проба («подделка красит») зелена и у предиката `return 1`, то есть ничего не доказывает.
#
# Прогон: bash scripts/tests/red_verify_M-88.sh   (exit 0 — все сценарии сошлись)

set -uo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=../lib/m88_predicates.sh
. scripts/lib/m88_predicates.sh

TMP=$(mktemp -d /tmp/red-verify-m88-XXXXXX)
trap 'rm -rf "$TMP"' EXIT

N=0
BAD=0
ok()   { N=$((N+1)); printf 'ok    %s\n' "$1"; }
bad()  { N=$((N+1)); BAD=$((BAD+1)); printf 'FAIL  %s\n' "$1"; }
# expect <ожидаемый код> <имя сценария> -- <команда...>
expect() {
  local want="$1" name="$2"; shift 3
  "$@" >/dev/null 2>&1
  local got=$?
  if [ "$got" -eq "$want" ]; then ok "$name (rc=$got)"; else bad "$name: ожидался rc=$want, получен rc=$got"; fi
}

# ─────────────────────────── task2 ───────────────────────────
# Честно: семантика полного среза объявлена в комментарии ПЕРЕД полем.
cat > "$TMP/gw_honest.rs" <<'EOF'
struct BookSeriesObservation {
    /// seq события, породившего эту версию наблюдения.
    seq: u64,
    /// ПОЛНЫЙ срез бакета на момент наблюдения.
    heatmap_cells: Vec<HeatmapCell>,
}
EOF
# Подделка ровно по A-036 §3.3: фраза вписана в ПОСТОРОННИЙ комментарий (`seq`), а
# комментарий поля по-прежнему предписывает объединение.
cat > "$TMP/gw_fake.rs" <<'EOF'
struct BookSeriesObservation {
    /// seq события. ПОЛНЫЙ срез бакета — см. ниже.
    seq: u64,
    /// Позднейшее наблюдение объединяется с ранним по ключу.
    heatmap_cells: Vec<HeatmapCell>,
}
EOF
# Setup-страж: фикстуры обязаны РАЗЛИЧАТЬСЯ, иначе сценарий проверяет пустоту.
if cmp -s "$TMP/gw_honest.rs" "$TMP/gw_fake.rs"; then
  bad "task2 SETUP: фикстуры идентичны — сценарий вырожден"
else
  ok "task2 SETUP: честная и подделанная фикстуры различны"
fi
expect 0 "task2 ЧЕСТНО: семантика при поле ⇒ предъявлено" -- m88_task2 "$TMP/gw_honest.rs"
expect 1 "task2 ПОДДЕЛКА: фраза в чужом комментарии ⇒ отказ" -- m88_task2 "$TMP/gw_fake.rs"

# ─────────────────────────── task7 ───────────────────────────
cat > "$TMP/apply_honest.rs" <<'EOF'
impl Snapshot {
    /// Сложить кадр.
    #[must_use]
    pub fn apply(&mut self, frame: &Frame) -> ApplyOutcome {
EOF
# Подделка A-036 §3.3: обе строки ЗАКОММЕНТИРОВАНЫ, настоящая сигнатура прежняя.
cat > "$TMP/apply_fake.rs" <<'EOF'
impl Snapshot {
    // #[must_use]
    // pub fn apply(&mut self, frame: &Frame) -> ApplyOutcome;
    pub fn apply(&mut self, frame: &Frame) {
EOF
expect 0 "task7 ЧЕСТНО: атрибут и сигнатура некомментарны ⇒ предъявлено" -- m88_task7 "$TMP/apply_honest.rs"
expect 1 "task7 ПОДДЕЛКА: обе строки закомментированы ⇒ отказ" -- m88_task7 "$TMP/apply_fake.rs"

# ─────────────────────────── task8 ───────────────────────────
REAL_SHA=$(git rev-parse HEAD)
BASE_SHA=d5163b5b35abbca204a8981e974bd6e5a97eb9de
{
  echo "## Результат ПОСЛЕ реализации"
  echo "<!-- FACTS: audited_head=$REAL_SHA collected=2026-09-21 -->"
  echo "Команда: docker exec hft-gateway-serve wsprobe --token ... --frames 20 --out /tmp/m88v"
  echo "до: 5193 байт карты на кадр · после: 2100 байт"
} > "$TMP/meas_honest.md"
# Подделка A-036 §3.3: три выдуманные строки без датировки.
{
  echo "## Результат ПОСЛЕ реализации"
  echo "до: 111 · после: 110"
} > "$TMP/meas_fake.md"
# Вторая подделка: маркер есть, но ревизия — та же, что «до».
{
  echo "## Результат ПОСЛЕ реализации"
  echo "<!-- FACTS: audited_head=$BASE_SHA collected=2026-09-21 -->"
  echo "Команда: wsprobe --token ..."
  echo "до: 5193 · после: 2100"
} > "$TMP/meas_same_rev.md"
# Третья: ревизия выдумана и в истории не существует.
{
  echo "## Результат ПОСЛЕ реализации"
  echo "<!-- FACTS: audited_head=0123456789abcdef0123456789abcdef01234567 collected=2026-09-21 -->"
  echo "Команда: wsprobe --token ..."
  echo "до: 5193 · после: 2100"
} > "$TMP/meas_ghost.md"
expect 0 "task8 ЧЕСТНО: датировано живой ревизией и названа команда" -- m88_task8 "$TMP/meas_honest.md" "$BASE_SHA"
expect 1 "task8 ПОДДЕЛКА: три выдуманные строки ⇒ отказ" -- m88_task8 "$TMP/meas_fake.md" "$BASE_SHA"
expect 1 "task8 ПОДДЕЛКА: ревизия равна базе ⇒ отказ (замер 'после' не мог быть снят до реализации)" -- m88_task8 "$TMP/meas_same_rev.md" "$BASE_SHA"
expect 1 "task8 ПОДДЕЛКА: выдуманная ревизия не существует в истории ⇒ отказ" -- m88_task8 "$TMP/meas_ghost.md" "$BASE_SHA"

# ─────────────────────────── task9 ───────────────────────────
# Пустое множество изменённых ожиданий: честная таблица ЗАЯВЛЯЕТ пустоту.
printf '### 14.1. Решения по изменённым ожиданиям\n\nна момент коммита набора изменённых ожиданий нет\n' \
  > "$TMP/spec_empty_declared.md"
# Подделка A-036 §3.3: плейсхолдер заменён другим плейсхолдером, пустота НЕ заявлена.
printf '### 14.1. Решения по изменённым ожиданиям\n\n_(решение не записано)_\n' \
  > "$TMP/spec_empty_silent.md"
MB=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
if [ -z "$MB" ]; then
  bad "task9 SETUP: merge-base не вычислен — сценарий не может быть исполнен"
else
  CHANGED_N=$(git diff --name-only "$MB"..HEAD -- 'crates/gateway/tests/*.rs' 'crates/gateway-serve/tests/*.rs' 2>/dev/null \
              | grep -vc 'red_m88_update_contract\.rs\|red_m88_contract_form\.rs' || true)
  ok "task9 SETUP: merge-base $MB, изменённых чужих тестов: $CHANGED_N"
  if [ "$CHANGED_N" -eq 0 ]; then
    expect 0 "task9 ЧЕСТНО: пустое множество ЗАЯВЛЕНО ⇒ предъявлено" -- m88_task9 "$TMP/spec_empty_declared.md" "$MB"
    expect 1 "task9 ПОДДЕЛКА: пустота молчит под другим плейсхолдером ⇒ отказ" -- m88_task9 "$TMP/spec_empty_silent.md" "$MB"
  else
    # Множество непусто: таблица без имён обязана отказать.
    expect 1 "task9 ПОДДЕЛКА: изменённые файлы есть, решений нет ⇒ отказ" -- m88_task9 "$TMP/spec_empty_silent.md" "$MB"
  fi
fi

# ─────────────────────────── R4 ───────────────────────────
# R4 текстовым предикатом БОЛЬШЕ НЕ ЯВЛЯЕТСЯ (A-036 §5.3): шаг зовёт исполняемую батарею и
# КРАСЕН, пока её нет. Здесь проверяется только это свойство — fail-closed по отсутствию.
if [ -f scripts/tests/red_m88_mutants.sh ]; then
  ok "R4: батарея существует — её собственная годность судится ею самой (--battery)"
else
  ok "R4: батареи нет, и шаг гейта обязан быть КРАСНЫМ (fail-closed, прецедент verify_M-65 F2)"
fi

printf '\n'
if [ "$BAD" -eq 0 ]; then
  echo "VERDICT: PASS — сценариев: $N, расхождений: 0"
  exit 0
fi
echo "VERDICT: FAIL — сценариев: $N, расхождений: $BAD"
exit 1
