// Time seam. Portable code reads time through Clock; web_time (std on desktop,
// performance.now on wasm) lives only here.
pub trait Clock {
    fn now_ms(&self) -> u64;
    // Rebase now() to 0. Called when a cart (re)loads so now() is cart-start-
    // relative — carts assume it starts near 0 (web keeps one Host across runs).
    fn reset(&self) {}
}

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
