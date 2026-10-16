# A-053 — M-98: обязан ли RED переподписки пиннить ТОЧНОЕ `history_start_seq` в s0/s3/s4

<!-- GATE-META
milestone: M-98
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: 6c2680ebf8e3293ef5b1c7e0d01ad01adcd5d3bd
verdict: DECISION
-->

Арбитр (`gates.md` §0, п.1: второй REJECT подряд по одному инварианту — `C-297` B1 → `C-302`
B1-r2), свежий контекст, Fable. Предмет — ветка `origin/feat/M-98-switch-provenance`, вершина
`6c2680eb` взята командой (Done Block); справочный SHA мандата `6c2680eb` совпал с вершиной.
Стороны: architect (автор набора, коммиты `ec85d138`…`18dec926`), critic (`C-297`, `C-302`).
Номер — `scripts/reserve_artifact_id.sh A` (резерв `A-053`, снят после приземления).

## Решение — коротко

1. **Контрпример `C-302` воспроизведён замером и РАСШИРЕН.** Против оракулов вершины `6c2680eb`
   мутант `completeone` (начало `= 1` на целом журнале) проходит 5/5; мутант `frozenwrong`
   (fail-closed исход `(frozen + 1, true)`) — тоже 5/5. Половина объявленного исхода `s3`/`s4`
   не судится никем. **По существу прав critic; блокировка dev в `C-302` законна.**
2. **`s0` обязан пиннить точное начало.** `VB-I-11` определяет поле как «seq первого РЕАЛЬНО
   свёрнутого события» (`docs/fa/viz-backend.md:294`), и оба соседних пути уже пиннят его на целом
   журнале: ADD — `red_m87_provenance_on_add_path::p2` (`assert_eq!(start_seq_of(&msg), 0)`),
   legacy — `red_ws_honesty_sessions::o6_full_journal_is_not_truncated` (`:210-211`). У ветки
   SWITCH соседнего оракула нет — `s0` единственный, кто может это сказать. **Эталон `s0` —
   seq первого события `journal::stream` под прод-`EpochFilter`** (тот же `first_retained_seq`,
   что в `s1`/`s2`; на целом журнале он равен началу журнала, в фикстуре — `0`).
3. **`s3`/`s4` обязаны пиннить `frozen_start_seq` точно.** Спека §3 п.2 объявляет исход ПАРОЙ
   `(frozen_start_seq, true)`; оракул, судящий одну компоненту пары, не меряет то, что обещает
   (`testing.md` «Оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ»). Прецедент на уровне функции —
   `red_m87_history_provenance_failclosed::h4` (`assert_eq!(start, FROZEN)`). **Эталон frozen —
   шапка ЧЕКПОИНТА**, с которого SWITCH поднимает редуктор: именно её `history_start_seq`
   `resume` кладёт в снимок (`crates/gateway/src/lib.rs:3968`, `:4520`), и именно `snap.history_start_seq`
   прототип и ADD/legacy-код базы подставляют в `(frozen, true)` (`a660b3a7` `lib.rs:1612-1616`,
   `:2454-2457`). Читается публичным API без транспорта:
   `gateway::checkpoint::read_checkpoint_header_pub(&gateway::checkpoint::ckpt_path_for_pub(ckpt, &sel())).history_start_seq`.
   Первый снимок подписки эталоном НЕ является: это выход соседней ветки ADD, совпадающий с
   frozen лишь потому, что журнал в тот момент цел.
4. **Соразмерность: требование одно на всех путях, но пробел ADD/legacy — ВНЕ `M-98`.** На ADD
   точность начала в мирах успеха уже пиннят `p1`/`p2` (M-87); `f0`…`f6` её не пиннят, а в
   fail-closed мирах (`f3`…`f6`, `s3`/`s4` до правки) точность frozen на ПУТИ не пиннит никто —
   `h3`/`h4` стерегут функцию, которую путь выдачи после M-95 не зовёт (спека `M-95` §3 говорит
   это прямо). Это долг, не аргумент: `M-98` §6 запрещает менять ADD/legacy сверх выноса блока,
   `I-2` объявляет их нетронутыми, а `red_m95_provenance_fresh.rs` — другой sacred-набор. Долг
   заводит reviewer карточкой `TD` на close-out (`TECH-DEBT.md` reviewer-owned); architect
   вписывает строку в `M-98` §5 «Что НЕ входит», чтобы не потерялось.
5. **Предписание — A1…A6 ниже, все architect'у; impl-код не трогается, T1 не задет.** Следующий
   круг critic'а — ОДИН, СУЖЕННЫЙ до исполнения A1…A6 и перемеренной матрицы (8 миров).
   `C-297` B2 (механизм матрицы), `s1`/`s2` с эталоном `first_retained_seq`, точка
   `m98-switch-catalog:<id>`, сторож inotify и свидетель места — повторному спору не подлежат.

## Вопрос 1 — воспроизведение контрпримера `C-302` замером

Дерево `/tmp/hft-arbiter-m98-meas` (detached на `6c2680eb`, снесено после замера): `lib.rs`
транспорта заменён на `a660b3a7:crates/gateway-serve/src/lib.rs`, применён
`scripts/tests/m98/prototype.patch` (как делает `red_m98_mutants.sh`), затем две правки прототипа:

```rust
// строка `snap.history_start_seq = …` прототипа — добавлен completeone (C-302, дословно):
snap.history_start_seq = if (m98 == "startone" && lt) || (m98 == "completeone" && !lt) { 1 } else { ls };
// `p.unwrap_or((snap.history_start_seq, true))` — добавлен frozenwrong (моё расширение):
p.unwrap_or((if m98 == "frozenwrong" { snap.history_start_seq + 1 } else { snap.history_start_seq }, true))
```

Оракулы вершины `6c2680eb` (нетронутые), `cargo test -p gateway-serve --features testing --test red_m98_switch_provenance`:

```text
##### M98_MUT=''              (прототип по §3 — контроль)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.49s
exit=0
##### M98_MUT='completeone'   (C-302: начало = 1 на целом журнале)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.07s
exit=0
##### M98_MUT='frozenwrong'   (моё расширение: fail-closed = (frozen + 1, true))
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.82s
exit=0
```

Посценарно (тот же бинарь, без `-q`):

```text
##### ИСХОДНЫЕ оракулы 6c2680eb, M98_MUT='completeone'
test s0_switch_without_retention_history_is_complete ... ok
test s1_switch_after_retention_is_honest ... ok
test s2_switch_retention_between_catalog_and_provenance_is_honest ... ok
test s3_switch_catalog_unlistable_is_fail_closed ... ok
test s4_switch_refresh_failure_after_stale_is_fail_closed ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.47s
exit=0
##### ИСХОДНЫЕ оракулы 6c2680eb, M98_MUT='frozenwrong'
test s0_switch_without_retention_history_is_complete ... ok
test s1_switch_after_retention_is_honest ... ok
test s2_switch_retention_between_catalog_and_provenance_is_honest ... ok
test s3_switch_catalog_unlistable_is_fail_closed ... ok
test s4_switch_refresh_failure_after_stale_is_fail_closed ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.67s
exit=0
```

Контрпример `C-302` подтверждён. Второй мутант показывает, что дыра шире названной критиком:
не только `s0`, но и объявленный спекой fail-closed исход `s3`/`s4` держится ТОЛЬКО на булеве.

## Вопрос 2 — требует ли `VB-I-11` точного начала в `s0`, `s3`, `s4`; что считать эталоном

**`s0` — да, по самому `VB-I-11`.** Инвариант определяет значение поля, а не только флаг:
«`history_start_seq` (seq первого РЕАЛЬНО свёрнутого события — НЕ `header.first_seq` …)»
(`docs/fa/viz-backend.md:294`). Снимок с `history_start_seq = 1` при журнале, начинающемся с `0`,
нарушает его буквально — при честном `truncated = false`. Оба соседних пути это уже пиннят на
целом журнале (`p2` ADD, `o6_full_journal_is_not_truncated` legacy); у SWITCH такого соседа нет.
Эталон — `first_retained_seq(dir)` (первое событие `journal::stream` под `EpochFilter::OwnCaptureOnly`),
не литерал `0`: литерал привязан к фикстуре, поток — к журналу.

**`s3`/`s4` — да, но основание другое: контракт fail-closed исхода, объявленный спекой.**
`VB-I-11` не определяет, ЧТО писать в `history_start_seq`, когда истина неизвестна; это определяет
контракт M-87 задачи 20 → `M-95` §3 → `M-98` §3 п.2: `(frozen_start_seq, true)` — «не знаем, не
обещаем». Пара — одно обещание; оракул, проверяющий только `true`, зелен против реализации,
пишущей в начало что угодно (`frozenwrong` — замер выше). `h4` на уровне функции пиннит именно
`start == FROZEN`; на пути SWITCH это обязан делать `s3`/`s4`.

**Эталон frozen — шапка чекпоинта, не первый снимок.** Что такое frozen по коду: на тёплом пути
`resume` снимок получает `history_start_seq: ckpt_header.history_start_seq`
(`crates/gateway/src/lib.rs:3968`, `:4520`); ветки ADD/legacy базы и прототип в `(frozen, true)`
подставляют ровно это `snap.history_start_seq`. Значит независимый источник — сам файл чекпоинта:

```rust
fn frozen_start(ckpt: &std::path::Path) -> u64 {
    gateway::checkpoint::read_checkpoint_header_pub(&gateway::checkpoint::ckpt_path_for_pub(ckpt, &sel()))
        .expect("шапка чекпоинта читается")
        .history_start_seq
}
```

Оба API публичны (`crates/gateway/src/lib.rs:4063`, `:4807`), транспорт не участвуют. Первый
снимок (ADD) брать эталоном нельзя: это выход соседнего кода, равный frozen только потому, что
журнал цел; оракул «X == выход соседа» — зависимый эталон (`testing.md` «Мутационный контроль»).
Setup-страж обязателен: ДО действия в остановке `frozen == first_retained_seq(dir)` — слепок и
целый журнал согласны в начале истории (в фикстуре оба `0`); иначе мир построен не тот.

**Предел, названный честно.** В фикстуре `frozen = 0` совпадает с началом целого журнала, поэтому
`s3`/`s4` порознь не различат «frozen» и «захардкоженный `0`». Набор целиком различает: `(0, true)`
всегда роняет `s0` (флаг) и `s1`/`s2` (точное начало `> 0`). Это свойство НАБОРА, и оно
достаточно; отдельный мир «чекпоинт снят на уже усечённом журнале» (frozen > 0) — усиление, не
требуемое этим решением.

## Вопрос 3 — соразмерность с ADD/legacy (`f0`, `f3`…`f6`)

Замер покрытия точного начала по путям (все файлы на `6c2680eb`):

| путь | мир | точное начало пиннится | где |
|---|---|---|---|
| ADD | цел | да, `== 0` | `red_m87_provenance_on_add_path::p2` |
| ADD | усечён (успех) | да, `== earliest` из `journal::stream` | `…::p1` |
| ADD | усечён в точке (успех) | только `> 0` | `red_m95_provenance_fresh::f1` |
| ADD | fail-closed | **нет** (только булев) | `f3`, `f5` |
| legacy | цел | да, `== 0` | `red_ws_honesty_sessions::o6_full_journal_is_not_truncated` |
| legacy | усечён | только `> 0` | `…::o6_pruned_journal_is_honestly_marked`, `f2` |
| legacy | fail-closed | **нет** | `f4`, `f6` |
| SWITCH | цел | **нет** → A1 | `s0` |
| SWITCH | усечён (успех) | да (`C-297` B1 закрыт) | `s1`, `s2` |
| SWITCH | fail-closed | **нет** → A2 | `s3`, `s4` |

Вывод: требование ОДНО, и на ADD/legacy оно в мирах успеха выполнено соседними оракулами M-87
(`p1`/`p2`, `o6`), а в fail-closed мирах не выполнено НИ НА ОДНОМ пути: `h3`/`h4` стерегут
`history_provenance_for_serve`, которую путь выдачи после M-95 не зовёт (спека `M-95` §3,
`docs/archive/M-95-catalog-once.md:81-82`). Это настоящий долг, но:

- `M-98` §6 запрещает менять поведение ADD/legacy сверх выноса блока; `I-2` объявляет их
  нетронутыми; `red_m95_provenance_fresh.rs` — чужой набор другого милестоуна;
- асимметрия НЕ оправдывает пробел на SWITCH: у ADD/legacy точность в мирах успеха уже стоит
  (M-87), у SWITCH её несут только `s0`…`s4`. «Соседи тоже не пиннят frozen» — основание завести
  долг соседям, а не снять требование с предмета.

**Решение:** вне `M-98`. Reviewer на close-out заводит карточку `TD` «точное `frozen_start_seq`
fail-closed исхода не пиннится на ПУТИ выдачи ADD/legacy (`f3`…`f6`); `f1`/`f2`/`o6_pruned` —
только `> 0`» (зона закрытия architect, RED). Architect вписывает строку в `M-98` §5 — предмет
назван, исполнитель назван, в этот милестоун не входит.

## Замер — предписываемые оракулы против прототипа и всех мутантов

Там же, в своём дереве, в КОПИЮ `red_m98_switch_provenance.rs` добавлены ровно A1/A2 (помощник
`frozen_start` выше; `s0`: `assert_eq!(start_seq(&body), first_retained_seq(dir.path()))`;
`s3`/`s4`: `let frozen = frozen_start(ckpt.path()); assert_eq!(frozen, first_retained_seq(dir.path()), "SETUP…")`
до переподписки, `assert_eq!(start_seq(&body), frozen)` после `truncated`). Предмет не менялся —
это проба решения, не правка набора (её делает architect, A1/A2).

```text
##### ПРОБА (оракулы A-053) M98_MUT=''
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.16s
exit=0
##### ПРОБА (оракулы A-053) M98_MUT='completeone'
test s0_switch_without_retention_history_is_complete ... FAILED
assertion `left == right` failed: ПРОБА A-053 s0: начало истории на целом журнале: {"cursor":{"upto_seq":2999},"history_start_seq":1,"history_truncated":false,…
test result: FAILED. 4 passed; 1 failed; …
exit=101
##### ПРОБА (оракулы A-053) M98_MUT='frozenwrong'
test s3_switch_catalog_unlistable_is_fail_closed ... FAILED
test s4_switch_refresh_failure_after_stale_is_fail_closed ... FAILED
assertion `left == right` failed: ПРОБА A-053 s3: fail-closed начало != frozen (шапка чекпоинта): {"cursor":{"upto_seq":2999},"history_start_seq":1,"history_truncated":true,…
assertion `left == right` failed: ПРОБА A-053 s4: fail-closed начало != frozen (шапка чекпоинта): {"cursor":{"upto_seq":2999},"history_start_seq":1,"history_truncated":true,…
test result: FAILED. 3 passed; 2 failed; …
exit=101
##### ПРОБА (оракулы A-053) M98_MUT='startone'
test s1 … FAILED   test s2 … FAILED   test s3 … FAILED   test s4 … FAILED   (s0 ok)
assertion `left == right` failed: ПРОБА A-053 s3: fail-closed начало != frozen …"history_start_seq":1,"history_truncated":true…
test result: FAILED. 1 passed; 4 failed; …
exit=101
##### ПРОБА (оракулы A-053) M98_MUT='alwaystrue'
test s0 … FAILED   test s1 … FAILED   test s2 … FAILED   (s3, s4 ok)
test result: FAILED. 2 passed; 3 failed; …
exit=101
##### ПРОБА (оракулы A-053) M98_MUT='nofresh'
test s2 … FAILED   test s3 … FAILED   test s4 … FAILED   (s0, s1 ok)
test result: FAILED. 2 passed; 3 failed; …
exit=101
##### ПРОБА (оракулы A-053) M98_MUT='swallow'
test s3 … FAILED   test s4 … FAILED   (s0, s1, s2 ok)
test result: FAILED. 3 passed; 2 failed; …
exit=101
```

Три факта из пробы, обязательные к учёту:

1. Прототип по §3 остаётся зелёным 5/5 — предписание исполнимо, честная реализация проходит.
2. `completeone` падает РОВНО на `s0`, `frozenwrong` — РОВНО на `s3 s4`: новые ассерты
   привязаны к своим дефектам, соседние миры не задевают.
3. **Объявленный набор `startone` сдвигается: `s1 s2` → `s1 s2 s3 s4`.** Мутант пишет `1` при
   любом `lt == true`, а в fail-closed мирах `lt == true` и frozen `= 0`. Это не ослабление, а
   усиление — но таблица §8 и `red_m98_mutants.sh` обязаны отражать ИЗМЕРЕННЫЙ набор
   (`C-302`: «declared sets and §8 must agree with the measured result»). Остальные наборы
   (`alwaystrue` `s0 s1 s2`, `nofresh` `s2 s3 s4`, `swallow` `s3 s4`) не изменились.

## Обязательные действия architect'у (решение арбитра, обязательно обеим сторонам)

- **A1 — `s0`:** после `!truncated` — `assert_eq!(start_seq(&body), first_retained_seq(dir.path()), …)`
  с сообщением класса «`M-98` / `C-302` B1-r2 / `VB-I-11`: на целом журнале начало истории не равно
  первому событию `journal::stream`». Doc-комментарий `first_retained_seq` расширить: эталон
  действует и на целом журнале (там он — начало журнала).
- **A2 — `s3`/`s4`:** помощник `frozen_start(ckpt)` через `gateway::checkpoint::{ckpt_path_for_pub,
  read_checkpoint_header_pub}` (форма — §«Вопрос 2»); ДО `resubscribe_with_pause` —
  `let frozen = frozen_start(ckpt.path())` + setup-страж `frozen == first_retained_seq(dir.path())`;
  ПОСЛЕ ассерта `truncated` — `assert_eq!(start_seq(&body), frozen, …)` с сообщением класса
  «fail-closed исход обязан быть `(frozen_start_seq, true)`: начало не равно шапке чекпоинта».
  Порядок ассертов сохранить (сначала `truncated`): сообщения `swallow`/`nofresh` остаются
  диагностичными.
- **A3 — `scripts/tests/m98/prototype.patch`:** два переключателя `M98_MUT`: `completeone` —
  начало `= 1` при `!lt` (дословно `C-302`); `frozenwrong` — `(snap.history_start_seq + 1, true)`
  в ветке `p.unwrap_or(…)` (ветка `None` каталога фикстурами не достигается — оговорить
  комментарием, не изображать покрытие).
- **A4 — `scripts/tests/red_m98_mutants.sh`:** две строки — `completeone` → `"s0"`,
  `frozenwrong` → `"s3 s4"`; строка `startone` → `"s1 s2 s3 s4"`; `VERDICT: PASS — 8 миров`.
  Наборы ПЕРЕМЕРИТЬ прогоном (не переписать с этой записи): отчёт арбитра — гипотеза, прогон —
  факт (`gates.md` §8). Расхождение с таблицей здесь — повод исправить таблицу, не замер.
- **A5 — спека `milestones/M-98-switch-provenance.md`:** §3 п.2 — назвать эталон frozen (шапка
  чекпоинта, читаемая публичным API); §4 — строка `I-1` получает `s0` с точным началом, строка
  `VB-I-11` — все пять; §8 таблица миров — ожидания `s0` (`start == первое событие журнала`),
  `s3`/`s4` (`(frozen из шапки чекпоинта, true)`), таблица мутаций — две новые строки и
  перемеренный `startone`; §5 — строка долга ADD/legacy (§«Вопрос 3»); §14 — строки `C-302` и
  `A-053`. Шапка RED-файла (таблица сценариев, строки `s1`/`s3`/`s4`) всё ещё говорит
  «`history_start_seq > 0`» — устарела с круга 1, привести к §8.
- **A6 — `scripts/verify_M-98.sh`:** метка шага матрицы «прототип + 4 мутанта» → «прототип + 6
  мутантов»; больше ничего — состав шагов верен.

Impl-код не трогается; T1 не задет; `allowed paths` §10 не расширяются.

## Нужен ли ещё круг

**Да, один, СУЖЕННЫЙ.** Решение обязательно обеим сторонам (§0), но plan-time гейт закрывает
critic, а не арбитр (`gates.md` §9: вердикт критика засчитывается за перепроверку, обратное —
нет). Круг 3 critic'а проверяет ТОЛЬКО исполнение A1…A6 и перемеренную матрицу (8 миров,
`red_m98_mutants.sh` `VERDICT: PASS`). **Законным** на круге 3 является REJECT лишь по (а)
неисполненному пункту A1…A6, (б) расхождению объявленных и измеренных наборов, (в) НОВОМУ
измеренному контрпримеру против НОВЫХ оракулов. **Не переоткрывается:** `C-297` B2 (механизм
матрицы — worktree на HEAD + `lib.rs` базы + патч), `s1`/`s2` и эталон `first_retained_seq`,
канал `m98-switch-catalog:<id>` и место точки, сторож inotify (`C-285`), свидетель места
(`A-050` A2), выбор `MINOR`, маршрут §13. Ожидаемый вердикт при исполнении — `NOTE`, затем
engine-dev по задаче 1.

## NOTE — вне спора, но названо

1. **`red_m98_mutants.sh` строит матрицу на `lib.rs` базы `a660b3a7`, а не на продуктовом коде.**
   Это правильно для свойства «различающая сила набора» и объявлено в шапке скрипта; но после
   реализации dev'а матрица НЕ скажет ничего о продуктовой ветке SWITCH — её судит обычный прогон
   `s0`…`s4`. Tester'у и reviewer'у не принимать `VERDICT: PASS — 8 миров` за проверку реализации.
2. **Пробел ADD/legacy (§«Вопрос 3») — карточка `TD` reviewer'у на close-out `M-98`.** Текст
   карточки дан там же; номер — `scripts/reserve_artifact_id.sh TD`.

## Done Block

```text
$ git -C /home/nous/hft-platform fetch origin && git rev-parse origin/feat/M-98-switch-provenance
6c2680ebf8e3293ef5b1c7e0d01ad01adcd5d3bd
$ git merge-base --is-ancestor 6c2680eb origin/feat/M-98-switch-provenance && echo "6c2680eb IS ancestor"
6c2680eb IS ancestor
$ git log --oneline a660b3a7..origin/feat/M-98-switch-provenance | wc -l
10

$ git worktree add --detach /tmp/hft-arbiter-m98-meas origin/feat/M-98-switch-provenance
$ git show a660b3a7:crates/gateway-serve/src/lib.rs > /tmp/hft-arbiter-m98-meas/crates/gateway-serve/src/lib.rs
$ (cd /tmp/hft-arbiter-m98-meas && patch -s -p1 < scripts/tests/m98/prototype.patch) && echo "patch applied"
patch applied
$ grep -n 'completeone\|frozenwrong' /tmp/hft-arbiter-m98-meas/crates/gateway-serve/src/lib.rs
1437:                                    p.unwrap_or((if m98 == "frozenwrong" { snap.history_start_seq + 1 } else { snap.history_start_seq }, true))
1441:                            snap.history_start_seq = if (m98 == "startone" && lt) || (m98 == "completeone" && !lt) { 1 } else { ls };

$ for m in "" completeone frozenwrong; do M98_MUT="$m" cargo test -q -p gateway-serve --features testing --test red_m98_switch_provenance; done
  → 5 passed / 5 passed / 5 passed (вывод — §«Вопрос 1»)

$ # проба A1/A2 в копии теста (не предмет): прототип 5/5 ok; completeone → s0 FAILED; frozenwrong → s3 s4 FAILED;
$ # startone → s1 s2 s3 s4 FAILED; alwaystrue → s0 s1 s2; nofresh → s2 s3 s4; swallow → s3 s4 (вывод — §«Замер»)

$ grep -n "VB-I-11" docs/fa/viz-backend.md | head -1
183:  `VB-I-11` 7→8, …      (определение — строка 294)
$ grep -n "assert_eq!(start_seq_of(&msg), 0" crates/gateway-serve/tests/red_m87_provenance_on_add_path.rs
273:    assert_eq!(start_seq_of(&msg), 0, "на целом журнале начало истории — 0");
$ grep -n "pub fn ckpt_path_for_pub\|pub fn read_checkpoint_header_pub" crates/gateway/src/lib.rs
4063:    pub fn ckpt_path_for_pub(ckpt_dir: &Path, sel: &Selector) -> PathBuf {
4807:    pub fn read_checkpoint_header_pub(path: &Path) -> Option<CkptHeader> {
$ grep -n "history_start_seq: ckpt_header.history_start_seq\|ckpt_header.history_start_seq," crates/gateway/src/lib.rs
3968:                    history_start_seq: ckpt_header.history_start_seq,
4520:                    ckpt_header.history_start_seq,

$ bash scripts/reserve_artifact_id.sh A
A-053

$ git fetch origin && git rev-parse origin/feat/M-98-switch-provenance   # повторно, перед записью (04-workflow §2 п.4)
6c2680ebf8e3293ef5b1c7e0d01ad01adcd5d3bd                                  # ветка не ушла
```

FA: у `crates/gateway-serve` FA нет (`reading-map.md` §2); опора — `docs/fa/viz-backend.md`
`VB-I-11` (предмет, `:294`) и `VB-I-2` (live == replay: провенанс меняет метку полноты, не ряды).
