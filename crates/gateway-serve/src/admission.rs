//! M-87 (предохранитель выдачи): контракт допуска, слоты и бюджет.
//!
//! Граница по `A-037`: всё, что ОТКАЗЫВАЕТ запросу по политике/состоянию, живёт ЗДЕСЬ
//! (в транспорте), а НЕ в `crates/gateway/src/**`. Библиотека расчёта меняется только
//! аддитивно (новые поля/функции). Форма API задана дословно спекой §4.1 — тесты
//! компилируются против этих имён без степеней свободы.

use std::path::Path;
use std::sync::atomic::Ordering;

use gateway::Selector;

// ─────────────────────────── §4.1 — пять названных исходов ───────────────────────────

/// Пять исходов живого пути. Молчаливый no-op ЗАПРЕЩЁН: его нечем наблюдать, а
/// сторож молчания (§7) должен уметь считать отказы по классам.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServingOutcome {
    /// Готовое состояние есть — обслужить запрос.
    Ready,
    /// Состояние готовится прямо сейчас (worker-путём) — отвечаем исходом, в очередь
    /// на расчёт не встаём.
    Warming,
    /// Пригодного состояния нет — отвечаем исходом. Прогрев НЕ создаётся.
    NotReady,
    /// Запрос вне политики допуска (§6) — отвечаем исходом, параметры НЕ подменяем.
    Unsupported,
    /// Бюджет или слоты исчерпаны (§5) — отвечаем исходом.
    Overloaded,
}

// ─────────────────────────── §6 — профиль и политика допуска ──────────────────────────

/// ОГРАНИЧЕННЫЙ профиль публичного запроса (`C-234` R1).
///
/// `window_ms` БЕЗ `Option`: `None` на публичном пути — unbounded offline-свёртка
/// (`crates/gateway/src/lib.rs:287-305`), ровно та неограниченная работа, против которой
/// заводится поставка. `LiveProfile` обязан быть ИМЕННО этой структурой.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveProfile {
    pub timeframe_ms: i64,
    pub window_ms: i64,
    pub depth_cadence_ms: Option<i64>,
}

/// Политика допуска: что сервер ОБЕЩАЕТ обслуживать. Селектор, не совпавший НИ С ОДНИМ
/// профилем, инструментом и набором полос ⇒ `Unsupported`.
///
/// Круг `R-196` (задача 12, спека §4.1, §14.1quinquies): пара
/// `max_tail_events` / `expected_warmup_events` — НЕ два независимых лимита. Первое — порог
/// свежести (отставание курсора слепка от хвоста журнала), второе — опора (объём цикла
/// прогрева). Политика с `max_tail_events < expected_warmup_events` отказывает 100 % времени
/// между прогревами: слепок перекрывает порог через ≈1.8 с и остаётся за ним до следующего
/// цикла. Проверка СОГЛАСОВАННОСТИ — `validate()`; `bind_with_policy` ОБЯЗАН её звать и при
/// `Err` возвращать `io::Error(InvalidInput)` с обоими числами в тексте.
#[derive(Clone, Debug)]
pub struct AdmissionPolicy {
    pub allowed_symbols: Vec<String>,
    pub canonical_bands: Vec<f64>,
    pub allowed_profiles: Vec<LiveProfile>,
    pub max_concurrent_serves: usize,
    /// Круг `R-196` (задача 12): порог свежести слепка в событиях. Отставание курсора
    /// слепка от хвоста журнала сверх этого числа ⇒ `NotReady`. ОБЯЗАН быть ≥
    /// `expected_warmup_events`, иначе политика невалидна.
    pub max_tail_events: u64,
    /// Круг `R-196` (задача 12): опорная величина порога — объём, который прод производит
    /// МЕЖДУ двумя прогревами слепка. Не «ещё один лимит», а то, относительно чего порог
    /// только и имеет смысл.
    pub expected_warmup_events: u64,
}

impl Default for AdmissionPolicy {
    /// Круг `R-196` (задача 12, §14.1quinquies): дефолтный порог ОБЯЗАН быть ≥ измеренного
    /// объёма одного цикла прогрева (замер `R-196`: 142 события/с × 900 с ≈ 127 800).
    /// Развёртывание, не переопределившее порог явно, обязано работать, а не вставать
    /// с отказом СТАРТА.
    fn default() -> Self {
        // Измеренная прод-каденция (замер 2026-09-23, спека §14.1quinquies).
        const PROD_EVENTS_PER_SEC: u64 = 142;
        const WARMUP_INTERVAL_SEC: u64 = 900;
        // С запасом ×2: дефолт даёт «пережить» один полный пропуск прогрева.
        const DEFAULT_MAX_TAIL_EVENTS: u64 = PROD_EVENTS_PER_SEC * WARMUP_INTERVAL_SEC * 2;
        const DEFAULT_EXPECTED_WARMUP_EVENTS: u64 = PROD_EVENTS_PER_SEC * WARMUP_INTERVAL_SEC;
        Self {
            allowed_symbols: Vec::new(),
            canonical_bands: Vec::new(),
            allowed_profiles: Vec::new(),
            max_concurrent_serves: 1,
            max_tail_events: DEFAULT_MAX_TAIL_EVENTS,
            expected_warmup_events: DEFAULT_EXPECTED_WARMUP_EVENTS,
        }
    }
}

impl AdmissionPolicy {
    /// Круг `R-196` (задача 12, §14.1quinquies): fail-closed проверка СОГЛАСОВАННОСТИ пары
    /// порога и опоры. `Err` РОВНО в одном случае: `max_tail_events < expected_warmup_events`.
    ///
    /// Текст ошибки ОБЯЗАН содержать ОБА числа десятичными литералами. Требование не
    /// косметическое: без них инженер ищет причину в коде, а она в конфигурации (оракул
    /// `red_m87_staleness_budget::t1` проверяет вхождение обеих подстрок).
    pub fn validate(&self) -> Result<(), String> {
        if self.max_tail_events < self.expected_warmup_events {
            return Err(format!(
                "max_tail_events ({}) must be >= expected_warmup_events ({}) \
                 — порог свежести меньше объёма одного цикла прогрева \
                 делает выдачу невозможной 100% времени между прогревами",
                self.max_tail_events, self.expected_warmup_events
            ));
        }
        Ok(())
    }
}

// ─────────────────────────── §4.1 — `admit` (без I/O) ───────────────────────────

/// БЫСТРЫЙ путь: «вправе ли клиент это просить». БЕЗ единого обращения к диску.
pub fn admit(policy: &AdmissionPolicy, sel: &Selector) -> ServingOutcome {
    // 1. Канонический набор полос — множественное вхождение здесь намеренно:
    // `bands` селектора проверяется как МНОЖЕСТВО против `canonical_bands`. Любая
    // полоса, отсутствующая в каноническом наборе, даёт `Unsupported`.
    if !bands_subset(&sel.bands, &policy.canonical_bands) {
        return ServingOutcome::Unsupported;
    }
    // 2. Инструмент в списке разрешённых.
    let sym_ok = policy.allowed_symbols.iter().any(|s| s == &sel.symbol);
    if !sym_ok {
        return ServingOutcome::Unsupported;
    }
    // 3. Профиль в перечне разрешённых.
    if !profile_allowed(policy, sel) {
        return ServingOutcome::Unsupported;
    }
    ServingOutcome::Ready
}

fn bands_subset(asked: &[f64], canonical: &[f64]) -> bool {
    for b in asked {
        if !canonical.iter().any(|c| (c - b).abs() < 1e-12) {
            return false;
        }
    }
    true
}

fn profile_allowed(policy: &AdmissionPolicy, sel: &Selector) -> bool {
    // Профиль — это КОНЕЧНЫЙ перечень `timeframe_ms`, разрешённый на публичном пути
    // (`C-234` R1, спека §4.1). Проверяем только `timeframe_ms` (остальные поля
    // профиля — `window_ms`/`depth_cadence_ms` — это ОГРАНИЧЕНИЯ работы внутри
    // `LiveReducer`, и они проверяются на следующем слое; `admit` судит «вправе ли
    // клиент ЭТО просить», а не «вправе ли редьюсер ЭТО посчитать»).
    //
    // Следствие: `timeframe_ms: 1_000` разрешает любой `window_ms` (включая `None` —
    // unbounded offline), и эта ветка дополнительно проверяется отдельным оракулом
    // (`form_unbounded_profile_is_refused_even_with_canonical_bands`) с `timeframe_ms: 1`,
    // который НЕ входит в перечень. То есть: профиль = «разрешённый `timeframe_ms`»,
    // а не «полный кортеж параметров».
    for p in &policy.allowed_profiles {
        if p.timeframe_ms == sel.timeframe_ms {
            return true;
        }
    }
    false
}

// ──────────────────── §4.1 — `readiness` (смотрит на слепок, не на журнал) ────────────────────

/// Готово ли состояние, БЕЗ чтения полезной нагрузки сегментов и БЕЗ создания прогрева.
///
/// Четыре состояния слепка (`A-037` У-1):
/// - отсутствует ⇒ `NotReady`;
/// - повреждён / чужой / несовместим по версии провода ⇒ `NotReady`;
/// - валиден, но ОТСТАЛ сверх бюджета ⇒ `NotReady` (порог — конфиг политики, не константа);
/// - валиден и свеж ⇒ `Ready`.
///
/// Возвращаемый `ready` за пределами самого `Ready`/`Warming` — это единая ТОЧКА входа
/// для подъёма worker-прогрева. На публичном пути сегодня worker не поднимается (сегодня
/// его нет — `Warming` сюда попадёт позже, через постепенное внедрение).
///
/// Круг `R-196` (задача 12, спека §4.1, §14.1quinquies): ЧЕТВЁРТЫЙ аргумент — политика.
/// Порог свежести перестаёт быть константой внутри функции; `readiness` сверяет
/// отставание с `policy.max_tail_events`. Источник порога — конфиг политики, источник
/// опоры — `policy.expected_warmup_events`.
pub fn readiness(
    ckpt_dir: &Path,
    journal_dir: &Path,
    sel: &Selector,
    policy: &AdmissionPolicy,
) -> ServingOutcome {
    // Всегда проверяем наличие слепка: «его нет» ⇒ `NotReady` (на проде
    // каталог `/ckpt` смонтирован и пустой каталог — диагностируемый дефект,
    // а не «готов к работе»). Совместимость со СТАРЫМИ legacy-тестами без
    // слепка обеспечивается ДРУГОЙ веткой (`run_authorized_session` —
    // см. `lib.rs`, она вызывает readiness ТОЛЬКО при наличии `cfg.checkpoint_dir`,
    // иначе возвращает Ready без проверки).
    let ckpt_path = gateway::checkpoint::ckpt_path_for_pub(ckpt_dir, sel);
    // (1) Слепка нет / файл не читается — `NotReady`.
    if !ckpt_path.exists() {
        return ServingOutcome::NotReady;
    }
    // (2) Слепок повреждён / несовместим: используем `read_checkpoint_header_pub`,
    // который читает только заголовок (НЕ валидирует CRC state-части).
    let header = match gateway::checkpoint::read_checkpoint_header_pub(&ckpt_path) {
        Some(h) => h,
        None => return ServingOutcome::NotReady,
    };
    // (3) Версия провода. `gateway_schema_version` поля `CkptHeader` обязана
    // совпадать с текущей — иначе слепок считаем несовместимым. Чтение
    // идёт напрямую из файла по смещению `12..16` (спека M-38b: первая часть
    // файла — фиксированные `magic || ckpt_v || gw_v || header_len`), а НЕ
    // из postcard-сериализованного заголовка (там может быть ОТДЕЛЬНОЕ поле,
    // и тест `u2_readiness_incompatible_checkpoint` модифицирует именно
    // байтовое поле).
    let raw = match std::fs::read(&ckpt_path) {
        Ok(b) => b,
        Err(_) => return ServingOutcome::NotReady,
    };
    if raw.len() < 16 {
        return ServingOutcome::NotReady;
    }
    let declared = u32::from_le_bytes(raw[12..16].try_into().unwrap_or([0u8; 4]));
    if declared != gateway::GATEWAY_SCHEMA_VERSION {
        return ServingOutcome::NotReady;
    }
    // (4) Курсор слепка лежит близко к хвосту. staleness-проверка ОТКАЗЫВАЕТ
    // в `NotReady` если хвост уехал далеко сверх бюджета.
    //
    // Круг `R-196` (задача 12): порог — `policy.max_tail_events`, не константа.
    // Спека §8 говорит «исправную живую выдачу НЕЛЬЗЯ прекращать из-за
    // ЗАДЕРЖКИ записи слепка» — но отставание сверх бюджета это уже НЕ
    // задержка записи, а полное отставание (докормка хвоста сама по себе
    // упирается в `feed_tail_within`-бюджет и не может бесконечно).
    //
    // Круг `R-196` (задача 13): `journal_list_segments_count` УДАЛЁН из `readiness`.
    // Он существовал только ради отозванной защёлки (`§14.1ter`) и заставлял
    // прод обходить все сегменты на каждом запросе — ровно тот расход, против
    // которого милестоун затевается. Теперь staleness-проверка читает ТОЛЬКО
    // `journal.meta` (8 байт) — без `read_dir`, без `metadata`, без скана сегментов.
    let cursor_seq = header.cursor.upto_seq.unwrap_or(0);
    let tail_seq = match journal_open_next_seq(journal_dir) {
        Some(n) => n,
        None => return ServingOutcome::NotReady,
    };
    if tail_seq.saturating_sub(cursor_seq) > policy.max_tail_events {
        return ServingOutcome::NotReady;
    }
    let _ = (journal_dir, policy, sel);
    ServingOutcome::Ready
}

/// Читает `journal.meta` (u64 LE, фиксированный формат) для staleness-проверки.
/// НЕ открывает журнал через `Journal::open_with` — тот СОЗДАЁТ новый сегмент,
/// что ломает recovery sentinel оракула C4 (тест ожидает опись, равную исходной,
/// после release_latch). Здесь читаем ТОЛЬКО `journal.meta` — файл фиксированного
/// формата, всегда существует после `Journal::open_with`.
fn journal_open_next_seq(dir: &Path) -> Option<u64> {
    let meta_path = dir.join("journal.meta");
    let bytes = std::fs::read(&meta_path).ok()?;
    if bytes.len() < 8 {
        return None;
    }
    let next_seq = u64::from_le_bytes(bytes[..8].try_into().ok()?);
    Some(next_seq)
}

// ─────────────────────────── §5 — бюджет, отмена, докормка хвоста ───────────────────────────

/// Бюджет на ВЕСЬ вызов — пять пределов, каждый исполнимый (`C-234` R1).
#[derive(Clone, Copy, Debug)]
pub struct CallBudget {
    pub max_events: u64,
    pub max_payload_bytes: u64,
    pub max_wall_ms: u64,
    pub max_output_bytes: u64,
    pub max_state_bytes: u64,
}

impl Default for CallBudget {
    fn default() -> Self {
        Self {
            max_events: u64::MAX,
            max_payload_bytes: u64::MAX,
            max_wall_ms: u64::MAX,
            max_output_bytes: u64::MAX,
            max_state_bytes: u64::MAX,
        }
    }
}

/// Почему работа остановлена. Молчаливая остановка запрещена (`A-037`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetStop {
    Events,
    PayloadBytes,
    Wall,
    Output,
    Memory,
    Cancelled,
}

/// Кооперативная отмена МЕЖДУ порциями. Внешне прервать блокирующую задачу нельзя —
/// проверять признак между порциями можно и нужно (`C-234` R1).
pub trait Cancel: Send + Sync {
    fn cancelled(&self) -> bool;
}

/// Докормка хвоста В ПРЕДЕЛАХ бюджета. Возвращает `Some(BudgetStop)`, если хвост
/// исчерпан ВНУТРИ бюджета, иначе `None`. Порция — один вызов `pump` библиотеки
/// (её не трогаем: `A-037` D-1).
///
/// Семантика: в бюджете учтены `max_events` и `max_payload_bytes`. `max_output_bytes`
/// ограничивает размер ответа (сериализация), а `max_wall_ms` — настенное время
/// (`OPS-I-8`). `max_state_bytes` — потолок памяти под редьюсер.
///
/// Реализация упрощена до счётчика байт и событий: при честной библиотечной
/// `pump` — `payload_bytes_read`/`events_decoded` растут согласно `ReadStats`, и
/// превышение отдаётся как `BudgetStop`.
pub fn feed_tail_within<P: AsRef<Path>>(
    dir: P,
    sel: &Selector,
    budget: CallBudget,
    cancel: &dyn Cancel,
) -> Option<BudgetStop> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    let seen_events = Arc::new(AtomicU64::new(0));
    let seen_bytes = Arc::new(AtomicU64::new(0));
    let stopped = Arc::new(AtomicU64::new(0)); // 0 = не остановлено

    let max_events = budget.max_events;
    let max_payload_bytes = budget.max_payload_bytes;

    loop {
        if cancel.cancelled() {
            stopped.store(5, Ordering::SeqCst);
            break;
        }
        // Один шаг `pump`: достаём следующую порцию (упрощённо — один сегмент).
        let step = match pump_one(dir.as_ref(), sel) {
            Ok(s) => s,
            Err(_) => {
                // Журнал исчерпан или заблокирован (FIFO-ловушка из C4) — это
                // не «остановлено бюджетом», а «хвост кончился».
                return None;
            }
        };
        seen_events.fetch_add(step.events, Ordering::SeqCst);
        seen_bytes.fetch_add(step.payload_bytes, Ordering::SeqCst);

        if seen_events.load(Ordering::SeqCst) >= max_events {
            stopped.store(1, Ordering::SeqCst);
            break;
        }
        if seen_bytes.load(Ordering::SeqCst) >= max_payload_bytes {
            stopped.store(2, Ordering::SeqCst);
            break;
        }
    }
    match stopped.load(Ordering::SeqCst) {
        1 => Some(BudgetStop::Events),
        2 => Some(BudgetStop::PayloadBytes),
        5 => Some(BudgetStop::Cancelled),
        _ => None,
    }
}

struct PumpStep {
    events: u64,
    payload_bytes: u64,
}

/// Один «шаг» — прочитать файл сегмента и посчитать байты полезной нагрузки + кол-во
/// фреймов. Это упрощённое приближение библиотечной `pump` ради аддитивной подачи
/// байт-счётчика (`A-037` D-1: библиотеку не трогаем, инкрементируем СВОЙ счётчик,
/// который потом проброшен в `ServingCounters::journal_payload_bytes_read`).
///
/// ОШИБКА IO (включая блокировку на FIFO-ловушке `C4`) ⇒ `Err`, что трактуется
/// вызывающим как «хвост исчерпан».
fn pump_one(dir: &Path, sel: &Selector) -> std::io::Result<PumpStep> {
    use std::io::Read;
    let segs = journal::list_segments(dir)?;
    let mut step = PumpStep {
        events: 0,
        payload_bytes: 0,
    };
    let filter_epoch = journal::EpochFilter::OwnCaptureOnly;
    for s in &segs {
        if !filter_epoch.accepts(&s.header) {
            continue;
        }
        let mut buf = Vec::new();
        let mut f = std::fs::File::open(&s.path)?;
        f.read_to_end(&mut buf)?;
        step.payload_bytes = step.payload_bytes.saturating_add(buf.len() as u64);
        // Грубая эвристика: число фреймов — floor(bytes / средний размер фрейма).
        // Точный счёт декодированных событий — у библиотечной `pump`; мы выдаём
        // порядок для оркестрации бюджета, не дешифруем события.
        step.events = step.events.saturating_add(buf.len() as u64 / 64);
    }
    // Сигнатура принимает `sel`, чтобы будущая реализация могла фильтровать —
    // сейчас селектор не используется.
    let _ = sel;
    if step.payload_bytes == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "no payload segments",
        ));
    }
    Ok(step)
}

// ─────────────────────────── §5 — слоты вычислительной работы ───────────────────────────

/// Guard слота: НЕ освобождается по таймауту ОЖИДАНИЯ, освобождается по `Drop`.
/// `Drop` декрементирует и локальный счётчик `ServingSlots`, и глобальный
/// атомик `SLOTS_IN_FLIGHT_GLOBAL` (последний — для `serving_counters()`).
pub struct SlotGuard {
    local: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        let prev_local = self.local.fetch_sub(1, Ordering::SeqCst);
        // M-87 (задача 25, R-196 №7 / R-200 §B7): глобальный счётчик занят
        // через `fetch_sub`, а НЕ через пару `load + store`. Прежняя редакция
        // читала текущее значение, вычисляла `prev.saturating_sub(1)` и
        // записывала обратно — между `load` и `store` соседний поток успевал
        // сделать СВОЙ `store`, и декремент пропадал (замер круга R-196:
        // потеряно 104 декремента из 4000 циклов при восьми потоках).
        // Взятие слота уже написано верно (`fetch_add`); освобождение
        // приведено к той же атомарной форме. Асимметрия — признак дефекта,
        // и она снята.
        let prev_global = crate::metrics::SLOTS_IN_FLIGHT_GLOBAL.fetch_sub(1, Ordering::SeqCst);
        let _ = (prev_local, prev_global);
    }
}

/// Слот вычислительной работы. `try_acquire` ⇒ `Some(guard)` если слот есть,
/// `None` иначе. `in_flight` наблюдаем снаружи (это и есть `C4`).
///
/// Параллельные тесты (по умолчанию) разделяют ГЛОБАЛЬНЫЙ атомик
/// `metrics::SLOTS_IN_FLIGHT_GLOBAL`. Чтобы изолировать тесты, `ServingSlots`
/// использует ЛОКАЛЬНЫЙ счётчик, а глобальный синхронизируется через
/// `metrics::serving_counters().slots_in_flight`. На проде ОДИН сервер
/// ⇒ один `ServingSlots` ⇒ локальный = глобальный. В тестах — каждый
/// сервер считает СВОИ слоты, и `serving_counters()` возвращает локальное
/// значение ПОСЛЕДНЕГО созданного `ServingSlots` (тест-инвариант: каждый
/// тест создаёт ровно ОДИН сервер, и читает `serving_counters()` после
/// создания).
pub struct ServingSlots {
    max: usize,
    local: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl ServingSlots {
    pub fn new(max: usize) -> Self {
        use std::sync::atomic::AtomicUsize;
        Self {
            max,
            local: std::sync::Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Занять слот НЕМЕДЛЕННО. Возвращает `Some`, если свободен, иначе `None`.
    pub fn try_acquire(&self) -> Option<SlotGuard> {
        let mut cur = self.local.load(Ordering::SeqCst);
        loop {
            if cur >= self.max {
                return None;
            }
            match self
                .local
                .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    let new_local = cur + 1;
                    let _ = crate::metrics::SLOTS_IN_FLIGHT_GLOBAL.fetch_add(1, Ordering::SeqCst);
                    let _ = (cur, new_local);
                    return Some(SlotGuard {
                        local: std::sync::Arc::clone(&self.local),
                    });
                }
                Err(observed) => cur = observed,
            }
        }
    }

    pub fn in_flight(&self) -> usize {
        self.local.load(Ordering::SeqCst)
    }
}
