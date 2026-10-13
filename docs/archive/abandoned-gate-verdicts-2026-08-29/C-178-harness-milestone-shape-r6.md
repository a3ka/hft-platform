<!-- GATE-META
milestone: C-101
audited_repo: a3ka/hft-platform
audited_base: c4cfb8564fb5549060762c7056485065557afee0
audited_head: cda46001ad958a25082de0e47d10b21b878aaace
verdict: REJECT
-->

# C-178 — REJECT: harness adversary audit `harness-milestone-shape`, round 6

## Предмет и маршрут

- Маршрут: `docs/workflow/harness-track.md` §3/§5.3 — обязательный адверсарий со свежим
  контекстом; этот файл — merge-precondition.
- Ветка: `feat/harness-milestone-shape`; база
  `c4cfb8564fb5549060762c7056485065557afee0` (merge-base с `origin/main`), вершина
  `cda46001ad958a25082de0e47d10b21b878aaace`; предыдущая аудированная вершина (круг 5)
  `107e5b00246b6fdcb71360cf0480b81cca58aa31`.
- Артефакты: `scripts/check_milestone_shape.sh` · `scripts/tests/red_milestone_shape.sh` ·
  джобы `milestone-shape`/`status-check` в `.github/workflows/ci.yml`. Диапазон
  `c4cfb85..cda4600` содержит ровно эти три пути плюс вердикт-файлы `research/critiques/` —
  граница трека соблюдена.
- Номер: `bash scripts/reserve_artifact_id.sh C` → `C-178` (на этом предмете номера
  сталкивались дважды; взят резервом).
- Прочитаны с диска: все шесть правил, профиль, `harness-track.md` целиком,
  `oracle-blindness-class-2026-08-28.md` целиком, все пять прошлых вердиктов
  (`C-101` — из `docs/archive/abandoned-gate-verdicts-2026-08-24/`, `C-173`, `C-175`,
  `C-176`, `C-177`) — целиком, с ветки.

## Вердикт: REJECT

Круг 6 закрыл оба экземпляра `C-177` и разложил перечислимые группы, живущие в bash-массивах:
оба стаба круга 5 теперь роняют пробу, и падает именно свой сценарий. Но утверждение «класс
закрыт ЦЕЛИКОМ» неверно: правило «у каждого члена группы — своя фикстура и своя мутация»
применено только к группам, которые автор ВЫПИСАЛ в массивы (`SECTIONS`/`HTML_TAGS`) и к
конъюнктам закрытия забора. Группы, живущие внутри регекспа и внутри цепочки fail-closed
стражей, перебором не накрыты — и против двух узких live-derived ослаблений полная проба
остаётся зелёной. Это нарушение harness-track §5.1 (анти-плацебо) и §5.2 (мутационный
контроль), тот же класс, что `C-177`.

### F-1 — терминаторы открывающего HTML-тега: группа из четырёх членов, запиннен один

`scripts/check_milestone_shape.sh:170`:

```
if (match(lower, /<(pre|script|style|textarea)[ \t>]/) || match(lower, /<(pre|script|style|textarea)$/)) {
```

Открывающий тег признаётся четырьмя формами терминатора: пробел, таб, `>`, конец строки —
это и есть CommonMark-условие старта type-1 raw-HTML-блока, и живой барьер реализует его
верно. Но ВСЕ фикстуры пробы (`spec_section_in_raw_html`, `red_milestone_shape.sh:180-185`)
строят только форму `<tag>` — то есть упражняют один член группы (`>`). Мутации `htmlblind`
и `html-$t` снимают ТЕГИ, а не терминаторы; у ветки `$` (тег в конце строки) нет ни
фикстуры, ни мутации вовсе. Это тот же промах меры, что `oracle-blindness` §1 №4: величина
снимается с одного носителя из нескольких.

Два узких стаба, выведенных sed'ом из живого барьера (каждый меняет ровно строку 170):

1. **E1** — `[ \t>]` → `[>]`: `<script src="x.js">` перестаёт открывать блок.
2. **E2** — снята альтернация `|| match(lower, /<(pre|script|style|textarea)$/)`:
   `<script` в конце строки перестаёт открывать блок.

Оба внеслись (`cmp` exit=1), полная проба под `MSHAPE_SELFTEST=1` против КАЖДОГО —
`PASS=47 FAIL=0`, exit=0. При этом расходимость с живым барьером предъявлена на закоммиченных
фикстурах, где единственный `Allowed paths` спрятан в блоке, открытом `<script src="x.js">`
(соотв. `<script`+EOL): живой барьер — exit=1 по обеим, E1 — exit=0 на первой, E2 — exit=0
на второй. То есть будущий регресс любой из этих двух форм даст ложное зелёное ровно на
обещанном инварианте, и проба смолчит — как смолчала на `style` в круге 5.

**Воспроизведение (из worktree на `cda4600`):**

```
sed 's@(pre|script|style|textarea)\[ \\t>\]@(pre|script|style|textarea)[>]@' \
  scripts/check_milestone_shape.sh > /tmp/term-gt.sh
cmp -s /tmp/term-gt.sh scripts/check_milestone_shape.sh   # exit=1 — внеслась
MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=/tmp/term-gt.sh bash scripts/tests/red_milestone_shape.sh
# → PASS=47 FAIL=0, exit=0 (ожидался ненулевой)

sed 's@ || match(lower, /<(pre|script|style|textarea)\$/)@@' \
  scripts/check_milestone_shape.sh > /tmp/term-eol.sh
cmp -s /tmp/term-eol.sh scripts/check_milestone_shape.sh  # exit=1 — внеслась
MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=/tmp/term-eol.sh bash scripts/tests/red_milestone_shape.sh
# → PASS=47 FAIL=0, exit=0 (ожидался ненулевой)
```

**Условие снятия.** Фикстуры сырого HTML порождаются не только по перечню ТЕГОВ, но и по
перечню ТЕРМИНАТОРОВ открывающего тега (минимум: `<tag attr>`-форма с пробелом и `<tag>`+EOL;
таб — по решению автора, с объявлением в грамматике). Плюс две setup-guarded мутации,
снимающие по одному терминатору (E1- и E2-образные), от каждой из которых полная проба
обязана падать поотдельно — своим сценарием.

### F-2 — страж «база не предок HEAD» не запиннен ничем; его снятие проходит пробу зелёным

Цепочка fail-closed стражей базы (`scripts/check_milestone_shape.sh:130-140`) — группа из
четырёх: пустая база · zero-SHA · несуществующий коммит · не-предок
(`git merge-base --is-ancestor`). Сценарии пробы (`red_milestone_shape.sh:392-401`) пиннят
первые три; у четвёртого нет ни сценария, ни мутации.

Стаб, выведенный из живого барьера заменой `git merge-base --is-ancestor "${raw}" HEAD
2>/dev/null` → `true` (`cmp` exit=1): полная проба под `MSHAPE_SELFTEST=1` — `PASS=47
FAIL=0`, exit=0. Расходимость с живым барьером предъявлена исполнением: репозиторий, где
`PR_BASE_SHA` существует, но НЕ предок HEAD (ветвление, force-push-класс), а неполная спека
есть в обоих деревьях — живой барьер отказывает fail-closed (exit=1, «база НЕ предок HEAD»),
стаб печатает «новых milestone-спек нет» и возвращает 0. Заявленное поведение «что введено,
недоказуемо → отказ» существует только в живом коде и не держится пробой.

**Воспроизведение:**

```
sed 's|git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null|true|' \
  scripts/check_milestone_shape.sh > /tmp/no-anc.sh
cmp -s /tmp/no-anc.sh scripts/check_milestone_shape.sh    # exit=1
MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=/tmp/no-anc.sh bash scripts/tests/red_milestone_shape.sh
# → PASS=47 FAIL=0, exit=0 (ожидался ненулевой)
# расходимость: git init; c0 → c1(добавляет неполную M-99) → side от c1 (+коммит) → main +c2;
# EVENT_NAME=pull_request PR_BASE_SHA=<side> → live exit=1, стаб exit=0
```

**Условие снятия.** Сценарий «существующая, но не-предковая база → exit=1» (конструкция —
ветвление, как выше) и setup-guarded мутация, снимающая только этот страж; проба обязана
падать от неё своим сценарием.

### N-3 — устаревшие ссылки на строки барьера в пробе (не блокирует)

`red_milestone_shape.sh:82` называет `check_milestone_shape.sh:183-186` для перечня разделов
и `:87` называет `:160` для форм HTML. На `cda4600` фактически: `check_section`-вызовы —
строки 193-196, альтернация тегов — строка 170 (шапка круга 6 сдвинула файл). Правка —
вместе со снятием F-1/F-2 (`gates.md` §9-в, связность).

## Что подтверждено исполнением (мандат (а)–(з) + §9)

- **(а) Пер-членный пин работает для разложенных групп.** Три узких стаба из живого барьера
  (`cmp` exit=1 у каждого): снятие `style` из альтернации → `FAIL: <style> прячет раздел`
  (46/1); снятие границы титула только у `§Tasks` → `FAIL: §TasksNOT-A-SECTION` (46/1);
  снятие колонки 0 только у `Objective` → падают ровно Objective-сценарии (отступ и H4,
  45/2). Оба стаба `C-177` закрыты, падает свой сценарий, не соседний.
- **(б) Перебор дал две неразложенные группы** — F-1 (терминаторы) и F-2 (стражи базы).
  Дополнительно проверен маркер `~` отдельным стабом (`c == "\`"` без тильды): проба красная
  через сценарий `~~~-фенса` — запиннено фикстурой. Конъюнкты закрытия забора покрыты
  мутациями `fencechar`/`runlen`/`fencetail` (честная батарея ловит все три).
- **(в) Порождающий перечень — рост громкий, не молчаливый.** Пятый тег (`iframe`),
  добавленный в перечень пробы-копии и в барьер-копию: сценарий `<iframe> прячет раздел`
  появился и прошёл, а батарея упала — sed-якорь `html-$t` жёстко несёт четыре имени, все
  пять `html-*`-стабов дали «SETUP НЕ СОСТОЯЛСЯ», и литерал сработал: «батарея неполна: 32
  ослаблений вместо 36», VERDICT: FAIL. Рост ломается громко в обе стороны (тег только в
  барьере ломает те же якоря). Оговорка: якорь захардкожен, а не порождён из массива — но
  протухание якоря fail-closed, поэтому не находка.
- **(г) Литералы и числа в прозе.** `BATTERY_EXPECTED=36`, фактическая батарея 36/36 на
  равенство; честная проба `PASS=83 FAIL=0 (сценариев: 83)`; имя шага CI — «Проба барьера
  (83 сценария + батарея из 36 ослаблений)»; комментарий CI — «БАТАРЕЯ ВЫРОСЛА 12 → 36».
  Вхождения «12»/«из четырёх» в комментариях — исторические ссылки на `C-177` N-2, не
  утверждения о текущем составе. Расхождений нет.
- **(д) Setup-guard на порождаемых мутациях.** Барьер-копия с поведенчески-идентичной
  заменой `--diff-filter=AR`→`RA` (якорь `renameblind` уезжает, семантика та же): полный
  прогон — `FAIL: стаб «renameblind» — SETUP НЕ СОСТОЯЛСЯ` + «батарея неполна: 35 вместо
  36», VERDICT: FAIL. Несостоявшаяся подстановка — громкий FAIL, не пропуск, в том числе
  в циклах (проверено и на `html-iframe` в (в)).
- **(е) Строитель падает в безопасную сторону.** Живой код строителя (строки 68-101,
  исполнен sourced): промах подстановки (имя-опечатка) даёт ПОЛНУЮ спеку — 9 строк, 4
  заголовка (сценарий, ждущий 1, получит 0 и упадёт громко). Пустую спеку промах дать не
  может: sed не опустошает вход, а sed-ошибка недостижима на текущих четырёх именах
  (каждое: `sed_rc=0`, вывод 9 строк, искажение внесено). Остаточный предел: имя раздела
  с `|`/`\` сломало бы sed и дало пустой вход — при текущем перечне недостижимо; при
  расширении перечня стоит guard на непустоту, но это не дефект аудируемой ревизии.
- **(ж) Шапка круга 6 не съела ни один якорь.** Честная батарея 36/36 красная — мутация,
  попавшая только в комментарий, оставила бы пробу зелёной против себя и уронила бы
  батарею; этого нет, значит каждая из 36 подстановок меняет поведение. Пересечения якорей
  с новым текстом шапки (`#{2,3} +` в прозе про регексп) захватываются `/g` ВМЕСТЕ с кодом,
  а не вместо него. Грамматика в шапке соответствует коду: колонка 0, `#{2,3}` + пробел(ы) +
  точное имя + опциональный закрывающий ATX; скрытие — фенсы/комментарий/четыре формы HTML.
- **(з) Покрытие кругов 1-5 сохранено целиком.** В пробе присутствуют и зелены все семьи:
  фенс ```/~~~/закрытый-фенс-после, HTML-комментарий, проза-вместо-заголовка, rename
  (неполная→отказ, полная→принята), dirty-worktree против HEAD, unicode-имя, несовпадающий
  маркер, короткое закрытие, хвост после маркера, 4 raw-HTML формы, 4 префикса титула,
  отступы, H4 и `##без пробела` по каждому имени, fail-closed setup ×3. Прод-форма вызова:
  `EVENT_NAME=pull_request PR_BASE_SHA=<база> bash scripts/check_milestone_shape.sh` на
  `cda4600` → «новых milestone-спек нет», exit=0 (диапазон milestones/ не трогает — верно).
- **(§9-а) Дерево слияния.** `bash scripts/verify_design_claims.sh --merge-preview
  origin/main` → `VERDICT: PASS (0 нарушений)`, exit=0. `bash -n` обоих скриптов — exit=0;
  `git diff --check` диапазона — exit=0.
- **(§9-б) Полномочия.** Диф диапазона: только `.github/workflows/ci.yml`,
  `scripts/check_milestone_shape.sh`, `scripts/tests/red_milestone_shape.sh`,
  `research/critiques/C-17{3,5,6,7}-*.md` — зона харнесс-трека. Замок §11 (`.claude/**`,
  `CLAUDE.md`, `docs/04-workflow.md`) не тронут; граница C не затронута; FOUNDER-APPROVED
  не требуется.
- **(§9-в) Связность.** Цитируемые пути существуют (`docs/workflow/harness-track.md`,
  `docs/workflow/binding-requires-mechanism.md`); единственный дефект — устаревшие номера
  строк, N-3 выше.

## Done Block

```text
$ git diff --name-status c4cfb8564fb5549060762c7056485065557afee0 cda46001ad958a25082de0e47d10b21b878aaace
M	.github/workflows/ci.yml
A	research/critiques/C-173-harness-milestone-shape-r2.md
A	research/critiques/C-175-harness-milestone-shape-r3.md
A	research/critiques/C-176-harness-milestone-shape-r4.md
A	research/critiques/C-177-harness-milestone-shape-r5.md
A	scripts/check_milestone_shape.sh
A	scripts/tests/red_milestone_shape.sh
exit=0

$ bash scripts/tests/red_milestone_shape.sh          # честный прогон
  батарея ослаблений: поймано 36 из 36 (ожидалось 36)
PASS=83 FAIL=0 (сценариев: 83)
VERDICT: PASS
уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: 0
probe_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=c4cfb85... bash scripts/check_milestone_shape.sh   # прод-форма
OK: в диапазоне c4cfb85..HEAD новых milestone-спек нет — проверять нечего
barrier_exit=0

# (а) стабы C-177 теперь пойманы, падает СВОЙ сценарий
$ style-only stub: cmp_exit=1
  FAIL: <style> прячет раздел → отказ — ожидался exit=1, получен exit=0
PASS=46 FAIL=1 → VERDICT: FAIL, probe_exit=1
$ §Tasks-boundary stub: cmp_exit=1; line 195: '^#{2,3} +§?Tasks'
  FAIL: §TasksNOT-A-SECTION → отказ — ожидался exit=1, получен exit=0
PASS=46 FAIL=1 → VERDICT: FAIL, probe_exit=1
$ col0-Objective stub: cmp_exit=1
  FAIL: отступ 4 пробела у «Objective» → отказ ...
  FAIL: #### Objective (H4) → отказ ...
PASS=45 FAIL=2 → VERDICT: FAIL, probe_exit=1
$ tilde-blind stub (c == "`" без "~"): cmp_exit=1
  FAIL: раздел только в ~~~-фенсе → отказ ...
PASS=46 FAIL=1 → VERDICT: FAIL, probe_exit=1

# F-1: терминаторы открывающего тега — проба ЗЕЛЁНАЯ против обоих узких ослаблений
$ E1 ([ \t>]→[>]): cmp_exit=1; изменена ТОЛЬКО строка 170
$ MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=E1 bash scripts/tests/red_milestone_shape.sh
PASS=47 FAIL=0 (сценариев: 47) → VERDICT: PASS
E1_probe_exit=0        # ожидался ненулевой
$ E2 (снята альтернация ...$): cmp_exit=1; изменена ТОЛЬКО строка 170
$ MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=E2 bash scripts/tests/red_milestone_shape.sh
PASS=47 FAIL=0 (сценариев: 47) → VERDICT: PASS
E2_probe_exit=0        # ожидался ненулевой
$ расходимость live↔стаб на закоммиченных фикстурах:
live: fixture_script-attr exit=1  fixture_script-eol exit=1
E1:   fixture_script-attr exit=0  fixture_script-eol exit=1
E2:   fixture_script-attr exit=1  fixture_script-eol exit=0

# F-2: страж не-предковой базы
$ no-ancestor-guard stub (merge-base --is-ancestor → true): cmp_exit=1
$ MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=stub bash scripts/tests/red_milestone_shape.sh
PASS=47 FAIL=0 (сценариев: 47) → VERDICT: PASS
anc_probe_exit=0       # ожидался ненулевой
$ существующая не-предковая база (is_ancestor_exit=1), неполная спека в обоих деревьях:
live: FAIL база ... НЕ предок HEAD ... exit=1
stub: OK: в диапазоне 0b6e5ba..HEAD новых milestone-спек нет — проверять нечего; exit=0

# (в) пятый член перечня — громкое красное
$ HTML_TAGS+=(iframe) в пробе-копии + iframe в барьер-копии:
  PASS: <iframe> прячет раздел → отказ (exit=1)
  FAIL: стаб «html-iframe» — SETUP НЕ СОСТОЯЛСЯ ...
  батарея ослаблений: поймано 32 из 32 (ожидалось 36)
  FAIL: батарея неполна: 32 ослаблений вместо 36 ...
PASS=80 FAIL=6 → VERDICT: FAIL, probe5_exit=1

# (д) поведенчески-идентичный AR→RA (якорь renameblind уезжает)
  FAIL: стаб «renameblind» — SETUP НЕ СОСТОЯЛСЯ: подстановка ничего не изменила
  батарея ослаблений: поймано 35 из 35 (ожидалось 36)
  FAIL: батарея неполна: 35 ослаблений вместо 36 ...
PASS=82 FAIL=2 → VERDICT: FAIL, ra_probe_exit=1

# (е) строитель: промах → ПОЛНАЯ спека, не пустая
$ spec_with_broken_heading 'Typo-name' 'XXX' → 9 строк, 4 заголовка
$ по каждому имени: sed_rc=0, lines=9, distorted=1

# (§9-а/в) дерево слияния и синтаксис
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
merge_preview_exit=0
$ bash -n scripts/check_milestone_shape.sh scripts/tests/red_milestone_shape.sh; → bash_n_exit=0
$ git diff --check c4cfb85... cda4600...; → diff_check_exit=0
$ sed -n '193,196p' scripts/check_milestone_shape.sh   # check_section-вызовы (проба говорит 183-186)
$ sed -n '170p'      scripts/check_milestone_shape.sh   # альтернация тегов (проба говорит 160)

$ bash scripts/reserve_artifact_id.sh C
C-178
```
