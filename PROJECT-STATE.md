# PROJECT-STATE — что реализовано

> **Reviewer-owned.** Обновляется ТОЛЬКО reviewer'ом после merge (scope-guard). Отражает
> фактическое состояние `main`, не планы (планы — `docs/DESIGN.md §10` роадмап).

## Инфраструктура (готово, проверено 2026-07-10)
- Репо `a3ka/hft-platform` (private, ветка `main`).
- VPS cpx32 (`167.233.192.131`): Ubuntu 26.04, Docker 29.6 + Compose, Rust 1.97; репо
  склонирован в `/root/hft-platform` (read-only deploy-key).
- CI/CD: `.github/workflows/ci.yml` (fmt+clippy+test+audit) + `deploy.yml` (build-on-VPS,
  `git push main` → SSH → `docker compose up --build` → healthcheck → rollback). **Проверено
  сквозным деплоем: recorder-заглушка Up/healthy, journal-том persistent.**

## Процессный слой (M-00 — готово)
- `CLAUDE.md` + `.claude/rules/` (5) + `.claude/agents/` (9) — EINHARD-модель под трейдинг.
- `PROJECT-STATE.md` + `TECH-DEBT.md` (reviewer-owned).

### Doc-гейт + protected-artifacts барьер (C-006 — CLOSED, MERGED 2026-07-15, reviewer APPROVED)
Цикл C-006 завершён (founder-решение: принять после P1/P17, без новых витков критика). Смержены
ДВЕ ветки (порядок td021-rules → doc-gate): `af33d61` (td021-rules) + `0d5f8f8` (doc-gate), полная
история rev3–rev12 (architect fix ⇄ critic REJECT) сохранена. Reviewer резолвил конфликты:
PROJECT-STATE/TECH-DEBT → в пользу main (надмножество); `.claude/rules/testing.md` → ОБЕ секции
(TD-021 «оракул мерит то, что обещает» + «Целостность гейта — 4 свойства»); `ci.yml` → ОБА job'а
(delivery + protected-artifacts, `status-check.needs` = все четыре).
- **`scripts/check_protected_artifacts.sh`** — барьер: коммит/мерж не смеет удалить/подменить/усечь
  вердикт критика (`research/critiques/`), milestone, RFC. **База сравнения из СОБЫТИЯ** (не
  `origin/main` — иначе диапазон пуст и гейт зелён всегда, блокер B1); пустая/zero/переписанная база
  → **fail-closed**. Ловит: удаление, rename-out, evil-merge, merge-born-then-dropped, подмену типа
  (каталог/симлинк), усечение в 0 байт.
- **`scripts/tests/red_protected_artifacts.sh`** — проба барьера ТОЙ ЖЕ проводкой, что CI (17
  сценариев). **Анти-плацебо доказан reviewer'ом независимо:** rev8-барьер → FAIL(3) P14/P15/P16
  (подмена типа/усечение); guard-мутация (P7 merge→true, P17 echo→true) → «SETUP НЕ СОСТОЯЛСЯ», не
  ложный PASS. На merged-дереве VERDICT: PASS (17/17).
- **CI job `protected-artifacts`** (`ci.yml`) — base-from-event, fail-closed, в `status-check.needs`.
- **Мета-правило `.claude/rules/testing.md` «Целостность гейта — 4 свойства»**: гейт обязан (1)
  гонять прод-форму, (2) мерить свой инвариант не окружение, (3) падать против слома И несостоявшегося
  setup, (4) наблюдать ОТСУТСТВИЕ не только сбой. Итог ~10 дефектов серии за сессию (D8/D9 — эталон).
- **Открытый пункт (founder ★):** барьер force-push ДЕТЕКТИРУЕТ (fail-closed на zero/переписанную
  базу), но не ПРЕДОТВРАЩАЕТ — закрывается branch protection «no force-push» на `main` (GitHub-настройка).

## Даталеер / поток данных (M-01 — РАБОТАЕТ, проверено на VPS 2026-07-10)
- `crates/contracts` — T1 `Event`/`EventKind::Md(MdEvent)`: Trade/L2Snapshot/Funding,
  fixed-point i64 ×1e8, Venue/Side/Level. Тесты: 2 GREEN.
- `crates/journal` — append-only (postcard+crc32 фреймы), монотонный seq персистится
  через рестарт, `read_all` replay. Единственный писатель. Тесты: 2 GREEN.
- `crates/venue-binance` — spot combined-stream `@trade` + `@depth20@100ms` → MdEvent.
- `crates/venue-hyperliquid` — WS `trades` + `l2Book` (уровни-объекты {px,sz,n}), ping-keepalive.
- `crates/recorder` — venue-supervisor (reconnect+backoff) → mpsc(EventKind) → журнал + heartbeat.
- **Проверено в проде (VPS):** Binance + Hyperliquid оба пишутся в персистентный журнал,
  реальные цены/стакан, seq монотонный, контейнер healthy, автодеплой работает.

## Journal integrity (M-05 — engine-dev part MERGED + прод-верифицирован 2026-07-11; milestone IN_PROGRESS)
Tasks 2/3/4 (engine-dev) НА main (`a356c81`/`e8c3540`/`7db4479`, push `7db4479`), founder ★-authorized
partial-merge (RN-5; B1 остаётся PENDING). Прошёл полный цикл: v1 откатан из-за TD-011 (full-segment
`read_to_end` в `open()` → recorder не писал, 101% CPU/2.48 GiB); v2 — **ХВОСТОВОЙ tail-scan O(1)
памяти**; reviewer НЕЗАВИСИМО перепроверил §8 на прод-масштабе (2.94 GiB синт-сегмент: open()=4 ms,
max RSS 6 MiB, next_seq корректен) ДО merge, и eyes-on на VPS после deploy: **новый recorder пишет**
(CPU 0.53%, MEM 5.41 MiB, сегмент растёт, `journal progress next_seq=3467845` — tail-scan реального
2.71 GiB прод-сегмента отработал за ~секунды). TD-011 **RESOLVED**.
- `crates/recorder` — `run_writer` select-seam в lib (юнит-тестируемый J1); `main` враппит SIGTERM/SIGINT
  → ветка `shutdown` дрейнит буфер (`try_recv`) + `flush()` перед exit. Heartbeat wall-clock — в отдельный
  `.heartbeat` файл, НЕ в journal-payload (детерминизм журнала сохранён).
- `crates/journal` — `next_seq` при `open()` из `scan_tail_for_last_seq` (читает последние ≤4 MiB
  сегмента, seek+read_exact, buf освобождается до write-open) = `max(meta, tail_seq+1)`; O(1) память,
  нет reuse (J2/TD-011 GREEN на прод-масштабе). `recover()` — resync-толерантное чтение (offline CLI,
  НЕ в горячем `open()`; полный read_to_end допустим только offline). `read_all` STRICT (Err на
  CRC-mismatch) — DET-I-1 exact-replay не ослаблен.
- **Урок (зафиксирован в `.claude/rules/testing.md`):** RED-оракул sacred I/O-пути ОБЯЗАН включать
  прод-масштаб (арх-оракул `red_open_bounded.rs` — 64 MiB + counting-allocator бюджет 8 MiB); зелёные
  юнит-тесты + Deploy-success ≠ рабочий прод — eyes-on §8 решающий. См. TD-011 (CLOSED), RN-4..8.
- **M-05 остаётся IN_PROGRESS:** task 5 B1 (venue-dev, anti-phantom resnapshot) PENDING → `verify_M-05.sh`
  exit=1 (только B1); task 6 (tester, verify exit 0) после B1. TD-010 (REST limit=5000) открыт.

## Data expansion (M-06 — #4 reland MERGED, reviewer APPROVED 2026-07-13; close-out pending)
Смержены ДВЕ инертные (не потребляются recorder'ом до #4 poller → прод-поведение НЕ изменено)
APPROVED-ветки; main стал полностью GREEN (впервые за цикл RED-on-main). §8 eyes-on после deploy:
recorder БЕЗ изменений (CPU 0.79%, MEM 5.6 MiB, сегмент растёт +261 KB/12s, next_seq растёт, restarts=0).
- `crates/venue-binance-futures` (venue-dev, tasks #2/#3 + N2/N3) — USDT-M перп fstream-адаптер:
  парсеры `@depth@100ms`→L2Snapshot, `@forceOrder`→Liquidation (side = ликвидируемая сторона, C2),
  `/fapi/v1/openInterest`→OpenInterest (C3); `parse_mark_price` (`markPriceUpdate`→Funding, знак, N3);
  `FuturesDepthBook.apply_snapshot` = REPLACE-семантика (INV-N2: gap-ресинк эвиктит stale дальние
  уровни → анти-фантомная ликвидность). 5/5 RED GREEN, MD-only (ордер-путь не тронут → risk-critic
  не нужен, gates.md §5 N4 carve-out). НЕ потребляется recorder'ом (нет в его deps).
- `crates/derive::funding_breadth` (research-dev, task #5) — чистый детерминированный агрегат
  funding-breadth (%+/−, top-N по universe); проходит ХАРДЕНУТЫЙ red_breadth (асимметрия 60/20,
  хардкод-пруф). Потребители — research-cli/signals (downstream, journal-first).
- **#4 recorder-wire BinanceFutures — ПОПРОБОВАН, РЕВЕРТНУТ (§8 eyes-on поймал прод-регрессию,
  2026-07-11).** engine-dev wiring (`2eee4bf`: default_venues loop + `Box<dyn Fn>` type-erasure,
  supervise() неизменён) прошёл code-review A+B (MD-only, boundary чист, fmt/clippy/workspace-test/
  verify_M-06 GREEN) + CI + Deploy success — и БЫЛ смержен. Но §8 eyes-on на VPS показал: живой
  futures-адаптер попал в hot-loop REST-ресинка → **133 × HTTP 418 (Binance IP ban) за 25s, депт-книга
  не бутстрапится, 0 futures L2Snapshot в журнал**, ~5 req/s абьюз биржи с IP, общего со спот-сбором.
  Дефект — в уже-инертном `venue-binance-futures` (no-backoff на snapshot-fail, `lib.rs:596-600`/
  `:613-620`), который #4 сделал LIVE (НЕ в engine-dev wiring — оно корректно). **Реверт**
  (`6ddf810`+`6de58e8`), main = tree(`3f38ab0`), прод re-verified inert-safe (418=0, CPU 0.99%,
  MEM 5.22 MiB, seg растёт +133KB/12s, hb свежий, 0 restarts). Заведён **TD-013 (BLOCKING #4,
  MAJOR)**. Реленд #4 — после фикса TD-013 (architect RED backoff-оракул → venue-dev impl → re-apply
  `2eee4bf`). Урок TD-011 подтверждён 3-й раз (RN-9).
- **TD-013 фикс (Backoff) — MERGED inert, reviewer APPROVED 2026-07-12.** Цепочка реленда:
  architect RED `449bb38` (`tests/red_backoff.rs` — политика `Backoff::next_delay`/`reset`: ≥100ms
  первый ретрай, exp-рост, cap 5мин, honor Retry-After, reset на success) → venue-dev `cc4f529`
  (impl + wiring в `handle_snapshot`). Reviewer подтвердил **анти-плацебо WIRING** (ключевой риск:
  RED тестит ТОЛЬКО чистую политику, НЕ I/O-await): `make_snapshot_future(.., Some(delay))` делает
  **РЕАЛЬНЫЙ `tokio::time::sleep(delay).await` ПЕРЕД `fetch_snapshot`** (не сконструированный-но-
  проигнорированный Backoff); `fetch_snapshot` распознаёт 418→120s/429→10s cooldown ДО
  `error_for_status` → hot-loop рвётся на первом 418. sleep суспендит только futures данного символа
  (FuturesUnordered), не runner. red_backoff + red_parse/red_funding/red_resnapshot все GREEN,
  workspace GREEN, fmt/clippy clean. **INERT** (recorder НЕ зависит от venue-binance-futures на
  этом merge — dep реверта #4 отсутствует; Backoff-код недостижим из recorder). §8 inert-safety
  на VPS: recorder БЕЗ изменений (spot+HL only, 0 futures/418, CPU 0.64%, MEM 5.4 MiB, seg
  +98KB/8s, hb ~10s cadence свежий, 0 restarts). Джиттер НЕ добавлен (спека оракула его не требует;
  политика детерминирована) — NOTE в TD-013, не блокер.
- **#4 reland после TD-013 — REJECTED / REVERTED (§8 live NOT GREEN, 2026-07-12).**
  Reland `8b26d6c` (recorder dep `venue-binance-futures`, `default_venues()`, итерационный spawn
  supervisor'ов; `supervise()` не тронут) прошёл локально RED `red_futures_wired` (1 passed),
  fmt/clippy/workspace tests, `verify_M-06.sh` PASS exit=0, GitHub CI + Deploy success. §8 eyes-on
  на VPS подтвердил часть TD-013: **hot-loop 418 НЕ повторился** (rate-limit retries spaced
  ~50-60s / cooldown, не 133×418/25s), CPU/MEM нормальные, restarts=0, heartbeat свежий, journal
  растёт, seq непрерывен. Но продуктовый критерий #4 НЕ выполнен: в live journal-tail были
  `BinanceFutures` OpenInterest + ConnUp, но **0 BinanceFutures L2Snapshot и 0 Funding** (20 MiB и
  115 MiB хвосты), при повторяющихся `depth continuity gap` / `snapshot stale ... backoff` циклах.
  Funding из `!markPrice@arr` не rare-event, поэтому это не §8-GREEN. Реверт `e6b4a75` + `d819cc3`;
  main снова inert-safe: VPS HEAD `d819cc3`, spot+HL only, 0 futures/418, hb age 8s, segment +60KB/5s,
  CPU ~0.7-5%, MEM ~5.8 MiB, restarts=0. Открыт **TD-014 (BLOCKING #4)**.
- **TD-014 fix + #4 RELAND-2 — REJECTED (§8 live NOT GREEN, 2026-07-12).**
  Цепочка `0f924dc` RED `red_live_emit` → `595fc24` FuturesSession seam/run() delegation →
  `3d9c214` RED recorder wiring → `af7725f` engine-dev reland прошла локально:
  `red_futures_wired` PASS, `venue-binance-futures` 7/7 PASS, workspace tests PASS,
  fmt/clippy clean, `verify_M-06.sh` PASS exit=0. Static review подтвердил: `run()` реально
  делегирует WS/snapshot/tick через `FuturesSession`, recorder wiring итерационный,
  `supervise()` не тронут, diff MD-only. Но pre-merge §8 deploy на VPS показал: 3 `venue connect`
  строки есть, `BinanceFutures` ConnUp + OpenInterest пишутся, seq непрерывен (`seq_gaps=0`),
  heartbeat свежий, CPU/MEM нормальные, restarts=0; **при этом live journal-tail с момента deploy:
  0 `BinanceFutures.L2Snapshot`, 0 `BinanceFutures.Funding`**. Логи продолжают цикл
  `depth continuity gap detected` / `snapshot stale vs buffered diffs` / 429 backoff.
  Это НЕ §8-GREEN; branch НЕ смержен. VPS восстановлен на `origin/main` `2bbcbd7`
  (spot+HL only, no futures supervisor, healthy, hb age ~3s).
- **TD-014 v2 + #4 reland `fac7c07` — REJECTED (§8 live NOT GREEN, 2026-07-12).**
  Цепочка `71255c5` strong live-lifecycle RED + `fac7c07` recovery-snapshot T/E fix прошла
  локально: `red_futures_wired` PASS, `venue-binance-futures` 7/7 PASS, workspace tests PASS,
  fmt/clippy clean, `verify_M-06.sh` PASS exit=0; static review подтвердил MD-only и реальное
  recorder wiring. Pre-merge deploy на VPS (`fac7c07`) стартовал 3 venue, был healthy,
  heartbeat свежий, seq непрерывен (`seq_gaps=0`), OI писал. Но §8 journal-tail с deploy:
  `BinanceFutures.L2Snapshot=16`, `OpenInterest=16`, **`Funding=0`**; L2 sparse, не ~1/s/symbol.
  Логи за live-window: `depth continuity gap` 311, `snapshot stale` 44, `429` 18, CPU до 6.99%
  на старте (позже ~1.2%). Это НЕ §8-GREEN; branch НЕ смержен. VPS восстановлен на
  `origin/main` `3eff0db` (spot+HL only, no futures supervisor, healthy, hb age ~4.5s).
- **TD-014 T2 + #4 reland `669ce40` — REJECTED (§8 live NOT GREEN, 2026-07-12).**
  Цепочка `38c3175` RED futures-continuity (`pu`, не spot `U == last+1`) + `669ce40`
  dual-rule fix прошла локально: `red_futures_wired` PASS, `venue-binance-futures` 8/8 PASS,
  workspace tests PASS, fmt/clippy clean, `verify_M-06.sh` PASS exit=0. Static review подтвердил
  MD-only и корректное разделение: steady-state strict `pu == last_update_id`, reconcile-loop
  Binance-style `U <= L+1 && u >= L+1`, `pu` fail-closed. Pre-merge §8 на VPS показал реальный
  прогресс: 3 venue стартовали, recorder healthy, heartbeat свежий, CPU ~1.1%, MEM ~7.5 MiB,
  restarts=0, `seq_gaps=0`, fresh tail с deploy: `BinanceFutures.L2Snapshot=470`,
  `OpenInterest=54`; после стартового окна последние 3 минуты имели `gap=0`, `stale=0`, `429=0`
  (одиночный 418 без hot-loop). Но обязательный live-критерий всё ещё НЕ выполнен:
  **`BinanceFutures.Funding=0`** в 48 MiB journal-tail за несколько минут live-window.
  `!markPrice@arr` не является rare-event, поэтому это не §8-GREEN. Branch НЕ смержен; VPS
  восстановлен на `origin/main` `4012c55` (spot+HL only, healthy, hb age ~6s, CPU 0.58%).
- **TD-014 T3 + #4 reland `99b1329` — REJECTED (§8 live NOT GREEN, 2026-07-13).**
  Цепочка `c747a97` RED per-symbol markPrice + `99b1329` per-symbol `<sym>@markPrice@1s`
  subscription прошла локально: `red_futures_wired` PASS, `venue-binance-futures` 9/9 PASS,
  workspace tests PASS, fmt/clippy clean, `verify_M-06.sh` PASS exit=0. Static review подтвердил:
  runner подписывает per-symbol `@markPrice@1s`, `FuturesSession` поддерживает одиночный
  `markPriceUpdate` и legacy `!markPrice@arr`, diff MD-only. Pre-merge §8 на VPS показал:
  recorder healthy, 3 venue стартовали, heartbeat свежий, CPU ~1.1-1.2%, MEM ~6-7 MiB,
  restarts=0, `seq_gaps=0`, fresh tails с deploy: `BinanceFutures.L2Snapshot=637`,
  `OpenInterest=66`; позднее окно имело `gap=0`, `stale=0`, `429=0`. Но обязательный
  live-критерий всё ещё НЕ выполнен: **`BinanceFutures.Funding=0`** в persisted journal
  после нескольких минут live-window; logs за позднее окно также `markPrice/Funding=0`.
  Branch НЕ смержен; VPS восстановлен на `origin/main` `1d5ecfa` (spot+HL only, healthy,
  futures logs after restore=0).
- **TD-014 T4 + #4 reland `c123bbd` — APPROVED / MERGED (§8 live GREEN, 2026-07-13).**
  Цепочка `d9b3b1c` RED premiumIndex REST funding poll + `c123bbd` venue-dev pivot прошла:
  local reviewer gates GREEN (`red_futures_wired`, `venue-binance-futures` 10/10 including T4,
  workspace tests, fmt, clippy, `verify_M-06.sh` PASS exit=0), remote Docker verify on VPS
  GREEN (`VERDICT: PASS exit=0` after installing rustfmt/clippy components in `rust:1-slim`),
  and §8 live GREEN. VPS candidate `c123bbd`: recorder healthy, 3 venue connect, heartbeat fresh,
  CPU ~1.5%, MEM ~9.5 MiB, restarts=0, late window `418=0`, `429=0`, `gap=0`, `stale=0`.
  Persisted journal since deploy: `seq_gaps=0`, `BinanceFutures.L2Snapshot=465`,
  `OpenInterest=48`, **`Funding=48`**. Merge commit: `1504d8b` (`M-06 reland #4
  (TD-014 v2+T2+T3+T4)`). TD-014 CLOSED.
- **M-06 статус после reviewer:** #1 compile/C1 green, inert venue-futures + derive части на main,
  **#4 recorder-wire BinanceFutures merged and live-green**, #5 funding-breadth green. Milestone
  close-out остаётся за tester/architect chain: tester #6 clean-checkout `verify_M-06.sh` /
  architect close-out docs. Reviewer НЕ трогал milestone status columns.
  Data-quality долг:
  TD-012 (futures REST depth limit=1000 undercount). TD-013 anti-hot-loop live-verified; TD-014
  live funding/depth emission closed by T4.

## Движок бэктеста (M-04 «Research core» — РЕАЛИЗОВАНО, reviewer APPROVED 2026-07-10)
Цепочка: architect → critic C-001 REJECT → фиксы `f02c418` → critic C-002 NOTE (все
находки C-001 закрыты) → dev (2 honest-STOP SVR, оба разрешены architect'ом) → tester
PASS на чистом чекауте → reviewer APPROVED. Тесты: 28 RED→GREEN + канарейки; verify_M-04
15/15 PASS, exit=0 (reviewer перепрогнал независимо).

- `crates/sim` — честный симулятор исполнения: пессимистичная модель fill'а (очередь на
  НАШЕЙ цене, ahead=объём на нашем уровне не на топе; SM-I-5 cancel-ahead = тождество;
  taker ест только видимую книгу); латентность из измеренных таблиц (SplitMix64 PRNG,
  собственный — DET-стабильный); fees/funding fail-closed (нет расписания → startup halt);
  `BacktestExchange` (on_event→SimFill in-memory, T2 — `Ord(...)` в journal откладывается
  до paper/live M-05); divergence gate-checker (P4-gate требует отчёта расхождения).
- `crates/signals` — Граница A: `trait Signal`, `SignalBank` (изоляция паник SG-I-9),
  registry-загрузчик (code_hash = sha256 исходника, D3; retired-skip), `obi.rs` OBI №1
  (TopN + Bands-режимы; направленный score i64 ×1e8; эмиссия только при |score|≥theta).
- `crates/research-cli` — research-платформа: trials-ledger (O_APPEND + hash-chain,
  подмена байта ломает цепочку); split (train/val/test, val-гейт токен); метрики
  (Sharpe/maxDD/fill-rate/turnover/capacity v1/decay + deflated Sharpe по Bailey & López
  de Prado от N из глобального ledger); grid/walk-forward (стресс ×1.5-издержки/×2-латентность);
  детерминированные отчёты; CLI. T1-формы `TrialRecord`/`ValidationReport` временно
  T1-designate в `research-cli/src/types.rs` (промоушен → contracts отложен, см. TD-008).
- `crates/book` — примитивы для sim: `top_n_depth` / `levels` (best-first) / `size_at`
  (carve-out C-001 C1 + SVR-резолюция; поуровневый доступ для taker_fills/ahead-семантики).
- Артефакты честности с `provenance`: `research/latency/*.json` (δ_md эмпирика из журнала +
  измеренный WS RTT VPS→биржа ×2 пессимизм) и `research/fees/*.json` (Binance/HL базовые
  тарифы, скидки намеренно не учтены). sim грузит их (default-задержек в коде нет, SM-I-7/8).
- SignalSpec `research/specs/S-001-obi-asym.md` сверена с H-карточкой пре-регистрации.
- **ОТКРЫТО (задача 8 M-04):** прогон OBI Трек A/B → `research/reports/R-001*` гейтится
  накоплением full-book данных (VPS пишет с 2026-07-10) + вердиктом risk-critic + подписью
  founder ★. Merge кода НЕ трогал risk/oms/venues/contracts, поэтому risk-critic — на отчёте.

## Мозг стратегии (M-07 «Strategy brain» — РЕАЛИЗОВАНО, reviewer APPROVED 2026-07-13)
Закрыта дыра равенства DESIGN §1 №2 (`backtest == paper == live`): решения больше НЕ захардкожены
в ad-hoc harness'е `research-cli/grid.rs` (taker-in по `SignalOut`, taker-out по `horizon_ms`,
фиксированный `qty=1.0`) — бэктест гоняет НАСТОЯЩИЙ код решений, тот же объект, который в P3+
исполнит live-`runner`. Цепочка: architect → critic C-004 (rev1 REJECT → rev2 APPROVE) → engine-dev
(2–5) → research-dev (6) → tester (7) → **reviewer CHANGES REQUESTED (equity-curve дефект)** →
architect rev3 (RED ST-I-8g/8h) → engine-dev (9) → reviewer APPROVED. Merge: ff `5141fd9`.
`verify_M-07.sh` — 21/21 PASS, exit=0 (reviewer перепрогнал независимо на чистом чекауте).

- `crates/alpha` (Слой 2) — `LinearAlpha`: ансамбль `edge = clamp(Σwᵢvᵢ/Σ|wᵢ|, ±1e8)`,
  `horizon = max(horizonᵢ)`, `confidence` = доля живого веса; stale-expiry сэмпла по `horizon_ms`
  (сигнал не живёт вечно, AL-I-4); BTreeMap-детерминизм обхода; веса fail-closed валидируются
  (пусто/ноль/дубль → Err). AL-I-1..5 GREEN.
- `crates/portfolio` (Слой 3) — `RiskBudget` + `size()`: `target = clamp(edge·max_pos/1e8, ±max_pos)`
  на i128 (PF-I-2 держит `i64::MAX`-edge без переполнения); **дефолтного лимита НЕ существует** —
  инструмент без явного лимита → target 0 (fail-closed, анти-`risk_guard` DESIGN §9); позиция без
  форкаста → flatten. PF-I-1..4 GREEN. ⚠ Это pre-trade sanity, **НЕ риск-гейт** — fail-closed
  `RiskApproved<Order>` (RK-I-1..10) приходит в M-08 и встаёт МЕЖДУ `strategy` и `oms`.
- `crates/strategy` (Слой 4) — `DirectionalStrategy`: `Event → signals → alpha → portfolio →
  diff(target vs current) → OrderIntent`; in-flight дедуп с TTL по **event-time** (никакого
  wall-clock, D4); маркетабельная цена (i128); нет видимой книги → интента НЕТ. Структурно не знает,
  кто его исполняет: нет зависимостей на `sim`/`venue-*`/`journal`/`risk`, нет `HashMap`/`rand`/
  `SystemTime` (ST-I-6 грепами + T8a-c). `OrderIntent`/`OrderKind` релоцированы `sim` → `strategy`
  (T2, D1; канарейка ST-I-7/T8d-e: `pub struct OrderIntent` ровно в одном крейте); `sim`
  ре-экспортирует. ST-I-1..7 GREEN.
- `crates/sim::StrategyBacktest` (D3) — harness `run(&[Event], &mut dyn Strategy) -> BacktestReport`.
  Порядок на событии строго: `exchange.on_event` → `strategy.on_fill` по каждому филлу (мост
  `SimFill → FillReport` через `order_meta`) → `strategy.on_event` → `exchange.submit`. Стратегия
  никогда не видит событие раньше биржи и не видит будущего (ST-I-8f: мутация будущего не меняет
  прошлого). ST-I-8a..h GREEN.
- `crates/research-cli` — ad-hoc harness (`OpenPosition`/`Action`) **удалён** (канарейка T9b);
  грид гоняет ячейку через `StrategyBacktest` + `DirectionalStrategy` (T9c/T9d — грепы игнорируют
  комментарии); `strategy_cell` (D7/D8): дефолты блока `strategy`, `cell_params_hash` (покрывает
  strategy+costs), `capital_ref_e8 = max_position·mid₀`, `returns = Δequity/capital_ref`.
  Гейт задачи 6 — **ПОВЕДЕНЧЕСКИЙ** (GR-I-6/7: разный `max_position_e8` обязан дать разный оборот;
  деадбенд шире лимита → ноль интентов — оба валят harness с фиксированным `qty=1.0`). GR-I-1..7 GREEN.
- **Дефект, пойманный reviewer'ом на PR-гейте (не оракулами) — equity-curve.** `StrategyBacktest`
  привязывал точку equity к НАКОПЛЕННОМУ числу филлов (`curve.len() < fills.len()`), а не к «на ЭТОМ
  событии был филл»: событие с 2+ филлами давало 1 точку, а дефицит добирался на ПОСЛЕДУЮЩИХ
  БЕСФИЛЛОВЫХ событиях → фантомные точки → лишние near-zero доходности → **σ занижена → Sharpe
  ЗАВЫШЕН** → `ValidationReport` → trials-ledger → подпись founder'а (gates §6/§7). Достижимо на
  реальных данных (ttl-expiry переотправляет интент, пока taker-ордер ждёт traded-тик → оба филлятся
  на одном тике). Дыра была в RED-suite: `equity_curve` не ассертилась НИГДЕ (GR-I-4 тестировала
  `returns_from_equity` как чистую функцию на рукописном векторе и кривую из `run()` не видела) —
  тот же класс, что C-004 C2, этажом выше. Фикс: `had_new_fill_this_event` (rev3, `5141fd9`).
  Новые sacred-оракулы **ST-I-8g** (кривая сверяется ПОЭЛЕМЕНТНО с независимо пересчитанным MTM:
  значения + моменты + количество; фикстура обязана быть мульти-филловой) и **ST-I-8h** (бесфилловые
  события не добавляют точек) + гейт T5b в `verify_M-07.sh`. **Анти-плацебо доказан reviewer'ом
  независимо:** оба оракула FAIL против пред-фиксной реализации (`left: 2, right: 1` — 2 фантомные
  точки при 1 событии с филлами), остальные 6 ST-I-8a..f остались GREEN (регрессии нет).
- **Прод инертен (T10):** `recorder` не зависит от `alpha`/`portfolio`/`strategy`/`sim` — мозг не
  торгует и не пишет журнал. §8 eyes-on после deploy подтвердил НУЛЕВОЕ изменение поведения recorder'а.

## Data durability (M-08 «сбор не останавливается» + CT-RFC-02 — MERGED + В ПРОДЕ 2026-07-14, reviewer APPROVED; **milestone НЕ закрыт: цель E7/E3 не достигнута**)
Прод: `b7721d1` (merge `1123b13` + фикс TD-018). CI+Deploy success; **§8 eyes-on ВЫПОЛНЕН**
(4.2 ч наблюдения). Прод здоров: `restarts=0`, `panic/ERROR=0`, `backstop=0`, heartbeat свежий.
**Что подтверждено на боевых данных:** старый сегмент 15 188 347 171 B **заморожен** (mtime =
момент деплоя, байт-в-байт цел) → пишется НОВЫЙ `segment-00000001.jrnl` с магией `HFTJRN02`;
`seq` непрерывен через границу (legacy `max=16049333` → new `min=16049334`, `seq_gaps=0`);
**РОТАЦИЯ ПОДТВЕРЖДЕНА ВЖИВУЮ** (13:10 UTC): `segment-00000001` закрылся на 1 073 741 818 B
(порог 1 GiB) → создан `segment-00000002` с магией; `seq` сшит через границу
(`17800473` → `17800474`, `seq_gaps=0`), `restarts=0`, healthy, полосы стабильны;
`declare_legacy` выполнен (`sha256:db1ef99e…`, size зафиксирован), и **fail-closed доказан**:
без манифеста `stream` отдаёт `foreign segment (no magic, no declaration)`, при этом **запись не
прерывается** (T7c в проде); полосы OBI на прогретой книге НЕ деградировали (`avg buckets
1154/969` vs baseline `1316/1452`; полоса 600–6000 bps `1115/873` vs `975/845`).
**Чего milestone НЕ дал (открыто):** ретеншен никем не вызывается (TD-020 — «сбор не остановится
никогда» НЕ достигнуто, ~40 дней до disk-guard); эвикция книги не удерживает рост (TD-016 остаётся
OPEN: уровни 5k → 13.8k за 4 ч, окно ±60% ничего не режет); `storage_status` не публикуется в
heartbeat (TD-019). Отдельно: метрика памяти, по которой TD-016 был заведён, оказалась
загрязнена page cache — настоящий рост кучи +1 MiB/час, не +8 (TD-021).

### rev 6 (задачи 11/12/13) — КОД MERGED + В ПРОДЕ (`8882c1e`, reviewer APPROVED 2026-07-14); **milestone ВСЁ ЕЩЁ НЕ ЗАКРЫТ: ГЛАВНАЯ цель (TD-020) не достигнута**
Цепочка: architect RED (`4475bfa`, `6f1b7f4`) → engine-dev (`8b4dc6f` task 11, `24d8e83` task 12) →
tester PASS → reviewer. Гейты (перепрогнаны reviewer'ом независимо на чистом worktree):
workspace **172 passed / 0 failed**; `verify_M-08.sh` **28/28 PASS, exit=0**; fmt/clippy clean;
CI + Deploy на merge-коммите — success. **Анти-плацебо доказан reviewer'ом независимо:** все 7
оракулов `red_retention_operator` (R1–R7) + `red_heartbeat_status` **FAIL против пред-фиксного
дерева `4475bfa`** (`not yet implemented` в `retention_plan`; heartbeat не JSON), GREEN на HEAD.
- `crates/journal` (**task 11, TD-020**) — `retention_plan(dir, policy, now_wall_ms)` /
  `retention_execute(...)` + **бинарь `journal-retention`** (`src/bin/`). Часы СНАРУЖИ (план
  детерминирован, `DET-I-1`-дисциплина); **`DryRun` — дефолт CLI** (конструктивный барьер против
  «случайно удалил»); Apply идёт ТОЛЬКО через `verify_cold_copy` → `ColdCopyProof` → `prune_segment`
  (сверка sha256 холодной копии; сбой сверки → сегмент остаётся ГОРЯЧИМ и попадает в `failed`,
  exit=2). Активный сегмент никогда не в плане; `keep_min_segments` защищает последние N;
  НЕЗАДЕКЛАРИРОВАННЫЙ legacy не удаляется (нет эпохи → нет права); `disk_pressure` при пустом плане
  поднимает флаг (exit=3), а не молчит. Оракулы содержат деградированные входы (недоступное
  холодное хранилище, чужой сегмент, пустой план) — per `.claude/rules/testing.md`.
- `crates/recorder` (**task 12, TD-019**) — heartbeat = JSON `{ts_wall_ms, next_seq, segment_index,
  events, free_bytes, min_free_bytes, writable}` вместо 13 байт таймстампа; финальный heartbeat при
  выходе. В журнал НЕ пишется (детерминизм). Healthcheck compose'а смотрит на **mtime** файла, не на
  содержимое → смена формата прод-безопасна (проверено: контейнер healthy после деплоя).
- `crates/venue-binance` (**task 13, TD-016 переспека после TD-021**) — `BACKSTOP_LEVELS_PER_SIDE`
  50k → **200k**: приоритет развёрнут (точность данных > экономия памяти), т.к. «лик» был измерен
  загрязнённой page-cache метрикой, а эвикция резала уровни внутри полос OBI 6–60 %. Кап остаётся
  ТОЛЬКО аварийным потолком от OOM.
- **§8 eyes-on (прод `8882c1e`, 2026-07-14) — GREEN по деплоенной части:** контейнер healthy,
  `restarts=0`, `panic/ERROR/backstop = 0`; **боевой legacy-сегмент цел БАЙТ-В-БАЙТ** — полный
  sha256 15 188 347 171 B до и после деплоя совпал (`234583c8e5c0…`), mtime заморожен (08:47);
  recorder продолжает писать в `segment-00000002.jrnl` (магия `HFTJRN02`, растёт 437 → 528 MB);
  **heartbeat несёт состояние** (`writable=true`, `free_bytes=119 134 494 720`,
  `min_free_bytes=10 737 418 240`, `next_seq=18 733 828`, `segment_index=2`) ⇒ **TD-019 CLOSED**;
  `RssAnon = 11 376 kB` (правильная метрика per TD-021), `book levels` ≈ 5000/сторона после
  рестарта — baseline для наблюдения асимптоты (задача 13).
- **БЛОКЕР close-out'а (найден reviewer'ом на PR-гейте, подтверждён на проде): TD-020 НЕ ЗАКРЫТ —
  бинарь `journal-retention` НЕ ДОСТАВЛЯЕТСЯ В ПРОД.** `Dockerfile` собирает `cargo build --release
  **--bin recorder**` и копирует в runtime-образ ТОЛЬКО `recorder` (факт на проде:
  `docker exec hft-recorder ls /usr/local/bin/` → один `recorder`); на VPS нет Rust toolchain;
  холодное хранилище не смонтировано (`/mnt/*` пуст, Storage Box не заведён); cron отсутствует
  (`/etc/cron.d/` → только `e2scrub_all`). ⇒ §8-пункты «dry-run ретеншена на проде» и «cron»
  **физически невыполнимы**, ретеншен по-прежнему **никем не вызывается**. Это тот же класс дефекта,
  что и исходный TD-020, этажом выше: раньше была библиотека без оператора — теперь оператор без
  доставки. Диск: 111 GB свободно, ~2.8 GB/сут ⇒ таймер ~40 дней тикает. Нужна **задача 14**
  (доставка: сборка `journal-retention` в образ/на хост + монтирование холодного хранилища +
  cron + алерт на exit≠0) — спека architect, impl engine-dev.
- **M-08 остаётся 🚧 IN_PROGRESS.** Закрывается ТОЛЬКО после задачи 14 + §8 с реальным dry-run
  ретеншена на проде.

### rev 9 (задачи 15/16) — REVIEWER APPROVED (код) → §8 PROD REJECTED + REVERTED (`82b33db`, 2026-07-14)
Стек rev9 (`cb46e34` RED C7-C9+D7 / `9cf5acf` task 15 crash-window self-heal / `1ff1b55` task 16
оператор компакции) закрыл ОБА rev8-блокера reviewer'а и подтверждён фактом:
- **D-COMP-1** (дубликаты в прод-пути): `segments()` теперь дедуплицирует raw+.zst через общий
  `dedup_indexed_paths` (raw побеждает при коллизии). Репро крах-окна: было 3172 события → 3000.
- **D-COMP-2** (self-heal): ветка `dst.exists()` сверяет sha256 распакованного `.zst` с оригиналом;
  совпало → доделать (удалить оригинал), битый `.zst` → удалить `.zst`, оригинал ГОРЯЧИЙ, `Err`.
- **D-COMP-3** (оператор): `--mode compact` у `journal-retention` + compose-сервис + cron + гейт D7
  (реальный запуск бинаря, не греп).

Локальные гейты на **merge-коммите** (не только feat): `fmt` 0, `clippy -D warnings` 0,
`cargo test --workspace` **181/0**, `verify_M-08` PASS, `verify_delivery` PASS (вкл. D5a+D7),
`crontab -n` 0. Анти-плацебо: C7/C8/C9 FAIL против `cb46e34`, GREEN на HEAD; наивная C5-мутация
"распаковать в RAM" валит C5 (100.7 MB пик). Merge `2b2311f` запушен в main.

**CI-флак (не блокер merge, но задержал):** первый CI на `2b2311f` — RED, exit 101 на
`td016_memory_bounded_when_price_drifts_out_of_band` (**НЕ** тест компакции; глобальный
аллокатор-счётчик, флак под параллельным `cargo test --all`). Re-run того же коммита — GREEN
(флак подтверждён). Заведён **TD-023**. Deploy re-run → success, компакция доехала до VPS.

**§8 PROD RED — CRITICAL data-loss дефект (доказан фактом, prod НЕ тронут):** eyes-on на VPS
показал, что `segment-00000000.jrnl` (15 GB) — **LEGACY** (магия `0c 00…`, не `HFTJRN02`;
задекларирован в `journal.legacy.json`). Оператор `--mode compact` жмёт СТАРЕЙШИЕ закрытые первыми
⇒ выбрал бы legacy-0. `compact_segment` его сжимает (sha сырых == sha распакованных → верификация
проходит → **оригинал удаляется**), но обратное чтение `.zst` требует v2-магии
(`skip_v2_header_forward`) → `CorruptHeader` → `list_segments`/`stream` падают ⇒ **ВЕСЬ ЖУРНАЛ
НЕЧИТАЕМ, 15 GB невосполнимой истории стёрты.** Воспроизведено в песочнице (legacy-0+v2-1+v2-2 →
`compact_closed_segments(keep_raw=1)` → `list_segments`/`stream` = `corrupt SegmentHeader`);
**реальную компакцию на prod-каталоге НЕ запускал** (prod цел: 5 сырых сегментов, cron НЕ
установлен). По правилу §8 «красный/опасный прод → revert» весь стек rev9 откатан `82b33db`.
См. **TD-022 rev9** (виток: компакция ОБЯЗАНА не трогать legacy; RED-набор обязан включать
legacy-сегмент — C1-C9 строят только v2, прод-раскладка не покрыта) + **TD-023** (флак-оракул).
**M-08 остаётся IN_PROGRESS; TD-020, TD-006, TD-022 остаются OPEN; TD-023 новый.**

### rev 10 (задачи 17/18 — legacy-безопасность компакции) — REVIEWER APPROVED + MERGED (`8a2e377`, §8 PROD GREEN, 2026-07-15)
Реленд rev9-стека + фикс CRITICAL data-loss (TD-022). Ветка `feat/M-08-compaction-reland` (5 коммитов,
линейна, fast-forward): `4d92373` (**чистый revert-of-revert** `82b33db` — восстановил rev9-стек 1:1;
reviewer сверил `tree(4d92373)==tree(2b2311f)` побайтово — architect НЕ дописывал impl, механическое
восстановление уже-ревьюненного) → `7754308` C10 RED (architect) → `0c7bef4` TD-023 fix (architect) →
`0cd4eca` **D-COMP-4** (engine-dev) → `8a2e377` §8-план (architect).
- **D-COMP-4** (`crates/journal/src/segments.rs`): `compact_segment` возвращает `Err` на сегменте, чьи
  первые байты `!= SEGMENT_MAGIC` (`HFTJRN02`), **ДО любой мутации** (конструктивный барьер — тот же
  принцип, что «активный не сжимаем»); `compact_closed_segments` тихо пропускает legacy/foreign (Err по
  маркеру → в `failed`, не пробрасывает). Legacy читается как есть; сжатие legacy архитектурно запрещено.
- **C10** (sacred RED, architect): прод-раскладка legacy-0(declared, no-magic) + v2-закрытые + активный;
  реальная `compact_closed_segments(keep_raw=1)` → legacy НЕ тронут, `.zst` для legacy НЕ создан,
  `stream` до==после. Закрывает дефект фикстуры C1-C9 (строили только v2 — прод-раскладка не покрыта).
- **Гейты (reviewer перепрогнал независимо на чистом worktree):** fmt/clippy clean, **workspace 182/0**,
  `red_compaction` **10/10** (C1-C10), `red_book_bounded` 7/7, `verify_M-08` PASS, `verify_delivery` PASS
  (D1-D7 + **deep** D1-deep/D2-deep, реальный образ). **Анти-плацебо доказан независимо:** C10 FAIL
  против `7754308` (без барьера — «legacy стёрты», `red_compaction.rs:562`), GREEN на HEAD. CI+Deploy
  на merge success.
- **§8 (два шага; `--mode compact --dry-run` НЕ существует — режимы взаимоисключающи):**
  - **Step A (delivered binary на sandbox):** образ `hft-platform-recorder:local` на faithful прод-
    раскладке → legacy байт-в-байт цел, legacy `.zst` НЕ создан, 32 v2 сжаты (14.5×), `stream`=3500
    до и после (потерь нет). Барьер доказан в ДОСТАВЛЕННОМ артефакте до касания боевого legacy.
  - **Step B (РЕАЛЬНАЯ компакция боевого `/journal`):** через доставленный cron-скрипт (exit=0, alert
    не взведён). **Боевой legacy-0 БАЙТ-В-БАЙТ ЦЕЛ** — полный sha256 `234583c8…bdbdc72` == эталон,
    size=15188347171, mtime=1784018822 не изменились (D-COMP-4 сработал на живом 15 GB legacy);
    сегменты 1-5 → `.jrnl.zst`, `zstd -t` каждого = исходный raw-размер (данные целы); **свободно
    111.20 → 115.88 GB (+4.69 GB) — диск ДВИНУЛСЯ**; recorder healthy, restarts=0, next_seq растёт,
    heartbeat свежий (конкурентная компакция закрытых не задела живого писателя).
- **⇒ TD-022 CLOSED** (legacy-безопасность доказана на РЕАЛЬНОМ активе), **TD-023 CLOSED** (флак устранён).
- **§8 ПОЙМАЛ новый delivery-дефект → TD-024 (MAJOR, OPEN):** compose-сервисы `journal-compaction`/
  `journal-retention` держат `command:` в equals-form (`--dir=/journal`), а бинарь `=`-форму НЕ
  разбирает → задокументированные `docker compose run --rm journal-<svc>` СЛОМАНЫ; работает только
  точный cron-argv (раздельная форма), через который reviewer и выполнил §8-B. `verify_delivery`
  гонял только cron-argv, не `command:`-блок против живого бинаря.
- **M-08 всё ещё IN_PROGRESS (НЕ закрыт):** cron НЕ установлен на проде (компакция разовая-ручная →
  для durable-сдвига дедлайна нужна установка + фикс TD-024); ретеншен (`--mode apply`, cold-выгрузка)
  не запускался — нет Storage Box (founder ★); TD-016 наблюдение и TD-006/TD-020 остаются OPEN.

### rev 12 (задача 20 — активация cron, хвост 1) — REVIEWER APPROVED + MERGED (`d3e7db2`, §8 CRON АКТИВЕН, 2026-07-15)
Durable-компакция: cron активирован на проде + позитивный heartbeat (silent-absence детектируется).
Ветка `feat/M-08-cron-activation` (2 коммита, ff): `eb0e6cc` architect (README модель активации +
мониторинг + гейт **D9 RED** + milestone) → `d3e7db2` engine-dev (cron-скрипты пишут `*.last-success`).
- **Positive heartbeat (D9):** оба cron-скрипта на УСПЕШНОМ прогоне пишут `*.last-success` (UTC).
  `*.alert` ловит «прогон УПАЛ», `*.last-success` freshness ловит «cron НЕ запускался» (не установлен/
  crond мёртв/ребут) — РАЗНЫЕ классы отказа, нужны ОБА (урок сессии: и сбой, и МОЛЧАНИЕ видимы). D9-гейт
  прогоняет скрипт со стабом-успехом, проверяет запись маркера — не грепом.
- **Модель активации (governance):** артефакты доставляются через репо/образ, но `install /etc/cron.d`
  — ОСОЗНАННЫЙ РУЧНОЙ шаг с founder-★ (не авто-`deploy.yml`: цена ошибки на автомате с data-модифи-
  цирующим расписанием выше). Retention остаётся `--mode=dry-run` (apply — после Storage Box).
- **Гейты (reviewer независимо):** fmt/clippy clean, verify_delivery PASS (D8+**D9** обоих сервисов),
  crontab -n 0. **CI на `d3e7db2` GREEN** (adequate-disk runner). RED-first: D9-гейт при `eb0e6cc` есть,
  `*.last-success` в скриптах — только с `d3e7db2`.
  ⚠ **`verify_M-08.sh` FAIL ЛОКАЛЬНО** на `red_prod_migration` (`error: StorageGuard`) — pre-existing
  **env-флейк**: тест берёт `WriterConfig::own_capture` (min_free=10 GiB), а локальный диск 8.9 GiB/98%.
  НЕ логика, НЕ эта ветка (crates/journal не тронут): **CI на adequate-disk = GREEN**. Заведён **TD-025**
  (architect: min_free_bytes:0 в тесте как у соседних фикстур ИЛИ требование к test-env; блокирует
  tester task 7 на full-disk чекауте).
- **§8 CRON АКТИВАЦИЯ на VPS (founder-★ авторизация relayed через диспетч):** deploy НЕ триггерился
  (deploy-only пути), поэтому reviewer обновил чекаут VPS до `d3e7db2` (скрипты несут `.last-success`),
  установил `/etc/cron.d/hft-journal-retention` (компакция `50 3`, ретеншен dry-run `7 4`) + `/var/{log,
  lib}/hft`, restart cron. **Eyes-on АВТО-прогона** (temp every-minute schedule): cron сам отработал →
  **свежий `compaction.last-success` 2026-07-15T18:56:02Z** (heartbeat пишется), alert не взведён,
  **legacy-0 байт-в-байт цел** (`234583c8…`), recorder healthy restarts=0; прогон компактил 0 (keep_raw=2
  берёг единственные 2 закрытых, legacy skipped — штатно). Real-schedule восстановлен. Disk-moving
  компакция через ЭТОТ код-путь доказана §8-B rev10/rev11 дважды (+4.69, +1.94 GB); recurring 03:50
  сожмёт по мере накопления. ⇒ **хвост 1 закрыт; TD-024 CLOSED; TD-006/TD-020 durable-замедлены.**
- **M-08 всё ещё IN_PROGRESS:** хвост 2 — Storage Box + retention apply (founder ★). После него:
  tester clean-checkout verify (см. TD-025 про disk) → architect close-out → reviewer финальный §8.

### rev 11 (задача 19 — TD-024 equals-form CLI) — REVIEWER APPROVED + MERGED (`e31e23e`, §8 PROD GREEN, 2026-07-15)
Фикс delivery-дефекта, пойманного §8 rev10: операторский путь через `docker compose run` был сломан.
Ветка `feat/M-08-td024` (3 коммита, fast-forward): `475bbd5` architect RED `red_cli_argv.rs` (гоняет
НАСТОЯЩИЙ бинарь: equals dry-run/compact + регресс раздельной) + гейт **D8** → `935bc9b` engine-dev
фикс парсера → `e31e23e` engine-dev README §4.
- **Фикс (`crates/journal/src/bin/journal-retention.rs`):** нормализация argv ДО цикла разбора —
  `--flag=value` → `split_once('=')` → `[--flag, value]` (equals-форма из compose `command:` и `--help`
  теперь понимается); раздельная форма (cron) проходит без изменений — регрессии нет.
- **D8** (`verify_delivery`): извлекает `command:`-блок ОБОИХ сервисов из `docker-compose.yml`, гонит
  реальный бинарь ровно этой формой argv → закрывает слепое пятно D5a/D7 (гоняли только cron-argv).
- **Гейты (reviewer независимо):** fmt/clippy clean, workspace **185/0**, `red_cli_argv` 3/3,
  `red_compaction` 10/10, verify_M-08 PASS, verify_delivery PASS (D8 обоих сервисов), crontab -n 0.
  **Анти-плацебо:** оба equals-теста FAIL против `475bbd5` (без фикса), раздельный проходит; GREEN на HEAD.
- **§8 PROD (VPS `e31e23e`, CI+Deploy success):** `docker compose --profile ops run --rm
  journal-compaction` (equals-form команда, падала «неизвестный флаг» до фикса) → **exit=0**, сжаты
  сегменты 6,7 (10.43×), **диск +1.94 GB**; `... journal-retention` (dry-run) → **exit=0**, 0 prune,
  legacy/active/young skipped, disk_pressure нет; **boевой legacy-0 БАЙТ-В-БАЙТ ЦЕЛ** (sha256
  `234583c8…bdbdc72`), recorder healthy, restarts=0. ⇒ **TD-024 CLOSED**; операторский compose-путь
  работает end-to-end.
- **M-08 всё ещё IN_PROGRESS:** остаётся установка cron (durable-компакция) + Storage Box (ретеншен
  apply, founder ★); TD-006/TD-020 OPEN до этого.

### rev 7/8 (задачи 14/15) — REVIEWER REJECTED + REVERTED (`b43044d`, 2026-07-14)
Стек `d43d923..91f11aa` (task 14 delivery + task 15 compaction + D5/C5 fixes) прошёл локальные
reviewer-гейты: `fmt`, `clippy -D warnings`, workspace **178 passed / 0 failed**,
`verify_M-08.sh` PASS, `verify_delivery_M-08.sh` PASS, deep delivery PASS. Анти-плацебо:
старый cron из `e4f23d1` валит новый D5 (`bad minute`, exit=1); C1-C6 валятся на `76aadb2`;
наивная C5-мутация "распаковать .zst в `Vec<u8>`" валится на ~100 MB пика.

**§8 PROD RED:** после merge/push `91f11aa` CI и Deploy были зелёные, VPS был healthy
(`restarts=0`, heartbeat свежий, recorder писал), но реальное задание
`/root/hft-platform/deploy/bin/journal-retention-cron.sh` упало ДО плана:
`journal-retention: неизвестный флаг --dir=/journal`. Причина: cron/compose передают
`--flag=value`, а CLI `journal-retention` парсит только пару `--flag value`. Это тот же класс
TD-020: артефакт в образе/cron существует, но операторский путь не отрабатывает на боевом
каталоге. Установленный для проверки cron-артефакт и alert-marker удалены.

По правилу §8 "красный прод → revert" стек откатан одним коммитом `b43044d`; rollback CI
`29359107762` и Deploy `29359107734` GREEN. Прод после отката: `/root/hft-platform` HEAD
`b43044d`, `hft-recorder` healthy/restarts=0, heartbeat свежий, активный `segment-00000003.jrnl`
растёт, `/etc/cron.d/hft-journal-retention` отсутствует. **M-08 остаётся IN_PROGRESS; TD-020,
TD-006 и TD-022 остаются OPEN.**

- `crates/contracts` (**CT-RFC-02**, atomic RFC `docs/rfc/CT-RFC-02-journal-provenance.md`) —
  `SCHEMA_VERSION` 1→2; provenance живёт в ЗАГОЛОВКЕ СЕГМЕНТА, не в `Event` (при 2.8 GB/сут тег в
  каждом событии = гигабайты мусора): `SegmentHeader{schema_version, source, provenance, epoch_id,
  created_wall_ms, first_seq}`, `DataSource{OwnCapture,Vendor,Synthetic}`, `LegacySegmentDecl`/
  `LegacyManifest`, `SEGMENT_MAGIC = HFTJRN02`. **`Event`/`EventKind` НЕ изменены** (аддитивно,
  старые журналы читаются навсегда — CT-I-3; в дифе только `derive(JsonSchema)`). Пакет полный:
  типы + JSON Schema **сгенерированная** (`examples/gen_schema.rs`, сверяется с типами тестом
  `red_schema`) + фикстуры valid/invalid + `CHANGELOG.md`. Классификация сегментов **fail-closed**
  (находка critic C-005 C2): магия есть → заголовок ОБЯЗАН разобраться; магии нет → сегмент
  читается ТОЛЬКО по ЯВНОЙ декларации в `journal.legacy.json` со сверкой отпечатка (sha256 первого
  MiB + размер). Прежнее «не разобрался → считаем OwnCapture» было fail-open — чужие данные
  получали наше происхождение.
- `crates/journal` — **ротация** (`segment-NNNNNNNN.jrnl`, 1 GiB, `seq` сквозной через границы,
  заголовок в каждом сегменте); **`stream(dir, EpochFilter)`** — bounded-memory итератор (прод-путь
  research; RED на 16/64 MiB с counting-allocator, пик < 8 MiB — на 15 GB `Vec<Event>` не влезет
  никогда, класс TD-011 этажом выше); **`EpochFilter`** (дефолт `OwnCaptureOnly` — вендор/синтетика
  в обучение по умолчанию НЕ попадают); **retention**: `prune_segment` требует `ColdCopyProof` с
  приватным конструктором → «удалить невыгруженный сегмент» невозможно ВЫРАЗИТЬ (типовой барьер,
  `compile_fail`-доктест), битая холодная копия → proof не выдан; **disk-guard fail-closed**
  (свободно < `min_free_bytes` → `append` → `Err`, ни байта и ни одного `seq`; `storage_status()
  .writable=false` в heartbeat; `Sys`-событие в журнал НЕ пишется — писать в журнал в момент,
  когда запись запрещена, самопротиворечиво).
- **Задача 10 (МИНА, поймана architect'ом при разборе SVR):** `read_all`/`recover` были
  захардкожены на `segment-00000000.jrnl` и парсили магию v2 как len-поле → на новом журнале
  **молча вернули бы 0 событий**, а их зовут `book/examples/{bands,obi_probe}.rs` — вся диагностика
  полос OBI. Исправлено: обход ВСЕХ сегментов, понимание v2 + legacy wire-формата. Остаются
  ОФЛАЙН-диагностикой с мягкой классификацией; барьер **T11e** (verify) запрещает звать их из
  любых `crates/*/src` кроме `journal` — прод и research ходят ТОЛЬКО через `stream` с явным
  `EpochFilter`. Reviewer проверил фактически: новый `recover` читает боевой legacy-хвост
  (14 119 событий из 40 MiB хвоста прод-сегмента).
- `crates/recorder` — пишет заголовок сегмента (provenance = версия recorder'а + git sha, эпоха
  `own-YYYY-MM`), переживает ротацию без потери/дублей `seq`. **Прод-миграция под тестом (T7c)**:
  recorder СТАРТУЕТ на каталоге с НЕзадекларированным боевым сегментом (запись не зависит от
  декларации — деплой не может остановить сбор), пишет в НОВЫЙ `segment-00000001.jrnl`, старый
  сегмент байт-в-байт нетронут (дописывать в безголовый запрещено).
- `crates/research-cli` — чтение переведено на `journal::stream` + `EpochFilter` (грид больше НЕ
  держит `Vec<Event>`); RED требует **ЭКВИВАЛЕНТНОСТИ** стрим-грида и in-memory (PnL до цента,
  интенты, филлы) — «оптимизация» не смеет тихо изменить измеряемую логику (урок M-07); gap-статистика
  (`data_quality`) → `research/data-quality/gaps-<epoch>.json`, отчёт обязан на неё ссылаться.
- `crates/venue-binance` (**TD-016, задачи 9/9b**) — эвикция уровней книги. **v1 отреджекчена
  reviewer'ом на PR-гейте** (кап 5000 + side-filter по mid ДИФФА стирал живые уровни, включая best
  bid, на асимметричном диффе → тихая порча `L2Snapshot` при зелёных RSS/health). **v2 (`421d5b6`)**:
  уровень удаляет ТОЛЬКО `size==0`; эвикция — по расстоянию от mid КНИГИ за пределами окна ЭМИССИИ
  (`MAX_REL_DIST` ±60%) ⇒ режется ровно то, что никогда не эмитится; `BACKSTOP_LEVELS_PER_SIDE
  = 50_000` от OOM (+`tracing::warn`); наблюдаемость D (`book levels` ≥1/мин) — чтобы §8 мерил
  УРОВНИ, а не только RSS. Reviewer доказал анти-плацебо независимо: 2 новых оракула FAIL против
  v1-impl, GREEN против v2. **Атрибуция лика к книге на проде НЕ доказана** — §8 покажет.
- `.github/workflows/deploy.yml` — Deploy гейтится на CI (fail-closed) + `set -euo pipefail` в
  ssh-скрипте (раньше упавший `git fetch/reset` не останавливал сборку → фантомный деплой).
  **Гейт РАБОТАЕТ, доказан сквозным прогоном** (TD-017 + TD-018 CLOSED): run @`1123b13` → Deploy
  FAILURE (гейт не пустил, 403 на чтении статуса CI), после `permissions: actions:read`
  (`b7721d1`) → CI success → Deploy success. Deploy при красном CI более невозможен.
- Гейты (reviewer перепрогнал независимо): workspace **164 passed / 0 failed**; `verify_M-08.sh`
  **26/26 PASS, exit=0**; fmt/clippy clean; CI на merge-коммите success.
- **Урок (зафиксирован architect'ом в процессе, `5fabd2b`):** два milestone'а подряд дефект прошёл
  ВСЕ зелёные оракулы и был пойман reviewer'ом на PR-гейте — оба раза причина одна: **фикстура
  «счастливого пути»** (M-07 — событие с одним филлом; M-08 — симметричный дифф). Оракул границы
  ресурса обязан иметь и деградированный/асимметричный вход.

## Пока НЕ реализовано (следующие фазы)
- Крейты `risk`/`killswitch`/`oms`, `runner` — пофазно per DESIGN §10 (M-08: fail-closed риск-гейт
  между `strategy` и `oms`). MM-котирование, wiring весов из `signals.json` (граница B),
  netting/корреляции — вне M-07 (named-not-silent). `book` microprice/depth-полосы сверх
  M-04-примитивов — по мере надобности.
- Полный формат журнала (сегмент-ротация, снапшоты, state_hash, DET-I-1 полный) — пофазно.
