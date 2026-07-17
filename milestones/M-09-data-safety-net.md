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
- `deploy/**` (бэкап cold-copy + restore-drill + `/metrics` scrape) — **engine-dev**
  (деплой-механика, scope-guard carve-out; не секреты).
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
| 2 | 🚧 | **(RED написан: `crates/ops` скелет + 14 оракулов; impl → venue-dev/engine-dev)** **Recon (OPS-I-1 + OPS-I-9):** периодический REST-снапшот vs локальная книга; расхождение > `ε` → алерт + ресинк + `Sys`-событие; **rate-budget/backoff** (honor 418/429/`Retry-After`, cap, запрет ресинк-штормов — прямой урок TD-013) | venue-dev (REST) + engine-dev (ops) | RED: (а) `ε_test` — инъецированная порча книги (удалённый/искажённый уровень, расхождение best) ОБЯЗАНА поднять алерт; (б) OPS-I-9 — инъецированный поток REST-ошибок НЕ даёт hot-loop |
| 3 | ⏳ | **Сохранность (OPS-I-2/3):** cold-copy журнала offsite (Storage Box) + **restore-drill** (скачать→прочитать `journal::stream`, `seq` непрерывен) — на РЕАЛЬНОМ сегменте, legacy-0 первым | engine-dev | RED/§8: restore-drill на реальном сегменте (не фикстуре); удаление горячей копии — только через `ColdCopyProof` |
| 4 | ⏳ | **Метрики+алерты (OPS-I-4..8):** `/metrics` (Prometheus text; recorder+venue+journal) + правила P0/P1/P2; **двусторонний паритет OPS-I-5** (каждый класс §7.1 → ≥1 правило; каждое правило → существующая метрика; CI в ОБЕ стороны); тишина потока (OPS-I-8); метрики НЕ в журнал (OPS-I-6), не в горячем пути (OPS-I-7) | engine-dev | RED: grep-канарейка на каждую метрику §3; CI-скрипт паритета валит и «правило без метрики», и «класс без правила» |
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
- **engine-dev рервайрит `crates/recorder/src/main.rs`** recon-loop: `ReconDetector::new(thr)` ДО
  `while let Some(reference)`, передать `&mut detector` первым аргументом `handle_recon_snapshot`
  (сигнатура sink изменена — old `thr`-параметр убран). 3-строчная механическая правка.
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

## Acceptance (исполняемые ворота — BACKLOG M-09 + OPS-I-*)

1. **`ε_test` не калибруется:** инъецированная порча книги ОБЯЗАНА поднять алерт; `ε_prod`
   калибруется отдельно, но не выше `ε_max` (fail-closed потолок). (OPS-I-1)
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
