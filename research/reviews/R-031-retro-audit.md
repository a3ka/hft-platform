# R-031 — РЕТРО-АУДИТ: M-32/M-33/M-34 (merge без артефакта гейта) + сверка PROJECT-STATE + процессный вывод

**Роль:** reviewer. **Дата:** 2026-08-03 (UTC). **Ветка:** `docs/retro-audit` (НЕ main — на main
параллельно работает другой reviewer). **База аудита:** worktree `/tmp/hft-rev-retro`, `origin/main`
= `b5d82f2` (код-часть тождественна `f5907e2`, на котором прогонялись тесты: `git diff --stat
f5907e2 b5d82f2 -- crates/ scripts/` = только `scripts/measure_M-54_connect.sh` +15).

**Тип:** ретро-аудит, НЕ PR-гейт. Ничего не мержится, код не правится. Пишутся только
`research/reviews/R-031-*.md` (этот файл) и `TECH-DEBT.md`.

**Итог одной строкой:** три milestone'а действительно уехали в `main` без коммитнутого артефакта
гейта, но причина НЕ та, что предполагалась (правило существовало с первого дня; отсутствовал
механизм, делающий вердикт reviewer'а артефактом). Технически: M-34 — чисто, M-33 — с оговорками,
**M-32 несёт живой методологический дефект: метрика, на которой стоит founder-подписанное решение,
систематически смещена в сторону "полосы живые"** (TD-103).

---

## Вердикты

| milestone | вердикт | суть |
|---|---|---|
| **M-32** depth-verification | **RETRO-DEFECT** | `cancel_fraction` слепа к перерождению уровня на той же цене ⇒ смещение вверх; ни один оракул этого не покрывает. Плюс 2 NOTE (scope, отсутствие critic'а при сработавшем триггере) |
| **M-33** depth-band 30-60% | **RETRO-NOTE** | оракулы и scope чисты; наследует метрику M-32; пре-регистрированный founder-флаг требует падения ПО ОБЕ стороны (конъюнкция) ⇒ слеп к односторонней заморозке |
| **M-34** funding-breadth | **RETRO-NOTE** | код корректен и wired сегодня; но breadth не покрыт ни оракулом (тест видит чистую функцию, не call-site), ни алертом (алерт на ноль, не на схлопывание широты) |

Гейты на сегодняшнем `main` — все зелёные (Done Block ниже): `verify_M-32/33/34.sh` VERDICT PASS,
exit=0; 14 DV-I оракулов + 3 FB-I оракула GREEN.

---

## Часть A — ретро-ревью

### A.0 Что оказалось фактом, а что — артефактом наблюдения

**Merge'и сделаны НЕ reviewer'ом — подтверждено:**

```
bb00915 engine-dev 2026-07-24 merge(M-32): depth-verification ...
6ec6453 engine-dev 2026-07-24 merge(M-33): depth-band 30-60% ...
211e452 engine-dev 2026-07-25 merge(M-34): funding-breadth ...
```

**Но найдено то, чего в исходной картине не было:** сразу за каждым merge'ем идёт close-out-коммит
ТОЙ ЖЕ личности, который правит `PROJECT-STATE.md` и `TECH-DEBT.md` — файлы, куда по
`scope-guard.md` пишет ТОЛЬКО reviewer:

```
c9a8f64 engine-dev docs(M-32): reviewer close-out — APPROVED/merged bb00915 ...  PROJECT-STATE.md +45, TECH-DEBT.md +12
e2cd735 engine-dev docs(M-33): reviewer close-out — APPROVED/merged 6ec6453 ...  PROJECT-STATE.md +36
17fc51c engine-dev docs(M-34): reviewer close-out — APPROVED/merged 211e452 ...  PROJECT-STATE.md +32, TECH-DEBT.md +8-1
41c328f tester    docs(M-20): reviewer close-out — VWAP APPROVED/merged (743d0b3) ...
e8885b9 tester    docs(M-22): reviewer close-out — Read Gateway APPROVED/merged (7799ff2) ...
```

Более того, записи в `PROJECT-STATE.md`, сделанные этими же коммитами, называют **отдельный
worktree на каждый milestone**: «Гейты (reviewer перепрогнал независимо, worktree
`/tmp/hft-reviewer-M-32`, чистый checkout)» (`PROJECT-STATE.md:280`), то же для
`/tmp/hft-reviewer-M-33` (:316) и `/tmp/hft-reviewer-M34` (:494). Это самое сильное имеющееся
свидетельство в пользу «reviewer работал, но под чужой git-личностью» — и оно всё ещё
самоаттестация: worktree'ев давно нет, проверить нечем.

Ровно тот же паттерн у обоих tester-merge'ей (M-20, M-22). То есть во всех пяти случаях работу
делала **reviewer-сессия с git-личностью предыдущей роли** (личность ставится один раз при
бутстрапе worktree и не переустанавливается при смене роли) — а не dev, самовольно мерживший
свой код. Это **смягчает** обвинение, но не снимает его: единственный различитель «независимый
reviewer» vs «dev ревьюит себя» — это git-личность, и она соврала. Ретроспективно развести эти
две гипотезы **невозможно** — транскриптов сессий нет, а вердикта-файла тогда не существовало
как класса (см. Часть C).

**Уточнение счёта: из восьми «merge'ей не тем агентом» один не является merge'ем в main.**
`ad841d9` [venue-dev] — `chore(M-08): sync feat/M-08 base`, синк-коммит ВНУТРИ feat-ветки; в
историю main он попал вместе с merge'ем M-08, который делал reviewer:

```
$ git rev-list --first-parent origin/main | grep -qx ad841d9 && echo да || echo НЕТ
НЕТ (боковая ветка/синк)
```

Это разрешённый intra-chain push на feat-ветку (`gates.md` §8, закреплено 2026-07-12 после TD-014),
а не обход гейта. На first-parent линии main остаются шесть: `bb00915`, `6ec6453`, `211e452`,
`545a41b` (engine-dev, M-35 survey), `743d0b3`, `7799ff2` (tester) плюс `83c340c` [engine-dev] —
безымянный `Merge remote-tracking branch ... into engine-dev-hft-engine-dev-1784572377` (M-09),
у которого нет даже conventional-subject'а.

**Отсутствие `research/reviews/R-NNN.md` за M-32/33/34 — НЕ нарушение того времени.** Каталог
заведён 2026-07-31 коммитом `7888be4` (R-001, M-49); требование «вердикт reviewer'а — артефакт»
закреплено в `gates.md` §4 коммитом `5439b1c` (2026-07-31), т.е. на неделю ПОЗЖЕ. До этой даты
вердикт reviewer'а по правилам жил в переписке. Поэтому «нет вердикта reviewer'а» для M-32..34 —
ожидаемо; настоящая находка в другом (Часть C).

**Отсутствие вердикта critic'а — нарушение, и оно реально.** По `gates.md` §1 триггер 3
(«оценка ≥5 атомарных коммитов») на M-32 сработал: в цепочке 14 коммитов до merge'а. Milestone-файл
`milestones/M-32-depth-verification.md:5` явно постулирует обратное: «Критик НЕ триггерится
(`crates/research-cli/{src,tests}`, `research/data-quality/` — не contracts/risk/ks/oms/venue,
не новый крейт) → reviewer-бэкстоп» — перечислены триггеры 1/2/4 и пропущен триггер 3. То есть
plan-time гейт был снят **обоснованием, которое не покрывает все триггеры правила**. Для M-33
(3 коммита) и M-34 (2 коммита) критик действительно не требовался.

### A.1 M-32 — depth-verification → **RETRO-DEFECT**

**Scope.** Allowed paths (`milestones/M-32-depth-verification.md:138-146`) разрешают research-dev
`crates/research-cli/src/{depth_lifetime.rs,orderflow.rs,lib.rs}` + `research/data-quality/*.md` +
свой `Cargo.toml` `[dependencies]`. Фактический diff содержит сверх этого:

- `crates/research-cli/examples/depth_lifetime.rs` (+272), добавлен `616bec1` [research-dev];
- секцию `[[example]]` в `crates/research-cli/Cargo.toml` — это **не** `[dependencies]`, а правка
  build-конфига, которую `scope-guard.md` («Билд-конфиги — shared-access правило») не покрывает.

Оба — за пределами Allowed paths. Справедливости ради: тогдашний reviewer это **видел и снял** —
`PROJECT-STATE.md:282` фиксирует «Scope: diff ⊂ allowed (Cargo.toml — additions-only `[[example]]`,
не правка чужих deps)». Но обоснование не сходится с текстом правила: `scope-guard.md` разрешает
dev'у добавлять СВОИ `[dependencies]`, а не заводить новые build-таргеты, и milestone перечислял
`src/{...}`, не `examples/`. Итог — **NOTE-1**: по существу оправдано (именно этот пример произвёл
прод-числа вердикта), по процедуре — расширение Allowed paths задним числом, которое должен был
санкционировать architect правкой milestone'а, а не reviewer формулировкой в close-out'е.
В зачёт цепочке: research-dev сам откатил избыточный lint-конфиг в `6b9577a` — self-correct.

**RED-first.** Порядок соблюдён строго и проверяем по `git log`: `d08141e` (DV-I-1..5 RED) и
`e91b843` (DV-I-6 RED + verify) → `6c3b73d`/`616bec1` (impl research-dev) → `4adbb5f`/`7813a33`
(усиление DV-I-7/8 после инцидента O(n²)) → merge. Тесты писал architect, impl — research-dev;
подмены тестов под реализацию нет.

**Анти-плацебо оракулов — по существу выдерживают.** DV-I-7/8 (`red_depth_scale.rs`) меряют
границу ресурса на 120k/400k событиях с таймаутом 15с и растущим числом distinct-цен — на
O(n²)-реализации инцидента не укладываются, это не тавтология и не «половина работы»
(`analyze` вызывается целиком, `consistency` целиком). DV-I-6 покрывает обе стороны
(`dv_i_6_trade_with_book_decrement_is_consistent` И `..._without_...is_inconsistent`) — не
односторонняя проверка. Есть отдельный чек-лист-тест `dv_i_checklist_asymmetry_and_multiplicity`.

**ДЕФЕКТ (TD-103) — метрика слепа к перерождению уровня; смещение направлено в сторону вывода.**

`crates/research-cli/src/depth_lifetime.rs:171`:

```rust
let new_birth = !self.states.contains_key(&l.price);
```

`states` **никогда не чистится** (это осознанно — так фиксируется fate после `size=0`,
строки 127-139). Следствие: цена, которая была отменена (`fate = Cancelled`) и потом **родилась
снова** тем же/другим ордером, не считается новым рождением, не возвращается в `alive`
(строка 182 достижима только при `new_birth`), и её fate навсегда остаётся `Cancelled` — даже
если уровень стоит живым до конца окна.

Проба (модуль взят вербатим из `main`, подменены только контрактные типы `Level`/`Side`;
исходники пробы — `<scratchpad>/probe/`):

```
ПРОБА-1 (цена отменена на t2, РОДИЛАСЬ СНОВА на t3, жива до конца окна):
  полоса [500,800) bid: born=1 cancelled=1 frozen=0 censored=0 cancel_fraction=Some(1.0)
```

Честный ответ для этого входа — `frozen` (уровень стоит на конце окна). Получено `cancelled`,
`cancel_fraction = 1.0`.

Почему это важно именно здесь. Заголовочное число вердикта —
`cancel_fraction = cancelled / (cancelled + frozen)` (строки 81-91), и оно интерпретируется в
`research/data-quality/depth-verdict.md:50` как «дальние уровни РЕАЛЬНО отменяются биржей в 80.5%
случаев — ЖИВЫЕ, не зависают вечно». Фактически измеряется другое: **доля distinct-ЦЕН, которые
хотя бы раз за окно получили `size=0`**. Эта величина:

1. **монотонно растёт с числом посещений цены** — она насыщается там, где churn выше. NEAR
   (плотная сетка у mid, тысячи касаний за 3.4 ч) даёт 0.981 практически по построению;
2. **смещена ВВЕРХ ровно в ту сторону, которая подтверждает вывод**: любой уровень, который был
   отменён однажды и после этого «завис» навсегда — то есть **буквально сигнатура фантома
   TD-016** — учитывается как `cancelled`, а не как `frozen`;
3. делает сравнение полос между собой (ключевой аргумент «FAR=0.805 ≈ того же порядка, что
   NEAR=0.981» и весь монотонный тренд 0.867→0.796→0.622 у M-33) **сравнением величин с разной
   насыщенностью**, а не сравнением живости.

Ни один из 14 оракулов не содержит фикстуры «отмена → перерождение на той же цене» (проверено
grep'ом по `red_depth_lifetime.rs`/`red_depth_band_3060.rs`; `dv_i_checklist_asymmetry_and_multiplicity`
покрывает множественность УРОВНЕЙ в одном тике, но не множественность ЖИЗНЕЙ одной цены). Это
ровно пункт «Множественность» из `testing.md` §«Фикстура счастливого пути», применённый не к тому
измерению.

**Побочно опровергается утверждение в `PROJECT-STATE.md:272`** — «Анти-плацебо в обе стороны в
КАЖДОМ оракуле». Для проверенных направлений это верно, но ни один оракул не давит на инвариант
«одна цена — несколько жизней», поэтому утверждение шире, чем факт. Формулировку правит эта же
запись (см. Часть B, правка PS-1).

**Класс дефекта:** методологический, НЕ рантаймовый. `research-cli` — оффлайн-инструмент вне
торгового пути; порчи журнала/прода нет. Но на этом числе стоит **founder-подписанное решение
границы C** (`depth-verdict.md:11-24`, диапазон полос 1.5–60% + `depth_band_provenance`), поэтому
цена ошибки не нулевая: продукт показывает полосы глубины как «живо-верифицированные».

**Что НЕ является дефектом (проверено, снимаю подозрения):**

- `band_for_bps` (строки 244-251): `BANDS_BPS` начинается с `(0,150)`, поэтому near-уровни
  (`|bps| < 1500`) НЕ проваливаются в fallback последней полосы — атрибуция корректна;
- «каскад gap'ов» (`depth-verdict.md` §Risks-1) воспроизводится и объяснён верно: при gap
  `prev_final_update_id` намеренно не обновляется (строки 293-300, fail-closed как в
  `book::OrderBook`), поэтому каждый следующий тик — тоже gap. Проба-2 (один разрыв на 4 тиках):
  `gaps=3`, все уровни `censored=1`, явная отмена после разрыва не учтена. Это задокументировано
  как ограничение, и вердикт честно опирается ТОЛЬКО на gap-free segment 78.

**Противоречие более поздним замерам — НЕ найдено.** `research/data-quality/depth-probe-staleness.md`
(2026-07-22) **предшествует** M-32 и является тем, что M-32 закрывал, а не опровержением.
`docs/06-data-layer-and-storage.md` §2 («объёмы ОПРОВЕРГНУТЫ замером», 2026-07-14) — про объёмы
журнала, к глубине отношения не имеет. TD-016 (корректность/эвикция книги) остаётся **OPEN** и
корректно разведён с M-32 в `TECH-DEBT.md:1246-1256`: M-32 = валидация ИЗМЕРЕНИЯ, M-31 =
корректность ПОДДЕРЖКИ. Замечу отдельно: **milestone M-31 в `main` отсутствует** (`ls milestones/`
не содержит `M-31-*`, есть только коммит `01be192` вне main) — то есть follow-up, на который
M-32/M-33 явно ссылаются как на закрывающий TD-016, в main не приземлён.

**Воспроизводимость.** Источник чисел — прод-segment 78; сегмент на VPS **сохранился**
(`segment-00000078.jrnl.zst`, всего 167 сегментов, ранние сжаты M-40). Пересъёмка с исправленной
метрикой физически возможна — TD-103 действенен, а не теоретичен.

**NOTE-2 (acceptance-скрипт).** 2 из 7 проверок `scripts/verify_M-32.sh` (строки 29-46) — это
grep по ПРОЗЕ документов: `grep -qiE 'CONFIRMED|REFUTED'` и `grep -qiE 'эталон|1\.3%' && ...`.
Такой гейт зеленеет на документе с ПРОТИВОПОЛОЖНЫМ выводом, лишь бы встретились ключевые слова.
Родственно правилу `33aff34` («запрет решать по выводу гейта вместо exit-кода»).

### A.2 M-33 — depth-band 30-60% → **RETRO-NOTE**

**Scope — чисто.** Diff: `depth_lifetime.rs` (+11: расширение `BANDS_BPS` константой + коммент),
sacred-файлы (`red_depth_band_3060.rs`, `verify_M-33.sh`, milestone) — architect, impl — research-dev.
Границы ролей соблюдены.

**RED-first — соблюдён:** `45b44eb` (DV-I-9 RED) → `ccbe441` (impl research-dev) → `def6490`
(вердикт) → merge. Анти-плацебо честное и проверяемое: до расширения `BANDS_BPS` уровень 45%
клампился в `[1500,3000)`, `r.band(Side::Buy, 3000)` возвращал `None` → тест падал.

**NOTE-1 — наследование TD-103.** Весь вердикт M-33 — это ОДНО сравнение `cancel_fraction`
(bid 0.622 / ask 0.366) с соседней полосой. Смещение из A.1 применимо целиком, и здесь оно
сильнее: чем глубже полоса, тем реже цена посещается повторно, тем меньше насыщение метрики —
т.е. **наблюдаемый монотонный спад 0.867→0.796→0.622 частично объясняется падением числа
повторных посещений, а не падением живости**. Вывод «живая-разрежённая» может быть верен, но
приведённое доказательство его не несёт.

**NOTE-2 — пре-регистрированный критерий слеп к асимметрии.** Founder-флаг сформулирован как
«drop >50% от соседней `[1500,3000)` **ПО ОБЕ стороны**» (`depth-verdict.md:21-23`). Это
конъюнкция: односторонняя заморозка (bid живой, ask замёрз — ровно то, что наблюдается, ask
n=41 против bid n=156) флаг НЕ поднимет. `testing.md` требует асимметричный вход в фикстуре;
здесь та же слепота сидит в самом ПРАВИЛЕ ПРИНЯТИЯ РЕШЕНИЯ, что опаснее — фикстуру чинит
следующий оракул, а пре-регистрированный критерий уже отработал и подписан.

**NOTE-3.** n=41 на ask при вынесении вердикта о полосе, которая войдёт в продуктовый контракт.
Caveat разрежённости в вердикте назван честно — поэтому NOTE, а не DEFECT.

### A.3 M-34 — funding-breadth → **RETRO-NOTE**

**RISK-BLOCK — разобран, применён верно.** Diff трогает `crates/venue-binance-futures/src/lib.rs`,
то есть формально `venue-*` (§5 триггер). Проверил MD-only carve-out **по факту, а не по
заявлению**: три правки — `select_funding_emit` (чистая функция без I/O), замена inline-фильтра в
`poll_premium_index`, константа `FUNDING_POLL_PERIOD` 10с→60с. Order-egress (submit/cancel/подпись
торговых действий) в diff'е отсутствует; путь `premiumIndex (REST read) → MdEvent::Funding →
журнал` — read-only. **risk-critic действительно не требовался** (`gates.md` §5, MD-only carve-out
от 2026-07-11). Scope соответствует Allowed paths (`milestones/M-34-funding-breadth.md:50-52`)
буквально, включая перечисленные три точки правки.

**RED-first — соблюдён:** `51543e7` (FB-I-1 RED, architect) → `0d123d5` (impl, venue-dev) → merge.

**NOTE-1 — оракул проверяет извлечённую функцию, но не проводку (класс TD-020).** Все три теста
`red_funding_breadth.rs` вызывают `select_funding_emit` напрямую. Если бы venue-dev реализовал
функцию правильно и **не заменил** inline-фильтр в `poll_premium_index`, вся сюита осталась бы
GREEN, `verify_M-34.sh` — PASS (его 5 проверок: fmt, clippy, FB-I-1, регресс, grep на
`from_secs(60)`; **проверки call-site нет**), и вселенная фандинга продолжала бы выбрасываться.
Это ровно тот класс, который проект уже ловил на M-35 task 2e («коллектор был готов, но не
заспавнен → §8 показал 0 событий»). Здесь дефекта не случилось: проводка сделана и жива на
сегодняшнем `main` — `crates/venue-binance-futures/src/lib.rs:1392`:
`for event in select_funding_emit(events, &subscribed, true) {`. Единственным доказательством
проводки был §8 eyes-on в close-out (`17fc51c`: 781 distinct символов / 779 не-BTC/ETH перпов).

**NOTE-2 — у breadth нет постоянного наблюдателя (TD-104).** Ops-алерт
(`crates/ops/src/alerts.rs:85`) реагирует на «нулевую производную `md_events_total` по kind» —
т.е. на ИСЧЕЗНОВЕНИЕ Funding. Схлопывание широты (781 символ → 2) не даёт нуля и **не поднимет
алерт**; юнит-тест при этом останется GREEN (он не смотрит на call-site). Свойство, ради которого
делался milestone, сегодня не защищено ни тестом, ни алертом — только одноразовым ручным замером
месячной давности.

**NOTE-3 — оценка объёма в merge-сообщении занижена вдвое.** `211e452`/`0d123d5` считают
«~400 перпов ≈ 576k/сутки ≈ 29 MB/сутки»; фактический замер того же дня в close-out — 781 distinct
символов, т.е. ≈1.12M событий/сутки. Числа в коммит-сообщении и коммент в коде (`lib.rs:55-56`)
остались с оценкой ~400. Не блокер (запас по диску есть: heartbeat показывает `free_bytes`
77.2 GB при `min_free_bytes` 10.7 GB), но цифра в комментарии к константе врёт вдвое.

**Прод сегодня (§8 eyes-on, выполнен в рамках этого аудита):**

```
hft-gateway-serve Up About an hour (healthy)
hft-recorder Up About an hour (healthy)
{"events":409431,"free_bytes":77249609728,"min_free_bytes":10737418240,"next_seq":159259982,
 "segment_index":167,"ts_wall_ms":1785788386533,"writable":true}
now=1785788395590  ⇒ heartbeat свежий (лаг ~9с)
```

Живую широту фандинга (distinct-символы за окно) переснять не удалось: метрик-порт наружу не
слушается, в логах recorder'а per-poll записей нет — что и есть NOTE-2.

---

## Часть B — сверка `PROJECT-STATE.md` с фактическим `main`

**Оговорка о методе.** Широкий обход `PROJECT-STATE.md` был делегирован субагенту; он отработал
(43 tool-вызова), но вернул пустой финальный ответ и на повторный запрос результата не ответил
до момента сдачи этого отчёта. Поэтому ниже — **мои собственные замеры**, выборка меньше
задуманной (10 утверждений), но каждое проверено командой, а не чтением. Оставшийся обход —
кандидат на отдельный проход.

| # | утверждение `PROJECT-STATE.md` | вердикт | пруф |
|---|---|---|---|
| PS-1 | :272 «Анти-плацебо в обе стороны в **каждом** оракуле» (DV-I-1..8) | **REFUTED (в части «каждом»)** | ни один оракул не покрывает «отмена → перерождение той же цены»; проба на вербатим-модуле: `born=1 cancelled=1 frozen=0` при живом на конце окна уровне → §A.1, TD-103. Формулировка исправлена в PROJECT-STATE |
| PS-2 | :263-267 Q2 «дальние уровни РЕАЛЬНО отменяются… ⇒ живые, не фантом» (FAR 0.805 vs NEAR 0.981) | **PARTIAL** | метрика меряет долю distinct-ЦЕН с ≥1 `size=0`, а не долю уровней; смещена вверх и насыщается с churn (`depth_lifetime.rs:81-91,171`). Вывод не опровергнут, но не доказан этим числом. Поправка внесена |
| PS-3 | :508-509 FB-I-1 «покрывает множественность/регрессию/отсутствие/порядок (`testing.md` чек-лист)» | **PARTIAL** | верно для чистой функции; call-site не покрыт ни тестом, ни `verify_M-34.sh` → TD-104. Поправка внесена |
| PS-4 | M-34: breadth-путь реально эмитит всю вселенную перпов | **CONFIRMED** | `crates/venue-binance-futures/src/lib.rs:1392` — `for event in select_funding_emit(events, &subscribed, true) {`; `:57` — `FUNDING_POLL_PERIOD = Duration::from_secs(60)`; `verify_M-34.sh` PASS 5/5 exit=0 |
| PS-5 | :1016 recorder спавнит metrics-server, bind loopback-only (`METRICS_BIND_ADDR`, дефолт `127.0.0.1:9101`) | **CONFIRMED замером на проде** | `docker exec hft-recorder cat /proc/net/tcp` → LISTEN `0100007F:238D` = `127.0.0.1:9101`. Наружу не слушается (`ss -lntp` на хосте: только 22/53) — ровно как задекларировано |
| PS-6 | M-48/M-54: снапшот-при-подключении идёт от чекпоинта, писатель — отдельный сервис | **CONFIRMED замером на проде** | `/var/lib/docker/volumes/hft-platform_gateway-ckpt/_data/`: `ckpt-2a00318f774d9689.bin` (1.7 MB) + `covered_through_seq`, mtime `Aug 3 20:30` — свежий на момент аудита. `docker ps -a` показывает только 2 контейнера (recorder, gateway-serve): retention/compaction/checkpoint — one-shot (`--rm`), это соответствует `docker-compose.yml:47,90,186` |
| PS-7 | ретеншен/компакция — операторский путь отдельным бинарём, не в recorder-цикле | **CONFIRMED** | `grep -rn "retention\|compact" crates/recorder/src/` → пусто; бинарь `crates/journal/src/bin/journal-retention.rs`; `docker-compose.yml:53,93` — `entrypoint: ["/usr/local/bin/journal-retention"]` в двух отдельных сервисах |
| PS-8 | шардинг журнала — НЕ заявлен как реализованный | **CONFIRMED (нет оверклейма)** | `grep -nE "шардинг\|sharding" PROJECT-STATE.md` → пусто; `grep -rln "shard" crates/journal/src/` → пусто; тема живёт как план (`docs/plans/journal-sharding-facts.md`) — документ и код согласованы |
| PS-9 | ops-алерт ловит пропажу потока событий | **CONFIRMED, но уже, чем нужно** | `crates/ops/src/alerts.rs:85` — «`md_events_total` — нулевая производная по kind при живом WS (Funding/Trade пропали)». Ловит НОЛЬ, не схлопывание широты 781→2 → TD-104 |
| PS-10 | M-32/M-33 ссылаются на **M-31** как на закрывающий TD-016 follow-up (`PROJECT-STATE:291`, `depth-verdict.md:118`) | **REFUTED (артефакт не приземлён)** | `ls milestones/ \| grep M-31` → пусто; коммит `01be192 docs+test(M-31): book eviction (TD-016)` в `origin/main` не входит. Follow-up, на который опираются оба вердикта, в main отсутствует; TD-016 остаётся OPEN без носителя-milestone'а |

**Контекст, важный для чтения таблицы.** Крейтов `risk`/`killswitch`/`oms` в репозитории **нет**
(`ls crates/` → alpha, book, contracts, derive, gateway, gateway-serve, journal, ops, portfolio,
recorder, research-cli, signals, sim, strategy, venue-binance, venue-binance-futures,
venue-hyperliquid). Ордер-пути не существует. Это прямо влияет на оценку тяжести Части A: три
пропущенных гейта пришлись на стадию, где ошибка не могла стоить денег — **повезло по стадии, а
не сработал процесс**.

### M-16 и M-21 — почему их нет в PROJECT-STATE

| | M-16 `historical-import` | M-21 `journal-hardening` |
|---|---|---|
| файл в main | `milestones/M-16-historical-import.md` есть | `milestones/M-21-journal-hardening.md` есть |
| STATUS в файле | `PROPOSED` (2026-07-20) | `PROPOSED / QUEUED` (2026-07-21), «НЕ стартует без явного `go` founder'а» |
| verify-скрипт | нет | нет |
| impl-коммиты | нет (`git log --all --grep=M-16` → только 3 docs-коммита architect'а: `cb3610f`, `0ee29de`, `556a1e6`) | нет (`git log --all --grep=M-21` → один docs-коммит `0cd447d`) |
| код в main | нет: `grep -rniE "binance.vision\|hyperliquid-archive\|historical.import" crates/` → пусто | нет отдельного носителя |

**Вердикт: запись в `PROJECT-STATE.md` НЕ нужна, пропуск корректен.** `PROJECT-STATE.md` по
определению («что реализовано») содержит выполненное; оба milestone'а никогда не исполнялись и
остались планами — тот же класс, что M-42/43/44. Ноль упоминаний — не дыра учёта, а верное
отражение состояния.

Оговорка, которую стоит держать в голове: **отсутствие в PROJECT-STATE делает такой milestone
невидимым** — забытый план и невыполненный план выглядят одинаково. Для M-16/M-21 это не
критично (оба живут файлами в `milestones/` и в BACKLOG), но контраст с PS-10 показателен: M-31
подобного носителя в main лишён и при этом на него ссылаются два подписанных вердикта.

---

## Часть C — процессный вывод

**Вопрос founder'а:** что в workflow позволило трём milestone'ам уехать в прод без гейта и что
мешает этому повториться сегодня?

### C.1 Гипотеза «правило появилось позже» — ОПРОВЕРГНУТА

`gates.md` §4 «PR-time (reviewer) — UNCONDITIONAL» существует **с первого коммита процессного
слоя**: `f06c901` «M-00: process layer» от 2026-07-10, то есть за две недели до M-32.
Проверено на дереве самого merge'а:

```
$ git show bb00915:.claude/rules/gates.md | grep -n UNCONDITIONAL
20:reviewer — бэкстоп на PR-time (гейт 4 ниже всё равно UNCONDITIONAL).
65:## 4. PR-time (reviewer) — UNCONDITIONAL

$ git show bb00915:.claude/rules/commit-discipline.md | grep -n "Reviewer (не dev"
81:4. Reviewer (не dev, не architect) — последний в цепочке для substantive-изменений;
```

Оба правила действовали на момент нарушения. **Дыра была не в тексте правил.**

### C.2 Чего действительно не было — вердикт reviewer'а не был АРТЕФАКТОМ

До 2026-07-31 вердикт reviewer'а по правилам жил в переписке. Для critic/risk-critic артефактность
была закреплена раньше (`branch-hygiene.md` §3), для reviewer'а — нет. Закрыто коммитом `5439b1c`
(2026-07-31, после трёх REJECT'ов на M-49): «Вердикт reviewer'а — АРТЕФАКТ, а не сообщение…
обязан записать вердикт в `research/reviews/R-NNN.md` и закоммитить+запушить его на ветку
milestone'а ДО завершения работы».

Последствие для этого аудита существенно: **до 5439b1c пропуск PR-гейта был принципиально
ненаблюдаем**. Единственным следом гейта было merge-сообщение, которое пишет тот же, кто мержит.
Сегодня отсутствие R-NNN — это дырка, находимая одним `git log`; именно так founder и вышел на
эти три milestone'а.

### C.3 Настоящий корень: git-личность — единственный различитель, и её никто не проверяет

Личность коммиттера ставится один раз при бутстрапе worktree и не переустанавливается при смене
роли внутри цепочки. Поэтому reviewer-сессия, поднятая в worktree предыдущей роли, коммитит как
`engine-dev`/`tester` — и в логе неотличима от dev'а, смерджившего себя. Это не гипотеза:
close-out-коммиты `c9a8f64`/`e2cd735`/`17fc51c`/`41c328f`/`e8885b9` правят
`PROJECT-STATE.md`/`TECH-DEBT.md` — зону, куда по `scope-guard.md` пишет только reviewer, — но
подписаны `engine-dev`/`tester`. Один из двух фактов ложен, и **какой именно — по репозиторию не
определяется**.

Автоматической защиты нет и не появилось: branch protection на GitHub недоступна на текущем плане
(`gh api .../branches/main/protection` → HTTP 403 «Upgrade to GitHub Pro or make this repository
public»), в `.github/workflows/{ci,deploy}.yml` проверок автора/наличия артефакта гейта нет,
хуков в `.claude/` нет. Дисциплина держится **только** на инструкции агенту.

### C.4 Что мешает повторению СЕГОДНЯ (и это работает)

1. `5439b1c` (2026-07-31) — вердикт reviewer'а обязан быть файлом `research/reviews/R-NNN.md`.
   Пропуск гейта стал **наблюдаемым постфактум**.
2. `69bb728` + `26b16bd` (RN-19, 2026-07-31) — приземление работы субагента проверяет
   оркестратор **фактами git**, не рапортом.
3. `d6ff609` (2026-08-01) — документ класса A проверяется на дереве слияния, а не на ветке.
4. `33aff34` (2026-08-03) — запрет решать по выводу гейта вместо exit-кода.
5. Эмпирика (замерено, а не на глаз): **45 merge'ей подряд с вершины `main` сделаны `reviewer`
   или `architect`** (architect — только на docs/process-ветках, что разрешено
   `commit-discipline.md` п.5); первый merge ниже этой полосы — `engine-dev` (`545a41b`,
   M-35 survey, 2026-07-25). Из них 12 самых верхних — подряд `reviewer`.
   `research/reviews/` содержит R-001..R-030 непрерывно с M-49.

### C.5 Находка уровня правила (остаётся ОТКРЫТОЙ) — TD-105

Ни одно из перечисленного не мешает merge'у в `main` состояться. Все четыре механизма —
**обнаруживающие**, ни одного **предотвращающего**; все четыре срабатывают, только если
кто-то потом смотрит. Merge-коммит с правдоподобным сообщением и без R-NNN сегодня уезжает
ровно так же тихо, как 2026-07-24.

Дешёвая и механическая проверка напрашивается: для каждого merge-коммита в `main` с сообщением
вида `merge(M-NN)` требовать (а) автора `reviewer` и (б) наличие в дереве слияния файла
`research/reviews/R-*.md`, называющего этот milestone. Это уже не «доверять роли», а
«предъявить артефакт». Дизайн (где именно — CI-job, pre-push hook или проверка внутри
`verify_*`), формулировку правила и RED-оракул к нему проектирует **architect** — граница
reviewer↔architect (`gates.md` §4): я описываю дыру, фикс не проектирую.

Второй, ортогональный пункт того же долга: **git-личность роли должна переустанавливаться при
смене роли в цепочке**, иначе аудит-трейл продолжит врать, и следующий такой аудит снова упрётся
в «развести невозможно».

### C.6 Почему это не стоило денег — и почему это не аргумент

Три пропущенных гейта пришлись на milestone'ы вне пути к деньгам: `research-cli` (оффлайн-анализ)
и MD-only venue-чтение. Ордер-пути в репозитории **не существует вообще** — крейтов
`risk`/`killswitch`/`oms` нет (`ls crates/`). RISK-BLOCK (`gates.md` §5) на этой стадии
**вакуумен**: его нечему блокировать, и он ни разу не был тем, что удержало ошибку.

Отсюда честный вывод: **сработала стадия, а не процесс**. Ровно тот же сбой (reviewer-сессия под
чужой личностью + merge без предъявленного артефакта) на milestone'е с ордер-egress прошёл бы
теми же путями, но цена была бы депозитом. Единственный слой, который в этом случае держит —
risk-critic, и он вызывается тем же диспетчером и подтверждается тем же способом (файл в
`research/critiques/`), который в июле для reviewer'а отсутствовал. Поэтому TD-105 стоит закрыть
**до** появления первого крейта ордер-пути, а не после.

---

## Заведённые TD

| TD | суть | владелец следующего шага |
|---|---|---|
| **TD-103** | `depth_lifetime`: `cancel_fraction` слепа к перерождению цены ⇒ смещение вверх; вывод M-32/M-33 и founder-решение стоят на смещённой метрике; фикстуры «отмена→перерождение» нет ни в одном оракуле | architect (переспека метрики + RED) |
| **TD-104** | funding-breadth не наблюдаем: оракул смотрит чистую функцию, не call-site; ops-алерт ловит ноль, а не схлопывание широты | architect |
| **TD-105** | пропуск PR-гейта обнаруживаем, но не предотвратим: нет проверки «merge в main ⇒ автор reviewer + артефакт R-NNN»; git-личность не переустанавливается при смене роли | architect |

---

## Done Block

```
$ cd /tmp/hft-rev-retro && git log --oneline -1
b5d82f2 merge(docs): архитектура масштаба read-path — DESIGN §16.1-16.3, роадмап Ф2, П-009/П-010, уроки сессии (R-030)

$ git diff --stat f5907e2 b5d82f2 -- crates/ scripts/
 scripts/measure_M-54_connect.sh | 15 +++++++++++++++
 1 file changed, 15 insertions(+)        # код-часть базы аудита не менялась

$ cargo test -p research-cli --test red_depth_lifetime --test red_depth_scale \
      --test red_orderflow_faith --test red_depth_band_3060 2>&1 | grep "^test result"
test result: ok. 2 passed; 0 failed  (red_depth_band_3060)
test result: ok. 6 passed; 0 failed  (red_depth_lifetime)
test result: ok. 2 passed; 0 failed  (red_depth_scale)
test result: ok. 4 passed; 0 failed  (red_orderflow_faith)
exit=0

$ bash scripts/verify_M-32.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
PASS: fmt clean
PASS: clippy research-cli 0 warnings
PASS: DV-I-1..5 (red_depth_lifetime) GREEN
PASS: DV-I-6 (red_orderflow_faith) GREEN
PASS: DV-I-7/8 (red_depth_scale) bounded — single-pass O(n)
PASS: Q1 memo: паритет CONFIRMED/REFUTED
PASS: вердикт называет 3 решения (эталон / достоверность / provenance)
VERDICT: PASS
exit=0

$ bash scripts/verify_M-33.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
PASS: fmt clean
PASS: clippy research-cli 0 warnings
PASS: DV-I-9 (red_depth_band_3060) GREEN
PASS: DV-I-1..6 регресс-GREEN
PASS: DV-I-7/8 bounded регресс-GREEN
PASS: memo содержит полосу 30–60%
VERDICT: PASS
exit=0

$ bash scripts/verify_M-34.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
PASS: fmt clean
PASS: clippy venue-binance-futures 0 warnings
PASS: FB-I-1 (red_funding_breadth) GREEN
PASS: venue funding/parse регресс-GREEN
PASS: FUNDING_POLL_PERIOD = 60с (даунсэмпл ~1/мин)
VERDICT: PASS
exit=0

$ ./probe   # модуль depth_lifetime вербатим из main, подменены только типы Level/Side
ПРОБА-1 (cancel->rebirth, уровень ЖИВ в конце окна):
  полоса [500,800) bid: born=1 cancelled=1 frozen=0 censored=0 cancel_fraction=Some(1.0)
ПРОБА-2 (после gap те же цены приходят снова): gaps=3
  полоса [0,150) bid: born=1 cancelled=0 frozen=0 censored=1 cancel_fraction=None
  полоса [500,800) bid: born=1 cancelled=0 frozen=0 censored=1 cancel_fraction=None

$ ssh root@167.233.192.131 'docker ps --format "{{.Names}} {{.Status}}"; cat .../recorder.heartbeat'
hft-gateway-serve Up About an hour (healthy)
hft-recorder Up About an hour (healthy)
{"events":409431,"free_bytes":77249609728,"min_free_bytes":10737418240,"next_seq":159259982,
 "segment_index":167,"ts_wall_ms":1785788386533,"writable":true}
exit=0

$ gh api repos/:owner/:repo/branches/main/protection
{"message":"Upgrade to GitHub Pro or make this repository public to enable this feature.","status":"403"}
```
