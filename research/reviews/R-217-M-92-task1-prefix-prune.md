<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 6ef8c672f3895e85ed73628c81ab3d3089638d69
audited_head: b318f50470640a1adba821519eb5fc0c7ada0e11
verdict: REJECT
-->

# R-217 — M-92, промежуточный PR-гейт задачи 1 (`I-2bis`, удаление префиксом)

**Дата (UTC):** 2026-10-03
**Предмет:** ветка `origin/feat/M-92-manifest-verified-prune`; вершина взята командой — `b318f50`.
Справка мандата tester'а: `b318f50` (совпала); коммит engine-dev `6003fe3` — предок
(`merge-base --is-ancestor` = 0). Судится код задачи 1 на вершине (`80b363f` + `6003fe3`),
попутно — бинарь задачи 2 (`6b1fa76`), он тоже на ветке.

**Вердикт: REJECT (CHANGES REQUESTED) → architect.** Работа dev'а по букве `I-2bis` выполнена
верно и оракулами защищена (мутация ниже). Отказ — по двум находкам, обе — расхождение
реализации с ОБЕЩАНИЕМ спеки, а не с её оракулами; первая доказана пробой.

**Merge в `main` невозможен независимо от вердикта:** задачи 3–5 открыты, `red_m92` красен
(4 из 17), `verify_M-92.sh` — `VERDICT: FAIL`. PR не открывался.

## Живые инварианты FA (`crates/journal/**` → `docs/fa/journal.md`)

- **`JR-I-2`** — «`seq` строго монотонен без дыр; разрыв при чтении → abort». Находка Б-1 —
  её нарушение уборщиком.
- **`JR-I-11`** — ни один путь чтения не сшивает немонотонный каталог молча; судит ПОРЯДОК, а не
  сплошность (`A-045` §6 N-1) — поэтому дыру из Б-1 читатели не видят.

Ярус C, что искал грепом: `TECH-DEBT.md` (`origin/main`) — `M-92`, `TD-020`, `TD-202`,
«разрыв/дыра/сплошность между сегментами» (карточки на `A-045` N-1 НЕТ); `PROJECT-STATE.md` — `M-92` (0 вхождений).

## Находки

### Б-1 (блокер) — «префикс ПЛАНА» ≠ «префикс КАТАЛОГА»: уборщик всё равно оставляет дыру

`segments.rs:3132-3280` (`retention_execute_with_manifest`) обрывает удаление на первом
неподтверждённом кандидате ПЛАНА, но не проверяет, что сами кандидаты плана — сплошной префикс
КАТАЛОГА. `retention_plan` (`segments.rs:5005-5040`, `enumerate_retention_segments`) выводит
из плана сегмент, который не классифицируется (порча/усечение заголовка, чужой файл, legacy без
декларации), и оставляет в плане старших И младших соседей. Все совпали с манифестом ⇒ удалены ⇒
в каталоге дыра. Спека `I-1` обещает «каталог остаётся сплошным суффиксом», `I-2bis` — лишь
«префикс плана»: обещание `I-1` не выполняется, а оракул (`assert_contiguous` в `p1`/`p2`/`p3`/`p8`/
`p9`/`b2`) этот случай не строит — все фикстуры имеют план, который и так префикс каталога.

**Воспроизведение** (временный тест `crates/journal/tests/zz_reviewer_probe.rs` в моём дереве,
удалён; фикстура = `world()` из `red_m92`: 900 событий, сегменты по 8 KiB, `keep_min=1`,
покрытие — всё кроме активного; заголовок `segment-00000002.jrnl` перезаписан 4 байтами;
манифест — честные суммы по именам плана):

```
PROBE catalog before: [segment-00000000 … segment-00000005]
PROBE corrupted: segment-00000002.jrnl
PROBE plan: ["segment-00000000.jrnl", "segment-00000001.jrnl", "segment-00000003.jrnl"]
PROBE pruned=3 failed=0
PROBE catalog after: ["segment-00000002.jrnl", "segment-00000004.jrnl", "segment-00000005.jrnl"]
```

Сегмент 3 удалён, 2 остался — разрыв 2 → 4, отчёт `failed=0`, выход бинаря был бы `0`.
Класс — ровно тот, ради которого введена `I-2bis` (`A-045` §3: ни один читатель дыру не видит).
Пред-существует и на legacy-пути `retention_execute`, но M-92 впервые ОБЕЩАЕТ сплошность (`I-1`).
Дизайн защиты (обрыв по каталогу, а не по плану; либо отказ плана с «дыркой») и RED на неё —
зона architect'а (`gates.md` §4, граница reviewer↔architect).

### Б-2 (блокер) — типовой барьер `ColdCopyProof` на манифестном пути снят

Спека §4 (форма «ДОСЛОВНО»): «`ColdCopyProof` выдаётся только при равенстве
sum(local) == manifest[name] (приватный конструктор сохраняется — тот же типовой барьер)».
Реализация удаляет сырым `fs::remove_file(&seg.path)` (`segments.rs:3256`) — `ColdCopyProof` не
выдаётся и `prune_segment(seg, proof)` (`segments.rs:2910`) не зовётся. Удаление данных журнала
снова выразимо без доказательства; барьер остался только на legacy-пути. Оракулы этого не
различают (черный ящик) — расхождение видно только чтением. Решение — architect: либо
привести код к спеке (выдача proof после сверки → `prune_segment`), либо снять фразу из спеки
с обоснованием.

### Н-1 — `DryRun` не видит несовпадения сумм, отчёт расходится с `Apply`

`segments.rs:3141-3184`: в `DryRun` хеширования нет, обрыв ставится только на «нет строки».
Испорченная копия старейшего кандидата в dry-run даёт `failed` пустой у младших, а `apply`
оборвёт ВСЁ. Сообщение `6003fe3` («отчёт согласован с Apply») верно только для `absent-remote`.
Важно для задачи 4(а): dry-run прод-пути покажет «всё сходится» там, где apply ничего не удалит.
Хеширование — чтение, не побочный эффект (`I-4` не мешает). Решение — architect.

### Н-2 — `offloaded` заполняется на пути, где ничего не копируется

`segments.rs:3258`: `offloaded.push` при каждом удалении; бинарь печатает `offloaded: N`
(`journal-retention.rs:767`). Оператор читает «выгружено N» там, где выгрузки не было.

### Н-3 — двойное хеширование при несовпадении

`segments.rs:3243`: сумма считается второй раз ради текста причины (сегменты до сотен МБ).
Эффективность, не корректность.

### Н-4 — кандидат TD из `A-045` §6 N-1 (reviewer-owned)

«Читатель журнала обязан отказать на разрыве `seq` между сегментами» — `stream`/`read_all`/
`recover` на дыре отдают `Ok(562)`. Карточки в `TECH-DEBT.md` нет (греп выше). Заводится
reviewer'ом на close-out M-92 вместе с прочими правками реестра — здесь зафиксирован, чтобы не
потеряться.

### Н-5 — мандаты tester'у: фильтр через `--`

Замечание tester'а подтверждено: `cargo test … p8 p9` отказывает (`unexpected argument`),
рабочая форма — `cargo test -p journal --test red_m92_manifest_prune -- p8 p9`.

## Scope / дисциплина — чисто

- Коммиты engine-dev: `80b363f` (`lib.rs`, `segments.rs`), `6b1fa76` (`bin/journal-retention.rs`),
  `6003fe3` (`segments.rs`, 89/19) — все в Allowed paths §7, атомарны, со ссылкой на задачу.
- `*/tests/**` коммитами engine-dev не тронуты (0). `scripts/verify_*` — не тронуты.
- `crates/contracts/**` — не тронут (Block-C N/A). `risk`/`killswitch`/`oms`/`venue-*` — не тронуты
  (RISK-BLOCK не применяется; RAW-гейт критика пройден `C-268`…`C-274` + `A-044`/`A-045`).
- Статус-колонка §8 не обновлена dev'ом (все ⏳) при сделанных задачах 1–2 — колонку ведёт
  architect по итогам.

## Мутационный контроль (свой)

Нейтрализован обрыв на несовпадении (`segments.rs:3246` `blocked_by = Some(name)` → no-op):
`p8` FAILED «несовпал старейший … в каталоге дыра», `p9` FAILED «удалён не ровно префикс»;
`p2`/`p3` ok (жертва — младший, обрыва не требуется — ожидаемо). Возврат — `p8`/`p9` ok.

## Условие APPROVE

Б-1 и Б-2 закрыты (решением architect'а: правка спеки и/или RED + impl); Н-1 — решение
architect'а названо; затем задачи 3/5, tester, полный PR-гейт.

## Done Block

```
$ git fetch origin && git rev-parse origin/feat/M-92-manifest-verified-prune
b318f50470640a1adba821519eb5fc0c7ada0e11
$ git merge-base origin/main HEAD
6ef8c672f3895e85ed73628c81ab3d3089638d69
$ git merge-base --is-ancestor 6003fe3 origin/feat/M-92-manifest-verified-prune; echo anc=$?
anc=0
$ cargo fmt --all -- --check; echo fmt_exit=$?
fmt_exit=0
$ cargo clippy --workspace --all-targets -- -D warnings; echo clippy_exit=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.56s
clippy_exit=0
$ cargo test -p journal --test red_m92_manifest_prune
test c0_compose_topology_is_the_prune_topology ... FAILED
test c5_no_audit_no_delete ... FAILED
test c4_audit_trail_survives_next_run ... FAILED
test c1_cron_apply_verifies_against_remote_copy ... FAILED
test result: FAILED. 13 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
m92_exit=101                                  # p1-p9, b1-b2, c2-c3 ok; c0/c1/c4/c5 — задача 3
$ cargo test -p journal --test red_retention --test red_retention_checkpoint_coverage \
    --test red_retention_compacted --test red_retention_operator
test result: ok. 3 passed; 0 failed
test result: ok. 6 passed; 0 failed
test result: ok. 9 passed; 0 failed
test result: ok. 7 passed; 0 failed
ret_exit=0                                    # 25/25
$ bash scripts/verify_M-92.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)" | grep -vE "^PASS  ci-parity"
FAIL  task1-3: red_m92_manifest_prune (p1-p9, b1-b2, c0-c5) (exit=101)
PASS  task1: red_retention + red_retention_checkpoint_coverage + red_retention_compacted + red_retention_operator
FAIL  task3: verify_M-48.sh (exit=1)          # гоняет cargo test -p journal → те же c0/c1/c4/c5
FAIL  task5: в docs/fa/journal.md нет определения JR-I-13 …
PASS  ci-map: проба карты CI-паритета
FAIL  ci-parity: cargo test --all (exit=101)  # единственный FAILED-блок лога — red_m92 13/4
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
VERDICT: FAIL (провалов: 4)
verify_exit=1
$ grep "test result: FAILED" verify.log | sort | uniq -c
      1 test result: FAILED. 13 passed; 4 failed; …   # регрессий вне red_m92 нет

# проба Б-1 (временный тест, удалён) — вывод в §Б-1; exit=0
# мутация обрыва (segments.rs:3246 → no-op)
test p8_oldest_mismatch_prunes_nothing ... FAILED
test p9_middle_mismatch_prunes_exactly_the_older_prefix ... FAILED
test result: FAILED. 2 passed; 2 failed
$ git checkout -- crates/journal/src/segments.rs && git status --porcelain
(пусто)
test result: ok. 2 passed; 0 failed           # p8/p9 после возврата
```

=== HANDOFF: REVIEWER → ARCHITECT ===

## §A — Метаданные
- Дата (UTC): 2026-10-03
- Milestone: M-92-manifest-verified-prune (задача 1, `I-2bis`)
- Статус: BLOCKED — REJECT (CHANGES REQUESTED)
- HEAD: b318f50 — docs(M-92): спека §4 п.4 … [architect] (судимая вершина; этот файл — следующий коммит)

## §B — Что я сделал
- Прогнал fmt/clippy/red_m92/red_retention*/verify_M-92 на вершине; результаты совпали с tester'ом.
- Свой мутант обрыва — убит `p8`/`p9`.
- Проба: повреждённый сегмент в середине каталога выпадает из плана ⇒ уборщик оставляет дыру (Б-1).
- Чтением: типовой барьер `ColdCopyProof` на манифестном пути снят вопреки §4 (Б-2).

## §C — Артефакты / результаты
- `research/reviews/R-217-M-92-task1-prefix-prune.md` (этот файл).
- Done Block — выше.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Startup-протокол: CLAUDE.md · .claude/rules/{gates,testing,scope-guard,branch-hygiene,commit-discipline,handoff-block}.md · .claude/agents/architect.md · docs/fa/journal.md (JR-I-2, JR-I-11).
  Предмет: origin/feat/M-92-manifest-verified-prune (вершину возьми сам; на момент вердикта b318f50).
  Разбери research/reviews/R-217-M-92-task1-prefix-prune.md (REJECT):
  Б-1 — обрыв по ПЛАНУ не гарантирует сплошность КАТАЛОГА: сегмент, не классифицированный retention_plan (порча заголовка), выпадает из плана, соседи удаляются — проба дала каталог [2,4,5]. Спроектируй защиту (обрыв по каталогу/отказ) + RED с фикстурой «план не префикс каталога».
  Б-2 — спека §4 требует ColdCopyProof на манифестном пути; код удаляет fs::remove_file (segments.rs:3256). Реши: код к спеке или спеку к коду.
  Н-1 — DryRun не хеширует, dry-run-отчёт расходится с apply (важно для задачи 4(а)); Н-2 offloaded на пути без копии; Н-3 двойной хеш.
  Затем — мандат engine-dev на правки задачи 1 (команды фильтра — через `-- p8 p9`), задача 3 идёт параллельно по A-045.
  ```
- Push-статус: ✅ вердикт закоммичен и запушен на `feat/M-92-manifest-verified-prune`; `main` не трогался, PR не открывался (milestone не готов, CI красен по построению).
- Кэш: ✅ `rm -rf /tmp/hft-reviewer-m92/target` после push.

## §E — Риски / открытые вопросы
- Срок: диск прода ≈ 3 недели от 01.10 (`A-045` N-2); Б-1/Б-2 — правки узкие, критический путь удлиняют на один круг.
- Н-4: карточка TD «читатель обязан отказать на разрыве seq между сегментами» — reviewer заводит на close-out M-92.
- Включение `apply` — только architect по `П-031` после задачи 4.

=== END HANDOFF ===
