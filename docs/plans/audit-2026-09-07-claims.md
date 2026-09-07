<!-- FACTS: audited_head=91a4b84d6f8a71760767e9ca3cfa9a0d1d27fa23 collected=2026-09-07 -->
# Аудит 2026-09-07 — где несущие документы врут о коде

**Ось:** расхождения между тем, что документы УТВЕРЖДАЮТ о коде, и тем, что в коде ЕСТЬ на
`origin/main` = `91a4b84d6f8a71760767e9ca3cfa9a0d1d27fa23`. Класс дефекта — `TD-138`
(«документ обосновывает инвариант механизмом, которого на этом пути нет»). Только список;
правок нет, карточек долга нет (`TECH-DEBT.md` — зона reviewer'а).

**Вне зоны (решение founder'а 2026-09-07):** `risk`/`killswitch`/`oms`/`runner`,
`RK-I-*`/`INTG-I-*`/`KS-I-*`/`OM-I-*`/`RN-I-*`, исполнение ордеров. Нулевое покрытие этих
семейств — объявленное состояние, а не находка; здесь они не разбираются.

**Дерево аудита:** `git worktree add /tmp/hft-audit-claims -b docs/audit-claims origin/main`.
Все команды ниже выполнены в этом дереве. Внутри блоков ``` — только вывод команды; прочтение
— отдельным абзацем.

---

## 0. Что покрывает `scripts/verify_design_claims.sh` и где он слеп

Прогон на дереве аудита. Команда: `bash scripts/verify_design_claims.sh 2>&1 | grep -E "^(PASS|FAIL|INFO)" | grep -v "SKIP-"`

```
PASS  [1-ЕСТЬ] все 3 маркеров [ЕСТЬ] в таблицах статусов сопровождены существующим пруфом (17 вне таблиц — ASCII-схемы/проза, не проверялись)
PASS  [2-ПОКРЫТИЕ] §22: SM-I — заявлено=10, в оракулах=10 — подтверждено замером (loose=10)
PASS  [2-ПОКРЫТИЕ] §22: VB-I — заявлено=11, в оракулах=8 — подтверждено замером (loose=8)
PASS  [2-ПОКРЫТИЕ] §22: OPS-I — заявлено=10, в оракулах=8 — подтверждено замером (loose=8)
PASS  [2-ПОКРЫТИЕ] §22: ST-I — заявлено=10, в оракулах=8 — подтверждено замером (loose=8)
PASS  [2-ПОКРЫТИЕ] §22: CT-I — заявлено=6, в оракулах=5 — подтверждено замером (loose=5)
PASS  [2-ПОКРЫТИЕ] §22: GW-I — заявлено=0, в оракулах=13 — подтверждено замером (loose=13)
PASS  [2-ПОКРЫТИЕ] §22: MD-I — заявлено=1, в оракулах=1 — подтверждено замером (loose=1)
PASS  [2-ПОКРЫТИЕ] §22: JR-I — заявлено=12, в оракулах=7 — подтверждено замером (loose=7)
PASS  [2-ПОКРЫТИЕ] §22: RK-I — заявлено=10, в оракулах=0 — подтверждено замером (loose=3)
PASS  [2-ПОКРЫТИЕ] §22: INTG-I — заявлено=7, в оракулах=0 — подтверждено замером (loose=0)
PASS  [2-ПОКРЫТИЕ] §22: BK-I — заявлено=8, в оракулах=0 — подтверждено замером (loose=0)
PASS  [2-ПОКРЫТИЕ] §22: VN-I — заявлено=9, в оракулах=2 — подтверждено замером (loose=2)
PASS  [3-ССЫЛКИ] все 7 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 445 ссылок вида docs/*.md указывают на существующие файлы
INFO  [V-АРХИВ] архивный класс выведен из проверок ссылок как датированные снимки (решение 2026-08-22, `TD-064`): research/critiques/ research/reviews/ research/arbitration/ — ссылки внутри них не судятся, вердикт отвечает за ревизию из своей шапки `GATE-META`, а не за сегодняшнее дерево
PASS  [H-FACTS-SHA] маркеров `FACTS:` проверено 13 — все ревизии сбора существуют и входят в историю HEAD
INFO  [5-ФАЗЫ] фаза «**P0 Журнал** ✅» помечена пройденной, но строка не цитирует ни одного M-NN — не проверяется машинно
INFO  [5-ФАЗЫ] фаза «**P1 Data plane** ✅» помечена пройденной, но строка не цитирует ни одного M-NN — не проверяется машинно
INFO  [5-ФАЗЫ] фаза «**P2 Research core** ✅» помечена пройденной, но строка не цитирует ни одного M-NN — не проверяется машинно
PASS  [5-ФАЗЫ] 4 строк(и) фаз, помеченных пройденными, не противоречат цитируемым milestone'ам (грубая проверка)
PASS  [6-RFC-SHA] SHA-подобных токенов (docs/DESIGN.md + docs/rfc/**.md): всего=38 проверено=38 пропущено=0 — все 38 проверенных существуют И входят в историю HEAD
PASS  [7-RFC-PATH] путей-кандидатов (токены со слэшем в backtick'ах, docs/rfc/**.md): всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
```

Прочтение. Гейт зелёный, и всё, что он проверяет, на этой ревизии верно. Его слепые зоны — там,
где лежат находки ниже:

| что гейт НЕ проверяет | почему это важно | срез |
|---|---|---|
| Семейства инвариантов, которых **нет в строках таблицы §22** — проверка 2 идёт по строкам документа, а не по корпусу тестов | семейство, живущее только в коде, невидимо; обратный дрейф ловится лишь для `GW-I`, потому что его строку однажды вписали | §1 K-3 |
| Вторую таблицу §22 (`PL-I`/`SRC-I`/`LIC-I`) — там нет колонки «в оракулах», проверять нечего | «все PENDING» при 31 оракуле `pl_i_*` | §1 K-4 |
| Пути и имена тестов в `docs/fa/**` — проверка 7 ограничена `docs/rfc/**`, проверка 4 — только `docs/*.md`-ссылками | §T каждого FA называет оракулы, которых нет | §2 K-1 |
| Идентификаторы кода (типы, функции) в любом документе | FA описывают API, которого в крейте нет | §2 K-2 |
| Ссылки `файл:строка` — объявленный предел («не проверяются и не будут») | протухшие номера в DESIGN/FA/10 профилях агентов | §2 S-3 |
| Утверждения о **проводке** барьеров (что зовётся CI, а что нет) | построено-не-проведено | §3 |
| Утверждения о **проде** | вне охвата любого репо-гейта | §4 |
| Существование файлов вне `docs/`, названных как «источник правды» (`research/registry/signals.json`) | файла нет, каталога нет | §2 S-4 |
| Числа в `.claude/rules/**` о собственном харнессе («20 сценариев») | норма ссылается на пробу, которой уже нет в этой форме | §2 S-2 |

---

## 1. Срез 1 — DESIGN §22: «Заявлено / В оракулах» против корпуса тестов

Пересчёт уникальных идентификаторов по семействам. Команда:
`for fam in SM VB OPS ST CT GW MD JR RK INTG BK VN DET PL SRC LIC GS DQ RC AL; do ids=$(grep -rhoE "\b${fam}-I-[0-9]+[a-z]?\b" crates/*/tests/ 2>/dev/null | sort -u | tr '\n' ' '); n=$(grep -rhoE "\b${fam}-I-[0-9]+[a-z]?\b" crates/*/tests/ 2>/dev/null | sort -u | wc -l); echo "$fam-I: uniq=$n [$ids]"; done`

```
SM-I: uniq=10 [SM-I-1 SM-I-10 SM-I-2 SM-I-3 SM-I-4 SM-I-5 SM-I-6 SM-I-7 SM-I-8 SM-I-9 ]
VB-I: uniq=9 [VB-I-1 VB-I-10 VB-I-11 VB-I-2 VB-I-3 VB-I-4 VB-I-5 VB-I-6 VB-I-9b ]
OPS-I: uniq=8 [OPS-I-1 OPS-I-10 OPS-I-4 OPS-I-5 OPS-I-6 OPS-I-7 OPS-I-8 OPS-I-9 ]
ST-I: uniq=19 [ST-I-1 ST-I-2 ST-I-3 ST-I-4 ST-I-5 ST-I-6 ST-I-6a ST-I-6b ST-I-6c ST-I-7 ST-I-8 ST-I-8a ST-I-8b ST-I-8c ST-I-8d ST-I-8e ST-I-8f ST-I-8g ST-I-8h ]
CT-I: uniq=5 [CT-I-1 CT-I-3 CT-I-4 CT-I-5 CT-I-6 ]
GW-I: uniq=13 [GW-I-1 GW-I-10 GW-I-11 GW-I-12 GW-I-14 GW-I-2 GW-I-3 GW-I-4 GW-I-5 GW-I-6 GW-I-7 GW-I-8 GW-I-9 ]
MD-I: uniq=1 [MD-I-8 ]
JR-I: uniq=7 [JR-I-1 JR-I-10 JR-I-11 JR-I-12 JR-I-4 JR-I-8 JR-I-9 ]
RK-I: uniq=3 [RK-I-1 RK-I-3 RK-I-8 ]
INTG-I: uniq=0 []
BK-I: uniq=0 []
VN-I: uniq=2 [VN-I-5 VN-I-7 ]
DET-I: uniq=3 [DET-I-1 DET-I-2 DET-I-3 ]
PL-I: uniq=4 [PL-I-1 PL-I-4 PL-I-5 PL-I-7 ]
SRC-I: uniq=0 []
LIC-I: uniq=0 []
GS-I: uniq=3 [GS-I-2 GS-I-4 GS-I-5 ]
DQ-I: uniq=0 []
RC-I: uniq=11 [RC-I-1 RC-I-10 RC-I-11 RC-I-2 RC-I-3 RC-I-4 RC-I-5 RC-I-6 RC-I-7 RC-I-8 RC-I-9 ]
AL-I: uniq=5 [AL-I-1 AL-I-2 AL-I-3 AL-I-4 AL-I-5 ]
```

Прочтение. По 12 строкам таблицы §22 числа СХОДЯТСЯ с гейтом (числовые ID без суффикса;
`VB-I-9b` и `ST-I-6a..8h` — суффиксные, гейт и таблица их не считают, это не расхождение, а
соглашение). Расхождения лежат ВНЕ строк таблицы.

### K-1 (крупный) — §22 перечисляет 12 семейств, в корпусе тестов их 28; десять семейств не имеют документа-дома вовсе

Где утверждение: `docs/DESIGN.md:917-930` (таблица §22 «Сверка "заявлено в docs / упомянуто в тестах"»).
Что утверждается: таблица — карта «семейство → зона покрытия» (профиль architect'а называет её
главной опорой).

Команда: `grep -rhoE "\b[A-Z]{2,5}-I-[0-9]+" crates/*/tests/ | sed -E 's/-I-[0-9]+//' | sort -u | tr '\n' ' '; echo; grep -rhoE "\b[A-Z]{2,5}-I-[0-9]+" crates/*/tests/ | sed -E 's/-I-[0-9]+//' | sort -u | wc -l; grep -cE "^\| [A-Z]+-I \|" docs/DESIGN.md`

```
AL BL CT DET DV FB GD GR GS GW HM HW JR MD MI OF OPS PF PL RC RK SG SM ST VB VN VP VW 
28
12
```

Семейства с оракулами и БЕЗ единого упоминания в `docs/fa/*.md` и `docs/DESIGN.md`. Команда:
`for fam in DV HW HM OF GD VP VW MI BL FB; do printf "%s-I: tests=%s docs/fa+DESIGN=%s\n" $fam "$(grep -rlE "\b${fam}-I-[0-9]+" crates/*/tests/ | wc -l)" "$(grep -lE "\b${fam}-I-[0-9]+" docs/fa/*.md docs/DESIGN.md | wc -l)"; done`

```
DV-I: tests=7 docs/fa+DESIGN=0
HW-I: tests=3 docs/fa+DESIGN=0
HM-I: tests=4 docs/fa+DESIGN=0
OF-I: tests=4 docs/fa+DESIGN=0
GD-I: tests=2 docs/fa+DESIGN=0
VP-I: tests=1 docs/fa+DESIGN=0
VW-I: tests=1 docs/fa+DESIGN=0
MI-I: tests=2 docs/fa+DESIGN=0
BL-I: tests=1 docs/fa+DESIGN=0
FB-I: tests=1 docs/fa+DESIGN=0
```

Семейства С домом в `docs/fa/*.md`, но не в таблице §22. Команда:
`for fam in RC SG AL PF GS DET GR; do printf "%s-I: uniq-ids-in-tests=%s in-§22-table=%s\n" $fam "$(grep -rhoE "\b${fam}-I-[0-9]+" crates/*/tests/ | sort -u | wc -l)" "$(awk '/^## §22/,/^## §23/' docs/DESIGN.md | grep -cE "^\| ${fam}-I ")"; done`

```
RC-I: uniq-ids-in-tests=11 in-§22-table=0
SG-I: uniq-ids-in-tests=11 in-§22-table=0
AL-I: uniq-ids-in-tests=5 in-§22-table=0
PF-I: uniq-ids-in-tests=4 in-§22-table=0
GS-I: uniq-ids-in-tests=3 in-§22-table=0
DET-I: uniq-ids-in-tests=3 in-§22-table=0
GR-I: uniq-ids-in-tests=7 in-§22-table=0
```

Прочтение. Это тот самый «обратный дрейф», который §22 признаёт для одного `GW-I` — а он
системный: `DV-I` (7 файлов `research-cli`), `HW-I`/`HM-I` (heatmap, `gateway`), `OF-I`
(footprint/ohlcv), `GD-I`/`BL-I` (`book`), `MI-I`, `FB-I`, `VP-I`, `VW-I` — идентификаторы
живут только в коде, как `MD-I-8` до `M-68` («указывал в пустоту», `R-154` N-9). И семь
семейств с настоящим домом в FA (`RC-I` 11 ID, `SG-I` 11, `GR-I` 7, `AL-I` 5, `PF-I` 4, `GS-I`
3, `DET-I` 3) в карте отсутствуют. Проверка 2 гейта идёт по строкам таблицы и потому слепа к
обоим классам. Вес — **крупный**: `DESIGN.md` §22 — ярус A, по нему architect выбирает, где
искать оракул.

### K-3 (крупный) — `BK-I` «8 / 0, ЗАЯВЛЕНО БЕЗ ОРАКУЛОВ» — покрытие книги существует под недекларированными семействами

Где утверждение: `docs/DESIGN.md:928` («BK-I | book | 8 | **0** | ЗАЯВЛЕНО БЕЗ ОРАКУЛОВ»).
Что утверждается: у `book` нет оракулов.

Команда: `grep -rhoE "\bBK-I-[0-9]+" crates/book/ | sort -u | wc -l; grep -rhoE "\b(GD|BL|DET)-I-[0-9]+" crates/book/tests/ | sort -u | tr '\n' ' '; sed -n 1,4p crates/book/tests/red_gap_detection.rs | cut -c1-160`

```
2
BL-I-1 BL-I-2 BL-I-3 BL-I-4 BL-I-5 BL-I-6 DET-I-2 GD-I-1 GD-I-2 GD-I-3 GD-I-4 GD-I-5 GD-I-6 
//! RED M-30 GD-I-1..6 (sacred, architect-only) — gap-detection по update-id chaining, fail-closed.
//!
//! `OrderBook::apply_l2delta(...) -> ContinuityStatus`: дельты ЧЕЙНЯТСЯ (спот `U==prev.u+1`, фьючерс
//! `pu==prev.u`): разрыв → `Gap` + книга `stale` + дельта НЕ применена (fail-closed, как риск-слой);
```

Прочтение. Два упоминания `BK-I` — комментарии в `crates/book/src/lib.rs` (`:114`, `:318`),
в тестах ноль. Но `crates/book/tests/` несёт 13 идентификаторов трёх семейств, и `GD-I-1..6`
пиннят ровно предмет `BK-I-2` из `docs/fa/book.md:56` («seq-гэп → Stale, fail-closed»).
Строка «без оракулов» верна буквально и ложна по существу: покрытие есть, метки — чужие,
документа-дома у `GD-I`/`BL-I` нет (см. K-1). Вес — **крупный**: architect, планирующий RED для
`book`, по §22 напишет второй набор на уже покрытый инвариант.

### K-4 (крупный) — вторая таблица §22: «PL-I … все — будущие RED-оракулы, статус PENDING» при 31 оракуле в коде

Где утверждение: `docs/DESIGN.md:932-933`.
Что утверждается: инварианты `PL-I-1..9` не имеют оракулов (кроме отмеченного `PL-I-1 [ЕСТЬ]`
как расширения `DET-I-1`).

Команда: `grep -rhoE "fn pl_i[0-9_]+[a-z0-9_]*" crates/*/tests/*.rs | wc -l; grep -rhoE "fn pl_i[0-9_]+[a-z0-9_]*" crates/*/tests/*.rs | head -3; grep -rhoE "\bPL-I-[0-9]+" crates/*/tests/*.rs | sort | uniq -c | sort -rn | tr '\n' ';'`

```
31
fn pl_i_5_n1_c_absent_var_yields_signed_default
fn pl_i_5_n1a_env_value_reaches_the_enforcement_crate
fn pl_i_5_n1b_reached_value_actually_governs
     88 PL-I-5;     10 PL-I-7;      9 PL-I-4;      2 PL-I-1;
```

Прочтение. `PL-I-5` (тарифные лимиты enforce'ятся сервером; `M-69`/`M-71`) — 88 упоминаний и
именованные оракулы `pl_i_5_*`; `PL-I-4`, `PL-I-7` — тоже. Тот же класс, что `C-041` Ф1
(занижение). Вес — **крупный**: таблица подсказывает «писать RED», где RED уже есть.

### N-1 (мелкий) — `DET-I-2`/`DET-I-3` есть в оракулах, но не определены ни в одном документе `docs/`

Команда: `grep -rn "DET-I-[23]" docs/*.md docs/fa/*.md | wc -l; grep -rn "DET-I-2\|DET-I-3" crates/*/tests/*.rs | head -2 | cut -c1-120`

```
1
crates/book/tests/red_det_projection.rs:1://! RED M-51 — **DET-I-2** (sacred, architect-only): проекция, посчитанная ИНКРЕМЕНТАЛ
crates/book/tests/red_det_projection.rs:23://! **DET-I-2.** Для любого окна журнала: состояние проекции, полученно
```

Прочтение. Единственное упоминание в `docs/` — `SESSION-HANDOFF.md:403` (индекс, не
определение). `docs/fa/journal.md` §I знает только `DET-I-1`. Инвариант определён телом
теста, а не документом — та же форма, что `MD-I-8` до `M-68`.

---

## 2. Срез 2 — `docs/fa/*.md`: инварианты и механизмы против кода

### K-2 (крупный) — §T «RED-тест маппинг» реализованных крейтов называет тесты, которых нет

Где утверждение: секции `## §T` в `docs/fa/{journal,book,contracts,alpha,portfolio,strategy,venues,signals,sim,research-cli}.md`.
Что утверждается: перечисленные функции — RED-оракулы инвариантов модуля («каждый падает на
соответствующей заглушке»).

Команда (скрипт: из секции §T каждого FA берутся backtick-токены `snake_case`, для каждого —
`grep -rlE "fn\s+<имя>\b" crates/`): результат по файлам —

```
docs/fa/alpha.md §T: names=10 found=0 missing=10 :: test_combine_overflow_rejects_tick test_forecast_determinism_replay test_no_online_learning_surface test_no_weight_mutation_surface test_paper_signal_excluded_from_live_sum test_retired_never_combined test_stale_input_excluded_not_zeroed test_unknown_instrument_no_forecast test_unregistered_signal_version_rejected test_weight_table_replay_parity
docs/fa/book.md §T: names=8 found=0 missing=8 :: test_depth_monotonic_in_pct_band test_duplicate_seq_not_a_gap test_gap_detection_is_sequence_only test_gap_marks_stale_synchronously test_no_network_deps test_no_upward_write_path test_replay_deterministic_x3 test_stale_view_never_masked_as_fresh
docs/fa/contracts.md §T: names=5 found=0 missing=5 :: test_eventkind_single_definition test_fixtures_valid_invalid test_old_version_roundtrip test_schema_matches_types test_schema_version_present
docs/fa/journal.md §T: names=8 found=0 missing=8 :: test_no_f64_in_money_payload test_old_segment_roundtrip test_ord_risk_ctl_fsynced test_replay_thrice_bit_identical test_seq_monotonic_no_gaps test_single_writer_enforced test_snapshot_equals_full_replay test_write_failure_halts
docs/fa/portfolio.md §T: names=10 found=0 missing=10 :: test_correlations_disabled_single_instrument_profile test_degenerate_forecast_refuses test_forecast_only_from_alpha test_inventory_is_readonly_projection_no_shadow_state test_inventory_skew_monotonic test_limits_require_signed_paramchange test_no_oms_venue_risk_dependency test_sizing_core_no_io_no_clock_no_net test_sizing_is_deterministic test_unknown_instrument_refuses_target
docs/fa/research-cli.md §T: names=11 found=9 missing=2 :: test_capacity_decay_no_llm test_test_segment_unreachable_before_val_gate
docs/fa/signals.md §T: names=12 found=8 missing=4 :: test_determinism_replay test_no_lookahead test_retired_not_instantiated test_status_tag_propagates
docs/fa/sim.md §T: names=10 found=8 missing=2 :: test_latency_sourced_only_from_measured_table test_paper_fill_cannot_reach_live_gateway
docs/fa/strategy.md §T: names=9 found=0 missing=9 :: test_diff_batch_priority_and_rate_budget test_diff_quoting_is_deterministic test_inventory_cap_one_sided_then_stop test_no_direct_oms_venue_risk_dependency test_no_shadow_sizing_consumes_portfolio_target_only test_no_wallclock_in_domain_quoting test_quoter_state_explicit_no_global_mutable test_skew_reacts_to_inventory_direction test_stale_target_or_mark_triggers_no_quote
docs/fa/venues.md §T: names=9 found=0 missing=9 :: test_client_order_id_deterministic test_disconnect_emits_before_reconnect test_malformed_message_dropped_not_fabricated test_no_authoritative_state_owned test_no_venue_types_in_public_api test_place_rejects_raw_order test_rate_limit_explicit_reject_not_silent_drop test_reconnect_backoff_deterministic test_registry_no_venue_special_case
```

Контрольная проверка для `journal` вручную. Команда: `for t in test_replay_thrice_bit_identical test_seq_monotonic_no_gaps test_ord_risk_ctl_fsynced test_write_failure_halts; do echo "$t -> $(grep -rl "fn $t\b" crates/journal/tests/ | tr '\n' ' ')"; done`

```
test_replay_thrice_bit_identical -> 
test_seq_monotonic_no_gaps -> 
test_ord_risk_ctl_fsynced -> 
test_write_failure_halts -> 
```

Прочтение. `docs/fa/journal.md:226-231` (§T) утверждает оракулы на `JR-I-1..7` поимённо; тот же
корпус в `DESIGN.md:926` говорит «Замер: `JR-I-1,4,8,9,10,11,12`», и греп (§1) подтверждает
второе: `JR-I-2,3,5,6,7` оракулов не имеют, а `JR-I-1`/`JR-I-4` покрыты под ДРУГИМИ именами.
FA противоречит DESIGN внутри одного корпуса. Для `book`, `alpha`, `portfolio`, `strategy`,
`venues`, `contracts` — 0 из N. `research-cli`/`sim`/`signals` — частично (там §T
пересинхронизировали). Вес — **крупный**: `docs/fa/<module>.md` — ярус B, обязательное чтение
целиком ДО первого утверждения о модуле; §T — единственное место, где FA называет, ЧЕМ
инвариант защищён.

### K-5 (крупный) — FA реализованных крейтов описывают API, типы и раскладку файлов, которых в крейте нет

Где утверждение: `docs/fa/book.md:31-48` (`BookState`/`BookView`/`DepthBands`, `reducer/`,
`derived/`), `docs/fa/alpha.md:36-41` (`combine.rs`/`normalize.rs`/`state.rs`,
`WeightTable`/`NormalizationParams`), `docs/fa/strategy.md`, `docs/fa/portfolio.md`,
`docs/fa/venues.md:11` («`crates/venues/` — трейты + реестр адаптеров»), `docs/fa/contracts.md:28-29`.
Что утверждается: секции §3 «Что модуль ЕСТЬ» описывают структуру и типы модуля в настоящем времени.

Команда (скрипт: CamelCase-токены в backtick'ах FA ∩ `pub struct|enum|trait|type` собственного крейта):

```
journal.md: FA-типов=3 в_крейте=1 нет_в_крейте=2 | pub-типов крейта=20 | последняя amendment-дата: 2026-07-10
book.md: FA-типов=4 в_крейте=0 нет_в_крейте=4 | pub-типов крейта=3 | последняя amendment-дата: 2026-07-10
contracts.md: FA-типов=8 в_крейте=1 нет_в_крейте=7 | pub-типов крейта=14 | последняя amendment-дата: 2026-07-10
viz-backend.md: FA-типов=6 в_крейте=4 нет_в_крейте=2 | pub-типов крейта=29 | последняя amendment-дата: ?
ops.md: FA-типов=13 в_крейте=3 нет_в_крейте=10 | pub-типов крейта=30 | последняя amendment-дата: ?
sim.md: FA-типов=10 в_крейте=4 нет_в_крейте=6 | pub-типов крейта=18 | последняя amendment-дата: 2026-07-10
research-cli.md: FA-типов=10 в_крейте=5 нет_в_крейте=5 | pub-типов крейта=36 | последняя amendment-дата: 2026-07-10
signals.md: FA-типов=11 в_крейте=5 нет_в_крейте=6 | pub-типов крейта=13 | последняя amendment-дата: 2026-07-10
alpha.md: FA-типов=8 в_крейте=0 нет_в_крейте=8 | pub-типов крейта=7 | последняя amendment-дата: 2026-07-10
portfolio.md: FA-типов=10 в_крейте=1 нет_в_крейте=9 | pub-типов крейта=4 | последняя amendment-дата: 2026-07-10
strategy.md: FA-типов=11 в_крейте=0 нет_в_крейте=11 | pub-типов крейта=8 | последняя amendment-дата: 2026-07-10
venues.md: FA-типов=10 в_крейте=0 нет_в_крейте=10 | pub-типов крейта=11 | последняя amendment-дата: 2026-07-10
```

Контроль руками для `book` и `alpha`. Команда: `grep -rhoE "pub (struct|enum) [A-Za-z]+" crates/book/src/*.rs | sort -u | tr '\n' ' '; echo; grep -c "BookState\|BookView\|DepthBands" docs/fa/book.md; grep -rc "BookState\|BookView\|DepthBands" crates/book/src/ | grep -v ":0"; echo "(none)"; ls crates/alpha/src/ crates/portfolio/src/ crates/strategy/src/; ls -d crates/venues crates/runner`

```
pub enum ContinuityStatus pub struct Books pub struct OrderBook 
18
(none)
crates/alpha/src/:
lib.rs

crates/portfolio/src/:
lib.rs

crates/strategy/src/:
lib.rs
types.rs
ls: cannot access 'crates/venues': No such file or directory
ls: cannot access 'crates/runner': No such file or directory
```

Прочтение. Счётчик «нет_в_крейте» — сырьё, не вердикт по каждому токену: в него попадают и
сквозные типы соседей (`EventKind` — в `contracts`; `RiskApproved`/`OrderGateway` — торговый
путь, вне зоны), и не-типы (`RssAnon`, `SystemTime`). Но для `book`/`alpha`/`strategy`/`venues`
пересечение с собственным крейтом — **ноль**, а `book.md` 18 раз называет типы, которых в
`crates/book` нет ни разу; `alpha.md` описывает три файла при одном `lib.rs`; `venues.md` —
крейт `crates/venues/`, которого нет. Даты последних amendment'ов — 2026-07-10, до реализации:
FA — pre-impl спека, никогда не сверенная с кодом, при статусе «✅ APPROVED» в
`docs/fa/README.md`. Вес — **крупный** (ярус B). Это не «изложение»: сдвиг границы ролей
`book`↔`gateway` (кто считает depth-полосы, где живёт `is_stale`) по FA не восстановить.

### S-1 (средний) — `docs/fa/ops.md` называет оракул, которого нет под этим именем

Где: `docs/fa/ops.md:367`, `:380`. Что утверждается: «добавлен `runtime_persistent_volume_is_silent`»;
«RED `runtime_persistent_volume_is_silent` кормит РЕАЛЬНЫЙ прод-путь».

Команда: `grep -n "runtime_persistent_volume_is_silent" docs/fa/ops.md | cut -c1-80; grep -rn "fn runtime_persistent" crates/ops/tests/ | cut -c1-110`

```
367:   `runtime_persistent_volume_is_silent` (персистентный объ�
380:RED `runtime_persistent_volume_is_silent` кормит РЕАЛЬНЫЙ пр
crates/ops/tests/red_recon_runtime.rs:105:fn runtime_persistent_volume_deficit_is_silent() {
crates/ops/tests/red_recon_runtime.rs:130:fn runtime_persistent_volume_surplus_is_silent() {
```

Прочтение. Механизм есть, разложен на две стороны (deficit/surplus); имя в FA — от прежней
редакции. Семантика цела; средний вес потому, что `ops.md` §4.3 — действующая секция B2, а не
приписка-история.

### S-2 (средний) — `research/registry/signals.json` объявлен «источником правды», в репозитории его нет; `trials-ledger.json` — файл называется иначе

Где: `docs/DESIGN.md:209`, `docs/03-integration-contract.md:33`, `docs/fa/signals.md` (×3),
`docs/fa/research-cli.md:90/180/210`; леджер — `.claude/rules/gates.md:248`,
`.claude/rules/scope-guard.md:14`, `docs/02-quant-desk.md:33`, `docs/fa/research-cli.md:126/174`.
Что утверждается: граница B — файл `research/registry/signals.json`, «источник правды, что движок
исполняет»; `research/trials-ledger.json` — append-only ledger.

Команда: `ls research/registry; ls research/trials-ledger*; sed -n 1p crates/signals/src/registry.rs | cut -c1-100`

```
ls: cannot access 'research/registry': No such file or directory
research/trials-ledger.jsonl
//! registry — outer-слой (FA §6): загрузка research/registry/signals.json РОВНО
```

Прочтение. Каталога `research/registry/` в дереве нет вовсе; код `crates/signals/src/registry.rs`
описывает загрузку именно этого пути. Ни один из документов не говорит, что файл заводится
позже. Леджер: документы и правила называют `.json`, файл — `.jsonl`; `scope-guard.md`
защищает путь, которого нет. Вес — средний: граница B — то, о чём агентам запрещено
догадываться (`INTG-I-2`), и сегодня у неё нет носителя, а документ говорит, что есть.

### S-3 (средний) — протухшие `файл:строка` в несущих документах и в 10 профилях агентов

Предел объявлен (`файл:строка` не проверяются ничем), поэтому — список, а не претензия к гейту.
Команда: `grep -n "segments.rs:1749\|TECH-DEBT.md:112" docs/DESIGN.md | cut -c1-60; grep -n "fn decide_open_segment" crates/journal/src/segments.rs; sed -n 112p TECH-DEBT.md | cut -c1-40; grep -n "TD-031 закрыл" TECH-DEBT.md | cut -c1-60; grep -n "lib.rs:1363\|lib.rs:1211" docs/fa/viz-backend.md | cut -c1-40; grep -n "fn depth_provenance_label\|let deep_thr" crates/gateway/src/lib.rs | cut -c1-70; grep -n "docker-compose.yml:134" docs/fa/viz-backend.md | cut -c1-40; grep -n "0.001" docker-compose.yml | cut -c1-70; grep -rln "check_review_fa.sh:57" .claude/agents/ | wc -l; grep -n "echo \"SKIP" scripts/check_review_fa.sh | cut -c1-60`

```
717:  `crates/journal/src/segments.rs:1749–1758`) — ме
719:  (`crates/contracts/src/lib.rs:16–19`) и `TECH-DEBT.
3222:pub(crate) fn decide_open_segment(
| **TD-171** | запрет рантай
3557:  на close-out M-18 как C-018 rev4 merge-condition
92:  `depth_provenance_label`, `crates/g
100:  `crates/gateway/src/lib.rs:1211-12
143:    `depth_provenance_label(band_pct
1990:        let deep_thr = (mid as f64 * 0.013) as i64; // 1.3% от 
2094:    let deep_thr = (mid as f64 * 0.013) as i64;
2203:fn depth_provenance_label(band_pct_e8: i64, side: Side, reach: f6
127:    (сегодня дефолт `0.
136:      GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001}
137:      GATEWAY_HEATMAP_WINDOW: ${GATEWAY_HEATMAP_WINDOW:-0.001}
217:      - --bands=${GATEWAY_BANDS:-0.001}
9
99:    echo "SKIP (диапазон трогает ТОЛЬ�
102:    echo "SKIP (диапазон не трогает cra
```

| где | заявлено | есть |
|---|---|---|
| `docs/DESIGN.md:717` | `decide_open_segment` в `segments.rs:1749–1758` | `:3222`; на `:1749` — комментарий про `events_scanned` |
| `docs/DESIGN.md:719` | `TECH-DEBT.md:112–124` (TD-031, **OPEN**) | `:112` = `TD-171`; `TD-031` закрыт (`TECH-DEBT.md:3557`) — не только номер, но и статус |
| `docs/fa/viz-backend.md:92`, `:143` | `depth_provenance_label` — `gateway/src/lib.rs:1363` | `:2203` |
| `docs/fa/viz-backend.md:100` | `deep_thr` — `lib.rs:1211-1212` | `:1990`, `:2094` |
| `docs/fa/viz-backend.md:127` | дефолт `0.001` — `docker-compose.yml:134,203` | `:136`, `:137`, `:217` |
| `.claude/agents/*.md` (9 файлов) + `architect.md:203` | «SKIP `check_review_fa.sh:57`» | `echo "SKIP` на `:99`/`:102`; `:57` — комментарий |

Прочтение. Механизмы на месте, номера уехали. Опасен только `DESIGN.md:719`: там протух не
номер, а СТАТУС (`TD-031` назван OPEN при закрытом). `docs/04-workflow.md:99` (`check_gate_meta.sh:393`)
и `:140` (`check_context_budgets.sh:19`) — сходятся.

### S-4 (средний) — `docs/fa/README.md` индексирует 14 FA при 18 в каталоге и называет модули без крейтов

Команда: `ls docs/fa/*.md | grep -v "README\|_TEMPLATE" | wc -l; grep -n "^14 module-FA" docs/fa/README.md; grep -oE "^\| [a-z-]+ \|" docs/fa/README.md | tr -d '| ' | tr '\n' ' '`

```
18
26:14 module-FA. Sacred (4) авторил architect; остальные 10 — субагенты по шаблону+эталону,
contracts journal venues book signals alpha portfolio strategy oms risk killswitch sim runner research-cli 
```

Прочтение. Вне индекса: `ops.md`, `viz-backend.md`, `strategy-brain.md`, `ai-copilot.md` — три из
них описывают РАБОТАЮЩИЙ прод-код (`ops`, `gateway`/`gateway-serve`), а карта «готовности»
их не знает. `venues` и `runner` в карте есть, крейтов `crates/venues`/`crates/runner` нет (см.
K-5). Статус-колонка «✅ ACCEPTED/APPROVED» датирована 2026-07-10 и означает ревью ТЕКСТА, а
читается как готовность.

### S-5 (средний) — `docs/fa/contracts.md` §3 перечисляет T1-типы как существующие в `types/`

Где: `docs/fa/contracts.md:28-29`. Команда: `sed -n 28,29p docs/fa/contracts.md | cut -c1-120; grep -rhoE "pub (struct|enum) [A-Za-z]+" crates/contracts/src/*.rs | sort -u | tr '\n' ' '; echo; sed -n 153p crates/contracts/src/lib.rs`

```
- core: `types/` — канонические Rust-типы T1 (`Event`/`EventKind`, `SignalRegistryEntry`,
  `ParamChange`, `SignalSpec`, `ValidationReport`, `TrialsLedgerEntry`, `Decision`).
pub enum DataSource pub enum EventKind pub enum MdPayload pub enum ReconAction pub enum Side pub enum SysEvent pub enum Venue pub struct Event pub struct LegacyManifest pub struct LegacySegmentDecl pub struct Level pub struct MdEvent pub struct ReconAudit pub struct SegmentHeader 
    // Ord(..), Risk(..), Recon(..), Ctl(..) — добавляются в P3 via contract-RFC.
```

Прочтение. Из семи названных типов в крейте один (`EventKind`); каталога `types/` нет.
`contracts.md:112` фазирует часть («P3: ParamChange…»), но §3 написан в настоящем времени.
`ParamChange`/`Ctl` — граница C, вне зоны по существу; в зоне здесь только форма утверждения:
`crates/contracts` — sacred T1, и его FA не различает «есть» и «будет».

### N-2 (мелкий) — имена в `docs/fa/sim.md` §3 не совпадают с кодом при полном покрытии `SM-I`

`FullFill` → `FillDecision::Full{qty}`; `queue_position_ahead` → поле `ahead`; трейты
`OrderGateway`/`MarketDataFeed` — `crates/sim/src/exchange.rs:4` прямо говорит «формальный trait
появится вместе с oms (P3)». Оракулы `SM-I-1..10` — все на месте (§1), поэтому — мелкий.

### N-3 (мелкий) — `docs/fa/ops.md:280`, `:365` цитируют `crates/ops/tests/red_recon_window.rs`

Файла нет — но документ сам говорит, что он переименован в `red_recon_runtime.rs`, и тот
существует. Не ложь, а история, помеченная как история. Занесено, чтобы следующий скан не
принял за находку.

### Что в `journal.md` §I ВЕРНО (проверено, потому что это опорный пример класса)

`JR-I-11` — таблица путей и три ребра `SegmentCatalog`. Команда: `grep -n "check_monotonic_paths" crates/journal/src/lib.rs; echo "recover-count: $(grep -c recover crates/journal/tests/red_stitch_monotonic.rs)"; for n in 199 426 554; do awk -v N=$n 'NR<=N && /^\s*(pub(\([a-z]+\))? )?fn /{f=$0} NR==N{print N": "f}' crates/journal/src/segments.rs | cut -c1-80; done; grep -rn "fn mn_9_\|fn mn_10_\|fn mn_11_" crates/journal/tests/ | cut -c1-80`

```
447:    segments::check_monotonic_paths(dir, &segs, &mut ops)?;
473:    segments::check_monotonic_paths(dir, &segs, &mut ops)?;
recover-count: 18
199:     pub fn open(dir: &Path) -> io::Result<(Self, SegmentOps)> {
426:     pub fn is_fresh(&mut self, dir: &Path) -> io::Result<(bool, SegmentOps)> {
554:     pub fn refresh(&mut self, dir: &Path) -> io::Result<SegmentOps> {
crates/journal/tests/red_stitch_monotonic.rs:193:fn mn_9_recover_refuses_non_monotonic_catalogue() {
crates/journal/tests/red_stitch_monotonic.rs:231:fn mn_10_recover_reads_monotonic_catalogue() {
crates/journal/tests/red_stitch_monotonic.rs:245:fn mn_11_recover_boundary_catalogues_are_not_monotonicity_violations() {
```

Прочтение. Два вызова `check_monotonic_paths` (`read_all`/`recover`), 18 вхождений `recover`,
`segments_counted` в `open` (`:199`) и `refresh` (`:554`), `validate_like_full_path` — один вызов из
`is_fresh` (`:426`), три оракула `mn_9/10/11` — всё как написано. `JR-I-12`: бинарь
`journal-retention` с `--mode replay-digest` собирается и копируется в образ
(`Dockerfile:18`, `:30`). Соседи `TD-138` внутри `journal.md` — не в §I, а в §T (K-2).

---

## 3. Срез 3 — барьеры `scripts/check_*.sh`: кто их зовёт

Команда: `for f in scripts/check_*.sh; do b=$(basename $f); printf "%-32s ci.yml=%s\n" "$b" "$(grep -c "$b" .github/workflows/ci.yml)"; done; grep -n "run: bash scripts/check_branch_health.sh" .github/workflows/ci.yml; grep -c "branch-health" <(awk '/^  status-check:/,0' .github/workflows/ci.yml)`

```
check_artifact_ids.sh            ci.yml=2
check_branch_health.sh           ci.yml=1
check_context_budgets.sh         ci.yml=1
check_disk_budget.sh             ci.yml=0
check_docs_freeze.sh             ci.yml=1
check_gate_meta.sh               ci.yml=2
check_protected_artifacts.sh     ci.yml=1
check_resource_oracles.sh        ci.yml=1
check_review_fa.sh               ci.yml=1
check_rollout_composition.sh     ci.yml=1
check_unreachable_work.sh        ci.yml=0
472:        run: bash scripts/check_branch_health.sh || true
0
```

Агрегат `status-check.needs` (`ci.yml:557`): `build-test, security, delivery, protected-artifacts,
contracts, docs-freeze, artifact-ids, reserve-ids, design-claims, context-budgets, gate-meta,
deploy-catchup, review-fa, resource-oracles, rollout-composition`.

| барьер | проводка | утверждение документов | вердикт |
|---|---|---|---|
| 8 из 11 (`artifact_ids`, `context_budgets`, `docs_freeze`, `gate_meta`, `protected_artifacts`, `resource_oracles`, `review_fa`, `rollout_composition`) | джоб в агрегате | `gates.md` §4/§9/§11/§12, профили — «входит в агрегат», «держит merge» | сходится |
| `check_branch_health.sh` | джоб `branch-health`, `\|\| true`, **не в агрегате** | `ci.yml:458-471` сам объявляет «наблюдатель, НЕ блокирует»; `reading-map.md:141`, `architect.md:115` — как команда яруса S | сходится; блокирующим нигде не назван |
| `check_disk_budget.sh` | не в CI (по конструкции, `M-60b:249`); зовётся только `verify_M-60b.sh` | `M-60b:114` задача 9: «преамбула в `verify_M-60b.sh` и в шаблон новых verify» — колонка `⏳ OPEN`; `SESSION-HANDOFF.md:101` — «M-60b ИСПОЛНЕН И РАБОТАЕТ» | **см. S-6** |
| `check_unreachable_work.sh` | не в CI; `branch-hygiene.md:107` — ручной шаг закрытия сессии | нигде не объявлен CI-барьером | сходится |

### S-6 (средний) — `check_disk_budget.sh`: преамбула обещана «шаблону новых verify», шесть verify после M-60b её не зовут

Команда: `grep -l "check_disk_budget" scripts/verify_*.sh; ls scripts/verify_M-6*.sh scripts/verify_M-7*.sh; sed -n 114p milestones/M-60b-gate-mechanisms.md | cut -c1-120`

```
scripts/verify_M-60b.sh
scripts/verify_M-60a.sh
scripts/verify_M-60b.sh
scripts/verify_M-60c.sh
scripts/verify_M-61.sh
scripts/verify_M-65.sh
scripts/verify_M-66.sh
scripts/verify_M-68.sh
scripts/verify_M-71.sh
scripts/verify_M-73.sh
| 9 | `check_disk_budget.sh` + вызов-преамбула в `verify_M-60b.sh` (прод-форма) и в шабло
```

Прочтение. Барьер построен и проведён РОВНО в одно место; «шаблон новых verify» не
материализовался — `verify_M-61/65/66/68/71/73` преамбулы не несут. Индекс говорит «исполнен и
работает», колонка задачи — `⏳ OPEN`. Класс ENOSPC-ложного-красного, ради которого барьер
писали, для новых verify открыт. Не чиню; называю.

### S-7 (средний) — `scripts/tests/red_segment_meta_battery.sh`: батарея мутантов M-62 без живого вызывателя

Команда: `for f in scripts/tests/red_*.sh; do b=$(basename $f); n=$(grep -l "$b" .github/workflows/*.yml scripts/verify_*.sh scripts/check_*.sh scripts/tests/*.sh 2>/dev/null | grep -v "scripts/tests/$b$" | wc -l); [ "$n" = 0 ] && echo "$b -> callers=0"; done; grep -rln "red_segment_meta_battery" scripts .github | grep -v "^scripts/tests/red_segment_meta_battery.sh"; echo "(no live caller)"; grep -n "segment_meta_battery" docs/archive/verify_M-62.sh | head -1`

```
red_segment_meta_battery.sh -> callers=0
(no live caller)
211:BATTERY="scripts/tests/red_segment_meta_battery.sh"
```

Прочтение. 24 пробы `red_*.sh`; 23 зовутся CI/verify/соседями, одна — только из
`docs/archive/verify_M-62.sh`, то есть из архива. Батарея, доказывающая, что оракулы M-62
ЛОВЯТ (а не только зелены), не исполняется ничем в живом дереве — «built-not-wired» в чистом
виде, у проекта уже был такой случай (`LiveReducer`, `gates.md` §4). `docs/plans/plan-branches-and-ci-2026-08-19.md:184`
считает её «не заведённой» — верно, но это план, а не норма.

### S-8 (средний) — `gates.md:399`: «`red_protected_artifacts.sh` (20 сценариев)» — проба печатает 33

Команда: `bash scripts/tests/red_protected_artifacts.sh 2>&1 | grep "^VERDICT"; sed -n 399p .claude/rules/gates.md | cut -c1-110`

```
VERDICT: PASS (33/33) — барьер держит при ТОЙ ЖЕ проводке, какой его зовёт CI
Барьер сам под пробой: `scripts/tests/red_protected_artifacts.sh` (20 сценариев, fa
```

Прочтение. Правило утверждает число, которое проба опровергает своим же выводом; та же строка
требует, чтобы «число в вердикте пробы СЧИТАЛОСЬ, не заявлялось». `branch-hygiene.md:55`
(«8 сценариев» у `red_commit_paths.sh`) — сходится: `VERDICT: PASS (8/8 сценариев)`.
Профиль `architect.md` (30 `FACTS`, 40 `H-FACTS`, семь мест вызова `H-FACTS-SHA`,
`FACTS_HEAD_LINES = 5`/`FACTS_NOTE_THRESHOLD = 20` на `:630-631`) — сходится по всем числам.

---

## 4. Срез 4 — утверждения о ПРОДЕ (список для отдельной проверки; здесь НЕ проверялись)

### K-6 (крупный, проверяемо по репо) — DESIGN §23.2 и таблица уроков: «журнал в ЕДИНСТВЕННОЙ копии», «offsite-переноса НЕТ», «чекпоинтер 04:00» — против `deploy/cron.d` и живого индекса

Команда: `grep -n "ЕДИНСТВЕННОЙ копии\|offsite-переноса НЕТ\|только dry-run\|чекпоинтер 04:00" docs/DESIGN.md | cut -c1-140; ls deploy/cron.d/ deploy/bin/journal-offsite-cron.sh; grep -hE "^[0-9*]" deploy/cron.d/journal-offsite deploy/cron.d/journal-retention | cut -c1-90; grep -n "ПЕРВАЯ КОПИЯ" docs/SESSION-HANDOFF.md | cut -c1-90`

```
275:| Offsite-копия данных | — (у них терялось иначе) | журнал в ЕДИНСТВЕННОЙ копии; �
1003:(~1 GB сжато после компакции); cron: компакция 03:50, чекпоинтер 04:00, ретеншен 04:07 �
1004:**только dry-run** (offsite-переноса НЕТ — R1). Этого хватает текущему этапу и НЕ хва
deploy/bin/journal-offsite-cron.sh

deploy/cron.d/:
builder-prune
journal-offsite
journal-retention
22 * * * * root /root/hft-platform/deploy/bin/journal-offsite-cron.sh
7 4 * * * root /root/hft-platform/deploy/bin/journal-retention-cron.sh
50 3 * * * root /root/hft-platform/deploy/bin/journal-compaction-cron.sh
*/15 * * * * root flock -n /var/lock/hft-gateway-checkpoint.lock /root/hft-platform/deploy
465:| **`R1` — офсайт-копия журнала** | **ПЕРВАЯ КОПИЯ СУ
```

Прочтение. В дереве — механизм офсайт-копии (cron `:22` ежечасно + обёртка), а чекпоинтер
стоит на `*/15`, не на `04:00`. `SESSION-HANDOFF.md:465` говорит «первая копия существует
2026-08-29». Мастер-документ (ярус A) противоречит и репозиторию, и живому индексу того же
корпуса, и толкает к ПОВТОРНОМУ решению `R1`. Что копия реально идёт на VPS — вопрос прода
(ниже); что DESIGN отстал от `deploy/` — факт репо.

### Список утверждений о проде — проверять на сервере отдельно

| # | где | утверждение | что проверять |
|---|---|---|---|
| P-1 | `docs/DESIGN.md:274`, `:967-975`; `docs/PENDING-SIGNATURE.md:72-80` (П-003) | алертинг `ops-watchdog` смержен и «в проде», но канал **НЕ включён**: нет `TELEGRAM_BOT_TOKEN`/`TELEGRAM_CHAT_ID` и строки cron | env на VPS, crontab, свежие записи watchdog |
| P-2 | `docs/DESIGN.md:443` | чекпоинты `[ЕСТЬ] (M-48; cron ещё не стрелял по расписанию)` | `deploy/cron.d/journal-retention:79` — `*/15`; артефакты чекпоинта на VPS, mtime |
| P-3 | `docs/DESIGN.md:1001-1004` | 125 сегментов (115 `.zst` + 10 raw), 85 GB свободно, ~8.83 GB/сут; cron компакция 03:50 / чекпоинтер 04:00 / ретеншен 04:07 «только dry-run»; offsite НЕТ | `df`, `ls` тома журнала, crontab, режим `journal-retention` (dry-run/apply) |
| P-4 | `docs/DESIGN.md:275`; `docs/08-arch-improvement-roadmap.md:28` (R1); `docs/09-roadmap-v2.md:54` | «ретеншен-apply ни разу не выполнялся»; «cron сейчас dry-run!» | журнал запусков retention на VPS |
| P-5 | `docs/SESSION-HANDOFF.md:465-466` | офсайт-копия существует (Storage Box BX21, субаккаунт); диск прода 90 % → 62 % | `ssh` Storage Box, `df` на VPS |
| P-6 | `docs/SESSION-HANDOFF.md:432` | `main` = `10bc072`, прод = `a9f0bf5`, диск 80 %, worktree 74 | заведомо протухло по датам (§0ter датирован 2026-08-18); что на проде сейчас |
| P-7 | `docs/SESSION-HANDOFF.md:103` (§0, M-62) | «в `main`, но НЕ на проде» | SHA бинаря на VPS |
| P-8 | `docs/fa/viz-backend.md:42` | полосы глубины подписаны `П-014`, «в проде НЕ включено (`GATEWAY_BANDS=0.001`) до `TD-159`» | env `GATEWAY_BANDS` на VPS |
| P-9 | `docs/07-cockpit-backend-roadmap.md:67` | recorder 24/7 в проде: Binance spot+futures, HL; ротация+компакция+ретеншен-доставка «живут» | контейнеры, heartbeat, состав venue в журнале |
| P-10 | `docs/08-arch-improvement-roadmap.md:38` (R10) | 0 ресурс-лимитов у контейнеров | `docker inspect` / `docker-compose.yml` секции `deploy.resources` |
| P-11 | `docs/DESIGN.md:566` (§16.1 `[ЗАМЕРЕНО]` 2026-08-03) | замеры read-path на проде | датированы; не перепроверялись |
| P-12 | `docs/fa/journal.md:218-219` (`JR-I-12`) | «доставляемый бинарь умеет `--mode=replay-digest`» | в образе есть (`Dockerfile:18`, `:30`); что его кто-то на проде ЗАПУСКАЕТ — cron/журнал запусков |
| P-13 | `deploy/cron.d/*` (репо) vs crontab VPS | расписания `:22`, `04:07`, `03:50`, `*/15`, `04:30` | `crontab -l`/`/etc/cron.d` на VPS совпадают с репо |

---

## 5. Сводная таблица

| # | вес | где | класс | суть |
|---|---|---|---|---|
| K-1 | крупный | `DESIGN.md` §22 (`:917-930`) | обратный дрейф, системный | 12 семейств в таблице, 28 в тестах; 10 без документа-дома, 7 с домом в FA — вне карты |
| K-2 | крупный | `docs/fa/*.md` §T (10 файлов) | ложный механизм | §T называет 67 тестов, которых нет; `journal.md` §T противоречит `DESIGN` §22 |
| K-3 | крупный | `DESIGN.md:928` | ложное «без оракулов» | `book` покрыт `GD-I`/`BL-I`/`DET-I-2`, семейства недекларированы |
| K-4 | крупный | `DESIGN.md:932-933` | занижение (класс `C-041`) | «PL-I все PENDING» при 31 оракуле `pl_i_*` |
| K-5 | крупный | `docs/fa/{book,alpha,strategy,portfolio,venues,contracts}.md` §3 | pre-impl FA как действующая | типы/файлы/крейты, которых нет; 0 пересечений с крейтом у четырёх FA |
| K-6 | крупный | `DESIGN.md:275`, `:1003-1004`; `docs/08:28`; `docs/09:54` | документ отстал от `deploy/` | «offsite НЕТ / единственная копия / чекпоинтер 04:00» против `deploy/cron.d` и `SESSION-HANDOFF:465` |
| S-1 | средний | `docs/fa/ops.md:367`, `:380` | имя оракула | `runtime_persistent_volume_is_silent` → `_deficit_`/`_surplus_` |
| S-2 | средний | `DESIGN.md:209`, `docs/03:33`, `fa/signals`, `fa/research-cli`, `gates.md:248`, `scope-guard.md:14` | несуществующий носитель | `research/registry/signals.json` нет; ledger — `.jsonl`, не `.json` |
| S-3 | средний | `DESIGN.md:717/719`, `fa/viz-backend.md:92/100/127/143`, 10 профилей | протухшие `файл:строка` | номера уехали; `DESIGN:719` протух ещё и статусом `TD-031` |
| S-4 | средний | `docs/fa/README.md` | индекс | 14 при 18; `venues`/`runner` без крейтов; ops/viz-backend вне карты |
| S-5 | средний | `docs/fa/contracts.md:28-29` | «есть» vs «будет» | 1 из 7 T1-типов в крейте, каталога `types/` нет |
| S-6 | средний | `M-60b:114`, `SESSION-HANDOFF:101` | построено-не-проведено | `check_disk_budget.sh` — преамбула только в `verify_M-60b.sh`, шесть verify после — без |
| S-7 | средний | `scripts/tests/red_segment_meta_battery.sh` | построено-не-проведено | 0 живых вызывателей; единственный — в `docs/archive/` |
| S-8 | средний | `gates.md:399` | число о харнессе | «20 сценариев» при `33/33` |
| N-1 | мелкий | `docs/` | ID без определения | `DET-I-2`/`DET-I-3` определены только телом теста |
| N-2 | мелкий | `docs/fa/sim.md` §3 | имена | `FullFill`/`queue_position_ahead`/трейты при полном `SM-I` |
| N-3 | мелкий | `docs/fa/ops.md:280`, `:365` | история, помеченная как история | `red_recon_window.rs` переименован — сказано в том же месте |
| P-1..P-13 | — | см. §4 | прод | не проверялись, список для отдельного прогона |

Что ВЕРНО и проверено (чтобы следующий скан не тратил на это работу): 12 строк таблицы §22
(числовые ID); `VN-I` «40 тестов» (11+9+7+4+9); `MD-I-8` «21 оракул»; `JR-I-11` (рёбра, оракулы
`mn_9/10/11`, счётчик 18); `JR-I-12` (бинарь в образе); все числа `architect.md` о
`verify_design_claims.sh`; `branch-hygiene.md` «8 сценариев»; агрегат `status-check` и
`review-fa`/`docs-freeze` в нём; проверки 3/4/6/7 гейта.

---

## 6. ПРЕДЕЛЫ РАЗБОРА — что НЕ проверено и почему

1. **`docs/fa/*.md` (486 KB) целиком не читались.** Работал скриптами по backtick-токенам
   (пути, имена тестов, идентификаторы кода) плюс выборочное чтение: `journal.md` §I/§T
   целиком, `README.md`, отдельные строки `viz-backend.md`/`ops.md`/`book.md`/`contracts.md`/
   `sim.md`/`alpha.md`. Утверждение о механизме, сформулированное прозой без идентификатора в
   backtick'ах, скрипты не видят. Срез 2 — по форме, не по смыслу каждого инварианта.
2. **Действующий и исторический текст скрипты не различают** (кроме `lines.py`, который
   пропускал строки, начинающиеся с `>`). Для K-2/K-5 проверено руками, что §T и §3 —
   действующие секции, не приписки. Счётчики «нет_в_крейте» — сырьё: в них есть сквозные типы
   соседей и не-типы; выводы сделаны по нулевым пересечениям и ручному контролю, а не по числу.
3. **`docs/0N-*.md`** проверялись только грепом путей и прод-слов; `docs/00`, `01`, `02`, `07`,
   `08`, `09` содержательно не читались. `docs/plans/**`, `docs/workflow/**`, `milestones/*.md`
   — не проверялись (кроме двух процитированных строк).
4. **Архивный класс** (`research/critiques|reviews|arbitration`) — не проверялся, по
   решению `TD-064` он датированный снимок.
5. **`TECH-DEBT.md`/`PROJECT-STATE.md`/`PENDING-SIGNATURE.md`** — только грепом по предмету
   (`TD-138`, `TD-031`, `segment_meta_battery`, `П-003`, `П-014`); утверждения самих этих файлов
   о коде не аудировались (reviewer-owned).
6. **Прод не трогался** по мандату — срез 4 только перечисляет.
7. **Вне зоны:** `risk`/`killswitch`/`oms`/`runner`, семейства `RK-I`/`INTG-I`/`KS-I`/`OM-I`/
   `RN-I`. Их FA (`risk.md`, `killswitch.md`, `oms.md`, `runner.md`) в скрипт §T попадали и дали
   0 из N — это объявленное состояние, в находки не внесено. Пограничные упоминания
   (`RiskApproved`, `ParamChange`) названы только как форма утверждения, не как предмет.
8. **`cargo test` не запускался.** Покрытие считалось грепом идентификаторов, как это делает
   сама проверка 2 гейта; что тесты зелёные и что они ловят (мутация) — не проверялось.
9. **Проверка 2 гейта не считает суффиксные ID** (`VB-I-9b`, `ST-I-6a..8h`) — не разбирал, дефект
   это или соглашение; таблица §22 с ними согласована по числовым ID.
10. **`.claude/rules/**` и профили** проверялись только там, где они называют числа о харнессе
    или `файл:строка`; остальные утверждения правил о механизмах — не проверялись, кроме
    таблицы проводки в §3.
11. **Ни одна находка не чинилась и карточек долга не заводилось.**
