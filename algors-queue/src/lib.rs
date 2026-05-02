#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod mpmc;
pub mod slot;
pub mod spsc;

mod sync;
