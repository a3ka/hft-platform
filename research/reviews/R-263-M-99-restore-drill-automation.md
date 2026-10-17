<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: 374f6a5c2e39e6e85467ab8ab0e3c1773ca43a5b
verdict: REJECT
-->

# R-263 — M-99 restore-drill automation, PR-гейт: REJECT

FA-WAIVER: crates/recorder — FA для крейта recorder не существует (reading-map §2); опора — OPS-I-10 из docs/fa/ops.md

## §0. Предмет

- Ветка `origin/feat/M-99-restore-drill-automation`, вершина взята командой: `374f6a5c`
  (= справке tester'а; повторный fetch перед записью — без сдвига).
- База аудита — `merge-base origin/main HEAD` = `a660b3a7` (та же, что у `C-298`/`C-306`).
- Прочитано по предмету: спека `M-99` целиком; `C-298`, `C-306`; `A-032` §2.3/§2.4; `M-74`
  §«Сигнатура читателя», §«Контракт обёртки», T2-таблица; `docs/fa/ops.md` (`OPS-I-2`,
  `OPS-I-3`, `OPS-I-10`, `OPS-I-11`); `docs/fa/journal.md` `JR-I-6`. Ярус C — грепом:
  `TD-193` в `TECH-DEBT.md` (ЧАСТИЧНО, MAJOR), `M-99` в `PROJECT-STATE.md` (0 совпадений).
- Инварианты тронутых модулей: **`OPS-I-3`** (копия периодически восстанавливается и
  читается), **`OPS-I-10`** (объявлена ⟹ эмитится), **`JR-I-6`** (старый журнал читается
  новым кодом). Именно их нарушают находки Б-1…Б-4 ниже: на проде drill не исполнится, а
  метрика будет показывать 0 и при успешном drill'е.

## §1. Вердикт

**REJECT.** Гейт `verify_M-99.sh`, проба и все оракулы зелёные (с одной оговоркой — §4), но
**прод-путь drill'а мёртв по построению в четырёх независимых местах**. Ни одно из них гейт не
видит: проба гоняет `TRANSPORT=local`, а сценарий `T` проверяет ветку `remote` только на
ОТКАЗЕ транспорта. Это ровно класс `testing.md` «Целостность гейта», свойство 1: гейт,
проверенный не тем вызовом, каким его зовёт прод, не проверен. Задача 8 (первый живой прогон
на VPS) упала бы на каждой из Б-1…Б-3, но её делают после merge'а — значит в `main` уехал бы
механизм, который ни разу не может завершиться успехом.

Маршрут по профилю reviewer'а: находки — **architect'у** (разбор класса), не dev'у.
Дизайн исправлений здесь НЕ предлагается (`gates.md` §4, граница reviewer↔architect).

## §2. Блокеры

### Б-1 — ветка `remote` обёртки не может закончиться успехом (`OPS-I-3`)

`deploy/bin/journal-restore-drill-cron.sh`, ветка `TRANSPORT=remote` (дефолт на проде):

1. **Читатель зовётся на ПУСТОМ каталоге.** После доставки обёртка делает
   `mv "${RESTORE}" "${SOURCE_DIR}"; mkdir -p "${RESTORE}"` и зовёт читатель с
   `--dir "${RESTORE}"` (блок «УНИЧТОЖЕНИЕ источника»; комментарий там же прямо говорит:
   «Читатель ниже зовётся на ПУСТОЙ `${RESTORE}` — и НЕ сможет прочитать ничего»). Итог —
   всегда `rc=5`, `ok=0`, причина «empty». `M-99` §4 п.1 описывает уничтожение источника
   В ПРОБЕ (источник — `cold`), а не уничтожение доставленных байтов перед чтением.
2. **Отбор индексов делается локальным `ls` по удалённому адресу.**
   `ALL="$(ls "${COLD}" …)"`, где на проде `COLD=u659392-sub1@…:journal` — локального такого
   пути нет, `N=0`, исход `cold-empty`.
3. **Задумана доставка копии ЦЕЛИКОМ.** Комментарий: «remote-режим — каталог привёз ВСЮ копию;
   отбор ПО ИНДЕКСАМ ниже оставит только выбранные». `M-99` §7 запрещает прямо: «качать копию
   целиком — 695 ГБ на прогон». Сама команда транспорта `${REMOTE_CMD} "${COLD}" "${RESTORE}"`
   = `ssh -i … -p 23 user@host:journal /var/lib/hft/restore-drill` — это не rsync, а ssh с
   хостом, содержащим `:journal`, и путём в роли удалённой команды.

**Воспроизведение** (скрипт — Приложение ниже, фикстура пробы прод-формы, настоящий
`journal-drill-read`; транспорт подменён самой благоприятной заглушкой — копирует всё без
ошибок, `COLD` локальный, чтобы обойти п.2):

```
── control-local (TRANSPORT=local) rc=0
   READER-SPY argv: --dir …/control-local/restore --min-events 1; файлов в --dir: 6
   state: {"ok":1,…,"checked":3,"events_read":4125,"digest":"e0eeed2a…","reason":""}
── remote-ok-transport (TRANSPORT=remote) rc=5
   TRANSPORT-STUB argv: …/cold …/remote-ok-transport/restore
   TRANSPORT-STUB delivered: 11 файлов
   READER-SPY argv: --dir …/remote-ok-transport/restore --min-events 1; файлов в --dir: 0
   state: {"ok":0,…,"events_read":0,"digest":"","reason":"reader-rc-5 (empty — копия не привезла событий)"}
```

Транспорт привёз 11 файлов, читатель увидел 0. Та же фикстура и тот же читатель на `local` —
`ok=1`.

### Б-2 — расписание никогда не запускает drill (двойной `flock`)

`deploy/cron.d/journal-restore-drill`: `… root flock -n /var/lock/hft-journal-restore-drill.lock <обёртка>`.
Обёртка внутри сама открывает тот же файл (`exec 9>"${LOCK_FILE}"; flock -n 9`). Блокировка
`flock(1)` принадлежит родителю и держится, пока идёт дочерний процесс; новое открытие файла
внутри — другое описание файла, `flock -n` отказывает. Обёртка пишет «SKIP предыдущий прогон
ещё работает» и выходит с `0`, состояние не пишется.

```
── cron-form (TRANSPORT=local, prefix: flock -n …/drill.lock) rc=0
   state: <НЕТ ФАЙЛА>
   log:   2026-10-13T14:26:00Z SKIP предыдущий прогон ещё работает
```

То же поведение показано на минимальной модели (`flock -n LOCK inner.sh` → «INNER: SKIP»,
`inner.sh` напрямую → «lock acquired»). Сторож со временем поднимет `Missing`/`Stale`, то есть
отказ виден — но drill по расписанию не исполнится ни разу. Строку с `flock` в cron'е при
этом ТРЕБУЕТ гейт (`verify_M-99.sh:121`, `grep -q flock`) — см. Г-2.

### Б-3 — читатель по умолчанию ссылается на несуществующий образ

Дефолт `READER` = `docker run … --entrypoint /usr/local/bin/journal-drill-read hft-recorder`.
`hft-recorder` — имя КОНТЕЙНЕРА, а не образа. Замер на VPS (только чтение):

```
$ docker inspect hft-recorder --format '{{.Config.Image}}'
hft-platform-recorder:local
$ docker image inspect hft-recorder; echo rc=$?
inspect hft-recorder rc=1
```

`docker run hft-recorder` попытается скачать `hft-recorder:latest` из публичного реестра:
в лучшем случае отказ (`reader-rc-125`), в худшем — исполнение чужого образа от root (сеть
выключена, каталог `:ro`, но поверхность лишняя). Гейт (`verify_M-99.sh:99`) проверяет лишь
наличие подстроки `hft-recorder` в argv.

### Б-4 — метрика на проде всегда 0: recorder не видит файл состояния (`OPS-I-10`)

Обёртка (на хосте) пишет `/var/lib/hft/journal-restore-drill.json`; `sample_restore_drill` в
`crates/recorder/src/main.rs` читает тот же путь ВНУТРИ контейнера recorder'а. Контейнер
монтирует только том журнала:

```
$ docker inspect hft-recorder --format '{{range .Mounts}}{{.Source}} -> {{.Destination}}{{end}}'
/var/lib/docker/volumes/hft-platform_journal-data/_data -> /journal
$ docker exec hft-recorder ls -la /var/lib/hft
ls: cannot access '/var/lib/hft': No such file or directory
```

Значит вердикт всегда `Missing`, `backup_restore_drill_ok 0` — и после успешного drill'а.
Цель (а) из `M-99` §1 на проде не выполняется; метрика эмитится, но врёт. `docker-compose.yml`
не входит в Allowed paths `M-99` §6 — монтирование спекой не предусмотрено, это пробел спеки,
а не только реализации. Сторож (`ops-watchdog`, на хосте) путь видит — его часть работает.

## §3. Находки по гейту и спеке (зона architect'а)

- **Г-1.** Сценарий `T` пробы (`red_restore_drill.sh`, «ШОВ ТРАНСПОРТА») гоняет `remote` только
  при `JOURNAL_DRILL_STUB_RC=1`. Успешный транспорт не моделируется нигде, поэтому Б-1 (п.1–3)
  проходит весь набор. `A-032` §2.4 переносит на VPS сеть и ключ, но не логику ветки после
  доставки — её можно судить заглушкой, как сделано в воспроизведении выше.
- **Г-2.** `verify_M-99.sh:121` требует `flock` в строке cron'а, а спека задачи 2 требует
  `flock` в обёртке; вместе они и дают Б-2. Ни один шаг не исполняет строку cron'а в той форме,
  в какой её вызывает cron.
- **Г-3.** `verify_M-99.sh:99` судит образ читателя по подстроке, а не по существованию образа
  (Б-3). Спека §4 п.3 «образ работающего hft-recorder» допускает оба прочтения.
- **Г-4.** Нет проверки композиции «путь, куда пишет обёртка на хосте» = «путь, который видит
  recorder в контейнере» (`testing.md` «Механизм несущего пути» п.2) — Б-4.

## §4. Не блокеры (замечания)

- **Н-1.** `restore_drill_verdict` отходит от таблицы приоритетов `M-99` §5 (там же: «порядок
  проверок — порядок приоритета»): нет поля `ok` ⇒ `Failed` (по таблице — `Unparseable`);
  `FromFuture` проверяется раньше `digest`/`events_read`; нет `events_read` ⇒ `Unparseable`
  (по таблице — `Empty`); обязательное по T2 поле `checked` (`M-74` T2-таблица, «обязательно»)
  не проверяется вовсе — состояние без него даёт `Ok`. На `is_ok()` первые три не влияют, четвёртое —
  да (fail-open на неполном состоянии).
- **Н-2.** `now_wall_ms - ts_wall_ms` (`restore_drill.rs:155`) — без `saturating_sub`; в release
  `overflow-checks` не включены (`Cargo.toml` `[profile.release]`), и при `ts_wall_ms` около
  `i64::MIN` разность переполняется в отрицательную, вердикт `Ok`. Вход искусственный, но
  doc-комментарий обещает «никогда не паникует» и fail-closed.
- **Н-3.** `is_64_hex` принимает и заглавные буквы, doc-комментарий говорит «строчных».
- **Н-4.** `crates/journal/src/bin/journal-drill-read.rs`, тест `parse_args_requires_dir` ничего
  не проверяет (плацебо). Тесты в `src/` от dev'а — вне sacred-зоны, но `testing.md` запрещает
  dev'у писать тесты на свою реализацию, а этот к тому же пустой.
- **Н-5.** Колонка Status в §Tasks `M-99` не обновлена (задачи 2/2b/4 — `⏳ OPEN`, 3/5/6 —
  `🚧 COMPILE-RED`), хотя реализация лежит.
- **Н-6.** Ветка отстаёт от `main`: на ветке `cargo audit` красен (`quinn-proto 0.11.16`,
  RUSTSEC-2026-0338/0339/0341/0342), в `main` уже `0.11.19`. На дереве слияния tester получил
  PASS. К диффу M-99 отношения не имеет, но перед PR ветку нужно синхронизировать с `main`.
- **Н-7 — флак tester'а не воспроизвёлся.** В моём прогоне шаг
  `red_deploy_catchup.sh --battery` прошёл (PASS) с первого раза. Повода заводить долг нет;
  при повторе — отдельный предмет харнесса.

## §5. Что проверено и в порядке

- **Scope:** все 37 файлов дифа `a660b3a7..374f6a5c` лежат в Allowed paths `M-99` §6; путь
  удаления, `crates/journal/src/**` (кроме нового бинаря) и `RETENTION` в compose не тронуты
  (шаги «граница» гейта PASS). На Storage Box ничего не пишется (гейт: нет `--delete`/`rm` по
  `COLD`).
- **Block-C:** `crates/contracts/**` не тронут — не применимо.
- **Risk-блок:** `risk`/`killswitch`/`oms`/`venue-*` не тронуты — `risk-critic` не требуется.
- **RED-first:** все `*/tests/**`, проба, гейт и CI-шаг — только коммиты `[architect]`
  (`84888776`, `7a05af4c`, `8349f7e8`, `f283457f`, `2a9778d0`); реализация — после
  (`af13e7b3`…`6b2655d1`). `374f6a5c` (architect) — однострочная замена
  `(n + 1) / 2` → `n.div_ceil(2)` в фикстуре по SCOPE VIOLATION REQUEST, для `usize`
  тождественно.
- **Атомарность:** по коммиту на задачу (3+5 вместе — помечено в subject), метки ролей есть.
- **Plan-time:** `C-298` REJECT → `C-306` NOTE.
- **Сторож:** `check_restore_drill`/`run_cycle_with_drill` по форме совпадают со спекой §5;
  бинарь всегда передаёт `Some(&verdict)`.

## §6. Условие APPROVED

1. Б-1…Б-4 устранены, и каждое закрытие подтверждено оракулом, который краснеет на
   сегодняшней реализации (исправление по вердикту тоже требует RED — `testing.md`):
   успешный транспорт в ветке `remote`; строка cron'а в форме cron'а; существование образа
   читателя; композиция пути состояния хост ↔ контейнер recorder'а.
2. Решение по Н-1 (`checked`) — принять как отклонение спекой или привести код к таблице.
3. Ветка синхронизирована с `main`; `verify_M-99.sh` exit 0 на её вершине.

## Done Block

```
$ git fetch origin && git rev-parse origin/feat/M-99-restore-drill-automation
374f6a5c2e39e6e85467ab8ab0e3c1773ca43a5b
$ git merge-base --is-ancestor 374f6a5c origin/feat/M-99-restore-drill-automation; echo $?
0
$ git merge-base origin/main HEAD
a660b3a7b78b116a0a98bc7c9d9eb1e03153725c

$ bash scripts/verify_M-99.sh   (своё дерево /tmp/hft-reviewer-m99, без merge main; real 33m1s)
PASS-строк: 84
FAIL: ci-parity: cargo audit (exit=1)
PASS: ci-parity: cargo test --all
PASS: ci-parity: bash scripts/tests/red_restore_drill.sh
PASS: ci-parity: bash scripts/tests/red_deploy_catchup.sh --battery
PASS: ci-parity: учтено шагов 65 из 65 (исполнено 58, исключено по карте 6)
SKIP: task #8 — первый живой прогон на VPS TRANSPORT=remote (A-032 §2.4) — снимает reviewer
VERDICT: FAIL (провалов: 1)
exit=1

$ cargo audit | grep -E 'Crate|ID'
Crate: quinn-proto  ID: RUSTSEC-2026-0342 / -0339 / -0341 / -0338
audit_exit=1
$ diff <(git show origin/main:Cargo.lock) Cargo.lock | head -2
955c955
< version = "0.11.19"   (main)   > version = "0.11.16"   (ветка; ветка Cargo.lock не трогала)

$ bash repro_remote.sh   (Приложение)
── control-local (TRANSPORT=local) rc=0         … файлов в --dir: 6 … "ok":1,"events_read":4125
── remote-ok-transport (TRANSPORT=remote) rc=5  … delivered: 11 файлов … файлов в --dir: 0 … "ok":0 reader-rc-5
── cron-form (flock -n LOCK <обёртка>) rc=0     state: <НЕТ ФАЙЛА>; log: SKIP предыдущий прогон ещё работает

$ ssh … 'docker inspect hft-recorder --format "{{.Config.Image}}"; docker image inspect hft-recorder; echo rc=$?'
hft-platform-recorder:local
inspect hft-recorder rc=1
$ ssh … 'docker inspect hft-recorder --format "{{range .Mounts}}…"; docker exec hft-recorder ls /var/lib/hft'
/var/lib/docker/volumes/hft-platform_journal-data/_data -> /journal true
ls: cannot access '/var/lib/hft': No such file or directory
$ ssh … 'cd /root/hft-platform && git rev-parse --short HEAD'
dc03af35
```

## Handoff §D

Следующий агент — **`architect`** (разбор класса находок Б-1…Б-4 и Г-1…Г-4; затем, по его
решению, — engine-dev на правку реализации).

## Приложение — скрипт воспроизведения Б-1/Б-2 (`repro_remote.sh`, запуск из корня дерева ветки)

```bash
#!/usr/bin/env bash
# Reviewer repro M-99: ветка TRANSPORT=remote обёртки на УСПЕШНОМ транспорте + двойной flock.
# Транспорт-заглушка — самый благоприятный мыслимый: копирует ВСЮ копию без ошибок.
set -u
ROOT=/tmp/hft-reviewer-m99
W="${ROOT}/deploy/bin/journal-restore-drill-cron.sh"
D="$(mktemp -d /tmp/rv-m99-XXXXXX)"
cd "${ROOT}"
out="$(DRILL_FIXTURE_OUT="${D}" DRILL_FIXTURE_VARIANT=healthy cargo test -p journal --test fixture_restore_drill_cold --quiet -- --exact materialize_for_shell_probe --nocapture 2>&1)"
printf '%s\n' "${out}" | grep -q DRILL_FIXTURE_READY || { echo "SETUP FAIL fixture"; exit 2; }
cargo build -q -p journal --bin journal-drill-read || { echo "SETUP FAIL reader"; exit 2; }
R="${ROOT}/target/debug/journal-drill-read"
echo "fixture cold: $(ls "${D}/cold" | wc -l) файлов; индексов: $(ls "${D}/cold" | grep -c '^segment-.*\.jrnl$')"

cat > "${D}/stub-transport.sh" <<'EOF'
#!/usr/bin/env bash
echo "TRANSPORT-STUB argv: $*" >> "${STUB_LOG}"
cp -a "$1"/. "$2"/ && echo "TRANSPORT-STUB delivered: $(ls "$2" | wc -l) файлов" >> "${STUB_LOG}"
EOF
cat > "${D}/spy-reader.sh" <<EOF
#!/usr/bin/env bash
echo "READER-SPY argv: \$*; файлов в --dir: \$(ls "\$2" 2>/dev/null | wc -l)" >> "\${STUB_LOG}"
exec "${R}" "\$@"
EOF
chmod +x "${D}"/*.sh

run() { # $1=метка $2=TRANSPORT [flock-prefix...]
  local tag="$1" tr="$2"; shift 2
  local box="${D}/${tag}"; mkdir -p "${box}"; : > "${box}/stub.log"
  STUB_LOG="${box}/stub.log" JOURNAL_DRILL_COLD="${D}/cold" JOURNAL_DRILL_RESTORE="${box}/restore" \
  JOURNAL_DRILL_STATE="${box}/state.json" JOURNAL_DRILL_TRANSPORT="${tr}" \
  JOURNAL_DRILL_REMOTE="bash ${D}/stub-transport.sh" JOURNAL_DRILL_READER="bash ${D}/spy-reader.sh" \
  JOURNAL_DRILL_LOG="${box}/drill.log" JOURNAL_DRILL_ALERT_FILE="${box}/alert" \
  JOURNAL_DRILL_LOCK="${D}/drill.lock" "$@" bash "${W}" >/dev/null 2>&1
  local rc=$?
  echo "── ${tag} (TRANSPORT=${tr}${*:+, prefix: $*}) rc=${rc}"
  sed 's/^/   /' "${box}/stub.log"
  echo "   state: $(cat "${box}/state.json" 2>/dev/null || echo '<НЕТ ФАЙЛА>')"
  echo "   log:   $(tail -1 "${box}/drill.log" 2>/dev/null)"
}
run control-local local
run remote-ok-transport remote
run cron-form local flock -n "${D}/drill.lock"
rm -rf "${D}"
```
