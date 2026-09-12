<!-- GATE-META
milestone: M-84
audited_repo: a3ka/hft-platform
audited_base: 4885e6defffadb3e88eb53ad474ebf149b6759ff
audited_head: 49d1a90fd88dda58e1752c1d541a6369b2057eb9
verdict: REJECT
-->

# C-222 — M-84, круг 3: REJECT

## Вердикт

**REJECT — engine-dev не назначать.** Полный RED-набор теперь реализуем: точный
кандидат проходит 29/29, а четыре независимые жульнические реализации ловятся. Однако
план сохраняет ложный факт о цене пути и на его основании запрещает dev трогать именно
этот путь. Это не допустимое сокращение метрики: подписанное `П-029` и код на судимой
ревизии оба говорят, что работа равна `уровни × полосы`.

Это третий critic-круг по одному предмету: `C-220` (round 1), `C-221` (round 2) и этот
`C-222` (round 3). Поэтому сработал `gates.md` §0, trigger 2: следующий шаг —
**арбитраж, не круг 4 architect/critic**.

Живой инвариант тронутых `crates/gateway/**` и `crates/gateway-serve/**` —
**VB-I-12** из `docs/fa/viz-backend.md`: сервер владеет ровно семью полосами, выдаёт 14
`(band, side)` строк, а клиентский ключ `bands` явно отвергается без молчаливой
канонизации. Он существует на audited-head как «принято, в коде ещё нет» и именно вокруг
него построены RED. Нахождение ниже не ослабляет этот инвариант.

## Предмет и полнота committed artifact set

Аудирован commit-chain
`4885e6defffadb3e88eb53ad474ebf149b6759ff..49d1a90fd88dda58e1752c1d541a6369b2057eb9`,
а не один milestone. База — verdict `C-221`; `git merge-base --is-ancestor` подтвердил
цепочку. Текущая дельта: три RED-файла, RFC, milestone и acceptance script; весь
M-84-набор включает также RED, заведённые в предыдущем круге.

| Обязательный слой | Результат |
|---|---|
| T-contracts | `crates/contracts/**` в судимом диапазоне не тронут: T1 не меняется. Форма локального gateway subscribe описана в `CT-RFC-09`, не в T1. |
| Trait/signature | Нового trait или публичной T1/T2-сигнатуры нет. План явно назначает единственный источник `gateway::CANONICAL_DEPTH_BANDS`, crate guard и три транспортных рубежа; внутренний `Selector.bands` намеренно остаётся. |
| RED | Шесть файлов: `red_fixed_bands_{canonical,fingerprint,frame,prewarm}.rs` в gateway и `red_fixed_bands_{entrypoint,wire}.rs` в gateway-serve. Они сейчас сознательно COMPILE-RED только по отсутствующей константе. |
| Acceptance | `scripts/verify_M-84.sh` содержит CI-тройку, прямые исполнения шести наборов и именованные оракулы. Решение берётся по rc агрегатора; формы `cmd && echo PASS || echo FAIL` нет. |
| Milestone | `milestones/M-84-fixed-depth-bands.md` назначает owners, Allowed/Forbidden paths, три слоя валидирования, prewarm и два живых блокера task 6. |

`C-221` B-1 снят правильно: provenance доступен только при разборе JSON, поэтому
`parse_selector` судит наличие ключа, а нормализованный `Selector` в session/crate guard
судится только по значению. `C-221` B-2 также снят: legacy bytes кладутся под именем,
выведенным чистым `selector_fingerprint`; `checkpoint::advance_to` остаётся защищённым.
Эти требования одновременно выполнимы.

## C-222-F1 — «цена не множится на число полос» опровергнута судимым кодом и `П-029`

`milestones/M-84-fixed-depth-bands.md:25` объявляет измеренным фактом: «цена такта НЕ
множится на число полос». На той же посылке `:101` запрещает менять `depth_from_book`
ради оптимизации для семи полос.

Это противоречит подписанному founder decision `П-029`:
`docs/PENDING-SIGNATURE.md:1800-1801` фиксирует «цена такта линейна по числу полос:
цикл по порогам на КАЖДЫЙ уровень книги», то есть `уровни × полосы`. Это не спор о
термине «один внешний проход»: на audited-head `depth_from_book` сначала строит
`thresholds` из каждого элемента `bands` (`crates/gateway/src/lib.rs:1195-1201`), затем
для каждого ненулевого уровня (`:1202-1206`) итерирует **все** thresholds (`:1207-1215`).
Следовательно число сравнений и возможных сложений одной стороны растёт как
`nonzero_levels × bands.len()`.

Счётчик `depth_levels_visited` не опровергает этот факт: он увеличивается один раз на
уровень в `:1206` и вообще не измеряет внутренние threshold-итерации. Поэтому ссылки
M-84 на старый комментарий `lib.rs:1388-1391` и `C-156 F1` являются недопустимой
подменой вычислительной цены прокси-счётчиком. Подписанное `П-029` уже устранило эту
неоднозначность точной формулировкой и строками кода.

Последствие существенное: M-84 фиксирует семь полос и тем самым фиксирует полезную,
но не нулевую цену `7 × уровни` на сторону. План не может одновременно называть цену
независящей от числа полос и запрещать измеренную оптимизацию этого пути. Ни один RED
и ни один шаг `verify_M-84.sh` не проверяет это утверждение, так что dev получит
неверный запрет без способа его оспорить в рамках task list.

**Условие снятия:** арбитр должен применить `П-029` к M-84: либо заменить `:25` и `:101`
на точную формулу (один материализационный обход уровней, но `O(levels × 7)` threshold
work) и снять ложный запрет, либо принять отдельное, измеренное определение ресурса с
оракулом, который действительно считает threshold work. Из одного комментария M-68
нельзя вывести более сильную физическую характеристику. До этого engine-dev не должен
получать противоречивое руководство.

## Воспроизведение реализуемости и мутаций

В отдельном временном worktree я внёс ровно обещанную честную реализацию: константу
`[0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60]`, точный `to_bits()` crate guard,
parse-time отказ по наличию `bands`, нормализацию лишь при отсутствии ключа и уже
имеющийся session guard по нормализованному значению. Шесть наборов дали:

| Набор | Честный кандидат | Мутант, который ловит |
|---|---:|---|
| canonical | 9 passed | — |
| fingerprint | 4 passed | D: 1 passed, 3 failed |
| frame | 3 passed | — |
| prewarm | 3 passed | A: 2 passed, 1 failed; D: 1 passed, 2 failed |
| entrypoint | 5 passed | B: 1 passed, 4 failed |
| wire | 5 passed | C: 1 passed, 4 failed |

Итого честный кандидат — **29 passed**. Мутации исполнены, не выведены чтением:

- **A:** удалить `validate_selector(sel)?` из `checkpoint::advance_to`. Падает ровно
  `legacy_selector_is_refused_by_the_normal_checkpoint_api` в prewarm; старый обход
  гварда не пройдёт.
- **B:** убрать parse-time проверку наличия ключа, но молча строить канонический selector.
  Entrypoint ловит чужой, канонический и пустой supplied key, а также silent path.
- **C:** свести `session::validate_selector` к `Ok(())`. Wire ловит все foreign variants,
  сохраняя единственный canonical success.
- **D (новая, не из C-220/C-221):** в `selector_fingerprint` хешировать всегда
  `CANONICAL_DEPTH_BANDS`, игнорируя фактические `sel.bands`. Fingerprint ловит исчезновение
  оси bands, legacy collision и потерю order sensitivity; prewarm ловит исчезновение двух
  разных checkpoint names/coexistence.

Следовательно F1 не повторяет B-1/B-2 и не является заявленным COMPILE-RED: набор
семантики selector/fingerprint/prewarm честно достижим и хорошо различает обходы.

## Gates §9 и границы

Покрытие `gates.md` §9 предъявлено явно, поскольку предмет включает `docs/fa/**` и
`docs/rfc/**`.

1. **(a) Утверждения о коде на merge tree:**
   `bash scripts/verify_design_claims.sh --merge-preview origin/main` завершился
   `VERDICT: PASS (0 нарушений)`, rc 0. Это не снимает F1: script проверяет ссылки и
   declared code claims, но не асимптотику внутреннего цикла.
2. **(б) Полномочия и граница C:** `П-029` — подписанное решение founder, а task 6 всё ещё
   блокируют `TD-159` и барьер восстановления глубины; `docker-compose.yml` остаётся на
   `GATEWAY_BANDS:-0.001`. `crates/contracts/**` не менялся, так что Block-C для T1 не
   включается.
3. **(в) Связность и висячие ссылки:** тот же merge-preview проверил 8 DESIGN section links,
   594 doc-file links и 183 RFC path candidates без нарушения. RFC §2.2bis и VB-I-12
   существуют на audited revision.

Scope чист: `git diff --check` rc 0; текущая дельта не трогает contracts, book, journal,
venue, `GATEWAY_BANDS`, `PROJECT-STATE.md` или `TECH-DEBT.md`. Это не устраняет F1,
потому что именно milestone в разрешенном architect scope утверждает неверный факт.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-222
[exit=0]

$ bash scripts/check_artifact_ids.sh 49d1a90fd88dda58e1752c1d541a6369b2057eb9
OK: ни один коммит диапазона 49d1a90..HEAD не ввёл второй носитель под занятым идентификатором
[exit=0]

$ git merge-base --is-ancestor 4885e6defffadb3e88eb53ad474ebf149b6759ff 49d1a90fd88dda58e1752c1d541a6369b2057eb9
[exit=0]

$ git diff --check 4885e6defffadb3e88eb53ad474ebf149b6759ff..49d1a90fd88dda58e1752c1d541a6369b2057eb9
[exit=0]

$ EVENT_NAME=push PUSH_BEFORE=49d1a90fd88dda58e1752c1d541a6369b2057eb9 bash scripts/check_gate_meta.sh
── GATE-META: диапазон 49d1a90f..HEAD, origin=a3ka/hft-platform
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
[exit=0]

$ git diff --name-only 4885e6d..49d1a90
crates/gateway-serve/tests/red_fixed_bands_wire.rs
crates/gateway/tests/red_fixed_bands_canonical.rs
crates/gateway/tests/red_fixed_bands_prewarm.rs
docs/rfc/CT-RFC-09-ws-session.md
milestones/M-84-fixed-depth-bands.md
scripts/verify_M-84.sh
[exit=0]

$ CARGO_TARGET_DIR=/tmp/hft-critic-M84r3-plan-target bash scripts/verify_M-84.sh
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all
FAIL: cargo test -p gateway --test red_fixed_bands_canonical
FAIL: grep -q 'CANONICAL_DEPTH_BANDS' crates/gateway/src/lib.rs
FAIL: cargo test -p gateway-serve --test red_fixed_bands_wire
FAIL: cargo test -p gateway-serve --test red_fixed_bands_entrypoint
FAIL: cargo test -p gateway --test red_fixed_bands_frame
FAIL: cargo test -p gateway --test red_fixed_bands_fingerprint
FAIL: cargo test -p gateway --test red_fixed_bands_prewarm
VERDICT: FAIL (14)
[exit=1]  # declared COMPILE-RED: absent gateway::CANONICAL_DEPTH_BANDS

$ honest candidate: six red_fixed_bands suites
canonical 9 passed; fingerprint 4 passed; frame 3 passed; prewarm 3 passed;
entrypoint 5 passed; wire 5 passed
[exit=0]  # 29 passed

$ mutation A / B / C / D: six suites each
A: prewarm 2 passed, 1 failed; other five suites passed
B: entrypoint 1 passed, 4 failed; other five suites passed
C: wire 1 passed, 4 failed; other five suites passed
D: fingerprint 1 passed, 3 failed; prewarm 1 passed, 2 failed; other four suites passed
[exit=101 for each failing test invocation]

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [1-ЕСТЬ] все 3 маркеров [ЕСТЬ] в таблицах статусов сопровождены существующим пруфом
PASS  [3-ССЫЛКИ] все 8 ссылок DESIGN.md §N указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 594 ссылок вида docs/*.md указывают на существующие файлы
PASS  [7-RFC-PATH] путей-кандидатов: всего=275 проверено=183 пропущено=92
VERDICT: PASS (0 нарушений)
[exit=0]

$ nl -ba docs/PENDING-SIGNATURE.md | sed -n '1797,1801p'
1797 | в отпечаток селектора входят bands.len() И КАЖДОЕ значение полосы
1800 | цена такта линейна по числу полос: цикл по порогам на КАЖДЫЙ уровень книги
1801 | цена такта = уровни × полосы
[exit=0]

$ nl -ba crates/gateway/src/lib.rs | sed -n '1202,1215p'
1202 for &(price, size) in levels {
1207     for (i, &threshold) in thresholds.iter().enumerate() {
1212         if qualifies { sums[i] += size; }
1215     }
[exit=0]
```

## Handoff Block

### §A — Метаданные

- Дата (UTC, ISO-8601): 2026-09-12T13:25Z
- Milestone: M-84-fixed-depth-bands
- Статус: BLOCKED (`REJECT C-222`, круг 3)
- Audited HEAD: `49d1a90fd88dda58e1752c1d541a6369b2057eb9`

### §B — Что сделано

- Проверен committed полный M-84 artifact set, а не только plan text; B-1/B-2 предыдущего
  reject сняты и одновременно реализуемы.
- Повторены честный кандидат и A/B/C мутации architect; добавлена D-мутация identity of
  checkpoint, которую набор ловит.
- Найден C-222-F1: milestone противоречит подписанному `П-029` и реальной вложенной петле
  `depth_from_book` о цене одной стороны.

### §C — Артефакты / результаты

- `research/critiques/C-222-M-84-round3.md`
- `verify_M-84.sh`: ожидаемый declared `FAIL (14)`, rc 1; honest candidate 29/29; §9
  merge-preview PASS, rc 0.

### §D — Следующий агент + инвокация

- **Следующий агент:** `arbiter`.
- **Paste-ready prompt:**

  ```text
  Ты — arbiter. M-84 после трёх critic-кругов: C-220, C-221, C-222. Не назначай круг 4.
  Аудируй C-222 на `feat/M-84-fixed-bands`, HEAD 49d1a90, и реши C-222-F1: M-84:25/:101
  говорит «цена не множится на число полос» и запрещает оптимизацию, но подписанное
  П-029:1800-1801 и crates/gateway/src/lib.rs:1195-1215 фиксируют уровни × полосы.
  Проверь, является ли старый depth_levels_visited допустимым определением ресурса или
  ошибочным прокси. Выбери authoritative формулировку/необходимый oracle, не меняй
  production gate task 6 и не открывай заново B-1/B-2: C-222 воспроизвёл 29/29 и четыре
  мутации. Запиши A-NNN verdict механизмом, committed/pushed на subject branch.
  ```

- Push-статус: delivery-commit этого файла отправляется в
  `origin/feat/M-84-fixed-bands`; соответствующий immutable ref — audit trail передачи.
- Кэш: временные `CARGO_TARGET_DIR` и mutation worktree удалить после commit/push; факт
  удаления записать в итог операции.

### §E — Риски / открытые вопросы

- Арбитр должен различить внешний обход levels и реальную вычислительную стоимость
  внутреннего цикла; решение меняет формулировку/метрику, не founder-owned состав семи полос.
- Два условия включения task 6 остаются живыми: `TD-159` и барьер до восстановления глубины.
  REJECT не разрешает включение и не даёт менять `GATEWAY_BANDS`.
