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

impl Event {
    /// `observed_us` is stamped by the sink when the event is recorded.
    pub const fn new(kind: u32, epoch: u32, at_us: u32, related_us: u32, data: [u32; 9]) -> Self {
        Self { kind, epoch, at_us, related_us, observed_us: 0, data }
    }
}

pub type Sink = fn(Level, Event);
static SINK: AtomicUsize = AtomicUsize::new(0);
static FILTER: AtomicUsize = AtomicUsize::new(0);
pub fn install_filter(filter: fn(Level, u32) -> bool) -> bool {
    FILTER
        .compare_exchange(0, filter as usize, Ordering::Release, Ordering::Relaxed)
        .is_ok()
}
pub fn enabled_event(level: Level, kind: u32) -> bool {
    if !enabled(level) {
        return false;
    }
    let pointer = FILTER.load(Ordering::Acquire);
    if pointer == 0 {
        return true;
    }
    // The immutable slot is written only by install_filter from this exact function type.
    let filter: fn(Level, u32) -> bool = unsafe { core::mem::transmute(pointer) };
    filter(level, kind)
}
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
    if !enabled_event(level, event.kind) {
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
    ($level:expr, $kind:expr, $event:expr) => {{
        let level = $level;
        if $crate::sfx::diag::enabled_event(level, $kind) {
            $crate::sfx::diag::record(level, $event);
        }
    }};
    ($level:expr, $event:expr) => {{
        let level = $level;
        if $crate::sfx::diag::enabled(level) {
            $crate::sfx::diag::record(level, $event);
        }
    }};
}

// Without `diag` the arguments are still type-checked (so builds agree) but never evaluated.
#[cfg(not(feature = "diag"))]
#[macro_export]
macro_rules! trace {
    ($level:expr, $kind:expr, $event:expr) => {{
        if false { let _ = ($level, $kind, $event); }
    }};
    ($level:expr, $event:expr) => {{
        if false { let _ = ($level, $event); }
    }};
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Stage {
    Usb = 1,
    Spsc = 2,
    Synth = 3,
    Mixer = 4,
    Dma = 5,
    Imu = 6,
    Motion = 7,
    Power = 8,
    System = 9,
    Profile = 10,
}
