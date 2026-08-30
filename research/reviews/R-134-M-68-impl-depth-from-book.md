<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: 44d6aacb39934f01f28dbf7881c65c9b67964cda
verdict: REJECT
-->

# R-134 — M-68 «депт-серия на каждом L2-событии» (impl engine-dev): PR-time reviewer, **REJECTED**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-26
**Предмет:** `3b49620..44d6aac` на `origin/feat/M-68-rev3`; содержательный коммит один —
`44d6aac` «feat(M-68): tasks #1+2+3+4+5+7+8+9 …[engine-dev]», `crates/gateway/src/lib.rs`
(+234 −80). Остальные 20 коммитов диапазона — артефакты гейтов и RED-набор architect'а,
судимые прошлыми кругами (`C-094`, `C-138`, `C-156`, `C-160`, `C-162`, `C-164`, `A-018`,
`A-023`, `R-130`).
**Чекаут:** свой worktree `/tmp/hft-reviewer-m68`, detached от `origin/feat/M-68-rev3`
(`git rev-parse` сверен с мандатом), НЕ чужое дерево (`branch-hygiene` §1).
**Предшественник в цепочке:** tester, вердикт PASS. **Все его числа воспроизведены здесь
независимо и совпали** — см. Done Block. Находки ниже — те, которых гейт видеть НЕ МОЖЕТ.

**Ярус C прочитан ГРЕПОМ по предмету, не целиком** (`reading-map.md` §2):
`TECH-DEBT.md` по `TD-158|TD-159|TD-161|M-68` (`:92-94`, `:777`, `:797`, `:825-828`),
`PROJECT-STATE.md` по `M-68|TD-158|GATEWAY_SCHEMA_VERSION` (`:1499-1515`).

**Предъявление FA (M-66).** Диф трогает `crates/gateway/src/**`; FA этого крейта —
`docs/fa/viz-backend.md` (`scope-guard.md`: «`crates/gateway` = viz-backend Read Gateway,
Слой 8, M-22»). Живые инварианты, названные ПО ФАКТУ ПРОВЕРКИ на ревизии `44d6aac`:
**`VB-I-2`** (`docs/fa/viz-backend.md:189` — live == replay; шаг `F` гейта, зелён),
**`VB-I-5`** (`:192` — серия глубже 1.3 % несёт `depth_band_provenance`; предмет находки
`B-2`), **`VB-I-10`** (`:197` — bounded-window snapshot; шаг `E` гейта, зелён).
`FA-WAIVER` не нужен и не выписывается.

**RISK-BLOCK не применяется.** Диапазон не трогает `crates/{risk,killswitch,oms,venue-*}`
и `crates/contracts` — проверено `git diff --name-only` (см. Block-scope). `crates/gateway` —
read-only консюмер журнала (`VB-I-3`), order-egress отсутствует. `risk-critic` не требуется.

---

## Вердикт

**REJECTED.** Четыре блокера. Ни один из них гейт `verify_M-68.sh` поймать НЕ МОЖЕТ, и это
не претензия к гейту: три из четырёх — утверждения кода О СЕБЕ, четвёртый живёт на пути
(`LiveReducer::pump`), который ни один оракул набора не исполняет.

Работа по существу сделана и сделана хорошо: проводка верна, мутационный контроль реален,
соседние инварианты не куплены. Отвергается не механизм, а его самоописание, гранулярность
истории и один непокрытый путь.

---

## Block-scope — ПРОЙДЕН

```
$ git diff --name-only 3b49620..HEAD | grep -E '^(crates/contracts|crates/book|crates/venue|crates/journal|crates/risk|crates/killswitch|crates/oms|docker-compose.yml|docs/09-roadmap-v2.md|TECH-DEBT.md|PROJECT-STATE.md|docs/fa/)'
NONE — зона не нарушена
```

Impl-коммит `44d6aac` тронул РОВНО `crates/gateway/src/lib.rs` — зона engine-dev по
спеке §3. `*/tests/**` dev не тронул (проверено: единственный файл коммита) — RED-набор
sacred цел. RED-first соблюдён: тесты `aad0d89`/`b21a0cb`/`c6e596e`/`f625c39` предшествуют
impl'у в истории. Процессный слой (`gates.md` §11) не задет ⇒ `FOUNDER-APPROVED` не нужен.
Удалений защищённых артефактов нет.

## Block-DoneBlock — ПРОЙДЕН

Done Block tester'а — сырой stdout с exit-кодами, не пересказ. Воспроизведён независимо в
чистом worktree: совпало ВСЁ (см. Done Block ниже, `VERDICT: PASS`, `exit=0`).

## Block-C — ПРОЙДЕН

`crates/contracts/**` не тронут (шаг `H` гейта, независимо перепроверено). Форма события не
меняется — читаем то, что уже пишется (`CT-I-2`). contract-RFC не требуется.

---

## B-1 — БЛОКЕР: бандл-коммит на ВОСЕМЬ задач

**Что.** `44d6aac` — один коммит, subject которого сам называет восемь задач §Tasks:
`tasks #1+2+3+4+5+7+8+9`.

**Норма, нарушенная дословно, в трёх местах корпуса:**
- `docs/04-workflow.md` §4: «Атомарные коммиты: одна задача = ≥1 коммит… **Бандл на 5 задач
  = авто-reject**»;
- `.claude/rules/commit-discipline.md` §Атомарные коммиты: «Одна задача из §Tasks
  milestone'а = минимум один коммит. **Бандл на несколько задач одним коммитом = авто-reject
  reviewer'ом**»;
- `.claude/agents/reviewer.md` §6: «одна задача ≥1 коммит… **бандл-коммит на 5 задач =
  авто-reject**».

**Почему это не формализм именно здесь.** Задачи 1,2,3,4,5,7 — действительно ОДНО
семантическое изменение (одна точка проводки `recompute_depth_from_book`), и их слияние
защитимо. Но задача 8 (поле `ReadStats::depth_levels_visited` + смена сигнатуры
`reduce_event_stream`/`read_stats_from_stream` + 5 call sites) и задача 9 (bump
`GATEWAY_SCHEMA_VERSION` 8→9 + чейнджлог) — отдельные, самостоятельно откатываемые правки.
Именно их слипание с остальным делает находки `B-3`/`B-4` неоткатываемыми поодиночке:
чтобы вернуть счётчик, придётся трогать проводку полос.

**Воспроизведение:** `git log --oneline 3b49620..HEAD | head -1`.

**Условие снятия:** история ветки пересобирается минимум в три коммита — проводка
(1,2,3,4,5,7), счётчик (8), bump (9). Ветка в `main` не влита, переписывание истории
законно.

## B-2 — БЛОКЕР: код утверждает о себе ТРИ вещи, которые на этой же ревизии ложны

Класс — тот самый, ради которого созывался арбитр `A-023` («ложное самоописание») и заведён
`TD-138` («документ обосновывает инвариант механизмом, которого нет»). Все три утверждения
живут в `crates/gateway/src/**`, то есть в зоне dev'а, и снимаются им же.

### (i) `lib.rs:636-658` — нормативный комментарий поля `depth_reach_bid` описывает СНЯТУЮ семантику

Дословно на `44d6aac`:
```
:644  /// описывать ТЕ ЖЕ данные, из которых посчитаны числа полосы, а числа полосы snapshot-only:
:645  /// `L2Delta` двигает книгу и heatmap, но `depth_series` НЕ пересчитывает (M-22 семантика,
:646  /// см. ветку `MdPayload::L2Delta` в `apply`).
…
:649  /// 100 мс, то есть ПОСЛЕДНИЙ кадр почти всегда delta-only): кадр без снимка не несёт строк
:650  /// `depth_series` вовсе (их заводит только `L2Snapshot`), поэтому склейка
:651  /// `snapshot(C)+frames` про сдвиг охвата после последнего снимка узнать НЕ МОЖЕТ ни при
:652  /// какой семантике слияния…
```
Каждое из трёх выделенных утверждений опровергается кодом на 400 строк ниже, в этом же
коммите: `:1066-1067` (ветка `L2Delta` зовёт `ensure_depth_rows_initialized` +
`recompute_depth_from_book`) и `:1137-1160`. Комментарий не «устарел на полях» — он
НОРМАТИВЕН: он объясняет, ПОЧЕМУ охват снимается там, где снимается, и является тем самым
«вторым основанием», о котором спека §2bis сказала «РАСТВОРЯЕТСЯ». Растворение не выполнено.

Читатель этого поля завтра получит обоснование, прямо противоположное поведению — и это
ровно та поверхность, на которой `TD-158` и родился.

**Воспроизведение:**
`grep -n "числа полосы snapshot-only\|кадр без снимка не несёт строк" crates/gateway/src/lib.rs`
→ `:644`, `:649`; `sed -n '1060,1068p' crates/gateway/src/lib.rs` → противоположное.

### (ii) `lib.rs:933` и `:1118` — «zero дополнительных аллокаций» ЛОЖНО, и опровергается одной командой

Оба комментария утверждают, что уровни берутся из ТЕХ ЖЕ материализованных, что уже читает
`refresh_heatmap_bucket`, — «zero дополнительных аллокаций на такте». Замер:

```
$ grep -n "self\.book\.levels(" crates/gateway/src/lib.rs
1086:        let bids = self.book.levels(Side::Buy);      # refresh_heatmap_bucket
1087:        let asks = self.book.levels(Side::Sell);     # refresh_heatmap_bucket
1138:        let bid_levels = self.book.levels(Side::Buy);   # recompute_depth_from_book
1139:        let ask_levels = self.book.levels(Side::Sell);  # recompute_depth_from_book
```

`book::OrderBook::levels` (`crates/book/src/lib.rs:260-267`) — `…iter().map(…).collect()`,
то есть КАЖДЫЙ вызов аллоцирует новый `Vec` на всю глубину стороны;
`refresh_heatmap_bucket` эти два вектора ОТДАЁТ ВЛАДЕНИЕМ в бакет (`entry.refresh(bids,
asks)`), а `recompute_depth_from_book` материализует книгу ВТОРОЙ РАЗ. На L2-событие:
было 2 полнокнижных `Vec`, стало 4. Плюс на каждый вызов `depth_from_book` — `sums`
(`:952`) и `thresholds` (`:955`), ещё 4 мелких `Vec` на событие.

Дешёвая альтернатива существовала и названа самим комментарием: те же векторы лежат в
`self.heatmap_buckets[time_s]` сразу после `refresh`. Проектировать фикс — не моя зона
(`gates.md` §4, граница reviewer↔architect); фиксирую факт: **claim ложен, и он был опорой
ресурсного аргумента спеки §0.1bis.**

Ни один оракул этого не ловит: `d6a`/`d6b` считают ПОСЕЩЁННЫЕ УРОВНИ, а не аллокации;
`red_snapshot_noclone`/`red_gateway_bounded` меряют путь `snapshot()`/`finish_ref`, не
`apply`. Это `testing.md` §«Оракул границы ресурса меряет ресурс, а не прокси».

### (iii) `lib.rs:1134-1136` — «то же поведение, что прежний `depth_within` с `None` mid» ЛОЖНО

Комментарий:
```
:1134  /// Односторонняя книга (нет bid или ask) — early-return без записи: пороги полос
:1135  /// невычислимы, `depth_reach_*` остаются прежними (то же поведение, что прежний
:1136  /// `depth_within` с `None` mid).
```
Прежнее поведение (удалено этим же коммитом, `git show 44d6aac`):
`depth_within` при невычислимом mid возвращал `0`, а вызывающий цикл делал
`row.values.insert(time_s, 0)` БЕЗУСЛОВНО — точка в серию **писалась, со значением 0**.
Новое поведение — `return` до записи: **точки нет вовсе**. Это не «то же поведение», это
противоположное. Содержательное следствие — в `B-3`.

---

## B-3 — БЛОКЕР: неспецифицированная смена поведения на ДЕГЕНЕРИРОВАННОМ входе, ни одним оракулом не покрытая

**Что.** Односторонняя книга (нет bid либо нет ask — окно ресинка, тонкий инструмент,
первый L2-эвент сессии): раньше бакет получал точку `0`, теперь не получает точки.
Выдача `depth_series` меняет форму (пропуск точки вместо нулевой) — это видно консюмеру.

**Почему блокер, а не примечание.**
1. `testing.md` §«Дегенерированный вход обязателен» п.4 требует фикстуру «пусто / один
   элемент / граница» именно для такого класса; здесь класс СУЩЕСТВУЕТ в коде
   (`mid_from` → `Option`, явный early-return) и не покрыт ни одним из девяти `d*`:
   `grep -niE "односторон|one.sided|mid.*None" crates/gateway/tests/red_depth_from_book.rs`
   → ни одного совпадения по существу.
2. Смена не названа ни в спеке (§4 задач такой нет), ни в §3.1 (запретный список её не
   разрешает и не запрещает), ни в теле коммита — а комментарий (ii)(iii) прямо утверждает
   ОБРАТНОЕ. То есть изменение поведения выдачи проехало гейт как «эквивалентная правка».
3. `VB-I-2` (live == replay) при этом НЕ нарушен — оба пути ведут себя одинаково; шаг `F`
   зелён честно. Дефект не в расхождении путей, а в НЕЗАЯВЛЕННОЙ смене контракта выдачи
   и в отсутствии оракула на неё.

**Воспроизведение:** `git show 44d6aac -- crates/gateway/src/lib.rs` — удалённый блок
(`for row in &mut self.depth { row.values.insert(time_s, depth_within(...)) }` +
`depth_within`, возвращавший `0` при `(None, None)` mid) против нового
`recompute_depth_from_book` (`:1137-1142`).

**Зона фикса:** решение «0-точка или отсутствие точки» — семантика выдачи, то есть
architect (`gates.md` §4: reviewer описывает, architect проектирует + пишет RED-оракул на
регресс). Оракула на одностороннюю книгу в наборе нет — его надо завести до фикса.

## B-4 — БЛОКЕР: `ReadStats::depth_levels_visited` ломает контракт своей структуры на live-пути

**Что.** `ReadStats` объявлен аддитивным (`impl Add`, `:2251-2260`, и `ReadStats::sum`), и
все его поля — счётчики РАБОТЫ, сделанной ЭТИМ вызовом: `events_decoded`,
`segments_opened`, `events_scanned`, `segment_meta_ops` читаются из `stream`, который
создаётся заново на КАЖДЫЙ `pump()` (`:3448-3455`,
`journal::stream_from_at_with_catalog`). Новое поле на этом же пути читается иначе:

```
:3503  let stats = read_stats_from_stream(&stream, self.full.depth_levels_visited());
```

`self.full` — ПЕРСИСТЕНТНЫЙ аккумулятор сессии; его счётчик монотонно растёт с самого
старта `LiveReducer`. То есть один и тот же `ReadStats`, отданный из `pump()`, несёт
четыре величины «за тик» и одну «за всё время сессии». Суммирование тиков (`Add`/`sum` —
ровно то, для чего структура и заведена) даёт для нового поля квадратичный перечёт.

На путях `frames_since_with_stats` (`:2173`) и `snapshot_from_checkpoint`
(`:2337`, `:2364`) поле считается ПО ВЫЗОВУ — то есть семантика поля зависит от того,
каким API его получили. Это не тонкость реализации: это два разных смысла у одного имени.

**Почему оракул этого не видит.** `red_depth_recompute_cost` (`d6a`/`d6b`) ходит ТОЛЬКО
через `snapshot_from_checkpoint`:
`grep -n "snapshot_from_checkpoint" crates/gateway/tests/red_depth_recompute_cost.rs` →
`:142`. Путь `LiveReducer::pump` набором M-68 не исполняется ни разу.

**Цена сегодня — латентная, и я это называю честно:** `gateway-serve` логирует из
`ReadStats` только `events_decoded`/`segments_opened` (`crates/gateway-serve/src/lib.rs:1392-1396`),
нового поля пока не читает никто. Это «built-not-wired» наоборот: поле проводится в
публичный `ReadStats` уже сейчас, с двумя несовместимыми смыслами, и первый же его
потребитель унаследует ошибку молча. `gates.md` §4 «Механизм на пути (DoD)» — тот же класс.

**Зона фикса:** RED-оракул на `pump`-путь — architect; правка проводки — engine-dev.

---

## Находки НЕ блокирующие (названы, чтобы не потерялись)

**N-1 — сигнатура `depth_from_book` отклонена от зафиксированной спекой, без SCOPE VIOLATION REQUEST.**
Спека §4 (врезка) и §3.1 фиксируют
`fn depth_from_book(&self, levels: &[(i64, i64)], mid: i64, bands: &[f64]) -> (Vec<i64>, u64)`
и прямо числят «менять сигнатуру `depth_from_book`» в таблице ЗАПРЕЩЕНО. Реализация
(`:951`) добавила параметр `side: Side`. Отступление **названо честно** в комментарии
(`:940-944`), вред не материализовался (шаг `B` гейта воспроизведён здесь, мутация вносится,
набор красен), и по существу dev прав: без стороны порог невычислим — ошибка спеки, не его.
Но маршрут был предписан другой: `scope-guard.md` §«Формат SCOPE VIOLATION REQUEST» →
STOP и WAIT, решение architect'а. Фиксирую как процессную находку; блокером не делаю —
блокировать за то, что исполнитель починил дефект спеки и об этом СКАЗАЛ, значит поощрять
молчание.

**N-2 — `d6b` меряет обходы, а не цену; внутренний цикл по полосам ему невидим.**
`depth_from_book` инкрементирует счётчик ОДИН раз на уровень (`:966`), а сравнение с
порогами идёт по всем полосам внутри (`:968-976`) — то есть фактическая работа всё-таки
`O(levels × bands)`. Дословную формулировку спеки («семь полос обходят книгу столько же
раз, сколько одна») это НЕ нарушает: обход один. Но метрика структурно слепа к множителю,
который инвариант называет запрещённым, и это стоит знать при следующем ужесточении
бюджета.

---

## Что проверено и ПРОЙДЕНО (чтобы отказ не читался как «всё плохо»)

- **Проводка верна.** Обе ветки `apply` считают полосы от `self.book` — источник ОДИН с
  heatmap (`:1045-1046`, `:1066-1067`); хвост дельт входит в серию; охват снимается там же,
  где числа. `TD-158` по существу закрыт.
- **Мутационный контроль РЕАЛЕН, не греп.** Шаг `B` воспроизведён независимо: мутация
  вносится `perl` в копию дерева, набор гоняется ТАМ и КРАСНЕЕТ.
- **Соседние инварианты не куплены.** `VB-I-10` (`red_gateway_bounded`,
  `red_snapshot_noclone`) и `VB-I-2` (`red_gateway_live_eq_replay`) зелены; шаг `G`
  (`red_depth_provenance_by_reach`, 9/9) зелён.
- **Рычаг инвалидации выбран ВЕРНЫЙ.** `GATEWAY_SCHEMA_VERSION` 8→9 (`:83`), а не
  `CKPT_SCHEMA_VERSION`: отвергает чекпоинт со старым СМЫСЛОМ (`read_and_validate` шаг 3)
  и исполняет `П-014` п.3. `selector_fingerprint` не подогнан (шаг `J`).
- **Состав выдачи не тронут** (`GATEWAY_BANDS`, шаг `I`) — граница C соблюдена.

---

## Done Block (сырой stdout, свой worktree, независимый прогон)

```
$ pwd
/tmp/hft-reviewer-m68

$ git rev-parse HEAD; git rev-parse origin/feat/M-68-rev3
44d6aacb39934f01f28dbf7881c65c9b67964cda
44d6aacb39934f01f28dbf7881c65c9b67964cda

$ git log --oneline -1
44d6aac feat(M-68): tasks #1+2+3+4+5+7+8+9 — depth_from_book + ReadStats counter + 8→9 bump [engine-dev]

$ git merge-base HEAD origin/main
3b496208a64edbf00a66b93986ff8529d0c93aa9

$ git diff --name-only 3b49620..HEAD | grep -E '^(crates/contracts|crates/book|crates/venue|crates/journal|crates/risk|crates/killswitch|crates/oms|docker-compose.yml|docs/09-roadmap-v2.md|TECH-DEBT.md|PROJECT-STATE.md|docs/fa/)'
{пусто}

$ git show --numstat --format='' 44d6aac
234	80	crates/gateway/src/lib.rs

$ bash scripts/verify_M-68.sh 2>&1 | grep -E '^(===|PASS|FAIL|VERDICT)'; echo exit=$?
=== task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all ===
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
=== A (задачи 1,2,3,4,5,7) — набор MD-I-8 целиком ===
PASS: cargo test -p gateway --test red_depth_from_book --quiet
PASS: A состав набора — 9 оракулов (ожидалось ровно 9: d1 d2 d3 d4 d5 d7 d7b d8 d8b)
=== B (задача 4) — мутационный контроль ИСПОЛНЯЕТСЯ ===
PASS: B набор КРАСЕН против мутанта C-M68-1 (мутация внесена и прогнана в копии)
=== C (задача 8) — ресурсный оракул пути L2Delta → depth ===
PASS: cargo test -p gateway --test red_depth_recompute_cost --quiet
=== D (задача 9) — смена СЕМАНТИКИ объявлена bump'ом GATEWAY_SCHEMA_VERSION ===
PASS: D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)
PASS: cargo test -p gateway --test red_gateway_schema_version --quiet
=== E (задача 10) — VB-I-10 не ослаблен ===
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
=== F (задача 6) — VB-I-2 live == replay ===
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
=== G (задача 7) — метка и её числа сняты одним наблюдением ===
PASS: cargo test -p gateway --test red_depth_provenance_by_reach --quiet
=== H — Block-C: contracts не тронуты предметом ===
PASS: H crates/contracts не тронут
=== I — состав ВЫДАЧИ не тронут ===
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут
=== J (C-094 B3) — selector_fingerprint не подогнан под кэш ===
PASS: J selector_fingerprint не переписан
=== K — зона предмета ===
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: PASS
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | tail -2; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ grep -n "self\.book\.levels(" crates/gateway/src/lib.rs
1086:        let bids = self.book.levels(Side::Buy);
1087:        let asks = self.book.levels(Side::Sell);
1138:        let bid_levels = self.book.levels(Side::Buy);
1139:        let ask_levels = self.book.levels(Side::Sell);

$ grep -n "числа полосы snapshot-only\|кадр без снимка не несёт строк\|zero дополнительных аллокаций\|то же поведение, что прежний" crates/gateway/src/lib.rs
644:    /// описывать ТЕ ЖЕ данные, из которых посчитаны числа полосы, а числа полосы snapshot-only:
649:    /// 100 мс, то есть ПОСЛЕДНИЙ кадр почти всегда delta-only): кадр без снимка не несёт строк
933:    // `refresh_heatmap_bucket` на каждом L2-событии (zero дополнительных аллокаций; §0.1bis).
1118:    /// ветках `apply`) — zero дополнительных аллокаций на такте, `crates/book/src/**` не
1135:    /// невычислимы, `depth_reach_*` остаются прежними (то же поведение, что прежний

$ grep -n "snapshot_from_checkpoint\|LiveReducer\|pump" crates/gateway/tests/red_depth_recompute_cost.rs
142:    let (snap, stats) = gateway::snapshot_from_checkpoint(
      # LiveReducer/pump — НИ ОДНОГО совпадения
```

**Гейт зелёный, вердикт красный — и это не противоречие.** `verify_M-68.sh` судит то,
что ему поручено судить, и судит честно. Четыре блокера выше лежат вне его поля зрения по
конструкции: три — утверждения кода о себе (машине не проверяемые), один — путь, который
набор не исполняет.

## Cross-references

- `milestones/M-68-depth-from-book.md` §3.1 (запретный список), §4 (врезка о сигнатуре), §2bis
- `research/arbitration/A-018-m68-cadence-not-reach.md` · `A-023-m68-artifact-self-consistency.md`
- `research/reviews/R-130-M-68-a023-closing-commit.md` (предыдущий круг reviewer'а — docs-коммит)
- `docs/fa/viz-backend.md:189` (`VB-I-2`), `:192` (`VB-I-5`), `:197` (`VB-I-10`)
- `.claude/rules/commit-discipline.md` §Атомарные коммиты · `docs/04-workflow.md` §4
- `.claude/rules/testing.md` §«Дегенерированный вход обязателен», §«Оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ»
- `TECH-DEBT.md:92` (`TD-158`) — по существу закрыт этой работой, карточку закрывает
  reviewer в close-out ПОСЛЕ снятия блокеров
