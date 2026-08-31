<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: a4d51613f5c70dd98e0c8daf8f74f0e5e02893f8
audited_head: 2cd39a40a0379f02700f2c81e19048f15150ba2e
verdict: REJECT
-->

# C-185 — M-68: FA viz-backend close — REJECT

## Предмет и граница аудита

Сужу финальное дерево merge-коммита `2cd39a4`, а не только текст плана. Его второй
родитель — `origin/main` `a4d5161`; именно разность `2cd39a4^2..2cd39a4` содержит три
заявленных файла и `+309/-7`.

Указанная в handoff формула
`git diff $(git merge-base 2cd39a4 origin/main)..3a88f90` не является этим диапазоном:
она сравнивает `a4d5161` с предком merge `3a88f90` и показывает 37 файлов. Это NOTE к
воспроизводимости handoff, не основание вердикта: содержание трёх файлов в `3a88f90` и в
финальном merge-дереве совпадает.

## Вердикт: REJECT

### B-1 — один FA одновременно объявляет `TD-158` закрытым и OPEN

`docs/fa/viz-backend.md:163-179` говорит, что M-68 закрыл предусловие (б), исполнил
`П-014` п.2 и закрыл `TD-158`. Но тот же действующий FA:

- в таблице статуса, `:42`, держит `TD-158` среди причин, по которым прод не включает
  полосы;
- в Cross-references, `:277-279`, дословно называет `TD-158` «каденция, OPEN».

Это не историческая справка: обе строки находятся в живых статусных разделах, без
датирующей оговорки. Авторитетные состояния противоречат им: `PROJECT-STATE.md:2022-2025`
объявляет `TD-158` закрытым обеими половинами, а `TECH-DEBT.md:58-59` подтверждает его
перенос в архив. Таким образом новая правка N-13 не довела документ до единственной
истины о закрытом долге.

Условие снятия: architect должен привести все живые статусы FA к одному состоянию
`TD-158`, не убирая при этом `TD-159` как блокер `П-014` п.4 и не превращая закрытие (б)
в разрешение менять `GATEWAY_BANDS`.

### B-2 — определение `MD-I-8` не покрывает корпус, который оно объявляет одним инвариантом

Дом семейства выбран верно: это выдаваемая депт-серия на стыке `gateway`↔`book`, а не
примитив реконструкции книги из `fa/book.md`. Разрыв `MD-I-1..7` также допустим: это
объявленная дыра, а не коллизия идентификатора.

Но единственная строка определения (`docs/fa/viz-backend.md:214`) описывает пересчёт от
книги, каждую полосу, конфиг и его объявление, стартовую валидацию, инвалидацию чекпоинта
и live==replay. В наборе, который сам milestone и verify называют `MD-I-8`, остаются
несформулированными самостоятельные обязательства:

| Оракул | Что он пиннит | Чего нет в определении `:214` |
|---|---|---|
| `d5` | snapshot после delta обязан **заменять**, а не merge-ить книгу | resync/replace-семантика |
| `d6a`/`d6b` | счётчик честен и семь полос стоят один проход книги | граница работы / один проход |
| `d7`/`d7b` | провенанс снят из того же наблюдения, что и число, в обе стороны | связка число↔провенанс |
| `d8b` | реально прочитанный checkpoint на delta-хвосте равен full replay | warm-start/фактическое чтение checkpoint |

Это не придирка к числу ассертов: `scripts/verify_M-68.sh:102-106,238-243` включает эти
суиты в контракт milestone, а `milestones/M-68-depth-from-book.md:1006-1016` называет их
частями набора. В коде 52 строки несут метку `MD-I-8`, но `DESIGN` §22 теперь записывает
«заявлено 1 / в оракулах 1». Без полного текста либо явной карты «oracle → обязательство
инварианта» эта строка снова превращает набор оракулов в ярлык — ровно дефект R-154 N-9.

Условие снятия: architect выбирает и предъявляет проверяемую модель покрытия — дополнить
`MD-I-8` всеми обязательствами корпуса либо разделить их на корректно определённые
инварианты — и согласует с ней счёт `DESIGN` §22 и состав verify. Это не поручение
критика проектировать форму решения.

## Проверенные неблокирующие пункты

- `GATEWAY_SCHEMA_VERSION` действительно равен 9; точная старая фраза
  «депт-серия остаётся snapshot-only» из исходников снята. Оставшиеся три употребления
  `snapshot-only` в `gateway/src/lib.rs` — историческое описание прежней семантики либо
  явное отрицание, не действующее самоописание.
- `TD-159` остаётся OPEN и блокирует `П-014` п.4; `TD-161` остаётся OPEN. `M-68` и его
  verify по-прежнему запрещают изменение `GATEWAY_BANDS`; закрытие (б) не выдано за
  разрешение включить полосы.
- Диапазон не меняет `docs/PENDING-SIGNATURE.md`, решения founder'а, `GATEWAY_BANDS` или
  T1/contracts. Новая запись лишь ссылается на уже подписанные `П-013`/`П-014` и не
  дописывает подпись founder'а.
- Правка устраняет конкретный обратный дрейф `MD-I`: теперь у него есть дом и определение.
  Она не устраняет общий обратный дрейф: FA для `gateway`, `gateway-serve`, `derive` и
  `recorder` отсутствуют, а в коде остаются `GW-I-1..12,14` (217 строк-носителей).
  Документ честно оставляет это отдельным долгом; считать правку системным лечением
  обратного дрейфа нельзя.

## Done Block

```text
$ git rev-parse 2cd39a4^2
a4d51613f5c70dd98e0c8daf8f74f0e5e02893f8
$ git rev-parse 2cd39a4
2cd39a40a0379f02700f2c81e19048f15150ba2e
$ git diff --name-status 2cd39a4^2..2cd39a4
M docs/DESIGN.md
M docs/fa/viz-backend.md
A docs/plans/fa-viz-backend-audit-2026-08-25.md
exit=0

$ git diff --name-only $(git merge-base 2cd39a4 origin/main)..3a88f90 | wc -l
37
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: MD-I — заявлено=1, в оракулах=1 — подтверждено замером (loose=1)
PASS  [3-ССЫЛКИ] все 7 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 341 ссылок вида docs/*.md указывают на существующие файлы
PASS  [6-RFC-SHA] SHA-подобных токенов: всего=38 проверено=38 пропущено=0
PASS  [7-RFC-PATH] путей-кандидатов: всего=274 проверено=182 пропущено=92
VERDICT: PASS (0 нарушений)
exit=0

$ rg -n '^pub const GATEWAY_SCHEMA_VERSION|GATEWAY_SCHEMA_VERSION\s*=\s*9' crates/gateway/src/lib.rs
85:pub const GATEWAY_SCHEMA_VERSION: u32 = 9;
exit=0

$ rg -n --glob '*.rs' '\bMD-I-8\b' crates | wc -l
52
exit=0

$ rg -nF 'депт-серия остаётся snapshot-only' crates/gateway/src crates/gateway-serve/src
exit=1  # no exact stale phrase; expected no-match

$ rg -n 'TD-158' PROJECT-STATE.md TECH-DEBT.md docs/fa/viz-backend.md
docs/fa/viz-backend.md:168:    выдаче и отвергает непредставимую каденцию на старте; `TD-158` закрывается его
docs/fa/viz-backend.md:278:  2026-08-17 как ЛОЖНАЯ карточка) · `TD-158` (каденция, OPEN) · `TD-159` (метка одна на ряд
PROJECT-STATE.md:2022:**`TD-158` ЗАКРЫТ этим милестоуном** — обе его половины: отставание 1 Гц против дельт 100 мс
TECH-DEBT.md:58:> → **47** (42 прежних + шесть новых `TD-185`…`TD-190` − закрытый `TD-158`; close-out M-68,
td158_state_probe_exit=0

$ bash scripts/verify_M-68.sh 2>&1 | grep -E '^(===|PASS:|FAIL:|VERDICT:)'
=== task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all ===
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
=== A (задачи 1,2,3,4,5,7) — набор MD-I-8 целиком ===
PASS: cargo test -p gateway --test red_depth_from_book --quiet
PASS: A состав набора — 9 оракулов
=== B (задача 4) — мутационный контроль ИСПОЛНЯЕТСЯ: набор обязан быть КРАСНЫМ против C-M68-1 ===
PASS: B набор КРАСЕН против мутанта C-M68-1
=== C (задача 8) — ресурсный оракул пути L2Delta → depth ===
PASS: cargo test -p gateway --test red_depth_recompute_cost --quiet
=== C2 (задачи 13,14 — R-134 B-3/B-4) — вырожденный вход и честность счётчика ===
PASS: cargo test -p gateway --test red_depth_semantics --quiet
PASS: C2 состав набора — 3 оракулов
=== C3ter (задача 23 — R-141 Б-1) — ОРАКУЛ ТОЧКИ ВХОДА: прод-писатель и прод-читатель находят ОДИН слепок ===
PASS: cargo test -p gateway --test red_checkpoint_bin_prod_argv --quiet
PASS: C3ter состав набора — 8 оракулов
=== C3bis (задачи 22 И 24 — R-138 Б-3, R-141 Б-3) — ручка каденции есть КОНФИГ ===
PASS: cargo test -p gateway-serve --test red_depth_cadence_from_env --quiet
PASS: C3bis состав набора — 6 оракулов
=== C3 (задачи 15,16 + C-167) — каденция управляет, объявлена, представима, инвалидирует чекпоинт ===
PASS: cargo test -p gateway --test red_depth_cadence --quiet
PASS: C3 состав набора — 7 оракулов
=== C4 (задача 12 — R-134 B-2(ii)) — самоописание кода не расходится с кодом ===
PASS: C4 самоописание согласовано (обещаний=0, собственных материализаций=2)
PASS: C4 ложное самоописание снято — снятая snapshot-only семантика поля depth_reach_bid (lib.rs:636-658)
PASS: C4 ложное самоописание снято — то же, вторая половина того же комментария
PASS: C4 ложное самоописание снято — ложное «как прежний depth_within с None mid» (lib.rs:1134-1136)
=== D (задача 9) — смена СЕМАНТИКИ объявлена bump'ом GATEWAY_SCHEMA_VERSION ===
PASS: D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)
PASS: cargo test -p gateway --test red_gateway_schema_version --quiet
=== E (задача 10) — VB-I-10 не ослаблен переходом на пересчёт по книге ===
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
=== F (задача 6) — VB-I-2 live == replay ===
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
=== G (задача 7) — метка и её числа сняты одним наблюдением; соседний инвариант не куплен ===
PASS: cargo test -p gateway --test red_depth_provenance_by_reach --quiet
=== H — Block-C: contracts не тронуты предметом ===
PASS: H crates/contracts не тронут
=== I — состав ВЫДАЧИ не тронут: GATEWAY_BANDS остаётся прод-дефолтом ===
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут
=== J (C-094 B3) — selector_fingerprint не подогнан под кэш ===
PASS: J selector_fingerprint не переписан
=== K — зона предмета: чужие крейты и роадмап в диапазоне не участвуют ===
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: PASS
verify_M-68_exit=0

$ git diff --check 2cd39a4^2..2cd39a4
exit=0
```
