<!-- GATE-META
milestone: M-93
audited_repo: a3ka/hft-platform
audited_base: 81132e10c0d51a2a9d8ccfe2d74dac991fe38d19
audited_head: 052d8859c00a61377eb1246fc868a61636c95bae
verdict: APPROVE
-->

# R-222 — PR-гейт M-93: сторож `ops-watchdog` доставляется деплоем (`TD-231`)

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL: диапазон трогает `Dockerfile`,
`deploy/**`, `.github/workflows/deploy.yml`). Засчитывается также как перепроверка `gates.md` §9
для `.github/workflows/deploy.yml` (пункты (а)–(в) — §4 ниже); вердикт вынесен со свежим контекстом,
в авторстве правки не участвовал.
**Предмет:** ВЕТКА `origin/feat/M-93-watchdog-delivery` → `main`.
**Цепочка:** `C-277` REJECT → `C-278` REJECT → `A-046` DECISION → engine-dev (задачи 1–2) →
architect (задача 3) → tester (`/tmp/m93-tester.md`, 5/6 на `4f7fc3b`, шаг 6 — subject-lock) →
architect `052d885` (токен `ALLOW-SUBJECT-CHANGE`) → этот гейт.
**Вердикт: APPROVE** — с двумя неблокирующими находками, заведёнными в реестр долга (§5).
Задача 4 (§8-гейт на проде) исполняется после merge и предъявляется в close-out.

## §0 — Вершина взята командой

```
$ git fetch origin && git rev-parse origin/feat/M-93-watchdog-delivery origin/main
052d8859c00a61377eb1246fc868a61636c95bae      ← вершина ветки (справка мандата 052d885 — совпала)
40e88a840e599ff74c70eca867d840c20b479a33      ← origin/main
$ git merge-base origin/main origin/feat/M-93-watchdog-delivery
81132e10c0d51a2a9d8ccfe2d74dac991fe38d19      ← ветка ОТСТАЛА от main ⇒ гейты — на дереве слияния
```
Повторный `fetch` перед записью вердикта: вершина `052d885` — не ушла.

## §1 — Block-scope

Пофайлово по коммитам (`git show --numstat`), против §7 спеки:

| коммит | роль | файлы | зона по §7 |
|---|---|---|---|
| `dd4e183` | engine-dev | `Dockerfile` 6/1 | engine-dev ✔ |
| `6c2a2bc` | engine-dev | `deploy/bin/install-watchdog.sh` 106/0, `deploy/README.md` 97/2, `scripts/watchdog_cron.sh` 13/4 | engine-dev ✔ |
| `6016b92` | architect | `.github/workflows/deploy.yml` 7/0 | architect (§7, `A-046` §3.4) ✔ |
| `312f181`…`052d885` (прочие) | architect / critic / arbiter | `milestones/M-93-*`, `scripts/tests/red_m93_*`, `red_verify_M-93_ci_map.sh`, `verify_M-93.sh`, `verify_delivery_M-08.sh`, `docs/ROADMAP.md`, `research/critiques/C-277/278`, `research/arbitration/A-046` | каждая роль — в своей зоне ✔ |

`crates/**` диапазоном НЕ тронут. Удалений защищённых артефактов нет. Атомарность: задачи 1/2/3 —
отдельные коммиты со ссылкой на `M-93 task #N`. `Co-Authored-By` в теле — нет.

**RED-first:** `red_m93_watchdog_delivery.sh` — `eda7c2b` / `a409930` / `d9191d4`, все ДО
`dd4e183` (первый impl). После impl-коммитов `scripts/tests/**` и `verify_*` не правились.

## §2 — Block-C / Block-risk

- `crates/contracts/**` — не тронут ⇒ Block-C не применим.
- `crates/{risk,killswitch,oms,venue-*}/**` — не тронуты ⇒ RISK-BLOCK (`gates.md` §5) не применим,
  `risk-critic` не требуется. Сторож — наблюдение, без order-egress и без канала наружу (`П-003`).

## §3 — Block-DoneBlock: прогоны на ДЕРЕВЕ СЛИЯНИЯ (`origin/main` + ветка, `82cd2bc`, локально, не пушится)

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/tests/red_m93_watchdog_delivery.sh; echo exit=$?
pass  w1 … pass  w8 (9 сценариев)
сценариев: 9   pass=9   FAIL=0
VERDICT: PASS
exit=0

$ bash scripts/verify_M-93.sh; echo exit=$?            # дерево слияния
PASS=55 SKIP=8 FAIL=2(одна проверка)
PASS  task1-3: red_m93_watchdog_delivery
PASS  task1: cargo build -p ops --bin ops-watchdog
PASS  task1: verify_delivery_M-08.sh (мелкая форма; D9-deep — в CI)
PASS  task2: cargo test -p ops
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
PASS  ci-parity: cargo test --all
FAIL  ci-parity: bash scripts/check_gate_meta.sh (exit=1)
FAIL  merge 82cd2bc4 называет M-93, но research/reviews/R-*.md с этим литералом в дереве слияния НЕТ (класс TD-105)
SKIP  task4: §8-гейт …
VERDICT: FAIL (провалов: 1)
exit=1
```

**Разбор единственного FAIL.** Это барьер «merge без вердикта» (`TD-105`), и на дереве слияния ДО
появления этого файла он обязан краснеть: мой пробный merge-коммит называет M-93, а `R-*` с этим
литералом ещё не было. Он закрывается ИМЕННО этим файлом; перепрогон шага после коммита вердикта —
в §3bis. Цифра tester'а/architect'а («56 PASS, exit=0 на `052d885`») снята на ВЕТКЕ без merge-коммита,
где барьеру нечего ловить; расхождение объяснено, а не проигнорировано.

### §3bis — перепрогон барьера после коммита вердикта

Предъявляется в close-out (сырой вывод `check_gate_meta.sh` на дереве слияния с этим файлом и
зелёный `All checks passed` PR'а) — решение о merge принимается по коду возврата `gh pr checks --watch`.

## §4 — Перепроверка `gates.md` §9 для `.github/workflows/deploy.yml`

**(а) Утверждения о коде — командой на дереве слияния.**

1. *Ветка healthy: отказ установки ⇒ красный деплой.* Шаг исполняется под `set -euo pipefail`
   (`deploy.yml`, `script:` шага «Deploy via SSH»), `cd /root/hft-platform` — относительный путь
   `deploy/bin/install-watchdog.sh` разрешается. Строка
   `bash deploy/bin/install-watchdog.sh || { echo "=== WATCHDOG INSTALL FAILED ===" >&2; exit 1; }`
   стоит ПОСЛЕ `=== healthy … ===` и ДО `docker image prune -f` ⇒ временный контейнер уже снят
   установщиком, prune его не ждёт. Отказ ⇒ `exit 1` ⇒ джоб красный; контейнеры при этом НЕ
   откатываются (они healthy) — это сознательная форма спеки §4: сигнал, а не откат.
2. *Ветка отката: `|| true`, затем `exit 1`.* Откат делает `git reset --hard "$PREV"`; если `PREV` —
   коммит ДО M-93, установщика в дереве нет ⇒ `bash` вернёт 127 ⇒ `|| true` ⇒ `exit 1` сохраняет
   причину (красный откат). На хосте остаётся прежний бинарь, а обёртка cron после отката — СТАРАЯ
   (`target/release/ops-watchdog`), который не удаляется (спека §5) ⇒ композиция цела в обе стороны.
3. *Установщик fail-closed.* Семантика выхода с EXIT-trap, возвращающим `return "$rc"`, проверена
   исполнением (`set -e; trap f EXIT; exit 1|true|false` → 1/0/1): trap не маскирует код. На отказе
   `inspect` — `|| exit 1`, `create` не зовётся (`w4`); на отказе `cp` — trap снимает контейнер и
   `DST.new.$$` (`w3`); пустой файл — `exit 1` до `mv` (`w5`); `mv -f` в той же директории — rename.
4. *Деплой ЗАПУСТИТСЯ этой поставкой.* Фильтр `on.push.paths` содержит `'Dockerfile'` и
   `'.github/workflows/deploy.yml'` — оба в диффе. Предел спеки §6 п.5 (`deploy/bin/**` и
   `scripts/watchdog_cron.sh` вне фильтра) подтверждён тем же чтением.
5. *Образ несёт бинарь.* `Dockerfile:18` `--bin ops-watchdog`, `Dockerfile:40`
   `COPY --from=builder … /usr/local/bin/ops-watchdog` (дерево слияния).

**Мутационный контроль `w7` (свой, сверх M1–M10).** Подмена в ветке healthy
`|| { …; exit 1; }` → `|| true` (копия дерева слияния в scratchpad):
```
$ grep -n 'install-watchdog.sh' .github/workflows/deploy.yml
305:              bash deploy/bin/install-watchdog.sh || true
315:              bash deploy/bin/install-watchdog.sh || true
$ bash scripts/tests/red_m93_watchdog_delivery.sh | grep -E 'w7|VERDICT'
pass  w7 deploy.yml: установка сторожа на ветке healthy и на ветке отката
VERDICT: PASS
```
Мутант ЖИВ — см. находку **N-1**. Текущий код при этом верен (п.1).

**(б) Полномочия.** `deploy.yml` — зона architect'а по спеке §7 и `A-046` §3.4 (CI/CD — харнесс);
коммит `6016b92` помечен `[architect]`. Зона `gates.md` §11 (`.claude/**`, `CLAUDE.md`,
`docs/04-workflow.md`) диапазоном не тронута ⇒ `FOUNDER-APPROVED` не требуется. Subject-lock после
`A-046` DECISION снят токеном `ALLOW-SUBJECT-CHANGE` в теле `052d885` (проверено `git log -1 --format=%B`).
Граница C не задета: состав данных, пороги тревог, токены `TELEGRAM_*` не меняются.

**(в) Связность.** Путь установки (`install-watchdog.sh` `DST`) = путь cron'а (`watchdog_cron.sh`
`WATCHDOG_BIN`) — оба `${HFT_WATCHDOG_ROOT:-}/usr/local/lib/hft/ops-watchdog` (`w1`/`w1b` зелены);
`/etc/cron.d/hft-watchdog` зовёт `/root/hft-platform/scripts/watchdog_cron.sh` (замер прода до
деплоя: `grep -c 'HFT_WATCHDOG_ROOT\|HFT_WATCHDOG_DST\|WATCHDOG_BIN' /etc/cron.d/hft-watchdog
/etc/environment` → `0`/`0`). Висячие ссылки в `deploy/README.md` — **N-2**.

## §5 — Находки

**N-1 (MINOR, неблокирующая) — `w7` не пиннит половину `I-5`, которую спека ему приписывает.**
Спека §3 `I-5`: «деплой зовёт установку на ветке healthy (**отказ установки — деплой красный**)…»,
оракул — `w7`. `w7` (`red_m93_watchdog_delivery.sh:142-143`) считает ВХОЖДЕНИЯ строки
`deploy/bin/install-watchdog.sh` в тексте ветвей — мутант `|| true` на ветке healthy (выше)
и закомментированная строка проходят его зелёными. Спека §9 честно перечисляет для `w7` только мутант
«установка только на ветке healthy», так что ложного утверждения о мутациях нет — завышена
колонка «оракул» у `I-5`. Почему не блокер: код верен (§4 п.1), и единственный путь ослабления —
будущая правка `deploy.yml`, которая сама пройдёт §9. Описание дефекта — здесь; форма защиты —
architect'у (`gates.md` §4, граница reviewer↔architect). Заведено **`TD-238`**.

**N-2 (MINOR, неблокирующая) — инструкция оператора в `deploy/README.md` §0a ссылается на то,
чего нет в коде.**
- `deploy/README.md:99`: «джоб зелёный И `=== WATCHDOG INSTALL OK ===` в логах шага» — эту строку
  не печатает ни установщик, ни `deploy.yml` (`git grep -n "WATCHDOG INSTALL OK"` → единственное
  вхождение — сам README). Оператор, ищущий маркер при ALERT, его не найдёт и на зелёном деплое.
- `deploy/README.md:102`: «`journalctl -u deploy`» — юнита `deploy` нет: деплой идёт через
  `appleboy/ssh-action`, его лог — только в логе джоба GitHub Actions.
- там же: «образ, который собрал CI» — образ собирается на VPS (`docker compose up -d --build`),
  CI собирает его лишь в джобе `delivery` для D9-deep.
Почему не блокер: это runbook ALERT-ветки, на исполнение доставки не влияет; круг ради трёх строк
README дороже записи. Заведено **`TD-239`** (зона — engine-dev, `deploy/**`).

**Замечание (не находка) — однократная ложная тревога при ПЕРВОМ деплое.** Между `git reset --hard`
(новая обёртка зовёт `/usr/local/lib/hft/ops-watchdog`) и концом установки после healthy идёт
сборка образа (минуты); тик cron'а в этом окне запишет `ALERT … бинарь не найден` и
`watchdog.alert`. Следующий успешный тик снимает маркер (`watchdog_cron.sh`: `rm -f "${ALERT_FILE}"`).
Только первая поставка: дальше файл на хосте уже есть. В §8 это видно и не трактуется как отказ.

## §6 — Предъявление FA

`crates/ops/**` кодом не тронут (барьер `review-fa` даёт SKIP). Опора — `docs/fa/ops.md`:
**`OPS-I-10`** («объявлена ⟹ эмитится»: тревога, вычисляемая бинарём вне доставки, объявлена, но не
исполняется) и **`OPS-I-11`** (тишина выдачи; её «предел, названный честно» — «потребитель НЕ
ИСПОЛНЯЕТСЯ … `TD-231`, `TD-220`» — снимает именно эта поставка; правка текста инварианта после §8 —
architect).

## §7 — Условие APPROVED и что дальше

1. PR → `gh pr checks <N> --watch` → merge ТОЛЬКО по коду возврата 0.
2. §8 (задача 4 спеки), обе ноги: деплой ПО ДЖОБУ (`Deploy (build on VPS)` = `success`, не
   `skipped`); ssh — HEAD = merge-коммит, `/usr/local/lib/hft/ops-watchdog` существует и `sha256`
   равен `/usr/local/bin/ops-watchdog` в работающем `hft-recorder`, ближайший тик cron пишет результат
   сторожа, `grep -c` переменных шва → 0, контейнеры healthy, heartbeat свежий.
3. Close-out: `TD-231` закрыть, `TD-220`/`TD-176` обновить, `TD-238`/`TD-239` завести, `PROJECT-STATE`.
   Ручной `/root/hft-platform/target/release/ops-watchdog` НЕ удалять (решение architect'а после §8).

Поиск в реестрах (ярус C, грепом): `TD-231`, `TD-220`, `TD-176`, `M-93`, `ops-watchdog` по
`TECH-DEBT.md` и `PROJECT-STATE.md` на `origin/main`.
