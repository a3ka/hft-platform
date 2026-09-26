<!-- GATE-META
milestone: M-86
audited_repo: a3ka/hft-platform
audited_base: 0f55ea7577386c08c22b4cedd428b354308db52e
audited_head: 69c6ecc1497805e64c65b7545786bb011a001f7f
verdict: REJECT
-->

# C-230 — M-86 fixed VP bin width, круг 2 — REJECT

## Предмет

Проверен закоммиченный набор `0f55ea7..69c6ecc` на
`feat/M-86-vp-bin-width`, включая три architect-коммита поверх `C-229`:
milestone, весь RED-набор, `verify_M-86.sh`, `docs/ROADMAP.md` и их поведение
на merge-preview с `origin/main`.

T1 не меняется: в диапазоне нет `crates/contracts/**`, contract-RFC и ревизия
workspace-`match` не требуются. Новых trait-сигнатур нет; T2 API
`DEFAULT_VP_BIN_WIDTH_E8` / getter / setter объявлены в milestone и RED-оракулах.

Живой инвариант затронутого read-path: **VB-I-2** (`docs/fa/viz-backend.md:200`) —
`live == replay`. M-86 обязан сохранить его и одновременно не ослабить **VB-I-10**
(эвикция прошлой VP-сессии в bounded-window).

## Вердикт: REJECT

`R1`–`R6` из `C-229` закрыты. Но dev нельзя диспетчеризовать: два обязательных
architect-артефакта всё ещё неполны.

### Закрытие C-229

| Находка | Результат круга 2 |
|---|---|
| R1 | **CLOSED.** §2.2 задаёт `div_euclid`; `V2-b` пиннит `-1 → -W` и `0 → 0` с сохранением нулевого объёма. |
| R2 | **CLOSED.** `V1b` строит `BTreeSet` ожидаемых ключей из входных цен и сверяет точное множество с выдачей. Изолированный мутант `10·W` упал: `2043` ключа против канонических `20417`, exit 101. |
| R3 | **CLOSED.** `V7` вызывает `LiveReducer::resume`, `pump` до хвоста и сравнивает с независимым полным replay; шаг E перечисляет обязательства по именам, а не по счётчику. |
| R4 | **CLOSED.** `red_vp_bin_width_bridge.rs::v5b_*` наблюдает effective width после `serve_config_from_env` и сохраняет прежнее значение после отвергнутого старта. |
| R5 | **CLOSED.** Шаг I хеширует тело `selector_fingerprint`, F требует ровно `25000000`, J мутирует оба входа и наблюдает различие. |
| R6 | **CLOSED.** `docs/ROADMAP.md` внесён только в architect `Allowed paths`; dev-зона не расширена. |

### B1 — отсутствует обязательная architect-правка FA `VP-I-4`

M-86 task #6 требует до dev уточнить в `docs/fa/viz-backend.md`, что VP-корзина —
диапазон, а не точная цена. Файл отсутствует в диапазоне `87ca112..69c6ecc`, и
`rg -n 'VP-I-4' docs/fa/viz-backend.md` не находит ни одной строки. Это не ожидаемый
RED реализации: путь принадлежит architect, а шаг G собственного acceptance-скрипта
красный ровно по двум проверкам `VP-I-4`.

Пока инвариант не закоммичен, dev получает противоречие: существующий `VB-I-8` требует
не выдумывать цены, а новая форма меняет семантику ключа без нормативного определения.

**Условие снятия:** architect коммитит task #6 в FA с живым `VP-I-4`, определяющим
корзину как диапазон и запрещающим пустые выдуманные корзины; G обязан стать зелёным
без правки dev-кода.

### B2 — V1 не предъявляет эвикцию прошлой сессии

§7.1 milestone требует две сессии в журнале: текущую в кадре и прошлую, эвиктнутую
оконным правилом. `red_vp_bin_width_size.rs` создаёт только текущую: все timestamps
лежат в `[SESSION_START_MS, SESSION_START_MS + 86_000_000)`, а затем лишь проверяется
одна строка VP. Поэтому реализация со сломанным whole-session drop
`session_max_time_s < lo_time_s` неотличима от правильной на этой фикстуре. Это прямо
оставляет без RED-защиты затронутый `VB-I-10` и может занизить измеренную цену кадра
относительно продовой формы.

Разнос торгов сам по себе предмет не ослабил: изолированная мутация «сетка = тик» дала
`4_718_967 B > 2_000_000 B`, V1 упал (exit 101). Но она не заменяет отсутствующую
прошлую сессию.

**Условие снятия:** V1 добавляет торговлю в предыдущей UTC-сессии, оставляет текущую
прод-геометрию и setup-стражом доказывает, что в итоговом bounded-window осталась ровно
текущая VP-сессия. Мутант без whole-session eviction обязан краснеть.

## §7.0bis — разделение труда V7/V8

Согласен. В изолированном мутанте без применения сетки `V7` и `V8` оба зелёные (по одному
passed, exit 0): это верное следствие того, что они проверяют равенство двух путей, а не
форму корзины. Дыры не остаётся, только если их не используют вместо оракулов формы:
`V1b` и `V2/V2-b` пиннят наличие и точность сетки, а `V7/V8` — её согласованность на
live/replay и snapshot/frame путях. R2-проба выше подтверждает различающую силу V1b.

## NOTE

§0 и §7.2bis milestone называют для `10·W` **2042** корзины. В воспроизведённом мутанте
на фактической V1-фикстуре их **2043**; защитное сравнение множеств корректно, но
иллюстративную арифметику следует привести к исполняемой фикстуре при исправлении B2.

## Done Block

```text
$ git rev-parse HEAD; git merge-base HEAD origin/main
69c6ecc1497805e64c65b7545786bb011a001f7f
0f55ea7577386c08c22b4cedd428b354308db52e
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [1-ЕСТЬ] все 3 маркеров [ЕСТЬ] в таблицах статусов сопровождены существующим пруфом
PASS  [2-ПОКРЫТИЕ] §22: VB-I — заявлено=11, в оракулах=8 — подтверждено замером
PASS  [7-RFC-PATH] путей-кандидатов: все проверенные существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_M-86.sh | grep -E '^(PASS|FAIL|VERDICT)'; echo exit=$?
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
PASS: обязательство v1_ предъявлено
PASS: обязательство v1b предъявлено
PASS: обязательство v2b предъявлено
PASS: обязательство v7_ предъявлено
PASS: обязательство v8_ предъявлено
FAIL: docker-compose.yml объявляет GATEWAY_VP_BIN_WIDTH_E8 с дефолтом
FAIL: docs/fa/viz-backend.md вообще называет VP-I-4 (сегодня инвариант живёт только в архивной спеке)
FAIL: VP-I-4 в FA описывает корзину профиля как ДИАПАЗОН, а не как точку
PASS: тело selector_fingerprint не тронуто (sha256 e99e808bbce3b2f98a24bc3317230e7896439f4e39616c8ee3e54cb2ec44e181)
PASS: сторож тела fingerprint РАЗЛИЧАЕТ мутацию (внедрённый hash ширины меняет sha256)
PASS: сторож дефолта РАЗЛИЧАЕТ выключающее значение (:-2 отвергается литералом 25000000)
VERDICT: FAIL (12)
exit=1

$ cargo test -p gateway --test red_vp_bin_width_size --quiet
# isolated critic worktree; temporary DEFAULT_VP_BIN_WIDTH_E8=1 (no-grid mutant)
V1 НАРУШЕН: кадр прод-формы весит 4718967 Б при подписанном пределе 2000000 Б (2.36×).
test result: FAILED. 0 passed; 1 failed; finished in 32.16s
exit=101

$ cargo test -p gateway --test red_vp_bin_width_size --quiet
# isolated critic worktree; temporary forbidden `10 * W` large-profile mutation
V1b НАРУШЕН: корзин 2043 против канонического разбиения шириной 25000000 e8 — 20417.
test result: FAILED. 0 passed; 1 failed; finished in 32.16s
exit=101

$ cargo test -p gateway --test red_vp_bin_width --quiet v7_live_reducer_equals_independent_replay_under_grid
test result: ok. 1 passed; 0 failed; 9 filtered out; finished in 0.02s
$ cargo test -p gateway --test red_vp_bin_width --quiet v8_snapshot_plus_frames_equals_full_replay
test result: ok. 1 passed; 0 failed; 9 filtered out; finished in 0.02s
# both in isolated no-grid mutant
v7_exit=0 v8_exit=0

$ bash scripts/next_artifact_id.sh C
C-230
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-19T14:42Z
- Milestone: M-86-vp-bin-width
- Статус: BLOCKED — REJECT
- HEAD: 69c6ecc — fix(M-86): гейт rev2 — именованный состав, хеш тела fingerprint, точный дефолт [architect]

## §B — Что я сделал
- Аудировал committed artifact set и merge-preview, а не только rev2-текст.
- Подтвердил закрытие C-229 R1–R6 и воспроизвёл мутанты no-grid / `10·W`.
- Нашёл B1 (FA task #6 отсутствует) и B2 (V1 не несёт прошлую эвиктнутую сессию).

## §C — Артефакты / результаты
- `research/critiques/C-230-m86-vp-bin-width-round2.md`
- Done Block: merge-preview exit=0; `verify_M-86.sh` exit=1 — RED ожидаем по dev-задачам, но G красен и B2 не покрыт до dev.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  На feat/M-86-vp-bin-width закрой C-230 B1 и B2 ТОЛЬКО в architect-зоне. До dev закоммить task #6: живой VP-I-4 в docs/fa/viz-backend.md, определяющий корзину VP как диапазон и запрещающий выдуманные пустые корзины. Затем усили V1: добавь прошлую UTC-сессию и setup-страж, доказывающий её whole-session eviction при bounded-window; мутант без eviction должен краснеть. Сохрани проверенное закрытие C-229 R1–R6, не меняй T1, Value Area, response limit, bands или implementation. Исправь NOTE 2042→2043, если та же фикстура остаётся каноном. Закоммить и push на эту ветку, затем передай full SHA critic для круга 3.
  ```
- Push-статус: ✅ verdict committed and pushed to origin/feat/M-86-vp-bin-width (see §A after commit).
- ✅ кэш убран: `target/` в рабочем и изолированном critic-worktree удалены; временный worktree снят.

## §E — Риски / открытые вопросы
- B1 нельзя переложить на engine-dev: `docs/fa/**` — зона architect.
- B2 касается ограниченности memory/frame формы, а не только воспроизводимости; отсутствие прошлой сессии делает доказательство bounded-window вакуумным.

=== END HANDOFF ===
