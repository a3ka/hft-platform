<!-- GATE-META
milestone: TD-236
audited_repo: a3ka/hft-platform
audited_base: b0da03bce294a5e456b143c8dc66eeb6b8851696
audited_head: 27df7011345e865311251dd56c549f0fb9a4da24
verdict: REJECT
-->

# R-219 — PR-гейт `TD-236` rev2: повторный круг по правке комментариев `gateway-serve`

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL: диапазон трогает
`crates/gateway-serve/src/**`).
**Предмет:** `origin/fix/td-236-comments` → `main`, PR #302. Второй круг после
`R-218` (REJECT, `d217829`).
**База аудита:** `b0da03b` (merge-base с `origin/main`) · **вершина:** `27df701`.

## §0 — Вершина взята КОМАНДОЙ; ветка ОТСТАЛА, гейты прогнаны на дереве слияния

```
$ git fetch origin; echo fetch=$?
fetch=0
$ git rev-parse origin/fix/td-236-comments
27df7011345e865311251dd56c549f0fb9a4da24
$ git log --oneline origin/main..origin/fix/td-236-comments
27df701 fix(TD-236): comment-only corrections (R-218) — верная симметрия v1, frames_received, оба inc_attempts [engine-dev]
1869f7f Merge remote-tracking branch 'origin/main' into fix/td-236-comments
d217829 docs(review): R-218 — PR-гейт TD-236 REJECT (...) [reviewer]
05ed925 fix(TD-236): comment-only corrections — OPS-I-11, дельты, имена функций, M-89 архивные ссылки [engine-dev]
```

Вершина = SHA мандата. Ветка не ушла вперёд.

**Ветка отстала от `origin/main` на ТРИ коммита — названо, а не умолчано:**

```
$ git log --oneline origin/fix/td-236-comments..origin/main
403c29e Merge pull request #303 from a3ka/docs/archive-m90-m91
8a790e5 docs(archive): close-out M-90, M-91 — спеки, гейты и пробы карт в docs/archive по норме Р-2 [architect]
6282266 test(harness): red_artifact_ids — метка ARCHIVED-REF-OK на синтетической фикстуре milestones/M-90-a.md [architect]
```

Это НЕ формальность: `403c29e` вынес `milestones/M-91-legacy-serving-counters.md` и
`scripts/verify_M-9{0,1}.sh` в `docs/archive/`, а предмет этого PR — комментарии,
ссылающиеся на спеку `M-91`. Защита стоит с `strict: false` (`gates.md` §8), поэтому
зелёный чек на ветке был бы снят на СТАРОЙ базе. Все гейты §8 прогнаны на дереве
слияния `953858f` (`git merge origin/main`, конфликтов нет), а не на ветке. Проверка,
которую мог дать только merge-preview, — барьер архивных ссылок: PASS, 68 артефактов
(было 64 — прибавились четыре вынесенных), НОВЫХ висячих ссылок нет.

Тестер (PASS, 2026-10-03T13:15Z) прогонял на ветке `27df701`, не на дереве слияния.
Исход совпал почленно (см. §8), претензии нет — но пункт 4 условия `R-218` закрыт ЭТИМ
прогоном, а не его.

## §1 — Block-scope: ЧИСТО, comment-only подтверждено СВОИМ прогоном

```
$ git diff --numstat 27df701~1 27df701
17	9	crates/gateway-serve/src/lib.rs

$ git diff 27df701~1 27df701 -- crates/gateway-serve/src/lib.rs \
    | grep -E "^[+-]" | grep -vE "^[+-]{3}" | grep -vcE "^[+-]\s*//"
0
```

Ноль не-комментарных строк в диапазоне ⇒ «comment-only» ПОДТВЕРЖДЕНО (считал сам, не
принял на слово). Один файл, зона engine-dev (`scope-guard.md`:
`crates/gateway-serve/src/**`). Sacred не тронут: `crates/*/tests/**`,
`crates/contracts/**`, `scripts/{verify_*,check_*}.sh` в диапазоне отсутствуют.

Коммит атомарный, один, subject ссылается на `TD-236` и на круг `R-218`, метка
`[engine-dev]`, co-author-трейлера нет.

## §2 — Block-C (контракты): НЕПРИМЕНИМ

`crates/contracts/**` не тронут ⇒ contract-RFC не требуется.

## §3 — Block-risk: НЕПРИМЕНИМ, основание названо

`gates.md` §5 привязывает RISK-BLOCK к `crates/risk|killswitch|oms|venue-*|contracts`.
Ни одного в диапазоне; `gateway-serve` — read-only WS-транспорт журнала без order-path
(`GS-I-1`). risk-critic в цепочке не требуется.

## §4 — Предъявление FA (`gates.md` §4, M-66)

Тронут `crates/gateway-serve/src/**`; по таблице `check_review_fa.sh:195-199` крейт
отображён на `docs/fa/viz-backend.md`, префикс `GS`. Живые инварианты, названные по
существу предмета и проверенные на вершине:

- **`GS-I-4`** (`docs/fa/viz-backend.md:289`, внутри текста `VB-I-6`) — `gateway-serve`
  сериализует `Snapshot`/`Frame` ЦЕЛИКОМ (`serde_json::to_vec`). Это ровно тот путь, на
  котором стоит правимый комментарий: сдача кадра снимка в сокет и инкремент успеха
  сразу после неё.
- **`OPS-I-11`** (`docs/fa/ops.md:479`) — тишина ВЫДАЧИ; теперь живёт в `main` (см. §5).

Напоминаю то, что делает находку §6 не придиркой и что `docs/fa/viz-backend.md:369-371`
называет само: `GS-I-*` таблицей в §5 FA НЕ объявлены, дом инвариантов — спека `M-28`, а
`crates/gateway-serve` своей FA не имеет (`M-87` §17). Комментарий в прод-файле —
единственная документация этих инвариантов НА МЕСТЕ.

## §5 — Что круг закрыл ВЕРНО (предъявляю замером)

**Б-2 СНЯТ — `OPS-I-11` объявлен в `main` раньше ссылки на него.**

```
$ git grep -n 'OPS-I-11' origin/main -- docs/fa/ops.md | cut -c1-120
origin/main:docs/fa/ops.md:476:| OPS-I-8 | ... тишина ВЫДАЧИ клиентам — `OPS-I-11` ...
origin/main:docs/fa/ops.md:479:| OPS-I-11 | **Тишина ВЫДАЧИ — алерт (`TD-237`; ...
```

PR #301 (`docs/td-237-ops-i-11`) влит `b0da03b` в 12:26Z — ДО этого круга. Порядок, на
котором я настаивал в `R-218` §7, соблюдён: документация инварианта в `main` не позже
ссылки из прод-кода. Висячей ссылки больше нет.

**N-3 ЗАКРЫТ — обе точки `inc_attempts` названы, и названы ВЕРНО.** Новый текст `:2081-2082`:
«ДВА `inc_attempts`: :1172 в ветке С `policy`, ДО `admit`; :1293 в bind-пути БЕЗ `policy`,
ДО `readiness` — `admit` там не зовётся». Факт:

```
$ sed -n '1170,1174p' crates/gateway-serve/src/lib.rs
                if let Some(policy) = inner.policy.clone() {
                    use super::admission::{admit, readiness, ServingOutcome};
                    metrics::inc_attempts(inner.counters.as_ref()); // Попытка ... ДО admit/readiness,
                    ...
                    let outcome = admit(&policy, &sel);

$ sed -n '1293,1296p' crates/gateway-serve/src/lib.rs
                    metrics::inc_attempts(inner.counters.as_ref());
                    let ready_outcome = if inner.cfg.checkpoint_dir.is_some() {
                        use super::admission::{readiness, AdmissionPolicy, ServingOutcome};
```

Обе характеристики точны почленно: `:1172` — внутри `if let Some(policy)`, до
`admit(&policy, &sel)`; `:1293` — ветка БЕЗ политики, и `use` этой ветки импортирует
`readiness`, но НЕ `admit` (тот в ветке не зовётся). Утверждение «`admit` там не зовётся»
предъявлено не прозой, а составом импорта.

**N-4 ЗАКРЫТ — усиление опирается на уже существующий признак.** Новый текст `:2384-2385`
называет `frames_received` на стороне зонда рабочим признаком доставки, прежде чем
упоминать гипотетический ack-кадр. Признак существует и работает:

```
$ grep -rn 'frames_received' --include=*.rs crates/ | grep -c wsprobe
9
$ grep -n 'frames_received' crates/gateway-serve/src/bin/wsprobe.rs | head -2
337:    frames_received: usize,
474:        frames_received: n_frames,
```

**Б-1 — ГЛАВНОЕ утверждение исправлено верно.** Ложь прошлого круга («в `spawn_blocking`
после `resume` ADD/SWITCH») снята; новый текст называет адреса и место правильно:

```
$ grep -n 'inc_successes' crates/gateway-serve/src/lib.rs
1477:                    metrics::inc_successes(inner.counters.as_ref());
1625:                metrics::inc_successes(inner.counters.as_ref());
2376:        // на :1625/:1477). ОБА `inc_successes` v1 стоят после
2389:        metrics::inc_successes(counters.as_ref());

$ sed -n '1475,1477p' crates/gateway-serve/src/lib.rs      # SWITCH
                    if sink.send(Message::Text(snap_text)).await.is_err() {
                        return Err("client disconnected during switch snapshot send".to_string());
                    }
                    metrics::inc_successes(inner.counters.as_ref());

$ sed -n '1622,1625p' crates/gateway-serve/src/lib.rs      # ADD
                if sink.send(Message::Text(snap_text)).await.is_err() {
                    return Err("client disconnected during snapshot send".to_string());
                }
                metrics::inc_successes(inner.counters.as_ref());
```

Адреса `:1477`/`:1625` точны, «после `sink.send(...).await` и ВНЕ `spawn_blocking`» —
верно (`.await` в sync-замыкании невозможен; замыкания `:1376`/`:1497` закрываются
ЗАДОЛГО до этих строк). Адреса `add_journal_payload_bytes` `:1405`/`:1555` тоже точны и
действительно внутри `spawn_blocking`. Внутреннее противоречие прошлого круга снято.

**Остаётся остаток в ТОЙ ЖЕ фразе — §6.**

## §6 — Б-3 (БЛОКЕР): последняя клауза фразы приписывает запрету v1 НЕ ТУ МЕТРИКУ

**Файл:** `crates/gateway-serve/src/lib.rs:2378-2379` (вершина `27df701`).

**Что написано:**

```rust
// после `resume` живёт `add_journal_payload_bytes` (:1555/:1405), и v1 на
// :1615-1621 прямо запрещает дублировать там `successes` (двойной счёт).
```

**Что на `:1615-1621` написано на самом деле — сырой текст с той же вершины:**

```
$ sed -n '1615,1621p' crates/gateway-serve/src/lib.rs
                // M-87 (задача 23, R-196 №5 / R-200 §B7): счётчик прочитанных байт
                // кормится ЧЕСТНЫМ `ReadStats.payload_bytes_read` из resume, а НЕ
                // `snap_text.len()` (длина ОТПРАВЛЕННОГО снимка). Метрика УЖЕ
                // инкрементирована внутри `spawn_blocking` — здесь
                // дублировать нельзя (см. комментарий в ADD-пути): это
                // двойной счёт и возврат к «растёт на ответе» через
                // косвенный путь.
```

Диапазон строк назван ВЕРНО, но субъект запрета — **счётчик прочитанных БАЙТ**
(`add_journal_payload_bytes`), а не `successes`. Слова `successes` в этом комментарии нет
вовсе; «Метрика» в нём — та, что введена первой строкой («счётчик прочитанных байт»).

**Почему это не буквоедство, а блокер — два следствия, оба проверяемы.**

1. **Клауза утверждает, что v1 запрещает ровно то, что v1 делает — в трёх строках ниже
   названного диапазона.** «Здесь» внутри цитируемого комментария v1 означает точку
   отправки (`:1622` `sink.send`), и именно ТАМ, на `:1625`, v1 `successes` и считает.
   Фраза нового комментария, прочитанная буквально, объявляет этот законный инкремент
   запрещённым — и противоречит собственной первой половине («ОБА `inc_successes` v1
   стоят после `sink.send(...).await`»), которую я в §5 признал верной. Это тот же
   дефект формы, что `R-218` Б-1: указатель ведёт в место, где правило обратно
   утверждаемому.

2. **Последствие поведенческое, а не стилистическое.** Сопровождающий, поверивший
   атрибуции, заключает, что `successes` уже инкрементирован внутри `spawn_blocking`, и
   снимает «дубль» — законный `metrics::inc_successes(counters.as_ref())` на `:2389`,
   единственный на legacy-пути. Это буквальный возврат `TD-228` (`TECH-DEBT.md:201`:
   «legacy-путь не инкрементирует `attempts`/`successes` НИ НА ОТКАЗЕ, НИ НА УСПЕХЕ» —
   MAJOR, прод-замер: `attempts:0, successes:0` при +583 МБ прочитанных байт), то есть
   отмена того самого дефекта, который `M-91` закрыл. Ложный комментарий здесь способен
   мотивировать удаление верного кода.

**ПРОИСХОЖДЕНИЕ КЛАУЗЫ — МОЙ ЖЕ ВЕРДИКТ. Фиксирую против себя, а не умалчиваю.**
`R-218` §6 писал: «комментарии v1 рядом с ним прямо запрещают трогать там успех:
"Метрика УЖЕ инкрементирована внутри `spawn_blocking` — здесь дублировать нельзя… это
двойной счёт" (`:1616-1621`)». Характеристика «запрещают трогать там УСПЕХ» в моём
вердикте была неточна — цитируемый комментарий говорит о счётчике байт. Исполнитель
перенёс мою формулировку в код добросовестно. Поэтому: (а) находка адресована фразе, а
не исполнителю; (б) мандат ниже даёт СЫРОЙ текст `:1615-1621`, чтобы следующий круг
опирался на файл, а не на мой пересказ; (в) `R-219` — место, где неточность `R-218`
исправлена явно.

**Почему всё равно БЛОКЕР, а не замечание с новой карточкой.** Предмет PR — достоверность
комментариев на этих самых строках; `TD-236` называет диапазон `:2372-2379` буквально.
Влить фразу с ложной атрибуцией и закрыть карточку — значит объявить долг закрытым в том
самом месте, где он остался. Это ровно то, что `R-218` §6 назвал «заменой закрытия долга
его переименованием». Объём правки — одна клауза; круг дешевле, чем ложное CLOSED в
реестре.

**Отличие от прошлого круга, которое обязан назвать честно:** состояние НЕ регресс
относительно базы. Главное утверждение стало верным и точным (§5), остаток — одна
приписка внутри верной фразы. Это улучшение с остатком, а не второй виток той же лжи;
блокирую по месту и последствию (п. 1-2), не по тяжести формулировки.

**Фикс не проектирую** (`gates.md` §4, граница reviewer↔architect). Факты для исполнителя:
субъект запрета на `:1615-1621` — `add_journal_payload_bytes`; к `successes` этот запрет
не относится; v1 считает `successes` на `:1477` (SWITCH) и `:1625` (ADD), обе точки — на
точке отправки, ВНЕ `spawn_blocking`. Формулировку выбирает исполнитель.

## §7 — N-1 (замечание о МАРШРУТЕ, адресовано мне): `R-218` вернул круг не той роли

`R-218` §11 написал «Возврат `engine-dev` (SVR-цикл, не self-fix у architect'а)». Профиль
на диске говорит обратное:

```
$ sed -n '48,52p' .claude/agents/reviewer.md
- REJECT/CHANGES REQUESTED → **`architect`**: разбор находки — его, не dev'а (решение
  founder'а 2026-09-04, зафиксировано в
  `docs/workflow/session-handover-2026-09-04.md` §4 п.2; охват уточнён им же 2026-09-05 ...). Он правит спеку/оракул сам либо через founder'а диспетчеризует
  dev на impl-правку. Прямой возврат dev'у ОТМЕНЁН: он чинил названное МЕСТО, а не КЛАСС,
  и следующий круг ловил ту же находку в новой одежде.
```

Охват правила — «по коду, идущему в прод», что ровно наш случай. Исход круга
подтвердил основание нормы буквально: место было починено, класс («комментарий
приписывает утверждение не той метрике») воспроизвёлся в новой одежде — §6. Этот круг
маршрутизирую `architect`'у, как велит профиль. Замечание адресовано мне, не исполнителю.

## §8 — Done Block (прогон на ДЕРЕВЕ СЛИЯНИЯ `953858f`)

```
$ git merge --no-edit origin/main        # merge-preview, gates.md §8
Merge made by the 'ort' strategy.
 ... rename {milestones => docs/archive}/M-91-legacy-serving-counters.md (100%)

$ git rev-parse HEAD
953858fe862bd0a2e28b9a85fa5d53ab79577c60

$ cargo fmt --all -- --check; echo exit=$?
exit=0

$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.73s
exit=0

$ cargo test -p gateway-serve 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=203 failed=0 (блоков: 46)
exit=0

$ cargo test -p ops 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=172 failed=0 (блоков: 19)
exit=0

$ grep -c FAILED <лог прогона>
0

$ bash scripts/check_archived_refs.sh
PASS  вынесенных артефактов найдено: 68
PASS  НОВЫХ висячих ссылок нет (проверено 68 артефактов)
PASS  базовая линия актуальна (унаследованных ссылок: 21)
VERDICT: PASS
exit=0
```

Гейты зелёные — и это ожидаемо, а не аргумент: правка comment-only, компилятор и тесты к
достоверности комментария слепы по построению (`git diff | grep -vcE "^[+-]\s*//"` → `0`).
Б-3 найден ЧТЕНИЕМ против файла, как и Б-1 прошлого круга; механического барьера на
истинность комментария у нас нет, и этот вердикт — единственное место, где он проверяется.

**Ярус C прочитан ГРЕПОМ (`reading-map.md` §2), называю предметы поиска:** `TECH-DEBT.md`
— `TD-236` (карточка, `:210`), `TD-228` (`:201`), `TD-220` (`:195`), `TD-237`;
`PROJECT-STATE.md` — `TD-236`, `M-91`. Целиком эти файлы не читал и не мог: 1 015 KB.

## §9 — ВЕРДИКТ

**REJECT (CHANGES REQUESTED).** Возврат — `architect` (§7).

Условие APPROVED на следующем круге:

1. **Б-3 устранён** — клауза на `:2378-2379` называет субъектом запрета `:1615-1621` тот
   счётчик, о котором этот комментарий говорит (`add_journal_payload_bytes`), либо
   клауза снята; утверждение, что v1 запрещает `successes` на точке отправки, из файла
   уходит — v1 именно там его и считает (`:1477`, `:1625`).
2. **Ветка синхронизирована с `origin/main`** (сейчас отстаёт на три коммита) либо гейты
   снова прогнаны на дереве слияния: `strict: false`, `gates.md` §8.

Что ПРИНЯТО и повторной проверки не требует: §1 (scope чист, comment-only подтверждено
прогоном), §2-§3 (Block-C и Block-risk неприменимы, основания названы), §5 полностью —
Б-2 снят порядком merge'а, N-3 и N-4 закрыты верно, главное утверждение Б-1 исправлено
точно, §8 (fmt/clippy/тесты/archived-refs зелёные на дереве слияния `953858f`).

`PROJECT-STATE.md` и `TECH-DEBT.md` не обновляю: `TD-236` остаётся **OPEN**, новой
карточки не завожу — остаток зафиксирован здесь, в §6, по норме «не переименовывать долг
вместо закрытия». Неточность `R-218` §6, породившая Б-3, исправлена в §6 этого файла.
