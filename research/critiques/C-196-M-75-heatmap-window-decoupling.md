<!-- GATE-META
milestone: M-75
audited_repo: a3ka/hft-platform
audited_base: af2945284a728112c7aae7ef1a764f6a265c3dc0
audited_head: c9963900ba3bbd78440d467e0a35278817bdcc5c
verdict: REJECT
-->

# C-196 — M-75 heatmap-window-decoupling, круг 2: REJECT

## Судимый набор

Суждение относится к `origin/feat/M-75-heatmap-window-decoupling` на
`c9963900ba3bbd78440d467e0a35278817bdcc5c`, против текущей базы
`origin/main` `af2945284a728112c7aae7ef1a764f6a265c3dc0`. Диапазон содержит
исходный RED-набор, `C-194`, а затем один architect-коммит закрытия: H-2,
H-5/H-5b и обновление verify/milestone. `crates/contracts/**`, `docs/rfc/**`,
`GATEWAY_SCHEMA_VERSION` и `GATEWAY_BANDS` в нём не затронуты.

Применён актуальный Р-4 из `origin/main` `af29452`:
признак обязан быть недоступен миру, в котором событие не произошло, и это
предъявляется мутацией признака. Прочитаны `VB-I-2`, `VB-I-5`, `VB-I-10` и
`MD-I-8` из FA viz-backend; для этого предмета существен **VB-I-10** —
bounded-window snapshot не может быть «починен» ни пустой картой, ни
клиентски управляемым широким окном. `П-014`/`П-020` подтверждают состав
полос и 2 MB cap. `П-027` существует на
`origin/docs/M-45-rollout-signature` (`c1ebac1`, решение добавлено
`c57583b`); оно разрешает именно это расцепление без новой подписи.

## Что C-194 действительно закрыто

### B-1 — принято: H-2 сделан исполняемым RED

`red_heatmap_window_env.rs` существует, компилируется и исполняет пять
сценариев. Три красные проверки наблюдают нынешний дефект (`env` не читается,
мусор и значение вне `(0,1)` принимаются), а два парных vantage не дают
«починить» набор отказом любого старта. Это лучше COMPILE-RED для той части
контракта, которую можно наблюдать до появления getter/setter: существующий
`serve_config_from_env` уже возвращает `Result`. Прецедент M-67 §10 применим
к этому узкому выбору: некомпилируемый тест против ещё несуществующей формы
сломал бы весь workspace и не добавил бы наблюдения malformed/out-of-range.

### Первая половина B-2 — H-5 различает зажатую связку

Мутация `max(bands).min(0.001)` сделала старые H-1/H-3/H-4 зелёными и
оставила H-5 красным. Следовательно, ранее слепый мир из C-194 действительно
закрыт для первой половины требования: полоса `0.0004` не может сузить
heatmap/COB относительно `0.001`.

## Блокеры

### B-3 — C-194 B-2 остаётся открытым: H-6 отложен за реализацию

C-194 B-2 требовал не только below-config пару, но и наблюдение, что
**смена effective server setting меняет окно**. M-75 §8bis прямо признаёт,
что H-5 этого не доказывает, переносит H-6 в task 5b после task 2 и называет
живой остаточный мир: «жёсткая константа, конфиг игнорируется».

Этот мир воспроизведён, а не предположен: временная замена в
`crates/gateway/src/lib.rs:1557` на `let w = 0.001;` дала 3/3 PASS в
`red_heatmap_window_decoupled` и 2/2 PASS в
`red_heatmap_window_server_owned`. Значит все пять имеющихся оракулов
принимают реализацию, где `GATEWAY_HEATMAP_WINDOW` не влияет на выдачу.
Именно это запрещает Р-4: признак «карты равны» всё ещё изготовим в мире
¬P. H-6 отсутствует и `verify_M-75.sh` не содержит ни task #5b, ни H-6,
хотя milestone обещает минимум одну проверку на каждую задачу.

M-67 §10 не оправдывает dispatch task 2 без H-6. Там оракулы против
несуществующего типа не писались потому, что соответствующие задачи были
заблокированы и **не диспетчеризовались**. Здесь task 2 будет создавать
заявленную сигнатуру, а task 5b предписывает architect'у написать RED уже
после этой реализации. Это прямо обращает RED-first. Названный остаточный
риск делает пробел честным, но не делает его допустимым plan-time набором.

**Условие снятия:** до dispatch task 2 должен существовать полный committed
RED-артефакт, наблюдающий effect server setting на heatmap и COB, а verify
должен исполнять его как проверку task 5b. Если architect считает такой
порядок технически невозможным без COMPILE-RED, это методологический спор,
а не основание молча отложить оракул.

Это второй REJECT подряд по той же причине C-194 B-2. Поэтому следующий
адресат — независимый арбитр со свежим контекстом (`gates.md` §0), не третий
self-fix круг architect↔critic.

### B-4 — §14 milestone остался handoff'ом круга 1 и сообщает ложное состояние

В `milestones/M-75-heatmap-window-decoupling.md:270-276` следующий critic всё
ещё назван «кругом 1», а отдельным вопросом названо отсутствие H-2. Это
противоречит шапке rev2, task 5 и текущему коммиту, где H-2 уже существует и
исполнен. Раздел не помечен историческим. Handoff должен быть самодостаточным
и правдивым; в таком виде он маршрутизирует следующий круг к уже снятому
вопросу и скрывает единственный живой B-3.

**Условие снятия:** architect обновляет §14 на актуальный маршрут: C-196,
арбитр по повтору B-2, затем только решение арбитра/исправленный набор.

## NOTES

- Сообщение commit `c996390` заявляет `verify_M-75.sh` как `VERDICT: FAIL
  (14)`. Независимый прогон дал `VERDICT: FAIL (12)`. Это не меняет красный
  status пред-implementation набора, но дальнейшие артефакты должны ссылаться
  на фактический вывод, не на число из commit message.
- `П-027` не является предком audited head: решение доступно на названной
  origin-ветке, но не в дереве `c996390`. Это не отдельный блокер при данном
  commit-chain reference, однако merge/handoff должен сохранить явную ссылку
  на `c1ebac1` до попадания решения в main.

## Required disposition

**REJECT.** Dev не диспетчеризуется. B-1 принят, но B-2 закрыт только
наполовину; B-3 повторяет ту же причину, поэтому founder передаёт предмет
арбитру. B-4 остаётся обязательной mechanical правкой architect'а перед
повторной подачей.

## Done Block

```text
$ git rev-parse origin/main; git rev-parse c996390; git merge-base c996390 origin/main
af2945284a728112c7aae7ef1a764f6a265c3dc0
c9963900ba3bbd78440d467e0a35278817bdcc5c
d77398d7b22396c452d2651e90498033186055dd

$ git log --oneline origin/main..c996390
c996390 spec(M-75): C-194 закрыт — H-2 написан, H-5 отличает зажатую связку [architect]
5489a4d docs(M-75): C-194 — reject incomplete heatmap oracle set [critic]
c3ee54b spec(M-75): расцепление окна heatmap от полос — набор architect'а закоммичен ПОЛНОСТЬЮ [architect]

$ cargo test -p gateway --test red_heatmap_window_decoupled --quiet
running 3 tests
. 1/3
hw_i_1_heatmap_size_is_independent_of_bands --- FAILED
hw_i_3_canonical_bands_fit_under_signed_cap --- FAILED
test result: FAILED. 1 passed; 2 failed
exit=101

$ cargo test -p gateway --test red_heatmap_window_server_owned --quiet
running 2 tests
. 1/2
hw_i_5_below_config_band_cannot_shrink_the_map --- FAILED
test result: FAILED. 1 passed; 1 failed
exit=101

$ cargo test -p gateway-serve --test red_heatmap_window_env --quiet
running 5 tests
. 1/5
malformed_heatmap_window_is_rejected --- FAILED
out_of_range_heatmap_window_is_rejected --- FAILED
. 4/5
heatmap_window_is_declared_in_compose --- FAILED
test result: FAILED. 2 passed; 3 failed
exit=101

$ mutant: w = max(bands).min(0.001)
$ cargo test -p gateway --test red_heatmap_window_decoupled --quiet
test result: ok. 3 passed; 0 failed
exit=0
$ cargo test -p gateway --test red_heatmap_window_server_owned --quiet
hw_i_5_below_config_band_cannot_shrink_the_map --- FAILED
test result: FAILED. 1 passed; 1 failed
exit=101

$ mutant: w = 0.001
$ cargo test -p gateway --test red_heatmap_window_decoupled --quiet
test result: ok. 3 passed; 0 failed
exit=0
$ cargo test -p gateway --test red_heatmap_window_server_owned --quiet
test result: ok. 2 passed; 0 failed
exit=0
$ git diff --exit-code -- crates/gateway/src/lib.rs
exit=0

$ bash scripts/verify_M-75.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: оракул расцепления (H-1 · H-3 · H-4) (исполнено тестов: 3, exit=101)
FAIL: оракул серверного владения окном (H-5 · H-5b) (исполнено тестов: 2, exit=101)
FAIL: оракул fail-closed разбора GATEWAY_HEATMAP_WINDOW (исполнено тестов: 5, exit=101)
VERDICT: FAIL (12)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ bash -n scripts/verify_M-75.sh; git diff --check origin/main..c996390
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31T19:01Z
- Milestone: M-75-heatmap-window-decoupling
- Статус: BLOCKED
- HEAD: c996390 — spec(M-75): C-194 закрыт — H-2 написан, H-5 отличает зажатую связку [architect]

## §B — Что я сделал
- Проверил committed artifact set и повторно воспроизвёл обе мутации из §8ter.
- Подтвердил закрытие C-194 B-1 и первой половины B-2; нашёл, что H-6 отсутствует и `w = 0.001` проходит все пять существующих оракулов.

## §C — Артефакты / результаты
- `research/critiques/C-196-M-75-heatmap-window-decoupling.md`
- Done Block: baseline RED exit=101; clamp mutation old suite exit=0 / H-5 exit=101; hard-code mutation 3/3+2/2 PASS; verify_M-75 exit=1; verify_design_claims exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter`
- **Paste-ready промпт:**
  ```
  Свежий арбитраж по M-75-heatmap-window-decoupling. Прочти C-194 и C-196,
  milestones/M-75-heatmap-window-decoupling.md §8bis/§8ter,
  docs/workflow/oracle-blindness-class-2026-08-28.md §5 Р-4 на main af29452,
  и фактические RED-файлы. Реши спор метода: допустимо ли dispatch task 2,
  если H-6, единственный оракул смены server setting, сознательно пишется
  только после появления этой реализации. Проверь фактом мутант `w = 0.001`:
  H-1/H-3/H-4/H-5/H-5b зелёные. Вынеси обязательное решение и маршрут;
  отдельно назови, должен ли §14 milestone быть обновлён до нового handoff.
  Предмет: origin/feat/M-75-heatmap-window-decoupling @ c996390; база main @ af29452.
  ```
- Push-статус: pending commit/push C-196 on subject branch
- Кэш: pending cleanup after push

## §E — Риски / открытые вопросы
- Второй REJECT по C-194 B-2; `gates.md` §0 требует арбитра, не третий круг critic.
- `П-027` пока живёт на origin/docs/M-45-rollout-signature @ c1ebac1, не в audited head.

=== END HANDOFF ===
