# M-04 — Research core: sim + signals + research-cli + OBI Трек A (P2)

STATUS: 🚧 IN_PROGRESS. Authored: architect (Fable), 2026-07-10.
Гейты: critic (триггеры: новые крейты, ≥5 коммитов) → dev → tester → reviewer;
risk-critic + founder ★ — на финальном OBI-отчёте (гейт 6 из `.claude/rules/gates.md`),
не на merge кода (risk/oms/venues/contracts НЕ трогаются).

## Objective

Движок бэктеста per DESIGN §5/§7/§10 (P2): честный симулятор исполнения (`sim`),
зона Границы A (`signals`: trait + SignalBank + registry-загрузчик + OBI №1),
research-платформа (`research-cli`: grid/walk-forward/trials-ledger/метрики/отчёты) —
чтобы прогнать гипотезу `research/hypotheses/H-20260710-obi-asym.md` Трек A (top-N
imbalance) и Трек B-частично (полосы — Binance full-book уже пишется) по
пре-регистрированным критериям фальсификации.

Авторитетные FA: `docs/fa/sim.md` (SM-I-1..10) · `docs/fa/signals.md` (SG-I-1..11) ·
`docs/fa/research-cli.md` (RC-I-1..11). Анти-оверфит: `docs/02-quant-desk.md` §4 +
`.claude/rules/gates.md` §6.

## Contract impact (T1)

**`crates/contracts/**` НЕ трогается** (Block-C не срабатывает). Уточнение per critic
C-001 M1 (классификация НЕ переопределяется — выравнено с 05 §2 + FA research-cli §N):
- `Ord(Ack|Fill|...)` EventKind (sim FA §N "Produced") нужен ТОЛЬКО paper/live-режимам
  (запись в журнал). Backtest M-04 возвращает fills in-memory (`SimFill`, T2) в
  grid-раннер. Добавление `Ord(...)` в `EventKind` = contract-RFC на входе в M-05 (P3).
- `TrialRecord`/`ValidationReport` **ОСТАЮТСЯ T1-формами** per 05 §2; их Rust-типы
  временно живут в `crates/research-cli/src/types.rs` («T1-designate, промоушен
  отложен») — FA research-cli §N несёт named-амендмент об этом (Amendment history
  2026-07-10). JSON-артефакты несут `report_schema_version`; правило единственного
  писателя/читателя выполняется. Промоушен в `crates/contracts` + JSON Schema
  генерация (CT-I-4) — отдельный contract-RFC при появлении Python-консюмера.
  **Reviewer при merge (Block J): завести TECH-DEBT entry `TD-008-t1-report-forms-
  promotion` (TECH-DEBT.md reviewer-owned — architect писать не может).**

## Архитектурные решения (закрывают §O открытые вопросы FA — до реализации)

| # | Вопрос (FA §O) | Решение M-04 |
|---|---|---|
| D1 | `SignalOut.value` диапазон (signals §O) | Направленный score `i64 ×1e8` в `[-1e8, +1e8]` (−1=SELL..+1=BUY). Эмиссия только при `|score| ≥ theta_e8`; «нет сигнала» = `None`, не 0 |
| D2 | `horizon_ms` owner (signals §O) | Метаданные сигнала (`SignalOut.meta`), консюмер в M-04 — research-harness (горизонт выхода позиции в бэктесте). Owner на live — решается в P3 (alpha) |
| D3 | `code_hash` (signals §6) | sha256 байт ИСХОДНИКА `crates/signals/src/<name>.rs` (детерминирован, проверяем в CI; хэш бинаря нестабилен) |
| D4 | deflated Sharpe формула (research-cli §O) | Bailey & López de Prado (2014): `DSR = Φ(((SR−SR₀)·√(T−1)) / √(1−γ₃·SR+((γ₄−1)/4)·SR²))`; `SR₀ = √(V[SR_family])·((1−γ)·Φ⁻¹(1−1/N)+γ·Φ⁻¹(1−1/(N·e)))`, `N` = счётчик семейства СТРОГО из trials-ledger (RC-I-3), `V[SR_family]` — дисперсия SR по trial-записям семейства; при N<2 → `SR₀=0` (PSR-вырождение) |
| D5 | Capacity v1 (research-cli §O) | `capacity_notional = participation_cap(5%) × медиана(traded notional за горизонт)` — детерминированная эвристика v1, помечена в отчёте `capacity_method: "v1-participation"` |
| D6 | Walk-forward окна (research-cli §O) | Per-signal в `SignalSpec` (S-001 задаёт); общий дефолт train=4h/test=1h/step=1h (данных пока часы, не дни) |
| D7 | Латентность без P1 order-path замеров | Версионируемый артефакт `research/latency/<venue>-<symbol>.json`: `delta_md_ns` — эмпирика из журнала (`ts_wall_ms − ts_exch_ms`); `delta_submit/cancel_ns` — измеренный WS RTT VPS→биржа ×2 (пессимизм), `provenance` поле ОБЯЗАТЕЛЬНО описывает методику. Код не содержит default-задержек (SM-I-7/8); честность артефакта — предмет risk-critic на отчёте |
| D8 | trials-ledger hash-chain (research-cli §O) | v1: включён — каждая запись несёт `prev_sha256`; файл открывается `O_APPEND` |
| D9 | Транспорт depth-полос в сигнал (signals §7 предполагал DepthBands-события — их нет в T1) | OBI держит `book::OrderBook` (Layer-1 крейт `book` — санкционированная зависимость вниз) и вызывает его примитивы `depth_within`/top-N; вычисление полос НЕ реимплементируется. Деривативные события в журнале — вопрос P3+ |
| D10 | PRNG | Собственный SplitMix64 в `sim` (без внешних зависимостей; стабилен навсегда — rand-крейт меняет алгоритмы между версиями, ломая DET) |
| D11 | Registry-загрузчик scope | Минимальный per signals FA §6: чтение `research/registry/signals.json`, code_hash-сверка (D3), params-валидация, retired-skip. Горячий движок его подключит в P3; в M-04 консюмер — research-cli (`--from-registry` опционален, грид инстанцирует напрямую по SignalSpec) |

## Allowed / Forbidden paths (scope-guard)

| Агент | Allowed | Forbidden |
|---|---|---|
| architect (Fable) | `milestones/`, `crates/*/tests/**` (RED, sacred), типы/трейты-скелеты, `scripts/verify_M-04.sh`, `research/specs/S-001-*.md` | impl-код |
| engine-dev | `crates/sim/src/**`, `crates/sim/Cargo.toml` (deps), **carve-out per C-001 C1 + SVR-резолюция 2026-07-10: `crates/book/src/lib.rs` ТОЛЬКО реализация `top_n_depth`, `levels`, `size_at` (сигнатуры и RED-тесты — architect; levels/size_at добавлены после honest-STOP engine-dev: taker_fills требует поуровневого доступа, ahead — объёма на нашей цене per FA sim §5)** | tests, другие крейты, contracts |
| signal-engineer | `crates/signals/src/**`, `crates/signals/Cargo.toml`, `research/specs/` | tests (sacred), risk/oms |
| research-dev | `crates/research-cli/src/**`, `crates/research-cli/Cargo.toml`, `research/latency|fees/` (артефакты по D7) | tests, registry/signals.json |
| все dev | — | `crates/contracts/**`, `crates/journal/**`, `crates/book/**`, `crates/venue-*/**`, `.claude/**`, `docs/**`, `*/tests/**`, `scripts/**` |

## §Tasks

| # | Status | Задача | Агент | Verify |
|---|---|---|---|---|
| 1 | ✅ | Скелеты крейтов `sim`/`signals`/`research-cli` (типы T2, трейты, todo!-стабы) + RED-тесты + verify-скрипт | architect | компиляция OK; RED-suite падает |
| 2 | ✅ | `sim` impl: fill_model (пессимистичная очередь §5) + latency (таблица, SplitMix64) + fees/funding + BacktestExchange + divergence gate-checker; **+ `book::top_n_depth` (carve-out C1, RED-тест `crates/book/tests/test_top_n_depth.rs`)** | engine-dev | SM-I-1,2,4,5,6,7,8,10 GREEN + book-тест GREEN |
| 3 | ✅ | `signals` impl: SignalBank (изоляция паник SG-I-9) + registry-загрузчик (D3/D11) + `obi.rs` (TopN+Bands режимы, D1/D9) | signal-engineer | SG-I-1..11 + OBI-тесты GREEN |
| 4 | ✅ | `research-cli` impl: ledger (O_APPEND+hash-chain D8) + split (val-gate токен) + metrics (Sharpe/DSR D4/maxDD/fill-rate/turnover/capacity D5/decay) + grid/walkforward (стресс ×1.5-cost/×2-latency) + report (детерминизм) + CLI | research-dev | RC-I-1..11 GREEN |
| 5 | ✅ | Артефакты D7: latency-probe (δ_md из журнала + RTT-замер) → `research/latency/*.json`; `research/fees/*.json` (тарифы Binance spot/HL `[verify-at-impl]` с ссылкой на доку) | research-dev | файлы валидны, sim их грузит |
| 6 | ✅ | `scripts/verify_M-04.sh` exit=0 (fmt+clippy+все тесты+грепы) | tester | exit=0 |
| 7 | ✅ | SignalSpec `research/specs/S-001-obi-asym.md` (params-схема, гриды, walk-forward D6) — сверка с H-карточкой | architect+signal-engineer | RC validate находит карточку |
| 8 | ⏳ | Прогон OBI Трек A (+Трек B на Binance full-book выборке): grid train → топ-K val → walk-forward → ОДНО касание test → `research/reports/R-<НОМЕР ОТ АЛЛОКАТОРА>-obi-track-a.md` (номер выдаёт `scripts/next_artifact_id.sh R` в момент создания отчёта — см. амендмент ниже) | research-dev запуск; risk-critic вердикт; founder ★ | отчёт по пре-рег. критериям H-карточки; trials-ledger заполнен |

Задача 8 гейтится накоплением данных полной книги (VPS пишет с 2026-07-10) — запуск
допустим на имеющихся часах для Трек A, полноценный вывод — после ≥нескольких дней.

**⚠ Амендмент 2 к задаче 8 (2026-08-14, `TD-139` п.(в)): номер отчёта БОЛЬШЕ НЕ НАЗВАН —
и не будет.** Прежняя редакция предписывала acceptance-артефакт `research/reports/R-001*`.
Идентификатор `R-001` за это время занят другим носителем — `research/reviews/R-001-M-49.md`,
— а по `gates.md` §12 идентификатор `КЛАСС-НОМЕР` УНИКАЛЕН независимо от каталога и предмета.
То есть первый же прогон OBI Трека A ввёл бы второго носителя под занятым номером, и барьер
`check_artifact_ids.sh` покрасил бы `main` ЗАКОННО. Проверено исполнением, а не рассуждением:
симуляция задачи 8 роняет барьер `exit=1`.

**Почему номер не заменён на другой свободный.** Это перенесло бы мину, а не сняло: между
правкой спеки и реальным прогоном OBI пройдут недели, за которые параллельные роли займут
любой заранее вписанный номер — тот же класс сработал за 13.08 ЧЕТЫРЕ раза (в том числе
`R-065` дважды). Спека фиксирует ПРАВИЛО получения номера, а не сам номер: исполнитель зовёт
`bash scripts/next_artifact_id.sh R` НЕПОСРЕДСТВЕННО перед созданием файла и берёт то, что
выдано. Ниже по тексту отчёт называется «отчёт задачи 8», а не номером.

**⚠ Амендмент к задаче 8 (2026-07-13, после merge M-07; TD-015, `.claude/rules/gates.md` §6.3/§6.4):**
прогон OBI и отчёт задачи 8 идут ТОЛЬКО на strategy-пайплайне M-07 (`sim::StrategyBacktest` +
`DirectionalStrategy`) — ad-hoc harness, которым мерили пилот, УДАЛЁН, и его результаты
описывают несуществующую логику. Обязательные условия валидности отчёта задачи 8:
1. **Эпоха ledger'а названа явно.** `N` (счётчик семейства) и `V[SR_family]` для deflated
   Sharpe считаются ТОЛЬКО по записям, сделанным кодом `>= 5141fd9`. Пре-M-07 записи
   (4 шт., `code_hash f7f4761`, Sharpe −1.73..−2.21) в статистику НЕ входят — они мерили
   ad-hoc-harness (`qty=1.0`). Записи из окна equity-бага (`37753a6..5141fd9`) невалидны
   (завышенный Sharpe); в репо-ledger'е их нет, всплывшие из локальных прогонов — отбросить.
2. **Ledger append-only не нарушается** — старые записи НЕ удаляются и НЕ переписываются;
   фильтрация — на чтении, с записью критерия фильтра в отчёт.
3. Отчёт несёт `code_hash` эпохи + `strategy`-блок ячейки (D8 M-07) — иначе воспроизводимость
   мнимая.
4. risk-critic проверяет пункт (0) своего чек-листа (`gates.md` §6.4): смешение эпох = KILL.
5. `research/reports/` в репозитории ещё НЕ существует — отчёт создаётся вместе с каталогом
   (черновик пилота в `tmp/pilot/` вне git-зоны доказательством НЕ является).

## RED-тесты (sacred, architect-only)

- `crates/sim/tests/red_sim.rs` — SM-I-1,2,4,5,6,7,8 юниты; SM-I-10 gate-checker.
- `crates/sim/tests/structural.rs` — SM-I-3 (grep `cfg(sim)` по workspace), SM-I-9
  (cargo-metadata: sim не зависит от venue-*).
- `crates/signals/tests/red_signals.rs` — SG-I-1,2,6,7,8,9,11 + OBI determinism +
  no-signal-below-theta.
- `crates/signals/tests/structural.rs` — SG-I-3,4,5,10 (грепы/структура).
- `crates/research-cli/tests/red_research.rs` — RC-I-2,3,4,5,8,9,10.
- `crates/research-cli/tests/structural.rs` — RC-I-1,6,7,11 (грепы Cargo/src).

Отложено (нужны P3-крейты, задокументировано здесь named-not-silent): полный SM-I-3
(strategy-стек ещё не существует — греп уже стоит на workspace), SM-I-9 paper-обвязка
(структурная половина — cargo-граф — активна), SM-I-10 полный paper-цикл (gate-checker
юнит активен).

## Acceptance

`bash scripts/verify_M-04.sh; echo "exit=$?"` → `VERDICT: PASS`, exit=0.

## Handoff

Диспетчеризация по §Tasks: architect → critic → (2,3,4 параллельно) → 5 → 6 tester →
reviewer → 8 (risk-critic → founder ★).
