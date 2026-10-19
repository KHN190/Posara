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

#[cfg(not(loom))]
mod against_a_deque {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use posara::sfx::spsc::Spsc;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn any_push_pop_sequence_matches_a_bounded_deque(cap in 1usize..9, ops in prop::collection::vec(any::<bool>(), 0..200)) {
            let q = Spsc::new(cap);
            let mut model = VecDeque::new();
            for (i, push) in ops.into_iter().enumerate() {
                if push {
                    let full = model.len() == cap;
                    prop_assert_eq!(q.try_push(i), if full { Err(i) } else { Ok(()) });
                    if !full { model.push_back(i); }
                } else {
                    prop_assert_eq!(q.try_pop(), model.pop_front());
                }
            }
        }

        #[test]
        fn threads_hand_over_every_item_once_in_order(cap in 1usize..9, n in 0usize..2000, bursts in prop::collection::vec((1usize..20, 0u32..50), 1..16)) {
            let q = Arc::new(Spsc::new(cap));
            let tx = Arc::clone(&q);
            let plan = bursts.clone();
            let producer = std::thread::spawn(move || {
                let mut next = 0;
                for (burst, spin) in plan.iter().cycle() {
                    for _ in 0..*burst {
                        if next == n { return; }
                        while tx.try_push(next).is_err() { std::hint::spin_loop(); }
                        next += 1;
                    }
                    for _ in 0..*spin { std::hint::spin_loop(); }
                }
            });
            let mut got = Vec::with_capacity(n);
            for (burst, spin) in bursts.iter().rev().cycle() {
                if got.len() == n { break; }
                for _ in 0..*burst { if let Some(v) = q.try_pop() { got.push(v); } }
                for _ in 0..*spin { std::hint::spin_loop(); }
            }
            producer.join().unwrap();
            prop_assert_eq!(got, (0..n).collect::<Vec<_>>());
            prop_assert_eq!(q.try_pop(), None);
        }
    }
}

#[cfg(loom)]
#[test]
fn loom_every_interleaving_hands_over_in_order_across_full_and_wrap() {
    loom::model(|| {
        let q = loom::sync::Arc::new(Spsc::new(2));
        let tx = q.clone();
        let producer = loom::thread::spawn(move || {
            for i in 0..3 {
                while tx.try_push(i).is_err() { loom::thread::yield_now(); }
            }
        });
        let mut got = Vec::new();
        while got.len() < 3 {
            match q.try_pop() {
                Some(v) => got.push(v),
                None => loom::thread::yield_now(),
            }
        }
        producer.join().unwrap();
        assert_eq!(got, [0, 1, 2]);
        assert_eq!(q.try_pop(), None);
    });
}
