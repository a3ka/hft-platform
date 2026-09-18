#!/usr/bin/env bash
# Проба CAS-резерва номеров артефактов — `scripts/reserve_artifact_id.sh`.
# Решение founder'а 2026-08-15; конструкция и замеры — `docs/plans/process-decisions-2026-08-14.md`
# §Р-1, advisory критика — `research/critiques/C-084-process-decisions.md` N-1..N-3.
#
# ИНВАРИАНТ, который проба защищает:
#   номер, напечатанный обёрткой, ЗАНЯТ на `origin` в тот же момент, когда напечатан, —
#   то есть НИ ОДИН другой резервирующий не может получить тот же номер, как бы ни совпали
#   их вершины, время и порядок. Всё остальное (уборка, наблюдаемость хвоста, коды возврата)
#   — обслуживание этого инварианта.
#
# ГЕРМЕТИЧНОСТЬ: `origin` — ЛОКАЛЬНЫЙ bare-репозиторий в `/tmp`; сети не требуется ни одному
# сценарию. Причина не в удобстве: проба, ходящая в GitHub, мерила бы доступность сети, а не
# свой инвариант (`testing.md` «целостность гейта», свойство 2 — тот же класс, что `TD-135`).
#
# АНТИ-ПЛАЦЕБО — ГЛАВНОЕ здесь. Механизм целиком держится на ОДНОЙ идее: SHA резервного
# коммита уникален на резервирующего. Наивная реализация (push вершины `main` в
# `refs/reserved/<ID>`) выглядит рабочей и проходит ручную проверку: первый push создаёт ref,
# и «всё работает». Она ломается ровно там, где механизм и нужен, — у двух ролей с ОДНОЙ
# вершиной: push ТОГО ЖЕ SHA даёт «Everything up-to-date», exit=0, и обе роли считают номер
# своим (замер §Р-1 E2, воспроизведён на живом GitHub — P3). Поэтому мутант `sametip` — не
# один из девяти, а ОРАКУЛ ВСЕГО МЕХАНИЗМА: набор, зелёный против него, ничего не проверяет.
#
# ДВА РЕЖИМА:
#   red_reserve_id.sh             — 15 сценариев против ЧЕСТНОЙ реализации (позитивный
#                                   контроль: обязана быть зелёной целиком)
#   red_reserve_id.sh --battery   — 9 мутантов; каждый обязан уронить РОВНО заявленное
#                                   множество сценариев — ни больше, ни меньше
# Равенство множеств, а не «мутант где-то покраснел», — по уроку `red_artifact_ids.sh`:
# у слабой формы нет нижней границы (мутант, уронивший setup пробы, засчитывался как
# «закреплён») и нет верхней (мутант, сломанный сверх своей дыры, выглядел точечным).
#
# KILL-SET'Ы ИЗМЕРЕНЫ ПРОГОНОМ, а не выведены рассуждением (`testing.md`: «числа СЧИТАНЫ, а
# не заявлены»). Три измеренных результата разошлись с ожиданием и внесены как измерены:
#   * `sametip` роняет ПЯТЬ сценариев, а не три: LIST и SWEEP тоже строят по два резерва, и
#     реализация, выдающая один номер дважды, оставляет на origin один ref вместо двух;
#   * `forcepush` неточечный (девять сценариев) — `--force` убивает CAS целиком, а не только
#     кражу чужого ref'а. Неточечность названа, а не подогнана сужением мутанта;
#   * `weaknonce` роняет и NONCESRC, и NONCEDEG — тихий откат на `$RANDOM` обходит ОБЕ
#     проверки источника, что и есть причина, по которой отката в реализации нет.
#
# ФИКСТУРЫ УБИРАЮТСЯ: реестр — ФАЙЛ (не переменная: `d="$(mk_fix …)"` исполняется в
# ПОДОБОЛОЧКЕ, и присваивание в родителя не возвращается — на этом классе `red_artifact_ids`
# накопил 37 547 каталогов в `/tmp` при живом `trap`), `trap EXIT`, и число оставшихся
# каталогов ПЕЧАТАЕТСЯ, а не подразумевается (`docs/workflow/harness-track.md` §5 п.5).

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET="${TARGET:-${ROOT}/scripts/reserve_artifact_id.sh}"
ALLOC_SH="${ALLOC_SH:-${ROOT}/scripts/next_artifact_id.sh}"
ROLE_TAG="probe-role"
MODE="${1:-scenarios}"

[ -r "${TARGET}" ]   || { echo "SETUP НЕ СОСТОЯЛСЯ: нет ${TARGET}" >&2; exit 1; }
[ -r "${ALLOC_SH}" ] || { echo "SETUP НЕ СОСТОЯЛСЯ: нет ${ALLOC_SH}" >&2; exit 1; }

FAILED=0; PASSED=0
FAILED_NAMES=""
REG="$(mktemp /tmp/red-reserve-reg-XXXXXX)" || { echo "реестр фикстур не создан" >&2; exit 1; }

die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

cleanup() {
  local d left
  if [ "${KEEP_FIXTURES:-0}" = "1" ]; then echo "фикстуры сохранены: ${REG}" >&2; return; fi
  # `-e`, а не `-d`: в реестр попадают и временные ФАЙЛЫ (обмен результатом с подоболочкой
  # батареи). Первая редакция чистила только каталоги, и файл переживал прогон — ровно тот
  # класс «механизм объявлен и мёртв», ради которого реестр вообще ведётся файлом.
  while IFS= read -r d; do
    [ -n "$d" ] && [ -e "$d" ] && case "$d" in /tmp/red-reserve-*) rm -rf "$d";; esac
  done < "${REG}"
  rm -f "${REG}"
  # Число, а не обещание: сколько каталогов пробы осталось в /tmp ПОСЛЕ уборки.
  left="$(find /tmp -maxdepth 1 -name 'red-reserve-*' 2>/dev/null | wc -l | tr -d ' ')"
  echo "фикстур осталось в /tmp после уборки: ${left}" >&2
}
trap cleanup EXIT

# ─── фикстура: локальный bare-origin + корпус артефактов + N клонов ──────────────────
# Корпус даёт аллокатору максимумы: C-084 → следующий C-085 (все сценарии класса C),
# M-60 → M-61 (сценарий формата), и НЕ содержит ни одного артефакта класса A — на этом
# стоит сценарий ALLOCFAIL (аллокатор обязан умереть setup-guard'ом, а резерв — за ним).
mk_fix() {
  local d n cnt="${2:-1}"
  d="$(mktemp -d "/tmp/red-reserve-$1-XXXXXX")" || die "mktemp $1"
  printf '%s\n' "$d" >> "${REG}"
  git init -q --bare -b main "$d/origin.git" >/dev/null 2>&1 || die "bare $1"
  git init -q -b main "$d/seed" >/dev/null 2>&1 || die "seed $1"
  ( cd "$d/seed" \
    && git config user.email a@b.c && git config user.name t \
    && mkdir -p research/critiques research/reviews milestones \
    && echo x > research/critiques/C-084-x.md \
    && echo x > research/reviews/R-085-x.md \
    && echo x > milestones/M-60-x.md \
    && printf '# TECH-DEBT\n\n- **TD-141** `x`\n' > TECH-DEBT.md \
    && git add -A && git commit -q -m base \
    && git remote add origin "$d/origin.git" && git push -q origin main ) >/dev/null 2>&1 \
    || die "наполнение фикстуры $1"
  for n in $(seq 1 "${cnt}"); do
    git clone -q "$d/origin.git" "$d/c$n" >/dev/null 2>&1 || die "clone $n ($1)"
  done
  printf '%s\n' "$d"
}

# ЧУЖОЙ резерв. SHA обязан отличаться и от вершины main, и от коммитов обёртки: если занять
# ref вершиной main, наивный мутант `sametip` пушил бы в него ТОТ ЖЕ SHA, получал бы
# «up-to-date» и выглядел бы сломанным на сценариях, к его дыре отношения не имеющих, —
# kill-set перестал бы что-либо значить.
occupy() {
  local d="$1"; shift
  local sha tree
  tree="$(git -C "${d}/origin.git" rev-parse main^{tree})" || die "occupy: дерево"
  sha="$(GIT_AUTHOR_NAME=foreign GIT_AUTHOR_EMAIL=f@f GIT_COMMITTER_NAME=foreign \
         GIT_COMMITTER_EMAIL=f@f git -C "${d}/origin.git" commit-tree "${tree}" \
         -m "foreign reserve (не наш коммит)")" || die "occupy: commit-tree"
  { for id in "$@"; do printf 'create refs/reserved/%s %s\n' "${id}" "${sha}"; done; } \
    | git -C "${d}/origin.git" update-ref --stdin || die "occupy: update-ref"
  printf '%s\n' "${sha}"
}
occupy_range() {  # $1=фикстура $2=класс $3=от $4=до
  local d="$1" cls="$2" i ids=""
  for i in $(seq "$3" "$4"); do ids="${ids} $(printf '%s-%03d' "${cls}" "${i}")"; done
  occupy "$d" ${ids}
}

n_refs()  { git -C "$1/origin.git" for-each-ref --format='%(refname)' refs/reserved | wc -l | tr -d ' '; }
ref_sha() { git -C "$1/origin.git" rev-parse --verify --quiet "refs/reserved/$2" 2>/dev/null; }
alloc_in() { ( cd "$1" && bash "${ALLOC_SH}" "$2" 2>/dev/null ); }

# Прогон обёртки в ПРОД-ФОРМЕ вызова: из каталога клона, аргументом — класс, stdout отдельно
# от stderr (контракт «stdout несёт только результат» иначе непроверяем).
EXTRA_ENV=()
run_res() {  # $1=клон $2=файл stdout $3=файл stderr, далее — аргументы обёртки
  local d="$1" so="$2" se="$3"; shift 3
  ( cd "$d" && env ALLOC="${ALLOC_SH}" RESERVE_ROLE="${ROLE_TAG}" \
      ${EXTRA_ENV[@]+"${EXTRA_ENV[@]}"} bash "${TARGET}" "$@" ) >"$so" 2>"$se"
}

# Setup-guard: КОНЪЮНКЦИЯ «предмет есть, где должен» И «его нет, где не должен». Одно лишь
# отрицание выполняется вакуумно, когда фикстура не построилась вовсе, — и страж молчит
# ровно тогда, когда обязан кричать.
#
# ГРАНИЦА, найденная первым же прогоном батареи (и стоившая бы всей её достоверности):
# страж накрывает ТОЛЬКО то, что построила сама проба, — корпус, клоны, чужие резервы,
# подменённый аллокатор. Предусловие, созданное РАБОТОЙ ПРЕДМЕТА («перед свипом на origin
# два резерва»), стражем быть не может: под мутантом оно законно не выполняется, и проба
# умирала бы с «SETUP НЕ СОСТОЯЛСЯ» вместо того, чтобы засчитать мутанту падение сценария.
# Такие предусловия проверяются как обычные ассерты сценария (fail + return 1).
setup_assert() {  # $1=сценарий $2=почему $3=условие (shell)
  eval "$3" >/dev/null 2>&1 || die "$1: $2"
}
attempts_in() { grep -c '^reserve: попытка' "$1" 2>/dev/null || true; }

# ══ СЦЕНАРИИ ═══════════════════════════════════════════════════════════════════════
# Имя сценария = имя функции без префикса `sc_`; возврат 0 — пройден, 1 — провален.
SCENARIOS="PARALLEL TWICE STDOUT REISSUE BLOCK TAIL EXHAUST RELEASE STALE LIST SWEEP NONCESRC NONCEDEG FMTDRIFT ALLOCFAIL"

# Два резерва одного класса из РАЗНЫХ клонов ОДНОЙ вершины — случай, ради которого механизм
# существует. Оба клона получают от аллокатора C-085 (аллокатор резервов не видит по
# построению), развести их обязан ТОЛЬКО push.
#
# ДВЕ ФАЗЫ, и вторая — не дубль первой. Одновременная фаза моделирует живой случай, но её
# исход при СЛОМАННОЙ реализации зависит от того, попали ли пуши в одну блокировку ref'а:
# замер показал, что мутант `forcepush` при перекрытии получает «cannot lock ref», уходит на
# следующий номер и выглядит исправным. Гейт, исход которого зависит от планировщика, мерит
# ОКРУЖЕНИЕ, а не свой инвариант (`testing.md`, свойство 2). Поэтому добавлена ФАЗА СО
# СДВИГОМ: второй резервирующий стартует после полного завершения первого — блокировок нет,
# и любая реализация, не различающая резервирующих, обязана выдать один и тот же номер
# ДЕТЕРМИНИРОВАННО. Одновременная фаза при этом сохранена: она ловила бы слом, который
# проявляется только при перекрытии.
sc_PARALLEL() {
  local d r1 r2 r3 r4 id1 id2 id3 id4 all
  d="$(mk_fix parallel 4)"
  setup_assert PARALLEL "все клоны обязаны видеть один и тот же следующий номер C-085" \
    '[ "$(alloc_in "'"$d"'/c1" C)" = C-085 ] && [ "$(alloc_in "'"$d"'/c2" C)" = C-085 ] && [ "$(alloc_in "'"$d"'/c4" C)" = C-085 ]'
  setup_assert PARALLEL "перед прогоном резервов быть не должно" '[ "$(n_refs "'"$d"'")" = 0 ]'
  ( run_res "$d/c1" "$d/o1" "$d/e1" C; echo $? > "$d/r1" ) &
  ( run_res "$d/c2" "$d/o2" "$d/e2" C; echo $? > "$d/r2" ) &
  wait
  run_res "$d/c3" "$d/o3" "$d/e3" C; r3=$?
  run_res "$d/c4" "$d/o4" "$d/e4" C; r4=$?
  r1="$(cat "$d/r1")"; r2="$(cat "$d/r2")"
  id1="$(cat "$d/o1")"; id2="$(cat "$d/o2")"; id3="$(cat "$d/o3")"; id4="$(cat "$d/o4")"
  [ "$r1" = 0 ] && [ "$r2" = 0 ] && [ "$r3" = 0 ] && [ "$r4" = 0 ] \
    || { fail "PARALLEL — коды возврата ${r1}/${r2}/${r3}/${r4}, ожидались нули"; return 1; }
  [ "$id1" != "$id2" ] \
    || { fail "PARALLEL(одновременно) — ОБА получили '${id1}': гонка воспроизведена, резерв не атомарен"; return 1; }
  [ "$id3" != "$id4" ] \
    || { fail "PARALLEL(со сдвигом) — ОБА получили '${id3}': резерв не различает резервирующих"; return 1; }
  all="$(printf '%s\n%s\n%s\n%s\n' "$id1" "$id2" "$id3" "$id4" | sort | tr '\n' ' ')"
  [ "$all" = "C-085 C-086 C-087 C-088 " ] \
    || { fail "PARALLEL — выданы {${all}}, ожидались C-085..C-088"; return 1; }
  [ "$(n_refs "$d")" = 4 ] || { fail "PARALLEL — на origin $(n_refs "$d") резервов, ожидалось 4"; return 1; }
  [ "$(ref_sha "$d" C-085)" != "$(ref_sha "$d" C-086)" ] \
    || { fail "PARALLEL — два резерва указывают на один SHA"; return 1; }
  pass "PARALLEL — одновременные ${id1}/${id2}, со сдвигом ${id3}/${id4} — все различны"
}

# Повторный резерв из ОДНОГО клона без приземления носителя: аллокатор снова говорит C-085,
# и это ровно вход, на котором наивная реализация даёт «Everything up-to-date» + exit 0.
sc_TWICE() {
  local d id1 id2
  d="$(mk_fix twice 1)"
  setup_assert TWICE "аллокатор обязан дать C-085" '[ "$(alloc_in "'"$d"'/c1" C)" = C-085 ]'
  run_res "$d/c1" "$d/o1" "$d/e1" C || { fail "TWICE — первый резерв упал"; return 1; }
  setup_assert TWICE "после первого резерва аллокатор ОБЯЗАН по-прежнему говорить C-085 (носитель не приземлён) — иначе сценарий проверяет не тот вход" \
    '[ "$(alloc_in "'"$d"'/c1" C)" = C-085 ]'
  run_res "$d/c1" "$d/o2" "$d/e2" C || { fail "TWICE — второй резерв упал"; return 1; }
  id1="$(cat "$d/o1")"; id2="$(cat "$d/o2")"
  [ "$id1" = C-085 ] && [ "$id2" = C-086 ] \
    || { fail "TWICE — '${id1}' и '${id2}', ожидались C-085 и C-086 (ложный успех на том же SHA)"; return 1; }
  [ "$(n_refs "$d")" = 2 ] || { fail "TWICE — на origin $(n_refs "$d") резервов, ожидалось 2"; return 1; }
  pass "TWICE — повторный резерв не дал ложного успеха: ${id1} → ${id2}"
}

# Drop-in для `next_artifact_id.sh`: stdout — РОВНО одна строка с идентификатором.
sc_STDOUT() {
  local d n
  d="$(mk_fix stdout 1)"
  run_res "$d/c1" "$d/o" "$d/e" C || { fail "STDOUT — резерв упал"; return 1; }
  n="$(wc -l < "$d/o" | tr -d ' ')"
  [ "$n" = 1 ] || { fail "STDOUT — строк в stdout: ${n}, ожидалась 1 (обёртка не drop-in)"; return 1; }
  [ "$(cat "$d/o")" = C-085 ] || { fail "STDOUT — '$(cat "$d/o")' вместо C-085"; return 1; }
  grep -q '^reserve: попытка' "$d/e" || { fail "STDOUT — диагностики нет и в stderr"; return 1; }
  pass "STDOUT — одна строка 'C-085', диагностика в stderr"
}

# Занятый номер (чужой SHA) → перевыдача. Плюс steal-check: чужой ref обязан ОСТАТЬСЯ чужим.
sc_REISSUE() {
  local d foreign id
  d="$(mk_fix reissue 1)"
  foreign="$(occupy "$d" C-085)"
  setup_assert REISSUE "C-085 обязан быть занят чужим SHA, C-086 — свободен" \
    '[ -n "$(ref_sha "'"$d"'" C-085)" ] && [ -z "$(ref_sha "'"$d"'" C-086)" ]'
  setup_assert REISSUE "аллокатор обязан целиться именно в занятый C-085" \
    '[ "$(alloc_in "'"$d"'/c1" C)" = C-085 ]'
  run_res "$d/c1" "$d/o" "$d/e" C || { fail "REISSUE — резерв упал вместо перевыдачи"; return 1; }
  id="$(cat "$d/o")"
  [ "$id" = C-086 ] || { fail "REISSUE — выдан '${id}', ожидался C-086"; return 1; }
  [ "$(ref_sha "$d" C-085)" = "$foreign" ] \
    || { fail "REISSUE — чужой резерв C-085 ПЕРЕЗАПИСАН (кража номера)"; return 1; }
  pass "REISSUE — перевыдал C-086, чужой C-085 не тронут"
}

# Блок занятых номеров перепрыгивается за ОДНУ повторную попытку, а не шагом +1.
sc_BLOCK() {
  local d id a
  d="$(mk_fix block 1)"
  occupy_range "$d" C 85 89 >/dev/null
  setup_assert BLOCK "заняты обязаны быть C-085..C-089, C-090 — свободен" \
    '[ -n "$(ref_sha "'"$d"'" C-089)" ] && [ -z "$(ref_sha "'"$d"'" C-090)" ]'
  run_res "$d/c1" "$d/o" "$d/e" C || { fail "BLOCK — резерв упал"; return 1; }
  id="$(cat "$d/o")"; a="$(attempts_in "$d/e")"
  [ "$id" = C-090 ] || { fail "BLOCK — выдан '${id}', ожидался C-090"; return 1; }
  [ "$a" -le 2 ] || { fail "BLOCK — попыток ${a}, ожидалось ≤2 (прыжок через блок не работает)"; return 1; }
  pass "BLOCK — 5 занятых перепрыгнуты за ${a} попытки → ${id}"
}

# `C-084` N-3: операционный хвост. Стоимость резерва не растёт с числом протухших резервов —
# меряется ЧИСЛОМ ПОПЫТОК (время мерило бы окружение, а не инвариант).
sc_TAIL() {
  local d id a
  d="$(mk_fix tail 1)"
  occupy_range "$d" C 85 284 >/dev/null
  setup_assert TAIL "хвост обязан быть длиной 200" '[ "$(n_refs "'"$d"'")" = 200 ]'
  setup_assert TAIL "C-285 обязан быть свободен" '[ -z "$(ref_sha "'"$d"'" C-285)" ]'
  run_res "$d/c1" "$d/o" "$d/e" C || { fail "TAIL — резерв упал на хвосте в 200 резервов"; return 1; }
  id="$(cat "$d/o")"; a="$(attempts_in "$d/e")"
  [ "$id" = C-285 ] || { fail "TAIL — выдан '${id}', ожидался C-285"; return 1; }
  [ "$a" -le 2 ] || { fail "TAIL — попыток ${a} на хвосте 200: стоимость растёт с хвостом"; return 1; }
  pass "TAIL — хвост 200 протухших резервов пройден за ${a} попытки → ${id}"
}

# Исчерпание попыток — fail-closed: exit 3, номер НЕ напечатан, лишних ref'ов нет.
sc_EXHAUST() {
  local d rc
  d="$(mk_fix exhaust 1)"
  occupy "$d" C-085 >/dev/null
  setup_assert EXHAUST "C-085 занят, C-086 свободен — при MAX=1 второй попытки не будет" \
    '[ -n "$(ref_sha "'"$d"'" C-085)" ] && [ -z "$(ref_sha "'"$d"'" C-086)" ]'
  EXTRA_ENV=(RESERVE_MAX_ATTEMPTS=1)
  run_res "$d/c1" "$d/o" "$d/e" C; rc=$?
  EXTRA_ENV=()
  [ "$rc" = 3 ] || { fail "EXHAUST — exit=${rc}, ожидался 3 (fail-closed на исчерпании)"; return 1; }
  [ ! -s "$d/o" ] || { fail "EXHAUST — напечатан номер '$(cat "$d/o")' при исчерпании попыток"; return 1; }
  [ "$(n_refs "$d")" = 1 ] || { fail "EXHAUST — резервов $(n_refs "$d"), ожидался 1 (чужой)"; return 1; }
  pass "EXHAUST — exit=3, номер не выдан, лишних резервов нет"
}

# Уборка: снятие резерва работает, а снятие НЕСУЩЕСТВУЮЩЕГО — fail-closed. Второе не
# формальность: `git push origin :refs/reserved/X` на отсутствующем ref'е возвращает 0
# (замер 2026-08-15), то есть без предпроверки «резерв снят» было бы ложью при опечатке.
sc_RELEASE() {
  local d rc
  d="$(mk_fix release 1)"
  run_res "$d/c1" "$d/o" "$d/e" C || { fail "RELEASE — резерв упал"; return 1; }
  [ -n "$(ref_sha "$d" C-085)" ] || { fail "RELEASE — резерв C-085 не создан, снимать нечего"; return 1; }
  run_res "$d/c1" "$d/o2" "$d/e2" --release C-085 || { fail "RELEASE — снятие упало"; return 1; }
  [ -z "$(ref_sha "$d" C-085)" ] || { fail "RELEASE — ref остался на origin после снятия"; return 1; }
  run_res "$d/c1" "$d/o3" "$d/e3" --release C-085; rc=$?
  [ "$rc" != 0 ] || { fail "RELEASE — снятие НЕСУЩЕСТВУЮЩЕГО резерва вернуло 0 (ложный успех)"; return 1; }
  pass "RELEASE — снят; повторное снятие fail-closed (exit=${rc})"
}

# Протухший резерв НЕ переиспользуется: номер пропускается навсегда (§12 «разрывы не
# занимаются»), третий резерв идёт дальше, а не возвращается к брошенному.
sc_STALE() {
  local d i1 i2 i3
  d="$(mk_fix stale 1)"
  run_res "$d/c1" "$d/o1" "$d/e1" C || { fail "STALE — резерв 1 упал"; return 1; }
  run_res "$d/c1" "$d/o2" "$d/e2" C || { fail "STALE — резерв 2 упал"; return 1; }
  run_res "$d/c1" "$d/o3" "$d/e3" C || { fail "STALE — резерв 3 упал"; return 1; }
  i1="$(cat "$d/o1")"; i2="$(cat "$d/o2")"; i3="$(cat "$d/o3")"
  [ "$i1" = C-085 ] && [ "$i2" = C-086 ] && [ "$i3" = C-087 ] \
    || { fail "STALE — '${i1}'/'${i2}'/'${i3}', ожидались C-085/C-086/C-087"; return 1; }
  [ "$(n_refs "$d")" = 3 ] || { fail "STALE — резервов $(n_refs "$d"), ожидалось 3"; return 1; }
  pass "STALE — брошенные ${i1}/${i2} не переиспользованы, третий взял ${i3}"
}

# Хвост НАБЛЮДАЕМ: кто и когда занял номер — из тела резервного коммита.
sc_LIST() {
  local d n
  d="$(mk_fix list 1)"
  run_res "$d/c1" "$d/o1" "$d/e1" C || { fail "LIST — резерв 1 упал"; return 1; }
  run_res "$d/c1" "$d/o2" "$d/e2" C || { fail "LIST — резерв 2 упал"; return 1; }
  run_res "$d/c1" "$d/ol" "$d/el" --list || { fail "LIST — --list упал"; return 1; }
  n="$(wc -l < "$d/ol" | tr -d ' ')"
  [ "$n" = 2 ] || { fail "LIST — строк ${n}, ожидалось 2"; return 1; }
  grep -q 'C-085' "$d/ol" && grep -q 'C-086' "$d/ol" \
    || { fail "LIST — в выдаче нет C-085/C-086"; return 1; }
  grep -q "${ROLE_TAG}" "$d/ol" || { fail "LIST — роль резервирующего не видна"; return 1; }
  pass "LIST — оба резерва видны с ролью '${ROLE_TAG}'"
}

# `--sweep` ПЕЧАТАЕТ кандидатов и НИЧЕГО не удаляет: удаление вернуло бы номер в оборот,
# а брошенность доказывается молчанием во времени (branch-hygiene §8), не снимком.
sc_SWEEP() {
  local d
  d="$(mk_fix sweep 1)"
  run_res "$d/c1" "$d/o1" "$d/e1" C || { fail "SWEEP — резерв 1 упал"; return 1; }
  run_res "$d/c1" "$d/o2" "$d/e2" C || { fail "SWEEP — резерв 2 упал"; return 1; }
  [ "$(n_refs "$d")" = 2 ] || { fail "SWEEP — до свипа $(n_refs "$d") резервов, ожидалось 2"; return 1; }
  run_res "$d/c1" "$d/os" "$d/es" --sweep 0 || { fail "SWEEP — --sweep упал"; return 1; }
  grep -q 'снять: bash scripts/reserve_artifact_id.sh --release C-085' "$d/os" \
    || { fail "SWEEP — не напечатал команду снятия"; return 1; }
  [ "$(n_refs "$d")" = 2 ] \
    || { fail "SWEEP — УДАЛИЛ резервы (осталось $(n_refs "$d")): номер вернулся в оборот"; return 1; }
  pass "SWEEP — кандидаты напечатаны, ни один резерв не удалён"
}

# Источника энтропии нет → fail-closed exit 2. Тихого отката на $RANDOM быть не должно:
# 15 бит, засеянных pid+временем, у двух одновременных ролей совпадают регулярно.
sc_NONCESRC() {
  local d rc
  d="$(mk_fix noncesrc 1)"
  EXTRA_ENV=(RESERVE_NONCE_SOURCE=/nonexistent-nonce-source)
  run_res "$d/c1" "$d/o" "$d/e" C; rc=$?
  EXTRA_ENV=()
  [ "$rc" = 2 ] || { fail "NONCESRC — exit=${rc}, ожидался 2 (нет источника nonce)"; return 1; }
  [ "$(n_refs "$d")" = 0 ] || { fail "NONCESRC — резерв ВЗЯТ без источника энтропии"; return 1; }
  pass "NONCESRC — exit=2, резерв не взят"
}

# Источник вырожден (два чтения дают одно значение) → fail-closed exit 2. Это обнаружение
# вырожденного ИСТОЧНИКА, а не доказательство недостижимости коллизии — см. шапку обёртки.
sc_NONCEDEG() {
  local d rc
  d="$(mk_fix noncedeg 1)"
  printf 'CONSTANT-NONCE-VALUE-0123456789abcdef\n' > "$d/const-nonce"
  setup_assert NONCEDEG "источник обязан быть длиннее 16 символов, иначе сценарий проверил бы длину, а не вырожденность" \
    '[ "$(wc -c < "'"$d"'/const-nonce")" -gt 17 ]'
  EXTRA_ENV=(RESERVE_NONCE_SOURCE="$d/const-nonce")
  run_res "$d/c1" "$d/o" "$d/e" C; rc=$?
  EXTRA_ENV=()
  [ "$rc" = 2 ] || { fail "NONCEDEG — exit=${rc}, ожидался 2 (вырожденный источник)"; return 1; }
  [ "$(n_refs "$d")" = 0 ] || { fail "NONCEDEG — резерв взят на константном nonce"; return 1; }
  pass "NONCEDEG — exit=2, резерв не взят"
}

# Формат печати продублирован в обёртке вынужденно (аллокатор герметичен и резервов не
# видит). Дубль обязан СВЕРЯТЬСЯ: аллокатор, начавший печатать другую ширину, обязан
# уронить резерв, а не тихо разъехаться с именем файла артефакта.
sc_FMTDRIFT() {
  local d rc
  d="$(mk_fix fmtdrift 1)"
  sed 's|%s-%03d|%s-%04d|' "${ALLOC_SH}" > "$d/alloc-drift.sh" || die "FMTDRIFT: копия аллокатора"
  setup_assert FMTDRIFT "подменённый аллокатор обязан печатать C-0085, а честный — C-085" \
    '[ "$( cd "'"$d"'/c1" && bash "'"$d"'/alloc-drift.sh" C )" = C-0085 ] && [ "$(alloc_in "'"$d"'/c1" C)" = C-085 ]'
  EXTRA_ENV=(ALLOC="$d/alloc-drift.sh")
  run_res "$d/c1" "$d/o" "$d/e" C; rc=$?
  EXTRA_ENV=()
  [ "$rc" != 0 ] || { fail "FMTDRIFT — резерв взят при разошедшемся формате аллокатора"; return 1; }
  [ "$(n_refs "$d")" = 0 ] || { fail "FMTDRIFT — ref создан при разошедшемся формате"; return 1; }
  pass "FMTDRIFT — расхождение формата с аллокатором fail-closed (exit=${rc})"
}

# Аллокатор отказал (класс A в корпусе фикстуры отсутствует — его setup-guard) ⇒ резерв
# НЕ берётся: номер от сломанного аллокатора хуже отсутствия номера.
sc_ALLOCFAIL() {
  local d rc
  d="$(mk_fix allocfail 1)"
  setup_assert ALLOCFAIL "аллокатор обязан отказать по классу A и работать по классу C" \
    '! ( cd "'"$d"'/c1" && bash "'"${ALLOC_SH}"'" A >/dev/null 2>&1 ) && [ "$(alloc_in "'"$d"'/c1" C)" = C-085 ]'
  run_res "$d/c1" "$d/o" "$d/e" A; rc=$?
  [ "$rc" != 0 ] || { fail "ALLOCFAIL — резерв взят при отказавшем аллокаторе"; return 1; }
  [ "$(n_refs "$d")" = 0 ] || { fail "ALLOCFAIL — ref создан при отказавшем аллокаторе"; return 1; }
  pass "ALLOCFAIL — отказ аллокатора утащил резерв (exit=${rc})"
}

run_all() {  # печатает имена ПРОВАЛЕННЫХ сценариев, по одному на строку (в FAILED_NAMES)
  local s
  FAILED_NAMES=""
  for s in ${SCENARIOS}; do
    if ! "sc_${s}"; then FAILED_NAMES="${FAILED_NAMES}${s} "; fi
  done
}

# ══ БАТАРЕЯ МУТАНТОВ ═══════════════════════════════════════════════════════════════
# Каждая строка: имя|sed-программа|ЗАЯВЛЕННЫЙ kill-set (множества ИЗМЕРЕНЫ прогоном).
# Мутант обязан (а) отличаться от оригинала побайтово, (б) быть синтаксически корректным
# (`bash -n`), (в) уронить РОВНО заявленное множество. Без (а)/(б) «красный» мутант не
# отличим от несостоявшейся подмены — это и есть плацебо батареи.
MUTANTS='
sametip|s|^build_sha() .*|build_sha() { git rev-parse --verify --quiet "refs/remotes/${REMOTE}/main"; }|;|PARALLEL TWICE STALE LIST SWEEP
forcepush|s|^  if git push |  if git push --force |;|PARALLEL TWICE STALE LIST SWEEP REISSUE BLOCK TAIL EXHAUST
nojump|/rmax + 1/s|.*|  :|;|BLOCK TAIL
nolimit|s|^MAXA=.*|MAXA=99|;|EXHAUST
weaknonce|s|^draw_nonce() .*|draw_nonce() { printf "weak-fallback-nonce-%s%s" "${RANDOM}" "${RANDOM}"; }|;|NONCESRC NONCEDEG
nodegen|/n1.*!=.*n2/s|.*|:|;|NONCEDEG
nofmtcheck|/разошёлся с аллокатором/s|.*|:|;|FMTDRIFT
allocblind|/аллокатор отказал по классу/s#.*#base="$(bash "${ALLOC}" "${CLS}" 2>/dev/null)"; [ -n "${base}" ] || base="${CLS}-001"#;|ALLOCFAIL
blindrelease|s|^remote_has_ref() .*|remote_has_ref() { return 0; }|;|RELEASE
'

run_battery() {
  local line name sedprog decl mdir mut got bad=0 n=0
  mdir="$(mktemp -d /tmp/red-reserve-mutants-XXXXXX)" || die "mktemp мутантов"
  printf '%s\n' "${mdir}" >> "${REG}"
  echo "══ БАТАРЕЯ: каждый мутант обязан уронить РОВНО заявленное множество ══"
  while IFS= read -r line; do
    [ -z "${line}" ] && continue
    name="${line%%|*}"; line="${line#*|}"
    sedprog="${line%|*}"; decl="${line##*|}"
    mut="${mdir}/${name}.sh"
    sed "${sedprog}" "${TARGET}" > "${mut}" || die "мутант ${name}: sed"
    cmp -s "${mut}" "${TARGET}" \
      && die "мутант ${name} ИДЕНТИЧЕН оригиналу — подмена не состоялась, батарея была бы плацебо"
    bash -n "${mut}" 2>/dev/null \
      || die "мутант ${name} не парсится — «красный» был бы следствием синтаксиса, а не дыры"
    n=$((n + 1))
    local save="${TARGET}"; TARGET="${mut}"; run_all_isolated; TARGET="${save}"
    got="$(printf '%s' "${MUT_FAILED}" | tr ' ' '\n' | grep . | sort | tr '\n' ' ')"
    decl="$(printf '%s' "${decl}" | tr ' ' '\n' | grep . | sort | tr '\n' ' ')"
    if [ "${got}" = "${decl}" ]; then
      echo "PASS  ${name} → уронил {${got% }} = заявлено"
    else
      echo "FAIL  ${name} → уронил {${got}} ≠ заявлено {${decl}}"
      bad=$((bad + 1))
    fi
  done <<< "${MUTANTS}"
  echo
  if [ "${bad}" -gt 0 ]; then
    echo "VERDICT: FAIL (${bad} из ${n} мутантов разошлись с заявленным kill-set'ом)"
    return 1
  fi
  echo "VERDICT: PASS (${n}/${n}) — каждый мутант красен РОВНО по своим сценариям"
  return 0
}

# Прогон полного набора против ${TARGET} в ПОДОБОЛОЧКЕ, чтобы счётчики основного прогона не
# смешивались с мутантскими; результат возвращается через файл (подоболочка в родителя
# переменных не отдаёт — тот же класс ошибки, что убил реестр фикстур в red_artifact_ids).
MUT_FAILED=""
run_all_isolated() {
  local tmp; tmp="$(mktemp /tmp/red-reserve-mut-XXXXXX)"; printf '%s\n' "${tmp}" >> "${REG}"
  # stderr НЕ глушится: сюда пишет `die`. Прогон, оборвавшийся на setup-guard'е, отдал бы
  # НЕПОЛНЫЙ список провалов, и мутант выглядел бы точечным — поэтому подоболочка обязана
  # дописать маркер COMPLETE, а его отсутствие фатально.
  ( run_all >/dev/null; printf '%s\nCOMPLETE\n' "${FAILED_NAMES}" > "${tmp}" )
  grep -qx COMPLETE "${tmp}" \
    || die "прогон набора против мутанта оборвался (setup-guard) — kill-set недостоверен"
  MUT_FAILED="$(head -1 "${tmp}")"; rm -f "${tmp}"
}

# ══ ВХОД ═══════════════════════════════════════════════════════════════════════════
case "${MODE}" in
  --battery)
    run_battery; exit $? ;;
  scenarios)
    echo "══ СЦЕНАРИИ (позитивный контроль: честная реализация обязана быть зелёной) ══"
    run_all
    echo
    if [ "${FAILED}" -gt 0 ]; then
      echo "VERDICT: FAIL (${FAILED}) — провалены: ${FAILED_NAMES}"
      exit 1
    fi
    echo "VERDICT: PASS (${PASSED}/${PASSED}) — инвариант «напечатанный номер занят на origin» держится"
    exit 0 ;;
  *)
    echo "usage: red_reserve_id.sh [--battery]" >&2; exit 1 ;;
esac
