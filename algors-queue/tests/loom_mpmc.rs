#![cfg(loom)]

use algors_queue::mpmc::new_mpmc;
use loom::thread;
extern crate alloc;
use alloc::string::String;

#[test]
fn loom_mpmc_concurrent_push_pop() {
    loom::model(|| {
        // Small buffer (size 2) to force contention and wrap-around.
        let (c, p) = new_mpmc::<usize>(1);

        let p1 = p.clone();
        let t1 = thread::spawn(move || {
            while p1.try_push(1).is_err() {
                thread::yield_now();
            }
        });

        let p2 = p.clone();
        let t2 = thread::spawn(move || {
            while p2.try_push(2).is_err() {
                thread::yield_now();
            }
        });

        let mut sum = 0;
        for _ in 0..2 {
            let mut v = c.try_pop();
            while v.is_none() {
                thread::yield_now();
                v = c.try_pop();
            }
            sum += v.unwrap();
        }

        t1.join().unwrap();
        t2.join().unwrap();

        assert_eq!(sum, 3);
    });
}

#[test]
fn loom_mpmc_drop_with_items() {
    loom::model(|| {
        let (c, p) = new_mpmc::<String>(1); // size 2

        let p1 = p.clone();
        let t1 = thread::spawn(move || {
            let _ = p1.try_push(String::from("hello"));
        });

        let p2 = p.clone();
        let t2 = thread::spawn(move || {
            let _ = p2.try_push(String::from("world"));
        });

        let c1 = c.clone();
        let t3 = thread::spawn(move || {
            let _ = c1.try_pop();
        });

        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();

        // Dropping the queue with items inside verifies memory safety (no leaks/double frees).
    });
}
