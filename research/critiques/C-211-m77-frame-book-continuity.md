<!-- GATE-META
milestone: M-77
audited_repo: a3ka/hft-platform
audited_base: a03336f78822081bbce7f5dbc069b89a7019edd1
audited_head: 6fb3553509391c5be77b6ff79e7e2f0e8282df40
verdict: REJECT
-->

# C-211 — M-77 frame-book continuity — REJECT

Дата: 2026-09-05 (UTC)
Режим: plan-time hft critic; предмет — `feat/M-77-frame-book-continuity` @ `6fb3553`.

## Решение гейта

**REJECT.** Ветвь содержит RED-оракул, milestone и verify-скрипт, но не содержит
полного набора до диспетчеризации: выбранная развязка ещё не закреплена буквальным
контрактом/сигнатурами и RED-оракулом её опасной границы. Кроме того, три названных
в milestone доказательных артефакта отсутствуют на audited head, а `verify_M-77.sh`
не проверяет закрытие task 2 и task 4 самостоятельно.

До следующего круга engine-dev не диспетчеризировать.

## Набор, который был реально проверен

Диапазон `a03336f..6fb3553` вводит только:

```
A  crates/gateway/tests/red_m77_frame_book_continuity.rs
A  milestones/M-77-frame-book-continuity.md
A  scripts/verify_M-77.sh
```

Нового T-contract/trait-signature в наборе нет; milestone честно заявляет отсутствие
T1. Однако task 2 в самом milestone остаётся `OPEN` и требует после решения критика
зафиксировать буквально «signatures + source rule». Это не опциональная запись: при
выборе Б она определяет, какой курсор и какой источник глубины описывают кадр.
Пока её нет как committed artifact, будущий RED не может пиннить ни форму, ни
семантику выбранной развязки.

Следующие имена приведены в M-77 как основания/доказательства, но на `HEAD` их нет:

```
research/arbitration/A-028-m74-pre-dispatch-completeness.md
research/reviews/R-172-M-70-pr-gate-rev3.md
research/critiques/C-210-M-70-fa-vb-i-5.md
```

По универсальному A-028 §1 названный оракул должен существовать как committed text и
каждый его путь должен быть разрешён. Нахождение этих файлов в иных refs не делает
их частью audited artifact set. Их надо сделать достижимыми на subject branch либо
заменить ссылку проверяемой неизменяемой ссылкой с доступным текстом; пока это
blocker полноты, а не замечание к оформлению.

## RED-набор: достаточен для текущего дефекта

`red_m77_frame_book_continuity.rs` использует настоящий journal/reader,
`EpochFilter::OwnCaptureOnly`, `LiveReducer::resume`, `pump` и потребительский
`Snapshot::apply`; затем сверяет состояние клиента с независимым `gateway::snapshot`.
Это правильная граница потребителя (Р-1), а не внутренний счётчик редьюсера.

Продовый селектор удержан во всех subject-сценариях: Binance/BTCUSDT, timeframe
1000, `window_ms=Some(60000)`, `depth_cadence=Some(1000)`, полосы `.001` и `.02`.
Следовательно, Р-2 соблюдён. Оракул сравнивает пары `(time, value)`, а не число
точек; guard ненулевого полного replay и предварительная сверка server snapshot с
full replay дают различающий признак (Р-4). Это корректно ловит rev2-дефект:
на .001 число точек совпадает, значения расходятся. Фикстура — вход, который
принимает production reader, а не отдельный тестовый формат.

Собственный запуск на audited head дал control PASS и ровно три предметных FAIL;
значит краснота локализована. Обратный анти-плацебо также выполнен собственным
запуском на отдельном недостижимом WIP `d9531ce`: его единственная существенная
реализационная строка `batch.book = self.full.book.clone()` и идентичный финальный
тестовый файл дали 4/4 PASS. Это доказывает чувствительность оракула к текущей
семантике, но WIP не является предлагаемым committed artifact и не снимает гейт.

## Выбор развязки: Б, не А

Выбираю **Б: кадр несёт depth-delta от живого редьюсера**.

Цена А известна и находится на hot `pump` path: `self.full.book.clone()` копирует
самое дорогое поле на **каждом** batch. Четыре зелёных M-77-теста у WIP доказывают
лишь семантическое совпадение; существующий `red_snapshot_noclone` измеряет
аллокации `snapshot()`, предварительно вызывая `pump` вне измеряемого участка, и
потому не страхует эту цену. Это прямо возвращает риск VB-I-10 и M-56/TD-097.

У Б есть реальная цена: в кадре два источника истины и `self.full` может опережать
delivery cursor при отказе cap. Она принимается только вместе с обязательным
следующим артефактным набором:

1. task 2 должен буквально закрепить signature/shape и source rule для каждой
   series, включая связь depth-delta с delivery cursor;
2. RED-оракул должен пройти refused-cap/terminal-delivery/retry, где full уже впереди
   курсора, и всё же доказать `snapshot(C) + delivered frames == full replay`;
3. task 4 должен иметь проверку цены Б на границе `pump` (не только `snapshot()`), а
   verify обязан запускать и требовать этот оракул.

Это условия выбора Б, не проектирование новой реализации. Без них Б оставляет
непинненную рассинхронизацию; с ними избегает известного безусловного clone-cost А.

## `verify_M-77.sh`

T5 честен: скрипт запускает весь набор, требует ровно три предметных RED и отдельно
показывает control PASS. Затем T4 намеренно делает итог `FAIL`, поэтому T5 —
локализация красноты, а не амнистия красному.

Но скрипт зеленее требуемого предмета в двух местах. Он не проверяет существование
и буквальное содержание выбранного task-2 contract/source rule, и не имеет
самостоятельного запуска/требования task-4 price oracle на `pump`. После будущего
исправления общая crate suite может стать зелёной при отсутствии обоих artefacts.
Это нарушает требование гейта «не менее одной механической проверки на задачу»;
один T4-статус всей suite не доказывает task 2 или task 4.

## Done Block

```text
$ git ls-remote origin refs/heads/feat/M-77-frame-book-continuity
6fb3553509391c5be77b6ff79e7e2f0e8282df40	refs/heads/feat/M-77-frame-book-continuity
exit=0

$ git merge-base 6fb3553509391c5be77b6ff79e7e2f0e8282df40 origin/main
a03336f78822081bbce7f5dbc069b89a7019edd1
exit=0

$ git diff --name-status a03336f..6fb3553
A	crates/gateway/tests/red_m77_frame_book_continuity.rs
A	milestones/M-77-frame-book-continuity.md
A	scripts/verify_M-77.sh
exit=0

$ cargo test -p gateway --test red_m77_frame_book_continuity -- --nocapture
running 4 tests
test control_snapshot_tail_passes ... ok
test steady_values_match_full_replay ... FAILED
test one_sided_depth_delta_matches_full_replay ... FAILED
test resync_then_delta_matches_full_replay ... FAILED
test result: FAILED. 1 passed; 3 failed
exit=101

$ git diff d9531ce:crates/gateway/tests/red_m77_frame_book_continuity.rs HEAD:crates/gateway/tests/red_m77_frame_book_continuity.rs
tree_test_diff_exit=0
$ (cd /tmp/hft-critic-m77-candidate && cargo test -p gateway --test red_m77_frame_book_continuity -- --nocapture)
running 4 tests
test result: ok. 4 passed; 0 failed
exit=0

$ bash scripts/verify_M-77.sh
INFO RED-ФАЗА: cargo test --all exit=101 — задача 3 не исполнена, это ОЖИДАЕМО
PASS T5: локализация: ровно 3 предметных RED, control зелёный
FAIL T4: задачи 2–4 не исполнены
PASS T6: scope
VERDICT FAIL (1)
exit=1

$ git cat-file -e HEAD:research/arbitration/A-028-m74-pre-dispatch-completeness.md
exit=128
$ git cat-file -e HEAD:research/reviews/R-172-M-70-pr-gate-rev3.md
exit=128
$ git cat-file -e HEAD:research/critiques/C-210-M-70-fa-vb-i-5.md
exit=128

$ rg -n -C 2 'peak_delta|live\.snapshot|pump' crates/gateway/tests/red_snapshot_noclone.rs
189: ... live.pump(...)
221: let peak = peak_delta(|| live.snapshot());
exit=0

$ git diff --check a03336f..6fb3553
exit=0

$ bash scripts/next_artifact_id.sh C
C-211
exit=0

$ EVENT_NAME=push PUSH_BEFORE=6fb3553509391c5be77b6ff79e7e2f0e8282df40 PR_BASE_SHA='' bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 6fb3553..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ EVENT_NAME=push PUSH_BEFORE=6fb3553509391c5be77b6ff79e7e2f0e8282df40 PR_BASE_SHA='' GITHUB_SHA='' bash scripts/check_gate_meta.sh
── GATE-META: диапазон 6fb35535..HEAD, origin=a3ka/hft-platform
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
```
