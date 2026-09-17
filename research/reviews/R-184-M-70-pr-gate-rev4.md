<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: 7b16eec266e841c439a9f2d15470cfd5134accf1
audited_head: c49fbc46da10acb3089ed0ad45ccb0785556d7e1
verdict: REJECT
-->

# R-184 — M-70 (включение полос глубины), PR-гейт круга 4: **REJECTED (merge отказан)**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-09-17T19:22Z
**Предмет:** `7b16eec..c49fbc4` на `origin/docs/M-70-rev2` — всё, что легло поверх ревизии,
судимой `R-172`.
**Мандат:** вердикт tester'а PASS (8/8 команд, `verify_M-70.sh` exit=0, 994/994).
**Предыдущие круги:** `R-170` (маршрут), `R-171` (REJECT: форма ломала `VB-I-4`),
`R-172` (REJECT: `DB-I-4c` не сторожит эвикцию; задачи 6/7 не исполнены; ложный exit-код).

**Главное одной строкой.** **ВСЕ ТРИ БЛОКЕРА `R-172` ЗАКРЫТЫ, и каждый предъявлен МОИМ
прогоном, а не отчётом** — включая мутационный контроль, которым `R-172` и убил прошлый круг.
Инженерных претензий к работе у меня НЕТ: гейт 42 PASS / 0 FAIL, `exit=0`, воспроизведён в
чистом дереве. **Merge всё равно отказан, и ни одна причина не является дефектом кода:**
merge в `main` ⇒ деплой ⇒ **включение полос пользователям без подписи founder'а** (граница C,
Б-1), и ревизии спеки `rev10`/`rev11`, уезжающие тем же merge'ем, **не прошли круг критика,
который спека требует от себя сама** (Б-2).

---

## Block-scope — PASS

Разделение зон проверено ПОКОММИТНО по ветке (коммиты, пришедшие из `main`, исключены —
`--not origin/main`), а не по итоговому диффу:

```
$ for c in $(git log 7b16eec..HEAD --format='%h' --not origin/main); do ... git show --numstat ...
--- c8e9f4a [architect]   milestones/M-70-depth-bands-enablement.md
--- af16be5 [architect]   milestones/M-70-depth-bands-enablement.md
--- 8562a06 [architect]   milestones/M-77-frame-book-continuity.md (снятие дубля)
--- 8f1bfe3 [architect]   milestones/M-70-depth-bands-enablement.md
--- d386aaa [architect]   crates/gateway/tests/red_checkpoint_bin_prod_argv.rs
--- ce592f3 [engine-dev]  docker-compose.yml
--- 5a22a82 [architect]   crates/gateway/tests/{red_depth_point_provenance,red_gateway_schema_version}.rs
                          docs/fa/viz-backend.md
--- 39b78c4 [engine-dev]  docker-compose.yml
--- 09a7fdf [engine-dev]  crates/gateway/src/lib.rs
--- 2f54595 [architect]   crates/gateway/tests/*.rs + milestones/M-70-*.md + scripts/verify_M-70.sh
--- 622117a [engine-dev]  merge origin/main (M-77)
--- c49fbc4 [engine-dev]  merge origin/main (M-85)
```

**Ни один коммит engine-dev'а не трогает `*/tests/**`** — sacred-спеки правит только architect
(`5a22a82`, `2f54595`, `d386aaa`). **Ни один коммит architect'а не трогает `crates/*/src/**`.**
`docs/fa/viz-backend.md` (`5a22a82`) — зона architect'а и уже прошла СВОЙ круг `gates.md` §9
(`C-210`, NOTE, `audited_head=5a22a82`).

Чистый диф предмета против `main` — ровно `Allowed paths` спеки §2:

```
$ git diff --numstat origin/main...HEAD | sort -k3
308/93   crates/gateway/src/lib.rs
15/2     docker-compose.yml
4/3      docs/fa/viz-backend.md
978/24   milestones/M-70-depth-bands-enablement.md
365/0    scripts/verify_M-70.sh
+ 12 файлов crates/{gateway,gateway-serve}/tests/** и 7 вердиктов research/**
```

Процессный слой (`gates.md` §11) в чистом дифе НЕ тронут: `.claude/**`, `CLAUDE.md`,
`docs/04-workflow.md` отсутствуют. Проверено барьером, а не глазами:

```
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0
```

## Block-DoneBlock — PASS. Класс `R-172` Б-3 НЕ ПОВТОРИЛСЯ

`R-172` Б-3 убил прошлый круг за `exit=0` в отчёте при `exit=1` у гейта. В этот раз число
отчёта совпало с прогоном — проверено в СВОЁМ дереве (`/tmp/hft-reviewer-m70-r4`, detached
`c49fbc4`), без переиспользования чужого `target/`:

```
$ bash scripts/verify_M-70.sh > /tmp/r4-verify.out 2>&1; echo "VERIFY_EXIT=$?"
VERIFY_EXIT=0
$ grep -c '^PASS' /tmp/r4-verify.out; grep -c '^FAIL' /tmp/r4-verify.out
42
0
$ tail -2 /tmp/r4-verify.out
VERDICT: PASS
VERIFY_EXIT=0
```

**Отдельно об отличии от прогона tester'а.** Tester гонял с `target/`, симлинкнутым в дерево
engine-dev'а. Это не подлог — SHA совпадали, — но и не независимый прогон: артефакты сборки
брались из дерева проверяемого. Моё дерево собиралось своё. Числа сошлись поимённо
(42 PASS / 0 FAIL), значит замечание остаётся методологическим, а не находкой.

## Block-C — N/A, и это ПРЕДЪЯВЛЕНО шагом гейта, а не заявлено

`crates/contracts/**` и `docs/rfc/**` не тронуты — contract-RFC не требуется:

```
PASS: git diff --name-only c564fa0..HEAD -- crates/contracts docs/rfc | grep -q . && exit 1 || exit 0
```

## Block-risk — N/A по `gates.md` §5

Диф не трогает `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**`.
Предмет — read-only выдача (`gateway`/`gateway-serve`), order-egress отсутствует как класс.
Состав ЗАПИСИ не тронут, и это тоже шаг гейта, а не моё слово:

```
PASS: git diff --name-only c564fa0..HEAD -- crates/venue-binance crates/venue-binance-futures crates/journal | grep -q . && exit 1 || exit 0
```

`risk-critic` не требуется (спека §7, и я с этим согласен).

## Предъявление FA (`gates.md` §4, M-66) — живые инварианты `docs/fa/viz-backend.md`

Диф трогает `crates/gateway/src/**` и `crates/gateway-serve/tests/**`. FA обоих крейтов —
`docs/fa/viz-backend.md` (`scripts/check_review_fa.sh:190-198`: `gateway`→префикс `VB`,
`gateway-serve`→`GS`). Живые ID, относящиеся к предмету, названы и проверены на этой ревизии:

- **`VB-I-4`** — форма выдачи меняется ТОЛЬКО вместе с bump'ом версии схемы. Это тот инвариант,
  которым `R-171` убил первую форму задачи 4 и которым `R-172` держал блокер Б-2. Обе половины
  исполнены: v1-потребитель читает (аддитивная форма, `series_provenance` рядом с `series`), и
  версия поднята 9→10 (`09a7fdf`), sacred-пин согласован (`5a22a82`).
- **`VB-I-2`** (`live == replay`) — именно он стоит за новым стражем `db_i_4d` (см. Б-1 ниже) и
  за чужим `gw_i_4`, державшим ветку красной до `M-85`.
- **`VB-I-10`** (bounded-window snapshot) — запрет §2.1 спеки «предел выдачи не куплен ценой
  предела памяти»; шаг гейта исполнил существующие `red_gateway_bounded`/`red_snapshot_noclone`
  (2 теста, `PASS`).
- **`GS-I-1`** (`gateway-serve` — тонкая read-only IO-оболочка, stateless) — диф в этом крейте
  ограничен тестом доставки `red_depth_bands_delivery.rs`, прод-код транспорта не тронут.

**Ярус C предъявляю грепом, как требует `reading-map.md` §2** — я эти файлы пишу, поэтому
называю, что искал: `TD-159`, `TD-161`, `TD-198`, `TD-204`, `TD-205` в `TECH-DEBT.md`
(строки 107, 108, 88, 144, 145); `M-70` в `docs/ROADMAP.md` (строка 95); индекс `П-` в
`docs/PENDING-SIGNATURE.md` + тела `П-014`, `П-027`, `П-029`. `PROJECT-STATE.md` целиком не
читал и не утверждаю, что читал.

---

# Закрытие находок `R-172` — каждая предъявлена МОИМ прогоном

## `R-172` Б-1 — ЗАКРЫТ. Мутация воспроизведена ревьюером, страж краснеет

`R-172` установил замером: `DB-I-4c` зелен против реализации, где оконная эвикция режет
`series`, не трогая `series_provenance`. Architect закрыл находку НЕ предложенной мною
развязкой (он показал, что сторожить длины на merge-пути бессмысленно — `zip` их
самовосстанавливает, разрушая данные), а новым стражем СВЯЗКИ `db_i_4d`. **Я это не принял
на слово — я повторил ту же мутацию у себя:**

```
$ # crates/gateway/src/lib.rs, evict_series_bundle_under_window (:2780-2790) заменено на:
$ #     row.series.retain(|pt| pt.0 >= lo_time_s);   // series_provenance НЕ тронут
$ cargo test -p gateway --test red_depth_point_provenance 2>&1 | tail -3
test db_i_4d_point_to_label_binding_survives_merge_and_eviction ... FAILED
test result: FAILED. 3 passed; 1 failed
```

Сырое расхождение, которое печатает страж:

```
left  (клиент snapshot+frames): [... (1752000011, Some("not-observed band=0.030000 reach=0.013000")), ...]
right (полный реплей):          [... (1752000011, Some("diff-reconstructed, liveness=confirmed")), ...]
```

Метка уехала на чужую точку — ровно то, что `DB-I-4c` пропускал. **Возврат кода проверен:**

```
$ cp /tmp/r4-lib.rs.bak crates/gateway/src/lib.rs && git status --porcelain
{пусто}
$ cargo test -p gateway --test red_depth_point_provenance 2>&1 | tail -3
test result: ok. 4 passed; 0 failed
```

Находка закрыта **лучше**, чем требовал мой же вердикт: предложенная мною развязка не
работала, и это доказано, а не отговорено.

## `R-172` Б-2 — ЗАКРЫТ. Задачи 6 и 7 исполнены, `VB-I-4` удовлетворён

```
PASS: task #6 версия схемы поднята 9 → 10                      (09a7fdf)
PASS: task #6b sacred-пин версии согласован с константой (10)   (5a22a82)
PASS: task #7 канонический набор из семи полос объявлен записью в блоке gateway-serve  (39b78c4, ce592f3)
```

## `R-172` Б-3 — ЗАКРЫТ. См. Block-DoneBlock: `exit=0` отчёта = `exit=0` прогона

## `R-172` Н-1 (`TD-199`) — ЗАКРЫТ ДВУМЯ ЧУЖИМИ ПРЕДМЕТАМИ, проверено на дереве слияния

`M-77` починил прод-путь (`pump`), `M-85` — `frames_since` (`seed_book`). Оба в `main`
(`b008f5e`, `3f399ec`). Красное, державшее ветку, снято — и это мой прогон, а не пересказ:

```
$ cargo test --all --no-fail-fast 2>&1 | grep -E "^test .* FAILED"
{пусто}
PASS: cargo test --all --quiet
```

Слияние сохранило ОБЕ чужие работы, проверено счётом, а не доверием к merge-коммиту:
`grep -c book_series_in` → 11 на ветке против 10 в `main`; `grep -c seed_book` → 3 и 3.

## `R-172` Н-2 (`VB-I-5` в FA) — ЗАКРЫТ кругом `C-210` (NOTE, `audited_head=5a22a82`)

---

# Находки круга 4 — обе ВНЕ кода, и ни одна не адресована dev'у

## Б-1 — БЛОКЕР. Merge = ВКЛЮЧЕНИЕ полос на проде. Это граница C, подписи нет

Спека называет это сама (§3sexies (а)), и я это ПЕРЕПРОВЕРИЛ, а не процитировал.

**Что меняет merge (замер на дереве):**

```
$ grep -n 'GATEWAY_BANDS\|--bands' docker-compose.yml          # ветка
141:      GATEWAY_BANDS: ${GATEWAY_BANDS:-0.015,0.03,0.05,0.08,0.15,0.3,0.6}
230:      - --bands=${GATEWAY_BANDS:-0.015,0.03,0.05,0.08,0.15,0.3,0.6}
$ git show origin/main:docker-compose.yml | grep -n 'GATEWAY_BANDS\|--bands'
136:      GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001}
217:      - --bands=${GATEWAY_BANDS:-0.001}
```

**Что стоит на проде СЕЙЧАС (ssh, а не память):**

```
$ ssh … 'cd /root/hft-platform && grep -n GATEWAY_BANDS .env || echo "(нет в .env)"'
(нет в .env)
$ ssh … 'docker inspect hft-gateway-serve --format "{{range .Config.Env}}{{println .}}{{end}}" | grep -i bands'
GATEWAY_BANDS=0.001
```

Переопределения на сервере НЕТ ⇒ прод берёт дефолт из compose ⇒ **merge в `main` ⇒ Deploy ⇒
семь полос включены пользователям одним действием, без отдельного решения.** Зазора между
«влить работу» и «включить на проде» в сегодняшней конструкции не существует.

**Почему это не инженерный вопрос и почему его не может закрыть ни architect, ни я.**
`gates.md` §0.1: «арбитр решает, как ПРАВИЛЬНО; founder решает, чего мы ХОТИМ» — состав
выдаваемых данных и момент включения отнесены к границе C прямо. `П-014` даёт ПРАВО включить,
но не назначает МОМЕНТ, и это записано в самой подписи.

**Более того — живая founder-запись называет включение ЗАБЛОКИРОВАННЫМ, и не только по
`TD-159`.** `П-029` (РЕШЕНИЕ FOUNDER'А 2026-09-10), раздел «Цена, названная заранее», п. 4 —
**ДВА живых предусловия включения**:

> · **`TD-159` OPEN, MAJOR** … блокирует `П-014` п.4 и входит в `M-70` задачей 4;
> · **БАРЬЕРА, УДЕРЖИВАЮЩЕГО ЭМИССИЮ ДО ВОССТАНОВЛЕНИЯ ГЛУБИНЫ, НЕТ** — и это второе живое
>   предусловие, **дороже первого**.

Первое этот merge закрывает (задача 4 зелена, `TD-159` закрою close-out'ом). **Второе — нет.**
Спека §3sexies (б) аргументирует, что половина мотивировки устарела (метка больше не молчит —
и это правда, я проверил: `db_i_4*` зелены), и выносит остаток отдельным милестоуном, которого
ещё не существует. **Аргумент инженерно сильный, но предусловие поставил founder, и снять его
может только founder.** Тем же решением §3sexies (б) оставляет `docs/fa/viz-backend.md:158-162`
нетронутым — то есть FA на дереве слияния ПО-ПРЕЖНЕМУ говорит «RED-спека на это — предусловие
включения». Влить работу, включающую полосы, поверх документа, называющего включение
заблокированным, нельзя.

**Развязки, названные спекой (§3sexies (а)), обе дешёвые — выбор не мой:**

| # | развязка | цена |
|---|---|---|
| A | подпись founder'а на включение вместе с merge'ем | ноль работы; деплой-гейт `gates.md` §8 в усиленной форме (§5 спеки: `RssAnon` до/после, размер кадра на проводе, sanity метки) |
| B | прод-дефолт вернуть к `0.001`, канонический состав держать в оракулах и включить отдельным действием | одна строка `docker-compose.yml` (engine-dev) + шаг гейта task #7 переписать с «объявлен записью» на «объявлен, но не дефолт» (architect) |

**Эскалация:** `gates.md` §0.1 — арбитру НЕ передаётся, идёт founder'у. Это мой Handoff §D.

## Б-2 — БЛОКЕР. `rev10`/`rev11` уезжают в `main` без круга критика, которого спека требует ОТ СЕБЯ

`gates.md` §9: критик обязателен, когда правка документа меняет ФОРМУ. Спека это про себя
объявила и не исполнила (§3sexies (г), дословно):

> Раздел §3sexies — НЕ изложение: он сужает границу предмета и выносит вопрос founder'у, то
> есть меняет ФОРМУ. Значит rev10 идёт **кругом критика по `gates.md` §9 перед любым
> merge'ем**, и это объявлено здесь, а не обнаружено барьером.

`rev11` пошла дальше `rev10`: она СНЯЛА её главное утверждение как ложное и переназначила
предмет-блокер. Замер — критика по этим ревизиям нет:

```
$ git log af16be5..HEAD --name-only -- research/critiques/
C-223, C-224, C-225   ← все три по M-85, не по M-70
$ sed -n '1,7p' research/critiques/C-210-M-70-fa-vb-i-5.md
audited_head: 5a22a824e926b46c518c2690eeb2ebc3b20fbb3e     ← ДО rev10 (af16be5) и rev11 (c8e9f4a)
```

Предмет уезжает в `main` этим же merge'ем — `milestones/M-70-depth-bands-enablement.md`,
978 вставок / 24 удаления в чистом дифе. Правка на 978 строк, которая сама себя назвала
изменением формы, входит в `main` без гейта, который она себе назначила.

**Это не формальность, и цена уже платилась на этом же предмете:** `rev10` продержалась в
`main`-мандатах с ложным заглавным утверждением, и её снимала `rev11` — то есть ошибка ревизии
спеки успела развернуть работу следующей сессии. Ровно этот класс круг критика и ловит.

## Н-1 — NOTE. `FA-WAIVER` вердикта tester'а выписан НА НЕСУЩЕСТВУЮЩЕЕ ОСНОВАНИЕ

Tester написал: «`FA-WAIVER: crates/gateway — … docs/fa/gateway.md отсутствует`». **Барьер так
не считает, и это проверяемо:**

```
$ sed -n '190,198p' scripts/check_review_fa.sh
    gateway)        FA_OF["${c}"]="docs/fa/viz-backend.md"; PFX_OF["${c}"]="VB"; LIVE_CRA+=("${c}")
    gateway-serve)  FA_OF["${c}"]="docs/fa/viz-backend.md"; PFX_OF["${c}"]="GS"; LIVE_CRA+=("${c}")
```

`gateway` — **НЕ** NO-FA-крейт: его FA — `docs/fa/viz-backend.md`, живых `VB-I-*` там
одиннадцать. Waiver для него не только не нужен, но и не сработал бы: барьер требует waiver
только для крейтов из списка NO-FA (`recorder`, `derive`), а для живых — НАЗВАННЫЙ ID.
Вреда waiver не нанёс (ветка не мержилась), но как факт о коде он ложен. Класс — утверждение
о механизме без открытия механизма (`reading-map.md` §3). Верная форма предъявления —
Block выше в этом вердикте.

## Н-2 — NOTE. `MAX_BANDS = 32` (задача 3) и `П-029` — разные миры, и это надо назвать ДО close-out'а

`П-029` (founder, 10.09) п. 3 цены: «**Предел числа полос перестаёт быть нужен** и `TD-198`
закрывается не заплаткой, а исчезновением предмета: клиент больше не задаёт число. Прежний
план (поставить верхний предел) **отклонён** founder'ом в пользу этого решения».

Задача 3 `M-70` ставит ровно тот предел (`MAX_BANDS = 32`). **Противоречия сегодня НЕТ, и вот
замер, почему:** сетку по-прежнему присылает клиент —

```
$ grep -n 'bands' crates/gateway-serve/src/session.rs | head -4
75:    if sel.bands.is_empty() { return Err("bands is empty (CT-RFC-09 §2.7)") }
79:    for b in &sel.bands { … }
```

Фиксированный серверный набор `П-029` — предмет `M-84`, которого в `main` нет. Пока клиент
задаёт число, гвард `MAX_BANDS` защищает реальную дыру (`TD-198`: 4096 полос = 14 МБ и 18.13 с
работы сервера ради отказа). **Что от этого следует:** `TD-198` при close-out'е закрывается
как «закрыт гвардом `M-70`», а не «исчез по `П-029`», и в карточке называется, что с приходом
`M-84` гвард станет избыточным, а не неверным. Пишу это здесь, чтобы close-out не выдал одно
за другое.

## Н-3 — NOTE. `TD-161` этим merge'ем закрывается ЧАСТИЧНО, и остаток — не код

Задача 5 зелена (4 теста, голая строка `"diff-reconstructed"` из кода убрана — шаг гейта
`PASS`). Остаток карточки — вторая цитата снятого литерала `validated<=1.3%` в
`docs/fa/viz-backend.md` (блок прошлого решения, без пометки «снято»). Зона architect,
маршрут `gates.md` §9. Карточку при close-out'е переведу в частично-закрытую с явным
остатком, а не закрою целиком.

---

# Условие APPROVED — ровно два пункта, оба вне кода

1. **Founder отвечает на §3sexies (а)** — развязка A (подпись на включение вместе с merge'ем)
   или B (вернуть прод-дефолт `0.001`, включать отдельным действием). При A — деплой-гейт
   `gates.md` §8 в усиленной форме §5 спеки. При B — одна строка compose (engine-dev) + шаг
   гейта (architect), и merge проходит без подписи.
   **Если выбрана A — отдельно нужен ответ по второму предусловию `П-029` п. 4** (барьер
   удержания эмиссии): снимается ли оно доводом §3sexies (б), или включение ждёт его.
2. **Круг критика `gates.md` §9 по `rev10`+`rev11`** — вердикт `C-NNN` на ветке. `rev11` сняла
   главное утверждение `rev10` как ложное; предмет круга — обе ревизии вместе, не последняя.

Ни один пункт не возвращается engine-dev'у: его работа принята целиком.

---

## Done Block

```
$ pwd; git log -1 --oneline
/tmp/hft-reviewer-m70-r4
c49fbc4 merge(M-70): влить origin/main — M-85 закрывает TD-204, гейт зелёный [engine-dev]

$ git status --porcelain
{пусто — мутация возвращена}

$ bash scripts/verify_M-70.sh > /tmp/r4-verify.out 2>&1; echo "VERIFY_EXIT=$?"
VERIFY_EXIT=0
$ grep -c '^PASS' /tmp/r4-verify.out; grep -c '^FAIL' /tmp/r4-verify.out; tail -2 /tmp/r4-verify.out
42
0
VERDICT: PASS
VERIFY_EXIT=0

$ cargo test -p gateway --test red_depth_point_provenance 2>&1 | tail -3     # мутация ЖИВА
test result: FAILED. 3 passed; 1 failed
$ cargo test -p gateway --test red_depth_point_provenance 2>&1 | tail -3     # код возвращён
test result: ok. 4 passed; 0 failed

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0

$ gh run list --branch main --limit 2
completed success Deploy to VPS   main workflow_run 35216646089
completed success Merge PR #188   CI   main push    35215686567
$ ssh … 'git rev-parse --short HEAD; docker ps --format "{{.Names}} {{.Status}}"'
3f399ec
hft-gateway-serve Up 9 hours (healthy)
hft-recorder      Up 9 hours (healthy)
```

## Cross-references

- `research/reviews/R-172-M-70-pr-gate-rev3.md` (круг 3, три блокера — все закрыты выше)
- `research/critiques/C-210-M-70-fa-vb-i-5.md` (§9 по `VB-I-5`, `audited_head=5a22a82`)
- `milestones/M-70-depth-bands-enablement.md` §3sexies (а)/(б)/(г), §2bis.-3
- `docs/PENDING-SIGNATURE.md` `П-014` (право включить), `П-029` п. 4 (два живых предусловия)
- `docs/fa/viz-backend.md` `VB-I-2`/`VB-I-4`/`VB-I-10`, `GS-I-1`, строки 42 и 150-162
- `TECH-DEBT.md` `TD-159`, `TD-161`, `TD-198`, `TD-204`, `TD-205`
- `.claude/rules/gates.md` §0.1 (граница C — founder), §4 (PR-гейт), §8 (деплой), §9 (документы)
