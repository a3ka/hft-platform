#!/usr/bin/env bash
# Генератор эталона и мутантов для `red_artifact_ids.sh --battery` (M-61, спека §4.5).
# Эталон — ЧЕСТНАЯ реализация инварианта §4.1; каждый мутант отличается ровно одним
# свойством, названным осью и значением. Лежит в репозитории, а не в /tmp сессии:
# `A-005` §6.5 — сырые прогоны, снятые несуществующей пробой, четыре круга никто не заметил.
#
# ═══ РЕШЕНИЕ Б (founder, 2026-08-12) — что изменилось в самом эталоне ════════════════
# Барьер БОЛЬШЕ НЕ ВЫЧИСЛЯЕТ ПРЕДМЕТ артефакта. Идентификатор — `КЛАСС-НОМЕР` с
# необязательной БУКВОЙ (`M-60`, `M-60a`, `C-058b`), и он УНИКАЛЕН: два разных НОСИТЕЛЯ под
# одним идентификатором — нарушение, об одном они предмете или о разных. Следствия для
# эталона, все три предъявлены замером в `docs/plans/M-61-migration-to-decision-B.md`:
#   • `cls_num` + `slug_of` СЛИВАЮТСЯ в один разборщик `id_of` (место сравнения P2), а
#     `subject_from_body` умирает целиком вместе с чтением ТЕЛА файла;
#   • буква ВХОДИТ в ключ (`M-38a` ≠ `M-38b`), иначе барьер краснеет на каждом дроблении
#     (замер на реальном `c29102b`: 3 нарушения из 3);
#   • различитель буквы и одно-буквенного слага — ДЕФИС (`M-60a-docs-freeze` против
#     `M-90-a`): замер корпуса — 5 имён с буквенным суффиксом, 0 имён вида `КЛАСС-НОМЕР-<одна
#     буква>-…`. ЗАМЕР УТОЧНЯЕТ ПЛАН: fail-OPEN даёт не сам `-?([a-z])?`, а он ВМЕСТЕ с вольным
#     хвостом слага (`(.*)$` вместо `(-.*)?$`) — на корпусе 209 артефактов строгий хвост даёт
#     те же 7 множественных ключей, что эталон, а вольный теряет три семьи (`C-018`, `C-058`,
#     `M-46`) и оставляет 4. Поэтому хвост слага ОБЯЗАН начинаться с дефиса, а ДВЕ формы
#     дефекта разведены на ДВА мутанта: `dashoptional` (строгий хвост) пиннит половину ложного
#     КРАСНОГО (`L6ONELETTER`), `slugletter` (вольный хвост) — половину fail-OPEN, включая
#     прод-форму `M-46` (`B6DASH`). Замер приёмки: потерянных семей при вольном хвосте
#     ЧЕТЫРЕ — `C-18`, `C-58`, `M-46`, `R-1`.
#
# ═══ ЧЕТЫРЕ МЕСТА СРАВНЕНИЯ (план §5) — по одному на каждое есть мутант ══════════════
#   P1  путь → класс-носитель      `id_of` (карта каталогов)  → `dirblind`, `reviewsonly`
#   P2  basename → КЛАСС НОМЕР БУКВА `ID_RE`/`ID_SUB`         → `letterblind`, `dashoptional`,
#                                                                `slugletter`, `nameskip`,
#                                                                `letterarith`
#   P3  строка TECH-DEBT.md → номер  `TD_HOLD_RE`/`TD_PAT`    → `tdcanonical`, `tdanyline`,
#                                                                `tdregex`, `tdtrim`, `namesonly`
#   P4  сверка «ID совпал, НОСИТЕЛЬ различается»               → `letterexempt`, `subjectreader`,
#                                                                `revstrip`
#
# ЗАПИСЬ — ЧЕТВЁРКА `КЛАСС · НОМЕР · БУКВА · НОСИТЕЛЬ`, разделённая `SEP=$'\037'` (US).
# Разделитель выбран ЗАМЕРОМ, а не вкусом. Поле БУКВЫ пустое у 204 из 209 артефактов корпуса,
# поле НОСИТЕЛЯ у класса TD — строка с пробелами. Значит нужен разделитель, который (а) не
# встречается в путях и markdown-строках и (б) СОХРАНЯЕТ ПУСТОЕ ПОЛЕ при `read`. Табуляция
# условию (б) НЕ удовлетворяет: TAB — IFS-ПРОБЕЛЬНЫЙ символ, соседние табы схлопываются в один
# разделитель, и `read -r cls num lt car` кладёт ПУТЬ в переменную БУКВЫ. Замер первой редакции
# этого генератора: при TAB-разделителе эталон пропускал B1R/B1C/B1A/B1M/B6NOSLUG/B6DASH/B4REN/
# B5THIRD/B3ORIG — то есть был зелен на ВСЕХ безбуквенных коллизиях, а батарея этого не видела
# бы (мутанты красны, эталон «зелен»). `\037` не пробельный, схлопывания нет, пустое поле живёт.
set -uo pipefail
D="${1:?каталог назначения}"; mkdir -p "$D" || exit 1

# ─── общие части ────────────────────────────────────────────────────────────────────
# Разбор вынесен в ПЕРЕМЕННЫЕ (ID_RE/ID_SUB/TD_PAT/TD_HOLD_RE/TD_CANON_RE), а не зашит в
# тело функций: мутация регекспа тогда — замена ОДНОЙ строки целиком (`s|^ID_RE=.*|…|`), без
# экранирования метасимволов в ПАТТЕРНЕ sed'а. Круг 3 показал, чем кончается обратное:
# `${BODY//pat/repl}` со слэшами и кавычками молча не срабатывал, и мутант совпадал с эталоном.
read -r -d '' HEAD_COMMON <<'EOF'
#!/usr/bin/env bash
set -uo pipefail
ZERO=0000000000000000000000000000000000000000
SEP=$'\037'     # US — не IFS-пробельный, значит пустое поле БУКВЫ переживает `read`
refs_all() { git for-each-ref --format='%(refname)' refs/remotes/origin refs/heads 2>/dev/null; }

# ── P2: basename → «КЛАСС НОМЕР БУКВА» ──────────────────────────────────────────────
# Буква стоит ВПЛОТНУЮ к цифрам и входит в идентификатор; всё после первого дефиса — слаг,
# в суждение не входящий. Ведущие нули НЕ снимаются здесь: нормализация живёт в `10#` тела
# (та же дисциплина, что в next_artifact_id.sh), и именно она — предмет мутанта letterarith.
ID_RE='^([MRCA])-([0-9]+)([a-z])?(-.*)?$'
ID_SUB="\\1${SEP}\\2${SEP}\\3"
# ── P1: путь → класс-носитель. Файл, чей префикс не совпадает с классом СВОЕГО каталога
# (`research/reports/M-53-tester-report.md`), номер лишь УПОМИНАЕТ и носителем не является.
id_of() {
  case "$1" in
    milestones/M-*|research/reviews/R-*|research/reports/R-*|research/critiques/C-*|research/arbitration/A-*) :;;
    *) return 1;;
  esac
  basename "$1" .md | sed -nE "s|${ID_RE}|${ID_SUB}|p"
}

# ── P3: строка TECH-DEBT.md → носитель номера ───────────────────────────────────────
# АСИММЕТРИЯ НАМЕРЕННАЯ (план §3): ЗАНЯТОСТЬ широка (номер держит ЛЮБАЯ строка-запись во всех
# четырёх формах прод-корпуса — 124 строки на 126 номеров), ВВЕДЕНИЕ узко (только ЗАВОДЯЩАЯ
# карточка `- **TD-NNN** `слаг``). Широкая занятость убирает ложное ЗЕЛЁНОЕ — замер: 5 из 5
# неканонических номеров давали exit 0 на настоящем переиспользовании; узкое введение убирает
# ложное КРАСНОЕ на штатной форме close-out'а reviewer'а (реальный коммит `51e4023`).
# Регекспы занятости — для ДИНАМИЧЕСКОГО awk-разбора, поэтому `[*]`, а не `\*`: gawk на строке
# `\*` печатает warning и СНИМАЕТ экранирование, после чего `- **TD` читается как «дефис, ноль
# и более пробелов, звёздочка-квантор» — тихая порча регекспа ровно того класса, что стоила
# кругов 3 и 5. Скобочный класс однозначен во всех реализациях awk.
TD_HOLD_RE='^- [*][*]TD-[0-9]+'
TD_CANON_RE='^- [*][*]TD-0*[0-9]+[*][*] `[^`]+`'
TD_PAT='- \*\*TD-0*([0-9]+)\*\* `([^`]+)`.*'
# Носитель канонической карточки — её (номер, слаг); носитель прочих форм — сама строка.
# Мультиномерный пункт (`- **TD-049 / TD-050 / TD-051 — ✅ CLOSED**`) держит ВСЕ ТРИ номера:
# они берутся из ведущего жирного сегмента, а не из всей строки, — иначе перекрёстная ссылка
# внутри карточки («см. TD-103») читалась бы как держатель (72 из 126 номеров упомянуты >1 раза).
td_occupancy() {
  awk -v hold="$TD_HOLD_RE" -v canonre="$TD_CANON_RE" -v sep="$SEP" '
    $0 ~ hold {
      line = $0
      canon = (line ~ canonre)
      slug = line; sub(/^[^`]*`/, "", slug); sub(/`.*$/, "", slug)
      head = line; sub(/^- \*\*/, "", head)
      i = index(head, "**"); if (i > 0) head = substr(head, 1, i - 1)
      while (match(head, /TD-[0-9]+/)) {
        num = substr(head, RSTART + 3, RLENGTH - 3); sub(/^0+/, "", num); if (num == "") num = "0"
        car = canon ? num " " slug : line
        printf "TD%s%s%s%s%s\n", sep, num, sep, sep, car
        head = substr(head, RSTART + RLENGTH)
      }
    }'
}
EOF

# Мутант subjectreader несёт СВОЮ функцию — регресс к правилу 1 §3.1 (чтение шапки).
read -r -d '' SUBJ_READER <<'EOF'
# Регресс к удалённому правилу 1 §3.1: носителем идентичности снова объявлена шапка
# `**Предмет:**` / `**Контекст**`. Fallback — ПУТЬ (не слаг): иначе мутант ронял бы ещё и
# B7REV, то есть доказывал бы не ту дыру, ради которой построен.
subj_or_path() {  # $1 = rev:path, $2 = path
  local body hdr
  body="$(git show "$1" 2>/dev/null)" || { printf '%s\n' "$2"; return 0; }
  hdr="$(printf '%s\n' "$body" | awk '/^\*\*(Предмет:|Контекст)/{f=1} f{print} f&&/^$/{exit}' \
        | grep -oE '`[^`]+`' | head -1 | tr -d '`')"
  [ -n "$hdr" ] && { printf '%s\n' "$hdr"; return 0; }
  printf '%s\n' "$2"
}
EOF

# Тело барьера.
read -r -d '' BODY_CHECK <<'EOF'
case "${EVENT_NAME:-}" in
  push)         BASE="${PUSH_BEFORE:-}";;
  pull_request) BASE="${PR_BASE_SHA:-}";;
  *) exit 1;;
esac
[ -n "$BASE" ] || exit 1
[ "$BASE" != "$ZERO" ] || exit 1
git cat-file -e "$BASE" 2>/dev/null || exit 1
git merge-base --is-ancestor "$BASE" HEAD 2>/dev/null || exit 1

# Все носители объединения ref'ов: «КЛАСС \t НОМЕР \t БУКВА \t НОСИТЕЛЬ»
universe() {
  local ref f id
  for ref in $(refs_all); do
    while IFS= read -r -d '' f; do
      id="$(id_of "$f")" || continue
      [ -n "$id" ] || continue
      printf '%s%s%s\n' "$id" "$SEP" "$f"
    done < <(git ls-tree -r --name-only -z "$ref" 2>/dev/null)
  done
  # записи TECH-DEBT.md всех ref'ов
  for ref in $(refs_all); do
    git show "$ref:TECH-DEBT.md" 2>/dev/null | td_occupancy
  done
}
# Что ВВЕДЕНО диапазоном. «Введён» = появился в диапазоне И ПРИСУТСТВУЕТ В РЕЗУЛЬТАТЕ:
# инвариант §4.1 сформулирован ОТ РЕЗУЛЬТАТА, а покоммитный обход без сверки с HEAD судит
# промежуточные состояния. Цена конкретна: ветка, исправившая СВОЮ ЖЕ ошибку перенумерацией
# (реальный коммит `f0e915b`: R-038-M-60a → R-042), блокировалась, хотя в результате коллизии
# нет. Проверка `cat-file -e HEAD:$f` снимает ложный красный, НЕ ослабляя ни одного нарушения:
# и переименование В занятый номер, и коллизия в не-вершинном коммите оставляют файл в HEAD.
introduced() {
  local c f id head_td
  for c in $(git rev-list "$BASE..HEAD"); do
    while IFS= read -r -d '' f; do
      id="$(id_of "$f")" || continue
      [ -n "$id" ] || continue
      git cat-file -e "HEAD:$f" 2>/dev/null || continue
      printf '%s%s%s\n' "$id" "$SEP" "$f"
    done < <(git show --cc --name-only --no-renames --diff-filter=A -z --format= "$c" 2>/dev/null)
    # Новые ЗАВОДЯЩИЕ карточки TECH-DEBT.md этого коммита — и только те, что дожили до HEAD.
    #
    # СРАВНИВАЮТСЯ ДАННЫЕ, А НЕ ШАБЛОНЫ (Б-6, `R-054`). Прежняя редакция подставляла слаг
    # долга в ERE: `grep -qE "…\`${tsubj}\`"`. Слаг с метасимволом переставал совпадать САМ
    # С СОБОЙ, grep возвращал не-0, кандидат МОЛЧА выпадал — и барьер печатал «ни один
    # артефакт не введён», exit 0, на настоящей коллизии. Замер reviewer'а: 6 слагов из 111
    # в `origin/main` (5.4 %) уже несут метасимволы, а на реальном `TD-3` (`[verify-at-impl]`)
    # grep падает с «Invalid range end» — и падение игнорируется, потому что различались
    # только «нашёл / не нашёл». Направление отказа было выбрано ПРОТИВОПОЛОЖНО смыслу
    # барьера, вся ценность которого в fail-closed.
    #
    # Обе стороны проходят ОДИН И ТОТ ЖЕ разбор (`TD_PAT`), сверка — по точной строке (`-qxF`).
    # `IFS= read -r` сохраняет краевые пробелы: прежний `read -r tcls tnum tsubj` их срезал,
    # и строка, которая в файле ЕСТЬ, не находилась (третье воспроизведение Б-6).
    head_td="$(git show "HEAD:TECH-DEBT.md" 2>/dev/null | sed -nE "s|^${TD_PAT}$|\1 \2|p")"
    git show "$c" -- TECH-DEBT.md 2>/dev/null \
      | sed -nE "s|^\+${TD_PAT}$|\1 \2|p" \
      | while IFS= read -r td_line; do
          [ -n "$td_line" ] || continue
          printf '%s\n' "$head_td" | grep -qxF -- "$td_line" \
            && printf 'TD%s%s%s%s%s\n' "$SEP" "${td_line%% *}" "$SEP" "$SEP" "$td_line"
        done
  done
}
U="$(universe | sort -u)"
IN="$(introduced | sort -u)"
[ -z "$IN" ] && exit 0
# P4 — сверка идентичности БЕЗ разбора: ID совпал, а НОСИТЕЛЬ различается. Один и тот же
# файл, видимый в N ref'ах, даёт один и тот же носитель и нарушением не является.
while IFS="$SEP" read -r cls num lt car; do
  [ -n "${cls:-}" ] || continue
  num=$((10#${num}))
  while IFS="$SEP" read -r c2 n2 l2 car2; do
    [ -n "${c2:-}" ] || continue
    n2=$((10#${n2}))
    [ "$c2" = "$cls" ] && [ "$n2" = "$num" ] && [ "$l2" = "$lt" ] && [ "$car2" != "$car" ] && exit 1
  done <<< "$U"
done <<< "$IN"
exit 0
EOF

# Мутант showall: судит ВСЮ вселенную на дубли, а не введённое диапазоном (ось 5).
read -r -d '' BODY_SHOWALL <<'EOF'
case "${EVENT_NAME:-}" in push) BASE="${PUSH_BEFORE:-}";; pull_request) BASE="${PR_BASE_SHA:-}";; *) exit 1;; esac
[ -n "$BASE" ] || exit 1
[ "$BASE" != "$ZERO" ] || exit 1
git cat-file -e "$BASE" 2>/dev/null || exit 1
universe() {
  local ref f id
  for ref in $(refs_all); do
    while IFS= read -r -d '' f; do
      id="$(id_of "$f")" || continue
      [ -n "$id" ] || continue
      printf '%s%s%s\n' "$id" "$SEP" "$f"
    done < <(git ls-tree -r --name-only -z "$ref" 2>/dev/null)
  done
  for ref in $(refs_all); do
    git show "$ref:TECH-DEBT.md" 2>/dev/null | td_occupancy
  done
}
U="$(universe | sort -u)"
dup="$(printf '%s\n' "$U" | awk -F"$SEP" 'NF>=4 {k=$1"|"$2"|"$3
        if (k in seen && seen[k] != $4) bad[k]=1; seen[k]=$4} END {for (k in bad) print k}')"
[ -n "$dup" ] && exit 1
exit 0
EOF

# ── МУТАНТЫ ТЕЛА ────────────────────────────────────────────────────────────────────
# Мутант renameblind: детекция переименований ВКЛЮЧЕНА, поэтому увод показывается как R и
# фильтром A не ловится — слеп именно к переименованию, а не сломан целиком.
BODY_RENAMEBLIND="${BODY_CHECK//--no-renames /}"
# Мутант touchcounts: правка (M) считается введением носителя (ось 4 / правка существующего).
BODY_TOUCHCOUNTS="${BODY_CHECK//--diff-filter=A /--diff-filter=AM }"

# Мутанты строятся SED'ом, а не подстановкой ${//} (правка круга 3). Причина не
# стилистическая: `${BODY//pat/repl}` с кавычками и слэшами внутри шаблона молча НЕ
# СРАБАТЫВАЕТ (originonly дал 0 изменений и совпал с эталоном) либо портит тело (absolute
# вклеивал остаток шаблона в текст скрипта). Оба отказа тихие — ровно тот класс, из-за
# которого понадобился круг 3. Страж «мутант обязан отличаться» ниже ловит несработавшую
# подстановку немедленно.
mutate()      { printf '%s\n' "${BODY_CHECK}"  | sed "$1"; }
mutate_head() { printf '%s\n' "${HEAD_COMMON}" | sed "$1"; }
# Страж подстановки — ДВУСТОРОННИЙ. Мало проверить «результат отличается»: sed, упавший на
# собственном скрипте (например `||` внутри выражения с делимитером `|`), печатает ошибку в
# stderr и отдаёт ПУСТО, а пустое тело «отличается от оригинала» и стража первой редакции
# проходило. Мутант при этом вырождается в пустой файл, красный на ВСЁМ. Поэтому проверяются
# оба края: непусто И отличается.
guard() {  # $1=имя $2=результат $3=оригинал
  [ -n "$2" ] || { echo "мутант $1: подстановка ОТКАЗАЛА (пустой результат)" >&2; exit 1; }
  [ "$2" != "$3" ] || { echo "мутант $1: подстановка НЕ СРАБОТАЛА (совпал с оригиналом)" >&2; exit 1; }
}

# quotedname — Б-4 (R-052): мутант обязан отличаться от эталона ТОЛЬКО КАНАЛОМ ЧТЕНИЯ ИМЁН.
# Прежняя сплошная подстановка срезала `-z` и у shell-тестов (`[ -z "$IN" ]` → `[ "$IN" ]`),
# из-за чего мутант пропускал ЛЮБУЮ коллизию и краснел на всех 12 сценариях — то есть был
# сломан целиком, а не слеп к квотированию. Трогаются ровно три места: два git-вызова теряют
# `-z` (git начинает КВОТИРОВАТЬ не-ASCII имена), читатель переходит на построчный.
BODY_QUOTEDNAME="$(mutate 's|--name-only -z |--name-only |; s|--diff-filter=A -z |--diff-filter=A |; s|read -r -d '"'"''"'"' f|read -r f|')"
guard quotedname "$BODY_QUOTEDNAME" "$BODY_CHECK"
# namesonly — записи в TECH-DEBT.md не читаются вовсе (ось 2)
BODY_NAMESONLY="$(mutate 's|:TECH-DEBT\.md|:TECH-DEBT.НЕТ.md|')"
guard namesonly "$BODY_NAMESONLY" "$BODY_CHECK"
# absolute — судит всю историю вместо диапазона (ось 5)
BODY_ABSOLUTE="$(mutate 's|git rev-list "\$BASE\.\.HEAD"|git rev-list HEAD|')"
guard absolute "$BODY_ABSOLUTE" "$BODY_CHECK"
# rangeblind — смотрит только вершину диапазона, а не каждый коммит (ось 5)
BODY_RANGEBLIND="$(mutate 's|git rev-list "\$BASE\.\.HEAD"|git rev-list -n 1 "$BASE..HEAD"|')"
guard rangeblind "$BODY_RANGEBLIND" "$BODY_CHECK"
# tdregex — ДАННЫЕ слага интерпретируются как ШАБЛОН (Б-6, `R-054`). Это дословно прежняя
# редакция: одна замена `-qxF` на `-qE` возвращает fail-open, при котором барьер печатал
# «OK» на настоящей коллизии.
BODY_TDREGEX="$(mutate 's|grep -qxF -- "\$td_line"|grep -qE -- "$td_line"|')"
guard tdregex "$BODY_TDREGEX" "$BODY_CHECK"
# tdtrim — краевой пробел слага срезается чтением (третье воспроизведение Б-6): дефект ДРУГОЙ,
# поэтому и мутант отдельный — иначе один kill-set покрывал бы два разных класса.
BODY_TDTRIM="$(mutate 's|while IFS= read -r td_line|while read -r td_line|')"
guard tdtrim "$BODY_TDTRIM" "$BODY_CHECK"

# letterexempt — БУКВА объявлена амнистией: носитель с буквой в суждение не входит вовсе.
# Это ровно та лазейка, в которую вырождается норма «продолжение берёт букву», если её никто
# не стережёт: `M-85b-alpha` и `M-85b-beta` — два РАЗНЫХ носителя под ОДНИМ идентификатором.
read -r -d '' SED_LETTEREXEMPT <<'EOF'
s@\[ -n "\${cls:-}" \] || continue@[ -n "${cls:-}" ] || continue; case "${lt}" in [a-z]) continue;; esac@
EOF
BODY_LETTEREXEMPT="$(mutate "$SED_LETTEREXEMPT")"
guard letterexempt "$BODY_LETTEREXEMPT" "$BODY_CHECK"
# letterarith — БУКВА затягивается в арифметику номера. ЗАМЕР сильнее плана: `10#85b` при
# `set -uo pipefail` без `-e` не просто теряет нормализацию нулей — ошибка РАЗВОРАЧИВАНИЯ
# обрывает ВЕСЬ цикл сравнения, и барьер печатает exit=0 на ЛЮБОМ буквенном идентификаторе
# (предъявлено: `M-85b-alpha` + `M-85b-beta` ⇒ exit=0 при сообщении «10#85b: value too great
# for base»). Поэтому kill-set — `B6LETTERDUP B6ZERO`, а не один `B6ZERO`, и он совпадает с
# kill-set'ом `letterexempt`. Мутируются ОБЕ стороны сверки: односторонняя мутация невидима,
# потому что ведущий ноль лежит на той стороне, где его снимают (замер: правка только `num=`
# оставляет B6ZERO зелёным).
BODY_LETTERARITH="$(mutate 's|num=\$((10#\${num}))|num=$((10#${num}${lt}))|; s|n2=\$((10#\${n2}))|n2=$((10#${n2}${l2}))|')"
guard letterarith "$BODY_LETTERARITH" "$BODY_CHECK"

# revstrip — регресс к удалённому правилу 4 §3.1: `-rev<N>`/`-addendum` считаются «той же
# вещью», то есть ОДНИМ носителем. Под Б ревизия обязана брать свой идентификатор.
read -r -d '' SED_REVSTRIP <<'EOF'
s|printf '%s%s%s\\n' "$id" "$SEP" "$f"|c="${f%.md}"; c="${c%-rev*}"; c="${c%-addendum*}"; printf '%s%s%s\\n' "$id" "$SEP" "$c"|
EOF
BODY_REVSTRIP="$(mutate "$SED_REVSTRIP")"
guard revstrip "$BODY_REVSTRIP" "$BODY_CHECK"

# subjectreader — регресс к удалённому правилу 1 §3.1: совпавшая шапка «Предмет»/«Контекст»
# ПРОЩАЕТ дубликат идентификатора. Две точки эмиссии мутируются ПО-РАЗНОМУ (ref у вселенной,
# HEAD у введённого), поэтому адресуются диапазонами строк, а не одной подстановкой.
read -r -d '' SED_SUBJECTREADER <<'EOF'
/^universe()/,/^}/ s|printf '%s%s%s\\n' "$id" "$SEP" "$f"|printf '%s%s%s\\n' "$id" "$SEP" "$(subj_or_path "$ref:$f" "$f")"|
/^introduced()/,/^}/ s|printf '%s%s%s\\n' "$id" "$SEP" "$f"|printf '%s%s%s\\n' "$id" "$SEP" "$(subj_or_path "HEAD:$f" "$f")"|
EOF
BODY_SUBJECTREADER="$(mutate "$SED_SUBJECTREADER")"
guard subjectreader "$BODY_SUBJECTREADER" "$BODY_CHECK"

# ── МУТАНТЫ ШАПКИ (P1/P2/P3 живут в HEAD_COMMON) ────────────────────────────────────
# originonly мутирует ШАПКУ АЛЛОКАТОРА: `refs_all()` объявлена в HEAD_COMMON, а не в теле.
HEAD_ORIGINONLY="$(mutate_head 's|refs/remotes/origin refs/heads|refs/remotes/origin|')"
guard originonly "$HEAD_ORIGINONLY" "$HEAD_COMMON"
# headsonly — ЗЕРКАЛО originonly: из перечисления выпал `refs/remotes/origin`. Это корневой
# дефект §1 («номер, свободный локально, занят в соседней ветке»), и до круга 4 его не пиннил
# ни один мутант: ось 3 была покрыта только в сторону refs/heads.
HEAD_HEADSONLY="$(mutate_head 's|refs/remotes/origin refs/heads|refs/heads|')"
guard headsonly "$HEAD_HEADSONLY" "$HEAD_COMMON"

# letterblind — БУКВА выброшена из идентификатора: сегодняшний прод-разбор `cls_num`, у
# которого `M-38a` и `M-38b` дают один ключ. Под Б это ложное КРАСНОЕ на каждом дроблении.
read -r -d '' SED_LETTERBLIND <<'EOF'
s|^ID_RE=.*|ID_RE='^([MRCA])-([0-9]+)[a-z]?(-.*)?$'|
s|^ID_SUB=.*|ID_SUB="\\\\1${SEP}\\\\2${SEP}"|
EOF
HEAD_LETTERBLIND="$(mutate_head "$SED_LETTERBLIND")"
guard letterblind "$HEAD_LETTERBLIND" "$HEAD_COMMON"
# dashoptional — «естественная» починка letterblind: дефис перед буквой объявлен
# необязательным. Различитель буквы и одно-буквенного слага исчезает, и `M-90-a` (слаг «a»)
# становится тем же идентификатором, что `M-90a-thing`. ЗАМЕР: при СТРОГОМ хвосте слага
# (`(-.*)?$`) многобуквенные слаги не затрагиваются (`M-46-order-flow-indicators` разбирается
# так же, как эталоном), поэтому kill-set мутанта — ровно `L6ONELETTER`, а не пара с `B6DASH`,
# как предсказывал план. `B6DASH` остаётся сценарием без мутанта (предел §4.5 п.3): парсер,
# который его роняет, ломает заодно `B1R`/`B1C`/`B1A`/`B1M`/`B5THIRD`/`B4REN`/`B3ORIG` и
# доказывает не ту дыру.
read -r -d '' SED_DASHOPTIONAL <<'EOF'
s|^ID_RE=.*|ID_RE='^([MRCA])-([0-9]+)-?([a-z])?(-.*)?$'|
EOF
HEAD_DASHOPTIONAL="$(mutate_head "$SED_DASHOPTIONAL")"
guard dashoptional "$HEAD_DASHOPTIONAL" "$HEAD_COMMON"
# slugletter — ВТОРАЯ форма того же дефекта и ЕДИНСТВЕННАЯ, дающая fail-OPEN. Хвост слага
# перестаёт быть обязан начинаться с дефиса (`.*$` вместо `(-.*)?$`), и ПЕРВАЯ БУКВА ЛЮБОГО
# слага читается как буква идентификатора: `M-46-order-flow-indicators` → `M-46o`,
# `M-46-read-path-probe` → `M-46r` — та самая множественность, ради которой заведён M-61,
# становится ЗЕЛЁНОЙ. Замер приёмки на корпусе 209 имён: теряются 4 семьи из 7 (`C-18`,
# `C-58`, `M-46`, `R-1`); у `dashoptional` (строгий хвост) — НИ ОДНОЙ, там дефект ровно
# обратный (ложное КРАСНОЕ на `M-90-a`). Мутант НЕ точечен по построению и это названо в
# §4.5: правило разбора имени общее для ВСЕХ носителей, поэтому его порча видна каждому
# сценарию, где два носителя различаются слагом (замерено 15). Kill-set объявлен ПОЛНОСТЬЮ —
# страж требует равенства множеств, а не «хоть как-то красен».
read -r -d '' SED_SLUGLETTER <<'EOF'
s|^ID_RE=.*|ID_RE='^([MRCA])-([0-9]+)-?([a-z])?.*$'|
EOF
HEAD_SLUGLETTER="$(mutate_head "$SED_SLUGLETTER")"
guard slugletter "$HEAD_SLUGLETTER" "$HEAD_COMMON"
# nameskip — занятость ТРЕБУЕТ слага, и носитель без слага (`C-710.md`) МОЛЧА выпадает из
# подсчёта вместо участия в сравнении. Наследник класса `slugskip` (`A-006` §2.3): «шаг, не
# сумевший выполнить работу, молчит, а агрегат отчитывается успехом».
read -r -d '' SED_NAMESKIP <<'EOF'
s|^ID_RE=.*|ID_RE='^([MRCA])-([0-9]+)([a-z])?(-.+)$'|
EOF
HEAD_NAMESKIP="$(mutate_head "$SED_NAMESKIP")"
guard nameskip "$HEAD_NAMESKIP" "$HEAD_COMMON"
# dirblind — карта каталогов забыта, класс берётся из префикса ИМЕНИ: отчёт тестера
# `research/reports/M-53-tester-report.md` начинает ДЕРЖАТЬ номер M-53 вместо того, чтобы его
# упоминать. В корпусе таких файлов восемь, и все выданы штатным воркфлоу (`M-53` §Allowed paths).
read -r -d '' SED_DIRBLIND <<'EOF'
s|^ *milestones/M-.*$|    *) :;;|
EOF
HEAD_DIRBLIND="$(mutate_head "$SED_DIRBLIND")"
guard dirblind "$HEAD_DIRBLIND" "$HEAD_COMMON"
# reviewsonly — зеркало dirblind: из карты выпал ВТОРОЙ реестр класса R (`gates.md` §6,
# backtest-отчёты `research/reports/R-NNN`). Номер, занятый вердиктом, тихо переиспользуется отчётом.
read -r -d '' SED_REVIEWSONLY <<'EOF'
s|research/reports/R-\*[|]||
EOF
HEAD_REVIEWSONLY="$(mutate_head "$SED_REVIEWSONLY")"
guard reviewsonly "$HEAD_REVIEWSONLY" "$HEAD_COMMON"
# tdcanonical — ЗАНЯТОСТЬ сужена до канонической карточки: 15 номеров из 126 в прод-корпусе
# становятся «свободными», и повторное использование TD-007/TD-103 проходит affirmative-«OK».
read -r -d '' SED_TDCANONICAL <<'EOF'
s|^TD_HOLD_RE=.*|TD_HOLD_RE='^- [*][*]TD-0*[0-9]+[*][*] `[^`]+`'|
EOF
HEAD_TDCANONICAL="$(mutate_head "$SED_TDCANONICAL")"
guard tdcanonical "$HEAD_TDCANONICAL" "$HEAD_COMMON"
# tdanyline — ВВЕДЕНИЕ расширено до любой строки-записи: штатная close-out-строка reviewer'а
# (`- **TD-061** ✅ **CLOSED …** — см. секцию CLOSED.`, реальный коммит `51e4023`) читается как
# заведение второго долга под занятым номером. Ложное КРАСНОЕ на форме, применяемой каждый merge.
read -r -d '' SED_TDANYLINE <<'EOF'
s|^TD_PAT=.*|TD_PAT='- \\*\\*TD-0*([0-9]+)\\*\\*(.*)'|
EOF
HEAD_TDANYLINE="$(mutate_head "$SED_TDANYLINE")"
guard tdanyline "$HEAD_TDANYLINE" "$HEAD_COMMON"

# ─── аллокаторы ─────────────────────────────────────────────────────────────────────
read -r -d '' NEXT_REF <<'EOF'
CLS="${1:?класс}"
# origin сконфигурирован, но ни одного его ref'а нет ⇒ занятость перечислить невозможно.
if git remote get-url origin >/dev/null 2>&1; then
  [ -n "$(git for-each-ref --format='%(refname)' refs/remotes/origin 2>/dev/null)" ] || exit 1
fi
max=0
for ref in $(refs_all); do
  case "$CLS" in
    TD) n="$(git show "$ref:TECH-DEBT.md" 2>/dev/null | grep -oE 'TD-[0-9]+' | grep -oE '[0-9]+')";;
    *)  n="$(git ls-tree -r --name-only "$ref" 2>/dev/null | grep -oE "(^|/)${CLS}-[0-9]+" | grep -oE '[0-9]+')";;
  esac
  for x in $n; do x=$((10#$x)); [ "$x" -gt "$max" ] && max=$x; done
done
[ "$max" -eq 0 ] && exit 1
case "$CLS" in M) printf 'M-%02d\n' $((max+1));; *) printf '%s-%03d\n' "$CLS" $((max+1));; esac
EOF
read -r -d '' NEXT_LOCALMAX <<'EOF'
CLS="${1:?класс}"
max=0
case "$CLS" in
  TD) n="$(grep -oE 'TD-[0-9]+' TECH-DEBT.md 2>/dev/null | grep -oE '[0-9]+')";;
  *)  n="$(git ls-tree -r --name-only HEAD 2>/dev/null | grep -oE "(^|/)${CLS}-[0-9]+" | grep -oE '[0-9]+')";;
esac
for x in $n; do x=$((10#$x)); [ "$x" -gt "$max" ] && max=$x; done
[ "$max" -eq 0 ] && exit 1
case "$CLS" in M) printf 'M-%02d\n' $((max+1));; *) printf '%s-%03d\n' "$CLS" $((max+1));; esac
EOF

# ─── сборка ─────────────────────────────────────────────────────────────────────────
emit() {  # $1=имя файла $2=шапка $3=вставка (может быть пустой) $4=тело
  printf '%s\n%s\n%s\n' "$2" "$3" "$4" > "$D/$1"; bash -n "$D/$1" || exit 1
}
# Б-5 (R-052): пять мутантов были ОБЪЯВЛЕНЫ в §4.5 и не построены; батарея пропускала их
# молча и печатала PASS по знаменателю исполненного. Поэтому список ниже — исчерпывающий, а
# сверку «§4.5 ⇄ построенное» делает сама батарея, в обе стороны.

# ── эталон и мутанты БАРЬЕРА ────────────────────────────────────────────────────────
emit ref-check.sh           "$HEAD_COMMON"       ""              "$BODY_CHECK"
emit showall-check.sh       "$HEAD_COMMON"       ""              "$BODY_SHOWALL"
emit renameblind-check.sh   "$HEAD_COMMON"       ""              "$BODY_RENAMEBLIND"
emit quotedname-check.sh    "$HEAD_COMMON"       ""              "$BODY_QUOTEDNAME"
emit touchcounts-check.sh   "$HEAD_COMMON"       ""              "$BODY_TOUCHCOUNTS"
emit namesonly-check.sh     "$HEAD_COMMON"       ""              "$BODY_NAMESONLY"
emit absolute-check.sh      "$HEAD_COMMON"       ""              "$BODY_ABSOLUTE"
emit rangeblind-check.sh    "$HEAD_COMMON"       ""              "$BODY_RANGEBLIND"
emit tdregex-check.sh       "$HEAD_COMMON"       ""              "$BODY_TDREGEX"
emit tdtrim-check.sh        "$HEAD_COMMON"       ""              "$BODY_TDTRIM"
emit letterexempt-check.sh  "$HEAD_COMMON"       ""              "$BODY_LETTEREXEMPT"
emit letterarith-check.sh   "$HEAD_COMMON"       ""              "$BODY_LETTERARITH"
emit revstrip-check.sh      "$HEAD_COMMON"       ""              "$BODY_REVSTRIP"
emit subjectreader-check.sh "$HEAD_COMMON"       "$SUBJ_READER"  "$BODY_SUBJECTREADER"
emit letterblind-check.sh   "$HEAD_LETTERBLIND"  ""              "$BODY_CHECK"
emit dashoptional-check.sh  "$HEAD_DASHOPTIONAL" ""              "$BODY_CHECK"
emit slugletter-check.sh    "$HEAD_SLUGLETTER"   ""              "$BODY_CHECK"
emit nameskip-check.sh      "$HEAD_NAMESKIP"     ""              "$BODY_CHECK"
emit dirblind-check.sh      "$HEAD_DIRBLIND"     ""              "$BODY_CHECK"
emit reviewsonly-check.sh   "$HEAD_REVIEWSONLY"  ""              "$BODY_CHECK"
emit tdcanonical-check.sh   "$HEAD_TDCANONICAL"  ""              "$BODY_CHECK"
emit tdanyline-check.sh     "$HEAD_TDANYLINE"    ""              "$BODY_CHECK"
# bheadsonly — то же, что headsonly, но у БАРЬЕРА: перечисление ref'ов теряет origin.
emit bheadsonly-check.sh    "$HEAD_HEADSONLY"    ""              "$BODY_CHECK"
# localmax / originonly / headsonly — мутанты АЛЛОКАТОРА (ось 3): барьер у них эталонный.
# A-006 §2.3 признал точечный профиль originonly структурно недостижимым — это верно ТОЛЬКО
# для мутанта, построенного на БАРЬЕРЕ: `universe()` фикстур не несёт origin-ref'ов, поэтому
# барьер без `refs/heads` слеп ко ВСЕМУ и краснеет на всех блокирующих сценариях. Тот же
# дефект, внесённый в АЛЛОКАТОР, точечен (замер круга 4).
emit localmax-check.sh      "$HEAD_COMMON"       ""              "$BODY_CHECK"
emit originonly-check.sh    "$HEAD_COMMON"       ""              "$BODY_CHECK"
emit headsonly-check.sh     "$HEAD_COMMON"       ""              "$BODY_CHECK"

# ── аллокаторы. Список ВЫВОДИТСЯ из построенных барьеров, а не дублируется руками:
# прежний захардкоженный перечень — то же ручное соответствие, что дрейфовало везде (A-006 §2.5).
for f in "$D"/*-check.sh; do
  v="$(basename "$f" -check.sh)"
  printf '%s\n%s\n' "$HEAD_COMMON" "$NEXT_REF" > "$D/$v-next.sh"; bash -n "$D/$v-next.sh" || exit 1
done
printf '%s\n%s\n' "$HEAD_COMMON"     "$NEXT_LOCALMAX" > "$D/localmax-next.sh";   bash -n "$D/localmax-next.sh"   || exit 1
printf '%s\n%s\n' "$HEAD_ORIGINONLY" "$NEXT_REF"      > "$D/originonly-next.sh"; bash -n "$D/originonly-next.sh" || exit 1
printf '%s\n%s\n' "$HEAD_HEADSONLY"  "$NEXT_REF"      > "$D/headsonly-next.sh";  bash -n "$D/headsonly-next.sh"  || exit 1

# Страж генератора: мутант, совпавший с эталоном, тестировал бы эталон под чужим именем —
# ровно то, чем оборачивается ТИХО НЕ СРАБОТАВШАЯ подстановка. Перебираются ВСЕ построенные:
# прежний список был захардкожен (7 имён при 13 мутантах) и сам являлся ручным соответствием,
# то есть стражем, не стерегущим половину состава (A-006 §2.5 — механизируй, а не сверяй глазом).
built=0
for f in "$D"/*-check.sh; do
  m="$(basename "$f" -check.sh)"
  [ "$m" = ref ] && continue
  built=$((built + 1))
  if cmp -s "$D/ref-check.sh" "$D/$m-check.sh" && cmp -s "$D/ref-next.sh" "$D/$m-next.sh"; then
    echo "мутант $m НЕ ПОСТРОЕН — совпал с эталоном И по барьеру, И по аллокатору" >&2; exit 1
  fi
done
echo "эталон и $built мутантов собраны в $D"
