<!-- GATE-META
milestone: M-84
audited_repo: a3ka/hft-platform
audited_base: 4885e6defffadb3e88eb53ad474ebf149b6759ff
audited_head: 10ac14ae0f2993e42e81b2fa9c4e69bd5f1834fa
verdict: DECISION
-->

# A-033 — арбитраж `M-84` после трёх кругов критика (`C-220` → `C-221` → `C-222`)

**Роль:** арбитр по `gates.md` §0, триггер 2 (три круга по одному предмету). Контекст свежий,
рамку ни одной из сторон не наследую. Код не пишу, оракулы не пишу, предмет не правлю:
единственный файл этого круга — этот вердикт.

**Предмет:** ветка `feat/M-84-fixed-bands`, HEAD `10ac14ae0f2993e42e81b2fa9c4e69bd5f1834fa`
(«C-222 F1 — ложное утверждение о цене такта снято»), база — вердикт `C-221`
(`4885e6de…`). Диапазон снят командой (§0).

**Вопрос мандата — ровно один:** годен ли набор к выдаче engine-dev'у, и если нет — что
доделать. Спора о факте `C-222` F1 нет: architect его признал и исправил в `10ac14a`.

---

## РЕШЕНИЕ: **НЕ ГОДЕН** — набор невыполним против собственного корпуса и против прода; правкой текста не лечится

Три круга проверяли ШЕСТЬ новых наборов (29 тестов) и нашли в них всё, что там было. Никто —
ни architect, ни три круга критика — не прогнал против «честного кандидата» ОСТАЛЬНОЙ корпус
`gateway`/`gateway-serve` и не запустил прод-форму argv. Я прогнал (§3). Результат:

- честный кандидат, собранный РОВНО по спеке (константа + точный гвард в
  `gateway::validate_selector` + отказ по наличию ключа на разборе + слой 2), даёт
  **29/29 по шести наборам M-84 — и 209 красных из 329 по корпусу двух крейтов**
  (52 тестовых бинаря из 78). Тесты sacred, dev их не правит; `verify_M-84.sh` шаг
  `cargo test --all` не позеленеет НИКОГДА;
- среди упавших — **`md_i8_d6b`** (ось «одна полоса против семи»: под гвардом крейта её
  нельзя даже сформулировать), **`red_depth_provenance_by_reach`** (`PROD_BAND`/`DEEP_BAND`),
  **`red_checkpoint_bin_prod_argv`** (прод-форма `--bands=0.001` → `advance_to failed err=VB-I-12`)
  и **`red_serve_passthrough`** (`GATEWAY_BANDS=0.001` → `VB-I-12`);
- то же на проде: `docker-compose.yml:136` и `:217` подают `0.001` и в `gateway-serve`, и в
  чекпоинтер. Спека ТРЕБУЕТ это сохранить (гейт, шаг 9: `GATEWAY_BANDS:-0.001`; задача 6
  заблокирована двумя предусловиями), и та же спека предписывает бинарь, который `0.001`
  ОТВЕРГАЕТ. Merge = деплой (`gates.md` §8) ⇒ выкатка M-84 либо кладёт прод (чекпоинтер
  падает, легаси-слепок перестаёт двигаться, сервер отдаёт `invalid_selector`), либо включает
  семь полос ДО закрытия предусловий — что спека сама называет «осознанной ложью в выдаче».

Это дефект КОНСТРУКЦИИ (место гварда + отсутствие переходного режима), а не текста, и его не
поймал ни один круг, потому что все три меряли предмет изнутри предмета. Ниже — что именно
менять (§4) и что при этом остаётся верным и переделке не подлежит (§2).

---

## §0 — Что судилось; снято командой

```text
$ git rev-parse HEAD; git branch --show-current
10ac14ae0f2993e42e81b2fa9c4e69bd5f1834fa
feat/M-84-fixed-bands

$ git merge-base --is-ancestor 4885e6defffadb3e88eb53ad474ebf149b6759ff 10ac14ae0f2993e42e81b2fa9c4e69bd5f1834fa; echo exit=$?
exit=0

$ git log --oneline 4885e6d..HEAD
10ac14a docs(M-84): C-222 F1 — ложное утверждение о цене такта снято [architect]
5f2fbc7 docs(M-84): record critic cleanup [critic]
7dd592d critique(M-84): round 3 rejects false tick-cost claim [critic]
49d1a90 test(M-84): task #1 — мутационный контроль снят прогоном [architect]
c28208d test(M-84): task #1 — C-221, набор перестал требовать невозможного [architect]

$ git rev-parse --short origin/main
4196ff7
```

Ярус S (состояние мира) снят в начале работы: `main` зелен (`gh run list --branch main
--limit 5` — пять `success`), деплой зелен, VPS на `b008f5e`, `check_branch_health.sh` —
`VERDICT: PASS`, `feat/M-84-fixed-bands` — единственная ветка предмета, дублей нет.

Прочитано целиком (ярус B): `docs/fa/viz-backend.md`, `docs/rfc/CT-RFC-09-ws-session.md`,
`milestones/M-84-fixed-depth-bands.md`, `П-029` (`docs/PENDING-SIGNATURE.md:1769-1930`),
`R-179` (четыре круга), `C-220`, `C-221`, `C-222`; все шесть RED-файлов, `verify_M-84.sh`,
тела `depth_from_book`, `selector_fingerprint`, `ckpt_path_for`, `validate_selector` (оба),
`wire_v1::parse_selector`, `checkpoint::advance_to`, `serve_config_from_env`,
`md_i8_d6b_cost_does_not_multiply_by_number_of_bands`.

---

## §1 — По шести вопросам мандата

### 1.1 Применена ли `П-029` к M-84 по-настоящему — ДА, формула и ссылки верны

Открыто тело и обе строки подписи:

```text
$ nl -ba crates/gateway/src/lib.rs | sed -n '1195,1215p'   (сокращено)
1195   let thresholds: Vec<i64> = bands.iter().map(|b| ...).collect();     ← пороги
1202   for &(price, size) in levels {                                       ← обход уровней
1203       if size == 0 { continue; }
1206       count += 1;                                                       ← счётчик: один на уровень
1207       for (i, &threshold) in thresholds.iter().enumerate() {           ← цикл по ВСЕМ порогам
1212           if qualifies { sums[i] += size; }
1215       }

$ nl -ba docs/PENDING-SIGNATURE.md | sed -n '1800,1801p'
1800 | цена такта линейна по числу полос: цикл по порогам на КАЖДЫЙ уровень книги | `crates/gateway/src/lib.rs:1191-1215` |
1801 | цена такта = уровни × полосы; **размер книги брать по замеру `TD-200`: до 16 000 уровней НА СТОРОНУ** … |
```

Строка спеки `:59` («один МАТЕРИАЛИЗАЦИОННЫЙ обход уровней, но на КАЖДОМ ненулевом уровне
цикл по ВСЕМ порогам»; ссылки `:1195-1201`, `:1202-1206`, `:1207-1215`, `:1206`) — совпадает
с кодом и с подписью. Раздел «ЦЕНА ТАКТА — ИСПРАВЛЕНО» (`:91-116`) называет и механизм ошибки
(комментарий `:1388-1391` вместо тела), и пробел (оракула на `уровни × полосы` нет). Проверены
и остальные ссылки таблицы «Почему — замер»: `:3449-3462` (отпечаток хеширует `bands.len()` и
каждое значение `to_bits`), `:3305-3314` (`ckpt-<fp_hex16>.bin`, `ckpt_path_for` `:3310-3315`),
`:1425-1440` (обе стороны одним вызовом на сторону, строки `(band, side)`),
`docs/fa/viz-backend.md:42`, `:80-84`, `:158-162`, `:169`, `crates/venue-binance/src/lib.rs:33`
(`MAX_REL_DIST = 0.60`), `docker-compose.yml:136,217`, `session.rs:70-106` (полосы сегодня
принимаются). Все указывают туда, куда обещают.

Одна ссылка вне спеки, в самой `П-029:1801`: `lib.rs:1150`, `:1177` за `book.levels(side)` —
на судимой ревизии `:1150` есть `} = &md.payload`, `:1177` — строка комментария. Это дрейф
уже влитой записи (её судил `R-179`), к M-84 отношения не имеет; называю, чтобы не тиражировать.

### 1.2 Достаточен ли путь (а) снятия F1 — ДА, с одной обязательной доделкой

`C-222` предлагал: (а) точная формула + снять ложный запрет, либо (б) отдельное измеренное
определение ресурса с оракулом по порогам. Architect выбрал (а), запрет трогать
`depth_from_book` ОСТАВИЛ на новом основании («M-84 не меняет расчёт; оптимизировать нечем
измерить»), пробел назвал кандидатом в долг.

**Решение: (а) достаточно для M-84.** Милестоун фиксирует СОСТАВ набора и не трогает расчёт;
оракул на `уровни × полосы` принадлежит милестоуну оптимизации, и первым его шагом обязан
идти именно этот оракул — спека так и говорит. Заводить его здесь — расширять предмет, и
утверждение о ресурсе в спеке теперь НЕ делается: спека говорит «цена ЗАВИСИТ от числа
полос», что верно по коду, и не называет её измеренной.

**Но класс F1 в корпусе живёт дальше — в исполняемой форме.** Ложное утверждение, из которого
architect его скопировал, стоит не только в комментарии `:1389`, а в ИМЕНИ живого оракула
`crates/gateway/tests/red_depth_recompute_cost.rs:229`:

```text
fn md_i8_d6b_cost_does_not_multiply_by_number_of_bands()
```

Тело (`:230-262`) меряет `depth_levels_visited` — счётчик, который к работе по порогам слеп по
построению (`:1206`). Оракул пиннит реальное свойство (один МАТЕРИАЛИЗАЦИОННЫЙ обход, не семь),
но НАЗВАН утверждением о ЦЕНЕ, и в карте покрытия `MD-I-8` (`docs/fa/viz-backend.md:228`,
строка 3: «семь полос **стоят** ОДИН проход книги, а не семь») это утверждение о цене
воспроизведено прозой. Ровно эта фраза в спеке M-84 была признана ложной кругом 3. Она
однокоренная и должна быть снята там же, где живёт: см. §4 D-4.

### 1.3 Реализуемость — 29/29 воспроизведено; корпус — 209 красных из 329

Одноразовое дерево `git worktree add --detach /tmp/hft-arb-M84-mut 10ac14a`, честная
реализация (43 строки, 3 файла): `pub const CANONICAL_DEPTH_BANDS: [f64; 7]`, точный
`to_bits`-гвард первым делом в `gateway::validate_selector`, отказ по `value.get("bands").is_some()`
в `wire_v1::parse_selector` с подстановкой канонического при отсутствии ключа, слой 2 в
`session::validate_selector` по нормализованному значению.

```text
$ cargo test -p gateway --test red_fixed_bands_{canonical,fingerprint,frame,prewarm}; -p gateway-serve --test red_fixed_bands_{entrypoint,wire}
red_fixed_bands_canonical:   test result: ok. 9 passed; 0 failed
red_fixed_bands_fingerprint: test result: ok. 4 passed; 0 failed
red_fixed_bands_frame:       test result: ok. 3 passed; 0 failed
red_fixed_bands_prewarm:     test result: ok. 3 passed; 0 failed
red_fixed_bands_entrypoint:  test result: ok. 5 passed; 0 failed
red_fixed_bands_wire:        test result: ok. 5 passed; 0 failed
```

Заявленные 5+5+9+4+3+3 = 29 — подтверждены. Дальше — то, чего не сделал никто:

```text
$ cargo test -p gateway -p gateway-serve --no-fail-fast > /tmp/hft-arb-fulltest.log 2>&1; echo exit=$?
exit=101
$ grep -E '^test result' /tmp/hft-arb-fulltest.log | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=120 failed=209 (блоков: 78)
$ grep -cE '^test .* \.\.\. FAILED' /tmp/hft-arb-fulltest.log
209
$ grep -E 'error: test failed, to rerun pass' … | sed 's/.*--test //' | sort -u | wc -l
52
```

Причина у всех одна и та же — гвард крейта:

```text
snapshot: Custom { kind: InvalidInput, error: "VB-I-12 (П-029): selector.bands=[0.001] не равен каноническому набору полос [0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6] — сетку задаёт сервер" }
```

Корпус строит `Selector { bands: vec![0.001] }` в 38 местах и другие не-канонические наборы
ещё в шести (`grep -rhoE 'bands: *vec!\[[^]]*\]' crates/*/tests crates/*/src | sort | uniq -c`),
и все они идут через `snapshot`/`frames_since`/`replay`/`advance_to`, где гвард стоит. Самые
показательные падения — не «случайные» тесты на VWAP, а те, где ось полос — ПРЕДМЕТ оракула:

| оракул | что упало | почему это не «перепишем фикстуры» |
|---|---|---|
| `md_i8_d6b_cost_does_not_multiply_by_number_of_bands` | FAILED | сравнивает `[0.60]` с семью: под гвардом крейта одна полоса — невалидный вход. Обязательство 3 карты `MD-I-8` (`fa/viz-backend.md:228`) под этим гвардом НЕПРЕДЪЯВИМО |
| `red_depth_provenance_by_reach` (9) | `PROD_BAND`/`DEEP_BAND` | оракул охвата по сторонам — тоже дифференциален по полосам |
| `red_checkpoint_bin_prod_argv` (8) | `SETUP НЕ СОСТОЯЛСЯ: прод-писатель вышел с Some(1): gateway-checkpoint: advance_to failed … err=VB-I-12` | это ПРОД-ФОРМА argv `--bands=0.001` (`docker-compose.yml:217`) — не фикстура, а то, что крутится на VPS |
| `red_serve_passthrough` (2), `smoke_ws`, `red_ws_session` (9) | `VB-I-12 … selector.bands=[0.001]` | серверный селектор из `GATEWAY_BANDS` (`serve_config_from_env`, `lib.rs:2082`) |

Вывод: гвард «ничего, кроме канонического» НА УРОВНЕ КРЕЙТА несовместим с (i) дифференциальными
оракулами `MD-I-8` по оси полос, (ii) прод-конфигурацией, которую спека сама требует сохранить,
(iii) `research-cli` и любым офлайн-читателем библиотеки. Переписать 44 файла sacred-тестов
под канонический набор нельзя не потому, что долго, а потому, что часть из них ПЕРЕСТАЁТ
существовать как оракулы: у `d6b` нет формулировки без второго набора.

### 1.4 Мутации — все три числа сошлись; четвёртая моя — ловится

Каждая — на честном кандидате, с восстановлением между прогонами (`git checkout -- . &&
git apply honest.patch`):

```text
=== A: удалить validate_selector(sel)? из checkpoint::advance_to
  red_fixed_bands_prewarm      FAILED 2 passed 1 failed      ← заявлено 2/1 ✓; пять прочих ok
=== B: разбор принимает любой набор и МОЛЧА заменяет каноническим
  red_fixed_bands_entrypoint   FAILED 1 passed 4 failed      ← заявлено 1/4 ✓; пять прочих ok
=== C: session::validate_selector -> Ok(())
  red_fixed_bands_wire         FAILED 1 passed 4 failed      ← заявлено 1/4 ✓
  red_fixed_bands_entrypoint   ok 5 passed 0 failed          ← entrypoint ЗЕЛЁН, как заявлено (слоение)
=== F (арбитра): разбор принимает КАНОНИЧЕСКИЙ набор от клиента молча, чужой отвергает
  red_fixed_bands_entrypoint   FAILED 4 passed 1 failed      ← `even_canonical_bands_from_client_rejected_at_entrypoint`
```

Мутация F выбрана как самый вероятный «разумный» срез dev'а: «правильный набор — почему бы
не принять». Её ловит ровно один тест, и он есть. Мутационный раздел спеки — честен в каждом
числе. Что он НЕ говорит — что тот же кандидат роняет 209 оракулов рядом (§1.3): мутационный
контроль отвечает на вопрос «пиннит ли набор дефект», а не «выполним ли набор вместе с
остальным корпусом». Второй вопрос `testing.md` («что пришлось ослабить рядом») здесь надо
было задать не про слой 2, а про соседние 52 бинаря.

### 1.5 Однокоренные ложные утверждения о коде — найдены три, ни одно не в спеке M-84

Спека после `10ac14a` чиста: таблица замеров и раздел «Почему — замер» проверены открытием
каждого места (§1.1). Класс живёт в СОСЕДНИХ носителях:

| где | что | зона |
|---|---|---|
| `docs/fa/viz-backend.md:228` (карта `MD-I-8`, строка 3) | «семь полос **стоят** ОДИН проход книги, а не семь» — утверждение о ЦЕНЕ в форме, признанной ложной `C-222` F1 | architect (в Allowed paths M-84) |
| `crates/gateway/tests/red_depth_recompute_cost.rs:229` | имя оракула `…cost_does_not_multiply_by_number_of_bands`; меряет `depth_levels_visited` | architect (tests sacred) |
| `crates/gateway/src/lib.rs:1165`, `:1388-1389` | комментарии «цена не множится на число полос» — источник, из которого F1 был скопирован | engine-dev (комментарий, не поведение) |

Про отпечаток селектора и имя слепка (мандат просил отдельно): `:3449-3462` — хеш `venue`,
`symbol`, `timeframe_ms`, `window_ms`, `bands.len()`, каждый `b.to_bits()`, затем
`depth_cadence_ms` (`:3469`) — шесть осей, как и написано; `:3310-3315` —
`ckpt-{fp:016x}.bin`. Утверждения спеки верны.

### 1.6 Гейт — `FAIL (14)`, exit=1, ровно как объявлено; два латентных дефекта

```text
$ CARGO_TARGET_DIR=/tmp/hft-arb-target-clean bash scripts/verify_M-84.sh; echo exit=$?
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all
FAIL: cargo test -p gateway --test red_fixed_bands_canonical
PASS: grep -q '1\.5/3/5/8/15/30/60' docs/fa/viz-backend.md
FAIL: grep -q 'CANONICAL_DEPTH_BANDS' crates/gateway/src/lib.rs
FAIL: … crate_rejects_foreign_band_set
FAIL: … red_fixed_bands_wire  (×3 вызова)
FAIL: … red_fixed_bands_entrypoint
FAIL: … red_fixed_bands_frame
FAIL: … red_fixed_bands_fingerprint (×2)
PASS: grep -q 'П-029' … PASS: grep -q '2.2bis' … PASS: grep -q 'включая канонический' …
PASS: grep -q 'VB-I-12' docs/fa/viz-backend.md
PASS: grep -q 'в коде ЕЩЁ НЕТ' docs/fa/viz-backend.md
PASS: bash scripts/verify_design_claims.sh
FAIL: … red_fixed_bands_prewarm (×2)
PASS: grep -q 'GATEWAY_BANDS:-0.001' docker-compose.yml
PASS: grep -qE '^\| \*\*TD-159\*\*' TECH-DEBT.md
PASS: grep -q 'Барьера, удерживающего эмиссию до восстановления глубины, НЕТ' docs/fa/viz-backend.md
PASS: grep -qE 'TD-159' milestones/M-84-fixed-depth-bands.md
PASS: grep -q 'барьер, удерживающий эмиссию' milestones/M-84-fixed-depth-bands.md
VERDICT: FAIL (14)
exit=1
```

Все 14 — объявленный COMPILE-RED (`E0425` на `CANONICAL_DEPTH_BANDS`), причина одна. Шаги
орфографии вместо поведения (`C-220` B-4) сняты; текстовые шаги задачи 4 — сверка ДОКУМЕНТА с
решением, это их законный предмет. Два дефекта остались:

- **G-1 (латентный, сработает при первом же GREEN).** Шаг `grep -q 'в коде ЕЩЁ НЕТ'
  docs/fa/viz-backend.md` пиннит ДО-реализационный статус `VB-I-12`. Как только задачи 2-3
  зелены, FA обязана сказать «в коде ЕСТЬ» — и шаг краснеет; либо FA продолжает лгать, чтобы
  гейт был зелен. Фраза в FA одна (`grep -c` → `1`), значит развязки нет. Это класс `TD-138`
  наоборот: гейт обосновывает состояние, которое обязан пережить.
- **G-2.** Шаг 1 `cargo test --all` — верный по замыслу — на честном кандидате красен по
  §1.3, и это ЕДИНСТВЕННЫЙ шаг гейта, который видит дефект конструкции. Он не дефект, он
  свидетель; называю, чтобы его не «починили» сужением до `-p gateway --test red_fixed_bands_*`.

---

## §2 — Что ВЕРНО и переделке не подлежит (чтобы круг 4 не тронул лишнего)

1. **Три слоя контракта** (`CT-RFC-09` §2.2bis, спека «Контракт — три слоя») — правильная
   развязка `C-221` B-1: провенанс есть только на разборе. Слои 1 и 2 (разбор кадра, валидатор
   сессии) — на месте, оракулы `entrypoint`/`wire` их пиннят, мутации B/C/F ловятся.
2. **Оракулы `fingerprint`, `frame`, `prewarm`** — верны и не зависят от места гварда:
   отпечаток, 14 строк на настоящем `gateway::snapshot`, сосуществование слепков разных
   отпечатков. Легаси-слепок как ФИКСТУРА (байты под именем из `selector_fingerprint`) —
   верное снятие `C-221` B-2.
3. **Формула цены и её ссылки** — верны (§1.1). Запрет трогать `depth_from_book` в M-84 —
   верен по новому основанию.
4. **Задача 6 заблокирована двумя предусловиями** (`TD-159` OPEN MAJOR — `TECH-DEBT.md:107`,
   `:880`; барьер эмиссии — `fa/viz-backend.md:152-162`). Каденция (`M-68`) — исполненный
   контекст. Всё названо верно; я это не трогаю (§5).
5. **RAW-гейт (сильная модель)** назван в спеке явно — верно, `CT-RFC-09` тронут.

---

## §3 — Находки, ранжированные

### F-1 (BLOCKER, конструкция) — гвард «только канонический» на уровне КРЕЙТА невыполним

Замер — §1.3. Аргумент спеки за место гварда (`Acceptance` п.3, `red_fixed_bands_canonical.rs`
шапка): «`Selector` собирают напрямую research-cli, чекпоинтер, M-39 — гвард только в
транспорте оставил бы байпас, класс `GW-I-14`». Аналогия не держит: `GW-I-14` отвергает
НЕВАЛИДНОЕ значение (`window_ms < 0` ведёт себя как unbounded — мусор, выглядящий валидным).
`[0.001]` — валидный, корректно считаемый селектор; «не канонический» — свойство ПРОДУКТОВОЙ
композиции (что сервер отдаёт клиентам), а не корректности библиотеки. Библиотека, которая
умеет считать только один набор, теряет собственные дифференциальные оракулы (`d6b`,
`provenance_by_reach`) и офлайн-читателей.

«Байпас», которого боялись: чекпоинт чужого отпечатка. Он ИНЕРТЕН — сервер с каноническим
селектором его никогда не откроет (`ckpt_path_for` — имя из отпечатка, `:3310`; сверка
заголовка `:3982`). Файл на диске, который никто не читает, — не путь мимо решения.

### F-2 (BLOCKER, конструкция) — между merge'ем и задачей 6 бинарь несовместим с продом

Спека не говорит, как ведёт себя ВЫКАЧЕННЫЙ бинарь, пока задача 6 заблокирована. При этом:
гейт (шаг 9) требует сохранить `GATEWAY_BANDS:-0.001`; `serve_config_from_env` (`lib.rs:2082`)
и чекпоинтер (`--bands`, `gateway-checkpoint.rs:138,181`) строят селектор из этого значения;
честный кандидат его отвергает (§1.3 — `red_checkpoint_bin_prod_argv`, `red_serve_passthrough`).
Merge в `main` — это деплой. Значит после merge'а M-84 в его нынешней форме прод либо падает
(cron чекпоинтера — exit 1, легаси-слепок замирает, каждое подключение — полный проход
журнала: ровно то «окно», которое задача 5 обещала сделать нулевым), либо dev «чинит» это,
переключив компоуз на семь полос — то есть исполняет задачу 6 мимо founder'а.

`П-029` явно оставила спеке два вопроса — «отношение к `M-70`» и «момент включения». Спека
не отвечает ни на один: `grep -c 'M-70' milestones/M-84-fixed-depth-bands.md` → `0`.

### F-3 (MAJOR, гейт) — G-1 из §1.6: шаг гейта пиннит состояние, которое обязан пережить

### N-1 (NOTE, класс F1) — однокоренные утверждения о цене вне спеки: §1.5

### N-2 (NOTE) — `П-029:1801` ссылается на `lib.rs:1150`, `:1177`, где не `book.levels` (§1.1)

---

## §4 — РЕШЕНИЕ, обязательное к исполнению обеими сторонами

По каждому пункту — кто прав и что делать. Существо `П-029` не переоткрывается (граница C).

**D-1. `C-222` F1: критик прав, architect признал, путь (а) ДОСТАТОЧЕН.** Оракул на
`уровни × полосы` в M-84 не заводится; долг «нет оракула на цену такта depth-расчёта;
существующий счётчик меряет прокси» передаётся reviewer'у карточкой, как спека и предлагает.
Дальше по F1 — ничего, кроме D-4.

**D-2. Гвард «только канонический» УБИРАЕТСЯ из `gateway::validate_selector` и из
`checkpoint::advance_to`; он живёт на ПРОДУКТОВЫХ входах.** Единственный источник набора —
`gateway::CANONICAL_DEPTH_BANDS` (T2, константа крейта, остаётся). Отвергают не-канонический
набор: слой 1 `wire_v1::parse_selector` (наличие ключа) и слой 2 `session::validate_selector`
(значение) — как сейчас; плюс серверный вход (`serve_config_from_env`) и чекпоинтер — по D-3.
Библиотека валидирует КОРРЕКТНОСТЬ набора (непустой, в `(0,1)`, строго возрастающий — сегодня
это делает только `session.rs:75-91`, крейт не проверяет вовсе; поднять эту проверку в крейт —
законно и полезно, это класс `TD-198`), но не его состав. Следствия для набора:
- `red_fixed_bands_canonical.rs`: `canonical_set_exists_and_equals_product_set`,
  `…sorted_and_without_duplicates`, `…within_source_coverage` — остаются; семейство
  `crate_rejects_*` и `crate_accepts_canonical_set` — переезжает на слой 2 (`wire.rs` уже
  несёт большинство) либо на серверный вход по D-3; `f32`-двойник (`C-220` B-2) переезжает
  вместе с ними — свойство «точное сравнение, не допуск» верно там, где сравнение есть;
- `red_fixed_bands_prewarm.rs::legacy_selector_is_refused_by_the_normal_checkpoint_api` —
  переезжает на БИН чекпоинтера в прод-форме argv (прецедент — `red_checkpoint_bin_prod_argv.rs`)
  и судится ПО D-3 (в переходном режиме легаси-набор через явное переопределение ПРИНИМАЕТСЯ —
  см. ниже; отвергается набор, не равный ни константе, ни явно переданному операторскому);
  два прочих теста файла не меняются;
- строка Forbidden «убирать `validate_selector` из `advance_to`» — снимается как предмет;
  `validate_selector` в `advance_to` остаётся, но проверяет корректность, не состав;
- Acceptance п.3 переформулируется: «чужой набор отвергается на КАЖДОМ продуктовом входе
  (кадр клиента, env сервера, argv чекпоинтера); библиотека состав не диктует — иначе
  `MD-I-8` теряет обязательство 3».
После правки architect обязан предъявить ПОЛНЫЙ `cargo test -p gateway -p gateway-serve`
против честного кандидата, а не шесть наборов: число `passed`/`failed` по корпусу — в раздел
«Мутационный контроль», рядом с 29.

**D-3. Переходный режим, названный явно, с концом.** Между merge'ем M-84 и задачей 6 прод
НЕ меняет вычисляемый набор и НЕ падает. Конструкция — по прецеденту `M-75` в этом же крейте
(`DEFAULT_HEATMAP_WINDOW_FRAC` + процессное эффективное значение, `lib.rs:109-123`; оракулы
`red_heatmap_window_server_owned.rs`, `red_heatmap_window_decoupled.rs`) и по норме
`CT-RFC-09` §2.5 («переходный период и его КОНЕЦ»):
- серверный набор = `CANONICAL_DEPTH_BANDS` по умолчанию; `GATEWAY_BANDS` остаётся ЯВНЫМ
  операторским переопределением на переходный период, при каждом старте логируется
  `deprecated` с именем переменной и применённым значением; чекпоинтер — то же через `--bands`;
- слой 2 и слой 1 судят клиента против ЭФФЕКТИВНОГО серверного набора (в тестах без env
  это константа — оракулы `entrypoint`/`wire` не меняются по существу; формулировка
  `legacy_single_band_is_rejected_by_session` уточняется: «не равный эффективному набору»);
- клиент НЕ присылает `bands` ни в каком режиме — это ядро `П-029`, оно не переходное;
- **конец режима = задача 6** (founder): переменная удаляется, оракул «сервер не читает
  `GATEWAY_BANDS`» заводится тем же шагом. Дата/условие — в спеке, не «когда-нибудь»
  (`CT-RFC-09` §2.5 дословно). Спека называет отношение к `M-70`: кто закрывает `TD-159`
  и барьер эмиссии — `M-70` задача 4 или новая задача M-84 — решение architect'а как порядок
  работ, но оно обязано быть НАПИСАНО, `П-029` это спеке и адресовала;
- гейт: шаг 9 остаётся (компоуз на `0.001` — это и есть переходный режим), плюс два новых:
  без env — эффективный набор равен константе; с env — применён И залогирован. Оба — по
  поведению, не по тексту.
Альтернатива — держать M-84 незамерженным до закрытия обоих предусловий — допустима ТОЛЬКО
как решение founder'а о порядке работ (`M-70` несёт неснятый `R-172`, срок неизвестен);
по умолчанию действует D-3.

**D-4. Однокоренные утверждения о цене снимаются в ЭТОМ же круге там, где зона architect'а:**
`docs/fa/viz-backend.md:228` — «семь полос стоят ОДИН проход книги» → «семь полос требуют ОДНОГО
материализационного обхода книги, а не семи; работа по порогам — `уровни × полосы`, оракула на
неё нет (`C-222` F1)». Имя оракула `md_i8_d6b_cost_does_not_multiply_by_number_of_bands` —
переименовать в то, что он меряет (`…single_materialization_pass_for_all_bands` или
эквивалент), с правкой карты покрытия и `verify_M-68` (если зовёт по имени — проверить грепом).
Комментарии `lib.rs:1165`, `:1388-1389` — зона engine-dev; спека РАЗРЕШАЕТ dev'у поправить
ТОЛЬКО текст комментария (не тело функции) отдельным коммитом «docs-comment» — оставлять
источник F1 в коде, откуда его скопируют в следующий раз, нельзя.

**D-5. Гейт:** шаг `grep -q 'в коде ЕЩЁ НЕТ'` заменить на проверку СОГЛАСОВАННОСТИ: FA не
имеет права говорить «в коде ЕЩЁ НЕТ», когда `CANONICAL_DEPTH_BANDS` есть в `lib.rs`, и
наоборот. Шаг 1 (`cargo test --all`) не сужать.

**Маршрут после исправления.** Правки D-2/D-3 меняют ФОРМУ спеки (Acceptance, слои,
Forbidden) и `CT-RFC-09` не трогают по существу (§2.2bis остаётся верной: клиент не присылает
`bands`; переходный режим — §2.5 того же RFC, уже нормативен). Это круг критика 4 по
`gates.md` §9/§1 (RAW — сильная модель), НЕ круг перепроверки. Арбитраж §0 сработал на
«три круга по одному предмету»; предмет круга 4 — НОВАЯ конструкция по этому решению, и
его вердикт судит исполнение D-2..D-5, не переоткрывая F1/B-1/B-2. Engine-dev — только после
него.

---

## §5 — Что здесь НЕ решено и founder'у не подменяется (`gates.md` §0.1)

- Включение семи полос на проде (задача 6) — остаётся заблокированным `TD-159` и барьером
  эмиссии; статус не трогаю, рекомендаций по нему не даю.
- Порядок `M-70` ↔ `M-84` — порядок работ, architect пишет в спеку; если он выбирает «держать
  M-84 до закрытия предусловий» вместо D-3 — это founder'у.
- Состав набора (семь полос) — подписан, не обсуждается.

## §6 — Предъявление FA (M-66)

Предмет трогает `crates/gateway/**` и `crates/gateway-serve/**`. Живые инварианты
`docs/fa/viz-backend.md` на судимой ревизии, на которые опирается вердикт: **`VB-I-12`**
(§5, строка 209 — сетку задаёт сервер; статус «принято, в коде ЕЩЁ НЕТ» верен), **`MD-I-8`**
(карта покрытия, обязательство 3 — `d6a`/`d6b`/`d10`: под гвардом крейта непредъявимо,
это ядро D-2), **`VB-I-5`** (провенанс глубже 1.3 % — `red_depth_provenance_by_reach` в числе
упавших). Все три существуют в файле на `10ac14a`.

---

## Done Block

```text
$ git worktree add /tmp/hft-arb-M84 feat/M-84-fixed-bands
fatal: 'feat/M-84-fixed-bands' is already used by worktree at '/tmp/hft-rm2'
$ git worktree add --detach /tmp/hft-arb-M84 origin/feat/M-84-fixed-bands && cd /tmp/hft-arb-M84 && git checkout -B feat/M-84-fixed-bands origin/feat/M-84-fixed-bands
Your branch is up to date with 'origin/feat/M-84-fixed-bands'.
$ git rev-parse HEAD
10ac14ae0f2993e42e81b2fa9c4e69bd5f1834fa

$ gh run list --branch main --limit 5            → 5 × completed success (CI ×2, Deploy ×3)
$ ssh … 'cd /root/hft-platform && git rev-parse --short HEAD'   → b008f5e
$ bash scripts/check_branch_health.sh | tail -1  → VERDICT: PASS — наблюдение состоялось
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-arb-target-clean bash scripts/verify_M-84.sh; echo exit=$?
VERDICT: FAIL (14)
exit=1        # объявленный COMPILE-RED, причина одна: E0425 CANONICAL_DEPTH_BANDS

$ git worktree add --detach /tmp/hft-arb-M84-mut 10ac14ae0f2993e42e81b2fa9c4e69bd5f1834fa
$ (честный кандидат) git diff --stat
 crates/gateway-serve/src/session.rs | 13 +++++++++++++
 crates/gateway-serve/src/wire_v1.rs | 12 ++++++++++--
 crates/gateway/src/lib.rs           | 20 ++++++++++++++++++++
 3 files changed, 43 insertions(+), 2 deletions(-)

$ шесть наборов M-84 против честного кандидата
canonical 9 passed; fingerprint 4 passed; frame 3 passed; prewarm 3 passed; entrypoint 5 passed; wire 5 passed
# 29 passed, 0 failed

$ cargo test -p gateway -p gateway-serve --no-fail-fast > /tmp/hft-arb-fulltest.log 2>&1; echo exit=$?
exit=101
$ grep -E '^test result' /tmp/hft-arb-fulltest.log | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=120 failed=209 (блоков: 78)
$ grep -cE '^test .* \.\.\. FAILED' /tmp/hft-arb-fulltest.log
209
$ grep -E '^test md_i8_d6b|^test md_i8_d6a' /tmp/hft-arb-fulltest.log
test md_i8_d6b_cost_does_not_multiply_by_number_of_bands ... FAILED
test md_i8_d6a_counter_actually_measures_visited_levels ... ok
$ awk '/^     Running/{bin=$2} /^test .* \.\.\. FAILED/{c[bin]++} END{for(b in c) print c[b], b}' … | sort -rn | head -5
10 red_checkpoint_bootstrap_truncated.rs
9 red_ws_session.rs
9 red_depth_provenance_by_reach.rs
9 red_depth_from_book.rs
8 red_window_selector_guard.rs
$ grep -E 'SETUP НЕ СОСТОЯЛСЯ: прод-писатель' /tmp/hft-arb-fulltest.log | head -1 | cut -c1-200
SETUP НЕ СОСТОЯЛСЯ: прод-писатель вышел с Some(1): gateway-checkpoint: advance_to failed dir=… ckpt=… err=VB-I-12 (П-029): sel…

$ мутации (каждая на восстановленном честном кандидате; шесть наборов)
A: prewarm FAILED 2 passed 1 failed; пять прочих ok
B: entrypoint FAILED 1 passed 4 failed; пять прочих ok
C: wire FAILED 1 passed 4 failed; entrypoint ok 5 passed; четыре прочих ok
F: entrypoint FAILED 4 passed 1 failed; пять прочих ok
$ git checkout -- . && git apply /tmp/hft-arb-honest.patch   → восстановлено; дерево снесено после замера, ничего не коммичено

$ grep -rn 'не множится\|стоят ОДИН проход' docs/fa/viz-backend.md crates/gateway/src/lib.rs
docs/fa/viz-backend.md:228:| 3 | **граница работы:** … семь полос стоят ОДИН проход книги, а не семь | `d6a`, `d6b`, `d10` |
crates/gateway/src/lib.rs:1165:    // Принимает ВСЕ полосы разом — … (`d6b`: цена не множится на число
crates/gateway/src/lib.rs:1389:    /// задача 8, `d6b`): цена не множится на число полос. …
$ grep -n 'fn md_i8_d6b' crates/gateway/tests/red_depth_recompute_cost.rs
229:fn md_i8_d6b_cost_does_not_multiply_by_number_of_bands() {
$ grep -c 'в коде ЕЩЁ НЕТ' docs/fa/viz-backend.md
1
$ grep -c 'M-70' milestones/M-84-fixed-depth-bands.md; echo exit=$?
0
exit=1
$ grep -n 'GATEWAY_BANDS' docker-compose.yml
136:      GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001}
217:      - --bands=${GATEWAY_BANDS:-0.001}
$ grep -rhoE 'bands: *vec!\[[^]]*\]' crates/*/tests crates/*/src | sort | uniq -c | sort -rn | head -3
     38 bands: vec![0.001]
      2 bands: vec![PROD_BAND, DEEP_BAND]
      2 bands: vec![PROD_BAND]
```
