// web_time (std on desktop, performance.now on wasm) lives only here.
pub use posara_core::drivers::Clock;

use std::cell::Cell;

pub struct SystemClock {
    epoch: Cell<web_time::Instant>,
}

impl SystemClock {
    pub fn new() -> Self { Self { epoch: Cell::new(web_time::Instant::now()) } }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 { self.epoch.get().elapsed().as_millis() as u64 }
    fn reset(&self) { self.epoch.set(web_time::Instant::now()); }
}
