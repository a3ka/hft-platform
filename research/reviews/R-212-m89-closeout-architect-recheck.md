<!-- GATE-META
milestone: M-89
audited_repo: a3ka/hft-platform
audited_base: afb7fa94d70abfbb1f7a6e01794a50f4ef9779bd
audited_head: 38ad763e83f16f02479bb541fa01b27b7a58ee34
verdict: NOTE
-->

# R-212 — перепроверка `gates.md` §9 ветки `docs/m89-closeout-architect` (architect-клон, свежий контекст)

**Вердикт: NOTE.** Три коммита ветки делают то, что заявляют: задача 12 `M-89` закрыта с
исходом, дословно совпадающим с `R-211` по всем числам; спека и гейт `M-87` перенесены в
`docs/archive/` как чистые rename (100 %), оба токена на месте; `ROADMAP` обновлён в зоне
architect'а (порядок + новые строки), колонки «Состояние» существующих строк не тронуты. Все
шесть барьеров репозитория и `verify_design_claims.sh --merge-preview` зелены; ветка стоит
РОВНО на `origin/main` (отставание 0), поэтому дерево ветки и дерево слияния совпадают.

Блокера нет. Одна находка требует правки на ветке до merge'а (`Б-1`: тот же коммит переезда
оставил в ЖИВОЙ спеке `M-89` путь на уже вынесенный файл, и барьер `check_archived_refs.sh`
этот класс по построению не видит); три замечания — точность утверждений в телах коммитов и
полнота исполнения рекомендации `R-211` §7, без последствий для предмета.

Роль: architect-клон со свежим контекстом (`.claude/agents/architect.md` §Делегирование),
автор ветки — ведущий architect, он сторона. Зона записи — только этот файл.

## 0. Предмет, дрейф, форма

```
$ git fetch origin && git rev-parse origin/docs/m89-closeout-architect origin/main
38ad763e83f16f02479bb541fa01b27b7a58ee34
afb7fa94d70abfbb1f7a6e01794a50f4ef9779bd
$ git merge-base --is-ancestor 38ad763 origin/docs/m89-closeout-architect && echo YES
YES
$ git log --oneline origin/main..origin/docs/m89-closeout-architect
38ad763 docs(ROADMAP): M-90 первым (TD-227, взрыватель деплоя); TD-228, TD-229 строками; M-87 — путь архива [architect]
8d81b44 docs(M-87 close-out): спека и гейт M-87 → docs/archive/ (норма Р-2); ссылка red_m87_admission.rs на новый путь [architect]
1fc0305 docs(M-89): задача 12 ✅ — исход прод-замера R-211: I-1/I-2 исполнены, порог признан неполным (каталог — TD-229); журнал кругов PR-3 [architect]
$ git rev-list --count HEAD..origin/main
0                                  ← ветка НЕ отстаёт: merge-preview == дерево ветки
$ git diff -M --stat origin/main..HEAD
 crates/gateway-serve/tests/red_m87_admission.rs              | 2 +-
 docs/ROADMAP.md                                              | 5 ++++-
 {milestones => docs/archive}/M-87-serving-circuit-breaker.md | 0
 {scripts => docs/archive}/verify_M-87.sh                     | 0
 milestones/M-89-s0-volume-guard-observability.md             | 3 ++-
$ git diff -M --summary origin/main..HEAD
 rename {milestones => docs/archive}/M-87-serving-circuit-breaker.md (100%)
 rename {scripts => docs/archive}/verify_M-87.sh (100%)
```

Worktree — свой, detached: `/tmp/hft-recheck-m89co`. Повторный `git fetch` перед записью
вердикта — §6, дрейфа нет.

## 1. (а) Утверждения О КОДЕ — каждое командой

### 1.1. «Сторож `TD-225` `red_m89_serial_guard` исполняется `cargo test --all` в CI без флагов» — ВЕРНО

```
$ find crates -name 'red_m89_serial_guard*'
crates/gateway-serve/tests/red_m89_serial_guard.rs
$ grep -c 'required-features' crates/gateway-serve/Cargo.toml ; echo "grep exit=$?"
0
grep exit=1                        ← ни одной секции required-features
$ grep -c '\[\[test\]\]' crates/gateway-serve/Cargo.toml ; echo "grep exit=$?"
0
grep exit=1                        ← ни одного [[test]]-таргета: все tests/*.rs авто-обнаружены
$ head -20 crates/gateway-serve/tests/red_m89_serial_guard.rs | grep -n 'cfg\|ignore\|feature'; echo "grep exit=$?"
grep exit=1                        ← без cfg/ignore/feature-гейтов в шапке
$ grep -n 'cargo test' .github/workflows/ci.yml
24:        run: cargo test --all
```

Форма показа с `grep exit=1` намеренная: `grep -c` при нуле печатает «0», но `exit=1` отличает
«нет совпадений» от «не запускали» (`architect.md` §Утверждение о состоянии, `R-097` N-7).

### 1.2. «Ссылки `verify_M-87` в `red_m87_registry.rs` / `verify_M-89.sh` — имя, не путь» — ВЕРНО, перечень неполон (`Н-2`)

```
$ git grep -n 'verify_M-87' -- 'crates/**' 'scripts/**' 'deploy/**' '.github/**' 'docker-compose.yml'
crates/gateway-serve/tests/red_m87_r196_conditions.rs:7://! а приписка: «гейт зелен при всех пяти». Шаги задач 4, 5, 6, 8 в `verify_M-87.sh` были
crates/gateway-serve/tests/red_m87_registry.rs:643:/// Шаг `A-040` в `verify_M-87.sh` сводит перечень харнесса с перечнем реестра.
scripts/tests/red_verify_M-89_task_status.sh:3:# `verify_M-87.sh` с исправлениями `TD-221`.
scripts/verify_M-89.sh:54:# ─────────── круг `R-202` → `TD-221`: §Tasks НЕ ВРЁТ О СЕБЕ (перенос из verify_M-87.sh) ───
```

Все четыре — комментарии (`//!`, `///`, `#`), все четыре — голое имя `verify_M-87.sh`, без
`scripts/`. Ни одна не исполняет и не открывает файл. Тело `8d81b44` называет два из четырёх
(`red_m87_registry.rs`, `verify_M-89.sh`); ещё два того же класса — `red_m87_r196_conditions.rs:7`,
`red_verify_M-89_task_status.sh:3`. Утверждение не ложно, перечень неполон — `Н-2`.

### 1.3. Исход задачи 12 против `R-211` — ДОСЛОВНО по числам

Строка `milestones/M-89-s0-volume-guard-observability.md:539` (§13) сверена с
`research/reviews/R-211-M-89-task12-prod-measure-closeout.md` (на `origin/main`, прочитан
целиком):

| утверждение в строке 539 | где в `R-211` | совпадение |
|---|---|---|
| прод `5b1afa24`, 2026-10-01 | §0 «прод `5b1afa2`, 2026-10-01T13:41Z»; §11 «`5b1afa24`» | да |
| `lseek(…, 274307325)` — позиция курсора | §4.1 `lseek(11<…893.jrnl>, 274307325, SEEK_SET)` | да |
| префикс активного сегмента 276.6 МБ не читается | §2 «≈ 276.6 МБ», §4.1 «эти 276.6 МБ префикса… не читаются» | да |
| порог `rchar_delta ≤ слепок + отставание × 1 351 Б + 2 МиБ` | §3 первая строка | да |
| ПРОВАЛЕН ×3: 328.3 / 331.1 / 334.4 против 288.0 / 290.8 / 299.4 | §3 таблица, строки 1–3 | да, все шесть чисел |
| 54 % байт — каталог сегментов, 384 КиБ с каждого `.zst`, трижды | §4 «54 % ВСЕГО ОБЪЁМА», §4.2 «384 КиБ… ТРИЖДЫ за одну подписку» | да |
| `M-62`/`TD-120` — пред-существующий дизайн | §4.2 `segments.rs:2557-2601` | да |
| «порог признан НЕПОЛНЫМ» — решение architect'а | §7 п.1: «он же решает, чинить порог или признать его неполным» | решение в праве architect'а, вариант назван reviewer'ом |
| `TD-229` со своим оракулом | §8 `TD-229` «Зона — architect» | да |

Строка журнала кругов `:719` (`PR-3`): `R-210` APPROVED → PR #284 — `git log` main:
`5b1afa2 Merge pull request #284`; `R-211` NOTE — шапка `R-211` `verdict: NOTE`. Утверждение
«спека и гейт `M-89` ждут переноса до снятия двух ссылок чужой зоны» проверено:

```
$ git grep -n -E 'M-89([^a-z0-9]|$)' -- docker-compose.yml 'deploy/**' | grep verify_M-89
deploy/cron.d/watchdog:20:# gateway-serve пишет. Проверяется `verify_M-89.sh` task11 и
docker-compose.yml:233:      # проверяется `verify_M-89.sh` task11 и `red_m89_heartbeat_entrypoint::h0`).
```

Обе зоны — engine-dev (`scope-guard.md`); отложить вынос `M-89` — ровно требование 2 нормы.

### 1.4. Тело `8d81b44`: «M-87 закрыт reviewer'ом (`R-210`: код в main и в проде, §8 пройден 2026-09-26)» — факт верен, атрибуция неточна (`Н-1`)

```
$ grep -n -E '2026-09-26' research/reviews/R-210-M-89-s0-volume-guard-pr-gate-r3.md; echo "exit=$?"
exit=1                              ← даты в R-210 нет
$ sed -n '326,328p' research/reviews/R-210-M-89-s0-volume-guard-pr-gate-r3.md
**Маршрут:** merge через PR → §8-гейт (CI + Deploy + ssh + задача 12) → close-out
(`PROJECT-STATE.md`, `TECH-DEBT.md`) → architect (перенос спеки и гейта `M-89`, а также
`M-87`, в `docs/archive/`).
$ git grep -n '2026-09-26' -- PROJECT-STATE.md | grep 'M-87' | cut -c1-120
PROJECT-STATE.md:2860:## M-87 «предохранитель выдачи» (S0 программы SCALE, арбитраж `A-037`) — ✅ КОД В MAIN (merge PR #226 `843894f` 2026-09-26, `R-205` APPROVED, круг 5) + **В …
```

Закрытость `M-87` reviewer НАЗВАЛ: `R-210:326-328` прямо отправляет `M-87` в архив,
`R-211` §7 говорит о «переносе `M-87`/`M-89` в архив» как о следующем шаге. Требование нормы
Р-2 (строка 1 таблицы ролей: «назвать, какие милестоуны ЗАКРЫТЫ — reviewer») выполнено. Но
дата «2026-09-26» и «§8 пройден» живут в `PROJECT-STATE.md:2860` (зона reviewer'а, прочитано
грепом по `M-87`), а не в `R-210`. Тело коммита — история, не правится; фиксирую как
неточность цитаты, не как дефект предмета.

## 2. (б) Три требования переезда — `04-workflow.md` §2 «Close-out»

### 2.1. Универсум номеров не сужается — ВЫПОЛНЕНО механизмом

```
$ grep -n 'docs/archive' scripts/check_artifact_ids.sh scripts/next_artifact_id.sh
scripts/next_artifact_id.sh:91:        M) _sed_re='^(milestones|docs/archive)/M-[0]*([0-9]+)([a-z])?(-.*)?\.md$'  _sed_cap='\2' ;;
scripts/check_artifact_ids.sh:66:    docs/archive/M-*.md) ;;
```

`M-87` после переезда продолжает занимать номер и в барьере, и в аллокаторе.

### 2.2. Ссылок из ЧУЖИХ зон на гейт/спеку `M-87` нет — греп с `**` по ОБОИМ именам

```
$ git grep -n 'M-87-serving-circuit-breaker' -- 'crates/**' 'scripts/**' 'deploy/**' '.github/**' 'docker-compose.yml' 'milestones/**' 'docs/ROADMAP.md'
crates/gateway-serve/tests/red_m87_admission.rs:5://! `docs/archive/M-87-serving-circuit-breaker.md` §4, §5, §7 (вынесена на close-out, норма Р-2).
docs/ROADMAP.md:108:| **SCALE** | … (`docs/archive/M-87-serving-circuit-breaker.md`) …
milestones/M-89-s0-volume-guard-observability.md:708:- `milestones/M-87-serving-circuit-breaker.md` §4.0bis, §11 п. 5, §14.1ter, §16; задачи 15/16/24
```

`crates/*/src/**`, `deploy/**`, `docker-compose.yml` — ноль попаданий по обоим именам (вывод
`verify_M-87` — §1.2: только `*/tests/**` и `scripts/verify_M-89.sh`, все sacred-зона
architect'а, все комментарии). Единственная ссылка в `crates/**` — `red_m87_admission.rs:5` —
sacred-зона architect'а, обновлена тем же коммитом на новый путь. Требование 2 — выполнено.

**Но `milestones/M-89-…md:708` — ПУТЬ в пустоту, оставленный тем же переездом** — `Б-1`, §4.

### 2.3. Живой барьер на `verify_M-87` не опирается — ВЫПОЛНЕНО

`git grep 'verify_M-87' -- 'scripts/**' '.github/**'` (§1.2) даёт две строки, обе — `#`-комментарии
(`verify_M-89.sh:54`, `red_verify_M-89_task_status.sh:3`). Ни `scripts/check_*.sh`, ни
`scripts/lib/**`, ни workflows его не зовут.

### 2.4. Барьеры — на дереве ветки (== merge-preview, отставание 0)

```
$ BASE=$(git merge-base origin/main HEAD)   # afb7fa94d70abfbb1f7a6e01794a50f4ef9779bd
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_protected_artifacts.sh
NOTE  milestones/M-87-serving-circuit-breaker.md: ALLOW-ARTIFACT-DELETE в 8d81b44
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 0, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_roadmap_sync.sh
VERDICT: PASS
exit=0
$ bash scripts/check_archived_refs.sh
PASS  вынесенных артефактов найдено: 64
PASS  НОВЫХ висячих ссылок нет (проверено 64 артефактов)
PASS  базовая линия актуальна (унаследованных ссылок: 21)
VERDICT: PASS
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_docs_freeze.sh
exit=0                              ← молчит и выходит 0: зона §11 не тронута (`:104-107`)
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_artifact_ids.sh | tail -1
OK: ни один коммит диапазона afb7fa9..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -2
VERDICT: PASS (0 нарушений)
exit=0
$ cargo fmt --all -- --check; echo exit=$?
exit=0
```

**Предел `check_gate_meta` на этом диапазоне назван:** «вердиктов проверено: 0» — subject-lock
не взводился, потому что в `afb7fa9..38ad763` вердиктов нет. Токен `ALLOW-SUBJECT-CHANGE` в
`8d81b44` барьер на этом дереве НЕ ЧИТАЛ (ровно латентность, описанная в
`04-workflow.md` §Close-out «Поправка 2026-08-24»). После коммита ЭТОГО вердикта на ветку
диапазон содержит вердикт — повторный прогон в §6.

**Предел `check_archived_refs` назван:** `LIVE` (`:54-55`) — `crates/** scripts/** deploy/**
.github/** .claude/** docs/fa/** docs/0*.md docs/DESIGN.md docs/ROADMAP.md CLAUDE.md`;
`milestones/**` в нём НЕТ. PASS барьера не свидетельствует о `:708` (`Б-1`).

## 3. (в) Полномочия

- **Зона architect'а** (`scope-guard.md`): `milestones/M-NN-*.md` — да; `scripts/verify_*.sh` —
  sacred architect; `*/tests/` — sacred architect (`red_m87_admission.rs` — doc-комментарий `//!`,
  кода не меняет; `cargo fmt --check` exit=0); `docs/ROADMAP.md` — `docs/**`. Пять файлов, ни
  одного вне зоны.
- **ROADMAP, колонка «Состояние» существующих строк** — не тронута. Word-diff показывает ровно
  одну замену в существующей строке `SCALE`:
  ```
  $ git diff --word-diff=plain origin/main..HEAD -- docs/ROADMAP.md | grep -o '\[-[^]]*-\]'
  [-(`milestones/M-87-serving-circuit-breaker.md`):-]
  ```
  — путь, не состояние. Три строки добавлены (`M-90`, `TD-228`, `TD-229`), у всех 6 колонок, как
  у заголовка `:92` (`| № | Предмет | Что сделать | Где | Владелец | Состояние |`); состояние
  НОВОЙ строки автор задать вправе (перечень и порядок — architect). Состояние `M-90` «⏳ критик,
  круг 2 (`C-265` REJECT закрыт правками)» сверено с веткой:
  ```
  $ git log --oneline origin/main..origin/feat/M-90-warmer-selector-single-source | head -5
  6936f0b docs(M-90): спека круг 2 — I-1bis, w4a-w4e, точная карта CI, журнал кругов (C-265 REJECT) [architect]
  331ce43 test(M-90): red_verify_M-90_ci_map.sh — … (C-265 F2) [architect]
  a40c69d fix(M-90): verify — ТОЧНАЯ карта CI-паритета … (C-265 F2) [architect]
  3706465 test(M-90): w4a-w4e — … (C-265 F1) [architect]
  8cd1636 docs(M-90): C-265 — REJECT warmer selector gate [critic]
  ```
- **Граница C** — не задета: `research/registry/`, `research/decisions/`, веса/лимиты/промоушены
  в дифе отсутствуют (`git diff --stat` §0 — пять файлов, ни одного из них).
- **Замок §11** — не задет: `.claude/**`, `CLAUDE.md`, `docs/04-workflow.md` в дифе отсутствуют;
  `check_docs_freeze.sh` exit=0 молча (зона не тронута).
- **Push-scope** — три коммита, все `[architect]`, все по предмету.

## 4. (г) Связность

### Б-1 (NOTE, правится на ветке): `milestones/M-89-s0-volume-guard-observability.md:708` — путь на вынесенный файл

```
$ sed -n '708p' milestones/M-89-s0-volume-guard-observability.md
- `milestones/M-87-serving-circuit-breaker.md` §4.0bis, §11 п. 5, §14.1ter, §16; задачи 15/16/24
$ ls milestones/M-87-serving-circuit-breaker.md; echo "exit=$?"
ls: cannot access 'milestones/M-87-serving-circuit-breaker.md': No such file or directory
exit=2
```

§19 «Cross-references» — навигационная ссылка, по которой читатель ЖИВОЙ спеки (`M-89` ещё в
`milestones/`, её вынос отложен — `:719`) идёт за содержанием. Коммит `8d81b44` обновил такую
же ссылку в `red_m87_admission.rs:5`, а эту — в собственной зоне, в документе, который тот же
автор правил коммитом раньше (`1fc0305`) — нет. Барьер этот класс не видит (`LIVE` без
`milestones/**`, §2.4), поэтому зелёный `check_archived_refs` здесь не аргумент. Это тот самый
класс, ради которого барьер заведён (`check_archived_refs.sh:9-15`: переезд `M-88` оставил
восемь висячих ссылок при теле коммита «ссылок нет»).

Того же класса, ниже весом: `:68` — `scripts/verify_M-87.sh:447,454` в таблице «что было» (замер
на момент `TD-221`, путь со строками). Это ИСТОРИЧЕСКОЕ утверждение о коде на тогдашней ревизии,
не навигация; править не требую, назвать обязан.

**Что сделать:** одна строка — `docs/archive/M-87-serving-circuit-breaker.md` в `:708`, своим
коммитом на эту же ветку (зона architect'а; `milestones/*.md` защищён только от удаления).

### Н-3 (замечание): рекомендация `R-211` §7 исполнена в §13, но не в §15

`R-211` §7: «перед переносом переключить Status задачи 12 и вписать в §13/§15 исход замера».
`:539` (§13) — исход вписан полностью; §15 (`:622-650`) не менялся и по-прежнему описывает
ФОРМУ шага («SKIP-шаг прод-замера печатается явно и не зеленеет сам»). Противоречия нет —
§15 описывает гейт, не результат, — и спека `M-89` в архив этим диапазоном не уходит, так что
«архив унесёт спеку без следа» не наступает. Замечание, не находка.

### M-90 первым — ОБОСНОВАНО

`R-211` §10–§11: взрыватель `TD-227` уже срабатывал (`0f9a980`, Deploy-джоб `success`
23.09 12:45:42–12:48:14), цена — восемь суток недоступности read-path при зелёных liveness;
«заряжает взрыватель именно КОДОВЫЙ деплой по `push`… `M-90` от этого обязательнее». Строка
`ROADMAP:109` воспроизводит ровно это (восемь суток 23.09→10.01, `0f9a980`, «стирается первым
кодовым деплоем»). Порядок — проектное суждение architect'а (таблица ролей Р-2, строка 4), и
основание у него не из головы, а из вердикта reviewer'а.

`TD-228`/`TD-229` в `ROADMAP` соответствуют `R-211` §5.2/§4.2/§8 (54 %, 384 КиБ, ×3,
`red_segment_meta_bound` снизу; legacy-путь без счётчиков, `OPS-I-8` слеп). Карточки на `main`:
`grep -c 'TD-228\*\*' TECH-DEBT.md` → 2, `TD-229` → 2 (носитель и упоминание; `check_artifact_ids`
на диапазоне ветки — OK). Утверждение `TD-228` «класс отказа решён: `refusals_supported`, как в
v1-пути» — проектное решение architect'а; v1-путь действительно считает отказы этим счётчиком
(`crates/gateway-serve/src/lib.rs:1121,1161,1196,1261` — `inc_refusals_supported`).

Висячих ссылок в ДОБАВЛЕННОМ тексте не нашёл: `R-210`, `R-211`, `C-265`, `TD-220/226/227/228/229`,
`M-62`/`TD-120`, `docs/archive/M-87-…` — все существуют на дереве.

## 5. FA (M-66)

Диф трогает `crates/gateway-serve/tests/**` (комментарий). Живые инварианты на `38ad763`, проверены
командой: `VB-I-2` (`docs/fa/viz-backend.md:205`), `VB-I-10` (`:213`), `VB-I-11` (`:214`) — те, что
`M-89` §19 называет своими; `OPS-I-8` (`docs/fa/ops.md:476`) — тот, на который опирается `TD-228`.

```
$ grep -n 'VB-I-2\b\|VB-I-10\b\|VB-I-11\b' docs/fa/viz-backend.md | head -3; grep -n 'OPS-I-8\b' docs/fa/ops.md | head -1
```
(вывод — в §6 Done Block, снят на дереве ветки.)

## 6. Done Block

Снят на `/tmp/hft-recheck-m89co` перед коммитом вердикта; вывод — сырой, усечён по ширине.

```
$ date -u +%Y-%m-%dT%H:%MZ
2026-10-01T17:48Z
$ git fetch origin && git rev-parse origin/docs/m89-closeout-architect HEAD
38ad763e83f16f02479bb541fa01b27b7a58ee34
38ad763e83f16f02479bb541fa01b27b7a58ee34
$ git status --porcelain
?? research/reviews/R-212-m89-closeout-architect-recheck.md
$ grep -n "VB-I-2\b\|VB-I-10\b\|VB-I-11\b" docs/fa/viz-backend.md | sed -n 2,4p
205:| VB-I-2 | **live == replay**: серия, посчитанная на live-хвосте, бит-идентична серии из replay 
213:| VB-I-10 | **Bounded-window snapshot (M-37, TD-039).** Память `snapshot`/`frames_since` огранич
214:| VB-I-11 | **Провенанс ИСТОРИИ (M-48, TD-048).** «All-time» ≡ «от самого раннего seq, доступног
$ grep -n "OPS-I-8\b" docs/fa/ops.md | head -1
476:| OPS-I-8 | Тишина в потоке (`md_event_age_ms > порог`) — алерт P1: «жив, но не работает» (TD-01
$ EVENT_NAME=pull_request PR_BASE_SHA=afb7fa9… bash scripts/check_gate_meta.sh | tail -1   (до коммита вердикта)
VERDICT: PASS — вердиктов проверено: 0, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
```

После коммита вердикта диапазон содержит вердикт — повторный прогон `check_gate_meta` /
`check_artifact_ids` / `check_protected_artifacts` снят ПЕРЕД push'ем, вывод — в отчёте клона
(`gates.md` §8: предмет гейта обязан быть в `origin` — вердикт пушится на ту же ветку).

## 7. Итог

| № | класс | где | что |
|---|---|---|---|
| Б-1 | NOTE — правка на ветке | `milestones/M-89-s0-volume-guard-observability.md:708` | путь `milestones/M-87-…md` на вынесенный файл; барьер слеп к `milestones/**` |
| Н-1 | замечание | тело `8d81b44` | «§8 пройден 2026-09-26» приписано `R-210`; источник — `PROJECT-STATE.md:2860` |
| Н-2 | замечание | тело `8d81b44` | перечень имя-ссылок неполон (ещё `red_m87_r196_conditions.rs:7`, `red_verify_M-89_task_status.sh:3`) |
| Н-3 | замечание | `M-89` §15 | рекомендация `R-211` §7 исполнена в §13, §15 без исхода — противоречия нет |

**NOTE.** Merge после одной строки по `Б-1` (своим коммитом автора на ту же ветку);
`Н-1`…`Н-3` правок не требуют. Перепроверка §9 засчитана; критик по триггерам `gates.md` §9 не
требуется: инварианты, границы A/B/C, фазы не менялись, новой milestone-спеки нет — правка
статуса, журнала кругов, порядка роадмапа и переезд по норме Р-2.

Предел этого вердикта: `scripts/verify_M-89.sh` целиком (с `cargo test`) на ветке не гонял —
диапазон не меняет ни кода, ни гейтов, ни оракулов (единственная правка в `crates/**` — doc-комментарий,
`cargo fmt --check` exit=0); приёмка `M-89` снята `R-210` на `58c766a`, `R-211` на `009d82a`.
