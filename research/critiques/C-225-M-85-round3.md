<!-- GATE-META
milestone: M-85
audited_repo: a3ka/hft-platform
audited_base: de43b61ba0cb5074dd08fed8c3e8b25797e0c7fa
audited_head: 3b6a16c279e262f8ad0ac7ad2a9deef98a449035
verdict: APPROVE
-->

# C-225 — M-85 frames/book continuity, round 3 — APPROVE

Дата: 2026-09-16 UTC
Роль: critic, ограниченная проверка закрытия после `A-034`
Предмет: `feat/M-85-frames-book-continuity` @ `3b6a16c`; база сверки — `de43b61`.

## Verdict

**APPROVE — plan-time gate закрыт.** Проверен только исчерпывающий список `A-034` §3/§5; B-2 по неизменённому коду повторно не открывался. `engine-dev` может быть диспетчеризован по задачам 1/2/2b/4/5 M-85.

Живые инварианты предмета: `VB-I-2` (live/replay сходимость) и `VB-I-10` (состояние ограничено окном, не длиной истории), названные и открытые арбитром в `A-034`.

## Закрытие предписанного списка

1. `git diff --name-status de43b61..3b6a16c` содержит ровно три разрешённых файла:
   `red_m85_frames_book_continuity.rs`, `M-85-frames-book-continuity.md` и
   `verify_M-85.sh`. В них закрыты четыре текстовые точки §3: фактическая топология в
   шапке и диагностике RED-оракула, противоречивое следствие и список потребителей в
   milestone, весь `crates/*/src/` в SEEK-guard.
2. Единственные hunks RED-файла от базы — строки 14–24 и 278–281. Фикстура базы
   `:182–203` не пересекается ни с одним hunk; рабочие операнды `m85_1` и строка
   `assert_eq!(` базы `:273` не менялись. Изменён только требуемый A-034 §3.1' текст
   диагностики внутри этого assertion, поэтому она больше не приписывает путь
   `frames_since_with_stats` проверяемому `frames_since`.
3. `git diff --quiet de43b61..3b6a16c -- red_m85_bootstrap_cost.rs` завершился `0`:
   оракул задачи 3 не тронут.
4. Предъявлен повторный прогон: RED-набор даёт ровно `3 passed; 1 failed` с ожидаемым
   `m85_1`; в панике нет `frames_since_with_stats`. Стоимостной набор даёт `2 passed`.
   Полный acceptance даёт PASS расширенному SEEK-guard и ожидаемый `VERDICT: FAIL (3)`:
   это RED `m85_1`, workspace-suite, и отдельная задача 5, а не дефект закрытия A-034.
5. Новые утверждения о коде в изменённых строках подтверждены на audited head:
   `frames_since` использует `journal::stream` (`lib.rs:2974`), seek-вариант использует
   `journal::stream_from` (`:3037`), а общий reducer создаётся в `:2233` и для
   `seq <= after` вызывает только `seed_vwap` (`:2253–2255`). Поиск по
   `crates/*/src/` после исключения комментариев и объявления не нашёл caller'ов
   `frames_since_with_stats`; три названных тестовых caller'а существуют.
6. Запись остаточного риска присутствует в M-85 §6 и занимает ровно шесть физических
   непустых строк (`:270–275`), то есть не превышает лимит `A-034` §6.

## Non-blocking note for reviewer

Ссылка на `A-028` остаётся на невлитой `docs/M-73-closeout-architect`. Это явно исключено
`A-034` §4 из условия круга 3; исправление не требуется для M-85 и остаётся NOTE на
close-out reviewer'а.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-225
allocator_exit=0

$ EVENT_NAME=push PUSH_BEFORE=3b6a16c279e262f8ad0ac7ad2a9deef98a449035 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 3b6a16c..HEAD не ввёл второй носитель под занятым идентификатором
artifact_ids_exit=0

$ git diff --name-status de43b61..3b6a16c
M	crates/gateway/tests/red_m85_frames_book_continuity.rs
M	milestones/M-85-frames-book-continuity.md
M	scripts/verify_M-85.sh
exit=0

$ git diff --quiet de43b61..3b6a16c -- crates/gateway/tests/red_m85_bootstrap_cost.rs
exit=0

$ git diff --unified=0 de43b61..3b6a16c -- crates/gateway/tests/red_m85_frames_book_continuity.rs
@@ -14,3 +14,4 @@
@@ -19,4 +20,5 @@
@@ -278,4 +280,4 @@ fn m85_1_client_assembled_series_equals_full_replay_on_delta_tail() {
exit=0

$ cargo test -p gateway --test red_m85_frames_book_continuity --quiet
running 4 tests
m85_1_client_assembled_series_equals_full_replay_on_delta_tail --- FAILED
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
... Причина структурная: `frames_since` отдаёт окно СВЕЖЕМУ `Reducer::new` ...
frames_exit=101

$ cargo test -p gateway --test red_m85_bootstrap_cost --quiet
running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
bootstrap_exit=0

$ bash scripts/verify_M-85.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: оракул m85_1 (VB-I-2 на пути frames_since) (исполнено тестов: 1, exit=101)
PASS: оракул m85_4 + страж оси m85_5 (VB-I-10 на пути frames_since) (исполнено тестов: 2)
PASS: ! grep -rn 'frames_since_with_stats' crates/*/src/ | grep -v '///' | grep -v 'pub fn frames_since_with_stats' | grep -q .
FAIL: ! grep -qE 'инкрементальный push.*frames_msgs|frames_msgs.*инкрементальный push' crates/gateway-serve/src/lib.rs
VERDICT: FAIL (3)
verify_exit=1

$ awk 'NR>=2226 && NR<=2256 {print NR ":" $0}' crates/gateway/src/lib.rs | grep -E '2226:|2233:|2253:|2254:|2255:'
2226:fn reduce_event_stream(
2233:    let mut reducer = Reducer::new(selector);
2253:        if after.upto_seq.is_some_and(|seq| event.seq <= seq) {
2254:            reducer.seed_vwap(&event);
2255:            continue;

$ awk 'NR>=2966 && NR<=3045 {print NR ":" $0}' crates/gateway/src/lib.rs | grep -E '2966:|2974:|3020:|3037:'
2966:pub fn frames_since(
2974:    let mut stream = journal::stream(dir, filter)?;
3020:pub fn frames_since_with_stats(
3037:    let mut stream = journal::stream_from(dir, filter, after.upto_seq)?;
exit=0

$ awk 'NR>=269 && NR<=276 {if ($0 != "") n++} END {print n}' milestones/M-85-frames-book-continuity.md
6
exit=0
```
