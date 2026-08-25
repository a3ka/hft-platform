#!/usr/bin/env bash
# Проба сторожа живого процесса — предмет: `scripts/gc_worktrees.sh`, условие (4).
#
# ЗАЧЕМ (инцидент 2026-08-24). Условия (1)-(3) `gc_worktrees.sh` описывают состояние ГИТА:
# дерево чистое, локальных коммитов нет, HEAD на `origin/main`. Агент, только что
# забутстрапившийся в свежем дереве, проходит все три идеально — вердикт ещё не написан,
# коммитов нет, HEAD ровно `origin/main`. Так `--reclaim` снёс рабочий каталог ЖИВОГО
# критика: четыре процесса получили `cwd` на удалённый inode, сессия стала нерабочей.
# Страж, который смотрит на git, когда предмет — процесс, не «работает криво»: его нет.
#
# ТРИ СВОЙСТВА (`docs/workflow/harness-track.md` §5):
#   1. ПОЗИТИВНЫЙ КОНТРОЛЬ — без живого процесса дерево по-прежнему сносится, а `target/`
#      по-прежнему забирается. Сторож, блокирующий всё, бесполезен так же, как отсутствующий:
#      его выключат первым же «он мешает».
#   2. АНТИ-ПЛАЦЕБО В ОБЕ СТОРОНЫ — держит живой процесс (в дереве, в подкаталоге, при
#      удалённом каталоге) И не держит мёртвый.
#   3. МУТАЦИОННЫЙ КОНТРОЛЬ (`--battery`) — нейтрализация каждого шва роняет РОВНО свой
#      сценарий; kill-set заявляется и сверяется, а не пересказывается.
#
# ПРОД-ФОРМА. Сценарии L1-L5, L8, L9 гоняют НАСТОЯЩИЙ `/proc` и НАСТОЯЩИЕ фоновые процессы:
# сторож обязан быть проверен тем же механизмом, каким его дёргает прод. Ручка
# `GC_PROC_ROOT` существует ТОЛЬКО ради L6/L7 — «`/proc` недоступен» без неё не
# воспроизвести без root, а fail-closed без сценария остаётся декларацией.
#
# ГЕРМЕТИЧНОСТЬ. Проба строит СВОЙ репозиторий в TMPDIR, в сеть не ходит и `origin` подделывает
# локальным ref'ом. Иначе она мерила бы доступность сети, а не свой инвариант.
#
# Прогон: bash scripts/tests/red_gc_live_cwd.sh [--battery]

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUT="${ROOT}/scripts/gc_worktrees.sh"
SUT_ACTIVE="${SUT}"

FAKE_BIN=""
PASS=0; FAIL=0; FAILED_NAMES=()
ok()   { PASS=$((PASS + 1)); printf 'ok         %-28s %s\n' "$1" "${2:-}"; }
nok()  { FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'FAIL       %-28s %s\n' "$1" "$2"; }
sfail(){ FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1"); printf 'SETUP-FAIL %-28s %s\n' "$1" "$2"; }

own_dirs(){ find "${TMPDIR:-/tmp}" -maxdepth 1 -type d -name 'red-gclive-*' 2>/dev/null | wc -l; }
TMP_BEFORE="$(own_dirs)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/red-gclive-XXXXXX")"
REG="${WORK}/.fixtures"; : > "${REG}"
PIDREG="${WORK}/.pids"; : > "${PIDREG}"
register(){ printf '%s\n' "$1" >> "${REG}"; }
regpid(){ printf '%s\n' "$1" >> "${PIDREG}"; }
cleanup(){
  # Процессы гасятся ПЕРВЫМИ: живой `sleep` с cwd в фикстуре не даст снести каталог начисто
  # на некоторых ФС и оставит мусор — тот самый класс, что дал 10 400 каталогов в `/tmp`.
  [ -f "${PIDREG}" ] && while IFS= read -r q; do [ -n "$q" ] && kill "$q" 2>/dev/null; done < "${PIDREG}"
  wait 2>/dev/null
  # Права возвращаются ПЕРЕД удалением: сценарий L11 снимает их с каталога PID, и без этого
  # `rm -rf` оставил бы фикстуру — тот же класс, что дал 10 400 каталогов в /tmp.
  [ -f "${REG}" ] && while IFS= read -r p; do [ -n "$p" ] && [ -e "$p" ] && chmod -R u+rwX "$p" 2>/dev/null; done < "${REG}"
  [ -f "${REG}" ] && while IFS= read -r p; do [ -n "$p" ] && [ -e "$p" ] && rm -rf "$p"; done < "${REG}"
  rm -rf "${WORK}"
}
trap cleanup EXIT
register "${WORK}"

# mk_case <имя> → печатает "<главный чекаут> <путь worktree>".
# Строит репозиторий, где worktree удовлетворяет ВСЕМ ТРЁМ git-условиям: чистое дерево,
# ноль только-локальных коммитов, HEAD == origin/main. То есть по прежней редакции скрипта
# он ПОДЛЕЖИТ СНОСУ — и ровно это делает сценарии осмысленными.
mk_case() {
  local name="$1"
  local d; d="$(mktemp -d "${WORK}/case-${name}-XXXXXX")" || return 1
  register "${d}"
  local main="${d}/main" wt="${d}/wt"
  (
    mkdir -p "${main}" && cd "${main}" || exit 1
    git init -q .
    git config user.email t@t; git config user.name t
    echo base > f.txt; git add f.txt; git commit -qm base
    git update-ref refs/remotes/origin/main HEAD
    git worktree add -q --detach "${wt}" HEAD
    mkdir -p "${wt}/sub"
  ) >/dev/null 2>&1 || return 1
  # Перевод строки ОБЯЗАТЕЛЕН: `read` без него возвращает 1 даже успешно присвоив переменные,
  # и каждый сценарий объявлял бы «фикстура не построена». Проба, падающая на своём setup'е,
  # мерит себя, а не предмет.
  printf '%s %s\n' "${main}" "${wt}"
}

# spawn_in <каталог> → печатает PID фонового процесса, чей cwd находится в этом каталоге.
# `exec sleep` намеренно: без него cwd держал бы промежуточный bash, а не тот процесс,
# чей PID мы вернули, и сценарий проверял бы не то, что заявляет.
spawn_in() {
  local dir="$1"
  # stdout/stderr ОБЯЗАНЫ уйти в /dev/null: фоновый процесс наследует канал подстановки
  # команд `$(spawn_in …)` и держит его открытым, а `$(…)` ждёт ЗАКРЫТИЯ канала, а не
  # завершения переднего плана. Без этого проба висит ровно `sleep`-таймаут на каждом
  # сценарии и выглядит как «зависший гейт», а не как своя ошибка.
  ( cd "$dir" && exec sleep 120 ) >/dev/null 2>&1 &
  local q=$!
  regpid "${q}"
  # Дожидаемся, пока cwd действительно установится: гонка старта сделала бы сценарий
  # мигающим, а мигающий сценарий объявят шумом и выключат.
  local i
  for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
    [ "$(readlink "/proc/${q}/cwd" 2>/dev/null | sed 's/ (deleted)$//')" = "$dir" ] && break
    sleep 0.05
  done
  printf '%s' "${q}"
}

# mk_pgrep <каталог> <код-возврата> — кладёт в <каталог>/bin поддельный `pgrep`.
#
# ЗАЧЕМ. Режим `--reclaim` fail-closed отказывает целиком, если на МАШИНЕ идёт сборка
# (`pgrep -x cargo || pgrep -x rustc`, `gc_worktrees.sh:158`). Это верное прод-поведение, но
# для пробы — конфаундинг: исход зависел бы от того, собирает ли кто-то рядом. `testing.md`,
# целостность гейта, свойство 2: оракул обязан мерить СВОЙ инвариант, а не окружение;
# конфаундинг-величину держат КОНСТАНТНОЙ. Подменяется ВНЕШНИЙ инструмент, путь вызова
# внутри предмета не меняется — та же техника, что с поддельным `gh` в `red_branch_health.sh`.
mk_pgrep() {
  local d="$1" rc="$2"
  mkdir -p "${d}/bin" || return 1
  printf '#!/usr/bin/env bash
exit %s
' "${rc}" > "${d}/bin/pgrep"
  chmod +x "${d}/bin/pgrep"
}

# benign_proc <главный> → печатает путь фиктивного /proc с ОДНИМ безобидным PID.
#
# ЗАЧЕМ, И ЭТО НЕ ОБХОД. Сценарии делятся на два класса. Те, что утверждают УДЕРЖАНИЕ
# (L2/L3/L5/L8), поднимают НАСТОЯЩИЙ процесс и обязаны идти по настоящему `/proc` — они
# робастны к посторонним PID'ам: лишний неопрошенный процесс их вывод не меняет. Те, что
# утверждают УДАЛЕНИЕ (L1/L4/L9/L13), наоборот, зависят от того, ЧЕГО на хосте нет, — и на
# GitHub-раннере нашлись ЖИВЫЕ процессы нашего uid с нечитаемым `cwd` (прогон
# 2026-08-24T23:2xZ: L9 «кэш остался — сторож переблокировал»). Такой сценарий мерил бы
# таблицу процессов раннера, а не свой инвариант (`testing.md`, целостность гейта,
# свойство 2: конфаундинг-величину держат КОНСТАНТНОЙ, варьируют только измеряемую).
# Фикстура даёт перечислимый `/proc` с одним PID, чей `cwd` читаем и ведёт ВНЕ песочницы:
# страж `readable` не срабатывает, держателей нет — определённо и воспроизводимо.
benign_proc() {
  local main="$1" fp="$1/benignproc"
  mkdir -p "${fp}/1" || return 1
  ln -sfn / "${fp}/1/cwd" || return 1
  printf '%s' "${fp}"
}

# proc_of <главный> <pid> → фиктивный /proc, содержащий РОВНО один PID — симлинк на
# НАСТОЯЩИЙ `/proc/<pid>`. Прод-форма сохраняется полностью: `readlink … /cwd` читает
# настоящее ядро и отдаёт настоящую строку, включая суффикс `(deleted)`; `stat` отдаёт
# настоящего владельца. Убирается ровно одно — ШУМ ЧУЖИХ ПРОЦЕССОВ.
#
# Зачем: на GitHub-раннере живут процессы нашего uid с нечитаемым `cwd`, и сторож законно
# считает их держателями. Для сценариев это не находка, а КОНФАУНДИНГ: он не только ронял
# L9, но и МАСКИРОВАЛ мутанта G3 — kill-set пришёл пустым, потому что удержание
# обеспечивалось посторонним процессом, а не проверяемой строкой. Мутант, который «не
# роняет», читается как «строка ничего не пиннит», и это был бы ложный вывод о предмете.
proc_of() {
  local main="$1" pid="$2" fp="$1/procof-${2}"
  mkdir -p "${fp}" || return 1
  ln -sfn "/proc/${pid}" "${fp}/${pid}" || return 1
  printf '%s' "${fp}"
}

# expect_gc <имя> <главный> <ожидаемый-код> <обязательная|-> <запрещённая|-> [аргументы gc…]
expect_gc() {
  local name="$1" main="$2" wantrc="$3" must="$4" mustnot="$5"; shift 5
  local pth="${PATH}"
  [ -n "${FAKE_BIN:-}" ] && pth="${FAKE_BIN}:${PATH}"
  OUT="$(cd "${main}" && PATH="${pth}" bash "${SUT_ACTIVE}" "$@" 2>&1)"; RC=$?
  if [ "${RC}" -ne "${wantrc}" ]; then nok "${name}" "exit=${RC}, ожидался ${wantrc}"; return; fi
  if [ "${must}" != "-" ] && ! grep -qF "${must}" <<<"${OUT}"; then nok "${name}" "нет «${must}»"; return; fi
  if [ "${mustnot}" != "-" ] && grep -qF "${mustnot}" <<<"${OUT}"; then nok "${name}" "ЛОЖНОЕ: есть «${mustnot}»"; return; fi
  ok "${name}" "exit=${RC}"
}

scenarios() {

# --- L1 ПОЗИТИВНЫЙ КОНТРОЛЬ: без живого процесса дерево сносится, как и раньше -----------
read -r main wt < <(mk_case l1) || { sfail "L1-без-процесса-снос" "фикстура"; return; }
GC_PROC_ROOT="$(benign_proc "${main}")"; export GC_PROC_ROOT
expect_gc "L1-без-процесса-снос" "${main}" 0 "REMOVED  wt" "ЖИВОЙ процесс"
if [ -d "${wt}" ]; then nok "L1-каталог-исчез" "каталог остался — снос не состоялся"; else ok "L1-каталог-исчез"; fi
unset GC_PROC_ROOT

# --- L2 живой процесс, cwd == дерево -----------------------------------------------------
read -r main wt < <(mk_case l2) || { sfail "L2-живой-в-дереве" "фикстура"; return; }
q="$(spawn_in "${wt}")"
GC_PROC_ROOT="$(proc_of "${main}" "${q}")"; export GC_PROC_ROOT
expect_gc "L2-живой-в-дереве" "${main}" 0 "ЖИВОЙ процесс держит cwd" "REMOVED  wt"
if [ -d "${wt}" ]; then ok "L2-каталог-уцелел"; else nok "L2-каталог-уцелел" "дерево снесено под живым процессом"; fi
unset GC_PROC_ROOT
kill "${q}" 2>/dev/null

# --- L3 живой процесс, cwd в ПОДКАТАЛОГЕ -------------------------------------------------
# Отдельный сценарий, а не вариация L2: наивный сторож сравнивает пути на РАВЕНСТВО и
# пропускает всё, что глубже. Агент почти всегда сидит глубже корня дерева.
read -r main wt < <(mk_case l3) || { sfail "L3-живой-в-подкаталоге" "фикстура"; return; }
q="$(spawn_in "${wt}/sub")"
GC_PROC_ROOT="$(proc_of "${main}" "${q}")"; export GC_PROC_ROOT
expect_gc "L3-живой-в-подкаталоге" "${main}" 0 "ЖИВОЙ процесс держит cwd" "REMOVED  wt"
unset GC_PROC_ROOT
kill "${q}" 2>/dev/null

# --- L4 АНТИ-ПЛАЦЕБО В ДРУГУЮ СТОРОНУ: процесс умер — дерево снова сносится --------------
read -r main wt < <(mk_case l4) || { sfail "L4-процесс-умер-снос" "фикстура"; return; }
q="$(spawn_in "${wt}")"
kill "${q}" 2>/dev/null; wait "${q}" 2>/dev/null
GC_PROC_ROOT="$(benign_proc "${main}")"; export GC_PROC_ROOT
expect_gc "L4-процесс-умер-снос" "${main}" 0 "REMOVED  wt" "ЖИВОЙ процесс"
unset GC_PROC_ROOT

# --- L5 cwd помечен `(deleted)` ----------------------------------------------------------
# Именно этим состоянием кончился инцидент 24.08. Сторож, не снимающий суффикс, слеп РОВНО
# на те процессы, которым уже навредили, — и молча разрешает добить остаток.
#
# УДАЛЯЕТСЯ КОРЕНЬ ДЕРЕВА, А НЕ ПОДКАТАЛОГ, И ЭТО НЕСУЩАЯ ДЕТАЛЬ. Первая редакция сценария
# гасила процесс в `<wt>/sub` и удаляла `sub`: `readlink` отдавал `<wt>/sub (deleted)`, что
# СОВПАДАЕТ с образцом `"$wt"/*` и БЕЗ снятия суффикса. Сценарий был зелен против мутанта
# G3 и не пиннил ничего — та самая «проверка, утверждающая покрытие, не покрывая». Суффикс
# решает ровно тогда, когда удалён САМ корень: `<wt> (deleted)` не совпадает ни с `"$wt"`,
# ни с `"$wt"/*`. Поймано батареей: kill-set G3 пришёл пустым.
read -r main wt < <(mk_case l5) || { sfail "L5-cwd-deleted" "фикстура"; return; }
q="$(spawn_in "${wt}")"
GC_PROC_ROOT="$(proc_of "${main}" "${q}")"; export GC_PROC_ROOT
rm -rf "${wt}"
expect_gc "L5-cwd-deleted" "${main}" 0 "ЖИВОЙ процесс держит cwd" "REMOVED  wt"
unset GC_PROC_ROOT
kill "${q}" 2>/dev/null

# --- L6 FAIL-CLOSED: `/proc` отсутствует -------------------------------------------------
read -r main wt < <(mk_case l6) || { sfail "L6-proc-нет-fail-closed" "фикстура"; return; }
OUT="$(cd "${main}" && GC_PROC_ROOT="${WORK}/no-such-proc" bash "${SUT_ACTIVE}" 2>&1)"; RC=$?
if [ "${RC}" -ne 0 ]; then nok "L6-proc-нет-fail-closed" "exit=${RC}"
elif ! grep -qF "ЖИВОЙ процесс держит cwd" <<<"${OUT}"; then nok "L6-proc-нет-fail-closed" "«не знаю» не превратилось в «держит»"
elif grep -qF "REMOVED  wt" <<<"${OUT}"; then nok "L6-proc-нет-fail-closed" "снесено при неизвестном состоянии"
else ok "L6-proc-нет-fail-closed" "exit=0"; fi

# --- L7 FAIL-CLOSED: `/proc` есть, но пуст -----------------------------------------------
# Отдельно от L6: «каталога нет» и «каталог есть, а записей в нём нет» — разные ветки кода,
# и вторая выглядит как честный ответ «никто не держит». Она им не является.
read -r main wt < <(mk_case l7) || { sfail "L7-proc-пуст-fail-closed" "фикстура"; return; }
emptyproc="${WORK}/empty-proc"; mkdir -p "${emptyproc}"; register "${emptyproc}"
OUT="$(cd "${main}" && GC_PROC_ROOT="${emptyproc}" bash "${SUT_ACTIVE}" 2>&1)"; RC=$?
if [ "${RC}" -ne 0 ]; then nok "L7-proc-пуст-fail-closed" "exit=${RC}"
elif ! grep -qF "ЖИВОЙ процесс держит cwd" <<<"${OUT}"; then nok "L7-proc-пуст-fail-closed" "пустой proc принят за «никто не держит»"
else ok "L7-proc-пуст-fail-closed" "exit=0"; fi

# --- L8 RECLAIM: живой процесс — `target/` НЕ забирается ---------------------------------
# Порог молчания этого не ловит: процесс может держать дерево и молчать (бутстрап, ожидание
# ввода, пауза между шагами). Ровно так и было 24.08.
read -r main wt < <(mk_case l8) || { sfail "L8-reclaim-живой" "фикстура"; return; }
mkdir -p "${wt}/target"; echo x > "${wt}/target/x"; touch -d '2020-01-01' "${wt}/target"
mk_pgrep "${main}" 1 || { sfail "L8-reclaim-живой" "поддельный pgrep"; return; }
FAKE_BIN="${main}/bin"
q="$(spawn_in "${wt}")"
GC_PROC_ROOT="$(proc_of "${main}" "${q}")"; export GC_PROC_ROOT
expect_gc "L8-reclaim-живой" "${main}" 0 "ЖИВОЙ процесс держит cwd" "RECLAIMED" --reclaim 0
if [ -f "${wt}/target/x" ]; then ok "L8-target-уцелел"; else nok "L8-target-уцелел" "кэш снесён под живой сборкой"; fi
unset GC_PROC_ROOT
kill "${q}" 2>/dev/null

# --- L9 ПОЗИТИВНЫЙ КОНТРОЛЬ RECLAIM: без процесса `target/` забирается -------------------
read -r main wt < <(mk_case l9) || { sfail "L9-reclaim-без-процесса" "фикстура"; return; }
mkdir -p "${wt}/target"; echo x > "${wt}/target/x"; touch -d '2020-01-01' "${wt}/target"
mk_pgrep "${main}" 1 || { sfail "L9-reclaim-без-процесса" "поддельный pgrep"; return; }
FAKE_BIN="${main}/bin"
GC_PROC_ROOT="$(benign_proc "${main}")"; export GC_PROC_ROOT
expect_gc "L9-reclaim-без-процесса" "${main}" 0 "RECLAIMED" "ЖИВОЙ процесс" --reclaim 0
if [ -f "${wt}/target/x" ]; then nok "L9-target-забран" "кэш остался — сторож переблокировал"; else ok "L9-target-забран"; fi
unset GC_PROC_ROOT

# --- L11 НЕЧИТАЕМЫЙ cwd СВОЕГО процесса = «не знаю» ⇒ «держит» (C-140-1) ----------------
# Воспроизведение находки критика ИСПОЛНЕНИЕМ: прежняя редакция делала `continue` и доходила
# до удаления. Каталог PID перечисляем (glob его видит), а `readlink` по cwd падает —
# смоделировано `chmod 000` на каталоге PID: снять право поиска у СВОЕГО каталога можно без
# root, и это ровно то, что видит сторож при чужих правах.
read -r main wt < <(mk_case l11) || { sfail "L11-нечитаемый-cwd-свой" "фикстура"; return; }
fp="${main}/fakeproc-l11"; mkdir -p "${fp}/4242"; register "${fp}"
ln -s "${wt}" "${fp}/4242/cwd"
chmod 000 "${fp}/4242"
OUT="$(cd "${main}" && GC_PROC_ROOT="${fp}" bash "${SUT_ACTIVE}" 2>&1)"; RC=$?
chmod 755 "${fp}/4242"
if [ "${RC}" -ne 0 ]; then nok "L11-нечитаемый-cwd-свой" "exit=${RC}"
elif ! grep -qF "ЖИВОЙ процесс держит cwd" <<<"${OUT}"; then nok "L11-нечитаемый-cwd-свой" "нечитаемый cwd СВОЕГО процесса принят за «не держит»"
elif grep -qF "REMOVED  wt" <<<"${OUT}"; then nok "L11-нечитаемый-cwd-свой" "снесено при неопрошенном процессе"
else ok "L11-нечитаемый-cwd-свой" "exit=0"; fi

# --- L12 АЛИАС каталога: тот же dev:inode, другой текст пути (C-140-2) -------------------
# Критик предъявил `WOULD-REMOVE` при cwd на алиасе того же дерева: текстовое сравнение
# слепо к bind-mount и к пути через симлинк. Настоящий `mount --bind` без root не создать,
# поэтому алиас строится симлинком — но проверяется НЕ он, а то, что решение принимается по
# устройству:иноду, а не по строке: `readlink` даёт путь БЕЗ префикса дерева.
read -r main wt < <(mk_case l12) || { sfail "L12-алиас-того-же-каталога" "фикстура"; return; }
ln -s "${wt}" "${main}/alias"
fp="${main}/fakeproc-l12"; mkdir -p "${fp}/4343"; register "${fp}"
ln -s "${main}/alias" "${fp}/4343/cwd"
OUT="$(cd "${main}" && GC_PROC_ROOT="${fp}" bash "${SUT_ACTIVE}" 2>&1)"; RC=$?
if [ "${RC}" -ne 0 ]; then nok "L12-алиас-того-же-каталога" "exit=${RC}"
elif ! grep -qF "ЖИВОЙ процесс держит cwd" <<<"${OUT}"; then nok "L12-алиас-того-же-каталога" "алиас дерева принят за посторонний путь"
elif grep -qF "REMOVED  wt" <<<"${OUT}"; then nok "L12-алиас-того-же-каталога" "снесено при cwd на алиасе"
else ok "L12-алиас-того-же-каталога" "exit=0"; fi

# --- L13 ПОСТОРОННИЙ путь с тем же ПРЕФИКСОМ не держит (ложное удержание — тоже дефект) --
# Зеркало L12: сторож обязан не хватать лишнего. `/tmp/wt-foo` не есть `/tmp/wt`.
read -r main wt < <(mk_case l13) || { sfail "L13-похожий-префикс-не-держит" "фикстура"; return; }
mkdir -p "${wt}-foo"
fp="${main}/fakeproc-l13"; mkdir -p "${fp}/4444"; register "${fp}"
ln -s "${wt}-foo" "${fp}/4444/cwd"
OUT="$(cd "${main}" && GC_PROC_ROOT="${fp}" bash "${SUT_ACTIVE}" 2>&1)"; RC=$?
if [ "${RC}" -ne 0 ]; then nok "L13-похожий-префикс-не-держит" "exit=${RC}"
elif grep -qF "ЖИВОЙ процесс держит cwd" <<<"${OUT}"; then nok "L13-похожий-префикс-не-держит" "ЛОЖНОЕ удержание по совпадению префикса"
elif ! grep -qF "REMOVED  wt" <<<"${OUT}"; then nok "L13-похожий-префикс-не-держит" "не снесено, хотя держателя нет"
else ok "L13-похожий-префикс-не-держит" "exit=0"; fi

# --- L14 ИСЧЕЗНУВШИЙ PID держателем НЕ является (C-141 R2-1) -----------------------------
# Находка критика круга 2: ветка, отличающая исчезнувший PID от живого с нечитаемым cwd, не
# была запиннена НИЧЕМ. Внешний мутант, превращающий мёртвый PID в держателя, проходил все
# 18 сценариев и все шесть kill-set'ов — и при этом блокировал GC навсегда. Ложное зелёное:
# регрессия сделала бы мёртвый PID УНИВЕРСАЛЬНЫМ держателем, выключив и обычный GC, и reclaim.
#
# ДЕТЕРМИНИЗМ ВМЕСТО ГОНКИ. Настоящая гонка «PID исчез между glob и readlink» невоспроизводима
# по расписанию, а мигающий сценарий объявят шумом и выключат. Моделируется НАБЛЮДАЕМОЕ
# состояние, а не тайминг: запись перечислима (`-e` истинно), но каталогом процесса НЕ
# является — ровно то, что видит сторож на месте исчезнувшего PID. Предел назван: сценарий
# пиннит РЕАКЦИЮ на это состояние, а не сам факт гонки.
read -r main wt < <(mk_case l14) || { sfail "L14-мёртвый-PID-не-держит" "фикстура"; return; }
fp="${main}/fakeproc-l14"; mkdir -p "${fp}"; register "${fp}"
: > "${fp}/4242"          # перечислимо, но не каталог процесса
OUT="$(cd "${main}" && GC_PROC_ROOT="${fp}" bash "${SUT_ACTIVE}" 2>&1)"; RC=$?
if [ "${RC}" -ne 0 ]; then nok "L14-мёртвый-PID-не-держит" "exit=${RC}"
elif grep -qF "ЖИВОЙ процесс держит cwd" <<<"${OUT}"; then nok "L14-мёртвый-PID-не-держит" "исчезнувший PID стал УНИВЕРСАЛЬНЫМ держателем — GC заблокирован навсегда"
elif ! grep -qF "REMOVED  wt" <<<"${OUT}"; then nok "L14-мёртвый-PID-не-держит" "не снесено, хотя держателя нет"
else ok "L14-мёртвый-PID-не-держит" "exit=0"; fi

# --- L10 существующий страж «идёт сборка» НЕ ослаблен новым сторожем ---------------------
# Обратная мутация (`testing.md`: «второй вопрос — что пришлось ослабить рядом»). Правка
# добавила сторож РЯДОМ с уже работавшим fail-closed'ом; сценарий сторожит, что старый
# по-прежнему отказывает целиком, а не подменён новым.
read -r main wt < <(mk_case l10) || { sfail "L10-сборка-отказ-цел" "фикстура"; return; }
mkdir -p "${wt}/target"; echo x > "${wt}/target/x"; touch -d '2020-01-01' "${wt}/target"
mk_pgrep "${main}" 0 || { sfail "L10-сборка-отказ-цел" "поддельный pgrep"; return; }
FAKE_BIN="${main}/bin"
expect_gc "L10-сборка-отказ-цел" "${main}" 1 "GC REFUSED" "RECLAIMED" --reclaim 0
FAKE_BIN=""
if [ -f "${wt}/target/x" ]; then ok "L10-target-уцелел"; else nok "L10-target-уцелел" "кэш снесён при активной сборке"; fi

}

# ═══ Батарея мутантов ═══════════════════════════════════════════════════════════════════
# Мутант целится в НЕСУЩУЮ строку шва. Замена на no-op, а не вырезание: удаление строки
# внутри `if`/`for` оставляет пустое тело, мутант перестаёт парситься, и проба мерила бы
# синтаксис вместо инварианта.
battery() {
  local mutants=(
    # G2 («снять проверку существования /proc») УДАЛЁН вместе с самой проверкой: его
    # kill-set приходил ПУСТЫМ — случай уже закрывал страж `readable`, и защищённой строка
    # не была. Правильный ответ на непиннящуюся строку — убрать её, а не подогнать сценарий.
    # РАЗДЕЛИТЕЛЬ `~|~`, а не голый `|`: needle'ы содержат `||`, и `${rest%%|*}` резал их
    # ПОСЕРЕДИНЕ оператора. Мутанты G2 и G5 строились из обрубка и не парсились — батарея
    # печатала SETUP-FAIL и выглядела как дефект предмета, хотя дефект был в ней самой.
    'G1-сторож-молчит~|~[ "$found" = "1" ]~|~false~|~L11-нечитаемый-cwd-свой L12-алиас-того-же-каталога L2-живой-в-дереве L2-каталог-уцелел L3-живой-в-подкаталоге L5-cwd-deleted L8-reclaim-живой L8-target-уцелел'
    'G3-не-снят-deleted~|~t="${t% (deleted)}"~|~:~|~L5-cwd-deleted'
    'G4-нет-сторожа-в-reclaim~|~if hp="$(holder_pids "$wt")"; then\n      echo "KEEP-CACHE~|~if false; then\n      echo "KEEP-CACHE~|~L8-reclaim-живой L8-target-уцелел'
    # G5 роняет ОБА fail-closed сценария, и это не недосмотр: после удаления избыточной
    # проверки `-d` страж `readable` — ЕДИНСТВЕННЫЙ, кто отвечает «не знаю» и при
    # отсутствующем `/proc` (L6), и при пустом (L7). Обе принадлежности заявлены явно,
    # иначе kill-set «сошёлся» бы по недосмотру.
    'G5-нет-readable-стража~|~[ "$readable" = "1" ] || { echo "?"; return 0; }~|~:~|~L6-proc-нет-fail-closed L7-proc-пуст-fail-closed'
    # G6/G7 — мутанты, восстанавливающие ровно те две дыры, которые нашёл C-140.
    # Needle G6 — САМА несущая строка, а не условие вокруг неё: условие содержит кавычки,
    # которые в одинарно-кавыченном элементе массива не экранируются, и мутант строился из
    # обрубка (SETUP-FAIL читался как дефект предмета — второй раз за круг).
    'G6-скип-нечитаемого-cwd~|~echo "${pid}?"; found=1~|~:~|~L11-нечитаемый-cwd-свой'
    'G7-нет-сравнения-инода~|~[ -n "${cwdid}" ] && [ "${cwdid}" = "${wtid}" ]~|~false~|~L12-алиас-того-же-каталога'
    # G8 — ДОСЛОВНО мутант критика из C-141 R2-1: мёртвый PID объявляется держателем.
    'G8-мёртвый-PID-держит~|~      [ -d "$p" ] || continue~|~      [ -d "$p" ] || { echo "${pid}?"; found=1; continue; }~|~L14-мёртвый-PID-не-держит'
  )
  local bfail=0 spec
  for spec in "${mutants[@]}"; do
    local name="${spec%%~|~*}" rest="${spec#*~|~}"
    local needle="${rest%%~|~*}"; rest="${rest#*~|~}"
    local repl="${rest%%~|~*}"; local declared="${rest##*~|~}"
    local mut="${WORK}/mutant-${name}.sh"; register "${mut}"
    python3 - "${SUT}" "${mut}" "${needle}" "${repl}" <<'PYEOF'
import io,sys
src,dst,needle,repl=sys.argv[1],sys.argv[2],sys.argv[3],sys.argv[4]
needle=needle.replace('\\n','\n'); repl=repl.replace('\\n','\n')
s=io.open(src,encoding='utf-8').read()
io.open(dst,'w',encoding='utf-8').write(s.replace(needle,repl,1))
PYEOF
    if cmp -s "${SUT}" "${mut}"; then
      printf 'SETUP-FAIL %-28s мутант не построен: «%s» не найдено\n' "${name}" "${needle}"
      bfail=$((bfail + 1)); continue
    fi
    if ! bash -n "${mut}" 2>/dev/null; then
      printf 'SETUP-FAIL %-28s мутант не парсится\n' "${name}"; bfail=$((bfail + 1)); continue
    fi
    local bp=${PASS} bf=${FAIL}; local saved=("${FAILED_NAMES[@]+"${FAILED_NAMES[@]}"}")
    PASS=0; FAIL=0; FAILED_NAMES=()
    SUT_ACTIVE="${mut}"; local out; out="$(scenarios 2>&1)"; SUT_ACTIVE="${SUT}"
    local killed; killed="$(grep -E '^(FAIL|SETUP-FAIL)' <<<"${out}" | awk '{print $2}' | sort | tr '\n' ' ')"
    PASS=${bp}; FAIL=${bf}; FAILED_NAMES=("${saved[@]+"${saved[@]}"}")
    local expect_set; expect_set="$(tr ' ' '\n' <<<"${declared}" | sed '/^$/d' | sort | tr '\n' ' ')"
    if [ "${killed}" = "${expect_set}" ]; then
      printf 'ok         %-28s kill-set совпал (%s)\n' "${name}" "${expect_set}"
    else
      printf 'FAIL       %-28s kill-set РАЗОШЁЛСЯ\n' "${name}"
      printf '           заявлено: %s\n           получено: %s\n' "${expect_set}" "${killed}"
      bfail=$((bfail + 1))
    fi
  done
  return ${bfail}
}

# ═══ Прогон ═════════════════════════════════════════════════════════════════════════════
[ -f "${SUT}" ] || { echo "SETUP-FAIL: предмет ${SUT} не найден"; exit 1; }
[ -d /proc ] || { echo "SETUP-FAIL: /proc недоступен — проба меряет прод-форму сторожа и без него недействительна"; exit 1; }

echo "── СЦЕНАРИИ (позитивный контроль + анти-плацебо в обе стороны + fail-closed)"
scenarios
BATT=0
if [ "${1:-}" = "--battery" ]; then
  echo; echo "── БАТАРЕЯ МУТАНТОВ (равенство kill-set'ов)"; battery || BATT=$?
fi

echo
echo "сценариев исполнено: $((PASS + FAIL))  ok: ${PASS}  FAIL: ${FAIL}"
[ ${#FAILED_NAMES[@]} -gt 0 ] && printf 'упали: %s\n' "${FAILED_NAMES[*]}"
cleanup; trap - EXIT
TMP_AFTER="$(own_dirs)"
echo "каталогов red-gclive-* до: ${TMP_BEFORE}, после уборки: ${TMP_AFTER}"
if [ "${TMP_AFTER}" -gt "${TMP_BEFORE}" ]; then
  echo "VERDICT: FAIL — проба оставила за собой фикстуры"; exit 1
fi
if [ "${FAIL}" -eq 0 ] && [ "${BATT}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (сценариев: ${FAIL}, мутантов с разошедшимся kill-set: ${BATT})"; exit 1
