<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: aa1f4230274cc8008aaf79ade62b224f6b4842a7
audited_head: 977ee4f4b143d723b6e8a368cd5dbbea05c90cd8
verdict: APPROVE
-->

# R-237 — M-92, дополнение к `R-228`: фикс оракула `p12` (Б-1)

**Дата (UTC):** 2026-10-04
**Номер.** Вердикт впервые запушен как `R-235` (`9c6045c`). Тот же номер параллельно получил
`R-235-TD-242-unsupported-msg-pr-gate.md` в `main`, и барьер `artifact-ids` на PR #310 покраснел.
Перенумерован по новому резерву `scripts/reserve_artifact_id.sh R` → `R-237`. Содержание не менялось.
**Предмет:** ветка `origin/feat/M-92-manifest-verified-prune`, PR #310. Вершину я взял командой
(`git fetch origin && git rev-parse`) — `977ee4f`, совпадает со справкой мандата.
**База:** `aa1f423` — коммит вердикта `R-228` (REJECT над `5307f04`). После него на ветке ровно
два коммита architect'а: `f8d37a8` (тест `p12`) и `977ee4f` (строка журнала кругов в спеке).
`origin/main` на момент проверки — `e4cab87`; `git merge-tree --write-tree origin/main 977ee4f` → exit=0.

**Вердикт: APPROVE.** `R-228` Б-1 закрыт. Сценарий `p12` больше не зависит от скорости записи,
страж подготовки на месте, мутант роняет основную проверку, все чеки PR #310 зелёные.

## Живые инварианты FA (`crates/journal/**` → `docs/fa/journal.md`)

- **`JR-I-13`** (`docs/fa/journal.md:221` на ветке): локальный сегмент удаляется ТОЛЬКО при сверке
  его sha256 с офсайт-копией. Код, который его держит, после `R-228` не менялся (см. диф ниже).
- **`JR-I-2`** (`docs/fa/journal.md:112`, сплошной `seq`): `p12` проверяет, что удаление
  оставляет сплошной суффикс (`assert_contiguous`) и не уходит за известную плану
  не-кандидатную позицию.

Ярус C, искал грепом по `origin/main`: в `TECH-DEBT.md` и `PROJECT-STATE.md` — `M-92`, `TD-020`,
`TD-202`. Состояние то же, что в `R-228`: обе карточки открыты, закрытие — на close-out после
задачи 4.

## Что проверено

| пункт мандата | как проверено | итог |
|---|---|---|
| код не менялся | `git diff aa1f423..977ee4f --stat` | 2 файла: `crates/journal/tests/red_m92_manifest_prune.rs` (+15/−16), `milestones/M-92-*.md` (+1). `crates/*/src` не тронут. `git show --numstat` обоих коммитов совпадает с заявленным |
| (1) сценарий не зависит от скорости | чтение дифа + `crates/journal/src/lib.rs:233-239` + `segments.rs` `retention_plan` | Ротация идёт ДО записи (`seg_size + frame_len > max` ⇒ `rotate()`, затем `write_all`). Значит событие, решённое «свежим» при двух файлах на диске, и есть первое событие сегмента 2, при любой скорости. Сегмент 1 открыт событием, решённым при одном файле, то есть обычным. План судит возраст по `ts_exch_ms` ПЕРВОГО события (`segments.rs:4890`), так что свежий хвост сегмента 1 его решения не меняет. Двухпроходной схемы в файле больше нет: грепом `let probe` / `probe.path` — 0 вхождений |
| (1) замер, а не только рассуждение | временная копия теста с паузой 20 мс каждые 50 событий в цикле `p12` (та пауза, что в `R-228` сдвигала `first_seq` на 4-8). Файл удалён, в ветку не попал | `p12 ... ok` |
| (1) под нагрузкой | 20 прогонов `p12` при 4 потоках CPU-нагрузки | ok=20 fail=0 |
| (1) страж подготовки остался | `red_m92_manifest_prune.rs:504-508` и `:513-520` | Оба на месте: `сегментов >= 5 && fresh_written > 0`, и «`fresh` в `skipped`, кандидаты по обе стороны». Второй — тот, что упал в CI по `R-228`, — не ослаблен |
| (2) анти-плацебо: реверс `25dd118` | `git show 25dd118 -- crates/journal/src/segments.rs \| git apply -R`, прогон набора, `git checkout --` | `p12 ... FAILED`, паника на `red_m92_manifest_prune.rs:535:5`, то есть на `assert_eq!(gone, older, ...)`, основной проверке, не на страже. Остальные 20 зелёные. Код возвращён, `git status --porcelain` пуст |
| (3) чеки PR #310 | `gh pr checks 310` | 21 из 21 `pass`, включая `fmt + clippy + test` (16m3s) и `All checks passed`; exit=0 |
| scope | коммиты после `aa1f423` | оба `[architect]`, зона architect'а (`*/tests/**`, `milestones/`). Block-C N/A, RISK-BLOCK не применяется (`risk`/`killswitch`/`oms`/`venue-*` не тронуты) |
| режим удаления | `deploy/cron.d/journal-retention` не в дифе | `RETENTION_MODE=dry-run` остаётся. Включение — задача 4 architect'а по `П-031` |

## Наблюдения (не блокируют)

- **N-1.** `R-228` спрашивал, подвержены ли другие оракулы файла тому же классу. Architect в §13
  ответил грепом: двухпроходной схемы больше нет. Я подтверждаю это тем же грепом. Остальные
  оракулы строят сценарий по позиции внутри ОДНОГО журнала, поэтому от скорости записи не зависят.
- **N-2.** Долг из `R-223` N-2 и `R-221` Н-2/Н-3/Н-5 по-прежнему заводится карточкой на close-out
  M-92, после задачи 4.

## Done Block (сырой вывод, агрегирован)

```
$ git fetch origin && git rev-parse origin/feat/M-92-manifest-verified-prune
977ee4f4b143d723b6e8a368cd5dbbea05c90cd8
$ git diff aa1f423..HEAD --stat
 crates/journal/tests/red_m92_manifest_prune.rs | 31 +++++++++++++-------------
 milestones/M-92-manifest-verified-prune.md     |  1 +
 2 files changed, 16 insertions(+), 16 deletions(-)
$ git merge-tree --write-tree origin/main(e4cab87) 977ee4f >/dev/null; echo exit=$?
exit=0

$ cargo test -p journal --test red_m92_manifest_prune
test result: ok. 21 passed; 0 failed                                    exit=0
# p12 с паузой 20 мс / 50 событий (временный файл, удалён)
test p12_known_non_candidate_position_blocks_younger ... ok
# 20 прогонов p12 под 4 потоками CPU-нагрузки
p12 под нагрузкой: ok=20 fail=0

# мутант: git show 25dd118 -- crates/journal/src/segments.rs | git apply -R
test p12_known_non_candidate_position_blocks_younger ... FAILED
panicked at crates/journal/tests/red_m92_manifest_prune.rs:535:5     (assert_eq!(gone, older))
test result: FAILED. 20 passed; 1 failed
$ git checkout -- crates/journal/src/segments.rs && git status --porcelain
(пусто)

$ gh pr checks 310; echo checks_exit=$?
... 21 строка, все pass (All checks passed · fmt + clippy + test 16m3s · fmt + clippy + test (ветка) 14m3s · ...)
checks_exit=0

# дерево /tmp/hft-reviewer-M-92-r3 на 977ee4f, прогон 2
$ bash scripts/verify_M-92.sh 2>&1 | grep -E '^(FAIL|VERDICT)'; echo exit=$?
VERDICT: PASS
verify_exit=0              (PASS: 58; FAIL: 0; SKIP: 7 — task4 по спеке + 6 по карте CI;
                            ci-parity: учтено шагов 57 из 57 (исполнено 51, исключено по карте 6))
PASS  task1-3: red_m92_manifest_prune (p1-p12, b1-b2, c0-c6)
PASS  task1: red_retention + red_retention_checkpoint_coverage + red_retention_compacted + red_retention_operator
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
PASS  ci-parity: cargo test --all
```

**Прогон 1 скрипта приёмки был КРАСНЫМ, и это записано, а не скрыто.** `VERDICT: FAIL (провалов: 1)`,
exit=1: `task1` → `red_retention.rs:63:18`, `disk_guard_halts_writes_explicitly_when_free_space_is_low`.
Это известный `TD-151` (оракул диска берёт порог `free + 1` и проигрывает гонку с хостом, где соседние
сессии освобождают место). Файл в проверяемом диапазоне не тронут (последний коммит `ebc509c`, M-08).
Повтор отдельно: 5/5 зелёные, весь набор task1 зелёный; прогон 2 целиком — PASS. В CI этот тест
зелёный. Решение принято по коду возврата прогона 2 и чеков PR, а рецидив `TD-151` я допишу в
карточку на close-out.

## Handoff

APPROVE → reviewer мержит PR #310 по коду возврата `gh pr checks 310 --watch` → `gates.md` §8
(режим остаётся `dry-run`). Дальше задача 4 architect'а по `П-031`, первым пунктом (а0). Спека
M-92 в архив не переносится, пока задача 4 открыта.
