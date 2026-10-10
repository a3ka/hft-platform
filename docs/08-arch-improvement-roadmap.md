# docs/08 — Роадмап улучшений архитектуры (сквозной аудит 2026-07-27)

**Долговечный план.** Основан на сквозном fable-архитектурном аудите ВСЕХ слоёв (2026-07-27, метод:
5 параллельных аудит-агентов + разбор DESIGN/TECH-DEBT/PROJECT-STATE). Read-only анализ. Read-path
(латентность/чекпоинт) НЕ здесь — он в `milestones/M-38-roadmap.md`. HEAD аудита: origin/main @ 1a31136.

**Обновление 2026-07-31 (SaaS-решение founder'а, `DESIGN.md` §0/§13–§23).** Приоритизация ниже
писалась под однопользовательский/кокпит-для-founder'а продукт. При многопользовательском SaaS
на десятки тысяч пользователей меняется статус ровно одного пункта — **R7** (см. таблицу):
из «HIGH, латентный долг» в «CRIT, блокер существования продукта» (`DESIGN.md` §16). Остальные
риски (R1-R6, R8-R10) по существу не переоценены этим решением — асимметрия цены ошибки на
их путях (данные/journal/quant) не зависит от числа пользователей.

## Executive summary
Фундамент (журнал + контракты T1 + детерминизм + governance) — **крепкий и честный, выше типичного
проекта такого масштаба**. Активной порчи данных на main НЕ найдено. Риски — по КРАЯМ (эксплуатация,
непортированные фиксы, будущий quant/safety-слой), не в фундаменте. Единственный **экзистенциальный**
риск — журнал в единственной копии (R1). Всё остальное — приоритизируемый долг.

Приоритизация под текущую фазу: путь к деньгам (risk/oms/killswitch) физически НЕ построен (0 крейтов,
отложен пивотом P-COCKPIT), поэтому «наивысший приоритет = деньги» переводится в **«наивысший приоритет
= ДАННЫЕ»** — журнал единственный необратимый актив.

## TOP системных рисков

| ID | Сев. | Где | Цена ошибки / как проявится | Фикс |
|---|---|---|---|---|
| **R1** | CRIT | TD-006/020, docs/06 §7, deploy | Журнал в ЕДИНСТВЕННОЙ копии на 1 VPS. retention-`apply` (offsite на Storage Box) НИ РАЗУ не запускался (cron на `dry-run`, `/mnt/*` пуст, SB не смонтирован). Компакция сжимает in-place — копию НЕ создаёт. Пожар/диск/блок Hetzner = потеря ВСЕЙ истории. + ~40 дней до disk-guard-halt (2.8 GB/сут × 113 GB). | **founder-действие вне milestone:** смонтировать Storage Box + первый реальный apply + **restore-drill** (доказать восстановление). |
| ~~**R2**~~ ✅ **ЗАКРЫТ** `984f6f9` | — | journal/segments.rs `retention_plan()` | Сканирует каталог своим `read_dir` с фильтром `extension=="jrnl"` → `.jrnl.zst` (сжатые) БЕЗУСЛОВНО выпадают из плана. Включат R1 — .zst останутся на NVMe навсегда, offload/prune для них не сработает (противоречит docs/06 §4). Тихо подрывает R1. Не пойман: нет теста на смесь raw+.zst. | RED на смешанный каталог (.zst входят в план по возрасту) + фикс: `retention_plan` через общий `segments()`. ДО TD-020. |
| ~~**R2b**~~ ✅ **ЗАКРЫТ** `984f6f9` | — | journal/segments.rs `latest_segment_index()` | **Найден при спеке M-40 (2026-07-29), тот же корень, что R2, цена выше.** Третий энумератор каталога с тем же слепым фильтром `extension=="jrnl"`; через него работают `decide_open_segment` и `resolve_next_seq_with`. Каталог, восстановленный из холодного хранилища (одни `.zst`, без `journal.meta`), выглядит для писателя ПУСТЫМ ⇒ создаётся `segment-00000000.jrnl` поверх существующего `.zst`, `next_seq` стартует с 0 (дубликаты seq), и по D-COMP-1 (raw побеждает) восстановленный сжатый сегмент выпадает из чтения навсегда. Замер: 849 событий → 681 после старта; на прод-масштабе 59881 → 29933 (**потеряна половина истории**), при `open_with` = `Ok` и зелёном healthcheck. **Restore-drill R1 на этом коде заканчивается порчей того, что восстанавливали.** | Тот же фикс, что R2: один энумератор. + forward-only чтение хвоста `.zst` для `next_seq`. Оракулы `red_restore_from_cold.rs`, `red_restore_next_seq_bounded.rs` в `M-40`. |
| **R3** | CRIT (quant) | sim/funding.rs `funding_pnl_e8` (мёртвый код), sim/exchange.rs:275 `Md(Funding)` игнорируется | Funding НЕ применяется к PnL/equity бэктеста → систематически ОПТИМИСТИЧНЫЙ Sharpe по перпам (directional через 8h тик недосчитывает статью). fees закрыты fail-closed, funding — инварианта нет. M-04 task2 ✅«fees/funding» — funding по факту не поставлен (нет sacred RED). | architect-RED (Md(Funding) без применения → Err/Halt либо явный вызов) ДО первого R-NNN/промоушена. |
| **R4** | HIGH | venue-hyperliquid/src/lib.rs, `tests/` НЕТ | 0 тестов на единственный парсер нормализации HL→Event (parse_l2book/trade/message). Не покрыто: объектный формат {px,sz,n} (код сам «CRITICAL»), malformed (VN-I-7 неверифиц.), MID-фильтр, snapshot-семантика. HL — первая венью (DESIGN §0), в проде, пишет данные. Регрессия уйдёт в журнал тихо/необратимо. | RED-суита паритетно venue-binance. Дёшево, высокий ROI. |
| **R5** | HIGH | book/src/lib.rs | Staleness НЕ в типе (BK-I-3 не материализован). `depth_within`/`microprice`/`best_bid` — геттеры, не проверяют `self.stale`; `is_stale()` надо НЕ забыть вызвать. Кокпит покажет мёртвую ликвидность как живую (класс тихой лжи hft-core-rs). | Тип-барьер `BookView<Fresh\|Stale>` (как RiskApproved/ColdCopyProof), не bool. |
| **R6** | HIGH | venue-binance-futures/src/lib.rs:229 | TD-016 фикс (эвикция distance-window + backstop-кап) НЕ портирован из spot. Внутренняя книга фьючерсов на непрерывном diff растёт unbounded. `bucket_levels` скопирован 1:1, общего venue-common нет. | Портировать эвикцию+backstop + RED; вынести общий книжный код в venue-common. |
| **R7** | ~~HIGH~~ → **CRIT при SaaS** (повышен 2026-07-31, решение founder'а `DESIGN.md` §0/§13–§23) | gateway-serve/src/lib.rs:195, :528 | Неограниченные WS-соединения (spawn/коннект без cap → N клиентов = N полных сканов). + `GATEWAY_WINDOW_MS` parse-error ТИХО → None=unbounded (OOM-режим, разваливший прод). Единственное отступление от fail-closed. Noisy-neighbor с recorder (общий journal_dir). **При однопользовательском кокпите — эксплуатационный риск (мало клиентов); при SaaS на десятки тысяч пользователей — блокер существования продукта** (`DESIGN.md` §16 «Read-path под десятки тысяч пользователей»: N клиентов = N полных сканов журнала не масштабируется в принципе, не «пока не оптимизировано»). | (а) невалидный env → Err при старте; (б) cap соединений + rate-limit. Раньше — «в M-39 shared-tailer» (латентный долг); теперь — предусловие Ф2 `09-roadmap-v2.md` (shared-tailer + HOT-проекция + cap'ы/квоты, PL-I-4/PL-I-5), не опциональный рефакторинг. |
| **R8** | HIGH | strategy/src/lib.rs:223 | `in_flight` перезаписывается, не аккумулируется: повторный интент до филла ПЕРЕЗАПИСЫВАЕТ → effective_pos недосчитывает → избыточный интент (сам ST-I-3 называет «двойная позиция в live», покрыл половину). Pre-risk сейчас; долг до P3. | RED (смена target до филла) + `in_flight += `. До P3 (oms). |
| **R9** | HIGH | docs/03 §6, testing.md, CT-RFC | Заявленные sacred: INTG-I-1..7 (границы A/B/C) — 0 реальных тестов; CT-I-5 (Python-консюмер) — фикция (Python-кода нет). CT-RFC-02/03/04 STATUS: PROPOSED хотя MERGED. Читатель testing.md думает, что защита действует. | (1) синхр. RFC-статусы (PROPOSED→ADOPTED); (2) пометить INTG/CT-I-5 как «PENDING P3 — оракул не написан»; (3) INTG-оракулы RED-first перед safety-слоем. |
| **R10** | HIGH | docker-compose, ops | (а) 0 ресурс-лимитов → OOM gateway-serve уводит ВЕСЬ хост + роняет recorder (нет cgroup-изоляции). (б) gateway-serve на loopback, нет `ports:`/reverse-proxy → WS-эндпоинт кокпита НЕДОСТИЖИМ (блокирует P-COCKPIT). (в) push-алертинг не задеплоен → слепота между ssh-проверками. | (а) `mem_limit` на сервис (recorder приоритетно); (б) проброс порта + reverse-proxy runbook; (в) минимальный cron-watchdog→Telegram. |

## Последовательность работ (read-path M-38a→b→39 идёт своим чередом, см. M-38-roadmap)

**ШАГ 0 — необратимость данных (ВПЕРЁД кокпита):**
- **0a. R1** — Storage Box + первый retention-apply + restore-drill. **founder-действие** (операторская зона).
  **ОТЛОЖЕНО founder'ом ~2 недели (2026-07-27)** — пока не критично; вернуться ~2026-08-10. Диск тикает (~40 дней) — держать в поле зрения.
- **0b. R2 + R2b → `M-40`** ✅ **ЗАКРЫТО 2026-07-29** (merge `984f6f9`, §8 GREEN прод-замером: 122
  упоминания `.jrnl.zst` в dry-run, до фикса план был пуст). Остаточный долг вынесен в TD-049/050/051
  (хвост нечитаемого `.zst` при старте recorder'а — см. `M-49`). Ниже — исходная формулировка задачи:
  RED (retention над смесью raw+.zst; restore-drill поверх сжатой
  истории) + фикс: ОДИН энумератор сегментов на крейт. architect→engine-dev.
  **Порядок с 0a изменился: M-40 обязан лечь ДО первого apply и ДО restore-drill'а** — на текущем
  коде retention не увидит ~120 сжатых сегментов (бэкап дырявый), а restore-drill повредит
  восстановленный журнал. SPEC READY, RED закоммичен и красный (2026-07-29).

### Привязка к milestone'ам (scheduled)
| Шаг | Риск | Milestone | Статус |
|---|---|---|---|
| 0a | R1 | — (founder-действие) | ОТЛОЖЕНО ~2 нед |
| 0b | R2 + R2b | `M-40-retention-compaction-dedup` | ✅ **DONE** (merge `984f6f9`, §8 GREEN) |
| 1a | R4 | `M-41-venue-hyperliquid-tests` | PLANNED |
| 1b | R9 + докс-дрейф | `M-42-docs-governance-sync` | PLANNED |
| 1c | R10 | `M-43-ops-hardening` | PLANNED |
| 2 | R5, R7 | встроены в `M-38b`/`M-39` (M-38-roadmap); R7 теперь также = предусловие Ф2 `09-roadmap-v2.md` (см. R7 выше) | — |
| 3 | R6 + TD-029/030/032/033 | `M-44-book-hardening` | PLANNED |
| 5 | R3, R8, INTG-I-* | отложено до quant/P3 | DEFERRED |

**ШАГ 1 — параллельно M-38 (дёшево, высокий ROI, разные зоны):**
- 1a. venue-hyperliquid RED-суита (R4) — architect-RED → venue-dev.
- 1b. docs-sync milestone (docs-only, reviewer-бэкстоп): RFC-статусы + README/SESSION-HANDOFF + **домерж ветки `docs/06-volume-truth`** (фикс объёмов §2, замер опроверг на ~10-28×) + пометка INTG/CT-I-5 как PENDING (R9).
- 1c. compose: `mem_limit` на все сервисы + проброс порта gateway-serve (R10 а,б). Предусловие живого кокпита.

**ШАГ 2 — сложить в M-38b/M-39 (тот же read-path код):**
- 2a. R7 (fail-closed на GATEWAY_WINDOW_MS + cap соединений) → в M-39 shared-tailer.
- 2b. R5 (BookView) → до того, как фронт трактует книгу как источник решений.

**ШАГ 3 — journal-hardening milestone (латентный долг, один заход):** R6 (эвикция в futures + venue-common) + TD-029/030/032/033 (осторожно с TD-030: legacy first_seq=0).

**ШАГ 4 — push-алертинг (R10 в):** минимальный cron-watchdog→Telegram (heartbeat+disk_free); полный Prometheus — по BACKLOG.

**ШАГ 5 — при возобновлении quant/P3 (ПЕРЕД первым R-NNN и промоушеном):** R3 (funding в sim) → TD-015 epoch-механизация → R8 (in_flight) → INTG-I-1..7 оракулы RED-first → risk/killswitch/oms крейты.

## Тех-долг — приоритет
- **Тир A (данные/необратимость, текущая фаза):** TD-020/006 (=R1); TD-044 (=M-38, + R7 в тот же трек); НОВЫЙ (R2) dedup retention/compaction — завести TD, привязать к TD-020.
- **Тир B (governance/докс-дрейф, дёшево):** docs/fa/README + SESSION-HANDOFF устарели на десятки milestone (свежая сессия читает первыми → ложная картина); домерж docs/06-volume-truth; RFC-статусы; trials-ledger .json→.jsonl имя.
- **Тир C (латентные, defense-in-depth):** TD-032/033 (provenance-константа; SCHEMA_VERSION без машинного энфорса); TD-029/030 (startup schema-guard; reader first_seq monotonic); TD-034 (VP bins i64, недостижимо сейчас); TD-010/012 (REST depth undercount).

## Системные паттерны (что копится, дороже одного RED)
1. **«Идеальная фикстура» → композиция стадий.** Дефект проходит все зелёные гейты, ловится глазами на PR/§8 (4+ раза: M-07/M-08/TD-042 + аудит нашёл 5-й латентный R2). Чек-лист testing.md применяется к ОДНОМУ тесту; слепая зона — КОМПОЗИЦИЯ двух независимо-зелёных путей (compaction×retention, apply×live-frames_since). **Фикс: 6-й пункт чек-листа testing.md — «композиция стадий»:** если фича — стадия конвейера (cron-цепочка, live-push-loop, multi-apply fold), оракул ОБЯЗАН гонять последовательную композицию, не только стадию в изоляции.
2. **«Код на main ≠ функция в проде»** (TD-020-класс: коллектор не заспавнен, метрика не эмитится, retention не вызван, порт не проброшен). Лечится §8 eyes-on с DECODE (не grep). Дешёвый форсинг: verify-канарейка «объявлено ⟹ вызвано» (как OPS-I-10) на каждый «библиотека+вызыватель» шов.
3. **Асимметрия зрелости:** journal/contracts/sim/research-cli — промышленного качества; venue-hyperliquid (0 тестов, в проде) и safety-слой (0 крейтов) — дыры. Проект силён там, где прошёл через инцидент. venue-hyperliquid — исключение, закрыть.

## Что УЖЕ хорошо (НЕ сломать при рефакторинге)
Journal-ядро (CRC на каждом фрейме, tail-scan O(1) память TD-011, recover() отдельно от строгого read_all, компакция sha256-verify-перед-remove, ColdCopyProof приватным полем); contracts T1 (EventKind canary, hex-roundtrip старых версий, discriminant-freeze, JSON Schema в CI, legacy fail-closed); recorder fail-closed (JR-I-5, disk-guard до записи, backpressure, RssAnon); sim fill-model (cancel-ahead SM-I-5, no-lookahead submitted_seq, латентность/fees только из таблиц); research-cli барьеры (ValGateToken, ledger hash-chain, deflated Sharpe); book gap-детекция (fail-closed на разрыв); venue-binance эвикция v2 (документирует 2 прошлых провала оракула, red_recon_liveness с 418-моком); CI/governance (0 `#[ignore]`, clippy -D all-features, protected-artifacts барьер, 27 critiques, §8 поймал 5 инцидентов).

## Cross-references
- Fable-аудит 2026-07-27 (4 сообщения — у founder'а). `milestones/M-38-roadmap.md` (read-path).
- `docs/DESIGN.md`, `TECH-DEBT.md`, `.claude/rules/testing.md` (чек-лист — расширить п.6 композиция).
