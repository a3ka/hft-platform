#!/usr/bin/env bash
# red_milestone_shape.sh — проба барьера `check_milestone_shape.sh`.
#
# Проба обязана быть КРАСНОЙ против обманных стабов и ЗЕЛЁНОЙ против честной реализации
# (`docs/workflow/harness-track.md` §5). Каждый сценарий несёт setup-guard: проба, молча
# тестирующая не тот сценарий, — плацебо самой себя (`testing.md`, целостность гейта, св. 3).
#
# ЧТО ИМЕННО ПРОБА ПИННИТ — по одному сценарию на каждое ослабление, а не «вообще форму»:
#   заголовок засчитывается ТОЛЬКО в видимом теле  → сценарии фенса/комментария (`C-101` B-1)
#   заголовок, а не вхождение слова                → «имя раздела только в прозе» (B-2)
#   переименование есть ВВЕДЕНИЕ в зону            → сценарии rename (B-3)
# и батарея ослаблений в конце: проба обязана покраснеть против КАЖДОГО из них, иначе
# соответствующее свойство не запиннено ничем.
#
# УБОРКА: всё временное живёт под ОДНИМ корнем `$SBOX`, снимаемым `trap EXIT` целиком, и проба
# ПЕЧАТАЕТ ЧИСЛО остатка. Класс, ради которого: 10 400 каталогов `/tmp/red-freeze-*` и диск на
# 100 %. Первая редакция этой пробы держала реестр отдельных путей и чистила только каталоги
# (`-d`) — собственный замер показал «остаточных 3» (два стаба и out-файл), и конструкция была
# заменена на единый корень. Замер уборки в выводе — не украшение: он и поймал эту течь.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER_OVERRIDE:-$ROOT/scripts/check_milestone_shape.sh}"
# ВСЁ временное — внутри ОДНОГО корня. Реестр отдельных путей отвергнут замером: первая
# редакция держала список и чистила только каталоги (`-d`), из-за чего стабы и out-файл
# переживали уборку («остаточных 3» в собственном выводе пробы), а вложенный self-test плодил
# свои. Один корень убирается целиком и корректно при любой вложенности.
SBOX="$(mktemp -d /tmp/red-mshape-root-XXXXXX)"
REGISTRY="$SBOX/registry"; : > "$REGISTRY"
OUT="$SBOX/out"
PASS=0; FAIL=0

cleanup() {
  rm -rf "$SBOX"
  local leaked
  leaked=$(find /tmp -maxdepth 1 -name 'red-mshape-*' 2>/dev/null | wc -l)
  echo "уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: $leaked"
  [ "$leaked" -eq 0 ] || echo "ВНИМАНИЕ: проба течёт — $leaked объектов осталось" >&2
}
trap cleanup EXIT

ok()   { PASS=$((PASS + 1)); echo "  PASS: $*"; }
bad()  { FAIL=$((FAIL + 1)); echo "  FAIL: $*" >&2; }

# Полная спека — эталон формы.
full_spec() {
  cat <<'EOF'
# M-99 — тестовая спека
## Objective
Цель.
## Allowed paths
| путь | кто |
## §Tasks
| # | Status |
## Acceptance
`scripts/verify_M-99.sh`
EOF
}

# Спека, где `Allowed paths` существует ТОЛЬКО как ПРИМЕР внутри fenced-code (`C-101` B-1).
# Настоящего раздела у документа нет — барьер обязан отказать.
spec_section_in_fence() {
  local fence="${1:-\`\`\`}"
  cat <<EOF
# M-99 — тестовая спека
## Objective
Цель.
## §Tasks
| # | Status |
## Acceptance
\`scripts/verify_M-99.sh\`

Ниже — ОБРАЗЕЦ формы, а не раздел этого документа:

${fence}markdown
## Allowed paths
| путь | кто |
${fence}
EOF
}

# То же, но раздел спрятан в HTML-комментарии (черновик, который забыли раскомментировать).
spec_section_in_comment() {
  cat <<'EOF'
# M-99 — тестовая спека
## Objective
Цель.
## §Tasks
| # | Status |
## Acceptance
`scripts/verify_M-99.sh`

<!--
## Allowed paths
| путь | кто |
-->
EOF
}

# Спека, где ИМЯ раздела встречается в прозе, но заголовка нет. Честный барьер якорит
# заголовок и отказывает; substring-стаб (`grep -qi -- "Acceptance"`) — принимает.
# Это ЕДИНСТВЕННЫЙ сценарий, различающий их, и он введён по `C-101` B-2.
spec_name_in_prose_only() {
  cat <<'EOF'
# M-99 — тестовая спека
## Objective
Цель.
## Allowed paths
| путь | кто |
## §Tasks
| # | Status |
Acceptance описан прозой в теле задач, отдельного раздела нет.
EOF
}

# `C-173` B-5: фенс ОТКРЫТ ```, а «закрыт» ~~~. По CommonMark забор закрывается тем же
# символом, значит блок НЕ закрыт и `## Allowed paths` внутри него — не раздел документа.
spec_mismatched_fence() {
  cat <<'EOF'
# M-99 — тестовая спека
## Objective
Цель.
## §Tasks
| # | Status |
## Acceptance
`scripts/verify_M-99.sh`

```markdown
~~~
## Allowed paths
| путь | кто |
EOF
}

# `C-175` B-8.1: забор открыт ЧЕТЫРЬМЯ бэктиками, «закрыт» ТРЕМЯ. По CommonMark закрывающий
# забор не может быть короче открывающего — блок не закрыт, раздел внутри него не раздел.
spec_shorter_closing_run() {
    printf '%s\n' \
      '# M-99 — тестовая спека' '## Objective' 'Цель.' '## §Tasks' '| # | Status |' \
      '## Acceptance' '`scripts/verify_M-99.sh`' '' '````markdown' '```' \
      '## Allowed paths' '| путь | кто |'
}

# `C-175` B-8.2: строка забора несёт ТЕКСТ после маркера — это содержимое блока, а не
# закрытие (закрывающий забор допускает после себя только пробелы).
spec_closing_fence_with_trailing_text() {
    printf '%s\n' \
      '# M-99 — тестовая спека' '## Objective' 'Цель.' '## §Tasks' '| # | Status |' \
      '## Acceptance' '`scripts/verify_M-99.sh`' '' '```markdown' '``` not-a-closing-fence' \
      '## Allowed paths' '| путь | кто |'
}

# `C-173` B-6: `##Allowed paths` без пробела — не ATX-заголовок, а обычная строка.
spec_no_space_after_hashes() {
  cat <<'EOF'
# M-99 — тестовая спека
## Objective
Цель.
##Allowed paths
| путь | кто |
## §Tasks
| # | Status |
## Acceptance
`scripts/verify_M-99.sh`
EOF
}

# `C-173` B-7.3: глубина заголовка ЗНАЧИМА — форма допускает `## X` и `### X`, не `#### X`.
spec_h4_section() {
  cat <<'EOF'
# M-99 — тестовая спека
## Objective
Цель.
#### Allowed paths
| путь | кто |
## §Tasks
| # | Status |
## Acceptance
`scripts/verify_M-99.sh`
EOF
}

# Позитивный контроль к фиксу B-1: ЗАКРЫТЫЙ фенс не должен скрывать разделы ПОСЛЕ себя.
# Без этого сценария фикс «выкинуть всё после первого ```» прошёл бы пробу.
spec_fence_then_real_sections() {
  cat <<'EOF'
# M-99 — тестовая спека
## Objective
Цель.

```bash
echo "пример вызова"
```

## Allowed paths
| путь | кто |
## §Tasks
| # | Status |
## Acceptance
`scripts/verify_M-99.sh`
EOF
}

# Песочница: git-репозиторий с базой и одним коммитом поверх.
sandbox() {
  local d; d="$(mktemp -d "$SBOX/sandbox-XXXXXX")"
  git -C "$d" init -q
  git -C "$d" config user.email t@t; git -C "$d" config user.name t
  mkdir -p "$d/milestones" "$d/scripts"
  cp "$BARRIER" "$d/scripts/check_milestone_shape.sh"
  chmod +x "$d/scripts/check_milestone_shape.sh"
  echo seed > "$d/seed.txt"
  git -C "$d" add -A >/dev/null; git -C "$d" commit -qm base
  echo "$d"
}

run_barrier() {  # $1=dir  → печатает exit-код
  local d="$1" base
  base="$(git -C "$d" rev-parse HEAD~1 2>/dev/null || git -C "$d" rev-parse HEAD)"
  ( cd "$d" && EVENT_NAME=pull_request PR_BASE_SHA="$base" \
      bash scripts/check_milestone_shape.sh >"$OUT" 2>&1; echo $? )
}

scenario() {  # $1=имя  $2=ожидаемый_код  $3=тело_спеки_или_MISSING  $4=режим(add|modify|none)
  local name="$1" want="$2" body="$3" mode="$4"
  local d; d="$(sandbox)"
  case "$mode" in
    add)
      printf '%s\n' "$body" > "$d/milestones/M-99-probe.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "add spec" ;;
    modify)
      printf '%s\n' "$body" > "$d/milestones/M-98-old.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "seed old spec"
      echo "правка" >> "$d/milestones/M-98-old.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "modify spec" ;;
    dirty)
      # C-173 B-7.1: закоммичена НЕПОЛНАЯ спека, а в РАБОЧЕМ ДЕРЕВЕ лежит полная.
      # Барьер обязан судить HEAD (предмет — закоммиченный диапазон), а не то, что под рукой.
      printf '%s\n' "$(full_spec | grep -v 'Allowed paths')" > "$d/milestones/M-99-probe.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "add incomplete spec"
      printf '%s\n' "$body" > "$d/milestones/M-99-probe.md" ;;   # НЕ коммитим
    unicode)
      # C-173 B-7.2: имя файла вне ASCII. В текстовом режиме git КВОТИРУЕТ его, и
      # обработка без `-z`/`mapfile -d ''` промахивается мимо файла молча.
      printf '%s\n' "$body" > "$d/milestones/M-99-кириллица.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "add unicode-named spec" ;;
    rename)
      printf '%s\n' "$body" > "$d/milestones/M-98-old.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "seed old spec"
      git -C "$d" mv milestones/M-98-old.md milestones/M-97-renamed.md
      git -C "$d" commit -qm "rename spec" >/dev/null ;;
    none)
      echo x > "$d/other.txt"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "unrelated" ;;
  esac
  # SETUP-GUARD: сценарий обязан состояться. Для add — файл ДОЛЖЕН числиться добавленным.
  if [ "$mode" = add ]; then
    git -C "$d" diff --diff-filter=A --name-only HEAD~1 HEAD -- 'milestones/M-*.md' \
      | grep -q . || { bad "$name — SETUP НЕ СОСТОЯЛСЯ: файл не числится добавленным"; return; }
  fi
  # SETUP-GUARD для dirty: рабочее дерево ОБЯЗАНО расходиться с HEAD, иначе сценарий
  # вырождается в обычный `add` и ничего не различает.
  if [ "$mode" = dirty ]; then
    git -C "$d" diff --quiet -- milestones/M-99-probe.md \
      && { bad "$name — SETUP НЕ СОСТОЯЛСЯ: дерево не расходится с HEAD"; return; }
  fi
  # SETUP-GUARD для unicode: git ОБЯЗАН квотировать имя в текстовом режиме, иначе
  # сценарий не давит на `-z` (на другой локали/конфиге quoting может быть выключен).
  if [ "$mode" = unicode ]; then
    git -C "$d" diff --diff-filter=AR --name-only HEAD~1 HEAD -- 'milestones/M-*.md' \
      | grep -q '"' || { bad "$name — SETUP НЕ СОСТОЯЛСЯ: git не квотирует не-ASCII имя"; return; }
  fi
  # SETUP-GUARD для rename: git ОБЯЗАН числить правку статусом `R`, иначе проверяется не тот
  # сценарий (при слишком мелком файле детектор переименований может дать `A`+`D`).
  if [ "$mode" = rename ]; then
    git -C "$d" diff --name-status -M HEAD~1 HEAD -- 'milestones/M-*.md' \
      | grep -q '^R' || { bad "$name — SETUP НЕ СОСТОЯЛСЯ: git не числит правку переименованием"; return; }
  fi
  local got; got="$(run_barrier "$d")"
  if [ "$got" = "$want" ]; then ok "$name (exit=$got)"; else
    bad "$name — ожидался exit=$want, получен exit=$got"; sed -n '1,6p' "$OUT" >&2
  fi
}

echo "=== ЧЕСТНАЯ РЕАЛИЗАЦИЯ: позитивный контроль + отказы ==="
scenario "полная спека принимается"                    0 "$(full_spec)"                                       add
scenario "нет Allowed paths → отказ"                   1 "$(full_spec | grep -v 'Allowed paths')"             add
scenario "нет Objective → отказ"                       1 "$(full_spec | grep -v '## Objective')"              add
scenario "нет §Tasks → отказ"                          1 "$(full_spec | grep -v '## §Tasks')"                 add
scenario "нет Acceptance → отказ"                      1 "$(full_spec | grep -v '## Acceptance')"             add
scenario "три решётки (### Objective) принимаются"     0 "$(full_spec | sed 's/^## /### /')"                  add
scenario "ИЗМЕНЁННАЯ неполная спека НЕ трогается"      0 "# M-98 — старая"                                    modify
scenario "нет новых спек — проверять нечего"           0 ""                                                    none

echo "=== СКРЫТЫЙ ТЕКСТ НЕ ЕСТЬ РАЗДЕЛ (C-101 B-1) ==="
scenario "раздел только в \`\`\`-фенсе → отказ"            1 "$(spec_section_in_fence '```')"                      add
scenario "раздел только в ~~~-фенсе → отказ"           1 "$(spec_section_in_fence '~~~')"                      add
scenario "раздел только в HTML-комментарии → отказ"    1 "$(spec_section_in_comment)"                          add
scenario "ЗАКРЫТЫЙ фенс не скрывает разделы после"     0 "$(spec_fence_then_real_sections)"                    add

echo "=== ФОРМА ЗАГОЛОВКА И ГРАНИЦА ФЕНСА (C-173 B-5/B-6/B-7.3) ==="
scenario "несовпадающий маркер фенса не закрывает блок" 1 "$(spec_mismatched_fence)"      add
scenario "##Allowed paths без пробела → отказ"          1 "$(spec_no_space_after_hashes)" add
scenario "#### Allowed paths (H4) → отказ"              1 "$(spec_h4_section)"            add

scenario "закрытие КОРОЧЕ открывающего не закрывает"  1 "$(spec_shorter_closing_run)"            add
scenario "забор с текстом после маркера не закрывает" 1 "$(spec_closing_fence_with_trailing_text)" add

echo "=== СУДИТСЯ HEAD, А НЕ РАБОЧЕЕ ДЕРЕВО (C-173 B-7.1) ==="
scenario "полная спека в дереве не спасает неполную в HEAD" 1 "$(full_spec)"              dirty

echo "=== НЕ-ASCII ИМЯ ФАЙЛА НЕ ТЕРЯЕТСЯ (C-173 B-7.2) ==="
scenario "спека с кириллицей в имени принимается"       0 "$(full_spec)"                  unicode

echo "=== ЗАГОЛОВОК, А НЕ ВХОЖДЕНИЕ СЛОВА (C-101 B-2) ==="
scenario "имя раздела только в прозе → отказ"          1 "$(spec_name_in_prose_only)"                          add

echo "=== ПЕРЕИМЕНОВАНИЕ — ВВЕДЕНИЕ В ЗОНУ (C-101 B-3) ==="
scenario "rename неполной спеки под новым именем → отказ" 1 "# M-98 — старая неполная спека, ни одного раздела"  rename
scenario "rename ПОЛНОЙ спеки принимается"                0 "$(full_spec)"                                       rename

echo "=== FAIL-CLOSED SETUP (барьер зовут не так, как зовёт CI) ==="
d="$(sandbox)"
got="$( cd "$d" && bash scripts/check_milestone_shape.sh >/dev/null 2>&1; echo $? )"
[ "$got" = 1 ] && ok "пустой EVENT_NAME → отказ (exit=1)" || bad "пустой EVENT_NAME: ожидался 1, получен $got"
got="$( cd "$d" && EVENT_NAME=pull_request PR_BASE_SHA=0000000000000000000000000000000000000000 \
        bash scripts/check_milestone_shape.sh >/dev/null 2>&1; echo $? )"
[ "$got" = 1 ] && ok "zero-SHA база → отказ (exit=1)" || bad "zero-SHA: ожидался 1, получен $got"
got="$( cd "$d" && EVENT_NAME=pull_request PR_BASE_SHA=deadbeefdeadbeefdeadbeefdeadbeefdeadbeef \
        bash scripts/check_milestone_shape.sh >/dev/null 2>&1; echo $? )"
[ "$got" = 1 ] && ok "несуществующая база → отказ (exit=1)" || bad "нет базы: ожидался 1, получен $got"

echo "=== АНТИ-ПЛАЦЕБО: обманные стабы обязаны быть ПОЙМАНЫ ==="
# Стаб 1 — «всегда успех» (классический no-op барьер).
stub1="$(mktemp "$SBOX/stub1-XXXXXX.sh")"
printf '#!/usr/bin/env bash\nexit 0\n' > "$stub1"; chmod +x "$stub1"
d="$(sandbox)"; printf '%s\n' "$(full_spec | grep -v 'Allowed paths')" > "$d/milestones/M-99-probe.md"
git -C "$d" add -A >/dev/null; git -C "$d" commit -qm add
cp "$stub1" "$d/scripts/check_milestone_shape.sh"
got="$(run_barrier "$d")"
[ "$got" = 0 ] && ok "стаб «всегда 0» пойман бы: на спеке без Allowed paths он даёт 0 вместо 1" \
               || bad "стаб «всегда 0» повёл себя неожиданно (exit=$got)"

# Стаб 2 — «отказ 127» (барьера нет / не исполняется). Отличается от честного отказа кодом.
stub2="$(mktemp "$SBOX/stub2-XXXXXX.sh")"
printf '#!/usr/bin/env bash\nexit 127\n' > "$stub2"; chmod +x "$stub2"
d="$(sandbox)"; printf '%s\n' "$(full_spec)" > "$d/milestones/M-99-probe.md"
git -C "$d" add -A >/dev/null; git -C "$d" commit -qm add
cp "$stub2" "$d/scripts/check_milestone_shape.sh"
got="$(run_barrier "$d")"
[ "$got" = 127 ] && ok "стаб «127» отличим от честного отказа (1) — страж не путает их" \
                 || bad "стаб «127» дал exit=$got"

# ── НАСТОЯЩИЙ анти-плацебо: БАТАРЕЯ стабов, проба против КАЖДОГО обязана вернуть FAIL ─
#
# ПОЧЕМУ БАТАРЕЯ, А НЕ ОДИН СТАБ (`C-101` B-2, воспроизведено). Прежняя редакция гоняла
# самопроверку против единственного стаба «всегда 0» — стаба, которого хватает любой пробе,
# и потому ничего не доказывающего. Адверсарий подменил ЯКОРЬ на вхождение подстроки
# (`grep -qi -- "$human"` вместо `grep -qiE '^#{2,3} *Allowed paths'`) — такой барьер
# принимает `Acceptance описан прозой` за раздел, — и проба прошла ЦЕЛИКОМ: `PASS=14 FAIL=0`,
# exit=0. То есть проба не пиннила именно то свойство, ради которого барьер существует.
#
# СТАБЫ ВЫВОДЯТСЯ ИЗ ЖИВОГО БАРЬЕРА, А НЕ ПИШУТСЯ РУКАМИ. Рукописный стаб протухает молча:
# он остаётся «сломанной» копией версии, которой больше нет, и проба продолжает краснеть
# против прошлогоднего кода. `sed` по текущему файлу гарантирует, что ослабление вносится
# в СЕГОДНЯШНИЙ барьер. Отсюда обязательный setup-guard: если подстановка не изменила файл,
# сценарий НЕ СОСТОЯЛСЯ, и это FAIL, а не пропуск (`testing.md` §«Целостность гейта», св. 3;
# ровно тот же класс, что exit=101 от несобравшейся мутации).
# Результат кладётся в ГЛОБАЛЬНУЮ переменную, а не печатается в `$( )`. Причина замерена на
# первой редакции этой функции: внутри подстановки команд `bad` инкрементирует счётчик В
# ПОДОБОЛОЧКЕ, сообщение печатается, а итог пробы его не считает — «FAIL=1» при ДВУХ находках.
# Проба, чьё число расходится с её же выводом, — ровно тот дефект, который она призвана ловить.
STUB_PATH=""
make_stub() {  # $1=имя  $2=sed-выражение → STUB_PATH ("" если setup не состоялся)
  local nm="$1" expr="$2" out
  STUB_PATH=""
  out="$(mktemp "$SBOX/stub-${nm}-XXXXXX.sh")"
  sed "$expr" "$BARRIER" > "$out"; chmod +x "$out"
  if cmp -s "$out" "$BARRIER"; then
    bad "стаб «${nm}» — SETUP НЕ СОСТОЯЛСЯ: подстановка ничего не изменила (якорь уехал)"
    return 1
  fi
  STUB_PATH="$out"
}

if [ -z "${MSHAPE_SELFTEST:-}" ]; then
  echo "=== САМОПРОВЕРКА: проба обязана КРАСНЕТЬ против КАЖДОГО ослабления ==="

  always0="$(mktemp "$SBOX/stub-always0-XXXXXX.sh")"
  printf '#!/usr/bin/env bash\nexit 0\n' > "$always0"; chmod +x "$always0"

  BATTERY_OK=0; BATTERY_N=0
  try_stub() {  # $1=человекочитаемое имя  $2=путь стаба
    local nm="$1" st="$2"
    [ -n "$st" ] || return
    BATTERY_N=$((BATTERY_N + 1))
    if MSHAPE_SELFTEST=1 BARRIER_OVERRIDE="$st" bash "$0" >/dev/null 2>&1; then
      bad "САМОПРОВЕРКА: проба ЗЕЛЁНАЯ против ослабления «${nm}» — она этого свойства не пиннит"
    else
      BATTERY_OK=$((BATTERY_OK + 1)); ok "проба краснеет против ослабления «${nm}»"
    fi
  }

  try_stub "всегда 0 (барьер-заглушка)" "$always0"
  # Ослабление 1 — якорь заголовка заменён вхождением слова (адверсарий `C-101` B-2).
  make_stub substring 's|grep -qiE "${re}"|grep -qi -- "${human}"|' \
    && try_stub "вхождение слова вместо заголовка" "$STUB_PATH"
  # Ослабление 2 — снят разбор скрытого текста: пример в фенсе снова сойдёт за раздел (B-1).
  make_stub fenceblind 's|visible_body "${file}"|git show "HEAD:${file}" 2>/dev/null|' \
    && try_stub "фенс/комментарий снова считаются телом" "$STUB_PATH"
  # Ослабление 3 — фильтр сужен обратно до `A`: переименование снова невидимо (B-3).
  make_stub renameblind 's|--diff-filter=AR|--diff-filter=A|' \
    && try_stub "rename снова невидим (--diff-filter=A)" "$STUB_PATH"

  # Ослабление 4 (`C-173` B-7.1) — барьер читает РАБОЧЕЕ ДЕРЕВО вместо закоммиченного объекта.
  make_stub worktree 's|git show "HEAD:$1" 2>/dev/null|cat "$1"|' \
    && try_stub "рабочее дерево вместо HEAD" "$STUB_PATH"
  # Ослабление 5 (`C-173` B-7.2) — потеряна NUL-безопасность: не-ASCII имя уходит квотированным.
  make_stub nulunsafe 's|--name-only -z|--name-only|' \
    && try_stub "не-ASCII имя теряется (без -z)" "$STUB_PATH"
  # Ослабление 6 (`C-173` B-7.3) — расширена допустимая глубина заголовка.
  make_stub h4depth 's|#{2,3} +|#{2,4} +|g' \
    && try_stub "H4 принимается как раздел" "$STUB_PATH"

  # Ослабление 8 (`C-175` B-8.1) — снята проверка ДЛИНЫ закрывающего забора.
  make_stub runlen 's|c == fchar \&\& run >= flen \&\& tail_blank|c == fchar \&\& tail_blank|' \
    && try_stub "закрытие короче открывающего снова закрывает" "$STUB_PATH"
  # Ослабление 9 (`C-175` B-8.2) — снята проверка ХВОСТА после маркера.
  make_stub fencetail 's|c == fchar \&\& run >= flen \&\& tail_blank|c == fchar \&\& run >= flen|' \
    && try_stub "забор с хвостом снова закрывает" "$STUB_PATH"

  echo "  батарея ослаблений: поймано ${BATTERY_OK} из ${BATTERY_N}"
  if [ "$BATTERY_N" -lt 9 ]; then
    bad "батарея неполна: ${BATTERY_N} ослаблений вместо 9 — стаб не собрался, значит не проверен"
  fi
fi

echo
echo "PASS=$PASS FAIL=$FAIL (сценариев: $((PASS + FAIL)))"
[ "$FAIL" -eq 0 ] && { echo "VERDICT: PASS"; exit 0; } || { echo "VERDICT: FAIL"; exit 1; }
