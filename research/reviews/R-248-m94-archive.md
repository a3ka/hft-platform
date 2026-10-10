<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 14c6ae14b54757ecf656d9d37efe526fb9f19ee9
audited_head: 64d04c4c61e751b0b3713f644a502de743c76aa1
verdict: APPROVE
-->

# R-248 — PR-гейт close-out `M-94` (переезд в архив по норме Р-2), PR #331

**Вердикт: APPROVE.** Предмет — ветка `docs/m94-archive`, вершина взята командой
`git fetch origin && git rev-parse origin/docs/m94-archive` → `64d04c4c61e751b0b3713f644a502de743c76aa1`
(совпала со справкой мандата `64d04c4`). База — `origin/main` `14c6ae1` (PR #330, маркер закрытия).

Ярус C прочитан грепом, а не целиком: `PROJECT-STATE.md` — `MS-STATE: M-94`;
`TECH-DEBT.md` — `M-94`, `verify_M-94`, `TD-229`.

## Block-scope

`git diff -M --stat origin/main...origin/docs/m94-archive` → 13 файлов, +64/−22:

| коммит | автор-роль | что | зона |
|---|---|---|---|
| `5dba73f` | architect | `git mv` спеки, гейта, пробы карты CI → `docs/archive/`; статус CLOSED, §11 DONE, строка журнала `R-247`; путь спеки в комментариях 3 RED-оракулов и 5 скриптов | architect (`milestones/*`, `scripts/verify_*`, `*/tests/`) ✔ |
| `e95e568` | architect | `docs/workflow/host-migration-order.md` (+41), урок окна миграции | `docs/` ✔ |
| `3cc3717` | architect | `docs/ROADMAP.md`: S1a-P — архивные пути; TD-229 → `M-95`. Колонка «Состояние» S1a-P не тронута (стояла ✅ ЗАКРЫТ от reviewer'а) | перечень — architect ✔ |
| `64d04c4` | engine-dev | `crates/gateway/src/calc_profile.rs:8` — путь спеки в doc-комментарии | `crates/gateway/src` — engine-dev ✔ |

Все изменённые строки в `crates/**` и `scripts/**` — комментарии (`//!` / `#`), смена
`milestones/M-94-calc-profile.md` → `docs/archive/M-94-calc-profile.md`; исполняемый код не
тронут. Переносы гейта и пробы — 0/0 строк (чистое переименование). Коммиты атомарны,
роль в subject'е, ссылка на `M-94`. Block-C: `crates/contracts/**` не тронут. RISK-BLOCK:
`risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic не нужен.

## Block-FA (M-66)

Тронут `crates/gateway/src/**` → FA `docs/fa/viz-backend.md`. Живые инварианты: **`VB-I-2`**
(live == replay) и **`VB-I-3`** (Read Gateway read-only). Правка — одна строка
doc-комментария, в бинарь не входит; ни `VB-I-2`, ни `VB-I-3` не затронуты.
Модуль `calc_profile` по существу как раз обслуживает `VB-I-2`: один источник определения
расчёта для прогревателя и сервера (`R-247`).

## Done Block — барьеры в CI-форме (`EVENT_NAME=pull_request PR_BASE_SHA=14c6ae1`, дерево `64d04c4`)

```
=== check_archived_refs
PASS  НОВЫХ висячих ссылок нет (проверено 76 артефактов)
PASS  базовая линия актуальна (унаследованных ссылок: 21)
VERDICT: PASS
=== check_ps_on_archive
OK    M-94: переезд, PROJECT-STATE.md несёт маркер CLOSED
VERDICT: PASS
=== check_roadmap_sync
OK: close-out (1) сопровождён правкой docs/ROADMAP.md
    закрыт: docs/archive/M-94-calc-profile.md
VERDICT: PASS
=== check_protected_artifacts
NOTE  milestones/M-94-calc-profile.md: ALLOW-ARTIFACT-DELETE в 5dba73f
OK: защищённые артефакты целы на HEAD (14c6ae1..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
=== check_gate_meta
VERDICT: PASS — вердиктов проверено: 0, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
=== check_review_fa (до этого вердикта)
FAIL  ни один review-файл не введён диапазоном и не назван полным путём в %B (S=∅) — механизм D
```

`check_review_fa` красен ровно по ожидаемой причине (вердикта не было). Этот файл её
устраняет; повторный прогон — в CI PR #331 на вершине с вердиктом.
CI run 37852463081 на `64d04c4`: 19 success, красны только `Review FA` и агрегат.
Сборка и тесты локально не гонялись: исполняемый код не менялся; базовая тройка — в CI.

## Находки

**N-1 (не блокер) — висячая ссылка вне сканируемого барьером текста.**
`config/calc-profile/active.env:8`:
`# меняет имя слепка: сперва next.env (прогрев), потом переключение — milestones/M-94 §3.5.`
После переезда `milestones/M-94…` не существует. Барьер `check_archived_refs.sh` не видит
строку: в его списке `LIVE` нет `config/**`. Файл — прод-носитель профиля, комментарий
в нём читает оператор, меняющий профиль, — ровно «живой текст» по определению шапки барьера.
Правка файла профиля по `П-032` требует ссылки на `П-NNN` (барьер CI), поэтому её
нельзя было сделать в этом PR «попутно». Решение — за architect'ом: расширить
`LIVE` и/или исправить ссылку следующей подписанной сменой профиля. Заведено как TD.

**N-2 (к сведению) — §9-перепроверка.** Диапазон трогает зону `gates.md` §9
(`scripts/verify_M-70.sh`, `scripts/check_calc_profile.sh`, `scripts/tests/**`). Правки —
пути в комментариях, проверенные механически (`check_archived_refs` PASS). Отдельной
перепроверки Fable со свежим контекстом в цепочке нет; норма `COGNITIVE-ONLY`. Для
комментариев-путей это не блокер.

## Условие APPROVE

Выполнено: scope, атомарность, барьеры, FA. Merge — после зелёного `All checks passed`
на вершине с этим вердиктом.

## Handoff §D

APPROVE → merge через PR #331, далее `architect` (запуск `M-95`); находка N-1 — `architect`.
