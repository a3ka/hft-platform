<!-- GATE-META
milestone: M-88
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: 4d86c2c0d03c40ab80043458fd8e607d3f2e787b
verdict: APPROVE
-->

# R-197 — M-88 «снятая ликвидность исчезает и у клиента»: PR-time гейт, круг 2 — **APPROVED**

**Дата (UTC):** 2026-09-23 · **Роль:** reviewer · **Ветка:** `feat/M-88-liquidity-removal-contract`
**База аудита:** `d5163b5` (= `origin/main`; `git merge-base --is-ancestor origin/main HEAD` → 0, ветка НЕ отстаёт)
**Вершина:** `4d86c2c` — взята командой `git rev-parse origin/feat/M-88-liquidity-removal-contract`, не из мандата.
**Предыдущий круг:** `research/reviews/R-195-m88-close-out.md` (REJECT на `fdfde17`).

## Вердикт

**APPROVED.** Оба блокера круга 1 закрыты, и закрыты той формой, которую я требовал:
подчинение списка наблюдений окну сделано ОДИНАКОВО на живом пути и на пути пересчёта, а
противоречие текста и атрибута снято правкой ТЕКСТА — то есть без появления `skip`, который
развёл бы две стороны `VB-I-2`. Приёмка у меня зелёная самостоятельным прогоном:
**17 PASS / 0 FAIL / `VERDICT: PASS` / exit=0**.

Главное, ради чего этот круг вообще имел смысл: **новые оракулы КРАСНЕЮТ против дефекта, а не
украшают набор** — предъявлено тремя изолированными мутациями ниже, включая ОБРАТНУЮ
(односторонняя починка), которую ловит сторож `W2`.

**Тронут `crates/gateway/**` ⇒ предъявляю живые инварианты его FA** (`docs/fa/viz-backend.md`,
ревизия `4d86c2c`): **`VB-I-10`** (`:208` — «память `snapshot`/`frames_since` ограничена ОКНОМ
`[at−W, at]`, **НЕ числом time-бакетов истории**») и **`VB-I-2`** (`:200` — live == replay).
Находка `R-195` Б-1 была прямым регрессом `VB-I-10`; её закрытие проверено по этому же ID.

---

## 1. Что изменилось с моего REJECT — проверено ПОКОММИТНО, не по отчёту

```
$ git log --oneline fdfde175..4d86c2c
4d86c2c fix(M-88): task9 — исключение по КЛАССУ вместо перечисления имён [architect]
7de7c3d fix(M-88): task #11 — текст поля heatmap_buckets_observed говорит ПРАВДУ о serde [engine-dev]
db18687 fix(M-88): task #10 — список наблюдений подчинён ОКНУ на обоих путях [engine-dev]
cf92b8e docs(M-88): §13bis — решение по вердикту R-195, задачи 10-11, границы зоны [architect]
46bb458 feat(M-88): гейт — шаги 10 и 11 по вердикту R-195 [architect]
c23c8cf test(M-88): перечень сторожа окна дополнен списком наблюдений [architect]
8354974 test(M-88): task #10 — RED: список наблюдений обязан следовать ОКНУ, не истории [architect]
e14d299 docs(M-88): R-195 PR-time гейт — REJECT: новое состояние редьюсера растёт с историей [reviewer]
```

Порядок верен по `gates.md` §2: RED (`8354974`, `c23c8cf`) → гейт (`46bb458`) → impl
(`db18687`, `7de7c3d`). Реализация НЕ опередила оракул.

### Б-1 закрыт — и симметрия есть свойство конструкции, а не обещание

`crates/gateway/src/lib.rs:1278` (в `Reducer::evict_window_state`, в том же блоке эвикции, ниже
`self.heatmap_buckets.retain(...)` на `:1272`):

```rust
self.heatmap_buckets_observed.retain(|&t| t >= lo_time_s);
```

`:3176` (в `evict_series_bundle_under_window`, рядом с `series.heatmap.retain(...)` на `:3168`):

```rust
series.heatmap_observed_time_s.retain(|&t| t >= lo_time_s);
```

Тот же предикат, та же граница `lo_time_s`, та же точка вызова — ровно форма §13bis.2.
Третьего места действительно не требуется: `Snapshot::apply` зовёт
`evict_series_bundle_under_window` под финальным окном кадра, и клиентское накопление
закрывается той же правкой (проверено оракулом `W3`, а не рассуждением — см. §3).

### Б-2 закрыт — правкой ТЕКСТА, атрибут `#[serde(default)]` сохранён

`:1047-1057` — док-комментарий больше не утверждает `#[serde(skip, default)]`; он называет
поле частью чекпоинта НАМЕРЕННО и объясняет, почему `skip` был бы дефектом
(`snapshot_from_checkpoint` обещает байт-идентичность с `gateway::snapshot`; со `skip`
редьюсер из слепка объявил бы список пустым там, где редьюсер из реплея объявляет его
полным). Это решение §13bis.3, и оно верное: обратный выбор ломал бы `VB-I-2` с другого конца.

Гейт `task11` держит обе редакции лжи симметрично (`scripts/lib/m88_predicates.sh:m88_task11`):
`skip` в атрибуте → FAIL; `skip` в тексте без атрибута → FAIL; поле исчезло → FAIL (fail-closed).

---

## 2. Block-scope

| проверка | результат |
|---|---|
| `crates/contracts/**` тронут | **НЕТ** (0 файлов) — Block-C неприменим, T-designate по `05` §2 |
| `crates/risk\|killswitch\|oms\|venue-*` тронуты | **НЕТ** (0 файлов) — RISK-BLOCK неприменим, risk-critic не требуется |
| impl вне `crates/gateway/src/lib.rs` | НЕТ (весь диапазон `d5163b5..4d86c2c`) |
| dev правил `*/tests/**` | **НЕТ.** Файлы всех восьми `[engine-dev]`-коммитов: `crates/gateway/src/lib.rs`, `docs/plans/m88-frame-size-measurement.md`, `milestones/M-88-*.md` (последний — `efda2d6`, 9 строк: РОВНО колонка Status, carve-out `scope-guard.md`) |
| RED раньше impl | ДА, в обе итерации: `f3e40a9`→`670c5e9` (круг 1), `8354974`→`db18687` (круг 2) |
| атомарность | 42 коммита, задача → коммит, ссылка на `M-88` в каждом subject'е; метки ролей: 28 architect / 8 engine-dev / 5 critic / 1 reviewer |
| `§7 Allowed paths` | дополнен этим кругом по моему `R-195` NOTE-1 — предписанные §12.4 файлы (`scripts/lib/m88_predicates.sh`, `scripts/tests/red_verify_M-88.sh`, `scripts/tests/red_m88_mutants.sh`) и артефакт задачи 8 теперь перечислены. Противоречие внутри документа снято |
| `docs/ROADMAP.md` — колонка «Состояние» | **НЕ тронута**: маркер строки `P0-CORR` на обеих ревизиях идентичен (`🟡 НАПРАВЛЕНИЕ ПРИНЯТО 2026-09-21`); диф в строке — изложение хода работ. Граница `R-195` NOTE-2 соблюдена и закреплена текстом §7 |

Вне §7 в диапазоне остаются вердикты гейтов (`C-233/235/237/239/241`, `A-036`, `R-195`) и
`docs/ROADMAP.md` — законны по `gates.md` §4/§0 и §9 соответственно; плюс `PROJECT-STATE.md`
и `TECH-DEBT.md`, которые вношу я сам этим close-out'ом (`gates.md` §4, reviewer-owned).

---

## 3. Оракулы КРАСНЕЮТ против дефекта — три изолированные мутации, сырой вывод

Мутировалось дерево ревьюера (`/tmp/hft-reviewer-m88-r2`), код восстанавливался после каждой;
итоговое состояние дерева — чистое (Done Block ниже).

**МУТАЦИЯ 1 — снят `retain` в `evict_window_state` (путь редьюсера):**

```
$ sed -i 's|^        self\.heatmap_buckets_observed\.retain(|        // МУТАЦИЯ РЕВЬЮЕРА: ...|'
$ cargo test -p gateway --test red_m88_observed_window
thread 'w4_every_bundle_field_is_classified_as_windowed_or_not' panicked at
  crates/gateway/tests/red_m88_observed_window.rs:352:9:
heatmap_observed_time_s: 119 точек вне окна [1752000119, 1752000179]
  (первые: [1752000000, 1752000001, 1752000002, 1752000003, 1752000004]) — VB-I-10
failures:
    w1_observed_list_is_window_bounded_not_history
    w2_fold_equals_replay_on_observed_list
    w4_every_bundle_field_is_classified_as_windowed_or_not
test result: FAILED. 1 passed; 3 failed
```

**МУТАЦИЯ 2 — ОБРАТНЫЙ вопрос мутационного контроля (`testing.md`): что ослаблено РЯДОМ.**
Снят `retain` ТОЛЬКО на клиентском пути (`evict_series_bundle_under_window`), путь редьюсера
цел — то есть починка сделана ОДНОСТОРОННЕ:

```
thread 'w3_client_accumulation_stays_window_bounded' panicked at
  crates/gateway/tests/red_m88_observed_window.rs:294:5:
накопленный у клиента список наблюдений держит 119 колонок вне окна [1752000119, 1752000179]
  (всего 180). Долгоживущая сессия кокпита копила бы его неограниченно
failures:
    w2_fold_equals_replay_on_observed_list
    w3_client_accumulation_stays_window_bounded
test result: FAILED. 2 passed; 2 failed
```

Это и есть доказательство, которого я требовал при передаче: **`W2` краснеет не на дефекте, а
на НЕСИММЕТРИЧНОЙ починке**. Сторож соседнего инварианта существует и работает в обе стороны.

**МУТАЦИЯ 3 — ЦЕНА, а не форма.** Та же мутация 1, ресурсный оракул:

```
$ cargo test -p gateway --test red_m88_observed_response_limit
R-195 Б-1 (PL-I-5): снимок НЕ ОТДАН на истории в 20000 бакетов при пределе 150000 Б —
«PL-I-5: response exceeds limit: limit=150000 bytes, observed=291178 bytes
 (heatmap=488 cob=8 vp_bins=1 bubbles=61 ohlcv=61 depth_rows=2)»
test result: FAILED. 0 passed; 1 failed
```

**ВОССТАНОВЛЕНИЕ И ПЕРЕПРОВЕРКА:**

```
$ git status --porcelain
{пусто}
$ cargo test -p gateway --test red_m88_observed_window
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
```

Оба оракула несут setup-стражи, и это не формальность: `W1` падает, если окно не обрезало
историю ИЛИ список наблюдений пуст (`:143`, `:149`) — ровно два способа, которыми сценарий
выродился бы в тавтологию, и ровно те, которыми он был пропущен оракулами круга 1
(их фикстуры состояли из одних сделок). `W3` падает, если батчей вышло ≤3 — накоплению
негде было бы копиться. `red_m88_observed_response_limit` двигает процессный предел явно и
восстанавливает его `Drop`-стражем на любом выходе, включая панику.

---

## 4. Done Block — гейт прогнан МНОЙ, сырой вывод

```
$ git rev-parse origin/feat/M-88-liquidity-removal-contract
4d86c2c0d03c40ab80043458fd8e607d3f2e787b

$ git worktree add --detach /tmp/hft-reviewer-m88-r2 origin/feat/M-88-liquidity-removal-contract
HEAD is now at 4d86c2c fix(M-88): task9 — исключение по КЛАССУ вместо перечисления имён [architect]

$ git status --porcelain
{пусто}

$ bash scripts/verify_M-88.sh > /tmp/m88-rev.txt 2>&1; echo "exit=$?"
exit=0

$ grep -E '^(PASS|FAIL|VERDICT)' /tmp/m88-rev.txt
PASS  task1: поля контракта объявлены в SeriesBundle
PASS  task1: GATEWAY_SCHEMA_VERSION поднят до 11
PASS  task2: док-комментарий ПОЛЯ heatmap_cells объявляет полный срез и не предписывает объединение
PASS  task3-6: red_m88_update_contract — test result: ok. 14 passed; 0 failed; ...
PASS  task1+7: red_m88_contract_form — test result: ok. 9 passed; 0 failed; ...
PASS  task7: apply возвращает ApplyOutcome и помечен must_use НЕПОСРЕДСТВЕННО перед сигнатурой
PASS  task8: результат до/после датирован существующей ревизией и снят названной командой
PASS  task9: решение записано по каждому изменённому ожиданию (дифф от d5163b5b35abbca204a8981e974bd6e5a97eb9de)
PASS  R4: батарея мутантов ЗЕЛЕНА — обе изолированные мутации красят набор
PASS  A-036 п.1: эталонные векторы предъявлены и сходятся — test result: ok. 2 passed; 0 failed; ...
PASS  проба предикатов: все сценарии сошлись (scripts/tests/red_verify_M-88.sh)
PASS  task10: red_m88_observed_window — test result: ok. 4 passed; 0 failed; ...
PASS  task10: бюджет ответа не съеден списком наблюдений — test result: ok. 1 passed; 0 failed; ...
PASS  task11: док-комментарий heatmap_buckets_observed не противоречит своему serde-атрибуту
PASS  CI-паритет: cargo fmt --all -- --check
PASS  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
PASS  CI-паритет: cargo test --all
VERDICT: PASS

$ grep -cE '^PASS' /tmp/m88-rev.txt
17
```

Решение принято по КОДУ ВОЗВРАТА (`exit=0`), а не по тексту вывода (`gates.md` §3).

---

## 5. Ярус C — что искал грепом (файлы читаются запросом, не целиком)

`TECH-DEBT.md` (565 КБ) и `PROJECT-STATE.md` (300 КБ) прочитаны грепом по предмету
(`reading-map.md` §2):

- `grep -n "built-not-wired" TECH-DEBT.md` → `TD-202` и три прецедента формулировки severity
  (`:230`, `:1988`, `:2853`) — форма карточки взята оттуда;
- `grep -n "M-88\|P0-CORR" TECH-DEBT.md PROJECT-STATE.md` → **ноль попаданий**: ни карточки
  долга, ни раздела состояния по предмету ещё нет, заводить их мне;
- `bash scripts/next_artifact_id.sh R` → `R-197`; `… TD` → `TD-217`. Номера выданы
  механизмом, не выбраны (`gates.md` §12).

---

## 6. Замечания без блокировки — три, и ни одно не стоит круга

**NOTE-1. Колонка Status задач 10 и 11 осталась `⏳ OPEN`** (`milestones/M-88-*.md:270-271`),
хотя работа сделана и принята гейтом. Правка колонки — carve-out dev'а/architect'а
(`scope-guard.md` §«Milestone-файлы»), в мою зону не входит; вреда нет, потому что состояние
предмета определяет `verify`, а не таблица. Чинится строкой при close-out'е architect'а.

**NOTE-2. Батарея мутантов `R4` не покрывает новую эвикцию.** `scripts/tests/red_m88_mutants.sh`
несёт двух мутантов, и оба — про семантику склейки (`МУТАНТ-А` объединение, `МУТАНТ-Б` пустой
срез). Мутанта «снят `retain`» в ней нет, поэтому анти-плацебо для правки круга 2 держится на
моём ручном прогоне (§3), а не на гейте. Класс тот же, что спека нашла у себя в §3: механизм
проверки не подан на путь, где он бы понадобился. Кандидат на механизацию назван — `МУТАНТ-В`
по образцу двух существующих; зона architect'а.

**NOTE-3. Док-комментарий скаляра `cob_observed` (`:1059-1064`) обещает окно, которого у него
нет.** Текст: «`true` ⇔ в окне редьюсера было ХОТЯ БЫ одно L2-событие». Фактически флаг
выставляется на `:1595`/`:1635` и НИКОГДА не сбрасывается — `evict_window_state` его не
трогает, и после ухода последнего L2-события за границу окна он остаётся `true`. Это НЕ
блокер и не тот же класс, что Б-2: направление безопасно по существу — `SeriesBundle.cob` есть
точечный срез книги на `at`, а не бакет-оконный ряд, поэтому `Replace` полного среза верен при
любом возрасте последнего L2; цена — один бит. Но формулировка «в окне редьюсера» неточна, а
на неточных формулировках в этом же милестоуне уже потерян круг. Зона правки — architect.

---

## 7. Что НЕ проверено и почему — названо, а не умолчано

- **Прогон против реального фронта (`code2alpha`)** — внешняя зависимость `A-036` §4.3,
  владелец founder. Именно поэтому строка `P0-CORR` НЕ закрывается (§8 ниже).
- **Стоимость прогрева слепка на форме v11** не измерена (§11 п.6 спеки). Ограничение
  остаётся открытым и названо в спеке; после закрытия Б-1 оно больше не бессмысленно, но и
  не снято.
- **Эталонные векторы к оконному пути слепы** — все одиннадцать сняты при `window_ms: null`
  (замер §13bis.4, проверен мной грепом по `crates/gateway/tests/fixtures/m88/*/snapshot.json`).
  Это названо спекой честно; оконный путь покрывают `W1`–`W4`, а не векторы.
- **Post-merge деплой-гейт** (`gates.md` §8) исполняется ПОСЛЕ merge'а; пруф — в отчёте
  close-out'а.

---

## 8. Close-out — мои обязанности по §12.2, исполняются ВМЕСТЕ с APPROVED

1. **`TD-217` заведён**, класс «built-not-wired», **severity MAJOR**: контракт обновления
   реализован в серверной библиотеке, но сквозная приёмка реальным потребителем не
   состоялась — фронт его ещё не исполняет. `gates.md` §4 DoD «Механизм на пути»: подключение
   осознанно отложено ⇒ долг виден, а не подразумевается.
2. **Строка `P0-CORR` в `docs/ROADMAP.md` НЕ ЗАКРЫВАЕТСЯ** — решение арбитра `A-036` §4.3 п.3:
   merge `M-88` закрывает серверную половину, а не предмет. Закрывает его прогон эталонных
   векторов против фронта.
3. `PROJECT-STATE.md` и `TECH-DEBT.md` обновлены этим же диапазоном.

## Cross-references

- `docs/fa/viz-backend.md` §5 — `VB-I-2` (`:200`), `VB-I-10` (`:208`)
- `milestones/M-88-liquidity-removal-contract.md` §7, §12.2, §13bis, §14.1
- `research/reviews/R-195-m88-close-out.md` (круг 1, REJECT) · `research/arbitration/A-036-m88-real-client.md`
- `.claude/rules/gates.md` §3 (решение по коду возврата), §4 (PR-time, FA-предъявление), §8 (деплой-гейт)
- `.claude/rules/testing.md` §«Мутационный контроль — вопросов ДВА»
