use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Level {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Event {
    pub kind: u32,
    pub epoch: u32,
    pub at_us: u32,
    pub related_us: u32,
    pub observed_us: u32,
    pub data: [u32; 9],
}

pub type Sink = fn(Level, Event);
static SINK: AtomicUsize = AtomicUsize::new(0);
static LEVEL: AtomicU32 = AtomicU32::new(0);

pub fn install(sink: Sink) -> bool {
    SINK.compare_exchange(0, sink as usize, Ordering::Release, Ordering::Relaxed)
        .is_ok()
}

pub fn set_level(level: Option<Level>) {
    LEVEL.store(level.map_or(0, |level| level as u32), Ordering::Relaxed);
}

#[inline]
pub fn enabled(level: Level) -> bool {
    cfg!(feature = "diag")
        && level as u32 <= LEVEL.load(Ordering::Relaxed)
        && SINK.load(Ordering::Acquire) != 0
}

pub fn record(level: Level, event: Event) {
    if !enabled(level) {
        return;
    }
    let pointer = SINK.load(Ordering::Acquire);
    // Only install writes this value, once, from a function pointer of type Sink.
    let sink: Sink = unsafe { core::mem::transmute(pointer) };
    sink(level, event);
}

#[cfg(feature = "diag")]
#[macro_export]
macro_rules! trace {
    ($level:expr, $event:expr) => {{
        let level = $level;
        if $crate::sfx::diag::enabled(level) {
            $crate::sfx::diag::record(level, $event);
        }
    }};
}

#[cfg(not(feature = "diag"))]
#[macro_export]
macro_rules! trace {
    ($level:expr, $event:expr) => {{}};
}
