<!-- GATE-META
milestone: M-75
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: 8609acdd421d68fb1a771c6a851a20b34a7050ed
verdict: REJECT
-->

# C-198 — M-75 heatmap-window-decoupling, круг 3: REJECT

## Судимый набор

Проверен committed-набор `origin/feat/M-75-heatmap-window-decoupling` на
`8609acdd421d68fb1a771c6a851a20b34a7050ed`, против merge-base с актуальным
`origin/main` `d77398d7b22396c452d2651e90498033186055dd`. В наборе есть
T-контракты §5, declared trait/function signatures, четыре RED-target'а
(H-1..H-6), `scripts/verify_M-75.sh` и milestone. Диапазон не меняет
`crates/contracts/**`, RFC, `GATEWAY_SCHEMA_VERSION` или `GATEWAY_BANDS`.

Применены Р-4 из `docs/workflow/oracle-blindness-class-2026-08-28.md` §5
на `main` `2e63a37`, а также `VB-I-2`, `VB-I-5`, **VB-I-10** и **MD-I-8**
из FA viz-backend. У `gateway` и `gateway-serve` нет собственного FA
(известный gap reading-map), поэтому эти живые инварианты остаются
обязательным внешним якорем проверки.

## C-196 закрыт

### B-3 — принято: H-6 теперь committed до dispatch

`crates/gateway-serve/tests/red_heatmap_window_effective_setting.rs` содержит
H-6/H-6b. Он наблюдает цепь
`GATEWAY_HEATMAP_WINDOW` → `serve_config_from_env` → выдача при неизменном
клиентском selector: narrow и wide server setting обязаны давать различный
heatmap/COB. Поэтому старый слепой мир «`w = 0.001`, настройка игнорируется»
больше не принимается: H-6 красен.

Я воспроизвёл таблицу §8ter в отдельном worktree, а не по её отчёту. Мутант
`max(bands).min(0.001)` оставил H-5 красным при зелёном старом наборе;
`w = 0.001` оставил зелёными все пять прежних оракулов и красным только H-6.
Исходный `crates/gateway/src/lib.rs` после проб возвращён точно к `8609acd`.

### B-4 — принято: §14 обновлён

§14 теперь называет C-196, фактическое pre-dispatch появление H-6 и актуальный
следующий маршрут; устаревшего handoff'а круга 1 в нём нет.

### Нужен ли арбитр C-196

Нет, не для уже снятого B-3. `gates.md` §0 назначает арбитраж как разрешение
**спора/тупика** при повторном REJECT, а не как карательную формальность.
Architect не оспорил требование C-196 и до dispatch положил именно требуемый
committed H-6. Следовательно, предмет методологического разногласия исчез
работой, а не обходом. Это узкое применение §0: оно не разрешает пропускать
арбитра при продолжающемся споре или объявлять неисполненное условие
«принятым».

## Блокер

### B-5 — M-75 ломает committed MD-I-8 witness M-68, но plan-time набор не управляет этим

Применение заявленной M-75 семантики — default effective server window
`0.001` в месте построения heatmap/COB вместо `Selector.bands` — делает
красными два existing sacred witness'а `crates/gateway/tests/red_depth_from_book.rs`:

- `md_i8_d2_setup_heatmap_sees_the_tail_delta_level` специально ставит
  `FAR_BAND = 0.60` и требует увидеть tail level `FAR_B_OFF = 0.45`;
- `md_i8_d7b_reach_is_sampled_where_the_numbers_are_delta_grows_the_book`
  требует live COB reach больше `.04`; его комментарий прямо фиксирует, что
  широкий client selector нужен, поскольку окно было `max(selector.bands)`.

Это не предположение о побочном эффекте: во временной, полностью откаченной
пробе planned call path все новые H-1..H-6 targets вели себя как ожидается,
но `cargo test --all --quiet` завершился `101` ровно на этих D-2/D-7b.
Тем самым M-75 меняет предусловие независимого **MD-I-8** witness, а
plan-time артефакты не называют его и не доказывают совместимость. Dev task
2--4 не вправе молча переделывать sacred RED-test; §Tasks и verify не дают
architect'у committed решения. Это также ставит под риск **VB-I-10**:
bounded server-owned snapshot должен быть доказан без потери независимого
наблюдения depth delta.

**Условие снятия:** до dispatch architect обязан закоммитить полный
предdispatch-набор, который явно управляет M-68 MD-I-8 witness'ами при
server-owned окне: именованная область/инвентарь в §Tasks, исполнимое
свидетельство и verify coverage, сохраняющие их независимый смысл и
state/serial hygiene, если настройка process-global. В совокупности набор
должен доказывать зелёный `cargo test --all` после intended implementation.
Конкретный внутренний дизайн здесь не предписывается критиком.

## NOTE

`П-014` (provenance bands) и `П-020` (2 MB cap) не изменены. Подпись П-027
для немедленного расцепления доступна на
`origin/docs/M-45-rollout-signature` `d9e015bdb9598bc6aba87bc212aa200be25715d6`,
но не является предком audited head или current main. Это не отдельный
REJECT при переданном commit-chain reference, однако следующий handoff/PR
должен сохранить immutable reference до merge.

## Required disposition

**REJECT.** C-196 B-3/B-4 закрыты; арбитр для снятого спора не требуется.
Но B-5 — самостоятельный plan-time blocker: dev не диспетчеризуется, пока
architect не представит committed управление затронутым M-68 MD-I-8
свидетельством и обновлённый полный набор.

## Done Block

```text
$ git fetch origin feat/M-75-heatmap-window-decoupling; git rev-parse origin/feat/M-75-heatmap-window-decoupling; git rev-parse HEAD; git status --porcelain
8609acdd421d68fb1a771c6a851a20b34a7050ed
8609acdd421d68fb1a771c6a851a20b34a7050ed
exit=0; worktree clean

$ bash scripts/next_artifact_id.sh C
C-198
allocator_exit=0

$ EVENT_NAME=push PUSH_BEFORE=8609acdd421d68fb1a771c6a851a20b34a7050ed bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 8609acd..HEAD не ввёл второй носитель под занятым идентификатором
check_artifact_ids_exit=0

$ mutant: let w = max(bands).min(0.001)
$ cargo test -p gateway --test red_heatmap_window_decoupled --quiet
test result: ok. 3 passed; 0 failed
exit=0
$ cargo test -p gateway --test red_heatmap_window_server_owned --quiet
hw_i_5_below_config_band_cannot_shrink_the_map --- FAILED
test result: FAILED. 1 passed; 1 failed
exit=101
$ cargo test -p gateway-serve --test red_heatmap_window_effective_setting --quiet
hw_i_6_server_setting_changes_effective_window --- FAILED
test result: FAILED. 1 passed; 1 failed
exit=101

$ mutant: let w = 0.001
$ cargo test -p gateway --test red_heatmap_window_decoupled --quiet
test result: ok. 3 passed; 0 failed
exit=0
$ cargo test -p gateway --test red_heatmap_window_server_owned --quiet
test result: ok. 2 passed; 0 failed
exit=0
$ cargo test -p gateway-serve --test red_heatmap_window_effective_setting --quiet
hw_i_6_server_setting_changes_effective_window --- FAILED
test result: FAILED. 1 passed; 1 failed
exit=101
$ git diff --check; git diff --exit-code -- crates/gateway/src/lib.rs; git status --porcelain
exit=0; mutation worktree clean

$ temporary intended-call-path probe; cargo test --all --quiet
md_i8_d7b_reach_is_sampled_where_the_numbers_are_delta_grows_the_book --- FAILED
SETUP НЕ СОСТОЯЛСЯ [bid]: живой охват 0.000500 — дельта не достроила книгу
md_i8_d2_setup_heatmap_sees_the_tail_delta_level --- FAILED
SETUP НЕ СОСТОЯЛСЯ: heatmap не видит уровень 3575000000000; Ячеек heatmap: 12
error: test failed, to rerun pass -p gateway --test red_depth_from_book
exit=101
probe reverted; mutation worktree clean

$ bash scripts/verify_M-75.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: оракул расцепления (H-1 · H-3 · H-4) (исполнено тестов: 3, exit=101)
FAIL: оракул серверного владения окном (H-5 · H-5b) (исполнено тестов: 2, exit=101)
FAIL: оракул fail-closed разбора GATEWAY_HEATMAP_WINDOW (исполнено тестов: 5, exit=101)
FAIL: оракул effective server setting (H-6 · H-6b) (исполнено тестов: 2, exit=101)
VERDICT: FAIL (13)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
$ bash -n scripts/verify_M-75.sh; git diff --check origin/main...HEAD
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31
- Milestone: M-75-heatmap-window-decoupling
- Статус: BLOCKED
- HEAD: 8609acd — spec(M-75): §8ter — «семь из восьми» не говорило, что считает [architect]

## §B — Что я сделал
- Аудировал committed T-контракты, signatures, RED H-1..H-6, verify и milestone.
- Независимо воспроизвёл обе мутации §8ter и откатил source в отдельном worktree.
- Принял C-196 B-3/B-4 и проверил законность отсутствия арбитража по снятому спору.
- Нашёл B-5: intended M-75 path делает красными existing MD-I-8 D-2/D-7b witness'ы M-68.

## §C — Артефакты / результаты
- `research/critiques/C-198-M-75-heatmap-window-decoupling.md`
- Мутация clamp: H-1/H-3/H-4 зелёные, H-5 красный; hard-code: пять прежних зелёные, только H-6 красный.
- Intended-call-path probe: `cargo test --all --quiet` exit=101 на `red_depth_from_book` D-2/D-7b; проба полностью откачена.
- `verify_M-75.sh` exit=1 (ожидаемый RED plan); `verify_design_claims.sh --merge-preview origin/main` exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Architect round 4 по M-75. Прочти C-198 полностью, особенно B-5 и Done Block,
  milestones/M-75-heatmap-window-decoupling.md §8bis/§8ter/§14, FA
  viz-backend VB-I-10 и MD-I-8, P-014/P-020/P-027, и sacred M-68 тест
  crates/gateway/tests/red_depth_from_book.rs D-2/D-7b. Предмет:
  origin/feat/M-75-heatmap-window-decoupling @ <C-198-pushed-head>.
  C-196 B-3/B-4 уже закрыты и H-6 нельзя потерять. До dev dispatch создай и
  закоммить полный architect-набор, который явно управляет тем, что default
  server-owned window .001 меняет независимые MD-I-8 witness'ы D-2/D-7b:
  обнови §Tasks с именованным scope, committed RED/fixture-владение и verify,
  сохрани независимый смысл witness'ов и state/serial hygiene. Не меняй
  implementation в dev; не подменяй доказательство transcript'ом. Покажи,
  что intended implementation может пройти cargo test --all, затем верни
  critic commit-chain, milestone path и stakes.
  ```
- Push-статус: C-198 должен быть committed/pushed на `feat/M-75-heatmap-window-decoupling`.
- Кэш: удалить worktree-local `target/` после push.

## §E — Риски / открытые вопросы
- B-5 самостоятельный; это не продолжение спора C-196 B-3, поэтому арбитраж §0 им не заменяется.
- П-027 не предок audited head/current main: сохранять `d9e015b` как immutable provenance до merge.
- У gateway/gateway-serve нет собственного FA; scope обязан продолжать явно якориться на VB-I-10/MD-I-8.

=== END HANDOFF ===
