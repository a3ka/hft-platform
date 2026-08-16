//! M-65 (round 3, R-086 §10.3): точка синхронизации для оракула на гонку
//! «switch × in-flight pump».
//!
//! ## Зачем (R-086 §10.3 / `milestones/M-65-ws-session.md` §10.3)
//!
//! Существующий оракул `o1` ЗАЯВЛЯЛ покрытие оси 4 значения «кадр в полёте приходит после
//! смены», но ГАСИЛ условие `_settle_before_switch`-паузой в 1100 мс — на tempdir-журнале из
//! нескольких событий pump занимает микросекунды, поэтому к моменту `subscribe` карта
//! заведомо полна и берётся ветка SWITCH (а не ADD). То есть заявка есть, исполнения нет.
//! На проде, где pump идёт секунды (`CT-RFC-09` §4 фиксирует 5096/6498/10263 ms против
//! `PUSH_INTERVAL_MS = 250`), in-flight подписка — НОРМА, а не краевой случай: гонка
//! воспроизводится СЕЙЧАС и каждый день.
//!
//! «Вероятностное» окно (sleep + флак, класс `TD-097`) — НЕприемлемо. Поэтому pump
//! сигналит «вошёл» и ЖДЁТ разрешения от теста; тест в этом окне исполняет действие
//! (switch) и даёт pump'у завершиться. Контракт — задача dev'а, активен ТОЛЬКО в
//! тестовой сборке (`#[cfg(test)]`); на прод-путь не влияет.
//!
//! ## Контракт
//!
//! 1. Тест ОБЯЗАН `arm(id)` ДО первой возможности pump'а (или вызов будет молча работать
//!    на прошлом состоянии channel'а). `arm` сбрасывает оба флага в начальное состояние.
//! 2. Pump (в `spawn_blocking`) вызывает `pump_signal_and_wait(id)`:
//!    - помечает `entered = true`, будит ждущий тест;
//!    - блокируется на Condvar до прихода `release == true`.
//!    Тест, перехвативший entered, ОБЯЗАН в какой-то момент вызвать `test_release(id)`.
//! 3. Тест вызывает `test_wait_for_pump(id, timeout)` — блокируется до `entered == true`
//!    или возвращает `false` по таймауту.
//! 4. Тест вызывает `test_remove(id)` ПОСЛЕ завершения сценария — чтобы не оставлять
//!    в глобальной карте мёртвые каналы для случайных будущих pump'ов с тем же `id`.
//!
//! ## Почему блокирующий wait внутри `spawn_blocking`
//!
//! `spawn_blocking`-задача ЗАВЕДОМО может блокироваться (для этого пул и создан);
//! `block_on` НЕ используется — мы не должны вешать tokio-worker. Condvar-вариант —
//! единственный, который одновременно (а) будит ждущий поток, (б) гарантирует
//! прогресс при `test_release` от теста в любом порядке относительно сети сообщений.
//!
//! ## Cleanup policy (BINDING)
//!
//! Каналы живут в `OnceLock<Mutex<HashMap<...>>>`. Test отвечает за вызов `test_remove`
//! после сценария: «test пропустил cleanup» ⇒ канал остаётся в map до конца процесса,
//! сбрасывается только через гонку с новым `test_arm`/`test_release`. Эффект — следующий
//! тест с тем же `id` увидит ПРОШЛОЕ состояние pump'а и поймает флак. Тонкий момент,
//! и потому явный: архитектор владеет оракулом, dev — механикой; не оставлять мёртвое
//! состояние — часть контракта.

#[cfg(test)]
pub mod rendezvous {
    use std::collections::HashMap;
    use std::sync::{Arc, Condvar, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    /// Один rendezvous-канал на id подписки.
    pub struct Channel {
        /// pump пометил «я вошёл» — тест ждёт на `entered_cv`.
        pub entered: Mutex<bool>,
        pub entered_cv: Condvar,
        /// test поднял «можно продолжить» — pump ждёт на `release_cv`.
        pub release: Mutex<bool>,
        pub release_cv: Condvar,
    }

    impl Channel {
        fn new() -> Self {
            Self {
                entered: Mutex::new(false),
                entered_cv: Condvar::new(),
                // Стартуем `true` — pump мог бы пройти насквозь, если test не успел
                // `arm`. `arm` сбрасывает в `false`, требуя явного release'а.
                release: Mutex::new(true),
                release_cv: Condvar::new(),
            }
        }
    }

    static CHANNELS: OnceLock<Mutex<HashMap<String, Arc<Channel>>>> = OnceLock::new();

    fn channels() -> &'static Mutex<HashMap<String, Arc<Channel>>> {
        CHANNELS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// Возвращает существующий канал или создаёт новый для `id`. Идемпотентно.
    fn channel_for(id: &str) -> Arc<Channel> {
        let mut map = channels().lock().expect("rendezvous: channels poisoned");
        map.entry(id.to_string())
            .or_insert_with(|| Arc::new(Channel::new()))
            .clone()
    }

    /// Подготовить канал для теста: сбросить `entered = false`, `release = false`
    /// (pump ОБЯЗАН ждать явного release). Вызывать ДО первой возможности pump'а.
    pub fn arm(id: &str) {
        let ch = channel_for(id);
        let mut entered_g = ch.entered.lock().expect("rendezvous: entered poisoned");
        *entered_g = false;
        drop(entered_g);
        let mut release_g = ch.release.lock().expect("rendezvous: release poisoned");
        *release_g = false;
        ch.release_cv.notify_all();
    }

    /// Pump: блокирующий wait, пока test не вызовет `test_release(id)` с тем же `id`.
    /// ДО этого — сигналит «вошёл» (test может дождаться и исполнить действие).
    ///
    /// BINDING: вызывается из `spawn_blocking`-замыкания. Никогда из tokio-worker'а —
    /// пул блокирующих потоков не должен быть узким местом. Один канал ⇒ одна ожидающая
    /// pump-задача на момент rendezvous; если их несколько — все ждут.
    pub fn pump_signal_and_wait(id: &str) {
        let ch = channel_for(id);
        // (1) Сигнал «вошёл»: поднять entered и разбудить ждущий test.
        {
            let mut entered_g = ch.entered.lock().expect("rendezvous: entered poisoned");
            *entered_g = true;
            ch.entered_cv.notify_all();
        }
        // (2) Ждать release == true. Condvar-цикл — стандартный паттерн против
        //     spurious wakeup и против состояния «release был true ровно между check и wait».
        let mut release_g = ch.release.lock().expect("rendezvous: release poisoned");
        while !*release_g {
            release_g = ch
                .release_cv
                .wait(release_g)
                .expect("rendezvous: wait failed");
        }
        // (3) Сбрасываем entered для возможного повторного rendezvous с тем же id
        //     в той же сессии. Сам `release` НЕ сбрасываем — это ответственность test'
        //     через `arm` (test, желающий повторить, вызовет `arm` снова).
        drop(release_g);
        let mut entered_g = ch.entered.lock().expect("rendezvous: entered poisoned");
        *entered_g = false;
    }

    /// Test: ждать, пока pump пометит «вошёл» (или таймаут). Возвращает `true`, если
    /// pump действительно вошёл, `false` — по таймауту (тест ОБЯЗАН явно решить, как
    /// реагировать: обычно — fail с подсказкой «pump не дошёл до rendezvous»).
    pub fn test_wait_for_pump(id: &str, timeout: Duration) -> bool {
        let ch = channel_for(id);
        let mut entered_g = ch.entered.lock().expect("rendezvous: entered poisoned");
        let deadline = Instant::now() + timeout;
        while !*entered_g {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let wait_for = deadline - now;
            let (g, _) = ch
                .entered_cv
                .wait_timeout(entered_g, wait_for)
                .expect("rendezvous: wait_timeout failed");
            entered_g = g;
        }
        true
    }

    /// Test: разрешить pump'у продолжить. Идемпотентно — повторный вызов без `arm` не
    /// меняет состояние.
    pub fn test_release(id: &str) {
        let ch = channel_for(id);
        let mut release_g = ch.release.lock().expect("rendezvous: release poisoned");
        *release_g = true;
        ch.release_cv.notify_all();
    }

    /// Test: убрать канал из глобальной карты ПОСЛЕ завершения сценария. Если следующий
    /// тест начнёт с тем же `id` без `arm`, он увидит свежий `Channel` со state'ом
    /// по умолчанию (`release = true`) — pump пройдёт насквозь, что эквивалентно
    /// «rendezvous отключён». Правильная последовательность для свежего сценария —
    /// `arm → scenario → release → remove`.
    pub fn test_remove(id: &str) {
        let mut map = channels().lock().expect("rendezvous: channels poisoned");
        map.remove(id);
    }
}
