<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: fc47008fc43d0e04e75c4f32130de2210c6cd2f0
audited_head: 432aa3e5a6cfd2ccd3bf10d64da235b7024d9777
verdict: ESCALATE
-->

# C-293 — M-95 catalog-once, круг 7: исполнение C-292 — ESCALATE(→approved)

**Роль:** critic, суженный plan-time круг только по исполнению `C-292` для
`R-249` Н-4. **Предмет:** ветка `origin/feat/M-95-catalog-once`; вершина
получена командой и равна `432aa3e5a6cfd2ccd3bf10d64da235b7024d9777`.
**Диапазон architect:** `fc47008f..432aa3e5` (`96cbe76f`, `432aa3e5`).

## Verdict: ESCALATE(→approved)

Технического REJECT в суженном предмете нет. Новый RED-оракул действительно
пиннит `VB-I-11`: cold `LiveReducer::resume` обязан объявлять seq первого
**реально свёрнутого** события, а не `header.first_seq` каталога. Он различает
полный и усечённый legacy-журнал, проходит на образце прежнего источника и
падает на всех трёх требуемых мутациях.

`R-249` Б-1 (`f3`/`f4`) уже подтверждён `C-292` и намеренно не
переоткрывался. `UnlistableDir` исправлен в новом committed тестовом наборе:
`seal` сохраняет `metadata(...).permissions().mode() & 0o7777`, а `Drop`
восстанавливает именно сохранённый режим; это закрывает NOTE `C-292`, не
меняя его технический вердикт.

Это третий круг нити после `C-286` / `R-249` / `C-292`. Поэтому формальный
маршрут — `ESCALATE` по `gates.md` §0 п.2. Как разъяснено `A-051` §1–§2,
технический plan-time результат уже достаточен для `→approved`: architect
добавляет только механическую запись этого вердикта в журнал кругов, затем
диспетчеризует engine-dev. Новый технический круг critic не требуется.

## Artifact and scope check

| Required artifact | Result |
|---|---|
| T-contracts / trait signatures | N/A — `fc47008f..432aa3e5` не меняет `contracts/**`, публичные T-типы или trait-сигнатуры; задача 5 не требует нового контракта. |
| RED | `crates/gateway/tests/red_m95_cold_resume_provenance.rs` добавлен architect; `c1` RED на голове, `c0` — парный страж. |
| Verify | `scripts/verify_M-95.sh` добавляет отдельный task-5 шаг, использует `set -uo pipefail`, агрегирует `FAIL` и выходит ненулево при провале. CI-map dry-run учёл 61/61 шаг. |
| Milestone | §3/§6 запрещают смену источника cold provenance; §8 определяет `c0`/`c1` и мутации; §11 task 5 ссылается на оба оракула; §14 фиксирует круг 6. |

Задача 5 исполнима engine-dev в §10: Б-1 затрагивает только
`crates/gateway-serve/src/lib.rs`; Н-2/Н-4 — только
`crates/gateway/src/lib.rs`; Н-3 — общая часть в уже разрешённых gateway /
gateway-serve source paths. RED-тесты, verify и milestone остаются sacred
architect paths; engine-dev их не правит.

## Oracle evidence

1. На audit head текущий cold path берёт provenance через
   `current_history_provenance_with_catalog`; эта функция читает
   `s.header.first_seq`. На legacy-файле header синтезирован как `0`, поэтому
   `c1` падает, а `c0` проходит.
2. Локальный временный образец прежнего источника заменил это значение на
   `first_seq` из фактически пройденного `journal::stream`, с
   `history_truncated = history_start_seq > 0`: `c0` и `c1` прошли (2/2).
3. Временный мутант безусловного `(0, false)` уронил `c1`; текущий
   catalog/header путь даёт тот же наблюдаемый провал `c1`; мутант «всегда
   truncated = true» уронил `c0`.
4. Setup-стражи находятся в самом оракуле: legacy-сегмент до запуска виден
   через `journal::stream` с ожидаемым первым seq и длиной; холодность
   подтверждает `stats.events_scanned >= N` при пустом каталоге слепков.

Все временные изменения `crates/gateway/src/lib.rs` откачены до записи
вердикта; рабочее дерево перед добавлением verdict-файла было чистым.

## Required mechanical next step

1. Architect добавляет только запись `C-293` / `ESCALATE(→approved)` в §14
   `milestones/M-95-catalog-once.md`; нормативные секции, RED и verify не
   меняются.
2. Architect передаёт engine-dev задачу 5 строго по §10 и §11. Каждый из
   Б-1, Н-4, Н-2, Н-3 получает свой commit с ссылкой на `R-249`; dev не
   меняет `*/tests/**` или `scripts/verify_M-95.sh`.
3. После GREEN task 5: tester, затем повторный PR-time reviewer; reviewer
   выполняет task 4 (§8 production measurement) в своём маршруте.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-95-catalog-once
432aa3e5a6cfd2ccd3bf10d64da235b7024d9777
exit=0

$ git log --oneline fc47008f..432aa3e5
432aa3e5 docs(M-95): C-292 — оракул холодного resume в §8, задача 5, шаг гейта; журнал круга 6 [architect]
96cbe76f test(M-95): C-292 — RED холодного resume (c0/c1), UnlistableDir возвращает исходный режим [architect]
exit=0

$ cargo test -p gateway --test red_m95_cold_resume_provenance -- --nocapture
test c0_cold_resume_full_history_is_complete ... ok
test c1_cold_resume_legacy_pruned_prefix_declares_real_first_seq ... FAILED
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101  # expected RED baseline at plan time

$ <local specimen: cold provenance = first scanned event; truncated = start > 0>; cargo test -p gateway --test red_m95_cold_resume_provenance -- --nocapture
test c1_cold_resume_legacy_pruned_prefix_declares_real_first_seq ... ok
test c0_cold_resume_full_history_is_complete ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ <local mutant: unconditional (0, false)>; cargo test -p gateway --test red_m95_cold_resume_provenance c1_cold_resume_legacy_pruned_prefix_declares_real_first_seq -- --nocapture
test c1_cold_resume_legacy_pruned_prefix_declares_real_first_seq ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out
exit=101

$ <current catalog/header.first_seq mutant>; cargo test -p gateway --test red_m95_cold_resume_provenance c1_cold_resume_legacy_pruned_prefix_declares_real_first_seq -- --nocapture
test c1_cold_resume_legacy_pruned_prefix_declares_real_first_seq ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out
exit=101

$ <local mutant: history_truncated = true>; cargo test -p gateway --test red_m95_cold_resume_provenance c0_cold_resume_full_history_is_complete -- --nocapture
test c0_cold_resume_full_history_is_complete ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out
exit=101

$ git restore --source=HEAD -- crates/gateway/src/lib.rs && git status --porcelain
exit=0

$ VERIFY_M95_CI_DRY=1 bash scripts/verify_M-95.sh
PASS  ci-parity: учтено шагов 61 из 61 (исполнено 55, исключено по карте 6)
VERDICT: PASS
exit=0

$ EVENT_NAME=pull_request BASE_SHA=ad20a7e7... HEAD_SHA=<C-293> PR_BASE_SHA=ad20a7e7... PR_HEAD_SHA=<C-293> bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (ad20a7e..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request BASE_SHA=ad20a7e7... HEAD_SHA=<C-293> PR_BASE_SHA=ad20a7e7... PR_HEAD_SHA=<C-293> bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 7, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
exit=0

$ EVENT_NAME=pull_request BASE_SHA=ad20a7e7... HEAD_SHA=<C-293> PR_BASE_SHA=ad20a7e7... PR_HEAD_SHA=<C-293> bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона ad20a7e..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-09T00:00Z
- Milestone: M-95-catalog-once
- Статус: DONE — ESCALATE(→approved), formal routing only
- HEAD: 432aa3e — architect artifact subject audited by this verdict

## §B — Что я сделал
- Проверил committed response `fc47008f..432aa3e5`; Б-1 не переоткрывал.
- Исполнил baseline, прежний источник и три мутационных прогона cold-resume RED.

## §C — Артефакты / результаты
- `research/critiques/C-293-M-95-catalog-once-round7.md`
- Done Block: raw commands and exit codes above.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-95-catalog-once: C-293 вынес ESCALATE(→approved) на исполнение C-292. Добавь ТОЛЬКО механическую строку C-293 в §14 milestones/M-95-catalog-once.md: технических REJECT нет; третий круг нити C-286 → R-249 → C-292 → C-293 маршрутизирован по gates.md §0 п.2 / A-051 §1–§2. Не меняй RED, verify или нормативные секции. Затем передай engine-dev task 5 §11 в зоне §10: отдельные commits для R-249 Б-1, Н-4, Н-2, Н-3; dev не правит tests/scripts. Начальная вершина: <critic verdict commit after push>.
  ```
- Push-статус: pending critic verdict commit and CI-form barriers
- ⏸ кэш оставлен — нужен до завершения commit/push и барьеров

## §E — Риски / открытые вопросы
- Формальный маршрут третьего круга требует mechanical appendix; технических замечаний к C-292 нет.
- `f3`/`f4` не переоткрывать: их уже подтвердил C-292.

=== END HANDOFF ===
