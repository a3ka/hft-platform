<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 4b974a4039fe834a27d0866de4f05d85d1bdbb5e
audited_head: a35910164ce8cb35462e504d82acb86c9af821c7
verdict: ESCALATE
-->

# C-138 — M-68 depth-from-book, круг 2: ESCALATE

## Вердикт

**ESCALATE — не dispatch engine-dev до решения арбитра.** Это круг 2 по
`C-094`. `gates.md` §0 запрещает выносить третий `REJECT` по той же причине:
`B1`, `B2`, `B3` и `B6` ниже остаются теми же причинами, хотя rev2 назвал
их закрытыми. Кроме того, независимая проверка production-form данных показала
новую коренную проблему: `d1` моделирует обрезанный биржевой snapshot, тогда как
реальный Binance emitter строит `L2Snapshot` из локальной diff-книги ±60 %.
Следовательно, green `d1` не доказывает дефект, сформулированный в цели M-68.

Предмет — весь commit-chain
`4b974a4039fe834a27d0866de4f05d85d1bdbb5e..a35910164ce8cb35462e504d82acb86c9af821c7`,
не один текст milestone. Аудированы T2/signature decision, оба RED-набора
(`d1..d5b`, `d6`), `scripts/verify_M-68.sh` и
`milestones/M-68-depth-from-book.md` rev2. Новых contracts или trait signatures
нет; явное `N/A` с reuse `book::OrderBook::depth_within` — достаточная фиксация
T2-решения. Живые инварианты, которыми судился предмет: **VB-I-2**, **VB-I-5** и
**VB-I-10**; book replay/monotonicity также остаются в границе **BK-I-4/BK-I-6**.

## Результат проверки шести заявленных закрытий C-094

| C-094 | заявленное закрытие rev2 | исполненный результат |
|---|---|---|
| B1 | verify + явное T2 N/A | T2 N/A есть, но verify не содержит обязательного `cargo test --all`, хотя он есть в CI. Его task #0 уже не может стать GREEN от одной реализации: `d5` — assertion над compile-time const, который `clippy -D warnings` всегда запрещает. Это прежняя причина «неполный/незелёнеющий acceptance gate». |
| B2 | `d4` и шаг C против C-M68-1 | Изолированный C-M68-1 действительно даёт `d1=PASS`, `d4=FAIL (101)`: сам `d4` различает частичную проводку. Но step C verify не мутирует код и не запускает тест: он только копирует `crates` и делает `grep row.band`. Утверждённый mutation acceptance не исполняется — та же причина B2. |
| B3 | `d5`/`d5b`, version bump | `d5b` создаёт пустой каталог checkpoint и сразу зовёт `snapshot_from_checkpoint`; код берёт no-checkpoint fallback, то есть сравнивает full replay с full replay. Старый checkpoint не создаётся и не проверяется на rejection. Это та же причина B3, замаскированная названием «resume». |
| B4 | запретные границы и П-011 | Запретные строки в milestone стали явнее, но предъявленный П-011 amendment существует только в `origin/main`, не в audited chain: `21da1a8`/`dc24d64` не предки `a359101`. Поэтому он не доказывает границу предмета на данной ревизии. Удаление FA-задачи корректно снимает её scope conflict, но не материализует доказательство П-011 в цепочке. |
| B5 | compile-RED `d6` | `d6` — содержательный будущий oracle (глубина 10 против 400, setup guard и граница ×4), но сейчас не компилируется из-за отсутствующего `ReadStats.depth_levels_visited`. Его нельзя засчитать как исполненное доказательство ресурса до реализации; это не снимает более ранние блоки. |
| B6 | решение вынесено в П-018 | В audited chain всё ещё есть `1b9e0c9`, который меняет `docs/09-roadmap-v2.md` и замораживает F0/снимает её prerequisite. Указанный вынос П-018 не является предком audited head, поэтому историческая смешанная цепочка не очищена. Это та же причина B6. |

## Новое независимое основание: d1 не имеет production-form входа

`crates/venue-binance/src/lib.rs` строит `MdPayload::L2Snapshot` из состояния
локальной книги: `bucket_levels(&book.bids/asks)` и ограничивает его
`MAX_REL_DIST = 0.60`. Это подтверждает и P-011 amendment: payload не является
exchange-capped REST snapshot; фактический дефект — cadence snapshot-only, не
отсутствие дальних уровней в snapshot.

`d1`, напротив, кладёт дальний уровень только в `L2Delta` и утверждает, что
snapshot capped примерно на 1.3 %. В отдельном worktree я добавил этот дальний
уровень в snapshot payload (минимальная production-form мутация тестовой
фикстуры, код gateway не менялся). На текущей реализации `d1` стал зелёным.
Значит RED измеряет свой synthetic input, а не названный в цели production
дефект; это нарушает требование testing.md к независимому, не-плацебо оракулу.

Для checkpoint применена та же проверка на тавтологию: `d5b` никогда не
вызывает `checkpoint::advance_to`/не записывает checkpoint. Функция получает
пустой каталог и применяет fallback от `Cursor::START`; обе стороны сравнения
построены из одного журнального входа полным replay. Это именно ловушка
«двух источников, фактически собранных из одного входа».

## Что должен решить арбитр

1. Согласовать фактическую product hypothesis с P-011/venue emitter: остаётся ли
   M-68 вообще задачей «far depth absent», либо это только fix cadence на
   `L2Delta`; затем переписать objective и production-form RED соответственно.
2. Определить минимальный очищенный subject chain для B6 и требуемую форму
   доказательства П-011 в audited revision.
3. Вернуть M-68 к critic только с исполнимым acceptance: полный CI triple,
   реальный C-M68-1 mutation run, checkpoint, записанный pre-change и затем
   rejected/rebuilt, и два действительно независимых пути сходимости.

## Done Block

```text
$ git merge-base origin/main a35910164ce8cb35462e504d82acb86c9af821c7
4b974a4039fe834a27d0866de4f05d85d1bdbb5e
exit=0

$ bash scripts/verify_M-68.sh
FAIL: cargo clippy --all-targets --all-features -- -D warnings
error: this assertion has a constant value
FAIL: cargo test -p gateway --test red_depth_from_book --quiet
FAIL: cargo test -p gateway --test red_depth_recompute_cost --quiet
PASS: C setup — точка мутации найдена
FAIL: D CKPT_SCHEMA_VERSION > 2 (на момент спеки было 2)
VERDICT: FAIL (4)
exit=1

$ rg -n 'cargo (fmt|clippy|test)' scripts/verify_M-68.sh .github/workflows/ci.yml
.github/workflows/ci.yml:20:        run: cargo fmt --all -- --check
.github/workflows/ci.yml:22:        run: cargo clippy --all-targets --all-features -- -D warnings
.github/workflows/ci.yml:24:        run: cargo test --all
scripts/verify_M-68.sh:27:chk cargo fmt --all -- --check
scripts/verify_M-68.sh:28:chk cargo clippy --all-targets --all-features -- -D warnings
# no cargo test --all in verify_M-68.sh
exit=0

$ C-M68-1 isolated mutation: recompute only row.band >= 0.60 on L2Delta
md_i8_d1_far_band_counts_levels_that_only_deltas_delivered ... ok
d1_exit=0
md_i8_d4_narrow_band_moves_with_delta_not_only_far_band ... FAILED
d4_exit=101

$ production-form fixture mutation: include far level in L2Snapshot payload
md_i8_d1_far_band_counts_levels_that_only_deltas_delivered ... ok
prod_form_fixture_d1_exit=0

$ cargo test -p gateway --test red_depth_from_book md_i8_d5b_checkpoint_resume_still_equals_full_replay --quiet
test result: ok. 1 passed; 0 failed
d5b_exit=0

$ git show -s --format='%H%n%s' df7ba6e7989a1b99d880f80cf8f2c7c8be97b908
df7ba6e7989a1b99d880f80cf8f2c7c8be97b908
reserve C-138 nous 2026-08-24T21:06:00Z Ubuntu-2404-noble-amd64-base 3535726 17810fa7-1bef-4cfd-b2ac-63263d87ec44
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
exit=0
```
