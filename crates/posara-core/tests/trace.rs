use posara_core::sfx::diag::*;
use posara_core::trace;
use std::sync::atomic::{AtomicU32, Ordering};
static CALLS: AtomicU32 = AtomicU32::new(0);
fn sink(_: Level, event: Event) {
    assert_eq!(event.kind, 42);
    CALLS.fetch_add(1, Ordering::Relaxed);
}
#[test]
fn filtering_lazy_payload_and_single_registration() {
    assert_eq!(std::mem::size_of::<Event>(), 56);
    assert!(install(sink));
    assert!(!install(sink));
    let evaluated = AtomicU32::new(0);
    let levels = [
        Level::Error,
        Level::Warn,
        Level::Info,
        Level::Debug,
        Level::Trace,
    ];
    let mut expected = 0;
    for threshold in [
        None,
        Some(Level::Error),
        Some(Level::Info),
        Some(Level::Trace),
    ] {
        set_level(threshold);
        for level in levels {
            trace!(level, {
                evaluated.fetch_add(1, Ordering::Relaxed);
                Event {
                    kind: 42,
                    ..Event::default()
                }
            });
            if cfg!(feature = "diag") && threshold.is_some_and(|t| level as u32 <= t as u32) {
                expected += 1;
            }
            assert_eq!(evaluated.load(Ordering::Relaxed), expected);
            assert_eq!(CALLS.load(Ordering::Relaxed), expected);
        }
    }
}
