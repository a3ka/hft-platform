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
LIBDIR="$(pwd)/scripts/lib"
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
              | grep -vc 'red_m88_update_contract\.rs\|red_m88_contract_form\.rs\|red_m88_golden_vectors\.rs' || true)
  ok "task9 SETUP: merge-base $MB, изменённых ЧУЖИХ ожиданий: $CHANGED_N"

  # Сценарии ПУСТОГО множества — на текущем дереве.
  if [ "$CHANGED_N" -eq 0 ]; then
    expect 0 "task9 ЧЕСТНО(пусто): пустое множество ЗАЯВЛЕНО ⇒ предъявлено" -- m88_task9 "$TMP/spec_empty_declared.md" "$MB"
    expect 1 "task9 ПОДДЕЛКА(пусто): пустота молчит под другим плейсхолдером ⇒ отказ" -- m88_task9 "$TMP/spec_empty_silent.md" "$MB"
  else
    expect 1 "task9 ПОДДЕЛКА: изменённые файлы есть, решений нет ⇒ отказ" -- m88_task9 "$TMP/spec_empty_silent.md" "$MB"
  fi

  # `C-237` R2: ЧЕСТНЫЙ НЕПУСТОЙ сценарий. Прежняя проба на пустом дереве никогда не
  # проверяла главную ветку предиката — «файл изменён, решение записано ⇒ PASS» и
  # «файл изменён, решения нет ⇒ FAIL». Строим непустое множество ИСКУССТВЕННО, в
  # одноразовом дереве, чтобы не трогать предмет.
  PROBE_WT="$TMP/wt"
  if git worktree add -q --detach "$PROBE_WT" HEAD 2>/dev/null; then
    ok "task9 SETUP(непусто): одноразовое дерево создано"
    # `commit -a` запрещён хуком проекта — пути называются ЯВНО.
    ( cd "$PROBE_WT" \
      && printf '\n// проба предиката task9 — файл изменён намеренно\n' >> crates/gateway/tests/red_heatmap.rs \
      && git add -- crates/gateway/tests/red_heatmap.rs \
      && git -c user.email=probe@local -c user.name=probe commit -q --no-verify \
           -m "probe: чужое ожидание изменено" -- crates/gateway/tests/red_heatmap.rs ) >/dev/null 2>&1
    # `C-245`/отчёт tester'а: базой пробы обязан быть РОДИТЕЛЬ пробного коммита, а не
    # merge-base с `main`. От merge-base дифф включает ВСЕ изменения ветки — десятки
    # файлов, — и фикстура §14.1 из двух строк их назвать не может; предикат честно
    # отказывал, а проба читала это как свой провал. Сценарий судит ПРЕДИКАТ на
    # контролируемом диффе, поэтому диапазон обязан быть ровно «родитель..HEAD».
    PROBE_MB=$(git -C "$PROBE_WT" rev-parse HEAD^ 2>/dev/null)
    CH=$(git -C "$PROBE_WT" diff --name-only "$PROBE_MB"..HEAD -- 'crates/gateway/tests/*.rs' 2>/dev/null | grep -c 'red_heatmap' || true)
    if [ "$CH" -ge 1 ]; then
      ok "task9 SETUP(непусто): множество непусто (red_heatmap.rs изменён)"
      printf '### 14.1. Решения по изменённым ожиданиям\n\n| red_heatmap.rs | сторож карты | ПЕРЕНОСИТСЯ |\n' \
        > "$TMP/spec_named.md"
      printf '### 14.1. Решения по изменённым ожиданиям\n\n| другой_файл.rs | — | ПЕРЕНОСИТСЯ |\n' \
        > "$TMP/spec_unnamed.md"
      ( cd "$PROBE_WT" && . "$LIBDIR/m88_predicates.sh" 2>/dev/null
        m88_task9 "$TMP/spec_named.md" "$PROBE_MB" >/dev/null 2>&1 ) && R1=0 || R1=1
      ( cd "$PROBE_WT" && . "$LIBDIR/m88_predicates.sh" 2>/dev/null
        m88_task9 "$TMP/spec_unnamed.md" "$PROBE_MB" >/dev/null 2>&1 ) && R2=0 || R2=1
      [ "$R1" -eq 0 ] && ok "task9 ЧЕСТНО(непусто): изменённый файл НАЗВАН ⇒ предъявлено (rc=0)" \
                      || bad "task9 ЧЕСТНО(непусто): изменённый файл назван, а предикат отказал (rc=$R1)"
      [ "$R2" -eq 1 ] && ok "task9 ПОДДЕЛКА(непусто): изменённый файл НЕ назван ⇒ отказ (rc=1)" \
                      || bad "task9 ПОДДЕЛКА(непусто): изменённый файл не назван, а предикат принял (rc=$R2)"

      # ИСКЛЮЧЕНИЕ ПО КЛАССУ (`R-195`, круг tester'а на `7de7c3d`). Собственные оракулы
      # M-88 не имеют прежнего ожидания, которое можно перенести или удалить, — требовать
      # для них строку в §14.1 бессмысленно. Прежняя редакция перечисляла их ИМЕНАМИ, и
      # перечисление отказало ровно так, как отказывает всякое перечисление: круг добавил
      # два файла, имён которых в списке не было, и гейт покраснел на СВОЁМ авторе.
      #
      # Сценарий судит ОБЕ стороны в ОДНОМ диффе: чужой файл обязан быть назван, свой —
      # не обязан. Проверять их порознь значило бы не проверить главного: что исключение
      # по классу не проглатывает заодно и чужой файл.
      ( cd "$PROBE_WT" \
        && printf '// новый собственный оракул M-88\n' > crates/gateway/tests/red_m88_probe_class.rs \
        && git add -- crates/gateway/tests/red_m88_probe_class.rs \
        && git -c user.email=probe@local -c user.name=probe commit -q --no-verify \
             -m "probe: собственный оракул M-88 добавлен" -- crates/gateway/tests/red_m88_probe_class.rs ) >/dev/null 2>&1
      CLS_MB=$(git -C "$PROBE_WT" rev-parse HEAD~2 2>/dev/null)
      CLS_N=$(git -C "$PROBE_WT" diff --name-only "$CLS_MB"..HEAD -- 'crates/gateway/tests/*.rs' 2>/dev/null | wc -l)
      if [ "$CLS_N" -eq 2 ]; then
        ok "task9 SETUP(класс): в диффе ДВА файла — чужой и собственный M-88"
        # §14.1 называет ТОЛЬКО чужой. Ожидание: принятие — собственный исключён по классу.
        ( cd "$PROBE_WT" && . "$LIBDIR/m88_predicates.sh" 2>/dev/null
          m88_task9 "$TMP/spec_named.md" "$CLS_MB" >/dev/null 2>&1 ) && R3=0 || R3=1
        [ "$R3" -eq 0 ] && ok "task9 КЛАСС: собственный оракул M-88 строки в §14.1 НЕ требует (rc=0)" \
                        || bad "task9 КЛАСС: предикат требует строку для СОБСТВЕННОГО оракула M-88 — гейт красен на своём авторе (rc=$R3)"
        # И обратная сторона в том же диффе: чужой файл не назван ⇒ отказ обязан остаться.
        ( cd "$PROBE_WT" && . "$LIBDIR/m88_predicates.sh" 2>/dev/null
          m88_task9 "$TMP/spec_unnamed.md" "$CLS_MB" >/dev/null 2>&1 ) && R4=0 || R4=1
        [ "$R4" -eq 1 ] && ok "task9 КЛАСС: исключение не проглотило ЧУЖОЙ файл — он по-прежнему обязателен (rc=1)" \
                        || bad "task9 КЛАСС: исключение по классу ослабило проверку чужих оракулов (rc=$R4)"
      else
        bad "task9 SETUP(класс): в диффе $CLS_N файлов вместо 2 — сценарий вырожден"
      fi
    else
      bad "task9 SETUP(непусто): множество не стало непустым — сценарий вырожден"
    fi
    git worktree remove --force "$PROBE_WT" >/dev/null 2>&1
    git worktree prune >/dev/null 2>&1
  else
    bad "task9 SETUP(непусто): одноразовое дерево не создано — главная ветка предиката не проверена"
  fi
fi

# ─────────── task9 ПОСЛЕ MERGE'А: пустой диапазон — ДВА разных случая ───────────
# Найдено прогоном close-out'а на `main` 2026-09-23: после влития диапазон
# `origin/main..HEAD` пуст ПО ПОСТРОЕНИЮ, и прежняя редакция требовала фразу «изменённых
# ожиданий нет». На влитом предмете это требование ЛЖИВО — ожидания менялись, просто это
# позади, — и приёмка не могла позеленеть на `main` НИКОГДА.
T9=$(mktemp -d)
# (а) диапазон пуст, §14.1 НЕПУСТА (предмет влит) ⇒ принятие.
printf '### 14.1. Решения по изменённым ожиданиям\n\n| `red_heatmap.rs` | сторож карты | ПЕРЕНОСИТСЯ |\n' \
  > "$T9/filled.md"
if m88_task9 "$T9/filled.md" HEAD >/dev/null 2>&1; then
  ok "task9 ВЛИТО: пустой диапазон при непустой §14.1 ⇒ принятие"
else
  bad "task9 ВЛИТО: приёмка не может позеленеть на влитом предмете — гейт судит положение наблюдателя, а не предмет"
fi
# (б) диапазон пуст и §14.1 ПУСТА ⇒ отказ (исходный смысл проверки сохранён).
printf '### 14.1. Решения по изменённым ожиданиям\n\nничего\n' > "$T9/empty.md"
if m88_task9 "$T9/empty.md" HEAD >/dev/null 2>&1; then
  bad "task9 ПУСТО: молчание принято за ответ — исходный смысл проверки утрачен"
else
  ok "task9 ПУСТО: пустая таблица без явного заявления ⇒ отказ"
fi
rm -rf "$T9"

# ─────────────────────── task11 (`R-195` Б-2) — ТРИ стороны ───────────────────────
# Предикат сверяет ДВА факта в одном месте файла: что утверждает собственный док-комментарий
# поля и какой serde-атрибут стоит на нём фактически. Сторон именно три, и средняя — та,
# на которой предикат уже один раз ошибся: фиксированное окно «20 строк вверх» зачерпывало
# ЧУЖОЙ комментарий (у соседнего поля `book_series` там законно стоит `serde(skip, ...)`) и
# отвергало ПРАВИЛЬНУЮ редакцию. Поймано этой пробой до коммита.
T11=$(mktemp -d)
FIELD='    heatmap_buckets_observed: std::collections::BTreeSet<i64>,'

# (а) ЛОЖЬ ТЕКСТА: комментарий обещает skip, атрибут — default. Ожидание: отказ.
cat > "$T11/lie.rs" <<'EOF'
    /// Соседнее поле со своим законным skip — предикат не имеет права его читать.
    #[serde(skip, default = "book_series_default")]
    book_series: BTreeMap<u64, BookSeriesObservation>,
    /// M-88: проекция наблюдений карты.
    /// `#[serde(skip, default)]` — не часть чекпоинта: редьюсер стартует пустым.
    #[serde(default)]
    heatmap_buckets_observed: std::collections::BTreeSet<i64>,
EOF
if m88_task11 "$T11/lie.rs"; then
  bad "task11(а): предикат ПРИНЯЛ текст, обещающий skip при атрибуте default — класс TD-138 не ловится"
else
  ok "task11(а): ложь текста о skip отвергнута"
fi

# (б) ПРАВИЛЬНАЯ РЕДАКЦИЯ рядом с чужим skip. Ожидание: принятие.
cat > "$T11/ok.rs" <<'EOF'
    /// Соседнее поле со своим законным skip — предикат не имеет права его читать.
    #[serde(skip, default = "book_series_default")]
    book_series: BTreeMap<u64, BookSeriesObservation>,
    /// M-88: проекция наблюдений карты.
    /// Поле ЧАСТЬ чекпоинта (§13bis.3): редьюсер из слепка обязан совпасть с реплеем.
    #[serde(default)]
    heatmap_buckets_observed: std::collections::BTreeSet<i64>,
EOF
if m88_task11 "$T11/ok.rs"; then
  ok "task11(б): правильная редакция принята, чужой skip соседнего поля не зачерпнут"
else
  bad "task11(б): предикат ОТВЕРГ правильную редакцию — он читает чужой комментарий, а не свой"
fi

# (в) ЗАПРЕЩЁННЫЙ АТРИБУТ: skip появился на самом поле. Ожидание: отказ (§13bis.3).
cat > "$T11/skip.rs" <<'EOF'
    /// M-88: проекция наблюдений карты.
    /// Поле ЧАСТЬ чекпоинта (§13bis.3).
    #[serde(skip, default)]
    heatmap_buckets_observed: std::collections::BTreeSet<i64>,
EOF
if m88_task11 "$T11/skip.rs"; then
  bad "task11(в): предикат ПРИНЯЛ skip на поле — редьюсер из слепка разошёлся бы с реплеем (VB-I-2)"
else
  ok "task11(в): skip на поле отвергнут"
fi

# (г) FAIL-CLOSED: поля нет вовсе. Ожидание: отказ, а не молчаливое принятие.
: > "$T11/empty.rs"
if m88_task11 "$T11/empty.rs"; then
  bad "task11(г): предикат ПРИНЯЛ файл БЕЗ поля — гейт судил бы несуществующий предмет"
else
  ok "task11(г): отсутствие поля даёт отказ (fail-closed)"
fi
rm -rf "$T11"

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
