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

## Data expansion (M-06 — inert-части MERGED, reviewer APPROVED 2026-07-11; milestone IN_PROGRESS)
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
- **M-06 остаётся IN_PROGRESS:** #1 (blast-radius compile — на main workspace компилируется),
  inert venue-futures / derive части на main, **#4 BLOCKED by TD-014** (live Funding emission still
  absent; depth cadence/churn materially improved by T2 but full §8 not green), #6 verify_M-06.sh
  exit 0 (tester) только после успешного #4. Следующая цепочка: architect RED/live-equivalent oracle
  stronger than current TD-014 T2 production miss (dense L2 + OI, but 0 persisted Funding) →
  venue-dev fix → engine-dev reland → reviewer full §8 → tester verify.
  Data-quality долг:
  TD-012 (futures REST depth limit=1000 undercount). TD-013 anti-hot-loop live-verified, но milestone
  close-out не достигнут из-за TD-014.

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

## Пока НЕ реализовано (следующие фазы)
- Крейты `alpha`/`portfolio`/`strategy`, `risk`/`killswitch`/`oms`, `runner` — пофазно per
  DESIGN §10. `book` microprice/depth-полосы сверх M-04-примитивов — по мере надобности.
- Полный формат журнала (сегмент-ротация, снапшоты, state_hash, DET-I-1 полный) — пофазно.
