<!-- GATE-META
milestone: M-90
audited_repo: a3ka/hft-platform
audited_base: 009d82a71377877d0b9ab239790f0e6a3c28a1b8
audited_head: e128ee723c7ed0671926976e1a6032e2dee035f3
verdict: DECISION
-->

# A-043 — M-90: идентичность шага CI для карты исключений гейта; RED на каденцию; маршрут после двух REJECT

**Созван:** `gates.md` §0 п.1 — `C-265` F2 и `C-266` F2: два REJECT подряд по одной причине
(карта CI-паритета `scripts/verify_M-90.sh`). Арбитр — свежий контекст, сильная модель; ведущий
architect — сторона спора, его правки после `C-266` (`f9643f8`, `f5ae6a5`, `cfda760`, `e128ee7`)
судятся как предмет, а не принимаются по его словам.

**Предмет — ветка** `origin/feat/M-90-warmer-selector-single-source`; вершина взята командой
(`e128ee7`), справка мандата `e128ee7` с ней совпала, `6936f0b` (предмет `C-266`) — её предок.
Повторный `fetch` перед записью — дрейфа нет (§7). Собственное detached-дерево
`/tmp/hft-arbiter-m90`, временные пробы построены в scratchpad и в дереве не оставлены.

Живые инварианты предмета: `VB-I-11` (`docs/fa/viz-backend.md`: писатель и читатель слепка не
смеют молча пользоваться разной историей) и `OPS-I-8` (`docs/fa/ops.md`: жив, но не работает —
инцидент). Диапазон ветки `crates/**` не трогает (три файла architect'а + вердикты), поэтому
`check_review_fa.sh` для этого коммита даст `SKIP`; инварианты названы по существу, а не для барьера.

---

## 1. Решение — коротко

1. **Идентичность шага CI для карты исключений — отпечаток ПОЛНОГО текста `run:` (sha256),
   БЕЗ джоба и БЕЗ имени шага.** Единственная нормализация — строка условия агрегата
   `status-check` строгой формы. Форма, приземлённая architect'ом в `f5ae6a5`, принимается:
   атака `C-266` F2 воспроизведена и даёт `FAIL` (§2.1); восемь дополнительных миров обхода не
   нашли ни одного пути, по которому ИЗМЕНЕНИЕ текста исключённого шага проходит молча (§2.2).
   Требование «джоб + имя шага + текст» из `C-266` отклоняется — измерением показано, что оно не
   закрывает ни одного обхода сверх отпечатка, а ложные «протухла» на переносе плумбинга
   между джобами и на правке прозы имени создало бы (§2.3).
   **`C-265` F2 этой формой разряжается — но не целиком:** тот же класс «ключ по первой строке»
   уцелел в ДРУГОМ месте того же скрипта — `SKIP` для `check_review_fa.sh` сравнивает `$first`,
   и строка, дописанная в этот шаг, исполнения не получает (§5 N-1, измерено: `exit=0`,
   `evil.sh` не упомянут). Это ОБЯЗАТЕЛЬНАЯ правка К-1 (§2.4); без неё карта «точной» не является.
2. **RED на `CHECKPOINT_DEPTH_CADENCE_MS` обязателен — и он выполнен.** Мутант «отказ на пять,
   своя копия каденции в argv» роняет `w3` с именем переменной; эталон «отказ на шесть» — `w3`
   зелен. Снято мной, не по словам architect'а (§3). `C-266` F1 — закрыт на `f9643f8`.
3. **Разногласия по существу между architect'ом и critic'ом НЕТ.** Два REJECT по F2 — два
   законных обнаружения НЕДОДЕЛАННОЙ правки (пропуск всех многострочных ⇒ ключ по первой
   строке ⇒ отпечаток полного текста), а не спор о форме; `C-266` сам передал выбор формы
   арбитру, architect выбрал до арбитража — выбор верный. Третий круг architect↔critic по
   ТОЙ ЖЕ причине невозможен: форма решена здесь (§6, правило заморозки).
4. **Маршрут (§4): dev — НЕМЕДЛЕННО, по RED-набору как он есть на `e128ee7`; architect —
   ПАРАЛЛЕЛЬНО правит гейт по К-1…К-5 (только `scripts/verify_M-90.sh`,
   `scripts/tests/red_verify_M-90_ci_map.sh`, `milestones/M-90-*.md` §9); критик круга 3 —
   по ЗАМОРОЖЕННОМУ перечню §6, его вердикт — предусловие `tester`, не `dev`.** Основание —
   прецедент `A-035` §1 (ремонт гейт-скрипта не меняет контракт dev'а) и цена: прод-выдача
   держится на строке в `/etc/cron.d`, которую стирает первый кодовый деплой (`R-211` §5.3/§10:
   восемь суток `not_ready` уже оплачены).
5. **Новые находки — только по свежему предмету:** N-1 (блокирующая, в К-1), N-2 и N-3
   (пределы разбора и переноса шага — К-2/К-4), N-4 (литерал «3 мира» в метке шага гейта —
   К-5). Ничего в `red_m90_warmer_cron_composition.rs` менять не требуется.

---

## 2. Пункт 1 — идентичность шага CI

### 2.1. Атака `C-266` F2 воспроизведена руками — `FAIL`

Строка `bash scripts/tests/red_artifact_ids.sh --battery`, дописанная в блок «база события»
после `echo "sha=${raw}" >> "$GITHUB_OUTPUT"` (копия `ci.yml` в scratchpad, якорь
единственный):

```text
$ VERIFY_M90_CI_DRY=1 VERIFY_M90_CI_FILE=$S/atk_base_append.yml bash scripts/verify_M-90.sh | grep -E '^(FAIL|VERDICT)'
FAIL  ci-parity: исключение [ddd7a1d0ce254659] шаг «база события» (12 строк, пишет sha в GITHUB_OUTPUT); … не совпало ни с одним шагом CI — карта протухла (шаг изменён или удалён)
VERDICT: FAIL (провалов: 1)
attack_exit=1
```

Механизм двусторонний: изменённый блок не совпал с ключом ⇒ (а) запись карты «протухла» —
`FAIL`; (б) сам блок ушёл на исполнение (в не-DRY режиме `bash -ec` на нём падает ещё и по
`EVENT_NAME` — второй `FAIL`). Проба `red_verify_M-90_ci_map.sh` на вершине — 7/7; тот же
набор против `verify_M-90.sh` ревизии `f5ae6a5~1` — три новых мира `FAIL` (§7): проба
красна против прежнего скрипта, зелена против нового — анти-плацебо в обе стороны состоялось.

### 2.2. Поиск обхода — девять миров сверх пробы (все `VERIFY_M90_CI_DRY=1`, копии `ci.yml`)

| мир | что изменено | исход | оценка |
|---|---|---|---|
| A | шаг `check_review_fa.sh` превращён в блок с дописанной `bash scripts/evil.sh` | `exit=0`, `SKIP check_review_fa…`, `evil` не упомянут | **ОБХОД** — ключ по первой строке в `verify_M-90.sh:110` (§5 N-1, К-1) |
| B | шаг `git fetch … refs/salvage/*` перенесён из `gate-meta` в `build-test` | `exit=0`, исключено 6 | законно: плумбинг плумбингом и остаётся; идентичность джоба ничего не добавила бы |
| C | `pip install --quiet jsonschema` продублирован во второй джоб | `exit=0`, «учтено 56 из 56, исключено по карте 6» | оба исключены; счётчик считает КЛЮЧИ, а не шаги (N-4, косметика) |
| D1 | `if: false` на `cargo test --all` | невидимо, шаг исполняется локально | направление безопасное: гейт СТРОЖЕ CI; предел назвать (К-4) |
| D2 | `env: HFT_DELIVERY_DEEP: "1"` снят с `delivery` | невидимо | **и без правки** гейт исполняет `verify_delivery_M-08.sh` БЕЗ `DEEP` ⇒ локально `SKIP D1-deep`; это единственный шаг, где локальная форма мягче CI — предел назвать явно (К-4) |
| E1 | `continue-on-error: true` на пробе | невидимо | направление безопасное (гейт строже) |
| E2 | `secret-material` выкинут из условия агрегата | карта `exit=0` (нормализация строгой формы) | **по замыслу не её предмет**: `CI_AGG_YML=$S/E_cond_drop_job.yml bash scripts/tests/red_ci_aggregate.sh` ⇒ `VERDICT: FAIL — сценариев: 8, расхождений: 2`, exit=1; эта проба исполняется гейтом как шаг CI (`EXEC`), запись карты на это и ссылается |
| F1 | `run: >` (folded) вместо `run:` | DRY: «исполнилось бы «>»», содержимое блока отброшено | в не-DRY `bash -ec '>'` ⇒ exit=2 ⇒ `FAIL` — fail-closed СЛУЧАЙНО, по синтаксической ошибке, с ложной причиной (§5 N-2, К-2) |
| F2 | `run: \|-` (chomp) на блоке `red_artifact_ids.sh` | DRY: «исполнилось бы «\|-»», строки блока потеряны | то же: `bash -ec '\|-'` ⇒ exit=2, но `red_artifact_ids.sh` локально НЕ исполнится (К-2) |
| H | исключённый однострочный `pip install …` превращён в блок с дописанной командой | `exit=1`, «карта протухла» | закрыт тем же отпечатком |

Итог: ни одно ИЗМЕНЕНИЕ ТЕКСТА `run:` исключённого шага молча не проходит (A — не карта, а
соседняя первая-строчная проверка; H, атака §2.1 — `FAIL`). Невидимы изменения ВНЕ `run:`
(`if:`/`env:`/`continue-on-error:`), и это предел, который надо НАЗВАТЬ, а не закрыть: все они,
кроме D2, в безопасном направлении (локальный гейт исполняет больше, чем CI).

### 2.3. Почему не «джоб + имя шага + текст»

- Имя шага у агрегата отсутствует (`- run: |`, `ci.yml:618`), у остальных — свободная проза,
  повторяющаяся («Проба барьера (счёт печатает сама проба)» встречается не раз). Как ключ
  оно не уникально и не стабильно: правка прозы роняла бы гейт «протухла».
- Идентичность джоба ничего не ловит сверх отпечатка (миры B, C): исключаемы ТОЛЬКО шаги,
  чей смысл задан содержимым (установка инструмента, плумбинг рефов, база события, агрегат);
  переезд такого шага между джобами не меняет его плумбинговости. Зато ложные `FAIL` на
  рефакторинге джобов подрывают доверие к гейту, который dev и tester обязаны гонять.
- Опасный класс — «в исключённый блок дописали работу» — закрыт отпечатком полного текста
  целиком (§2.1, H). Опасный класс «работа спрятана в строку условия агрегата» закрыт строгой
  формой `AGG_RE` (мир `agg_inject` пробы). Класс «джоб выпал из условия» — не предмет карты и
  закрыт исполняемой пробой агрегата (E2).

### 2.4. Обязательные правки architect'а — К-набор (зона: только три файла)

| # | файл | что | предъявление |
|---|---|---|---|
| К-1 | `scripts/verify_M-90.sh` | ВСЕ сравнения, открывающие `SKIP`/особую ветку шага, — по ПОЛНОМУ тексту `"$step"`, а не `"$first"` (`:107`, `:110`): блок, начинающийся с `check_review_fa.sh` и несущий ещё строку, идёт на исполнение | проба, мир (i) |
| К-2 | `scripts/verify_M-90.sh` `ci_steps` | `run: \|`, `\|-`, `\|+` — блок (чомпинг на отпечаток не влияет: хвостовые пустые строки и так отброшены); `run: >`, `>-`, `>+` — явный `FAIL` «блочный скаляр `>` не поддержан: складывание строк меняет команду» вместо исполнения `>` как команды | проба, миры (ii), (iii) |
| К-3 | `scripts/tests/red_verify_M-90_ci_map.sh` | +3 мира с setup-стражем: (i) блок `check_review_fa.sh` + дописанная строка ⇒ `exit=0` И строка «исполнилось бы «bash scripts/check_review_fa.sh»» присутствует (на нынешнем скрипте её нет — проба красна); (ii) `run: \|-` на блоке `red_artifact_ids.sh` ⇒ `exit=0` И «исполнилось бы «bash scripts/tests/red_artifact_ids.sh»»; (iii) `run: >` ⇒ `exit=1` И строка про блочный скаляр. Прежние 7 — без изменений | `VERDICT: PASS — 10 сценариев`; тот же набор против скрипта `e128ee7` — ровно 3 `FAIL` |
| К-4 | `milestones/M-90-*.md` §9 | пределы карты — текстом: (а) ключ — отпечаток ПОЛНОГО текста `run:`; джоб и имя шага в ключ не входят — по этому решению; (б) карта судит ТОЛЬКО текст `run:`: `if:`/`env:`/`continue-on-error:` шага не переносятся; единственный CI-only `env:` сегодня — `HFT_DELIVERY_DEEP=1` (сборка образа — CI и §8), локальный гейт исполняет мелкую форму `verify_delivery_M-08.sh`; (в) дрейф `needs`/условия агрегата — предмет исполняемой `red_ci_aggregate.sh`, не карты; (г) отпечаток непрозрачен — текст причины обязан описывать шаг так, чтобы reviewer сверил его глазами (число строк у блоков) | текст §9 + ссылка на `A-043` |
| К-5 | `scripts/verify_M-90.sh:39` | метка «3 мира» — литерал, живущий отдельно от предмета, уже лжёт (миров 7, будет 10): число из метки убрать, счёт печатает проба | `grep -c 'мира' scripts/verify_M-90.sh` ⇒ 0 в метке |

Правка RED-файла `crates/gateway/tests/red_m90_warmer_cron_composition.rs` в К-набор НЕ
входит и dev'у нужна как есть.

---

## 3. Пункт 2 — RED на `CHECKPOINT_DEPTH_CADENCE_MS`

`selector_fingerprint` (`crates/gateway/src/lib.rs:4252-4274`, открыт) хеширует
`depth_cadence_ms`; `I-1bis`/`w4e` требует брать каденцию из `GATEWAY_DEPTH_CADENCE_MS`.
Реализация «берём `GATEWAY_*`, но при наличии `CHECKPOINT_DEPTH_CADENCE_MS` в cron'е — её»
проходит `w1`…`w4e` и возвращает класс `TD-227` на оси, меняющей имя слепка. Значит отказ
на шестую переменную — не «для симметрии», а закрытие реального пути. Мутационный контроль —
временные правки `deploy/bin/gateway-checkpoint-cron.sh` в моём дереве, возвращены
(`cmp … && echo identical` ⇒ `identical`, `git status --porcelain` пуст):

```text
== REFERENCE (отказ на шесть) — w3 ==
test w3_script_refuses_own_selector_copy_and_names_it ... ok
test result: ok. 1 passed; 0 failed; …
== MUTANT (отказ на пять, своя копия каденции `--depth-cadence-ms "${CHECKPOINT_DEPTH_CADENCE_MS:-1000}"`) — w3 ==
test w3_script_refuses_own_selector_copy_and_names_it ... FAILED
TD-227: при `CHECKPOINT_DEPTH_CADENCE_MS` в окружении cron'а скрипт позвал прогреватель (Some(["compose", "run", "--rm", "gateway-checkpoint", …
test result: FAILED. 0 passed; 1 failed; …
```

`w3` на вершине красен поведенчески (runner позван при `CHECKPOINT_VENUE`), `w4e` зелен как
сторож (каденция уже идёт через `environment:` compose) — состояние набора 2 passed / 6 failed
совпадает с `C-266` §«C-265-F1 disposition». **`C-266` F1 закрыт**, дополнительных RED не нужно.

---

## 4. Пункт 3 — природа спора и маршрут

**Природа.** Круг 1: гейт пропускал все многострочные шаги. Круг 2: ключ — первая строка.
Оба — верные находки одного класса «карта не точна», оба исправлены architect'ом в
заявленную сторону; critic не оспаривал направление, он нашёл остаток. Это незавершённая
правка, не разногласие. Арбитраж тем не менее стоит цены: класс «ключ по первой строке»
живёт в скрипте ЕЩЁ в одном месте (N-1), и без решения о форме круг 3 нашёл бы его как
«третий REJECT по той же причине».

**Маршрут — обязателен обеим сторонам:**

| шаг | кто | что | когда |
|---|---|---|---|
| 1 | founder | диспетчеризует `engine-dev` на `feat/M-90-warmer-selector-single-source` (вершина берётся командой; на момент решения `e128ee7`) — задачи 1–3 спеки §7 по §3/§4/§6 и RED `w1`…`w4e` КАК ЕСТЬ | немедленно |
| 2 | architect, ПАРАЛЛЕЛЬНО | К-1…К-5 (§2.4) — ТОЛЬКО `scripts/verify_M-90.sh`, `scripts/tests/red_verify_M-90_ci_map.sh`, `milestones/M-90-*.md`; §12 спеки — строка круга 3 со ссылкой сюда; явные пути в коммите; перед push — `git pull --rebase`; push сопровождается сообщением с перечнем файлов (`branch-hygiene.md` п.10) | до диспетчеризации критика круга 3 |
| 3 | founder → critic | круг 3 на СВЕЖЕМ контексте (критик, уже забутстрапленный под `C-265`/`C-266`, годится — модель отлична от обеих сторон) — по перечню §6 на вершине ветки после шага 2; вердикт — файл `C-NNN` на ветке | параллельно dev'у; **вердикт — предусловие `tester`, не `dev`** |
| 4 | tester → reviewer | обычная цепочка; reviewer дополнительно судит К-набор как адверсарий харнесса на PR-гейте; задача 4 (§8-гейт) — reviewer после деплоя; `TD-227` закрывает reviewer | PR-time |

Две роли на одной ветке с непересекающимися путями (`crates/gateway/src/bin/**`,
`deploy/bin/**`, `docker-compose.yml`, `deploy/README.md` у dev; три файла §2.4 у architect).
Пересечение — `SCOPE VIOLATION`, не молчаливый merge. Если критик круга 3 находит дефект в
RED-наборе ПО СВЕЖЕМУ предмету — это обычный `REJECT` architect'у с именем пункта, dev
перепрогоняет; это не повод остановить dev заранее.

**Что этим НЕ отменено:** вердикты `C-265`/`C-266` исполняются, не отклоняются;
UNCONDITIONAL reviewer; запрет dev'у трогать `*/tests/**` и `scripts/verify_*`; граница C —
состав полос не меняется (`П-014` п.4, `П-029`).

**Риск, названный, а не решённый:** до merge `M-90` каждый деплой, прошедший фильтр `paths`
`deploy.yml`, стирает временную строку `CHECKPOINT_BANDS` (`R-211` §5.3). Повторять ручной шаг
после такого деплоя — операторское действие founder'а (решение 2026-10-01), не предмет этого
вердикта.

---

## 5. Пункт 4 — новые находки по свежему предмету

- **N-1 (блокирующая, входит в К-1).** `verify_M-90.sh:110`: `[ "$first" = "bash scripts/check_review_fa.sh" ] && [ -z "$have_review" ]` ⇒ `SKIP` по ПЕРВОЙ строке. Мир A: блок `check_review_fa.sh` + `bash scripts/evil.sh` ⇒ `exit=0`, работа не исполнена и не названа. Тот же класс, что `C-266` F2, в соседней строке. На ветке `R-NNN` по `M-90` нет, поэтому ветка `SKIP` активна ровно сейчас. Строка `:107` (`cargo audit`) — тот же шаблон, безопасный только потому, что его исход `FAIL`; привести к полному тексту для единообразия.
- **N-2 (предел разбора, К-2).** `ci_steps` знает только `run: |` и однострочную форму; `|-`/`|+`/`>` попадают в однострочную ветку как команды `|-`/`>`, содержимое блока теряется. Исход `FAIL` — случайный (синтаксическая ошибка bash), причина в выводе ложная. В `ci.yml` сегодня таких форм нет (`grep -cE 'run: *[|>][+-]' .github/workflows/ci.yml` ⇒ 0), поэтому не обход, а хрупкость.
- **N-3 (предел переноса, К-4).** Карта судит только текст `run:`; `env:` шага не переносится. Единственный CI-only `env:` — `HFT_DELIVERY_DEEP: "1"` на `verify_delivery_M-08.sh` (`:20` — `DEEP=0` ⇒ `SKIP D1-deep`): локальный гейт исполняет мягкую форму. Закрывать переносом `env:` не требую — глубокая форма строит докер-образ и по спеке живёт в CI и §8; требую НАЗВАТЬ.
- **N-4 (косметика, К-5 + сообщение).** `verify_M-90.sh:39` метка «3 мира» при 7 мирах; «исключено по карте N» считает ключи, не шаги (мир C: 56 шагов, 6 ключей, 7 исключённых шагов). Литерал, живущий отдельно от предмета, — известный класс этого репозитория (`ci.yml:429-430`).

---

## 6. Правило заморозки — что проверяет критик круга 3, чтобы не было четвёртого

Решены здесь и повторно НЕ обсуждаются (спор о них в круге 3 — основание для повторного
арбитража по `gates.md` §0 п.4, не для `REJECT`): (а) форма идентичности шага — отпечаток
полного текста `run:` без джоба/имени; (б) достаточность RED на каденцию (`w3`, шесть
переменных); (в) маршрут §4 — dev параллельно К-набору. Критик проверяет РОВНО:

1. К-1: в `verify_M-90.sh` нет ни одного сравнения `"$first" = …`, открывающего `SKIP`/обход; мир (i) пробы зелен, и ТОТ ЖЕ мир против скрипта `e128ee7` красен (`git show e128ee7:scripts/verify_M-90.sh`).
2. К-2/К-3: миры (ii)/(iii) зелены; проба печатает `10 сценариев`; прежние 7 без изменений; `bash scripts/tests/red_verify_M-90_ci_map.sh` ⇒ exit=0; тот же набор против `e128ee7` ⇒ ровно 3 `FAIL`.
3. К-4: §9 спеки несёт четыре предела (а)–(г) текстом и ссылку `A-043`; §12 — строка круга 3.
4. К-5: число миров в метке шага гейта отсутствует.
5. Диапазон ветки после `e128ee7` со стороны architect'а трогает ТОЛЬКО три файла §2.4 (и, при наличии, свои вердикт-файлы); `red_m90_warmer_cron_composition.rs` не изменён (`git diff e128ee7 -- crates/gateway/tests/` пуст); `w1`/`w3`/`w4a`…`w4d` по-прежнему красны по своей причине, `w2`/`w4e` зелены — пока dev не влил GREEN; после GREEN dev'а — все 8 зелены.
6. Ничего из закрытого `C-265` F1, `C-266` F1 не переоткрыто прогоном.

Пункты 1–6 выполнены ⇒ `NOTE` или чистый пропуск к `tester`; нет — `REJECT` с указанием
ПУНКТА. Новые находки вне перечня допустимы только по свежему предмету (не по форме ключа,
не по каденции, не по маршруту).

---

## 7. Done Block (сырой stdout; дерево `/tmp/hft-arbiter-m90`, detached `e128ee7`)

```text
$ git fetch origin && git rev-parse origin/feat/M-90-warmer-selector-single-source
e128ee723c7ed0671926976e1a6032e2dee035f3
$ git merge-base --is-ancestor e128ee7 origin/feat/M-90-warmer-selector-single-source && echo YES
e128ee7 is ancestor: YES
$ git merge-base --is-ancestor 6936f0b origin/feat/M-90-warmer-selector-single-source && echo YES
6936f0b ancestor: YES
$ git merge-base origin/main origin/feat/M-90-warmer-selector-single-source
009d82a71377877d0b9ab239790f0e6a3c28a1b8
$ git log --oneline 6936f0b~1..origin/feat/M-90-warmer-selector-single-source
e128ee7 docs(M-90): спека — шесть переменных отказа, карта CI по полному шагу, журнал кругов C-266 → арбитр [architect]
cfda760 test(M-90): red_verify_M-90_ci_map.sh — 7 миров … (C-266 F2) [architect]
f5ae6a5 fix(M-90): verify — исключение карты CI по отпечатку ПОЛНОГО шага … (C-266 F2) [architect]
f9643f8 test(M-90): w3 — отказ и на CHECKPOINT_DEPTH_CADENCE_MS … (C-266 F1) [architect]
a1b56de docs(M-90): C-266 — reject round-2 warmer selector gate [critic]
6936f0b docs(M-90): спека круг 2 — I-1bis, w4a-w4e, точная карта CI, журнал кругов (C-265 REJECT) [architect]

$ bash scripts/tests/red_verify_M-90_ci_map.sh            # вершина
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
PASS  строка дописана в блок базы ⇒ FAIL (C-266 F2) (exit=1)
PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
VERDICT: PASS — 7 сценариев
probe_exit=0

$ git show f5ae6a5~1:scripts/verify_M-90.sh > scripts/verify_M-90.sh; bash scripts/tests/red_verify_M-90_ci_map.sh; git checkout -- scripts/verify_M-90.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
FAIL  строка дописана в блок базы ⇒ FAIL (C-266 F2): exit=0 (ожидался 1) или нет строки «карта протухла»
FAIL  строка дописана в блок агрегата ⇒ FAIL: exit=0 (ожидался 1) или нет строки «карта протухла»
PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
FAIL  агрегат: команда в строке условия ⇒ FAIL: exit=0 (ожидался 1) или нет строки «карта протухла»
VERDICT: FAIL (провалов: 3)
old_verify_probe_exit=1
restored: yes

$ VERIFY_M90_CI_DRY=1 VERIFY_M90_CI_FILE=$S/atk_base_append.yml bash scripts/verify_M-90.sh | grep -E '^(FAIL|VERDICT)'
FAIL  ci-parity: исключение [ddd7a1d0ce254659] шаг «база события» (12 строк, пишет sha в GITHUB_OUTPUT); … — карта протухла (шаг изменён или удалён)
VERDICT: FAIL (провалов: 1)
attack_exit=1

$ for w in A_review_fa_append B_moved_step C_dup_text D_if_false D_env_dropped E_continue_on_error E_cond_drop_job F_folded F_chomp; do VERIFY_M90_CI_DRY=1 VERIFY_M90_CI_FILE=$S/$w.yml bash scripts/verify_M-90.sh; echo "== $w: exit=$?"; done   # setup-страж cmp на каждом — все изменены
== A_review_fa_append: exit=0
SKIP  ci-parity: check_review_fa — вердикта R-NNN по M-90 в диапазоне ещё нет (зеленеет на PR-гейте)
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
VERDICT: PASS                                   ← «evil» в выводе отсутствует: ОБХОД (N-1)
== B_moved_step: exit=0        … учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
== C_dup_text: exit=0          … учтено шагов 56 из 56 (исполнено 48, исключено по карте 6)
== D_if_false: exit=0          … 55 из 55
== D_env_dropped: exit=0       … 55 из 55
== E_continue_on_error: exit=0 … 55 из 55
== E_cond_drop_job: exit=0     … 55 из 55
== F_folded: exit=0            PASS  ci-parity(dry): исполнилось бы «>»
== F_chomp: exit=0             PASS  ci-parity(dry): исполнилось бы «|-»   (строки блока red_artifact_ids.sh в выводе нет)

$ VERIFY_M90_CI_DRY=1 VERIFY_M90_CI_FILE=$S/H_mapped_single_to_block.yml bash scripts/verify_M-90.sh | grep -E '^(FAIL|VERDICT)'
FAIL  ci-parity: исключение [0ada681c1a994d65] «pip install --quiet jsonschema» — установка инструмента не совпало ни с одним шагом CI — карта протухла …
VERDICT: FAIL (провалов: 1)
H exit=1

$ CI_AGG_YML=$S/E_cond_drop_job.yml bash scripts/tests/red_ci_aggregate.sh | tail -1
VERDICT: FAIL — сценариев: 8, расхождений: 2
agg_exit=1

$ bash -ec '>'; echo "exit=$?"; bash -ec '|-'; echo "exit=$?"
bash: -c: line 1: syntax error near unexpected token `newline'
exit=2
bash: -c: line 1: syntax error near unexpected token `|'
exit=2

$ grep -n 'HFT_DELIVERY_DEEP' scripts/verify_delivery_M-08.sh | head -2
20:DEEP="${HFT_DELIVERY_DEEP:-0}"   # 1 = реально собрать образ и ЗАПУСТИТЬ бинарь (CI-job, §8)
58:  echo "SKIP  D1-deep (сборка образа) — включается HFT_DELIVERY_DEEP=1 (CI-job + §8 на VPS)"

$ cargo test -p gateway --test red_m90_warmer_cron_composition        # базовая линия вершины
test w3_script_refuses_own_selector_copy_and_names_it ... FAILED
test w2_default_dotenv_snapshot_is_found_positive_control ... ok
test w4b_symbol_axis_single_source ... FAILED
test w1_cron_warmer_snapshot_is_found_by_server_with_prod_dotenv ... FAILED
test w4e_depth_cadence_axis_single_source ... ok
test w4c_timeframe_axis_single_source ... FAILED
test w4a_venue_axis_single_source ... FAILED
test w4d_window_axis_single_source ... FAILED
TD-227: при `CHECKPOINT_VENUE` в окружении cron'а скрипт позвал прогреватель (Some(["compose", "run", "--rm", "gateway-che…
test result: FAILED. 2 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
exit=101

$ # мутационный контроль F1 — временные правки deploy/bin/gateway-checkpoint-cron.sh, возвращены
== REFERENCE (refuse six) — w3 ==
test w3_script_refuses_own_selector_copy_and_names_it ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.04s
exit=0
== MUTANT (refuse five, own cadence copy) — w3 ==
98:  --depth-cadence-ms "${CHECKPOINT_DEPTH_CADENCE_MS:-1000}"
test w3_script_refuses_own_selector_copy_and_names_it ... FAILED
TD-227: при `CHECKPOINT_DEPTH_CADENCE_MS` в окружении cron'а скрипт позвал прогреватель (Some(["compose", "run", "--rm", "gateway-checkpoint", "--dir", "/journal", "--ckpt-dir", "/ck…
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.04s
exit=101
restored:
identical

$ sed -n '4252p;4273p' crates/gateway/src/lib.rs
    pub fn selector_fingerprint(sel: &Selector) -> u64 {
        sel.depth_cadence_ms.hash(&mut h);

$ bash scripts/check_branch_health.sh | grep -E 'M-90|ВИСЯК|ДУБЛЬ|VERDICT'
feat/M-90-warmer-selector-single-source      11     15      6 —        0 сут
ok    ВИСЯК: зелёных PR без merge'а старше 1 сут не найдено
ok    ДУБЛЬ: ни один предмет не живёт больше чем на одной ветке
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
bh_exit=0

$ bash scripts/next_artifact_id.sh A; bash scripts/reserve_artifact_id.sh A
A-043
reserve: попытка 1/8 — A-043 ← 677795b34652d36cab605196a00a5ce36d29b5f3
reserve: резерв A-043 взят; снять после приземления носителя: bash scripts/reserve_artifact_id.sh --release A-043
$ git ls-remote origin 'refs/reserved/A-04*'
677795b34652d36cab605196a00a5ce36d29b5f3	refs/reserved/A-043

$ git fetch origin && git rev-parse origin/feat/M-90-warmer-selector-single-source   # перед записью, 2026-10-01T18:38Z
e128ee723c7ed0671926976e1a6032e2dee035f3
drift=none
```

Опора, открытая целиком: `C-265`, `C-266`, `milestones/M-90-*.md`,
`red_m90_warmer_cron_composition.rs`, `scripts/verify_M-90.sh`, `red_verify_M-90_ci_map.sh`,
`.github/workflows/ci.yml`, `deploy/bin/gateway-checkpoint-cron.sh`, compose `gateway-serve`/
`gateway-checkpoint`, `lib.rs:4245-4280`; `R-211` §5.3/§10 — из `origin/main` (на ветке файла нет:
база ветки старше его merge'а); прецеденты маршрута — `A-035` §1, `A-042` §1/§7.

=== HANDOFF: ARBITER → FOUNDER ===

## §A — Метаданные
- Дата (UTC): 2026-10-01T18:40Z
- Milestone: M-90-warmer-selector-single-source
- Статус: DECISION — dev разблокирован; К-набор architect'у параллельно; критик круга 3 по §6 — предусловие tester
- HEAD: e128ee7 — спека: шесть переменных отказа, карта CI по полному шагу (судимая вершина)

## §B — Что я сделал
- Воспроизвёл атаку `C-266` F2 (FAIL), прогнал 9 миров обхода + 1 (§2.2, H); нашёл уцелевший ключ-по-первой-строке (N-1).
- Снял мутационный контроль F1 сам: эталон `w3` ok, мутант «своя каденция» — FAILED с именем переменной.
- Решил форму идентичности, достаточность RED, маршрут; выписал К-набор и правило заморозки.

## §C — Артефакты / результаты
- `research/arbitration/A-043-m90-ci-step-identity.md` — на ветке предмета.
- Done Block — §7 выше (exit-коды при каждом утверждении).

## §D — Следующий агент + инвокация
1. **`engine-dev`** (немедленно):
   ```
   M-90 на origin/feat/M-90-warmer-selector-single-source (вершину возьми: git fetch origin && git rev-parse …; на момент решения e128ee7). Задачи 1–3 спеки §7 по §3 (рекомендуемая форма) и §4 (запрещено); RED — crates/gateway/tests/red_m90_warmer_cron_composition.rs (w1…w4e) как есть, sacred. Зона: deploy/bin/gateway-checkpoint-cron.sh, crates/gateway/src/bin/gateway-checkpoint.rs, docker-compose.yml (сервис gateway-checkpoint), deploy/README.md. Architect параллельно правит scripts/verify_M-90.sh, scripts/tests/red_verify_M-90_ci_map.sh, milestones/M-90-*.md — не трогай; перед push git pull --rebase, в сообщении перечисли файлы. Done Block: cargo test -p gateway --test red_m90_warmer_cron_composition (8 passed), red_checkpoint_bin_prod_argv зелен, bash scripts/verify_M-48.sh, bash scripts/verify_M-90.sh (по финальной форме architect'а). Решение арбитра — research/arbitration/A-043-m90-ci-step-identity.md §4.
   ```
2. **`architect`** (параллельно): К-1…К-5 по `A-043` §2.4; §12 спеки — круг 3 со ссылкой; push ветки; затем Handoff критику круга 3 с перечнем §6.
3. **`critic`** (после п.2, свежий контекст): перечень `A-043` §6, вердикт `C-NNN` на ветке; `NOTE`/пропуск ⇒ tester после GREEN dev'а.
- Push-статус: ⏸ вердикт коммитится на ветку в этом же ответе и пушится `git push origin HEAD:feat/M-90-warmer-selector-single-source`; в `main` ничего не идёт.
- Кэш: ✅ `rm -rf /tmp/hft-arbiter-m90/target` + `git worktree remove` после push.

## §E — Риски / открытые вопросы
- До merge `M-90` каждый кодовый деплой стирает временную строку `CHECKPOINT_BANDS` в `/etc/cron.d` (`R-211` §5.3) — операторское повторение шага после такого деплоя остаётся за founder'ом.
- `HFT_DELIVERY_DEEP` локально не исполняется (N-3) — назвать, не чинить в этом милестоуне.
- `next_artifact_id.sh` сегодня работает > 60 с (user 14 с / sys 22 с на вызов) — наблюдение, не предмет.

=== END HANDOFF ===
