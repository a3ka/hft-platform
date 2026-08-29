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
# ═══ КЛАСС `C-177`: МУТАЦИЯ ОБЯЗАНА СНИМАТЬ РОВНО ОДНО СВОЙСТВО ═══
# Круг 5 отклонён не за дыру в барьере — барьер реализовал объявленную грамматику верно, — а
# за дыру В ЭТОЙ ПРОБЕ. Ослабление `htmlblind` снимало скрытие у ВСЕХ ЧЕТЫРЁХ тегов разом,
# `titleprefix` — границу титула у ВСЕХ ЧЕТЫРЁХ имён разом. Групповая мутация краснеет, если
# покрыт ХОТЬ ОДИН член группы: она пиннит ОБЪЕДИНЕНИЕ, а не каждый элемент. Отсюда две
# зелёные дыры (`C-177` B-12 `<style>`, B-13 `§Tasks`) при батарее 12/12.
#
# ЭТО ТА ЖЕ ОШИБКА МЕРЫ, что «счётчик ведёт один из двух путей» (`docs/workflow/
# oracle-blindness-class-2026-08-28.md` §1 №4): величина снимается не с того множества
# носителей, о котором делается утверждение.
#
# ПРАВИЛО, ПРИНЯТОЕ КРУГОМ 6 И ПРИМЕНЁННОЕ ЗДЕСЬ ЦЕЛИКОМ, А НЕ К ДВУМ НАЗВАННЫМ ЭКЗЕМПЛЯРАМ:
#   (а) у КАЖДОГО перечислимого члена объявленной группы есть СВОЯ фикстура;
#   (б) у КАЖДОГО есть мутация, снимающая свойство РОВНО У НЕГО, и проба обязана падать от
#       неё ПООТДЕЛЬНО;
#   (в) групповая мутация сохраняется — она пиннит МЕХАНИЗМ (наличие скрытия/якоря как
#       такового), что не то же самое, чем членство в перечне.
# Перебор всей батареи по этому правилу нашёл, помимо двух названных, ещё четыре пробела:
# скрытие фенса и скрытие HTML-комментария снимались ОДНОЙ подстановкой `fenceblind`;
# у первого конъюнкта закрытия забора (совпадение символа) мутации не было вовсе; у
# обязательного пробела после решёток была фикстура, но не было мутации.
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

# ЧЕТЫРЕ имени разделов, объявленные барьером (вызовы `check_section` в
# `check_milestone_shape.sh`; номера строк здесь НЕ фиксируются намеренно — `C-178` N-3:
# предыдущая редакция называла `:183-186`, и шапка круга 6 сдвинула файл на десять строк.
# Ссылка на ИМЯ конструкции не протухает, ссылка на номер строки протухает при любой правке). Перечень
# ЖИВЁТ ОДИН РАЗ: фикстуры и пер-членные мутации ходят по нему, поэтому появление пятого
# раздела не может добавить сценарий и забыть мутацию — они порождаются вместе.
SECTIONS=(Objective 'Allowed paths' '§Tasks' Acceptance)

# ЧЕТЫРЕ формы сырого HTML, объявленные барьером (альтернация тегов в `visible_body`).
HTML_TAGS=(pre script style textarea)

# ЧЕТЫРЕ ТЕРМИНАТОРА открывающего тега — вторая группа В ТОЙ ЖЕ строке барьера, и `C-178` F-1
# нашёл её незапиннутой: `[ \t>]` есть символьный класс из трёх форм (пробел · таб · `>`), а
# альтернация `|| match(lower, /<(…)$/)` — четвёртая (конец строки). Круг 6 разложил ТЕГИ и
# счёл группу закрытой; терминаторы упражнялись ровно одним членом (`>`), потому что все
# фикстуры строили форму `<tag>`. Стабы `[ \t>]`→`[>]` и снятие альтернации `…$` оставляли
# полную пробу ЗЕЛЁНОЙ (47/0).
#
# ВЫВОД, КОТОРЫЙ ЭТО ДОБАВЛЯЕТ К ПРАВИЛУ КРУГА 6: опасна ровно та группа, которая НЕ ВЫПИСАНА.
# Вынесение перечня в массив её обезвреживает — и потому создаёт иллюзию, что закрыты все.
# Группа, живущая ВНУТРИ регекспа или ВНУТРИ цепочки `if`-стражей, перебором по массивам не
# накрывается. Отсюда этот массив: терминаторы теперь тоже ВЫПИСАНЫ.
# `%s` — место тега; форма подставляется в `spec_section_in_raw_html`.
HTML_OPEN_FORMS=('<%s>|>' '<%s src="x.js">|пробел' '<%s\tsrc="x.js">|таб' '<%s|конец строки')

# ── ОБЩИЙ СТРОИТЕЛЬ ИСКАЖЁННОГО ЗАГОЛОВКА ───────────────────────────────────────────
# Берёт ПОЛНУЮ спеку и портит ровно ОДИН заголовок — тот, что назван. Остальные три
# остаются целыми, поэтому отказ барьера однозначно относится к названному разделу, а
# соответствующая пер-членная мутация краснеет только через ЭТУ фикстуру.
#
# FAIL-CLOSED ПО ПОСТРОЕНИЮ: не совпала подстановка (опечатка в имени, сменилась форма
# `full_spec`) — на выходе остаётся ПОЛНАЯ спека, барьер вернёт 0 там, где сценарий ждёт 1,
# и сценарий упадёт ГРОМКО. Обратное направление (пустой вывод при промахе) дало бы ложное
# зелёное — сценарий «раздела нет» прошёл бы, ничего не проверив.
spec_with_broken_heading() {   # $1=имя раздела  $2=строка, которой заменяется его заголовок
  full_spec | sed "s|^## $1\$|$2|"
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

# `C-176` B-9: сырой HTML-блок. По CommonMark pre/script/style/textarea открывают блок до
# СВОЕГО закрывающего тега; разметка внутри разметкой не является.
spec_section_in_raw_html() {   # $1=тег  $2=printf-форма открывающего тега (по умолч. `<tag>`)
    tag="${1:-script}"
    local fmt="${2:-<%s>}"
    local open; open="$(printf "$fmt" "$tag")"
    printf '%s\n' \
      '# M-99 — тестовая спека' '## Objective' 'Цель.' '## §Tasks' '| # | Status |' \
      '## Acceptance' '`scripts/verify_M-99.sh`' '' "${open}" '## Allowed paths' "</${tag}>"
}

# `C-176` B-10 + `C-177` B-13: ИНОЙ заголовок, разделяющий требуемый ПРЕФИКС. Регексп без
# границы титула принимал его за раздел. Зовётся по КАЖДОМУ из четырёх имён: до круга 6
# фикстур было три, и снятие границы у одного лишь `§Tasks` проходило пробу зелёным.
spec_title_prefix_only() { spec_with_broken_heading "$1" "## $1NOT-A-SECTION"; }

# `C-176` B-11: отступ. Объявленная грамматика — заголовок с КОЛОНКИ 0; всё отступленное
# разделом не считается (сужение против CommonMark названо в шапке барьера).
spec_indented_section() { spec_with_broken_heading "$2" "$1## $2"; }   # $1=отступ $2=имя

# Позитивные контроли объявленной грамматики: закрывающая ATX-последовательность и хвостовые
# пробелы — ВАЛИДНЫЙ заголовок, и барьер обязан его принять. Без них фикс «требовать конец
# строки сразу после титула» был бы КРАСНЫМ против правильной спеки.
spec_closing_atx_sequence() {
    printf '%s\n' \
      '# M-99 — тестовая спека' '## Objective' 'Цель.' '## Allowed paths ##' '| путь |' \
      '## §Tasks' '| # |' '## Acceptance' '`v.sh`'
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
spec_no_space_after_hashes() { spec_with_broken_heading "$1" "##$1"; }

# `C-173` B-7.3: глубина заголовка ЗНАЧИМА — форма допускает `## X` и `### X`, не `#### X`.
spec_h4_section() { spec_with_broken_heading "$1" "#### $1"; }

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

# ПРОД-МАСШТАБ (`testing.md` §«Дегенерированный вход», пункт 5). Все фикстуры выше — спеки
# по девять строк, и ровно этот размер СКРЫВАЛ дефект барьера семь кругов подряд: при
# коротком входе `awk` успевает дописать всё раньше, чем `grep -q` выйдет по первому
# совпадению, и SIGPIPE не наступает. На первой РЕАЛЬНОЙ спеке (`M-72`, 127 строк) барьер
# отверг три раздела из четырёх, все четыре присутствуя, — исход зависел от того, где в файле
# лежит раздел, и от планировщика.
#
# Заполнитель намеренно ДЛИННЕЕ буфера трубы (64 KiB): на коротком входе дефект флаковал бы,
# на длинном он ДЕТЕРМИНИРОВАН — `grep -q` выходит на `Objective` во второй строке, когда у
# производителя остаются десятки тысяч байт. Фикстура, воспроизводящая дефект через раз,
# была бы флаком, а не оракулом (`testing.md`, целостность гейта, свойство 2).
spec_prod_scale() {
  full_spec
  printf 'Заполнитель прод-масштаба: строка %s — реальная спека длиннее девяти строк.\n' $(seq 1 2000)
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

echo "=== ГРАНИЦА ФЕНСА (C-173 B-5, C-175 B-8) ==="
scenario "несовпадающий маркер фенса не закрывает блок" 1 "$(spec_mismatched_fence)"      add
scenario "закрытие КОРОЧЕ открывающего не закрывает"  1 "$(spec_shorter_closing_run)"            add
scenario "забор с текстом после маркера не закрывает" 1 "$(spec_closing_fence_with_trailing_text)" add

# `C-177` B-12: барьер объявляет ЧЕТЫРЕ формы сырого HTML — значит фикстур обязано быть
# четыре. Круг 5 нёс три (`pre`/`script`/`textarea`), и снятие ОДНОГО лишь `style` из
# альтернации проходило пробу зелёным: 36/36.
echo "=== СЫРОЙ HTML-БЛОК НЕ ЕСТЬ ВИДИМОЕ ТЕЛО (C-176 B-9; все ЧЕТЫРЕ формы — C-177 B-12) ==="
for t in "${HTML_TAGS[@]}"; do
  scenario "<$t> прячет раздел → отказ"              1 "$(spec_section_in_raw_html "$t")" add
done

# `C-178` F-1: тег открывает блок ЧЕТЫРЬМЯ терминаторами, а фикстуры знали один (`>`).
# Каждый член перечня получает свой сценарий; тег берётся один (`script`) — предмет здесь
# терминатор, а теги уже покрыты циклом выше.
echo "=== ТЕРМИНАТОР ОТКРЫВАЮЩЕГО ТЕГА — ЧЕТЫРЕ ФОРМЫ (C-178 F-1) ==="
for spec in "${HTML_OPEN_FORMS[@]}"; do
  fmt="${spec%%|*}"; human="${spec##*|}"
  scenario "<script> открыт через «$human» → отказ"  1 "$(spec_section_in_raw_html script "$fmt")" add
done

# `C-177` B-13: та же мера — по КАЖДОМУ из четырёх имён, а не по трём из четырёх.
echo "=== ТИТУЛ СУДИТСЯ ЦЕЛИКОМ, А НЕ ПРЕФИКСОМ (C-176 B-10; все ЧЕТЫРЕ имени — C-177 B-13) ==="
for sec in "${SECTIONS[@]}"; do
  scenario "${sec}NOT-A-SECTION → отказ"             1 "$(spec_title_prefix_only "$sec")" add
done
scenario "закрывающая ATX ## принимается"            0 "$(spec_closing_atx_sequence)"     add

echo "=== ЗАГОЛОВОК С КОЛОНКИ 0 — ОБЪЯВЛЕННОЕ СУЖЕНИЕ (C-176 B-11), по КАЖДОМУ имени ==="
for sec in "${SECTIONS[@]}"; do
  scenario "отступ 4 пробела у «$sec» → отказ"       1 "$(spec_indented_section '    ' "$sec")" add
done
scenario "отступ 1 пробел → отказ (сужение)"         1 "$(spec_indented_section ' ' 'Allowed paths')" add

echo "=== ГЛУБИНА И ОБЯЗАТЕЛЬНЫЙ ПРОБЕЛ (C-173 B-6/B-7.3), по КАЖДОМУ имени ==="
for sec in "${SECTIONS[@]}"; do
  scenario "#### $sec (H4) → отказ"                  1 "$(spec_h4_section "$sec")"            add
  scenario "##$sec без пробела → отказ"              1 "$(spec_no_space_after_hashes "$sec")" add
done

echo "=== ПРОД-МАСШТАБ: СПЕКА ДЛИННЕЕ БУФЕРА ТРУБЫ (M-72) ==="
scenario "спека прод-масштаба принимается"           0 "$(spec_prod_scale)" add

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

# `C-178` F-2: цепочка стражей базы — ЧЕТВЁРТАЯ группа того же класса (пусто · zero-SHA ·
# несуществующий коммит · НЕ-ПРЕДОК). Три первые пиннились сценариями выше; у четвёртого не
# было ни сценария, ни мутации, и стаб `merge-base --is-ancestor → true` проходил пробу
# зелёным. Барьер обещает «история переписана ⇒ что введено, недоказуемо ⇒ отказ» — обещание
# жило только в живом коде и не держалось ничем.
d="$(sandbox)"
printf '%s\n' "$(full_spec | grep -v 'Allowed paths')" > "$d/milestones/M-99-probe.md"
git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "add incomplete spec"
git -C "$d" checkout -q -b side
echo side > "$d/side.txt"; git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "side commit"
SIDE="$(git -C "$d" rev-parse HEAD)"
git -C "$d" checkout -q -
echo more > "$d/more.txt"; git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "main moves on"
# SETUP-GUARD: база ОБЯЗАНА существовать и ОБЯЗАНА не быть предком HEAD. Не выполнено —
# сценарий вырождается в «несуществующую базу», которую пиннит строка выше, и не различает
# ничего (`testing.md`, целостность гейта, св. 3).
if ! git -C "$d" rev-parse -q --verify "${SIDE}^{commit}" >/dev/null 2>&1; then
  bad "не-предковая база — SETUP НЕ СОСТОЯЛСЯ: коммит не создан"
elif git -C "$d" merge-base --is-ancestor "$SIDE" HEAD 2>/dev/null; then
  bad "не-предковая база — SETUP НЕ СОСТОЯЛСЯ: база оказалась предком HEAD"
else
  got="$( cd "$d" && EVENT_NAME=pull_request PR_BASE_SHA="$SIDE" \
          bash scripts/check_milestone_shape.sh >/dev/null 2>&1; echo $? )"
  [ "$got" = 1 ] && ok "существующая НЕ-предковая база → отказ (exit=1)" \
                 || bad "не-предковая база: ожидался 1, получен $got"
fi

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

  # Ожидаемый состав батареи. Меняешь состав — меняешь это число ТЕМ ЖЕ коммитом; сверка
  # ниже идёт на РАВЕНСТВО, поэтому расхождение видно и при добавлении, и при пропаже.
  BATTERY_EXPECTED=42
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
  # Якорь указывает на ЕДИНСТВЕННУЮ точку вызова в цикле по файлам. Прежний
  # (`visible_body "${file}"`) после фикса `M-72` в коде не встречался вовсе и попадал в
  # КОММЕНТАРИЙ: `cmp` видел различие, setup считался состоявшимся, поведение не менялось,
  # и проба честно доложила «зелёная против ослабления». Мутация обязана целить в код.
  make_stub fenceblind 's@body="$(visible_body "$f")"@body="$(git show "HEAD:$f" 2>/dev/null)"@' \
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

  # Ослабление 10 (`C-176` B-9) — сырой HTML-блок снова печатается как тело.
  make_stub htmlblind 's|if (index(lower, "</" htag ">") == 0) { html = 1 }|if (0) { html = 1 }|' \
    && try_stub "сырой HTML снова считается телом" "$STUB_PATH"
  # Ослабление 11 (`C-176` B-10) — снята граница титула: префикс снова проходит.
  make_stub titleprefix 's/\[ \\t\]\*#\*\[ \\t\]\*\$//g' \
    && try_stub "префикс титула снова принимается" "$STUB_PATH"
  # Ослабление 12 (`C-176` B-11) — допущен отступ: indented code снова считается заголовком.
  make_stub indentwiden 's|\^#{2,3} +|^[ ]*#{2,3} +|g' \
    && try_stub "отступленный заголовок снова принимается" "$STUB_PATH"

  # ── ПРОБЕЛЫ, НАЙДЕННЫЕ ПЕРЕБОРОМ ВСЕЙ БАТАРЕИ ПО КЛАССУ `C-177` ─────────────────
  # Ни одна из них не названа вердиктом: это результат применения правила ко ВСЕМУ набору,
  # а не к двум его экземплярам. Без них седьмой круг нашёл бы следующего члена группы.
  #
  # 13 — закрытие забора судится ТРЕМЯ конъюнктами (`C-175` B-8), мутации снимали второй
  # (`runlen`) и третий (`fencetail`); ПЕРВЫЙ — совпадение символа — не снимала ни одна,
  # хотя фикстура `spec_mismatched_fence` под него есть с круга 3.
  make_stub fencechar 's|c == fchar \&\& run >= flen \&\& tail_blank|run >= flen \&\& tail_blank|' \
    && try_stub "закрытие ДРУГИМ символом снова закрывает" "$STUB_PATH"
  # 14-15 — декомпозиция `fenceblind`: он снимал скрытие ЦЕЛИКОМ (фенс + комментарий + HTML)
  # одной подстановкой, то есть краснел от любого одного из трёх механизмов.
  make_stub commentblind 's|if (line ~ /<!--/) { if (line !~ /-->/) comment = 1; next }|if (0) { comment = 1; next }|' \
    && try_stub "скрытие снято ТОЛЬКО у HTML-комментария" "$STUB_PATH"
  make_stub fenceopenblind 's|if (is_marker) { fchar = c; flen = run; fence = 1; next }|if (0) { fchar = c; flen = run; fence = 1; next }|' \
    && try_stub "скрытие снято ТОЛЬКО у fenced-code" "$STUB_PATH"
  # 16 — обязательный пробел после решёток: фикстура была с круга 3, мутации не было вовсе.
  make_stub nospacewiden 's|#{2,3} +|#{2,3} *|g' \
    && try_stub "пробел после решёток перестал быть обязательным" "$STUB_PATH"

  # ── `M-72`: возврат ПАЙПА в `check_section` ────────────────────────────────────
  # Форма `visible_body "$file" | grep -q` под `set -o pipefail` возвращает 141 (SIGPIPE),
  # когда `grep` выходит по раннему совпадению, а производитель ещё пишет. Мутация возвращает
  # ровно ту форму, что стояла до фикса; ронять пробу обязана фикстура прод-масштаба, и
  # ДЕТЕРМИНИРОВАННО — заполнитель длиннее буфера трубы.
  make_stub pipeSIGPIPE 's@if grep -qiE "${re}" <<<"${body}"; then@if visible_body "${file}" | grep -qiE "${re}"; then@' \
    && try_stub "чтение тела вернулось в пайп (SIGPIPE при pipefail)" "$STUB_PATH"

  # ── ПЕР-ЧЛЕННЫЕ МУТАЦИИ — ядро фикса `C-177` ───────────────────────────────────
  # Групповая мутация выше пиннит МЕХАНИЗМ (скрытие/якорь как таковой) и краснеет, если
  # покрыт хоть один член группы. Ниже снимается свойство РОВНО У ОДНОГО члена, и проба
  # обязана падать от каждой ПООТДЕЛЬНО — иначе фикстура этого члена ничего не держит.
  # Мутации ПОРОЖДАЮТСЯ ИЗ ТЕХ ЖЕ ПЕРЕЧНЕЙ, что и фикстуры (`SECTIONS`/`HTML_TAGS`):
  # добавить пятый раздел и забыть мутацию к нему структурно невозможно.
  for t in "${HTML_TAGS[@]}"; do
    rest="$(printf '%s\n' "${HTML_TAGS[@]}" | grep -vx "$t" | paste -sd'|')"
    make_stub "html-$t" "s@(pre|script|style|textarea)@($rest)@g" \
      && try_stub "скрытие снято ТОЛЬКО у <$t>" "$STUB_PATH"
  done

  # ── `C-178` F-1: ТЕРМИНАТОРЫ открывающего тега — по мутации на КАЖДЫЙ ─────────────
  # Группа жила ВНУТРИ символьного класса `[ \t>]` плюс альтернация `…$`. Круг 6 разложил
  # теги и счёл её закрытой; она не выписана — значит и не запиннена. Теперь выписана.
  make_stub term-space 's@\[ \\t>\]@[\\t>]@' \
    && try_stub "терминатор снят ТОЛЬКО у «пробел»" "$STUB_PATH"
  make_stub term-tab   's@\[ \\t>\]@[ >]@' \
    && try_stub "терминатор снят ТОЛЬКО у «таб»" "$STUB_PATH"
  make_stub term-gt    's@\[ \\t>\]@[ \\t]@' \
    && try_stub "терминатор снят ТОЛЬКО у «>»" "$STUB_PATH"
  make_stub term-eol   's@ || match(lower, /<(pre|script|style|textarea)$/)@@' \
    && try_stub "терминатор снят ТОЛЬКО у «конец строки»" "$STUB_PATH"

  # ── `C-178` F-2: ЧЕТВЁРТЫЙ страж базы ────────────────────────────────────────────
  # Цепочка fail-closed стражей — тоже группа: пусто · zero-SHA · нет коммита · не-предок.
  # Первые три пиннились сценариями с круга 1; четвёртый не пиннился ничем, и барьер мог
  # молча начать судить диапазон с переписанной историей.
  make_stub noancestor 's@git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null@true@' \
    && try_stub "страж «база не предок HEAD» снят" "$STUB_PATH"

  for sec in "${SECTIONS[@]}"; do
    # Адрес сужает подстановку до ОДНОЙ строки `check_section` — той, что несёт это имя.
    addr="/check_section .*\"${sec}\"/"
    make_stub "title-$sec" "${addr}"'s/\[ \\t\]\*#\*\[ \\t\]\*\$//' \
      && try_stub "граница титула снята ТОЛЬКО у «$sec»" "$STUB_PATH"
    make_stub "col0-$sec"  "${addr}"'s/\^#{2,3} +/#{2,3} +/' \
      && try_stub "колонка 0 снята ТОЛЬКО у «$sec»" "$STUB_PATH"
    make_stub "depth-$sec" "${addr}"'s/#{2,3} +/#{2,4} +/' \
      && try_stub "глубина расширена ТОЛЬКО у «$sec»" "$STUB_PATH"
    make_stub "space-$sec" "${addr}"'s/#{2,3} +/#{2,3} */' \
      && try_stub "пробел после решёток снят ТОЛЬКО у «$sec»" "$STUB_PATH"
  done

  # ЛИТЕРАЛ ЖИВЁТ ОДИН РАЗ И СВЕРЯЕТСЯ НА РАВЕНСТВО, А НЕ НА «НЕ МЕНЬШЕ». `-lt` пропускал
  # РОСТ батареи молча — то есть новую мутацию, добавленную без обновления числа; а число
  # в шаге CI при этом продолжало называть старое (`C-177` N-2: «из четырёх» при двенадцати).
  echo "  батарея ослаблений: поймано ${BATTERY_OK} из ${BATTERY_N} (ожидалось ${BATTERY_EXPECTED})"
  if [ "$BATTERY_N" -ne "$BATTERY_EXPECTED" ]; then
    bad "батарея неполна: ${BATTERY_N} ослаблений вместо ${BATTERY_EXPECTED} — стаб не собрался либо литерал не обновлён"
  fi
fi

echo
echo "PASS=$PASS FAIL=$FAIL (сценариев: $((PASS + FAIL)))"
[ "$FAIL" -eq 0 ] && { echo "VERDICT: PASS"; exit 0; } || { echo "VERDICT: FAIL"; exit 1; }
