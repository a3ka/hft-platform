<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: 1dda5b180b1f3e8e881364a381e4f02e540f5b24
audited_head: b8d989eee53b6aebd425de37280ca5e22d98bb95
verdict: DECISION
-->

# A-028 — M-74: полнота набора до диспетчеризации dev'а — РЕШЕНИЕ (вариант «а»)

Созыв законен: два REJECT подряд по одной причине (`C-187` B-1 → `C-188` B-1,
«набор до dev неполон») — триггер `gates.md` §0 п.1. Арбитр — свежий контекст,
сторонами не являюсь. Предмет — вершина `b8d989e` ветки `docs/M-73-closeout-architect`
поверх `1dda5b1`; вердикт `C-188` (`caebf7d`) добавлен на ветку ПОСЛЕ предмета и в
аудируемый диапазон не входит.

## 1. Решение по вопросу полноты — (а), и это НЕ новое правило

**Стандарт ЕДИН, а не контекстен: набор architect'а полон, только когда каждый оракул,
названный спекой, СУЩЕСТВУЕТ как закоммиченный текст до диспетчеризации dev'а, и путь
каждого оракула покрыт Allowed paths. Если оракул целит в сигнатуру, которую вносит dev,
architect объявляет сигнатуру в спеке ДОСЛОВНО и коммитит оракул против неё в состоянии
COMPILE-RED. Послабление касается только КОМПИЛИРУЕМОСТИ оракула — никогда его
СУЩЕСТВОВАНИЯ.**

`C-188` прав. Основания — не мои построения, а уже действующие нормы:

1. `.claude/agents/critic.md:31` — critic «не запускается ДО того, как architect
   закоммитил T2-типы + trait-сигнатуры + RED-тесты + verify-скрипт + milestone-файл;
   если что-то отсутствует → немедленный verdict NOT REVIEWED — ARCHITECT ARTIFACTS
   INCOMPLETE». Норма существования оракула ДО dev'а уже записана; спор мог идти только
   о том, ослабляет ли её прецедент `M-72`. Не ослабляет — см. §2.
2. Профиль architect'а, п.5 Responsibilities: «КОММИТИТ весь набор (тесты+контракт+
   milestone+verify) ДО диспетчеризации dev-агента — тест = спецификация, код без
   падающего теста не пишется».
3. Сама спека `M-74:199-200` обещает: «задачи 1, 5, 6 за architect'ом и пишутся ДО dev'а
   (RED-first)» — и одновременно объявляет оракул задачи 5 ненаписанным (`:163-166`).
   Набор противоречит собственному Handoff'у.

«Тест не может предшествовать самому себе» — верная фраза о СИГНАТУРЕ, не об ОРАКУЛЕ.
Сигнатуру объявляет architect (ему для этого dev не нужен); текст оракула пишется против
объявленной сигнатуры и предшествует ФИКСУ/реализации поведения — ровно так, как это
сформулировала спека `M-72` (см. §2). Ничто в `M-74` не мешало сделать это до dispatch:
T2-контракт файла состояния уже объявлен (формат, атомарность, окно 40 суток), таблица
«кто ставит gauge» уже называет sampler recorder'а. Оставалось объявить сигнатуру
продюсера и написать против неё тест. Препятствия не существовало — существовал пропуск.

## 2. Разбор расхождения с `M-72` — расхождения НЕТ, есть неверная цитата прецедента

`M-74:163-166` утверждает: «сигнатура продюсера объявляется задачей 3, и оракул пишется
против неё — тот же порядок, что на `M-72` задаче 2». Я открыл прецедент и сверил замером.
Утверждение ложно по обеим половинам:

1. **На `M-72` оракул задачи 2 БЫЛ написан и закоммичен architect'ом ДО dev'а.** Коммит
   `2a701eb` («test(M-72): задача 2 — RED на TD-177 против объявленного шва; гейт
   различает вакуум и COMPILE-RED») несёт 155 строк оракула в
   `crates/gateway-serve/tests/red_ws_terminality_entrypoint.rs`; задача 3 (dev) на тот
   момент ⏳ OPEN. Статус задачи 2 — «🚧 COMPILE-RED: **написан**» — состояние
   закоммиченного текста, а не обещание его написать.
2. **На `M-72` сигнатуру объявил ARCHITECT, дословно, в спеке** — раздел «Сигнатура шва —
   объявлена ЗДЕСЬ, дословно» (коммит `d4778f8`), с прямым обоснованием: «"объявленная
   сигнатура", против которой пишется оракул задачи 2, должна быть предъявима, иначе dev
   выберет её сам, а это ровно то, что раздел запрещает». `M-74` делает противоположное:
   отдаёт объявление сигнатуры задаче 3, то есть dev'у.

Прецедент, прочитанный открытием, а не памятью, поддерживает критика, и целиком:
`M-72` ослабил требование «оракул компилируется и падает» до «оракул закоммичен
COMPILE-RED против объявленной сигнатуры», причём гейт `verify_M-72.sh:56-84`
(`chk_named_test`) различает ТРИ исхода — вакуум (фильтр не нашёл оракула) / COMPILE-RED /
исполнено-и-упало — и красен во всех трёх до закрытия задач. «RED-first по сути»
(`harness-track.md` §3) снимает бухгалтерию раздельных коммитов, но не существование
предмета: «значение имеет предъявленное красное» — у ненаписанного оракула предъявлять
нечего.

Это класс «правило цитаты» (профиль architect'а, BINDING): «документ X говорит Y» требует
открытия X. Спека сослалась на прецедент, который при открытии говорит обратное.

## 3. Что обязана сделать каждая сторона

### Architect (до круга 3; пп. 1-2 признаны им до созыва, спора по ним нет)

1. **Снять внутреннее противоречие путей:** Allowed paths (`M-74:84`) дополняется
   `crates/recorder/tests/**` (architect; `*/tests/` — его зона по scope-guard во всех
   крейтах). Спека не имеет права называть оракул (`:162`), чей путь сама не разрешает.
2. **Фикстура — прод-форма, а не суррогат** (`testing.md` §«Целостность гейта», свойство 1).
   Сегменты фикстуры `red_restore_drill.sh:76` — плоские байты `SEGMENT-…-PAYLOAD`;
   манифест `:79` — `{"legacy":[…]}`. Прод-формат сегмента — магия `SEGMENT_MAGIC =
   *b"HFTJRN02"` (`crates/contracts/src/lib.rs:43`) + header-frame + event-frame'ы с crc
   (`crates/journal/src/segments.rs:15-21`); прод-манифест — `LegacyManifest {
   declarations: Vec<LegacySegmentDecl> }` (`crates/contracts/src/lib.rs:68-70`).
   Позитивный контроль `H` обязан проходить у обёртки, зовущей РЕАЛЬНЫЙ читатель;
   фикстура, которую способен принять только mock-читатель, подталкивает исполнителя в
   обход прод-пути и запрещена. КАК строить прод-фикстуру (fixture-builder на реальном
   writer'е, закоммиченный образец, генерация в тест-таргете) — дизайн architect'а
   (§4 `gates.md`, граница reviewer↔architect), требование — только результат: `H` зелен
   с прод-читателем, `C`/`M` красны с ним же.
3. **Объявить сигнатуру продюсера ДОСЛОВНО в спеке** (по образцу `M-72` §«Сигнатура шва»):
   функция/шов recorder'а «файл состояния → значение gauge» с явными параметрами времени
   и окна свежести. Форма — за architect'ом; критерий — сигнатура предъявима, оракул
   пишется против неё, dev её не выбирает.
4. **Написать и закоммитить `crates/recorder/tests/red_restore_drill_metric.rs` ДО
   dispatch.** Пиннит отображение в РЕНДЕРЕ `/metrics` (не в реестре имён): файла нет ⇒ 0;
   `ok≠1` ⇒ 0; `ts` старше окна ⇒ 0; свежий `ok=1` ⇒ 1. COMPILE-RED — санкционированное
   состояние, объявляется в §Tasks явно (как `M-72` задача 2).
5. **Шаги задач 3/5 в `verify_M-74.sh` — трёхисходная форма** (`chk_named_test`-класс из
   `verify_M-72.sh:61-84`): вакуум ≠ COMPILE-RED ≠ исполнено-и-упало. Это не косметика:
   шаг задачи 5 `chk "cargo test … --quiet stale"` решает исход по коду возврата, а cargo
   возвращает 0 при НУЛЕ исполненных тестов — появись файл, где фильтру `stale` нечего
   ловить, шаг зеленеет вакуумно. Урок уже оплачен и записан в `verify_M-72.sh:56-58`:
   «исход шага решает НЕ код возврата в одиночку, а ЧИСЛО ИСПОЛНЕННЫХ тестов».
6. NOTE (не блокер): канарейка задачи 3 `grep -q 'backup_restore_drill_ok' ${EMIT}`
   зелена уже СЕГОДНЯ — она ловит слово в комментарии «deferred», то есть текст, а не
   вызов (`testing.md`: «grep по имени ловит и лог-строки»). Соседний шаг `! deferred` её
   страхует; при правке п.5 стоит связать канарейку с реальным вызовом `set_gauge`.

### Critic (круг 3)

1. Судит ТОЛЬКО закрытие пунктов §3 + находки ИНОГО класса. Вопрос момента написания
   оракула РЕШЁН этим арбитражем и не переоткрывается (`gates.md` §0: решение обязательно
   обеим сторонам); REJECT, основанный на нём заново, кругом не считается.
2. Закрытия `C-188` (B-3, B-4, самопроверка `chk`, граница C) НЕ переоткрываются без
   нового факта.

## 4. Предел кругов — по образцу `П-024`, назначается ЗАРАНЕЕ

Потрачено 2 круга (`C-187`, `C-188`). **Допускается ещё максимум ДВА круга plan-time
критика по pre-dispatch набору `M-74` — круги 3 и 4; итого 4.**

- Круг 3 чист или NOTE ⇒ mechanical appendix и dispatch dev (маршрут `04-workflow.md` §2).
- Круг 3 REJECT ⇒ architect правит, круг 4.
- Круг 4 снова находит класс «набор неполон» (названный-но-отсутствующий артефакт, путь
  вне Allowed paths, фикстура, которую прод-читатель не принимает) ⇒ **СТОП, не круг 5**:
  предмет возвращается founder'у с выбором — пересборка милестоуна другой конструкцией
  (по образцу варианта 3 `П-024`; вердикты `C-187`/`C-188`/`A-028` остаются материалом)
  либо передача набора другому исполнителю. «Протолкнуть dev поверх неполного набора»
  вариантом НЕ является — это отменяло бы §1 этого же решения.
- Находки иного класса на круге 4, не блокирующие по своей природе (не safety), идут
  NOTE-ами; блокирующая находка иного класса — новый предмет со своим счётом
  (`П-022`: решения по одному предмету не распространяются на другие без своего круга).

Почему предел назначается сейчас: `П-024` — «чтобы решение не принималось под
впечатлением от результата»; architect в споре сторона, и счёт кругов без внешнего стопа
уже дал `M-45` три REJECT'а подряд.

## 5. Проверено замером (сверка с числами сторон)

Все прогоны — на чистом worktree `/tmp/a028-wt` @ `b8d989e`; сырой вывод — в Done Block.

1. `red_restore_drill.sh` → `VERDICT: FAIL (1 из 1)`, exit=1 — совпадает с `C-188`.
2. `verify_M-74.sh` → см. Done Block; счёт отказов сверен с `C-188` (`FAIL (7)`).
3. Граница C: диапазон `1dda5b1..b8d989e` не трогает `RETENTION_MODE` (диф пуст);
   `deploy/cron.d/journal-retention:41` = `RETENTION_MODE=dry-run`. `П-023` не нарушен.
4. Шаги задач 3/5 гейта падают ЧЕСТНО (считаются в FAIL, гейт красный), но причина
   проглатывается (`>/dev/null 2>&1` в `chk`) и вакуум неотличим от COMPILE-RED — закрыто
   предписанием §3 п.5.
5. `verify_M-73.sh` и мутация crontab: прод-парсер отвергает испорченный файл (exit=1),
   принимает целый (exit=0); полный гейт — см. Done Block.
6. `SEGMENT_MAGIC`: определение — `crates/contracts/src/lib.rs:43`, а не
   `segments.rs:17` (там doc-коммент wire-формата); существо признанной находки B-1a это
   не меняет.

## Done Block

```text
$ git rev-parse origin/docs/M-73-closeout-architect; git rev-parse b8d989e; git merge-base --is-ancestor b8d989e caebf7d && echo ancestor-ok
caebf7dec720264dbcda3ccdbd1221f740f29db2
b8d989eee53b6aebd425de37280ca5e22d98bb95
ancestor-ok
# вершина ветки ушла на один коммит вперёд предмета — это сам вердикт C-188 (docs(critic))

$ git worktree add /tmp/a028-wt --detach b8d989e   # чистое дерево предмета
HEAD is now at b8d989e spec(M-74): C-187 B-1/B-2/B-3 — набор закоммичен, producer-path назван, выборка достроена [architect]

$ bash scripts/tests/red_restore_drill.sh; echo red_exit=$?
── RESTORE-DRILL: обёртка ещё не внесена
FAIL  обёртки deploy/bin/journal-restore-drill-cron.sh НЕ СУЩЕСТВУЕТ — RED задачи 1 (её вносит engine-dev задачей 2)
VERDICT: FAIL (1 из 1) — RED-first: спецификация есть, реализации нет
red_exit=1                                          # совпадает с C-188

$ bash scripts/verify_M-74.sh; echo verify_M74_exit=$?   # полный прогон, ~12 мин
PASS: самопроверка chk — зелёное проходит, красное СЧИТАЕТСЯ
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
FAIL: bash scripts/tests/red_restore_drill.sh
FAIL: test -x deploy/bin/journal-restore-drill-cron.sh
PASS: ! grep -q 'hft-platform_journal-data/_data' deploy/bin/journal-restore-drill-cron.sh
PASS: grep -q 'backup_restore_drill_ok' crates/recorder/src/metric_emit.rs
FAIL: ! grep -qE 'backup_restore_drill_ok.*deferred' crates/recorder/src/metric_emit.rs
FAIL: cargo test -p recorder --test red_restore_drill_metric --quiet
FAIL: test -f deploy/cron.d/journal-restore-drill
FAIL: crontab -n deploy/cron.d/journal-restore-drill
FAIL: cargo test -p recorder --test red_restore_drill_metric --quiet stale
PASS: git diff 1dda5b1..HEAD -- docker-compose.yml | grep -qE '^[+-].*RETENTION_MODE' && exit 1 || exit 0
PASS: git diff --name-only 1dda5b1..HEAD -- crates/journal crates/contracts | grep -q . && exit 1 || exit 0
VERDICT: FAIL (7)
verify_M74_exit=1                                   # счёт 7 совпадает с C-188

$ git diff 1dda5b1..b8d989e -- docker-compose.yml | grep RETENTION_MODE; echo diff_grep_exit=$?
diff_grep_exit=1                                    # диф пуст: граница C не тронута (П-023)
$ grep -n RETENTION_MODE deploy/cron.d/journal-retention
41:RETENTION_MODE=dry-run

$ cp deploy/cron.d/journal-offsite /tmp/a028-cron-mut && echo 'BAD LINE not a cron entry' >> /tmp/a028-cron-mut && crontab -n /tmp/a028-cron-mut; echo mutated_crontab_exit=$?
"/tmp/a028-cron-mut":63: bad minute
errors in crontab file, can't install.
mutated_crontab_exit=1
$ crontab -n deploy/cron.d/journal-offsite; echo intact_crontab_exit=$?
The syntax of the crontab file was successfully checked.
intact_crontab_exit=0

$ bash scripts/verify_M-73.sh; echo verify_M73_exit=$?   # агрегировано: PASS=22 FAIL=0
PASS: crontab -n 'deploy/cron.d/builder-prune'
PASS: crontab -n 'deploy/cron.d/journal-offsite'
PASS: crontab -n 'deploy/cron.d/journal-retention'
VERDICT: PASS
verify_M73_exit=0                                   # утверждение C-188 о M-73 подтверждено

# Прецедент M-72 — замер, не память:
$ git show 2a701eb --stat --format='%h %s' | head -6
2a701eb test(M-72): задача 2 — RED на TD-177 против объявленного шва; гейт различает вакуум и COMPILE-RED [architect]
 .../tests/red_ws_terminality_entrypoint.rs         | 155 +++++++++++++++++++++
 milestones/M-72-subscription-terminality.md        |   2 +-
 scripts/verify_M-72.sh                             |  10 +-
$ git show origin/feat/M-72-subscription-terminality:milestones/M-72-subscription-terminality.md | grep -c 'Сигнатура шва — объявлена ЗДЕСЬ, дословно'
1

# Форма прод-сегмента и манифеста — замер:
$ grep -n 'pub const SEGMENT_MAGIC' crates/contracts/src/lib.rs
43:pub const SEGMENT_MAGIC: [u8; 8] = *b"HFTJRN02";
$ grep -n -A2 'pub struct LegacyManifest' crates/contracts/src/lib.rs
68:pub struct LegacyManifest {
69:    pub declarations: Vec<LegacySegmentDecl>,
70:}
$ grep -nE "SEGMENT-|\{\"legacy\"" scripts/tests/red_restore_drill.sh | head -2
76:    printf 'SEGMENT-%s-PAYLOAD' "$i" > "${d}/cold/segment-${i}.jrnl" || die "сегмент"
79:    printf '{"legacy":["segment-0001.jrnl"]}' > "${d}/cold/journal.legacy.json" || die "манифест"
```

Дополнительный факт, снятый прогоном: шаги `! grep -q … ${DRILL}` (задача 2) и
`grep -q backup_restore_drill_ok ${EMIT}` (задача 3) зелены ВАКУУМНО — первый потому, что
`grep` на несуществующем файле даёт exit=2 и отрицание превращает это в PASS, второй потому,
что имя метрики находится в КОММЕНТАРИИ «deferred». Сегодня оба прикрыты красными соседями
(`test -x` и `! deferred`), но при правке гейта по §3 п.5 обе проверки следует привязать к
исполнению, а не к тексту.

## Несогласие сторон

Фиксируется в этом файле при появлении; решение обязательно к исполнению обеими сторонами
(`gates.md` §0).
