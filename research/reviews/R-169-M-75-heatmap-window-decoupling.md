<!-- GATE-META
milestone: M-75
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: de6e87fc35360221733d2cce1c48e1248a4a154a
verdict: APPROVE
-->

# R-169 — M-75 (расцепление окна heatmap от полос), PR-time reviewer: **APPROVED**

## ВЕРДИКТ КОРОТКО

**APPROVED.** Диф делает ровно то, что объявлено в §5 спеки, и не делает ничего сверх.
Расцепление предъявлено не рассуждением, а конструкцией: `build_heatmap_and_cob` больше
физически не может достать полосы — они не входят в сигнатуру. Мутационный контроль
ревьюера (мой, не пересказ чужого) убивает оба мира, которых боялись пять кругов критика.

Блокеров нет. Один долг заводится картой — он был назван architect'ом заранее (§4bis(2)),
а не найден мной, и это правильный порядок.

## Что я прочитал, и чем греп ограничен (ярус C, `reading-map.md` §2)

Ярус A целиком: `CLAUDE.md`, `.claude/rules/{gates,testing,commit-discipline,branch-hygiene,scope-guard,handoff-block}.md`,
`docs/04-workflow.md`, `docs/05-contract-layer.md`, `docs/SESSION-HANDOFF.md` §0bis/§0ter.
Ярус B по предмету: `milestones/M-75-heatmap-window-decoupling.md` (шапка, §1-§14),
`docs/fa/viz-backend.md`, `research/critiques/C-207-*.md`.

**Ярус C — ТОЛЬКО грепом, и я называю, чем именно грепал** (файлы вместе ≈1 МБ; «прочитал
TECH-DEBT» было бы заведомо ложным утверждением):

- `TECH-DEBT.md`: `grep -nE 'heatmap|HEATMAP|M-75|M-70|M-71'` → `TD-161`, `TD-171`, `TD-174`
  (закрыт), `TD-179`, плюс замеры объёма heatmap;
- `PROJECT-STATE.md`: `grep -nE 'M-75|M-71|M-68|GATEWAY_BANDS'` → `П-014` п.4 не начат,
  прод несёт `GATEWAY_BANDS=0.001` (замер 2026-08-23), разделы M-71/M-68/M-73.

**Чего греп НЕ покрыл и я это не изображаю:** карточки, называющие предмет иными словами
(«окно», «COB», «egress») в теле, а не в заголовке, я не искал.

## Предъявление FA — живые инварианты ОБОИХ тронутых крейтов (M-66)

Диф трогает два крейта, и барьер `check_review_fa.sh:190-199` требует по инварианту на
каждый — префиксы РАЗНЫЕ. Тестер назвал только `VB-I-2`; для `gateway-serve` этого бы не
хватило, барьер требует `GS-`.

- **`crates/gateway`** → `docs/fa/viz-backend.md:199` **`VB-I-2`** (live == replay): серия на
  live-хвосте бит-идентична серии из replay того же окна. Это инвариант, по которому
  расцепление и надо судить, и он ДЕРЖИТСЯ по конструкции: `finish` (`lib.rs:1388-1397`)
  делегирует в `finish_ref`, у `build_heatmap_and_cob` РОВНО ОДИН вызыватель
  (`lib.rs:1506`), оба пути читают один процессный атомик. Разойтись им негде.
- **`crates/gateway-serve`** → `docs/fa/viz-backend.md:206` **`GS-I-1`/`VB-I-9`** (граница
  плоскостей: транспорт не читает application-БД, auth — только stateless verify). Правка
  задачи 3 — 29 строк чистого разбора env в `serve_config_from_env`; ни БД, ни lookup'а не
  вводит. Инвариант не задет.

Оба ID существуют в файле на проверяемой ревизии (`grep -oE '\b(VB|GS)-I-[0-9]+\b'`).

## Block-scope — ПРОЙДЕН

18 файлов, `git diff --stat d77398d7..HEAD`. Соответствие §6 Allowed paths поимённо:

| путь | зона по §6 | факт |
|---|---|---|
| `crates/gateway/src/lib.rs` (+36/−11) | engine-dev | ✅ |
| `crates/gateway-serve/src/lib.rs` (+29) | engine-dev, «только разбор env» | ✅ разбор + один сеттер |
| `docker-compose.yml` (+1) | engine-dev, задача 4 | ✅ одна строка `GATEWAY_HEATMAP_WINDOW` |
| `crates/gateway{,-serve}/tests/red_heatmap_window_*.rs` | architect, sacred | ✅ |
| четыре чужих оракула (задача 2b) | architect, sacred | ✅ |
| `scripts/verify_M-75.sh`, `milestones/M-75-*.md` | architect | ✅ |
| `research/critiques/C-19{4,6,8},C-20{1,7}-*.md` | critic (артефакты гейта, `gates.md` §4) | ✅ |

**§7 Forbidden — проверен ЗАМЕРОМ, не доверием:** `crates/contracts/**` и `docs/rfc/**` вне
диапазона (`git diff --name-only` → пусто); `GATEWAY_SCHEMA_VERSION` не тронут (остался `9`);
`Selector` и `selector_fingerprint` не изменены; `GATEWAY_BANDS` в compose не тронут. Все
четыре запрета продублированы шагами самого гейта (`verify_M-75.sh:297-299`).

## Block-DoneBlock — ПРОЙДЕН, и я прогнал сам

Done Block тестера — сырой stdout, не пересказ. Мой независимый прогон на СВОЁМ worktree
совпал с ним по всем четырём числам (см. «Done Block — прогон ревьюера» ниже). Расхождений
ноль.

## Block-C — N/A, основание предъявлено

`git diff --name-only d77398d7..HEAD -- crates/contracts docs/rfc` → пусто. T1 не тронут,
contract-RFC не требуется. `GATEWAY_SCHEMA_VERSION` не бампнут — и это не упущение, а
осознанный резерв за пирамидой (§4 спеки): два бампа с разных веток дали бы коллизию версий,
класс уже разбирался в `C-190`.

## Block-risk — N/A по путям, и это НЕ отговорка

`gates.md` §5 привязывает RISK-BLOCK к `crates/{risk,killswitch,oms,venue-*}` и
`crates/contracts`. Диапазон не трогает НИ ОДИН из них (перечень файлов выше исчерпывающий).
risk-critic не требуется; отсутствие его вердикта в цепочке блокером не является.

Отдельно отмечаю, потому что путь к ресурсу тут всё же есть: милестоун **усиливает** `PL-I-5`,
а не ослабляет. Ширина карты перестаёт быть клиентским входом ПО ПОСТРОЕНИЮ, предел `M-71`
остаётся бэкстопом и не тронут (`enforce_response_limit` на обоих путях, включая
чекпоинт-путь `lib.rs:2735`).

## RED-first — ПРОЙДЕН, порядок предъявлен git'ом

Хронология коммитов (`git log --reverse` + `--numstat` по каждому):

| # | коммит | файлы | роль |
|---|---|---|---|
| 1 | `c3ee54b` | `tests/red_heatmap_window_decoupled.rs` + спека + гейт | architect |
| 2 | `c996390` | ещё два файла оракулов | architect |
| 3 | `af738fc` | `tests/red_heatmap_window_effective_setting.rs` | architect |
| 4 | `94a105f` | сужение стражей присутствия | architect |
| **5** | **`5941aef`** | **`crates/gateway/src/lib.rs` — ПЕРВАЯ строка импла** | engine-dev |
| 6 | `38c6227` | `crates/gateway-serve/src/lib.rs` | engine-dev |
| 7 | `5bf9f28` | `docker-compose.yml` | engine-dev |
| 8 | `0078bda` | четыре чужих оракула (задача 2b) | architect |

**Все четыре оракула легли ДО первой строки реализации.** Дев не тронул `*/tests/` ни разу —
проверено `--numstat` по каждому из трёх его коммитов: только `src/` и compose. Sacred-зона
не нарушена; задачу 2b (правка чужих оракулов) исполнил architect, как и предписано §8.

## Атомарность коммитов — ПРОЙДЕНА

Одна задача = один коммит, ссылка на milestone/task в subject'е, метка роли в конце. Бандла
нет. Co-author трейлеров нет (`git log --format=%B | grep -c Co-Authored` → 0).

## Мутационный контроль — МОЙ, две мутации, обе убивают набор

`testing.md` требует не «тесты зелёные», а «оракул падает против сломанного». Проверял сам,
в своём дереве, точка мутации — `crates/gateway/src/lib.rs:1581` (`let w = window_frac;`).

**Мутация A — «жёсткая константа, конфиг игнорируется»** (`let w = 0.001_f64;`). Это третий
мир, ради которого писался `H-6`, и он самый опасный: удовлетворяет `H-1`, `H-3`, `H-4`,
`H-5` — все пять оракулов `crates/gateway` его пропускают, потому что они судят выдачу при
ОДНОМ значении окна.

```
test hw_i_6_effective_server_setting_controls_map_extent ... FAILED
test hw_i_6b_both_settings_produce_a_nonempty_map ... ok
panicked at crates/gateway-serve/tests/red_heatmap_window_effective_setting.rs:242:5
test result: FAILED. 1 passed; 1 failed
```

Убит ровно тем оракулом, который для него написан, и парный сторож `H-6b` при этом ОСТАЛСЯ
зелёным — то есть `H-6` пиннит предмет, а не шумит.

**Мутация B — «расцепить обнулением»** (`let w = 0.0_f64;`). Удовлетворяет `H-1` и `H-3`
идеально (карта одинакова при любых полосах — её нет) и уничтожает продукт.

```
test hw_i_4_decoupling_does_not_empty_the_heatmap ... FAILED
test hw_i_3_canonical_bands_fit_under_signed_cap ... FAILED
test hw_i_1_heatmap_size_is_independent_of_bands ... FAILED
test result: FAILED. 0 passed; 3 failed
test hw_i_5b_server_window_still_produces_a_map_for_a_below_config_band ... FAILED
test hw_i_5_below_config_band_cannot_shrink_the_map ... FAILED
test result: FAILED. 0 passed; 2 failed
```

Пять из пяти. Сторожи `H-4`/`H-5b` сработали по прямому назначению.

**Восстановление предъявлено:** `sed -n '1581p'` → `let w = window_frac;`,
`git status --porcelain` → пусто, контрольный прогон 3+2+2+5 = **12 passed, 0 failed**.

**Второй вопрос мутации («что пришлось ослабить рядом») — ответ: ничего.** Обратная проверка
— парные сторожи `H-4`, `H-5b`, `H-6b` зелены на неизменённом коде и краснеют только на своей
мутации. Послабления, купленного ценой соседнего инварианта, нет.

## Стражи setup — предъявлены, не предполагаются

`Р-4` требует, чтобы признак был НЕДОСТУПЕН миру ¬P, и чтобы недоступность предъявлялась.
Проверил чтением, а не доверием к §10 спеки:

- `assert_selectors_actually_differ` (`red_heatmap_window_decoupled.rs:170-190`) —
  положительный контроль различающей силы: депт-серия ОБЯЗАНА различаться (`w > n`), иначе
  «карта не изменилась» истинно при любой реализации. Плюс страж непустоты узкой карты;
- `assert_discriminating_power_of_the_fixture` (`red_heatmap_window_server_owned.rs:196`);
- `assert_setting_gap_is_observable` + **страж чистоты опыта**
  (`red_heatmap_window_effective_setting.rs:229-234`): клиентские полосы держатся
  КОНСТАНТНЫМИ, варьируется ровно одна величина — серверная настройка. Это прямое исполнение
  `testing.md` («конфаундинг-величину держать КОНСТАНТНОЙ»), и без него `H-6` доказывал бы
  влияние селектора, а не конфига.

## Механизм на пути (DoD, `gates.md` §4) — ПОДКЛЮЧЕНИЕ ЕСТЬ, установлено замером

Механизм не «построен и не подключён»: цепь прод-пути замкнута и проверена по вызову.

1. `main.rs:21` — прод-вызыватель `serve_config_from_env(|k| std::env::var(k).ok())`;
2. `gateway-serve/src/lib.rs:2089-2113` — fail-closed разбор; `:2357` —
   `set_effective_heatmap_window_frac`, **после** всех `return Err`, рядом с
   `set_effective_max_response_bytes`. Отвергнутая конфигурация сервисом не управляет;
3. `gateway/src/lib.rs:1506` — единственный потребитель, читает атомик на каждом построении;
4. `docker-compose.yml:137` — ручка объявлена оператору.

`grep -rn 'effective_heatmap_window_frac()' crates/*/src/` → ровно один вызов (`:1506`) плюс
объявление. Второго пути к окну не существует.

## §4bis(1) — чекпоинт НЕ инвалидируется: проверено кодом, а не аналогией

Спека утверждает, что расширять `selector_fingerprint` не нужно. Аналогия с `M-68` (каденция
в отпечатке) не проходит, и я это подтвердил чтением:

- `read_checkpoint` (`lib.rs:3362-3367`) возвращает **`Reducer`** — СОСТОЯНИЕ, а не
  построенную выдачу;
- состояние бакета несёт ПОЛНЫЙ снимок книги: `entry.refresh(bids, asks)` (`lib.rs:1204`),
  без отсечения по окну;
- окно применяется на ПОСТРОЕНИИ (`:1506`), уже после `state.apply`.

Значит чекпоинт, снятый при одном окне, описывает то же состояние при другом. Тёплый старт
равен полному реплею (`MD-I-8` обязательство 9), `VB-I-2` цел. Расширение отпечатка
инвалидировало бы чекпоинты без причины.

**Проверил и то, о чём спека молчит:** чекпоинтер — ОТДЕЛЬНЫЙ процесс
(`crates/gateway/src/bin/gateway-checkpoint.rs`), и он `serve_config_from_env` не зовёт, то
есть `GATEWAY_HEATMAP_WINDOW` не читает. Это было бы дырой, если бы он строил выдачу. Не
строит: `grep -nE 'finish|enforce_response_limit|snapshot|HEATMAP'` по файлу даёт ОДИН
комментарий и ни одного вызова; единственная env, которую он читает, — `GATEWAY_DEPTH_CADENCE_MS`
(`:319`). Расхождения окна между процессами не существует ПО ПОСТРОЕНИЮ.

## Fail-closed разбор env — дегенерированный вход покрыт

Прод зовёт `std::env::var(k).ok()`, поэтому пустая строка приходит как `Some("")`, а не
`None`. Разбор (`gateway-serve/src/lib.rs:2091-2112`):

| вход | исход | верно |
|---|---|---|
| переменная отсутствует | `DEFAULT_HEATMAP_WINDOW_FRAC` = 0.001 | ✅ подписанный дефолт |
| `""` / пробелы | `trim()` → `parse` Err → **отказ старта** | ✅ не тихий дефолт |
| неразбираемое | Err → отказ | ✅ |
| `NaN` / `inf` | `is_finite()` false → отказ | ✅ ловится, хотя `parse` их принимает |
| `0.0`, `1.0`, отрицательное | вне `(0,1)` → отказ | ✅ границы исключены строго |

Оракул `H-2` покрывает пять сценариев, включая объявление ручки в compose. Подстановки
дефолта на невалидном входе нет — прямое требование `PL-I-5` и урок `R7`.

## Прод-ожидание — проверено НА ПРОДЕ, а не по compose-дефолту

Утверждение спеки «данные не меняются ни на байт» держится на равенстве нового дефолта
сегодняшнему эффективному окну. Compose-дефолт `${GATEWAY_BANDS:-0.001}` этого не доказывает —
оператор мог задать иное в окружении. Снял с живого контейнера:

```
$ ssh … 'docker inspect $(docker ps -qf name=gateway-serve) --format "{{range .Config.Env}}…"'
GATEWAY_BANDS=0.001
```

`max(bands) = 0.001` = `DEFAULT_HEATMAP_WINDOW_FRAC = 0.001`. Выдача обязана остаться
прежней байт-в-байт. `GATEWAY_HEATMAP_WINDOW` на проде сейчас не задан — после деплоя придёт
из compose тем же значением.

## Merge-preview — `strict: false`, и я не понадеялся на «наверное сойдётся»

`main` уехал на **84 коммита** от базы M-75, защита ветки свежести не требует (`gates.md` §8,
«предел защиты назван честно»). Проверял дерево слияния, а не ветку.

- сухое слияние чистое: `git merge-tree --write-tree origin/main HEAD` → exit=0, конфликтов
  нет. `docker-compose.yml` тронут ОБЕИМИ сторонами, но в разных секциях (`recorder` :31-32
  против `gateway-serve` :137) — обе правки на месте в дереве слияния;
- **дрейф `main` не содержит НИ ОДНОГО `.rs` и ни одного `Cargo.toml`**
  (`git diff --name-only … | grep -E '\.rs$|Cargo\.(toml|lock)$'` → пусто). Значит тройка CI
  на дереве слияния тождественна прогону на ветке — не по вере, а структурно;
- **но в `main` приехал НОВЫЙ джоб `rollout-composition`**, входящий в агрегат
  `status-check` (`ci.yml:557`), и M-75 трогает `docker-compose.yml`. Прогнал его на дереве
  слияния: `VERDICT: PASS — состав, эпоха и подпись согласованы`, exit=0;
- остальные барьеры на дереве слияния: `gate-meta`, `protected-artifacts`, `artifact-ids`,
  `docs-freeze`, `context-budgets` — exit=0 каждый; `verify_design_claims.sh --merge-preview
  origin/main` → `VERDICT: PASS (0 нарушений)`.

## Done Block — прогон РЕВЬЮЕРА (сырой, агрегированный)

Дерево `/tmp/hft-reviewer-m75`, HEAD `de6e87fc35360221733d2cce1c48e1248a4a154a`
(= `origin/feat/M-75-heatmap-window-decoupling`), `git status --porcelain` пусто до и после.

```
$ cargo fmt --all -- --check; echo "fmt_exit=$?"
fmt_exit=0

$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.09s
clippy_exit=0

$ cargo test --all --quiet --no-fail-fast 2>&1 | grep -E '^test result' | awk …
passed=960 failed=0 (блоков: 225)

$ bash scripts/verify_M-75.sh 2>&1 | grep -E '^(FAIL|VERDICT)'; echo "verify_exit=$?"
VERDICT: PASS
verify_exit=0
```

Совпадение с замером тестера — по всем четырём числам, включая 960/0 и 225 блоков.

### Мутационный контроль (повторно, компактно)

```
мутация A (w = 0.001 константа)  → hw_i_6 FAILED, hw_i_6b ok        (1 passed, 1 failed)
мутация B (w = 0.0)              → H-1/H-3/H-4 FAILED               (0 passed, 3 failed)
                                 → H-5/H-5b   FAILED               (0 passed, 2 failed)
восстановление                   → 3+2+2+5 = 12 passed, 0 failed; дерево чисто
```

### Состояние мира

```
$ ssh … 'docker inspect … | grep GATEWAY_'
GATEWAY_BANDS=0.001
$ df -h /  →  84 % (порог уборки 85 %, gc в close-out)
$ git merge-tree --write-tree origin/main HEAD  →  exit=0, конфликтов нет
```

## Примечания — НЕ блокируют, заводятся картой долга

**1. Окно не объявлено в кадре (`§4bis(2)`).** Клиент выводил охват карты из своих `bands`;
после расцепления он этого сделать не может, а в кадре окна нет. Пока значение равно
сегодняшнему эффективному (`0.001`), потеря не материализуется — но воспроизвести серию по
ОДНОМУ ответу нельзя, нужен серверный конфиг. Это потеря наблюдаемости класса `PL-I-7`.

Долг назван architect'ом ЗАРАНЕЕ, а не найден мной постфактум, и принят сознательно по
названному основанию: объявить = сменить форму кадра = бампнуть `GATEWAY_SCHEMA_VERSION`, а
бамп зарезервирован за пирамидой. Носитель — спека пирамиды
(`docs/plans/depth-delivery-architecture-2026-08-31.md` §6 п.3), не M-75. Завожу картой,
чтобы долг был виден в списке, а не подразумевался милестоуном.

**2. Оракулы `crates/gateway` живут на процессном ДЕФОЛТЕ.** `red_heatmap_window_decoupled.rs`
сеттер не зовёт вовсе (`grep -c` → 0). Сегодня это верно: каждый тестовый файл в Rust — свой
бинарь, внутри файла окно никто не меняет. Но свойство держится на структуре теста, а не на
страже: тест, добавленный в ЭТОТ файл и дёрнувший сеттер, тихо испортит соседей. Гигиена
`serial()` применена задачей 2b к четырём чужим оракулам, здесь её нет за ненадобностью.
Не блокер и не долг — отмечаю как наблюдение для следующего, кто будет править этот файл.

## Условие APPROVED — исполняется мной же, немедленно

1. вердикт закоммичен на ветку ДО merge'а (этот файл);
2. merge через PR с зелёным `All checks passed`;
3. `PROJECT-STATE.md` + `TECH-DEBT.md` — после merge'а;
4. деплой-гейт `gates.md` §8: CI+Deploy до терминального статуса, ssh-проверка контейнеров и
   heartbeat'а, **sanity свежих WS-кадров** — деплой меняет путь построения выдачи, а
   liveness-проверки верны и при испорченном содержимом;
5. `bash scripts/gc_worktrees.sh` — диск 84 %.
