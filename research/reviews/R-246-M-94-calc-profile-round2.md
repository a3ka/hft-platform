<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: df2fe01ba2b978bbb0b9777ccf36da03a5e84ea3
verdict: REJECT
-->

# R-246 — M-94 calc-profile, повторный PR-гейт (после `R-245`) — REJECT

**Предмет:** ветка `origin/feat/M-94-calc-profile`. Вершина взята командой
(`git fetch origin && git rev-parse origin/feat/M-94-calc-profile`) =
`df2fe01ba2b978bbb0b9777ccf36da03a5e84ea3`, совпала со справкой мандата (tester). Повторный
`fetch` перед записью вердикта — та же вершина. База — `045fef9` (merge-base с `origin/main`);
`main` = `6ee3f1a`, впереди на 2 коммита (только `docs/ROADMAP.md`), слияние чистое (`abbbce6`).
Работа — в detached worktree `/tmp/hft-reviewer-M-94` (ветка) и `/tmp/hft-reviewer-M-94-merge`
(дерево слияния). Новое в предмете с `R-245`: `C-287`/`C-288`/`A-051` (plan-time, закрыт
арбитром), dev-коммиты `6c9e0fc` (задача 12) и `df2fe01` (задача 13).

**Живые инварианты тронутых модулей** (диф трогает `crates/gateway/src/**`,
`crates/gateway-serve/src/**`; `crates/ops/**` — только `tests/`):
- `VB-I-2` (`docs/fa/viz-backend.md:285`, live == replay: одно определение расчёта у писателя и
  читателя слепка) — предмет находки B-1 ниже: правило величины, живущее в прогревателе
  отдельной копией, есть второе определение;
- `VB-I-9` (`docs/fa/viz-backend.md:292`, граница плоскостей) — держится:
  `grep -rnE 'postgres|sqlx|diesel' crates/gateway-serve/src crates/gateway/src | wc -l` → `0`;
- `OPS-I-11` (`docs/fa/ops.md:479`) — читатель сердцебиения цел (`o1`), сторож в откате
  восстановлен (B-2 `R-245` закрыт).
`crates/gateway` / `crates/gateway-serve` собственной FA не имеют (`reading-map.md` §2, дом — `M-81`).

**Греп яруса C** (`origin/main`): `TECH-DEBT.md` — по `M-94`, `calc-profile`, `calc_profile`,
`deploy_catchup`, `compose_deploy_form` (ноль попаданий: кандидаты `K-1`/`K-2` из `R-245` ещё не
заведены — это close-out); последний номер `TD-243`. `PROJECT-STATE.md` — по `M-94`
(ноль), `MS-STATE: M-9` (`M-90…M-93` CLOSED).

## Вердикт

**REJECT** — одна блокирующая находка, узкая и дешёвая в исправлении. Всё остальное, что
требовал `R-245`, сделано и проверено мной исполнением, включая мутации. Блокер — та же
норма, что `R-245` B-1, в месте, куда исправление не дошло: в прогревателе остались собственные
копии правил двух величин из шести. Сегодня они недостижимы и прод не ломают, но спека §5
запрещает их поимённо («второй экземпляр грамматики … в `gateway-checkpoint`»), а задача 12 —
«свои копии разбора удаляются». Dev в теле `6c9e0fc` и tester в §B/§E мандата заявили обратное.

## Блокирующие

### B-1 — в `gateway-checkpoint` остались свои копии правил таймфрейма, каденции и «полосы не NaN» (спека §5 строка 4, задача 12)

Факт на `df2fe01`:

| место | что проверяет | откуда пришло значение | достижимо? |
|---|---|---|---|
| `crates/gateway/src/bin/gateway-checkpoint.rs:439-446` (режим профиля) | таймфрейм `> 0` и делит сутки | `load_profile` → `parse_timeframe_ms` (`calc_profile.rs:240`) | нет |
| `gateway-checkpoint.rs:447-456` (режим профиля) | каденция `≥ 1000` и делит сутки | `load_profile` → `parse_depth_cadence_ms` (`calc_profile.rs:239`) | нет |
| `gateway-checkpoint.rs:459-462` (режим профиля) | полосы без NaN | `parse_bands` (конечные `> 0`) | нет |
| `gateway-checkpoint.rs:592-599` (без профиля) | таймфрейм `> 0` и делит сутки | `parse_args` → `parse_timeframe_ms` (флаг `:157-160`, окружение `:217`) | нет |
| `gateway-checkpoint.rs:634-637` (без профиля) | полосы без NaN | `parse_args` → `parse_bands` (`:222`, `:224`) | нет |

Две строки профиля добавлены этим милестоуном (`git log -S'(из профиля) не выравнен'` →
`488ee1b`), строки без профиля — наследие `M-47`/`M-38b`. Несимметрия с выдачей: тот же гвард
`M-47` в `serve_config_from_env` dev в `6c9e0fc` удалил, а в прогревателе оставил; прежний
гвард каденции без профиля (`045fef9:…/gateway-checkpoint.rs:426`) тоже удалён, гвард таймфрейма
рядом — нет.

**Почему это не косметика — воспроизведение.** На дереве слияния я ослабил ОБЩЕЕ правило
(`parse_timeframe_ms`: проверка «делит сутки» выключена) и прогнал
`cargo test -p gateway -p gateway-serve --no-fail-fast`:

```text
red_timeframe_guard_startup  misaligned_timeframe_env_blocks_startup ... FAILED   (gateway-serve)
red_timeframe_guard_startup  weekly_timeframe_env_blocks_startup     ... FAILED   (gateway-serve)
red_window_guard_startup     timeframe_guard_untouched_by_window_guard ... FAILED (gateway-serve)
крейт gateway (прогреватель, включая w1…w3)                          — ни одного FAILED
```

То есть изменение общего правила доходит до выдачи и НЕ доходит до прогревателя: тот продолжает
судить по своей копии, и ни один оракул этого не видит. Это ровно класс `TD-227` (восемь суток
отказа выдачи): при будущей законной смене грамматики выдача примет значение, прогреватель
откажет на каждом цикле cron'а, а гейт деплоя (`--print-ckpt-name` печатает имя ДО этих
гвардов, `:410-428`) пропустит. Удаление же этих строк не роняет ничего — они недостижимы,
мутацией их не нащупать.

**Почему не увидели гейты.** Проба единственности `red_m94_shared_grammar_probe.sh` вживляет
отказ В НАЧАЛО общей функции — значит доказывает, что результат вычислен ею, но не что после
неё нет второй проверки. Оракулы `w1…w3` сравнивают формы входа ОДНОГО бинаря между собой, а
`g1…g3` — режимы выдачи между собой; сравнения прогревателя с выдачей на одном значении нет.
Это не упрёк исполнителю оракулов — у находки два адресата: реализация и оракул (`testing.md`
«исправление по вердикту тоже требует оракула»). Дизайн защиты — за architect'ом (`gates.md` §4,
граница reviewer↔architect).

## Не блокирующие

- **N-1 — `R-245` N-5 не сделан.** `deploy/bin/calc-profile-gate.sh:78` — `compose run … 2>/dev/null`
  по-прежнему глотает stderr прогревателя: при отказе гейта оператор не видит причины. Кандидат в
  долг при close-out, если не будет исправлено попутно.
- **N-2 — неверное утверждение в отчёте tester'а.** §E п.6: «N-1 R-245 — НЕ правил, FA
  переписана ещё до M-94». Ложно: правка `docs/fa/viz-backend.md:42` сделана на этой ветке
  (`git diff 045fef9 df2fe01 -- docs/fa/viz-backend.md` — строка TPP заменена). Предмет от этого
  не страдает: N-1 `R-245` закрыт.
- **N-3 — `FA-WAIVER` в теле dev-коммитов барьер не читает** (`TD-165`: он ищет строку в файле
  вердикта). Безвредно; отмечаю, чтобы никто не считал эти строки предъявлением.
- **N-4 — `R-245` N-2/N-3 и кандидат K-2** (W7 судит текст; пробы деплоя вне CI; холостая
  `GATEWAY_BANDS` в `scripts/lib/compose_deploy_form.sh`) — остаются кандидатами в долг; заводятся
  при close-out, как и было объявлено.

## Что из `R-245` закрыто — проверено исполнением

| пункт | как проверено | итог |
|---|---|---|
| B-1 выдача: копии окна heatmap, шага VP, полос, таймфрейма, окна, каденции | диф `6c9e0fc`: в `serve_config_from_env` все шесть идут через `gateway::calc_profile::parse_*`; мутант «полосы без профиля — прежней копией» | `g1_agreement_g2_policy_g3_absence ... FAILED` — оракул ловит |
| B-1 загрузчик разбирает сам | `load_profile` зовёт `parse_*` (`calc_profile.rs:238-243`); окно `0` — политика, не грамматика | закрыт |
| B-2 сторож в откате по здоровью | `df2fe01` `deploy-apply.sh:105`; мутант «строка сторожа в откате закомментирована» | `a4 a4r a4s a4t a4m` FAILED (5 из 13), без мутанта 13/13 |
| N-1 FA | `docs/fa/viz-backend.md:42` называет носителем `config/calc-profile/active.env` | закрыт |
| N-4 мёртвая `applied_profile_optional_io` | удалена в `6c9e0fc` | закрыт |
| условие (а) `A-051` | `git diff --name-status 03a9dbd 9432299` → только `C-288`, `A-051`, строка §16 | соблюдено |
| sacred не тронут dev'ом | `git log 045fef9..df2fe01 -- '*/tests/*' 'scripts/tests/*'` — ни одного `[engine-dev]` | соблюдено |

**Scope.** Два новых dev-коммита трогают `crates/gateway/src/calc_profile.rs`,
`crates/gateway/src/bin/gateway-checkpoint.rs`, `crates/gateway-serve/src/lib.rs`,
`deploy/bin/deploy-apply.sh` — все в §10. Остальной диф ветки судился `R-245` (п.1) и с тех пор
изменён только наборами architect'а под plan-time гейтом (`C-287`, `C-288`, `A-051`).
**Risk-блок:** `risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic не требуется.
**Block-C:** `crates/contracts/**` не тронут. **Атомарность:** задача 12 и задача 13 — по
коммиту, ссылки на задачу и находку в subject'ах.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile origin/main
df2fe01ba2b978bbb0b9777ccf36da03a5e84ea3
6ee3f1af60fd1a482842d267d84fc625505c2db4

# ветка, df2fe01 (/tmp/hft-reviewer-M-94)
$ cargo fmt --all -- --check; echo fmt=$?
fmt=0
$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -1; echo clippy=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.43s
clippy=0
$ cargo test --all … | awk (агрегат)
test=0
passed=1233 failed=0 ignored=3 (блоков: 276)
$ bash scripts/verify_M-94.sh; echo verify=$?
PASS ×71 (task1-13 + ci-parity: учтено шагов 61 из 61, исполнено 55, исключено по карте 6)
SKIP ×7 — 6 ci-parity по карте + task11 (§8, после merge)
VERDICT: PASS
verify=0

# дерево слияния: detached df2fe01 + merge origin/main → abbbce6 (только docs/ROADMAP.md, +3)
$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

# мутации (дерево слияния; каждая откачена, git status --porcelain пуст после)
M1  deploy-apply.sh: вызов сторожа в full_rollback_prev закомментирован
    $ bash scripts/tests/red_m94_deploy_apply.sh
    FAIL a4 / a4r / a4s / a4t / a4m — «при откате сторож не переустановлен на PREV после up»
    сценариев: 13 (pass=8 fail=5)  VERDICT: FAIL  rc=1     (без мутанта: VERDICT: PASS)
M2  serve_config_from_env: полосы без профиля — прежней копией split/parse::<f64>
    $ cargo test -p gateway-serve --test red_m94_single_grammar
    g1_agreement_g2_policy_g3_absence --- FAILED (red_m94_single_grammar.rs:278)
M3  calc_profile::parse_timeframe_ms: проверка «делит сутки» выключена (B-1)
    $ cargo test -p gateway -p gateway-serve --no-fail-fast
    gateway-serve: red_timeframe_guard_startup 2 FAILED, red_window_guard_startup 1 FAILED
    gateway (прогреватель): 0 FAILED — копия в gateway-checkpoint.rs:439/:592 держит старое правило

# B-1: остаточные копии
$ grep -n '86_400_000 %\|< 1000\|is_nan' crates/gateway/src/bin/gateway-checkpoint.rs
439:    if selector.timeframe_ms <= 0 || 86_400_000 % selector.timeframe_ms != 0 {
447:    if selector.depth_cadence_ms.unwrap_or(0) < 1000
448:        || 86_400_000 % selector.depth_cadence_ms.unwrap_or(0) != 0
459:    if selector.bands.iter().any(|b| b.is_nan()) {
592:    if timeframe_ms <= 0 || 86_400_000 % timeframe_ms != 0 {
634:    if selector.bands.iter().any(|b| b.is_nan()) {
$ grep -n '86_400_000 %\|< 1000\|is_nan' crates/gateway-serve/src/lib.rs | grep -v '^[0-9]*:\s*//'
3162:        if cadence >= selector.timeframe_ms && cadence % selector.timeframe_ms != 0 {   ← межключевое, по §3.2 остаётся

# VB-I-9
$ grep -rnE 'postgres|sqlx|diesel' crates/gateway-serve/src crates/gateway/src | wc -l
0
```

## Условие APPROVED

B-1 снят — исправлением кода С оракулом на саму находку (изменение общего правила обязано
доходить до прогревателя наблюдаемо), либо явной правкой спеки architect'ом с основанием (если
он сочтёт гварды прогревателя не грамматикой, а отдельным правилом — тогда §5 говорит об этом
прямо). Затем tester на новой вершине и повторный PR-гейт. Задача 11 (удаление строки
`GATEWAY_BANDS` из host `.env`) **НЕ исполнялась**: merge не состоялся, текущему проду строка
нужна (замер: `grep -c '^GATEWAY_BANDS=' /root/hft-platform/.env` → `1`).

**Маршрут — внимание `gates.md` §0 п.1.** Это второй REJECT подряд на PR-гейте по одной причине
(«второй экземпляр правила величины», `R-245` B-1 → здесь, в ином месте). Предикат «одна
причина» — суждение арбитра, не моё и не architect'а (`A-051` §1.2); называю, чтобы его не
пропустили.

## Handoff §D

Следующий агент — **architect** (`.claude/agents/reviewer.md` §Handoff: разбор REJECT — его).
