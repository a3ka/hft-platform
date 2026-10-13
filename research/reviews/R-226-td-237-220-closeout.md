<!-- GATE-META
milestone: TD-237
audited_repo: a3ka/hft-platform
audited_base: 947991c46b44b8c9fb74d785a32627fe0ff3572b
audited_head: 947991c46b44b8c9fb74d785a32627fe0ff3572b
verdict: NOTE
-->

# R-226 — закрытие `TD-237`, сверка `TD-220`, остаток `R-224` Н-1 (реестры к факту после PR #308)

**Роль:** reviewer (владелец `TECH-DEBT.md` / `PROJECT-STATE.md`; колонка «Состояние» `docs/ROADMAP.md`).
**Предмет:** состояние `origin/main` после merge PR #308 — вершина взята командой:
`git fetch origin && git rev-parse origin/main` → `947991c46b44b8c9fb74d785a32627fe0ff3572b`.
Кода диапазон не трогает; это приведение реестров к факту, поэтому вердикт — **NOTE**, а не APPROVE.
**Ветка:** `docs/td-237-220-closeout` от `947991c`.

**Чем грепал ярус C** (`reading-map.md` §2): `TECH-DEBT.md` — `TD-237`, `TD-220`, `TD-231`, `§7.1`,
`OPS-I-11`, `TD-238`/`TD-239` (место вставки); `PROJECT-STATE.md` — `TD-237`, `TD-220`;
`docs/ROADMAP.md` — `тишин`, `TD-23x`, `4bis`, `9-A-bis`.

**FA тронутого модуля.** Диф `crates/**` не трогает (барьер `review-fa` даст `SKIP`); предмет при этом —
`docs/fa/ops.md`, и сверка шла по живым инвариантам `OPS-I-11` (`:479`), `OPS-I-8` (`:476`),
`OPS-I-5` (`:473`).

## §1 — `TD-237`: ЗАКРЫТА

```
$ grep -c 'check_serving_silence' docs/fa/ops.md          # в карточке на заведении было 0
1
$ grep -n '^| OPS-I-11' docs/fa/ops.md | cut -c1-90
479:| OPS-I-11 | **Тишина ВЫДАЧИ — алерт (`TD-237`; механизм построен `M-89`, продюсер …
$ grep -n '^| OPS-I-8' docs/fa/ops.md | grep -o 'тишина ВЫДАЧИ клиентам — `OPS-I-11`'
тишина ВЫДАЧИ клиентам — `OPS-I-11`
$ git log --oneline -5 origin/main
947991c Merge pull request #308 from a3ka/docs/ops-i-11-prod
0d20dbe docs(review): R-225 — перепроверка §9 OPS-I-11 круг 2 APPROVE [architect-recheck]
791748e docs(fa/ops): OPS-I-11 — адрес прод-замера и канал тревоги по R-224 Б-1/Б-2 [architect]
8c11ce0 docs(review): R-224 — перепроверка §9 OPS-I-11 (prod) REJECT [architect-recheck]
f64e72f Merge pull request #307 from a3ka/docs/archive-m93
```

`OPS-I-11` несёт продюсера, потребителя (`check_serving_silence`, `crates/ops/src/watchdog.rs`), форму
дельт, оракулы и абзац исполнения на проде с пределом `П-003`. Предмет карточки снят. Тело — в
`docs/archive/TECH-DEBT-closed-2026-08-16.md` с записью закрытия.

**Находка при закрытии (→ `TD-241`, MINOR).** Условие закрытия `TD-237` включало «снять ссылки на
`OPS-I-8` в комментариях прод-кода». Прежняя сверка (`R-220`) грепала только `crates/gateway-serve/src`.
Шире:

```
$ git grep -n "OPS-I-8" origin/main -- 'crates/**' | grep -i "выдач\|serving\|m89\|m91" | cut -c1-110
origin/main:crates/gateway-serve/tests/red_m91_legacy_counters.rs:12://! `OPS-I-8` (дельты …
origin/main:crates/gateway-serve/tests/red_m91_legacy_counters.rs:242:  … Сердцебиение не видит …
origin/main:crates/gateway-serve/tests/red_m91_legacy_counters.rs:341:  … тишины OPS-I-8 не увидит …
origin/main:crates/ops/src/watchdog.rs:54:    /// M-89 (задача #10, §4.3 / `OPS-I-8` на выдачу): …
origin/main:crates/ops/src/watchdog.rs:214:    /// M-89 (задача #10, §4.3 / `OPS-I-8` на выдачу): …
origin/main:crates/ops/src/watchdog.rs:624:/// M-89 (задача #10, §4.3 / `OPS-I-8` на выдачу): правило молчания …
origin/main:crates/ops/tests/red_m89_serving_silence.rs:2://! … (`I-6`, `TD-220`, `OPS-I-8` распространён на выдачу …
```

Это противоречит `docs/fa/ops.md:476`. Держать `TD-237` открытой ради комментариев значило бы
смешать два предмета: объявление (сделано) и чистку атрибуции (другие зоны — engine-dev и sacred
architect'а). Вынесено отдельной карточкой, предмет не теряется.

## §2 — остаток `R-224` Н-1 → `TD-240` (MINOR)

Карточки на Н-1 не было (`grep -n "§7\.1" TECH-DEBT.md` → пусто). Замер шире названного в `R-224`:

```
$ awk '/^### §7.1/,/^## §O/' docs/fa/ops.md | grep -c "OPS-I-11\|SERVING"
0
$ grep -o '"WD-[A-Z-]*"' crates/ops/src/watchdog.rs | sort -u | wc -l
15
$ grep -c "WD-" docs/fa/ops.md
0
$ sed -n 24,35p crates/ops/tests/red_ops_alerts.rs | head -2
const REQUIRED_INCIDENTS: &[&str] = &[
    "TD-011",      // запись остановилась (P0)
```

Ни один из 15 кодов инцидентов сторожа не числится в §7.1; паритет-оракул сверяет `ALERT_RULES` с
захардкоженным списком из 10 классов и дыру не видит по построению. Форма закрытия — architect.

## §3 — `TD-220`: сверена, ОСТАЁТСЯ ОТКРЫТОЙ

Мандат: «если остаток только `П-003` — закрыть». Остаток НЕ только `П-003`:

```
$ grep -n "refusals_unsupported" crates/ops/src/*.rs
crates/ops/src/watchdog.rs:168:    pub refusals_unsupported: u64,
$ sed -n 629,640p crates/ops/src/watchdog.rs | grep "if d_"
    if d_attempts > 0 && d_refusals_supported > 0 && d_successes == 0 {
```

Карточка сама называет условием закрытия «правило тревоги в `crates/ops` на отказы по бюджету»
(`M-87` §11 п. 5, `A-037` У-7). Поле разбирается, правила нет, решения architect'а «не нужно» нет ни в FA,
ни в `R-222`/`R-224`/`R-225`. Закрыть карточку ссылкой на `П-003` значило бы молча снять требование.
Доставка и исполнение потребителя — закрыты (`R-222`; прод-замер ниже). Индекс-строка переписана:
единственный остаток — решение architect'а; `П-003` — не предмет карточки. Severity MAJOR не трогаю:
цена (отказы по бюджету невидимы как тревога) не изменилась.

## §4 — `docs/ROADMAP.md`, колонка «Состояние»

Строк `TD-237`/`TD-220` в роадмапе нет. Предмет обоих пунктов живёт в двух строках, чьё «Состояние»
противоречило факту:
- **4bis** (`M-93`): было «⏳ набор принят … engine-dev — задачи 1–2» → ✅ поставлен и исполняется.
- **9-A-bis** (сторож доставки выдачи): было «тревога НЕ ВЫЧИСЛЯЕТСЯ, бинарь не собран» → ✅ продюсер и
  потребитель на проде; открытые `П-003` (founder) и `TD-220` (architect) названы.

Правлена только последняя колонка. §8 по этим строкам пройден ранее (`R-222`); собственный eyes-on
reviewer'а:

```
$ ssh … 'date -u; tail -1 /var/log/hft/watchdog.log; cat /var/lib/hft/watchdog.last-success; ls -l /usr/local/lib/hft/ops-watchdog; grep -c prev_serving_heartbeat /var/lib/hft/watchdog.state.json; ls /var/lib/hft/watchdog.alert'
2026-10-04T18:25:16Z
[ops-watchdog] 1791138301805 — норма, ни одно условие не сработало
2026-10-04T18:25:01Z
-rwxr-xr-x 1 root root 5228496 Oct  4 10:39 /usr/local/lib/hft/ops-watchdog
1
ls: cannot access '/var/lib/hft/watchdog.alert': No such file or directory
```

## §5 — Счёт индекса «В ПЛАНЕ»

```
$ awk -v P=$P -v Z=$Z 'NR>P && NR<Z && /^\| \*\*TD-[0-9]+\*\*/' <origin/main:TECH-DEBT.md> | wc -l
74
$ awk -v P=$P -v Z=$Z 'NR>P && NR<Z && /^\| \*\*TD-[0-9]+\*\*/' TECH-DEBT.md | wc -l
75
```
74 − 1 (`TD-237`) + 2 (`TD-240`, `TD-241`) = 75; заголовок исправлен. Номера `TD-240`, `TD-241`,
`R-226` — резервом `scripts/reserve_artifact_id.sh`.

## §6 — Вне мандата, замечено

- `TD-176` (индекс): «не исполняется — бинарь `ops-watchdog` не собирается» — после `M-93` тоже
  устарело в части исполнения; карточку не трогал (предмет — прогноз диска, сверка требует своего
  замера). Кандидат на следующий close-out.
- `R-224` Н-2 (шапка `docs/fa/ops.md:5` «OPS-I-1..9», §O п. 1 про Prometheus+Alertmanager) — изложение,
  зона architect; карточки не завожу (автор приземляет при следующем касании).

## Вердикт: NOTE

Реестры приведены к факту `947991c`: `TD-237` закрыта, `TD-240`/`TD-241` заведены, `TD-220` сверена и
оставлена с точным остатком, две строки роадмапа обновлены. Блокеров нет; кода не касается.

## Handoff §D

Следующий агент — **architect** (разбор открытых остатков, `reviewer.md` §Handoff): `TD-220` (решение о
правиле на `refusals_unsupported`), `TD-240` (§7.1 + охват паритета `WD-*`), `TD-241` (sacred-шапки
оракулов; правка `watchdog.rs` — диспетч engine-dev).

## Done Block

Сырой вывод барьеров — в теле PR и в ответе reviewer'а; здесь команды, которыми он снят:
`check_artifact_ids.sh`, `check_gate_meta.sh`, `check_roadmap_sync.sh`, `check_protected_artifacts.sh`,
`check_review_fa.sh`, затем `gh pr checks <N> --watch` → exit-код.
