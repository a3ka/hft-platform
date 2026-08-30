<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: c82253b337687462d0d81168c83b0117600bdda4
audited_head: b9d05d1905eeeb916ab8c0cf9bc53a8ecfe4a975
verdict: REJECT
-->

# R-138 — M-68 круг 2 impl (задачи 12,14-19): PR-time reviewer, **REJECTED**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-27
**Предмет:** `c82253b..b9d05d1` на `origin/feat/M-68-rev4` — восемь коммитов engine-dev
(семь impl в `crates/gateway/src/lib.rs` + один статус-колонка §Tasks).
**Дерево ревью:** `/tmp/hft-reviewer-M-68-r9`, detached на `b9d05d1`, чистый чекаут из origin.
**Мандат сверх штатных блоков:** `A-025` §5.3 (а) перепроверить команды §5.1; (б) завести три
карточки долга; (в) **свежим взглядом судить, не реализована ли «первое событие интервала»
под зелёным `d12`** — единственный контур close-семантики, пока долг №1 открыт.

**Прочитано на этой ревизии:** `milestones/M-68-depth-from-book.md` (889 строк, §0sexies целиком,
§3/§3.1 Allowed/Forbidden, §4 Tasks), `A-025` целиком, `A-024` §5/§8, `C-170`, `R-134` (шапка и
находки B-1..B-4/N-1), `docs/fa/viz-backend.md` (§5 таблица инвариантов), `red_depth_cadence.rs`
(1-200 + `d12` целиком), `crates/gateway/src/lib.rs` в тронутых местах. Ярус C — **грепом по
предмету**, не целиком: `TECH-DEBT.md` по `TD-158|TD-161|TD-167|M-68` (:92, :94, :100, :778,
:826, :829, :1149-1187), `PROJECT-STATE.md` по `M-68|depth_cadence` — **совпадений ноль**.

**Предъявление FA (M-66).** Диф трогает `crates/gateway/src/**` ⇒ живые инварианты названы
прямым чтением `docs/fa/viz-backend.md` на этой ревизии: **`VB-I-1`** (`:188` — «каждый
индикатор — чистый редьюсер над `journal::stream`; детерминизм-тест обязателен») и
**`VB-I-2`** (`:189` — «live == replay»). Оба held: каденция ведётся ВРЕМЕНЕМ СОБЫТИЯ
(`lib.rs:1162-1169`, не wall-clock), шаг `F` гейта (`red_gateway_live_eq_replay`) зелен.
`FA-WAIVER` не требуется.

---

## Block-scope — ЧИСТО

| коммит | пути | вердикт |
|---|---|---|
| `eb46cdf` `2f7dd7f` `5e8f574` `6f48fb3` `d3919fc` `32824cd` `21837d8` | `crates/gateway/src/lib.rs` | ✅ в `Allowed paths` engine-dev (§3: `crates/gateway/src/**`) |
| `b9d05d1` | `milestones/M-68-depth-from-book.md` | ✅ carve-out `scope-guard.md` — правка ТОЛЬКО колонки Status (диф проверен: семь строк, изменена одна ячейка в каждой) |

`crates/contracts/**` не тронут (шаг `H` гейта), `crates/book|venue-*|journal` не тронуты (шаг
`K`), `GATEWAY_BANDS` не тронут (шаг `I`), `selector_fingerprint` не переписан, а ДОПОЛНЕН одной
строкой (шаг `J`; `git diff` :2651-2660 — добавление `sel.depth_cadence_ms.hash(&mut h)`, что и
предписано задачей 18 как ЯВНАЯ инвалидация, а не подгонка). Тесты dev не трогал ни в одном
коммите — RED-first цел, sacred-зона не задета.

**RISK-BLOCK не применяется** и это проверено, а не предположено: диапазон не трогает
`crates/risk|killswitch|oms|venue-*|contracts`; предмет — Слой 8, read-only консюмер журнала
(`VB-I-3`), order-egress отсутствует. risk-critic по `gates.md` §5 не требуется.

## Block-commits — ЧИСТО

`R-134` B-1 (бандл на восемь задач) исполнен **вперёд**, как и предписал `A-024` §5: семь задач
— семь коммитов, каждый называет номер задачи и оракул. `Co-Authored-By` ни в одном теле нет
(`git log --format=%B | grep -c Co-Authored-By` → 0). Переписи истории не было и не требовалось.

## Block-DoneBlock — ВОСПРОИЗВЕДЁН СВОИМ ПРОГОНОМ

Done Block tester'а не пересказан, а перепрогнан в собственном дереве — см. Done Block ниже.
`VERDICT: PASS`, `exit=0`, 25/25 шагов зелены; совпадает с предъявленным.

## Block-A025 §5.3(а) — команды norm-правки перепроверены

| команда `A-025` §5.1 | заявлено | мой прогон |
|---|---|---|
| `grep -n 'расширены на ОБА' milestones/M-68-*.md` | `exit=1` | **`exit=1`** ✅ |
| диф правки — ровно один файл | один | **`milestones/M-68-depth-from-book.md`, один** ✅ |
| базовая линия на `c82253b` | `FAIL (7)`, `exit=1` | **`VERDICT: FAIL (7)`, `verify_exit=1`**, поимённо тот же состав ✅ |

Дельта `FAIL(7) → PASS` целиком объясняется работой dev'а. Предписание §5.1 исполнено честно.

---

## B-1 (БЛОКЕР) — реализована «точка в НАЧАЛЕ интервала», а спека пиннит CLOSE

**Это ровно то, что `A-025` §5.3(в) поручил reviewer'у искать, и оно найдено ЗАМЕРОМ.**

`crates/gateway/src/lib.rs:1162-1169`:

```rust
if let Some(ms) = self.selector.depth_cadence_ms {
    let cadence_s = ms / 1000;
    if time_s % cadence_s != 0 { return; }        // ← событие ПРОПУСКАЕТСЯ целиком
}
…
row.values.insert(time_s, sum);                    // ключ = time_s пропущенного через фильтр события
```

Пересчёт происходит ТОЛЬКО в первом timeframe-бакете каждого каденс-интервала; события
секунд `t0+1 … t0+cadence−1` не читаются вовсе. Значение точки с ключом `t0` есть состояние
книги на КОНЕЦ ПЕРВОЙ СЕКУНДЫ интервала, а не на конец интервала.

**Замер (проба reviewer'а поверх фикстуры `journal()` из `red_depth_cadence.rs`, но с
варьирующим SIZE — `5.0 + i*0.01`, форма `A-025` §5.4 П-1а; проба прогнана в дереве ревью и
удалена, `git status --porcelain` пуст):**

```
R138-PROBE cadence=1000  points=120 first3=[(…000,500000000),(…001,501000000),(…002,502000000)]
R138-PROBE cadence=10000 points=12  first3=[(…000,500000000),(…010,510000000),(…020,520000000)]
R138-PROBE cadence=60000 points=2   first3=[(…000,500000000),(…060,560000000)]
R138-EXPECT i=0 -> 500000000   i=59 -> 559000000   i=60 -> 560000000   i=119 -> 619000000
```

Точка каденции 60 с на `t0` несёт **500000000** — значение события `i=0`, ПЕРВОГО в интервале.
CLOSE дал бы **559000000** (событие `i=59`). Для каденции 10 с точка на `t0+10` несёт
`510000000` (событие `i=10`), close дал бы `519000000` (`i=19`).

**Норма, которая нарушена, — не моё прочтение, а пиннинг спеки.** §0sexies.2bis: «В интервале
каденции побеждает **последнее** наблюдение… Альтернатива „первое событие интервала"
**отвергнута**: она рассогласовала бы депт-серию с heatmap, у которого close уже действует, —
то есть вернула бы РАЗНУЮ семантику двум сериям одного окна, ради устранения которой милестоун
и существует». `A-025` §5.2 подтверждает: close «обязательна ТЕКСТОМ §0sexies.2bis и
проверяется reviewer'ом, пока долг №1 открыт».

**Дефект проявляется ТОЛЬКО при `cadence > timeframe`** — при дефолтных 1000/1000 `cadence_s=1`
и фильтр пропускает всё, close-по-бакету цел. Но именно `cadence > timeframe` и есть
затребованный founder'ом режим: «для исторического анализа даже раз в минуту».

### B-1bis — почему это НЕ вина dev'а, и почему я всё равно не мержу

Committed-оракул `d12` в своём ассерте значения (`red_depth_cadence.rs:164-194`) сверяет точку
грубой серии с `fine_pts.filter(|(ft,_)| *ft <= *t).last()` — то есть с последним тонким
наблюдением, чей ключ `≤` ключу грубой точки. При ключе-НАЧАЛЕ интервала это буквально
значение ПЕРВОГО события интервала. **Оракул требует того, что спека запрещает.** `A-025` §1
назвал этот третий дефект прямо («форма `ft <= t` … ассерт потребовал бы запрещённой
семантики») и отложил его в долг №1 вместе с вакуумностью.

Итог: dev исполнил единственную ИСПОЛНИМУЮ спецификацию (зелёный sacred-оракул), а нормативный
ТЕКСТ требует обратного; исправить импл, не покраснев `d12`, dev не может — `d12` sacred
(architect-only). Развилка выше и dev'а, и reviewer'а:

1. **Закрыть долг №1 сейчас:** architect перестраивает `d12` по `A-025` §5.4 П-1 (фикстура с
   варьирующим size, setup-guard, ключ точки запиннен спекой, сравнение ПО СТРОКАМ), dev
   реализует close. Цена — один круг architect+dev.
2. **Признать «съём в начале интервала» действующей семантикой:** тогда правится §0sexies.2bis
   (снятие пиннинга close), и это решение о СЕМАНТИКЕ ПРОДУКТА для аналитической серии —
   не моё и не архитектора.

`A-025` §5.5 запрещает пятый арбитраж и отправляет класс «набор недостижим GREEN в разрешённой
зоне» **founder'у**. Я вердиктом эту развилку не решаю — я её предъявляю. Reviewer описывает
дефект и не проектирует фикс (`gates.md` §4, граница reviewer↔architect).

## B-2 (БЛОКЕР-кандидат / MAJOR) — эффективная каденция молча становится НОК, а не заявленной

`time_s` уже выровнен на `timeframe_ms` (`bucket_time_s` `:834-841`), поэтому фильтр
`time_s % cadence_s` даёт точки не с шагом каденции, а с шагом `lcm(timeframe_s, cadence_s)`.
`validate_selector` проверяет делимость каждого параметра на сутки, но НЕ проверяет их
отношение друг к другу. Замер той же пробы:

```
R138-LCM timeframe=3000 cadence=10000 -> keys=[…000, …030, …060, …090]
```

Оба значения валидны по отдельности (`86400000 % 3000 = 0`, `86400000 % 10000 = 0`), а
фактическая каденция — **30 с вместо 10 с, без единого сообщения**. Это ТОТ ЖЕ класс тихой
деградации (`GW-I-14`), ради закрытия которого существует задача 17, — воспроизведённый
соседней арифметикой в том же коммите. Выдача при этом объявляет `depth_series = 10000`
(`:1347` берёт значение СЕЛЕКТОРА, не эффективное), то есть метка задачи 16 в этом случае
**лжёт** — прямое попадание в `П-014` п.2 («выдача обязана ЭТО НАЗЫВАТЬ, а не умалчивать»).

## B-3 (MAJOR, built-not-wired) — каденция недостижима на несущем пути

`depth_cadence_ms` жёстко `None` во ВСЕХ трёх прод-инициализаторах, и ни одна env-переменная
её не читает:

```
crates/gateway-serve/src/lib.rs:1786   build_selector(...) → depth_cadence_ms: None   ← прод-транспорт
crates/gateway/src/bin/gateway-checkpoint.rs:247                depth_cadence_ms: None
crates/gateway/src/lib.rs:34           default_selector()      → depth_cadence_ms: None
$ grep -oE 'GATEWAY_[A-Z_]+' crates/gateway-serve/src/lib.rs | sort -u   → GATEWAY_DEPTH_CADENCE_MS ОТСУТСТВУЕТ
$ grep -n 'GATEWAY_' docker-compose.yml                                   → переменной нет
```

Задача 15 требует «дефолт **1000 мс**, значение **настраивается**». Фактически: дефолт `None`,
настроить нельзя ничем. Следствия, обе — не теория:

1. механизм каденции на проде **не исполняется никогда** (что, к слову, обнуляет прод-цену
   B-1 и B-2 сегодня — но только до момента проводки);
2. кадр на проде объявляет `cadence_ms = [("depth_series", None), ("heatmap", None)]` —
   то есть **не различает две серии**, ради чего задача 16 и вводилась. `d13` зелен потому,
   что тест задаёт каденцию явно; на прод-пути метка инертна.

`gates.md` §4 (DoD «Механизм на пути») даёт ровно два законных исхода: подключение, доказанное
оракулом точки входа, ЛИБО merge с TD-записью **built-not-wired severity MAJOR**. Проводка
env→`build_selector` лежит в `crates/gateway-serve/src/**`, что для engine-dev на этом
милестоуне **вне Allowed paths** (§3 даёт ему только `crates/gateway/src/**`) — то есть dev и
здесь не мог исполнить требование, не совершив SCOPE VIOLATION. Дефект строки спеки, не dev'а.

## N-1 (NOTE) — статус-колонка §Tasks лжёт о половине милестоуна

После `b9d05d1` задачи **1-10 и 13** остались `⏳ OPEN`, хотя реализованы `44d6aac` и зелены
гейтом (шаги `A`/`C2`: `d1`-`d8b`, `d9`). Милестоун с зелёным acceptance утверждает о себе, что
одиннадцать его задач не сделаны. Это ровно класс `TD-167` (самосогласованность артефактов
милестоуна), шестикратно сработавший на M-68. Не блокер сам по себе; чинится колонкой.

## Что я проверил и НЕ нашёл дефекта — названо явно

- **Задача 14** (`d3919fc`): дельта `depth_after − depth_before` вокруг цикла `pump` —
  корректный приём per-call метрики на персистентном аккумуляторе; складываемость `ReadStats`
  (`impl Add`) восстановлена. `R-134` B-4 закрыт по существу.
- **Задача 12 и 19** (`32824cd`, `21837d8`): три ложных самоописания сняты; новый текст
  `:1120-1134` прямо признаёт два собственных вызова `self.book.levels` и называет прежнее
  «zero дополнительных аллокаций» ложным. Шаг `C4` гейта (fail-closed, четыре пары) зелен.
- **Задача 18** (`2f7dd7f`): отпечаток ДОПОЛНЕН полем — явная инвалидация (`C-094` B3), не
  подгонка; §3.1 не нарушен.
- **Задача 17** (`eb46cdf`): отказ подсекундной каденции реализован fail-closed, сообщение
  называет причину и класс. Timeframe-половина не покрыта — это уже вынесенный долг №3, не
  находка круга.
- **`VB-I-2`/`VB-I-10`**: шаги `E`/`F`/`red_gateway_bounded`/`red_snapshot_noclone` зелены —
  фикс не куплен ценой соседнего инварианта.

---

## ВЕРДИКТ: **REJECTED**

Блокер один по существу — **B-1**: доставленная семантика прямо противоположна той, что спека
пиннит, и это установлено замером, а не чтением. **B-2** и **B-3** сопровождают его как MAJOR.
Ни один из трёх не является виной engine-dev'а: B-1 требует sacred-оракула, B-3 — чужой зоны,
B-2 — арифметики, которую спека не запинила. Поэтому **маршрут — не `SVR-response` к dev'у, а
диспетч founder'а** по `A-025` §5.5 (пятый арбитраж запрещён тем же пунктом).

**Условие APPROVED:** (1) close-семантика приведена в соответствие с §0sexies.2bis ЛИБО
§0sexies.2bis правится решением founder'а; (2) B-2 — либо гвард `cadence_ms % timeframe_ms == 0`
в `validate_selector`, либо эффективная каденция в метке выдачи; (3) B-3 — проводка env ЛИБО
TD-карточка «built-not-wired» MAJOR по `gates.md` §4; (4) N-1 — колонка приведена к правде.

**Три карточки долга `A-025` §6 НЕ заведены этим кругом** — намеренно: `TECH-DEBT.md`
обновляется reviewer'ом в close-out после merge'а, merge'а не было. Тексты карточек лежат в
`A-025` §6 и не потеряны; к ним добавляется четвёртая (B-3, built-not-wired) и уточнение
карточки №1: «реализация «первое событие интервала» **проходит набор зелёной**» перестало быть
риском и стало **замеренным фактом** (`R-138` B-1).

---

## Done Block (сырой stdout; `/tmp/hft-reviewer-M-68-r9`, detached `b9d05d1`)

```
$ pwd; git rev-parse HEAD; git status --porcelain
/tmp/hft-reviewer-M-68-r9
b9d05d1905eeeb916ab8c0cf9bc53a8ecfe4a975
{пусто}

$ bash scripts/verify_M-68.sh; echo "verify_exit=$?"
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
PASS: cargo test -p gateway --test red_depth_from_book --quiet
PASS: A состав набора — 9 оракулов (ожидалось ровно 9: d1 d2 d3 d4 d5 d7 d7b d8 d8b)
PASS: B набор КРАСЕН против мутанта C-M68-1 (мутация внесена и прогнана в копии)
PASS: cargo test -p gateway --test red_depth_recompute_cost --quiet
PASS: cargo test -p gateway --test red_depth_semantics --quiet
PASS: C2 состав набора — 3 оракулов (ожидалось ровно 3: d9 d9-C d10)
PASS: cargo test -p gateway --test red_depth_cadence --quiet
PASS: C3 состав набора — 5 оракулов (ожидалось ровно 5: d12 d13 d14 d15 d16)
PASS: C4 самоописание согласовано (обещаний=0, собственных материализаций=2)
PASS: C4 ложное самоописание снято — снятая snapshot-only семантика поля depth_reach_bid (lib.rs:636-658)
PASS: C4 ложное самоописание снято — то же, вторая половина того же комментария
PASS: C4 ложное самоописание снято — ложное «как прежний depth_within с None mid» (lib.rs:1134-1136)
PASS: D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)
PASS: cargo test -p gateway --test red_gateway_schema_version --quiet
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
PASS: cargo test -p gateway --test red_depth_provenance_by_reach --quiet
PASS: H crates/contracts не тронут
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут
PASS: J selector_fingerprint не переписан
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: PASS
verify_exit=0

$ # БАЗОВАЯ ЛИНИЯ — отдельное дерево /tmp/hft-rev-M68-base, detached c82253b (A-025 §5.1)
$ bash scripts/verify_M-68.sh; echo "verify_exit=$?"
FAIL: cargo test --all --quiet
FAIL: cargo test -p gateway --test red_depth_semantics --quiet
FAIL: cargo test -p gateway --test red_depth_cadence --quiet
FAIL: C4 комментарий обещает переиспользование уровней heatmap (1 упом.), а recompute_depth_from_book материализует книгу сам (2 вызовов self.book.levels)
FAIL: C4 ложное самоописание ЖИВО (1 упом.) — снятая snapshot-only семантика поля depth_reach_bid (lib.rs:636-658)
FAIL: C4 ложное самоописание ЖИВО (1 упом.) — то же, вторая половина того же комментария
FAIL: C4 ложное самоописание ЖИВО (1 упом.) — ложное «как прежний depth_within с None mid» (lib.rs:1134-1136)
VERDICT: FAIL (7)
verify_exit=1

$ grep -n 'расширены на ОБА' milestones/M-68-depth-from-book.md; echo "grep_exit=$?"
grep_exit=1
$ git show --name-only --format='' c82253b
milestones/M-68-depth-from-book.md

$ # ПРОБА B-1/B-2 (временный файл, прогнан и УДАЛЁН; дерево чистое)
$ cargo test -p gateway --test zz_reviewer_probe_r138 -- --nocapture
R138-PROBE cadence=1000 points=120 first3=[(1752000000, 500000000), (1752000001, 501000000), (1752000002, 502000000)] last=Some((1752000119, 619000000))
R138-PROBE cadence=10000 points=12 first3=[(1752000000, 500000000), (1752000010, 510000000), (1752000020, 520000000)] last=Some((1752000110, 610000000))
R138-PROBE cadence=60000 points=2 first3=[(1752000000, 500000000), (1752000060, 560000000)] last=Some((1752000060, 560000000))
R138-EXPECT i=0 -> 500000000   i=59 -> 559000000   i=60 -> 560000000   i=119 -> 619000000
R138-LCM timeframe=3000 cadence=10000 -> keys=[1752000000, 1752000030, 1752000060, 1752000090]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

$ git log --format='%h %s' c82253b..b9d05d1
b9d05d1 docs(M-68): статус-колонка §Tasks обновлена [engine-dev]
21837d8 feat(M-68): task #19 — три ложных самоописания кода о себе приведены к правде [engine-dev]
32824cd feat(M-68): task #12 — комментарий recompute_depth_from_book о materialisation приведён к правде [engine-dev]
d3919fc feat(M-68): task #14 — ReadStats::depth_levels_visited per-call на pump-пути (d10, R-134 B-4) [engine-dev]
6f48fb3 feat(M-68): task #16 — выдача называет каденцию depth_series и heatmap (d13) [engine-dev]
5e8f574 feat(M-68): task #15 — каденция депт-серии на границе события (d12) [engine-dev]
2f7dd7f feat(M-68): task #18 — selector_fingerprint различает depth_cadence_ms (d15, C-167) [engine-dev]
eb46cdf feat(M-68): task #17 — depth_cadence_ms sub-second refused in validate_selector (d14, C-167) [engine-dev]

$ git log --format='%B' c82253b..b9d05d1 | grep -c 'Co-Authored-By'
0
```

## Cross-references

- `A-025` §5.1/§5.3/§5.5/§6 (маршрут, мандат reviewer'а, тексты долгов, жёсткий предел)
- `A-024` §5 (бандл исполняется вперёд), `C-170`, `R-134` (B-1..B-4, N-1)
- `milestones/M-68-depth-from-book.md` §0sexies.2bis (close), §3/§3.1, §4
- `gates.md` §4 (PR-time, DoD «Механизм на пути», граница reviewer↔architect), §5 (RISK-BLOCK — н/п)
- `docs/fa/viz-backend.md:188-189` (`VB-I-1`, `VB-I-2`)
