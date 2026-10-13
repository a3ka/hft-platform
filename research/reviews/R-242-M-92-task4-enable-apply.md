<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: cdb50fd51644b51c9f2ffd4edd32ed6e6fd9e0c7
audited_head: e78c765c255093e20ea0e506c23be9b6c050ece5
verdict: APPROVE
-->

# R-242 — M-92 задача 4: включение `apply` (П-031) подтверждено по аудит-следу

**Дата (UTC):** 2026-10-05 · **Роль:** reviewer (PR-гейт + подтверждение задачи 4) ·
**Предмет:** ветка `origin/docs/m92-task4` + аудит-след на VPS.

## §0. Предмет — вершина взята командой

```
$ git fetch origin && git rev-parse origin/docs/m92-task4 origin/main
e78c765c255093e20ea0e506c23be9b6c050ece5      ← вершина = SHA мандата (e78c765)
cdb50fd51644b51c9f2ffd4edd32ed6e6fd9e0c7      ← origin/main, предок вершины (merge-base --is-ancestor → 0)
$ git log --oneline origin/main..origin/docs/m92-task4
e78c765 docs(M-92): задача 4 — удаление включено и проверено на проде (П-031) [architect]
$ git diff --stat origin/main...origin/docs/m92-task4
 milestones/M-92-manifest-verified-prune.md | 2 +-
```

Грепом искал в ярусе C: `M-92`, `2ter`, `TD-020`, `TD-202`, `TD-203` в `PROJECT-STATE.md`,
`TECH-DEBT.md`, `docs/ROADMAP.md`; `П-031` в `docs/PENDING-SIGNATURE.md`.

## §1. Блоки PR-гейта

| блок | итог | основание |
|---|---|---|
| Block-scope | PASS | диф — одна строка `milestones/M-92-*.md` §8 задача 4, зона architect'а (спека §7) |
| Block-C | N/A | `crates/contracts/**` не тронут |
| Block-risk | N/A | `risk`/`killswitch`/`oms`/`venue-*` не тронуты; путь удаления журнала шёл RAW-гейтом критика на сильной модели (`C-268`…`C-275`, `A-044`/`A-045`) и PR-гейтами `R-217`…`R-241` |
| RED sacred | PASS | `*/tests/**` в диапазоне не тронуты |
| Атомарность | PASS | один коммит, ссылка `M-92` задача 4, метка `[architect]` |
| Полномочия | PASS | включение `apply` делегировано architect'у решением founder'а `П-031` (2026-10-02); правила отбора (`retain_days=14`, `keep_min=4`, покрытие) не менялись; на коробке ничего не удалено |

**FA (`docs/fa/journal.md`).** Предмет — удаление сегментов журнала, поэтому названы живые
инварианты: **`JR-I-13`** (`:221`, удаление только при равенстве sha256 сумме на стороне
офсайт-копии) и **`JR-I-2`** (`:112`, `seq` без дыр — отсюда проверка сплошности каталога).
Диапазон не трогает `crates/**`, барьер `check_review_fa.sh` здесь даёт SKIP; инварианты
названы по существу, а не ради барьера.

## §2. Подтверждение задачи 4 — по аудит-следу, своими командами

Все команды — `ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131`,
2026-10-05 ≈ 01:00Z. D = `20261005T005125Z-3407107` (dry-run), A = `20261005T005428Z-3408030` (apply).

**П.1 — планы, манифест, отчёт.**
```
59 D-plan.txt  59 D-manifest.txt  59 D-report.txt
59 A-plan.txt  59 A-manifest.txt  59 A-report.txt
sha256 D-plan = sha256 A-plan = 9b659512…22c8;  cmp D-plan A-plan → YES;  cmp D-manifest A-manifest → YES
манифест: строк вне формы ^[0-9a-f]{64}  segment-NNNNNNNN.jrnl(.zst)?$ → 0
имена манифеста == план (diff) → YES;  дублей имён → 0
план: segment-00000691 … segment-00000749, разрывов нет
отчёт apply: 59 pruned (kept — 0); имена отчёта == план → YES
отчёт dry-run: 59 kept, причина «dry-run» (0 удалений)
```
**П.2 — каталог журнала.**
```
удалённых имён плана в каталоге: present=0
каталог: first=750 last=927 count=178 — разрывов нет
```
**П.3 — выборочно 5 имён: сумма на коробке, запрошенная заново, = строке манифеста.**
```
segment-00000691  d9002ec7…a14d  EQ
segment-00000706  217fa585…751f  EQ
segment-00000720  c4400189…b15a  EQ
segment-00000737  67515679…8d3e  EQ
segment-00000749  c36169fb…76b8  EQ
```
Дополнительно повторил (а0): `sha256sum journal/segment-00000700.jrnl.zst journal/segment-99999999.jrnl.zst`
одним вызовом → одна строка суммы (`7fc97257…8c7e` = строке манифеста) + stderr
`No such file or directory`, `rc=1` — то поведение, на котором стоит `I-10`.

**П.4 — переключатель, тревога, прод.**
```
/var/lib/hft/retention.mode → apply
retention.alert → отсутствует (ls /var/lib/hft; find / -xdev -name 'retention.alert*' → пусто)
retention.last-success → 2026-10-05T00:54:56Z
лог apply: mode=Apply · offloaded: 0  pruned: 59  failed: 0 · freed_bytes: 12665168344
hft-gateway-serve Up 33 minutes (healthy) · hft-recorder Up 33 minutes (healthy)
heartbeat: ts_wall_ms 1791162092277 (= 01:01:32Z, проверка в 01:01:35Z), segment_index 927, writable true
прогреватель после удаления: covered=791443675, затем covered=791476025 (gateway-checkpoint ok)
RssAnon: recorder 12488 kB, gateway-serve 5424 kB;  df /: 47 %
```

**Вывод.** Каждое утверждение строки задачи 4 подтверждено независимым замером: удалены ровно
59 сверенных сегментов, суммы на коробке совпадают с манифестом, каталог сплошной, тревоги нет,
сервисы здоровы. `JR-I-13` исполнен на проде.

## §3. Наблюдения (не блокируют)

- **Н-1.** Строка задачи 4 в спеке говорит «ждёт подтверждения reviewer'ом», шапка спеки —
  «Статус: PROPOSED». После этого вердикта оба места устарели. Исправить — architect, тем же
  коммитом, что переносит спеку в архив (`Р-2`).
- **Н-2.** `/etc/cron.d/hft-journal-retention` по-прежнему несёт `RETENTION_MODE=dry-run`, хотя
  удаление включено. Так задумано (`I-7`: режим задаёт только файл-переключатель), но оператор,
  читающий cron-файл, увидит не тот режим. Дефекта поведения нет; при следующей правке
  `deploy/README.md` / cron-файла стоит пометить строку как не действующую.
- **Н-3.** Первый автоматический ночной прогон в режиме `apply` (04:07 UTC 2026-10-05) этим
  вердиктом не наблюдался — на момент записи он ещё не наступил. Ожидание: план пуст или почти
  пуст (весь возрастной префикс уже удалён), `failed: 0`, тревоги нет.
- **Н-4.** Объём на коробке растёт без удаления (`П-031` «что не решено»). Не долг этого
  милестоуна — решение founder'а.

## §4. Условие APPROVED

Выполнено: задача 4 подтверждена по аудит-следу (§2), PR-гейт по дифу чист (§1).

## §5. Done Block

```
$ git rev-parse origin/docs/m92-task4
e78c765c255093e20ea0e506c23be9b6c050ece5
$ git status --porcelain   (worktree /tmp/hft-reviewer-m92t4, после прогона verify)
?? research/reviews/R-242-M-92-task4-enable-apply.md      ← только этот вердикт
$ bash scripts/verify_M-92.sh > verify92.log 2>&1; echo "exit=$?"
$ grep -cE "^PASS" verify92.log; grep -cE "^FAIL" verify92.log; grep -E "^(SKIP|VERDICT)|^exit=|cargo test" verify92.log
58
0
PASS  ci-parity: cargo test --all
SKIP  ci-parity: [188b5f08be3448fb] «cargo install cargo-audit --locked» — установка инструмента (cargo audit исполняется ниже)
SKIP  ci-parity: [0ada681c1a994d65] «pip install --quiet jsonschema» — установка инструмента
SKIP  ci-parity: [ddd7a1d0ce254659] шаг «база события» (12 строк, пишет sha в GITHUB_OUTPUT); локальный эквивалент — merge
SKIP  ci-parity: [48fbd5e3f735e6ad] «git fetch … refs/salvage/*» — плумбинг CI: спас-рефы в свежий клон; локальный клон их
SKIP  ci-parity: check_review_fa — вердикта R-NNN по M-92 в диапазоне ещё нет (зеленеет на PR-гейте)
SKIP  ci-parity: [ba2ff42976ef05e2] «python3 -m pip install --quiet pyyaml» — установка инструмента
SKIP  ci-parity: [eb31b754c4a50419] агрегат «All checks passed» (условие по needs); его правильность исполняет red_ci_aggregate.
SKIP  task4: §8-гейт — dry-run прод-пути (план и манифест в аудит-следе, 0 удалений), затем включение archite…
VERDICT: PASS
exit=0
```
SKIP `task4` — шаг, который по спеке §10 судится не скриптом, а этим вердиктом (§2).
SKIP `check_review_fa` закрывается коммитом этого файла.
