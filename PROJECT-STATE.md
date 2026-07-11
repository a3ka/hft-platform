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
