<!-- GATE-META
milestone: TD-239
audited_repo: a3ka/hft-platform
audited_base: 947991c46b44b8c9fb74d785a32627fe0ff3572b
audited_head: 3db461560224d22ecb85f3af640d2bbce3902349
verdict: REJECT
-->

# R-231 — TD-239 rev2: runbook ALERT-ветки сторожа (`deploy/README.md` §0a)

**Вердикт: REJECT.** Все находки `R-227` (B-1, B-2, B-3, N-1, N-2) сняты верно: каждая
ссылка `файл:строка` открыта и совпадает с кодом. Но новый текст про ветку отката (N-3,
включён в этот круг) содержит **неверное утверждение о порядке вывода в логе**. Правило,
по которому оператор узнаёт случай «пустой файл», для ветки отката не работает. Это тот же
класс, что закрывает `TD-239` (оператор при тревоге ищет то, чего в логе нет), и это
**второй REJECT подряд по одной причине** (см. §«Маршрут»).

## Предмет

- Ветка `fix/TD-239-watchdog-runbook`, вершина взята командой:
  `git fetch origin && git rev-parse origin/fix/TD-239-watchdog-runbook` → `3db4615…`
  (совпадает со справкой из мандата).
- База для `audited_base` — `merge-base` с `origin/main` = `947991c` (база ветки; мандат
  назвал `49c9cc0` — это коммит вердикта `R-227`, а не база ветки; диапазон rev2
  `49c9cc0..3db4615` = один коммит `3db4615`, судился вместе со всем дифом ветки к `main`).
- Диф ветки к `main`: `deploy/README.md` +62/−11, `research/reviews/R-227-…` (+147).
- Ярус C грепом: `TECH-DEBT.md` на `origin/main` — `TD-239` (индекс `:215`, карточка
  `:5614`); `PROJECT-STATE.md` — `TD-239` (`:3062`, упоминание); `docs/ROADMAP.md:81`
  (строка 4bis, остаток M-93). Close-out не делается (REJECT).
- Ярус B: `docs/fa/ops.md` — правкой не тронута, утверждений о её инвариантах текст не делает.

## Block-scope — PASS

Только `deploy/README.md` (зона engine-dev, `scope-guard.md`). `crates/**`, `*/tests/**`,
`contracts/**`, `scripts/**`, `milestones/**`, зона §11 не тронуты. Коммит атомарный, `TD-239`
в subject, путь назван явно, трейлера co-author нет.

FA-WAIVER не требуется: диапазон не трогает `crates/**` (барьер `check_review_fa.sh` → SKIP).

## Block-C / Block-risk — N/A

Контракты и safety-путь не тронуты.

## Сверка находок R-227 (каждая строка открыта на `3db4615`)

| R-227 | Что сказано в README | Код | Статус |
|---|---|---|---|
| B-1 | `set -euo pipefail` без `-x`; на отказах скрипт сам не печатает | `install-watchdog.sh:43`; `:78`, `:83`, `:88-89`, `:100-101` — голый `exit 1`; единственный `printf` — `:51` (режим печати пути) | снято |
| B-2 | шаг `Deploy via SSH`, `appleboy/ssh-action@v1`, `script: \|` | `deploy.yml:239`, `:240`, `:246` | снято |
| B-3 | цель `inspect` — только `CONTAINER` (`hft-recorder`) | `install-watchdog.sh:46`, `:78` | снято |
| N-1 | `up -d --build` — первое звено условия; сторож — в `then` после двух `healthy` | `deploy.yml:298-300`, `:305` | снято |
| N-2 | `ci.yml` `delivery` собирает образ, но на VPS не доставляет; registry нет | `ci.yml:37-45`, `deploy.yml:3`; `branch-build.yml` — только fmt/clippy/test (`:103-107`) | снято |
| N-3 | ветка отката: `install … \|\| true`, маркер `FAILED` не печатается, красный по `exit 1` | `deploy.yml:308-316` | **описано, но см. B-1 ниже** |
| N-4 | не тронут | — | подтверждено |

## Находки

### B-1 (блокер) — для ветки отката указан неверный порядок вывода

`deploy/README.md:150-154` (п.2): ветка «пустой файл» — «**НИКАКОЙ** ошибки docker перед
маркером `=== WATCHDOG INSTALL FAILED ===` от `deploy.yml:305` (**или `=== DEPLOY FAILED ===`
от `deploy.yml:308`**, см. п.1): само отсутствие docker-ошибки в этой ветке — единственный
признак «пустого файла»».

Факт, `deploy.yml` в ветке `else`:

```
308  echo "=== DEPLOY FAILED — logs + rollback to … ==="     ← маркер печатается ПЕРВЫМ
309  docker logs hft-recorder --tail 50 || true
310  docker logs hft-gateway-serve --tail 50 || true
311  git reset --hard -q "$PREV"
312  docker compose up -d --build recorder gateway-serve
315  bash deploy/bin/install-watchdog.sh || true              ← установка — ПОСЛЕ маркера
316  exit 1
```

В ветке отката маркер `DEPLOY FAILED` стоит **до** запуска установщика. Значит, перед ним
вывода установщика нет ни в каком случае. После установщика никакого маркера тоже нет:
`|| true`, затем сразу `exit 1`. Правило «нет docker-ошибки перед маркером = пустой файл»,
если применить его к этой ветке, смотрит на вывод упавшего деплоя ДО отката
(`up --build`, ожидание healthy). К установке этот вывод отношения не имеет. Сам README в
п.1 (`:120-131`) порядок описывает верно; в п.2 он ему противоречит.

Воспроизведение: `git show origin/fix/TD-239-watchdog-runbook:.github/workflows/deploy.yml | sed -n 307,316p`.

Условие снятия: правило «пустого файла» относится только к ветке `then` (`:305`). Для
ветки отката указано, что вывод установщика идёт ПОСЛЕ `DEPLOY FAILED`, маркер установки не
печатается, а его отказ гасится `|| true`. Либо нужно прямо сказать, что в этой ветке
различать случаи установки не имеет смысла, и почему.

### N-1 — в логе нет строки `=== DEPLOY FAILED ===` дословно

`deploy/README.md:130` («ищите `=== DEPLOY FAILED ===` в логах») и `:152`. Код печатает
`=== DEPLOY FAILED — logs + rollback to <sha> ===` (`deploy.yml:308`). Поиск по строке,
скопированной из runbook'а, ничего не находит:

```
$ echo "=== DEPLOY FAILED — logs + rollback to abc1234 ===" | grep -cF "=== DEPLOY FAILED ==="
0
```

На `:122` строка дана верно (с `— logs + rollback to … ===`). Исходный дефект `TD-239`
был именно маркером, которого в логе нет (`=== WATCHDOG INSTALL OK ===`). Строки
`:130`/`:152` всё равно правятся из-за B-1, поэтому чинить нужно тем же коммитом.

### N-2 — «stderr выдаёт ТОЛЬКО docker CLI» — неполно

`deploy/README.md:143-144`. Кроме четырёх названных точек, у установщика есть ещё три
места отказа с выводом не от docker: `mkdir -p` (`install-watchdog.sh:57`, самый первый
шаг, `set -e`), `chmod` (`:105`), `mv -f` (`:106`). Их stderr — от coreutils. Вероятность
отказа низкая (деплой идёт от root'а, `:38-41`), а сообщение говорит само за себя, поэтому
это не блокер. Но формулировка «ТОЛЬКО docker CLI» как утверждение о коде неверна:
либо назвать и эти три точки, либо убрать слово «ТОЛЬКО».

### N-3 — решение по включению N-3 в объём не записано артефактом

`R-227` относил решение о N-3 к architect'у. В §E dev пишет, что включил N-3 «по явной
инструкции инвокации», но такой инструкции нет ни в одном файле. Само включение не
возражаю (текст полезен после правки B-1). Фиксирую, что решение живёт только в переписке.

## Маршрут (для architect)

Это **второй REJECT подряд по одной причине**: текст runbook'а утверждает о коде то, чего
в коде нет. `R-227` назвал класс, rev2 закрыл названные места, а в новом месте тот же класс
повторился (ветка отката). `gates.md` §0 п.1 формально требует созвать арбитра, не дожидаясь
третьего круга. Спора сторон здесь нет, поэтому architect решает, что делать: (а) созвать
арбитра по §0; (б) спроектировать защиту класса. `R-227` §«Класс» отмечает, что оракула
«утверждение runbook'а о коде ↔ код» нет, а `verify_delivery_M-08.sh` D6 к истинности §0a
слеп. Сам фикс B-1/N-1/N-2 мелкий и остаётся в зоне engine-dev (`deploy/**`).

## Условие APPROVED

B-1 снят: правило распознавания случаев установки описано отдельно для `then` (`:305`) и
`else` (`:308-316`), порядок вывода совпадает с кодом. N-1: строка `DEPLOY FAILED` дана
так, как её печатает код. N-2: либо названы `:57`/`:105`/`:106`, либо убрано «ТОЛЬКО».
`verify_delivery_M-08.sh` остаётся зелёным.

## Done Block (reviewer, worktree `/tmp/hft-reviewer-TD-239-r2`, detached на `3db4615`)

```
$ git fetch origin && git rev-parse origin/fix/TD-239-watchdog-runbook
3db461560224d22ecb85f3af640d2bbce3902349
$ git merge-base origin/main origin/fix/TD-239-watchdog-runbook
947991c46b44b8c9fb74d785a32627fe0ff3572b
$ git log --oneline origin/main..origin/fix/TD-239-watchdog-runbook
3db4615 docs(TD-239): runbook §0a — сверено с кодом по R-227 [engine-dev]
49c9cc0 docs(review): R-227 — TD-239 REJECT: фикс runbook'а вносит три новых несуществующих утверждения [reviewer]
eea45db docs(TD-239): runbook ALERT сторожа — реальные маркер/лог/место сборки [engine-dev]
$ git diff --stat origin/main...origin/fix/TD-239-watchdog-runbook
 deploy/README.md                                  |  73 +++++++++--
 research/reviews/R-227-TD-239-watchdog-runbook.md | 147 ++++++++++++++++++++++
 2 files changed, 209 insertions(+), 11 deletions(-)

$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E "^(PASS|FAIL|SKIP|DELIVERY)"; echo exit=${PIPESTATUS[0]}
PASS  D1 … PASS D2 … SKIP D1-deep … PASS D3 D4 D5a D5 D7 D6 D8 D8 D9
DELIVERY: PASS
exit=0

$ grep -nE "set -x|echo|printf" deploy/bin/install-watchdog.sh | grep -v '^ *[0-9]*:#'
51:  printf '%s\n' "${DST}"
$ grep -n "DEPLOY FAILED" deploy/README.md
122:     уже напечатал `=== DEPLOY FAILED — logs + rollback to … ===`
130:     В этом исходе ищите `=== DEPLOY FAILED ===` в логах (п.2), прежде чем
152:   `deploy.yml:305` (или `=== DEPLOY FAILED ===` от `deploy.yml:308`, см. п.1):
$ echo "=== DEPLOY FAILED — logs + rollback to abc1234 ===" | grep -cF "=== DEPLOY FAILED ==="
0

$ gh run list --branch main --limit 3
completed success Deploy to VPS … 2026-10-04T18:53:11Z
completed success Merge pull request #309 … CI … 2026-10-04T18:42:58Z
completed success Deploy to VPS … 2026-10-04T18:17:57Z
```

Зелёный гейт находок не опровергает: §0a он не проверяет (`R-227` §«Класс»).
