//! RED M-51 — **DET-I-3** (sacred, architect-only): порядок вывода доменного редьюсера не
//! зависит от хэш-сида.
//!
//! ## Находка, которую этот оракул закрывает
//!
//! `research/measurements/td-007-determinism-coverage.md` §3.1 — единственное место в кодовой
//! базе, где нарушение БУКВАЛЬНО совпадает с явным запретом `CLAUDE.md` («в доменном коде —
//! никакого недетерминизма: нет wall-clock/`rand()`/**итерации по HashMap без сортировки в
//! редьюсерах**»):
//!
//! ```text
//! crates/sim/src/exchange.rs:51    active: HashMap<u64, SimOrder>,
//! crates/sim/src/exchange.rs:240   for (id, order) in self.active.iter_mut() {
//! crates/sim/src/exchange.rs:262       fills.push(SimFill { order_id: *id, .. });
//! ```
//! `std::collections::HashMap` берёт `RandomState` — случайный хэш-сид. Если на ОДНОМ
//! traded-тике исполняются ДВА И БОЛЕЕ maker-ордера одного инструмента, порядок элементов в
//! возвращаемом `Vec<SimFill>` меняется между экземплярами и между процессами.
//!
//! ## Почему существующий оракул этого не ловит
//!
//! `red_sim.rs::test_replay_deterministic_given_seed` гоняет `run_scenario`, которая вызывает
//! `ex.submit(..)` **РОВНО ОДИН РАЗ** — в `active` никогда не больше одного элемента, и
//! итерация по множеству из одного элемента детерминирована тривиально. Это ровно дефект
//! «фикстура счастливого пути» из `.claude/rules/testing.md` (пункт «2. Множественность»),
//! тем же классом, что M-07 (equity-кривая ломалась на событии с 2+ филлами) — тот же слой,
//! тот же класс, другое проявление.
//!
//! ## Контракт
//!
//! **DET-I-3.** Доменный редьюсер не имеет права выводить результат в порядке, определяемом
//! обходом хэш-контейнера. Для `BacktestExchange::on_event`: исполнения, порождённые
//! traded-тиком одного события, следуют в порядке **возрастания `order_id`** — тотального
//! порядка постановки (`next_id` монотонен). Порядок обязан быть свойством ДАННЫХ, а не
//! памяти.
//!
//! Цена ошибки — не абстрактная. `docs/DESIGN.md` §1 обещает «форензику любого убытка, аудит
//! каждого цента»: при двух одновременных филлах оператор, реплеящий журнал дважды, получает
//! РАЗНЫЙ ответ на вопрос «какой из двух моих ордеров исполнился первым». Кроме того,
//! `Vec<SimFill>` уходит дальше в порядке своего построения — `strategy_backtest.rs:90-123` и
//! `research-cli/src/grid.rs:355-392` скармливают его `strategy.on_fill` подряд и сохраняют в
//! отчётный `fills_out`. Сегодняшняя `DirectionalStrategy` СЛУЧАЙНО порядково-инвариантна
//! (целочисленное сложение позиции коммутативно), но это свойство конкретной стратегии, а не
//! структурная гарантия: трейлинг-стоп или «последняя цена входа» сломались бы молча.

use contracts::{to_fixed, Event, EventKind, Level, MdPayload, Side, Venue};
use sim::{BacktestExchange, FeeRates, FeeSchedule, LatencyTable, OrderIntent, OrderKind, SimFill};

// ── фикстуры (форма — из red_sim.rs, чтобы оракулы не разъезжались) ──────────────────────

fn table() -> LatencyTable {
    let mut t = LatencyTable::new();
    for sym in ["BTCUSDT", "ETHUSDT"] {
        t.insert_samples(
            Venue::Binance,
            sym,
            vec![1_000_000],
            vec![1_000_000],
            vec![500_000],
            "synthetic-test-fixture",
        );
    }
    t
}

fn fee_sched() -> FeeSchedule {
    let mut f = FeeSchedule::new();
    f.insert_rates(
        Venue::Binance,
        FeeRates {
            maker_rate_e8: 10_000,
            taker_rate_e8: 45_000,
        },
    );
    f
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

fn snap(seq: u64, ts_ms: u64, symbol: &str) -> Event {
    Event {
        seq,
        ts_mono_ns: ts_ms * 1_000_000,
        ts_wall_ms: ts_ms as i64,
        kind: EventKind::md(
            Venue::Binance,
            symbol,
            MdPayload::L2Snapshot {
                bids: vec![lvl(100.0, 2.0), lvl(99.0, 5.0)],
                asks: vec![lvl(101.0, 2.0)],
                ts_exch_ms: ts_ms as i64,
            },
        ),
    }
}

fn trade(seq: u64, ts_ms: u64, symbol: &str, price: f64, qty: f64) -> Event {
    Event {
        seq,
        ts_mono_ns: ts_ms * 1_000_000,
        ts_wall_ms: ts_ms as i64,
        kind: EventKind::md(
            Venue::Binance,
            symbol,
            MdPayload::Trade {
                price: to_fixed(price),
                size: to_fixed(qty),
                side: Side::Sell,
                ts_exch_ms: ts_ms as i64,
            },
        ),
    }
}

fn maker(symbol: &str, price: f64, qty: f64) -> OrderIntent {
    OrderIntent {
        venue: Venue::Binance,
        symbol: symbol.into(),
        side: Side::Buy,
        price: to_fixed(price),
        qty: to_fixed(qty),
        kind: OrderKind::Maker,
    }
}

/// Сценарий «N maker-ордеров одного инструмента исполняются на ОДНОМ traded-тике».
///
/// `ev1` строит книгу (bid 100.0 размером 2.0 → `ahead` = 2.0 у всех). Ордера подаются после
/// `ev1` (δ_submit = 1 мс → активируются на `ev2`, ts +50 мс). `ev2` — крупная сделка по 100.0
/// на 50.0: `excess = 50 − 2 = 48` покрывает всех, каждый получает Full-филл.
/// Возвращает филлы ИМЕННО этого тика.
fn fills_on_single_tick(n: u64, extra: &[OrderIntent]) -> Vec<SimFill> {
    let mut ex = BacktestExchange::new(table(), fee_sched(), 42);
    ex.on_event(&snap(1, 1_000, "BTCUSDT"));
    ex.on_event(&snap(2, 1_001, "ETHUSDT"));
    for _ in 0..n {
        ex.submit(maker("BTCUSDT", 100.0, 1.0))
            .expect("submit maker");
    }
    for intent in extra {
        ex.submit(intent.clone()).expect("submit extra");
    }
    ex.on_event(&trade(3, 1_050, "BTCUSDT", 100.0, 50.0))
}

fn ids(fills: &[SimFill]) -> Vec<u64> {
    fills.iter().map(|f| f.order_id).collect()
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_14 — МНОЖЕСТВЕННОСТЬ: 12 филлов на одном такте идут по возрастанию order_id.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_14_fills_on_one_tick_are_ordered_by_order_id() {
    const N: u64 = 12;
    let fills = fills_on_single_tick(N, &[]);

    assert_eq!(
        fills.len(),
        N as usize,
        "фикстура: на одном такте обязаны исполниться ВСЕ {N} ордеров (excess 48 покрывает \
         всех) — иначе множественность не проверяется"
    );
    assert_eq!(
        ids(&fills),
        (1..=N).collect::<Vec<u64>>(),
        "DET-I-3: филлы одного такта вышли НЕ в порядке возрастания order_id. Порядок задан \
         обходом `HashMap<u64, SimOrder>` (exchange.rs:240) — то есть хэш-сидом процесса, а не \
         данными. Реплей журнала дважды даёт разный ответ на вопрос «какой ордер исполнился \
         первым» (DESIGN §1: аудит каждого цента)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_15 — независимые экземпляры обязаны согласиться (ловит хэш-сид напрямую).
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_15_independent_exchanges_agree_on_fill_order() {
    // `RandomState` различается МЕЖДУ ЭКЗЕМПЛЯРАМИ HashMap (потоко-локальный ключ
    // инкрементируется на каждый `HashMap::new`), поэтому расхождение проявляется уже внутри
    // одного процесса — нужно лишь достаточно элементов и достаточно попыток.
    const N: u64 = 16;
    const K: usize = 24;

    let first = fills_on_single_tick(N, &[]);
    assert_eq!(first.len(), N as usize, "фикстура: {N} филлов на такте");

    for k in 0..K {
        let again = fills_on_single_tick(N, &[]);
        assert_eq!(
            again, first,
            "DET-I-3: экземпляр #{k} вернул ДРУГОЙ Vec<SimFill> на том же входе и том же seed \
             — вывод редьюсера зависит от состояния памяти, а не от данных (SM-I-2 нарушен)"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_16 — АСИММЕТРИЯ + ОТСУТСТВИЕ: порядок не покупается ценой «отсортировать всё подряд».
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_16_only_eligible_orders_fill_and_order_is_still_ascending() {
    // АСИММЕТРИЯ: в `active` лежит СМЕСЬ — подходящие и неподходящие по каждому критерию
    // фильтра (другой символ / другая цена / не Maker). Реализация, «починившая» детерминизм
    // сортировкой ВСЕГО набора и потерявшая фильтр, провалится здесь.
    let extra = vec![
        maker("ETHUSDT", 100.0, 1.0), // другой инструмент — тик по BTCUSDT его не касается
        maker("BTCUSDT", 99.0, 1.0),  // другая цена — очередь не движется (on_traded_tick)
        OrderIntent {
            kind: OrderKind::Taker,
            ..maker("BTCUSDT", 100.0, 1.0)
        }, // не Maker — путь traded-тика его не обслуживает
    ];
    let fills = fills_on_single_tick(3, &extra);

    assert_eq!(
        ids(&fills),
        vec![1, 2, 3],
        "DET-I-3/АСИММЕТРИЯ: на такте обязаны исполниться РОВНО три подходящих maker-ордера \
         (id 1..3) по возрастанию id; посторонние (другой символ/цена/Taker) обязаны остаться \
         нетронутыми"
    );

    // ОТСУТСТВИЕ: сделка по ДРУГОМУ инструменту не имеет права ничего исполнить и не имеет
    // права «додумать» филлы за источник.
    let mut ex = BacktestExchange::new(table(), fee_sched(), 42);
    ex.on_event(&snap(1, 1_000, "BTCUSDT"));
    ex.on_event(&snap(2, 1_001, "ETHUSDT"));
    for _ in 0..4 {
        ex.submit(maker("BTCUSDT", 100.0, 1.0)).expect("submit");
    }
    let none = ex.on_event(&trade(3, 1_050, "ETHUSDT", 100.0, 50.0));
    assert!(
        none.is_empty(),
        "ОТСУТСТВИЕ: сделка по ETHUSDT исполнила ордера по BTCUSDT — фильтр инструмента потерян"
    );
    // ...и после этого штатный тик обязан отработать в том же порядке.
    let after = ex.on_event(&trade(4, 1_060, "BTCUSDT", 100.0, 50.0));
    assert_eq!(
        ids(&after),
        vec![1, 2, 3, 4],
        "DET-I-3: после нерелевантного события порядок филлов сломался"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_17 — ГРАНИЦЫ: ноль / один / частичные филлы; и МАСШТАБ, при котором совпадение
//          «случайно» невозможно.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_17_boundaries_and_scale() {
    // ГРАНИЦА «ноль активных» — пустой результат, не паника.
    let mut ex = BacktestExchange::new(table(), fee_sched(), 42);
    ex.on_event(&snap(1, 1_000, "BTCUSDT"));
    assert!(
        ex.on_event(&trade(2, 1_050, "BTCUSDT", 100.0, 50.0))
            .is_empty(),
        "ГРАНИЦА: тик без активных ордеров обязан дать пусто"
    );

    // ГРАНИЦА «один активный» — парный vantage: этот случай зелёный и ДО фикса
    // (единственный порядок для множества из одного элемента). Обязан остаться зелёным.
    assert_eq!(
        ids(&fills_on_single_tick(1, &[])),
        vec![1],
        "ГРАНИЦА: один активный ордер обязан дать ровно один филл с id=1"
    );

    // ГРАНИЦА «частичные филлы»: excess мал и делится между ордерами. Проверяет, что порядок
    // пиннится и там, где от него зависят ВЕЛИЧИНЫ (кто первым получил дефицитный объём),
    // а не только последовательность id. Это уже НЕ порядково-инвариантно ни для какой
    // стратегии — прямое расхождение денег между двумя реплеями.
    let mut ex = BacktestExchange::new(table(), fee_sched(), 42);
    ex.on_event(&snap(1, 1_000, "BTCUSDT"));
    for _ in 0..4 {
        ex.submit(maker("BTCUSDT", 100.0, 1.0)).expect("submit");
    }
    // ahead = 2.0, traded = 3.5 → excess = 1.5 на ЧЕТЫРЁХ ордеров по 1.0.
    let partial = ex.on_event(&trade(2, 1_050, "BTCUSDT", 100.0, 3.5));
    assert!(
        !partial.is_empty(),
        "фикстура: дефицитный excess обязан породить хотя бы один филл"
    );
    let seen: Vec<u64> = ids(&partial);
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    assert_eq!(
        seen, sorted,
        "DET-I-3/ГРАНИЦА: при ДЕФИЦИТНОМ объёме филлы вышли не по возрастанию order_id — \
         значит и распределение дефицитного объёма между ордерами зависит от хэш-сида: два \
         реплея одного журнала дают РАЗНЫЕ деньги"
    );
    let repeat: Vec<u64> = {
        let mut ex2 = BacktestExchange::new(table(), fee_sched(), 42);
        ex2.on_event(&snap(1, 1_000, "BTCUSDT"));
        for _ in 0..4 {
            ex2.submit(maker("BTCUSDT", 100.0, 1.0)).expect("submit");
        }
        ids(&ex2.on_event(&trade(2, 1_050, "BTCUSDT", 100.0, 3.5)))
    };
    assert_eq!(
        seen, repeat,
        "DET-I-3: дефицитный сценарий недетерминирован между экземплярами"
    );

    // МАСШТАБ: 64 ордера. Вероятность, что обход хэш-контейнера СЛУЧАЙНО совпадёт с
    // возрастанием id, — 1/64! ≈ 0. Зелёный здесь означает порядок по построению, не удачу.
    const BIG: u64 = 64;
    assert_eq!(
        ids(&fills_on_single_tick(BIG, &[])),
        (1..=BIG).collect::<Vec<u64>>(),
        "DET-I-3: на {BIG} одновременных филлах порядок не по возрастанию order_id"
    );
}
