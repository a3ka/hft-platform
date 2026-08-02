# R-021 — `docs/ct-rfc-06-l2delta` @ `7c42e97` — PR-гейт, круг 2 (DOC класс A)

**Date (UTC):** 2026-08-02
**Agent:** reviewer
**Branch:** `origin/docs/ct-rfc-06-l2delta` @ `7c42e97`
**Merge-цель:** `origin/main` @ `db219d6`
**Worktree:** `/tmp/hft-rev-rfc06v2` (detached, свой — `branch-hygiene.md` §1; общий чекаут
на чужой `docs/06-volume-truth` НЕ трогал)
**Класс гейта:** DOC класс A (`gates.md` §9) — `docs/rfc/**`.
**Предыдущие гейты:** `C-045` PASS (critic) → `R-019` CHANGES REQUESTED (reviewer, круг 1).

## Вердикт: **APPROVED — merge в `main`**

Все пять нарушений `R-019` закрыты, и закрыты **по существу, а не косметикой** — проверено
моим прогоном гейта на MERGE-ЦЕЛИ (не на ветке) и моими собственными грепами. Содержательная
часть документа проверена в круге 1 дважды независимо (`C-045` + `R-019` ЧАСТЬ 1) и правками
не затронута — повторный critic не требовался и не запрашивался.

Отдельно проверено и **ПОДТВЕРЖДЕНО** встречное утверждение architect'а: заявление
`docs/NEXT-SESSION-PROMPT.md:149` о том, что карта в замере «НЕПОЛНА (три match вместо
пяти)», — **само ложно**. Замер называет ровно ПЯТЬ мест, и это тот же список, что дал мой
греп в `R-019` §1.2. Подробности — §2 ниже; заведён долг на исправление документа-источника.

**STATUS документа НЕ переведён в ACTIVE и не может быть переведён этим гейтом** — см. N1.

---

## Block-scope — ЧИСТО

Дифф **docs/research-only**, только добавления. Ни `crates/`, ни `contracts/`, ни `*/tests/`,
ни `scripts/`, ни `milestones/`. Авторство корректное (architect — документ и пруфы, critic —
свой вердикт, reviewer — свой).

```
$ git diff --stat origin/main...HEAD
 docs/rfc/CT-RFC-06-l2delta.md                 | 421 ++++++++++++++++++++++++++
 research/critiques/C-045-ct-rfc-06-l2delta.md | 182 +++++++++++
 research/measurements/m-45-l2delta-impact.md  | 380 +++++++++++++++++++++++
 research/measurements/td-053-event-size.md    | 298 ++++++++++++++++++
 research/reviews/R-019-ct-rfc-06-l2delta.md   | 306 +++++++++++++++++++
 5 files changed, 1587 insertions(+)

$ git log --format='%h %an <%ae> %s' origin/main..HEAD
7c42e97 architect  docs(CT-RFC-06): R-019 F3-F6 — путь docs/07 развёрнут; названы остаточные классы...
df03366 architect  merge(CT-RFC-06): пруф-якорь m-45-l2delta-impact в дерево RFC — R-019 F1
87181b4 architect  merge(CT-RFC-06): пруф-якорь td-053-event-size в дерево RFC — R-019 F2
22715b7 reviewer   docs(reviewer): R-019 — CT-RFC-06 CHANGES REQUESTED ...
2852fae critic     docs(critic): C-045 — CT-RFC-06 L2Delta PASS ...
cdd2dd6 architect  docs(CT-RFC-06): §6-§9 ...
bcc071c architect  docs(CT-RFC-06): §3-§5 ...
3ff4355 architect  docs(CT-RFC-06): §0-§2 ...
49fc8c3 architect  docs(CT-RFC-06): скелет contract-RFC L2Delta ...
6122fce architect  research(M-45): карта влияния L2Delta ...
3efaabc architect  research(TD-053): замер размеров событий ...
```

Оба merge-коммита F1/F2 несут РОВНО по одному файлу (`git show --stat`: 298 и 380 строк) —
приземление пруфа, без попутного багажа. Коммиты атомарны, каждый ссылается на находку R-019.

## Block-risk — RISK-BLOCK корректно НЕ применён (перепроверено)

Дифф не трогает ни строки кода и ни байта `crates/contracts/**` — триггер `gates.md` §5 не
срабатывает. Основание держится на опровержении посылки (вариант `L2Delta` уже в T1 с
`CT-RFC-04`/M-18), проверенном мной в `R-019` §1.1 и критиком в `C-045`. **Оговорка та же:**
реализация M-45 трогает `crates/venue-*/src/**` — MD-only carve-out там применим только если
правка остаётся read-only-MD (константа allow-list) без order-egress; подтверждает это
reviewer M-45, не этот вердикт. См. также N1 — документ САМ заявляет risk-critic впереди.

---

## ЧАСТЬ 1. Закрытие находок R-019 — замером

### F1 (БЛОКЕР) → ЗАКРЫТ. `m-45-l2delta-impact.md` приземлён в дерево RFC

```
$ git ls-tree -r origin/main --name-only | grep research/measurements
(пусто — каталога в main НЕТ)
$ git ls-tree -r HEAD --name-only | grep research/measurements
research/measurements/m-45-l2delta-impact.md
research/measurements/td-053-event-size.md
$ git merge-base --is-ancestor 6122fce HEAD; echo $?   -> 0 (теперь ANCESTOR)
```

Пруф-якорь достижим из merge-цели: `6122fce` входит в историю, файл лежит в дереве.
Гейт `6-RFC-SHA` это подтверждает машинно (ниже).

### F2 (БЛОКЕР) → ЗАКРЫТ. `td-053-event-size.md` приземлён; §6 больше не ссылается в пустоту

`3efaabc` — ANCESTOR HEAD. Четыре числа §6 (32 799 / 3236 / 71 237 / 66 032) теперь
подкреплены файлом, лежащим в том же дереве.

**Побочный эффект, который стоит зафиксировать:** на этот же путь уже ссылались ЧЕТЫРЕ
документа в `main`, где файла не было, — включая мои собственные `PROJECT-STATE.md` и
`TECH-DEBT.md`:

```
$ git grep -l "measurements/td-053-event-size\|measurements/m-45-l2delta-impact" origin/main -- '*.md'
origin/main:PROJECT-STATE.md
origin/main:TECH-DEBT.md
origin/main:docs/NEXT-SESSION-PROMPT.md
origin/main:docs/ORCHESTRATION-STATE.md
origin/main:milestones/M-50-floor-scan-large-events.md
```

Этот merge чинит висячую ссылку сразу в пяти местах, а не только в RFC. Класс тот же, что
`C-044` F1, и он был шире, чем виделось в круге 1.

### F3 → ЗАКРЫТ. Путь развёрнут

`docs/07` → `docs/07-cockpit-backend-roadmap.md` (файл существует, проверено `ls docs/`).

### F4 → ЗАКРЫТ ПО СУЩЕСТВУ. Остаточные классы НАЗВАНЫ

§3 дополнен блоком «Чего механизм эпох НЕ решает», где явно поименованы оба класса, которых
мне не хватало: (1) **незамеченный семантический сдвиг** — собственно E-001, против которого
`epoch_id` бессилен, потому что маркер выставляют ЗАРАНЕЕ, а о дефекте не знали; (2)
**забытый операторский шаг** — прямым текстом: «Машинного fail-closed на "состав эмиссии
изменился, а `epoch_id` — нет" **не существует**», с явным выводом класса за объём M-45.
Это то, чего я требовал: назвать, а не умолчать. Формулировка сильнее моей находки — она ещё
и указывает, ЧТО работает против класса 1 (обнаружение: recon-сверка + eyes-on), не выдавая
это за закрытие.

### F5 → ЗАКРЫТ ПО СУЩЕСТВУ. Условие невакуумности `JR-I-10` сформулировано и применено к СЕГОДНЯ

§4 дополнен: COLD засчитывается читаемым **только при фактическом монтировании и проверенном
чтении**, иначе инвариант вакуумен. Ключевое — документ не ограничился абстрактной оговоркой,
а применил её к текущему состоянию: «Storage Box не заведён … **на сегодня COLD читаемым НЕ
считается, и JR-I-10 держится на HOT/WARM**», плюс честно сказано, что оракул §4 п.2 этот
аспект НЕ покрывает (он про `retention_plan`, не про доступность тира).

### F6 → ЗАКРЫТ ПО СУЩЕСТВУ, и закрыт замером, а не переформулировкой

§5 дополнен таблицей фикстур и прямым признанием: у **DET-I-1** (`red_det_replay_digest.rs`)
фикстур с `L2Delta` **ноль**, покрытие «аргументировано, а не проверено оракулом». Проверил
своим замером — цифра верна:

```
$ grep -c L2Delta crates/journal/tests/red_det_replay_digest.rs
0
$ grep -c L2Snapshot crates/journal/tests/red_det_replay_digest.rs
0
```

Сверх требования R-019 документ выводит из находки **задачу для M-45** («добавить `L2Delta`
в фикстуры `red_det_replay_digest.rs`, реплей ×3 бит-идентичен») — то есть остаточный класс
не просто назван, а поставлен в очередь. Это ровно то, чего требует
`.claude/rules/testing.md` («фикстура счастливого пути»).

---

## ЧАСТЬ 2. Встречное утверждение architect'а — проверено МОИМ грепом, ПОДТВЕРЖДЕНО

Architect заявил, что ложным является не замер, а `docs/NEXT-SESSION-PROMPT.md` §5. Проверил
оба вопроса мандата независимо.

### 2.1 В `segments.rs` — ОДИН exhaustive `match` по `MdPayload`, не два

```
$ grep -n -E "match .*payload" crates/journal/src/segments.rs
280:    let payload = match read_frame_payload(&mut r)? {     <- scrutinee ДРУГОЙ (результат
                                                                 read_frame_payload, не MdPayload)
411/460/1409/1755/1787: match postcard::from_bytes / r.read_exact / verify_large_frame  <- не MdPayload
2569:        EventKind::Md(md) => match &md.payload {          <- ЕДИНСТВЕННЫЙ по MdPayload
```

`sed -n '2566,2580p'` — `fn event_data_ts`, восемь вариантов через `|`, БЕЗ `_ =>`.
Мест **пять**, не шесть; объём M-45 не занижен.

Расхождение номеров строк (замер: 2312, факт: 2566-2580) — реальное, но безобидное: файл и
функция те же, строки съехали. RFC §8.2 это оговаривает открытым текстом («мандат говорил
2312-2323 — строки съехали»), а раздел «Источники и пруфы» отдельно предупреждает не доверять
номерам после merge'ей. Это правильная реакция на класс, а не отписка.

### 2.2 Список пяти файлов §1.1 замера == мой список из R-019 §1.2 — совпадает поэлементно

| §1.1 замера | R-019 §1.2 (мой греп круга 1) | Совпало |
|---|---|---|
| `recorder/src/lib.rs:69-79` (`md_kind_label`) | `crates/recorder/src/lib.rs:70` | ✅ |
| `journal/src/segments.rs:2312-2323` (`event_data_ts`) | `crates/journal/src/segments.rs:2569` | ✅ (файл+функция; строки съехали) |
| `sim/src/exchange.rs:222-283` (`on_event`) | `crates/sim/src/exchange.rs:227` | ✅ |
| `research-cli/src/bin/latency_probe.rs:120-134` | `crates/research-cli/src/bin/latency_probe.rs:120` | ✅ |
| `journal/examples/dump.rs:18-38` (первый match) | `crates/journal/examples/dump.rs:18` | ✅ |

**Независимая перекрёстная проверка другим методом** (не грепом по слову `match`, а по
8-му варианту: любой exhaustive-`match` ОБЯЗАН упоминать `MarginInventory`):

```
$ grep -rln --include=*.rs "MdPayload::MarginInventory" crates/ | grep -v "^crates/contracts" | grep -v "/tests/"
crates/journal/examples/dump.rs
crates/journal/src/segments.rs
crates/recorder/src/lib.rs
crates/research-cli/src/bin/latency_probe.rs
crates/sim/src/exchange.rs
crates/venue-binance/src/lib.rs        <- КОНСТРУИРОВАНИЕ (payload: MdPayload::MarginInventory {...},
                                          lib.rs:882), не match — в счёт не идёт
```

5 совпадающих файлов + 1 отсеянный конструктор. Два независимых метода дают один и тот же
набор — **карта из ПЯТИ подтверждена третий раз**.

### 2.3 Значит, ложен документ-источник

```
$ git show origin/main:docs/NEXT-SESSION-PROMPT.md | sed -n '149p'
- **пять** `match`-мест, а не три (карта в `research/measurements/m-45-l2delta-impact.md`
  НЕПОЛНА, правильная — в `CT-RFC-06` §8.2). Проверь своим грепом, включая `examples/**`...
```

Число «пять» верно, а **обвинение замера в неполноте — ложно**: замер сам называет пять и
перечисляет те же пять файлов. Это ровно тот класс, ради которого построен
`verify_design_claims.sh` (документ врёт о состоянии — здесь о состоянии другого документа),
но гейт его не ловит: проверяются пути и SHA, не смысловые утверждения о содержимом. Файл
`docs/NEXT-SESSION-PROMPT.md` — зона architect'а, я его не правлю (`gates.md` §4: описываю
дефект, не проектирую фикс). Заведён **TD-055**.

---

## ЧАСТЬ 3. Doc-гейт класса A — PASS на MERGE-ЦЕЛИ

Гейта с RFC-проверками в `main` ещё НЕТ (версия в `main` — 721 строка, без `6-RFC-SHA`/
`7-RFC-PATH`); взял скрипт с `origin/feat/gate-rfc-claims` (999 строк), как предписано.

```
$ git show origin/feat/gate-rfc-claims:scripts/verify_design_claims.sh > /tmp/vdc-r2.sh
$ bash /tmp/vdc-r2.sh --merge-preview origin/main /tmp/hft-rev-rfc06v2
...
PASS  [6-RFC-SHA] все 26 цитат коммитов (docs/DESIGN.md + docs/rfc/**.md) существуют И входят в историю HEAD/MERGE_HEAD
PASS  [7-RFC-PATH] все 104 путей, процитированных в docs/rfc/**.md, существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0
```

Числа сошлись с заявленными architect'ом (26 SHA / 104 пути) — но взяты моим прогоном, не
переносом. Дополнительно прогнал:

- гейт **из `main`** (без RFC-проверок) на merge-цели → `VERDICT: PASS (0 нарушений)`,
  exit=0 — merge не ломает то, что уже стоит в `main`;
- гейт с `feat/gate-rfc-claims` в обычном режиме на ветке → `VERDICT: PASS (0 нарушений)`,
  exit=0 (11 SHA / 68 путей — меньше, т.к. ветка не содержит доков, доехавших в `main` позже;
  merge-preview — решающий режим, R-013 Б-2/Б-3).

---

## NOTES (не блокируют merge; фиксируются для следующего агента и founder'а)

### N1 — STATUS остаётся `PROPOSED`; ратификация — за founder'ом, и цепочка гейтов документа НЕ исчерпана

Merge приземляет документ, но **не ратифицирует** его. Причины, почему я не перевожу
`STATUS: PROPOSED → ACTIVE`:

1. §9 документа перечисляет ЧЕТЫРЕ пункта, требующих подписи founder'а, первый из которых —
   «Ратификация CT-RFC-06 как whole». `gates.md` §7: ни один агент, включая reviewer'а, не
   подставляет approve вместо явной подписи.
2. Документ САМ объявляет впереди risk-critic (шапка «Гейты впереди: critic → risk-critic →
   founder», §0.3 строка 65: «`contracts`-тематика = RISK-BLOCK»). По `gates.md` §5 этот
   триггер на ДИФФЕ не срабатывает (кода и контрактов дифф не трогает), поэтому merge
   документа законен без risk-critic — но **самозаявленная цепочка документа этим merge'ем
   НЕ считается пройденной**, и следующий агент не вправе трактовать APPROVED как «risk-critic
   пройден». Расхождение шапки с собственным выводом §0.2 (contract-пакет не нужен ⇒
   RISK-BLOCK неприменим) — правка architect'а или явное решение founder'а, не моя зона.
3. `docs/**` вне зоны записи reviewer'а (`scope-guard.md`) — я пишу только
   `PROJECT-STATE.md`, `TECH-DEBT.md` и этот вердикт.

### N2 — «Источники и пруфы» описывают `6122fce` как живущий на ветке

Раздел в конце RFC всё ещё пишет «`6122fce` (ветка `origin/research/m-45-impact`)», хотя
коммит теперь в истории самого RFC. Утверждение исторически верное и гейт его пропускает
(SHA — ANCESTOR), но читатель `main` получит подсказку искать не там. Косметика; не блокер.

---

## Done Block

```
$ git -C /tmp/hft-rev-rfc06v2 log --oneline -1
7c42e97 docs(CT-RFC-06): R-019 F3-F6 — путь docs/07 развёрнут; названы остаточные классы эпох...

$ git diff --stat origin/main...HEAD | tail -1
 5 files changed, 1587 insertions(+)

$ git log --format='%an' origin/main..HEAD | sort | uniq -c
      9 architect
      1 critic
      1 reviewer

$ grep -n -E "match .*payload" crates/journal/src/segments.rs | grep -c "md.payload"
1                       # ОДИН exhaustive match по MdPayload в segments.rs — мест ПЯТЬ, не шесть

$ grep -rln --include=*.rs "MdPayload::MarginInventory" crates/ | grep -v contracts | grep -v /tests/ | wc -l
6                       # 5 match-мест + venue-binance (конструирование, не match)

$ grep -c L2Delta crates/journal/tests/red_det_replay_digest.rs
0                       # F6 подтверждён замером

$ bash /tmp/vdc-r2.sh --merge-preview origin/main /tmp/hft-rev-rfc06v2 | grep -E "^(FAIL|VERDICT)"; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ bash /tmp/vdc-main.sh --merge-preview origin/main /tmp/hft-rev-rfc06v2 | grep -E "^(FAIL|VERDICT)"; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ git status --porcelain
(пусто, кроме этого вердикта до коммита)
```

## Merge

Выполняется этим вердиктом: `git merge --no-ff` ветки в `main` + обновление
`PROJECT-STATE.md`/`TECH-DEBT.md` + post-merge деплой-гейт (`gates.md` §8). Пруф CI/деплоя и
ssh-проверки прода — дописан ниже после merge.
