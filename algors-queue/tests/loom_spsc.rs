#![cfg(loom)]

use algors_queue::spsc::new_spsc;
use loom::thread;
extern crate alloc;
use alloc::string::String;

#[test]
fn loom_spsc_concurrent_push_pop() {
    loom::model(|| {
        // Size 2 buffer to test wrap-around and boundary conditions.
        let (mut c, mut p) = new_spsc::<usize>(1); // size 2

        let t1 = thread::spawn(move || {
            for i in 1..=3 {
                while p.try_push(i).is_err() {
                    thread::yield_now();
                }
            }
        });

        let mut sum = 0;
        for _ in 0..3 {
            let mut v = c.try_pop();
            while v.is_none() {
                thread::yield_now();
                v = c.try_pop();
            }
            sum += v.unwrap();
        }
        assert_eq!(sum, 6); // 1 + 2 + 3

        t1.join().unwrap();
    });
}

#[test]
fn loom_spsc_drop_with_items() {
    loom::model(|| {
        let (mut c, mut p) = new_spsc::<String>(1); // size 2

        let t1 = thread::spawn(move || {
            let _ = p.try_push(String::from("first"));
            let _ = p.try_push(String::from("second"));
            p
        });

        let t2 = thread::spawn(move || {
            let _ = c.try_pop();
            c
        });

        let _p = t1.join().unwrap();
        let _c = t2.join().unwrap();
    });
}
