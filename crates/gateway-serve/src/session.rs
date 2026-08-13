//! M-65 v1 session — типы данных и валидатор селектора.
//!
//! ## Историческое замечание (M-65 round 2, М-1/М-2 по `R-057`)
//!
//! До round 2 этот модуль ДЕКЛАРИРОВАЛ структуру `Session` с методами `add`/`switch`/`remove`,
//! которые обещали инварианты («число subs ≤ max_subs», «удаление освобождает место СРАЗУ»,
//! «generation +1 на switch/remove»). Документация говорила «SACRED: per-connection»,
//! инварианты формулировались как гарантии модуля. **НИ ОДИН метод не вызывался из
//! исполняемого кода** (`grep -rn "Session::new\|session::Session::add" crates/` — пусто),
//! параллельная реализация жила в `lib.rs::SessionInner`. Мёртвая половина модуля
//! утверждала гарантии, которых на пути не было — ровно класс built-not-wired
//! (`gates.md` §4, инцидент `LiveReducer` M-38b→M-53).
//!
//! Round 2 устраняет расхождение вариантом «удалить мёртвую половину» (допустимо по
//! `R-057` §7 условие 4). Модуль сводится к типам данных (`Sub`) и валидатору
//! (`validate_selector`), которые lib.rs::SessionInner действительно использует.
//! Состояние подписок ЖИВЁТ в `lib.rs::SessionInner`; эта структура несёт полную
//! ответственность за инварианты — её документация описывает исполняемый код.
//!
//! ## Что осталось
//!
//! - `Sub` — одна подписка (id + selector + live + generation). Используется в типах
//!   `Result<...>` для `spawn_blocking`-pump'ов и в `BTreeMap<String, Sub>` карте
//!   подписок на соединении.
//! - `validate_selector` — чистая функция валидации `gateway::Selector` per
//!   `CT-RFC-09` §2.7 + `GW-I-10`. Вызывается ДО построения `LiveReducer` (быстрый
//!   путь без I/O; реальный селектор дополнительно проверяется в `LiveReducer::resume`).
//!
//! ## Concurrency
//!
//! Типы — POD (`Send + Sync` по полям). Блокирующие операции (`LiveReducer::resume`/
//! `pump`) выполняются в `spawn_blocking` вызывающей стороной, эта структура данных
//! только переносится через `'static`-границу.

use gateway::{validate_selector as gw_validate_selector, Selector};

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
    pub live: gateway::LiveReducer,
    /// Монотонный счётчик правок этого sub: 0 при создании, +1 при switch, +1 при remove.
    /// После завершения pump'a вызывающий сравнивает захваченное поколение с текущим
    /// (`Sub.generation` в карте) — расхождение = sub был заменён/удалён во время
    /// блокирующего чтения, результат выбрасывается.
    pub generation: u64,
}

/// Валидация селектора ДО построения `LiveReducer` (быстрый путь, без I/O).
/// `gateway::validate_selector` проверяет `timeframe_ms > 0` и `86_400_000 % timeframe_ms == 0`
/// (GW-I-10). Остальные проверки per `CT-RFC-09` §2.7 локальные.
///
/// Возвращает `Err(String)` с человеческим описанием причины (для лога и error-сообщения
/// клиенту по §2.7). Машиночитаемый код (`invalid_selector`) ставит вызывающий на
/// основании самого факта `Err`.
pub fn validate_selector(sel: &Selector) -> Result<(), String> {
    if sel.symbol.is_empty() {
        return Err("symbol is empty (CT-RFC-09 §2.7)".to_string());
    }
    // Bands: вне `(0, 1)`, не отсортированы или с дублями (§2.7, O-7).
    if sel.bands.is_empty() {
        return Err("bands is empty (CT-RFC-09 §2.7)".to_string());
    }
    let mut prev = None;
    for b in &sel.bands {
        if !(b > &0.0 && b < &1.0) {
            return Err(format!("band {b} вне (0, 1) (CT-RFC-09 §2.7)"));
        }
        if let Some(p) = prev {
            if b <= &p {
                return Err(format!(
                    "bands не отсортированы или содержат дубли: prev={p}, cur={b} (CT-RFC-09 §2.7)"
                ));
            }
        }
        prev = Some(*b);
    }
    // timeframe alignment (GW-I-10) — duplicate of `gateway::validate_selector` для
    // сообщения с человеческим текстом; двойное падение на той же ошибке безвредно.
    if sel.timeframe_ms <= 0 {
        return Err("timeframe_ms <= 0 (CT-RFC-09 §2.7)".to_string());
    }
    if 86_400_000 % sel.timeframe_ms != 0 {
        return Err(format!(
            "timeframe_ms={} не выравнен по UTC-суткам (CT-RFC-09 §2.7 / GW-I-10)",
            sel.timeframe_ms
        ));
    }
    // Проверка `gateway` для страховки (вдруг конвенция поменяется).
    gw_validate_selector(sel).map_err(|e| format!("gateway::validate_selector: {e}"))?;
    Ok(())
}
