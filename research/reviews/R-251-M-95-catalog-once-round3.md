<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: 2b73ec4cb0c7726e473f18a55afda0f6d425ce6e
verdict: APPROVE
-->

# R-251 — M-95 «одна подписка обходит каталог один раз»: PR-гейт, круг 3 — APPROVE

**Роль:** reviewer. **Предмет — ветка** `origin/feat/M-95-catalog-once`.
**Вершина взята командой:** `git fetch origin && git rev-parse origin/feat/M-95-catalog-once` →
`2b73ec4cb0c7726e473f18a55afda0f6d425ce6e` — совпадает со справкой tester'а (`2b73ec4c`);
коммит движковца `b91af0d8` — её предок.
**База:** `origin/main` = `ad20a7e7…` = `merge-base` ветки; `HEAD..origin/main` пуст — ветка свежая,
merge-preview совпадает с деревом ветки.
**Судился диапазон** `c33a814b..2b73ec4c` (исполнение `R-250`) поверх уже просмотренного в
`R-249`/`R-250`.

## Вердикт: APPROVE

Единственный блокер `R-250` (Б-1: у ветки «`is_fresh` → `Ok(false)`, затем `refresh` → `Err`»
не было оракула) закрыт. Оракулы `f5` (v1) и `f6` (legacy) исполняют именно эту ветку — это
доказывает setup-страж, — а мутанты MV2/ML2, оставлявшие `R-250` зелёным, теперь их роняют.
Я прогнал мутантов сам, а не по отчёту. Гейт приёмки у меня зелёный, `exit=0`.

## Б-1 `R-250` — закрыт

| требование `R-250` | чем исполнено | проверка |
|---|---|---|
| 1. оракул ветки «`refresh` → `Err`» на v1 и legacy; MV2/ML2 обязаны его ронять; setup-страж доказывает ветку | `511937e0` [architect]: `act_refresh_fails` (`red_m95_provenance_fresh.rs:641-678`) удаляет три ранних `.zst` (дифф > 2), каталог оставляет читаемым, один оставшийся `.zst` закрывает `000`. Страж строит СВОЙ `journal::SegmentCatalog` до изменений и требует `is_fresh == Ok(false)` и `refresh.is_err()`, иначе `SETUP НЕ СОСТОЯЛСЯ`. Рукав `Err` у `is_fresh` (это мир `f3`/`f4`) тем самым исключён | **MV2** (v1, `:1611` `Err(_) => None` → провенанс по каталогу) — `f5` **FAILED**, 6 passed; **ML2** (legacy, `:2451`) — `f6` **FAILED**, 6 passed; честный код — 7/7 (гейт). Done Block |
| 2. спека §3/§8 — к факту покрытия | `d6625520` [architect]: §3 называет `f3`/`f4` для `is_fresh → Err` и `f5`/`f6` для `refresh → Err`; §8 — строки `f5`/`f6` и абзац «`f3`/`f4` ветку `refresh → Err` НЕ исполняют» | прочитано на вершине |
| 3. комментарии кода — к факту | `b91af0d8` [engine-dev], только комментарии (+12/−4 в `crates/gateway-serve/src/lib.rs`, обе ветки) | диф прочитан; код не менялся |

Критик (`C-296` NOTE) независимо получил те же исходы MV2/ML2.

## Что проверено и в порядке

- **Scope.** Весь диф ветки против `origin/main` — только `crates/gateway/src/lib.rs`,
  `crates/gateway-serve/src/lib.rs` (engine-dev, §10 Allowed), RED-файлы M-95 и
  `red_segment_meta_bound.rs`, `verify_M-95.sh`, проба CI-карты, спека (architect), вердикты
  `C-284…C-296`, `A-050`, `R-249`, `R-250`. Фильтр по разрешённым путям — 0 лишних файлов.
  `crates/journal/**`, `crates/contracts/**`, `risk`/`killswitch`/`oms`/`venue-*` — не тронуты.
- **RED-first / sacred.** Каждый коммит ветки в `*/tests/*` и `scripts/*` — `[architect]`, плюс
  один merge `main` (`35e1210b`). Тесты dev'ом не правились.
- **Block-C.** `crates/contracts/**` не тронут. **Риск-блок** — не применим, risk-critic не нужен.
- **Атомарность.** Задача 6 — один коммит, называет `R-250`. Пробел `R-249` Н-1 (задача 2 без
  собственного коммита) остаётся в истории; она не переписывается, потому что на коммиты
  ссылаются вердикты. Исправления после `R-249` атомарны.
- **Защищённые артефакты.** `check_protected_artifacts.sh origin/main HEAD` → `OK`.
- **Граница наблюдения M-62** (`C-284` B1) не нарушена — правок в `crates/gateway/src/lib.rs`
  в этом круге нет.
- **Механизм на пути.** Код исполняется прод-процессом `gateway-serve` на обоих путях выдачи. Его
  исполняют прод-бинарём `k1…k4` и `f0…f6`, точка входа подключена. Built-not-wired нет.

## Замечания (не блокируют)

- **Н-1 — неточность в новом комментарии.** `crates/gateway-serve/src/lib.rs:1592` и `:2434`:
  «затем ретеншен закрывает один оставшийся `.zst` (`000`)». На самом деле `chmod 000` делает
  фикстура оракула. Ретеншен удаляет файлы, права он не меняет. Механизм указан верно (отказ
  `refresh` на классификации), неверно названо, кто его вызывает. Карточку не завожу: это одно
  слово в комментарии, на поведение оно не влияет. Править при ближайшем касании.
- **Н-2 — колонка Status спеки отстаёт.** `milestones/M-95-catalog-once.md` §11: задачи 1, 2, 3, 5
  стоят `⏳ OPEN`, хотя исполнены (коммиты `a3d77986`, `2cbb3882`, `dcfa1465…c33a814b`).
  Исправляется при переезде спеки в архив (зона architect'а).
- **Н-3 — ошибка в отчёте tester'а, на вердикт не влияет.** В разделе FA Done Block tester'а
  сказано, что «предмет трогает `crates/journal/**`», и даны `FA-WAIVER` на `gateway`/`gateway-serve`.
  Оба утверждения неверны. `crates/journal` диф не трогает: `red_stitch_monotonic` только
  исполняется гейтом. А `check_review_fa.sh:190-196` относит `gateway` и `gateway-serve` к
  `docs/fa/viz-backend.md`, поэтому waiver там не нужен.

## Предъявление FA

`crates/gateway` и `crates/gateway-serve` → `docs/fa/viz-backend.md` (`check_review_fa.sh:190-196`):
**`VB-I-11`** (стр. 294, провенанс истории: `history_start_seq` — «seq первого РЕАЛЬНО свёрнутого
события — НЕ `header.first_seq`», `history_truncated` не выдаёт усечённое за полное) — это предмет
`f0…f6`, `c0`/`c1` и всей нити Б-1. **`VB-I-6`** (стр. 289) упоминает `GS-I-4`:
`Snapshot` уходит на провод целиком, значит ложный `history_truncated` дошёл бы до клиента.
Отдельной FA у `gateway-serve` нет — это долг `M-81`, он уже назван в спеке §12.
Справочно: `crates/journal` не тронут; `JR-I-11` (`docs/fa/journal.md:133`) охраняет
`red_stitch_monotonic`, в гейте он зелёный.

## Обязательства close-out (исполняет reviewer после merge)

1. **Задача 4** — §8-замер на проде: `strace` одной подписки, как `R-211` §4.2. Ожидается
   1 открытие на каждый `.zst` и названное число байт подписки.
2. **Карточка `TD`** на провенанс ветки переподписки с тем же `id` (`VB-I-11`; спека §5,
   `A-050` NOTE 1, `C-286`, `R-249`, `R-250`).
3. **`TD-229`** — привести к факту после замера (×3 → ×1; линейность по истории остаётся, её
   снимают S2/формат).
4. `PROJECT-STATE.md` — раздел M-95; `docs/ROADMAP.md` — колонка «Состояние» строки `TD-229`.

## Ярус C — что искал грепом

`TECH-DEBT.md` (`origin/main`): `TD-229` (`:214`, `:4962`), `TD-250` (`:5403` — флак
`red_m89_warm_resume_seek` `w1`; может покрасить CI слияния, к предмету не относится).
`PROJECT-STATE.md`: `M-95` — записей нет; `MS-STATE` — образец раздела `M-94` (`:3028-3030`).
`docs/ROADMAP.md`: `TD-229` — `:115`.

## Done Block

```
$ pwd; git fetch origin && git rev-parse origin/feat/M-95-catalog-once
/tmp/hft-reviewer-m95
2b73ec4cb0c7726e473f18a55afda0f6d425ce6e
$ git merge-base HEAD origin/main; git log --oneline -1 origin/main
ad20a7e7b844bf3882d846ebb66d3b049753c74b
ad20a7e7 Merge pull request #332 from a3ka/docs/r248-closeout

$ git diff --name-only origin/main...HEAD | grep -vE '^(crates/gateway(-serve)?/|milestones/M-95|research/(critiques|reviews|arbitration)/|scripts/(verify_M-95|tests/red_verify_M-95))'
(пусто)
$ git log --format='%h %s' origin/main..HEAD -- '*/tests/*' 'scripts/*' | grep -v '\[architect\]'
35e1210b Merge remote-tracking branch 'origin/main' into feat/M-95-catalog-once
$ bash scripts/check_protected_artifacts.sh origin/main HEAD 2>&1 | tail -1
OK: защищённые артефакты целы на HEAD (ad20a7e..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)

$ TMPDIR=/tmp bash scripts/verify_M-95.sh > log 2>&1; echo exit=$?      # агрегат: PASS-строк=65
$ grep -E '^(FAIL|SKIP|VERDICT|exit=)' log
SKIP  ci-parity: [188b5f08be3448fb] «cargo install cargo-audit --locked» — установка инструмента (cargo audit исполняется ниже)
SKIP  ci-parity: [0ada681c1a994d65] «pip install --quiet jsonschema» — установка инструмента
SKIP  ci-parity: [ddd7a1d0ce254659] шаг «база события» (12 строк, пишет sha в GITHUB_OUTPUT); локальный эквивалент — merge-base origin/main HEAD выше
SKIP  ci-parity: [48fbd5e3f735e6ad] «git fetch … refs/salvage/*» — плумбинг CI: спас-рефы в свежий клон; локальный клон их несёт
SKIP  ci-parity: [ba2ff42976ef05e2] «python3 -m pip install --quiet pyyaml» — установка инструмента
SKIP  ci-parity: [eb31b754c4a50419] агрегат «All checks passed» (условие по needs); его правильность исполняет red_ci_aggregate.sh (EXEC)
SKIP  task4: §8-замер на проде — strace одной подписки (как R-211 §4.2): открытий .zst ×1, байт подписки — снимает reviewer
VERDICT: PASS
exit=0
$ grep -E '^PASS  (task|I-4|ci-parity: (cargo|учтено))' log
PASS  task1-3: red_m95_catalog_once (k1…k4)
PASS  task1,3,5 / I-5: red_m95_provenance_fresh (f0…f6, --features testing — CI их не гоняет)
PASS  task5 / Н-4: red_m95_cold_resume_provenance (c0 c1 — холодный resume, VB-I-11)
PASS  I-4: red_m87_history_provenance_failclosed
PASS  I-4: red_segment_meta_bound (SM-*)
PASS  I-4: red_checkpoint_bin_prod_argv
PASS  I-4: red_checkpoint_prefix_pruned
PASS  I-4: journal red_stitch_monotonic (JR-I-11)
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
PASS  ci-parity: cargo test --all
PASS  ci-parity: cargo audit
PASS  ci-parity: учтено шагов 61 из 61 (исполнено 55, исключено по карте 6)

# мутационный контроль, то же дерево, правки откачены (cp оригинала), git status --porcelain | wc -l → 0
### MV2 — v1 :1611 `Err(_) => None` у refresh → провенанс по каталогу
f5_v1_refresh_failure_after_stale_is_fail_closed --- FAILED
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.25s
### ML2 — legacy :2451 то же
f6_legacy_refresh_failure_after_stale_is_fail_closed --- FAILED
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.29s

$ git fetch origin && git rev-parse origin/feat/M-95-catalog-once   # перед записью
2b73ec4cb0c7726e473f18a55afda0f6d425ce6e   # ветка не ушла
```
