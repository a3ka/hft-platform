# M-09 — Data safety net: система сама сообщает о тихой деградации (P2.5)

STATUS: **ACTIVE** (план ПРИНЯТ 2026-07-16: critic C-007 APPROVE → reviewer APPROVED → founder ★
приёмка фазы P2.5; исходный PROPOSED — 2026-07-15, architect). Doc-гейт `gates.md` §9 пройден,
`docs/fa/ops.md` → ACTIVE. **Следующий шаг impl-цепочки:** architect пишет CT-RFC-03 (T1,
БЛОКИРУЮЩАЯ) + RED-оракулы по OPS-I-1..9 на `feat/M-09` (RED не живёт на main, `gates.md` §8).

**ЗАВИСИМОСТЬ (жёсткая):** `docs/fa/ops.md` — STATUS PROPOSED. Пока FA не ПРИНЯТ
(critic → reviewer → founder ★), этот milestone не уходит в impl: OPS-I-1..9 из FA — источник
RED-оракулов, и если FA сдвинется на аудите, сдвинутся и они. **FA аудируется ПЕРВЫМ.**

## Objective

За пять инцидентов подряд (TD-011/013/014/016 + C1 M-08) ни один не пойман нашей автоматикой —
все глазами/ssh/code-review, при зелёном healthcheck. Плюс 15 GB боевых данных лежат **в одном
экземпляре на одном диске**. Мы строим торговлю на активе, который (а) не защищён от потери,
(б) не проверяется на правильность. Данные невосстановимы — код можно переписать.

M-09 закрывает три дыры (`docs/fa/ops.md` §2): **A. метрики+алерты**, **B. сверка с биржей
(recon)**, **C. сохранность (бэкап+restore-drill)**. Recon — единственная проверка, ловящая
C1-класс (порча данных при зелёном статусе). «Глаза оператора» (§8) — последний рубеж, не первый.

## Contract impact (T1) — ЕСТЬ: CT-RFC-03 (БЛОКИРУЮЩАЯ)

Recon-аудит (расхождение → ресинк) обязан оставлять след в журнале — новый вариант `SysEvent`
(`EventKind` — T1). Значит **atomic contract-RFC обязателен** (`05-contract-layer.md` §4,
`gates.md` §1.1). Без CT-RFC-03 подзадача recon НЕ стартует (BACKLOG M-09). Форма варианта
(что именно фиксируем: venue/symbol/ε/сторона расхождения/действие ресинка) определяется
recon-дизайном FA §4 и уточняется на critic-аудите ДО написания типов.

Аддитивность (CT-I-3): новый вариант `SysEvent` — расширение, старые журналы читаются вечно;
пакет RFC полный (Rust-канон + сгенерированная JSON Schema + фикстуры valid/invalid + CHANGELOG),
как CT-RFC-02.

## Allowed / Forbidden paths (scope-guard)

Роли — под ОБНОВЛЁННЫЙ `scope-guard.md` (C-007 C3/C4: без нового agent-профиля, явные carve-out'ы
существующим ролям):

- `contracts/` T1 — ТОЛЬКО через CT-RFC-03 (**architect**, atomic RFC).
- новый крейт `crates/ops/**` (метрики + recon-оркестратор-компаратор + правила алертов) —
  вводится этим milestone'ом (триггер critic §1.4). Владелец — **engine-dev** (scope-guard
  carve-out; ops — слой наблюдаемости, MD-only, НЕ risk/killswitch/oms).
- `crates/venue-*/src/**` — recon **REST-fetch** снапшота (**venue-dev**; MD-only, добавляет
  order-НЕзависимый REST-трафик → OPS-I-9 обязателен).
- `deploy/**` (бэкап cold-copy + restore-drill + `/metrics` scrape + **`deploy/alerts/` правила
  P0/P1/P2, task 4B**) — **engine-dev** (деплой-механика, scope-guard carve-out; не секреты).
- **`crates/recorder/src/{main.rs,metrics_server.rs}` — task-4 metrics-server carve-out (engine-dev).**
  Task 4A: рекордер спавнит async-таск, биндит `tokio::net::TcpListener` на loopback (127.0.0.1:порт,
  без внешнего доступа — §3) и на каждый запрос зовёт ЧИСТУЮ `ops::server::http_response(request_line,
  &metrics)`. Socket-loop — новый `metrics_server.rs`; `main.rs` — только `spawn` + bind-addr из env.
  `Arc<Metrics>` рекордер уже владеет. **journal-write путь (`JR-I-1`) и order-путь НЕ трогаются;
  экспорт НЕ в горячем пути (OPS-I-7: scrape читает атомики по запросу).** recorder Cargo.toml: engine-dev
  добавляет СВОЮ tokio-feature `net` (shared-access правило scope-guard). Reviewer в Block-scope
  подтверждает: диф recorder ⊂ `{main.rs (spawn), metrics_server.rs}`, MD-only, без journal/order.
- **`crates/recorder/src/{lib.rs,main.rs,recon_loop.rs,metric_emit.rs}` + `crates/venue-*/src/**` —
  task-4C metric-emission carve-out (engine-dev; venue-dev для reconnect).** Развести продюсеры (§3
  продюсер-карта): `run_writer` (lib.rs) эмитит `journal_frames_written_total`/`journal_seq_current`/
  `journal_segment_index`/`journal_disk_free_bytes`/`journal_write_errors_total` (последний — writer-event
  на ошибке append) + `md_events_total` (классификация `EventKind` при append; **канон `kind`-label — ОДИН на вариант
  `MdPayload`: `trade`/`l2snapshot`/`funding`/`open_interest`/`liquidation`/`margin_rate`; kind ОБЯЗАН
  отражать РЕАЛЬНЫЙ тип payload; venue/symbol/kind РАЗЛИЧИМЫ, не схлопнуты — C-014 re-audit #2/#3**;
  данные storage/seq УЖЕ считаются для heartbeat; steady `journal_seq_current`/`disk_free_bytes` > 0,
  не dead-zero). **`md_event_age_ms{venue}` — ОТДЕЛЬНЫЙ sampler-сейм `metric_emit::sample_md_age(metrics,
  venue, now_ms, last_receipt_ms)` = `now - last` (silence растёт при тишине, OPS-I-8); recorder трекает
  `last_receipt` per venue, периодический sampler (main спавнит; live-wiring канарейка) зовёт с реальным
  `now`.** ЖИВОЙ feeder-loop `run_books_feeder` (lib.rs; экстракт inline books-feeder'а
  из `main.rs` — применяет `apply_md_to_books` + эмитит) → `book_levels`, и `main` ОБЯЗАН его спавнить
  (C-014 gap-2 live-wiring); периодический sampler (`main.rs` спавн + `metric_emit.rs`) → чистый
  `parse_rss_anon(status)` берёт ИМЕННО `RssAnon` (НЕ `VmRSS`/cgroup — page cache завышает, TD-021) ×1024 →
  `recorder_rss_anon_bytes` (>0 на живом); supervisor (`main.rs`) ИЛИ venue-`run` → `venue_ws_reconnects_total`.
  **ЖЁСТКИЕ ГРАНИЦЫ:** метрики — ТОЛЬКО
  атомик-инкременты рядом с существующими операциями (**OPS-I-7**, не новый горячий путь); **в журнал
  НЕ пишутся** (**OPS-I-6** — `metrics.inc/set`, НЕ `journal.append`); `run_writer` меняет ТОЛЬКО эмиссию
  (append/flush/shutdown-семантика `JR-I-1` НЕ меняется — architect обновляет её RED-вызовы под новую
  сигнатуру с `&Metrics`); venue-* тронуть ТОЛЬКО reconnect-счётчик (MD-only, без order-egress). Reviewer
  Block-scope: диф ⊂ названных файлов, каждая правка = атомик-эмиссия рядом с существующим кодом.
- **`crates/recorder/src/{main.rs,recon_loop.rs}` — ТОЛЬКО recon-loop wiring (engine-dev, явный
  Task-2 carve-out).** Оркестратор recon (`main.rs`: hoist `ReconDetector::new(thr)` ДО цикла
  `while let Some(reference)`, передать `&mut detector` первым аргументом `handle_recon_snapshot`,
  убрать старый `thr`-аргумент) + books-feeder (`recon_loop.rs`: `apply_md_to_books` из
  `MdEvent::L2Snapshot`). MD-only плюмбинг для recon; **journal-write путь (`JR-I-1`: журнал пишет
  только recorder) и прочая логика recorder НЕ трогаются**. Причина carve-out: оконное состояние
  (`ReconDetector`) обязано жить у ВЛАДЕЛЬЦА цикла снапшотов (рекордера) — вынести некуда без
  скрытого глобального состояния; recorder не входит в scope-guard-строку engine-dev, поэтому нужен
  явный milestone-carve-out (C-011 B2). Reviewer в Block-scope подтверждает: диф recorder ⊂ этих
  двух файлов и не трогает journal-write/order-путь.
- `research/data-quality/**` (офлайн-сводка recon-расхождений + gap-статистика, агрегация журнала)
  — **research-dev** (scope-guard; ТА ЖЕ роль/путь, что уже пишет gap-статистику).
- `*/tests/**` (OPS-I-* RED, sacred), `scripts/verify_M-09.sh` — **architect**.

**Forbidden:** `crates/risk|killswitch|oms` (не эта фаза); order-egress в venue-* (recon — только
чтение REST); запись рантаймом в `research/data-quality/` (это офлайн-агрегация — рантайм пишет
`Sys`-событие в журнал + живые метрики, OPS-I-6); ручная правка `research/registry/signals.json`.

## §Tasks (план; RED-оракулы пишутся ПОСЛЕ critic+FA-приёмки)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 0 | ✅ | **FA-приёмка (ПРЕДУСЛОВИЕ):** `docs/fa/ops.md` PROPOSED → ACTIVE через doc-гейт | critic → reviewer → founder ★ | ✅ 2026-07-16: critic C-007 APPROVE + reviewer APPROVED + founder ★; `ops.md` STATUS ACTIVE |
| 1 | ✅ | **CT-RFC-03 (T1, БЛОКИРУЮЩАЯ):** `SysEvent::ReconDivergence(ReconAudit)` + `ReconAction` (аддитивно, хвост) + сген. JSON Schema + фикстуры + `red_rfc03` | architect | ✅ merged `cf53e81`, §8 inert-safe; roundtrip+CT-I-3+compile-RED зелёные; schema_version не бампнут |
| 2 | ✅ | **B2 ЗАКРЫТ + В ПРОДЕ (`17b02a7`, §8 PROD GREEN 2026-07-18: healthy 0 эмиссий, injection best-порчи → 4× best=true, флуд B удалён).** Recon (OPS-I-1 + OPS-I-9): рантайм = best-price per-cycle + seed-gate; объёмная near-touch сверка снята с рантайма (REST-неверифицируема) → офлайн-трек; rate-budget/backoff (TD-013) | engine-dev (ops) | ✅ merge B2 `4939d8f`; §8 PROD GREEN `17b02a7`; TD-025(B) CLOSED |
| 3 | ⏳ | **Сохранность (OPS-I-2/3):** cold-copy журнала offsite (Storage Box) + **restore-drill** (скачать→прочитать `journal::stream`, `seq` непрерывен) — на РЕАЛЬНОМ сегменте, legacy-0 первым. **+ NOTE-2 (TD-027): продюсер `journal_seq_gaps_total` живёт ЗДЕСЬ** (replay/drill считает разрывы seq через границы сегментов → OPS-GAP; writer-seq монотонен) | engine-dev | RED/§8: restore-drill на реальном сегменте (не фикстуре); удаление горячей копии — только через `ColdCopyProof`; seq_gaps инкрементируется на инъецированном разрыве при replay |
| 4 | ✅ | **Метрики+алерты (OPS-I-4..8) ЗАКРЫТ + В ПРОДЕ (`1919350`/код `9a352d6`, §8 PROD GREEN).** (A) `/metrics` loopback-сервер отдаёт; (B) каталог правил + паритет OPS-I-5. **⚠ §8 вскрыл TD-027 (MAJOR): 13/15 метрик ОБЪЯВЛЕНЫ, но НЕ wired — правила формирующих инцидентов ссылаются на мёртвые метрики → задача 4C.** | architect → critic → engine-dev → tester → reviewer | ✅ merge `9a352d6`; §8 GREEN (/metrics отдаёт `book_divergence_bps` non-zero). TD-027 OPEN → task 4C |
| 4C | ✅ | **Живая ЭМИССИЯ метрик (OPS-I-10, TD-027) ЗАКРЫТ + В ПРОДЕ (`e61dd3a`/код `ac645ac`, §8 PROD GREEN: 13 ранее-мёртвых метрик несут живые SAMPLE'ы).** Развёл каждую steady-метрику до реального инкремента (writer→journal_*+md_events; feeder→book_levels; sampler→rss/md_age). RED (7 раундов critic-аудита C-014, APPROVE `7bc6dd3`). **OPS-I-6/7 сохранены.** | architect→critic→engine-dev(+venue-dev)→tester→reviewer §8 | ✅ merge `ac645ac`; §8 GREEN (живые SAMPLE'ы); TD-027 CLOSED. 2 NOTE → task 4D |
| 4D | ✅ | **Метрик-контракт: NOTE-1/NOTE-2 из TD-027 ЗАКРЫТ + В ПРОДЕ (`e9ca1ab`/код `83c340c`, §8 PROD GREEN).** NOTE-1 — `journal_bytes_written_total`→**`journal_frames_written_total`** (честное имя: считает кадры; TD-011-liveness сохранён). NOTE-2 — `journal_seq_gaps_total` **reclassified read-side** (продюсер = restore-drill/replay, приходит с task 3; writer-seq монотонен → OPS-GAP иначе false-safety класса TD-027). | architect RED → critic C-015 APPROVE → engine-dev rename → tester → reviewer §8 | ✅ merge `83c340c`; §8 GREEN (frames в /metrics). **TD-027 NOTE-1 CLOSED; NOTE-2 отложен до task 3.** |
| 5 | ⏳ | `scripts/verify_M-09.sh` — ≥1 проверка на задачу; финальный `VERDICT` | architect | exit=0 на GREEN |
| 6 | ⏳ | tester clean-checkout + reviewer §8 (прод НЕ инертен: recon реально шлёт REST и пишет `Sys` при инъекции; `/metrics` отдаёт) | tester/reviewer | Done Block + §8 пруф |

## Task 2 — §8-провал и recon-redesign (2026-07-17, architect + founder ★)

§8 первого impl провалился: recon эмитил `ReconDivergence` на КАЖДОМ цикле здорового рынка
(`divergence_bps=2436..7754`, `best_price_diverged=true`) без инъекции порчи. **Измеренный корень**
(architect, живой Binance REST): `/api/v3/depth?limit=5000` достаёт лишь **~1.1–1.7% от mid**, а
`RECON_BANDS=[1.5%,3%,8%]` сравнивали суммы полос там, где у reference НЕТ данных → структурный флуд
(асимметрия глубины, не бакетинг). Бакетинг эмита при этом даёт **0.0 bps** на мелких полосах — фикс
«сырая книга адаптера» ОТВЕРГНУТ замером.

**Redesign (founder ★ 2026-07-17 — near-book recon + отдельный трек 6–60%), детали `docs/fa/ops.md §4.2`:**
- фикс — **ТОЛЬКО `crates/ops::recon`** (engine-dev): (a) best-price толерантность `BEST_SKEW_BPS`;
  (b) `RECON_BANDS → [0.1%,0.3%,0.5%]` + пропуск полосы за `reference.max_reach_pct`; (c) пустой
  reference → расхождение best. **venue-dev/wiring НЕ трогаются** (бакет-фидер валиден, замер 0 bps).
- sacred RED переписан на LIVE-режим: `crates/ops/tests/red_recon_live.rs` (7 оракулов: skew-толер.,
  десинк, §8-анти-флуд глубины, near-book C1, near-touch фантом, пустой ref, гард мелких полос) +
  `red_ops_recon.rs`/`red_recon_sink.rs` переведены на band-достающие фикстуры. Прогон против
  прототипа корректного impl — 16/16 GREEN (достижимо); против текущего impl — падают ровно
  band-shallow + skew + depth-skip (анти-плацебо в обе стороны).
- **отдельный трек (НЕ M-09):** фантом дальних полос 6–60% (TD-016) REST-неверифицируем — корректность
  within-band эвикции + возможный platform-reference (§5.1 магнитудная загадка). Founder ★ развилка позже.

## Task 2 — ВТОРОЙ §8-провал: объёмный timing-skew → оконная персистентность (2026-07-17, architect + founder ★)

После near-book redesign engine-dev реализовал near-book recon (`4bd67ec`, 16/16 recon-сюита GREEN).
Reviewer прогнал §8 отдельным замером (2 живых прогона Binance ×210с, оба feeder'а): depth-asymmetry
(виток 1) ЗАКРЫТА, best-price ложняки УСТРАНЕНЫ (`best_price_diverged=false`), магнитуда упала ~10–30×.
**НО recon ВСЁ ЕЩЁ флудил** `ReconDivergence` ~1/цикл/символ (`band_divergence` 16–853 bps, большинство
`> ε_test=50`) на ЗДОРОВОМ рынке. §8-тишина НЕ достигнута — этажом глубже.

**Измеренный корень (architect, воспроизведён 2026-07-17):** near-book ОБЪЁМНЫЕ суммы полос расходятся
между local (WS-книга, момент T1) и reference (async REST, момент T2) из-за timing-skew — near-touch
объём BTC/ETH churn'ит за секунды, **ЗНАК per-cycle ГУЛЯЕТ** (`+-+`, `+--`, `-0+`; BTC BID 0.1% даже
`---`). Прежний оракул `deep_local_vs_truncated_reference_does_not_flood` сравнивал ИДЕНТИЧНЫЙ near-book
объём у local и reference → снова не смоделировал live (объёмный скью), тот же класс green-unit/live-flood.
Per-cycle порог по объёму **принципиально нежизнеспособен** (тупик семантики), НЕ калибровка.

**Redesign (founder ★ 2026-07-17 — оконная персистентность; детали `docs/fa/ops.md §4.3`):**
- дискриминатор churn↔порча — НЕ магнитуда, а ПЕРСИСТЕНТНОСТЬ ЗНАКА (churn mean→0 за окно; порча держит
  знак). recon становится STATEFUL (`ReconDetector`): окно `RECON_WINDOW` циклов на (полосу,сторону),
  знаковое среднее. Best-price — ПО-ПРЕЖНЕМУ per-cycle (immediate). `book_divergence_bps` per-cycle →
  ДЕМОТИРОВАН в метрику-гейдж (не триггер эмиссии). T1 НЕ меняется (windowed-алерт = `ReconAudit`
  {best=false, divergence_bps=|mean|}) — **CT-RFC не нужен**.
- фикс — ТОЛЬКО `crates/ops` (`ReconDetector` в `recon.rs` + stateful `sink.rs`; **engine-dev**).
  venue REST-фетчер / best-price / depth-skip / `BEST_SKEW` — НЕ трогаются (работают).
- **engine-dev рервайрит `crates/recorder/src/main.rs`** recon-loop (явный Task-2 carve-out —
  см. §Allowed paths): `ReconDetector::new(thr)` ДО `while let Some(reference)`, передать
  `&mut detector` первым аргументом `handle_recon_snapshot` (сигнатура sink изменена — old
  `thr`-параметр убран). 3-строчная механическая правка, MD-only, journal-write не трогается.
- sacred RED: НОВЫЙ `crates/ops/tests/red_recon_window.rs` (7 оракулов: churn→тишина вкл. 3-подряд-знак,
  персистентный дефицit/профицит, C1-эвикция оконно, ε_test не калибруем оконно, детерминизм);
  `red_recon_sink.rs` переписан на stateful (churn-последовательность→тишина, персистент/best→эмит);
  `red_recon_live.rs`/`red_ops_recon.rs` — per-cycle объёмные оракулы сняты (переехали в окно), best/
  depth-skip остались. Прототип корректного impl → 21/21 GREEN (достижимо); анти-плацебо в обе стороны
  (per-cycle → churn падает; always-silent → персистент падает). Skeleton (todo!()): window+sink RED,
  live+ops_recon GREEN.
- **§8-acceptance уточнён (founder ★):** «recon молчит на здоровой книге» достижимо ТОЛЬКО оконно;
  per-cycle тишина по объёму невозможна физически (churn до 2007 bps). Задержка объёмной порчи — до
  `K` циклов (best-порча — immediate). `K` + оконный `ε_prod` — калибровка на живом churn + §8.

## Task 2 — ТРЕТИЙ §8-провал: seed-race (A) + объёмный runtime нежизнеспособен (B, развилка ★) (2026-07-18)

Код смержен в main (`b1adec0`), юнит-гейты зелёные (ops 33/33, workspace 256/0, `verify_M-09` PASS),
но reviewer §8 eyes-on намерил ФЛУД `Sys(ReconDivergence)` в durable-журнал на ЗДОРОВОМ рынке (прод НЕ
инертен). Детали и дизайн — `docs/fa/ops.md §4.3.1` (seed-gate) + `§4.3.2` (развилка). Два дефекта:

- **(A) seed-race — ИСПРАВЛЯЕТСЯ (RED написан).** 4 стартовых `best_diverged=true div=10000/рестарт`:
  fetcher тянет REST ДО первого `L2Snapshot` feeder'а → сравнение с ПУСТОЙ local. Дизайн — SELF-SEEDING
  `ReconDetector` (`seeded: bool`, true на первой непустой local; до seed — no-alert, окно не кормится;
  пост-seed пустота = порча → эмит). RED (sacred): `empty_local_before_first_seed_does_not_emit` (падает
  против текущего impl), `empty_local_after_seed_is_corruption_and_emits` (анти-плацебо). Импл — engine-dev
  в `crates/ops/src/recon.rs` (`ReconDetector`, БЕЗ смены сигнатуры sink/recorder — carve-out не расширяется).

- **(B) объёмный оконный флуд — ✅ B2 ПРИНЯТ founder ★ 2026-07-18.** 12+ оконных `best=false
  div=41..1129 Resynced` (§8 re-run: 103..747), ~1/мин, все символы; часть ≫ ε_max=50 → порогом не
  подавляемы; **в т.ч. на нетронутом инъекцией `BinanceFutures`** → систематический (не zero-mean) сдвиг
  WS-бакет-книги(T1) vs raw-REST(T2) по near-touch объёму — усреднение окна гасит дисперсию, НЕ bias.
  ТРЕТИЙ §8-провал одного класса. B1 (калибровка+K) не лечит систематический bias и упирается в
  fail-closed `ε_max=50` → отклонён. **★ РЕШЕНИЕ B2 (founder ★):** рантайм-recon = best-price per-cycle +
  seed-gate; рантайм-эмиссию объёмной near-touch сверки убрать (REST-неверифицируема). Полная
  книга/объёмы/глубина пишутся в журнал БЕЗ изменений. Объёмная сверка — офлайн-трек (research-dev),
  необязательный follow-up. Детали — `docs/fa/ops.md` §4.3.2 (принято через doc-гейт `gates.md` §9 —
  правка ACTIVE FA + milestone Acceptance).

### Task 2 — B2 impl-контракт (2026-07-18, architect RED-first готов → engine-dev)

**Что делает B2 (в рамках M-09 task 2, БЕЗ нового milestone, БЕЗ CT-RFC — `ReconAudit` T1 НЕ трогается):**
- **engine-dev, `crates/ops/src/recon.rs`:** `ReconDetector::observe` → **рантайм-alert ⟺
  `best_price_diverged`** (объёмный оконный путь снят из решения об эмиссии). Рекомендация architect —
  чистое удаление window-машинерии (окна/`window_alert`), т.к. под B2 они мёртвый код: гейдж
  `book_divergence_bps` берётся из `reconcile().divergence_bps` (per-cycle), а не из окна. `seeded`
  (seed-gate §4.3.1) СОХРАНЯЕТСЯ (`&mut ReconDetector` по-прежнему нужен → **сигнатуры
  `sink::handle_recon_snapshot`/recorder НЕ растут, carve-out НЕ расширяется**). Гейдж обновляется каждый
  цикл (наблюдаемость), но алерт не поднимает. Детерминизм сохранён. **Обнови doc-комменты в `recon.rs`,
  ссылающиеся на `red_recon_window.rs` → `red_recon_runtime.rs`.**
- **architect (sacred RED, ГОТОВО — reachability §8-подтверждён локально):** `red_recon_window.rs` →
  переименован `red_recon_runtime.rs`; объёмные оконные оракулы 1–7 СНЯТЫ (идея → офлайн-спека
  research-dev); добавлены `runtime_persistent_volume_deficit/surplus_is_silent`,
  `runtime_nonbest_eviction_is_silent`, `runtime_post_seed_empty_local_still_emits`, детерминизм;
  seed-gate 9a/9b/9c ОСТАЮТСЯ. `red_recon_sink.rs` переспецирован (персистентный объём→тишина,
  best-десинк→эмит+метрики). `red_recon_live.rs`/`red_ops_recon.rs` — best/depth-skip гейдж без изменений
  логики (только устаревшие ссылки поправлены). Анти-плацебо ПОДТВЕРЖДЁН в ОБЕ стороны: против текущего
  window-active impl 4 B2-silent оракула ПАДАЮТ; против always-silent impl best-эмит оракулы ПАДАЮТ;
  против best-only+seed-gate impl вся ops-сюита GREEN (reachability).

**§8-acceptance task-2 (для reviewer):** healthy → **тишина** (best-путь; §8 уже дал 0 эмиссий на healthy
после seed-gate); injection best-порчи (заморозка WS, REST жив) → **эмит** (§8 уже дал 6× `best=true`).
**Объёмной тишины в рантайме НЕ требовать** — объёмного рантайм-пути больше нет (удалён, а не подавлен
порогом). Это закрывает дефект B: единственный флудивший путь удалён; best-путь §8-зелёный.

**Офлайн-объёмный трек — НЕ блокирует закрытие дефекта B** (см. BACKLOG «M-09 хвост: офлайн data-quality
объёмная сверка», research-dev).

## Task 4 — spec (2026-07-19, architect RED-first готов → engine-dev)

Реестр метрик (`ops::metrics::METRICS`, `prometheus_text()`), silence (`ops::silence`) и паритет ИМЁН
метрик (`verify_M-09.sh`) УЖЕ есть и зелёные. Task 4 закрывает ДВЕ дыры:

**4A — `/metrics` HTTP-сервер (гэп: `prometheus_text()` есть, но никто не сервит — §8 recon мерили
bounded-декодером журнала).**
- **architect RED (sacred, ГОТОВО):** `crates/ops/tests/red_ops_server.rs` — ЧИСТЫЙ контракт
  `ops::server::http_response(request_line: &str, metrics: &Metrics) -> String`:
  `GET /metrics HTTP/1.1` → `HTTP/1.1 200`, заголовок `Content-Type: text/plain; version=0.0.4`, тело =
  `prometheus_text()` (несёт §3-метрики + РЕАЛЬНОЕ значение set-гейджа, не заглушка); не-`/metrics` →
  `404`; не-GET → `405`. + `crates/recorder/tests/red_metrics_endpoint.rs` — bind эфемерного
  `127.0.0.1:0`, `recorder::metrics_server::serve(listener, Arc<Metrics>)`, реальный TCP GET → 200 + тело
  несёт метрику (socket-путь, loopback = без внешнего доступа).
- **engine-dev impl:** `crates/ops/src/server.rs` (ЧИСТАЯ `http_response`, БЕЗ tokio — ops остаётся
  лёгким, только `contracts`+`book`); `crates/recorder/src/metrics_server.rs` (`serve` — accept-loop:
  прочитать request-line, вызвать `ops::server::http_response`, записать, закрыть) + `main.rs` спавн +
  bind-addr из env (дефолт loopback). recorder Cargo: +tokio feature `net`.

**4B — правила алертов P0/P1/P2 + двусторонний паритет OPS-I-5 (rule-side).**
- **architect RED (sacred, ГОТОВО):** `crates/ops/tests/red_ops_alerts.rs` — контракт
  `ops::alerts::{Severity{P0,P1,P2}, AlertRule{incident,severity,metric,summary}, ALERT_RULES,
  to_prometheus_rules()}`: (1) КАЖДОЕ `rule.metric ∈ metric_names()` (правило-без-метрики → FAIL);
  (2) КАЖДЫЙ обязательный класс инцидента §7.1 имеет ≥1 правило (класс-без-правила → FAIL, канон-список
  в оракуле = `verify` REQUIRED_INCIDENTS); (3) `to_prometheus_rules()` рендерит каждое правило с его
  метрикой И severity (не пустой рендер — семантика, анти-плацебо). Анти-плацебо в обе стороны:
  правило→несуществующая метрика падает (1); удаление правила класса падает (2); пустой рендер падает (3).
- **verify_M-09.sh:** кросс-чек каталога `ALERT_RULES` ↔ FA §7.1 incident-IDs В ОБЕ СТОРОНЫ (Rust-канон и
  FA не расходятся); прогон `red_ops_server` + `red_ops_alerts`.
- **engine-dev impl:** `crates/ops/src/alerts.rs` (каталог зеркалит FA §7.1 + рендер Prometheus-правил —
  ОДИН канон); `deploy/alerts/ops.rules.yml` = вывод `to_prometheus_rules()` (deploy-артефакт). **Живой
  Alertmanager/Prometheus НЕ провижен (§O открыт)** — правила АВТОРИРУЮТСЯ + паритет-проверяются сейчас;
  live-alerting включается, когда founder ★ провижит Prometheus. Это ЧЕСТНО: «метрика без алерта
  бесполезна» (OPS-I-5) закрывается артефактом+паритетом; scrape-endpoint (4A) §8-валидируется живым curl.

**Гейты task 4:** critic (новый модуль `ops::server`+`ops::alerts` + recorder carve-out + milestone
Allowed-paths правка = doc-гейт §9 Class A). MD-only (только чтение атомиков + serve, без order-egress) →
risk-critic N/A. §8: curl `/metrics` на VPS отдаёт `book_divergence_bps{venue,symbol}` (наблюдаемость,
которой не было). T1 НЕ трогается → CT-RFC не нужен.

## Task 4C — spec (2026-07-19, architect RED-first готов → engine-dev)

**Корень TD-027 (§8, reviewer):** task 4 дал реестр+сервер+правила+паритет OPS-I-5 (зелёный), но 13/15
метрик НЕ подключены к продюсерам → инкрементируются только `book_divergence_bps` + `venue_http_status_
total`. Правила P0/P1 формирующих инцидентов (TD-011/014/016/OPS-GAP) ссылаются на МЁРТВЫЕ метрики.
Паритет проверял ИМЕНА реестра, а не РАНТАЙМ-ЭМИССИЮ — тот же класс, что recon-wiring кормил пустую книгу.

**Инвариант OPS-I-10 (FA §6):** объявлена ⟹ эмитится. У каждой §3-метрики — названный продюсер-сейм (§3
продюсер-карта); RED прогоняет ПРОДЮСЕР и ассертит SAMPLE-серию (`name{labels} value`), не только HELP/TYPE.

- **architect RED (sacred, ГОТОВО):**
  - `crates/recorder/tests/red_metrics_emission.rs` — прогоняет РЕАЛЬНЫЕ продюсеры с общим `Arc<Metrics>`:
    (а) `run_writer` (новая сигнатура с `&Metrics`) на последовательности Md+Sys событий + shutdown →
    `prometheus_text()` несёт SAMPLE для `journal_frames_written_total`>0, `journal_seq_current`>0,
    `journal_segment_index`, `journal_disk_free_bytes`, `md_events_total{venue,symbol,kind}`>0,
    `md_event_age_ms{venue}`; (б) ЖИВОЙ loop `run_books_feeder(md_rx, books, &Metrics)` — ТОТ ЖЕ, что
    спавнит `main` (НЕ leaf-хелпер) — на L2Snapshot → `book_levels{venue,symbol,side}` SAMPLE; (в) sampler
    → `recorder_rss_anon_bytes` SAMPLE. **C-014 gap-1:** labeled-метрики (`md_*`, `book_levels`)
    проверяются `has_labeled_sample(text,name,keys)` — размерность ОБЯЗАНА присутствовать (схлопнутый
    `md_events_total 30` без `{venue,symbol,kind}` НЕ проходит, урок C-009 M2). `has_sample` отличает
    SAMPLE от `# HELP/# TYPE` — анти-TD-027. **C-014 gap-2:** book_levels/rss тестируются ЧЕРЕЗ live-loop
    /sampler, а verify live-wiring-канарейка требует их ВЫЗОВА в `main.rs` (helper-only-non-live → FAIL).
  - Обновлены sacred RED-вызовы `run_writer` под новую сигнатуру (`red_shutdown_j1`, `red_heartbeat_status`,
    `red_rss_bounded`, `red_recon_wiring`) — append/flush/shutdown-семантика (`JR-I-1`) НЕ меняется, только
    добавлен `&Metrics`-аргумент.
  - `scripts/verify_M-09.sh` — **emission-канарейка (OPS-I-10):** каждая `METRICS`-запись покрыта
    assert'ом в emission-оракуле ИЛИ явно помечена `event`/`deferred` (backup_restore_drill_ok — task 3).
- **engine-dev impl (+venue-dev):** продюсер-инкременты по carve-out (§Allowed paths task-4C). ТОЛЬКО
  атомики рядом с существующими операциями (OPS-I-7); НЕ в журнал (OPS-I-6); `run_writer` меняет ТОЛЬКО
  эмиссию. `venue_ws_reconnects_total` — supervisor или venue-`run` (venue-dev, MD-only).
- **Гейты:** critic (Class A: FA §1/§3/§6 + milestone Allowed-paths + касание sacred writer-сейма `JR-I-1`).
  MD-only (метрики read-side, без order-egress) → risk-critic N/A. **§8: curl /metrics на VPS — ВСЕ
  steady-метрики non-zero** (ровно проверка, что вскрыла TD-027). T1 НЕ трогается → CT-RFC не нужен.

## Acceptance (исполняемые ворота — BACKLOG M-09 + OPS-I-*)

1. **(B2) рантайм = best-only + seed-gate:** инъецированная BEST-порча (пропавший/сдвинутый best сверх
   `BEST_SKEW_BPS`) ОБЯЗАНА эмитить `Sys`; персистентный ОБЪЁМНЫЙ сдвиг (даже ≫`ε_max`) в рантайме
   МОЛЧИТ (объём REST-неверифицируем → офлайн-трек); seed-gate: пустая своя книга до первого снапшота
   молчит. `ReconThresholds` fail-closed (`ε_prod ≤ ε_max`) сохранён как конструктор-инвариант. (OPS-I-1)
2. **OPS-I-9 rate-budget:** инъецированный поток REST-ошибок НЕ даёт hot-loop; 418/429/`Retry-After`
   соблюдаются; бюджет на venue не превышается (recon добавляет ровно тот трафик, что нас банил).
3. **restore-drill:** холодная копия СКАЧИВАЕТСЯ и ЧИТАЕТСЯ на РЕАЛЬНОМ сегменте, `seq` непрерывен.
4. **Двусторонний паритет алертов:** каждая строка матрицы §7.1 → правило; каждое правило →
   существующая метрика (OPS-I-5, CI в обе стороны).

## Гейты

- **Plan-time:** critic (новый крейт §1.4 + T1 §1.1). FA-приёмка ПЕРВОЙ.
- **T1:** CT-RFC-03 atomic contract-RFC (`05-contract-layer.md` §4).
- **§8:** прод не инертен — recon шлёт REST и пишет `Sys` при инъекции порчи; `/metrics` отдаёт;
  restore-drill на реальном сегменте.
- Recon MD-only (только чтение REST, без order-egress) → risk-critic НЕ требуется (gates §5
  MD-only carve-out); reviewer подтверждает отсутствие order-пути.

## Зависимости и границы

- **Storage Box (founder ★)** — общая с хвостом 2 M-08: cold-copy/restore-drill (задача 3) и
  retention-apply M-08 используют один смонтированный `/mnt/journal-cold`. Задача 3 не стартует
  без него; задачи 1/2/4 — не зависят.
- **HL-глубина:** recon для Binance full-book осмыслен; HL книга ≤20 уровней (TD-005) — recon
  дальних полос там невычислим. Recon-покрытие HL — по развилке founder'а (HL-depth), не блокер M-09.

## Handoff (план)

FA-приёмка (critic → reviewer → founder ★) → CT-RFC-03 (architect + critic) → RED-оракулы
(architect по OPS-I-*) → dev (venue-dev REST-fetch + engine-dev ops-компаратор/метрики/бэкап; research-dev data-quality) → tester → reviewer §8.
