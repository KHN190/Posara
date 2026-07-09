// Lock-free single-producer single-consumer ring buffer.
// Producer: cart-facing native (main thread). Consumer: audio callback thread.
// Capacity is fixed at construction; push when full returns Err(value).
// Write-on-real-time-thread safety: no allocations, no locks, only atomics.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Spsc<T> {
    buf: Box<[UnsafeCell<MaybeUninit<T>>]>,
    cap: usize,
    head: AtomicUsize, // next write index (producer, monotonically increasing)
    tail: AtomicUsize, // next read index  (consumer, monotonically increasing)
}

// SAFETY: each slot is touched by at most one side at a time, gated by the
// head/tail handshake. T need only be Send for cross-thread transfer.
unsafe impl<T: Send> Send for Spsc<T> {}
unsafe impl<T: Send> Sync for Spsc<T> {}

impl<T> Spsc<T> {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "Spsc cap must be > 0");
        let mut v = Vec::with_capacity(cap);
        for _ in 0..cap { v.push(UnsafeCell::new(MaybeUninit::uninit())); }
        Self { buf: v.into_boxed_slice(), cap, head: AtomicUsize::new(0), tail: AtomicUsize::new(0) }
    }

    pub fn try_push(&self, v: T) -> Result<(), T> {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Acquire);
        if h.wrapping_sub(t) >= self.cap { return Err(v); }
        // SAFETY: producer owns slot at h until store(Release) below.
        unsafe { (*self.buf[h % self.cap].get()).write(v); }
        self.head.store(h.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    pub fn try_pop(&self) -> Option<T> {
        let t = self.tail.load(Ordering::Relaxed);
        let h = self.head.load(Ordering::Acquire);
        if t == h { return None; }
        // SAFETY: producer published this slot (Release above); consumer reads
        // and then releases the slot via tail store.
        let v = unsafe { (*self.buf[t % self.cap].get()).assume_init_read() };
        self.tail.store(t.wrapping_add(1), Ordering::Release);
        Some(v)
    }
}

impl<T> Drop for Spsc<T> {
    fn drop(&mut self) {
        while self.try_pop().is_some() {}
    }
}
