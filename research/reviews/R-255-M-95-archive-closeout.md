<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: 63080cc40c2f33b80cb4274d68a358f14adb231f
audited_head: c2e41db7c0915a1912857b38dedb8d471c2dbcfc
verdict: APPROVE
-->

# R-255 — PR-гейт close-out `M-95` (переезд в архив по норме Р-2)

**Вердикт: APPROVE.** Предмет — ветка `docs/m95-archive`. Вершина взята командой
`git fetch origin && git rev-parse origin/docs/m95-archive` → `c2e41db7c0915a1912857b38dedb8d471c2dbcfc`,
совпала со справкой мандата `c2e41db7`. База — `origin/main` `63080cc4`; ветка — ровно один
коммит architect'а поверх неё (`git log origin/docs/m95-archive..origin/main` пуст).

**Startup-протокол:** `CLAUDE.md`; `.claude/rules/{gates,scope-guard,commit-discipline,branch-hygiene,handoff-block,testing}.md`;
`.claude/agents/reviewer.md`; `docs/04-workflow.md` §2 «Предмет гейта — ВЕТКА» и
«Close-out … (Р-2)», включая требования 1–3 к коммиту переезда. Ярус C — грепом:
`PROJECT-STATE.md` по `M-95` / `MS-STATE: M-95`; `TECH-DEBT.md` по `TD-25[0-9]`,
`red_m95_provenance_fresh`, `red_m87_entrypoint`, `red_m91_legacy_counters`, `features testing`.

**FA.** Диапазон трогает `crates/gateway-serve/tests/**` (только комментарий). Барьер `review-fa`
даёт SKIP (tests-only), требование когнитивное. Живой инвариант по предмету — `VB-I-11`
(`docs/fa/viz-backend.md:294`, провенанс истории). Его сторожат оракулы `f0…f6`
`red_m95_provenance_fresh.rs`; о них Н-2.

## Блоки

| блок | результат |
|---|---|
| Block-scope | 5 путей: 3 переезда (`milestones/M-95-catalog-once.md`, `scripts/verify_M-95.sh`, `scripts/tests/red_verify_M-95_ci_map.sh` → `docs/archive/`), комментарий `crates/gateway-serve/tests/red_m95_catalog_once.rs:2`, ячейка `docs/ROADMAP.md:115`. Всё в зоне architect'а (`04-workflow.md` Р-2: `git mv` гейта и спеки, перечень роадмапа; `*/tests/**` sacred architect). Импл-кода, `contracts/**`, `PROJECT-STATE`/`TECH-DEBT` нет |
| Block-C | `crates/contracts/**` не тронут |
| Block-risk | `risk`/`killswitch`/`oms`/`venue-*` не тронуты, risk-critic не требуется |
| Р-2: переезд без потери | оба скрипта — rename 100 % (0/0); спека — similarity 91 %, `+7/−6`: строка статуса, пять строк §11, строка журнала круга 10. Нормативные секции §1–§10, §12–§13 не менялись (диф ниже) |
| Р-2: маркер ДО переезда | `PROJECT-STATE.md:3057` `<!-- MS-STATE: M-95 CLOSED -->` уже в `main` (PR #334); `check_ps_on_archive` PASS |
| Р-2: два токена | `ALLOW-SUBJECT-CHANGE:` и `ALLOW-ARTIFACT-DELETE:` в теле `c2e41db7` |
| §11 ↔ коммиты | все семь SHA — предки `origin/main`, subject'ы соответствуют задачам: `a3d77986` task #1 (задача 2 — тем же коммитом, `k2`, как записано), `2cbb3882` task #3, `dcfa1465` Б-1 / `5b3dbf84` Н-4 / `bb44d060` Н-2 / `c33a814b` Н-3 (задача 5), `b91af0d8` task #6; задача 4 — `R-253` (файл в `research/reviews/`, вердикт по `M-95`). Расхождений нет |
| атомарность | один коммит = одна задача (close-out), метка `[architect]`, без co-author трейлера |

## Находки

**Н-1 (MINOR, зона architect'а) — устаревшая ссылка на вынесенный гейт в собственной зоне.**
`crates/gateway-serve/tests/red_m95_provenance_fresh.rs:32`: «этот файл … исполняется только
гейтом приёмки (`verify_M-95.sh` — с флагом)». Гейт теперь лежит в `docs/archive/`, и запустить его
оттуда нельзя: `docs/archive/verify_M-95.sh:42` вызывает `scripts/tests/red_verify_M-95_ci_map.sh`,
которого больше нет. Требование 2 нормы Р-2 обязывает снимать ссылки в своей зоне тем же
коммитом. Сосед `red_m95_catalog_once.rs:2` поправлен, а эта строка — нет: она называет гейт
голым именем, без `scripts/`. `check_archived_refs.sh` ищет прежние ПУТИ, голое имя он не видит
(PASS ниже). Тот же класс, что форма ошибки 1 в шапке барьера. Не блокирует: это комментарий,
поведение не меняется. Ссылки в `scripts/verify_M-96.sh:3` и `milestones/M-96-*.md:133`
(«копия `verify_M-95.sh`») — родословная, их не трогали сознательно; прецедент — `verify_M-95.sh:3`
→ `verify_M-94.sh` при `R-248`. Исправить при ближайшем касании файла; правку текста строки
связать с Н-2.

**Н-2 (MAJOR, системная, НЕ создана этим PR) → `TD-253`.** Оракулы под `#![cfg(feature = "testing")]`
закрытых милестоунов не исполняет ни один живой механизм. `ci.yml`/`branch-build.yml` запускают
`cargo test --all` без фич; `--all-features` встречается только у `clippy` (компиляция, не прогон).
Из живых гейтов `--features testing` зовёт только `verify_M-65.sh:227`, и то для одного
`red_ws_session`. Под замок попадают: `red_m95_provenance_fresh` `f0…f6` (`VB-I-11`, ветка
fail-closed `is_fresh/refresh → Err`, ради которой были `R-249` Б-1 и `R-250`),
`red_m91_legacy_counters` `l4`, `red_m87_entrypoint`. Всё это сторожилось только своими
`verify_M-NN.sh`, а эти гейты CI не вызывает. Значит, после merge'а они не исполнялись уже до
переезда, и переезд статус не меняет. Поэтому это не блокер close-out'а. Последствие: регрессия
fail-closed-ветки провенанса в `crates/gateway-serve/src/lib.rs` пройдёт CI зелёной. Класс —
`TD-138`: документ ссылается на механизм, который на этом пути не работает. Форма закрытия — за
architect'ом (`gates.md` §4).

## Done Block

```
$ git fetch origin && git rev-parse origin/docs/m95-archive origin/main
c2e41db7c0915a1912857b38dedb8d471c2dbcfc
63080cc40c2f33b80cb4274d68a358f14adb231f
$ git merge-base --is-ancestor c2e41db7 origin/docs/m95-archive && echo anc-ok
anc-ok
$ git log --oneline origin/main..origin/docs/m95-archive
c2e41db7 docs(archive): close-out M-95 — спека, гейт и проба карты CI в docs/archive по норме Р-2 [architect]

$ git diff -M origin/main origin/docs/m95-archive --stat
 crates/gateway-serve/tests/red_m95_catalog_once.rs        |  2 +-
 docs/ROADMAP.md                                           |  2 +-
 {milestones => docs/archive}/M-95-catalog-once.md         | 13 +++++++------
 {scripts/tests => docs/archive}/red_verify_M-95_ci_map.sh |  0
 {scripts => docs/archive}/verify_M-95.sh                  |  0
 5 files changed, 9 insertions(+), 8 deletions(-)
$ git show --numstat --format='' c2e41db7
1	1	crates/gateway-serve/tests/red_m95_catalog_once.rs
1	1	docs/ROADMAP.md
7	6	{milestones => docs/archive}/M-95-catalog-once.md
0	0	{scripts/tests => docs/archive}/red_verify_M-95_ci_map.sh
0	0	{scripts => docs/archive}/verify_M-95.sh

$ for c in a3d77986 2cbb3882 dcfa1465 5b3dbf84 bb44d060 c33a814b b91af0d8; do git merge-base --is-ancestor $c origin/main && echo -n "inmain "; git log -1 --format='%h %s' $c; done
inmain a3d77986 feat(M-95): task #1 — один каталог на подписку [engine-dev]
inmain 2cbb3882 feat(M-95): task #3 — legacy-путь с m95-catalog:legacy и переподписка [engine-dev]
inmain dcfa1465 fix(M-95): R-249 Б-1 — отказ is_fresh/refresh на v1+legacy ⇒ (frozen, true), без проглатывания [engine-dev]
inmain 5b3dbf84 fix(M-95): R-249 Н-4 — cold-путь resume берёт history_start_seq из первого свёрнутого события, не из header.first_seq каталога [engine-dev]
inmain bb44d060 refactor(M-95): R-249 Н-2 — удалить мёртвый LiveReducer::segment_catalog() (ноль вызывающих) [engine-dev]
inmain c33a814b refactor(M-95): R-249 Н-3 — read_and_validate(_with_catalog) сведены к read_and_validate_inner + 2 тонкие обёртки [engine-dev]
inmain b91af0d8 docs(M-95): task #6 — комментарии покрытия ветки отказа (R-250) [engine-dev]

# барьеры в форме CI (pull_request, база = origin/main)
$ export EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) PUSH_BEFORE=
$ bash scripts/check_archived_refs.sh        → VERDICT: PASS                       rc=0
$ bash scripts/check_protected_artifacts.sh  → OK: защищённые артефакты целы на HEAD (63080cc..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)  rc=0
$ bash scripts/check_roadmap_sync.sh         → VERDICT: PASS                       rc=0
$ bash scripts/check_docs_freeze.sh          → (пусто)                             rc=0
$ bash scripts/check_gate_meta.sh            → VERDICT: PASS — вердиктов проверено: 0, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0  rc=0
$ bash scripts/check_artifact_ids.sh         → OK: ни один коммит диапазона 63080cc..HEAD не ввёл второй носитель под занятым идентификатором  rc=0
$ bash scripts/check_ps_on_archive.sh        → VERDICT: PASS                       rc=0
$ bash scripts/check_review_fa.sh            → SKIP (диапазон трогает ТОЛЬКО не-прод пути крейтов — tests/examples/benches …)  rc=0

# висячие ссылки (живой текст = вне docs/archive, research/**, PROJECT-STATE, TECH-DEBT)
$ grep -rnE "milestones/M-95|scripts/verify_M-95|verify_M-95|red_verify_M-95|M-95-catalog-once" . | grep -vE '^\./(docs/archive|research/…)/'
crates/gateway-serve/tests/red_m95_provenance_fresh.rs:32:… (`verify_M-95.sh` — с флагом).   ← Н-1
crates/gateway-serve/tests/red_m95_catalog_once.rs:2:… спека `docs/archive/M-95-catalog-once.md`
docs/ROADMAP.md:115:… `docs/archive/M-95-catalog-once.md`, гейт `docs/archive/verify_M-95.sh` …
milestones/M-96-tail-seek-window.md:133:`verify_M-95.sh` (`A-043`) …                         ← родословная, активный M-96
scripts/verify_M-96.sh:3:# … ВРЕМЕННАЯ копия verify_M-95.sh                                ← родословная, активный M-96

# Н-2: кто исполняет --features testing
$ grep -c "verify_M-" .github/workflows/{ci,deploy,branch-build}.yml
ci.yml:1 (комментарий :635)  deploy.yml:0  branch-build.yml:0
$ grep -n "features" .github/workflows/ci.yml
22:        run: cargo clippy --all-targets --all-features -- -D warnings
$ grep -n "features testing" scripts/verify_M-*.sh   # живые гейты
scripts/verify_M-65.sh:227:if cargo test -p gateway-serve --features testing --test red_ws_session \

$ bash scripts/next_artifact_id.sh R   → R-255
$ bash scripts/next_artifact_id.sh TD  → TD-253

$ git fetch origin && git rev-parse origin/docs/m95-archive   # перед записью
c2e41db7c0915a1912857b38dedb8d471c2dbcfc   (ветка не ушла)
```

## Условие и дальнейшее

APPROVE без условий. После merge'а: запись close-out в `PROJECT-STATE.md` (переезд исполнен),
карточка `TD-253` в `TECH-DEBT.md`. Пруф §8 — только CI на `main`: прод-код не менялся.
