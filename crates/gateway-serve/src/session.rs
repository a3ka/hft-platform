//! M-65 per-WS-session subscription state (CT-RFC-09 §2, `docs/rfc/CT-RFC-09-ws-session.md`).
//!
//! ## Структура
//!
//! - `Session` держит состояние подписок НА ОДНО WS-СОЕДИНЕНИЕ (`F-035-2`,
//!   `M-65-ws-session.md` §5): `sub id` одного соединения не имеет смысла в другом.
//!   Per-connection map ключей — `String` (client-assigned, §2.2), не глобальный.
//! - `Sub` несёт свой `Selector` + свой `LiveReducer`. Каждый `pump_all` пробегает все subs;
//!   следующий тик снова проходит всех.
//!
//! ## Инварианты (спека §4.1, оракулы `O-2, O-4, O-9, O-10`)
//!
//! - Число subs ≤ `max_subs` (cap fail-closed, `O-4`). Превышение ⇒ `Err(AddError::CapExceeded)`;
//!   `add` НИКОГДА не удаляет старый sub при превышении.
//! - Каждый sub имеет валидный selector на момент добавления (`O-7`). Невалидный ⇒
//!   `Err(AddError::InvalidSelector(..))`; sub не создаётся.
//! - Удаление подписки освобождает место СРАЗУ (`O-10`). Один `remove` уменьшает счётчик на 1;
//!   следующий `add` может встать в эту ячейку.
//! - Удаление несуществующего id ⇒ `Err(RemoveError::UnknownId(..))`. Повторный `unsubscribe`
//!   не молчит (`O-10`, спека §4.2).
//!
//! ## Concurrency
//!
//! `Session` синхронный (`std::sync::Mutex` на стороне вызывающего; см.
//! `server::run_v1_session`). Блокирующие операции (resume/pump) выполняются ВЫЗЫВАЮЩИМ в
//! `spawn_blocking` — тип содержит только данные (`'static`-safe, читается в `LiveReducer::pump`).

use std::collections::HashMap;
use std::path::PathBuf;

use gateway::{validate_selector, LiveReducer, Selector, Snapshot};
use journal::EpochFilter;

/// Ошибка при попытке добавить новую подписку. Sub в карту НЕ кладётся.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddError {
    /// Превышение лимита `max_subscriptions_per_connection` (`CT-RFC-09` §2.6, `O-4`).
    /// Текущее число подписок — `current`, лимит — `max`.
    CapExceeded { current: usize, max: usize },
    /// Селектор не прошёл валидацию (`O-7`, §2.7): пустой/невалидный символ, `bands` вне
    /// `(0, 1)` / не отсортированы / с дублями, или `gateway::validate_selector` (GW-I-10:
    /// `timeframe_ms` не выравнен по UTC-суткам). Текст — для лога; машиночитаемая ветка —
    /// сам `AddError::InvalidSelector`, код ошибки в WS — `invalid_selector` (§2.7).
    InvalidSelector(String),
    /// Subscribe с id, который УЖЕ существует, через `add`. Для смены селектора у
    /// существующего id вызывающий должен звать `switch`, а не `add` — иначе конфликт
    /// с cap'ой (добавить sub с существующим id нельзя: место занято, а перезаписать без
    /// подтверждения изменения селектора было бы тихим switch'ем).
    DuplicateId(String),
}

/// Ошибка при попытке удалить подписку (спека §4.2, `O-10`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveError {
    /// `unsubscribe` неизвестного или уже снятого id. Код в WS — `unknown_id`.
    UnknownId(String),
}

/// Запись одной подписки на соединении: id + selector + live-редьюсер + generation.
///
/// M-65 (race fix, `O-10` / `F-035-2`): `generation` инкрементируется при каждой
/// СМЕНЕ селектора (switch) или удалении (remove). pump-задача в `spawn_blocking`
/// захватывает generation при старте; при завершении — сверяет. Если generation
/// изменился (был switch/remove пока pump работал), результат pump'a ОТБРАСЫВАЕТСЯ,
/// и sub возвращается ТОЛЬКО если в карте сейчас sub с тем же id И той же generation
/// (= pump пришёл «от того же» sub'а, кто уже в карте). Без generation'а старый
/// pump затирал бы новый sub при switch'е (карта `insert`-replace по key), и
/// `unsubscribe`, выданный во время in-flight pump'a, восстанавливал бы sub — оба
/// случая нарушают §4.1 («то, что клиент получает, определяется его ТЕКУЩИМ множеством
/// подписок и ничем иным»).
pub struct Sub {
    pub id: String,
    pub selector: Selector,
    pub live: LiveReducer,
    /// Монотонный счётчик правок этого sub: 0 при создании, +1 при switch, +1 при remove.
    /// После завершения pump'a вызывающий сравнивает захваченное поколение с текущим
    /// (`Sub.generation` в карте) — расхождение = sub был заменён/удалён во время
    /// блокирующего чтения, результат выбрасывается.
    pub generation: u64,
}

/// Состояние одного WS-соединения (SACRED: per-connection). Гонки подписок разных соединений
/// — НЕ существуют: у каждого соединения свой экземпляр `Session`.
pub struct Session {
    subs: HashMap<String, Sub>,
    max_subs: usize,
    journal_dir: PathBuf,
    filter: EpochFilter,
    ckpt_dir: PathBuf,
}

impl Session {
    pub fn new(
        journal_dir: PathBuf,
        filter: EpochFilter,
        ckpt_dir: PathBuf,
        max_subs: usize,
    ) -> Self {
        Self {
            subs: HashMap::new(),
            max_subs,
            journal_dir,
            filter,
            ckpt_dir,
        }
    }

    pub fn has(&self, id: &str) -> bool {
        self.subs.contains_key(id)
    }
    pub fn len(&self) -> usize {
        self.subs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.subs.is_empty()
    }
    pub fn max_subs(&self) -> usize {
        self.max_subs
    }

    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.subs.keys()
    }

    /// Итератор по `Sub` для pump-цикла (порядок не гарантируется, но pump'ы
    /// независимы — порядок неважен для корректности).
    pub fn subs(&self) -> impl Iterator<Item = &Sub> {
        self.subs.values()
    }

    /// Валидация селектора ДО построения `LiveReducer` (быстрый путь, без I/O).
    /// `gateway::validate_selector` проверяет `timeframe_ms > 0` и `86_400_000 % timeframe_ms == 0`
    /// (GW-I-10). Остальные проверки per `CT-RFC-09` §2.7 локальные.
    pub fn validate_selector_local(sel: &Selector) -> Result<(), AddError> {
        if sel.symbol.is_empty() {
            return Err(AddError::InvalidSelector(
                "symbol is empty (CT-RFC-09 §2.7)".to_string(),
            ));
        }
        // Bands: вне `(0, 1)`, не отсортированы или с дублями (§2.7, O-7).
        if sel.bands.is_empty() {
            return Err(AddError::InvalidSelector(
                "bands is empty (CT-RFC-09 §2.7)".to_string(),
            ));
        }
        let mut prev = None;
        for b in &sel.bands {
            if !(b > &0.0 && b < &1.0) {
                return Err(AddError::InvalidSelector(format!(
                    "band {b} вне (0, 1) (CT-RFC-09 §2.7)"
                )));
            }
            if let Some(p) = prev {
                if b <= &p {
                    return Err(AddError::InvalidSelector(format!(
                        "bands не отсортированы или содержат дубли: prev={p}, cur={b} (CT-RFC-09 §2.7)"
                    )));
                }
            }
            prev = Some(*b);
        }
        // timeframe alignment (GW-I-10) — duplicate of `gateway::validate_selector` для
        // сообщения с человеческим текстом; двойное падение на той же ошибке безвредно.
        if sel.timeframe_ms <= 0 {
            return Err(AddError::InvalidSelector(
                "timeframe_ms <= 0 (CT-RFC-09 §2.7)".to_string(),
            ));
        }
        if 86_400_000 % sel.timeframe_ms != 0 {
            return Err(AddError::InvalidSelector(format!(
                "timeframe_ms={} не выравнен по UTC-суткам (CT-RFC-09 §2.7 / GW-I-10)",
                sel.timeframe_ms
            )));
        }
        // Проверка `gateway` для страховки (вдруг конвенция поменяется).
        validate_selector(sel)
            .map_err(|e| AddError::InvalidSelector(format!("gateway::validate_selector: {e}")))?;
        Ok(())
    }

    /// Добавить новую подписку с ДАННЫМ id (если id существует — `Err(DuplicateId)`;
    /// для смены селектора существующего id — `switch`).
    ///
    /// Возвращает `Snapshot` для первого кадра клиенту (`CT-RFC-09` §2.3: первое сообщение
    /// по новой подписке — snapshot).
    ///
    /// **Блокирующая операция внутри:** `LiveReducer::resume` читает журнал/чекпоинт. Вызывающий
    /// ОБЯЗАН обернуть вызов в `tokio::task::spawn_blocking`.
    pub fn add(&mut self, id: String, selector: Selector) -> Result<Snapshot, AddError> {
        if self.subs.contains_key(&id) {
            return Err(AddError::DuplicateId(id));
        }
        if self.subs.len() >= self.max_subs {
            return Err(AddError::CapExceeded {
                current: self.subs.len(),
                max: self.max_subs,
            });
        }
        Self::validate_selector_local(&selector)?;
        let (live, _stats) = LiveReducer::resume(
            &self.journal_dir,
            self.filter.clone(),
            &selector,
            self.ckpt_dir.as_path(),
        )
        .map_err(|e| AddError::InvalidSelector(format!("resume: {e}")))?;
        let snap = live.snapshot();
        self.subs.insert(
            id.clone(),
            Sub {
                id,
                selector,
                live,
                generation: 0,
            },
        );
        Ok(snap)
    }

    /// Переключить селектор существующего id. Старый `LiveReducer` дропается (всё накопленное
    /// per-selector состояние утрачено, новый стартует с новым селектором — §2.4 семантика).
    /// Generation инкрементируется — pump-задача, стартовавшая ДО switch'а, отбросит
    /// свой результат по завершении (защита от «кадр прежнего селектора после switch'а»
    /// при неудачном тайминге, `O-1`, спека §2.4 «никаких промежуточных кадров старого
    /// селектора после нового снапшота»).
    pub fn switch(&mut self, id: &str, new_selector: Selector) -> Result<Snapshot, AddError> {
        Self::validate_selector_local(&new_selector)?;
        let sub = self
            .subs
            .get_mut(id)
            .ok_or_else(|| AddError::InvalidSelector(format!("id {id:?} не найден для switch")))?;
        let (live, _stats) = LiveReducer::resume(
            &self.journal_dir,
            self.filter.clone(),
            &new_selector,
            self.ckpt_dir.as_path(),
        )
        .map_err(|e| AddError::InvalidSelector(format!("resume switch: {e}")))?;
        let snap = live.snapshot();
        sub.selector = new_selector;
        sub.live = live;
        sub.generation = sub.generation.wrapping_add(1);
        Ok(snap)
    }

    /// Удалить подписку. Возвращает `Err(UnknownId)` для несуществующего id (спека §4.2, `O-10`).
    /// Место под лимитом освобождается СРАЗУ при удалении — следующий `add` встанет в эту ячейку.
    /// Generation удалённого sub'а bump'ается — pump-задача, стартовавшая ДО remove'а,
    /// при завершении увидит несовпадение и не восстановит sub (иначе `unsubscribe` во время
    /// in-flight pump'a фактически отменялся бы следующим тиком).
    pub fn remove(&mut self, id: &str) -> Result<(), RemoveError> {
        if let Some(mut sub) = self.subs.remove(id) {
            sub.generation = sub.generation.wrapping_add(1);
            Ok(())
        } else {
            Err(RemoveError::UnknownId(id.to_string()))
        }
    }

    /// Селектор существующей подписки (для диагностики / сериализации ошибок).
    pub fn selector_of(&self, id: &str) -> Option<&Selector> {
        self.subs.get(id).map(|s| &s.selector)
    }
}
