<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: aacab22205d436e9e5d9399bc7098cd499a73e7c
audited_head: 547bf24311897155afd9b5bdb98d55ae1a721bc2
verdict: REJECT
-->

# R-171 — M-70 (включение полос глубины), PR-гейт по прогону tester'а: **REJECTED**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-09-03T23:55Z
**Предмет:** `aacab22..547bf24` на `origin/docs/M-70-rev2` — 24 коммита, 19 файлов,
+3350/−145 против merge-base.
**Мандат:** отчёт tester'а `VERDICT: INVALID` (Done Block с сырым stdout, worktree
`/tmp/hft-tester-1788474599`, HEAD `547bf24`).
**Предыдущий круг:** `R-170` — REJECT маршрута на `7a98fe8` (plan-time принесли на PR-гейт).

Прогон tester'а воспроизведён ПОЛНОСТЬЮ и сошёлся по каждому числу: `VERDICT: FAIL (4)`,
`exit=1`, те же четыре строки FAIL, те же два красных оракула. Сверх прогона сняты **три
замера, которых tester не делал**, и каждый из них ПЕРЕВОРАЧИВАЕТ маршрут починки,
предложенный в его §B. Ровно поэтому вердикт — не «повторить прогон», а REJECT с
переадресацией.

---

## Block-scope — PASS

Диф не выходит за `Allowed paths` §2 милестоуна ни одним файлом.

```
$ git diff --numstat aacab22..547bf24 | sort -k3
319	0	crates/gateway-serve/tests/red_depth_bands_delivery.rs
198	63	crates/gateway/src/lib.rs
279	0	crates/gateway/tests/red_depth_bands_cap.rs
11	2	crates/gateway/tests/red_depth_cadence.rs
270	0	crates/gateway/tests/red_depth_egress_canonical.rs
16	6	crates/gateway/tests/red_depth_from_book.rs
359	0	crates/gateway/tests/red_depth_label_dictionary.rs
276	0	crates/gateway/tests/red_depth_point_provenance.rs
77	45	crates/gateway/tests/red_depth_provenance_by_reach.rs
4	1	crates/gateway/tests/red_depth_semantics.rs
5	1	crates/gateway/tests/red_gateway_bounded.rs
14	3	crates/gateway/tests/red_gateway_export_v2.rs
550	24	milestones/M-70-depth-bands-enablement.md
103	0	research/critiques/C-193-M-70-depth-bands-rev2.md
161	0	research/critiques/C-208-M-70-depth-bands-rev2.md
109	0	research/critiques/C-209-M-70-c208-closure.md
257	0	research/reviews/R-170-M-70-plan-time-merge-refused.md
343	0	scripts/verify_M-70.sh
```

**Разделение зон соблюдено ПОКОММИТНО** — это проверено, а не предположено:

```
$ for c in 6416441 1cb9388 fe2be3c 547bf24 88e39e0; do \
    echo "--- $c"; git show --numstat --format='' $c; done
--- 6416441 [engine-dev]  39  0  crates/gateway/src/lib.rs
--- 1cb9388 [engine-dev]  67 29  crates/gateway/src/lib.rs
--- fe2be3c [engine-dev]  64 36  crates/gateway/src/lib.rs
--- 547bf24 [engine-dev]  36  6  crates/gateway/src/lib.rs
--- 88e39e0 [architect]   восемь файлов crates/gateway/tests/** + milestones/M-70-*.md
```

Все четыре коммита engine-dev'а трогают РОВНО `crates/gateway/src/lib.rs`. `*/tests/**`
тронуты только коммитом с меткой `[architect]` — **RED-first не нарушен: dev не правил
sacred-оракулы** (`scope-guard.md` §Тесты — sacred). Атомарность соблюдена: каждый коммит
называет свою задачу; задача 4 разбита на два коммита, что нормой разрешено («одна задача =
**минимум** один коммит»).

## Block-C — N/A, проверено командой, а не глазами

```
$ git diff --name-only aacab22..547bf24 -- crates/contracts docs/rfc | wc -l
0
```

T1 не тронут ⇒ contract-RFC не требуется. Форма `DepthPoint` — T-designate
(`05-contract-layer.md` §2), и это верно. **Но из «не T1» НЕ следует «аддитивно» — см. Б-1.**

## Block-risk — N/A

Диф не трогает `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**`
(проверено тем же грепом по именам файлов выше). `gateway` — read-only консюмер журнала
(`VB-I-3`), order-egress отсутствует как класс. `risk-critic` в цепочке не требуется
(`gates.md` §5).

## Block-DoneBlock — PASS по форме, ПРОВАЛ по выводам

Done Block tester'а сырой, агрегированный и воспроизводимый. Воспроизведение сошлось:

```
$ cd /tmp/hft-reviewer-m70 && bash scripts/verify_M-70.sh; echo VERIFY_EXIT=$?
...
VERDICT: FAIL (4)
VERIFY_EXIT=1
--- stderr ---
FAIL: cargo test --all --quiet
FAIL: ! grep -qE 'vec!\[row\.depth_band_provenance' crates/gateway/tests/red_depth_point_provenance.rs
FAIL: task #6 версия схемы НЕ поднята (база 9, HEAD 9): смена формы выдачи обязана объявляться бампом (VB-I-4)
FAIL: task #7 — в записи GATEWAY_BANDS нет полос: 0.015 0.03 0.05 0.08 0.15 0.3 0.6 (строка:       GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001})
```

Числа те же. **Причинные цепочки — нет.** Две из них неверны, и обе разворачивают работу не
туда; замеры — в Н-2.

---

# Находки

## Б-1 — БЛОКЕР. Форма задачи 4 ЛОМАЕТ `VB-I-4`, и бамп её НЕ ЧИНИТ (зона: architect)

`docs/fa/viz-backend.md:201` — **`VB-I-4`: «export v2 аддитивен: старые консюмеры v1 не
ломаются; форма меняется только с bump»**. Инвариант ЖИВОЙ и имеет свой оракул —
`crates/gateway/tests/red_gateway_export_v2.rs::snapshot_carries_schema_version_and_is_v1_additive`.

**Замер 1 — оракул зелен на базе, красен на вершине:**

```
$ cd /tmp/hft-reviewer-base && git checkout -q aacab22
$ cargo test -p gateway --test red_gateway_export_v2
test deep_band_carries_provenance ... ok
test snapshot_carries_schema_version_and_is_v1_additive ... ok
test result: ok. 2 passed; 0 failed

$ git checkout -q 547bf24 && cargo test -p gateway --no-fail-fast --test red_gateway_export_v2
test snapshot_carries_schema_version_and_is_v1_additive ... FAILED
  panicked at crates/gateway/tests/red_gateway_export_v2.rs:103:10:
  v1-потребитель обязан распарсить depth-вывод gateway (аддитивность нарушена):
  Error("invalid type: map, expected a tuple of size 2", line: 1, column: 46)
test result: FAILED. 1 passed; 1 failed
```

**Замер 2 — МУТАЦИЯ: бамп применён, оракул ОСТАЁТСЯ КРАСНЫМ.** Это ключевой замер вердикта,
и он опровергает и §E dev'а, и §B tester'а:

```
$ sed -i '85s/= 9;/= 10;/' crates/gateway/src/lib.rs                     # задача 6
$ sed -i 's/EXPECTED_SCHEMA_VERSION: u32 = 9;/... = 10;/' \
        crates/gateway/tests/red_gateway_schema_version.rs                # задача 6b
$ cargo test -p gateway --no-fail-fast --test red_gateway_export_v2 --test red_gateway_schema_version
test snapshot_carries_schema_version_and_is_v1_additive ... FAILED
  Error("invalid type: map, expected a tuple of size 2", line: 1, column: 46)
test result: FAILED. 1 passed; 1 failed
test result: ok. 3 passed; 0 failed        ← пины версии 6b зеленеют, export_v2 НЕТ
$ git checkout -q -- crates/gateway/src/lib.rs crates/gateway/tests/red_gateway_schema_version.rs
```

**Что это значит.** Утверждение «форма сменилась без бампа ⇒ красный» ЛОЖНО. Красное здесь
структурно и от номера версии не зависит: `DepthRowV1.series: Vec<(i64, i64)>` не распарсит
`Vec<DepthPoint>` НИ ПРИ КАКОЙ версии. Спека §2bis объявляет `series: Vec<(i64,i64)>` →
`Vec<DepthPoint>` и **`ПОЛЕ depth_band_provenance УДАЛЯЕТСЯ`** — по `05-contract-layer.md` §4
это дословно **ЛОМАЮЩЕЕ** изменение («удаление/переименование поля, смена типа»), тогда как
§2 той же спеки числит формы кокпита «T-designate, **аддитивно**». Спека противоречит
собственному основанию, и оракул это предъявил.

**Почему зона architect'а, а не dev'а.** Решений ровно два, и оба вне зоны dev'а:
(а) сделать форму аддитивной (новое поле рядом со старым `series`, старое — deprecated);
(б) объявить `VB-I-4` неприменимым к депт-строке и переписать sacred-оракул — но это правка
`docs/fa/viz-backend.md`, то есть `gates.md` §9: инвариант меняется через critic, а не через
милестоун. Reviewer ОПИСЫВАЕТ дефект и не проектирует фикс (`gates.md` §4, граница
reviewer↔architect).

## Б-2 — БЛОКЕР. `VB-I-2` (live == replay) красен: расходится НАБОР ТОЧЕК, не метки (зона: architect)

```
$ cargo test -p gateway --test red_depth_provenance_by_reach
test gw_i_4_holds_when_the_tail_frame_is_delta_only ... FAILED
  crates/gateway/tests/red_depth_provenance_by_reach.rs:810:
  GW-I-4/VB-I-2 НАРУШЕН на метке при DELTA-ONLY хвосте. side=bid.
  Реплей:            [confirmed, not-observed reach=0.000015, not-observed reach=0.005000, confirmed]
  собранное клиентом:[confirmed,                              not-observed reach=0.005000, confirmed]
test result: FAILED. 8 passed; 1 failed
```

**Замер — на merge-base оракул ЗЕЛЁН:**

```
$ cd /tmp/hft-reviewer-base && git checkout -q aacab22
$ cargo test -p gateway --test red_depth_provenance_by_reach
test gw_i_4_holds_when_the_tail_frame_is_delta_only ... ok
test result: ok. 9 passed; 0 failed
```

**Но это НЕ регресс задачи 4, и это тоже замер, а не мнение.** Расходятся не метки, а ЧИСЛО
точек: 4 против 3, причём три общие метки совпадают поэлементно и по порядку. Набор точек
задаётся КЛЮЧОМ `time_s`, а задача 4 ключи не трогала — она изменила только ЗНАЧЕНИЕ:

```
$ git diff aacab22..547bf24 -- crates/gateway/src/lib.rs | grep -E '^[-+].*values\.insert|^[-+].*values:'
-    values: BTreeMap<i64, i64>,
+    values: BTreeMap<i64, (i64, f64)>,
-            row.values.insert(time_s, sum);
+            row.values.insert(time_s, (sum, reach));
-            row.values.insert(key_time_s, sum);
+            row.values.insert(key_time_s, (sum, reach));
```

Те же два места вставки, тот же ключ. Значит расхождение НАБОРА точек между полным реплеем и
инкрементальной склейкой **предсуществует в `main`** и было НЕВИДИМО, пока оракул сравнивал
ОДНУ строковую метку на ряд. Красным его сделало усиление адаптера в `88e39e0` (architect,
задача 4b): «сравнивается ВЕСЬ РЯД меток». Усиление правильное — оно вскрыло настоящий дефект
`VB-I-2`, живущий в проде сегодня.

**Следствие, которое нельзя замолчать:** `main` зелен не потому, что путь верен, а потому что
его никто не мерил. Это кандидат в `TECH-DEBT` (`VB-I-2`, MAJOR); карточку заводит reviewer —
см. §E.

## Б-3 — БЛОКЕР. Задача 6 не исполнена (зона: engine-dev)

```
FAIL: task #6 версия схемы НЕ поднята (база 9, HEAD 9)
$ grep -n 'pub const GATEWAY_SCHEMA_VERSION' crates/gateway/src/lib.rs
85:pub const GATEWAY_SCHEMA_VERSION: u32 = 9;
```

Милестоун §3 числит задачу 6 зоной `engine-dev`, статус `⏳ OPEN`. Бамп обязателен независимо
от Б-1 (`VB-I-4` требует его при ЛЮБОЙ смене формы), но Б-1 обязан быть решён ДО него: если
architect выберет аддитивную форму, содержимое задачи 6 изменится.

## Б-4 — БЛОКЕР. Задача 7 не исполнена (зона: engine-dev)

```
FAIL: task #7 — в записи GATEWAY_BANDS нет полос: 0.015 0.03 0.05 0.08 0.15 0.3 0.6
      (строка:       GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001})
$ git diff --name-only aacab22..547bf24 -- docker-compose.yml | wc -l
0
```

`docker-compose.yml` не тронут ВООБЩЕ. Милестоун §3 числит задачу 7 зоной `engine-dev` и
ставит её ПОСЛЕДНЕЙ («после 0 и 3-6») — то есть она и не должна была быть сделана раньше Б-3.
Блокер существует, но он последний в очереди, а не первый.

## Н-1 — NOTE. Ложное красное шага `task #4b`: страж наблюдает ТЕКСТ (зона: architect)

Один из четырёх FAIL гейта — **ложный**, и это замер:

```
$ sed -n '214p' scripts/verify_M-70.sh
  chk "! grep -qE 'vec!\\[row\\.depth_band_provenance' ${T4}"
$ grep -n 'depth_band_provenance' crates/gateway/tests/red_depth_point_provenance.rs
 12://!     pub depth_band_provenance: Option<String>,  // ОДНА метка на весь ряд
 37://! названо, а не предположено: пока `depth_band_provenance` есть `Option<String>`...
263:/// `DepthRow.depth_band_provenance` есть `Option<String>` на строку (`lib.rs:320`)...
270:/// стояло `vec![row.depth_band_provenance.clone()]` — «взять больше неоткуда», потому что
```

Строка `:270` — **ДОК-КОММЕНТАРИЙ, описывающий прежнюю форму**. Сам адаптер обновлён верно:

```
$ sed -n '274,276p' crates/gateway/tests/red_depth_point_provenance.rs
fn point_provenances(row: &DepthRow) -> Vec<Option<String>> {
    row.series.iter().map(|p| p.provenance.clone()).collect()
}
```

и ИСПОЛНЕНИЕ этого же шага — `PASS: DB-I-4 против НОВОЙ формы (адаптер обновлён)
(исполнено тестов: 2)`. То есть шаг гейта противоречит собственному соседнему шагу.

Класс дефекта назван самой спекой: §3ter объявил, что ЧЕТЫРЕ стража, наблюдавшие ТЕКСТ,
сужены до предмета требования. Этот — пятый, и его пропустили. Проверка по вхождению строки
краснеет от комментария, объясняющего, почему строки больше нет; это дословно то, от чего
§3ter лечил шаги 1-4. **Из `FAIL (4)` реальны три.**

## Н-2 — NOTE. Две причинные цепочки отчёта tester'а опровергнуты замером

Форма Done Block'а безупречна, числа воспроизводятся. Опровергнуты ВЫВОДЫ:

| в отчёте | замер |
|---|---|
| **X-1:** `gw_i_4_*` «падает на **baseline** (не от твоих коммитов)», отнесён в «вне M-70» | На `aacab22` оракул **ЗЕЛЁН**: `9 passed; 0 failed`. Красным он стал на этой ветке. «Вне M-70» — неверно; это Б-2 и он блокирующий |
| **E-2:** «форма сменилась (task 4) без бампа (task 6)» ⇒ `v1_additive` красный, зона engine-dev | Мутация бампа 9→10 оставляет оракул красным с той же ошибкой. Причина не в бампе и зона не dev'а — это Б-1, зона architect'а |

Цена ошибки не теоретическая: по маршруту tester'а engine-dev сделал бы бамп, получил бы тот
же красный и вернулся бы на второй круг ни с чем. Отчёт агента — гипотеза, прогон — факт
(`gates.md` §8).

## Н-3 — NOTE. Расхождение «мои/не мои» по задачам 6-7 разрешается спекой

tester зафиксировал спор, не разрешая его, — верно по его роли. Разрешаю: милестоун
`M-70-depth-bands-enablement.md` §3 называет зоной задач 6 и 7 **`engine-dev`**, §2 отдаёт
ему `crates/gateway/src/**` и `docker-compose.yml`. Обе задачи — dev'ские (Б-3, Б-4).
Основание вердикта — committed-текст спеки, а не переписка.

---

# Что проверено сверх находок (и прошло)

```
$ EVENT_NAME=pull_request PR_BASE_SHA=aacab22... bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 4, до-нормативных приземлений: 0
$ bash scripts/check_protected_artifacts.sh   → OK: защищённые артефакты целы
$ bash scripts/check_artifact_ids.sh          → OK: второй носитель под занятым ID не введён
$ bash scripts/check_docs_freeze.sh           → exit=0 (процессный слой не тронут)
```

Паритет с CI внутри гейта зелен по обоим статическим шагам:
`PASS: cargo fmt --all -- --check` · `PASS: cargo clippy --all-targets --all-features -- -D warnings`.

Номер вердикта выдан механизмом: `bash scripts/next_artifact_id.sh R` → `R-171`
(`gates.md` §12).

**Ярус C прочитан ГРЕПОМ ПО ПРЕДМЕТУ** (`reading-map.md` §2), а не целиком:
`grep -n 'TD-159\|TD-161\|TD-158' TECH-DEBT.md` → строки 100, 101, 838, 869 (обе карточки
живы, `TD-159` MAJOR и числится блокирующей `П-014` п.4);
`grep -n 'M-70\|GATEWAY_BANDS\|GATEWAY_SCHEMA_VERSION' PROJECT-STATE.md` → строки 1508-1513
(«п.4 состав выдачи — НЕ НАЧАТ»), 2000 (прецедент бампа 8→9 на M-68).

# FA-attestation (M-66)

Диф трогает `crates/gateway/src/**` ⇒ FA — `docs/fa/viz-backend.md`, префикс `VB`.
Названы ЖИВЫЕ инварианты проверяемой ревизии, и каждый — предмет находки, а не украшение:

- **`VB-I-4`** (`docs/fa/viz-backend.md:201`) — «export v2 аддитивен; форма меняется только с
  bump». Нарушен — **Б-1**, замером доказано, что бамп его не восстанавливает.
- **`VB-I-2`** (`:199`) — «live == replay: серия бит-идентична серии из replay». Нарушен —
  **Б-2**, расходится набор точек.
- **`VB-I-5`** (`:202`) — «серия глубже 1.3 % несёт `depth_band_provenance`»; предмет задач 4/5.
- **`VB-I-10`** (`:207`) — bounded-window; задача 8, шаг гейта PASS.

`FA-WAIVER` не требуется: у обоих тронутых крейтов (`gateway`, `gateway-serve`) FA есть —
`scripts/check_review_fa.sh:190-198` отображает оба на `docs/fa/viz-backend.md`.

---

# ВЕРДИКТ: **REJECTED**

Merge отказан. `main` эту вершину не принимает: два sacred-оракула, ЗЕЛЁНЫЕ на merge-base,
на ней красны (`VB-I-4`, `VB-I-2`), и две задачи милестоуна не исполнены.

**Условие APPROVED** — все четыре блокера закрыты И `bash scripts/verify_M-70.sh` даёт
`VERDICT: PASS`, `exit=0` на чистом чекауте.

**Порядок обязателен и он не совпадает с нумерацией задач:**

1. **architect** — Б-1 (решение по форме: аддитивная либо явная правка `VB-I-4` через critic
   по `gates.md` §9), Б-2 (`VB-I-2` на дельта-хвосте), Н-1 (снять ложный страж `:214`).
   Б-1 идёт ПЕРВЫМ: от него зависит содержание задачи 6.
2. **engine-dev** — Б-3 (задача 6) и Б-4 (задача 7), после того как форма зафиксирована.

Отдельно: набор артефактов и работа dev'а по задачам 3, 4, 5 претензий не имеют — гейт по ним
зелен поимённо (`DB-I-0` 2/2, `DB-I-3` 4/4, `DB-I-4` 2/2, `DB-I-5` 4/4, `DB-I-7` 4/4,
task #8 2/2). Отвергнута ВЕРШИНА, а не проделанная работа.
