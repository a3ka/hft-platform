<!-- GATE-META
milestone: M-77
audited_repo: a3ka/hft-platform
audited_base: fb713d48c5756a216789c11956837f22658b3438
audited_head: c5e27fd052f42424fa0db68e294202fa38c250b8
verdict: APPROVE
-->

# R-176 — M-77 frame-book continuity, PR-гейт — APPROVED

Дата: 2026-09-07 (UTC) · Роль: reviewer (PR-time, `gates.md` §4 UNCONDITIONAL)
Предмет: `feat/M-77-frame-book-continuity` @ `c5e27fd`, база `origin/main` @ `fb713d4`.
Дерево аудита: `/tmp/hft-reviewer-m77` (detached `c5e27fd`), гейт прогнан РЕВЬЮЕРОМ заново —
вердикт не стоит на пересказе Done Block'а тестера.

## Решение

**APPROVED.** Блокирующих находок нет. Три находки — ненулевые и записываются долгом
(`TD-200`, `TD-201`), ни одна не отменяет предмет и не требует круга правки до merge'а.

## Block-scope — диф соответствует §7 спеки

Диапазон реализации `1b99136..c5e27fd` (две задачи — два коммита):

```
crates/gateway/src/lib.rs                 544 +/ 18 −      (зона engine-dev, §7)
milestones/M-77-frame-book-continuity.md    3 +/  3 −      (ТОЛЬКО колонка Status задач 3/4 —
                                                            carve-out `scope-guard.md`)
```

Запретный список §7 проверен явно и пуст: `crates/contracts/**`, `crates/gateway-serve/src/**`,
`docker-compose.yml` — ни одного касания в диапазоне `origin/main...c5e27fd`.

**Тесты sacred — целы.** Все три RED-набора тронуты ТОЛЬКО коммитами `[architect]`
(`c571694`, `a8ef764`, `4dd7011`, `4ca6cec`, `8b1c5c5`) и все они лежат ДО коммита
реализации `8e2ea49`. Ни один dev-коммит не касается `*/tests/**` — RED-first соблюдён
по ЛОГУ, а не по заявлению.

**Атомарность.** Задача 3 — `8e2ea49`, задача 4 — `c5e27fd`; бандла нет, ссылка на
milestone/task в subject'е у обоих, трейлеров `Co-Authored-By` в диапазоне ноль.

**Контракт §6bis.3 исполнен ДОСЛОВНО** (спека объявляла сигнатуры до реализации):
`BookSeriesSlice{depth_series,heatmap,cob}` — `pub(crate)`, на провод не идёт;
`Reducer::book_series_in(&self, Option<u64>, Option<u64>) -> BookSeriesSlice`;
`SeriesBundle::set_book_series(&mut self, BookSeriesSlice)` — замещение, не слияние.
Расхождения с объявленным контрактом (за которое `testing.md` требует SCOPE VIOLATION) нет.

**Обе точки создания кадра закрыты** — `:4502` (граница батча) и `:4572` (хвост). Это то
самое место, на котором кандидат А из §9bis оставался незамеченным четырьмя тестами круга 1.

## Block-C — контрактный слой не тронут

`crates/contracts/**` в диапазоне отсутствует. `GATEWAY_SCHEMA_VERSION` не бампнут — форма
выдачи не менялась, менялось наполнение уже объявленных полей (§5 спеки). Новый тип
`BookSeriesSlice` — T3, крейт-приватный, консюмеров вне `crates/gateway` не имеет:
contract-RFC по `docs/05-contract-layer.md` §2/§4 не требуется. Block-C чист.

## Block-risk — RISK-BLOCK не триггерится, и это проверено путями, а не памятью

Диапазон не трогает `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**`,
`crates/contracts/**`. Путь `gateway` read-only по построению (`VB-I-3`), order-egress в
дифе отсутствует. `risk-critic` по `gates.md` §5 не обязателен; plan-time гейт закрыт
кругами `C-211` (REJECT) → `C-212` (REJECT) → `C-213` (**NOTE**, диспетчеризация разрешена).

## Предъявление FA (M-66) — живой инвариант тронутого модуля

Диф трогает `crates/gateway/**`, поэтому вердикт называет ЖИВЫЕ инварианты из
`docs/fa/viz-backend.md` на проверяемой ревизии:

- **`VB-I-2`** (`docs/fa/viz-backend.md:199`) — «live == replay: серия, посчитанная на
  live-хвосте, бит-идентична серии из replay того же окна журнала». Это восстанавливаемый
  предмет: `TD-199` фиксировал его нарушение на прод-пути `pump`.
- **`VB-I-10`** (`:207`) — «Bounded-window snapshot»: память ограничена ОКНОМ, не историей.
  Именно он ограничивает новое хранилище `Reducer::book_series` — см. Н-1.

## Findings

### Н-1 (MINOR, не блокер) — новое хранилище наблюдений НИЧЕМ не сторожится, а спека говорит, что сторожится

`crates/gateway/src/lib.rs:843` заводит `book_series: BTreeMap<u64, BookSeriesObservation>` —
структуру, растущую per-event на `self.full`. §6bis.3 спеки ставила к ней ровно одно
требование и НАЗЫВАЛА его сторожа: «не заводить структуру, растущую вне оконной эвикции
`evict_window_state` (`VB-I-10`; сторожит `T8` гейта, §6bis.4)».

Реализация требование выполняет ПО СУЩЕСТВУ (эвикция по закладке доставки
`evict_book_series_below_cursor`, и удержание ограничено одним батчем: `pump` при отказе по
пределу возвращает `Err` ДО применения следующих событий, `:4529-4537`), но **названный
сторож её не сторожит**:

- `T8`/`red_m77_pump_cost.rs` меряет ОТНОШЕНИЕ АЛЛОКАЦИЙ по оси «число батчей» при
  ПОСТОЯННОЙ книге — это не удержанная память и не её граница;
- `red_gateway_bounded.rs::snapshot_memory_bounded_by_window_not_history` (сторож `VB-I-10`)
  ходит через `gateway::snapshot`, где флаг `capture_book_observations` выключен ПО
  ПОСТРОЕНИЮ — то есть слеп к новому буферу не случайно, а конструктивно.

Класс — `TD-138` («документ обосновывает инвариант механизмом, который на этом пути не
работает»). Долг заведён карточкой `TD-201`; блокером не является, потому что граница
удержания выведена из кода и предъявлена выше, а не предположена.

### Н-2 (MINOR) — оценка стоимости в доке поля не замерена и расходится с замером соседнего коммита

Док-комментарий поля утверждает «≤600 Б на событие; при проде `PUSH_MAX_EVENTS=256` буфер
удержания ≤150 КБ». Замер задачи 4 (тот же диапазон, `c5e27fd`) на прод-форме даёт прирост
аллокаций `pump` +2 740 661 Б на 64 события ≈ **43 КБ на событие** — два порядка от
заявленного. Величины разные (аллокационная churn против удержания), и именно поэтому
цифра в доке — утверждение, а не замер: `testing.md` («метрика, на которой заводится
инвариант, сама подлежит валидации») требует называть, что именно считает цифра.
Записано в `TD-201` вместе с Н-1.

### Н-3 (MAJOR, ПРЕДСУЩЕСТВУЕТ в `main`, вне предмета) — per-event стоимость растёт с размером книги

§9ter спеки передаёт reviewer'у долг, найденный замером: `refresh_heatmap_bucket` (`:1150`,
`:1177`) зовёт `book.levels(side)` на КАЖДОМ `L2`-событии — отношение аллокаций на книге
16 000 против 2 000 уровней = **4.39** при равном выходе (1 256 Б). `M-77` его не вносит и
не чинит; гейт `M-77` его намеренно не судит (оракул на этой оси краснел бы до и после
любой развязки — `A-031`). Заведена карточка `TD-200`.

Цена самой развязки Б названа честно и лежит под потолком: отношение по оси «число батчей»
1.160 против потолка 1.350 (было 0.994), абсолютный прирост тика +7.3 % на одном батче и
+25 % на 64. Условие (iii) `C-211` исполнено.

## Условия APPROVED

1. Merge — через PR (`gh pr create` → зелёный `All checks passed` → `gh pr merge --merge
   --delete-branch`), прямой push в `main` закрыт защитой ветки.
2. `TD-199` закрывается этим milestone'ом; `TD-200`/`TD-201` заводятся reviewer'ом в
   `TECH-DEBT.md` тем же close-out'ом.
3. Post-merge деплой-гейт `gates.md` §8: дождаться CI+Deploy и проверить прод глазами;
   пруф — в close-out.

## Done Block (прогон РЕВЬЮЕРА, сырой вывод)

```text
$ pwd; git log --oneline -1
/tmp/hft-reviewer-m77
c5e27fd feat(M-77): task #4 — замер цены развязки Б (ДО/ПОСЛЕ) на прод-форме [engine-dev]

$ git diff --stat origin/main...c5e27fd
 crates/gateway/src/lib.rs                          | 562 ++++++++++++++++++-
 crates/gateway/tests/red_m77_delivery_window.rs    | 537 +++++++++++++++++++
 .../gateway/tests/red_m77_frame_book_continuity.rs | 592 +++++++++++++++++++++
 crates/gateway/tests/red_m77_pump_cost.rs          | 308 +++++++++++
 milestones/M-77-frame-book-continuity.md           | 465 ++++++++++++++++
 .../critiques/C-211-m77-frame-book-continuity.md   | 182 +++++++
 research/critiques/C-212-m77-round2.md             | 167 ++++++
 research/critiques/C-213-m77-b1-followup.md        | 184 +++++++
 scripts/verify_M-77.sh                             | 286 ++++++++++
 9 files changed, 3265 insertions(+), 18 deletions(-)

$ git diff --name-only origin/main...c5e27fd | grep -E "crates/contracts|gateway-serve/src|docker-compose.yml"; echo exit=$?
exit=1 (1 = совпадений нет, запретные пути чисты)

$ bash scripts/verify_M-77.sh; echo exit=$?
=== M-77 acceptance ===
PASS  T0 все три набора на месте
PASS  T0 состав полон: 2 контроля + 1 сторож цены + 7 предметных
PASS  T1 оба набора исполняют resume+pump в ПРОД-ФОРМЕ (Р-2)
PASS  T2 контроль снимочного хвоста ЗЕЛЁН (исполнено 1)
PASS  T2 дискриминатор окна отказа ЗЕЛЁН (исполнено 1) — окно достижимо
PASS  T3 cargo fmt --all -- --check (exit=0)
PASS  T3 cargo clippy --all-targets --all-features -D warnings (exit=0)
PASS  T4 cargo test --all ЗЕЛЁН (exit=0) — развязка внесена (задачи 2-3 исполнены)
PASS  T5 все 7 предметных + 2 контроля ИСПОЛНЕНЫ и ЗЕЛЕНЫ поимённо
PASS  T6 запретные пути не тронуты (contracts / gateway-serve/src / docker-compose.yml)
PASS  T7 контракт развязки Б объявлен в спеке (§6bis, сигнатура названа) — присутствие
PASS  T7 оба оракула окна доставки ИСПОЛНЕНЫ и ЗЕЛЕНЫ — контракт диапазона держится под отказом
PASS  T8 сторож цены на границе pump ЗЕЛЁН (исполнено 1) — работа тика не растёт с числом батчей
---
VERDICT: PASS
exit=0
```

### Мутационный контроль — прогнан РЕВЬЮЕРОМ, оракулы кусают

Нейтрализованы ОБЕ точки применения развязки (`:4502`, `:4572`):
`delta.set_book_series(slice);` → `let _ = slice;`. Дерево восстановлено
`git checkout -- crates/gateway/src/lib.rs` (проверено `git status --porcelain`).

```text
$ cargo test -p gateway --test red_m77_delivery_window -- --test-threads=1   # МУТАНТ
running 3 tests
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p gateway --test red_m77_frame_book_continuity -- --test-threads=1   # МУТАНТ
running 6 tests
test result: FAILED. 1 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
```

Зелёными в обоих наборах остаются РОВНО контроли (снимочный хвост; дискриминатор окна
отказа) — то есть наборы не «краснеют на всё подряд», а различают миры. Против
восстановленного дерева те же наборы зелены целиком (`T5` гейта выше, поимённо).
