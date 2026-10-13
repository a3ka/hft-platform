<!-- GATE-META
milestone: TD-257
audited_repo: a3ka/hft-platform
audited_base: f4ac015d52c8ac75d2b90d68408440f334032458
audited_head: 1ac11c6c0918e2a3453d027cab4ed5cbbc4b9170
verdict: APPROVE
-->

# R-262 — `TD-257`: оракул `o12` судит договор `CT-RFC-09` §2.4, а не порядок `select!` — APPROVE

**Роль:** reviewer. **Дата:** 2026-10-13. **Предмет:** ветка `origin/fix/td-257-o12-contract-order`
(architect), PR #345. Вершина взята командой (`04-workflow.md` §2):
`git fetch origin && git rev-parse origin/fix/td-257-o12-contract-order` →
`1ac11c6c0918e2a3453d027cab4ed5cbbc4b9170` — совпадает со справкой мандата (merge `main` поверх
`f2b9bf9f`; `f2b9bf9f` — предок вершины). База аудита — `merge-base origin/main` = `f4ac015d`.
Повторный `fetch` перед записью вердикта: вершина ветки не сдвинулась (`1ac11c6c`); `main` ушёл
на `0a907ad6` (переезд архива `M-98`, PR #346) — проверено деревом слияния, §5.

FA: `VB-I-2` (`docs/fa/viz-backend.md:285`, live == replay) — инвариант, который защищает
весь набор `red_ws_session`: виджет не смешивает серии двух селекторов. Договор предмета —
`docs/rfc/CT-RFC-09-ws-session.md` §2.4 (:97-105): «промежуточных кадров старого селектора
**после** нового снапшота быть не должно».

## §0. Что прочитано (ярус C — только грепом)

- `TECH-DEBT.md` (на `origin/main`) — греп `TD-257`: строка таблицы `:239`, карточка `:5571`,
  запись счёта `:53`. Гипотеза причины в карточке — `select!` без `biased`, ассерт строже договора.
- `PROJECT-STATE.md` — греп `M-98|o12`: раздел `M-98` (:3028, `MS-STATE: M-98 CLOSED`), `o12` в `M-65` (:1672).
- `CT-RFC-09` §2.4 целиком, строки `O-1`/`O-12` таблицы оракулов.

## §1. Block-scope

Диф `f4ac015d..1ac11c6c` (без вклада `main`): ровно один файл,
`crates/gateway-serve/tests/red_ws_session.rs` +46/−4, внутри `o12`. Файл — sacred RED
architect'а (`scope-guard.md`); автор — architect (метка `[architect]` в subject). `src/`,
`contracts/`, скрипты гейтов не тронуты. RISK-BLOCK не применим: `gateway-serve` — read-only
WS-транспорт, не `risk/killswitch/oms/venue-*/contracts` — risk-critic не требуется. Block-C: N/A.
RED-first: правка теста сделана владельцем зоны, dev тест не трогал (в диапазоне один
тестовый коммит, `f2b9bf9f`). Атомарность: один коммит на одну задачу со ссылкой `TD-257`.

## §2. Суть правки — по коду, не по телу коммита

Было: `let after = recv(..); assert_eq!(type_of(&after), Some("snapshot"))` — «первым после
release — snapshot». Стало: цикл до `snapshot`; до него допускаются только `frame` той же
подписки `w1` (не более 4), любой иной тип — panic; затем `snapshot` обязан нести
`/data/selector/symbol == "ETHUSDT"`. Хвост теста (`stale` — ни одного кадра цены BTC после
snapshot; `fresh` — доставка ETH непуста; setup-guard'ы) не изменён.

Оценка: прежний ассерт требовал больше, чем §2.4 (запрет — ПОСЛЕ снапшота, не до), и держался
на порядке готовых веток `tokio::select!`, который тест не контролирует. Новая форма судит ровно
договор. Ослабление компенсировано: добавлен ассерт символа в snapshot (раньше его не было —
snapshot старого селектора проходил), число предснапшотных кадров ограничено.

## §3. Мутационный критерий `R-093` §5 п.1 — исполнен reviewer'ом

Обе строки `crates/gateway-serve/src/lib.rs:2094,2787`
`let live_keeps = !drained && current_gen == Some(gen_at_pump);` заменены (sed, `replaced=2`):

```
mut=[!drained] replaced=2 run1 rc=101 :: O-12 НАРУШЕН: после смены селектора, выданной во время in-flight pump'а, пришло 1 сообщ
mut=[!drained] replaced=2 run2 rc=101 :: O-12 НАРУШЕН: ... пришло 1 сообщ
mut=[!drained] replaced=2 run3 rc=101 :: O-12 НАРУШЕН: ... пришло 1 сообщ
mut=[true]     replaced=2 run1 rc=101 :: O-12 НАРУШЕН: ... пришло 1 сообщ
mut=[true]     replaced=2 run2 rc=101 :: O-12 НАРУШЕН: ... пришло 1 сообщ
mut=[true]     replaced=2 run3 rc=101 :: O-12 НАРУШЕН: ... пришло 1 сообщ
restored (git status --porcelain пуст)
```

6/6 FAILED — мутации ловит хвост, послабление первого ассерта инвариант не открыло.

## §4. Проба порядка (воспроизведение подписи CI)

В копии теста (только в моём дереве, не коммитилась) внутри тела `o12` переставлено:
`rendezvous::test_release("w1")` → `sleep(600 мс)` → `send(subscribe("w1","ETHUSDT"))` —
завершение pump'а гарантированно раньше чтения смены. Setup-guard пробы: первая попытка
правки не применилась (строка смены не уникальна в файле — Python-assert упал), результаты той
попытки отброшены; вторая привязана к телу `o12` и напечатала применённый фрагмент:

```
1397-    rendezvous::test_release("w1");
1398:    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
1399-    send(&mut ws, subscribe("w1", "ETHUSDT")).await;
ver=f4ac015d run1 rc=101 ::  left: Some("frame")  right: Some("snapshot") test result: FAILED. 0 passed; 1 failed
ver=f4ac015d run2 rc=101 ::  left: Some("frame")  right: Some("snapshot") test result: FAILED. 0 passed; 1 failed
ver=f4ac015d run3 rc=101 ::  left: Some("frame")  right: Some("snapshot") test result: FAILED. 0 passed; 1 failed
ver=1ac11c6c run1 rc=0 :: test result: ok. 1 passed; 0 failed
ver=1ac11c6c run2 rc=0 :: test result: ok. 1 passed; 0 failed
ver=1ac11c6c run3 rc=0 :: test result: ok. 1 passed; 0 failed
```

Старая редакция детерминированно даёт подпись push-CI `38184484913` (`left: frame / right:
snapshot`), новая — зелёная. Гипотеза карточки `TD-257` подтверждена исполнением.

## §5. Done Block (сырой вывод)

```
$ git rev-parse origin/fix/td-257-o12-contract-order
1ac11c6c0918e2a3453d027cab4ed5cbbc4b9170

$ cargo test -p gateway-serve --features testing --test red_ws_session
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.61s
exit=0

$ for i in 1..25: cargo test -q -p gateway-serve --features testing --test red_ws_session o12
o12 x25: ok=25 fail=0

$ cargo fmt --all -- --check
fmt_exit=0
$ cargo clippy -p gateway-serve --all-targets --features testing -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.61s
clippy_exit=0

$ gh pr checks 345 --watch >/dev/null 2>&1; echo CHECKS_EXIT=$?
CHECKS_EXIT=0
All checks passed	pass	7s	https://github.com/a3ka/hft-platform/actions/runs/38328984632/job/115078616876

# дерево слияния с origin/main 0a907ad6 (gates.md §8, strict:false): git merge-tree → ed5e19d6, без конфликтов
$ cargo test -p gateway-serve --features testing   (на merge-preview 8e93e81d)
passed=226 failed=0 (блоков: 51)
exit=0
merge-tree o12 x10 ok=10
```

## §6. Находки

Блокеров нет.

- **Н-1 (не блокер, заведена `TD-258`, MINOR).** `crates/gateway-serve/src/lib.rs:1916` —
  комментарий «Приоритет: клиентские сообщения» над `tokio::select!` без `biased;`: приоритета
  в коде нет, готовые ветки опрашиваются в случайном порядке. Ровно этот разрыв породил
  `TD-257`: оракул был написан под обещание комментария. Договор §2.4 приоритета не требует,
  поэтому дефектом поведения это не является; дефект — ложное утверждение в коде, на которое
  опирается следующий автор оракула. Выбор (убрать/исправить комментарий либо добавить
  `biased;`) — architect'у: `biased` на ветке клиента меняет поведение под потоком сообщений
  (push-ветка может голодать), это решение о дизайне, а не правка текста. Исполнение — engine-dev
  (`crates/gateway-serve/src/**`). Reviewer фикс не проектирует (`gates.md` §4).

## §7. Вердикт

**APPROVE.** Тест судит договор §2.4, мутационный критерий `R-093` §5 п.1 держится (6/6),
подпись CI воспроизведена и снята, 13/13, o12 ×25 и ×10 на дереве слияния без падений,
«All checks passed» — зелёный. После merge — §8-гейт, `TD-257` закрыть, `TD-258` завести.
