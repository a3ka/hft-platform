<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: 2ec2eaec211ba3540284a2a71934d59d0225bc9a
audited_head: 371203b0f1466580bb354b48bf0f6089ea1db9b4
verdict: REJECT
-->

# R-266 — M-99 restore-drill automation, PR-гейт круг 2: REJECT

FA-WAIVER: crates/recorder — FA для крейта recorder не существует (reading-map §2); опора — OPS-I-10 из docs/fa/ops.md

## §0. Предмет

- Ветка `origin/feat/M-99-restore-drill-automation`, вершина взята командой после `git fetch`:
  `371203b0` (= справке tester'а). Повторный fetch перед записью вердикта — без сдвига.
- База аудита — `merge-base origin/main HEAD` = `2ec2eaec`; `origin/main` = `815d735a`
  (ветка отстаёт на 2 коммита, оба — `PROJECT-STATE`/`TECH-DEBT`/`ROADMAP` close-out M-100).
- Прочитано по предмету: спека `M-99` целиком; `R-263` (мой круг 1); `C-314`, `C-316`; `A-055`
  §0–§1, §6; `docs/fa/ops.md` — `OPS-I-2`, `OPS-I-3`, `OPS-I-10`, `OPS-I-11`; `docs/fa/journal.md`
  `JR-I-6`; `П-028`, `П-031` в `PENDING-SIGNATURE.md`. Ярус C — грепом: `M-99` в
  `PROJECT-STATE.md`; ничего иного не открывалось.
- Инварианты тронутых модулей: **`OPS-I-3`** (холодная копия периодически восстанавливается и
  читается), **`OPS-I-10`** (объявлена ⟹ эмитится), **`JR-I-6`** (старый журнал читается новым
  кодом). Б-1 ниже — о T2-контракте файла состояния, на котором стоят оба потребителя
  `OPS-I-3`/`OPS-I-10`.

## §1. Вердикт

**REJECT — один блокер, узкий.** Работа круга 2 по существу хорошая: все четыре блокера `R-263`
закрыты и закрытие предъявлено исполнением (§3). Но реализация обёртки нарушает запрет,
названный в `M-99` §7 поимённо, — пишет `ok=1` без `digest`, — и ни один оракул набора этого
не ловит. Вторая половина — пробел ОРАКУЛА, то есть зона architect'а, поэтому маршрут —
architect (профиль reviewer'а §Handoff; дизайн исправления здесь не предлагается, `gates.md` §4).

**Счёт кругов (`A-055` §6 п.4).** Находка касается `M-99` §4 п.4 / §7, а НЕ §4 п.7–10.
Условие «СТОП, предмет founder'у» сформулировано арбитром только для п.7–10, поэтому здесь оно
не срабатывает: это блокирующая находка иной клетки со своим счётом. Решать, не тот же ли это
класс по существу («обещание спеки без оракула»), — не мне; если architect сочтёт, что тот же, —
развилка `A-055` §6 п.3 (свежий арбитр).

## §2. Блокер

### Б-1 — обёртка пишет `ok=1` без `digest` (запрет `M-99` §7; T2-контракт `M-74`)

`deploy/bin/journal-restore-drill-cron.sh`, хвост скрипта: при успехе читателя (`RC=0`,
непрерывность `true`) и отпечатке, не прошедшем `^[0-9a-f]{64}$`, обёртка делает
`write_state 1 "${EV:-0}" "" "missing-digest-on-success" "${CHECKED}"`, `exit 4`.

`M-99` §7, строка таблицы «ЗАПРЕЩЕНО»: «считать drill пройденным по `exit=0` обёртки; **писать
`ok=1` без `digest`** — `R-157` Б-5; T2-контракт». `M-74` T2: «`digest` обязателен при `ok=1`».
Запрет относится к ПИСАТЕЛЮ, а не только к потребителю: для потребителя есть отдельное
требование `M-99` §4 п.4.

**Воспроизведение** (локальный транспорт, читатель-заглушка печатает форму прод-читателя
с пустым отпечатком и выходит с `0`):

```
$ JOURNAL_DRILL_TRANSPORT=local JOURNAL_DRILL_COLD=$S/cold … JOURNAL_DRILL_READER="bash $S/reader.sh" \
  bash deploy/bin/journal-restore-drill-cron.sh; echo wrapper_exit=$?
wrapper_exit=4
$ cat $S/state.json
{"ok":1,"ts_wall_ms":1791985887000,"checked":3,"events_read":7,"digest":"","reason":"missing-digest-on-success"}
```

**Чего это стоит — названо честно, без раздувания.** Оба потребителя (`restore_drill_verdict`
→ recorder и `ops-watchdog`) такое состояние отвергают (`Unparseable` ⇒ метрика `0`, тревога
`WD-RESTORE-DRILL`), то есть fail-closed держится на стороне чтения. Наш `journal-drill-read`
при `rc=0` всегда печатает 64 строчных hex (`format!("{:x}", sha256)`), так что ветка
достижима только при подменённом/сломанном читателе. Блокером это делает не тяжесть исхода,
а то, что (1) нарушен ПОИМЁННЫЙ запрет спеки, и (2) **у запрета нет оракула**: проба
(`red_restore_drill.sh`, 33/33) зелёная при этой строке — `grep -n 'missing-digest\|без digest'`
по пробе пуст. Файл состояния с `"ok":1` — ровно «заявление о действии» (класс `C-191`), против
которого заведён T2, и он лежит на диске, который читает оператор по runbook'у.

**Происхождение:** строка введена `0d6292cc` (круг 1, задача 2) и пропущена `R-263` — моим
вердиктом. Пропуск reviewer'а не делает её допустимой; называю, чтобы не выдавать находку за
новую регрессию круга 2.

Условие снятия — в §6.

## §3. Закрытие `R-263` — предъявлено исполнением, а не таблицей

| `R-263` | что было | что предъявлено на `371203b0` |
|---|---|---|
| Б-1 | ветка `remote` не могла закончиться успехом | проба `RT`: «ветка remote на успешном транспорте: ok=1, отпечаток, состав и формы сходятся»; `RW` — транскрипт коробки только белым списком; `T`/`TF` — отказ листинга и ОДНОЙ выборки ⇒ `transport-failed`, код 5 |
| Б-2 | двойной `flock` — SKIP каждый прогон | `deploy/cron.d/journal-restore-drill`: строка без `flock`; проба `CR` — строка в форме cron'а исполнила drill, `ok=1`; `CL`/`CL-M` — пропуск тика без следа, 8 мутантов пойманы |
| Б-3 | образ читателя = имя контейнера | дефолт `READER` — `hft-platform-recorder:local` с `--network none` и `-v R:R:ro`; проба `IM` PASS; совпадает с `image:` сервиса `recorder` (`docker-compose.yml:15`) и с образом живых контейнеров на VPS (§Done Block) |
| Б-4 | recorder не видел файл состояния | путь `/var/lib/hft/drill-state/…`; ОДНА строка тома `:ro` в `recorder`; recorder берёт `DEFAULT_RESTORE_DRILL_STATE`, литерала нет (гейт task2); ops `v5`/`v5m` PASS |
| Н-1…Н-3 | приоритет таблицы §5, `checked`, `saturating_sub`, строчные hex | `restore_drill.rs` — порядок проверок совпадает с таблицей §5; `v4` PASS |
| Н-4 | плацебо-тест в читателе | удалён (`68ca08d4`) |
| Н-5 | колонка Status | обновлена (`371203b0`) |

Встраивание в прод-путь (`gates.md` §4 «механизм на пути»): деплой ставит ВСЕ `deploy/cron.d/*`
в `/etc/cron.d/hft-<имя>` после `crontab -n` (`deploy/bin/deploy-apply.sh` шаги 3/установка);
образ получает `journal-drill-read` (`Dockerfile`, `--bin` + `COPY`); обёртка исполнима
(`100755`); на VPS есть `/root/.ssh/storagebox`, `rsync`, `flock`. Built-not-wired нет.

## §4. Не блокеры

- **Н-1 — `cargo audit` красен, но НЕ из-за M-99.** `rustls 0.23.45`, `RUSTSEC-2026-0346…0349`.
  `git diff --quiet origin/main HEAD -- Cargo.lock` ⇒ 0: lock-файл ветки ИДЕНТИЧЕН `main`, то
  есть следующий прогон CI на `main` покраснеет так же. Это отдельный предмет (обновление
  зависимости), не долг M-99; но он ФИЗИЧЕСКИ блокирует merge любого PR через агрегат
  `All checks passed`. Tester получил `verify_exit=0` раньше — база уязвимостей обновилась между
  прогонами; его Done Block не ложен.
- **Н-2 — устаревшие тексты, расходящиеся с кодом:**
  `crates/ops/src/bin/ops-watchdog.rs` док-комментарий — дефолт `/var/lib/hft/journal-restore-drill.json`
  (прежний путь; код берёт `DEFAULT_RESTORE_DRILL_STATE` — верно); комментарий обёртки перед
  чтением — «каталог существует и ПУСТ» (после круга 2 там доставленные байты); `Dockerfile` —
  «из образа `hft-recorder`» (имя контейнера, то, что `R-263` Б-3 и исправил).
- **Н-3 — runbook `deploy/README.md` обещает больше, чем делает обёртка:** причины
  `sidecars-missing`, `source-stayed-present`, «`transport-failed` (DRILL_REMOTE вариант)» обёртка
  не пишет никогда; «проверяются на непустоту» про sidecar'ы — обёртка их не проверяет; команда
  ручного перечитывания `journal-drill-read --dir …` — на хосте бинаря нет, он в образе (нужен
  `docker run … --entrypoint`).
- **Н-4 — нет предела времени у транспорта.** `ConnectTimeout=10` ограничивает только
  соединение; зависший `rsync` держит замок, и следующие прогоны молча уходят в SKIP. Наблюдаемо
  (сторож поднимет `Stale` через 40 суток), но поздно. Предел назван, решения не предлагаю.
- **Н-5 — тесты в `src/` от dev'а.** `#[cfg(test)] mod tests` в `crates/ops/src/restore_drill.rs`
  (`af13e7b3`/`293f9545`, engine-dev) — тесты на собственную реализацию (`testing.md`). Не
  sacred-зона, оракулы architect'а независимы; отмечаю как отклонение от нормы.

## §5. Что проверено и в порядке

- **Scope:** диф `2ec2eaec..371203b0` — 44 файла, все в Allowed paths `M-99` §6; граница §7
  (путь удаления, `crates/journal/src/**` кроме нового бинаря, `RETENTION` в compose,
  `crates/contracts/**`) не тронута — шаги «граница» гейта PASS.
- **RED-first / sacred:** ни один коммит с меткой `[engine-dev]` не трогает `*/tests/**` и
  `scripts/**` (перебор по `git show --name-only`); все оракулы, проба, гейт и шаг CI —
  коммиты `[architect]`.
- **Атомарность:** круг 2 — по коммиту на задачу (2, 2b, 3, 4, 5, 6, Н-5), метки ролей есть.
- **Block-C:** `crates/contracts/**` не тронут. **Risk-блок:** `risk`/`killswitch`/`oms`/`venue-*`
  не тронуты — `risk-critic` не требуется.
- **Plan-time:** `C-298` REJECT → `C-306` NOTE → `R-263` REJECT → `C-311` REJECT → `C-313`
  ESCALATE → `A-055` DECISION → `C-314` REJECT (иной класс: мерж-конфликт `DESIGN` §22) →
  `C-316` NOTE.
- **Ярус S:** `main` зелёный (последние 5 прогонов success), прод = `2ec2eaec`, `main` впереди
  только документами; `check_branch_health.sh` — PASS, дублей и висяков нет.

## §6. Условие APPROVED

1. Б-1 устранён в реализации, и закрытие подтверждено оракулом, который краснеет на сегодняшней
   обёртке (исправление по вердикту требует RED — `testing.md`).
2. Merge — на зелёном `cargo audit`: Н-1 решается отдельным предметом в `main` до или вместе
   с синхронизацией ветки; без этого агрегат CI merge не пропустит.
3. Н-2/Н-3 — желательно тем же кругом (правки текста), не условие.
4. После merge — задача 8 (первый живой прогон `TRANSPORT=remote`) за мной, по `M-99` §13.

## Done Block

```
$ git fetch origin && git rev-parse origin/feat/M-99-restore-drill-automation
371203b0f1466580bb354b48bf0f6089ea1db9b4
$ git rev-parse origin/main; git merge-base origin/main HEAD
815d735a4c3c8b2c4f7ebfc91a30f3abdc41af09
2ec2eaec211ba3540284a2a71934d59d0225bc9a

$ bash scripts/verify_M-99.sh   (своё дерево /tmp/hft-reviewer-m99, вершина ветки; real 30m14s)
PASS-строк: 87
FAIL: ci-parity: cargo audit (exit=1)
PASS: ci-parity: учтено шагов 65 из 65 (исполнено 59, исключено по карте 6)
SKIP: task #8 — первый живой прогон на VPS TRANSPORT=remote (A-032 §2.4) — снимает reviewer
VERDICT: FAIL (провалов: 1)
verify_exit=1

$ cargo audit | grep -E 'Crate|Version|ID'
Crate: rustls  Version: 0.23.45  ID: RUSTSEC-2026-0347 / -0349 / -0346 / -0348
$ git diff --quiet origin/main HEAD -- Cargo.lock; echo lock_same_as_main=$?
lock_same_as_main=0

$ bash scripts/tests/red_restore_drill.sh | grep -E '^(PASS|FAIL)  (T|TF|RT|RW|CR|CL|IM)\b|VERDICT'
PASS  T отказ листинга: ssh вызван (1×), ok=0, причина transport-failed, код 5, без отпечатка
PASS  TF отказ ОДНОЙ выборки (1× вызвано) при живом листинге: ok=0, причина transport-failed
PASS  RT ветка remote на успешном транспорте: ok=1, отпечаток, состав и формы сходятся
PASS  RW транскрипт коробки — только белый список (листинг 1×, выборка одного файла)
PASS  CR строка расписания в форме cron'а исполнила drill: ok=1
PASS  CL при занятом замке обёртки строка cron'а пропустила тик: код 0, новая строка SKIP
PASS  IM читатель по умолчанию — из образа recorder'а «hft-platform-recorder:local», без сети
VERDICT: PASS (33/33) — копия читается прод-читателем, исходы различимы
probe_exit=0

$ (Б-1) читатель-заглушка rc=0, "digest":"" → обёртка
wrapper_exit=4
{"ok":1,"ts_wall_ms":1791985887000,"checked":3,"events_read":7,"digest":"","reason":"missing-digest-on-success"}

$ crontab -n deploy/cron.d/journal-restore-drill; echo rc=$?
The syntax of the crontab file was successfully checked.
rc=0
$ git ls-files -s deploy/bin/journal-restore-drill-cron.sh
100755 62e5a7b0… deploy/bin/journal-restore-drill-cron.sh

$ gh run list --branch main --limit 5   → 5 × completed success (CI #352/#351, Deploy ×3)
$ ssh … 'git rev-parse --short HEAD; ls -la /root/.ssh/storagebox; which rsync flock; docker ps …'
2ec2eaec
-rw------- 1 root root 444 Aug 29 10:12 /root/.ssh/storagebox
/usr/bin/rsync
/usr/bin/flock
hft-gateway-serve hft-platform-recorder:local Up 17 hours (healthy)
hft-recorder hft-platform-recorder:local Up 17 hours (healthy)
$ bash scripts/check_branch_health.sh | grep -E 'M-99|VERDICT'
feat/M-99-restore-drill-automation  2  39  32 —  0 сут
VERDICT: PASS — наблюдение состоялось
```

## Handoff §D

Следующий агент — **`architect`**: разбор Б-1 (реализация + отсутствующий оракул запрета §7);
Н-1 (`rustls`) — отдельный предмет, блокирующий merge любого PR. Затем, по его решению, —
engine-dev на правку обёртки → tester → reviewer, круг 3.
