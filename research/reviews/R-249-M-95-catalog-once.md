<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: aa73cbf9e9831ea1636a9d945ccb99e8f2200434
verdict: REJECT
-->

# R-249 — M-95 «одна подписка обходит каталог один раз»: PR-гейт, круг 1 — REJECT

**Роль:** reviewer. **Предмет — ветка** `origin/feat/M-95-catalog-once`.
**Вершина взята командой:** `git fetch origin && git rev-parse origin/feat/M-95-catalog-once` →
`aa73cbf9e9831ea1636a9d945ccb99e8f2200434` (совпадает со справкой мандата tester'а `aa73cbf9`).
**База:** `origin/main` = `ad20a7e7…` = `merge-base` ветки — ветка свежая, `HEAD..origin/main` пуст.
**Повторный fetch перед записью:** см. Done Block, строка «после записи».

## Вердикт: REJECT

Работа по существу хорошая: обходов каталога действительно стало по одному на подписку (k1…k4
зелёные), точка остановки стоит там, где её закрепил `A-050`, тестер прогнал гейт целиком. Но
вместе с обходом с пути выдачи снята **обвязка fail-closed провенанса истории (`VB-I-11`, M-87
задача 20)**, а оракул, который её сторожит, остался зелёным, потому что проверяет функцию, которую
путь больше не вызывает. Это блокер по §3 и §6 самой спеки.

## Б-1 (БЛОКЕР) — сбой обновления каталога глотается, снимок обещает «история полная»

**Где.** `crates/gateway-serve/src/lib.rs:1579-1581` (v1, новая подписка) и `:2393-2395` (legacy):

```rust
let (is_fresh, _ops) = cat.is_fresh(&path_for_history).unwrap_or_default();
if !is_fresh {
    let _ = cat.refresh(&path_for_history);
}
let (start, truncated) = current_history_provenance_with_catalog(&cat, &filter_for_history);
```

`SegmentCatalog::refresh` (`crates/journal/src/segments.rs:552-561`) при `Err` выходит по `?` ДО
присваивания полей — каталог остаётся прежним, то есть устаревшим. Ошибка отбрасывается `let _ =`,
и провенанс считается по каталогу, свежесть которого проверить не удалось.

**Что было до M-95.** Путь звал `history_provenance_for_serve`, который на ЛЮБОЙ `Err` отдаёт
`(frozen_start_seq, true)` — «не знаем, не обещаем». Это и есть контракт M-87 задачи 20, оракул
`crates/gateway/tests/red_m87_history_provenance_failclosed.rs` `h3`/`h4`.

**Почему оракул молчит.** `h1…h4` зовут `gateway::checkpoint::history_provenance_for_serve`
напрямую. После M-95 эту функцию не вызывает ни одна строка выдачи (`grep` по
`crates/gateway-serve/src` — только реэкспорт в `_gw`). Оракул зелёный, механизм с пути снят —
класс «оракул не меряет то, что обещает» (`testing.md`). Спека прямо назвала этот оракул сторожем
`I-4` и требовала «семантика отказа сохраняется побайтно» (§3) и «запрещено менять семантику
`history_provenance_for_serve` на `Err`… считать провенанс по каталогу без проверки свежести» (§6).
Проверка, чей результат отброшен, проверкой свежести не является.

**Воспроизведение (исполнено, файл в дерево НЕ коммитился).** Копия `red_m95_provenance_fresh.rs`
`f1` с одной добавкой в точке `m95-catalog:s1`: после удаления раннего `.zst` и свидетеля дописи —
`chmod 0300` каталогу журнала (stat работает, `read_dir` падает; setup-guard
`assert!(read_dir(..).is_err())`). Это ровно то окно, которое оракул `f1` и строит: ретеншен
удалил префикс между каталогом и провенансом; дополнительно отказало чтение каталога.

```
$ cargo test -p gateway-serve --features testing --test zz_reviewer_repro_r249 f1_ -- --nocapture
REPRO body history_truncated=Some(Bool(false)) history_start_seq=Some(Number(0))
test f1_v1_retention_between_catalog_and_provenance_is_honest ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.80s
```

Клиент получает `history_truncated=false, history_start_seq=0` при удалённом префиксе — ровно та
ложь, ради которой заведён `VB-I-11` (`docs/fa/viz-backend.md:294`). Поле уходит на провод целиком
(`GS-I-4` — `Snapshot` сериализуется в контракт с фронтом без фильтра), то есть ложь доходит до
пользователя. legacy-путь (`:2393-2395`) устроен буквально так же.

**Почему это не «экзотика».** `refresh` обходит каталог с классификацией каждого файла; ретеншен,
удаляющий файлы ВО ВРЕМЯ обхода, — штатный конкурент этого пути (тот же сценарий, ради которого
введены `f1`/`f2`). Сбой обхода именно в момент чистки — вероятнейший вид отказа, а не редчайший.

**Чего требует закрытие (описание дефекта, не проект фикса — `gates.md` §4).** На обоих путях
сбой проверки свежести или обновления каталога обязан давать исход `(frozen_start_seq, true)`,
как давал до M-95. Оракул — RED на САМУ находку (`testing.md`: «исправление по вердикту тоже
требует оракула»), на прод-пути выдачи, а не на функции в стороне; мутант «вернуть `let _ =`»
обязан его ронять. Где этот оракул живёт и как он сделан — решает architect.

## Н-1 — задача 2 без своего коммита, задача 1 разнесена под номером задачи 3

`a3d77986` «task #1» (`crates/gateway/src/lib.rs`, +301/−20) содержит и задачу 2 (`segment_catalog:
catalog` в редукторе — первый `pump` без `SegmentCatalog::open`), не называя её. `2cbb3882`
«task #3» несёт, кроме legacy и переподписки, v1-точку `m95-catalog:<id>` и v1-провенанс — это
задача 1 по §11 спеки. Итог: у задачи 2 коммита нет, граница задач 1/3 по логу не читается.
`commit-discipline.md` §Атомарные коммиты. История не переписывается (на коммиты ветки ссылаются
вердикты `C-284…C-286`, `A-050`); требование — к коммитам исправления: каждый называет свою задачу.

## Н-2 — мёртвый публичный API

`LiveReducer::segment_catalog()` (`crates/gateway/src/lib.rs:5793`) — ноль вызывающих в
`crates/**` (выдача пользуется `take_`/`put_segment_catalog`). Тот же класс, что `R-209` п.4
(`tail_bytes_for_dir` pub при нуле вызывающих).

## Н-3 — 70 строк проверки слепка скопированы, а не разделены

`read_and_validate_with_catalog` (`:4898`) — копия шагов (1)–(9) `read_and_validate` (`:4811`);
`diff` шагов расходится только в комментариях. Для lineage общая часть вынесена
(`validate_lineage_inner`), для остальной цепочки — нет: любая будущая правка валидации слепка
обязана попасть в обе копии, и ничто этого не проверяет. Не блокер, но долг, если не закрыть.

## Н-4 — холодный путь `resume` сменил источник провенанса вне предмета

`crates/gateway/src/lib.rs:5377-5383, 5398`: на пути без слепка `history_start_seq` берётся теперь
из `header.first_seq` каталога, а не из первого свёрнутого события; при `Err` построения каталога —
`(0, false)`. Спека этого не просила (§3 говорит о проверке родословной, провенансе выдачи и первом
такте). На v1/legacy значение перезаписывается выдачей, но ветка ПЕРЕПОДПИСКИ провенанс не
пересчитывает (§5) — там новое значение уходит клиенту. `VB-I-11` в FA прямо оговаривает: «seq
первого РЕАЛЬНО свёрнутого события — НЕ `header.first_seq`». Нужно либо вернуть прежний источник,
либо обосновать смену и закрыть `(0, false)` на `Err` тем же правилом fail-closed, что и Б-1.

## Что проверено и в порядке

- **Scope.** Диф против `origin/main`: `crates/gateway/src/lib.rs`, `crates/gateway-serve/src/lib.rs`
  (engine-dev, §10 Allowed); `red_m95_*.rs`, `red_segment_meta_bound.rs`, `verify_M-95.sh`,
  `red_verify_M-95_ci_map.sh`, спека (architect); вердикты `C-284…C-286`, `A-050`.
  `crates/journal/**` НЕ тронут. Тесты dev'ом не правились: `git log origin/main..HEAD -- '*/tests/*'`
  — только коммиты `[architect]` и два merge `main`. Выход за зону — нет.
- **Block-C.** `crates/contracts/**` не тронут.
- **Риск-блок.** `risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic не требуется.
- **Защищённые артефакты.** `check_protected_artifacts.sh origin/main HEAD` → `OK`, exit=0;
  удаление `milestones/M-94-calc-profile.md` предъявлено токеном в `aa73cbf9`, содержимое живёт в
  `docs/archive/M-94-calc-profile.md` (`5dba73f`). Пара `3f2a5e12`+`aa73cbf9` — атомарна и
  объяснена.
- **Точка `m95-catalog:*`** стоит там, где её закрепил `A-050` A1: v1 — после `snapshot_checked`,
  непосредственно перед провенансом; legacy — после догона. Между точкой и провенансом других
  операций с каталогом нет.
- **Граница наблюдения M-62** (`C-284` B1): `segment_meta_ops` построения учитывается в `resume`
  (`:5335`, `:5402`), в такт не переносится.

## Предъявление FA

`crates/gateway` и `crates/gateway-serve` → `docs/fa/viz-backend.md`: живой `VB-I-11` (стр. 294)
— предмет Б-1; `GS-I-4` (упомянут в тексте `VB-I-6`) — поле `history_truncated` идёт на провод в
контракт с фронтом. `crates/journal` не тронут.

## Обязательство, перенесённое на следующий круг

`C-286`/спека §5: reviewer на PR-гейте заводит `TD` на провенанс ветки переподписки (`VB-I-11`,
снимок несёт замороженные `history_*`). Карточка заводится в круге APPROVED, вместе с close-out
(`TECH-DEBT.md` пишется в `main` через PR, а не на ветку, на которой предстоит работа); здесь
записано, чтобы не потерялось.

## Ярус C — что искал грепом

`TECH-DEBT.md`: `TD-229` (предмет; `:212`, `:4955`), `TD-250` (в `main` не найден — живёт на
харнесс-ветке, не предмет M-95). `PROJECT-STATE.md`: `M-95` — записей нет.

## Done Block

```
$ pwd; git fetch origin && git rev-parse origin/feat/M-95-catalog-once
/tmp/hft-reviewer-m95
aa73cbf9e9831ea1636a9d945ccb99e8f2200434
$ git merge-base --is-ancestor aa73cbf9 origin/feat/M-95-catalog-once; echo anc=$?
anc=0
$ git merge-base HEAD origin/main
ad20a7e7b844bf3882d846ebb66d3b049753c74b          # = origin/main, HEAD..origin/main пуст

$ bash scripts/check_protected_artifacts.sh origin/main HEAD; echo exit=$?
NOTE  milestones/M-94-calc-profile.md: ALLOW-ARTIFACT-DELETE в aa73cbf9
OK: защищённые артефакты целы на HEAD (ad20a7e..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ bash scripts/verify_M-95.sh 2>&1 | grep -E "^(PASS|FAIL|SKIP|VERDICT)"; echo exit=$?   # агрегат: PASS-строк=63, SKIP-строк=8
PASS  task1-3: red_m95_catalog_once (k1…k4)
PASS  task1,3 / I-5: red_m95_provenance_fresh (f0…f2, --features testing — CI их не гоняет)
PASS  I-4: red_m87_history_provenance_failclosed      <- зелёный, но функцию с пути выдачи сняли (Б-1)
PASS  I-4: red_segment_meta_bound (SM-*)
PASS  I-4: red_checkpoint_bin_prod_argv
PASS  I-4: red_checkpoint_prefix_pruned
PASS  I-4: journal red_stitch_monotonic (JR-I-11)
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
SKIP  ci-parity: check_review_fa — вердикта R-NNN по M-95 в диапазоне ещё нет (зеленеет на PR-гейте)
SKIP  task4: §8-замер на проде — strace одной подписки (как R-211 §4.2): открытий .zst ×1, байт подписки — снимает reviewer
(+ 6 SKIP карты исключений CI: установка инструментов / плумбинг / агрегат)
VERDICT: PASS
exit=0

$ cargo test -p gateway-serve --features testing --test zz_reviewer_repro_r249 f1_ -- --nocapture   # Б-1, локально, удалён
REPRO body history_truncated=Some(Bool(false)) history_start_seq=Some(Number(0))
test f1_v1_retention_between_catalog_and_provenance_is_honest ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.80s

$ grep -rn history_provenance_for_serve crates/gateway-serve/src | grep -v '//'
crates/gateway-serve/src/lib.rs:2881:        checkpoint::{current_history_provenance_with_catalog, history_provenance_for_serve},   # только реэкспорт

$ git log --format='%h %s' origin/main..HEAD -- '*/tests/*' 'scripts/verify_*' 'scripts/check_*'
# только коммиты [architect] (87c8b5fc 5e02ce19 e2be229f 8d91f59c f37677e4 ded3fd9e ab776631) + два merge main (0108b055, 35e1210b)

$ git fetch origin && git rev-parse origin/feat/M-95-catalog-once   # перед записью
aa73cbf9e9831ea1636a9d945ccb99e8f2200434   # ветка не ушла
```

Задача 4 (§8-замер на проде) не исполнялась: снимается после деплоя, а предмет не одобрен.
