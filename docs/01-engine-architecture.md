# 01 — Rust-движок: архитектура торгового ядра

STATUS: DESIGN v1 (2026-07-10, Fable). Венью: Hyperliquid → Binance. Ярус: crypto mid-freq (ms–sec).
Родственный документ: 02 (квант-команда), 03 (контракт интеграции). Venue-специфика помечена
`[verify-at-impl]` — проверяется по актуальной документации биржи при реализации.

---

## §1. Стержневой принцип

**Всё — событие в едином упорядоченном журнале; вся логика — детерминированные машины
состояний над ним.** Из этого следуют три равенства, ради которых строится система:

1. `replay(journal) == реальность` — бит-в-бит (отладка, форензика, аудит каждого цента PnL).
2. `backtest == paper == live-micro == live` — один код стратегии, четыре источника событий.
3. `failover == replay` — резерв догоняет журнал.

Никакой LLM в горячем цикле. LLM-выход попадает в движок только через реестр сигналов
с подписью (см. 03).

## §2. Workspace и модульные границы

```
platform/                     (Cargo workspace, edition 2021, tokio + serde + tracing)
├── crates/
│   ├── journal          # ядро №1: событийная модель, append-only лог, replay, снапшоты
│   ├── venues           # trait'ы MarketDataFeed / OrderGateway + реестр адаптеров
│   │   ├── venue-hyperliquid
│   │   └── venue-binance          (Фаза 5)
│   ├── book             # book builder: L2, целостность, microprice, depth-полосы
│   ├── signals          # Signal trait + реализации (ЕДИНСТВЕННАЯ зона кода квант-агентов)
│   ├── alpha            # ансамбль: weighted combine калиброванных сигналов → forecast
│   ├── portfolio        # sizing: forecast + лимиты + инвентарь → целевая позиция/котировки
│   ├── strategy         # MM-квотер: book→signals→alpha→portfolio→desired orders, diff-квоты
│   ├── oms              # order state machines, идемпотентность, rate-limit бюджет
│   ├── risk             # pre-trade gate + позиции/PnL + reconciliation + аудит (ЗАЩИЩЁННАЯ ЗОНА)
│   ├── killswitch       # ОТДЕЛЬНЫЙ бинарь/процесс (ЗАЩИЩЁННАЯ ЗОНА)
│   ├── sim              # симулятор исполнения (backtest + paper): fill model, латентность, комиссии
│   ├── runner           # композиция режимов: backtest|paper|live-micro|live; arming
│   └── research-cli     # грид-раннер, walk-forward, отчёты metrics.json (интерфейс к 02)
└── ops/                 # compose, дашборд (v0: CLI + JSONL tail), алерты, runbooks
```

**Зависимости — только вниз** (arch-lint правило с первого дня): `signals` не видит `oms/risk/venues`;
`risk` не зависит ни от чего, кроме `journal`-типов; `venues` не видит стратегию. Квант-агенты
имеют write-доступ ТОЛЬКО к `crates/signals/` + `research/` (см. 03 §4).

## §3. Журнал (crate `journal`)

- `Event { seq: u64 /* тотальный порядок, единственный писатель */, ts_mono_ns: u64,
  ts_wall_ms: i64, kind: EventKind, payload }`.
- `EventKind` (закрытый enum, версионируемый): `Md(L2Delta|L2Snapshot|Trade|Funding|MarkPx)`,
  `Ord(Submit|Ack|Reject|CancelReq|Cancelled|Fill{maker|taker}|Expire)`,
  `Risk(Approved|Rejected{rule,margin}|Halt{reason}|NearMiss{rule,utilization})`,
  `Recon(Snapshot|Mismatch)`, `Ctl(Arm|Disarm|ParamChange{signed}|KillSwitch)`, `Sys(ConnUp|ConnDown|Heartbeat)`.
- Диск: сегментированные append-only файлы, postcard-фреймы + crc32 на фрейм, fsync-политика
  по классу события (Ord/Risk/Ctl — синхронно; Md — батчем). ОДИН wire-формат навсегда
  (урок hft-core-rs: три формата = смерть).
- Снапшоты состояния каждые N событий; инвариант: `replay(snapshot_k + tail) == replay(full)` —
  проверяется хэшем состояния (state_hash каждые M событий пишется в журнал).
- Цены/объёмы:整 u64/i64 fixed-point ×1e8 (не f64 в деньгах); время: mono для порядка,
  wall для отчётов.
- **DET-I-1 (RED):** реплей одного дня трижды → бит-идентичное состояние + равные state_hash.

## §4. Data plane (`venues`, `book`)

- `trait MarketDataFeed`: subscribe(l2, trades, funding, mark, userFills) → нормализованные
  события в журнал. `trait OrderGateway`: place/cancel/batch, детерминированный
  `client_order_id = hash(strategy_id, seq, nonce)` (идемпотентность), cancel-all.
- **venue-hyperliquid**: WS `l2Book/trades/userFills/funding` + REST info/exchange; подпись
  действий (EIP-712-класс) [verify-at-impl]; testnet-профиль; лимиты запросов —
  бюджетируются в oms [verify-at-impl].
- `book`: восстановление со снапшота + дельты; sequence-гэп → пометить stale → ресинк
  снапшотом (честный, событийный — никакого `rand()`); производные: microprice,
  spread, **depth-полосы** `depth(side, pct_band)` — примитив, на котором строится
  сигнал №1 (OBI 3%/8%).
- Разрыв связи = события `Sys(ConnDown)`+`Ctl(Disarm)` в журнале; авто-reconnect с
  экспоненциальным backoff + jitter; после reconnect: снапшот → recon → READY (re-arm
  ручной или авто по конфигу).

## §5. Стратегийный контур (`signals`, `alpha`, `portfolio`, `strategy`)

- `trait Signal { fn on_event(&mut self, &Event) -> Option<SignalOut>; fn spec() -> SignalSpecRef; }`
  — чистые, детерминированные, без I/O и без часов (время только из событий). Каждый сигнал —
  Rust-модуль в `crates/signals/`, зарегистрированный в **SignalRegistry** (см. 03 §2)
  с version + params-schema + code_hash.
- `alpha`: `forecast = Σ w_i · normalize(signal_i)`; веса — из подписанного
  калибровочного артефакта (Ctl(ParamChange)); никакого онлайн-обучения в рантайме.
- `portfolio`: forecast + лимиты (max_position, max_notional, инвентарь-кап,
  дневной loss-лимит) → целевая позиция; для MM — целевые bid/ask (цена, размер,
  скос от инвентаря).
- `strategy` (MM-квотер): пересчёт desired quotes на событие; **diff-квотирование**
  (менять только изменившееся — экономия rate-limit); pure-функции, состояние явное.

## §6. OMS (`oms`)

- Машина состояний ордера: `New→Sent→Acked→{Partial}*→Filled|Cancelled|Rejected`,
  таймаут на каждый переход (нет Ack за T мс → инцидент-событие + consеrvative cancel).
- Rate-limit бюджет по venue-правилам [verify-at-impl]; очередь намерений с приоритетом
  cancel > modify > place.
- Reconcile открытых ордеров с биржей при старте и периодически.
- **Cancel-on-disconnect**: биржевой механизм, если есть [verify-at-impl]; независимо от него —
  killswitch-протокол (см. §7).

## §7. Риск-слой (`risk`, `killswitch`) — перенос EINHARD-валидатора

**Pre-trade gate** — единственная дверь к бирже, структурно: `OrderGateway::place`
принимает ТОЛЬКО тип `RiskApproved<Order>` (приватный конструктор в `risk`) — байпас
не выражается в системе типов. Проверки (детерминированные, порядок фиксирован):
инструмент зарегистрирован → размер/нотионал → позиция после исполнения → скорость
ордеров → отклонение цены от mark → дневной PnL-лимит → инвентарь-кап.

**Инварианты (RED-оракулы; тест обязан падать на заглушке):**

| ID | Инвариант |
|---|---|
| RK-I-1 | Ни один ордер не достигает venue без `RiskApproved` (типовой + grep-тест) |
| RK-I-2 | Байпас-флага НЕ СУЩЕСТВУЕТ: нет конфиг-поля, отключающего gate (тест утверждает отсутствие поверхности) |
| RK-I-3 | Неизвестный инструмент/стратегия → Reject (fail-closed, не default-лимиты) |
| RK-I-4 | Gate недоступен/паника → торговля стоит; деградация никогда не «пропускает» |
| RK-I-5 | Отказ записи аудита/журнала → halt (никаких `let _ =`) |
| RK-I-6 | Kill switch работает при мёртвом движке (отдельный процесс, свои креды, свой коннект) |
| RK-I-7 | После разрыва связи на бирже нет наших заявок (cancel-on-disconnect и/или KS-sweep drill) |
| RK-I-8 | Recon-расхождение позиции/ордеров > ε → halt + алерт |
| RK-I-9 | Пробитие дневного loss-лимита → halt; re-arm только человеком |
| RK-I-10 | Параметры/веса меняются только подписанным Ctl(ParamChange) через очередь решений (03 §3) |

Каждое решение gate — событие в журнале, включая **NearMiss** (прошёл при
utilization ≥ 80% лимита) — телеметрия «что почти сломалось».

**killswitch** — отдельный бинарь: слушает heartbeat движка + команду оператора +
триггеры (RK-I-8/9); действие: cancel-all по своему коннекту + установка halt-замка
(файл/ключ), который движок обязан уважать при старте.

## §8. Симулятор (`sim`) — честность бэктеста

- Вход: журнал (исторический — backtest; живой — paper). Наши ордера встраиваются в
  реконструированный L2.
- **Queue position**: консервативно — встаём в ХВОСТ уровня; maker-fill только когда
  traded-объём по цене превысил глубину впереди нас; отмены впереди нас НЕ учитываем
  (пессимистичная оценка — лучше недооценить fill-rate, чем переоценить).
- **Латентность**: δ_submit / δ_cancel / δ_md из измеренных распределений (Фаза 1 собирает);
  ордер «появляется» на рынке через δ, рынок успевает уйти.
- Taker-исполнение — по книге с проеданием уровней; комиссии/ребейты HL по тарифу
  [verify-at-impl]; funding начисляется по расписанию.
- Paper == тот же sim на живом потоке; расхождение sim-vs-live по fill-rate/PnL —
  первоклассная метрика (гейт Фазы 4).

## §9. Режимы и arming (`runner`)

`backtest | paper | live-micro | live` — одна композиция, разные источники/приёмники.
Запуск: config+limits → журнал открыт → фиды → книга → recon → `READY`. Торговля
начинается ТОЛЬКО по `Ctl(Arm)` от оператора (двухключевой старт). `live-micro` =
live с зашитым микро-профилем лимитов (не флагом — отдельный профиль конфига,
подписанный).

## §10. Наблюдаемость и ops

- `tracing` + Prometheus-экспортеры per-crate (`hft_<crate>_*`); гистограммы на бюджеты:
  event→decision p99 < 1ms in-process; place→ack p99 (venue RTT) — измеряем, не обещаем.
- Алерт-каталог (перенос адаптированной таксономии): P0 — halt-события, recon mismatch,
  журнал не пишется, connection lost > T; P1 — stale book, rate-limit близко, sim-vs-live
  дивергенция; P2 — рестарты, ресурсы. **Паритет alert↔exporter — проверка в verify-скрипте**
  (анти-плацебо, урок hft-core-rs).
- EOD: сверка с биржей → PnL-атрибуция (spread capture / inventory / fees / funding) →
  архив журнала → контрольный replay (state_hash совпал = день аудитопригоден).

## §11. Карта reuse/rewrite/delete по hft-core-rs

| Что | Решение | Обоснование |
|---|---|---|
| Fixed-point цены (u64 ×1e6 в types.rs) | идея reuse (у нас ×1e8, i64 для позиций) | верная дисциплина |
| Arena orderbook дизайн | отложенный порт (Фаза 5+, если профилировка потребует) | mid-freq живёт на BTreeMap |
| Трёхуровневое хранение + MAXLEN-уроки | концепт reuse в retention журнала | оплачено их OOM-инцидентом |
| Alert-таксономия P0/P1/P2 + runbooks | reuse адаптированно (§10) | лучшее, что там есть |
| event_bus / 4 пула / 3 retry / DLQ | **delete** (не портируем) | 3 wire-формата, $-баг, пустые DLQ |
| fault_tolerant_feed / stream_router | **delete** | демо-код на несущем пути |
| risk_guard | **rewrite с обратной полярностью** (§7) | fail-open by design |
| snapshot_loader | не нужен (свой ресинк в `book`) | это бутстрап, не реплей |
| marketfeed WS-скелеты | справочно при написании venue-hyperliquid | образец подключения, не код |

Итог: **новый workspace, чистый код; из старого — уроки и 2-3 идеи, не крейты.**
(Отдельно: старый репо не имеет LICENSE — свой код держим отдельно и лицензируем явно.)

## §12. Фазовый роадмап (acceptance = ворота, не даты)

| Фаза | Содержимое | Acceptance (все — исполняемые ворота) |
|---|---|---|
| **P0 Журнал** | journal + replay + state_hash + recorder HL-фида | 24ч записи; replay ×3 бит-идентичен (DET-I-1) |
| **P1 Data plane** | venue-hyperliquid MD, book+ресинк, recorder-демон, замеры латентностей | 7 дней данных; gap-статистика; целостность книги green; распределения δ собраны |
| **P2 Research core** | sim fill-model, research-cli (grid, walk-forward, metrics.json), сигнал №1 OBI формализован | OBI-отчёт по пре-регистрированным критериям (02 §3); paper-режим работает |
| **P3 Risk+OMS** | gate, killswitch, oms; HL **testnet** торговля | RED-suite RK-I-1..10 GREEN (падали на заглушках); 48ч testnet-MM чисто; disconnect-drill: заявок на бирже нет |
| **P4 Live-micro** | одна пара, микро-инвентарь, 2–4 недели | recon mismatch = 0; каждый цент PnL объясняется реплеем; sim-vs-live дивергенция в допуске |
| **P5 Scale** | Binance-адаптер, lead-lag сигнал №3, рабочие лимиты, квант-деск на полном цикле | по отдельному плану |

Параллельно с P0–P2 квант-деск (02) уже работает на записанных данных.
