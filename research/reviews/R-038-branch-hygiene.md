# R-038 — гигиена веток по инвентаризации architect'а (reviewer)

**Дата:** 2026-08-07T09:3xZ · **Роль:** reviewer · **Предмет:** удаление 19 веток `origin/*`
по списку architect'а (группы A/B/C) · **База:** `origin/main` = `28a6ee3`

**Вердикт:** **APPROVED ЧАСТИЧНО** — 15 веток удалено, **1 удержана** (`feat/M-57-tail-follower`,
предусловие не выполнено), **2 не трогались и требуют решения** (несут файлы, которых в `main` НЕТ).

Правило проверки — `gates.md` §8: «отчёт агента — гипотеза, состояние git — факт». Ни одно
удаление не сделано по списку: каждое подтверждено собственной командой, вывод которой ниже.

---

## §A — Метод

| критерий | команда | что доказывает |
|---|---|---|
| целиком влита | `git cherry origin/main origin/<b> \| grep -c '^+'` = 0 | каждый коммит ветки имеет patch-эквивалент в `main` (ловит rebase/cherry-pick, чего `--merged` не видит) |
| файл идентичен | `git rev-parse origin/main:<f> origin/<b>:<f>` — один блоб | побайтовое совпадение, сильнее patch-id |
| main полнее | `diff <(git show origin/main:<f>) <(git show origin/<b>:<f>)` + `wc -l` обеих сторон | merge был бы откатом |
| конфликт | `git merge-tree --write-tree origin/main origin/<b>; echo $?` | ветка не сливается механически |

Тип ветки восстановим по SHA (§E) до ближайшего `git gc`.

## §B — Группа A: удалены как целиком влитые (7)

`cherry_plus` — число коммитов ветки БЕЗ patch-эквивалента в `main`.

| ветка | cherry_plus | дополнительный пруф | tip |
|---|---|---|---|
| `engine-dev-hft-engine-dev-1784074858` | **0** | `crates/journal/tests/red_compaction.rs`, `crates/venue-binance/tests/red_book_bounded.rs` — блобы IDENTICAL с `main` | `9a0fbc8` |
| `engine-dev/TD-031` | **0** | добавляемый ветвью schema-гейт reuse (`header.schema_version == contracts::SCHEMA_VERSION`) присутствует в `main:crates/journal/src/segments.rs:2271` (файл вырос 1941 → 3330 строк) | `e13e686` |
| `feat/M-45-spec` | **0** | `milestones/M-45-persist-l2delta.md`: **main 356 строк / ветка 211** — main строго полнее | `424defe` |
| `feat/M-58-depth-metric` | **0** | `research/reviews/R-035-M-58-rev2.md` — блоб IDENTICAL; M-58 смержен `a633333` | `573d41b` |
| `test/TD-072-det-mixed` | **0** | `crates/journal/tests/red_det_replay_digest.rs` — блоб IDENTICAL | `91d572b` |
| `venue-dev/M-18-l2delta` | **0** | `pub fn l2delta_event` живёт в `main:crates/venue-binance/src/lib.rs:827`; L2Delta — 27/29 упоминаний в обоих venue-крейтах | `3fe1583` |
| `research/M-32-q1` | 1 (см. ниже) | **`research/data-quality/depth-sources-survey.md` — один и тот же блоб `dc056ff` в `main` и на ветке**, 479 строк | `19be3f4` |

**Оговорка по `research/M-32-q1`.** Здесь `cherry` даёт 1 «+»: коммит `19be3f4` не совпал по
patch-id (иной контекст применения). Критерий, названный architect'ом для этой ветки, — иной, и
он выполнен точно: единственный файл коммита побайтово тот же блоб, что в `main`. Дополнительно:
`milestones/M-32-depth-verification.md` на ветке несёт статусы `⏳` там, где в `main` уже `✅` —
merge был бы откатом статусов.

Локальная ветка `feat/M-58-depth-metric` (`d7318aa`) удалена тем же основанием: `cherry_plus=0`,
и она — предок удалённой `origin/feat/M-58-depth-metric`.

## §C — Группа B: удалены как устаревшие, merge = регресс (4)

| ветка | предмет | замер | tip |
|---|---|---|---|
| `feat/arch-audit` | `research/reviews/R-027-arch-audit.md` | main **558** строк / ветка **520**; в `main` есть §N-2 (:281) и заведённый по нему **`TD-094`** (:478), закрытый затем в `R-029` | `a519edc` |
| `docs/branch-hygiene-gc` | `.claude/rules/branch-hygiene.md` | ветка = ДОРЕДАКЦИОННЫЙ длинный текст (нарративы инцидентов, снятые компрессией `A-003`); в `main` живёт §«Worktree lifecycle» (:57-66) И сам `scripts/gc_worktrees.sh`, **которого на ветке нет вовсе** | `b1727c4` |
| `docs/margin-reprobe` | `research/data-quality/margin-source-survey.md` | §9 есть в обеих версиях, но заголовки разные: ветка — «ВЕРДИКТ ПЕРЕВЁРНУТ: FEASIBLE», `main` (:411) — «**proxy достижим; ledger НЕТ (§8 стоит)**». Main несёт осторожную, позднюю редакцию; merge вернул бы переоценку | `caddaf2` |
| `research/M-32-impl` | milestone + код | 3 из 4 коммитов patch-эквивалентны; четвёртый (`619c525`) — `cargo fmt` над RED-тестами, его эффект в `main` предъявлен прогоном: `cargo fmt --all -- --check` **exit=0**. Milestone-файл на ветке — со статусами `⏳` против `✅` в `main` | `619c525` |

### F-1 (NOTE, устранено этим же вердиктом) — `feat/arch-audit` несла ПОЗДНЕЕ уточнение замера

Формулировка «версия main полнее» для этой ветки верна в целом, но не поточечно. Уникальные
15 строк ветки — не старый текст, а **уточнение того же §N-2, сделанное позже**
(`a519edc`: «уточнение замером»). `main` говорит «из трёх полных прогонов красный был ОДИН»;
ветка — «**2 красных из 3**» с адресом паники. Удаление ветки без сохранения потеряло бы более
точное число. Дефект давно закрыт (`TD-094` → `R-029`: оракул переведён с настенного времени),
поэтому правка `R-027` задним числом не делается, но замер сохраняется здесь ДОСЛОВНО:

```
Воспроизводимость (три полных прогона `--workspace` подряд на одном дереве, без правок):

прогон 1: td083_tick_wallclock_does_not_grow_with_history ... FAILED   (exit=101)
прогон 2: td083_tick_wallclock_does_not_grow_with_history ... ok       (exit=0)
прогон 3: td083_tick_wallclock_does_not_grow_with_history ... FAILED   (exit=101)
          panicked at crates/gateway/tests/red_frames_seek_bound.rs:379:5
$ cargo test -p gateway --test red_frames_seek_bound   # в изоляции
          test result: ok. 6 passed; 0 failed                          (exit=0)

**2 красных из 3** — это не редкая невезуха, а почти монета: `main` перестаёт быть
детерминированно зелёным. Хуже всего, что такой FAIL легко списать на инфраструктуру и
обойти руками — дословная формулировка `TD-023`.
```

## §D — Группа C: удалены как заброшенные черновики M-09 (4)

Все четыре не сливаются механически (`git merge-tree` exit=1), а несомая ими функция в `main`
присутствует в развитом виде:

| ветка | конфликтует по | что в `main` вместо | tip |
|---|---|---|---|
| `venue-dev/m-09-task2` | `crates/venue-binance/Cargo.toml`, `.../src/lib.rs` | `crates/venue-binance-futures/src/recon.rs` (REST `/fapi/v1/depth` recon-fetch, :33-34) | `a02a9ea` |
| `venue-dev/m-09-task2-v2` | `crates/ops/src/{budget,metrics,recon,silence}.rs`, `venue-binance-futures/src/recon.rs` | те же файлы в `main`: `recon.rs` **319**, `metrics.rs` **241**, `budget.rs` **123**, `silence.rs` **16** строк | `ea109be` |
| `engine-dev/M-09-feeder-apply-md` | **`crates/recorder/src/main.rs`**, `crates/recorder/src/recon_loop.rs` | `apply_md_to_books` + `ReconBooks` живут в `main:crates/recorder/src/recon_loop.rs` (86 строк), books-feeder-таск — `lib.rs:256`, `main.rs:305`; оракулы `red_recon_loop.rs`, `red_recon_wiring.rs` | `7204ecc` |
| `engine-dev/feat-M-09-task2-books-feeder` | **`crates/recorder/src/main.rs`** | то же | `463f6c2` |

### F-2 (NOTE) — формулировка «все конфликтуют по `recorder/src/main.rs`» верна для ДВУХ из четырёх

Две venue-ветки конфликтуют по `crates/venue-binance/Cargo.toml` и `crates/ops/*`, а не по
`recorder/src/main.rs`. На вердикт не влияет (конфликт и вытеснение содержимого подтверждены для
всех четырёх), но список architect'а в этой строке — гипотеза, не факт.

**Проверено отдельно, что ничего не потеряно.** Символ `venue_dispatch` из `7204ecc` в `main`
отсутствует, и это не пропажа механизма, а его переработка: в `main` fan-out делает
books-feeder-таск, читающий `md_rx` и зовущий `apply_md_to_books` для каждого события
(`crates/recorder/src/lib.rs:245-260`), под sacred-оракулом `red_recon_wiring`.

## §E — Что удалено: 15 веток (восстановимо по SHA)

```
engine-dev-hft-engine-dev-1784074858     9a0fbc8959754c84e93d693dfb7f4809f5548eb1
engine-dev/TD-031                        e13e686f39425f6ddcffd378566f8c464cc0dc15
feat/M-45-spec                           424defe3d777831c46795a00605b3d13174267a1
feat/M-58-depth-metric                   573d41bdbf229067dcf983d9ff33eb8a2e01b9d1
test/TD-072-det-mixed                    91d572b105d2c3e998b1cd2600cf1b6ad2ad3c8e
venue-dev/M-18-l2delta                   3fe1583289cec3c990105424cc1f000ef7c312f3
research/M-32-q1                         19be3f4797cc65d6697572c6acbb49f55221d537
feat/arch-audit                          a519edc1eb4563fb98c9a8b6571bfdca3af779e3
docs/branch-hygiene-gc                   b1727c4a888921ef28a69c92ffa401284a19ac8f
docs/margin-reprobe                      caddaf2a6c9f8ef4743881719cd5de006a72692c
research/M-32-impl                       619c525602074023b9e4e8f67fb2563ff0ac501c
venue-dev/m-09-task2                     a02a9ea123dfc2d1e49679bf2313d9ccf4c884c7
venue-dev/m-09-task2-v2                  ea109be9be9451184d5c03eb1c5c42610e3f323f
engine-dev/M-09-feeder-apply-md          7204eccb4934adc5563d0de2962eed030cb800ee
engine-dev/feat-M-09-task2-books-feeder  463f6c21fdfc471840878767b6592488a218f783
```

`origin` до/после: **104 → 89** удалённых веток.

## §F — БЛОКЕР-1: `feat/M-57-tail-follower` НЕ удалена (предусловие не выполнено)

Architect назвал два условия. Проверены оба, и они разошлись:

```
$ git merge-base --is-ancestor origin/feat/M-57-task5 origin/main       # M-57 в main?
NO
$ git rev-list --count origin/main..origin/feat/M-57-task5
25
$ git rev-list --count origin/feat/M-57-task5..origin/feat/M-57-tail-follower
0
```

Подмножественность выполнена (`tail-follower` ⊆ `task5`, счётчик 0), **но merge M-57 не
состоялся** — `origin/main` = `28a6ee3` не содержит ни `task5`, ни `tail-follower`; `task5`
опережает `main` на 25 коммитов. Условие звучало «ТОЛЬКО ПОСЛЕ merge M-57», значит удаление
сейчас — нарушение собственного предусловия инвентаризации, а `SESSION-HANDOFF` §0 держит M-57
активным предметом (`C-059`/`C-060` закрыты, дальше tester). Ветка удерживается до merge M-57.
Тогда удаление станет механическим: содержимое уже целиком внутри `task5`.

Побочно проверено: тип `6531029`, на котором стоит worktree `/tmp/hft-critic-m57b` и одноимённая
локальная ветка, — тоже предок `origin/feat/M-57-task5` (`--is-ancestor` = YES), то есть ничего
уникального он не держит.

## §G — БЛОКЕР-2: две «research»-ветки несут файлы, которых в `main` НЕТ

В хвосте задания они помечены «почему не закрыто: чистый merge». Замер показывает, что это не
остаток формальности, а **невлитый артефакт**:

| ветка | файл | строк | есть в `main`? | merge-tree |
|---|---|---|---|---|
| `research/depth-probe` (`a99c917`, 2026-07-22) | `research/data-quality/depth-probe-binance.md` | **287** | **НЕТ** | exit=0 (чисто) |
| `research/td-007-determinism-coverage` (`b5fc0ca`, 2026-07-31) | `research/measurements/td-007-determinism-coverage.md` | **368** | **НЕТ** | exit=0 (чисто) |

Это прямо противоречит `docs/SESSION-HANDOFF.md` §9, где сказано: по `research/depth-probe` —
«результаты в `main`», по `td-007` — «TD-007 CLOSED … ветка устарела». Результаты СМЕЖНЫХ работ
(M-32/M-33) в `main` действительно есть, а сами эти два документа — нет ни под каким именем.
655 строк замеров существуют в одном экземпляре, на ветке, вне `main`.

Ни одну из двух я не удалял (в списке на удаление их и не было) и не мержил: merge документа —
это добавление содержимого в `main`, а не гигиена, и по `gates.md` §8 утверждения документа о
коде проверяются на ДЕРЕВЕ СЛИЯНИЯ (`verify_design_claims.sh --merge-preview`), чего заявка не
включала. Решение — за architect'ом: либо влить оба через свой гейт, либо явно списать с
основанием, которое переживёт следующую инвентаризацию.

## §H — NOTE: worktree-GC не запускался

`bash scripts/gc_worktrees.sh --dry-run` → единственный кандидат `WOULD-REMOVE`
`/tmp/hft-engine-dev-1786094488`. `stat` даёт mtime **2026-08-07 09:21Z** при текущем времени
**09:37Z** — 16 минут. По `branch-hygiene.md` §8 свежий mtime трактуется как «агент жив»,
поэтому снос не делался. Остальные 24 worktree помечены KEEP штатно (не смержены / dirty /
только-локальные коммиты).

## §I — Done Block

```
$ git branch -r | wc -l
89                              # было 104

$ git push origin --delete <7 веток группы A>
 - [deleted]  engine-dev-hft-engine-dev-1784074858
 - [deleted]  engine-dev/TD-031
 - [deleted]  feat/M-45-spec
 - [deleted]  feat/M-58-depth-metric
 - [deleted]  research/M-32-q1
 - [deleted]  test/TD-072-det-mixed
 - [deleted]  venue-dev/M-18-l2delta
push_exit=0

$ git push origin --delete <4 ветки группы B>
 - [deleted]  docs/branch-hygiene-gc
 - [deleted]  docs/margin-reprobe
 - [deleted]  feat/arch-audit
 - [deleted]  research/M-32-impl

$ git push origin --delete <4 ветки группы C>
 - [deleted]  engine-dev/M-09-feeder-apply-md
 - [deleted]  engine-dev/feat-M-09-task2-books-feeder
 - [deleted]  venue-dev/m-09-task2
 - [deleted]  venue-dev/m-09-task2-v2

$ git branch -D feat/M-58-depth-metric
Deleted branch feat/M-58-depth-metric (was d7318aa).

$ cargo fmt --all -- --check; echo exit=$?      # пруф для research/M-32-impl (619c525)
exit=0

$ git branch -r | grep M-57                      # удержано
origin/feat/M-57-tail-follower
origin/feat/M-57-task5
```

Кода, контрактов и `crates/**` работа не касается: удалялись ссылки, `main` не менялся ни на
байт (кроме этого вердикт-файла). RISK-BLOCK (`gates.md` §5) не применим — ни один путь
`risk`/`killswitch`/`oms`/`venue-*`/`contracts` в `main` не тронут.

## §J — Close-out: деплой-гейт `gates.md` §8

```
$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (28a6ee3..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
barrier_exit=0

$ git push origin main
   28a6ee3..5894cf0  main -> main

$ gh run watch 31166819371 --exit-status; echo ci_exit=$?
ci_exit=0
31166819371 CI 5894cf0 completed success

$ gh run list --workflow=deploy.yml --limit 3
dca889a completed success        # Deploy на 5894cf0 НЕ триггерился — docs-only push
c714d0f completed success        # (фильтр путей crates/**, Cargo.toml, Cargo.lock)
aeb409b completed success

$ ssh … root@167.233.192.131 'docker ps; cat …/recorder.heartbeat; df -h'
hft-gateway-serve Up 35 hours (healthy)
hft-recorder      Up 35 hours (healthy)
{"events":10627050,"free_bytes":71771959296,"min_free_bytes":10737418240,
 "next_seq":186914761,"segment_index":204,"ts_wall_ms":1786096187500,"writable":true}
heartbeat = 2026-08-07T09:49:47Z, now 09:50:03Z ⇒ отставание 16 с
/dev/sda1  150G  77G  67G  54% /
```

Uptime 35 часов подтверждает ожидаемое: раскатки не было, контейнеры не перезапускались.
Журнал пишется (`next_seq` 186 914 761, сегмент 204, `writable: true`), свободно 67 GB.
