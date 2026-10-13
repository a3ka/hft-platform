<!-- GATE-META
milestone: M-57
audited_repo: a3ka/hft-platform
audited_base: 379e3bc
audited_head: f3583c6
verdict: ESCALATE
-->

# C-063 - M-57 / TD-083 measure audit

**Вердикт: ESCALATE.**

Фактический вывод architect'а по направлению подтверждён: краснота `td083` по
wall-clock ratio не доказывает M-57-регресс. `small` после M-57 дешевеет резко,
`big` не показывает устойчивого ухудшения и в среднем у меня чуть дешевле, поэтому
ratio растёт главным образом из-за знаменателя.

Но это уже третий круг вокруг одного предмета, а разблокировка требует решения по
sacred-оракулу M-53. Critic не должен ослаблять или переписывать `td083` внутри
M-57-цепочки, а текущий wall-clock порог одновременно может пройти полный
`verify_M-57.sh` и упасть на точечном запуске того же теста. Нужен арбитр по
`.claude/rules/gates.md` §0, после чего architect владельца M-53 делает отдельный
oracle-repair круг.

## Находки

### F1 - `td083` wall-clock ratio меряет не тот инвариант

`crates/gateway/tests/red_frames_seek_bound.rs:449-466` считает
`ratio = big / small` и блокирует при `ratio >= 4.0`. Это не инвариант "цена тика
не растёт с историей": если `small` подешевел сильнее `big`, ratio краснеет на
улучшении. Мой A/B замер это воспроизвёл.

Та же проверка зависит от окружения. На одном и том же `f3583c6` полный
`bash scripts/verify_M-57.sh` у меня прошёл `VERDICT: PASS`, но три точечных
запуска `td083` дали `FAIL, PASS, FAIL` с ratio `4.6, 3.8, 4.9`.

### F2 - корректная замена wall-clock части уже детерминирована

Рабочая мера должна быть не `big_time / small_time`, а абсолютный бюджет работы
тика:

```text
work_small = stats.events_scanned for 1_000 + INCREMENT
work_big   = stats.events_scanned for 8_000 + INCREMENT

assert work_small <= INCREMENT * 4
assert work_big   <= INCREMENT * 4
assert stats.segments_opened <= 2
```

Кодовые опоры уже есть:

- `crates/gateway/tests/red_frames_seek_bound.rs:375-405` - текущий work-check
  берёт `st.events_scanned`.
- `crates/journal/src/segments.rs:1187-1223` - `events_scanned` инкрементируется
  сразу после чтения event frame и до `after_seq`-фильтра.
- `crates/gateway/src/lib.rs:1960-1965` - `gateway::ReadStats` пробрасывает
  `stream.events_scanned()`.

Проверяемая мутация, против которой мера должна краснеть: в
`crates/journal/src/segments.rs:1144` заменить успешный путь `Ok(saved_offset)` на
`Ok(header_end)` (или эквивалентно сделать `read_tail_offset()` всегда `None`).
Это возвращает full forward-scan активного raw-сегмента. Ожидаемый результат:
`events_scanned` на measured tick становится больше `INCREMENT * 4`, а O-2 M-57
тоже краснеет. Удешевление `small` не меняет этот счётчик и не может создать
ложный FAIL.

Кто меняет: architect в отдельном M-53/TD-083 oracle-repair круге после
арбитражного решения. Не M-57 dev/tester/reviewer и не этот critic.

### F3 - остаточный рост создаёт не decode событий, а каталог/заголовки

При зелёном `events_scanned` остаточная стоимость big-пути идёт до чтения
событий:

- `crates/gateway/src/lib.rs:3047` - каждый `LiveReducer::pump()` заново вызывает
  `journal::stream_from`.
- `crates/journal/src/segments.rs:1279` - `stream_from()` сначала вызывает
  `segments(dir)`, ещё до `after_seq`-skip.
- `crates/journal/src/segments.rs:775-780` - `segments()` классифицирует каждый
  путь из каталога.
- `crates/journal/src/segments.rs:711-750` - `dedup_indexed_paths()` делает
  `fs::read_dir` и строит `BTreeMap` по всем `segment-*.jrnl[.zst]`.
- `crates/journal/src/segments.rs:576-584` и `591-593` - raw segment получает
  `metadata`, magic read и повторное open/read header.
- `crates/journal/src/segments.rs:645-651` - compacted segment открывается и
  декодирует zstd-заголовок.
- Только после этого `crates/journal/src/segments.rs:1290-1310` отбрасывает
  старые сегменты по `after_seq`.

Активный сегмент тоже открывается и читает заголовок в
`resolve_active_start_offset()` / `PositionedBufReader::open`
(`crates/journal/src/segments.rs:1112-1117`, `978-985`), но это константа одного
tail-сегмента. Рост big/small создаёт прежде всего полный обход каталога и
классификация заголовков всех сегментов на каждый tick. Это отдельный дефект ниже
M-57: нужен отдельный milestone на tail metadata/index cache для `stream_from`,
не расширение M-57.

## A/B замер

Метод: внешний `/tmp/td083_probe_{a,b}`, без правок репозитория, повторяет
`tick_cost(1_000)` и `tick_cost(8_000)` из `td083`: `REPEATS=5`,
`INCREMENT=3`, `SEG_BYTES=16 KiB`, `provenance="test"`,
`epoch_id="own-test"`. Запускались готовые debug-бинари, пары чередовались
`A control -> B current`. Control: `/tmp/hft-critic-m57-control` at `7d19c5c`
from `origin/feat/M-58-depth-metric`, `events_scanned` отсутствует в
`segments.rs`. Current: `/tmp/hft-critic-m57c` at `f3583c6`.

| pair | load before A | A small | A big | A ratio | load before B | B small | B big | B ratio |
|---:|---|---:|---:|---:|---|---:|---:|---:|
| 1 | 11.76 12.38 11.02 | 1.1336 ms | 2.5286 ms | 2.231 | 11.71 12.34 11.03 | 0.4951 ms | 2.3582 ms | 4.763 |
| 2 | 12.18 12.43 11.07 | 1.1414 ms | 2.5286 ms | 2.215 | 12.24 12.44 11.09 | 0.5413 ms | 2.7341 ms | 5.051 |
| 3 | 11.86 12.35 11.07 | 1.3415 ms | 3.8014 ms | 2.834 | 16.67 13.35 11.42 | 0.4766 ms | 3.2699 ms | 6.861 |

Средние по этим трём парам: `small` 1.2055 ms -> 0.5043 ms (-58.2%),
`big` 2.9529 ms -> 2.7874 ms (-5.6%). `big` попарно шумит, но устойчивого
ухудшения M-57 я не вижу; ratio растёт из-за более сильного удешевления `small`.

## Ответы на три вопроса

1. Вывод architect'а в главном верен: красный ratio не доказывает регресс M-57.
   В моём замере абсолютный `small` стабильно лучше, `big` в среднем не хуже
   control, но точное число architect'а `big -13%` я не воспроизвёл как стабильное:
   у меня это `-5.6%` на среднем под заметным load drift.

2. Заменить wall-clock big/small надо детерминированной work-мерой
   `events_scanned` + `segments_opened` с абсолютным бюджетом на каждый сценарий,
   а не отношением времён. Она краснеет против мутации `Ok(saved_offset) ->
   Ok(header_end)` в `resolve_active_start_offset`, не краснеет от удешевления
   `small` и не зависит от load average.

3. Остаточный рост не должен расширять M-57. M-57 лечит пересканирование активного
   сегмента; это зелёное по O-1..O-4 и `events_scanned`. Остаток создаётся
   каталогом/классификацией заголовков всех сегментов перед skip, значит нужен
   отдельный milestone на bounded `stream_from` metadata path.

## Done Block

```text
$ cd /home/nous/hft-platform && git fetch origin --quiet && git worktree add /tmp/hft-critic-m57c feat/M-57-task5
Preparing worktree (checking out 'feat/M-57-task5')
fatal: 'feat/M-57-task5' is already used by worktree at '/tmp/hft-dev-m57-task5'

$ cd /home/nous/hft-platform && git worktree add /tmp/hft-critic-m57c --detach origin/feat/M-57-task5 && cd /tmp/hft-critic-m57c && git checkout -B critic/M-57-td083 origin/feat/M-57-task5 && git remote get-url origin && git rev-parse --show-toplevel && git log -1 --format='%h %s'
https://github.com/a3ka/hft-platform.git
/tmp/hft-critic-m57c
f3583c6 docs(M-57): §6 — A/B замер td083: регресса нет, красный оракул есть артефакт отношения; старый зелёный был ложным [architect]

$ cd /tmp/hft-critic-m57-control && git log -1 --format='%h %s' && if rg -q "events_scanned" crates/journal/src/segments.rs; then echo events_scanned_present; else echo events_scanned_absent; fi
7d19c5c docs(M-58): F-1 R-033 — полная транскрипция 14/14 строк замера, сырой вывод файлом [research-dev]
events_scanned_absent

$ CARGO_TARGET_DIR=/tmp/td083_probe_a_target cargo build --quiet --manifest-path /tmp/td083_probe_a/Cargo.toml; echo exit=$?
exit=0

$ CARGO_TARGET_DIR=/tmp/td083_probe_b_target cargo build --quiet --manifest-path /tmp/td083_probe_b/Cargo.toml; echo exit=$?
exit=0

$ /tmp/td083_probe_a_target/debug/td083_probe ; /tmp/td083_probe_b_target/debug/td083_probe
PAIR2=1 A control load=11.76 12.38 11.02 4/3755 2470192 utc=2026-08-05T11:21:40Z
small_us=1133.6 big_us=2528.6 ratio=2.231 small=1.133585ms big=2.528619ms repeats=5 increment=3
PAIR2=1 B current load=11.71 12.34 11.03 15/3741 2470442 utc=2026-08-05T11:21:51Z
small_us=495.1 big_us=2358.2 ratio=4.763 small=495.078µs big=2.35822ms repeats=5 increment=3
PAIR2=2 A control load=12.18 12.43 11.07 9/3651 2470715 utc=2026-08-05T11:22:02Z
small_us=1141.4 big_us=2528.6 ratio=2.215 small=1.141375ms big=2.528629ms repeats=5 increment=3
PAIR2=2 B current load=12.24 12.44 11.09 20/3681 2471001 utc=2026-08-05T11:22:14Z
small_us=541.3 big_us=2734.1 ratio=5.051 small=541.287µs big=2.734148ms repeats=5 increment=3
PAIR2=3 A control load=11.86 12.35 11.07 28/3745 2472232 utc=2026-08-05T11:22:25Z
small_us=1341.5 big_us=3801.4 ratio=2.834 small=1.341475ms big=3.801374ms repeats=5 increment=3
PAIR2=3 B current load=16.67 13.35 11.42 31/3785 2475365 utc=2026-08-05T11:22:39Z
small_us=476.6 big_us=3269.9 ratio=6.861 small=476.568µs big=3.269885ms repeats=5 increment=3

$ bash scripts/verify_M-57.sh; echo exit=$?
PASS  T0 crates/journal/tests/red_tail_scan_bounded.rs
PASS  T1 build --workspace
PASS  T2 clippy --workspace --all-targets -D warnings
PASS  T2b fmt --check
PASS  T3 O-1..O-4 GREEN
PASS  T4 events_scanned есть в segments.rs
PASS  T4 events_decoded СОХРАНЁН (на нём стоят прежние оракулы)
PASS  T5 journal GREEN (счётчики и чекпоинт-ресурс целы)
PASS  T5 M-53/M-54/M-56 GREEN
PASS  T5 gateway-serve GREEN (сверка WS↔реплей цела)
PASS  T6 crates/contracts/** не тронут
VERDICT: PASS
exit=0

$ for i in 1 2 3; do cargo test -p gateway --test red_frames_seek_bound td083_tick_wallclock_does_not_grow_with_history -- --nocapture --exact; echo td083_exit=$?; done
TD083_RUN=1 load=24.68 15.52 12.19 utc=2026-08-05T11:23:05Z
TD-083 O-B (время, диагностика): журнал ×8 → тик ×4.6 (630.637µs → 2.879078ms)
test result: FAILED. 0 passed; 1 failed; finished in 26.11s
td083_exit=101
TD083_RUN=2 load=32.34 18.30 13.23 utc=2026-08-05T11:23:32Z
TD-083 O-B (время, диагностика): журнал ×8 → тик ×3.8 (539.547µs → 2.061661ms)
test result: ok. 1 passed; 0 failed; finished in 29.42s
td083_exit=0
TD083_RUN=3 load=26.31 18.16 13.34 utc=2026-08-05T11:24:01Z
TD-083 O-B (время, диагностика): журнал ×8 → тик ×4.9 (516.867µs → 2.551738ms)
test result: FAILED. 0 passed; 1 failed; finished in 18.45s
td083_exit=101

$ git status --porcelain
?? research/critiques/C-063-M-57-td083-measure.md
```
