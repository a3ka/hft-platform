//! Детерминированный PRNG (D10): SplitMix64 (Steele et al.) — 64-бит state,
//! константы 0x9E3779B97F4A7C15 / 0xBF58476D1CE4E5B9 / 0x94D049BB133111EB.
//! Без внешних зависимостей: rand-крейт меняет алгоритмы между версиями — ломает
//! бит-идентичный реплей (SM-I-2). Реализация — engine-dev (M-04 task 2).

#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        let _ = &self.state;
        todo!("engine-dev: M-04 task 2")
    }

    /// Равномерное [0,1).
    pub fn next_f64(&mut self) -> f64 {
        todo!("engine-dev: M-04 task 2")
    }
}
