#![cfg(not(loom))]

use algors_queue::spsc::new_spsc;
use algors_utils::waiter::YieldWait;
use proptest::prelude::*;
use std::thread;

#[test]
fn test_spsc_cache_and_wrap() {
    let (mut c, mut p) = new_spsc::<usize>(2); // size 4
    let n = 1000;

    // This test forces the head_cache and tail_cache to be updated multiple times
    // and verifies that the wrap-around logic is correct.
    for i in 0..n {
        p.try_push(i).map_err(|_| "push failed").unwrap();
        assert_eq!(c.try_pop(), Some(i));
    }

    // fill the queue
    for i in 0..4 {
        p.try_push(i).map_err(|_| "push failed").unwrap();
    }
    assert!(p.try_push(99).is_err());

    for i in 0..4 {
        assert_eq!(c.try_pop(), Some(i));
    }
    assert_eq!(c.try_pop(), None);
}

#[test]
fn test_spsc_blocking_api() {
    let (mut c, mut p) = new_spsc::<usize>(1); // size 2
    let n = 10_000;
    let waiter = YieldWait::default();

    thread::scope(|s| {
        let waiter_ref = &waiter;
        s.spawn(move || {
            for i in 0..n {
                p.push(i, waiter_ref, waiter_ref).unwrap();
            }
        });

        s.spawn(move || {
            for i in 0..n {
                assert_eq!(c.pop(waiter_ref, waiter_ref).unwrap(), i);
            }
        });
    });
}

proptest! {
    #[test]
    fn test_spsc_fifo_integrity(
        values in prop::collection::vec(0..1000i32, 1..1000),
        pow in 1..10u8 // capacities from 2 to 1024
    ) {
        let (mut c, mut p) = new_spsc::<i32>(pow);
        let values_clone = values.clone();
        let waiter = YieldWait::default();

        thread::scope(|s| {
            let waiter_ref = &waiter;
            s.spawn(move || {
                for v in values {
                    p.push(v, waiter_ref, waiter_ref).unwrap();
                }
            });

            s.spawn(move || {
                for expected in values_clone {
                    assert_eq!(c.pop(waiter_ref, waiter_ref).unwrap(), expected);
                }
            });
        });
    }
}
