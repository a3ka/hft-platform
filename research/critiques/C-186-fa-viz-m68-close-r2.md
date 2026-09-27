<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: a4d51613f5c70dd98e0c8daf8f74f0e5e02893f8
audited_head: 299c05ff97240b678d780e22b8d729b70200f4cb
verdict: NOTE
-->

# C-186 — M-68 FA viz-backend close, круг 2 — NOTE

## Предмет и граница аудита

Сужу закоммиченную вершину `299c05f` ветки
`docs/fa-viz-M-68-close`, а не handoff-текст. Это круг 2 только по
закрытию B-1/B-2 из `C-185`; прошлые находки не открываются заново.
База аудита — `origin/main` `a4d5161`.

Наследованный набор M-68 присутствует: milestone, реальный acceptance-гейт и четыре
RED-файла. Milestone явно объявляет два публичных T2-поля
(`Selector::depth_cadence_ms`, `SeriesBundle::cadence_ms`) и отсутствие T1-изменения;
в диапазоне закрытия нет ни `contracts`, ни `crates/*/src`, ни verify-скрипта.

## Вердикт: NOTE

Оба блокера `C-185` сняты. NOTE не блокирует приземление предмета.

### B-1 — снят

`TD-158` больше не имеет живого состояния OPEN в FA: статус TPP называет его закрытым,
а cross-reference фиксирует close-out. `TD-159` по-прежнему явно OPEN и блокирует
`П-014` п.4; `TD-161` остаётся названным отдельным расхождением. Диапазон не меняет
`docker-compose.yml`, следовательно не выдаёт закрытие предусловия (б) за разрешение
поменять `GATEWAY_BANDS` — граница C остаётся на месте.

### B-2 — снят

`MD-I-8` теперь определён как составной инвариант с десятью обязательствами и картой
"обязательство → оракул". Независимый пересчёт нашёл 21 `md_i8_*`-оракул в четырёх
файлах; все 21 ID карты существуют, а разность множества кода и карты пуста. Строка
`DESIGN` §22 честно считает идентификаторы (`1 / 1`), отдельно называя корпус из 21
оракула, поэтому не выдаёт один ID за одну проверку.

### NOTE-1 — граница корпуса стартовой валидации названа честно

Строка 7 карты включает `d14`, а связанные входные оракулы `d18e/d18f/d18g` намеренно
не несут метку `MD-I-8` и не входят в счёт 21. Это не скрытая неполнота: FA прямо
называет оба файла, нулевую маркировку и то, что без них обязательство покрыто лишь
наполовину. Acceptance-гейт также запускает эти три оракула в C3ter/C3bis. Если в будущем
будет заявлено *полное* покрытие стартовой валидации корпусом `MD-I-8`, надо либо
промаркировать эти оракулы, либо оформить отдельный entrypoint-инвариант; текущая
формулировка такой ложной полноты не заявляет.

### Проверенные неблокирующие условия

- Утверждение о цене переименования воспроизводится: в `crates/**` 52 вхождения
  `MD-I-8`, включая 7 в engine-dev `src`-зонах. Это поддерживает выбор карты без
  вторжения architect в чужую зону; это не самостоятельный замер стоимости будущего рефакторинга.
- `VB-I-5` остаётся живым инвариантом тронутого предмета: серия глубже 1.3 % без
  `depth_band_provenance` невалидна (`docs/fa/viz-backend.md:199`).
- `verify_design_claims` на merge-preview и полный `verify_M-68.sh` завершились PASS.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-186
exit=0

$ git rev-parse origin/main; git rev-parse HEAD; git merge-base HEAD origin/main
a4d51613f5c70dd98e0c8daf8f74f0e5e02893f8
299c05ff97240b678d780e22b8d729b70200f4cb
a4d51613f5c70dd98e0c8daf8f74f0e5e02893f8
exit=0

$ git diff --name-only 2cd39a4..HEAD
docs/DESIGN.md
docs/fa/viz-backend.md
research/critiques/C-185-fa-viz-m68-close.md
exit=0

$ rg -n 'T1|T2|trait|Trait|Contract impact|RED-тест|Acceptance' milestones/M-68-depth-from-book.md
793:`research/{critiques,arbitration}/**` ... **T2-поля по `A-024` `O-1`
805:**T2-контракт / сигнатура — решение принято явно, а не оставлено догадкой dev'а (`C-094` B1):**
814:| `Selector::depth_cadence_ms` | **T2** — публичное поле публичного типа | **architect** ... |
815:| `SeriesBundle::cadence_ms` | **T2** — публичное поле публичного типа | **architect** ... |
817:T1 предмет не трогает: форма события не меняется, читаем то, что уже пишется (`CT-I-2`).
1062:## 6. Acceptance — `scripts/verify_M-68.sh`
exit=0

$ grep -rhoE '^fn md_i8_[a-z0-9_]+' crates/*/tests/*.rs | wc -l
21
exit=0

$ comm -3 <(code IDs) <(21 IDs из карты C-186)
exit=0  # empty: no missing map oracle and no unmapped md_i8 oracle

$ rg -n 'TD-158.*OPEN|OPEN.*TD-158' docs/fa/viz-backend.md
exit=1  # expected: no live FA state declares TD-158 OPEN

$ git diff --unified=0 2cd39a4..HEAD -- docker-compose.yml | grep -E '^[+-][^+-]' | grep -q 'GATEWAY_BANDS'
exit=1  # expected: GATEWAY_BANDS unchanged

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: MD-I — заявлено=1, в оракулах=1 — подтверждено замером (loose=1)
PASS  [3-ССЫЛКИ] все 7 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 341 ссылок вида docs/*.md указывают на существующие файлы
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_M-68.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
PASS: A состав набора — 9 оракулов (ожидалось ровно 9: d1 d2 d3 d4 d5 d7 d7b d8 d8b)
PASS: C3ter состав набора — 8 оракулов (... d18g_garbage_cadence_is_rejected_naming_the_variable)
PASS: C3bis состав набора — 6 оракулов (... d18e невыравненная пара, d18f carve-out)
PASS: C3 состав набора — 7 оракулов (ожидалось ровно 7: d12 d13 d14 d15 d16 d17 d20)
PASS: H crates/contracts не тронут
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут (судятся только изменённые строки)
VERDICT: PASS
exit=0

$ git diff --check 2cd39a4..HEAD
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-30T14:30Z
- Milestone: M-68
- Статус: DONE — NOTE, предмет не блокируется
- HEAD: 299c05f — docs(fa): C-185 B-1/B-2 — статус TD-158 сведён к одному, MD-I-8 получил карту покрытия [architect]

## §B — Что я сделал
- Независимо проверил закрытие B-1/B-2 из C-185 на закоммиченной вершине и дереве слияния.
- Сверил T2/trait-декларации, RED-корпус, verify и milestone; прогнал acceptance-гейт.

## §C — Артефакты / результаты
- `research/critiques/C-186-fa-viz-m68-close-r2.md`
- Done Block: `verify_design_claims` exit=0; `verify_M-68.sh` exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  C-186 вынес NOTE по M-68 FA close на 299c05f: оба блока C-185 сняты, NOTE-1 только фиксирует честно названную границу d18e/d18f/d18g вне счёта 21. Используй C-186 как gate artifact, затем передай документ на обычный PR-time reviewer-маршрут; не меняй GATEWAY_BANDS и не ослабляй TD-159/TD-161.
  ```
- Push-статус: ⏸ commit and push are performed with this gate artifact
- Кэш: ⏸ кэш оставлен — worktree ещё нужен для commit/push гейта

## §E — Риски / открытые вопросы
- NOTE-1: не объявлять корпус из 21 оракулов полным покрытием entrypoint-валидации, пока d18e/d18f/d18g остаются вне идентификатора.

=== END HANDOFF ===
