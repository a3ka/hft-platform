<!-- GATE-META
milestone: M-96
audited_repo: a3ka/hft-platform
audited_base: 63080cc40c2f33b80cb4274d68a358f14adb231f
audited_head: 55592e2b4f647c098c880c81d1fba3fbfa3c74a2
verdict: APPROVE
-->

# R-256 — PR-гейт close-out `M-96` (решение «закрыть с остатком в `TD-252`» + переезд по Р-2)

**Вердикт: APPROVE.** Предмет — ветка `docs/m96-closeout`. Вершина взята командой
`git fetch origin && git rev-parse origin/docs/m96-closeout` → `55592e2b4f647c098c880c81d1fba3fbfa3c74a2`,
совпала со справкой мандата `55592e2b`. База — `origin/main` `63080cc4`. Повторный `fetch` перед
записью вердикта: вершина та же, ветка не ушла.

Ветка — два коммита architect'а поверх базы:

- `c2e41db7` — переезд закрытого `M-95`. **Этот коммит уже судил `R-255` APPROVE** (ветка
  `docs/m95-archive`, PR #337 открыт на момент вердикта). Здесь его НЕ пересуживаю; проверил
  только то, что мандат назвал явно (п. 2 — SHA §11 `M-95`): все семь — предки `origin/main`,
  `R-253` существует.
- `55592e2b` — `M-96`: решение architect'а и переезд. Предмет этого вердикта.

**Startup-протокол:** `CLAUDE.md`; `.claude/rules/{gates,scope-guard,commit-discipline,branch-hygiene,handoff-block,testing}.md`;
`.claude/agents/reviewer.md`; `docs/04-workflow.md` §2 «Предмет гейта — ВЕТКА» и «Close-out …
(Р-2)»; шапка `scripts/check_ps_on_archive.sh`.
**Ярус C — грепом:** `PROJECT-STATE.md` по `MS-STATE: M-9[4-6]`, раздел `M-96`;
`TECH-DEBT.md` по `TD-249`…`TD-253`, заголовкам корзин (`^# `), образцу закрытия `TD-242`
(`420a1024`); `docs/archive/TECH-DEBT-closed-2026-08-16.md` — формат закрытой карточки (`TD-241`).

**FA.** Диапазон трогает `crates/gateway-serve/tests/red_m95_catalog_once.rs` (только путь в
комментарии, коммит `c2e41db7`). Живые инварианты по предмету: `VB-I-11`
(`docs/fa/viz-backend.md:294`, провенанс истории — предмет `M-95`) и `JR-I-2`
(`docs/fa/journal.md:112`, `seq` без дыр — выдача после сдвига начинается ровно с `after+1`;
предмет `M-96`). Код этим PR не меняется.

## Блоки

| блок | результат |
|---|---|
| Block-scope | `55592e2b`: 4 пути — переезд `milestones/M-96-tail-seek-window.md`, `scripts/verify_M-96.sh`, `scripts/tests/red_verify_M-96_ci_map.sh` → `docs/archive/`, ячейка `docs/ROADMAP.md:116`. Всё в зоне architect'а по Р-2. Impl-кода, `contracts/**`, `PROJECT-STATE`/`TECH-DEBT` нет. Мой коммит `cb6360fc` — `PROJECT-STATE.md`, `TECH-DEBT.md`, его архив закрытых карточек, колонка «Состояние» `ROADMAP.md` — зона reviewer'а |
| Block-C | `crates/contracts/**` не тронут |
| Block-risk | `risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic не требуется |
| Переезд без потерь (п. 1) | `git diff --quiet origin/main:scripts/<f> HEAD:docs/archive/<f>` — все четыре гейта/пробы (`verify_M-95/96.sh`, `red_verify_M-95/96_ci_map.sh`) байт-в-байт. Спеки: word-diff показывает ТОЛЬКО строку статуса, колонку Status §11 и новую строку журнала (§14) — нормативные секции не менялись |
| Status §11 (п. 2) | `M-96`: задача 1 `ee8ffa1d` — предок `origin/main`, subject `fix(M-96): task #1 — окно поиска в обе стороны…`; задача 2 — `R-254` (файл есть). `M-95`: `a3d77986`, `2cbb3882`, `dcfa1465`, `5b3dbf84`, `bb44d060`, `c33a814b`, `b91af0d8` — все в `main`, `R-253` есть |
| Решение §14 круг 4 (п. 3) | см. §1 ниже — согласен |
| Висячие ссылки (п. 4) | `grep -rnE 'milestones/M-9[56]\|scripts/verify_M-9[56]\|scripts/tests/red_verify_M-9[56]'` вне `docs/archive/**` и `research/{reviews,critiques,arbitration}/**` — **ноль** для M-96. Для M-95 — одна, `R-255` Н-1 (см. Н-3) |
| Барьеры | до маркера `check_ps_on_archive` rc=1 (по построению), после — rc=0; прочие семь rc=0 на обеих вершинах (Done Block) |

## 1. Решение о закрытии `M-96` — согласен

`R-254` §5 прямо оставил architect'у выбор «закрыть с остатком в `TD-252` либо продолжить»;
решение §14 круг 4 ему не противоречит, а исполняет его. Проверено по существу:

- **цель, ради которой милестоун заведён,** — флак `w1`, красивший `main` и державший деплой
  (`TD-250`, `R-248`), — снята. Наблюдение: после влития `M-96` `w1` прошёл во ВСЕХ пяти прогонах
  CI (лог джоба `fmt + clippy + test`, строка `test w1_first_pump_after_warm_resume_costs_tail_not_prefix ... ok`):
  push `main` `53153ae6` (run 38056916600), `63080cc4` (38059783282); PR `77edd463` (38054330334),
  `937bdc9e` (38055635181), `d467f671` (38058712934). Красных нет. Основание не число прогонов, а
  детерминированный оракул `t1`/`t5` и мутант `WIN_HALF` 16→4 КиБ ⇒ `t5` FAILED (`R-252`);
- **остаток** — быстрый путь на проде не достигается (ошибка оценки 3.7…16.7 МБ при окне ±16 КиБ) —
  назван, измерен (`R-254` §2), оценён (≈2–3 % байт подписки), корректность не затронута; подстройкой
  окна не лечится (ошибка на три порядка больше окна), дом — указатель «seq → смещение» с S2;
- граница C не задета: фазы, деньги, состав данных — нет. Founder вправе отменить (§E мандата).

`R-252` Н-1 записана в карточку `TD-252` (этим кругом); Н-2 (256 КиБ в §4/§8 против 172 КиБ
оракула) названа в журнале спеки — нормативную секцию архивной спеки не правят, действует оракул.
Принимаю.

## Находки (неблокирующие)

**Н-1 (NOTE, architect) — «✅ DONE» у задачи 2 при невыполненном критерии.** Критерий §11
задачи 2 — «подписка читает быстрым путём»; он не выполнен, а ячейка несёт `✅ DONE` с
оговоркой «быстрый путь на проде НЕ достигнут — остаток `TD-252`». Оговорка честная и рядом, так
что читатель не обманут; но знак «DONE» означает «исполнена (замер снят)», а не «достигнута».
Править архивную спеку не требую — фиксирую трактовку здесь.

**Н-2 (reviewer-owned, мой реестр) — расхождение счёта «В ПЛАНЕ».** `TD-249` и `TD-250` заведены
(`R-248`) карточками без строки сводной таблицы и без изменения счёта; `TD-251`/`TD-252` имеют
строки таблицы, но счёт `76` не менялся с `R-247`. Закрытие `TD-250` поэтому записано «76 → 76»
с явной оговоркой. Пересчёт корзины — отдельной ревизией реестра, не этим PR.

**Н-3 (не этого предмета) — `R-255` Н-1 жива и на этой ветке.**
`crates/gateway-serve/tests/red_m95_provenance_fresh.rs:32` ссылается на `verify_M-95.sh` как на
единственного исполнителя `--features testing`-оракулов; гейт в архиве. Это предмет `R-255`
(Н-1, Н-2 → `TD-253`), здесь не пересуживается и не дублируется карточкой.

## Что сделано reviewer'ом (`cb6360fc`)

- `PROJECT-STATE.md`: `<!-- MS-STATE: M-96 OPEN -->` → `CLOSED`, заголовок «✅ ЗАКРЫТ с остатком
  в `TD-252`», разделы «Долг» и «Закрытие»;
- `TECH-DEBT.md`: `TD-250` — тело в `docs/archive/TECH-DEBT-closed-2026-08-16.md` с записью
  закрытия и пятью прогонами; `TD-252` — дом остатка и `R-252` Н-1 (`segments.rs:3741`,
  `i = frame_end` в `seek_back_from_tail` `:3631`); заметка у счёта корзины;
- `docs/ROADMAP.md:116` — колонка «Состояние» строки `TD-250`: ✅ ЗАКРЫТ с остатком. Строка
  `TD-229` уже несла «✅ ЗАКРЫТ 2026-10-10» — не трогал.

## Done Block

```
$ git fetch origin && git rev-parse origin/docs/m96-closeout origin/main
55592e2b4f647c098c880c81d1fba3fbfa3c74a2
63080cc40c2f33b80cb4274d68a358f14adb231f

$ git diff -M origin/main origin/docs/m96-closeout --stat
 crates/gateway-serve/tests/red_m95_catalog_once.rs        |  2 +-
 docs/ROADMAP.md                                           |  4 ++--
 {milestones => docs/archive}/M-95-catalog-once.md         | 13 +++++++------
 {milestones => docs/archive}/M-96-tail-seek-window.md     |  7 ++++---
 {scripts/tests => docs/archive}/red_verify_M-95_ci_map.sh |  0
 {scripts/tests => docs/archive}/red_verify_M-96_ci_map.sh |  0
 {scripts => docs/archive}/verify_M-95.sh                  |  0
 {scripts => docs/archive}/verify_M-96.sh                  |  0
 8 files changed, 14 insertions(+), 12 deletions(-)

$ for f in verify_M-95.sh verify_M-96.sh; do git diff --quiet origin/main:scripts/$f HEAD:docs/archive/$f && echo "$f identical"; done
verify_M-95.sh identical
verify_M-96.sh identical
$ (то же для scripts/tests/red_verify_M-9{5,6}_ci_map.sh)
red_verify_M-95_ci_map.sh identical
red_verify_M-96_ci_map.sh identical

# барьеры в CI-форме, EVENT_NAME=pull_request PR_BASE_SHA=63080cc4…, на 55592e2b (до маркера)
check_archived_refs rc=0 | VERDICT: PASS
check_protected_artifacts rc=0
check_roadmap_sync rc=0 | VERDICT: PASS
check_artifact_ids rc=0
check_gate_meta rc=0 | VERDICT: PASS — вердиктов проверено: 0
check_docs_freeze rc=0
check_review_fa rc=0
check_ps_on_archive rc=1 | FAIL  PROJECT-STATE.md несёт «M-96 OPEN», а спека M-96 лежит в docs/archive/ — реестр противоречит дереву

# то же на cb6360fc (после маркера)
check_archived_refs rc=0 | VERDICT: PASS
check_protected_artifacts rc=0
check_roadmap_sync rc=0 | VERDICT: PASS
check_artifact_ids rc=0
check_gate_meta rc=0 | VERDICT: PASS
check_docs_freeze rc=0
check_review_fa rc=0
check_ps_on_archive rc=0 | VERDICT: PASS

$ gh run list --branch main --event push --workflow ci.yml --limit 2
38059783282 63080cc4 completed success 2026-10-10T14:29:12Z
38056916600 53153ae6 completed success 2026-10-10T13:45:00Z

$ gh api repos/a3ka/hft-platform/actions/jobs/<J>/logs | grep 'test w1_first_pump_after_warm_resume_costs_tail_not_prefix \.\.\.'
38056916600: ... ok      38059783282: ... ok
38054330334: ... ok      38055635181: ... ok      38058712934: ... ok

$ git show --numstat --format='%h %s' cb6360fc
13	5	PROJECT-STATE.md
11	34	TECH-DEBT.md
1	1	docs/ROADMAP.md
42	0	docs/archive/TECH-DEBT-closed-2026-08-16.md

$ bash scripts/next_artifact_id.sh R
R-256
```

## Handoff §D

Следующий — **`architect`** (после merge): остаток по `TD-252` (указатель «seq → смещение» с
S2), `TD-251`, `TD-253` (`R-255`), `TD-245…248`; удалить `harness/td250-w1-difference`.
