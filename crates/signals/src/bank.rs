//! SignalBank — mid-слой (FA §3): маршрутизирует каждый Event во все живые инстансы,
//! собирает SignalOut'ы, изолирует паники отдельного сигнала (SG-I-9).
//!
//! Реализация — signal-engineer (M-04 task 3).

use contracts::Event;

use crate::{Signal, SignalOut};

#[derive(Default)]
pub struct SignalBank {
    signals: Vec<Box<dyn Signal>>,
}

impl SignalBank {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, s: Box<dyn Signal>) {
        self.signals.push(s);
    }

    pub fn len(&self) -> usize {
        self.signals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }

    /// Каждому сигналу — каждый Event. Паника/ошибка ОДНОГО сигнала изолируется:
    /// его SignalOut на этом тике отсутствует, остальные и последующие события
    /// продолжают обрабатываться (SG-I-9). Никакого выдуманного значения.
    pub fn on_event(&mut self, ev: &Event) -> Vec<SignalOut> {
        let mut outs = Vec::with_capacity(self.signals.len());
        for s in self.signals.iter_mut() {
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.on_event(ev)));
            match caught {
                Ok(Some(out)) => outs.push(out),
                Ok(None) => {}
                Err(_) => {
                    // SG-I-9: паника изолирована на этот тик/этот сигнал; никакого
                    // выдуманного SignalOut; остальные сигналы и последующие события
                    // продолжают обрабатываться нормально.
                }
            }
        }
        outs
    }
}
