#![cfg(not(loom))]

use algors_queue::mpmc::new_mpmc;
use algors_utils::waiter::YieldWait;
use proptest::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

#[cfg(feature = "std")]
use algors_utils::waiter::BlockingWait;

#[test]
#[cfg(feature = "std")]
fn test_mpmc_blocking_contention() {
    let (c, p) = new_mpmc::<usize>(4); // size 16
    let num_producers = 4;
    let num_consumers = 4;
    let items_per_thread = 1000;
    let total_items = num_producers * items_per_thread;

    // Use the most complex waiter to stress the notification logic.
    let waiter = Arc::new(BlockingWait::default());
    let count = Arc::new(AtomicUsize::new(0));

    thread::scope(|s| {
        for _ in 0..num_producers {
            let p = p.clone();
            let waiter = waiter.clone();
            s.spawn(move || {
                for i in 0..items_per_thread {
                    p.push(i, waiter.as_ref(), waiter.as_ref()).unwrap();
                }
            });
        }

        for _ in 0..num_consumers {
            let c = c.clone();
            let waiter = waiter.clone();
            let count = &count;
            s.spawn(move || {
                for _ in 0..items_per_thread {
                    let _ = c.pop(waiter.as_ref(), waiter.as_ref()).unwrap();
                    count.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });

    assert_eq!(count.load(Ordering::Relaxed), total_items);
}

#[test]
fn test_mpmc_drop_safety() {
    static DROPPED_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, PartialEq)]
    struct Droppable;
    impl Drop for Droppable {
        fn drop(&mut self) {
            DROPPED_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    let (c, p) = new_mpmc::<Droppable>(4); // size 16

    // Push 10 , pop 5.
    for _ in 0..10 {
        p.try_push(Droppable).map_err(|_| "push failed").unwrap();
    }
    for _ in 0..5 {
        let _ = c.try_pop().expect("pop failed");
    }

    assert_eq!(DROPPED_COUNT.load(Ordering::SeqCst), 5);

    // Dropping c and p should drop the remaining 5 items in the inner queue.
    drop(c);
    drop(p);

    assert_eq!(DROPPED_COUNT.load(Ordering::SeqCst), 10);
}

proptest! {
    #[test]
    fn test_mpmc_invariants(
        num_items in 1..2000usize,
        pow in 1..10u8 // Test capacities from 2 to 1024
    ) {
        let (c, p) = new_mpmc::<usize>(pow);
        let sum_received = Arc::new(AtomicUsize::new(0));
        let count_received = Arc::new(AtomicUsize::new(0));
        let waiter = YieldWait::default();

        thread::scope(|s| {
            let p_clone = p.clone();
            let waiter_ref = &waiter;
            s.spawn(move || {
                for i in 0..num_items {
                    p_clone.push(i, waiter_ref, waiter_ref).unwrap();
                }
            });

            let c_clone = c.clone();
            let sum_received_clone = sum_received.clone();
            let count_received_clone = count_received.clone();
            s.spawn(move || {
                for _ in 0..num_items {
                    let val = c_clone.pop(waiter_ref, waiter_ref).unwrap();
                    sum_received_clone.fetch_add(val, Ordering::Relaxed);
                    count_received_clone.fetch_add(1, Ordering::SeqCst);
                }
            });
        });

        let expected_sum = num_items * (num_items - 1) / 2; // n * (n - 1) / 2
        assert_eq!(sum_received.load(Ordering::Relaxed), expected_sum);
        assert_eq!(count_received.load(Ordering::Relaxed), num_items);
    }
}
