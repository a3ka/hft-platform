<!-- GATE-META
milestone: TD-239
audited_repo: a3ka/hft-platform
audited_base: 947991c46b44b8c9fb74d785a32627fe0ff3572b
audited_head: 7bbebf9da4f2df5b3b23e20933d83c8fc95016e1
verdict: APPROVE
-->

# R-234 — TD-239 runbook §0a, круг 3 (по `A-047` §4): APPROVE

**Роль:** reviewer. **Дата:** 2026-10-04. **Предмет:** ветка `origin/fix/TD-239-watchdog-runbook`,
коммит круга 3 `7bbebf9` (engine-dev, один файл `deploy/README.md`). Решение арбитра, по которому
судится круг, — `research/arbitration/A-047-td-239-runbook.md` §4 (открыт целиком).

## Вердикт

**APPROVE.** Все три правки `A-047` §4 сделаны ровно в указанных местах, каждое новое утверждение о
коде сверено мной ОТКРЫТИЕМ источника на вершине ветки (не по таблице арбитра и не по пересказу
dev'а). Новых несуществующих утверждений правка не вносит. Объём не превышен.

## Предмет — вершина взята командой (`04-workflow.md` §2)

```
$ git fetch origin -q && git rev-parse origin/fix/TD-239-watchdog-runbook origin/main
7bbebf9da4f2df5b3b23e20933d83c8fc95016e1      ← совпадает со справкой мандата (7bbebf9)
e4cab872344282dcc5c8ade0c6394095f1d049b4
$ git merge-base origin/main origin/fix/TD-239-watchdog-runbook
947991c46b44b8c9fb74d785a32627fe0ff3572b
$ git log --oneline origin/main..origin/fix/TD-239-watchdog-runbook
7bbebf9 docs(TD-239): runbook §0a — круг 3 по A-047 §4 [engine-dev]
62585e1 docs(arbiter): A-047 — TD-239 runbook: находки R-231 верны; круг 3 — …
06aeb95 docs(review): R-231 — TD-239 rev2 REJECT: …
3db4615 docs(TD-239): runbook §0a — сверено с кодом по R-227 [engine-dev]
49c9cc0 docs(review): R-227 — TD-239 REJECT: …
eea45db docs(TD-239): runbook ALERT сторожа — реальные маркер/лог/место сборки [engine-dev]
```

## Block-scope

```
$ git show --numstat --format='' 7bbebf9
24	14	deploy/README.md
$ git diff --stat origin/main...origin/fix/TD-239-watchdog-runbook
 deploy/README.md                                   |  83 +++++-
 research/arbitration/A-047-td-239-runbook.md       | 303 +++++++++++++++++++++
 research/reviews/R-227-TD-239-watchdog-runbook.md  | 147 ++++++++++
 .../reviews/R-231-TD-239-watchdog-runbook-rev2.md  | 174 ++++++++++++
$ git diff --name-only origin/main...HEAD | grep '^crates/'; echo rc=$?
rc=1
$ git diff --stat 947991c origin/main -- deploy/README.md .github/workflows/deploy.yml deploy/bin/install-watchdog.sh
(пусто — за время жизни ветки источники в main не менялись; номера строк на дереве слияния те же)
```

Коммит круга 3 трогает ТОЛЬКО `deploy/README.md`, два ханка (`:130-133`, `:145-164`), оба внутри §0a
и внутри мест, названных `A-047` §4. Разрешённые арбитром «оставить без изменений» участки
(`:37-129`, `:134-144` до «и, наконец», `:165-179`) диффом не задеты. Запрет §4 соблюдён:
`scripts/**`, `.github/**`, `TECH-DEBT.md`, `Dockerfile` (N-4) — не тронуты. Зона engine-dev —
`deploy/**` (`scope-guard.md`). Block-C — N/A (контрактов нет). Block-risk — N/A: ни `risk`,
ни `killswitch`, ни `oms`, ни `venue-*` в диффе нет, risk-critic не требуется.

**FA.** `crates/**` не тронут ⇒ барьер `check_review_fa.sh` даёт SKIP. Ближайшая FA —
`docs/fa/ops.md`; правка runbook'а её инвариантов (`OPS-I-5`, `OPS-I-11`) не касается.
Строку `FA-WAIVER` dev положил в ТЕЛО КОММИТА — это прежняя ложная норма (`TD-165`, барьер ищет её
в review-файле); здесь безвредно, т.к. waiver не требуется вовсе.

## Сверка трёх правок — открытием источника

Открыто на вершине `7bbebf9`: `.github/workflows/deploy.yml:295-317`, `deploy/bin/install-watchdog.sh:40-106`,
`deploy/README.md:70-96,105-179`.

### (1) `README:130-133` — поиск `DEPLOY FAILED` по префиксу — ВЕРНО

```
$ sed -n 308p .github/workflows/deploy.yml
              echo "=== DEPLOY FAILED — logs + rollback to $(git rev-parse --short "$PREV") ==="
$ grep -nF '=== DEPLOY FAILED ===' deploy/README.md; echo rc=$?
rc=1
$ grep -noE '=== [^=`]+' deploy/README.md | grep 'DEPLOY'
122:=== DEPLOY FAILED — logs + rollback to … 
130:=== DEPLOY FAILED'
131:=== DEPLOY FAILED — logs +
157:=== DEPLOY FAILED — logs + rollback to <sha> 
```

`grep -F '=== DEPLOY FAILED'` — префикс строки `:308`, совпадает дословно; цитата полной формы
(`:131-132`, `:157`) совпадает с `:308` с точностью до подстановки `<sha>`. Дословного
несуществующего маркера в runbook'е больше нет (`R-231` N-1 закрыт).

### (2) `README:145-149` — источники stderr установщика — ВЕРНО

```
$ nl -ba deploy/bin/install-watchdog.sh | sed -n '57p;65,73p;78p;83p;88,89p;105,106p'
    57	mkdir -p "$(dirname "${DST}")"
    65	cleanup() {
    67	  [ -n "${CID}" ] && docker rm "${CID}" >/dev/null 2>&1 || true
    69	    [ -e "${TMP}" ] && rm -f "${TMP}" >/dev/null 2>&1 || true
    73	trap cleanup EXIT
    78	IMG="$(docker inspect -f '{{.Image}}' "${CONTAINER}")" || exit 1
    83	CID="$(docker create "${IMG}")" || exit 1
    88	if ! docker cp "${CID}:/usr/local/bin/ops-watchdog" "${TMP}"; then
    89	  exit 1
   105	chmod 0755 "${TMP}"
   106	mv -f "${TMP}" "${DST}"
```

«ТОЛЬКО» снято; перечислены все шесть команд, способных печатать stderr при отказе, с верными
номерами строк; trap действительно глушит оба своих вызова в `/dev/null`. Отдельно проверено
место, которое правка НЕ называет, но которое могло бы опровергнуть правило «пусто ⇒ пустой
файл»: промежуточный `docker rm` (`:95`) тоже уведён в `/dev/null 2>&1`; `inspect`/`create`
захвачены `$(…)` (stdout в лог не идёт). Шумящих команд вне перечня нет (`R-231` N-2 закрыт).

### (3) `README:150-164` — правило «пустого файла» разнесено по веткам — ВЕРНО

```
$ nl -ba .github/workflows/deploy.yml | sed -n '301p;305p;307,316p'
   301	              echo "=== healthy (recorder + gateway-serve) — deployed $(git rev-parse --short HEAD) ==="
   305	              bash deploy/bin/install-watchdog.sh || { echo "=== WATCHDOG INSTALL FAILED ===" >&2; exit 1; }
   307	            else
   308	              echo "=== DEPLOY FAILED — logs + rollback to $(git rev-parse --short "$PREV") ==="
   309	              docker logs hft-recorder --tail 50 || true
   310	              docker logs hft-gateway-serve --tail 50 || true
   311	              git reset --hard -q "$PREV"
   312	              docker compose up -d --build recorder gateway-serve
   315	              bash deploy/bin/install-watchdog.sh || true
   316	              exit 1
```

- **`then`:** между `:301` и `:305` нет иных команд (`:302-304` — комментарии) ⇒ всё, что в логе
  стоит между двумя маркерами, — вывод установщика; пусто ⇒ `install-watchdog.sh:100-101`. Верно.
- **`else`:** порядок `:308 → :309-310 → :311 → :312 → :315 → :316` описан точно; маркера после
  `:315` нет; вывод «исход установки из лога Actions не определим» — верен (успех и `:100-101`
  оба молчат). Ссылок на строки `install-watchdog.sh` в абзаце `else` нет — как требовал `A-047`
  §4 (после `:311` исполняется установщик дерева `$PREV`). Отсылка к проверке доставки
  «(1)-(2) (`:74-82`)» сверена: `README:74` — `# (1)`, `:82` — конец комментария `(2)`. Отсылка к
  «п.4 (ручной перезапуск)» — `README:171-178`, существует. `R-231` B-1 закрыт.

Примечание, не находка: фраза `:132-133` «прежде чем разбирать установку, проверьте п.2»
слегка смещает смысл прежней «ищите … в логах (п.2)» — п.2 говорит, ГДЕ лог, а не что проверить.
Оператора не дезинформирует (п.2 следующий по порядку и о том же логе), правки не требует.

## Block-DoneBlock — перепрогон мной (dev'у не верю на слово)

```
$ git -C /tmp/hft-reviewer-td239-r3 rev-parse HEAD
7bbebf9da4f2df5b3b23e20933d83c8fc95016e1
$ grep -nF '=== DEPLOY FAILED ===' deploy/README.md; echo rc=$?
rc=1
$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E "^(PASS|FAIL|SKIP|DELIVERY|VERDICT)"; echo exit=${PIPESTATUS[0]}
PASS  D1 Dockerfile собирает и копирует journal-retention в прод-образ
PASS  D2 recorder в образе цел (доставка ретеншена не сломала сбор)
SKIP  D1-deep (сборка образа) — включается HFT_DELIVERY_DEEP=1 (CI-job + §8 на VPS)
PASS  D3 docker-compose монтирует холодное хранилище в /cold
PASS  D4 сервис journal-retention объявлен под ops-профилем
PASS  D5a настоящий journal-retention ПРИНИМАЕТ argv задания (dry-run отработал, exit=0)
PASS  D5 cron-юнит РЕАЛЬНО устанавливается (crontab -n), задание алертит и пробрасывает exit≠0 (проверено прогоном со стабом, не грепом)
PASS  D7 компакция РЕАЛЬНО вызывается из доставленного бинаря (--mode compact) и стоит в cron — дедлайн диска двигается фактом, а не тестом
PASS  D6 deploy/README описывает монтирование холодного хранилища и включение Apply
PASS  D8 journal-retention: compose command:-форма разобрана настоящим бинарём
PASS  D8 journal-compaction: compose command:-форма разобрана настоящим бинарём
PASS  D9 cron-задания пишут позитивный *.last-success — silent-absence детектируется по свежести маркера (не только сбой)
DELIVERY: PASS
exit=0
```

Done Block dev'а сверен с перепрогоном: совпадает. Предел назван: `verify_delivery_M-08.sh` к
истинности §0a слеп по построению (`R-227` §«Класс», `A-047` §2) — PASS здесь значит «доставка не
сломана», а не «runbook верен». Истинность установлена чтением с открытием источников выше.

**RED-first / sacred.** Диф не трогает `*/tests/**`, оракулов не переписывает — N/A.
**Атомарность.** Один коммит на одну задачу (круг 3), subject со ссылкой на `TD-239` и `A-047` §4,
без co-author трейлера.

## Условие и обязательства close-out (`A-047` §2 п.2)

1. `TD-239` закрывается этим merge'ем (runbook больше не ссылается на несуществующее).
2. Заводится карточка остаточного риска: утверждения runbook'ов о поведении кода, не выраженные
   маркером, оракула не имеют; маркерная половина класса — harness-track architect'а по `A-047`
   §2 п.1 (шаг в `verify_delivery_M-08.sh` рядом с `D6`). Номер — механизмом
   (`scripts/next_artifact_id.sh TD` → `TD-243`). Ярус C грепом: `TECH-DEBT.md` — `TD-239`
   (индекс `:215`, карточка `:5614`), `TD-165`; `PROJECT-STATE.md` — `M-93`, `TD-239`.

## Наблюдение вне предмета (не блокер)

`git config user.name` в общем чекауте — `engine-dev` (локальный override; глобально — владелец),
16 из последних 20 коммитов `main` подписаны `engine-dev`, включая reviewer'ские. Это расходится с
`branch-hygiene.md` п.6 («ролевые `user.name` не выставляются»). Не предмет `TD-239`; передаю
founder'у строкой в handoff, карточку не завожу (аудит-трейл держится на метке роли в subject).
