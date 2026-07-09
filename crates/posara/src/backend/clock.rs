// Time seam. Portable code reads time through Clock; web_time (std on desktop,
// performance.now on wasm) lives only here.
pub trait Clock {
    fn now_ms(&self) -> u64;
}

pub struct SystemClock {
    epoch: web_time::Instant,
}

impl SystemClock {
    pub fn new() -> Self { Self { epoch: web_time::Instant::now() } }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 { self.epoch.elapsed().as_millis() as u64 }
}
