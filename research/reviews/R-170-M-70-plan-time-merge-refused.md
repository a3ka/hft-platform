<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: aacab22205d436e9e5d9399bc7098cd499a73e7c
audited_head: 7a98fe878f3d5c9d4bf73f2b6a31cd50c64429de
verdict: REJECT
-->

# R-170 — M-70: merge ОТКАЗАН, маршрут critic→reviewer на этой стадии неверен

## Вердикт: REJECT — отказан МЕРЖ и МАРШРУТ, а не набор артефактов

Меня позвали исполнить протокол ревьюера над `origin/docs/M-70-rev2`
(`aacab22..7a98fe8`, 17 коммитов) после проходного `C-209`. Протокол исполнен
целиком; его результат — **не мержу**, и следующий агент цепочки не я.

Отказ адресован ровно двум вещам: (1) вливанию этой ветки в `main` и (2) звену
`critic → reviewer` в `§D` хендоффа `C-209`. Набор артефактов `M-70` (спека, пять
`DB-I-*` оракулов, `verify_M-70.sh`) я НЕ отвергаю: он plan-time, круг критика его
прошёл, и переделывать в нём по моему вердикту нечего. Дефект — в стадии, на которую
его принесли.

---

## Block-scope — PASS

Диапазон трогает ровно зоны, объявленные `M-70` §2, и ничего сверх:

| путь | владелец по §2 / scope-guard |
|---|---|
| `crates/gateway/tests/**` (4 файла) | architect (sacred RED) |
| `crates/gateway-serve/tests/**` (1 файл) | architect (sacred RED) |
| `milestones/M-70-depth-bands-enablement.md` | architect |
| `scripts/verify_M-70.sh` | architect |
| `research/critiques/C-193,C-208,C-209` | critic |

Ни одного `crates/*/src/**`, ни `deploy/**`, ни `docker-compose.yml`, ни
`TECH-DEBT.md`/`PROJECT-STATE.md`. Превышения зоны нет.

## Block-C (contracts) — PASS (неприменим)

`crates/contracts/**` в диапазоне отсутствует; шаг гейта проверяет это fail-closed
и зелен. Contract-RFC не требуется.

## Block-risk — неприменим, и основание названо

Диапазон не трогает `crates/risk|killswitch|oms|venue-*`. `gateway`/`gateway-serve` —
read-only виз-бэкенд (`VB-I-3`: gateway не импортирует journal-writer/recorder-write;
`VB-I-9`: транспорт не ходит в application-БД). Order-egress отсутствует ⇒ `gates.md`
§5 не взводится, `risk-critic` в цепочке не требуется.

## Block-DoneBlock — PASS

`C-208` и `C-209` несут сырой stdout, не пересказ. Я перепроверил их независимо
своим прогоном (ниже) — числа сходятся.

## Предъявление FA (M-66)

Барьер `check_review_fa.sh` на этом диапазоне даёт **SKIP**: тронуты только
`crates/*/tests/**`, в прод-образ не входящие. То есть требование здесь **когнитивное,
а не машинное**, и я это говорю прямо, а не выдаю SKIP за проверку. Живые инварианты
тронутого модуля, вычитанные мной на проверяемой ревизии в `docs/fa/viz-backend.md`:

- **`VB-I-10`** (`:207`) — bounded-window snapshot: память `snapshot`/`frames_since`
  ограничена ОКНОМ `[at−W, at]`, окно привязано к курсору, не к wall-clock. Это ровно
  предмет задачи 8 `M-70` («предел выдачи не куплен ценой предела памяти»).
- **`VB-I-5`** (`:203`) — серия глубже 1.3 % несёт `depth_band_provenance`; отсутствие
  поля делает серию невалидной. Предмет задач 4 и 5.

---

## Н-1 (БЛОКЕР) — ветка не вливается в `main` ни по правилу, ни механически

**Правило.** `gates.md` §8: «**RED до реализации не живёт в `main`** (main всегда
зелёный). Два санкционированных пути: держать RED-коммиты локально до GREEN, либо
feat-ветка, которую reviewer мержит **уже зелёной**». Здесь второй путь и он ещё не
пройден: реализации нет, оракулы красны ПО ЗАМЫСЛУ.

**Механика.** `.github/workflows/ci.yml` job `build-test` гоняет `cargo test --all` на
`pull_request`; job входит в агрегат `All checks passed`, который защита `main`
требует обязательным чеком. Красный оракул ⇒ красный чек ⇒ `gh pr merge` физически
не проходит. То есть даже если бы я захотел влить — не смог бы, и это хорошо: правило
и барьер здесь совпадают.

Воспроизведение (моё, не цитата вердикта):

```
$ cargo test -p gateway --test red_depth_bands_cap
test db_i_3_selector_with_too_many_bands_is_rejected_before_any_work ... FAILED
test db_i_3d_boundary_is_inclusive_and_exact ... FAILED
test db_i_3b_snapshot_path_rejects_by_band_cap_not_by_response_size ... FAILED
test db_i_3c_signed_canonical_set_is_accepted ... ok
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
```

**Правильный следующий агент — `engine-dev`, диспетчеризуемый founder'ом**, по порядку
`§Tasks`: 3 → 4 → (4b architect) → 5 → 6 → (6b architect) → 7 → 8. Reviewer возвращается
в цепочку PR-time, когда `verify_M-70.sh` даст `VERDICT: PASS`, exit=0. `§D` вердикта
`C-209` («Next agent: reviewer») пропускает всю реализацию — это не мелкая неточность
адресации: исполненный буквально, он ставит гейт приёмки перед работой, которую он
принимает.

## Н-2 (БЛОКЕР ДИСПЕТЧА) — тело спеки говорит, что dev НЕ диспетчеризуется

`milestones/M-70-depth-bands-enablement.md:504`:

> **Набор к dispatch НЕ готов и dev не диспетчеризуется** — стандарт `A-028` §1 остаётся
> в силе. Готовность наступает после вливания `M-75` и написания `DB-I-0`/`DB-I-3`.

Шапка rev7 (`:1-3`) говорит обратное: «Набор к кругу критика **ГОТОВ**». Оба условия
строки 504 фактически исполнены — `M-75` в `main` (шаг гейта зелен), `DB-I-0` и `DB-I-3`
написаны и исполняются гейтом, — но текст не переписан.

Это операционно значимая половина того, что `C-209` записал как «stale prose», и
именно её вердикт не назвал. Founder диспетчеризует по спеке; engine-dev, прочитавший
строку 504, обязан остановиться. Правка — **зона architect'а** (`milestones/*.md`
architect-only, `scope-guard.md`); reviewer её не делает.

## Н-3 — три разных числа гейта в одном документе, и ни одно не верное

| место | сказано | факт |
|---|---|---|
| `:475` (§Tasks, задача 9) | `VERDICT: FAIL (6)` | `FAIL (10)` |
| `:495` (§3bis, строка гейта) | `VERDICT: FAIL (6)` | `FAIL (10)` |
| `:546` (§3ter) | `VERDICT: FAIL (8)` | `FAIL (10)` |
| `:497` (§3bis) | `DB-I-0`, `DB-I-3` «**НЕ написаны намеренно**» | оба файла есть, гейт их ИСПОЛНЯЕТ |

Мой прогон: `VERDICT: FAIL (10)`, exit=1 (сырой вывод — Done Block ниже). Зона
architect'а. Не блокирует merge только потому, что merge отказан по Н-1.

## Н-4 — идентификатор `P-022` выдан мимо аллокатора и КОЛЛИДИРУЕТ

`C-209` заводит долг под именем `P-022`. Две проблемы, обе проверяемы командой:

1. **Класса `P` не существует.** `scripts/next_artifact_id.sh:14` — `case "${CLS}" in
   TD|R|C|A|M)`; всё прочее → `die "неизвестный класс"`. `gates.md` §12: «Номер берётся
   ТОЛЬКО механизмом». Номер `022` здесь не выдан никем.
2. **Гомоглиф уже занят.** `docs/PENDING-SIGNATURE.md:1511` — `## П-022 — РЕШЕНИЕ
   FOUNDER'А 2026-08-26`. Латинская `P` и кириллическая `П` в наших документах стоят
   рядом; агент, грепнувший `P-022`, попадёт в founder-решение, а грепнувший `П-022` —
   не найдёт долг `C-209`. `gates.md` §12 объявляет идентификатор УНИКАЛЬНЫМ.

Долг из `C-209` реален (он же Н-3 выше) — но носить его должен ТЕКСТ, а не выдуманный
идентификатор. Просьба к critic'у: не тиражировать `P-022` в следующих вердиктах.

## Н-5 — моё собственное нарушение `branch-hygiene` п.2, устранённое и предъявленное

Собирая дерево, я сделал `git checkout -B docs/M-70-rev2` в своём worktree. Ветка была
занята `/tmp/hft-arch-m70-probe` (HEAD `252ad3a`); git не отказал, и ref уехал вперёд на
`7a98fe8`. Чужое дерево немедленно показало **13 файлов со статусом `D`** — ровно тот
сценарий, о котором предупреждает `branch-hygiene` п.10 («один `git commit -a`, и чужая
работа снесена»).

Устранено в том же ответе: `git checkout --detach`, затем
`git update-ref refs/heads/docs/M-70-rev2 252ad3a`. Проверено —
`probe HEAD=252ad3a`, `git status --porcelain | wc -l` → `0`. Потерь нет: `252ad3a`
предок `7a98fe8`, движение было fast-forward. Свой вердикт я коммичу в detached-дереве
и пушу `HEAD:docs/M-70-rev2`, локальную ветку больше не трогая.

Записываю это находкой, а не умалчиваю: `git branch -f` барьер имеет, `git checkout -B`
на занятую ветку — по замеру этого дерева — НЕТ. Пункт правила когнитивен там, где
казался механическим.

---

## Что НЕ является находкой — проверено и чисто

Все процессные барьеры прогнаны мной ТОЙ ЖЕ проводкой, какой их зовёт CI
(`EVENT_NAME=pull_request PR_BASE_SHA=aacab222`):

- `check_gate_meta.sh` → `VERDICT: PASS — вердиктов проверено: 3`, exit=0
- `check_artifact_ids.sh` → OK, exit=0
- `check_docs_freeze.sh` → exit=0 (процессная зона `§11` не тронута — токен не требуется)
- `check_protected_artifacts.sh` → OK, exit=0
- `check_review_fa.sh` → SKIP с перечислением пяти файлов, exit=0
- `cargo fmt --all -- --check` → PASS; `cargo clippy --all-targets --all-features -D warnings` → PASS

Атомарность коммитов: 17 коммитов на диапазоне, каждый с ссылкой `M-70` и ролевой
меткой; бандла на несколько задач нет. `c82057c` закрывает три блокера `C-208` одним
коммитом — это ответ на вердикт, а не бандл задач `§Tasks`; замечанием не считаю.

## Заведённый долг (моя зона)

`M-70` §0.3 говорит дословно: «Долг заводит reviewer (`TECH-DEBT.md` — его зона)».
Исполнено отдельным PR: карточка о клиентском числе полос, не ограниченном ничем ДО
построения ответа (класс `PL-I-5`). Проверено мной на `origin/main`, а не принято на
слово:

```
$ git show origin/main:crates/gateway/src/lib.rs | grep -n -A32 'pub fn validate_selector' | grep -ci 'bands'
0
$ git show origin/main:crates/gateway/src/lib.rs | grep -cE '^pub const MAX_BANDS'
0
```

`gateway-serve/src/session.rs:70-91` проверяет диапазон `(0,1)`, сортировку и дубли —
количество не проверяет никто. Предел `M-71` срабатывает ПОСЛЕ построения ответа:
замер architect'а (`M-70` §2bis.3) — 4096 полос = 14 077 293 Б собрано и 18.13 с работы
сервера ради отказа. Закрывается задачей 3 этого милестоуна.

## Done Block

```text
$ git rev-parse HEAD; git merge-base origin/main HEAD
7a98fe8 (docs(M-70): C-209 — C-208 closure NOTE [critic])
aacab22205d436e9e5d9399bc7098cd499a73e7c
exit=0

$ git diff --stat origin/main...origin/docs/M-70-rev2 | tail -3
 research/critiques/C-209-M-70-c208-closure.md      | 109 +++++
 scripts/verify_M-70.sh                             | 343 +++++++++++++++
 10 files changed, 2653 insertions(+), 17 deletions(-)
exit=0

$ cargo test -p gateway --test red_depth_bands_cap 2>&1 | grep '^test result'
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ bash scripts/verify_M-70.sh 2>&1 | grep -E '^(FAIL|VERDICT)'
FAIL: cargo test --all --quiet
FAIL: оракул DB-I-3 (предел полос + анти-плацебо на подписанный состав) (исполнено тестов: 4, exit=101)
FAIL: grep -qE '^pub const MAX_BANDS' crates/gateway/src/lib.rs
FAIL: task #3 — MAX_BANDS в crates/gateway/src/lib.rs нет: гвард не введён (оракул ждёт 32)
FAIL: оракул DB-I-4 (точка/ряд + анти-плацебо) (исполнено тестов: 2, exit=101)
FAIL: task #4b — формы 'pub struct DepthPoint' в crates/gateway/src/lib.rs нет
FAIL: оракул DB-I-5 (один словарь на всю выдачу) (исполнено тестов: 4, exit=101)
FAIL: ! grep -q 'let prov_str = "diff-reconstructed".to_string()' crates/gateway/src/lib.rs
FAIL: task #6 версия схемы НЕ поднята (база 9, HEAD 9)
FAIL: task #7 — в записи GATEWAY_BANDS нет полос: 0.015 0.03 0.05 0.08 0.15 0.3 0.6
VERDICT: FAIL (10)
exit=1

$ EVENT_NAME=pull_request PR_BASE_SHA=aacab222… bash scripts/check_gate_meta.sh | tail -1
VERDICT: PASS — вердиктов проверено: 3, до-нормативных приземлений: 0
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=aacab222… bash scripts/check_artifact_ids.sh | tail -1
OK: ни один коммит диапазона aacab22..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=aacab222… bash scripts/check_protected_artifacts.sh | tail -1
OK: защищённые артефакты целы на HEAD (aacab22..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=aacab222… bash scripts/check_review_fa.sh | head -1
SKIP (диапазон трогает ТОЛЬКО не-прод пути крейтов — tests/examples/benches)
exit=0

$ bash scripts/next_artifact_id.sh R
R-170
exit=0

$ cd /tmp/hft-arch-m70-probe && git rev-parse --short HEAD; git status --porcelain | wc -l
252ad3a
0
exit=0
```
