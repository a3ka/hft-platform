# TECH-DEBT — открытый долг

> **Reviewer-owned.** Открытые долги/риски, замеченные при работе. Закрытые переносятся вниз.

## OPEN
- **TD-001** recorder Docker-образ работает root'ом (M-00 заглушка). Hardening (non-root +
  права journal-тома) — при реальном recorder (M-01). Severity: MINOR.
- **TD-002** `hetzner-server` приватный ключ был вставлен в чат (скомпрометирован). Пересоздать
  на лэптопе + заменить на VPS при случае. Severity: MINOR (доступ и так только founder+ключи).
- **TD-003** `[verify-at-impl]` по Hyperliquid и Binance (rate-лимиты, подпись действий для
  ордеров) — уточнить при реализации order-стороны. Severity: NOTE.
- **TD-004** Binance L2 сейчас `@depth20@100ms` (частичный снапшот, топ-20). Для OBI-сигнала
  (полосы 3%/8%) нужна бОльшая глубина → полноценный snapshot+diff-sync (recon §A/§D). Severity: NOTE (следующая фаза).
- **TD-005** HL `l2Book` даёт снапшоты по изменению книги (наблюдалось ~реже Binance). Проверить
  полноту cadence; при нужде добавить `bbo`. Funding/liquidations пока не подписаны. Severity: NOTE.
- **TD-006** Журнал — один сегмент без ротации/ретеншена/cold-выгрузки (docs/06). Пока места вдоволь
  (150GB). Добавить сегмент-ротацию + retention→Storage Box когда объём вырастет. Severity: NOTE.
- **TD-007** DET-I-1 (бит-идентичный replay + state_hash) реализован частично (seq+read_all).
  Полный snapshot/state_hash — следующая фаза journal. Severity: NOTE.
- **TD-008** `t1-report-forms-promotion` (M-04). Rust-типы T1-форм `TrialRecord`/
  `ValidationReport` временно живут в `crates/research-cli/src/types.rs` со статусом
  «T1-designate» (per docs/fa/research-cli.md §N amendment 2026-07-10 + critic C-001 M1).
  Единственный продюсер/консюмер сейчас — research-cli; JSON несёт `report_schema_version`.
  Промоушен в `crates/contracts` + генерация JSON Schema (CT-I-4) — отдельным contract-RFC
  при появлении первого кросс-языкового консюмера (Python-тулинг). Severity: NOTE.
- **TD-009** `obi-track-a-report-pending` (M-04 задача 8, ОТКРЫТА). Прогон OBI Трек A/B →
  `research/reports/R-001*` гейтится накоплением данных полной книги (VPS пишет с 2026-07-10),
  вердиктом risk-critic (анти-оверфит чек-лист gates.md §6) и подписью founder ★. Merge
  M-04-кода risk/oms/venues/contracts не трогал — risk-critic обязателен на ОТЧЁТЕ, не на
  этом merge. Также см. TD-004 (Binance @depth20 недостаточен для полос 3%/8% — нужен
  full-book snapshot+diff). Severity: NOTE (гейт пути к деньгам, не долг кода).
- **TD-010** `binance-rest-depth-limit-5000-undercount` (M-05 task 5 / B1, venue-dev, ОТКРЫТА).
  Заведено по флагу founder'а от venue-dev: REST-resnapshot глубины Binance ограничен
  `limit=5000` уровнями на один вызов — дальние полосы книги за пределами топ-5000 одним
  снапшотом не покрываются, а reconcile против diff-книги ограничен этим потолком. Прямое
  следствие для anti-phantom eviction (B1): в самых дальних полосах устаревшие лимитки могут
  не эвиктиться из-за неполноты reference-снапшота. Точный масштаб undercount + стратегия
  (пагинация vs принятие потолка с явной границей достоверности полос) — за venue-dev при
  посадке task 5/B1; на этом merge (engine-dev part) код venue не трогался. Связано с TD-004.
  Severity: NOTE (граница достоверности данных дальних полос, не риск ордер-пути).
- **TD-012** `binance-futures-rest-depth-limit-1000-undercount` (M-06 venue-dev, ОТКРЫТА).
  Аналог TD-010/TD-004 для USDT-M перп: REST depth-снапшот `/fapi/v1/depth` futures ограничен
  `limit=1000` уровнями на вызов — дальние полосы книги за топ-1000 одним снапшотом не покрываются,
  reconcile diff-книги ограничен этим потолком. `FuturesDepthBook.apply_snapshot` (REPLACE, INV-N2)
  корректно эвиктит stale в пределах снапшота, но за границей top-1000 reference неполон. Также
  открытый вопрос `!markPrice@arr` update-rate (согласовать с research-dev, если важна cadence
  funding-breadth). Точный масштаб undercount + стратегия (пагинация vs явная граница достоверности
  полос) — за venue-dev/architect при углублении deep-book. Класс TD-004. Severity: NOTE (граница
  достоверности данных, не риск ордер-пути; MD-only).

- **TD-013** `binance-futures-rest-resnapshot-no-backoff-418-ban` (M-06 venue-binance-futures,
  **ФИКС MERGED inert 2026-07-12, closes при §8 реленда #4**). **Прод-регрессия, поймана §8 eyes-on** (класс TD-011:
  зелёные юниты + Deploy-success замаскировали). При wire BinanceFutures в recorder (#4,
  `2eee4bf`) futures-адаптер на ЖИВОМ прод-трафике попал в hot-loop REST-ресинка: депт-книга не
  бутстрапится, `fetch_snapshot` (`/fapi/v1/depth?limit=1000`) отдаёт **HTTP 418 "I'm a teapot"**
  (Binance IP rate-limit ban), и код НЕМЕДЛЕННО (без backoff) реквестит снова. Замер на VPS:
  **133 × 418 за 25s (~5 req/s), 0 успешных снапшотов, книга не собралась**. Петля
  само-поддерживает бан (продолжающиеся реквесты во время бана сбрасывают его таймер) и
  абьюзит биржу с IP, ОБЩЕГО со спот-пайплайном (`venue-binance`) — риск коллатерального бана
  рабочего спот-сбора. **Корень (reviewer описал, architect проектирует фикс — gates.md §4):**
  `crates/venue-binance-futures/src/lib.rs:596-600` (snapshot fetch failed → `pending_snapshots.push(make_snapshot_future(...))`
  без задержки) и `:613-620` (snapshot stale → тот же немедленный refetch). Нет exp-backoff,
  нет honoring `Retry-After`/429/418, нет cap на частоту REST. **Нужен RED-оракул (architect):**
  ресинк-путь при повторной ошибке снапшота ОБЯЗАН backoff'ить (exp + jitter, honor 418/429
  cooldown), не hot-loop'ить; анти-плацебо на наивной немедленной-retry реализации.
  Затем venue-dev impl → engine-dev релендит #4 (тривиально: re-apply `2eee4bf`). **#4 РЕВЕРТНУТ**
  (`6ddf810`+`6de58e8`), main = tree(`3f38ab0`) inert, прод inert-safe re-verified (418=0,
  CPU 0.99%, seg растёт, hb свежий). Связано с TD-012 (тот же limit=1000, но это completeness;
  TD-013 — корректность/rate ресинка). Severity: MAJOR (была live прод-регрессия + exchange-abuse;
  сейчас блокирует реленд #4).
  **ФИКС (MERGED inert `cc4f529`, reviewer APPROVED 2026-07-12):** architect RED `449bb38`
  (`tests/red_backoff.rs`) → venue-dev `cc4f529` — чистая политика `pub struct Backoff`
  (`next_delay(Option<Retry-After>)`: BASE 100ms, exp ×2, cap 5мин, honor cooldown; `reset()` на
  success), wire'нута в `handle_snapshot`: на fail/stale → `next_delay` → **реальный
  `tokio::time::sleep(delay).await` внутри `make_snapshot_future` ПЕРЕД `fetch_snapshot`**; на
  success → `reset()`. `fetch_snapshot` мапит 418→120s/429→10s cooldown (или `Retry-After` header)
  ДО `error_for_status` → hot-loop рвётся на первом 418. **Reviewer-верификация анти-плацебо (RED
  тестит только политику, НЕ await):** код-рид подтвердил РЕАЛЬНЫЙ sleep в I/O-future (не
  сконструированный-и-забытый Backoff); sleep суспендит только futures символа, не runner. Все
  тесты + workspace GREEN, fmt/clippy clean. **ОСТАЁТСЯ:** (1) реленд #4 (engine-dev, re-apply
  `2eee4bf`) → ПОЛНЫЙ §8 eyes-on LIVE-проверка (418-backoff реально работает: cooldown-sleeps,
  книга бутстрапится, futures L2Snapshot в журнал) — ТОЛЬКО тогда TD-013 CLOSED; (2) **RN-10
  (jitter, NOTE):** джиттер decorrel'ации hammering'а НЕ добавлен — спека RED-оракула его не требует
  (политика детерминирована, джиттер = забота I/O-caller). При 2 символах + 418→120s cooldown
  риск синхронного hammering'а низкий. Если нужен ±jitter — потребует rand/fastrand в
  venue-binance-futures `[dependencies]` (own-crate, formally allowed) + покрытие; отдельная
  мелкая задача, не блокер реленда.
  **LIVE RELAND RESULT (`8b26d6c`, 2026-07-12):** anti-hot-loop часть TD-013 прошла §8: при 418
  recorder логировал cooldown/retry-after sleeps с интервалами ~50-60s на BTCUSDT/ETHUSDT, а не
  прошлые 133×418/25s; CPU/MEM нормальные, restarts=0. Но полный #4 §8 NOT GREEN из-за нового
  blocker'а TD-014 (нет live L2Snapshot/Funding), поэтому milestone close-out не достигнут.

- **TD-014** `binance-futures-live-depth-funding-not-emitted-after-backoff` (M-06 #4 reland,
  **ОТКРЫТА / BLOCKING #4**). После фикса TD-013 reland `8b26d6c` прошёл code-review, локальные
  gates (`red_futures_wired`, fmt, clippy, workspace tests, `verify_M-06.sh` PASS) и GitHub
  CI+Deploy, но §8 eyes-on на VPS НЕ прошёл продуктовый критерий recorder wire. Наблюдения:
  3 `venue connect` строки были (`binance`, `hyperliquid`, `binance_futures`), journal рос, seq
  непрерывен (`seq_gaps=0` на tail-inspection), heartbeat свежий, restarts=0, TD-013 backoff
  live-работал. Однако в 20 MiB / 115 MiB live journal tails были только `BinanceFutures`
  OpenInterest + ConnUp; **0 `BinanceFutures` L2Snapshot и 0 Funding**, при частых
  `venue-binance-futures: depth continuity gap detected, resyncing book` и `snapshot stale vs
  buffered diffs, refetching with backoff`. Liquidation может быть редким событием, но Funding из
  `!markPrice@arr` rare-event'ом не является, поэтому отсутствие Funding блокирует reland.
  Реверт выполнен (`e6b4a75` + `d819cc3`); prod inert-safe re-verified (VPS HEAD `d819cc3`,
  spot+HL only, futures/418=0, hb age 8s, segment +60KB/5s, CPU/MEM нормальные, restarts=0).
  **Нужен architect RED/live-equivalent oracle:** futures runner обязан, при mock/controlled fstream
  depth + markPrice + REST snapshot/backoff сценарии, стабильно эмитить L2Snapshot и Funding после
  resync/backoff, без hot-loop и без starvation markPrice path. Затем venue-dev fix → engine-dev
  reland #4 → reviewer full §8. Severity: MAJOR (prod behavior blocker, no order-path impact).
  **LIVE RELAND-2 RESULT (`af7725f` over `595fc24`, 2026-07-12):** TD-014 fix attempt added
  `FuturesSession` seam + `run()` delegation and local `red_live_emit` passed; reviewer static check
  confirmed live path delegates WS text / snapshot result / tick through the seam (no obvious parallel
  untested runner path). Local gates all GREEN: `red_futures_wired`, `venue-binance-futures` 7/7,
  workspace tests, fmt/clippy, `verify_M-06.sh` PASS exit=0. Pre-merge §8 on VPS still NOT GREEN:
  journal tail since deploy had `BinanceFutures` ConnUp and OpenInterest=16 with `seq_gaps=0`, but
  **0 `BinanceFutures.L2Snapshot` and 0 `BinanceFutures.Funding`**; logs showed repeated
  `depth continuity gap detected`, `snapshot stale vs buffered diffs`, and 429 backoff. Candidate was
  not merged; VPS restored to `origin/main` `2bbcbd7` and rechecked healthy, spot+HL only. Current
  RED/live oracle missed this production mode; TD-014 remains OPEN/BLOCKING.
  **LIVE TD-014 v2 RESULT (`fac7c07` over `71255c5`, 2026-07-12):** stronger local lifecycle
  oracle passed and reviewer confirmed the code path is still MD-only and recorder wiring is real.
  Local gates all GREEN: `red_futures_wired`, `venue-binance-futures` 7/7, workspace tests,
  fmt/clippy, `verify_M-06.sh` PASS exit=0. Pre-merge §8 on VPS still NOT GREEN: journal tail since
  deploy had `BinanceFutures.L2Snapshot=16`, `OpenInterest=16`, `seq_gaps=0`, but
  **`BinanceFutures.Funding=0`**; L2 was sparse rather than expected ~1/s/symbol. Logs during the
  live window showed ongoing churn (`depth continuity gap` 311, `snapshot stale` 44, `429` 18);
  initial CPU reached 6.99% before settling near 1.2%. Candidate was not merged; VPS restored to
  `origin/main` `3eff0db` and rechecked healthy, spot+HL only. TD-014 remains OPEN/BLOCKING.

## Замечания reviewer'а M-06 #4 (2026-07-11)
- **RN-9** (§8 eyes-on поймал то, что все зелёные гейты пропустили — снова) Code-review A+B
  #4 PASS: wiring engine-dev'а КОРРЕКТЕН (default_venues loop, `Box<dyn Fn>` type-erasure,
  supervise() неизменён, MD-only, boundary чист, fmt/clippy/workspace-test/verify_M-06 все
  GREEN на worktree). Дефект НЕ в #4-wiring, а в уже-смерженном (инертном) `venue-binance-futures`
  (venue-dev), который #4 сделал LIVE. Урок TD-011 подтверждён третий раз: «Deploy success» ≠
  «прод работает»; юнит-тесты futures-адаптера (фикстуры, offline) не могли поймать реакцию на
  реальный Binance rate-limit. **Wiring #4 сам по себе безупречен** — при фиксе TD-013 реленд
  тривиален. Для architect: RED-оракул фьючерс-адаптера должен включать симуляцию 418/429-ответа
  REST (прод-масштаб дисциплина `.claude/rules/testing.md`), не только happy-path парсинг.

## Замечания reviewer'а M-06 #4 reland (2026-07-12)
- **RN-11** (§8 split-result) Reland `8b26d6c` доказал, что TD-013 backoff больше не hot-loop'ит
  Binance 418, но одновременно показал новый live blocker: после backoff futures depth/funding
  не доходят до journal. Урок: RED #4 `default_venues` wiring достаточен для engine-dev contract,
  но не покрывает venue-runner liveness. Следующий RED должен быть не только "recorder wires
  BinanceFutures", а "futures runner under resync/backoff emits depth+funding".
- **RN-12** (§8 reland-2 oracle miss) `red_live_emit` + `FuturesSession` seam closed an obvious
  anti-placebo gap, but still did not model the live Binance sequence that keeps the adapter in
  gap/stale/backoff with 429 and no L2/Funding emission. Static delegation proof is necessary but
  insufficient; next chain must make the liveness oracle reproduce this prod failure mode before
  another #4 reland.
- **RN-13** (§8 TD-014 v2 miss) `71255c5` strengthened the lifecycle oracle enough to make L2
  nonzero in live, but not enough to satisfy product behavior: Funding stayed at 0, L2 cadence was
  sparse, and the runner continued gap/stale/429 churn. Next oracle must cover the actual persisted
  cadence and funding path under this churn, not only a deterministic recovery-snapshot unit path.

## Замечания reviewer'а M-05 (не блокирующие, 2026-07-11)
- **RN-8** (fmt-гейт под-покрытие) `verify_M-05.sh` fmt-гейт проверяет только `journal+book`, не
  `recorder` — из-за чего v2 recorder-файлы без trailing newline (`cargo fmt --all --check` FAIL)
  прошли verify зелёным. Поймано reviewer'ом вручную (`cargo fmt --all`), engine-dev пофиксил
  (`7db4479`). → architect: расширить fmt-гейт verify_M-05.sh на recorder. Также урок: verify-скрипт
  milestone'а обязан fmt-check ВСЕ тронутые крейты, не подмножество.
- **RN-4** (AUDIT sacred-файла) engine-dev правил `scripts/verify_M-05.sh` (architect/tester-owned,
  SACRED per scope-guard) в коммите `2a21b8c` (task #4). Правка УЗКАЯ: замена placeholder
  `echo PENDING J1 + FAIL++` на реальный прогон `run "J1 …" cargo test -p recorder --test
  red_shutdown_j1` — оракул J1 стал доступен после task #2. Reviewer подтверждает допустимость:
  (а) явная авторизация founder'а на эту J1-строку; (б) правка НЕ ослабляет гейт — конвертирует
  форсированный FAIL в честный тест-прогон; (в) сверено построчно — J2/J3/B1/fmt/clippy-строки и
  FAIL-агрегатор не тронуты. РЕВЕРТ НЕ ТРЕБУЕТСЯ. На будущее: wiring sacred-скрипта — отдельный
  коммит tester/architect (паттерн M-06 task 6), не бандл в feature-коммит dev'а.
- **RN-5** (partial-merge, founder-authorized) engine-dev part M-05 (tasks 2/3/4) смержен в `main`
  ДО полного close-out milestone'а. `verify_M-05.sh` → `VERDICT: FAIL (1)`, и единственный FAIL —
  `B1 resnapshot anti-phantom` (venue-dev task 5) PENDING, ортогональный к journal/recorder-фиксу.
  Push разрешён явным founder-override правила auto-push-only-on-exit-0 (B1 не в зоне engine-dev,
  фикс journal-integrity прод-критичен). Milestone остаётся IN_PROGRESS до B1 (task 5) + wiring
  task 6 (verify exit 0). НЕ close-out. **⚠ ОТКАЧЕНО через ~4 мин — прод-регрессия, см. TD-011.**
  Урок: eyes-on §8 ssh-проверка ОБЯЗАТЕЛЬНА и поймала то, что зелёный CI/юнит-тесты/Deploy-success
  пропустили; «Deploy success» ≠ «прод пишет данные».
- **RN-6** (DET-I-1 подтверждение) `read_all` остался STRICT (Err на первом CRC-mismatch +
  postcard-decode→Err — сверено на `b22583c`); resync-толерантность вынесена в ОТДЕЛЬНУЮ
  `recover()` (честный побайтовый ресинк, без rand/wall-clock). DET-I-1 exact-replay НЕ ослаблен.
  `next_seq = meta.max(seg-scan)` — источник истины сегмент, reuse исключён (мета-lag не даёт
  отката; мета-ahead даёт gap, не reuse — оба безопасны для монотонности).

## Замечания reviewer'а M-04 (не блокирующие, 2026-07-10)
- **RN-1** (NOTE) `verify_M-04.sh` T6 объединяет `contracts+journal+book` в один `check` —
  провал любого из трёх не различается по строке. Приемлемо для регресс-гейта (все GREEN),
  но при росте числа крейтов стоит разнести на per-crate строки для точной диагностики.
- **RN-2** (NOTE) Латентность δ_md — эмпирика из журнала, но δ_submit/δ_cancel — measured WS
  RTT ×2 (пессимизм-прокси, НЕ реальный order-path замер: P1 order-path ещё нет, D7 это
  честно фиксирует в provenance). Честность δ_submit/δ_cancel обязана быть предметом
  risk-critic на отчёте R-001 (чувствительность ×2 латентности per gates.md §6.4) — уже
  учтено дизайном стресс-вариантов, отмечаю для явной проверки на задаче 8.

## Замечания reviewer'а (фикс ts_exch_ms=0 у L2Snapshot, 2026-07-11)
- **RN-3** (NOTE) В фикс-коммите `1477bca` sacred inline-модуль `ts_exch_tests`
  (architect-owned) получил rustfmt-переносы (multi-line `assert_eq!`/let-else/let-binding).
  Сверено построчно: семантика тестов идентична (те же литералы 1_752_000_000_123 / 777_000 /
  1_600_000_000_000, те же ассерты и сообщения, та же структура). Переформатирование
  ВЫНУЖДЕНО гейтом `verify_M-04.sh` T1a (`cargo fmt --check`) — architect закоммитил RED-тесты
  с строками >100 col (допустимо: compile-RED всё равно не собирается), а GREEN обязан пройти
  fmt-гейт. Приемлемо (whitespace-only, semantics-preserving); отмечено для аудита касания
  sacred-файла dev-агентом.

## CLOSED
- **TD-011** `scan_next_seq-full-segment-read-oom` (M-05 task#3) — **RESOLVED 2026-07-11**.
  Инцидент: v1 `Journal::open()` делал `read_to_end` ВСЕГО сегмента (прод 2.65 GiB) в RAM на каждом
  старте → recorder не писал (101% CPU, 2.48 GiB RAM, OOM-риск); юнит-RED на крошечных фикстурах не
  поймал; healthcheck обманут; поймано eyes-on §8. Откачено (`c2ad02c`/`ffdc410`/`e190356`).
  ФИКС (v2, `a356c81`): `scan_tail_for_last_seq` — читает последние ≤4 MiB (seek+read_exact),
  `next_seq = max(meta, tail+1)`, O(1) память. Верификация: (а) architect RED-оракул
  `red_open_bounded.rs` (64 MiB + counting-allocator, бюджет 8 MiB) GREEN; (б) reviewer НЕЗАВИСИМЫЙ
  прод-масштаб харнес (2.94 GiB): open()=4 ms, max RSS 6 MiB, next_seq корректен; (в) eyes-on §8 на
  VPS после merge/deploy: новый recorder пишет (CPU 0.53%, MEM 5.41 MiB, tail-scan реального 2.71 GiB
  прод-сегмента → `next_seq=3467845`, сегмент растёт). Урок закреплён в `.claude/rules/testing.md`
  (прод-масштаб RED для sacred I/O) + RN-8 (fmt-гейт под-покрытие).
