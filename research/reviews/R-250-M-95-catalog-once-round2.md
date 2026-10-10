<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: c33a814b2ec4d81745b266a1a2e5841775a2d818
verdict: REJECT
-->

# R-250 — M-95 «одна подписка обходит каталог один раз»: PR-гейт, круг 2 — REJECT (узкий)

**Роль:** reviewer. **Предмет — ветка** `origin/feat/M-95-catalog-once`.
**Вершина взята командой:** `git fetch origin && git rev-parse origin/feat/M-95-catalog-once` →
`c33a814b2ec4d81745b266a1a2e5841775a2d818` — совпадает со справкой мандата tester'а (`c33a814b`).
**База:** `origin/main` = `ad20a7e7…` = `merge-base` ветки; `HEAD..origin/main` пуст.
**Судился диапазон** `aa73cbf9..c33a814b` (исполнение `R-249`) поверх уже просмотренного в `R-249`.

## Вердикт: REJECT — один пункт, зона architect'а

Код исправлен правильно по всем четырём пунктам `R-249` (Б-1, Н-2, Н-3, Н-4); гейт приёмки
зелёный у tester'а и у меня. Не выполнено одно условие APPROVED, названное в `R-249` Б-1
дословно: «мутант “вернуть `let _ =`” обязан его ронять». Мутант на `let _ = cat.refresh(..)` —
то самое место, которое `R-249` цитировал, — **оракулы `f3`/`f4` не роняет ни на v1, ни на
legacy.** Они исполняют только ветку «`is_fresh` вернул `Err`», ветку «`is_fresh` = `false`,
затем `refresh` вернул `Err`» не исполняет ни один тест. При этом спека (§3, §8 строка `f3`) и
комментарии нового кода утверждают, что `f3`/`f4` её покрывают.

## Б-1 (блокер) — ветка «`refresh` упал» без оракула, хотя покрытие заявлено

**Где.** `crates/gateway-serve/src/lib.rs:1605` (v1) и `:2439` (legacy) — рукав
`Ok(false) => match cat.refresh(..) { …, Err(_) => None }`. Сам код верен: на `Err` выходит
`None` ⇒ `(frozen_start_seq, true)`.

**Почему `f3`/`f4` эту ветку не трогают.** Фикстура удаляет самый РАННИЙ `.zst` и закрывает
каталог `chmod 0300`. `SegmentCatalog::is_fresh` (`crates/journal/src/segments.rs:241`) сперва
делает `stat` хвоста (он жив — `Ok`), затем `scan_dir_layout` → `read_dir` → `Err`. Значит,
исполняется рукав `Err(_)` у `is_fresh` (`:1607`/`:2441`), до `refresh` выполнение не доходит.

**Ветка достижима на проде.** `is_fresh` отдаёт `Ok(false)`, когда исчез файл хвоста (ротация)
или дифф каталога больше двух файлов (`segments.rs:246-286`); тогда `refresh` (`:552`) делает
полный обход с классификацией каждого файла — и может упасть, если ретеншен или компакция
удалили файл посреди обхода. Это и есть сценарий, на котором стоял `R-249` Б-1 («сбой обхода в
момент чистки — вероятнейший вид отказа»).

**Мутационный контроль (исполнен в отдельном дереве `/tmp/hft-reviewer-m95-mut`, в ветку не
коммитился; сырой вывод — Done Block):**

| мутант | место | `f0` `f1` `f2` `f3` `f4` | вывод |
|---|---|---|---|
| MV1 — v1, `Err` у `is_fresh` проглочен | `:1607` | `f3` **FAILED**, остальные ok | ветка защищена |
| ML1 — legacy, `Err` у `is_fresh` проглочен | `:2441` | `f4` **FAILED**, остальные ok | ветка защищена |
| **MV2 — v1, `Err` у `refresh` проглочен** (= `let _ = cat.refresh`) | `:1605` | **5 passed** | **ветка не защищена** |
| **ML2 — legacy, `Err` у `refresh` проглочен** | `:2439` | **5 passed** | **ветка не защищена** |
| MC — откат `5b3dbf84` (Н-4) | `crates/gateway/src/lib.rs` | `c1` **FAILED**, `c0` ok | Н-4 защищён |

**Ложные утверждения о покрытии (класс `TD-138`: документ ссылается на проверку, которой нет):**
- `milestones/M-95-catalog-once.md` §3 — «Если `is_fresh` или `refresh` вернули `Err` … Оракул на
  прод-пути выдачи — `f3` (v1), `f4` (legacy)»; §8 строка `f3` — «⇒ `is_fresh`/`refresh` → `Err`»;
- `crates/gateway-serve/src/lib.rs:1586-1587` и аналог на legacy — «Мутант “вернуть `let _ =`”
  роняет оракулы `f3`/`f4`».

**Чего требует закрытие (описание, не проект фикса — `gates.md` §4).**
1. Оракул на прод-пути выдачи, исполняющий ветку «`is_fresh` = `false`, затем `refresh` →
   `Err`» на v1 и на legacy; мутанты MV2 и ML2 обязаны его ронять, setup-страж обязан
   доказывать, что исполнилась именно эта ветка, а не рукав `Err` у `is_fresh`. Как вызвать
   отказ `refresh` после успешного `is_fresh` — решает architect.
2. Спека §3/§8 приведена к тому, что оракулы действительно покрывают.
3. Комментарии `crates/gateway-serve/src/lib.rs` в двух местах — к факту (зона engine-dev, одна
   строка в каждом месте); мутационные утверждения в коде — только с прогоном.

Код Б-1 переделывать не нужно. Задача — защитить уже правильный код.

## Что исполнено по `R-249` и в порядке

| пункт `R-249` | коммит | проверка |
|---|---|---|
| Б-1 fail-closed отказа свежести | `dcfa1465` | обе ветки `Err` дают `None` ⇒ `(frozen_start_seq, true)`; ветка `is_fresh`-`Err` защищена (MV1/ML1); ветка `refresh`-`Err` — см. Б-1 выше |
| Н-4 источник провенанса холодного пути | `5b3dbf84` | `(first_seq, first_seq > 0)` из `journal::stream`, побайтно как на `origin/main` (`git show origin/main:crates/gateway/src/lib.rs`, `resume`, холодная ветка); `(0, false)` на `Err` каталога снят; MC роняет `c1` |
| Н-2 мёртвый `LiveReducer::segment_catalog()` | `bb44d060` | удалён, −17 строк |
| Н-3 копия валидации слепка | `c33a814b` | одна `read_and_validate_inner` + две обёртки; различается только шаг (10) — источник родословной |
| Н-1 атомарность | — | четыре пункта — четыре коммита, каждый называет свой пункт `R-249` |

- **Scope.** `aa73cbf9..c33a814b`: dev — только `crates/gateway/src/lib.rs`,
  `crates/gateway-serve/src/lib.rs` (§10 Allowed); architect — `red_m95_provenance_fresh.rs`
  (`f3`/`f4`), `red_m95_cold_resume_provenance.rs`, спека, `verify_M-95.sh`; critic — `C-292`,
  `C-293`. `crates/journal/**`, `crates/contracts/**`, `risk`/`killswitch`/`oms`/`venue-*` — не
  тронуты (`git diff --name-only` — 0 файлов).
- **RED-first / sacred.** Все коммиты `*/tests/*`, `scripts/verify_*`, `scripts/tests/*` на
  ветке — `[architect]` либо merge `main`; dev тестов не трогал.
- **Block-C.** `crates/contracts/**` не тронут. **Риск-блок** — не применим, risk-critic не нужен.
- **Граница наблюдения M-62** (`C-284` B1) не нарушена — `segment_meta_ops` построения в `resume`.

## Предъявление FA

`crates/gateway`, `crates/gateway-serve` своей FA не имеют (долг `M-81`); опора —
`docs/fa/viz-backend.md`: **`VB-I-11`** (стр. 294: «seq первого РЕАЛЬНО свёрнутого события — НЕ
`header.first_seq`») — предмет Н-4 и Б-1. `crates/journal` не тронут; для справки — `JR-I-11`
(`docs/fa/journal.md:133`) охраняет `red_stitch_monotonic`, в гейте зелёный.

## Обязательства, переносимые на круг APPROVED

1. Карточка `TD` на провенанс ветки переподписки (`VB-I-11`; спека §5, `C-286`, `R-249`).
2. Задача 4 (§8-замер на проде, `strace` одной подписки) — после деплоя.

## Ярус C — что искал грепом

`TECH-DEBT.md`: `TD-229` (`:212`, `:4955`). `PROJECT-STATE.md`: `M-95` — нет записей; `TD-229` —
`:3323`, `:3432`. `docs/ROADMAP.md`: `TD-229` — `:115`.

## Done Block

```
$ pwd; git fetch origin && git rev-parse origin/feat/M-95-catalog-once
/tmp/hft-reviewer-m95
c33a814b2ec4d81745b266a1a2e5841775a2d818
$ git merge-base --is-ancestor c33a814b origin/feat/M-95-catalog-once; echo anc=$?
anc=0
$ git merge-base HEAD origin/main
ad20a7e7b844bf3882d846ebb66d3b049753c74b          # = origin/main

$ git diff --name-only origin/main...HEAD -- crates/journal crates/contracts crates/risk crates/killswitch crates/oms 'crates/venue-*' | wc -l
0

$ TMPDIR=/tmp bash scripts/verify_M-95.sh 2>&1 | grep -E "^(FAIL|SKIP|VERDICT)"; echo exit=$?    # PASS-строк=65
SKIP  ci-parity: [188b5f08be3448fb] «cargo install cargo-audit --locked» — установка инструмента (cargo audit исполняется ниже)
SKIP  ci-parity: [0ada681c1a994d65] «pip install --quiet jsonschema» — установка инструмента
SKIP  ci-parity: [ddd7a1d0ce254659] шаг «база события» (12 строк, пишет sha в GITHUB_OUTPUT); локальный эквивалент — merge-base origin/main HEAD выше
SKIP  ci-parity: [48fbd5e3f735e6ad] «git fetch … refs/salvage/*» — плумбинг CI: спас-рефы в свежий клон; локальный клон их несёт
SKIP  ci-parity: [ba2ff42976ef05e2] «python3 -m pip install --quiet pyyaml» — установка инструмента
SKIP  ci-parity: [eb31b754c4a50419] агрегат «All checks passed» (условие по needs); его правильность исполняет red_ci_aggregate.sh (EXEC)
SKIP  task4: §8-замер на проде — strace одной подписки (как R-211 §4.2): открытий .zst ×1, байт подписки — снимает reviewer
VERDICT: PASS
exit=0

# мутационный контроль, дерево /tmp/hft-reviewer-m95-mut (c33a814b), правки откачены, git status --porcelain | wc -l → 0
$ cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh   # BASE
test result: ok. 5 passed; 0 failed
### MV1 :1607 is_fresh-Err проглочен (v1)   → test f3_v1_catalog_refresh_failure_is_fail_closed ... FAILED   (4 passed; 1 failed)
### ML1 :2441 is_fresh-Err проглочен (legacy) → test f4_legacy_catalog_refresh_failure_is_fail_closed ... FAILED (4 passed; 1 failed)
### MV2 :1605 refresh-Err проглочен (v1)    → test result: ok. 5 passed; 0 failed      <- Б-1
### ML2 :2439 refresh-Err проглочен (legacy) → test result: ok. 5 passed; 0 failed      <- Б-1
$ cargo test -p gateway --test red_m95_cold_resume_provenance   # BASE: ok. 2 passed
### MC  git revert --no-commit 5b3dbf84     → test c1_cold_resume_legacy_pruned_prefix_declares_real_first_seq ... FAILED (1 passed; 1 failed)

$ git fetch origin && git rev-parse origin/feat/M-95-catalog-once   # перед записью
c33a814b2ec4d81745b266a1a2e5841775a2d818   # ветка не ушла
```

Задача 4 (§8-замер на проде) не исполнялась: снимается после деплоя, предмет не одобрен.
