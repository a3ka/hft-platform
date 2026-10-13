<!-- GATE-META
milestone: TD-242
audited_repo: a3ka/hft-platform
audited_base: 3670d4b3e16bb99dedb3dc4c081256b70a1eedbd
audited_head: c676e1bc959196705cb544a0b284fa6a028a3d5c
verdict: APPROVE
-->

# R-235 — TD-242: текст отказа `unsupported` называет оси `admit()` — APPROVE

**Роль:** reviewer. **Дата:** 2026-10-04. **Предмет:** ветка `origin/fix/TD-242-unsupported-msg`,
один коммит `c676e1b` (engine-dev, один файл `crates/gateway-serve/src/lib.rs`, +8/−2).
Карточка: `TECH-DEBT.md` `TD-242` (Ф2, MINOR, зона engine-dev), источник `R-230` Б-2 / `R-232` Н-3.

## Вердикт

**APPROVE** с двумя незаблокирующими замечаниями (§5 Н-1, Н-2), переданными architect'у.

## §0. Предмет — вершина взята командой (`04-workflow.md` §2)

Мандат называл локальную ветку `engine-dev-hft-engine-dev-1791149558` в worktree
`/tmp/hft-engine-dev-1791149558`, HEAD `c676e1b`. **В `origin` ветки не было** —
`git rev-parse origin/engine-dev-hft-engine-dev-1791149558` → `unknown revision`. Это нарушение
dev'ом `gates.md` §8 («push на origin ПЕРЕД handoff — hard-precondition»), см. §5 Н-3. Ветку
опубликовал reviewer после проверки push-scope (только предмет), под осмысленным именем:

```
$ git log origin/main..HEAD --format='%h %s'          # в worktree dev'а
c676e1b fix(TD-242): точный текст отказа unsupported — bands/symbol/timeframe_ms
$ git push origin HEAD:refs/heads/fix/TD-242-unsupported-msg
 * [new branch]      HEAD -> fix/TD-242-unsupported-msg
$ git fetch origin -q && git rev-parse origin/fix/TD-242-unsupported-msg origin/main
c676e1bc959196705cb544a0b284fa6a028a3d5c      ← совпадает со справкой мандата (c676e1b)
3670d4b3e16bb99dedb3dc4c081256b70a1eedbd
$ git merge-base --is-ancestor origin/main origin/fix/TD-242-unsupported-msg && echo ancestor
ancestor                                       ← ветка от свежего main, merge-preview = сама ветка
```

## §1. Block-scope

```
$ git show --numstat --format='' c676e1b
8       2       crates/gateway-serve/src/lib.rs
$ git diff --stat origin/main...c676e1b -- '*/tests/*'
(пусто)
```

- Зона: `crates/gateway-serve/src/**` — engine-dev (`scope-guard.md`, таблица владения). Совпадает
  с зоной карточки. Тесты, `contracts`, `risk`/`killswitch`/`oms`/`venue-*` — не тронуты.
- Block-C: `crates/contracts/**` не тронут — N/A.
- Block-risk: путь — read-only выдача (`gateway-serve`), order-egress нет — risk-critic не требуется
  (`gates.md` §5).
- RED-first: `*/tests/` не тронуты (sacred цел).
- Атомарность: один коммит на одну карточку, ссылка `TD-242` в subject'е, метка роли `[engine-dev]`,
  без co-author трейлера.

## §2. Суть правки — сверено ОТКРЫТИЕМ кода, не по пересказу dev'а

`crates/gateway-serve/src/admission.rs:118-134` (`admit`) на вершине:

```
122:    if !bands_subset(&sel.bands, &policy.canonical_bands) {      → Unsupported
126:    let sym_ok = policy.allowed_symbols.iter().any(|s| s == &sel.symbol);
127:    if !sym_ok {                                                  → Unsupported
131:    if !profile_allowed(policy, sel) {                            → Unsupported
```

`profile_allowed` (`:146-156`) судит только `timeframe_ms`; `window_ms`/`depth_cadence_ms` там
прямо названы ограничениями следующего слоя. Значит, оси отказа — ровно `bands`, `symbol`,
`timeframe_ms`. Новая строка `lib.rs:1185` — «selector вне политики допуска
(bands/symbol/timeframe_ms)» — совпадает с кодом; ложная ось `window` убрана, пропущенная
`symbol` добавлена. Комментарий над строкой (`:1179-1184`) ссылается на `admission.rs:122-131` —
диапазон верен (три `if` проверки).

Поведение не тронуто: `metrics::inc_refusals_unsupported` остался на `:1178`, код WS-сообщения
`"unsupported"` и `return Err(...)` — те же. Старый текст в коде не остался:

```
$ grep -rn 'band/timeframe/window' crates/ deploy/ scripts/
(пусто)
```

Потребителей, матчащих текст `msg`, нет (`grep -rn 'вне политики допуска' crates/*/tests` — пусто).

## §3. Block-DoneBlock — прогнано reviewer'ом самостоятельно

```
$ cd /tmp/hft-engine-dev-1791149558   # HEAD c676e1b
$ cargo fmt --all -- --check >/dev/null 2>&1; echo fmt-exit=$?
fmt-exit=0
$ cargo clippy -p gateway-serve --all-targets -- -D warnings 2>&1 | tail -1; echo clippy-exit=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
clippy-exit=0
$ cargo test -p gateway-serve > td242_test.log; echo test-exit=$?
test-exit=0
$ grep -E "^test result" td242_test.log | awk '{p+=$4; f+=$6; i+=$8} END {print "passed="p" failed="f" ignored="i" (блоков: "NR")"}'
passed=203 failed=0 ignored=0 (блоков: 46)
```

Расхождение с Done Block dev'а (`passed=178`, 45 блоков) — в счёте, не в исходе: оба прогона
`failed=0`, exit 0. Верным считается прогон reviewer'а. Полный workspace-CI (fmt/clippy
`--all-features`/test `--all`) прогоняет PR-агрегат `All checks passed` — merge идёт только по нему.

verify-скрипта нет: долговая карточка MINOR, не milestone с acceptance-гейтом.

## §4. Предъявление FA (M-66) — инвариант тронутого модуля

Диф трогает `crates/gateway-serve/src/**`. Барьер `scripts/check_review_fa.sh:195-197` сопоставляет
этому крейту FA `docs/fa/viz-backend.md` с префиксом `GS`. Живые ID на вершине:
`grep -noE '\bGS-I-[0-9]+\b' docs/fa/viz-backend.md` → `GS-I-1`, `GS-I-2`, `GS-I-4`.

- **`GS-I-1`** (= `VB-I-9a`, граница плоскостей: `gateway-serve` не читает/не пишет application-БД,
  auth — только stateless JWT; `docs/fa/viz-backend.md:292`, `:369`) — правка его не задевает:
  строковый литерал и комментарий, новых импортов нет.
- Предметный инвариант правки живёт НЕ в `viz-backend.md`, а в `docs/fa/ops.md:479` **`OPS-I-11`**
  («три проверки `admit()` — `bands`, `symbol`, `timeframe_ms`»): после правки текст отказа и
  `OPS-I-11` говорят одно и то же — это и было условием закрытия карточки.

**Поправка к §E handoff'а dev'а:** утверждение «`check_review_fa.sh:57` пропустит, у
`gateway-serve` FA нет» — неверно. `SKIP` на `:57` срабатывает, только если диапазон не трогает
прод-код крейтов; здесь тронут `crates/gateway-serve/src/**`, и FA у крейта есть
(`viz-backend.md`, префикс `GS`). Требование закрывает этот вердикт, а не пропуск барьера.

## §5. Замечания (не блокируют)

**Н-1 — правка сдвинула ссылку на строку в живой FA.** `docs/fa/ops.md:479` (`OPS-I-11`) говорит:
«`overloaded`, `crates/gateway-serve/src/lib.rs:1228` идут в `refusals_supported`». На `origin/main`
это верно (`:1228` = `metrics::inc_refusals_supported`). Правка добавила 6 строк выше, и на вершине
ветки вызов стоит на `:1234`, а `:1228` — строка комментария:

```
$ sed -n '1228p;1234p' crates/gateway-serve/src/lib.rs          # на c676e1b
                                 // приобретение/освобождение гарантирована
                                 metrics::inc_refusals_supported(inner.counters.as_ref());
```

Ссылка `:1178` в той же строке FA осталась верной (сдвиг ниже неё). Зона FA — architect;
reviewer её не правит. Тот же класс, что `TD-236` (в): номер строки, инвалидированный соседней
правкой. Передано architect'у в handoff: `:1228` → `:1234` (или имя ветки вместо номера).

**Н-2 — у текста отказа нет оракула.** Мутация «вернуть старую строку» не роняет ни один тест:
`grep -rn 'вне политики допуска' crates/*/tests` пусто — оракулы `red_m87_admission.rs` сверяют
enum-исход и код `"unsupported"`, не `msg`. Карточка прямо оставила «тест на текст — по решению
architect'а», поэтому это не блокер. Остаточный риск назван: следующая правка `admit()` (новая ось)
снова разойдётся с текстом молча. Решение — architect (оракул на соответствие осей либо принятие
риска).

**Н-3 — dev не запушил ветку перед handoff.** `gates.md` §8: «Коммит, живущий только в worktree,
для следующего агента не существует». Ветку опубликовал reviewer (§0). Процессная находка без
последствий для предмета.

## §6. Что искал грепом в ярусе C

`TECH-DEBT.md`: `TD-242` (строки 219, 5552 — карточка и тело). `PROJECT-STATE.md`: `TD-242`
(строка 3084 — запись `R-233`). `research/reviews/`: `TD-242` → `R-233`. `docs/fa/ops.md`:
`OPS-I-11`, `lib.rs:12`.

## §7. Условие и маршрут

APPROVE → PR в `main` с этим вердиктом в диапазоне → зелёный `All checks passed` → merge →
§8-деплой (`gateway-serve` пересобирается) → close-out: `TD-242` закрыть в `TECH-DEBT.md`
(тело — в архив), запись в `PROJECT-STATE.md`.
