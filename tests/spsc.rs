#![cfg(feature = "sfx")]

use posara::sfx::spsc::Spsc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn fifo_order() {
    let q = Spsc::new(4);
    for i in 0..4 { q.try_push(i).unwrap(); }
    for i in 0..4 { assert_eq!(q.try_pop(), Some(i)); }
    assert_eq!(q.try_pop(), None);
}

#[test]
fn push_full_returns_value() {
    let q = Spsc::new(2);
    q.try_push(1).unwrap();
    q.try_push(2).unwrap();
    assert_eq!(q.try_push(3), Err(3));
}

#[test]
fn pop_empty_is_none() {
    let q: Spsc<u8> = Spsc::new(2);
    assert_eq!(q.try_pop(), None);
}

#[test]
fn wraps_around_capacity() {
    let q = Spsc::new(3);
    // push 2 / pop 2 each round so the indices cross the cap boundary many times
    let mut next = 0;
    let mut expect = 0;
    for _ in 0..10 {
        q.try_push(next).unwrap();
        next += 1;
        q.try_push(next).unwrap();
        next += 1;
        assert_eq!(q.try_pop(), Some(expect));
        expect += 1;
        assert_eq!(q.try_pop(), Some(expect));
        expect += 1;
    }
    assert_eq!(q.try_pop(), None);
    assert_eq!(expect, next);
}

#[test]
fn drop_runs_for_unread_items() {
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) { DROPS.fetch_add(1, Ordering::Relaxed); }
    }
    {
        let q = Spsc::new(4);
        assert!(q.try_push(Bomb).is_ok());
        assert!(q.try_push(Bomb).is_ok());
        drop(q.try_pop()); // consume one explicitly
    }
    assert_eq!(DROPS.load(Ordering::Relaxed), 2);
}
