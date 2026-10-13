<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 6ef8c672f3895e85ed73628c81ab3d3089638d69
audited_head: ac94efbfc8adb5e015607d3f180ba4d1feffe09e
verdict: REJECT
-->

# R-221 — M-92, PR-гейт задач 1–3 на вершине ветки

**Дата (UTC):** 2026-10-03
**Предмет:** ветка `origin/feat/M-92-manifest-verified-prune`; вершина взята командой
(`git fetch origin && git rev-parse`) — `ac94efb`. Справка мандата (вердикт tester'а): `9643591`
— предок вершины (`merge-base --is-ancestor` = 0). **Ветка ушла вперёд:** `9643591` (задача 3, D5c)
→ `ac94efb` (задача 1, исправление по `R-217`). Судится ВЕРШИНА. Tester прогонял `9643591`, на
`ac94efb` прогона tester'а нет — гейты ниже прогнаны мной на `ac94efb`.
Повторный fetch перед записью: вершина та же (`ac94efb`).

**Вердикт: REJECT (CHANGES REQUESTED) → architect.**
Гейт `verify_M-92.sh` на вершине ЗЕЛЁН (56 PASS, exit=0), закрытие `R-217` по букве оракулов
подтверждено. Отказ — по двум находкам, обе доказаны исполнением и обе НЕ ловятся оракулами:
(Б-1) прод-путь cron'а мёртв в настоящей топологии контейнера; (Б-2) граница удаления
пропускает известные плану позиции без обрыва — дыра в каталоге при `failed = 0`.

## Живые инварианты FA (`crates/journal/**` → `docs/fa/journal.md`)

- **`JR-I-2`** (`docs/fa/journal.md:112`) — «`seq` строго монотонен без дыр; разрыв при чтении →
  abort». Б-2 — её нарушение уборщиком.
- **`JR-I-13`** (`docs/fa/journal.md:221` на вершине) — «удаление только по сверке с офсайт-копией».
  Б-1 делает его невыполнимым на проде: сверка не начинается.

Ярус C, что искал грепом (`origin/main`): `TECH-DEBT.md` — `M-92`, `TD-020`, `TD-202` (карточка
TD-202 — ТОТ ЖЕ класс, что Б-1: путь, которого нет в контейнере ⇒ механизм молчит);
`PROJECT-STATE.md` — `M-92` (0), `M-90` (✅ ЗАКРЫТ, `R-215` — условие спеки §12 «вливать после
M-90» выполнено).

## Находки

### Б-1 (блокер) — cron передаёт бинарю в контейнере ПУТИ ХОСТА; прод-путь падает на шаге 1

`deploy/bin/journal-retention-cron.sh`: `--plan-out "${RETENTION_WORK_DIR}/plan.txt"` (ARGV, шаг 1)
и `--cold-manifest "${MANIFEST_FILE}"` = `${RETENTION_WORK_DIR}/manifest.txt` (APPLY_ARGV, шаг 5).
`RETENTION_WORK_DIR=/var/lib/hft/retention-work` (`deploy/cron.d/journal-retention`) — путь ХОСТА;
в контейнер он смонтирован как `/work` (`docker-compose.yml`, сервис `journal-retention`), а
`/var/lib/hft` в образе нет (`Dockerfile:27` создаёт только `/journal /cold`). Спека §4 п.1
задаёт ДОСЛОВНО `--plan-out=/work/plan.txt` и `--cold-manifest=/work/manifest.txt`.
`write_plan_out` (`journal-retention.rs:690`) каталог не создаёт ⇒ `ENOENT` ⇒ exit 1.

**Почему оракулы зелёные.** Заглушка `run_wrapper` (`red_m92_manifest_prune.rs:820-935`) подменяет
в argv только ЦЕЛИ томов (`/work` → каталог хоста) и исполняет бинарь НА ХОСТЕ. Путь хоста, который
скрипт передаёт как есть, на хосте существует — бинарь его находит. Обещание шапки заглушки «цель,
которой нет в томах сервиса, НЕ подключается — бинарь её не найдёт» не выполняется для путей хоста.
Класс — `testing.md` «Механизм несущего пути обязан иметь оракул точки входа», п.2 (КОМПОЗИЦИЯ:
путь продюсера ≠ путь консюмера).

**Воспроизведение** (настоящий `journal-retention-cron.sh`, настоящий бинарь `target/debug`,
`RETENTION_RUNNER` = `docker run` с томами РОВНО как у сервиса: `journal:/journal`,
`ckpt:/ckpt:ro`, `hostwork:/work`; образ `ubuntu:24.04`, бинарь примонтирован в
`/usr/local/bin/journal-retention`):

```
ALERT plan step exit=1 (1=arg/io, 3=disk_pressure). Лог: …/probe-b1/log
exit=1
--- log:
journal-retention: не удалось записать --plan-out …/probe-b1/hostwork/plan.txt: No such file or directory (os error 2)
```

**Контроль** (тот же контейнер, единственная разница — путь `/work/plan.txt`):

```
exit=0
-rw-r--r-- 1 root root    0 Oct  3 18:57 plan.txt      # появился в каталоге хоста
```

Следствие на проде: каждый ежедневный прогон — тревога `plan step exit=1`, ни план, ни манифест,
ни аудит-след не пишутся; задача 4(а) («dry-run прод-пути — план и манифест в аудит-следе»)
неисполнима. Fail-closed (данные не теряются), но механизм built-not-wired — `gates.md` §4 DoD.

Дизайн защиты (заглушка, исполняющая бинарь в изоляции томов, либо проверка, что каждый путь
argv лежит под целью тома) и RED на неё — зона architect'а; правка скрипта — engine-dev.

### Б-2 (блокер) — известная плану НЕ-кандидатная позиция пропускается без обрыва ⇒ дыра

Спека §3 `I-2ter`: «позиция удаляется, только если КАЖДЫЙ её файл — кандидат плана с выданным
`ColdCopyProof`; **первая позиция, не удовлетворяющая этому**, … обрывает удаление».
Реализация (`segments.rs`, `retention_execute_with_manifest`, ветка `plan_known.contains(…)` →
`continue`) обрывает только на позиции ВНЕ плана; позиции из `skipped`/`offload_only` (моложе
`retain_days`, `keep_min`, без покрытия) пропускаются, и удаление идёт дальше. Обоснование в
комментарии — «иначе `p1` зарезал бы `keep_min`-сегмент и активный, оставив `pruned` пустым» —
**ложно** (мутация ниже).

Достижимость: возрастной фильтр `retention_plan` не монотонен по индексу — `segment_decision_ts`
= `ts_exch` ПЕРВОГО события сегмента (fallback — `created_wall_ms`, другие часы). Средний сегмент,
чьё первое событие несёт более позднее биржевое время (другая площадка, лаг), на границе
`retain_days` оказывается «моложе» соседей.

**Воспроизведение** (временный `crates/journal/tests/zz_reviewer_probe.rs` в моём дереве, удалён;
фикстура как `world()` из `red_m92`: 900 событий, сегменты 8 KiB, `keep_min=1`, покрытие — всё,
кроме активного; у ПЕРВОГО события сегмента 2 `ts_exch = now − 1 s`, остальные — как в `red_m92`;
манифест — честные суммы по именам плана):

```
PROBE catalog before: [segment-00000000 … segment-00000005]
PROBE plan.offload_and_prune: ["segment-00000000.jrnl", "segment-00000001.jrnl", "segment-00000003.jrnl"]
PROBE skipped: segment-00000002.jrnl — younger than retain_days: age=1000ms < 86400000ms (seg_ts=1760639999000)
PROBE pruned=3 failed=0
PROBE catalog after: ["segment-00000002.jrnl", "segment-00000004.jrnl", "segment-00000005.jrnl"]
assertion `left == right` failed: ДЫРА в каталоге: [2, 4, 5]
```

Разрыв 2 → 4, отчёт `failed=0`, выход бинаря был бы 0 — ровно класс, ради которого введены
`I-2bis`/`I-2ter` (`A-045` §3: ни один читатель дыру не видит).

**Мутационный контроль (свой).** Ветка `plan_known` → `blocked_by = Some(name); continue;`
(буква `I-2ter`):

```
red_m92_manifest_prune       test result: ok. 19 passed; 0 failed
red_retention*  (4 файла)    test result: ok. 3 / 6 / 9 / 7 passed; 0 failed
zz_reviewer_probe            test result: ok. 1 passed; 0 failed
```

То есть (а) реализация по букве спеки проходит ВСЕ оракулы — отступление ничем не вынуждено;
(б) оракулы НЕ различают обе формы — `I-2ter` для известных не-кандидатов не запиннена ни одной
фикстурой. Код возвращён (`git status --porcelain` пуст).

RED на этот случай — architect; правка — engine-dev.

### Н-1 — отсутствие копии на коробке обрывает ВЕСЬ прогон, `absent-remote` на проде недостижим

Шаг 2 скрипта: `${RETENTION_REMOTE_SHA_CMD} journal/<имя> | sed …` под `pipefail`;
`sha256sum` отсутствующего файла выходит ≠0 ⇒ тревога, `exit 1`, ничего не удаляется, в т.ч.
сверенные старшие. Fail-closed, но расходится со спекой `I-2` («совпавшие СТАРШЕ него удаляются»).
Там же: одно SSH-соединение на сегмент (≈300 на первом прогоне после включения). Решение — architect.

### Н-2 — план считается дважды (шаг 1 и шаг 5) с разным «сейчас»

Новый кандидат, появившийся между шагами, на шаге 5 не имеет строки манифеста ⇒ `absent-remote`,
обрыв, `exit 2`, тревога — fail-closed, но ложная тревога; в отчёт аудит-следа он не попадает
(отчёт строится по файлу плана шага 1). Наблюдение.

### Н-3 — «pruned» в отчёте выводится из ОТСУТСТВИЯ строки `FAIL`

Предел назван в самом скрипте (`A-045` §5 п.3). Остаётся открытым; к сведению architect'а.

### Н-4 — вердикт tester'а судил не вершину

Tester прогнал `9643591`; к моменту ревью вершина `ac94efb`. Его G3 (`verify_M-48.sh` exit=1,
три теста `p1`/`p10`/`p11`) на вершине ЗЕЛЁН — дрейфа нет, красное было задачей 1, как он и
утверждал. Его §E-вопрос («те же три теста, а не другие») закрыт: на `ac94efb` — ноль красных.

### Н-5 — кандидат TD из `R-217` Н-4 / `A-045` §6 N-1

«Читатель журнала обязан отказать на разрыве `seq` между сегментами» — по-прежнему без карточки;
заводится reviewer'ом на close-out M-92.

## Закрытие `R-217` на вершине — подтверждено

| пункт R-217 | состояние на `ac94efb` |
|---|---|
| Б-1 обрыв по каталогу (позиция ВНЕ плана) | закрыт: `p10` PASS |
| Б-2 удаление только `prune_segment(seg, proof)` | закрыт: шаг `task1-proof` PASS; proof чеканит приватная `verify_segment_against_manifest` |
| Н-1 DryRun = Apply | закрыт: `p11` PASS; DryRun хеширует |
| Н-2 `offloaded` пуст | закрыт: `p1` PASS |
| Н-3 двойной хеш | закрыт: одна сумма на исход `Mismatch` |

## Scope / дисциплина — чисто

- Коммиты engine-dev (`80b363f`, `6b1fa76`, `6003fe3`, `48927a7`, `9643591`, `ac94efb`) — только
  Allowed paths §7 (`crates/journal/src/{segments,lib}.rs`, `bin/journal-retention.rs`,
  `deploy/bin/journal-retention-cron.sh`, `deploy/cron.d/journal-retention`, `docker-compose.yml`,
  `deploy/README.md`). Атомарны, со ссылкой на задачу.
- `*/tests/**`, `scripts/verify_*`, `scripts/tests/**` — коммиты ТОЛЬКО architect'а (10 шт.).
- `crates/contracts/**` не тронут — Block-C N/A. `risk`/`killswitch`/`oms`/`venue-*` не тронуты —
  RISK-BLOCK не применяется; RAW-гейт критика на сильной модели пройден
  (`C-268`→`C-270`→`A-044`→`C-274`→`A-045`→`C-275` NOTE).
- `deploy/cron.d/journal-retention`: `RETENTION_MODE=dry-run` сохранён; режим — только
  файл-переключатель; файл кодом не создаётся (§5) — соблюдено.
- Статус-колонка §8 (задачи 1–3) не обновлена — ведёт architect.

## Done Block (сырой вывод, агрегирован)

```
$ git fetch origin && git rev-parse origin/feat/M-92-manifest-verified-prune
ac94efbfc8adb5e015607d3f180ba4d1feffe09e
$ git merge-base --is-ancestor 9643591 origin/feat/M-92-manifest-verified-prune; echo anc=$?
anc=0

$ bash scripts/verify_M-92.sh       (дерево /tmp/hft-reviewer-m92 @ ac94efb)
PASS  task1-3: red_m92_manifest_prune (p1-p11, b1-b2, c0-c5)
PASS  task1: red_retention + red_retention_checkpoint_coverage + red_retention_compacted + red_retention_operator
PASS  task1-proof: удаление на манифестном пути — только prune_segment(seg, ColdCopyProof)
PASS  task3: verify_M-48.sh
PASS  task5: JR-I-13 определён в docs/fa/journal.md (…); «первый свободный» сдвинут
PASS  ci-map: проба карты CI-паритета (red_verify_M-92_ci_map.sh; число миров печатает проба)
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
…
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 49, исключено по карте 6)
VERDICT: PASS
exit=0                               (PASS-строк: 56; FAIL: 0)

$ Б-1 проба (docker, тома сервиса)   → exit=1, ALERT plan step exit=1, ENOENT на --plan-out
$ Б-1 контроль (--plan-out /work/…)  → exit=0, plan.txt в каталоге хоста
$ Б-2 проба на ac94efb               → FAILED: ДЫРА в каталоге: [2, 4, 5]; pruned=3 failed=0
$ Б-2 мутант «буква I-2ter»          → red_m92 19/19, red_retention* 25/25, проба 1/1 — всё ok
$ git status --porcelain (после возврата мутанта и удаления пробы)
<пусто>
```

## Условие APPROVED

1. Б-1: прод-путь доказан оракулом, исполняющим бинарь так, что пути вне томов сервиса ему
   НЕДОСТУПНЫ (или эквивалентная проверка композиции argv ↔ цели томов); скрипт передаёт
   `/work/…`. Проба выше — красная на `ac94efb`, зелёная после правки.
2. Б-2: фикстура «известная плану не-кандидатная позиция между кандидатами» в RED; реализация —
   обрыв по букве `I-2ter`. Проба выше — зелёная.
3. `verify_M-92.sh` exit=0 на новой вершине; прогон tester'а на ней.
4. Н-1 — решение architect'а (принять как fail-closed с записью в спеке либо изменить).
