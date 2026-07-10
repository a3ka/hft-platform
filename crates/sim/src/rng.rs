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
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Равномерное [0,1) из старших 53 бит next_u64.
    pub fn next_f64(&mut self) -> f64 {
        let v = self.next_u64();
        (v >> 11) as f64 / (1u64 << 53) as f64
    }
}
