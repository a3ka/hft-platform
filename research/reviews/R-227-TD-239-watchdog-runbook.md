<!-- GATE-META
milestone: TD-239
audited_repo: a3ka/hft-platform
audited_base: 947991c46b44b8c9fb74d785a32627fe0ff3572b
audited_head: eea45dbaaa40e8feb5246ab7eb2c0fd844fe8ba4
verdict: REJECT
-->

# R-227 — TD-239: runbook ALERT-ветки сторожа (`deploy/README.md` §0a)

**Вердикт: REJECT.** Три исходных ложных утверждения `TD-239` сняты корректно, но правка
вносит в тот же runbook **три новых утверждения о коде, которых в коде нет** — ровно класс,
который `TD-239` закрывает («оператор при тревоге ищет то, чего нет»). Для docs-правки,
единственный критерий которой — истинность текста, это блокер, а не примечание.

## Предмет

- Ветка `fix/TD-239-watchdog-runbook`, вершина взята командой:
  `git fetch origin && git rev-parse origin/fix/TD-239-watchdog-runbook` → `eea45db…`;
  база `947991c` (= `origin/main` на момент аудита, `merge-base` совпадает).
- Диапазон: один коммит `eea45db docs(TD-239): … [engine-dev]`, `deploy/README.md` +26/−11.
- Ярус C грепом: `TECH-DEBT.md` — `TD-239` (строка индекса `:214`, карточка `:5642`, заведена
  `R-222` N-2); `PROJECT-STATE.md` не читался (REJECT — close-out не делается).

## Block-scope — PASS

Диф — только `deploy/README.md`. `crates/**`, `*/tests/**`, `contracts/**`, `scripts/**`,
`milestones/**` не тронуты. engine-dev владеет `deploy/**` (`scope-guard.md`). Коммит атомарен,
ссылка на `TD-239` в subject. Зона §11 (`docs-freeze`) не тронута.

FA-WAIVER не требуется: диапазон не трогает `crates/**` (барьер `check_review_fa.sh` → SKIP).
Ближайшая FA по предмету — `docs/fa/ops.md` не затронута текстом правки.

## Block-C / Block-risk — N/A

Контракты и safety-путь не тронуты.

## Что сделано верно (сверено)

| Старое утверждение | Факт | Статус |
|---|---|---|
| маркер `=== WATCHDOG INSTALL OK ===` | `deploy.yml:305` печатает только `FAILED`; `install-watchdog.sh` на успехе молчит | снято верно |
| `journalctl -u deploy` | юнита в репо нет | снято верно |
| «образ, который собрал CI» (как источник прод-бинаря) | `deploy.yml:3` — build-on-VPS, без registry | снято верно по сути |

## Находки

### B-1 (блокер) — несуществующий «`set -e` trace» на каждом отказе

`deploy/README.md` (ветка, п.2 runbook'а): «каждый случай печатает свой собственный `set -e`
выход с предшествующим `docker`/`set -e` trace'ом».

Факт (`deploy/bin/install-watchdog.sh` на `eea45db`): `set -euo pipefail` (`:43`), **`set -x`
нет нигде** — trace не печатается. Сам скрипт на отказах НЕ печатает ничего: `:78`
`|| exit 1`, `:83` `|| exit 1`, `:88-89` `exit 1`, `:100-101` `exit 1` — без `echo`.
Что оператор реально увидит: stderr самого docker CLI на `inspect`/`create`/`cp`; на шаге
пустого файла (`:100`) — **ничего**, только `=== WATCHDOG INSTALL FAILED ===` от `deploy.yml`.
Воспроизведение: `grep -nE 'set -x|echo' deploy/bin/install-watchdog.sh` — `set -x` нет, `echo`
в путях отказа нет.
Симптом: оператор ищет в логе trace, которого нет, и не узнаёт случай «пустой файл» по
единственному его признаку — отсутствию ошибки docker перед `FAILED`.

### B-2 (блокер) — `ssh … 'bash -s …'` — несуществующая форма вызова

README (п.2): «на VPS джоб исполняется через `ssh … 'bash -s …'`».
Факт: `deploy.yml:239-246` — шаг `Deploy via SSH`, `uses: appleboy/ssh-action@v1`,
`script: |`. `grep -n "bash -s" .github/workflows/deploy.yml` — пусто. Единственный голый
`ssh` в файле (`:134`) — catchup-проба SHA, не деплой. Вывод «лог = лог Actions-шага» верен,
обоснование — выдумано.

### B-3 (блокер) — `hft-gateway-serve` в путях отказа установщика

README (п.2): «`docker inspect` (нет `hft-recorder`/`hft-gateway-serve`?)».
Факт: `install-watchdog.sh:46` `CONTAINER="${HFT_WATCHDOG_CONTAINER:-hft-recorder}"`, `:78`
инспектирует только его. `hft-gateway-serve` установщик не трогает. Оператор, увидев
отсутствующий gateway-serve, пойдёт по ложному следу.

### N-1 — «сборка … после healthy-гейта»

README (абзац «Почему НЕ собирать на хосте»): образ собирается «шагом `docker compose up -d
--build` в `deploy.yml` (задача 3, после healthy-гейта)». Факт `deploy.yml:298-300`: `up -d
--build` — ПЕРВОЕ звено условия healthy-гейта, а не шаг после него; после гейта идёт установка
сторожа (`:305`). Формулировка путает порядок; совместно с B-1..B-3 подлежит той же сверке.

### N-2 — «Никакого образа, который собрал CI, нет» буквально неверно

`ci.yml:37-44` джоб `delivery` с `HFT_DELIVERY_DEEP=1` собирает прод-образ в CI. Он не
доставляется на VPS, поэтому суть (источник прод-бинаря — сборка на VPS) верна, но абсолютное
«нет» и ссылка только на `branch-build.yml` — неполны.

### N-3 — ветка отката не описана (до правки — тоже)

`deploy.yml:315`: на ветке отката `install-watchdog.sh || true` — маркер `FAILED` не
печатается, джоб красный по `DEPLOY FAILED`. Дихотомия п.1 runbook'а («зелёный без FAILED /
красный с FAILED») этот исход не покрывает. Предсуществующий пробел, вне объёма трёх маркеров
`TD-239`; решение — architect'у (включать ли в объём или заводить отдельно).

### N-4 — `Dockerfile:18` (поднято dev'ом в §E)

Подтверждаю: `:18` — `cargo build … --bin ops-watchdog`, копия в `/usr/local/bin` — `:40`.
Вне объёма `TD-239`.

## Класс, а не место (для architect)

Правка, исправляющая «текст утверждает несуществующее», внесла три новых несуществующих
утверждения — при том что Done Block зелёный. Гейт `verify_delivery_M-08.sh` D6 смотрит лишь
наличие слов «storage box / /mnt/journal-cold» и к истинности §0a слеп по построению.
Оракула «утверждение runbook'а о коде ↔ код» нет; `testing.md` («исправление по вердикту тоже
требует оракула») это прямо не покрывает для документов. Проектирование защиты — зона
architect'а; reviewer фиксирует только факт повторения класса в самом фиксе.

## Условие APPROVED

B-1, B-2, B-3 сняты: каждое утверждение п.2 runbook'а о поведении `install-watchdog.sh` и
форме вызова деплоя совпадает с кодом (`file:line`); N-1/N-2 — сверены и либо исправлены, либо
явно отклонены с основанием; N-3 — решение architect'а записано.

## Done Block (reviewer, свой worktree `/tmp/hft-reviewer-TD-239` на `eea45db`)

```
$ git rev-parse origin/fix/TD-239-watchdog-runbook origin/main
eea45dbaaa40e8feb5246ab7eb2c0fd844fe8ba4
947991c46b44b8c9fb74d785a32627fe0ff3572b

$ git diff --stat origin/main...origin/fix/TD-239-watchdog-runbook
 deploy/README.md | 37 ++++++++++++++++++++++++++-----------
 1 file changed, 26 insertions(+), 11 deletions(-)

$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E "^(PASS|FAIL|SKIP|DELIVERY)"; echo exit=${PIPESTATUS[0]}
PASS  D1 … PASS D2 … SKIP D1-deep … PASS D3 D4 D5a D5 D7 D6 D8 D8 D9
DELIVERY: PASS
exit=0

$ git show origin/fix/TD-239-watchdog-runbook:.github/workflows/deploy.yml | grep -n "bash -s"
(пусто)
$ … deploy.yml | sed -n 239,246p
      - name: Deploy via SSH
        uses: appleboy/ssh-action@v1
        …
          script: |
$ … install-watchdog.sh | grep -n CONTAINER=
46:CONTAINER="${HFT_WATCHDOG_CONTAINER:-hft-recorder}"
$ … deploy.yml | grep -n WATCHDOG
305:              bash deploy/bin/install-watchdog.sh || { echo "=== WATCHDOG INSTALL FAILED ===" >&2; exit 1; }
```

Зелёный гейт не опровергает находок: он не проверяет §0a (см. «Класс»).
